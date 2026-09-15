# IVM-only implementation and validation

Iroha executes Kotodama contracts as IVM bytecode (`.to`). Wasm/WASI
implementations, SDK adapters, targets and release artifacts are prohibited.
This is a first-release interface removal; retired fields and exports do not
have compatibility aliases or alternate decoders.

## Changes

- Removed the browser codec crate, loader, initialization API, exports/types,
  build/publication scripts, and package asset paths. The canonical native Rust
  account/instruction owner remains. Browser code cannot manufacture a native
  codec capability or use a managed replacement.
- Removed target-specific browser entropy and serial-proof dependencies and the
  three browser-only PQCrypto portability copies. Native dependencies return to
  the same locked upstream versions; actual native cryptographic code remains
  required.
- Removed the unused browser artifact-admission JSON projection. Native IVM
  admission retains its typed result and all direct positive/adversarial checks.
- Removed the compute mode enum, selector and WASI budget flag. Defaults, CLI,
  fixtures, config and schema inventory use the sole IVM shape. Negative tests
  frame removed payloads under the current nominal identities, so a header
  mismatch cannot conceal an accepted old layout.
- Payload-free compute policies use exact strings in both JSON and TOML. The
  randomness, storage, authentication and risk-class policies reject their old
  object envelopes and case aliases. Binary discriminants remain canonical.
- The current-tree policy check includes tracked, unstaged and nonignored new
  source. It rejects alternate runtime/build declarations and Wasm artifacts,
  including foreign binary magic under a `.to` name. It has no override or
  base-ref exemption and runs in pull-request CI. Upstream optional dependency
  metadata and security MIME classification are not execution support.

## Native owner corrections under verification

Execution STARK hashing now calls the shared six-lane Goldilocks owner for both
one-shot and streamed frames. The execution-local Poseidon2 implementation and
parameter tables are removed, and source/profile identities bind the shared
owner. BFV delegates its duplicate six-lane implementation to the same acyclic
primitive dependency. Execution qualification and proof/hardware evidence must
be regenerated for the changed source and domains.

Native SDK adapters reconstruct the existing typed game, verifying-key, NFT,
contract-deployment, activation/ownership and SoraFS replication instructions.
All six lifecycle operations require exact decimal-string CAS revisions; the
superseded activation/deactivation converters are removed. Kaigi uses a lossless
unsigned-integer JSON projection. Manifest schemas are checked on encoding and
decoding; generic string envelopes cannot bypass the protected parsers. SDK
regressions retain canonical frame/archive bytes, closed shapes, missing-native
rejection and existing signing assertions. These changes do not grant proof,
contract, resource, key or replication authorization; Core retains those checks.

Instruction frame strings use explicit lowercase `0x` hex or exact standard
base64, followed by native decode/re-encode equality. Decoder probing and bare
hex aliases are removed. Lane proof builders and decoded projections use the
native one-element commitment tuple and checksummed hash literals. Ballot
public inputs retain the native canonical JSON field order and full-width tokens.

SoraFS pin detail reads only the native finalized manifest record, checks the
requested digest and optional finalized anchor, and preserves full-width integer
tokens. Decimal metadata is accepted only within the metadata subtree. The
retired alias-header hook, options and response wrappers are removed; native
alias integrity/freshness assertions remain at their actual evaluator owner.
Pin lists retain native approval history and nullable cursor fields. Replication
lists preserve provider authority, completion revision and finalized anchors;
future/conflicting anchors and unknown chunker profiles are rejected. Full-width
integer parsing and pagination follow native bounds. One immutable chunker
catalog serves archive and response validation, which loads through the existing
optional module without increasing any bundle ceiling.

A subsequent source audit found a separate unfinished ZK-AMS RNS-native qPCS/FRI
owner with 32-byte Keccak proof roots and transcript state. Its composite and
activation boundaries remain unavailable; it is not a fallback for current
Core STARK admission. Its role-specific six-lane cutover, canonical packing and
unchanged-cap validation remain outstanding. FASTPQ development/benchmark trace-Merkle helpers now use the complete six-lane
owner; native validation of that cutover is pending.

## Validation

