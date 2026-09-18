# Kennel lotto audit (round 0)

**Launch decision: FAIL** for a production-ready trustless lottery.

The kennel is an honest, disclosed entertainment draw. It is not VRF, not explorer-verified, and not immutable. Do not upgrade the live program while round 0 is funded.

Snapshot: 2026-09-15. Live confirmation: Solana mainnet RPC + https://petrock.fun/api/lotto/independent?round=0 + https://petrock.fun/lotto.

## 1. Architecture and trust model

On-chain program `jrock_lotto` owns a config PDA and per-round PDAs. Buyers pay 0.05 SOL per slip. 1% goes to the kennel fee wallet; 99% sits on the round account. After `end_ts`, anyone may `close_sales` (locks `entropy_slot = clock.slot + lag`), `settle` (SlotHashes → sha256 → modulo), `claim` (85% to the derived winner), and `open_round` (moves leftover seed).

Players must trust SlotHashes plus closer timing, and that upgrade authority does not replace the ELF. They do not need to trust the website for payout math. Claim is on-chain and permissionless.

## 2. Findings

| Sev | ID | Finding | Status |
| --- | --- | --- | --- |
| Critical | C1 | Randomness is SlotHashes after close, not a VRF. The closer chooses close time, hence the future slot. | Live, disclosed |
| Critical | C2 | Upgrade authority is a single wallet and can replace code mid-round. | Live, disclosed |
| Critical | C3 | Deployed binary is not explorer-verified. | Program repo published (`SpareCashFinance/jrock-lotto`, tag `lotto-v1-mainnet`). Explorer badge still not filed. |
| Critical | C4 | If SlotHashes expires before settle, the round can stick. No refund instruction. | Live |
| High | H1 | `u64 % n` has a tiny bias unless `n` divides 2^64. | Live, disclosed |
| High | H2 | No Anchor integration, local-validator, devnet lifecycle, fuzz, or solana-verify CI. | Program repo builds SBF on push. `solana-verify` is still a manual `verify-from-repo` against tag `lotto-v1-mainnet`. |
| Medium | M1 | Home DexTape still says “Contract pending” because `$JROCK` mint is unpublished. Not the lotto program. | Expected |
| Fixed | F1 | UI treated raw PDA balance as the prize pool (rent included). | Shipped |
| Fixed | F2 | Header copied empty mint (“Contract pending”) instead of the lotto program id. | Shipped |
| Fixed | F3 | Countdown hydration #418. | Shipped |
| Fixed | F4 | `SlotHeadline` crashed (`RangeError: Invalid array length`) on non-numeric strings. | Shipped |
| Fixed | F5 | Slip receipt dialog was covered by `.glass-panel { position: relative }`. | Shipped |
| Fixed | F6 | First paint described wallet-mode fairness, then flipped to program SlotHashes. | Shipped |
| Fixed | F7 | `openRoundIx` did not mark the previous round writable (seed transfer needs `mut`). | Shipped (client) |

## 3. Exact randomness

After close:

1. `entropy_slot = clock.slot + lag_slots` (config = 150).
2. `settle` requires `clock.slot > entropy_slot` and reads that SlotHashes entry only.
3. `digest = sha256(slot_hash || round_id_le || ticket_count_le)`
4. `winner_index = u64be(digest[0..8]) % ticket_count` in `0..n-1`
5. Winner wallet is the buyer whose `[from_index, from_index + tickets)` contains that index.

This is **not** ORAO or Switchboard VRF. It cannot be requested, bound, or retried as a unique VRF job.

## 4. Manipulability

| Threat | Result |
| --- | --- |
| Closer chooses settlement timing | **Yes** — choosing when to close chooses the entropy slot. |
| Selecting among multiple future slots | **Partial** — lag is fixed; close time is not. |
| Reroll after an unfavorable result | **No** for the same close — entropy slot is stored. Upgrade authority can still replace code. |
| SlotHashes expiration | **Yes** — settle can fail; round can stick. |
| Operator delaying settlement | **Yes** — anyone can crank, but nobody can recover a dropped slot. |
| Modulo bias | **Yes**, tiny. |
| Admin passing a winner | **No** — claim derives the wallet from stored index. |
| RPC lying to the website | **Yes** for display; **no** for on-chain settle if the cranker uses a honest node. |

## 5. Accounting (live round 0)

Independently confirmed:

