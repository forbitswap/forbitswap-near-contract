use near_sdk::collections::UnorderedMap;

use crate::utils::{ext_self, GAS_FOR_FT_TRANSFER, GAS_FOR_RESOLVE_TRANSFER};
use crate::*;
use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{env, AccountId, Balance, StorageUsage};

// USAGE UNIT
const U128_STORAGE: StorageUsage = 16;
const U64_STORAGE: StorageUsage = 8;
const U32_STORAGE: StorageUsage = 4;
/// max length of account id is 64 bytes. We charge per byte.
const ACC_ID_STORAGE: StorageUsage = 64;
/// As a key, 4 bytes length would be added to the head
const ACC_ID_AS_KEY_STORAGE: StorageUsage = ACC_ID_STORAGE + 4;
const KEY_PREFIX_ACC: StorageUsage = 64;
/// As a near_sdk::collection key, 1 byte for prefiex
const ACC_ID_AS_CLT_KEY_STORAGE: StorageUsage = ACC_ID_AS_KEY_STORAGE + 1;

// ACC_ID: the Contract accounts map key length
// + VAccount enum: 1 byte
// + U128_STORAGE: near_amount storage
// + U32_STORAGE: legacy_tokens HashMap length
// + U32_STORAGE: tokens HashMap length
// + U64_STORAGE: storage_used
pub const INIT_ACCOUNT_STORAGE: StorageUsage =
    ACC_ID_AS_CLT_KEY_STORAGE + 1 + U128_STORAGE + U32_STORAGE + U64_STORAGE;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    pub near_amount: Balance,
    pub tokens: UnorderedMap<AccountId, Balance>,
    pub storage_used: StorageUsage,
}

impl Account {
    pub fn new(account_id: &AccountId) -> Self {
        Account {
            near_amount: 0,
            tokens: UnorderedMap::new(StorageKey::AccountTokens {
                account_id: account_id.clone(),
            }),
            storage_used: 0,
        }
    }

    pub fn get_balance(&self, token_id: &AccountId) -> Option<Balance> {
        if let Some(token_balance) = self.tokens.get(token_id) {
            Some(token_balance)
        } else {
            None
        }
    }

    /// Returns minimal account deposit storage usage possible.
    pub fn min_storage_usage() -> Balance {
        INIT_ACCOUNT_STORAGE as Balance * env::storage_byte_cost()
    }

    pub(crate) fn register(&mut self, token_ids: &Vec<ValidAccountId>) {
        for token_id in token_ids {
            let t = token_id.as_ref();
            if self.get_balance(t).is_none() {
                self.tokens.insert(t, &0);
            }
        }
    }

    pub(crate) fn unregister(&mut self, token_id: &AccountId) {
        let amount = self.tokens.remove(token_id).unwrap_or_default();
        assert_eq!(amount, 0, "{}", ERR24_NON_ZERO_TOKEN_BALANCE);
    }

    pub(crate) fn withdraw(&mut self, token: &AccountId, amount: Balance) {
        if let Some(x) = self.tokens.get(token) {
            assert!(x >= amount, "{}", ERR22_NOT_ENOUGH_TOKENS);
            self.tokens.insert(token, &(x - amount));
        } else {
            env::panic(ERR21_TOKEN_NOT_REG.as_bytes());
        }
    }

    pub fn assert_storage_usage(&self) {
        assert!(
            self.storage_usage() <= self.near_amount,
            "{}",
            ERR11_INSUFFICIENT_STORAGE
        )
    }

    pub fn storage_usage(&self) -> Balance {
        (INIT_ACCOUNT_STORAGE
            + self.tokens.len() as u64 * (KEY_PREFIX_ACC + ACC_ID_AS_KEY_STORAGE + U128_STORAGE))
            as u128
            * env::storage_byte_cost()
    }

    pub fn storage_available(&self) -> Balance {
        // [AUDIT_01] avoid math overflow
        let locked = self.storage_usage();
        if self.near_amount > locked {
            self.near_amount - locked
        } else {
            0
        }
    }

    /// Deposit amount to the balance of given token,
    /// if given token not register and not enough storage, deposit fails
    pub(crate) fn deposit_with_storage_check(
        &mut self,
        token: &AccountId,
        amount: Balance,
    ) -> bool {
        if let Some(balance) = self.tokens.get(token) {
            // token has been registered, just add without storage check,
            let new_balance = balance + amount;
            self.tokens.insert(token, &new_balance);
            true
        } else {
            // check storage after insert, if fail should unregister the token
            self.tokens.insert(token, &(amount));
            if self.storage_usage() <= self.near_amount {
                true
            } else {
                self.tokens.remove(token);
                false
            }
        }
    }

    /// Deposit amount to the balance of given token.
    pub(crate) fn deposit(&mut self, token: &AccountId, amount: Balance) {
        if let Some(x) = self.tokens.get(token) {
            self.tokens.insert(token, &(amount + x));
        } else {
            self.tokens.insert(token, &amount);
        }
    }
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn register_tokens(&mut self, token_ids: Vec<ValidAccountId>) {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        account.register(&token_ids);
        self.internal_save_account(&sender_id, account);
    }

