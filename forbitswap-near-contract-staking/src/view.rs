use near_sdk::{
    serde::{Deserialize, Serialize},
};
use near_sdk::{AccountId, Balance};

pub type TokenId = AccountId  ;
pub type Time = u128 ;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct FarmData {
    pub farm_id : u128 ,
    pub account: AccountId,
    pub time_register: Time,
    pub time_start: Time,
    pub time_end: Time,
    pub token_id: TokenId ,
    pub old_balance: Balance,
    pub new_balance: Balance ,
}

impl FarmData {
    pub fn new(farm_id : u128 ,account: AccountId,time_register: Time,time_start: Time,time_end: Time,token_id: TokenId ,old_balance: Balance, new_balance: Balance) -> Self {
        FarmData { 
            farm_id: farm_id.clone(), 
            account: account.clone(), 
            time_register: time_register.clone(),
            time_start: time_start.clone(), 
            time_end: time_end.clone(), 
            token_id: token_id.clone(), 
            old_balance: old_balance.clone(),
            new_balance: new_balance.clone() 
        }
    }
}