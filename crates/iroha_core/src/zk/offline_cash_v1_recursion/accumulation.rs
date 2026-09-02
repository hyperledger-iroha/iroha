//! Native fixed-degree IPA accumulation and terminal decisions.
//!
//! The 544-byte wire is exactly sixteen canonical 32-byte scalar challenges followed by one
//! canonical compressed non-identity curve point. Parity is carried by the Rust type and the
//! authenticated circuit role, so no dynamic version, degree, or parity selector is present in
//! the accumulator bytes.

use ff::{Field, PrimeField};
use halo2_proofs::{
    halo2curves::{
        CurveExt as _,
        group::{Curve as _, Group as _, GroupEncoding},
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
        ipa::{
            Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey, IpaSuccinctVerifyingKey,
        },
    },
    system::halo2::transcript::halo2::PoseidonTranscript,
    util::arithmetic::{Domain, root_of_unity},
};

use super::{
    OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1,
    OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1, OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_RATE_V1, OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1,
    OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1, OFFLINE_CASH_RECURSION_IPA_K_V1, OfflineCashPastaParityV1,
    OfflineCashRecursionErrorV1,
};

const ACCUMULATOR_CHALLENGE_BYTES: usize = 32;
const ACCUMULATOR_POINT_BYTES: usize = 32;
const ACCUMULATOR_ROUNDS: usize = OFFLINE_CASH_RECURSION_IPA_K_V1 as usize;
const ACCUMULATOR_BYTES: usize =
    ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES + ACCUMULATOR_POINT_BYTES;

const POSEIDON_WIDTH: usize = OFFLINE_CASH_IPA_POSEIDON_WIDTH_V1;
const POSEIDON_RATE: usize = OFFLINE_CASH_IPA_POSEIDON_RATE_V1;
const POSEIDON_FULL_ROUNDS: usize = OFFLINE_CASH_IPA_POSEIDON_FULL_ROUNDS_V1;
const POSEIDON_PARTIAL_ROUNDS: usize = OFFLINE_CASH_IPA_POSEIDON_PARTIAL_ROUNDS_V1;
const POSEIDON_SECURE_MDS: usize = OFFLINE_CASH_IPA_POSEIDON_SECURE_MDS_V1;

type EqAccumulation = IpaAs<EqAffine, Bgh19>;
type EpAccumulation = IpaAs<EpAffine, Bgh19>;
type EqTranscript<S> = PoseidonTranscript<
    EqAffine,
    NativeLoader,
    S,
    POSEIDON_WIDTH,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;
type EpTranscript<S> = PoseidonTranscript<
    EpAffine,
    NativeLoader,
    S,
    POSEIDON_WIDTH,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;

/// Canonical fixed-size Eq/Fp delayed-history accumulator.
#[derive(Clone, PartialEq, Eq)]
pub struct OfflineCashEqAccumulatorV1([u8; ACCUMULATOR_BYTES]);

impl core::fmt::Debug for OfflineCashEqAccumulatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEqAccumulatorV1")
            .field("bytes", &ACCUMULATOR_BYTES)
            .finish()
    }
}

impl OfflineCashEqAccumulatorV1 {
    /// Strictly decode exactly 544 bytes without scalar reduction or point normalization.
    ///
    /// # Errors
    ///
    /// Rejects any other length, non-canonical `Fp` scalar, invalid compressed Eq point, or the
    /// Eq identity.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, OfflineCashRecursionErrorV1> {
        validate_accumulator_length(OfflineCashPastaParityV1::Eq, bytes)?;
        let raw: [u8; ACCUMULATOR_BYTES] = bytes.try_into().map_err(|_| {
            OfflineCashRecursionErrorV1::InvalidAccumulatorLength {
                parity: OfflineCashPastaParityV1::Eq,
                actual: bytes.len(),
                expected: ACCUMULATOR_BYTES,
            }
        })?;
        parse_eq(&raw)?;
        Ok(Self(raw))
    }

    /// Encode a native Eq accumulator in the sole canonical 544-byte layout.
    ///
    /// # Errors
    ///
    /// Rejects a native accumulator with a non-fixed round count or an identity point.
    pub fn from_native(
        accumulator: &IpaAccumulator<EqAffine, NativeLoader>,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        if accumulator.xi.len() != ACCUMULATOR_ROUNDS {
            return Err(OfflineCashRecursionErrorV1::InvalidAccumulatorRounds {
                parity: OfflineCashPastaParityV1::Eq,
                actual: accumulator.xi.len(),
            });
        }
        let mut raw = [0_u8; ACCUMULATOR_BYTES];
        for (index, scalar) in accumulator.xi.iter().enumerate() {
            let start = index * ACCUMULATOR_CHALLENGE_BYTES;
            raw[start..start + ACCUMULATOR_CHALLENGE_BYTES]
                .copy_from_slice(scalar.to_repr().as_ref());
        }
        raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
            .copy_from_slice(accumulator.u.to_bytes().as_ref());
        parse_eq(&raw)?;
        Ok(Self(raw))
    }

    /// Borrow the exact canonical bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] {
        &self.0
    }

    /// Parse the canonical wire into the native Eq accumulator used by BGH19.
    ///
    /// # Errors
    ///
    /// Rejects any non-canonical scalar or point. Values constructed by this type are expected to
    /// parse successfully; the result remains fallible to keep the proof boundary explicit.
    pub fn to_native(
        &self,
    ) -> Result<IpaAccumulator<EqAffine, NativeLoader>, OfflineCashRecursionErrorV1> {
        parse_eq(&self.0)
    }
}

