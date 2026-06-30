use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;
use uuid::Uuid;

use crate::jwt_service::{JwtService, ApiKeyService};
use crate::password::PasswordService;
use iam_core::config::Config;
use iam_core::error::{ApiError, Result};
use iam_core::domain::identity::{Claims, Role, AuthResult, RegistrationResult, ResendVerificationResult, VerifyEmailResult, OnChainOnboardingResult, ApiKey};
use iam_core::domain::identity::{User, UserWallet, UserType, UserWithHash};
use iam_core::traits::{
    UserRepositoryTrait, WalletRepositoryTrait, ApiKeyRepositoryTrait,
    CacheTrait, EventBusTrait, BlockchainTrait
};
use iam_core::domain::identity::Event;

/// Minimum delay between successive verification-email resends for one account.
/// Throttles email-bombing without blocking a legitimate retry for long.
const RESEND_VERIFICATION_COOLDOWN_SECS: u64 = 60;

/// Failed-login attempts before the account is temporarily locked.
const LOGIN_MAX_FAILED_ATTEMPTS: u64 = 5;
/// Sliding window over which failed-login attempts are counted, and the lockout
/// duration once the threshold is hit. The attempts counter carries this TTL so
/// old failures decay instead of accumulating forever (which previously caused
/// permanent re-lock after a single mistype once the count had ever reached 5).
const LOGIN_LOCKOUT_SECS: u64 = 900; // 15 minutes

#[derive(Clone)]
pub struct AuthService {
    pub user_repo: Arc<dyn UserRepositoryTrait>,
    pub wallet_repo: Arc<dyn WalletRepositoryTrait>,
    pub api_key_repo: Arc<dyn ApiKeyRepositoryTrait>,
    pub config: Arc<Config>,
    jwt_service: JwtService,
    api_key_service: ApiKeyService,
    pub cache: Arc<dyn CacheTrait>,
    event_bus: Arc<dyn EventBusTrait>,
    pub blockchain_service: Arc<dyn BlockchainTrait>,
    pub wallet_service: Arc<gridtokenx_blockchain_core::WalletService>,
    /// Semaphore to limit concurrent CPU-bound tasks (e.g. password hashing)
    cpu_semaphore: Arc<Semaphore>,
}

impl AuthService {
    /// Creates a new instance of the AuthService with all its dependencies.
    pub fn new(
        user_repo: Arc<dyn UserRepositoryTrait>,
        wallet_repo: Arc<dyn WalletRepositoryTrait>,
        api_key_repo: Arc<dyn ApiKeyRepositoryTrait>,
        config: Arc<Config>,
        jwt_service: JwtService,
        api_key_service: ApiKeyService,
        cache: Arc<dyn CacheTrait>,
        event_bus: Arc<dyn EventBusTrait>,
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
            blockchain_service,
            wallet_service,
            cpu_semaphore,
        }
    }

    /// Returns a reference to the JWT service used by this auth service.
    pub fn jwt_service(&self) -> &JwtService {
        &self.jwt_service
    }

    /// Issues a fresh access token for an already-authenticated user.
    ///
    /// The caller must present a still-valid (non-expired) token — the
    /// `AuthenticatedUser` extractor validates and rejects expired tokens before
    /// this is reached. A new token is minted from the existing claims with a
    /// fresh expiry. Note: expired tokens cannot be refreshed (decode rejects
    /// them), so clients must refresh **proactively**, before expiry. Returns the
    /// new token and its lifetime in seconds.
    pub fn refresh_token(&self, claims: &Claims) -> Result<(String, i64)> {
        let new_claims = Claims::new(claims.sub, claims.username.clone(), claims.role.clone());
        let token = self.jwt_service.encode_token(&new_claims)?;
        Ok((token, self.config.jwt_expiration))
    }
}

impl AuthService {
    /// Authenticates a user with username/email and password.
    /// 
    /// This method performs rate-limiting checks, credential verification (using bcrypt in a blocking thread),
    /// and issues a JWT token upon success. It also publishes domain events for audit trails.
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

