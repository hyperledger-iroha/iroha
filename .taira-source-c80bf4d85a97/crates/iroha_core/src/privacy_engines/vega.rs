//! Canonical native Vega engine for ISO/IEC 18013-5 mDL age proofs.
//!
//! The proof system follows Microsoft `vega-prover` commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d` (MIT). The application relation
//! is the paper's Figure 9 mDL circuit, closed to one first-release profile.

mod cbor;
mod mdl;

use iroha_data_model::privacy::{
    PRIVACY_MAX_CHAIN_ID_BYTES_V1, PrivacyP256PointV1, PrivacyStatementContextV1,
    PrivacyVegaDeviceAuthenticationDigestV1, PrivacyVegaMdlDateV1,
    VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MAX_PRESENTATION_YEAR_V1,
    VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MIN_PRESENTATION_YEAR_V1,
    VegaExistingCredentialStatementV1,
};
use iroha_zkp_halo2::vega::{VegaFieldError, VegaT256ScalarV1};
use p256::{EncodedPoint, PublicKey, elliptic_curve::sec1::ToEncodedPoint};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::{Date, Month, OffsetDateTime};

pub use mdl::{
    VegaEcdsaWitnessV1, VegaMdlLookupTableV1, VegaMdlValidatedWitnessV1, VegaMdlWitnessV1,
    validate_mdl_witness,
};

/// Pinned upstream source revision implemented by this engine.
pub const VEGA_PINNED_SOURCE_COMMIT_V1: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
/// Canonical Figure 9 public-input count.
pub const VEGA_MDL_PUBLIC_INPUT_COUNT_V1: usize = 14;
/// Domain of the Iroha device-authentication binding frame.
pub const VEGA_MDL_DEVICE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"iroha.vega.mdl.device-authentication.v1";
/// Version of the Iroha device-authentication binding frame.
pub const VEGA_MDL_DEVICE_AUTHENTICATION_FRAME_VERSION_V1: u8 = 1;
/// Exact ISO/IEC 18013-5 mDL document type.
pub const VEGA_MDL_DOCUMENT_TYPE_V1: &[u8] = b"org.iso.18013.5.1.mDL";
/// Exact ISO/IEC 18013-5 mDL namespace.
pub const VEGA_MDL_NAMESPACE_V1: &[u8] = b"org.iso.18013.5.1";

/// Consensus field whose duplicated binding did not match the public
/// statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VegaBindingFieldV1 {
    /// Chain identifier.
    ChainId,
    /// Action index.
    ActionIndex,
    /// Parameter-set identifier.
    ParameterId,
    /// Parameter-set digest.
    ParameterDigest,
    /// Verifier digest.
    VerifierDigest,
    /// Statement-schema digest.
    StatementSchemaDigest,
    /// Engine-manifest digest.
    EngineManifestDigest,
}

/// ECDSA role used by a Vega witness diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VegaSignatureRoleV1 {
    /// Credential issuer authentication.
    Issuer,
    /// Holder-device authentication.
    Device,
}

