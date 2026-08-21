//! Limb-bounded verification for seekable collective source evidence.
//!
//! This module deliberately does not construct `RnsPolynomial` relation graphs. Canonical public
//! polynomials remain in the caller's immutable seekable provider, every limb reread is checked
//! against two independently domain- separated index digests, and proof responses are the only
//! release-sized owner retained across limbs.
use super::*;
use crate::vega::sponge::Shake256Reader;
const SOURCE_RELEASE_LIMBS_V1: usize = 38;
const INDEXED_SOURCE_NATIVE_LIMB_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest.indexed-limb";
const INDEXED_SOURCE_WIRE_LIMB_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-polynomial.indexed-limb";
/// Seekable, digest-bound location of one canonical release polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct IndexedActiveSourcePolynomialV1 {
    residues_offset: u64,
    native_digest: [u8; 32],
    wire_digest: [u8; 32],
    native_limb_digests: [[u8; 32]; SOURCE_RELEASE_LIMBS_V1],
    wire_limb_digests: [[u8; 32]; SOURCE_RELEASE_LIMBS_V1],
    nonzero: bool,
}
impl IndexedActiveSourcePolynomialV1 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn new(
        residues_offset: u64,
        native_digest: [u8; 32],
        wire_digest: [u8; 32],
        native_limb_digests: [[u8; 32]; SOURCE_RELEASE_LIMBS_V1],
        wire_limb_digests: [[u8; 32]; SOURCE_RELEASE_LIMBS_V1],
        nonzero: bool,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if native_digest == [0; 32]
            || wire_digest == [0; 32]
            || native_limb_digests.contains(&[0; 32])
            || wire_limb_digests.contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            residues_offset,
            native_digest,
            wire_digest,
            native_limb_digests,
            wire_limb_digests,
            nonzero,
        })
    }
    pub(in super::super) const fn native_digest(self) -> [u8; 32] {
        self.native_digest
    }
    pub(in super::super) const fn wire_digest(self) -> [u8; 32] {
        self.wire_digest
    }
}
/// Small indexed form of one of the three source-statement families.
#[allow(
    clippy::large_enum_variant,
    reason = "fixed indexed-source variants remain allocation-free and preserve reviewed protocol field layout"
)]
pub(in super::super) enum IndexedActiveSourceStatementV1 {
    RkgRoundOne {
        public_a: IndexedActiveSourcePolynomialV1,
        party_public_b: IndexedActiveSourcePolynomialV1,
        common_a: IndexedActiveSourcePolynomialV1,
        h0: IndexedActiveSourcePolynomialV1,
        h1: IndexedActiveSourcePolynomialV1,
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        digit_index: u32,
    },
    RkgRoundTwo {
        public_a: IndexedActiveSourcePolynomialV1,
        party_public_b: IndexedActiveSourcePolynomialV1,
        common_a: IndexedActiveSourcePolynomialV1,
        h0: IndexedActiveSourcePolynomialV1,
        h1: IndexedActiveSourcePolynomialV1,
        aggregate_h0: IndexedActiveSourcePolynomialV1,
        aggregate_h1: IndexedActiveSourcePolynomialV1,
        k0: IndexedActiveSourcePolynomialV1,
        left: ZkAmsMkhePartyIdV1,
        right: ZkAmsMkhePartyIdV1,
        digit_index: u32,
    },
    Galois {
        public_a: IndexedActiveSourcePolynomialV1,
        party_public_b: IndexedActiveSourcePolynomialV1,
        source_constant: IndexedActiveSourcePolynomialV1,
        source_linear: IndexedActiveSourcePolynomialV1,
        schedule_index: u8,
        exponent: u32,
        digit_index: u32,
    },
}
impl IndexedActiveSourceStatementV1 {
    pub(in super::super) const fn public_key_polynomials(
        &self,
    ) -> (
        IndexedActiveSourcePolynomialV1,
        IndexedActiveSourcePolynomialV1,
    ) {
        match self {
            Self::RkgRoundOne {
                public_a,
                party_public_b,
                ..
            }
            | Self::RkgRoundTwo {
                public_a,
                party_public_b,
                ..
            }
            | Self::Galois {
                public_a,
                party_public_b,
                ..
            } => (*public_a, *party_public_b),
        }
    }
    pub(in super::super) fn expected_witnesses(&self) -> usize {
        match self {
            Self::RkgRoundOne { .. } | Self::Galois { .. } => 5,
            Self::RkgRoundTwo { .. } => 6,
        }
    }
}
pub(in super::super) fn indexed_source_limb_hashers_v1(
    profile: &super::super::BgvProfile,
    limb: usize,
) -> Result<(Keccak256, Keccak256), ZkAmsMkheErrorV1> {
    profile.validate()?;
    if profile.moduli.len() != SOURCE_RELEASE_LIMBS_V1 || limb >= profile.moduli.len() {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let mut native = Keccak256::new();
    native.update(INDEXED_SOURCE_NATIVE_LIMB_DOMAIN_V1);
    let mut wire = Keccak256::new();
    wire.update(INDEXED_SOURCE_WIRE_LIMB_DOMAIN_V1);
    for hash in [&mut native, &mut wire] {
        hash.update(
            &u32::try_from(profile.ring_degree)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                .to_be_bytes(),
        );
        hash.update(
            &u16::try_from(limb)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                .to_be_bytes(),
        );
        hash.update(&profile.moduli[limb].to_be_bytes());
    }
    Ok((native, wire))
}
struct ZeroizingSourceU64V1(Vec<u64>);
impl ZeroizingSourceU64V1 {
    fn zeroed(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        values.resize(length, 0);
        Ok(Self(values))
    }
    fn with_capacity(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self(values))
    }
}
impl core::ops::Deref for ZeroizingSourceU64V1 {
    type Target = [u64];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for ZeroizingSourceU64V1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ZeroizingSourceU64V1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.0);
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(values);
    }
}
struct ZeroizingSourceI64V1(Vec<i64>);
impl ZeroizingSourceI64V1 {
    fn zeroed(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        values.resize(length, 0);
        Ok(Self(values))
    }
}
impl core::ops::Deref for ZeroizingSourceI64V1 {
    type Target = [i64];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for ZeroizingSourceI64V1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.0);
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(values);
    }
}
struct ZeroizingSourceBoolV1(Vec<bool>);
impl ZeroizingSourceBoolV1 {
    fn zeroed(length: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        values.resize(length, false);
        Ok(Self(values))
    }
}
impl core::ops::Deref for ZeroizingSourceBoolV1 {
    type Target = [bool];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for ZeroizingSourceBoolV1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ZeroizingSourceBoolV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.0);
        values.fill(false);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(values);
    }
}
struct ZeroizingSourceResponsesV1 {
    witness_count: usize,
    ring_degree: usize,
    values: Vec<i64>,
}
impl ZeroizingSourceResponsesV1 {
    fn allocate(witness_count: usize, ring_degree: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let length = witness_count
            .checked_mul(ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(Self {
            witness_count,
            ring_degree,
            values,
        })
    }
    fn push(&mut self, value: i64) {
        self.values.push(value);
    }
    fn response(&self, index: usize) -> Result<&[i64], ZkAmsMkheErrorV1> {
        if index >= self.witness_count {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let start = index
            .checked_mul(self.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(self.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.values
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
    }
}
impl Drop for ZeroizingSourceResponsesV1 {
    fn drop(&mut self) {
        let values = core::hint::black_box(&mut self.values);
        values.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(values);
    }
}
/// Parsed proof state with no retained canonical byte vector.
pub(in super::super) struct ParsedIndexedActiveSourceProofV1 {
    statement_digest: [u8; 32],
    challenge_seed: [u8; 32],
    challenge: ZeroizingSourceI64V1,
    responses: ZeroizingSourceResponsesV1,
    contribution: ZkAmsMkheActiveContributionV1,
}
impl core::fmt::Debug for ParsedIndexedActiveSourceProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ParsedIndexedActiveSourceProofV1")
            .field("statement_digest", &hex::encode(self.statement_digest))
            .field("witness_count", &self.responses.witness_count)
            .finish_non_exhaustive()
    }
}
fn derive_indexed_sparse_challenge_v1(
    ring_degree: usize,
    challenge_seed: [u8; 32],
) -> Result<ZeroizingSourceI64V1, ZkAmsMkheErrorV1> {
    if challenge_seed == [0; 32] || !ring_degree.is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let weight = linear_challenge_weight(ring_degree)?;
    let ring_degree_bytes = u32::try_from(ring_degree)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
        .to_be_bytes();
    let weight_bytes = u32::try_from(weight)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
        .to_be_bytes();
    let domain = b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-sparse-challenge";
    let mut frame = [0_u8; 128];
    let mut cursor = 0_usize;
    for part in [
        domain.as_slice(),
        challenge_seed.as_slice(),
        ring_degree_bytes.as_slice(),
        weight_bytes.as_slice(),
    ] {
        let end = cursor
            .checked_add(part.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let destination = frame
            .get_mut(cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        destination.copy_from_slice(part);
        cursor = end;
    }
    let position_mask = u64::try_from(
        ring_degree
            .checked_sub(1)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut selected_positions = ZeroizingSourceBoolV1::zeroed(ring_degree)?;
    let mut dense = ZeroizingSourceI64V1::zeroed(ring_degree)?;
    let mut stream = Shake256Reader::new(&frame[..cursor]);
    let attempts = weight
        .checked_mul(RANDOM_REJECTION_ATTEMPTS_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut selected = 0_usize;
    for _ in 0..attempts {
        let mut bytes = [0_u8; 8];
        stream.read(&mut bytes);
        let candidate = u64::from_le_bytes(bytes);
        let position = usize::try_from(candidate & position_mask)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
        if selected_positions[position] {
            continue;
        }
        selected_positions[position] = true;
        dense.0[position] = if candidate >> 63 == 0 { -1 } else { 1 };
        selected += 1;
        if selected == weight {
            return Ok(dense);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
}
pub(in super::super) fn decode_indexed_active_source_proof_v1<R: std::io::Read>(
    reader: &mut R,
    encoded_len: u64,
    expected_witnesses: usize,
) -> Result<ParsedIndexedActiveSourceProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    if expected_witnesses == 0 || expected_witnesses > RKG_LINEAR_PROOF_MAX_WITNESSES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let linear_bytes = linear_proof_wire_bytes(expected_witnesses, profile.ring_degree)?;
    let exact_bytes = ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1
        .checked_add(linear_bytes)
        .and_then(|bytes| bytes.checked_add(ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if usize::try_from(encoded_len).ok() != Some(exact_bytes) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut evidence_header = [0_u8; ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1];
    read_active_evidence_io_exact(reader, &mut evidence_header)?;
    if evidence_header[..4] != ACTIVE_RKG_EVIDENCE_TAG_V1
        || evidence_header[4] != MKHE_VERSION_V1
        || usize::from(evidence_header[37]) != expected_witnesses
        || usize::try_from(u32::from_be_bytes(
            evidence_header[38..42]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        ))
        .ok()
            != Some(linear_bytes)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let statement_digest = evidence_header[5..37]
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if statement_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut proof_header = [0_u8; RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_V1];
    read_active_evidence_io_exact(reader, &mut proof_header)?;
    if proof_header[..4] != RKG_LINEAR_PROOF_WIRE_TAG_V1
        || proof_header[4] != MKHE_VERSION_V1
        || usize::from(proof_header[37]) != expected_witnesses
        || usize::try_from(u32::from_be_bytes(
            proof_header[38..42]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        ))
        .ok()
            != Some(profile.ring_degree)
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let challenge_seed = proof_header[5..37]
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if challenge_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    // Allocate the public challenge before the much larger response owner so
    // an allocation failure cannot strand partially decoded proof material.
    let challenge = derive_indexed_sparse_challenge_v1(profile.ring_degree, challenge_seed)?;
    let mut responses =
        ZeroizingSourceResponsesV1::allocate(expected_witnesses, profile.ring_degree)?;
    const RESPONSE_BATCH_BYTES: usize = 8 * 1024;
    let mut buffer = [0_u8; RESPONSE_BATCH_BYTES];
    let response_bytes = expected_witnesses
        .checked_mul(profile.ring_degree)
        .and_then(|count| count.checked_mul(8))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut remaining = response_bytes;
    while remaining != 0 {
        let take = remaining.min(buffer.len());
        read_active_evidence_io_exact(reader, &mut buffer[..take])?;
        for encoded in buffer[..take].chunks_exact(8) {
            responses.push(i64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            ));
        }
        remaining -= take;
    }
    if responses.values.len() != expected_witnesses * profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut contribution_bytes = [0_u8; ACTIVE_RKG_EVIDENCE_CONTRIBUTION_BYTES_V1];
    read_active_evidence_io_exact(reader, &mut contribution_bytes)?;
    let contribution = decode_active_evidence_contribution_exact(&contribution_bytes)?;
    Ok(ParsedIndexedActiveSourceProofV1 {
        statement_digest,
        challenge_seed,
        challenge,
        responses,
        contribution,
    })
}
fn read_indexed_source_limb<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    indexed: IndexedActiveSourcePolynomialV1,
    profile: &super::super::BgvProfile,
    limb: usize,
) -> Result<ZeroizingSourceU64V1, ZkAmsMkheErrorV1> {
    let offset = u64::try_from(limb)
        .ok()
        .and_then(|limb| limb.checked_mul(profile.ring_degree as u64))
        .and_then(|residue| residue.checked_mul(8))
        .and_then(|bytes| indexed.residues_offset.checked_add(bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    reader
        .seek(std::io::SeekFrom::Start(offset))
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let modulus = *profile
        .moduli
        .get(limb)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let (mut native, mut wire) = indexed_source_limb_hashers_v1(profile, limb)?;
    let mut residues = ZeroizingSourceU64V1::with_capacity(profile.ring_degree)?;
    let mut buffer = [0_u8; 8 * 1024];
    while residues.0.len() < profile.ring_degree {
        let take_residues = (profile.ring_degree - residues.0.len()).min(buffer.len() / 8);
        let take_bytes = take_residues * 8;
        reader
            .read_exact(&mut buffer[..take_bytes])
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        native.update(&buffer[..take_bytes]);
        wire.update(&buffer[..take_bytes]);
        for encoded in buffer[..take_bytes].chunks_exact(8) {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            );
            if residue >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            residues.0.push(residue);
        }
    }
    if native.finalize() != indexed.native_limb_digests[limb]
        || wire.finalize() != indexed.wire_limb_digests[limb]
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(residues)
}
fn derive_rkg_common_a_limb_v1(
    profile: &super::super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
    digit: usize,
    limb: usize,
) -> Result<ZeroizingSourceU64V1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let party_set = active_party_set(roster)?;
    if transcript_digest == [0; 32]
        || left > right
        || party_set.index_of(left).is_none()
        || party_set.index_of(right).is_none()
        || digit >= profile.gadget_digits
        || limb >= profile.moduli.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut context = Vec::with_capacity(130);
    context.extend_from_slice(&party_set.digest);
    context.extend_from_slice(&transcript_digest);
    context.extend_from_slice(&left.to_bytes());
    context.extend_from_slice(&right.to_bytes());
    context.extend_from_slice(
        &u16::try_from(digit)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    let domain = b"iroha.zk-ams.v1.mkhe.rkg-common-a";
    let maximum_xof_bytes = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .and_then(|value| value.checked_mul(super::super::MAX_RANDOM_REJECTION_ATTEMPTS_V1))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::super::checked_rng_bytes(profile, maximum_xof_bytes)?;
    let mut frame = Vec::with_capacity(domain.len() + context.len() + 48);
    frame.extend_from_slice(domain);
    frame.extend_from_slice(&profile.digest()?);
    frame.extend_from_slice(
        &u32::try_from(context.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&context);
    frame.extend_from_slice(
        &u16::try_from(limb)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    let modulus = profile.moduli[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    let mut stream = Shake256Reader::new(&frame);
    let mut output = ZeroizingSourceU64V1::with_capacity(profile.ring_degree)?;
    for _ in 0..profile.ring_degree {
        let mut accepted = None;
        for _ in 0..super::super::MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
            let mut bytes = [0_u8; 8];
            stream.read(&mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < zone {
                accepted = Some(candidate % modulus);
                break;
            }
        }
        output
            .0
            .push(accepted.ok_or(ZkAmsMkheErrorV1::InvalidProfile)?);
    }
    Ok(output)
}
#[allow(
    clippy::too_many_arguments,
    reason = "fixed protocol axes remain explicit to preserve reviewed binding order"
)]
fn validate_indexed_common_a<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    profile: &super::super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    indexed: IndexedActiveSourcePolynomialV1,
    left: ZkAmsMkhePartyIdV1,
    right: ZkAmsMkhePartyIdV1,
    digit: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    for limb in 0..profile.moduli.len() {
        let observed = read_indexed_source_limb(reader, indexed, profile, limb)?;
        let expected = derive_rkg_common_a_limb_v1(
            profile,
            roster,
            transcript_digest,
            left,
            right,
            digit,
            limb,
        )?;
        if *observed != *expected {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}
fn update_rns_hash_header(
    hash: &mut Keccak256,
    profile: &super::super::BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    hash.update(
        &u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(
        &u32::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    Ok(())
}
fn update_indexed_rns_hash<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    hash: &mut Keccak256,
    profile: &super::super::BgvProfile,
    indexed: IndexedActiveSourcePolynomialV1,
    negate: bool,
) -> Result<(), ZkAmsMkheErrorV1> {
    update_rns_hash_header(hash, profile)?;
    for limb in 0..profile.moduli.len() {
        let values = read_indexed_source_limb(reader, indexed, profile, limb)?;
        let modulus = profile.moduli[limb];
        hash.update(&(limb as u32).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
        for value in values.iter() {
            let value = if negate && *value != 0 {
                modulus - *value
            } else {
                *value
            };
            hash.update(&value.to_be_bytes());
        }
    }
    Ok(())
}
fn indexed_difference_nonzero<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    profile: &super::super::BgvProfile,
    positive: Option<IndexedActiveSourcePolynomialV1>,
    negative: IndexedActiveSourcePolynomialV1,
) -> Result<bool, ZkAmsMkheErrorV1> {
    let mut nonzero = false;
    for limb in 0..profile.moduli.len() {
        let negative_values = read_indexed_source_limb(reader, negative, profile, limb)?;
        let positive_values = positive
            .map(|indexed| read_indexed_source_limb(reader, indexed, profile, limb))
            .transpose()?;
        let modulus = profile.moduli[limb];
        for (index, negative) in negative_values.iter().enumerate() {
            let positive = positive_values.as_ref().map_or(0, |values| values[index]);
            nonzero |= super::super::mod_sub(positive, *negative, modulus) != 0;
        }
    }
    Ok(nonzero)
}
fn update_indexed_difference_rns_hash<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    hash: &mut Keccak256,
    profile: &super::super::BgvProfile,
    positive: Option<IndexedActiveSourcePolynomialV1>,
    negative: IndexedActiveSourcePolynomialV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    update_rns_hash_header(hash, profile)?;
    for limb in 0..profile.moduli.len() {
        let negative_values = read_indexed_source_limb(reader, negative, profile, limb)?;
        let positive_values = positive
            .map(|indexed| read_indexed_source_limb(reader, indexed, profile, limb))
            .transpose()?;
        let modulus = profile.moduli[limb];
        hash.update(&(limb as u32).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
        for (index, negative) in negative_values.iter().enumerate() {
            let positive = positive_values.as_ref().map_or(0, |values| values[index]);
            hash.update(&super::super::mod_sub(positive, *negative, modulus).to_be_bytes());
        }
    }
    Ok(())
}
fn update_statement_prefix(
    hash: &mut Keccak256,
    profile: &super::super::BgvProfile,
    bounds: &[i64],
    exponents: &[usize],
    outputs: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-statement");
    hash.update(&profile.digest()?);
    hash.update(
        &u32::try_from(bounds.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    for (index, (bound, exponent)) in bounds.iter().zip(exponents).enumerate() {
        hash.update(&(index as u32).to_be_bytes());
        hash.update(&bound.to_be_bytes());
        hash.update(
            &u32::try_from(*exponent)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                .to_be_bytes(),
        );
    }
    hash.update(
        &u32::try_from(outputs)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    Ok(())
}
fn update_term_prefix(
    hash: &mut Keccak256,
    witness_index: usize,
    automorphism_exponent: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    hash.update(
        &u32::try_from(witness_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    hash.update(
        &u32::try_from(automorphism_exponent)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    Ok(())
}
fn update_collective_public_key_statement_hash<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    hash: &mut Keccak256,
    profile: &super::super::BgvProfile,
    public_a: IndexedActiveSourcePolynomialV1,
    party_public_b: IndexedActiveSourcePolynomialV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if !public_a.nonzero {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    hash.update(&0_u32.to_be_bytes());
    hash.update(&1_u32.to_be_bytes());
    update_indexed_rns_hash(reader, hash, profile, party_public_b, false)?;
    hash.update(&2_u32.to_be_bytes());
    update_term_prefix(hash, 0, 1)?;
    update_indexed_rns_hash(reader, hash, profile, public_a, true)?;
    update_term_prefix(hash, 1, 1)?;
    update_scaled_identity_rns_hash_v1(hash, profile, None)
}
fn indexed_source_statement_digest<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    profile: &super::super::BgvProfile,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    party_index: usize,
    statement: &IndexedActiveSourceStatementV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let party = roster.participant(party_index)?.party;
    let eta = i64::from(profile.error_eta);
    let mut hash = Keccak256::new();
    match statement {
        IndexedActiveSourceStatementV1::RkgRoundOne {
            public_a,
            party_public_b,
            common_a,
            h0,
            h1,
            left,
            right,
            digit_index,
        } => {
            update_statement_prefix(&mut hash, profile, &[1, eta, 1, eta, eta], &[1; 5], 3)?;
            update_collective_public_key_statement_hash(
                reader,
                &mut hash,
                profile,
                *public_a,
                *party_public_b,
            )?;
            hash.update(&1_u32.to_be_bytes());
            hash.update(&1_u32.to_be_bytes());
            update_indexed_rns_hash(reader, &mut hash, profile, *h0, false)?;
            let h0_terms = usize::from(party == *left) + 2;
            hash.update(&(h0_terms as u32).to_be_bytes());
            if party == *left {
                update_term_prefix(&mut hash, 0, 1)?;
                update_scaled_identity_rns_hash_v1(
                    &mut hash,
                    profile,
                    Some(
                        usize::try_from(*digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    ),
                )?;
            }
            update_term_prefix(&mut hash, 2, 1)?;
            update_indexed_rns_hash(reader, &mut hash, profile, *common_a, true)?;
            update_term_prefix(&mut hash, 3, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
            hash.update(&2_u32.to_be_bytes());
            hash.update(&1_u32.to_be_bytes());
            update_indexed_rns_hash(reader, &mut hash, profile, *h1, false)?;
            let h1_terms = usize::from(party == *right) + 1;
            hash.update(&(h1_terms as u32).to_be_bytes());
            if party == *right {
                update_term_prefix(&mut hash, 0, 1)?;
                update_indexed_rns_hash(reader, &mut hash, profile, *common_a, false)?;
            }
            update_term_prefix(&mut hash, 4, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
        }
        IndexedActiveSourceStatementV1::RkgRoundTwo {
            public_a,
            party_public_b,
            common_a,
            h0,
            h1,
            aggregate_h0,
            aggregate_h1,
            k0,
            left,
            right,
            digit_index,
        } => {
            update_statement_prefix(&mut hash, profile, &[1, eta, 1, eta, eta, eta], &[1; 6], 4)?;
            update_collective_public_key_statement_hash(
                reader,
                &mut hash,
                profile,
                *public_a,
                *party_public_b,
            )?;
            hash.update(&1_u32.to_be_bytes());
            hash.update(&1_u32.to_be_bytes());
            update_indexed_rns_hash(reader, &mut hash, profile, *h0, false)?;
            let h0_terms = usize::from(party == *left) + 2;
            hash.update(&(h0_terms as u32).to_be_bytes());
            if party == *left {
                update_term_prefix(&mut hash, 0, 1)?;
                update_scaled_identity_rns_hash_v1(
                    &mut hash,
                    profile,
                    Some(
                        usize::try_from(*digit_index)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                    ),
                )?;
            }
            update_term_prefix(&mut hash, 2, 1)?;
            update_indexed_rns_hash(reader, &mut hash, profile, *common_a, true)?;
            update_term_prefix(&mut hash, 3, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
            hash.update(&2_u32.to_be_bytes());
            hash.update(&1_u32.to_be_bytes());
            update_indexed_rns_hash(reader, &mut hash, profile, *h1, false)?;
            let h1_terms = usize::from(party == *right) + 1;
            hash.update(&(h1_terms as u32).to_be_bytes());
            if party == *right {
                update_term_prefix(&mut hash, 0, 1)?;
                update_indexed_rns_hash(reader, &mut hash, profile, *common_a, false)?;
            }
            update_term_prefix(&mut hash, 4, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
            let positive = (party == *right).then_some(*aggregate_h0);
            let secret_multiplier_nonzero =
                indexed_difference_nonzero(reader, profile, positive, *aggregate_h1)?;
            hash.update(&3_u32.to_be_bytes());
            hash.update(&1_u32.to_be_bytes());
            update_indexed_rns_hash(reader, &mut hash, profile, *k0, false)?;
            let k0_terms =
                usize::from(secret_multiplier_nonzero) + usize::from(aggregate_h1.nonzero) + 1;
            hash.update(&(k0_terms as u32).to_be_bytes());
            if secret_multiplier_nonzero {
                update_term_prefix(&mut hash, 0, 1)?;
                update_indexed_difference_rns_hash(
                    reader,
                    &mut hash,
                    profile,
                    positive,
                    *aggregate_h1,
                )?;
            }
            if aggregate_h1.nonzero {
                update_term_prefix(&mut hash, 2, 1)?;
                update_indexed_rns_hash(reader, &mut hash, profile, *aggregate_h1, false)?;
            }
            update_term_prefix(&mut hash, 5, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
        }
        IndexedActiveSourceStatementV1::Galois {
            public_a,
            party_public_b,
            source_constant,
            source_linear,
            exponent,
            digit_index,
            ..
        } => {
            let exponent =
                usize::try_from(*exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if !party_public_b.nonzero {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            update_statement_prefix(
                &mut hash,
                profile,
                &[1, eta, 1, eta, eta],
                &[1, 1, exponent, exponent, exponent],
                3,
            )?;
            update_collective_public_key_statement_hash(
                reader,
                &mut hash,
                profile,
                *public_a,
                *party_public_b,
            )?;
            hash.update(&1_u32.to_be_bytes());
            hash.update(
                &u32::try_from(exponent)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .to_be_bytes(),
            );
            update_indexed_rns_hash(reader, &mut hash, profile, *source_constant, false)?;
            hash.update(&3_u32.to_be_bytes());
            update_term_prefix(&mut hash, 0, exponent)?;
            update_scaled_identity_rns_hash_v1(
                &mut hash,
                profile,
                Some(
                    usize::try_from(*digit_index)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                ),
            )?;
            update_term_prefix(&mut hash, 2, 1)?;
            update_indexed_rns_hash(reader, &mut hash, profile, *party_public_b, false)?;
            update_term_prefix(&mut hash, 3, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
            hash.update(&2_u32.to_be_bytes());
            hash.update(
                &u32::try_from(exponent)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
                    .to_be_bytes(),
            );
            update_indexed_rns_hash(reader, &mut hash, profile, *source_linear, false)?;
            hash.update(&2_u32.to_be_bytes());
            update_term_prefix(&mut hash, 2, 1)?;
            update_indexed_rns_hash(reader, &mut hash, profile, *public_a, false)?;
            update_term_prefix(&mut hash, 4, 1)?;
            update_scaled_identity_rns_hash_v1(&mut hash, profile, None)?;
        }
    }
    Ok(hash.finalize())
}
fn zeroizing_negacyclic_multiply_signed_v1(
    left: &[u64],
    right: &[i64],
    modulus: u64,
    root: u64,
) -> Result<ZeroizingSourceU64V1, ZkAmsMkheErrorV1> {
    if left.len() != right.len() || left.is_empty() || !left.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut left_twisted = ZeroizingSourceU64V1::with_capacity(left.len())?;
    let mut right_twisted = ZeroizingSourceU64V1::with_capacity(right.len())?;
    let mut twist = 1_u64;
    for (&left, &right) in left.iter().zip(right) {
        left_twisted
            .0
            .push(super::super::mod_mul(left, twist, modulus));
        right_twisted.0.push(super::super::mod_mul(
            super::super::signed_mod(right, modulus),
            twist,
            modulus,
        ));
        twist = super::super::mod_mul(twist, root, modulus);
    }
    let cyclic_root = super::super::mod_mul(root, root, modulus);
    super::super::cyclic_ntt(&mut left_twisted, cyclic_root, modulus);
    super::super::cyclic_ntt(&mut right_twisted, cyclic_root, modulus);
    for (left, right) in left_twisted.iter_mut().zip(right_twisted.iter()) {
        *left = super::super::mod_mul(*left, *right, modulus);
    }
    super::super::inverse_cyclic_ntt(&mut left_twisted, cyclic_root, modulus)?;
    let inverse_root =
        super::super::mod_inverse(root, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for value in left_twisted.iter_mut() {
        *value = super::super::mod_mul(*value, untwist, modulus);
        untwist = super::super::mod_mul(untwist, inverse_root, modulus);
    }
    Ok(left_twisted)
}
fn add_product(
    output: &mut [u64],
    product: &[u64],
    modulus: u64,
    subtract: bool,
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.len() != product.len() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (output, product) in output.iter_mut().zip(product) {
        *output = if subtract {
            super::super::mod_sub(*output, *product, modulus)
        } else {
            super::super::mod_add(*output, *product, modulus)
        };
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum LimbMultiplierV1<'a> {
    Indexed(&'a IndexedActiveSourcePolynomialV1, bool),
    Difference {
        positive: Option<&'a IndexedActiveSourcePolynomialV1>,
        negative: &'a IndexedActiveSourcePolynomialV1,
    },
    ScaledIdentity(Option<usize>),
}
#[derive(Clone, Copy)]
struct SourceRelationTermV1<'a> {
    witness_index: usize,
    witness_automorphism_exponent: usize,
    multiplier: LimbMultiplierV1<'a>,
}
fn transformed_signed(
    values: &[i64],
    exponent: usize,
) -> Result<Option<ZeroizingSourceI64V1>, ZkAmsMkheErrorV1> {
    if exponent == 1 {
        return Ok(None);
    }
    if values.is_empty() || !values.len().is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let twice_degree = values
        .len()
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    if exponent == 0 || exponent >= twice_degree || exponent.is_multiple_of(2) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut output = ZeroizingSourceI64V1::zeroed(values.len())?;
    for (index, coefficient) in values.iter().copied().enumerate() {
        let mapped = index
            .checked_mul(exponent)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            % twice_degree;
        let (destination, sign) = if mapped >= values.len() {
            (mapped - values.len(), -1_i64)
        } else {
            (mapped, 1_i64)
        };
        output.0[destination] = coefficient
            .checked_mul(sign)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    Ok(Some(output))
}
fn read_limb_multiplier<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    multiplier: &LimbMultiplierV1<'_>,
    profile: &super::super::BgvProfile,
    limb: usize,
) -> Result<Option<ZeroizingSourceU64V1>, ZkAmsMkheErrorV1> {
    match multiplier {
        LimbMultiplierV1::Indexed(indexed, negate) => {
            let mut values = read_indexed_source_limb(reader, **indexed, profile, limb)?;
            if *negate {
                let modulus = profile.moduli[limb];
                for value in values.iter_mut() {
                    *value = if *value == 0 { 0 } else { modulus - *value };
                }
            }
            Ok(Some(values))
        }
        LimbMultiplierV1::Difference { positive, negative } => {
            let mut values = read_indexed_source_limb(reader, **negative, profile, limb)?;
            let positive_values = positive
                .map(|indexed| read_indexed_source_limb(reader, *indexed, profile, limb))
                .transpose()?;
            let modulus = profile.moduli[limb];
            for index in 0..values.len() {
                let positive = positive_values.as_ref().map_or(0, |owner| owner[index]);
                values[index] = super::super::mod_sub(positive, values[index], modulus);
            }
            Ok(Some(values))
        }
        LimbMultiplierV1::ScaledIdentity(_) => Ok(None),
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "fixed protocol axes remain explicit to preserve reviewed relation order"
)]
fn verify_relation_output<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    hash: &mut Keccak256,
    output_index: usize,
    output_challenge_exponent: usize,
    target: IndexedActiveSourcePolynomialV1,
    terms: &[Option<SourceRelationTermV1<'_>>; 3],
    profile: &super::super::BgvProfile,
    proof: &ParsedIndexedActiveSourceProofV1,
    challenge: &[i64],
) -> Result<(), ZkAmsMkheErrorV1> {
    hash.update(
        &u32::try_from(output_index)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?
            .to_be_bytes(),
    );
    update_rns_hash_header(hash, profile)?;
    let transformed_challenge = transformed_signed(challenge, output_challenge_exponent)?;
    let challenge = transformed_challenge
        .as_ref()
        .map_or(challenge, |owner| &**owner);
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let root = profile.negacyclic_roots[limb];
        let mut commitment = ZeroizingSourceU64V1::zeroed(profile.ring_degree)?;
        for term in terms.iter().flatten() {
            let response = proof.responses.response(term.witness_index)?;
            let transformed = transformed_signed(response, term.witness_automorphism_exponent)?;
            let response = transformed.as_ref().map_or(response, |owner| &**owner);
            match term.multiplier {
                LimbMultiplierV1::ScaledIdentity(digit) => {
                    let scalar = match digit {
                        Some(digit) => super::super::mod_pow(
                            super::super::mod_pow(2, u64::from(profile.gadget_base_log), modulus),
                            u64::try_from(digit)
                                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
                            modulus,
                        ),
                        None => profile.plaintext_modulus.residue(modulus),
                    };
                    for (output, response) in commitment.iter_mut().zip(response) {
                        *output = super::super::mod_add(
                            *output,
                            super::super::mod_mul(
                                super::super::signed_mod(*response, modulus),
                                scalar,
                                modulus,
                            ),
                            modulus,
                        );
                    }
                }
                _ => {
                    let multiplier = read_limb_multiplier(reader, &term.multiplier, profile, limb)?
                        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
                    let product = zeroizing_negacyclic_multiply_signed_v1(
                        &multiplier,
                        response,
                        modulus,
                        root,
                    )?;
                    add_product(&mut commitment, &product, modulus, false)?;
                }
            }
        }
        let target = read_indexed_source_limb(reader, target, profile, limb)?;
        let target_product =
            zeroizing_negacyclic_multiply_signed_v1(&target, challenge, modulus, root)?;
        add_product(&mut commitment, &target_product, modulus, true)?;
        hash.update(&(limb as u32).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
        for value in commitment.iter() {
            hash.update(&value.to_be_bytes());
        }
    }
    Ok(())
}
fn verify_collective_public_key_output<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    hash: &mut Keccak256,
    public_a: IndexedActiveSourcePolynomialV1,
    party_public_b: IndexedActiveSourcePolynomialV1,
    profile: &super::super::BgvProfile,
    proof: &ParsedIndexedActiveSourceProofV1,
    challenge: &[i64],
) -> Result<(), ZkAmsMkheErrorV1> {
    hash.update(&0_u32.to_be_bytes());
    update_rns_hash_header(hash, profile)?;
    let secret = proof.responses.response(0)?;
    let error = proof.responses.response(1)?;
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let root = profile.negacyclic_roots[limb];
        let public_a = read_indexed_source_limb(reader, public_a, profile, limb)?;
        let mut commitment =
            zeroizing_negacyclic_multiply_signed_v1(&public_a, secret, modulus, root)?;
        for value in commitment.iter_mut() {
            *value = if *value == 0 { 0 } else { modulus - *value };
        }
        let plaintext = profile.plaintext_modulus.residue(modulus);
        for (value, error) in commitment.iter_mut().zip(error) {
            *value = super::super::mod_add(
                *value,
                super::super::mod_mul(
                    plaintext,
                    super::super::signed_mod(*error, modulus),
                    modulus,
                ),
                modulus,
            );
        }
        let party_b = read_indexed_source_limb(reader, party_public_b, profile, limb)?;
        let party_b_challenge =
            zeroizing_negacyclic_multiply_signed_v1(&party_b, challenge, modulus, root)?;
        add_product(&mut commitment, &party_b_challenge, modulus, true)?;
        hash.update(&(limb as u32).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
        for value in commitment.iter() {
            hash.update(&value.to_be_bytes());
        }
    }
    Ok(())
}
fn proof_payload_digest(
    profile: &super::super::BgvProfile,
    context: LinearProofContextV1,
    statement_digest: [u8; 32],
    proof: &ParsedIndexedActiveSourceProofV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-relation-proof");
    hash.update(&linear_context_digest(profile, context)?);
    hash.update(&statement_digest);
    hash.update(&RKG_LINEAR_PROOF_WIRE_TAG_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&proof.challenge_seed);
    hash.update(&[u8::try_from(proof.responses.witness_count)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?]);
    hash.update(
        &u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    for value in &proof.responses.values {
        hash.update(&value.to_be_bytes());
    }
    Ok(hash.finalize())
}
/// Fixed-size result retained by the outer receipt seal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct VerifiedIndexedActiveSourceProofV1 {
    pub(in super::super) statement_digest: [u8; 32],
    pub(in super::super) payload_digest: [u8; 32],
    pub(in super::super) contribution_digest: [u8; 32],
}
pub(in super::super) fn verify_indexed_active_source_proof_v1<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    statement: &IndexedActiveSourceStatementV1,
    proof: &ParsedIndexedActiveSourceProofV1,
) -> Result<VerifiedIndexedActiveSourceProofV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    roster.validate()?;
    if proof.responses.witness_count != statement.expected_witnesses()
        || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    if let IndexedActiveSourceStatementV1::RkgRoundOne {
        common_a,
        left,
        right,
        digit_index,
        ..
    }
    | IndexedActiveSourceStatementV1::RkgRoundTwo {
        common_a,
        left,
        right,
        digit_index,
        ..
    } = statement
    {
        if !common_a.nonzero {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_indexed_common_a(
            reader,
            &profile,
            roster,
            transcript_digest,
            *common_a,
            *left,
            *right,
            usize::try_from(*digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        )?;
    }
    let (context, round) = match statement {
        IndexedActiveSourceStatementV1::RkgRoundOne {
            left,
            right,
            digit_index,
            ..
        }
        | IndexedActiveSourceStatementV1::RkgRoundTwo {
            left,
            right,
            digit_index,
            ..
        } => {
            let pair_index = canonical_rkg_pair_index(roster, *left, *right)?;
            let active_record_index = pair_index
                .checked_mul(
                    u32::try_from(profile.gadget_digits)
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                )
                .and_then(|base| base.checked_add(*digit_index))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let round = if matches!(
                statement,
                IndexedActiveSourceStatementV1::RkgRoundOne { .. }
            ) {
                ZkAmsMkheActiveRoundV1::RkgRoundOne
            } else {
                ZkAmsMkheActiveRoundV1::RkgRoundTwo
            };
            (
                active_linear_context(
                    &profile,
                    roster,
                    transcript_digest,
                    round,
                    party_index,
                    active_record_index,
                    pair_index,
                )?,
                round,
            )
        }
        IndexedActiveSourceStatementV1::Galois {
            schedule_index,
            exponent,
            digit_index,
            ..
        } => {
            validate_galois_source_coordinate(
                usize::from(*schedule_index),
                *exponent,
                usize::try_from(*digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            )?;
            let active_record_index = usize::from(*schedule_index)
                .checked_mul(profile.gadget_digits)
                .and_then(|base| base.checked_add(usize::try_from(*digit_index).ok()?))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            (
                active_linear_context(
                    &profile,
                    roster,
                    transcript_digest,
                    ZkAmsMkheActiveRoundV1::GaloisSource,
                    party_index,
                    active_record_index,
                    *exponent,
                )?,
                ZkAmsMkheActiveRoundV1::GaloisSource,
            )
        }
    };
    let statement_digest =
        indexed_source_statement_digest(reader, &profile, roster, party_index, statement)?;
    if proof.statement_digest != statement_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let challenge_weight = linear_challenge_weight(profile.ring_degree)?;
    let eta = i64::from(profile.error_eta);
    let round_two_bounds = [1, eta, 1, eta, eta, eta];
    let other_bounds = [1, eta, 1, eta, eta];
    let bounds: &[i64] = match statement {
        IndexedActiveSourceStatementV1::RkgRoundTwo { .. } => &round_two_bounds,
        _ => &other_bounds,
    };
    for (index, bound) in bounds.iter().enumerate() {
        validate_linear_response_coefficients(
            proof.responses.response(index)?,
            profile.ring_degree,
            *bound,
            challenge_weight,
        )?;
    }
    let challenge: &[i64] = &proof.challenge;
    let (public_a, party_public_b) = statement.public_key_polynomials();
    let mut commitment_hash = Keccak256::new();
    commitment_hash.update(b"iroha.zk-ams.v1.mkhe.rkg-linear-proof-fiat-shamir");
    commitment_hash.update(&linear_context_digest(&profile, context)?);
    commitment_hash.update(&statement_digest);
    let output_count = match statement {
        IndexedActiveSourceStatementV1::RkgRoundTwo { .. } => 4_u32,
        _ => 3_u32,
    };
    commitment_hash.update(&output_count.to_be_bytes());
    verify_collective_public_key_output(
        reader,
        &mut commitment_hash,
        public_a,
        party_public_b,
        &profile,
        proof,
        challenge,
    )?;
    match statement {
        IndexedActiveSourceStatementV1::RkgRoundOne {
            common_a,
            h0,
            h1,
            left,
            right,
            digit_index,
            ..
        }
        | IndexedActiveSourceStatementV1::RkgRoundTwo {
            common_a,
            h0,
            h1,
            left,
            right,
            digit_index,
            ..
        } => {
            let party = roster.participant(party_index)?.party;
            let gadget =
                usize::try_from(*digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            let h0_terms = [
                (party == *left).then_some(SourceRelationTermV1 {
                    witness_index: 0,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::ScaledIdentity(Some(gadget)),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 2,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::Indexed(common_a, true),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 3,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::ScaledIdentity(None),
                }),
            ];
            verify_relation_output(
                reader,
                &mut commitment_hash,
                1,
                1,
                *h0,
                &h0_terms,
                &profile,
                proof,
                challenge,
            )?;
            let h1_terms = [
                (party == *right).then_some(SourceRelationTermV1 {
                    witness_index: 0,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::Indexed(common_a, false),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 4,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::ScaledIdentity(None),
                }),
                None,
            ];
            verify_relation_output(
                reader,
                &mut commitment_hash,
                2,
                1,
                *h1,
                &h1_terms,
                &profile,
                proof,
                challenge,
            )?;
            if let IndexedActiveSourceStatementV1::RkgRoundTwo {
                aggregate_h0,
                aggregate_h1,
                k0,
                ..
            } = statement
            {
                let positive = (party == *right).then_some(aggregate_h0);
                let secret_multiplier_nonzero =
                    indexed_difference_nonzero(reader, &profile, positive.copied(), *aggregate_h1)?;
                let k0_terms = [
                    secret_multiplier_nonzero.then_some(SourceRelationTermV1 {
                        witness_index: 0,
                        witness_automorphism_exponent: 1,
                        multiplier: LimbMultiplierV1::Difference {
                            positive,
                            negative: aggregate_h1,
                        },
                    }),
                    aggregate_h1.nonzero.then_some(SourceRelationTermV1 {
                        witness_index: 2,
                        witness_automorphism_exponent: 1,
                        multiplier: LimbMultiplierV1::Indexed(aggregate_h1, false),
                    }),
                    Some(SourceRelationTermV1 {
                        witness_index: 5,
                        witness_automorphism_exponent: 1,
                        multiplier: LimbMultiplierV1::ScaledIdentity(None),
                    }),
                ];
                verify_relation_output(
                    reader,
                    &mut commitment_hash,
                    3,
                    1,
                    *k0,
                    &k0_terms,
                    &profile,
                    proof,
                    challenge,
                )?;
            }
        }
        IndexedActiveSourceStatementV1::Galois {
            source_constant,
            source_linear,
            exponent,
            digit_index,
            ..
        } => {
            let exponent =
                usize::try_from(*exponent).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            let digit =
                usize::try_from(*digit_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            let constant_terms = [
                Some(SourceRelationTermV1 {
                    witness_index: 0,
                    witness_automorphism_exponent: exponent,
                    multiplier: LimbMultiplierV1::ScaledIdentity(Some(digit)),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 2,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::Indexed(&party_public_b, false),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 3,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::ScaledIdentity(None),
                }),
            ];
            verify_relation_output(
                reader,
                &mut commitment_hash,
                1,
                exponent,
                *source_constant,
                &constant_terms,
                &profile,
                proof,
                challenge,
            )?;
            let linear_terms = [
                Some(SourceRelationTermV1 {
                    witness_index: 2,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::Indexed(&public_a, false),
                }),
                Some(SourceRelationTermV1 {
                    witness_index: 4,
                    witness_automorphism_exponent: 1,
                    multiplier: LimbMultiplierV1::ScaledIdentity(None),
                }),
                None,
            ];
            verify_relation_output(
                reader,
                &mut commitment_hash,
                2,
                exponent,
                *source_linear,
                &linear_terms,
                &profile,
                proof,
                challenge,
            )?;
        }
    }
    if commitment_hash.finalize() != proof.challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let payload_digest = proof_payload_digest(&profile, context, statement_digest, proof)?;
    if proof.contribution.payload_digest != payload_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_active_contribution(
        roster,
        transcript_digest,
        round,
        party_index,
        &proof.contribution,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidAuthentication)?;
    Ok(VerifiedIndexedActiveSourceProofV1 {
        statement_digest,
        payload_digest,
        contribution_digest: proof.contribution.digest()?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_major_heap_equation_excludes_full_rns_owner() {
        const LIMB_BYTES: usize = 131_072 * 8;
        const RESPONSE_BYTES: usize = 6 * LIMB_BYTES;
        const MAJOR_HEAP_BYTES: usize = RESPONSE_BYTES + 6 * LIMB_BYTES + 8 * 1024;
        assert_eq!(MAJOR_HEAP_BYTES, 12 * 1024 * 1024 + 8 * 1024);
        const { assert!(MAJOR_HEAP_BYTES < 13 * 1024 * 1024) };
        // Each descriptor retains 2 * 38 limb digests plus two overall
        // digests and an offset. Even the eight-polynomial statement stays in
        // fixed tens of KiB and owns no residue allocation.
        assert!(core::mem::size_of::<IndexedActiveSourcePolynomialV1>() <= 2_512);
        assert!(core::mem::size_of::<IndexedActiveSourceStatementV1>() <= 21 * 1024);
    }
    #[test]
    fn production_source_has_no_full_rns_relation_builder() {
        let source = include_str!("source_stream.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(!production.contains("release_wire_polynomial("));
        assert!(!production.contains("super::RnsPolynomial"));
        assert!(!production.contains("proof_bytes: Vec<u8>"));
        assert!(production.contains("read_indexed_source_limb"));
        assert!(production.contains("native_limb_digests"));
        assert!(production.contains("wire_limb_digests"));
    }
}
