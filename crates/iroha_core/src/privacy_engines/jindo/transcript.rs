//! Canonically framed Fiat--Shamir transcript for revised Jindo.

use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use thiserror::Error;

use crate::privacy_engines::p256::{P256EngineError, TranscriptBindingV1};

use super::{
    JINDO_RING_DEGREE_V1,
    field::JindoFieldElementV1,
    parameters::JINDO_PARAMETERS_V1,
    ring::{
        JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoPrimeModulusV1, JindoRnsPolynomialV1,
    },
};

const TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.privacy.jindo.current.transcript.v1";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.privacy.jindo.current.challenge.v1";
const TRANSCRIPT_VERSION_V1: u8 = 2;
const MAX_DERIVATION_RETRIES_V1: u32 = 1 << 16;
const MAX_RANGE_REJECTIONS_V1: usize = 4096;

/// One uniformly sampled signed fixed-weight challenge in `S_35`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JindoShortChallengeV1 {
    coefficients: [i8; JINDO_RING_DEGREE_V1],
}

impl JindoShortChallengeV1 {
    pub(crate) fn polynomial(&self, moduli: [JindoPrimeModulusV1; 2]) -> JindoRnsPolynomialV1 {
        JindoRnsPolynomialV1::from_balanced_coefficients(self.coefficients.map(i128::from), moduli)
    }

    pub(crate) fn inner_polynomial(&self) -> JindoRnsPolynomialV1 {
        self.polynomial(JINDO_INNER_MODULI_V1)
    }

    pub(crate) fn outer_polynomial(&self) -> JindoRnsPolynomialV1 {
        self.polynomial(JINDO_OUTER_MODULI_V1)
    }

    pub(crate) fn encoded(&self) -> [u8; JINDO_RING_DEGREE_V1] {
        self.coefficients.map(|value| value as u8)
    }

    #[cfg(test)]
    pub(crate) const fn coefficients(&self) -> &[i8; JINDO_RING_DEGREE_V1] {
        &self.coefficients
    }
}

/// Append-only Jindo transcript state.
#[derive(Clone, Debug)]
pub(crate) struct JindoTranscriptV1 {
    state: Vec<u8>,
}

impl JindoTranscriptV1 {
    pub(crate) fn new(
        binding: &TranscriptBindingV1<'_>,
        expected_crs_digest: [u8; 32],
    ) -> Result<Self, JindoTranscriptErrorV1> {
        binding
            .validate()
            .map_err(JindoTranscriptErrorV1::Binding)?;
        if binding.generator_digest != expected_crs_digest {
            return Err(JindoTranscriptErrorV1::CrsDigestMismatch);
        }
        let mut transcript = Self {
            state: Vec::with_capacity(4096),
        };
        transcript.append_message(b"domain", TRANSCRIPT_DOMAIN_V1)?;
        transcript.append_message(b"version", &[TRANSCRIPT_VERSION_V1])?;
        transcript.append_message(b"chain_id", binding.chain_id)?;
        transcript.append_message(b"genesis_hash", &binding.genesis_hash)?;
        transcript.append_message(b"action_index", &binding.action_index.to_be_bytes())?;
        transcript.append_message(b"statement_digest", &binding.statement_digest)?;
        transcript.append_message(b"parameter_id", &binding.parameter_id)?;
        transcript.append_message(b"parameter_digest", &binding.parameter_digest)?;
        transcript.append_message(b"verifier_digest", &binding.verifier_digest)?;
        transcript.append_message(b"statement_schema_digest", &binding.statement_schema_digest)?;
        transcript.append_message(b"engine_manifest_digest", &binding.engine_manifest_digest)?;
        transcript.append_message(b"crs_digest", &binding.generator_digest)?;
        Ok(transcript)
    }

    pub(crate) fn append_message(
        &mut self,
        label: &[u8],
        value: &[u8],
    ) -> Result<(), JindoTranscriptErrorV1> {
        let label_len =
            u16::try_from(label.len()).map_err(|_| JindoTranscriptErrorV1::FieldTooLarge)?;
        let value_len =
            u32::try_from(value.len()).map_err(|_| JindoTranscriptErrorV1::FieldTooLarge)?;
        self.state.extend_from_slice(&label_len.to_be_bytes());
        self.state.extend_from_slice(label);
        self.state.extend_from_slice(&value_len.to_be_bytes());
        self.state.extend_from_slice(value);
        Ok(())
    }