/// Canonical fixed-size Ep/Fq delayed-history accumulator.
#[derive(Clone, PartialEq, Eq)]
pub struct OfflineCashEpAccumulatorV1([u8; ACCUMULATOR_BYTES]);

impl core::fmt::Debug for OfflineCashEpAccumulatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEpAccumulatorV1")
            .field("bytes", &ACCUMULATOR_BYTES)
            .finish()
    }
}

impl OfflineCashEpAccumulatorV1 {
    /// Strictly decode exactly 544 bytes without scalar reduction or point normalization.
    ///
    /// # Errors
    ///
    /// Rejects any other length, non-canonical `Fq` scalar, invalid compressed Ep point, or the
    /// Ep identity.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, OfflineCashRecursionErrorV1> {
        validate_accumulator_length(OfflineCashPastaParityV1::Ep, bytes)?;
        let raw: [u8; ACCUMULATOR_BYTES] = bytes.try_into().map_err(|_| {
            OfflineCashRecursionErrorV1::InvalidAccumulatorLength {
                parity: OfflineCashPastaParityV1::Ep,
                actual: bytes.len(),
                expected: ACCUMULATOR_BYTES,
            }
        })?;
        parse_ep(&raw)?;
        Ok(Self(raw))
    }

    /// Encode a native Ep accumulator in the sole canonical 544-byte layout.
    ///
    /// # Errors
    ///
    /// Rejects a native accumulator with a non-fixed round count or an identity point.
    pub fn from_native(
        accumulator: &IpaAccumulator<EpAffine, NativeLoader>,
    ) -> Result<Self, OfflineCashRecursionErrorV1> {
        if accumulator.xi.len() != ACCUMULATOR_ROUNDS {
            return Err(OfflineCashRecursionErrorV1::InvalidAccumulatorRounds {
                parity: OfflineCashPastaParityV1::Ep,
                actual: accumulator.xi.len(),
            });
        }
        let mut raw = [0_u8; ACCUMULATOR_BYTES];
        for (index, scalar) in accumulator.xi.iter().enumerate() {
            let start = index * ACCUMULATOR_CHALLENGE_BYTES;
            raw[start..start + ACCUMULATOR_CHALLENGE_BYTES]
                .copy_from_slice(scalar.to_repr().as_ref());
        }
        raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
            .copy_from_slice(accumulator.u.to_bytes().as_ref());
        parse_ep(&raw)?;
        Ok(Self(raw))
    }

    /// Borrow the exact canonical bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] {
        &self.0
    }

    /// Parse the canonical wire into the native Ep accumulator used by BGH19.
    ///
    /// # Errors
    ///
    /// Rejects any non-canonical scalar or point. Values constructed by this type are expected to
    /// parse successfully; the result remains fallible to keep the proof boundary explicit.
    pub fn to_native(
        &self,
    ) -> Result<IpaAccumulator<EpAffine, NativeLoader>, OfflineCashRecursionErrorV1> {
        parse_ep(&self.0)
    }
}

/// Exact fixed-profile Eq BGH19 fold transcript.
#[derive(Clone, PartialEq, Eq)]
pub struct OfflineCashEqFoldProofV1([u8; OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1]);

