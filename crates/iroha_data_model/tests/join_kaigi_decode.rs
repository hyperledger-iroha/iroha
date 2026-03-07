//! Verify `JoinKaigi` instructions roundtrip through Norito encoding.

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::{InstructionBox, kaigi::JoinKaigi},
    kaigi::{KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier},
    prelude::{AccountId, DomainId, Name},
};

#[test]
fn join_kaigi_roundtrip_preserves_optional_fields() {
    let domain_id = "wonderland".parse::<DomainId>().expect("domain id");
    let call_id = KaigiId::new(
        domain_id.clone(),
        "kaigi".parse::<Name>().expect("call name"),
    );
    let participant =
        AccountId::parse_encoded("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw")
            .expect("participant account id")
            .into_account_id();
    let commitment = KaigiParticipantCommitment {
        commitment: Hash::new([0xAA; Hash::LENGTH]),
        alias_tag: Some("alice".into()),
    };
    let nullifier = KaigiParticipantNullifier {
        digest: Hash::new([0xBB; Hash::LENGTH]),
        issued_at_ms: 1_704_000_000_000,
    };

    let join = JoinKaigi {
        call_id: call_id.clone(),
        participant: participant.clone(),
        commitment: Some(commitment.clone()),
        nullifier: Some(nullifier.clone()),
        roster_root: Some(Hash::new([0xCC; Hash::LENGTH])),
        proof: Some(vec![0x10, 0x20, 0x30]),
    };

    let boxed = iroha_data_model::isi::Instruction::into_instruction_box(Box::new(join.clone()));
    let encoded = norito::to_bytes(&boxed).expect("JoinKaigi should be serializable with norito");
    let decoded: InstructionBox =
        norito::decode_from_bytes(&encoded).expect("JoinKaigi should decode via norito");

    assert_eq!(
        decoded, boxed,
        "roundtrip should preserve instruction bytes"
    );

    let decoded_join = iroha_data_model::isi::Instruction::as_any(&decoded)
        .downcast_ref::<JoinKaigi>()
        .expect("decoded JoinKaigi instruction");
    assert_eq!(decoded_join.call_id, call_id);
    assert_eq!(decoded_join.participant, participant);
    assert_eq!(decoded_join.commitment.as_ref(), Some(&commitment));
    assert_eq!(decoded_join.nullifier.as_ref(), Some(&nullifier));
    assert_eq!(
        decoded_join.roster_root,
        Some(Hash::new([0xCC; Hash::LENGTH]))
    );
    assert_eq!(decoded_join.proof.as_deref(), Some(&[0x10, 0x20, 0x30][..]));
}
