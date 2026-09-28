use iroha_torii_shared::canonical_request_form::{
    CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1, CanonicalFormError, CanonicalRequestExactWriter,
    CanonicalRequestFormPlan, canonical_request_decimal_len, canonical_request_query_pair_count,
    write_canonical_request_decimal,
};

/// Validate `raw` against the V1 query limits and plan its canonical form.
fn canonical_request_form_plan(raw: &str) -> Result<CanonicalRequestFormPlan<'_>> {
    validate_canonical_request_raw_query(raw)?;
    CanonicalRequestFormPlan::new(raw).map_err(|error| match error {
        CanonicalFormError::TooManyPairs => canonical_request_pair_limit_error(),
        CanonicalFormError::Capacity => canonical_request_capacity_error(),
    })
}

fn allocate_exact_canonical_request_bytes(length: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| eyre!("failed to reserve {length} canonical request bytes"))?;
    bytes.resize(length, 0);
    Ok(bytes)
}

#[cfg(test)]
fn canonical_query_string_v1(raw: Option<&str>) -> Result<String> {
    let plan = canonical_request_form_plan(raw.unwrap_or_default())?;
    let mut output = allocate_exact_canonical_request_bytes(plan.encoded_bytes())?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    plan.write_to(&mut writer);
    debug_assert_eq!(writer.offset(), plan.encoded_bytes());
    String::from_utf8(output).map_err(|_| eyre!("canonical request query is not valid UTF-8"))
}

fn validate_canonical_request_raw_query(raw: &str) -> Result<()> {
    if raw.len() > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 {
        return Err(eyre!(
            "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1} raw bytes"
        ));
    }
    if canonical_request_query_pair_count(raw) > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 {
        return Err(canonical_request_pair_limit_error());
    }
    Ok(())
}

fn canonical_request_pair_limit_error() -> eyre::Report {
    eyre!(
        "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs"
    )
}

fn validate_canonical_request_target(method: &HttpMethod, url: &Url) -> Result<()> {
    if method.as_str().len() > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 {
        return Err(eyre!(
            "canonical request method exceeds the V1 limit of {CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} bytes"
        ));
    }
    if url.path().len() > CANONICAL_REQUEST_MAX_PATH_BYTES_V1 {
        return Err(eyre!(
            "canonical request path exceeds the V1 limit of {CANONICAL_REQUEST_MAX_PATH_BYTES_V1} bytes"
        ));
    }
    validate_canonical_request_raw_query(url.query().unwrap_or_default())
}

fn canonical_request_capacity_error() -> eyre::Report {
    eyre!("canonical request byte length exceeds platform capacity")
}

