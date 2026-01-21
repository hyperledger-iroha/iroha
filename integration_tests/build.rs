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

const LITERAL_DATA_START: i16 = 16;

fn len_u32(len: usize) -> u32 {
    u32::try_from(len).expect("length fits in u32")
}

fn syscall_u8(id: u32) -> u8 {
    u8::try_from(id).expect("syscall id fits in u8")
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    use iroha_data_model::prelude::Hash;

    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&len_u32(payload.len()).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; Hash::LENGTH] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn assemble_program_with_literals(code: &[u8], literal_data: &[u8]) -> Vec<u8> {
    let mut program = Vec::new();
    program.extend_from_slice(b"IVM\0");
    program.extend_from_slice(&[1, 0, 0, 4]);
    program.extend_from_slice(&default_max_cycles().to_le_bytes());
    program.push(1); // abi_version
    if !literal_data.is_empty() {
        program.extend_from_slice(b"LTLB");
        program.extend_from_slice(&0u32.to_le_bytes()); // literal entries
        program.extend_from_slice(&0u32.to_le_bytes()); // post-pad bytes
        program.extend_from_slice(&len_u32(literal_data.len()).to_le_bytes());
        program.extend_from_slice(literal_data);
    }
    program.extend_from_slice(code);
    program
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
    use iroha_data_model::prelude::AssetDefinitionId;
    use ivm::{
        PointerType, encoding, instruction::wide,
        kotodama::wide::encode_addi_checked as encode_addi, syscalls as ivm_sys,
    };
    use norito::codec::Encode as _;

    // Assemble: mint 1 unit of rose#wonderland to the authority with pointer-ABI inputs.
    let asset_def: AssetDefinitionId = "rose#wonderland".parse().expect("asset definition");
    let asset_payload = asset_def.encode();
    let asset_tlv = make_tlv(PointerType::AssetDefinitionId as u16, &asset_payload);

    let mut code = Vec::new();
    // r10 <- &AccountId(authority)
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_GET_AUTHORITY),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encode_addi(13, 10, 0).expect("encode addi").to_le_bytes()); // save account pointer
    // r10 <- &AssetDefinitionId (literal TLV)
    code.extend_from_slice(
        &encode_addi(10, 0, LITERAL_DATA_START)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encode_addi(11, 10, 0).expect("encode addi").to_le_bytes()); // r11 = asset ptr
    code.extend_from_slice(&encode_addi(10, 13, 0).expect("encode addi").to_le_bytes()); // r10 = account ptr
    code.extend_from_slice(&encode_addi(12, 0, 1).expect("encode addi").to_le_bytes()); // amount = 1
    code.extend_from_slice(
        &encoding::wide::encode_sys(wide::system::SCALL, syscall_u8(ivm_sys::SYSCALL_MINT_ASSET))
            .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    assemble_program_with_literals(&code, &asset_tlv)
}

fn build_program_create_nft_for_authority() -> Vec<u8> {
    use ivm::{
        encoding, instruction::wide, kotodama::wide::encode_addi_checked as encode_addi,
        syscalls as ivm_sys,
    };

    // Assemble: create NFTs for all users convenience syscall
    let mut code = Vec::new();
    code.extend_from_slice(&encode_addi(10, 0, 0).expect("encode addi").to_le_bytes()); // unused
    code.extend_from_slice(&encode_addi(11, 0, 0).expect("encode addi").to_le_bytes()); // unused
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
    use iroha_data_model::{prelude::Name, query::parameters::ForwardCursor};
    use ivm::{
        PointerType, encoding, instruction::wide,
        kotodama::wide::encode_addi_checked as encode_addi, syscalls as ivm_sys,
    };
    use norito::{codec::Encode as _, json};
    use std::num::NonZeroU64;

    let key: Name = "cursor".parse().expect("key name");
    let key_payload = key.encode();
    let cursor = ForwardCursor {
        query: "sc_dummy".to_owned(),
        cursor: NonZeroU64::new(1).expect("cursor non-zero"),
        gas_budget: None,
    };
    let value_json = json::to_vec(&cursor).expect("encode ForwardCursor");
    let key_tlv = make_tlv(PointerType::Name as u16, &key_payload);
    let value_tlv = make_tlv(PointerType::Json as u16, &value_json);
    let value_ptr = LITERAL_DATA_START + i16::try_from(key_tlv.len()).unwrap_or(0);

    let mut code = Vec::new();
    // r10 <- &AccountId(authority)
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_GET_AUTHORITY),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encode_addi(13, 10, 0).expect("encode addi").to_le_bytes()); // save account pointer
    // r10 <- &Name("cursor")
    code.extend_from_slice(
        &encode_addi(10, 0, LITERAL_DATA_START)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encode_addi(11, 10, 0).expect("encode addi").to_le_bytes()); // r11 = key ptr
    // r10 <- &Json(cursor)
    code.extend_from_slice(
        &encode_addi(10, 0, value_ptr)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_INPUT_PUBLISH_TLV),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encode_addi(12, 10, 0).expect("encode addi").to_le_bytes()); // r12 = value ptr
    code.extend_from_slice(&encode_addi(10, 13, 0).expect("encode addi").to_le_bytes()); // r10 = account ptr
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            syscall_u8(ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut literal_data = Vec::with_capacity(key_tlv.len() + value_tlv.len());
    literal_data.extend_from_slice(&key_tlv);
    literal_data.extend_from_slice(&value_tlv);
    assemble_program_with_literals(&code, &literal_data)
}

fn compile_kotodama_sample(root: &Path, name: &str) -> Option<Vec<u8>> {
    // Compile a Kotodama .ko source located in crates/kotodama_lang/src/samples/<name>.ko
    let path = root
        .join("crates/kotodama_lang/src/samples")
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
    println!("cargo:rerun-if-changed=../crates/kotodama_lang/src");
    println!("cargo:rerun-if-changed=../crates/ivm/src/bin/ivm_prebuild.rs");
    println!("cargo:rerun-if-changed=../crates/kotodama_lang/src/samples");
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
                        encoding, instruction::wide,
                        kotodama::wide::encode_addi_checked as encode_addi, syscalls as ivm_sys,
                    };
                    let mut code = Vec::new();
                    // x10 = 111
                    code.extend_from_slice(
                        &encode_addi(10, 0, 111).expect("encode addi").to_le_bytes(),
                    );
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
