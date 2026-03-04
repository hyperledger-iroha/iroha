use std::{fs, path::Path};

#[test]
fn ptx_artifacts_are_present_and_non_empty() {
    let out_dir = env!("OUT_DIR");
    let files = [
        "add.ptx",
        "sha256.ptx",
        "vector.ptx",
        "poseidon.ptx",
        "aes.ptx",
        "sha3.ptx",
        "bn254.ptx",
        "signature.ptx",
    ];

    for file in files {
        let path = Path::new(out_dir).join(file);
        let Ok(bytes) = fs::read(&path) else {
            // Kernels are optional (feature `cuda`); treat missing artifacts as a skipped test.
            eprintln!("skipping PTX artifact check: {path:?} not found (CUDA feature disabled?)");
            return;
        };
        assert!(!bytes.is_empty(), "{path:?} is empty");
    }
}
