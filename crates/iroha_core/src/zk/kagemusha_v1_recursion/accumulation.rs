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
use iroha_crypto::kagemusha::KagemushaRecoverySeedV1;
use sha2::{Digest as _, Sha256};
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
    KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1, KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1, KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaPastaParityV1,
    KagemushaRecursionErrorV1,
};

const ACCUMULATOR_CHALLENGE_BYTES: usize = 32;
const ACCUMULATOR_POINT_BYTES: usize = 32;
const ACCUMULATOR_ROUNDS: usize = KAGEMUSHA_RECURSION_IPA_K_V1 as usize;
const ACCUMULATOR_BYTES: usize =
    ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES + ACCUMULATOR_POINT_BYTES;

const POSEIDON_WIDTH: usize = KAGEMUSHA_IPA_POSEIDON_WIDTH_V1;
const POSEIDON_RATE: usize = KAGEMUSHA_IPA_POSEIDON_RATE_V1;
const POSEIDON_FULL_ROUNDS: usize = KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1;
const POSEIDON_PARTIAL_ROUNDS: usize = KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1;
const POSEIDON_SECURE_MDS: usize = KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1;

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
pub struct KagemushaEqAccumulatorV1([u8; ACCUMULATOR_BYTES]);

impl core::fmt::Debug for KagemushaEqAccumulatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaEqAccumulatorV1")
            .field("bytes", &ACCUMULATOR_BYTES)
            .finish()
    }
}

impl KagemushaEqAccumulatorV1 {
    /// Strictly decode exactly 544 bytes without scalar reduction or point normalization.
    ///
    /// # Errors
    ///
    /// Rejects any other length, non-canonical `Fp` scalar, invalid compressed Eq point, or the
    /// Eq identity.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, KagemushaRecursionErrorV1> {
        validate_accumulator_length(KagemushaPastaParityV1::Eq, bytes)?;
        let raw: [u8; ACCUMULATOR_BYTES] =
            bytes
                .try_into()
                .map_err(|_| KagemushaRecursionErrorV1::InvalidAccumulatorLength {
                    parity: KagemushaPastaParityV1::Eq,
                    actual: bytes.len(),
                    expected: ACCUMULATOR_BYTES,
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
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if accumulator.xi.len() != ACCUMULATOR_ROUNDS {
            return Err(KagemushaRecursionErrorV1::InvalidAccumulatorRounds {
                parity: KagemushaPastaParityV1::Eq,
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
    pub const fn as_bytes(&self) -> &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] {
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
    ) -> Result<IpaAccumulator<EqAffine, NativeLoader>, KagemushaRecursionErrorV1> {
        parse_eq(&self.0)
    }
}

/// Canonical fixed-size Ep/Fq delayed-history accumulator.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaEpAccumulatorV1([u8; ACCUMULATOR_BYTES]);

impl core::fmt::Debug for KagemushaEpAccumulatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaEpAccumulatorV1")
            .field("bytes", &ACCUMULATOR_BYTES)
            .finish()
    }
}

impl KagemushaEpAccumulatorV1 {
    /// Strictly decode exactly 544 bytes without scalar reduction or point normalization.
    ///
    /// # Errors
    ///
    /// Rejects any other length, non-canonical `Fq` scalar, invalid compressed Ep point, or the
    /// Ep identity.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, KagemushaRecursionErrorV1> {
        validate_accumulator_length(KagemushaPastaParityV1::Ep, bytes)?;
        let raw: [u8; ACCUMULATOR_BYTES] =
            bytes
                .try_into()
                .map_err(|_| KagemushaRecursionErrorV1::InvalidAccumulatorLength {
                    parity: KagemushaPastaParityV1::Ep,
                    actual: bytes.len(),
                    expected: ACCUMULATOR_BYTES,
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
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if accumulator.xi.len() != ACCUMULATOR_ROUNDS {
            return Err(KagemushaRecursionErrorV1::InvalidAccumulatorRounds {
                parity: KagemushaPastaParityV1::Ep,
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
    pub const fn as_bytes(&self) -> &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1] {
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
    ) -> Result<IpaAccumulator<EpAffine, NativeLoader>, KagemushaRecursionErrorV1> {
        parse_ep(&self.0)
    }
}

/// Exact fixed-profile Eq BGH19 fold transcript.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaEqFoldProofV1([u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]);

impl core::fmt::Debug for KagemushaEqFoldProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaEqFoldProofV1")
            .field("bytes", &KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1)
            .finish()
    }
}

impl KagemushaEqFoldProofV1 {
    /// Strictly construct an Eq fold proof from exactly 1,280 transcript bytes.
    ///
    /// # Errors
    ///
    /// Rejects every non-fixed length. Transcript elements are parsed and authenticated by the
    /// native verifier, never normalized here.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, KagemushaRecursionErrorV1> {
        let raw =
            bytes
                .try_into()
                .map_err(|_| KagemushaRecursionErrorV1::InvalidFoldProofLength {
                    parity: KagemushaPastaParityV1::Eq,
                    actual: bytes.len(),
                    expected: KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
                })?;
        Ok(Self(raw))
    }

    /// Borrow the exact fold transcript bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1] {
        &self.0
    }
}

/// Exact fixed-profile Ep BGH19 fold transcript.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaEpFoldProofV1([u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]);

impl core::fmt::Debug for KagemushaEpFoldProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KagemushaEpFoldProofV1")
            .field("bytes", &KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1)
            .finish()
    }
}

