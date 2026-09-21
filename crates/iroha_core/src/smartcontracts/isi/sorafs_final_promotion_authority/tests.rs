//! Real native transaction/history regressions with simulated signed hardware enrollment.
//! These tests exercise Core custody/operation authority, not deployment hardware or consensus QC.
use super::*;
use crate::query::signer_custody_history::{
    ControlIndexV1, control_head_key, control_height_key, control_record_key, key_path,
};
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    block::{BlockHeader, builder::BlockBuilder},
    permission::{Permission, Permissions},
    sorafs::final_promotion_authority::{
        FinalPromotionCompleteV1, FinalPromotionExpireV1, FinalPromotionReserveV1,
        FinalPromotionRevocationV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};
use sorafs_manifest::signer::custody_control::SignerCustodyPolicyV1;
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationIntentV1, SignerPurposeBindingV1, SignerRoleV1,
    },
};
use std::sync::Arc;

const DEPLOYMENT: &str = "promotion-primary";
fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked fixture key")
}
struct Fixture {
    state: State,
    manager: AccountId,
    operator: AccountId,
    observer: AccountId,
    other: AccountId,
    policy: SignerCustodyPolicyV1,
    attester: KeyPair,
}
fn fixture() -> Fixture {
    let manager = AccountId::new(key(1).public_key().clone());
    let operator = AccountId::new(key(2).public_key().clone());
    let other = AccountId::new(key(3).public_key().clone());
    let observer = AccountId::new(key(9).public_key().clone());
    let mut world = World::new();
    for account in [&manager, &operator, &other, &observer] {
        let (id, value) = Account::new(account.clone())
            .build(&manager)
            .into_key_value();
        world.accounts.insert(id, value);
    }
    for (account, permission) in [
        (
            &observer,
            Permission::from(CanCheckSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &manager,
            Permission::from(CanManageSorafsFinalPromotionCustody {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &operator,
            Permission::from(CanOperateSorafsFinalPromotion {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &other,
            Permission::from(CanOperateSorafsFinalPromotion {
                deployment_id: "promotion-secondary".into(),
            }),
        ),
    ] {
        let mut permissions = Permissions::new();
        permissions.insert(permission);
        world
            .account_permissions
            .insert(account.clone(), permissions);
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let attester = key(7);
    let policy = SignerCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: state.view().chain_id().to_string(),
            network_id: *state.view().network_id().as_bytes(),
            runtime_handle: "hsm://promotion/primary".into(),
            key_handle: "pkcs11:promotion/key-1".into(),
            service_id: "promotion-service".into(),
            administrator_id: "promotion-admin".into(),
            role: SignerRoleV1::FinalPromotionProvenance,
            purpose: SignerPurposeBindingV1::FinalPromotionProvenance {
                deployment_id: DEPLOYMENT.into(),
            },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: key(4).public_key().clone(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [5; 32],
        },
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "custody-service".into(),
            administrator_id: "custody-admin".into(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [6; 32],
        },
        attester_public_key: attester.public_key().clone(),
        active_from_unix_ms: 100,
        active_until_unix_ms: 500_000,
        max_validity_ms: 120_000,
        max_anchor_age_ms: 60_000,
    };
    Fixture {
        state,
        manager,
        operator,
        observer,
        other,
        policy,
        attester,
    }
}
fn transact(state: &mut State, now: u64, call: impl FnOnce(&mut StateTransaction<'_, '_>)) {
    let height = u64::try_from(state.view().block_hashes().len()).expect("height") + 1;
    let header = BlockHeader::new(
        height.try_into().expect("nonzero height"),
        state.view().latest_block_hash(),
        None,
        now,
        0,
    );
    let mut block = state.block(header.clone());
    let mut tx = block.transaction();
    call(&mut tx);
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit fixture world");
    let signed = BlockBuilder::new(header)
        .try_build_with_signature(0, key(0xFE).private_key())
        .expect("executed fixture block");
    let hash = signed.hash();
    let header = signed.header().clone();
    state
        .kura()
        .store_block(Arc::new(signed))
        .expect("fixture committed block");
    state.push_block_hash_for_testing(hash);
    state.update_latest_block_header_cache_for_tests(header);
}
fn instruction(
    tx: &StateTransaction<'_, '_>,
    action: Action,
) -> MutateSorafsFinalPromotionAuthority {
    let old = read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT).expect("coherent control");
    MutateSorafsFinalPromotionAuthority {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: old.as_ref().map_or(0, |value| value.index.revision),
        expected_control_digest: old.as_ref().map_or([0; 32], |value| value.index.digest),
        action,
    }
}
fn configure(f: &mut Fixture) {
    let bytes = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        instruction(tx, Action::Configure(bytes))
            .execute(&f.manager, tx)
            .expect("configure")
    });
}
fn snapshot(f: &Fixture, operation: Option<[u8; 32]>) -> FinalPromotionAuthoritySnapshotV1 {
    read_final_promotion_authority_at_v1(
        &f.state.view(),
        &f.policy.binding,
        f.state.view().block_hashes().len() as u64,
        operation,
    )
    .expect("coherent read")
    .expect("configured")
}
fn attest(f: &Fixture, now: u64, expires: u64) -> Vec<u8> {
    let current = snapshot(f, None);
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: f.policy.binding.clone(),
        authority: f.policy.attester_authority.clone(),
        anchor: current.custody_anchor,
        sequence: current.control.next_sequence,
        predecessor_digest: current.control.predecessor_digest,
        issued_at_unix_ms: now,
        expires_at_unix_ms: expires,
        evidence_digest: [12; 32],
        revoked: false,
    };
    let signature = Signature::try_new(
        f.attester.private_key(),
        &statement.signing_payload().unwrap(),
    )
    .unwrap();
    encode(&SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().unwrap(),
    })
    .unwrap()
}
fn enroll(f: &mut Fixture) {
    let bytes = attest(f, 1_500, 100_000);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, Action::Enroll(bytes))
            .execute(&f.manager, tx)
            .expect("enroll")
    });
}
fn reserve_request(f: &Fixture, id: u8) -> FinalPromotionReserveV1 {
    let current = snapshot(f, None);
    FinalPromotionReserveV1 {
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: [id; 32],
            request_digest: [id.wrapping_add(1); 32],
            previous_audit: current.operations.audit,
        },
        custody: SignerOperationCustodyV1 {
            record_digest: current.control.active_head.unwrap().record_digest,
            control_state_digest: current.custody_anchor.state_digest,
        },
    }
}
fn reserve(f: &mut Fixture, id: u8, now: u64) -> FinalPromotionOperationRecordV1 {
    let request = reserve_request(f, id);
    transact(&mut f.state, now, |tx| {
        instruction(tx, Action::Reserve(request))
            .execute(&f.operator, tx)
            .expect("reserve")
    });
    snapshot(f, Some([id; 32])).operation.unwrap()
}
fn completion(record: &FinalPromotionOperationRecordV1) -> FinalPromotionCompleteV1 {
    FinalPromotionCompleteV1 {
        intent: record.intent,
        custody: record.custody,
        reservation: record.reservation,
        commitment: SignerOperationCommitmentV1 {
            audit: SignerOperationAuditHeadV1 {
                sequence: record.intent.previous_audit.sequence + 1,
                digest: [20_u8.wrapping_add((record.intent.previous_audit.sequence + 1) as u8); 32],
            },
            response_digest: [22; 32],
        },
        signatures_digest: [23; 32],
    }
}
fn retained(tx: &StateTransaction<'_, '_>) -> Vec<(StatePath, Vec<u8>)> {
    tx.world()
        .smart_contract_state()
        .iter()
        .map(|(key, bytes)| (key.clone(), bytes.clone()))
        .collect()
}

