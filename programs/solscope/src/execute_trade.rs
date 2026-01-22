// programs/solscope/src/execute_trade.rs

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    system_instruction,
};
use std::str::FromStr;

use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, CloseAccount, Mint, SyncNative, Token, TokenAccount},
};

use crate::{errors::SolscopeError, state::BotMeta};

pub const SIDE_BUY: u8 = 0;
pub const SIDE_SELL: u8 = 1;

/* ======================================================
 * Helpers
 * ====================================================== */

fn raydium_amm_program() -> Pubkey {
    Pubkey::from_str("RVKd61ztZW9KQqkHn7kYk9Z3n5Vf3L7hPwrKyYVJZZz").unwrap()
}

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct ExecuteTrade<'info> {
    /// Bot owner (payer for ATA creation)
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"bot", owner.key().as_ref(), &bot_id_hash],
        bump,
        has_one = owner,
        has_one = vault
    )]
    pub bot_meta: Account<'info, BotMeta>,

    /// CHECK: Vault PDA (system-owned, SOL-only)
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref(), &bot_id_hash],
        bump
    )]
    pub vault: AccountInfo<'info>,

    /// Token being purchased
    pub mint: Account<'info, Mint>,

    /// Vault ATA for output token
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    /// CHECK: Temporary wSOL account owned by vault
    #[account(mut)]
    pub vault_wsol: AccountInfo<'info>,

    /* =========================
     * Raydium AMM Accounts
     * ========================= */
    /// CHECK: Raydium AMM program
    #[account(address = raydium_amm_program())]
    pub amm_program: AccountInfo<'info>,

    /// CHECK: AMM state
    #[account(mut)]
    pub amm: AccountInfo<'info>,

    /// CHECK: AMM authority
    pub amm_authority: AccountInfo<'info>,

    /// CHECK: AMM open orders
    #[account(mut)]
    pub amm_open_orders: AccountInfo<'info>,

    /// CHECK: AMM target orders
    #[account(mut)]
    pub amm_target_orders: AccountInfo<'info>,

    /// CHECK: Pool coin (wSOL) vault
    #[account(mut)]
    pub pool_coin_token_account: AccountInfo<'info>,

    /// CHECK: Pool pc (token) vault
    #[account(mut)]
    pub pool_pc_token_account: AccountInfo<'info>,

    /// CHECK: Serum market
    pub serum_market: AccountInfo<'info>,

    /// CHECK: Serum bids
    #[account(mut)]
    pub serum_bids: AccountInfo<'info>,

    /// CHECK: Serum asks
    #[account(mut)]
    pub serum_asks: AccountInfo<'info>,

    /// CHECK: Serum event queue
    #[account(mut)]
    pub serum_event_queue: AccountInfo<'info>,

    /// CHECK: Serum coin vault
    #[account(mut)]
    pub serum_coin_vault: AccountInfo<'info>,

    /// CHECK: Serum pc vault
    #[account(mut)]
    pub serum_pc_vault: AccountInfo<'info>,

    /// CHECK: Serum vault signer
    pub serum_vault_signer: AccountInfo<'info>,

    /* ========================= */
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
    require!(side == SIDE_BUY, SolscopeError::Unauthorized);
    require!(amount_in > 0, SolscopeError::InvalidAmount);
    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );
    require!(!ctx.accounts.bot_meta.paused, SolscopeError::BotPaused);

    /* =========================
     * PDA signer seeds
     * ========================= */
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        ctx.accounts.owner.key.as_ref(),
        &bot_id_hash,
        &[ctx.bumps.vault],
    ];
    let signer_seeds = &[vault_seeds];

    /* =========================
     * Create + fund wSOL account
     * ========================= */
    let rent = Rent::get()?;
    let lamports = amount_in
        .checked_add(rent.minimum_balance(TokenAccount::LEN))
        .unwrap();

    let create_wsol_ix = system_instruction::create_account(
        ctx.accounts.vault.key,
        ctx.accounts.vault_wsol.key,
        lamports,
        TokenAccount::LEN as u64,
        &token::ID,
    );

    invoke_signed(
        &create_wsol_ix,
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.vault_wsol.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    token::sync_native(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        SyncNative {
            account: ctx.accounts.vault_wsol.to_account_info(),
        },
    ))?;

    /* =========================
     * Raydium swap CPI
     * ========================= */
    let ix = Instruction {
        program_id: ctx.accounts.amm_program.key(),
        accounts: vec![
            AccountMeta::new(*ctx.accounts.amm.key, false),
            AccountMeta::new_readonly(*ctx.accounts.amm_authority.key, false),
            AccountMeta::new(*ctx.accounts.amm_open_orders.key, false),
            AccountMeta::new(*ctx.accounts.amm_target_orders.key, false),
            AccountMeta::new(*ctx.accounts.pool_coin_token_account.key, false),
            AccountMeta::new(*ctx.accounts.pool_pc_token_account.key, false),
            AccountMeta::new(*ctx.accounts.vault_wsol.key, false),
            AccountMeta::new(ctx.accounts.vault_ata.key(), false),
            AccountMeta::new_readonly(*ctx.accounts.serum_market.key, false),
            AccountMeta::new(*ctx.accounts.serum_bids.key, false),
            AccountMeta::new(*ctx.accounts.serum_asks.key, false),
            AccountMeta::new(*ctx.accounts.serum_event_queue.key, false),
            AccountMeta::new(*ctx.accounts.serum_coin_vault.key, false),
            AccountMeta::new(*ctx.accounts.serum_pc_vault.key, false),
            AccountMeta::new_readonly(*ctx.accounts.serum_vault_signer.key, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        data: raydium_swap_data(amount_in, min_out),
    };

    invoke_signed(&ix, &ctx.accounts.to_account_infos(), signer_seeds)?;

    /* =========================
     * Close wSOL (refund rent)
     * ========================= */
    token::close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault_wsol.to_account_info(),
            destination: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        },
        signer_seeds,
    ))?;

    Ok(())
}

/* ======================================================
 * Raydium instruction data helper
 * ====================================================== */
fn raydium_swap_data(amount_in: u64, min_out: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 8 + 8);
    data.push(9); // Raydium v4 SwapBaseIn
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&min_out.to_le_bytes());
    data
}
