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

## September 12: complete aggregate library execution

The exact build-5 executable completed with **3,860 passed, seven failed and
six ignored**, using four ordinary workers without a stack override. Runtime
was 95.14 seconds (97.81 seconds including process startup); the source seal
remained `e642c881fbf9c7a8cd82b2f94dc2786c4ad81dec8dda841f6257b0357c0dfe0e`.
The executable SHA-256 is
`e68c7ea198b87fd4c1b9ea799dbe00c297a012f3e7afde0369ec29504b4cc03b`.
This execution did not overflow the stack. It is a failing suite, not release
qualification: concrete identity and Nexus captures differ in payload bytes;
five other cases expose depth-error, key-error, parser-error, hex-case and
allocation-charge expectations. Captured fixtures remain unchanged while the
wire differences are traced to their actual source owners.

The follow-up retains canonical depth details and parser errors, checks the
fixed-width byte-key owner's uppercase output while accepting both cases, and
accounts independently for the signed argument's sequence-planning and final
buffer charges. The original allocation rejection remains, with an additional
exact final-buffer rejection boundary. These are assertion changes following
inspection of the owning implementations, not weakened production limits.
The public manual-frame, JSON-key and query-allocation targets now carry
explicit contexts; their existing allocator definitions and measurement
windows remain unchanged. Fresh compilation and execution are pending.

The next build compiles the library and four explicit public codec targets
with **zero diagnostics**, source
`7a575789805e8fd84d883ff53136b8153d44307e96a5b007771a3aae313656b2`,
in 375.97 seconds with an unchanged seal. Public execution passes both
block-signature allocation cases, the query allocation case, all six JSON-key
cases and ten manual-frame cases. The remaining manual block-signature capture
fails for the same input cause as the two library captures: production commit
`1729d177d4a4359dcbdc22cb4eae9c2b40107412` changed the genesis confidential
policy digest after those captures were produced. The repair makes their
recorded input explicit before hashing/signing, preserving current production
defaults and all captured bytes. Static byte/checksum analysis reproduces all
12 changed concrete frames and the observed Nexus checksum from this single
policy substitution; compiled execution of the repaired capture is pending.

The declared grouped integration targets now use explicit fixture contexts.
Independent review retains 183 test names and 427 assertion sites across the
31-file migration and checks 208 calls plus eight JSON macro result bindings.
Checked permission-payload retention replaces the removed infallible Value
conversion. Query envelopes receive context at conversion, and account-string
expectations use the same explicit namespace as their codecs. Direct
measurement finds all **679** current model Rust files within the unchanged
5,000/3,000 limits, without exceptions or ignored source.

The codec retirement guard previously resolved an enclosing Git checkout
instead of the isolated source tree carrying the script. Its staged correction
uses that script's source root and searches explicitly owned Rust roots even
when an enclosing repository ignores the snapshot. All 29 existing and new
cross-checkout/snapshot tests pass; review found an additional basename-script
invocation case, covered by a separate follow-up. These checks do not substitute
for execution of the guard on the complete candidate.

The first complete declared-test-target build finishes in 172.03 seconds at
unchanged source
`a542a98415d73c2e65cbae64b991005cd18fc41ec73fb41e625104e84efa5488`.
Its five actionable diagnostics all arise from one remaining old-style
predicate builder in `tests/data_model.rs`: the fixture now uses the fallible
builder, explicit context and the same existence predicate. Its existing wire
and retained-JSON assertions remain. The next build requests all declared test
targets with `--keep-going` so independent diagnostics are gathered together.

The fault-injection API now propagates malformed-overlay and encoding errors
before updating metadata. Both external and sealed-reveal committed entrypoints
have direct regressions for unchanged payload, cached identity, proofs and
result; empty additions preserve even an invalid overlay. These tests remain
pending feature-enabled execution.

The complete guard correction passes **31 tests** with pytest 9.0.3, including
clean and adversarial basename invocation, unrelated caller checkout, ignored
nested source snapshots, and the existing fail-closed grep fallback. The actual
corrected guard also passes all four retirement checks on the isolated
candidate. The history verifier reconstructs 64,736 records and 67,311
occurrences. These successes are scoped checks; full candidate, feature,
workspace, memory, native and four-validator qualification remains open.

Build 8 did not compile: this Cargo version rejects `--keep-going` for
`cargo test`. The unchanged source seal is
`0701f2fad07260a176b2e04a139e44f56ffad648d110034d2dd54c8433222258`;
command validation failed in 2.70 seconds. Build 9 uses the supported
`cargo test -p iroha_data_model --tests --no-run` invocation and includes the
fixture-generator context migration. No result is inferred from the rejected
command or from its lack of compiler diagnostics.

The candidate's authoritative dependency checker passes all 20 configured
selections, including five SDK and five storage-client shipping normal/build
graphs. Explicit manifest/config paths and recorded Cargo-tree hashes bind the
check to the isolated candidate; 107 manifests and its lockfile remain
unchanged. This proves absence of the configured forbidden paths, not complete
architecture ownership. The audit finds remaining storage CLI feature bundles,
inherited SDK TLS defaults in alternative storage selections, an unselected
SDK `ids_projection` feature, and missing policy owners for most reachable
workspace packages. The storage policy still permits configuration and
telemetry dependencies; the reachable Halo2 feature surface also needs ownership
review. These gaps remain open rather than being inferred away from the
configured checks' passing result.

## September 12: all declared default model harnesses pass

Build 9 failed in 8.11 seconds at unchanged source
`2cd2428154c40e4e54a3f66b6e3672c3bf2a3c481e60c34d843069a7f16b9848`: nested game fixture macros still used the retired JSON constructor.
Build 10 gathered all remaining targets with `cargo build --tests --keep-going`
and failed in 500.66 seconds at unchanged source
`c25e56fa2139e598879fcefcac3758fa7f150b8dd3f79853440232b6c61599b6`.
Its seven remaining grouped-target diagnostics were migrated at their actual
Musubi, metadata and parser owners; six other test executables were emitted.

Build 11 then compiled all seven declared default model test harnesses with
zero diagnostics in 19.71 seconds at source
`8709508537b2b8a915485a656094df93eb90e27727f1726439f17f1387ad1882`.
The library passed all 3,868 tests on ordinary worker stacks. The complete
seven-harness execution retained one failure: an inline block-header golden
also inherited the newer confidential-policy default. Its builder now uses
the same explicit recorded input as the other captures. The inline golden
bytes and separate current-default roundtrip remain unchanged.

Build 12 compiles all seven harnesses without diagnostics in 19.05 seconds.
The exact compiler-selected executables pass **4,172 tests, zero failures and
13 existing ignores**. Counts use each parent harness result, excluding nested
subprocess summaries. The library runs with four ordinary workers in 99.80
seconds; no stack override is present. Both build and execution retain source
`44e2a5f2107768df80202f75e8dd6a217a98885ec686f3322709b58b3050c08e`.

| Harness | Passed | Ignored |
| --- | ---: | ---: |
| Aggregate library | 3,868 | 6 |
| Group 01 | 101 | 2 |
| Group 02 | 183 | 5 |
| Manual frame identity | 11 | 0 |
| JSON object-key contract | 6 | 0 |
| Block-signature allocations | 2 | 0 |
| Query allocations | 1 | 0 |

Exact commands, source inventories, executable/runtime-library hashes and
per-target logs are bound by `asset-model-tests-build-12.json` and
`asset-model-all-runtime-2.json` under the composed candidate. These are
default-feature results; HTTP, fault-injection, fixture generators, benchmarks,
external consumers, strict aggregate lint and full release qualification remain
separate gates.

## September 12: storage TLS selection and archive ownership

Taikai packaging now belongs to CAR's `manifest` library feature. The CLI and
orchestrator request that capability directly. The three packaging JSON calls
carry explicit context; output borrows the metadata map instead of cloning it.
Existing assertions are retained and a metadata readback test is added. This
stage has graph evidence but does not yet have CAR compile/runtime evidence.
The remaining eleven CAR command binaries retain unique capabilities that must
move into CLI ownership before their feature bundles can be removed.

The composed storage TLS change disables inherited defaults at the SDK, CAR and
orchestrator dependency edges and forwards the selected backend/root source
through every HTTP owner. Defaults consistently select Rustls with native roots.
Archive-only CAR (`--no-default-features --features manifest`) selects no TLS.
All 20 configured boundary graphs pass, including ten strengthened SDK/storage
selections. Required features are checked per resolved package instance; mixed
backends, wrong root stores and missing `gost`/`sm` features are rejected.
Metric limits, baseline, ownership layers and Cargo.lock remain unchanged.

