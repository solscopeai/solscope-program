// programs/solscope/src/mod.rs

pub mod assert_vault;
pub mod fund_vault;
pub mod register_bot;
pub mod withdraw;

// ✅ Re-export ONLY account structs + args
pub use assert_vault::AssertVault;
pub use fund_vault::FundVault;
pub use register_bot::{RegisterBot, RegisterBotArgs};
pub use withdraw::Withdraw;
