//! Utilities for Norito encoding and Axum integration.

use std::{
    any::{Any, TypeId},
    future::Future,
    sync::Arc,
};

use axum::{
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use iroha_data_model::{
    query::{SignedQuery, json_wrappers::SignedQueryJson},
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_torii_shared::ErrorEnvelope;
use iroha_version::Version;
use norito::{
    json::{self, JsonDeserializeOwned, JsonSerialize, Value},
    prelude::*,
};

/// MIME used in Torii for Norito encoding
// note: no elegant way to associate it with generic `NoritoBody<T>`
pub const NORITO_MIME_TYPE: &str = "application/x-norito";
const JSON_MIME_TYPE: &str = "application/json";
pub(crate) const MAX_ERROR_MESSAGE_CHARACTERS: usize = 1024;
pub(crate) const MAX_ERROR_DETAIL_CHARACTERS: usize = 1024;
pub(crate) const MAX_REJECT_CODE_BYTES: usize = 128;

/// Bounded stable error code copied into response extensions for telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpErrorCode(Arc<str>);

impl HttpErrorCode {
    /// Return the bounded metric-label value.
    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn new(code: &str) -> Self {
        if is_valid_error_code(code) {
            Self(Arc::from(code))
        } else {
            Self(Arc::from("invalid_error_code"))
        }
    }

    pub(crate) fn from_envelope(envelope: &ErrorEnvelope) -> Self {
        Self::new(envelope.code())
    }
}

/// Return whether a public error code is a bounded lower-snake-case identifier.
#[must_use]
pub(crate) fn is_valid_error_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 64
        && code.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (byte == b'_' && index > 0)
        })
}

/// Return whether a public error message is safe, exact human-readable text.
///
/// Messages are deliberately not stable identifiers, but the public envelope
/// still requires a non-empty value without surrounding whitespace or control
/// characters so every first-party SDK observes the same finite contract.
#[must_use]
pub(crate) fn is_valid_error_message(message: &str) -> bool {
    is_valid_bounded_public_text(message, MAX_ERROR_MESSAGE_CHARACTERS)
}

/// Return whether a public detail string is bounded exact text.
#[must_use]
pub(crate) fn is_valid_error_detail_text(value: &str) -> bool {
    is_valid_bounded_public_text(value, MAX_ERROR_DETAIL_CHARACTERS)
}

/// Return whether a protocol/domain rejection identifier is safe to expose.
#[must_use]
pub(crate) fn is_valid_reject_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= MAX_REJECT_CODE_BYTES
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn is_valid_bounded_public_text(value: &str, max_characters: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_characters.saturating_mul(4)
        && value.trim() == value
        && value.chars().take(max_characters + 1).count() <= max_characters
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod public_error_grammar_tests {
    use super::{MAX_ERROR_MESSAGE_CHARACTERS, is_valid_error_message, is_valid_reject_code};

    #[test]
    fn error_messages_require_exact_non_control_text() {
        for valid in [
            "invalid request",
            "再試行してください",
            "proof rejected: 42",
        ] {
            assert!(is_valid_error_message(valid), "valid message: {valid:?}");
        }
        for invalid in [
            "",
            " leading",
            "trailing ",
            "line\nbreak",
            "nul\0byte",
            "\u{85}",
        ] {
            assert!(
                !is_valid_error_message(invalid),
                "invalid message: {invalid:?}"
            );
        }
        assert!(!is_valid_error_message(
            &"x".repeat(MAX_ERROR_MESSAGE_CHARACTERS + 1)
        ));
        assert!(is_valid_error_message(
            &"界".repeat(MAX_ERROR_MESSAGE_CHARACTERS)
        ));
    }

    #[test]
    fn reject_codes_are_bounded_printable_identifiers() {
        for valid in ["queue_full", "PRTRY:BAD", "ISO-20022.CODE"] {
            assert!(is_valid_reject_code(valid), "valid code: {valid:?}");
        }
        for invalid in ["", " leading", "has/slash", "line\nbreak", "quote\""] {
            assert!(!is_valid_reject_code(invalid), "invalid code: {invalid:?}");
        }
        assert!(!is_valid_reject_code(&"x".repeat(129)));
    }
}

fn serialization_failure_response(format: ResponseFormat) -> Response {
    let envelope = ErrorEnvelope::new(
        "response_serialization_failed",
        "Torii could not encode the response payload.",
    );
    let (content_type, bytes) = match format {
        ResponseFormat::Norito => {
            let mut bytes = Vec::new();
            if let Err(error) = norito::core::to_bytes_in(&envelope, &mut bytes) {
                iroha_logger::error!(?error, "failed to encode fallback Norito error envelope");
                bytes.clear();
            }
            (NORITO_MIME_TYPE, bytes)
        }
        ResponseFormat::Json => {
            let bytes = norito::json::to_vec(&envelope).unwrap_or_else(|error| {
                iroha_logger::error!(?error, "failed to encode fallback JSON error envelope");
                Vec::new()
            });
            (JSON_MIME_TYPE, bytes)
        }
    };
    let mut response = Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(bytes))
        .expect("build response serialization error");
    response
        .extensions_mut()
        .insert(HttpErrorCode::from_envelope(&envelope));
    response
}

fn telemetry_error_code<T: Any>(value: &T) -> Option<HttpErrorCode> {
    let value: &dyn Any = value;
    value
        .downcast_ref::<ErrorEnvelope>()
        .map(HttpErrorCode::from_envelope)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MediaParameter {
    name: String,
    value: String,
    quoted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedMediaType {
    type_name: String,
    subtype: String,
    parameters: Vec<MediaParameter>,
}

impl ParsedMediaType {
    fn essence(&self) -> String {
        format!("{}/{}", self.type_name, self.subtype)
    }

    fn has_concrete_type(&self) -> bool {
        !self.type_name.contains('*') && !self.subtype.contains('*')
    }

    fn has_valid_range_wildcards(&self) -> bool {
        match (self.type_name.as_str(), self.subtype.as_str()) {
            ("*", "*") => true,
            ("*", _) => false,
            (type_name, "*") => !type_name.contains('*'),
            _ => self.has_concrete_type(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MediaParseError {
    InvalidSyntax,
    DuplicateParameter,
    InvalidQuality,
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn is_optional_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

fn is_quoted_text_byte(byte: u8) -> bool {
    matches!(byte, b'\t' | b' ' | b'!' | b'#'..=b'[' | b']'..=b'~')
}

fn is_quoted_pair_byte(byte: u8) -> bool {
    matches!(byte, b'\t' | b' '..=b'~')
}

fn trim_optional_whitespace(raw: &str) -> &str {
    raw.trim_matches(|character| matches!(character, ' ' | '\t'))
}

struct MediaCursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> MediaCursor<'a> {
    fn new(raw: &'a str) -> Result<Self, MediaParseError> {
        if !raw.is_ascii() {
            return Err(MediaParseError::InvalidSyntax);
        }
        Ok(Self {
            bytes: raw.as_bytes(),
            position: 0,
        })
    }

    fn at_end(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn current(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn skip_optional_whitespace(&mut self) {
        while self.current().is_some_and(is_optional_whitespace) {
            self.position += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> Result<(), MediaParseError> {
        if self.current() != Some(expected) {
            return Err(MediaParseError::InvalidSyntax);
        }
        self.position += 1;
        Ok(())
    }

    fn token(&mut self) -> Result<&'a str, MediaParseError> {
        let start = self.position;
        while self.current().is_some_and(is_http_token_byte) {
            self.position += 1;
        }
        if self.position == start {
            return Err(MediaParseError::InvalidSyntax);
        }
        // `MediaCursor::new` rejects non-ASCII input, so token boundaries are
        // always UTF-8 boundaries.
        Ok(std::str::from_utf8(&self.bytes[start..self.position])
            .expect("ASCII media token is valid UTF-8"))
    }

    fn parameter_value(&mut self) -> Result<(String, bool), MediaParseError> {
        if self.current() != Some(b'"') {
            return self.token().map(|value| (value.to_owned(), false));
        }

        self.position += 1;
        let mut decoded = String::new();
        loop {
            let byte = self.current().ok_or(MediaParseError::InvalidSyntax)?;
            match byte {
                b'"' => {
                    self.position += 1;
                    return Ok((decoded, true));
                }
                b'\\' => {
                    self.position += 1;
                    let escaped = self.current().ok_or(MediaParseError::InvalidSyntax)?;
                    if !is_quoted_pair_byte(escaped) {
                        return Err(MediaParseError::InvalidSyntax);
                    }
                    decoded.push(char::from(escaped));
                    self.position += 1;
                }
                value if is_quoted_text_byte(value) => {
                    decoded.push(char::from(value));
                    self.position += 1;
                }
                _ => return Err(MediaParseError::InvalidSyntax),
            }
        }
    }
}

fn parse_media_type(raw: &str) -> Result<ParsedMediaType, MediaParseError> {
    let mut cursor = MediaCursor::new(raw)?;
    cursor.skip_optional_whitespace();
    let type_name = cursor.token()?.to_ascii_lowercase();
    cursor.consume(b'/')?;
    let subtype = cursor.token()?.to_ascii_lowercase();

    let mut parameters: Vec<MediaParameter> = Vec::new();
    loop {
        cursor.skip_optional_whitespace();
        if cursor.at_end() {
            break;
        }
        cursor.consume(b';')?;
        cursor.skip_optional_whitespace();
        let name = cursor.token()?.to_ascii_lowercase();
        // Whitespace around `=` is not part of the HTTP parameter grammar.
        cursor.consume(b'=')?;
        let (value, quoted) = cursor.parameter_value()?;
        if parameters.iter().any(|parameter| parameter.name == name) {
            return Err(MediaParseError::DuplicateParameter);
        }
        parameters.push(MediaParameter {
            name,
            value,
            quoted,
        });
    }

    Ok(ParsedMediaType {
        type_name,
        subtype,
        parameters,
    })
}

fn is_json_subtype(subtype: &str) -> bool {
    subtype == "json"
        || subtype
            .strip_suffix("+json")
            .is_some_and(|prefix| !prefix.is_empty())
}

fn has_supported_json_charset(parameters: &[MediaParameter]) -> bool {
    parameters.iter().all(|parameter| {
        parameter.name != "charset" || parameter.value.eq_ignore_ascii_case("utf-8")
    })
}

fn is_norito_media_type(raw: &str) -> bool {
    parse_media_type(raw).is_ok_and(|media_type| {
        media_type.has_concrete_type()
            && media_type.type_name == "application"
            && media_type.subtype == "x-norito"
            && media_type
                .parameters
                .iter()
                .all(|parameter| parameter.name != "q")
    })
}

fn is_json_media_type(raw: &str) -> bool {
    parse_media_type(raw).is_ok_and(|media_type| {
        media_type.has_concrete_type()
            && media_type.type_name == "application"
            && is_json_subtype(&media_type.subtype)
            && has_supported_json_charset(&media_type.parameters)
            && media_type
                .parameters
                .iter()
                .all(|parameter| parameter.name != "q")
    })
}

/// Classify a response `Content-Type` as one of Torii's two typed representations.
///
/// Protocol-native media types such as SSE, Prometheus text, WebSocket upgrades,
/// raw blobs, hosted content, and concrete structured-suffix JSON types return
/// `None` and retain their exact media-type negotiation rules.
#[must_use]
pub fn typed_response_format_for_content_type(raw: &str) -> Option<ResponseFormat> {
    let media_type = parse_media_type(raw).ok()?;
    if !media_type.has_concrete_type()
        || media_type
            .parameters
            .iter()
            .any(|parameter| parameter.name == "q")
    {
        return None;
    }
    if media_type.type_name == "application"
        && media_type.subtype == "json"
        && has_supported_json_charset(&media_type.parameters)
    {
        return Some(ResponseFormat::Json);
    }
    if media_type.type_name == "application" && media_type.subtype == "x-norito" {
        return Some(ResponseFormat::Norito);
    }
    None
}

/// Preferred response encoding negotiated from the `Accept` header.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResponseFormat {
    /// Use Norito binary encoding.
    Norito,
    /// Use JSON encoding backed by the Norito JSON codec.
    Json,
}

tokio::task_local! {
    static CURRENT_RESPONSE_FORMAT: ResponseFormat;
}

/// Run a future with the response format negotiated for the current request.
pub async fn with_current_response_format<F>(format: ResponseFormat, future: F) -> F::Output
where
    F: Future,
{
    CURRENT_RESPONSE_FORMAT.scope(format, future).await
}

/// Return the response format associated with the current request.
///
/// Code paths outside an HTTP request use Norito, matching Torii's default wire preference.
pub fn current_response_format() -> ResponseFormat {
    CURRENT_RESPONSE_FORMAT
        .try_with(|format| *format)
        .unwrap_or(ResponseFormat::Norito)
}

/// Supported representation declared by a typed request body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypedRequestContentFormat {
    /// Norito-backed JSON encoded as UTF-8.
    Json,
    /// Native Norito binary encoding.
    Norito,
}

fn typed_request_media_rejection(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response {
    respond_with_status_and_format(
        status,
        ErrorEnvelope::new(code, message),
        current_response_format(),
    )
}

/// Validate and classify a typed request's `Content-Type` without reading its body.
///
/// The header must occur exactly once and be either `application/json` (with no
/// parameter or one `charset=utf-8`) or parameter-free `application/x-norito`.
/// This helper is shared by early admission middleware and body extractors so
/// unsupported media cannot reach idempotency handling or body collection through
/// a divergent second parsing path.
#[allow(clippy::result_large_err)]
pub(crate) fn typed_request_content_format(
    headers: &axum::http::HeaderMap,
) -> Result<TypedRequestContentFormat, Response> {
    let mut content_types = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = content_types.next() else {
        return Err(typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_missing",
            format!("missing Content-Type; use application/json or {NORITO_MIME_TYPE}"),
        ));
    };
    if content_types.next().is_some() {
        return Err(typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type must appear exactly once.",
        ));
    }
    let declared = value.to_str().map_err(|_| {
        typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type is not valid ASCII.",
        )
    })?;
    let declared = trim_optional_whitespace(declared);
    if declared.is_empty() {
        return Err(typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_missing",
            format!("missing Content-Type; use application/json or {NORITO_MIME_TYPE}"),
        ));
    }

    let media_type = parse_media_type(declared).map_err(|_| {
        typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type has invalid media-type syntax.",
        )
    })?;
    if !media_type.has_concrete_type() {
        return Err(typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type must declare one concrete media type.",
        ));
    }

    let request_format = if media_type.type_name == "application" {
        if media_type.subtype == "json"
            && match media_type.parameters.as_slice() {
                [] => true,
                [parameter] => {
                    parameter.name == "charset" && parameter.value.eq_ignore_ascii_case("utf-8")
                }
                _ => false,
            }
        {
            Some(TypedRequestContentFormat::Json)
        } else if media_type.subtype == "x-norito" && media_type.parameters.is_empty() {
            Some(TypedRequestContentFormat::Norito)
        } else {
            None
        }
    } else {
        None
    };

    request_format.ok_or_else(|| {
        typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_unsupported",
            format!(
                "unsupported Content-Type `{declared}`; use application/json or {NORITO_MIME_TYPE}"
            ),
        )
    })
}

/// Validate the canonical first-release JSON request representation.
///
/// Unlike the compatibility-oriented [`extractors::JsonOnly`] extractor, this
/// requires exactly one `Content-Type` header and accepts only
/// `application/json` with an optional UTF-8 charset. It deliberately rejects
/// structured-suffix JSON and native Norito so an endpoint cannot advertise a
/// narrower protocol than it actually enforces.
#[allow(clippy::result_large_err)]
pub(crate) fn canonical_json_request_content_type(
    headers: &axum::http::HeaderMap,
) -> Result<(), Response> {
    match typed_request_content_format(headers)? {
        TypedRequestContentFormat::Json => Ok(()),
        TypedRequestContentFormat::Norito => Err(typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_unsupported",
            "unsupported Content-Type `application/x-norito`; use application/json",
        )),
    }
}

