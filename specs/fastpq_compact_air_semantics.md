# Compact one-delta AIR semantic audit

Read-only source review, 2026-09-06. Scope: the 65,536-row, 342-column, 923-slot compact ordinary/AXT transfer relation. No Rust edits, compilation, FFT, proof generation, or test execution were performed for this review. The exact reviewed source snapshot is retained in the local audit artifact. This is a source-linked semantic argument, not a machine-checked theorem or a production security qualification.

## Result and precise claim

No concrete missing bit, limb, BLAKE2b round, message-port, sibling, path-bit, or root-chain constraint was found. The standalone formal AIR establishes **two sequential depth-32 SMT updates between the supplied public digest ports**. The immutable public preparation additionally checks the complete transfer arithmetic, canonical keys, exact row multiplicity, asset normalization, and deterministic path allocation. Together they support the specified public one-delta transfer relation, conditional on the hash assumptions and the protocol reducing acceptance to satisfaction of this formal AIR.

They do not establish that the supplied balances are ledger balances, that a signer authorized the transfer, that a permission root contains an applicable grant, or that all committed AXT business assertions are true. These are explicit caller obligations and outstanding requirements for a production integration that removes replay. The distinction is enforced in the existing [low-level constructor contract](../crates/fastpq_prover/src/backend/compact_transfer_air.rs#L102) and [AXT wrapper contract](../crates/fastpq_prover/src/backend/compact_axt_air.rs#L1).

## Exact relation and encoding

The [column mapping](../crates/fastpq_prover/src/gadgets/compact_trace_columns.rs#L17) preserves all 310 hash cells and four eight-limb SMT ports: old child, new child, sibling, and starting root. Decoding checks exact width and canonical Goldilocks elements; it does not narrow arbitrary LDE values to `u32`. This is necessary: range restrictions belong to the trace equations, not to off-subgroup decoding.

| Global constraint slots | Source of equations | Semantic obligation |
| --- | --- | --- |
| 0–596 | 597 hash-local slots | Boolean data, exact message import, arithmetic, initialization, feed-forward, export; zero hash cells in padding |
| 597–679 | 83 hash-transition slots | Register/message/count/chaining continuity inside each hash |
| 680–922 | 243 SMT slots | Exact node message, all digest ports, public leaves, sibling continuity, path order, update chain, endpoints |

The [combiner](../crates/fastpq_prover/src/backend/compact_transfer_air.rs#L306) appends both hash arrays and all SMT slots. The [hash compiler](../crates/fastpq_prover/src/backend/compact_hash_quotient.rs#L539) invokes the reference residues with `active = 1` on every phase, explicitly constrains all 310 hash cells to zero on phases 408–511, and checks the exact maximum slot counts. There is no witness-controlled inactive selector. Fixed periodic masks select the physical phase, and public sparse masks select the supplied path and endpoints. The [source degree ledger](fastpq_compact_air_degree_ledger.md) establishes their polynomial degree bounds; this semantic audit also requires their stated exact subgroup evaluations.

## BLAKE2b semantics: satisfaction-to-computation argument

The relevant equations are [local residues](../crates/fastpq_prover/src/gadgets/compact_blake2b_air.rs#L277) and [transition residues](../crates/fastpq_prover/src/gadgets/compact_blake2b_air.rs#L415).

1. **Canonical input.** Every bit, carry bit, and presence flag is Boolean. Presence flags form one prefix across all six import phases; absent bytes have zero message bits. Packed 32-bit halves bind all sixteen message words, including partially occupied and unused words. Prefix counting and the length phase determine the exact message length; the SMT equations require 83 bytes. Thus an alternative prefix or nonzero block suffix cannot represent the same declared message.
2. **Word ranges follow by induction.** Message limbs are packed from bits and copied. Initial chaining words are fixed IV/parameter limbs. Working registers initialize from these fixed limbs and subsequently change only to packed sum/XOR limbs. Chaining updates likewise come from packed XOR bits. Consequently the registers used by arithmetic are actual `u32` halves, even though the wire and AIR field allow full Goldilocks values. No potentially aliasing 64-bit word is packed into a single field element.
3. **Exact modular addition.** The low/high half equations include the low carry in the high sum. Boolean carry pairs exclude `(1,1)`, allowing only 0, 1, or 2 where three operands are added; two-operand additions further constrain the carry. For these bounded limbs and carries, an equation's integer residual has magnitude far below the Goldilocks modulus. A zero field residual therefore gives the intended integer equality, not an additional solution obtained by wrapping modulo the field modulus.
4. **Correct compression.** Phases 7–390 implement 96 G invocations over twelve rounds, with the repeated ten-row sigma schedule and rotations 32, 24, 16, 63. Phases 391–406 perform all eight feed-forward words. Initialization pins the unkeyed 32-byte digest parameter, byte counter, and final-block complement. These constants and operations agree with the primary [BLAKE2 specification, RFC 7693 §§2–3](https://www.rfc-editor.org/rfc/rfc7693.html#section-3.1).
5. **Exact marked digest.** Phase 407 exports all eight 32-bit limbs. The final limb formula sets bit 248 of the ordinary BLAKE2b-256 output, matching Iroha's marked `Hash` convention. This is a 32-byte digest with one fixed marker bit, not a claim that all 256 output bits remain independently variable. Nonexport digest cells are zero. The SMT carries, rather than a hash transition across phase 407, bind this export to later node inputs.

The resulting induction determines the digest for every admitted node message. Hash collision/second-preimage resistance remains a cryptographic assumption; correct evaluation of the hash circuit does not prove that assumption.

## SMT semantics: exact update chain

[Fixed columns](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs#L91) select 128 hashes: two updates × 32 levels × old/new hashing. The [243 equations](../crates/fastpq_prover/src/backend/compact_smt_quotient.rs#L293) yield the following chain.

| Obligation | Enforced behavior |
| --- | --- |
| Domain and node bytes | The exact 19-byte node domain and both 32-byte children make an 83-byte message. Marker bits on both input digests are fixed. |
| All child limbs | Every one of the sixteen 32-bit message limbs equals its chosen old/new/sibling port; limbs crossing the 24-byte import boundary use the next row's bits. |
| Path/index | Each of the public path's 32 bits chooses the child side at that level for both old and new hashing. |
| Shared sibling | The sibling port is carried through both hashes and their padding. It is free to change only after the new hash's final padding row, so the old/new roots use the same sibling at each level. |
| Leaves | At rows 0 and 32,768, the current old/new children equal all eight limbs of the appropriate public leaves. |
| Hash outputs | An old/new export changes the corresponding next child to that hash's digest. Carry equations persist through padding and later imports. |
| Old root | At each update's last new-hash export and last padding row, the completed old child equals the starting root. |
| Debit-to-credit link | The sole update-reset row changes the next starting root to the debit's completed new child. The credit's public leaves are imported separately at its first row. |
| Endpoints and final edge | The first starting root equals the public old root. Both the final export digest and final padded new child equal the public new root. The final row disables the otherwise cyclic state carry. |

At an ordinary old-export edge, for example, `(next_old - old) + (old - digest) = 0` gives `next_old = digest`. The padding rows cannot detach that value. At reset the child registers may change to the next public leaves while the starting-root equation preserves the root connection. Sibling range/marker properties follow from its complete input packing at each level and its carry equations; separate range checks on every carried row are unnecessary.

This proves existence of sequential openings in a binary digest tree for the public paths. It does not prove that unopened subtrees encode a particular ledger state or that an externally chosen root is authoritative.

## Native public checks needed for the transfer claim

The [prepared-public constructor](../crates/fastpq_prover/src/gadgets/public_transfer_statement.rs#L365) operates on bounded public data and returns an object with private fields and immutable borrows. These are part of verification, not private witness-generation assumptions.

| Public fact | Check performed before AIR construction |
| --- | --- |
| Amount and balances | [Normalization and arithmetic](../crates/fastpq_prover/src/gadgets/public_transfer_statement.rs#L676) convert all five quantities exactly to nonnegative `u64`, reject sender underflow/receiver overflow, and check both balance equations. Self-transfers must chain the debit into the credit. |
| Decimal scale | One deterministic scale per asset is obtained from the whole claims table, with the first balance seed for each account and every amount included. [Numeric conversion](../crates/iroha_data_model/src/fastpq.rs#L197) rejects negative, inexact, and out-of-range values. This is not an asset-definition `NumericSpec` authorization check. |
| Account, asset, row multiplicity | Full canonical account/asset balance keys and exact eight-byte pre/post values match a FIFO of chronological delta occurrences and debit/credit legs. There are exactly two rows per delta, and no unmatched or reused occurrence. Repeated keys must chain. |
| Key/path binding | Distinct sorted complete keys are hashed with the key domain. The first four hash bytes provide only the initial path candidate; bounded deterministic first-free allocation resolves collisions over the complete key table. Full key hashes remain in the leaf computation. |
| Leaf/value binding | [Public leaves](../crates/fastpq_prover/src/gadgets/public_transfer_statement.rs#L733) commit to the complete key hash and domain-separated hash of the exact normalized eight-byte value. All digest limbs reach the public SMT ports. |
| Transcript/ordering binding | The one-delta Poseidon preimage digest is recomputed; multi-delta transcript digest presence follows its explicit policy. Exact canonically ordered transition bytes determine the ordering hash. |
| Complete expected context | [Context encoding](../crates/fastpq_prover/src/backend/compact_public_transfer.rs#L114) compares all seven PublicIO fields with independently supplied expectations and binds full rows, typed quantities/accounts/assets, transcript hashes, authority digests, and semantics before the protocol challenges. The [ordinary facade](../crates/fastpq_prover/src/backend/compact_public_api.rs#L77) requires the ordinary profile. |

Arithmetic is public and can be checked natively; it need not be duplicated as secret trace columns. Calling the lower-level SMT constructor alone, however, does not inherit these transfer checks. No caller may promote that lower-level proof to a transfer authorization.

## What AXT adds, and what it leaves external

The [AXT wrapper](../crates/fastpq_prover/src/backend/compact_axt_air.rs#L45) changes the authenticated identity/context while retaining the same SMT equations. [Public fact checks](../crates/fastpq_prover/src/axt_binding.rs#L1270) require canonical supported transfer claims, matching parameter/source dataspace, nonempty transfer rows, an entry hash matching the source transaction commitment, and that commitment on every transfer transcript. Opaque effects do not select this relation.

When remote-spend commitments are present, exact validated claim preimages must reproduce their canonical commitments. [Transfer matching](../crates/fastpq_prover/src/axt_binding.rs#L1149) then requires multiset equality of asset, from, to, and effective amount, including cardinality; it also checks source asset/dataspace and transfer operation. This binds the proof to the supplied handles' exact transfer facts. Handle authentication and replay protection remain external.

[Metadata checks](../crates/fastpq_prover/src/axt_binding.rs#L1320) parse canonical committed amount, expiry, manifest, and optional DA values and compare their exact outer mirrors. They do not establish current-time expiry, source finality, or an arithmetic relation between the committed scalar and the transfer amount.

With no remote-spend commitments, remote claim presence is forbidden and the validator returns after the header/transaction checks. General effect-binding accounts/assets/scalars, claim digest, witness commitment, policy commitment, receipt identifier, target dataspaces, and effect-type labels are bound assertions; they are not generally derived from the transfer computation. [Effect canonicalization](../crates/fastpq_prover/src/axt_binding.rs#L1510) only validates their canonical representation. The [data-model contract](../crates/iroha_data_model/src/nexus/axt.rs#L361) assigns business-effect comparison to maintained contracts and states that effect scalars are not ledger amounts. A valid proof of this relation alone must therefore not be described as proof of an arbitrary named business policy or compliance claim.

## Actionable acceptance prerequisites and remaining evidence

1. **Authenticate the statement's connection to ledger execution before removing replay.** [SMT construction](../crates/fastpq_prover/src/gadgets/transfer.rs#L394) builds a tree from touched transcript keys and seeds balances from those transcripts. [Core batch construction](../crates/iroha_core/src/fastpq/mod.rs#L746) replaces PublicIO roots with these touched-tree roots. Independently authenticating only a transaction identifier or accepting a prover-provided touched root is insufficient to establish actual balances. The production interface needs an authenticated execution commitment or authenticated state openings that bind the complete public accounts, initial balances, normalization/asset policy, and expected roots. Add caller-level rejection tests for an internally consistent but unauthenticated balance/root/scale statement and for mismatched execution authority. This is an integration prerequisite, not a newly demonstrated AIR bypass.
2. **Give each AXT consumer a precise verified predicate.** Treat committed metadata as committed metadata unless a consumer explicitly validates its meaning. Test a newly generated valid proof with self-consistent but irrelevant effect accounts/scalars or policy digest at the consumer boundary; reusing an old proof after changing context tests binding only. Preserve exact remote-spend matching, signatures/replay, expiry-at-use, business amount conversion, finality, and post-proof amount-commitment checks. The generic AXT binding field names must not substitute for those predicates.
3. **Complete the semantic proof/evidence boundary.** The source induction above provides no observed algebraic counterexample, but is not machine-checked. Formalize or independently audit the Boolean/range induction, exact carry bounds, all 128 hash-to-port transitions, reset/final edges, native preparation invariants, and their composition with the fixed-mask degree ledger. Mutation tests show sensitivity at selected valid traces; they do not exclude every coordinated malformed satisfying trace.
4. **Qualify the full cryptographic protocol separately.** The interactive AIR/proximity argument assumes correct semantics, ideal binding and challenge/query conditions. Concrete hash security must cover marked 32-byte state/key/value/node hashes as well as the separate commitment digest, relevant collision/second-preimage and multiple-target attack classes, Fiat–Shamir, and the intended post-quantum model. Neither this semantic audit nor the 923-slot count supplies that security estimate.

Existing source tests provide useful coverage: [hash boundaries/carries/import/export](../crates/fastpq_prover/src/gadgets/compact_blake2b_air.rs#L527), [padding and detached-root attempts](../crates/fastpq_prover/src/gadgets/compact_smt_air.rs#L840), [every public path bit and full ports](../crates/fastpq_prover/src/gadgets/compact_smt_air.rs#L1125), [compiled phase/reference parity](../crates/fastpq_prover/src/backend/compact_hash_quotient.rs#L797), and [public normalization/multiplicity fixtures](../crates/fastpq_prover/src/gadgets/public_transfer_statement.rs#L1121). Their presence was inspected; execution results are recorded in [production readiness](fastpq_production_readiness.md). No “fully secure,” production-ready, optimal, or zero-knowledge conclusion follows from this review.

