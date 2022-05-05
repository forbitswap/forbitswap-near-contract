// View functions for the contract

use near_sdk::json_types::U128;
use near_sdk::{
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId,
};

use crate::pool::Pool;
use crate::utils::SwapVolume;
use crate::errors::*;
use crate::*;

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Deserialize, Debug))]
pub struct ContractMetadata {
    pub version: String,
    pub owner: AccountId,
    pub pool_count: u64,
    pub exchange_fee: u32,
    pub referral_fee: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct PoolInfo {
    /// Pool kind.
    pub pool_kind: String,
    /// List of tokens in the pool.
    pub token_account_ids: Vec<AccountId>,
    /// How much NEAR this contract has.
    pub amounts: Vec<U128>,
    /// Fee charged for swap.
    pub total_fee: u32,
    /// Total number of shares.
    pub shares_total_supply: U128,
    pub amp: u64,
}

impl From<Pool> for PoolInfo {
    fn from(pool: Pool) -> Self {
        let pool_kind = pool.kind();
        match pool {
            Pool::SimplePool(pool) => Self {
                pool_kind,
                amp: 0,
                token_account_ids: pool.token_account_ids,
                amounts: pool.amounts.into_iter().map(|a| U128(a)).collect(),
                total_fee: pool.total_fee,
                shares_total_supply: U128(pool.shares_total_supply),
            },
        }
    }
}

#[near_bindgen]
impl Contract {
    /// Return contract basic info
    pub fn metadata(&self) -> ContractMetadata {
        ContractMetadata {
            version: env!("CARGO_PKG_VERSION").to_string(),
            owner: self.owner_id.clone(),
            pool_count: self.pools.len(),
            exchange_fee: self.exchange_fee,
            referral_fee: self.referral_fee,
        }
    }

    /// Returns version of this contract.
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Returns number of pools
    pub fn get_number_of_pools(&self) -> u64 {
        self.pools.len()
    }

    /// Get pools from `from` index and `limit` specifies how many pools to get.
    /// `limit` will be limited to last pool index if the given `limit` is out of pool length
    pub fn get_pools(&self, from_index: u64, limit: u64) -> Vec<PoolInfo> {
        (from_index..std::cmp::min(limit, self.pools.len()))
            .map(|index| self.get_pool(index))
            .collect()
    }

    /// Given specific pool, returns amount of token_out recevied swapping amount_in of token_in.
    pub fn get_return(
        &self,
        pool_id: u64,
        token_in: ValidAccountId,
        amount_in: U128,
        token_out: ValidAccountId,
    ) -> U128 {
        let pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        pool.get_return(token_in.as_ref(), amount_in.into(), token_out.as_ref())
            .into()
    }

    /// Given specific pool, returns amount of token_out recevied swapping amount_in of token_in.
    pub fn get_income(
        &self,
        pool_id: u64,
        token_in: ValidAccountId,
        amount_in: U128,
        token_out: ValidAccountId,
    ) -> U128 {
        let pool = self.pools.get(pool_id).expect(ERR85_NO_POOL);
        pool.get_income(token_in.as_ref(), token_out.as_ref(), amount_in.into())
            .into()
    }

    /// Get a single pool by given `id`
    pub fn get_pool(&self, pool_id: u64) -> PoolInfo {
        self.pools.get(pool_id).expect("ERR_POOL_NOT_FOUND").into()
    }

    pub fn get_whitelisted_tokens(&self) -> Vec<AccountId> {
        self.whitelisted_tokens.to_vec()
    }

    /// Return total fee of the given pool.
    pub fn get_pool_fee(&self, pool_id: u64) -> u32 {
        self.pools.get(pool_id).expect("ERR_NO_POOL").get_fee()
    }

    /// Return volumes of the given pool.
    pub fn get_pool_volumes(&self, pool_id: u64) -> Vec<SwapVolume> {
        self.pools.get(pool_id).expect("ERR_NO_POOL").get_volumes()
    }

    pub fn get_pool_share_price(&self, pool_id: u64) -> U128 {
        self.pools
            .get(pool_id)
            .expect("ERR_NO_POOL")
            .get_share_price()
            .into()
    }

    /// Return number of shares given account has in given pool
    pub fn get_account_shares_in_pool(&self, pool_id: u64, account_id: ValidAccountId) -> U128 {
        self.pools
            .get(pool_id)
            .expect("ERR_NO_POOL")
            .share_balances(account_id.as_ref())
            .into()
    }

    /// Returns total number of shares in the given pool.
    pub fn get_pool_total_shares(&self, pool_id: u64) -> U128 {
        self.pools
            .get(pool_id)
            .expect("ERR_NO_POOL")
            .share_total_balance()
            .into()
    }

    pub fn get_deposited_tokens(&self, account_id: &AccountId) -> HashMap<AccountId, U128> {
        let tokens: HashMap<AccountId, U128> = self
            .internal_unwrap_account(account_id)
            .tokens
            .iter()
            .map(|(token, amount)| (token.clone(), U128(amount)))
            .collect();
        tokens
    }

    /// Returns balance of the deposit for given user in the exchange.
    pub fn get_deposited_token(
        &self,
        account_id: ValidAccountId,
        token_id: ValidAccountId,
    ) -> U128 {
        unimplemented!()
    }

    pub fn predict_remove_liquidity(&self, pool_id: u64, shares: U128) -> Vec<U128> {
        let pool = self.pools.get(pool_id).expect("ERR_NO_POOL");
        pool.predict_remove_liquidity(shares.into())
            .into_iter()
            .map(|x| U128(x))
            .collect()
    }

    pub fn get_account_balance(&self, account_id: AccountId, token_id: AccountId) -> U128 {
        let account = self.internal_get_account(&account_id).unwrap();
        U128::from(account.get_balance(&token_id).unwrap())
    }
}