The composed corrections additionally pass 60 shared Goldilocks primitive tests
(one timing diagnostic ignored), eight BFV hard-cut tests and all 49 native SDK
codec tests, including nine new activation/ownership regressions. Initial native test-only compile errors and one invalid signature
fixture were corrected and retained in the logs. The portable package smoke
passes clean installation/import and missing-native rejection before signing.
The four native execution digest tests pass, including an independent six-lane
vector, typed domain separation and streamed framing boundaries. Their Core
test build retains four warnings in unchanged owners. Nine additional native
execution profile/export checks pass on the same compiled Core harness, covering
RACE/Touring exports, fixed geometry and fail-closed registration. They do not
qualify proof soundness or hardware/deployment parity.
All 62 changed JavaScript files pass ESLint. Pin/alias/replication projection and
declaration checks pass 37 tests; bundle/report checks pass 21 tests with their
existing ceilings and exact measured closure assertions. The current measured
Torii bundle is 813,099 bytes across 125 modules, with 3,029 bytes of headroom
under its unchanged ceiling. Exact measurement assertions are regenerated from
the pinned bundler; lazy-chunk and growth limits remain unchanged.

- JavaScript: 103 focused tests plus one packaging regression pass, zero skipped;
  distribution generation and targeted ESLint pass. Bundle ceilings are unchanged.
- Policy guard: `python3 -m pytest -q pytests/scripts/ivm_only_guard_test.py`
  passes 47 cases; an independent root-agent replay and current-tree check pass.
- The IVM-removal native Rust checkpoint passes 1,138 tests: 26 compute/schema, 881 config,
  33 IVM artifact admission, seven canonical SDK codec, 187 post-quantum unit
  tests and four post-quantum known-answer tests. One manual compute fixture
  printer is ignored and is not counted as passing. This IVM-removal checkpoint
  emits no compiler diagnostics. CLI, xtask and compute gateway checks also pass;
  their 28 warnings belong to unchanged source owners.
- JavaScript declaration/export checks pass 16 cases and bundle checks pass 19.
  The first maintained native build/copy and distribution generation pass with
  identical source fingerprints before and after testing. The complete unit run
  records 3,030 passes and 123 failures, with no skips: stale codec doubles plus
  missing native instruction conversions, ABI validation and closed SDK
  projections. The portable package smoke also fails when its transaction recipe
  incorrectly assumes an installed native binding. These failures are retained;
  the subsequent maintained build/copy succeeds with identical before/after
  source fingerprints and the same 21 compiler warnings. Its focused run records
  1,283 passes and 37 failures out of 1,320 tests, with no skips. The corrected
  frame/schema tests pass against that binding; three activation cases await the
  new native converter rebuild. Full unit/package replay remains pending.
- The Go vendor inventory was regenerated twice through its canonical owner,
  byte-identically, after removing the foreign target file and build tags.
  Native CPU/endian and terminal checks pass on macOS arm64. All four message
  and four epoch R1CS identities were actually remeasured under the new source
  closure. The epochs take 381–395 seconds each and peak at 88–90.1 GiB RSS,
  with zero swaps. All eight current identities are published from their actual
  receipts; ceremony, proof, Linux-builder and deployment gates remain open.
- Bridge tooling uses distinct official native Ethereum and TRON 0.7.6
  compilers. The old embedded-Wasm compiler and mislabeled TRON alias are
  removed. Native compiler/artifact checks, complete EVM contract and replay
  smokes, 112 Python tests, five EDR tests, 18 TVM receipt tests and both required
  npm audits pass. A real-parser regression covers the TVM phase's current CLI.
  Compiler execution was macOS x86-64 via Rosetta; Linux execution and real TRE
  deployment remain unverified. The prior compiler evidence is explicitly
  retired, and no production limits were relaxed.

Native Rust evidence, commands, harness hashes and retained failures/corrections
are in `target/ivm-only-validation/`. Epoch receipts and authenticated publication
are in `target/agent-work/sccp-epochs-final-20260913/`. The pinned Linux builder
cannot run here because no supported container runtime is installed; native
macOS execution is separately recorded.

