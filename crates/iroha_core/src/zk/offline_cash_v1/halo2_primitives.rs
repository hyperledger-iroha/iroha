//! Strict low-level primitives for the Offline Cash V1 Pasta verifier.
//!
//! These helpers deliberately do not reconstruct or accept a STATE proof.  They
//! freeze the compact IPA history, safely parse governed Halo2 artifacts, and
//! implement the augmented-proof opening check that the eventual exact STATE
//! circuit verifier will call.

use core::fmt;
use std::{
    io::{self, Cursor, Write},
    panic::{AssertUnwindSafe, catch_unwind},
};

use halo2_proofs::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    arithmetic::best_multiexp,
    halo2curves::{
        CurveAffine,
        ff::{Field, FromUniformBytes, PrimeField, WithSmallOrderMulGroup},
        group::Curve as _,
        pasta::{EpAffine, EqAffine},
    },
    plonk::{
        Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey,
        verify_proof as halo2_verify_proof,
    },
    poly::{
        VerificationStrategy,
        commitment::{MSM as _, Params as _, ParamsProver as _},
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            msm::MSMIPA,
            multiopen::VerifierIPA,
            strategy::{Accumulator, GuardIPA},
        },
    },
    transcript::{Blake2bRead, Challenge255, TranscriptReadBuffer as _},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1,
    OFFLINE_CASH_PARAMS_BYTES_V1, OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1,
};

use super::OfflineCashHalo2ParityV1;

const SCALAR_BYTES: usize = 32;
const POINT_BYTES: usize = 32;
const HISTORY_ROUNDS: usize = OFFLINE_CASH_HALO2_K_V1 as usize;
const PROCESSED_VK_VERSION: u8 = 0x02;
const PROCESSED_VK_HEADER_BYTES: usize = 10;
const UNCOMPRESSED_SELECTORS: u8 = 0;

const _: () = assert!(OFFLINE_CASH_HALO2_K_V1 == 16);
const _: () = assert!(
    OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1 == HISTORY_ROUNDS * SCALAR_BYTES + POINT_BYTES
);

/// Strict parsing or native-verification failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashHalo2PrimitiveErrorV1 {
    /// The compact accumulator has the wrong length or element encoding.
    InvalidHistory,
    /// The serialized parameters have the wrong fixed shape.
    InvalidParameterShape,
    /// A bounded parameter payload could not be decoded.
    InvalidParameterEncoding,
    /// Decoding and canonical reserialization differed.
    NonCanonicalParameterEncoding,
    /// Parameters differ from the transparent deterministic derivation.
    NonTransparentParameters,
    /// The processed verifier key has the wrong bounded header or circuit shape.
    InvalidVerifierKeyShape,
    /// A processed verifier key could not be decoded.
    InvalidVerifierKeyEncoding,
    /// Decoding and processed reserialization differed.
    NonCanonicalVerifierKeyEncoding,
    /// The augmented proof suffix or proof-derived accumulator differs from history.
    HistoryBindingMismatch,
    /// Native Halo2 proof parsing or verification failed.
    InvalidProof,
    /// The delayed IPA equation failed.
    InvalidHistoryDecision,
}

impl fmt::Display for OfflineCashHalo2PrimitiveErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidHistory => "invalid offline-cash IPA history encoding",
            Self::InvalidParameterShape => "invalid offline-cash IPA parameter shape",
            Self::InvalidParameterEncoding => "invalid offline-cash IPA parameter encoding",
            Self::NonCanonicalParameterEncoding => {
                "non-canonical offline-cash IPA parameter encoding"
            }
            Self::NonTransparentParameters => {
                "offline-cash IPA parameters differ from transparent derivation"
            }
            Self::InvalidVerifierKeyShape => "invalid offline-cash processed verifier-key shape",
            Self::InvalidVerifierKeyEncoding => {
                "invalid offline-cash processed verifier-key encoding"
            }
            Self::NonCanonicalVerifierKeyEncoding => {
                "non-canonical offline-cash processed verifier-key encoding"
            }
            Self::HistoryBindingMismatch => {
                "offline-cash augmented proof does not bind the supplied history"
            }
            Self::InvalidProof => "invalid offline-cash augmented IPA proof",
            Self::InvalidHistoryDecision => "invalid offline-cash delayed IPA decision",
        })
    }
}

