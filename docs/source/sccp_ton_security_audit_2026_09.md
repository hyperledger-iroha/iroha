# SCCP TON scoped security audit — 2026-09-06

This audit covers the TON wallet/master/bridge message and replay boundaries,
native TON finality/account openings, consensus breaker readbacks, and SCCP
message checkpoint authorization. It reviews the shared working tree while
merge and OpenAPI work proceeds independently. Existing changes are preserved;
the audit does not own the merge, index, lockfile, or OpenAPI output.

## Validated findings and fixes

| Boundary | Finding | Fix and regression |
| --- | --- | --- |
| TON ordinary Jetton transfer | Mode 64 added an explicit transfer value to the remaining inbound value. Exact balance reservation then caused action error 37, even with sufficient funding. | Send zero explicit value with mode 64. The emulator checks recipient deployment, both token balances, total supply, and retained TON reserves. |
| TON replay processing | Every message scanned every stored replay shard. Four populated wallet forests exhausted the hard 1,000,000 gas limit before burn validation, making mature wallets unusable. | Runtime validates counters/emptiness and the selected root; immutable initialization and append-only admission preserve other roots. Complete supplied-forest validation remains available. A regression populates all 256 shards of all four wallet forests and reaches the expected witness rejection without exhausting gas. |
| Native TON wire decoding | ShardIdent incorrectly expected an in-memory shard terminator on the wire; BlockExtra and GlobalVersion omitted required constructor tags. Honest canonical proofs were rejected. | Restore the terminator from the wire prefix, consume exact constructors, and reject malformed alternatives. Independent wire vectors and signed ordinary/Simplex continuation fixtures cover the corrected layouts. |
| Message circuit checkpoint | At the anchor height, message authorization checked height/epoch/roster but did not require the exact checkpoint block, context, and finality-artifact identities. | Apply the same equality requirements as epoch authorization. Six independent negative cases cover TON BLS12-381 and BN254; matching and earlier checkpoints remain accepted. This is an authorization-consistency fix, not a demonstrated signature forgery. |
| Consensus TON breaker readback | Native verification returned authenticated transaction hash/start LT/storage LT, but Core dropped them from the persisted record and its digest. | Preserve all three mandatory fields, enforce native logical-time rules, and cover binary/JSON round trips, absent JSON fields, conversion, and digest sensitivity. |

The shared shard descriptor parser also rejects zero masterchain registration
for a nonzero shard block. This is fail-closed input hardening, not a separately
demonstrated false-finality acceptance.

TON wire layouts were checked against upstream
[`block.tlb`](https://github.com/ton-blockchain/ton/blob/master/crypto/block/block.tlb)
and [`ShardIdent::pack/unpack`](https://github.com/ton-blockchain/ton/blob/master/crypto/block/block-parse.cpp).

## Validation

- Pinned Acton 1.1.0 / Tolk 1.4.1: `acton test --project-root
  contracts/ton/sccp` passes all **42 tests**. The three contracts remain within
  the tested code-size/depth limits.
- `python3 -m pytest -q scripts/tests/ton_sccp_builder_test.py
  scripts/tests/ton_sccp_stateinit_golden_test.py`: **15 passed**.
- The authenticated golden generator `--write` and independent `--check`
  agree. Fixture SHA-256 is
  `e2cb473512dd9ac5ae7e1d574c58917d3be0a967ca9a5163a118db5cd1f97206`;
  route/master code depths are 53/37 and initial-data depths are 11/11.
- Go 1.25.7: the message-anchor regression, focused KAT/inventory/epoch-anchor
  checks, `go test ./... -run '^$'`, and CLI build pass. The anchor regression
  was independently rerun with `-count=1`.
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
  the updated model. The complete Core test harness, the new Core converter
  unit test, and Torii runtime tests were not executed; none is claimed as a
  test pass.

The native signature/finality, replay authentication, reserve, and breaker
controls remain enabled throughout validation. Full forest validation is still
used for supplied state; constant runtime work relies on canonical immutable
deployment and the sole authenticated append-only mutation path.

## Scope limits

Changed message constraints require new R1CS counts and regenerated dependent
keys/proofs/verifiers under the existing signed release corridor. The circuit
manifest explicitly marks prior message counts historical. Local tests do not
qualify a deployed TON contract or a production ceremony.

The TON builder's Git environment and signature-helper isolation were reviewed
but no bypass was validated in this pass; they remain follow-up review items.
The full workspace, unrelated SDK/OpenAPI surfaces, live funds, and production
deployment were outside this scoped validation.
