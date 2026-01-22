// programs/solscope/src/register_bot.rs

use anchor_lang::prelude::*;

use crate::state::BotMeta;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct RegisterBot<'info> {
    /// Bot owner
    #[account(mut)]
    pub owner: Signer<'info>,

    /// BotMeta PDA (1 per bot)
    #[account(
        init,
        payer = owner,
        space = 8 + BotMeta::LEN,
        seeds = [
            b"bot",
            owner.key().as_ref(),
            &bot_id_hash,
        ],
        bump
    )]
    pub bot_meta: Account<'info, BotMeta>,

    /// Vault PDA (SOL-only system account)
    #[account(
        init,
        payer = owner,
        space = 0, // SOL-only account
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

pub fn handler(ctx: Context<RegisterBot>, bot_id_hash: [u8; 32]) -> Result<()> {
    let bot_meta = &mut ctx.accounts.bot_meta;

    bot_meta.owner = ctx.accounts.owner.key();
    bot_meta.bot_id_hash = bot_id_hash;
    bot_meta.vault = ctx.accounts.vault.key();
    bot_meta.created_at = Clock::get()?.unix_timestamp;
    bot_meta.bump = ctx.bumps.bot_meta;
    bot_meta.paused = false;

    Ok(())
}
