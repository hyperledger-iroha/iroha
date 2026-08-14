#[cfg(all(test, feature = "cuda"))]
mod tests {
    use std::{fs, path::Path};
    #[test]
    fn cuda_generated_ptx_files_exist() {
        let out_dir = env!("OUT_DIR");
        let files = [
            "add.ptx",
            "aes.ptx",
            "bitonic_sort.ptx",
            "bn254.ptx",
            "poseidon.ptx",
            "sha256.ptx",
            "sha256_leaves.ptx",
            "sha256_pairs_reduce.ptx",
            "sha3.ptx",
            "signature.ptx",
            "vector.ptx",
        ];
        for file in files {
            let path = Path::new(out_dir).join(file);
            let bytes = fs::read(&path).unwrap_or_else(|err| {
                panic!("required CUDA PTX artifact {path:?} is missing: {err}")
            });
            let text = std::str::from_utf8(&bytes)
                .unwrap_or_else(|err| panic!("{path:?} is not UTF-8 PTX text: {err}"));
            for directive in [".version", ".target", ".address_size", ".entry"] {
                assert!(
                    text.split_ascii_whitespace()
                        .any(|token| token == directive),
                    "{path:?} is missing required {directive} directive"
                );
            }
        }
    }
}
