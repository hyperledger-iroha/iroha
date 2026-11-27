//! Original-pool custody for physical carrier keys, records and current/undo map nodes.
//!
//! Immutable shared payloads keep their exact dynamic allocations alive through every map
//! copy. This owner grants memory custody only; selected invocation capabilities authorize
//! access separately. Prepared writes use the existing World transaction rollback owner.

use std::{alloc::Layout, borrow::Borrow, cmp::Ordering, fmt, ops::Deref};

use concread::bptree::{
    AllocationDemand, ClonePlanning, NodeCloning, NodeFunding, PlanningError, Prepaid,
};
use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationReservation, ChargedShared, PrepaidSharedError,
};
use iroha_data_model::{
    ValidationFail,
    runtime::carrier::{CarrierStorageKeyV1, MAX_CARRIER_FRAME_BYTES, MAX_CARRIER_KEY_BYTES},
};
use mv::storage::AdmittedStoragePolicy;

/// Physical custody refusal without inventing a source or pool release observation.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Protocol bounds or an internal ownership relation differs.
    #[error("carrier storage: {0}")]
    Invalid(&'static str),
    /// Original finite owner refused the complete requested payload/control layouts.
    #[error(transparent)]
    Admission(#[from] iroha_allocation::AllocationRefusal),
    /// Physical allocator refused its actual admitted layout.
    #[error(transparent)]
    Allocator(#[from] iroha_allocation::AllocatorRefusal),
}

impl Error {
    /// Preserve actual local refusal when crossing the VM-facing invocation boundary.
    pub(crate) fn into_vm_error(self) -> ivm::VMError {
        use crate::execution_attempt::ExecutionDeferred;
        match self {
            Self::Admission(original) => ExecutionDeferred::from(original).into_vm_error(),
            Self::Allocator(original) => ExecutionDeferred::from(original).into_vm_error(),
            Self::Invalid(_) => ivm::VMError::Trap(ivm::error::VmTrap::PermissionDenied),
        }
    }
}

// These concrete graphs contain only a Vec<u8>; dropping their payload cannot unwind or
// expose shared/interior mutable backing. Declaration order frees it before its reservation.
struct KeyPayload {
    value: CarrierStorageKeyV1,
    _backing: AllocationReservation,
}
struct ValuePayload {
    value: Vec<u8>,
    _backing: AllocationReservation,
}

/// Immutable physical key; clones share the original payload and control charge.
#[derive(Clone)]
pub struct RetainedCarrierKey(ChargedShared<KeyPayload>);
/// Immutable raw bytes; each outer storage owner validates its own record or code domain.
#[derive(Clone)]
pub struct RetainedCarrierBytes(ChargedShared<ValuePayload>);

fn shared(error: PrepaidSharedError) -> Error {
    match error {
        original @ PrepaidSharedError::Allocator { .. } => Error::Allocator(
            iroha_allocation::AllocatorRefusal::try_from(original)
                .expect("matched physical shared refusal"),
        ),
        PrepaidSharedError::Reservation(_) => {
            Error::Invalid("shared shell differs from its original reservation")
        }
    }
}
fn copy(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let layout = Layout::array::<u8>(bytes.len())
        .map_err(|_| Error::Invalid("payload allocation overflows"))?;
    let mut owned = Vec::new();
    owned.try_reserve_exact(bytes.len()).map_err(|original| {
        Error::Allocator(iroha_allocation::AllocatorRefusal::from_vec_reserve(
            original, layout,
        ))
    })?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

impl RetainedCarrierKey {
    /// Fund and copy the exact original bounded bare key, without granting access authority.
    pub(crate) fn admit(
        key: &CarrierStorageKeyV1,
        budget: &AllocationBudget,
    ) -> Result<Self, Error> {
        Self::admit_parts(key.storage, &key.key, budget)
    }
    pub(crate) fn admit_parts(
        storage: iroha_data_model::runtime::definition::DefinitionId,
        key: &[u8],
        budget: &AllocationBudget,
    ) -> Result<Self, Error> {
        if key.len() > MAX_CARRIER_KEY_BYTES {
            return Err(Error::Invalid("key exceeds protocol bound"));
        }
        let bytes = Layout::array::<u8>(key.len())
            .map_err(|_| Error::Invalid("key allocation overflows"))?;
        let mut backing = budget
            .try_reserve_layouts([bytes, ChargedShared::<KeyPayload>::allocation_layout()])?;
        let shell = ChargedShared::<KeyPayload>::reserve_from(&mut backing).map_err(shared)?;
        let value = CarrierStorageKeyV1 {
            storage,
            key: copy(key)?,
        };
        Ok(Self(shell.initialize(KeyPayload {
            value,
            _backing: backing,
        })))
    }
    /// Whether the complete immutable key belongs to this exact original pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.belongs_to(budget) && self.0._backing.belongs_to(budget)
    }
}
impl RetainedCarrierBytes {
    /// Fund the exact record bytes. Canonical schema validation belongs to the selected owner.
    pub(crate) fn admit(bytes: &[u8], budget: &AllocationBudget) -> Result<Self, Error> {
        let layout = Layout::array::<u8>(bytes.len())
            .map_err(|_| Error::Invalid("record allocation overflows"))?;
        let mut backing = budget
            .try_reserve_layouts([layout, ChargedShared::<ValuePayload>::allocation_layout()])?;
        let shell = ChargedShared::<ValuePayload>::reserve_from(&mut backing).map_err(shared)?;
        let value = copy(bytes)?;
        Ok(Self(shell.initialize(ValuePayload {
            value,
            _backing: backing,
        })))
    }
    /// Whether record bytes and their control belong to this exact original pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.0.belongs_to(budget) && self.0._backing.belongs_to(budget)
    }
}
impl Deref for RetainedCarrierKey {
    type Target = CarrierStorageKeyV1;
    fn deref(&self) -> &Self::Target {
        &self.0.value
    }
}
impl Borrow<CarrierStorageKeyV1> for RetainedCarrierKey {
    fn borrow(&self) -> &CarrierStorageKeyV1 {
        self
    }
}
impl Deref for RetainedCarrierBytes {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0.value
    }
}
impl fmt::Debug for RetainedCarrierKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl fmt::Debug for RetainedCarrierBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RetainedCarrierBytes")
            .field(&self.len())
            .finish()
    }
}
impl PartialEq for RetainedCarrierKey {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl Eq for RetainedCarrierKey {}
impl PartialOrd for RetainedCarrierKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RetainedCarrierKey {
    fn cmp(&self, other: &Self) -> Ordering {
        (**self).cmp(&**other)
    }
}
impl PartialEq for RetainedCarrierBytes {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl Eq for RetainedCarrierBytes {}

macro_rules! transparent_payload {
    ($retained:ty, $wire:ty) => {
        impl norito::SerializePayload for $retained {
            fn serialize(
                &self,
                encoder: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::Error> {
                norito::SerializePayload::serialize(&**self, encoder)
            }
            fn encoded_len_hint(&self) -> Option<usize> {
                norito::SerializePayload::encoded_len_hint(&**self)
            }
            fn encoded_len_exact(&self) -> Option<usize> {
                norito::SerializePayload::encoded_len_exact(&**self)
            }
        }
        impl norito::NoritoSchema for $retained {
            fn nominal_name() -> String {
                <$wire as norito::NoritoSchema>::nominal_name()
            }
            fn static_nominal_name() -> Option<&'static str> {
                <$wire as norito::NoritoSchema>::static_nominal_name()
            }
            fn frame_name() -> String {
                <$wire as norito::NoritoSchema>::frame_name()
            }
            fn static_frame_name() -> Option<&'static str> {
                <$wire as norito::NoritoSchema>::static_frame_name()
            }
        }
    };
}
transparent_payload!(RetainedCarrierKey, CarrierStorageKeyV1);
transparent_payload!(RetainedCarrierBytes, Vec<u8>);

/// Original prepaid map node policy; copying keys/values only clones admitted shared owners.
pub struct CarrierStoragePolicy(AllocationReservation);
impl NodeFunding for CarrierStoragePolicy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.0.try_split(layout).expect("admitted carrier map node")
    }
}
macro_rules! retained_copy {
    ($value:ty) => {
        impl NodeCloning<RetainedCarrierKey, $value> for CarrierStoragePolicy {
            fn clone_key(&mut self, key: &RetainedCarrierKey) -> RetainedCarrierKey {
                key.clone()
            }
            fn clone_value(&mut self, value: &$value) -> $value {
                value.clone()
            }
        }
        impl ClonePlanning<RetainedCarrierKey, $value> for CarrierStoragePolicy {
            fn plan_key(
                _: &RetainedCarrierKey,
                _: &mut AllocationDemand,
            ) -> Result<(), PlanningError> {
                Ok(())
            }
            fn plan_value(_: &$value, _: &mut AllocationDemand) -> Result<(), PlanningError> {
                Ok(())
            }
        }
    };
}
retained_copy!(RetainedCarrierBytes);
retained_copy!(Option<RetainedCarrierBytes>);
impl AdmittedStoragePolicy for CarrierStoragePolicy {
    fn from_admission(admission: AllocationReservation) -> Self {
        Self(admission)
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0
    }
}
/// Sole carrier table mode; both current and undo maps retain original allocation custody.
pub type CarrierStorageMode = Prepaid<CarrierStoragePolicy>;
/// Physical backing storage with no untracked payload or node variant.
pub type CarrierStorage =
    mv::storage::Storage<RetainedCarrierKey, RetainedCarrierBytes, CarrierStorageMode>;

