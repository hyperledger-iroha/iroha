# Aggregate JSON consumer migration

This is an isolated candidate checkpoint. The ordinary JSON destruction fix
has not been cut over to the live workspace. Source stages, before-images,
compiler diagnostics and executable identities remain under
`target/architecture-redesign/model-base-extraction-v1/`.

Account JSON and object/storage keys now receive an explicit immutable address
format. Native account, role, permission, custom-parameter, governance and
metadata-event decoders borrow their input fields. System parameter decoding
preserves validation order and charges its nonempty algorithm default before
allocating it. Consensus custom carriers require context and return typed
errors; their external callers still require migration.

The event-frame macro retains canonical Base64/Norito bytes and terminal resource
errors. The EventSet generator now emits the checked JSON contract and streams
flags in the original order without constructing a temporary decomposition
vector. Existing schema identities and captured event frames are retained for
the pending aggregate test run.

Storage keys use the shared checked string escaper. Structural keys stream
embedded JSON without an intermediate document. AXT keys compare canonical
output directly against the supplied text; nested key decoders reject trailing
documents. SCCP hashes use fixed arrays. Account-bearing sponsor keys carry the
same explicit network format as their account owner.

Contract aliases and addresses now share their validated scalar and object-key
owners. Nexus lane decoding uses one field inventory for streaming and borrowed
input, preserving required-field order without allocating a default record.
Time records decode borrowed fields directly; identifiers, IPFS paths, backend
tags and log levels use their canonical validation and checked writers.

An independent token review of the 96 duplicate writer removals found no
unintended checked-body, schema or fixture changes. Aggregate test execution is
still pending; static review is not runtime qualification.

## Scoped executed results

- At source
  `b5fa1af1d101d31bf311f520afabeb46f1594adc32e390687810fdb8c2875985`,
  all 328 primitive library tests and strict all-target Clippy pass. This covers
  the direct canonical empty-object constructor used by ExecuteTrigger;
  validated trigger arguments are consumed without reserializing them.
- At source
  `469907decac5016091049ff87fd8c2114707f7d96793167ba6bae1f5afe5e55f`,
  all 548 Norito library tests pass, with one existing ignored test. The three
  added embedded-JSON tests exercise exact output/resource bounds, canonical
  escaping, explicit context, first-error preservation and independent depth
  accounting. Execution uses four ordinary-stack workers and the exact
  compiler-selected artifact.
- Strict Norito all-target Clippy passes at source
  `e62bb36633ce594e1b21b469656fc113f73baacac1e574f653dad11bcdc65a56`.
  This later source also exposes the same canonical object-key writer for
  storage adapters. It is not an aggregate lint pass.

- At source
  `4d81bb0d7f4be59c62e28542b8b490f536118e6141986c584be879ad636aea59`,
  all three crypto scalar identity tests and strict crypto all-target Clippy
  pass. The new allocator observer records zero actual allocations across
  128 valid Hash endings and six borrowed scalar/key paths. The shared Hash
  JSON validator now uses a fixed 32-byte buffer; valid checksum, uppercase,
  marked-bit and exact-width rules remain enforced.

## Aggregate compile checkpoints

Each check uses the complete composed source, locked offline dependencies and
the recorded compiler. Source hashes remain unchanged during each check.

