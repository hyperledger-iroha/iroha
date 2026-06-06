//! Normalize a genesis manifest JSON and print its expanded instruction batches.

use std::{env, path::PathBuf};

use eyre::{Result, WrapErr, eyre};
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

    let signer = normalization_signer()?;
    let block = RawGenesisTransaction::from_path(&path)?.build_and_sign(&signer)?;
    println!(
        "event=manifest_normalize stage=signed_block path={} batches={}",
        path.display(),
        block.0.external_transactions().len()
    );
    for (batch_idx, tx) in block.0.external_transactions().enumerate() {
        if let Executable::Instructions(batch) = tx.instructions() {
            print_batch("signed_block", batch_idx, batch);
        }
    }

    Ok(())
}

fn normalization_signer() -> Result<KeyPair> {
    KeyPair::try_random().wrap_err("failed to generate manifest normalization signer")
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

#[cfg(test)]
mod tests {
    use iroha_crypto::Algorithm;

    use super::*;

    #[test]
    fn normalization_signer_uses_checked_default_key_generation() {
        let keypair = normalization_signer().expect("checked default signer generation");

        assert_eq!(
            keypair
                .public_key()
                .try_algorithm()
                .expect("generated public key algorithm"),
            Algorithm::default()
        );
    }
}
