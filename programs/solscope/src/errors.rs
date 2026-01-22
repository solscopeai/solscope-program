// programs/solscope/src/errors.rs

use anchor_lang::prelude::*;

#[error_code]
pub enum SolscopeError {
    /* ======================================================
     * Vault & Balance Errors
     * ====================================================== */
    #[msg("Invalid vault account")]
    InvalidVault,

    #[msg("Insufficient vault funds")]
    InsufficientVaultFunds,

    #[msg("Invalid amount")]
    InvalidAmount,

    /* ======================================================
     * Bot Identity & State Errors
     * ====================================================== */
    #[msg("Bot ID hash mismatch")]
    BotIdMismatch,

    #[msg("Bot is paused")]
    BotPaused,

    #[msg("Invalid BotMeta bump")]
    InvalidBotMetaBump,

    /* ======================================================
     * Generic / Safety
     * ====================================================== */
    #[msg("Unauthorized operation")]
    Unauthorized,

    #[msg("Slippage exceeded minimum output")]
    SlippageExceeded,
}
