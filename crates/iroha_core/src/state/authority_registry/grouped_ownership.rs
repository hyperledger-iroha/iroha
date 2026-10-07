//! Bounded exact grouping checks over original native current and undo maps.
//!
//! Asset, NFT, RWA, escrow and repo captures retain the same source and index
//! readers through row encoding. This supplies no finalized State authority.
//! TODO: consume all derived checks in complete State/Kura publication.

use crate::state::World;
use iroha_data_model::{
    account::AccountId,
    nft::{NftId, NftValue},
    rwa::{RwaId, RwaValue},
};
use iroha_model_base::{domain::DomainId, name::Name};
#[cfg(test)]
use mv::storage::StorageReadOnly;
use mv::{PublicationPreparationError, storage::CommittedStorageView};
use std::{collections::BTreeSet, convert::Infallible};

mod escrows;
#[cfg(test)]
pub(in crate::state) use escrows::test_support as escrow_test_support;
pub(in crate::state) use escrows::validate_original_escrows;
pub(super) use escrows::{CheckedEscrows, ESCROW_WORK_PER_ROW};
mod repo_agreements;
#[cfg(test)]
pub(in crate::state) use repo_agreements::test_support as repo_agreement_test_support;
pub(in crate::state) use repo_agreements::validate_original_repo_agreements;
pub(super) use repo_agreements::{CheckedRepoAgreements, REPO_AGREEMENT_WORK_PER_ROW};
mod asset_definitions;
#[cfg(test)]
pub(in crate::state) use asset_definitions::test_support as asset_definition_test_support;
pub(in crate::state) use asset_definitions::validate_original_asset_definitions;
pub(super) use asset_definitions::{ASSET_DEFINITION_WORK_PER_ROW, CheckedAssetDefinitions};
mod assets;
mod confidential_policies;
#[cfg(test)]
pub(in crate::state) use assets::test_support as asset_balance_test_support;
pub(in crate::state) use assets::validate_original_assets;
pub(super) use assets::{ASSET_BALANCE_WORK_PER_ROW, CheckedAssets};
mod contract_aliases;
#[cfg(test)]
pub(in crate::state) use contract_aliases::test_support as contract_alias_test_support;
pub(in crate::state) use contract_aliases::validate_original_contract_aliases;
pub(super) use contract_aliases::{CONTRACT_ALIAS_WORK_PER_ROW, CheckedContractAliases};
mod account_rekeys;
#[cfg(test)]
pub(in crate::state) use account_rekeys::test_support as account_rekey_test_support;
pub(in crate::state) use account_rekeys::validate_original_account_rekeys;
pub(super) use account_rekeys::{ACCOUNT_REKEY_WORK_PER_ROW, CheckedAccountRekeys};
mod validation_fee_proposals;
#[cfg(test)]
pub(in crate::state) use validation_fee_proposals::test_support as validation_fee_proposal_test_support;
pub(in crate::state) use validation_fee_proposals::validate_original_validation_fee_proposals;
pub(super) use validation_fee_proposals::{
    CheckedValidationFeeProposals, VALIDATION_FEE_PROPOSAL_WORK_PER_ROW,
};
mod proof_status;
pub(super) use proof_status::CheckedProofRecords;
pub(in crate::state) use proof_status::validate_original_proofs;
mod contract_subjects;
pub(super) use contract_subjects::CheckedContractSubjects;
pub(in crate::state) use contract_subjects::validate_original_contract_subjects;
mod verifying_keys;
pub(super) use verifying_keys::CheckedVerifyingKeys;

/// Original native image whose exact grouping is being checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupImage {
    /// Published rows.
    Current,
    /// Exact logical predecessor reconstructed from original undo entries.
    Predecessor,
}

/// Failure of one exact grouping relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupMismatch {
    /// A canonical source row is missing from its expected group.
    MissingMember,
    /// A materialized group has no members.
    EmptyGroup,
    /// A group contains an absent source or a member belonging to another group.
    ForeignMember,
}

