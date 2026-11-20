//! Scoped admission, grant lifecycle and row ownership at the certified cut.
use super::*;
use crate::state::World;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{Account, Registrable, nexus::UniversalAccountId};
use iroha_executor_data_model::isi::multisig::MultisigProposalId;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn fixture() -> World {
    World::with(
        [],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [],
    )
}

fn install(world: &mut World) {
    super::super::keyed_state::install_for_test(world);
    let mut block = world.block();
    let current = block.runtime_selection.get().clone();
    let published = super::super::change_set::PublishedDefinitions(&block.runtime_definitions);
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"test")));
    let change = activation(&current, &published, network, 2, [1; 32]).unwrap();
    assert_eq!(change.publish, [descriptor()]);
    let applied = change.apply(&current, &published).unwrap();
    for (id, descriptor) in applied.published {
        block.runtime_definitions.insert(id, descriptor);
    }
    *block.runtime_selection.get_mut() = applied.selection;
    block.commit();
}

fn asset(byte: u8) -> AssetDefinitionId {
    let mut id = [byte; 16];
    id[6] = 0x40;
    id[8] = 0x80;
    AssetDefinitionId::from_uuid_bytes(id).unwrap()
}

fn scoped(scope: StateReadScope) -> CanReadStateScope {
    CanReadStateScope { scope }
}

#[test]
fn every_operator_payload_round_trips_through_the_checked_grant_writer() {
    let policy: IdentifierPolicyId = "phone#retail".parse().unwrap();
    let path: StatePath = "sc/native/escrow/".parse().unwrap();
    for (scope, kind, value) in [
        (
            StateReadScope::AssetBalances(asset(1)),
            "AssetBalances",
            norito::json!(asset(1).to_string()),
        ),
        (
            StateReadScope::AssetEscrows(asset(1)),
            "AssetEscrows",
            norito::json!(asset(1).to_string()),
        ),
        (
            StateReadScope::ContractStatePrefix(path.clone()),
            "ContractStatePrefix",
            norito::json!(path.to_string()),
        ),
        (
            StateReadScope::IdentifierClaims(policy),
            "IdentifierClaims",
            norito::json!({"kind":"phone","business_rule":"retail"}),
        ),
        (
            StateReadScope::MultisigRecords(ALICE_ID.clone()),
            "MultisigRecords",
            norito::json!(ALICE_ID.canonical_i105().unwrap()),
        ),
    ] {
        let grant = scoped(scope);
        let typed = Permission::from(grant.clone());
        let json = norito::json!({"scope":{"kind":kind,"value":value}});
        let supplied = Permission::new(NAME.into(), json);
        assert_eq!(typed, supplied);
        assert_eq!(CanReadStateScope::try_from(&supplied).unwrap(), grant);
        assert_eq!(
            &iroha_primitives::json::Json::try_new(grant).unwrap(),
            supplied.payload()
        );
    }
}

#[test]
fn scoped_grants_require_keyed_selection_and_the_global_issuer() {
    let mut world = fixture();
    let grant = scoped(StateReadScope::AssetBalances(asset(1)));
    world
        .account_permissions
        .insert(ALICE_ID.clone(), [CanReadAllLedgerData.into()].into());
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"test")));
    assert!(
        activation(
            world.view().runtime_selection(),
            &super::super::release_catalog().unwrap().definitions,
            network,
            10,
            [1; 32]
        )
        .is_err()
    );
    assert!(validate_activation_inventory(&world.view()).is_ok());
    let row = norito::codec::encode_adaptive(&AssetId::new(asset(1), BOB_ID.clone()));
    install(&mut world);
    let token = Permission::from(grant.clone());
    assert!(read_grant(&world.view(), &BOB_ID, "world.assets", &row).is_none());
    for revoke in [false, true] {
        assert!(guard_account_change(&world.view(), &ALICE_ID, &token, revoke).is_ok());
        assert!(guard_account_change(&world.view(), &BOB_ID, &token, revoke).is_err());
    }
    assert!(guard_role_grant(&world.view(), &token).is_err());
    let bad: Permission =
        scoped(StateReadScope::ContractStatePrefix("abc".parse().unwrap())).into();
    assert!(guard_account_change(&world.view(), &ALICE_ID, &bad, false).is_err());
    world
        .account_permissions
        .insert(BOB_ID.clone(), [token.clone()].into());
    assert!(validate_activation_inventory(&world.view()).is_err());
    assert_eq!(
        read_grant(&world.view(), &BOB_ID, "world.assets", &row),
        Some(&token)
    );
    let mut block = world.block();
    block.runtime_selection.get_mut().restrictions.push(
        iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition: descriptor().id().unwrap(),
            restriction: iroha_data_model::runtime::definition::RestrictionV1::AdmissionClosed(2),
        },
    );
    block.commit();
    assert!(read_grant(&world.view(), &BOB_ID, "world.assets", &row).is_none());
    assert!(guard_account_change(&world.view(), &ALICE_ID, &token, false).is_err());
    assert!(guard_account_change(&world.view(), &ALICE_ID, &token, true).is_ok());
}

