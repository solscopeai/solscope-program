// programs/solscope/src/execute_trade.rs

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount},
};

use crate::{errors::SolscopeError, state::BotMeta};

pub const SIDE_BUY: u8 = 0;
pub const SIDE_SELL: u8 = 1;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct ExecuteTrade<'info> {
    /// OPTIONAL FOR v1:
    /// Keep owner as signer while testing in Playground.
    /// Later, you can remove Signer and allow “keepers” to call it permissionlessly.
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"bot", owner.key().as_ref(), &bot_id_hash],
        bump,
        has_one = owner,
        has_one = vault
    )]
    pub bot_meta: Account<'info, BotMeta>,

    /// Vault PDA (SOL vault)
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref(), &bot_id_hash],
        bump
    )]
    pub vault: SystemAccount<'info>,

    /// Token mint you are buying/selling
    pub mint: Account<'info, Mint>,

    /// Vault’s ATA for this mint (created if needed)
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    // Programs
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ExecuteTrade>,
    bot_id_hash: [u8; 32],
    side: u8,
    amount_in: u64,
    min_out: u64,
) -> Result<()> {
    require!(amount_in > 0, SolscopeError::InvalidAmount);

    // hard gates
    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );
    require!(
        !ctx.accounts.bot_meta.paused,
        SolscopeError::BotPaused
    );

    require!(
        side == SIDE_BUY || side == SIDE_SELL,
        SolscopeError::Unauthorized
    );

    // From here:
    // - BUY: spend SOL from vault -> swap -> receive tokens into vault_ata
    // - SELL: spend tokens from vault_ata -> swap -> receive SOL into vault
    //
    // Next step is Raydium CPI wiring.
    // For now, this skeleton ensures:
    // ✅ correct PDAs
    // ✅ correct token destination (vault ATA)
    // ✅ slippage parameter present (min_out)

    Ok(())
}