/// Failure returned by the closed Vega mDL engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum VegaMdlError {
    /// A duplicated consensus binding differs from the statement context.
    #[error("Vega consensus binding mismatches statement field {field:?}")]
    BindingMismatch {
        /// Mismatched field.
        field: VegaBindingFieldV1,
    },
    /// The chain id is empty or too large.
    #[error("Vega chain id length {actual} is outside 1..={max}")]
    InvalidChainIdLength {
        /// Actual byte length.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// A mandatory consensus digest is the all-zero sentinel.
    #[error("Vega consensus digest `{field}` must be non-zero")]
    ZeroConsensusDigest {
        /// Stable field label.
        field: &'static str,
    },
    /// A canonical frame label or value does not fit its length prefix.
    #[error("Vega device-authentication frame field is too large")]
    FrameFieldTooLarge,
    /// The statement's device digest is not the canonical Iroha binding hash.
    #[error("Vega device-authentication digest does not match the consensus frame")]
    DeviceAuthenticationDigestMismatch,
    /// A trusted block timestamp cannot be represented as an admitted UTC date.
    #[error("Vega trusted block timestamp is outside the admitted UTC range")]
    TrustedTimestampOutOfRange,
    /// The public date differs from the trusted block timestamp's UTC date.
    #[error("Vega public presentation date differs from trusted UTC block date")]
    TrustedPresentationDateMismatch,
    /// A public or private date is not a valid proleptic Gregorian date.
    #[error("Vega `{field}` is not a valid Gregorian date")]
    InvalidDate {
        /// Stable date field label.
        field: &'static str,
    },
    /// The public age threshold is outside the closed first-release range.
    #[error("Vega age threshold {actual} is outside {min}..={max}")]
    InvalidAgeThreshold {
        /// Supplied threshold.
        actual: u8,
        /// Inclusive minimum.
        min: u8,
        /// Inclusive maximum.
        max: u8,
    },
    /// An input exceeds the exact closed byte bound or is empty.
    #[error("Vega `{field}` length {actual} is outside {min}..={max}")]
    InvalidInputLength {
        /// Stable input label.
        field: &'static str,
        /// Actual byte length.
        actual: usize,
        /// Inclusive minimum.
        min: usize,
        /// Inclusive maximum.
        max: usize,
    },
    /// Deterministic CBOR parsing failed.
    #[error("Vega input is not strict deterministic CBOR")]
    InvalidCanonicalCbor,
    /// The COSE `Sig_structure` is not the exact first-release shape.
    #[error("Vega issuer COSE Sig_structure has an invalid shape")]
    InvalidIssuerSignatureStructure,
    /// The protected COSE header is not exactly `{1: -7}`.
    #[error("Vega protected COSE header is not the closed ES256 profile")]
    InvalidProtectedHeader,
    /// The authenticated COSE payload differs from the supplied MSO payload.
    #[error("Vega issuer COSE payload is not the supplied MSO payload")]
    IssuerPayloadMismatch,
    /// A Tag-24 byte-string wrapper is absent or malformed.
    #[error("Vega `{field}` is not a canonical Tag-24 encoded-CBOR byte string")]
    InvalidTag24Wrapper {
        /// Stable wrapped field label.
        field: &'static str,
    },
    /// A mandatory MSO or signed-item field is missing.
    #[error("Vega document field `{field}` is missing")]
    MissingDocumentField {
        /// Stable field label.
        field: &'static str,
    },
    /// A mandatory field has the wrong CBOR type or shape.
    #[error("Vega document field `{field}` has an invalid CBOR shape")]
    InvalidDocumentFieldShape {
        /// Stable field label.
        field: &'static str,
    },
    /// A mandatory field differs from the closed first-release value.
    #[error("Vega document field `{field}` is outside the closed profile")]
    InvalidDocumentFieldValue {
        /// Stable field label.
        field: &'static str,
    },
    /// A paper-required unique field prefix was absent or repeated.
    #[error("Vega MSO prefix `{field}` must occur exactly once")]
    NonUniqueFieldPrefix {
        /// Stable field label.
        field: &'static str,
    },
    /// A P-256 public key is malformed, non-canonical, off-curve, or identity.
    #[error("Vega `{field}` is not a valid P-256 public key")]
    InvalidP256PublicKey {
        /// Stable public-key label.
        field: &'static str,
    },
    /// An ECDSA signature component is zero, non-canonical, or outside P-256's
    /// scalar order.
    #[error("Vega {role:?} ES256 signature encoding is invalid")]
    InvalidSignatureEncoding {
        /// Signature role.
        role: VegaSignatureRoleV1,
    },
    /// Native ES256 verification failed during witness preflight.
    #[error("Vega {role:?} ES256 signature verification failed")]
    SignatureVerificationFailed {
        /// Signature role.
        role: VegaSignatureRoleV1,
    },
    /// The birth signed-item randomizer is outside the closed size bound.
    #[error("Vega birth-date randomizer length {actual} is outside 16..=64")]
    InvalidBirthRandomLength {
        /// Actual randomizer byte length.
        actual: usize,
    },
    /// The birth signed-item digest does not match its authenticated MSO entry.
    #[error("Vega birth-date signed-item digest mismatch")]
    BirthDateDigestMismatch,
    /// The credential is expired on the public presentation date.
    #[error("Vega credential validUntil date must be after presentation date")]
    CredentialExpired,
    /// The private date of birth follows the presentation date.
    #[error("Vega birth date follows the presentation date")]
    BirthDateAfterPresentation,
    /// The private date of birth does not satisfy the public age threshold.
    #[error("Vega completed Gregorian age is below the public threshold")]
    AgeThresholdNotMet,
    /// An address used by the lookup relation cannot be represented by the
    /// fixed `u32` witness format.
    #[error("Vega lookup address does not fit u32")]
    LookupAddressOverflow,
    /// A Figure 9 public input is not canonical in the T256 scalar field.
    #[error("Vega Figure 9 public input is not a canonical T256 scalar")]
    InvalidPublicInputScalar,
}

