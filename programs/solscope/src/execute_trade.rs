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
    token::{self, CloseAccount, InitializeAccount, Mint, SyncNative, Token, TokenAccount},
};
use spl_token::native_mint;

use crate::{errors::SolscopeError, state::BotMeta};

pub const SIDE_BUY: u8 = 0;

/* ======================================================
 * Helpers
 * ====================================================== */

fn raydium_amm_program() -> Pubkey {
    Pubkey::from_str("RVKd61ztZW9KQqkHn7kYk9Z3n5Vf3L7hPwrKyYVJZZz").unwrap()
}

#[derive(Accounts)]
#[instruction(bot_id_hash: [u8; 32])]
pub struct ExecuteTrade<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"bot", owner.key().as_ref(), &bot_id_hash],
        bump,
        has_one = owner,
        has_one = vault
    )]
    pub bot_meta: Account<'info, BotMeta>,

    /// CHECK: Vault PDA (SOL only)
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref(), &bot_id_hash],
        bump
    )]
    pub vault: AccountInfo<'info>,

    /// Output token mint
    pub mint: Account<'info, Mint>,

    /// Vault ATA for output token
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    /// CHECK: Native SOL mint (special-cased; not an initialized Mint account)
    #[account(address = native_mint::id())]
    pub native_mint: UncheckedAccount<'info>,

    /// CHECK: Temporary wSOL account (must be system-owned before init)
    #[account(mut)]
    pub vault_wsol: AccountInfo<'info>,

    /* ========== Raydium Accounts ========== */
    #[account(address = raydium_amm_program())]
    pub amm_program: AccountInfo<'info>,

    #[account(mut)]
    pub amm: AccountInfo<'info>,
    pub amm_authority: AccountInfo<'info>,
    #[account(mut)]
    pub amm_open_orders: AccountInfo<'info>,
    #[account(mut)]
    pub amm_target_orders: AccountInfo<'info>,
    #[account(mut)]
    pub pool_coin_token_account: AccountInfo<'info>,
    #[account(mut)]
    pub pool_pc_token_account: AccountInfo<'info>,
    pub serum_market: AccountInfo<'info>,
    #[account(mut)]
    pub serum_bids: AccountInfo<'info>,
    #[account(mut)]
    pub serum_asks: AccountInfo<'info>,
    #[account(mut)]
    pub serum_event_queue: AccountInfo<'info>,
    #[account(mut)]
    pub serum_coin_vault: AccountInfo<'info>,
    #[account(mut)]
    pub serum_pc_vault: AccountInfo<'info>,
    pub serum_vault_signer: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
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

    // vault_wsol must be fresh/uninitialized before we create+init it
    require!(
        ctx.accounts.vault_wsol.owner == &System::id(),
        SolscopeError::InvalidVault
    );

    /* ================= PDA signer ================= */
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        ctx.accounts.owner.key.as_ref(),
        &bot_id_hash,
        &[ctx.bumps.vault],
    ];
    let signer_seeds = &[vault_seeds];

    /* ================= Create wSOL account ================= */
    let lamports = amount_in
        .checked_add(ctx.accounts.rent.minimum_balance(TokenAccount::LEN))
        .unwrap();

    let create_ix = system_instruction::create_account(
        ctx.accounts.vault.key,
        ctx.accounts.vault_wsol.key,
        lamports,
        TokenAccount::LEN as u64,
        &token::ID,
    );

    invoke_signed(
        &create_ix,
        &[
            ctx.accounts.vault.to_account_info(),
            ctx.accounts.vault_wsol.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    /* ================= Initialize wSOL token account ================= */
    token::initialize_account(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        InitializeAccount {
            account: ctx.accounts.vault_wsol.to_account_info(),
            mint: ctx.accounts.native_mint.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        },
    ))?;

    /* ================= Sync native ================= */
    token::sync_native(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        SyncNative {
            account: ctx.accounts.vault_wsol.to_account_info(),
        },
    ))?;

    /* ================= Raydium swap ================= */
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
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
        ],
        data: raydium_swap_data(amount_in, min_out),
    };

    invoke_signed(&ix, &ctx.accounts.to_account_infos(), signer_seeds)?;

    /* ================= Close wSOL ================= */
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

fn raydium_swap_data(amount_in: u64, min_out: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(17);
    data.push(9); // SwapBaseIn
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&min_out.to_le_bytes());
    data
}
