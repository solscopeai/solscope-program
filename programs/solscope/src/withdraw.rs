// programs/solscope/src/withdraw.rs

use anchor_lang::prelude::*;
use anchor_lang::system_program;

use crate::errors::SolscopeError;
use crate::state::BotMeta;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct Withdraw<'info> {
    /// Bot owner (must match BotMeta + vault derivation)
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
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<Withdraw>,
    bot_id_hash: [u8; 32],
    amount: u64,
) -> Result<()> {
    require!(amount > 0, SolscopeError::InvalidAmount);

    // hard gates
    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );
    require!(
        !ctx.accounts.bot_meta.paused,
        SolscopeError::BotPaused
    );

    let vault_lamports = ctx.accounts.vault.to_account_info().lamports();
    require!(vault_lamports >= amount, SolscopeError::InsufficientVaultFunds);

    let seeds: &[&[u8]] = &[
        b"vault",
        ctx.accounts.owner.key.as_ref(),
        &bot_id_hash,
        &[ctx.bumps.vault],
    ];
    let signer_seeds = &[seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.system_program.to_account_info(),
        system_program::Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.owner.to_account_info(),
        },
        signer_seeds,
    );

    system_program::transfer(cpi_ctx, amount)?;
    Ok(())
}
