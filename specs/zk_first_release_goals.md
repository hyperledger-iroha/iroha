# ZK first-release completion goals

Set: 2026-09-26. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Implementation remains in the
existing `optimizations` checkout. Separate captured SDK, Apple and network
candidates preserve validation provenance. Unrelated concurrent changes are
preserved.

The outcome is useful, safe proof algorithms with simple developer APIs. A
renamed placeholder, disabled feature, new status object, passing source hash,
or relaxed resource ceiling does not complete an algorithm. External review
and physical-device evidence cannot be replaced by self-issued certificates.

## Goals and acceptance criteria

| ID | Status | Owner / outcome | Completion criteria |
| --- | --- | --- | --- |
| ZK01 | Complete (implementation) | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. Target timing and independent qualification remain ZK06. |
| ZK02 | Complete (implementation) | Confidential circuits / optional inputs | One owned note can be fully redeemed at maximum tree capacity. Absent inputs require no empty-leaf membership or caller-created dummy witness. Ownership, nonzero/duplicate-nullifier, range and conservation constraints remain enforced. Regenerate all changed circuit keys, digests and dependent fixtures; reject superseded keys. Positive and adversarial circuit/native/SDK tests pass. |
| ZK03 | In progress | Core / honest proof semantics | Binding, replay-validated computation and private semantic proofs have explicit distinct contracts. Generic verification cannot confer a stronger guarantee than its compiled relation. Preserve mandatory IVM replay until a real IVM transition relation is implemented and independently reviewed. Remove first-release misleading APIs without compatibility aliases and migrate all consumers. |
| ZK04 | In progress | FASTPQ / bounded private verification | Implement a sound source-state-bound relation with reviewed trace/composition masking and bounded verifier work; fit unchanged proof and total resource limits. Produce and verify real maximum-shape proofs, negative source/witness/statement cases, and CPU/accelerator parity. Full replay and unmasked offline compact proofs do not satisfy this goal. |
| ZK05 | In progress | ZK-X509 / complete bounded credential proof | Redesign or compose the full certificate, CRL, disclosure and ownership relation below 9,437,184 bytes without reducing supported coverage. Account for all segments, recursion, openings and prover resources together. Regenerate fixed profiles and produce actual positive/negative proofs before activation. Arithmetic size projections alone are insufficient. |
| ZK06 | Open | Cryptographic qualification | Obtain independent artifact-bound soundness, zero-knowledge, Fiat–Shamir/qROM, digest/multi-target, arithmetic and side-channel evidence for the selected release protocols. Keep explicit protocol-specific blockers; do not infer qualification from implementation markers. |
| ZK07 | In progress | SDK / simple developer workflow | One typed prepare/prove/verify workflow per actual capability, actionable errors, early availability/resource checks, secure randomness and private witness ownership by default. Callers do not construct dummy inputs, choose transcript internals, or infer guarantees from backend names. Maintain executable Rust examples and equivalent supported SDK entrypoints, with public guidance in `iroha-docs`. |
| ZK08 | In progress | Validation / reproducible candidate | Reconcile current failing Halo2/note-STARK source contracts by preserving their substantive assertions. Run changed-function tests, real proof/adversarial suites, serialization guards, formatting and applicable SDK tests; then the workspace and four-validator qualification on a fixed candidate. Record exact passes, failures and unexecuted checks separately. |

## Execution order

1. Repair secret arithmetic and optional-input constraints with regression tests.
2. Make current relation guarantees explicit at the shared verifier and developer
   boundary; migrate callers and remove stale claims.
3. Use one allocation-free resource plan at developer preflight and actual prover
   admission. Resolve the mathematical construction before widening execution.
4. Complete FASTPQ and X509 constructions against fixed statement and resource
   contracts, then regenerate profiles, keys and fixtures.
5. Qualify end-to-end SDK/native/network paths and independent cryptographic
   evidence. Close each goal only with its acceptance evidence.

## Opening evidence

- Vega private row commitments trim by secret value and use variable-time MSM;
  the compiled Vega activation remains unavailable.
- Optional second confidential inputs require zero-leaf membership even when
  absent; a full 65,536-leaf tree has no such dummy leaf.
- `ivm-execution-v1` is a public-value equality circuit. Core correctness relies
  on unconditional deterministic execution replay.
- Admitted FASTPQ verification reconstructs complete batch commitments. Offline
  compact proofs are unmasked and cannot acquire production authority.
