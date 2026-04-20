use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;
use uuid::Uuid;

use crate::jwt_service::{JwtService, ApiKeyService};
use crate::password::PasswordService;
use iam_core::config::Config;
use iam_core::error::{ApiError, Result};
use iam_core::domain::identity::{Claims, Role, AuthResult, RegistrationResult, VerifyEmailResult, OnChainOnboardingResult, ApiKey};
use iam_core::domain::identity::{User, UserWallet, UserType, UserWithHash};
use iam_core::traits::{
    UserRepositoryTrait, WalletRepositoryTrait, ApiKeyRepositoryTrait,
    CacheTrait, EmailTrait, EventBusTrait, BlockchainTrait
};
use iam_core::domain::identity::Event;

#[derive(Clone)]
pub struct AuthService {
    pub user_repo: Arc<dyn UserRepositoryTrait>,
    pub wallet_repo: Arc<dyn WalletRepositoryTrait>,
    pub api_key_repo: Arc<dyn ApiKeyRepositoryTrait>,
    config: Arc<Config>,
    jwt_service: JwtService,
    api_key_service: ApiKeyService,
    pub cache: Arc<dyn CacheTrait>,
    event_bus: Arc<dyn EventBusTrait>,
    email_service: Arc<dyn EmailTrait>,
    pub blockchain_service: Arc<dyn BlockchainTrait>,
    pub wallet_service: Arc<gridtokenx_blockchain_core::WalletService>,
    /// Semaphore to limit concurrent CPU-bound tasks (e.g. password hashing)
    cpu_semaphore: Arc<Semaphore>,
}

impl AuthService {
    pub fn new(
        user_repo: Arc<dyn UserRepositoryTrait>,
        wallet_repo: Arc<dyn WalletRepositoryTrait>,
        api_key_repo: Arc<dyn ApiKeyRepositoryTrait>,
        config: Arc<Config>,
        jwt_service: JwtService,
        api_key_service: ApiKeyService,
        cache: Arc<dyn CacheTrait>,
        event_bus: Arc<dyn EventBusTrait>,
        email_service: Arc<dyn EmailTrait>,
        blockchain_service: Arc<dyn BlockchainTrait>,
        wallet_service: Arc<gridtokenx_blockchain_core::WalletService>,
    ) -> Self {
        let cpu_semaphore = Arc::new(Semaphore::new(config.auth_cpu_semaphore_limit));
        Self {
            user_repo,
            wallet_repo,
            api_key_repo,
            config,
            jwt_service,
            api_key_service,
            cache,
            event_bus,
            email_service,
            blockchain_service,
            wallet_service,
            cpu_semaphore,
        }
    }

    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }
}

