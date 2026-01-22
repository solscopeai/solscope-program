// programs/solscope/src/set_paused.rs

use anchor_lang::prelude::*;
use crate::state::BotMeta;

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct SetPaused<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"bot", owner.key().as_ref(), &bot_id_hash],
        bump,
        has_one = owner,
    )]
    pub bot_meta: Account<'info, BotMeta>,
}

pub fn handler(ctx: Context<SetPaused>, _bot_id_hash: [u8; 32], paused: bool) -> Result<()> {
    // owner-only enforced by has_one + owner signer
    ctx.accounts.bot_meta.paused = paused;
    Ok(())
}

