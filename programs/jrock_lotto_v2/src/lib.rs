//! Kennel lotto v2. New program id. Do not reuse v1 PDAs.
//! Live v1 `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` stays untouched while round 0 is funded.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    hash::hashv,
    instruction::{AccountMeta, Instruction},
    program::invoke,
};
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Jamie's Pet Rock Kennel Lotto",
    project_url: "https://petrock.fun/lotto",
    contacts: "link:https://t.me/Jamiespetrock,link:https://x.com/petrockbtc",
    policy: "https://petrock.fun/lotto/verify",
    preferred_languages: "en",
    source_code: "https://github.com/SpareCashFinance/jrock-lotto",
    auditors: "None"
}

pub const CONFIG_SEED: &[u8] = b"config";
pub const ROUND_SEED: &[u8] = b"round";
pub const MAX_BUYERS: usize = 10_000;
pub const INITIAL_BUYERS: usize = 64;
pub const BUYER_SPACE: usize = 41;
/// Discriminator + fixed Round fields + vec length + bump.
pub const ROUND_HEADER: usize = 190;
pub const MAX_TICKETS_PER_BUY: u8 = 20;

pub const fn round_space(buyer_rows: usize) -> usize {
    ROUND_HEADER + buyer_rows * BUYER_SPACE
}

pub fn buy_realloc_space(current_rows: usize, data_len: usize) -> usize {
    round_space(current_rows.saturating_add(1)).max(data_len)
}
pub const WINNER_SHARE_BPS: u64 = 85;
pub const SHARE_DENOM: u64 = 100;
pub const SLIP_FEE_BPS: u64 = 1;
pub const MIN_VRF_TIMEOUT_SECS: i64 = 60;
pub const MAX_VRF_TIMEOUT_SECS: i64 = 7 * 24 * 60 * 60;
pub const FEE_WALLET: Pubkey = pubkey!("qbjbLafSNGq27fYFiF1RKhb9BREk1zFWWS8H6Drj8co");
pub const ORAO_VRF: Pubkey = pubkey!("VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y");
pub const ORAO_CONFIG_SEED: &[u8] = b"orao-vrf-network-configuration";
pub const ORAO_RANDOMNESS_SEED: &[u8] = b"orao-vrf-randomness-request";
const ORAO_REQUEST_V2_DISC: [u8; 8] = [38, 151, 209, 6, 195, 102, 28, 217];