impl std::error::Error for OfflineCashHalo2PrimitiveErrorV1 {}

/// Exact challenge-major delayed accumulator: 16 scalars followed by one point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OfflineCashIpaHistoryV1<C: CurveAffine> {
    round_challenges: [C::Scalar; HISTORY_ROUNDS],
    folded_generator: C,
}

impl<C> OfflineCashIpaHistoryV1<C>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    fn parse(bytes: &[u8]) -> Result<Self, OfflineCashHalo2PrimitiveErrorV1> {
        if bytes.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1 {
            return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory);
        }
        let round_challenges = bytes[..HISTORY_ROUNDS * SCALAR_BYTES]
            .chunks_exact(SCALAR_BYTES)
            .map(parse_scalar::<C::Scalar>)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)?;
        let folded_generator = parse_point::<C>(&bytes[HISTORY_ROUNDS * SCALAR_BYTES..])?;
        if bool::from(folded_generator.is_identity()) {
            return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory);
        }
        Ok(Self {
            round_challenges,
            folded_generator,
        })
    }

    #[cfg(test)]
    fn from_parts(
        round_challenges: [C::Scalar; HISTORY_ROUNDS],
        folded_generator: C,
    ) -> Result<Self, OfflineCashHalo2PrimitiveErrorV1> {
        if bool::from(folded_generator.is_identity()) {
            return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory);
        }
        Ok(Self {
            round_challenges,
            folded_generator,
        })
    }

    #[cfg(test)]
    fn to_bytes(&self) -> [u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1] {
        let mut bytes = [0_u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1];
        for (index, scalar) in self.round_challenges.iter().enumerate() {
            let repr = scalar.to_repr();
            bytes[index * SCALAR_BYTES..(index + 1) * SCALAR_BYTES].copy_from_slice(repr.as_ref());
        }
        bytes[HISTORY_ROUNDS * SCALAR_BYTES..]
            .copy_from_slice(self.folded_generator.to_bytes().as_ref());
        bytes
    }
}

fn parse_scalar<F: PrimeField>(bytes: &[u8]) -> Result<F, OfflineCashHalo2PrimitiveErrorV1> {
    let mut repr = F::Repr::default();
    if bytes.len() != SCALAR_BYTES || repr.as_ref().len() != SCALAR_BYTES {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory);
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr)).ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)
}

fn parse_point<C: CurveAffine>(bytes: &[u8]) -> Result<C, OfflineCashHalo2PrimitiveErrorV1> {
    let mut repr = C::Repr::default();
    if bytes.len() != POINT_BYTES || repr.as_ref().len() != POINT_BYTES {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory);
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<C>::from(C::from_bytes(&repr)).ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidHistory)
}

pub(super) fn validate_offline_cash_history_v1(
    parity: OfflineCashHalo2ParityV1,
    bytes: &[u8],
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1> {
    match parity {
        OfflineCashHalo2ParityV1::Eq => {
            OfflineCashIpaHistoryV1::<EqAffine>::parse(bytes).map(|_| ())
        }
        OfflineCashHalo2ParityV1::Ep => {
            OfflineCashIpaHistoryV1::<EpAffine>::parse(bytes).map(|_| ())
        }
    }
}

struct ExactBytesWriter<'a> {
    expected: &'a [u8],
    offset: usize,
}

impl ExactBytesWriter<'_> {
    fn finish(self) -> io::Result<()> {
        if self.offset == self.expected.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialization ended before the expected bytes",
            ))
        }
    }
}

impl Write for ExactBytesWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self.offset.checked_add(bytes.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "serialization length overflow")
        })?;
        if self.expected.get(self.offset..end) != Some(bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialization differs from the expected bytes",
            ));
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn exact_params_length(k: u32) -> Option<usize> {
    if k >= usize::BITS {
        return None;
    }
    let rows = 1_usize.checked_shl(k)?;
    rows.checked_mul(2)?
        .checked_add(2)?
        .checked_mul(POINT_BYTES)?
        .checked_add(4)
}

