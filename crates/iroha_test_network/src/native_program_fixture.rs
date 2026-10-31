//! One genuine native executable shared by release resolver admission and lifetime controls.

use std::{fs, process::Command, sync::OnceLock};

/// Compile once and retain a genuine native image for command admission controls.
pub fn image() -> &'static [u8] {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let root = tempfile::tempdir().expect("native resolver probe directory");
        let source = root.path().join("resolver_probe.rs");
        let executable = root.path().join(if cfg!(windows) {
            "resolver_probe.exe"
        } else {
            "resolver_probe"
        });
        fs::write(
            &source,
            "//! Native resolver command fixture.\nfn main() { println!(\"resolver-probe\"); }\n",
        )
        .unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg("--edition=2024")
            .arg("--crate-name=resolver_probe")
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("compile genuine native resolver fixture");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(executable).expect("read compiled native resolver fixture")
    })
}
