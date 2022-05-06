use std::convert::TryInto;
use std::fmt;

use actions::{ActionResult, SwapAction};
use admin_fee::AdminFees;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet, Vector};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, near_bindgen, BorshStorageKey, Promise, PromiseResult, StorageUsage,
};
use pool::Pool;
use simple_pool::SimplePool;
use std::collections::HashMap;
use utils::check_duplicate_tokens;
use crate::utils::{ext_self, GAS_FOR_FT_TRANSFER, GAS_FOR_RESOLVE_TRANSFER};
use crate::account::Account;
use crate::actions::Action;
use crate::errors::*;

mod account;
mod actions;
mod admin_fee;
mod errors;
mod owner;
mod pool;
mod simple_pool;
mod storage_impl;
mod token_receiver;
mod utils;
mod views;

pub type AccountId = String;
pub type Balance = u128;

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Account,
    AccountTokens { account_id: AccountId },
    Whitelist,
    Shares { pool_id: u32 },
    Pools,
    Guardian,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum RunningState {
    Running,
    Paused,
}
impl fmt::Display for RunningState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RunningState::Running => write!(f, "Running"),
            RunningState::Paused => write!(f, "Paused"),
        }
    }
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Contract {
    /// Account of the owner
    owner_id: AccountId,

    /// account with it's information store in Account
    accounts: LookupMap<AccountId, Account>,

    /// List of all the pools
    pools: Vector<Pool>,

    exchange_fee: u32,

    referral_fee: u32,

    /// Set of whitelisted tokens by "owner"
    whitelisted_tokens: UnorderedSet<AccountId>,

    /// Set of guardians.
    guardians: UnorderedSet<AccountId>,
    /// Running state
    state: RunningState,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            owner_id: env::predecessor_account_id(),
            exchange_fee: 0,
            referral_fee: 0,
            accounts: LookupMap::new(StorageKey::Account),
            pools: Vector::new(StorageKey::Pools),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            guardians: UnorderedSet::new(StorageKey::Guardian),
            state: RunningState::Running,
        }
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            accounts: LookupMap::new(StorageKey::Account),
            exchange_fee: 30,
            referral_fee: 0,
            pools: Vector::new(StorageKey::Pools),
            whitelisted_tokens: UnorderedSet::new(StorageKey::Whitelist),
            guardians: UnorderedSet::new(StorageKey::Guardian),
            state: RunningState::Running,
        }
    }

    #[payable]
    pub fn add_simple_pool(&mut self, tokens: Vec<ValidAccountId>, fee: u32) -> u64 {
        self.assert_contract_running();
        check_duplicate_tokens(&tokens);
        self.internal_check_existed_pool(&tokens);
        self.internal_add_pool(Pool::SimplePool(SimplePool::new(
            self.pools.len() as u32,
            tokens,
            fee,
            0,
            0,
        )))
    }

    /// Add liquidity from already deposited amounts to given pool.
    #[payable]
    pub fn add_liquidity(
        &mut self,
        pool_id: u64,
        amounts: Vec<U128>,
        min_amounts: Option<Vec<U128>>,
    ) {
        self.assert_contract_running();
        assert!(
            env::attached_deposit() > 0,
            "Requires attached deposit of at least 1 yoctoNEAR"
        );
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let mut amounts: Vec<u128> = amounts.into_iter().map(|amount| amount.into()).collect();
        let mut pool = self.pools.get(pool_id).expect("ERR_NO_POOL");
        // Add amounts given to liquidity first. It will return the balanced amounts.
        pool.add_liquidity(&sender_id, &mut amounts);
        if let Some(min_amounts) = min_amounts {
            // Check that all amounts are above request min amounts in case of front running that changes the exchange rate.
            for (amount, min_amount) in amounts.iter().zip(min_amounts.iter()) {
                assert!(amount >= &min_amount.0, "ERR_MIN_AMOUNT");
            }
        }
        let mut deposits_acc = self.internal_unwrap_or_default_account(&sender_id);
        let tokens = pool.tokens();
        // Subtract updated amounts from deposits. This will fail if there is not enough funds for any of the tokens.
        for i in 0..tokens.len() {
            deposits_acc.withdraw(&tokens[i], amounts[i]);
        }
        self.internal_save_account(&sender_id, deposits_acc);
        self.pools.replace(pool_id, &pool);
        self.internal_check_storage(prev_storage);
    }

    /// Remove liquidity from the pool into general pool of liquidity.
    #[payable]
    pub fn remove_liquidity(&mut self, pool_id: u64, shares: U128, min_amounts: Vec<U128>) {
        assert_one_yocto();
        self.assert_contract_running();
        let prev_storage = env::storage_usage();
        let sender_id = env::predecessor_account_id();
        let mut pool = self.pools.get(pool_id).expect("ERR_NO_POOL");
        let amounts = pool.remove_liquidity(
            &sender_id,
            shares.into(),
            min_amounts
                .into_iter()
                .map(|amount| amount.into())
                .collect(),
        );
        self.pools.replace(pool_id, &pool);
        let tokens = pool.tokens();
        let mut deposits = self.internal_unwrap_or_default_account(&sender_id);
        for i in 0..tokens.len() {
            deposits.deposit(&tokens[i], amounts[i]);
        }
        // Freed up storage balance from LP tokens will be returned to near_balance.
        if prev_storage > env::storage_usage() {
            deposits.near_amount +=
                (prev_storage - env::storage_usage()) as Balance * env::storage_byte_cost();
        }
        self.internal_save_account(&sender_id, deposits);
    }

    /// [AUDIT_03_reject(NOPE action is allowed by design)]
    /// [AUDIT_04]
    /// Executes generic set of actions.
    /// If referrer provided, pays referral_fee to it.
    /// If no attached deposit, outgoing tokens used in swaps must be whitelisted.
    #[payable]
    pub fn execute_actions(
        &mut self,
        actions: Vec<Action>,
        referral_id: Option<ValidAccountId>,
    ) -> ActionResult {
        self.assert_contract_running();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        // Validate that all tokens are whitelisted if no deposit (e.g trade with access key)
        if env::attached_deposit() == 0 {
            for action in &actions {
                for token in action.tokens() {
                    assert!(
                        account.get_balance(&token).is_some()
                            || self.whitelisted_tokens.contains(&token),
                        "{}",
                        // [AUDIT_05]
                        ERR27_DEPOSIT_NEEDED
                    );
                }
            }
        }

        let referral_id = referral_id.map(|r| r.into());
        let result =
            self.internal_execute_actions(&mut account, &referral_id, &actions, ActionResult::None);
        self.internal_save_account(&sender_id, account);
        result
    }

    #[payable]
    pub fn swap(&mut self, actions: Vec<SwapAction>, referral_id: Option<ValidAccountId>) -> U128 {
        self.assert_contract_running();
        assert_ne!(actions.len(), 0, "ERR_AT_LEAST_ONE_SWAP");
        U128(
            self.execute_actions(
                actions
                    .into_iter()
                    .map(|swap_action| Action::Swap(swap_action))
                    .collect(),
                referral_id,
            )
            .to_amount(),
        )
    }

    pub fn is_lp(&self, account_id: &ValidAccountId, pool_id: u64) -> bool {
        // let filterd_pools: Vec<SimplePool> = pools.iter().filter(|a|a);
        self.pools
            .get(pool_id)
            .expect("ERR_NO_POOL")
            .is_lp(account_id.as_ref())
            .into()

        // for pool in pools.iter() {
        //     if pool.is_lp(&account_id) {
        //         return true;
        //     } else {
        //         return false;
        //     }
        // }
        // false
    }
}

