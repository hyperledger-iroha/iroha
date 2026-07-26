//! Closed first-release Vega Figure 9 mDL relation.
//!
//! The circuit proves the authenticated bytes themselves.  Native preflight in
//! `iroha_core` is only an early rejection path and is not part of the
//! relation's soundness argument.

use core::fmt;

use thiserror::Error;

use super::{
    VegaT256ScalarV1 as Scalar,
    circuit::{CircuitAssignment, CircuitBuilder, CircuitError},
    date::{
        enforce_completed_age, enforce_not_before, enforce_strictly_after, parse_full_date,
        parse_rfc3339_seconds, public_age_threshold, public_date,
    },
    figure9_layout::FIGURE9_LAYOUT,
    p256::{private_point_from_be_bytes, public_point, verify_es256},
    sha256::{ByteVar, allocate_bytes, enforce_byte_constant, public_word, sha256},
};

/// Exact number of public T256 scalars in the released Figure 9 relation.
pub const VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1: usize = 14;

const ISSUER_X_INDEX: usize = 0;
const ISSUER_Y_INDEX: usize = 1;
const DEVICE_DIGEST_WORD_START: usize = 2;
const PRESENTATION_YEAR_INDEX: usize = 10;
const PRESENTATION_MONTH_INDEX: usize = 11;
const PRESENTATION_DAY_INDEX: usize = 12;
const AGE_THRESHOLD_INDEX: usize = 13;

/// Failure at the public boundary of the fixed Figure 9 relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlFigure9ErrorV1 {
    /// A private byte string does not have the one released fixed width.
    #[error(
        "Vega Figure 9 witness field `{field}` has length {actual}, expected exactly {expected}"
    )]
    InvalidWitnessLength {
        /// Stable field label.
        field: &'static str,
        /// Supplied length.
        actual: usize,
        /// Required length.
        expected: usize,
    },
    /// A supposedly fixed byte differs from the one released deterministic
    /// CBOR encoding.
    #[error("Vega Figure 9 witness field `{field}` is not the released deterministic encoding")]
    NonCanonicalWitnessEncoding {
        /// Stable field label.
        field: &'static str,
    },
    /// Circuit synthesis or strict relation validation failed.
    #[error("Vega Figure 9 witness does not satisfy the released relation")]
    UnsatisfiedRelation,
}

/// Borrowed private inputs for the one released Figure 9 mDL relation.
///
/// The caller retains ownership so document bytes and signature witnesses do
/// not acquire another long-lived secret copy in the proof-system crate.
#[derive(Clone, Copy)]
pub struct VegaMdlFigure9WitnessV1<'a> {
    issuer_authentication_sig_structure: &'a [u8],
    birth_date_issuer_signed_item: &'a [u8],
    issuer_r: &'a [u8; 32],
    issuer_s_inverse: &'a [u8; 32],
    device_r: &'a [u8; 32],
    device_s_inverse: &'a [u8; 32],
}

impl<'a> VegaMdlFigure9WitnessV1<'a> {
    /// Construct a witness after enforcing the exact first-release byte shape.
    ///
    /// # Errors
    ///
    /// Returns [`VegaMdlFigure9ErrorV1::InvalidWitnessLength`] unless the
    /// issuer COSE `Sig_structure` is exactly 368 bytes and the Tag-24 birth
    /// item is exactly 92 bytes.
    pub fn new(
        issuer_authentication_sig_structure: &'a [u8],
        birth_date_issuer_signed_item: &'a [u8],
        issuer_r: &'a [u8; 32],
        issuer_s_inverse: &'a [u8; 32],
        device_r: &'a [u8; 32],
        device_s_inverse: &'a [u8; 32],
    ) -> Result<Self, VegaMdlFigure9ErrorV1> {
        validate_vega_mdl_figure9_encoding_v1(
            issuer_authentication_sig_structure,
            birth_date_issuer_signed_item,
        )?;
        Ok(Self {
            issuer_authentication_sig_structure,
            birth_date_issuer_signed_item,
            issuer_r,
            issuer_s_inverse,
            device_r,
            device_s_inverse,
        })
    }
}

/// Check the exact fixed-byte deterministic-CBOR Figure 9 profile.
///
/// Only the authenticated digest, P-256 device coordinates, three RFC 3339
/// timestamps, birth randomizer, and birth date may vary. All containers,
/// lengths, keys, algorithms, namespace values, and map ordering are fixed.
///
/// # Errors
///
/// Returns a length or deterministic-encoding error at the first public
/// boundary, before any circuit allocation.
pub fn validate_vega_mdl_figure9_encoding_v1(
    issuer_authentication_sig_structure: &[u8],
    birth_date_issuer_signed_item: &[u8],
) -> Result<(), VegaMdlFigure9ErrorV1> {
    validate_profile_encoding(
        "issuer_authentication_sig_structure",
        issuer_authentication_sig_structure,
        &FIGURE9_LAYOUT.issuer_template,
        &FIGURE9_LAYOUT.issuer_fixed,
    )?;
    validate_profile_encoding(
        "birth_date_issuer_signed_item",
        birth_date_issuer_signed_item,
        &FIGURE9_LAYOUT.birth_template,
        &FIGURE9_LAYOUT.birth_fixed,
    )
}

