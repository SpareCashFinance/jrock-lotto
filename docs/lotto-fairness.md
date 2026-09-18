# Kennel lotto fairness (jrock-lotto-v3)

This is the disclosed rule set for the live program `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` on Solana mainnet-beta.

This document is a specification, not an audit certificate. A matching source hash (when filed) only proves the binary came from public source.

## Product rules

- Ticket price: 50_000_000 lamports (0.05 SOL), paid in full.
- Round length: live config is 72 hours (`259200`). Later rocks are meant to be 48 hours (`172800`) via `set_round_secs` after a safe upgrade. The current rock's `end_ts` is already written.
- Buy size: 1, 2, 5, 10, or up to 20 tickets per instruction.
- Kennel fee: 1% of that gross, sent to `qbjbLafSNGq27fYFiF1RKhb9BREk1zFWWS8H6Drj8co`.
- 99% of the gross is transferred into the current round PDA.
- Ticket numbers are `0 .. ticket_count-1`.
- Winner payout: `floor(distributable * 85 / 100)` where distributable is round lamports minus rent-exempt reserve.
- Remainder after that floor, including the intended 15%, stays on the round as the next seed.
- Rent-exempt reserve is never prize money.
- Conservation: `account_balance = rent + distributable` and `distributable = winner_payout + next_seed` and `distributable = ticket_net + prior_seed + unexpected_deposits`.

## Randomness (interim, not a VRF)

After `close_sales`:

1. `entropy_slot = clock.slot + lag_slots` (live config uses 150).
2. `settle` reads that exact SlotHashes entry.
3. `digest = sha256(slot_hash || round_id_le || ticket_count_le)`
4. `winner_index = u64be(digest[0..8]) % ticket_count`

Anyone may crank close, settle, claim, and open. Nobody may pass in a winner wallet except the account that already matches the stored winner.

This is **not** ORAO or Switchboard VRF. Known limits:

- The closer chooses the moment of close, which chooses the future slot.
- SlotHashes only retains recent slots. If settle is late, the round can stick.
- `% ticket_count` has a tiny bias unless ticket_count divides 2^64.
- The program is upgradeable. Upgrade authority today is a single wallet.

Do not describe this draw as fully trustless, cryptographically fair, or verified until a VRF is shipped, the binary is explorer-verified, and upgrade authority is revoked or moved to a disclosed timelocked multisig.

Live freeze: `docs/lotto-v1-snapshot.md`. Successor spec: `docs/lotto-fairness-v2.md`. Verifiable-build notes: `docs/lotto-verifiable-build.md`.

## Independent check

- Site: https://petrock.fun/lotto/verify
- Program source: https://github.com/SpareCashFinance/jrock-lotto (`lotto-v1-mainnet`)
- CLI: `node scripts/verify-lotto.mjs [roundId|roundPda]`
- Audit findings: `docs/lotto-audit.md` (launch decision: FAIL for a trustless production lottery)

Both read public mainnet RPC and recompute from account bytes. They do not trust the lotto tape API.

## Upgrade policy

Do not upgrade this program while a funded round is open unless that round is first settled or refunded on-chain under the rules above. Do not revoke upgrade authority until tests, a VRF (or an honestly labeled successor), and a migration plan exist.