/// Separate artifact map policy; fixed-size native keys and admitted immutable code bytes.
pub struct ArtifactStoragePolicy(AllocationReservation);
impl NodeFunding for ArtifactStoragePolicy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.0
            .try_split(layout)
            .expect("admitted artifact map node")
    }
}
macro_rules! artifact_copy {
    ($value:ty) => {
        impl NodeCloning<iroha_data_model::smart_contract::ContractArtifactId, $value>
            for ArtifactStoragePolicy
        {
            fn clone_key(
                &mut self,
                key: &iroha_data_model::smart_contract::ContractArtifactId,
            ) -> iroha_data_model::smart_contract::ContractArtifactId {
                *key
            }
            fn clone_value(&mut self, value: &$value) -> $value {
                value.clone()
            }
        }
        impl ClonePlanning<iroha_data_model::smart_contract::ContractArtifactId, $value>
            for ArtifactStoragePolicy
        {
            fn plan_key(
                _: &iroha_data_model::smart_contract::ContractArtifactId,
                _: &mut AllocationDemand,
            ) -> Result<(), PlanningError> {
                Ok(())
            }
            fn plan_value(_: &$value, _: &mut AllocationDemand) -> Result<(), PlanningError> {
                Ok(())
            }
        }
    };
}
artifact_copy!(RetainedCarrierBytes);
artifact_copy!(Option<RetainedCarrierBytes>);
impl AdmittedStoragePolicy for ArtifactStoragePolicy {
    fn from_admission(admission: AllocationReservation) -> Self {
        Self(admission)
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0
    }
}
/// Independent artifact current/undo map mode, owned by the new staging service.
pub type ArtifactStorageMode = Prepaid<ArtifactStoragePolicy>;
/// Exact data-space-scoped staged artifacts; never aliases the installed contract code table.
pub type ArtifactStorage = mv::storage::Storage<
    iroha_data_model::smart_contract::ContractArtifactId,
    RetainedCarrierBytes,
    ArtifactStorageMode,
