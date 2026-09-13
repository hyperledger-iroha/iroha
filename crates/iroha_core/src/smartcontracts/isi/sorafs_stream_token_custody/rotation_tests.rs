//! Native generation, idempotency and finite emergency capacity regressions.
use super::*;
fn rows(tx: &StateTransaction<'_, '_>) -> Vec<(StatePath, Vec<u8>)> {
    tx.world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
#[test]
fn revoked_generations_cannot_revive_through_policy_rotation_and_enrollment_sequence_survives() {
    let mut f = fixture();
    configure(&mut f);
    let signed = attest(&f, 1_500);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, f.provider, Action::Enroll(signed))
            .execute(&f.authority, tx)
            .expect("enroll")
    });
    transact(&mut f.state, 2_000, |tx| {
        instruction(
            tx,
            f.provider,
            Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: true,
                attester: true,
            }),
        )
        .execute(&f.authority, tx)
        .expect("revoke both");
        let old = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        let mut next = f.policy.clone();
        next.binding.policy_revision += 1;
        next.binding.policy_digest = [13; 32];
        instruction(
            tx,
            f.provider,
            Action::Configure(encode(&next).expect("policy")),
        )
        .execute(&f.authority, tx)
        .expect("policy update retains revocation");
        let changed = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        assert!(changed.state.signer_revoked && changed.state.attester_revoked);
        assert!(changed.state.active_head.is_none());
        assert_eq!(changed.state.next_sequence, old.state.next_sequence);
        assert_eq!(
            changed.state.predecessor_digest,
            old.state.predecessor_digest
        );
        next.binding.key_revision += 1;
        next.binding.public_key = key(14).public_key().clone();
        next.binding.key_handle = "pkcs11:stream/key-2".into();
        next.attester_authority.key_revision += 1;
        next.attester_public_key = key(15).public_key().clone();
        instruction(
            tx,
            f.provider,
            Action::Configure(encode(&next).expect("policy")),
        )
        .execute(&f.authority, tx)
        .expect("fresh governed key generations");
        let fresh = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        assert!(!fresh.state.signer_revoked && !fresh.state.attester_revoked);
        assert_eq!(fresh.state.next_sequence, 2);
        assert_eq!(fresh.state.predecessor_digest, old.state.predecessor_digest);
        assert!(fresh.state.active_head.is_none());
    });
}
#[test]
fn historical_key_reuse_and_key_or_policy_rollback_are_rejected_atomically() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 1_500, |tx| {
        let mut next = f.policy.clone();
        next.binding.key_revision = 2;
        next.binding.public_key = key(16).public_key().clone();
        next.binding.key_handle = "pkcs11:stream/key-2".into();
        next.attester_authority.key_revision = 2;
        next.attester_public_key = key(17).public_key().clone();
        instruction(
            tx,
            f.provider,
            Action::Configure(encode(&next).expect("policy")),
        )
        .execute(&f.authority, tx)
        .expect("rotate");
        let before = rows(tx);
        let mut reuse = next.clone();
        reuse.binding.key_revision = 3;
        reuse.binding.public_key = f.policy.binding.public_key.clone();
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&reuse).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        let mut reuse = next.clone();
        reuse.attester_authority.key_revision = 3;
        reuse.attester_public_key = f.policy.attester_public_key.clone();
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&reuse).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        let mut same_key = next.clone();
        same_key.binding.key_revision = 3;
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&same_key).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        let mut rollback = next.clone();
        rollback.binding.policy_revision = 0;
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&rollback).expect("policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(rows(tx), before);
    });
}
#[test]
fn exact_authorized_historical_retry_never_reactivates_replaced_or_revoked_state() {
    let mut f = fixture();
    let original = MutateSorafsStreamTokenCustody {
        provider_id: f.provider,
        expected_revision: 0,
        expected_digest: [0; 32],
        action: Action::Configure(encode(&f.policy).expect("policy")),
    };
    transact(&mut f.state, 1_000, |tx| {
        original
            .clone()
            .execute(&f.authority, tx)
            .expect("first configure")
    });
    transact(&mut f.state, 1_500, |tx| {
        instruction(
            tx,
            f.provider,
            Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: true,
                attester: true,
            }),
        )
        .execute(&f.authority, tx)
        .expect("revoke");
        let mut policy = f.policy.clone();
        policy.binding.policy_revision = 2;
        policy.binding.policy_digest = [18; 32];
        instruction(
            tx,
            f.provider,
            Action::Configure(encode(&policy).expect("policy")),
        )
        .execute(&f.authority, tx)
        .expect("later policy");
        let before = rows(tx);
        original
            .clone()
            .execute(&f.authority, tx)
            .expect("exact historical retry");
        assert_eq!(rows(tx), before);
        let head = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        assert!(head.state.signer_revoked && head.state.attester_revoked);
        assert_eq!(head.state.policy.binding.policy_revision, 2);
        let mut changed = original.clone();
        changed.action = Action::Configure(encode(&policy).expect("changed payload"));
        assert!(changed.execute(&f.authority, tx).is_err());
        let mut permissions = Permissions::new();
        permissions.insert(Permission::from(CanManageSorafsStreamTokenCustody {
            provider_id: f.provider,
        }));
        tx.world
            .account_permissions
            .insert(f.other.clone(), permissions);
        assert!(original.clone().execute(&f.other, tx).is_err());
        tx.world.account_permissions.remove(f.authority.clone());
        assert!(original.clone().execute(&f.authority, tx).is_err());
        assert_eq!(rows(tx), before);
    });
}
#[test]
fn normal_capacity_reserves_two_terminal_revocations_without_reset_or_pruning() {
    let mut f = fixture();
    transact(&mut f.state, 1_000, |tx| {
        let mut policy = f.policy.clone();
        for revision in 1..=STREAM_TOKEN_CUSTODY_NORMAL_REVISIONS_V1 {
            policy.binding.policy_revision = revision;
            policy.binding.policy_digest = *Hash::new(revision.to_le_bytes()).as_ref();
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&policy).expect("bounded policy")),
            )
            .execute(&f.authority, tx)
            .expect("normal history slot");
        }
        let before = rows(tx);
        policy.binding.policy_revision += 1;
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Configure(encode(&policy).expect("next policy"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(rows(tx), before);
        instruction(
            tx,
            f.provider,
            Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: true,
                attester: false,
            }),
        )
        .execute(&f.authority, tx)
        .expect("reserved signer revocation");
        instruction(
            tx,
            f.provider,
            Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: false,
                attester: true,
            }),
        )
        .execute(&f.authority, tx)
        .expect("reserved attester revocation");
        let head = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        assert_eq!(head.index.revision, STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1);
        assert!(head.state.signer_revoked && head.state.attester_revoked);
        let before = rows(tx);
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                    signer: true,
                    attester: true
                })
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(rows(tx), before);
        assert!(
            tx.world
                .smart_contract_state
                .get(&record_key(f.provider, 1))
                .is_some()
        );
        assert_eq!(head.state.next_sequence, 1);
        assert!(head.state.active_head.is_none());
    });
}
#[test]
fn earlier_revocations_consume_total_capacity_and_noop_revoke_does_not_consume_a_slot() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 1_500, |tx| {
        instruction(
            tx,
            f.provider,
            Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                signer: true,
                attester: false,
            }),
        )
        .execute(&f.authority, tx)
        .expect("ordinary-slot revocation");
        let before = rows(tx);
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Revoke(SorafsStreamTokenCustodyRevocationV1 {
                    signer: true,
                    attester: false
                })
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(rows(tx), before);
        let head = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        assert_eq!(head.index.revision, 2);
        assert!(head.state.signer_revoked);
    });
}