impl core::fmt::Debug for OfflineCashEqFoldProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEqFoldProofV1")
            .field("bytes", &OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1)
            .finish()
    }
}

impl OfflineCashEqFoldProofV1 {
    /// Strictly construct an Eq fold proof from exactly 1,280 transcript bytes.
    ///
    /// # Errors
    ///
    /// Rejects every non-fixed length. Transcript elements are parsed and authenticated by the
    /// native verifier, never normalized here.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, OfflineCashRecursionErrorV1> {
        let raw =
            bytes
                .try_into()
                .map_err(|_| OfflineCashRecursionErrorV1::InvalidFoldProofLength {
                    parity: OfflineCashPastaParityV1::Eq,
                    actual: bytes.len(),
                    expected: OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1,
                })?;
        Ok(Self(raw))
    }

    /// Borrow the exact fold transcript bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1] {
        &self.0
    }
}

/// Exact fixed-profile Ep BGH19 fold transcript.
#[derive(Clone, PartialEq, Eq)]
pub struct OfflineCashEpFoldProofV1([u8; OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1]);

impl core::fmt::Debug for OfflineCashEpFoldProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashEpFoldProofV1")
            .field("bytes", &OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1)
            .finish()
    }
}

impl OfflineCashEpFoldProofV1 {
    /// Strictly construct an Ep fold proof from exactly 1,280 transcript bytes.
    ///
    /// # Errors
    ///
    /// Rejects every non-fixed length. Transcript elements are parsed and authenticated by the
    /// native verifier, never normalized here.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, OfflineCashRecursionErrorV1> {
        let raw =
            bytes
                .try_into()
                .map_err(|_| OfflineCashRecursionErrorV1::InvalidFoldProofLength {
                    parity: OfflineCashPastaParityV1::Ep,
                    actual: bytes.len(),
                    expected: OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1,
                })?;
        Ok(Self(raw))
    }

    /// Borrow the exact fold transcript bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1] {
        &self.0
    }
}

/// Eq successor accumulator and the one-predecessor BGH19 fold which created it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashEqFoldOutputV1 {
    successor: OfflineCashEqAccumulatorV1,
    proof: OfflineCashEqFoldProofV1,
}

impl OfflineCashEqFoldOutputV1 {
    /// Assemble a claimed Eq successor and exact fold transcript for verification.
    #[must_use]
    pub const fn from_parts(
        successor: OfflineCashEqAccumulatorV1,
        proof: OfflineCashEqFoldProofV1,
    ) -> Self {
        Self { successor, proof }
    }

    /// Borrow the folded Eq successor accumulator.
    #[must_use]
    pub const fn successor(&self) -> &OfflineCashEqAccumulatorV1 {
        &self.successor
    }

    /// Borrow the exact Eq fold transcript.
    #[must_use]
    pub const fn proof(&self) -> &OfflineCashEqFoldProofV1 {
        &self.proof
    }
}

/// Ep successor accumulator and the one-predecessor BGH19 fold which created it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashEpFoldOutputV1 {
    successor: OfflineCashEpAccumulatorV1,
    proof: OfflineCashEpFoldProofV1,
}

impl OfflineCashEpFoldOutputV1 {
    /// Assemble a claimed Ep successor and exact fold transcript for verification.
    #[must_use]
    pub const fn from_parts(
        successor: OfflineCashEpAccumulatorV1,
        proof: OfflineCashEpFoldProofV1,
    ) -> Self {
        Self { successor, proof }
    }

    /// Borrow the folded Ep successor accumulator.
    #[must_use]
    pub const fn successor(&self) -> &OfflineCashEpAccumulatorV1 {
        &self.successor
    }

    /// Borrow the exact Ep fold transcript.
    #[must_use]
    pub const fn proof(&self) -> &OfflineCashEpFoldProofV1 {
        &self.proof
    }
}

