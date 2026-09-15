//! Exact privileged permission ownership, delegation and management policy regressions.
use super::{
    AnyPermission, OnlyGenesis, PassCondition, ValidateGrantRevoke, has_permission_in_roles,
    permission_owned_in_sources,
};
use crate::permission::test_override;
use crate::{
    data_model::{Registrable as _, ValidationFail},
    prelude::Context,
    smart_contract::{
        Iroha,
        data_model::{
            block::BlockHeader,
            nexus::{FeeSponsorProgramId, UniversalAccountId},
            permission::Permission as PermissionObject,
            prelude::{
                AccountId, AssetDefinitionId, AssetId, Json, ResolvedAssetDefinitionAliasV1, RoleId,
            },
            smart_contract::ContractAddress,
        },
    },
};
use iroha_crypto::{Hash, PublicKey};
use iroha_executor_data_model::permission::kagemusha::CanManageKagemushaReserve;
use iroha_executor_data_model::permission::{
    account::{
        AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanResolveAccountAlias,
    },
    asset::{CanMintAssetToAccount, CanMintAssetWithDefinition},
    asset_definition::{
        AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
        CanManageAssetDefinitionConfidentialPolicy,
    },
    domain::CanRegisterDomain,
    governance::{CanManageConfidentialParams, CanManageConsensusKeys, CanManageRuntimeUpgrades},
    nexus::{
        CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram, CanPublishSpaceDirectoryManifest,
        CanPublishSpaceDirectoryManifestForAccountDomain, CanPublishSpaceDirectoryManifestForUaid,
    },
    parameter::CanSetHijiriParameters,
    peer::CanManagePeers,
    query::{CanReadAccountData, CanReadAllLedgerData, CanReadRestrictedDataspace},
    sccp::CanProposeSccpRouteGovernance,
    settlement::{CanExecuteSettlement, CanSetFxCorridorPolicy},
    smart_contract::CanInvokeContractEntrypoint,
    soranet::{CanIssueSoranetVpnQuote, CanManageSoranetVpnQuoteIssuers},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::topology::DataSpaceId;
use std::{num::NonZeroU64, vec::Vec};
fn make_context(authority: &AccountId, height: u64) -> Context {
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("height must be non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    Context {
        authority: authority.clone(),
        curr_block: header,
    }
}
fn make_account_id() -> AccountId {
    let public_key: PublicKey =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .unwrap();
    AccountId::new(public_key)
}
fn make_other_account_id() -> AccountId {
    let public_key: PublicKey =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
            .parse()
            .unwrap();
    AccountId::new(public_key)
}
#[test]
fn code_management_grants_are_separate_and_genesis_rooted() {
    use iroha_executor_data_model::permission::smart_contract::{
        CanGrantSmartContractCodeManagement, CanManageSmartContractCode,
    };
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let code_manager = PermissionObject::from(CanManageSmartContractCode);
    let manager = PermissionObject::from(CanGrantSmartContractCodeManagement);
    assert_eq!(code_manager.name(), "CanManageSmartContractCode");
    assert_eq!(manager.name(), "CanGrantSmartContractCodeManagement");
    for token in [&code_manager, &manager] {
        let encoded = norito::json::to_vec(token).expect("canonical permission JSON");
        let decoded: PermissionObject =
            norito::json::from_slice(&encoded).expect("permission JSON roundtrip");
        assert_eq!(&decoded, token);
    }
    let management_dispatch = AnyPermission::try_from(&code_manager).expect("code-management type");
    let manager_dispatch = AnyPermission::try_from(&manager).expect("manager type");
    for (held, expected) in [(code_manager.clone(), false), (manager.clone(), true)] {
        let old = test_override::replace_permissions(vec![held]);
        let grant = management_dispatch.validate_grant(&authority, &context, &Iroha);
        let revoke = management_dispatch.validate_revoke(&authority, &context, &Iroha);
        let grant_manager = manager_dispatch.validate_grant(&authority, &context, &Iroha);
        let revoke_manager = manager_dispatch.validate_revoke(&authority, &context, &Iroha);
        test_override::replace_permissions(old);
        assert_eq!(grant.is_ok(), expected);
        assert_eq!(revoke.is_ok(), expected);
        assert!(grant_manager.is_err());
        assert!(revoke_manager.is_err());
    }
    let genesis = make_context(&authority, 1);
    assert!(
        CanGrantSmartContractCodeManagement
            .validate_grant(&authority, &genesis, &Iroha)
            .is_ok()
    );
    assert!(
        CanManageSmartContractCode
            .validate_grant(&authority, &genesis, &Iroha)
            .is_ok()
    );
    for raw in [code_manager, manager] {
        let encoded = norito::json::to_json(&raw).expect("permission JSON");
        let decoded: PermissionObject =
            norito::json::from_str(&encoded).expect("permission roundtrip");
        assert_eq!(decoded, raw);
        let malformed = PermissionObject::new(raw.name().to_owned(), Json::new(true));
        assert!(AnyPermission::try_from(&malformed).is_err());
    }
}
#[test]
fn code_management_grant_role_requires_exact_assigned_membership() {
    use iroha_executor_data_model::permission::smart_contract::CanGrantSmartContractCodeManagement;
    let manager_role: RoleId = "contract_code_management_granters".parse().expect("role");
    let other_role: RoleId = "builders".parse().expect("role");
    let roles = vec![(
        manager_role.clone(),
        PermissionObject::from(CanGrantSmartContractCodeManagement),
    )];
    assert!(permission_owned_in_sources(
        &[],
        &roles,
        &[manager_role],
        &CanGrantSmartContractCodeManagement
    ));
    assert!(!permission_owned_in_sources(
        &[],
        &roles,
        &[other_role],
        &CanGrantSmartContractCodeManagement
    ));
}
#[test]
fn operational_governance_permissions_require_canonical_unit_payloads() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let permissions = [
        PermissionObject::from(CanManageRuntimeUpgrades),
        PermissionObject::from(CanManageConsensusKeys),
        PermissionObject::from(CanManageConfidentialParams),
        PermissionObject::from(CanSetHijiriParameters),
    ];

    for raw in permissions {
        let name = raw.name().to_owned();
        let dispatched =
            AnyPermission::try_from(&raw).expect("canonical unit permission must be typed");
        let previous = test_override::replace_permissions(vec![raw]);
        assert!(
            dispatched
                .validate_grant(&authority, &context, &Iroha)
                .is_ok(),
            "exact holder could not grant {name}",
        );
        assert!(
            dispatched
                .validate_revoke(&authority, &context, &Iroha)
                .is_ok(),
            "exact holder could not revoke {name}",
        );
        test_override::replace_permissions(previous);

        let malformed = PermissionObject::new(
            name.parse().expect("permission ident"),
            Json::from_raw_json("{\"invented_scope\":true}".to_owned())
                .expect("valid JSON fixture"),
        );
        assert!(
            AnyPermission::try_from(&malformed).is_err(),
            "same-name non-unit {name} payload must fail typed dispatch",
        );
    }
}
fn make_third_account_id() -> AccountId {
    let public_key: PublicKey =
        "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
            .parse()
            .unwrap();
    AccountId::new(public_key)
}
fn make_fee_sponsor_program_id(sponsor: AccountId, name: &str) -> FeeSponsorProgramId {
    FeeSponsorProgramId::new(
        sponsor,
        name.parse()
            .expect("fee sponsor program name must be valid"),
    )
}
#[test]
fn has_permission_in_roles_filters_by_role_ids() {
    let role_id: RoleId = "role1".parse().unwrap();
    let other_role_id: RoleId = "role2".parse().unwrap();
    let permission = PermissionObject::from(CanManagePeers);
    let roles = vec![
        (role_id.clone(), permission.clone()),
        (other_role_id, PermissionObject::from(CanManagePeers)),
    ];
    let role_ids = vec![role_id];
    assert!(has_permission_in_roles(roles, &role_ids, &CanManagePeers));
}
#[test]
fn has_permission_in_roles_returns_false_when_role_ids_empty() {
    let role_id: RoleId = "role1".parse().unwrap();
    let roles = vec![(role_id, PermissionObject::from(CanManagePeers))];
    let role_ids: Vec<RoleId> = Vec::new();
    assert!(!has_permission_in_roles(roles, &role_ids, &CanManagePeers));
}
#[test]
fn has_permission_in_roles_deduplicates_role_ids() {
    let role_id: RoleId = "role1".parse().unwrap();
    let roles = vec![(role_id.clone(), PermissionObject::from(CanManagePeers))];
    let role_ids = vec![role_id.clone(), role_id];
    assert!(has_permission_in_roles(roles, &role_ids, &CanManagePeers));
}
#[test]
fn permission_owned_returns_true_for_direct_permission() {
    let account_permissions = vec![PermissionObject::from(CanManagePeers)];
    let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
    let role_ids: Vec<RoleId> = Vec::new();
    assert!(permission_owned_in_sources(
        &account_permissions,
        &role_permissions,
        &role_ids,
        &CanManagePeers,
    ));
}
#[test]
fn permission_owned_returns_true_via_roles() {
    let role_id: RoleId = "validators".parse().unwrap();
    let account_permissions = Vec::new();
    let role_permissions = vec![(role_id.clone(), PermissionObject::from(CanManagePeers))];
    let role_ids = vec![role_id];
    assert!(permission_owned_in_sources(
        &account_permissions,
        &role_permissions,
        &role_ids,
        &CanManagePeers,
    ));
}
#[test]
fn permission_owned_returns_false_when_missing() {
    let account_permissions = vec![PermissionObject::from(CanManagePeers)];
    let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
    let role_ids: Vec<RoleId> = Vec::new();
    assert!(!permission_owned_in_sources(
        &account_permissions,
        &role_permissions,
        &role_ids,
        &CanRegisterDomain,
    ));
}
#[test]
fn permission_owned_matches_opaque_asset_definition_permission() {
    let asset_definition = AssetDefinitionId::from_uuid_bytes([
        0x68, 0x72, 0x45, 0x4e, 0x9c, 0x04, 0x46, 0x41, 0xaa, 0x58, 0x1e, 0xc5, 0xf3, 0x80, 0x16,
        0x19,
    ])
    .expect("opaque asset definition parses");
    let token = CanMintAssetWithDefinition {
        asset_definition: asset_definition.clone(),
    };
    let account_permissions = vec![PermissionObject::from(token.clone())];
    let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
    let role_ids: Vec<RoleId> = Vec::new();
    assert!(permission_owned_in_sources(
        &account_permissions,
        &role_permissions,
        &role_ids,
        &token,
    ));
}
#[test]
fn confidential_policy_permission_holder_can_delegate_only_the_exact_asset_definition() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let target = AssetDefinitionId::derive_from_components(
        DomainId::try_new("currency", "paynet").expect("asset domain"),
        "pkr".parse().expect("asset name"),
    );
    let other = AssetDefinitionId::derive_from_components(
        DomainId::try_new("currency", "paynet").expect("asset domain"),
        "usd".parse().expect("asset name"),
    );
    let exact = CanManageAssetDefinitionConfidentialPolicy {
        asset_definition: target,
    };
    let sibling = CanManageAssetDefinitionConfidentialPolicy {
        asset_definition: other.clone(),
    };
    let held: PermissionObject = exact.clone().into();
    let exact_dispatched =
        AnyPermission::try_from(&held).expect("confidential-policy permission must be typed");
    let sibling_dispatched = AnyPermission::try_from(&PermissionObject::from(sibling))
        .expect("sibling confidential-policy permission must be typed");
    let previous = test_override::replace_permissions(vec![held]);
    let exact_grant = exact_dispatched.validate_grant(&authority, &context, &Iroha);
    let exact_revoke = exact_dispatched.validate_revoke(&authority, &context, &Iroha);
    let sibling_definition = crate::data_model::asset::AssetDefinition::numeric(
        other,
        "USD",
        crate::data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&make_other_account_id());
    let sibling_grant = crate::tests::with_mock_asset_definitions(vec![sibling_definition], || {
        sibling_dispatched.validate_grant(&authority, &context, &Iroha)
    });
    test_override::replace_permissions(previous);
    assert!(exact_grant.is_ok());
    assert!(exact_revoke.is_ok());
    assert!(matches!(
        sibling_grant,
        Err(ValidationFail::NotPermitted(_))
    ));
}
#[test]
fn only_genesis_allows_first_block() {
    let authority = make_account_id();
    let context = make_context(&authority, 1);
    assert!(OnlyGenesis.validate(&authority, &Iroha, &context).is_ok());
}
#[test]
fn only_genesis_rejects_other_blocks() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let err = OnlyGenesis
        .validate(&authority, &Iroha, &context)
        .expect_err("expected rejection");
    assert!(matches!(err, ValidationFail::NotPermitted(_)));
}
#[test]
fn bilateral_settlement_consent_is_controlled_by_debited_account() {
    let debited_account = make_account_id();
    let other = make_other_account_id();
    let permission = CanExecuteSettlement {
        debited_asset: AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("fixture", "universal").expect("asset domain"),
                "rose".parse().expect("asset name"),
            ),
            debited_account.clone(),
        ),
        settlement_id: "permission_consent".parse().expect("settlement id"),
        intent_hash: Hash::new(b"exact bilateral settlement intent"),
    };
    let debited_context = make_context(&debited_account, 2);
    let other_context = make_context(&other, 2);
    permission
        .validate_grant(&debited_account, &debited_context, &Iroha)
        .expect("debited account may grant exact consent");
    permission
        .validate_revoke(&debited_account, &debited_context, &Iroha)
        .expect("debited account may revoke exact consent");
    assert!(matches!(
        permission
            .validate_grant(&other, &other_context, &Iroha)
            .expect_err("unrelated authority must not grant consent"),
        ValidationFail::NotPermitted(_)
    ));
}
#[test]
fn exact_leaf_holders_cannot_bypass_dedicated_delegation_roots() {
    let holder = make_account_id();
    let root = make_other_account_id();
    let context = make_context(&holder, 2);
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fixture", "universal").expect("asset domain"),
        "rose".parse().expect("asset name"),
    );
    let permissions = vec![
        AnyPermission::CanResolveAccountAlias(CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10)),
        }),
        AnyPermission::CanExecuteSettlement(CanExecuteSettlement {
            debited_asset: AssetId::new(asset_definition, root.clone()),
            settlement_id: "holder_bypass".parse().expect("settlement id"),
            intent_hash: Hash::new(b"holder-only settlement consent"),
        }),
        AnyPermission::CanSetFxCorridorPolicy(CanSetFxCorridorPolicy {
            policy_id: "holder_only_policy".parse().expect("policy id"),
        }),
        AnyPermission::CanManageFeeSponsorProgram(CanManageFeeSponsorProgram { sponsor: root }),
    ];
    for permission in permissions {
        let name = PermissionObject::from(permission.clone()).name().to_owned();
        assert!(
            !permission.is_holder_delegable(),
            "{name} must retain its dedicated lifecycle root"
        );
        let previous = test_override::replace_permissions(vec![permission.clone().into()]);
        let grant = permission.validate_grant(&holder, &context, &Iroha);
        let revoke = permission.validate_revoke(&holder, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(
            matches!(grant, Err(ValidationFail::NotPermitted(_))),
            "an exact {name} holder unexpectedly delegated it: {grant:?}"
        );
        assert!(
            matches!(revoke, Err(ValidationFail::NotPermitted(_))),
            "an exact {name} holder unexpectedly revoked it: {revoke:?}"
        );
    }
}
#[test]
fn vpn_quote_issuer_leaf_requires_manager_delegation() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let leaf = AnyPermission::CanIssueSoranetVpnQuote(CanIssueSoranetVpnQuote);
    let previous = test_override::replace_permissions(Vec::new());
    assert!(matches!(
        leaf.validate_grant(&authority, &context, &Iroha)
            .expect_err("an unrelated account must not appoint a VPN quote issuer"),
        ValidationFail::NotPermitted(_)
    ));
    test_override::replace_permissions(vec![CanIssueSoranetVpnQuote.into()]);
    assert!(matches!(
        leaf.validate_grant(&authority, &context, &Iroha)
            .expect_err("an issuer leaf must not propagate itself"),
        ValidationFail::NotPermitted(_)
    ));
    test_override::replace_permissions(vec![CanManageSoranetVpnQuoteIssuers.into()]);
    leaf.validate_grant(&authority, &context, &Iroha)
        .expect("the issuer manager may grant the leaf");
    leaf.validate_revoke(&authority, &context, &Iroha)
        .expect("the issuer manager may revoke the leaf");
    test_override::replace_permissions(previous);
}
#[test]
fn governed_kagemusha_permissions_are_immutable_after_genesis() {
    let banking_authority = make_account_id();
    let context = make_context(&banking_authority, 2);
    let results = [(
        "CanManageKagemushaReserve",
        CanManageKagemushaReserve.validate_grant(&banking_authority, &context, &Iroha),
        CanManageKagemushaReserve.validate_revoke(&banking_authority, &context, &Iroha),
    )];
    for (name, grant, revoke) in results {
        for result in [grant, revoke] {
            let error = result
                .expect_err("a genesis-seeded offline permission must not be mutated post-genesis");
            assert!(matches!(error, ValidationFail::NotPermitted(_)));
            assert!(
                error
                    .to_string()
                    .contains("only allowed inside the genesis block"),
                "unexpected {name} mutation rejection: {error}",
            );
        }
    }
}
#[test]
fn governed_kagemusha_permissions_can_only_be_seeded_in_genesis() {
    let genesis_authority = make_account_id();
    let context = make_context(&genesis_authority, 1);
    let results = [(
        "CanManageKagemushaReserve",
        CanManageKagemushaReserve.validate_grant(&genesis_authority, &context, &Iroha),
        CanManageKagemushaReserve.validate_revoke(&genesis_authority, &context, &Iroha),
    )];
    for (name, grant, revoke) in results {
        assert!(grant.is_ok(), "genesis must grant {name}: {grant:?}");
        assert!(revoke.is_ok(), "genesis must revoke {name}: {revoke:?}");
    }
}
#[test]
fn fee_sponsor_program_manager_is_typed_and_only_the_sponsor_may_delegate_it() {
    let sponsor = make_account_id();
    let outsider = make_other_account_id();
    let context = make_context(&sponsor, 2);
    let permission = CanManageFeeSponsorProgram {
        sponsor: sponsor.clone(),
    };
    let raw: PermissionObject = permission.clone().into();
    assert!(matches!(
        AnyPermission::try_from(&raw),
        Ok(AnyPermission::CanManageFeeSponsorProgram(parsed)) if parsed == permission
    ));
    assert!(
        permission
            .validate_grant(&sponsor, &context, &Iroha)
            .is_ok()
    );
    assert!(
        permission
            .validate_revoke(&sponsor, &context, &Iroha)
            .is_ok()
    );
    assert!(matches!(
        permission.validate_grant(&outsider, &context, &Iroha),
        Err(ValidationFail::NotPermitted(_))
    ));
    assert!(matches!(
        permission.validate_revoke(&outsider, &context, &Iroha),
        Err(ValidationFail::NotPermitted(_))
    ));
}
#[test]
fn alias_resolution_delegate_can_propagate_only_the_exact_scope() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let delegated_scope = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10));
    let other_scope = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(12));
    let held = CanDelegateAccountAliasResolution {
        scope: delegated_scope.clone(),
    };
    let exact = CanResolveAccountAlias {
        scope: delegated_scope,
    };
    let other = CanResolveAccountAlias { scope: other_scope };
    let held_object = PermissionObject::from(held.clone());
    let held_dispatched =
        AnyPermission::try_from(&held_object).expect("delegation token must be typed");
    let previous = test_override::replace_permissions(vec![held_object]);
    let exact_grant = exact.validate_grant(&authority, &context, &Iroha);
    let exact_revoke = exact.validate_revoke(&authority, &context, &Iroha);
    let cross_scope_grant = other.validate_grant(&authority, &context, &Iroha);
    let recursive_grant = held_dispatched.validate_grant(&authority, &context, &Iroha);
    let recursive_revoke = held_dispatched.validate_revoke(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(exact_grant.is_ok());
    assert!(exact_revoke.is_ok());
    assert!(matches!(
        cross_scope_grant,
        Err(ValidationFail::NotPermitted(_))
    ));
    assert!(recursive_grant.is_ok());
    assert!(recursive_revoke.is_ok());
}
#[test]
fn exact_asset_alias_holder_cannot_revoke_after_binding_clear_without_namespace_root() {
    let holder = make_account_id();
    let context = make_context(&holder, 2);
    let target = ResolvedAssetDefinitionAliasV1::new(
        "usd#banka.paynet".parse().expect("asset alias"),
        DataSpaceId::new(7),
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("banka", "paynet").expect("alias domain"),
            "usd".parse().expect("asset name"),
        ),
    );
    let exact = CanManageAssetDefinitionAlias {
        scope: AssetDefinitionAliasPermissionScope::Alias(target),
    };
    let exact_raw = PermissionObject::from(exact.clone());
    let dispatched =
        AnyPermission::try_from(&exact_raw).expect("exact alias permission must be typed");
    // The binding is intentionally absent. Exact possession must not bypass the native
    // namespace-root lookup; the definition pin prevents rebinding escalation, while the
    // namespace root remains the lifecycle authority after clear.
    let previous = test_override::replace_permissions(vec![exact_raw]);
    let holder_revoke = dispatched.validate_revoke(&holder, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(holder_revoke.is_err());
    assert!(matches!(
        super::asset_definition::asset_definition_alias_namespace_scope(match &exact.scope {
            AssetDefinitionAliasPermissionScope::Alias(alias) => alias,
            _ => unreachable!("test constructs an exact alias"),
        }).expect("valid exact alias namespace"),
        AssetDefinitionAliasPermissionScope::Domain(domain)
            if domain == DomainId::try_new("banka", "paynet").expect("alias domain")
    ));
}
#[test]
fn exact_holder_dispatch_covers_each_corrected_delegation_family() {
    let authority = make_account_id();
    let adjacent_owner = make_other_account_id();
    let context = make_context(&authority, 2);
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("grant_policy", "universal").expect("asset domain"),
        "root_asset".parse().expect("asset name"),
    );
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &adjacent_owner,
        77,
        DataSpaceId::UNIVERSAL,
    )
    .expect("contract address");
    let dataspace = DataSpaceId::new(7);
    let permissions = vec![
        PermissionObject::from(CanMintAssetToAccount {
            // Possessing this exact token authorizes propagation even though the authority is
            // neither the destination account nor queried as the definition owner.
            asset_definition,
            account: adjacent_owner,
        }),
        PermissionObject::from(CanInvokeContractEntrypoint {
            contract,
            entrypoint: "main".to_owned(),
        }),
        PermissionObject::from(CanDelegateAccountAliasResolution {
            scope: AccountAliasPermissionScope::Dataspace(dataspace),
        }),
        PermissionObject::from(CanPublishSpaceDirectoryManifestForUaid {
            dataspace,
            uaid: UniversalAccountId::from_hash(Hash::new(b"grant-policy-uaid")),
        }),
        PermissionObject::from(CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace,
            domain: DomainId::try_new("retail", "universal").expect("account domain"),
        }),
        PermissionObject::from(CanEnrollFeeSponsorProgram {
            program_id: make_fee_sponsor_program_id(authority.clone(), "retail"),
        }),
        PermissionObject::from(CanProposeSccpRouteGovernance),
    ];
    for raw in permissions {
        let name = raw.name().to_owned();
        let dispatched = AnyPermission::try_from(&raw).expect("corrected permission must be typed");
        let previous = test_override::replace_permissions(vec![raw]);
        let grant = dispatched.validate_grant(&authority, &context, &Iroha);
        let revoke = dispatched.validate_revoke(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(
            grant.is_ok(),
            "exact holder could not grant {name}: {grant:?}"
        );
        assert!(
            revoke.is_ok(),
            "exact holder could not revoke {name}: {revoke:?}",
        );
    }
}
#[test]
fn exact_contract_holder_cannot_propagate_noncanonical_selector() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let raw = PermissionObject::from(CanInvokeContractEntrypoint {
        contract: ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &make_other_account_id(),
            88,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address"),
        entrypoint: " main".to_owned(),
    });
    let dispatched =
        AnyPermission::try_from(&raw).expect("contract permission must be structurally typed");
    let previous = test_override::replace_permissions(vec![raw]);
    let grant = dispatched.validate_grant(&authority, &context, &Iroha);
    let revoke = dispatched.validate_revoke(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    for result in [grant, revoke] {
        assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
    }
}
#[test]
fn restricted_dataspace_reader_cannot_grant_or_revoke_after_genesis() {
    let authority = make_account_id();
    let post_genesis = make_context(&authority, 2);
    let exact = CanReadRestrictedDataspace {
        dataspace: DataSpaceId::new(10),
    };
    let permission = PermissionObject::from(exact);
    let role_dispatched =
        AnyPermission::try_from(&permission).expect("restricted-read permission must be typed");
    let previous = test_override::replace_permissions(vec![permission]);
    let denied = [
        exact.validate_grant(&authority, &post_genesis, &Iroha),
        exact.validate_revoke(&authority, &post_genesis, &Iroha),
        role_dispatched.validate_grant(&authority, &post_genesis, &Iroha),
        role_dispatched.validate_revoke(&authority, &post_genesis, &Iroha),
    ];
    test_override::replace_permissions(previous);
    for result in denied {
        let error =
            result.expect_err("a restricted reader must not mutate the exact token after genesis");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(
            error
                .to_string()
                .contains("only allowed inside the genesis block"),
            "unexpected restricted-read mutation rejection: {error}",
        );
    }
    let genesis = make_context(&authority, 1);
    assert!(exact.validate_grant(&authority, &genesis, &Iroha).is_ok());
    assert!(exact.validate_revoke(&authority, &genesis, &Iroha).is_ok());
}
#[test]
fn global_ledger_reader_cannot_grant_or_revoke_after_genesis() {
    let authority = make_account_id();
    let post_genesis = make_context(&authority, 2);
    let exact = CanReadAllLedgerData;
    let permission = PermissionObject::from(exact);
    let dispatched =
        AnyPermission::try_from(&permission).expect("global-read permission must be typed");
    let previous = test_override::replace_permissions(vec![permission]);
    let denied = [
        dispatched.validate_grant(&authority, &post_genesis, &Iroha),
        dispatched.validate_revoke(&authority, &post_genesis, &Iroha),
    ];
    test_override::replace_permissions(previous);
    for result in denied {
        let error = result.expect_err(
            "possession of the global read root must not permit post-genesis propagation",
        );
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
    }
    let genesis = make_context(&authority, 1);
    assert!(exact.validate_grant(&authority, &genesis, &Iroha).is_ok());
    assert!(exact.validate_revoke(&authority, &genesis, &Iroha).is_ok());
}
#[test]
fn account_subject_exclusively_controls_account_read_grants() {
    let account = make_account_id();
    let reader = make_other_account_id();
    let context = make_context(&account, 2);
    let exact = CanReadAccountData {
        account: account.clone(),
    };
    let permission = PermissionObject::from(exact.clone());
    let dispatched =
        AnyPermission::try_from(&permission).expect("account-read permission must be typed");
    assert!(
        dispatched
            .validate_grant(&account, &context, &Iroha)
            .is_ok(),
        "the account subject must control its read grant"
    );
    assert!(
        dispatched
            .validate_revoke(&account, &context, &Iroha)
            .is_ok(),
        "the account subject must control revocation"
    );
    let previous = test_override::replace_permissions(vec![permission]);
    let reader_context = make_context(&reader, 2);
    let denied = [
        dispatched.validate_grant(&reader, &reader_context, &Iroha),
        dispatched.validate_revoke(&reader, &reader_context, &Iroha),
    ];
    test_override::replace_permissions(previous);
    for result in denied {
        assert!(
            matches!(result, Err(ValidationFail::NotPermitted(_))),
            "an exact reader may use but must not propagate the account's grant"
        );
    }
}
#[test]
fn alias_resolution_domain_delegate_cannot_widen_or_cross_scope_kind() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let delegated_scope = AccountAliasPermissionScope::Domain(
        iroha_model_base::domain::DomainId::try_new("hbl", "sbp").expect("HBL SBP domain fixture"),
    );
    let sibling_scope = AccountAliasPermissionScope::Domain(
        iroha_model_base::domain::DomainId::try_new("ubl", "sbp").expect("UBL SBP domain fixture"),
    );
    let held = CanDelegateAccountAliasResolution {
        scope: delegated_scope.clone(),
    };
    let exact = CanResolveAccountAlias {
        scope: delegated_scope,
    };
    let sibling = CanResolveAccountAlias {
        scope: sibling_scope,
    };
    let dataspace = CanResolveAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10)),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(held)]);
    let exact_grant = exact.validate_grant(&authority, &context, &Iroha);
    let sibling_grant = sibling.validate_grant(&authority, &context, &Iroha);
    let dataspace_grant = dataspace.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(exact_grant.is_ok());
    assert!(matches!(
        sibling_grant,
        Err(ValidationFail::NotPermitted(_))
    ));
    assert!(matches!(
        dataspace_grant,
        Err(ValidationFail::NotPermitted(_))
    ));
}
#[test]
fn can_publish_space_directory_manifest_grant_allows_existing_holder_after_genesis() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let token = CanPublishSpaceDirectoryManifest {
        dataspace: DataSpaceId::new(10),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(token)]);
    let result = token.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(result.is_ok());
}
#[test]
fn can_publish_space_directory_manifest_grant_rejects_unscoped_null_payload_after_genesis() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let token = CanPublishSpaceDirectoryManifest {
        dataspace: DataSpaceId::new(10),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::new(
        "CanPublishSpaceDirectoryManifest"
            .parse()
            .expect("permission ident"),
        Json::from_raw_json("null".to_owned()).expect("valid JSON fixture"),
    )]);
    let result = token.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
}
#[test]
fn can_publish_space_directory_manifest_grant_rejects_missing_holder_after_genesis() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let token = CanPublishSpaceDirectoryManifest {
        dataspace: DataSpaceId::new(10),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(CanManagePeers)]);
    let err = token
        .validate_grant(&authority, &context, &Iroha)
        .expect_err("expected rejection");
    test_override::replace_permissions(previous);
    assert!(matches!(err, ValidationFail::NotPermitted(_)));
}
#[test]
fn dataspace_manifest_holder_can_delegate_one_exact_uaid_after_genesis() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let dataspace = DataSpaceId::new(10);
    let scoped = CanPublishSpaceDirectoryManifestForUaid {
        dataspace,
        uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::delegated-customer")),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(
        CanPublishSpaceDirectoryManifest { dataspace },
    )]);
    let result = scoped.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(result.is_ok());
}
#[test]
fn uaid_manifest_holder_cannot_delegate_a_different_uaid_after_genesis() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let dataspace = DataSpaceId::new(10);
    let held = CanPublishSpaceDirectoryManifestForUaid {
        dataspace,
        uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::hbl-customer")),
    };
    let requested = CanPublishSpaceDirectoryManifestForUaid {
        dataspace,
        uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::ubl-customer")),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(held)]);
    let result = requested.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
}
#[test]
fn account_domain_manifest_delegation_is_exact_across_hbl_and_ubl() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let dataspace = DataSpaceId::new(10);
    let hbl = CanPublishSpaceDirectoryManifestForAccountDomain {
        dataspace,
        domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
    };
    let ubl = CanPublishSpaceDirectoryManifestForAccountDomain {
        dataspace,
        domain: DomainId::try_new("ubl", "sbp").expect("UBL domain"),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(hbl.clone())]);
    let own_result = hbl.validate_grant(&authority, &context, &Iroha);
    let cross_fi_result = ubl.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(own_result.is_ok());
    assert!(matches!(
        cross_fi_result,
        Err(ValidationFail::NotPermitted(_))
    ));
}
#[test]
fn dataspace_manifest_holder_can_delegate_account_domain_scope() {
    let authority = make_account_id();
    let context = make_context(&authority, 2);
    let dataspace = DataSpaceId::new(10);
    let hbl = CanPublishSpaceDirectoryManifestForAccountDomain {
        dataspace,
        domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(
        CanPublishSpaceDirectoryManifest { dataspace },
    )]);
    let result = hbl.validate_grant(&authority, &context, &Iroha);
    test_override::replace_permissions(previous);
    assert!(result.is_ok());
}
#[test]
fn fee_program_manager_can_delegate_exact_enrollment_scope() {
    let sponsor = make_account_id();
    let manager = make_other_account_id();
    let context = make_context(&manager, 2);
    let program_id = make_fee_sponsor_program_id(sponsor.clone(), "retail");
    let enrollment = CanEnrollFeeSponsorProgram {
        program_id: program_id.clone(),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(
        CanManageFeeSponsorProgram { sponsor },
    )]);
    for result in [
        enrollment.validate_grant(&manager, &context, &Iroha),
        enrollment.validate_revoke(&manager, &context, &Iroha),
    ] {
        assert!(result.is_ok(), "program manager must delegate exact scopes");
    }
    test_override::replace_permissions(previous);
}
#[test]
fn fee_program_delegation_is_exact_to_the_program_sponsor() {
    let first_sponsor = make_account_id();
    let second_sponsor = make_third_account_id();
    let manager = make_other_account_id();
    let context = make_context(&manager, 2);
    let first = CanEnrollFeeSponsorProgram {
        program_id: make_fee_sponsor_program_id(first_sponsor.clone(), "retail"),
    };
    let second = CanEnrollFeeSponsorProgram {
        program_id: make_fee_sponsor_program_id(second_sponsor, "retail"),
    };
    let previous = test_override::replace_permissions(vec![PermissionObject::from(
        CanManageFeeSponsorProgram {
            sponsor: first_sponsor,
        },
    )]);
    assert!(first.validate_grant(&manager, &context, &Iroha).is_ok());
    assert!(matches!(
        second.validate_grant(&manager, &context, &Iroha),
        Err(ValidationFail::NotPermitted(_))
    ));
    test_override::replace_permissions(previous);
}
#[test]
fn exact_fee_program_enrollment_holder_can_propagate_exact_token() {
    let sponsor = make_account_id();
    let code_manager = make_other_account_id();
    let context = make_context(&code_manager, 2);
    let token = CanEnrollFeeSponsorProgram {
        program_id: make_fee_sponsor_program_id(sponsor, "retail"),
    };
    let raw = PermissionObject::from(token);
    let dispatched =
        AnyPermission::try_from(&raw).expect("fee-program enrollment token must be typed");
    let previous = test_override::replace_permissions(vec![raw]);
    assert!(
        dispatched
            .validate_grant(&code_manager, &context, &Iroha)
            .is_ok()
    );
    assert!(
        dispatched
            .validate_revoke(&code_manager, &context, &Iroha)
            .is_ok()
    );
    test_override::replace_permissions(previous);
}
#[test]
fn genesis_can_seed_fee_program_permissions() {
    let sponsor = make_account_id();
    let genesis_authority = make_other_account_id();
    let context = make_context(&genesis_authority, 1);
    let program_id = make_fee_sponsor_program_id(sponsor.clone(), "retail");
    let permissions = [
        AnyPermission::CanManageFeeSponsorProgram(CanManageFeeSponsorProgram { sponsor }),
        AnyPermission::CanEnrollFeeSponsorProgram(CanEnrollFeeSponsorProgram { program_id }),
    ];
    for permission in permissions {
        assert!(
            permission
                .validate_grant(&genesis_authority, &context, &Iroha)
                .is_ok()
        );
        assert!(
            permission
                .validate_revoke(&genesis_authority, &context, &Iroha)
                .is_ok()
        );
    }
}
#[test]
fn account_domain_manifest_permission_json_uses_dot_fqn() {
    let token = CanPublishSpaceDirectoryManifestForAccountDomain {
        dataspace: DataSpaceId::new(10),
        domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
    };
    let payload = norito::json::to_json(&token).expect("serialize publisher permission");
    assert_eq!(payload, r#"{"dataspace":10,"domain":"hbl.sbp"}"#);
    assert_eq!(
        norito::json::from_str::<CanPublishSpaceDirectoryManifestForAccountDomain>(&payload)
            .expect("deserialize publisher permission"),
        token,
    );
}
#[test]
fn sponsor_program_permissions_json_use_exact_program_id() {
    let sponsor = make_account_id();
    let token = CanEnrollFeeSponsorProgram {
        program_id: make_fee_sponsor_program_id(sponsor, "retail"),
    };
    let payload = norito::json::to_json(&token).expect("serialize enrollment permission");
    assert_eq!(
        payload,
        format!(
            r#"{{"program_id":{{"sponsor":"{}","name":"{}"}}}}"#,
            token.program_id.sponsor, token.program_id.name,
        ),
    );
    assert_eq!(
        norito::json::from_str::<CanEnrollFeeSponsorProgram>(&payload)
            .expect("deserialize enrollment permission"),
        token,
    );
}
