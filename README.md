# jrock-lotto

On-chain kennel lotto for `$JROCK` / [petrock.fun](https://petrock.fun). This repo is the **program root** for `solana-verify`. The site lives at [SpareCashFinance/jrock](https://github.com/SpareCashFinance/jrock).

A matching source hash proves the deployed ELF came from this tree. It does **not** revoke upgrade authority.

## Live v2 (ORAO VRF Classic)

| | |
| --- | --- |
| Program | `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg` |
| Program-data | `4MDqKt9HiM6oHCCbDSsadXuzDXteuhsJh3QJVniztqoA` |
| Upgrade authority | `62C41rN2uUrsZoRkZyTqxD8GJYpa6KtERAtehfNmiXwq` |
| Config | `FBFL6W8ARdkhNM1WFhEfKHqw3HohacK1qhy9jhXQPLwv` |
| Round 0 | `6R2TdLtWQEqgUVe2yLnhf1GKX651JHja1yUfwEurL8st` |
| Library name | `jrock_lotto_v2` |
| Tag | `lotto-v2-mainnet` |
| Ticket | 0.05 SOL (1% kennel fee inside the price) |
| Split | Winner 85% after rent · 15% seeds the next round |
| Randomness | One bound ORAO Classic `request_v2`. Rejection sampling. If anyone bought, settle always picks one of those wallets. `refund_one` is disabled. |
| Book | 10,000 buy rows (1–20 slips each). The round account grows as people buy so we do not lock a huge rent bill up front. |
| Round length | 48 hours (`172800` seconds) |

```
solana-verify verify-from-repo \
  https://github.com/SpareCashFinance/jrock-lotto \
  --program-id 66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg \
  --library-name jrock_lotto_v2 \
  --commit-hash lotto-v2-mainnet
```

Independent draw check: https://petrock.fun/lotto/verify

## Previous v1 (SlotHashes)

| | |
| --- | --- |
| Program | `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` |
| Tag | `lotto-v1-48h` |
| Randomness | SlotHashes after `close_sales`. Interim. Not a VRF. |

Do not upgrade v1 while a funded round holds ticket SOL.

## Build

Anchor 0.31.1. Solana / Agave v4.2.2 in CI.

```
cargo-build-sbf --manifest-path programs/jrock_lotto/Cargo.toml
cargo-build-sbf --manifest-path programs/jrock_lotto_v2/Cargo.toml
```

## Specs

- Live fairness: `docs/lotto-fairness.md`
- v2 fairness: `docs/lotto-fairness-v2.md`
- Audit: `docs/lotto-audit.md` (launch decision: FAIL for a trustless production lottery)
- Frozen v1 snapshot: `docs/lotto-v1-snapshot.md`
