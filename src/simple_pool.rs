use std::cmp::min;

use crate::admin_fee::AdminFees;
use crate::StorageKey;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::ValidAccountId;
use near_sdk::serde::Serialize;
use near_sdk::{env, AccountId, Balance};
use crate::*;
use crate::errors::{ERR14_LP_ALREADY_REGISTERED, ERR31_ZERO_AMOUNT, ERR32_ZERO_SHARES};

use crate::utils::{
    add_to_collection, uint_sqrt, SwapVolume, FEE_DIVISOR, INIT_SHARES_SUPPLY, NUM_TOKENS, U256,
};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SimplePool {
    /// List of tokens in the pool.
    pub token_account_ids: Vec<AccountId>,
    /// How much NEAR this contract has.
    pub amounts: Vec<Balance>,
    /// Volumes accumulated by this pool.
    pub volumes: Vec<SwapVolume>,
    /// Fee charged for swap (gets divided by FEE_DIVISOR).
    pub total_fee: u32,
    /// Obsolete, reserve to simplify upgrade.
    pub exchange_fee: u32,
    /// Obsolete, reserve to simplify upgrade.
    pub referral_fee: u32,
    /// Shares of the pool by liquidity providers.
    pub shares: UnorderedMap<AccountId, Balance>,
    /// Total number of shares.
    pub shares_total_supply: Balance,

    first_provider: Option<AccountId>
}

impl SimplePool {
    pub fn new(
        id: u32,
        token_account_ids: Vec<ValidAccountId>,
        total_fee: u32,
        exchange_fee: u32,
        referral_fee: u32,
    ) -> Self {
        assert!(total_fee < FEE_DIVISOR, "ERR_FEE_TOO_LARGE");

        // [AUDIT_10]
        assert_eq!(
            token_account_ids.len(),
            NUM_TOKENS,
            "ERR_SHOULD_HAVE_2_TOKENS"
        );
        Self {
            token_account_ids: token_account_ids.iter().map(|a| a.clone().into()).collect(),
            amounts: vec![0u128; token_account_ids.len()],
            volumes: vec![SwapVolume::default(); token_account_ids.len()],
            total_fee,
            exchange_fee,
            referral_fee,
            // [AUDIT 11]
            shares: UnorderedMap::new(StorageKey::Shares { pool_id: id }),
            shares_total_supply: 0,
            first_provider: None,
        }
    }

    /// Register given account with 0 balance in shares.
    /// Storage payment should be checked by caller.
    pub fn share_register(&mut self, account_id: &AccountId) {
        if self.shares.get(account_id).is_some() {
            env::panic(ERR14_LP_ALREADY_REGISTERED.as_bytes());
        }

        self.shares.insert(account_id, &0);
    }

    /// Returns balance of shares for given user.
    pub fn share_balance_of(&self, account_id: &AccountId) -> Balance {
        self.shares.get(account_id).unwrap_or_default()
    }

    /// Returns total number of shares in this pool.
    pub fn share_total_balance(&self) -> Balance {
        self.shares_total_supply
    }

    /// Returns list of tokens in this pool.
    pub fn tokens(&self) -> &[AccountId] {
        &self.token_account_ids
    }

    pub fn is_lp(&self, account_id: &AccountId) -> bool {
        if self.shares.get(account_id).is_some() {
            return true;
        } else {
            return false;
        }
    }

