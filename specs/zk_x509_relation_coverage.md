# zk-X509 relation coverage

Source contract for the coverage part of task X.3 of the
[ZK delivery plan](zk_delivery_plan.md). It records which predicates of the
native reference relation
(`privacy_engines::zk_x509::relation::validate_reference_relation_v1`) the
verifier forces on a prover who commits arbitrary trace values, and which it
does not. It makes no soundness, zero-knowledge or performance claim, and it
does not replace the [joint relation contract](zk_x509_joint_private_relation.md).

First read on 2026-11-10 at `11d30fb89d`. Second read on 2026-11-11 at
`979baf6cda`, the revision that closes gaps 1 and 2: it covered the strict-DER
segment and the DER-to-RFC source tables, SHA-256, P-256 arithmetic and the
ECDSA layer, the calendar and temporal relations and the lookup arguments,
and it confirmed gaps 3 and 4. A predicate counts as forced only by a residue
the verifier evaluates, a verifier-fixed column or public input, or a
cross-segment join with both endpoints constrained. Checks in witness or
trace-building code do not count. Gaps 1 to 4 and 7 to 9 below were checked
against the constraint code and the verifier's term list at the revision of
their read; gap 5 is one reader's finding on the grammar side. The covered
rows are per-cluster readings with spot checks; the second read's sections
were each read once and their gaps and load-bearing claims re-checked. On
that revision the 1,506 non-ignored `zk_x509` library tests pass in release.
The closures of gaps 3 and 7 are backed by forged traces, of the complete RFC
5280 trace and of the SHA capacity AIR, and the closure of gap 4 by every
zero-flag choice for non-canonical signature encodings and by a malleated sink
on the actual traces; nothing has a forged-proof control yet.

## What the verifier adds across segments

`verify_main_deep_constraints_with_ca_v1` is the complete list: the local
residues of each registered segment, then

- 192 terminal links: 8 DER to RFC, 80 P-256 bus, 104 P-256 chain;
- the key plan: 17 key blocks including the four signature-scalar blocks and
  the Projection block, 5 SHA-to-P-256 digest blocks, 2 authoritative CRL
  digest blocks, 2 authoritative CRL time blocks;
- 24 SHA-union links: 16 SHA endpoint, 4 RFC consumer roles, 4 Projection to SHA;
- 108 CA links.

The RFC adapter closes each output role internally, producer product equal to
consumer product. Only an external term carries a role to another segment:

| RFC output role | External term |
|---|---|
| `Projection` | key plan Projection block |
| `CertificateTbsSha`, `CrlTbsP256Message`, `CrlCommitment`, `IssuerSpkiSha` | SHA union |
| `P256Signature` | none |
| `P256PublicKey` | key blocks |
| `GovernedTrustAnchor` | four CA links |
| `CertificateSlotActive` | key block 16 and its I/O block |

`P256Signature` still closes inside the adapter. The signature scalars reach
P-256 another way: the `SignatureScalar` family parses each document's
`signatureValue` from the DER tables, and key blocks 11 to 14 join its output
rows to the `r || s` rows of sinks 0 to 3 (gap 4).

The ByteMemory I/O segment is joined to other segments only by the six key
and selector blocks. Its channel declarations are not evidence of coverage
for any other channel; the Projection block joins the RFC rows to the
Projection segment directly.

## Gaps closed in source

These are closed by relation changes recorded in the
[joint relation contract](zk_x509_joint_private_relation.md). Gaps 1 and 2
were found by the first read, and their controls evaluate plans and residues
directly. Gap 7 was found by the second read; its control builds whole forged
traces of the SHA capacity AIR. None has a forged-proof control yet.

1. **Projection inputs were not joined to the parser.** The leaf SPKI, issuer
   SPKI, leaf serial and disclosed attribute bytes that the Projection segment
   frames and hashes were private input rows constrained only inside that
   segment; the only equality with the RFC outputs was the host check in
   `main_assembly::build_io_material_v1`. A holder of one valid credential
   could publish a nullifier, scoped key digest and attribute digests computed
   from bytes of their choice. The key plan's Projection block now equates
   every RFC `Projection`-role producer byte, raw length bytes included, with
   the Projection input row at the same position. Controls:
   `projection_join_rows_are_the_whole_leading_projection_role_run`,
   `projection_block_pairs_each_rfc_byte_with_the_same_projection_input_row`
   and the power-16 cases of the byte-block quotient tests.
