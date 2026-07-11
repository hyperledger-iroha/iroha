//! Golden Norito wire vector for the Kagemusha V2 semantic-lineage DAG.

use iroha_data_model::offline::{
    KagemushaRecursiveSpendLineageNodeV2, KagemushaRecursiveSpendLineageWitnessV2,
};

#[test]
fn two_root_join_witness_matches_norito_golden() {
    let witness = KagemushaRecursiveSpendLineageWitnessV2 {
        nodes: vec![
            KagemushaRecursiveSpendLineageNodeV2 {
                result_bundle_digest: [0x10; 32],
                parent_bundle_digests: Vec::new(),
                proof_step_count: 1,
                verified_at_block_height: 100,
                transition_archive: vec![0xA1],
            },
            KagemushaRecursiveSpendLineageNodeV2 {
                result_bundle_digest: [0x20; 32],
                parent_bundle_digests: Vec::new(),
                proof_step_count: 1,
                verified_at_block_height: 100,
                transition_archive: vec![0xB2],
            },
            KagemushaRecursiveSpendLineageNodeV2 {
                result_bundle_digest: [0x30; 32],
                parent_bundle_digests: vec![[0x10; 32], [0x20; 32]],
                proof_step_count: 2,
                verified_at_block_height: 101,
                transition_archive: vec![0xC3],
            },
        ],
        final_bundle_digest: [0x30; 32],
    };

    let encoded = norito::to_bytes(&witness).expect("encode deterministic semantic DAG vector");
    assert_eq!(
        hex::encode(encoded),
        "4e52543000003604117c64ddb476ec54ce10bfd0662f00780100000000000063899240769af2d102d5020300000000000000422010101010101010101010101010101010101010101010101010101010101010100800000000000000000401000000086400000000000000090100000000000000a1422020202020202020202020202020202020202020202020202020202020202020200800000000000000000401000000086400000000000000090100000000000000b2c5012030303030303030303030303030303030303030303030303030303030303030308a010200000000000000400110011001100110011001100110011001100110011001100110011001100110011001100110011001100110011001100110011001100110011001100110011040012001200120012001200120012001200120012001200120012001200120012001200120012001200120012001200120012001200120012001200120012001200402000000086500000000000000090100000000000000c3203030303030303030303030303030303030303030303030303030303030303030"
    );
}