/// Local work/publication refusals remain distinct from inconsistent indexes.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum GroupedOwnershipError {
    /// Retry locally with sufficient admitted work; no validity verdict follows.
    #[error("grouped ownership capture exceeds its local work bound")]
    WorkLimit,
    /// The original allocation pool refused exact registry backing before construction.
    #[error("direct-home capture allocation admission failed: {0}")]
    Admission(iroha_allocation::AllocationRefusal),
    /// An admitted registry backing could not be physically allocated; no release wait is invented.
    #[error("direct-home capture allocator refused {requested_bytes:?} bytes")]
    Allocation {
        /// Actual requested bytes when retained by the originating allocator.
        requested_bytes: Option<u64>,
    },
    /// An exact original prepaid partition was insufficient; the pool was not reacquired.
    #[error("direct-home capture prepaid partition is insufficient: {0}")]
    Reservation(iroha_allocation::InsufficientReservation),
    /// Preserve the original enclosing codec resource refusal and its scope identity.
    #[error("direct-home capture retained a codec resource refusal: {0}")]
    DecodeResource(norito::core::ScopedDecodeResourceError),
    /// A local codec ceiling refused inspection without proving invalid source state.
    #[error("direct-home capture exceeds its enclosing codec inspection bound")]
    DecodeLimit,
    /// Canonical rows violate a reference required by their derived projection.
    #[error("grouped ownership source {table} in {image:?}: {reason}")]
    Source {
        /// Canonical inventory identity of the source.
        table: &'static str,
        /// Failed current or predecessor image.
        image: GroupImage,
        /// Fixed allocation-free explanation of the invalid source reference.
        reason: &'static str,
    },
    /// The original source and derived membership disagree.
    #[error("grouped ownership mismatch for {index} in {image:?}: {mismatch:?}")]
    Corrupt {
        /// Canonical inventory identity of the derived index.
        index: &'static str,
        /// Failed current or predecessor image.
        image: GroupImage,
        /// Failed exact relation.
        mismatch: GroupMismatch,
    },
    /// Preserve the exact original Cell retention refusal without a ledger-validity verdict.
    #[error("grouped ownership parameter cell could not be retained: {0:?}")]
    Cell(mv::cell::CommittedCellReadError),
    /// Preserve the actual native publication refusal or identity change.
    #[error("grouped ownership source could not be retained: {0:?}")]
    Publication(PublicationPreparationError<Infallible>),
}

impl From<PublicationPreparationError<Infallible>> for GroupedOwnershipError {
    fn from(error: PublicationPreparationError<Infallible>) -> Self {
        Self::Publication(error)
    }
}

/// Temporary exact original-pool backing for the two authoritative parameter images.
/// Each fixed binding contains only inline values. The buffers drop their real backing
/// before refund; no decoded map, serializer scratch, or unowned counter escapes.
struct AdmittedAssetHomeImages {
    images: [Option<
        iroha_allocation::ChargedBuffer<iroha_data_model::asset::AssetDefinitionDataspaceBindingV1>,
    >; 2],
}

impl AdmittedAssetHomeImages {
    fn capture(
        parameters: [&iroha_data_model::parameter::Parameters; 2],
        budget: &iroha_allocation::AllocationBudget,
        mut prepay: impl FnMut(usize) -> Result<(), GroupedOwnershipError>,
    ) -> Result<Self, GroupedOwnershipError> {
        use iroha_data_model::asset::AssetDefinitionDataspaceBindingV1;
        let plans = [
            asset_home_read_plan(parameters[0], GroupImage::Current, &mut prepay)?,
            asset_home_read_plan(parameters[1], GroupImage::Predecessor, &mut prepay)?,
        ];
        if plans.iter().all(Option::is_none) {
            return Ok(Self {
                images: [None, None],
            });
        }
        let counts = plans
            .each_ref()
            .map(|plan| plan.as_ref().map_or(0, |plan| plan.binding_count()));
        let layouts = counts.map(|count| {
            std::alloc::Layout::array::<AssetDefinitionDataspaceBindingV1>(count).map_err(|_| {
                GroupedOwnershipError::Admission(
                    iroha_allocation::AllocationRefusal::DemandOverflow,
                )
            })
        });
        let [current_layout, previous_layout] = layouts;
        // One original reservation covers both real layouts before either allocation.
        let mut reservation = budget
            .try_reserve_layouts([current_layout?, previous_layout?])
            .map_err(GroupedOwnershipError::Admission)?;
        let mut images = [None, None];
        for (index, (plan, image)) in plans
            .into_iter()
            .zip([GroupImage::Current, GroupImage::Predecessor])
            .enumerate()
        {
            if let Some(plan) = plan {
                let mut bindings = iroha_allocation::ChargedBuffer::from_reservation(
                    plan.binding_count(),
                    &mut reservation,
                )
                .map_err(registry_buffer_error)?;
                plan.decode_into(&mut bindings)
                    .map_err(|error| registry_read_error(error, image))?;
                images[index] = Some(bindings);
            }
        }
        Ok(Self { images })
    }

