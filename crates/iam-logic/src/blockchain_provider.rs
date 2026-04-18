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
            service.register_user_on_chain(
                authority,
                user_type,
                lat_e7,
                long_e7,
                h3_index,
                shard_id,
            ).await.map_err(|e| iam_core::error::ApiError::Internal(e.to_string()))
        }.boxed()
    }
}
