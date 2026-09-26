# ZK-AMS native RNS proof wire V1

The source owners are `iroha_zkp_halo2::vega::zk_ams::mkhe::rns_native_*`.
This specifies the sole native40 representation. The ordered 40-entry
modulus/root tables are canonical; the still-unqualified 38-limb release profile
derives its prefix from those tables. Each root has exact order 262,144 for the
degree-131,072 ring; duplicated parameter tables are not an alternative
authority. The [governed profile boundary](../../docs/history/2026-09-24/mkhe-native40-governed-profile-source-boundary.md)
checks a source's complete profile claim before publication. It does not
establish live coefficient correspondence or declare the unfinished
source/prover, composite admission or resource qualification ready.

## Commitments, fields and identity

Native proof roots, authentication nodes, staged transcript states/seeds and
qPCS bindings use `RnsNativeProofDigestV1`: exactly 48 little-endian bytes, with
all six words strictly below the Goldilocks modulus. The shared owner is
`fastpq_isi`; public exports preserve that exact type. No 32-byte proof root,
padding, truncation or alternate decoder is accepted.

Public profile/source/storage identities and complete transport hashes retain
their existing 32-byte roles. T256 curve generators, scalar transcripts and
Bulletproof identities remain separately owned 32-byte values. A mixed registry
compares tagged public32/native48 identities. Each bridge to a native root
binds complete current context only after its underlying curve verifier returns.
Whole-section transport identities cannot replace proof verification.

Each Fq2 coordinate is below its selected unchanged RNS modulus, and every
modulus is below 2^60. The pair occupies exactly 15 big-endian bytes encoding
`(c0 << 60) | c1`; both modulus checks are mandatory. The 16-byte layout is
removed. Public scalar evaluation grids keep their canonical 8-byte residues.

The Exact12 commitment has one owner in `iroha_crypto::privacy`. Canonical
parameter reconstruction binds that identity, the actual six-lane parameters,
all role/length frames, field packing, sampler rules and current wire geometry.
It does not recursively hash its own output.

## Transcript and Merkle geometry

The shared frame includes catalog, protocol, profile, role, phase, level, index,
counter, lane and length-framed fields. Payload commitments bind all 6,000
canonical bytes, exact geometry and oracle role. Indexed leaves bind all 48
payload-digest bytes and the exact tree context/index. Ordered nodes use the
same owner. One immutable public-payload cache may reuse only the payload
commitment after complete byte equality; index binding is always recomputed.

A sampled Goldilocks word is accepted below `pG - pG % q` and reduced modulo q.
Coordinates, components and attempts are verifier-owned and separately framed.
Joint Fq2 retries reject either tail or a zero pair, with an exact 256-attempt
bound. The T256 scalar sampler remains part of its separate curve protocol.

The full LDE domain is 2^19; queries use its 2^18 half-domain. The actual pair
mapping and canonical multiproof frontier are fixed by the native owners.
Four authenticated final-layer leaves retain the sole native40 terminal rule.
After qPCS, only cross-field and global roots remain. The final global discharge
checks the actual verified root and retained lineage, the final global-root
field, and independently replayed composite seed and final state. There are 27
seeds; the composite uses ordinal 26 / purpose 11. Detached padding roots, seeds,
obligations and tokens are removed. Full source decoding still checks all 65,536
slots and the zero tails of X (89), rE (1,024), and rW (512) before any used-slot callback.

## Exact sizes and unchanged caps

| Owner | Fixed bytes |
|---|---:|
| Initial / prefix / complete-FRI headers |250 /668 /496|
| Common typed section |186|
| RNS/qPCS / cross-global section framing |12369 /4075|
| ZRLS source / ZSTL source-terminal anchors |898 /610|
| Direct frame header / owned frame |323 /32303|
| Three-section envelope header |357|

The envelope accepts exactly ordered kinds 1 terminal, 2 RNS/qPCS and 3
cross/global. Retired kind/count 4 and ZAZP/ZZPC payloads are rejected. ZSTL has
an 18-byte header and 16 fixed identities: native48 at positions 0, 6, 7, 12, 13,
public32 elsewhere. Its two unowned fixed marker bytes are removed; the old
form is rejected rather than reinterpreted.

Initial+quotient bound 4,165,632 fits 4,313,088; correlated FRI bound 25,129,440 fits
26,409,984. Together with 16,983 fixed/evaluation bytes, 29,312,055 fits the unchanged
qPCS cap 30,740,352 and leaves 1,428,297 for its mandatory source continuation.
ZRLS leaves 1,427,399; ZSTL leaves 1,426,789. These are upper-bound inequalities,
not evidence that an unfinished complete source proof fits the residual.

The unchanged 8 MiB cross/global limit leaves 6,778,981 after current section and
inventory bytes. Direct framing leaves 6,746,678; the retained curve chain leaves
107,168 after membership and 107,043 after source same-opening. The three section
maxima plus 357 occupy 41,226,469 under the unchanged 41,943,040 whole-envelope cap.
Resident 512 MiB, spool 16 GiB and I/O 64 GiB limits are unchanged.

