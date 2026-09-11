# Name and StatePath compilation boundary (2026-09-10)

`iroha_model_base` owns `Name`, `StatePath` and `ParseError`, their complete
validation and normalization, codec/schema contracts, FFI implementations and
JSON/storage keys. Retired aggregate modules, root/prelude exports and consumer
imports are removed. The explicit captured wire identities remain protocol
declarations; they are not Rust API aliases. Remaining foundational owners,
including Metadata and account identities, are still pending.

The model's optimization setting is unchanged. Cargo.lock adds one local package
and updates 31 existing local dependency lists, with no external package/version
changes. The exact dependency budgets account for the owner split without added
headroom. Four feature-resolved base selections forbid normal/build paths into
aggregate models, node/runtime services, transports and proof execution.

The retained development records are under
`target/architecture-redesign/model-base-extraction-v1/`. Their checks have
separate source fingerprints and do not constitute release qualification:

| Check | Evidence and scope |
| --- | --- |
| Base owner | All 35 moved tests pass for default, transparent, FFI and combined FFI/transparent selections. The corrected FFI source seals include the optional FFI crates. Strict base library Clippy passes. |
| Wire and schema | Both mixed-owner fixture tests pass again after final formatting, preserving all 189 captured frames, schema/JSON/key records and signing assertions. `aggregate-final-wire-1` uses the same source and executable as the complete model run below. |
| Aggregate model | `aggregate-final-build-1` and `aggregate-final-runtime-1` pass all 3,705 library tests with zero failures and six existing ignored fixture-regeneration printers. All 3,775 selected inputs remain unchanged: source `a7f79976005f259656da3228d16de3cb34d5e128bd6d4cac96fe6abb40afee36`, executable `4ee220094dbd7c3929da0d5c3c18bc493fef16354f2d5f84d84b5f0d9614cfd2`. This development selection includes the transparent API through dev dependencies; it does not qualify the separate FFI feature matrix. |
| Consumers | `consumers-final-check-1` passes all-target compilation for 34 packages, including SDK/CLI, Core/Torii/daemon, model/schema/executor, storage, compiler and native bridge consumers. All 9,080 selected inputs remain unchanged: source `2c2e1b6d6425e709d0fea8be44dd884c761b041a35257e6360da4249a1ff823e`. Compiler warnings remain; this is not strict workspace Clippy or runtime execution. |
| Musubi | The rebuilt consumer passes 386 library tests, including 35 resolver regressions on four ordinary-stack workers. The one ignored abrupt-exit worker is exercised by its parent. The [resolver checkpoint](musubi-sdk-and-resolver.md) records exact source and executable identities. |
| Architecture and CI | The recorded checks pass 20 resolved boundaries, 46 dependency guard tests, 88 CI routing tests and lane validation for 102 packages/seven lanes. The source-size guard retains 249 findings and 171 exceptions; none were expanded. |

Earlier consumer failures remain recorded. They exposed missing direct imports
in inherited test scopes and SDK/executor facades, then Core/Torii/CLI owners.
The final repairs change only imports and formatting; independent review retains
all non-import tokens. An intermediate check exhausted disk space and did not
produce a valid success report. Obsolete libraries from this task's inactive
cache were removed with an inventory; retained evidence and executables remain.
The preceding successful `consumers-check-4` retains its source
`20fc632413a21074dd3549b7b8897a328cde9b912df93cdc224fcbbdff2eb149`; the final
check includes the subsequent test-fixture ownership correction.

Remaining aggregate feature checks, remaining model owners,
workspace/native/device execution, mandatory four-validator scenarios and the
pinned baseline/candidate memory comparison remain outstanding. No 25% peak-memory
reduction or release ceiling is claimed from these development checks.
