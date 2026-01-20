// programs/solscope/src/assert_vault.rs

use anchor_lang::prelude::*;

use crate::errors::SolscopeError;
use crate::state::BotMeta;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct AssertVault<'info> {
    /// Bot owner
    pub owner: Signer<'info>,

    /// BotMeta PDA (1 per bot)
    #[account(
        seeds = [
            b"bot",
            owner.key().as_ref(),
            &bot_id_hash,
        ],
        bump,
        has_one = owner,
        has_one = vault
    )]
    pub bot_meta: Account<'info, BotMeta>,

    /// Vault PDA (SOL-only system account)
    #[account(
        seeds = [
            b"vault",
            owner.key().as_ref(),
            &bot_id_hash,
        ],
        bump
    )]
    pub vault: SystemAccount<'info>,
}

pub fn handler(ctx: Context<AssertVault>, bot_id_hash: [u8; 32]) -> Result<()> {
    // bot_id_hash must match what’s recorded
    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );

    // emergency pause
    require!(
        !ctx.accounts.bot_meta.paused,
        SolscopeError::BotPaused
    );

    // optional: ensure stored bump matches derived bump
    require!(
        ctx.accounts.bot_meta.bump == ctx.bumps.bot_meta,
        SolscopeError::InvalidBotMetaBump
    );

    Ok(())
}
