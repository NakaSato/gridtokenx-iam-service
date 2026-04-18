#[cfg(test)]
mod tests {
    use sqlx::PgPool;
    use uuid::Uuid;
    use iam_core::traits::UserRepositoryTrait;
    use crate::repository::UserRepository;

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_user_repository_crud(pool: PgPool) -> iam_core::error::Result<()> {
        let repo = UserRepository::new(pool);
        let id = Uuid::new_v4();
        let username = format!("user_{}", Uuid::new_v4().simple());
        let email = format!("{}@test.com", Uuid::new_v4().simple());
        let password_hash = "hashed_password";
        let role = "user";

        // 1. Create User
        repo.create(id, &username, &email, password_hash, role, Some("First"), Some("Last")).await?;

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

        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_wallet_repository(pool: PgPool) -> iam_core::error::Result<()> {
        use crate::repository::WalletRepository;
        use iam_core::traits::WalletRepositoryTrait;

        let user_repo = UserRepository::new(pool.clone());
        let wallet_repo = WalletRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        user_repo.create(user_id, "walletuser", "wallet@test.com", "hash", "user", None, None).await?;

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
    async fn test_api_key_repository(pool: PgPool) -> iam_core::error::Result<()> {
        use crate::repository::ApiKeyRepository;
        use iam_core::traits::ApiKeyRepositoryTrait;

        let user_repo = UserRepository::new(pool.clone());
        let api_key_repo = ApiKeyRepository::new(pool);
        
        let user_id = Uuid::new_v4();
        user_repo.create(user_id, "keyuser", "key@test.com", "hash", "user", None, None).await?;

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