fn params_write_matches<C: CurveAffine>(params: &ParamsIPA<C>, expected: &[u8]) -> bool {
    let mut writer = ExactBytesWriter {
        expected,
        offset: 0,
    };
    params.write(&mut writer).is_ok() && writer.finish().is_ok()
}

fn parse_params_exact_for_k<C>(
    bytes: &[u8],
    expected_k: u32,
) -> Result<ParamsIPA<C>, OfflineCashHalo2PrimitiveErrorV1>
where
    C: CurveAffine,
{
    let expected_len = exact_params_length(expected_k)
        .ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape)?;
    if expected_k >= 32 || bytes.len() != expected_len || bytes.len() < 4 {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    let encoded_k = u32::from_le_bytes(
        bytes[..4]
            .try_into()
            .expect("four-byte preflight slice has exact length"),
    );
    if encoded_k != expected_k {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }

    let mut cursor = Cursor::new(bytes);
    let parsed = catch_unwind(AssertUnwindSafe(|| ParamsIPA::<C>::read(&mut cursor)))
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding)?
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding)?;
    if cursor.position() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || parsed.k() != expected_k
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding);
    }
    if !params_write_matches(&parsed, bytes) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonCanonicalParameterEncoding);
    }
    drop(parsed);

    let canonical = catch_unwind(AssertUnwindSafe(|| ParamsIPA::<C>::new(expected_k)))
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::NonTransparentParameters)?;
    if !params_write_matches(&canonical, bytes) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonTransparentParameters);
    }
    Ok(canonical)
}

/// Parse exact Eq parameters only after fixed-k16 allocation preflight.
pub(super) fn parse_offline_cash_eq_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EqAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_HALO2_K_V1)
}

/// Parse exact Ep parameters only after fixed-k16 allocation preflight.
pub(super) fn parse_offline_cash_ep_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EpAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_HALO2_K_V1)
}

fn configured_uncompressed_fixed_columns<F, ConcreteCircuit>() -> Option<usize>
where
    F: Field,
    ConcreteCircuit: Circuit<F>,
    ConcreteCircuit::Params: Default,
{
    let mut cs = ConstraintSystem::<F>::default();
    #[cfg(feature = "circuit-params")]
    let _ = ConcreteCircuit::configure_with_params(&mut cs, ConcreteCircuit::Params::default());
    #[cfg(not(feature = "circuit-params"))]
    let _ = ConcreteCircuit::configure(&mut cs);
    cs.num_fixed_columns().checked_add(cs.num_selectors())
}

/// Preflight, parse, and canonically round-trip one processed verifier key.
///
/// This stays generic until the exact STATE `Circuit` types and instance ABI
/// exist; no placeholder circuit may be used to activate production parsing.
pub(super) fn parse_processed_verifier_key_v1<C, ConcreteCircuit>(
    bytes: &[u8],
    expected_k: u32,
) -> Result<VerifyingKey<C>, OfflineCashHalo2PrimitiveErrorV1>
where
    C: SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
    ConcreteCircuit::Params: Default,
{
    if expected_k >= 32
        || bytes.len() < PROCESSED_VK_HEADER_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
        || bytes[0] != PROCESSED_VK_VERSION
        || u32::from_le_bytes(bytes[1..5].try_into().expect("VK k slice has four bytes"))
            != expected_k
        || bytes[5] != UNCOMPRESSED_SELECTORS
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape);
    }
    let expected_fixed = configured_uncompressed_fixed_columns::<C::ScalarExt, ConcreteCircuit>()
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape)?;
    let encoded_fixed = u32::from_le_bytes(
        bytes[6..10]
            .try_into()
            .expect("VK fixed-count slice has four bytes"),
    );
    if encoded_fixed != expected_fixed {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape);
    }

    let mut cursor = Cursor::new(bytes);
    let key = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(feature = "circuit-params")]
        {
            VerifyingKey::<C>::read::<_, ConcreteCircuit>(
                &mut cursor,
                SerdeFormat::Processed,
                ConcreteCircuit::Params::default(),
            )
        }
        #[cfg(not(feature = "circuit-params"))]
        {
            VerifyingKey::<C>::read::<_, ConcreteCircuit>(&mut cursor, SerdeFormat::Processed)
        }
    }))
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding)?
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding)?;
    if cursor.position() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || key.get_domain().k() != expected_k
        || key.fixed_commitments().len() != usize::try_from(expected_fixed).unwrap_or(usize::MAX)
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding);
    }
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonCanonicalVerifierKeyEncoding);
    }
    Ok(key)
}

