# Kotodama / IVM reconciliation on optimizations

All work in this checkpoint uses `/Users/takemiyamakoto/soramitsudev/iroha`
on `optimizations`. Earlier checkout results do not qualify this combined source.
The [completion goals](../../../specs/kotodama_ivm_completion.md) remain open;
there is one final ABI V1 and no backward-compatibility path.

## Current-checkout evidence

- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked
  -p ivm --lib`: 984 passed, zero failed or ignored, after the block-execution
  extraction and explicit test-scope imports. Runtime was 146.27 seconds.
  This binary predates the subsequent semantic-owner extraction; its receipt
  records that compiler source hash, and a rebuilt compiler/consumer run remains
  required. This suite is not physical hardware or release qualification.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked
  -p mv --tests`: 419 passed, zero failed or ignored. The preceding failures
  exposed two stale reservation assertions and a test allocator-observer
  collision. The assertions now retain the actual release-notification charge
  until the final future owner drops. The unwind observer is armed after its
  uncharged notification scaffolding is created, so it observes the intended
  root/reader allocation. Production cleanup and free-before-refund assertions
  remain unchanged. The focused linear-custody rerun also passes 14/14.
- `.venv-sdk-nonrust/bin/python -m pytest -q
  scripts/tests/check_sora_parliament_source_contract_test.py`: 184 passed.
  The guard follows the actual `beacon/tests.rs` module and checks canonical
  request admission inside both pulse fixtures; nine added mutation controls
  reject disabled, redirected or disconnected test ownership.
- Python registry, native-registry source and CLI-graph controls: 27 passed.
  These are not a native SDK qualification result.
- `scripts.tests.large_static_contract_asset_compaction_test`: four passed,
  preserving the 2,000-line reduction requirement; the reviewed reduction is
  2,171 lines. This is source-layout evidence, not a codec roundtrip result.
- `scripts/check_kotodama_test_sources.py`: 302 fixtures and 591 tests validate.
  The stale Python expected total was corrected from 588 after comparing the
  final table-ABI, numeric and anchored-spend test inventory. All 25 fixture-
  manifest controls pass after that correction.
- Fresh Kotodama Rust tests after the parser, semantic and descriptor extractions
  and native AXT regeneration pass all 1,115 library and 84 integration tests.
  The earlier 1,114-pass/one-failure library result exposed a fixture missing
  its clear committed amount; the validator and failing assertion are unchanged.
  A concurrent data-model test-only file changed during the fresh run; its
  enclosing `cfg(test)` module is excluded from the compiler's dependency build.
  The receipt preserves this drift and the scoped feature adjudication.
- The AXT native generator passes its three boundary tests, regeneration and
  `--check`. Checked conversion rejects zero, fractions and amounts beyond u128;
  the maximum u128 succeeds. Only two proof payloads, their four attached copies
  and four covering spend signatures changed. The canonical envelope SHA-256 is
  `089f3400e106309a84b19064a50fd9d07624d33ac091a410395b18ad9a986037`.
  These fixtures contain placeholder FASTPQ proof bytes and do not establish
  execution-proof, authenticated-State or production AXT qualification.
- The fresh ABI library passes 181/181. Its receipt retains the concurrent
  two-line data-model test expectation correction and proves both removed
  duplicate entries are inside an excluded dependency `cfg(test)` module;
  no compiled production input changed.
- The next consumer matrix passes three model and one artifact-admission
  controls, then IVM groups 01/04/05 pass 23/9/60 selected tests. The group05
  post-run guard detects ten changed data-model files, including production
  consensus and committee code, and stops the matrix. This is evidence for
  the recorded prior source cut; it does not qualify the current candidate.
  Groups 06/08, the extracted ISO157 and Metal82 remain unexecuted in this run.
  `axt-consumer-validation/post-model-drift-adjudication.json` records the exact
  changes; external work is preserved and fresh current-source validation is
  required.
- A fresh production data-model build with only `bls,gost,sm,pqc,transparent_api`
  and no default features succeeds. Its post-build guard detects further changes
  in `commit_certificate.rs` and `nexus/committee.rs` and refuses to compile or run
  the separate registry consumer. The exact five-feature artifact is recorded,
  but all 361 ordinary wire assignments and 21 governance exclusions still need
  a stable-source production runtime check. Added/deleted/changed source entries
  are included in the guard; `native-capture-maintenance/no-governance-consumer/`
  retains this failed qualification attempt without replacing prior evidence.
- Seven focused Core owners pass 90 tests with zero failures or ignores:
  State observation 11, membership root/record/append 38, paired typed capture 6,
  complete transaction membership 7, grouped table capture 11, Kura storage 10
  and Queue authority 7. The selected source hashes and binary are unchanged
  across execution. The Kura source census has 11 declarations because one is
  for unsupported platforms; it is not an executed test on this macOS runner.
- The test-only fixed-stack BLS candidate passes six controls covering both
  current signature orientations, malformed/subgroup rejection, exact upstream
  message points, independently generated signatures and allocation-observer
  positive/unwind behavior. The Rust allocator observation and pinned C
  call-chain audit provide separate evidence. This earlier test-only result
  did not change production verification.
