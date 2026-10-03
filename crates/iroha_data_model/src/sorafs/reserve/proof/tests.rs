//! Genuine finality signatures over explicitly synthetic reserve World preimages, not execution qualification.

use super::*;
use crate::{
    IntoKeyValue, Registrable,
    account::Account,
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
    permission::Permission,
    sorafs::reserve::{
        RESERVE_AUTHORITY_POLICY_VERSION_V1, ReservePolicyV1, history::ReserveEventJournalHeadV1,
    },
    sumeragi_finality::{
        WorldStateElementKindV1, WorldStateSnapshotEntryV1, test_fixtures::NativeFinalityFixture,
        world_state_value_hash_v1,
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_model_base::state_path::StatePath;
use iroha_primitives::{json::Json, numeric::NumericSpec};
use sorafs_manifest::deal::XorQuantity;

const CHAIN: &str = "reserve-policy-synthetic-world";
fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}
fn policy() -> ReserveAuthorityPolicyV1 {
    ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
        custody_account: account(2),
        treasury_account: account(3),
        operations_authority: account(4),
        decision_authority: account(5),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: XorQuantity::try_from_micro(1_000_000_000).unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    }
}
fn original(policy: &ReserveAuthorityPolicyV1) -> ReserveStateV1 {
    ReserveStateV1 {
        policy: ReserveAuthorityPolicyRecordV1 {
            policy: policy.clone(),
            policy_digest: policy.digest().unwrap(),
            activated_by: account(1),
            activated_at_unix: 1,
        },
        journal_head: ReserveEventJournalHeadV1 {
            last_sequence: 1,
            last_target_block_height: 2,
            last_event_index: 0,
        },
    }
}
fn schema() -> Hash {
    Hash::new(b"independently qualified reserve schema specimen")
}
fn row<K: norito::codec::Encode, V: norito::codec::Encode>(
    field: &str,
    key: &K,
    value: &V,
) -> WorldStateSnapshotEntryV1 {
    WorldStateSnapshotEntryV1 {
        field_id: field.into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(key).unwrap()),
        value_hash: world_state_value_hash_v1(value).unwrap(),
    }
}
fn sort(proof: &mut ReservePolicyProofV1) {
    proof
        .world
        .entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
}
fn rebuild(proof: &mut ReservePolicyProofV1, policy: &ReserveAuthorityPolicyV1) {
    let manager = account(1);
    let mut entries = Vec::new();
    for id in [
        &manager,
        &policy.custody_account,
        &policy.treasury_account,
        &policy.operations_authority,
        &policy.decision_authority,
    ] {
        let (_, value) = Account::new(id.clone()).build(&manager).into_key_value();
        entries.push(row("world.accounts", id, &value));
    }
    let definition = AssetDefinition::new(
        policy.asset_definition.clone(),
        "Reserve XOR",
        NumericSpec::fractional(6),
        AssetBalancePolicy::Global,
        None,
    )
    .build(&manager);
    entries.push(row(
        "world.asset_definitions",
        &policy.asset_definition,
        &definition,
    ));
    entries.push(row(
        "world.account_permissions",
        &manager,
        &proof.manager_permissions,
    ));
    if let Some(bytes) = &proof.current {
        entries.push(row(
            "world.smart_contract_state",
            reserve_state_key(),
            bytes,
        ));
    }
    proof.world.entries = entries;
    sort(proof);
}
fn certify(
    native: &mut NativeFinalityFixture,
    proof: &ReservePolicyProofV1,
) -> VerifiedSumeragiBlock {
    let mut header = native.next_header();
    header.creation_time_ms = header.creation_time_ms.max(2_000);
    let block = native.block_with_submitted_work(header);
    let certificate = native.certify_with_world_root(block, proof.world.root().unwrap());
    native
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap()
}
fn fixture(
    present: bool,
) -> (
    NativeFinalityFixture,
    ReservePolicyProofV1,
    ReserveAuthorityPolicyV1,
    VerifiedSumeragiBlock,
) {
    let mut native = NativeFinalityFixture::start(CHAIN);
    let policy = policy();
    let mut proof = ReservePolicyProofV1 {
        world: WorldStateSnapshotV1 {
            schema_hash: schema(),
            entries: Vec::new(),
        },
        manager_permissions: Permissions::from([reserve_policy_permission()]),
        current: present.then(|| norito::encode_canonical(&original(&policy)).unwrap()),
    };
    rebuild(&mut proof, &policy);
    let block = certify(&mut native, &proof);
    (native, proof, policy, block)
}
fn verify(
    proof: &ReservePolicyProofV1,
    policy: &ReserveAuthorityPolicyV1,
    block: &VerifiedSumeragiBlock,
) -> Result<VerifiedReservePolicyStateV1, FinalityError> {
    proof.verify(
        CHAIN,
        block.commitment().schedule.current.network_id,
        &account(1),
        policy,
        schema(),
        block,
    )
}

