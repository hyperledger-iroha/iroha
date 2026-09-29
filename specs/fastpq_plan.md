# FASTPQ implementation and completion plan

Updated: 2026-09-28. The first release has one canonical masked quantity-artifact
path for ordinary Core transfers and AXT envelopes. The
[DEEP protocol contract](fastpq_deep_protocol_contract.md) owns the exact relation,
geometry, transcript, masking and wire rules. The
[production readiness record](fastpq_production_readiness.md) owns scoped evidence
and outstanding qualification. Neither implementation presence nor a local proof
constitutes independent cryptographic or deployment qualification.

## Implemented release boundary

- Core's ordinary lane and standalone callers use
  `fastpq_prover::offline_compact` to prove and verify complete ordered quantity
  statements. The producer self-verifies before returning. Verifiers derive
  `ExpectedStatement::from_statement` from their own authenticated statement;
  artifact contents cannot choose the verifier's expectations or semantic role.
- The public statement binds dataspace, slot, touched-balance roots, permission
  context, transaction-set commitment, ordering and the complete canonical
  transfer statement. Each segment binds its ordinal and ordered root chain.
  These roots cover the touched balance tree, not complete world state.
- Balance keys use the canonical `FastpqBalanceKeyV1` Norito frame containing the
  typed asset definition and domainless account controller. The sole producer is
  `iroha_data_model::fastpq::transfer_balance_key`; display aliases and alternative
  layouts are not statement encodings. Public normalization and transfer matching
  precede the fixed private SMT relation.
- The sealed relation proves the complete 32-level SMT updates. Its fixed child
  has 65,536 trace rows, 342 total columns, 923 AIR slots and 8,388,608 evaluation
  rows. It commits 301 private columns and independently reconstructs 41 public
  columns. Sixty-four distinct queries and the fixed higher-arity FRI schedule
  provide bounded verification without full private-witness replay.
- Trace masks, quotient masks and an independent composition mask use fresh
  cryptographic entropy. Exact quotient division rejects nonzero remainders.
  Private source, coefficients, replay buffers and partial owners clear on
  success, failure and unwind. These controls do not establish complete
  zero-knowledge, side-channel or Fiat–Shamir security by themselves.
- Quantity admission covers witnessed transfers. Mint, Burn, RoleGrant,
  RoleRevoke and MetaSet require their own complete relations. An operation's
  presence in the sole V1 wire catalog does not admit its state transition.
  Opaque metadata carriers are not transfer proofs and cannot acquire quantity
  authority through labels or generic verification.
- AXT additionally checks the exact outer binding, metadata, mirrors and remote
  preimages. Cryptographic artifact consistency does not authenticate source
  finality, authorize a remote spend or commit business effects. Production
  CoreHost rejects non-null standalone `AXT_VERIFY_DS_PROOF` before recording a
  proof or mutating its cache. Finalized lane-relay and authoritative fee-vault
  paths retain their own state anchors and authorization checks.

## Developer and resource contract

Callers choose an ordinary or AXT statement and explicit resource limits, not
circuits, verifier keys, query counts or transcript internals. The public Rustdoc
contains an executable typed example. The public guide is maintained in
[iroha-docs](https://docs.iroha.tech/blockchain/fastpq).

`quantity_artifact_resources` provides witness-free planning. The real producer
independently admits its exact statement-dependent plan before private work.
Default limits remain 524,288 bytes per child, 2 GiB of charged payload per
segment and 2^42 structural work units. Source conversion, private SMT owners,
complete bundle framing and cumulative decoding have additional limits. Payload
accounting is not a measurement of process RSS or latency.

One producer is admitted per process and bundle segments are built sequentially.
Applications run synchronous proving on a worker and handle `ProvingError::Busy`
with a bounded queue or retry policy. `ProvingLimits.digest_execution` defaults
to CPU. Explicit required-device execution checks readiness before private work
and does not silently fall back. Metal accelerates bulk leaves and lower Merkle
parents; upper ordered work, transcript and trace arithmetic retain their CPU
owners. Unknown device completion quarantines retained buffers and blocks new
proof admission for the rest of that process; recovery requires a fresh process.

Same-attempt internal-node caches are bound to immutable context and original
roots. Selected leaf stripes and siblings are regenerated after queries are
chosen; each opening reconstructs the committed root. The resource plan includes
retained nodes, coverage, pending owners and opening scratch together. The cache
does not change entropy order, committed polynomials, transcript or proof bytes.

## Outstanding outcomes

The first-release API cut is implemented: direct canonical AXT batch proving is
public, and transparent replay is restricted to tests and `dev-tools`. Callers
are migrated. The normal external API passes 15 controls, Rustdoc passes five
compile-fail guards and one public example, developer integration passes 28,
the exact raw-transcript control passes, and AXT binding passes 53. Separate
artifact-dependent ignored tests remain open. Both normal and developer builds
are warning-free; optimized artifact and integrated admission checks continue
below. Exact source and failure history are retained in the
[September 28 validation record](../docs/history/2026-09-28/fastpq-masked-native-validation.md).

| Owner | Outcome | Completion evidence |
| --- | --- | --- |
| Producer/verifier | Complete maximum application and multi-child qualification under unchanged aggregate budgets. | Actual ordinary/AXT artifacts, source/statement/witness negatives, whole-process time/RSS and retained independent verifier replay at every supported boundary. |
| Hardware | Preserve complete CPU/Metal parity and qualify optimized public artifacts and supported device paths. | Same-fixture whole-proof byte parity passes. Obtain optimized complete facade timings/resources and device failure/cleanup evidence; CUDA hardware qualification remains separate. |
| Cryptographic reviewers | Qualify the full masked AIR/DEEP/FRI and six-lane commitment construction. | Independent artifact-bound soundness, hiding, Fiat–Shamir/qROM, digest/multi-target and arithmetic/side-channel reviews. A local rank or arithmetic calculation is insufficient. |
| Core/Nexus | Qualify authoritative admission, relay and AXT effects. | Exact finalized source anchors, intent/replay/custody checks and four-validator fault/restart evidence on one fixed candidate. Artifact consistency alone grants no business authority. |
| Release owners | Bind all results to the release candidate. | Exact source/lockfile/toolchain, native artifacts, SDK/CLI consumers, signed provenance and scoped validation. Preserve failed and unexecuted checks separately. |

The [ZK completion goals](zk_first_release_goals.md) coordinate these outcomes
with wallet, Vega and X509 remediation. Unsupported semantics remain rejected;
resource ceilings and statement coverage are not reduced to obtain a pass.

## Current proof evidence

The September 28 retained fixed-SMT child is 482,978 bytes and independently
verifies with cap, context, tamper and changed-statement rejection controls.
Complete one-child ordinary and AXT facade artifacts are 485,600 and 484,750
bytes, respectively, and also pass retained verification. The cached seeded
Metal child matches its pre-cache bytes. The same immutable executable's CPU
child also matches those exact proof bytes and passes separate artifact replay.
These runs use an unoptimized FASTPQ caller with optimized ISI arithmetic;
optimized public-facade performance remains to be measured.

The [dated validation record](../docs/history/2026-09-28/fastpq-masked-native-validation.md)
contains exact binaries, source captures, measured timings, memory observations
and earlier failures. Contended measurements are not controlled speed ratios or
release throughput qualification. The superseded work breakdown is preserved
verbatim as [historical source](../docs/history/2026-09-28/fastpq-plan-before-deep-reconciliation.md);
its replay and earlier-geometry claims do not describe the current artifact path.