    #[payable]
    pub fn unregister_tokens(&mut self, token_ids: Vec<ValidAccountId>) {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);
        for token_id in token_ids {
            account.unregister(token_id.as_ref());
        }
        self.internal_save_account(&sender_id, account);
    }

    /// Withdraws given token from the deposits of given user.
    /// a zero amount means to withdraw all in user's inner account.
    /// Optional unregister will try to remove record of this token from AccountDeposit for given user.
    /// Unregister will fail if the left over balance is non 0.
    #[payable]
    pub fn withdraw(
        &mut self,
        token_id: ValidAccountId,
        amount: U128,
        unregister: Option<bool>,
    ) -> Promise {
        assert_one_yocto();
        let token_id: AccountId = token_id.into();
        let sender_id = env::predecessor_account_id();
        let mut account = self.internal_unwrap_account(&sender_id);

        // get full amount if amount param is 0
        let mut amount: u128 = amount.into();
        if amount == 0 {
            amount = account.get_balance(&token_id).expect(ERR21_TOKEN_NOT_REG);
        }
        assert!(amount > 0, "{}", ERR29_ILLEGAL_WITHDRAW_AMOUNT);

        // Note: subtraction and deregistration will be reverted if the promise fails.
        account.withdraw(&token_id, amount);
        if unregister == Some(true) {
            account.unregister(&token_id);
        }
        self.internal_save_account(&sender_id, account);
        self.internal_send_tokens(&sender_id, &token_id, amount)
    }

    #[private]
    pub fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: U128,
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
                // This reverts the changes from withdraw function.
                // If account doesn't exit, deposits to the owner's account as lostfound.
                let mut failed = false;
                if let Some(mut account) = self.internal_get_account(&sender_id) {
                    if account.deposit_with_storage_check(&token_id, amount.0) {
                        // cause storage already checked, here can directly save
                        self.accounts.insert(&sender_id, &account.into());
                    } else {
                        // we can ensure that internal_get_account here would NOT cause a version upgrade,
                        // cause it is callback, the account must be the current version or non-exist,
                        // so, here we can just leave it without insert, won't cause storage collection inconsistency.
                        env::log(
                            format!(
                                "Account {} has not enough storage. Depositing to owner.",
                                sender_id
                            )
                            .as_bytes(),
                        );
                        failed = true;
                    }
                } else {
                    env::log(
                        format!(
                            "Account {} is not registered. Depositing to owner.",
                            sender_id
                        )
                        .as_bytes(),
                    );
                    failed = true;
                }
                if failed {
                    self.internal_lostfound(&token_id, amount.0);
                }
            }
        };
    }
}
impl Contract {
    // unwrap an account from Option<Account> to Account
    pub fn internal_unwrap_account(&self, account_id: &AccountId) -> Account {
        self.internal_get_account(account_id)
            .expect("ACCOUNT IS NOT REGISTER!!!")

        // self.accounts.get(account_id).unwrap()
    }
    pub(crate) fn internal_deposit(
        &mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) {
        let mut account = self.internal_unwrap_account(sender_id);
        assert!(
            self.whitelisted_tokens.contains(token_id) || account.get_balance(token_id).is_some(),
            "{}",
            ERR12_TOKEN_NOT_WHITELISTED
        );
        account.deposit(token_id, amount);
        self.internal_save_account(&sender_id, account);
    }

    // Return Option<Account> with accout_id
    pub fn internal_get_account(&self, account_id: &AccountId) -> Option<Account> {
        self.accounts.get(account_id)
    }
    /// save token to owner account as lostfound, no need to care about storage
    /// only global whitelisted token can be stored in lost-found
    pub(crate) fn internal_lostfound(&mut self, token_id: &AccountId, amount: u128) {
        if self.whitelisted_tokens.contains(token_id) {
            let mut lostfound = self.internal_unwrap_or_default_account(&self.owner_id);
            lostfound.deposit(token_id, amount);
            self.accounts.insert(&self.owner_id, &lostfound.into());
        } else {
            env::panic("ERR: non-whitelisted token can NOT deposit into lost-found.".as_bytes());
        }
    }

    pub fn internal_save_account(&mut self, account_id: &AccountId, account: Account) {
        account.assert_storage_usage();
        self.accounts.insert(&account_id, &account.into());
    }

    pub(crate) fn internal_register_account(&mut self, account_id: &AccountId, amount: Balance) {
        let mut account = self.internal_unwrap_or_default_account(&account_id);
        account.near_amount += amount;
        self.internal_save_account(&account_id, account);
    }

    pub fn internal_unwrap_or_default_account(&self, account_id: &AccountId) -> Account {
        self.internal_get_account(account_id)
            .unwrap_or_else(|| Account::new(account_id))
    }

    pub(crate) fn internal_storage_withdraw(
        &mut self,
        account_id: &AccountId,
        amount: Balance,
    ) -> u128 {
        let mut account = self.internal_unwrap_account(&account_id);
        let available = account.storage_available();
        assert!(available > 0, "ERR_NO_STORAGE_CAN_WITHDRAW");
        let mut withdraw_amount = amount;
        if amount == 0 {
            withdraw_amount = available;
        }
        assert!(
            withdraw_amount <= available,
            "ERR_STORAGE_WITHDRAW_TOO_MUCH"
        );
        account.near_amount -= withdraw_amount;
        self.internal_save_account(&account_id, account);
        withdraw_amount
    }

    pub fn internal_transfer_from_user(
        &mut self,
        account_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
    ) {
        let mut account = self.internal_unwrap_or_default_account(account_id);
        assert!(amount > 0, "AMOUNT_MUST_BE_GREATER_THAN_0");
        let account_amount = account.tokens.get(token_id).unwrap_or_default();

        //  check if a token existed
        //      plus new + old value then save
        //  else
        //      insert and save
        if account_amount == 0 {
            account.tokens.insert(token_id, &amount);
        } else {
            account.tokens.insert(token_id, &(amount + account_amount));
        }

        self.internal_save_account(account_id, account);
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
            U128(amount),
            &env::current_account_id(),
            0,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
    }
}
