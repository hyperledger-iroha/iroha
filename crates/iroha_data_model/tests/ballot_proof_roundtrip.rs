//! Norito roundtrip for `BallotProof` (feature `zk-ballot`).
#![cfg(feature = "zk-ballot")]

#[test]
fn ballot_proof_roundtrip() {
    use iroha_data_model::isi::governance::BallotProof;
    let v = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1, 2, 3, 4],
        root_hint: Some([0xAA; 32]),
        owner: None,
        nullifier: Some([0x55; 32]),
        amount: Some("42".to_string()),
        duration_blocks: Some(64),
        direction: Some("Aye".to_string()),
    };
    let enc = norito::to_bytes(&v).expect("encode");
    let arch = norito::from_bytes::<BallotProof>(&enc).expect("archived");
    let dec: BallotProof = norito::core::NoritoDeserialize::deserialize(arch);
    assert_eq!(dec.backend, "halo2/ipa");
    assert_eq!(dec.envelope_bytes, vec![1, 2, 3, 4]);
    assert_eq!(dec.root_hint, Some([0xAA; 32]));
    assert_eq!(dec.nullifier, Some([0x55; 32]));
    assert_eq!(dec.amount.as_deref(), Some("42"));
    assert_eq!(dec.duration_blocks, Some(64));
    assert_eq!(dec.direction.as_deref(), Some("Aye"));
}
