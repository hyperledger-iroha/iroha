//! Exact canonical frames and payloads at the IVM's typed codec boundaries.

use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json::Value};

pub(crate) fn groups(scope: &str, count: usize) -> Vec<Value> {
    let observations: Vec<Value> = norito::json::from_str(include_str!(
        "../tests/fixtures/frame_identity_observations.v1.json"
    ))
    .expect("captured original IVM frames");
    assert_eq!(observations.len(), 2);
    let mut scopes = observations
        .iter()
        .filter(|row| row["scope"].as_str() == Some(scope));
    let observation = scopes.next().expect("captured scope");
    assert!(scopes.next().is_none(), "scope is unique");
    assert_eq!(
        observation["schema"].as_str(),
        Some("iroha.ivm.original-frames.v1")
    );
    assert_eq!(
        observation["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    let groups = observation["groups"].as_array().unwrap();
    assert_eq!(groups.len(), count);
    groups.clone()
}

fn assert_frame<T>(row: &Value, shape: &str, value: T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert_eq!(row["shape"].as_str(), Some(shape));
    assert_eq!(
        row["nominal"].as_str(),
        Some(<T as NoritoSchema>::nominal_name().as_str())
    );
    let hash = hex::encode(norito::schema::identity::frame_hash::<T>());
    assert_eq!(row["serialize_schema_hash"].as_str(), Some(hash.as_str()));
    assert_eq!(row["deserialize_schema_hash"].as_str(), Some(hash.as_str()));
    let expected = hex::decode(row["frame_hex"].as_str().unwrap()).unwrap();
    let ambient = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    assert_eq!(
        norito::encode_canonical(&value).unwrap(),
        expected,
        "{shape}"
    );
    let decoded: T =
        ivm_abi::codec::decode_canonical_norito(&expected).expect("original frame decodes");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), expected);
    assert_eq!(norito::core::get_decode_flags(), ambient);
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut payload = Vec::new();
    norito::SerializePayload::serialize(&value, &mut norito::core::Encoder::new(&mut payload))
        .unwrap();
    assert_eq!(
        payload,
        hex::decode(row["payload_hex"].as_str().unwrap()).unwrap()
    );
    let mut wrong_identity = expected.clone();
    wrong_identity[6] ^= 1;
    assert!(ivm_abi::codec::decode_canonical_norito::<T>(&wrong_identity).is_err());
    assert!(ivm_abi::codec::decode_canonical_norito::<T>(&expected[..expected.len() - 1]).is_err());
    let mut trailing = expected;
    trailing.push(0);
    assert!(ivm_abi::codec::decode_canonical_norito::<T>(&trailing).is_err());
}

pub(crate) fn assert_group<T>(group: &Value, case: &str, value: T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + Clone,
{
    assert_eq!(group["case"].as_str(), Some(case));
    let frames = group["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 5);
    assert_frame(&frames[0], "root", value.clone());
    assert_frame(&frames[1], "option_none", None::<T>);
    assert_frame(&frames[2], "option_some", Some(value.clone()));
    assert_frame(&frames[3], "vec_empty", Vec::<T>::new());
    assert_frame(&frames[4], "vec_two", vec![value.clone(), value]);
}

#[test]
fn captured_original_ivm_public_frames() {
    use crate::execution_summary::{EXECUTION_SUMMARY_VERSION_V1, ExecutionSummary};
    use crate::signature::{Ed25519BatchEntry, Ed25519BatchRequest, verify_ed25519_batch};
    use ed25519_dalek::{Signer as _, SigningKey};
    let populated = ExecutionSummary {
        version: EXECUTION_SUMMARY_VERSION_V1,
        code_hash: [0x01; 32],
        final_register_root: [0x02; 32],
        final_memory_root: [0x03; 32],
        output_hash: [0x04; 32],
        pc_trace_hash: [0x05; 32],
        delta_trace_hash: [0x06; 32],
        register_trace_hash: [0x07; 32],
        constraint_hash: [0x08; 32],
        memory_log_hash: [0x09; 32],
        register_log_hash: [0x0a; 32],
        step_log_hash: [0x0b; 32],
        cycles: u64::MAX,
        max_cycles: u64::MAX - 1,
        gas_used: u64::MAX - 2,
        gas_remaining: u64::MAX - 3,
        pc_trace_len: u64::MAX - 4,
        delta_trace_len: u64::MAX - 5,
        register_trace_len: u64::MAX - 6,
        constraint_len: u64::MAX - 7,
        memory_log_len: u64::MAX - 8,
        register_log_len: u64::MAX - 9,
        step_log_len: u64::MAX - 10,
        zk_mode: true,
        halted: true,
        constraint_failed: true,
    };
    let key = SigningKey::from_bytes(&[0x31; 32]);
    let entry = |message: &[u8]| Ed25519BatchEntry {
        message: message.to_vec(),
        signature: key.sign(message).to_bytes().to_vec(),
        public_key: key.verifying_key().to_bytes().to_vec(),
    };
    let one = Ed25519BatchRequest {
        entries: vec![entry(b"ivm-codec-capture")],
    };
    let two = Ed25519BatchRequest {
        entries: vec![entry(b"ivm-codec-capture"), entry(b"")],
    };
    verify_ed25519_batch(&one, 2).unwrap();
    verify_ed25519_batch(&two, 2).unwrap();
    let groups = groups("public", 5);
    assert_group(&groups[0], "summary_default", ExecutionSummary::default());
    assert_group(&groups[1], "summary_populated", populated);
    assert_group(
        &groups[2],
        "ed25519_empty_request",
        Ed25519BatchRequest { entries: vec![] },
    );
    assert_group(&groups[3], "ed25519_one_valid", one);
    assert_group(&groups[4], "ed25519_two_valid", two);
}
