//! Exact-network Iroha Connect MCP request construction.
use base64::Engine as _;
use norito::json::{Map, Value};
pub(super) fn build_connect_session_create_body(arguments: &Map) -> Result<Value, String> {
    for retired in [
        "sid",
        "session_id",
        "body",
        "chain_id",
        "chainId",
        "node_url",
    ] {
        if arguments.contains_key(retired) {
            return Err(format!(
                "`{retired}` is not accepted by the Connect V1 hard cut; provide exact `network_id`, `app_pk`, and `nonce`"
            ));
        }
    }
    let network_id_literal = required_string(arguments, "network_id")?;
    let network_id = network_id_literal
        .parse::<iroha_data_model::NetworkId>()
        .map_err(|error| format!("`network_id` must be a canonical NetworkId: {error}"))?;
    if network_id.to_string() != network_id_literal {
        return Err(
            "`network_id` must use the canonical checksummed NetworkId spelling".to_owned(),
        );
    }
    let app_pk: [u8; 32] = decode_canonical(arguments, "app_pk", 32)?
        .try_into()
        .map_err(|_| "validated Connect app key had the wrong length".to_owned())?;
    let nonce: [u8; 16] = decode_canonical(arguments, "nonce", 16)?
        .try_into()
        .map_err(|_| "validated Connect nonce had the wrong length".to_owned())?;
    let sid = iroha_torii_shared::connect_sdk::derive_session_id(&network_id, &app_pk, &nonce);
    let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let mut payload = Map::new();
    payload.insert("sid".into(), Value::String(encode(&sid)));
    payload.insert(
        "network_id".into(),
        Value::String(network_id_literal.to_owned()),
    );
    payload.insert("app_pk".into(), Value::String(encode(&app_pk)));
    payload.insert("nonce".into(), Value::String(encode(&nonce)));
    if let Some(node) = arguments.get("node") {
        let node = node
            .as_str()
            .ok_or_else(|| "`node` must be a string".to_owned())?;
        if node.is_empty() || node.trim() != node {
            return Err(
                "`node` must be non-empty exact text without surrounding whitespace".to_owned(),
            );
        }
        payload.insert("node".into(), Value::String(node.to_owned()));
    }
    Ok(Value::Object(payload))
}
pub(super) fn required_string<'a>(arguments: &'a Map, field: &str) -> Result<&'a str, String> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{field}` is required and must be a string"))?;
    if value.is_empty() || value.trim() != value {
        return Err(format!(
            "`{field}` must be non-empty canonical text without surrounding whitespace"
        ));
    }
    Ok(value)
}
pub(super) fn decode_canonical(
    arguments: &Map,
    field: &str,
    expected_len: usize,
) -> Result<Vec<u8>, String> {
    let literal = required_string(arguments, field)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(literal)
        .map_err(|_| format!("`{field}` must be canonical base64url without padding"))?;
    if bytes.len() != expected_len
        || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes) != literal
    {
        return Err(format!(
            "`{field}` must be canonical base64url for exactly {expected_len} bytes"
        ));
    }
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("`{field}` must not be all zero"));
    }
    Ok(bytes)
}
