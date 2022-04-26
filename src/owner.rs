//! implement all relevant logic for owner of this contract.

use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_sdk::env::predecessor_account_id;
use near_sdk::json_types::WrappedTimestamp;

use crate::utils::{FEE_DIVISOR, GAS_FOR_BASIC_OP};
use crate::*;

#[near_bindgen]
impl Contract {
    /// Change owner. Only can be called by owner.
    pub fn set_owner(&mut self, owner_id: ValidAccountId) {
        self.assert_owner();
        self.owner_id = owner_id.as_ref().clone();
    }

    /// Get owner of this account.
    pub fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    /// Retrieve NEP-141 tokens that not managed by contract to owner,
    /// Caution: Must check that `amount <= total_amount_in_account - amount_managed_by_contract` before calling !!!
    /// Returns promise of ft_transfer action.
    #[payable]
    pub fn retrieve_unmanaged_token(&mut self, token_id: ValidAccountId, amount: U128) -> Promise {
        self.assert_owner();
        assert_one_yocto();
        let token_id: AccountId = token_id.into();
        let amount: u128 = amount.into();
        assert!(amount > 0, "{}", ERR29_ILLEGAL_WITHDRAW_AMOUNT);
        env::log(
            format!(
                "Going to retrieve token {} to owner, amount: {}",
                &token_id, amount
            )
            .as_bytes(),
        );

        ext_fungible_token::ft_transfer(
            self.owner_id.clone(),
            U128(amount),
            None,
            &token_id,
            1,
            env::prepaid_gas() - GAS_FOR_BASIC_OP,
        )
    }

    /// Extend whitelisted tokens with new tokens. Only can be called by owner.
    #[payable]
    pub fn extend_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        assert!(self.is_owner(), "ERR_NOT_ALLOWED");
        for token in tokens {
            self.whitelisted_tokens.insert(token.as_ref());
        }
    }
    /// Remove whitelisted token. Only can be called by owner.
    pub fn remove_whitelisted_tokens(&mut self, tokens: Vec<ValidAccountId>) {
        assert!(self.is_owner(), "ERR_NOT_ALLOWED");
        for token in tokens {
            self.whitelisted_tokens.remove(token.as_ref());
        }
    }

    pub fn modify_admin_fee(&mut self, exchange_fee: u32, referral_fee: u32) {
        self.assert_owner();
        assert!(exchange_fee + referral_fee <= FEE_DIVISOR, "ERR_ILLEGAL_FEE");
        self.exchange_fee = exchange_fee;
        self.referral_fee = referral_fee;
    }

    pub(crate) fn is_owner(&self) -> bool {
        env::predecessor_account_id() == self.owner_id
    }

    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NOT_ALLOWED"
        );
    }
}