2. **CRL serial comparisons were not forced to cover every entry.** The
   comparison slots were a private active prefix with no tie to the entry
   count, so a prover holding a revoked leaf could leave them inactive; an
   empty CRL produced the same trace shape. The entry census now weighs every
   comparison slot and equals twice the entry count on the last row. Controls:
   `a_revoked_leaf_cannot_deactivate_or_truncate_its_serial_comparisons`,
   `serial_census_balances_only_for_the_complete_comparison_prefix` and
   `entry_census_weights_live_only_on_entry_relations_and_serial_comparisons`.

Closed on 2026-11-11 with the owner's approval. Gap 7 is committed as
`c3191d4c32`, and gap 3, with gap 9, as `f6c5c4cf1b`. Gap 4 was closed on
2026-11-12, also with the owner's approval.

- **Gap 7. SHA-256 padding was not forced minimal.** The SHA capacity AIR
  (`evaluate_sha_word_capacity_residues_with_context_v1` in
  `sha_word_stark.rs`) set `final_block` by activity alone, checked the
  marker, the zero fill and the two length words in the final block only,
  and tied the number of active blocks to nothing: after the marker, words 14
  and 15 of a non-final block were merely zero. A prover could activate extra
  all-zero blocks up to the call's capacity, and the digest rows then read
  the compression of `m ‖ 0x80 ‖ 0^(k+64j) ‖ 8L`, not SHA-256(m). Where the
  digest meets a P-256 signature, a governed digest or a CA hash (calls 0 to
  4, 12 and 16 to 28) that needs a compression collision. Where it is itself
  a public output it did not: the Projection calls have a capacity of 33
  blocks against frames of five or six, the certificate nullifier is
  deterministic in the reference relation (`derive_certificate_nullifier_v1`),
  Projection pins call 6's digest to `statement.certificate_nullifier`, and
  consensus marks only the published value consumed (`x509_submission.rs`).
  One certificate could therefore publish up to 29 different nullifiers and
  be presented as fresh up to 29 times; the scoped subject-key digest (call 5)
  had the same freedom. A capacity residue now forbids the marker before word
  14 of any active block that is not final, which is exactly minimal padding.
  The compiled-profile pin is `dd803a10`; `cbf1bd12` is a rejection fixture.
  Controls: `non_minimal_sha_padding_is_rejected_only_by_the_minimal_padding_residue`,
  which builds whole non-minimal capacity traces under the canonical fixed
  schedule and finds every other residue zero and the new one nonzero at
  word 14 of each non-final block after the marker, one per extra block, and
  `compiled_profile_rejects_non_minimal_sha_padding`.
- **Gap 3. `FixedByte` addresses were not anchored.** A run constrained
  `BASE_ADDRESS` only by its increment, while the topology query tied its
  start and exact length to a required node and the byte lookup was keyed by
  document, address and value. The compared bytes could therefore be any
  consecutive occurrence of the pattern in the same document, so profile
  checks on the top-level certificates (algorithm identifiers, version,
  extension and name OIDs, the `critical` byte, the key prefix) could read
  their bytes elsewhere. Exploiting it needed a nonconforming certificate
  signed by a governed CA. One RFC residue now anchors every row,
  `ADDRESS = START + OFFSET`. The compiled-profile pin is `a7d5eaa2`;
  `dd803a10` is a rejection fixture. Controls:
  `a_fixed_byte_run_read_from_another_occurrence_fails_only_the_address_anchor`,
  which evaluates every local residue on every row of the complete canonical
  trace, finds them all zero, then moves the run that checks the leaf's TBS
  signature AlgorithmIdentifier onto the identical outer signatureAlgorithm
  bytes, rebalances the DER byte table, and finds only the new residue
  nonzero, on each row of that run; and
  `compiled_profile_rejects_unanchored_fixed_byte_addresses`.
- **Gap 9. The BIT STRING unused-bit count was not bound to its octet.**
  Closed by masking, with no DER change: the closed grammar admits BIT
  STRINGs only for the two signature values, the public key and the key
  usage root, and the profile patterns that pin their unused-bits octet to
  zero and the whole key usage TLV are now anchored. The DER AIR still
  carries the column without reading the octet, which would matter only if
  the grammar admitted another BIT STRING.
