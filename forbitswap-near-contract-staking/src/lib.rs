use near_sdk::collections::UnorderedMap;
use crate::utils::{ext_self, GAS_FOR_FT_TRANSFER, GAS_FOR_RESOLVE_TRANSFER};
use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{env, AccountId, Balance};
use near_sdk::{
    near_bindgen, Promise, PromiseResult,
};

use near_sdk::{
    serde::{Deserialize, Serialize},
};
use crate::account::Tokens;
use crate::view::FarmData;
use crate::errors::*;
mod errors;
mod account ;
mod token_receiver;
mod view ;
mod utils;

pub type Time = u128;
pub type TokenId = String ;
pub const TIME_DAY:u128 = 86400 ;
pub const MIN_TIME: u128 = 2592000 ;

impl Default for Contract {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            farm: UnorderedMap::new(b"map-uid-1".to_vec()),
        }
    }
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Contract {
    owner_id: AccountId,
    farm: UnorderedMap<u128, Tokens>,
}

#[near_bindgen]
impl Contract {

    #[init]
    pub fn new() -> Self {
         Self {
            owner_id: env::predecessor_account_id(),
            farm: UnorderedMap::new(b"map-uid-1".to_vec()),
        }
    }
   
    pub fn get_len(&self) -> u128 {
        self.farm.len() as u128
    }

    #[payable]
    pub fn withdraw(&mut self,farm_id: &u128 , amount: Balance, time_secons: &Time) ->Balance {
        let account_owner = env::predecessor_account_id();  
        let mut act = self.internal_unwrap_account(&farm_id);
        if account_owner == act.account {
            let new_balance = act.withdraw(amount, time_secons) ;
            self.farm.insert(farm_id, &act);
            let token = act.token_id;
            let sender_id = env::predecessor_account_id();
            self.internal_send_tokens(&sender_id, &token, amount) ;
            new_balance
        } else {
            env::panic(ERR20_ACCOUNT_NOT_OWNER.as_bytes());
        }
    }

    pub fn get_owner(&self) -> String {
        self.owner_id.to_string()
    }
   
    pub fn get_farm(&self , farm_id : &u128) ->Tokens {
        let token = self.internal_unwrap_account(&farm_id);
        token
    }

    pub fn get_last_farm(&self) ->Tokens {
        let farm_id  = self.get_len() -1 ;
        let token = self.internal_unwrap_account(&farm_id);
        token
    }

    pub fn get_sum_tokens(&self , token_id: TokenId) ->Balance {
        let mut sum_balace_token = 0;
        for (_k,v) in self.farm.to_vec() {
            if v.token_id == token_id {
                sum_balace_token += v.balance ;
            } 
        }
        sum_balace_token
    }

    pub fn start_time(&mut self,farm_id: &u128,time_register: &Time,time_start: &Time, time_about: Time) -> String {
        assert!(farm_id < &self.get_len(), "ID does not exist");
        let mut token = self.internal_unwrap_account(&farm_id) ;    
        let account_owner = env::predecessor_account_id();
        if account_owner == token.account {
            token.set_time(time_register,time_start,time_about);
            self.farm.insert(&farm_id, &token);
            return "success".to_string();
        }
        else {
            env::panic(ERR20_ACCOUNT_NOT_OWNER.as_bytes());
        }
    }

    pub fn get_farm_data_after_secons(&self ,account_id: &AccountId, time_secons: Time) -> Vec<FarmData>{
        let mut list_data: Vec<FarmData> = Vec::new();
        let list_account: Vec<Tokens> = self.get_farm_account(account_id.clone());
        let list_key: Vec<u128> = self.get_key_farm_account(account_id.clone());
        let mut i = 0;
        while i <list_account.len() {
            let mut time_about =  0 ;
            if time_secons > list_account[i].time_start {
                time_about = time_secons - list_account[i].time_start;
            }
            let balance = self.balance_after_time_secons(&list_key[i], &time_about);
            list_data.push(FarmData::new(list_key[i], list_account[i].account.clone(),list_account[i].time_register, list_account[i].time_start, list_account[i].time_end, list_account[i].token_id.clone(),list_account[i].balance, balance) );
            i +=1 ;
        }
        list_data
    }

    pub fn remove_farm(&mut self, farm_id : &u128) -> String{
        let owner_id = env::predecessor_account_id();
        assert!(owner_id == self.get_owner(), "NOT OWNER");
        self.farm.remove(farm_id);
        "success".to_string()
    }

    #[private]
    pub fn exchange_callback_post_withdraw(
        &mut self,
        _token_id: AccountId,
        sender_id: AccountId,
        _amount: U128,
    ) {
        assert_eq!(
            env::promise_results_count(),
            1,
            "{}",
            ERR25_CALLBACK_POST_WITHDRAW_INVALID
        );
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {}
            PromiseResult::Failed => {
                        env::log(
                            format!(
                                "Account {} has not enough storage. Depositing to owner.",
                                sender_id
                            )
                            .as_bytes(),
                        );
            }
        };
    }



}

impl Contract {

    pub fn internal_unwrap_account(&self, id: &u128) -> Tokens {
        self.internal_get_account(id)
            .expect("NOT ID!!!")
        // self.accounts.get(account_id).unwrap()
    }

    pub fn internal_get_account(&self, id: &u128) -> Option<Tokens> {
        self.farm.get(id)
    }

    pub(crate) fn internal_deposit(&mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) {
        let farm_id = self.get_len();
        let x = Tokens::new(&sender_id,&token_id,amount) ;
        self.farm.insert(&farm_id, &x.into());      
    }

    pub fn balance_after_time_day(&self ,farm_id : &u128,day: &Time) -> Time {
        let token = self.internal_unwrap_account(&farm_id) ;
        let about_time = day * TIME_DAY ;
        return  token.get_balance_after_time(about_time);

    }

    pub fn balance_after_time_secons(&self ,farm_id : &u128,time_secons: &Time) -> Time {
        let token = self.internal_unwrap_account(&farm_id) ;
        return  token.get_balance_after_time(time_secons.clone());

    }

    pub fn get_farm_account(&self , account_id: AccountId) -> Vec<Tokens> {
        let mut list_token: Vec<Tokens> = Vec::new();
        for (_k,v) in self.farm.to_vec() {
            if v.account == account_id {
                list_token.push(v);
            } 
        }
        list_token
    }

    pub fn get_key_farm_account(&self , account_id: AccountId) -> Vec<u128> {
        let mut list_token: Vec<u128> = Vec::new();
        for (k,v) in self.farm.to_vec() {
            if v.account == account_id {
                list_token.push(k);
            } 
        }
        list_token
    }

    /// Sends given amount to given user and if it fails, returns it back to user's balance.
    /// Tokens must already be subtracted from internal balance.
    pub(crate) fn internal_send_tokens(
        &self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) -> Promise {
        ext_fungible_token::ft_transfer(
            sender_id.clone(),
            U128(amount),
            None,
            token_id,
            1,
            GAS_FOR_FT_TRANSFER,
        )
        .then(ext_self::exchange_callback_post_withdraw(
            token_id.clone(),
            sender_id.clone(),
            amount,
            &env::current_account_id(),
            0,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
    }

    // pub fn internal_save_account(&mut self, account_id: &u128, token: Tokens) {
    //      //token.assert_storage_usage();
    //     self.farm.insert(&account_id, &token.into());
    // }

    // pub fn deposit_account(&mut self ,account: &AccountId) -> String {
       
    //     return "tao thanh cong".to_string() ;
    // }
    
   
}