impl fmt::Debug for VegaMdlFigure9WitnessV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VegaMdlFigure9WitnessV1")
            .field(
                "issuer_authentication_sig_structure_bytes",
                &self.issuer_authentication_sig_structure.len(),
            )
            .field(
                "birth_date_issuer_signed_item_bytes",
                &self.birth_date_issuer_signed_item.len(),
            )
            .field("private_values", &"[REDACTED]")
            .finish()
    }
}

/// Validate the complete strict Figure 9 relation without creating a proof.
///
/// This is useful for prover-side diagnostics and cross-crate integration
/// tests. Production proof creation uses the same deterministic synthesis.
///
/// # Errors
///
/// Returns [`VegaMdlFigure9ErrorV1::UnsatisfiedRelation`] if any authenticated
/// byte, hash, P-256 signature, date, expiry, or completed-age constraint
/// fails.
pub fn validate_vega_mdl_figure9_relation_v1(
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
) -> Result<(), VegaMdlFigure9ErrorV1> {
    let assignment = synthesize_figure9(public_inputs, witness)
        .map_err(|_| VegaMdlFigure9ErrorV1::UnsatisfiedRelation)?;
    assignment
        .shape
        .validate_relaxed_assignment(
            &assignment.witness,
            Scalar::one(),
            &assignment.public_inputs,
            &vec![Scalar::zero(); assignment.shape.constraint_count()],
        )
        .map_err(|_| VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
}

pub(super) fn synthesize_figure9(
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
) -> Result<CircuitAssignment, CircuitError> {
    let layout = &*FIGURE9_LAYOUT;
    if witness.issuer_authentication_sig_structure.len() != layout.issuer_template.len()
        || witness.birth_date_issuer_signed_item.len() != layout.birth_template.len()
    {
        return Err(CircuitError::InvalidDimension);
    }

    let mut builder = CircuitBuilder::new(public_inputs.to_vec())?;
    let issuer = allocate_profile_bytes(
        &mut builder,
        witness.issuer_authentication_sig_structure,
        &layout.issuer_template,
        &layout.issuer_fixed,
    )?;
    let birth = allocate_profile_bytes(
        &mut builder,
        witness.birth_date_issuer_signed_item,
        &layout.birth_template,
        &layout.birth_fixed,
    )?;

    // The disclosed birth item is bound to the exact digest entry in the
    // issuer-authenticated MSO bytes.
    let birth_digest = sha256(&mut builder, &birth)?;
    bind_digest_to_bytes(
        &mut builder,
        birth_digest,
        &issuer[layout.issuer_birth_digest.clone()],
    )?;

    // The device key is private but authenticated as part of the issuer's
    // exact Sig_structure.
    let device_key = private_point_from_be_bytes(
        &mut builder,
        &issuer[layout.issuer_device_x.clone()],
        &issuer[layout.issuer_device_y.clone()],
    )?;

    // Parse and constrain every private calendar value, including the two
    // validity endpoints that cheap native preflight must not be trusted to
    // enforce for soundness.
    let signed =
        parse_rfc3339_seconds(&mut builder, &issuer[layout.issuer_signed_datetime.clone()])?;
    let valid_from = parse_rfc3339_seconds(
        &mut builder,
        &issuer[layout.issuer_valid_from_datetime.clone()],
    )?;
    let valid_until = parse_rfc3339_seconds(
        &mut builder,
        &issuer[layout.issuer_valid_until_datetime.clone()],
    )?;
    let birth_date = parse_full_date(&mut builder, &birth[layout.birth_date.clone()])?;
    let presentation = public_date(
        &mut builder,
        PRESENTATION_YEAR_INDEX,
        PRESENTATION_MONTH_INDEX,
        PRESENTATION_DAY_INDEX,
    )?;
    let (threshold, _) = public_age_threshold(&mut builder, AGE_THRESHOLD_INDEX)?;
    enforce_not_before(&mut builder, &valid_from, &signed)?;
    enforce_not_before(&mut builder, &presentation, &valid_from)?;
    enforce_strictly_after(&mut builder, &valid_until, &presentation)?;
    enforce_completed_age(&mut builder, &birth_date, &presentation, threshold)?;

    let issuer_digest = sha256(&mut builder, &issuer)?;
    let issuer_key = public_point(&mut builder, ISSUER_X_INDEX, ISSUER_Y_INDEX)?;
    verify_es256(
        &mut builder,
        issuer_digest,
        &issuer_key,
        *witness.issuer_r,
        *witness.issuer_s_inverse,
    )?;

    let device_digest = (0..8)
        .map(|offset| public_word(&mut builder, DEVICE_DIGEST_WORD_START + offset))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| CircuitError::InvalidDimension)?;
    verify_es256(
        &mut builder,
        device_digest,
        &device_key,
        *witness.device_r,
        *witness.device_s_inverse,
    )?;

    builder.finalize()
}

