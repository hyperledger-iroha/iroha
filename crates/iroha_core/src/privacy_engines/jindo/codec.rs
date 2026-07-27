//! Fixed-width canonical proof wire for the native Jindo profile.
//!
//! The proof shape is determined entirely by the compiled parameter tuple.
//! Decoding checks the byte cap and exact length before allocating, then checks
//! every RNS residue against its pinned prime.  There are no attacker-selected
//! vector lengths and no alternate encodings of one ring element.

use thiserror::Error;

use super::{
    JINDO_RING_DEGREE_V1,
    parameters::JINDO_PARAMETERS_V1,
    ring::{
        JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoPrimeModulusV1, JindoRnsPolynomialV1,
    },
};

/// Exact proof magic.
pub const JINDO_PROOF_MAGIC_V1: [u8; 4] = *b"IJP1";
/// Exact proof version.
pub const JINDO_PROOF_VERSION_V1: u8 = 1;
/// Fixed proof-header byte width.
pub const JINDO_PROOF_HEADER_BYTES_V1: usize = 8;
/// Exact outer-ring polynomial count in one proof.
pub const JINDO_PROOF_OUTER_POLYNOMIALS_V1: usize = 30;
/// Exact inner-ring polynomial count in one proof.
pub const JINDO_PROOF_INNER_POLYNOMIALS_V1: usize = 66;
/// Exact byte width of one two-prime RNS polynomial.
pub const JINDO_PROOF_RNS_POLYNOMIAL_BYTES_V1: usize =
    2 * JINDO_RING_DEGREE_V1 * core::mem::size_of::<u64>();
/// Exact canonical proof byte width.
pub const JINDO_PROOF_BYTES_V1: usize = JINDO_PROOF_HEADER_BYTES_V1
    + (JINDO_PROOF_OUTER_POLYNOMIALS_V1 + JINDO_PROOF_INNER_POLYNOMIALS_V1)
        * JINDO_PROOF_RNS_POLYNOMIAL_BYTES_V1;

const PARTIAL_POLYNOMIALS_V1: usize = 1;
const PARTIAL_MASK_POLYNOMIALS_V1: usize = 1;
const ENCODE_RESPONSE_POLYNOMIALS_V1: usize = 17;
const MLWE_RESPONSE_POLYNOMIALS_V1: usize = 47;

const _: () = {
    assert!(
        JINDO_PROOF_OUTER_POLYNOMIALS_V1
            == JINDO_PARAMETERS_V1.inner_msis_rank * (JINDO_PARAMETERS_V1.columns + 1)
    );
    assert!(PARTIAL_POLYNOMIALS_V1 == JINDO_PARAMETERS_V1.columns);
    assert!(ENCODE_RESPONSE_POLYNOMIALS_V1 == JINDO_PARAMETERS_V1.rows);
    assert!(
        MLWE_RESPONSE_POLYNOMIALS_V1
            == JINDO_PARAMETERS_V1.mlwe_rank + JINDO_PARAMETERS_V1.inner_msis_rank
    );
    assert!(
        JINDO_PROOF_INNER_POLYNOMIALS_V1
            == PARTIAL_POLYNOMIALS_V1
                + PARTIAL_MASK_POLYNOMIALS_V1
                + ENCODE_RESPONSE_POLYNOMIALS_V1
                + MLWE_RESPONSE_POLYNOMIALS_V1
    );
};

/// Strictly decoded Jindo evaluation proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JindoEvaluationProofV1 {
    pub(crate) batch_count: u8,
    pub(crate) rounded_inner_commitments: Vec<JindoRnsPolynomialV1>,
    pub(crate) partials: Vec<JindoRnsPolynomialV1>,
    pub(crate) partial_mask: JindoRnsPolynomialV1,
    pub(crate) encode_responses: Vec<JindoRnsPolynomialV1>,
    pub(crate) mlwe_responses: Vec<JindoRnsPolynomialV1>,
}

