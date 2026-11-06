use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

declare_id!("HSyiRChzuxHGmg29Ezsvxyxj6tcR7wLrUySNn9NAJ1gy");

#[program]
pub mod escrow {
    use super::*;

    pub fn make(
        ctx: Context<Make>,
        offer_id: u64,
        offered_amount: u64,
        wanted_amount: u64,
        designated_taker: Option<Pubkey>,
        expires_at: i64,
    ) -> Result<()> {
        require!(
            offered_amount > 0 && wanted_amount > 0,
            EscrowError::InvalidAmount
        );
        require!(
            ctx.accounts.offered_mint.key() != ctx.accounts.wanted_mint.key(),
            EscrowError::SameMint
        );
        if expires_at > 0 {
            require!(
                expires_at > Clock::get()?.unix_timestamp,
                EscrowError::InvalidExpiry
            );
        }

        let offer = &mut ctx.accounts.offer;
        offer.maker = ctx.accounts.maker.key();
        offer.offered_mint = ctx.accounts.offered_mint.key();
        offer.wanted_mint = ctx.accounts.wanted_mint.key();
        offer.offer_id = offer_id;
        offer.offered_amount = offered_amount;
        offer.wanted_amount = wanted_amount;
        offer.designated_taker = designated_taker;
        offer.expires_at = expires_at;
        offer.bump = ctx.bumps.offer;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.maker_offered_ata.to_account_info(),
                    mint: ctx.accounts.offered_mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.maker.to_account_info(),
                },
            ),
            offered_amount,
            ctx.accounts.offered_mint.decimals,
        )?;
        Ok(())
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        let offer = &ctx.accounts.offer;
        if offer.expires_at > 0 {
            require!(
                Clock::get()?.unix_timestamp <= offer.expires_at,
                EscrowError::OfferExpired
            );
        }
        if let Some(expected) = offer.designated_taker {
            require_keys_eq!(ctx.accounts.taker.key(), expected, EscrowError::WrongTaker);
        }

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.taker_wanted_ata.to_account_info(),
                    mint: ctx.accounts.wanted_mint.to_account_info(),
                    to: ctx.accounts.maker_wanted_ata.to_account_info(),
                    authority: ctx.accounts.taker.to_account_info(),
                },
            ),
            offer.wanted_amount,
            ctx.accounts.wanted_mint.decimals,
        )?;

        let offer_id = offer.offer_id.to_le_bytes();
        let bump = [offer.bump];
        let signer_seeds: &[&[u8]] = &[b"offer", offer.maker.as_ref(), &offer_id, &bump];
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.offered_mint.to_account_info(),
                    to: ctx.accounts.taker_offered_ata.to_account_info(),
                    authority: ctx.accounts.offer.to_account_info(),
                },
                &[signer_seeds],
            ),
            offer.offered_amount,
            ctx.accounts.offered_mint.decimals,
        )?;
        close_vault(
            &ctx.accounts.token_program,
            &ctx.accounts.vault,
            &ctx.accounts.maker.to_account_info(),
            &ctx.accounts.offer,
            signer_seeds,
        )
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        refund_tokens(
            &ctx.accounts.token_program,
            &ctx.accounts.offered_mint,
            &ctx.accounts.vault,
            &ctx.accounts.maker_offered_ata,
            &ctx.accounts.maker.to_account_info(),
            &ctx.accounts.offer,
        )
    }

    pub fn update(
        ctx: Context<Update>,
        wanted_amount: u64,
        designated_taker: Option<Pubkey>,
        expires_at: i64,
    ) -> Result<()> {
        require!(wanted_amount > 0, EscrowError::InvalidAmount);
        if expires_at > 0 {
            require!(
                expires_at > Clock::get()?.unix_timestamp,
                EscrowError::InvalidExpiry
            );
        }
        let offer = &mut ctx.accounts.offer;
        offer.wanted_amount = wanted_amount;
        offer.designated_taker = designated_taker;
        offer.expires_at = expires_at;
        Ok(())
    }

    pub fn expire_refund(ctx: Context<ExpireRefund>) -> Result<()> {
        require!(ctx.accounts.offer.expires_at > 0, EscrowError::NoExpiry);
        require!(
            Clock::get()?.unix_timestamp > ctx.accounts.offer.expires_at,
            EscrowError::NotExpired
        );
        refund_tokens(
            &ctx.accounts.token_program,
            &ctx.accounts.offered_mint,
            &ctx.accounts.vault,
            &ctx.accounts.maker_offered_ata,
            &ctx.accounts.maker.to_account_info(),
            &ctx.accounts.offer,
        )
    }
}

fn close_vault<'info>(
    token_program: &Interface<'info, TokenInterface>,
    vault: &InterfaceAccount<'info, TokenAccount>,
    maker: &AccountInfo<'info>,
    offer: &Account<'info, Offer>,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    close_account(CpiContext::new_with_signer(
        token_program.to_account_info(),
        CloseAccount {
            account: vault.to_account_info(),
            destination: maker.to_account_info(),
            authority: offer.to_account_info(),
        },
        &[signer_seeds],
    ))
}