    /// Derive the paper's uniform coefficient-field challenge
    /// `x* <- F_p^x` for ΠSplit.
    pub(crate) fn field_challenge(
        &mut self,
        label: &[u8],
        ordinal: u32,
    ) -> Result<JindoFieldElementV1, JindoTranscriptErrorV1> {
        for retry in 0..MAX_DERIVATION_RETRIES_V1 {
            let mut reader = self.challenge_reader(label, ordinal, retry)?;
            for _ in 0..MAX_RANGE_REJECTIONS_V1 {
                let mut bytes = [0_u8; 32];
                reader.read(&mut bytes);
                if let Some(value) = decode_nonzero_field_challenge(bytes) {
                    drop(reader);
                    self.bind_challenge(label, ordinal, retry, &bytes)?;
                    return Ok(value);
                }
            }
        }
        Err(JindoTranscriptErrorV1::ChallengeExhausted)
    }

    /// Derive one uniform member of the paper's complete signed fixed-weight
    /// challenge set `S_35`.
    ///
    /// Individual challenges are deliberately not conditioned on being
    /// units. The revised paper's extraction argument uses well-spreadness of
    /// the complete `S_35` distribution and the probability that the
    /// difference of two challenges is invertible.
    pub(crate) fn sparse_challenge(
        &mut self,
        label: &[u8],
        ordinal: u32,
    ) -> Result<JindoShortChallengeV1, JindoTranscriptErrorV1> {
        let mut reader = self.challenge_reader(label, ordinal, 0)?;
        let challenge = sample_uniform_fixed_weight(&mut reader)?;
        let encoded = challenge.encoded();
        drop(reader);
        self.bind_challenge(label, ordinal, 0, &encoded)?;
        Ok(challenge)
    }

    fn challenge_reader(
        &self,
        label: &[u8],
        ordinal: u32,
        retry: u32,
    ) -> Result<impl XofReader, JindoTranscriptErrorV1> {
        let mut hash = Shake256::default();
        absorb(&mut hash, CHALLENGE_DOMAIN_V1)?;
        absorb(&mut hash, &self.state)?;
        absorb(&mut hash, label)?;
        hash.update(&ordinal.to_be_bytes());
        hash.update(&retry.to_be_bytes());
        Ok(hash.finalize_xof())
    }

    fn bind_challenge(
        &mut self,
        label: &[u8],
        ordinal: u32,
        retry: u32,
        encoded: &[u8],
    ) -> Result<(), JindoTranscriptErrorV1> {
        self.append_message(b"challenge_label", label)?;
        self.append_message(b"challenge_ordinal", &ordinal.to_be_bytes())?;
        self.append_message(b"challenge_retry", &retry.to_be_bytes())?;
        self.append_message(b"challenge_value", encoded)
    }
}

fn decode_nonzero_field_challenge(bytes: [u8; 32]) -> Option<JindoFieldElementV1> {
    let value = JindoFieldElementV1::from_canonical_bytes(bytes)?;
    (!value.is_zero()).then_some(value)
}

fn absorb(hash: &mut Shake256, value: &[u8]) -> Result<(), JindoTranscriptErrorV1> {
    let len = u64::try_from(value.len()).map_err(|_| JindoTranscriptErrorV1::FieldTooLarge)?;
    hash.update(&len.to_be_bytes());
    hash.update(value);
    Ok(())
}

/// Partial Fisher--Yates: every ordered 35-tuple without replacement has the
/// same probability, and each unordered support has exactly `35!` preimages.
/// Independent sign bits then make all `2^35 * C(1024,35)` members equiprobable.
fn sample_uniform_fixed_weight(
    reader: &mut impl XofReader,
) -> Result<JindoShortChallengeV1, JindoTranscriptErrorV1> {
    let mut positions: [u16; JINDO_RING_DEGREE_V1] =
        core::array::from_fn(|index| u16::try_from(index).expect("degree fits u16"));
    let mut coefficients = [0_i8; JINDO_RING_DEGREE_V1];
    for selected in 0..JINDO_PARAMETERS_V1.challenge_weight {
        let remaining = JINDO_RING_DEGREE_V1 - selected;
        let offset = usize::try_from(sample_bounded(reader, remaining as u64)?)
            .map_err(|_| JindoTranscriptErrorV1::ChallengeExhausted)?;
        positions.swap(selected, selected + offset);
        let mut sign = [0_u8; 1];
        reader.read(&mut sign);
        coefficients[usize::from(positions[selected])] = if sign[0] & 1 == 0 { 1 } else { -1 };
    }
    Ok(JindoShortChallengeV1 { coefficients })
}

fn sample_bounded(reader: &mut impl XofReader, bound: u64) -> Result<u64, JindoTranscriptErrorV1> {
    let acceptance_limit = u64::MAX - (u64::MAX % bound);
    for _ in 0..MAX_RANGE_REJECTIONS_V1 {
        let mut bytes = [0_u8; 8];
        reader.read(&mut bytes);
        let candidate = u64::from_be_bytes(bytes);
        if candidate < acceptance_limit {
            return Ok(candidate % bound);
        }
    }
    Err(JindoTranscriptErrorV1::ChallengeExhausted)
}