- The BLS single-signature facade now uses the shared canonical parser and
  fixed-stack contextual relation for ordinary, typed and admission callers;
  the prepared-key verification cache and former relation are removed. A
  temporary migration oracle passes 75 differential controls, then is removed
  with exact shipping hashes restored. The final shipping BLS suite separately
  passes 75/75 with no ignores or source drift across 19 local dependency
  packages. It covers declared parse-error ordering, malformed encodings,
  subgroup/identity cases and cache/allocation controls. The full Crypto run
  finishes in 12m20s with 1,506 passed, three failed and three explicitly ignored,
  without source drift. All three failures reproduce in the retained pre-BLS
  executable with identical FHE material/bundle/statement digest mismatches.
  A prior eight-u32 statement-binding change updated only the first schema pin;
  native recapture of its remaining downstream pins is still pending. After
  narrow fixes preserving assertions and original refusal ownership, the final
  shipping selection passes 124 tests: 75 BLS, 43 affected unit controls and six
  allocation integration controls. Crypto-only strict Clippy passes. The
  dependency-inclusive invocation remains failed with five MV representation
  diagnostics and two numeric casts; these are tracked separately rather than
  counted as a strict pass. Exact captures remain unchanged, and the temporary
  differential owner is absent. The final source disposition is recorded in
  `bls-production-staged/final-qualification.json`.
  General crypto error/resource custody, positive cache funding and non-BLS
  algorithm admission remain open.
- The full language/script selection passes 96/96 after source-layout reconciliation:
  fixture inventories and staging, regeneration controls, compiler ownership guards,
  indexed VM metadata, Metal dispatch source guards and CUDA bundle provenance.
  This is recorded in `current-language-python-receipt.json`; it does not qualify
  physical hardware execution.
- Native maintenance-capture controls pass 5/5, with one explicit printer ignored
  by the ordinary control run. The separately requested 111 schema printers,
  complete 324-record printer and wire-ID printer all succeed. Review-only
  proposals contain eight new schema identities and current FASTPQ framing.
  The reviewed pins are now applied: all five retired Kaigi cases and all 20
  negative decoder assertions have dedicated rejection-only custody, while one
  canonical positive record inventory replaces the redundant Kaigi fixture.
  The published JSON preserves its layout and matches the native proposals as
  parsed values. Eight mutation controls and exact application hashes pass;
  post-update native Rust capture checks pass 199/199 and generated record
  checks pass 328/328. Their ordinary runs explicitly ignore 111 schema and
  four record printers; those maintenance printers have separate capture
  evidence above. The first registry run passed 43/44 because the expected
  governance list had two duplicate rows. Removing only those rows, with no
  fixture or hash repins, now passes all 44 rebuilt registry tests, with no
  ignores and unchanged source/binary hashes. The exact model executable was
  emitted successfully by the combined build described below; its success
  does not turn that failed daemon build into a pass. The receipt is
  `daemon-authorization-audit/registry-controls-receipt.json`. Six exact Kaigi
  tests explicitly revalidate all 20 retired-scalar rejection assertions and
  positive record cases; these are covered by the 328 total, not extra tests.
- Locked workspace metadata, the IVM-only and retired-codec guards, and historical archive
  reconstruction pass. Reconstruction covers 64,736 records and 67,311 source
  occurrences; it makes no current-release readiness claim.

The MV result/log record is
`.codex_artifacts/kotodama-v1/mv-full-tests-after-observer.json`, with log SHA-256
`253d391be32878d8f07c8bfd2a0442960297c43878a09f22e7bccc3681e0cee9`.
The formal result is `state-owner/FORMAL_LAYOUT_EVIDENCE.json` under the same
artifact directory, with log SHA-256
`7d42f682a32e685ba44e88fb279d63b217df8d6f65eafbb844fec336a709bd70`.
Both records identify their scoped source hashes; neither seals the workspace.

## Integration and remaining gates

The first combined Core/daemon check stopped at an allocation-free Musubi
error-enum lint. The rerun reached Core and found a stale State-inventory
module path and missing authenticated-release digests in the production
KAGEMUSHA runtime constructor. Corrections require a fresh combined check.
The next check passed those former compile points and stopped at a Queue
consumer of a removed scheduling helper. The caller now validates and uses the
committed proposal-height schedule directly, including geometry, canonical order,
proofs of possession and parameters; all seven focused controls now pass,
including distinct committees at each retained height and populated live
registrations with no retained schedule.
The next combined check passed Core and Torii with warnings, then reached a
daemon snapshot-probe caller missing the new execution-pool argument. The probe
now creates the same configured execution budget as startup and passes that owner
into restore; the obsolete trait import was also removed. The combined Core/daemon library rerun passes in 10m06s, with warnings.
Its receipt is `current-core-daemon-check-probe-budget-receipt.json`. This is a
library typecheck before the next descriptor and fixture update, not a strict
Clippy or release pass. The Core test-binary build stopped at concurrent
`epoch.rs` slice-type changes already corrected in the current source. The
fresh locked Core/data-model library test build now passes in 14m16s, with
warnings and all 166 selected input hashes unchanged. The exact binaries and
log are bound by `current-core-model-test-build-receipt.json`; the 90 focused
Core runtime checks above run against that successful build.

