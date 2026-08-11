//! Closed first-release Vega Figure 9 mDL relation.
//!
//! The circuit proves the authenticated bytes themselves.  Native preflight in
//! `iroha_core` is only an early rejection path and is not part of the
//! relation's soundness argument. Both ES256 checks reconstruct the unique
//! P1363 `s` scalar from the inverse witness and enforce the canonical low-s
//! representative inside the proof relation.

use core::fmt;

use thiserror::Error;

#[cfg(test)]
use super::{
    VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1,
    VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1,
};
use super::{
    VegaT256ScalarV1 as Scalar,
    circuit::{Bit, CircuitAssignment, CircuitBuilder, CircuitError, Variable},
    date::{
        enforce_completed_age, enforce_not_before, enforce_rfc3339_not_before,
        enforce_strictly_after, parse_full_date, parse_rfc3339_seconds, public_age_threshold,
        public_date,
    },
    figure9_layout::FIGURE9_LAYOUT,
    p256::{private_point_from_be_bytes, public_point, verify_es256_low_s_from_inverse},
    sha256::{
        ByteVar, Sha256Trace, WordVar, allocate_bytes, enforce_byte_constant, public_word,
        sha256_with_trace,
    },
};

/// Exact number of public T256 scalars in the released Figure 9 relation.
pub const VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1: usize = 14;
pub(super) const VEGA_MDL_FIGURE9_SHA256_STEPS_V1: usize = 8;

#[derive(Clone)]
pub(super) struct Figure9McMaterial {
    pub(super) assignment: CircuitAssignment,
}

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
/// `s^-1` is only an assignment representation: the circuit reconstructs `s`,
/// proves the ECDSA group equation, and admits only the low-s P1363
/// representative for both signatures.
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
    /// issuer COSE `Sig_structure` and Tag-24 birth item have their one
    /// canonical released widths.
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
        .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
        .map_err(|_| VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
}

pub(super) fn synthesize_figure9(
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
) -> Result<CircuitAssignment, CircuitError> {
    synthesize_figure9_mc_material(public_inputs, witness).map(|material| material.assignment)
}