- X509's current maximum encoding is 19,156,074 bytes against a 9,437,184-byte
  ceiling. Even removing all P-256 trace-opening bytes leaves 10,452,074 bytes.
- The reviewed FASTPQ DEEP plan needs 20,199,768,064 bytes for its retained LDE
  alone; widened row openings occupy 616,448 bytes against a 524,288-byte target.
- The critique's focused Python selection passed 8 checks and failed 3 current
  source/inventory contracts. No Rust proof suite or timing attack was executed
  during that review.

## Current candidate progress

- Vega private rows now use the shared fixed-window secret MSM and clearing
  scalar owners. Removed the unused memory-ceiling API. Twenty-five native
  arithmetic, erasure and worker tests pass; no activation change. A September 28
  current-source call-site review confirms that private row/evaluation/mask
  commitments use exact-capacity secret MSM, public padding dimensions and fixed
  digit scans. Remaining variable-time MSM callers process public transcript or
  verifier values. Secret scalar/point/table owners clear, nested T256 parallel
  fan-out is disabled, and worker bounds are enforced. Together with the ZK-ACE
  checks below, this closes ZK01's software defect scope; compiler-created copies,
  device timing and independent cryptographic qualification remain ZK06.
- Optional-input transfer/full/change circuits gate root equality by presence.
  Path builders accept one path per actual input. Native depth-16 full-capacity
  one-input full/change proofs, the full-capacity list-builder boundary,
  adversarial relation cases and regenerated canonical key goldens pass. Exact
  canonical-key guards reject differing or superseded keys. Rust and all five
  supported SDK wallet workflows now have real native proof evidence; retained
  SDK full/change proofs also pass current-Core canonical decoding and typed
  verification with wrong-key/relation/cap/tamper negatives. This closes ZK02's
  implementation scope. Same-candidate four-validator qualification, signed
  distribution, device execution and independent review remain ZK08/ZK06;
  ZK07 and the overall remediation remain active.
- `zk::verify_for_relation` requires an explicit semantic role and returns typed
  failures. IVM and Kaigi consumers use it; mandatory IVM replay remains.
  Data-model docs now state validators observe execution and gas through replay.
  The first-release registry, schema, circuit, proof helpers and keygen command
  use `ivm-replay-binding-v1`; retired execution-proof labels are rejected.
  All Halo2 envelopes and key records require full canonical CIDs. Torii shares
  Core identity validation, and built-in key records emit the complete CID.
  Torii's derive, queue and prover stages now require the complete replay-binding
  identity instead of comparing it with a bare suffix. Three exact-source native
  role/backend regressions pass, including early rejection of verifier backend
  identities unsupported by this proof generator. The server derives its proving key from the
  canonical registered VK, eliminating separate operator PK files, and hashes
  overlays with the same canonical codec as Core. Normal route and four-peer
  tests remain pending; their execution fixtures use registered signed contracts.
  A retained-Core component check passes actual registration/activation,
  derivation, native proof generation and mandatory replay of the returned
  payload. This does not establish current-source HTTP or ledger admission.
  The key generator and CLI now share one strict public registration DTO;
  canonical keys, schemas and limits come from Core, signing credentials stay
  in the client configuration, and PK export is optional. Four generator
  component controls and actual public artifact output pass. The normal Cargo
  generator suite also passes all four tests. The main CLI build exposed a stale
  SoraNet dispute caller after its ledger API migration; that caller and its
  unused treasury argument are corrected with an actual-output regression.
  A command-level test also exposes JSON incorrectly sent to stderr; the
  command now uses the typed data-output API. The final amended frozen candidate
  passes all 20 selected CLI tests with no skips: four generator, eight registry,
  five job-status, one real HTTP polling and two dispute regressions. Exact
  sources remain unchanged throughout. The retained CLI generates the corrected
  help text; its public snippet refresh still requires the documentation
  repository's clean, pinned signed-source workflow. A subsequent CLI audit
  removes the misleading value-hashing `schema-hash` command and makes memo
  encoding a single bounded stdout format. VK registration and updates delegate
  to the standard fee, signing and receipt owner instead of maintaining a second
  submission path. The expanded normal run passes all 28 tests with no skips or
  source drift, including credential-free memo creation and actual HTTP polling.
  A subsequent removal of three unused helpers awaits the integrated rerun.
  Normal Torii compiles and executes all 129 selected tests: 117 pass and 12 fail.
  One failure exposes synthetic signing in the production derive/prove path;
  Core and Torii now derive from an unsigned payload while retaining request
  authentication, contract permissions and signed final admission. The other
  failures expose missing worker-supervisor startup and stale scanner fixtures.
  Repairs preserve production lifecycle and retry checks; their normal rerun
  and the four-validator suite remain pending. Six native Core controls now pass
  for unsigned derivation, contract dispatch/binding, mandatory STARK replay and
  fee/replay gas limits on the retained debug executable. Exact prior failures and the
  source-capture race rejected by the CLI guard remain in the dated record.
  Current label regressions pass in JavaScript (2 source/dist controls), Python
  (13 installed-registry and SDK source-parity controls), Swift (9), C# (87),
  and Kotlin with Java-source consumers (12).
  Python uses its normal authenticated native wheel loader; the registry module
  matches current source exactly. These checks do not establish native proof or
  admission correctness. Native admission reruns remain separate qualification
  work.
  RAM-LFE registration/activation and stateless receipt entry now reject unavailable
  proof mode before parameter or proof work; implementing its complete program
  relation remains outstanding. The shared internal proof helper now refuses too;
  its generic backend/payload-hash accepting path is removed. Native unrelated-proof
  regressions and migrated supported-verifier metadata/resource controls await the
  integrated rerun. The [execution-proof implementation contract](ram_lfe_execution_proof.md)
  records the missing policy-hash binding, bounded deterministic initial-state
  derivation, complete interpreter constraints and native/network acceptance
  criteria. No existing binding relation can substitute for that circuit.
