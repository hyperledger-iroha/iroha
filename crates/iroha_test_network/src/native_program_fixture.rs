//! One genuine native executable and the fixture roots shared by release resolver
//! admission and lifetime controls.

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

/// Create a temporary fixture root spelled without linked ancestors.
///
/// Private custody refuses linked ancestors and owner directories report resolved
/// paths, so the macOS `/var` alias must be spelled as `/private/var` here.
pub fn temporary_root() -> tempfile::TempDir {
    let parent = std::env::temp_dir();
    #[cfg(unix)]
    let parent = parent
        .canonicalize()
        .expect("canonical temporary directory");
    tempfile::tempdir_in(parent).expect("temporary fixture root")
}

/// Whether a retained release selection still admits its command.
///
/// Hosts without qualified original-descriptor execution complete the same custody
/// recheck before refusing the launch as unsupported; any other refusal is custody.
pub fn command_admitted(program: &crate::ResolvedProgram) -> bool {
    match program.command() {
        Ok(_) => true,
        Err(error) => {
            !cfg!(any(windows, target_os = "linux", target_os = "android"))
                && error.kind() == std::io::ErrorKind::Unsupported
        }
    }
}