            // ── Track failed attempts in Cache (sliding window) ──────
            // TTL on the counter so stale failures decay; otherwise the count
            // accumulates forever and a single mistype re-locks indefinitely.
            let attempts_key = iam_core::domain::identity::keys::cache::login_attempts(&username);
            let attempts = self.cache
                .increment_with_ttl(&attempts_key, LOGIN_LOCKOUT_SECS)
                .await
                .unwrap_or(0u64);

            // Publish attempt event
            let _ = self.event_bus.publish(&Event::login_attempt(&username, false, None)).await;

            // Lock account after too many failed attempts
            if attempts >= LOGIN_MAX_FAILED_ATTEMPTS {
                let _ = self.cache_set(&lock_key, &true, Some(LOGIN_LOCKOUT_SECS)).await;

                // Clear the counter so that when the lock expires the account
                // starts from a clean slate instead of re-locking on the next
                // single failure.
                let _ = self.cache.delete(&attempts_key).await;

                // Publish locked event
                let _ = self.event_bus.publish(&Event::account_locked(&username, LOGIN_LOCKOUT_SECS)).await;

                return Err(ApiError::with_code(
                    iam_core::error::ErrorCode::AccountLocked,
                    format!("Account locked for {} seconds due to too many failed attempts", LOGIN_LOCKOUT_SECS),
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

    /// Registers a new user account.
    /// 
    /// This method validates the username/email, hashes the password, creates the user record,
    /// and generates an email verification token.
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

        let role = Role::User.to_string();
        self.user_repo.create(iam_core::domain::identity::NewUser {
            id: user_id,
            username: &username,
            email: &email,
            password_hash: &password_hash,
            role: &role,
            first_name: first_name.as_deref(),
            last_name: last_name.as_deref(),
            verification_token: Some(&verification_token),
        }).await
        .map_err(|e| {
            if let ApiError::Database(sqlx::Error::Database(db_err)) = &e {
                if db_err.is_unique_violation() {
                    return ApiError::Conflict("Username or email already exists".to_string());
                }
            }
            e
        })?;

        if let Err(e) = self.event_bus.publish(&Event::user_registered(
            &user_id, &username, &email,
        )).await {
            tracing::warn!("failed to publish UserRegistered event: {}", e);
        }

        // Hand the verification token to the notification service so it can
        // send the click-to-verify email.
        if let Err(e) = self.event_bus.publish(&Event::verification_email_requested(
            &user_id, &username, &email, &verification_token,
        )).await {
            tracing::warn!("failed to publish VerificationEmailRequested event: {}", e);
        }

        Ok(RegistrationResult {
            id: user_id,
            username,
            email,
            first_name,
            last_name,
            message: "User registered successfully. Please verify your email.".to_string(),
        })
    }

    /// Verifies a user's email address using a verification token.
    /// 
    /// Upon success, the account is activated and a primary Solana wallet is initialized.
    pub async fn verify_email(&self, token: String) -> Result<VerifyEmailResult> {
        info!("📧 Email verification attempt for token: {}", token);

        // The `verify_<email>` shortcut skips the DB token lookup entirely, so
        // anyone who knows an email could activate the account and mint its
        // custodial wallet. Permit it ONLY outside production (dev/test convenience
        // for skipping the email round-trip); production always requires a real,
        // server-issued token.
        let dev_shortcut = !self.config.environment.eq_ignore_ascii_case("production");
        let email = if dev_shortcut && token.starts_with("verify_") {
            token.trim_start_matches("verify_").to_string()
        } else {
            self.user_repo.find_email_by_token(&token).await?
                .ok_or_else(|| ApiError::BadRequest("Invalid or expired verification token".to_string()))?
        };

        let mut user = self.user_repo.verify_email(&email).await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

        // Auto-provision a custodial wallet so the user can trade immediately
        // without bringing their own keypair. Failure must not block
        // verification — the user can still link a wallet manually later.
        let has_wallet = self.wallet_repo.has_any_wallet(user.id).await.unwrap_or_else(|e| {
            // Fail safe: on a DB error assume a wallet exists so we never
            // double-provision; surface the error so the blip is visible.
            tracing::warn!("has_any_wallet check failed for user {}, skipping provisioning: {}", user.id, e);
            true
        });
        if user.wallet_address.is_none() && !has_wallet {
            match self.provision_custodial_wallet(&user).await {
                Ok(wallet) => {
                    info!("🪪 Provisioned custodial wallet {} for user {}", wallet.wallet_address, user.id);
                    user.wallet_address = Some(wallet.wallet_address);
                    user.blockchain_registered = wallet.blockchain_registered;
                }
                Err(e) => {
                    tracing::warn!("Custodial wallet provisioning failed for user {}: {}", user.id, e);
                }
            }
        }

        // Generate token
        let claims = Claims::new(user.id, user.username.clone(), user.role.clone());
        let auth_token = self.jwt_service.encode_token(&claims)?;

        // ── Publish email verification event ─────────────────────────
        let verify_event = Event::email_verified(
            &user.id,
            &user.username,
            &user.email,
            user.wallet_address.as_deref().unwrap_or(""),
        );
        let _ = self.event_bus.publish(&verify_event).await;

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

    /// Provisions a custodial Solana wallet for a freshly verified user:
    /// generates a keypair, encrypts it under the service secrets
    /// (AES-256-GCM, current KDF version), persists the key material, links
    /// the address as the user's primary wallet, and best-effort registers
    /// the user on-chain via Chain Bridge.
    async fn provision_custodial_wallet(&self, user: &User) -> Result<UserWallet> {
        use base64::Engine as _;
        use gridtokenx_blockchain_core::wallet::CURRENT_KDF_VERSION;
        use solana_sdk::signature::Signer as _;

        let keypair = gridtokenx_blockchain_core::WalletService::create_keypair();
        let pubkey = keypair.pubkey();
        let wallet_address = pubkey.to_string();

        // Encrypt the full 64-byte keypair (same layout as a Solana wallet
        // file) under the service secrets. No user password is involved —
        // custody is service-side by design so IAM can sign on the user's
        // behalf.
        let (enc_b64, salt_b64, iv_b64) =
            gridtokenx_blockchain_core::WalletService::encrypt_private_key_versioned(
                CURRENT_KDF_VERSION,
                &self.config.encryption_secret,
                &self.config.master_secret,
                &keypair.to_bytes(),
            )
            .map_err(|e| ApiError::internal(e.to_string()))?;

        let b64 = base64::engine::general_purpose::STANDARD;
        let encrypted = b64.decode(&enc_b64).map_err(|e| ApiError::internal(e.to_string()))?;
        let salt = b64.decode(&salt_b64).map_err(|e| ApiError::internal(e.to_string()))?;
        let iv = b64.decode(&iv_b64).map_err(|e| ApiError::internal(e.to_string()))?;

        // Insert the wallet row and write the key material + primary address in a
        // single DB transaction: a key without a wallet (or an address without a
        // recoverable key) must never be observable, even across a crash.
        let wallet = self
            .wallet_repo
            .persist_custodial_wallet(
                user.id,
                &wallet_address,
                Some("Custodial"),
                &encrypted,
                &salt,
                &iv,
                i16::from(CURRENT_KDF_VERSION),
            )
            .await?;

        // ── On-chain registration (Registry PDA) — deferred, detached ──
        // register_user_on_chain retries with backoff (~14s) and the PDA
        // confirmation poll adds up to ~15s. Awaiting that here would block the
        // email-verification HTTP response for up to ~30s on a slow/oversubscribed
        // validator (the e2e golden path tripped its 10s client timeout this way).
        // The wallet is already persisted and usable off-chain, and registration is
        // best-effort + separately retryable (InitializeUserWallet / POST
        // /me/registration), so it must not gate verification. Spawn it; the task
        // flips `blockchain_registered` in the DB once the tx confirms.
        let user_type = user.user_type.unwrap_or(UserType::Consumer);
        self.spawn_onchain_registration(
            user.id,
            pubkey,
            wallet_address.clone(),
            user_type,
            user.latitude.unwrap_or(0.0),
            user.longitude.unwrap_or(0.0),
        );

        // Returned wallet reflects the just-persisted row (blockchain_registered =
        // false); the detached task updates the DB once the tx confirms.
        Ok(wallet)
    }

    /// Spawns a detached task that registers a freshly provisioned custodial
    /// wallet on-chain (Registry PDA) and, once the tx confirms, flips the
    /// `blockchain_registered` / onboarding columns in the DB.
    ///
    /// Detached because `register_user_on_chain`'s retry-with-backoff (~14s) plus
    /// the PDA confirmation poll (~15s) must never block the caller (email
    /// verification). Best-effort: every failure is logged and remains retryable
    /// via `InitializeUserWallet` / `POST /me/registration`.
    fn spawn_onchain_registration(
        &self,
        user_id: Uuid,
        pubkey: solana_sdk::pubkey::Pubkey,
        wallet_address: String,
        user_type: UserType,
        latitude: f64,
        longitude: f64,
    ) {
        let blockchain_service = Arc::clone(&self.blockchain_service);
        let wallet_repo = Arc::clone(&self.wallet_repo);
        let user_repo = Arc::clone(&self.user_repo);
        let config = Arc::clone(&self.config);
        tokio::spawn(async move {
            let (blockchain_user_type, user_type_str) = match user_type {
                UserType::Prosumer => (
                    gridtokenx_blockchain_core::rpc::instructions::UserType::Prosumer,
                    "Prosumer",
                ),
                UserType::Consumer => (
                    gridtokenx_blockchain_core::rpc::instructions::UserType::Consumer,
                    "Consumer",
                ),
            };
            match blockchain_service
                .register_user_on_chain(pubkey, blockchain_user_type, 0, 0, 0, 0)
                .await
            {
                Ok(sig) if Self::confirm_registered(&blockchain_service, &config, &pubkey).await => {
                    let sig_str = sig.to_string();
                    if let Err(e) = wallet_repo
                        .mark_registered(user_id, &wallet_address, &sig_str)
                        .await
                    {
                        tracing::warn!(
                            "mark_registered failed after on-chain register for wallet {} (user {}): {}",
                            wallet_address, user_id, e
                        );
                    }

                    let pda = config
                        .registry_program_id
                        .parse::<solana_sdk::pubkey::Pubkey>()
                        .ok()
                        .map(|program_id| {
                            solana_sdk::pubkey::Pubkey::find_program_address(
                                &[b"user", pubkey.as_ref()],
                                &program_id,
                            )
                            .0
                            .to_string()
                        })
                        .unwrap_or_default();
                    if let Err(e) = user_repo
                        .mark_user_onboarded(
                            user_id,
                            user_type_str,
                            latitude,
                            longitude,
                            &pda,
                            &sig_str,
                        )
                        .await
                    {
                        tracing::warn!(
                            "mark_user_onboarded failed after on-chain register for user {}: {}",
                            user_id, e
                        );
                    }

                    info!(
                        "🔗 On-chain registration confirmed for custodial wallet {} (user {})",
                        wallet_address, user_id
                    );
                }
                Ok(_) => {
                    // Submitted but the user PDA never became observable — treat as
                    // unregistered (off-chain wallet still works; retry via
                    // InitializeUserWallet). Do NOT mark registered on an unconfirmed tx.
                    tracing::warn!(
                        "On-chain registration submitted but not confirmed for custodial wallet {} (user {}) — leaving unregistered",
                        wallet_address, user_id
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "On-chain registration failed for custodial wallet {} (user {}): {}",
                        wallet_address, user_id, e
                    );
                }
            }
        });
    }

    /// `&self`-free variant of [`Self::confirm_user_registered`] usable from a
    /// detached task: polls the Registry user PDA for ~15s.
    async fn confirm_registered(
        blockchain_service: &Arc<dyn BlockchainTrait>,
        config: &Config,
        pubkey: &solana_sdk::pubkey::Pubkey,
    ) -> bool {
        let program_id = match config.registry_program_id.parse::<solana_sdk::pubkey::Pubkey>() {
            Ok(p) => p,
            Err(_) => return false,
        };
        let (pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
            &[b"user", pubkey.as_ref()],
            &program_id,
        );
        for _ in 0..20 {
            if blockchain_service.account_exists(pda).await.unwrap_or(false) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
        }
        false
    }

    /// Re-sends the verification email for an unverified account.
    ///
    /// Always reports success for unknown emails to avoid account enumeration.
    pub async fn resend_verification(&self, email: &str) -> Result<ResendVerificationResult> {
        info!("📧 Verification email resend requested");

        let sent = ResendVerificationResult {
            status: "sent".to_string(),
            message: "If that email is registered and unverified, a new verification email has been sent".to_string(),
        };

        let Some(state) = self.user_repo.find_verification_state_by_email(email).await? else {
            return Ok(sent);
        };

        // Already-verified accounts return the SAME generic response as unknown
        // emails — never expose that the address exists+is verified, or this
        // endpoint becomes an account-enumeration oracle. Just don't re-send.
        if state.email_verified {
            return Ok(sent);
        }

        // Cooldown gate: throttle repeat resends per account to stop email-bombing
        // and rate-limit evasion. Still returns the generic `sent` response while
        // throttled so the response is indistinguishable from a real send (no
        // enumeration / timing oracle). Keyed by user ID — see keys.rs.
        let cooldown_key =
            iam_core::domain::identity::keys::cache::resend_verification_cooldown(
                &state.user_id.to_string(),
            );
        if self.cache.exists(&cooldown_key).await.unwrap_or(false) {
            info!("resend within cooldown window — suppressing re-send");
            return Ok(sent);
        }

        let token = match state.verification_token {
            Some(token) => token,
            None => {
                let token = Uuid::new_v4().to_string();
                self.user_repo.set_verification_token(state.user_id, &token).await?;
                token
            }
        };

        if let Err(e) = self.event_bus.publish(&Event::verification_email_requested(
            &state.user_id, &state.username, email, &token,
        )).await {
            tracing::warn!("failed to publish VerificationEmailRequested event: {}", e);
        }

        // Arm the cooldown only after a (best-effort) send. Non-fatal on failure —
        // worst case the next request is allowed through, which is the safe side.
        if let Err(e) = self
            .cache_set(&cooldown_key, &true, Some(RESEND_VERIFICATION_COOLDOWN_SECS))
            .await
        {
            tracing::warn!("failed to set resend cooldown key (non-critical): {}", e);
        }

        Ok(sent)
    }

    pub async fn verify_api_key(&self, key: &str) -> Result<ApiKey> {
        info!("🔑 API Key verification attempt");

        let key_hash = self.api_key_service.hash_key(key)?;

        // Cache TTL bounds the revocation window: a key deactivated in the
        // DB stays accepted until its cached entry expires. There is no
        // app-level revoke hook to invalidate the cache, so keep this short.
        const API_KEY_CACHE_TTL_SECS: u64 = 30;

        // ── Check cache first ────────────────────────────────────────
        let cache_key = iam_core::domain::identity::keys::cache::api_key(&key_hash);
        let cached: Option<ApiKey> = self.cache_get(&cache_key).await
            .unwrap_or_else(|e| {
                tracing::warn!("API key cache GET failed (non-critical): {}", e);
                None
            });

        if let Some(api_key) = cached {
            // Cache hit ⇒ this key was verified within the TTL window. Skip
            // the per-request `update_last_used` DB write and the event
            // publish to avoid write/event amplification on the hot gateway
            // path. `last_used_at` is therefore tracked at ~TTL granularity,
            // refreshed on the cache-miss path below.
            return Ok(api_key);
        }

        // Cache miss — query DB. A missing/inactive row is a genuine auth
        // failure (Unauthorized); infra errors propagate as Database/Internal
        // so callers can distinguish "bad key" from "IAM degraded".
        let api_key = self.api_key_repo.find_by_hash(&key_hash).await?
            .ok_or_else(|| {
                info!("API Key not found or inactive");
                ApiError::Unauthorized("Invalid API Key".to_string())
            })?;

        // Update last_used_at (fire-and-forget; once per TTL window)
        let _ = self.api_key_repo.update_last_used(api_key.id).await;

        // ── Cache the API key ────────────────────────────────────────
        let _ = self.cache_set(&cache_key, &api_key, Some(API_KEY_CACHE_TTL_SECS)).await;

        // ── Publish event ────────────────────────────────────────────
        let event = Event::api_key_verified(
            &api_key.name,
            &api_key.role,
        );
        let _ = self.event_bus.publish(&event).await;

        Ok(api_key)
    }

    /// Retrieves the public profile of a user by their ID.
    pub async fn get_user_profile(&self, user_id: Uuid) -> Result<User> {
        self.user_repo.find_by_id(user_id).await?
            .ok_or_else(|| ApiError::NotFound("User not found".to_string()))
    }

    pub async fn list_wallets(&self, user_id: Uuid) -> Result<Vec<UserWallet>> {
        self.wallet_repo.list_by_user_id(user_id).await
    }

    /// Resolves a user's primary on-chain wallet address. Used by the
    /// Aggregator Bridge (via gRPC `GetUserWallet`) to find the mint
    /// recipient for generation settlement.
    pub async fn get_primary_wallet_address(&self, user_id: Uuid) -> Result<String> {
        self.wallet_repo.find_primary_address(user_id).await?
            .ok_or_else(|| ApiError::NotFound("No primary wallet found".to_string()))
    }

    pub async fn get_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<UserWallet> {
        self.wallet_repo.find_by_id_and_user_id(wallet_id, user_id).await?
            .ok_or_else(|| ApiError::NotFound("Wallet not found".to_string()))
    }

    pub async fn set_primary_wallet(&self, user_id: Uuid, wallet_id: Uuid) -> Result<UserWallet> {
        let wallet = self.wallet_repo.set_primary(user_id, wallet_id).await?
            .ok_or_else(|| ApiError::NotFound("Wallet not found".to_string()))?;
        // Keep users.wallet_address in sync with the primary wallet.
        let _ = self.user_repo.set_wallet_address(user_id, &wallet.wallet_address).await;
        Ok(wallet)
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

    /// Links a new Solana wallet to an existing user account.
    pub async fn link_wallet(
        &self,
        user_id: Uuid,
        wallet_address: String,
        label: Option<String>,
        is_primary: bool,
    ) -> Result<UserWallet> {
        // Reject malformed addresses before any persistence — a wallet must be a valid
        // base58 Solana pubkey. Previously this was only enforced in the auto-on-chain
        // registration branch below, so a non-onboarded user could link an invalid
        // address and get a 200. Mirror that parse up front for every caller.
        gridtokenx_blockchain_core::BlockchainService::parse_pubkey(&wallet_address)
            .map_err(|e| ApiError::BadRequest(format!("invalid wallet address: {e}")))?;

        let has_existing = self.wallet_repo.has_any_wallet(user_id).await?;

        if has_existing && is_primary {
            self.wallet_repo.clear_primary(user_id).await?;
        }

        let mut wallet = self.wallet_repo.insert(user_id, &wallet_address, label.as_deref(), is_primary).await?;

        // First wallet, or one flagged primary, becomes the user's on-chain address.
        if is_primary || !has_existing {
            let _ = self.user_repo.set_wallet_address(user_id, &wallet_address).await;
        }

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

                // Register on-chain. Confirm the PDA landed before marking
                // registered — the signature is returned optimistically.
                if let Ok(sig) = self.blockchain_service.register_user_on_chain(pubkey, blockchain_user_type, lat, long, 0, 0).await {
                    if !self.confirm_user_registered(&pubkey).await {
                        tracing::warn!(
                            "Auto-register on link submitted but not confirmed for wallet {} (user {}) — leaving unregistered",
                            wallet_address, user_id
                        );
                        return Ok(wallet);
                    }
                    let sig_str = sig.to_string();
                    let _ = self.wallet_repo.mark_registered(user_id, &wallet_address, &sig_str).await;
                    
                    // Update the wallet object to return
                    wallet.blockchain_registered = true;
                    wallet.blockchain_tx_signature = Some(sig_str.clone());
                    
                    // Derive PDA for return
                    let program_id: solana_sdk::pubkey::Pubkey = self.config.registry_program_id.parse()
                        .map_err(|e| ApiError::Configuration(format!("Invalid registry program ID '{}': {e}", self.config.registry_program_id)))?;
                    let (pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
                        &[b"user", pubkey.as_ref()],
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

    /// Confirm a user's on-chain registration landed before recording it.
    ///
    /// `register_user_on_chain` returns a signature optimistically — Chain
    /// Bridge does not confirm execution, so a dropped or simulation-rejected tx
    /// can masquerade as success. Derive the user PDA and poll until it is
    /// observable on-chain (absorbing confirmation lag). Returns true only when
    /// the account exists; callers MUST gate `mark_registered` /
    /// `mark_user_onboarded` on this, or a failed tx is recorded as success.
    async fn confirm_user_registered(&self, pubkey: &solana_sdk::pubkey::Pubkey) -> bool {
        // ~15s window: Chain Bridge reads at `confirmed` (~1-2s) but the
        // provider-side retry can land the tx several seconds after the call returns.
        Self::confirm_registered(&self.blockchain_service, &self.config, pubkey).await
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

        // Derive the Registry PDA up front. A bad `registry_program_id` config is a
        // fatal-but-fallible error, not a panic — parse before spending an on-chain
        // transaction so a misconfig fails fast and wastes nothing.
        let program_id: solana_sdk::pubkey::Pubkey = self.config.registry_program_id
            .parse()
            .map_err(|e| ApiError::Internal(format!("Invalid registry program ID: {e}")))?;
        let (pda, _) = solana_sdk::pubkey::Pubkey::find_program_address(
            &[b"user", pubkey.as_ref()],
            &program_id,
        );

        let user_type_str = match user_type_domain {
            UserType::Prosumer => "Prosumer",
            UserType::Consumer => "Consumer",
        };

        // ── Idempotency ─────────────────────────────────────────────
        // `register_user_on_chain` submits without confirming execution, and the
        // provider-side retry can resubmit a tx that already landed — the Registry
        // program then rejects the duplicate with AccountAlreadyInUse. If the user
        // PDA already exists on-chain, registration is done: heal the DB flags and
        // return without spending another transaction.
        if self.blockchain_service.account_exists(pda).await.unwrap_or(false) {
            let _ = self.user_repo.mark_user_onboarded(user_id, user_type_str, lat_e7 as f64, long_e7 as f64, &pda.to_string(), "preexisting-onchain").await;
            let _ = self.wallet_repo.mark_registered(user_id, &wallet_address, "preexisting-onchain").await;
            return Ok(OnChainOnboardingResult {
                success: true,
                message: "User already registered on-chain".to_string(),
                transaction_signature: None,
            });
        }

        match self.blockchain_service.register_user_on_chain(pubkey, blockchain_user_type, lat_e7, long_e7, h3_index, shard_id).await {
            Ok(sig) => {
                let sig_str = sig.to_string();

                // ── Confirm the submit actually landed ───────────────────────
                // The signature is returned optimistically (no execution
                // confirmation), so a dropped or failed tx can masquerade as
                // success. Only mark the user registered once the PDA is
                // observable on-chain. Poll briefly to absorb confirmation lag.
                // Poll up to ~15s: Chain Bridge reads at `confirmed` (~1-2s) but the
                // provider-side retry loop can resubmit before returning, so the
                // landing tx may be several seconds behind the returned signature.
                let mut confirmed = false;
                for _ in 0..20 {
                    if self.blockchain_service.account_exists(pda).await.unwrap_or(false) {
                        confirmed = true;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(750)).await;
                }
                if !confirmed {
                    tracing::error!(
                        user_id = %user_id, signature = %sig_str, pda = %pda,
                        "On-chain onboarding submitted but user PDA not observable — treating as unconfirmed, NOT marking registered"
                    );
                    return Ok(OnChainOnboardingResult {
                        success: false,
                        message: "On-chain registration submitted but not confirmed on-chain; please retry".to_string(),
                        transaction_signature: Some(sig_str),
                    });
                }

                // The chain write is the source of truth and cannot be rolled back.
                // If a DB mark fails we are in a drift state (on-chain registered, DB
                // shows not) which makes a retry re-submit a tx the Registry program
                // will reject. Do NOT swallow these errors: log loud with the
                // signature + PDA for reconciliation, and flag it on the result.
                let mut db_persisted = true;

                if let Err(e) = self.user_repo.mark_user_onboarded(
                    user_id,
                    user_type_str,
                    lat_e7 as f64,
                    long_e7 as f64,
                    &pda.to_string(),
                    &sig_str,
                ).await {
                    db_persisted = false;
                    tracing::error!(
                        user_id = %user_id, signature = %sig_str, pda = %pda,
                        "DRIFT: on-chain onboarding succeeded but mark_user_onboarded failed: {e}. Manual reconciliation required."
                    );
                }

                if let Err(e) = self.wallet_repo.mark_registered(user_id, &wallet_address, &sig_str).await {
                    db_persisted = false;
                    tracing::error!(
                        user_id = %user_id, signature = %sig_str, wallet = %wallet_address,
                        "DRIFT: on-chain onboarding succeeded but mark_registered failed: {e}. Manual reconciliation required."
                    );
                }

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
                    message: if db_persisted {
                        "User registered on-chain".to_string()
                    } else {
                        "User registered on-chain; DB persistence lagged — reconciliation pending".to_string()
                    },
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

    /// Initiates the password reset process by sending an email with a reset token.
    pub async fn forgot_password(&self, email: &str) -> Result<()> {
        // Always return Ok to avoid email enumeration. Single DB lookup — reuse
        // the row for both the existence check and the event payload.
        let Some(user_with_hash) = self.user_repo.find_by_username_or_email(email).await? else {
            return Ok(());
        };

        let token = Uuid::new_v4().to_string();
        let ttl_secs = 900u64; // 15 minutes
        let key = iam_core::domain::identity::keys::cache::password_reset_token(&token);
        self.cache_set(&key, &email.to_lowercase(), Some(ttl_secs)).await?;

        // ── Publish Event ──────────────────────────────────────────
        let reset_url = format!("{}/reset-password?token={}", self.config.app_base_url, token);
        let event = Event::password_reset_requested(
            &user_with_hash.user.id,
            email,
            &reset_url,
        );
        let _ = self.event_bus.publish(&event).await;

        Ok(())
    }

    /// Resets a user's password using a valid reset token.
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

    /// Initializes a user's wallet on-chain and funds it with an initial balance.
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

        // Signature is optimistic — confirm the PDA landed before recording it.
        if !self.confirm_user_registered(&pubkey).await {
            return Err(ApiError::Internal(
                "On-chain registration submitted but not confirmed on-chain; please retry".to_string(),
            ));
        }

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