- The Rust wallet entrypoint now owns its clearing spend key, consumes clearing
  note openings, checks shape/amount/path-index errors before key preparation,
  and selects transfer/full-redemption/private-change circuits internally.
  `ConfidentialProverError` distinguishes preflight, key and proving failures
  without string parsing. The six caller-selected-key Rust builders and their
  three result types are now internal to the ZK module. Seven external compiler
  controls pass against fresh normal Core metadata: the typed wallet/change
  helper compiles, and all six low-level imports reject with `E0603`. Public
  note/tree and verifier-key registry primitives remain available. Rust's
  consuming `change.into_input(index)` helper restores the correct default
  change owner; current shape and nondefault-input-to-change redemption tests
  pass in the captured Core retry. The three earlier real native workflow controls
  pass; the current runnable Rust example also produces and locally verifies a
  13,741-byte full-redemption proof through the public API. Its captured proof,
  wallet and codec production sources remain unchanged; unrelated concurrent
  Core changes are recorded separately. These produce local proof artifacts;
  they do not restore retired ledger instructions or authorize value movement.
  Secret note/path and internal witness Debug output is redacted. JavaScript's
  `ConfidentialProver` delegates to the Core owner without caller circuit/key
  selection. Proof methods return promises and run native work on a worker with
  clearing input ownership. Disposing the wallet closes future work while
  accepted jobs finish independently. The three caller-key builders and their
  native exports are removed. The final frozen candidate publishes through the
  normal native builder and passes 66 SDK/native/TypeScript/provenance controls
  with no skips, including four mandatory native wallet/root/change-helper cases.
  The public dist recipe proves change (14,215 bytes), restores its default
  owner and fully redeems it (13,741 bytes), with event-loop progress and an
  accepted final job surviving parent disposal. Entropy buffers are owned before
  fallible random draws. Managed JavaScript witness strings do not carry an
  erasure guarantee. Both public SDK proofs also pass independent verification
  against fresh current Core, including canonical Norito roundtrips and rejection
  of wrong relations/keys, insufficient budgets, tampering and retired formats.
  Normal external npm installation exposes the public helpers
  and correctly rejects moving a dirty native artifact outside its authenticated
  source checkout. Clean release distribution remains separate qualification.
  The [September 28 wallet/kernel receipt](../docs/history/2026-09-28/zk-wallet-and-x509-kernels.md)
  separates wrapper, native, artifact and source-contract evidence. The public
  anonymous-transaction page and 20 translations pass scoped i18n/content checks;
  independent language and final native distribution qualification remain open.