| Check | Source SHA-256 | Result |
| --- | --- | --- |
| 5 | `7ee86b645bdb67b290ea91048cc39a863b0cf90954b70d25acbf6fd22475cdf7` | Failed: 993 errors |
| 6 | `e62bb36633ce594e1b21b469656fc113f73baacac1e574f653dad11bcdc65a56` | Failed: 674 errors |
| 7 | `e862da87abbb57e0071c64cccfbcb621b2c9c95cdc82bee872bb8d5a37a9ed8a` | Failed: 564 errors |
| 8 | `29f7f270591e3da0fce6b002de62c2472709d99fcf79e7abecbfc4399b537bf8` | Failed: 306 errors |
| 9 | `193dabda370d72751fa1f79f34312c97342eafde4fa23a5c09d8cc8a43a7667a` | Failed early: one Nexus macro tokenization error; corrected in the next source |
| 10 | `0c715c9dece52e2b5eb7a08596565924391ccf47fde6c1ecd5d7d85f56b4481b` | Failed: 290 errors |
| 11 | `5b5d81ff053b5ff580604ac5b7f909a71aba865a4a3ab916b090ece8ac2433bf` | Failed: 253 errors |
| 12 | `6a63a2fc1f23454dab77ce81a09c3b6eb7d521ad8c00e9a6f80b8f7cca7a3ddc` | Failed: 239 errors |
| 13 | `0a9ba5d05a41e63385caa7be455b3201fca79bb7248d447072eff116d5a347f3` | Failed: 212 errors, including proof field-binding and core-error integration mistakes corrected in the next source |
| 14 | `66f4df98dc47ed4e6db8df528724cd2a581010388812ccc2f55fdd56715894c1` | Failed: 180 errors |
| 15 | `5468ebb92824516b5d31fd325aa356eac8fd600530a124432143e64b53c29512` | Failed: 172 errors, including a transaction allocation-error constructor corrected in the next source |
| 16 | `91a491fd6834b99202677d46432b5125052eb5b16db30e46597435342516a18f` | Failed: 160 errors; shared canonical frame owners have no production diagnostics. The allocation diagnostic length still required an explicit usize/u64 conversion, corrected afterward. |
| 17 | `25dc565174c6a3f1f316f46a05a4f98cd0ec3080fd8be41c0790e50ed47f7d63` | Failed: 147 errors; Kaigi, X.509, entrypoint schema and bounded transaction-record production diagnostics cleared. |
| 18 | `39a874a668c04af70471e72e42d83c8a41cc2050bbb3f42c34bd23fe70b70e50` | Failed: 129 errors, including query API, private-owner and denied-cast integration findings; corrected in the next source. |
| 19 | `d74f985accdbc10bca40c08857d55aad9c1ecbb3912b9748ca25985da6af7302` | Failed: 91 errors; query and new block/consensus native owners have no primary production diagnostics. |
| 20 | `535055162dae20b0d6bacd67e5dcb378a37a889f03439f0796d6482a17dc7506` | Failed: 65 errors; confidential, DA encryption, registry and gateway production diagnostics cleared. |
| 21 | `79ee8caa92280b4d3cd655c59cad4d873f601bd144c4ead2417eb1ac910b01ba` | Failed: 41 errors; borrowed alias validation, Soracloud schema/config owners and canonical Ministry writers compile. |
| 22 | `eec6f4035d4546fda71eb812b92712b9cf24ca5b9c69fbf8c93e2786871f8e0c` | Failed: 25 errors; Nexus and oracle fixture production diagnostics cleared. Two concrete metadata ContextError conversions required an explicit JSON error wrapper, corrected afterward. |
| 23 | `24f7201e13ac4a795145e7b112769ce62f63e3894a5cdd1c37323d0a451ed643` | Failed: one denied missing-Copy implementation and two unused query warnings; fixed without changing a shipping decoder. All remaining carrier and contextual writer bodies type-check. |
| 24 | `24328744cd7051f50bafaaf3b30fcdbf16db180fcd0690419b17e380843ea0c7` | Passed: aggregate library check, zero errors and zero warnings. Unit-test compilation/execution and external consumer qualification remain separate and pending. |

Diagnostic counts are not a monotonic qualification metric: correcting early
macro and trait failures exposes additional method bodies to type checking.


Native wrapper, proof, generated builder, enum, transaction and instruction
owners now have prepared contextual decoders. Their new aggregate regressions
have not executed while the aggregate compilation remains incomplete. Review
also records intentional first-release rejection of short NFT JSON literals;
only canonical identifiers are accepted, without a compatibility alias.

