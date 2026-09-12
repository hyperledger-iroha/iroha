//! Exact provider scope and delegation regressions for stream-token custody.
use super::*;
use crate::tests::with_mock_permissions;
use iroha_data_model::sorafs::capacity::ProviderId;
use iroha_executor_data_model::permission::sorafs::{
    CanManageSorafsStreamTokenCustody, CanSetSorafsPricing,
};
use std::num::NonZeroU64;

fn token(byte: u8) -> CanManageSorafsStreamTokenCustody {
    CanManageSorafsStreamTokenCustody {
        provider_id: ProviderId::new([byte; 32]),
    }
}

#[test]
fn stream_token_custody_permission_roundtrip_keeps_provider_scope() {
    let permission: PermissionObject = token(1).into();
    let any = AnyPermission::try_from(&permission).expect("known typed custody permission");
    assert!(
        matches!(&any, AnyPermission::CanManageSorafsStreamTokenCustody(value) if value == &token(1))
    );
    assert!(!any.is_genesis_only());
    assert!(any.is_holder_delegable());
    let roundtrip: PermissionObject = any.into();
    assert_eq!(roundtrip, permission);
    assert_ne!(roundtrip, PermissionObject::from(token(2)));
    let malformed = PermissionObject::new(
        "CanManageSorafsStreamTokenCustody".to_owned(),
        Json::new(()),
    );
    assert!(AnyPermission::try_from(&malformed).is_err());
}

#[test]
fn stream_token_custody_permissions_match_only_assigned_roles_and_exact_provider() {
    let role: RoleId = "custody_operator".parse().expect("role id");
    let other: RoleId = "other_operator".parse().expect("role id");
    let roles = vec![(role.clone(), PermissionObject::from(token(1)))];
    assert!(permission_owned_in_sources(
        &[],
        &roles,
        &[role.clone()],
        &token(1)
    ));
    assert!(!permission_owned_in_sources(
        &[],
        &roles,
        &[role],
        &token(2)
    ));
    assert!(!permission_owned_in_sources(
        &[],
        &roles,
        &[other],
        &token(1)
    ));
    assert!(permission_owned_in_sources(
        &[token(1).into()],
        &[],
        &[],
        &token(1)
    ));
    assert!(!permission_owned_in_sources(
        &[token(2).into(), CanSetSorafsPricing.into()],
        &[],
        &[],
        &token(1)
    ));
}

#[test]
fn stream_token_custody_exact_holders_can_grant_and_revoke_without_scope_expansion() {
    let authority = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse::<iroha_crypto::PublicKey>()
            .expect("fixture key"),
    );
    let context = Context {
        authority: authority.clone(),
        curr_block: BlockHeader::new(NonZeroU64::new(2).expect("height"), None, None, None, 0, 0),
    };
    let exact: PermissionObject = token(1).into();
    let any = AnyPermission::try_from(&exact).expect("typed permission");
    with_mock_permissions(vec![exact], || {
        assert!(any.validate_grant(&authority, &context, &Iroha).is_ok());
        assert!(any.validate_revoke(&authority, &context, &Iroha).is_ok());
    });
    with_mock_permissions(vec![token(2).into(), CanSetSorafsPricing.into()], || {
        assert!(any.validate_grant(&authority, &context, &Iroha).is_err());
        assert!(any.validate_revoke(&authority, &context, &Iroha).is_err());
    });
}
