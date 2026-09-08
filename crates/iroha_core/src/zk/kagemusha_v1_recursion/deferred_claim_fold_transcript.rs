//! Complete Claim fold transcripts with fixed-geometry native selection and Base fallback.

use super::*;
use crate::zk::pasta_native_poseidon::PastaNativePoseidonTranscriptV1;
use snark_verifier::{
    system::halo2::transcript::halo2::{NativeEncoding as _, TranscriptObject},
    util::transcript::{Transcript, TranscriptRead},
};

pub(super) const FOLD_PERMUTATIONS: usize = 75;
const FOLD_SQUEEZE_INPUTS: [usize; 21] = {
    let mut counts = [4; 21];
    counts[0] = 41;
    counts[1] = 0;
    counts[2] = 2;
    counts[3] = 1;
    counts[20] = 3;
    counts
};
// Existing k16, two-lane and complete 1,008-source/seven-equation Claim envelopes.
const NATIVE_CAPACITY: usize = 1_984;
const MAX_BATCH_PERMUTATIONS: usize = 1_054;

/// Whole-transcript implementation chosen only from authenticated protocol inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::zk::kagemusha_v1_recursion) enum ClaimFoldTranscriptModeV1 {
    /// Preserve the existing complete constrained transcript for a larger initial seed.
    Base,
    /// Retain all 75 permutations in the already-configured native queue.
    Native,
}

/// Deterministic reservation of the two complete folds after mandatory native hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::zk::kagemusha_v1_recursion) struct ClaimFoldTranscriptPlanV1 {
    parent: ClaimFoldTranscriptModeV1,
    successor: ClaimFoldTranscriptModeV1,
}

impl ClaimFoldTranscriptPlanV1 {
    /// Existing fold-only reservation, retained for its original geometry regressions.
    #[cfg(test)]
    pub(in crate::zk::kagemusha_v1_recursion) fn new(
        parent_preprocessed: usize,
        shard_preprocessed: usize,
    ) -> Result<Self, String> {
        Self::with_reserved_ordinary(parent_preprocessed, shard_preprocessed, 0)
    }

    /// Remaining capacity after the unchanged maximum batch and exact protocol identities.
    pub(super) fn capacity_after_mandatory(
        parent_preprocessed: usize,
        shard_preprocessed: usize,
    ) -> Result<usize, String> {
        let mandatory = parent_preprocessed
            .checked_add(shard_preprocessed)
            .and_then(|count| count.checked_add(14))
            .and_then(|count| count.checked_add(MAX_BATCH_PERMUTATIONS))
            .ok_or_else(|| "Claim fold transcript geometry overflow".to_owned())?;
        Ok(NATIVE_CAPACITY.saturating_sub(mandatory))
    }

    /// Reserve both complete folds after already selected complete ordinary transcripts.
    pub(super) fn with_reserved_ordinary(
        parent_preprocessed: usize,
        shard_preprocessed: usize,
        reserved: usize,
    ) -> Result<Self, String> {
        let mut remaining =
            Self::capacity_after_mandatory(parent_preprocessed, shard_preprocessed)?
                .checked_sub(reserved)
                .ok_or_else(|| "ordinary Claim transcript exceeds reserved capacity".to_owned())?;
        let mut modes = [ClaimFoldTranscriptModeV1::Base; 2];
        for mode in &mut modes {
            if remaining >= FOLD_PERMUTATIONS {
                *mode = ClaimFoldTranscriptModeV1::Native;
                remaining -= FOLD_PERMUTATIONS;
            }
        }
        Ok(Self {
            parent: modes[0],
            successor: modes[1],
        })
    }

    /// Selected implementation of the predecessor-history fold.
    pub(in crate::zk::kagemusha_v1_recursion) fn parent(self) -> ClaimFoldTranscriptModeV1 {
        self.parent
    }

    /// Selected implementation of the lifted-shard/successor-history fold.
    pub(in crate::zk::kagemusha_v1_recursion) fn successor(self) -> ClaimFoldTranscriptModeV1 {
        self.successor
    }

    /// Exact fold contribution to the complete emitted-graph inventory regression.
    #[cfg(test)]
    pub(in crate::zk::kagemusha_v1_recursion) fn native_permutation_count(self) -> usize {
        [self.parent, self.successor]
            .into_iter()
            .filter(|mode| *mode == ClaimFoldTranscriptModeV1::Native)
            .count()
            * FOLD_PERMUTATIONS
    }
}

struct ClaimNativeFoldTranscript<'jobs, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    loader: DeferredLoader<'chip, C>,
    stream: R,
    loaded_stream: Vec<TranscriptObject<C, DeferredLoader<'chip, C>>>,
    sponge: PastaNativePoseidonTranscriptV1<'jobs, C::ScalarExt>,
}

impl<'jobs, 'chip, C, R> ClaimNativeFoldTranscript<'jobs, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn new(
        loader: &DeferredLoader<'chip, C>,
        stream: R,
        jobs: &'jobs mut PastaNativePoseidonJobsV1<C::ScalarExt>,
    ) -> Result<Self, Error> {
        let sponge = jobs
            .begin_transcript(loader.ctx_mut().main(), &FOLD_SQUEEZE_INPUTS)
            .map_err(transcript_error)?;
        Ok(Self {
            loader: loader.clone(),
            stream,
            loaded_stream: Vec::new(),
            sponge,
        })
    }

    fn finish(self) -> Result<AssignedValue<C::ScalarExt>, Error> {
        self.sponge.finish().map_err(transcript_error)
    }
}