impl JindoEvaluationProofV1 {
    /// Construct a proof after checking every fixed shape.
    pub(crate) fn new(
        batch_count: u8,
        rounded_inner_commitments: Vec<JindoRnsPolynomialV1>,
        partials: Vec<JindoRnsPolynomialV1>,
        partial_mask: JindoRnsPolynomialV1,
        encode_responses: Vec<JindoRnsPolynomialV1>,
        mlwe_responses: Vec<JindoRnsPolynomialV1>,
    ) -> Result<Self, JindoProofCodecErrorV1> {
        validate_batch_count(batch_count)?;
        validate_count(
            JindoProofSectionV1::RoundedInnerCommitments,
            rounded_inner_commitments.len(),
            JINDO_PROOF_OUTER_POLYNOMIALS_V1,
        )?;
        validate_count(
            JindoProofSectionV1::Partials,
            partials.len(),
            PARTIAL_POLYNOMIALS_V1,
        )?;
        validate_count(
            JindoProofSectionV1::EncodeResponses,
            encode_responses.len(),
            ENCODE_RESPONSE_POLYNOMIALS_V1,
        )?;
        validate_count(
            JindoProofSectionV1::MlweResponses,
            mlwe_responses.len(),
            MLWE_RESPONSE_POLYNOMIALS_V1,
        )?;
        Ok(Self {
            batch_count,
            rounded_inner_commitments,
            partials,
            partial_mask,
            encode_responses,
            mlwe_responses,
        })
    }

    /// Decode one exact proof without trusting any embedded count.
    pub(crate) fn decode_exact(
        bytes: &[u8],
        expected_batch_count: usize,
        max_bytes: u32,
    ) -> Result<Self, JindoProofCodecErrorV1> {
        let observed =
            u64::try_from(bytes.len()).map_err(|_| JindoProofCodecErrorV1::LengthOverflow)?;
        if observed > u64::from(max_bytes) {
            return Err(JindoProofCodecErrorV1::TooLarge {
                bytes: observed,
                max: max_bytes,
            });
        }
        if bytes.len() != JINDO_PROOF_BYTES_V1 {
            return Err(JindoProofCodecErrorV1::WrongLength {
                bytes: observed,
                expected: u64::try_from(JINDO_PROOF_BYTES_V1)
                    .expect("fixed Jindo proof length fits u64"),
            });
        }
        if bytes[..4] != JINDO_PROOF_MAGIC_V1 {
            return Err(JindoProofCodecErrorV1::InvalidMagic);
        }
        if bytes[4] != JINDO_PROOF_VERSION_V1 {
            return Err(JindoProofCodecErrorV1::UnsupportedVersion { version: bytes[4] });
        }
        let batch_count = bytes[5];
        validate_batch_count(batch_count)?;
        if usize::from(batch_count) != expected_batch_count {
            return Err(JindoProofCodecErrorV1::BatchCountMismatch {
                proof: batch_count,
                statement: u8::try_from(expected_batch_count).unwrap_or(u8::MAX),
            });
        }
        if bytes[6] != 0 {
            return Err(JindoProofCodecErrorV1::NonZeroFlags { flags: bytes[6] });
        }
        if bytes[7] != 0 {
            return Err(JindoProofCodecErrorV1::NonZeroReserved { reserved: bytes[7] });
        }

        let mut cursor = JINDO_PROOF_HEADER_BYTES_V1;
        let mut polynomial_index = 0_u16;
        let rounded_inner_commitments = read_polynomials(
            bytes,
            &mut cursor,
            &mut polynomial_index,
            JINDO_PROOF_OUTER_POLYNOMIALS_V1,
            JINDO_OUTER_MODULI_V1,
        )?;
        let partials = read_polynomials(
            bytes,
            &mut cursor,
            &mut polynomial_index,
            PARTIAL_POLYNOMIALS_V1,
            JINDO_INNER_MODULI_V1,
        )?;
        let partial_mask = read_polynomials(
            bytes,
            &mut cursor,
            &mut polynomial_index,
            PARTIAL_MASK_POLYNOMIALS_V1,
            JINDO_INNER_MODULI_V1,
        )?
        .pop()
        .expect("fixed one-polynomial mask section");
        let encode_responses = read_polynomials(
            bytes,
            &mut cursor,
            &mut polynomial_index,
            ENCODE_RESPONSE_POLYNOMIALS_V1,
            JINDO_INNER_MODULI_V1,
        )?;
        let mlwe_responses = read_polynomials(
            bytes,
            &mut cursor,
            &mut polynomial_index,
            MLWE_RESPONSE_POLYNOMIALS_V1,
            JINDO_INNER_MODULI_V1,
        )?;
        debug_assert_eq!(cursor, bytes.len());
        Self::new(
            batch_count,
            rounded_inner_commitments,
            partials,
            partial_mask,
            encode_responses,
            mlwe_responses,
        )
    }

