//! Regenerate Norito codec sample files.
//!
//! External dependencies:
//! - `clap` for CLI parsing
//! - `color-eyre` for error handling

use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use color_eyre::eyre::Result;
use iroha_data_model::{account::NewAccount, ipfs::IpfsPath, metadata::Metadata, name::Name};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use norito::codec::Encode;

/// Regenerate Norito codec sample files used by `kagami`.
#[derive(Parser)]
struct Args {
    /// Output directory for regenerated samples
    #[arg(long, default_value = "crates/iroha_kagami/samples/codec")]
    out: PathBuf,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    regenerate_account(&args.out)?;
    regenerate_domain(&args.out)?;
    regenerate_executor_to()?;

    Ok(())
}

fn regenerate_account(out_dir: &PathBuf) -> Result<()> {

    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("hat")?,
        Json::from(norito::json!({ "Name": "white" })),
    );

    let account = NewAccount {
        id: (*ALICE_ID).clone(),
        metadata,
        label: None,
    };

    let json = norito::json::to_json_pretty(&account)?;
    fs::write(out_dir.join("account.json"), format!("{json}\n"))?;
    let bin = account.encode();
    fs::write(out_dir.join("account.bin"), bin)?;
    Ok(())
}

fn regenerate_domain(out_dir: &PathBuf) -> Result<()> {
    let mut metadata = Metadata::default();
    metadata.insert(Name::from_str("Is_Jabberwocky_alive")?, Json::from(true));

    let domain = iroha_data_model::domain::Domain::new("wonderland".parse()?)
        .with_logo(IpfsPath::from_str(
            "/ipfs/Qme7ss3ARVgxv6rXqVPiikMJ8u2NLgmgszg13pYrDKEoiu",
        )?)
        .with_metadata(metadata);

    let json = norito::json::to_json_pretty(&domain)?;
    fs::write(out_dir.join("domain.json"), format!("{json}\n"))?;
    let bin = domain.encode();
    fs::write(out_dir.join("domain.bin"), bin)?;
    Ok(())
}

const WIDE_IMM_MIN: i32 = -128;
const WIDE_IMM_MAX: i32 = 127;

fn chunk_immediate(mut value: i32) -> i8 {
    if value > WIDE_IMM_MAX {
        value = WIDE_IMM_MAX;
    } else if value < WIDE_IMM_MIN {
        value = WIDE_IMM_MIN;
    }
    value as i8
}

fn emit_addi_inplace(code: &mut Vec<u32>, reg: u8, mut value: i32) {
    while value != 0 {
        let chunk = chunk_immediate(value);
        code.push(ivm::kotodama::wide::encode_addi(reg, reg, chunk));
        value -= chunk as i32;
    }
}

fn set_reg(code: &mut Vec<u32>, reg: u8, value: i32) {
    code.push(ivm::kotodama::wide::encode_move(reg, 0));
    emit_addi_inplace(code, reg, value);
}

fn build_copy_program(data_offset: i32, chunks: usize) -> Vec<u32> {
    const SRC_PTR: u8 = 12;
    const DST_PTR: u8 = 13;
    const TEMP: u8 = 14;

    let mut code = Vec::new();
    set_reg(&mut code, SRC_PTR, data_offset);
    code.push(ivm::kotodama::wide::encode_move(DST_PTR, 10));

    for _ in 0..chunks {
        code.push(ivm::kotodama::wide::encode_load64(SRC_PTR, TEMP, 0));
        code.push(ivm::kotodama::wide::encode_store64(DST_PTR, TEMP, 0));
        emit_addi_inplace(&mut code, SRC_PTR, 8);
        emit_addi_inplace(&mut code, DST_PTR, 8);
    }

    code.push(ivm::encoding::wide::encode_halt());
    code
}

fn regenerate_executor_to() -> Result<()> {
    use norito::codec::Encode;
    use std::{fs, mem::size_of, path::PathBuf};

    // 1) Build Norito-encoded Result<(), ValidationFail>::Ok(())
    let verdict: Result<(), iroha_data_model::ValidationFail> = Ok(());
    let verdict_bytes = verdict.encode();
    let total_len = size_of::<usize>() as u64 + verdict_bytes.len() as u64;

    // 2) Data blob = [u64 total_len] + verdict_bytes (padded to 8-byte boundary)
    let mut data = Vec::new();
    data.extend_from_slice(&total_len.to_le_bytes());
    data.extend_from_slice(&verdict_bytes);
    while data.len() % 8 != 0 {
        data.push(0);
    }
    let chunk_count = data.len() / 8;

    // 3) Generate copy program using wide opcodes
    let mut data_offset: i32 = 0;
    let mut code: Vec<u32>;
    loop {
        code = build_copy_program(data_offset, chunk_count);
        let new_offset = (code.len() as i32) * 4;
        if new_offset == data_offset {
            break;
        }
        data_offset = new_offset;
    }
    if (code.len() as i32) * 4 != data_offset {
        code = build_copy_program(data_offset, chunk_count);
    }

    // 4) Assemble final program
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000_000,
        abi_version: 1,
    };
    let mut program = meta.encode();
    for instr in &code {
        program.extend_from_slice(&instr.to_le_bytes());
    }
    program.extend_from_slice(&data);

    let out = PathBuf::from("defaults/executor.to");
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, &program)?;
    eprintln!("Wrote {} ({} bytes)", out.display(), program.len());
    Ok(())
}