- Python now has the same typed `ConfidentialProver` workflow through the Core
  owner, with automatic key/relation selection, context-managed closure and
  explicit error codes. Native proof work releases the GIL and retains its own
  clearing key/note lifetime; bounded parsing rejects malformed evidence before
  proving. The four old caller-selected-key Python proof builders are removed;
  their exact NetworkId assertions move to the canonical wallet owner. Seven
  mocked boundary controls plus six native-free identity checks pass. The first
  native run passes six controls and exposes a test fixture that encoded path
  directions as Python bytes instead of the public helper's integer list. The
  fixture now calls that actual helper. The fresh native retry passes all seven
  controls in 30.86 seconds, including the real proof and independent Python
  thread progress while proving. All four
  Python wheels build and install in an isolated environment, but the first
  packaged example is rejected by the macOS loader for a misaligned Mach-O
  string pool. Target-only native stripping is disabled in the normal Maturin
  configuration; the rebuilt wheel installs and loads normally. The public
  example then exposed an export-order bug, now fixed by adding the wallet
  symbols to the package's base export list. The rebuilt pure SDK wheel and
  native wheel produce a locally verified 13,741-byte full-redemption proof
  through the installed public API. The refreshed native wheel passes all three
  installed regression tests in 79.919 seconds: real proof with independent Python
  thread progress, early typed amount rejection, closed-owner rejection, public
  exports and creating then redeeming retained change through the default-owner
  helper. Its installed example passes in 33.528 seconds. The installed Python
  modules match their captured source bytes. This is local
  arm64 macOS/Python 3.12 evidence, not every supported host or ledger admission.
- Swift and Kotlin now delegate local confidential proving to the same shared
  C/JNI Rust owner, with bounded note/path DTOs, automatic circuit/key selection,
  stable errors and explicit closure. Accepted jobs retain their native key until
  completion. Swift now also closes a job after a driver fails before native
  consumption, with harmless closure after successful consumption. Six current
  injected-driver lifecycle checks pass; eight earlier actual native
  note derivation/encryption/change-helper checks and Kotlin's eight boundary
  controls pass. The refreshed normal XCFramework passes all five target builds,
  native C linking, source/export checks and atomic publication. Its authenticated
  macOS consumer passes all eight current Swift controls, including three real
  proofs and retained-change redemption; the public example produces a locally
  verified 13,741-byte proof. Exact source and artifact evidence is recorded in
  [the Apple qualification receipt](../docs/history/2026-09-29/apple-confidential-wallet.md).
  The shared C bridge passes all nine native controls in 33.45 seconds, including
  a real proof after owner closure, output allocation/free, non-reused handles,
  partial-input cleanup and stable amount errors. The fresh normal JNI build
  passes all four real Kotlin consumer tests in 71.751 seconds, including three
  complete proofs and restoring then redeeming retained change. The public Kotlin
  example locally verifies a 13,741-byte proof in a 32-second Gradle run. These
  are macOS arm64 results; physical Android/Apple device execution and signed
  release qualification remain pending. The first JNI build stopped on
  six unrelated SCCP BSC integration compile errors; those sources have since
  changed; the successful JNI retry uses the retained SDK candidate with all
  nine relevant bridge/wallet source files verified equal to the current checkout.
  The original Apple publication is retained separately from the four-file Swift
  refresh and its new authenticated source seal. Both SDKs have executable public
  redemption examples. The artifact export inventories require the new C/JNI contract.
- SDK change-note helpers retain amount/rho and obtain the protocol default
  diversifier through the native Core API; they do not reuse the input note's
  potentially different diversifier. Developers securely retain the opening
  before proving and supply an authenticated change index/root when spending it.
  Swift native-note and Python/JS/Kotlin boundary controls pass. Python's new
  two-proof change-then-redemption control passes through its installed wheel;
  Kotlin's equivalent two-proof cycle now passes through the actual JNI library;
  JavaScript's corresponding public native recipe and Swift's two-proof cycle
  through the refreshed authenticated XCFramework also pass.
- Entropy samplers now guard raw bytes, partial Fp4 coefficients and the final
  mask owner before any fallible random draw. The actual-source isolated
  regression observes initialized cells clearing on entropy error, unwind and
  normal drop. Its integrated Core rerun now passes.
