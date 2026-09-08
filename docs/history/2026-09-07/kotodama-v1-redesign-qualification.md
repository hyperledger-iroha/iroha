# Historical Kotodama V1 qualification checkpoints

These checkpoints predate resumed qualification. Temporary evidence paths are
unavailable and do not qualify current source or release readiness. Current
outcomes are tracked in [the implementation ledger](../../../specs/kotodama_v1_redesign.md).

The following original section was moved byte-for-byte from
`specs/kotodama_v1_redesign.md`; its SHA-256 is
`2794bd47133c4d13e29a008d5c9896aa5394743612927b1988c56449d7f67ebe`.

## Historical local validation checkpoints

The following counts and paths record earlier work. Missing execution records
must be recovered or replaced by fresh evidence before they can support closure.

The final semantic ABI descriptor has 221 syscalls and hash
`db4259aa28486967f2d8799b4e66e1b919f4d18690db2d94cd96efa05ee840a3`.
It binds explicit Unit return schemas, nominal errors, qualified exported type
names, bounded state scanning, fused arithmetic, and tagged Option JSON.
Public arguments, returns and state JSON preserve exact one-key sum objects.
Native JSON supports tagged Option and rejects implicit Result values; the caller
handles the Result before constructing JSON. There is no nullable Option path.

The language and tooling implementation has reached final artifact generation
and installed verification. The independent aggregate Result audit is repaired:
the fresh complete compiler suite passes 1,085 tests, and artifact regeneration
and installed checks pass. Fresh runtime and affected Core selections also pass.
Remaining qualification covers full Swift, final native resealing,
four-validator execution and the separately classified workspace failures.
Counts below are scoped
local checkpoints, not a claim that the complete workspace or release has passed.

| Area | Latest evidence | Remaining qualification |
| --- | --- | --- |
| Compiler and semantic editor | Complete compiler library 1,085/1,085 and fresh koto binary suite 37/37 pass after the aggregate Result repair. Fresh normal binaries pass all 23 actual-CLI Result probes and regenerate the verified artifacts. | Combined consumers and workspace validation. |
| ABI and shared model | Final-hash ABI library 172/172, AXT vectors 6/6 and official exporter checks pass. Installed final artifacts pass admission 17/17 library and 15/15 artifact tests. Model smart-contract filter 56/56 and separate nominal nested-rejection roundtrip pass at the preceding checkpoint. | Workspace consumers. |
| VM and arithmetic | All eight executable redesign cases pass again against the installed final artifacts. Earlier state 90/90, grouped compiler/VM 147/147, arithmetic 13/13 and numeric syscall 22/22 selections pass. Final native tagged JSON passes 24 library and three executable cases. | Four-validator execution. |
| Core | Typed Unit/error nested returns, exact abort mapping and stored-product nested calls pass. The fresh affected codec/contract filter passes all 47 cases after exact authorization/verifier fixture repairs, including the denial cases. | Network execution and broader workspace qualification. |
| Torii | Final contract-state suite 36/36 and exact NFT field-filter regression 1/1 pass; fresh daemon builds successfully. | Production route integration and workspace consumers. |
| CLI and onboarding | Four scaffold and four debug-call tests pass. Fresh frozen final binaries pass all 18 offline checks and three generated tests with unchanged binary hashes. The exact network probe and tutorial initialize/add/read sequence pass locally on the final hash. | Four-validator network harness and execution. |
| SDKs | Installed final artifacts pass Python manifest tests 220/220 and JavaScript artifact/schema tests 12/12. Fresh native Kotlin: 1,414 pass, eight executed-HEAD fixture failures, zero skips. Fresh native JavaScript: 2,856 pass, the same 22 separately classified failures, zero skips; four direct Result-checker controls pass. Focused Kotlin/Java, C#, Python, JavaScript and extracted Swift schema/identity checks pass. All five Apple targets, complete-archive links, host crypto and official package checks pass. | Full Swift is being requalified after a narrow SCCP compile repair and the required warm artifact reseal. Final ledger edits require official JavaScript resealing with unchanged native-binary verification. |
| Editor package and documentation | Client 3/3; VSIX packages 392 files. Complete authoring/deployment/call tutorial and all 20 translations pass content/i18n/provenance, production build and built-link checks. Exact local commands and the SDK-generated trusted call intent are verified. | Network execution remains a separate required gate. |
| Source assets and guards | 302 fixtures/589 extracted-source tests; 193 IR variants/101 tests/54 assets; Python IR+syntax 19/19; all 18 generated syntax files current. Workspace format, retired-codec, diff and history/current-view guards pass. | Workspace build/test/lint. |

### Behavior evidence

