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
Only the closure of gap 7 is backed by forged traces, at the level of the SHA
capacity AIR; nothing has a forged-proof control yet.

## What the verifier adds across segments

`verify_main_deep_constraints_with_ca_v1` is the complete list: the local
residues of each registered segment, then

- 192 terminal links: 8 DER to RFC, 80 P-256 bus, 104 P-256 chain;
- the key plan: 13 key blocks including the Projection block, 5 SHA-to-P-256
  digest blocks, 2 authoritative CRL digest blocks, 2 authoritative CRL time
  blocks;
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
| `CertificateSlotActive` | key block 11 and its I/O block |

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

Closed on 2026-11-11 with the owner's approval, not yet committed:

7. **SHA-256 padding was not forced minimal.** The SHA capacity AIR
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

## Open gaps

3. **`FixedByte` addresses are not anchored.** Confirmed on 2026-11-11
   against the residues and the verifier. The family's residues (the
   `FixedByte` block of `evaluate_rfc5280_relation_with_context_v1`,
   `name_policy::residues` and `profile_identity::residues`) constrain
   `BASE_ADDRESS` only by `next − current = 1` inside a run; nothing equates
   it to `BASE_START + BASE_OFFSET`. The first row of a run (`OFFSET = 0`)
   queries the node topology table (domain 97), which ties `START`, the node's
   start, content start and content end, its instance and tag to a node of a
   required role, and every row queries the profile table (domain 96) and the
   DER byte table (domain 91, keyed by document, address and value). So the
   run's start and exact length belong to the node, but the compared bytes can
   be any consecutive occurrence of the pattern in the same document. Every
   sibling family anchors its addresses: Decimal, copy census, NameValue,
   variable output, path length, Projection serial and disclosure. Reach: the
   patterns in top-level documents, that is signature and SPKI algorithm
   identifiers, version, the signature unused-bits byte, the `00 04` key
   prefix, certificate and CRL extension OIDs, the `critical` byte and name
   attribute OIDs. The key usage, EKU and CRL-number patterns span their whole
   embedded document and are therefore self-anchored. Disclosed-attribute OIDs
   are anchored separately to verifier-fixed bytes in `projection_disclosure`,
   so disclosure selection is unaffected. The CRL is digest-bound to the
   governed record, so in practice the gap concerns the private certificates.
   Exploiting it needs a certificate outside the closed profile that a governed
   CA signed; the typed value checks still apply. A run cannot spill past the
   end of the family block: the next row is the first `EqualByte` row, a
   required issuer slot anchored to its own node, which cannot also be the
   run's required-role node. Fix proposal below.
4. **Signature scalars are not joined to the documents.** Confirmed on
   2026-11-11. The RFC `P256Signature` role (the 72-byte padded signature DER
   and its length, for each certificate and the CRL) closes inside the RFC
   adapter and has no external term in the key plan, the terminal links, the
   SHA union or the CA links. Each P-256 binding sink holds
   `(Qx, Qy, r, s, digest)` big-endian in its real-input column at rows
   40,960 to 41,119 (`p256_input_selection_byte_v1`). The five sink key
   blocks join rows 40,960 to 41,023 and the digest blocks rows 41,088 to
   41,119; nothing joins rows 41,024 to 41,087, which feed the ECDSA writers
   `SIGNATURE_R_ID_V1` and `SIGNATURE_S_ID_V1`. The proof therefore shows that
   some valid signature on the bound TBS digest exists under the bound issuer
   key, not that it is the one in the document, nor that the document's
   `signatureValue` is the strict-DER `ECDSA-Sig-Value` that
   `validate_ecdsa_signature_der_v1` requires. This is a mismatch with the
   reference relation and not a forgery: a valid signature over the TBS is
   still needed. It affects sinks 0 to 3; the wallet signature (sink 4) is not
   in a document. Fix proposal below.
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
9. **The BIT STRING unused-bit count is not bound to its octet.** Found on
   2026-11-11 and confirmed. In the DER AIR, `BASE_UNUSED_BITS` is packed
   from free payload cells and carried through the primitive; the only check
   on the octet itself is that it is below 8. The empty-payload and padding
   checks read the column, so `03 02 05 FF` and `03 01 07` pass with the
   column set to zero, against `der.rs` (`UnusedBitsWithoutPayload`,
   `NonZeroUnusedBits`). Masked: the closed grammar admits BIT STRINGs only
   for the two signature values, the public key and the key usage root, and
   profile patterns pin the octet to zero and the whole key usage TLV. Those
   are `FixedByte` rows, so the gap is live only through gap 3 and closes
   with it.

