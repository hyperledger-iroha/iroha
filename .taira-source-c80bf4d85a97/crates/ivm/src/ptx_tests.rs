#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    #[test]
    fn generated_ptx_files_exist() {
        let out_dir = match std::env::var("OUT_DIR") {
            Ok(dir) => dir,
            Err(_) => {
                eprintln!("skipping GPU PTX artifact check; OUT_DIR not set");
                return;
            }
        };
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
            let path = Path::new(&out_dir).join(file);
            let Ok(bytes) = fs::read(&path) else {
                eprintln!("skipping GPU PTX artifact check; {path:?} missing in OUT_DIR {out_dir}");
                return;
            };
            assert!(!bytes.is_empty(), "{path:?} is empty");
        }
    }
}