    fn bindings(
        &self,
        image: GroupImage,
    ) -> &[iroha_data_model::asset::AssetDefinitionDataspaceBindingV1] {
        self.images[usize::from(image == GroupImage::Predecessor)]
            .as_ref()
            .map_or(&[], |rows| rows.as_slice())
    }

    fn get(
        &self,
        image: GroupImage,
        id: &iroha_data_model::asset::AssetDefinitionId,
    ) -> Option<&iroha_data_model::asset::AssetDefinitionDataspaceBindingV1> {
        let rows = self.bindings(image);
        rows.binary_search_by(|binding| binding.asset_definition_id.cmp(id))
            .ok()
            .map(|index| &rows[index])
    }

    fn validate_transition(&self) -> Result<(), GroupedOwnershipError> {
        for previous in self.bindings(GroupImage::Predecessor) {
            crate::state::validate_asset_definition_home_transition(
                previous,
                self.get(GroupImage::Current, &previous.asset_definition_id),
            )
            .map_err(|_| GroupedOwnershipError::Source {
                table: "world.parameters",
                image: GroupImage::Current,
                reason: "direct-home registry changed immutable predecessor authority",
            })?;
        }
        Ok(())
    }
}

fn asset_home_read_plan<'a>(
    parameters: &'a iroha_data_model::parameter::Parameters,
    image: GroupImage,
    prepay: &mut impl FnMut(usize) -> Result<(), GroupedOwnershipError>,
) -> Result<
    Option<iroha_data_model::asset::AssetDefinitionDataspaceRegistryReadPlan<'a>>,
    GroupedOwnershipError,
> {
    for id in parameters.custom().keys() {
        prepay(id.name().as_ref().len().saturating_mul(2))?;
    }
    let Some(parameter) = crate::state::asset_definition_registry_parameter(parameters) else {
        return Ok(None);
    };
    prepay(parameter.payload().get().len())?;
    iroha_data_model::asset::AssetDefinitionDataspaceRegistryReadPlan::from_custom_parameter(
        parameter,
    )
    .map_err(|error| registry_read_error(error, image))?
    .map(Some)
    .ok_or(GroupedOwnershipError::Source {
        table: "world.parameters",
        image,
        reason: "protected direct-home parameter identity differs",
    })
}

fn registry_buffer_error(error: iroha_allocation::PrepaidBufferError) -> GroupedOwnershipError {
    use iroha_allocation::{ChargedBufferError, PrepaidBufferError};
    match error {
        PrepaidBufferError::Reservation(original) => GroupedOwnershipError::Reservation(original),
        PrepaidBufferError::Allocation(ChargedBufferError::Admission(original)) => {
            GroupedOwnershipError::Admission(original)
        }
        PrepaidBufferError::Allocation(ChargedBufferError::Allocator { requested_bytes }) => {
            GroupedOwnershipError::Allocation {
                requested_bytes: Some(requested_bytes as u64),
            }
        }
    }
}