impl KagemushaEpFoldProofV1 {
    /// Strictly construct an Ep fold proof from exactly 1,280 transcript bytes.
    ///
    /// # Errors
    ///
    /// Rejects every non-fixed length. Transcript elements are parsed and authenticated by the
    /// native verifier, never normalized here.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, KagemushaRecursionErrorV1> {
        let raw =
            bytes
                .try_into()
                .map_err(|_| KagemushaRecursionErrorV1::InvalidFoldProofLength {
                    parity: KagemushaPastaParityV1::Ep,
                    actual: bytes.len(),
                    expected: KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
                })?;
        Ok(Self(raw))
    }

    /// Borrow the exact fold transcript bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1] {
        &self.0
    }
}

/// Eq successor accumulator and the one-predecessor BGH19 fold which created it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaEqFoldOutputV1 {
    successor: KagemushaEqAccumulatorV1,
    proof: KagemushaEqFoldProofV1,
}

impl KagemushaEqFoldOutputV1 {
    /// Assemble a claimed Eq successor and exact fold transcript for verification.
    #[must_use]
    pub const fn from_parts(
        successor: KagemushaEqAccumulatorV1,
        proof: KagemushaEqFoldProofV1,
    ) -> Self {
        Self { successor, proof }
    }

    /// Borrow the folded Eq successor accumulator.
    #[must_use]
    pub const fn successor(&self) -> &KagemushaEqAccumulatorV1 {
        &self.successor
    }

    /// Borrow the exact Eq fold transcript.
    #[must_use]
    pub const fn proof(&self) -> &KagemushaEqFoldProofV1 {
        &self.proof
    }
}

/// Ep successor accumulator and the one-predecessor BGH19 fold which created it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaEpFoldOutputV1 {
    successor: KagemushaEpAccumulatorV1,
    proof: KagemushaEpFoldProofV1,
}

impl KagemushaEpFoldOutputV1 {
    /// Assemble a claimed Ep successor and exact fold transcript for verification.
    #[must_use]
    pub const fn from_parts(
        successor: KagemushaEpAccumulatorV1,
        proof: KagemushaEpFoldProofV1,
    ) -> Self {
        Self { successor, proof }
    }

    /// Borrow the folded Ep successor accumulator.
    #[must_use]
    pub const fn successor(&self) -> &KagemushaEpAccumulatorV1 {
        &self.successor
    }

    /// Borrow the exact Ep fold transcript.
    #[must_use]
    pub const fn proof(&self) -> &KagemushaEpFoldProofV1 {
        &self.proof
    }
}

