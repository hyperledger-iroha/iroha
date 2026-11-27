//! Restore refuses an unsupported selection without replacing an original local codec refusal.

use super::*;

pub(super) fn validate(world: &impl WorldReadOnly) -> Result<(), StateRestoreError> {
    use crate::{
        execution_attempt::ExecutionAttemptError,
        runtime::attestation::{ReleaseSupport, committed_release_support},
    };
    let catalog = crate::runtime::release_catalog().map_err(StateRestoreError::RuntimeCatalog)?;
    match committed_release_support(catalog, world).map_err(|original| {
        match original.into_attempt() {
            ExecutionAttemptError::Deferred(original) => {
                StateRestoreError::ExecutionDeferred(original)
            }
            ExecutionAttemptError::Rejected(original) => {
                StateRestoreError::RuntimeSupport(original)
            }
        }
    })? {
        ReleaseSupport::Supported => Ok(()),
        ReleaseSupport::Missing(definition) => {
            Err(StateRestoreError::RuntimeUnsupported(definition))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_restore_keeps_its_support_check_and_generic_qualification_stays_closed() {
        let world = World::default();
        validate(&world.view()).unwrap();
        let catalog = crate::runtime::release_catalog().unwrap();
        let kernel = catalog.generic_carriers.unwrap().kernel;
        {
            let mut block = world.block();
            block.runtime_selection.get_mut().kernel = kernel;
            block.commit();
        }
        let error = validate(&world.view()).unwrap_err();
        assert!(matches!(error, StateRestoreError::RuntimeUnsupported(actual) if actual == kernel));
        let exported = crate::snapshot::TryReadError::from(error);
        assert!(
            exported.halts_restore(),
            "older generations must not hide unsupported selected code"
        );
    }
}