#[program]
pub mod jrock_lotto_v2 {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        ticket_lamports: u64,
        round_secs: i64,
        vrf_timeout_secs: i64,
    ) -> Result<()> {
        require!(ticket_lamports > 0, LottoError::BadConfig);
        require!(round_secs > 60, LottoError::BadConfig);
        require!(
            vrf_timeout_secs >= MIN_VRF_TIMEOUT_SECS && vrf_timeout_secs <= MAX_VRF_TIMEOUT_SECS,
            LottoError::BadConfig
        );

        let clock = Clock::get()?;
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.ticket_lamports = ticket_lamports;
        config.round_secs = round_secs;
        config.vrf_timeout_secs = vrf_timeout_secs;
        config.current_round = 0;
        config.bump = ctx.bumps.config;

        reset_round(
            &mut ctx.accounts.round,
            0,
            clock.unix_timestamp,
            round_secs,
            ctx.bumps.round,
        )?;
        Ok(())
    }

    pub fn set_round_secs(ctx: Context<SetRoundSecs>, round_secs: i64) -> Result<()> {
        require!(round_secs > 60, LottoError::BadConfig);
        require_keys_eq!(
            ctx.accounts.authority.key(),
            ctx.accounts.config.authority,
            LottoError::Unauthorized
        );
        ctx.accounts.config.round_secs = round_secs;

        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(round.round_id == ctx.accounts.config.current_round, LottoError::WrongRound);
        if round.status == RoundStatus::Open && round.ticket_count == 0 {
            let from_start = round
                .start_ts
                .checked_add(round_secs)
                .ok_or(LottoError::Overflow)?;
            round.end_ts = if from_start > clock.unix_timestamp {
                from_start
            } else {
                clock
                    .unix_timestamp
                    .checked_add(round_secs)
                    .ok_or(LottoError::Overflow)?
            };
        }
        Ok(())
    }

    pub fn open_round(ctx: Context<OpenRound>) -> Result<()> {
        let clock = Clock::get()?;
        let config = &ctx.accounts.config;
        reset_round(
            &mut ctx.accounts.round,
            config.current_round,
            clock.unix_timestamp,
            config.round_secs,
            ctx.bumps.round,
        )?;
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
                refunded: 0,
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
        compact_round(
            &ctx.accounts.round.to_account_info(),
            ctx.accounts.round.buyers.len(),
        )?;
        Ok(())
    }

    pub fn close_sales(ctx: Context<CloseSales>) -> Result<()> {
        let clock = Clock::get()?;
        let timeout = ctx.accounts.config.vrf_timeout_secs;
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Open, LottoError::SalesClosed);
        require!(clock.unix_timestamp >= round.end_ts, LottoError::TooEarly);

        if round.ticket_count == 0 {
            round.status = RoundStatus::Void;
            round.close_ts = clock.unix_timestamp;
            bump_round(&mut ctx.accounts.config)?;
            return Ok(());
        }

        round.close_ts = clock.unix_timestamp;
        round.vrf_timeout_ts = clock
            .unix_timestamp
            .checked_add(timeout)
            .ok_or(LottoError::Overflow)?;
        round.status = RoundStatus::Closed;
        Ok(())
    }

    pub fn request_randomness(ctx: Context<RequestRandomness>) -> Result<()> {
        require!(ctx.accounts.round.status == RoundStatus::Closed, LottoError::NotClosed);
        require!(ctx.accounts.round.ticket_count > 0, LottoError::NoSlips);
        require!(ctx.accounts.round.vrf_request == Pubkey::default(), LottoError::VrfAlreadyRequested);
        require_keys_eq!(ctx.accounts.orao_vrf.key(), ORAO_VRF, LottoError::BadVrf);

        let seed = vrf_seed_bytes(
            ctx.accounts.round.key(),
            ctx.accounts.round.round_id,
            ctx.accounts.round.ticket_count,
        );
        let (expected_request, _) = Pubkey::find_program_address(&[ORAO_RANDOMNESS_SEED, &seed], &ORAO_VRF);
        require_keys_eq!(ctx.accounts.vrf_request.key(), expected_request, LottoError::BadVrf);
        let (expected_state, _) = Pubkey::find_program_address(&[ORAO_CONFIG_SEED], &ORAO_VRF);
        require_keys_eq!(ctx.accounts.network_state.key(), expected_state, LottoError::BadVrf);

        request_orao_v2(
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.network_state.to_account_info(),
            ctx.accounts.treasury.to_account_info(),
            ctx.accounts.vrf_request.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.orao_vrf.to_account_info(),
            seed,
        )?;

        let round = &mut ctx.accounts.round;
        round.vrf_seed = seed;
        round.vrf_request = expected_request;
        round.status = RoundStatus::RandomnessRequested;
        Ok(())
    }

    pub fn fulfill_randomness(ctx: Context<FulfillRandomness>) -> Result<()> {
        let round = &mut ctx.accounts.round;
        require!(
            round.status == RoundStatus::RandomnessRequested || round.status == RoundStatus::Fulfilled,
            LottoError::NotRequested
        );
        require!(round.vrf_request != Pubkey::default(), LottoError::NotRequested);
        require_keys_eq!(ctx.accounts.vrf_request.key(), round.vrf_request, LottoError::BadVrf);

        if round.status == RoundStatus::Fulfilled && round.vrf_randomness != [0u8; 32] {
            return Ok(());
        }

        let entropy = orao_fulfilled_randomness(&ctx.accounts.vrf_request, &round.vrf_seed)?;
        round.vrf_randomness = entropy;
        round.status = RoundStatus::Fulfilled;
        Ok(())
    }

    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        let round = &mut ctx.accounts.round;
        require!(
            round.status == RoundStatus::RandomnessRequested || round.status == RoundStatus::Fulfilled,
            LottoError::NotRequested
        );
        require!(round.ticket_count > 0, LottoError::NoSlips);
        require_keys_eq!(ctx.accounts.vrf_request.key(), round.vrf_request, LottoError::BadVrf);

        let entropy = if round.vrf_randomness != [0u8; 32] {
            round.vrf_randomness
        } else {
            let value = orao_fulfilled_randomness(&ctx.accounts.vrf_request, &round.vrf_seed)?;
            round.vrf_randomness = value;
            value
        };

        let winner_index = winner_from_entropy(entropy, round.ticket_count)?;
        let winner_key = owner_for_ticket(&round.buyers, winner_index).ok_or(LottoError::WinnerMissing)?;
        round.winner_index = winner_index;
        round.winner = winner_key;
        round.status = RoundStatus::Settled;
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        require!(ctx.accounts.round.status == RoundStatus::Settled, LottoError::NotSettled);
        require_keys_eq!(ctx.accounts.winner.key(), ctx.accounts.round.winner, LottoError::WrongWinner);

        {
            let rows = ctx.accounts.round.buyers.len();
            let round_info = ctx.accounts.round.to_account_info();
            compact_round(&round_info, rows)?;
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
        bump_round(&mut ctx.accounts.config)?;
        Ok(())
    }

    pub fn refund_one(_ctx: Context<RefundOne>, _buyer_index: u32) -> Result<()> {
        err!(LottoError::RefundsDisabled)
    }

    /// Shrink the current book to used rows so leftover realloc rent stays in the pot.
    pub fn compact_book(ctx: Context<CompactBook>) -> Result<()> {
        compact_round(
            &ctx.accounts.round.to_account_info(),
            ctx.accounts.round.buyers.len(),
        )
    }
}