#[test]
fn exact_policy_and_singleton_absence_authenticate_at_the_selected_cut() {
    for present in [false, true] {
        let (_, proof, policy, block) = fixture(present);
        let result = verify(&proof, &policy, &block).unwrap();
        assert_eq!(
            result.network_id(),
            block.commitment().schedule.current.network_id
        );
        assert_eq!(result.manager(), &account(1));
        assert_eq!(result.height(), block.height());
        assert_eq!(result.context_id(), block.context_id());
        assert_eq!(result.current().is_some(), present);
        if let Some(current) = result.current() {
            assert_eq!(current, &original(&policy).policy);
        }
        let borrowed = ReservePolicyProofRefV1::new(
            &proof.world,
            &proof.manager_permissions,
            proof.current.as_ref(),
        );
        let bytes = norito::encode_canonical(&proof).unwrap();
        assert_eq!(norito::encode_canonical(&borrowed).unwrap(), bytes);
        assert_eq!(
            norito::json::to_vec(&borrowed).unwrap(),
            norito::json::to_vec(&proof).unwrap()
        );
        assert_eq!(ReservePolicyProofV1::decode_frame(&bytes).unwrap(), proof);
        if let Some(bytes) = &proof.current {
            assert_eq!(
                ReserveStateV1::decode_frame(bytes).unwrap(),
                original(&policy)
            );
        }
    }
}

#[test]
fn policy_absence_does_not_claim_empty_namespace_or_initial_eligibility() {
    let (mut native, mut proof, policy, _) = fixture(false);
    // Deliberately inconsistent synthetic World: Core initial Set must reject this namespace.
    // This reader authenticates singleton absence only and exposes no eligibility constructor.
    let orphan: StatePath = "sorafs_reserve_orphan_v1".parse().unwrap();
    proof
        .world
        .entries
        .push(row("world.smart_contract_state", &orphan, &vec![9_u8]));
    sort(&mut proof);
    let block = certify(&mut native, &proof);
    assert!(verify(&proof, &policy, &block).unwrap().current().is_none());
}

#[test]
fn independent_scope_policy_roles_and_manager_cannot_be_substituted() {
    let (_, proof, policy, block) = fixture(true);
    let network = block.commitment().schedule.current.network_id;
    assert!(
        proof
            .verify(
                "other-chain",
                network,
                &account(1),
                &policy,
                schema(),
                &block
            )
            .is_err()
    );
    assert!(
        proof
            .verify(CHAIN, network, &account(9), &policy, schema(), &block)
            .is_err()
    );
    assert!(
        proof
            .verify(
                CHAIN,
                network,
                &account(1),
                &policy,
                Hash::new(b"unqualified schema"),
                &block
            )
            .is_err()
    );
    // NetworkId follows signed genesis, not the human-readable chain label.
    let other = NativeFinalityFixture::start_with_mode(
        "other-reserve-network",
        crate::parameter::system::SumeragiConsensusMode::Npos,
    );
    assert_ne!(other.network_id(), network);
    assert!(
        proof
            .verify(
                CHAIN,
                other.network_id(),
                &account(1),
                &policy,
                schema(),
                &block
            )
            .is_err()
    );
    let mut changed = Vec::new();
    let mut p = policy.clone();
    p.custody_account = account(9);
    changed.push(p);
    let mut p = policy.clone();
    p.treasury_account = account(9);
    changed.push(p);
    let mut p = policy.clone();
    p.operations_authority = account(9);
    changed.push(p);
    let mut p = policy.clone();
    p.decision_authority = account(9);
    changed.push(p);
    let mut p = policy.clone();
    p.grace_period_days += 1;
    changed.push(p);
    let mut p = policy.clone();
    p.revision += 1;
    p.predecessor_policy_digest = Some([1; 32]);
    changed.push(p);
    let mut p = policy.clone();
    let mut uuid = [8; 16];
    uuid[6] = 0x40;
    uuid[8] = 0x80;
    p.asset_definition = AssetDefinitionId::from_uuid_bytes(uuid).unwrap();
    changed.push(p);
    for p in changed {
        assert!(verify(&proof, &p, &block).is_err());
    }
    let mut changed = proof.clone();
    changed.current = None;
    assert!(
        verify(&changed, &policy, &block).is_err(),
        "concealed singleton is not absence"
    );
}

