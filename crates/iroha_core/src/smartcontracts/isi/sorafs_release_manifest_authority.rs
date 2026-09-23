//! Closed native role-13 admission with exact deployment permission separation.
//!
//! The canonical instruction and grants are registered now so no generic signer can interpret
//! a role-13 claim as signing authority. Core must not accept any transition until its own
//! finalized custody, replay-tombstone operation journal and executed Check consumer exist.
//! TODO: Implement the purpose-owned State transition and Kura-backed Check proof as one
//! reviewed cut; only then change this handler's closed dispatch disposition.

use super::Execute;
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{MutateSorafsReleaseManifestAuthority, RELEASE_MANIFEST_INSTRUCTION_MAX_BYTES_V1},
    },
    permission::Permission,
    sorafs::release_manifest_authority::{
        RELEASE_MANIFEST_ACTION_MAX_BYTES_V1, ReleaseManifestActionV1 as Action,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsReleaseManifest, CanManageSorafsReleaseManifestCustody,
    CanOperateSorafsReleaseManifest,
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::protocol::{
    SIGNER_MAX_ID_BYTES_V1, SignerPurposeBindingV1, SignerRoleV1,
};

const INVALID: &str = "release-manifest authority instruction or permission rejected";
const CLOSED: &str = "release-manifest native authority is closed until finalized storage and Check execution are installed";

/// Check canonical bounds and exact deployment-scoped role-13 grants, without admitting state.
fn authorized(
    world: &impl WorldReadOnly,
    authority: &AccountId,
    instruction: &MutateSorafsReleaseManifestAuthority,
) -> bool {
    let deployment = &instruction.deployment_id;
    if deployment.is_empty()
        || deployment.len() > SIGNER_MAX_ID_BYTES_V1
        || !(SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: deployment.clone(),
        })
        .validates_role(SignerRoleV1::ReleaseManifest)
        || norito::canonical_frame_len(&instruction.action)
            .map_or(true, |size| size > RELEASE_MANIFEST_ACTION_MAX_BYTES_V1)
        || norito::canonical_frame_len(instruction).map_or(true, |size| {
            size > RELEASE_MANIFEST_INSTRUCTION_MAX_BYTES_V1
        })
    {
        return false;
    }
    let has = |account: &AccountId, permission: Permission| {
        world.accounts().get(account).is_some()
            && (world.account_contains_inherent_permission(account, &permission)
                || world
                    .account_roles_iter(account)
                    .filter_map(|id| world.roles().get(id))
                    .any(|role| role.permissions().any(|token| token == &permission)))
    };
    let required = match &instruction.action {
        Action::Configure(_) | Action::Enroll(_) | Action::Revoke { .. } => {
            CanManageSorafsReleaseManifestCustody {
                deployment_id: deployment.clone(),
            }
            .into()
        }
        Action::Reserve(_) | Action::Complete(_) | Action::Expire(_) => {
            CanOperateSorafsReleaseManifest {
                deployment_id: deployment.clone(),
            }
            .into()
        }
        Action::Check(check) => {
            if authority == &check.expected_operator
                || !has(
                    &check.expected_operator,
                    CanOperateSorafsReleaseManifest {
                        deployment_id: deployment.clone(),
                    }
                    .into(),
                )
            {
                return false;
            }
            CanCheckSorafsReleaseManifest {
                deployment_id: deployment.clone(),
            }
            .into()
        }
    };
    has(authority, required)
}

impl Execute for MutateSorafsReleaseManifestAuthority {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let reason = if authorized(tx.world(), authority, &self) {
            CLOSED
        } else {
            INVALID
        };
        Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(reason.into()),
        ))
    }
}

#[cfg(test)]
mod tests;