#[test]
fn historical_retry_rejects_coherent_but_wrong_chain_restored_control() {
    let mut f = fixture();
    let original = MutateSorafsStreamTokenCustody {
        provider_id: f.provider,
        expected_revision: 0,
        expected_digest: [0; 32],
        action: Action::Configure(encode(&f.policy).expect("policy")),
    };
    transact(&mut f.state, 1_000, |tx| {
        original
            .clone()
            .execute(&f.authority, tx)
            .expect("first configure");
        let mut current = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head");
        current.state.policy.binding.chain_id = "other-chain".into();
        current.record.control_state = encode(&current.state).expect("wrong-chain canonical state");
        current.index.digest = record_digest(&current.record).expect("wrong-chain record digest");
        let index_bytes = encode(&current.index).expect("index");
        tx.world.smart_contract_state.insert(
            record_key(f.provider, 1),
            encode(&current.record).expect("record"),
        );
        tx.world
            .smart_contract_state
            .insert(head_key(f.provider), index_bytes.clone());
        tx.world
            .smart_contract_state
            .insert(height_key(f.provider, 1, 0), index_bytes.clone());
        for (signer, key) in [
            (true, &f.policy.binding.public_key),
            (false, &f.policy.attester_public_key),
        ] {
            tx.world.smart_contract_state.insert(
                key_path(f.provider, signer, key).expect("key index"),
                index_bytes.clone(),
            );
        }
        assert!(
            read_active(tx.world(), f.provider).is_ok(),
            "structurally coherent adversarial restore"
        );
        let before = rows(tx);
        assert!(
            original.execute(&f.authority, tx).is_err(),
            "native chain binding precedes historical success"
        );
        assert_eq!(rows(tx), before);
    });
}
