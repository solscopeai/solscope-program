// programs/solscope/src/fund_vault.rs

use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::errors::SolscopeError;
use crate::state::BotMeta;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct FundVault<'info> {
    /// Bot owner funding the vault
    #[account(mut)]
    pub owner: Signer<'info>,

    /// BotMeta PDA (must link owner + vault)
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

    /// Vault PDA (lamports-only system account)
    #[account(
        mut,
        seeds = [
            b"vault",
            owner.key().as_ref(),
            &bot_id_hash,
        ],
        bump
    )]
    pub vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<FundVault>, bot_id_hash: [u8; 32], amount: u64) -> Result<()> {
    require!(amount > 0, SolscopeError::InvalidAmount);

    // extra safety: bot_id must match + not paused
    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );
    require!(!ctx.accounts.bot_meta.paused, SolscopeError::BotPaused);

    let cpi_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.owner.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        },
    );

    system_program::transfer(cpi_ctx, amount)?;
    Ok(())
}
