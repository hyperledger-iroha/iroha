# SCCP TON scoped security audit — 2026-09-06–07

This audit covers the TON wallet/master/bridge message and replay boundaries,
native TON finality/account openings, consensus breaker readbacks, SCCP
message checkpoint authorization, and the SCCP release builders' signed-source
boundaries. It reviews the shared working tree while
merge and OpenAPI work proceeds independently. Existing changes are preserved;
the audit does not own the merge, index, lockfile, or OpenAPI output.

## Validated findings and fixes

| Boundary | Finding | Fix and regression |
| --- | --- | --- |
| TON ordinary Jetton transfer | Mode 64 added an explicit transfer value to the remaining inbound value. Exact balance reservation then caused action error 37, even with sufficient funding. | Send zero explicit value with mode 64. The emulator checks recipient deployment, both token balances, total supply, and retained TON reserves. |
| TON replay processing | Runtime replay validation scanned every stored shard. Four populated wallet forests exhausted the hard 1,000,000 gas limit before burn validation, making mature wallets unusable. | Runtime validates counters/emptiness and the selected root; immutable initialization and append-only admission preserve other roots. Complete supplied-forest validation remains available. A regression populates all 256 shards of all four wallet forests and reaches the expected witness rejection without exhausting gas. |
| Native TON wire decoding | ShardIdent incorrectly expected an in-memory shard terminator on the wire; BlockExtra and GlobalVersion omitted required constructor tags. Honest canonical proofs were rejected. | Restore the terminator from the wire prefix, consume exact constructors, and reject malformed alternatives. Independent wire vectors and signed ordinary/Simplex continuation fixtures cover the corrected layouts. |
| Message circuit checkpoint | At the anchor height, message authorization checked height/epoch/roster but did not require the exact checkpoint block, context, and finality-artifact identities. | Apply the same equality requirements as epoch authorization. Six independent negative cases cover TON BLS12-381 and BN254; matching and earlier checkpoints remain accepted. This is an authorization-consistency fix, not a demonstrated signature forgery. |
| Consensus TON breaker readback | Native verification returned authenticated transaction hash/start LT/storage LT, but Core dropped them from the persisted record and its digest. | Preserve all three mandatory fields, enforce native logical-time rules, and cover binary/JSON round trips, absent JSON fields, conversion, and digest sensitivity. |
| SCCP release builder source trust | TON left ambient Git configuration, replacement objects, hooks, and signature helpers active. Both builders allowed unsigned attributes and configured clean/smudge helpers during status or archive; a pass-through smudge helper executed while preserving the archive digest. The validator also left non-OpenPGP signature helpers unpinned. | Isolate source checks and archival in fresh private Git metadata using original objects and a read-only original index. Exclude unsigned attributes/configuration and pin every signature-helper slot, including archive signature substitutions. Require an approved TON verifier digest. Regressions cover original-object reads, helper dispatch, rejected digests, archive bytes, clean/dirty/staged status, and unchanged original index bytes. |
| Cross-language SCCP wire identifiers | The Rust model and SDKs deliberately use TRON domain 5, payload codecs 1/2/5/7, Transfer tag 2 and hub kind 5, while circuits and destination contracts retained compacted aliases. TON also selected destination proof backend 3 instead of 2. Correctly encoded bridge messages could not pass these inconsistent boundaries. | Align circuits, contracts and release tooling with the canonical model; reject compacted aliases and compare independent canonical fixtures across implementations. Recompile affected circuit identities and contract artifacts. |

The shared shard descriptor parser also rejects zero masterchain registration
for a nonzero shard block. This is fail-closed input hardening, not a separately
demonstrated false-finality acceptance.

