//! Canonical wire types for externally signed SoraFS moderation recovery.
//!
//! Preparing a dead-letter resolution returns one opaque canonical Norito
//! frame and its signing message. The attestor signs that message outside
//! Torii, then submits the original frame and detached Ed25519 signature to
//! the apply route. No field-by-field or alternate binary representation is
//! part of the V1 contract.
//!
//! The DTO validators enforce the transport representation and size bounds.
//! The apply handler must additionally decode the embedded core resolution
//! with bounded Norito limits and require a byte-identical canonical re-encode.
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
/// Stable schema label returned by the dead-letter prepare route.
pub const SORAFS_MODERATION_DEAD_LETTER_PREPARE_RESPONSE_SCHEMA_V1: &str =
    "sorafs.moderation.dead_letter_resolution.prepare.v1";
/// Stable success status returned by the dead-letter prepare route.
pub const SORAFS_MODERATION_DEAD_LETTER_PREPARE_STATUS_V1: &str = "prepared";
/// Stable schema label returned by the dead-letter apply route.
pub const SORAFS_MODERATION_DEAD_LETTER_APPLY_RESPONSE_SCHEMA_V1: &str =
    "sorafs.moderation.dead_letter_resolution.apply.v1";
/// Stable success status returned by the dead-letter apply route.
pub const SORAFS_MODERATION_DEAD_LETTER_APPLY_STATUS_V1: &str = "applied";
/// Maximum canonical bytes in one prepared dead-letter resolution frame.
pub const SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_CANONICAL_BYTES_V1: usize = 4 * 1024;
/// Maximum padded-standard-base64 bytes representing one resolution frame.
pub const SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1: usize =
    SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_CANONICAL_BYTES_V1.div_ceil(3) * 4;
/// Maximum JSON bytes accepted by the dead-letter prepare route.
pub const SORAFS_MODERATION_DEAD_LETTER_PREPARE_REQUEST_MAX_BYTES_V1: usize = 1024;
/// Maximum JSON bytes accepted by the dead-letter apply route.
pub const SORAFS_MODERATION_DEAD_LETTER_APPLY_REQUEST_MAX_BYTES_V1: usize = 8 * 1024;
/// Maximum JSON bytes accepted from either successful dead-letter route response.
pub const SORAFS_MODERATION_DEAD_LETTER_JSON_RESPONSE_MAX_BYTES_V1: usize = 8 * 1024;
/// Closed dead-letter source selected for resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoDeserialize, NoritoSerialize)]
#[norito(schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.kind")]
pub enum SorafsModerationDeadLetterKindV1 {
    /// A native moderation submission exhausted durable delivery attempts.
    NativeSubmission,
    /// A terminal handoff exhausted durable delivery attempts.
    TerminalHandoff,
    /// A panel notification exhausted durable delivery attempts.
    PanelNotification,
}
impl norito::json::JsonSerialize for SorafsModerationDeadLetterKindV1 {
    fn json_serialize(&self, out: &mut String) {
        let value = match self {
            Self::NativeSubmission => "native_submission",
            Self::TerminalHandoff => "terminal_handoff",
            Self::PanelNotification => "panel_notification",
        };
        norito::json::write_json_string(value, out);
    }
}
impl norito::json::JsonDeserialize for SorafsModerationDeadLetterKindV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "native_submission" => Ok(Self::NativeSubmission),
            "terminal_handoff" => Ok(Self::TerminalHandoff),
            "panel_notification" => Ok(Self::PanelNotification),
            other => Err(norito::json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}
/// Closed disposition applied to one unresolved dead letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoDeserialize, NoritoSerialize)]
#[norito(schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.action")]
pub enum SorafsModerationDeadLetterResolutionActionV1 {
    /// Begin a fresh bounded delivery attempt cycle.
    Redrive,
    /// Seal the incident without scheduling another delivery attempt.
    Acknowledge,
}
impl norito::json::JsonSerialize for SorafsModerationDeadLetterResolutionActionV1 {
    fn json_serialize(&self, out: &mut String) {
        let value = match self {
            Self::Redrive => "redrive",
            Self::Acknowledge => "acknowledge",
        };
        norito::json::write_json_string(value, out);
    }
}
impl norito::json::JsonDeserialize for SorafsModerationDeadLetterResolutionActionV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "redrive" => Ok(Self::Redrive),
            "acknowledge" => Ok(Self::Acknowledge),
            other => Err(norito::json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}