- **Gap 4. Signature scalars were not joined to the documents.** Each P-256
  binding sink takes `(Qx, Qy, r, s, digest)` in its real-input column, and
  nothing joined rows 41,024 to 41,087, which feed the ECDSA writers, so the
  proof showed only that some valid signature on the bound TBS digest exists
  under the bound issuer key, not the document's, and not that the
  document's `signatureValue` is a strict-DER `ECDSA-Sig-Value`. The RFC
  adapter now parses every certificate's and the CRL's `signatureValue`
  through the DER byte and node lookups into fixed 32-byte `r` and `s`, and
  four key blocks join them to those rows of sinks 0 to 3. The compiled-profile
  pin is `82e3b93f`; `a7d5eaa2` is a rejection fixture. Controls:
  `non_canonical_signature_encodings_fail_the_parsing_residues`,
  `canonical_encodings_satisfy_every_residue_and_output_the_padded_scalars`,
  `actual_traces_join_each_documents_scalars_and_reject_a_malleated_sink`,
  `an_absent_third_certificate_group_is_zero_and_a_present_one_is_active`,
  `fixed_layout_reads_the_encoding_and_the_two_scalar_windows`, the key-plan
  tests and `compiled_profile_rejects_unjoined_signature_scalars`.

## Open gaps

5. **An empty `revokedCertificates` sequence is accepted** by the grammar
   rules and rejected by the native parser. The CRL is digest-bound to the
   governed record.
6. **Block time on a quiet chain.** Consensus checks the executing block second
   against the window and the governed CRL, but a proposer can backdate a block
   after an idle period. This is the open quiet-chain item of task X.6.

Found by the second read:

8. **Child ordinals are not tied to DER sibling order.** Found on 2026-11-11
   and confirmed. A node's role and instance follow from its `BASE_CHILD`
   ordinal through the grammar rule lookup, but the ordinal product (domain
   95) pairs `(document, parent, child, count)` on `SourceNode` rows with the
   grammar ordinal table and carries no node ordinal or start, and no node
   residue relates `BASE_CHILD` to position. The product only forces the
   ordinals of each parent's children to be the set 0 to `count − 1`, so
   siblings with the same tag can exchange ordinals and roles: issuer and
   subject, notBefore and notAfter, the extensions, CRL entries, RDNs and
   attributes, EKU OIDs. The span-derived ordinal check in
   `validate_zk_x509_rfc5280_provenance_v1` runs only in the prover. The
   reference relation parses positionally, requires the fixed extension order
   and requires CRL serials strictly increasing in DER order. Exploiting it
   needs a document outside the closed profile that a governed CA signed: for
   example a leaf whose subject equals the issuing CA's Name and whose issuer
   field is another Name X passes name chaining after the swap, and its
   disclosed attributes are X's. The time slots of the CRL are pinned by the
   authoritative time blocks, the disclosed attribute is selected by its
   anchored OID and the value adjacent to it, and every CRL entry is still
   compared with the leaf, so revocation cannot be bypassed.

By severity: gap 7, open to any holder of an in-profile credential, and gaps
3 and 4 are closed in source above, gap 9 with gap 3. Of the open gaps, 8
needs a nonconforming document signed by a governed CA; gap 5 is a mismatch
without a forgery. Gap 6 is the separate quiet-chain item of task X.6.

## Fix proposals awaiting approval

The proposals for gaps 3, 4 and 7 are applied; the others are not. Each changes the relation and
the profile identity, so the affected segment and profile descriptors, the
compiled-profile pin and the fixtures are regenerated from the compilers and
the maximum proof rerun. Proof sizes below are source-derived from the framing
in the [joint relation contract](zk_x509_joint_private_relation.md).

### Gap 3: anchor every `FixedByte` row (applied)

Applied as proposed on 2026-11-11 with the owner's approval and committed as
`f6c5c4cf1b`; its pin was `a7d5eaa2`.

- **Constraint.** One RFC residue, `ACTIVE · fixed[FixedByte] · (ADDRESS −
  START − OFFSET)`, in the `FixedByte` block. Honest rows already satisfy it:
  the row builder sets `START = address − offset`. With it, every row of a
  topology-anchored run compares the node byte at `START + OFFSET`, where
  `START` is the node's start or content start. The existing continuation
  `next.ADDRESS − ADDRESS − 1` becomes implied inside the block and can stay.
- **Size.** Trace degree two, so the RFC maximum stays four. RFC local
  constraints go from 2,145 to 2,146 (the
  `fixed-equal-decimal-and-name-uniqueness` section gains one). No base or
  auxiliary column, no opening, no X5M1 field: X5M1 stays 1,034 framing bytes
  and the maximum proof 9,436,016 bytes.