- X509 private owners now cover partial DER parsing, parsed certificate/CRL
  precursors, semantic witnesses, I/O materialization and assembly construction.
  The DER parser borrows header and extension spans; displaced trace allocations
  are wiped before replacement. All 35 actual-source DER controls pass. The
  private-witness codec establishes recursive ownership before decoding and
  reserves its exact encoded size before writing private bytes; all 14 isolated
  codec/owner controls and the retired-codec guard pass. The fresh integrated
  Core candidate compiled, but storage exhaustion during stripping prevented
  any tests from running. A fresh integrated run and complete proof evidence
  remain open.
  A subsequent repository-policy audit found that the local `IRX509W1` witness
  container had no independent protocol requirement. It is replaced by one
  bounded, explicitly headed Norito schema; the old decoder is removed.
  Streaming encoding and partial decoding retain clearing ownership throughout.
  The derived maximum frame is 20,398 bytes, and all 20 actual-source codec/owner
  controls pass, including checksum-valid malformed fields, old-format rejection,
  stricter outer budgets and partial-copy erasure. Native Core codec, regenerated
  preparation-schema profile pins and I/O/projection KAT assertions now pass.
  Mathematical DER preimages and
  the separately specified proof transcript are unchanged by this DTO replacement.
- Private CUDA FFT/LDE/Poseidon workspaces and pinned host buffers now erase full
  capacities after completion before pool reuse or replacement. Unknown in-flight
  completion quarantines allocations without touching or freeing active memory.
  A compiled host harness runs the actual CUDA host cleanup code across success,
  output-copy ordering, growth, drop, partial failure and quarantine paths and
  passes. Physical CUDA execution remains unqualified. Rust GPU host owners and
  Metal private staging now use clearing guards. Abandoned and failed Metal
  dispatches drain their outstanding tickets under one cleanup deadline. Unknown
  completion retains device-owned buffers and blocks subsequent proof admission,
  including a CPU retry, instead of spending another proof's memory allowance.
  The final lifetime snapshot passes 177 focused controls, 12 public API checks,
  13 transform/Poseidon checks and three required-device dispatch controls.
  The revised exact-root Metal transform also passes full-size eight-column
  native19/common22 CPU parity and independent Horner checks through the actual
  Rust API; current timings include staging, wait and cleanup. Its integration
  into X509 passes the captured Core transform selection. These checks do not
  establish side-channel resistance.
- ZK-ACE's concatenated private identity preimage has a borrowed clearing guard.
  Client preparation checks compiled availability before hashing that witness.
  Its seven focused prover tests pass, including early availability/entropy checks.
  All 23 focused data-model ZK tests pass, including preimage cleanup and binding.
  One-shot hashing also erases lane/rate, MDS and byte-packing arrays through the
  existing workspace `zeroize` library. Streams and lane snapshots now require
  explicit cloning and erase their buffers on success, failure and unwind. Cached
  private lanes and packing scratch clear too. `fastpq_isi` passes 71 native unit
  tests; one existing timing diagnostic is ignored. Strict all-target Clippy
  passes for this extension. Accelerator memory and target-specific
  side-channel qualification remain open; no guarantee is made about
  compiler-created scalar/register copies.
