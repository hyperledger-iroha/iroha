//! Explicitly closed role-16 topology authority until native storage and finality are connected.

use super::{Execute, INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsTopologyAuthority,
    },
    permission::Permission,
    sorafs::topology_authority::{TopologyActionV1, TopologyTransitionV1},
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsTopologyApproval, CanManageSorafsTopologyCustody,
    CanOperateSorafsTopologyApproval,
};
use mv::storage::StorageReadOnly;

const TOPOLOGY_PERMISSION_REQUIRED_REASON: &str =
    "Exact deployment-scoped topology action permission is required";

fn has_permission(world: &impl WorldReadOnly, account: &AccountId, permission: Permission) -> bool {
    world.accounts().get(account).is_some()
        && (world.account_contains_inherent_permission(account, &permission)
            || world
                .account_roles_iter(account)
                .filter_map(|id| world.roles().get(id))
                .any(|role| role.permissions().any(|token| token == &permission)))
}

/// Read exact registered-account and role grants from the supplied current World view.
/// The caller must establish the execution or applied-finality cut; this predicate alone grants
/// no native topology operation, finalized approval, or signer use.
pub(crate) fn authorized(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    transition: &TopologyTransitionV1,
) -> bool {
    let deployment_id = &transition.deployment_id;
    let permission: Permission = match &transition.action {
        TopologyActionV1::Configure(_)
        | TopologyActionV1::Enroll(_)
        | TopologyActionV1::Revoke(_) => CanManageSorafsTopologyCustody {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        TopologyActionV1::Reserve(_)
        | TopologyActionV1::Complete(_)
        | TopologyActionV1::Expire(_) => CanOperateSorafsTopologyApproval {
            deployment_id: deployment_id.clone(),
        }
        .into(),
        TopologyActionV1::Check(check) => {
            if authority == &check.expected_operator
                || !has_permission(
                    world,
                    &check.expected_operator,
                    CanOperateSorafsTopologyApproval {
                        deployment_id: deployment_id.clone(),
                    }
                    .into(),
                )
            {
                return false;
            }
            CanCheckSorafsTopologyApproval {
                deployment_id: deployment_id.clone(),
            }
            .into()
        }
    };
    has_permission(world, authority, permission)
}

fn rejected(reason: &str) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(reason.into()))
}

impl Execute for MutateSorafsTopologyAuthority {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if !authorized(tx.world(), authority, &self.transition) {
            return Err(rejected(TOPOLOGY_PERMISSION_REQUIRED_REASON));
        }
        // TODO: retain and authenticate role-16 topology State, execution source, and finality
        // before replacing this closed result with any native transition or Check success.
        Err(rejected(INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON))
    }
}

#[cfg(test)]
mod tests;
