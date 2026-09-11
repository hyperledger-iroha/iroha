# Peer identity compilation boundary (2026-09-10)

`iroha_model_base::peer::PeerId` owns the public-key identity wrapper, canonical
binary/JSON codecs, JSON object keys and slice decoding. The ledger `Peer` entity,
address/identity composition and registry membership remain in the aggregate.
Superseded aggregate, SDK and executor facade exports are removed. Crypto remains
the algorithm and public-key owner; no opaque handle or ABI version is added.

The initial coherent application covers 332 paths. It includes nine owner files,
282 caller files, three dependency manifests, the previously reviewed aggregate
documentation cleanup and the coordinated repository documentation. Overlapping
images were composed before application and every live precondition was checked.
All three original identity tests move with the owner; eight entity/mixed tests
remain. Six retained private-field accesses use the existing public-key getter,
preserving their compared values and assertions. The caller inventory retains
40,255 functions, 8,858 tests and 39,880 assertion macros.

All seven captured PeerId envelopes remain unchanged within the 189-row fixture,
SHA-256 `9eaba83f63302101a16a324d07bd85bd316dfc1cea8eca7bbafb876e0b57342a`.
The first owner run passes 80 tests and fails the new complete-schema comparison:
the minimal crypto build omits the BLS/GOST/SM2 algorithm enum variants present
in the original capture. Test-only crypto features now select those exact schema
variants. No fixture, assertion or codec implementation is changed to accommodate
that failure. The separately rebuilt minimal production artifact enables only
crypto `json` and `mv`; it excludes those additional test features.

All four owner configurations (default, transparent, FFI and combined) then pass
81 unit tests and three allocation tests. Default/transparent bind unchanged
source `396071fdb8dd32b2fefe0a1d68700b19e55df2102e3ec3a8a212eedbf97a89ac`
with 2,329 selected inputs. FFI/combined bind unchanged source
`fe2cd3331f28277e23bf04e667456d902875ac5630ff16e2f76150ea232c61f4`
with 2,415 inputs. Strict owner library/test Clippy and the three private-field
compile-fail examples pass on the latter source. Runtime tests use ordinary
stacks; allocation tests retain their single-worker allocator accounting.

Base promotes crypto to a normal dependency with its minimal JSON surface and
forwards FFI exports. P2P promotes base to normal. SCCP uses an optional normal
base edge in its existing fixture feature and an explicit development edge.
The root lock changes only SCCP's dependency list, adding base; external package
identities, versions and sources are unchanged. Root lock SHA-256 is
`6df90b42c9b3a51109aaa66f7b1fddcefe7f5e7af4126a37beb76b3c345ac71d`.
Canonical lock-pin generation and checking pass. The initial pin invocation
rejects an OS temporary path with a symlink ancestor; the retry uses its resolved
external directory without weakening the owner guard.

Dependency budgets follow exact manifest measurements without policy exceptions
or extra headroom. The test-only schema selection leaves all shipping metrics
unchanged; workspace declared/required edges are 1,727/1,594. Manifest fingerprint
is `sha256:98ce3b5cddd3219e3a574c3df71898dad207ef4de1e7868101f822d640560737`.
All 20 feature-resolved normal/build boundary checks pass. Offline resolution of
the standalone fuzz workspace also passes with unchanged external identities;
its ignored local lock updates base/P2P dependency lists. No fuzz target has run.

Generated-input ownership passes against a private index with the three new peer
sources added to the prior reviewed index. The real index remains unchanged.
This does not execute the generators. Codec-retirement and diff guards pass;
120 dependency, source-budget and generated-registry guard tests pass. The source
budget retains the same 249 affected paths and 170 exceptions at this checkpoint.

The aggregate cleanup first preserves all non-comment tokens while correcting
114 documentation/spacing sites associated with 116 retained lint diagnostics.
A subsequent nine-file patch adds 38 precise error sections, preserving all
359 function bodies, 30 test attributes and 111 assertion sites in those files.
The 47-package all-target consumer check, including the SCCP fixture feature,
passes in 575.43 seconds on unchanged source
`265e6b90c5b4c0b93cd4c47a2a701ef2c173252225c4f1c4bf22d742b7090410`
with 9,283 selected inputs. Its 150 warning occurrences exactly match the prior
topology check by message and primary source path; no new warning is added.
The separate P2P all-target QUIC check passes in 134.36 seconds on source
`af6c385631c7652aa6228a98b8faac689ebe03e5567184ece79437ffef88fe1d`
with 4,107 unchanged inputs. These checks compile developer/UI harnesses but do
not execute their compiler subprocesses or network scenarios. Composed runtime/FFI
suites and fresh aggregate strict lint remain pending. Broader workspace, memory
and release qualification remain open.