fn canonical_request_nonce_is_valid(nonce: &str) -> bool {
    !nonce.is_empty()
        && nonce.len() <= CANONICAL_REQUEST_MAX_NONCE_BYTES_V1
        && nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn bounded_network_request_message(
    domain: &[u8],
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
    freshness: Option<(u64, &str)>,
) -> Result<Vec<u8>> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    validate_canonical_request_target(method, url)?;
    if let Some((_, nonce)) = freshness
        && !canonical_request_nonce_is_valid(nonce)
    {
        return Err(eyre!("invalid canonical request nonce"));
    }
    let query = canonical_request_form_plan(url.query().unwrap_or_default())?;
    let freshness_bytes = if let Some((timestamp_ms, nonce)) = freshness {
        1_usize
            .checked_add(canonical_request_decimal_len(timestamp_ms))
            .and_then(|length| length.checked_add(1))
            .and_then(|length| length.checked_add(nonce.len()))
            .ok_or_else(canonical_request_capacity_error)?
    } else {
        0
    };
    let total_bytes = domain
        .len()
        .checked_add(network_id.as_bytes().len())
        .and_then(|length| length.checked_add(method.as_str().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(url.path().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(query.encoded_bytes()))
        .and_then(|length| length.checked_add(1 + 64))
        .and_then(|length| length.checked_add(freshness_bytes))
        .ok_or_else(canonical_request_capacity_error)?;
    let mut output = allocate_exact_canonical_request_bytes(total_bytes)?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    writer.extend(domain);
    writer.extend(network_id.as_bytes());
    for byte in method.as_str().bytes() {
        writer.push(byte.to_ascii_uppercase());
    }
    writer.push(b'\n');
    writer.extend(url.path().as_bytes());
    writer.push(b'\n');
    query.write_to(&mut writer);
    writer.push(b'\n');
    let body_hash = Sha256::digest(body);
    for byte in body_hash {
        writer.push(HEX[usize::from(byte >> 4)]);
        writer.push(HEX[usize::from(byte & 0x0f)]);
    }
    if let Some((timestamp_ms, nonce)) = freshness {
        writer.push(b'\n');
        write_canonical_request_decimal(timestamp_ms, &mut writer);
        writer.push(b'\n');
        writer.extend(nonce.as_bytes());
    }
    debug_assert_eq!(writer.offset(), total_bytes);
    Ok(output)
}

/// Construct exact-network canonical V1 request bytes for signing or hashing.
///
/// The envelope binds the V1 domain and exact genesis-derived network, an
/// uppercase method, percent-encoded path, canonical query, and lowercase
/// SHA-256 body digest. Query components are form-decoded (`+` is space),
/// compared as lossy UTF-8 `(key, value)` pairs, and form-encoded with only
/// ASCII alphanumerics plus `*`, `-`, `.`, and `_` left unescaped.
///
/// # Errors
/// Returns an error when the method, path, or query exceeds the V1 bounds or
/// the exact output allocation fails.
pub fn canonical_network_request_message(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
) -> Result<Vec<u8>> {
    bounded_network_request_message(
        b"iroha.app.request.network.v1\0",
        network_id,
        method,
        url,
        body,
        None,
    )
}

/// Hash one exact-network canonical V1 request for a multisignature witness.
///
/// # Errors
/// Returns an error when canonical request construction fails.
pub fn canonical_network_request_hash(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
) -> Result<Hash> {
    canonical_network_request_message(network_id, method, url, body)
        .map(|message| Hash::new(&message))
}

/// Construct exact-network canonical V1 request bytes with freshness metadata.
///
/// # Errors
/// Returns an error when the target or nonce exceeds the V1 bounds or the
/// exact output allocation fails.
pub fn canonical_network_request_signature_message(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Result<Vec<u8>> {
    bounded_network_request_message(
        b"iroha.app.request.network.v1\0",
        network_id,
        method,
        url,
        body,
        Some((timestamp_ms, nonce)),
    )
}

/// Render the strict ASCII account value used by canonical V1 auth headers.
///
/// # Errors
/// Returns an error when canonical address conversion fails or the resulting
/// literal exceeds the V1 account bound.
pub fn canonical_request_account_header_value(account: &AccountId) -> Result<String> {
    validate_canonical_request_account_encoded_size(account)?;
    let value = account
        .to_canonical_hex()
        .wrap_err("failed to encode canonical request account header")?;
    validate_canonical_request_account_literal(&value)?;
    Ok(value)
}

fn validate_canonical_request_account_encoded_size(account: &AccountId) -> Result<()> {
    const MAX_CANONICAL_BYTES: usize =
        (CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 - "0x".len()) / 2;
    let add = |total: &mut usize, bytes: usize| -> Result<()> {
        *total = total
            .checked_add(bytes)
            .ok_or_else(canonical_request_capacity_error)?;
        if *total > MAX_CANONICAL_BYTES {
            return Err(eyre!(
                "canonical request account exceeds the V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
            ));
        }
        Ok(())
    };
    // One byte is the address-class/domain header. The remaining lengths are
    // the exact `AccountAddress` controller wire prefixes.
    let mut canonical_bytes = 1_usize;
    match account.controller() {
        iroha_data_model::account::AccountController::Single(public_key) => {
            let (_, payload) = public_key
                .try_to_bytes()
                .wrap_err("canonical request account contains a malformed public key")?;
            add(
                &mut canonical_bytes,
                if u8::try_from(payload.len()).is_ok() {
                    3
                } else {
                    4
                },
            )?;
            add(&mut canonical_bytes, payload.len())?;
        }
        iroha_data_model::account::AccountController::Multisig(policy) => {
            add(&mut canonical_bytes, 6)?;
            for member in policy.members() {
                let (_, payload) = member
                    .public_key()
                    .try_to_bytes()
                    .wrap_err("canonical request account contains a malformed multisig key")?;
                add(&mut canonical_bytes, 5)?;
                add(&mut canonical_bytes, payload.len())?;
            }
        }
    }
    Ok(())
}

fn validate_canonical_request_account_literal(value: &str) -> Result<()> {
    if value.len() > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 {
        return Err(eyre!(
            "canonical request account exceeds the V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
        ));
    }
    let Some(payload) = value.strip_prefix("0x") else {
        return Err(eyre!(
            "canonical request account header is not lowercase ASCII hexadecimal"
        ));
    };
    if payload.is_empty()
        || payload.len() % 2 != 0
        || !value.is_ascii()
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "canonical request account header is not lowercase ASCII hexadecimal"
        ));
    }
    Ok(())
}

