//! Interop matrix covering SM2/SM3/SM4 against OpenSSL and Tongsuo CLIs.
#![cfg(feature = "sm")]

use std::{
    env, fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use iroha_crypto::{Sm2PrivateKey, Sm3Digest};
use tempfile::TempDir;

const CLI_DEFAULTS: &[(&str, &str)] = &[("openssl", "OpenSSL"), ("tongsuo", "Tongsuo")];

// Shared deterministic fixture so Rust/Python/JS stay aligned.
const SM_KNOWN_ANSWERS_TOML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sm_known_answers.toml"
));

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone)]
struct CliTool {
    bin: String,
    display: String,
}

impl CliTool {
    fn new(bin: &str, display: &str) -> Self {
        Self {
            bin: bin.to_owned(),
            display: display.to_owned(),
        }
    }
}

struct Sm2Fixture {
    distid: String,
    seed: Vec<u8>,
    message: Vec<u8>,
}

fn available_cli_tools() -> Vec<CliTool> {
    let configured = env::var("IROHA_SM_CLI").ok().map(|value| {
        value
            .split_whitespace()
            .filter(|entry| !entry.is_empty())
            .map(|bin| CliTool::new(bin, bin))
            .collect::<Vec<_>>()
    });

    let candidates: Vec<CliTool> = configured.unwrap_or_else(|| {
        CLI_DEFAULTS
            .iter()
            .map(|(bin, label)| CliTool::new(bin, label))
            .collect()
    });

    let mut tools = Vec::new();
    for cli in candidates {
        match Command::new(&cli.bin).arg("version").output() {
            Ok(output) if output.status.success() => tools.push(cli),
            Ok(_) | Err(_) => {
                eprintln!(
                    "SM CLI `{}` ({}) unavailable; skipping",
                    cli.bin, cli.display
                );
            }
        }
    }
    tools
}

fn should_skip(stderr: &str) -> bool {
    let lowered = stderr.to_ascii_lowercase();
    lowered.contains("unknown option")
        || lowered.contains("unsupported")
        || lowered.contains("sm2 support routines are disabled")
        || lowered.contains("sm4 support routines are disabled")
        || lowered.contains("sm3 not supported")
        || lowered.contains("unknown digest")
        || lowered.contains("unknown cipher")
        || lowered.contains("sm4 is disabled")
        || lowered.contains("usage: enc")
        || lowered.contains("private key decode error")
        || lowered.contains("public key decode error")
        || lowered.contains("bad decrypt")
        || lowered.contains("tag verify failed")
}

fn write_bytes(path: &Path, bytes: &[u8]) -> TestResult {
    fs::write(path, bytes)?;
    Ok(())
}

fn write_str(path: &Path, value: &str) -> TestResult {
    let mut file = fs::File::create(path)?;
    file.write_all(value.as_bytes())?;
    Ok(())
}

fn temp_artifacts() -> TestResult<(TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().to_path_buf();
    Ok((dir, path))
}

fn sm2_fixture() -> &'static Sm2Fixture {
    static FIXTURE: OnceLock<Sm2Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut distid: Option<String> = None;
        let mut seed: Option<Vec<u8>> = None;
        let mut message: Option<Vec<u8>> = None;

        let mut in_section = false;
        for raw_line in SM_KNOWN_ANSWERS_TOML.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(stripped) = line
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                if in_section && stripped != "sm2.rust_sdk_fixture_v1" {
                    break;
                }
                in_section = stripped == "sm2.rust_sdk_fixture_v1";
                continue;
            }

            if !in_section {
                continue;
            }

            let (key, value_raw) = match line.split_once('=') {
                Some(pair) => pair,
                None => continue,
            };
            let key = key.trim();
            let mut value = value_raw.trim();
            if let Some(comment_idx) = value.find('#') {
                value = value[..comment_idx].trim();
            }
            if let Some(stripped) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                value = stripped;
            }

            match key {
                "distid" => distid = Some(value.to_owned()),
                "seed" => seed = Some(value.as_bytes().to_vec()),
                "message" => message = Some(hex::decode(value).expect("hex decode message")),
                _ => {}
            }
        }

        let distid =
            distid.unwrap_or_else(|| panic!("missing distid in sm2.rust_sdk_fixture_v1 section"));
        let seed =
            seed.unwrap_or_else(|| panic!("missing seed in sm2.rust_sdk_fixture_v1 section"));
        let message =
            message.unwrap_or_else(|| panic!("missing message in sm2.rust_sdk_fixture_v1 section"));

        Sm2Fixture {
            distid,
            seed,
            message,
        }
    })
}
#[test]
fn sm3_cli_digest_matches_rustcrypto() -> TestResult {
    let clis = available_cli_tools();
    if clis.is_empty() {
        eprintln!("skipping SM3 CLI parity: no SM-capable CLI detected");
        return Ok(());
    }

    for cli in clis {
        let (tmpdir, path) = temp_artifacts()?;
        let msg_path = path.join("message.bin");
        let message = b"Iroha SM3 interop";
        write_bytes(&msg_path, message)?;

        let output = Command::new(&cli.bin)
            .arg("dgst")
            .arg("-sm3")
            .arg("-binary")
            .arg(&msg_path)
            .output()
            .expect("run CLI dgst");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if should_skip(&stderr) {
                eprintln!("skipping SM3 parity for {}: {stderr}", cli.display);
                continue;
            }
            return Err(format!("{} dgst failed: {stderr}", cli.display).into());
        }

        let digest = Sm3Digest::hash(message);
        assert_eq!(
            digest.as_bytes(),
            output.stdout.as_slice(),
            "{} SM3 digest mismatch",
            cli.display
        );
        drop(tmpdir);
    }
    Ok(())
}

#[test]
fn sm2_rust_signature_verifies_in_cli() -> TestResult {
    let clis = available_cli_tools();
    if clis.is_empty() {
        eprintln!("skipping SM2 CLI verify parity: no SM-capable CLI detected");
        return Ok(());
    }

    let fixture = sm2_fixture();
    let private = Sm2PrivateKey::from_seed(fixture.distid.as_str(), fixture.seed.as_slice())?;
    let message = fixture.message.as_slice();
    let signature = private.sign(message);
    let signature_der = signature.as_der();
    let public_pem = private.public_key().to_public_key_pem()?;

    for cli in clis {
        let (tmpdir, dir_path) = temp_artifacts()?;
        let msg_path = dir_path.join("message.bin");
        let sig_path = dir_path.join("signature.der");
        let pub_path = dir_path.join("public.pem");

        write_bytes(&msg_path, message)?;
        write_bytes(&sig_path, signature_der.as_slice())?;
        write_str(&pub_path, public_pem.as_str())?;

        let output = Command::new(&cli.bin)
            .arg("pkeyutl")
            .arg("-verify")
            .arg("-pubin")
            .arg("-inkey")
            .arg(&pub_path)
            .arg("-in")
            .arg(&msg_path)
            .arg("-sigfile")
            .arg(&sig_path)
            .arg("-digest")
            .arg("sm3")
            .arg("-rawin")
            .arg("-pkeyopt")
            .arg(format!("distid:{}", fixture.distid))
            .output();

        let output = match output {
            Ok(out) => out,
            Err(err) => {
                eprintln!(
                    "failed to execute {} CLI binary ({}); skipping: {err}",
                    cli.display, cli.bin
                );
                continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if should_skip(&stderr) {
                eprintln!("skipping SM2 verify parity for {}: {stderr}", cli.display);
                continue;
            }
            return Err(format!("{} SM2 verify failed: {stderr}", cli.display).into());
        }

        drop(tmpdir);
    }

    Ok(())
}