fn refund_tokens<'info>(
    token_program: &Interface<'info, TokenInterface>,
    offered_mint: &InterfaceAccount<'info, Mint>,
    vault: &InterfaceAccount<'info, TokenAccount>,
    maker_offered_ata: &InterfaceAccount<'info, TokenAccount>,
    maker: &AccountInfo<'info>,
    offer: &Account<'info, Offer>,
) -> Result<()> {
    let offer_id = offer.offer_id.to_le_bytes();
    let bump = [offer.bump];
    let signer_seeds: &[&[u8]] = &[b"offer", offer.maker.as_ref(), &offer_id, &bump];
    transfer_checked(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            TransferChecked {
                from: vault.to_account_info(),
                mint: offered_mint.to_account_info(),
                to: maker_offered_ata.to_account_info(),
                authority: offer.to_account_info(),
            },
            &[signer_seeds],
        ),
        offer.offered_amount,
        offered_mint.decimals,
    )?;
    close_vault(token_program, vault, maker, offer, signer_seeds)
}

#[derive(Accounts)]
#[instruction(offer_id: u64)]
pub struct Make<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    pub offered_mint: InterfaceAccount<'info, Mint>,
    pub wanted_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = maker,
        space = 8 + Offer::INIT_SPACE,
        seeds = [b"offer", maker.key().as_ref(), offer_id.to_le_bytes().as_ref()],
        bump
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut, token::mint = offered_mint, token::authority = maker)]
    pub maker_offered_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        associated_token::mint = offered_mint,
        associated_token::authority = offer,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Take<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,
    #[account(mut, address = offer.maker)]
    pub maker: SystemAccount<'info>,
    pub offered_mint: InterfaceAccount<'info, Mint>,
    pub wanted_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        close = maker,
        seeds = [b"offer", offer.maker.as_ref(), offer.offer_id.to_le_bytes().as_ref()],
        bump = offer.bump,
        has_one = offered_mint @ EscrowError::WrongMint,
        has_one = wanted_mint @ EscrowError::WrongMint
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut, associated_token::mint = offered_mint, associated_token::authority = offer)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = wanted_mint, token::authority = taker)]
    pub taker_wanted_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = wanted_mint, token::authority = maker)]
    pub maker_wanted_ata: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = offered_mint, token::authority = taker)]
    pub taker_offered_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut, address = offer.maker @ EscrowError::Unauthorized)]
    pub maker: Signer<'info>,
    pub offered_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        close = maker,
        seeds = [b"offer", offer.maker.as_ref(), offer.offer_id.to_le_bytes().as_ref()],
        bump = offer.bump,
        has_one = offered_mint @ EscrowError::WrongMint
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut, associated_token::mint = offered_mint, associated_token::authority = offer)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = offered_mint, token::authority = maker)]
    pub maker_offered_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub maker: Signer<'info>,
    #[account(
        mut,
        seeds = [b"offer", maker.key().as_ref(), offer.offer_id.to_le_bytes().as_ref()],
        bump = offer.bump,
        has_one = maker @ EscrowError::Unauthorized
    )]
    pub offer: Account<'info, Offer>,
}

#[derive(Accounts)]
pub struct ExpireRefund<'info> {
    pub caller: Signer<'info>,
    #[account(mut, address = offer.maker)]
    pub maker: SystemAccount<'info>,
    pub offered_mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        close = maker,
        seeds = [b"offer", offer.maker.as_ref(), offer.offer_id.to_le_bytes().as_ref()],
        bump = offer.bump,
        has_one = offered_mint @ EscrowError::WrongMint
    )]
    pub offer: Account<'info, Offer>,
    #[account(mut, associated_token::mint = offered_mint, associated_token::authority = offer)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, token::mint = offered_mint, token::authority = maker)]
    pub maker_offered_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
#[derive(InitSpace)]
pub struct Offer {
    pub maker: Pubkey,
    pub offered_mint: Pubkey,
    pub wanted_mint: Pubkey,
    pub offer_id: u64,
    pub offered_amount: u64,
    pub wanted_amount: u64,
    pub designated_taker: Option<Pubkey>,
    pub expires_at: i64,
    pub bump: u8,
}

#[error_code]
pub enum EscrowError {
    #[msg("Amounts must be greater than zero")]
    InvalidAmount,
    #[msg("Offered and wanted mints must differ")]
    SameMint,
    #[msg("The supplied mint does not match the offer")]
    WrongMint,
    #[msg("Only the maker may perform this action")]
    Unauthorized,
    #[msg("This offer is restricted to a different taker")]
    WrongTaker,
    #[msg("The offer has expired")]
    OfferExpired,
    #[msg("Expiry must be in the future")]
    InvalidExpiry,
    #[msg("The offer does not define an expiry")]
    NoExpiry,
    #[msg("The offer has not expired")]
    NotExpired,
}