The TLS stage manifest is
`4bc60895bbdccfa1fdeb94d68eefe8041ea10699f847ba89682a365fd650e131`.
Its complete Python run passes 105 cases and fails seven repository-integrated
release-provenance cases: the prior release checker borrows an enclosing Git
index for a copied source snapshot. All 65 dependency-budget cases pass. The
source-root defect is reproduced against the original checker; neither the
release seal nor a runtime TLS result is inferred from the graph checks.
Full ownership coverage and CLI/configuration dependency migration remain open.

## September 12: feature execution, generator identity and storage retention

The first HTTP/fault-injection/test-fixtures/dev-tools/Exact12 build failed in
259.67 seconds at unchanged source
`cf078b5c17f6076caca093099a92efc11d4443e71cc5d3e4a84c31082a37922d`.
Its only actionable diagnostic was a new regression fixture importing sealed
transaction types from their parent rather than their canonical `signed` module.

After that import correction, feature build 2 compiles all declared model tests,
five generators and four benchmarks with **zero diagnostics** in 342.17 seconds.
All twelve test harnesses pass **4,207 tests, zero failures, 13 existing ignores**.
The library contributes 3,880 passes in 96.93 seconds with four ordinary workers;
the 21 manual-frame cases include HTTP captures. Signed-overlay failures preserve
external and sealed-reveal payloads, cached identities, proofs and results.
Both build and runtime retain source
`404d191d783dbce3bef5ba607151a0729b1e1a3380e0f0f1de53ca220d029f29`.

Read-only generator execution passes Sumeragi/Native AMX, cancel-asset-lock,
Exact12, and the two exact Musubi stdout-envelope comparisons. AXT alone reports
a stale envelope; the descriptor and Poseidon documents are unchanged. A new
end-to-end generator regression compiles without diagnostics in 150.24 seconds
and reproduces the mismatch at source
`5d4a5cf0b2202d831c6140a2ab531117801ef324ed8ed1b7edacd7739b5d1f1c`.
The existing account fixture regression passes; the new full-document case fails.

This discrepancy is an intentional protocol change predating this restructuring:
commit `1e762a4c14e255eb328b2f253d1679f46e85f1c6` changed the generator and
FASTPQ owner to the sole accepted `fastpq-state-transition-stark-v1` parameter.
The checked-in capture still encoded `fastpq-lane-balanced`. Exactly six proof
arrays differ, representing two unique payloads repeated in proof and handle
records. Independent reconstruction changes only the parameter text, four
affected length prefixes, frame length and CRC64. Payload sizes grow from
771/706 to 783/718 bytes; accounts, signatures, asset IDs, descriptor/schema and
every other fixture byte remain identical.

The reconciled file is the exact Norito generator string recovered from the
failed assertion, SHA-256
`bcc87c0cb699f4a096a0cb2574421af9cc8bea9098576dfea6cbcea5e69cc21e`.
It was not reserialized through another codec. Stage
`axt-envelope-canonical-parameter-v1` adds an ordinary envelope-suite assertion
for the sole first-release parameter. Production policy is retained; no retired
parameter lookup or compatibility path is added. A fresh generator/test run is
required after composition.

Full strict model Clippy stops at seven existing SoraFS dependency design
findings in 34.28 seconds at that same unchanged source: three oversized proof
argument lists, two unit-error results, a large signer-state enum and a nested
conditional. This is a failed full lint gate, not a model lint pass. Dependency
repairs and a separately labeled model-owner diagnostic inventory are in progress.

CAR's production lexical JSON boundaries now carry explicit contexts. Its new
`ScoreboardMetadata` privately retains an independently validated, bounded
`Arc<Value>`. Clones share immutable metadata; persistence borrows it. Native
retention and the shared owner are charged to active decode budgets. Five new
regressions cover 32,768-level input rejection, owner allocation admission,
independent source ownership, shared configuration clones, exact nested output,
and enclosing-depth rejection before filesystem creation. Existing tests and
wire assertions are retained. Compilation/runtime evidence is still pending.

Storage options use the same owner, assemble gateway annotations before
retention, and propagate a typed metadata error before fetching. Storage, the
Rust CLI, CAR and orchestrator command producers, and JS/Python native bridges
use the canonical constructor and borrowed persistence. Their existing payload
producers remain intact. These six consumer-file migrations are composed;
external/native execution remains unqualified.

## September 12: explicit release source ownership

The release checker now rejects a copied source root before querying an enclosing
Git index or reading its manifests. An actual owned checkout or nested repository
remains supported. Git environment redirection, malformed/escaping index paths,
symlink-parent escapes and unavailable roots fail closed. Compiler snapshot
provenance remains a separate boundary from clean-commit release qualification.
The bootstrap helper digest is reconciled to the exact changed checker; the
reviewed source-seal value and stable source/artifact readers are unchanged.

Complete affected Python suites pass **174 cases and fail seven**. All 24 new
root/path regressions and all 110 profile/prebuilt cases pass. The seven
repository-integrated checks explicitly reject this copied source without its
own Git worktree; they remain unqualified. No seal exception or successful
release result is inferred from the passing unit suites.

## September 12: reconciled generator execution and archive transport boundary

`asset-model-feature-targets-build-3` compiles every model test, binary and
benchmark with `http,fault_injection,test-fixtures,dev-tools,privacy-exact12-conformance`
in 325.14 seconds without diagnostics. Its before/after source SHA-256 is
`12fd41dd36dbad343f43043e159902bb25f660b5b4692410e57ae3d853c1f7ce`.
`asset-model-features-runtime-3` executes all 12 compiler-selected harnesses:
**4,208 passed, zero failed, 13 existing ignores**, with ordinary stacks and four
test workers. The library passes 3,880 tests in 96.89 seconds; the AXT generator
now passes both its seed and complete fixture-identity regression.

`asset-model-generators-runtime-3` passes all five generator checks on that same
source and recorded executable/runtime-library identities. AXT, cancel-lock,
Sumeragi and Exact12 use their check modes; Musubi's stdout-only fixture set
exactly reproduces both expected documents. Source bytes remain unchanged.
This closes the identified AXT fixture mismatch without restoring a retired
parameter or rewriting unrelated capture bytes. It does not qualify release
artifacts or build-memory reduction.

The preceding `car-manifest-check-1` fails archive-only library compilation in
111.01 seconds: the constructor references Reqwest certificate/root APIs that
are absent when no TLS feature is selected. A CAR development dependency on
the upper-layer orchestrator would otherwise reactivate TLS during its tests.
The staged correction rejects missing HTTPS support with a typed capability
error before provider iteration or DNS, gates certificate APIs on the explicit
backend, and moves the entire orchestrator manifest-verification test suite to
its owner. HTTPS-only operation, pinned-root trust, public-address enforcement,
disabled redirects and deadline bounds remain intact. Both feature variants
and the removed development edge still require qualification.

## September 12: manifest ownership and actual archive-only execution

Five reviewed manifest stages introduce one borrowed PoP presentation context,
structured signer digest errors preserving codec causes, and a heap-owned
completion row with an explicit unchanged wire projection. The complete original
384 assertions remain, with 16 additional assertions. State tags, field order,
schema names, proof verification order and signed transcripts remain unchanged.
The projection tests cover all ten layouts and charge the new owning allocation
before construction. External Core/node/Torii/daemon callers are migrated but
their execution is not qualified by the manifest suite.

`manifest-design-tests-build-1` compiles all seven library, integration and
generator test targets without diagnostics in 94.07 seconds. The source SHA-256
is `52ff66a87ccc25a622f130a16c02feb15e88e34b8cc0c686d9f91b06237ec76a`.
`manifest-design-tests-runtime-1` passes **1,086 tests, zero failed or ignored**:
1,012 library, 62 integration and 12 generator tests, on ordinary stacks. The
library completes in 205.74 seconds. Strict all-target Clippy then finds four
test/generator diagnostics: three complex unnamed types and a redundant string
borrow. These are corrected without removing assertions or adding allowances;
the rerun is pending.