#[test]
fn direct_permission_and_all_selected_entity_keys_are_required() {
    let (mut native, proof, policy, _) = fixture(false);
    let manager = account(1);
    let selected = [
        &manager,
        &policy.custody_account,
        &policy.treasury_account,
        &policy.operations_authority,
        &policy.decision_authority,
    ]
    .into_iter()
    .map(|key| ("world.accounts", world_state_value_hash_v1(key).unwrap()))
    .chain(std::iter::once((
        "world.asset_definitions",
        world_state_value_hash_v1(&policy.asset_definition).unwrap(),
    )))
    .collect::<Vec<_>>();
    for (field, key) in selected {
        let mut missing = proof.clone();
        missing
            .world
            .entries
            .retain(|entry| !(entry.field_id == field && entry.key_hash == Some(key)));
        let block = certify(&mut native, &missing);
        assert!(
            verify(&missing, &policy, &block).is_err(),
            "missing {field}"
        );
    }
    for permissions in [
        Permissions::new(),
        Permissions::from([Permission::new(
            "CanSetSorafsReservePolicy".into(),
            Json::new(7_u64),
        )]),
        Permissions::from([Permission::new(
            "CanReadAllLedgerData".into(),
            Json::new(()),
        )]),
    ] {
        let mut changed = proof.clone();
        changed.manager_permissions = permissions;
        rebuild(&mut changed, &policy);
        let block = certify(&mut native, &changed);
        assert!(verify(&changed, &policy, &block).is_err());
    }
    let mut changed = proof.clone();
    changed.manager_permissions.insert(Permission::new(
        "CanReadAllLedgerData".into(),
        Json::new(()),
    ));
    let block = certify(&mut native, &proof);
    assert!(
        verify(&changed, &policy, &block).is_err(),
        "original permission-row hash is mandatory"
    );
}

#[test]
fn authenticated_present_bytes_still_require_native_structure_and_activation_provenance() {
    let (mut native, proof, policy, _) = fixture(true);
    let original = original(&policy);
    let mut changed = Vec::new();
    let mut state = original.clone();
    state.policy.policy_digest[0] ^= 1;
    changed.push(state);
    let mut state = original.clone();
    state.policy.activated_at_unix = 0;
    changed.push(state);
    let mut state = original.clone();
    state.policy.activated_at_unix = u64::MAX;
    changed.push(state);
    let mut state = original.clone();
    state.policy.activated_by = account(9);
    changed.push(state);
    let mut state = original.clone();
    state.policy.policy.revision = 0;
    changed.push(state);
    let mut state = original.clone();
    state.journal_head.last_sequence = 0;
    changed.push(state);
    let mut state = original.clone();
    state.journal_head.last_target_block_height = 0;
    changed.push(state);
    let mut state = original;
    state.journal_head.last_target_block_height = u64::MAX;
    changed.push(state);
    for state in changed {
        let mut candidate = proof.clone();
        candidate.current = Some(norito::encode_canonical(&state).unwrap());
        rebuild(&mut candidate, &policy);
        let block = certify(&mut native, &candidate);
        assert!(verify(&candidate, &policy, &block).is_err());
    }
    let mut candidate = proof;
    candidate.current.as_mut().unwrap().push(0);
    rebuild(&mut candidate, &policy);
    let block = certify(&mut native, &candidate);
    assert!(verify(&candidate, &policy, &block).is_err());
}

#[test]
fn canonical_response_component_and_cumulative_decode_bounds_are_enforced() {
    let (_, proof, policy, block) = fixture(true);
    let bytes = norito::encode_canonical(&proof).unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(ReservePolicyProofV1::decode_frame(&trailing).is_err());
    assert!(ReservePolicyProofV1::decode_frame(&[]).is_err());
    assert!(
        ReservePolicyProofV1::decode_frame(&vec![0; MAX_RESERVE_POLICY_PROOF_BYTES_V1 + 1])
            .is_err()
    );
    assert!(ReserveStateV1::decode_frame(&vec![0; STATE_MAX_BYTES + 1]).is_err());
    let refused = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(131_072, MAX_RESERVE_POLICY_PROOF_BYTES_V1, 131_072, 0, 64),
        || ReservePolicyProofV1::decode_frame(&bytes),
    );
    assert!(refused.unwrap_err().is_decode_resource_limit());
    assert!(ReservePolicyProofV1::decode_frame(&bytes).is_ok());
    let mut too_many = proof.clone();
    for index in 0..MAX_RESERVE_POLICY_MANAGER_PERMISSIONS_V1 {
        too_many
            .manager_permissions
            .insert(Permission::new(format!("Extra{index}"), Json::new(())));
    }
    assert!(verify(&too_many, &policy, &block).is_err());
    let mut too_large = proof;
    too_large.manager_permissions.insert(Permission::new(
        "Extra".into(),
        Json::new("x".repeat(MAX_RESERVE_POLICY_PERMISSION_BYTES_V1)),
    ));
    assert!(verify(&too_large, &policy, &block).is_err());
}