>;

/// Transfer bounded code into the actual original-pool artifact table after the staging
/// handler has checked selected service, authority, scope, identity and executable profile.
pub(crate) fn stage_artifact(
    state: &mut super::StateTransaction<'_, '_>,
    artifact_id: iroha_data_model::smart_contract::ContractArtifactId,
    bytes: &[u8],
) -> Result<(), ValidationFail> {
    if bytes.len() > iroha_data_model::isi::runtime_carrier::MAX_CARRIER_ARTIFACT_BYTES_V1 {
        return Err(ValidationFail::NotPermitted(
            "carrier artifact exceeds its exact staging bound".into(),
        ));
    }
    let owner = RetainedCarrierBytes::admit(
        bytes,
        state.world.runtime_generic_artifacts.allocation_budget(),
    )
    .map_err(|original| {
        state.vm_error_to_validation_fail(original.into_vm_error(), |error| {
            ValidationFail::NotPermitted(error.to_string())
        })
    })?;
    match state
        .world
        .runtime_generic_artifacts
        .try_insert_admitted(artifact_id, owner)
    {
        Ok(_) => Ok(()),
        Err((_, original)) => {
            state.arm_local_storage_refusal(super::StateStorageAdmissionError::World(original));
            Err(ValidationFail::NotPermitted(
                "original carrier artifact storage admission did not complete".into(),
            ))
        }
    }
}

