    #[test]
    fn multisig_proposal_batch_entries_reject_underpayment_and_overpayment() {
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let multisig = account(5);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        for (observed, expected_error) in [
            (
                19,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 19,
                },
            ),
            (
                21,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 21,
                },
            ),
        ] {
            let proposal = MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        TransferAssetBatch::new(vec![
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                recipient_a.clone(),
                                fee_asset.clone(),
                                1_u64,
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                recipient_b.clone(),
                                fee_asset.clone(),
                                1_u64,
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                treasury.clone(),
                                fee_asset.clone(),
                                minor_units(observed),
                            ),
                        ])
                        .into(),
                    ],
                    0,
                    Some(2),
                ),
                None,
            );
            let tx = tx(1, vec![proposal.into()], metadata_for(&policy));

            assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
        }
    }
