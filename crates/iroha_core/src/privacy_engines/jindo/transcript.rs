//! Canonically framed Fiat--Shamir transcript for Jindo.
//!
//! Challenges use exactly 120 bits, matching the published parameter search.
//! The 15-byte value is injected into one balanced base-183 digit in
//! `[-91, 91]` followed by fifteen balanced base-181 digits in `[-90, 90]`.
//! The mixed-radix space has more than `2^120` elements, while its maximum
//! Euclidean norm remains below the parameter search's challenge norm.  This
//! mapping is injective and every nonzero difference decodes to a nonzero
//! element of the prime coefficient field because its integer magnitude is
//! strictly below `p = 60272^16 + 1`.

use sha3::{
    Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use thiserror::Error;

use crate::privacy_engines::p256::{P256EngineError, TranscriptBindingV1};

use super::{
    JINDO_RING_DEGREE_V1,
    ring::{JindoPrimeModulusV1, JindoRnsPolynomialV1},
};

const TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.privacy.jindo.transcript.v1";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.privacy.jindo.challenge.v1";
const TRANSCRIPT_VERSION_V1: u8 = 1;
const JINDO_CHALLENGE_BYTES_V1: usize = 15;
const JINDO_CHALLENGE_FIRST_RADIX_V1: u128 = 183;
const JINDO_CHALLENGE_REMAINING_RADIX_V1: u128 = 181;
const JINDO_CHALLENGE_FIRST_OFFSET_V1: i16 = 91;
const JINDO_CHALLENGE_REMAINING_OFFSET_V1: i16 = 90;
const MAX_CHALLENGE_RETRIES_V1: u32 = 1 << 16;

/// One accepted 120-bit short challenge in coefficient form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JindoShortChallengeV1 {
    coefficients: [i16; 16],
}

impl JindoShortChallengeV1 {
    pub(crate) fn polynomial(self, moduli: [JindoPrimeModulusV1; 2]) -> JindoRnsPolynomialV1 {
        let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
        for (digit, coefficient) in self.coefficients.into_iter().enumerate() {
            coefficients[digit * 16] = i128::from(coefficient);
        }
        JindoRnsPolynomialV1::from_balanced_coefficients(coefficients, moduli)
    }

    #[cfg(test)]
    const fn coefficients(&self) -> &[i16; 16] {
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
            state: Vec::with_capacity(1024),
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

    pub(crate) fn challenge(
        &mut self,
        label: &[u8],
        ordinal: u32,
    ) -> Result<JindoShortChallengeV1, JindoTranscriptErrorV1> {
        if label.len() > usize::from(u16::MAX) {
            return Err(JindoTranscriptErrorV1::FieldTooLarge);
        }
        for retry in 0..MAX_CHALLENGE_RETRIES_V1 {
            let mut hash = Shake256::default();
            hash.update(
                &u64::try_from(CHALLENGE_DOMAIN_V1.len())
                    .expect("fixed challenge domain length fits u64")
                    .to_be_bytes(),
            );
            hash.update(CHALLENGE_DOMAIN_V1);
            hash.update(
                &u64::try_from(self.state.len())
                    .map_err(|_| JindoTranscriptErrorV1::FieldTooLarge)?
                    .to_be_bytes(),
            );
            hash.update(&self.state);
            hash.update(
                &u16::try_from(label.len())
                    .expect("label length prevalidated")
                    .to_be_bytes(),
            );
            hash.update(label);
            hash.update(&ordinal.to_be_bytes());
            hash.update(&retry.to_be_bytes());
            let mut reader = hash.finalize_xof();
            let mut bytes = [0_u8; JINDO_CHALLENGE_BYTES_V1];
            reader.read(&mut bytes);
            let challenge = decode_challenge(bytes);
            if challenge
                .coefficients
                .iter()
                .all(|coefficient| *coefficient == 0)
            {
                continue;
            }
            self.append_message(b"challenge_label", label)?;
            self.append_message(b"challenge_ordinal", &ordinal.to_be_bytes())?;
            self.append_message(b"challenge_retry", &retry.to_be_bytes())?;
            self.append_message(b"challenge_value", &bytes)?;
            return Ok(challenge);
        }
        Err(JindoTranscriptErrorV1::ChallengeExhausted)
    }
}

fn decode_challenge(bytes: [u8; JINDO_CHALLENGE_BYTES_V1]) -> JindoShortChallengeV1 {
    let mut wide = [0_u8; 16];
    wide[1..].copy_from_slice(&bytes);
    let mut value = u128::from_be_bytes(wide);
    let mut coefficients = [0_i16; 16];
    let first_digit = value % JINDO_CHALLENGE_FIRST_RADIX_V1;
    value /= JINDO_CHALLENGE_FIRST_RADIX_V1;
    coefficients[0] = i16::try_from(first_digit).expect("base-183 digit fits i16")
        - JINDO_CHALLENGE_FIRST_OFFSET_V1;
    for coefficient in &mut coefficients[1..] {
        let digit = value % JINDO_CHALLENGE_REMAINING_RADIX_V1;
        value /= JINDO_CHALLENGE_REMAINING_RADIX_V1;
        *coefficient = i16::try_from(digit).expect("base-181 digit fits i16")
            - JINDO_CHALLENGE_REMAINING_OFFSET_V1;
    }
    debug_assert_eq!(
        value, 0,
        "183 * 181^15 exceeds the 120-bit challenge domain"
    );
    JindoShortChallengeV1 { coefficients }
}

/// Jindo transcript failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoTranscriptErrorV1 {
    /// A mandatory shared consensus binding was malformed.
    #[error("invalid Jindo consensus transcript binding: {0}")]
    Binding(P256EngineError),
    /// The runtime supplied a CRS digest other than the compiled transparent key.
    #[error("Jindo transcript CRS digest differs from the compiled transparent key")]
    CrsDigestMismatch,
    /// A transcript label, value, or accumulated state exceeded its fixed framing.
    #[error("Jindo transcript field is too large")]
    FieldTooLarge,
    /// Every bounded candidate was the excluded all-zero short challenge.
    #[error("Jindo short-challenge derivation exhausted its fixed retry budget")]
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
    fn mixed_radix_mapping_covers_bounds_and_preserves_all_120_input_bits() {
        for bytes in [
            [0; JINDO_CHALLENGE_BYTES_V1],
            [u8::MAX; JINDO_CHALLENGE_BYTES_V1],
            {
                let mut value = [0; JINDO_CHALLENGE_BYTES_V1];
                value[JINDO_CHALLENGE_BYTES_V1 - 1] = 1;
                value
            },
        ] {
            let challenge = decode_challenge(bytes);
            assert!(
                (-JINDO_CHALLENGE_FIRST_OFFSET_V1..=JINDO_CHALLENGE_FIRST_OFFSET_V1)
                    .contains(&challenge.coefficients()[0])
            );
            assert!(challenge.coefficients()[1..].iter().all(|coefficient| {
                (-JINDO_CHALLENGE_REMAINING_OFFSET_V1..=JINDO_CHALLENGE_REMAINING_OFFSET_V1)
                    .contains(coefficient)
            }));
            let mut reconstructed = 0_u128;
            for coefficient in challenge.coefficients()[1..].iter().rev() {
                reconstructed = reconstructed * JINDO_CHALLENGE_REMAINING_RADIX_V1
                    + u128::try_from(*coefficient + JINDO_CHALLENGE_REMAINING_OFFSET_V1)
                        .expect("shifted base-181 digit is nonnegative");
            }
            reconstructed = reconstructed * JINDO_CHALLENGE_FIRST_RADIX_V1
                + u128::try_from(challenge.coefficients()[0] + JINDO_CHALLENGE_FIRST_OFFSET_V1)
                    .expect("shifted base-183 digit is nonnegative");
            let mut wide = [0; 16];
            wide[1..].copy_from_slice(&bytes);
            assert_eq!(reconstructed, u128::from_be_bytes(wide));
        }
    }

