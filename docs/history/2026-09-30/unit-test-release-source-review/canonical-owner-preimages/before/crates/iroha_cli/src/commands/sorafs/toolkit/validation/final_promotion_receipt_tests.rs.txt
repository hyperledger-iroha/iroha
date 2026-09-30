//! Offline final-promotion verification with genuine software-simulated signatures.

use super::*;
use std::fs;

mod fixture {
    use sorafs_manifest as manifest;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../sorafs_manifest/src/signer/final_promotion/tests/fixture_support.rs"
    ));
}

const REQUIRED: [&str; 11] = [
    "--statement",
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
];

fn parse(arguments: &[String]) -> Result<Args, CliError> {
    super::super::tests::parse_args(arguments)
}

fn replace(arguments: &mut [String], flag: &str, value: String) {
    let index = arguments.iter().position(|value| value == flag).unwrap();
    arguments[index + 1] = value;
}

fn inputs(evidence: &fixture::EvidenceFixture) -> (tempfile::TempDir, Vec<String>) {
    let directory = tempfile::tempdir().unwrap();
    let physical = directory.path().canonicalize().unwrap();
    let documents = [
        ("--statement", evidence.receipt.message.clone()),
        (
            "--signature",
            evidence.receipt.receipt.signatures[0].signature.clone(),
        ),
        ("--public-key", evidence.raw_public_key().to_vec()),
        ("--signer-policy", evidence.policy_bytes()),
        ("--custody-trust", evidence.trust_bytes()),
        ("--completed-operation-state", evidence.state_bytes()),
        ("--operation-receipt", evidence.receipt_bytes()),
    ];
    let mut arguments = Vec::new();
    for (flag, bytes) in documents {
        let path = physical.join(flag.trim_start_matches('-'));
        fs::write(&path, bytes).unwrap();
        arguments.extend([flag.to_owned(), path.to_str().unwrap().to_owned()]);
    }
    for (flag, digest) in [
        (
            "--public-key-fingerprint",
            evidence.expected.public_key_fingerprint_sha256,
        ),
        ("--signer-policy-sha256", evidence.expected.policy_sha256),
        ("--custody-trust-sha256", evidence.expected.trust_sha256),
    ] {
        arguments.extend([flag.into(), hex::encode(digest)]);
    }
    arguments.extend([
        "--now-unix-ms".into(),
        evidence.expected.now_unix_ms.to_string(),
    ]);
    (directory, arguments)
}

#[test]
fn final_promotion_cli_verifies_complete_signed_fixture_and_reports_exact_scope() {
    let evidence = fixture::EvidenceFixture::new();
    assert!(fixture::verify_receipt(&evidence.receipt).is_ok());
    assert!(evidence.verify().is_ok());
    let (_directory, arguments) = inputs(&evidence);
    let args = parse(&arguments).unwrap();
    let json::Value::Object(result) = verify(&args).unwrap() else {
        panic!("verification output must be an object");
    };
    assert_eq!(
        result["schema"],
        json::Value::from("sorafs.final_promotion_receipt_verification.v1")
    );
    assert_eq!(result["status"], json::Value::from("verified"));
    assert_eq!(
        result["verification_scope"],
        json::Value::from("final_promotion_signer_receipt")
    );
    assert_eq!(
        result["statement_sha256"],
        json::Value::from(hex::encode(sha256(&evidence.receipt.message)))
    );
    assert_eq!(
        result["statement_size"],
        json::Value::from(evidence.receipt.message.len() as u64)
    );
    assert_eq!(
        result["role"],
        json::Value::from("final_promotion_provenance")
    );
    assert!(!result.contains_key("backend"));
    assert_eq!(
        result["chain_id"],
        json::Value::from(evidence.policy.binding.chain_id.clone())
    );
    assert_eq!(
        result["network_id"],
        json::Value::from(hex::encode(evidence.policy.binding.network_id))
    );
    assert_eq!(
        result["operation_id"],
        json::Value::from(hex::encode(evidence.policy.operation_id))
    );
    assert!(!result.contains_key("promotion_eligible"));
    assert!(!result.contains_key("release_qualified"));
    assert_eq!(run(args).unwrap(), ExitCode::SUCCESS);
}

#[test]
fn final_promotion_cli_requires_every_independent_input_and_closed_options() {
    let (_directory, arguments) = inputs(&fixture::EvidenceFixture::new());
    assert!(parse(&arguments).is_ok());
    for flag in REQUIRED {
        let mut missing = arguments.clone();
        let index = missing.iter().position(|value| value == flag).unwrap();
        missing.drain(index..index + 2);
        assert!(parse(&missing).is_err(), "missing {flag}");
    }
    for flag in [
        "--manifest",
        "--hardware-verified",
        "--signing-seed",
        "--signer-policy-sha256",
    ] {
        let mut extra = arguments.clone();
        extra.extend([flag.into(), "unexpected".into()]);
        assert!(parse(&extra).is_err(), "extra {flag}");
    }
}

#[test]
fn final_promotion_cli_is_registered_as_offline_artifact_command() {
    use super::super::super::Command;
    use clap::{FromArgMatches as _, Subcommand as _};
    let (_directory, arguments) = inputs(&fixture::EvidenceFixture::new());
    let matches = Command::augment_subcommands(clap::Command::new("toolkit"))
        .try_get_matches_from(
            ["toolkit".to_owned(), "final-promotion-receipt".to_owned()]
                .into_iter()
                .chain(arguments),
        )
        .unwrap();
    let command = Command::from_arg_matches(&matches).unwrap();
    assert!(command.is_artifact_tool());
    assert!(matches!(&command, Command::FinalPromotionReceipt(_)));
    assert_eq!(command.run_artifact().unwrap(), ExitCode::SUCCESS);
}

