# ZK first-release completion goals

Set: 2026-09-26. Overall status: **Active**.

This record owns the remediation requested after the current-source ZK critique.
It supplements [first-release completion](first_release_completion_goals.md) and
[privacy closure](privacy_first_release_closure.md). Work remains in the existing
`optimizations` checkout. Unrelated concurrent changes are preserved.

The outcome is useful, safe proof algorithms with simple developer APIs. A
renamed placeholder, disabled feature, new status object, passing source hash,
or relaxed resource ceiling does not complete an algorithm. External review
and physical-device evidence cannot be replaced by self-issued certificates.

## Goals and acceptance criteria

| ID | Status | Owner / outcome | Completion criteria |
| --- | --- | --- | --- |
| ZK01 | Complete (implementation) | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. Target timing and independent qualification remain ZK06. |
| ZK02 | In progress | Confidential circuits / optional inputs | One owned note can be fully redeemed at maximum tree capacity. Absent inputs require no empty-leaf membership or caller-created dummy witness. Ownership, nonzero/duplicate-nullifier, range and conservation constraints remain enforced. Regenerate all changed circuit keys, digests and dependent fixtures; reject superseded keys. Positive and adversarial circuit/native/SDK tests pass. |
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
  Path builders accept one path per actual input. Native full-capacity one-input
  path/list proofs, adversarial relation cases and regenerated canonical key
  goldens pass. The wallet's real transfer/full/change controls also pass; final
  source-bound reruns and packaged SDK qualification remain open.
- `zk::verify_for_relation` requires an explicit semantic role and returns typed
  failures. IVM and Kaigi consumers use it; mandatory IVM replay remains.
  Data-model docs now state validators observe execution and gas through replay.
  The first-release registry, schema, circuit, proof helpers and keygen command
  use `ivm-replay-binding-v1`; retired execution-proof labels are rejected.
  All Halo2 envelopes and key records require full canonical CIDs. Torii shares
  Core identity validation, and built-in key records emit the complete CID.
  Cross-SDK labels and native alias/admission regressions are being validated.
  RAM-LFE registration/activation and stateless receipt entry now reject unavailable
  proof mode before parameter or proof work; implementing its complete program
  relation remains outstanding.