- **Profile.** No compiled-profile field binds the RFC 5280 schedule by digest
  (`TODO(X.3)` in `compiled_profile_fields_v1`), so the change reaches the pin
  through the `rfc5280-temporal-air` text of field 9, which now records the
  count and the rule; the segmented STARK and RFC descriptors record them
  too, and a test ties all three to the actual constant.
- **Ledger.** Unchanged. No new map, so the point exclusion stays
  E = 3,353,141,249. D, B_MAIN = 19,398,656 and B_CA = 40,960 are unchanged
  because the RFC fixed-aware degree is. The alpha term 64/|F| counts one
  cancellation event, whatever the number of constraints. No lookup event is
  added, so the RFC collision term keeps 39 relations of at most 2^19 events.
  The exponents stay [160, 187, 160, 199, 164, 165, 158].
- **Controls.** A whole-trace control on the canonical fixture: every local
  residue on every one of the 2^19 rows is zero for the honest trace. The
  forged trace moves the twelve-row run that checks the leaf's TBS signature
  AlgorithmIdentifier onto the identical outer signatureAlgorithm bytes,
  keeping its node, start and length, and moves the DER byte-table
  multiplicity with it; the auxiliary columns are rebuilt by the prover's
  own replay, which accepts the balanced lookup. Every residue still
  vanishes except the new one, which is nonzero on each of the twelve rows.
  The profile test checks that `dd803a10` differs only in field 9 and is
  rejected.
- **Proofs.** Ordinary and maximum diagnostic proofs of 9,436,016 bytes verify
  with producer self-check and independent replay, and reject the
  wrong-genesis, 32 nonce-byte and tamper mutations; proving took 340 s and
  456 s on a loaded host, still over the 300 s target.

### Gap 4: join `r` and `s` to the documents (applied)

Option A applied on 2026-11-12 with the owner's approval; the pin is
`82e3b93f`. Options A′ and B were not taken.

- **RFC rows.** A new verifier-fixed family, `SignatureScalar`, of four
  74-row groups, one per signature: sinks 0, 1, 3 and 2 in that order, so the
  optional third certificate's group is last and the family's active rows are
  one prefix. Each group reads one document's `signatureValue` BIT STRING:
  six header rows, one row for the first byte of each INTEGER and one for the
  byte after it, then 32 output rows for `r` and 32 for `s`, big-endian and
  left-zero-padded. Group 2 is active exactly when `BASE_CERT2_ACTIVE` is one
  and the CRL group reads document `2 + BASE_CERT2_ACTIVE`; an inactive group
  is all zero, which its dummy sink matches.
- **RFC constraints.** 33 residues, all gated by the family selector: 5 of
  degree two, 22 of degree three and 6 of degree four, so the RFC maximum
  stays four. They pin the document, the grammar role and instance and a
  primitive universal BIT STRING on every row, carry the content start and
  both INTEGER lengths through the group, and require
  `ce − cs = 7 + Lr + Ls`. The first row's node query (domain 92) ties
  `(cs, ce)` to the document's `signatureValue` node. Every other byte comes
  from a DER byte query (domain 91) at `cs + offset + coefficient·Lr +
  coefficient·Ls`, with the offsets and coefficients verifier-fixed. The
  header bytes are `30`, `4 + Lr + Ls`, `02`, `Lr`, `02`, `Ls`; all lengths
  are below `0x80`, so only short-form lengths are admitted. Each length is
  `L = 1 + z + u` with `u` in [0, 31] by five bits, and `z = [c0 = 0]` by a
  zero flag and an inverse on the first byte, whose top bit must be clear. The
  second-byte row reads `c0 + z` and needs its top bit set when `z = 1`. This
  is exactly a minimal positive INTEGER of at most 32 value bytes: a 33-byte
  INTEGER must start with its sign octet. Output row `j` reads `cs + Lr − 27
  + j` for `r` and `ce − 32 + j` for `s`, and is live exactly when `L ≥ 32 −
  j`, by a six-bit range check of `L − (32 − j)` or `(31 − j) − L`; a live
  row queries its byte and any other row is zero. The record proposed a
  separate `g = [L = 33]` flag and a monotone live counter; the zero flag and
  the range check force the same predicate with fewer residues.
