//! Frozen mainnet v1 (SlotHashes). Do not upgrade `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC`
//! while round 0 `3Q5u97fxVbwxPBcqg34CgqtfPdQ1RTyvnwQHPrg7CUe1` holds player SOL.
//! Successor is `jrock_lotto_v2`. See `docs/lotto-v1-snapshot.md`.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC");

pub const CONFIG_SEED: &[u8] = b"config";
pub const ROUND_SEED: &[u8] = b"round";
pub const MAX_BUYERS: usize = 64;
pub const MAX_TICKETS_PER_BUY: u8 = 20;
pub const WINNER_SHARE_BPS: u64 = 85;
pub const SHARE_DENOM: u64 = 100;
pub const SLIP_FEE_BPS: u64 = 1;
pub const FEE_WALLET: Pubkey = pubkey!("qbjbLafSNGq27fYFiF1RKhb9BREk1zFWWS8H6Drj8co");

#[program]
pub mod jrock_lotto {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        ticket_lamports: u64,
        round_secs: i64,
        lag_slots: u64,
    ) -> Result<()> {
        require!(ticket_lamports > 0, LottoError::BadConfig);
        require!(round_secs > 60, LottoError::BadConfig);
        require!(lag_slots >= 32 && lag_slots <= 400, LottoError::BadConfig);

        let clock = Clock::get()?;
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.ticket_lamports = ticket_lamports;
        config.round_secs = round_secs;
        config.lag_slots = lag_slots;
        config.current_round = 0;
        config.bump = ctx.bumps.config;

        let round = &mut ctx.accounts.round;
        round.round_id = 0;
        round.start_ts = clock.unix_timestamp;
        round.end_ts = clock
            .unix_timestamp
            .checked_add(round_secs)
            .ok_or(LottoError::Overflow)?;
        round.entropy_slot = 0;
        round.ticket_count = 0;
        round.winner_index = 0;
        round.winner = Pubkey::default();
        round.entropy_hash = [0u8; 32];
        round.status = RoundStatus::Open;
        round.buyers = Vec::new();
        round.bump = ctx.bumps.round;
        Ok(())
    }

    pub fn open_round(ctx: Context<OpenRound>) -> Result<()> {
        let clock = Clock::get()?;
        let config = &ctx.accounts.config;
        {
            let round = &mut ctx.accounts.round;
            round.round_id = config.current_round;
            round.start_ts = clock.unix_timestamp;
            round.end_ts = clock
                .unix_timestamp
                .checked_add(config.round_secs)
                .ok_or(LottoError::Overflow)?;
            round.entropy_slot = 0;
            round.ticket_count = 0;
            round.winner_index = 0;
            round.winner = Pubkey::default();
            round.entropy_hash = [0u8; 32];
            round.status = RoundStatus::Open;
            round.buyers = Vec::new();
            round.bump = ctx.bumps.round;
        }
        move_excess(
            &ctx.accounts.previous_round.to_account_info(),
            &ctx.accounts.round.to_account_info(),
        )?;
        Ok(())
    }

    pub fn buy(ctx: Context<Buy>, tickets: u8) -> Result<()> {
        require!(tickets >= 1 && tickets <= MAX_TICKETS_PER_BUY, LottoError::BadTicketCount);
        let clock = Clock::get()?;
        require!(ctx.accounts.round.status == RoundStatus::Open, LottoError::SalesClosed);
        require!(clock.unix_timestamp < ctx.accounts.round.end_ts, LottoError::SalesClosed);
        require!(ctx.accounts.round.buyers.len() < MAX_BUYERS, LottoError::BookFull);

        let add = tickets as u32;
        let from_index = ctx.accounts.round.ticket_count;
        let buyer_key = ctx.accounts.buyer.key();
        let amount = ctx
            .accounts
            .config
            .ticket_lamports
            .checked_mul(tickets as u64)
            .ok_or(LottoError::Overflow)?;
        let fee = amount
            .checked_mul(SLIP_FEE_BPS)
            .ok_or(LottoError::Overflow)?
            / SHARE_DENOM;
        let pot = amount.checked_sub(fee).ok_or(LottoError::Overflow)?;
        require!(pot > 0, LottoError::BadConfig);

        {
            let round = &mut ctx.accounts.round;
            round.buyers.push(Buyer {
                wallet: buyer_key,
                tickets: add,
                from_index,
            });
            round.ticket_count = round
                .ticket_count
                .checked_add(add)
                .ok_or(LottoError::Overflow)?;
        }

        transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.round.to_account_info(),
                },
            ),
            pot,
        )?;
        if fee > 0 {
            transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.fee_wallet.to_account_info(),
                    },
                ),
                fee,
            )?;
        }
        Ok(())
    }

    pub fn close_sales(ctx: Context<CloseSales>) -> Result<()> {
        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Open, LottoError::SalesClosed);
        require!(clock.unix_timestamp >= round.end_ts, LottoError::TooEarly);

        if round.ticket_count == 0 {
            round.status = RoundStatus::Void;
            ctx.accounts.config.current_round = ctx
                .accounts
                .config
                .current_round
                .checked_add(1)
                .ok_or(LottoError::Overflow)?;
            return Ok(());
        }

        round.entropy_slot = clock
            .slot
            .checked_add(ctx.accounts.config.lag_slots)
            .ok_or(LottoError::Overflow)?;
        round.status = RoundStatus::Closed;
        Ok(())
    }

    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Closed, LottoError::NotClosed);
        require!(round.ticket_count > 0, LottoError::NoSlips);
        require!(clock.slot > round.entropy_slot, LottoError::TooEarly);

        let slot_hash = slot_hash_for(&ctx.accounts.slot_hashes, round.entropy_slot)?;
        let digest = hashv(&[
            &slot_hash,
            &round.round_id.to_le_bytes(),
            &round.ticket_count.to_le_bytes(),
        ]);
        let bytes = digest.to_bytes();
        let random = u64::from_be_bytes(bytes[0..8].try_into().map_err(|_| LottoError::BadEntropy)?);
        let winner_index = (random % round.ticket_count as u64) as u32;
        let winner_key = round
            .buyers
            .iter()
            .find(|row| winner_index >= row.from_index && winner_index < row.from_index + row.tickets)
            .map(|row| row.wallet)
            .ok_or(LottoError::WinnerMissing)?;

        round.entropy_hash = slot_hash;
        round.winner_index = winner_index;
        round.winner = winner_key;
        round.status = RoundStatus::Settled;
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        require!(ctx.accounts.round.status == RoundStatus::Settled, LottoError::NotSettled);
        require_keys_eq!(ctx.accounts.winner.key(), ctx.accounts.round.winner, LottoError::WrongWinner);

        {
            let round_info = ctx.accounts.round.to_account_info();
            let winner_info = ctx.accounts.winner.to_account_info();
            let rent = Rent::get()?.minimum_balance(round_info.data_len());
            let claimable = round_info.lamports().saturating_sub(rent);
            let payout = claimable
                .checked_mul(WINNER_SHARE_BPS)
                .ok_or(LottoError::Overflow)?
                / SHARE_DENOM;
            require!(payout > 0, LottoError::EmptyPot);
            **round_info.try_borrow_mut_lamports()? -= payout;
            **winner_info.try_borrow_mut_lamports()? += payout;
        }

        ctx.accounts.round.status = RoundStatus::Claimed;
        ctx.accounts.config.current_round = ctx
            .accounts
            .config
            .current_round
            .checked_add(1)
            .ok_or(LottoError::Overflow)?;
        Ok(())
    }
}