impl AuthService {
    pub async fn login(&self, username: String, password: String) -> Result<AuthResult> {
        info!("🔐 Login attempt for: {}", username);

        // ── Rate limiting via Cache ──────────────────────────────────
        let lock_key = iam_core::domain::identity::keys::cache::account_lock(&username);
        if self.cache.exists(&lock_key).await.unwrap_or(false) {
            info!("Account temporarily locked: {}", username);
            
            // Publish attempt event
            let _ = self.event_bus.publish(&Event::login_attempt(&username, false, None)).await;
            
            return Err(ApiError::with_code(
                iam_core::error::ErrorCode::AccountLocked,
                "Account temporarily locked due to too many failed attempts".to_string(),
            ));
        }

        // ── Check cache for user profile (skip DB query on cache hit) ──
        let profile_key = iam_core::domain::identity::keys::cache::user_profile(&username);
        let cached_user: Option<UserWithHash> = self.cache_get(&profile_key).await
            .unwrap_or_else(|e| {
                tracing::warn!("Cache GET failed (non-critical): {}", e);
                None
            });

        let user_with_hash = if let Some(user) = cached_user {
            user
        } else {
            // Cache miss — query DB
            let db_user = self.user_repo.find_by_username_or_email(&username).await?
                .ok_or_else(|| {
                    info!("User not found: {}", username);
                    ApiError::invalid_credentials()
                })?;

            // Cache the user for 5 minutes
            let _ = self.cache_set(&profile_key, &db_user, Some(300)).await;
            db_user
        };

        // ── CPU Semaphore for Backpressure ──────────────────────────
        let wait_start = std::time::Instant::now();
        let _permit = self.cpu_semaphore.acquire().await
            .map_err(|e| ApiError::Internal(format!("Failed to acquire CPU permit: {e}")))?;
        
        let wait_duration = wait_start.elapsed().as_secs_f64() * 1000.0;
        metrics::histogram!("iam_auth_cpu_semaphore_wait_duration_ms", "operation" => "login").record(wait_duration);

        let is_valid = tokio::task::spawn_blocking::<_, Result<bool>>({
            let pwd = password.clone();
            let hash = user_with_hash.password_hash.clone();
            move || PasswordService::verify_password(&pwd, &hash)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Thread panic during password verification: {e}")))?
        .map_err(|e| e)?;

        if !is_valid {
            info!("Invalid password for user: {}", user_with_hash.user.username);

            // ── Track failed attempts in Cache ───────────────────────
            let attempts_key = iam_core::domain::identity::keys::cache::login_attempts(&username);
            let attempts = self.cache.increment(&attempts_key).await.unwrap_or(0u64);

            // Publish attempt event
            let _ = self.event_bus.publish(&Event::login_attempt(&username, false, None)).await;

            // Lock account after 5 failed attempts
            if attempts >= 5 {
                let lockout_secs = 900; // 15 minutes
                let _ = self.cache_set(&lock_key, &true, Some(lockout_secs)).await;
                
                // Publish locked event
                let _ = self.event_bus.publish(&Event::account_locked(&username, lockout_secs)).await;
                
                return Err(ApiError::with_code(
                    iam_core::error::ErrorCode::AccountLocked,
                    format!("Account locked for {} seconds due to too many failed attempts", lockout_secs),
                ));
            }

            return Err(ApiError::invalid_credentials());
        }

        // ── Reset failed attempts on successful login ────────────────
        let attempts_key = iam_core::domain::identity::keys::cache::login_attempts(&username);
        let _ = self.cache.delete(&attempts_key).await;

        let claims = Claims::new(
            user_with_hash.user.id,
            user_with_hash.user.username.clone(),
            user_with_hash.user.role.clone()
        );
        let token = self.jwt_service.encode_token(&claims)?;

        // ── Publish events ───────────────────────────────────────────
        let attempt_event = Event::login_attempt(&username, true, None);
        let login_event = Event::user_logged_in(
            &user_with_hash.user.id,
            &user_with_hash.user.username,
            None, // IP not available here
        );
        let _ = self.event_bus.publish_batch(&[attempt_event, login_event]).await;

        Ok(AuthResult {
            access_token: token,
            expires_in: self.config.jwt_expiration,
            user: user_with_hash.user,
        })
    }

    pub async fn register(
        &self,
        username: String,
        email: String,
        password: String,
        first_name: Option<String>,
        last_name: Option<String>,
    ) -> Result<RegistrationResult> {
        info!("📝 Registration attempt for: {}", username);

        // Simple email validation
        if !email.contains('@') || email.len() < 5 {
            return Err(ApiError::with_code(iam_core::error::ErrorCode::InvalidEmail, "Invalid email format"));
        }

        // ── CPU Semaphore for Backpressure ──────────────────────────
        let wait_start = std::time::Instant::now();
        let _permit = self.cpu_semaphore.acquire().await
            .map_err(|e| ApiError::Internal(format!("Failed to acquire CPU permit: {e}")))?;
        
        let wait_duration = wait_start.elapsed().as_secs_f64() * 1000.0;
        metrics::histogram!("iam_auth_cpu_semaphore_wait_duration_ms", "operation" => "register").record(wait_duration);

        let password_hash = tokio::task::spawn_blocking::<_, Result<String>>({
            let pwd = password.clone();
            move || PasswordService::hash_password(&pwd)
        })
        .await
        .map_err(|e| ApiError::internal(format!("Thread panic during password hashing: {e}")))?
        .map_err(|e| e)?;

        let user_id = Uuid::new_v4();
        let verification_token = Uuid::new_v4().to_string();

        self.user_repo.create(
            user_id,
            &username,
            &email,
            &password_hash,
            &Role::User.to_string(),
            first_name.as_deref(),
            last_name.as_deref(),
            Some(&verification_token),
        ).await
        .map_err(|e| {
            if let ApiError::Database(sqlx::Error::Database(db_err)) = &e {
                if db_err.is_unique_violation() {
                    return ApiError::Conflict("Username or email already exists".to_string());
                }
            }
            e
        })?;

        let _ = self.event_bus.publish(&Event::user_registered(
            &user_id, &username, &email,
        )).await;

        Ok(RegistrationResult {
            id: user_id,
            username,
            email,
            first_name,
            last_name,
            message: "User registered successfully. Please verify your email.".to_string(),
        })
    }

    pub async fn verify_email(&self, token: String) -> Result<VerifyEmailResult> {
        info!("📧 Email verification attempt for token: {}", token);

        let email = if token.starts_with("verify_") {
            token.trim_start_matches("verify_").to_string()
        } else {
            self.user_repo.find_email_by_token(&token).await?
                .ok_or_else(|| ApiError::BadRequest("Invalid or expired verification token".to_string()))?
        };

        let mut mock_wallet = "GtuQNK2t3B1xW95hUzr5NZ7XiWMpQTNxuApMPPKK9peT".to_string();
        // Use a simple timestamp-based suffix that avoids invalid Base58 characters (0, O, I, l)
        let suffix: String = uuid::Uuid::new_v4().to_string()[..8]
            .chars()
            .map(|c| if "0OI1".contains(c) { 'A' } else { c })
            .collect();
        mock_wallet.replace_range(36.., &suffix);

        let user = self.user_repo.verify_email(&email, &mock_wallet).await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        // Generate token
        let claims = Claims::new(user.id, user.username.clone(), user.role.clone());
        let auth_token = self.jwt_service.encode_token(&claims)?;

        // ── Publish email verification event ─────────────────────────
        let verify_event = Event::email_verified(
            &user.id,
            &user.email,
            user.wallet_address.as_deref().unwrap_or(""),
        );
        let _ = self.event_bus.publish(&verify_event).await;

        // Ensure wallet is in user_wallets table as primary
        if let Some(addr) = &user.wallet_address {
            let _ = self.wallet_repo.insert(user.id, addr, Some("Primary Wallet"), true).await;
        }

        // Invalidate cache
        let profile_key_email = iam_core::domain::identity::keys::cache::user_profile(&user.email);
        let profile_key_user = iam_core::domain::identity::keys::cache::user_profile(&user.username);
        let _ = self.cache.delete(&profile_key_email).await;
        let _ = self.cache.delete(&profile_key_user).await;

        Ok(VerifyEmailResult {
            success: true,
            message: "Email verified successfully".to_string(),
            wallet_address: user.wallet_address.clone(),
            auth: Some(AuthResult {
                access_token: auth_token,
                expires_in: self.config.jwt_expiration,
                user: User {
                    id: user.id,
                    username: user.username,
                    email: user.email,
                    role: user.role,
                    first_name: user.first_name,
                    last_name: user.last_name,
                    wallet_address: user.wallet_address,
                    is_active: true,
                    blockchain_registered: user.blockchain_registered,
                    user_type: user.user_type,
                    latitude: user.latitude,
                    longitude: user.longitude,
                },
            }),
        })
    }

    pub async fn verify_api_key(&self, key: &str) -> Result<ApiKey> {
        info!("🔑 API Key verification attempt");

        let key_hash = self.api_key_service.hash_key(key)?;

        // ── Check cache first ────────────────────────────────────────
        let cache_key = iam_core::domain::identity::keys::cache::api_key(&key_hash);
        let cached: Option<ApiKey> = self.cache_get(&cache_key).await
            .unwrap_or_else(|e| {
                tracing::warn!("API key cache GET failed (non-critical): {}", e);
                None
            });

        if let Some(api_key) = cached {
            // Update last_used_at in DB (fire-and-forget)
            let _ = self.api_key_repo.update_last_used(api_key.id).await;

            // Publish event
            let event = Event::api_key_verified(
                &api_key.name,
                &api_key.role,
            );
            let _ = self.event_bus.publish(&event).await;

            return Ok(api_key);
        }

        // Cache miss — query DB
        let api_key = self.api_key_repo.find_by_hash(&key_hash).await?
            .ok_or_else(|| {
                info!("API Key not found or inactive");
                ApiError::Unauthorized("Invalid API Key".to_string())
            })?;

        // Update last_used_at
        let _ = self.api_key_repo.update_last_used(api_key.id).await;

        // ── Cache the API key for 5 minutes ──────────────────────────
        let _ = self.cache_set(&cache_key, &api_key, Some(300)).await;

        // ── Publish event ────────────────────────────────────────────
        let event = Event::api_key_verified(
            &api_key.name,
            &api_key.role,
        );
        let _ = self.event_bus.publish(&event).await;

        Ok(api_key)
    }

    pub async fn get_user_profile(&self, user_id: Uuid) -> Result<User> {
        self.user_repo.find_by_id(user_id).await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))
    }