- FASTPQ's normal offline Quantity route uses the bounded masked DEEP producer:
  base-field trace replay, zero-remainder quotient division, independently
  masked quotient chunks and composition, coefficient/FRI replay, canonical
  encoding and independent bounded verification. The sealed relation binds the
  complete prepared ordinary/AXT statement and identity. The unchanged limits
  remain 524,288 child-proof bytes, 2 GiB per segment and 2^42 work units.
  Impossible plans reject before private work. Device readiness precedes private
  work, and RequiredMetal never falls back to CPU.
  A same-attempt cache retains internal nodes for seven committed oracles;
  selected leaves and level-zero siblings are regenerated after querying.
  Coverage, natural coordinates, immutable context identity and the committed
  root are checked before opening. Rejected writes poison the cache, and private
  owners clear on success, failure and unwind. Complete cache payloads, coverage,
  pending owners and opening scratch are charged together with replay/FFT work.
  The exact measured context admits 1,900,861,550 payload bytes and
  3,475,021,175,280 work units without widening either cap.
  The fresh native selection passes 203 controls and initially fails one stale
  fixture using the retired commit-with-queries API. The fixture now follows
  root commitment, same-attempt binding and opening; both affected producer
  fixtures pass on the fresh binary with all independent root/mutation assertions
  preserved. Twelve public API checks and three required-device controls pass.
  Geometry/hiding/source-budget checks pass all 37 cases. Full-size exact-root
  CPU/Metal and independent Horner parity are separately captured kernel evidence.
  The complete cached RequiredMetal proof passes construction, self-check and
  independent verification: 482,978 bytes, exactly the pre-cache proof hash,
  948.983 seconds construction/self-check and 1,918,730,240-byte maximum RSS.
  A test-only follow-up retains the same raw public child and passes independent
  verification without reproving, including five changed-statement controls.
  It observes 907.486 seconds construction/self-check and 1,919,795,200-byte
  maximum RSS; separate verification takes 2.15 seconds and 19,644,416 bytes.
  Exact immutable binaries, source captures and public artifacts are retained.
  Complete CPU-versus-Metal proof parity passes on the same fixture and immutable
  binary: both produce the exact 482,978-byte public child. The CPU run takes
  2,532.67 seconds with 1,895,317,504 bytes maximum RSS; separate artifact-only
  verification and mutations pass in 2.07 seconds. These runs use the unoptimized
  FASTPQ caller and optimized ISI arithmetic. These contended observations are
  not throughput qualification. A separately retained optimized normal binary
  completes a two-child ordinary proof: 968,475 bytes, 1,726.794 seconds for
  construction/self-check, followed by independent artifact verification in
  7.62 seconds. Its maximum process RSS is 2,174,222,336 bytes, exceeding 2 GiB
  by 26,738,688 bytes; the unchanged structural payload admission passed, which
  does not establish a 2 GiB process bound. The same-binary AXT run also passes:
  971,571 bytes, 1,727.694 seconds construction/self-check and 2,087,878,656
  bytes maximum RSS; independent artifact verification passes in 13.99 seconds.
  The subsequent source owner transfers the physical trace into replay and
  releases its redundant 179,306,496-byte allocation immediately. The complete
  shared Metal pool allowance now follows its lifetime across digest and CPU
  quotient phases. The fixed-SMT plan charges 1,967,970,414 bytes without
  widening the 2 GiB or work ceilings. Thirty-nine native ownership/resource
  controls and 15 public API controls pass. Fresh same-seed proof-byte parity
  and same-shape ordinary/AXT process-memory measurements are running; the
  structural charge alone does not close the observed RSS overrun.
  Earlier normal-facade captures retain an ordinary 485,600-byte artifact and an
  AXT 484,750-byte artifact, each independently verified with context, cap, shape
  and transport rejection controls. Both artifacts also pass the current
  bounded verifier without reproving. Exact sources, earlier failures and timings
  are in the [September 28 receipt](../docs/history/2026-09-28/fastpq-masked-native-validation.md).
  Core's ordinary lane uses the canonical masked artifact against its exact
  finalized statement; its roots cover the touched-balance tree. Relay records
  separately authenticate finalized QC-bound roots. AXT artifact consistency
  alone grants no remote-spend or business-effect authority. Developers derive
  exact verifier expectations through `ExpectedStatement::from_statement`, which
  streams the canonical frame without retaining an encoded copy. Its Rustdoc
  example compiles; the API includes bounded prove/verify and worker, busy and
  device-error guidance. Complete hiding/soundness review, maximum application
  shapes and source-bound network qualification remain open.
- Reviewed Halo2/note-STARK/FASTPQ source-contract repairs pass their focused
  29-check Python selection. The retired-codec guard passes. The current JS
  confidential input and TypeScript checks pass 6 cases including mocked
  full-capacity one-input forwarding. Native SDK execution is separate; the
  refreshed frozen addon now has the scoped native evidence above. Workspace
  formatting check finds concurrent unfinished
  formatting, so it is not a pass; changed files are formatted by their owners.
- X509 replay now batches native interpolation and SHA extraction, groups DEEP
  division while checking every individual claim, and caches a public prefix of
  quotient coefficients within the unchanged arithmetic/source envelope.
  Independent actual-source kernel controls pass; the fixed-layout cache census
  reduces quotient IFFT replays from 20,126 to 8,281. Current/next polynomial
  values, masks, transcript, proof encoding and caps remain unchanged. Focused
  Core checks pass; maximum-proof qualification is still pending. Exact scope and baseline
  corrections are in the [September 28 X509 work record](../docs/history/2026-09-28/zk-x509-exact-root-and-deep-work.md).