#[derive(Debug)]
struct ClaimedHistoryStrategy<'params, C: CurveAffine> {
    msm: MSMIPA<'params, C>,
    folded_generator: Option<C>,
}

impl<'params, C: CurveAffine> ClaimedHistoryStrategy<'params, C> {
    fn with_generator(params: &'params ParamsIPA<C>, folded_generator: C) -> Self {
        Self {
            msm: MSMIPA::new(params),
            folded_generator: Some(folded_generator),
        }
    }
}

impl<'params, C> VerificationStrategy<'params, IPACommitmentScheme<C>, VerifierIPA<'params, C>>
    for ClaimedHistoryStrategy<'params, C>
where
    C: CurveAffine,
{
    type Output = Accumulator<C>;

    fn new(params: &'params ParamsIPA<C>) -> Self {
        Self {
            msm: MSMIPA::new(params),
            folded_generator: None,
        }
    }

    fn process(
        self,
        verify: impl FnOnce(MSMIPA<'params, C>) -> Result<GuardIPA<'params, C>, PlonkError>,
    ) -> Result<Self::Output, PlonkError> {
        let folded_generator = self
            .folded_generator
            .ok_or(PlonkError::ConstraintSystemFailure)?;
        let guard = verify(self.msm)?;
        let (msm, accumulator) = guard.use_g(folded_generator);
        if msm.check() {
            Ok(accumulator)
        } else {
            Err(PlonkError::ConstraintSystemFailure)
        }
    }

    fn finalize(self) -> bool {
        unreachable!("offline-cash single-proof strategy decides during process")
    }
}

fn verify_augmented_claim<C>(
    params: &ParamsIPA<C>,
    verifying_key: &VerifyingKey<C>,
    augmented_proof: &[u8],
    instances: &[&[&[C::Scalar]]],
    round_challenges: &[C::Scalar],
    folded_generator: C,
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    let expected_rounds = usize::try_from(params.k())
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch)?;
    if round_challenges.len() != expected_rounds
        || augmented_proof.len() <= POINT_BYTES
        || bool::from(folded_generator.is_identity())
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch);
    }
    let prefix_len = augmented_proof.len() - POINT_BYTES;
    if folded_generator.to_bytes().as_ref() != &augmented_proof[prefix_len..] {
        return Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch);
    }
    let proof_prefix = &augmented_proof[..prefix_len];
    let verified = catch_unwind(AssertUnwindSafe(|| {
        let mut cursor = Cursor::new(proof_prefix);
        let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(&mut cursor);
        let accumulator = halo2_verify_proof::<
            IPACommitmentScheme<C>,
            VerifierIPA<'_, C>,
            Challenge255<C>,
            _,
            _,
        >(
            params,
            verifying_key,
            ClaimedHistoryStrategy::with_generator(params, folded_generator),
            instances,
            &mut transcript,
        )?;
        drop(transcript);
        Ok::<_, PlonkError>((accumulator, cursor.position()))
    }))
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidProof)?
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidProof)?;
    let (accumulator, consumed) = verified;
    if consumed != u64::try_from(proof_prefix.len()).unwrap_or(u64::MAX)
        || accumulator.g != folded_generator
        || accumulator.u_packed != round_challenges
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch);
    }
    Ok(())
}

