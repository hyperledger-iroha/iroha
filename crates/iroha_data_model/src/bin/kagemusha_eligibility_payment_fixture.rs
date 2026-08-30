//! Publish one exact current ABI-21/V4 request/payment pair and its ABI-22 eligibility envelope.
//!
//! The input request and payment are decoded, semantically validated, and
//! byte-for-byte canonical-round-tripped. Only the new outer credential and
//! one-use envelope are generated. Deterministic fixture keys are test-only and
//! must never be copied into a wallet or release configuration.

use std::{
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Cursor, Write as _},
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    offline::{
        KAGEMUSHA_ELIGIBILITY_PAYMENT_ENVELOPE_MAX_ARCHIVE_BYTES_V1,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        KAGEMUSHA_SINGLE_RECURSIVE_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4,
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2,
        KagemushaEligibilityPaymentEnvelopePayloadV1, KagemushaEligibilityPaymentEnvelopeV1,
        KagemushaRecipientPaymentRequestV2, KagemushaRecursiveSpendPeerPaymentV4,
        OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1,
        OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1,
        OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1,
        OfflineDeviceEligibilityCredentialPayloadV1, OfflineDeviceEligibilityCredentialV1,
        OfflineDeviceEligibilityOutcomeV1, OfflineDevicePolicyFinalityBindingV1,
    },
};
use norito::{NoritoDeserialize, NoritoSerialize};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

const ISSUER_SEED: u8 = 0x61;
const ACCOUNT_SEED: u8 = 0x62;
const DEVICE_KEY_SEED: u8 = 0x72;
const ASSERTION_KEY_SEED: u8 = 0x73;
const CANONICAL_DECODE_ALLOCATION_MULTIPLIER: usize = 4;
const CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE: usize = 64 * 1024;
const CANONICAL_DECODE_MAX_NESTING_DEPTH: usize = 64;
const REQUEST_FIXTURE_NAME: &str = "recipient-request-v2.norito.b64";
const PAYMENT_FIXTURE_NAME: &str = "peer-payment-v4.norito.b64";
const ENVELOPE_FIXTURE_NAME: &str = "eligibility-payment-envelope-v1.norito.b64";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Write,
    Check,
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    mode: Mode,
    request: PathBuf,
    payment: PathBuf,
    output_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(env::args().skip(1))?;
    let request_bytes = read_encoded_fixture(&args.request)?;
    let payment_bytes = read_encoded_fixture(&args.payment)?;
    let envelope_bytes = generate_envelope(&request_bytes, &payment_bytes)?;
    let rendered = [
        (REQUEST_FIXTURE_NAME, render_base64(&request_bytes)),
        (PAYMENT_FIXTURE_NAME, render_base64(&payment_bytes)),
        (ENVELOPE_FIXTURE_NAME, render_base64(&envelope_bytes)),
    ];

    match args.mode {
        Mode::Write => write_new_fixture_set(&args.output_dir, &rendered)?,
        Mode::Check => check_fixture_set(&args.output_dir, &rendered)?,
    }

    println!("request_sha256={}", sha256_hex(&request_bytes));
    println!("request_bytes={}", request_bytes.len());
    println!("payment_sha256={}", sha256_hex(&payment_bytes));
    println!("payment_bytes={}", payment_bytes.len());
    println!("envelope_sha256={}", sha256_hex(&envelope_bytes));
    println!("envelope_bytes={}", envelope_bytes.len());
    Ok(())
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<Args, io::Error> {
    let mut mode = None;
    let mut request = None;
    let mut payment = None;
    let mut output_dir = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let slot = match argument.as_str() {
            "--write" | "--check" if mode.is_none() => {
                mode = Some(if argument == "--write" {
                    Mode::Write
                } else {
                    Mode::Check
                });
                continue;
            }
            "--request" if request.is_none() => &mut request,
            "--payment" if payment.is_none() => &mut payment,
            "--output-dir" if output_dir.is_none() => &mut output_dir,
            _ => return Err(usage_error()),
        };
        *slot = Some(PathBuf::from(arguments.next().ok_or_else(usage_error)?));
    }
    Ok(Args {
        mode: mode.ok_or_else(usage_error)?,
        request: request.ok_or_else(usage_error)?,
        payment: payment.ok_or_else(usage_error)?,
        output_dir: output_dir.ok_or_else(usage_error)?,
    })
}

