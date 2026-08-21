// NEVO DPN permission regressions share the default-executor module.
#[cfg(test)]
mod dpn_permission_tests {
    use core::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        permission::Permission as PermissionObject,
        prelude::{AccountId, Grant, Json, Register, Revoke, Role, RoleId, ValidationFail},
    };
    use iroha_executor_data_model::permission::dpn::{
        DpnAdmin, DpnEprGuard, DpnInori, DpnSettlement, DpnUser,
    };

    use super::*;
    use crate::{Iroha, permission::test_override, prelude};

    #[derive(Debug)]
    struct TestExecutor {
        host: Iroha,
        context: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }

    impl TestExecutor {
        fn at_height(authority: AccountId, height: u64) -> Self {
            Self {
                host: Iroha,
                context: prelude::Context {
                    authority,
                    curr_block: BlockHeader::new(
                        NonZeroU64::new(height).expect("non-zero block height"),
                        None,
                        None,
                        None,
                        0,
                        0,
                    ),
                },
                verdict: Ok(()),
            }
        }

        fn genesis(authority: AccountId) -> Self {
            Self::at_height(authority, 1)
        }

        fn post_genesis(authority: AccountId) -> Self {
            Self::at_height(authority, 2)
        }
    }

    impl Execute for TestExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }

        fn context(&self) -> &prelude::Context {
            &self.context
        }

        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.context
        }

        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }

        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }

    impl Visit for TestExecutor {}

    struct PermissionOverride(Vec<PermissionObject>);

    impl PermissionOverride {
        fn install(permissions: Vec<PermissionObject>) -> Self {
            Self(test_override::replace_permissions(permissions))
        }
    }

    impl Drop for PermissionOverride {
        fn drop(&mut self) {
            test_override::replace_permissions(core::mem::take(&mut self.0));
        }
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic DPN permission fixture account");
        AccountId::new(key_pair.public_key().clone())
    }

    fn dpn_permissions() -> [PermissionObject; 5] {
        [
            DpnAdmin.into(),
            DpnUser.into(),
            DpnInori.into(),
            DpnSettlement.into(),
            DpnEprGuard.into(),
        ]
    }

    fn subordinate_dpn_permissions() -> [PermissionObject; 4] {
        [
            DpnUser.into(),
            DpnInori.into(),
            DpnSettlement.into(),
            DpnEprGuard.into(),
        ]
    }

    #[test]
    fn genesis_accepts_exact_dpn_account_permission_grants() {
        let bootstrap = account(70);
        let destination = account(71);
        for permission in dpn_permissions() {
            let grant = Grant::account_permission(permission.clone(), destination.clone());
            let mut executor = TestExecutor::genesis(bootstrap.clone());
            permission::visit_grant_account_permission(&mut executor, &grant);
            assert!(
                executor.verdict().is_ok(),
                "genesis must accept exact DPN permission {permission:?}: {:?}",
                executor.verdict(),
            );
        }
    }

    #[test]
    fn exact_dpn_admin_may_grant_and_revoke_every_dpn_permission() {
        let admin = account(72);
        let destination = account(73);
        let _override = PermissionOverride::install(vec![DpnAdmin.into()]);
        for permission in dpn_permissions() {
            let grant = Grant::account_permission(permission.clone(), destination.clone());
            let mut grant_executor = TestExecutor::post_genesis(admin.clone());
            permission::visit_grant_account_permission(&mut grant_executor, &grant);
            assert!(
                grant_executor.verdict().is_ok(),
                "DpnAdmin must be able to grant {permission:?}: {:?}",
                grant_executor.verdict(),
            );

            let revoke = Revoke::account_permission(permission.clone(), destination.clone());
            let mut revoke_executor = TestExecutor::post_genesis(admin.clone());
            permission::visit_revoke_account_permission(&mut revoke_executor, &revoke);
            assert!(
                revoke_executor.verdict().is_ok(),
                "DpnAdmin must be able to revoke {permission:?}: {:?}",
                revoke_executor.verdict(),
            );
        }
    }

    #[test]
    fn subordinate_dpn_holders_cannot_redelegate_or_revoke_their_permission() {
        let holder = account(74);
        let destination = account(75);
        for permission in subordinate_dpn_permissions() {
            let _override = PermissionOverride::install(vec![permission.clone()]);
            for operation in ["grant", "revoke"] {
                let mut executor = TestExecutor::post_genesis(holder.clone());
                if operation == "grant" {
                    permission::visit_grant_account_permission(
                        &mut executor,
                        &Grant::account_permission(permission.clone(), destination.clone()),
                    );
                } else {
                    permission::visit_revoke_account_permission(
                        &mut executor,
                        &Revoke::account_permission(permission.clone(), destination.clone()),
                    );
                }
                let error = executor
                    .verdict()
                    .as_ref()
                    .expect_err("a non-admin DPN holder must not mutate DPN permissions");
                assert!(matches!(error, ValidationFail::NotPermitted(_)));
                assert!(error.to_string().contains("exact DpnAdmin holder"));
            }
        }
    }

    #[test]
    fn non_admin_cannot_grant_dpn_admin() {
        let authority = account(80);
        let destination = account(81);
        let _override = PermissionOverride::install(vec![DpnUser.into()]);
        let mut executor = TestExecutor::post_genesis(authority);
        permission::visit_grant_account_permission(
            &mut executor,
            &Grant::account_permission(DpnAdmin, destination),
        );
        let error = executor
            .verdict()
            .as_ref()
            .expect_err("a non-admin must not create a DpnAdmin holder");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(error.to_string().contains("exact DpnAdmin holder"));
    }

    #[test]
    fn malformed_dpn_payloads_fail_closed_even_at_genesis() {
        let bootstrap = account(76);
        let destination = account(77);
        for name in [
            "DpnAdmin",
            "DpnUser",
            "DpnInori",
            "DpnSettlement",
            "DpnEprGuard",
        ] {
            for payload in ["{}", "[]", "true", "\"unexpected\""] {
                let malformed = PermissionObject::new(
                    name.parse().expect("DPN permission name"),
                    Json::from_raw_json(payload.to_owned()).expect("valid JSON fixture"),
                );
                let grant = Grant::account_permission(malformed, destination.clone());
                let mut executor = TestExecutor::genesis(bootstrap.clone());
                permission::visit_grant_account_permission(&mut executor, &grant);
                let error = executor
                    .verdict()
                    .as_ref()
                    .expect_err("malformed DPN payload must be rejected");
                assert!(matches!(error, ValidationFail::NotPermitted(_)));
                assert!(error.to_string().contains("Unknown permission"));
            }
        }
    }

    #[test]
    fn dpn_permissions_cannot_be_embedded_in_roles() {
        let bootstrap = account(78);
        let destination = account(79);
        for (index, permission) in dpn_permissions().into_iter().enumerate() {
            let role = Role::new(
                format!("dpn_role_{index}")
                    .parse::<RoleId>()
                    .expect("role id"),
                destination.clone(),
            )
            .add_permission(permission);
            let mut executor = TestExecutor::genesis(bootstrap.clone());
            role::visit_register_role(&mut executor, &Register::role(role));
            let error = executor
                .verdict()
                .as_ref()
                .expect_err("DPN permissions must remain exact account grants");
            assert!(matches!(error, ValidationFail::NotPermitted(_)));
            assert!(error.to_string().contains("never embedded in roles"));
        }
    }


    #[test]
    fn dpn_admin_cannot_be_granted_to_a_role() {
        let bootstrap = account(82);
        let role_id = "dpn_admin_role"
            .parse::<RoleId>()
            .expect("role id");
        let mut executor = TestExecutor::genesis(bootstrap);
        role::visit_grant_role_permission(
            &mut executor,
            &Grant::role_permission(DpnAdmin, role_id),
        );
        let error = executor
            .verdict()
            .as_ref()
            .expect_err("DpnAdmin must not be granted to a role");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(error.to_string().contains("never to roles"));
    }
}
