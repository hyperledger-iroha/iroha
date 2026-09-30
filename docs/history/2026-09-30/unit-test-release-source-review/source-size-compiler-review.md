# Source-size retirement and compiler ownership review

The last successfully tested selected-source seal was
`db3d6515f79a5183000d3c434b0cb219682c7983f5d0257eff4ebe8df0d4c674`.
The later optional-tool inventory captured an extra historical Cargo manifest;
its unqualified intermediate seal is superseded here, as recorded in
`optional-tools-inventory-correction.md`. The corrected candidate selects 7,021
inputs. `source-size-compiler-preimages.json` preserves all 65 changed prior
inputs byte-for-byte and every surviving candidate input. Historical manifests
are stored with `.txt` suffixes and cannot create Cargo packages.

The user's removal of source-size policy retires the generic line-count gate,
baseline, gate-only unit suite and PR/release calls. The remaining source guards
retain case inventories, semantic token identities, ownership, transcript order,
authority refusal and allocation checks. Build-efficiency provenance schema 4
still authenticates the five original Git roles, 14 selected historical inputs,
protected integration source and commit lockfile; historical source budgets no
longer constrain candidate code. No runtime or wire byte bound is retired.
The Rust assertion preservation ledger separately records each removed source
line/byte assertion, including helper calls whose sole purpose was code size.
All 28 retained selected Rust semantic and lexical-authority controls pass.
The test-only scanner borrows source text and bounds stored tokens and nesting;
it accepts comments and whitespace larger than its retired input-byte cap.

The compiler dependency moves from IVM production into its actual owners. CLI
owns its direct compiler dependency; Core and integration tests own development
dependencies; Torii's app_api feature owns its optional production compiler.
IVM tests retain a development dependency, and the compiler facade is removed
instead of retaining compatibility exports. The workspace adds references to
the existing compiler surface/toolchain packages. Current Cargo unit graphs
confirm the required compiler extern for app_api/test-network consumers and its
absence from IVM production. The exact reviewed dependency budget adds only the
observed ownership edges; shipping node units decrease. Its 46 refusal controls
pass. Current IVM library tests pass 1,014/1,014.

The already reviewed optional-tool changes preserve complete HTTP deadlines,
native custody, exact subprocess fixture assertions, canonical Kotlin Log
encoding and canonical Android consumer builds. Taira's 104 parallel unit tests
pass. Canonical privacy lock anchors follow the maintained Cargo.lock and retain
prior originals. The panic inventory review retains all 973 source boundaries
and unchanged category counts while accounting for compiler path ownership and
test/connect gating; all 107 panic guard tests pass.

FASTPQ's source guard now audits the maintained exhaustive 4/8/16 FriValues
arity-byte writer. It retains canonical proof/frame limits and malformed-arity
refusals, with new changed-tag and changed-writer controls. The complete FASTPQ
and telemetry helper directories pass 608 tests and 14 subtests. The SoraFS
readiness unit fixtures invoke the production signature verifier on every
verification; the former test-only signature-verdict cache and module verifier
override are removed. A bounded immutable public-key memo remains. Actual
verifier identity and repeated valid/changed-message controls pass.

The selected-source fingerprint authenticates this reviewed development
candidate only. Full workspace, standalone/mixed Rust targets, current native
SDK artifacts and complete script qualification remain independent. This
record makes no release, deployment, network or settlement readiness claim.
