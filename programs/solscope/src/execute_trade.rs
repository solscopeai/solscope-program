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
pub const SIDE_SELL: u8 = 1;

/* ======================================================
 * Helpers
 * ====================================================== */

fn raydium_amm_program() -> Pubkey {
    Pubkey::from_str("RVKd61ztZW9KQqkHn7kYk9Z3n5Vf3L7hPwrKyYVJZZz").unwrap()
}

fn raydium_swap_base_in_data(amount_in: u64, min_out: u64) -> Vec<u8> {
    // Raydium v4 SwapBaseIn (commonly 9). You already used this.
    let mut data = Vec::with_capacity(17);
    data.push(9);
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&min_out.to_le_bytes());
    data
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

    /// CHECK: Vault PDA (system-owned SOL vault)
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref(), &bot_id_hash],
        bump
    )]
    pub vault: AccountInfo<'info>,

    /// Output/Input token mint (depends on side)
    pub mint: Account<'info, Mint>,

    /// Vault ATA for this mint.
    /// BUY: destination (receives tokens)
    /// SELL: source (spends tokens)
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = vault
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    /// CHECK: Native SOL mint (special-cased)
    #[account(address = native_mint::id())]
    pub native_mint: UncheckedAccount<'info>,

    /// CHECK: Temporary wSOL token account.
    /// Must be a fresh system-owned account coming in (client provides a new Keypair each time).
    #[account(mut)]
    pub vault_wsol: AccountInfo<'info>,

    /* ========== Raydium Accounts ========== */
    /// CHECK: Raydium AMM program
    #[account(address = raydium_amm_program())]
    pub amm_program: AccountInfo<'info>,

    /// CHECK: AMM state
    #[account(mut)]
    pub amm: AccountInfo<'info>,
    /// CHECK: AMM authority
    pub amm_authority: AccountInfo<'info>,
    /// CHECK: OpenOrders
    #[account(mut)]
    pub amm_open_orders: AccountInfo<'info>,
    /// CHECK: TargetOrders
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
    require!(amount_in > 0, SolscopeError::InvalidAmount);
    require!(min_out > 0, SolscopeError::InvalidAmount);
    require!(
        side == SIDE_BUY || side == SIDE_SELL,
        SolscopeError::Unauthorized
    );

    require!(
        ctx.accounts.bot_meta.bot_id_hash == bot_id_hash,
        SolscopeError::BotIdMismatch
    );
    require!(!ctx.accounts.bot_meta.paused, SolscopeError::BotPaused);

    // client must provide a fresh Keypair for vault_wsol and sign the tx
    require!(
        ctx.accounts.vault_wsol.is_signer,
        SolscopeError::Unauthorized
    );

    // vault_wsol must start uninitialized (system-owned) before we create+init it
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

    /* ================= Pre-swap balance snapshots (extra slippage guard) ================= */
    let before_token = token::accessor::amount(&ctx.accounts.vault_ata.to_account_info())?;
    // vault_wsol isn't initialized yet, so "before" is 0 for wsol.

    /* ================= Create + init temp wSOL token account ================= */
    let rent_min = ctx.accounts.rent.minimum_balance(TokenAccount::LEN);

    // BUY spends SOL from vault into wSOL -> fund with amount_in + rent
    // SELL doesn't spend SOL; wSOL is destination -> fund with rent only
    let lamports = if side == SIDE_BUY {
        rent_min.checked_add(amount_in).unwrap()
    } else {
        rent_min
    };

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

    token::initialize_account(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        InitializeAccount {
            account: ctx.accounts.vault_wsol.to_account_info(),
            mint: ctx.accounts.native_mint.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        },
    ))?;

    // For BUY: needed so the SPL token account reflects deposited lamports as wSOL
    // For SELL: harmless (still fine)
    token::sync_native(CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        SyncNative {
            account: ctx.accounts.vault_wsol.to_account_info(),
        },
    ))?;

    /* ================= Raydium swap =================
     * We keep SwapBaseIn for both directions by swapping user source/dest accounts.
     * BUY: source = wSOL, dest = token
     * SELL: source = token, dest = wSOL
     */
    let (user_source, user_dest) = if side == SIDE_BUY {
        (
            ctx.accounts.vault_wsol.to_account_info(),
            ctx.accounts.vault_ata.to_account_info(),
        )
    } else {
        (
            ctx.accounts.vault_ata.to_account_info(),
            ctx.accounts.vault_wsol.to_account_info(),
        )
    };

    let ix = Instruction {
        program_id: ctx.accounts.amm_program.key(),
        accounts: vec![
            AccountMeta::new(*ctx.accounts.amm.key, false),
            AccountMeta::new_readonly(*ctx.accounts.amm_authority.key, false),
            AccountMeta::new(*ctx.accounts.amm_open_orders.key, false),
            AccountMeta::new(*ctx.accounts.amm_target_orders.key, false),
            AccountMeta::new(*ctx.accounts.pool_coin_token_account.key, false),
            AccountMeta::new(*ctx.accounts.pool_pc_token_account.key, false),
            AccountMeta::new(*user_source.key, false),
            AccountMeta::new(*user_dest.key, false),
            AccountMeta::new_readonly(*ctx.accounts.serum_market.key, false),
            AccountMeta::new(*ctx.accounts.serum_bids.key, false),
            AccountMeta::new(*ctx.accounts.serum_asks.key, false),
            AccountMeta::new(*ctx.accounts.serum_event_queue.key, false),
            AccountMeta::new(*ctx.accounts.serum_coin_vault.key, false),
            AccountMeta::new(*ctx.accounts.serum_pc_vault.key, false),
            AccountMeta::new_readonly(*ctx.accounts.serum_vault_signer.key, false),
            AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
        ],
        data: raydium_swap_base_in_data(amount_in, min_out),
    };

    invoke_signed(&ix, &ctx.accounts.to_account_infos(), signer_seeds)?;

    /* ================= Post-swap delta checks (extra slippage protection) ================= */
    if side == SIDE_BUY {
        let after_token = token::accessor::amount(&ctx.accounts.vault_ata.to_account_info())?;
        let received = after_token.saturating_sub(before_token);
        require!(received >= min_out, SolscopeError::SlippageExceeded);
    } else {
        // SELL: output is wSOL -> check wSOL token amount >= min_out
        let after_wsol = token::accessor::amount(&ctx.accounts.vault_wsol.to_account_info())?;
        require!(after_wsol >= min_out, SolscopeError::SlippageExceeded);
    }

    /* ================= Close wSOL (unwrap) =================
     * BUY: wSOL should be empty; close refunds rent
     * SELL: wSOL contains proceeds; close converts to SOL into vault
     */
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