The same source passes `car-manifest-check-2` without diagnostics in 80.96
seconds, with `--no-default-features --features manifest`. The lockfile was
regenerated by Cargo; its sole difference removes the upward CAR-to-orchestrator
development edge. The complete old manifest-transport suite moves to an explicit
orchestrator-owned target with unchanged tests and assertions.

The first complete CAR test compilation still fails at 22 lexical JSON calls in
two CLI-consumer files; those calls are migrated without changing registration.
`car-manifest-lib-build-1` independently compiles the actual library harness in
216.73 seconds with no diagnostics. Its compiler-selected Reqwest artifact has
only the `blocking` feature, proving TLS was not silently restored by a dev
dependency. `car-manifest-lib-runtime-1` runs all 330 library tests: **329 passed,
one failed, zero ignored**, in 5.43 seconds with unchanged source and executable.
The new missing-TLS regression and all immutable scoreboard ownership/persistence
regressions pass. The remaining source-root test expects invalid root count/size
to be rejected consistently before backend selection. The corrected constructor
performs that bounded validation first, then rejects missing TLS before provider
iteration or DNS. DER parsing remains selected-backend-only; no plaintext or
implicit trust-root fallback is introduced. Both feature variants require rerun.

The separately scoped `asset-model-owner-clippy-1` reports 14 model diagnostics
in 166.38 seconds at source
`12fd41dd36dbad343f43043e159902bb25f660b5b4692410e57ae3d853c1f7ce`.
The corrections separate ordered limit decoding from instruction dispatch,
preserve allocation/error precedence, name the borrowed parameter conversion,
export the public predicate parser's structured error from its owning module,
and remove redundant syntax. A new limit-decoder regression covers ordered
values, optional quantities and allocation admission before malformed entries.
These edits remain subject to the full model lint and runtime gates.

## September 12: both CAR transport variants and manifest lint pass

At unchanged source
`7fb9589ea78efb462765881afaa2d7bd05a381f944a3e8201a62f3ae9c795d09`,
`manifest-design-clippy-2` passes strict all-target Clippy with `dev-tools` in
19.66 seconds without diagnostics. `asset-model-features-clippy-2` reaches the
generator targets and fails only two AXT diagnostics in 176.81 seconds: an
assigned clone and the oversized envelope assembly function. Those are corrected
by sharing the handle-proof inventory and reusing its string allocation; the
complete fixture identity regression remains required. No fixture bytes change
in this follow-up.

`car-manifest-lib-build-2` builds the actual no-TLS library harness without
diagnostics in 154.07 seconds. `car-manifest-lib-runtime-2` passes **331 tests,
zero failed or ignored**, in 5.45 seconds. The compiler-selected Reqwest unit
again has only `blocking`, and the artifact's CAR features contain no TLS
backend. Root count/size validation and missing-backend-before-provider tests
both execute. Every scoreboard retention, allocation and filesystem-boundary
regression passes.

`car-native-tls-lib-build-1` builds the Rustls/native-roots variant without
diagnostics in 14.75 seconds. Its runtime passes **330 tests, zero failed or
ignored**, in 5.63 seconds. The root-bound regressions execute again with the
configured backend. Both runs use four ordinary workers and verify the same
source seal plus their distinct compiler-selected executable and runtime-library
hashes before and after execution. They qualify the CAR library profiles only;
the higher orchestrator, CLI and native consumers remain unqualified.

`dependency-boundaries-final-car-1` passes all **20** feature-resolved normal/build
dependency selections after the lockfile change. The direct source-size scan
still covers all **679** model files with zero violations and no exceptions;
its evidence SHA-256 is
`609f4dd97442eda99910fa7d6dc84dc2ac6561761417ad0decc266ebf8957a7f`.
All four candidate codec guards pass, and the historical archive verifies
64,736 records and 67,311 occurrences. Full workspace, build-memory reduction,
release/native/device and four-validator qualification remain open.

## September 12: model test ownership and generator qualification

`asset-model-features-clippy-3` reaches all-target test compilation and reports
41 test diagnostics. The corrections retain the existing assertions and cases:
shared native-codec checks borrow their inputs, fixture construction and semantic
checks have named owners, and test modules follow their production items. Seven
oversized tests are decomposed with token-expansion checks preserving their
original fixture values, assertions and execution order. No lint allowances,
larger stacks or source-size exceptions are introduced.

`asset-model-features-clippy-4` clears those findings and reaches three further
generator diagnostics: two clone assignments and a large negative-fixture enum.
The latter generates only the removed standalone payload-manifest variant; its
five unused variants are deleted, and its captured wire index is explicitly 5.
This changes neither the production decoder nor its rejection requirement.
The Native AMX generator reuses its existing validator/PoP allocations with the
same source values. The fresh strict gate, all model harnesses and byte-identical
generator checks remain pending for these final edits.

The direct size scan after the test decomposition covers all 679 model Rust
files with zero violations. Report `model-owner-size-lints4.json` has SHA-256
`e7e78c3830168d23042d35bd1bf2b6eccc02acebcb62b945b2baf9414f719124`.

`asset-model-features-clippy-5` identifies two missing oracle-example context
arguments and a shared test constant's effective visibility. The complete
keep-going run, `asset-model-features-clippy-6`, reports eight distinct remaining
errors in Musubi generators and two other examples (15 compiler messages with
duplicate target diagnostics and notes). The fixes borrow generator inputs,
preserve all original assertions, and pass explicit contexts in the examples.
Musubi route construction is divided into eleven capability helpers, with a
token-expansion audit preserving every original constructor, validation call,
serialization order and the twelve route entries. The generator rejects
unsupported arguments without echoing arbitrary terminal text.

`asset-model-features-clippy-7` now passes **strict all-target Clippy with zero
diagnostics**, including HTTP, fault injection, test fixtures, developer tools
and Exact12 conformance. It completes in 150.50 seconds at unchanged source
`291b2987c7c4d886f59856ee4e5779dc490d3a8cfb85b6f9693417d6a6c36897`.
The new full executable build and runtime/fixture pipeline are still in progress.

The separate `car-manifest-lib-clippy-1` passes strict no-TLS CAR library Clippy
without diagnostics in 181.67 seconds, at unchanged source
`b9d4ef009e544d8a82c0cdfe2f56421a2cb431604ad5da117cad3f33116d5dd0`.
This lint pass does not qualify the remaining CLI or orchestrator consumers.

## September 12: complete model candidate gate passes

The complete `model-final-qualification-pipeline-5` passes on the same unchanged
source `291b2987c7c4d886f59856ee4e5779dc490d3a8cfb85b6f9693417d6a6c36897`.
After the zero-diagnostic all-target lint, `asset-model-feature-targets-build-4`
builds every selected test, generator, benchmark and example without diagnostics
in 328.31 seconds. `asset-model-features-runtime-4` runs all twelve exact
compiler-selected harnesses with four ordinary workers: **4,210 passed, zero
failed, 13 existing ignores**. The library accounts for 3,882 passing tests and
six ignores in 97.23 seconds; the two grouped harnesses pass 101 and 183 tests
with two and five existing ignores. The remaining targets cover manual frames,
allocation admission, object keys and the five generators. Child-process result
lines are not double-counted in the total.

`asset-model-generators-runtime-4` passes all five fixture owners. Both Musubi
outputs remain byte-identical, including the SDK route document after its helper
extraction. AXT, cancellation, revision-4 Sumeragi and Exact12 check modes pass;
the negative standalone manifest keeps its original rejected wire bytes.
`asset-model-oracle-example-runtime-1` executes the compiled example and compares
every one of its ten JSON sections with the shared price/social fixtures: all
match. Source, executable and runtime-library identities are checked throughout.

The next candidate composes the reviewed storage CLI test ownership change:
all 97 original CAR tests are retained exactly once, with 96 test bodies unchanged
and all eleven viewer workflow assertions preserved. The viewer's nested Cargo
build and silent missing-Python pass are removed. Explicit feature-gated targets
and cross-package artifact requirements replace those behaviors, with five
artifact-admission assertions. Production CLI retirement remains outstanding.
The detached-signing example also replaces a retired textual asset selector with
the same typed canonical derivation used by Nexus fee configuration. These later
consumer changes require their own execution and are outside the preceding model
source seal.

## September 12: storage consumers and source-copy freshness

