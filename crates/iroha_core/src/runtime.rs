//! Live runtime upgrades: the definitions this release executes and the selection each root runs
//! (`specs/live_runtime_upgrades.md`).
//!
//! A World of this release is founded on the baseline of [`catalog`]: its `runtime_definitions`
//! table holds the published definitions and its `runtime_selection` cell the one selection the
//! universal root executes. Execution resolves every instruction against that selection, and the
//! State commitment covers exactly the storage it installs.

pub mod attestation;
pub mod catalog;
pub(crate) mod change_set;
pub(crate) mod containment;
pub(crate) mod envelope;
pub mod ironwood;
pub mod keyed_state;
pub mod lifecycle_activation;
pub mod lifecycle_preflight;
pub mod state_row_permission;
pub(crate) mod lifecycle_support;
pub(crate) mod obligations;
pub(crate) mod sorafs_authority;
/// The owner-authorized, all-four-certified one-time Taira SoraFS transition.
pub mod sorafs_repair;
pub mod taira_activation;

use iroha_data_model::runtime::{
    definition::{DefinitionDescriptorV1, DefinitionId, RestrictionV1, RootSelectionV1},
    lifecycle::RuntimeRejectionV1,
};
use mv::storage::Storage;

pub use catalog::{CatalogError, ReleaseCatalog, release_catalog};

/// The definitions a new World is founded with: the baseline's.
///
/// # Errors
/// The release catalog cannot be built (a build defect).
pub(crate) fn founded_definitions()
-> Result<Storage<DefinitionId, DefinitionDescriptorV1>, CatalogError> {
    let catalog = release_catalog()?;
    Ok(catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect())
}

/// The selection a new World is founded with: the baseline.
///
/// # Errors
/// The release catalog cannot be built (a build defect).
pub(crate) fn founded_selection() -> Result<RootSelectionV1, CatalogError> {
    Ok(release_catalog()?.baseline.clone())
}

/// Activate this release's additions on `world`'s universal root, in declaration order, up to and
/// including `through`, as their governed change sets do at block start: publish each addition's
/// definitions, install its storage (a cell at its published initial value) and move the
/// selection. The independent authority addition activates alone. An addition already active
/// stays as it is.
///
/// Fixtures only. A network activates an addition through a governed change set
/// (`specs/runtime_upgrades.md`); no production World is founded on one.
///
/// # Errors
/// `through` is not an addition of this release, or one of the activations does not apply.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn activate_additions_for_testing(
    world: &crate::state::World,
    through: &str,
) -> Result<(), String> {
    let catalog = release_catalog().map_err(|error| error.to_string())?;
    let last = catalog
        .additions
        .iter()
        .position(|addition| addition.module == through)
        .ok_or_else(|| format!("{through} is not an addition of this release"))?;
    // A change set's network scope does not shape the selection it produces.
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0; 32])),
    );
    let mut block = world.block();
    let additions = if through == catalog::additions::SORAFS_AUTHORITY.module {
        &catalog.additions[last..=last]
    } else {
        &catalog.additions[..=last]
    };
    for addition in additions {
        let current = block.runtime_selection.get().clone();
        if current.active.contains(&addition.implementation)
            || (addition.module == catalog::additions::SORAFS_AUTHORITY.module
                && sorafs_authority_enabled(&current).map_err(|error| error.to_string())?)
        {
            continue;
        }
        let published = change_set::PublishedDefinitions(&block.runtime_definitions);
        let change = catalog
            .activation(addition.module, &current, &published, network, 1, [0; 32])
            .map_err(|error| error.to_string())?;
        let applied = change
            .apply(&current, &published)
            .map_err(|error| error.to_string())?;
        for (definition, descriptor) in applied.published {
            block.runtime_definitions.insert(definition, descriptor);
        }
        *block.runtime_selection.get_mut() = applied.selection;
        let mut transaction = block.transaction_without_telemetry(
            iroha_config::parameters::actual::LaneConfig::default(),
            0,
        );
        change_set::install_cells(&mut transaction, &change, catalog, 1)
            .map_err(|error| error.to_string())?;
        transaction.apply();
    }
    block.commit();
    Ok(())
}

