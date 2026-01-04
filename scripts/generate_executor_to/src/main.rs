//! Generates a minimal IVM executor bytecode blob and writes it to disk.
use std::{fs, mem::size_of, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use iroha_data_model::ValidationFail;
use ivm::{encoding, kotodama::wide, ProgramMetadata};
use norito::codec::Encode;

#[derive(Parser, Debug)]
struct Args {
    /// Output path for the generated executor bytecode
    #[arg(long, default_value = "defaults/executor.to")]
    out: PathBuf,
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
        code.push(wide::encode_addi(reg, reg, chunk));
        value -= chunk as i32;
    }
}

fn set_reg(code: &mut Vec<u32>, reg: u8, value: i32) {
    code.push(wide::encode_move(reg, 0));
    emit_addi_inplace(code, reg, value);
}

fn build_copy_program(data_offset: i32, chunks: usize) -> Vec<u32> {
    const SRC_PTR: u8 = 12;
    const DST_PTR: u8 = 13;
    const TEMP: u8 = 14;

    let mut code = Vec::new();
    set_reg(&mut code, SRC_PTR, data_offset);
    code.push(wide::encode_move(DST_PTR, 10));

    for _ in 0..chunks {
        code.push(wide::encode_load64(SRC_PTR, TEMP, 0));
        code.push(wide::encode_store64(DST_PTR, TEMP, 0));
        emit_addi_inplace(&mut code, SRC_PTR, 8);
        emit_addi_inplace(&mut code, DST_PTR, 8);
    }

    code.push(encoding::wide::encode_halt());
    code
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Build Norito-encoded verdict: Ok(()) of type Result<(), ValidationFail>
    let verdict: Result<(), ValidationFail> = Ok(());
    let verdict_bytes = verdict.encode();
    let total_len = size_of::<usize>() as u64 + verdict_bytes.len() as u64;

    // Data blob: [u64 total_len LE] + verdict_bytes
    let mut data = Vec::new();
    data.extend_from_slice(&total_len.to_le_bytes());
    data.extend_from_slice(&verdict_bytes);
    while data.len() % 8 != 0 {
        data.push(0);
    }
    let chunk_count = data.len() / 8;

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
    // Ensure code is built with the stabilised offset.
    if (code.len() as i32) * 4 != data_offset {
        code = build_copy_program(data_offset, chunk_count);
    }

    // Assemble final program: IVM header + code (little-endian) + data
    let meta = ProgramMetadata {
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

    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.out, &program)?;
    eprintln!("Wrote {} ({} bytes)", args.out.display(), program.len());

    Ok(())
}
