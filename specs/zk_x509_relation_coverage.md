# zk-X509 relation coverage

Source contract for the coverage part of task X.3 of the
[ZK delivery plan](zk_delivery_plan.md). It records which predicates of the
native reference relation
(`privacy_engines::zk_x509::relation::validate_reference_relation_v1`) the
verifier forces on a prover who commits arbitrary trace values, and which it
does not. It makes no soundness, zero-knowledge or performance claim, and it
does not replace the [joint relation contract](zk_x509_joint_private_relation.md).

Read on 2026-11-10 at `11d30fb89d`. A predicate counts as forced only by a
residue the verifier evaluates, a verifier-fixed column or public input, or a
cross-segment join with both endpoints constrained. Checks in witness or
trace-building code do not count. Gaps 1 to 4 below were re-read against the
constraint code and the verifier's term list as it stood at that revision;
gap 5 is one reader's finding on the grammar side. The covered rows are per-cluster readings with spot checks.
Nothing here is backed by a forged-trace control yet.

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

These were found by the read above and are closed by relation changes recorded
in the [joint relation contract](zk_x509_joint_private_relation.md). The
controls evaluate plans and residues directly. Neither has a forged-proof
control yet.

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

## Open gaps

3. **`FixedByte` addresses are not anchored.** A run constrains
   `BASE_ADDRESS` only by its increment; nothing equates it to
   `BASE_START + BASE_OFFSET`, and the byte lookup is keyed by document,
   address and value. A fixed pattern such as an extension OID or `critical`
   byte can be read elsewhere in the same document. Exploiting it needs a
   certificate outside the closed profile that a governed CA signed; the typed
   value checks still apply. The sibling families anchor their addresses.
4. **Signature scalars are not joined to the documents.** The `P256Signature`
   role has no external term, so `r` and `s` are any pair valid for the bound
   key and digest. This is a mismatch with the reference relation, which
   parses the signature value, and not a forgery.
5. **An empty `revokedCertificates` sequence is accepted** by the grammar
   rules and rejected by the native parser. The CRL is digest-bound to the
   governed record.
6. **Block time on a quiet chain.** Consensus checks the executing block second
   against the window and the governed CRL, but a proposer can backdate a block
   after an idle period. This is the open quiet-chain item of task X.6.

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
| Projection frames to SHA and public digests | verifier-fixed frame constants and expected digests, four Projection-to-SHA links |

## Not yet read

The DER segment and the integrity of the DER-to-RFC source tables, including
uniqueness of embedded document bytes and of roles per document; SHA-256
compression, padding and digest-row selection; P-256 arithmetic internals;
calendar arithmetic; soundness of the zero-safe lookup arguments. Several
covered rows above depend on them.

## Closing a gap

A gap closes with a constraint change and a control at the verifier level: a
trace or proof that violates the predicate while satisfying every other
residue must be rejected by verifier-evaluated terms, not by witness-side
validation. A relation change alters the profile identity, so descriptors,
pins and fixtures are regenerated from the actual compiler and the maximum
proof is rerun.

TODO(X.3): add forged-proof controls for gaps 1 and 2, close gaps 3 and 4,
read the remaining segments, then repeat this read against the changed
relation.
