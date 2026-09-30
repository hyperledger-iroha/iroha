//! Bounded seekable decode and verification of one canonical source record.
//!
//! The decoder indexes canonical public polynomials without owning their
//! residues, parses only the bounded signed proof responses, checks the record
//! footer and EOF, then verifies every relation one RNS limb at a time. The
//! returned receipt is move-only and contains no replayable evidence graph.
use super::*;
const SOURCE_RELEASE_LIMBS_V1: usize = 38;
const SOURCE_EVIDENCE_PREFIX_BYTES_V1: usize = 4 + 1 + 1 + 8;
const SOURCE_TRUSTED_CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-source-trusted-context";
const SOURCE_VALIDATED_RECEIPT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-source-validated-receipt";
/// Compact move-only context for seekable source-evidence verification.
///
/// Construction first validates the materialized aggregate key and ordered
/// shares (or an already sealed staged CPK context). Only governed identity,
/// the active authentication roster, and public-polynomial digests survive.
pub struct ZkAmsMkheTrustedSourceContextV1 {
    active_roster: ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    public_key_a_native_digest: [u8; 32],
    public_key_a_wire_digest: [u8; 32],
    party_public_b_native_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    party_public_b_wire_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    cks_verification_seal: [u8; 32],
    verification_seal: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheTrustedSourceContextV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheTrustedSourceContextV1")
            .field(
                "profile_digest",
                &hex::encode(self.active_roster.profile_digest()),
            )
            .field(
                "roster_digest",
                &hex::encode(self.active_roster.roster_digest()),
            )
            .field(
                "key_material_digest",
                &hex::encode(self.active_roster.key_material_digest()),
            )
            .field("epoch", &self.active_roster.epoch())
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field(
                "collective_key_digest",
                &hex::encode(self.collective_key_digest),
            )
            .field("privately_sealed", &(self.verification_seal != [0; 32]))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheTrustedSourceContextV1 {
    /// Crate-private mint seam for the consuming staged CPK ceremony.
    ///
    /// The CKS context has already sealed the exact verified A/B native and
    /// wire digests. This seam borrows no residue, key, or share owner and is
    /// deliberately not a public digest-authority constructor.
    pub(in super::super) fn from_staged_verified_digests(
        active_roster: ZkAmsMkheGovernedActiveRosterV1,
        cks_context: &ZkAmsMkheTrustedCksContextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        active_roster.validate()?;
        cks_context.validate()?;
        if active_roster.to_wire_roster()? != cks_context.roster
            || active_roster.key_material_digest() != cks_context.key_material_digest
            || cks_context.transcript_digest == [0; 32]
            || cks_context.collective_key_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut context = Self {
            active_roster,
            transcript_digest: cks_context.transcript_digest,
            collective_key_digest: cks_context.collective_key_digest,
            public_key_a_native_digest: cks_context.public_key_a_native_digest,
            public_key_a_wire_digest: cks_context.public_key_a_wire_digest,
            party_public_b_native_digests: cks_context.party_public_b_native_digests,
            party_public_b_wire_digests: cks_context.party_public_b_wire_digests,
            cks_verification_seal: cks_context.verification_seal,
            verification_seal: [0; 32],
        };
        context.verification_seal = trusted_source_context_seal(&context);
        context.validate()?;
        Ok(context)
    }
    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.active_roster.validate()?;
        if self.transcript_digest == [0; 32]
            || self.collective_key_digest == [0; 32]
            || self.public_key_a_native_digest == [0; 32]
            || self.public_key_a_wire_digest == [0; 32]
            || self.party_public_b_native_digests.contains(&[0; 32])
            || self.party_public_b_wire_digests.contains(&[0; 32])
            || self.cks_verification_seal == [0; 32]
            || self.verification_seal == [0; 32]
            || self.verification_seal != trusted_source_context_seal(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
fn trusted_source_context_seal(context: &ZkAmsMkheTrustedSourceContextV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_TRUSTED_CONTEXT_DOMAIN_V1);
    hash.update(&context.active_roster.profile_digest());
    hash.update(&context.active_roster.roster_digest());
    hash.update(&context.active_roster.key_material_digest());
    hash.update(&context.active_roster.epoch().to_be_bytes());
    hash.update(&context.transcript_digest);
    hash.update(&context.collective_key_digest);
    hash.update(&context.public_key_a_native_digest);
    hash.update(&context.public_key_a_wire_digest);
    for digest in context.party_public_b_native_digests {
        hash.update(&digest);
    }
    for digest in context.party_public_b_wire_digests {
        hash.update(&digest);
    }
    hash.update(&context.cks_verification_seal);
    hash.finalize()
}
/// Move-only receipt for one exactly decoded and fully verified `ZASE` record.
///
/// No polynomial, proof, canonical payload, reader, path, or provider is
/// retained, and there is deliberately no replay method.
pub struct ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    statement_digest: [u8; 32],
    payload_digest: [u8; 32],
    contribution_digest: [u8; 32],
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
    verification_seal: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1")
            .field("kind", &self.kind)
            .field("ordinal", &self.ordinal)
            .field("source_record_index", &self.source_record_index)
            .field("party_index", &self.party_index)
            .field("statement_digest", &hex::encode(self.statement_digest))
            .field("canonical_bytes", &self.canonical_bytes)
            .field("canonical_digest", &hex::encode(self.canonical_digest))
            .field("privately_sealed", &(self.verification_seal != [0; 32]))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
    /// Canonical source-record family which was verified.
    #[must_use]
    pub const fn kind(&self) -> ZkAmsMkheCollectiveEvidenceRecordKindV1 {
        self.kind
    }
    /// Evaluated-key ordinal containing this source record.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Gap-free canonical record position within that source set.
    #[must_use]
    pub const fn source_record_index(&self) -> u32 {
        self.source_record_index
    }
    /// Exact governed contributor position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }
    /// Exact verified active-relation statement digest.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }
    /// Authenticated active proof payload digest.
    #[must_use]
    pub const fn payload_digest(&self) -> [u8; 32] {
        self.payload_digest
    }
    /// Digest of the governed authenticated contribution.
    #[must_use]
    pub const fn contribution_digest(&self) -> [u8; 32] {
        self.contribution_digest
    }
    /// Exact canonical record length accepted from durable storage.
    #[must_use]
    pub const fn canonical_bytes(&self) -> u64 {
        self.canonical_bytes
    }
    /// Verified digest footer of every preceding canonical record byte.
    #[must_use]
    pub const fn canonical_digest(&self) -> [u8; 32] {
        self.canonical_digest
    }
    /// Decode exactly one seekable `ZASE` record and mint a bounded receipt.
    ///
    /// Every public polynomial is indexed in place and digest-checked on each
    /// limb reread. Exact EOF is checked both before and after verification.
    pub fn decode_and_verify_canonical_exact<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        trusted_context: &ZkAmsMkheTrustedSourceContextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        decode_and_verify_source_evidence_record_streaming(reader, trusted_context)
    }
}
fn index_canonical_source_polynomial<R>(
    body: &mut CanonicalBodyReader<'_, R>,
    profile: &BgvProfile,
) -> Result<super::super::active::IndexedActiveSourcePolynomialV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let residue_count = canonical_polynomial_residue_count()?;
    if usize::try_from(read_canonical_u32(body)?).ok() != Some(residue_count)
        || profile.moduli.len() != SOURCE_RELEASE_LIMBS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let residues_offset = body.absolute_position()?;
    let mut native_hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut wire_hash = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut native_limb_digests = [[0_u8; 32]; SOURCE_RELEASE_LIMBS_V1];
    let mut wire_limb_digests = [[0_u8; 32]; SOURCE_RELEASE_LIMBS_V1];
    let mut buffer = [0_u8; SEEKABLE_EVALUATED_KEY_READ_BYTES_V1];
    let mut nonzero = false;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let (mut native_limb, mut wire_limb) =
            super::super::active::indexed_source_limb_hashers_v1(profile, limb)?;
        let mut remaining = profile.ring_degree;
        while remaining != 0 {
            let take_residues = remaining.min(buffer.len() / 8);
            let take_bytes = take_residues * 8;
            read_canonical_raw_exact(body, &mut buffer[..take_bytes])?;
            native_hash.update(&buffer[..take_bytes]);
            wire_hash.update(&buffer[..take_bytes]);
            native_limb.update(&buffer[..take_bytes]);
            wire_limb.update(&buffer[..take_bytes]);
            for encoded in buffer[..take_bytes].chunks_exact(8) {
                let residue = u64::from_be_bytes(
                    encoded
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                if residue >= modulus {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                nonzero |= residue != 0;
            }
            remaining -= take_residues;
        }
        native_limb_digests[limb] = native_limb.finalize();
        wire_limb_digests[limb] = wire_limb.finalize();
    }
    super::super::active::IndexedActiveSourcePolynomialV1::new(
        residues_offset,
        native_hash.finalize(),
        wire_hash.finalize(),
        native_limb_digests,
        wire_limb_digests,
        nonzero,
    )
}
fn validate_source_outer_coordinate_v1(
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    ordinal: u8,
    source_record_index: u32,
    party_index: usize,
    statement: &super::super::active::IndexedActiveSourceStatementV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected = match (kind, statement) {
        (
            ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne,
            super::super::active::IndexedActiveSourceStatementV1::RkgRoundOne {
                left,
                right,
                digit_index,
                ..
            },
        ) => {
            if ordinal != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            expected_rkg_source_record_index(
                &trusted_context.active_roster,
                *left,
                *right,
                *digit_index,
                party_index,
                false,
            )?
        }
        (
            ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo,
            super::super::active::IndexedActiveSourceStatementV1::RkgRoundTwo {
                left,
                right,
                digit_index,
                ..
            },
        ) => {
            if ordinal != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            expected_rkg_source_record_index(
                &trusted_context.active_roster,
                *left,
                *right,
                *digit_index,
                party_index,
                true,
            )?
        }
        (
            ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource,
            super::super::active::IndexedActiveSourceStatementV1::Galois {
                schedule_index,
                exponent,
                digit_index,
                ..
            },
        ) => {
            let schedule = zk_ams_t256_galois_key_schedule_v1()?;
            validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
            if usize::from(*schedule_index) >= ZK_AMS_T256_GALOIS_KEY_COUNT_V1
                || schedule
                    .entries
                    .get(usize::from(*schedule_index))
                    .is_none_or(|entry| entry.exponent != *exponent)
                || ordinal != schedule_index.saturating_add(1)
                || usize::try_from(*digit_index)
                    .ok()
                    .is_none_or(|digit| digit >= release_profile_v1().gadget_digits)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            digit_index
                .checked_mul(
                    u32::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                )
                .and_then(|base| base.checked_add(u32::try_from(party_index).ok()?))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        }
        _ => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
    };
    if source_record_index != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn source_validated_receipt_seal(
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    statement_digest: [u8; 32],
    payload_digest: [u8; 32],
    contribution_digest: [u8; 32],
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(SOURCE_VALIDATED_RECEIPT_DOMAIN_V1);
    hash.update(&trusted_context.verification_seal);
    hash.update(&[kind as u8, ordinal, party_index]);
    hash.update(&source_record_index.to_be_bytes());
    hash.update(&statement_digest);
    hash.update(&payload_digest);
    hash.update(&contribution_digest);
    hash.update(&canonical_bytes.to_be_bytes());
    hash.update(&canonical_digest);
    hash.finalize()
}
/// Return only the fixed admission axes of an exactly resealed trusted source
/// context. Sibling evidence aggregation cannot inspect or retain the roster.
pub(super) fn verified_evidence_context_summary_v1(
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
) -> Result<super::evidence_set::SourceEvidenceContextSummaryV1, ZkAmsMkheErrorV1> {
    trusted_context.validate()?;
    Ok(super::evidence_set::SourceEvidenceContextSummaryV1 {
        axes: super::evidence_set::EvidenceContextAxesV1 {
            profile_digest: trusted_context.active_roster.profile_digest(),
            roster_digest: trusted_context.active_roster.roster_digest(),
            key_material_digest: trusted_context.active_roster.key_material_digest(),
            epoch: trusted_context.active_roster.epoch(),
            transcript_digest: trusted_context.transcript_digest,
            collective_key_digest: trusted_context.collective_key_digest,
        },
        context_seal: trusted_context.verification_seal,
        linked_cks_context_seal: trusted_context.cks_verification_seal,
    })
}
/// Consume one move-only source receipt after recomputing its private seal
/// against the exact trusted context. Only compact descriptor fields escape.
pub(super) fn consume_verified_evidence_receipt_v1(
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
    receipt: ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1,
) -> Result<super::evidence_set::SourceEvidenceReceiptSummaryV1, ZkAmsMkheErrorV1> {
    trusted_context.validate()?;
    let expected = source_validated_receipt_seal(
        trusted_context,
        receipt.kind,
        receipt.ordinal,
        receipt.source_record_index,
        receipt.party_index,
        receipt.statement_digest,
        receipt.payload_digest,
        receipt.contribution_digest,
        receipt.canonical_bytes,
        receipt.canonical_digest,
    );
    if receipt.verification_seal == [0; 32] || receipt.verification_seal != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(super::evidence_set::SourceEvidenceReceiptSummaryV1 {
        kind: receipt.kind,
        ordinal: receipt.ordinal,
        record_index: receipt.source_record_index,
        party_index: receipt.party_index,
        canonical_bytes: receipt.canonical_bytes,
        canonical_digest: receipt.canonical_digest,
    })
}
fn decode_and_verify_source_evidence_record_streaming<R>(
    reader: &mut R,
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
) -> Result<ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    trusted_context.validate()?;
    let mut prefix = [0_u8; SOURCE_EVIDENCE_PREFIX_BYTES_V1];
    read_canonical_raw_exact(reader, &mut prefix)?;
    if prefix[..4] != SOURCE_EVIDENCE_RECORD_TAG_V1 || prefix[4] != MKHE_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let kind = ZkAmsMkheCollectiveEvidenceRecordKindV1::decode(prefix[5])?;
    if kind == ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let canonical_bytes = u64::from_be_bytes(
        prefix[6..14]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    let body_bytes = source_evidence_body_bytes_v1(canonical_bytes)?;
    let mut body = CanonicalBodyReader::new(reader, &prefix, body_bytes);
    let ordinal = read_canonical_u8(&mut body)?;
    let source_record_index = read_canonical_u32(&mut body)?;
    let party_index = read_canonical_u8(&mut body)?;
    let profile_digest = read_canonical_array(&mut body)?;
    let roster_digest = read_canonical_array(&mut body)?;
    let key_material_digest = read_canonical_array(&mut body)?;
    let epoch = read_canonical_u64(&mut body)?;
    let transcript_digest = read_canonical_array(&mut body)?;
    let collective_key_digest = read_canonical_array(&mut body)?;
    if profile_digest != trusted_context.active_roster.profile_digest()
        || roster_digest != trusted_context.active_roster.roster_digest()
        || key_material_digest != trusted_context.active_roster.key_material_digest()
        || epoch != trusted_context.active_roster.epoch()
        || transcript_digest != trusted_context.transcript_digest
        || collective_key_digest != trusted_context.collective_key_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    let statement = match kind {
        ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne => {
            let left = read_canonical_party(&mut body)?;
            let right = read_canonical_party(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            super::super::active::IndexedActiveSourceStatementV1::RkgRoundOne {
                public_a: index_canonical_source_polynomial(&mut body, &profile)?,
                party_public_b: index_canonical_source_polynomial(&mut body, &profile)?,
                common_a: index_canonical_source_polynomial(&mut body, &profile)?,
                h0: index_canonical_source_polynomial(&mut body, &profile)?,
                h1: index_canonical_source_polynomial(&mut body, &profile)?,
                left,
                right,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo => {
            let left = read_canonical_party(&mut body)?;
            let right = read_canonical_party(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            super::super::active::IndexedActiveSourceStatementV1::RkgRoundTwo {
                public_a: index_canonical_source_polynomial(&mut body, &profile)?,
                party_public_b: index_canonical_source_polynomial(&mut body, &profile)?,
                common_a: index_canonical_source_polynomial(&mut body, &profile)?,
                h0: index_canonical_source_polynomial(&mut body, &profile)?,
                h1: index_canonical_source_polynomial(&mut body, &profile)?,
                aggregate_h0: index_canonical_source_polynomial(&mut body, &profile)?,
                aggregate_h1: index_canonical_source_polynomial(&mut body, &profile)?,
                k0: index_canonical_source_polynomial(&mut body, &profile)?,
                left,
                right,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource => {
            let schedule_index = read_canonical_u8(&mut body)?;
            let exponent = read_canonical_u32(&mut body)?;
            let digit_index = read_canonical_u32(&mut body)?;
            super::super::active::IndexedActiveSourceStatementV1::Galois {
                public_a: index_canonical_source_polynomial(&mut body, &profile)?,
                party_public_b: index_canonical_source_polynomial(&mut body, &profile)?,
                source_constant: index_canonical_source_polynomial(&mut body, &profile)?,
                source_linear: index_canonical_source_polynomial(&mut body, &profile)?,
                schedule_index,
                exponent,
                digit_index,
            }
        }
        ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit => unreachable!(),
    };
    let party_index_usize = usize::from(party_index);
    if party_index_usize >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    validate_source_outer_coordinate_v1(
        trusted_context,
        kind,
        ordinal,
        source_record_index,
        party_index_usize,
        &statement,
    )?;
    let (public_a, party_public_b) = statement.public_key_polynomials();
    if public_a.native_digest() != trusted_context.public_key_a_native_digest
        || public_a.wire_digest() != trusted_context.public_key_a_wire_digest
        || party_public_b.native_digest()
            != trusted_context.party_public_b_native_digests[party_index_usize]
        || party_public_b.wire_digest()
            != trusted_context.party_public_b_wire_digests[party_index_usize]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let proof_bytes = read_canonical_u64(&mut body)?;
    if proof_bytes == 0 || proof_bytes > body.remaining() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let proof = super::super::active::decode_indexed_active_source_proof_v1(
        &mut body,
        proof_bytes,
        statement.expected_witnesses(),
    )?;
    let canonical_digest = finish_canonical_body(body)?;
    let canonical_end = reader
        .stream_position()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    require_canonical_reader_eof(reader)?;
    let verified = super::super::active::verify_indexed_active_source_proof_v1(
        reader,
        &trusted_context.active_roster,
        trusted_context.transcript_digest,
        party_index_usize,
        &statement,
        &proof,
    )?;
    reader
        .seek(std::io::SeekFrom::Start(canonical_end))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    // Verification seeks backward through the same provider. Re-establish EOF
    // so an append or stateful-source substitution cannot survive the receipt.
    require_canonical_reader_eof(reader)?;
    let verification_seal = source_validated_receipt_seal(
        trusted_context,
        kind,
        ordinal,
        source_record_index,
        party_index,
        verified.statement_digest,
        verified.payload_digest,
        verified.contribution_digest,
        canonical_bytes,
        canonical_digest,
    );
    Ok(ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
        kind,
        ordinal,
        source_record_index,
        party_index,
        statement_digest: verified.statement_digest,
        payload_digest: verified.payload_digest,
        contribution_digest: verified.contribution_digest,
        canonical_bytes,
        canonical_digest,
        verification_seal,
    })
}
pub(super) fn source_evidence_body_bytes_v1(canonical_bytes: u64) -> Result<u64, ZkAmsMkheErrorV1> {
    let maximum = u64::try_from(maximum_source_evidence_record_bytes()?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let minimum =
        u64::try_from(SOURCE_EVIDENCE_COMMON_BODY_BYTES_V1 + EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if canonical_bytes < minimum || canonical_bytes > maximum {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    canonical_bytes
        .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1 as u64)
        .and_then(|value| value.checked_sub(SOURCE_EVIDENCE_PREFIX_BYTES_V1 as u64))
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
}
#[cfg(test)]
pub(super) fn test_mint_verified_evidence_receipt_v1(
    trusted_context: &ZkAmsMkheTrustedSourceContextV1,
    kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    ordinal: u8,
    source_record_index: u32,
    party_index: u8,
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
) -> ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
    let statement_digest = keccak256(&[
        b's',
        kind as u8,
        ordinal,
        party_index,
        source_record_index as u8,
    ]);
    let payload_digest = keccak256(&[
        b'p',
        kind as u8,
        ordinal,
        party_index,
        source_record_index as u8,
    ]);
    let contribution_digest = keccak256(&[
        b'c',
        kind as u8,
        ordinal,
        party_index,
        source_record_index as u8,
    ]);
    let verification_seal = source_validated_receipt_seal(
        trusted_context,
        kind,
        ordinal,
        source_record_index,
        party_index,
        statement_digest,
        payload_digest,
        contribution_digest,
        canonical_bytes,
        canonical_digest,
    );
    ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1 {
        kind,
        ordinal,
        source_record_index,
        party_index,
        statement_digest,
        payload_digest,
        contribution_digest,
        canonical_bytes,
        canonical_digest,
        verification_seal,
    }
}
#[cfg(test)]
pub(super) fn test_tamper_verified_evidence_receipt_seal_v1(
    receipt: &mut ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1,
) {
    receipt.verification_seal[0] ^= 1;
}
#[cfg(test)]
mod tests {
    #[test]
    fn exact_release_source_record_lengths_are_frozen() {
        const LIMB_BYTES: usize = 131_072 * 8;
        const POLYNOMIAL_BYTES: usize = 4 + 38 * LIMB_BYTES;
        const ACTIVE_PROOF_FIXED_BYTES: usize = 42 + 42 + 305;
        const RKG_ONE_PROOF: usize = ACTIVE_PROOF_FIXED_BYTES + 5 * LIMB_BYTES;
        const RKG_TWO_PROOF: usize = ACTIVE_PROOF_FIXED_BYTES + 6 * LIMB_BYTES;
        const COMMON: usize = 188;
        const FOOTER: usize = 32;
        assert_eq!(POLYNOMIAL_BYTES, 39_845_892);
        assert_eq!(RKG_ONE_PROOF, 5_243_269);
        assert_eq!(RKG_TWO_PROOF, 6_291_845);
        assert_eq!(
            COMMON + 68 + 5 * POLYNOMIAL_BYTES + 8 + RKG_ONE_PROOF + FOOTER,
            204_473_025
        );
        assert_eq!(
            COMMON + 68 + 8 * POLYNOMIAL_BYTES + 8 + RKG_TWO_PROOF + FOOTER,
            325_059_277
        );
        assert_eq!(
            COMMON + 9 + 4 * POLYNOMIAL_BYTES + 8 + RKG_ONE_PROOF + FOOTER,
            164_627_074
        );
    }
    #[test]
    fn production_decoder_is_seekable_bounded_and_receipt_only() {
        let source = include_str!("source_stream.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(production.lines().count() <= 900);
        assert!(production.contains("std::io::Read + std::io::Seek"));
        assert!(production.contains("index_canonical_source_polynomial"));
        assert!(production.contains("native_limb_digests"));
        assert!(production.contains("wire_limb_digests"));
        assert!(
            production
                .matches("require_canonical_reader_eof(reader)?")
                .count()
                >= 2
        );
        assert!(!production.contains("read_canonical_wire_polynomial"));
        assert!(!production.contains("ZkAmsMkheRnsPolynomialWireV1"));
        assert!(!production.contains("pub fn verify("));
        assert!(!production.contains("OwnedCollectiveSource"));
        let receipt = production
            .split("pub struct ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1")
            .nth(1)
            .expect("receipt exists")
            .split("impl core::fmt::Debug")
            .next()
            .expect("receipt fields end before Debug");
        for forbidden in [
            "Vec<",
            "reader:",
            "path:",
            "provider:",
            "proof:",
            "statement:",
        ] {
            assert!(
                !receipt.contains(forbidden),
                "forbidden receipt owner: {forbidden}"
            );
        }
    }
    #[test]
    fn production_parent_has_no_legacy_owned_decoder_or_heavy_generator_entry() {
        let source = include_str!("source_stream.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let parent = include_str!("../collective_eval_keys.rs");
        assert!(parent.lines().count() <= 5_000);
        assert!(!parent.contains("OwnedCollectiveSource"));
        assert!(!parent.contains("decode_source_evidence_record"));
        assert!(!parent.contains("read_canonical_wire_polynomial"));
        for name in [
            "fn add_weighted_pair_source(",
            "fn generate_zk_ams_mkhe_collective_relinearization_key_v1",
            "fn generate_zk_ams_mkhe_collective_galois_key_v1",
        ] {
            let position = parent
                .find(name)
                .expect("test reference implementation remains");
            let prelude = &parent[position.saturating_sub(192)..position];
            assert!(
                prelude.contains("#[cfg(test)]"),
                "production-heavy entry: {name}"
            );
        }
        let context_name = "pub struct ZkAmsMkheTrustedSourceContextV1";
        let context_position = source
            .find(context_name)
            .expect("trusted source context exists");
        let context_prelude = &source[context_position.saturating_sub(192)..context_position];
        assert!(!context_prelude.contains("derive(Clone"));
        let context_impl = production
            .split("impl ZkAmsMkheTrustedSourceContextV1")
            .nth(1)
            .expect("trusted source context implementation exists")
            .split("fn trusted_source_context_seal")
            .next()
            .expect("context implementation is bounded");
        assert!(context_impl.contains("pub(in super::super) fn from_staged_verified_digests"));
        assert!(!context_impl.contains("pub fn from_"));
        assert!(!context_impl.contains("shares:"));
        assert!(!context_impl.contains("residues:"));
    }
}