Query owners now carry Context and Result through their builders and envelope
conversions. Both quadratic predicate ordering paths use an admitted index
permutation and O(n log n) sorting with original-index tie-breaking. The
independent permutation audit covers all 46,234 permutations of lengths 0–8;
Rust ordering tests and aggregate query fixtures still await execution. The
caller census retains 210 canonical references across 23 files plus 66 receiver
review candidates and three filter-with candidates; unrelated references are
excluded explicitly in the frozen addendum.

The subsequent base owner source
`10e5caa46b5f16f1f1ef9779a2ccca22d63e04ff90b92f7fc6f5e412dce85323`
passes all 109 base library tests and three metadata allocation tests on four
ordinary workers. Strict base all-target Clippy passes at check 21's source;
the base files remain identical. Name's MV key decoder now shares canonical
spelling and resource admission with its JSON key owner. Borrowed name
validation lets composed aliases check Unicode scratch and syntax without
allocating temporary Name owners; only the complete alias is retained.

The native gateway decoders retain their existing bare Norito Base64 format,
admit the decoded buffer, preserve resource errors and reject trailing bytes.
DA encryption forwards its context through native and prepared-tape decoding.
CID and moderation decoders borrow their actual fixed-width or label input.
GAR and Ministry delete their duplicate unchecked field writers. Existing
fixtures gain exact output, malformed input, budget and deep-input assertions;
aggregate tests still have not executed.

The Soracloud stage preserves all 83 existing tests and 72 assertion sites,
adding seven tests and 25 assertions. Twenty-eight equality assertions are
adapted to exact variant/field/reason matches because the owning validation
error now retains a typed JSON error. Valid workflow/config hash preimages
remain unchanged. Its compiler pass is limited to production owners; tests,
external callers and runtime qualification remain open.

Review corrected the unqualified RWA key migration before its runtime tests:
RWA composite keys embed raw lowercase hash text, whereas standalone Hash JSON
uses a checksummed uppercase literal. The corrected decoder preserves the
composite spelling with a fixed byte buffer and Hash's marker validation.
Native UAID decoding uses the same fixed-buffer approach. Nexus variable entry
counts now invoke the existing authoritative sequence/total/planning admission;
two exact-budget assertions include the newly enforced planning byte per entry.

The Musubi follow-up removes its duplicate JSON field traversal and
I105 encoder. It measures the canonical serializer with explicit Context and
returns typed resolver response validation errors. It introduces no global
address selection, checksum copy or compatibility wrapper. At source
`90bf2525cb8ff3b318b1aa7f16455b1ff54653bb8306ac3f0f7ee6413a72e19f`,
the rebuilt codec passes 558 library tests and all four actual allocation/drop
tests on four ordinary workers, with one existing ignored diagnostic. This
qualifies the new length measurement and public sequence-admission boundary;
strict codec lint for that source and external caller migration remain open.

Nine custom-parameter carrier owners now accept Context and return Result;
unrelated IDs remain distinguishable from malformed or resource-failed input.
All 122 original tests and 476 assertions remain, with ten added regressions.
The separate literal review preserves every original literal token in order
across those 122 test bodies. The caller inventory records 213 direct or
receiver candidates across 32 files, including Core's retired Hijiri error
match. Existing binary fee snapshot error-to-invalid-commitment rules remain a
distinct unresolved seam; this JSON stage does not qualify them.

Direct enumeration of every Rust source in both model package directories
finds 678 files and no 5,000/3,000-line violations. This uses the authoritative
test-path classification and line counter with no Git-ignore filtering or
size exceptions; it is not a workspace-wide budget pass.

TODO: Complete the remaining native decoders, canonical writer callers and
external context migration, then execute aggregate fixtures and integration
suites. A reduced diagnostic count does not qualify unexecuted code. Backend
and final-tree allocation ownership, dependency/crate extraction, measured
memory reduction, complete workspace, native/device and four-validator gates
remain open.


