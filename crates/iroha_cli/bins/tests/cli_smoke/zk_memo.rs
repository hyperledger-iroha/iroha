//! Bounded local memo encodings and removal of misleading schema hashing.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_data_model::confidential::{
    CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1, CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1,
    CONFIDENTIAL_MEMO_XCHACHA_TAG_BYTES_V1, ConfidentialMemoEnvelopeV1,
    ConfidentialMemoRecipientSlotV1, ConfidentialMemoSuiteV1,
};

use super::{command, fs, torii_mock_support::TempDir};

fn memo() -> ConfidentialMemoEnvelopeV1 {
    ConfidentialMemoEnvelopeV1::new(
        core::array::from_fn(|index| {
            let suite = ConfidentialMemoSuiteV1::MlKem768XChaCha20Poly1305;
            ConfidentialMemoRecipientSlotV1::new(
                suite,
                vec![u8::try_from(index).unwrap() + 1; suite.encapsulation_bytes()],
                [17; CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1],
                [33; CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1],
            )
            .unwrap()
        }),
        [0xA5; CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1],
        vec![0x5A; CONFIDENTIAL_MEMO_XCHACHA_TAG_BYTES_V1],
    )
    .unwrap()
}

#[test]
fn memo_emits_one_requested_stdout_encoding_and_exact_file() {
    let directory = TempDir::new("zk_memo_outputs").unwrap();
    let input = directory.path().join("memo.json");
    let output_file = directory.path().join("memo.bin");
    let memo = memo();
    fs::write(&input, norito::json::to_vec(&memo).unwrap()).unwrap();
    let expected = norito::codec::encode_adaptive(&memo);
    for output_format in ["json", "text"] {
        for format in [None, Some("base64"), Some("hex"), Some("json")] {
            for write_file in [false, true] {
                let mut cli = command();
                cli.args([
                    "--output-format",
                    output_format,
                    "app",
                    "zk",
                    "envelope",
                    "--envelope-json",
                ])
                .arg(&input);
                if let Some(format) = format {
                    cli.args(["--format", format]);
                }
                if write_file {
                    cli.arg("--output").arg(&output_file);
                }
                let output = cli.output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                if write_file {
                    assert_eq!(fs::read(&output_file).unwrap(), expected);
                }
                if write_file && format.is_none() {
                    assert!(output.stdout.is_empty());
                    continue;
                }
                let text = std::str::from_utf8(&output.stdout).unwrap().trim();
                match format.unwrap_or("base64") {
                    "base64" => assert_eq!(BASE64.decode(text).unwrap(), expected),
                    "hex" => assert_eq!(hex::decode(text).unwrap(), expected),
                    "json" => assert_eq!(
                        norito::json::from_str::<ConfidentialMemoEnvelopeV1>(text).unwrap(),
                        memo
                    ),
                    _ => unreachable!(),
                }
            }
        }
    }
}

#[test]
fn memo_rejects_oversized_or_invalid_input_before_writing_output() {
    let directory = TempDir::new("zk_memo_bounds").unwrap();
    let input = directory.path().join("memo.json");
    let output_file = directory.path().join("memo.bin");
    let sparse = fs::File::create(&input).unwrap();
    sparse.set_len(64 * 1024 * 1024 + 1).unwrap();
    for (contents, expected_error) in [
        (None, "first-release limit"),
        (
            Some(norito::json::to_vec(&ConfidentialMemoEnvelopeV1::default()).unwrap()),
            "recipient slot 0",
        ),
        (
            Some(b"{\"unknown\":true}".to_vec()),
            "confidential memo envelope",
        ),
    ] {
        if let Some(contents) = contents {
            fs::write(&input, contents).unwrap();
        }
        let result = command()
            .args(["app", "zk", "envelope", "--envelope-json"])
            .arg(&input)
            .arg("--output")
            .arg(&output_file)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!output_file.exists());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected_error),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn memo_is_credential_free_and_rejects_transaction_globals() {
    let directory = TempDir::new("zk_memo_configless").unwrap();
    let input = directory.path().join("memo.json");
    let memo = memo();
    fs::write(&input, norito::json::to_vec(&memo).unwrap()).unwrap();
    let expected = norito::codec::encode_adaptive(&memo);
    for malformed_default in [false, true] {
        if malformed_default {
            fs::write(directory.path().join("client.toml"), b"invalid = [toml").unwrap();
        }
        let output = command()
            .current_dir(directory.path())
            .args(["--machine", "app", "zk", "envelope", "--envelope-json"])
            .arg(&input)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            BASE64
                .decode(std::str::from_utf8(&output.stdout).unwrap().trim())
                .unwrap(),
            expected,
        );
    }
    for globals in [
        vec!["--config", "missing-client.toml"],
        vec![
            "--config-fd",
            "9",
            "--config-source-path",
            "/missing-client.toml",
        ],
        vec!["--operator-private-key-file", "missing-private-key"],
        vec!["--metadata", "missing-metadata.json"],
        vec!["--fee-payer", "authority"],
        vec!["--stdin-instructions"],
        vec!["--emit-instructions"],
        vec!["--verbose"],
    ] {
        let result = command()
            .current_dir(directory.path())
            .args(globals)
            .args(["app", "zk", "envelope", "--envelope-json"])
            .arg(&input)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("credential-free local tooling"),
            "{}",
            String::from_utf8_lossy(&result.stderr),
        );
    }
}

#[test]
fn retired_schema_hash_and_mixed_memo_output_flags_are_rejected() {
    for (args, expected_error) in [
        (
            vec!["app", "zk", "schema-hash", "--public-inputs-hex", "00"],
            "unrecognized subcommand 'schema-hash'",
        ),
        (
            vec!["app", "zk", "envelope", "--print-hex"],
            "unexpected argument '--print-hex'",
        ),
        (
            vec![
                "app",
                "zk",
                "envelope",
                "--envelope-json",
                "unused.json",
                "--format",
                "hex",
                "--format",
                "json",
            ],
            "cannot be used multiple times",
        ),
    ] {
        let result = command().args(args).output().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected_error),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
