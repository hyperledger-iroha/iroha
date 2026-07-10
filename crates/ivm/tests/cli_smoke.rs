use std::{fs, path::PathBuf, process::Command};

#[test]
fn koto_build_meta_header_smoke() {
    // Path to the compiled CLI binary provided by Cargo.
    let bin = env!("CARGO_BIN_EXE_koto");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let input = PathBuf::from(manifest_dir)
        .join("docs")
        .join("examples")
        .join("10_meta_header.ko");

    // Output into a temporary file under target dir to avoid permissions issues.
    let out = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_meta.to");
    let status = Command::new(bin)
        .arg("build")
        .arg(input.as_os_str())
        .arg("--out")
        .arg(out.as_os_str())
        .arg("--max-cycles")
        .arg("2000")
        .status()
        .expect("spawn CLI");
    assert!(status.success(), "CLI did not exit successfully");

    let bytes = fs::read(&out).expect("read output .to");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse header");
    let meta = parsed.metadata;
    assert_eq!(
        meta.version_minor, 1,
        "contract artifacts must emit IVM 1.1"
    );
    assert!(
        parsed.contract_interface.is_some(),
        "compiled contract must embed a CNTR section",
    );
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 0);
    assert_eq!(meta.max_cycles, 2000);
    assert_ne!(meta.mode & ivm::ivm_mode::ZK, 0);
    assert_eq!(meta.mode & ivm::ivm_mode::VECTOR, 0);
}

#[test]
fn compile_tuple_return_minimal() {
    let src = "seiyaku Tuple { view fn pair(a: i64, b: i64) -> (i64, i64) { return (a, b); } }";
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile tuple return");
    let parsed = ivm::ProgramMetadata::parse(&code).expect("parse compiled tuple contract");
    assert!(parsed.contract_interface.is_some());
}

#[test]
fn koto_build_manifest_out_smoke() {
    // Path to CLI binary and sample input
    let bin = env!("CARGO_BIN_EXE_koto");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let input = PathBuf::from(manifest_dir)
        .join("docs")
        .join("examples")
        .join("10_meta_header.ko");

    // Output paths
    let out_to = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_manifest.to");
    let out_manifest = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_manifest.json");

    let status = std::process::Command::new(bin)
        .arg("build")
        .arg(input.as_os_str())
        .arg("--out")
        .arg(out_to.as_os_str())
        .arg("--manifest-out")
        .arg(out_manifest.as_os_str())
        .status()
        .expect("spawn CLI");
    assert!(status.success(), "CLI did not exit successfully");

    // Read and sanity-check manifest JSON
    let s = std::fs::read_to_string(&out_manifest).expect("read manifest json");
    assert!(s.contains("abi_hash"), "manifest JSON missing abi_hash");
    assert!(
        s.contains("compiler_fingerprint"),
        "manifest JSON missing compiler_fingerprint",
    );
}

#[test]
fn koto_build_manifest_out_stdout_smoke() {
    let bin = env!("CARGO_BIN_EXE_koto");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let input = PathBuf::from(manifest_dir)
        .join("docs")
        .join("examples")
        .join("10_meta_header.ko");

    let out_to = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_manifest_stdout.to");
    let sibling_manifest = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_manifest_stdout.manifest.json");
    let _ = fs::remove_file(&sibling_manifest);

    let output = std::process::Command::new(bin)
        .arg("build")
        .arg(input.as_os_str())
        .arg("--out")
        .arg(out_to.as_os_str())
        .arg("--manifest-out")
        .arg("-")
        .output()
        .expect("spawn CLI");
    assert!(output.status.success(), "CLI did not exit successfully");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("abi_hash"), "stdout missing manifest JSON");
    assert!(
        stdout.contains("compiler_fingerprint"),
        "stdout missing compiler_fingerprint",
    );
    assert!(
        !sibling_manifest.exists(),
        "stdout mode must not publish an unexpected sibling manifest"
    );
}

#[test]
fn koto_build_verify_is_read_only_and_fails_on_tampering() {
    let bin = env!("CARGO_BIN_EXE_koto");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let input = manifest_dir
        .join("docs")
        .join("examples")
        .join("01_hajimari.ko");
    let target = manifest_dir.join("target");
    let artifact = target.join("cli_smoke_verify.to");
    let manifest = target.join("cli_smoke_verify.manifest.json");
    let record = target
        .join(".fingerprints")
        .join("cli_smoke_verify.to.record");
    for path in [&artifact, &manifest, &record] {
        let _ = fs::remove_file(path);
    }

    let initial = Command::new(bin)
        .arg("build")
        .arg(&input)
        .arg("--out")
        .arg(&artifact)
        .status()
        .expect("spawn initial build");
    assert!(initial.success(), "initial build failed");
    let before = fs::metadata(&artifact)
        .expect("artifact metadata")
        .modified()
        .ok();

    let verified = Command::new(bin)
        .arg("build")
        .arg("--verify")
        .arg(&input)
        .arg("--out")
        .arg(&artifact)
        .status()
        .expect("spawn verify build");
    assert!(verified.success(), "current outputs must verify");
    assert_eq!(
        fs::metadata(&artifact)
            .expect("verified artifact metadata")
            .modified()
            .ok(),
        before,
        "verification must not rewrite a current output"
    );

    fs::write(&artifact, b"tampered").expect("tamper artifact");
    let rejected = Command::new(bin)
        .arg("build")
        .arg("--verify")
        .arg(&input)
        .arg("--out")
        .arg(&artifact)
        .status()
        .expect("spawn tamper verification");
    assert!(
        !rejected.success(),
        "tampered output must fail verification"
    );
    assert_eq!(
        fs::read(&artifact).expect("read tampered artifact"),
        b"tampered",
        "verification must never repair or otherwise mutate stale output"
    );
    for path in [&artifact, &manifest, &record] {
        let _ = fs::remove_file(path);
    }
}