- X509 now joins all six native MAIN column groups under one base and one
  auxiliary commitment while preserving all 49 registrations. The codec opens
  only the authenticated current rows; full Fp4 DEEP checks bind both current
  and native-next claims before the shared FRI verifier. Paired FRI leaves and
  the corrected RFC temporal relation give a codec-derived combined maximum of
  9,420,938 bytes, leaving 16,246 bytes under the unchanged ceiling. Nine geometry
  checks pass. On the earlier joined profile, fresh
  real compact-CA proof roundtrip, credential-context, public/root/DEEP/FRI/query/
  frontier mutation and resource-gate controls pass. Native I/O and projection
  proofs verify and re-encode canonically; their obsolete KAT literals and the
  independently computed 29-field engine pin are updated, and fresh pinned
  assertions pass. No maximum-shape proof or
  resource qualification is claimed from this bound. Independent code review
  found no reduced-opening arithmetic or joined-root binding blocker.
  All component-family polynomial kernels and verifier-derived fixed schedules
  are implemented. Independent interpolation tests cover nonboolean off-domain
  selectors. SHA polynomial selection has its actual degree six registered;
  binding sinks use their actual degree three. Both stay within the existing
  degree ceiling, masking and quotient chunk counts. Activation still requires
  independent soundness and hiding review, real proof vectors, and resource
  evidence on the final candidate.
  The whole-prover audit found that retaining all masked coefficients alone used
  16,226,947,392 bytes, above the unchanged 12 GiB ceiling. The implementation now
  retains the original 81,690,944 bytes of masks and replays columns from closed
  immutable source owners, preserving entropy order and committed polynomials.
  Quotient evaluation uses interleaved stripes of at most 2^19 rows, including
  all higher masked coefficients and exact native-next translation. Six isolated
  native tests using the actual field/FFT, stripe and private-owner source pass;
  the refreshed Core kernel and ownership selection also passes. Private replay/quotient/composition owners
  now use guaranteed zeroization. The transformed-buffer ledger and recursive
  borrowed assembly/P256/DER capacity checks are implemented. Preconstruction
  forecasts admit source dimensions against separate source/scratch allowances;
  actual capacities are rechecked before masking, binding and finishing. Native
  structural maximum-shape assembly admits 288,345,698 bytes of owned source
  payload against its 600,114,496-byte allowance and constructs in 4.734 seconds
  (474,808,320 bytes process maximum RSS). This is assembly, not a complete proof.
  Measured common-domain CPU transforms imply a forward-only linear estimate of
  428 seconds, already above the 300-second whole-proof target. A bounded exact-root
  GPU adapter with explicit scratch admission is in progress. Full maximum proof
  time/memory measurements remain open; no whole-prover compliance is inferred
  from component measurements or the explicit runtime reserve.
- The first Core build stopped on variant-size lints before producing key
  digests. The typed-error enum and an explicitly justified bounded inline
  consensus-parent enum expectation resolve those source blockers. The rebuilt
  native circuit generated replacement full/change unshield key digests; pins
  are updated. A stale Vega build
  captured dependency metadata before the `zeroize` manifest change; its fresh
  locked rerun and the additional native commitment suites now pass. The next
  Core compile found a compact-CA alpha shape mismatch and a concurrent SoraFS
  fixture mutation error; both sources are repaired and the fresh run is queued.
  The September 28 build reached Core but encountered a concurrent reputation/
  archive API migration before executing tests. The later broad native Core run
  compiles in 8m03s and completes 479 selected tests in 999.67s: 466 pass, 10 fail,
  three are ignored. All ten failures are repaired in source: three engine-pin
  dependent checks, two protocol KAT literals, one SHA block-count expectation
  and four positive fixtures still using a retired circuit-ID label. A fresh
  targeted run compiles and completes with 236 passes, two failures and three
  ignored checks in 302.29 seconds. All repaired ZK pins, KATs, circuit and private
  owner controls pass. The two failures are direct SCCP component fixtures that
  opened unauthenticated raw transactions; both now use the existing explicit
  component-test constructor and await their focused rerun. Production transaction
  authentication is unchanged. A separate full P-256 trace/cross-table selection
  passes all 19 tests in 12.55 seconds.
  The complete 49-family MAIN OODS differential test, typed relation-confusion
  negatives and unavailable RAM-LFE preflight pass in the broad run. The refreshed
  source-contract/geometry selection separately passes 21 checks; the codec and
  historical archive guards also pass.
