# SCCP light clients and live-mainnet builders, September 28

This is scoped implementation evidence from the shared checkout. It does not
qualify a Taira deployment, real-value transfers, or release activation.

## Scope

The Ethereum, BSC, TRON and TON light clients verify bootstraps, advances,
backfills, inbound/void proofs and equivocation evidence from the compiled
chain profiles (`specs/sccp.md` §4.13). Builders in `iroha_sccp_rpc` produce
the frames from public mainnet endpoints; the CLI covers the Solidity
destinations (Ethereum, BSC, TRON) and the TON wallet-v5r1 lane. Core reserves
estimated verifier work under the `[zk.sccp]` transaction and block limits
before any verifier runs.

## Live mainnet checks

```sh
cargo test -p iroha_sccp_rpc --test live_mainnet -- --ignored --nocapture
```

All four checks pass against the compiled default public endpoints. Each builds
a bootstrap, installs it in an in-memory light client with the compiled
verifier, then builds and applies an advance from the installed head:

- Ethereum: bootstrap at sync-committee period 1869 (30 872 bytes); a
  3 172-byte advance verifies.
- BSC: bootstrap at epoch checkpoint 124 485 000 (4 724 bytes); a 1 180-byte
  advance moves the head to 124 485 709.
- TRON: bootstrap of maintenance period 82 897 (1 767 bytes); an 11 658-byte
  advance moves the solid head from 86 636 135 to 86 636 146.
- TON: bootstrap at key block 95 444 355 (35 174 bytes). A second run
  bootstraps at the previous key block 95 424 597 and applies a 44 978-byte
  advance that hops to 95 444 355 under real validator signatures and config
  proofs, at a Taira time just after that key block while the earlier epoch
  was still fresh.

`iroha sccp lc-bootstrap` builds and self-verifies the Parliament
`InitializeLightClient` action for every chain against the same endpoints.

Without a deployed SCCP contract, each EVM/TRON test also builds a source
proof of an ordinary successful mainnet transaction and checks that the
verifier passes every anchor, header-link and inclusion step before failing
at the SCCP event:

- Ethereum: an 11 174-byte proof (finality update, header chain, receipt
  trie) fails with `log topic0 does not match the event`.
- BSC: a 3 616-byte proof (vote attestation, headers, receipt trie) fails
  with the same event mismatch.
- TRON: a 6 681-byte proof (solid segment, `txTrieRoot` branch, re-encoded
  transaction) fails with `calldata selector does not match the function`.

Two defects surfaced only against live data and are fixed with regression
tests: the TRON maintenance grid is aligned to 00:00, 06:00, 12:00 and
18:00 UTC (`getnextmaintenancetime` returned 1 790 596 800 000), and the TRON
maintenance-block search now reads forward from its lower bound, because
missed slots move the block later than the time-based estimate.

## Focused suites

- `cargo test -p iroha_sccp --lib`: 250 pass (Ethereum, BSC, TRON and TON
  light clients, TON BoC and cell codecs, synthetic chains).
- `cargo test -p iroha_sccp --tests`: 21 light-client, 1 native and 1 vector
  test pass (2 ignored).
- `cargo test -p iroha_sccp_rpc --lib`: 102 pass.
- `cargo test -p iroha_sccp_wallet --lib`: 48 pass (EVM, TRON and TON wallet
  lanes).
- `cargo test -p iroha_cli --bin iroha -- sccp`: 12 pass.
- `cargo test -p iroha_core --lib -- sccp_governance_proposals planner_tests
  smartcontracts::isi::sccp`: 115 pass, including verifier-work metering,
  the Parliament planner and the proposals read.
- `cargo test -p irohad --lib -- sccp`: 11 attestor and keeper tests pass.
- `cargo test -p iroha_config --lib -- sccp` and `--test '*'`: 22 and 284 pass,
  including the configuration snapshot with the new `[zk.sccp]` defaults.

## Parliament driver

`GET /v1/gov/parliament/attempts/{id}/plan` returns Core's driver plan: every
permissionless transition the reducer accepts at the execution height of a
transaction sent now (tip + 3), found by trial application to a copy of the
attempt, plus exact-height checkpoints and ballots awaiting a relay or the TLE
release. `GET /v1/sccp/governance/proposals` lists open SCCP proposals with
their admissibility and newest attempt. `iroha sccp governance drive` submits
from both and ticks an idle tip. Five planner tests, one proposals-read test
and two driver tests pass; the OpenAPI authority now documents the SCCP read
API and the plan route.

## Not established

- No SCCP contract is deployed, so no inbound or void source proof has been
  built from a real transfer.
- The driver's exact-height checkpoint timing has not run on a live
  four-validator network.
- Four-validator integration tests and real-value canaries have not run.

## Retired status row

The status row below described the retired native-proof design and is
preserved verbatim, with its link rebased to this directory:

| SCCP TON scoped audit | [Validated fixes and evidence](../../source/sccp_ton_security_audit_2026_09.md): ordinary transfer funding, bounded replay work, native TL-B parsing, exact checkpoint identity, complete breaker readbacks, builder Git/verifier/attribute isolation, and canonical wire identifiers across Rust/SDKs, circuits and contracts. Earlier focused Rust/Core/model/production compile checks pass. Fresh validation: 459 Python tests with zero skips, 45 TON contract tests, authenticated StateInit write/check, and pinned EVM/TRON compiler plus EVM runtime smoke pass. Rust validator suites pass 22/27 tests, and the compiled Rust wire fixture passes. Policy/proof negatives have positive controls and precise rejection checks. All 8 R1CS identities are freshly measured, with a verified source closure and no pending profiles. | Full Core/workspace and Torii runtime tests are unclaimed. Production keys/proofs, trusted release signatures and authenticated deployment readbacks remain separate release artifacts. |
