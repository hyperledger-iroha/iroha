// Governed offline permission regressions share the default-executor module.
#[cfg(test)]
mod governed_offline_permission_tests {
    use core::num::NonZeroU64;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        nexus::FeeSponsorProgramId,
        permission::Permission as PermissionObject,
        prelude::{AccountId, Grant, Json, Register, Revoke, Role, RoleId, ValidationFail},
    };
    use iroha_executor_data_model::permission::{
        nexus::{CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram},
        offline::{
            CanActivateKagemushaRecursiveReleaseV4, CanManageOfflineDeviceAttestationPolicy,
            CanManageOfflineEscrow,
        },
        parameter::CanSetParameters,
        role::CanManageRoles,
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
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic permission fixture account");
        AccountId::new(key_pair.public_key().clone())
    }
    fn governed_offline_permissions() -> [PermissionObject; 3] {
        [
            CanManageOfflineEscrow.into(),
            CanActivateKagemushaRecursiveReleaseV4.into(),
            CanManageOfflineDeviceAttestationPolicy.into(),
        ]
    }
    fn assert_genesis_only_denial(
        verdict: &Result<(), ValidationFail>,
        permission: &PermissionObject,
    ) {
        let error = verdict
            .as_ref()
            .expect_err("post-genesis governed offline permission mutation must fail");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(
            error
                .to_string()
                .contains("only allowed inside the genesis block"),
            "unexpected rejection for {permission:?}: {error}",
        );
    }
    #[test]
    fn genesis_accepts_all_exact_governed_offline_permission_grants() {
        let bootstrap = account(40);
        let destination = account(41);
        for token in governed_offline_permissions() {
            let grant = Grant::account_permission(token.clone(), destination.clone());
            let mut executor = TestExecutor::genesis(bootstrap.clone());
            permission::visit_grant_account_permission(&mut executor, &grant);
            assert!(
                executor.verdict().is_ok(),
                "default executor must admit the exact genesis grant {token:?}: {:?}",
                executor.verdict(),
            );
        }
    }
    #[test]
    fn governed_offline_permissions_are_not_delegable_post_genesis() {
        let banking = account(41);
        let third_party = account(42);
        for token in governed_offline_permissions() {
            let previous = test_override::replace_permissions(vec![token.clone()]);
            for destination in [banking.clone(), third_party.clone()] {
                let grant = Grant::account_permission(token.clone(), destination);
                let mut executor = TestExecutor::post_genesis(banking.clone());
                permission::visit_grant_account_permission(&mut executor, &grant);
                assert_genesis_only_denial(executor.verdict(), &token);
            }
            test_override::replace_permissions(previous);
        }
    }
    #[test]
    fn governed_offline_permissions_are_not_revocable_post_genesis() {
        let banking = account(43);
        for token in governed_offline_permissions() {
            let previous = test_override::replace_permissions(vec![token.clone()]);
            let revoke = Revoke::account_permission(token.clone(), banking.clone());
            let mut executor = TestExecutor::post_genesis(banking.clone());
            permission::visit_revoke_account_permission(&mut executor, &revoke);
            test_override::replace_permissions(previous);
            assert_genesis_only_denial(executor.verdict(), &token);
        }
    }
    fn role_with_permissions(
        id: &str,
        grant_to: AccountId,
        permissions: impl IntoIterator<Item = PermissionObject>,
    ) -> Role {
        let mut role = Role::new(id.parse::<RoleId>().expect("role id"), grant_to);
        for permission in permissions {
            role = role.add_permission(permission);
        }
        role.inner().clone()
    }
    #[test]
    fn role_membership_revalidates_genesis_only_permissions() {
        let holder = account(44);
        let context = TestExecutor::post_genesis(holder.clone());
        for (index, permission) in governed_offline_permissions().into_iter().enumerate() {
            let role = role_with_permissions(
                &format!("governed_offline_{index}"),
                holder.clone(),
                [permission.clone()],
            );
            for operation in [
                role::RoleDelegationOperation::Grant,
                role::RoleDelegationOperation::Revoke,
            ] {
                let error = role::validate_role_delegation_permissions(
                    &role,
                    &holder,
                    context.context(),
                    context.host(),
                    operation,
                )
                .expect_err("a genesis-only permission must not escape through role membership");
                assert!(
                    error
                        .to_string()
                        .contains("only allowed inside the genesis block"),
                    "unexpected role rejection for {permission:?}: {error}",
                );
            }
        }
    }
    #[test]
    fn role_membership_revalidates_every_sponsor_bound_permission() {
        let sponsor = account(45);
        let outsider = account(46);
        let context = TestExecutor::post_genesis(outsider.clone());
        let program_id = FeeSponsorProgramId::new(
            sponsor.clone(),
            "retail".parse().expect("retail program name"),
        );
        let permissions = [
            PermissionObject::from(CanManageFeeSponsorProgram {
                sponsor: sponsor.clone(),
            }),
            PermissionObject::from(CanEnrollFeeSponsorProgram { program_id }),
        ];
        let previous =
            test_override::replace_permissions(vec![PermissionObject::from(CanSetParameters)]);
        for (index, permission) in permissions.into_iter().enumerate() {
            let role = role_with_permissions(
                &format!("sponsor_bound_{index}"),
                outsider.clone(),
                [permission.clone()],
            );
            for operation in [
                role::RoleDelegationOperation::Grant,
                role::RoleDelegationOperation::Revoke,
            ] {
                let error = role::validate_role_delegation_permissions(
                    &role,
                    &outsider,
                    context.context(),
                    context.host(),
                    operation,
                )
                .expect_err("a non-sponsor role holder must not redelegate sponsor authority");
                assert!(matches!(error, ValidationFail::NotPermitted(_)));
            }
        }
        test_override::replace_permissions(previous);
    }
    #[test]
    fn role_membership_preserves_safe_exact_permission_delegation() {
        let holder = account(47);
        let context = TestExecutor::post_genesis(holder.clone());
        let permission = PermissionObject::from(CanSetParameters);
        let role = role_with_permissions(
            "ordinary_exact_permission",
            holder.clone(),
            [permission.clone()],
        );
        let previous = test_override::replace_permissions(vec![permission]);
        for operation in [
            role::RoleDelegationOperation::Grant,
            role::RoleDelegationOperation::Revoke,
        ] {
            role::validate_role_delegation_permissions(
                &role,
                &holder,
                context.context(),
                context.host(),
                operation,
            )
            .expect("an exact holder may delegate an ordinary role");
        }
        test_override::replace_permissions(previous);
    }
    #[test]
    fn role_membership_rejects_unknown_permission_contents() {
        let holder = account(48);
        let context = TestExecutor::post_genesis(holder.clone());
        let role = role_with_permissions(
            "unknown_permission",
            holder.clone(),
            [PermissionObject::new(
                "UnknownRolePermission".to_owned(),
                Json::new(()),
            )],
        );
        let error = role::validate_role_delegation_permissions(
            &role,
            &holder,
            context.context(),
            context.host(),
            role::RoleDelegationOperation::Grant,
        )
        .expect_err("unknown role permissions must fail closed");
        assert!(error.to_string().contains("Unknown permission"));
    }
    #[test]
    fn role_registration_validates_sponsor_permissions_against_transaction_authority() {
        let sponsor = account(49);
        let manager = account(50);
        let role = Role::new(
            "manager_seeded_sponsor_role"
                .parse::<RoleId>()
                .expect("role id"),
            sponsor.clone(),
        )
        .add_permission(CanManageFeeSponsorProgram { sponsor });
        let registration = Register::role(role);
        let previous =
            test_override::replace_permissions(vec![PermissionObject::from(CanManageRoles)]);
        let mut executor = TestExecutor::post_genesis(manager);
        role::visit_register_role(&mut executor, &registration);
        test_override::replace_permissions(previous);
        let error = executor
            .verdict()
            .as_ref()
            .expect_err("a non-sponsor role manager must not seed sponsor authority");
        assert!(matches!(error, ValidationFail::NotPermitted(_)));
        assert!(error.to_string().contains("only the sponsor account"));
    }
}