/// Fold exactly one current Eq opening claim with exactly one predecessor history accumulator.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, malformed accumulators, a backend failure, or a transcript whose
/// size differs from the fixed profile.
pub fn fold_offline_cash_eq_accumulators_v1(
    params: &ParamsIPA<EqAffine>,
    current: &OfflineCashEqAccumulatorV1,
    predecessor: &OfflineCashEqAccumulatorV1,
) -> Result<OfflineCashEqFoldOutputV1, OfflineCashRecursionErrorV1> {
    validate_eq_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let proving_key = eq_proving_key(params);
    let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let successor = <EqAccumulation as AccumulationSchemeProver<EqAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rand_core_06::OsRng,
    )
    .map_err(|error| OfflineCashRecursionErrorV1::FoldCreation {
        parity: OfflineCashPastaParityV1::Eq,
        reason: format!("{error:?}"),
    })?;
    let proof = OfflineCashEqFoldProofV1::try_from_bytes(&transcript.finalize())?;
    Ok(OfflineCashEqFoldOutputV1 {
        successor: OfflineCashEqAccumulatorV1::from_native(&successor)?,
        proof,
    })
}

/// Fold exactly one current Ep opening claim with exactly one predecessor history accumulator.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, malformed accumulators, a backend failure, or a transcript whose
/// size differs from the fixed profile.
pub fn fold_offline_cash_ep_accumulators_v1(
    params: &ParamsIPA<EpAffine>,
    current: &OfflineCashEpAccumulatorV1,
    predecessor: &OfflineCashEpAccumulatorV1,
) -> Result<OfflineCashEpFoldOutputV1, OfflineCashRecursionErrorV1> {
    validate_ep_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let proving_key = ep_proving_key(params);
    let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let successor = <EpAccumulation as AccumulationSchemeProver<EpAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rand_core_06::OsRng,
    )
    .map_err(|error| OfflineCashRecursionErrorV1::FoldCreation {
        parity: OfflineCashPastaParityV1::Ep,
        reason: format!("{error:?}"),
    })?;
    let proof = OfflineCashEpFoldProofV1::try_from_bytes(&transcript.finalize())?;
    Ok(OfflineCashEpFoldOutputV1 {
        successor: OfflineCashEpAccumulatorV1::from_native(&successor)?,
        proof,
    })
}

/// Verify and terminally decide one exact Eq predecessor/current fold.
///
/// # Errors
///
/// Rejects malformed inputs, proof substitution, an invalid BGH19 relation, a trailing transcript,
/// a false terminal accumulator relation, or a substituted successor.
pub fn verify_and_decide_offline_cash_eq_fold_v1(
    params: &ParamsIPA<EqAffine>,
    current: &OfflineCashEqAccumulatorV1,
    predecessor: &OfflineCashEqAccumulatorV1,
    fold: &OfflineCashEqFoldOutputV1,
) -> Result<(), OfflineCashRecursionErrorV1> {
    validate_eq_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let deciding_key = eq_deciding_key(params);
    let cursor = std::io::Cursor::new(fold.proof.0.to_vec());
    let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
    let parsed = catch_native_verifier_panic(OfflineCashPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Eq,
        reason: format!("proof parse failed: {error:?}"),
    })?;
    let successor = catch_native_verifier_panic(OfflineCashPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Eq,
        reason: format!("proof relation failed: {error:?}"),
    })?;
    ensure_transcript_consumed(
        OfflineCashPastaParityV1::Eq,
        transcript.finalize().position(),
    )?;
    decide_eq_native(&deciding_key, successor.clone())?;
    if OfflineCashEqAccumulatorV1::from_native(&successor)? != fold.successor {
        return Err(OfflineCashRecursionErrorV1::FoldSuccessorSubstitution(
            OfflineCashPastaParityV1::Eq,
        ));
    }
    Ok(())
}

/// Verify and terminally decide one exact Ep predecessor/current fold.
///
/// # Errors
///
/// Rejects malformed inputs, proof substitution, an invalid BGH19 relation, a trailing transcript,
/// a false terminal accumulator relation, or a substituted successor.
pub fn verify_and_decide_offline_cash_ep_fold_v1(
    params: &ParamsIPA<EpAffine>,
    current: &OfflineCashEpAccumulatorV1,
    predecessor: &OfflineCashEpAccumulatorV1,
    fold: &OfflineCashEpFoldOutputV1,
) -> Result<(), OfflineCashRecursionErrorV1> {
    validate_ep_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let deciding_key = ep_deciding_key(params);
    let cursor = std::io::Cursor::new(fold.proof.0.to_vec());
    let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
    let parsed = catch_native_verifier_panic(OfflineCashPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Ep,
        reason: format!("proof parse failed: {error:?}"),
    })?;
    let successor = catch_native_verifier_panic(OfflineCashPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Ep,
        reason: format!("proof relation failed: {error:?}"),
    })?;
    ensure_transcript_consumed(
        OfflineCashPastaParityV1::Ep,
        transcript.finalize().position(),
    )?;
    decide_ep_native(&deciding_key, successor.clone())?;
    if OfflineCashEpAccumulatorV1::from_native(&successor)? != fold.successor {
        return Err(OfflineCashRecursionErrorV1::FoldSuccessorSubstitution(
            OfflineCashPastaParityV1::Ep,
        ));
    }
    Ok(())
}