fn usage_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: kagemusha_eligibility_payment_fixture (--write|--check) \
         --request <recipient-request-v2.norito.(hex|b64)> \
         --payment <peer-payment-v4.norito.(hex|b64)> --output-dir <fixture-directory>",
    )
}

fn read_encoded_fixture(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("b64") => read_base64_fixture(path),
        Some("hex") => read_hex_fixture(path),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("fixture path must end in .hex or .b64: {}", path.display()),
        )
        .into()),
    }
}

fn read_base64_fixture(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let compact: String = text
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    if compact.is_empty()
        || text
            .chars()
            .any(|character| character.is_whitespace() && !character.is_ascii_whitespace())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "fixture is empty or contains non-ASCII whitespace: {}",
                path.display()
            ),
        )
        .into());
    }
    let decoded = BASE64.decode(compact.as_bytes())?;
    if BASE64.encode(&decoded) != compact {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "fixture Base64 spelling is not canonical: {}",
                path.display()
            ),
        )
        .into());
    }
    Ok(decoded)
}

fn read_hex_fixture(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let compact: String = text
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    if compact.is_empty()
        || text
            .chars()
            .any(|character| character.is_whitespace() && !character.is_ascii_whitespace())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "fixture is empty or contains non-ASCII whitespace: {}",
                path.display()
            ),
        )
        .into());
    }
    let decoded = hex::decode(&compact)?;
    if hex::encode(&decoded) != compact {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("fixture hex spelling is not canonical: {}", path.display()),
        )
        .into());
    }
    Ok(decoded)
}

fn generate_envelope(
    request_bytes: &[u8],
    payment_bytes: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let request: KagemushaRecipientPaymentRequestV2 = decode_bridge_archive(
        request_bytes,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
        CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE,
    )
    .map_err(|error| contextual_error("decode recipient request", error))?;
    let payment: KagemushaRecursiveSpendPeerPaymentV4 = decode_bridge_archive(
        payment_bytes,
        KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        KAGEMUSHA_SINGLE_RECURSIVE_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4,
    )
    .map_err(|error| contextual_error("decode peer payment", error))?;
    require_exact_canonical(&request, request_bytes, "recipient request")?;
    require_exact_canonical(&payment, payment_bytes, "peer payment")?;
    request
        .validate_public_binding()
        .map_err(|error| contextual_error("validate recipient request", error))?;
    payment
        .validate_public_binding()
        .map_err(|error| contextual_error("validate peer payment", error))?;

    let device_key = p256_signing_key(DEVICE_KEY_SEED)
        .map_err(|error| contextual_error("derive device fixture key", error))?;
    let assertion_key = p256_signing_key(ASSERTION_KEY_SEED)
        .map_err(|error| contextual_error("derive assertion fixture key", error))?;
    let credential = eligibility_credential(&request, &device_key, &assertion_key)
        .map_err(|error| contextual_error("construct eligibility credential", error))?;
    let payload = KagemushaEligibilityPaymentEnvelopePayloadV1::prepare_v1(
        payment_bytes.to_vec(),
        credential,
        &request,
    )
    .map_err(|error| contextual_error("prepare eligibility envelope", error))?;
    let signing_bytes = payload
        .signing_bytes_v1()
        .map_err(|error| contextual_error("encode eligibility envelope signing bytes", error))?;
    let signature = p256_sign(&device_key, &signing_bytes)
        .map_err(|error| contextual_error("sign eligibility envelope", error))?;
    let envelope = KagemushaEligibilityPaymentEnvelopeV1::finalize_v1(payload, signature)
        .map_err(|error| contextual_error("finalize eligibility envelope", error))?;
    envelope
        .validate_static_binding_v1()
        .map_err(|error| contextual_error("validate eligibility envelope", error))?;
    let encoded = norito::encode_canonical(&envelope)
        .map_err(|error| contextual_error("encode eligibility envelope", error))?;
    let decoded: KagemushaEligibilityPaymentEnvelopeV1 = decode_bridge_archive(
        &encoded,
        KAGEMUSHA_ELIGIBILITY_PAYMENT_ENVELOPE_MAX_ARCHIVE_BYTES_V1,
        CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE,
    )
    .map_err(|error| contextual_error("decode generated eligibility envelope", error))?;
    if decoded != envelope || decoded.payment_v4_norito() != payment_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "generated envelope did not preserve its exact ABI-21/V4 payment",
        )
        .into());
    }
    Ok(encoded)
}

