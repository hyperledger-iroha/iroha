//! JSON base64 helper test for `BallotProof` (feature `zk-ballot`).
#![cfg(feature = "zk-ballot")]

#[test]
fn ballot_proof_json_base64() {
    use iroha_data_model::isi::governance::BallotProof;
    let v = BallotProof {
        backend: "halo2/pasta/tiny-add".into(),
        envelope_bytes: vec![0u8, 1, 2, 3, 4],
        root_hint: None,
        owner: None,
        nullifier: None,
        amount: Some("900".to_string()),
        duration_blocks: Some(32),
        direction: Some("Abstain".to_string()),
    };
    // Serialize to JSON string (via norito::json for consistency)
    let s = norito::json::to_json(&v).expect("json");
    // Ensure envelope_bytes appears as a base64 string
    let parsed: norito::json::Value = norito::json::from_str(&s).unwrap();
    let b64 = parsed
        .get("envelope_bytes")
        .and_then(|x| x.as_str())
        .expect("b64");
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .unwrap();
    assert_eq!(raw, vec![0u8, 1, 2, 3, 4]);
    // Deserialize back
    let round: BallotProof = norito::json::from_str(&s).unwrap();
    assert_eq!(round.envelope_bytes, v.envelope_bytes);
    assert_eq!(round.amount.as_deref(), Some("900"));
    assert_eq!(round.duration_blocks, Some(32));
    assert_eq!(round.direction.as_deref(), Some("Abstain"));
}
