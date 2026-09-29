//! Borrowed syntax checks shared by Musubi text construction and revalidation.
//!
//! Revalidating decoded text must not allocate an owned replacement. These checks
//! borrow the retained text and return static parse errors on every path. They
//! preserve byte bounds and never normalize accepted text. Namespace checks borrow
//! Name's canonical syntax and profile-checked NFC owner. ICU decomposition scratch
//! is planned here and held by the State source-work caller; standalone semantic
//! callers must provide their own original-pool reservation. Cryptographic
//! verification and outer formatted errors remain separate custody requirements
//! before complete State capture can register semantic Musubi rows.

use super::{
    MUSUBI_MAX_ALIAS_BYTES_V1, MUSUBI_MAX_NAMESPACE_BYTES_V1, MUSUBI_MAX_PACKAGE_NAME_BYTES_V1,
    Name, ParseError, parse_clean,
};

// The source-work owner admits the maximum of these sequential segment checks.
// Other semantic callers still need their own original execution-pool custody.
fn namespace_segments(raw: &str) -> Result<std::str::Split<'_, char>, ParseError> {
    parse_clean(
        raw,
        "Musubi namespace must not be empty",
        "Musubi namespace is not canonical",
    )?;
    if raw.len() > MUSUBI_MAX_NAMESPACE_BYTES_V1 || raw.contains(['/', '@', ':']) {
        return Err(ParseError::new("Musubi namespace is not canonical"));
    }
    let segments = raw.split('.');
    if segments.clone().nth(2).is_some() {
        return Err(ParseError::new(
            "Musubi namespace must be `<dataspace>` or `<domain>.<dataspace>`",
        ));
    }
    Ok(segments)
}

pub(super) fn namespace(raw: &str) -> Result<(), ParseError> {
    for segment in namespace_segments(raw)? {
        Name::validate_canonical(segment)
            .map_err(|_| ParseError::new("Musubi namespace segment is invalid"))?;
    }
    Ok(())
}

pub(super) fn namespace_scratch_bytes(raw: &str) -> usize {
    // Never report an error while planning a later row. The original validator
    // will reject in its original order; these exact preliminary failures never
    // enter normalization. Segment maxima conservatively include unreachable
    // later segments if an earlier segment will fail NFC validation.
    namespace_segments(raw).map_or(0, |segments| {
        segments
            .map(Name::canonical_validation_scratch_bytes)
            .max()
            .unwrap_or(0)
    })
}

pub(super) fn package_name(raw: &str) -> Result<(), ParseError> {
    ascii_kebab(
        raw,
        MUSUBI_MAX_PACKAGE_NAME_BYTES_V1,
        "Musubi package name must be lowercase ASCII kebab text",
    )
}

pub(super) fn keyword(raw: &str) -> Result<(), ParseError> {
    ascii_kebab(raw, 64, "Musubi keyword must be lowercase ASCII kebab text")
}

pub(super) fn alias(raw: &str) -> Result<(), ParseError> {
    ascii_kebab(
        raw,
        MUSUBI_MAX_ALIAS_BYTES_V1,
        "Musubi alias must be 1-32 lowercase ASCII kebab characters",
    )
}

pub(super) fn bounded(raw: &str, maximum: usize, label: &'static str) -> Result<(), ParseError> {
    parse_clean(raw, label, label)?;
    if raw.len() > maximum {
        return Err(ParseError::new(label));
    }
    Ok(())
}

fn ascii_kebab(raw: &str, maximum: usize, label: &'static str) -> Result<(), ParseError> {
    parse_clean(raw, label, label)?;
    if raw.len() > maximum
        || raw.starts_with('-')
        || raw.ends_with('-')
        || raw.contains("--")
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ParseError::new(label));
    }
    Ok(())
}
