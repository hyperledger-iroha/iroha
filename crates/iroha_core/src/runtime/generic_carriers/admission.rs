//! Separate external envelope admission for the exact new carrier kernel.
//!
//! Candidate shape grants no authority. The signed original owner and execution boundary
//! independently retain source identity; native envelopes keep their existing admission path.

use super::*;
use crate::{
    execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error},
    state::WorldReadOnly,
};
use iroha_data_model::{ValidationFail, transaction::Executable};

/// Recognize only one opaque original instruction under the exact carrier kernel.
/// Batches, typed instructions and all other kernels receive no candidate exemption.
pub(crate) fn original_candidate<'a>(
    selection: &RootSelectionV1,
    executable: &'a Executable,
) -> Result<Option<&'a InstructionBox>, super::super::CatalogError> {
    let catalog = super::super::release_catalog()?;
    let Some(profile) = catalog.generic_carriers else {
        return Ok(None);
    };
    if selection.kernel != profile.kernel {
        return Ok(None);
    }
    let Executable::Instructions(instructions) = executable else {
        return Ok(None);
    };
    if instructions.len() != 1 {
        return Ok(None);
    }
    Ok(instructions[0].as_opaque().map(|_| &instructions[0]))
}

/// Admit the new candidate through actual selected definitions and canonical carrier decoding.
/// `false` leaves the caller on the existing native envelope path. No sealed reveal or nested
/// invocation may call this external seam in place of its original envelope rule.
pub(crate) fn admit(
    world: &impl WorldReadOnly,
    height: u64,
    executable: &Executable,
) -> Result<bool, ExecutionAttemptError<ValidationFail>> {
    let rejected_catalog = |error: super::super::CatalogError| {
        ExecutionAttemptError::Rejected(ValidationFail::InternalError(error.to_string()))
    };
    let Some(instruction) =
        original_candidate(world.runtime_selection(), executable).map_err(rejected_catalog)?
    else {
        return Ok(false);
    };
    let catalog = super::super::release_catalog().map_err(rejected_catalog)?;
    let published = super::super::change_set::PublishedDefinitions(world.runtime_definitions());
    let decoded = decode_selected(
        catalog,
        world.runtime_selection(),
        &published,
        height,
        instruction,
    )
    .map_err(|error| match error {
        Error::Rejected(error) => {
            ExecutionAttemptError::Rejected(ValidationFail::InstructionFailed(error))
        }
        Error::Contract(reason) => {
            ExecutionAttemptError::Rejected(ValidationFail::NotPermitted(reason.into()))
        }
        Error::Codec(original) => canonical_decode_attempt_error(original, |error| {
            ValidationFail::NotPermitted(error.to_string())
        }),
    })?;
    if decoded.is_none() {
        return Err(ExecutionAttemptError::Rejected(
            ValidationFail::InternalError(
                "carrier candidate lost its exact selected decoder".into(),
            ),
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::World;

    #[test]
    fn exact_external_candidate_uses_actual_world_descriptors_and_preserves_local_refusal() {
        let fixture = super::super::tests::fixture();
        let executable = Executable::Instructions(vec![fixture.instruction.clone()].into());
        let world = World::default();
        assert!(
            original_candidate(world.runtime_selection.view().get(), &executable)
                .unwrap()
                .is_none()
        );
        {
            let mut block = world.block();
            *block.runtime_selection.get_mut() = fixture.selection;
            for (id, definition) in fixture.published {
                block.runtime_definitions.insert(id, definition);
            }
            block.commit();
        }
        assert!(admit(&world.view(), 10, &executable).unwrap());
        let no_allocation =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        assert!(
            norito::with_decode_limits_scope(no_allocation, || {
                admit(&world.view(), 10, &executable)
            })
            .unwrap()
        );
        let no_fields =
            norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX);
        let result =
            norito::with_decode_limits_scope(no_fields, || admit(&world.view(), 10, &executable));
        assert!(
            matches!(result, Err(ExecutionAttemptError::Deferred(original)) if original.decode_refusal().is_some())
        );
        let repeated =
            Executable::Instructions(vec![fixture.instruction.clone(), fixture.instruction].into());
        assert!(
            original_candidate(world.runtime_selection.view().get(), &repeated)
                .unwrap()
                .is_none()
        );
        assert!(!admit(&world.view(), 10, &repeated).unwrap());
    }
}
