//! Private T256 cross-basis equality kernel for exact terminal rows.
//!
//! This module is deliberately not a release capability.  Its production
//! owner contains uninhabited source and packing bindings, while tests may use
//! a local permit to exercise the cryptographic kernel.  In particular, no
//! constructor accepts detached digests, commitments, or opening vectors.
//!
//! The kernel uses a representation-equality Schnorr proof over the exact
//! 1,025-coordinate terminal opening.  Its fresh 512-bit-reduced response
//! mask gives statistical honest-verifier zero knowledge within less than
//! 2^-245 distance from the ideal uniform-vector simulator; Fiat--Shamir binds
//! both first messages and the complete ordered statement before deriving the
//! challenge.  No clear evaluation or deterministic folded witness scalar
//! enters the wire.
use super::super::super::{
    MaskedRelaxedRandomSourceV1, VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
    bulletproof_t256::{ZeroizingT256ScalarVecV1, ZkAmsT256BulletproofSuiteV1},
    commitment::CommitmentKey,
    masked_relaxed::MASKED_RELAXED_COMMITMENT_COLUMNS_V1,
    sponge::Keccak256,
};
use super::{
    ZeroizingRandomBytesV1,
    phase23_encrypted::{
        ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1,
        ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1,
    },
    terminal::ZkAmsPhase3PreparedTerminalOpeningsV1,
};
use crate::generalized_bulletproof::{ProofSuite, SecretMultiexpBuilder, multiexp};
use core::{convert::Infallible, mem};
use std::collections::BTreeSet;
use thiserror::Error;
const BRIDGE_VERSION_V2: u8 = 2;
const BRIDGE_ROWS_V2: usize = ZK_AMS_PHASE23_RELEASE_ERROR_COMMITMENT_ROWS_V1
    + ZK_AMS_PHASE23_RELEASE_WITNESS_COMMITMENT_ROWS_V1;