/// Validate the first-release Kagemusha command media type.
///
/// Offline top-up and redemption accept one canonical Norito representation;
/// JSON is deliberately not a second request protocol.
#[allow(clippy::result_large_err)]
pub(crate) fn norito_request_content_type(headers: &axum::http::HeaderMap) -> Result<(), Response> {
    let mut content_types = headers.get_all(CONTENT_TYPE).iter();
    let Some(value) = content_types.next() else {
        return Err(typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_missing",
            format!("missing Content-Type; use {NORITO_MIME_TYPE}"),
        ));
    };
    if content_types.next().is_some() {
        return Err(typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type must appear exactly once.",
        ));
    }
    let declared = value.to_str().map_err(|_| {
        typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type is not valid ASCII.",
        )
    })?;
    let declared = trim_optional_whitespace(declared);
    if declared.is_empty() {
        return Err(typed_request_media_rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "request_content_type_missing",
            format!("missing Content-Type; use {NORITO_MIME_TYPE}"),
        ));
    }
    let media_type = parse_media_type(declared).map_err(|_| {
        typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type has invalid media-type syntax.",
        )
    })?;
    if !media_type.has_concrete_type() {
        return Err(typed_request_media_rejection(
            StatusCode::BAD_REQUEST,
            "request_content_type_invalid",
            "Content-Type must declare one concrete media type.",
        ));
    }
    if media_type.type_name == "application"
        && media_type.subtype == "x-norito"
        && media_type.parameters.is_empty()
    {
        return Ok(());
    }
    Err(typed_request_media_rejection(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "request_content_type_unsupported",
        format!("unsupported Content-Type `{declared}`; use {NORITO_MIME_TYPE}"),
    ))
}

fn not_acceptable(message: impl Into<String>) -> Response {
    // When no requested representation can be selected there is no negotiated
    // format to honor. The first-release contract uses a typed JSON envelope as
    // the deterministic fallback for the 406 itself.
    respond_with_status_and_format(
        StatusCode::NOT_ACCEPTABLE,
        iroha_torii_shared::ErrorEnvelope::new("response_not_acceptable", message),
        ResponseFormat::Json,
    )
}

fn parse_accept_qvalue(raw: &str) -> Option<u16> {
    let (whole, fraction) = raw.split_once('.').map_or((raw, ""), |parts| parts);
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let padded_fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u16>().ok()?.checked_mul(100)?,
        2 => fraction.parse::<u16>().ok()?.checked_mul(10)?,
        3 => fraction.parse::<u16>().ok()?,
        _ => return None,
    };
    match whole {
        "0" => Some(padded_fraction),
        "1" if padded_fraction == 0 => Some(1_000),
        _ => None,
    }
}

fn split_accept_list(raw: &str) -> Result<Vec<&str>, MediaParseError> {
    if !raw.is_ascii() {
        return Err(MediaParseError::InvalidSyntax);
    }

    let bytes = raw.as_bytes();
    let mut entries = Vec::new();
    let mut start = 0;
    let mut position = 0;
    let mut in_quotes = false;
    while position < bytes.len() {
        let byte = bytes[position];
        if in_quotes {
            match byte {
                b'"' => {
                    in_quotes = false;
                    position += 1;
                }
                b'\\' => {
                    position += 1;
                    let escaped = bytes
                        .get(position)
                        .copied()
                        .ok_or(MediaParseError::InvalidSyntax)?;
                    if !is_quoted_pair_byte(escaped) {
                        return Err(MediaParseError::InvalidSyntax);
                    }
                    position += 1;
                }
                value if is_quoted_text_byte(value) => position += 1,
                _ => return Err(MediaParseError::InvalidSyntax),
            }
            continue;
        }

        match byte {
            b'"' => {
                in_quotes = true;
                position += 1;
            }
            b',' => {
                let entry = trim_optional_whitespace(&raw[start..position]);
                if entry.is_empty() {
                    return Err(MediaParseError::InvalidSyntax);
                }
                entries.push(entry);
                position += 1;
                start = position;
            }
            b'\t' | b' '..=b'~' => position += 1,
            _ => return Err(MediaParseError::InvalidSyntax),
        }
    }
    if in_quotes {
        return Err(MediaParseError::InvalidSyntax);
    }
    let entry = trim_optional_whitespace(&raw[start..]);
    if entry.is_empty() {
        return Err(MediaParseError::InvalidSyntax);
    }
    entries.push(entry);
    Ok(entries)
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MediaSpecificity {
    type_level: u8,
    parameter_count: usize,
}

#[derive(Clone, Debug)]
struct ParsedAcceptRange {
    media_type: ParsedMediaType,
    quality: u16,
}

fn parse_accept_ranges(raw: &str) -> Result<Vec<ParsedAcceptRange>, MediaParseError> {
    split_accept_list(raw)?
        .into_iter()
        .map(|entry| {
            let mut media_type = parse_media_type(entry)?;
            if !media_type.has_valid_range_wildcards() {
                return Err(MediaParseError::InvalidSyntax);
            }

            let quality_index = media_type
                .parameters
                .iter()
                .position(|parameter| parameter.name == "q");
            let quality = quality_index.map_or(Ok(1_000), |index| {
                let parameter = &media_type.parameters[index];
                if parameter.quoted {
                    return Err(MediaParseError::InvalidQuality);
                }
                parse_accept_qvalue(&parameter.value).ok_or(MediaParseError::InvalidQuality)
            })?;
            // Parameters after `q` are Accept extensions. They are required to
            // be syntactically valid and unique, but do not constrain the
            // selected representation.
            if let Some(index) = quality_index {
                media_type.parameters.truncate(index);
            }
            Ok(ParsedAcceptRange {
                media_type,
                quality,
            })
        })
        .collect()
}

#[allow(clippy::result_large_err)]
fn parse_accept_header(header: &HeaderValue) -> Result<Vec<ParsedAcceptRange>, Response> {
    let raw = header.to_str().map_err(|_| {
        not_acceptable(
            "invalid Accept header encoding; supported: application/json, application/x-norito",
        )
    })?;
    parse_accept_ranges(raw).map_err(|error| {
        let message = match error {
            MediaParseError::InvalidQuality => "invalid q-value in Accept header",
            MediaParseError::DuplicateParameter => "duplicate parameter in Accept header",
            MediaParseError::InvalidSyntax => "malformed Accept header",
        };
        not_acceptable(message)
    })
}

fn typed_range_specificity(
    media_type: &ParsedMediaType,
    format: ResponseFormat,
) -> Option<MediaSpecificity> {
    let type_level = match (media_type.type_name.as_str(), media_type.subtype.as_str()) {
        ("*", "*") => 0,
        ("application", "*") => 1,
        ("application", subtype)
            if match format {
                // Torii emits the typed JSON representation as
                // `application/json`. A concrete structured-suffix type is a
                // distinct representation, not an alias for it.
                ResponseFormat::Json => subtype == "json",
                ResponseFormat::Norito => subtype == "x-norito",
            } =>
        {
            2
        }
        _ => return None,
    };

    let parameters_match = media_type.parameters.iter().all(|parameter| match format {
        ResponseFormat::Json => {
            parameter.name == "charset" && parameter.value.eq_ignore_ascii_case("utf-8")
        }
        ResponseFormat::Norito => false,
    });
    parameters_match.then_some(MediaSpecificity {
        type_level,
        parameter_count: media_type.parameters.len(),
    })
}

fn parameter_values_match(name: &str, left: &str, right: &str) -> bool {
    if name == "charset" {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn native_range_specificity(
    media_type: &ParsedMediaType,
    actual: &ParsedMediaType,
) -> Option<MediaSpecificity> {
    let type_level = if media_type.type_name == "*" && media_type.subtype == "*" {
        0
    } else if media_type.type_name == actual.type_name && media_type.subtype == "*" {
        1
    } else if media_type.type_name == actual.type_name && media_type.subtype == actual.subtype {
        2
    } else {
        return None;
    };

    let parameters_match = media_type.parameters.iter().all(|expected| {
        actual.parameters.iter().any(|candidate| {
            candidate.name == expected.name
                && parameter_values_match(&expected.name, &expected.value, &candidate.value)
        })
    });
    parameters_match.then_some(MediaSpecificity {
        type_level,
        parameter_count: media_type.parameters.len(),
    })
}

/// Negotiate the response format from an optional `Accept` header value.
///
/// Returns an HTTP response carrying status `406 Not Acceptable` when the header
/// explicitly forbids both JSON and Norito or contains an invalid q-value.
#[allow(clippy::result_large_err)] // callers expect to bubble the full HTTP response on negotiation failure
pub fn negotiate_response_format(accept: Option<&HeaderValue>) -> Result<ResponseFormat, Response> {
    negotiate_response_format_with_default(accept, ResponseFormat::Norito)
}

#[allow(clippy::result_large_err)] // callers expect to bubble the full HTTP response on negotiation failure
fn negotiate_response_format_with_default(
    accept: Option<&HeaderValue>,
    default_format: ResponseFormat,
) -> Result<ResponseFormat, Response> {
    let Some(header) = accept else {
        return Ok(default_format);
    };

    let parsed_ranges = parse_accept_header(header)?;

    #[derive(Copy, Clone, Debug)]
    struct MediaRange {
        quality: u16,
        specificity: MediaSpecificity,
        index: usize,
        matches_json: bool,
        matches_norito: bool,
    }

    let mut ranges = Vec::new();

    for (index, range) in parsed_ranges.iter().enumerate() {
        let json_specificity = typed_range_specificity(&range.media_type, ResponseFormat::Json);
        let norito_specificity = typed_range_specificity(&range.media_type, ResponseFormat::Norito);
        if let Some(specificity) = json_specificity {
            ranges.push(MediaRange {
                quality: range.quality,
                specificity,
                index,
                matches_json: true,
                matches_norito: false,
            });
        }
        if let Some(specificity) = norito_specificity {
            ranges.push(MediaRange {
                quality: range.quality,
                specificity,
                index,
                matches_json: false,
                matches_norito: true,
            });
        }
    }

    #[derive(Copy, Clone)]
    struct EffectivePreference {
        format: ResponseFormat,
        quality: u16,
        specificity: MediaSpecificity,
    }

    let effective = |format: ResponseFormat| {
        ranges
            .iter()
            .filter(|range| match format {
                ResponseFormat::Json => range.matches_json,
                ResponseFormat::Norito => range.matches_norito,
            })
            // RFC-style precedence: the most-specific media range determines
            // the quality for a representation. This is important for an
            // explicit `q=0`, which must not be undone by a wildcard.
            .max_by_key(|range| (range.specificity, core::cmp::Reverse(range.index)))
            .map(|range| EffectivePreference {
                format,
                quality: range.quality,
                specificity: range.specificity,
            })
    };

    let json = effective(ResponseFormat::Json).filter(|candidate| candidate.quality > 0);
    let norito = effective(ResponseFormat::Norito).filter(|candidate| candidate.quality > 0);
    let selected = match (json, norito) {
        (Some(json), Some(norito)) => {
            if json.quality > norito.quality {
                json
            } else if norito.quality > json.quality {
                norito
            } else if json.specificity > norito.specificity {
                json
            } else if norito.specificity > json.specificity {
                norito
            } else if json.specificity.type_level == 2 {
                // Equal explicit preferences use Torii's binary-first tie
                // break. Wildcard-only ties retain the endpoint's default.
                norito
            } else if default_format == ResponseFormat::Json {
                json
            } else {
                norito
            }
        }
        (Some(candidate), None) | (None, Some(candidate)) => candidate,
        (None, None) => {
            return Err(not_acceptable(
                "unsupported Accept header; use application/json or application/x-norito",
            ));
        }
    };

    if selected.quality == 0 {
        return Err(not_acceptable(
            "unsupported Accept header; use application/json or application/x-norito",
        ));
    }
    Ok(selected.format)
}

/// Validate `Accept` for JSON-only dynamic endpoints.
///
/// Dynamic `norito::json::Value` payloads do not have a stable Norito schema,
/// so JSON-only routes accept omitted, wildcard, and JSON `Accept` values, but
/// reject clients that explicitly ask for Norito without accepting JSON.
#[allow(clippy::result_large_err)] // callers bubble the full HTTP response on negotiation failure
pub fn negotiate_json_only_response(accept: Option<&HeaderValue>) -> Result<(), Response> {
    negotiate_single_typed_response(accept, ResponseFormat::Json)
}

/// Validate `Accept` for a response that is available only as Norito.
///
/// The most-specific matching range wins, so an explicit `application/x-norito;q=0`
/// cannot be overridden by a positive wildcard.
#[allow(clippy::result_large_err)] // callers bubble the full HTTP response on negotiation failure
pub fn negotiate_norito_only_response(accept: Option<&HeaderValue>) -> Result<(), Response> {
    negotiate_single_typed_response(accept, ResponseFormat::Norito)
}

/// Validate whether `format` is acceptable without selecting between representations.
///
/// This is used after safe requests run so protocol-native handlers can advertise their
/// actual media type instead of being rejected by the JSON/Norito negotiation layer.
#[allow(clippy::result_large_err)] // callers bubble the full HTTP response on negotiation failure
pub fn ensure_typed_response_format_acceptable(
    accept: Option<&HeaderValue>,
    format: ResponseFormat,
) -> Result<(), Response> {
    negotiate_single_typed_response(accept, format)
}

/// Validate a protocol-native response media type against `Accept`.
///
/// The handler remains responsible for selecting its actual SSE, metrics,
/// artifact, image, or other native representation. This check applies the
/// client's exact media ranges after a safe handler has produced that response.
#[allow(clippy::result_large_err)]
pub fn ensure_response_media_type_acceptable(
    accept: Option<&HeaderValue>,
    actual_content_type: &str,
) -> Result<(), Response> {
    let actual = parse_media_type(actual_content_type)
        .map_err(|_| not_acceptable("response Content-Type is invalid and cannot be negotiated"))?;
    if !actual.has_concrete_type()
        || actual
            .parameters
            .iter()
            .any(|parameter| parameter.name == "q")
        || (actual.type_name == "application"
            && is_json_subtype(&actual.subtype)
            && !has_supported_json_charset(&actual.parameters))
    {
        return Err(not_acceptable(
            "response Content-Type is invalid and cannot be negotiated",
        ));
    }
    let Some(header) = accept else {
        return Ok(());
    };
    let parsed_ranges = parse_accept_header(header)?;
    let mut effective: Option<(MediaSpecificity, usize, u16)> = None;
    for (index, range) in parsed_ranges.iter().enumerate() {
        let Some(specificity) = native_range_specificity(&range.media_type, &actual) else {
            continue;
        };
        if effective.is_none_or(|(current_specificity, current_index, _)| {
            specificity > current_specificity
                || (specificity == current_specificity && index < current_index)
        }) {
            effective = Some((specificity, index, range.quality));
        }
    }
    if effective.is_some_and(|(_, _, quality)| quality > 0) {
        return Ok(());
    }
    Err(not_acceptable(format!(
        "requested content type is not acceptable for this endpoint; response uses {}",
        actual.essence()
    )))
}

#[allow(clippy::result_large_err)] // callers bubble the full HTTP response on negotiation failure
fn negotiate_single_typed_response(
    accept: Option<&HeaderValue>,
    format: ResponseFormat,
) -> Result<(), Response> {
    let Some(header) = accept else {
        return Ok(());
    };

    let supported = match format {
        ResponseFormat::Json => JSON_MIME_TYPE,
        ResponseFormat::Norito => NORITO_MIME_TYPE,
    };
    let parsed_ranges = parse_accept_header(header)?;

    let mut effective: Option<(MediaSpecificity, usize, u16)> = None;
    for (index, range) in parsed_ranges.iter().enumerate() {
        let Some(specificity) = typed_range_specificity(&range.media_type, format) else {
            continue;
        };
        if effective.is_none_or(|(current_specificity, current_index, _)| {
            specificity > current_specificity
                || (specificity == current_specificity && index < current_index)
        }) {
            effective = Some((specificity, index, range.quality));
        }
    }

    if effective.is_some_and(|(_, _, quality)| quality > 0) {
        return Ok(());
    }

    Err(not_acceptable(format!(
        "requested content type is not acceptable for this endpoint; supported: {supported}"
    )))
}

/// Encode a response payload using the negotiated format.
pub fn respond_with_format<T>(value: T, format: ResponseFormat) -> Response
where
    T: JsonSerialize + norito::core::NoritoSerialize + 'static,
{
    respond_with_status_and_format(StatusCode::OK, value, format)
}

/// Encode a response payload using the given HTTP status and negotiated format.
pub fn respond_with_status_and_format<T>(
    status: StatusCode,
    value: T,
    format: ResponseFormat,
) -> Response
where
    T: JsonSerialize + norito::core::NoritoSerialize + 'static,
{
    let error_code = telemetry_error_code(&value);
    let mut response = match format {
        ResponseFormat::Norito => {
            let mut bytes = Vec::new();
            match norito::core::to_bytes_in(&value, &mut bytes) {
                Ok(()) => Response::builder()
                    .status(status)
                    .header(CONTENT_TYPE, HeaderValue::from_static(NORITO_MIME_TYPE))
                    .body(axum::body::Body::from(bytes))
                    .expect("build Norito response"),
                Err(err) => {
                    iroha_logger::error!(?err, "failed to serialise response payload");
                    serialization_failure_response(ResponseFormat::Norito)
                }
            }
        }
        ResponseFormat::Json => match norito::json::to_vec(&value) {
            Ok(bytes) => Response::builder()
                .status(status)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(axum::body::Body::from(bytes))
                .expect("build JSON response"),
            Err(err) => {
                iroha_logger::error!(?err, "failed to serialise response payload");
                serialization_failure_response(ResponseFormat::Json)
            }
        },
    };
    if let Some(error_code) = error_code {
        response.extensions_mut().insert(error_code);
    }
    response
}

/// Encode a dynamically constructed Norito JSON value as JSON.
pub fn respond_json_value(value: Value) -> Response {
    respond_json_value_with_status(StatusCode::OK, value)
}

/// Encode a dynamically constructed Norito JSON value as JSON with an HTTP status.
pub fn respond_json_value_with_status(status: StatusCode, value: Value) -> Response {
    match norito::json::to_vec(&value) {
        Ok(bytes) => Response::builder()
            .status(status)
            .header(CONTENT_TYPE, HeaderValue::from_static(JSON_MIME_TYPE))
            .body(axum::body::Body::from(bytes))
            .expect("build JSON response"),
        Err(err) => {
            iroha_logger::error!(?err, "failed to serialise response payload");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialise response",
            )
                .into_response()
        }
    }
}

/// Encode a dynamically constructed JSON document using the negotiated response format.
///
/// Dynamic values do not carry a stable Norito object schema. For Norito responses, the
/// JSON document is encoded as a Norito string so clients still receive a checksummed
/// Norito envelope without relying on a dynamic schema.
pub fn respond_json_document_with_status_and_format(
    status: StatusCode,
    value: Value,
    format: ResponseFormat,
) -> Response {
    match format {
        ResponseFormat::Json => respond_json_value_with_status(status, value),
        ResponseFormat::Norito => match norito::json::to_string(&value) {
            Ok(json) => respond_with_status_and_format(status, json, ResponseFormat::Norito),
            Err(err) => {
                iroha_logger::error!(?err, "failed to serialise response payload");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to serialise response",
                )
                    .into_response()
            }
        },
    }
}

/// Encode a dynamically constructed Norito JSON value.
///
/// Dynamic values do not carry a stable Norito schema, so they remain JSON-only.
/// Use [`respond_with_format`] for typed DTOs that support both JSON and Norito.
pub fn respond_value_with_format(value: Value, _format: ResponseFormat) -> Response {
    respond_json_value(value)
}

#[cfg(test)]
mod response_format_tests {
    use http_body_util::BodyExt as _;

    use super::*;

    #[derive(
        Clone,
        Debug,
        PartialEq,
        norito::derive::NoritoSerialize,
        norito::derive::NoritoDeserialize,
        crate::json_macros::JsonSerialize,
        crate::json_macros::JsonDeserialize,
    )]
    struct DummyPayload {
        value: u32,
    }

    #[tokio::test]
    async fn respond_with_format_produces_norito_bytes() {
        let payload = DummyPayload { value: 42 };
        let (parts, body) =
            respond_with_format(payload.clone(), ResponseFormat::Norito).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(NORITO_MIME_TYPE))
        );
        let bytes = body
            .collect()
            .await
            .expect("collect Norito body")
            .to_bytes();
        let decoded: DummyPayload = norito::decode_from_bytes(&bytes).expect("decode Norito body");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn respond_with_format_produces_json() {
        let payload = DummyPayload { value: 7 };
        let (parts, body) = respond_with_format(payload.clone(), ResponseFormat::Json).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/json"))
        );
        let bytes = body.collect().await.expect("collect JSON body").to_bytes();
        let decoded: DummyPayload = norito::json::from_slice(&bytes).expect("decode JSON body");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn typed_error_response_carries_bounded_telemetry_code() {
        let response = respond_with_status_and_format(
            StatusCode::CONFLICT,
            ErrorEnvelope::new("idempotency_key_conflict", "conflict"),
            ResponseFormat::Json,
        );
        assert_eq!(
            response
                .extensions()
                .get::<HttpErrorCode>()
                .map(HttpErrorCode::as_str),
            Some("idempotency_key_conflict")
        );

        let response = respond_with_status_and_format(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("raw/value/from/request", "invalid"),
            ResponseFormat::Norito,
        );
        assert_eq!(
            response
                .extensions()
                .get::<HttpErrorCode>()
                .map(HttpErrorCode::as_str),
            Some("invalid_error_code")
        );
    }

    #[tokio::test]
    async fn unacceptable_representation_uses_typed_json_fallback() {
        let header = HeaderValue::from_static("image/png");
        let response = negotiate_response_format(Some(&header))
            .expect_err("unsupported representation must be rejected");
        let (parts, body) = response.into_parts();
        assert_eq!(parts.status, StatusCode::NOT_ACCEPTABLE);
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(JSON_MIME_TYPE))
        );
        let bytes = body.collect().await.expect("collect error body").to_bytes();
        let envelope: iroha_torii_shared::ErrorEnvelope =
            norito::json::from_slice(&bytes).expect("decode typed error envelope");
        assert_eq!(envelope.code(), "response_not_acceptable");
    }

    #[tokio::test]
    async fn respond_value_with_format_keeps_dynamic_payloads_json_only() {
        let value = json::Value::from(7_u64);
        let (parts, body) = respond_value_with_format(value, ResponseFormat::Norito).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(JSON_MIME_TYPE))
        );
        let bytes = body.collect().await.expect("collect JSON body").to_bytes();
        let parsed: json::Value = json::from_slice(&bytes).expect("decode JSON payload");
        assert_eq!(parsed, json::Value::from(7_u64));
    }

    #[tokio::test]
    async fn respond_json_document_with_format_wraps_json_string_as_norito() {
        let mut object = json::Map::new();
        object.insert("ok".to_owned(), json::Value::Bool(true));
        let (parts, body) = respond_json_document_with_status_and_format(
            StatusCode::ACCEPTED,
            json::Value::Object(object),
            ResponseFormat::Norito,
        )
        .into_parts();

        assert_eq!(parts.status, StatusCode::ACCEPTED);
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(NORITO_MIME_TYPE))
        );
        let bytes = body
            .collect()
            .await
            .expect("collect Norito body")
            .to_bytes();
        let json: String = norito::decode_from_bytes(&bytes).expect("decode Norito JSON string");
        let decoded: json::Value = json::from_str(&json).expect("decode JSON document");
        assert_eq!(decoded["ok"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn respond_json_document_with_format_renders_json() {
        let mut object = json::Map::new();
        object.insert("ok".to_owned(), json::Value::Bool(true));
        let (parts, body) = respond_json_document_with_status_and_format(
            StatusCode::ACCEPTED,
            json::Value::Object(object),
            ResponseFormat::Json,
        )
        .into_parts();

        assert_eq!(parts.status, StatusCode::ACCEPTED);
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(JSON_MIME_TYPE))
        );
        let bytes = body.collect().await.expect("collect JSON body").to_bytes();
        let decoded: json::Value = json::from_slice(&bytes).expect("decode JSON document");
        assert_eq!(decoded["ok"].as_bool(), Some(true));
    }
}

