//! Native account-custody fixtures with software-signed simulated device attestation.
//! These fixtures do not qualify hardware, independent time, rollback protection or consensus.
use super::*;
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
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionAccountCustody,
    CanOperateSorafsFinalPromotion,
};
use sorafs_manifest::signer::{
    custody::{
        SIGNER_CUSTODY_MAGIC_V1, SIGNER_CUSTODY_VERSION_V1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1, SignerCustodyRecordV1, SignerCustodyStatementV1,
    },
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use std::sync::Arc;

pub(super) const DEPLOYMENT: &str = "promotion-primary";
pub(super) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked fixture key")
}
pub(super) struct Fixture {
    pub(super) state: State,
    pub(super) manager: AccountId,
    pub(super) observer: AccountId,
    pub(super) target: AccountId,
    pub(super) other: AccountId,
    pub(super) policy: SignerCustodyPolicyV1,
    pub(super) attester: KeyPair,
}
pub(super) fn fixture() -> Fixture {
    let manager = AccountId::new(key(1).public_key().clone());
    let observer = AccountId::new(key(2).public_key().clone());
    let other = AccountId::new(key(3).public_key().clone());
    let target = AccountId::new(key(4).public_key().clone());
    let mut world = World::new();
    for account in [&manager, &observer, &other, &target] {
        let (id, value) = Account::new(account.clone())
            .build(&manager)
            .into_key_value();
        world.accounts.insert(id, value);
    }
    for (account, permission) in [
        (
            &manager,
            Permission::from(CanManageSorafsFinalPromotionAccountCustody {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &observer,
            Permission::from(CanCheckSorafsFinalPromotionAccountCustody {
                deployment_id: DEPLOYMENT.into(),
            }),
        ),
        (
            &target,
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
            role: SignerRoleV1::FinalPromotionAccountTransaction,
            purpose: SignerPurposeBindingV1::FinalPromotionAccountTransaction {
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
        observer,
        target,
        other,
        policy,
        attester,
    }
}
pub(super) fn transact(
    state: &mut State,
    now: u64,
    call: impl FnOnce(&mut StateTransaction<'_, '_>),
) {
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
pub(super) fn instruction(
    tx: &StateTransaction<'_, '_>,
    action: Action,
) -> MutateSorafsFinalPromotionAccountCustody {
    let old =
        history::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT).expect("coherent control");
    MutateSorafsFinalPromotionAccountCustody {
        deployment_id: DEPLOYMENT.into(),
        expected_control_revision: old.as_ref().map_or(0, |value| value.index.revision),
        expected_control_digest: old.as_ref().map_or([0; 32], |value| value.index.digest),
        action,
    }
}
pub(super) fn configure(f: &mut Fixture) {
    let bytes = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        instruction(tx, Action::Configure(bytes))
            .execute(&f.manager, tx)
            .expect("configure")
    });
}
pub(super) fn snapshot(f: &Fixture) -> FinalPromotionAccountCustodySnapshotV1 {
    read_final_promotion_account_custody_at_v1(
        &f.state.view(),
        &f.policy.binding,
        f.state.view().block_hashes().len() as u64,
    )
    .expect("coherent read")
    .expect("configured")
}
pub(super) fn attest(f: &Fixture, now: u64, expires: u64) -> Vec<u8> {
    let current = snapshot(f);
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
pub(super) fn enroll(f: &mut Fixture) {
    let bytes = attest(f, 1_500, 100_000);
    transact(&mut f.state, 1_500, |tx| {
        instruction(tx, Action::Enroll(bytes))
            .execute(&f.manager, tx)
            .expect("enroll")
    });
}