By severity: gap 7, open to any holder of an in-profile credential, is closed
in source above. Of the open gaps, 3 and 8 need a nonconforming document
signed by a governed CA, gap 3 being the more likely misissuance; gaps 4, 5
and 9 are mismatches without a forgery, gap 9 masked by the profile patterns.
Gap 6 is the separate quiet-chain item of task X.6.

## Fix proposals awaiting approval

Gap 7's proposal is applied; the others are not. Each changes the relation and
the profile identity, so the affected segment and profile descriptors, the
compiled-profile pin and the fixtures are regenerated from the compilers and
the maximum proof rerun. Proof sizes below are source-derived from the framing
in the [joint relation contract](zk_x509_joint_private_relation.md).

### Gap 3: anchor every `FixedByte` row

- **Constraint.** One RFC residue, `ACTIVE · fixed[FixedByte] · (ADDRESS −
  START − OFFSET)`, in the `FixedByte` block. Honest rows already satisfy it:
  the row builder sets `START = address − offset`. With it, every row of a
  topology-anchored run compares the node byte at `START + OFFSET`, where
  `START` is the node's start or content start. The existing continuation
  `next.ADDRESS − ADDRESS − 1` becomes implied inside the block and can stay.
- **Size.** Trace degree two, so the RFC maximum stays four. RFC constraints
  go from 2,145 to 2,146 (the `fixed-equal-decimal-and-name-uniqueness`
  section gains one). No base or auxiliary column, no opening, no X5M1 field:
  X5M1 stays 1,034 framing bytes and the maximum proof 9,436,016 bytes.
- **Ledger.** Unchanged. No new map, so the point exclusion stays
  E = 3,353,141,249. D, B_MAIN = 19,398,656 and B_CA = 40,960 are unchanged
  because the RFC fixed-aware degree is. The alpha term 64/|F| counts one
  cancellation event, whatever the number of constraints. No lookup event is
  added, so the RFC collision term keeps 39 relations of at most 2^19 events.
  The exponents stay [160, 187, 160, 199, 164, 165, 158].
- **Controls.** A residue control that shifts every row of an honest
  top-level run, for example the leaf's first extension OID, to another
  occurrence of the same bytes in the same document: every existing residue
  still vanishes and the DER byte query equals a genuine table factor, while
  the new residue does not vanish. A forged-trace control later.

### Gap 4: join `r` and `s` to the documents

**Option A, recommended: the RFC adapter normalizes `r` and `s`, and four new
key blocks join them.**

- **RFC rows.** A new fixed-layout family of four groups, one per signature
  in sink order (documents 0, 1, 2 and the CRL; group 2 active exactly when
  `BASE_CERT2_ACTIVE` is one, the CRL document taken as in the copy census),
  74 rows each: six header-byte rows, the first of which is also anchored by a
  node query to the `signatureValue` BIT STRING node of its document, four
  integer-head rows, then 64 output rows holding `r‖s` big-endian and
  left-zero-padded. The node's content start `cs` and end `ce` and the integer
  lengths `Lr`, `Ls` are carried through the group.