The first binary-free CAR integration run passes all 331 library tests and 14
of 15 integration tests. Its remaining manifest-digest assertion identifies a
stale constant: commit `3a406045534478e701804c2a02131008b4b5c349` added the mandatory
PoR root and updated the captured manifest, metadata and detached signature, but
left three runtime/spec digest constants unchanged. Independent artifact hashing
and signature verification confirm the current captured bundle. The correction
updates those constants and adds four checks for exact frames, artifact order,
metadata and signature binding; no fixture bytes are changed. Rust execution of
this correction remains pending.

The first detached-example execution exposed a Cargo freshness flaw in the
local composition helper: APFS source clones kept their preparation timestamps,
which could predate an intervening build. The helper now refreshes timestamps on
changed destination files. `compiler-freshness-audit-v1` verifies that the complete
preceding model build compiled all 27 model targets freshly; that runtime result
is unaffected. The later `detached-example-build-1` reused the old example binary.
After invalidation, `detached-example-build-2` recompiles the actual source without
diagnostics, and `detached-example-runtime-2` passes signature verification and
all eight output-field checks. No native SDK bridge parity is implied.

The proxy/configuration stages now use checked JSON sinks, canonical mode labels,
borrowed native values and structured codec errors. They remove the recursive
native-value clone at the orchestrator configuration boundary. Seventeen new
proxy/configuration regressions cover exact labels, bounded/redacted errors,
secret allocation admission, sink failure and deep native input. The first
orchestrator library check stops in lower consumers: ten configuration-base,
nine reward-metadata and 101 shared-Torii API errors. These are outstanding API
migrations, not passing proxy runtime evidence. Reward metadata now propagates
codec failures before returning an instruction; its focused qualification and
the explicit-context configuration reader migration are in progress.

`reward-metadata-clippy-3` passes strict all-target lint without diagnostics.
`reward-metadata-tests-build-1` builds the complete reward test harness without
diagnostics, and `reward-metadata-runtime-1` passes all **15 tests** on four
ordinary workers. Both reward and skipped-payout construction return the typed
codec resource failure before emitting an instruction when allocation admission
fails. Exact metadata scalar projections and the existing economic tests pass.
The redundant payout-ledger dispute factory and the CLI's unused treasury
argument are removed; callers use the existing model record constructor. The
CLI and orchestrator consumer edits still need their own runtime qualification.

The reward crate's direct Norito dependency is reflected by exactly one added
lockfile edge. `dependency-boundaries-reward-metadata-1` passes all 20 normal/build
feature selections, the codec-retirement guard passes, and the direct model
source scan still covers 679 files with zero violations or exceptions. The latter
report is `model-owner-size-current-consumers-v1.json`, SHA-256
`12c2b97f1be33a60baef949e610bea859294782cf17a6375214381f7a2fb7dda`.

The next source composes the explicit configuration-reader Context, its derive
emitter, and the checked shared-API scalar codecs. Closed enum errors no longer
echo arbitrary input labels; the empty-detail query rejects borrowed native
objects without copying their nested values. The one-instruction field checks
cardinality before allocating or decoding its elements. These shared API changes
remain unqualified until the surrounding configuration DTO migration compiles.

`gateway-storage-clippy-1` passes strict all-target Clippy for CAR and manifest
with zero diagnostics. `config-context-clippy-2` likewise passes configuration
base strict all-target Clippy. `gateway-config-tests-build-1` builds the complete
configuration/manifest/CAR test selection without diagnostics or source drift.
`config-context-runtime-1` passes **61 tests** (36 library, 18 reader/derive and
seven explicit-context contracts); `car-gateway-runtime-2` passes **331 library
and 15 integration tests**, including the previously failing published gateway
fixture identity. Both use ordinary four-worker harnesses and exact compiler
artifacts. The corrected constants describe the unchanged signed fixture bundle.
`manifest-gateway-runtime-1` also passes **1,090 tests**: 1,016 library, 62
integration and 12 generator tests, with all seven expected harnesses accounted
for and no failures or ignores.

Further frozen shared-status changes replace the allocating histogram decoder
with exact-cardinality admission and stack-array construction. Uptime previously
normalized or saturated malformed nanoseconds inconsistently, and could overflow
in an infallible path. The proposed checked JSON and binary decoders require
nanoseconds below one second while preserving valid tuple bytes and schema
identities. These changes entered the subsequent shared-DTO qualification.

The first shared runtime capture passes 320 library tests, both Android approval
fixture tests, and the status-wire harness covering all 32 captured records. One
new uptime regression fails because its test uses the physical archived-object
view for a compact 14-byte tuple with a 16-byte Rust footprint. The test now uses
the existing checked compact-field decoder; production decoding and fixture
bytes are unchanged. This first capture remains a failed suite.

The configuration owner now declares its 24 records and 130 fields once, with
checked writers and borrowed native-object dispatch. All 22 existing tests and
eight new context/closed-field contracts pass in that first capture. KAGEMUSHA
response parsing propagates structured errors and binds the caller's address
context; its six new context/resource tests also pass there. SDK callers remain
outside that runtime scope.

An independent receipt audit caught a proposed Box placement changing Norito
length framing. The corrected owner encodes the canonical receipt payload and
admits the heap allocation explicitly during checked decoding. Tests compare all
three response variants against the captured enum inventory across all ten
layouts, including frame and JSON identity and allocation accounting. Two
integration callers now explicitly borrow or consume the boxed receipt. The
nine extracted private-settlement fixture factories preserve seeded keys,
validation order, signatures and digest material; all original assertions remain.
The audit is `private-settlement-response-ownership-review-v1.json`, SHA-256
`c3a2c7d492005bae3a1b9e50bec33e452e2ab4d4b46f4dbee5162796b744ecc1`.

`shared-config-context-clippy-5` passes strict all-target Clippy with the Connect
feature and zero diagnostics. Connect examples use the canonical account parser,
validate exact invitation/session identities, and reject unknown wallet actions
before network access. `shared-config-context-tests-build-3` builds every test
target, including all four example harnesses, with zero diagnostics.
`shared-config-context-runtime-2` passes **341 tests** across all seven exact
compiler-selected harnesses: 332 library, three wallet, two app, one permission
preimage, two Android approval fixture and one status-wire test. The Open-dump
example compiles and its empty harness executes successfully. There are no
failures or ignores; source and executable identities remain unchanged, and all
workers use ordinary stacks. The uptime compact-layout regression, receipt
wire/allocation tests and Connect signature/substitution tests all pass.

The updated source-size report covers all 741 model/shared Rust files with no
violations or exceptions: `model-shared-owner-size-v2.json`, SHA-256
`a83b7fc7f75c63ed22f18ca5d6e6fc181f7a13441c1f2e20fb22b4f9b46241da`.
All four codec-retirement guards pass. Storage orchestration now has an explicit
context at its root privacy ingestion boundary and migrated integration-fixture
calls; its own library qualification is in progress. External consumers,
workspace, release, native/device and build-memory qualification remain open.

The following node-configuration stage migrates all 49 manual user codecs to
checked explicit-context writers and borrowed native-value readers. Its audit
preserves all 799 original assertions in order and all 234 public field/type/order
inventories across the 12 existing user files. Eight new tests add 51 assertions
for canonical labels, closed object fields, resource admission and native/text
equivalence. The review is `node-user-json-context-review-v1.json`, SHA-256
`f543fd67f228e1a1e3c15447d84573f6e98c801bccfd8ba5c9d45842f1ca44cb`.

Kura, logger and snapshot scalar checked decoders now propagate truncated-string
errors and include the string prefix in their exact binary length. Native wrong
type rejection borrows its input; a new regression exercises 32,768 nested levels
on ordinary workers. Nullable telemetry fields distinguish missing from present
null, reject duplicates before decoding replacements, and remain unassigned on a
resource failure. The configuration factories require a caller-owned Context;
the consensus commitment example derives its address capability from the same
TOML source and rejects wrong types or out-of-range discriminants.

The canonical FASTPQ user configuration is `kura.fastpq_artifacts` with
`artifact_bytes`, `artifact_count` and `total_bytes` ceilings. Its actual storage
policy and successful values are unchanged. The default artifact size now uses
the canonical `usize` proof limit directly, avoiding a narrowing roundtrip.
Taira CPU allocation is constructed with checked conversion and addition at
`defaults::taira::inrou_max_cpu_millis()`; the duplicate daemon constant is
removed and its callers migrated. These external daemon/CLI callers are not yet
qualified by the configuration-only checks.

