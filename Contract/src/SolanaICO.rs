use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};

/// Replace with actual program ID after `anchor keys list`
declare_id!("11111111111111111111111111111111");

#[error_code]
pub enum ErrorCode {
    /// @notice Triggered when an arithmetic operation (addition, subtraction, multiplication, or division) 
    /// results in a value that exceeds the capacity of the data type (e.g., u64 overflow).
    /// @dev Essential for preventing "wrap-around" vulnerabilities in token balance and price calculations.
    #[msg("Arithmetic Overflow.")]
    Overflow,

    /// @notice Triggered when an instruction requiring administrative privileges is invoked by an unauthorized account.
    /// @dev Enforces access control by validating the signer's public key against the stored authority in the state account.
    #[msg("Invalid Admin.")]
    InvalidAdmin,
}

#[program]
pub mod ico {
    use super::*;

    // Constant: Target SPL Token Mint address for the ICO
    pub const ICO_MINT_ADDRESS: &str = "k9NVtVp5r8Nn7xG94t5UjRKoqcbJ9GWBn76eEFXUbae";
    // Price Discovery: 0.001 SOL per token (1,000,000 Lamports)
    pub const LAMPORTS_PER_TOKEN: u64 = 1_000_000;
    // Precision: Standard SPL decimals (9)
    pub const TOKEN_DECIMALS: u64 = 1_000_000_000;

    // Initializes the Program Derived Address (PDA) Token Account to hold ICO inventory.
    pub fn create_ico_ata(ctx: Context<CreateIcoATA>, ico_amount: u64) -> Result<()> {
        msg!("Creating program ATA to hold ICO tokens.");

        // Convert amount to token decimals
        let raw_amount = ico_amount.checked_mul(TOKEN_DECIMALS).ok_or(ErrorCode::Overflow);

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.ico_ata_for_admin.to_account_info(),
                to: ctx.accounts.ico_ata_for_ico_program.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
            }
        );

        token::transfer(cpi_ctx, raw_amount)?;
        msg!("Transferred {} ICO tokens to program ATA.", ico_amount);

        let data = &mut ctx.accounts.data;
        data.admin = *ctx.accounts.admin.key;
        data.total_tokens = ico_amount;
        data.tokens_sold = 0;
        msg!("Initialized ICO data.");
        Ok(())
    }

    // Transfers tokens from the authority's provider account to the ICO vault.
    pub fn deposit_ico_in_ata(ctx: Context<DepositIcoATA>, ico_amount: u64) -> Result<()> {
        if ctx.accounts.data.admin != *ctx.accounts.admin.key{
            return Err(error!(ErrorCode::InvalidAdmin));
        }

        // Convert amount to token decimals
        let raw_amount = ico_amount.checked_mul(TOKEN_DECIMALS).ok_or(ErrorCode::Overflow);

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.ico_ata_for_admin.to_account_info(),
                to: ctx.accounts.ico_ata_for_ico_program.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
            }
        );

        token::transfer(cpi_ctx, raw_amount)?;

        let data = &mut ctx.accounts.data;
        data.total_tokens += ico_amount;

        msg!("Deposit {} additional ICO tokens.", ico_amount);
        Ok(())
    }

    // Executes the swap: Transfers SOL from user to treasury and mints/transfers tokens to user.
    pub fn buy_tokens(ctx: Context<BuyTokens>, _ico_ata_for_ico_program_bump: u8,
    token_amount: u64) -> Result<()> {
        // Convert token amount to include decimals for SPL transfer
        let raw_token_amount = token_amount.checked_mul(TOKEN_DECIMALS).ok_or(ErrorCode::Overflow);

        // Calculate SOL const (0.001 Sol per token)
        let sol_amount = token_amount.checked_mul(LAMPORTS_PER_TOKEN).ok_or(ErrorCode::Overflow);  

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.admin.key(),
            sol_amount,
        );

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
        )?;

        msg!("Transferred {} lamports to admin.", sol_amount);

        // Transfer Tokens to User
        let ico_mint_address = ctx.accounts.ico_mint.key();
        let seeds = &[ico_mint_address.as_ref(), &[_ico_ata_for_ico_program_bump]];
        let signer = &[&seeds[..]];


        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.ico_ata_for_ico_program.to_account_info(),
                to: ctx.accounts.ico_ata_for_user.to_account_info(),
                authority: ctx.accounts.ico_ata_for_ico_program.to_account_info(),
            },
            &signer,
        );

        token::transfer(cpi_ctx, raw_token_amount)?;

        // Update Data
        let data = &mut ctx.accounts.data;
        data.tokens_sold = data.tokens_sold.checked_add(token_amount).ok_or(ErrorCode::Overflow)?;

        msg!("Transferred {} token to buyer.", token_amount);
        Ok(())
    }

    #[derive(Accounts)]
    pub struct CreateIcoATA<'info>{
        #[account(
            init,
            payer = admin,
            seeds = [ ICO_MINT_ADDRESS.parse::<Pubkey>().unwrap().as_ref() ],
            bump,
            token::mint = ico_mint,
            token::authority = ico_ata_for_ico_program,
        )]

        pub ico_ata_for_ico_program: Account<'info, TokenAccount>,

        #[account(init, payer=admin, space=900, seeds=[b"data", admin.key().as_ref()], bump)]

        pub data: Account<'info, Data>,
        
        #[account(
            address = ICO_MINT_ADDRESS.parse::<Pubkey>().unwrap(),
        )]
        pub ico_mint: Account<'info, Mint>,

        #[account(mut)]
        pub ico_ata_for_admin: Account<'info, TokenAccount>,

        #[account(mut)]
        pub admin: Signer<'info>,

        pub system_program: Program<'info, System>,
        pub token_program: Program<'info, Token>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct DepositIcoATA<'info>{
        #[account(mut)]
        pub ico_ata_for_ico_program: Account<'info, TokenAccount>,

        #[account(mut)]
        pub data: Account<'info, Data>,

        #[account(
            address = ICO_MINT_ADDRESS.parse::<Pubkey>().unwrap(),
        )]

        pub ico_mint: Account<'info, Mint>,

        #[account(mut)]
        pub ico_ata_for_admin: Account<'info, TokenAccount>,

        #[account(mut)]
        pub admin: Signer<'info>,
        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    #[instruction(_ico_ata_for_ico_program_bump: u8)]
    pub struct BuyTokens<'info>{
        #[account(
            mut,
            seeds = [ ico_mint.key().as_ref() ],
            bump = _ico_ata_for_ico_program_bump,
        )]

        pub ico_ata_for_ico_program: Account<'info, TokenAccount>,

        #[account(mut)]
        pub data: Account<'info, Data>,

        #[account(
            address = ICO_MINT_ADDRESS.parse::<Pubkey>().unwrap(),
        )]
        pub ico_mint: Account<'info, Mint>,

        #[account(mut)]
        pub ico_ata_for_user: Account<'info, TokenAccount>,

        #[account(mut)]
        pub user: Signer<'info>,

        // Check
        #[account(mut)]
        pub admin: AccountInfo<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    #[account]
    pub struct Data{
        pub admin: Pubkey,
        pub total_tokens: u64,
        pub tokens_sold: u64,
    }
}