/// Fold exactly one current Eq opening claim with exactly one predecessor history accumulator.
///
/// Randomness is regenerated from the authenticated operation seed and an exact
/// ordered-input/parameter context. Recovering the same operation reproduces the
/// entire fold; this API never falls back to fresh operating-system entropy.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, malformed accumulators, a backend failure, or a transcript whose
/// size differs from the fixed profile.
pub fn fold_kagemusha_eq_accumulators_v1(
    params: &ParamsIPA<EqAffine>,
    current: &KagemushaEqAccumulatorV1,
    predecessor: &KagemushaEqAccumulatorV1,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaEqFoldOutputV1, KagemushaRecursionErrorV1> {
    validate_eq_params(params)?;
    let context = fold_recovery_context_v1(
        KagemushaPastaParityV1::Eq,
        params.get_g().iter().map(GroupEncoding::to_bytes),
        current.as_bytes(),
        predecessor.as_bytes(),
    );
    let rng = recovery_seed
        .rng(b"ipa-accumulator-fold:eq", &context)
        .map_err(|error| KagemushaRecursionErrorV1::FoldCreation {
            parity: KagemushaPastaParityV1::Eq,
            reason: error.to_string(),
        })?;
    fold_kagemusha_eq_accumulators_with_rng_v1(params, current, predecessor, rng)
}

/// Fold Eq inputs with explicitly supplied entropy for the online issuer path.
///
/// Device-state and postcommit recovery callers must use the seed-requiring
/// public entry point. An online issuer may use fresh entropy when it persists
/// the resulting checkpoint before delivering it to a device.
///
/// # Errors
///
/// Rejects malformed parameters/accumulators, backend failure, or wrong proof size.
pub(super) fn fold_kagemusha_eq_accumulators_with_rng_v1(
    params: &ParamsIPA<EqAffine>,
    current: &KagemushaEqAccumulatorV1,
    predecessor: &KagemushaEqAccumulatorV1,
    rng: impl rand_core_06::RngCore + rand_core_06::CryptoRng,
) -> Result<KagemushaEqFoldOutputV1, KagemushaRecursionErrorV1> {
    validate_eq_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let proving_key = eq_proving_key(params);
    let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let successor = <EqAccumulation as AccumulationSchemeProver<EqAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rng,
    )
    .map_err(|error| KagemushaRecursionErrorV1::FoldCreation {
        parity: KagemushaPastaParityV1::Eq,
        reason: format!("{error:?}"),
    })?;
    let proof = KagemushaEqFoldProofV1::try_from_bytes(&transcript.finalize())?;
    Ok(KagemushaEqFoldOutputV1 {
        successor: KagemushaEqAccumulatorV1::from_native(&successor)?,
        proof,
    })
}

/// Fold exactly one current Ep opening claim with exactly one predecessor history accumulator.
///
/// Randomness is regenerated from the authenticated operation seed and an exact
/// ordered-input/parameter context, separate from Eq and other proof purposes.
/// This API never falls back to fresh operating-system entropy.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, malformed accumulators, a backend failure, or a transcript whose
/// size differs from the fixed profile.
pub fn fold_kagemusha_ep_accumulators_v1(
    params: &ParamsIPA<EpAffine>,
    current: &KagemushaEpAccumulatorV1,
    predecessor: &KagemushaEpAccumulatorV1,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaEpFoldOutputV1, KagemushaRecursionErrorV1> {
    validate_ep_params(params)?;
    let context = fold_recovery_context_v1(
        KagemushaPastaParityV1::Ep,
        params.get_g().iter().map(GroupEncoding::to_bytes),
        current.as_bytes(),
        predecessor.as_bytes(),
    );
    let rng = recovery_seed
        .rng(b"ipa-accumulator-fold:ep", &context)
        .map_err(|error| KagemushaRecursionErrorV1::FoldCreation {
            parity: KagemushaPastaParityV1::Ep,
            reason: error.to_string(),
        })?;
    fold_kagemusha_ep_accumulators_with_rng_v1(params, current, predecessor, rng)
}

/// Fold Ep inputs with explicitly supplied entropy for the online issuer path.
///
/// Device-state and postcommit recovery callers must use the seed-requiring
/// public entry point. The online issuer must persist its resulting checkpoint.
///
/// # Errors
///
/// Rejects malformed parameters/accumulators, backend failure, or wrong proof size.
pub(super) fn fold_kagemusha_ep_accumulators_with_rng_v1(
    params: &ParamsIPA<EpAffine>,
    current: &KagemushaEpAccumulatorV1,
    predecessor: &KagemushaEpAccumulatorV1,
    rng: impl rand_core_06::RngCore + rand_core_06::CryptoRng,
) -> Result<KagemushaEpFoldOutputV1, KagemushaRecursionErrorV1> {
    validate_ep_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let proving_key = ep_proving_key(params);
    let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let successor = <EpAccumulation as AccumulationSchemeProver<EpAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        rng,
    )
    .map_err(|error| KagemushaRecursionErrorV1::FoldCreation {
        parity: KagemushaPastaParityV1::Ep,
        reason: format!("{error:?}"),
    })?;
    let proof = KagemushaEpFoldProofV1::try_from_bytes(&transcript.finalize())?;
    Ok(KagemushaEpFoldOutputV1 {
        successor: KagemushaEpAccumulatorV1::from_native(&successor)?,
        proof,
    })
}