impl From<cbor::CborError> for VegaMdlError {
    fn from(_: cbor::CborError) -> Self {
        Self::InvalidCanonicalCbor
    }
}

impl From<VegaFieldError> for VegaMdlError {
    fn from(_: VegaFieldError) -> Self {
        Self::InvalidPublicInputScalar
    }
}

/// Duplicated consensus values required to bind a public Vega statement to the
/// active chain and governed artifacts.
#[derive(Clone, Copy, Debug)]
pub struct VegaMdlConsensusBindingV1<'a> {
    /// Exact chain-id bytes.
    pub chain_id: &'a [u8],
    /// Hash of the exact genesis block or genesis manifest.
    pub genesis_hash: [u8; 32],
    /// Zero-based privacy action index inside its transaction.
    pub action_index: u32,
    /// Exact governed parameter-set identifier.
    pub parameter_id: [u8; 32],
    /// Digest of the governed parameter set.
    pub parameter_digest: [u8; 32],
    /// Digest of the exact verifier artifact.
    pub verifier_digest: [u8; 32],
    /// Digest of the exact typed public-statement schema.
    pub statement_schema_digest: [u8; 32],
    /// Digest of the native engine manifest admitted by governance.
    pub engine_manifest_digest: [u8; 32],
}

impl<'a> VegaMdlConsensusBindingV1<'a> {
    /// Build a binding from a statement context plus the independently trusted
    /// genesis hash.
    #[must_use]
    pub fn from_context(context: &'a PrivacyStatementContextV1, genesis_hash: [u8; 32]) -> Self {
        Self {
            chain_id: context.chain_id.as_str().as_bytes(),
            genesis_hash,
            action_index: context.action_index,
            parameter_id: *context.parameter_id.as_bytes(),
            parameter_digest: *context.parameter_digest.as_bytes(),
            verifier_digest: *context.verifier_digest.as_bytes(),
            statement_schema_digest: *context.statement_schema_digest.as_bytes(),
            engine_manifest_digest: *context.engine_manifest_digest.as_bytes(),
        }
    }

