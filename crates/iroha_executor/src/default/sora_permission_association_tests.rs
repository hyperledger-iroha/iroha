// Sora permission association fixtures and exact deployment regressions.
// Included within default::trigger::tests so the private trigger helper is tested directly.
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
    CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};

fn sora_permissions() -> Vec<AnyPermission> {
    let mut permissions = vec![
        AnyPermission::CanBindSorafsAlias(CanBindSorafsAlias),
        AnyPermission::CanDeclareSorafsCapacity(CanDeclareSorafsCapacity),
        AnyPermission::CanSubmitSorafsTelemetry(CanSubmitSorafsTelemetry),
        AnyPermission::CanFileSorafsCapacityDispute(CanFileSorafsCapacityDispute),
        AnyPermission::CanIssueSorafsReplicationOrder(CanIssueSorafsReplicationOrder),
        AnyPermission::CanCompleteSorafsReplicationOrder(CanCompleteSorafsReplicationOrder),
        AnyPermission::CanManageSorafsModeration(CanManageSorafsModeration),
        AnyPermission::CanManageSorafsPopRegistry(CanManageSorafsPopRegistry),
        AnyPermission::CanOperateSorafsPopIssuer(CanOperateSorafsPopIssuer),
        AnyPermission::CanSetSorafsPricing(CanSetSorafsPricing),
        AnyPermission::CanSetSorafsReservePolicy(CanSetSorafsReservePolicy),
        AnyPermission::CanUpsertSorafsProviderCredit(CanUpsertSorafsProviderCredit),
        AnyPermission::CanManageSoranetVpnQuoteIssuers(CanManageSoranetVpnQuoteIssuers),
        AnyPermission::CanIssueSoranetVpnQuote(CanIssueSoranetVpnQuote),
        AnyPermission::CanIngestSoranetPrivacy(CanIngestSoranetPrivacy),
        AnyPermission::CanManageSccpGovernance(CanManageSccpGovernance),
    ];
    permissions.extend([
        AnyPermission::CanManageSorafsFinalPromotionCustody(CanManageSorafsFinalPromotionCustody {
            deployment_id: "promotion".to_owned(),
        }),
        AnyPermission::CanOperateSorafsFinalPromotion(CanOperateSorafsFinalPromotion {
            deployment_id: "promotion".to_owned(),
        }),
        AnyPermission::CanCheckSorafsFinalPromotion(CanCheckSorafsFinalPromotion {
            deployment_id: "promotion".to_owned(),
        }),
        AnyPermission::CanManageSorafsFinalPromotionAccountCustody(
            CanManageSorafsFinalPromotionAccountCustody {
                deployment_id: "promotion".into(),
            },
        ),
        AnyPermission::CanCheckSorafsFinalPromotionAccountCustody(
            CanCheckSorafsFinalPromotionAccountCustody {
                deployment_id: "promotion".into(),
            },
        ),
    ]);
    permissions
}

#[test]
fn final_promotion_permissions_have_no_incidental_resource_associations() {
    use iroha_executor_data_model::permission::{
        account::CanUnregisterAccount, asset_definition::CanUnregisterAssetDefinition,
        domain::CanUnregisterDomain, trigger::CanExecuteTrigger,
    };
    let domain_id = DomainId::try_new("promotion", "universal").expect("candidate domain");
    let account_id = sample_account_id(0x31, &domain_id);
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "promotion".parse().expect("candidate definition name"),
    );
    let owned_definitions = [asset_definition_id.clone()];
    let trigger_id = TriggerId::from_str("promotion").expect("candidate trigger");

    // Exact resource tokens must be associated with these very same candidates. These positive
    // controls prevent an invalid fixture or an always-false helper from passing the regression.
    assert!(domain::is_permission_domain_associated(
        &CanUnregisterDomain {
            domain: domain_id.clone()
        }
        .into(),
        &domain_id,
        &owned_definitions,
    ));
    assert!(account::is_permission_account_associated(
        &CanUnregisterAccount {
            account: account_id.clone()
        }
        .into(),
        &account_id,
    ));
    assert!(asset_definition::is_permission_asset_definition_associated(
        &CanUnregisterAssetDefinition {
            asset_definition: asset_definition_id.clone()
        }
        .into(),
        &asset_definition_id,
    ));
    assert!(is_permission_trigger_associated(
        &CanExecuteTrigger {
            trigger: trigger_id.clone()
        }
        .into(),
        &trigger_id,
    ));

    let mut checked = 0;
    for token in sora_permissions() {
        match &token {
            AnyPermission::CanManageSorafsFinalPromotionCustody(permission) => {
                assert_eq!(permission.deployment_id, "promotion");
            }
            AnyPermission::CanOperateSorafsFinalPromotion(permission) => {
                assert_eq!(permission.deployment_id, "promotion");
            }
            AnyPermission::CanCheckSorafsFinalPromotion(permission) => {
                assert_eq!(permission.deployment_id, "promotion");
            }
            AnyPermission::CanManageSorafsFinalPromotionAccountCustody(permission) => {
                assert_eq!(permission.deployment_id, "promotion");
            }
            AnyPermission::CanCheckSorafsFinalPromotionAccountCustody(permission) => {
                assert_eq!(permission.deployment_id, "promotion");
            }
            _ => continue,
        }
        let permission = Permission::from(token);
        assert!(
            AnyPermission::try_from(&permission).is_ok(),
            "valid canonical deployment token"
        );
        assert!(!domain::is_permission_domain_associated(
            &permission,
            &domain_id,
            &owned_definitions
        ));
        assert!(!account::is_permission_account_associated(
            &permission,
            &account_id
        ));
        assert!(
            !asset_definition::is_permission_asset_definition_associated(
                &permission,
                &asset_definition_id
            )
        );
        assert!(!is_permission_trigger_associated(&permission, &trigger_id));
        checked += 1;
    }
    assert_eq!(
        checked, 5,
        "receipt and account-custody capabilities must be exercised"
    );
}

#[test]
fn sora_permissions_not_trigger_associated() {
    let trigger_id = TriggerId::from_str("metadata_cleanup").expect("trigger id must be valid");
    for permission in sora_permissions() {
        let permission = Permission::from(permission);
        assert!(
            !is_permission_trigger_associated(&permission, &trigger_id),
            "Sora-specific permissions must not bind to triggers"
        );
    }
}
#[test]
fn sora_permissions_not_domain_account_or_definition_associated() {
    let domain_id = DomainId::try_new("test", "universal").expect("domain id must be valid");
    let account_id = sample_account_id(0x12, &domain_id);
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("test", "universal").unwrap(),
        "token".parse().unwrap(),
    );
    for permission in sora_permissions() {
        let permission = Permission::from(permission);
        assert!(
            !domain::is_permission_domain_associated(&permission, &domain_id, &[]),
            "Sora-specific permissions must not bind to domains"
        );
        assert!(
            !account::is_permission_account_associated(&permission, &account_id),
            "Sora-specific permissions must not bind to accounts"
        );
        assert!(
            !asset_definition::is_permission_asset_definition_associated(
                &permission,
                &asset_definition_id
            ),
            "Sora-specific permissions must not bind to asset definitions"
        );
    }
}
