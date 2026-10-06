# KAGEMUSHA A-split context framing v1

This record fixes the implementation framing of Lambda §2.8. It describes the
context and proof-ownership components in `iroha_kagemusha_proof::a_relation`.
Bootstrap now composes its genuine initial-state sigma, recursive proofs and
authenticated signed objects. This record does not establish qualification of
all operation variants, the final Omega artifacts, or native lineage acceptance.

`D_ctx = P_Fp(kgwctx_1, items)` uses the ordinary RP57 domain/arity framing.
`kgwctx_1` is distinct from the final lineage domain `kgwomg_1`.
`ContextPlan` fixes every presence flag, group length and ordering at keygen.
No stage accepts a witness-supplied list of obligations or object kinds.

The item order is:

1. Version `1`, operation code (one-based order of `ledger::Variant::ALL`),
   completed stage `1`, predecessor-verification stage (`0` absent, otherwise
   the one-based stage), total stage count, total Q count, object count.
   Each stage then contributes its Q count and ordered Q indices, then its
   operation-task count and ordered task codes. Every Q index occurs once.
   `ContextPlan::with_operation_tasks` requires the complete named task set
   exactly once: Bootstrap state/authentication (codes1/2), Load recovery/
   authentication/current-credential (3/4/8), or Send objects/pending/fee-and-carry/
   own-authorization/consuming-proof (5/6/7/9/10).
   `LoadStagePlan` and `SendStagePlan` execute the groups assigned to their fixed
   stage. Each authorization task must share its owning signature-Q stage.
   Send's consuming-proof task must share the hard predecessor stage. A generic frame
   component has empty task lists and cannot qualify an operation artifact.
2. For each Q in descriptor order: complete base-field VK digest, descriptor
   digest low/high128 limbs, instance-column count, and each column length.
   For each object: its nonzero unique category tag and fixed byte capacity.
   A complete Receive task plan then appends the result namespace `kgwrslt1`,
   count5 and the five ordered `(tag, zero-based owner stage)` pairs. Owner
   ordinals are checked u32 constants; this group is absent for other variants
   and generic frame-only components.
3. Own statement26, followed by incoming statement26 exactly when that sigma
   slot exists.
4. Predecessor core33/rest8/public18 and both transported Pallas/Vesta claims
   when present, then successor
   core33/rest8/public18. State opening hashes and their public-header binding
   are constrained on the shared verifier hash lane.
5. Incoming original public18, its constrained encoding/version/width validity,
   Pallas claim, Vesta claim, actual LE32 proof length and
   every original Omega message as low128/high127/top1, when incoming Omega
   exists. The message count is fixed by its admitted descriptor.
6. Every Q instance, column-major in descriptor order, each canonical scalar
   encoded as low128/high127. Every stage rebinds the entire original list and
   copy-binds its fixed partition to its own hard verifier outputs. A previous
   stage's verification cannot be silently reassigned to the current stage.
7. For each object: authenticated-object digest, actual LE32 length, tape digest.
   The tape digest is `P_Fp(kgwctap1, [tag, capacity, chunks...])` over the exact
   `LE32(actual_length) || fixed_buffer` carrier, split into consecutive31-byte
   little-endian integers, with the final short chunk. Length and message bytes
   come from one constrained byte tape. A length above capacity is representable
   because a malformed incoming object must be able to produce a soft failure;
   this context layer does not claim to authenticate an object. Hard fixed-size
   own objects can instead use `from_exact_run`: the same semantic parser tape
   is reblocked with a pinned LE32 length, without assigning another byte copy.
   Total original-byte consumers use `from_active`: `ActiveBytes` proves the
   exact u32 length and a zero inactive suffix. Its separate-domain commitment
   is `P_Fp(kgwcact1, [tag, capacity, length,
   P_bytes(kgwcact1, LE32(length) || active_original_bytes)])`. This cannot
   substitute the padded descriptor-sized verifier view for the original
   short or overlong string. The raw Omega/sigma consuming digest similarly
   concatenates their active length-prefixed strings without intervening padding.
8. Mode triples Accept/Trivial/Corrected in order: incoming Pallas, incoming
   Omega opening, incoming Vesta, incoming sigma, omitting absent groups.
   The continuation retains and copy-binds these original public fields,
   claims, proof messages, modes and corrections to its incoming verification
   and selection outputs. Native correction coordinates follow for the first two, then foreign S6
   x/y coordinates for incoming Vesta. The sigma correction belongs to the hard
   Q relation and is not an unused A context field.
9. A1's carried Pallas claim.

For a complete Receive task plan, exactly five boolean proposed result values
follow the object commitments and precede the modes. `ReceiveResultClaims`
checks their Boolean representation and owner schema but exposes no acceptance
verdict. Their typed owning stages must derive and bind all predicates; the
context commitment alone is not evidence that any predicate was executed.

