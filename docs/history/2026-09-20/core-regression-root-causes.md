# Core regression root causes — 2026-09-20

Scope: the reported `iroha_core` unit run with 16,108 passes and 462 failures.
This record describes the repair and bounded local validation, not release or
four-validator network qualification. No compatibility path was added.

## Production corrections

- Kura replica claim completion and release checks retain the exact authenticated
  physical `LaneStorageEntry` through startup. They no longer look it up through
  the active routing map before that map has been installed. Lane, dataspace,
  incarnation, activation and network checks remain enforced.
- Receipt append recovery refuses a competing raw-evidence rewrite before
  resuming its own append. Strict startup remains the recovery owner.
- Kotodama instruction bridges advertise conservative dynamic read/write access
  even when their payload is a compiler-known literal. Literal access keys remain
  useful hints, but do not claim completeness for a dynamic syscall. This status
  propagates through internal calls and agrees with IVM artifact admission.
- Committed autoscale policy drift uses the deterministic transition-plan error,
  so it cannot be mistaken for a local physical-storage failure. Rejection keeps
  the previously committed catalog and geometry intact.

## Canonical fixture corrections

- Catalog, alias, manifest and autoscale fixtures install canonical configuration
  or lifecycle transitions. Mutating the process-local Nexus cache does not
  change authoritative World/Cell state. Deliberately malformed router snapshots
  are confined to read-only component tests and cannot publish storage.
- Ordinary network execution follows a real signed genesis, captured execution
  outputs, verified three-of-four finality and committed history. Missing genesis
  authority is established by its first self-registration instruction. No genesis
  admission or execution-output checks are waived.
- Direct callback tests explicitly create their root execution owner and consume
  actual callback capture before applying a successful component overlay. Block
  tests retain the original output owner through finality and publication.
- Recovery fixtures bind configured geometry and network identity before creating
  physical storage. Unchanged lanes retain their incarnation through unrelated
  catalog changes. Hostile-body tests distinguish rejection by the canonical
  writer from rejection while reading deliberately corrupted persisted evidence.
- Snapshot fixtures retain actual predecessors and authoritative source records.
  Component-only AXT replay is labeled separately from publication; a forged
  post-validation envelope has an explicit publication-rejection regression.
- Removed detached/DAG execution paths are no longer asserted by tests of the
  canonical output executor. Those tests retain state, rollback, ordering and
  deterministic-output assertions. Gas tests account for the zero-cost HALT and
  use actual charged instructions when testing consumed work.
- Benchmark fixtures use explicit test startup, account permissions, asset
  owning domains and retained execution/finality publication. A release
  benchmark also rejects failed transaction outputs instead of silently
  measuring rejected work.

## Validation

The Core harness was built with:

```sh
cargo test -p iroha_core --lib \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run
```

Named Core selections ran with four test threads. Together they cover 2,745
distinct passing tests and one existing ignored measurement. The following
receipts are separate selections and overlap; their counts must not be added
together.

| Selection | Result |
| --- | --- |
| Complete `kura::tests` suite | 1,081 passed; one existing measurement ignored |
| Lifecycle startup recovery | 12 passed |
| Storage/recovery/proof/snapshot coverage, including the Kura suite | 1,341 distinct passes; one existing measurement ignored |
| Catalog, router, gossip, alias, committee, transaction and AXT coverage | 881 passed with default stack settings |
| Block, callback and trigger regressions | 123 passed with default stack settings |
| Adjacent canonical-genesis, output ownership, FASTPQ and SCCP controls | 20 passed with default stack settings |
| Executor, overlay, signature, publication, query-index and historical-evidence coverage | 334 passed |
| Additional changed-fixture and helper coverage | 34 passed |
| Typed autoscale drift, original local-storage errors and publication atomicity | 20 passed on build 13 |
| Complete `kotodama_lang` library suite | 1,110 passed |

The storage receipts span repair builds 3, 8 and 10; the other Core selections
above were rerun on build 12, with the final autoscale changes checked on build
13. The coverage audit maps every reported failure to a passing test, including
14 renamed successors that exercise the current canonical execution model.
These are scoped regression receipts, not a fresh run of every Core test or the
complete workspace.

Additional checks passed:

```sh
cargo test -p kotodama_lang --lib -- --test-threads=4
cargo check -p iroha_core --features iroha-core-tests \
  --bench apply_blocks --bench validate_blocks \
  --example apply_blocks --example validate_blocks
cargo check -p iroha_core --features iroha-core-tests --test iroha_core_group_03
cargo build -p iroha_core \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry \
  --example apply_blocks --example validate_blocks
target/debug/examples/validate_blocks
cargo fmt --all -- --check
scripts/check_no_legacy_codec.sh
python3 scripts/archive_project_history.py verify \
  --archive docs/history/2026-09-06 --check-current
```

The validation benchmark completed all three registration, deletion and
restoration blocks with 100 accounts and 100 asset definitions. A bounded
application smoke driver imported the same shared benchmark helpers, published
all three phases for two domains with four accounts and four asset definitions,
then replayed them into an identical second State. Final heights, block hashes
and restored entities matched. Both shared benchmark helper unit tests also
passed. The larger 1,000-account application benchmark remains a separate,
unfinished debug run; it is not counted as a passing check. The complete
workspace and deployed network matrix are outside these local receipts.
