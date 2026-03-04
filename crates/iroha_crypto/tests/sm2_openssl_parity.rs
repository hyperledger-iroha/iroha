//! Cross-check SM2 signatures against OpenSSL.
#![cfg(feature = "sm")]

use std::{fs::File, io::Write as _, path::Path, process::Command};

const PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKl
hESV6XwE/03yVIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
";

const SIG_DER_HEX: &str = "3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7";
const MESSAGE: &str = "message digest";
const DISTID: &str = "ALICE123@YAHOO.COM";

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex).expect("valid hex literal")
}

fn write_file(path: &Path, contents: &[u8]) {
    let mut file = File::create(path).expect("create file");
    file.write_all(contents).expect("write contents");
}

#[test]
fn openssl_cli_verifies_annex_example_signature() {
    // Skip the test gracefully if the openssl CLI is unavailable.
    let openssl_present = Command::new("openssl").arg("version").output();
    if openssl_present.is_err() {
        eprintln!("skipping SM2 OpenSSL parity test: `openssl` binary not available");
        return;
    }

    let tmpdir = tempfile::tempdir().expect("create temp dir");
    let pub_path = tmpdir.path().join("pub.pem");
    let msg_path = tmpdir.path().join("msg.bin");
    let sig_path = tmpdir.path().join("sig.der");

    write_file(&pub_path, PUB_PEM.as_bytes());
    write_file(&msg_path, MESSAGE.as_bytes());
    write_file(&sig_path, &hex_to_bytes(SIG_DER_HEX));

    let output = Command::new("openssl")
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
        .arg(format!("distid:{DISTID}"))
        .output()
        .expect("run openssl pkeyutl");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Usage:")
            || stderr.contains("unknown option")
            || stderr.contains("unsupported")
            || stderr.contains("public key decode error")
        {
            eprintln!(
                "skipping SM2 OpenSSL parity test: local openssl lacks SM2 support ({stderr})"
            );
            return;
        }
        panic!("OpenSSL should accept Annex Example 1 signature; stderr: {stderr}");
    }

    // Clean up explicitly to avoid lingering temp files on CI nodes that do not auto-clean.
    tmpdir.close().expect("remove temp dir");
}