The subsequent combined locked daemon/data-model test build failed after
14m44s at one Musubi test fixture calling `AdmissionRegistry::empty` without
its required network ID; all 970 scoped source hashes were unchanged. That
fixture now supplies its existing service configuration's network identity.
The original failure is retained in
`daemon-authorization-audit/combined-build-receipt.json`; the guarded one-site
repair and fresh combined rerun are recorded separately. The locked rerun
passes in 8m25s with unchanged scoped inputs and produces both exact test
executables, recorded in `daemon-authorization-audit/combined-rerun-receipt.json`.
The emitted model executable changed, so all 44 registry tests were revalidated
against its new hash; `registry-rerun-controls-receipt.json` records that pass.
All 26 selected daemon controls now pass against the fresh executable: the
three new checked-reservation cases, eleven remaining native signer cases,
ten snapshot compatibility cases, one signed-genesis/config case and the
affected Musubi factory case. The selected source and both executable hashes
remain unchanged; `fresh-controls-receipt.json` binds this evidence. Genuine
finalized capabilities reject substituted local custody and expiry, and the
valid reservation sequence requires its distinct before-provider check.
These controls do not independently isolate removal of only the second
`checked_context` wrapper. No production publication gate was changed.
Two additional tests are now applied: an independently signed custody renewal
is enrolled by native execution, then the actual BeforeProvider transaction
either finalizes within its eight-second lifetime or is held until that lifetime
expires. The refusal case requires all three native actions to finalize while
policy, request, reservation and original transaction deadline remain valid;
only local custody freshness may refuse return. Both expiry controls now pass
against a fresh daemon build. Removing only the second checked wrapper makes
the regression fail at its required freshness assertion after all three real
actions complete. The original wrapper is restored exactly. The subsequent
restored build passes, but two concurrent Core staking test-file edits stop its
full selected-source guard. Those files are excluded from daemon compilation;
the failed receipt is preserved and a new independently captured shipping run
is validating the remaining controls without repeating the mutation.
The earlier three tests remain byte-for-byte intact. The guarded application is
recorded under `daemon-authorization-audit/second-boundary/`.

The State table-selection map is replaced by an inline bitset over a
compile-time catalog of borrowed static descriptors. The catalog rejects
duplicate identities and preserves lexical schema folding; selection preserves
count, schema and duplicate-error ordering. Paired/table verification consumers
use the same implementation, and borrowed proof verification creates no
resident allocation pool. The enclosing retained allocation includes the
inline bytes. Nominal schema Strings, codec scratch, proof-output funding and
complete-State publication remain open. Six exact guarded source changes pass
scoped Rust formatting and diff checks; three new controls plus existing leaf,
capture and membership tests now pass against the recorded Core build described
below. The application
receipt is `state-selection-inline/application.json`. Locked dependency analysis
confirmed that the concurrently running model/IVM/FASTPQ consumer builds do not
depend on Core, so this edit does not change their compiled inputs.
Unknown-field and unselected-table refusals now use fixed typed variants instead
of copying arbitrary caller-owned identity strings. Their internal consumers
and negative assertions use the final variants directly. Count/error ordering
is preserved, and the additional large Unicode identity/lifetime control passes
against that same recorded build. This removes those two error payload allocations; schema,
codec and other resource obligations remain open. The guarded four-file update
and independent review are in `state-selection-refusals/`; the selection group
now requires four tests.

Musubi source validation now returns a token retaining the exact validated native
World borrow and original allocation-pool reference. Its three projection
iterators borrow that cut's canonical keys. A private sealed carrier trait
restricts construction to native WorldView, WorldBlock and WorldTransaction
carriers; a generic public read-only trait alone cannot guarantee immutable
observations. All four new controls and seven existing source-work controls pass
against the recorded Core build; one existing assertion requires the qualified
error-field correction described below and a fresh rerun. The token grants neither funding nor finalized-root
authority, and the three missing semantic readers remain unregistered until
every reachable crypto/parser/error allocation and refusal has real ownership.
The guarded application and independent review are recorded under
`state-owner/next-capture/musubi-source-boundary/`.

The interpreter's complete block-execution owner is extracted into
`ivm/block_execution.rs`: fallible worker snapshots, reservation custody,
ordered commit and sequential rollback move together. The parent is reduced
from 9,414 to 8,658 lines, below the previous 8,829-line ceiling. The moved
production sections and existing test module retain their bytes after
formatting. All 984 IVM library tests pass after correcting test-only imports
for the new owner. The compiler separately moves typed-HIR traits/rendering
and trigger lowering into internal modules, reducing `semantic.rs` from 18,627
to 16,100 lines under its previous 16,156-line ceiling. Expression grammar has also moved intact into
`parser/expressions.rs`, reducing its parent from 7,073 to 5,992 lines. The moved
method bodies and parent tests are unchanged apart from required internal
visibility. Entrypoint descriptor construction and the semantic ABI surface
also move into internal owners; their parents are now 19,290 and 4,724 lines.
Eight reduced compiler/IVM owner ceilings ratchet down to their actual sizes;
no limit was increased. Fresh Kotodama tests after all four compiler/ABI owner
extractions and corrected native AXT regeneration pass 1,115 library and 84
integration tests. The generator and its reproducibility check pass; remaining
ABI, Core and native consumer validation is still required.

