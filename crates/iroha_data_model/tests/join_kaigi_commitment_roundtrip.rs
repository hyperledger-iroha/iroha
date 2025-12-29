//! Verifies `JoinKaigi` Norito encoding supports canonical hash literals.
use std::str::FromStr;

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    domain::DomainId,
    isi::{Instruction as _, InstructionBox, kaigi::JoinKaigi},
    kaigi::{KaigiId, KaigiParticipantCommitment},
    name::Name,
};
use norito::{
    core::NoritoDeserialize,
    json::{self, JsonDeserialize},
};

fn parse_hash_literal(literal: &str) -> Hash {
    let raw = format!("\"{literal}\"");
    let mut parser = json::Parser::new(raw.as_str());
    Hash::json_deserialize(&mut parser).expect("parse hash literal")
}

#[test]
fn join_kaigi_accepts_canonical_commitment_literal() {
    let call = KaigiId::new(
        DomainId::from_str("wonderland").expect("domain"),
        Name::from_str("weekly-sync").expect("call name"),
    );
    let participant = AccountId::from_str(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
    )
    .expect("account id");
    let commitment_literal =
        "hash:1111111111111111111111111111111111111111111111111111111111111111#4667";
    let commitment = parse_hash_literal(commitment_literal);
    let join = JoinKaigi {
        call_id: call,
        participant,
        commitment: Some(KaigiParticipantCommitment {
            commitment,
            alias_tag: None,
        }),
        nullifier: None,
        roster_root: None,
        proof: None,
    };
    let instruction =
        iroha_data_model::isi::Instruction::into_instruction_box(Box::new(join.clone()));
    let bytes = norito::core::to_bytes(&instruction).expect("serialize");
    let archived = norito::core::from_bytes::<InstructionBox>(&bytes).expect("from bytes");
    let decoded = InstructionBox::try_deserialize(archived).expect("deserialize");
    assert_eq!(decoded.as_any().downcast_ref::<JoinKaigi>(), Some(&join));
}