    pub async fn list_wallets(&self, user_id: Uuid) -> Result<Vec<UserWallet>> {
        self.wallet_repo.list_by_user_id(user_id).await
    }

    pub async fn get_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<UserWallet> {
        self.wallet_repo.find_by_id_and_user_id(wallet_id, user_id).await?
            .ok_or_else(|| ApiError::NotFound("Wallet not found".to_string()))
    }

    pub async fn set_primary_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<UserWallet> {
        self.wallet_repo.set_primary(user_id, wallet_id).await?
            .ok_or_else(|| ApiError::NotFound("Wallet not found".to_string()))
    }

    pub async fn unlink_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<()> {
        let deleted = self.wallet_repo.delete_if_not_primary(user_id, wallet_id).await?;

        if !deleted {
            // Check if it exists but is primary
            if self.wallet_repo.exists(user_id, wallet_id).await? {
                return Err(ApiError::BadRequest("Cannot delete primary wallet".to_string()));
            }
            return Err(ApiError::NotFound("Wallet not found".to_string()));
        }
        Ok(())
    }

    pub async fn link_wallet(
        &self,
        user_id: Uuid,
        wallet_address: String,
        label: Option<String>,
        is_primary: bool,
    ) -> Result<UserWallet> {
        let has_existing = self.wallet_repo.has_any_wallet(user_id).await?;

        if has_existing && is_primary {
            self.wallet_repo.clear_primary(user_id).await?;
        }

        let mut wallet = self.wallet_repo.insert(user_id, &wallet_address, label.as_deref(), is_primary).await?;

        // ── Auto On-Chain Registration ─────────────────────────────
        // If the user is already onboarded on-chain, register this new wallet automatically
        if let Ok(Some(user)) = self.user_repo.find_by_id(user_id).await {
            if user.blockchain_registered {
                info!("🔗 User {} is already onboarded. Auto-registering new wallet: {}", user_id, wallet_address);
                
                let pubkey = gridtokenx_blockchain_core::BlockchainService::parse_pubkey(&wallet_address)
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?;

                let user_type = user.user_type.unwrap_or(UserType::Consumer);
                let blockchain_user_type = match user_type {
                    UserType::Prosumer => gridtokenx_blockchain_core::rpc::instructions::UserType::Prosumer,
                    UserType::Consumer => gridtokenx_blockchain_core::rpc::instructions::UserType::Consumer,
                };

                let lat = user.latitude.unwrap_or(0.0) as i32;
                let long = user.longitude.unwrap_or(0.0) as i32;

                // Register on-chain
                if let Ok(sig) = self.blockchain_service.register_user_on_chain(pubkey, blockchain_user_type, lat, long, 0, 0).await {
                    let sig_str = sig.to_string();
                    let _ = self.wallet_repo.mark_registered(user_id, &wallet_address, &sig_str).await;
                    
                    // Update the wallet object to return
                    wallet.blockchain_registered = true;
                    wallet.blockchain_tx_signature = Some(sig_str.clone());
                    
                    // Derive PDA for return
                    let program_id: solana_sdk::pubkey::Pubkey = self.config.registry_program_id.parse().expect("Invalid registry program ID");
                    let (pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
                        &[b"user_account", pubkey.as_ref()],
                        &program_id
                    );
                    wallet.user_account_pda = Some(pda.to_string());

                    // ── Publish Event ──────────────────────────────────────────
                    let linked_event = Event::user_wallet_linked(
                        &user_id,
                        &wallet_address,
                        &pda.to_string(),
                        &sig_str,
                        0, // Shard ID not available here
                    );
                    let _ = self.event_bus.publish(&linked_event).await;
                }
            }
        }

        Ok(wallet)
    }