/// Hash the exact ordered fold inputs and the complete fixed-profile IPA basis.
///
/// Every generator is included, not merely the first generator or the degree.
/// The fixed domain and H/S hash-to-curve inputs match the proving-key builders
/// below. Length-framed generators and fixed-width ordered accumulator encodings
/// leave no concatenation ambiguity and prevent seed reuse across parameter sets.
fn fold_recovery_context_v1<I, B>(
    parity: KagemushaPastaParityV1,
    generators: I,
    current: &[u8; ACCUMULATOR_BYTES],
    predecessor: &[u8; ACCUMULATOR_BYTES],
) -> [u8; 32]
where
    I: ExactSizeIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ipa-fold-recovery-context\0");
    hash.update([match parity {
        KagemushaPastaParityV1::Eq => 0,
        KagemushaPastaParityV1::Ep => 1,
    }]);
    hash.update(KAGEMUSHA_RECURSION_IPA_K_V1.to_le_bytes());
    for profile_value in [
        POSEIDON_WIDTH,
        POSEIDON_RATE,
        POSEIDON_FULL_ROUNDS,
        POSEIDON_PARTIAL_ROUNDS,
        POSEIDON_SECURE_MDS,
        KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
    ] {
        hash.update((profile_value as u64).to_le_bytes());
    }
    hash.update(b"Halo2-Parameters\0");
    hash.update([2, 1]); // H and the enabled ZK blinding base S, respectively.
    hash.update((generators.len() as u64).to_le_bytes());
    for generator in generators {
        let bytes = generator.as_ref();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.update(current);
    hash.update(predecessor);
    hash.finalize().into()
}

