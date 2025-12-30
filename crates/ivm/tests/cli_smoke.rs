use std::{fs, path::PathBuf, process::Command};

#[test]
fn koto_compile_meta_header_smoke() {
    // Path to the compiled CLI binary provided by Cargo.
    let bin = env!("CARGO_BIN_EXE_koto_compile");
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
        .arg(input.as_os_str())
        .arg("--out")
        .arg(out.as_os_str())
        .status()
        .expect("spawn CLI");
    assert!(status.success(), "CLI did not exit successfully");

    let bytes = fs::read(&out).expect("read output .to");
    let parsed = ivm::ProgramMetadata::parse(&bytes).expect("parse header");
    let meta = parsed.metadata;
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 8);
    assert_eq!(meta.max_cycles, 2000);
    assert_ne!(meta.mode & ivm::ivm_mode::ZK, 0);
    assert_ne!(meta.mode & ivm::ivm_mode::VECTOR, 0);
}

#[test]
fn compile_and_run_tuple_return_minimal() {
    // Minimal tuple return: return (a, b) into r10/r11
    let src = "fn f(a, b) -> (int, int) { return (a, b); }";
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile tuple return");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.set_register(10, 7); // a
    vm.set_register(11, 9); // b
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 7);
    assert_eq!(vm.register(11), 9);
}

#[test]
fn koto_compile_manifest_out_smoke() {
    // Path to CLI binary and sample input
    let bin = env!("CARGO_BIN_EXE_koto_compile");
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
}

#[test]
fn koto_compile_manifest_out_stdout_smoke() {
    let bin = env!("CARGO_BIN_EXE_koto_compile");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let input = PathBuf::from(manifest_dir)
        .join("docs")
        .join("examples")
        .join("10_meta_header.ko");

    let out_to = PathBuf::from(manifest_dir)
        .join("target")
        .join("cli_smoke_manifest_stdout.to");

    let output = std::process::Command::new(bin)
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
}