- **RFC constraints.** DER byte queries at anchored addresses force
  `cs+1 = 0x30`, `cs+2 = ce − cs − 3`, `cs+3 = 0x02`, `cs+4 = Lr`,
  `cs+5+Lr = 0x02`, `cs+6+Lr = Ls`, and `ce − cs = 7 + Lr + Ls`. Each length
  lies in [1, 33]: five bits plus a flag `g = [L = 33]` that zeroes them. Each
  integer is minimal and positive: the first content byte `c0` has its top bit
  clear; if `c0 = 0` then `L ≥ 2` and the next byte has its top bit set;
  `g = 1` forces `c0 = 0`. Output row `j` of `r` reads address
  `cs + Lr − 27 + j`, of `s` address `ce − 32 + j`. A Boolean `live`, monotone
  in `j` with exactly `L − g` live rows, gates the DER byte query, and
  non-live rows hold zero. The 32 output bytes are then the integer's value,
  left-padded, a sign octet inside the window contributing its zero byte. The
  BIT STRING's unused-bits byte stays the purpose-4 profile pattern, anchored
  once gap 3 is closed. The range `1 ≤ r, s < n` stays with P-256.
- **RFC cost.** No new committed column: one base column costs about 1,152
  proof bytes (136 query values and two DEEP values) against 1,168 bytes of
  room, so the rows reuse the generic columns. One more verifier-fixed family
  selector, about 50 to 60 residues at degree at most four, and 296 rows out
  of 232,082 padding rows. The queries join the existing domain 91 and 92
  lookups, so the RFC collision term is unchanged.
- **Key plan.** Four new blocks, signature `t` in 0 to 3: RFC `BASE_VALUE` on
  the group's 64 output rows equals sink `t`'s real-input column on rows
  41,024 to 41,087, through the native-root power map `X^8` from log 19 to
  log 16. The blocks are unconditional: an inactive group 2 has zero rows and
  an inactive sink holds zero there (`p256_inactive_real_byte_v1`).
- **Openings and frame.** Four extra DEEP openings, one per block, each the
  sink column at its own point `scale · z^8`. The plan goes from 13 to 17 key
  blocks, from 32 to 36 openings and from 22 to 26 alphas; each sink column's
  mask closure becomes (2 + 2)(136 + 4) = 560 ≤ 1,816. X5M1 grows from 32 to
  36 Fp4 values, 1,034 to 1,162 framing bytes; the maximum proof goes from
  9,436,016 to 9,436,144 bytes, 1,040 under the 9,437,184-byte cap. The MAIN
  batch goes from 11,724 to 11,728 weighted functions; X5S1 is unchanged.
- **Ledger.** Four more power-8 maps take the summed map degree from 709 to
  741, so E = 1 + 741(2^19 + 2^22) + 110(2^12 + 2^16) = 3,504,136,193. B_MAIN
  and B_CA are unchanged: 8D stays below the 32D of the digest blocks, every
  divisor still lies in H19, and no block exceeds 1,357 roots. The point term
  64(B_MAIN + B_CA)/(|F| − E) stays below 2^-220 and the exponents are
  unchanged. The degree census reads 17 key blocks, and the changed exclusion
  census needs the independent review already outstanding.
- **Also changes.** `ZK_X509_MAIN_FRAME_BYTES_V1` and the envelope constant,
  `scripts/check_zk_x509_proof_geometry.py` (the 1,034 literal and the X5S1
  maximum), the joint-ledger fields of the profile descriptor, and
  `deep_point_census_counts_the_actual_opening_maps`.
- **Controls.** A trace whose output rows hold a valid signature other than
  the document's, a non-minimal or negative integer, a 33-byte integer with a
  nonzero first byte, and a wrong length byte, each rejected by evaluated
  terms.

**Option A′: no new openings.** Reorder the RFC output topology so that each
key channel is followed by a 64-byte `r‖s` channel, and extend the four
certificate and CRL sink key blocks from 64 to 128 equalities (RFC rows
`rows[i]+1` to `rows[i]+128` against sink rows 40,960 to 41,087). E, X5M1 and the proof size stay as they
are, but the I/O declarations, which mirror the RFC channels, and every later
channel offset change, and the header rows still need a family of their own.