`node-config-context-lib-check-2` passes with zero diagnostics. The successive
strict lint captures retain their failures while closing removed helper calls,
field-presence design, checked resource defaults and fixture ownership.
`node-config-context-clippy-4` passes strict all-target Clippy with zero
diagnostics and unchanged source. Configuration runtime, telemetry and
orchestration qualification remain pending at this checkpoint.

`node-config-context-tests-build-1` then builds all configuration test targets
with zero diagnostics. The first runtime capture passes 891 tests but fails two
snapshot checks because the standalone runner lacks Cargo's manifest directory.
The runner now supplies the exact candidate manifest and workspace roots,
records them, and clears snapshot-update mode. This avoids resolving fixtures
from the enclosing live checkout. No test source or executable changed for the
repeat: `node-config-context-runtime-2` passes all **893 tests**, with no failures
or ignores (656 library, 229 integration, six Taira contracts and two consensus
commitment example tests). Both immutable snapshot comparisons now pass.

Strict lint, test build and successful runtime share source SHA-256
`918250abb9b3965264fc7eabb85795987ea695e397c4248a213db4f7b12cc955`,
covering 20,192 files and 13 recorded deletions. The successful runtime report is
SHA-256 `345ccd9700de360649c9b35b8dc5e7a7f610557ea954a351f638c0f62e90a79d`.
This source also passes the deep native-value, truncated scalar, nullable field,
allocation admission and configuration-context tests on ordinary workers.

The next orchestration source stages only pending payout outcomes and per-relay
accounting snapshots before committing a batch. Duplicate epochs, later metadata
failure and aggregate overflow therefore return errors without partial payout
state. Reward construction propagates canonical metrics codec failures;
enumerated filesystem logging failures retain their existing best-effort policy.
All 117 prior assertions remain, and six new tests cover rollback, retry and
mixed-relay/nonmonotonic batch equivalence. Its static audit is
`orchestrator-payout-preparation-review-v1.json`, SHA-256
`c3d26643d96e1542ee0e245a0e1817e2e5520e005b686486c3cdda3d36be67e1`.
Those orchestration regressions and telemetry feature qualification are pending.


### Observability and orchestration qualification, 2026-09-12

Telemetry redaction now traverses mutable child iterators with explicit ancestry,
without recursive rebuilding or a work list proportional to a wide container.
String truncation reuses the existing buffer and preserves UTF-8 boundaries.
Secret subtrees use the ordinary iterative `Value` destructor. A failed ancestry
reservation replaces the complete field with the redaction marker. Tests exercise
32,768-level mixed trees, wide-buffer ownership, and actual thread-local allocator
rejection at both initial and later ancestry reservations. No production failure
hook, additional stack allocation setting or larger worker stack is introduced.

Future telemetry conversion borrows owned variants and transfers the name buffer
with `mem::take`. Correctly typed duplicates, wrong types, negative numeric fields
and wrong targets reject without recursively cloning or dropping deep retained
remainders. Journal validation borrows native objects, preserving stored pending
bytes, canonical hash preimages, signatures and durable replay semantics.

The first combined runtime (`node-observability-context-runtime-1`) records 431
passes, 15 failures and two ignored tests. The failures expose independent test
isolation/fixture issues and a real metrics output-admission gap. Process-global
logger setup poisoned later policy tests when the initial synchronous test had no
Tokio reactor. Telemetry tests also consumed each other's events; fixtures mixed
signed and unsigned native numbers and one expected obsolete field ordering.
Tests now scope event-enabled subscribers to their own work. Fetch lifecycle,
retry, provider-failure and stall assertions are mandatory rather than silently
skipped. Poll-duration checks use actual work and total elapsed execution bounds,
without assuming an operating-system scheduling upper bound. The isolated Cargo
resolution adds only test-owned tracing/registry edges; package versions, sources
and checksums remain unchanged.

The metrics log writer previously counted its frame but then used the generic
encoder's unaccounted output buffer. It now admits the complete counted allocation
and uses the existing exact bounded encoder under canonical flags. Zero, short and
exact allocation tests pass; the original end-to-end failure/file/retry assertions
remain intact. The manifest integration fixture now signs the actual unsigned
manifest digest with a deterministic Ed25519 fixture key; substitution of the PoR
root fails signature verification. Production signature verification is retained.
Manifest verification, chunk profiles and moderation result signing are library
capabilities independent of the CLI feature selection.

`node-observability-context-clippy-9` and
`node-observability-context-tests-build-3` both pass all targets with zero
diagnostics. `node-observability-context-runtime-2` executes their exact selected
artifacts on ordinary four-worker stacks: **449 passed, zero failed, two ignored**
across twelve harnesses. Breakdown: logger 25, futures 23, telemetry 150 and
orchestrator 251. The two existing ignores are gateway chunk streaming and
empty-provider rejection; they are not passing qualification. This combined graph
includes telemetry-selected TLS dependencies and does not establish a separately
isolated no-TLS orchestrator result.

All three successful records share source SHA-256
`1885f581228fece35edf4f18201695a26b15a96b7c29d9a6ca3c055e138f10e7`,
covering 20,194 files and 13 recorded deletions. Strict lint report SHA-256 is
`8391275c12a7fe4487ad5dfdc75bc5358892c2d92418f468449ffb25b777862e`;
test-build report is
`19b4a15f5c67d6d3889aa38e59b99bae091852a2a2edf1e84a9c89dfcf526259`;
runtime report is
`54cbf6185381e072e24aaebdfd7e96b6257336d3418a5b04a53b716758a7936c`.
These records live under the ignored composed-candidate evidence directory.
Four codec-retirement checks pass again. All twenty feature-resolved dependency
boundaries passed after the library capability correction; subsequent lock changes
only add development edges for local test subscribers.

The ordinary destructor, sanitizer and retained conversion paths are qualified by
these tests. Arbitrary caller-provided `Debug` implementations and recursive derived
`Clone`/`Debug` on manually constructed unbounded `Event`/`Fields` remain separate
boundaries; this checkpoint does not claim they became iterative. The storage
client's canonical retained DA response design, remaining external context callers,
configuration test-owner extraction, full workspace, release memory, four-validator
and current-source native/device qualification remain outstanding. This source is
still the isolated candidate, not a completed live-checkout cutover.

### SDK dependency and projection continuation, 2026-09-12

The default shipping SDK check reached an unmigrated SCCP dependency and failed
with 93 diagnostics before checking the SDK itself. The source did not change
during the check. Its report SHA-256 is
`6cfc09b76bd087ca7d236334f1eb295ad9a2ff4bac98e6849af2cdbf16699067`.

SCCP now has separate JSON field and closed-variant implementation modules in the
isolated candidate. Hex and decimal writers stream through bounded sinks without
allocating intermediate formatted strings. Hex decoding admits its retained byte
buffer; nested hex vectors also admit the retained outer vector. Native variant
decoding borrows input, and existing wire field order, schema identities and
canonical output spellings are retained. The offline evidence validator uses the
explicit wire-record context and removes fields by borrowing its owned JSON map.
These changes do not qualify production SCCP proof corridors.

The next strict library attempt, `sccp-context-lib-clippy-1`, passed the retired-API
errors but failed with 84 lint diagnostics, including existing replay/TON issues
and cleanup in the new helpers. Its report SHA-256 is
`76f98abc47b37fb26628813e8444e0e4cf2f6bf585df1e1134d3cd220234b231`.
Investigation of an unused hex decoder found a derive defect: named tagged-enum
variants ignored the field's custom decoder and its trait-bound selection. This
made canonical SCCP destination hex strings use ordinary array decoding. The
derive correction and regression are being prepared; the warning is not hidden.

SDK credentials and alias/evidence projections have also been migrated in the
candidate, but have not yet passed their SDK build. Alias output streams the
typed account through the caller's explicit address-format context rather than
constructing globally formatted string projections. Credential decoding admits
the owned scalar buffers, and bounded output retains secret redaction. The
private DA bundle and corresponding CLI migration are frozen for review and
qualification. Full SDK, SCCP runtime and strict lint qualification remain open.