/// Verify that an augmented proof prefix yields exactly the supplied k16 history.
pub(super) fn verify_augmented_ipa_proof_v1<C>(
    params: &ParamsIPA<C>,
    verifying_key: &VerifyingKey<C>,
    augmented_proof: &[u8],
    instances: &[&[&[C::Scalar]]],
    history: &OfflineCashIpaHistoryV1<C>,
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1>
where
    C: CurveAffine,
    C::Scalar: PrimeField + FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
{
    if params.k() != OFFLINE_CASH_HALO2_K_V1 {
        return Err(OfflineCashHalo2PrimitiveErrorV1::HistoryBindingMismatch);
    }
    verify_augmented_claim(
        params,
        verifying_key,
        augmented_proof,
        instances,
        &history.round_challenges,
        history.folded_generator,
    )
}

fn history_coefficients<F: Field>(round_challenges: &[F]) -> Option<Vec<F>> {
    if round_challenges.is_empty() || round_challenges.len() >= usize::BITS as usize {
        return None;
    }
    let mut coefficients = vec![F::ZERO; 1_usize.checked_shl(round_challenges.len() as u32)?];
    coefficients[0] = F::ONE;
    for (len, challenge) in round_challenges
        .iter()
        .rev()
        .enumerate()
        .map(|(index, challenge)| (1_usize << index, challenge))
    {
        let (left, right) = coefficients.split_at_mut(len);
        let right = &mut right[..len];
        right.copy_from_slice(left);
        for coefficient in right {
            *coefficient *= challenge;
        }
    }
    Some(coefficients)
}

fn decide_claim<C>(
    params: &ParamsIPA<C>,
    round_challenges: &[C::Scalar],
    folded_generator: C,
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1>
where
    C: CurveAffine,
{
    let Some(expected_generators) = 1_usize.checked_shl(
        u32::try_from(round_challenges.len())
            .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision)?,
    ) else {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision);
    };
    if usize::try_from(params.k()).ok() != Some(round_challenges.len())
        || params.get_g().len() != expected_generators
        || bool::from(folded_generator.is_identity())
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision);
    }
    let coefficients = history_coefficients(round_challenges)
        .ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision)?;
    let decided = catch_unwind(AssertUnwindSafe(|| {
        best_multiexp(&coefficients, params.get_g()).to_affine()
    }))
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision)?;
    if decided == folded_generator {
        Ok(())
    } else {
        Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision)
    }
}

/// Decide one exact Eq/Fp delayed accumulator.
pub(super) fn decide_eq_history_v1(
    params: &ParamsIPA<EqAffine>,
    history: &OfflineCashIpaHistoryV1<EqAffine>,
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1> {
    if params.k() != OFFLINE_CASH_HALO2_K_V1 {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision);
    }
    decide_claim(params, &history.round_challenges, history.folded_generator)
}

