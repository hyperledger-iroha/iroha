# FASTPQ production readiness

Updated: 2026-09-30. **Production qualification is unavailable.** Canonical masked
quantity proving and bounded verification are implemented. Local proof tests,
resource plans and hardware measurements establish only their captured scope;
they do not replace independent cryptographic or deployment qualification.

The [DEEP protocol contract](fastpq_deep_protocol_contract.md) owns the current
relation, geometry, transcript and wire rules. The
[implementation plan](fastpq_plan.md) owns remaining outcomes. This record summarizes the current implementation and remaining qualification.

## Current masked integration

Core ordinary transfers and AXT envelopes use the canonical masked quantity
artifact verifier through `fastpq_prover::offline_compact`. The fixed child has
65,536 trace rows, 301 committed private columns, 41 reconstructed public
columns, 923 AIR slots, 8,388,608 evaluation rows and 77 distinct queries. Its
five FRI folds use fixed arities `[16,16,8,8,4]`; the degree progression
`[131072,8192,512,64,8,2]` binds the final degree-`<2` check. The exact maximal
canonical frame is 500,084 bytes, including the composition-mask opening.

Fresh cryptographic entropy supplies subgroup-vanishing trace masks, paired
quotient masks and an independently committed composition mask. The integrated
profile uses 162 base-field mask coefficients per private trace column and 78
extension-field quotient-mask coefficients. The complete relation is evaluated
at the OOD point; exact numerator division rejects nonzero remainders.
Independent whole-transcript hiding, soundness and Fiat–Shamir/qROM review remain
required. Public statement values are not hidden.

Each verifier derives its complete expected statement from independently
authenticated context. Ordinary roots cover the touched-balance tree, not the
complete persisted ledger. AXT artifact consistency additionally binds its
outer context, but does not authenticate source finality or authorize remote
spending. Core rejects unanchored remote spending and standalone non-null
`AXT_VERIFY_DS_PROOF` promotion. Finalized relay and fee-vault paths retain
separate authorization and source-state checks. Opaque metadata carriers cannot
acquire transfer authority; catalogued mint, burn and other operations need their
own complete relations.

## Developer and resource contract

The normal quantity API owns circuit, transcript, masking and geometry choices.
Callers supply a statement, its independently derived `ExpectedStatement` and
explicit proving/verification limits. The producer verifies its output before
returning. The public [developer guide](https://docs.iroha.tech/blockchain/fastpq)
and crate Rustdoc demonstrate this workflow. `prove_axt_bound_batch` exposes the
same canonical producer directly for bound batches. Transparent `Proof`,
`Prover` and replay verification are restricted to tests and `dev-tools`, with
no normal-library compatibility aliases. Binding rejects a transfer transcript
from another source before changing metadata. The q77 candidate needs current native normal-API, precise preflight, mapped
retirement, compile-fail and complete proof/replay controls. Previously passing
two-child and maximum artifacts bind the prior profile and cannot qualify this
changed source. No retired proof engine, decoder or compatibility route remains.

`quantity_artifact_resources(segments, maximum_segment_statement_bytes)` provides
witness-free planning. A zero statement length gives a floor, not an exact
admission result. Actual proving separately admits complete contexts, private
SMT source dimensions and checked allocation/work limits before private work.
Whole artifacts and cumulative decoding have additional bounds.

Defaults remain 524,288 bytes per child, 2 GiB charged construction payload per
segment and 2^42 structural work units. Current q77 native measurements and genuine profile/proof pins remain pending.
The structural plan includes the complete 64 MiB retained Metal pool during the CPU quotient
phase as well as digest dispatches. These are structural charges, not process
RSS reservations or latency promises.
The same limits include retained internal Merkle nodes, coverage, pending
owners and opening scratch. Same-attempt caches bind immutable context and the
original roots; selected leaf stripes are regenerated and every opening must
reconstruct its committed root before disclosure.

Only one producer runs per process; bundle segments are sequential. Applications
run synchronous proving on a worker and handle `ProvingError::Busy` using a
bounded queue or retry policy. CPU is the default. Explicit required-device
execution checks availability before private work and never silently falls back.
Metal accelerates bulk leaf and lower-parent hashing; this is not whole-proof
GPU execution. Unknown device completion quarantines retained buffers and blocks
future proof admission in that process; recovery requires a fresh process.

## Captured proof evidence

The following captures describe their earlier source/profile only. They are
not q77 SHA3/SHAKE or retirement evidence. Current native generation, independent
replay, exact pin review, complete CPU/SIMD/Metal parity and final-source network
validation must be rerun before any current-candidate qualification claim.

The [two-child memory-repair measurement](../docs/validation/fastpq/two-child-memory-repair.md)
records complete ordinary and AXT proofs after physical-source release and
retained Metal-pool admission. Each artifact passes a separate no-prover replay.
The [maximum-occupancy measurement](../docs/validation/fastpq/maximum-occupancy-proof.md)
covers four distinct account keys and the largest admitted AXT context.

These measurements bind their captured source and immutable binaries. Required
Metal handles digest work and CPU handles arithmetic. The host had concurrent
work; timings establish neither a speed ratio nor deployment latency. Structural
payload admission and measured process RSS are distinct. Neither qualifies all
application shapes, the changing checkout, source-state authority or independent
cryptographic security.

## Completion goals

| Owner | Required outcome | Remaining evidence |
| --- | --- | --- |
| API | One canonical normal proving/verification workflow | Current q77 normal API, retained and migrated controls, authentic descriptor/proof pins, complete producer/replay and finalized-network checks remain pending. |
| Relation and resource owners | Complete supported application shapes within unchanged limits | Rerun two-child and maximum four-key ordinary/AXT-context proofs and independent replays on q77 under unchanged caps; broader shapes and fleet time/RSS remain open. |
| Hardware owners | Deterministic complete proofs and bounded failures | Preserve complete same-fixture parity while measuring optimized public facades; qualify actual device cleanup/quarantine and supported hardware. No missing-device skip counts as a pass. |
| Cryptographic reviewers | Independently qualified masked protocol | Artifact-bound soundness, hiding, Fiat–Shamir/qROM, concrete SHA3/SHAKE framing and multi-target and arithmetic/side-channel analysis. |
| Core/Nexus | Authorized source-state and business effects | Exact finalized roots, intent, proof-bound hidden amounts, durable atomic spend nonces, budget/custody checks, restart/recovery and four-validator adversarial admission on one candidate. |
| Release owners | Reproducible source, artifacts and consumers | Fixed source/lockfile/toolchain, native and SDK consumers, signed provenance, applicable workspace/Clippy tests and network qualification; failures and unexecuted checks remain explicit. |

The [hiding construction](fastpq_deep_hiding_construction.md) and
[digest qualification boundary](fastpq_compact_digest_security.md) state the
remaining cryptographic obligations. The compact tree commitments are opaque
SHA3-256 outputs; separate noncompact six-lane callers retain their own obligations.
All 32-byte external identity commitments need separate analysis; `iroha_crypto::Hash` fixes one marker bit, leaving 255 variable output
bits. Width, local algebra or an implementation cross-check is not an
independent security argument.

The [ZK first-release goals](zk_first_release_goals.md) coordinate wallet, Vega,
X509 and FASTPQ completion. Full replay or an unmasked proof cannot satisfy the
private succinct-proof goal, and increasing limits cannot substitute for the
required construction.
