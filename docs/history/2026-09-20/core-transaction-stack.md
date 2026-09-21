# Core transaction stack ownership — 2026-09-20

The reported `manager_sponsored_contract_registration_survives_block_and_committed_replay`
test overflowed the ordinary 2 MiB test stack during committed replay. The
debugger stopped in `WorldTransaction::apply`; the call chain contained no
recursion. Inline transaction journals accumulated across ordinary output
execution, transaction application and the replay caller.

World transaction constructors now return `Box<WorldTransaction>`, and
`StateTransaction` retains that allocation through execution. World application
consumes individual fields from the box, preserving all 278 store applications
in their original order and exhaustive coverage of all 290 fields. Discard and
unwind retain the original checkpoint rollback. This adds one heap allocation
per World transaction instead of repeatedly moving its complete journal through
caller frames. The owned representation uses the existing read-accessor macro.

Measured x86_64 debug stack reservations in the original and repaired harnesses:

| Function | Before, bytes | After, bytes |
| --- | ---: | ---: |
| `execute_network_source` | 746,296 | 135,784 |
| `OutputTransaction::apply` | 196,520 | 30,008 |
| `StateTransaction::apply` | 129,224 | 18,216 |
| `WorldTransaction::apply` | 113,160 | 60,056 |

`StateTransaction` itself shrank from 65,472 to 9,968 bytes. These are measured
debug-build sizes, not portable ABI or release-profile guarantees.

## Validation

The fresh harness was built with:

```sh
cargo test -p iroha_core --lib \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run
```

Runtime tests ran with `RUST_MIN_STACK` unset. The exact reported test passes,
including serial/parallel apply and committed replay. Two new tests enforce a
32 KiB bound on `StateTransaction` and exercise World apply, discard and unwind
on an explicitly fixed 2 MiB stack, checking map/cell checkpoints and event and
catalog publication.

A four-thread selection of all `block::tests` and `state::output_capacity` tests,
the World apply/encoder/index regressions and both bounded-stack native
publication tests passed 175 cases. One periodic-trigger fixture failed with
`configured primary binding differs from its durable anchor`; its exact test,
`repeated_periodic_matches_bind_distinct_positions_and_use_time_actions`, fails
identically in the original and repaired binaries before transaction execution.
Together with the new tests, 177 distinct Rust cases pass.

`cargo fmt --all -- --check`, `scripts/check_no_legacy_codec.sh`,
`git diff --check` and the historical archive verification pass. The accessor
source guard was updated for the boxed owner: seven cases pass, while its
current-source and callback-escape cases still fail on the pre-existing schema
line budget, also reproduced against clean HEAD source. Its unrelated schema
and emitter pins remain unchanged. Full workspace tests were not run.