/// Decide one exact Ep/Fq delayed accumulator.
pub(super) fn decide_ep_history_v1(
    params: &ParamsIPA<EpAffine>,
    history: &OfflineCashIpaHistoryV1<EpAffine>,
) -> Result<(), OfflineCashHalo2PrimitiveErrorV1> {
    if params.k() != OFFLINE_CASH_HALO2_K_V1 {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidHistoryDecision);
    }
    decide_claim(params, &history.round_challenges, history.folded_generator)
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    pub(in crate::zk::offline_cash_v1) fn history_from_eq_parts(
        challenges: [<EqAffine as CurveAffine>::ScalarExt; HISTORY_ROUNDS],
        point: EqAffine,
    ) -> Result<OfflineCashIpaHistoryV1<EqAffine>, OfflineCashHalo2PrimitiveErrorV1> {
        OfflineCashIpaHistoryV1::from_parts(challenges, point)
    }

    pub(in crate::zk::offline_cash_v1) fn history_from_ep_parts(
        challenges: [<EpAffine as CurveAffine>::ScalarExt; HISTORY_ROUNDS],
        point: EpAffine,
    ) -> Result<OfflineCashIpaHistoryV1<EpAffine>, OfflineCashHalo2PrimitiveErrorV1> {
        OfflineCashIpaHistoryV1::from_parts(challenges, point)
    }

    pub(in crate::zk::offline_cash_v1) fn encode_history<C>(
        history: &OfflineCashIpaHistoryV1<C>,
    ) -> [u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1]
    where
        C: CurveAffine,
        C::Scalar: PrimeField,
    {
        history.to_bytes()
    }

    pub(in crate::zk::offline_cash_v1) fn parse_eq_history(
        bytes: &[u8],
    ) -> Result<OfflineCashIpaHistoryV1<EqAffine>, OfflineCashHalo2PrimitiveErrorV1> {
        OfflineCashIpaHistoryV1::parse(bytes)
    }

    pub(in crate::zk::offline_cash_v1) fn parse_ep_history(
        bytes: &[u8],
    ) -> Result<OfflineCashIpaHistoryV1<EpAffine>, OfflineCashHalo2PrimitiveErrorV1> {
        OfflineCashIpaHistoryV1::parse(bytes)
    }

    pub(in crate::zk::offline_cash_v1) fn parse_params_for_k<C>(
        bytes: &[u8],
        expected_k: u32,
    ) -> Result<ParamsIPA<C>, OfflineCashHalo2PrimitiveErrorV1>
    where
        C: CurveAffine,
    {
        parse_params_exact_for_k(bytes, expected_k)
    }

    pub(in crate::zk::offline_cash_v1) fn derive_claim<C>(
        params: &ParamsIPA<C>,
        verifying_key: &VerifyingKey<C>,
        proof: &[u8],
        instances: &[&[&[C::Scalar]]],
    ) -> Result<Accumulator<C>, OfflineCashHalo2PrimitiveErrorV1>
    where
        C: CurveAffine,
        C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
    {
        #[derive(Debug)]
        struct ComputeStrategy<'params, C: CurveAffine>(MSMIPA<'params, C>);
        impl<'params, C>
            VerificationStrategy<'params, IPACommitmentScheme<C>, VerifierIPA<'params, C>>
            for ComputeStrategy<'params, C>
        where
            C: CurveAffine,
        {
            type Output = Accumulator<C>;

            fn new(params: &'params ParamsIPA<C>) -> Self {
                Self(MSMIPA::new(params))
            }

            fn process(
                self,
                verify: impl FnOnce(MSMIPA<'params, C>) -> Result<GuardIPA<'params, C>, PlonkError>,
            ) -> Result<Self::Output, PlonkError> {
                let guard = verify(self.0)?;
                let generator = guard.compute_g();
                let (msm, accumulator) = guard.use_g(generator);
                if msm.check() {
                    Ok(accumulator)
                } else {
                    Err(PlonkError::ConstraintSystemFailure)
                }
            }

            fn finalize(self) -> bool {
                unreachable!("test derivation decides during process")
            }
        }

        let verified = catch_unwind(AssertUnwindSafe(|| {
            let mut cursor = Cursor::new(proof);
            let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(&mut cursor);
            let accumulator = halo2_verify_proof::<
                IPACommitmentScheme<C>,
                VerifierIPA<'_, C>,
                Challenge255<C>,
                _,
                _,
            >(
                params,
                verifying_key,
                ComputeStrategy::new(params),
                instances,
                &mut transcript,
            )?;
            drop(transcript);
            Ok::<_, PlonkError>((accumulator, cursor.position()))
        }))
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidProof)?
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidProof)?;
        if verified.1 != u64::try_from(proof.len()).unwrap_or(u64::MAX) {
            return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidProof);
        }
        Ok(verified.0)
    }

    pub(in crate::zk::offline_cash_v1) fn verify_augmented_claim_for_test<C>(
        params: &ParamsIPA<C>,
        verifying_key: &VerifyingKey<C>,
        proof: &[u8],
        instances: &[&[&[C::Scalar]]],
        accumulator: &Accumulator<C>,
    ) -> Result<(), OfflineCashHalo2PrimitiveErrorV1>
    where
        C: CurveAffine,
        C::Scalar: FromUniformBytes<64> + WithSmallOrderMulGroup<3>,
    {
        verify_augmented_claim(
            params,
            verifying_key,
            proof,
            instances,
            &accumulator.u_packed,
            accumulator.g,
        )
    }

    pub(in crate::zk::offline_cash_v1) fn decide_claim_for_test<C>(
        params: &ParamsIPA<C>,
        accumulator: &Accumulator<C>,
    ) -> Result<(), OfflineCashHalo2PrimitiveErrorV1>
    where
        C: CurveAffine,
    {
        decide_claim(params, &accumulator.u_packed, accumulator.g)
    }
}