impl<'chip, C, R> Transcript<C, DeferredLoader<'chip, C>>
    for ClaimNativeFoldTranscript<'_, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn loader(&self) -> &DeferredLoader<'chip, C> {
        &self.loader
    }

    fn squeeze_challenge(&mut self) -> DeferredScalar<'chip, C> {
        let chip = self.loader.ecc_chip();
        let mut ctx = self.loader.ctx_mut();
        let output = self.sponge.squeeze(ctx.main(), chip.range().gate());
        self.loader.scalar_from_assigned(output)
    }

    fn common_scalar(&mut self, scalar: &DeferredScalar<'chip, C>) -> Result<(), Error> {
        let cell = *scalar.assigned();
        self.sponge.absorb(&[cell]).map_err(transcript_error)
    }

    fn common_ec_point(&mut self, point: &DeferredEcPoint<'chip, C>) -> Result<(), Error> {
        let encoded = self
            .loader
            .ecc_chip()
            .encode(&mut self.loader.ctx_mut(), &point.assigned())
            .map_err(|_| {
                Error::Transcript(
                    io::ErrorKind::Other,
                    "Failed to encode elliptic curve point into native field elements".to_owned(),
                )
            })?;
        self.sponge.absorb(&encoded).map_err(transcript_error)
    }
}

impl<'chip, C, R> TranscriptRead<C, DeferredLoader<'chip, C>>
    for ClaimNativeFoldTranscript<'_, 'chip, C, R>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    R: Read,
{
    fn read_scalar(&mut self) -> Result<DeferredScalar<'chip, C>, Error> {
        let mut data = <C::ScalarExt as ff::PrimeField>::Repr::default();
        self.stream
            .read_exact(data.as_mut())
            .map_err(|err| Error::Transcript(err.kind(), err.to_string()))?;
        let scalar = C::ScalarExt::from_repr_vartime(data).ok_or_else(|| {
            Error::Transcript(
                io::ErrorKind::Other,
                "Invalid scalar encoding in proof".to_owned(),
            )
        })?;
        let scalar = self.loader.assign_scalar(scalar);
        self.loaded_stream
            .push(TranscriptObject::Scalar(scalar.clone()));
        self.common_scalar(&scalar)?;
        Ok(scalar)
    }

    fn read_ec_point(&mut self) -> Result<DeferredEcPoint<'chip, C>, Error> {
        let mut compressed = C::Repr::default();
        self.stream
            .read_exact(compressed.as_mut())
            .map_err(|err| Error::Transcript(err.kind(), err.to_string()))?;
        let point = Option::<C>::from(C::from_bytes(&compressed)).ok_or_else(|| {
            Error::Transcript(
                io::ErrorKind::Other,
                "Invalid elliptic curve point encoding in proof".to_owned(),
            )
        })?;
        let point = self.loader.assign_ec_point(point);
        self.loaded_stream
            .push(TranscriptObject::EcPoint(point.clone()));
        self.common_ec_point(&point)?;
        Ok(point)
    }
}

/// Verify a complete Claim fold using its preselected constrained implementation.
///
/// Native reads, challenges, deferred equations and final binding follow the original helper.
/// Failed parsing or reservation completion aborts the whole build; it never switches modes.
pub(in crate::zk::kagemusha_v1_recursion) fn verify_claim_fold_with_transcript_binding_v1<
    'chip,
    C,
>(
    loader: &DeferredLoader<'chip, C>,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    inputs: &[DeferredAccumulator<'chip, C>],
    proof_bytes: &[u8],
    mode: ClaimFoldTranscriptModeV1,
    jobs: &mut PastaNativePoseidonJobsV1<C::ScalarExt>,
) -> Result<KagemushaAssignedFoldV1<'chip, C>, Error>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if mode == ClaimFoldTranscriptModeV1::Base {
        return verify_fold_with_transcript_binding_v1(loader, succinct_vk, inputs, proof_bytes);
    }
    validate_zk_ipa_succinct_key_v1(succinct_vk, KagemushaIpaProofKindV1::Fold)?;
    if succinct_vk.domain.k != KAGEMUSHA_RECURSION_IPA_K_V1 as usize {
        return Err(transcript_error(
            "Kagemusha fold proof requires the fixed zero-knowledge IPA key and domain",
        ));
    }
    if inputs.len() != 2 || proof_bytes.len() != KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1 {
        return Err(transcript_error(
            "Kagemusha BGH19 fold has the wrong input or byte count",
        ));
    }
    let (reader, position) = ExactReader::new(proof_bytes);
    let mut transcript = ClaimNativeFoldTranscript::new(loader, reader, jobs)?;
    let parsed = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::read_proof(
        succinct_vk,
        inputs,
        &mut transcript,
    )?;
    let accumulated = <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::verify(
        succinct_vk,
        inputs,
        &parsed,
    )?;
    if position.get() != proof_bytes.len() {
        return Err(transcript_error("Kagemusha BGH19 fold has trailing bytes"));
    }
    let _ = transcript.squeeze_challenge();
    let transcript_binding = transcript.finish()?;
    Ok(KagemushaAssignedFoldV1 {
        accumulator: accumulated,
        transcript_binding,
    })
}

#[cfg(test)]
#[path = "deferred_claim_fold_transcript_tests.rs"]
mod tests;