- The Rust wallet entrypoint now owns its clearing spend key, consumes clearing
  note openings, checks shape/amount/path-index errors before key preparation,
  and selects transfer/full-redemption/private-change circuits internally.
  `ConfidentialProverError` distinguishes preflight, key and proving failures
  without string parsing. The six caller-selected-key Rust builders and their
  three result types are now internal to the ZK module; six external rustdoc
  rejection guards await compilation with the final Core candidate. Public
  note/tree and verifier-key registry primitives remain available. Rust's
  consuming `change.into_input(index)` helper restores the correct default
  change owner; shape and nondefault-input-to-change redemption tests await
  the same final Core run. The three earlier real native workflow controls
  pass; the
  runnable local redemption example awaits its fresh executable check. These produce local proof artifacts;
  they do not restore retired ledger instructions or authorize value movement.
  Secret note/path and internal witness Debug output is redacted. JavaScript's
  `ConfidentialProver` delegates to that Core owner, without caller circuit
  or key selection. Its proof methods now return promises and execute native
  proof work on a worker; the native job owns clearing input copies and consumes
  them exactly once. Disposing the wallet closes future work while queued jobs
  finish independently. All three caller-key JavaScript builders and their native
  exports are removed. Twenty-one current boundary/cardinality/TypeScript checks
  pass, including deferred success, worker rejection, immediate FFI-key cleanup,
  reentrant disposal, retired-option rejection and non-Error input failures.
  A bounded asynchronous root helper and executable redemption recipe use the
  public API; the recipe queues work, closes the owner and observes event-loop
  progress while the accepted job finishes.
  Ten native worker and parsing tests pass in 37.91 seconds, including public
  root-helper parity and a real full-redemption proof self-verified on a worker.
  The normal macOS builder disables stripping only for the final addon, fixing
  the actual loader's misaligned string-pool rejection; 133 build/provenance
  controls pass. The frozen candidate builds and publishes through normal
  provenance checks, passes 24 captured SDK/native/TypeScript controls with no
  skips, and exposes its native root helper through the public dist entrypoint.
  Its redemption recipe self-verifies a 13,741-byte proof after disposal while
  recording 2,717 event-loop ticks. This qualifies the captured source/dist
  loader; a clean installed package still requires release native distribution
  and provenance qualification. Newer change helpers are outside that snapshot.
  Managed JavaScript witness strings do not carry an erasure guarantee.
  The [September 28 wallet/kernel receipt](../docs/history/2026-09-28/zk-wallet-and-x509-kernels.md)
  separates mocked API, real native wallet, arithmetic and source-contract evidence.
  The rewritten public anonymous-transaction page and 20 translations pass their
  scoped i18n/content checks; independent language and native SDK qualification
  remain open.
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
  completion. Swift's five injected-driver lifecycle checks, eight actual native
  note derivation/encryption/change-helper checks and Kotlin's eight boundary
  controls pass. Swift's
  executable consumer now links after narrowing force-loading to NoritoBridge.
  The shared C bridge passes all nine native controls in 33.45 seconds, including
  a real proof after owner closure, output allocation/free, non-reused handles,
  partial-input cleanup and stable amount errors. The fresh normal JNI build
  passes all four real Kotlin consumer tests in 71.751 seconds, including three
  complete proofs and restoring then redeeming retained change. The public Kotlin
  example locally verifies a 13,741-byte proof in a 32-second Gradle run. These
  are macOS arm64 results; Android devices and authenticated XCFramework consumer
  proofs remain pending. The first JNI build stopped on
  six unrelated SCCP BSC integration compile errors; those sources have since
  changed; the successful JNI retry uses the retained SDK candidate with all
  nine relevant bridge/wallet source files verified equal to the current checkout.
  The frozen Apple artifact build was restarted after storage exhaustion, with
  unchanged source and provenance checks. Both have executable public-API redemption
  examples. The artifact export inventories require the new C/JNI contract.
- SDK change-note helpers retain amount/rho and obtain the protocol default
  diversifier through the native Core API; they do not reuse the input note's
  potentially different diversifier. Developers securely retain the opening
  before proving and supply an authenticated change index/root when spending it.
  Swift native-note and Python/JS/Kotlin boundary controls pass. Python's new
  two-proof change-then-redemption control passes through its installed wheel;
  Kotlin's equivalent two-proof cycle now passes through the actual JNI library;
  corresponding Swift/JS native controls await refreshed artifacts.