#[test]
fn a_scoped_grant_requires_an_executable_revocation_binding() {
    let mut world = fixture();
    install(&mut world);
    world
        .account_permissions
        .insert(ALICE_ID.clone(), [CanReadAllLedgerData.into()].into());
    let token = Permission::from(scoped(StateReadScope::AssetBalances(asset(1))));
    assert!(guard_account_change(&world.view(), &ALICE_ID, &token, false).is_ok());
    let revoke = iroha_data_model::isi::wire_id_of::<iroha_data_model::isi::RevokeBox>().unwrap();
    let mut block = world.block();
    block
        .runtime_selection
        .get_mut()
        .bindings
        .retain(|b| b.wire_id != revoke);
    block.commit();
    assert!(!revocation_executable(world.view().runtime_selection()));
    assert!(guard_account_change(&world.view(), &ALICE_ID, &token, false).is_err());
}

#[test]
fn preexisting_role_token_blocks_activation_even_with_an_unrecognized_payload() {
    use iroha_data_model::role::{Role, RoleId};
    let world = fixture();
    let role: RoleId = "scoped_readers".parse().unwrap();
    let mut block = world.block();
    block.roles.insert(
        role.clone(),
        Role::new(role, ALICE_ID.clone())
            .add_permission(Permission::new(
                NAME.into(),
                iroha_primitives::json::Json::new(9_u32),
            ))
            .build(&ALICE_ID),
    );
    block.commit();
    assert!(validate_activation_inventory(&world.view()).is_err());
}

#[test]
fn balances_prefix_and_multisig_match_only_canonical_requested_keys() {
    use crate::smartcontracts::isi::multisig::{
        multisig_approval_outcome_state_key, multisig_proposal_terminal_execution_state_key,
    };
    let balances = scoped(StateReadScope::AssetBalances(asset(1)));
    for account in [&*ALICE_ID, &*BOB_ID] {
        for (definition, allowed) in [(asset(1), true), (asset(2), false)] {
            let mut key =
                norito::codec::encode_adaptive(&AssetId::new(definition, account.clone()));
            assert_eq!(
                admits_certified_value(&balances, "world.assets", &key, None),
                allowed
            );
            assert!(!admits_certified_value(
                &balances,
                "world.accounts",
                &key,
                None
            ));
            key.push(0);
            assert!(!admits_certified_value(
                &balances,
                "world.assets",
                &key,
                None
            ));
        }
    }
    let prefix = scoped(StateReadScope::ContractStatePrefix(
        "sc/native/escrow/".parse().unwrap(),
    ));
    for (path, allowed) in [
        ("sc/native/escrow/id", true),
        ("sc/native/escrow2/id", false),
        ("sc/native/escrow/", true),
        ("sc/native/escrow", false),
    ] {
        let key = norito::codec::encode_adaptive(&path.parse::<StatePath>().unwrap());
        assert_eq!(
            admits_certified_value(&prefix, "world.smart_contract_state", &key, None),
            allowed
        );
    }
    let grant = scoped(StateReadScope::MultisigRecords(ALICE_ID.clone()));
    let proposal: MultisigProposalId = "ab".repeat(32).parse().unwrap();
    for make in [
        multisig_approval_outcome_state_key,
        multisig_proposal_terminal_execution_state_key,
    ] {
        for hash in [[13; 32], [21; 32]] {
            for (account, allowed) in [(&*ALICE_ID, true), (&*BOB_ID, false)] {
                let path = make(hash, account, &proposal);
                let key = norito::codec::encode_adaptive(&path);
                assert_eq!(
                    admits_certified_value(&grant, "world.smart_contract_state", &key, None),
                    allowed
                );
                for invalid in [
                    format!("{path}/more"),
                    path.to_string().replacen("multisig/", "contract/", 1),
                ] {
                    let key =
                        norito::codec::encode_adaptive(&invalid.parse::<StatePath>().unwrap());
                    assert!(!admits_certified_value(
                        &grant,
                        "world.smart_contract_state",
                        &key,
                        None
                    ));
                }
            }
        }
    }
}

