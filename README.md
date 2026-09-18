# jrock-lotto

On-chain kennel lotto for `$JROCK` / [petrock.fun](https://petrock.fun). This repo is the **program root** for `solana-verify`. The site lives at [SpareCashFinance/jrock](https://github.com/SpareCashFinance/jrock).

A matching source hash proves the deployed ELF came from this tree. It does **not** make SlotHashes a VRF, and it does not make the live draw 100% fair.

## Live v1 (SlotHashes)

| | |
| --- | --- |
| Program | `FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC` |
| Program-data | `BZ2RWk6QZFspNKCHiESE7uYzfeCXMjpSvBJYgas26oWi` |
| Upgrade authority | `62C41rN2uUrsZoRkZyTqxD8GJYpa6KtERAtehfNmiXwq` |
| Config | `p9XrAnsutdWKWnBxiCb57fPuuoH2mvdq32nBHKhwh7K` |
| Library name | `jrock_lotto` |
| Frozen tag | `lotto-v1-mainnet` |
| Ticket | 0.05 SOL (1% kennel fee inside the price) |
| Split | Winner 85% after rent · 15% seeds the next round |
| Randomness | SlotHashes after `close_sales`. Interim. Not a VRF. |
| Round length at init | 72 hours (`259200` seconds), stored on config |

This branch is the **48-hour ELF**. If the live round is Open with zero slips, `set_round_secs(172800)` rewrites that rock's `end_ts` to start + 48 hours and sets later rocks to 48 hours. Leftover seed stays. Do not deploy this over a round that already has tickets.

After this ELF is live, verify against this commit instead of `lotto-v1-mainnet`.

Pending (not live): branch [`v1-48h`](https://github.com/SpareCashFinance/jrock-lotto/tree/v1-48h) adds `set_round_secs` so later rocks can be 48 hours. Do not verify the live program against that branch. The current rock's `end_ts` stays at the 72-hour clock written on-chain.

```
solana-verify verify-from-repo \
  https://github.com/SpareCashFinance/jrock-lotto \
  --program-id FvQfcJYAcRFEDeq8rS19MNXTZfeiCxcSN5nmfA6RdWuC \
  --library-name jrock_lotto \
  --commit-hash lotto-v1-mainnet
```

Then confirm the verified badge on Explorer and Solscan. Independent draw check: https://petrock.fun/lotto/verify

## v2 successor (not production)

| | |
| --- | --- |
| Program id (undeployed) | `66FyiUTkw4JMYMi3yErfa7UBqrHm9GZha1meAxcgbjDg` |
| Library name | `jrock_lotto_v2` |
| Randomness | ORAO VRF Classic |
| Default round length | 48 hours (`172800` seconds) at initialize |

Do not point production at v2 until it is initialized on mainnet after the live v1 round is claimed or refunded.

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
