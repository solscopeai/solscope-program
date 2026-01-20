// programs/solscope/src/state.rs

use anchor_lang::prelude::*;

/* ======================================================
 * Bot Metadata (1 per bot)
 * ====================================================== */
#[account]
pub struct BotMeta {
    /// Wallet that owns this bot
    pub owner: Pubkey,

    /// Hash of bot ID (off-chain identifier)
    pub bot_id_hash: [u8; 32],

    /// Vault PDA that holds funds for this bot
    pub vault: Pubkey,

    /// Unix timestamp (seconds)
    pub created_at: i64,

    /// PDA bump for BotMeta
    pub bump: u8,

    /// Emergency pause flag
    pub paused: bool,
}

impl BotMeta {
    /// Anchor discriminator (8)
    /// owner Pubkey (32)
    /// bot_id_hash [u8;32] (32)
    /// vault Pubkey (32)
    /// created_at i64 (8)
    /// bump u8 (1)
    /// paused bool (1)
    pub const LEN: usize =
        32 + // owner
        32 + // bot_id_hash
        32 + // vault
        8  + // created_at
        1  + // bump
        1;   // paused
}
