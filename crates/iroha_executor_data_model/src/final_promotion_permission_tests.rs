//! Exact JSON and schema identities for deployment-scoped final-promotion permissions.
use crate::permission::{
    Permission as _,
    sorafs::{
        CanCheckSorafsFinalPromotion, CanCheckSorafsFinalPromotionAccountCustody,
        CanManageSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionCustody,
        CanOperateSorafsFinalPromotion,
    },
};
use iroha_data_model::permission::Permission;

#[test]
fn final_promotion_permission_json_is_closed_and_preserves_exact_deployment() {
    macro_rules! check {
        ($permission:ident) => {{
            let token = $permission {
                deployment_id: "production-primary".to_owned(),
            };
            assert_eq!($permission::name(), stringify!($permission));
            assert_eq!(
                norito::json::to_json(&token).expect("canonical JSON"),
                r#"{"deployment_id":"production-primary"}"#
            );
            let object: Permission = token.clone().into();
            assert_eq!(
                $permission::try_from(&object).expect("typed roundtrip"),
                token
            );
            for malformed in [
                "null",
                "{}",
                r#"{"deployment_id":null}"#,
                r#"{"deployment_id":true}"#,
                r#"{"deployment_id":1}"#,
                r#"{"deploymentId":"production-primary"}"#,
                r#"{"deployment_id":"production-primary","extra":0}"#,
                r#"{"deployment_id":"production-primary","deployment_id":"production-secondary"}"#,
            ] {
                assert!(
                    norito::json::from_str::<$permission>(malformed).is_err(),
                    "{malformed}"
                );
            }
        }};
    }
    check!(CanManageSorafsFinalPromotionCustody);
    check!(CanOperateSorafsFinalPromotion);
    check!(CanCheckSorafsFinalPromotion);
    check!(CanManageSorafsFinalPromotionAccountCustody);
    check!(CanCheckSorafsFinalPromotionAccountCustody);
}

#[test]
fn account_custody_tokens_are_distinct_from_receipt_management_and_operation() {
    let deployment_id = "production-primary".to_owned();
    let tokens: [Permission; 5] = [
        CanManageSorafsFinalPromotionCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanOperateSorafsFinalPromotion {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanManageSorafsFinalPromotionAccountCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanCheckSorafsFinalPromotionAccountCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        CanCheckSorafsFinalPromotion { deployment_id }.into(),
    ];
    for (index, token) in tokens.iter().enumerate() {
        assert_eq!(
            CanManageSorafsFinalPromotionAccountCustody::try_from(token).is_ok(),
            index == 2
        );
        assert_eq!(
            CanCheckSorafsFinalPromotionAccountCustody::try_from(token).is_ok(),
            index == 3
        );
        assert_eq!(
            CanCheckSorafsFinalPromotion::try_from(token).is_ok(),
            index == 4
        );
        for other in tokens.iter().skip(index + 1) {
            assert_ne!(token, other);
        }
    }
}

#[test]
fn receipt_check_permission_has_exact_canonical_object_identity() {
    let token = CanCheckSorafsFinalPromotion {
        deployment_id: "production-primary".into(),
    };
    let permission: Permission = token.clone().into();
    let frame = norito::encode_canonical(&permission).unwrap();
    let decoded = norito::decode_canonical::<Permission>(&frame).unwrap();
    assert_eq!(decoded, permission);
    assert_eq!(
        CanCheckSorafsFinalPromotion::try_from(&decoded).unwrap(),
        token
    );
    assert!(CanOperateSorafsFinalPromotion::try_from(&decoded).is_err());
    assert!(CanManageSorafsFinalPromotionCustody::try_from(&decoded).is_err());
    assert!(CanCheckSorafsFinalPromotionAccountCustody::try_from(&decoded).is_err());
    assert_ne!(
        permission,
        CanCheckSorafsFinalPromotion {
            deployment_id: "production-secondary".into()
        }
        .into()
    );
}