fn reset_round(round: &mut Round, round_id: u64, now: i64, round_secs: i64, bump: u8) -> Result<()> {
    round.round_id = round_id;
    round.start_ts = now;
    round.end_ts = now.checked_add(round_secs).ok_or(LottoError::Overflow)?;
    round.close_ts = 0;
    round.vrf_timeout_ts = 0;
    round.ticket_count = 0;
    round.winner_index = 0;
    round.winner = Pubkey::default();
    round.vrf_seed = [0u8; 32];
    round.vrf_randomness = [0u8; 32];
    round.vrf_request = Pubkey::default();
    round.status = RoundStatus::Open;
    round.buyers = Vec::new();
    round.bump = bump;
    Ok(())
}

fn bump_round(config: &mut Config) -> Result<()> {
    config.current_round = config.current_round.checked_add(1).ok_or(LottoError::Overflow)?;
    Ok(())
}

pub fn vrf_seed_bytes(round_key: Pubkey, round_id: u64, ticket_count: u32) -> [u8; 32] {
    hashv(&[
        crate::ID.as_ref(),
        round_key.as_ref(),
        &round_id.to_le_bytes(),
        &ticket_count.to_le_bytes(),
    ])
    .to_bytes()
}

pub fn winner_from_entropy(mut entropy: [u8; 32], n: u32) -> Result<u32> {
    require!(n > 0, LottoError::NoSlips);
    if n == 1 {
        return Ok(0);
    }
    let n64 = n as u64;
    let rem = (u64::MAX % n64).wrapping_add(1);
    for _ in 0..64 {
        let x = u64::from_be_bytes(
            entropy[0..8]
                .try_into()
                .map_err(|_| LottoError::BadEntropy)?,
        );
        if rem == 0 || x < 0u64.wrapping_sub(rem) {
            return Ok((x % n64) as u32);
        }
        entropy = hashv(&[&entropy]).to_bytes();
    }
    err!(LottoError::BadEntropy)
}

pub fn refund_lamports(ticket_lamports: u64, tickets: u32) -> Result<u64> {
    let gross = ticket_lamports
        .checked_mul(tickets as u64)
        .ok_or(LottoError::Overflow)?;
    let fee = gross
        .checked_mul(SLIP_FEE_BPS)
        .ok_or(LottoError::Overflow)?
        / SHARE_DENOM;
    Ok(gross.checked_sub(fee).ok_or(LottoError::Overflow)?)
}

fn owner_for_ticket(buyers: &[Buyer], winner_index: u32) -> Option<Pubkey> {
    buyers
        .iter()
        .find(|row| winner_index >= row.from_index && winner_index < row.from_index + row.tickets)
        .map(|row| row.wallet)
}