The ISO message-stack test owner is now separate from production code, reducing
the parent from 5,141 to 2,955 lines and retaining all 157 tests in a 2,184-line
owner. Reinserting the extracted test module and formatting reproduces the
original source exactly, including literal contents. Test module names, include
paths and asset bytes remain unchanged; fresh Rust execution is pending. The
ABI syscall parent is below the default 5,000-line limit, so its obsolete size
exception is removed rather than retained as a smaller exception.
The three FASTPQ Metal test owners likewise retain all 80 extracted tests and
two existing parent parity tests, reducing the parent from 7,318 to 5,857 lines.
Production bytes are unchanged. Rustfmt orders the out-of-line test module
declarations; the separate formatting delta is retained with the extraction
receipt. The existing 5,862-line ceiling ratchets down to 5,857. Fresh component
execution is pending, and existing device-unavailable early returns cannot
establish mandatory kernel execution evidence.

The first language-script selection recorded 77 passes and eight failures.
After reviewing the V1 implementation and preserving all fixture payloads,
the complete expanded selection passes 96/96. The refreshed source-budget
checker reports no compiler, IVM, ABI or FASTPQ findings after these owner
extractions. The whole checkout still reports 228 findings and requires
cohesive extraction, not larger limits.
JavaScript's two pure registry-rejection tests pass, but its native-backed
verifying-key suite cannot load the current Darwin bridge checksum profile.
Native schema/frame capture and rebuilt SDK artifacts remain required.
Whole-workspace formatting check also reports outstanding differences; the
working-tree whitespace check passes.

The shared full-width integer implementation now converts bounded work counts
with checked `u16` conversion: every backend magnitude is at most 64 limbs, and
the initial root of a validated positive input needs at most five. Numeric
values, staged gas events and refusal ordering are unchanged. Two new controls
cover every backend word boundary and refusal before maximum-input root
materialization. All six integer controls pass against the exact Primitives
binary emitted by the successful combined build, selected from Cargo JSON and
frozen before execution. This is recorded-build evidence, not unchanged-current
checkout qualification.
The MV lint correction retains the original inline capture/publication unions
and original journals on refusal; three precise fulfilled expectations document
why heap indirection would add unadmitted storage. The owned return uses the
existing exact result type alias, and one unfulfilled expectation is removed.
No executable body, field order or physical layout changes in that patch.
Dependency-inclusive strict Clippy still requires a fresh rerun.

The current local runner reports Apple M1 Ultra with 128 GiB of memory. Required
exact-candidate gas calibration and measured kernel execution remain pending;
runner availability alone is not hardware qualification.

The FASTPQ shipping audit finds a separate remaining acceleration dependency.
Its four-unit, 12-entrypoint Metal build passes an absolute build-directory path
to runtime, then compiles embedded source when that path is unavailable. Its
nine CUDA kernels and native host wrappers still use nvcc/cudart, independently
of IVM's ten PTX families. Ordinary daemon defaults do not enable the complete
FASTPQ feature chain. Bundled artifact bytes, authenticated provenance,
toolchain-free startup, target-default wiring and a dynamic CUDA host owner
remain required; the native full-proof GPU preflight remains closed. Existing
component test success cannot qualify these missing shipping owners.
The shared runtime CUDA loader now caches successful symbol resolutions only;
an initial unavailable-driver error no longer permanently poisons that symbol's
cache. The two-file change passes scoped formatting and diff checks, all 76
library tests, the one explicitly enabled absent-driver integration test and
strict all-target Clippy. The source and file-membership captures remain stable.
The typed local mock-library control proves symbol-resolution retry and reuse;
no public-wrapper driver-installation, kernel or physical recovery result is
inferred. Receipts are under `fastpq-cuda-runtime-cut/loader-validation/`.

The next combined Core/daemon/Primitives test build stops after 765 seconds
with two Torii accesses to the removed preparation `roster` field. A concurrent
edit to the Core epoch-election test owner also differs from the initial source
capture. The failed receipt and original runner are retained; no runtime tests
from that attempt count as current-source qualification. Eight typed consumers
across Torii, daemon signing, disposable network preparation and the rotation
integration test now read the final `committee` directly. All eight only project
validator identities: canonical ordering, indices, hashes and custody checks
are unchanged. No retired-field alias or decoder is retained. The guarded
five-file patch passes scoped formatting and diff checks. The fresh retry adds
Torii to the source inventory and passes the build in 368 seconds with all 5,214
captured inputs unchanged through compilation. A subsequent Core World edit
stops the runtime sequence after four passing selection controls; that attempt
does not qualify the now-changed checkout.