The complete base/Fp4 FASTPQ evaluator and its bounded public polynomials pass
44 native tests, including all 11 new field-point regressions. Two existing full
prover/cache diagnostics remain ignored. The native build emits no diagnostics.
These tests preserve the 923-slot relation and base prover caches; they do not
complete witness masking, coefficient commitments or production proof admission.

Torii discovery passes all 50 native tests on one unchanged source snapshot,
including finalized pin/alias readback, exact cursor bounds, canonical V1 JSON
and Norito caller-signed submission, and malformed-envelope/network/signature
rejection. The public JSON route preserves the queued transaction's full
versioned bytes, hash and signature. The native library harness is byte-identical
to the prior passing run, retaining its 57 compliance tests (including three new
acknowledgement signing adversaries), three custody tests, five finality tests and
one cursor-parser test; these 66 tests were not redundantly re-executed.

The discovery fixture isolates persistence and stages, acknowledges, promotes and
reopens an actually signed catalog through the production file store and feed
transport with an exact required-feed inventory and pin policy. The supplied feed
is synthetic local input; no HTTPS fetch or remote provenance is claimed. The
public acknowledgement signing-digest method shares canonical validation and
exact signing bytes with verification. Controller authority, revocation and
freshness checks remain mandatory. Custody fixtures retain the same Kura handle
supplied to State and use narrow test-only state helpers. Software-signed durable
artifacts and exact three-of-four certificate checks do not qualify a physical
HSM or replicated network. All preceding failed build and runtime logs remain
retained with their actual source and harness hashes.

The final maintained SDK binding build, copy and distribution preparation pass
on one unchanged source snapshot. All 1,414 focused tests and all 3,209 full
unit tests pass, with zero skips. The portable package smoke validates 197
published files and passes clean installation/import and native capability
boundaries. The canonical tuple/hash-literal, recipe contract and exact browser
module inventory corrections retain their negative and immutability assertions.
The source snapshot SHA-256 is
`12442811627a1053aa4eab01e6b69b43068da7344c68e979ba805b98d390e263`;
all preceding failed attempts remain preserved. This SDK checkpoint predates
the subsequently composed FASTPQ changes and is not a full-release result.

The reviewed FASTPQ degree analysis, explicit coefficient masking, exact full
quotient arithmetic, guarded secret buffers and native six-lane benchmark
owners are now composed for native validation. Benchmark consumers share one
explicit V1 report contract, reject retired scalar fields and incomplete
operation inventories, and preserve complete six-lane geometry/parity evidence.
The reviewed consumer correction passes 552 Python consumer/release-isolation
tests on the composed source (14 additional subtests); 16 shared parser fixtures reproduce byte-for-byte.
Their device and timing values are synthetic. Complete raw/flat report validation
now precedes rollout policy and geometry classification, including exact staging
metrics, typed counts, invocation overflow and GPU timing presence.

The preceding native snapshot
`0d11c6c2caa4e1058866c81a146113a4ddd6ee8f27a9eae3d6c785a4306aa996`
passes 206 focused arithmetic/benchmark tests, 12 executor/dispatch controls and
actual Metal six-lane parity across the framing boundaries. Both explicitly
scheduled full-size tests pass: public Fp4 preparation takes 58.17 seconds with
484,605,952 bytes maximum resident memory; the complete 65,536-by-342 masked
numerator and exact quotient take 1,928.90 seconds with 4,181,819,392 bytes maximum
resident memory. The latter checks full remainder/high padding and independent
Horner/current/next-row AIR identities. Measurements use macOS `/usr/bin/time -l`.
Masks are explicit deterministic arithmetic-test inputs; production entropy, PCS,
bounded-opening admission and complete proof qualification remain unfinished.
The native results and source/harness hashes are retained under
`target/ivm-only-validation/fastpq-composed-native/` and
`target/ivm-only-validation/fastpq-fullsize-native/`.

The subsequent xtask report build exposed two `&&str` indexing errors, corrected
at the shared Norito JSON consumer. The standalone Rust profile suite then passes
all 13 tests, including the 16 shared fixture cases. Its separate tooling lock
was resolved offline and its exact hash is retained with the native result.
The unsupported Metal entry point passes two actual process checks: exit 1,
exact prerequisite diagnostic, empty stdout and no report artifact.

