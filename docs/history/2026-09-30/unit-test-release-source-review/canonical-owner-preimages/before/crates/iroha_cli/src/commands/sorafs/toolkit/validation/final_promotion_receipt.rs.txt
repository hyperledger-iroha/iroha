//! Offline final-promotion signer receipt verification with independent public trust and time.
//!
//! Success authenticates the exact final-promotion signature and signed custody/operation claims.
//! Complete promotion still requires the replay, archive, provenance and underlying lane checks.

use super::{CliError, parse_release_fingerprint, read_release_input};
use iroha_crypto::sha256;
use norito::json;
use sorafs_manifest::signer::{
    final_promotion::{
        SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1, SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1,
        evidence::{
            SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
            SignerFinalPromotionEvidenceExpectedV1, verify_final_promotion_evidence_v1,
        },
    },
    protocol::SignerPurposeBindingV1,
};
use std::{path::Path, process::ExitCode};

/// Independently pinned final-promotion receipt verification inputs.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Exact domain-prefixed canonical statement file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    statement: String,
    /// Detached raw 64-byte Ed25519 signature file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    signature: String,
    /// Raw 32-byte Ed25519 public-key file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    public_key: String,
    /// Independently trusted lowercase SHA-256 of the raw public key.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    public_key_fingerprint: String,
    /// Canonical independently reviewed signer-policy file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    signer_policy: String,
    /// Independently trusted lowercase SHA-256 of the signer policy.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    signer_policy_sha256: String,
    /// Canonical independently reviewed custody and observer trust file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    custody_trust: String,
    /// Independently trusted lowercase SHA-256 of custody trust.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    custody_trust_sha256: String,
    /// Canonical signed current-state observation file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    completed_operation_state: String,
    /// Canonical complete final-promotion operation receipt file.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    operation_receipt: String,
    /// Explicit trusted current Unix time in canonical positive milliseconds.
    #[arg(long, value_parser = super::args::parse_nonempty_text)]
    now_unix_ms: String,
    #[arg(long, default_value = "json", value_parser = ["json"])]
    format: String,
}

fn verify(args: &Args) -> Result<json::Value, CliError> {
    if args.format != "json" {
        return Err(CliError::Config(
            "final-promotion-receipt supports only JSON output".into(),
        ));
    }
    let now_text = &args.now_unix_ms;
    let now_unix_ms = now_text.parse::<u64>().map_err(|_| {
        CliError::Config(
            "final-promotion-receipt requires a canonical positive trusted Unix millisecond time"
                .into(),
        )
    })?;
    if now_unix_ms == 0 || now_unix_ms.to_string() != *now_text {
        return Err(CliError::Config(
            "final-promotion-receipt trusted time is not canonical".into(),
        ));
    }
    let expected = SignerFinalPromotionEvidenceExpectedV1 {
        policy_sha256: parse_release_fingerprint(&args.signer_policy_sha256)?,
        trust_sha256: parse_release_fingerprint(&args.custody_trust_sha256)?,
        public_key_fingerprint_sha256: parse_release_fingerprint(&args.public_key_fingerprint)?,
        now_unix_ms,
    };
    let read = |path: &str, flag: &str, maximum: usize, exact: Option<u64>| {
        read_release_input(Path::new(path), flag, maximum as u64, exact, false)
    };
    let statement = read(
        &args.statement,
        "--statement",
        SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1,
        None,
    )?;
    let signature = read(&args.signature, "--signature", 64, Some(64))?;
    let public_key = read(&args.public_key, "--public-key", 32, Some(32))?;
    let policy = read(
        &args.signer_policy,
        "--signer-policy",
        SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let trust = read(
        &args.custody_trust,
        "--custody-trust",
        SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let state = read(
        &args.completed_operation_state,
        "--completed-operation-state",
        SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let receipt = read(
        &args.operation_receipt,
        "--operation-receipt",
        SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
        None,
    )?;
    let public_key: [u8; 32] = public_key.try_into().map_err(|_| {
        CliError::Validation("final-promotion-receipt public key length is invalid".into())
    })?;
    let verified = verify_final_promotion_evidence_v1(
        &policy,
        &trust,
        &state,
        &receipt,
        &statement,
        &signature,
        &public_key,
        &expected,
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let custody = verified.custody();
    let binding = &custody.statement().binding;
    let completion = verified.completion();
    let SignerPurposeBindingV1::FinalPromotionProvenance { deployment_id } = &binding.purpose
    else {
        return Err(CliError::Validation(
            "verified final-promotion receipt purpose is inconsistent".into(),
        ));
    };
    let fields = [
        (
            "schema",
            json::Value::from("sorafs.final_promotion_receipt_verification.v1"),
        ),
        ("status", json::Value::from("verified")),
        (
            "verification_scope",
            json::Value::from("final_promotion_signer_receipt"),
        ),
        (
            "statement_sha256",
            json::Value::from(hex::encode(sha256(&statement))),
        ),
        ("statement_size", json::Value::from(statement.len() as u64)),
        (
            "signature_sha256",
            json::Value::from(hex::encode(sha256(&signature))),
        ),
        (
            "public_key_fingerprint_sha256",
            json::Value::from(hex::encode(sha256(public_key))),
        ),
        (
            "signer_policy_sha256",
            json::Value::from(hex::encode(expected.policy_sha256)),
        ),
        (
            "custody_trust_sha256",
            json::Value::from(hex::encode(expected.trust_sha256)),
        ),
        (
            "completed_operation_state_sha256",
            json::Value::from(hex::encode(sha256(&state))),
        ),
        (
            "operation_receipt_sha256",
            json::Value::from(hex::encode(sha256(&receipt))),
        ),
        (
            "operation_id",
            json::Value::from(hex::encode(completion.operation_id)),
        ),
        (
            "custody_record_digest",
            json::Value::from(hex::encode(custody.record_digest())),
        ),
        (
            "policy_digest",
            json::Value::from(hex::encode(binding.policy_digest)),
        ),
        ("key_revision", json::Value::from(binding.key_revision)),
        (
            "policy_revision",
            json::Value::from(binding.policy_revision),
        ),
        ("service_id", json::Value::from(binding.service_id.clone())),
        (
            "administrator_id",
            json::Value::from(binding.administrator_id.clone()),
        ),
        ("role", json::Value::from(binding.role.as_str())),
        ("deployment_id", json::Value::from(deployment_id.clone())),
        ("chain_id", json::Value::from(binding.chain_id.clone())),
        (
            "network_id",
            json::Value::from(hex::encode(binding.network_id)),
        ),
        (
            "finalized_height",
            json::Value::from(completion.anchor.height),
        ),
        (
            "finalized_block_hash",
            json::Value::from(hex::encode(completion.anchor.block_hash)),
        ),
        ("verified_at_unix_ms", json::Value::from(now_unix_ms)),
    ];
    let result = json::Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    );
    Ok(result)
}

pub(crate) fn run(args: Args) -> Result<ExitCode, CliError> {
    let result = verify(&args)?;
    println!(
        "{}",
        json::to_string(&result).map_err(|_| CliError::Internal(
            "failed to encode final-promotion-receipt result".into()
        ))?
    );
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[path = "final_promotion_receipt_tests.rs"]
mod tests;
