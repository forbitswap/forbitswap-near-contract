use near_sdk::{AccountId, Balance};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

use crate::errors::*;
pub type TokenId = AccountId  ;
pub type Time = u128 ;
pub const APR: f64 = 120.0 ;
pub const MIN_TIME:u128 = 30;
pub const TIME_YEAR:u128 = 31536000;
// pub const TIME_DAY:u128 = 86400 ;
use near_sdk::{
    serde::{Deserialize, Serialize},
};

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running,
    Paused,
}
//1659064422

#[derive(BorshSerialize, BorshDeserialize)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Tokens {
    pub account: AccountId,
    pub time_register:Time,
    pub time_start: Time,
    pub time_end: Time,
    pub token_id: TokenId ,
    pub balance: Balance,
    pub state: RunningState,
}

impl Tokens {
    pub fn new(acc: &AccountId, token: &TokenId, amount: Balance) -> Self {
        Tokens {
           account : acc.to_string(),
           time_register:0,
           time_start:  0,
           time_end: 0 ,
           token_id: token.to_string() ,
           balance: amount,
           state: RunningState::Running
       }
    }
    pub fn set_time(&mut self,time_register: &Time,time_start: &Time, time_about: Time){
            self.time_register = time_register.clone();
            self.time_start = time_start.clone();
            self.time_end = self.time_start + time_about;
    }
      /// Deposit amount to the balance of given token.
    pub fn deposit(&mut self ,token: &TokenId, amount: Balance) {
        self.token_id = token.to_string() ;
        self.balance = amount ;
    }

    pub fn withdraw(&mut self, amount: Balance, time_secons: &Time) ->Balance {
        let mut after_time = 0 ;
        if time_secons.clone() > self.time_start {
            after_time = time_secons.clone() - self.time_start ;
        }
        if time_secons.clone() > self.time_end {
            after_time = self.time_end - self.time_start ;
        }
        let new_balance = self.get_balance_after_time(after_time);
        assert!(new_balance >= amount, "cannot exceed the current number : {} - (token {}  = {} < amount = {})", ERR22_NOT_ENOUGH_TOKENS,self.token_id,new_balance,amount);
        assert!(self.time_start > 0, "no start");
        assert!(after_time > MIN_TIME, "need to pass minimum time");
        if new_balance - amount > 0 {
            self.balance = self.get_old_balance(new_balance - amount, after_time);
            return new_balance - amount;
        }
        self.balance = 0;
        self.state = RunningState::Paused ;
        return 0 ;
    }

    pub fn get_old_balance(&self,amount: Balance ,about_time: Time) -> Balance{
        let year = ((about_time) as f64)/ (TIME_YEAR as f64) ;
        ((amount as f64) / (1.0 + year*(APR/100.0))) as Balance
    }

    pub fn get_balance_after_time(&self ,about_time: Time) -> Balance {
        let year = ((about_time) as f64)/ (TIME_YEAR as f64) ;
        let balance = self.balance;
        return (balance as Balance) + ((balance as f64)*year*(APR/100.0))as Balance ;

    }

    // pub fn get_real_time() -> Time{
    //     let start = SystemTime::now();
    //     let since_the_epoch = start
    //         .duration_since(UNIX_EPOCH)
    //         .expect("Time went backwards");
    //     since_the_epoch.as_millis()
    // }

    // pub fn get_balance(&self, token_id: &TokenId) -> Balance{
    //     let real_time = Tokens::get_real_time();
    //     let year = ((real_time - self.time_start) as f64)/ (TIME_YEAR as f64) ;
    //     let balance = self.balance;
    //     return (balance as Balance) + ((balance as f64)*year*(APR/100.0))as Balance ;
    // }

    // pub fn get_old_balance(&self,amount: Balance) -> Balance{
    //     let real_time = Tokens::get_real_time();
    //     let year = ((real_time - self.time_start) as f64)/ (TIME_YEAR as f64) ;
    //     ((amount as f64) / (1.0 + year*APR)) as Balance
    // }
   
}