On snapshot `e17e5a28649bcb990df0092a4ef07bd9e7c821ec5b69836d9c3206c0033b8320`,
actual required-GPU Metal captures pass for eight-row `all` and three-row
trace-column/pair filters, each with one warmup and two timed invocations.
All three outputs pass the maintained wrapper and complete Python projection;
the Rust renderer accepts all six raw/wrapped inputs and retains their exact
complete JSON records. These small debug-build captures qualify primitive
execution/report consistency, not production performance. Results reside in
`target/ivm-only-validation/fastpq-metal-capture-native/`,
`fastpq-metal-rust-readback/` and `fastpq-profile-native/`.
The same candidate passes the IVM-only guard, its 47 regressions, retired-codec
guards and exact historical-archive verification. The wider xtask rebuild found
a merged Core caller of the now-private ledger replay-authority field. Its
ledger-owned API now requires the exact claim, retained row and verified context
before authenticating historical source coverage. The fresh merged Core binary
passes the direct claim-join test, five cold-owner tests, the marker-authority
test and two additional timeout-history/cancellation tests. The additional
checks retain their source and binary hashes under
`target/ivm-only-validation/ledger-merged-additional-native/`.
The earlier combined Core/xtask build changed source during a merge and failed
with zero tests executed; its drift record remains preserved. The subsequent
stable xtask build passes 40 tests and fails one matrix-filter ordering assertion.
The writer now emits the canonical inventory order, and signed-manifest admission
rejects empty, duplicate and reordered filter arrays even with a valid signature.
The writer also omits an empty source filter set as absent metadata. All 43
actual manifest/report tests pass on unchanged source
`65a6198425cc1e5cfb4b24c19b936e7db6a94d3161c450e4ed9ec9729a71618e`.
Both the original failure and corrected run remain under
`target/ivm-only-validation/fastpq-manifest-merged-native/` and
`fastpq-manifest-final-filters-native/`. The preceding 42-test ordering-only
run remains separately preserved. Synthetic signing fixtures establish
validator behavior; they are not actual release signatures or device provenance.

The matrix builder now requires wrapped, fully validated reports and explicit
filters before deriving measurements or thresholds. A single device label cannot
combine CPU, Metal and CUDA backends. Its ordered filter-array rule matches Rust
manifest admission, including rejection through the isolated release runner.
The composed Python consumer/release suite passes all 598 tests and 14 subtests
on unchanged source `8f35e289bd1c653c354243ab8c522eadddcf82dca59fbeaad1f6988612e6aa1d`;
results are in `target/ivm-only-validation/fastpq-matrix-contract-python/`.
All device/timing values in this suite are synthetic.

That merge also reintroduced four generated Wasm SDK files. All four were
removed from the checkout and index; the IVM-only guard, all 47 guard regressions
and all 19 focused JavaScript packaging/type tests pass after removal. The
exact removed artifact hashes and focused results reside under
`target/ivm-only-validation/merge-wasm-retirement-20260913/`.
Both native benchmark entry points now share preflight checks for canonical row
geometry, count overflow and actual contiguous allocation limits before tracing
or device work. On unchanged source `b513939ec34970be0961cce65992858c845c977e179196243f9e2ed3f97b07ae`,
all 72 native geometry/producer tests and 16 actual invalid-input processes pass.
Invalid rows/counts, unrepresentable sample/flattened buffers and the parameter
alias exit with status 1 and no report or trace artifact. An eight-row,
six-operation required-Metal capture passes the wrapper and complete projection.
The same native artifact passes 20 executor/dispatch controls plus explicit
actual-Metal tests for all six lanes at framing boundaries and indexed predicates
at nonce boundaries. CUDA device parity remains unexecuted. Logs, commands,
source and binary hashes are retained under
`target/ivm-only-validation/fastpq-benchmark-preflight-native/` and
`fastpq-benchmark-merged-digest-native/`. These are bounded native behavior and
primitive checks; full proof/performance qualification remains open. See
[the benchmark contract](../specs/fastpq_benchmark_v1.md).

