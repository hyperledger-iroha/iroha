//! Prebuild minimal IVM sample bytecode files for integration tests.
//!
//! This utility writes small placeholder `.to` files under
//! `crates/ivm/target/prebuilt/samples/` and a `build_config.toml` indicating
//! the current profile. The integration tests and CLI look for these files by
//! name when performing executor upgrades.
//!
//! Usage:
//!   cargo run -p ivm --bin ivm_prebuild

use std::{env, fs, io::Write, path::PathBuf};

const DEFAULT_MAX_CYCLES: u64 = 1_000_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let prebuilt_dir = crate_dir.join("target/prebuilt");
    let samples_dir = prebuilt_dir.join("samples");
    fs::create_dir_all(&samples_dir)?;

    // Record the build profile so tests can adjust expectations if needed
    let profile = if cfg!(debug_assertions) {
        "Debug"
    } else {
        "Release"
    };
    let build_config = format!("profile = \"{profile}\"\n");
    fs::write(
        prebuilt_dir.join("build_config.toml"),
        build_config.as_bytes(),
    )?;

    // Sample names expected by integration tests
    let samples = [
        // executors used by some benches/tests
        "executor_with_admin",
        "executor_with_custom_permission",
        "executor_remove_permission",
        "executor_custom_instructions_simple",
        "executor_custom_instructions_complex",
        "executor_with_migration_fail",
        "executor_with_fuel",
        // IVM samples referenced by integration tests
        "mint_rose_trigger",
        "create_nft_for_every_user_trigger",
        "query_assets_and_save_cursor",
        "smart_contract_can_filter_queries",
        "trigger_cat_and_mouse",
    ];

    for (i, name) in samples.iter().enumerate() {
        let mut path = samples_dir.join(name);
        path.set_extension("to");
        let mut file = fs::File::create(&path)?;
        // Create realistic bytecode for known samples; fall back to a minimal HALT
        // program for others.
        let payload = match *name {
            // Mint 1 unit of rose#wonderland to current authority using sentinel inputs
            "mint_rose_trigger" => build_program_mint_rose_for_authority(),
            // Convenience: create one NFT per known account
            "create_nft_for_every_user_trigger" => build_program_create_nft_for_authority(),
            // Store a ForwardCursor under key "cursor" in the authority's metadata (WAT template)
            "query_assets_and_save_cursor" => build_program_set_account_detail_defaults(),
            // Just succeed; embed a harmless metadata write so the executor path succeeds
            "smart_contract_can_filter_queries" => build_program_set_account_detail_defaults(),
            // Set SmartContract execution depth parameter to 111
            "trigger_cat_and_mouse" => build_program_set_sc_exec_depth(111),
            _ => build_minimal_valid_program(i as u8),
        };
        file.write_all(&payload)?;
        eprintln!("wrote {} ({} bytes)", path.display(), payload.len());
    }

    Ok(())
}

fn default_max_cycles() -> u64 {
    DEFAULT_MAX_CYCLES
}

fn build_minimal_valid_program(tag: u8) -> Vec<u8> {
    // Construct a valid metadata header (IVM) and a tiny code section that
    // immediately HALTs. Prebuilt samples are not meant to exercise real
    // logic; they only need to be accepted by the VM and allow the executor
    // migration call to complete without error.
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        // Encode a tiny discriminator in the metadata so the node can
        // emulate specific sample behaviours (e.g., force a migration
        // failure for `executor_with_migration_fail`). Zero selects the
        // host maximum, so use `tag + 1` to avoid zero.
        vector_length: tag.saturating_add(1),
        // Provide a sensible cycle budget as part of metadata; the program
        // halts on the first instruction so this is not actually consumed.
        max_cycles: default_max_cycles(),
        abi_version: 1,
    };
    const PAD_LEN: usize = 64;
    const PAD_LEN_U32: u32 = PAD_LEN as u32;
    let mut v = meta.encode();

    // Reserve a literal section filled with zeros so padding does not become
    // executable code.
    v.extend_from_slice(b"LTLB");
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&PAD_LEN_U32.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend(std::iter::repeat_n(0u8, PAD_LEN));

    // Emit a single HALT instruction using the native wide opcode encoding so
    // `IVM::run()` returns immediately.
    let halt = ivm::encoding::wide::encode_halt().to_le_bytes();
    v.extend_from_slice(&halt);

    v
}

fn build_program_mint_rose_for_authority() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};
    let mut code = Vec::new();
    // a0 (x10) = 0 => authority sentinel
    code.extend_from_slice(
        &encode_addi(10, 0, 0)
            .expect("encode addi")
            .to_le_bytes(),
    );
    // a1 (x11) = 0 => asset_def sentinel (rose#wonderland)
    code.extend_from_slice(
        &encode_addi(11, 0, 0)
            .expect("encode addi")
            .to_le_bytes(),
    );
    // a2 (x12) = 1 => amount
    code.extend_from_slice(
        &encode_addi(12, 0, 1)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(wide::system::SCALL, ivm_sys::SYSCALL_MINT_ASSET as u8)
            .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.extend_from_slice(&[1, 0, 0, 4]);
    v.extend_from_slice(&default_max_cycles().to_le_bytes());
    v.push(1); // abi_version
    v.extend_from_slice(&code);
    v
}

fn build_program_create_nft_for_authority() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, syscalls as ivm_sys};
    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm_sys::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.extend_from_slice(&[1, 0, 0, 4]);
    v.extend_from_slice(&default_max_cycles().to_le_bytes());
    v.push(1);
    v.extend_from_slice(&code);
    v
}

fn build_program_set_sc_exec_depth(depth: u8) -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};
    let mut code = Vec::new();
    code.extend_from_slice(
        &encode_addi(10, 0, depth.into())
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm_sys::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.extend_from_slice(&[1, 0, 0, 4]);
    v.extend_from_slice(&default_max_cycles().to_le_bytes());
    v.push(1);
    v.extend_from_slice(&code);
    v
}

fn build_program_set_account_detail_defaults() -> Vec<u8> {
    use ivm::{encoding, instruction::wide, kotodama::compiler::encode_addi, syscalls as ivm_sys};

    // Use host-provided sentinels for authority/key/value to enqueue a
    // SetKeyValue instruction storing ForwardCursor{"sc_dummy",1}.
    let mut code = Vec::new();
    code.extend_from_slice(
        &encode_addi(10, 0, 0)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encode_addi(11, 0, 0)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encode_addi(12, 0, 0)
            .expect("encode addi")
            .to_le_bytes(),
    );
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            wide::system::SCALL,
            ivm_sys::SYSCALL_SET_ACCOUNT_DETAIL as u8,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parses_with_abi(bytes: &[u8]) {
        let parsed = ivm::ProgramMetadata::parse(bytes).expect("metadata parses");
        assert_eq!(parsed.metadata.abi_version, 1);
    }

    #[test]
    fn fallback_samples_encode_valid_headers() {
        assert_parses_with_abi(&build_program_mint_rose_for_authority());
        assert_parses_with_abi(&build_program_create_nft_for_authority());
        assert_parses_with_abi(&build_program_set_account_detail_defaults());
        assert_parses_with_abi(&build_program_set_sc_exec_depth(5));
    }
}