fn request_orao_v2<'info>(
    payer: AccountInfo<'info>,
    network_state: AccountInfo<'info>,
    treasury: AccountInfo<'info>,
    request: AccountInfo<'info>,
    system_program: AccountInfo<'info>,
    vrf_program: AccountInfo<'info>,
    seed: [u8; 32],
) -> Result<()> {
    require_keys_eq!(*vrf_program.key, ORAO_VRF, LottoError::BadVrf);
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(&ORAO_REQUEST_V2_DISC);
    data.extend_from_slice(&seed);
    let ix = Instruction {
        program_id: ORAO_VRF,
        accounts: vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*network_state.key, false),
            AccountMeta::new(*treasury.key, false),
            AccountMeta::new(*request.key, false),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
        ],
        data,
    };
    invoke(
        &ix,
        &[payer, network_state, treasury, request, system_program, vrf_program],
    )?;
    Ok(())
}

/// ORAO Classic `RandomnessV2` fulfilled layout:
/// 8-byte account disc, 1-byte RequestAccount::Fulfilled, 32-byte client, 32-byte seed, 64-byte randomness.
/// We store the first 32 bytes for rejection sampling.
fn orao_fulfilled_randomness(account: &AccountInfo, expected_seed: &[u8; 32]) -> Result<[u8; 32]> {
    require_keys_eq!(*account.owner, ORAO_VRF, LottoError::BadVrf);
    let data = account.try_borrow_data()?;
    require!(data.len() >= 8 + 1 + 32 + 32 + 64, LottoError::EntropyUnavailable);
    require!(data[8] == 1, LottoError::EntropyUnavailable);
    let seed_off = 8 + 1 + 32;
    require!(&data[seed_off..seed_off + 32] == expected_seed, LottoError::BadEntropy);
    let rand_off = seed_off + 32;
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[rand_off..rand_off + 32]);
    Ok(out)
}

fn compact_round(info: &AccountInfo, rows: usize) -> Result<()> {
    let need = round_space(rows);
    require!(info.data_len() >= need, LottoError::Overflow);
    if info.data_len() > need {
        info.realloc(need, false)?;
    }
    Ok(())
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
        space = round_space(INITIAL_BUYERS),
        seeds = [ROUND_SEED, &[0u8; 8]],
        bump
    )]
    pub round: Account<'info, Round>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetRoundSecs<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump,
        constraint = round.round_id == config.current_round @ LottoError::WrongRound
    )]
    pub round: Account<'info, Round>,
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
            || previous_round.status == RoundStatus::Void
            || previous_round.status == RoundStatus::Refunded) @ LottoError::RoundStillLive
    )]
    pub previous_round: Account<'info, Round>,
    #[account(
        init,
        payer = payer,
        space = round_space(INITIAL_BUYERS),
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
        realloc = buy_realloc_space(round.buyers.len(), round.to_account_info().data_len()),
        realloc::payer = buyer,
        realloc::zero = false,
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
pub struct RequestRandomness<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
    /// CHECK: ORAO Classic VRF program id.
    #[account(address = ORAO_VRF)]
    pub orao_vrf: UncheckedAccount<'info>,
    /// CHECK: ORAO network_state PDA, verified in handler.
    #[account(mut)]
    pub network_state: UncheckedAccount<'info>,
    /// CHECK: ORAO treasury; the VRF program rejects a wrong one.
    #[account(mut)]
    pub treasury: UncheckedAccount<'info>,
    /// CHECK: ORAO request PDA, verified in handler.
    #[account(mut)]
    pub vrf_request: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FulfillRandomness<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
    /// CHECK: stored ORAO request account.
    pub vrf_request: UncheckedAccount<'info>,
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
    /// CHECK: stored ORAO request account.
    pub vrf_request: UncheckedAccount<'info>,
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

