//! Kotodama + IVM example runner with optional external tool coverage.
//!
//! This integration test compiles examples under `examples/` and runs an entrypoint locally
//! if the external tools `koto` and `iroha` are available on PATH (or specified via
//! environment variables `KOTO_BIN` and `IROHA_BIN`). The tests self-skip when those tools are not
//! present, so they do not fail CI on hosts without the local Kotodama toolchain.
//!
//! Run manually with:
//!   cargo test -p `integration_tests` --test `nexus_and_streaming` `kotodama_examples::` -- --nocapture
use integration_tests::process::{output_with_timeout, process_timeout, status_with_timeout};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
fn on_path(bin: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for p in env::split_paths(&path) {
        let candidate = p.join(bin);
        if candidate.is_file() && is_executable(&candidate) {
            return Some(candidate);
        }
        // Windows: add .exe fallback
        #[cfg(windows)]
        {
            let candidate_exe = p.join(format!("{}.exe", bin));
            if candidate_exe.is_file() && is_executable(&candidate_exe) {
                return Some(candidate_exe);
            }
        }
    }
    None
}
#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(windows)]
fn is_executable(p: &Path) -> bool {
    p.extension().map(|e| e == "exe").unwrap_or(false)
}
fn tool(variable: &str, name: &str) -> Option<PathBuf> {
    let tool = env::var(variable)
        .ok()
        .map(PathBuf::from)
        .or_else(|| on_path(name));
    if tool.is_none() {
        eprintln!("Skipping: {variable} not set and {name} not found on PATH");
    }
    tool
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
/// Compile one example into `target/examples` and return the artifact path.
fn compile(koto: &Path, example: &str) -> PathBuf {
    let root = root();
    let source = root.join(format!("examples/{example}/{example}.ko"));
    let out = root.join(format!("target/examples/{example}.to"));
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let mut command = Command::new(koto);
    command.arg("build").arg(&source).arg("--out").arg(&out);
    let status =
        status_with_timeout(&mut command, process_timeout()).expect("failed to spawn koto");
    assert!(status.success(), "koto build failed: {status:?}");
    out
}
/// Run one entrypoint locally and require the debugger to report success.
fn debug_call(iroha: &Path, artifact: &Path, entrypoint: &str) {
    let mut command = Command::new(iroha);
    command
        .arg("--config")
        .arg(root().join("defaults/client.toml"))
        .arg("--output-format")
        .arg("json")
        .args(["contract", "debug-call", "--code-file"])
        .arg(artifact)
        .args(["--entrypoint", entrypoint]);
    let output =
        output_with_timeout(&mut command, process_timeout()).expect("failed to spawn iroha");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("\"ok\": true"),
        "debug-call failed: status={:?}\nstdout=\n{stdout}\nstderr=\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn compile_and_run_hello() {
    let (Some(koto), Some(iroha)) = (tool("KOTO_BIN", "koto"), tool("IROHA_BIN", "iroha")) else {
        return;
    };
    let artifact = compile(&koto, "hello");
    debug_call(&iroha, &artifact, "hajimari");
}
#[test]
fn inspect_hello() {
    let (Some(koto), Some(iroha)) = (tool("KOTO_BIN", "koto"), tool("IROHA_BIN", "iroha")) else {
        return;
    };
    let artifact = compile(&koto, "hello");
    let mut command = Command::new(&iroha);
    command
        .args([
            "--output-format",
            "json",
            "tools",
            "ivm",
            "manifest-gen",
            "--file",
        ])
        .arg(&artifact);
    let output =
        output_with_timeout(&mut command, process_timeout()).expect("failed to spawn iroha");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("code_hash") && stdout.contains("abi_hash"),
        "manifest-gen failed: status={:?}\nstdout=\n{stdout}\nstderr=\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn compile_and_run_nft() {
    let (Some(koto), Some(iroha)) = (tool("KOTO_BIN", "koto"), tool("IROHA_BIN", "iroha")) else {
        return;
    };
    let artifact = compile(&koto, "nft");
    debug_call(&iroha, &artifact, "create");
}
