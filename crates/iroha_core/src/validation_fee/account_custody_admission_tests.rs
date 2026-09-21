// Actual DS fee admission retains all four independent account-custody actions.
#[test]
fn final_promotion_account_custody_actions_remain_available_under_validation_fee_policy() {
    use iroha_data_model::{
        isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
        sorafs::final_promotion_account_custody::{
            FinalPromotionAccountCustodyActionV1 as Action, FinalPromotionAccountCustodyCheckV1,
            FinalPromotionAccountCustodyRevocationV1,
        },
    };
    let policy = policy(&account(3));
    for action in [
        Action::Configure(Vec::new()),
        Action::Enroll(Vec::new()),
        Action::Revoke(FinalPromotionAccountCustodyRevocationV1 {
            signer: true,
            attester: true,
        }),
        Action::Check(FinalPromotionAccountCustodyCheckV1 {
            challenge: [10; 32],
            network_id: [11; 32],
            minimum_height: 1,
            minimum_block_hash: [12; 32],
            expected_account: account(4),
            transaction_payload_digest: [14; 32],
        }),
    ] {
        let instruction: InstructionBox = MutateSorafsFinalPromotionAccountCustody {
            deployment_id: "promotion-primary".into(),
            expected_control_revision: 1,
            expected_control_digest: [4; 32],
            action,
        }
        .into();
        assert_eq!(
            crate::smartcontracts::isi::registered_native_instruction_type_name(&instruction),
            Some(core::any::type_name::<
                MutateSorafsFinalPromotionAccountCustody,
            >())
        );
        assert_eq!(
            native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect
        );
        assert_eq!(
            enforce_policy(
                &tx(1, vec![instruction.clone()], Metadata::default()),
                &policy
            ),
            Ok(())
        );
        for instructions in [
            vec![
                instruction.clone(),
                transfer(
                    &account(1),
                    &policy_fee_asset(&policy),
                    Quantity::from(1_u64),
                    &account(2),
                ),
            ],
            vec![
                transfer(
                    &account(1),
                    &policy_fee_asset(&policy),
                    Quantity::from(1_u64),
                    &account(2),
                ),
                instruction,
            ],
        ] {
            assert_eq!(
                enforce_policy(&tx(1, instructions, metadata_for(&policy)), &policy),
                Err(ValidationFeeAdmissionError::MissingFee {
                    required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS
                })
            );
        }
    }
}
