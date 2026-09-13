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
guards and exact historical-archive verification. The wider xtask rebuild finds
a separate merged Core caller of the now-private ledger replay-authority field;
its owning-API repair and signed-manifest tests remain pending. All failed
builds remain preserved. See [the benchmark contract](../specs/fastpq_benchmark_v1.md).

The full privacy/ZK, SDK, hardware, audit and four-validator release gates remain
open. These checks do not establish production qualification.
