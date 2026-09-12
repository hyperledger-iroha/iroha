# Model validation and test ownership checkpoint (2026-09-10)

This continues the [Peer boundary checkpoint](model-base-peer.md). It records
applied changes and source-scoped local evidence, not release qualification.
The Musubi resolver remains iterative; its previously qualified 35 regressions
use ordinary worker stacks and unchanged graph/depth limits.

## Applied validation and construction changes

Thirty source paths separate private audit, release, rollout, governance,
lease/checkpoint, VPN and hosting validation into cohesive phases. Ordered
checks, first-error selection, signature verification, checked arithmetic and
wire declarations remain intact. New adversarial tests exercise those phase
boundaries. Contract entrypoint decoding shares scalar leaf handling while its
aggregate traversal remains iterative.

Confidential memo recipients now use one fixed eight-slot array. Explicit
slot_0 through slot_7 JSON keys, closed-object rejection, field ordering and
schema identity remain unchanged. There is no compatibility representation.
`JoinGameSessionV1` and `ExpireSorafsModerationChallenge` are ordinary public
named-field records, independent of the model's transparent_api feature.
Their positional pass-through constructors are removed and all callers migrated.
The original codec derives, field order, decoders and registry identities remain.

The expanded aggregate lib/tests Clippy run exposed 199 test-target diagnostics
on the 26-path validation source; it was a failure. A separate default-feature
production check exposed the moderation record's macro-hidden fields. After
that record and its ten callers changed, the production library passes strict
Clippy. Shipping SDK/storage and Core/Torii/daemon/CLI checks also pass on that
30-path source. Their exact independent source closures are:

| Check | Result | Source SHA-256 |
| --- | --- | --- |
| Aggregate production strict Clippy, run 5 | Pass | `bb292712811854b836e94e30dea5012fcba336674fd3e2ee0a5df8d344ef4727` |
| SDK/storage production, run 5 | Pass | `f035d29143942f30598a1f23ab1cfe03633653f9723e80071604d6b030aaba51` |
| Core/Torii/daemon/CLI production, run 5 | Pass | `d6d3299ccd96cda6c666250a1fcd9b5b3cda366175b6a70a7f928a5a2c34ca8c` |
| Core/executor all targets without test feature, run 5 | Fail: 51 missing gated helper calls | `989f009280d04c9a2a8216c101f520edfaddab33644316713a5343f33428d1e7` |

The last command omitted the existing `iroha-core-tests` feature required by
those integration helpers. A new run explicitly selects that feature for tests;
the shipping configuration remains separate. The unfeatured all-target result
is not a pass. The run-5 pipeline stopped there, before its planned runtime and
default-model-consumer checks.

## Applied test ownership and fixture corrections

A further 150-path composition addresses test implementation rather than changing
protocol fixtures or relaxing lint policy:

- All 1,770 captured codec cases retain their exact type, nominal identity,
  supported direction and encounter order. The 104 assertion-only tests use
  static case inventories; five mixed tests retain each check at its original
  execution point. The shared comparison bodies and fixture bytes are unchanged.
  The preexisting FastpqTransitionBatch row is still checked by its dedicated
  rejection test, explaining the fixture's 1,771 rows.
- The JSON fixture comparison module has one library-test compilation owner.
  Its five consumers share it. Both helper tests remain and run once each,
  removing eight duplicate executions without deleting distinct assertions.
- Fixture integer casts are checked, Copy values are used directly, and concrete
  default types are explicit. The budget rejection test names its expected panic.
  Private test modules own helper visibility; no shipping API is added.
- The manual frame target retains all four original tests and 168 assertions.
  Batch emptiness is compared against every underlying column after retaining
  all row-length assertions. Option branches retain their original evaluation
  order, and both fixture documents are unchanged.

Full workspace formatting, codec-retirement and diff checks pass, and the 150
applied images match the reviewed composition. Source budgets retain exactly
248 findings and 170 exceptions, with no new or changed finding. The real Git
index and Cargo.lock are unchanged. Batch 6 builds the four model/SDK libraries
and passes 81 foundation, 3,720 aggregate, 802 SDK and 386 Musubi tests on the
same source closure
`c915eeddf0c6c29019360966da6615103d1557d257dc2bfd443438feb28c302d`.
All 35 resolver regressions pass on ordinary stacks. The aggregate retains six
ignored fixture printers; Musubi's ignored abrupt-exit worker is exercised by
its parent. SDK/Musubi strict library/test Clippy and documentation pass on that
source. All 11 manual-frame tests and base/model doctests pass. The isolated
consumer builds and executes both public record construction tests with default
model features. Combined FFI runs again pass 81 foundation and 3,720 aggregate
tests. Exact selected source closures for the additional checks are:

| Check, run 6 | Result | Source SHA-256 |
| --- | --- | --- |
| Manual frames, model docs and production strict Clippy | Pass | `ca58315d4949d860b292443b2ba6f0b6b3cd07637d7d5c7e43d4532a2e839e75` |
| Isolated default-feature record consumer, run 2 | Pass | `891810a759f8976eef8fb4b316e605b44705f3d7592c5fa78399e015a5cc1a89` |
| SDK/storage production | Pass | `297da03acce251a8a0480c19eb3801d41b1901b6b9b94e930d613e72ba88953a` |
| Core/Torii/daemon/CLI production | Pass | `4b2d88e1ffd12f13b2d546fe74d5c4558d1e792e6304101e8fdc2fc996d89fba` |
| Combined model FFI runtime | Pass | `e523a458c17cf3d80bc0efe044bf55ce0b60c5ba34cf44daeb0832439c538e39` |
| Core/executor all targets with iroha-core-tests | Pass | `d8e88ef7abbce07e61223635ac3bcfbc41e166f7e3b1a9657090407b9bb2ca44` |

The test-feature check compiles consumers; it does not execute Core integration
or consensus scenarios. Batch 6 completed 21 checks: 20 pass and aggregate strict
library/test Clippy fails with 96 diagnostics, reduced from 199. Its exact source
is `ca58315d4949d860b292443b2ba6f0b6b3cd07637d7d5c7e43d4532a2e839e75`.
The remainder includes long test builders, borrowed helpers, test item ownership
and one newly exposed test-module glob collision. Corrections are staged
separately; the aggregate strict library/test gate remains open.

## Further test and source ownership

The next applied composition changes 41 paths, including three new files.
Borrowed codec helpers preserve concrete type inference and signed fixture
bytes. Test declarations move to their owning scopes. Musubi and foundational
fixture construction use cohesive builders; their original comparison suites
remain. Another 164 literal codec captures use ordered static cases, retaining
the feature-gated GovernanceEventFilter entry. A Policy Jury fixture owns its
internally bound ballot/body fields. The private settlement test module is
visible only to its Nexus parent, preventing a test namespace glob collision.

Consensus context tests now have an ordinary parent module and a dedicated
context-validation child; the parent drops below 3,000 lines. The exact reviewed
Rust-component map and checker component list have one canonical Python owner.
Both the proof checker and AST reader consume it, and release fixtures retain
that owner. No former-owner fallback or digest bypass is added. The source-seal
checker drops from 5,442 to 4,826 lines. Existing oversized checker and receipt
test files also shrink. Source-budget findings fall from 248 to 246 with all
170 exceptions unchanged and no new finding. Formatting, codec and diff guards
pass; all formatted images match their reviewed composition.

Batch 7 passes 3,718 selected model tests and 183 generated-fixture group tests
on source `846b29d4a1fa7d3dea0140b98993b21dc6606b58c5c8e82fd4a082c5d071795f`.
The group retains five ignored fixtures and exercises its two isolated child
cases. Compared with batch 6, the context split adds two tests while this
model-only graph lacks four tests behind SDK-selected HTTP/test-fixtures
features. The exact cohort delta is recorded; no assertions were removed.
The aggregate strict library/test check fails with 47 diagnostics, down from
96. Batch 7 completes with five passing checks and that explicit lint failure.
The FFI configuration passes 3,722 model tests, retaining six ignored fixture
printers, on source
`2075e3d9c299b8555000b121191d42fbe6b6455493cabd02384537fb6a3880db`.
Its retained executable SHA-256 is
`cf508818a9f9891a6417c6d233fad156af31044f0961b91a4c3bf38e5f238a07`.
This checks Rust FFI features; it does not qualify rebuilt JNI/device delivery.

The initial broader Python run reports 249 failed, 80 passed. Of these failures,
248 arise while creating outdated synthetic scaling evidence before changed
component handling; one concerns Darwin source-to-derived extension relocation.
That failing run and its original log remain retained.

The six-file correction gives validator and receipt tests one complete-cohort
fixture builder. Every synthetic offer has an explicit acceptance or rejection;
accepted identities have globally resolved Applied records, with warmup and
bounded drain accounting. The receipt envelope still invokes the actual
production validator and retains its source pins. Darwin archive assertions now
bind every declared transformation to its authenticated input, derived record
and actual output bytes; unchanged files retain their byte-equality checks.
Production validation and all original negative assertions remain unchanged.

The combined live-source run passes **388 tests and 86 subtests in 547.14s**,
including receipt, component, source-inventory, reviewed-Rust-source and scaling
validator suites. All 970 scoped Python/budget inputs match before and after;
their sorted compact JSON inventory has SHA-256
`fc1aed9232a329e58b197113e7411984f4ab57e7ef05f2a38e72f5c56ca9c620`.
The command, input maps and log are retained in
`receipt-python-repairs-application-v1/combined-pytest.json` and
`combined-pytest.log` under the evidence directory below. The log retains pytest
cleanup warnings for older protected temporary runtime directories. This is
synthetic fixture qualification, not live scaling evidence or a release seal.