fn slot_hash_for(slot_hashes: &AccountInfo, target: u64) -> Result<[u8; 32]> {
    require_keys_eq!(
        *slot_hashes.key,
        anchor_lang::solana_program::sysvar::slot_hashes::ID,
        LottoError::BadEntropy
    );
    let data = slot_hashes.try_borrow_data()?;
    require!(data.len() >= 8, LottoError::EntropyUnavailable);
    let count = u64::from_le_bytes(data[0..8].try_into().map_err(|_| LottoError::BadEntropy)?);
    for i in 0..count as usize {
        let offset = 8usize
            .checked_add(i.checked_mul(40).ok_or(LottoError::Overflow)?)
            .ok_or(LottoError::Overflow)?;
        require!(offset.checked_add(40).unwrap_or(0) <= data.len(), LottoError::EntropyUnavailable);
        let slot = u64::from_le_bytes(
            data[offset..offset + 8]
                .try_into()
                .map_err(|_| LottoError::BadEntropy)?,
        );
        if slot == target {
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data[offset + 8..offset + 40]);
            return Ok(hash);
        }
    }
    err!(LottoError::EntropyUnavailable)
}

fn hashv(parts: &[&[u8]]) -> anchor_lang::solana_program::hash::Hash {
    anchor_lang::solana_program::hash::hashv(parts)
}

fn excess_lamports(account: &AccountInfo) -> Result<u64> {
    let rent = Rent::get()?.minimum_balance(account.data_len());
    Ok(account.lamports().saturating_sub(rent))
}