/// Structure to reply using Norito encoding
#[derive(Debug)]
pub struct NoritoBody<T>(pub T);

impl<T: NoritoSerialize + Send + 'static> IntoResponse for NoritoBody<T> {
    fn into_response(self) -> Response {
        let error_code = telemetry_error_code(&self.0);
        // Encode with Norito header + checksum so clients can reliably decode.
        let mut buf = Vec::new();
        norito::core::to_bytes_in(&self.0, &mut buf).expect("norito serialization failed");
        let mut res = Response::new(buf.into());
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(NORITO_MIME_TYPE));
        if let Some(error_code) = error_code {
            res.extensions_mut().insert(error_code);
        }
        res
    }
}

/// Structure to reply with a dynamically constructed JSON value.
///
/// Prefer typed DTOs plus [`NoritoBody`] or [`JsonBody`] for dual-format routes.
#[derive(Debug)]
pub struct JsonValueBody(pub Value);

impl IntoResponse for JsonValueBody {
    fn into_response(self) -> Response {
        respond_json_value(self.0)
    }
}

/// Structure to reply using Norito-backed JSON encoding.
#[derive(Debug)]
pub struct JsonBody<T>(pub T);

impl<T: JsonSerialize + Send + 'static> IntoResponse for JsonBody<T> {
    fn into_response(self) -> Response {
        let error_code = telemetry_error_code(&self.0);
        // Serialize using Norito's JSON codec and attach the appropriate MIME type header.
        let buf = norito::json::to_vec(&self.0).expect("json serialization failed");
        let mut res = Response::new(buf.into());
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(error_code) = error_code {
            res.extensions_mut().insert(error_code);
        }
        res
    }
}

pub mod extractors {
    use axum::{
        body::Bytes,
        extract::{FromRequest, FromRequestParts, OptionalFromRequestParts, Request},
        http::StatusCode,
    };
    use norito::{
        core::NoritoDeserialize,
        json::{self, JsonDeserializeOwned, Number, Value},
    };
    use urlencoding::decode;

    use super::*;

    fn typed_request_rejection(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
    ) -> Response {
        super::respond_with_status_and_format(
            status,
            iroha_torii_shared::ErrorEnvelope::new(code, message),
            super::current_response_format(),
        )
    }

    fn typed_body_rejection(rejection: axum::extract::rejection::BytesRejection) -> Response {
        let status = rejection.into_response().status();
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            return typed_request_rejection(
                status,
                "request_payload_too_large",
                "The request body exceeds the configured size limit.",
            );
        }