fn decode_bridge_archive<T>(
    bytes: &[u8],
    maximum_bytes: usize,
    fixed_allocation_allowance: usize,
) -> Result<T, io::Error>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "canonical archive length {} is outside 1..={maximum_bytes}",
                bytes.len()
            ),
        ));
    }
    let header = norito::core::Header::read(Cursor::new(bytes))
        .map_err(|error| contextual_error("read Norito header", error))?;
    if header.compression != norito::Compression::None
        || header.flags != norito::core::default_encode_flags()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "canonical archive uses non-default header flags or compression",
        ));
    }
    let _decode_flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let _payload_context = norito::core::PayloadCtxGuard::enter(bytes);
    let limits = norito::DecodeLimits::new(
        bytes.len(),
        bytes.len(),
        bytes.len().saturating_mul(2),
        bytes
            .len()
            .saturating_mul(CANONICAL_DECODE_ALLOCATION_MULTIPLIER)
            .saturating_add(fixed_allocation_allowance),
        CANONICAL_DECODE_MAX_NESTING_DEPTH,
    );
    let value: T = norito::decode_from_bytes_with_limits(bytes, limits)
        .map_err(|error| contextual_error("decode bounded Norito archive", error))?;
    let canonical = norito::encode_canonical(&value)
        .map_err(|error| contextual_error("re-encode canonical Norito archive", error))?;
    if canonical != bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive is not the exact canonical Norito encoding",
        ));
    }
    Ok(value)
}

fn contextual_error(context: &str, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{context}: {error}"))
}

fn require_exact_canonical<T>(value: &T, source: &[u8], label: &str) -> Result<(), Box<dyn Error>>
where
    T: norito::codec::Encode,
{
    if norito::encode_canonical(value)? != source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} is not the exact canonical Norito encoding"),
        )
        .into());
    }
    Ok(())
}