/// The top-level World members whose storage `selection` does not install, by snapshot member
/// name (`specs/live_runtime_upgrades.md` §5.5).
///
/// A World field a release addition adds stays out of snapshots until a change set installs its
/// storage, and a restore gives it its empty default, so the release before the addition keeps
/// reading this release's snapshots until activation.
///
/// # Errors
/// The release catalog cannot be built (a build defect), or an uninstalled field is not a
/// top-level World member.
pub(crate) fn uninstalled_world_members(
    selection: &RootSelectionV1,
) -> Result<Vec<&'static str>, String> {
    let catalog = release_catalog().map_err(|error| error.to_string())?;
    let members = <crate::state::World as norito::json::FastJsonWrite>::json_object_field_order()
        .unwrap_or_default();
    catalog
        .world_storage
        .iter()
        .filter(|(_, storage)| selection.installed_storage.binary_search(storage).is_err())
        .map(|(field, _)| {
            field
                .strip_prefix("world.")
                .and_then(|name| members.iter().find(|member| **member == name).copied())
                .ok_or_else(|| format!("uninstalled World field {field} is not a top-level member"))
        })
        .collect()
}

/// The change set that moves the universal root of `world`, on `network`, from its committed
/// selection to the addition `module` of this release, effective at `effective_height`
/// ([`ReleaseCatalog::activation`]); an operator endorses and proposes it.
///
/// # Errors
/// `module` is not an addition of this release, or the change set does not apply to the
/// committed selection (an earlier addition is not active yet, or this one already is).
pub fn draft_addition(
    world: &impl crate::state::WorldReadOnly,
    network: iroha_data_model::NetworkId,
    module: &str,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<iroha_data_model::runtime::lifecycle::ChangeSetV1, CatalogError> {
    let published = change_set::PublishedDefinitions(world.runtime_definitions());
    if module == iroha_data_model::runtime::keyed_state::PROFILE_NAME {
        return keyed_state::next_change_set(network, world.runtime_selection(), &published, effective_height, qualification)
            .map_err(|reason| CatalogError::Addition { module: module.to_owned(), reason });
    }
    release_catalog()?.activation(
        module,
        world.runtime_selection(),
        &published,
        network,
        effective_height,
        qualification,
    )
}

/// Resolve `instruction` against the selection its root executes at `height` (§5.2).
///
/// The instruction's wire ID must be bound, and neither its interface nor the bound
/// implementation may have admission closed, be completion-only or be disabled at `height`
/// (§7.1), unless the instruction continues or completes a live obligation that the restriction
/// lets through (§7.2): `obligation_step` says how it acts on one, and runs only for a restricted
/// binding. Admission closure lets work already admitted continue; completion-only lets it
/// complete. Every node resolves an envelope against the same committed selection and State, so
/// the first envelope a transaction cannot execute is the same everywhere. The bound implementation is
/// native code of this release: a node whose release cannot execute the selection stops before
/// the height rather than reaching this check (§6).
///
/// An instruction without a wire ID never arrives in a transaction or contract: decoding
/// accepts only registered wire IDs. Native code constructs it as a private primitive inside its
/// own reference closure (§3.2), which the resolution of the instruction that invoked it covers.
///
/// Returns the implementation the selection binds the instruction's wire ID to, which executes
/// it, or `None` for a private primitive.
///
/// # Errors
/// The stable rejection of the live-upgrade mechanism.
pub(crate) fn resolve(
    instruction: &iroha_data_model::isi::InstructionBox,
    selection: &RootSelectionV1,
    height: u64,
    obligation_step: impl FnOnce() -> Option<obligations::Step>,
) -> Result<Option<DefinitionId>, iroha_data_model::isi::error::InstructionExecutionError> {
    let Some(wire_id) = iroha_data_model::isi::instruction_wire_id(instruction) else {
        return Ok(None);
    };
    let binding = selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .map(|index| &selection.bindings[index])
        .map_err(|_| RuntimeRejectionV1::DefinitionInactive {
            wire_id: wire_id.to_owned(),
        })?;
    let mut obligation_step = Some(obligation_step);
    let mut step = None;
    for definition in [binding.interface, binding.implementation] {
        if let Some(restriction) = blocking_restriction(selection, &definition, height) {
            let step: Option<obligations::Step> =
                *step.get_or_insert_with(|| obligation_step.take().and_then(|step| step()));
            if !step.is_some_and(|step| step.runs_under(restriction)) {
                return Err(RuntimeRejectionV1::DefinitionRestricted {
                    definition,
                    restriction,
                }
                .into());
            }
        }
    }
    if ironwood::admission::direct(instruction) {
        ensure_privacy_protocol_storage_v1(
            iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            selection,
            wire_id,
        )?;
    }
    Ok(Some(binding.implementation))
}

/// Require the successor storage contract for an Ironwood write, including private native calls.
pub(crate) fn ensure_privacy_protocol_storage_v1(
    protocol: iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1,
    selection: &RootSelectionV1,
    operation: &str,
) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
    if protocol == iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1
        && !ironwood::enabled(selection)
    {
        return Err(
            iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                iroha_data_model::isi::error::InvalidParameterError::SmartContract(format!(
                    "Ironwood protocol storage is inactive for {operation}; the committed selection does not admit {}",
                    protocol.canonical_label(),
                )),
            ),
        );
    }
    Ok(())
}

