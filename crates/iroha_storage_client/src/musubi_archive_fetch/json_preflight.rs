//! Allocation-free structural envelope for provider-owned JSON responses.
/// Endpoint-specific upper bounds applied before an owned JSON DOM is built.
#[derive(Clone, Copy, Debug)]
pub(super) struct JsonDomEnvelopeV1 {
    /// Maximum container, scalar, and string token count.
    pub(super) tokens: usize,
    /// Maximum nested object or array depth.
    pub(super) depth: usize,
    /// Maximum encoded byte length of one JSON string.
    pub(super) single_string_bytes: usize,
    /// Maximum aggregate encoded byte length of all JSON strings.
    pub(super) total_string_bytes: usize,
    /// Maximum byte length of one unquoted scalar literal.
    pub(super) atom_bytes: usize,
}
/// Structural-envelope rejection reported before DOM allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum JsonDomPreflightErrorV1 {
    /// The byte sequence cannot form a structurally valid JSON document.
    Malformed,
    /// Container nesting exceeds the endpoint envelope.
    TooDeep,
    /// The structural token count exceeds the endpoint envelope.
    TooManyTokens,
    /// A single string or aggregate string bytes exceed the endpoint envelope.
    StringBytes,
    /// An unquoted scalar literal exceeds the endpoint envelope.
    AtomBytes,
}
/// Scan a response before Norito constructs its owned JSON value tree.
///
/// The scanner itself allocates nothing. It deliberately counts object keys as tokens because the
/// DOM owns them separately from their values. Full grammar, number, duplicate-key, and UTF-8
/// validation remains the Norito parser's job; this pass only ensures that any valid prefix it can
/// allocate is inside the endpoint-specific structural and string envelope.
#[expect(
    clippy::too_many_lines,
    reason = "the allocation-free JSON scanner keeps its state transitions in one audit surface"
)]
pub(super) fn preflight_json_dom(
    body: &[u8],
    limits: JsonDomEnvelopeV1,
) -> Result<(), JsonDomPreflightErrorV1> {
    const MAX_TRACKED_DEPTH: usize = 32;
    if limits.depth == 0 || limits.depth > MAX_TRACKED_DEPTH {
        return Err(JsonDomPreflightErrorV1::Malformed);
    }
    let mut stack = [0_u8; MAX_TRACKED_DEPTH];
    let mut depth = 0_usize;
    let mut tokens = 0_usize;
    let mut total_string_bytes = 0_usize;
    let mut index = 0_usize;
    while index < body.len() {
        match body[index] {
            b' ' | b'\n' | b'\r' | b'\t' | b':' | b',' => index += 1,
            b'{' | b'[' => {
                tokens = tokens
                    .checked_add(1)
                    .ok_or(JsonDomPreflightErrorV1::TooManyTokens)?;
                if tokens > limits.tokens {
                    return Err(JsonDomPreflightErrorV1::TooManyTokens);
                }
                depth = depth
                    .checked_add(1)
                    .ok_or(JsonDomPreflightErrorV1::TooDeep)?;
                if depth > limits.depth {
                    return Err(JsonDomPreflightErrorV1::TooDeep);
                }
                stack[depth - 1] = if body[index] == b'{' { b'}' } else { b']' };
                index += 1;
            }
            b'}' | b']' => {
                if depth == 0 || stack[depth - 1] != body[index] {
                    return Err(JsonDomPreflightErrorV1::Malformed);
                }
                depth -= 1;
                index += 1;
            }
            b'"' => {
                tokens = tokens
                    .checked_add(1)
                    .ok_or(JsonDomPreflightErrorV1::TooManyTokens)?;
                if tokens > limits.tokens {
                    return Err(JsonDomPreflightErrorV1::TooManyTokens);
                }
                index += 1;
                let string_start = index;
                loop {
                    let byte = *body.get(index).ok_or(JsonDomPreflightErrorV1::Malformed)?;
                    match byte {
                        b'"' => {
                            let encoded_bytes = index - string_start;
                            if encoded_bytes > limits.single_string_bytes {
                                return Err(JsonDomPreflightErrorV1::StringBytes);
                            }
                            total_string_bytes = total_string_bytes
                                .checked_add(encoded_bytes)
                                .ok_or(JsonDomPreflightErrorV1::StringBytes)?;
                            if total_string_bytes > limits.total_string_bytes {
                                return Err(JsonDomPreflightErrorV1::StringBytes);
                            }
                            index += 1;
                            break;
                        }
                        b'\\' => {
                            let escaped = *body
                                .get(index + 1)
                                .ok_or(JsonDomPreflightErrorV1::Malformed)?;
                            index = index
                                .checked_add(if escaped == b'u' { 6 } else { 2 })
                                .ok_or(JsonDomPreflightErrorV1::Malformed)?;
                            if index > body.len() {
                                return Err(JsonDomPreflightErrorV1::Malformed);
                            }
                        }
                        0x00..=0x1f => return Err(JsonDomPreflightErrorV1::Malformed),
                        _ => index += 1,
                    }
                }
            }
            b'-' | b'0'..=b'9' | b't' | b'f' | b'n' => {
                tokens = tokens
                    .checked_add(1)
                    .ok_or(JsonDomPreflightErrorV1::TooManyTokens)?;
                if tokens > limits.tokens {
                    return Err(JsonDomPreflightErrorV1::TooManyTokens);
                }
                let atom_start = index;
                index += 1;
                while let Some(byte) = body.get(index).copied() {
                    if matches!(byte, b' ' | b'\n' | b'\r' | b'\t' | b',' | b'}' | b']') {
                        break;
                    }
                    if matches!(byte, b'"' | b'{' | b'[' | b':') {
                        return Err(JsonDomPreflightErrorV1::Malformed);
                    }
                    index += 1;
                }
                if index - atom_start > limits.atom_bytes {
                    return Err(JsonDomPreflightErrorV1::AtomBytes);
                }
            }
            _ => return Err(JsonDomPreflightErrorV1::Malformed),
        }
    }
    if depth != 0 {
        return Err(JsonDomPreflightErrorV1::Malformed);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    const PLAN_ENVELOPE: JsonDomEnvelopeV1 = JsonDomEnvelopeV1 {
        tokens: 65_536,
        depth: 16,
        single_string_bytes: 4 * 1024,
        total_string_bytes: 4 * 1024 * 1024,
        atom_bytes: 64,
    };
    #[test]
    fn accepts_bounded_escaped_structure() {
        let body = br#"{"plan":{"files":[{"path":["src","escaped\".ko"],"size":1}],"ok":true}}"#;
        preflight_json_dom(
            body,
            JsonDomEnvelopeV1 {
                tokens: 32,
                depth: 8,
                single_string_bytes: 32,
                total_string_bytes: 128,
                atom_bytes: 64,
            },
        )
        .expect("bounded representative response");
    }
    #[test]
    fn rejects_pathological_unknown_field_before_dom() {
        let mut body = String::from("{\"unknown\":[");
        for index in 0..PLAN_ENVELOPE.tokens {
            if index != 0 {
                body.push(',');
            }
            body.push('0');
        }
        body.push_str("]}");
        assert_eq!(
            preflight_json_dom(body.as_bytes(), PLAN_ENVELOPE),
            Err(JsonDomPreflightErrorV1::TooManyTokens)
        );
    }
    #[test]
    fn rejects_depth_string_and_atom_envelopes() {
        let limits = JsonDomEnvelopeV1 {
            tokens: 16,
            depth: 3,
            single_string_bytes: 4,
            total_string_bytes: 6,
            atom_bytes: 4,
        };
        assert_eq!(
            preflight_json_dom(br"[[[[0]]]]", limits),
            Err(JsonDomPreflightErrorV1::TooDeep)
        );
        assert_eq!(
            preflight_json_dom(br#"{"abcde":0}"#, limits),
            Err(JsonDomPreflightErrorV1::StringBytes)
        );
        assert_eq!(
            preflight_json_dom(br#"{"abc":1,"defg":2}"#, limits),
            Err(JsonDomPreflightErrorV1::StringBytes)
        );
        assert_eq!(
            preflight_json_dom(br"[0]]", limits),
            Err(JsonDomPreflightErrorV1::Malformed)
        );
        assert_eq!(
            preflight_json_dom(br"[12345]", limits),
            Err(JsonDomPreflightErrorV1::AtomBytes)
        );
    }
}