- **RFC cost.** No committed column: the rows use the generic base columns
  below `BASE_CERT2_ACTIVE`. One verifier-fixed column, the family selector:
  RFC fixed 147 to 148, the MAIN log19 public fixed matrix 331 to 332. The
  per-row values reuse fixed cells that every other family reads only under
  its own selector: `FIX_EXPECTED` 0 to 8, the temporal document, CRL
  document, role and instance cells, `FIX_DOCUMENT_FIXED` for optional,
  `FIX_LOCAL_FIRST` and `FIX_LOCAL_LAST`, and the global `FIX_REQUIRED_ACTIVE`
  rule. RFC local constraints go from 2,146 to 2,179 (2,150 to 2,183 with the
  terminal residues), 296 of the 232,082 padding rows become family rows, and
  the queries join the existing domain 91 and 92 lookups, so the RFC collision
  term keeps 39 relations.
- **Key plan.** Key blocks 11 to 14 join RFC `BASE_VALUE` on each group's 64
  output rows to sink `t`'s real-input column on rows 41,024 to 41,087,
  through `X^8` from log 19 to log 16. The Projection block moves to 15 and the
  activity pair to 16. Each scalar block is divided and opened while its
  sink's key block has the sink column replayed, so every real-input column is
  still replayed once per phase; the four divisions add 137,935,104 field
  steps.
- **Openings, frame and proof.** Four DEEP openings, each the sink column at
  `scale · z^8`: 17 key blocks, 36 openings, 26 key-and-digest alphas. Each of
  the first four sink columns now carries six opening sets, so its mask closure
  is (2 + 6)(136 + 4) = 1,120 ≤ 1,816. X5M1 goes from 1,034 to 1,162 framing
  bytes and the maximum X5S1 from 9,436,016 to 9,436,144 bytes, 1,040 under
  the 9,437,184-byte cap. The MAIN batch goes from 11,724 to 11,728 weighted
  functions.
- **Ledger.** Four more power-8 maps take the summed MAIN map degree from 709
  to 741, so E = 1 + 741(2^19 + 2^22) + 110(2^12 + 2^16) = 3,504,136,193.
  B_MAIN = 19,398,656 and B_CA = 40,960 are unchanged (8D is below the 32D of
  the digest blocks and every divisor lies in H19), and the algebraic error
  stays at 219 bits. The changed exclusion census still needs the
  independent review that is outstanding for the Projection map.
- **Profile.** Only field 9 changes: `x5m1`, `main-frame-bytes`,
  `main-key-byte-joins`, the 26 key-and-digest descriptors, the `deep-points`
  census, `joint-point-exclusion-bound`, `max-encoded-combined-bound` and the
  `rfc5280-temporal-air` text, which still ends `-39-relations`. The pin is
  `82e3b93f`; `a7d5eaa2`, identical except for field 9, is a rejection
  fixture, and the `dd803a10`, `cbf1bd12` and `f5d2b994` fixtures still
  reproduce. The segmented STARK, MAIN proof, key-join and RFC descriptors
  record the new counts too.
- **Prover resources.** The family selector adds one 4 MiB log19 public
  fixed column to the 12 GiB plan's live buffers (3,872,463,592 to
  3,876,657,920 bytes; every reserve still fits). On the measured maximum
  fixture the weighted terminal-link replay now admits three native device
  columns instead of four: four need 102,318,204 bytes against 102,142,158
  remaining. The device batch is the widest that fits, so proving still
  completes, with a narrower Metal batch on the largest credential.
- **Controls.** `non_canonical_signature_encodings_fail_the_parsing_residues`
  builds the group's rows from synthetic `signatureValue` contents, with every
  DER lookup answered by those contents, for every choice of the two zero
  flags: an extra leading zero (a 33-byte INTEGER and `00 01`), a missing
  leading zero, a 33-byte INTEGER with a nonzero first byte, a wrong SEQUENCE
  length with the true or matching INTEGER lengths, a trailing byte, and a
  zero-length INTEGER are each rejected by the named parsing residue or by the
  lookup. `canonical_encodings_satisfy_every_residue_and_output_the_padded_scalars`
  accepts canonical encodings with exactly one choice of flags and outputs
  the padded scalars. `actual_traces_join_each_documents_scalars_and_reject_a_malleated_sink`
  builds the depth-two and depth-three RFC traces, checks that each group's
  output rows equal `r || s` as the `p256` crate parses the document's DER,
  and that a sink holding the malleated `(r, n − s)`, another valid signature
  that certificate sinks admit, differs from the joined rows only where the
  scalar block reads, while the key and digest block rows are unchanged.
  `an_absent_third_certificate_group_is_zero_and_a_present_one_is_active`
  checks activity. The whole-trace control of gap 3 evaluates every local
  residue, the new ones included, on every row of the canonical trace.
  The key-plan tests check the four blocks' coordinates, the census of 36
  opening maps and the work counts.