/// Admitted immutable inputs for one existing ordered host write; no access authority implied.
#[derive(Debug)]
pub(crate) struct PreparedWrite {
    key: RetainedCarrierKey,
    value: Option<RetainedCarrierBytes>,
}

/// Prepare payload custody before any ordered write is applied.
pub(crate) fn prepare(
    key: &CarrierStorageKeyV1,
    value: Option<&[u8]>,
    budget: &AllocationBudget,
) -> Result<PreparedWrite, Error> {
    if value.is_some_and(|bytes| bytes.len() > MAX_CARRIER_FRAME_BYTES) {
        return Err(Error::Invalid("record exceeds protocol bound"));
    }
    let key = RetainedCarrierKey::admit(key, budget)?;
    let value = value
        .map(|value| RetainedCarrierBytes::admit(value, budget))
        .transpose()?;
    Ok(PreparedWrite { key, value })
}

impl PreparedWrite {
    /// Apply through the original admitted current/undo transaction. A local map refusal arms
    /// the enclosing State against publication, so the entire ordinary transaction rolls back.
    pub(crate) fn apply(
        self,
        state: &mut super::StateTransaction<'_, '_>,
    ) -> Result<(), ValidationFail> {
        let budget = state.world.runtime_generic_values.allocation_budget();
        if !self.key.belongs_to(budget)
            || self
                .value
                .as_ref()
                .is_some_and(|value| !value.belongs_to(budget))
        {
            state.arm_local_storage_refusal(super::StateStorageAdmissionError::World(
                mv::storage::AdmittedStorageError::PolicyIdentity,
            ));
            return Err(ValidationFail::NotPermitted(
                "carrier payload belongs to a different original pool".into(),
            ));
        }
        let result = if let Some(value) = self.value {
            state
                .world
                .runtime_generic_values
                .try_insert_admitted(self.key, value)
                .map_err(|(_, original)| original)
        } else {
            state
                .world
                .runtime_generic_values
                .try_remove_admitted(self.key)
                .map_err(|(_, original)| original)
        };
        match result {
            Ok(_) => Ok(()),
            Err(original) => {
                state.arm_local_storage_refusal(super::StateStorageAdmissionError::World(original));
                Err(ValidationFail::NotPermitted(
                    "original carrier storage admission did not complete".into(),
                ))
            }
        }
    }
}

mod snapshot_json_impl;
pub(crate) use snapshot_json_impl::{
    artifact_snapshot_block_json, artifact_snapshot_json, snapshot_block_json, snapshot_json,
};

#[cfg(test)]
mod fixture;
#[cfg(test)]
pub(crate) use fixture::CarrierFixtureStorage;
#[cfg(test)]
mod tests;
