use super::*;
use crate::vega::sponge::Keccak256;
use hex_literal::hex;

const PARAMETER_KAT_V2: [u8; 32] =
    hex!("cc56911877ef83b04c3ce879640f2943ceabe13c38a7372d5c4f69637fe77566");
const MAPPING_KATS_V2: [(u8, [u8; 32]); 4] = [
    (
        2,
        hex!("4524adfdedbeb59639b3845566f579645ea955f76024740aef83df1d09b5312d"),
    ),
    (
        9,
        hex!("77f0afab5610a33f4ce63d244a9fff8cc376251bee0f4317a221b27fc5965311"),
    ),
    (
        10,
        hex!("569ccadebbb4b1680ca3315ce19a37ecdb904f49b810340e55e2c6301547f88f"),
    ),
    (
        17,
        hex!("22944d7520aa47612b8b6cbda51e2f85a10e59ecc8d30cffa7ac23a441789d7c"),
    ),
];
const LEAF_KATS_V2: [(u8, [u8; 32]); 4] = [
    (
        2,
        hex!("35f6ccb7d2430eb7752dc6dfee01c313f457fa32ea2bf0ec818cacf2e72fc1a7"),
    ),
    (
        9,
        hex!("accf2f5401dd6f1984797639d7927af833f4536db12d7c57f2b048aab6f78dc0"),
    ),
    (
        10,
        hex!("38cbfc47e70782c0ba1c41bb2b7f35349ce9eb8dafcff7bcad8723ff518fa066"),
    ),
    (
        17,
        hex!("be522bd632a5f035b38bc0c7f448dd1462cd27cf3620aa7bf652d4e10f7726bd"),
    ),
];
const NODE_KATS_V2: [(u8, u8, [u8; 32]); 4] = [
    (
        2,
        7,
        hex!("96502bd0dcfa7dc4409d8dafee1f55e8d27dfb37d9e32888b2629e077fb82361"),
    ),
    (
        9,
        7,
        hex!("0c22309c718da7c8a06b10dbf158025bfeddef32faa62fddaa94bae48263280a"),
    ),
    (
        10,
        7,
        hex!("a58076434d910a558b2d10408f90c3cc10f12803702ef3adf23cfbe512cc036e"),
    ),
    (
        17,
        2,
        hex!("ecd99303d3927df127705bf0717d3a2da080b160910185256ceae3e92fc59e32"),
    ),
];

fn manual_mapping_v2(layer: u8) -> [u8; 32] {
    let logical_length = REPLAY_DOMAIN_VALUES_V2 >> layer;
    let values_per_block = u16::try_from(logical_length.min(1_024)).unwrap();
    let blocks = logical_length / u64::from(values_per_block);
    let slots = blocks * 380;
    let plaintext = u64::from(values_per_block) * 16;
    let file = slots * (plaintext + 16);
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.fri-layer.mapping\0");
    hash.update(&[2, 3, layer]);
    hash.update(&PARAMETER_KAT_V2);
    hash.update(&logical_length.to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(&values_per_block.to_be_bytes());
    hash.update(&blocks.to_be_bytes());
    hash.update(&slots.to_be_bytes());
    hash.update(&plaintext.to_be_bytes());
    hash.update(&file.to_be_bytes());
    hash.update(b"slot=block*380+column;first_index=block*values_per_block");
    hash.update(b"canonical Fq2=(c0,c1), each canonical big-endian u64");
    hash.update(&slots.to_be_bytes());
    for slot in 0..slots {
        let block = slot / 380;
        let column = u16::try_from(slot % 380).unwrap();
        hash.update(&slot.to_be_bytes());
        hash.update(&[layer]);
        hash.update(&logical_length.to_be_bytes());
        hash.update(&block.to_be_bytes());
        hash.update(&column.to_be_bytes());
        hash.update(&(block * u64::from(values_per_block)).to_be_bytes());
        hash.update(&values_per_block.to_be_bytes());
    }
    hash.finalize()
}

fn manual_leaf_v2(parameter: [u8; 32], layer: u8, values: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, layer]);
    hash.update(&u32::try_from(524_288_u64 >> layer).unwrap().to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(values);
    hash.finalize()
}

fn manual_node_v2(
    parameter: [u8; 32],
    layer: u8,
    height: u8,
    left: [u8; 32],
    right: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, layer, height]);
    hash.update(&left);
    hash.update(&right);
    hash.finalize()
}

