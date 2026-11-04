//! Provider-specific JWT signing using the existing AWS-LC cryptographic backend.
//!
//! This module accepts only FCM's RSA/SHA-256 and APNs' P-256/SHA-256 signing
//! formats. Norito owns JSON; existing libraries own PEM, DER and cryptography.

use aws_lc_rs::{rand::SystemRandom, signature};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::pkcs8::der::{Decode, asn1::SequenceRef};
use rustls_pki_types::pem::{PemObject, SectionKind};
use zeroize::Zeroizing;

// The caller already bounds the complete FCM credential file to this size and
// the APNs key file more tightly. Retain a ceiling at this parsing boundary too.
const MAX_PRIVATE_KEY_PEM_BYTES: usize = 64 * 1024;

/// Sign an FCM assertion with RSA PKCS#1 v1.5 and SHA-256.
pub(super) fn encode_fcm(
    claims: &norito::json::Value,
    private_key_pem: &[u8],
) -> Result<String, String> {
    let key = rsa_key(private_key_pem)?;
    let message = signing_input("RS256", None, claims)?;
    let mut signature = vec![0; key.public_modulus_len()];
    key.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        message.as_bytes(),
        &mut signature,
    )
    .map_err(|_| "failed to sign FCM token".to_string())?;
    Ok(complete_token(message, &signature))
}

/// Sign an APNs assertion with the fixed-width P-256/SHA-256 JOSE signature.
pub(super) fn encode_apns(
    key_id: &str,
    claims: &norito::json::Value,
    private_key_pem: &[u8],
) -> Result<String, String> {
    let key = ec_key(private_key_pem)?;
    let message = signing_input("ES256", Some(key_id), claims)?;
    let signature = key
        .sign(&SystemRandom::new(), message.as_bytes())
        .map_err(|_| "failed to sign APNs token".to_string())?;
    Ok(complete_token(message, signature.as_ref()))
}

fn rsa_key(private_key_pem: &[u8]) -> Result<signature::RsaKeyPair, String> {
    let (kind, der) = private_key_der(private_key_pem)?;
    match kind {
        SectionKind::RsaPrivateKey => signature::RsaKeyPair::from_der(&der),
        SectionKind::PrivateKey => signature::RsaKeyPair::from_pkcs8(&der),
        _ => return Err("FCM requires an RSA PKCS#1 or PKCS#8 private key".into()),
    }
    .map_err(|_| "invalid FCM RSA private key".into())
}

fn ec_key(private_key_pem: &[u8]) -> Result<signature::EcdsaKeyPair, String> {
    let (kind, der) = private_key_der(private_key_pem)?;
    if kind != SectionKind::PrivateKey {
        return Err("APNs requires a P-256 PKCS#8 private key".into());
    }
    signature::EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, &der)
        .map_err(|_| "invalid APNs P-256 private key".into())
}

fn private_key_der(private_key_pem: &[u8]) -> Result<(SectionKind, Zeroizing<Vec<u8>>), String> {
    if private_key_pem.len() > MAX_PRIVATE_KEY_PEM_BYTES {
        return Err("push signing key exceeds the PEM size limit".into());
    }
    let pem = private_key_pem.trim_ascii();
    // The library decoder deliberately searches past unrelated text and resets
    // on nested BEGIN lines. Credentials must instead contain one complete key.
    // These lexical boundaries do not decode PEM or interpret ASN.1.
    let mut lines = pem
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .filter(|line| !line.trim_ascii().is_empty());
    let (begin, end) = match lines.next().map(<[u8]>::trim_ascii_end) {
        Some(b"-----BEGIN PRIVATE KEY-----") => (
            b"-----BEGIN PRIVATE KEY-----".as_slice(),
            b"-----END PRIVATE KEY-----".as_slice(),
        ),
        Some(b"-----BEGIN RSA PRIVATE KEY-----") => (
            b"-----BEGIN RSA PRIVATE KEY-----".as_slice(),
            b"-----END RSA PRIVATE KEY-----".as_slice(),
        ),
        _ => return Err("unsupported push signing key PEM label".into()),
    };
    if lines.next_back().map(<[u8]>::trim_ascii_end) != Some(end)
        || lines.any(|line| line.starts_with(b"-----BEGIN ") || line.starts_with(b"-----END "))
    {
        return Err("push signing key must contain exactly one complete PEM section".into());
    }
    debug_assert!(pem.starts_with(begin));
    let mut sections = <(SectionKind, Vec<u8>)>::pem_slice_iter(pem);
    let (kind, der) = sections
        .next()
        .ok_or_else(|| "missing push signing key PEM section".to_string())?
        .map_err(|_| "invalid push signing key PEM".to_string())?;
    let der = Zeroizing::new(der);
    if !sections.remainder().trim_ascii().is_empty() {
        return Err("trailing data after push signing key PEM".into());
    }
    // AWS-LC validates key structure, family, parameters and key consistency.
    // The DER decoder additionally rejects bytes following the outer sequence,
    // which AWS-LC's key readers otherwise permit.
    <&SequenceRef>::from_der(&der).map_err(|_| "invalid push signing key DER".to_string())?;
    Ok((kind, der))
}

fn signing_input(
    algorithm: &str,
    key_id: Option<&str>,
    claims: &norito::json::Value,
) -> Result<String, String> {
    let mut header = norito::json::Map::new();
    header.insert("typ".to_string(), norito::json::Value::from("JWT"));
    header.insert("alg".to_string(), norito::json::Value::from(algorithm));
    if let Some(key_id) = key_id {
        header.insert("kid".to_string(), norito::json::Value::from(key_id));
    }
    let header = norito::json::to_vec(&norito::json::Value::Object(header))
        .map_err(|error| error.to_string())?;
    let claims = norito::json::to_vec(claims).map_err(|error| error.to_string())?;
    let mut message = URL_SAFE_NO_PAD.encode(header);
    message.push('.');
    URL_SAFE_NO_PAD.encode_string(claims, &mut message);
    Ok(message)
}

fn complete_token(mut message: String, signature: &[u8]) -> String {
    message.push('.');
    URL_SAFE_NO_PAD.encode_string(signature, &mut message);
    message
}

#[cfg(test)]
#[path = "jwt/tests.rs"]
mod tests;
