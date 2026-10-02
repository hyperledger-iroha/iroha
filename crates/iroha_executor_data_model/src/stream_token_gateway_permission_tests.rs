//! Genesis-bootstrap gateway policy administration and exact scoped operation permissions.

use crate::permission::{
    Permission as _,
    sorafs::{CanManageSorafsStreamTokenGateway, CanOperateSorafsStreamTokenGateway},
};
use iroha_data_model::permission::Permission;

#[test]
fn gateway_management_and_operation_permissions_have_distinct_canonical_identity() {
    let manage = CanManageSorafsStreamTokenGateway;
    let operate = CanOperateSorafsStreamTokenGateway {
        gateway_id: [0x11; 32],
    };
    assert_eq!(
        CanManageSorafsStreamTokenGateway::name(),
        "CanManageSorafsStreamTokenGateway"
    );
    assert_eq!(
        CanOperateSorafsStreamTokenGateway::name(),
        "CanOperateSorafsStreamTokenGateway"
    );
    let manage_token: Permission = manage.into();
    let operate_token: Permission = operate.into();
    assert_ne!(manage_token, operate_token);
    assert_eq!(
        CanManageSorafsStreamTokenGateway::try_from(&manage_token).unwrap(),
        manage
    );
    assert_eq!(
        CanOperateSorafsStreamTokenGateway::try_from(&operate_token).unwrap(),
        operate
    );
    assert!(CanManageSorafsStreamTokenGateway::try_from(&operate_token).is_err());
    assert!(CanOperateSorafsStreamTokenGateway::try_from(&manage_token).is_err());
    assert_ne!(
        Permission::from(CanOperateSorafsStreamTokenGateway {
            gateway_id: [0x12; 32]
        }),
        operate_token
    );
    for token in [manage_token, operate_token] {
        let frame = norito::encode_canonical(&token).unwrap();
        assert_eq!(
            norito::decode_canonical::<Permission>(&frame).unwrap(),
            token
        );
        let json = norito::json::to_json(&token).unwrap();
        assert_eq!(norito::json::from_str::<Permission>(&json).unwrap(), token);
    }
    let json = norito::json::to_json(&operate).unwrap();
    assert_eq!(
        norito::json::from_str::<CanOperateSorafsStreamTokenGateway>(&json).unwrap(),
        operate
    );
    let extra = json.replacen('{', "{\"unexpected\":true,", 1);
    assert!(norito::json::from_str::<CanOperateSorafsStreamTokenGateway>(&extra).is_err());
    assert!(norito::json::from_str::<CanOperateSorafsStreamTokenGateway>("{}").is_err());
    assert!(
        norito::json::from_str::<CanOperateSorafsStreamTokenGateway>("{\"gateway_id\":[17]}")
            .is_err()
    );
    let manage_json = norito::json::to_json(&manage).unwrap();
    assert_eq!(
        norito::json::from_str::<CanManageSorafsStreamTokenGateway>(&manage_json).unwrap(),
        manage
    );
    for invalid in ["{}", "{\"gateway_id\":[17]}", "[]"] {
        assert!(norito::json::from_str::<CanManageSorafsStreamTokenGateway>(invalid).is_err());
    }
}

#[test]
fn gateway_check_permission_is_canonical_exact_scope_and_separate_from_operation() {
    use crate::permission::sorafs::CanCheckSorafsStreamTokenGateway;
    let check = CanCheckSorafsStreamTokenGateway {
        gateway_id: [0x11; 32],
    };
    assert_eq!(
        CanCheckSorafsStreamTokenGateway::name(),
        "CanCheckSorafsStreamTokenGateway"
    );
    let token: Permission = check.into();
    let operate: Permission = CanOperateSorafsStreamTokenGateway {
        gateway_id: check.gateway_id,
    }
    .into();
    let manage: Permission = CanManageSorafsStreamTokenGateway.into();
    assert_ne!(token, operate);
    assert_ne!(token, manage);
    assert!(CanOperateSorafsStreamTokenGateway::try_from(&token).is_err());
    assert!(CanCheckSorafsStreamTokenGateway::try_from(&operate).is_err());
    assert!(CanCheckSorafsStreamTokenGateway::try_from(&manage).is_err());
    assert_ne!(
        token,
        Permission::from(CanCheckSorafsStreamTokenGateway {
            gateway_id: [0x12; 32]
        })
    );
    assert_eq!(
        CanCheckSorafsStreamTokenGateway::try_from(&token).unwrap(),
        check
    );
    let frame = norito::encode_canonical(&token).unwrap();
    assert_eq!(
        norito::decode_canonical::<Permission>(&frame).unwrap(),
        token
    );
    let json = norito::json::to_json(&token).unwrap();
    assert_eq!(norito::json::from_str::<Permission>(&json).unwrap(), token);
    let json = norito::json::to_json(&check).unwrap();
    assert_eq!(
        norito::json::from_str::<CanCheckSorafsStreamTokenGateway>(&json).unwrap(),
        check
    );
    assert!(
        norito::json::from_str::<CanCheckSorafsStreamTokenGateway>(&json.replacen(
            '{',
            "{\"unexpected\":true,",
            1
        ))
        .is_err()
    );
    for malformed in ["null", "{}", "{\"gateway_id\":[17]}"] {
        assert!(norito::json::from_str::<CanCheckSorafsStreamTokenGateway>(malformed).is_err());
    }
}
