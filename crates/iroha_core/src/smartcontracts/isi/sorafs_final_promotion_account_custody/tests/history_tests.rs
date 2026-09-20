//! Distinct namespace, committed enrollment and immutable account-history rejection controls.
use super::*;
use history::{control_digest, control_head_key, control_height_key, control_record_key, key_path};
use sorafs_manifest::signer::{
    custody::SignerCustodyRecordV1,
    protocol::{SignerPurposeBindingV1, SignerRoleV1},
};

#[test]
fn account_configuration_requires_exact_native_manager_even_at_genesis_and_cas_on_retry() {
    let mut f = fixture();
    let policy = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        let first = instruction(tx, Action::Configure(policy));
        let before = retained(tx);
        for authority in [&f.observer, &f.target, &f.other] {
            assert!(first.clone().execute(authority, tx).is_err());
            assert_eq!(retained(tx), before);
        }
        first.clone().execute(&f.manager, tx).unwrap();
        let after = retained(tx);
        assert!(!after.is_empty());
        assert!(first.execute(&f.manager, tx).is_err());
        assert_eq!(retained(tx), after);
    });
    let current = snapshot(&f);
    assert_eq!(current.control_record.execution.authority, f.manager);
    assert_eq!(current.control_record.execution.height, 1);
    assert_eq!(current.control_record.execution.ordinal, 0);
    assert!(current.control_record.enrollment.is_none());
    assert_ne!(
        history::scope::<AccountPurpose>(DEPLOYMENT),
        history::scope::<history::ReceiptPurpose>(DEPLOYMENT)
    );
}

#[test]
fn account_configuration_rejects_foreign_role_purpose_network_and_malformed_handles() {
    let mut f = fixture();
    transact(&mut f.state, 1_000, |tx| {
        let before = retained(tx);
        for field in 0..7 {
            let mut policy = f.policy.clone();
            match field {
                0 => policy.binding.chain_id.push('x'),
                1 => policy.binding.network_id[0] ^= 1,
                2 => policy.binding.role = SignerRoleV1::FinalPromotionProvenance,
                3 => {
                    policy.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
                        deployment_id: DEPLOYMENT.into(),
                    }
                }
                4 => {
                    policy.binding.purpose =
                        SignerPurposeBindingV1::FinalPromotionAccountTransaction {
                            deployment_id: "promotion-secondary".into(),
                        }
                }
                5 => policy.binding.runtime_handle = "unknown://account/key".into(),
                6 => policy.binding.key_revision = 0,
                _ => unreachable!(),
            }
            let mutation = instruction(tx, Action::Configure(encode(&policy).unwrap()));
            assert!(mutation.execute(&f.manager, tx).is_err(), "field {field}");
            assert_eq!(retained(tx), before);
        }
    });
}

#[test]
fn account_configuration_and_enrollment_accept_operator_selected_signer_handles() {
    for scheme in ["software", "signer", "hsm", "kms", "pkcs11"] {
        let mut f = fixture();
        f.policy.binding.runtime_handle = format!("{scheme}://promotion/primary");
        f.policy.binding.key_handle = format!("{scheme}://promotion/key-1");
        let configured = encode(&f.policy).unwrap();
        transact(&mut f.state, 1_000, |tx| {
            let mutation = instruction(tx, Action::Configure(configured));
            let before = retained(tx);
            assert!(mutation.clone().execute(&f.target, tx).is_err());
            assert_eq!(retained(tx), before);
            mutation.execute(&f.manager, tx).unwrap();
        });
        assert_eq!(snapshot(&f).control.policy, f.policy);
        enroll(&mut f);
        let enrolled = snapshot(&f);
        assert!(enrolled.control.active_head.is_some());
        assert_eq!(enrolled.control.policy, f.policy);
        assert_ne!(f.target, f.observer);
        assert_ne!(f.policy.binding.public_key, f.policy.attester_public_key);
        let check = check_instruction(&f);
        transact(&mut f.state, 2_000, |tx| {
            assert_no_writes(tx, &check, &f.target, false);
            assert_no_writes(tx, &check, &f.observer, true);
        });
    }
}

#[test]
fn account_enrollment_and_check_require_committed_control_predecessors() {
    let mut f = fixture();
    let policy = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        instruction(tx, Action::Configure(policy))
            .execute(&f.manager, tx)
            .unwrap();
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Enroll(vec![1]))
                .execute(&f.manager, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
    let attestation = attest(&f, 1_500, 100_000);
    let base = check_instruction(&f);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, Action::Enroll(attestation.clone()))
            .execute(&f.manager, tx)
            .unwrap();
        let check = instruction(tx, base.action.clone());
        assert_no_writes(tx, &check, &f.observer, false);
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Enroll(attestation))
                .execute(&f.manager, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
    });
    let check = check_instruction(&f);
    transact(&mut f.state, 1_700, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
    });
    assert!(applied(&f, &check, 1_700).is_ok());
}

#[test]
fn account_history_rejects_missing_or_substituted_head_record_height_and_first_use_indexes() {
    let mut f = enrolled();
    let check = check_instruction(&f);
    let current = snapshot(&f);
    let keys = [
        control_head_key::<AccountPurpose>(DEPLOYMENT).unwrap(),
        control_record_key::<AccountPurpose>(DEPLOYMENT, current.control_record.revision).unwrap(),
        control_height_key::<AccountPurpose>(
            DEPLOYMENT,
            current.control_record.execution.height,
            current.control_record.execution.ordinal,
        )
        .unwrap(),
        key_path::<AccountPurpose>(DEPLOYMENT, true, &f.policy.binding.public_key).unwrap(),
        key_path::<AccountPurpose>(DEPLOYMENT, false, &f.policy.attester_public_key).unwrap(),
    ];
    transact(&mut f.state, 2_000, |tx| {
        assert_no_writes(tx, &check, &f.observer, true);
        for path in keys {
            let original = tx.world.smart_contract_state.get(&path).unwrap().clone();
            tx.world.smart_contract_state.remove(path.clone());
            assert_no_writes(tx, &check, &f.observer, false);
            tx.world.smart_contract_state.insert(path.clone(), vec![0]);
            assert_no_writes(tx, &check, &f.observer, false);
            tx.world.smart_contract_state.insert(path, original);
            assert_no_writes(tx, &check, &f.observer, true);
        }
    });
    assert!(applied(&f, &check, 2_000).is_ok());
}