The native40 RNS cutover is now composed: full six-lane proof digests, typed
15-byte Fq2 fields, the sole three-section V1 envelope, and canonical confidential
spool callers replace the disconnected proof and detached-padding owners.
The 44 disconnected proof files are removed; live source padding and curve-verifier authority
remain enforced. Cargo resolved exactly three new proof-crate dependency edges
without changing package versions. The first production build exits 101 with
29 compiler diagnostics and zero tests on unchanged source
`5327ca03d03202672c0b5c5862f7d512c19939aadaa143ac988db08616973fe8`.
Its logs remain in `target/ivm-only-validation/rns-native40-initial-native-20260914/`.
The first retry compiles far enough to expose two production and 28 unit-test
diagnostics on unchanged source
`dcc73cf26d2e9ad49df34627d3054bc82802e07df66458b154befccccb7cb977`;
no tests execute. Its separate logs remain in `rns-native40-compiler-retry01-20260914/`.
Corrections restore missing private module registrations, preserve both native48
source-packing inputs, update derived accounting assertions and repair typed
test fixtures. Two obsolete topology-only adapter files and their bulk preflight
are removed because their accumulator and collective verifier APIs were already
retired; the actual streaming encryption/source owners remain. The next build
compiles production, unit and both integration-test targets on unchanged source
`a564f63d2ee076fedb4ccf7439af9339dd30232cd78906314d0b637f41e59333`.
The first 84 native foundation tests pass 21 and fail 63: two mistyped roots in
the duplicated parameter table cause profile rejection. The corrected table now
derives the exact source prefix under one owner; all 40 roots are independently
reproduced. The rebuilt unchanged source
`5a595aa37310d4bd97bf1e36cc620cfa92b48fa442db40fa65a6358b3ed9c560`
passes all 95 selected native tests with no ignores: hash, field, sampling,
wire, leaf/tree, profile, transcript, section codec and nine ordered-storage tests.
Real small encrypted-file pairs exercise order, context, partial seal, substitution,
malformed plaintext and failure cleanup; full-size storage is not measured.
Results and hashes remain in `rns-native40-foundations-retry01-native-20260914/`.
The next unchanged source
`c629dd31cc52e8633f8b51e390132a645a4ab0f2736f6e54466ca3e574264445`
passes 91 of 95 source-ownership tests, including all 23 source-packing, ten
plane-opening and seven plane-codec cases. The four failures identify fragile
source guards: unary logical-not misread as a macro, a quoted declaration
counted as a registration, an unrelated derive attributed to a secret owner,
and a stale byte pin for the unchanged context leaf. Their corrections preserve
the full adversarial inventory and add compiler-checked non-Clone/non-Copy
assertions for all six confidential source owners.

On that same source, the initial qPCS tests pass eight, complete FRI passes 15,
and prefix passes nine with one outdated expected error: invalid layer geometry
rejects before Merkle traversal. These positive fixtures authenticate synthetic
frontiers and local equations, not a full transcript-bound production proof.
IPA minimum-dimension integration passes three; Vega reachability passes nine
with one stale expectation for the existing `parallel -> full` feature edge.
The data-model library freshly compiles and all eight selected catalog/wire
tests pass, including independent catalog reproduction and canonical six-lane
encoding. Native receipts retain complete test lists, failures and executable
hashes in `rns-native40-source-ownership-native-20260914/`,
`rns-native40-qpcs-stages-native-20260914/`, the two named integration directories,
and `privacy-catalog-model-native-20260914/` under `target/ivm-only-validation/`.

The candidate removes the unused 702-scalar mask commitment from the
native40 global lookup wire, typed roles, transcript and profile identity.
Multiplicity and the inverse-product mask remain mandatory and distinct. Exact
two-point framing rejects the former three-point shape; the parent proof limit
is unchanged. Compiler retry 06 passes on unchanged source
`02b9f2f03c932d82bacd47688060edc5b47c5593c09334c3473ae9fd7d0aed3c`,
discovering 1,586 unit tests and 13 integration tests. Its separate native runs
pass 96 foundation/storage, 90 global-wire/composite, 34 qPCS, ten Vega and three
IPA tests. All 11 phase-2/3 guards, 44 link guards and 19 radix/comparator tests
also pass. The new nonzero prefix arithmetic vectors independently reproduce all
1,480 field words; their native test passes without qualifying a complete proof.