/// Terminally decide one canonical Eq accumulator under the fixed authenticated parameters.
///
/// # Errors
///
/// Rejects non-fixed parameters, malformed encoding, or a false accumulator relation.
pub fn decide_offline_cash_eq_accumulator_v1(
    params: &ParamsIPA<EqAffine>,
    accumulator: &OfflineCashEqAccumulatorV1,
) -> Result<(), OfflineCashRecursionErrorV1> {
    validate_eq_params(params)?;
    decide_eq_native(&eq_deciding_key(params), accumulator.to_native()?)
}

/// Terminally decide one canonical Ep accumulator under the fixed authenticated parameters.
///
/// # Errors
///
/// Rejects non-fixed parameters, malformed encoding, or a false accumulator relation.
pub fn decide_offline_cash_ep_accumulator_v1(
    params: &ParamsIPA<EpAffine>,
    accumulator: &OfflineCashEpAccumulatorV1,
) -> Result<(), OfflineCashRecursionErrorV1> {
    validate_ep_params(params)?;
    decide_ep_native(&ep_deciding_key(params), accumulator.to_native()?)
}

/// Construct the canonical terminally valid empty Eq history accumulator.
///
/// The fixed challenge vector is all ones. Therefore every coefficient produced by the IPA
/// decider's `h_coeffs` expansion is one and the committed point is the sum of the complete
/// authenticated parameter basis. This gives bootstrap a real decided accumulator without
/// asserting any predecessor proof.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, an unexpected identity commitment, or a failed terminal check.
pub fn initial_offline_cash_eq_accumulator_v1(
    params: &ParamsIPA<EqAffine>,
) -> Result<OfflineCashEqAccumulatorV1, OfflineCashRecursionErrorV1> {
    validate_eq_params(params)?;
    let point = params
        .get_g()
        .iter()
        .fold(Eq::identity(), |sum, base| sum + *base)
        .to_affine();
    let accumulator = OfflineCashEqAccumulatorV1::from_native(&IpaAccumulator::new(
        vec![Fp::ONE; ACCUMULATOR_ROUNDS],
        point,
    ))?;
    decide_offline_cash_eq_accumulator_v1(params, &accumulator)?;
    Ok(accumulator)
}

/// Construct the canonical terminally valid empty Ep history accumulator.
///
/// This is the reciprocal-parity analogue of
/// [`initial_offline_cash_eq_accumulator_v1`]. It is terminally decided before it can become a
/// bootstrap witness.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, an unexpected identity commitment, or a failed terminal check.
pub fn initial_offline_cash_ep_accumulator_v1(
    params: &ParamsIPA<EpAffine>,
) -> Result<OfflineCashEpAccumulatorV1, OfflineCashRecursionErrorV1> {
    validate_ep_params(params)?;
    let point = params
        .get_g()
        .iter()
        .fold(Ep::identity(), |sum, base| sum + *base)
        .to_affine();
    let accumulator = OfflineCashEpAccumulatorV1::from_native(&IpaAccumulator::new(
        vec![Fq::ONE; ACCUMULATOR_ROUNDS],
        point,
    ))?;
    decide_offline_cash_ep_accumulator_v1(params, &accumulator)?;
    Ok(accumulator)
}

fn validate_accumulator_length(
    parity: OfflineCashPastaParityV1,
    bytes: &[u8],
) -> Result<(), OfflineCashRecursionErrorV1> {
    if bytes.len() != ACCUMULATOR_BYTES {
        return Err(OfflineCashRecursionErrorV1::InvalidAccumulatorLength {
            parity,
            actual: bytes.len(),
            expected: ACCUMULATOR_BYTES,
        });
    }
    Ok(())
}

