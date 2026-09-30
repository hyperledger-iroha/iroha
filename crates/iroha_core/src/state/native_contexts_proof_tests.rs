//! Shared native proof cross-checks against the actual Core ordinary-write SMT.

use super::*;
use iroha_data_model::sumeragi_finality::{
    LANE_CONSENSUS_CONTEXTS_WITNESS_KEY, NativeContextsProof,
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};

fn witness() -> (NetworkId, ExecWitness) {
    let network = crate::unit_test_support::synthetic_network_id("native model proof cross-check");
    let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
        network,
        2,
        &LaneConsensusContextsV1::default(),
    )
    .unwrap();
    let mut writes = (0_u32..37)
        .map(|index| ExecKv {
            key: index.to_le_bytes().to_vec(),
            value: (index * 19).to_le_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    writes.push(ExecKv {
        key: writes[3].key.clone(),
        value: b"last original write wins".to_vec(),
    });
    writes.push(ExecKv {
        key: LANE_CONSENSUS_CONTEXTS_WITNESS_KEY.to_vec(),
        value: norito::encode_canonical(&commitment).unwrap(),
    });
    (
        network,
        ExecWitness {
            writes,
            ..ExecWitness::default()
        },
    )
}
fn core_root(witness: &ExecWitness) -> Hash {
    let (_, writes) = crate::exec_witness::roots::witness_pairs(witness);
    crate::exec_witness::smt::compute_post_state_root(&[], &writes)
}

#[test]
fn shared_fixed_proof_matches_real_core_smt_after_canonical_last_writes() {
    let (network, original) = witness();
    let demand = NativeContextsProof::scratch_bytes(original.writes.len()).unwrap();
    let budget = AllocationBudget::new(demand);
    let proof = NativeContextsProof::from_witness(&original, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(proof.verify(network, 2, core_root(&original)));
    let mut reordered = original.clone();
    reordered.writes.remove(3);
    reordered.writes.reverse();
    let repeated = NativeContextsProof::from_witness(&reordered, &budget).unwrap();
    assert!(repeated.verify(network, 2, core_root(&original)));
    assert_eq!(proof, repeated);
    let mut single = original;
    single
        .writes
        .retain(|write| write.key == LANE_CONSENSUS_CONTEXTS_WITNESS_KEY);
    assert!(
        NativeContextsProof::from_witness(&single, &budget)
            .unwrap()
            .verify(network, 2, core_root(&single))
    );
}

#[test]
fn model_scratch_refusal_retains_real_original_witness_and_same_pool_retry() {
    let (network, original) = witness();
    let demand = NativeContextsProof::scratch_bytes(original.writes.len()).unwrap();
    assert!(NativeContextsProof::scratch_bytes(usize::MAX).is_err());
    let budget = AllocationBudget::new(demand);
    let held = ChargedBuffer::<u8>::new(1, &budget).unwrap();
    let identity = original.writes.as_ptr();
    assert!(
        NativeContextsProof::from_witness(&original, &budget)
            .unwrap_err()
            .is_local_refusal()
    );
    assert_eq!(original.writes.as_ptr(), identity);
    assert_eq!(budget.reserved_bytes(), 1);
    drop(held);
    assert!(
        NativeContextsProof::from_witness(&original, &budget.clone())
            .unwrap()
            .verify(network, 2, core_root(&original))
    );
    assert_eq!(budget.reserved_bytes(), 0);
}