The final read-only acceptance crosswalk maps 34 requirements to 93 distinct
evidence records, including exact test names, source locations/hashes, inspected
assertions and pass-log locations:
`/tmp/kotodama-result-acceptance-crosswalk.json`. It found no additional uncovered
compiler/VM/Core/ABI/editor/onboarding behavior. Earlier isolated checkpoints and
fresh combined runs remain distinguished; this crosswalk does not replace the
pending native SDK and four-validator execution gates.

The repaired must-use analysis tracks Result-bearing aggregate fields, retaining
obligations when unrelated fields or collection metadata are read. Seventeen
focused tests cover implicit aggregate loss, temporary projections, explicit
pattern discards, whole-value transfers and equality, nested error projections,
source-order mutation, shared list aliases, alias overwrites, branch/block tails,
loop exits and newly appended elements beyond a captured traversal length. Valid
source identifiers cannot claim the compiler-owned binding exemption. Symbolic
slot masks and bounded conservative joins avoid enumerating nested capacities or
branch combinations; 24 nested lists, 32 sequential joins and oversized indices
have regressions. The complete 1,085-test compiler run passes in 16.60 seconds in
`/tmp/kotodama-result-final-compiler-lib-v2.log`. A fresh frozen CLI passes 23
before/after probes: 16 exact `E_RESULT_MUST_USE` rejections and seven valid
controls. Fifteen previously accepted bypasses now reject; all binary hashes
remain unchanged during the probes. Reports are under
`/private/tmp/kotodama-result-use-acceptance-05pk3of0`. This compares the preceding
and repaired frozen binaries, not an executed HEAD baseline. Artifact source seals
have now been regenerated; fresh native qualification remains required.

The compiler and runtime acceptance cases cover declaration-only call labels,
source-order evaluation of reordered named arguments, named struct destructuring
with one initializer evaluation, composable Unit and nominal errors, checked-list
transaction rollback, fallible-list preservation, and unconsumed Result rejection.
The full native test driver passes 44 tests: exact error identity/schema/code,
unexpected success, wrong-stage authorization/argument failures, deliberate
catch-all rejection, and state/caller restoration. Arbitrary authorization names
require their exact custom tokens; zero-parameter calls require an empty argument
object. Fixture shorthand does not weaken either rule.

Runtime pagination tests cover 192-entry pages `[64,64,64,0]`, a 200-entry combined
probe, deleted cursor keys, inter-page insertions before and after the cursor,
64 tombstones, wrong instance/map/schema/key kinds and gas exhaustion before
publication. Core's merge tests verify direct range seeks and propagation of the
64th-candidate stop through backing state and overlays. Independent source review
confirms the query-state adapter also seeks directly and does not count or perform
an exhaustion probe. The four-validator probe will exercise this adapter with
persisted state and restart recovery.

Fused arithmetic uses the existing mathematical intermediate and rounds once.
Tests compare folded and executable results across all seven rounding modes,
negative values, ties, repeating quotients and large intermediates with
representable results. All 54 shipping numeric operations are covered by the
22-test syscall suite. Financial samples declare precision and rounding policy;
DEX input fees and final payouts round down at their respective precisions.
Named bounded-list transfer batches also have executable coverage for full
source-order argument evaluation, active length, empty batches, sequential funding
and atomic rollback after a later failure.

The shared schema checks cover exact `package::SourceUnit::Struct` identities,
including public arguments, returns, state, imported aliases and package-version
separation. Shared SDK vectors contain seven valid and eighteen invalid qualified
names. Unit, error variants and cursors have explicit schemas and canonical wire
representations. Torii's complete state selection now passes all 36 cases,
including public argument/state projection roundtrips, nested Unit/Option/error
products, typed cursor pages, wrong identity/schema/code/key rejection and strict
path/depth/response bounds. Negative JSON fixtures use canonical lexical ordering
so they reach the intended typed check. The unchanged HEAD cardinality test now
inspects the exact typed conversion cause and retains its 10,000-entry positive
boundary. The separately selected NFT field-filter regression also passes.

Native JSON tests distinguish Some(Unit), None and every nested Option state,
including lists, malformed handles, exact object-member/output-byte gas and real
public argument encode/validate/materialize/rebuild roundtrips. Iterative conversion
and cleanup handle 255 nested Options on a small stack. Serialization preserves
Norito's existing depth limit of 33 and rejects deeper output. SDK HTTP/payload
checks preserve the tags without interpreting schema-free JSON as a nullable
Option: Kotlin one test/four cases, Java four cases, C# four cases, Python five
cases and JavaScript five cases pass. Swift execution awaits its native bridge.

Editor tests cover Japanese/emoji UTF-16 positions, incomplete source, scopes,
receiver types, imported error completion, argument snippets, cancellation and
stale buffers. Fourteen diagnostic tests cover real caret underlines, Japanese
cell widths, tabs, combining marks, controls, clipped excerpts and related source
locations. Rename includes nominal enum prefixes in expressions, match and if-let
patterns. It produces one versioned source/export edit set, uses exact Norito
JSON token ranges, rechecks the rewritten graph, preserves resolver identities
against capture and permits names in disjoint scopes. Unopened files are checked
against their captured contents; unsaved manifests carry their buffer versions;
external graphs without local manifest authority cannot be edited. The client
synchronizes only its configured manifest without taking over JSON providers.

