//! Original-codec-refusal identities for newly selected lifecycle owners.

use std::cell::RefCell;

use super::*;
use norito::core::DecodeAttemptError;

/// A deterministic transition rejection or the original canonical codec refusal.
#[derive(Debug, thiserror::Error)]
pub enum CheckedChangeSetErrorV1 {
    /// The existing deterministic transition engine rejected the change.
    #[error("{0}")]
    Rejected(ChangeSetError),
    /// The exact encoding attempt failed; the caller preserves its local classification.
    #[error("{0}")]
    Codec(DecodeAttemptError),
}

fn identity<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        let mut encoded = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(domain)?;
            encoded = norito::core::write_canonical_to_writer(value, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(hash.into())
    })
}

/// The existing change-set identity, streamed without losing the original codec error.
///
/// # Errors
/// The original canonical writer refusal.
pub fn checked_change_set_id_v1(value: &ChangeSetV1) -> Result<ChangeSetId, DecodeAttemptError> {
    identity(CHANGE_SET_ID_DOMAIN, value).map(ChangeSetId)
}

/// The existing selection identity, streamed without losing the original codec error.
///
/// # Errors
/// The original canonical writer refusal.
pub fn checked_selection_id_v1(value: &RootSelectionV1) -> Result<SelectionId, DecodeAttemptError> {
    identity(super::super::definition::SELECTION_ID_DOMAIN, value).map(SelectionId)
}

impl ChangeSetV1 {
    /// Apply the existing transition rules while retaining original identity codec refusals.
    /// This entrypoint is for new selected owners; the installed `apply` contract is unchanged.
    ///
    /// # Errors
    /// A deterministic transition rejection or its original canonical identity refusal.
    pub fn apply_checked_v1(
        &self,
        predecessor: &RootSelectionV1,
        published: &impl DefinitionLookup,
    ) -> Result<AppliedChangeSet, CheckedChangeSetErrorV1> {
        let original = RefCell::new(None);
        let retain = |error| {
            *original.borrow_mut() = Some(error);
            ChangeSetError::Encoding("original refusal retained by checked lifecycle owner".into())
        };
        let result = self.apply_with_identities(
            predecessor,
            published,
            |selection| checked_selection_id_v1(selection).map_err(&retain),
            |definition| {
                definition.validate()?;
                super::super::carrier::carrier_definition_id_v1(definition).map_err(&retain)
            },
            |change, previous| {
                if !change.transitions.iter().any(|step| {
                    *step
                        == SelectionTransitionV1::SetCommitmentProfile(
                            super::super::keyed_state::profile_id(),
                        )
                }) {
                    return Ok(false);
                }
                let mut storage: Vec<_> = change
                    .transitions
                    .iter()
                    .filter_map(|step| match step {
                        SelectionTransitionV1::InstallStorage(id) => Some(*id),
                        _ => None,
                    })
                    .collect();
                storage.sort_unstable();
                if storage.is_empty() {
                    return Ok(false);
                }
                super::super::carrier::carrier_definition_id_v1(
                    &super::super::keyed_state::adoption_kernel(&storage),
                )
                .map(|id| id == previous)
                .map_err(&retain)
            },
        );
        match original.into_inner() {
            Some(error) => Err(CheckedChangeSetErrorV1::Codec(error)),
            None => result.map_err(CheckedChangeSetErrorV1::Rejected),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_identities_preserve_installed_canonical_preimages() {
        let selection = RootSelectionV1 {
            kernel: DefinitionId([1; 32]),
            commitment_profile: DefinitionId([2; 32]),
            services_profile: DefinitionId([3; 32]),
            installed_storage: Vec::new(),
            active: Vec::new(),
            bindings: Vec::new(),
            restrictions: Vec::new(),
            completion_closures: Vec::new(),
        };
        assert_eq!(
            checked_selection_id_v1(&selection).unwrap(),
            selection.id().unwrap()
        );
        let change = ChangeSetV1 {
            network: crate::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([9; 32])),
            ),
            root: RootScopeV1::Universal,
            predecessor: selection.id().unwrap(),
            successor: selection.id().unwrap(),
            effective_height: 7,
            publish: Vec::new(),
            transitions: Vec::new(),
            qualification: [8; 32],
        };
        assert_eq!(
            checked_change_set_id_v1(&change).unwrap(),
            change.id().unwrap()
        );
        let lookup = BTreeMap::<DefinitionId, DefinitionDescriptorV1>::new();
        let old = change.apply(&selection, &lookup).unwrap_err();
        let new = change.apply_checked_v1(&selection, &lookup).unwrap_err();
        assert!(matches!(new, CheckedChangeSetErrorV1::Rejected(error) if error == old));
    }
}