fn registry_read_error(error: norito::json::Error, image: GroupImage) -> GroupedOwnershipError {
    use norito::json::Error;
    match error {
        Error::ScopedDecodeResource(original) => GroupedOwnershipError::DecodeResource(original),
        Error::DecodeResourceLimit | Error::NestingDepthExceeded { .. } => {
            GroupedOwnershipError::DecodeLimit
        }
        Error::DecodeAllocationFailed { bytes } => GroupedOwnershipError::Allocation {
            requested_bytes: Some(bytes),
        },
        Error::AllocationFailed => GroupedOwnershipError::Allocation {
            requested_bytes: None,
        },
        _ => GroupedOwnershipError::Source {
            table: "world.parameters",
            image,
            reason: "invalid protected direct-home registry",
        },
    }
}

struct Work(u64);

impl Work {
    fn charge(&mut self) -> Result<(), GroupedOwnershipError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(GroupedOwnershipError::WorkLimit)?;
        Ok(())
    }
}

#[cfg(test)]
fn get_at<'a, K: mv::Key, V: mv::Value>(
    view: &'a CommittedStorageView<'_, K, V>,
    image: GroupImage,
    key: &K,
) -> Option<&'a V> {
    if image == GroupImage::Predecessor {
        if let Some(prior) = view.undo().get(key) {
            return prior.as_ref();
        }
    }
    view.current().get(key)
}

