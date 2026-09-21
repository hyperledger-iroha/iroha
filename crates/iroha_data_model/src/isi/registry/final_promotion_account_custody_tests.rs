//! Distinct account-custody instruction identity and strict typed registry framing.
use super::*;
use crate::{
    isi::sorafs::{MutateSorafsFinalPromotionAccountCustody, MutateSorafsFinalPromotionAuthority},
    sorafs::{
        final_promotion_account_custody::{
            FinalPromotionAccountCustodyActionV1, FinalPromotionAccountCustodyCheckV1,
            FinalPromotionAccountCustodyRevocationV1,
        },
        final_promotion_authority::{FinalPromotionAuthorityActionV1, FinalPromotionRevocationV1},
    },
};

fn instruction(
    action: FinalPromotionAccountCustodyActionV1,
) -> MutateSorafsFinalPromotionAccountCustody {
    MutateSorafsFinalPromotionAccountCustody {
        deployment_id: "production-primary".into(),
        expected_control_revision: 2,
        expected_control_digest: [1; 32],
        action,
    }
}

#[test]
fn account_custody_instruction_registers_exactly_one_distinct_wire_id_for_all_actions() {
    let key = iroha_crypto::KeyPair::try_from_seed(vec![2; 32], iroha_crypto::Algorithm::Ed25519)
        .unwrap();
    let actions = [
        FinalPromotionAccountCustodyActionV1::Configure(vec![1, 2]),
        FinalPromotionAccountCustodyActionV1::Enroll(vec![3, 4]),
        FinalPromotionAccountCustodyActionV1::Revoke(FinalPromotionAccountCustodyRevocationV1 {
            signer: true,
            attester: false,
        }),
        FinalPromotionAccountCustodyActionV1::Check(FinalPromotionAccountCustodyCheckV1 {
            challenge: [3; 32],
            network_id: [4; 32],
            minimum_height: 1,
            minimum_block_hash: [5; 32],
            expected_account: AccountId::new(key.public_key().clone()),
            transaction_payload_digest: [6; 32],
        }),
    ];
    let registry = default();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsFinalPromotionAccountCustody";
    assert_eq!(
        wire_ids::ALL
            .iter()
            .filter(|entry| entry.wire_id == expected)
            .count(),
        1
    );
    assert_eq!(
        registry.wire_id(std::any::type_name::<
            MutateSorafsFinalPromotionAccountCustody,
        >()),
        Some(expected)
    );
    for action in actions {
        let original = instruction(action);
        let pair =
            crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(original.clone()))
                .unwrap();
        assert_eq!(pair.0, expected);
        let decoded = registry.decode(expected, &pair.1).unwrap().unwrap();
        let decoded = decoded
            .as_any()
            .downcast_ref::<MutateSorafsFinalPromotionAccountCustody>()
            .unwrap();
        assert_eq!(decoded, &original);
        assert_eq!(
            crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(decoded.clone()))
                .unwrap(),
            pair
        );
        assert!(
            registry
                .decode(
                    std::any::type_name::<MutateSorafsFinalPromotionAccountCustody>(),
                    &pair.1
                )
                .is_none()
        );
        assert!(
            registry
                .decode(expected, &pair.1[..pair.1.len() - 1])
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn account_custody_registry_rejects_receipt_custody_frame_substitution_in_both_directions() {
    let account = instruction(FinalPromotionAccountCustodyActionV1::Revoke(
        FinalPromotionAccountCustodyRevocationV1 {
            signer: true,
            attester: false,
        },
    ));
    let receipt = MutateSorafsFinalPromotionAuthority {
        deployment_id: account.deployment_id.clone(),
        expected_control_revision: account.expected_control_revision,
        expected_control_digest: account.expected_control_digest,
        action: FinalPromotionAuthorityActionV1::Revoke(FinalPromotionRevocationV1 {
            signer: true,
            attester: false,
        }),
    };
    let account =
        crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(account)).unwrap();
    let receipt =
        crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(receipt)).unwrap();
    assert_ne!(account.0, receipt.0);
    assert_ne!(account.1, receipt.1);
    let registry = default();
    assert!(registry.decode(account.0, &receipt.1).unwrap().is_err());
    assert!(registry.decode(receipt.0, &account.1).unwrap().is_err());
}

#[test]
fn account_custody_instruction_json_rejects_missing_extra_and_duplicate_outer_cas() {
    let instruction = instruction(FinalPromotionAccountCustodyActionV1::Revoke(
        FinalPromotionAccountCustodyRevocationV1 {
            signer: true,
            attester: false,
        },
    ));
    let json = norito::json::to_json(&instruction).unwrap();
    let changed = json.replacen('{', "{\"expected_revision\":2,", 1);
    assert!(norito::json::from_str::<MutateSorafsFinalPromotionAccountCustody>(&changed).is_err());
    let changed = json.replacen('{', "{\"expected_control_revision\":2,", 1);
    assert!(norito::json::from_str::<MutateSorafsFinalPromotionAccountCustody>(&changed).is_err());
    let value = norito::json::to_value(&instruction).unwrap();
    let norito::json::Value::Object(fields) = value else {
        panic!("instruction object")
    };
    for field in fields.keys() {
        let mut incomplete = fields.clone();
        incomplete.remove(field);
        let json = norito::json::to_json(&norito::json::Value::Object(incomplete)).unwrap();
        assert!(
            norito::json::from_str::<MutateSorafsFinalPromotionAccountCustody>(&json).is_err(),
            "{field}"
        );
    }
}