The exact three emitted binaries were then frozen and hash-checked for explicitly
scoped recorded-build execution. The Core selection, leaf, table-capture and
transaction-membership groups pass 88 controls. Musubi passes eleven controls
and fails one assertion expecting an unqualified error field; all six numeric
controls also pass. The total is 105 passing controls and one failed assertion.
The production error helper already returns `world.musubi_resolver_index`; the
test now expects that exact field while preserving its direct/wrapped error
equality, work-refusal and resource assertions. Fresh execution of the corrected
test remains pending. The frozen component receipt is under
`state-selection-inline/runtime-recorded-components/`.

Daemon expiry and second-wrapper mutation validation use an independent
eleven-owner source capture and fresh daemon build, including intervening edits.
The subsequent restored-only continuation builds successfully in 2.7 seconds
and passes the five checked-reservation controls with unchanged captured inputs.
The remaining native group passes nine and fails two on that unchanged capture:
production issuance rejects an actual queued transaction, and software
issuance/recovery returns `AmbiguousCompletion`. Their transaction-phase
diagnostics remain pending; no timeout or production error mapping is relaxed.
The seventeen independent remaining controls pass against the explicitly frozen
binary, with subsequent source drift recorded separately. These results do not
turn the two genuine native workflow failures into a current-source pass.
Broader Torii and four-validator network qualification remain open.

The staged physical acceleration owner now retains device/context identity,
exact artifact bytes, finite host/pinned/device reservations, checked cleanup
and charged escaping outputs. The CPU crate passes 22 tests and the extended
helper selection passes 34 against recorded dependency artifacts; the earlier
18 controls are included. Independent static reviews find no blocking lifetime
issue. Native CUDA compilation, the complete consumer/configuration migration,
Metal integration and hardware execution remain open; no production factory
replacement is applied from these staged controls.

The guarded integrated Crypto change is applied across 24 files. ML-DSA now
borrows canonical key bytes instead of retaining a decoded-key Vec/cache copy.
SM2 verification uses the canonical portable relation regardless of optional
OpenSSL SM3/SM4 acceleration; the incorrect ECDSA-on-SM2 branch is retired.
Public SM2 envelope parsing borrows its identifier and compares inline SEC1
points. Explicit owned public keys still retain their dependency-owned identity
String; this change does not fund all cryptographic memory or activate State
readers. Shell smoke controls preserve prerequisite/build/test exit status.

A native FHE capture retains all 38 records (26 byte artifacts and 12 digests),
verifies the current 38-column/eight-u32 statement-hash geometry, and rejects
retired geometry. The temporary diagnostic owner is removed exactly. Four
stale downstream digest pins now match those captured native bytes; their
reverse substitutions reconstruct the original test source. No proof relation,
decoder, assertion or acceptance gate is weakened. Ordinary FHE controls and
the shared full Crypto suite remain the required runtime qualification.

Integrated qualification first encountered an uncached locked dependency during
offline metadata preparation; locked online metadata fetched the pinned input
without changing Cargo.lock. The next build exposed two integration-test
accesses to a crate-private ParseError field. Both now use its public Display
adapter with identical expected strings. The fresh third run seals 22 local
package roots and 2,775 files. Its initial grouped, scalar and allocation
controls pass. The full library finishes with 1,517 passes, two failures and
three existing ignores on unchanged captured inputs; all four FHE controls
pass. Both failing SM2 tests incorrectly treat the shared fixture's explicitly
tagged Annex D Fp256 point as a production sm2p256v1 point. The applied test
correction preserves both production vectors and their mutation assertions,
checks every fixture identity/domain and requires rejection of the other curve.
The OpenSSL smoke consumer now makes that same distinction. Later production
and codec edits make the earlier full result evidence for that earlier source,
not a full current-candidate pass.

The September 28 correction also removes SM2 from the generic decoded-key cache.
Ordinary, typed and admission verification borrow the original compact envelope
and use one fixed-storage relation shared with explicit owned keys. Review found
that the pinned dependency accepts a constructed nonzero r/s pair whose combined
point is infinity, exposing its x=0 sentinel. The shared owner now rejects that
point before affine extraction. The regression constructs the actual infinite
point and requires rejection through every public verification path; it does not
claim an unknown-key forgery. The [implementation record](../../../crates/iroha_crypto/src/sm/verification.md)
states the relation and remaining allocation boundary. The nine source/test/doc
files and three runner files were applied with exact before/after guards
(`398d4350f8d4a464d014c872754379f7dc2940f85d9e808250436a0b8a3378f4`).
Nineteen runner controls and six shell controls pass. The next build exposed two
retired `try_payload` test callers; they now exercise the infallible borrowed
`Cow` payload owner. Successful SM2 decoding no longer charges an owned decoded
key that it does not allocate. The following locked build passes, but four
concurrent MV/Concread source edits invalidate its unchanged-candidate guard.
Separate frozen-executable feedback passes 607 tests with zero failures and two
existing generator ignores: SM2 63, signature 228, root 99, FHE four, grouped 200,
ML-DSA six and allocation seven. All Crypto source and executable hashes remain
unchanged for that feedback (`8ac5f6d064252e055c9b2b32ef5f21069210b7e4f0bdf7a5acd2ff377dabc372`).
Doctests, optional OpenSSL, no-PQC, native and dependency-inclusive strict Clippy
remain open. Failure logs, repairs and distinct captures remain under
`crypto-integrated-qualification/`. These scoped results do not establish full
workspace or release qualification.