fn eligibility_credential(
    request: &KagemushaRecipientPaymentRequestV2,
    device_key: &SigningKey,
    assertion_key: &SigningKey,
) -> Result<OfflineDeviceEligibilityCredentialV1, Box<dyn Error>> {
    let issuer = KeyPair::try_from_seed(vec![ISSUER_SEED; 32], Algorithm::Ed25519)?;
    let account_key = KeyPair::try_from_seed(vec![ACCOUNT_SEED; 32], Algorithm::Ed25519)?;
    let expires_at_ms = request.expires_at_ms();
    let issued_at_ms = expires_at_ms
        .saturating_sub(OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1)
        .max(2);
    if issued_at_ms >= expires_at_ms {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "recipient request expiry cannot anchor a fixture credential",
        )
        .into());
    }
    let finality_evidence = b"deterministic ABI-22 eligibility fixture finality evidence";
    let finality = OfflineDevicePolicyFinalityBindingV1 {
        version: OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1,
        network_id: *request.network_id(),
        finalized_block_height: 1,
        finalized_block_hash: Hash::new(b"deterministic ABI-22 eligibility fixture block"),
        finalized_block_timestamp_ms: issued_at_ms.saturating_sub(1).max(1),
        finality_evidence_hash: Hash::new(finality_evidence),
    };
    let payload = OfflineDeviceEligibilityCredentialPayloadV1 {
        version: OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1,
        network_id: *request.network_id(),
        account_id: AccountId::new(account_key.public_key().clone()),
        device_id: "abi22-fixture-sender".to_owned(),
        attestation_key_id: "abi22-fixture-assertion-key".to_owned(),
        device_public_key: p256_public_key(device_key)?,
        assertion_public_key: assertion_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec(),
        registration_hash: Hash::new(b"deterministic ABI-22 fixture registration").into(),
        eligibility: OfflineDeviceEligibilityOutcomeV1::Eligible,
        policy_epoch: 1,
        policy_hash: Hash::new(b"deterministic ABI-22 fixture policy").into(),
        policy_finality: finality,
        policy_freshness_deadline_ms: expires_at_ms,
        issued_at_ms,
        expires_at_ms,
    };
    Ok(OfflineDeviceEligibilityCredentialV1::sign_v1(
        payload,
        issuer.public_key().clone(),
        issuer.private_key(),
    )?)
}

fn p256_signing_key(seed: u8) -> Result<SigningKey, Box<dyn Error>> {
    Ok(SigningKey::from_bytes((&[seed; 32]).into())?)
}

fn p256_public_key(key: &SigningKey) -> Result<KagemushaDevicePublicKeyV2, Box<dyn Error>> {
    Ok(KagemushaDevicePublicKeyV2::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )?)
}

fn p256_sign(
    key: &SigningKey,
    message: &[u8],
) -> Result<KagemushaDeviceSignatureV2, Box<dyn Error>> {
    let signature: P256Signature = key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    let raw: [u8; 64] = signature.to_bytes().into();
    Ok(KagemushaDeviceSignatureV2::from_raw_bytes(&raw)?)
}

fn render_base64(bytes: &[u8]) -> Vec<u8> {
    format!("{}\n", BASE64.encode(bytes)).into_bytes()
}

fn write_new_fixture_set(output_dir: &Path, fixtures: &[(&str, Vec<u8>)]) -> Result<(), io::Error> {
    match fs::symlink_metadata(output_dir) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "fixture output must be a regular directory: {}",
                        output_dir.display()
                    ),
                ));
            }
            if fs::read_dir(output_dir)?.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "fixture staging directory is not empty: {}",
                        output_dir.display()
                    ),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(output_dir)?,
        Err(error) => return Err(error),
    }
    for (name, bytes) in fixtures {
        write_new(&output_dir.join(name), bytes)?;
    }
    fs::File::open(output_dir)?.sync_all()
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn check_fixture_set(output_dir: &Path, fixtures: &[(&str, Vec<u8>)]) -> Result<(), io::Error> {
    for (name, expected) in fixtures {
        let path = output_dir.join(name);
        if fs::read(&path)? != *expected {
            return Err(io::Error::other(format!(
                "eligibility-payment fixture is stale: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_require_one_mode_and_all_paths() {
        let args = parse_args(
            [
                "--check",
                "--request",
                "request.b64",
                "--payment",
                "payment.b64",
                "--output-dir",
                "fixtures",
            ]
            .map(str::to_owned),
        )
        .expect("valid fixture arguments");
        assert_eq!(args.mode, Mode::Check);
        assert!(parse_args(["--write"].map(str::to_owned)).is_err());
        assert!(
            parse_args(
                [
                    "--write",
                    "--check",
                    "--request",
                    "r",
                    "--payment",
                    "p",
                    "--output-dir",
                    "out",
                ]
                .map(str::to_owned),
            )
            .is_err()
        );
    }
}