A Pallas claim is `[source_k=16, Gx, Gy, u0_lo, u0_hi, ..., u15_lo, u15_hi]`.
A Vesta claim is `[Gx_lo,Gx_hi,Gy_lo,Gy_hi,u0,...,u15]`, with source k16 fixed.
Transport claims and correction coordinates are checked canonical cells; a
malformed incoming transport supplies its deterministic dummy and its decode
bit to the complete incoming-mode predicate. The context does not replace that
predicate or any operation, map, signature, selector or object-to-byte check.

The global four-bit sigma selector is the sorted wire `(tag,mask)` table:
Bootstrap0, Load1, Send masks0..7 at2..9, Receive masks0/1 at10/11,
ArchiveSent12, Unload13, the shared constrained RefreshPolicy union14, and
Retiring15. The five refresh A variants share tag7; separate unconstrained
refresh aliases do not fit this catalog. The circuit selector constrains the
same Send control-mask cells or Receive recorded-list predicate used by the
operation. Earlier Load selector5 diagnostics are not the canonical catalog.

`ContextPlan::with_schedule` fixes the ordered Q-index partition for each A
stage and the unique stage owning the hard predecessor P/opening pair. A1
requires at least one actual Pallas obligation. One opening is forwarded;
multiple claims require the fixed hard fold. A zero-Q first stage is permitted
only with the hard predecessor pair. It commits the exact Q frames and sigma
part before authentication of those Q frames; all deferred Q proofs remain
mandatory in their assigned later stages. An intermediate A/W proof alone
cannot accept a lineage.

For stage indices starting at one, let `P_i` be A_i's accumulated Pallas claim,
`V_i` be W_i's Vesta fold output, and `D_i` be A_i's context digest. `D_1` is the
complete original context above. An intermediate A_i, for `i > 1`, emits:

```
D_i = P_Fp(kgwctx_1, [1, variant, i, D_(i-1),
                    encode(P_(i-1)), encode(V_(i-1)), encode(P_i)])
```

The continuation supplies the fixed-length trace of prior `(P_j,V_j)` pairs,
recomputes `D_1` from the original inputs and `P_1`, then replays that recurrence.
It hard-verifies the exact preceding W key against the resulting digest and
current V accumulator. The trace is hash-bound data, never another validity
verdict or an extra opening obligation. Every prior P and V remains retained
by the current accumulators; the trace is not folded again.

Each internal A frame has the uniform69 layout with its `D_i`, current part and
explicit trivial absent slots. W1 folds the original sigma part plus the A1
opening and two fixed trivial slots. Later W_i folds `V_(i-1)` plus the A_i
opening and the two fillers. Thus the original sigma part and each A opening
occur exactly once. A_i for `i > 1` folds, in order, `P_(i-1)`, W_(i-1)'s opening,
the predecessor P/opening pair if assigned to this stage, the two mode-selected
incoming Pallas claims if terminal, and this stage's exact Q openings.

Only the last scheduled A stage returns opaque `FinalHalfCells`. It hashes the
final P claim with the successor public18 under `kgwomg_1`, and retains the last
W's V accumulator, actual predecessor V and selected incoming V in the final
frame. Intermediate `ContinuationCells` cannot call the final-output API;
terminal output cannot be reinterpreted as an internal continuation. The closed
result owns its claims and provenance, so the output API cannot substitute a
different P claim or incoming frame after the checked fold. Every original
incoming field, decode bit, proof, mode, correction and carried key is rebound
at terminal closure.

The corrected Load schedule is A1 (hard predecessor and D32 recovery), W1,
A2 (hard Q_sigma), W2, A3 (hard receipt/voucher/LoadAuthorization signature Q),
W3, A4 (hard current-credential/Enrollment-certificate signature Q). It retains
all three Q openings and all three W openings exactly once. The fixed operation
tasks require C4 on every step; authenticating a predecessor does not replace it.
Five original object tapes, all source keys/instances, state and statement enter
the same context. The corrected four-stage component passes actual native
proofs and decisions at k16: the canonical two-bus Q_sigma/four-bus A candidate
has maxima 55,611/58,682/57,646/58,164 rows and 8,480-byte A proofs. The run includes
hard-verifier forwarding, exact signature-Q context/index/shape bundles and
full public-transcript byte binding with mutation checks. This is component
evidence; final catalog, transport size, timing/RSS and artifact admission
remain unqualified.

Earlier three-stage Load chains omitted the current credential/certificate
re-verification. Their passing native proofs and four-bus occupancy are only
partial-relation diagnostics and cannot authorize complete Load. Send likewise
requires a separate own 2V/1F Q in addition to Q_sigma and its map/object tasks.
Its mandatory `SendProof` task binds `P_bytes(kgwprf_1, LE32||Ω||LE32||σ)`
to field9 of the exact receipt body retained by `D_ctx`. Ω includes the full
320-byte public transcript before `πΩ||accP||accV`. This task executes beside
the hard predecessor and links all original proof messages and both full-k16
claims. The separate signature-Q stage authenticates that same complete receipt
tape and enforces its operation/state/provider semantics. Neither task can be
omitted, and moving `SendProof` away from the predecessor stage is rejected.