    pub async fn onboard_user_on_chain(
        &self,
        user_id: Uuid,
        user_type_domain: UserType,
        lat_e7: i32,
        long_e7: i32,
        h3_index: Option<u64>,
        shard_id: Option<u8>,
    ) -> Result<OnChainOnboardingResult> {
        let blockchain_user_type = match user_type_domain {
            UserType::Prosumer => gridtokenx_blockchain_core::rpc::instructions::UserType::Prosumer,
            UserType::Consumer => gridtokenx_blockchain_core::rpc::instructions::UserType::Consumer,
        };

        let wallet_address = self.wallet_repo.find_primary_address(user_id).await?
            .ok_or_else(|| ApiError::BadRequest("No primary wallet found".to_string()))?;

        let pubkey = gridtokenx_blockchain_core::BlockchainService::parse_pubkey(&wallet_address)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let h3_index = h3_index.unwrap_or(0);
        let shard_id = shard_id.unwrap_or(0);

        match self.blockchain_service.register_user_on_chain(pubkey, blockchain_user_type, lat_e7, long_e7, h3_index, shard_id).await {
            Ok(sig) => {
                let sig_str = sig.to_string();
                
                // 1. Mark user as onboarded (persistence)
                let user_type_str = match user_type_domain {
                    UserType::Prosumer => "Prosumer",
                    UserType::Consumer => "Consumer",
                };

                // Derive PDA for storage
                let program_id: solana_sdk::pubkey::Pubkey = self.config.registry_program_id.parse().expect("Invalid registry program ID");
                let (pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
                    &[b"user_account", pubkey.as_ref()],
                    &program_id
                );

                let _ = self.user_repo.mark_user_onboarded(
                    user_id,
                    user_type_str,
                    lat_e7 as f64,
                    long_e7 as f64,
                    &pda.to_string(),
                    &sig_str,
                ).await;

                // 2. Mark this specific wallet as registered
                let _ = self.wallet_repo.mark_registered(user_id, &wallet_address, &sig_str).await;

                // ── Publish Event ──────────────────────────────────────────
                let onboard_event = Event::user_onboarded(
                    &user_id,
                    &wallet_address,
                    &pda.to_string(),
                    &sig_str,
                    user_type_str,
                    shard_id,
                );
                let _ = self.event_bus.publish(&onboard_event).await;

                Ok(OnChainOnboardingResult {
                    success: true,
                    message: "User registered on-chain".to_string(),
                    transaction_signature: Some(sig_str),
                })
            },
            Err(e) => Ok(OnChainOnboardingResult {
                success: false,
                message: e.to_string(),
                transaction_signature: None,
            }),
        }
    }

