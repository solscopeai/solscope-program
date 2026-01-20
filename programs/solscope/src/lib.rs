// programs/solscope/src/lib.rs

use anchor_lang::prelude::*;

pub mod register_bot;
pub mod assert_vault;
pub mod fund_vault;
pub mod withdraw;
pub mod state;
pub mod errors;

pub use register_bot::*;
pub use assert_vault::*;
pub use fund_vault::*;
pub use withdraw::*;

declare_id!("pxrgZ1DR257Ahz7fBxUFUmE6w6kq9nktz6h7eFHTrZP");

#[program]
pub mod solscope {
    use super::*;

    /* ======================================================
     * Bot Registration
     * ====================================================== */
    pub fn register_bot(
        ctx: Context<RegisterBot>,
        bot_id_hash: [u8; 32],
    ) -> Result<()> {
        register_bot::handler(ctx, bot_id_hash)
    }

    /* ======================================================
     * Vault Assertions (HARD SAFETY GATE)
     * ====================================================== */
    pub fn assert_vault(
        ctx: Context<AssertVault>,
        bot_id_hash: [u8; 32],
    ) -> Result<()> {
        assert_vault::handler(ctx, bot_id_hash)
    }

    /* ======================================================
     * Funding
     * ====================================================== */
    pub fn fund_vault(
        ctx: Context<FundVault>,
        bot_id_hash: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        fund_vault::handler(ctx, bot_id_hash, amount)
    }

    /* ======================================================
     * Withdrawals
     * ====================================================== */
    pub fn withdraw(
        ctx: Context<Withdraw>,
        bot_id_hash: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        withdraw::handler(ctx, bot_id_hash, amount)
    }
}