The canonical mask0 component now proves the five-stage Send schedule with
genuine Bootstrap and Load predecessors. Under Q_sigma's two-bus profile and
A's four-bus profile, stage maxima are 50,098/56,018/57,868/61,864/60,051 rows;
all five source proofs are 8,480 bytes. The predecessor's two-terminal catalog
is rebuilt under its actual Omega digest and both source terminal keys and
descriptors remain equal. Every pending claim is independently decided.
This covers complete mask0 operation composition, not a final catalog: the
new Send terminal key still needs inclusion and root-digest rebuilding, all
other control masks remain, and the generic Omega transport is oversized.

Receive's task metadata defines eight mandatory groups, including five named
soft results (proofs, objects, signatures, nonmembership and recorded blacklist).
The private fixed five-owner plan rejects missing/doubled/relabelled groups;
its schema and five original Boolean claims now enter `D_ctx`. Typed owning
task producers and the terminal iff rule are still pending and must compose
before admitting a Receive key.

A uniform final Omega catalog must exclude intermediate A/W keys and rebind the
normal Omega digest throughout the chain. Full-catalog admission, proof size,
time/RSS and cryptographic qualification remain separate gates.

The separate Bootstrap signature workload is a second Q leaf: its two variable
keys and one fixed key are not silently included in `Q_sigma`. Unsplit Bootstrap
with more than one Q hard-folds exactly its ordered Q openings, with no
predecessor slots. A single-Q component fixture still forwards its opening.
All Q openings have k16, so this fold needs no full-length filler. The split
component places `Q_sigma` in A1 and the signature Q in A2, where it is retained
alongside contextP and W's own opening.

Key construction is acyclic: source Q keys → A1 keys → W1 key → A2 keys →
W2 key → … → terminal A keys. A future W key cannot be an earlier A constant.
`WKey` is constructed by W-specific key generation and retains its exact
wrapped-stage class; `SplitPlan` accepts it only for the immediately following
stage of the same context schema. Every continuation pins its complete W key.
No witness or runtime terminal flag selects the stage class. The normal Omega key digest remains
carried in the public18 fields and pinned by native artifact admission as in
Lambda §2.6. The artifact builder must supply actual A1 keys for the stated context schema:
`WCircuit::new` accepts an admitted binding and key-digest catalog, and does not
prove that a caller-generated arbitrary circuit implements A1. This is the same
reviewed-artifact trust boundary as the final A allowlist. There is no native
acceptance entry point for W; a final verifier
must use its admitted final Omega key and recompute `kgwomg_1`.

The final Omega catalog must exclude every intermediate A key and all W keys.
This remains a reviewed artifact-admission requirement: typed internal wrappers
are not substitutes for building the complete final catalog and rebinding the
normal carried Omega digest.

Remaining qualification includes actual proofs for every stage of every split variant,
all operation/consumer constraints, independent missing-obligation and splice
mutations, the final native key-type/admission boundary, resource measurements,
and the broader cryptographic audit obligations in the soundness record.

The Bootstrap test `actual_split_chain_retains_context_wrapper_and_final_claims`
generates an actual Bootstrap sigma and recursive Q_sigma proof, then actual
native A1, W and A2 proofs, and independently decides the retained claims.
`admin_sigma::BootstrapCircuit` proves the exact initial zero-value state,
empty maps/counters, disabled controls, and state/statement/public bindings at
k12 with five advice columns (1,453 maximum rows, 6,133 assigned cells and a
3,296-byte PIPA-R proof). Its fixed sponge and glue phases share four columns;
the range lane is separate. Rehashed invalid initial states are rejected.

A2 hard-verifies a genuine 2-variable/1-fixed signature Q over the exact
receipt, credential and root certificate messages. `BootstrapObjects` binds
those opaque outputs to the same canonical object tapes committed by both
halves, requires all credential/evidence/current-state and receipt predicates,
and pins scheme/provider/root policy. The receipt's sigma-only proof digest
comes from the same length-prefixed chunks bound to Q_sigma. The credential's
enrollment ID equals the Bootstrap statement's first effect pair. The second
pair is the nonzero generation-zero enrollment-marker identity: its SHA-256
preimage and durable one-time installation remain G2/OQ-1 responsibilities.

The test rejects altered object/context/key/proof-length/proof bytes, a
variable certificate-root policy, swapped receipt/credential signature slots,
an omitted authentication schema, and every dropped Pallas obligation. The
known and unknown witness layouts agree. Full incoming-variant proof chains,
operation partitioning for other variants, final Omega/native admission and
resource/cryptographic qualification remain open.

This genuine Bootstrap composition occupies at most 38,961 advice rows in
A1 and 62,158 in A2 with the previous two-stage schema header. A1's other lanes peak at 35,372 sponge, 27,339 ECC and
14,749 foreign-field rows; A2 peaks at 54,688 ECC, 49,543 sponge and 20,006
foreign-field rows. Both fit k16. These are source-layout inventories, not
time or RSS qualification. Predecessor/incoming verification and other
operations' map work still require an explicit fixed partition.