### Local workflow and artifact qualification

The offline project workflow covers new/check/schema/locked build, three generated
tests, format, compiler check/build/verify, source documentation, initialization,
persisted-state readback and overwrite rejection. It creates no signing credentials
or network effects. Local debug-call accepts Kotoage, Hajimari and Kaizen; debug-view
has a separate selector. The executable lifecycle regression verifies initializer
7 → upgrade 8 → readback 8, explicit Unit returns and exact write counts.

The final-hash local network probe uses the exact source embedded in the integration
harness, SHA256 `4ec3701e721bde50e19f71e37ba27fe711f99ce96ef90de0e6ac49934970dfda`.
Initialization uses 4,299 gas, verification 3,884,297 gas with 201 writes, and
readback 3,028,172 gas with no writes and result 7. It persists 205 entries across
separate CLI processes. The network test uses a measured 5,000,000 allowance;
the 1,500,000 attempt fails at bounded-scan precharge as expected. These are local
measurements, not network qualification.

Fresh normal binaries include the final editor, tagged Option and aggregate Result
changes. Their
owner-only frozen copies are in
`/private/tmp/kotodama-result-final-binaries-wadt8td5`, with exact build/copy evidence
in `built-binaries.json`. The koto SHA256 is
`073e81beebc041f13cb1b2dc3b7b7042545f85a702494f10deaf257c18cd92a9`;
iroha is `28c98000aefb67ada22953adfa95df6d150cc6cbdaa77d606fb2a0c5a2aa94b0`.
The final eighteen offline checks and three generated tests pass using these
copies; their hashes remain unchanged. Detailed outputs are in
`/private/tmp/kotodama-onboarding-l9lv2_7k`.

The official generator independently builds and compares all 56 compiler artifact
mappings and four demo manifests, publishes an exact 60-file inventory, and checks
the installed tree with zero drift. The final publication is
`/private/tmp/kotodama-result-final-artifacts-k9zs1qty/goldens`; its
`.kotodama-v1-owner-manifest.json` SHA256 is
`e018b1eee5d68a601d63c26f9e9d50b46c343d7c17359239c4fb87c5b263e7d1`.
All 59 repository IVM images, including the separate executor/predecoder outputs,
carry the final db4259 ABI. The 73 other `.to` files are non-IVM records.

The installed `javascript/iroha_js/test/fixtures/current_rust_contract_artifact.json`
passes official generation and installed verification. Its artifact SHA256 is
`6f420bcc858b85ccd95a313bbd3f10d9abc255cbd8342677fc17bb83bbc1e742`;
the whole fixture SHA256 is
`e35809a15a4a5e1f71a8b09ab21e31f526d9fc55f09d0af80fbf2080b30a0891`.
Its 2,512-file semantic source closure hashes to
`289ccd85da68adffb903b91dd97e2298b8f19790bb6085b62d4a3e18a5460f44`.
Under `/private/tmp/kotodama-result-final-artifacts-k9zs1qty`, generation attestation
lives in `current-rust-contract-artifact.work.5e9d7ed80896cacbb659b965/` and
installed-check attestation in `current-rust-contract-artifact.work.8cf02e6c0e4906225511d4d1/`, each
named `generation-attestation.json`. Final installed-artifact consumer checks pass
220 Python and 12 JavaScript tests. Fresh admission 17-library/15-artifact and
all eight redesigned runtime cases pass against these installed artifacts in
`/tmp/kotodama-result-final-admission.log` and
`/tmp/kotodama-result-final-runtime.log`.
Native Apple builds began after installation and source-attestation verification;
full native SDK execution remains pending.

Captured final regeneration logs use `/tmp/kotodama-result-final-` with the suffixes
`goldens-publication.log`, `goldens-check.log`, `current-rust-generation.log`,
`current-rust-check.log`, `python-manifests.log`, `js-manifests.log` and
`onboarding.log`. Torii's complete passing selections are recorded in
`/tmp/kotodama-final-torii-state-closure.log` and
`/tmp/kotodama-final-torii-nft-filter.log`. These are local execution records;
they do not close C9 or attest a signed release candidate.