/// Verify and terminally decide one exact Eq predecessor/current fold.
///
/// # Errors
///
/// Rejects malformed inputs, proof substitution, an invalid BGH19 relation, a trailing transcript,
/// a false terminal accumulator relation, or a substituted successor.
pub fn verify_and_decide_kagemusha_eq_fold_v1(
    params: &ParamsIPA<EqAffine>,
    current: &KagemushaEqAccumulatorV1,
    predecessor: &KagemushaEqAccumulatorV1,
    fold: &KagemushaEqFoldOutputV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    validate_eq_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let deciding_key = eq_deciding_key(params);
    let cursor = std::io::Cursor::new(fold.proof.0.to_vec());
    let mut transcript = EqTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
    let parsed = catch_native_verifier_panic(KagemushaPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Eq,
        reason: format!("proof parse failed: {error:?}"),
    })?;
    let successor = catch_native_verifier_panic(KagemushaPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationScheme<EqAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Eq,
        reason: format!("proof relation failed: {error:?}"),
    })?;
    ensure_transcript_consumed(KagemushaPastaParityV1::Eq, transcript.finalize().position())?;
    decide_eq_native(&deciding_key, successor.clone())?;
    if KagemushaEqAccumulatorV1::from_native(&successor)? != fold.successor {
        return Err(KagemushaRecursionErrorV1::FoldSuccessorSubstitution(
            KagemushaPastaParityV1::Eq,
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
pub fn verify_and_decide_kagemusha_ep_fold_v1(
    params: &ParamsIPA<EpAffine>,
    current: &KagemushaEpAccumulatorV1,
    predecessor: &KagemushaEpAccumulatorV1,
    fold: &KagemushaEpFoldOutputV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    validate_ep_params(params)?;
    let inputs = [current.to_native()?, predecessor.to_native()?];
    let deciding_key = ep_deciding_key(params);
    let cursor = std::io::Cursor::new(fold.proof.0.to_vec());
    let mut transcript = EpTranscript::new::<POSEIDON_SECURE_MDS>(cursor);
    let parsed = catch_native_verifier_panic(KagemushaPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
            deciding_key.as_ref(),
            &inputs,
            &mut transcript,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Ep,
        reason: format!("proof parse failed: {error:?}"),
    })?;
    let successor = catch_native_verifier_panic(KagemushaPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationScheme<EpAffine, NativeLoader>>::verify(
            deciding_key.as_ref(),
            &inputs,
            &parsed,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Ep,
        reason: format!("proof relation failed: {error:?}"),
    })?;
    ensure_transcript_consumed(KagemushaPastaParityV1::Ep, transcript.finalize().position())?;
    decide_ep_native(&deciding_key, successor.clone())?;
    if KagemushaEpAccumulatorV1::from_native(&successor)? != fold.successor {
        return Err(KagemushaRecursionErrorV1::FoldSuccessorSubstitution(
            KagemushaPastaParityV1::Ep,
        ));
    }
    Ok(())
}

/// Terminally decide one canonical Eq accumulator under the fixed authenticated parameters.
///
/// # Errors
///
/// Rejects non-fixed parameters, malformed encoding, or a false accumulator relation.
pub fn decide_kagemusha_eq_accumulator_v1(
    params: &ParamsIPA<EqAffine>,
    accumulator: &KagemushaEqAccumulatorV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    validate_eq_params(params)?;
    decide_eq_native(&eq_deciding_key(params), accumulator.to_native()?)
}

/// Terminally decide one canonical Ep accumulator under the fixed authenticated parameters.
///
/// # Errors
///
/// Rejects non-fixed parameters, malformed encoding, or a false accumulator relation.
pub fn decide_kagemusha_ep_accumulator_v1(
    params: &ParamsIPA<EpAffine>,
    accumulator: &KagemushaEpAccumulatorV1,
) -> Result<(), KagemushaRecursionErrorV1> {
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
pub fn initial_kagemusha_eq_accumulator_v1(
    params: &ParamsIPA<EqAffine>,
) -> Result<KagemushaEqAccumulatorV1, KagemushaRecursionErrorV1> {
    validate_eq_params(params)?;
    let point = params
        .get_g()
        .iter()
        .fold(Eq::identity(), |sum, base| sum + *base)
        .to_affine();
    let accumulator = KagemushaEqAccumulatorV1::from_native(&IpaAccumulator::new(
        vec![Fp::ONE; ACCUMULATOR_ROUNDS],
        point,
    ))?;
    decide_kagemusha_eq_accumulator_v1(params, &accumulator)?;
    Ok(accumulator)
}

/// Construct the canonical terminally valid empty Ep history accumulator.
///
/// This is the reciprocal-parity analogue of
/// [`initial_kagemusha_eq_accumulator_v1`]. It is terminally decided before it can become a
/// bootstrap witness.
///
/// # Errors
///
/// Rejects non-`k=16` parameters, an unexpected identity commitment, or a failed terminal check.
pub fn initial_kagemusha_ep_accumulator_v1(
    params: &ParamsIPA<EpAffine>,
) -> Result<KagemushaEpAccumulatorV1, KagemushaRecursionErrorV1> {
    validate_ep_params(params)?;
    let point = params
        .get_g()
        .iter()
        .fold(Ep::identity(), |sum, base| sum + *base)
        .to_affine();
    let accumulator = KagemushaEpAccumulatorV1::from_native(&IpaAccumulator::new(
        vec![Fq::ONE; ACCUMULATOR_ROUNDS],
        point,
    ))?;
    decide_kagemusha_ep_accumulator_v1(params, &accumulator)?;
    Ok(accumulator)
}

fn validate_accumulator_length(
    parity: KagemushaPastaParityV1,
    bytes: &[u8],
) -> Result<(), KagemushaRecursionErrorV1> {
    if bytes.len() != ACCUMULATOR_BYTES {
        return Err(KagemushaRecursionErrorV1::InvalidAccumulatorLength {
            parity,
            actual: bytes.len(),
            expected: ACCUMULATOR_BYTES,
        });
    }
    Ok(())
}

fn parse_eq(
    raw: &[u8; ACCUMULATOR_BYTES],
) -> Result<IpaAccumulator<EqAffine, NativeLoader>, KagemushaRecursionErrorV1> {
    use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

    let mut challenges = Vec::with_capacity(ACCUMULATOR_ROUNDS);
    for (round, chunk) in raw[..ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES]
        .chunks_exact(ACCUMULATOR_CHALLENGE_BYTES)
        .enumerate()
    {
        let repr: [u8; 32] = chunk.try_into().expect("fixed challenge chunk");
        let scalar = Option::<Fp>::from(Fp::from_repr(repr.into())).ok_or(
            KagemushaRecursionErrorV1::NonCanonicalAccumulatorScalar {
                parity: KagemushaPastaParityV1::Eq,
                round,
            },
        )?;
        challenges.push(scalar);
    }
    let point_bytes: [u8; 32] = raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
        .try_into()
        .expect("fixed point chunk");
    let point = Option::<EqAffine>::from(EqAffine::from_bytes(&point_bytes.into())).ok_or(
        KagemushaRecursionErrorV1::InvalidAccumulatorPoint(KagemushaPastaParityV1::Eq),
    )?;
    if bool::from(point.is_identity()) {
        return Err(KagemushaRecursionErrorV1::InvalidAccumulatorPoint(
            KagemushaPastaParityV1::Eq,
        ));
    }
    Ok(IpaAccumulator::new(challenges, point))
}

fn parse_ep(
    raw: &[u8; ACCUMULATOR_BYTES],
) -> Result<IpaAccumulator<EpAffine, NativeLoader>, KagemushaRecursionErrorV1> {
    use halo2_proofs::halo2curves::group::prime::PrimeCurveAffine as _;

    let mut challenges = Vec::with_capacity(ACCUMULATOR_ROUNDS);
    for (round, chunk) in raw[..ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES]
        .chunks_exact(ACCUMULATOR_CHALLENGE_BYTES)
        .enumerate()
    {
        let repr: [u8; 32] = chunk.try_into().expect("fixed challenge chunk");
        let scalar = Option::<Fq>::from(Fq::from_repr(repr.into())).ok_or(
            KagemushaRecursionErrorV1::NonCanonicalAccumulatorScalar {
                parity: KagemushaPastaParityV1::Ep,
                round,
            },
        )?;
        challenges.push(scalar);
    }
    let point_bytes: [u8; 32] = raw[ACCUMULATOR_ROUNDS * ACCUMULATOR_CHALLENGE_BYTES..]
        .try_into()
        .expect("fixed point chunk");
    let point = Option::<EpAffine>::from(EpAffine::from_bytes(&point_bytes.into())).ok_or(
        KagemushaRecursionErrorV1::InvalidAccumulatorPoint(KagemushaPastaParityV1::Ep),
    )?;
    if bool::from(point.is_identity()) {
        return Err(KagemushaRecursionErrorV1::InvalidAccumulatorPoint(
            KagemushaPastaParityV1::Ep,
        ));
    }
    Ok(IpaAccumulator::new(challenges, point))
}

fn validate_eq_params(params: &ParamsIPA<EqAffine>) -> Result<(), KagemushaRecursionErrorV1> {
    if params.k() != KAGEMUSHA_RECURSION_IPA_K_V1 {
        return Err(KagemushaRecursionErrorV1::InvalidIpaParameters {
            parity: KagemushaPastaParityV1::Eq,
            actual: params.k(),
        });
    }
    Ok(())
}

fn validate_ep_params(params: &ParamsIPA<EpAffine>) -> Result<(), KagemushaRecursionErrorV1> {
    if params.k() != KAGEMUSHA_RECURSION_IPA_K_V1 {
        return Err(KagemushaRecursionErrorV1::InvalidIpaParameters {
            parity: KagemushaPastaParityV1::Ep,
            actual: params.k(),
        });
    }
    Ok(())
}

fn eq_proving_key(params: &ParamsIPA<EqAffine>) -> IpaProvingKey<EqAffine> {
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    IpaProvingKey::new(
        Domain::new(
            KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
            root_of_unity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize),
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
            KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
            root_of_unity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize),
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
                KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
                root_of_unity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize),
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
                KAGEMUSHA_RECURSION_IPA_K_V1 as usize,
                root_of_unity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize),
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
) -> Result<(), KagemushaRecursionErrorV1> {
    catch_native_verifier_panic(KagemushaPastaParityV1::Eq, || {
        <EqAccumulation as AccumulationDecider<EqAffine, NativeLoader>>::decide(
            deciding_key,
            accumulator,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Eq,
        reason: format!("terminal decision failed: {error:?}"),
    })
}

fn decide_ep_native(
    deciding_key: &IpaDecidingKey<EpAffine>,
    accumulator: IpaAccumulator<EpAffine, NativeLoader>,
) -> Result<(), KagemushaRecursionErrorV1> {
    catch_native_verifier_panic(KagemushaPastaParityV1::Ep, || {
        <EpAccumulation as AccumulationDecider<EpAffine, NativeLoader>>::decide(
            deciding_key,
            accumulator,
        )
    })?
    .map_err(|error| KagemushaRecursionErrorV1::FoldDecision {
        parity: KagemushaPastaParityV1::Ep,
        reason: format!("terminal decision failed: {error:?}"),
    })
}

fn ensure_transcript_consumed(
    parity: KagemushaPastaParityV1,
    consumed: u64,
) -> Result<(), KagemushaRecursionErrorV1> {
    if consumed
        != u64::try_from(KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1)
            .map_err(|_| KagemushaRecursionErrorV1::LengthOverflow)?
    {
        return Err(KagemushaRecursionErrorV1::FoldDecision {
            parity,
            reason: "fold transcript has trailing or unconsumed bytes".to_owned(),
        });
    }
    Ok(())
}

fn catch_native_verifier_panic<T>(
    parity: KagemushaPastaParityV1,
    verify: impl FnOnce() -> T,
) -> Result<T, KagemushaRecursionErrorV1> {
    crate::panic_hook::catch_unwind_suppressed(verify)
        .map_err(|_| KagemushaRecursionErrorV1::NativeVerifierPanic(parity))
}

const _: () = assert!(ACCUMULATOR_BYTES == 544);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_context_binds_parity_all_generators_and_ordered_inputs() {
        // Artificial bytes test transcript framing only, not accumulator validity.
        let generators = [[1_u8; 32], [2_u8; 32], [3_u8; 32]];
        let current = [4_u8; ACCUMULATOR_BYTES];
        let predecessor = [5_u8; ACCUMULATOR_BYTES];
        let context = fold_recovery_context_v1(
            KagemushaPastaParityV1::Eq,
            generators.iter(),
            &current,
            &predecessor,
        );
        assert_eq!(
            context,
            fold_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                generators.iter(),
                &current,
                &predecessor,
            )
        );
        assert_ne!(
            context,
            fold_recovery_context_v1(
                KagemushaPastaParityV1::Ep,
                generators.iter(),
                &current,
                &predecessor,
            )
        );
        assert_ne!(
            context,
            fold_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                generators.iter(),
                &predecessor,
                &current,
            )
        );
        for index in 0..generators.len() {
            let mut changed_generators = generators;
            changed_generators[index][0] ^= 1;
            assert_ne!(
                context,
                fold_recovery_context_v1(
                    KagemushaPastaParityV1::Eq,
                    changed_generators.iter(),
                    &current,
                    &predecessor,
                ),
                "every parameter generator must be bound"
            );
        }
        let mut changed_current = current;
        changed_current[ACCUMULATOR_BYTES - 1] ^= 1;
        assert_ne!(
            context,
            fold_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                generators.iter(),
                &changed_current,
                &predecessor,
            )
        );
        let mut changed_predecessor = predecessor;
        changed_predecessor[ACCUMULATOR_BYTES - 1] ^= 1;
        assert_ne!(
            context,
            fold_recovery_context_v1(
                KagemushaPastaParityV1::Eq,
                generators.iter(),
                &current,
                &changed_predecessor,
            )
        );
    }

    #[test]
    fn canonical_empty_histories_are_terminally_decided() {
        let eq_params = ParamsIPA::<EqAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
        let ep_params = ParamsIPA::<EpAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
        let eq = initial_kagemusha_eq_accumulator_v1(&eq_params).expect("Eq empty history");
        let ep = initial_kagemusha_ep_accumulator_v1(&ep_params).expect("Ep empty history");

        decide_kagemusha_eq_accumulator_v1(&eq_params, &eq).expect("decided Eq history");
        decide_kagemusha_ep_accumulator_v1(&ep_params, &ep).expect("decided Ep history");
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