The named-variant derive correction subsequently passes 791 tests with zero
failures: 65 derive units, 168 tests in the existing `norito_group_03` harness and
558 Norito library tests. The existing ignored streaming snapshot-dump utility
remains ignored. The regression includes a custom field with no JSON traits,
exact emitted bytes, text/native decoding, caller-context isolation, missing
context, malformed/duplicate/unknown fields and typed resource failure. Ordinary
worker stacks and unchanged source seals are recorded. Source SHA-256 is
`9df7127f5e41346229033acaf32d49fab9ec19858b01cedac5c346f3589e837a`;
test-build report SHA-256 is
`bffb17f490d9439094b38ac015ac6d7d26f61001c8efc7769a875a6d140b4efa`,
and runtime report SHA-256 is
`f42fdb282ffa6f16af664583edd1de88bd643fe86d9ad7bf95f83002ccf44209`.
All four codec-retirement guards pass again. This is focused codec evidence, not
SCCP/SDK or workspace qualification.


## Executor conversion and SCCP qualification continuation, 2026-09-12

The named-variant Norito correction also passes strict Clippy for its selected
library, derive and `norito_group_03` targets with zero diagnostics. The report
SHA-256 is `8c23ccafb793a2181439a780607e123df39b536be4481fe2fd273a8d2c3cdc83`.

SCCP subsequently passes all 231 tests across its library, Ethereum integration,
registry-security integration and offline release-evidence harnesses with
`dev-tools,test-fixtures`. The same sealed source was used for the test build and
ordinary four-worker execution; no stack-size override was set. Source SHA-256
is `d915038b101214d15027a5d6eab39e0c097fbb58b918617c92c4bafd44bd703b`.
Build/runtime report SHA-256 values are
`75b50f5778014f0e3101a9f1c8aed9f8747fbdee9d7c470ee93978a251e7e10a` and
`cc7b29edc73c3fce3fd14d2e07407fcd6b6f26f3218535d2537e693bb694315a`.
The build retained one unused test-environment warning in the non-test developer
binary. Subsequent strict all-target Clippy failed with 110 diagnostics, including
46 library and 64 library-test findings; its unchanged-source report SHA-256 is
`93af81bd9b7a10b55ff33ad5495ccc580486e063548b38145c801d99ec8c16be`.
Replay/TON decomposition and parsed-payload ownership corrections are being
qualified separately. The runtime result does not imply a strict-lint pass.

The next shipping-SDK check passed the SCCP compile boundary and exposed the
executor model's implicit JSON conversion owners. SDK methods themselves had
not yet compiled. Its report SHA-256 is
`fd67573213cc8593902ec7ceab8afb6add23769bb807664d557939170dcb4047`.
The candidate now removes infallible permission/parameter and multisig envelope
conversions. One fallible operation per conversion takes explicit JSON Context;
wrong registry identities are rejected before payload parsing without copying or
echoing the supplied identity. Multisig native decoders borrow their fields and
retain checked collections, with exact address context for signatory keys.
The new immutable DA bundle and CLI-owned persistence migration have also been
reviewed and composed; SDK/storage/CLI compilation remains required.

Executor model and derive test targets compile with zero diagnostics and pass
47 tests (45 model and two derive), with no ignored tests. This includes all
existing captured frame identities and new resource, duplicate/unknown field,
account-format isolation, missing-context and 32,768-deep hostile native-value
cases. An initial new diagnostic assertion incorrectly expected a Display prefix;
it was corrected to assert the actual structured Message error and exact fixed
text. Source SHA-256 is
`1adfe5583e5fbd811982ab4d1c3aedcee54a8d717fe2a23c0e7b5b8e0a82e45a`;
build/runtime report SHA-256 values are
`04a2abd2ea3843b0f77d023677344115ecd5295845687546bac49f4f41bb203b` and
`543285debf0165d69d9db03fc8f25868d6b487bdce430e3b2f5d2be25aee8387`.
The optional trybuild consumer suite, strict executor Clippy, external conversion
callers, full workspace and same-revision release/native/network qualification
remain open at this checkpoint.


Executor strict all-target Clippy subsequently passes with zero diagnostics.
The sole preceding failure was redundant `pub(crate)` inside a private fixture
module; its visibility was corrected without changing fixture calls. Passing
report SHA-256: `996412933ff396cae98df8e4a77fcd403d882b442f5a802162cd43757652478a`;
source: `e43f15373f5f64a59afc6ae973df48f9b092d44ffccba55281ccd7bf7a4192c7`.
The optional trybuild consumer suite and full-source release checks remain open.


## SCCP ownership and SDK readback continuation, 2026-09-12

The complete rebuilt SCCP test selection now passes 235 tests with zero failures
and no ignores: 206 library, 27 offline evidence-tool and two native/registry
integration tests. The build has zero diagnostics; source and artifact seals
remain unchanged and all harnesses use ordinary four-worker stacks. Source
SHA-256 is `f429720c9f724374e397bcfe9c21edb3e95ca48c1a04c02c7bf0328036622bed`.
Build/runtime report SHA-256 values are
`c802f3d61b7d4c2dcc0175675d42219af3af0223c5d7b1b059fa531d3922436e` and
`59188ad2d4e55f5f56544b60bee90947a7702523d2514a29cebf88fb7a42ff1b`.

Private parsed proof variants now own boxed backend material. Both verified
backend paths retain the parsed bundle and canonical payload buffers instead of
cloning them; pointer-identity regressions exercise the consuming boundary.
Replay updates borrow accumulator identities, preserve atomic admission, and
retain exact tree rebuilding/witness checks. TON parsing is split by cell
framing, level hashes, source statement, shard and execution validation. Existing
hash preimages, nominal schemas, binary field order and JSON field names are
preserved. The new TON field-name test initially expected unprefixed hex; the
assertion was corrected to the pre-existing canonical `0x` encoding. No original
golden fixture was regenerated. The production TON file still exceeds the
5,000-line budget; this checkpoint does not claim its full module decomposition.

Strict SCCP Clippy now passes the library and its unit-test compilation, but the
all-target command still fails in later targets: 23 evidence-tool findings and
two registry-fixture findings. Those owners remain under active correction. The
235-test result must not be read as an all-target strict-lint or release pass.

The SDK itself is now reached by compilation. Retired JSON macro/trait/writer
references are being migrated without compatibility definitions. Subscription
request signing, response decoding and exact draft metadata/permission/trigger
checks use the immutable client format. Another 102 canonical JSON terminals in
Client receiver methods now receive that explicit context. DA response pin scopes
and artifact projections retain the same context, including their account owner.
Reserve and repair response wrappers decode borrowed typed payloads rather than
cloning native JSON trees; settlement responses and their eight request encoders
also use their owning Client context. These SDK changes are composed but still
require a passing SDK build and runtime suite. Storage-client, CLI, optional
trybuild, workspace and release/native/four-validator qualification remain open.


### Superseded current-view paragraph, preserved 2026-09-12

The [aggregate JSON migration](docs/history/2026-09-11/aggregate-json-context.md) remains isolated. Its recorded feature-enabled source compiles all model tests, generators and benchmarks without diagnostics and passes 4,210 tests on ordinary stacks, with 13 existing ignores. The earlier default selection passes 4,172 tests. All five generator checks pass, including the reconciled AXT envelope. The [native ownership fix](docs/history/2026-09-11/json-native-ownership.md) passes 558 Norito library and four allocator/destructor tests plus strict all-target lint. All 679 model source files meet their existing size limits; four codec guards and 20 dependency boundaries pass. The manifest design corrections pass 1,086 tests and strict all-target lint. CAR passes 331 library tests without TLS and 330 with Rustls native roots, including immutable metadata ownership and root-bound validation. The same source passes strict model all-target Clippy, all twelve harnesses, all five generator checks and all ten oracle example fixture comparisons. Reward metadata now passes strict all-target lint and all 15 library tests, including atomic codec-budget failure. The stale gateway digest constants are reconciled against unchanged signed fixtures; the rebuilt CAR suite passes all 331 library and 15 integration tests. Configuration base passes all 61 tests and strict all-target lint. Shared DTOs and Connect examples pass strict all-target Clippy and all 341 tests across seven exact harnesses, including canonical receipt bytes and allocation admission. Node configuration passes strict all-target Clippy and all 893 tests across four exact harnesses, including explicit caller contexts, checked native/scalar codecs and unchanged snapshot fixtures. The subsequent logger/futures/telemetry/orchestrator candidate passes strict all-target Clippy and 449 tests across twelve exact harnesses on ordinary stacks; two existing gateway tests remain ignored. Deep redaction and destruction, allocator rejection, signed journal recovery and atomic payout failure are covered. Storage-client, external-context and full release qualification remain open.