#[test]
fn final_promotion_cli_rejects_noncanonical_clock_and_pins_before_file_access() {
    let (_directory, arguments) = inputs(&fixture::EvidenceFixture::new());
    for clock in ["0", "0120000", "+120000", "-1", "18446744073709551616"] {
        let mut changed = arguments.clone();
        replace(&mut changed, "--statement", "/unused-source-file".into());
        replace(&mut changed, "--now-unix-ms", clock.into());
        assert!(matches!(
            parse(&changed).and_then(|args| verify(&args)),
            Err(CliError::Config(_))
        ));
    }
    for flag in [
        "--public-key-fingerprint",
        "--signer-policy-sha256",
        "--custody-trust-sha256",
    ] {
        for pin in [
            "AA".repeat(32),
            "11".repeat(31),
            format!(" {}", "11".repeat(32)),
        ] {
            let mut changed = arguments.clone();
            replace(&mut changed, "--statement", "/unused-source-file".into());
            replace(&mut changed, flag, pin);
            assert!(matches!(
                verify(&parse(&changed).unwrap()),
                Err(CliError::Config(_))
            ));
        }
    }
}

#[test]
fn final_promotion_cli_rejects_tampering_in_every_detached_document() {
    let evidence = fixture::EvidenceFixture::new();
    for flag in [
        "--statement",
        "--signature",
        "--public-key",
        "--signer-policy",
        "--custody-trust",
        "--completed-operation-state",
        "--operation-receipt",
    ] {
        let (_directory, arguments) = inputs(&evidence);
        assert!(verify(&parse(&arguments).unwrap()).is_ok());
        let index = arguments.iter().position(|value| value == flag).unwrap();
        let path = Path::new(&arguments[index + 1]);
        let mut bytes = fs::read(path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(path, bytes).unwrap();
        assert!(
            verify(&parse(&arguments).unwrap()).is_err(),
            "tampered {flag}"
        );
    }
}

#[test]
fn final_promotion_cli_cannot_select_policy_or_trust_from_candidate_bytes() {
    let (_directory, arguments) = inputs(&fixture::EvidenceFixture::new());
    assert!(verify(&parse(&arguments).unwrap()).is_ok());
    for flag in [
        "--public-key-fingerprint",
        "--signer-policy-sha256",
        "--custody-trust-sha256",
    ] {
        let mut changed = arguments.clone();
        replace(&mut changed, flag, "ab".repeat(32));
        assert!(
            verify(&parse(&changed).unwrap()).is_err(),
            "substituted {flag}"
        );
    }
}

#[test]
fn final_promotion_cli_attestation_key_cannot_replace_statement_signer() {
    let evidence = fixture::EvidenceFixture::new();
    let (_directory, arguments) = inputs(&evidence);
    assert!(verify(&parse(&arguments).unwrap()).is_ok());
    let counterfeit = iroha_crypto::Signature::new(
        evidence.receipt.attester.private_key(),
        &evidence.receipt.message,
    );
    let index = arguments
        .iter()
        .position(|value| value == "--signature")
        .unwrap();
    fs::write(&arguments[index + 1], counterfeit.payload()).unwrap();
    assert!(verify(&parse(&arguments).unwrap()).is_err());
}

#[test]
fn final_promotion_cli_rejects_resigned_revocation_and_context_substitution() {
    let mut evidence = fixture::EvidenceFixture::new();
    assert!(evidence.verify().is_ok());
    evidence.state.body.signer_revoked = true;
    evidence.sign_state();
    let (_directory, arguments) = inputs(&evidence);
    assert!(verify(&parse(&arguments).unwrap()).is_err());
    evidence.state.body.signer_revoked = false;
    evidence.state.body.chain_id = "another-chain".into();
    evidence.sign_state();
    let (_directory, arguments) = inputs(&evidence);
    assert!(verify(&parse(&arguments).unwrap()).is_err());
}

#[test]
fn final_promotion_cli_enforces_input_lengths_and_document_bounds() {
    let evidence = fixture::EvidenceFixture::new();
    for (flag, size) in [
        ("--signature", 63),
        ("--public-key", 31),
        (
            "--statement",
            SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64 + 1,
        ),
        (
            "--signer-policy",
            SIGNER_FINAL_PROMOTION_EVIDENCE_DOCUMENT_MAX_BYTES_V1 as u64 + 1,
        ),
        (
            "--operation-receipt",
            SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1 as u64 + 1,
        ),
    ] {
        let (_directory, arguments) = inputs(&evidence);
        let index = arguments.iter().position(|value| value == flag).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&arguments[index + 1])
            .unwrap()
            .set_len(size)
            .unwrap();
        assert!(
            verify(&parse(&arguments).unwrap()).is_err(),
            "invalid size {flag}"
        );
    }
}

#[test]
fn final_promotion_cli_rejects_missing_file_and_unsupported_output() {
    let (_directory, mut arguments) = inputs(&fixture::EvidenceFixture::new());
    replace(&mut arguments, "--statement", "/unused-source-file".into());
    let mut args = parse(&arguments).unwrap();
    assert!(matches!(
        run(parse(&arguments).unwrap()),
        Err(CliError::Io(_))
    ));
    args.format = "yaml".into();
    assert!(matches!(verify(&args), Err(CliError::Config(_))));
}