- **Proofs.** Ordinary and maximum diagnostic proofs of 9,436,144 bytes, the
  new maximum, verify with producer self-check and independent replay, and
  reject the wrong-genesis, 32 nonce-byte and tamper mutations; proving took
  336 s and 330 s on a loaded host, still over the 300 s target.

### Gap 7: force minimal SHA-256 padding (applied)

Applied as proposed on 2026-11-11 with the owner's approval and committed as
`c3191d4c32`; its pin was `dd803a10`.

- **Constraint.** One SHA capacity residue, `INPUT_WORD · LENGTH_HIGH_WORD ·
  row_active · (1 − final_block) · padding_phase`: in an active block that is
  not final, the marker must not have occurred before word 14. With the
  existing final-block checks this is exactly minimal padding: either the
  marker lies in bytes 0 to 55 of the final block, or in bytes 56 to 63 of the
  block before it, and an extra all-zero block is rejected at its word 14
  because the phase is already one there. `padding_phase` is the phase before
  the row's word, so the honest boundary case, a marker in word 14 or 15 of
  the next-to-last block, still satisfies it.
- **Size.** Degree five with its two fixed selectors, inside the SHA maximum
  of six. Capacity residues go from 335 to 336 and SHA local constraints from
  593 to 594 (801 to 802 with the CA boundary equations) in every
  registration. It reuses the fixed `INPUT_WORD` and `LENGTH_HIGH_WORD`
  columns, so the four fixed schedules and their digests are unchanged; the
  SHA call-bus descriptor, a compiled-profile field, records the count and
  the rule, so the profile identity changes. No column, opening or X5M1
  field; the maximum proof stays 9,436,016 bytes.
- **Ledger.** Unchanged: no map, no lookup event, no degree increase. E stays
  3,353,141,249 and the exponents stay as they are.
- **Controls.** Whole non-minimal capacity traces, built by a test-only hook
  under the canonical fixed schedule, at the capacities of a certificate TBS
  call (4,096 bytes) and a Projection call (2,048 bytes, up to 28 extra
  blocks) and at 127 bytes, including a marker in word 14 or 15 followed by
  an extra all-zero block: every other residue vanishes and the new one is
  nonzero exactly at word 14 of each non-final block after the marker. The
  honest traces of the same messages, markers at bytes 56 and 63 of a block
  included, keep every residue zero. The profile test checks that the
  descriptor records the actual count and that `cbf1bd12` differs only in
  that field and is rejected.
- **Proofs.** Ordinary and maximum diagnostic proofs of 9,436,016 bytes verify
  with producer self-check and independent replay, and reject the
  wrong-genesis, 32 nonce-byte and tamper mutations; proving took 492 s and
  441 s on a loaded host, still over the 300 s target.

### Gaps 8 and 9: sketches

Not worked out to the same precision.

- **Gap 8.** Add each child's start and end to the domain 95 tuple on both
  sides, and give the grammar ordinal table rows a carried span: the first
  child of a group starts at the parent's content start, read by a node query
  on the group's first row, each next child starts where the previous one
  ends, and the last ends at the parent's content end. The children's spans
  then tile the parent in ordinal order, so the ordinal is the DER position.
  This reuses free tuple slots and generic columns, adds a node query to the
  existing domain 92 lookup and some RFC residues, and needs no column,
  opening or X5M1 field; the ledger is unchanged.
- **Gap 9.** Closed by masking with gap 3; no DER change was made. A DER
  residue binding `BASE_UNUSED_BITS` to the octet on the first content row of
  a BIT STRING stays available should the grammar ever admit another BIT
  STRING.

## Reported covered