The receipt test shrinks from 5,238 to 4,969 lines; its existing 5,048-line
exception is ratcheted to 4,969. It still needs decomposition to meet the
3,000-line test limit. Source-budget findings fall from 246 to 245, retaining
all 170 exceptions with no new or changed finding. At that checkpoint, aggregate
strict Rust test lint still had 47 diagnostics; workspace qualification remained open.

## Closed model test lint and file-size budgets

The final 22-path Rust composition, including three new test modules, gives
retail enrollment one fixture owner, borrows large fixture inputs, separates
governance/certificate checks into coherent phases, and shares typed Soracloud
JSON assertions. Ordered negative cases, original seeds, signed bytes, literal
artifact-role arrays and schema identities remain. The query visitor's exhaustive
variant check is a compile-time test assertion. No lint allowance is added.

The SCCP registry now has a 3,597-line production owner and a 2,249-line ordinary
test child. Soracloud schema assertions and runtime-state validation have their
own ordinary test modules. The two previously oversized test owners drop to
2,442 and 2,808 lines; their new children contain 883 and 1,676 lines. No type or
schema declaration moves in that split. Every aggregate model source file now
meets the 5,000/3,000-line defaults without exceptions. The repository-wide gate
still fails with 242 findings elsewhere, down from 245, and all 170 exceptions
are unchanged. No new or changed finding remains.

Run 8 clears the original 47 diagnostics but fails with eight formatting lints
in extracted assertion helpers. Exact same-message corrections produce a clean
run 9, passing default model and fixture-group suites. The final module split
passes default lint and model, fixture-group and FFI runtimes in run 10, while its expanded FFI lint
check exposes three implicit raw-pointer conversions in existing handle-rejection
tests. Those conversions now explicitly take the same raw addresses; IDs, null
inputs and all rejection/output-preservation assertions remain unchanged. Run 11
qualifies the complete composition:

| Final check | Result | Selected source SHA-256 |
| --- | --- | --- |
| Default library/test strict Clippy, model and fixture-group build/runtime | 3,718 model and 183 group tests pass; zero lint warnings | `caf5a0de0a739a55645b048269e391a8e3f1ad80d01b8e54cee2598cd1d36b5e` |
| FFI library/test strict Clippy and model build/runtime | 3,722 tests pass; zero lint warnings | `125bbf0340a34fb51da2e6b285422415c98a76907106591d9ee0d74af3a3ce2d` |

The default and FFI suites each retain six ignored fixture printers. The group
retains five ignored cases and executes both isolated child cases. An exact
executed-name/outcome comparison accounts for all 76 moved tests in both
configurations; there are no additions, omissions or changed outcomes outside
that owner rename. The comparison covers 3,724 default and 3,728 FFI names,
including ignored tests. Static reviews additionally preserve assertion bodies,
fixture fields and order.

Workspace Rust formatting, codec-retirement and diff checks pass. The focused
source inventory, reviewed-Rust-source and budget regression suites pass all
76 tests on the final source. Cargo.lock and the real Git index remain unchanged.
Current resolver source is unchanged from its ordinary-stack qualification. These checks establish
neither the required measured memory reduction nor full workspace, native/JNI,
device or four-validator release qualification.

The evidence directory below retains `peer-composed-batch-9.json`, all seven
run-11 command reports, `peer-test-cohort-comparison-11.json`, and
`peer-source-budget-comparison-11.json`. Source transformations and independent
reviews live in `aggregate-test-completion-v1`, `root-binding-fixture-review-v1`,
`soracloud-test-contracts-v1`, `soracloud-test-format-captures-v1` and
`soracloud-test-ownership-v1` and `metadata-ffi-pointer-clarity-v1`; the final
five-path application has its own record.

## Evidence ownership and limitations

Evidence lives under `target/architecture-redesign/model-base-extraction-v1/`:
`aggregate-validation-phases-application-v1`, `aggregate-hosting-application-v1`,
`aggregate-moderation-expiry-construction-v1` and
`aggregate-test-contracts-application-v1` preserve exact before/after source and
patches. Independent moderation and schema-inventory reviews bind their inputs.
Every qualification command retains its source seal, compiler artifacts,
stdout/stderr and explicit result. The qualification driver clears ambient
stack-size overrides.

The run-5 driver accidentally wrote its rolling summary to the run-3 summary
path. Only that summary was affected. All individual reports and artifacts were
intact; run 5 was preserved under its correct name, and the run-3 summary was
reconstructed in the original order from its 14 hashed command reports.
`peer-batch-summary-routing-correction.json` records that correction. The executed
run-5 driver is retained unchanged; run 6 has its own output path.

The borrowed Norito context proposal remains unapplied. Its standalone lifetime
and trait tests do not qualify codec, derive, account-format or MV integration.
Measured model memory reduction, unchanged-unit budgets, full workspace checks,
consensus harnesses, four-validator execution and native/device delivery remain
separate open release obligations.