#[test]
fn selected_layer_mappings_match_literal_independent_kats() {
    assert_eq!(
        parameter_digest_v2(SpoolGeometryV2::release_v2()).unwrap(),
        PARAMETER_KAT_V2
    );
    for (layer, expected) in MAPPING_KATS_V2 {
        let descriptor = fri_layer_layout_v2(PARAMETER_KAT_V2, layer).unwrap();
        assert_eq!(descriptor.mapping_digest, expected);
        assert_eq!(manual_mapping_v2(layer), expected);
    }
}

#[test]
fn selected_layer_leaf_and_node_frames_match_literal_independent_kats() {
    let values = [0x42; 6_080];
    for ((layer, leaf), (node_layer, height, node)) in LEAF_KATS_V2.into_iter().zip(NODE_KATS_V2) {
        assert_eq!(layer, node_layer);
        assert_eq!(
            continuation_leaf_hash_v2([0x11; 32], layer, &values).unwrap(),
            leaf
        );
        assert_eq!(manual_leaf_v2([0x11; 32], layer, &values), leaf);
        assert_eq!(
            continuation_node_hash_v2(
                [0x11; 32],
                layer,
                usize::from(height),
                [0x31; 32],
                [0x52; 32],
            )
            .unwrap(),
            node
        );
        assert_eq!(
            manual_node_v2([0x11; 32], layer, height, [0x31; 32], [0x52; 32]),
            node
        );
    }
}

#[test]
fn terminal_scatter_places_one_value_in_each_equal_leaf() {
    let mut terminal = ZeroizingFriTerminalV2::new_v2();
    terminal.scatter_v2(379, &[0x42; 32]).unwrap();
    let offset = 379 * 16;
    assert_eq!(&terminal.bytes_v2()[offset..offset + 16], &[0x42; 16]);
    assert_eq!(
        &terminal.bytes_v2()[6_080 + offset..6_080 + offset + 16],
        &[0x42; 16]
    );
    assert_eq!(&terminal.bytes_v2()[..6_080], &terminal.bytes_v2()[6_080..]);
}

#[test]
fn source_guards_pin_the_bounded_nonauthorizing_continuation() {
    let batch = include_str!("fri_layers2_17_v2.rs");
    let storage = include_str!("storage_v2/fold_layer1_v2/fold_layers2_17_v2.rs");
    let parent = include_str!("../batch_fri_v2.rs");
    assert!(batch.lines().count() <= 400);
    assert!(storage.lines().count() <= 650);
    assert!(parent.lines().count() <= 500);
    for required in [
        "accepted_fri_layers: [Option<FriLayerRootedV2>; FRI_CONTINUATION_LAYERS_V2]",
        "terminal_replay_complete: Option<FriLayerReplayCompleteV2>",
        "array::from_fn(|_| None)",
        "for source_layer in 1..17_usize",
        "fold_terminal_v2(ready, source)",
        "authenticated_layers: Infallible",
        "exact_folds: Infallible",
        "equal_terminal: Infallible",
        "FRI_CONTINUATION_TOTAL_IO_BYTES_V2: u64 = 7_977_032_960",
        "FRI_WITH_PRIOR_RETAINED_BYTES_V2: u64 = 13_561_628_480",
        "FRI_CONTINUATION_EXPLICIT_PEAK_BYTES_V2: usize = 12_599_296",
    ] {
        assert!(batch.contains(required), "missing batch pin: {required}");
    }
    for false_gate in [
        "AUTHENTICATED_FRI_REPLAY_COMPLETE_V2: bool = false",
        "FRI_ALL_FOLDS_COMPLETE_V2: bool = false",
        "FRI_TERMINAL_EQUALITY_BOUND_V2: bool = false",
        "FRI_QUERIES_DERIVED_V2: bool = false",
        "FRI_CONTINUATION_ZERO_KNOWLEDGE_BOUND_V2: bool = false",
        "FRI_CONTINUATION_PROOF_EMITTED_V2: bool = false",
        "FRI_CONTINUATION_RSS_ACCEPTED_V2: bool = false",
        "FRI_CONTINUATION_RECEIPT_ACCEPTED_V2: bool = false",
        "FRI_CONTINUATION_RELEASE_READY_V2: bool = false",
    ] {
        assert!(
            batch.contains(false_gate),
            "raised continuation gate: {false_gate}"
        );
    }
    assert!(storage.contains("owner.descriptor.blocks_per_column >= 2"));
    assert!(storage.contains("self.lower.bytes_v2()[bytes..2 * bytes]"));
    assert!(storage.contains("continuation_leaf_hash_v2"));
    assert!(storage.contains("continuation_node_hash_v2"));
    assert!(!batch.contains("B18"));
    assert!(!storage.contains("layer 18"));
}