The complete public tutorial's validated source hash is
`a1aa24be8e90c3ceccd1507f35876c3130b00a190d6f37df861988aaf854de79`.
All 21 pages preserve identical code fences; eight shell blocks pass syntax
checks. Pinned Node 24/pnpm 9 validation, production build and link checks pass.
The exact local initialize/add/read commands execute successfully. The public SDK
helper verifies the local artifact, emits a canonical 180-byte argument record for
`{"amount":"3"}`, rejects numeric `3`, and produces an intent accepted by the
current Rust CLI. Its deliberately unavailable loopback endpoint proves parsing,
not network deployment. The tutorial uses the existing deployment utility and
explicit permission grant, and keeps the project outside the SDK source checkout.
The deployment prerequisites now name both code registration and management of
the exact `counter@universal` account-alias scope for contract alias
`counter::universal`. English and all twenty translations pass fresh content,
provenance, production-build, link and rendered-locale checks. The final guide also
grants the deployed address's exact `hajimari` invocation permission before calling
the hook; the existing `add` grant remains unchanged. All 21 translated command
sequences, code fences, hashes and rendered directions pass; the production build
takes 96.12 seconds with zero broken links. Evidence is
`/private/tmp/kotodama-docs-hajimari-grant-evidence.json`. All eight final shell
blocks pass parse-only checks in `/tmp/kotodama-final-docs-hajimari-shell-check.json`.
Iroha's build/test graph does not depend on the optional sibling documentation.

### Existing validation failures and narrow repairs

The fresh affected codec/contract run is
`/tmp/kotodama-result-final-core-affected.log`: **47 pass, zero fail** in 15.77
seconds. Exact test-name comparison confirms the same selection as the two earlier
42-pass/five-fail checkpoints. All five formerly failing tests, under
`smartcontracts::ivm::host::tests`, now pass:

- `call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts`
- `contract_call_transaction_resolve_account_alias_builtin_uses_current_binding`
- `contract_call_transaction_domain_qualified_resolve_account_alias_builtin_uses_current_binding`
- `contract_call_transaction_domain_qualified_resolve_account_alias_builtin_accepts_domain_permission_without_dataspace_permission`
- `from_state_hydrates_zk_snapshots`

The first four reach the exact asset-source authorization gate in
`smartcontracts/isi/asset.rs`; compilation, alias resolution and ABI decoding have
already succeeded. Contract effects execute under the queued contract subject,
but these fixtures grant only named `AssetOps` entry authorization. Their source
asset belongs to the transaction caller. The minimal setup correction is an exact
`CanTransferAsset` grant to the payout contract for that asset; the nested-transfer
case additionally needs the callee's exact grant for the caller contract's asset.
The fixtures now grant those exact source-asset permissions. The nested case also
asserts the queued authority sequence explicitly; balance, ordering and
alias-permission denial assertions remain intact. The complete focused rerun passes.

The fifth fixture supplies `[7; 32]` as the Halo2 `ivm-execution-v1` schema hash.
After the authenticated-ledger-time fixture repair, snapshot hydration reaches
the existing strict verifier and rejects that value as `ZkSnapshot(NoritoInvalid)`.
The fixture now uses `crate::zk::ivm_execution_public_inputs_schema_hash()`;
the keyless-record and snapshot assertions remain applicable. Its focused rerun
passes.

Source comparison confirms that the implicated transfer authority/queue gates,
permission setup helpers, verifier gate and fake schema value match HEAD. The
alias catalog change only adds the required `alias:` label; the hydration fixture
only adds authenticated ledger time. These are high-confidence setup mismatches,
not demonstrated introduced production regressions. **HEAD was not executed**.
The narrow fixture corrections were applied before the new native source freeze.
The exact before/after case comparison is recorded in
`/tmp/kotodama-result-final-core-comparison.json`. These 47 passing cases do not
establish a full Core pass.