**Option B: no circuit change.** Change the reference relation so that the
certificate and CRL signature predicate reads "the TBS digest has a valid
P-256 signature under the issuer key", with `signatureValue` checked only by
the grammar and the profile. The security property, that the issuing CA
authenticated the TBS, is kept and the mismatch is closed by definition, but
the relation then accepts a document whose own signature field is malformed
or holds a different signature.

### Gap 7: force minimal SHA-256 padding (applied)

Applied as proposed on 2026-11-11 with the owner's approval, and not yet
committed; the pin is `dd803a10`.

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

Not yet worked out to the same precision.

- **Gap 8.** Add each child's start and end to the domain 95 tuple on both
  sides, and give the grammar ordinal table rows a carried span: the first
  child of a group starts at the parent's content start, read by a node query
  on the group's first row, each next child starts where the previous one
  ends, and the last ends at the parent's content end. The children's spans
  then tile the parent in ordinal order, so the ordinal is the DER position.
  This reuses free tuple slots and generic columns, adds a node query to the
  existing domain 92 lookup and some RFC residues, and needs no column,
  opening or X5M1 field; the ledger is unchanged.
- **Gap 9.** One DER residue binding `BASE_UNUSED_BITS` to the octet on the
  first content row of a BIT STRING, its degree to be checked against the DER
  maximum. No column or opening, ledger unchanged. Since gap 3's fix masks it
  fully, it could ride with that revision.

## Reported covered

