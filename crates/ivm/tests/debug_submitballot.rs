use iroha_data_model::isi as dm;

#[test]
fn decode_submit_ballot_roundtrip() {
    // Build a SubmitBallot, encode as InstructionBox, then decode and confirm type
    let sb = dm::zk::SubmitBallot {
        election_id: "e1".to_string(),
        ciphertext: vec![1, 2, 3],
        ballot_proof: iroha_data_model::proof::ProofAttachment::new_inline(
            "halo2/ipa".into(),
            iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![0x01]),
            iroha_data_model::proof::VerifyingKeyBox::new("halo2/ipa".into(), vec![0x02]),
        ),
        nullifier: [7u8; 32],
    };
    let ib = dm::InstructionBox::from(sb.clone());
    let bytes = norito::to_bytes(&ib).expect("encode InstructionBox");
    let decoded: dm::InstructionBox =
        norito::decode_from_bytes(&bytes).expect("decode InstructionBox");

    // Downcast to SubmitBallot via Instruction::as_any peeler
    let instr: &dyn dm::Instruction = &decoded;
    let recovered = instr
        .as_any()
        .downcast_ref::<dm::zk::SubmitBallot>()
        .expect("SubmitBallot downcast");
    assert_eq!(recovered, &sb);
}
