//! Final-promotion capability separation, exact deployment scope and delegation.
use super::*;
use crate::tests::with_mock_permissions;
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Capability {
    Manage,
    Operate,
    Check,
}

fn token(capability: Capability, deployment: &str) -> PermissionObject {
    let deployment_id = deployment.to_owned();
    match capability {
        Capability::Manage => CanManageSorafsFinalPromotionCustody { deployment_id }.into(),
        Capability::Operate => CanOperateSorafsFinalPromotion { deployment_id }.into(),
        Capability::Check => CanCheckSorafsFinalPromotion { deployment_id }.into(),
    }
}

#[test]
fn final_promotion_permissions_roundtrip_preserves_action_and_deployment_scope() {
    for capability in [Capability::Manage, Capability::Operate, Capability::Check] {
        let permission = token(capability, "production-primary");
        let any = AnyPermission::try_from(&permission).expect("known final-promotion permission");
        assert_eq!(
            matches!(&any, AnyPermission::CanManageSorafsFinalPromotionCustody(_)),
            capability == Capability::Manage
        );
        assert!(!any.is_genesis_only());
        assert!(any.is_holder_delegable());
        assert_eq!(PermissionObject::from(any), permission);
        assert_ne!(permission, token(capability, "production-secondary"));
        for other in [Capability::Manage, Capability::Operate, Capability::Check] {
            assert_eq!(
                permission == token(other, "production-primary"),
                capability == other
            );
        }
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
fn final_promotion_permissions_match_only_exact_direct_or_assigned_role_capabilities() {
    let role: RoleId = "promotion_operator".parse().expect("role id");
    let other: RoleId = "other_operator".parse().expect("role id");
    let authority = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse::<iroha_crypto::PublicKey>()
            .expect("fixture key"),
    );
    for capability in [Capability::Manage, Capability::Operate, Capability::Check] {
        let exact = token(capability, "production-primary");
        let any = AnyPermission::try_from(&exact).expect("typed permission");
        for held in [
            exact.clone(),
            token(Capability::Manage, "production-primary"),
            token(Capability::Operate, "production-primary"),
            token(Capability::Check, "production-primary"),
            token(capability, "production-secondary"),
        ] {
            let allowed = held == exact;
            with_mock_permissions(vec![held.clone()], || {
                assert_eq!(any.is_owned_by(&authority, &Iroha), allowed);
            });
            let roles = vec![(role.clone(), held)];
            match &any {
                AnyPermission::CanManageSorafsFinalPromotionCustody(permission) => {
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
                AnyPermission::CanOperateSorafsFinalPromotion(permission) => {
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
                AnyPermission::CanCheckSorafsFinalPromotion(permission) => {
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
fn final_promotion_exact_holders_can_grant_and_revoke_without_capability_expansion() {
    let authority = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse::<iroha_crypto::PublicKey>()
            .expect("fixture key"),
    );
    let context = Context {
        authority: authority.clone(),
        curr_block: BlockHeader::new(NonZeroU64::new(2).expect("height"), None, None, None, 0, 0),
    };
    for capability in [Capability::Manage, Capability::Operate, Capability::Check] {
        let exact = token(capability, "production-primary");
        let any = AnyPermission::try_from(&exact).expect("typed permission");
        for permissions in [
            vec![exact],
            vec![],
            [Capability::Manage, Capability::Operate, Capability::Check]
                .into_iter()
                .filter(|other| *other != capability)
                .map(|other| token(other, "production-primary"))
                .chain([token(capability, "production-secondary")])
                .collect(),
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
