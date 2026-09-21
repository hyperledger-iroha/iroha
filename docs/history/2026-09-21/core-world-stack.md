# Core World stack ownership — 2026-09-21

The reported
`state::tests::approved_pin_snapshot_requires_its_exact_automatic_replication_order`
overflow reproduces on the ordinary 2 MiB test stack. A debugger stops inside
`rebuild_private_settlement_recipient_index_v1`, called by `State::new_inner`.
The call chain has no recursion: inline World owners and State return values
accumulate across the fixture, constructor and validation frames. The failing
test reserves 1,007,240 bytes, `snapshot_state_from_world` 190,472 bytes and
`State::new_inner` 310,696 bytes in the original x86_64 debug harness, before
accounting for the intervening constructor wrappers and validators.

`World` now owns its storage fields through one `Box<WorldData>`. Its default,
populated and snapshot constructors create that owner, and moving World through
State construction and snapshot assembly retains the allocation. The World
handle is pointer-sized; adding another store no longer enlarges every caller
and State return slot. Construction may still use a bounded WorldData temporary;
this change removes the repeated full-World handoffs rather than depending on
compiler placement optimizations.

The sole canonical snapshot schema remains field-for-field in WorldData. World
forwards the schema order and both JSON writer paths to that owner. No alternate
snapshot format, migration decoder, stack-size setting or thread-stack increase
is introduced. Storage predecessor ownership and validation remain mandatory.

The regression tests pin a 2 MiB thread stack independently of `RUST_MIN_STACK`.
They exercise the original approved-pin validation test and production State
construction followed by canonical-string restore, checking the original heap
allocation through construction and current/undo values after restore. Further
checks enforce the compact owner size and the canonical bounded JSON writer.

## Measured stack reservations

The reported executable and repaired candidate have these x86_64 debug prolog
reservations. These are individual frames, not complete runtime high-water marks
or portable ABI guarantees.

| Function | Before, bytes | After, bytes |
| --- | ---: | ---: |
| Reported approved-pin test | 1,007,240 | 185,528 |
| `snapshot_state_from_world` | 190,472 | 131,800 |
| `State::new_inner` | 310,696 | 222,776 |
| `State::try_new_with_chain_and_network_id` | 301,640 | 125,560 |
| `KuraSeed::into_state_from_snapshot_map` | 370,936 | 195,544 |
| `deserialize::build_state` | 233,176 | 145,176 |

## Validation

The harness build command is:

```sh
cargo test -p iroha_core --lib \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run
```

The reported test passes with `RUST_MIN_STACK` unset. A four-thread selection of
339 State tests whose names contain `snapshot`, `world_`,
`transaction_stack_tests`, `account_scope` or `alias_index` passes 336 and fails
three. The selection excludes the new `world_stack_tests` module, exercised
separately. All three failures reproduce with identical error messages in the
original executable: `ConfiguredCatalogBaseline` rejects their pre-genesis lane
catalog. They are:

- `certified_snapshot_corruption_cannot_become_an_empty_lane`
- `set_nexus_recreation_preserves_lineage_across_snapshot_and_accepts_first_merge`
- `state_rehydrates_multi_lane_merge_ledger_from_kura_snapshot`

The initial new JSON regression incorrectly expected complete World bounded
serialization to succeed. Existing storage fields reject that sink; the test now
checks the zero-byte budget and propagation of the underlying writer result.
The production ownership and serializer changes are identical across these
harnesses. Full workspace tests and network qualification were not run.

The final harness passes all four `state::tests::world_stack_tests` cases with
`RUST_MIN_STACK` unset, including both explicit 2 MiB thread cases. Together with
the unchanged production-source selection above, 340 distinct scoped tests pass.
The final regression command was:

```sh
env -u RUST_MIN_STACK target/debug/deps/iroha_core-9c4608cf3dc031e2 \
  state::tests::world_stack_tests --nocapture
```

`cargo fmt --all -- --check`, `scripts/check_no_legacy_codec.sh`,
`git diff --check` and the historical archive verification pass.
