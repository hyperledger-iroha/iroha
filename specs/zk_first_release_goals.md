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
| ZK01 | In progress | Vega / secret-safe proving | Every private commitment uses public work dimensions and constant-time secret arithmetic; no raw uncleared witness copies cross the MSM boundary. Differential arithmetic, zero/partial/full row, failure/cleanup and worker-bound tests pass. Report resource limits only when actually enforced. Review other reachable secret hashing scratch, including ZK-ACE. |
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
  arithmetic, erasure and worker tests pass; no activation change.
- Optional-input transfer/full/change circuits gate root equality by presence.
  Path builders accept one path per actual input. Native golden/key regeneration
  and full-capacity/adversarial validation are in progress.
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
  without string parsing. A runnable local redemption example and three real
  native workflow controls are queued. These produce local proof artifacts;
  they do not restore retired ledger instructions or authorize value movement.
  Secret note/path and internal witness Debug output is redacted. JavaScript's
  `ConfidentialProver` now delegates to that Core owner, without caller circuit
  or key selection; transfer/full/change workflows, exact amounts, failures,
  key-copy disposal and TypeScript shapes pass 12 focused checks. Its new native
  entrypoints and clearing hex/string decoding are queued for Rust validation.
  Managed JavaScript witness strings do not carry an erasure guarantee.
  The rewritten public anonymous-transaction page and 20 translations pass their
  scoped i18n/content checks; independent language and native SDK qualification
  remain open.
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
- FASTPQ's offline API and real producer share one allocation-free resource
  calculation. Impossible shape/byte policies reject before private-tree work.
  The actual producer now replays coset stripes from retained coefficients:
  its trace subtotal falls from a 1,434,451,968-byte LDE matrix alone to
  358,612,992 bytes for coefficients and one stripe. Other memory and repeated
  passes are separately charged. The private DEEP candidate now connects masked
  trace replay, exact quotient division, independent composition masking,
  committed coefficient/FRI replay, OOD checks, canonical encoding and independent
  verifier self-check. The fresh kernel run passes 80 tests, including the prior
  malformed public-digest fixture correction and private framing cleanup.
  The complete candidate is test-only; its full-size diagnostic and source-state
  admission remain outstanding. A bounded scalar debug sample indicates many
  hours for full execution; bounded parallel hashing and release measurements
  are being implemented before making any performance claim.
- Reviewed Halo2/note-STARK/FASTPQ source-contract repairs pass their focused
  29-check Python selection. The retired-codec guard passes. The current JS
  confidential input and TypeScript checks pass 6 cases including mocked
  full-capacity one-input forwarding. Native SDK execution is separate; the
  existing addon rejects the changed source tree's provenance. Workspace
  formatting check finds concurrent unfinished
  formatting, so it is not a pass; changed files are formatted by their owners.
- X509's prover, verifier and exact codec now use authenticated paired FRI leaves.
  The current maximum projection falls by 867,456 bytes to 18,288,618; seven
  geometry checks pass. Common-domain grouping and full extension-field AIR
  evaluation remain required for the projected 9,204,362-byte candidate. That
  projection is not proof-size or soundness qualification. Compact CA now
  checks its complete quotient at Fp4 DEEP openings. MAIN fixed-schedule Fp4
  evaluation and all component-family polynomial kernels are implemented;
  typed registration integration and native validation are in progress.
  Independent interpolation tests cover nonboolean off-domain selectors. The
  SHA capacity selector now uses polynomial selection and its actual degree six
  is registered. Binding sinks also contained degree-three terms despite a
  degree-two registration; the degree and quotient accounting are corrected.
  Both stay within the existing global degree ceiling, masks and chunk counts.
- The first Core build stopped on variant-size lints before producing key
  digests. The typed-error enum and an explicitly justified bounded inline
  consensus-parent enum expectation resolve those source blockers. The rebuilt
  native circuit generated replacement full/change unshield key digests; pins
  are updated and the complete focused Core suite is queued. A stale Vega build
  captured dependency metadata before the `zeroize` manifest change; its fresh
  locked rerun and the additional native commitment suites now pass. The next
  Core compile found a compact-CA alpha shape mismatch and a concurrent SoraFS
  fixture mutation error; both sources are repaired and the fresh run is queued.
- The renamed CLI graph checks pass 5 focused tests under Python 3.12. Its wider
  10-test module has four observed mock-boundary failures (`/fixed/cargo`), and
  the workspace target inventory reports a concurrently added KAGEMUSHA binary
  outside the reviewed inventory. Neither check is recorded as a pass.

## Evidence discipline

Implementation and local tests may close individual software defects. They do
not close independent audit, proof-system completeness, production activation,
physical hardware, signed artifact or network qualification requirements.
Keep historical observations in dated validation records and current blockers
in this goal ledger. Do not remove failed checks or bless changed source hashes
without reviewing the behavior and test coverage they protect.