const BRIDGE_VALUE_COLUMNS_V2: usize = MASKED_RELAXED_COMMITMENT_COLUMNS_V1;
const BRIDGE_BASIS_VIEW_V2: usize = BRIDGE_VALUE_COLUMNS_V2 + 1;
const BRIDGE_POINT_BYTES_V2: usize = 33;
const BRIDGE_SCALAR_BYTES_V2: usize = 32;
const BRIDGE_MASK_POINT_BYTES_V2: usize = 2 * BRIDGE_POINT_BYTES_V2;
const BRIDGE_RESPONSE_BYTES_V2: usize = BRIDGE_BASIS_VIEW_V2 * BRIDGE_SCALAR_BYTES_V2;
const BRIDGE_RAW_PROOF_BYTES_V2: usize = BRIDGE_MASK_POINT_BYTES_V2 + BRIDGE_RESPONSE_BYTES_V2;
const BRIDGE_MAX_CHALLENGE_ATTEMPTS_V2: u16 = 128;
const BRIDGE_MAX_MASK_ATTEMPTS_V2: u8 = 2;
const BRIDGE_MASK_ENTROPY_BYTES_V2: usize = 64;
const BRIDGE_MASK_STATISTICAL_SECURITY_BITS_V2: u16 = 245;
const HYRAX_KEY_LABEL_V2: &[u8] = b"iroha.zk-ams.v1.batch-admission.hyrax-t256";
const BASIS_DIGEST_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.cross-basis.basis";
const COMMITMENT_ROOT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.cross-basis.commitment-root";
const ETA_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.cross-basis.eta";
const BRIDGE_ROOT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.cross-basis.root";
const SCHNORR_TRANSCRIPT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.cross-basis.representation-equality";
const SCHNORR_CHALLENGE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.cross-basis.representation-equality.challenge";
// These remain false until a single source-and-packing owner replaces both
// uninhabited fields below and the terminal consumes its verified result.
const BRIDGE_SOURCE_BOUND_V2: bool = false;
const BRIDGE_PACKING_BOUND_V2: bool = false;
const BRIDGE_TERMINAL_WIRED_V2: bool = false;
const BRIDGE_RELEASE_ENABLED_V2: bool = false;
const _: [(); 1_536] = [(); BRIDGE_ROWS_V2];
const _: [(); 1_025] = [(); BRIDGE_BASIS_VIEW_V2];
const _: [(); 32_866] = [(); BRIDGE_RAW_PROOF_BYTES_V2];
#[derive(Debug, Error, PartialEq, Eq)]
enum BridgeErrorV2 {
    #[error("cross-basis bridge dimensions are not the exact terminal shape")]
    Shape,
    #[error("cross-basis bridge context is invalid")]
    Context,
    #[error("cross-basis bridge generator basis is invalid")]
    Basis,
    #[error("cross-basis bridge commitment/opening equality failed")]
    Commitment,
    #[error("cross-basis bridge proof exceeds its exact cap")]
    ProofTooLarge,
    #[error("cross-basis bridge proof framing is invalid")]
    Wire,
    #[error("cross-basis bridge representation-equality equation failed")]
    Representation,
    #[error("cross-basis bridge masking entropy is unavailable")]
    Random,
    #[error("cross-basis bridge arithmetic or allocation failed")]
    Arithmetic,
    #[error("cross-basis bridge production binding is unavailable")]
    BindingUnavailable,
}
/// Move-only production input.  There is intentionally no constructor.
///
/// The terminal borrow is not enough to mint this type: the source and
/// packing fields are separately uninhabited.  Future wiring must replace
/// them with one consuming, verified owner rather than add a point-only path.
struct BoundT256BridgeRowSetV2<'a> {
    terminal_rows: Option<ZkAmsPhase3PreparedTerminalOpeningsV1<'a>>,
    source_binding: Infallible,
    packing_binding: Infallible,
}
/// Move-only production result.  It cannot exist while either upstream seal
/// is uninhabited and exposes neither proof bytes nor any raw opening.
struct VerifiedBridgeBindingV2 {
    source_binding: Infallible,
    packing_binding: Infallible,
    proof: RawBridgeProofV2,
    bridge_root: [u8; 32],
}
impl BoundT256BridgeRowSetV2<'_> {
    /// Poison the row owner before any validation which could fail.
    fn into_verified_v2(mut self) -> Result<VerifiedBridgeBindingV2, BridgeErrorV2> {
        let terminal_rows = self
            .terminal_rows
            .take()
            .ok_or(BridgeErrorV2::BindingUnavailable)?;
        if terminal_rows.context_digest_v1() == [0; 32]
            || terminal_rows.materialized_digest_v1() == [0; 32]
        {
            return Err(BridgeErrorV2::Context);
        }
        let source_binding = self.source_binding;
        let packing_binding = self.packing_binding;
        let _ = terminal_rows;
        match (source_binding, packing_binding) {}
    }
}
struct RawBridgeProofV2 {
    bytes: [u8; BRIDGE_RAW_PROOF_BYTES_V2],
}
struct CheckedBasisV2 {
    points: Vec<Point>,
    digest: [u8; 32],
}
struct KernelStatementV2<'a> {
    binding_digest: [u8; 32],
    hyrax_commitments: &'a [Point],
    bp_commitments: &'a [Point],
}
struct KernelProverRowsV2<'a> {
    statement: KernelStatementV2<'a>,
    openings: &'a [Scalar],
}
struct AggregatedRowsV2 {
    opening: ZeroizingT256ScalarVecV1,
    hyrax_commitment: Point,
    bp_commitment: Point,
}
struct ProofWriterV2 {
    bytes: [u8; BRIDGE_RAW_PROOF_BYTES_V2],
    cursor: usize,
}
impl ProofWriterV2 {
    fn new() -> Self {
        Self {
            bytes: [0; BRIDGE_RAW_PROOF_BYTES_V2],
            cursor: 0,
        }
    }
    fn scalar(&mut self, scalar: Scalar) -> Result<(), BridgeErrorV2> {
        self.bytes(self.cursor, &scalar.to_le_bytes())
    }
    fn point(&mut self, point: Point) -> Result<(), BridgeErrorV2> {
        let encoded = point
            .to_non_identity_wire_bytes()
            .map_err(|_| BridgeErrorV2::Representation)?;
        self.bytes(self.cursor, &encoded)
    }
    fn bytes(&mut self, offset: usize, value: &[u8]) -> Result<(), BridgeErrorV2> {
        if offset != self.cursor {
            return Err(BridgeErrorV2::Wire);
        }
        let end = self
            .cursor
            .checked_add(value.len())
            .ok_or(BridgeErrorV2::Arithmetic)?;
        let destination = self
            .bytes
            .get_mut(self.cursor..end)
            .ok_or(BridgeErrorV2::Wire)?;
        destination.copy_from_slice(value);
        self.cursor = end;
        Ok(())
    }
    fn finish(mut self) -> Result<RawBridgeProofV2, BridgeErrorV2> {
        if self.cursor != BRIDGE_RAW_PROOF_BYTES_V2 {
            return Err(BridgeErrorV2::Wire);
        }
        let bytes = mem::replace(&mut self.bytes, [0; BRIDGE_RAW_PROOF_BYTES_V2]);
        Ok(RawBridgeProofV2 { bytes })
    }
}
impl Drop for ProofWriterV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        self.cursor = 0;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.bytes);
    }
}
struct ProofReaderV2<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> ProofReaderV2<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, BridgeErrorV2> {
        if bytes.len() > BRIDGE_RAW_PROOF_BYTES_V2 {
            return Err(BridgeErrorV2::ProofTooLarge);
        }
        if bytes.len() != BRIDGE_RAW_PROOF_BYTES_V2 {
            return Err(BridgeErrorV2::Wire);
        }
        Ok(Self { bytes, cursor: 0 })
    }
    fn scalar(&mut self) -> Result<Scalar, BridgeErrorV2> {
        let bytes: [u8; BRIDGE_SCALAR_BYTES_V2] = self
            .take::<BRIDGE_SCALAR_BYTES_V2>()?
            .try_into()
            .map_err(|_| BridgeErrorV2::Wire)?;
        Scalar::from_le_bytes_exact(bytes).map_err(|_| BridgeErrorV2::Wire)
    }
    fn point(&mut self) -> Result<Point, BridgeErrorV2> {
        Point::from_non_identity_wire_bytes_exact(self.take::<BRIDGE_POINT_BYTES_V2>()?)
            .map_err(|_| BridgeErrorV2::Wire)
    }
    fn take<const N: usize>(&mut self) -> Result<&'a [u8], BridgeErrorV2> {
        let end = self.cursor.checked_add(N).ok_or(BridgeErrorV2::Wire)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(BridgeErrorV2::Wire)?;
        self.cursor = end;
        Ok(value)
    }
    fn finish(self) -> Result<(), BridgeErrorV2> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(BridgeErrorV2::Wire)
        }
    }
}
fn assert_t256_suite_v2() {
    fn same_types<S: ProofSuite<Scalar = Scalar, Point = Point>>() {}
    same_types::<ZkAmsT256BulletproofSuiteV1>();
}
fn hyrax_basis_v2() -> Result<CheckedBasisV2, BridgeErrorV2> {
    if HYRAX_KEY_LABEL_V2 != super::super::COMMITMENT_KEY_LABEL_V1 {
        return Err(BridgeErrorV2::Basis);
    }
    let key = CommitmentKey::derive(HYRAX_KEY_LABEL_V2, BRIDGE_VALUE_COLUMNS_V2)
        .map_err(|_| BridgeErrorV2::Basis)?;
    let mut view = Vec::with_capacity(BRIDGE_BASIS_VIEW_V2);
    view.extend_from_slice(key.generators());
    view.push(key.hiding_generator());
    checked_basis_v2(view)
}
fn bp_basis_v2() -> Result<CheckedBasisV2, BridgeErrorV2> {
    assert_t256_suite_v2();
    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    let mut view = Vec::with_capacity(BRIDGE_BASIS_VIEW_V2);
    view.extend_from_slice(
        generators
            .g_bold
            .get(..BRIDGE_VALUE_COLUMNS_V2)
            .ok_or(BridgeErrorV2::Basis)?,
    );
    view.push(generators.h);
    checked_basis_v2(view)
}
fn checked_basis_v2(view: Vec<Point>) -> Result<CheckedBasisV2, BridgeErrorV2> {
    if view.len() != BRIDGE_BASIS_VIEW_V2 {
        return Err(BridgeErrorV2::Basis);
    }
    validate_independent_points_v2(&view)?;
    let digest = basis_digest_v2(&view)?;
    Ok(CheckedBasisV2 {
        points: view,
        digest,
    })
}
fn validate_independent_points_v2(points: &[Point]) -> Result<(), BridgeErrorV2> {
    let mut seen = BTreeSet::new();
    for point in points.iter().copied() {
        let wire = point
            .to_non_identity_wire_bytes()
            .map_err(|_| BridgeErrorV2::Basis)?;
        let inverse = point
            .negate()
            .to_non_identity_wire_bytes()
            .map_err(|_| BridgeErrorV2::Basis)?;
        if seen.contains(&wire) || seen.contains(&inverse) || !seen.insert(wire) {
            return Err(BridgeErrorV2::Basis);
        }
    }
    Ok(())
}
fn validate_disjoint_bases_v2(
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
) -> Result<(), BridgeErrorV2> {
    let mut hyrax_points = BTreeSet::new();
    for point in &hyrax_basis.points {
        hyrax_points.insert(
            point
                .to_non_identity_wire_bytes()
                .map_err(|_| BridgeErrorV2::Basis)?,
        );
        hyrax_points.insert(
            point
                .negate()
                .to_non_identity_wire_bytes()
                .map_err(|_| BridgeErrorV2::Basis)?,
        );
    }
    for point in &bp_basis.points {
        let wire = point
            .to_non_identity_wire_bytes()
            .map_err(|_| BridgeErrorV2::Basis)?;
        if hyrax_points.contains(&wire) {
            return Err(BridgeErrorV2::Basis);
        }
    }
    Ok(())
}
fn basis_digest_v2(points: &[Point]) -> Result<[u8; 32], BridgeErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(BASIS_DIGEST_DOMAIN_V2);
    hash.update(&[BRIDGE_VERSION_V2]);
    hash.update(
        &u32::try_from(BRIDGE_BASIS_VIEW_V2)
            .map_err(|_| BridgeErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    hash.update(
        &u32::try_from(points.len())
            .map_err(|_| BridgeErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    for point in points {
        hash.update(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| BridgeErrorV2::Basis)?,
        );
    }
    Ok(hash.finalize())
}
fn framed_hash_v2(domain: &[u8], fields: &[&[u8]]) -> Result<[u8; 32], BridgeErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(b"ZBIP");
    hash.update(&[BRIDGE_VERSION_V2]);
    hash.update(
        &u16::try_from(domain.len())
            .map_err(|_| BridgeErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    hash.update(domain);
    hash.update(
        &u16::try_from(fields.len())
            .map_err(|_| BridgeErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    for field in fields {
        hash.update(
            &u32::try_from(field.len())
                .map_err(|_| BridgeErrorV2::Arithmetic)?
                .to_be_bytes(),
        );
        hash.update(field);
    }
    Ok(hash.finalize())
}
fn challenge_v2(domain: &[u8], seed: [u8; 32]) -> Result<Scalar, BridgeErrorV2> {
    for attempt in 0..BRIDGE_MAX_CHALLENGE_ATTEMPTS_V2 {
        let attempt = attempt.to_be_bytes();
        let left = framed_hash_v2(domain, &[&seed, &attempt, &[0]])?;
        let right = framed_hash_v2(domain, &[&seed, &attempt, &[1]])?;
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&left);
        wide[32..].copy_from_slice(&right);
        let scalar = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !scalar.is_zero() {
            return Ok(scalar);
        }
    }
    Err(BridgeErrorV2::Arithmetic)
}
fn validate_statement_v2(statement: &KernelStatementV2<'_>) -> Result<(), BridgeErrorV2> {
    if statement.binding_digest == [0; 32]
        || statement.hyrax_commitments.len() != BRIDGE_ROWS_V2
        || statement.bp_commitments.len() != BRIDGE_ROWS_V2
        || statement
            .hyrax_commitments
            .iter()
            .chain(statement.bp_commitments)
            .any(|point| point.is_identity())
    {
        return Err(BridgeErrorV2::Shape);
    }
    Ok(())
}
fn commitment_root_v2(
    statement: &KernelStatementV2<'_>,
    hyrax_basis_digest: [u8; 32],
    bp_basis_digest: [u8; 32],
) -> Result<[u8; 32], BridgeErrorV2> {
    validate_statement_v2(statement)?;
    let mut hash = Keccak256::new();
    hash.update(COMMITMENT_ROOT_DOMAIN_V2);
    hash.update(&[BRIDGE_VERSION_V2]);
    hash.update(&statement.binding_digest);
    hash.update(&hyrax_basis_digest);
    hash.update(&bp_basis_digest);
    hash.update(
        &u32::try_from(BRIDGE_ROWS_V2)
            .map_err(|_| BridgeErrorV2::Arithmetic)?
            .to_be_bytes(),
    );
    for (ordinal, (hyrax, bp)) in statement
        .hyrax_commitments
        .iter()
        .zip(statement.bp_commitments)
        .enumerate()
    {
        hash.update(
            &u32::try_from(ordinal)
                .map_err(|_| BridgeErrorV2::Arithmetic)?
                .to_be_bytes(),
        );
        hash.update(
            &hyrax
                .to_non_identity_wire_bytes()
                .map_err(|_| BridgeErrorV2::Commitment)?,
        );
        hash.update(
            &bp.to_non_identity_wire_bytes()
                .map_err(|_| BridgeErrorV2::Commitment)?,
        );
    }
    Ok(hash.finalize())
}
fn aggregate_rows_v2(
    rows: &KernelProverRowsV2<'_>,
    eta: Scalar,
) -> Result<AggregatedRowsV2, BridgeErrorV2> {
    validate_statement_v2(&rows.statement)?;
    let expected = BRIDGE_ROWS_V2
        .checked_mul(BRIDGE_BASIS_VIEW_V2)
        .ok_or(BridgeErrorV2::Arithmetic)?;
    if rows.openings.len() != expected {
        return Err(BridgeErrorV2::Shape);
    }
    let mut opening = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    for _ in 0..BRIDGE_BASIS_VIEW_V2 {
        opening.push(Scalar::zero());
    }
    let mut hyrax_terms = Vec::new();
    let mut bp_terms = Vec::new();
    hyrax_terms
        .try_reserve_exact(BRIDGE_ROWS_V2)
        .map_err(|_| BridgeErrorV2::Arithmetic)?;
    bp_terms
        .try_reserve_exact(BRIDGE_ROWS_V2)
        .map_err(|_| BridgeErrorV2::Arithmetic)?;
    let mut weight = Scalar::one();
    for (ordinal, opening_row) in rows.openings.chunks_exact(BRIDGE_BASIS_VIEW_V2).enumerate() {
        for (aggregate, value) in opening
            .as_mut_slice()
            .iter_mut()
            .zip(opening_row)
            .take(BRIDGE_BASIS_VIEW_V2)
        {
            *aggregate += weight * *value;
        }
        hyrax_terms.push((weight, rows.statement.hyrax_commitments[ordinal]));
        bp_terms.push((weight, rows.statement.bp_commitments[ordinal]));
        weight *= eta;
    }
    let hyrax_commitment = multiexp::<ZkAmsT256BulletproofSuiteV1>(&hyrax_terms);
    let bp_commitment = multiexp::<ZkAmsT256BulletproofSuiteV1>(&bp_terms);
    if hyrax_commitment.is_identity() || bp_commitment.is_identity() {
        return Err(BridgeErrorV2::Commitment);
    }
    Ok(AggregatedRowsV2 {
        opening,
        hyrax_commitment,
        bp_commitment,
    })
}
fn secret_commit_v2(points: &[Point], scalars: &[Scalar]) -> Result<Point, BridgeErrorV2> {
    if points.len() != scalars.len() || points.is_empty() {
        return Err(BridgeErrorV2::Shape);
    }
    let mut terms = SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(points.len())
        .map_err(|_| BridgeErrorV2::Arithmetic)?;
    for (scalar, point) in scalars.iter().zip(points) {
        terms
            .push(scalar, point)
            .map_err(|_| BridgeErrorV2::Arithmetic)?;
    }
    terms.evaluate().map_err(|_| BridgeErrorV2::Arithmetic)
}
fn bridge_root_v2(
    statement: &KernelStatementV2<'_>,
    commitment_root: [u8; 32],
    eta: Scalar,
    aggregate: &AggregatedRowsV2,
    hyrax_basis_digest: [u8; 32],
    bp_basis_digest: [u8; 32],
) -> Result<[u8; 32], BridgeErrorV2> {
    let eta = eta.to_le_bytes();
    let hyrax = aggregate
        .hyrax_commitment
        .to_non_identity_wire_bytes()
        .map_err(|_| BridgeErrorV2::Commitment)?;
    let bp = aggregate
        .bp_commitment
        .to_non_identity_wire_bytes()
        .map_err(|_| BridgeErrorV2::Commitment)?;
    framed_hash_v2(
        BRIDGE_ROOT_DOMAIN_V2,
        &[
            &statement.binding_digest,
            &commitment_root,
            &eta,
            &hyrax,
            &bp,
            &hyrax_basis_digest,
            &bp_basis_digest,
        ],
    )
}
fn sample_mask_v2<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
) -> Result<(ZeroizingT256ScalarVecV1, Point, Point), BridgeErrorV2> {
    if hyrax_basis.points.len() != BRIDGE_BASIS_VIEW_V2
        || bp_basis.points.len() != BRIDGE_BASIS_VIEW_V2
    {
        return Err(BridgeErrorV2::Basis);
    }
    for _ in 0..BRIDGE_MAX_MASK_ATTEMPTS_V2 {
        let mut mask = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
        for _ in 0..BRIDGE_BASIS_VIEW_V2 {
            let mut entropy = ZeroizingRandomBytesV1::<BRIDGE_MASK_ENTROPY_BYTES_V2>::zeroed();
            random
                .fill_bytes(entropy.as_mut_slice())
                .map_err(|_| BridgeErrorV2::Random)?;
            mask.push(Scalar::from_uniform_le_bytes_ref(entropy.as_array()));
        }
        let hyrax_mask = secret_commit_v2(&hyrax_basis.points, mask.as_slice())?;
        let bp_mask = secret_commit_v2(&bp_basis.points, mask.as_slice())?;
        if !hyrax_mask.is_identity() && !bp_mask.is_identity() {
            return Ok((mask, hyrax_mask, bp_mask));
        }
    }
    Err(BridgeErrorV2::Random)
}
fn schnorr_challenge_v2(
    bridge_root: [u8; 32],
    hyrax_basis_digest: [u8; 32],
    bp_basis_digest: [u8; 32],
    hyrax_mask: Point,
    bp_mask: Point,
) -> Result<Scalar, BridgeErrorV2> {
    let hyrax_mask = hyrax_mask
        .to_non_identity_wire_bytes()
        .map_err(|_| BridgeErrorV2::Representation)?;
    let bp_mask = bp_mask
        .to_non_identity_wire_bytes()
        .map_err(|_| BridgeErrorV2::Representation)?;
    let seed = framed_hash_v2(
        SCHNORR_TRANSCRIPT_DOMAIN_V2,
        &[
            &bridge_root,
            &hyrax_basis_digest,
            &bp_basis_digest,
            &hyrax_mask,
            &bp_mask,
        ],
    )?;
    challenge_v2(SCHNORR_CHALLENGE_DOMAIN_V2, seed)
}
fn respond_v2(
    mut mask: ZeroizingT256ScalarVecV1,
    opening: &ZeroizingT256ScalarVecV1,
    challenge: Scalar,
) -> Result<ZeroizingT256ScalarVecV1, BridgeErrorV2> {
    if mask.len() != BRIDGE_BASIS_VIEW_V2 || opening.len() != BRIDGE_BASIS_VIEW_V2 {
        return Err(BridgeErrorV2::Shape);
    }
    for (response, value) in mask.as_mut_slice().iter_mut().zip(opening.as_slice()) {
        *response += challenge * *value;
    }
    Ok(mask)
}
fn verify_representation_v2(
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
    aggregate: &AggregatedRowsV2,
    hyrax_mask: Point,
    bp_mask: Point,
    challenge: Scalar,
    response: &ZeroizingT256ScalarVecV1,
) -> Result<(), BridgeErrorV2> {
    if response.len() != BRIDGE_BASIS_VIEW_V2 {
        return Err(BridgeErrorV2::Shape);
    }
    let hyrax_response = secret_commit_v2(&hyrax_basis.points, response.as_slice())?;
    let bp_response = secret_commit_v2(&bp_basis.points, response.as_slice())?;
    if hyrax_response != hyrax_mask + aggregate.hyrax_commitment.mul_scalar(challenge)
        || bp_response != bp_mask + aggregate.bp_commitment.mul_scalar(challenge)
    {
        return Err(BridgeErrorV2::Representation);
    }
    Ok(())
}
fn prepare_kernel_v2(
    statement: &KernelStatementV2<'_>,
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
) -> Result<([u8; 32], Scalar), BridgeErrorV2> {
    validate_disjoint_bases_v2(hyrax_basis, bp_basis)?;
    let commitment_root = commitment_root_v2(statement, hyrax_basis.digest, bp_basis.digest)?;
    let eta = challenge_v2(ETA_DOMAIN_V2, commitment_root)?;
    Ok((commitment_root, eta))
}
/// Consume the proof-session entropy owner so a successful first message
/// cannot be replayed by invoking this session a second time.
fn prove_kernel_v2<R: MaskedRelaxedRandomSourceV1>(
    mut random: R,
    rows: &KernelProverRowsV2<'_>,
) -> Result<RawBridgeProofV2, BridgeErrorV2> {
    let hyrax_basis = hyrax_basis_v2()?;
    let bp_basis = bp_basis_v2()?;
    let (commitment_root, eta) = prepare_kernel_v2(&rows.statement, &hyrax_basis, &bp_basis)?;
    let aggregate = aggregate_rows_v2(rows, eta)?;
    if secret_commit_v2(&hyrax_basis.points, aggregate.opening.as_slice())?
        != aggregate.hyrax_commitment
        || secret_commit_v2(&bp_basis.points, aggregate.opening.as_slice())?
            != aggregate.bp_commitment
    {
        return Err(BridgeErrorV2::Commitment);
    }
    let bridge_root = bridge_root_v2(
        &rows.statement,
        commitment_root,
        eta,
        &aggregate,
        hyrax_basis.digest,
        bp_basis.digest,
    )?;
    let (mask, hyrax_mask, bp_mask) = sample_mask_v2(&mut random, &hyrax_basis, &bp_basis)?;
    let challenge = schnorr_challenge_v2(
        bridge_root,
        hyrax_basis.digest,
        bp_basis.digest,
        hyrax_mask,
        bp_mask,
    )?;
    let response = respond_v2(mask, &aggregate.opening, challenge)?;
    let mut writer = ProofWriterV2::new();
    writer.point(hyrax_mask)?;
    writer.point(bp_mask)?;
    for scalar in response.as_slice() {
        writer.scalar(*scalar)?;
    }
    let proof = writer.finish()?;
    verify_kernel_with_bases_v2(&rows.statement, &proof.bytes, &hyrax_basis, &bp_basis)?;
    Ok(proof)
}
fn verify_kernel_with_bases_v2(
    statement: &KernelStatementV2<'_>,
    proof_bytes: &[u8],
    hyrax_basis: &CheckedBasisV2,
    bp_basis: &CheckedBasisV2,
) -> Result<[u8; 32], BridgeErrorV2> {
    let (commitment_root, eta) = prepare_kernel_v2(statement, hyrax_basis, bp_basis)?;
    let mut hyrax_terms = Vec::with_capacity(BRIDGE_ROWS_V2);
    let mut bp_terms = Vec::with_capacity(BRIDGE_ROWS_V2);
    let mut weight = Scalar::one();
    for (hyrax, bp) in statement
        .hyrax_commitments
        .iter()
        .zip(statement.bp_commitments)
    {
        hyrax_terms.push((weight, *hyrax));
        bp_terms.push((weight, *bp));
        weight *= eta;
    }
    let aggregate = AggregatedRowsV2 {
        opening: ZeroizingT256ScalarVecV1::with_capacity(0),
        hyrax_commitment: multiexp::<ZkAmsT256BulletproofSuiteV1>(&hyrax_terms),
        bp_commitment: multiexp::<ZkAmsT256BulletproofSuiteV1>(&bp_terms),
    };
    if aggregate.hyrax_commitment.is_identity() || aggregate.bp_commitment.is_identity() {
        return Err(BridgeErrorV2::Commitment);
    }
    let bridge_root = bridge_root_v2(
        statement,
        commitment_root,
        eta,
        &aggregate,
        hyrax_basis.digest,
        bp_basis.digest,
    )?;
    let mut reader = ProofReaderV2::new(proof_bytes)?;
    let hyrax_mask = reader.point()?;
    let bp_mask = reader.point()?;
    let mut response = ZeroizingT256ScalarVecV1::with_capacity(BRIDGE_BASIS_VIEW_V2);
    for _ in 0..BRIDGE_BASIS_VIEW_V2 {
        response.push(reader.scalar()?);
    }
    reader.finish()?;
    let challenge = schnorr_challenge_v2(
        bridge_root,
        hyrax_basis.digest,
        bp_basis.digest,
        hyrax_mask,
        bp_mask,
    )?;
    verify_representation_v2(
        hyrax_basis,
        bp_basis,
        &aggregate,
        hyrax_mask,
        bp_mask,
        challenge,
        &response,
    )?;
    Ok(bridge_root)
}
fn verify_kernel_v2(
    statement: &KernelStatementV2<'_>,
    proof_bytes: &[u8],
) -> Result<[u8; 32], BridgeErrorV2> {
    verify_kernel_with_bases_v2(statement, proof_bytes, &hyrax_basis_v2()?, &bp_basis_v2()?)
}
#[cfg(test)]
struct TestBridgePermitV2(());
#[cfg(test)]
impl TestBridgePermitV2 {
    fn mint() -> Self {
        Self(())
    }
}
#[cfg(test)]
fn prove_with_test_permit_v2<R: MaskedRelaxedRandomSourceV1>(
    _permit: TestBridgePermitV2,
    random: R,
    rows: &KernelProverRowsV2<'_>,
) -> Result<RawBridgeProofV2, BridgeErrorV2> {
    prove_kernel_v2(random, rows)
}
#[cfg(test)]
#[path = "terminal_cross_basis_ipa_tests.rs"]
mod tests;