        typed_request_rejection(
            status,
            "request_body_unreadable",
            "The request body could not be read.",
        )
    }

    /// Extractor of Norito-encoded, versioned data from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`;
    /// decode failures surface as `400 Bad Request` to distinguish payload issues from negotiation.
    #[derive(Clone, Copy, Debug)]
    pub struct Norito<T>(pub T);

    impl<S, T> FromRequest<S> for Norito<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: SupportsNoritoDecode + Send + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;

            decode_as_norito::<T>(&body).map(Norito)
        }
    }

    /// Extractor of Norito-encoded, versioned data from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`;
    /// decode failures surface as `400 Bad Request` to distinguish payload issues from negotiation.
    #[derive(Clone, Copy, Debug)]
    pub struct NoritoVersioned<T>(pub T);

    /// Extractor of raw Norito bytes from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`.
    /// Callers that need exact payload bytes can decode the returned buffer with
    /// the schema-specific Norito decoder.
    #[derive(Clone, Debug)]
    pub struct NoritoBytes(pub Bytes);

    impl<S> FromRequest<S> for NoritoBytes
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            Bytes::from_request(req, state)
                .await
                .map(NoritoBytes)
                .map_err(typed_body_rejection)
        }
    }

    impl<S, T> FromRequest<S> for NoritoVersioned<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        // Accept payloads encoded with Norito + iroha_version leading byte
        T: iroha_version::codec::DecodeVersioned,
        T: 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;

            match <T as iroha_version::codec::DecodeVersioned>::decode_all_versioned(&body) {
                Ok(val) => Ok(NoritoVersioned(val)),
                Err(versioned_err) => Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Could not decode versioned request: {versioned_err}"),
                )
                    .into_response()),
            }
        }
    }

    fn version_from_json_value(value: &Value) -> Option<u8> {
        match value {
            Value::Number(number) => u8::try_from(number.as_u64()?).ok(),
            Value::String(raw) => raw.parse::<u8>().ok(),
            _ => None,
        }
    }

    #[allow(clippy::result_large_err)]
    fn decode_versioned_json<T>(body: &Bytes, expected: &'static str) -> Result<T, Response>
    where
        T: JsonDeserializeOwned + Version,
    {
        let value = json::from_slice::<Value>(body.as_ref()).map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("invalid JSON body: {e}")).into_response()
        })?;
        let object = value.as_object().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid JSON body: expected versioned {expected} object"),
            )
                .into_response()
        })?;
        let version = object
            .get("version")
            .and_then(version_from_json_value)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "invalid JSON body: missing numeric `version` field",
                )
                    .into_response()
            })?;
        if !T::supported_versions().contains(&version) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unsupported JSON payload version `{version}` for {expected}"),
            )
                .into_response());
        }
        let content = object.get("content").ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "invalid JSON body: missing `content` field",
            )
                .into_response()
        })?;
        json::from_value::<T>(content.clone()).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid JSON {expected} content: {e}"),
            )
                .into_response()
        })
    }

    /// Helper trait exposing versioned JSON decoding for selected Torii ingress payloads.
    pub trait SupportsVersionedJsonDecode: Sized {
        /// Decode a JSON request body carrying a versioned payload.
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response>;
    }

    impl SupportsVersionedJsonDecode for SignedTransaction {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            decode_versioned_json::<Self>(body, "SignedTransaction").map_err(|_| {
                versioned_decode_rejection::<Self>(
                    "invalid canonical JSON SignedTransaction representation",
                )
            })
        }
    }

    impl SupportsVersionedJsonDecode for TransactionEntrypoint {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            decode_versioned_json::<Self>(body, "TransactionEntrypoint")
        }
    }

    impl SupportsVersionedJsonDecode for SignedQuery {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            let json = json::from_slice::<SignedQueryJson>(body.as_ref()).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("invalid JSON SignedQuery body: {e}"),
                )
                    .into_response()
            })?;
            json.try_into().map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("invalid JSON SignedQuery content: {e}"),
                )
                    .into_response()
            })
        }
    }

    /// Extractor for versioned request bodies supporting both Norito and JSON payloads.
    #[derive(Clone, Copy, Debug)]
    pub struct JsonOrNoritoVersioned<T>(pub T);

    fn versioned_decode_rejection<T: 'static>(versioned_err: impl std::fmt::Display) -> Response {
        let message = format!("Could not decode versioned request: {versioned_err}");
        if TypeId::of::<T>() == TypeId::of::<SignedTransaction>() {
            let mut response = super::respond_with_status_and_format(
                StatusCode::BAD_REQUEST,
                iroha_torii_shared::ErrorEnvelope::new(
                    "invalid_transaction_payload",
                    format!("transaction payload could not be decoded: {message}"),
                ),
                super::current_response_format(),
            );
            response.headers_mut().insert(
                "x-iroha-reject-code",
                HeaderValue::from_static("invalid_transaction_payload"),
            );
            return response;
        }
        (StatusCode::BAD_REQUEST, message).into_response()
    }

    impl<S, T> FromRequest<S> for JsonOrNoritoVersioned<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: iroha_version::codec::DecodeVersioned + SupportsVersionedJsonDecode + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "versioned requests must set `Content-Type: {}` or `Content-Type: application/json`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;

            if super::is_norito_media_type(raw) {
                return <T as iroha_version::codec::DecodeVersioned>::decode_all_versioned(&body)
                    .map(JsonOrNoritoVersioned)
                    .map_err(versioned_decode_rejection::<T>);
            }

            if super::is_json_media_type(raw) {
                return T::decode_versioned_json_body(&body).map(JsonOrNoritoVersioned);
            }

            Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported Content-Type `{raw}`; use application/json or {}",
                    super::NORITO_MIME_TYPE
                ),
            )
                .into_response())
        }
    }

    /// Extractor of Accept header
    #[cfg_attr(not(feature = "telemetry"), allow(unused))]
    pub struct ExtractAccept(pub HeaderValue);

    impl<S> FromRequestParts<S> for ExtractAccept
    where
        S: Send + Sync,
    {
        type Rejection = (StatusCode, &'static str);

        fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> impl core::future::Future<Output = Result<Self, Self::Rejection>> + Send {
            let res = parts
                .headers
                .get(axum::http::header::ACCEPT)
                .cloned()
                .map(ExtractAccept)
                .ok_or((StatusCode::BAD_REQUEST, "`Accept` header is missing"));
            core::future::ready(res)
        }
    }

    impl<S> OptionalFromRequestParts<S> for ExtractAccept
    where
        S: Send + Sync,
    {
        type Rejection = (StatusCode, &'static str);

        fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> impl core::future::Future<Output = Result<Option<Self>, Self::Rejection>> + Send
        {
            let value = parts
                .headers
                .get(axum::http::header::ACCEPT)
                .cloned()
                .map(ExtractAccept);
            core::future::ready(Ok(value))
        }
    }

    /// Helper trait exposing Norito decoding for extractor-bound types.
    pub trait SupportsNoritoDecode: Sized {
        fn decode_norito(bytes: &[u8]) -> Result<Self, norito::Error>;
    }

    impl<T> SupportsNoritoDecode for T
    where
        for<'a> T: NoritoDeserialize<'a>,
    {
        fn decode_norito(bytes: &[u8]) -> Result<Self, norito::Error> {
            norito::decode_from_bytes::<T>(bytes)
        }
    }

    #[allow(clippy::result_large_err)] // extraction needs to return a fully-formed HTTP rejection response
    fn decode_as_json<T: JsonDeserializeOwned>(body: &Bytes) -> Result<T, Response> {
        norito::json::from_slice::<T>(body.as_ref()).map_err(|e| {
            typed_request_rejection(
                StatusCode::BAD_REQUEST,
                "request_json_invalid",
                format!("Invalid JSON body: {e}"),
            )
        })
    }

    fn payload_kind_label<T: 'static>() -> &'static str {
        if TypeId::of::<T>() == TypeId::of::<SignedTransaction>() {
            "signed_transaction"
        } else if TypeId::of::<T>() == TypeId::of::<SignedQuery>() {
            "signed_query"
        } else {
            "other"
        }
    }

    fn record_payload_decode_failure<T: 'static>(error: &norito::Error) {
        record_norito_decode_failure(payload_kind_label::<T>(), error);
    }

    #[allow(clippy::result_large_err)] // extraction needs to return a fully-formed HTTP rejection response
    fn decode_as_norito<T: SupportsNoritoDecode + 'static>(body: &Bytes) -> Result<T, Response> {
        T::decode_norito(body.as_ref()).map_err(|e| {
            record_payload_decode_failure::<T>(&e);
            typed_request_rejection(
                StatusCode::BAD_REQUEST,
                "request_norito_invalid",
                format!("Invalid Norito body: {e}"),
            )
        })
    }

    #[cfg(feature = "telemetry")]
    fn record_norito_decode_failure(payload_kind: &'static str, error: &norito::Error) {
        iroha_telemetry::metrics::global_or_default()
            .inc_torii_norito_decode_failure(payload_kind, classify_norito_error(error));
    }

    #[cfg(feature = "telemetry")]
    fn classify_norito_error(error: &norito::Error) -> &'static str {
        use norito::Error;

        match error {
            Error::InvalidMagic => "invalid_magic",
            Error::UnsupportedVersion { .. } => "unsupported_version",
            Error::UnsupportedMinorVersion { .. } => "unsupported_minor_version",
            Error::UnsupportedCompression { .. } => "unsupported_compression",
            Error::LengthMismatch => "length_mismatch",
            Error::ArchiveLengthExceeded { .. } => "archive_length_exceeded",
            Error::ChecksumMismatch => "checksum_mismatch",
            Error::SchemaMismatch => "schema_mismatch",
            Error::UnsupportedFeature(flag) => match *flag {
                "layout flag" => "unsupported_feature_layout_flag",
                _ => "unsupported_feature",
            },
            Error::MissingPayloadContext => "missing_payload_context",
            Error::MissingLayoutFlags => "missing_layout_flags",
            Error::InvalidUtf8 => "invalid_utf8",
            Error::InvalidTag { .. } => "invalid_tag",
            Error::InvalidNonZero => "invalid_non_zero",
            Error::DecodePanic { .. } => "decode_panic",
            Error::Misaligned { .. } => "misaligned",
            Error::Io(_) => "io_error",
            Error::Message(_) => "message",
            #[cfg(feature = "json")]
            Error::Json(_) => "json_error",
            _ => "other",
        }
    }

    #[cfg(not(feature = "telemetry"))]
    fn record_norito_decode_failure(_: &'static str, _: &norito::Error) {}

    #[allow(clippy::result_large_err)]
    fn decode_body_as_norito_or_json<T: JsonDeserializeOwned + SupportsNoritoDecode + 'static>(
        body: &Bytes,
        format: super::TypedRequestContentFormat,
    ) -> Result<T, Response> {
        match format {
            super::TypedRequestContentFormat::Json => decode_as_json::<T>(body),
            super::TypedRequestContentFormat::Norito => decode_as_norito::<T>(body),
        }
    }

    /// Extractor for request bodies supporting both Norito and JSON payloads.
    #[derive(Clone, Copy, Debug)]
    pub struct NoritoJson<T>(pub T);

    impl<S, T> FromRequest<S> for NoritoJson<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send + 'static,
        T: SupportsNoritoDecode,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let format = super::typed_request_content_format(req.headers())?;
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;

            decode_body_as_norito_or_json::<T>(&body, format).map(NoritoJson)
        }
    }

    #[cfg(feature = "app_api")]
    const OFFLINE_CANONICAL_TOTAL_ALLOCATION_MULTIPLIER: usize = 4;
    #[cfg(feature = "app_api")]
    const OFFLINE_CANONICAL_STRUCTURAL_ALLOCATION_ALLOWANCE_BYTES: usize = 64 * 1024;
    /// Schema-specific body and decode limits for public Offline API requests.
    #[cfg(feature = "app_api")]
    pub(crate) trait OfflineCanonicalNoritoSchema:
        NoritoSerialize + for<'de> NoritoDeserialize<'de> + Sized
    {
        /// Exact protocol body ceiling, in canonical framed bytes.
        const MAX_BODY_BYTES: usize;
        /// Maximum nested value-decode depth needed by this request schema.
        const MAX_NESTING_DEPTH: usize;
        /// Extra frame-derived copies needed before semantic field caps run.
        const EXTRA_ENCODED_ALLOCATION_MULTIPLIER: usize = 0;
        /// Fixed allowance for schema-bounded nested proof reconstruction.
        const FIXED_ALLOCATION_ALLOWANCE_BYTES: usize = 0;

        /// Inspect schema-specific bounded fields before owned reconstruction.
        fn preflight_canonical_norito(_body: &[u8]) -> Result<(), norito::Error> {
            Ok(())
        }

        /// Build payload-derived limits for one already body-capped archive.
        fn decode_limits(encoded_len: usize) -> norito::core::DecodeLimits {
            // Every variable-length member is represented in the exact
            // uncompressed canonical frame. Per-sequence and cumulative
            // element limits therefore derive from the supplied frame, while
            // the fourfold base accounts for decoded structs and nested
            // collection storage. Schemas carrying bounded proof material add
            // only the fixed allowance required by their statically known
            // canonical field depth. This avoids inheriting Norito's generic
            // 32-fold allowance. The schema-specific body ceiling is checked
            // before these limits are installed.
            norito::core::DecodeLimits::new(
                encoded_len,
                encoded_len,
                encoded_len.saturating_mul(2),
                encoded_len
                    .saturating_mul(
                        OFFLINE_CANONICAL_TOTAL_ALLOCATION_MULTIPLIER
                            .saturating_add(Self::EXTRA_ENCODED_ALLOCATION_MULTIPLIER),
                    )
                    .saturating_add(Self::FIXED_ALLOCATION_ALLOWANCE_BYTES),
                Self::MAX_NESTING_DEPTH,
            )
        }
    }

    #[cfg(feature = "app_api")]
    impl OfflineCanonicalNoritoSchema
        for iroha_torii_shared::offline_api::OfflineRecipientLineageRequest
    {
        const MAX_BODY_BYTES: usize =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2;
        const MAX_NESTING_DEPTH: usize = 32;
        const FIXED_ALLOCATION_ALLOWANCE_BYTES: usize =
            OFFLINE_CANONICAL_STRUCTURAL_ALLOCATION_ALLOWANCE_BYTES;
    }

    #[cfg(feature = "app_api")]
    impl OfflineCanonicalNoritoSchema for iroha_torii_shared::offline_api::OfflineTopUpRequest {
        const MAX_BODY_BYTES: usize =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4;
        const MAX_NESTING_DEPTH: usize = 32;
        // The proof bytes cross up to ten charged reconstruction boundaries;
        // the fourfold frame-derived base already covers four of them.
        const FIXED_ALLOCATION_ALLOWANCE_BYTES: usize =
            iroha_data_model::offline::KAGEMUSHA_TOPUP_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4;
    }

    #[cfg(feature = "app_api")]
    impl OfflineCanonicalNoritoSchema for iroha_torii_shared::offline_api::OfflineRedeemRequest {
        const MAX_BODY_BYTES: usize =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4;
        const MAX_NESTING_DEPTH: usize = 64;
        // The main and optional-change recursive proofs have fixed protocol
        // caps. Wire preflight reads the nested unshield Vec length and rejects
        // it above 192 KiB before reconstruction; the fixed allowance therefore
        // carries its three remaining charged copies.
        const EXTRA_ENCODED_ALLOCATION_MULTIPLIER: usize =
            iroha_data_model::offline::KAGEMUSHA_REDEEM_CANONICAL_DECODE_EXTRA_ALLOCATION_MULTIPLIER_V4;
        const FIXED_ALLOCATION_ALLOWANCE_BYTES: usize =
            iroha_data_model::offline::KAGEMUSHA_REDEEM_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4;

        fn preflight_canonical_norito(body: &[u8]) -> Result<(), norito::Error> {
            iroha_data_model::offline::preflight_kagemusha_redeem_request_archive_v4(body)
        }
    }

    #[cfg(feature = "app_api")]
    #[derive(Debug)]
    enum OfflineCanonicalNoritoDecodeError {
        Empty,
        TooLarge { actual: usize, maximum: usize },
        Norito(norito::Error),
    }

    #[cfg(feature = "app_api")]
    fn validate_offline_canonical_norito_body_len(
        actual: usize,
        maximum: usize,
    ) -> Result<(), OfflineCanonicalNoritoDecodeError> {
        if actual == 0 {
            return Err(OfflineCanonicalNoritoDecodeError::Empty);
        }
        if actual > maximum {
            return Err(OfflineCanonicalNoritoDecodeError::TooLarge { actual, maximum });
        }
        Ok(())
    }

    #[cfg(feature = "app_api")]
    fn decode_offline_canonical_norito<T: OfflineCanonicalNoritoSchema>(
        body: &[u8],
    ) -> Result<T, OfflineCanonicalNoritoDecodeError> {
        validate_offline_canonical_norito_body_len(body.len(), T::MAX_BODY_BYTES)?;
        T::preflight_canonical_norito(body).map_err(OfflineCanonicalNoritoDecodeError::Norito)?;
        norito::decode_canonical_with_limits(body, T::decode_limits(body.len()))
            .map_err(OfflineCanonicalNoritoDecodeError::Norito)
    }

    #[cfg(feature = "app_api")]
    #[allow(clippy::result_large_err)]
    fn offline_canonical_norito_rejection<T: 'static>(
        error: OfflineCanonicalNoritoDecodeError,
    ) -> Response {
        match error {
            OfflineCanonicalNoritoDecodeError::Empty => typed_request_rejection(
                StatusCode::BAD_REQUEST,
                "request_norito_invalid",
                "Offline Norito request body must not be empty.",
            ),
            OfflineCanonicalNoritoDecodeError::TooLarge { actual, maximum } => {
                typed_request_rejection(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request_payload_too_large",
                    format!(
                        "Offline Norito request body is {actual} bytes; maximum is {maximum} bytes."
                    ),
                )
            }
            OfflineCanonicalNoritoDecodeError::Norito(error) => {
                record_payload_decode_failure::<T>(&error);
                typed_request_rejection(
                    StatusCode::BAD_REQUEST,
                    "request_norito_invalid",
                    format!("Invalid canonical offline Norito body: {error}"),
                )
            }
        }
    }

    /// Extractor for one canonical, schema-bounded Offline API Norito request.
    #[cfg(feature = "app_api")]
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct OfflineNorito<T>(
        /// Decoded canonical request.
        pub(crate) T,
    );

    #[cfg(feature = "app_api")]
    impl<S, T> FromRequest<S> for OfflineNorito<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: OfflineCanonicalNoritoSchema + Send + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            super::norito_request_content_type(req.headers())?;
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;
            decode_offline_canonical_norito::<T>(&body)
                .map(OfflineNorito)
                .map_err(offline_canonical_norito_rejection::<T>)
        }
    }

    /// Extractor for a native-Norito request body using compatibility decoding.
    #[derive(Clone, Copy, Debug)]
    pub struct NoritoOnly<T>(pub T);

    impl<S, T> FromRequest<S> for NoritoOnly<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: SupportsNoritoDecode + Send + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            super::norito_request_content_type(req.headers())?;
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;
            decode_as_norito::<T>(&body).map(NoritoOnly)
        }
    }

    /// Extractor that returns both the decoded payload and the raw request body.
    #[derive(Clone, Debug)]
    pub struct NoritoJsonWithBytes<T> {
        /// Parsed payload decoded from Norito or JSON.
        pub value: T,
        /// Raw request body bytes as sent over the wire.
        pub raw: Bytes,
    }

    impl<S, T> FromRequest<S> for NoritoJsonWithBytes<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send + 'static,
        T: SupportsNoritoDecode,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let format = super::typed_request_content_format(req.headers())?;
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;

            decode_body_as_norito_or_json::<T>(&body, format)
                .map(|value| NoritoJsonWithBytes { value, raw: body })
        }
    }

    /// Extractor enforcing JSON payloads decoded with the Norito JSON codec.
    #[derive(Clone, Debug)]
    pub struct JsonOnly<T>(pub T);

    impl<S, T> FromRequest<S> for JsonOnly<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let content_type = req.headers().get(CONTENT_TYPE).cloned();
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;
            let declared = content_type
                .as_ref()
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty());

            if let Some(ct) = declared {
                if !super::is_json_media_type(ct) {
                    return Err((
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        format!("unsupported Content-Type `{ct}`; expected application/json"),
                    )
                        .into_response());
                }
            }

            decode_as_json::<T>(&body).map(JsonOnly)
        }
    }

    /// Extractor enforcing the canonical first-release JSON request media type.
    #[derive(Clone, Debug)]
    pub struct CanonicalJsonOnly<T>(pub T);

    impl<S, T> FromRequest<S> for CanonicalJsonOnly<T>
    where
        Bytes: FromRequest<S, Rejection = axum::extract::rejection::BytesRejection>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            super::canonical_json_request_content_type(req.headers())?;
            let body = Bytes::from_request(req, state)
                .await
                .map_err(typed_body_rejection)?;
            decode_as_json::<T>(&body).map(CanonicalJsonOnly)
        }
    }

    /// Extractor for URL query strings decoded into `JsonDeserialize` types.
    #[derive(Clone, Debug)]
    pub struct NoritoQuery<T>(pub T);

    impl<S, T> FromRequestParts<S> for NoritoQuery<T>
    where
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            let query = parts.uri.query().unwrap_or("");
            match decode_query::<T>(query) {
                Ok(value) => Ok(NoritoQuery(value)),
                Err(e) => Err(typed_request_rejection(
                    StatusCode::BAD_REQUEST,
                    "request_query_invalid",
                    format!("invalid query params: {e}"),
                )),
            }
        }
    }

    /// Extractor for URL query strings decoded into `JsonDeserialize` types
    /// without scalar type coercion.
    #[derive(Clone, Debug)]
    pub struct NoritoStringQuery<T>(pub T);

    impl<S, T> FromRequestParts<S> for NoritoStringQuery<T>
    where
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            let query = parts.uri.query().unwrap_or("");
            match decode_string_query::<T>(query) {
                Ok(value) => Ok(NoritoStringQuery(value)),
                Err(e) => Err(typed_request_rejection(
                    StatusCode::BAD_REQUEST,
                    "request_query_invalid",
                    format!("invalid query params: {e}"),
                )),
            }
        }
    }

    fn decode_query<T: JsonDeserializeOwned>(query: &str) -> Result<T, json::Error> {
        let pairs = query_pairs(query)?;
        reject_duplicate_query_keys(&pairs)?;
        let mut object = json::Map::new();
        for (key, value) in pairs {
            object.insert(key, scalar_to_value(&value));
        }
        json::from_value(Value::Object(object))
    }

    fn decode_string_query<T: JsonDeserializeOwned>(query: &str) -> Result<T, json::Error> {
        let pairs = query_pairs(query)?;
        reject_duplicate_query_keys(&pairs)?;
        let mut object = json::Map::new();
        for (key, value) in pairs {
            object.insert(key, Value::String(value));
        }
        json::from_value(Value::Object(object))
    }

    fn reject_duplicate_query_keys(pairs: &[(String, String)]) -> Result<(), json::Error> {
        // Structured query DTOs have exactly one value per field. Keep this
        // check local to the DTO decoders so protocol-specific parsers can
        // still define ordered or repeated-key semantics explicitly.
        let mut seen = std::collections::BTreeSet::new();
        for (key, _) in pairs {
            if !seen.insert(key.as_str()) {
                return Err(json::Error::duplicate_field(key));
            }
        }
        Ok(())
    }

    fn query_pairs(query: &str) -> Result<Vec<(String, String)>, json::Error> {
        query
            .split('&')
            .filter(|segment| !segment.is_empty())
            .map(|segment| {
                let mut parts = segment.splitn(2, '=');
                let raw_key = parts.next().unwrap_or("");
                let raw_value = parts.next().unwrap_or("");
                Ok((decode_component(raw_key)?, decode_component(raw_value)?))
            })
            .collect()
    }

    fn decode_component(input: &str) -> Result<String, json::Error> {
        // HTML form query semantics decode `+` as a space before percent
        // decoding. A literal plus must therefore be encoded as `%2B`.
        let bytes = input.as_bytes();
        let mut position = 0;
        while position < bytes.len() {
            if bytes[position] != b'%' {
                position += 1;
                continue;
            }
            let Some(high) = bytes.get(position + 1).copied() else {
                return Err(json::Error::Message(
                    "invalid percent-encoding in query component".to_owned(),
                ));
            };
            let Some(low) = bytes.get(position + 2).copied() else {
                return Err(json::Error::Message(
                    "invalid percent-encoding in query component".to_owned(),
                ));
            };
            if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
                return Err(json::Error::Message(
                    "invalid percent-encoding in query component".to_owned(),
                ));
            }
            position += 3;
        }

        let replaced = input.replace('+', " ");
        decode(&replaced)
            .map(std::borrow::Cow::into_owned)
            .map_err(|error| {
                json::Error::Message(format!(
                    "invalid percent-encoding in query component: {error}"
                ))
            })
    }

    fn scalar_to_value(raw: &str) -> Value {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("null") {
            Value::Null
        } else if trimmed.eq_ignore_ascii_case("true") {
            Value::Bool(true)
        } else if trimmed.eq_ignore_ascii_case("false") {
            Value::Bool(false)
        } else if let Ok(u) = trimmed.parse::<u64>() {
            Value::Number(Number::from(u))
        } else if let Ok(i) = trimmed.parse::<i64>() {
            Value::Number(Number::from(i))
        } else if let Ok(f) = trimmed.parse::<f64>() {
            Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(trimmed.to_string()))
        } else {
            Value::String(trimmed.to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use axum::{
            body::Body,
            extract::{DefaultBodyLimit, FromRequestParts},
            http::{HeaderValue, Request, StatusCode, header::CONTENT_TYPE},
        };
        use http_body_util::BodyExt as _;
        use iroha_version::{RawVersioned, UnsupportedVersion, Version};
        use norito::core::{NoritoDeserialize, NoritoSerialize};

        use super::*;

        #[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
        struct Dummy(u32);

        #[cfg(feature = "app_api")]
        impl OfflineCanonicalNoritoSchema for Vec<u64> {
            const MAX_BODY_BYTES: usize = 4 * 1024;
            const MAX_NESTING_DEPTH: usize = 8;
        }

        #[cfg(feature = "app_api")]
        fn offline_ingress_top_up_fixture() -> iroha_torii_shared::offline_api::OfflineTopUpRequest
        {
            use iroha_crypto::{Algorithm, KeyPair};
            use iroha_data_model::{
                ChainId,
                account::AccountId,
                asset::{AssetDefinitionId, AssetId},
                domain::DomainId,
                offline::{
                    KagemushaAndroidKeyMintHardwareAssertionV1, KagemushaDeviceSignatureV2,
                    KagemushaOnlineHardwareAssertionV1, KagemushaRecursiveSpendArtifactBindingV4,
                    KagemushaRequestAuthorizationV2, KagemushaScaledAmountV2,
                    KagemushaSpendableNoteDescriptorV2, KagemushaTopUpShieldEvidenceV2,
                },
                proof::{ProofAttachment, ProofBox, VerifyingKeyId},
            };

            let key_pair = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519)
                .expect("derive offline-ingress fixture key");
            let authority = AccountId::new(key_pair.public_key().clone());
            let definition = AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("fixture domain"),
                "ingress".parse().expect("fixture asset name"),
            );
            let chain_id = ChainId::from("offline-ingress-test");
            let amount = KagemushaScaledAmountV2 {
                atomic_units: 500,
                scale: 0,
            };
            let operation_id = [0x42; 32];
            let backend = "halo2/ipa";
            let mut proof = ProofAttachment::new_ref(
                backend.into(),
                ProofBox::new(backend.to_owned(), vec![0x43]),
                VerifyingKeyId::new(backend, "offline-ingress-topup"),
            );
            proof.vk_commitment = Some([0x44; 32]);

            iroha_torii_shared::offline_api::OfflineTopUpRequest {
                version: 4,
                asset: AssetId::new(definition.clone(), authority.clone()),
                amount,
                current_note: KagemushaSpendableNoteDescriptorV2 {
                    chain_id,
                    asset: definition.clone(),
                    note_commitment: [0x45; 32],
                    spend_nullifier: [0x46; 32],
                    amount,
                },
                shield_evidence: KagemushaTopUpShieldEvidenceV2 {
                    initial_root: [0x47; 32],
                    finalized_root: [0x48; 32],
                    leaf_index: 0,
                    proof,
                },
                artifact_binding: KagemushaRecursiveSpendArtifactBindingV4 {
                    version: 4,
                    generation: "offline-ingress-fixture".to_owned(),
                    manifest_sha256: [0x49; 32],
                },
                operation_id,
                authorization: KagemushaRequestAuthorizationV2 {
                    authority,
                    device_id: "offline-ingress-device".to_owned(),
                    asset_definition_id: definition,
                    operation_id,
                    issued_at_ms: 1,
                    expires_at_ms: 2,
                    nonce: [0x4A; 32],
                    payload_digest: [0x4B; 32],
                    registration_hash: [0x4C; 32],
                    hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                        KagemushaAndroidKeyMintHardwareAssertionV1 {
                            signature: KagemushaDeviceSignatureV2::from_raw_bytes(&[1; 64])
                                .expect("canonical low-S fixture signature"),
                        },
                    ),
                },
            }
        }

        #[cfg(feature = "app_api")]
        fn offline_ingress_redeem_fixture(
            top_up: &iroha_torii_shared::offline_api::OfflineTopUpRequest,
        ) -> iroha_torii_shared::offline_api::OfflineRedeemRequest {
            use iroha_data_model::{
                offline::{
                    KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2,
                    KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KagemushaPastaCycleParityV1,
                    KagemushaPastaCycleProofEnvelopeV4, KagemushaRecursiveSpendBranchClaimV2,
                    KagemushaRecursiveSpendBundleV4, KagemushaRecursiveSpendOperationVectorV4,
                    KagemushaRecursiveSpendProofV4, KagemushaRecursiveSpendPublicStatementV4,
                    KagemushaRecursiveSpendRedemptionIntentV4,
                    KagemushaRecursiveSpendStateBoundaryV2,
                    KagemushaRecursiveSpendTopUpAnchorRefV2,
                    KagemushaUnshieldPublicInputsBindingV2,
                    kagemusha_recursive_spend_verifier_key_id_v4,
                },
                proof::{ProofAttachment, ProofBox, VerifyingKeyId},
            };

            let operation_id = [0x51; 32];
            let amount = top_up.amount;
            let note = top_up.current_note.clone();
            let binding = top_up.artifact_binding.clone();
            let anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
                topup_operation_id: top_up.operation_id,
                anchor_digest: [0x52; 32],
            };
            let branch_claim = KagemushaRecursiveSpendBranchClaimV2::root(anchor_ref.anchor_digest)
                .expect("canonical root branch claim");
            let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
                KagemushaPastaCycleParityV1::StepEq,
                binding.manifest_sha256,
            );
            let statement = KagemushaRecursiveSpendPublicStatementV4 {
                chain_id: note.chain_id.clone(),
                asset: note.asset.clone(),
                asset_scale: amount.scale,
                final_root: [0x53; 32],
                next_zero_leaf_index: 1,
                topup_anchor_refs: vec![anchor_ref],
                proof_step_count: 1,
                peer_hop_count: 0,
                current_note: note.clone(),
                branch_claims: vec![branch_claim.clone()],
                transition: None,
                artifact_binding: binding.clone(),
                verifier_key_id: verifier_key_id.clone(),
            };
            let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
            state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
            let bundle = KagemushaRecursiveSpendBundleV4 {
                statement,
                operation: KagemushaRecursiveSpendOperationVectorV4 {
                    limbs: [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4],
                },
                recursive_proof: KagemushaRecursiveSpendProofV4 {
                    verifier_key_id: verifier_key_id.clone(),
                    public_statement_digest: [0x54; 32],
                    proof_envelope: KagemushaPastaCycleProofEnvelopeV4 {
                        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
                            .to_owned(),
                        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
                            .to_owned(),
                        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4
                            .to_owned(),
                        artifact_generation: binding.generation.clone(),
                        manifest_sha256: binding.manifest_sha256,
                        step_eq_parameter_generation: "offline-ingress-eq".to_owned(),
                        step_ep_parameter_generation: "offline-ingress-ep".to_owned(),
                        step_eq_circuit_params_sha256: [0x55; 32],
                        step_ep_circuit_params_sha256: [0x56; 32],
                        step_eq_verifier_key_sha256: [0x57; 32],
                        step_ep_verifier_key_sha256: [0x58; 32],
                        state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)
                            .expect("fixture state boundary"),
                        proof: ProofBox::new(
                            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                            vec![0x59],
                        ),
                    },
                },
            };
            let public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
                input_commitment_0: note.note_commitment,
                input_commitment_1: [0; 32],
                nullifier_0: note.spend_nullifier,
                nullifier_1: [0; 32],
                change_output_commitment: [0; 32],
                root: [0x53; 32],
                public_amount: [0x5A; 32],
                asset_tag: [0x5B; 32],
                chain_tag: [0x5C; 32],
            };
            let redemption = KagemushaRecursiveSpendRedemptionIntentV4 {
                chain_id: note.chain_id.clone(),
                asset: note.asset.clone(),
                input_note: note,
                parent_branch_claims: vec![branch_claim],
                parent_topup_anchor_refs: vec![anchor_ref],
                parent_proof_step_count: 1,
                parent_peer_hop_count: 0,
                parent_bundle_digest: [0x5D; 32],
                input_root: [0x53; 32],
                recipient: top_up.authorization.authority.clone(),
                public_amount: amount,
                change_output: None,
                change_artifact_binding: None,
                unshield_public_inputs: public_inputs,
                unshield_public_inputs_digest: [0x5E; 32],
                operation_id,
            };
            let backend = "halo2/ipa";
            let mut redeem_proof = ProofAttachment::new_ref(
                backend.into(),
                ProofBox::new(backend.to_owned(), vec![0x5F]),
                VerifyingKeyId::new(backend, "offline-ingress-unshield"),
            );
            redeem_proof.vk_commitment = Some([0x60; 32]);
            let mut authorization = top_up.authorization.clone();
            authorization.operation_id = operation_id;
            authorization.payload_digest = [0x61; 32];

            iroha_torii_shared::offline_api::OfflineRedeemRequest {
                version: 4,
                bundle,
                recipient: authorization.authority.clone(),
                amount,
                redeem_proof,
                redemption,
                offline_change: None,
                block_height: 1,
                operation_id,
                authorization,
            }
        }

        #[cfg(feature = "app_api")]
        fn offline_ingress_max_shaped_redeem_fixture(
            top_up: &iroha_torii_shared::offline_api::OfflineTopUpRequest,
        ) -> iroha_torii_shared::offline_api::OfflineRedeemRequest {
            let mut redeem = offline_ingress_redeem_fixture(top_up);
            redeem.bundle.recursive_proof.proof_envelope.proof.bytes = vec![
                0x72;
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
                    as usize
            ];
            redeem.redeem_proof.proof.bytes =
                vec![0x73; iroha_data_model::offline::KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4];
            let mut change_output = redeem.bundle.statement.current_note.clone();
            change_output.note_commitment = [0x74; 32];
            change_output.spend_nullifier = [0x75; 32];
            let mut change_bundle = redeem.bundle.clone();
            change_bundle.statement.current_note = change_output.clone();
            change_bundle.recursive_proof.proof_envelope.proof.bytes = vec![
                0x76;
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
                    as usize
            ];
            let change_branch_claims = change_bundle.statement.branch_claims.clone();
            redeem.redemption.change_output = Some(change_output.clone());
            redeem.redemption.change_artifact_binding =
                Some(change_bundle.statement.artifact_binding.clone());
            redeem.offline_change = Some(
                iroha_data_model::offline::KagemushaRecursiveSpendRedeemChangeBranchV4 {
                    output: change_output,
                    branch_claims: change_branch_claims,
                    bundle: change_bundle,
                },
            );
            redeem
        }

        #[derive(Clone, Debug, PartialEq, crate::json_macros::JsonDeserialize)]
        struct StringQueryForTest {
            numeric_hex: Option<String>,
            leading_zero_hex: Option<String>,
            null_like: Option<String>,
            bool_like: Option<String>,
            label: Option<String>,
        }

        #[derive(Clone, Debug, PartialEq, crate::json_macros::JsonDeserialize)]
        struct RequiredQueryForTest {
            asset_definition_id: String,
        }

        #[tokio::test]
        async fn malformed_query_uses_typed_error_envelope() {
            let (mut parts, _) = Request::builder()
                .uri("/?unexpected=value")
                .body(())
                .expect("request")
                .into_parts();
            let response = super::super::with_current_response_format(
                super::super::ResponseFormat::Json,
                NoritoQuery::<RequiredQueryForTest>::from_request_parts(&mut parts, &()),
            )
            .await
            .expect_err("missing required query field must fail");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                response.headers().get(CONTENT_TYPE),
                Some(&HeaderValue::from_static("application/json"))
            );
            let bytes = response
                .into_body()
                .collect()
                .await
                .expect("collect typed query error")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&bytes).expect("decode typed query error");
            assert_eq!(envelope.code(), "request_query_invalid");
        }

        #[tokio::test]
        async fn duplicate_query_fields_are_rejected_before_deserialization() {
            for query in [
                "asset_definition_id=first&asset_definition_id=second",
                "asset_definition_id=first&asset%5fdefinition%5fid=second",
                "asset_definition_id=first&%61sset_definition_id=second",
                "asset_definition_id=first&asset%5Fdefinition%5Fid=second",
            ] {
                let request = Request::builder()
                    .uri(format!("/?{query}"))
                    .body(())
                    .expect("request");
                let (mut parts, _) = request.into_parts();

                let response = super::super::with_current_response_format(
                    super::super::ResponseFormat::Json,
                    NoritoQuery::<RequiredQueryForTest>::from_request_parts(&mut parts, &()),
                )
                .await
                .expect_err("duplicate query field must fail");

                assert_eq!(response.status(), StatusCode::BAD_REQUEST, "query={query}");
                let bytes = response
                    .into_body()
                    .collect()
                    .await
                    .expect("collect duplicate-query error")
                    .to_bytes();
                let envelope: iroha_torii_shared::ErrorEnvelope =
                    norito::json::from_slice(&bytes).expect("decode duplicate-query error");
                assert_eq!(envelope.code(), "request_query_invalid", "query={query}");
                assert!(
                    envelope.message().contains("duplicate field"),
                    "query={query}, envelope={envelope:?}"
                );
            }
        }

        #[tokio::test]
        async fn malformed_percent_encoding_uses_typed_query_error() {
            for malformed in ["%", "%2", "%GG"] {
                for query in [
                    format!("asset_definition_id={malformed}"),
                    format!("{malformed}=value"),
                ] {
                    let request = Request::builder()
                        .uri(format!("/?{query}"))
                        .body(())
                        .expect("request URI");
                    let (mut parts, _) = request.into_parts();
                    let response = super::super::with_current_response_format(
                        super::super::ResponseFormat::Json,
                        NoritoQuery::<RequiredQueryForTest>::from_request_parts(&mut parts, &()),
                    )
                    .await
                    .expect_err("malformed percent escape must fail closed");

                    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "query={query}");
                    let bytes = response
                        .into_body()
                        .collect()
                        .await
                        .expect("collect malformed-query response")
                        .to_bytes();
                    let envelope: iroha_torii_shared::ErrorEnvelope =
                        norito::json::from_slice(&bytes).expect("decode typed query error");
                    assert_eq!(envelope.code(), "request_query_invalid");
                    assert!(
                        envelope.message().contains("percent-encoding"),
                        "query={query}, envelope={envelope:?}"
                    );
                }
            }
        }

        #[test]
        fn query_plus_and_percent_encoded_plus_have_distinct_form_semantics() {
            let space: StringQueryForTest =
                super::decode_string_query("label=tron+nile").expect("form-space query");
            assert_eq!(space.label.as_deref(), Some("tron nile"));

            let plus: StringQueryForTest =
                super::decode_string_query("label=tron%2Bnile").expect("literal-plus query");
            assert_eq!(plus.label.as_deref(), Some("tron+nile"));
        }

        #[tokio::test]
        async fn duplicate_string_query_fields_are_rejected_before_deserialization() {
            let request = Request::builder()
                .uri("/?asset_definition_id=first&asset_definition_id=second")
                .body(())
                .expect("request");
            let (mut parts, _) = request.into_parts();

            let response = super::super::with_current_response_format(
                super::super::ResponseFormat::Json,
                NoritoStringQuery::<RequiredQueryForTest>::from_request_parts(&mut parts, &()),
            )
            .await
            .expect_err("duplicate string query field must fail");

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let bytes = response
                .into_body()
                .collect()
                .await
                .expect("collect duplicate string-query error")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&bytes).expect("decode duplicate string-query error");
            assert_eq!(envelope.code(), "request_query_invalid");
            assert!(envelope.message().contains("duplicate field"));
        }

        impl Version for Dummy {
            fn version(&self) -> u8 {
                1
            }

            fn supported_versions() -> std::ops::Range<u8> {
                1..2
            }
        }

        impl iroha_version::codec::DecodeVersioned for Dummy {
            fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
                let (version, rest) = input
                    .split_first()
                    .ok_or(iroha_version::error::Error::NotVersioned)?;
                if *version != 1 {
                    return Err(iroha_version::error::Error::UnsupportedVersion(Box::new(
                        UnsupportedVersion::new(*version, RawVersioned::NoritoBytes(rest.to_vec())),
                    )));
                }
                norito::decode_from_bytes(rest).map_err(Into::into)
            }
        }

        #[tokio::test]
        async fn decodes_versioned_payload() {
            let mut payload = vec![1];
            payload
                .extend_from_slice(&norito::to_bytes(&Dummy(42)).expect("encode versioned dummy"));
            let req = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(payload))
                .unwrap();
            let extracted = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect("extract versioned");
            assert_eq!(extracted.0, Dummy(42));
        }

        #[tokio::test]
        async fn rejects_missing_content_type() {
            let body = norito::to_bytes(&Dummy(7)).expect("encode bare dummy");
            let req = Request::new(Body::from(body));
            let err = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect_err("missing content-type should be rejected");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn surfaces_decode_error() {
            let req = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(vec![2]))
                .expect("request");
            let err = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect_err("should fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            let body_bytes = http_body_util::BodyExt::collect(err.into_body())
                .await
                .expect("collect error body")
                .to_bytes();
            let body_text = String::from_utf8(body_bytes.to_vec()).expect("response text");
            assert!(
                body_text.contains("Could not decode versioned request"),
                "body should describe decode failure: {body_text}"
            );
            assert!(
                body_text.to_ascii_lowercase().contains("version"),
                "body should mention versioned decode reason: {body_text}"
            );
        }

        #[tokio::test]
        async fn norito_string_query_preserves_numeric_looking_strings() {
            let numeric_hex = "11".repeat(32);
            let leading_zero_hex = format!("{}1", "0".repeat(63));
            let request = Request::builder()
                .uri(format!(
                    "/test?numeric_hex={numeric_hex}&leading_zero_hex={leading_zero_hex}&null_like=null&bool_like=true&label=tron+nile"
                ))
                .body(())
                .expect("request");
            let (mut parts, _) = request.into_parts();

            let NoritoStringQuery(decoded) =
                NoritoStringQuery::<StringQueryForTest>::from_request_parts(&mut parts, &())
                    .await
                    .expect("string query should decode");

            assert_eq!(decoded.numeric_hex.as_deref(), Some(numeric_hex.as_str()));
            assert_eq!(
                decoded.leading_zero_hex.as_deref(),
                Some(leading_zero_hex.as_str())
            );
            assert_eq!(decoded.null_like.as_deref(), Some("null"));
            assert_eq!(decoded.bool_like.as_deref(), Some("true"));
            assert_eq!(decoded.label.as_deref(), Some("tron nile"));
        }

        #[tokio::test]
        async fn signed_transaction_versioned_decode_error_returns_error_envelope() {
            let req = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(Vec::<u8>::new()))
                .expect("request");
            let err = JsonOrNoritoVersioned::<SignedTransaction>::from_request(req, &())
                .await
                .expect_err("should fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                err.headers().get(CONTENT_TYPE),
                Some(&HeaderValue::from_static(super::super::NORITO_MIME_TYPE))
            );
            assert_eq!(
                err.headers()
                    .get("x-iroha-reject-code")
                    .and_then(|value| value.to_str().ok()),
                Some("invalid_transaction_payload")
            );

            let body = http_body_util::BodyExt::collect(err.into_body())
                .await
                .expect("collect error body")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::decode_from_bytes(&body).expect("decode error envelope");
            assert_eq!(envelope.code(), "invalid_transaction_payload");
            assert!(
                envelope
                    .message()
                    .contains("transaction payload could not be decoded"),
                "unexpected error envelope: {envelope:?}"
            );
        }

        #[tokio::test]
        async fn signed_transaction_json_decode_error_uses_the_same_exact_contract() {
            let req = Request::builder()
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(br#"{"version":1,"content":{}}"#.as_slice()))
                .expect("request");
            let err = JsonOrNoritoVersioned::<SignedTransaction>::from_request(req, &())
                .await
                .expect_err("malformed canonical JSON transaction should fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                err.headers()
                    .get("x-iroha-reject-code")
                    .and_then(|value| value.to_str().ok()),
                Some("invalid_transaction_payload")
            );

            let body = http_body_util::BodyExt::collect(err.into_body())
                .await
                .expect("collect error body")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::decode_from_bytes(&body).expect("decode error envelope");
            assert_eq!(envelope.code(), "invalid_transaction_payload");
            assert!(
                envelope
                    .message()
                    .contains("transaction payload could not be decoded"),
                "unexpected error envelope: {envelope:?}"
            );
        }

        #[tokio::test]
        async fn content_type_matrix_maps_to_status_codes() {
            let versioned_ok = {
                let mut payload = vec![1];
                payload.extend_from_slice(
                    &norito::to_bytes(&Dummy(11)).expect("encode versioned dummy"),
                );
                payload
            };
            let bare_ok = norito::to_bytes(&Dummy(7)).expect("encode bare dummy");
            let cases = [
                (
                    "missing",
                    None,
                    bare_ok.clone(),
                    Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
                    Some("Content-Type"),
                ),
                (
                    "wrong",
                    Some("text/plain"),
                    bare_ok.clone(),
                    Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
                    Some("Content-Type"),
                ),
                (
                    "versioned decode fail",
                    Some(super::super::NORITO_MIME_TYPE),
                    vec![2_u8],
                    Err(StatusCode::BAD_REQUEST),
                    Some("versioned"),
                ),
                (
                    "bare norito is rejected",
                    Some(super::super::NORITO_MIME_TYPE),
                    bare_ok,
                    Err(StatusCode::BAD_REQUEST),
                    Some("versioned"),
                ),
                (
                    "norito with parameters succeeds",
                    Some("application/x-norito; charset=utf-8"),
                    versioned_ok.clone(),
                    Ok(Dummy(11)),
                    None,
                ),
                (
                    "plain norito succeeds",
                    Some(super::super::NORITO_MIME_TYPE),
                    versioned_ok,
                    Ok(Dummy(11)),
                    None,
                ),
            ];

            for (label, content_type, payload, expected, snippet) in cases {
                let mut builder = Request::builder();
                if let Some(ct) = content_type {
                    builder = builder.header(CONTENT_TYPE, ct);
                }
                let req = builder.body(Body::from(payload)).expect("request");
                let result = NoritoVersioned::<Dummy>::from_request(req, &()).await;
                match expected {
                    Ok(expected_payload) => {
                        let ok = result.unwrap_or_else(|resp| {
                            panic!("{label} should succeed, got {}", resp.status())
                        });
                        assert_eq!(ok.0, expected_payload, "case `{label}` payload mismatch");
                    }
                    Err(expected_status) => {
                        let err = result.expect_err(label);
                        assert_eq!(err.status(), expected_status, "case `{label}`");
                        if let Some(snippet) = snippet {
                            let body = http_body_util::BodyExt::collect(err.into_body())
                                .await
                                .expect("collect body")
                                .to_bytes();
                            let text = String::from_utf8_lossy(&body);
                            assert!(
                                text.to_ascii_lowercase()
                                    .contains(&snippet.to_ascii_lowercase()),
                                "case `{label}` should mention `{snippet}`: {text}"
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn negotiate_accept_header_prefers_norito() {
            let header =
                HeaderValue::from_static("application/json;q=0.5, application/x-norito;q=0.9");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_honors_json() {
            let header = HeaderValue::from_static("application/json");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_accept_header_defaults_norito() {
            let format = super::super::negotiate_response_format(None).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_wildcard_defaults_norito() {
            let header = HeaderValue::from_static("*/*");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_explicit_zero_overrides_wildcard() {
            let header = HeaderValue::from_static("application/x-norito;q=0, */*;q=1");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_accept_header_uses_wildcard_quality_per_representation() {
            let header = HeaderValue::from_static("application/json;q=0.8, */*;q=0.9");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_prefers_more_specific_equal_quality() {
            let header = HeaderValue::from_static("application/json, application/*");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_uses_first_equally_specific_range() {
            let header = HeaderValue::from_static(
                "application/json;q=0.2, application/json;q=0.9, application/x-norito;q=0.5",
            );
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_rejects_concrete_structured_suffix_for_plain_json() {
            let header = HeaderValue::from_static("application/vnd.api+json");
            let error = super::super::negotiate_response_format(Some(&header))
                .expect_err("a concrete vendor type must not accept application/json");
            assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn negotiate_ignores_unrelated_suffix_quality_and_preserves_exact_precedence() {
            let header = HeaderValue::from_static(
                "application/vnd.api+json;q=1, application/json;q=0.4, application/x-norito;q=0.5",
            );
            assert_eq!(
                super::super::negotiate_response_format(Some(&header)).expect("format"),
                super::super::ResponseFormat::Norito,
                "a high-quality vendor type must not raise application/json's quality"
            );

            let header = HeaderValue::from_static(
                "application/vnd.api+json;q=1, application/json;q=0, */*;q=0.5",
            );
            assert_eq!(
                super::super::negotiate_response_format(Some(&header)).expect("format"),
                super::super::ResponseFormat::Norito,
                "an exact application/json rejection must override its wildcard match"
            );
        }

        #[test]
        fn negotiate_rejects_unsupported_media_type() {
            let header = HeaderValue::from_static("text/plain");
            let err = super::super::negotiate_response_format(Some(&header)).unwrap_err();
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn negotiate_rejects_invalid_q_value() {
            for raw in [
                "application/json;q=2",
                "application/json;q=.5",
                "application/json;q=0.1234",
                "application/json;q=1.001",
                "application/json;q=-0.1",
                "application/json;q",
                "application/json;q=0.5;q=0.7",
            ] {
                let header = raw
                    .parse::<HeaderValue>()
                    .expect("syntactically valid header bytes");
                let err = super::super::negotiate_response_format(Some(&header))
                    .expect_err("malformed q-value must fail closed");
                assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE, "header={raw}");
            }
        }

        #[test]
        fn negotiate_accepts_exact_http_qvalue_boundaries() {
            for raw in [
                "application/json;q=0",
                "application/json;q=0.",
                "application/json;q=0.000",
                "application/json;q=0.001",
                "application/json;q=0.999",
                "application/json;q=1",
                "application/json;q=1.",
                "application/json;q=1.000",
            ] {
                let header = raw
                    .parse::<HeaderValue>()
                    .expect("syntactically valid header bytes");
                let result = super::super::negotiate_response_format(Some(&header));
                if matches!(
                    raw,
                    "application/json;q=0" | "application/json;q=0." | "application/json;q=0.000"
                ) {
                    assert!(result.is_err(), "zero quality forbids the only range");
                } else {
                    assert_eq!(
                        result.expect("valid q-value"),
                        super::super::ResponseFormat::Json,
                        "header={raw}"
                    );
                }
            }
        }

        #[test]
        fn accept_parser_keeps_quoted_commas_and_escapes_inside_one_entry() {
            let header = HeaderValue::from_static(
                r#"application/json;profile="a,b\"c";q=0.4, application/x-norito;q=0.8;note="x,y""#,
            );
            let format = super::super::negotiate_response_format(Some(&header))
                .expect("quoted comma must not split an Accept entry");
            assert_eq!(format, super::super::ResponseFormat::Norito);

            let header =
                HeaderValue::from_static(r#"application/json;q=0.8;note="one,two\\three""#);
            let format = super::super::negotiate_response_format(Some(&header))
                .expect("valid quoted Accept extension");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn accept_parser_rejects_malformed_or_duplicate_parameters() {
            for raw in [
                "application/json;profile",
                "application/json;=value",
                "application/json;profile=",
                "application/json;profile =value",
                "application/json;profile= value",
                "application/json;profile=one;PROFILE=two",
                "application/json;",
                "application/json;;q=1",
                "application/json q=1",
                "application/json;q=1;Q=0.5",
                "*/json",
                "application/*+json",
                "application/json*",
            ] {
                let header = HeaderValue::from_str(raw).expect("valid header field bytes");
                let error = super::super::negotiate_response_format(Some(&header))
                    .expect_err("malformed parameter grammar must fail closed");
                assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE, "header={raw}");
            }
        }

        #[test]
        fn accept_parser_rejects_bad_quotes_and_quoted_qvalues() {
            for raw in [
                r#"application/json;profile="unterminated"#,
                r#"application/json;profile="closed"trailing"#,
                r#"application/json;profile="trailing\""#,
                r#"application/json;q="0.5""#,
            ] {
                let header = HeaderValue::from_str(raw).expect("valid header field bytes");
                let error = super::super::negotiate_response_format(Some(&header))
                    .expect_err("invalid quoted-string grammar must fail closed");
                assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE, "header={raw}");
            }

            for raw in [
                "application/json;profile=\"bad\\\u{7f}\"",
                "application/json;profile=\"bad\u{7f}\"",
                "application/json;profile=\"bad\rvalue\"",
                "application/json;profile=\"välue\"",
            ] {
                assert!(
                    super::super::parse_accept_ranges(raw).is_err(),
                    "strict ASCII quoted-string parser must reject {raw:?}"
                );
            }
        }

        #[test]
        fn accept_parser_rejects_empty_list_members() {
            for raw in [
                "",
                " ",
                ",application/json",
                "application/json,",
                "application/json,,application/x-norito",
                "application/json, \t , application/x-norito",
            ] {
                let header = HeaderValue::from_str(raw).expect("valid header field bytes");
                let error = super::super::negotiate_response_format(Some(&header))
                    .expect_err("empty Accept list members must fail closed");
                assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE, "header={raw:?}");
            }
        }

        #[test]
        fn accept_specificity_includes_matching_media_parameters() {
            let header = HeaderValue::from_static(
                "application/json;q=0, application/json;charset=utf-8;q=0.8, application/x-norito;q=0.7",
            );
            assert_eq!(
                super::super::negotiate_response_format(Some(&header)).expect("format"),
                super::super::ResponseFormat::Json
            );

            let header = HeaderValue::from_static("application/*;q=0, */*;q=1");
            assert_eq!(
                super::super::negotiate_response_format(Some(&header))
                    .expect_err("more-specific zero must override all-type wildcard")
                    .status(),
                StatusCode::NOT_ACCEPTABLE
            );

            let header =
                HeaderValue::from_static("application/json;charset=iso-8859-1;q=0, */*;q=1");
            assert_eq!(
                super::super::negotiate_response_format(Some(&header)).expect("format"),
                super::super::ResponseFormat::Norito,
                "an incompatible charset range must not match the UTF-8 JSON representation"
            );
        }

        #[test]
        fn json_accept_charset_is_utf8_only() {
            for raw in [
                "application/json;charset=utf-8",
                "application/json;charset=UTF-8",
                "application/json;charset=\"Utf-8\"",
            ] {
                let header = HeaderValue::from_str(raw).expect("Accept header");
                assert_eq!(
                    super::super::negotiate_response_format(Some(&header)).expect("UTF-8 JSON"),
                    super::super::ResponseFormat::Json,
                    "header={raw}"
                );
            }
            for raw in [
                "application/json;charset=latin1",
                "application/json;charset=\"\"",
                "application/json;charset=utf-8;CHARSET=utf-8",
            ] {
                let header = HeaderValue::from_str(raw).expect("Accept header");
                assert_eq!(
                    super::super::negotiate_response_format(Some(&header))
                        .expect_err("unsupported or duplicate charset must fail")
                        .status(),
                    StatusCode::NOT_ACCEPTABLE,
                    "header={raw}"
                );
            }
        }

        #[test]
        fn negotiate_json_only_accepts_missing_json_and_wildcards() {
            assert!(super::super::negotiate_json_only_response(None).is_ok());
            let json = HeaderValue::from_static("application/json");
            assert!(super::super::negotiate_json_only_response(Some(&json)).is_ok());
            for raw in ["application/*", "*/*"] {
                let wildcard = HeaderValue::from_static(raw);
                assert!(
                    super::super::negotiate_json_only_response(Some(&wildcard)).is_ok(),
                    "header={raw}"
                );
            }

            let unrelated_zero =
                HeaderValue::from_static("application/problem+json;q=0, application/*;q=0.7");
            assert!(
                super::super::negotiate_json_only_response(Some(&unrelated_zero)).is_ok(),
                "a concrete suffix rejection must not override the matching type wildcard"
            );
        }

        #[test]
        fn negotiate_json_only_rejects_nonmatching_concrete_types() {
            for raw in ["application/x-norito", "application/problem+json"] {
                let header = HeaderValue::from_static(raw);
                let err = super::super::negotiate_json_only_response(Some(&header))
                    .expect_err("a JSON-only route emits application/json exactly");
                assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE, "header={raw}");
            }
        }

        #[test]
        fn negotiate_json_only_exact_zero_overrides_wildcard() {
            let header = HeaderValue::from_static("application/json;q=0, */*;q=1");
            let err = super::super::negotiate_json_only_response(Some(&header)).unwrap_err();
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);

            let header = HeaderValue::from_static(
                "application/problem+json;q=1, application/json;q=0, */*;q=0.5",
            );
            let err = super::super::negotiate_json_only_response(Some(&header))
                .expect_err("an unrelated suffix type must not undo exact JSON q=0");
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn negotiate_norito_only_honors_specific_zero_and_application_wildcard() {
            let wildcard = HeaderValue::from_static("application/*;q=0.7");
            assert!(super::super::negotiate_norito_only_response(Some(&wildcard)).is_ok());

            let explicit_zero = HeaderValue::from_static("application/x-norito;q=0, */*;q=1");
            let err = super::super::negotiate_norito_only_response(Some(&explicit_zero))
                .expect_err("specific q=0 must override wildcard");
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn protocol_media_negotiation_honors_type_wildcards_and_specific_zero() {
            for accepted in ["text/event-stream", "text/*", "*/*"] {
                let header = accepted.parse::<HeaderValue>().expect("Accept header");
                super::super::ensure_response_media_type_acceptable(
                    Some(&header),
                    "text/event-stream; charset=utf-8",
                )
                .expect("matching native media type");
            }
            let header = HeaderValue::from_static("text/event-stream;q=0, */*;q=1");
            let error = super::super::ensure_response_media_type_acceptable(
                Some(&header),
                "text/event-stream",
            )
            .expect_err("specific zero must override wildcard");
            assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn protocol_media_negotiation_keeps_structured_suffix_types_exact() {
            let exact = HeaderValue::from_static("application/problem+json");
            super::super::ensure_response_media_type_acceptable(
                Some(&exact),
                "application/problem+json; charset=utf-8",
            )
            .expect("the exact structured-suffix media type must match");

            let wildcard = HeaderValue::from_static("application/*");
            super::super::ensure_response_media_type_acceptable(
                Some(&wildcard),
                "application/problem+json; charset=utf-8",
            )
            .expect("an application wildcard must match a suffix representation");

            let plain_json = HeaderValue::from_static("application/json");
            assert_eq!(
                super::super::ensure_response_media_type_acceptable(
                    Some(&plain_json),
                    "application/problem+json; charset=utf-8",
                )
                .expect_err("application/json is not application/problem+json")
                .status(),
                StatusCode::NOT_ACCEPTABLE
            );

            let exact_zero =
                HeaderValue::from_static("application/problem+json;q=0, application/*;q=1");
            assert_eq!(
                super::super::ensure_response_media_type_acceptable(
                    Some(&exact_zero),
                    "application/problem+json",
                )
                .expect_err("an exact q=0 must override a positive type wildcard")
                .status(),
                StatusCode::NOT_ACCEPTABLE
            );
        }

        #[test]
        fn protocol_media_negotiation_matches_strict_quoted_parameters() {
            let header = HeaderValue::from_static(
                r#"text/event-stream;profile="one,two\"three";q=0.7, */*;q=0.1"#,
            );
            super::super::ensure_response_media_type_acceptable(
                Some(&header),
                r#"text/event-stream; charset=utf-8; profile="one,two\"three""#,
            )
            .expect("matching decoded quoted parameter");

            let mismatched =
                HeaderValue::from_static(r#"text/event-stream;profile="different,value";q=1"#);
            assert_eq!(
                super::super::ensure_response_media_type_acceptable(
                    Some(&mismatched),
                    r#"text/event-stream;profile="one,two""#,
                )
                .expect_err("mismatched media parameter must not match")
                .status(),
                StatusCode::NOT_ACCEPTABLE
            );
        }

        #[test]
        fn protocol_media_negotiation_rejects_malformed_headers_and_actual_types() {
            for raw in [
                "text/event-stream;profile",
                "text/event-stream;profile=one;PROFILE=two",
                "text/event-stream,",
            ] {
                let header = HeaderValue::from_str(raw).expect("header field bytes");
                assert_eq!(
                    super::super::ensure_response_media_type_acceptable(
                        Some(&header),
                        "text/event-stream",
                    )
                    .expect_err("malformed Accept must fail")
                    .status(),
                    StatusCode::NOT_ACCEPTABLE,
                    "header={raw}"
                );
            }

            let wildcard = HeaderValue::from_static("*/*");
            for actual in [
                "text/event-stream;profile",
                "text/event-stream;profile=one;PROFILE=two",
                "application/json;charset=latin1",
                "application/*",
            ] {
                assert_eq!(
                    super::super::ensure_response_media_type_acceptable(Some(&wildcard), actual,)
                        .expect_err("invalid actual Content-Type must fail")
                        .status(),
                    StatusCode::NOT_ACCEPTABLE,
                    "actual={actual}"
                );
                assert_eq!(
                    super::super::ensure_response_media_type_acceptable(None, actual)
                        .expect_err("invalid actual Content-Type is invalid without Accept too")
                        .status(),
                    StatusCode::NOT_ACCEPTABLE,
                    "actual={actual}"
                );
            }
        }

        #[test]
        fn protocol_media_negotiation_rejects_unrelated_media() {
            let header = HeaderValue::from_static("application/json, image/*;q=0.5");
            let error = super::super::ensure_response_media_type_acceptable(
                Some(&header),
                "application/octet-stream",
            )
            .expect_err("unrelated media ranges must not match");
            assert_eq!(error.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn typed_response_content_type_classifier_excludes_protocol_media() {
            assert_eq!(
                super::super::typed_response_format_for_content_type(
                    "application/problem+json; charset=utf-8"
                ),
                None,
                "structured-suffix responses retain their concrete media type"
            );
            assert_eq!(
                super::super::typed_response_format_for_content_type(
                    "application/x-norito; profile=torii-v1"
                ),
                Some(super::super::ResponseFormat::Norito)
            );
            for native in [
                "text/event-stream",
                "text/plain; version=0.0.4",
                "application/octet-stream",
                "image/png",
            ] {
                assert_eq!(
                    super::super::typed_response_format_for_content_type(native),
                    None,
                    "protocol media type must not enter typed negotiation: {native}"
                );
            }
        }

        #[test]
        fn typed_response_content_type_classifier_enforces_strict_syntax_and_charset() {
            for valid in ["application/json; charset=utf-8"] {
                assert_eq!(
                    super::super::typed_response_format_for_content_type(valid),
                    Some(super::super::ResponseFormat::Json),
                    "valid={valid}"
                );
            }
            for invalid in [
                "application/json; charset=latin1",
                "application/json; charset=utf-8; CHARSET=utf-8",
                "application/json; charset =utf-8",
                "application/json; charset= utf-8",
                "application/json; profile=\"unterminated",
                "application/json;q=1",
                "APPLICATION/PROBLEM+JSON; CHARSET=\"UTF-8\"; profile=torii",
                "application/*+json",
                "*/json",
            ] {
                assert_eq!(
                    super::super::typed_response_format_for_content_type(invalid),
                    None,
                    "invalid={invalid}"
                );
            }
        }

        #[test]
        fn typed_request_content_type_is_the_exact_first_release_pair() {
            for (raw, expected) in [
                (
                    "application/json",
                    super::super::TypedRequestContentFormat::Json,
                ),
                (
                    "application/json;charset=utf-8",
                    super::super::TypedRequestContentFormat::Json,
                ),
                (
                    "application/json;charset=\"UTF-8\"",
                    super::super::TypedRequestContentFormat::Json,
                ),
                (
                    super::super::NORITO_MIME_TYPE,
                    super::super::TypedRequestContentFormat::Norito,
                ),
            ] {
                let mut headers = axum::http::HeaderMap::new();
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_str(raw).expect("Content-Type"),
                );
                assert_eq!(
                    super::super::typed_request_content_format(&headers).expect("supported type"),
                    expected,
                    "content_type={raw}"
                );
            }

            for raw in [
                "application/problem+json",
                "application/json;profile=torii",
                "application/json;charset=utf-8;profile=torii",
                "application/json;charset=latin1",
                "application/x-norito;charset=utf-8",
                "application/x-norito;profile=torii-v1",
            ] {
                let mut headers = axum::http::HeaderMap::new();
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_str(raw).expect("Content-Type"),
                );
                assert_eq!(
                    super::super::typed_request_content_format(&headers)
                        .expect_err("unsupported request media parameters")
                        .status(),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "content_type={raw}"
                );
            }
        }

        #[test]
        fn kagemusha_command_content_type_is_canonical_norito_only() {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static(super::super::NORITO_MIME_TYPE),
            );
            super::super::norito_request_content_type(&headers)
                .expect("canonical Norito command media type");

            for raw in [
                "application/json",
                "application/json;charset=utf-8",
                "application/x-norito;charset=utf-8",
                "application/octet-stream",
            ] {
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_str(raw).expect("Content-Type"),
                );
                assert_eq!(
                    super::super::norito_request_content_type(&headers)
                        .expect_err("Kagemusha commands have one wire representation")
                        .status(),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "content_type={raw}"
                );
            }
        }

        #[cfg(feature = "app_api")]
        #[test]
        fn offline_norito_decoder_accepts_only_the_canonical_layout() {
            let value = vec![3_u64, 5, 8, 13, 21];
            let canonical = norito::encode_canonical(&value).expect("encode canonical fixture");
            assert_eq!(
                decode_offline_canonical_norito::<Vec<u64>>(&canonical)
                    .expect("decode canonical fixture"),
                value
            );

            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let alternate = {
                let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
                norito::to_bytes(&value).expect("encode alternate-layout fixture")
            };
            assert_ne!(alternate, canonical);
            assert!(matches!(
                decode_offline_canonical_norito::<Vec<u64>>(&alternate),
                Err(OfflineCanonicalNoritoDecodeError::Norito(
                    norito::Error::NonCanonicalEncoding
                ))
            ));

            let compressed =
                norito::to_compressed_bytes(&value, Some(norito::CompressionConfig::default()))
                    .expect("encode compressed fixture");
            assert!(matches!(
                decode_offline_canonical_norito::<Vec<u64>>(&compressed),
                Err(OfflineCanonicalNoritoDecodeError::Norito(
                    norito::Error::NonCanonicalEncoding
                ))
            ));

            let mut trailing = canonical;
            trailing.push(0);
            assert!(matches!(
                decode_offline_canonical_norito::<Vec<u64>>(&trailing),
                Err(OfflineCanonicalNoritoDecodeError::Norito(_))
            ));
        }

        #[cfg(feature = "app_api")]
        #[test]
        fn offline_norito_decoder_rejects_forged_counts_before_allocation() {
            const FORGED_LENGTH: u64 = 1 << 40;
            let frame = norito::core::frame_bare_with_header_flags::<Vec<u64>>(
                &FORGED_LENGTH.to_le_bytes(),
                norito::core::default_encode_flags(),
            )
            .expect("frame forged count with a valid checksum");

            assert!(matches!(
                decode_offline_canonical_norito::<Vec<u64>>(&frame),
                Err(OfflineCanonicalNoritoDecodeError::Norito(
                    norito::Error::SequenceLengthExceeded { .. }
                        | norito::Error::TotalElementsExceeded { .. }
                        | norito::Error::TotalAllocationExceeded { .. }
                ))
            ));

            // Keep the forged count within the per-sequence and cumulative
            // element limits, but make the requested `u64` backing allocation
            // exceed the production fourfold byte budget. The decoder must
            // reject before attempting to read or allocate those elements.
            const ALLOCATION_COUNT: u64 = 128;
            let mut allocation_payload = ALLOCATION_COUNT.to_le_bytes().to_vec();
            allocation_payload.resize(allocation_payload.len() + ALLOCATION_COUNT as usize, 0);
            let allocation_frame = norito::core::frame_bare_with_header_flags::<Vec<u64>>(
                &allocation_payload,
                norito::core::default_encode_flags(),
            )
            .expect("frame forged allocation with a valid checksum");
            assert!(matches!(
                decode_offline_canonical_norito::<Vec<u64>>(&allocation_frame),
                Err(OfflineCanonicalNoritoDecodeError::Norito(
                    norito::Error::TotalAllocationExceeded { .. }
                ))
            ));
        }

        #[cfg(feature = "app_api")]
        #[test]
        fn offline_norito_body_caps_are_exact_and_fail_one_byte_over() {
            assert_eq!(
                <iroha_torii_shared::offline_api::OfflineRecipientLineageRequest as OfflineCanonicalNoritoSchema>::MAX_BODY_BYTES,
                32 * 1024,
                "the lineage extractor and route share the exact 32 KiB protocol cap"
            );
            for maximum in [
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_TOPUP_REQUEST_MAX_BYTES_V4,
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
            ] {
                assert!(
                    validate_offline_canonical_norito_body_len(maximum, maximum).is_ok(),
                    "the exact protocol body cap must be accepted"
                );
                assert!(matches!(
                    validate_offline_canonical_norito_body_len(maximum + 1, maximum),
                    Err(OfflineCanonicalNoritoDecodeError::TooLarge {
                        actual,
                        maximum: rejected_maximum
                    }) if actual == maximum + 1 && rejected_maximum == maximum
                ));
            }
            assert!(matches!(
                validate_offline_canonical_norito_body_len(0, 1),
                Err(OfflineCanonicalNoritoDecodeError::Empty)
            ));
        }

        #[cfg(feature = "app_api")]
        #[test]
        fn offline_norito_max_shaped_schema_archives_fit_the_schema_allocation_budget() {
            use iroha_torii_shared::offline_api::{
                OfflineRecipientLineageRequest, OfflineRecipientLineageSelectorV2,
            };

            fn assert_bounded_roundtrip<T>(value: &T)
            where
                T: OfflineCanonicalNoritoSchema + core::fmt::Debug + PartialEq,
            {
                let canonical =
                    norito::encode_canonical(value).expect("encode canonical Offline DTO");
                assert!(
                    canonical.len() <= T::MAX_BODY_BYTES,
                    "representative Offline DTO exceeds its exact route cap"
                );
                let limits = T::decode_limits(canonical.len());
                assert_eq!(
                    limits.max_total_allocated_bytes(),
                    canonical
                        .len()
                        .saturating_mul(
                            OFFLINE_CANONICAL_TOTAL_ALLOCATION_MULTIPLIER
                                .saturating_add(T::EXTRA_ENCODED_ALLOCATION_MULTIPLIER),
                        )
                        .saturating_add(T::FIXED_ALLOCATION_ALLOWANCE_BYTES)
                );
                let decoded = decode_offline_canonical_norito::<T>(&canonical)
                    .expect("real Offline DTO must fit its decode budget");
                assert_eq!(&decoded, value);
            }

            let mut top_up = offline_ingress_top_up_fixture();
            top_up.authorization.device_id = "d".repeat(128);
            top_up.artifact_binding.generation = "g".repeat(128);
            top_up.shield_evidence.proof.proof.bytes =
                vec![0x71; iroha_data_model::offline::KAGEMUSHA_TOPUP_SHIELD_MAX_PROOF_BYTES_V2];
            assert_bounded_roundtrip(&top_up);

            let redeem = offline_ingress_max_shaped_redeem_fixture(&top_up);
            assert_bounded_roundtrip(&redeem);

            let lineage = OfflineRecipientLineageRequest {
                version: iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_LINEAGE_VERSION,
                selector: OfflineRecipientLineageSelectorV2 {
                    chain_id: top_up.current_note.chain_id.clone(),
                    recipient: top_up.authorization.authority.clone(),
                    receiver_device_id: "d".repeat(128),
                    asset: top_up.current_note.asset.clone(),
                },
                trusted_checkpoint_height: 1,
            };
            assert_bounded_roundtrip(&lineage);
        }

        #[cfg(feature = "app_api")]
        #[tokio::test]
        async fn offline_norito_http_extractor_accepts_max_shaped_redeem_and_rejects_compression() {
            use iroha_torii_shared::offline_api::OfflineRedeemRequest;

            let top_up = offline_ingress_top_up_fixture();
            let redeem = offline_ingress_max_shaped_redeem_fixture(&top_up);
            let canonical =
                norito::encode_canonical(&redeem).expect("encode maximum-shaped redeem request");
            assert!(
                canonical.len()
                    <= <OfflineRedeemRequest as OfflineCanonicalNoritoSchema>::MAX_BODY_BYTES
            );

            let canonical_request = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(canonical))
                .expect("canonical HTTP request");
            let extracted =
                OfflineNorito::<OfflineRedeemRequest>::from_request(canonical_request, &())
                    .await
                    .expect("maximum-shaped canonical redeem must pass the HTTP extractor");
            assert_eq!(extracted.0, redeem);

            let mut oversized_unshield = redeem.clone();
            oversized_unshield.redeem_proof.proof.bytes.push(0x77);
            let oversized_request = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(
                    norito::encode_canonical(&oversized_unshield)
                        .expect("encode oversized unshield request"),
                ))
                .expect("oversized unshield HTTP request");
            let rejection =
                OfflineNorito::<OfflineRedeemRequest>::from_request(oversized_request, &())
                    .await
                    .expect_err("wire preflight must reject an oversized unshield proof");
            assert_eq!(rejection.status(), StatusCode::BAD_REQUEST);

            // A behavior assertion is deterministic under parallel tests,
            // unlike a process-wide RSS threshold. The canonical decoder's
            // fixed-header preflight guarantees this rejection precedes
            // decompression or its advertised allocation.
            let compressed =
                norito::to_compressed_bytes(&redeem, Some(norito::CompressionConfig::default()))
                    .expect("encode compressed redeem fixture");
            let compressed_request = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(compressed))
                .expect("compressed HTTP request");
            let rejection =
                OfflineNorito::<OfflineRedeemRequest>::from_request(compressed_request, &())
                    .await
                    .expect_err("compressed redeem must fail at the HTTP extractor");
            assert_eq!(rejection.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn typed_request_content_type_errors_have_exact_status_and_code() {
            for (raw, expected_status, expected_code) in [
                (
                    "application/json;profile=\"unterminated",
                    StatusCode::BAD_REQUEST,
                    "request_content_type_invalid",
                ),
                (
                    "application/json;charset=utf-8;CHARSET=utf-8",
                    StatusCode::BAD_REQUEST,
                    "request_content_type_invalid",
                ),
                (
                    "application/json;charset =utf-8",
                    StatusCode::BAD_REQUEST,
                    "request_content_type_invalid",
                ),
                (
                    "application/*",
                    StatusCode::BAD_REQUEST,
                    "request_content_type_invalid",
                ),
                (
                    "application/problem+json",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    "application/json;profile=torii",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    "application/json;charset=latin1",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    "application/x-norito;profile=torii-v1",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
            ] {
                let mut headers = axum::http::HeaderMap::new();
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_str(raw).expect("Content-Type field bytes"),
                );
                let response = super::super::with_current_response_format(
                    super::super::ResponseFormat::Json,
                    async {
                        super::super::typed_request_content_format(&headers)
                            .expect_err("invalid or unsupported request Content-Type")
                    },
                )
                .await;
                assert_eq!(response.status(), expected_status, "content_type={raw}");
                let body = response
                    .into_body()
                    .collect()
                    .await
                    .expect("collect Content-Type error")
                    .to_bytes();
                let envelope: iroha_torii_shared::ErrorEnvelope =
                    norito::json::from_slice(&body).expect("decode Content-Type error");
                assert_eq!(envelope.code(), expected_code, "content_type={raw}");
            }

            let missing_headers = axum::http::HeaderMap::new();
            let missing = super::super::with_current_response_format(
                super::super::ResponseFormat::Json,
                async {
                    super::super::typed_request_content_format(&missing_headers)
                        .expect_err("missing Content-Type")
                },
            )
            .await;
            assert_eq!(missing.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
            let body = missing
                .into_body()
                .collect()
                .await
                .expect("collect missing Content-Type error")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode missing Content-Type error");
            assert_eq!(envelope.code(), "request_content_type_missing");

            let mut duplicate_headers = axum::http::HeaderMap::new();
            duplicate_headers.append(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            duplicate_headers.append(
                CONTENT_TYPE,
                HeaderValue::from_static(super::super::NORITO_MIME_TYPE),
            );
            let duplicate = super::super::with_current_response_format(
                super::super::ResponseFormat::Json,
                async {
                    super::super::typed_request_content_format(&duplicate_headers)
                        .expect_err("duplicate Content-Type")
                },
            )
            .await;
            assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
            let body = duplicate
                .into_body()
                .collect()
                .await
                .expect("collect duplicate Content-Type error")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode duplicate Content-Type error");
            assert_eq!(envelope.code(), "request_content_type_invalid");
        }

        #[tokio::test]
        async fn norito_json_accepts_binary_body() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::to_bytes(&Payload { value: 42 }).expect("norito encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect("extract norito");
            assert_eq!(extracted.0.value, 42);
        }

        #[tokio::test]
        async fn norito_json_accepts_json_body() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::json::to_vec(&Payload { value: 7 }).expect("json encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect("extract json");
            assert_eq!(extracted.0.value, 7);
        }

        #[tokio::test]
        async fn norito_json_body_limit_uses_typed_error_envelope() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::json::to_vec(&Payload { value: 7 }).expect("json encode");
            for (format, expected_content_type) in [
                (
                    super::super::ResponseFormat::Json,
                    super::super::JSON_MIME_TYPE,
                ),
                (
                    super::super::ResponseFormat::Norito,
                    super::super::NORITO_MIME_TYPE,
                ),
            ] {
                let mut req = Request::builder()
                    .method("POST")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body_bytes.clone()))
                    .expect("build request");
                DefaultBodyLimit::max(4).apply(&mut req);
                let response = super::super::with_current_response_format(
                    format,
                    NoritoJson::<Payload>::from_request(req, &()),
                )
                .await
                .expect_err("oversized typed request must fail");

                assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
                assert_eq!(
                    response.headers().get(CONTENT_TYPE),
                    Some(&HeaderValue::from_static(expected_content_type))
                );
                let body = http_body_util::BodyExt::collect(response.into_body())
                    .await
                    .expect("collect typed body-limit response")
                    .to_bytes();
                let envelope: iroha_torii_shared::ErrorEnvelope =
                    match format {
                        super::super::ResponseFormat::Json => norito::json::from_slice(&body)
                            .expect("decode JSON body-limit envelope"),
                        super::super::ResponseFormat::Norito => norito::decode_from_bytes(&body)
                            .expect("decode Norito body-limit envelope"),
                    };
                assert_eq!(envelope.code(), "request_payload_too_large");
            }
        }

        #[tokio::test]
        async fn json_only_accepts_charset_and_suffix_json() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::json::to_vec(&Payload { value: 9 }).expect("json encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(body_bytes.clone()))
                .expect("build request");
            let extracted = JsonOnly::<Payload>::from_request(req, &())
                .await
                .expect("extract json");
            assert_eq!(extracted.0.value, 9);

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/ld+json")
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = JsonOnly::<Payload>::from_request(req, &())
                .await
                .expect("extract json suffix");
            assert_eq!(extracted.0.value, 9);
        }

        #[tokio::test]
        async fn canonical_json_only_accepts_exact_json_representations() {
            #[derive(Clone, Debug, PartialEq, crate::json_macros::JsonDeserialize)]
            struct Payload {
                value: u32,
            }

            for content_type in [
                "application/json",
                "application/json;charset=utf-8",
                "application/json; charset=\"UTF-8\"",
            ] {
                let req = Request::builder()
                    .method("POST")
                    .header(CONTENT_TYPE, content_type)
                    .body(Body::from(r#"{"value":9}"#))
                    .expect("build canonical JSON request");
                let extracted = CanonicalJsonOnly::<Payload>::from_request(req, &())
                    .await
                    .expect("extract canonical JSON");
                assert_eq!(extracted.0.value, 9, "content_type={content_type}");
            }
        }

        #[tokio::test]
        async fn canonical_json_only_rejects_noncanonical_media_before_body_decode() {
            #[derive(Clone, Debug, PartialEq, crate::json_macros::JsonDeserialize)]
            struct Payload {
                value: u32,
            }

            for (content_type, expected_status, expected_code) in [
                (
                    Some("application/x-norito"),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    Some("application/problem+json"),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    Some("application/json;profile=torii"),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_unsupported",
                ),
                (
                    Some("application/json;charset=utf-8;CHARSET=utf-8"),
                    StatusCode::BAD_REQUEST,
                    "request_content_type_invalid",
                ),
                (
                    None,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "request_content_type_missing",
                ),
            ] {
                let mut builder = Request::builder().method("POST");
                if let Some(content_type) = content_type {
                    builder = builder.header(CONTENT_TYPE, content_type);
                }
                let request = builder
                    .body(Body::from("not a JSON body"))
                    .expect("build rejected request");
                let response = super::super::with_current_response_format(
                    super::super::ResponseFormat::Json,
                    CanonicalJsonOnly::<Payload>::from_request(request, &()),
                )
                .await
                .expect_err("noncanonical media must be rejected");
                assert_eq!(
                    response.status(),
                    expected_status,
                    "content_type={content_type:?}"
                );
                let body = response
                    .into_body()
                    .collect()
                    .await
                    .expect("collect canonical JSON rejection")
                    .to_bytes();
                let envelope: iroha_torii_shared::ErrorEnvelope =
                    norito::json::from_slice(&body).expect("decode canonical JSON rejection");
                assert_eq!(
                    envelope.code(),
                    expected_code,
                    "content_type={content_type:?}"
                );
            }

            let mut duplicate = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from("not a JSON body"))
                .expect("build duplicate Content-Type request");
            duplicate
                .headers_mut()
                .append(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let response = super::super::with_current_response_format(
                super::super::ResponseFormat::Json,
                CanonicalJsonOnly::<Payload>::from_request(duplicate, &()),
            )
            .await
            .expect_err("duplicate Content-Type must be rejected");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = response
                .into_body()
                .collect()
                .await
                .expect("collect duplicate Content-Type rejection")
                .to_bytes();
            let envelope: iroha_torii_shared::ErrorEnvelope =
                norito::json::from_slice(&body).expect("decode duplicate Content-Type rejection");
            assert_eq!(envelope.code(), "request_content_type_invalid");
        }

        #[tokio::test]
        async fn norito_json_rejects_unsupported_content_type() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "text/plain")
                .body(Body::from("hello"))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("unsupported content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn norito_json_rejects_missing_content_type() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let req = Request::builder()
                .method("POST")
                .body(Body::from("{}"))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("missing content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn norito_json_rejects_invalid_content_type_before_body_collection() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let mut duplicate = Request::builder()
                .method("POST")
                .body(Body::from("oversized"))
                .expect("build duplicate-header request");
            duplicate
                .headers_mut()
                .append(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            duplicate.headers_mut().append(
                CONTENT_TYPE,
                HeaderValue::from_static(super::super::NORITO_MIME_TYPE),
            );
            DefaultBodyLimit::max(1).apply(&mut duplicate);
            let duplicate_error = NoritoJson::<Payload>::from_request(duplicate, &())
                .await
                .expect_err("duplicate Content-Type must fail");
            assert_eq!(duplicate_error.status(), StatusCode::BAD_REQUEST);

            let mut non_ascii = Request::builder()
                .method("POST")
                .header(
                    CONTENT_TYPE,
                    HeaderValue::from_bytes(&[0x80]).expect("opaque header value"),
                )
                .body(Body::from("oversized"))
                .expect("build non-ASCII-header request");
            DefaultBodyLimit::max(1).apply(&mut non_ascii);
            let non_ascii_error = NoritoJson::<Payload>::from_request(non_ascii, &())
                .await
                .expect_err("non-ASCII Content-Type must fail");
            assert_eq!(non_ascii_error.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn norito_json_rejects_malformed_or_non_utf8_media_before_body_collection() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            for (content_type, expected_status) in [
                (
                    "application/json; charset=latin1",
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ),
                (
                    "application/json; charset=utf-8; CHARSET=utf-8",
                    StatusCode::BAD_REQUEST,
                ),
                ("application/json; charset =utf-8", StatusCode::BAD_REQUEST),
                (
                    "application/json; profile=\"unterminated",
                    StatusCode::BAD_REQUEST,
                ),
                ("application/json;q=1", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            ] {
                let mut request = Request::builder()
                    .method("POST")
                    .header(CONTENT_TYPE, content_type)
                    .body(Body::from("oversized"))
                    .expect("request");
                DefaultBodyLimit::max(1).apply(&mut request);
                let error = NoritoJson::<Payload>::from_request(request, &())
                    .await
                    .expect_err("invalid media type must fail before body collection");
                assert_eq!(
                    error.status(),
                    expected_status,
                    "content_type={content_type}"
                );
            }
        }

        #[tokio::test]
        async fn norito_json_with_bytes_rejects_media_type_before_body_collection() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let mut request = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "text/plain")
                .body(Body::from("oversized"))
                .expect("build unsupported-media request");
            DefaultBodyLimit::max(1).apply(&mut request);
            let error = NoritoJsonWithBytes::<Payload>::from_request(request, &())
                .await
                .expect_err("unsupported Content-Type must fail");
            assert_eq!(error.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn norito_json_rejects_octet_stream_fallback() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let body = norito::to_bytes(&Payload).expect("encode norito payload");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(body))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("octet-stream content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[cfg(feature = "telemetry")]
        #[tokio::test]
        async fn norito_json_decode_failure_increments_metric() {
            use iroha_telemetry::metrics::global_or_default;

            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body = vec![0_u8; 4];
            let reason = super::classify_norito_error(
                &norito::decode_from_bytes::<Payload>(&body).expect_err("body must fail"),
            );
            let payload_kind = super::payload_kind_label::<Payload>();
            let metrics = global_or_default();
            let before = metrics
                .torii_norito_decode_failures_total
                .with_label_values(&[payload_kind, reason])
                .get();

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(body))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("decode must fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);

            let after = metrics
                .torii_norito_decode_failures_total
                .with_label_values(&[payload_kind, reason])
                .get();
            assert_eq!(after, before + 1, "decode metric should increment");
        }
    }
}
