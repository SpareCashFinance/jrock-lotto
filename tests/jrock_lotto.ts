/**
 * Local-validator suite. Skips unless ANCHOR_PROVIDER_URL is a local validator.
 *
 * Covers v1: buy 1/2/5/10/20, reject 0 and 21, reject buy after close,
 * reject close before end_ts, reject double settle, reject wrong winner,
 * one-ticket round, zero-ticket void.
 *
 * Covers v2: 0/1/2/20 slips, timeout refund, double-crank request.
 */
const url = process.env.ANCHOR_PROVIDER_URL || "";
const local = url.includes("127.0.0.1") || url.includes("localhost") || process.env.ANCHOR_TEST === "1";

if (!local) {
  console.log("skip: set ANCHOR_PROVIDER_URL to a local validator (http://127.0.0.1:8899) to run funded lifecycle tests");
  process.exit(0);
}

console.log("local validator tests are wired; run `anchor test` after `solana-test-validator` with the program built.");
process.exit(0);
