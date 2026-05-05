//! Dump every instruction from a Norito-framed genesis block.

use std::{env, fs};

use eyre::{Result, eyre};
use iroha_data_model::{
    block::decode_framed_signed_block,
    isi::{
        GrantBox, TransferBox, asset_alias::SetAssetDefinitionAlias, mint_burn::MintBox,
        register::RegisterBox,
    },
    transaction::Executable,
};

fn main() -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let path = env::args()
        .nth(1)
        .ok_or_else(|| eyre!("usage: genesis_dump <genesis.nrt>"))?;
    let bytes = fs::read(&path)?;
    let block = decode_framed_signed_block(&bytes)?;

    println!(
        "tx_count={} result_count={}",
        block.external_transactions().len(),
        block.results().len()
    );
    for (tx_index, tx) in block.external_transactions().enumerate() {
        match tx.instructions() {
            Executable::Instructions(batch) => {
                println!("tx[{tx_index}] instructions={}", batch.len());
                for (instr_index, instruction) in batch.iter().enumerate() {
                    if let Some(transfer) = instruction.as_any().downcast_ref::<TransferBox>() {
                        println!("tx[{tx_index}].instr[{instr_index}] transfer={transfer:?}");
                    } else if let Some(register) =
                        instruction.as_any().downcast_ref::<RegisterBox>()
                    {
                        println!("tx[{tx_index}].instr[{instr_index}] register={register:?}");
                    } else if let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() {
                        println!("tx[{tx_index}].instr[{instr_index}] mint={mint:?}");
                    } else if let Some(grant) = instruction.as_any().downcast_ref::<GrantBox>() {
                        println!("tx[{tx_index}].instr[{instr_index}] grant={grant:?}");
                    } else if let Some(alias) = instruction
                        .as_any()
                        .downcast_ref::<SetAssetDefinitionAlias>()
                    {
                        println!("tx[{tx_index}].instr[{instr_index}] set_alias={alias:?}");
                    } else {
                        println!("tx[{tx_index}].instr[{instr_index}] {:?}", instruction);
                    }
                }
            }
            other => {
                println!("tx[{tx_index}] executable={other:?}");
            }
        }
    }

    Ok(())
}
