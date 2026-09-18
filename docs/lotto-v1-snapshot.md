# Live v1 freeze (round 0)

**Do not upgrade** program `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` while round PDA `3Q5u97fxVbwxPBcqg34CgqtfPdQ1RTyvnwQHPrg7CUe1` holds player SOL.

This file is the Phase 0 snapshot from the kennel lotto fix plan. Round 0 must finish under disclosed SlotHashes rules (or an approved emergency refund-only upgrade if settle is permanently stuck).

## Frozen addresses

| Item | Value |
| --- | --- |
| Network | Solana mainnet-beta |
| Program | `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` |
| Program-data | `BZ2RWk6QZFspNKCHiESE7uYzfeCXMjpSvBJYgas26oWi` |
| Upgrade authority | `62C41rN2uUrsZoRkZyTqxD8GJYpa6KtERAtehfNmiXwq` |
| Config PDA | `p9XrAnsutdWKWnBxiCb57fPuuoH2mvdq32nBHKhwh7K` |
| Round 0 PDA | `3Q5u97fxVbwxPBcqg34CgqtfPdQ1RTyvnwQHPrg7CUe1` |
| Fee wallet | `qbjbLafSNGq27fYFiF1RKhb9BREk1zFWWS8H6Drj8co` |
| Ticket | 50_000_000 lamports |
| Round length | 259200 seconds |
| Lag slots | 150 |
| Window | 2026-09-15T01:55:28.000Z → 2026-09-18T01:55:28.000Z |

## Frozen ledger (9 slips)

| Bucket | Lamports |
| --- | ---: |
| accountBalanceLamports | 459_754_480 |
| rentExemptReserveLamports | 14_254_480 |
| ticketGrossLamports | 450_000_000 |
| kennelFeeLamports | 4_500_000 |
| distributablePotLamports | 445_500_000 |
| winnerPayoutLamports | 378_675_000 |
| nextRoundSeedLamports | 66_825_000 |

Git tag for this ELF's public source: `lotto-v1-mainnet`.

Successor program (not live): `jrock_lotto_v2` id `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg`. Do not point `NEXT_PUBLIC_LOTTO_PROGRAM` at v2 on production until v2 is initialized on mainnet after round 0 is claimed or refunded.
