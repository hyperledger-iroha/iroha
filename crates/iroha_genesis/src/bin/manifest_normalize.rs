//! Normalize a genesis manifest JSON and print its expanded instruction batches.

use std::{env, path::PathBuf};

use eyre::{Result, eyre};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    isi::SetParameter,
    parameter::{Parameter, system::SumeragiParameter},
    transaction::Executable,
};
use iroha_genesis::RawGenesisTransaction;

fn main() -> Result<()> {
    iroha_genesis::init_instruction_registry();

    let mut args = env::args().skip(1);
    let path: PathBuf = args
        .next()
        .ok_or_else(|| eyre!("usage: manifest_normalize <genesis-manifest-json>"))?
        .into();

    let manifest = RawGenesisTransaction::from_path(&path)?;
    let normalized = manifest.normalize()?;

    println!(
        "event=manifest_normalize stage=normalized path={} batches={}",
        path.display(),
        normalized.transactions.len()
    );

    for (batch_idx, batch) in normalized.transactions.iter().enumerate() {
        print_batch("normalized", batch_idx, batch);
    }

    let block = RawGenesisTransaction::from_path(&path)?.build_and_sign(&KeyPair::random())?;
    println!(
        "event=manifest_normalize stage=signed_block path={} batches={}",
        path.display(),
        block.0.external_transactions().len()
    );
    for (batch_idx, tx) in block.0.external_transactions().enumerate() {
        if let Executable::Instructions(batch) = tx.instructions() {
            print_batch("signed_block", batch_idx, &batch);
        }
    }

    Ok(())
}

fn print_batch(stage: &str, batch_idx: usize, batch: &[iroha_data_model::isi::InstructionBox]) {
    println!(
        "stage={stage} batch={batch_idx} instructions={}",
        batch.len()
    );
    for (instr_idx, instruction) in batch.iter().enumerate() {
        let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
            continue;
        };
        match set_parameter.inner() {
            Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(value)) => {
                println!(
                    "stage={stage} batch={batch_idx} instr={instr_idx} set_parameter=sumeragi.min_finality_ms value={value}"
                );
            }
            Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(value)) => {
                println!(
                    "stage={stage} batch={batch_idx} instr={instr_idx} set_parameter=sumeragi.block_time_ms value={value}"
                );
            }
            Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(value)) => {
                println!(
                    "stage={stage} batch={batch_idx} instr={instr_idx} set_parameter=sumeragi.commit_time_ms value={value}"
                );
            }
            other => {
                println!(
                    "stage={stage} batch={batch_idx} instr={instr_idx} set_parameter={other:?}"
                );
            }
        }
    }
}