/// Keep current and retained predecessor privacy records inside the installed storage grammar.
///
/// CorePrivacy can validate Ironwood candidate state in isolation. A restored or initially supplied
/// World cannot contain it until committed state selects the governed storage-schema successor.
pub(crate) fn validate_privacy_world_storage_v1(
    world: &mut crate::state::World,
) -> Result<(), String> {
    use crate::privacy_state::{
        PrivacyCommitmentKeyV1, PrivacyNullifierKeyV1, PrivacyRootProvenanceV1,
        PrivacyStateItemRecordV1,
    };
    use iroha_data_model::privacy::PrivacyRuntimeProtocolIdV1;
    use mv::storage::StorageReadOnly as _;

    if ironwood::enabled(world.runtime_selection.view().get()) {
        // The accumulator cache keys immutable selected identities. Cold restoration must
        // establish that the supplied registry has their exact compiled descriptor preimages;
        // ordinary governed publication cannot replace an existing content-addressed record.
        let catalog = release_catalog().map_err(|error| error.to_string())?;
        let definitions = world.runtime_definitions.view();
        for id in &world.runtime_selection.view().get().installed_storage {
            if definitions.get(id) != catalog.definitions.get(id) {
                return Err(format!(
                    "Ironwood restored storage definition {id} differs from its immutable compiled identity"
                ));
            }
        }
        return Ok(());
    }

    const IRONWOOD: PrivacyRuntimeProtocolIdV1 = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;

    fn check<K: mv::Key, V: mv::Value>(
        storage: &mut Storage<K, V>,
        table: &str,
        key_has_ironwood: impl Fn(&K) -> bool,
        value_has_ironwood: impl Fn(&V) -> bool,
    ) -> Result<(), String> {
        // Exclusive history acquisition reads the original current/undo pair without allocating
        // a replacement map or consuming undo. Even an absent undo value retains its typed key.
        // Inspect raw fields: role-owned accessors can hide a mismatched namespace in an undo row.
        let history = storage.history();
        for (key, value) in history
            .current()
            .iter()
            .map(|(key, value)| (key, Some(value)))
            .chain(
                history
                    .revert_map()
                    .iter()
                    .map(|(key, value)| (key, value.as_ref())),
            )
        {
            if key_has_ironwood(key) || value.is_some_and(&value_has_ironwood) {
                return Err(format!(
                    "{table} contains Ironwood before its governed storage-schema transition"
                ));
            }
        }
        Ok(())
    }
    fn item_has_ironwood(item: &PrivacyStateItemRecordV1) -> bool {
        match item {
            PrivacyStateItemRecordV1::PublicReserveCustody { owner, .. } => {
                owner.namespace().protocol_id() == IRONWOOD
            }
            // These records contain no protocol tags or namespaces, including their nested
            // protocol-specific bootstrap, policy, revision and accumulator structures.
            PrivacyStateItemRecordV1::ZkAcePolicyGovernance { .. }
            | PrivacyStateItemRecordV1::BootleLanternIssuerPolicyGovernance { .. }
            | PrivacyStateItemRecordV1::VegaIssuerGovernance { .. }
            | PrivacyStateItemRecordV1::ZkX509TrustAnchorGovernance { .. }
            | PrivacyStateItemRecordV1::ZkX509CertificatePolicyGovernance { .. }
            | PrivacyStateItemRecordV1::ZkX509CrlGovernance { .. }
            | PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier { .. }
            | PrivacyStateItemRecordV1::OrchardPoolState { .. }
            | PrivacyStateItemRecordV1::OrchardVerifiedNullifier { .. }
            | PrivacyStateItemRecordV1::ProofManagedPoolBootstrap { .. }
            | PrivacyStateItemRecordV1::ProofManagedPoolBootstrapCommitment { .. }
            | PrivacyStateItemRecordV1::FcmpBootstrapOutput { .. }
            | PrivacyStateItemRecordV1::ProofManagedPoolVerifiedNullifier { .. }
            | PrivacyStateItemRecordV1::ProofManagedPoolVerifiedCommitment { .. }
            | PrivacyStateItemRecordV1::FcmpVerifiedOutput { .. }
            | PrivacyStateItemRecordV1::ZkAceVerifiedAuthorization { .. }
            | PrivacyStateItemRecordV1::ZkAmsGovernance { .. }
            | PrivacyStateItemRecordV1::ZkAmsVerifiedProof { .. } => false,
        }
    }
    fn root_has_ironwood(provenance: &PrivacyRootProvenanceV1) -> bool {
        match provenance {
            PrivacyRootProvenanceV1::ZkX509CaGovernance { namespace, .. } => {
                namespace.protocol_id() == IRONWOOD
            }
            PrivacyRootProvenanceV1::ProofManagedPoolBootstrap { protocol_id, .. }
            | PrivacyRootProvenanceV1::ProofManagedPoolSuccessor { protocol_id, .. } => {
                *protocol_id == IRONWOOD
            }
            PrivacyRootProvenanceV1::Governance { .. }
            | PrivacyRootProvenanceV1::ZkAmsRegistryBootstrap { .. }
            | PrivacyRootProvenanceV1::ZkAmsRegistrySuccessor { .. }
            | PrivacyRootProvenanceV1::OrchardPoolBootstrap { .. }
            | PrivacyRootProvenanceV1::OrchardPoolSuccessor { .. }
            | PrivacyRootProvenanceV1::VerifiedBootstrap { .. }
            | PrivacyRootProvenanceV1::VerifiedProof { .. }
            | PrivacyRootProvenanceV1::VerifiedPgcSuccessor { .. } => false,
        }
    }
    check(
        &mut world.privacy_protocol_policies,
        "world.privacy_protocol_policies",
        |key| key.protocol_id() == IRONWOOD,
        |policy| {
            policy.protocol_id == IRONWOOD
                || policy.current_limits.protocol_id() == IRONWOOD
                || policy
                    .pending_update
                    .as_ref()
                    .is_some_and(|pending| pending.next_limits.protocol_id() == IRONWOOD)
        },
    )?;
    check(
        &mut world.privacy_pgc_accounts,
        "world.privacy_pgc_accounts",
        |key| key.namespace().protocol_id() == IRONWOOD,
        |_| false,
    )?;
    check(
        &mut world.privacy_pgc_pool_invariants,
        "world.privacy_pgc_pool_invariants",
        |key| key.namespace().protocol_id() == IRONWOOD,
        |_| false,
    )?;
    check(
        &mut world.privacy_nullifiers,
        "world.privacy_nullifiers",
        |key| match key {
            PrivacyNullifierKeyV1::ZkAceReplay { .. } => false,
            PrivacyNullifierKeyV1::ZkAmsKeyImage { namespace, .. }
            | PrivacyNullifierKeyV1::ZkX509CertificateNullifier { namespace, .. }
            | PrivacyNullifierKeyV1::OrchardNullifier { namespace, .. }
            | PrivacyNullifierKeyV1::FcmpKeyImage { namespace, .. }
            | PrivacyNullifierKeyV1::ProofManagedNullifier { namespace, .. } => {
                namespace.protocol_id() == IRONWOOD
            }
        },
        item_has_ironwood,
    )?;
    check(
        &mut world.privacy_commitments,
        "world.privacy_commitments",
        |key| match key {
            PrivacyCommitmentKeyV1::OrchardPoolState { namespace }
            | PrivacyCommitmentKeyV1::ProofManagedPoolConfig { namespace }
            | PrivacyCommitmentKeyV1::ProofManagedPoolCommitment { namespace, .. }
            | PrivacyCommitmentKeyV1::FcmpOutput { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsIssuerPolicyRecord { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsPhc { namespace, .. }
            | PrivacyCommitmentKeyV1::ZkAmsSeedKey { namespace, .. } => {
                namespace.protocol_id() == IRONWOOD
            }
            PrivacyCommitmentKeyV1::PublicReserveCustody { protocol_id, .. } => {
                *protocol_id == IRONWOOD
            }
            PrivacyCommitmentKeyV1::ZkAcePolicy { .. }
            | PrivacyCommitmentKeyV1::BootleLanternIssuerPolicy { .. }
            | PrivacyCommitmentKeyV1::VegaIssuerRevision { .. }
            | PrivacyCommitmentKeyV1::ZkX509TrustAnchorRevision { .. }
            | PrivacyCommitmentKeyV1::ZkX509CertificatePolicyRevision { .. }
            | PrivacyCommitmentKeyV1::ZkX509CrlCurrent { .. } => false,
        },
        item_has_ironwood,
    )?;
    check(
        &mut world.privacy_roots,
        "world.privacy_roots",
        |key| key.namespace().protocol_id() == IRONWOOD,
        root_has_ironwood,
    )?;
    check(
        &mut world.privacy_root_heads,
        "world.privacy_root_heads",
        |key| key.namespace().protocol_id() == IRONWOOD,
        |head| root_has_ironwood(head.provenance()),
    )
}

/// The restriction of `definition` that stops new work at `height`, if any (§7.1).
///
/// Deprecation never stops execution. [`resolve`] still lets a live obligation's continuations or
/// completions through admission closure and completion-only ([`obligations::Step::runs_under`]).
fn blocking_restriction(
    selection: &RootSelectionV1,
    definition: &DefinitionId,
    height: u64,
) -> Option<RestrictionV1> {
    let index = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(definition))
        .ok()?;
    let restriction = selection.restrictions[index].restriction;
    match restriction {
        RestrictionV1::Deprecated(_) => None,
        RestrictionV1::AdmissionClosed(from)
        | RestrictionV1::CompletionOnly(from)
        | RestrictionV1::Disabled(from) => (from <= height).then_some(restriction),
    }
}

/// Whether the root selected this release's exact nondelegable SoraFS services.
///
/// # Errors
/// The selected services profile is unsupported.
pub fn sorafs_authority_enabled(selection: &RootSelectionV1) -> Result<bool, CatalogError> {
    release_catalog()?.service_enabled(
        selection.services_profile,
        sorafs_authority::SERVICE,
        iroha_crypto::Hash::new(sorafs_authority::SERVICE_CLOSURE).into(),
    )
}

/// Check the unchanged native inventory precondition for the SoraFS authority addition.
///
/// This read-only observation neither authorizes a change set nor establishes governance
/// approval, qualification or activation readiness. Activation itself repeats the same check
/// on its actual World. No query permission or runtime selection is changed by this call.
///
/// # Errors
/// Returns the exact original native activation-inventory failure without reinterpretation.
pub fn validate_sorafs_authority_activation_inventory(
    world: &impl crate::state::WorldReadOnly,
) -> Result<(), String> {
    crate::smartcontracts::isi::sorafs_authority::validate_activation_inventory(world)
}

#[cfg(test)]
mod merged_tests;
#[cfg(test)]
mod tests;

/// Output semantics are selected by committed immutable services, never by row shape.
/// # Errors
/// The selected profile is absent or its lifecycle closure is unsupported by this release.
pub fn trigger_lifecycle_enabled(selection: &RootSelectionV1) -> Result<bool, CatalogError> {
    release_catalog()?.service_enabled(
        selection.services_profile,
        "trigger_lifecycle",
        iroha_crypto::Hash::new(iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE)
            .into(),
    )
}
