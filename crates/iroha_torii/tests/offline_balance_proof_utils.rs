#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Offline balance proof test utilities.
#![allow(dead_code)]

use iroha_core::smartcontracts::isi::offline::{build_balance_proof, compute_commitment};
use iroha_data_model::{
    ChainId,
    offline::{OfflineAllowanceCommitment, OfflineBalanceProof},
};
use iroha_primitives::numeric::Numeric;

/// Build a fixed-width scalar byte array with the provided low byte.
pub fn scalar_bytes(value: u8) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[0] = value;
    bytes
}

/// Build a balance proof payload for an allowance with explicit blinding seeds.
pub fn build_balance_proof_for_allowance(
    chain_id: &ChainId,
    allowance: &OfflineAllowanceCommitment,
    claimed_delta: &Numeric,
    initial_blinding: [u8; 32],
    resulting_blinding: [u8; 32],
) -> OfflineBalanceProof {
    let expected_scale = allowance.amount.scale();
    // Offline balance commitments track consumed value progression, not allowance cap.
    let initial_value = Numeric::new(0u64, expected_scale);
    let initial_commitment = compute_commitment(&initial_value, expected_scale, &initial_blinding)
        .expect("initial commitment");
    let resulting_value = claimed_delta.clone();
    let resulting_commitment =
        compute_commitment(&resulting_value, expected_scale, &resulting_blinding)
            .expect("resulting commitment");
    let zk_proof = build_balance_proof(
        chain_id,
        expected_scale,
        claimed_delta,
        &resulting_value,
        &initial_commitment,
        &resulting_commitment,
        &initial_blinding,
        &resulting_blinding,
    )
    .expect("balance proof");

    OfflineBalanceProof {
        initial_commitment: OfflineAllowanceCommitment {
            asset: allowance.asset.clone(),
            amount: allowance.amount.clone(),
            commitment: initial_commitment,
        },
        resulting_commitment,
        claimed_delta: claimed_delta.clone(),
        zk_proof: Some(zk_proof),
    }
}
