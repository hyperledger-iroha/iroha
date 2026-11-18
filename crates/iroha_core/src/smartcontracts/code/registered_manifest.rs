//! Family-preserving access to registered, authenticated contract manifests.
use std::borrow::Cow;

use crate::{smartcontracts::ivm::profile::ResolvedIvmProfile, state::WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    ivm_c10::ContractManifestC10V1,
    smart_contract::{
        ContractArtifactId,
        manifest::{
            ContractEventDescriptorV1, ContractManifest, ContractPermissionDescriptorV1,
            EntryPointKind, EntrypointAuthorizationV1,
        },
    },
    trigger::TriggerId,
};
use mv::storage::StorageReadOnly as _;

/// The manifest in the table committed for this artifact's registration interface.
pub(crate) enum RegisteredManifest<'a> {
    Baseline(Cow<'a, ContractManifest>),
    C10(Cow<'a, ContractManifestC10V1>),
}

impl<'a> RegisteredManifest<'a> {
    /// An artifact has exactly one registration family. A partial or conflicting binding is invalid.
    pub(crate) fn get(world: &'a impl WorldReadOnly, key: &ContractArtifactId) -> Option<Self> {
        let baseline = world.contract_manifests().get(key);
        let c10 = world.c10_contract_manifests().get(key);
        let binding = world.c10_contract_profiles().get(key);
        match (baseline, c10, binding) {
            (Some(m), None, None) => Some(Self::Baseline(Cow::Borrowed(m))),
            (None, Some(m), Some(id))
                if *id
                    == ResolvedIvmProfile::of_id(ivm::IvmProfileId::C10_ABI_V1).definition_id() =>
            {
                Some(Self::C10(Cow::Borrowed(m)))
            }
            _ => None,
        }
    }

    /// Resolve the retained family independently of the active selection and VM cache.
    pub(crate) fn captured_c10(
        world: &impl WorldReadOnly,
        key: &ContractArtifactId,
    ) -> Result<bool, iroha_data_model::ValidationFail> {
        if world.c10_contract_profiles().get(key).is_none()
            && world.c10_contract_manifests().get(key).is_none()
        {
            return Ok(false);
        }
        if matches!(
            RegisteredManifest::get(world, key),
            Some(RegisteredManifest::C10(_))
        ) {
            Ok(true)
        } else {
            Err(iroha_data_model::ValidationFail::NotPermitted(
                "contract registration has a conflicting or incomplete C10 host binding".into(),
            ))
        }
    }

    /// Shared permission declarations of the original signed manifest.
    pub(crate) fn permissions(&self) -> &[ContractPermissionDescriptorV1] {
        match self {
            Self::Baseline(m) => &m.permissions,
            Self::C10(m) => &m.permissions,
        }
    }

    /// Kind and authorization of the named entrypoint in the original manifest.
    pub(crate) fn authorization(
        &self,
        name: &str,
    ) -> Option<(EntryPointKind, &EntrypointAuthorizationV1)> {
        match self {
            Self::Baseline(m) => m
                .entrypoints
                .as_ref()?
                .iter()
                .find(|e| e.name == name)
                .map(|e| (e.kind, &e.authorization)),
            Self::C10(m) => m
                .entrypoints
                .as_ref()?
                .iter()
                .find(|e| e.name == name)
                .map(|e| (e.kind, &e.authorization)),
        }
    }

    pub(crate) fn into_owned(self) -> RegisteredManifest<'static> {
        match self {
            Self::Baseline(m) => RegisteredManifest::Baseline(Cow::Owned(m.into_owned())),
            Self::C10(m) => RegisteredManifest::C10(Cow::Owned(m.into_owned())),
        }
    }

    pub(crate) fn hashes(&self) -> (Option<Hash>, Option<Hash>) {
        match self {
            Self::Baseline(m) => (m.code_hash, m.abi_hash),
            Self::C10(m) => (m.code_hash, m.abi_hash),
        }
    }

    pub(crate) fn agrees_with(
        &self,
        profile: ivm::IvmProfileId,
        embedded: ContractManifest,
    ) -> bool {
        match self {
            Self::Baseline(m) => {
                profile != ivm::IvmProfileId::C10_ABI_V1 && m.same_signed_content(&embedded)
            }
            Self::C10(m) => {
                profile == ivm::IvmProfileId::C10_ABI_V1
                    && ContractManifestC10V1::try_from(embedded)
                        .is_ok_and(|expected| m.same_signed_content(&expected))
            }
        }
    }

    pub(crate) fn declares(&self, kind: EntryPointKind) -> bool {
        match self {
            Self::Baseline(m) => m.entrypoints.iter().flatten().any(|e| e.kind == kind),
            Self::C10(m) => m.entrypoints.iter().flatten().any(|e| e.kind == kind),
        }
    }

    pub(crate) fn trigger_ids(&self) -> Vec<TriggerId> {
        match self {
            Self::Baseline(m) => m
                .entrypoints
                .iter()
                .flatten()
                .flat_map(|e| e.triggers.iter().map(|t| t.id.clone()))
                .collect(),
            Self::C10(m) => m
                .entrypoints
                .iter()
                .flatten()
                .flat_map(|e| e.triggers.iter().map(|t| t.id.clone()))
                .collect(),
        }
    }

    pub(crate) fn entrypoint(&self, ordinal: usize) -> Option<(&str, EntryPointKind)> {
        match self {
            Self::Baseline(m) => m
                .entrypoints
                .as_ref()?
                .get(ordinal)
                .map(|e| (e.name.as_str(), e.kind)),
            Self::C10(m) => m
                .entrypoints
                .as_ref()?
                .get(ordinal)
                .map(|e| (e.name.as_str(), e.kind)),
        }
    }

    pub(crate) fn events(&self) -> &[ContractEventDescriptorV1] {
        match self {
            Self::Baseline(m) => &m.events,
            Self::C10(m) => &m.events,
        }
    }
}

