//! Native process double for Mochi's generated-identity and listener-handoff tests.
//!
//! This checks the config-preflight invocation, not daemon configuration semantics.
//! The real generator validates its configuration; native startup has separate coverage.

use std::{ffi::OsStr, io, io::Write as _, path::Path, process::Command};

fn main() -> io::Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .iter()
        .any(|argument| argument == "--check-config")
    {
        let expected = Path::new(env!("MOCHI_TEST_PREFLIGHT_CONFIG"));
        if arguments.len() != 4
            || arguments[0] != "--config"
            || arguments[1] != expected.as_os_str()
            || arguments[2] != "--check-config"
            || arguments[3] != "--json"
            || !expected.is_absolute()
            || expected.file_name() != Some(OsStr::new("peer0.toml"))
            || expected.parent() != Some(std::env::current_dir()?.as_path())
        {
            return Err(io::Error::other("unexpected fixture preflight invocation"));
        }
        let metadata = std::fs::symlink_metadata(expected)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(io::Error::other("fixture peer configuration is absent"));
        }
        let mut receipt = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(env!("MOCHI_TEST_PREFLIGHT_RECEIPT"))?;
        receipt.write_all(env!("MOCHI_TEST_PREFLIGHT_CONFIG").as_bytes())?;
        println!(r#"{{"status":"ready"}}"#);
        return Ok(());
    }

    // This executable never starts a validator. Preserve the existing native libtest
    // child's refusal for runtime invocations outside the one config-preflight shape.
    let status = Command::new(env!("MOCHI_TEST_PREFLIGHT_HARNESS"))
        .args(arguments)
        .status()?;
    std::process::exit(status.code().unwrap_or(1));
}