The tracked 128-billion work limit is unchanged. The private tree constructor
conservatively rejects its local dense-MDS field add/mul charge before
allocation/source access. Its whole-proof lifetime, other-stage accounting
and production caller are unfinished. Source-derived full-tree costs are
implementation diagnostics, not native timings or an approved whole-proof
policy. No cap raise, meter reset or operation-unit redefinition is allowed to
stand in for completing resource ownership and qualification.

The source-packing post-equation binding absorbs two canonical native48 values (source anchor and final aggregation schedule) and seventeen ordered public/curve32 components; its combined curve binding stays 32 bytes. The typed registry keeps those roles distinct. Retained source readers use the canonical `iroha_crypto::confidential_spool` API. The global plane uses one ordered storage owner split at plane 7,075, with 2,213 planes in the second file; both satisfy the existing per-file cap. Parent records bind the exact canonical plan. The local storage tests include real small encrypted-file pairs; they must be rerun against this exact inventory and its regenerated plan/context vectors. The complete authenticated source-to-storage handoff, commitment equations and replay of actual plane slots remain unfinished; the materializer and proof authority remain uninhabited. Small-file execution does not qualify full-size resources or complete proofs.

The native40 global lookup rendezvous (`ZGZ1`) has a 56-byte header, then exactly
two nonidentity compressed T256 points: multiplicity under the 32,768-coordinate
basis, followed by the dedicated 87-scalar inverse-product mask under the
16,384-coordinate basis. Its 13 pre-`z` roles contain 39,634 physical points;
11,696 existing inverse points follow the two-point prefix, with 20,712 added
inverse points still borrowed from the authenticated inventory. The former
three-point mask frame is rejected by exact geometry and length checks.

Removing the unused point leaves the parent limit at 499,343 bytes and derives
113,221 bytes after this view, 108,852 after compact inverse, 107,201 after direct
membership, and 107,076 after source-packing. These are residuals of the same
parent cap. Multiplicity and inverse-product masking remain separate and
mandatory. No point, field, parser branch or proof capability for the retired
702-scalar mask remains.

## Persistent source-opening inventory

`global_lookup_statement_v1` owns one role descriptor shared by the native
rendezvous and source session: 344 separately bound source commitments, 39,634
pre-`z` points, and 32,408 shared post-`z` inverse points, totalling 72,386
persistent openings. This count excludes ephemeral proof points and randomness.
The source/session context hashes this exact native40 descriptor; removed
38-limb, dual-`z`, and residual-vector identities are not alternate inputs.
The native role tags, sole-`z` chronology, multiplicity, and inverse-product mask
remain unchanged. No second commitment or randomness is assigned to the shared
D/S inverses.

The retained comparator/sign snapshot has exactly 9,288 planes in order
`bD[344], bS[344], beta[6192], m[344], x[1032], n[1032]`. Native direct product
statements 3, 5 and 8 require these inputs and have no committed `q3/q5/q8`
residual planes. Each plane has 32 value slots and one blinding/point tail slot.
The ordered two-file plan binds 306,504 slots and 5,026,665,600 encrypted bytes,
with the existing per-file and whole-proof limits unchanged.

The source and D/S inventory ordinals remain 0..12,040. D/S production order is
group-major (17 D-low then 17 S-low); inventory storage is purpose-major. The
consuming difference-digit driver now fills all 5,848 delta inventory positions
12,728..18,576 after the 688 `bD`/`bS` commitments. It rereads the original
canonical source in order, checks all three sealed comparator lanes against
that source, and derives the 17 base-2^15 subtraction digits per group. Each
point is the actual prepared-value MSM with rho sampled and retained by the
same original session before value emission; external points and randomness
cannot enter this transition. Integer underflow or an out-of-range delta
rejects before a field residue can conceal a malformed borrow.

The original source returns only after every delta plane emits its 32 canonical
value chunks and the complete authenticated source-read schedule matches the
materialization record. Only this exact completed delta owner admits the next
sealed-value comparator phase: 6,192 beta and 344 m commitments occupy physical
inventory 18,576..25,112 and retain another 209,152 bytes of original-session
blindings. Incomplete delta, wrong/repeated logical ordinals and occupied slots
reject before commitment work. The exact completed continuation now admits
1,032 signed source commitments and then 1,032 negative-magnitude commitments
at physical inventory 25,112..27,176. These read the original authenticated
compact signed slots in natural `1024*local_block+i` order, enforce the existing
role bounds, and retain another 66,048 bytes of original-session blindings.
The positive point and mask are derived by addition; a derived identity rejects
the owner without replacing randomness. The original-S stream now owns the
canonical uniform coefficients, same original entropy source, authenticated S
file and 6,400 original digit masks at inventory 27,176..33,576. Its named
complement consumer reads that same file once and computes the integer
`Sbar=q_l-1-S` before extracting four base-2^15 digits. It appends 6,400 actual
`sum_i G_i*dbar_i + H*rho` tickets at 33,576..39,976 using the same table, RNG
and inventory. The final S coefficient is zero; its complement is `q_l-1`.
Cursor 39,976 is `Multiplicity`, ordinal zero. No digitwise complement,
replacement randomness, caller point, parallel replay facade or new inventory
is admitted.

