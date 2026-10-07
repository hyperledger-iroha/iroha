//! Fixture-pinned preimages, exact continuation states and source tampering.

use super::*;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn bytes(j: &norito::json::Value, k: &str) -> Vec<u8> {
    hex(j.get(k).unwrap().as_str().unwrap())
}
fn fixture() -> Vec<ContextHashLeafCircuit> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let j: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let epoch = j
        .get("authenticated_schedule")
        .unwrap()
        .get("current")
        .unwrap();
    let context = epoch.get("context").unwrap();
    let payload = bytes(context, "payload_hex");
    let encoded = bytes(context, "frame_hex");
    assert_eq!(&encoded[6..22], &EPOCH_CODEC_ID);
    assert_eq!(
        u64::from_le_bytes(encoded[23..31].try_into().unwrap()),
        payload.len() as u64
    );
    assert_eq!(
        u64::from_le_bytes(encoded[31..39].try_into().unwrap()),
        norito::core::hardware_crc64(&payload)
    );
    assert_eq!(&encoded[40..], &payload);
    let frame = bytes(&j, "result_preimage_hex");
    let positions: Vec<_> = frame
        .windows(payload.len())
        .enumerate()
        .filter(|(_, v)| *v == payload.as_slice())
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "native payload appears at one exact R field"
    );
    let id: [u8; 32] = bytes(epoch, "context_id_hex").try_into().unwrap();
    let preimage = [EPOCH_TAG, encoded.as_slice()].concat();
    assert_eq!(<[u8; 32]>::from(iroha_crypto::Hash::new(preimage)), id);
    prepare_context_hash(
        frame,
        u32::try_from(positions[0]).unwrap(),
        u32::try_from(payload.len()).unwrap(),
        id,
    )
    .unwrap()
}
fn check(c: &ContextHashLeafCircuit) -> bool {
    check_circuit(c, 16, &c.instances().unwrap(), CheckMode::Strict)
        .unwrap()
        .is_satisfied()
}
#[test]
fn complete_native_epoch_witness_scan_has_exact_phase_and_interval_boundaries() {
    let leaves = fixture();
    let input = &leaves[0].input;
    assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
    assert_eq!(
        leaves[0].endpoints()[4],
        boundary_digest_native(input, false)
    );
    assert_eq!(
        leaves.last().unwrap().endpoints()[5],
        boundary_digest_native(input, true)
    );
    for pair in leaves.windows(2) {
        assert!(
            std::sync::Arc::ptr_eq(&pair[0].tape, &pair[1].tape),
            "all 2,561 leaves share one immutable native tape tree"
        );
        assert_eq!(pair[0].endpoints()[1], pair[1].endpoints()[1]);
        assert_eq!(pair[0].endpoints()[3], pair[1].endpoints()[2]);
        assert_eq!(pair[0].endpoints()[5], pair[1].endpoints()[4]);
    }
    assert_eq!(leaves[input.crc_steps() as usize - 1].after.crc, 0);
    assert_eq!(leaves[CRC_LEAVES as usize].before.blake, [0; 8]);
    assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
    assert!(
        leaves[input.crc_steps() as usize..CRC_LEAVES as usize]
            .iter()
            .all(|leaf| leaf.before == ContextHashState::default()
                && leaf.after == ContextHashState::default())
    );
}
#[test]
fn native_epoch_crc_and_blake_boundary_and_continuation_leaves_are_constrained() {
    let leaves = fixture();
    let crc = leaves[0].input.crc_steps() as usize;
    let n = CRC_LEAVES as usize;
    let blake = leaves[0].input.blake_steps() as usize;
    for i in [
        0,
        1,
        crc - 1,
        crc,
        n - 1,
        n,
        n + 1,
        n + blake - 1,
        n + blake,
        leaves.len() - 1,
    ] {
        assert!(check(&leaves[i]), "native source leaf {i}");
    }
}
#[test]
fn native_epoch_source_rejects_checksum_byte_state_counter_and_id_substitution() {
    let leaves = fixture();
    let n = CRC_LEAVES as usize;
    let data_crc = leaves[0].input.crc_steps() as usize;
    let data_blake = leaves[0].input.blake_steps() as usize;
    let mut crc = leaves[data_crc - 1].clone();
    crc.input.checksum ^= 1;
    assert!(!check(&crc));
    let mut bytes = leaves[1].clone();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let j: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut original = hex(j["result_preimage_hex"].as_str().unwrap());
    original[bytes.input.payload_start as usize + 32] ^= 1;
    bytes.tape =
        std::sync::Arc::new(ResultTapeWitness::from_frame(&Value::known(original)).unwrap());
    assert!(!check(&bytes));
    let mut state = leaves[1].clone();
    state.before.crc ^= 1;
    assert!(!check(&state));
    let mut cursor = leaves[1].clone();
    cursor.cursor += 1;
    assert!(!check(&cursor));
    let mut initial = leaves[0].clone();
    initial.before.crc = 1;
    assert!(!check(&initial));
    let mut handoff = leaves[data_crc - 1].clone();
    handoff.after.crc = 1;
    assert!(!check(&handoff));
    let mut id = leaves[n + data_blake - 1].clone();
    id.input.context_id[0] ^= 1;
    assert!(!check(&id));
    let mut compression = leaves[n + 1].clone();
    compression.before.blake[7] ^= 1;
    assert!(!check(&compression));
}
#[test]
fn native_epoch_source_layout_is_fixed_across_phase_positions_and_unknown_inputs() {
    let leaves = fixture();
    let n = CRC_LEAVES as usize;
    for indices in [[0, 1, n - 1], [n, n + 1, leaves.len() - 1]] {
        let first = &leaves[indices[0]];
        let known = synthesize(first, 16, Some(&first.instances().unwrap())).unwrap();
        let unknown = synthesize(&first.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
        let metadata =
            synthesize(&ContextHashLeafCircuit::for_source(first.phase), 16, None).unwrap();
        assert_eq!(known.tables.fixed(), metadata.tables.fixed());
        assert_eq!(known.tables.permutation(), metadata.tables.permutation());
        let rows = known
            .tables
            .advice_assigned()
            .iter()
            .map(|col| col.iter().rposition(|b| *b).map_or(0, |i| i + 1))
            .max()
            .unwrap_or(0);
        let cells: usize = known
            .tables
            .advice_assigned()
            .iter()
            .map(|col| col.iter().filter(|b| **b).count())
            .sum();
        eprintln!(
            "native context {:?}: {rows} advice rows, {cells} assigned cells",
            first.phase
        );
        for index in indices.into_iter().skip(1) {
            let other = &leaves[index];
            let assigned = synthesize(other, 16, Some(&other.instances().unwrap())).unwrap();
            assert_eq!(known.tables.fixed(), assigned.tables.fixed());
            assert_eq!(known.tables.selectors(), assigned.tables.selectors());
            assert_eq!(known.tables.permutation(), assigned.tables.permutation());
        }
    }
}

#[test]
fn fixed_catalog_covers_short_and_maximum_payloads_with_bound_noop_padding() {
    for length in [1_usize, 32, 33, 128, 65_536] {
        let payload = vec![0x37; length];
        let checksum = norito::core::hardware_crc64(&payload);
        let mut preimage = EPOCH_TAG.to_vec();
        preimage.extend_from_slice(b"NRT0\0\0");
        preimage.extend_from_slice(&EPOCH_CODEC_ID);
        preimage.push(0);
        preimage.extend_from_slice(&u64::try_from(length).unwrap().to_le_bytes());
        preimage.extend_from_slice(&checksum.to_le_bytes());
        preimage.push(2);
        preimage.extend_from_slice(&payload);
        let id = iroha_crypto::Hash::new(&preimage).into();
        let leaves = prepare_context_hash(payload, 0, u32::try_from(length).unwrap(), id).unwrap();
        assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
        assert_eq!(leaves[CRC_LEAVES as usize].phase, ContextHashPhase::Blake);
        assert_eq!(
            leaves.last().unwrap().endpoints()[3],
            Fp::from(u64::from(PROGRAM_LENGTH))
        );
        if length == 1 || length == 65_536 {
            for index in [
                leaves[0].input.crc_steps() as usize - 1,
                CRC_LEAVES as usize + leaves[0].input.blake_steps() as usize - 1,
            ] {
                assert!(
                    check(&leaves[index]),
                    "length {length}, source leaf {index}"
                );
            }
        }
    }
    let leaves = fixture();
    let mut padded_crc = leaves[CRC_LEAVES as usize - 1].clone();
    padded_crc.before.crc = 1;
    assert!(!check(&padded_crc));
    let mut padded_blake = leaves.last().unwrap().clone();
    padded_blake.before.blake[0] = 1;
    assert!(!check(&padded_blake));
    let mut phase = leaves[CRC_LEAVES as usize].clone();
    phase.phase = ContextHashPhase::Crc;
    assert!(!check(&phase));
}
