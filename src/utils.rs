#[allow(dead_code, unused_imports)]
use std::collections::HashSet;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{ext_contract, AccountId, Balance, Gas};
use uint::construct_uint;

use crate::errors::*;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

// 10^24 yocto near
// pub const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;

pub const NUM_TOKENS: usize = 2;

/// Attach no deposit.
// pub const NO_DEPOSIT: u128 = 0;

/// 10T gas for basic operation
pub const GAS_FOR_BASIC_OP: Gas = 10_000_000_000_000;

/// hotfix_insuffient_gas_for_mft_resolve_transfer.
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = 20_000_000_000_000;

// pub const GAS_FOR_FT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;

/// Amount of gas for fungible token transfers, increased to 20T to support AS token contracts.
pub const GAS_FOR_FT_TRANSFER: Gas = 20_000_000_000_000;

/// Fee divisor, allowing to provide fee in bps.
pub const FEE_DIVISOR: u32 = 10_000;

/// Initial shares supply on deposit of liquidity.
pub const INIT_SHARES_SUPPLY: u128 = 1_000_000_000_000_000_000_000_000;

// Square root of an unsigned integer
pub fn uint_sqrt(value: U256) -> U256 {
    let mut guess: U256 = (value + U256::one()) >> 1;
    let mut res = value;
    while guess < res {
        res = guess;
        guess = (value / guess + guess) >> 1;
    }
    res
}

pub fn check_duplicate_tokens(tokens: &Vec<ValidAccountId>) {
    let token_set: HashSet<_> = tokens.iter().map(|token| token.as_ref()).collect();
    assert_eq!(tokens.len(), token_set.len(), "{}", ERR_DUPLICATE_TOKENS);
}

pub fn add_to_collection(c: &mut LookupMap<AccountId, Balance>, key: &AccountId, value: Balance) {
    let prev_value = c.get(&key).unwrap_or(0);
    c.insert(key, &(value + prev_value));
}

/// Volume of swap on the given token.
#[derive(Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SwapVolume {
    pub input: U128,
    pub output: U128,
}

impl Default for SwapVolume {
    fn default() -> Self {
        Self {
            input: U128(0),
            output: U128(0),
        }
    }
}

#[ext_contract(ext_self)]
pub trait Exchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
    );
}