fn parse_eq(
    raw: &[u8; ACCUMULATOR_BYTES],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, OfflineCashRecursionErrorV1> {
    use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

    let mut challenges = Vec::with_capacity(ACCUMULATOR_ROUNDS);
    for (round, chunk) in raw[..ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES]
        .chunks_exact(ACCUMULATOR_CHALLENGE_BYTES)
        .enumerate()
    {
        let repr: [u8; 32] = chunk.try_into().expect("fixed challenge chunk");
        let scalar = Option::<Fp>::from(Fp::from_repr(repr.into())).ok_or(
            OfflineCashRecursionErrorV1::NonCanonicalAccumulatorScalar {
                parity: OfflineCashPastaParityV1::Eq,
                round,
            },
        )?;
        challenges.push(scalar);
    }
    let point_bytes: [u8; 32] = raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
        .try_into()
        .expect("fixed point chunk");
    let point = Option::<EqAffine>::from(EqAffine::from_bytes(&point_bytes.into())).ok_or(
        OfflineCashRecursionErrorV1::InvalidAccumulatorPoint(OfflineCashPastaParityV1::Eq),
    )?;
    if bool::from(point.is_identity()) {
        return Err(OfflineCashRecursionErrorV1::InvalidAccumulatorPoint(
            OfflineCashPastaParityV1::Eq,
        ));
    }
    Ok(IpaAccumulator::new(challenges, point))
}

fn parse_ep(
    raw: &[u8; ACCUMULATOR_BYTES],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, OfflineCashRecursionErrorV1> {
    use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

    let mut challenges = Vec::with_capacity(ACCUMULATOR_ROUNDS);
    for (round, chunk) in raw[..ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES]
        .chunks_exact(ACCUMULATOR_CHALLENGE_BYTES)
        .enumerate()
    {
        let repr: [u8; 32] = chunk.try_into().expect("fixed challenge chunk");
        let scalar = Option::<Fq>::from(Fq::from_repr(repr.into())).ok_or(
            OfflineCashRecursionErrorV1::NonCanonicalAccumulatorScalar {
                parity: OfflineCashPastaParityV1::Ep,
                round,
            },
        )?;
        challenges.push(scalar);
    }
    let point_bytes: [u8; 32] = raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
        .try_into()
        .expect("fixed point chunk");
    let point = Option::<EpAffine>::from(EpAffine::from_bytes(&point_bytes.into())).ok_or(
        OfflineCashRecursionErrorV1::InvalidAccumulatorPoint(OfflineCashPastaParityV1::Ep),
    )?;
    if bool::from(point.is_identity()) {
        return Err(OfflineCashRecursionErrorV1::InvalidAccumulatorPoint(
            OfflineCashPastaParityV1::Ep,
        ));
    }
    Ok(IpaAccumulator::new(challenges, point))
}

fn validate_eq_params(params: &ParamsIPA<EqAffine>) -> Result<(), OfflineCashRecursionErrorV1> {
    if params.k() != OFFLINE_CASH_RECURSION_IPA_K_V1 {
        return Err(OfflineCashRecursionErrorV1::InvalidIpaParameters {
            parity: OfflineCashPastaParityV1::Eq,
            actual: params.k(),
        });
    }
    Ok(())
}

fn validate_ep_params(params: &ParamsIPA<EpAffine>) -> Result<(), OfflineCashRecursionErrorV1> {
    if params.k() != OFFLINE_CASH_RECURSION_IPA_K_V1 {
        return Err(OfflineCashRecursionErrorV1::InvalidIpaParameters {
            parity: OfflineCashPastaParityV1::Ep,
            actual: params.k(),
        });
    }
    Ok(())
}

fn eq_proving_key(params: &ParamsIPA<EqAffine>) -> IpaProvingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    IpaProvingKey::new(
        Domain::new(
            OFFLINE_CASH_RECURSION_IPA_K_V1 as usize,
            root_of_unity(OFFLINE_CASH_RECURSION_IPA_K_V1 as usize),
        ),
        params.get_g().to_vec(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

fn ep_proving_key(params: &ParamsIPA<EpAffine>) -> IpaProvingKey<EpAffine> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    IpaProvingKey::new(
        Domain::new(
            OFFLINE_CASH_RECURSION_IPA_K_V1 as usize,
            root_of_unity(OFFLINE_CASH_RECURSION_IPA_K_V1 as usize),
        ),
        params.get_g().to_vec(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    )
}

fn eq_deciding_key(params: &ParamsIPA<EqAffine>) -> IpaDecidingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    IpaDecidingKey::new(
        IpaSuccinctVerifyingKey::new(
            Domain::new(
                OFFLINE_CASH_RECURSION_IPA_K_V1 as usize,
                root_of_unity(OFFLINE_CASH_RECURSION_IPA_K_V1 as usize),
            ),
            params.get_g()[0],
            hash_to_curve(&[2]).to_affine(),
            Some(hash_to_curve(&[1]).to_affine()),
        ),
        params.get_g().to_vec(),
    )
}

fn ep_deciding_key(params: &ParamsIPA<EpAffine>) -> IpaDecidingKey<EpAffine> {
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    IpaDecidingKey::new(
        IpaSuccinctVerifyingKey::new(
            Domain::new(
                OFFLINE_CASH_RECURSION_IPA_K_V1 as usize,
                root_of_unity(OFFLINE_CASH_RECURSION_IPA_K_V1 as usize),
            ),
            params.get_g()[0],
            hash_to_curve(&[2]).to_affine(),
            Some(hash_to_curve(&[1]).to_affine()),
        ),
        params.get_g().to_vec(),
    )
}

fn decide_eq_native(
    deciding_key: &IpaDecidingKey<EqAffine>,
    accumulator: IpaAccumulator<EqAffine, NativeLoader>,
) -> Result<(), OfflineCashRecursionErrorV1> {
    catch_native_verifier_panic(OfflineCashPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationDecider<EqAffine, NativeLoader>>::decide(
            deciding_key,
            accumulator,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Eq,
        reason: format!("terminal decision failed: {error:?}"),
    })
}

fn decide_ep_native(
    deciding_key: &IpaDecidingKey<EpAffine>,
    accumulator: IpaAccumulator<EpAffine, NativeLoader>,
) -> Result<(), OfflineCashRecursionErrorV1> {
    catch_native_verifier_panic(OfflineCashPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationDecider<EpAffine, NativeLoader>>::decide(
            deciding_key,
            accumulator,
        )
    })?
    .map_err(|error| OfflineCashRecursionErrorV1::FoldDecision {
        parity: OfflineCashPastaParityV1::Ep,
        reason: format!("terminal decision failed: {error:?}"),
    })
}

fn ensure_transcript_consumed(
    parity: OfflineCashPastaParityV1,
    consumed: u64,
) -> Result<(), OfflineCashRecursionErrorV1> {
    if consumed
        != u64::try_from(OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1)
            .map_err(|_| OfflineCashRecursionErrorV1::LengthOverflow)?
    {
        return Err(OfflineCashRecursionErrorV1::FoldDecision {
            parity,
            reason: "fold transcript has trailing or unconsumed bytes".to_owned(),
        });
    }
    Ok(())
}

fn catch_native_verifier_panic<T>(
    parity: OfflineCashPastaParityV1,
    verify: impl FnOnce() -> T,
) -> Result<T, OfflineCashRecursionErrorV1> {
    crate::panic_hook::catch_unwind_suppressed(verify)
        .map_err(|_| OfflineCashRecursionErrorV1::NativeVerifierPanic(parity))
}

const _: () = assert!(ACCUMULATOR_BYTES == 544);

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::poly::commitment::Params as _;

    #[test]
    fn canonical_empty_histories_are_terminally_decided() {
        let eq_params = ParamsIPA::<EqAffine>::new(OFFLINE_CASH_RECURSION_IPA_K_V1);
        let ep_params = ParamsIPA::<EpAffine>::new(OFFLINE_CASH_RECURSION_IPA_K_V1);
        let eq = initial_offline_cash_eq_accumulator_v1(&eq_params).expect("Eq empty history");
        let ep = initial_offline_cash_ep_accumulator_v1(&ep_params).expect("Ep empty history");

        decide_offline_cash_eq_accumulator_v1(&eq_params, &eq).expect("decided Eq history");
        decide_offline_cash_ep_accumulator_v1(&ep_params, &ep).expect("decided Ep history");
        assert_eq!(
            eq.to_native().expect("Eq native").xi,
            vec![Fp::ONE; ACCUMULATOR_ROUNDS]
        );
        assert_eq!(
            ep.to_native().expect("Ep native").xi,
            vec![Fq::ONE; ACCUMULATOR_ROUNDS]
        );
    }
}