| Predicate cluster | Mechanism |
|---|---|
| Statement against governed records, replay, binding to transaction, account and chain | native consensus checks in `validate_privacy_zk_x509_statement_state_v1` and the X.509 submission executor, before proof verification |
| Chain length, private depth | one Boolean `BASE_CERT2_ACTIVE`, constant across the trace, read by every depth-dependent rule and joined to P-256 sink 2 |
| Window against every certificate and the CRL | required relation slots with the public window as fixed cells and range-checked slack |
| Leaf and CA basic constraints, key usage, extended key usage, path length | verifier-fixed profile patterns, anchored to their nodes since gap 3, and path-length slots |
| Name chaining and key identifiers, including the root and the CRL issuer | fixed equality slots joined by a copy product |
| Signature messages, keys and their pairing for all five signatures | fixed output sources, SHA union, digest and key blocks; sinks other than slot 2 are required active |
| Wallet ownership digest and low `s` | verifier-fixed Projection constants, SHA call 11, digest block 4 |
| CRL number, digests and exact timestamps | profile pattern and the four authoritative blocks |
| Trust-anchor membership | governed-root producer rows of document `1 + depth bit`, four root links, 104 hash links, public root on a fixed CA row |
| Projection frames to SHA and public digests | verifier-fixed frame constants and expected digests, four Projection-to-SHA links, subject to gap 7 |
| Strict DER encoding | minimal identifiers (low or high tag form, no `0x80` lead, at most five octets), the universal tag set and form, no end-of-contents; definite minimal lengths up to `0x4000`; canonical BOOLEAN, NULL, INTEGER, ENUMERATED and OID contents; SET OF order by pair products and a zero-safe byte comparator (`der_stark.rs`); BIT STRING unused bits only through the anchored profile patterns (gap 9) |
| DER structure and node table | offset one per consumed byte, frame push and pop products equal at the last row with frame identity the unique node ordinal, so spans nest, siblings are contiguous, parents close exactly and the root covers the document; node ordinals unique per document and every node field in the tuple; document count at most 19 with indices one apart (document lengths are checked on the last document only and never exported) |
| DER to RFC source tables | the eight DER-to-RFC links equate the byte and node products (multiset equality of `(document, address, byte)` and of node tuples); `SourceNode` rows sit at fixed `(document, ordinal)` with prefix activity, so they equal the DER nodes row for row; `SourceByte` rows fix `(document, address)` below 4,096 per top-level document and are otherwise pinned by the multiset equality |
| Grammar | role, tag, parent role and count by the rule lookup against the verifier-fixed table and the parent lookup with multiplicity the parent's count; exact ordinals and counts force the required children; `(document, role, instance)` unique because the instance formulas are injective and name ordinals are bounded; which sibling takes which ordinal is gap 8 |
| Embedded documents | each embedded root is node 0 at start 0, its bytes equal the extension value bytes through the copy census (domain 104, live prefix equal to the span), and a fixed slot map assigns its kind, the extension OID being tied to the instance by an anchored `FixedByte` pattern (gap 8) |
| TBS bytes hashed | the variable-output producer reads `(document, START + i)` for every byte of the `CertificateTbs` span through the byte lookup, authenticates the node through the node lookup, and fixes the count to `CONTENT_END − START` |
| SHA-256 compression and chaining | evaluated through `evaluate_zk_x509_sha_batch_local_residues_over_field_v1`, the capacity AIR and the inner word AIR (`sha_word_stark.rs`); IV and the 64 round constants verifier-fixed as in FIPS 180-4; σ and Σ as XORs of Boolean rotated bits, Ch and Maj by 8-bit chunks; additions mod 2^32 with three carry bits over range-checked operands, at most 5·2^32 so no wrap; word memory with verifier-fixed addresses and a sorted table; blocks chained through fixed addresses, activity a block prefix, inactive rows zero and event-free |
| SHA-256 padding, message and digest rows | marker `0x80`, zero fill, word 14 zero and word 15 eight times the exact count in the final block, at most one marker; message bytes a prefix with exact count; RFC calls through the SHA union with the carried 13-bit length; Projection calls through the shared products (512 words, length, digest); frame-prefix bytes by fixed masks; eight digest rows forced active at `active_blocks·680 + j`, read by every join on the call's fixed final rows; subject to gap 7 |
| SHA union endpoints | the 24 union links close the SHA, RFC-role and Projection products at verifier-fixed points (see Observations); calls 13 to 15 are computed but no join reads them |
| P-256 field and group arithmetic | 16-bit limbs; every operand and result below the selected modulus by a borrow chain; `a·b = c + q·m` and `a ± b = c ± q·m` as exact integer identities with row residues below 2^41; complete Renes–Costello–Batina formulas for a = −3; one-hot window selection with constant Boolean bits, verifier-fixed generator table kG for k ≤ 15; `Z·Z⁻¹ = 1`, so R is not the identity; digest and `x(R)` reduced mod n with the result below n; strict low-s for the wallet; Q on the curve (`p256_air.rs`, `p256_group_air.rs`, `p256_window_air.rs`, `p256_reduction_air.rs`) |
| ECDSA equation and P-256 buses | complete schedule in `constrain_p256_ecdsa_from_source_v1`: `r·r⁻¹` and `s·s⁻¹` asserted equal to a pinned 1 with canonical operands, so `1 ≤ r, s < n`; e from the joined digest bytes as bits2int, reduced mod n; `u1 = e·s⁻¹`, `u2 = r·s⁻¹`; the 256 bits of each joined to the window bits by the scalar-bit bus; `x(R) mod n` asserted equal to r; value bus with verifier-fixed ids, limbs and access kinds and `EQUAL_NEXT` chains; cross-trace chain writer to window to reductions to low-s to sink, both ends one; every link per signature; optional sink 2 selects the dummy only when its Boolean activity, joined to the RFC, is zero; sinks 0 to 3 take `r` and `s` from the document's strict-DER `signatureValue` through the `SignatureScalar` family and key blocks 11 to 14 (gap 4) |
| Calendar arithmetic | decimal bytes anchored at content start plus a fixed position; digits 0 to 9 and `Z`; tag 23 or 24 matching the encoding; UTCTime 1970 to 2049 and GeneralizedTime 2050 to 9999; one-hot month; Euclidean year division by 4, 100 and 400 with bounded remainders and exact Gregorian leap flag; day at most the month's days, hour at most 23, minute and second at most 59; Unix seconds as a bounded integer below 2^38 (`rfc5280_calendar.rs`, `rfc5280_binding_glue.rs`) |
| Temporal relations | fixed relation slots with verifier-fixed operands, nextUpdate the only strict one; slack below 2^38 through `RangeByte` and copy domain 99; required slots active, slot 2 with the certificate selector; CRL entry times through domains 100 and 101 with the entry census setting the multiplicity |
| Lookup arguments | the gated zero-safe inverse forces its gate Boolean and pins `(zero, inverse)`; log-derivative sums start and end at zero, with an exact singular count; four independent base-field lanes; homogeneous factors are non-proportional within a domain through the constant domain tag, with an affine offset where domains share an accumulator (96 and 97, 100 and 101); table rows verifier-fixed (grammar rules in fixed cells, profile entries pinned by residues, DER byte and node rows at fixed document, address and node identifiers); multiplicities are free, so a lookup proves membership only, and "exactly once" comes from a pinned multiplicity, as for the topology table |

