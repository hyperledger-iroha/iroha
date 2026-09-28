const REPUTATION_CANONICAL_MAX_RAW_QUERY_BYTES_V1: usize = 64 * 1024;
const REPUTATION_CANONICAL_MAX_PATH_BYTES_V1: usize = 64 * 1024;
const REPUTATION_CANONICAL_MAX_NONCE_BYTES_V1: usize = 256;

/// Validate `raw` against the reputation V1 query limits and plan its canonical form.
fn reputation_form_plan(
    raw: &str,
) -> Result<iroha_torii_shared::canonical_request_form::CanonicalRequestFormPlan<'_>, String> {
    use iroha_torii_shared::canonical_request_form::{
        CanonicalFormError, CanonicalRequestFormPlan,
    };
    if raw.len() > REPUTATION_CANONICAL_MAX_RAW_QUERY_BYTES_V1 {
        return Err("reputation request query exceeds the canonical V1 byte limit".to_owned());
    }
    CanonicalRequestFormPlan::new(raw).map_err(|error| match error {
        CanonicalFormError::TooManyPairs => {
            "reputation request query exceeds the canonical V1 pair limit".to_owned()
        }
        CanonicalFormError::Capacity => "canonical reputation query length overflow".to_owned(),
    })
}

fn canonical_reputation_request_message(
    network_id: &NetworkId,
    endpoint: &Url,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<Vec<u8>, String> {
    use iroha_torii_shared::canonical_request_form::{
        CanonicalRequestExactWriter, canonical_request_decimal_len, write_canonical_request_decimal,
    };
    const DOMAIN: &[u8] = b"iroha.app.request.network.v1\0";
    const EMPTY_BODY_HASH: &[u8; 64] =
        b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    if endpoint.path().len() > REPUTATION_CANONICAL_MAX_PATH_BYTES_V1 {
        return Err("reputation request path exceeds the canonical V1 byte limit".to_owned());
    }
    if nonce.is_empty()
        || nonce.len() > REPUTATION_CANONICAL_MAX_NONCE_BYTES_V1
        || !nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("reputation request nonce is outside canonical V1 bounds".to_owned());
    }
    let query = reputation_form_plan(endpoint.query().unwrap_or_default())?;
    let total_bytes = DOMAIN
        .len()
        .checked_add(network_id.as_bytes().len())
        .and_then(|length| length.checked_add(b"GET\n".len()))
        .and_then(|length| length.checked_add(endpoint.path().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(query.encoded_bytes()))
        .and_then(|length| length.checked_add(1 + EMPTY_BODY_HASH.len() + 1))
        .and_then(|length| length.checked_add(canonical_request_decimal_len(timestamp_ms)))
        .and_then(|length| length.checked_add(1 + nonce.len()))
        .ok_or_else(|| "canonical reputation request length overflow".to_owned())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total_bytes)
        .map_err(|_| "failed to allocate the canonical reputation request".to_owned())?;
    output.resize(total_bytes, 0);
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    writer.extend(DOMAIN);
    writer.extend(network_id.as_bytes());
    writer.extend(b"GET\n");
    writer.extend(endpoint.path().as_bytes());
    writer.push(b'\n');
    query.write_to(&mut writer);
    writer.push(b'\n');
    writer.extend(EMPTY_BODY_HASH);
    writer.push(b'\n');
    write_canonical_request_decimal(timestamp_ms, &mut writer);
    writer.push(b'\n');
    writer.extend(nonce.as_bytes());
    debug_assert_eq!(writer.offset(), total_bytes);
    Ok(output)
}
