use near_sdk::{ext_contract};

#[ext_contract(ext_self)]
pub trait Contract {
    fn deposit_all(&self, &user_id: AccountId, &contract_id: AccountId, token_ids: Vec<AccountId>) -> String;
    pub fn get_deposited_tokens(&self, account_id: &AccountId) -> HashMap<AccountId, U128>;
    fn my_callback(&self) -> String;
}

#[ext_contract(ext_ft)]
pub trait FungibleToken {
    fn ft_balance_of(&mut self, account_id: AccountId) -> U128;

}

impl Contract {
    pub fn my_first_contract_call(&self, account_id: AccountId) -> Promise {


        ext_ft::ft_balance_of(
            account_id.into(),

        )
    }
}