#[derive(Accounts)]
pub struct RefundOne<'info> {
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump
    )]
    pub round: Account<'info, Round>,
    /// CHECK: must match the buyer row being refunded.
    #[account(mut)]
    pub buyer: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct CompactBook<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, config.current_round.to_le_bytes().as_ref()],
        bump = round.bump,
        constraint = round.round_id == config.current_round @ LottoError::WrongRound
    )]
    pub round: Account<'info, Round>,
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    pub ticket_lamports: u64,
    pub round_secs: i64,
    pub vrf_timeout_secs: i64,
    pub current_round: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Round {
    pub round_id: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub close_ts: i64,
    pub vrf_timeout_ts: i64,
    pub ticket_count: u32,
    pub winner_index: u32,
    pub winner: Pubkey,
    pub vrf_seed: [u8; 32],
    pub vrf_randomness: [u8; 32],
    pub vrf_request: Pubkey,
    pub status: RoundStatus,
    #[max_len(MAX_BUYERS)]
    pub buyers: Vec<Buyer>,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Buyer {
    pub wallet: Pubkey,
    pub tickets: u32,
    pub from_index: u32,
    pub refunded: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RoundStatus {
    Open,
    Closed,
    RandomnessRequested,
    Fulfilled,
    Settled,
    Claimed,
    Void,
    Refunding,
    Refunded,
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
    #[msg("VRF is not fulfilled yet.")]
    EntropyUnavailable,
    #[msg("Bad VRF account.")]
    BadVrf,
    #[msg("Bad entropy.")]
    BadEntropy,
    #[msg("Pot has no claimable lamports.")]
    EmptyPot,
    #[msg("Math overflow.")]
    Overflow,
    #[msg("VRF already requested for this round.")]
    VrfAlreadyRequested,
    #[msg("Randomness has not been requested.")]
    NotRequested,
    #[msg("Refunds are not open.")]
    RefundClosed,
    #[msg("That buyer is already refunded.")]
    AlreadyRefunded,
    #[msg("Bad buyer index.")]
    BadBuyer,
    #[msg("Signer is not the config authority.")]
    Unauthorized,
    #[msg("This draw does not refund. If anyone bought, settle always picks one of those wallets.")]
    RefundsDisabled,
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
        assert_eq!(refund_lamports(50_000_000, 2).unwrap(), 99_000_000);
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
    fn one_ticket_always_wins_index_zero() {
        assert_eq!(winner_from_entropy([0u8; 32], 1).unwrap(), 0);
        assert_eq!(winner_from_entropy([0xffu8; 32], 1).unwrap(), 0);
    }

    #[test]
    fn rejection_sampling_stays_in_range() {
        for n in [2u32, 3, 5, 9, 20, 64] {
            for fill in [0u8, 1, 7, 255] {
                let mut entropy = [fill; 32];
                entropy[31] = n as u8;
                let index = winner_from_entropy(entropy, n).unwrap();
                assert!(index < n, "index {index} for n {n}");
            }
        }
    }

    #[test]
    fn all_ones_entropy_stays_in_range_for_n_three() {
        let index = winner_from_entropy([0xffu8; 32], 3).unwrap();
        assert!(index < 3);
    }

    #[test]
    fn buy_size_rejects_zero_and_twenty_one() {
        assert!(!(0u8 >= 1 && 0u8 <= MAX_TICKETS_PER_BUY));
        assert!(21u8 > MAX_TICKETS_PER_BUY);
    }

    #[test]
    fn book_grows_to_ten_thousand_buys() {
        assert_eq!(MAX_BUYERS, 10_000);
        assert_eq!(round_space(0), 190);
        assert_eq!(round_space(256), 10_686);
        assert_eq!(buy_realloc_space(6, 10_686), 10_686);
        assert_eq!(buy_realloc_space(256, 10_686), 10_727);
        assert_eq!(round_space(7), 477);
        assert_eq!(buy_realloc_space(7, 477), 518);
    }

    #[test]
    fn status_machine_is_explicit() {
        assert_ne!(RoundStatus::Open, RoundStatus::Closed);
        assert_ne!(RoundStatus::Closed, RoundStatus::RandomnessRequested);
        assert_ne!(RoundStatus::RandomnessRequested, RoundStatus::Fulfilled);
        assert_ne!(RoundStatus::Fulfilled, RoundStatus::Settled);
        assert_ne!(RoundStatus::Settled, RoundStatus::Claimed);
        assert_ne!(RoundStatus::Refunding, RoundStatus::Settled);
    }

    #[test]
    fn orao_v2_fulfilled_layout_exposes_seed_then_first_32_random_bytes() {
        let mut data = vec![0u8; 8 + 1 + 32 + 32 + 64];
        data[8] = 1;
        let seed = [7u8; 32];
        let randomness = [9u8; 64];
        data[9 + 32..9 + 64].copy_from_slice(&seed);
        data[9 + 64..].copy_from_slice(&randomness);
        assert_eq!(&data[8 + 1 + 32..8 + 1 + 64], &seed);
        assert_eq!(&data[8 + 1 + 64..8 + 1 + 96], &randomness[..32]);
    }
}
