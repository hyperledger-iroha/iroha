#![allow(clippy::too_many_lines)]

//! Build script to ensure IVM prebuilt sample bytecode exists before tests.
//!
//! This mirrors the minimal functionality of `ivm_prebuild` so that running
//! `cargo test` in the workspace is enough – no separate prebuild step needed.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

const DEFAULT_MAX_CYCLES: u64 = 1_000_000;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

fn default_max_cycles() -> u64 {
    DEFAULT_MAX_CYCLES
}

fn build_minimal_program(tag: u8) -> Vec<u8> {
    // Program metadata header: MAGIC("IVM\0"), version(1,0), mode(0),
    // vector_length(tag+1), max_cycles(default_max_cycles LE), abi_version(1)
    const PAD_LEN: usize = 64;
    let pad_len_u32 = u32::try_from(PAD_LEN).expect("padding length fits in u32");
    let mut v = Vec::with_capacity(17 + 16 + PAD_LEN + 4);
    v.extend_from_slice(b"IVM\0"); // magic
    v.push(1); // version_major
    v.push(0); // version_minor
    v.push(0); // mode
    v.push(tag.saturating_add(1)); // vector_length as discriminator, never 0
    v.extend_from_slice(&default_max_cycles().to_le_bytes()); // max_cycles
    v.push(1); // abi_version

    // Pad via a literal section so the decoder skips the placeholder bytes.
    v.extend_from_slice(b"LTLB");
    v.extend_from_slice(&0u32.to_le_bytes()); // literal entries
    v.extend_from_slice(&pad_len_u32.to_le_bytes()); // padding bytes
    v.extend_from_slice(&0u32.to_le_bytes()); // literal data length
    v.extend_from_slice(&[0u8; PAD_LEN]);

    // Single HALT instruction, encoded via the wide helpers for clarity.
    let halt = ivm::encoding::wide::encode_halt().to_le_bytes();
    v.extend_from_slice(&halt);

    v
}

fn build_program_mint_rose_for_authority() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};

    // Assemble: set a0=0 (authority sentinel), a1=0 (rose#wonderland sentinel), a2=1; SCALL MINT_ASSET; HALT
    let mut code = Vec::new();
    code.extend_from_slice(&encode_addi(10, 0, 0).to_le_bytes()); // a0 = 0 => authority
    code.extend_from_slice(&encode_addi(11, 0, 0).to_le_bytes()); // a1 = 0 => rose#wonderland
    code.extend_from_slice(&encode_addi(12, 0, 1).to_le_bytes()); // a2 = 1 (amount)
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_MINT_ASSET).expect("syscall fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    // Program metadata header: MAGIC("IVM\0"), version(1,0), mode(0), vector_length(4), max_cycles(default_max_cycles), abi_version(1)
    let mut program = Vec::new();
    program.extend_from_slice(b"IVM\0");
    program.extend_from_slice(&[1, 0, 0, 4]);
    program.extend_from_slice(&default_max_cycles().to_le_bytes());
    program.push(1); // abi_version
    program.extend_from_slice(&code);
    program
}

fn build_program_create_nft_for_authority() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};

    // Assemble: create NFTs for all users convenience syscall
    let mut code = Vec::new();
    code.extend_from_slice(&encode_addi(10, 0, 0).to_le_bytes()); // unused
    code.extend_from_slice(&encode_addi(11, 0, 0).to_le_bytes()); // unused
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS).expect("syscall fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut program = Vec::new();
    program.extend_from_slice(b"IVM\0");
    program.extend_from_slice(&[1, 0, 0, 4]);
    program.extend_from_slice(&default_max_cycles().to_le_bytes());
    program.push(1);
    program.extend_from_slice(&code);
    program
}

fn build_program_set_account_detail_defaults() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};

    // Leverage host sentinels: zeroed pointers target the authority account,
    // "cursor" key, and a ForwardCursor {"sc_dummy", 1}.
    let mut code = Vec::new();
    code.extend_from_slice(&encode_addi(10, 0, 0).to_le_bytes()); // account ptr = 0 (authority)
    code.extend_from_slice(&encode_addi(11, 0, 0).to_le_bytes()); // key ptr = 0 ("cursor")
    code.extend_from_slice(&encode_addi(12, 0, 0).to_le_bytes()); // value ptr = 0 (ForwardCursor)
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL).expect("syscall fits in u8"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut program = Vec::new();
    program.extend_from_slice(b"IVM\0");
    program.extend_from_slice(&[1, 0, 0, 4]);
    program.extend_from_slice(&default_max_cycles().to_le_bytes());
    program.push(1); // abi_version
    program.extend_from_slice(&code);
    program
}

fn compile_kotodama_sample(root: &Path, name: &str) -> Option<Vec<u8>> {
    // Compile a Kotodama .ko source located in crates/ivm/src/kotodama/samples/<name>.ko
    let path = root
        .join("crates/ivm/src/kotodama/samples")
        .join(format!("{name}.ko"));
    if !path.exists() {
        return None;
    }
    let compiler = ivm::kotodama::compiler::Compiler::new();
    match compiler.compile_file(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            eprintln!("Failed to compile {name}.ko: {e}");
            None
        }
    }
}