/// Render one canonical V1 timestamp header without an infallible allocation.
///
/// # Errors
/// Returns an error if the exact decimal destination cannot be allocated.
pub fn canonical_request_timestamp_header_value(timestamp_ms: u64) -> Result<String> {
    let length = canonical_request_decimal_len(timestamp_ms);
    let mut output = allocate_exact_canonical_request_bytes(length)?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    write_canonical_request_decimal(timestamp_ms, &mut writer);
    debug_assert_eq!(writer.offset(), length);
    String::from_utf8(output).map_err(|_| eyre!("canonical request timestamp is not valid UTF-8"))
}

fn encode_bounded_canonical_base64_value(
    bytes: &[u8],
    maximum_decoded_bytes: usize,
    context: &'static str,
) -> Result<String> {
    if bytes.len() > maximum_decoded_bytes {
        return Err(eyre!(
            "{context} exceeds the V1 limit of {maximum_decoded_bytes} decoded bytes"
        ));
    }
    let encoded_len = bytes
        .len()
        .checked_add(2)
        .map(|length| length / 3)
        .and_then(|length| length.checked_mul(4))
        .ok_or_else(canonical_request_capacity_error)?;
    let mut encoded = allocate_exact_canonical_request_bytes(encoded_len)?;
    let written = base64::engine::general_purpose::STANDARD
        .encode_slice(bytes, &mut encoded)
        .map_err(|_| eyre!("failed to encode {context} as canonical base64"))?;
    if written != encoded_len {
        return Err(eyre!("canonical base64 length mismatch for {context}"));
    }
    String::from_utf8(encoded).map_err(|_| eyre!("canonical base64 for {context} is not UTF-8"))
}

/// Encode a checked detached signature for a canonical V1 request header.
///
/// # Errors
/// Returns an error for an empty, all-zero, or excessive signature payload or
/// when the exact base64 destination cannot be allocated.
pub fn canonical_request_signature_header_value(signature: &Signature) -> Result<String> {
    let payload = signature.payload();
    if payload.is_empty()
        || payload.len() > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
        || payload.iter().all(|byte| *byte == 0)
    {
        return Err(eyre!("invalid canonical request signature"));
    }
    encode_bounded_canonical_base64_value(
        payload,
        CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1,
        "canonical request signature",
    )
}

fn validate_canonical_request_witness_for_encoding(
    witness: &CanonicalRequestWitnessV1,
) -> Result<()> {
    if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
        return Err(eyre!(
            "unsupported canonical request witness schema version"
        ));
    }
    if !canonical_request_nonce_is_valid(&witness.nonce) {
        return Err(eyre!("invalid canonical request witness nonce"));
    }
    if witness.signatures.len() > CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1 {
        return Err(eyre!(
            "canonical request witness exceeds the V1 limit of {CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1} signatures"
        ));
    }
    for signature in &witness.signatures {
        let payload = signature.signature.payload();
        if payload.is_empty()
            || payload.len() > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
            || payload.iter().all(|byte| *byte == 0)
        {
            return Err(eyre!("invalid canonical request witness signature"));
        }
    }
    Ok(())
}

/// Construct the exact canonical V1 payload signed by every request-witness member.
///
/// The signature vector is intentionally excluded: each detached signer binds the
/// subject, freshness fields, and exact canonical request hash, then an assembler
/// may add the independently produced signatures to the witness header.
///
/// # Errors
/// Returns an error for an invalid witness envelope or a failed bounded encoding.
pub fn canonical_request_witness_message(witness: &CanonicalRequestWitnessV1) -> Result<Vec<u8>> {
    validate_canonical_request_witness_for_encoding(witness)?;
    iroha_torii_shared::canonical_request_witness::encode_signing_message(
        witness,
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
    )
    .wrap_err("failed to encode bounded canonical request witness message")
}

/// Encode one bounded canonical V1 multisignature witness header.
///
/// # Errors
/// Returns an error for invalid nonce, schema, signature-count, or signature
/// payload bounds, an excessive encoded witness, or a failed exact allocation.
pub fn canonical_request_witness_header_value(
    witness: &CanonicalRequestWitnessV1,
) -> Result<String> {
    validate_canonical_request_witness_for_encoding(witness)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes =
        norito::core::to_bytes_bounded(witness, CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1)
            .wrap_err("failed to encode bounded canonical request witness")?;
    encode_bounded_canonical_base64_value(
        &bytes,
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
        "canonical request witness",
    )
}
