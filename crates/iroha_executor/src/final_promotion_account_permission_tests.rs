//! Account-custody capability separation, exact deployment scope and delegation.
use super::*;
use crate::tests::with_mock_permissions;
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionAccountCustody,
};
use std::num::NonZeroU64;

fn token(manage: bool, deployment: &str) -> PermissionObject {
    if manage {
        CanManageSorafsFinalPromotionAccountCustody {
            deployment_id: deployment.to_owned(),
        }
        .into()
    } else {
        CanCheckSorafsFinalPromotionAccountCustody {
            deployment_id: deployment.to_owned(),
        }
        .into()
    }
}

#[test]
fn final_promotion_account_permissions_roundtrip_preserves_action_and_deployment_scope() {
    for manage in [false, true] {
        let permission = token(manage, "production-primary");
        let any = AnyPermission::try_from(&permission).expect("known final-promotion permission");
        assert_eq!(
            matches!(
                &any,
                AnyPermission::CanManageSorafsFinalPromotionAccountCustody(_)
            ),
            manage
        );
        assert!(!any.is_genesis_only());
        assert!(any.is_holder_delegable());
        assert_eq!(PermissionObject::from(any), permission);
        assert_ne!(permission, token(manage, "production-secondary"));
        assert_ne!(permission, token(!manage, "production-primary"));
        for payload in [
            Json::new(()),
            Json::new(norito::json!({})),
            Json::new(norito::json!({"deployment_id": 1})),
            Json::new(norito::json!({"deploymentId": "production-primary"})),
            Json::new(
                norito::json!({"deployment_id": "production-primary", "provider_id": "other"}),
            ),
        ] {
            let malformed = PermissionObject::new(permission.name().to_owned(), payload);
            assert!(AnyPermission::try_from(&malformed).is_err());
        }
    }
}

#[test]
fn final_promotion_account_permissions_match_only_exact_direct_or_assigned_role_capabilities() {
    let role: RoleId = "promotion_operator".parse().expect("role id");
    let other: RoleId = "other_operator".parse().expect("role id");
    let authority = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse::<iroha_crypto::PublicKey>()
            .expect("fixture key"),
    );
    for manage in [false, true] {
        let exact = token(manage, "production-primary");
        let any = AnyPermission::try_from(&exact).expect("typed permission");
        for held in [
            exact.clone(),
            token(!manage, "production-primary"),
            token(manage, "production-secondary"),
            iroha_executor_data_model::permission::sorafs::CanManageSorafsFinalPromotionCustody {
                deployment_id: "production-primary".into(),
            }
            .into(),
            iroha_executor_data_model::permission::sorafs::CanOperateSorafsFinalPromotion {
                deployment_id: "production-primary".into(),
            }
            .into(),
            iroha_executor_data_model::permission::sorafs::CanCheckSorafsFinalPromotion {
                deployment_id: "production-primary".into(),
            }
            .into(),
        ] {
            let allowed = held == exact;
            with_mock_permissions(vec![held.clone()], || {
                assert_eq!(any.is_owned_by(&authority, &Iroha), allowed);
            });
            let roles = vec![(role.clone(), held)];
            match &any {
                AnyPermission::CanManageSorafsFinalPromotionAccountCustody(permission) => {
                    assert_eq!(
                        permission_owned_in_sources(&[], &roles, &[role.clone()], permission),
                        allowed
                    );
                    assert!(!permission_owned_in_sources(
                        &[],
                        &roles,
                        &[other.clone()],
                        permission
                    ));
                }
                AnyPermission::CanCheckSorafsFinalPromotionAccountCustody(permission) => {
                    assert_eq!(
                        permission_owned_in_sources(&[], &roles, &[role.clone()], permission),
                        allowed
                    );
                    assert!(!permission_owned_in_sources(
                        &[],
                        &roles,
                        &[other.clone()],
                        permission
                    ));
                }
                _ => panic!("fixture is a final-promotion permission"),
            }
        }
    }
}

#[test]
fn final_promotion_account_exact_holders_can_grant_and_revoke_without_capability_expansion() {
    let authority = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse::<iroha_crypto::PublicKey>()
            .expect("fixture key"),
    );
    let context = Context {
        authority: authority.clone(),
        curr_block: BlockHeader::new(NonZeroU64::new(2).expect("height"), None, None, None, 0, 0),
    };
    for manage in [false, true] {
        let exact = token(manage, "production-primary");
        let any = AnyPermission::try_from(&exact).expect("typed permission");
        for permissions in [
            vec![exact],
            vec![],
            vec![
                token(!manage, "production-primary"),
                token(manage, "production-secondary"),
            ],
        ] {
            let allowed = permissions.len() == 1;
            with_mock_permissions(permissions, || {
                assert_eq!(
                    any.validate_grant(&authority, &context, &Iroha).is_ok(),
                    allowed
                );
                assert_eq!(
                    any.validate_revoke(&authority, &context, &Iroha).is_ok(),
                    allowed
                );
            });
        }
    }
}