## Observations

Not gaps under the standard, but load-bearing and easy to break:

- A `FixedByte` run cannot continue past the end of its family block only
  because the first `EqualByte` row is a required, node-anchored slot; the
  continuation is not gated by a family-local selector.
- `Relation` rows set `FIX_LOCAL_LAST` on every row, so the grouped-activity
  residue is vacuous there; the activity of a relation pair is forced only by
  `FIX_RELATION_CONTINUE`.
- Each SHA-union endpoint omits its own row's factor, which is safe only
  because every SHA segment ends on a call without RFC events (calls 20, 25,
  9 and 28).
- The P-256 verifier hard-codes operations 13 and 14, value ids 47, 48, 52,
  53, 58 and 849 and derived value 14,826 in several modules and never checks
  them against the compiled topology; only the prover and tests do.
- The DER document-count product is accumulated but closed only in the
  prover, and the 4 KiB top-level document cap holds only through the output
  and census capacities. Nothing observable depends on either today.
- The `SignatureScalar` family has a selector of its own but reads its
  per-row offsets, coefficients, document, role and instance from fixed cells
  that other families also fill: `FIX_EXPECTED` 0 to 8, the four temporal
  identity cells, `FIX_DOCUMENT_FIXED` and the local first and last cells.
  That is safe only because every other reader of those cells multiplies them
  by its own family selector, or by the temporal or relation selector.

## Not yet read

The verifier's algebraic evaluation of the log-19 fixed schedules at the DEEP
point (`fixed_algebraic.rs`, `fixed_algebraic_sha.rs`,
`fixed_algebraic_p256.rs`), on which every "verifier-fixed" mechanism above
relies, was matched only in meaning and by sample rows, and the SHA operation,
Boolean, read and sorted-memory axis compression not at all. Also unread: the
order in which the SHA memory, call and base-folding challenges are drawn
relative to the base commitments, and the DEEP binding of the extra key and
digest openings.

## Closing a gap

A gap closes with a constraint change and a control at the verifier level: a
trace or proof that violates the predicate while satisfying every other
residue must be rejected by verifier-evaluated terms, not by witness-side
validation. A relation change alters the profile identity, so descriptors,
pins and fixtures are regenerated from the actual compiler and the maximum
proof is rerun.

TODO(X.3): as the owner approves them, close gaps 8 and 4; add forged-proof
controls for gaps 1, 2, 3 and 7 and for each new closure; read the
fixed-schedule evaluators; then repeat this read against the changed
relation.