pub(super) fn synthesize_figure9_mc_material(
    public_inputs: &[Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
) -> Result<Figure9McMaterial, CircuitError> {
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
    let (birth_digest, birth_trace) = sha256_with_trace(&mut builder, &birth)?;
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
    enforce_rfc3339_not_before(&mut builder, &valid_from, &signed)?;
    enforce_not_before(&mut builder, &presentation, &valid_from.date)?;
    enforce_strictly_after(&mut builder, &valid_until.date, &presentation)?;
    enforce_completed_age(&mut builder, &birth_date, &presentation, threshold)?;

    let (issuer_digest, issuer_trace) = sha256_with_trace(&mut builder, &issuer)?;
    let issuer_key = public_point(&mut builder, ISSUER_X_INDEX, ISSUER_Y_INDEX)?;
    verify_es256_low_s_from_inverse(
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
    verify_es256_low_s_from_inverse(
        &mut builder,
        device_digest,
        &device_key,
        *witness.device_r,
        *witness.device_s_inverse,
    )?;

    byte_indices(&issuer)?;
    byte_indices(&birth)?;
    let issuer_state_count = state_indices(&issuer_trace)?.len();
    let birth_state_count = state_indices(&birth_trace)?.len();
    if issuer_state_count != 6 || birth_state_count != 2 {
        return Err(CircuitError::InvalidDimension);
    }
    Ok(Figure9McMaterial {
        assignment: builder.finalize()?,
    })
}

fn byte_indices(bytes: &[ByteVar]) -> Result<Vec<[usize; 8]>, CircuitError> {
    bytes
        .iter()
        .copied()
        .map(|byte| bit_indices(byte.bits_le()))
        .collect()
}

fn state_indices(trace: &Sha256Trace) -> Result<Vec<[[usize; 32]; 8]>, CircuitError> {
    trace
        .states_after_blocks
        .iter()
        .map(|state| {
            state
                .iter()
                .copied()
                .map(WordVar::bits_le)
                .map(bit_indices)
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| CircuitError::InvalidDimension)
        })
        .collect()
}

fn bit_indices<const N: usize>(bits: [Bit; N]) -> Result<[usize; N], CircuitError> {
    bits.map(|bit| match bit.variable() {
        Variable::Private(index) => Ok(index),
        Variable::Public(_) | Variable::One => Err(CircuitError::InvalidDimension),
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?
    .try_into()
    .map_err(|_| CircuitError::InvalidDimension)
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
pub(super) mod tests {
    use super::*;

    const ISSUER_X: &str = "df666ab5a8f2c65017756b27cabae13b4b8e3864c5a4182884c4872920f43364";
    const ISSUER_Y: &str = "b2470a2618899b3bc06b6e6d356a68d7eefcc120c828c628edbeb4352068b6e7";
    const DEVICE_X: &str = "864145351e998d7aaab002ed334edf912fb26a0a699c704fdde71a9ee43867f8";
    const DEVICE_Y: &str = "88d8642588166c08f40726875227b8c74dc459d322055f7902f2f05eb724dc6d";
    const DEVICE_DIGEST: &str = "de99d281426b98f14f930f795f2a94263542621674bac0818fbda34718a6450e";
    const DEVICE_R: &str = "0d9d54525b87cd31f9ecc122fd40b0f6dcb094db325ed2632f304797b3e89a5a";
    const DEVICE_S_INVERSE: &str =
        "bae1741e8d463d4c2127b7fccd7b3bc12d7f5e64c73fcf934a26f7deb53ba23b";
    const BASELINE_BIRTH_DIGEST: &str =
        "367ab450ea6746eb0eace42be70947e253d054fab10cc106d48f5de4db629951";
    const BASELINE_ISSUER_R: &str =
        "5f3ae8409f2db3f2ec0e30e1e80e1d9c1a43080eb88d37a46daba02249a06905";
    const BASELINE_ISSUER_S_INVERSE: &str =
        "6d916194a7bae7a44dae93513631a521200220e6ade5c756c931db53b10f1cb9";
    const BIRTHDAY_EXACT_BIRTH_DIGEST: &str =
        "3f12cd2ca9ab70fb3790ad6be00e2dc34b04bf8840ecc699218ec07025cf51bf";
    const BIRTHDAY_EXACT_ISSUER_R: &str =
        "22277179a54a13c3881d02401b0d4c921ec54a060e5a3f43d8971ddee5d5b858";
    const BIRTHDAY_EXACT_ISSUER_S_INVERSE: &str =
        "7566b512e04f5ea2154fde1f185ff1eb2b0d4e204ffe3d424a1292a3abc16a27";
    const UNDERAGE_BIRTH_DIGEST: &str =
        "43a3c93a26394ff5c01cbe2eb70a22f6de08a7dcb1e7198444c579da1099af0e";
    const UNDERAGE_ISSUER_R: &str =
        "e26ad0bcc6479037e69059dcf7cc721be61d7375e8077214f839ddbb20a517ee";
    const UNDERAGE_ISSUER_S_INVERSE: &str =
        "8ec3e8ebdf829bea6b594311521eb0fdbdf3783eeb697070d20baf1d53363798";
    const EXPIRY_EXACT_ISSUER_R: &str =
        "16dc3175efc6a7bbebc57d38958205f4628ba2fbd21a0acd4b22ff3dba0b0c60";
    const EXPIRY_EXACT_ISSUER_S_INVERSE: &str =
        "b5277f18cbc2e334b0b41a41214087f9909ef0b8a1fbd9c03f99a2dcb514e5dc";
    const EXPIRY_NEXT_ISSUER_R: &str =
        "25ed1214e1596916aed9e6f15b53a780118b469359de088eac63608655237e90";
    const EXPIRY_NEXT_ISSUER_S_INVERSE: &str =
        "9ea68d5750396687f671c18d5ae82d8ffd98789fd06daac57359ec2022b2b08b";
    const FUTURE_VALID_FROM_ISSUER_R: &str =
        "955e099cd9fd81cf01ac901e3ad83b7de663539755bb0e4094b12810983e6d3e";
    const FUTURE_VALID_FROM_ISSUER_S_INVERSE: &str =
        "64c0b480152b088da99f564dbbb7ef876bc26cffa0d5d8ce39db3e53fca9816d";
    const SIGNED_AFTER_VALID_FROM_ISSUER_R: &str =
        "c544f2c2e94236f2b6ab81ae61e5f0b7ee89a1af8a08d730c9e26a9c16537122";
    const SIGNED_AFTER_VALID_FROM_ISSUER_S_INVERSE: &str =
        "b63409e4735522039616beda2651614d3f616b4a9205598677075e9418f5e482";

    #[derive(Clone)]
    pub(crate) struct SignedFixture {
        pub(crate) public: [Scalar; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
        pub(crate) issuer: Vec<u8>,
        pub(crate) birth: Vec<u8>,
        pub(crate) issuer_r: [u8; 32],
        pub(crate) issuer_s_inverse: [u8; 32],
        pub(crate) device_r: [u8; 32],
        pub(crate) device_s_inverse: [u8; 32],
    }

    impl SignedFixture {
        pub(crate) fn witness(&self) -> VegaMdlFigure9WitnessV1<'_> {
            VegaMdlFigure9WitnessV1::new(
                &self.issuer,
                &self.birth,
                &self.issuer_r,
                &self.issuer_s_inverse,
                &self.device_r,
                &self.device_s_inverse,
            )
            .expect("closed signed fixture")
        }
    }

    fn hex32(value: &str) -> [u8; 32] {
        hex::decode(value)
            .expect("hex")
            .try_into()
            .expect("32 bytes")
    }

    pub(crate) fn high_s_counterpart_inverse(low_s_inverse: [u8; 32]) -> [u8; 32] {
        let mut high_s_inverse = [0_u8; 32];
        let mut borrow = 0_u16;
        for index in (0..32).rev() {
            let minuend = u16::from(super::super::p256::P256_ORDER_BE[index]);
            let subtrahend = u16::from(low_s_inverse[index]) + borrow;
            if minuend >= subtrahend {
                high_s_inverse[index] =
                    u8::try_from(minuend - subtrahend).expect("single-byte difference");
                borrow = 0;
            } else {
                high_s_inverse[index] =
                    u8::try_from(minuend + 256 - subtrahend).expect("single-byte difference");
                borrow = 1;
            }
        }
        assert_eq!(borrow, 0, "fixture inverse is below the P-256 order");
        assert_ne!(high_s_inverse, [0; 32], "fixture inverse is nonzero");
        high_s_inverse
    }

    pub(crate) fn baseline_signed_fixture() -> SignedFixture {
        let mut issuer = FIGURE9_LAYOUT.issuer_template.clone();
        let birth = FIGURE9_LAYOUT.birth_template.clone();
        issuer[FIGURE9_LAYOUT.issuer_birth_digest.clone()]
            .copy_from_slice(&hex32(BASELINE_BIRTH_DIGEST));
        issuer[FIGURE9_LAYOUT.issuer_device_x.clone()].copy_from_slice(&hex32(DEVICE_X));
        issuer[FIGURE9_LAYOUT.issuer_device_y.clone()].copy_from_slice(&hex32(DEVICE_Y));

        let issuer_x = Scalar::from_be_bytes_exact(hex32(ISSUER_X)).expect("canonical issuer x");
        let issuer_y = Scalar::from_be_bytes_exact(hex32(ISSUER_Y)).expect("canonical issuer y");
        let digest = hex32(DEVICE_DIGEST);
        let digest_words = digest
            .chunks_exact(4)
            .map(|word| {
                Scalar::from_u64(u64::from(u32::from_be_bytes(
                    word.try_into().expect("word"),
                )))
            })
            .collect::<Vec<_>>();
        let public = core::array::from_fn(|index| match index {
            0 => issuer_x,
            1 => issuer_y,
            2..=9 => digest_words[index - 2],
            10 => Scalar::from_u64(2026),
            11 => Scalar::from_u64(7),
            12 => Scalar::from_u64(26),
            13 => Scalar::from_u64(18),
            _ => unreachable!("14 public inputs"),
        });
        SignedFixture {
            public,
            issuer,
            birth,
            issuer_r: hex32(BASELINE_ISSUER_R),
            issuer_s_inverse: hex32(BASELINE_ISSUER_S_INVERSE),
            device_r: hex32(DEVICE_R),
            device_s_inverse: hex32(DEVICE_S_INVERSE),
        }
    }

    fn signed_variant(
        birth_date: Option<(&[u8; 10], &str)>,
        signed: Option<&[u8; 20]>,
        valid_from: Option<&[u8; 20]>,
        valid_until: Option<&[u8; 20]>,
        issuer_r: &str,
        issuer_s_inverse: &str,
    ) -> SignedFixture {
        let mut fixture = baseline_signed_fixture();
        if let Some((birth_date, birth_digest)) = birth_date {
            fixture.birth[FIGURE9_LAYOUT.birth_date.clone()].copy_from_slice(birth_date);
            fixture.issuer[FIGURE9_LAYOUT.issuer_birth_digest.clone()]
                .copy_from_slice(&hex32(birth_digest));
        }
        if let Some(value) = signed {
            fixture.issuer[FIGURE9_LAYOUT.issuer_signed_datetime.clone()].copy_from_slice(value);
        }
        if let Some(value) = valid_from {
            fixture.issuer[FIGURE9_LAYOUT.issuer_valid_from_datetime.clone()]
                .copy_from_slice(value);
        }
        if let Some(value) = valid_until {
            fixture.issuer[FIGURE9_LAYOUT.issuer_valid_until_datetime.clone()]
                .copy_from_slice(value);
        }
        fixture.issuer_r = hex32(issuer_r);
        fixture.issuer_s_inverse = hex32(issuer_s_inverse);
        fixture
    }

    fn dummy_signature_material() -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        ([1; 32], [2; 32], [3; 32], [4; 32])
    }

    #[test]
    fn encoding_boundary_rejects_every_fixed_byte_and_wrong_length() {
        let layout = &*FIGURE9_LAYOUT;
        let issuer = layout.issuer_template.clone();
        let birth = layout.birth_template.clone();
        assert_eq!(
            issuer.len(),
            VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1
        );
        assert_eq!(birth.len(), VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1);
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
        let mut overlong_issuer = issuer.clone();
        overlong_issuer.push(0);
        assert!(validate_vega_mdl_figure9_encoding_v1(&overlong_issuer, &birth).is_err());
        assert!(validate_vega_mdl_figure9_encoding_v1(&issuer, &birth[..birth.len() - 1]).is_err());
        let mut overlong_birth = birth.clone();
        overlong_birth.push(0);
        assert!(validate_vega_mdl_figure9_encoding_v1(&issuer, &overlong_birth).is_err());
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

        let (issuer_r, issuer_s_inverse, device_r, device_s_inverse) = dummy_signature_material();
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

    #[test]
    fn independent_openssl_signed_figure9_vector_satisfies_the_complete_relation() {
        let fixture = baseline_signed_fixture();
        validate_vega_mdl_figure9_relation_v1(&fixture.public, &fixture.witness())
            .expect("complete signed relation");
    }

    #[test]
    fn issuer_and_device_high_s_counterparts_are_unsatisfied_in_the_circuit_relation() {
        let baseline = baseline_signed_fixture();
        for role in ["issuer", "device"] {
            let mut changed = baseline.clone();
            let original_inverse = if role == "issuer" {
                &mut changed.issuer_s_inverse
            } else {
                &mut changed.device_s_inverse
            };
            let high_s_inverse = high_s_counterpart_inverse(*original_inverse);
            assert_eq!(
                high_s_counterpart_inverse(high_s_inverse),
                *original_inverse,
                "P-256 scalar negation is involutive"
            );
            *original_inverse = high_s_inverse;
            assert_eq!(
                validate_vega_mdl_figure9_relation_v1(&changed.public, &changed.witness()),
                Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation),
                "{role} high-s counterpart bypassed the proof relation"
            );
        }
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "full signed boundary vectors are a release-mode circuit gate"
    )]
    fn independently_signed_calendar_boundaries_enforce_age_expiry_and_validity_order() {
        let birthday_exact = signed_variant(
            Some((b"2008-07-26", BIRTHDAY_EXACT_BIRTH_DIGEST)),
            None,
            None,
            None,
            BIRTHDAY_EXACT_ISSUER_R,
            BIRTHDAY_EXACT_ISSUER_S_INVERSE,
        );
        validate_vega_mdl_figure9_relation_v1(&birthday_exact.public, &birthday_exact.witness())
            .expect("exact eighteenth birthday is accepted");

        let underage = signed_variant(
            Some((b"2008-07-27", UNDERAGE_BIRTH_DIGEST)),
            None,
            None,
            None,
            UNDERAGE_ISSUER_R,
            UNDERAGE_ISSUER_S_INVERSE,
        );
        assert_eq!(
            validate_vega_mdl_figure9_relation_v1(&underage.public, &underage.witness()),
            Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
        );

        let expiry_exact = signed_variant(
            None,
            None,
            None,
            Some(b"2026-07-26T12:34:56Z"),
            EXPIRY_EXACT_ISSUER_R,
            EXPIRY_EXACT_ISSUER_S_INVERSE,
        );
        assert_eq!(
            validate_vega_mdl_figure9_relation_v1(&expiry_exact.public, &expiry_exact.witness()),
            Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
        );

        let expiry_next = signed_variant(
            None,
            None,
            None,
            Some(b"2026-07-27T12:34:56Z"),
            EXPIRY_NEXT_ISSUER_R,
            EXPIRY_NEXT_ISSUER_S_INVERSE,
        );
        validate_vega_mdl_figure9_relation_v1(&expiry_next.public, &expiry_next.witness())
            .expect("next-day expiry is accepted");

        let future_valid_from = signed_variant(
            None,
            None,
            Some(b"2026-07-27T04:05:06Z"),
            None,
            FUTURE_VALID_FROM_ISSUER_R,
            FUTURE_VALID_FROM_ISSUER_S_INVERSE,
        );
        assert_eq!(
            validate_vega_mdl_figure9_relation_v1(
                &future_valid_from.public,
                &future_valid_from.witness()
            ),
            Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
        );

        let signed_after_valid_from = signed_variant(
            None,
            Some(b"2026-08-01T03:04:05Z"),
            Some(b"2026-07-01T04:05:06Z"),
            None,
            SIGNED_AFTER_VALID_FROM_ISSUER_R,
            SIGNED_AFTER_VALID_FROM_ISSUER_S_INVERSE,
        );
        assert_eq!(
            validate_vega_mdl_figure9_relation_v1(
                &signed_after_valid_from.public,
                &signed_after_valid_from.witness()
            ),
            Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
        );
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "full authenticated-field adversarial matrix is a release-mode circuit gate"
    )]
    fn every_authenticated_input_class_and_statement_class_fails_under_adversarial_mutation() {
        let baseline = baseline_signed_fixture();
        let assert_unsatisfied = |fixture: &SignedFixture| {
            assert_eq!(
                validate_vega_mdl_figure9_relation_v1(&fixture.public, &fixture.witness()),
                Err(VegaMdlFigure9ErrorV1::UnsatisfiedRelation)
            );
        };

        for range in [
            FIGURE9_LAYOUT.issuer_birth_digest.clone(),
            FIGURE9_LAYOUT.issuer_device_x.clone(),
            FIGURE9_LAYOUT.issuer_device_y.clone(),
            FIGURE9_LAYOUT.issuer_signed_datetime.clone(),
            FIGURE9_LAYOUT.issuer_valid_from_datetime.clone(),
            FIGURE9_LAYOUT.issuer_valid_until_datetime.clone(),
        ] {
            let mut changed = baseline.clone();
            changed.issuer[range.start] ^= 1;
            assert_unsatisfied(&changed);
        }
        for range in [
            crate::vega::figure9_layout::FIGURE9_BIRTH_RANDOM_RANGE,
            FIGURE9_LAYOUT.birth_date.clone(),
        ] {
            let mut changed = baseline.clone();
            changed.birth[range.start] ^= 1;
            assert_unsatisfied(&changed);
        }
        for signature_field in 0..4 {
            let mut changed = baseline.clone();
            match signature_field {
                0 => changed.issuer_r[0] ^= 1,
                1 => changed.issuer_s_inverse[0] ^= 1,
                2 => changed.device_r[0] ^= 1,
                3 => changed.device_s_inverse[0] ^= 1,
                _ => unreachable!(),
            }
            assert_unsatisfied(&changed);
        }
        for index in 0..10 {
            let mut changed = baseline.clone();
            changed.public[index] += Scalar::one();
            assert_unsatisfied(&changed);
        }

        let mut before_valid_from = baseline.clone();
        before_valid_from.public[10] = Scalar::from_u64(2025);
        before_valid_from.public[11] = Scalar::from_u64(1);
        before_valid_from.public[12] = Scalar::from_u64(1);
        assert_unsatisfied(&before_valid_from);

        let mut expiry_boundary = baseline.clone();
        expiry_boundary.public[10] = Scalar::from_u64(2035);
        expiry_boundary.public[11] = Scalar::from_u64(8);
        expiry_boundary.public[12] = Scalar::from_u64(17);
        assert_unsatisfied(&expiry_boundary);

        let mut invalid_calendar = baseline.clone();
        invalid_calendar.public[11] = Scalar::from_u64(13);
        assert_unsatisfied(&invalid_calendar);

        let mut threshold_too_high = baseline;
        threshold_too_high.public[13] = Scalar::from_u64(40);
        assert_unsatisfied(&threshold_too_high);
    }
}
