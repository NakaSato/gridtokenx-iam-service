#[cfg(test)]
mod tests {
    use sqlx::PgPool;
    use uuid::Uuid;
    use iam_core::traits::UserRepositoryTrait;
    use iam_core::domain::identity::NewUser;
    use crate::repository::UserRepository;

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_user_repository_crud(pool: PgPool) -> iam_core::error::Result<()> {
        let repo = UserRepository::new(pool);
        let id = Uuid::new_v4();
        let username = format!("user_{}", Uuid::new_v4().simple());
        let email = format!("{}@test.com", Uuid::new_v4().simple());
        let password_hash = "hashed_password";
        let role = "user";

        // 1. Create User — registration inserts an INACTIVE row (is_active = false);
        // the account is dormant until email verification activates it.
        repo.create(NewUser {
            id,
            username: &username,
            email: &email,
            password_hash,
            role,
            first_name: Some("First"),
            last_name: Some("Last"),
            verification_token: None,
        }).await?;

        // 1b. The finders filter `is_active = true`, so a freshly created (unverified)
        // user is intentionally not yet resolvable. Guards that create-inactive →
        // verify-activate contract.
        assert!(
            repo.find_by_username_or_email(&username).await?.is_none(),
            "unverified user must not be findable until activated"
        );

        // 1c. Activate via email verification (flips is_active = true).
        repo.verify_email(&email).await?.expect("verify_email should return the activated user");

        // 2. Find by username
        let user = repo.find_by_username_or_email(&username).await?.expect("User not found by username");
        assert_eq!(user.user.id, id);
        assert_eq!(user.user.username, username);
        assert_eq!(user.password_hash, password_hash);

        // 3. Find by email
        let user = repo.find_by_username_or_email(&email).await?.expect("User not found by email");
        assert_eq!(user.user.id, id);

        // 4. Find by ID
        let user = repo.find_by_id(id).await?.expect("User not found by ID");
        assert_eq!(user.username, username);

        // 5. Update password
        let new_hash = "new_hashed_password";
        let affected = repo.update_password(&email, new_hash).await?;
        assert_eq!(affected, 1);

        let user = repo.find_by_username_or_email(&email).await?.expect("User not found after password update");
        assert_eq!(user.password_hash, new_hash);

        // 6. Health Check
        repo.health_check().await?;

        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_wallet_repository(pool: PgPool) -> iam_core::error::Result<()> {
        use crate::repository::WalletRepository;
        use iam_core::traits::WalletRepositoryTrait;

        let user_repo = UserRepository::new(pool.clone());
        let wallet_repo = WalletRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        user_repo.create(NewUser {
            id: user_id,
            username: "walletuser",
            email: "wallet@test.com",
            password_hash: "hash",
            role: "user",
            first_name: None,
            last_name: None,
            verification_token: None,
        }).await?;

        // 1. Link Wallet
        let address = "SolWallet123";
        let label = "My Wallet";
        let wallet = wallet_repo.insert(user_id, address, Some(label), true).await?;
        assert_eq!(wallet.wallet_address, address);
        assert_eq!(wallet.label.as_deref(), Some(label));
        assert!(wallet.is_primary);

        // 2. List Wallets
        let wallets = wallet_repo.list_by_user_id(user_id).await?;
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].wallet_address, address);

        // 3. Mark Registered
        let signature = "tx_signature_abc";
        wallet_repo.mark_registered(user_id, address, signature).await?;
        