| Predicate cluster | Mechanism |
|---|---|
| Statement against governed records, replay, binding to transaction, account and chain | native consensus checks in `validate_privacy_zk_x509_statement_state_v1` and the X.509 submission executor, before proof verification |
| Chain length, private depth | one Boolean `BASE_CERT2_ACTIVE`, constant across the trace, read by every depth-dependent rule and joined to P-256 sink 2 |
| Window against every certificate and the CRL | required relation slots with the public window as fixed cells and range-checked slack |
| Leaf and CA basic constraints, key usage, extended key usage, path length | verifier-fixed profile patterns and path-length slots, subject to gap 3 |
| Name chaining and key identifiers, including the root and the CRL issuer | fixed equality slots joined by a copy product |
| Signature messages, keys and their pairing for all five signatures | fixed output sources, SHA union, digest and key blocks; sinks other than slot 2 are required active |
| Wallet ownership digest and low `s` | verifier-fixed Projection constants, SHA call 11, digest block 4 |
| CRL number, digests and exact timestamps | profile pattern and the four authoritative blocks |
| Trust-anchor membership | governed-root producer rows of document `1 + depth bit`, four root links, 104 hash links, public root on a fixed CA row |
| Projection frames to SHA and public digests | verifier-fixed frame constants and expected digests, four Projection-to-SHA links, subject to gap 7 |
| Strict DER encoding | minimal identifiers (low or high tag form, no `0x80` lead, at most five octets), the universal tag set and form, no end-of-contents; definite minimal lengths up to `0x4000`; canonical BOOLEAN, NULL, INTEGER, ENUMERATED and OID contents; SET OF order by pair products and a zero-safe byte comparator (`der_stark.rs`); BIT STRING subject to gap 9 |
| DER structure and node table | offset one per consumed byte, frame push and pop products equal at the last row with frame identity the unique node ordinal, so spans nest, siblings are contiguous, parents close exactly and the root covers the document; node ordinals unique per document and every node field in the tuple; document count at most 19 with indices one apart (document lengths are checked on the last document only and never exported) |
| DER to RFC source tables | the eight DER-to-RFC links equate the byte and node products (multiset equality of `(document, address, byte)` and of node tuples); `SourceNode` rows sit at fixed `(document, ordinal)` with prefix activity, so they equal the DER nodes row for row; `SourceByte` rows fix `(document, address)` below 4,096 per top-level document and are otherwise pinned by the multiset equality |
| Grammar | role, tag, parent role and count by the rule lookup against the verifier-fixed table and the parent lookup with multiplicity the parent's count; exact ordinals and counts force the required children; `(document, role, instance)` unique because the instance formulas are injective and name ordinals are bounded; which sibling takes which ordinal is gap 8 |
| Embedded documents | each embedded root is node 0 at start 0, its bytes equal the extension value bytes through the copy census (domain 104, live prefix equal to the span), and a fixed slot map assigns its kind, the extension OID being tied to the instance by a `FixedByte` pattern (gaps 3 and 8) |
| TBS bytes hashed | the variable-output producer reads `(document, START + i)` for every byte of the `CertificateTbs` span through the byte lookup, authenticates the node through the node lookup, and fixes the count to `CONTENT_END − START` |
| SHA-256 compression and chaining | evaluated through `evaluate_zk_x509_sha_batch_local_residues_over_field_v1`, the capacity AIR and the inner word AIR (`sha_word_stark.rs`); IV and the 64 round constants verifier-fixed as in FIPS 180-4; σ and Σ as XORs of Boolean rotated bits, Ch and Maj by 8-bit chunks; additions mod 2^32 with three carry bits over range-checked operands, at most 5·2^32 so no wrap; word memory with verifier-fixed addresses and a sorted table; blocks chained through fixed addresses, activity a block prefix, inactive rows zero and event-free |
| SHA-256 padding, message and digest rows | marker `0x80`, zero fill, word 14 zero and word 15 eight times the exact count in the final block, at most one marker; message bytes a prefix with exact count; RFC calls through the SHA union with the carried 13-bit length; Projection calls through the shared products (512 words, length, digest); frame-prefix bytes by fixed masks; eight digest rows forced active at `active_blocks·680 + j`, read by every join on the call's fixed final rows; subject to gap 7 |
| SHA union endpoints | the 24 union links close the SHA, RFC-role and Projection products at verifier-fixed points (see Observations); calls 13 to 15 are computed but no join reads them |
| P-256 field and group arithmetic | 16-bit limbs; every operand and result below the selected modulus by a borrow chain; `a·b = c + q·m` and `a ± b = c ± q·m` as exact integer identities with row residues below 2^41; complete Renes–Costello–Batina formulas for a = −3; one-hot window selection with constant Boolean bits, verifier-fixed generator table kG for k ≤ 15; `Z·Z⁻¹ = 1`, so R is not the identity; digest and `x(R)` reduced mod n with the result below n; strict low-s for the wallet; Q on the curve (`p256_air.rs`, `p256_group_air.rs`, `p256_window_air.rs`, `p256_reduction_air.rs`) |
| ECDSA equation and P-256 buses | complete schedule in `constrain_p256_ecdsa_from_source_v1`: `r·r⁻¹` and `s·s⁻¹` asserted equal to a pinned 1 with canonical operands, so `1 ≤ r, s < n`; e from the joined digest bytes as bits2int, reduced mod n; `u1 = e·s⁻¹`, `u2 = r·s⁻¹`; the 256 bits of each joined to the window bits by the scalar-bit bus; `x(R) mod n` asserted equal to r; value bus with verifier-fixed ids, limbs and access kinds and `EQUAL_NEXT` chains; cross-trace chain writer to window to reductions to low-s to sink, both ends one; every link per signature; optional sink 2 selects the dummy only when its Boolean activity, joined to the RFC, is zero; subject to gap 4 |
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

TODO(X.3): as the owner approves them, close gaps 3 (with 9), 8 and 4; add
forged-proof controls for gaps 1, 2 and 7 and for each new closure; read the
fixed-schedule evaluators; then repeat this read against the changed
relation.