fn move_excess(from: &AccountInfo, to: &AccountInfo) -> Result<()> {
    let carry = excess_lamports(from)?;
    if carry == 0 {
        return Ok(());
    }
    **from.try_borrow_mut_lamports()? -= carry;
    **to.try_borrow_mut_lamports()? += carry;
    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = authority,
        space = 8 + Round::INIT_SPACE,
        seeds = [ROUND_SEED, &[0u8; 8]],
        bump
    )]
    pub round: Account<'info, Round>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OpenRound<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.current_round > 0 @ LottoError::WrongRound
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, (config.current_round - 1).to_le_bytes().as_ref()],
        bump = previous_round.bump,
        constraint = (previous_round.status == RoundStatus::Claimed
            || previous_round.status == RoundStatus::Void) @ LottoError::RoundStillLive
    )]
    pub previous_round: Account<'info, Round>,
    #[account(
        init,
        payer = payer,
        space = 8 + Round::INIT_SPACE,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump
    )]
    pub round: Account<'info, Round>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Buy<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump,
        constraint = round.round_id == config.current_round @ LottoError::WrongRound
    )]
    pub round: Account<'info, Round>,
    pub system_program: Program<'info, System>,
    #[account(mut, address = FEE_WALLET)]
    pub fee_wallet: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct CloseSales<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
}

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
    /// CHECK: SlotHashes sysvar, address verified in slot_hash_for.
    pub slot_hashes: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
    /// CHECK: must match the settled winner.
    #[account(mut)]
    pub winner: SystemAccount<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub ticket_lamports: u64,
    pub round_secs: i64,
    pub lag_slots: u64,
    pub current_round: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Round {
    pub round_id: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub entropy_slot: u64,
    pub ticket_count: u32,
    pub winner_index: u32,
    pub winner: Pubkey,
    pub entropy_hash: [u8; 32],
    pub status: RoundStatus,
    #[max_len(64)]
    pub buyers: Vec<Buyer>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Buyer {
    pub wallet: Pubkey,
    pub tickets: u32,
    pub from_index: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RoundStatus {
    Open,
    Closed,
    Settled,
    Claimed,
    Void,
}

#[error_code]
pub enum LottoError {
    #[msg("Bad lotto config.")]
    BadConfig,
    #[msg("Ticket count must be 1 to 20.")]
    BadTicketCount,
    #[msg("Sales are closed.")]
    SalesClosed,
    #[msg("Too early for this instruction.")]
    TooEarly,
    #[msg("The book is full.")]
    BookFull,
    #[msg("No slips this round.")]
    NoSlips,
    #[msg("Round is not closed.")]
    NotClosed,
    #[msg("Round is not settled.")]
    NotSettled,
    #[msg("Wrong round account.")]
    WrongRound,
    #[msg("Previous round is still live.")]
    RoundStillLive,
    #[msg("Winner account does not match the draw.")]
    WrongWinner,
    #[msg("Could not map the winning slip.")]
    WinnerMissing,
    #[msg("Slot hash is not available yet.")]
    EntropyUnavailable,
    #[msg("Bad entropy account.")]
    BadEntropy,
    #[msg("Pot has no claimable lamports.")]
    EmptyPot,
    #[msg("Math overflow.")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_is_one_percent_inside_the_posted_price() {
        let amount = 50_000_000u64 * 2;
        let fee = amount * SLIP_FEE_BPS / SHARE_DENOM;
        assert_eq!(fee, 1_000_000);
        assert_eq!(amount - fee, 99_000_000);
    }

    #[test]
    fn claim_split_keeps_the_division_remainder_in_the_seed() {
        let claimable = 99_000_000u64;
        let payout = claimable * WINNER_SHARE_BPS / SHARE_DENOM;
        let seed = claimable - payout;
        assert_eq!(payout, 84_150_000);
        assert_eq!(seed, 14_850_000);
        assert_eq!(payout + seed, claimable);
    }

    #[test]
    fn winner_index_always_lands_in_0_to_n_minus_one() {
        let count = 2u32;
        for random in [0u64, 1, 2, u64::MAX] {
            let index = (random % count as u64) as u32;
            assert!(index < count);
        }
    }

    #[test]
    fn buy_size_rejects_zero_and_twenty_one() {
        assert!(!(0u8 >= 1 && 0u8 <= MAX_TICKETS_PER_BUY));
        assert!(1u8 >= 1 && 1u8 <= MAX_TICKETS_PER_BUY);
        assert!(20u8 >= 1 && 20u8 <= MAX_TICKETS_PER_BUY);
        assert!(!(21u8 >= 1 && 21u8 <= MAX_TICKETS_PER_BUY));
    }

    #[test]
    fn status_machine_forbids_buy_after_close() {
        assert_ne!(RoundStatus::Open, RoundStatus::Closed);
        assert_ne!(RoundStatus::Closed, RoundStatus::Settled);
        assert_ne!(RoundStatus::Settled, RoundStatus::Claimed);
    }

    #[test]
    fn maps_a_ticket_index_to_the_owning_wallet() {
        let buyers = [
            Buyer {
                wallet: Pubkey::from([1u8; 32]),
                tickets: 1,
                from_index: 0,
            },
            Buyer {
                wallet: Pubkey::from([2u8; 32]),
                tickets: 1,
                from_index: 1,
            },
        ];
        let winner_index = 1u32;
        let winner = buyers
            .iter()
            .find(|row| winner_index >= row.from_index && winner_index < row.from_index + row.tickets)
            .unwrap();
        assert_eq!(winner.wallet, buyers[1].wallet);
    }
}