/// Strict request for a checkpoint-bound dead-letter resolution statement.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(
    schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.prepare_request",
    deny_unknown_fields
)]
pub struct SorafsModerationDeadLetterPrepareRequestV1 {
    /// Exact non-zero dead-letter identity as 64 lowercase hexadecimal digits.
    pub identity_hex: String,
    /// Durable source family containing the unresolved dead letter.
    pub kind: SorafsModerationDeadLetterKindV1,
    /// Disposition the external attestor authorizes.
    pub action: SorafsModerationDeadLetterResolutionActionV1,
    /// Explicit authorization time in Unix milliseconds.
    pub authorized_at_unix_ms: u64,
}
impl SorafsModerationDeadLetterPrepareRequestV1 {
    /// Validate the canonical lexical and non-zero request bounds.
    ///
    /// # Errors
    ///
    /// Returns an error when the identity is not one non-zero lowercase
    /// 32-byte hexadecimal value or the authorization time is zero.
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex("identity_hex", &self.identity_hex, 32, true)?;
        if self.authorized_at_unix_ms == 0 {
            return Err("authorized_at_unix_ms must be non-zero".to_owned());
        }
        Ok(())
    }
}
/// Prepared statement that must be signed by the configured external attestor.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(
    schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.prepare_response",
    deny_unknown_fields
)]
pub struct SorafsModerationDeadLetterPrepareResponseV1 {
    /// Exact V1 response schema label.
    pub schema: String,
    /// Exact successful preparation status.
    pub status: String,
    /// Canonical padded-standard-base64 Norito resolution frame.
    pub resolution_norito_b64: String,
    /// Exact 32-byte signing message as lowercase hexadecimal.
    pub signing_message_hex: String,
}
impl SorafsModerationDeadLetterPrepareResponseV1 {
    /// Validate the complete successful prepare response contract.
    ///
    /// # Errors
    ///
    /// Returns an error for substituted schema/status labels, a non-canonical
    /// or oversized resolution frame, or a malformed signing-message digest.
    pub fn validate(&self) -> Result<(), String> {
        validate_exact(
            "schema",
            &self.schema,
            SORAFS_MODERATION_DEAD_LETTER_PREPARE_RESPONSE_SCHEMA_V1,
        )?;
        validate_exact(
            "status",
            &self.status,
            SORAFS_MODERATION_DEAD_LETTER_PREPARE_STATUS_V1,
        )?;
        validate_resolution_base64(&self.resolution_norito_b64)?;
        validate_lower_hex("signing_message_hex", &self.signing_message_hex, 32, false)
    }
}
/// Strict request applying one externally signed dead-letter resolution.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(
    schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.apply_request",
    deny_unknown_fields
)]
pub struct SorafsModerationDeadLetterApplyRequestV1 {
    /// Original canonical padded-standard-base64 Norito resolution frame.
    pub resolution_norito_b64: String,
    /// Detached 64-byte Ed25519 signature as lowercase hexadecimal.
    pub signature_hex: String,
}
impl SorafsModerationDeadLetterApplyRequestV1 {
    /// Validate the canonical frame and detached-signature representations.
    ///
    /// # Errors
    ///
    /// Returns an error when the resolution is empty, non-canonical, or over
    /// the V1 bound, or when the signature is not one non-zero lowercase
    /// 64-byte hexadecimal value.
    pub fn validate(&self) -> Result<(), String> {
        validate_resolution_base64(&self.resolution_norito_b64)?;
        validate_lower_hex("signature_hex", &self.signature_hex, 64, true)
    }
}
/// Successful result of atomically applying a signed dead-letter resolution.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(
    schema_name = "iroha.torii.v1.sorafs.moderation.dead_letter.apply_response",
    deny_unknown_fields
)]
pub struct SorafsModerationDeadLetterApplyResponseV1 {
    /// Exact V1 response schema label.
    pub schema: String,
    /// Exact successful application status.
    pub status: String,
    /// Resolved non-zero dead-letter identity as lowercase hexadecimal.
    pub identity_hex: String,
    /// Durable source family that was resolved.
    pub kind: SorafsModerationDeadLetterKindV1,
    /// Applied dead-letter disposition.
    pub action: SorafsModerationDeadLetterResolutionActionV1,
}
impl SorafsModerationDeadLetterApplyResponseV1 {
    /// Validate the complete successful apply response contract.
    ///
    /// # Errors
    ///
    /// Returns an error for substituted schema/status labels or a malformed
    /// dead-letter identity.
    pub fn validate(&self) -> Result<(), String> {
        validate_exact(
            "schema",
            &self.schema,
            SORAFS_MODERATION_DEAD_LETTER_APPLY_RESPONSE_SCHEMA_V1,
        )?;
        validate_exact(
            "status",
            &self.status,
            SORAFS_MODERATION_DEAD_LETTER_APPLY_STATUS_V1,
        )?;
        validate_lower_hex("identity_hex", &self.identity_hex, 32, true)
    }
}
fn validate_exact(field: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} must equal {expected:?}"))
    }
}
fn validate_lower_hex(
    field: &str,
    value: &str,
    byte_width: usize,
    require_nonzero: bool,
) -> Result<(), String> {
    let hex_width = byte_width
        .checked_mul(2)
        .expect("fixed hexadecimal width fits usize");
    if value.len() != hex_width
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || (require_nonzero && value.bytes().all(|byte| byte == b'0'))
    {
        let qualifier = if require_nonzero { " non-zero" } else { "" };
        return Err(format!(
            "{field} must be one{qualifier} lowercase {byte_width}-byte hexadecimal value"
        ));
    }
    Ok(())
}
fn validate_resolution_base64(value: &str) -> Result<(), String> {
    if value.len() > SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1 {
        return Err(format!(
            "resolution_norito_b64 must contain at most {SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1} bytes"
        ));
    }
    let decoded_bytes = canonical_standard_base64_decoded_len(value).ok_or_else(|| {
        "resolution_norito_b64 must use canonical padded standard base64".to_owned()
    })?;
    if decoded_bytes == 0
        || decoded_bytes > SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_CANONICAL_BYTES_V1
    {
        return Err(format!(
            "resolution_norito_b64 must decode to 1..={SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_CANONICAL_BYTES_V1} bytes"
        ));
    }
    Ok(())
}
fn canonical_standard_base64_decoded_len(value: &str) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    let padding = if bytes.ends_with(b"==") {
        2
    } else {
        usize::from(bytes.ends_with(b"="))
    };
    let symbol_count = bytes.len().checked_sub(padding)?;
    if symbol_count == 0
        || !bytes[..symbol_count]
            .iter()
            .copied()
            .all(|byte| base64_symbol_value(byte).is_some())
        || !bytes[symbol_count..].iter().all(|byte| *byte == b'=')
    {
        return None;
    }
    match padding {
        0 if symbol_count.is_multiple_of(4) => {}
        1 if symbol_count % 4 == 3
            && base64_symbol_value(bytes[symbol_count - 1])?.trailing_zeros() >= 2 => {}
        2 if symbol_count % 4 == 2
            && base64_symbol_value(bytes[symbol_count - 1])?.trailing_zeros() >= 4 => {}
        _ => return None,
    }
    bytes
        .len()
        .checked_div(4)?
        .checked_mul(3)?
        .checked_sub(padding)
}
const fn base64_symbol_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn prepare_request() -> SorafsModerationDeadLetterPrepareRequestV1 {
        SorafsModerationDeadLetterPrepareRequestV1 {
            identity_hex: "11".repeat(32),
            kind: SorafsModerationDeadLetterKindV1::NativeSubmission,
            action: SorafsModerationDeadLetterResolutionActionV1::Redrive,
            authorized_at_unix_ms: 1_725_000_000_123,
        }
    }
    fn prepare_response() -> SorafsModerationDeadLetterPrepareResponseV1 {
        SorafsModerationDeadLetterPrepareResponseV1 {
            schema: SORAFS_MODERATION_DEAD_LETTER_PREPARE_RESPONSE_SCHEMA_V1.to_owned(),
            status: SORAFS_MODERATION_DEAD_LETTER_PREPARE_STATUS_V1.to_owned(),
            resolution_norito_b64: "AQIDBA==".to_owned(),
            signing_message_hex: "22".repeat(32),
        }
    }
    fn apply_request() -> SorafsModerationDeadLetterApplyRequestV1 {
        SorafsModerationDeadLetterApplyRequestV1 {
            resolution_norito_b64: "AQIDBA==".to_owned(),
            signature_hex: "33".repeat(64),
        }
    }
    fn apply_response() -> SorafsModerationDeadLetterApplyResponseV1 {
        SorafsModerationDeadLetterApplyResponseV1 {
            schema: SORAFS_MODERATION_DEAD_LETTER_APPLY_RESPONSE_SCHEMA_V1.to_owned(),
            status: SORAFS_MODERATION_DEAD_LETTER_APPLY_STATUS_V1.to_owned(),
            identity_hex: "11".repeat(32),
            kind: SorafsModerationDeadLetterKindV1::PanelNotification,
            action: SorafsModerationDeadLetterResolutionActionV1::Acknowledge,
        }
    }
    #[test]
    fn json_contract_is_exact() {
        {
            let value = prepare_request();
            let json = norito::json::to_string(&value).expect("encode prepare request");
            assert_eq!(
                json,
                format!(
                    concat!(
                        r#"{{"identity_hex":"{}","kind":"native_submission","#,
                        r#""action":"redrive","authorized_at_unix_ms":1725000000123}}"#,
                    ),
                    "11".repeat(32)
                )
            );
            assert_eq!(
                norito::json::from_str::<SorafsModerationDeadLetterPrepareRequestV1>(&json)
                    .expect("decode prepare request"),
                value
            );
        }
        {
            let value = prepare_response();
            let json = norito::json::to_string(&value).expect("encode prepare response");
            assert_eq!(
                json,
                format!(
                    concat!(
                        r#"{{"schema":"{}","#,
                        r#""status":"prepared","resolution_norito_b64":"AQIDBA==","#,
                        r#""signing_message_hex":"{}"}}"#,
                    ),
                    SORAFS_MODERATION_DEAD_LETTER_PREPARE_RESPONSE_SCHEMA_V1,
                    "22".repeat(32)
                )
            );
            assert_eq!(
                norito::json::from_str::<SorafsModerationDeadLetterPrepareResponseV1>(&json)
                    .expect("decode prepare response"),
                value
            );
        }
        {
            let value = apply_request();
            let json = norito::json::to_string(&value).expect("encode apply request");
            assert_eq!(
                json,
                format!(
                    r#"{{"resolution_norito_b64":"AQIDBA==","signature_hex":"{}"}}"#,
                    "33".repeat(64)
                )
            );
            assert_eq!(
                norito::json::from_str::<SorafsModerationDeadLetterApplyRequestV1>(&json)
                    .expect("decode apply request"),
                value
            );
        }
        {
            let value = apply_response();
            let json = norito::json::to_string(&value).expect("encode apply response");
            assert_eq!(
                json,
                format!(
                    concat!(
                        r#"{{"schema":"{}","#,
                        r#""status":"applied","identity_hex":"{}","#,
                        r#""kind":"panel_notification","action":"acknowledge"}}"#,
                    ),
                    SORAFS_MODERATION_DEAD_LETTER_APPLY_RESPONSE_SCHEMA_V1,
                    "11".repeat(32)
                )
            );
            assert_eq!(
                norito::json::from_str::<SorafsModerationDeadLetterApplyResponseV1>(&json)
                    .expect("decode apply response"),
                value
            );
        }
    }
    #[test]
    fn closed_string_enums_reject_aliases() {
        for invalid in [
            r#""NativeSubmission""#,
            r#""native-submission""#,
            r#"{"kind":"native_submission","value":null}"#,
        ] {
            assert!(
                norito::json::from_str::<SorafsModerationDeadLetterKindV1>(invalid).is_err(),
                "kind alias must fail: {invalid}"
            );
        }
        for invalid in [r#""Redrive""#, r#""acknowledged""#] {
            assert!(
                norito::json::from_str::<SorafsModerationDeadLetterResolutionActionV1>(invalid)
                    .is_err(),
                "action alias must fail: {invalid}"
            );
        }
    }
    #[test]
    fn every_dto_rejects_unknown_json_fields() {
        let identity = "11".repeat(32);
        let signature = "33".repeat(64);
        for rejected in [
            norito::json::from_str::<SorafsModerationDeadLetterPrepareRequestV1>(&format!(
                r#"{{"identity_hex":"{identity}","kind":"native_submission","action":"redrive","authorized_at_unix_ms":1,"identity":"alias"}}"#
            ))
            .map(|_| ()),
            norito::json::from_str::<SorafsModerationDeadLetterPrepareResponseV1>(&format!(
                r#"{{"schema":"{SORAFS_MODERATION_DEAD_LETTER_PREPARE_RESPONSE_SCHEMA_V1}","status":"prepared","resolution_norito_b64":"AQIDBA==","signing_message_hex":"{}","payload_hex":"alias"}}"#,
                "22".repeat(32)
            ))
            .map(|_| ()),
            norito::json::from_str::<SorafsModerationDeadLetterApplyRequestV1>(&format!(
                r#"{{"resolution_norito_b64":"AQIDBA==","signature_hex":"{signature}","resolution_hex":"alias"}}"#
            ))
            .map(|_| ()),
            norito::json::from_str::<SorafsModerationDeadLetterApplyResponseV1>(&format!(
                r#"{{"schema":"{SORAFS_MODERATION_DEAD_LETTER_APPLY_RESPONSE_SCHEMA_V1}","status":"applied","identity_hex":"{identity}","kind":"panel_notification","action":"acknowledge","result":"alias"}}"#
            ))
            .map(|_| ()),
        ] {
            let error = rejected.expect_err("unknown DTO field must fail closed");
            assert_eq!(error.to_string(), "unknown JSON field");
        }
    }
    #[test]
    fn canonical_validation_rejects_aliases_and_out_of_bounds_values() {
        assert_eq!(
            SORAFS_MODERATION_DEAD_LETTER_RESOLUTION_MAX_BASE64_BYTES_V1,
            5_464
        );
        prepare_request().validate().expect("valid prepare request");
        prepare_response()
            .validate()
            .expect("valid prepare response");
        apply_request().validate().expect("valid apply request");
        apply_response().validate().expect("valid apply response");
        for identity in ["00".repeat(32), "AA".repeat(32), "11".repeat(31)] {
            let mut request = prepare_request();
            request.identity_hex = identity;
            assert!(request.validate().is_err());
        }
        let mut request = prepare_request();
        request.authorized_at_unix_ms = 0;
        assert!(request.validate().is_err());
        for resolution in [
            String::new(),
            "AQIDBA".to_owned(),
            "AR==".to_owned(),
            "AQID-A==".to_owned(),
            format!("{}=", "A".repeat(5_463)),
        ] {
            let mut request = apply_request();
            request.resolution_norito_b64 = resolution;
            assert!(request.validate().is_err());
        }
        for signature in ["00".repeat(64), "AA".repeat(64), "33".repeat(63)] {
            let mut request = apply_request();
            request.signature_hex = signature;
            assert!(request.validate().is_err());
        }
        let mut response = prepare_response();
        response.status = "queued".to_owned();
        assert!(response.validate().is_err());
        let mut response = apply_response();
        response.schema = "sorafs.moderation.dead_letter_resolution.v0".to_owned();
        assert!(response.validate().is_err());
    }
    #[test]
    fn every_dto_has_a_deterministic_norito_roundtrip() {
        {
            let value = prepare_request();
            let bytes = norito::to_bytes(&value).expect("encode prepare request");
            let decoded: SorafsModerationDeadLetterPrepareRequestV1 =
                norito::decode_from_bytes(&bytes).expect("decode prepare request");
            assert_eq!(decoded, value);
            assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), bytes);
        }
        {
            let value = prepare_response();
            let bytes = norito::to_bytes(&value).expect("encode prepare response");
            let decoded: SorafsModerationDeadLetterPrepareResponseV1 =
                norito::decode_from_bytes(&bytes).expect("decode prepare response");
            assert_eq!(decoded, value);
            assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), bytes);
        }
        {
            let value = apply_request();
            let bytes = norito::to_bytes(&value).expect("encode apply request");
            let decoded: SorafsModerationDeadLetterApplyRequestV1 =
                norito::decode_from_bytes(&bytes).expect("decode apply request");
            assert_eq!(decoded, value);
            assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), bytes);
        }
        {
            let value = apply_response();
            let bytes = norito::to_bytes(&value).expect("encode apply response");
            let decoded: SorafsModerationDeadLetterApplyResponseV1 =
                norito::decode_from_bytes(&bytes).expect("decode apply response");
            assert_eq!(decoded, value);
            assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), bytes);
        }
    }
}
