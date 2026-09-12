# Numeric topology compilation boundary (2026-09-10)

`iroha_model_base::topology` now owns `DataSpaceId`, `LaneId`, `ShardId` and
`LaneIdError`, including their numeric validation, binary/JSON codecs, schema
identities and applicable storage keys. Catalogs, lifecycle rules, ledger traits
and `IdBox` composition remain in the aggregate. The superseded aggregate
root, Nexus and events-prelude paths are removed; callers use the base owner.

The 463-path coherent application retains all six original topology tests and
29 aggregate Nexus tests. One additional owner test checks all 21 captured
frames and both storage keys. Three boundary regressions cover full-width
unsigned values, invalid JSON, negative values and overflow through both JSON
object-key and storage-key protocols. The complete 189-row shared fixture is
unchanged, SHA-256
`9eaba83f63302101a16a324d07bd85bd316dfc1cea8eca7bbafb876e0b57342a`.

The consumer migration changes 445 Rust files, preserving the inventories of
47,284 functions, 12,727 tests and 49,910 assertion macros. Review includes
conditional imports, inherited scopes and twelve Core macro definitions with
their invocation scopes. ABI/compiler variants and the unrelated Norito
streaming `DataSpaceId = u64` remain unchanged. The unreferenced Core governance
activation test fragment is migrated without claiming that it executes.

Base adds the existing workspace thiserror dependency; logger replaces its
direct aggregate edge with base; FASTPQ promotes base from test-only to normal.
Logger still reaches the aggregate through node configuration. Root Cargo.lock
changes only the base and logger dependency lists; external package identities,
versions and sources are unchanged. Its SHA-256 is
`beebafe532e7c554ffbefbd5843f99101710f838871bb5b7d21ffc2422b8cc46`.
The canonical OpenAPI lock-pin generator and check pass. Exact dependency budgets
add one declared and one required workspace edge, reaching 1,724 and 1,592,
without added headroom or layer exceptions. The manifest fingerprint is
`sha256:ee91c972f554048bf08bda2a42bba97796721479b34b27548659f624c18fa2ed`.
All 20 feature-resolved normal/build boundary checks pass.

The standalone fuzz workspace adds a relative base dependency. Its initial
offline resolution cannot satisfy the exact flate2 pin from the local cache;
network resolution then succeeds without relaxing manifest constraints. The
555-package graph and ignored local lock are recorded separately. This is
manifest resolution, not a fuzz-target build or execution result.

Nine broad generated-input closures now include base Cargo/source inputs, and
the focused Musubi fixture closure includes the topology owner. Output lists,
generation commands and drift checks are unchanged. Two exact ownership-set
tests receive the same new inputs; all 169 architecture/history/inventory guard
tests then pass. The initial two failures remain recorded.

Evidence is under `target/architecture-redesign/model-base-extraction-v1/`
with the `topology-` prefix. Four base configurations (default, transparent, FFI
and combined) each pass 77 unit tests and three allocation tests. Strict owner
library-and-test Clippy and all three private-field compile-fail examples pass.
Default/transparent runs bind source
`a3c83bff3d2c38e69fa41d41fc26e8697782ad646abd186ee9b3b8994dc91200`;
FFI/combined runs bind source
`43ae55707f8888c6073cd104e9b3b51656ece9057635cd5c07415da40db46649`.
All inputs remain unchanged during those runs.

The final 47-package all-target check passes on unchanged source
`f6252c432c0318ea9a828b32c7bca48f37ec571efa6468e7458cd050377f812a`
with 9,280 selected inputs. Its 150 prior warning occurrences remain unchanged;
the sole new redundant Unix trait import is removed without changing any file
permission assertion. Developer targets and the derive UI harness compile; the
UI compiler subprocess cases have not run.

The composed source
`09d3aff4126085899eeef7b305bc0a58a8e351c4e0269627ef456e371033db78`
passes 77 base, 3,675 aggregate, 802 SDK and 386 Musubi tests on four ordinary
workers. All six moved tests pass at base, every retained aggregate outcome is
unchanged, both captured wire fixtures pass and all 35 resolver regressions pass.
The six ignored aggregate fixture printers and the Musubi crash worker exercised
by its parent retain their existing disposition. All 5,666 source inputs remain
unchanged throughout the build and runtime checks.

The generated-artifact registry passes against a private copy of the Git index
with 31 reviewed additions, inspecting 281 outputs. The real index is unchanged.
This validates registry ownership, not execution of every generator. Historical
archive verification passes. The source budget retains the same 249 affected
paths and 170 exceptions after the GUI navigation extraction; its existing GUI
limit is lowered from 11,200 to 11,129. All 850 environment references and 210
variable owners are unchanged; generated JSON/Markdown line positions refresh.

Strict SDK/Musubi library-and-test Clippy and all ten SDK doctests pass on the
same composed source. Aggregate documentation separately passes seven examples,
and base passes three private-field compile-fail examples, on unchanged source
`092ed364bb92382b51c503313aa054762b8d3c9d79e95f51ce902fdf81000a38`.
Combined FFI/transparent execution passes 77 base and 3,675 aggregate tests,
including the captured fixtures and metadata export/disposal checks, on source
`8a6c89268534339805fb9d37026d7b88f19d256a8cd405f038e4f5ffe136a757`.
All 3,869 selected inputs remain unchanged; the six aggregate fixture printers
remain ignored. Reports are `topology-aggregate-ffi-build-1`,
`topology-composed-base-ffi-runtime-1` and `topology-aggregate-ffi-runtime-1`.

The associated [GUI navigation extraction](mochi-cli-owner.md) passes all 183
rebuilt GUI tests and strict binary/test Clippy on its unchanged source. The
complete 15-stage batch is recorded in `topology-final-checkpoint-1.json`.
The broader aggregate strict-lint diagnostics recorded at the Domain checkpoint
remain unresolved. No build-memory or full release qualification is claimed.