The original source-replay selection is incomplete: its third test repeatedly
recomputed the same fixed mapping and was explicitly stopped after the first
two passed. The full direct-proof selection fails a stale descriptor assertion
and then aborts with an ordinary-stack overflow in the four-core proof path.
Its partial output and 25,755,648-byte maximum RSS do not qualify full proof
execution. Core freshly compiles on the same source and discovers 15,241 tests;
its privacy-profile selection reports 26 passes, one digest-KAT failure and one
ignored operator printer. Receipts remain under `target/ivm-only-validation/`
in the retry-06, source-guards, direct-full-GBP and privacy-core directories.

The next applied repairs cache only the fixed source mapping, reject malformed
owned source-packing frames before acquiring replay, and retain prepared direct
rows and the four-proof bundle in bounded heap owners. Authenticated D/S-low
preparation emits the exact 11,696 value planes through the existing source
cursor; prepared values do not confer commitment or proof authority. Its docs
distinguish polynomial coefficients from logical-slot padding checked by NTT.
Exact preimages and reviewed composition are recorded in
`rns-owned-preparation-composition-20260914/composition.json`. The corrected
native candidate `295f1e17abd9193167c1bea15cc6733e2ca41156b996aef2ab39677b9acb4c94`
now passes all 34 direct-proof tests, including actual four-core, 16,384-gate
proof generation, verification, discharge and mutations on the default stack.
The complete selected process takes 468.04 seconds and peaks at 443,777,024 bytes
RSS. Its fixture source and deterministic test entropy do not establish a
production source; concurrent compilation means this is not a controlled release
benchmark. Results remain in
`rns-next-private-full-direct-native-20260914/result.json` under the same target root.

The exact 72,386-entry inventory replaces 22 disconnected challenge/session
owners. Original fallible entropy now moves from encryption into the source
commitment session and authenticated D/S preparation; no detached replacement
entropy is accepted there. Corrections to the consuming-signature assertion and
test-owner size split now pass all 49 source-algebra and 40 cleanup tests on main
source `8322c0a0f5e7c62a62690433399e860343aa912ea1cccd8619947c87bc135334`.
The separate nine-test streaming ingress selection passes eight, including both
new actual key/manifest mutation tests; its remaining source guard still expects
the phase-2/3 module to be compiled out instead of compiled privately. Results
remain in `rns-main-successor-selected-native-20260914/result.json`; that combined
98-test run is recorded as failed until the guard is corrected and rerun.
All 19 complete inventory tests and seven additional affected source guards pass.
The predecessor candidate separately passes 116 inventory/Z/link/initial tests,
nine lifetime/poison resource tests and the actual small T256 GBP roundtrip.
Do not add overlapping selections into a purported full-suite count.

The reviewed 87-path transfer is applied to the main checkout with the index
unchanged; exact provenance is retained in
`rns-next-private-transfer-20260914/application.json`. Core's profile KAT and
ordinary election-boundary repairs are applied, but fresh grouped compilation
exposed an accidentally deleted `checked_zk_stark_keypair` test helper. Restoring
its exact fallible generation and regression makes the grouped rebuild succeed.
The 181 selected Core cases, added Parliament deadline/retry test and real
four-validator execution remain pending. IVM-only, codec, formatting and the
historical archive guards pass at the native candidate checkpoint.

Full qPCS construction still exceeds the unchanged 128-billion-operation cap:
initial raw-payload absorption alone requires 436,224,393,216 nonlinear field
multiplications under the current round program, even granting framing, linear
layers and Merkle nodes for free. Allocation lifetime accounting does not solve
that design gap. A reviewed commitment/evaluation design is required; no raised
cap, per-stage meter reset or hardware discount authorizes the current full tree.
Complete source/prover authority, source-to-storage handoff, actual replay,
composite admission and resource qualification remain unfinished. See
[the native wire contract](../specs/crypto/zk_ams_rns_native_wire_v1.md).

The full privacy/ZK, SDK, hardware, audit and four-validator release gates remain
open. These checks do not establish production qualification.
