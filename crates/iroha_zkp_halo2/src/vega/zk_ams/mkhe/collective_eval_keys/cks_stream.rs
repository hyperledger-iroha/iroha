//! Bounded seekable decode and verification of one canonical CKS evidence record.
//!
//! The verified receipt retains no reader or canonical payload. Relation polynomials are indexed
//! once and digest-checked on every arithmetic reread; one canonical contribution is decoded,
//! verified, folded, and dropped before the next contribution is read.
use super::*;
fn cks_residue_digests(
    profile: &BgvProfile,
    residues: &[u64],
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    let residue_count = canonical_polynomial_residue_count()?;
    if residues.len() != residue_count {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (limb, values) in residues.chunks_exact(profile.ring_degree).enumerate() {
        let modulus = *profile
            .moduli
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        if values.iter().any(|value| *value >= modulus) {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    let mut native_hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut wire_hash = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    update_rns_digest_hasher(&mut native_hash, residues);
    update_rns_digest_hasher(&mut wire_hash, residues);
    Ok((native_hash.finalize(), wire_hash.finalize()))
}
#[cfg(test)]
pub(super) fn trusted_context_from_verified_key_and_shares(
    active_roster: &ZkAmsMkheGovernedActiveRosterV1,
    collective_key: &ZkAmsMkheCollectivePublicKeyV1,
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<ZkAmsMkheTrustedCksContextV1, ZkAmsMkheErrorV1> {
    validate_evidence_collective_context(
        active_roster,
        active_roster.profile_digest(),
        active_roster.roster_digest(),
        active_roster.key_material_digest(),
        active_roster.epoch(),
        collective_key.transcript_digest(),
        collective_key.digest(),
        collective_key,
        shares,
    )?;
    let profile = release_profile_v1();
    let roster = active_roster.to_wire_roster()?;
    let (public_key_a_native_digest, public_key_a_wire_digest) =
        cks_residue_digests(&profile, shares[0].public_a().residues())?;
    let mut party_public_b_native_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    let mut party_public_b_wire_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let (native, wire) =
            cks_residue_digests(&profile, shares[party_index].party_public_b().residues())?;
        party_public_b_native_digests[party_index] = native;
        party_public_b_wire_digests[party_index] = wire;
    }
    ZkAmsMkheTrustedCksContextV1::from_staged_verified_digests(
        roster,
        active_roster.key_material_digest(),
        collective_key.transcript_digest(),
        collective_key.digest(),
        std::array::from_fn(|index| shares[index].digest()),
        public_key_a_native_digest,
        public_key_a_wire_digest,
        party_public_b_native_digests,
        party_public_b_wire_digests,
    )
}
fn zero_native_cks_polynomial_digest(profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let residue_count = canonical_polynomial_residue_count()?;
    let mut hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let zeroes = [0_u64; 512];
    let mut remaining = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    while remaining != 0 {
        let take = remaining.min(zeroes.len());
        update_rns_digest_hasher(&mut hash, &zeroes[..take]);
        remaining -= take;
    }
    Ok(hash.finalize())
}
fn signed_cks_response_limb(
    values: &[i64],
    modulus: u64,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1> {
    let mut residues = ZeroizingU64VectorV1::with_capacity_exact(values.len())?;
    residues.extend(values.iter().map(|value| signed_mod(*value, modulus)));
    Ok(residues)
}
fn sparse_cks_challenge_limb(
    values: &[i8],
    modulus: u64,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1> {
    let mut residues = ZeroizingU64VectorV1::with_capacity_exact(values.len())?;
    residues.extend(
        values
            .iter()
            .map(|value| signed_mod(i64::from(*value), modulus)),
    );
    Ok(residues)
}
fn wide_cks_response_limb(
    proof: &ZkAmsMkheCksProofV1,
    modulus: u64,
    expected_degree: usize,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1> {
    let mut residues = ZeroizingU64VectorV1::with_capacity_exact(expected_degree)?;
    for response in proof.smudge_responses()? {
        residues.push(response?.mod_u64(modulus));
    }
    if residues.len() != expected_degree {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    Ok(residues)
}
fn zeroizing_negacyclic_multiply(
    left: &[u64],
    right: &[u64],
    modulus: u64,
    root: u64,
) -> Result<ZeroizingU64VectorV1, ZkAmsMkheErrorV1> {
    if left.len() != right.len() || left.is_empty() || !left.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut left_twisted = ZeroizingU64VectorV1::with_capacity_exact(left.len())?;
    let mut right_twisted = ZeroizingU64VectorV1::with_capacity_exact(right.len())?;
    let mut twist = 1_u64;
    for (&left, &right) in left.iter().zip(right) {
        left_twisted.push(mod_mul(left, twist, modulus));
        right_twisted.push(mod_mul(right, twist, modulus));
        twist = mod_mul(twist, root, modulus);
    }
    let cyclic_root = mod_mul(root, root, modulus);
    super::super::cyclic_ntt(&mut left_twisted, cyclic_root, modulus);
    super::super::cyclic_ntt(&mut right_twisted, cyclic_root, modulus);
    for (left, right) in left_twisted.iter_mut().zip(right_twisted.iter()) {
        *left = mod_mul(*left, *right, modulus);
    }
    super::super::inverse_cyclic_ntt(&mut left_twisted, cyclic_root, modulus)?;
    let inverse_root =
        super::super::mod_inverse(root, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for value in left_twisted.iter_mut() {
        *value = mod_mul(*value, untwist, modulus);
        untwist = mod_mul(untwist, inverse_root, modulus);
    }
    Ok(left_twisted)
}
fn update_indexed_cks_reread_hashes(
    native_hash: &mut Keccak256,
    wire_hash: &mut Keccak256,
    residues: &[u64],
) {
    update_rns_digest_hasher(native_hash, residues);
    update_rns_digest_hasher(wire_hash, residues);
}
fn finish_indexed_cks_reread_hashes(
    native_hash: Keccak256,
    wire_hash: Keccak256,
    indexed: IndexedCksPolynomialV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if native_hash.finalize() != indexed.native_digest
        || wire_hash.finalize() != indexed.wire_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn verify_indexed_cks_contribution_limbwise<R>(
    reader: &mut R,
    statement: &IndexedCksStatementV1,
    party_index: usize,
    wire: &ZkAmsMkheCksContributionWireV1,
    accumulator: &mut ZeroizingU64VectorV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let profile = release_profile_v1();
    let evidence = zk_ams_mkhe_cks_resource_evidence_v1()?;
    let binding = streaming_cks_binding_v1(
        &statement.roster,
        statement.transcript_digest,
        statement.source_digest,
        statement.key_context_digest,
        statement.source_record_index,
        statement.sample_index,
        party_index,
        statement.level,
    )?;
    let (contribution_digest, contribution_wire_digest) =
        cks_residue_digests(&profile, wire.contribution().residues())?;
    if wire.proof().statement_digest()
        != streaming_cks_wire_statement_digest_v1(
            wire.binding(),
            statement.source_digest,
            statement.roster.parties()[party_index],
            contribution_wire_digest,
        )
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let proof = ZkAmsMkheCksProofV1::decode_release_exact(wire.proof().proof_bytes())?;
    let authentication = ArtifactAuthentication {
        version: MKHE_VERSION_V1,
        party: wire.authentication().party(),
        public_key: wire.authentication().public_key(),
        signature: wire.authentication().signature(),
    };
    authentication.verify(
        streaming_cks_auth_domain_v1(),
        streaming_cks_record_digest_v1(&binding, contribution_digest, wire.proof().proof_bytes())?,
    )?;
    if authentication.party != statement.roster.parties()[party_index] {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let smudge_bits = usize::from(evidence.smudge_quotient_bits);
    proof.validate_release_response_bounds(smudge_bits)?;
    checked_ring_multiplication_work(&profile, 8)?;
    let challenge = derive_cks_sparse_challenge(profile.ring_degree, proof.challenge_seed())?;
    let residue_count = canonical_polynomial_residue_count()?;
    if accumulator.len() != residue_count || challenge.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    let mut public_a_native = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut public_a_wire = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut party_b_native = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut party_b_wire = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut target_native = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut target_wire = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let source_index = statement.source_components[party_index];
    let mut source_hashes = source_index
        .map(|_| {
            Ok::<_, ZkAmsMkheErrorV1>((
                new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?,
                new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?,
            ))
        })
        .transpose()?;
    let mut public_key_commitment_hash =
        new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut contribution_commitment_hash =
        new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let root = profile.negacyclic_roots[limb];
        let public_a = read_indexed_cks_limb(reader, statement.public_key_a, &profile, limb)?;
        let party_b = read_indexed_cks_limb(
            reader,
            statement.party_public_b[party_index],
            &profile,
            limb,
        )?;
        let target = read_indexed_cks_limb(reader, statement.target_a, &profile, limb)?;
        let source = match source_index {
            Some(indexed) => read_indexed_cks_limb(reader, indexed, &profile, limb)?,
            None => ZeroizingU64VectorV1::zeroed(profile.ring_degree)?,
        };
        update_indexed_cks_reread_hashes(&mut public_a_native, &mut public_a_wire, &public_a);
        update_indexed_cks_reread_hashes(&mut party_b_native, &mut party_b_wire, &party_b);
        update_indexed_cks_reread_hashes(&mut target_native, &mut target_wire, &target);
        if let Some((native, wire_hash)) = &mut source_hashes {
            update_indexed_cks_reread_hashes(native, wire_hash, &source);
        }
        let secret_response = signed_cks_response_limb(proof.secret_responses(), modulus)?;
        let error_response = signed_cks_response_limb(proof.public_key_error_responses(), modulus)?;
        let smudge_response = wide_cks_response_limb(&proof, modulus, profile.ring_degree)?;
        let challenge_response = sparse_cks_challenge_limb(&challenge, modulus)?;
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        let mut public_key_commitment =
            zeroizing_negacyclic_multiply(&public_a, &secret_response, modulus, root)?;
        for (value, error) in public_key_commitment.iter_mut().zip(error_response.iter()) {
            *value = if *value == 0 { 0 } else { modulus - *value };
            *value = mod_add(*value, mod_mul(plaintext_modulus, *error, modulus), modulus);
        }
        let party_b_challenge =
            zeroizing_negacyclic_multiply(&party_b, &challenge_response, modulus, root)?;
        for (value, folded) in public_key_commitment
            .iter_mut()
            .zip(party_b_challenge.iter())
        {
            *value = mod_sub(*value, *folded, modulus);
        }
        update_rns_digest_hasher(&mut public_key_commitment_hash, &public_key_commitment);
        drop(party_b_challenge);
        drop(public_key_commitment);
        drop(public_a);
        drop(party_b);
        drop(error_response);
        let mut multiplier = source;
        for (value, target) in multiplier.iter_mut().zip(target.iter()) {
            *value = mod_sub(*value, *target, modulus);
        }
        drop(target);
        let mut contribution_commitment =
            zeroizing_negacyclic_multiply(&multiplier, &secret_response, modulus, root)?;
        for (value, smudge) in contribution_commitment
            .iter_mut()
            .zip(smudge_response.iter())
        {
            *value = mod_add(
                *value,
                mod_mul(plaintext_modulus, *smudge, modulus),
                modulus,
            );
        }
        let limb_start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let limb_end = limb_start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let contribution = wire
            .contribution()
            .residues()
            .get(limb_start..limb_end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let contribution_challenge =
            zeroizing_negacyclic_multiply(contribution, &challenge_response, modulus, root)?;
        for (value, folded) in contribution_commitment
            .iter_mut()
            .zip(contribution_challenge.iter())
        {
            *value = mod_sub(*value, *folded, modulus);
        }
        update_rns_digest_hasher(&mut contribution_commitment_hash, &contribution_commitment);
        let accumulator_limb = accumulator
            .get_mut(limb_start..limb_end)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        for (value, contribution) in accumulator_limb.iter_mut().zip(contribution) {
            *value = mod_add(*value, *contribution, modulus);
        }
        drop(contribution_challenge);
        drop(contribution_commitment);
        drop(multiplier);
        drop(secret_response);
        drop(smudge_response);
        drop(challenge_response);
    }
    finish_indexed_cks_reread_hashes(public_a_native, public_a_wire, statement.public_key_a)?;
    finish_indexed_cks_reread_hashes(
        party_b_native,
        party_b_wire,
        statement.party_public_b[party_index],
    )?;
    finish_indexed_cks_reread_hashes(target_native, target_wire, statement.target_a)?;
    if let (Some(indexed), Some((native, wire_hash))) = (source_index, source_hashes) {
        finish_indexed_cks_reread_hashes(native, wire_hash, indexed)?;
    }
    let source_component_digest = match source_index {
        Some(indexed) => indexed.native_digest,
        None => zero_native_cks_polynomial_digest(&profile)?,
    };
    let expected_challenge_seed = streaming_cks_challenge_seed_v1(
        &profile,
        &binding,
        statement.public_key_a.native_digest,
        statement.party_public_b[party_index].native_digest,
        source_component_digest,
        statement.target_a.native_digest,
        contribution_digest,
        public_key_commitment_hash.finalize(),
        contribution_commitment_hash.finalize(),
        smudge_bits,
    )?;
    if expected_challenge_seed != proof.challenge_seed() {
        return Err(ZkAmsMkheErrorV1::InvalidCksProof);
    }
    Ok(contribution_digest)
}
fn compare_indexed_cks_polynomial<R>(
    reader: &mut R,
    indexed: IndexedCksPolynomialV1,
    expected: &[u64],
    mismatch_error: ZkAmsMkheErrorV1,
) -> Result<(), ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    let profile = release_profile_v1();
    let residue_count = canonical_polynomial_residue_count()?;
    if expected.len() != residue_count {
        return Err(mismatch_error);
    }
    let mut native_hash = new_rns_digest_hasher(RNS_NATIVE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut wire_hash = new_rns_digest_hasher(RNS_WIRE_DIGEST_DOMAIN_V1, residue_count)?;
    let mut equal = true;
    for limb in 0..profile.moduli.len() {
        let observed = read_indexed_cks_limb(reader, indexed, &profile, limb)?;
        update_indexed_cks_reread_hashes(&mut native_hash, &mut wire_hash, &observed);
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        equal &= expected.get(start..end) == Some(observed.as_slice());
    }
    finish_indexed_cks_reread_hashes(native_hash, wire_hash, indexed)?;
    if !equal {
        return Err(mismatch_error);
    }
    Ok(())
}
fn compare_indexed_cks_accumulator<R>(
    reader: &mut R,
    indexed: IndexedCksPolynomialV1,
    accumulator: &ZeroizingU64VectorV1,
) -> Result<(), ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    compare_indexed_cks_polynomial(
        reader,
        indexed,
        accumulator,
        ZkAmsMkheErrorV1::InvalidCksSet,
    )
}
#[allow(clippy::too_many_arguments)]
fn cks_validated_receipt_seal(
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
    statement: &IndexedCksStatementV1,
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    compact_constant_digest: [u8; 32],
    contribution_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
) -> [u8; 32] {
    cks_validated_receipt_seal_from_fields_v1(
        trusted_context,
        statement.transcript_digest,
        ordinal,
        digit_index,
        collective_key_digest,
        statement.source_digest,
        statement.key_context_digest,
        compact_constant_digest,
        contribution_digests,
        canonical_bytes,
        canonical_digest,
    )
}
#[allow(clippy::too_many_arguments)]
fn cks_validated_receipt_seal_from_fields_v1(
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
    transcript_digest: [u8; 32],
    ordinal: u8,
    digit_index: u8,
    collective_key_digest: [u8; 32],
    source_digest: [u8; 32],
    key_context_digest: [u8; 32],
    compact_constant_digest: [u8; 32],
    contribution_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CKS_VALIDATED_RECEIPT_DOMAIN_V1);
    hash.update(&trusted_context.verification_seal);
    hash.update(&trusted_context.roster.profile_digest());
    hash.update(&trusted_context.roster.roster_digest());
    hash.update(&trusted_context.key_material_digest);
    hash.update(&trusted_context.roster.epoch().to_be_bytes());
    hash.update(&transcript_digest);
    hash.update(&trusted_context.collective_key_digest);
    for digest in trusted_context.share_digests {
        hash.update(&digest);
    }
    hash.update(&[ordinal, digit_index]);
    hash.update(&collective_key_digest);
    hash.update(&source_digest);
    hash.update(&key_context_digest);
    hash.update(&compact_constant_digest);
    for digest in contribution_digests {
        hash.update(digest);
    }
    hash.update(&canonical_bytes.to_be_bytes());
    hash.update(&canonical_digest);
    hash.finalize()
}
/// Return the fixed axes of a resealed trusted CKS context without exposing
/// its share digests or polynomial identities to the sibling collector.
pub(super) fn verified_evidence_context_summary_v1(
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
) -> Result<super::evidence_set::CksEvidenceContextSummaryV1, ZkAmsMkheErrorV1> {
    trusted_context.validate()?;
    Ok(super::evidence_set::CksEvidenceContextSummaryV1 {
        axes: super::evidence_set::EvidenceContextAxesV1 {
            profile_digest: trusted_context.roster.profile_digest(),
            roster_digest: trusted_context.roster.roster_digest(),
            key_material_digest: trusted_context.key_material_digest,
            epoch: trusted_context.roster.epoch(),
            transcript_digest: trusted_context.transcript_digest,
            collective_key_digest: trusted_context.collective_key_digest,
        },
        context_seal: trusted_context.verification_seal,
    })
}
/// Consume one move-only CKS receipt after recomputing its seal against the
/// exact trusted context. The returned summary owns no proof or polynomial.
pub(super) fn consume_verified_evidence_receipt_v1(
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
    receipt: ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1,
) -> Result<super::evidence_set::CksEvidenceReceiptSummaryV1, ZkAmsMkheErrorV1> {
    trusted_context.validate()?;
    let expected = cks_validated_receipt_seal_from_fields_v1(
        trusted_context,
        trusted_context.transcript_digest,
        receipt.ordinal,
        receipt.digit_index,
        receipt.collective_key_digest,
        receipt.source_digest,
        receipt.key_context_digest,
        receipt.compact_constant_digest,
        &receipt.contribution_digests,
        receipt.canonical_bytes,
        receipt.canonical_digest,
    );
    if receipt.verification_seal == [0; 32]
        || receipt.verification_seal != expected
        || receipt.collective_key_digest != trusted_context.collective_key_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(super::evidence_set::CksEvidenceReceiptSummaryV1 {
        ordinal: receipt.ordinal,
        digit_index: receipt.digit_index,
        canonical_bytes: receipt.canonical_bytes,
        canonical_digest: receipt.canonical_digest,
        compact_constant_digest: receipt.compact_constant_digest,
    })
}
fn validate_cks_outer_coordinate_v1(
    profile: &BgvProfile,
    ordinal: u8,
    digit_index: u8,
    source_record_index: u32,
    sample_index: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    let ordinal = usize::from(ordinal);
    let digit_index = usize::from(digit_index);
    if ordinal > ZK_AMS_T256_GALOIS_KEY_COUNT_V1 || digit_index >= profile.gadget_digits {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let expected_record_index = ordinal
        .checked_mul(profile.gadget_digits)
        .and_then(|base| base.checked_add(digit_index))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if source_record_index != expected_record_index
        || sample_index != u64::from(expected_record_index)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}
pub(super) fn decode_and_verify_cks_evidence_record_streaming<R>(
    reader: &mut R,
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
) -> Result<ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1, ZkAmsMkheErrorV1>
where
    R: std::io::Read + std::io::Seek,
{
    const PREFIX_BYTES: usize = 4 + 1 + 8;
    let mut prefix = [0_u8; PREFIX_BYTES];
    read_canonical_raw_exact(reader, &mut prefix)?;
    if prefix[..4] != CKS_EVIDENCE_RECORD_TAG_V1 || prefix[4] != MKHE_VERSION_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let canonical_bytes = u64::from_be_bytes(
        prefix[5..13]
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
    );
    let maximum = u64::try_from(maximum_cks_evidence_record_bytes()?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let minimum =
        u64::try_from(CKS_EVIDENCE_COMMON_BODY_BYTES_V1 + EVIDENCE_RECORD_DIGEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if canonical_bytes < minimum || canonical_bytes > maximum {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let body_bytes = canonical_bytes
        .checked_sub(EVIDENCE_RECORD_DIGEST_BYTES_V1 as u64)
        .and_then(|value| value.checked_sub(PREFIX_BYTES as u64))
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let mut body = CanonicalBodyReader::new(reader, &prefix, body_bytes);
    let ordinal = read_canonical_u8(&mut body)?;
    let digit_index = read_canonical_u8(&mut body)?;
    let collective_key_digest = read_canonical_array(&mut body)?;
    trusted_context.validate()?;
    let trusted_roster = trusted_context.roster;
    let trusted_roster_bytes = trusted_roster.encode()?;
    let roster_bytes = usize::try_from(read_canonical_u32(&mut body)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if roster_bytes != trusted_roster_bytes.len() || roster_bytes > 4_096 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let encoded_roster = read_canonical_vec_exact(&mut body, roster_bytes, 4_096)?;
    let roster = ZkAmsMkheGovernedRosterWireV1::decode_exact(
        &encoded_roster,
        trusted_roster.profile_digest(),
        trusted_roster.epoch(),
    )?;
    if roster != trusted_roster {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let transcript_digest = read_canonical_array(&mut body)?;
    let source_record_index = read_canonical_u32(&mut body)?;
    let sample_index = read_canonical_u64(&mut body)?;
    let level = read_canonical_u8(&mut body)?;
    let encoded_source_digest = read_canonical_array(&mut body)?;
    let profile = release_profile_v1();
    validate_cks_outer_coordinate_v1(
        &profile,
        ordinal,
        digit_index,
        source_record_index,
        sample_index,
    )?;
    let source_constant = index_canonical_cks_polynomial(&mut body, &profile)?;
    let component_count = usize::from(read_canonical_u8(&mut body)?);
    if component_count != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let mut source_components = [None; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for (party_index, source_component) in source_components.iter_mut().enumerate() {
        let party = read_canonical_party(&mut body)?;
        if party != roster.parties()[party_index] {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        match read_canonical_u8(&mut body)? {
            0 => {}
            1 => {
                let component = index_canonical_cks_polynomial(&mut body, &profile)?;
                *source_component = Some(component);
            }
            _ => return Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
    ZkAmsMkheWireBindingV1::new(&roster, transcript_digest, source_record_index, level)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?;
    if sample_index
        >= super::super::manifest::zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch
        || source_components
            .iter()
            .flatten()
            .any(|component| !component.nonzero)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let source_component_digests =
        source_components.map(|component| component.map(|value| value.native_digest));
    let source_digest = streaming_cks_source_digest_v1(
        &roster,
        transcript_digest,
        source_record_index,
        sample_index,
        level,
        source_constant.native_digest,
        &source_component_digests,
    )?;
    if source_digest != encoded_source_digest {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let target_a = index_canonical_cks_polynomial(&mut body, &profile)?;
    let public_key_a = index_canonical_cks_polynomial(&mut body, &profile)?;
    if usize::from(read_canonical_u8(&mut body)?) != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let mut party_public_b = [source_constant; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for value in &mut party_public_b {
        *value = index_canonical_cks_polynomial(&mut body, &profile)?;
    }
    let compact_constant = index_canonical_cks_polynomial(&mut body, &profile)?;
    if usize::from(read_canonical_u8(&mut body)?) != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidCksSet);
    }
    if !target_a.nonzero
        || !public_key_a.nonzero
        || party_public_b.iter().any(|value| !value.nonzero)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let party_public_b_digests = party_public_b.map(|value| value.native_digest);
    let key_context_digest = streaming_cks_key_context_digest_v1(
        target_a.native_digest,
        public_key_a.native_digest,
        &party_public_b_digests,
    );
    let statement = IndexedCksStatementV1 {
        roster,
        transcript_digest,
        source_record_index,
        sample_index,
        level,
        source_digest,
        key_context_digest,
        source_constant,
        source_components,
        target_a,
        public_key_a,
        party_public_b,
        compact_constant,
    };
    let sequential_position = body.absolute_position()?;
    let mut accumulator =
        load_indexed_cks_accumulator(&mut *body.reader, source_constant, &profile)?;
    body.reader
        .seek(std::io::SeekFrom::Start(sequential_position))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let evidence = zk_ams_mkhe_cks_resource_evidence_v1()?;
    if !evidence.proof_payload_ceiling_met || !evidence.contribution_ceiling_met {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let exact_contribution_bytes = usize::try_from(evidence.total_contribution_record_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let contribution_ceiling = maximum_cks_contribution_record_bytes()?;
    let mut contribution_digests = [[0_u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1];
    for (party_index, contribution_digest) in contribution_digests.iter_mut().enumerate() {
        let bytes = usize::try_from(read_canonical_u64(&mut body)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if u64::try_from(bytes)
            .ok()
            .is_none_or(|bytes| bytes > body.remaining())
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if bytes == 0 || bytes > contribution_ceiling {
            return Err(ZkAmsMkheErrorV1::WireTooLarge);
        }
        if bytes != exact_contribution_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let encoded = ZeroizingByteVectorV1::read_exact(&mut body, bytes)?;
        let sequential_position = body.absolute_position()?;
        let binding = ZkAmsMkheWireBindingV1::new(
            &statement.roster,
            statement.transcript_digest,
            u32::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            statement.level,
        )?;
        let wire = ZkAmsMkheCksContributionWireV1::decode_exact(
            &encoded,
            &statement.roster,
            binding,
            statement.source_digest,
        )?;
        drop(encoded);
        *contribution_digest = verify_indexed_cks_contribution_limbwise(
            &mut *body.reader,
            &statement,
            party_index,
            &wire,
            &mut accumulator,
        )?;
        body.reader
            .seek(std::io::SeekFrom::Start(sequential_position))
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        drop(wire);
    }
    let canonical_digest = finish_canonical_body(body)?;
    let canonical_end = reader
        .stream_position()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    reader
        .seek(std::io::SeekFrom::Start(canonical_end))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    require_canonical_reader_eof(reader)?;
    if statement.transcript_digest != trusted_context.transcript_digest
        || collective_key_digest != trusted_context.collective_key_digest
        || statement.public_key_a.native_digest != trusted_context.public_key_a_native_digest
        || statement.public_key_a.wire_digest != trusted_context.public_key_a_wire_digest
        || statement
            .party_public_b
            .iter()
            .zip(trusted_context.party_public_b_native_digests)
            .any(|(indexed, trusted)| indexed.native_digest != trusted)
        || statement
            .party_public_b
            .iter()
            .zip(trusted_context.party_public_b_wire_digests)
            .any(|(indexed, trusted)| indexed.wire_digest != trusted)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    compare_indexed_cks_accumulator(reader, statement.compact_constant, &accumulator)?;
    reader
        .seek(std::io::SeekFrom::Start(canonical_end))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    // The compact-constant comparison seeks backward through the same
    // provider. Re-establish exact EOF after that final reread so a stateful
    // or concurrently extended source cannot append bytes between checks.
    require_canonical_reader_eof(reader)?;
    let verification_seal = cks_validated_receipt_seal(
        trusted_context,
        &statement,
        ordinal,
        digit_index,
        collective_key_digest,
        compact_constant.native_digest,
        &contribution_digests,
        canonical_bytes,
        canonical_digest,
    );
    Ok(ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
        ordinal,
        digit_index,
        collective_key_digest,
        source_digest,
        key_context_digest,
        compact_constant_digest: compact_constant.native_digest,
        contribution_digests,
        canonical_bytes,
        canonical_digest,
        verification_seal,
    })
}
#[cfg(test)]
pub(super) fn test_mint_verified_evidence_receipt_v1(
    trusted_context: &ZkAmsMkheTrustedCksContextV1,
    ordinal: u8,
    digit_index: u8,
    canonical_bytes: u64,
    canonical_digest: [u8; 32],
    compact_constant_digest: [u8; 32],
) -> ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
    let source_digest = keccak256(&[b's', ordinal, digit_index]);
    let key_context_digest = keccak256(&[b'k', ordinal, digit_index]);
    let contribution_digests = std::array::from_fn(|party_index| {
        keccak256(&[
            b'c',
            ordinal,
            digit_index,
            u8::try_from(party_index).expect("release party index fits u8"),
        ])
    });
    let verification_seal = cks_validated_receipt_seal_from_fields_v1(
        trusted_context,
        trusted_context.transcript_digest,
        ordinal,
        digit_index,
        trusted_context.collective_key_digest,
        source_digest,
        key_context_digest,
        compact_constant_digest,
        &contribution_digests,
        canonical_bytes,
        canonical_digest,
    );
    ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1 {
        ordinal,
        digit_index,
        collective_key_digest: trusted_context.collective_key_digest,
        source_digest,
        key_context_digest,
        compact_constant_digest,
        contribution_digests,
        canonical_bytes,
        canonical_digest,
        verification_seal,
    }
}
#[cfg(test)]
pub(super) fn test_tamper_verified_evidence_receipt_seal_v1(
    receipt: &mut ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1,
) {
    receipt.verification_seal[0] ^= 1;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_receipt_has_no_owned_evidence_graph_or_replay_api() {
        let parent = include_str!("../collective_eval_keys.rs");
        assert!(parent.lines().count() <= 5_000);
        let context_name = "pub struct ZkAmsMkheTrustedCksContextV1";
        let context_position = parent
            .find(context_name)
            .expect("compact trusted context remains public");
        let context_prelude = &parent[context_position.saturating_sub(192)..context_position];
        assert!(!context_prelude.contains("derive(Clone"));
        let context_shape = parent[context_position..]
            .split("impl core::fmt::Debug")
            .next()
            .expect("trusted context shape ends before Debug");
        for forbidden in ["Vec<", "reader:", "path:", "provider:", "RnsPolynomial"] {
            assert!(
                !context_shape.contains(forbidden),
                "forbidden trusted-context field: {forbidden}"
            );
        }
        assert!(parent.contains("from_staged_verified_digests"));
        let name = "pub struct ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1";
        let position = parent.find(name).expect("receipt remains public");
        let prelude = &parent[position.saturating_sub(192)..position];
        assert!(!prelude.contains("derive(Clone"));
        let shape = parent[position..]
            .split("impl core::fmt::Debug")
            .next()
            .expect("receipt shape ends before Debug");
        for forbidden in [
            "Vec<",
            "reader:",
            "path:",
            "provider:",
            "canonical_bytes: Vec",
        ] {
            assert!(
                !shape.contains(forbidden),
                "forbidden receipt field: {forbidden}"
            );
        }
        let implementation = parent
            .split("impl ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1")
            .nth(1)
            .expect("receipt implementation exists")
            .split("/// Generation-driven durable sink")
            .next()
            .expect("receipt implementation is bounded");
        assert!(implementation.contains("std::io::Read + std::io::Seek"));
        assert!(implementation.contains("trusted_context: &ZkAmsMkheTrustedCksContextV1"));
        assert!(!implementation.contains("collective_key: &"));
        assert!(!implementation.contains("shares: [&"));
        assert!(!implementation.contains("pub fn verify("));
    }
    #[test]
    fn streaming_source_keeps_one_record_and_checks_before_receipt() {
        let source = include_str!("cks_stream.rs");
        let implementation = source
            .split("#[cfg(test)]")
            .next()
            .expect("implementation precedes tests");
        assert!(implementation.lines().count() <= 1_000);
        let normalized_lines = implementation.lines().map(str::trim).collect::<Vec<_>>();
        assert!(
            !normalized_lines
                .windows(2)
                .any(|lines| lines[0].starts_with("return Err(") && lines[0] == lines[1])
        );
        assert!(implementation.contains("bytes != exact_contribution_bytes"));
        assert!(implementation.contains("ZeroizingByteVectorV1::read_exact"));
        assert!(implementation.contains("ZeroizingU64VectorV1::with_capacity_exact"));
        assert!(!normalized_lines.iter().any(|line| {
            line.contains("negacyclic_multiply(")
                && !line.contains("zeroizing_negacyclic_multiply(")
        }));
        assert!(
            implementation
                .matches("finish_indexed_cks_reread_hashes")
                .count()
                >= 5
        );
        let decoder = implementation
            .split("pub(super) fn decode_and_verify_cks_evidence_record_streaming")
            .nth(1)
            .expect("streaming decoder exists");
        let dropped_bytes = decoder
            .find("drop(encoded);")
            .expect("encoded record is dropped");
        let verified = decoder
            .find("verify_indexed_cks_contribution_limbwise")
            .expect("one contribution is verified");
        let dropped_wire = decoder
            .find("drop(wire);")
            .expect("decoded record is dropped");
        assert!(dropped_bytes < verified && verified < dropped_wire);
        let footer = decoder
            .find("finish_canonical_body")
            .expect("footer is checked");
        let eof_checks = decoder
            .match_indices("require_canonical_reader_eof(reader)?;")
            .map(|(position, _)| position)
            .collect::<Vec<_>>();
        assert_eq!(eof_checks.len(), 2);
        let first_eof = eof_checks[0];
        let final_eof = eof_checks[1];
        let context = decoder
            .find("statement.transcript_digest != trusted_context.transcript_digest")
            .expect("compact trusted context is checked");
        let compact = decoder
            .find("compare_indexed_cks_accumulator")
            .expect("compact output is checked exactly");
        let receipt = decoder
            .find("let verification_seal = cks_validated_receipt_seal")
            .expect("receipt is sealed only after final EOF");
        assert!(footer < first_eof && first_eof < context && context < compact);
        assert!(compact < final_eof && final_eof < receipt);
    }
    #[test]
    fn stream_scratch_owners_zeroize_on_success_error_and_unwind() {
        let reset = || CKS_STREAM_ZEROIZING_DROP_AUDIT_V1.with(|audit| audit.set((0, 0)));
        let observed = || CKS_STREAM_ZEROIZING_DROP_AUDIT_V1.with(core::cell::Cell::get);
        reset();
        drop(ZeroizingU64VectorV1::zeroed(4).unwrap());
        assert_eq!(observed(), (1, 0));
        fn fail_after_stream_owner() -> Result<(), ZkAmsMkheErrorV1> {
            let mut owner = ZeroizingU64VectorV1::with_capacity_exact(3)?;
            owner.extend([1, 2, 3]);
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        }
        reset();
        assert_eq!(
            fail_after_stream_owner(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        assert_eq!(observed(), (1, 0));
        reset();
        let unwound = std::panic::catch_unwind(|| {
            let _bytes = ZeroizingByteVectorV1(vec![4, 5, 6]);
            panic!("intentional CKS stream owner unwind");
        });
        assert!(unwound.is_err());
        assert_eq!(observed(), (1, 0));
        let source = include_str!("zeroizing_vectors.rs");
        let black_box = ["core::hint::black_", "box"].concat();
        let fence = ["compiler_", "fence"].concat();
        assert_eq!(source.matches(&black_box).count(), 4);
        assert_eq!(source.matches(&fence).count(), 2);
    }
    #[test]
    fn outer_coordinate_is_bound_to_authenticated_source_indices() {
        let profile = release_profile_v1();
        let digits = u32::try_from(profile.gadget_digits).unwrap();
        assert_eq!(
            validate_cks_outer_coordinate_v1(&profile, 0, 0, 0, 0),
            Ok(())
        );
        assert_eq!(
            validate_cks_outer_coordinate_v1(&profile, 1, 0, digits, u64::from(digits)),
            Ok(())
        );
        assert_eq!(
            validate_cks_outer_coordinate_v1(&profile, 0, 0, 1, 0),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            validate_cks_outer_coordinate_v1(&profile, 0, 0, 0, 1),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            validate_cks_outer_coordinate_v1(
                &profile,
                0,
                u8::try_from(profile.gadget_digits).unwrap(),
                0,
                0,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        assert_eq!(
            validate_cks_outer_coordinate_v1(
                &profile,
                u8::try_from(ZK_AMS_T256_GALOIS_KEY_COUNT_V1 + 1).unwrap(),
                0,
                0,
                0,
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        let source = include_str!("cks_stream.rs");
        let decoder = source
            .split("pub(super) fn decode_and_verify_cks_evidence_record_streaming")
            .nth(1)
            .expect("streaming decoder")
            .split("#[cfg(test)]")
            .next()
            .expect("production decoder");
        let binding = decoder
            .find("validate_cks_outer_coordinate_v1(")
            .expect("outer coordinate binding");
        let first_polynomial = decoder
            .find("index_canonical_cks_polynomial")
            .expect("first indexed polynomial");
        assert!(binding < first_polynomial);
    }
}