The next applied cut removes GOST's decoded-key cache and allocating verification
adapters. One fixed-width verifier borrows canonical key bytes and uses the shared
CryptoPro 256-bit and RFC 7836 512-bit parameter owner. It checks canonical points,
scalar ranges and infinity; ordinary, typed, admission and explicit owned-key
facades share the same relation. All 29 existing GOST tests and seven allocator
controls remain; nine unit and four allocator controls are added. Independent
source review and integer generator-order checks passed. The 19-file guarded
application includes coupled docs and runner controls
(`1c7e95010d0f0123ee132a2916cfce1a0c514cfaeefcaca5fdfdf2ac58b0bdf6`).
The fresh locked run06 selected build and execution pass with unchanged inputs:
620 selected runtime controls, one positive doctest and four compile-fail doctests.
Two existing fixture-generator tests remain ignored. The fresh no-default-feature
build and all four selected envelope/codec controls also pass with unchanged
inputs and no ignored tests. The GOST-only build and seven allocator controls
also pass. Native compilation succeeds, but its old module selector discovers
zero tests and correctly refuses a pass. The repaired runner names the seven
current `mldsa::verifier` controls explicitly; all 22 runner checks pass and a
new source capture owns the remaining native, OpenSSL and strict dependency
lint phases. Run07 passes all seven native verifier controls, one ML-DSA
known-answer control and six fake-tool shell controls. Its OpenSSL-feature build
and 71 SM, 37 GOST and 11 allocation controls pass; one existing GOST fixture
generator remains ignored. Four smoke cases explicitly report unavailable
SM4-GCM/CCM provider capabilities, so their aggregate six-pass test summary is
not provider qualification. That capability gate remains open. Strict Clippy
stops on two intentional inline MV publication-owner size diagnostics and one
complex refusal type. The narrow correction documents those inline owners with
local expected-lint reasons and names the unchanged refusal tuple; it introduces
no allocation or runtime behavior. A fresh strict run remains required.
Successful parsing now charges no nonexistent heap
intermediate, while compact storage remains
separately funded. Public error strings, inline scratch admission and private signing
allocation custody remain open. The local fixture's filename does not establish
independent upstream vector provenance.

The memory image now owns prepaid pending-commit and since-baseline dirty-leaf
bitmaps. Funded construction and cloning retain the original execution credit;
warm reset copies fixed backing without growing a dirty set. Incremental Merkle
updates borrow the bitmap directly and preserve canonical ascending update order.
The unused allocating arbitrary-index batch API and warm-reset allocation error
wrapper are retired, with their useful assertions migrated to the current path.
The locked library/grouped-integration build passes. Its source guard catches a
concurrent test-only repair: a nonexistent method call is removed and the actual
write must leave nonempty modified bits. The production code is unchanged and
all clone/root/log assertions remain; the accepted test repair is captured for a
fresh guarded run. That run builds successfully and passes all 41 memory tests,
then stops its candidate gate after a concurrent Cargo.lock change. Separately
captured exact-executable feedback preserves those results and passes the remaining
76 controls: 117 passed, zero ignored
(`583cb3805d28a32aa7e3736e190c1e48b4d637663b203a39d73e4a3aadfb0b6b`).
A separate development-profile diagnostic checks identical bytes and roots in all
60 samples against the extracted historical updater and full canonical rebuild;
it establishes no uniform speed winner or release threshold. Write-log growth,
worker snapshots, canonical-node construction and hardware scratch still require
allocation ownership. Immutable templates already retain their original aggregate
snapshot lease through final ownership; splitting that lease is a precision task,
not proof of early refund. See the [allocation record](../../../crates/ivm/src/memory/dirty_chunks.md).

The reviewed write-log ownership cut is now applied. Rows, payloads and detached
immutable snapshots retain their allocation charges, including through eviction,
concurrent borrowers, partial-copy failures and unwinds. Payloads scrub before
backing storage and its charge are released. The raw allocating snapshot API and
late aggregate estimate are removed, and all callers use the fallible owned
snapshot. Fresh locked compilation and all 182 selected tests pass with zero
ignored tests, including the eight new ownership controls and all prior 117
names. All 6,242 source/absence records across 41 local dependency roots remain
unchanged through the final guard; no frozen-executable continuation is needed
(`ec4c7becd1e2b88036f3bce28191297dc4d8508ec6b319962685836d874c38c6`).
This also covers the current AES correction and migrated snapshot consumers.
The run predates the subsequent genesis-policy pin correction below; it does
not qualify the final combined candidate.
Original finite active-pool funding for log growth and read snapshots, canonical
Merkle nodes and worker/hardware scratch remain open.