    /// Encode the unique fixed-width representation.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(JINDO_PROOF_BYTES_V1);
        bytes.extend_from_slice(&JINDO_PROOF_MAGIC_V1);
        bytes.push(JINDO_PROOF_VERSION_V1);
        bytes.push(self.batch_count);
        bytes.extend_from_slice(&[0, 0]);
        for polynomial in &self.rounded_inner_commitments {
            write_polynomial(&mut bytes, polynomial);
        }
        for polynomial in &self.partials {
            write_polynomial(&mut bytes, polynomial);
        }
        write_polynomial(&mut bytes, &self.partial_mask);
        for polynomial in &self.encode_responses {
            write_polynomial(&mut bytes, polynomial);
        }
        for polynomial in &self.mlwe_responses {
            write_polynomial(&mut bytes, polynomial);
        }
        debug_assert_eq!(bytes.len(), JINDO_PROOF_BYTES_V1);
        bytes
    }
}

fn validate_batch_count(batch_count: u8) -> Result<(), JindoProofCodecErrorV1> {
    if batch_count == 0 || usize::from(batch_count) > JINDO_PARAMETERS_V1.max_batch_size {
        return Err(JindoProofCodecErrorV1::InvalidBatchCount {
            count: batch_count,
            max: u8::try_from(JINDO_PARAMETERS_V1.max_batch_size)
                .expect("fixed Jindo batch count fits u8"),
        });
    }
    Ok(())
}

fn validate_count(
    section: JindoProofSectionV1,
    count: usize,
    expected: usize,
) -> Result<(), JindoProofCodecErrorV1> {
    if count != expected {
        return Err(JindoProofCodecErrorV1::WrongPolynomialCount {
            section,
            count: u16::try_from(count).unwrap_or(u16::MAX),
            expected: u16::try_from(expected).expect("fixed Jindo polynomial count fits u16"),
        });
    }
    Ok(())
}

fn read_polynomials(
    bytes: &[u8],
    cursor: &mut usize,
    polynomial_index: &mut u16,
    count: usize,
    moduli: [JindoPrimeModulusV1; 2],
) -> Result<Vec<JindoRnsPolynomialV1>, JindoProofCodecErrorV1> {
    let mut polynomials = Vec::with_capacity(count);
    for _ in 0..count {
        let mut residues = [[0_u64; JINDO_RING_DEGREE_V1]; 2];
        for (modulus_index, (row, modulus)) in residues.iter_mut().zip(moduli).enumerate() {
            for (coefficient_index, coefficient) in row.iter_mut().enumerate() {
                let end = cursor
                    .checked_add(core::mem::size_of::<u64>())
                    .expect("fixed proof cursor cannot overflow");
                let encoded: [u8; 8] = bytes[*cursor..end]
                    .try_into()
                    .expect("exact proof length prevalidated");
                *cursor = end;
                let residue = u64::from_le_bytes(encoded);
                if residue >= modulus.modulus() {
                    return Err(JindoProofCodecErrorV1::NonCanonicalResidue {
                        polynomial_index: *polynomial_index,
                        modulus_index: u8::try_from(modulus_index)
                            .expect("two-prime modulus index fits u8"),
                        coefficient_index: u16::try_from(coefficient_index)
                            .expect("ring coefficient index fits u16"),
                        residue,
                        modulus: modulus.modulus(),
                    });
                }
                *coefficient = residue;
            }
        }
        polynomials.push(
            JindoRnsPolynomialV1::from_residues(residues, moduli)
                .expect("all residues were checked against their modulus"),
        );
        *polynomial_index = polynomial_index
            .checked_add(1)
            .expect("fixed Jindo proof polynomial index cannot overflow");
    }
    Ok(polynomials)
}