use crate::smartcontracts::isi::helpers::verify_signature_for_signer;
use iroha_data_model::{
    isi::error::{InstructionExecutionError, InvalidParameterError},
    smart_contract::manifest::ManifestProvenance,
};

/// Verify an exact manifest signing frame under its source pool. Execution and restore share this check.
pub(crate) fn verify_provenance<'manifest>(
    provenance: Option<&'manifest ManifestProvenance>,
    budget: &iroha_allocation::AllocationBudget,
    frame_len: impl FnOnce() -> Result<usize, norito::core::Error>,
    encode: impl FnOnce(
        &norito::core::DecodeBudgetContext,
        usize,
    ) -> Result<Vec<u8>, norito::core::BoundedEncodeError>,
) -> Result<
    &'manifest ManifestProvenance,
    crate::execution_attempt::ExecutionAttemptError<InstructionExecutionError>,
> {
    use crate::execution_attempt::{
        ExecutionAttemptError, ExecutionDeferred, norito_decode_attempt_error,
    };
    use norito::core::{BoundedEncodeError, DecodeBudgetContext};
    let provenance = provenance.ok_or_else(|| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "manifest.provenance missing".into(),
        ))
    })?;

    let defer = |reason: ExecutionDeferred| ExecutionAttemptError::Deferred(reason);
    // The context and exact output frame belong to the same State execution pool.
    // Local capacity limits never become a verdict about the signed content.
    let max_frame_bytes = budget.limit_bytes();
    let mut counter = budget
        .try_reserve(DecodeBudgetContext::allocation_layout())
        .map_err(|original| defer(original.into()))?;
    let context = DecodeBudgetContext::from_reservation(
        norito::DecodeLimits::new(
            max_frame_bytes,
            max_frame_bytes,
            max_frame_bytes,
            max_frame_bytes,
            norito::core::MAX_VALUE_NESTING_DEPTH,
        ),
        &mut counter,
    )
    .map_err(|error| {
        let reason = match error {
            iroha_allocation::PrepaidSharedError::Allocator { .. } => {
                ivm::error::ExecutionDeferral::AllocationUnavailable
            }
            iroha_allocation::PrepaidSharedError::Reservation(_) => {
                ivm::error::ExecutionDeferral::ActiveMemoryCapacity
            }
        };
        defer(reason.into())
    })?;
    let encode_error = |error: BoundedEncodeError| match error {
        BoundedEncodeError::Serialization(error) => norito_decode_attempt_error(error, |e| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                format!("manifest signing payload encoding failed: {e}"),
            ))
        }),
        BoundedEncodeError::AllocationFailed { .. } => {
            defer(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
        }
        BoundedEncodeError::FrameTooLarge { .. } => {
            defer(ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into())
        }
    };
    // Count the borrowed canonical payload before reserving its real output.
    // The reservation remains live until the payload has been verified and freed.
    let encoded_bytes = context.with(|| {
        frame_len()
            .map_err(BoundedEncodeError::from)
            .map_err(encode_error)
    })?;
    let _frame = budget
        .try_reserve_bytes(encoded_bytes)
        .map_err(|original| defer(original.into()))?;
    let payload = context.with(|| encode(&context, max_frame_bytes).map_err(encode_error))?;
    verify_signature_for_signer(&provenance.signature, &provenance.signer, &payload).map_err(
        |_| {
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "manifest signature verification failed".into(),
            ))
        },
    )?;
    Ok(provenance)
}

