//! Actual native custody mutation and same-State historical reader regressions.
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::Account,
    block::{BlockHeader, builder::BlockBuilder},
    permission::Permissions,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use std::sync::Arc;
fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked fixture key")
}
struct Fixture {
    state: State,
    authority: AccountId,
    other: AccountId,
    provider: ProviderId,
    policy: StreamTokenCustodyPolicyV1,
    attester: KeyPair,
}
fn fixture() -> Fixture {
    let authority = AccountId::new(key(1).public_key().clone());
    let other = AccountId::new(key(2).public_key().clone());
    let provider = ProviderId::new([3; 32]);
    let mut world = World::new();
    for account in [&authority, &other] {
        let (id, value) = Account::new(account.clone())
            .build(&authority)
            .into_key_value();
        world.accounts.insert(id, value);
    }
    let mut permissions = Permissions::new();
    permissions.insert(Permission::from(CanManageSorafsStreamTokenCustody {
        provider_id: provider,
    }));
    world
        .account_permissions
        .insert(authority.clone(), permissions);
    world.provider_owners.insert(provider, authority.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let attester = key(7);
    let policy = StreamTokenCustodyPolicyV1 {
        binding: SignerCustodyBindingV1 {
            chain_id: state.view().chain_id().to_string(),
            network_id: *state.view().network_id().as_bytes(),
            runtime_handle: "hsm://stream/primary".into(),
            key_handle: "pkcs11:stream/key-1".into(),
            service_id: "stream-service".into(),
            administrator_id: "stream-admin".into(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken {
                provider_id: *provider.as_bytes(),
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
        active_until_unix_ms: 30_000,
        max_validity_ms: 10_000,
        max_anchor_age_ms: 2_000,
    };
    Fixture {
        state,
        authority,
        other,
        provider,
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
    provider: ProviderId,
    action: Action,
) -> MutateSorafsStreamTokenCustody {
    let old = read_active(tx.world(), provider).expect("coherent active control");
    MutateSorafsStreamTokenCustody {
        provider_id: provider,
        expected_revision: old.as_ref().map_or(0, |c| c.index.revision),
        expected_digest: old.as_ref().map_or([0; 32], |c| c.index.digest),
        action,
    }
}
fn configure(f: &mut Fixture) {
    let policy = encode(&f.policy).expect("policy");
    transact(&mut f.state, 1_000, |tx| {
        instruction(tx, f.provider, Action::Configure(policy))
            .execute(&f.authority, tx)
            .expect("authorized configure")
    });
}
fn attest(f: &Fixture, now: u64) -> Vec<u8> {
    let snapshot = read_stream_token_custody_control_at_v1(
        &f.state.view(),
        &f.policy.binding,
        f.state.view().block_hashes().len() as u64,
    )
    .expect("committed read")
    .expect("configured control");
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: f.policy.binding.clone(),
        authority: f.policy.attester_authority.clone(),
        anchor: snapshot.anchor,
        sequence: snapshot.state.next_sequence,
        predecessor_digest: snapshot.state.predecessor_digest,
        issued_at_unix_ms: now,
        expires_at_unix_ms: now + 5_000,
        hardware_identity_digest: [8; 32],
        evidence_digest: [9; 32],
        generated_in_hardware: true,
        exportable: false,
        ever_exported: false,
        revoked: false,
    };
    let signature = Signature::try_new(
        f.attester.private_key(),
        &statement
            .signing_payload()
            .expect("canonical signing preimage"),
    )
    .expect("attestation");
    encode(&SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().expect("Ed25519 signature"),
    })
    .expect("attested frame")
}
#[test]
fn committed_configuration_enrollment_and_history_use_exact_native_anchor() {
    let mut f = fixture();
    configure(&mut f);
    let approval = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
        .expect("approval")
        .expect("configured");
    assert!(approval.state.active_head.is_none());
    let bytes = attest(&f, 1_500);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, f.provider, Action::Enroll(bytes))
            .execute(&f.authority, tx)
            .expect("full verified enrollment")
    });
    let enrolled = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 2)
        .expect("enrolled")
        .expect("head");
    assert_eq!(enrolled.state.next_sequence, 2);
    assert_eq!(
        enrolled
            .state
            .active_head
            .expect("enrolled head")
            .approved_anchor,
        approval.anchor
    );
    assert_ne!(enrolled.anchor.state_digest, approval.anchor.state_digest);
    assert_eq!(
        read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 1)
            .expect("historical"),
        Some(approval)
    );
    let renewal = attest(&f, 2_000);
    transact(&mut f.state, 2_000, |tx| {
        instruction(tx, f.provider, Action::Enroll(renewal))
            .execute(&f.authority, tx)
            .expect("same-key renewal")
    });
    let renewed = read_stream_token_custody_control_at_v1(&f.state.view(), &f.policy.binding, 3)
        .expect("renewal")
        .expect("renewed head");
    assert_eq!(renewed.state.next_sequence, 3);
    assert_eq!(
        renewed.state.active_head.expect("head").approved_anchor,
        enrolled.anchor
    );
}
#[test]
fn exact_permission_provider_and_predecessor_are_required_before_any_write() {
    let mut f = fixture();
    transact(&mut f.state, 1_000, |tx| {
        let candidate = instruction(
            tx,
            f.provider,
            Action::Configure(encode(&f.policy).expect("policy")),
        );
        assert!(candidate.clone().execute(&f.other, tx).is_err());
        assert!(
            read_active(tx.world(), f.provider)
                .expect("absent")
                .is_none()
        );
        let mut wrong = candidate.clone();
        wrong.provider_id = ProviderId::new([99; 32]);
        assert!(wrong.execute(&f.authority, tx).is_err());
        candidate
            .clone()
            .execute(&f.authority, tx)
            .expect("exact scoped permission");
        let before = read_active(tx.world(), f.provider)
            .expect("active")
            .expect("record")
            .index;
        let all_rows = tx
            .world
            .smart_contract_state
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        candidate
            .clone()
            .execute(&f.authority, tx)
            .expect("exact authorized historical retry");
        assert_eq!(
            tx.world
                .smart_contract_state
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            all_rows
        );
        let mut wrong_predecessor = candidate;
        wrong_predecessor.expected_digest = [99; 32];
        assert!(wrong_predecessor.execute(&f.authority, tx).is_err());
        assert_eq!(
            read_active(tx.world(), f.provider)
                .expect("active")
                .expect("record")
                .index,
            before
        );
    });
}
#[test]
fn same_block_configuration_cannot_be_used_as_committed_enrollment_anchor() {
    let mut f = fixture();
    configure(&mut f);
    let bytes = attest(&f, 1_500);
    transact(&mut f.state, 1_500, |tx| {
        let mut changed = f.policy.clone();
        changed.binding.policy_revision += 1;
        changed.binding.policy_digest = [12; 32];
        instruction(
            tx,
            f.provider,
            Action::Configure(encode(&changed).expect("policy")),
        )
        .execute(&f.authority, tx)
        .expect("in-block configure");
        let before = read_active(tx.world(), f.provider)
            .expect("current")
            .expect("head")
            .index;
        assert!(
            instruction(tx, f.provider, Action::Enroll(bytes))
                .execute(&f.authority, tx)
                .is_err()
        );
        assert_eq!(
            read_active(tx.world(), f.provider)
                .expect("current")
                .expect("head")
                .index,
            before
        );
        instruction(
            tx,
            f.provider,
            Action::Revoke {
                signer: true,
                attester: false,
            },
        )
        .execute(&f.authority, tx)
        .expect("same-block revoke");
        let revoked = read_active(tx.world(), f.provider)
            .expect("current")
            .expect("head");
        assert_eq!(revoked.index.ordinal, 1);
        assert!(revoked.state.signer_revoked);
    });
}
#[test]
fn tampered_untrusted_expired_and_replayed_enrollment_leave_native_head_unchanged() {
    let mut f = fixture();
    configure(&mut f);
    let valid = attest(&f, 1_500);
    transact(&mut f.state, 1_500, |tx| {
        let before = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head")
            .index;
        let mut corrupt: SignerCustodyRecordV1 = decode(&valid).expect("record");
        corrupt.attestation[0] ^= 1;
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Enroll(encode(&corrupt).expect("frame"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        corrupt = decode(&valid).expect("record");
        corrupt.statement.binding.policy_digest = [33; 32];
        assert!(
            instruction(
                tx,
                f.provider,
                Action::Enroll(encode(&corrupt).expect("frame"))
            )
            .execute(&f.authority, tx)
            .is_err()
        );
        assert_eq!(
            read_active(tx.world(), f.provider)
                .expect("head")
                .expect("head")
                .index,
            before
        );
        instruction(tx, f.provider, Action::Enroll(valid.clone()))
            .execute(&f.authority, tx)
            .expect("valid control");
        let in_block_retry = instruction(tx, f.provider, Action::Enroll(valid.clone()))
            .execute(&f.authority, tx)
            .expect_err("pending enrollment cannot be labelled previous committed state");
        assert_eq!(
            in_block_retry.to_string(),
            rejected(Error::Conflict).to_string()
        );
    });
    let expired = attest(&f, 2_000);
    transact(&mut f.state, 7_000, |tx| {
        let before = read_active(tx.world(), f.provider)
            .expect("head")
            .expect("head")
            .index;
        assert!(
            instruction(tx, f.provider, Action::Enroll(expired))
                .execute(&f.authority, tx)
                .is_err()
        );
        assert_eq!(
            read_active(tx.world(), f.provider)
                .expect("head")
                .expect("head")
                .index,
            before
        );
    });
}

#[path = "reader_tests.rs"]
mod reader_tests;
#[path = "rotation_tests.rs"]
mod rotation_tests;