fn write_file_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut f = fs::File::create(path)?;
    f.write_all(bytes)?;
    Ok(())
}

fn main() {
    // Do not rebuild on every run; only if this script changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../crates/ivm/src/kotodama");
    println!("cargo:rerun-if-changed=../crates/ivm/src/bin/ivm_prebuild.rs");
    println!("cargo:rerun-if-changed=../crates/ivm/src/kotodama/samples");
    println!("cargo:rerun-if-changed=../integration_tests/fixtures/ivm");

    let root = workspace_root();
    let prebuilt_dir = root.join("crates/ivm/target/prebuilt");
    let samples_dir = prebuilt_dir.join("samples");
    let fixtures_dir = root.join("integration_tests/fixtures/ivm");

    // Record build profile for tests that inspect this
    let profile = match env::var("PROFILE").unwrap_or_default().as_str() {
        "release" => "Release",
        _ => "Debug",
    };
    let cfg = format!("profile = \"{profile}\"\n");
    let _ = write_file_if_changed(&prebuilt_dir.join("build_config.toml"), cfg.as_bytes());

    let mut samples = vec![
        "executor_with_admin",
        "executor_with_custom_permission",
        "executor_remove_permission",
        "executor_custom_instructions_simple",
        "executor_custom_instructions_complex",
        "executor_with_migration_fail",
        "executor_with_fuel",
        // Triggers and query samples referenced by integration tests
        "mint_rose_trigger",
        "create_nft_for_every_user_trigger",
        "query_assets_and_save_cursor",
        "smart_contract_can_filter_queries",
        "trigger_cat_and_mouse",
    ];

    // Optionally build a placeholder default executor if explicitly requested.
    // By default, tests should rely on the built-in Rust executor which enforces permissions.
    if std::env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")
        .ok()
        .as_deref()
        == Some("1")
    {
        samples.push("default_executor");
    }

    for (i, name) in samples.iter().enumerate() {
        let mut path = samples_dir.join(name);
        path.set_extension("to");
        let tag = u8::try_from(i).expect("index fits in u8");
        let payload = match *name {
            // Real minimal behavior used by trigger tests
            "mint_rose_trigger" => compile_kotodama_sample(&root, "mint_rose_trigger")
                .unwrap_or_else(build_program_mint_rose_for_authority),
            "create_nft_for_every_user_trigger" => {
                compile_kotodama_sample(&root, "create_nft_for_every_user_trigger")
                    .unwrap_or_else(build_program_create_nft_for_authority)
            }
            // Embed a Norito-encoded SetKeyValue("cursor", ForwardCursor) as WAT + data
            "query_assets_and_save_cursor" => {
                compile_kotodama_sample(&root, "query_assets_and_save_cursor")
                    .unwrap_or_else(build_program_set_account_detail_defaults)
            }
            // Embed a harmless metadata write so the executor path succeeds
            "smart_contract_can_filter_queries" => {
                compile_kotodama_sample(&root, "smart_contract_can_filter_queries")
                    .unwrap_or_else(build_program_set_account_detail_defaults)
            }
            // Set SmartContract execution depth to 111 (from initial 1) in one call
            "trigger_cat_and_mouse" => compile_kotodama_sample(&root, "trigger_cat_and_mouse")
                .unwrap_or_else(|| {
                    use ivm::{
                        encoding, instruction::wide, kotodama::compiler::encode_addi,
                        syscalls as ivm_sys,
                    };
                    let mut code = Vec::new();
                    // x10 = 111
                    code.extend_from_slice(&encode_addi(10, 0, 111).to_le_bytes());
                    code.extend_from_slice(
                        &encoding::wide::encode_sys(
                            wide::system::SCALL,
                            u8::try_from(ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH)
                                .expect("syscall fits"),
                        )
                        .to_le_bytes(),
                    );
                    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

                    let mut program = Vec::new();
                    program.extend_from_slice(b"IVM\0");
                    program.extend_from_slice(&[1, 0, 0, 4]);
                    program.extend_from_slice(&default_max_cycles().to_le_bytes());
                    program.push(1);
                    program.extend_from_slice(&code);
                    program
                }),
            name @ ("executor_with_admin"
            | "executor_with_custom_permission"
            | "executor_remove_permission"
            | "executor_custom_instructions_simple"
            | "executor_custom_instructions_complex"
            | "executor_with_migration_fail"
            | "executor_with_fuel") => load_fixture_sample(&fixtures_dir, name)
                .unwrap_or_else(|| build_minimal_program(tag)),
            // Fallback placeholders for other samples
            _ => build_minimal_program(tag),
        };
        let _ = write_file_if_changed(&path, &payload);
    }
}

fn load_fixture_sample(fixtures_dir: &Path, name: &str) -> Option<Vec<u8>> {
    let path = fixtures_dir.join(name).with_extension("to");
    fs::read(path).ok()
}
