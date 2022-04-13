use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::PromiseOrValue;

use crate::*;

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Callback on receiving tokens by this contract.
    /// `msg` format is either "" for deposit or `TokenReceiverMessage`.
    #[allow(unreachable_code)]
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let token_in = env::predecessor_account_id();
        assert!(msg.is_empty(), "msg must be empty on deposit action!");
        // Simple deposit.
        self.internal_transfer_from_user(&sender_id.to_string(), &token_in, amount.into());
        PromiseOrValue::Value(U128(0))
    }
}