mod nfts_rwas;
#[cfg(test)]
pub(in crate::state) use nfts_rwas::test_support as nft_rwa_test_support;
pub(super) use nfts_rwas::{CheckedNfts, CheckedRwas, NFT_WORK_PER_ROW, RWA_WORK_PER_ROW};
pub(in crate::state) use nfts_rwas::{validate_original_nfts, validate_original_rwas};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod admitted_asset_home_tests {
    use super::*;
    use crate::test_allocations::allocations_during;
    use iroha_allocation::{AllocationBudget, AllocationRefusal};
    use iroha_data_model::{
        asset::{
            AssetDefinitionDataspaceBindingV1, AssetDefinitionDataspaceRegistryV1,
            AssetDefinitionId,
        },
        nexus::AxtAssetIncarnationV1,
        parameter::{Parameter, Parameters},
    };
    use iroha_model_base::topology::DataSpaceId;
    use std::alloc::Layout;

    fn parameters(active: bool, incarnation_byte: u8, dataspace: u64) -> Parameters {
        let id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("chargedhomes", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let binding = AssetDefinitionDataspaceBindingV1 {
            asset_definition_id: id.clone(),
            incarnation: AxtAssetIncarnationV1::try_from_bytes(
                iroha_crypto::Hash::new([incarnation_byte]).into(),
            )
            .unwrap(),
            dataspace_id: DataSpaceId::new(dataspace),
            active,
        };
        let registry = AssetDefinitionDataspaceRegistryV1 {
            version: AssetDefinitionDataspaceRegistryV1::VERSION,
            bindings: [(id, binding)].into_iter().collect(),
        };
        let mut parameters = Parameters::default();
        parameters.set_parameter(Parameter::Custom(registry.into_custom_parameter().unwrap()));
        parameters
    }

    fn layout() -> Layout {
        Layout::array::<AssetDefinitionDataspaceBindingV1>(1).unwrap()
    }

    #[test]
    fn absent_authority_never_allocates_or_reserves() {
        let parameters = Parameters::default();
        let budget = AllocationBudget::new(0);
        assert_eq!(
            allocations_during(|| {
                let images =
                    AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| {
                        Ok(())
                    })
                    .unwrap();
                assert!(images.bindings(GroupImage::Current).is_empty());
                assert!(images.bindings(GroupImage::Predecessor).is_empty());
                images.validate_transition().unwrap();
            }),
            0
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn both_images_are_admitted_before_any_backing_allocation() {
        let parameters = parameters(true, 1, 7);
        let original = parameters.clone();
        let demand = 2 * layout().size();
        let budget = AllocationBudget::new(demand - 1);
        let mut error = None;
        assert_eq!(
            allocations_during(|| {
                error =
                    AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| {
                        Ok(())
                    })
                    .err();
            }),
            0
        );
        assert_eq!(
            error,
            Some(GroupedOwnershipError::Admission(
                AllocationRefusal::ExceedsLimit {
                    requested_bytes: demand,
                    limit_bytes: demand - 1,
                }
            ))
        );
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(parameters, original);
    }

    #[test]
    fn capacity_refusal_preserves_original_release_observation_and_retry() {
        let parameters = parameters(true, 1, 7);
        let demand = 2 * layout().size();
        let budget = AllocationBudget::new(demand);
        let held = budget.try_reserve(Layout::new::<u8>()).unwrap();
        let expected = budget
            .try_reserve_layouts([layout(), layout()])
            .unwrap_err();
        assert!(matches!(expected, AllocationRefusal::Capacity { .. }));
        let error =
            AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| Ok(()))
                .err()
                .unwrap();
        assert_eq!(error, GroupedOwnershipError::Admission(expected));
        assert_eq!(budget.reserved_bytes(), 1);
        drop(held);
        let mut captured = None;
        assert_eq!(
            allocations_during(|| {
                captured = Some(
                    AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| {
                        Ok(())
                    })
                    .unwrap(),
                );
            }),
            2,
            "only the two admitted fixed binding buffers allocate"
        );
        let images = captured.unwrap();
        assert_eq!(budget.reserved_bytes(), demand);
        assert!(
            images
                .images
                .iter()
                .flatten()
                .all(|rows| rows.belongs_to(&budget))
        );
        images.validate_transition().unwrap();
        drop(images);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn physical_backing_refusal_refunds_the_original_reservation() {
        let parameters = parameters(true, 1, 7);
        let original = parameters.clone();
        let budget = AllocationBudget::new(2 * layout().size());
        let (error, refused) = crate::test_allocations::refuse_one_layout_during(layout(), || {
            AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| Ok(())).err()
        });
        assert!(refused);
        assert_eq!(
            error,
            Some(GroupedOwnershipError::Allocation {
                requested_bytes: Some(layout().size() as u64),
            })
        );
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(parameters, original);
        drop(
            AdmittedAssetHomeImages::capture([&parameters, &parameters], &budget, |_| Ok(()))
                .unwrap(),
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn original_transition_checks_retain_tombstones_and_allow_new_incarnations() {
        let before = parameters(true, 1, 7);
        let tombstone = parameters(false, 1, 7);
        let recreated = parameters(true, 2, 9);
        let rebound = parameters(true, 1, 9);
        let absent = Parameters::default();
        let budget = AllocationBudget::new(2 * layout().size());
        for (current, previous, valid) in [
            (&tombstone, &before, true),
            (&recreated, &tombstone, true),
            (&before, &tombstone, false),
            (&rebound, &before, false),
            (&absent, &before, false),
        ] {
            let images =
                AdmittedAssetHomeImages::capture([current, previous], &budget, |_| Ok(())).unwrap();
            assert_eq!(images.validate_transition().is_ok(), valid);
            drop(images);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }

    #[test]
    fn local_codec_refusals_are_not_source_invalidity() {
        assert_eq!(
            registry_read_error(
                norito::json::Error::DecodeResourceLimit,
                GroupImage::Current
            ),
            GroupedOwnershipError::DecodeLimit
        );
        assert_eq!(
            registry_read_error(
                norito::json::Error::DecodeAllocationFailed { bytes: 37 },
                GroupImage::Current
            ),
            GroupedOwnershipError::Allocation {
                requested_bytes: Some(37)
            }
        );
        assert_eq!(
            registry_read_error(
                norito::json::Error::AllocationFailed,
                GroupImage::Predecessor
            ),
            GroupedOwnershipError::Allocation {
                requested_bytes: None
            }
        );
        assert_eq!(
            registry_buffer_error(iroha_allocation::PrepaidBufferError::Allocation(
                iroha_allocation::ChargedBufferError::Admission(AllocationRefusal::DemandOverflow)
            )),
            GroupedOwnershipError::Admission(AllocationRefusal::DemandOverflow)
        );
    }
}