#[test]
fn account_rotation_preserves_first_use_indexes_and_rejects_retired_signer_or_attester_reuse() {
    for signer in [false, true] {
        let mut f = enrolled();
        let original = f.policy.clone();
        let mut next = original.clone();
        if signer {
            next.binding.public_key = key(99).public_key().clone();
            next.binding.key_revision += 1;
        } else {
            next.attester_public_key = key(98).public_key().clone();
            next.attester_authority.key_revision += 1;
        }
        transact(&mut f.state, 2_000, |tx| {
            instruction(tx, Action::Configure(encode(&next).unwrap()))
                .execute(&f.manager, tx)
                .unwrap();
            let current = history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT)
                .unwrap()
                .unwrap();
            assert!(current.record.enrollment.is_none());
            assert!(current.state.active_head.is_none());
            let before = retained(tx);
            let mut reused = next.clone();
            if signer {
                reused.binding.public_key = original.binding.public_key.clone();
                reused.binding.key_revision += 1;
            } else {
                reused.attester_public_key = original.attester_public_key.clone();
                reused.attester_authority.key_revision += 1;
            }
            assert_eq!(
                apply(
                    instruction(tx, Action::Configure(encode(&reused).unwrap())),
                    &f.manager,
                    tx
                ),
                Err(HistoryError::Generation)
            );
            assert_eq!(retained(tx), before);
            assert!(
                tx.world
                    .smart_contract_state
                    .get(
                        &key_path::<AccountPurpose>(
                            DEPLOYMENT,
                            signer,
                            if signer {
                                &original.binding.public_key
                            } else {
                                &original.attester_public_key
                            }
                        )
                        .unwrap()
                    )
                    .is_some()
            );
        });
        assert!(
            read_final_promotion_account_custody_at_v1(&f.state.view(), &original.binding, 2)
                .unwrap()
                .is_some()
        );
        if signer {
            assert!(
                read_final_promotion_account_custody_at_v1(&f.state.view(), &original.binding, 3)
                    .is_err()
            );
        }
        assert!(
            read_final_promotion_account_custody_at_v1(&f.state.view(), &next.binding, 3)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn account_enrollment_signature_and_native_record_domain_reject_substitution() {
    let mut f = fixture();
    configure(&mut f);
    let bytes = attest(&f, 1_500, 100_000);
    let mut forged: SignerCustodyRecordV1 = history::decode(&bytes).unwrap();
    forged.attestation[0] ^= 1;
    transact(&mut f.state, 1_500, |tx| {
        let before = retained(tx);
        assert!(
            instruction(tx, Action::Enroll(encode(&forged).unwrap()))
                .execute(&f.manager, tx)
                .is_err()
        );
        assert_eq!(retained(tx), before);
        instruction(tx, Action::Enroll(bytes.clone()))
            .execute(&f.manager, tx)
            .unwrap();
        let row = history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        assert_eq!(row.record.enrollment.as_deref(), Some(bytes.as_slice()));
        assert_eq!(
            row.index.digest,
            control_digest::<AccountPurpose>(&row.record).unwrap()
        );
        let wrong = history::digest(iroha_data_model::sorafs::final_promotion_authority::FINAL_PROMOTION_CUSTODY_RECORD_DOMAIN_V1, &row.record).unwrap();
        assert_ne!(wrong, row.index.digest);
    });
}

#[test]
fn account_request_digest_binds_complete_mutation_cas_and_original_authority() {
    let mut f = fixture();
    let bytes = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        let base = instruction(tx, Action::Configure(bytes));
        let digest = request_digest(&base, &f.manager).unwrap();
        assert_ne!(digest, request_digest(&base, &f.observer).unwrap());
        for field in 0..4 {
            let mut changed = base.clone();
            match field {
                0 => changed.deployment_id.push('x'),
                1 => changed.expected_control_revision += 1,
                2 => changed.expected_control_digest[0] ^= 1,
                3 => {
                    changed.action = Action::Revoke(FinalPromotionAccountCustodyRevocationV1 {
                        signer: true,
                        attester: false,
                    })
                }
                _ => unreachable!(),
            }
            assert_ne!(digest, request_digest(&changed, &f.manager).unwrap());
        }
        base.execute(&f.manager, tx).unwrap();
        assert_eq!(
            history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT)
                .unwrap()
                .unwrap()
                .record
                .request_digest,
            digest
        );
        let mut oversized = instruction(
            tx,
            Action::Configure(vec![0; FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_RECORD_BYTES_V1]),
        );
        assert_eq!(
            request_digest(&oversized, &f.manager),
            Err(HistoryError::Invalid)
        );
        let before = retained(tx);
        assert!(oversized.clone().execute(&f.manager, tx).is_err());
        assert_eq!(retained(tx), before);
        oversized.action = Action::Revoke(FinalPromotionAccountCustodyRevocationV1 {
            signer: false,
            attester: false,
        });
        assert!(oversized.execute(&f.manager, tx).is_err());
        assert_eq!(retained(tx), before);
    });
}