impl Contract {
    fn assert_contract_running(&self) {
        match self.state {
            RunningState::Running => (),
            _ => env::panic(ERR51_CONTRACT_PAUSED.as_bytes()),
        };
    }
    // Adds given pool to the list and returns it's id.
    /// If there is not enough attached balance to cover storage, fails.
    /// If too much attached - refunds it back.
    fn internal_add_pool(&mut self, mut pool: Pool) -> u64 {
        let prev_storage = env::storage_usage();
        let id = self.pools.len() as u64;
        // exchange share was registered at creation time
        pool.share_register(&env::current_account_id());
        pool.share_register(&env::signer_account_id());
        self.pools.push(&pool);
        self.internal_check_storage(prev_storage);
        id
    }

    /// Check how much storage taken costs and refund the left over back.
    fn internal_check_storage(&self, prev_storage: StorageUsage) {
        let storage_cost = env::storage_usage()
            .checked_sub(prev_storage)
            .unwrap_or_default() as Balance
            * env::storage_byte_cost();

        let refund = env::attached_deposit().checked_sub(storage_cost).expect(
            format!(
                "ERR_STORAGE_DEPOSIT need {}, attatched {}",
                storage_cost,
                env::attached_deposit()
            )
            .as_str(),
        );
        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund);
        }
    }

    /// Execute sequence of actions on given account. Modifies passed account.
    /// Returns result of the last action.
    fn internal_execute_actions(
        &mut self,
        account: &mut Account,
        referral_id: &Option<AccountId>,
        actions: &[Action],
        prev_result: ActionResult,
    ) -> ActionResult {
        let mut result = prev_result;
        for action in actions {
            result = self.internal_execute_action(account, referral_id, action, result);
        }
        result
    }

    /// Executes single action on given account. Modifies passed account. Returns a result based on type of action.
    fn internal_execute_action(
        &mut self,
        account: &mut Account,
        referral_id: &Option<AccountId>,
        action: &Action,
        prev_result: ActionResult,
    ) -> ActionResult {
        match action {
            Action::Swap(swap_action) => {
                let amount_in = swap_action
                    .amount_in
                    .map(|value| value.0)
                    .unwrap_or_else(|| prev_result.to_amount());

                // Take amount of `token_in` out from account to pool.
                account.withdraw(&swap_action.token_in, amount_in);

                let amount_out = self.internal_pool_swap(
                    swap_action.pool_id,
                    &swap_action.token_in,
                    amount_in,
                    &swap_action.token_out,
                    swap_action.min_amount_out.0,
                    referral_id,
                );

                account.deposit(&swap_action.token_out, amount_out);
                // [AUDIT_02]
                ActionResult::Amount(U128(amount_out))
            }
        }
    }

    /// Swaps given amount_in of token_in into token_out via given pool.
    /// Should be at least min_amount_out or swap will fail (prevents front running and other slippage issues).
    fn internal_pool_swap(
        &mut self,
        pool_id: u64,
        token_in: &AccountId,
        amount_in: u128,
        token_out: &AccountId,
        min_amount_out: u128,
        referral_id: &Option<AccountId>,
    ) -> u128 {
        let mut pool = self.pools.get(pool_id).expect("ERR_NO_POOL");
        let amount_out = pool.swap(
            token_in,
            amount_in,
            token_out,
            min_amount_out,
            AdminFees {
                exchange_fee: self.exchange_fee,
                exchange_id: env::current_account_id(),
                referral_fee: self.referral_fee,
                referral_id: referral_id.clone(),
            },
        );
        self.pools.replace(pool_id, &pool);
        amount_out
    }

    /// Program will panic if input token pair exsists.
    fn internal_check_existed_pool(&self, tokens: &Vec<ValidAccountId>) {
        assert_eq!(tokens.len(), 2, "INVALID NUMBER OF TOKENS");

        let pools = &self.pools;
        for pool in pools.iter() {
            let is_existed = pool.check_existed_pool(&tokens);
            assert!(!is_existed, "THIS POOL PAIR ALREADY EXISTED!!!");
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use near_contract_standards::storage_management::StorageManagement;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, Balance, MockedBlockchain};

    const ONE_NEAR: u128 = 1_000_000_000_000_000_000_000_000;

    fn setup_contract() -> (VMContextBuilder, Contract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context
            .predecessor_account_id(accounts(0))
            .attached_deposit(ONE_NEAR)
            .build());
        let contract = Contract::new(accounts(0).to_string());
        (context, contract)
    }

    #[test]
    fn test_deposit_token() {
        let token_id = accounts(3);
        let (_, mut contract) = setup_contract();
        let amount: Balance = 10000;
        let account_id = accounts(1);

        contract.storage_deposit(Some(account_id), Some(false));

        let account_id = accounts(1);

        contract.internal_transfer_from_user(
            &account_id.to_string(),
            &token_id.to_string(),
            amount,
        );
        assert!(
            contract
                .accounts
                .get(&account_id.to_string())
                .unwrap()
                .tokens
                .get(&token_id.to_string())
                .unwrap()
                == 10000,
            "TEST FAILED!!!!"
        )
    }
    #[test]
    #[should_panic("ERR_TRANSFER_AMOUNT_EQUAL_ZERO")]
    fn test_deposit_token_with_zero_amount() {
        let token_id = accounts(3);
        let (_, mut contract) = setup_contract();
        let amount: Balance = 0;
        let account_id = accounts(1);

        contract.storage_deposit(Some(account_id), Some(false));

        let account_id = accounts(1);

        contract.internal_transfer_from_user(
            &account_id.to_string(),
            &token_id.to_string(),
            amount,
        );
    }
}
