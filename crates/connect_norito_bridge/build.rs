//! Build-time embedding for immutable mobile proving artifacts.

use std::{env, fs, path::PathBuf};

use sha2::{Digest as _, Sha256};

const ARTIFACTS: [(&str, &str, &str); 2] = [
    (
        "TRANSFER_V2",
        "CONNECT_NORITO_KAGEMUSHA_TRANSFER_V2_ARTIFACT_PATH",
        "kagemusha-transfer-v2-prover.bin",
    ),
    (
        "UNSHIELD_V3",
        "CONNECT_NORITO_KAGEMUSHA_UNSHIELD_V3_ARTIFACT_PATH",
        "kagemusha-unshield-v3-prover.bin",
    ),
];

fn main() {
    println!("cargo:rerun-if-env-changed=CONNECT_NORITO_SOURCE_REVISION");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    for (label, path_env, output_name) in ARTIFACTS {
        println!("cargo:rerun-if-env-changed={path_env}");
        let output = out_dir.join(output_name);
        let source = env::var_os(path_env).map(PathBuf::from);
        let bytes = source
            .as_ref()
            .and_then(|path| {
                println!("cargo:rerun-if-changed={}", path.display());
                fs::read(path).ok().filter(|bytes| !bytes.is_empty())
            })
            .unwrap_or_default();
        fs::write(&output, &bytes).expect("write embedded artifact staging file");
        let digest = Sha256::digest(&bytes);
        println!(
            "cargo:rustc-env=CONNECT_NORITO_EMBEDDED_{label}_AVAILABLE={}",
            u8::from(!bytes.is_empty())
        );
        println!(
            "cargo:rustc-env=CONNECT_NORITO_EMBEDDED_{label}_SIZE={}",
            bytes.len()
        );
        println!(
            "cargo:rustc-env=CONNECT_NORITO_EMBEDDED_{label}_SHA256={digest:x}"
        );
    }
    let revision = env::var("CONNECT_NORITO_SOURCE_REVISION")
        .unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=CONNECT_NORITO_EMBEDDED_SOURCE_REVISION={revision}");
}
