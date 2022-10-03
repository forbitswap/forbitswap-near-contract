// use std::time::{SystemTime, UNIX_EPOCH};

// mod utils {
// pub fn get_real_time() -> u128{
//     let start = SystemTime::now();
//     let since_the_epoch = start
//         .duration_since(UNIX_EPOCH)
//         .expect("Time went backwards");
//     since_the_epoch.as_millis
// }
// }

pub const GAS_FOR_FT_TRANSFER: Gas = 20_000_000_000_000;
pub const GAS_FOR_RESOLVE_TRANSFER: Gas = 20_000_000_000_000;
use near_sdk::{ext_contract, Gas};

#[ext_contract(ext_self)]
pub trait Exchange {
    fn exchange_callback_post_withdraw(
        &mut self,
        token_id: AccountId,
        sender_id: AccountId,
        amount: u128,
    );
}