- Entropy samplers now guard raw bytes, partial Fp4 coefficients and the final
  mask owner before any fallible random draw. The actual-source isolated
  regression observes initialized cells clearing on entropy error, unwind and
  normal drop. Its integrated Core rerun remains pending.
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
  stricter outer budgets and partial-copy erasure. Native Core and regenerated
  preparation-schema profile pins remain pending. Mathematical DER preimages and
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
  into X509 still needs a source-bound Core rerun. These checks do not establish side-channel resistance.
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
- FASTPQ's normal offline Quantity route now uses the bounded masked DEEP
  producer: base-field trace replay, zero-remainder full quotient division,
  independently masked quotient chunks and composition, coefficient/FRI replay,
  canonical encoding and independent bounded verifier self-check. The sealed
  relation owner binds the complete prepared ordinary/AXT statement and identity.
  The 301-column trace replay subtotal is 499,759,968 bytes; other active/retained
  buffers and repeated passes are separately charged. Its actual canonical child
  envelope is 502,895 bytes under the unchanged 524,288-byte cap. The 2 GiB segment
  charge and 2^42 work-unit default are preserved; impossible plans fail closed.
  Exact required-Metal measurements support bounded 1,024-job batches. The current
  candidate batches both leaves and independent lower Merkle parents; upper
  parents retain canonical row order on CPU. Exhaustive root/frontier parity,
  partial-failure and actual-capacity tests pass in the current 134-check native
  selection (nine diagnostics excluded); three required-device Metal checks
  pass separately. Public device readiness precedes private work, with no
  required-device fallback. The refreshed optimized build passes on the recorded
  203-file source snapshot. The earlier full 8M-row four-lane FFT/inverse
  diagnostic passes in 69 seconds with
  553,549,824 bytes maximum RSS. This measures one transform pair, not a proof.
  The current geometry, hiding-screen and source-budget Python selection passes
  37 checks. Exact commands and scoped digests are in the
  [September 28 receipt](../docs/history/2026-09-28/fastpq-masked-native-validation.md).
  Independent
  sequencing review found no blocker in the retained masked construction. The
  full-size RequiredMetal child-proof run completes successfully: 482,978
  canonical bytes, 4,344.265 seconds construction/self-check, and 1,118,158,848
  bytes process maximum RSS. This is actual masked proof generation and
  independent verification under the unchanged limits on the captured binary;
  the host was heavily contended, so it is not throughput qualification.
  The normal public AXT facade now generates and independently verifies a
  484,750-byte artifact containing a 481,729-byte child proof, with all context,
  cap, shape and transport rejection controls passing. Construction/self-check
  takes 2,045.209 seconds with 1,109,753,856-byte maximum RSS on the contended host.
  The ordinary facade also passes on the same immutable final-lifetime capture:
  485,600-byte artifact, 483,777-byte child, 5,247.025 seconds construction/self-check
  and 1,110,228,992-byte maximum RSS. Its same-artifact controls pass in 2.405
  seconds. Both timings are contended observations, not latency qualification.
  Complete hiding and soundness review,
  maximum application shapes and source-bound network qualification remain open.
  Core's ordinary lane uses the canonical masked artifact against its exact
  finalized statement; its roots cover the touched-balance tree. Relay records
  separately authenticate finalized QC-bound roots. AXT artifact consistency
  alone grants no remote-spend or business-effect authority.
  Developers can derive exact verifier expectations through
  `ExpectedStatement::from_statement`, which streams the canonical frame without
  retaining an encoded copy. All 11 external API checks pass, including ambient
  codec flags and complete-statement mutation controls. The API includes a
  bounded prove/verify example and explicit worker, busy and device-error guidance.
  Its Rustdoc example compiles successfully.
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
  values, masks, transcript, proof encoding and caps remain unchanged. Full Core
  and maximum-proof qualification are still pending. Exact scope and baseline
  corrections are in the [September 28 X509 work record](../docs/history/2026-09-28/zk-x509-exact-root-and-deep-work.md).
- X509 now joins all six native MAIN column groups under one base and one
  auxiliary commitment while preserving all 49 registrations. The codec opens
  only the authenticated current rows; full Fp4 DEEP checks bind both current
  and native-next claims before the shared FRI verifier. Paired FRI leaves and
  this layout give a codec-derived combined maximum of 9,204,362 bytes, leaving
  232,822 bytes under the unchanged ceiling. Eight geometry checks pass. Fresh
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
- The renamed CLI graph's complete 10-test module now passes under Python 3.12
  on the current source, including the earlier four mock-boundary failures.
  The workspace target inventory separately reports a concurrently added
  KAGEMUSHA binary outside the reviewed inventory; that check is not a pass.

## Evidence discipline

Implementation and local tests may close individual software defects. They do
not close independent audit, proof-system completeness, production activation,
physical hardware, signed artifact or network qualification requirements.
Keep historical observations in dated validation records and current blockers
in this goal ledger. Do not remove failed checks or bless changed source hashes
without reviewing the behavior and test coverage they protect.

- The C# SDK inventory found no equivalent local confidential wallet workflow.
  A typed disposable owner, bounded note/path values, asynchronous proving and
  retained-change redemption are being added over the existing shared C ABI.
  Required-symbol packaging checks and executable native consumer tests are
  part of this work; five-RID distribution qualification remains separate from
  local macOS host evidence.