- The final Norito/X509/wallet selection compiles and passes 106 controls with
  six ignored diagnostics. All selected codec, profile/KAT, cleanup, replay,
  resource, transform and real wallet-proof controls pass. Eleven additional
  relation, full-tree, optional-input and public-column controls pass on that
  same retained executable. One ordinary queue fixture fails because the bundled
  default genesis policy pin predates current ZK/SCCP consensus inputs; the
  existing State parity test independently reproduces it. The pin is regenerated
  from current public Core APIs and independently checked with framed SHA-256.
  No genesis validation or fixture bypass changes. The optimized normal Core
  binary passes both State pin parity and ordinary certified queue/replay
  regressions. Its maximum X509 diagnostic fails with `DerWitness` before
  producing a proof, after 1,159.68 seconds wall time and 7,964,540,928 bytes
  maximum RSS. The unchanged whole-proof target is 300 seconds; actual transform
  backend use was not observed in this run. Complete source-column preflights
  subsequently identify DER row-state bugs and omitted embedded documents and
  RFC witness fields. DER producer repairs, private-buffer cleanup and bounded
  initial mask-source batching preserve their relation. The RFC calendar scan
  additionally finds overlapping month/time columns and incorrect Gregorian
  leap arithmetic. The corrected 285-base/280-aux/102-fixed, degree-four relation
  binds the complete authenticated 72-time census, 73 fixed comparison slots
  and nonwrapping 38-bit differences. Ordinary and maximum actual-source RFC
  preflights pass every base/aux column and populated AIR boundary; full Fp4,
  degree and adversarial controls pass. Reusing one public fixed matrix across
  quotient stripes restores a 596,974,144-byte whole-assembly allowance. Its
  independent arithmetic/ownership controls pass, but the current whole-assembly
  native measurement and new profile pins remain pending. Details are retained
  in the [September 29 source record](../docs/history/2026-09-29/zk-x509-rfc-temporal-repair.md).
  Supported coverage and resource ceilings are unchanged.
  A new maximum proof requires the full focused regression pass and a freshly
  captured normal optimized binary.
- The renamed CLI graph's complete 10-test module now passes under Python 3.12
  on the current source, including the earlier four mock-boundary failures.
  `zk ivm prove --wait` now returns failure for a failed proof job, preserves its
  terminal JSON and verifies the requested job identity before handling a result.
  Five exact-source native status/output tests pass with warnings denied and a
  separate source review finds no blocker. All five also pass in the normal CLI,
  and actual HTTP polling passes all terminal outcome/identity cases through
  the retained shipping executable. Current Torii and network runs remain open.
  The workspace target inventory separately reports a concurrently added
  KAGEMUSHA binary outside the reviewed inventory; that check is not a pass.

## Evidence discipline

The public FASTPQ guide and all 20 translations now pass scoped i18n/content
validation; the actual Rust snippet compiles against the normal library.
Browser checks preserve RTL prose and LTR code in Arabic, Hebrew and Urdu.
The implementation plan is reconciled with the current masked protocol, with
its exact superseded source archived once. These checks improve the developer
contract; they do not close maximum-shape, native distribution, network or
independent cryptographic qualification. The transparent-API removal is applied:
normal builds expose the typed masked facade, and replay APIs require `dev-tools`
or tests. Normal facade, compile-fail, AXT and dev-only controls pass. Two-child
ordinary and AXT proof qualification uses a retained normal optimized binary;
both construction and artifact replay pass with the ordinary RSS limitation
above. The new source-lifetime repair's full proof and memory runs, and broader
resource qualification, remain open.

Implementation and local tests may close individual software defects. They do
not close independent audit, proof-system completeness, production activation,
physical hardware, signed artifact or network qualification requirements.
Keep historical observations in dated validation records and current blockers
in this goal ledger. Do not remove failed checks or bless changed source hashes
without reviewing the behavior and test coverage they protect.

- The C# SDK now exposes the typed disposable wallet over the shared Core C ABI,
  with bounded note/path owners, asynchronous accepted-job lifetime, automatic
  relation/key selection and `ConfidentialChangeNote.ToInput(index)` for restored
  change. Collection sizes are captured once before allocation; private copies
  clear on normal, rejected and dispatch-failure paths. The final selection passes
  10 controls with no skips (seven managed, three actual native), and its public
  example proves change then full redemption after parent disposal. Twenty-six
  artifact-script controls and the current C# lane's actual ABI/export probe pass.
  The clean-source release-manifest gate correctly rejects the dirty candidate;
  five-RID NuGet distribution and later unrelated Core changes remain unqualified.
  Exact sources, prior failures and macOS arm64 evidence are retained in the
  [C# wallet receipt](../docs/history/2026-09-28/csharp-confidential-wallet.md).