    #[test]
    fn challenge_space_and_norm_cover_the_full_security_target() {
        let mixed_radix_space =
            JINDO_CHALLENGE_FIRST_RADIX_V1 * JINDO_CHALLENGE_REMAINING_RADIX_V1.pow(15);
        assert!(mixed_radix_space > 1_u128 << 120);

        let maximum_norm_squared = i32::from(JINDO_CHALLENGE_FIRST_OFFSET_V1).pow(2)
            + 15 * i32::from(JINDO_CHALLENGE_REMAINING_OFFSET_V1).pow(2);
        // The parameter search uses c_1 = sqrt(16) * 2^(120/16) / 2,
        // whose square is exactly 131_072.
        assert!(maximum_norm_squared < 131_072);
    }

    #[test]
    fn transcript_binds_every_consensus_field_and_prior_message() {
        let expected = [8; 32];
        let mut first = JindoTranscriptV1::new(&binding(), expected).expect("transcript");
        first.append_message(b"commitments", b"one").unwrap();
        let first_challenge = first.challenge(b"batch", 0).expect("challenge");

        let mut replay = JindoTranscriptV1::new(&binding(), expected).expect("transcript");
        replay.append_message(b"commitments", b"one").unwrap();
        assert_eq!(
            replay.challenge(b"batch", 0).expect("challenge"),
            first_challenge
        );

        let mut changed = JindoTranscriptV1::new(&binding(), expected).expect("transcript");
        changed.append_message(b"commitments", b"two").unwrap();
        assert_ne!(
            changed.challenge(b"batch", 0).expect("challenge"),
            first_challenge
        );

        let mut wrong_binding = binding();
        wrong_binding.statement_digest[0] ^= 1;
        let mut changed =
            JindoTranscriptV1::new(&wrong_binding, expected).expect("changed transcript");
        changed.append_message(b"commitments", b"one").unwrap();
        assert_ne!(
            changed.challenge(b"batch", 0).expect("challenge"),
            first_challenge
        );
    }

    #[test]
    fn wrong_or_zero_binding_fails_closed() {
        assert!(matches!(
            JindoTranscriptV1::new(&binding(), [9; 32]),
            Err(JindoTranscriptErrorV1::CrsDigestMismatch)
        ));
        let mut invalid = binding();
        invalid.genesis_hash = [0; 32];
        assert!(matches!(
            JindoTranscriptV1::new(&invalid, [8; 32]),
            Err(JindoTranscriptErrorV1::Binding(_))
        ));
    }
}
