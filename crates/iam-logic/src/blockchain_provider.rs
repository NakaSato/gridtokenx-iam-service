use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use gridtokenx_blockchain_core::rpc::instructions::UserType;
use gridtokenx_blockchain_core::BlockchainService;
use iam_core::traits::BlockchainTrait;
use iam_core::error::Result;
use futures::future::{BoxFuture, FutureExt};

pub struct BlockchainProvider {
    service: Arc<BlockchainService>,
}

impl BlockchainProvider {
    pub fn new(service: Arc<BlockchainService>) -> Self {
        Self { service }
    }
}

impl BlockchainTrait for BlockchainProvider {
    fn register_user_on_chain(
        &self,
        authority: Pubkey,
        user_type: UserType,
        lat_e7: i32,
        long_e7: i32,
        h3_index: u64,
        shard_id: u8,
    ) -> BoxFuture<'static, Result<Signature>> {
        let service = self.service.clone();
        async move {
            let mut attempts = 0;
            // One retry only. Each attempt is capped at 15s (legit on-chain reg
            // measures ~14s under load), so worst-case wall time is 15 + 2s backoff
            // + 15 = 32s. This MUST stay under the HTTP TimeoutLayer budget
            // (REQUEST_TIMEOUT_SECS, default 40s) or verify returns 408 mid-retry.
            let max_attempts = 1;

            loop {
                // ── Timeout & Retry ──────────────────────────────────────────
                let rpc_call = service.register_user_on_chain(
                    authority,
                    user_type,
                    lat_e7,
                    long_e7,
                    h3_index,
                    shard_id,
                );

                match tokio::time::timeout(std::time::Duration::from_secs(15), rpc_call).await {
                    Ok(Ok(sig)) => return Ok(sig),
                    Ok(Err(e)) if attempts < max_attempts => {
                        attempts += 1;
                        let delay = 2u64.pow(attempts);
                        tracing::warn!(
                            "🔗 Blockchain RPC error (attempt {}/{}): {}. Retrying in {}s...",
                            attempts, max_attempts, e, delay
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }
                    Err(_) if attempts < max_attempts => {
                        attempts += 1;
                        let delay = 2u64.pow(attempts);
                        tracing::warn!(
                            "⏱️ Blockchain RPC timed out (attempt {}/{}). Retrying in {}s...",
                            attempts, max_attempts, delay
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }
                    Ok(Err(e)) => {
                        tracing::error!("❌ Blockchain registration failed after {} attempts: {}", max_attempts, e);
                        return Err(iam_core::error::ApiError::Internal(format!("On-chain registration failed: {}", e)));
                    }
                    Err(_) => {
                        tracing::error!("❌ Blockchain registration timed out after {} attempts", max_attempts);
                        return Err(iam_core::error::ApiError::Internal("On-chain registration timed out".to_string()));
                    }
                }
            }
        }.boxed()
    }

    fn account_exists(&self, pubkey: Pubkey) -> BoxFuture<'static, Result<bool>> {
        let service = self.service.clone();
        async move {
            service.account_manager.account_exists(&pubkey).await
                .map_err(|e| iam_core::error::ApiError::Internal(format!("Account existence check failed: {e}")))
        }.boxed()
    }
}
