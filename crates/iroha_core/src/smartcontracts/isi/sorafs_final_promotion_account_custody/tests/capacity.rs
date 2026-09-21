//! Pure shared preparation boundaries; omitted full histories are never installed as authority.
use super::*;
use iroha_data_model::sorafs::final_promotion_account_custody::{
    FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1,
    FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1,
};

#[test]
fn account_normal_capacity_preserves_two_emergency_revocations_without_publishing() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 2_000, |tx| {
        // This coherent suffix exercises capacity checks directly, not a fabricated full ledger.
        let mut tail = history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        tail.record.revision = FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1;
        tail.record.predecessor_digest = [50; 32];
        tail.index.revision = tail.record.revision;
        tail.index.digest = history::control_digest::<AccountPurpose>(&tail.record).unwrap();
        let before = retained(tx);
        let policy = encode(&f.policy).unwrap();
        for action in [
            ControlAction::Configure(&policy),
            ControlAction::Enroll(&[]),
        ] {
            assert!(matches!(
                history::prepare_control::<AccountPurpose>(
                    tx,
                    &f.manager,
                    Some(&tail),
                    ControlTransition {
                        deployment: DEPLOYMENT,
                        expected_revision: tail.index.revision,
                        expected_digest: tail.index.digest,
                        request_digest: [51; 32],
                        action
                    }
                ),
                Err(HistoryError::Capacity)
            ));
            assert_eq!(retained(tx), before);
        }
        let first = history::prepare_control::<AccountPurpose>(
            tx,
            &f.manager,
            Some(&tail),
            ControlTransition {
                deployment: DEPLOYMENT,
                expected_revision: tail.index.revision,
                expected_digest: tail.index.digest,
                request_digest: [52; 32],
                action: ControlAction::Revoke {
                    signer: true,
                    attester: false,
                },
            },
        )
        .unwrap();
        let first_control =
            history::staging_fixture::staged_control::<AccountPurpose>(&first, DEPLOYMENT);
        assert_eq!(
            first_control.index.revision,
            FINAL_PROMOTION_ACCOUNT_CUSTODY_NORMAL_REVISIONS_V1 + 1
        );
        assert!(first_control.state.signer_revoked && !first_control.state.attester_revoked);
        let second = history::prepare_control::<AccountPurpose>(
            tx,
            &f.manager,
            Some(&first_control),
            ControlTransition {
                deployment: DEPLOYMENT,
                expected_revision: first_control.index.revision,
                expected_digest: first_control.index.digest,
                request_digest: [53; 32],
                action: ControlAction::Revoke {
                    signer: false,
                    attester: true,
                },
            },
        )
        .unwrap();
        let second_control =
            history::staging_fixture::staged_control::<AccountPurpose>(&second, DEPLOYMENT);
        assert_eq!(
            second_control.index.revision,
            FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1
        );
        assert!(second_control.state.signer_revoked && second_control.state.attester_revoked);
        {
            let revision = FINAL_PROMOTION_ACCOUNT_CUSTODY_MAX_REVISIONS_V1;
            assert!(matches!(
                history::prepare_control::<AccountPurpose>(
                    tx,
                    &f.manager,
                    Some(&second_control),
                    ControlTransition {
                        deployment: DEPLOYMENT,
                        expected_revision: revision,
                        expected_digest: second_control.index.digest,
                        request_digest: [54; 32],
                        action: ControlAction::Revoke {
                            signer: true,
                            attester: true
                        }
                    }
                ),
                Err(HistoryError::Capacity)
            ));
        }
        assert_eq!(
            history::control_revision::<AccountPurpose>(&ControlTransition {
                deployment: DEPLOYMENT,
                expected_revision: u64::MAX,
                expected_digest: second_control.index.digest,
                request_digest: [55; 32],
                action: ControlAction::Revoke {
                    signer: true,
                    attester: true
                },
            }),
            Err(HistoryError::Capacity)
        );
        assert_eq!(
            retained(tx),
            before,
            "prepared control writes have not been published"
        );
    });
}

#[test]
fn account_preparation_rejects_immutable_collisions_without_publishing_earlier_writes() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 2_000, |tx| {
        let current = history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        let action = || ControlTransition {
            deployment: DEPLOYMENT,
            expected_revision: current.index.revision,
            expected_digest: current.index.digest,
            request_digest: [61; 32],
            action: ControlAction::Revoke {
                signer: true,
                attester: false,
            },
        };
        let prepared =
            history::prepare_control::<AccountPurpose>(tx, &f.manager, Some(&current), action())
                .unwrap();
        let before = retained(tx);
        assert!(prepared.len() >= 3);
        let staged =
            history::staging_fixture::staged_control::<AccountPurpose>(&prepared, DEPLOYMENT);
        assert_eq!(retained(tx), before);
        let collision = history::control_height_key::<AccountPurpose>(
            DEPLOYMENT,
            staged.record.execution.height,
            staged.record.execution.ordinal,
        )
        .unwrap();
        tx.world
            .smart_contract_state
            .insert(collision.clone(), vec![62]);
        let corrupted_before = retained(tx);
        assert!(matches!(
            history::prepare_control::<AccountPurpose>(tx, &f.manager, Some(&current), action()),
            Err(HistoryError::CorruptHistory)
        ));
        assert_eq!(retained(tx), corrupted_before);
        tx.world.smart_contract_state.remove(collision);
        assert_eq!(retained(tx), before);
    });
}