### SDK library and complete selected SCCP target checkpoint, 2026-09-12

The SDK library now compiles through its production JSON owners with no stack
size override. `sdk-context-lib-check-7` passes on source
`46c43a57e91db7c8d8c94d34934af950218cff39693a68ea403876312082b5f6`;
its report SHA-256 is
`455c8fa0477cb79ffeff03a303450ce09d8441e426e3303304fcaa1a4b8e56cf`.
That checkpoint retains one unused private alias-request projection warning;
the unused duplicate has subsequently been removed in a separate staged change.
This is a library compilation result, not an SDK runtime or release pass.

Explorer and Space Directory readers now borrow nested native JSON arrays and
objects throughout validation. Rejection diagnostics no longer recursively
format malformed native trees. Contract receipts and manifests use bounded
borrowed decoding, and contract request payloads pass through the bounded writer
before reconstruction. New tests exercise 32,768-level native inputs. Generic
typed response decoders are owned by Client; asynchronous account operations,
operator reads, subscriptions, Musubi, SNS and query cursors explicitly retain
or borrow their configured address format. Scalar-only error, proof and wire
projections use explicit contexts without formatter capabilities. Multisig
proposal validation uses the canonical fallible instruction conversion, retaining
exact metadata, executable, signature and identity checks. SDK test compilation
and execution are still underway at this checkpoint.

The SCCP evidence tool and registry fixture lint owners are now resolved. On one
unchanged source
`375035a514461bf92ba00232a25dbda3710e202ab8494a201359b262f0df2bab`,
`--all-targets --features dev-tools,test-fixtures` passes strict Clippy and all
four rebuilt harnesses: 206 library, 28 release-evidence, one Ethereum-native and
one registry-security test, for **236 passed, zero failed and zero ignored**.
The private destination-state enum owns boxed backend payloads; the new regression
checks the exact existing family/state JSON and text/native roundtrips for all
three backends. Original proof/signature preimages, validation checks and fixture
assertions remain. Four TON deployed and reciprocal address comparisons have a
separate protocol validation owner; no comparison was changed to a misleading
Clippy suggestion. Reports:

- `sccp-context-clippy-8`: `fac0feb36feb09b260c34e7ebfba34c76516facb094864606d58e1982b20a096`.
- `sccp-context-test-build-5`: `3664f18b044504a50edbd25c5de340370957f47df8d8cc457a18b1d32ab1d86d`.
- `sccp-context-runtime-4`: `1e162bbfc18ba1c4638f1c59cd7844671b370d4a6c7a1be4963e591324ae28fd`.

The SDK and SCCP checkpoints above have distinct source seals. The SCCP selection
without test-fixtures, other SDK consumers, optional trybuild, module budgets,
source reconciliation, measured memory reductions, workspace and
release/native/device/four-validator qualification remain open.

Superseded current-view paragraph preserved from this continuation:

The [aggregate JSON migration](docs/history/2026-09-11/aggregate-json-context.md) remains isolated. Its model, codec, configuration, storage and observability checkpoints retain their recorded source scopes. The latest executor model and derive selection passes 47 tests and strict all-target Clippy. The rebuilt SCCP selection passes 235 tests on ordinary stacks; strict library/unit-test lint passes, while the evidence tool and registry fixture still have outstanding lints. The SDK is now reached by compilation, with explicit-context request, subscription, DA and readback migrations composed but not yet qualified. Remaining work includes SDK/consumer compilation, module budgets, source reconciliation and full release/native/four-validator qualification.

### Semantic response context and canonical failure checkpoint, 2026-09-12

The first complete SDK execution on the new JSON implementation ran 834 tests
on ordinary four-worker stacks: **832 passed, two failed, zero ignored**. It did
not overflow. The failures were the transaction-details fixtures using an empty
404 and bare `ValidationFail`, while Torii emits one canonical `ErrorEnvelope`.
The public decoder also discarded the machine-readable authorization denial.
Only the exact canonical `query_validation_failed`/403 pair now retains the
existing structured HTTP error. Its bounded response body remains available to
callers; private remote messages are absent from the displayed error chain.
Reserved `transaction_details_not_found`/404 remains the sole proof-absence
mapping. Empty, malformed, substituted and wrong-status envelopes remain errors.
The test-only details reconciliation helper classifies that validated denial as
terminal. This is not a change to the production asynchronous finality waiter.
The failed runtime is retained, not reclassified as a pass:

- Source: `6bc7aacdaff0314a42705033c5050e59c4854e2fad00b9539e8da2ade6203daf`.
- Build report: `4501888c1f4e9df313fb8453ddb2678a4108593f47d68646b320b7d2316309c0`.
- Runtime report: `dd55edde1b0d520c4c142768e789af1c3af70c32043adf454d35f64b0917e6b9`.

Following review, all three remaining raw query response paths use their client
or cursor's actual formatting context. Governance ballot validation now decodes
network and authority identities from borrowed native fields, removing a clone
that could recurse through a hostile tree before admission. UAID portfolio,
bindings and manifest readers, including the asset filter, thread the operation
context through their semantic identity checks. New regressions exercise both
369/753 directions, signed query and ballot bytes, rejected foreign identities,
resource-limit errors and 32,768-level native input before HTTP dispatch. This
follow-up builds without diagnostics; runtime qualification remains underway.
Alias, onboarding, faucet and verifying-key semantic readers are under review.

The independent SCCP selection **without** `test-fixtures` also passes strict
all-target Clippy and its rebuilt test harnesses on unchanged source
`40ec25d2c41bcea25c4aca5229ad37235621806cbeb2643441205440fbcd9487`:
206 library, 22 release-evidence and one Ethereum-native test, for **229 passed,
zero failed and zero ignored**. The registry-security executable contains zero
tests in this feature selection; its one test is qualified by the earlier
`dev-tools,test-fixtures` run. The dev-only fixture was decomposed by trust
identities, proof policy, audits and provenance with its original values and
signing inputs preserved. Reports:

- Strict Clippy: `202f239949ca2df9e090f6e32c5527e1a2d831b5717f869dee73f2bfbbc9f10d`.
- Test build: `2a8a93ecdc943001995bea7c2a75494036c401f8c4e0e1b0b314ab02d2e1405e`.
- Runtime: `ea2d08c283777232ff31c48b34aa1c0dd1e831643e8e3ab6b5489f21ad1d5b31`.

These remain isolated source checkpoints. Consumer migration, module budgets,
source reconciliation, comparable memory measurements and full workspace,
release, JNI/device and four-validator qualification remain open.


### Shared onboarding and SDK semantic-owner checkpoint, 2026-09-12

Atomic onboarding-state records now own their constructor and semantic validation in a capability module. Their schema identities, encoded field order, snapshot checks and closed JSON fields are unchanged. The sole constructor and validators require caller context and preserve typed formatter/allocation errors. The rebuilt shared selection passes **344 tests, zero failures and zero ignores** across seven harnesses, plus strict all-target Clippy with `connect`. Both request and target account formatting are covered in both 369/753 directions. The three new tests also preserve same-snapshot consistency and allocation failure.

- `shared-onboarding-context-clippy-1`: `fb63c5f6268dd6d3a72111f55dd5b43ca08fc2b5eb3fa0a59e7aa3cb79bf8b87`; source `fa09208778ca6ef8f5c7681995ef48f63902da7e077ba21e97fa1bb7cb5a5f07`.
- `shared-onboarding-context-test-build-1`: `388a93a5cc6b403b6d9b6b82a68a085ebd88356371134d065ae12eae39104a21`; source `fa09208778ca6ef8f5c7681995ef48f63902da7e077ba21e97fa1bb7cb5a5f07`.
- `shared-onboarding-context-runtime-1`: `dc05ed249b3b0eac02f909b9011bbbcd48f26be652f24a8acc79951c1ccde8ca`; source `fa09208778ca6ef8f5c7681995ef48f63902da7e077ba21e97fa1bb7cb5a5f07`.

The next complete SDK run passed 844 tests and failed three newly added query-route assertions. They expected `/query`; the canonical route is `/v1/query`. The assertions are corrected while retaining signed-byte, signature, cursor, no-replay and formatter checks. All deep native-input, governance ballot, UAID context and canonical transaction-details tests passed. This failing run remains evidence of the actual result, not a pass. Its source is `edabc5cc015c97d186673cc9eb957c3f0b780df7980053d8e36766f2cc216448`; runtime report SHA-256 is `08b73a87ff33be1edf031762d5970e2d5b11949140f12e5ba88aa6beb83e8dab`.

