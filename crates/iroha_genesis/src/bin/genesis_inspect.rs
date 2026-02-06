//! Inspect a Norito-framed genesis block for validator and `PoP` registrations.

use std::{collections::BTreeMap, env, fs};

use eyre::{Result, eyre};
use iroha_data_model::{
    block::decode_framed_signed_block,
    isi::{
        register::RegisterPeerWithPop,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    transaction::Executable,
};

fn main() -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let path = env::args()
        .nth(1)
        .ok_or_else(|| eyre!("usage: genesis_inspect <genesis.nrt>"))?;
    let bytes = fs::read(&path)?;
    let block = decode_framed_signed_block(&bytes)?;

    let mut peers = Vec::new();
    let mut validators = Vec::new();
    let mut activations = Vec::new();

    for tx in block.external_transactions() {
        if let Executable::Instructions(batch) = tx.instructions() {
            for instr in batch {
                if let Some(peer) = instr.as_any().downcast_ref::<RegisterPeerWithPop>() {
                    peers.push((
                        peer.peer.clone(),
                        peer.pop.len(),
                        peer.activation_at,
                        peer.expiry_at,
                    ));
                    continue;
                }
                if let Some(reg) = instr.as_any().downcast_ref::<RegisterPublicLaneValidator>() {
                    validators.push((
                        *reg.lane_id(),
                        reg.validator().clone(),
                        reg.stake_account().clone(),
                        reg.initial_stake().clone(),
                    ));
                    continue;
                }
                if let Some(act) = instr.as_any().downcast_ref::<ActivatePublicLaneValidator>() {
                    activations.push((*act.lane_id(), act.validator().clone()));
                }
            }
        }
    }

    println!(
        "event=genesis_inspect peers_with_pop={count}",
        count = peers.len()
    );
    for (peer, pop_len, activation_at, expiry_at) in &peers {
        println!(
            "peer={peer} pop_len={pop_len} activation_at={activation_at:?} expiry_at={expiry_at:?}"
        );
    }

    println!(
        "event=genesis_inspect validators={count}",
        count = validators.len()
    );
    let mut validator_index = BTreeMap::new();
    for (lane, validator, stake_account, stake) in &validators {
        println!(
            "validator={validator} lane={lane} stake_account={stake_account} initial_stake={stake}"
        );
        validator_index
            .entry((*lane, validator.clone()))
            .or_insert_with(|| (stake_account.clone(), stake.clone()));
    }

    println!(
        "event=genesis_inspect activations={count}",
        count = activations.len()
    );
    for (lane, validator) in &activations {
        println!("activation lane={lane} validator={validator}");
    }

    Ok(())
}
