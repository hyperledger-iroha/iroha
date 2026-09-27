# SCCP v1 TON contracts: build, tests and artifacts

This note covers the repository tooling for the TON destination of SCCP v1
(`specs/sccp.md` revision 3, §5.3–§5.5). The contracts live in
[`contracts/ton/sccp`](../../contracts/ton/sccp):

| Contract | Source | Role |
|---|---|---|
| `SccpTairaXorMinter` | `contracts/SccpTairaXorMinter.tolk` | TEP-74 / TEP-89 Jetton master with the bridge built in |
| `SccpTairaXorWallet` | `contracts/SccpTairaXorWallet.tolk` | TEP-74 wallet; burns require the `sccp_burn_to_taira` payload |
| `SccpConsumedBucket` | `contracts/SccpConsumedBucket.tolk` | 512 replay flags of one minter, deployed once, never redeployed |

Shared modules: `sccp-constants`, `sccp-errors`, `sccp-messages`,
`sccp-storage` (exact TL-B of §5.3.1–§5.3.3), `sccp-crypto` (`GLOBALID`,
`ECRECOVER` and `HASHEXT` keccak asm wrappers), `sccp-cells` (canonical snake,
hash-chunk and member-chunk cells), `sccp-verify` (§3 digests, leaves, paths,
payloads, signature sets), `sccp-fees` (§5.3.5) and `sccp-addresses`.

There is no owner, admin, upgrade path or setter. The minting pause changes
only through `sccp_apply_control` / `sccp_apply_control_historical` with a
Parliament control leaf attested by the Taira bridge roster (§5.1.6).

## Toolchain

Acton 1.2.0 bundling Tolk 1.4.2, native on macOS arm64/x86-64 and Linux
arm64/x86-64; no Docker, container runtime or Rosetta. `Acton.toml` pins
`acton = "1.2.0"`. `scripts/ton_sccp_builder.py` pins the official release
archives by SHA-256 (from the release `sha256.sum`), downloads the host archive
once, verifies it, extracts only the `acton` executable into the ignored
`contracts/ton/sccp/.toolchain/` cache, and requires the exact version line
`acton 1.2.0 (16d49e1 2026-09-16)`. After a build it also requires the
project stdlib to declare `tolk 1.4.2`. `--acton /abs/path` uses an existing
executable with the same version check; `--offline` forbids downloads. No
environment variable changes behaviour.

## Commands

```text
scripts/sccp_ton_contract_build.sh            # everything below (default `all`)
python3 scripts/ton_sccp_builder.py toolchain  # resolve and print the pinned Acton
python3 scripts/ton_sccp_builder.py vectors [--check]
python3 scripts/ton_sccp_builder.py fmt [--write]
python3 scripts/ton_sccp_builder.py build
python3 scripts/ton_sccp_builder.py wrappers [--write]
python3 scripts/ton_sccp_builder.py test [acton test options]
python3 scripts/generate_ton_sccp_stateinit_golden.py [--check]
python3 -m pytest scripts/tests/ton_sccp_builder_test.py
```

`all` runs, in order: the test-vector freshness check, `acton fmt --check` on
the SCCP sources, `acton build` with the stdlib check, the wrapper freshness
check (`acton wrapper --all` in a scratch copy must reproduce
`wrappers/*.gen.tolk`), the emulator suite `acton test tests/unit`, and the
StateInit golden check. Wrapper names are the PascalCase contract ids.

For local end-to-end runs against a full TON node emulation, use
`acton simulator` (it replaces the retired lightweight localnet); the lane
tests of `integration_tests/tests/sccp_lanes/ton.rs` own that flow.

## Test vectors

Tolk has no secp256k1 signing, so `sccp-test-vectors.gen.tolk` carries 31
deterministic test keys (sorted by address) with one fixed ECDSA nonce each,
plus golden values computed by the independent Python implementation of §3 in
`ton_sccp_builder.py` (domain separator, attestation digest, roster digests
with keyless slots and six chunks, TON-target control leaves, inbound and
outbound payload hashes and message ids, transfer leaf, promote-odd Merkle
paths, history leaf, and an independently signed ECDSA vector). The Python
implementation first pins itself against every constant printed in the spec,
including both `SCCP/CONTROL/V1` example leaves. Tests sign any digest with
two modular products, so the suite runs in seconds.

## StateInit golden

`fixtures/sccp/ton_stateinit_v1.json` pins, for two fixed rosters
(`n = 4`, and `n = 31` with three keyless slots and six member chunks) under
NetworkId `0x11…11`:

- the code cell hash, depth, cell count and bit count of the minter, wallet
  and bucket;
- every roster field, the §3.7 digest, and each cell of the canonical initial
  data (§5.3.1) with its data bits, hash, depth and references;
- the StateInit hash and the workchain-0 address;
- the wallet address of owner `0:abab…ab` and the addresses of buckets 0 and 1.