fn allocate_profile_bytes(
    builder: &mut CircuitBuilder,
    actual: &[u8],
    template: &[u8],
    fixed: &[bool],
) -> Result<Vec<ByteVar>, CircuitError> {
    if actual.len() != template.len() || actual.len() != fixed.len() {
        return Err(CircuitError::InvalidDimension);
    }
    let allocated = allocate_bytes(builder, actual)?;
    for ((byte, expected), is_fixed) in allocated
        .iter()
        .copied()
        .zip(template.iter().copied())
        .zip(fixed.iter().copied())
    {
        if is_fixed {
            enforce_byte_constant(builder, byte, expected)?;
        }
    }
    Ok(allocated)
}

fn bind_digest_to_bytes(
    builder: &mut CircuitBuilder,
    digest: [super::sha256::WordVar; 8],
    bytes: &[ByteVar],
) -> Result<(), CircuitError> {
    if bytes.len() != 32 {
        return Err(CircuitError::InvalidDimension);
    }
    for (computed, embedded) in digest
        .into_iter()
        .flat_map(super::sha256::WordVar::to_be_bytes)
        .zip(bytes.iter().copied())
    {
        builder.enforce_equal(computed.lc(), embedded.lc())?;
    }
    Ok(())
}

fn validate_length(
    field: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), VegaMdlFigure9ErrorV1> {
    if actual != expected {
        return Err(VegaMdlFigure9ErrorV1::InvalidWitnessLength {
            field,
            actual,
            expected,
        });
    }
    Ok(())
}

fn validate_profile_encoding(
    field: &'static str,
    actual: &[u8],
    template: &[u8],
    fixed: &[bool],
) -> Result<(), VegaMdlFigure9ErrorV1> {
    validate_length(field, actual.len(), template.len())?;
    if actual
        .iter()
        .zip(template)
        .zip(fixed)
        .any(|((&actual, &expected), &is_fixed)| is_fixed && actual != expected)
    {
        return Err(VegaMdlFigure9ErrorV1::NonCanonicalWitnessEncoding { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_signature_material() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        ([1; 32], [2; 32], [3; 32], [4; 32])
    }

    #[test]
    fn encoding_boundary_rejects_every_fixed_byte_and_wrong_length() {
        let layout = &*FIGURE9_LAYOUT;
        let issuer = layout.issuer_template.clone();
        let birth = layout.birth_template.clone();
        validate_vega_mdl_figure9_encoding_v1(&issuer, &birth).expect("template encoding");

        for (index, fixed) in layout.issuer_fixed.iter().copied().enumerate() {
            if fixed {
                let mut altered = issuer.clone();
                altered[index] ^= 1;
                assert!(
                    validate_vega_mdl_figure9_encoding_v1(&altered, &birth).is_err(),
                    "issuer fixed byte {index}"
                );
            }
        }
        for (index, fixed) in layout.birth_fixed.iter().copied().enumerate() {
            if fixed {
                let mut altered = birth.clone();
                altered[index] ^= 1;
                assert!(
                    validate_vega_mdl_figure9_encoding_v1(&issuer, &altered).is_err(),
                    "birth fixed byte {index}"
                );
            }
        }

        assert!(
            validate_vega_mdl_figure9_encoding_v1(&issuer[..issuer.len() - 1], &birth).is_err()
        );
        let mut trailing = birth.clone();
        trailing.push(0);
        assert!(validate_vega_mdl_figure9_encoding_v1(&issuer, &trailing).is_err());
    }

    #[test]
    fn encoding_boundary_classifies_every_variable_byte_and_redacts_debug() {
        let layout = &*FIGURE9_LAYOUT;
        for (index, fixed) in layout.issuer_fixed.iter().copied().enumerate() {
            if !fixed {
                let mut altered = layout.issuer_template.clone();
                altered[index] ^= 1;
                validate_vega_mdl_figure9_encoding_v1(&altered, &layout.birth_template)
                    .unwrap_or_else(|_| panic!("issuer variable byte {index}"));
            }
        }
        for (index, fixed) in layout.birth_fixed.iter().copied().enumerate() {
            if !fixed {
                let mut altered = layout.birth_template.clone();
                altered[index] ^= 1;
                validate_vega_mdl_figure9_encoding_v1(&layout.issuer_template, &altered)
                    .unwrap_or_else(|_| panic!("birth variable byte {index}"));
            }
        }

        let (issuer_r, issuer_s_inverse, device_r, device_s_inverse) =
            dummy_signature_material();
        let witness = VegaMdlFigure9WitnessV1::new(
            &layout.issuer_template,
            &layout.birth_template,
            &issuer_r,
            &issuer_s_inverse,
            &device_r,
            &device_s_inverse,
        )
        .expect("closed encoding");
        let debug = format!("{witness:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(&hex::encode(issuer_r)));
    }
}
