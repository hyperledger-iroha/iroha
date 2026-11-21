//! Selection-aware release support without treating published carrier IDs as native catalog IDs.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};

/// An incomplete or unsupported descriptor-language support observation.
/// Local codec refusal retains its original provenance until its caller chooses a retry owner.
#[derive(Debug, thiserror::Error)]
pub enum ReleaseSupportError {
    /// The published selection does not satisfy the compiled carrier contract.
    #[error("runtime support contract: {0}")]
    Contract(&'static str),
    /// An original codec failure, including an enclosing local resource refusal.
    #[error("runtime support codec: {0}")]
    Codec(#[source] norito::core::DecodeAttemptError),
    /// An ordinary selected descriptor rejection remains typed.
    #[error("runtime support descriptor: {0}")]
    Rejected(#[source] iroha_data_model::isi::error::InstructionExecutionError),
}
impl ReleaseSupportError {
    /// Preserve the original local retry owner; only completed observations become rejections.
    pub fn into_attempt(self) -> ExecutionAttemptError<Self> {
        match self {
            Self::Codec(original) => canonical_decode_attempt_error(original, Self::Codec),
            original => ExecutionAttemptError::Rejected(original),
        }
    }
    pub(crate) fn into_block_start(self, height: u64) -> crate::state::StateStorageAdmissionError {
        let original = match self {
            Self::Codec(original) => match original.into_local_refusal() {
                Ok(original) => {
                    return crate::state::StateStorageAdmissionError::RuntimeSupportDecode(
                        original,
                    );
                }
                Err(original) => Self::Codec(original),
            },
            original => original,
        };
        crate::state::StateStorageAdmissionError::UnsupportedRuntime {
            height,
            reason: original.to_string(),
        }
    }
}

/// Whether this release executes an original selection with its actual published definitions.
///
/// Native roots retain the exact native catalog check. The separate carrier kernel first
/// retains every explicit compiled qualification refusal, then checks all selected dynamic
/// declarations through the shared capability validator. Published IDs alone confer no support.
///
/// # Errors
/// A missing/substituted descriptor, unsupported carrier contract, or original codec refusal.
pub fn release_support_with_definitions(
    catalog: &ReleaseCatalog,
    selection: &RootSelectionV1,
    published: &impl DefinitionLookup,
) -> Result<ReleaseSupport, ReleaseSupportError> {
    if !catalog
        .generic_carriers
        .is_some_and(|profile| profile.kernel == selection.kernel)
    {
        return Ok(release_support(catalog, selection));
    }
    // A dynamic declaration may be interpreted by the selected kernel. A compiled definition
    // explicitly withheld from qualification may not bypass its existing release guard.
    if let Some(id) = [
        selection.kernel,
        selection.commitment_profile,
        selection.services_profile,
    ]
    .into_iter()
    .chain(selection.installed_storage.iter().copied())
    .chain(selection.active.iter().copied())
    .find(|id| catalog.definitions.contains_key(id) && !catalog.executes_definition(id))
    {
        return Ok(ReleaseSupport::Missing(id));
    }
    super::super::generic_carriers::capability::validate_compiled_selection(
        catalog, selection, published,
    )
    .map_err(|error| match error {
        super::super::generic_carriers::Error::Codec(original) => {
            ReleaseSupportError::Codec(original)
        }
        super::super::generic_carriers::Error::Contract(reason) => {
            ReleaseSupportError::Contract(reason)
        }
        super::super::generic_carriers::Error::Rejected(original) => {
            ReleaseSupportError::Rejected(original)
        }
    })?;
    Ok(ReleaseSupport::Supported)
}

/// Check the committed selection against the same World's original published definition table.
///
/// # Errors
/// The original selection-aware support check refuses; no caller-supplied ID inventory is used.
pub fn committed_release_support(
    catalog: &ReleaseCatalog,
    world: &impl WorldReadOnly,
) -> Result<ReleaseSupport, ReleaseSupportError> {
    release_support_with_definitions(
        catalog,
        world.runtime_selection(),
        &PublishedDefinitions(world.runtime_definitions()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original_refusal() -> norito::core::DecodeAttemptError {
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX);
        norito::with_decode_limits_scope(limits, || {
            norito::core::classify_decode_attempt(|| {
                norito::core::read_seq_len_slice(&1_u64.to_le_bytes())
            })
            .unwrap_err()
        })
    }

    #[test]
    fn native_support_is_exact_and_new_profile_qualification_cannot_be_claimed() {
        let catalog = release_catalog().unwrap();
        assert_eq!(
            release_support_with_definitions(catalog, &catalog.baseline, &catalog.definitions)
                .unwrap(),
            release_support(catalog, &catalog.baseline)
        );
        let mut unknown = catalog.baseline.clone();
        unknown.kernel = DefinitionId([0xee; 32]);
        assert_eq!(
            release_support_with_definitions(catalog, &unknown, &catalog.definitions).unwrap(),
            release_support(catalog, &unknown)
        );
        let profile = catalog.generic_carriers.unwrap();
        let mut selected = catalog.baseline.clone();
        selected.kernel = profile.kernel;
        selected.commitment_profile = profile.commitment;
        selected.active.push(profile.host);
        selected.active.sort();
        selected.installed_storage.extend([
            profile.backing,
            catalog.world_storage["world.runtime_generic_artifacts"],
        ]);
        selected.installed_storage.sort();
        selected.check_closure(&catalog.definitions).unwrap();
        assert_eq!(
            release_support_with_definitions(catalog, &selected, &catalog.definitions).unwrap(),
            ReleaseSupport::Missing(profile.kernel)
        );
    }

    #[test]
    fn original_decode_refusal_moves_to_attempt_and_block_stop_without_reclassification() {
        let original = original_refusal();
        let expected = original.local_refusal().unwrap();
        let ExecutionAttemptError::Deferred(actual) =
            ReleaseSupportError::Codec(original).into_attempt()
        else {
            panic!("original local codec refusal")
        };
        assert_eq!(actual.decode_refusal(), Some(&expected));
        let original = original_refusal();
        let expected = original.local_refusal().unwrap();
        let actual = ReleaseSupportError::Codec(original).into_block_start(17);
        assert_eq!(
            actual,
            crate::state::StateStorageAdmissionError::RuntimeSupportDecode(expected)
        );
        assert!(
            actual.release_wait().is_none(),
            "codec refusal cannot invent physical release custody"
        );
        assert!(matches!(
            ReleaseSupportError::Contract("different selected contract").into_attempt(),
            ExecutionAttemptError::Rejected(ReleaseSupportError::Contract(
                "different selected contract"
            ))
        ));
    }
}