The composed validation patch applies 17 disjoint files after checking every
before/after image and all 41 binding-stage artifacts. Eleven predicate diagnostics
retain their original field associations, length checks, signature/digest inputs
and first-error precedence. Nine numeric diagnostics replace implicit narrowing
with exact limb carry/borrow operations and checked wire/budget projections.
Sixteen new regressions cover independent coordinate mismatches, slot/roster
lengths, maximum-width arithmetic, the exact 226-byte response transcript and
65,536-byte body bounds. The independent numeric review checks full 19-limb
reference arithmetic and every legal response length without changing limits.
These static checks do not substitute for the pending Rust runtime suites.

Separate shipping checks now pass without test-target feature unification.
The SDK check takes 131.35 seconds with no emitted warnings on unchanged source
`63454276aca96f43c58a43828290774686d6c3b99e44bed0b1b4c1a9d8c027b9`
(3,964 inputs). Core, Torii, daemon and CLI checks take 326.57 seconds on source
`12cc54f67c63e8dff07c0a4108a4a9f50b0951a7914ff58d0973c8177c2e35f9`
(8,421 inputs), retaining 119 warnings. These are compilation checks, not node
execution or release qualification. The refreshed environment inventory retains
all 850 references and 210 variables; only source positions and its timestamp
change. Generated-input checking additionally covers the two new regression
modules through a private index. The real index remains unchanged.

The first composed test build passes. Its base runtime passes all 81 tests;
the aggregate runs 3,687 successfully and retains six ignored fixture printers.
The sole failure is the query-inventory Rust label for the moved `PeerId` owner.
The corrected row names `iroha_model_base::peer::PeerId`; all 28 canonical wire
identifiers and the full inventory assertion remain unchanged. All 16 new
coordinate and numeric regressions pass in this initial run.

A subsequent 29-path cleanup moves 19 complete KAGEMUSHA implementation blocks
into hardware, exchange and funding modules. All 204 original function bodies
remain exact, and the root falls from 6,237 to 4,332 lines. Source-budget findings
fall from 249 to 248, with all 170 existing exceptions unchanged. Private validator
phases, checked borrowing and expression cleanup preserve protocol constants,
wire declarations, first-error order and existing assertions; six further
regressions cover KAGEMUSHA qualification boundaries. Independent review verifies
the composed images and corrects one frozen rationale: the replay-nullifier
default remains **48 zero bytes**, not a nonzero sentinel.

The second production SDK and node checks pass. The second test build fails
because one newly added fixture iterates a slice by reference where the record
requires a copied enum. The fixture now uses `iter().copied()`; production code
and assertions are unchanged. Third source-sealed production SDK and node checks
also pass. The codec, generated-input ownership and historical-archive checks
pass at this checkpoint. No passing production check substitutes for a test or
release qualification.

The third composed build passes in 351.36 seconds. One unchanged source,
`d6dd99163c2cd732bf0650f25df70305b5546e840cab26b341ceeda029dec96b`
(5,674 selected inputs), passes all 81 base, 3,694 aggregate, 802 SDK and 386 Musubi
library tests. The aggregate retains six ignored fixture printers; Musubi retains
one ignored abrupt-exit worker exercised through its parent test. All 35 resolver
regressions pass on ordinary four-worker stacks. These runs cover the moved Peer
owner, the corrected inventory and every new numeric/qualification regression.
SDK/Musubi strict lint and documentation pass on this source. Base/aggregate
documentation passes on its separately selected dependency closure. Combined
`ffi_export,transparent_api` compilation and runtime pass 81 base and 3,694
aggregate tests, retaining the six ignored fixture printers, under source seal
`11de5202135f56c020d369437c48eff15c63aa6cb41e90478215fd1553400a29`
(3,877 selected inputs). Aggregate strict lint still fails with 17 diagnostics:
three predicate groupings, fixed-slot storage, named game construction, and
remaining validation phases. The subsequent staged corrections are not covered
by these passing results.

Evidence is under `target/architecture-redesign/model-base-extraction-v1/`:
`peer-application-v1`, `peer-fixture-feature-fix-v1`, `peer-base-*-2`,
`peer-base-clippy-1`, `peer-base-docs-1`, `peer-minimal-production-features-1`,
`peer-dependency-budget-v2`, `peer-boundaries-2`, `peer-generated-registry-index-v1`
`aggregate-error-docs-application-v1`, `aggregate-validation-application-v1`,
`parent-numeric-review-v1`, `peer-sdk-production-check-1`,
`peer-node-production-check-1`, `peer-generated-registry-index-v2`,
`peer-model-test-comparison-1`, `aggregate-model-cleanup-application-v1`,
`parent-model-cleanup-review-v1`, `peer-source-budget-comparison-3`,
`peer-test-fixture-copy-fix-1`, `peer-composed-batch-3` and
`peer-composed-runtime-summary-3`.
