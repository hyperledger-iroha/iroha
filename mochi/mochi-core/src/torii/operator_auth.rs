//! Exact-network operator request signing for MOCHI's Torii client.
use super::{NORITO_MIME_TYPE, ToriiError, ToriiResult};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use iroha_crypto::{KeyPair, PublicKey, Signature, sha256};
use iroha_data_model::prelude::NetworkId;
use rand::{TryRngCore as _, rngs::OsRng};
use reqwest::{
    Client, Method, Request,
    header::{ACCEPT, HeaderValue},
};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
const OPERATOR_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha.operator.http-request.network.v1\0";
const HEADER_OPERATOR_PUBLIC_KEY: &str = "x-iroha-operator-public-key";
const HEADER_OPERATOR_TIMESTAMP_MS: &str = "x-iroha-operator-timestamp-ms";
const HEADER_OPERATOR_NONCE: &str = "x-iroha-operator-nonce";
const HEADER_OPERATOR_SIGNATURE: &str = "x-iroha-operator-signature";
/// Immutable signing material for one exact Torii network.
pub struct OperatorSigningContext {
    network_id: NetworkId,
    key_pair: KeyPair,
}
impl OperatorSigningContext {
    /// Bind an operator key pair to one genesis-derived network identity.
    #[must_use]
    pub fn new(network_id: NetworkId, key_pair: KeyPair) -> Self {
        Self {
            network_id,
            key_pair,
        }
    }
    /// Return the exact network identity covered by every request signature.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Return the public half of the configured operator key pair.
    #[must_use]
    pub fn public_key(&self) -> &PublicKey {
        self.key_pair.public_key()
    }
}
impl fmt::Debug for OperatorSigningContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperatorSigningContext")
            .field("network_id", &self.network_id)
            .field("public_key", self.key_pair.public_key())
            .finish_non_exhaustive()
    }
}
pub(super) fn build_operator_get_request(
    http: &Client,
    configured_network_id: Option<NetworkId>,
    context: Option<&OperatorSigningContext>,
    url: Url,
) -> ToriiResult<Request> {
    let context = context.ok_or_else(|| {
        ToriiError::SignedQueryContext(
            "operator signing context is required before request dispatch".to_owned(),
        )
    })?;
    let configured_network_id = configured_network_id.ok_or_else(|| {
        ToriiError::SignedQueryContext(
            "client has no exact genesis network_id configured for operator requests".to_owned(),
        )
    })?;
    if context.network_id != configured_network_id {
        return Err(ToriiError::SignedQueryContext(format!(
            "operator signing context network id `{}` does not match client network id `{configured_network_id}`",
            context.network_id
        )));
    }
    let timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ToriiError::SignedQueryContext(error.to_string()))?
        .as_millis()
        .try_into()
        .map_err(|_| {
            ToriiError::SignedQueryContext("Unix timestamp does not fit u64".to_owned())
        })?;
    let mut nonce_bytes = [0_u8; 16];
    OsRng.try_fill_bytes(&mut nonce_bytes).map_err(|error| {
        ToriiError::SignedQueryContext(format!("operator request nonce RNG failed: {error}"))
    })?;
    let nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
    let message = operator_request_message(&url, context.network_id, timestamp_ms, &nonce);
    let signature =
        Signature::try_new(context.key_pair.private_key(), &message).map_err(|error| {
            ToriiError::SignedQueryContext(format!("failed to sign operator request: {error}"))
        })?;
    http.get(url)
        .header(ACCEPT, NORITO_MIME_TYPE)
        .header(
            HEADER_OPERATOR_PUBLIC_KEY,
            operator_header_value(
                HEADER_OPERATOR_PUBLIC_KEY,
                &context.key_pair.public_key().to_string(),
            )?,
        )
        .header(
            HEADER_OPERATOR_TIMESTAMP_MS,
            operator_header_value(HEADER_OPERATOR_TIMESTAMP_MS, &timestamp_ms.to_string())?,
        )
        .header(
            HEADER_OPERATOR_NONCE,
            operator_header_value(HEADER_OPERATOR_NONCE, &nonce)?,
        )
        .header(
            HEADER_OPERATOR_SIGNATURE,
            operator_header_value(
                HEADER_OPERATOR_SIGNATURE,
                &BASE64_STANDARD.encode(signature.payload()),
            )?,
        )
        .build()
        .map_err(ToriiError::Http)
}
fn operator_header_value(name: &'static str, value: &str) -> ToriiResult<HeaderValue> {
    HeaderValue::from_str(value).map_err(|source| ToriiError::InvalidHeader {
        name: name.to_owned(),
        source,
    })
}
fn canonical_query_string(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return String::new();
    };
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    pairs.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(&key, &value);
    }
    serializer.finish()
}
fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}
fn operator_request_message(
    url: &Url,
    network_id: NetworkId,
    timestamp_ms: u64,
    nonce: &str,
) -> Vec<u8> {
    let canonical_request = format!(
        "{}\n{}\n{}\n{}",
        Method::GET.as_str(),
        url.path(),
        canonical_query_string(url.query()),
        encode_hex(&sha256(b""))
    );
    let mut message = Vec::with_capacity(
        OPERATOR_SIGNATURE_DOMAIN_V1.len()
            + network_id.as_bytes().len()
            + canonical_request.len()
            + nonce.len()
            + 32,
    );
    message.extend_from_slice(OPERATOR_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(network_id.as_bytes());
    message.extend_from_slice(canonical_request.as_bytes());
    message.push(b'\n');
    message.extend_from_slice(timestamp_ms.to_string().as_bytes());
    message.push(b'\n');
    message.extend_from_slice(nonce.as_bytes());
    message
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, ed25519_parse_signature};
    fn context() -> OperatorSigningContext {
        OperatorSigningContext::new(
            crate::torii::test_network_id(),
            KeyPair::random_with_algorithm(Algorithm::Ed25519),
        )
    }
    #[test]
    fn operator_get_signs_the_final_sorted_target_and_empty_body() {
        let context = context();
        let url =
            Url::parse("http://127.0.0.1:8080/v1/sumeragi/status?z=2&a=1").expect("valid URL");
        let request = build_operator_get_request(
            &Client::new(),
            Some(context.network_id()),
            Some(&context),
            url,
        )
        .expect("signed request");
        assert_eq!(request.method(), Method::GET);
        assert!(request.body().is_none());
        assert_eq!(request.url().query(), Some("z=2&a=1"));
        let headers = request.headers();
        assert_eq!(
            headers[HEADER_OPERATOR_PUBLIC_KEY]
                .to_str()
                .expect("public key"),
            context.public_key().to_string()
        );
        let timestamp_ms = headers[HEADER_OPERATOR_TIMESTAMP_MS]
            .to_str()
            .expect("timestamp")
            .parse::<u64>()
            .expect("u64 timestamp");
        let nonce = headers[HEADER_OPERATOR_NONCE].to_str().expect("nonce");
        let signature_bytes = BASE64_STANDARD
            .decode(headers[HEADER_OPERATOR_SIGNATURE].as_bytes())
            .expect("base64 signature");
        let signature = ed25519_parse_signature(&signature_bytes).expect("Ed25519 signature");
        let message =
            operator_request_message(request.url(), context.network_id(), timestamp_ms, nonce);
        signature
            .verify(context.public_key(), &message)
            .expect("signature covers exact request");
        assert!(
            message
                .windows(b"a=1&z=2".len())
                .any(|window| window == b"a=1&z=2")
        );
    }
}