/// Validate each persisted C10 registration against its original artifact and signing frame.
/// Neither restoration nor verification opens C10 to new work.
pub(crate) fn validate_cut(
    world: &impl WorldReadOnly,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), crate::state::deserialize::StateRestoreError> {
    use crate::{
        execution_attempt::{ExecutionAttemptError, ExecutionDeferred},
        state::deserialize::StateRestoreError,
    };
    let invalid = |msg: String| {
        StateRestoreError::Serialization(norito::json::Error::InvalidField {
            field: "world.c10_contract_manifests".into(),
            message: msg,
        })
    };
    let occupied = [
        (
            "world.c10_contract_profiles",
            !world.c10_contract_profiles().is_empty(),
        ),
        (
            "world.c10_contract_manifests",
            !world.c10_contract_manifests().is_empty(),
        ),
        ("world.c10_triggers", !world.c10_triggers().is_empty()),
    ];
    if !occupied.iter().any(|(_, present)| *present) {
        return Ok(());
    }
    let catalog = crate::runtime::catalog::release_catalog().map_err(|e| invalid(e.to_string()))?;
    let addition = catalog
        .additions
        .iter()
        .find(|addition| addition.module == crate::runtime::catalog::additions::IVM_C10.module)
        .ok_or_else(|| invalid("C10 storage has no compiled owner".into()))?;
    for (field, present) in occupied {
        if present
            && !addition.storage.get(field).is_some_and(|id| {
                world
                    .runtime_selection()
                    .installed_storage
                    .binary_search(id)
                    .is_ok()
            })
        {
            return Err(invalid(format!("rows in uninstalled C10 storage {field}")));
        }
    }
    for (id, _) in world.c10_contract_profiles().iter() {
        if world.c10_contract_manifests().get(id).is_none() {
            return Err(invalid(
                "C10 profile binding has no corresponding manifest".into(),
            ));
        }
    }
    for (id, manifest) in world.c10_contract_manifests().iter() {
        let Some(registered @ RegisteredManifest::C10(_)) = RegisteredManifest::get(world, id)
        else {
            return Err(invalid(
                "C10 manifest has a conflicting or absent profile binding".into(),
            ));
        };
        let bytes = world
            .contract_code()
            .get(id)
            .ok_or_else(|| invalid("C10 manifest has no registered artifact".into()))?;
        let verified = ivm::verify_contract_artifact_with_memory_budget_for_profile(
            bytes,
            budget,
            ivm::IvmProfileId::C10_ABI_V1,
        )
        .map_err(|error| match error.try_into_local_vm_error() {
            Ok(vm) => match ExecutionDeferred::try_from_vm_error(vm) {
                Ok(cause) => StateRestoreError::ExecutionDeferred(cause),
                Err(vm) => invalid(vm.to_string()),
            },
            Err(e) => invalid(e.to_string()),
        })?;
        if verified.code_hash != id.code_hash
            || !registered.agrees_with(verified.profile, verified.manifest)
        {
            return Err(invalid(
                "C10 manifest does not authenticate the registered code".into(),
            ));
        }
        verify_provenance(
            manifest.provenance.as_ref(),
            budget,
            || norito::canonical_frame_len(&manifest.signature_payload()),
            |ctx, limit| manifest.signature_payload_bytes(ctx, limit),
        )
        .map_err(|e| match e {
            ExecutionAttemptError::Deferred(original) => {
                StateRestoreError::ExecutionDeferred(original)
            }
            ExecutionAttemptError::Rejected(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(reason),
            )) => invalid(reason.to_string()),
            ExecutionAttemptError::Rejected(e) => invalid(e.to_string()),
        })?;
    }
    Ok(())
}