The following strict codec all-target Clippy run passes with zero diagnostics
at source `add0472faaf859ee4b212095636aca41ef7a9f7db5718906b8c49a9143b55de5`;
its Norito source matches the preceding 558-library/four-allocation runtime.
The source seal remains unchanged throughout the 8.82-second lint command.

Actual aggregate library-test builds expose distinct migration layers. Build 1
at `26bc6f69a745e116911169a1a4031a57376c6e03090d45c67801a77234a42d82`
fails on 132 initial macro/schema errors. After explicit fixture-context and
macro migration, build 2 at
`8bd0621af57a167c2236d77ee656c7e8550152ce1287e3fd2ee469ea8d1648a3`
fails on 326 name/import/trait-resolution errors, mostly retired JSON entry
points in tests. Both source seals remain unchanged; neither executes tests.
These counts cover different compiler phases, not a runtime regression count.

The subsequent staged fixture migrations retain the original wire literals,
assertions and fixture inputs. The identity stage preserves all 16 tests and
119 assertion sites, the Soracloud test stage all 299 tests and 485 assertions,
and the privacy/confidential stage all 166 tests and 940 assertions. Explicit
address contexts replace ambient setup in identity/digest regressions. Dedicated
tests for the still-shipping AccountId Display/parse/guard APIs remain until
those production APIs and their external callers are actually replaced; this
fixture work does not imply complete global address API removal.


Library-test build 3 reaches type/ownership checking and fails on 475 errors at
source `41a3a125e220c2beb3668cf38ef825e3b56a7610dd189f20098590111c65b278` after
207.89 seconds, with an unchanged source seal. Most errors are
missing explicit contexts in previously unresolved fixture groups. The remaining
errors include checked JSON payload construction, resource-scope error types,
and native Value destructuring after iterative Drop became authoritative.
The following corrections borrow native values and preserve exact structured
errors; they do not remove the iterative destructor or increase stacks.

Review also identifies incorrect receiver attachment in earlier fixture edits:
eight expected JSON-result unwraps were attached to Context, and one carrier
unwrap was applied after borrowing. A separate frozen correction fixes the
actual receiver. Token-preservation evidence alone did not establish type or
receiver correctness; the failed compilation is retained. Canonical Soracloud
config hashing now reads the returned validated Json's exact stored bytes.

After the complete fixture-context/type composition, direct measurement still
finds all 678 model source files within the existing limits, without exceptions.
Repository diff whitespace and exact history/archive verification pass. These
checks do not qualify aggregate test execution or external SDK/node consumers.


Library-test build 4 finishes with eight errors and one warning at source
`240ef465f5f7406a9555a7ffc38f2ac48e4cb2abea9cdac8db1b121469b840e8`
in 207.88 seconds; its source seal is unchanged. All eight failures are duplicate
unwraps introduced while adding Context to existing fallible object fixture
constructors. The follow-on correction retains each constructor's original
single descriptive unwrap. The warning is one unused test-only trait import.

An independent source review of the 21-file typed-ownership stage verifies all
321 test names, 1,001 assertion sites and unchanged schema attributes. It checks
actual receiver binding, borrowed native Value lifetime, concrete empty vector
inference and typed resource scopes against the current APIs. It finds no
concrete source blocker; this review does not replace runtime qualification.


Library-test build 5 **passes** at source
`e642c881fbf9c7a8cd82b2f94dc2786c4ad81dec8dda841f6257b0357c0dfe0e` in
155.26 seconds with an unchanged seal. It reports the same
single unused test-only import; the cleanup is frozen separately. The exact
compiler-selected executable is now running the complete aggregate library
suite with four ordinary workers and no RUST_MIN_STACK override. Compilation
alone does not establish that runtime suite's result.
