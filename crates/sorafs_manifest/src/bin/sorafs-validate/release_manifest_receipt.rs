//! Verification-only release receipt adapter with independent policy/trust pins and clock input.

use super::{CliError, parse_release_fingerprint, read_release_input};
use iroha_crypto::sha256;
use norito::json;
use sorafs_manifest::signer::{
    protocol::{SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, SignerPurposeBindingV1},
    receipt::SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
    release_evidence::{
        SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1, SignerReleaseEvidenceExpectedV1,
        verify_release_manifest_evidence_v1,
    },
};
use std::{collections::BTreeMap, path::Path, process::ExitCode};

const OPTIONS: [&str; 12] = [
    "--manifest",
    "--signature",
    "--public-key",
    "--public-key-fingerprint",
    "--signer-policy",
    "--signer-policy-sha256",
    "--custody-trust",
    "--custody-trust-sha256",
    "--completed-operation-state",
    "--operation-receipt",
    "--now-unix-ms",
    "--format",
];

fn parse(args: &[String]) -> Result<BTreeMap<String, String>, CliError> {
    let mut parsed = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let (name, value) = if let Some((name, value)) = args[index].split_once('=') {
            (name, value)
        } else {
            let name = args[index].as_str();
            index += 1;
            let value = args.get(index).ok_or_else(|| {
                CliError::Config("release-manifest-receipt option requires a value".into())
            })?;
            (name, value.as_str())
        };
        if !OPTIONS.contains(&name)
            || value.is_empty()
            || value.starts_with("--")
            || parsed.insert(name.to_owned(), value.to_owned()).is_some()
        {
            return Err(CliError::Config(
                "invalid or duplicate release-manifest-receipt option".into(),
            ));
        }
        index += 1;
    }
    if OPTIONS[..OPTIONS.len() - 1]
        .iter()
        .any(|name| !parsed.contains_key(*name))
        || parsed
            .get("--format")
            .is_some_and(|format| format != "json")
    {
        return Err(CliError::Config(
            "release-manifest-receipt requires all independent source paths, policy/trust/key pins and --now-unix-ms; only JSON output is supported".into()));
    }
    Ok(parsed)
}

pub(super) fn run(args: &[String]) -> Result<ExitCode, CliError> {
    let args = parse(args)?;
    let now_text = &args["--now-unix-ms"];
    let now_unix_ms = now_text.parse::<u64>().map_err(|_| {
        CliError::Config(
            "release-manifest-receipt requires a canonical positive trusted Unix millisecond time"
                .into(),
        )
    })?;
    if now_unix_ms == 0 || now_unix_ms.to_string() != *now_text {
        return Err(CliError::Config(
            "release-manifest-receipt trusted time is not canonical".into(),
        ));
    }
    let expected = SignerReleaseEvidenceExpectedV1 {
        policy_sha256: parse_release_fingerprint(&args["--signer-policy-sha256"])?,
        trust_sha256: parse_release_fingerprint(&args["--custody-trust-sha256"])?,
        public_key_fingerprint_sha256: parse_release_fingerprint(
            &args["--public-key-fingerprint"],
        )?,
        now_unix_ms,
    };
    let read = |flag: &str, maximum: usize, exact: Option<u64>| {
        read_release_input(Path::new(&args[flag]), flag, maximum as u64, exact, false)
    };
    let manifest = read("--manifest", SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, None)?;
    let signature = read("--signature", 64, Some(64))?;
    let public_key = read("--public-key", 32, Some(32))?;
    let policy = read(
        "--signer-policy",
        SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let trust = read(
        "--custody-trust",
        SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let state = read(
        "--completed-operation-state",
        SIGNER_RELEASE_EVIDENCE_DOCUMENT_MAX_BYTES_V1,
        None,
    )?;
    let receipt = read(
        "--operation-receipt",
        SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
        None,
    )?;
    let public_key: [u8; 32] = public_key.try_into().map_err(|_| {
        CliError::Validation("release-manifest-receipt public key length is invalid".into())
    })?;
    let verified = verify_release_manifest_evidence_v1(
        &policy,
        &trust,
        &state,
        &receipt,
        &manifest,
        &signature,
        &public_key,
        &expected,
    )
    .map_err(|error| CliError::Validation(error.to_string()))?;
    let custody = verified.custody();
    let binding = &custody.statement().binding;
    let completion = verified.completion();
    let SignerPurposeBindingV1::ReleaseManifest { deployment_id } = &binding.purpose else {
        return Err(CliError::Validation(
            "verified release receipt purpose is inconsistent".into(),
        ));
    };
    let fields = [
        (
            "schema",
            json::Value::from("sorafs.release_manifest_receipt_verification.v1"),
        ),
        ("status", json::Value::from("verified")),
        (
            "manifest_sha256",
            json::Value::from(hex::encode(sha256(&manifest))),
        ),
        ("manifest_size", json::Value::from(manifest.len() as u64)),
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
        ("backend", json::Value::from("hardware")),
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
    println!(
        "{}",
        json::to_string(&result).map_err(|_| CliError::Internal(
            "failed to encode release-manifest-receipt result".into()
        ))?
    );
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_args() -> Vec<String> {
        OPTIONS[..OPTIONS.len() - 1]
            .iter()
            .flat_map(|flag| {
                [
                    (*flag).to_owned(),
                    match *flag {
                        "--now-unix-ms" => "120000".into(),
                        flag if flag.ends_with("sha256") || flag.ends_with("fingerprint") => {
                            "11".repeat(32)
                        }
                        _ => "/unused-source-file".into(),
                    },
                ]
            })
            .collect()
    }

    #[test]
    fn receipt_cli_requires_all_independent_pins_and_closed_options() {
        assert!(parse(&all_args()).is_ok());
        for index in 0..11 {
            let mut args = all_args();
            args.drain(index * 2..index * 2 + 2);
            assert!(parse(&args).is_err());
        }
        for extra in [
            "--signer-policy-sha256",
            "--hardware-verified",
            "--signing-seed",
        ] {
            let mut args = all_args();
            args.extend([extra.into(), "unexpected".into()]);
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn receipt_cli_rejects_noncanonical_clock_before_source_access() {
        for clock in ["0", "0120000", "+120000", "-1", "18446744073709551616"] {
            let mut args = all_args();
            *args.last_mut().unwrap() = clock.into();
            assert!(matches!(run(&args), Err(CliError::Config(_))));
        }
    }

    #[test]
    fn receipt_cli_rejects_missing_source_without_emitting_success() {
        assert!(matches!(run(&all_args()), Err(CliError::Io(_))));
    }
}