The following SDK composition removes remaining ambient I105 helpers from alias, onboarding, faucet and VK draft validation; all six offline constructor/verifier APIs take explicit context. External CLI calls use the actual client or admitted inventory, and Torii fixture calls retain node configuration. Optional-string rejection no longer recursively formats hostile native values. Contract receipt expectations, faucet mutation fixtures, privacy schema/transport assertions, reserve transport assertions and registration-admission setup have separate owners without additional lint exceptions. These combined SDK and external consumer changes remain under qualification. Torii runtime formatter ownership and the full source/release qualification remain open.

Superseded current-view paragraph preserved exactly:

The [aggregate JSON migration](docs/history/2026-09-11/aggregate-json-context.md) remains isolated. Its model, codec, configuration, storage and observability checkpoints retain their recorded source scopes. The executor model and derive selection passes 47 tests and strict all-target Clippy. The latest SCCP selection passes all 236 tests on ordinary stacks and strict all-target Clippy with `dev-tools,test-fixtures`. The SDK library compiles after explicit-context and borrowed native JSON migrations; its tests and other consumer compilation remain under qualification. Remaining work includes consumer migration, module budgets, source reconciliation and full release/native/four-validator qualification.


### Complete SDK target checkpoint, 2026-09-12

The rebuilt default-feature SDK passes all five compiler-selected harnesses: 858 library tests, three transaction-TTL/signing integration tests and six example tests, for **867 passed, zero failed and zero ignored**. Strict Clippy also passes across all SDK targets without diagnostics. The runtime uses ordinary four-worker stacks and finishes its library in 176.84 seconds; no stack-size override, nested runtime workaround, compatibility API or weakened input limit was added. Deep native-input rejection, typed resource errors, 369/753 network isolation, alias/onboarding/faucet/VK semantic validation, signed bytes, streams and canonical transaction-details errors are exercised by the complete suite.

All three reports bind the same 20,219 source inputs, totaling 498,790,369 bytes, with source SHA-256 `5220042f8f28ccbe5732844c361826779819ef78e80f3dc46415a59e758a71cd`. Exact compiler-produced executable and Rust standard-library identities are retained in the runtime report.

- `sdk-context-all-targets-clippy-1`: `bab2011ecde36c5f80534cdb70192c163ba7d574162b54c6cc7376f69f45eefe`.
- `sdk-context-all-targets-test-build-1`: `cc9aee9a1b3d15d696f8813736d2df677a3af5a86e0cb94c93f0913fcf3194e4`.
- `sdk-context-all-targets-runtime-1`: `02b1444de33e67da380af6e8cfb0276230710d15aa53b8a865144537645e9fd9`.

This qualifies the isolated SDK checkpoint. External CLI/Torii consumers, other feature selections, module budgets, source reconciliation, comparable memory measurements and workspace/release/native/device/four-validator qualification remain open. The two earlier failed SDK runtimes remain recorded above with their original outcomes.

Superseded current-view paragraph preserved exactly:

The [aggregate JSON migration](docs/history/2026-09-11/aggregate-json-context.md) remains isolated. Its model, codec, configuration, storage and observability checkpoints retain their recorded source scopes. The executor model and derive selection passes 47 tests and strict all-target Clippy. SCCP passes strict all-target Clippy and all tests in both selected configurations: 236 with `dev-tools,test-fixtures`, and 229 with `dev-tools`. Shared DTOs pass strict all-target Clippy and 344 tests after explicit onboarding identity validation. The latest complete SDK execution passed 844 tests and failed three new assertions expecting an unversioned query route; those assertions are corrected. Deep native-input, UAID format-isolation and canonical transaction-details regressions passed. The composed alias/onboarding/faucet/VK continuation is under qualification. Consumer migration, module budgets, source reconciliation and full release/native/four-validator qualification remain open.


### Storage ownership and artifact-budget checkpoint, 2026-09-12

The shipping storage-client dependency no longer enables the orchestrator CLI bundle. All five storage TLS selections now forbid both `sorafs_orchestrator/cli-orchestrator` and `sorafs_car/cli` on resolved normal/build dependency paths. All **20 configured dependency boundaries** and **65 Python guard tests** pass. The complete four-part retired-codec guard passes. The Python run emitted 20 cleanup warnings concerning older temporary test directories; these were not assertion failures.

The first complete storage execution reported **56 passed, six failed and one registered ignore**. Five failures used reserved discriminator zero in the SDK fixture. One exposed manifest persistence using the ordinary unbounded pretty writer, which did not charge its output to the active allocation budget before filesystem effects. This was corrected in the persistence owner. `DaManifestOutputOptions` now requires the caller's directory and per-document byte cap. Both compact canonical JSON documents are counted and allocated within the explicit cap and active Norito budget before directories or files are created. Exact raw Norito manifest bytes are unchanged. Storage workflows borrow the same options; three CLI commands carry an explicit `--max-manifest-json-bytes` flag with a 64-MiB default. The reusable library supplies no implicit cap or compatibility overload.

The original zero-allocation rejection assertion remains, and new tests cover zero/one-below/exact output byte limits and absence of filesystem effects on failure. Scoreboard enrichment mutates its owned native JSON object without recursive copying; a 32,768-level test verifies that the leaf allocation is retained exactly. The second run passed 62 tests and failed one stale pretty-whitespace assertion, which now asserts the sole compact output while retaining all byte and decoded-value comparisons. The failed reports retain their actual outcomes:

- `storage-client-context-runtime-1`: `f2c22176cdbf736b2b13eb2622992f4edf3b4895a002645d71d5689781929d73`; source `f25a3f7f14f2f8fb77664593fa05779652220d2fd98188b51e80e84bbb464961`.
- `storage-client-context-runtime-2`: `29321f12067c9c6fdd42e9a529425514da203e7ccdc75abbc014bd463ceb23fb`; source `ae4a5c5e37ca181109ceaca19bb2dc72afc1fd812444cf48adfd23f281731a3b`.

Final rebuilt storage qualification passes **55 library and eight integration tests**, with zero failures. The memory integration harness registers one ignored child test, which its passing parent explicitly launches with `--ignored --exact` in a separate process and requires to succeed. Strict all-target Clippy passes without diagnostics. All final checks bind the same 20,220 source inputs, totaling 498,819,056 bytes, with SHA-256 `84bcc4a4a4af72e81fc069b916a044b2ef2969f95bbb746b3ef093c59ffe46f9`:

- `storage-client-context-clippy-7`: `cfcee9c92ebd931c8b51a6d408ada171df1cb2ebd7f251c954072ea4fccdf965`.
- `storage-client-context-test-build-3`: `bb42cc672a633bbdf1d8b94740eb88799d9e69416c6ed2cf57a188435a8c55ea`.
- `storage-client-context-runtime-3`: `ec65157575364adcdac4f55b76ff728c3f0c1a4a7e578ecfba5a672b470e905c`.

Torii runtime construction and admitted routed-request plans now retain an explicit AddressFormat in the composed candidate. The daemon and configured harnesses supply their actual configuration. Eight new tests and the existing authorization/resource-accounting assertions are prepared; Torii execution is not yet qualified. Core checking passed the repaired lexical i18n parser and stops in the unmigrated Kotodama driver JSON boundary. The new CLI artifact-option tests are prepared but have not run. Compiler/global-context migration, other consumers, module budgets, source reconciliation, measured build-memory reduction and full workspace/release/native/device/four-validator qualification remain open.

Superseded current-view paragraph preserved exactly:

The [aggregate JSON migration](docs/history/2026-09-11/aggregate-json-context.md) remains isolated. Its model, codec, configuration, storage and observability checkpoints retain their recorded source scopes. The executor model and derive selection passes 47 tests and strict all-target Clippy. SCCP passes strict all-target Clippy and all tests in both selected configurations: 236 with `dev-tools,test-fixtures`, and 229 with `dev-tools`. Shared DTOs pass strict all-target Clippy and 344 tests after explicit onboarding identity validation. The SDK now passes strict all-target Clippy and all 867 tests on ordinary stacks, including the deep native-input, network-context isolation, alias/onboarding/faucet/VK and canonical transaction-details regressions. Storage-client and Torii consumer migration, module budgets, source reconciliation and full release/native/four-validator qualification remain open.