        let wallets = wallet_repo.list_by_user_id(user_id).await?;
        assert!(wallets[0].blockchain_registered);
        assert_eq!(wallets[0].blockchain_tx_signature.as_deref(), Some(signature));

        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_outbox_repository_lifecycle(pool: PgPool) -> iam_core::error::Result<()> {
        use crate::repository::OutboxRepository;
        use iam_core::domain::identity::Event;
        use iam_core::traits::OutboxRepositoryTrait;

        let outbox = OutboxRepository::new(pool.clone());

        // 1. Enqueue → durably stored as PENDING and visible to the drain query.
        let event = Event::verification_email_requested(
            &Uuid::new_v4(),
            "outboxuser",
            "outbox@test.com",
            "verify-token-xyz",
        );
        outbox.enqueue(&event).await?;

        let pending = outbox.fetch_pending(10).await?;
        assert_eq!(pending.len(), 1, "enqueued event should be pending");
        let record = pending.into_iter().next().expect("one pending record");
        assert_eq!(record.event_type, "VerificationEmailRequested");

        // Payload round-trips back into a full Event (what the worker delivers).
        let decoded: Event = serde_json::from_value(record.payload.clone())
            .expect("payload deserializes to Event");
        assert_eq!(decoded.event_type, "VerificationEmailRequested");
        assert_eq!(
            decoded.data.as_ref().and_then(|d| d.get("token")).and_then(|v| v.as_str()),
            Some("verify-token-xyz"),
        );

        // 2. mark_processed → leaves the pending set, row is PROCESSED.
        outbox.mark_processed(record.id).await?;
        assert!(
            outbox.fetch_pending(10).await?.is_empty(),
            "processed event must not be re-fetched",
        );
        let status: String =
            sqlx::query_scalar("SELECT status FROM iam_outbox_events WHERE id = $1")
                .bind(record.id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(status, "PROCESSED");

        // 3. mark_failed → stays PENDING for retry (the no-loss-on-blip guarantee)
        //    until it exhausts the attempt budget, then is quarantined FAILED.
        let event2 = Event::user_registered(&Uuid::new_v4(), "retryuser", "retry@test.com");
        outbox.enqueue(&event2).await?;
        let id2 = outbox
            .fetch_pending(10)
            .await?
            .into_iter()
            .next()
            .expect("second event pending")
            .id;

        // MAX_ATTEMPTS = 10: first 9 failures keep it retryable.
        for expected_attempts in 1..=9 {
            outbox.mark_failed(id2).await?;
            let (status, attempts): (String, i32) = sqlx::query_as(
                "SELECT status, attempts FROM iam_outbox_events WHERE id = $1",
            )
            .bind(id2)
            .fetch_one(&pool)
            .await?;
            assert_eq!(status, "PENDING", "should still retry on attempt {expected_attempts}");
            assert_eq!(attempts, expected_attempts);
        }
        assert_eq!(
            outbox.fetch_pending(10).await?.len(),
            1,
            "still retryable after 9 failures",
        );

        // 10th failure crosses the budget → quarantined, no longer retried.
        outbox.mark_failed(id2).await?;
        let status: String =
            sqlx::query_scalar("SELECT status FROM iam_outbox_events WHERE id = $1")
                .bind(id2)
                .fetch_one(&pool)
                .await?;
        assert_eq!(status, "FAILED", "exhausted attempts must quarantine the row");
        assert!(
            outbox.fetch_pending(10).await?.is_empty(),
            "quarantined event must not be retried",
        );

        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_api_key_repository(pool: PgPool) -> iam_core::error::Result<()> {
        use crate::repository::ApiKeyRepository;
        use iam_core::traits::ApiKeyRepositoryTrait;

        let user_repo = UserRepository::new(pool.clone());
        let api_key_repo = ApiKeyRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        user_repo.create(NewUser {
            id: user_id,
            username: "keyuser",
            email: "key@test.com",
            password_hash: "hash",
            role: "user",
            first_name: None,
            last_name: None,
            verification_token: None,
        }).await?;

        // Manual insert since we don't have a create method in the trait yet (usually handled by a service)
        let key_id = Uuid::new_v4();
        let hash = "api_key_hash_123";
        sqlx::query("INSERT INTO api_keys (id, name, key_hash, is_active) VALUES ($1, $2, $3, true)")
            .bind(key_id)
            .bind("Test Key")
            .bind(hash)
            .execute(api_key_repo.get_pool())
            .await?;

        // 1. Find by Hash
        let api_key = api_key_repo.find_by_hash(hash).await?.expect("API key not found");
        assert_eq!(api_key.id, key_id);
        assert_eq!(api_key.name, "Test Key");

        // 2. Update Last Used
        api_key_repo.update_last_used(key_id).await?;
        
        Ok(())
    }
}
