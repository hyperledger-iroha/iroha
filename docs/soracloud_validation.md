# SoraCloud validation — 2026-09-09

This correction binds lease usage, runtime serving, rollover and snapshot replay
to the placement incarnation. A replacement assignment has its own counter;
delayed terminal usage can only finalize the predecessor's checkpoint. Identical
reports remain idempotent without consuming an audit sequence.

The lease regression now executes committed phases in separate function calls
instead of retaining every block and transaction overlay in one stack frame.
The original compiled test aborted on the default stack; its frame alone was
about 802 KiB. The decomposed regression now passes on the freshly compiled
Core test binary without a stack override; the complete Core suite is separate.

FHE preflight authenticates the AIR composition and FRI trees in their respective
Merkle domains and binds their opened evaluations. Their roots are not equal.
Valid bounded-noise fixtures use the supported single refresh round. Negative
two-round cases remain negative. Model rollback fixtures build real registered
and promoted history, and exhaustion fixtures set the authoritative sequence
watermark. Error assertions inspect the typed error and its reason.

Validation artifacts are local and ignored under
`dist/soracloud-root-causes-20260909/`:

- The bounded FHE fixture harness compiles the two current helper functions and
  their regression against fresh crypto/data-model libraries: one test passes.
- The lease replay harness compiles the current production helpers and their
  colocated tests against the fresh libraries: five tests pass on the default
  stack. Both new regressions fail against the original `HEAD` helpers, proving
  that replacement counters and rollover incarnation checks detect the bugs.
- The earlier `cargo test -p iroha_core --lib --features
  iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run` failed
  with 1,818 compilation errors from the incomplete Norito identity migration.
  Examples in unchanged source include retired `schema_name` attributes,
  `schema_hash` implementations on codec traits and missing `NoritoSchema`
  declarations. That failed build could not qualify the new Core regressions.
- Five existing SoraCloud Python source-contract modules run 27 tests with
  19 failures and one error. Reading the same source paths from `HEAD` reproduces
  those counts; the checks still assume the old monolithic test layout.
- The retired-codec guard passes.
- The earlier `cargo fmt --all` was blocked by the missing preexisting
  `crates/fastpq_prover/src/backend/compact_protocol/test_fixture.rs` module.
  The changed Rust files were formatted directly with Rustfmt.

The subsequent [workspace compilation repair](cargo_test_compile_validation.md)
restores the missing fixture under a tracked filename and compiles all workspace
test targets. Fresh Core execution passes both replay regressions and
`service_lease_usage_is_reporter_scoped_exact_and_replay_safe`; the lifecycle case
finishes in 10.90 seconds with `RUST_MIN_STACK` unset. Workspace formatting also
passes. Logs are retained under ignored `dist/cargo-test-fixes/` in
`core-proxy-and-lease-replays-01.log` and `core-service-lease-default-stack-01.log`.

The preexisting Core binary used in the earlier diagnosis does not provide
fresh validation; the subsequent results above use the rebuilt artifact.
The shared BFV operation-vector fixture still needs regeneration against the current profile;
no partially regenerated fixture is included here. Full Core/workspace and
four-validator execution remain open.