#[test]
fn opaque_keys_require_a_present_matching_certified_record_and_owner() {
    use iroha_data_model::escrow::{AssetEscrowKind, AssetEscrowStatus};
    use iroha_primitives::numeric::Quantity;
    let mut world = fixture();
    let escrow = AssetEscrowRecord {
        id: EscrowId::new(Hash::new(b"escrow")),
        seller: ALICE_ID.clone(),
        buyer: None,
        asset_definition: asset(1),
        amount: Quantity::one(),
        custody: BOB_ID.clone(),
        status: AssetEscrowStatus::Locked,
        kind: AssetEscrowKind::Lock,
        remaining_amount: Quantity::one(),
        release_authority: None,
        expires_at_ms: None,
        evidence_hashes: vec![],
        conditions: vec![],
        created_at_ms: 1,
        accepted_at_ms: None,
        payment_sent_at_ms: None,
        disputed_at_ms: None,
        closed_at_ms: None,
        resolution: None,
    };
    let claim = IdentifierClaimRecord {
        policy_id: "phone#retail".parse().unwrap(),
        opaque_id: OpaqueAccountId::from(Hash::new(b"opaque")),
        receipt_hash: Hash::new(b"receipt"),
        phone_retail_nullifier: None,
        uaid: UniversalAccountId::from_hash(Hash::new(b"uaid")),
        account_id: BOB_ID.clone(),
        verified_at_ms: 1,
        expires_at_ms: None,
    };
    let other_claim = IdentifierClaimRecord {
        policy_id: "phone#other".parse().unwrap(),
        ..claim.clone()
    };
    let other_escrow = AssetEscrowRecord {
        asset_definition: asset(2),
        ..escrow.clone()
    };
    world.asset_escrows.insert(escrow.id, escrow.clone());
    world
        .identifier_claims
        .insert(claim.opaque_id, claim.clone());
    for (table, key, value, other, scope) in [
        (
            "world.asset_escrows",
            norito::codec::encode_adaptive(&escrow.id),
            norito::codec::encode_adaptive(&escrow),
            norito::codec::encode_adaptive(&other_escrow),
            StateReadScope::AssetEscrows(asset(1)),
        ),
        (
            "world.identifier_claims",
            norito::codec::encode_adaptive(&claim.opaque_id),
            norito::codec::encode_adaptive(&claim),
            norito::codec::encode_adaptive(&other_claim),
            StateReadScope::IdentifierClaims(claim.policy_id.clone()),
        ),
    ] {
        let grant = scoped(scope);
        assert!(
            RowFamily::current(&world.view(), table, &key)
                .unwrap()
                .covered_by(&grant.scope)
        );
        assert!(admits_certified_value(&grant, table, &key, Some(&value)));
        assert!(!admits_certified_value(&grant, table, &key, None));
        assert!(!admits_certified_value(&grant, table, &key, Some(&other)));
        assert!(!admits_certified_value(
            &grant,
            table,
            b"different-key",
            Some(&value)
        ));
    }
}