    /// adds the amounts of tokens to liquidity pool and returns number of shares that this user receives.
    /// Updates amount to amount kept in the pool.
    pub fn add_liquidity(&mut self, sender_id: &AccountId, amounts: &mut Vec<Balance>) -> Balance {
        assert_eq!(
            amounts.len(),
            self.token_account_ids.len(),
            "ERR_WRONG_TOKEN_COUNT"
        );
        let shares = if self.shares_total_supply > 0 {
            let mut fair_supply = U256::max_value();
            for i in 0..self.token_account_ids.len() {
                assert!(amounts[i] > 0, "{}", ERR31_ZERO_AMOUNT);
                fair_supply = min(
                    fair_supply,
                    U256::from(amounts[i]) * U256::from(self.shares_total_supply) / self.amounts[i],
                );
            }
            for i in 0..self.token_account_ids.len() {
                let amount = (U256::from(self.amounts[i]) * fair_supply
                    / U256::from(self.shares_total_supply))
                .as_u128();
                assert!(amount > 0, "{}", ERR31_ZERO_AMOUNT);
                self.amounts[i] += amount;
                amounts[i] = amount;
                println!(
                    "Amount[{}]: {}, Self amount[{}]: {}",
                    i,
                    amount.clone(),
                    i,
                    self.amounts[i].clone()
                );
            }
            fair_supply.as_u128()
        } else {
            for i in 0..self.token_account_ids.len() {
                self.amounts[i] += amounts[i];
            }
            INIT_SHARES_SUPPLY
        };
        self.mint_shares(&sender_id, shares);
        assert!(shares > 0, "{}", ERR32_ZERO_SHARES);

        env::log(
            format!(
                "Liquidity added {:?}, minted {} shares",
                amounts
                    .iter()
                    .zip(self.token_account_ids.iter())
                    .map(|(amount, token_id)| format!("{} {}", amount, token_id))
                    .collect::<Vec<String>>(),
                shares
            )
            .as_bytes(),
        );

        // let shares = if self.shares_total_supply > 0 {
        //     let mut fair_supply = U256::max_value();
        //     for i in 0..self.token_account_ids.len() {
        //         assert!(amounts[i] > 0, "{}", ERR31_ZERO_AMOUNT);
        //         self.amounts[i] += amounts[i];
        //         fair_supply = min(
        //             fair_supply,
        //             U256::from(amounts[i]) * U256::from(self.shares_total_supply) / self.amounts[i],
        //         );
        //     }
        //     fair_supply.as_u128()
        // } else {
        //     for i in 0..self.token_account_ids.len() {
        //         self.amounts[i] += amounts[i];
        //     }
        //     INIT_SHARES_SUPPLY
        // };
        shares
    }

    pub fn mint_shares(&mut self, account_id: &AccountId, shares: Balance) {
        if shares == 0 {
            return;
        }
        self.shares_total_supply += shares;
        add_to_collection(&mut self.shares, &account_id, shares);
    }

    pub fn remove_liquidity(
        &mut self,
        sender_id: &AccountId,
        shares: Balance,
        min_amounts: Vec<Balance>,
    ) -> Vec<Balance> {
        assert_eq!(
            min_amounts.len(),
            self.token_account_ids.len(),
            "ERR_WRONG_TOKEN_COUNT"
        );
        // check current shares in pool, must be greater than input "shares"
        let prev_shares_amount = self.shares.get(&sender_id).expect("ERR_NO_SHARES");
        assert!(prev_shares_amount >= shares, "ERR_NOT_ENOUGH_SHARES");
        let mut amounts = vec![];
        for i in 0..self.token_account_ids.len() {
            let amount = (U256::from(self.amounts[i]) * U256::from(shares)
                / U256::from(self.shares_total_supply))
            .as_u128();
            assert!(amount >= min_amounts[i], "ERR_MIN_AMOUNT");
            self.amounts[i] -= amount;
            amounts.push(amount);
        }
        if prev_shares_amount == shares {
            // [AUDIT_13] Never unregister a LP when he removed all his liquidity.
            self.shares.insert(&sender_id, &0);
        } else {
            self.shares
                .insert(&sender_id, &(prev_shares_amount - shares));
        }
        env::log(
            format!(
                "{} shares of liquidity removed: receive back {:?}",
                shares,
                amounts
                    .iter()
                    .zip(self.token_account_ids.iter())
                    .map(|(amount, token_id)| format!("{} {}", amount, token_id))
                    .collect::<Vec<String>>(),
            )
            .as_bytes(),
        );
        self.shares_total_supply -= shares;
        amounts
    }

    // Returns index of token in given pool
    fn token_index(&self, token_id: &AccountId) -> usize {
        self.token_account_ids
            .iter()
            .position(|id| id == token_id)
            .expect("ERR_MISSING_TOKEN")
    }

    /// Returns number of tokens in outcome, given amount.
    /// Tokens are provided as indexes into token list for given pool.
    fn internal_get_return(
        &self,
        token_in: usize, // index of token_in in pool
        amount_in: Balance,
        token_out: usize, // index of token_out in pool
    ) -> Balance {
        let in_balance = U256::from(self.amounts[token_in]);
        let out_balance = U256::from(self.amounts[token_out]);
        assert!(
            in_balance > U256::zero()
                && out_balance > U256::zero()
                && token_in != token_out
                && amount_in > 0,
            "ERR_INVALID"
        );

        let amount_with_fee = U256::from(amount_in) * U256::from(FEE_DIVISOR - self.total_fee);
        (amount_with_fee * out_balance / (U256::from(FEE_DIVISOR) * in_balance + amount_with_fee))
            .as_u128()
    }

