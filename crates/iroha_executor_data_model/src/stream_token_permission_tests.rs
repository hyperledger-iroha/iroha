//! Provider-scoped role-11 operation and independent observer permission identities.

use crate::permission::{
    Permission as _,
    sorafs::{
        CanCheckSorafsStreamToken, CanManageSorafsStreamTokenCustody, CanOperateSorafsStreamToken,
    },
};
use iroha_data_model::{permission::Permission, sorafs::capacity::ProviderId};

#[test]
fn stream_token_operation_and_check_permissions_have_exact_distinct_provider_scope() {
    let provider_id = ProviderId::new([0x11; 32]);
    let operator = CanOperateSorafsStreamToken { provider_id };
    let observer = CanCheckSorafsStreamToken { provider_id };
    assert_eq!(
        CanOperateSorafsStreamToken::name(),
        "CanOperateSorafsStreamToken"
    );
    assert_eq!(
        CanCheckSorafsStreamToken::name(),
        "CanCheckSorafsStreamToken"
    );
    for permission in [
        Permission::from(operator),
        Permission::from(observer),
        Permission::from(CanManageSorafsStreamTokenCustody { provider_id }),
    ] {
        let frame = norito::encode_canonical(&permission).unwrap();
        assert_eq!(
            norito::decode_canonical::<Permission>(&frame).unwrap(),
            permission
        );
    }
    let operator_token: Permission = operator.into();
    let observer_token: Permission = observer.into();
    assert_ne!(operator_token, observer_token);
    assert_eq!(
        CanOperateSorafsStreamToken::try_from(&operator_token),
        Ok(operator)
    );
    assert_eq!(
        CanCheckSorafsStreamToken::try_from(&observer_token),
        Ok(observer)
    );
    assert!(CanCheckSorafsStreamToken::try_from(&operator_token).is_err());
    assert!(CanOperateSorafsStreamToken::try_from(&observer_token).is_err());
    assert!(CanManageSorafsStreamTokenCustody::try_from(&operator_token).is_err());
    let foreign = CanOperateSorafsStreamToken {
        provider_id: ProviderId::new([0x12; 32]),
    };
    assert_ne!(Permission::from(foreign), operator_token);
    for token in [operator_token, observer_token] {
        let json = norito::json::to_json(&token).unwrap();
        assert_eq!(norito::json::from_str::<Permission>(&json).unwrap(), token);
    }
    let json = norito::json::to_json(&operator).unwrap();
    assert_eq!(
        norito::json::from_str::<CanOperateSorafsStreamToken>(&json),
        Ok(operator)
    );
    let with_extra = json.replacen("\"provider_id\":", "\"extra\":1,\"provider_id\":", 1);
    assert_ne!(with_extra, json);
    assert!(norito::json::from_str::<CanOperateSorafsStreamToken>(&with_extra).is_err());
}
