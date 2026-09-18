# Kennel lotto v2 fairness (jrock-lotto-v4)

Successor program `jrock_lotto_v2` id `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg`.

This is **live on production** at `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg`. v1 SlotHashes remains at `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` as history.

Do not reuse v1 PDAs. Product rules stay: 0.05 SOL, 1–20 slips, 1% kennel fee inside the price, 85/15, tickets `0..n-1`, permissionless crank.

## State machine

`Open → Closed → RandomnessRequested → Fulfilled → Settled → Claimed`

Plus `Void` (close with zero tickets). `Refunding` / `Refunded` stay in the account layout so existing PDAs decode, but `refund_one` now returns `RefundsDisabled`. If anyone bought, settle always picks one of those wallets. The book holds 10,000 buy rows and grows with each buy.

## Randomness

After `close_sales` with tickets:

1. Anyone calls `request_randomness` once.
2. Seed = `sha256(program_id || round_pda || round_id_le || ticket_count_le)`.
3. The program CPIs ORAO Classic `request_v2` (`VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y`) and stores the request PDA.
4. A second request is rejected.
5. `fulfill_randomness` / `settle` read the fulfilled 64-byte ORAO output, store the first 32 bytes, and never overwrite them.
6. `winner_index` uses rejection sampling on that 256-bit value into `0..n-1` (not raw `% n`).
7. Winner wallet is the buyer whose `[from_index, from_index + tickets)` contains that index. Never an instruction argument.

## No timeout refund

`refund_one` is kept so the instruction index and account layout stay compatible. It always errors. If ORAO is late, wait and crank `fulfill_randomness` / `settle`. Do not tell buyers they can get a refund.

The book holds 10,000 buy rows (`MAX_BUYERS`). New rounds start at 64 rows and grow by one row per buy so rent is paid as people file, not all at once.

## Independent check

Same URLs as v1. After the env switch, `/lotto/verify` and `npm run verify:lotto` recompute from the stored VRF bytes.

## Authority

Keep upgradeable through devnet and the first mainnet week. Then either revoke upgrade authority or move it to a disclosed Squads timelock. Do not revoke on day one. Do not revoke v1 while round 0 is funded.