/// Jindo transcript failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoTranscriptErrorV1 {
    /// The consensus transcript binding is structurally invalid.
    #[error("invalid Jindo consensus transcript binding: {0}")]
    Binding(P256EngineError),
    /// The bound CRS digest differs from the compiled transparent key.
    #[error("Jindo transcript CRS digest differs from the compiled transparent key")]
    CrsDigestMismatch,
    /// A framed transcript field exceeds the canonical length limit.
    #[error("Jindo transcript field is too large")]
    FieldTooLarge,
    /// Challenge derivation exhausted its fixed retry budget.
    #[error("Jindo challenge derivation exhausted its fixed retry budget")]
    ChallengeExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> TranscriptBindingV1<'static> {
        TranscriptBindingV1 {
            chain_id: b"jindo-test",
            genesis_hash: [1; 32],
            action_index: 0,
            statement_digest: [2; 32],
            parameter_id: [3; 32],
            parameter_digest: [4; 32],
            verifier_digest: [5; 32],
            statement_schema_digest: [6; 32],
            engine_manifest_digest: [7; 32],
            generator_digest: [8; 32],
        }
    }

    #[test]
    fn sparse_challenge_has_exact_weight_and_signed_coefficients() {
        let mut transcript = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        let challenge = transcript.sparse_challenge(b"alpha", 0).unwrap();
        assert_eq!(
            challenge.coefficients().iter().filter(|v| **v != 0).count(),
            35
        );
        assert!(
            challenge
                .coefficients()
                .iter()
                .all(|v| [-1, 0, 1].contains(v))
        );
    }

    #[test]
    fn challenge_is_replayable_and_binds_prior_messages() {
        let mut a = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        a.append_message(b"phase", b"one").unwrap();
        let ca = a.sparse_challenge(b"alpha", 0).unwrap();
        let mut b = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        b.append_message(b"phase", b"one").unwrap();
        assert_eq!(ca, b.sparse_challenge(b"alpha", 0).unwrap());
        let mut c = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        c.append_message(b"phase", b"two").unwrap();
        assert_ne!(ca, c.sparse_challenge(b"alpha", 0).unwrap());
    }

    #[test]
    fn field_challenge_is_nonzero_and_bound() {
        let mut a = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        let x = a.field_challenge(b"split-x-star", 0).unwrap();
        assert!(!x.is_zero());
        let mut b = JindoTranscriptV1::new(&binding(), [8; 32]).unwrap();
        assert_eq!(x, b.field_challenge(b"split-x-star", 0).unwrap());
    }

    #[test]
    fn split_challenge_boundary_is_exactly_the_nonzero_canonical_field() {
        let zero = [0_u8; 32];
        let one = {
            let mut bytes = [0_u8; 32];
            bytes[0] = 1;
            bytes
        };
        let modulus_minus_two = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xa0, 0xf9, 0x0e, 0x57, 0x64, 0x77, 0xbe, 0x54,
            0xe8, 0x17, 0xec, 0xae, 0x55, 0x03, 0x13, 0x70, 0xde, 0xc1, 0x7c, 0x27, 0x71, 0xb8,
            0x69, 0x09, 0x00, 0x40,
        ];
        let modulus_minus_one = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa1, 0xf9, 0x0e, 0x57, 0x64, 0x77, 0xbe, 0x54,
            0xe8, 0x17, 0xec, 0xae, 0x55, 0x03, 0x13, 0x70, 0xde, 0xc1, 0x7c, 0x27, 0x71, 0xb8,
            0x69, 0x09, 0x00, 0x40,
        ];
        let modulus = [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa1, 0xf9, 0x0e, 0x57, 0x64, 0x77, 0xbe, 0x54,
            0xe8, 0x17, 0xec, 0xae, 0x55, 0x03, 0x13, 0x70, 0xde, 0xc1, 0x7c, 0x27, 0x71, 0xb8,
            0x69, 0x09, 0x00, 0x40,
        ];

        assert_eq!(decode_nonzero_field_challenge(zero), None);
        assert_eq!(
            decode_nonzero_field_challenge(one),
            Some(JindoFieldElementV1::ONE)
        );
        assert!(decode_nonzero_field_challenge(modulus_minus_two).is_some());
        assert_eq!(
            decode_nonzero_field_challenge(modulus_minus_one),
            Some(-JindoFieldElementV1::ONE),
            "unlike the Go digit sampler, the paper-canonical sampler admits -1"
        );
        assert_eq!(decode_nonzero_field_challenge(modulus), None);
    }
}