The staged Swift acceleration cut uses mandatory length-checked native exports,
preserves unsigned zero limits and rejects malformed present policy. Independent
parser review found and fixed silent loss of TOML opt-outs. Its 21 focused XCTest
controls pass with native calls stubbed. The complete 163-file SDK module compiles
with the staged C header and ordinary package settings; five actual acceleration
test files also compile against that module, with shared helpers and the generated
resource accessor. These checks link and execute no native bridge. A stricter
whole-module attempt retains an unrelated existing unreachable-default warning;
full SwiftPM/native execution and hardware qualification remain open. The Swift,
C and Rust acceleration changes are still artifact-only until coordinated application.

A separately reviewed AES correction is applied: full AES-128 decryption now
uses interior keys in reverse order from the same expanded encryption schedule.
The original source fails the independent FIPS-197 plaintext/ciphertext control;
the corrected exact source passes all seven focused CPU controls. The guarded
application is `edbaf9233c27cf7ec242d5a3f1ebf24688a9bd7a2adfbebc550e989cd31251cf`.
No batch API or native owner changes are part of that narrow fix. CUDA/Metal
full-cipher parity remains open. The current host identifies as an Apple M1 Ultra
with 128 GiB of RAM; that is runner availability, not exact-candidate calibration.
The codec-retirement guard and 64,736-record historical archive verification pass.

Daemon finality diagnostics retain the original deadlines and assertions. The
first build failed on eight duplicated WorldTransaction test helpers. Their
byte-identical test-only definitions remain after the redundant production copies
were removed. The second build succeeds; both tests then reject genesis before
signing because the pinned default confidential-policy hash differs from the
current canonical ZK/SCCP relation. Exact current State functions reproduce the
mismatch with unchanged configuration and SCCP dependencies. The pin is corrected
to `c736b694d3983182926ee2bd4944bc87b47cabbeea4f1eea874babcb37afaf54`;
no old digest is accepted and no validation or deadline is weakened. The third unchanged-source diagnostic builds successfully, but rejects a one-byte
transcription error in the updated pin. The constant above is now generated
directly from that computed byte array; fresh daemon execution remains required.
The exact current State functions and model-source constant now pass a scoped
compiled comparison with unchanged dependency artifacts
(`3f61cee619ac967f0699069495e8adea27b3d1ff3b37510908e636c1a80c6e49`).
All four diagnostic overlays were restored exactly. The fourth build succeeds
and both workflows pass genesis admission. They still fail after 60–64 seconds
with `RuntimeSignerOutputInvalid` or `AmbiguousCompletion`, after successful
Current, Reserve and repeated BeforeProvider/AfterProvider transactions. The
unchanged 10-second transaction deadlines are not the observed commit failure:
the logged commits succeed in roughly 0.4–0.9 seconds. The end-to-end failure and
growing intervals between checks remain under investigation. This executable
is diagnostic feedback after wider Core/lockfile drift
(`e9975e2d9583becf6c9fcbc09f78f5d7fb3df6cef09da91772cff101a4fe9571`).
Wider source drift is recorded as engineering feedback and cannot qualify the
combined release candidate.

Torii's unused proof JSON content-type helper and bytecode-metadata normalizer
are removed, together with its unused duplicate prover-key-directory field.
Generic prover configuration remains consumed by its actual service owner.
The former metadata-normalizer test now exercises the current public payload
relation using the compiler's exact entrypoint descriptor, preserving its
account, asset, full-width numeric and byte-field assertions. That exact
production relation and migrated test pass a native component diagnostic
against the third daemon build's libraries
(`ec932ea7372e95eb1915717a2308cbc9bc71cb50ece309dea89cae40c19ee4f3`).
The fourth daemon build compiles the changed Torii package. Full Torii package
tests and unchanged-candidate qualification remain required.

The guarded canonical memory-node and bounded key-decoder cuts are now applied
(13 and 15 files). Canonical memory Merkle nodes retain one exact backing array
and the original execution charge through in-place updates, reset and final
borrowers; the infallible ByteMerkle clone interface is retired. Binary key
decoding shares Norito's checked byte-sequence relation, validates within bounded
scratch, and allocates only the exact compact destination after validation.
Fixed typed rejection replaces temporary diagnostic allocations in that path.
JSON source strings, outer diagnostics, stack funding and private signing remain
separate open custody work. The first combined build catches a missing SEC1
encoding trait import at the remaining EVM-address consumer; that import is
restored and a new source capture owns the retry. The second build succeeds;
its first selection records 34 memory passes and 15 failures at the same telemetry
catalog-length assertion. The changed Merkle help text is now accompanied by all
length, SHA-256, BLAKE3 and semantic-ledger pins. Only that help row changes;
the 752-row/709-registered catalog guard and mutation self-test pass. A third
source capture owns the runtime retry. That unchanged-source retry passes all
191 IVM and four Crypto geometry controls on the normal stack
(`287b1687d1c8353ef86f8e085a64806cb017b5a9498d07f215fe1a69c0d3aabd`).
The fresh Crypto decoder target passes all nine controls across all 11 default
algorithms; Norito passes 487 tests with one existing ignore. Crypto run 08
retains four failed root assertions expecting the retired allocated error variant
(93 root tests pass); the sealed migration now requires the fixed typed error
and exact unchanged display text. JSON outer diagnostics and other behavioral
assertions remain intact. Its independent remaining selections pass, but the
failed run is not relabeled; feature and strict-Clippy reruns remain pending.