TON wire layouts were checked against upstream
[`block.tlb`](https://github.com/ton-blockchain/ton/blob/master/crypto/block/block.tlb)
and [`ShardIdent::pack/unpack`](https://github.com/ton-blockchain/ton/blob/master/crypto/block/block-parse.cpp).

## Validation

- Pinned Acton 1.1.0 / Tolk 1.4.1: `acton test --project-root
  contracts/ton/sccp` passes all **45 tests**. The three contracts remain within
  the tested code-size/depth limits.
- `python3 -m pytest -q scripts/tests/ton_sccp_builder_test.py
  scripts/tests/ton_sccp_stateinit_golden_test.py`: **41 passed**.
- `python3 -m pytest -q scripts/tests/sccp_validator_builder_hardening_test.py
  scripts/tests/sccp_validator_builder_test.py`: **75 passed**.
- The initial six-file Python run passed **377 tests with 28 skips**.
  Follow-up inspection corrected the skip classification: 26 needed only a
  local Rust executable, while two were empty placeholders. The local validator
  is now built and exercised. Policy mutations use current typed unit envelopes
  with valid ephemeral signatures; every control passes all policy/audit checks
  before an intentional absent-build-freshness rejection. Each mutation asserts
  its intended error. Native proof and lane-parser mutations run directly at
  their Rust boundaries with positive controls. The two placeholders are replaced
  by signed unit-evidence integrity/readiness and deterministic bundle tests.
- Final combined run of the six original builder/golden/release/corridor files,
  the wire-inventory guard and contract-artifact suite: **459 passed, zero skips**,
  with `SCCP_RELEASE_RUST_VALIDATOR` selecting the freshly built normal executable.
- `scripts/sccp_evm_contract_smoke.sh` passes with authenticated, pinned Solidity
  0.7.4 EVM/TRON compiler artifacts and the refreshed artifact lock. The EVM
  runtime verifies canonical Rust transfer vectors and rejects retired payload,
  codec and TRON-domain aliases. TRON compilation/static checks and the separate
  EVM compatibility harness pass; this run does not execute a TRON node.
- The release-corridor, cross-language wire-inventory and contract-artifact
  Python guard suites pass **81 tests**. The wire guard is included in the
  corridor's `evidence-scripts` phase.
- `iroha_sccp::source_identity::tests::shared_native_transfer_event_vectors_match_exact_rust_wire`
  passes in the compiled library harness, anchoring the shared transfer fixture
  used by that cross-language guard to the canonical Rust model and hashes.
- The normal `sccp_release_evidence` executable builds with only `dev-tools`.
  Its focused binary suite passes **22 tests**; the separate
  `dev-tools,test-fixtures` suite passes **27 tests**. A current typed unit
  context verifies two release and twelve audit signatures before the replay
  and noncanonical-scalar mutations. A valid native proof establishes both
  envelope-binding and canonical event-digest rejection controls. The ordinary
  test harness's stale fixture-feature gate and empty-policy fixture were
  repaired without adding fixture support to the normal executable.
- The authenticated golden generator `--write` and independent `--check`
  agree. Fixture SHA-256 is
  `fd3f75b1baaed8619c9d13265a150c0b9f7d3dcc4964edbe64a2fd1c385a2cae`;
  route/master code depths are 53/37 and initial-data depths are 11/11.
- Go 1.25.7: the message-anchor regression, focused KAT/inventory/epoch-anchor
  checks, `go test ./... -run '^$'`, and CLI build pass. The anchor regression
  was independently rerun with `-count=1`. The final identity serialization,
  definition-source closure, and inventory tests also pass in an independent
  all-package compile/test run. The source-closure guard detects modified,
  added, or symlinked definition inputs, and the actual vendor inventory was
  recomputed and compared successfully.
- All **8 circuit profiles** have fresh constraint counts, canonical R1CS byte
  lengths and SHA-256 identities after the wire fix. The source closure remained
  `2926b916e2e905126ad16157b907ac045280b75ded21d0050d5d5f7f36973318`
  throughout the measurements. The manifest marks all identities current with
  no pending profiles; final source-closure and inventory checks pass. All four
  message identities and the TRON epoch identity changed. The other three epoch
  identities were recompiled and independently confirmed unchanged.
- Rustfmt checks for the native/parser and breaker-model files, Acton format
  checks, scoped whitespace checks, and `scripts/check_no_legacy_codec.sh`
  pass.
- `cargo test --locked -p iroha_sccp --lib`: **191 passed**, including all
  **35 TON native tests** and 14 replay-archive tests. Initial failures in old
  TON fixtures were repaired to construct canonical shared-cell BOCs and
  exercise the accepted single outer proof wrapper while rejecting nested
  wrappers. Production validation was not relaxed.
- The freshly compiled data-model test binary passes the new account readback
  regression, 13 replay tests, and 11 FASTPQ tests. The locked Cargo build took
  5m53s. Initial attempts encountered unrelated SoraFS named-enum schema errors
  and then a merge marker in `fastpq.rs`; the merge task resolved these before
  the passing runs.
- `cargo check --locked -p iroha_torii --lib` passes, including Core's production
  library, in 4m21s. This confirms the native-to-consensus conversion against
  the updated model.
- The current compiled Core test binary passes all **9 TON breaker tests**,
  including transaction-coordinate conversion/digest sensitivity, quotas,
  outbound gates, anchor checks, compare-and-swap/latching, hydration, and
  restart. Source timestamps precede compilation and source/binary hashes
  stayed unchanged during the run. The full Core suite and Torii runtime
  tests were not executed.

The native signature/finality, replay authentication, reserve, and breaker
controls remain enabled throughout validation. Full forest validation is still
used for supplied state; constant runtime work relies on canonical immutable
deployment and the sole authenticated append-only mutation path.

The September 7 Rust validation used a separate checkout of committed
`bc87beb46436f0072c500caa95c6de0615fc3bcd` with the audit's final CLI source
overlay, preserving the concurrent merge and index. The preserved normal
executable is a local debug build with only `dev-tools`; production release
qualification is not inferred from its build profile or the ephemeral test keys.

## Scope limits

All eight current circuit definitions are recompiled and hashed in
[`constraint-counts-final-v1.json`](../../circuits/sccp/manifests/constraint-counts-final-v1.json).
Dependent keys/proofs/verifiers require regeneration
under the existing signed release corridor. Local tests do not qualify a
deployed TON contract or a production ceremony.

TON production policies must now contain `builder.host_commit_verifier_sha256`,
and both production commands require `--commit-verifier`. Policies without the
field fail validation. Production builders still authenticate the policy bytes
against their trusted digest; unit signing keys used to exercise validation do
not supply production policy-root, auditor, or release-role signatures.

The full workspace, unrelated SDK/OpenAPI surfaces, live funds, and production
deployment were outside this scoped validation.