    fn validate(&self, statement: &VegaExistingCredentialStatementV1) -> Result<(), VegaMdlError> {
        let max = usize::try_from(PRIVACY_MAX_CHAIN_ID_BYTES_V1)
            .expect("privacy chain-id bound fits usize");
        if self.chain_id.is_empty() || self.chain_id.len() > max {
            return Err(VegaMdlError::InvalidChainIdLength {
                actual: self.chain_id.len(),
                max,
            });
        }
        if self.chain_id != statement.context.chain_id.as_str().as_bytes() {
            return Err(VegaMdlError::BindingMismatch {
                field: VegaBindingFieldV1::ChainId,
            });
        }
        if self.action_index != statement.context.action_index {
            return Err(VegaMdlError::BindingMismatch {
                field: VegaBindingFieldV1::ActionIndex,
            });
        }
        for (field, supplied, expected) in [
            (
                VegaBindingFieldV1::ParameterId,
                self.parameter_id,
                *statement.context.parameter_id.as_bytes(),
            ),
            (
                VegaBindingFieldV1::ParameterDigest,
                self.parameter_digest,
                *statement.context.parameter_digest.as_bytes(),
            ),
            (
                VegaBindingFieldV1::VerifierDigest,
                self.verifier_digest,
                *statement.context.verifier_digest.as_bytes(),
            ),
            (
                VegaBindingFieldV1::StatementSchemaDigest,
                self.statement_schema_digest,
                *statement.context.statement_schema_digest.as_bytes(),
            ),
            (
                VegaBindingFieldV1::EngineManifestDigest,
                self.engine_manifest_digest,
                *statement.context.engine_manifest_digest.as_bytes(),
            ),
        ] {
            if supplied != expected {
                return Err(VegaMdlError::BindingMismatch { field });
            }
        }
        for (field, digest) in [
            ("genesis_hash", self.genesis_hash),
            ("parameter_id", self.parameter_id),
            ("parameter_digest", self.parameter_digest),
            ("verifier_digest", self.verifier_digest),
            ("statement_schema_digest", self.statement_schema_digest),
            ("engine_manifest_digest", self.engine_manifest_digest),
        ] {
            if digest == [0; 32] {
                return Err(VegaMdlError::ZeroConsensusDigest { field });
            }
        }
        Ok(())
    }
}

/// Exact ordered T256 scalar public inputs for the Figure 9 relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VegaMdlPublicInputsV1 {
    elements: [VegaT256ScalarV1; VEGA_MDL_PUBLIC_INPUT_COUNT_V1],
}

impl VegaMdlPublicInputsV1 {
    /// Translate a typed public statement without reducing any 256-bit value.
    ///
    /// The order is `Q_I.x`, `Q_I.y`, the eight big-endian 32-bit words of
    /// `H_dev`, `Y`, `M`, `D`, and `tau`.
    ///
    /// # Errors
    ///
    /// Returns [`VegaMdlError`] for an invalid issuer key, Gregorian date, or
    /// non-canonical coordinate.
    pub fn from_statement(
        statement: &VegaExistingCredentialStatementV1,
    ) -> Result<Self, VegaMdlError> {
        validate_public_statement(statement)?;
        let (issuer_x, issuer_y) = p256_affine_coordinates(statement.issuer_public_key)?;
        let mut elements = [VegaT256ScalarV1::from_u64(0); VEGA_MDL_PUBLIC_INPUT_COUNT_V1];
        elements[0] = VegaT256ScalarV1::from_be_bytes_exact(issuer_x)?;
        elements[1] = VegaT256ScalarV1::from_be_bytes_exact(issuer_y)?;
        for (index, word) in statement
            .device_authentication_digest
            .as_bytes()
            .chunks_exact(4)
            .enumerate()
        {
            let value = u32::from_be_bytes(
                word.try_into()
                    .expect("four-byte chunks have an exact fixed width"),
            );
            elements[index + 2] = VegaT256ScalarV1::from_u64(u64::from(value));
        }
        elements[10] = VegaT256ScalarV1::from_u64(u64::from(statement.presentation_date.year));
        elements[11] = VegaT256ScalarV1::from_u64(u64::from(statement.presentation_date.month));
        elements[12] = VegaT256ScalarV1::from_u64(u64::from(statement.presentation_date.day));
        elements[13] = VegaT256ScalarV1::from_u64(u64::from(statement.minimum_age_years));
        Ok(Self { elements })
    }

    /// Borrow the exact ordered public-input vector.
    #[must_use]
    pub const fn as_array(&self) -> &[VegaT256ScalarV1; VEGA_MDL_PUBLIC_INPUT_COUNT_V1] {
        &self.elements
    }
}