The generator parses the compiled code BoCs, recomputes everything in Python,
runs `contracts/ton/sccp/scripts/stateinit-golden.tolk` (the same values from
the contracts' own Tolk types via `acton script`) and requires exact agreement.
The Python tests recompute the data hash and every address from the recorded
code hashes and depths alone, which is exactly what Taira's `RegisterRoute`
and the Rust `iroha_sccp::v1::ton_cell` builder do.

## Emulator suite (`tests/unit`)

104 Acton tests cover every §5.3 rule and §11 Contracts bullet, including:
`sccp_init` (digest, generation, `n`/`t`/order, validity bounds, chain id,
non-canonical data, library-cell members, double init, operations before
init); the minter floor deficit charged to the first caller; direct and
historical finalize; deadline and cap edges; bucket boundaries, the four-ahead
deploy rule and the non-redeploy rule (a deleted bucket is never recreated
with clear flags); duplicate and parallel duplicate finalizes and pending
supply; bounced `sccp_consume`, bounced `internal_transfer` then unconsume and
re-finalization, bounced `sccp_consumed` and `sccp_range_consumed`; signature
malleability (high-S, `v ∉ {27,28}`, order, keyless slots, extra or missing
cells); proof and leaf negatives; voids (`void_expired` direct and historical,
deadline edge, nonce argument, `void_frozen` range, frozen and deploy rules);
rotations (grace cap, sequential replay, current-roster-only, digest,
timestamp, validity, skew and `validFromMs` bounds, library-cell member
chunks); controls (pause, resume, stale, equal and zero nonces, gaps,
historical mode, foreign network, destination, revision or Taira network, a
transfer leaf offered as a control, burns, rotations and voids open while
paused, frozen destinations); burns (payload and message id, sequential nonce,
1024-byte recipient, plain-burn and foreign-payload rejection, recipient
length, shape and library cells, workchain rule with balance restore);
canonical cells (a library reference with canonical content is rejected at
every chunk of a snake, hash path or member list); TEP-74 transfers and
bounces, TEP-89 and the TEP-64 content; and the fee constants.

## Measured gas and value (§5.4)

Measured with the Acton 1.2.0 emulator and its default network config
(`tests/unit/sccp-fees.test.tolk` prints these values and fails when a
per-step limit in `sccp-fees.tolk` is exceeded). Gas is the compute gas of
the named step; value is what the finalizer paid net of the returned excess.

| Operation | n = 4 (t = 3) | n = 31 (t = 21) | §5.4 estimate |
|---|---|---|---|
| finalize, minter step (direct) | 42 767 gas | 106 981 gas | ≈15k / ≈50k |
| finalize, minter step (historical, block path 9, history path 32, 1024-byte sender) | — | 171 514 gas | +2k–5k |
| finalize total value, bucket and wallet exist | 0.0052 TON | 0.0095 TON | 0.02–0.05 TON |
| finalize total value deploying 4 buckets and the wallet | — | 0.6456 TON | — |
| void expired, minter step (historical worst case) | — | 162 932 gas | as finalize |
| apply control, minter step | 30 019 gas | 97 120 gas | ≈12k / ≈45k |
| rotation, minter step (n → n) | 30 176 gas | 115 128 gas | ≈15k / ≈60k |
| burn: wallet / minter (1024-byte recipient) | 5 000 / 38 168 gas | — | burn + ≈10k |
| void frozen (512 nonces): minter / bucket / reply | 12 459 / 6 832 / 5 765 gas | — | ≈10k + round trip |
| other steps | bucket consume 5 940, minter `sccp_consumed` 9 982 (void 8 047), wallet receive 4 155 (7 173 with notification), wallet transfer 5 991, bucket unconsume 3 521, 16-bucket deploy 48 871 | | |

Signature checks dominate: `ECRECOVER` is 1 526 gas each and the per-signature
parsing and bound checks add about 2 400 gas. The floors at the same prices are
`BUCKET_FLOOR` 0.1559 TON and `MINTER_FLOOR` 1.2018 TON (100 years), and a new
wallet keeps 0.0052 TON; a finalize that deploys a bucket pays that bucket's
floor once (0.0003 TON amortized per nonce).

## Interface notes

- `finalize_required_value(nonce)`, `void_required_value(nonce)`,
  `void_frozen_required_value(first_nonce)` and
  `deploy_buckets_required_value(count)` are get methods; the nonce selects
  how many buckets the call deploys. The wallet exposes
  `burn_required_value()` (a burn must carry strictly more).
- External events use `addr_none` as destination; bodies follow §5.3.3 and
  are stored inline or in one reference as the message size requires.
- Both floors are reserved as `max(floor, balance before the message)` with
  `raw_reserve` mode 2, so top-ups extend an account's life instead of being
  forwarded to the next caller.
- Buckets expose `get_bucket_data`, `get_bits` (`(hi, lo)`, flag `i` is bit
  `i` of `hi ‖ lo`) and `is_consumed(nonce)`.
