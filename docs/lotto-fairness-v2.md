# Kennel lotto v2 fairness (jrock-lotto-v4)

Successor program `jrock_lotto_v2` id `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg`.

This is **not live on production** until `NEXT_PUBLIC_LOTTO_PROGRAM` is flipped after round 0 on v1 is claimed or refunded. Live desk copy stays on `docs/lotto-fairness.md` (SlotHashes v1).

Do not reuse v1 PDAs. Product rules stay: 0.05 SOL, 1–20 slips, 1% kennel fee inside the price, 85/15, tickets `0..n-1`, permissionless crank.

## State machine

`Open → Closed → RandomnessRequested → Fulfilled → Settled → Claimed`

Plus `Void` (close with zero tickets) and `Refunding` / `Refunded` (VRF timeout).

## Randomness

After `close_sales` with tickets:

1. Anyone calls `request_randomness` once.
2. Seed = `sha256(program_id || round_pda || round_id_le || ticket_count_le)`.
3. The program CPIs ORAO Classic `request_v2` (`VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y`) and stores the request PDA.
4. A second request is rejected.
5. `fulfill_randomness` / `settle` read the fulfilled 64-byte ORAO output, store the first 32 bytes, and never overwrite them.
6. `winner_index` uses rejection sampling on that 256-bit value into `0..n-1` (not raw `% n`).
7. Winner wallet is the buyer whose `[from_index, from_index + tickets)` contains that index. Never an instruction argument.

## Timeout refund

If VRF has not been consumed into a winner by `close_ts + vrf_timeout_secs`, anyone may `refund_one` for each unrefunded buyer. Payout is `tickets * price * 99/100` (the 1% kennel fee stays paid). After refunds start, settle is rejected. When every buyer is refunded the round is `Refunded` and `open_round` may roll leftover lamports.

## Independent check

Same URLs as v1. After the env switch, `/lotto/verify` and `npm run verify:lotto` recompute from the stored VRF bytes.

## Authority

Keep upgradeable through devnet and the first mainnet week. Then either revoke upgrade authority or move it to a disclosed Squads timelock. Do not revoke on day one. Do not revoke v1 while round 0 is funded.