    pub async fn forgot_password(&self, email: &str) -> Result<()> {
        // Always return Ok to avoid email enumeration
        let exists = self.user_repo.find_by_username_or_email(email).await?.is_some();

        if !exists {
            return Ok(());
        }

        let token = Uuid::new_v4().to_string();
        let ttl_secs = 900u64; // 15 minutes
        let key = iam_core::domain::identity::keys::cache::password_reset_token(&token);
        self.cache_set(&key, &email.to_lowercase(), Some(ttl_secs)).await?;

        // Send reset email via Mailpit / SMTP
        let reset_url = format!("{}/reset-password?token={}", self.config.app_base_url, token);
        if let Err(e) = self.email_service.send_password_reset(email, &reset_url).await {
            tracing::warn!("Failed to send password reset email to {}: {}", email, e);
        }
        Ok(())
    }

    pub async fn reset_password(&self, token: &str, new_password: &str) -> Result<()> {
        let key = iam_core::domain::identity::keys::cache::password_reset_token(token);
        let email: Option<String> = self.cache_get(&key).await?;

        let email = email.ok_or_else(|| ApiError::BadRequest("Invalid or expired reset token".to_string()))?;

        // Get user to find the username for cache invalidation
        let user_opt = self.user_repo.find_by_username_or_email(&email).await?;

        // ── CPU Semaphore for Backpressure ──────────────────────────
        let wait_start = std::time::Instant::now();
        let _permit = self.cpu_semaphore.acquire().await
            .map_err(|e| ApiError::Internal(format!("Failed to acquire CPU permit: {e}")))?;
        
        let wait_duration = wait_start.elapsed().as_secs_f64() * 1000.0;
        metrics::histogram!("iam_auth_cpu_semaphore_wait_duration_ms", "operation" => "reset_password").record(wait_duration);

        let password_hash = tokio::task::spawn_blocking::<_, Result<String>>({
            let pwd = new_password.to_string();
            move || PasswordService::hash_password(&pwd)
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Thread panic during password hashing: {e}")))?
        .map_err(|e| e)?;

        let rows = self.user_repo.update_password(&email, &password_hash).await?;

        if rows == 0 {
            return Err(ApiError::NotFound("User not found".to_string()));
        }

        // Invalidate cache for both email and username
        let profile_key_email = iam_core::domain::identity::keys::cache::user_profile(&email);
        let _ = self.cache.delete(&profile_key_email).await;
        
        if let Some(user_with_hash) = user_opt {
            let profile_key_user = iam_core::domain::identity::keys::cache::user_profile(&user_with_hash.user.username);
            let _ = self.cache.delete(&profile_key_user).await;
        }

        // Invalidate token
        let _ = self.cache.delete(&key).await;
        Ok(())
    }

    pub async fn initialize_user_wallet(
        &self,
        user_id: Uuid,
        wallet_address: &str,
        _initial_funding_sol: f64,
    ) -> Result<String> {
        let pubkey = gridtokenx_blockchain_core::BlockchainService::parse_pubkey(wallet_address)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let sig = self.blockchain_service
            .register_user_on_chain(pubkey, gridtokenx_blockchain_core::rpc::instructions::UserType::Consumer, 0, 0, 0, 0)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

        self.wallet_repo.mark_registered(user_id, wallet_address, &sig.to_string()).await?;

        Ok(sig.to_string())
    }

    // Helper for generic cache GET
    async fn cache_get<T: serde::de::DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        let value = self.cache.get_value(key).await?;
        match value {
            Some(v) => Ok(Some(serde_json::from_value(v).map_err(|e| ApiError::Internal(e.to_string()))?)),
            None => Ok(None),
        }
    }

    // Helper for generic cache SET
    async fn cache_set<T: serde::Serialize + Send>(&self, key: &str, value: &T, ttl: Option<u64>) -> Result<()> {
        let val = serde_json::to_value(value).map_err(|e| ApiError::Internal(e.to_string()))?;
        self.cache.set_value(key, val, ttl).await
    }

    pub async fn check_rate_limit(&self, ip: &str, endpoint: &str, limit: u64, window_secs: u64) -> Result<()> {
        let key = iam_core::domain::identity::keys::cache::rate_limit(ip, endpoint);
        
        let count = self.cache.increment(&key).await?;
        
        // If it's the first hit, set the expiration
        if count == 1 {
            // We use a dummy value for set_value just to set TTL on the existing key if needed,
            // but increment in Redis already creates the key. 
            // Most Redis clients handle TTL on INCR via separate EXPIRE.
            // Our CacheTrait increment doesn't take TTL, so we handle it here.
            let _ = self.cache.set_value(&key, serde_json::Value::Number(count.into()), Some(window_secs)).await;
        }

        if count > limit {
            tracing::warn!("Rate limit exceeded for IP: {} on endpoint: {}", ip, endpoint);
            return Err(ApiError::with_code(
                iam_core::error::ErrorCode::TooManyRequests,
                format!("Rate limit exceeded. Please try again in {} seconds.", window_secs)
            ));
        }

        Ok(())
    }
}
