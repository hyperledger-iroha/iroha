//! Masked-coefficient pass for the bounded qPCS V2 prover.
//!
//! This private prerequisite consumes exact, source-owned `(P, H)` coefficient
//! rows in limb/repetition order, samples one independently domain-separated
//! mask per row, and writes the fixed-width `P~ = P + (X^N + 1)S` and
//! `H~ = H + S` rows to the accepted authenticated coefficient spool.  `S` has
//! exactly `N - 1` stored coefficients, so `S[N - 1]`, `P~[2N - 1]`, and
//! `H~[N - 1]` are structurally zero.  The coefficient store must seal before
//! the resulting typestate can be used by a later LDE pass.
//!
//! The source/algebra/replay seal is production-uninhabited.  This slice does
//! not itself perform the LDE or initial Merkle pass.  A private child can
//! prepare those artifacts only behind three uninhabited production authorities. A second
//! independently uninhabited child can retain `S`, derive the post-root points, and freeze the
//! opening-quotient root. Batching, FRI, cross-field binding, and proof emission remain absent;
//! every completion gate remains false.
use super::*;
use core::{convert::Infallible, fmt, sync::atomic};
use std::path::Path;
const MASK_SAMPLE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-sample.external-entropy\0";
const MASK_SAMPLE_ATTEMPTS_V2: u16 = 256;
const PRODUCT_COMPONENTS_V2: usize = 2;
const MASKED_COEFFICIENTS_COMPLETE_V2: bool = false;
const INITIAL_C0_ROOT_PREPARED_V2: bool = false;
const INITIAL_C0_ROOT_FROZEN_V2: bool = false;
const POST_ROOT_POINTS_DERIVED_V2: bool = false;
const CQ_ROWS_WRITTEN_V2: bool = false;
const FRI_FIRST_PASS_COMPLETE_V2: bool = false;
const FRI_SECOND_PASS_COMPLETE_V2: bool = false;
const CANONICAL_PROOF_EMITTED_V2: bool = false;
const PROVER_RELEASE_READY_V2: bool = false;
const _: () = {
    assert!(OPENING_REPETITIONS_V2 == 5);
    assert!(COEFFICIENT_COMPONENTS_V2 == 3);
    assert!(!MASKED_COEFFICIENTS_COMPLETE_V2);
    assert!(!INITIAL_C0_ROOT_PREPARED_V2);
    assert!(!INITIAL_C0_ROOT_FROZEN_V2);
    assert!(!POST_ROOT_POINTS_DERIVED_V2);
    assert!(!CQ_ROWS_WRITTEN_V2);
    assert!(!FRI_FIRST_PASS_COMPLETE_V2);
    assert!(!FRI_SECOND_PASS_COMPLETE_V2);
    assert!(!CANONICAL_PROOF_EMITTED_V2);
    assert!(!PROVER_RELEASE_READY_V2);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProverPrerequisiteErrorV2 {
    InvalidRelationOrder,
    InvalidSourceShape,
    NonCanonicalResidue,
    EntropyFailure,
    RejectionBoundExhausted,
    ReusedMask,
    MissingRelations,
    Allocation,
    ArithmeticOverflow,
    InvalidC0Geometry,
    InvalidC0Context,
    InvalidNtt,
    InvalidMerkleRoot,
    InvalidPostRootTranscript,
    InvalidOpeningQuotient,
    InvalidRelation,
    InvalidCanonicalProof,
    CanonicalProofSink,
    Poisoned,
    Spool(QPcsSpoolErrorV2),
}
impl fmt::Display for ProverPrerequisiteErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl From<QPcsSpoolErrorV2> for ProverPrerequisiteErrorV2 {
    fn from(error: QPcsSpoolErrorV2) -> Self {
        Self::Spool(error)
    }
}
impl From<ConfidentialSpoolErrorV1> for ProverPrerequisiteErrorV2 {
    fn from(error: ConfidentialSpoolErrorV1) -> Self {
        Self::Spool(QPcsSpoolErrorV2::Leaf(error))
    }
}
/// The future source child must possess all three independent authorities.
pub(super) enum ProverSourceSealV2 {
    Production {
        source_aggregation: Infallible,
        algebra_verification: Infallible,
        authenticated_replay: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
/// Public coordinates supplied to the external, fallible entropy source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskSampleDomainV2 {
    pub(super) domain: &'static [u8],
    pub(super) sealed_source_transcript_digest: [u8; 32],
    pub(super) source_algebra_binding_digest: [u8; 32],
    pub(super) limb: u8,
    pub(super) repetition: u8,
    pub(super) coefficient: u32,
    pub(super) attempt: u16,
    pub(super) modulus: u64,
}
/// Entropy is injected by the eventual caller and may fail closed.
///
/// A successful call must overwrite all eight bytes with one independently
/// uniform word for this exact coordinate.  Returning `Err` must not be
/// retried as success; the entire pass is poisoned.
pub(super) trait MaskEntropyV2 {
    fn fill_word_v2(
        &mut self,
        coordinate: MaskSampleDomainV2,
        destination: &mut [u8; 8],
    ) -> Result<(), ()>;
}
/// Exact-capacity, move-only source/mask residue owner.
pub(super) struct SecretResiduesV2 {
    values: Vec<u64>,
}
impl SecretResiduesV2 {
    pub(super) fn new_zeroed_exact_v2(len: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        if len == 0 {
            return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if values.capacity() != len {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        values.resize(len, 0);
        Ok(Self { values })
    }
    pub(super) fn as_mut_slice_v2(&mut self) -> &mut [u64] {
        &mut self.values
    }
    fn as_slice_v2(&self) -> &[u64] {
        &self.values
    }
}
impl Drop for SecretResiduesV2 {
    fn drop(&mut self) {
        self.values.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        SECRET_RESIDUE_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}
/// One already-aggregated and algebra-verified source relation.
pub(super) struct SecretRelationCoefficientsV2 {
    limb: u8,
    repetition: u8,
    product: SecretResiduesV2,
    quotient: SecretResiduesV2,
}
impl SecretRelationCoefficientsV2 {
    pub(super) fn new_v2(
        limb: u8,
        repetition: u8,
        product: SecretResiduesV2,
        quotient: SecretResiduesV2,
    ) -> Self {
        Self {
            limb,
            repetition,
            product,
            quotient,
        }
    }
}
struct SecretEntropyWordV2 {
    bytes: [u8; 8],
}
impl SecretEntropyWordV2 {
    const fn zeroed_v2() -> Self {
        Self { bytes: [0; 8] }
    }
}
impl Drop for SecretEntropyWordV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
        #[cfg(test)]
        SECRET_ENTROPY_WORD_DROPS_V2.fetch_add(1, atomic::Ordering::SeqCst);
    }
}
struct MaskReuseGuardV2 {
    limb: u8,
    masks: Vec<SecretResiduesV2>,
}
impl MaskReuseGuardV2 {
    fn new_v2() -> Result<Self, ProverPrerequisiteErrorV2> {
        let capacity = usize::from(OPENING_REPETITIONS_V2);
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(capacity)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if masks.capacity() != capacity {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        Ok(Self { limb: 0, masks })
    }
    fn check_v2(
        &mut self,
        limb: u8,
        repetition: u8,
        mask: &SecretResiduesV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if repetition == 0 {
            self.masks.clear();
            self.limb = limb;
        }
        if self.limb != limb || usize::from(repetition) != self.masks.len() {
            return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
        }
        if self
            .masks
            .iter()
            .any(|previous| previous.as_slice_v2() == mask.as_slice_v2())
        {
            return Err(ProverPrerequisiteErrorV2::ReusedMask);
        }
        Ok(())
    }
    fn commit_v2(&mut self, mask: SecretResiduesV2) -> Result<(), ProverPrerequisiteErrorV2> {
        let capacity = usize::from(OPENING_REPETITIONS_V2);
        if self.masks.len() >= capacity || self.masks.capacity() != capacity {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        self.masks.push(mask);
        if self.masks.capacity() != capacity {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        Ok(())
    }
}
impl Drop for MaskReuseGuardV2 {
    fn drop(&mut self) {
        self.masks.clear();
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}
struct LiveMaskedCoefficientPassV2 {
    writer: QPcsSpoolWriterV2,
    mask_writer: MaskSpoolWriterV2,
    next_relation: u16,
    reuse: MaskReuseGuardV2,
}
pub(super) struct MaskedCoefficientPassV2 {
    live: Option<LiveMaskedCoefficientPassV2>,
    geometry: SpoolGeometryV2,
    context: PublicSpoolContextV2,
}
/// The only successful output: coefficients are sealed, but no LDE/root exists.
pub(super) struct CoefficientsSealedV2 {
    stage: Option<QPcsCoefficientReplayStageV2>,
    masks: Option<MaskSpoolSealedV2>,
    context: PublicSpoolContextV2,
}
impl MaskedCoefficientPassV2 {
    pub(super) fn create_in_v2(
        directory: &Path,
        context: PublicSpoolContextV2,
        seal: ProverSourceSealV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let permit = match seal {
            ProverSourceSealV2::Production {
                source_aggregation: _source_aggregation,
                algebra_verification: _algebra_verification,
                authenticated_replay,
            } => match authenticated_replay {},
            #[cfg(test)]
            ProverSourceSealV2::TestOnly => AuthenticatedReplayPermitV2::TestOnly,
        };
        Self::create_with_geometry_v2(directory, SpoolGeometryV2::release_v2(), context, permit)
    }
    fn create_with_geometry_v2(
        directory: &Path,
        geometry: SpoolGeometryV2,
        context: PublicSpoolContextV2,
        permit: AuthenticatedReplayPermitV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let reuse = MaskReuseGuardV2::new_v2()?;
        let writer =
            QPcsSpoolWriterV2::create_with_geometry_v2(directory, geometry, context, permit)?;
        let mask_writer = MaskSpoolWriterV2::create_v2(
            directory,
            geometry,
            parameter_digest_v2(geometry)?,
            context,
        )?;
        Ok(Self {
            live: Some(LiveMaskedCoefficientPassV2 {
                writer,
                mask_writer,
                next_relation: 0,
                reuse,
            }),
            geometry,
            context,
        })
    }
    pub(super) fn absorb_next_relation_v2(
        &mut self,
        relation: SecretRelationCoefficientsV2,
        entropy: &mut impl MaskEntropyV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        absorb_relation_operation_v2(&mut live, self.geometry, self.context, relation, entropy)?;
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn seal_coefficients_v2(
        mut self,
    ) -> Result<CoefficientsSealedV2, ProverPrerequisiteErrorV2> {
        let live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let expected = u16::from(self.geometry.limb_count_v2()?)
            .checked_mul(u16::from(OPENING_REPETITIONS_V2))
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let completed_limbs = live
            .reuse
            .limb
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if live.next_relation != expected
            || live.reuse.masks.len() != usize::from(OPENING_REPETITIONS_V2)
            || completed_limbs != self.geometry.limb_count_v2()?
        {
            return Err(ProverPrerequisiteErrorV2::MissingRelations);
        }
        let stage = live.writer.seal_coefficients_for_replay_v2()?;
        let masks = live.mask_writer.seal_v2()?;
        Ok(CoefficientsSealedV2 {
            stage: Some(stage),
            masks: Some(masks),
            context: self.context,
        })
    }
    #[cfg(test)]
    fn panic_after_take_for_test_v2(&mut self) {
        let _live = self.live.take().expect("live masked coefficient pass");
        panic!("intentional masked-coefficient unwind");
    }
}
fn absorb_relation_operation_v2(
    live: &mut LiveMaskedCoefficientPassV2,
    geometry: SpoolGeometryV2,
    context: PublicSpoolContextV2,
    relation: SecretRelationCoefficientsV2,
    entropy: &mut impl MaskEntropyV2,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let expected_limb = u8::try_from(live.next_relation / u16::from(OPENING_REPETITIONS_V2))
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let expected_repetition = u8::try_from(live.next_relation % u16::from(OPENING_REPETITIONS_V2))
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if relation.limb != expected_limb || relation.repetition != expected_repetition {
        return Err(ProverPrerequisiteErrorV2::InvalidRelationOrder);
    }
    let modulus = *geometry
        .moduli
        .get(usize::from(relation.limb))
        .ok_or(ProverPrerequisiteErrorV2::InvalidRelationOrder)?;
    let ring_degree = usize::try_from(geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let product_len = ring_degree
        .checked_mul(PRODUCT_COMPONENTS_V2)
        .and_then(|value| value.checked_sub(1))
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let mask_len = ring_degree
        .checked_sub(1)
        .ok_or(ProverPrerequisiteErrorV2::InvalidSourceShape)?;
    if relation.product.as_slice_v2().len() != product_len
        || relation.quotient.as_slice_v2().len() != mask_len
    {
        return Err(ProverPrerequisiteErrorV2::InvalidSourceShape);
    }
    if relation
        .product
        .as_slice_v2()
        .iter()
        .chain(relation.quotient.as_slice_v2())
        .any(|value| *value >= modulus)
    {
        return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
    }
    let mask = sample_mask_v2(
        mask_len,
        modulus,
        context,
        relation.limb,
        relation.repetition,
        entropy,
    )?;
    live.reuse
        .check_v2(relation.limb, relation.repetition, &mask)?;
    live.mask_writer
        .push_next_mask_v2(relation.limb, relation.repetition, &mask)?;
    write_masked_relation_v2(&mut live.writer, geometry, modulus, &relation, &mask)?;
    live.reuse.commit_v2(mask)?;
    live.next_relation = live
        .next_relation
        .checked_add(1)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    Ok(())
}
fn sample_mask_v2(
    len: usize,
    modulus: u64,
    context: PublicSpoolContextV2,
    limb: u8,
    repetition: u8,
    entropy: &mut impl MaskEntropyV2,
) -> Result<SecretResiduesV2, ProverPrerequisiteErrorV2> {
    let mut mask = SecretResiduesV2::new_zeroed_exact_v2(len)?;
    let zone = u64::MAX - u64::MAX % modulus;
    for (coefficient, destination) in mask.as_mut_slice_v2().iter_mut().enumerate() {
        let coefficient = u32::try_from(coefficient)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let mut accepted = None;
        for attempt in 0..MASK_SAMPLE_ATTEMPTS_V2 {
            let mut word = SecretEntropyWordV2::zeroed_v2();
            entropy
                .fill_word_v2(
                    MaskSampleDomainV2 {
                        domain: MASK_SAMPLE_DOMAIN_V2,
                        sealed_source_transcript_digest: context.sealed_source_transcript_digest,
                        source_algebra_binding_digest: context.source_algebra_binding_digest,
                        limb,
                        repetition,
                        coefficient,
                        attempt,
                        modulus,
                    },
                    &mut word.bytes,
                )
                .map_err(|()| ProverPrerequisiteErrorV2::EntropyFailure)?;
            let candidate = u64::from_be_bytes(word.bytes);
            if candidate < zone {
                accepted = Some(candidate % modulus);
                break;
            }
        }
        *destination = accepted.ok_or(ProverPrerequisiteErrorV2::RejectionBoundExhausted)?;
    }
    Ok(mask)
}
fn add_mod_v2(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(modulus)) as u64
}
fn write_masked_relation_v2(
    writer: &mut QPcsSpoolWriterV2,
    geometry: SpoolGeometryV2,
    modulus: u64,
    relation: &SecretRelationCoefficientsV2,
    mask: &SecretResiduesV2,
) -> Result<(), ProverPrerequisiteErrorV2> {
    let ring_degree = usize::try_from(geometry.ring_degree)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let values_per_block = usize::from(geometry.coefficient_values_per_block);
    let blocks = usize::try_from(geometry.coefficient_blocks_per_component_v2()?)
        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    for block in 0..blocks {
        let first = block
            .checked_mul(values_per_block)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        for component in 0..COEFFICIENT_COMPONENTS_V2 {
            let mut chunk =
                ConfidentialSpoolChunkV1::new_zeroed_v1(geometry.coefficient_block_bytes_v2()?)?;
            for (offset, encoded) in chunk.as_mut_slice_v1().chunks_exact_mut(8).enumerate() {
                let index = first
                    .checked_add(offset)
                    .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
                let value = match component {
                    0 => {
                        let source = relation.product.as_slice_v2()[index];
                        if index + 1 == ring_degree {
                            source
                        } else {
                            add_mod_v2(source, mask.as_slice_v2()[index], modulus)
                        }
                    }
                    1 => {
                        if index + 1 == ring_degree {
                            0
                        } else {
                            let product_index = ring_degree
                                .checked_add(index)
                                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
                            add_mod_v2(
                                relation.product.as_slice_v2()[product_index],
                                mask.as_slice_v2()[index],
                                modulus,
                            )
                        }
                    }
                    2 => {
                        if index + 1 == ring_degree {
                            0
                        } else {
                            add_mod_v2(
                                relation.quotient.as_slice_v2()[index],
                                mask.as_slice_v2()[index],
                                modulus,
                            )
                        }
                    }
                    _ => return Err(ProverPrerequisiteErrorV2::ArithmeticOverflow),
                };
                encoded.copy_from_slice(&value.to_be_bytes());
            }
            writer.push_coefficient_block_v2(chunk)?;
        }
    }
    Ok(())
}
#[cfg(test)]
static SECRET_RESIDUE_DROPS_V2: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static SECRET_ENTROPY_WORD_DROPS_V2: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[path = "prover_v2/s_spool_v2.rs"]
mod s_spool_v2;
use s_spool_v2::*;
#[path = "prover_v2/c0_v2.rs"]
mod c0_v2;
#[cfg(test)]
#[path = "prover_v2_tests.rs"]
mod tests;