/// Construct the exact length-delimited device-authentication consensus frame.
///
/// # Errors
///
/// Returns [`VegaMdlError`] when a duplicated binding mismatches the statement
/// or a mandatory value is invalid.
pub fn device_authentication_frame_v1(
    statement: &VegaExistingCredentialStatementV1,
    binding: &VegaMdlConsensusBindingV1<'_>,
) -> Result<Vec<u8>, VegaMdlError> {
    binding.validate(statement)?;
    validate_public_statement(statement)?;

    let mut frame = Vec::with_capacity(768);
    append_frame_field(
        &mut frame,
        b"domain",
        VEGA_MDL_DEVICE_AUTHENTICATION_DOMAIN_V1,
    )?;
    append_frame_field(
        &mut frame,
        b"frame_version",
        &[VEGA_MDL_DEVICE_AUTHENTICATION_FRAME_VERSION_V1],
    )?;
    append_frame_field(&mut frame, b"upstream_commit", VEGA_PINNED_SOURCE_COMMIT_V1)?;
    append_frame_field(&mut frame, b"chain_id", binding.chain_id)?;
    append_frame_field(&mut frame, b"genesis_hash", &binding.genesis_hash)?;
    append_frame_field(
        &mut frame,
        b"action_index",
        &binding.action_index.to_be_bytes(),
    )?;
    append_frame_field(&mut frame, b"parameter_id", &binding.parameter_id)?;
    append_frame_field(&mut frame, b"parameter_digest", &binding.parameter_digest)?;
    append_frame_field(&mut frame, b"verifier_digest", &binding.verifier_digest)?;
    append_frame_field(
        &mut frame,
        b"statement_schema_digest",
        &binding.statement_schema_digest,
    )?;
    append_frame_field(
        &mut frame,
        b"engine_manifest_digest",
        &binding.engine_manifest_digest,
    )?;
    append_frame_field(&mut frame, b"document_type", VEGA_MDL_DOCUMENT_TYPE_V1)?;
    append_frame_field(&mut frame, b"namespace", VEGA_MDL_NAMESPACE_V1)?;
    append_frame_field(&mut frame, b"digest_algorithm", b"SHA-256")?;
    append_frame_field(&mut frame, b"issuer_authentication", b"COSE_Sign1/ES256")?;
    append_frame_field(&mut frame, b"device_authentication", b"COSE_Sign1/ES256")?;
    append_frame_field(
        &mut frame,
        b"issuer_public_key",
        statement.issuer_public_key.as_bytes(),
    )?;
    append_frame_field(
        &mut frame,
        b"presentation_year",
        &statement.presentation_date.year.to_be_bytes(),
    )?;
    append_frame_field(
        &mut frame,
        b"presentation_month",
        &[statement.presentation_date.month],
    )?;
    append_frame_field(
        &mut frame,
        b"presentation_day",
        &[statement.presentation_date.day],
    )?;
    append_frame_field(
        &mut frame,
        b"minimum_age_years",
        &[statement.minimum_age_years],
    )?;
    append_frame_field(
        &mut frame,
        b"reader_challenge",
        statement.reader_challenge.as_bytes(),
    )?;
    append_frame_field(
        &mut frame,
        b"session_transcript_digest",
        statement.session_transcript_digest.as_bytes(),
    )?;
    Ok(frame)
}

/// Derive `H_dev` from the exact Iroha consensus frame.
///
/// # Errors
///
/// Returns [`VegaMdlError`] when the statement or duplicated binding is
/// malformed.
pub fn derive_device_authentication_digest_v1(
    statement: &VegaExistingCredentialStatementV1,
    binding: &VegaMdlConsensusBindingV1<'_>,
) -> Result<PrivacyVegaDeviceAuthenticationDigestV1, VegaMdlError> {
    let frame = device_authentication_frame_v1(statement, binding)?;
    Ok(PrivacyVegaDeviceAuthenticationDigestV1::new(
        Sha256::digest(frame).into(),
    ))
}