The broad Core IVM checkpoint executes 382 cases: 330 pass and 52 fail; a previously
executed expensive STARK self-verification failure was excluded. Subsequent scoped
repairs explicitly pass 22 of those 52 formerly failing cases in the latest
affected filter, including Unit/error stored-product nested returns, state
isolation/overlays and managed-state protections. The earlier broad run already
passes the typed StateMap scan and bounded seek/merge cases. No fresh complete Core
run is claimed. The initial invalid
snapshot depth default (128 against Norito's maximum 33) is fixed using the
canonical limit; all three configuration boundary/default tests pass. Core fixtures
now supply authenticated committed-header time where required. New Unit fixture
widths and lifecycle initialization are corrected without weakening runtime policy.

Source comparison with HEAD confirms older test/API mismatches in execution-summary,
Merkle quote, four Native AMX files, CLI audit, query/transfer fixtures and the Torii
account-filter test. Narrow corrections use current APIs and preserve authorization.
The Native AMX repair passes six participant-role, one signing-guard, one route-claim
and two retirement/archive cases; CLI audit passes thirteen tests. The account-filter
test now uses the generic helper's actually supported NFT identifier type.

Seven remaining query failures omit authority registration or CanReadAllLedgerData;
seven transfer fixtures lack exact contract-subject spending permission. Their
implicated gates/setup match HEAD. Other failures involve noncanonical verifier
schema hashes, a missing AXT touch, a FastPQ output cap, SM3 metric staging and
STARK self-verification. Source comparison plus current logs is not a separately
executed HEAD baseline; the STARK cause remains unproven. None of these policies
has been relaxed to make the tests pass.

The complete Kotlin checkpoint runs 1,421 tests: 1,365 pass and 56 fail. Forty-two
require current native/JNI exports and six require the explicitly configured
fixture-generator executable; its contract manifest and Java consumer tests pass.
The other eight have an executed narrow HEAD baseline: 28 unchanged source/test
files and two unchanged fixtures run 13 original JVM tests, with five passes and
the same eight failures. Six ownership tests expect three TSV columns where the
canonical fixture has four; two privacy tests encounter an unchanged stale KAT
digest. This is not a full HEAD/native baseline. Evidence is under
`/private/tmp/kotodama-kotlin-head-eight-1txs3ll5`. The fresh complete Kotlin run
passes 1,414 of 1,422 tests with zero skips; only those same eight fail. All 42
native/JNI and six fixture-generator failures are resolved. Complete reports and
before/after hashes are under `/private/tmp/kotodama-kotlin-fresh-native-_v_7jbxe`.
The ABI-23 dylib SHA256 is
`81b16d54c797aaefa74d577ae1036c13d0c2dbc11f4e251413aec219f40ce50d`; the frozen
fixture generator is `53bbea14f55ed40b6f52933a0461698f774b52658d354dcb1a0d16bf774b09a0`.
This is local JNI-backed validation, not the separate clean-source release
attestation. Full Swift initially encounters an ABI-21 native bridge where
ABI 23 is required. The fresh Apple build reaches its mandatory complete-archive
link guard, which rejects duplicate PQClean objects from the locked dependency's
duplicate link directives. The packager now removes only second copies whose names
and complete payloads match the exact Cargo-produced common archives. Conflicting
or unknown duplicates reject; the source seal covers the helper. The normalizer
and source-seal selections pass 55 tests plus six subtests, and the actual failing
archive passes its unchanged `-all_load` link check.
The final host Rust build passes in 35m07s. Packaging also preserves `ranlib`
invocation mode when the authenticated Xcode executable resolves to `libtool`.
The focused suites pass 58 tests plus six subtests, including two byte-identical
index rebuilds through the actual Xcode tool. The official warm host retry passes
complete-archive linking and ABI/SHA3/SHAKE/ML-DSA/ML-KEM runtime checks. The iOS
device target then passes in 29m03s and the arm64 simulator in 57m40s, each with
seven authenticated duplicate removals and a passing complete-archive C link.
The x86_64 iOS simulator then passes in 58m14s, with six authenticated duplicate
removals and a passing complete-archive C link. The final x86_64 macOS target passes
in 59m35s, again with six authenticated duplicate removals and a passing complete
C link. The official run is recorded in
`/tmp/kotodama-final-apple-native-ranlib-fixed.log`.
The first publication passes its official verifier and atomic publication, but its
temporary outer wrapper subsequently exits 2 after an edit made while Bash was
running. An immutable-wrapper warm retry exits 0 with every guard intact; all
fourteen XCFramework files, manifests, native/header hashes and source provenance
are byte-identical. Evidence is `/tmp/kotodama-final-apple-publication-comparison.json`.
Full Swift then stops before tests on one missing required `minimum` argument in
the SCCP JSON parser. Both affected full source files match HEAD, as recorded in
`/tmp/kotodama-swift-preexisting-compile-origin.json`; this is not an executed HEAD
baseline. The narrow repair supplies `minimum: 1`, matching the surrounding
positive checkpoint/epoch constraints, and adds exact zero-rejection coverage.
Because Swift source belongs to the Apple source seal, the official warm reseal
runs again and exits 0 across all five targets, link checks and host crypto checks.
Native slices, headers, toolchain and lock bytes remain unchanged; the authenticated
manifest records the repaired Swift source. Evidence is
`/tmp/kotodama-final-apple-sccp-fixed-comparison.json`.
The following full Swift run passes library compilation and executes tests, but
aborts on a HEAD-identical obsolete contract-address fixture before completing.
Four loader tests also look only in the repository's default artifact location,
although this run uses the supported authenticated external package. Narrow
test-only fixture repairs and a complete warm rerun remain required; this partial
run is not a full suite result.

The official JavaScript native build publishes successfully after the stale
ignored binary/checksum pair was preserved outside the repository. After fixing
all three task-related failures, its full unit run passes 2,856 and fails 22 tests,
with zero skips. The 18,652-file before/after source inventory has no drift, and
native checksum/authenticated loading pass. The compiler browser bundle now contains
required nominal-error, Unit and cursor validation: 56,823 bytes/seven inputs,
with a deliberate 56 KiB V1 budget and unchanged browser isolation guards. Same-tool
HEAD reconstruction confirms the other four oversized bundle targets already
exceeded their old caps. Other failure classifications use unchanged source and
fixtures, not an executed HEAD suite. The final full-run log is
`/tmp/kotodama-final-js-unit-tests-resealed.log`; its native binary SHA256 is
`024ab6f700191eabb89c31d858f75647851cc0642f6aa00f6e4d1a6722c7609c`.
The subsequent compiler Result repair has a fresh official native rebuild and
complete unit requalification. The build recompiles the compiler, artifact
admission, VM, Core and JavaScript host, then passes guarded transactional native
publication and distribution generation. All 2,878 unit cases run in 342.76 seconds:
2,856 pass and the same 22 fail, with identical failure names and diagnostic
headlines. Four direct public JavaScript/native compiler controls pass. The native
addon SHA256 is
`1e135b6e347009140c6fcaec44b01a7c2bb3fd131ab4c504668a01a0334cedea`;
the unchanged 18,655-file source inventory hashes to
`13f2296d7635c92ed8ab8c5238afe2b1387cdc824d5e593fe4e77c27d4df74b8`.
Git status, index, lockfile and native hashes remain unchanged throughout the run.
Complete evidence is
`/private/tmp/kotodama-result-final-js-_ofd3fn6/qualification.json`. This is not
a full JavaScript suite pass. Subsequent ledger edits require official warm
rebuilding/resealing and verification that the qualified native bytes are unchanged.
JavaScript's source guard includes every
tracked/unignored repository file, so documentation edits also require resealing;
the official guard remains enforced throughout.
The aggregate static-asset guard also reports an existing missing zk_x509 asset
consumer. Workspace build was attempted and fails at eight Kagami type/mutability
errors and twelve Python Rust binding import errors. The failing statements match
HEAD. The eight Kagami production errors, fifteen later Kagami test API errors,
and nine Python default-policy imports have narrow current-API repairs. Three
Python policy imports still require an unavailable direct dependency edge; no
Cargo.lock change or compatibility re-export has been introduced. The fresh Kagami
selection passes 130 of 147 tests, including all three required repair checks.
Its 17 failures comprise ten custody setups, six other assertions/fixture premises
(one comparison's exact differing field remains unproven), and an existing
no-config Taira bootstrap/staging stake-asset inconsistency. Implicated source
matches HEAD; no HEAD Rust baseline was executed. The exact classification is
`/private/tmp/kotodama-kagami-seventeen-xj_fs8qj/classification.json`.
The fresh locked workspace test attempt stops during compilation with four E0509
diagnostics from one SoraFS CLI test fixture: its struct update moves non-Copy
fields from `LocalQuicProxyConfig`, which implements Drop. Both the test file and
the defining proxy file match HEAD byte for byte. Strict workspace/all-target
Clippy stops at nine `doc_markdown` errors in the service-model policy comments;
both complete source files also match HEAD. The combined exact-line/source-hash
classification is `/private/tmp/kotodama-workspace-gates-qqfzc2ro/evidence.json`.
The logs are `/tmp/kotodama-result-final-workspace-tests.log` and
`/tmp/kotodama-result-final-workspace-clippy.log`. These failed attempts do not
establish a workspace test or lint pass; no full HEAD Rust baseline was executed.

After the Torii production repair, the daemon build reaches a HEAD-identical custody
error enum using an undeclared thiserror dependency. It now implements Display and
Error with the standard library, preserving all ten fixed messages and the existing
redaction test. No dependencies or lockfiles change. The fresh daemon and deployment
utility build passes in 11m20s with `iroha_cli/dev-tools`. Six HEAD-identical custody
test compile errors are repaired through explicit function-pointer types, an
unshadowed fixture reference and the direct signature byte slice; all fifteen
custody library tests pass. The frozen network binaries are in
`/private/tmp/kotodama-final-network-binaries-z_u4gmbs`, with `built-binaries.json`
recording daemon SHA256
`1fd6711913cface0ffe45c0d9628019e99071c6b024d6f01bf23a646e76c9f52`
and deployer SHA256
`915658ed46e04247dcda1ee4d7dd1fd42d4e1d399471cb8cddc90a67d6e23288`.
The network harness compiles and its pure probe test passes. A HEAD-identical
repair-ledger fixture needed one redundant `?` removed from a Unit-returning wait.
The first network attempt overflowed the default native test stack before any node
started. The probe now follows the existing four-validator test pattern with an
explicit 64 MiB thread/runtime stack; IVM limits remain unchanged. A frozen-binary
diagnostic run with the same larger native stack starts all four validators, which
connect with matching signed handshakes, but all three automatic startup attempts
time out before genesis commits (877.03 seconds total). The fresh harness with four
runtime workers and targeted consensus traces also times out after three startup
attempts (875.07 seconds). These are consensus startup observations, not contract
execution or rejection evidence.
The traced retry profiles the leader past proposal admission: 551 consensus-thread
samples are in canonical genesis wire encoding, and a later 499 I/O-thread samples
are in durable validation re-encoding the signed block. Other peers continue their
lifecycle loops. This demonstrates forward progress in slow unoptimized Norito
serialization rather than a permanently false proposal gate. The isolated
`local-release` daemon build passes in 36m06s. Its diagnostic four-validator run
passes genesis startup, then fails after 379.94s because the test requests certified
lane-block diagnostics before submitting any normal transaction. Genesis global
finality does not populate those lane-block artifacts. The assertion now follows
the first applied normal transaction; DA/RBC checks remain required.
The fresh optimized daemon after the Result repair builds in 20m08s. Its immutable
copy is `/private/tmp/kotodama-result-final-daemon-binaries-zk6fs828/iroha3d`, SHA256
`135f921281135c9b45c0ef6990e01faee3e49bd426fdfe0ac941dcd6bf1aa2e5`.
This executable uses `local-release`. The existing Cargo profile policy excludes
that profile from release evidence, so the following network runs are diagnostic
observations only. A normal `release` daemon build with the same source and
default shipping features passes in 44m15s. Its locked, offline build record is
`/private/tmp/kotodama-release-daemon-w_qw9coo/record.json`; the immutable daemon
SHA256 is `f9ae11868c31959b989ca3e7b605d293a23fa5b6e89497af82512d4ec1e6dc6c`.
The adjacent `frozen-binaries.json` verifies the compiled source/dependency
closure, executable version, lockfile and staged-content preservation. This is
local release-profile qualification, not signed distribution attestation. No
timeout or consensus policy changes follow from this profile correction.
The corrected harness compiles and passes its pure source probe; its immutable
SHA256 is `51416c583049ea953efefa10da7ba17deb845a46b0a1d4236838806f62aa387a`.
The same-source four-validator run fails after 1088.65 seconds. Registration
transaction `5b604d199596ea2536fc640f3ebfc79d036e8a7814b419572c686be72efb1e55`
times out 600 seconds after its initial Queued event while peers have reached
common durable block 6. Current typed inspection subsequently verifies its exact
QueuePlan certificate at global height 5 and its RegisterSmartContractCode body
anchored at global height 6, lane 0/lane-block height 2. The canonical body hash,
envelope coordinates and admission signatures match; there is no executed merge
result for registration. Its final authoritative transaction status was not
captured. A sampled harness stack proves it is waiting for registration's
Applied acknowledgment, before CommitContractDeployment, contract calls, the later
RBC assertions or restart. Durable block storage is not an Applied proof. Node
evidence identifies missing certificate/application completion for lane 0,
lane-block height 2, proposal/global height 6. The rollover loop already retries exact outputs and drains
terminal ingress; no missing production driver has been established. Relevant
production paths match HEAD. The large 147-test Kagami run overlapped the entire
registration window, so an exclusive rerun with the same immutable binaries is
required before attributing the stall to a production defect. The ordinary
selector timestamp does not measure terminal-ingress service and is not evidence
of complete starvation. No status, quorum or runtime policy has been weakened.
The log is `/tmp/kotodama-result-final-four-validator.log`, with runtime-only peer
data under `/private/tmp/kotodama-result-network-qpdkso8k`. Frozen binary hashes
remain unchanged. The current typed admission/anchor proof is
`/private/tmp/kotodama-canonical-prior-run-qw5sx1ye/current-typed-anchors-inspection.txt`.
The following controlled run also fails during registration confirmation:
1,024.80 seconds total, with transaction
`f5fc7b9d17f2040af0a4e544fb36bf926d0f1443901a6d2194965a09c5cbe83b`
timing out 600.002984625 seconds after its initial Queued event. The client warning
does not establish the final authoritative transaction status; no final exact
transaction-status response was captured. This run deliberately overlaps one
isolated Apple compiler job after the other test suites finish; it is not an
exclusive idle baseline. All peers finish at durable global height 4, and a fresh
HTTP status response confirms published State has also reached height 4. Earlier
QueuePlan gossip rejections reflect a transient State-height-3/Kura-height-4
publication gap for the preceding upload transaction. The previous height-6
completion warning does not recur.
All peers shut down cleanly and both frozen binary hashes remain unchanged.
Certificate/body retry paths exist; no omitted retry or skipped State-publication
defect has been established. A bounded inspector linked against the freshly built
current data model and Norito codecs verifies all four canonical blocks and their
complete referenced merge entry, including exact re-encoding, reference equality,
entry/result hashes and both Merkle roots. Block 4 contains entrypoint
`5d651db5425aaed2cd6867df9c323c3c68107f131752d6572ef308bc241cc14f`,
which successfully uploads and finalizes the single 8,173-byte artifact chunk.
The awaited registration hash is absent from every persisted block and referenced
merge entry. External signed-transaction and entrypoint hashes are identical.
The successful upload's stale gossip does not establish the cause of registration's
timeout. Typed inspection and toolchain/library provenance are recorded in
`/private/tmp/kotodama-canonical-block4-_c2nk2yl/current-typed-inspection.txt`
and its adjacent provenance records. Complete run evidence is
`/private/tmp/kotodama-controlled-evidence-wmbqv1u6/final-observation.json`.
No contract-call or restart success is claimed, and an exclusive rerun remains
required unless a concrete intervening defect changes the experiment.
The subsequent status diagnostic uses the same immutable binaries with a tested,
payload-free client hash filter. It fails after 1,066.73 seconds: registration
`4589cc33c9cdfd04a7a93099198dbf6527b59814f0e80fa722db5970ca7ffecb`
times out 600.00341275 seconds after its queued event. Nine bounded public reads
verify the upload as globally Applied at height 4; registration has no returned
global terminal outcome at +30, +120, +300, +540 or +580 seconds. Those exact 404
responses are resource-status NotFound, not a missing route, and do not exclude
pending admission: global status omits ordinary nonterminal hints. Current typed
inspection verifies registration's admission certificate in global block 5 on all
four peers, with its exact body anchored in block 6 on three peers. The fourth
peer stops at block 5. The retained pending certificate matches the validated
admission field byte for byte. No peer has a registration execution result.
Final sidecar indexes show execution inputs for lane-block 2 on all peers, a
lane-block-2 certified-block entry on one peer, and no lane-block-2 application
receipt. These sidecar counts do not independently validate their encoded bodies.
One guarded harness sample identifies the registration confirmation wait; no
daemon sample ran because its ownership guard stopped during cleanup. The harness
reports one raw exit status 0 and three raw statuses 3 (SIGQUIT), consistent with
its post-timeout cleanup escalation. The logs do not identify each signal's sender
or recipient, so this is not a claim that all peers shut down gracefully.
Both frozen binary hashes remain unchanged. This run overlaps one Apple compiler
and is not an idle baseline. Evidence is
`/private/tmp/kotodama-status-diagnostic-evidence-tu5kcqzx`, including the exact
status observations, stage snapshots and guarded sample interpretation. Canonical
admission/body inspection is in `/private/tmp/kotodama-canonical-status-run-9gp0_dqp`.
No deployment commitment, contract invocation or restart is reached.

The reviewed replacement integration harness corrects two later fixture checks:
Alice receives the exact `CanManageAccountAlias` scope for
`contract_state_probe@universal` in `DataSpaceId::UNIVERSAL`, and the RBC
transaction predicate distinguishes nonzero proposal height from the subsequent
authoritative Applied height. It retains exact transaction-hash membership and
every other ownership, descriptor, quorum, signer, availability and cross-peer
check. Four offline tests pass, including proposal 3/applied 4 and future-height,
zero-height and wrong-hash rejection. These corrections do not explain the earlier
registration timeout. The normal upload/register/Commit flow remains unchanged.
A companion test registers the signed artifact in genesis, then uses the normal
post-genesis deployment, invocation and restart flow. It provides additional
execution coverage without replacing the original full-workflow gate. Neither
network selector has run against this replacement harness yet. Source, test and
preservation evidence for the fixture correction is in
`/private/tmp/kotodama-genesis-companion-final-harness-zsbll4em/evidence.json`.
The final harness also emits bounded public stage/hash records for `hajimari`
and `verify`, with no arguments or credentials. Both probe modes commit the
deployment and grant Alice the exact deployed-address/`hajimari` invocation
permission in the same ordinary transaction. Ownership alone does not grant
lifecycle invocation. Generic deployment callers retain their existing single
commit instruction. Five focused tests pass, including exact grant scope,
instruction order and wrong-address/selector/grantee rejection; immutable SHA256 is
`935178dbe47a9c778a9c50ff822e28fbd2ce92be0e6fda74201ee62375774711`.
Source and test evidence is
`/private/tmp/kotodama-next-network-tools-az48mv2j/release-grant-yy69tt_f/qualification.json`.
The prepared launcher and observer pass ten offline guard tests and six filter
checks. They bind exact processes, binaries and public log paths, cap each status
read at five seconds/64 KiB, and preserve the existing test deadlines. Evidence is
in `/private/tmp/kotodama-next-network-tools-az48mv2j/preparation.json`. An
independent review verifies the final release daemon and harness bindings; both
network selectors remain pending.
Historical sampled callsite offsets apply only to the earlier binary hash.
Final closure requires every redesigned behavior, installed final artifacts and
the four-validator execution/restart evidence.