fn write_polynomial(bytes: &mut Vec<u8>, polynomial: &JindoRnsPolynomialV1) {
    for residue in polynomial.residues().iter().flatten() {
        bytes.extend_from_slice(&residue.to_le_bytes());
    }
}

/// Fixed proof section selected by a shape diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JindoProofSectionV1 {
    /// Batched, rounded inner commitments in the outer RNS basis.
    RoundedInnerCommitments,
    /// Evaluation partials for the committed data columns.
    Partials,
    /// Short encoded-column responses.
    EncodeResponses,
    /// Short MLWE-hiding responses.
    MlweResponses,
}

/// Strict Jindo proof-codec failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum JindoProofCodecErrorV1 {
    /// The platform cannot represent the supplied proof length.
    #[error("Jindo proof length cannot be represented")]
    LengthOverflow,
    /// The proof exceeds the governed action byte limit.
    #[error("Jindo proof uses {bytes} bytes, exceeding maximum {max}")]
    TooLarge {
        /// Observed proof bytes.
        bytes: u64,
        /// Governed maximum proof bytes.
        max: u32,
    },
    /// The proof does not have the profile's one exact fixed width.
    #[error("Jindo proof uses {bytes} bytes; expected exactly {expected}")]
    WrongLength {
        /// Observed proof bytes.
        bytes: u64,
        /// Exact compiled proof bytes.
        expected: u64,
    },
    /// The four-byte proof magic differs from `IJP1`.
    #[error("Jindo proof magic is invalid")]
    InvalidMagic,
    /// The proof declares an unsupported wire version.
    #[error("Jindo proof version {version} is unsupported")]
    UnsupportedVersion {
        /// Observed version byte.
        version: u8,
    },
    /// The embedded batch count is outside the compiled profile.
    #[error("Jindo proof batch count {count} is outside 1..={max}")]
    InvalidBatchCount {
        /// Observed batch count.
        count: u8,
        /// Compiled maximum batch count.
        max: u8,
    },
    /// The embedded batch count differs from the public statement.
    #[error("Jindo proof batch count {proof} differs from statement count {statement}")]
    BatchCountMismatch {
        /// Embedded proof batch count.
        proof: u8,
        /// Public statement batch count.
        statement: u8,
    },
    /// An unassigned proof flag was non-zero.
    #[error("Jindo proof flags byte must be zero, got {flags}")]
    NonZeroFlags {
        /// Observed flags byte.
        flags: u8,
    },
    /// The reserved header byte was non-zero.
    #[error("Jindo proof reserved byte must be zero, got {reserved}")]
    NonZeroReserved {
        /// Observed reserved byte.
        reserved: u8,
    },
    /// An in-memory proof section did not have its compiled polynomial count.
    #[error("Jindo proof section {section:?} has {count} polynomials; expected {expected}")]
    WrongPolynomialCount {
        /// Malformed proof section.
        section: JindoProofSectionV1,
        /// Observed polynomial count.
        count: u16,
        /// Exact compiled polynomial count.
        expected: u16,
    },
    /// A raw RNS word is not the canonical residue below its selected prime.
    #[error(
        "Jindo proof polynomial {polynomial_index}, modulus {modulus_index}, coefficient {coefficient_index} has residue {residue} outside [0,{modulus})"
    )]
    NonCanonicalResidue {
        /// Zero-based polynomial index in the fixed wire.
        polynomial_index: u16,
        /// Zero-based RNS-prime index.
        modulus_index: u8,
        /// Zero-based ring coefficient index.
        coefficient_index: u16,
        /// Observed raw residue.
        residue: u64,
        /// Exclusive canonical modulus bound.
        modulus: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn polynomial(seed: i128, moduli: [JindoPrimeModulusV1; 2]) -> JindoRnsPolynomialV1 {
        let coefficients =
            core::array::from_fn(|index| seed + i128::try_from(index % 17).expect("small index"));
        JindoRnsPolynomialV1::from_balanced_coefficients(coefficients, moduli)
    }

    fn proof(batch_count: u8) -> JindoEvaluationProofV1 {
        JindoEvaluationProofV1::new(
            batch_count,
            (0..JINDO_PROOF_OUTER_POLYNOMIALS_V1)
                .map(|index| {
                    polynomial(
                        i128::try_from(index).expect("small index") + 1,
                        JINDO_OUTER_MODULI_V1,
                    )
                })
                .collect(),
            vec![polynomial(40, JINDO_INNER_MODULI_V1)],
            polynomial(41, JINDO_INNER_MODULI_V1),
            (0..ENCODE_RESPONSE_POLYNOMIALS_V1)
                .map(|index| {
                    polynomial(
                        i128::try_from(index).expect("small index") + 50,
                        JINDO_INNER_MODULI_V1,
                    )
                })
                .collect(),
            (0..MLWE_RESPONSE_POLYNOMIALS_V1)
                .map(|index| {
                    polynomial(
                        i128::try_from(index).expect("small index") + 80,
                        JINDO_INNER_MODULI_V1,
                    )
                })
                .collect(),
        )
        .expect("fixed proof shape")
    }

    #[test]
    fn exact_codec_roundtrips_every_batch_count() {
        assert_eq!(JINDO_PROOF_BYTES_V1, 393_224);
        for batch_count in 1..=4 {
            let proof = proof(batch_count);
            let encoded = proof.encode();
            assert_eq!(encoded.len(), JINDO_PROOF_BYTES_V1);
            assert_eq!(
                JindoEvaluationProofV1::decode_exact(
                    &encoded,
                    usize::from(batch_count),
                    u32::try_from(encoded.len()).expect("proof length fits u32"),
                )
                .expect("exact proof"),
                proof
            );
        }
    }

    #[test]
    fn truncation_trailing_header_and_cap_mutations_fail_closed() {
        let encoded = proof(2).encode();
        let cap = u32::try_from(encoded.len()).expect("proof length fits u32");
        for end in [0, 1, 4, 7, 8, encoded.len() - 1] {
            assert!(JindoEvaluationProofV1::decode_exact(&encoded[..end], 2, cap).is_err());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(JindoEvaluationProofV1::decode_exact(&trailing, 2, cap + 1).is_err());
        assert!(JindoEvaluationProofV1::decode_exact(&encoded, 2, cap - 1).is_err());
        for (offset, value) in [(0, b'X'), (4, 2), (5, 0), (5, 5), (6, 1), (7, 1)] {
            let mut malformed = encoded.clone();
            malformed[offset] = value;
            assert!(JindoEvaluationProofV1::decode_exact(&malformed, 2, cap).is_err());
        }
        assert!(matches!(
            JindoEvaluationProofV1::decode_exact(&encoded, 1, cap),
            Err(JindoProofCodecErrorV1::BatchCountMismatch { .. })
        ));
    }

    #[test]
    fn either_rns_limb_at_its_modulus_is_rejected() {
        let encoded = proof(1).encode();
        let cap = u32::try_from(encoded.len()).expect("proof length fits u32");
        for (polynomial_index, moduli) in [
            (0_usize, JINDO_OUTER_MODULI_V1),
            (JINDO_PROOF_OUTER_POLYNOMIALS_V1, JINDO_INNER_MODULI_V1),
        ] {
            for (modulus_index, modulus) in moduli.into_iter().enumerate() {
                let offset = JINDO_PROOF_HEADER_BYTES_V1
                    + polynomial_index * JINDO_PROOF_RNS_POLYNOMIAL_BYTES_V1
                    + modulus_index * JINDO_RING_DEGREE_V1 * 8;
                let mut malformed = encoded.clone();
                malformed[offset..offset + 8].copy_from_slice(&modulus.modulus().to_le_bytes());
                assert!(matches!(
                    JindoEvaluationProofV1::decode_exact(&malformed, 1, cap),
                    Err(JindoProofCodecErrorV1::NonCanonicalResidue { .. })
                ));
            }
        }
    }
}
