use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::ValidAccountId;
use near_sdk::{AccountId, Balance};

use crate::admin_fee::AdminFees;
use crate::simple_pool::SimplePool;
use crate::utils::SwapVolume;

#[derive(BorshDeserialize, BorshSerialize)]
pub enum Pool {
    SimplePool(SimplePool),
}

impl Pool {
    // Returns pool kind.
    pub fn kind(&self) -> String {
        match self {
            Pool::SimplePool(_) => "SIMPLE_POOL".to_string(),
        }
    }

    // Returns which tokens are in the underlying pool.
    pub fn tokens(&self) -> &[AccountId] {
        match self {
            Pool::SimplePool(pool) => pool.tokens(),
        }
    }

    /// Adds liquidity into underlying pool
    /// Updates amounts to amount kept in the pool
    pub fn add_liquidity(&mut self, sender_id: &AccountId, amounts: &mut Vec<Balance>) -> Balance {
        match self {
            Pool::SimplePool(pool) => pool.add_liquidity(sender_id, amounts),
        }
    }

    pub fn remove_liquidity(
        &mut self,
        sender_id: &AccountId,
        shares: Balance,
        min_amounts: Vec<Balance>,
    ) -> Vec<Balance> {
        match self {
            Pool::SimplePool(pool) => pool.remove_liquidity(sender_id, shares, min_amounts),
        }
    }

    /// Returns how many tokens will one receive swapping given amount of token_in for token_out.
    pub fn get_return(
        &self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
        // fees: &AdminFees,
    ) -> Balance {
        match self {
            Pool::SimplePool(pool) => pool.get_return(token_in, amount_in, token_out),
        }
    }

    /// Return share decimal.
    pub fn get_share_decimal(&self) -> u8 {
        match self {
            Pool::SimplePool(_) => 24,
        }
    }

    /// Returns given pool's total fee
    pub fn get_fee(&self) -> u32 {
        match self {
            Pool::SimplePool(pool) => pool.get_fee(),
        }
    }

    /// returns volumes of the given pool.
    pub fn get_volumes(&self) -> Vec<SwapVolume> {
        match self {
            Pool::SimplePool(pool) => pool.get_volumes(),
        }
    }

    pub fn is_lp(&self, account_id: &AccountId) -> bool {
        match self {
            Pool::SimplePool(pool) => pool.is_lp(&account_id),
        }
    }

    /// Returns given pool's share price in precision 1e8
    pub fn get_share_price(&self) -> u128 {
        unimplemented!()
    }

    /// Swaps given number of token_in for token_out and returns received amount.
    pub fn swap(
        &mut self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
        min_amount_out: Balance,
        admin_fee: AdminFees,
    ) -> Balance {
        match self {
            Pool::SimplePool(pool) => {
                pool.swap(token_in, amount_in, token_out, min_amount_out, &admin_fee)
            }
        }
    }

    pub fn share_total_balance(&self) -> Balance {
        match self {
            Pool::SimplePool(pool) => pool.share_total_balance(),
        }
    }

    pub fn share_balances(&self, account_id: &AccountId) -> Balance {
        match self {
            Pool::SimplePool(pool) => pool.share_balance_of(account_id),
        }
    }

    pub fn share_register(&mut self, account_id: &AccountId) {
        match self {
            Pool::SimplePool(pool) => pool.share_register(account_id),
        }
    }

    pub fn predict_remove_liquidity(&self, shares: Balance) -> Vec<Balance> {
        match &self {
            &Pool::SimplePool(pool) => pool.predict_remove_liquidity(shares),
        }
    }

    pub fn check_existed_pool(&self, tokens: &Vec<ValidAccountId>) -> bool {
        let pool_tokens = self.tokens();

        if pool_tokens[0] == tokens[0].to_string() && pool_tokens[1] == tokens[1].to_string() {
            return true;
        } else {
            return false;
        }
    }
}