On September 29, the exact reviewed acceleration ownership (127 files), Swift
configuration/native consumer (20), original-pool write-log custody (eight) and
source-bound single certified traversal (12) are applied in `optimizations`.
Their file sets are disjoint and every pre/postimage is checked. Cargo resolves
only the new `iroha_accel` package and its three consumers into the existing lock
without changing any external package version. Fresh combined qualification
remains pending. The signer traversal keeps original authority predicates,
transaction deadlines and token lifetime; its effect on the failing end-to-end
publication workflow is not established by source review.

Authenticated complete State roots, full execution proofs, private invocation,
AXT finalized authority and durable spends, complete active memory funding,
production Musubi publication, signed hardware artifacts and exact-candidate
network/hardware/release qualification remain open. Production proof and AXT
admission do not become enabled by these component results.


### September 29 combined ownership and qualification follow-up

The write-log candidate 3 passes 201 IVM and four Crypto geometry tests with no
ignored controls and all 6,306 captured sources unchanged
(`a22ac89653111587df62c4ae825b11ebdce293dc2df87f7716abf3af73c5242e`).
Its previous interrupted and source-drifted runs remain separate failed evidence.
Crypto run 10 passes 635 selected default tests with two existing fixture-generator
ignores, 13 no-PQC controls and 16 GOST-only controls; run 11 passes five doctests.
OpenSSL smoke output still reports unavailable SM4-GCM/CCM providers, so positive
Rust summaries cannot close that provider gate. Strict Clippy exposed private
MV type complexity, documentation/private-module lints and one decoder-test
borrow; narrow fixes preserve algorithms and assertions. The feature-matrix
recheck remains separately recorded rather than relabeling earlier failures.

The shared acceleration owner passes all 30 tests. The combined run stops on a
configuration test accessing a private model field, followed by unrelated Core
source drift. The test now constructs the same canonical asset through its public
literal parser. Core's earlier six TON helper documentation/Copy errors have an
equivalent concurrent repair. A subsequent direct engineering Core test build
reaches Core but fails on test includes for the removed `sumeragi/v2_apply.rs`
and `state/queue_plan_priority_tests.rs`. Retired implementations are not restored.
Repeated whole-candidate captures also refuse concurrent documentation/config/SDK
changes; their failures are retained. Focused engineering observations do not
replace unchanged-candidate acceptance.

The four-file phase preparation cut consumes one source-bound snapshot for each
signer Check phase, preserving the existing signed check, Queue submission,
finalized verification, authority checks and deadlines. Independent source review
accepts the cut, and eight actual-capture/mutation controls are registered; current
runtime evidence is still missing. The finite read-log successor remains staged:
independent review found operational failures converted to invalid TLV, false
signature results, truncated INPUT scans and nested return decode errors. The
consumer correction extends through contract lookup, executor output and VRF
allocation. No incomplete cut is described as allocation closure.


The OpenSSL follow-up removes dependency-private `ossl300` branches that were
never enabled in `iroha_crypto`, directly using the pinned OpenSSL 3 provider.
SM3 and SM4 smoke baselines now disable the provider while computing the Rust
oracle, preventing self-comparison. Run 16 passes 72 SM, 37 GOST (one existing
fixture-generator ignore), six smoke, 11 allocation and nine decoder tests, plus
six shell controls, with no provider-skip diagnostics. Run 17 passes strict,
dependency-inclusive Clippy for default, OpenSSL, no-PQC and GOST-only builds,
and the codec guard. The reduced build's derivation API retains one exact
conditional lint expectation because only disabled algorithm variants can fail.
The newly reachable SM4 native-error path still needs canonical fallback and
quarantine before complete backend-failure parity can be claimed.

The target-aware 26-file packaging cut is applied with exact guards. Canonical,
Docker and Nix daemon producers select the target's backend and require the
independently reviewed CUDA public fingerprint; the single V1 prebuilt record
binds resolved production IVM features and the exact source bundle identity.
Retired records are rejected. All 160 applied packaging controls pass with
Python 3.12 throughout child tools; the first attempt retains 29 environmental
failures caused by the system Python lacking TOML support. The staged policy
and pipeline checks add 27 and two passes respectively, without pretending to
be actual Nix/Docker/Cargo release builds. The whole-candidate trusted source
seal is intentionally unchanged. Qualified signed PTX, signer identity,
hardware counters/parity/performance and frozen-source release evidence remain
open (`9fca177b212471a228e1f9be3a53d5f91a4075e8ecaba2eb63d71c50869d4453`).