/// Require the statement's public date to equal the trusted block timestamp's
/// UTC date.
///
/// # Errors
///
/// Returns [`VegaMdlError`] when the timestamp is outside the supported range
/// or the dates differ.
pub fn validate_trusted_presentation_date_v1(
    statement: &VegaExistingCredentialStatementV1,
    trusted_block_timestamp_ms: u64,
) -> Result<(), VegaMdlError> {
    validate_public_statement(statement)?;
    let unix_seconds = i64::try_from(trusted_block_timestamp_ms / 1_000)
        .map_err(|_| VegaMdlError::TrustedTimestampOutOfRange)?;
    let date = OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map_err(|_| VegaMdlError::TrustedTimestampOutOfRange)?
        .date();
    let trusted = PrivacyVegaMdlDateV1 {
        year: u16::try_from(date.year()).map_err(|_| VegaMdlError::TrustedTimestampOutOfRange)?,
        month: u8::from(date.month()),
        day: date.day(),
    };
    if trusted != statement.presentation_date {
        return Err(VegaMdlError::TrustedPresentationDateMismatch);
    }
    Ok(())
}

pub(super) fn validate_date(
    date: PrivacyVegaMdlDateV1,
    field: &'static str,
) -> Result<Date, VegaMdlError> {
    let month = Month::try_from(date.month).map_err(|_| VegaMdlError::InvalidDate { field })?;
    Date::from_calendar_date(i32::from(date.year), month, date.day)
        .map_err(|_| VegaMdlError::InvalidDate { field })
}

fn validate_public_statement(
    statement: &VegaExistingCredentialStatementV1,
) -> Result<(), VegaMdlError> {
    let _ = validate_date(statement.presentation_date, "presentation_date")?;
    if !(VEGA_MDL_MIN_PRESENTATION_YEAR_V1..=VEGA_MDL_MAX_PRESENTATION_YEAR_V1)
        .contains(&statement.presentation_date.year)
    {
        return Err(VegaMdlError::InvalidDate {
            field: "presentation_date",
        });
    }
    if !(VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1..=VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1)
        .contains(&statement.minimum_age_years)
    {
        return Err(VegaMdlError::InvalidAgeThreshold {
            actual: statement.minimum_age_years,
            min: VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1,
            max: VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1,
        });
    }
    for (field, digest) in [
        ("reader_challenge", statement.reader_challenge.as_bytes()),
        (
            "session_transcript_digest",
            statement.session_transcript_digest.as_bytes(),
        ),
    ] {
        if digest == &[0; 32] {
            return Err(VegaMdlError::ZeroConsensusDigest { field });
        }
    }
    let _ = p256_affine_coordinates(statement.issuer_public_key)?;
    Ok(())
}

fn p256_affine_coordinates(
    encoded: PrivacyP256PointV1,
) -> Result<([u8; 32], [u8; 32]), VegaMdlError> {
    let public_key = PublicKey::from_sec1_bytes(encoded.as_bytes()).map_err(|_| {
        VegaMdlError::InvalidP256PublicKey {
            field: "issuer_public_key",
        }
    })?;
    let uncompressed: EncodedPoint = public_key.to_encoded_point(false);
    let x = uncompressed.x().ok_or(VegaMdlError::InvalidP256PublicKey {
        field: "issuer_public_key",
    })?;
    let y = uncompressed.y().ok_or(VegaMdlError::InvalidP256PublicKey {
        field: "issuer_public_key",
    })?;
    let mut x_bytes = [0_u8; 32];
    let mut y_bytes = [0_u8; 32];
    x_bytes.copy_from_slice(x);
    y_bytes.copy_from_slice(y);
    Ok((x_bytes, y_bytes))
}

fn append_frame_field(frame: &mut Vec<u8>, label: &[u8], value: &[u8]) -> Result<(), VegaMdlError> {
    let label_len = u16::try_from(label.len()).map_err(|_| VegaMdlError::FrameFieldTooLarge)?;
    let value_len = u32::try_from(value.len()).map_err(|_| VegaMdlError::FrameFieldTooLarge)?;
    frame.extend_from_slice(&label_len.to_be_bytes());
    frame.extend_from_slice(label);
    frame.extend_from_slice(&value_len.to_be_bytes());
    frame.extend_from_slice(value);
    Ok(())
}