The complement phase reserves all 204,800 bytes of retained scalar payload
and exact owner metadata before allocation, then all four evaluator lifetimes
before each block. The same storage owner charges 131,200 read bytes before
any of that block's eight physical reads. Only pre-I/O capacity refusal returns
the unchanged original owner; authentication, coordinate, entropy, arithmetic
or post-read allocation failure consumes the phase. The terminal owner retains
both original mask sets and the immutable S file for later separately funded
P~/H~/qPCS consumers. No generic reset or raw-secret extraction exists.

These producers do not establish complete authenticated native40 source,
packing/governed parameters, same-opening relations or whole-proof admission.
The existing 38-limb source cannot be relabelled native40. Whole 6,400-ticket
complement execution, combined resource/work qualification and the numeric
handoff's production seal remain open. Bounded first/final commitment fixtures
with synthetic prior tickets do not discharge those gates.

Each comparator/signed commitment now also mints its opaque canonical tail from
the just-admitted original inventory ticket and retained rho. The same prepared
source sequence emits 32 ordered value slots followed by exactly one 16,384-byte
`rho_BE32 || point33 || zero-padding16319` tail. Source handoff requires all 33
emissions; early/repeated tails, wrong value order and missing tails consume the
owner. The value vector is erased before tail-chunk allocation. This adds one
32-byte scalar guard and the public point/ordinal while retaining the existing
inventory masks, with no extra sampled blinding or inventory slot. These named
payload counts do not qualify RSS or whole-proof resources. The consuming
ordered two-file writer and authenticated complete-pair reopen remain TODOs;
emitting a chunk does not prove it was durably stored.
The retained snapshot plan is unchanged: delta planes are not added to the
9,288 comparator/sign planes. The driver introduces one further full canonical
source reread and one read of each compact comparator slot; these must be
included in eventual whole-proof lifecycle accounting. No existing resource
cap or qualification flag changes. See the source-coupled
[difference preparation contract](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_radix_range_v2/prepared_difference_digit_plane_v1.md).
The two additional signed-source compact passes contribute 33,849,600
authenticated read bytes to that same pending accounting; see the
[signed preparation contract](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_radix_range_v2/prepared_small_signed_plane_v1.md).

The actual ordered storage owner now requires one caller-retained spool/I/O
issuer. Before either leaf is created, it reserves the complete 5,026,665,600-byte
pair and 10,053,331,200 bytes for its writes and full seal authentication pass.
Every later authenticated record read is charged against the same retained
ledger, including repeated reads. Local capacity refusal preserves the original
snapshot without I/O or invalid-proof classification; another writer dropping
unspent reservations can make the retry admissible. Failed/partial attempted
I/O is never refunded. Cancellation closes both already-unlinked files before
releasing live-file credits. The 16 GiB spool and 64 GiB I/O ceilings are unchanged.
This integration covers actual pair lifetimes, not all production source/qPCS
consumers, resident memory, allocator/control overhead or full-size qualification.
Those consumers still must share the eventual proof-session issuer; separate
issuers do not enforce a whole-proof cap. See the
[ordered storage resource contract](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/ordered_snapshot_v1/resource_budget_v1.md).

The unused 344-weight canonical-reopen placeholder is removed from the sole V1
source path and local plane context. It had no consuming proof equation or
transcript-derived production weights. The remaining local context binds the
actual source-replay, source-opening and radix materialization records; local
storage identities add no public transcript inputs. All original 344 source
commitments, blindings, inventory ownership and actual authenticated source
replay requirements remain. The existing 1,376-owner source/packing same-opening
relation and its transcript are unchanged. The completed original signed session
now privately prepares its 344 radix-derived D masks and 1,032 signed masks,
retaining the same inventory and its exact verifier point root without new
entropy or transcript frames. This bounded preparation adds 44,032 named secret
payload bytes; it does not inhabit the final authenticated derived-mask provider.
The native40 source/context join, actual source-to-storage writer/replay and
production source qualification remain required: the upstream 38-limb lineage
is not a 40-limb proof. See the
[derived-opening contract](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective/incremental_source_phase23_source_algebra/global_lookup_source_replay_v1/source_openings_v1/commitment_session_v1/existing_radix_candidate_v1/prepared_source_packing_openings_v1.md), the
[source dependency audit](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/canonical_reopen_reconciliation_audit_v1.md)
and [storage ownership proposal](../../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/storage_context_schedule_proposal_v1.md).

This bounded implementation has arithmetic and session-transition tests, not a
complete authenticated-source proof or executed full delta/signed MSM inventories.
Reconciled descriptors and storage permits do not mint source authority or
complete source-to-storage handoff. Production scratch-sink/proof authority,
complete opening checks, and resource/hardware qualification retain their
explicit unfinished gates.