#[test]
fn exact_native_completion_preserves_custody_and_original_reservation() {
    let mut f = fixture();
    configure(&mut f);
    enroll(&mut f);
    let control = snapshot(&f, None);
    let reserved = reserve(&mut f, 31, 2_000);
    assert_eq!(reserved.reservation.expires_at_unix_ms, 62_000);
    assert_eq!(
        snapshot(&f, None).custody_anchor.state_digest,
        control.custody_anchor.state_digest
    );
    let request = completion(&reserved);
    transact(&mut f.state, 3_000, |tx| {
        instruction(tx, Action::Complete(request))
            .execute(&f.operator, tx)
            .expect("timely complete")
    });
    let completed = snapshot(&f, Some([31; 32]));
    let row = completed.operation.unwrap();
    assert_eq!(row.reserved, reserved.reserved);
    assert_eq!(row.custody, reserved.custody);
    assert_eq!(row.reservation, reserved.reservation);
    assert_eq!(completed.operations.revision, 2);
    assert_eq!(completed.operations.fence, 1);
    assert_eq!(completed.operations.audit, request.commitment.audit);
    assert_eq!(completed.operations.active_operation, None);
    assert_eq!(
        completed.custody_anchor.state_digest,
        control.custody_anchor.state_digest
    );
    assert_eq!(
        read_final_promotion_authority_at_v1(&f.state.view(), &f.policy.binding, 3, Some([31; 32]))
            .unwrap()
            .unwrap()
            .operation
            .unwrap(),
        reserved
    );
    // Already committed retries do not renew the reservation or rewrite the native completion.
    transact(&mut f.state, 70_000, |tx| {
        let before = retained(tx);
        instruction(tx, Action::Complete(request))
            .execute(&f.operator, tx)
            .unwrap();
        assert_eq!(retained(tx), before);
    });
}

mod capacity;
mod check;
mod history;
mod rejection;
mod replay_integrity;
mod request_digest;
mod snapshot_integrity;

mod shared_history;
