use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("BctY8rmqHk5f6MJDZYmmZ78hXaqaqXMBHvr3Y52v8zK5");

#[program]
pub mod vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.owner = ctx.accounts.owner.key();
        vault.deposited_lamports = 0;
        vault.bump = ctx.bumps.vault;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);
        let next = ctx
            .accounts
            .vault
            .deposited_lamports
            .checked_add(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;
        transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.owner.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                },
            ),
            amount,
        )?;
        ctx.accounts.vault.deposited_lamports = next;
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        require!(amount > 0, VaultError::InvalidAmount);
        require!(
            amount <= ctx.accounts.vault.deposited_lamports,
            VaultError::InsufficientFunds
        );
        ctx.accounts.vault.deposited_lamports = ctx
            .accounts
            .vault
            .deposited_lamports
            .checked_sub(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;
        let vault_info = ctx.accounts.vault.to_account_info();
        let owner_info = ctx.accounts.owner.to_account_info();
        let next_vault_lamports = vault_info
            .lamports()
            .checked_sub(amount)
            .ok_or(VaultError::InsufficientFunds)?;
        let next_owner_lamports = owner_info
            .lamports()
            .checked_add(amount)
            .ok_or(VaultError::ArithmeticOverflow)?;
        **vault_info.try_borrow_mut_lamports()? = next_vault_lamports;
        **owner_info.try_borrow_mut_lamports()? = next_owner_lamports;
        Ok(())
    }

    pub fn close(ctx: Context<CloseVault>) -> Result<()> {
        require!(
            ctx.accounts.vault.deposited_lamports == 0,
            VaultError::VaultNotEmpty
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        init,
        payer = owner,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", owner.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct CloseVault<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        close = owner,
        seeds = [b"vault", owner.key().as_ref()],
        bump = vault.bump,
        has_one = owner @ VaultError::Unauthorized
    )]
    pub vault: Account<'info, Vault>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub owner: Pubkey,
    pub deposited_lamports: u64,
    pub bump: u8,
}

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Insufficient deposited funds")]
    InsufficientFunds,
    #[msg("Only the vault owner may perform this action")]
    Unauthorized,
    #[msg("Withdraw all deposited funds before closing")]
    VaultNotEmpty,
}
