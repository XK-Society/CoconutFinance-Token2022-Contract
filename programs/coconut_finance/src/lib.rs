use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, transfer_checked};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_2022_extensions::{
    transfer_fee,
    interest_bearing_mint,
};

declare_id!("E26SowuKYen9ePnVirUyxq73hKaomHhwdPiRdVCKcu6d");

#[program]
pub mod hotel_coconut {
    use super::*;

    pub fn initialize_hotel(
        ctx: Context<Initialize>,
        transfer_fee_basis_points: u16,
        interest_rate: i16,
    ) -> Result<()> {
        let hotel = &mut ctx.accounts.hotel;
        hotel.authority = ctx.accounts.authority.key();
        hotel.total_supply = 0;
        hotel.usdc_vault = ctx.accounts.usdc_vault.key();

        // Initialize transfer fee for the hotel token
        transfer_fee::transfer_fee_initialize(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                transfer_fee::TransferFeeInitialize {
                    token_program_id: ctx.accounts.token_program.to_account_info(),
                    mint: ctx.accounts.hotel_token_mint.to_account_info(),
                },
            ),
            Some(&hotel.key()),
            Some(&hotel.key()),
            transfer_fee_basis_points,
            0, // Maximum fee
        )?;

        // Initialize interest rate for the hotel token
        interest_bearing_mint::interest_bearing_mint_initialize(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                interest_bearing_mint::InterestBearingMintInitialize {
                    token_program_id: ctx.accounts.token_program.to_account_info(),
                    mint: ctx.accounts.hotel_token_mint.to_account_info(),
                },
            ),
            Some(hotel.key()),
            interest_rate,
        )?;

        Ok(())
    }

    pub fn invest(ctx: Context<Invest>, usdc_amount: u64) -> Result<()> {
        // Transfer USDC from investor to vault
        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.investor_usdc_account.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.usdc_vault.to_account_info(),
                    authority: ctx.accounts.investor.to_account_info(),
                },
            ),
            usdc_amount,
            6, // USDC decimals
        )?;

        // Mint hotel tokens to investor
        let hotel_tokens_to_mint = usdc_amount; // 1:1 ratio for simplicity
        anchor_spl::token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::MintTo {
                    mint: ctx.accounts.hotel_token_mint.to_account_info(),
                    to: ctx.accounts.investor_hotel_token_account.to_account_info(),
                    authority: ctx.accounts.hotel.to_account_info(),
                },
                &[&[b"hotel", &[ctx.bumps.hotel]]],
            ),
            hotel_tokens_to_mint,
        )?;

        ctx.accounts.hotel.total_supply += hotel_tokens_to_mint;

        emit!(InvestmentEvent {
            investor: ctx.accounts.investor.key(),
            usdc_amount,
            hotel_tokens: hotel_tokens_to_mint,
        });

        Ok(())
    }

    pub fn book_room(ctx: Context<BookRoom>, booking_price: u64) -> Result<()> {
        // Transfer USDC from tourist to vault
        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.tourist_usdc_account.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.usdc_vault.to_account_info(),
                    authority: ctx.accounts.tourist.to_account_info(),
                },
            ),
            booking_price,
            6, // USDC decimals
        )?;

        emit!(BookingEvent {
            tourist: ctx.accounts.tourist.key(),
            price: booking_price,
        });

        Ok(())
    }

    pub fn distribute_profits(ctx: Context<DistributeProfits>) -> Result<()> {
        let total_profit = ctx.accounts.usdc_vault.amount;
        require!(total_profit > 0, HotelError::NoProfitToDistribute);

        let profit_per_token = total_profit / ctx.accounts.hotel.total_supply;
        let user_token_balance = ctx.accounts.investor_hotel_token_account.amount;
        let user_profit = profit_per_token * user_token_balance;

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.usdc_vault.to_account_info(),
                    mint: ctx.accounts.usdc_mint.to_account_info(),
                    to: ctx.accounts.investor_usdc_account.to_account_info(),
                    authority: ctx.accounts.hotel.to_account_info(),
                },
                &[&[b"hotel", &[ctx.bumps.hotel]]],
            ),
            user_profit,
            6, // USDC decimals
        )?;

        emit!(ProfitDistributionEvent {
            investor: ctx.accounts.investor.key(),
            amount: user_profit,
        });

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(transfer_fee_basis_points: u16, interest_rate: i16)]
pub struct Initialize<'info> {
    #[account(init, payer = authority, space = 8 + 32 + 8 + 32, seeds = [b"hotel"], bump)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        mint::decimals = 9,
        mint::authority = hotel,
    )]
    pub hotel_token_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = authority,
        associated_token::mint = usdc_mint,
        associated_token::authority = hotel,
    )]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Invest<'info> {
    #[account(mut, seeds = [b"hotel"], bump)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub investor: Signer<'info>,
    #[account(mut)]
    pub investor_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub investor_hotel_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub hotel_token_mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BookRoom<'info> {
    #[account(mut, seeds = [b"hotel"], bump)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub tourist: Signer<'info>,
    #[account(mut)]
    pub tourist_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct DistributeProfits<'info> {
    #[account(mut, seeds = [b"hotel"], bump)]
    pub hotel: Account<'info, Hotel>,
    #[account(mut)]
    pub investor: Signer<'info>,
    #[account(mut)]
    pub investor_hotel_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub investor_usdc_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub usdc_vault: InterfaceAccount<'info, TokenAccount>,
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct Hotel {
    pub authority: Pubkey,
    pub total_supply: u64,
    pub usdc_vault: Pubkey,
}

#[error_code]
pub enum HotelError {
    #[msg("No profit to distribute")]
    NoProfitToDistribute,
}

#[event]
pub struct InvestmentEvent {
    pub investor: Pubkey,
    pub usdc_amount: u64,
    pub hotel_tokens: u64,
}

#[event]
pub struct BookingEvent {
    pub tourist: Pubkey,
    pub price: u64,
}

#[event]
pub struct ProfitDistributionEvent {
    pub investor: Pubkey,
    pub amount: u64,
}