    pub fn get_return(
        &self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
    ) -> Balance {
        self.internal_get_return(
            self.token_index(token_in),
            amount_in,
            self.token_index(token_out),
        )
    }

    pub fn get_fee(&self) -> u32 {
        self.total_fee
    }

    /// Returns volumes of the given pool.
    pub fn get_volumes(&self) -> Vec<SwapVolume> {
        self.volumes.clone()
    }

    /// Swap `token_amount_in` of `token_in` token into `token_out` and return how much was received.
    /// Assuming that `token_amount_in` was already received from `sender_id`.

    pub fn swap(
        &mut self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
        min_amount_out: Balance,
        admin_fee: &AdminFees,
    ) -> Balance {
        assert_ne!(token_in, token_out, "ERR_SAME_TOKEN_SWAP");
        let in_idx = self.token_index(token_in);
        let out_idx = self.token_index(token_out);
        let amount_out = self.internal_get_return(in_idx, amount_in, out_idx);
        assert!(amount_out >= min_amount_out, "ERR_MIN_AMOUNT");
        env::log(
            format!(
                "Swapped {} {} for {} {}",
                amount_in, token_in, amount_out, token_out
            )
            .as_bytes(),
        );
        
        let fee_charge = amount_in * self.total_fee as u128 / FEE_DIVISOR as u128;
        let fee_as_share = (U256::from(fee_charge * self.shares_total_supply) / U256::from(self.amounts[in_idx])).as_u128();
        let first_provider_earn = 80 * fee_as_share / 100;
        let other_provider_earn = 20 * fee_as_share / 100 / (self.shares.len() - 1) as u128;

        let prev_invariant =
            uint_sqrt(U256::from(self.amounts[in_idx]) * U256::from(self.amounts[out_idx]));

        self.amounts[in_idx] += amount_in;
        self.amounts[out_idx] -= amount_out;

        // "Invariant" is by how much the dot product of amounts increased due to fees.
        let new_invariant =
            uint_sqrt(U256::from(self.amounts[in_idx]) * U256::from(self.amounts[out_idx]));

        // Invcariant can not reduce (otherwise losing balance of the pool and something it broken).
        assert!(new_invariant >= prev_invariant, "ERR_INVARIANT");
        let numerator = (new_invariant - prev_invariant) * U256::from(self.shares_total_supply);

        // Allocates exchange fee as fraction of total fee by issuing LP shares proportionally
        if admin_fee.exchange_fee > 0 && numerator > U256::zero() {
            let denominator = new_invariant * FEE_DIVISOR / admin_fee.exchange_fee;
            self.mint_shares(&admin_fee.exchange_id, (numerator / denominator).as_u128());
        }

        // If there is referral provided and the account already registered LP, allocate it * of LP rewards.
        if let Some(referral_id) = &admin_fee.referral_id {
            if admin_fee.referral_fee > 0
                && numerator > U256::zero()
                && self.shares.get(referral_id).is_some()
            {
                let denominator = new_invariant * FEE_DIVISOR / admin_fee.referral_fee;
                self.mint_shares(referral_id, (numerator / denominator).as_u128());
            }
        }
        // if self.total_fee > 0 {
        //     let shares = self.shares.to_vec();
        //     let first_provider = self.first_provider.as_ref().unwrap();
        //     let first_provider_share = self.shares.get(&first_provider).unwrap();
        //     self.shares.insert(&first_provider, &(first_provider_share + first_provider_earn));
        //     for i in 0..shares.len() {
        //         add_to_collection(&mut self.shares, &shares[i].0, shares[i].1 + other_provider_earn);
        //     }
        // }

        // Keeping track of volume per each input traded separately.
        // Reported volume with fees will be sum of `input`, without fees will be sum of `output`.
        self.volumes[in_idx].input.0 += amount_in;
        self.volumes[in_idx].output.0 += amount_out;

        amount_out
    }

    pub fn predict_remove_liquidity(&self, shares: Balance) -> Vec<u128> {
        let num_tokens = self.token_account_ids.len();
        let mut result = vec![0u128; num_tokens];
        for i in 0..num_tokens {
            result[i] = U256::from(self.amounts[i])
                .checked_mul(shares.into())
                .unwrap()
                .checked_div(self.shares_total_supply.into())
                .unwrap_or_default()
                .as_u128();
        }
        result
    }
}