| Bucket | Lamports |
| --- | ---: |
| accountBalanceLamports | 459,754,480 |
| rentExemptReserveLamports | 14,254,480 |
| ticketGrossLamports | 450,000,000 |
| kennelFeeLamports | 4,500,000 |
| priorRoundSeedLamports | 0 |
| donationsOrUnexpectedDepositsLamports | 0 |
| distributablePotLamports | 445,500,000 |
| winnerPayoutLamports | 378,675,000 |
| nextRoundSeedLamports | 66,825,000 |
| otherReservedLiabilitiesLamports | 0 |

Conservation: `459,754,480 = 14,254,480 + 445,500,000` and `445,500,000 = 378,675,000 + 66,825,000` and `445,500,000 = 445,500,000 ticket net`. Remainder after the 85% floor stays in the seed.

## 6. Implemented fixes (files)

- `src/components/site/LottoDesk.tsx` — prize pool, rent copy, 20 slips, disclosed steps, independent check
- `src/components/site/Header.tsx` — Lotto program copy button
- `src/components/site/LottoVerify.tsx`, `src/app/lotto/verify/page.tsx`, `src/app/api/lotto/independent/route.ts`
- `src/lib/lotto-ledger.ts`, `src/lib/lotto-verify.ts`, `scripts/verify-lotto.mjs`
- `docs/lotto-v1-snapshot.md`, `docs/lotto-fairness-v2.md`, `docs/lotto-verifiable-build.md`
- `anchor/programs/jrock_lotto_v2` — successor ELF, undeployed
- `src/lib/lotto-client.ts` — mount-safe countdown
- `src/components/motion/SlotHeadline.tsx` — numeric cores only
- `src/lib/lotto-program.ts` — previous round writable on open
- `public/.well-known/security.txt`, `docs/lotto-fairness.md`

## 7–9. Tests

Ran: `npm test` (ledger conservation). CLI `npm run verify:lotto`. Production independent API `conserved: true`.

Not run to completion: `cargo test` (no cached `anchor-lang` in this environment), Anchor local-validator, devnet multi-round, fuzz, browser wallet buy (would spend real SOL).

## 10. Mainnet migration plan

1. Leave round 0 on the live ELF. Finish it under disclosed SlotHashes rules, or add a refund instruction in a **new** program after this round is settled.
2. Do not `solana program deploy` over `FvQfc…` while the round PDA holds player SOL.
3. Next program: ORAO VRF Classic `VRFzZoJdhFWL8rkvu87LpKM3RbcVezpMEc6X5GVDr7y`, rejection sampling, refund-on-timeout, then solana-verify, then either revoke upgrade authority or a disclosed timelocked multisig.
4. Devnet first, funded multi-round, then a new mainnet program id.

## 11–13. Verification and addresses

- Program: `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC`
- Program-data: `BZ2RWk6QZFspNKCHiESE7uYzfeCXMjpSvBJYgas26oWi`
- Upgrade authority: `62C41rN2uUrsZoRkZyTqxD8GJYpa6KtERAtehfNmiXwq`
- Config: `p9XrAnsutdWKWnBxiCb57fPuuoH2mvdq32nBHKhwh7K`
- Round 0: `3Q5u97fxVbwxPBcqg34CgqtfPdQ1RTyvnwQHPrg7CUe1`
- Verifiable-build / explorer badge: **not filed**
- Program source: https://github.com/SpareCashFinance/jrock-lotto (`lotto-v1-mainnet`)
- `solana-verify` job: **not submitted** (would not make the current randomness a VRF)

## 14. Public verifier

- https://petrock.fun/lotto/verify
- `npm run verify:lotto`
- https://petrock.fun/.well-known/security.txt

Production “Check this draw” on 2026-09-15: `Solana shows round 0 open, 9 slips, prize pool 0.4455 SOL. No settled winner to recompute yet.`

## 15. Remaining limitations

SlotHashes interim on the **live** program; upgradeable single wallet; unverified ELF; settle can stick; no VRF bind on v1; no refund on v1; `$JROCK` mint still unpublished so market “Contract pending” remains.

v2 source (`jrock_lotto_v2` / `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg`) is in-tree: ORAO bind, rejection sampling, timeout refund, explicit state machine. It is **not** the live program. Do not flip `NEXT_PUBLIC_LOTTO_PROGRAM` until v2 is initialized on mainnet after round 0 is claimed or refunded. See `docs/lotto-fairness-v2.md` and `docs/lotto-verifiable-build.md`.

## 16. Launch decision

**FAIL.** Do not describe this as production-ready, trustless, or cryptographically fair. The desk may stay up as a disclosed kennel with the copy now on the page.
