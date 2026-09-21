//! Canonical registration and framing of native final-promotion authority mutations.
use super::*;
use crate::{
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{
        FinalPromotionAuthorityActionV1, FinalPromotionCheckSubjectV1, FinalPromotionCheckV1,
        FinalPromotionRevocationV1,
    },
};
use sorafs_manifest::signer::{
    final_promotion::SignerFinalPromotionRequestV1,
    protocol::{SignerOperationAuditHeadV1, SignerOperationCustodyV1},
};

#[test]
fn final_promotion_authority_box_uses_one_canonical_wire_identity_and_roundtrips() {
    assert_canonical_registered_instruction(MutateSorafsFinalPromotionAuthority {
        deployment_id: "sora-main".to_owned(),
        expected_control_revision: 4,
        expected_control_digest: [5; 32],
        action: FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        }),
    });
}

#[test]
fn final_promotion_check_box_reuses_the_canonical_instruction_wire_identity() {
    assert_canonical_registered_instruction(MutateSorafsFinalPromotionAuthority {
        deployment_id: "sora-main".to_owned(),
        expected_control_revision: 4,
        expected_control_digest: [5; 32],
        action: FinalPromotionAuthorityActionV1::Check(FinalPromotionCheckV1 {
            challenge: [6; 32],
            network_id: [7; 32],
            expected_operator: crate::account::AccountId::new(
                iroha_crypto::KeyPair::try_from_seed(
                    vec![19; 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
                .expect("independent operator fixture")
                .public_key()
                .clone(),
            ),
            minimum_height: 2,
            minimum_block_hash: [8; 32],
            request: SignerFinalPromotionRequestV1 {
                operation_id: [9; 32],
                binding_digest: [10; 32],
                original_custody: SignerOperationCustodyV1 {
                    record_digest: [11; 32],
                    control_state_digest: [5; 32],
                },
                statement_digest: [12; 32],
                statement_size: 3_380,
            },
            subject: FinalPromotionCheckSubjectV1::Current(SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            }),
        }),
    });
}

fn assert_canonical_registered_instruction(instruction: MutateSorafsFinalPromotionAuthority) {
    let boxed: InstructionBox = instruction.clone().into();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsFinalPromotionAuthority";
    let (wire_id, framed) =
        crate::isi::encoded_instruction_pair_payload(&boxed).expect("registered custody mutation");
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsFinalPromotionAuthority>()),
        Some(expected)
    );
    assert_eq!(
        wire_ids::ALL
            .iter()
            .filter(|entry| entry.wire_id == expected)
            .count(),
        1
    );
    let decoded = registry
        .decode(wire_id, &framed)
        .expect("known wire id")
        .expect("canonical mutation frame");
    let decoded = decoded
        .as_any()
        .downcast_ref::<MutateSorafsFinalPromotionAuthority>()
        .expect("decoded custody instruction");
    assert_eq!(decoded, &instruction);
    let reencoded =
        crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(decoded.clone()))
            .expect("reencoded frame");
    assert_eq!(reencoded, (wire_id, framed.clone()));
    assert!(
        registry
            .decode(
                std::any::type_name::<MutateSorafsFinalPromotionAuthority>(),
                &framed
            )
            .is_none(),
        "Rust type paths are not alternate wire IDs"
    );
    let foreign = crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(Log::new(
        Level::INFO,
        "foreign root".to_owned(),
    )))
    .expect("registered log frame");
    assert!(
        registry
            .decode(expected, &foreign.1)
            .expect("known custody id")
            .is_err(),
        "a different valid instruction frame cannot substitute for custody"
    );
    assert!(
        registry
            .decode(expected, &framed[..framed.len() - 1])
            .expect("known custody id")
            .is_err()
    );
}
