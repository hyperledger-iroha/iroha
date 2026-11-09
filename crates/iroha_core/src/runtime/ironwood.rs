//! Governed Ironwood grammar and byte-preserving privacy storage cutover.
//!
//! The predecessor definitions remain immutable. The successor replaces their privacy and stored-instruction
//! storage contracts and the monolithic native bundle; no row bytes, pool identities or roots
//! inside those rows are rewritten. See `specs/ironwood_runtime_transition.md` for the fixed codec
//! extension, migration contract and mandatory decoder-rollout gate.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    isi::privacy::{
        BootstrapPrivacyIronwoodPoolV1, SchedulePrivacyIronwoodLimitsUpdateV1,
        SubmitPrivacyIronwoodProofV1,
    },
    runtime::{
        definition::{
            BindingV1, DefinitionDescriptorV1 as Descriptor, DefinitionId, EntrypointV1,
            ImplementationCodeV1, InterfaceDescriptorV1, MigrationDescriptorV1, ResourceLimitsV1,
            RootSelectionV1, SchemaV1, StorageShapeV1,
        },
        lifecycle::{ChangeSetV1, RestrictionKindV1, RootScopeV1, SelectionTransitionV1},
    },
};

use super::catalog::{CatalogError, ReleaseCatalog};

pub(crate) mod admission;

const DOMAIN: &[u8] = b"iroha:runtime:ironwood:v1\0";
const MODULE: &str = "iroha.ironwood.v1";
const PRIVACY_FIELDS: [&str; 12] = [
    "world.privacy_protocol_policies",
    "world.privacy_pgc_accounts",
    "world.privacy_pgc_pool_invariants",
    "world.privacy_nullifiers",
    "world.privacy_commitments",
    "world.privacy_roots",
    "world.privacy_root_heads",
    "triggers.data",
    "triggers.pipeline",
    "triggers.time",
    "triggers.by_call",
    "world.smart_contract_state",
];

/// Immutable successor composition implemented by this release, never founded automatically.
#[derive(Clone, Debug)]
pub struct IronwoodCatalog {
    /// Selection installed only by an approved, fully attested change set.
    pub selection: RootSelectionV1,
    /// Successor definitions to publish, in canonical identity order.
    pub definitions: Vec<DefinitionId>,
    /// Byte-preserving migration certificate, published with the successor.
    pub migration: DefinitionId,
    /// New storage identity for each affected physical storage owner.
    pub world_storage: BTreeMap<&'static str, DefinitionId>,
}

fn digest(parts: &[&[u8]]) -> [u8; 32] {
    let mut framed = Vec::new();
    framed.extend_from_slice(DOMAIN);
    for part in parts {
        framed.extend_from_slice(&(part.len() as u64).to_le_bytes());
        framed.extend_from_slice(part);
    }
    Hash::new(framed).into()
}

fn layout() -> String {
    let layout = crate::state::authority_registry::V1_LAYOUT;
    format!("{}.{}.{}", layout.major, layout.minor, layout.flags)
}

/// Name the fixed recursive extension of the encoded predecessor schema. The immutable contract
/// fixes the separate runtime protocol codec and three additive Ironwood wire IDs,
/// including their storage carriers. Baseline instruction payload codecs stay frozen.
fn extended_codec(previous: &SchemaV1) -> Result<SchemaV1, CatalogError> {
    let encoded = norito::encode_canonical(previous)
        .map_err(|error| CatalogError::Parameters(error.to_string()))?;
    let id = hex::encode(digest(&[b"recursive-codec-extension", &encoded]));
    Ok(SchemaV1::Codec(format!(
        "semantic:iroha.ironwood.v1.{id}@{}",
        layout()
    )))
}

pub(super) fn build_catalog(catalog: &mut ReleaseCatalog) -> Result<IronwoodCatalog, CatalogError> {
    let baseline = catalog.baseline.clone();
    let preceding: std::collections::BTreeSet<_> = catalog.definitions.keys().copied().collect();
    let old_bundle = baseline.bindings[0].implementation;
    let Descriptor::Implementation(mut bundle) = catalog.definitions[&old_bundle].clone() else {
        unreachable!("baseline native bundle");
    };
    let mut replacements = BTreeMap::new();
    let mut world_storage = BTreeMap::new();
    for field in PRIVACY_FIELDS {
        let previous = catalog.world_storage[field];
        let Descriptor::Storage(mut storage) = catalog.definitions[&previous].clone() else {
            unreachable!("baseline storage");
        };
        let StorageShapeV1::Table { key } = &mut storage.shape else {
            unreachable!("privacy owners are tables");
        };
        *key = extended_codec(key)?;
        storage.value = extended_codec(&storage.value)?;
        storage.invariants = digest(&[b"storage", &previous.0]);
        let successor = catalog.publish(Descriptor::Storage(storage))?;
        replacements.insert(previous, successor);
        world_storage.insert(field, successor);
    }
    let Descriptor::ServicesProfile(mut services) =
        catalog.definitions[&baseline.services_profile].clone()
    else {
        unreachable!("baseline services");
    };
    for service in &mut services.services {
        service.reference_closure = digest(&[b"service", service.name.as_bytes()]);
    }
    let services = catalog.publish(Descriptor::ServicesProfile(services))?;
    let mut events = BTreeMap::new();
    for previous in &bundle.events {
        let Descriptor::Event(mut event) = catalog.definitions[previous].clone() else {
            unreachable!("baseline events");
        };
        event.name = "iroha.events.data.ironwood.v1".into();
        event.schema = extended_codec(&event.schema)?;
        events.insert(*previous, catalog.publish(Descriptor::Event(event))?);
    }
    let mut interfaces = BTreeMap::new();
    for binding in &baseline.bindings {
        let Descriptor::Interface(mut interface) = catalog.definitions[&binding.interface].clone()
        else {
            unreachable!("baseline interface");
        };
        // Existing wire IDs retain their frozen public payload grammar. The successor
        // changes their execution closure; Ironwood payloads use dedicated additions.
        interface.contract_statement = digest(&[b"interface", &binding.interface.0]);
        interface.events = interface.events.iter().map(|id| events[id]).collect();
        interface.events.sort_unstable();
        interfaces.insert(
            binding.wire_id.clone(),
            catalog.publish(Descriptor::Interface(interface))?,
        );
    }
    for (wire_id, nominal) in [
        (
            BootstrapPrivacyIronwoodPoolV1::WIRE_ID,
            norito::schema::identity::nominal_name::<BootstrapPrivacyIronwoodPoolV1>(),
        ),
        (
            SubmitPrivacyIronwoodProofV1::WIRE_ID,
            norito::schema::identity::nominal_name::<SubmitPrivacyIronwoodProofV1>(),
        ),
        (
            SchedulePrivacyIronwoodLimitsUpdateV1::WIRE_ID,
            norito::schema::identity::nominal_name::<SchedulePrivacyIronwoodLimitsUpdateV1>(),
        ),
    ] {
        let interface = catalog.publish(Descriptor::Interface(InterfaceDescriptorV1 {
            wire_id: wire_id.into(),
            payload: SchemaV1::Codec(format!("norito:{nominal}@{}", layout())),
            rejection_codes: Vec::new(),
            events: events.values().copied().collect(),
            metering_class: "native.baseline".into(),
            contract_statement: digest(&[b"additive-interface", wire_id.as_bytes()]),
        }))?;
        interfaces.insert(wire_id.into(), interface);
    }
    let mut interface_ids: Vec<_> = interfaces.values().copied().collect();
    interface_ids.sort_unstable();
    for ids in [&mut bundle.reads, &mut bundle.writes, &mut bundle.scans] {
        for id in ids.iter_mut() {
            if let Some(replacement) = replacements.get(id) {
                *id = *replacement;
            }
        }
        ids.sort_unstable();
    }
    bundle.entrypoint = EntrypointV1::Instructions(interface_ids.clone());
    bundle.invokes = interface_ids.clone();
    bundle.events = events.values().copied().collect();
    bundle.events.sort_unstable();
    bundle.services_profile = services;
    bundle.code = ImplementationCodeV1::Native {
        module: MODULE.into(),
        reference_closure: digest(&[b"implementation"]),
    };
    let bundle = catalog.publish(Descriptor::Implementation(bundle))?;
    let mut selection = baseline.clone();
    selection.services_profile = services;
    for id in &mut selection.installed_storage {
        if let Some(replacement) = replacements.get(id) {
            *id = *replacement;
        }
    }
    selection.installed_storage.sort_unstable();
    let old_interfaces: Vec<_> = baseline.bindings.iter().map(|b| b.interface).collect();
    selection
        .active
        .retain(|id| *id != old_bundle && !old_interfaces.contains(id) && !events.contains_key(id));
    selection.active.extend(interface_ids);
    selection.active.extend(events.values().copied());
    selection.active.push(bundle);
    selection.active.sort_unstable();
    selection.bindings = interfaces
        .into_iter()
        .map(|(wire_id, interface)| BindingV1 {
            wire_id,
            interface,
            implementation: bundle,
        })
        .collect();
    selection.check_closure(&catalog.definitions)?;
    let mut from: Vec<_> = replacements.keys().copied().collect();
    let mut to: Vec<_> = replacements.values().copied().collect();
    from.sort_unstable();
    to.sort_unstable();
    let migration = catalog.publish(Descriptor::Migration(MigrationDescriptorV1 {
        code: ImplementationCodeV1::Native {
            module: "iroha.ironwood.identity-cutover.v1".into(),
            reference_closure: digest(&[b"identity-cutover"]),
        },
        from,
        to,
        cursor: SchemaV1::Codec(format!(
            "semantic:iroha.ironwood.identity-cutover.complete.v1@{}",
            layout()
        )),
        host_profile: None,
        services_profile: services,
        limits: ResourceLimitsV1 {
            max_gas: 12,
            max_effects: 12,
            max_call_depth: 1,
        },
    }))?;
    let definitions = catalog
        .definitions
        .keys()
        .filter(|id| !preceding.contains(id))
        .copied()
        .collect();
    Ok(IronwoodCatalog {
        selection,
        definitions,
        migration,
        world_storage,
    })
}

/// Whether the installed immutable storage contracts retain the Ironwood grammar.
/// Services and implementation identities may evolve independently; they do not define a
/// stored payload's grammar. Historical definitions remain in the compiled release catalog.
pub(crate) fn enabled(selection: &RootSelectionV1) -> bool {
    // Index the immutable compiled catalog once. The hot path binary-searches the few
    // definitions for each affected physical owner, rather than scanning the whole selection.
    // No node-local flag, unbounded selection cache, or per-instruction selection hashing.
    static OWNERS: std::sync::OnceLock<Vec<Vec<(DefinitionId, bool)>>> = std::sync::OnceLock::new();
    let owners = OWNERS.get_or_init(|| {
        let catalog = super::release_catalog().expect("compiled release catalog");
        let extension = catalog
            .ironwood
            .as_ref()
            .expect("compiled Ironwood catalog");
        extension
            .world_storage
            .iter()
            .map(|(field, expected)| {
                let Descriptor::Storage(expected) = &catalog.definitions[expected] else {
                    unreachable!()
                };
                catalog
                    .definitions
                    .iter()
                    .filter_map(|(id, descriptor)| {
                        let Descriptor::Storage(actual) = descriptor else {
                            return None;
                        };
                        (actual.namespace == *field).then_some((
                            *id,
                            actual.shape == expected.shape
                                && actual.value == expected.value
                                && actual.projection == expected.projection
                                && actual.initial == expected.initial,
                        ))
                    })
                    .collect()
            })
            .collect()
    });
    owners.iter().all(|definitions| {
        let mut installed = definitions
            .iter()
            .filter(|(id, _)| selection.installed_storage.binary_search(id).is_ok());
        matches!(installed.next(), Some((_, true))) && installed.next().is_none()
    })
}

/// Recognize compatible successors from their explicit published codec contracts, never their
/// labels or an equality check against one complete selection. Each physical owner must have
/// exactly one installed definition with the archived Ironwood key/value/projection contract.
pub(crate) fn enabled_with_definitions(
    selection: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
) -> bool {
    let Ok(catalog) = super::release_catalog() else {
        return false;
    };
    let Some(extension) = &catalog.ironwood else {
        return false;
    };
    extension.world_storage.iter().all(|(field, expected)| {
        let Some(Descriptor::Storage(expected)) = catalog.definitions.get(expected) else {
            return false;
        };
        let mut owners =
            selection
                .installed_storage
                .iter()
                .filter_map(|id| match published.definition(id) {
                    Some(Descriptor::Storage(storage)) if storage.namespace == *field => {
                        Some(storage)
                    }
                    _ => None,
                });
        let Some(actual) = owners.next() else {
            return false;
        };
        owners.next().is_none()
            && actual.shape == expected.shape
            && actual.value == expected.value
            && actual.projection == expected.projection
            && actual.initial == expected.initial
    })
}

/// Admission after committed activation protects existing owners against known predecessor
/// reinterpretation. Unknown future codec extensions may be proposed: ordinary governance,
/// closure and obligation rules still apply, and execution requires a qualified newer release.
/// Before activation this rule is never run, even if a scheduled projection contains Ironwood.
pub(crate) fn validate_admitted_successor(
    next: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
) -> Result<(), String> {
    let catalog = super::release_catalog().map_err(|e| e.to_string())?;
    for field in PRIVACY_FIELDS {
        let Descriptor::Storage(baseline) = &catalog.definitions[&catalog.world_storage[field]]
        else {
            unreachable!()
        };
        let mut owners =
            next.installed_storage
                .iter()
                .filter_map(|id| match published.definition(id) {
                    Some(Descriptor::Storage(storage)) if storage.namespace == field => {
                        Some(storage)
                    }
                    _ => None,
                });
        let Some(owner) = owners.next() else {
            return Err(format!("Ironwood successor removes owner {field}"));
        };
        if owners.next().is_some()
            || (owner.shape == baseline.shape && owner.value == baseline.value)
        {
            return Err(format!(
                "Ironwood successor duplicates or reinterprets owner {field} under the predecessor codec"
            ));
        }
    }
    Ok(())
}

/// Check the fixed, one-way identity migration before a successor can execute or commit.
/// Both the predecessor rows and their retained undo retain the same canonical encodings.
pub(crate) fn validate_cutover(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
) -> Result<(), String> {
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let extension = catalog
        .ironwood
        .as_ref()
        .expect("compiled Ironwood catalog");
    let uses_extension = |selection: &RootSelectionV1| {
        selection.services_profile == extension.selection.services_profile
            || extension.world_storage.iter().any(|(field, expected)| {
                let Some(Descriptor::Storage(expected)) = catalog.definitions.get(expected) else {
                    return false;
                };
                selection.installed_storage.iter().any(|id| {
                    matches!(published.definition(id), Some(Descriptor::Storage(actual))
                        if actual.namespace == *field
                            && (actual.shape == expected.shape || actual.value == expected.value))
                })
            })
    };
    if uses_extension(next) && !enabled_with_definitions(next, published) {
        return Err("Ironwood requires the complete successor privacy storage grammar".into());
    }
    if enabled_with_definitions(previous, published) && !enabled_with_definitions(next, published) {
        return Err(
            "Ironwood storage cannot be retired or reinterpreted under its predecessor codec"
                .into(),
        );
    }
    if !enabled_with_definitions(previous, published) && enabled_with_definitions(next, published) {
        let Some(Descriptor::ServicesProfile(actual)) =
            published.definition(&next.services_profile)
        else {
            return Err("first Ironwood activation requires published services".into());
        };
        let Descriptor::ServicesProfile(required) =
            &catalog.definitions[&extension.selection.services_profile]
        else {
            unreachable!()
        };
        if !required
            .services
            .iter()
            .all(|entry| actual.services.contains(entry))
        {
            return Err(
                "first Ironwood activation requires every reviewed Ironwood service closure".into(),
            );
        }
        if !extension
            .world_storage
            .values()
            .all(|id| next.installed_storage.binary_search(id).is_ok())
        {
            return Err(
                "first Ironwood activation requires the exact certified twelve-owner cutover"
                    .into(),
            );
        }
        for field in PRIVACY_FIELDS {
            if previous
                .installed_storage
                .binary_search(&catalog.world_storage[field])
                .is_err()
            {
                return Err(format!(
                    "Ironwood identity cutover has no predecessor owner for {field}"
                ));
            }
        }
        if published.definition(&extension.migration)
            != catalog.definitions.get(&extension.migration)
        {
            return Err(
                "Ironwood identity cutover requires its exact published migration descriptor"
                    .into(),
            );
        }
    }
    Ok(())
}

/// Recognize the one implemented byte-preserving storage retirement. The exact
/// published migration and complete twelve-owner replacement are mandatory; an
/// unrelated retirement cannot borrow this certificate.
pub(crate) fn supports_identity_cutover(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
    added: &BTreeMap<DefinitionId, Descriptor>,
) -> bool {
    use iroha_data_model::runtime::definition::DefinitionLookup;
    struct Overlay<'a, L> {
        published: &'a L,
        added: &'a BTreeMap<DefinitionId, Descriptor>,
    }
    impl<L: DefinitionLookup> DefinitionLookup for Overlay<'_, L> {
        fn definition(&self, id: &DefinitionId) -> Option<&Descriptor> {
            self.added.get(id).or_else(|| self.published.definition(id))
        }
    }
    let Ok(catalog) = super::release_catalog() else {
        return false;
    };
    let Some(extension) = &catalog.ironwood else {
        return false;
    };
    let retired: std::collections::BTreeSet<_> = previous
        .installed_storage
        .iter()
        .filter(|id| next.installed_storage.binary_search(id).is_err())
        .copied()
        .collect();
    let expected = PRIVACY_FIELDS
        .iter()
        .map(|field| catalog.world_storage[field])
        .collect();
    if retired != expected {
        return false;
    }
    let lookup = Overlay { published, added };
    !enabled_with_definitions(previous, &lookup)
        && enabled_with_definitions(next, &lookup)
        && lookup.definition(&extension.migration) == catalog.definitions.get(&extension.migration)
        && validate_cutover(previous, next, &lookup).is_ok()
}

/// Bounded cache of immutable compiled storage validation. A selection ID commits every
/// selected definition; the catalog is immutable for this process. Publication/migration
/// checks remain on the actual transition and are never cached across World registries.
#[derive(Default)]
struct StorageValidationCache(
    std::collections::VecDeque<(
        iroha_data_model::runtime::definition::SelectionId,
        Result<(), String>,
    )>,
);

impl StorageValidationCache {
    fn validate(
        &mut self,
        selection: &RootSelectionV1,
        check: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let id = selection.id().map_err(|error| error.to_string())?;
        if let Some((_, result)) = self.0.iter().find(|(key, _)| *key == id) {
            return result.clone();
        }
        let result = check();
        if self.0.len() == 64 {
            self.0.pop_front();
        }
        self.0.push_back((id, result.clone()));
        result
    }
}

/// Validate compiled storage composition once per selection, including capture and ordinary
/// blocks. The fixed cache bound cannot grow with adversarial historical selections.
pub(crate) fn validate_compiled_storage_selection(
    selection: &RootSelectionV1,
) -> Result<(), String> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<StorageValidationCache>> =
        std::sync::OnceLock::new();
    CACHE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "Ironwood storage cache poisoned".to_owned())?
        .validate(selection, || {
            #[cfg(test)]
            STORAGE_VALIDATION_MISSES.with(|count| count.set(count.get() + 1));
            let catalog = super::release_catalog().map_err(|error| error.to_string())?;
            validate_cutover(selection, selection, &catalog.definitions)
        })
}

#[cfg(test)]
thread_local! {
    pub(crate) static STORAGE_VALIDATION_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Compare a read-only observation of the network's committed selection with the exact
/// predecessor required by `activation_change_set`. This neither approves nor schedules it.
/// Callers must obtain the observation from their independently selected network; an HTTP
/// status response alone is not a finality proof or evidence of decoder readiness.
/// # Errors
/// The observation cannot be encoded or differs from the compiled predecessor.
pub fn check_activation_predecessor(observed: &RootSelectionV1) -> Result<(), String> {
    let expected = super::release_catalog()
        .map_err(|error| error.to_string())?
        .baseline_id;
    let actual = observed.id().map_err(|error| error.to_string())?;
    if actual != expected {
        return Err(format!(
            "Ironwood activation predecessor mismatch: expected {expected}, observed {actual}; construct and review a change set for the committed selection"
        ));
    }
    Ok(())
}

/// Authenticate the exact selection preimage at an independently verified World cut before
/// applying the read-only predecessor comparison. This grants no scheduling authority.
/// # Errors
/// Foreign schema, absent/substituted selection preimage, or a predecessor mismatch.
pub fn check_activation_predecessor_at(
    world: &iroha_data_model::sumeragi_finality::VerifiedWorldStateSnapshotV1,
    observed: &RootSelectionV1,
) -> Result<(), String> {
    authenticate_runtime_selection_at(world, observed)?;
    check_activation_predecessor(observed)
}

/// Authenticate an observed CURRENT selection at its independently certified native World cut.
/// This grants no predecessor/composition approval: apply the current-selection constructor
/// and scheduled projection checks separately to the authenticated published definitions.
/// # Errors
/// Foreign schema or an absent/substituted selection preimage.
pub fn authenticate_runtime_selection_at(
    world: &iroha_data_model::sumeragi_finality::VerifiedWorldStateSnapshotV1,
    observed: &RootSelectionV1,
) -> Result<(), String> {
    crate::state::State::select_native_world_schema_v1(world.schema_hash())?;
    world
        .verify_cell_value("world.runtime_selection", observed)
        .map_err(|error| error.to_string())
}

/// Construct the exact reviewed successor change set. Governance still supplies qualification,
/// approves notice and collects support from every validator; constructing it activates nothing.
///
/// # Errors
/// The release catalog or selection identities cannot be constructed.
pub fn activation_change_set(
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, CatalogError> {
    let catalog = super::release_catalog()?;
    let extension = catalog.ironwood.as_ref().expect("built Ironwood catalog");
    let previous = &catalog.baseline;
    let next = &extension.selection;
    selection_change_set(
        network,
        height,
        qualification,
        previous,
        next,
        extension
            .definitions
            .iter()
            .map(|id| catalog.definitions[id].clone())
            .collect(),
    )
}

fn selection_change_set(
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
    publish: Vec<Descriptor>,
) -> Result<ChangeSetV1, CatalogError> {
    let mut transitions = Vec::new();
    for binding in previous.bindings.iter().filter(|binding| {
        next.bindings
            .binary_search_by(|other| other.wire_id.cmp(&binding.wire_id))
            .is_err()
    }) {
        transitions.push(SelectionTransitionV1::Unbind(binding.wire_id.clone()));
    }
    for id in previous
        .active
        .iter()
        .filter(|id| next.active.binary_search(id).is_err())
    {
        transitions.push(SelectionTransitionV1::Restrict {
            definition: *id,
            restriction: RestrictionKindV1::Disabled,
        });
        transitions.push(SelectionTransitionV1::Deactivate(*id));
    }
    for id in previous
        .installed_storage
        .iter()
        .filter(|id| next.installed_storage.binary_search(id).is_err())
    {
        transitions.push(SelectionTransitionV1::RetireStorage(*id));
    }
    for id in next
        .installed_storage
        .iter()
        .filter(|id| previous.installed_storage.binary_search(id).is_err())
    {
        transitions.push(SelectionTransitionV1::InstallStorage(*id));
    }
    transitions.push(SelectionTransitionV1::SetServicesProfile(
        next.services_profile,
    ));
    for id in next
        .active
        .iter()
        .filter(|id| previous.active.binary_search(id).is_err())
    {
        transitions.push(SelectionTransitionV1::Activate(*id));
    }
    transitions.extend(
        next.bindings
            .iter()
            .cloned()
            .map(SelectionTransitionV1::Bind),
    );
    Ok(ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: previous.id()?,
        successor: next.id()?,
        effective_height: height,
        publish,
        transitions,
        qualification,
    })
}

/// A convenience composition cannot reinterpret restrictions when replacing identities.
/// The foundation's Restrict transition uses this change set's activation height; moving an
/// existing future-height restriction would change its policy. Keep retained entries exact
/// and require an independently reviewed governed transition for any restricted replacement.
fn require_retained_restrictions(
    previous: &RootSelectionV1,
    next: &RootSelectionV1,
) -> Result<(), CatalogError> {
    for entry in &previous.restrictions {
        let retained = entry.definition == next.services_profile
            || next
                .installed_storage
                .binary_search(&entry.definition)
                .is_ok()
            || next.active.binary_search(&entry.definition).is_ok();
        if !retained {
            return Err(CatalogError::Parameters(format!(
                "composition cannot replace restricted definition {}; resolve its restriction through an explicit governed transition first",
                entry.definition,
            )));
        }
    }
    Ok(())
}

/// Compose Ironwood with an independently qualified service-only successor of the current
/// committed selection. This creates a reviewable proposal, never runtime support: a merged
/// release must include and attest the returned content-addressed composition before H.
/// Retained storage owners, kernel, commitments, restrictions on retained definitions and
/// unrelated active definitions are preserved. Any restricted definition that this constructor
/// would replace is refused; resolve that policy through an explicit governed transition first.
/// Restriction heights are never moved to the replacement or silently dropped. Overlapping
/// service edits require an explicit reviewed composition.
/// # Errors
/// Missing/changed predecessor storage, incompatible native bundle, overlapping services,
/// a restricted definition requiring replacement, or malformed definition identities/closure.
/// Callers must authenticate `previous` and `published`.
pub fn activation_change_set_for_selection(
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, Descriptor>,
) -> Result<ChangeSetV1, CatalogError> {
    let base = super::release_catalog()?;
    let change = activation_change_set_in_catalog(
        base,
        network,
        height,
        qualification,
        previous,
        published,
    )?;
    let applied = change
        .apply(previous, published)
        .map_err(|e| CatalogError::Parameters(e.to_string()))?;
    let mut definitions = published.clone();
    definitions.extend(applied.published);
    validate_cutover(previous, &applied.selection, &definitions)
        .map_err(CatalogError::Parameters)?;
    Ok(change)
}

fn activation_change_set_in_catalog(
    base: &ReleaseCatalog,
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, Descriptor>,
) -> Result<ChangeSetV1, CatalogError> {
    let invalid = |message: &str| CatalogError::Parameters(message.into());
    if previous == &base.baseline {
        let extension = base.ironwood.as_ref().expect("compiled Ironwood");
        return selection_change_set(
            network,
            height,
            qualification,
            previous,
            &extension.selection,
            extension
                .definitions
                .iter()
                .filter(|id| !published.contains_key(id))
                .map(|id| base.definitions[id].clone())
                .collect(),
        );
    }
    previous.check_closure(published)?;
    if base.unsupported(previous).is_some() {
        return Err(invalid("current service predecessor is not compiled"));
    }
    let extension = base.ironwood.as_ref().expect("compiled Ironwood");
    if previous.kernel != base.baseline.kernel
        || previous.commitment_profile != base.baseline.commitment_profile
    {
        return Err(invalid(
            "Ironwood composition requires the qualified native kernel and commitment profile",
        ));
    }
    for field in PRIVACY_FIELDS {
        if previous
            .installed_storage
            .binary_search(&base.world_storage[field])
            .is_err()
        {
            return Err(invalid(
                "Ironwood composition is missing an exact predecessor owner",
            ));
        }
    }
    let Descriptor::ServicesProfile(original_services) =
        &base.definitions[&base.baseline.services_profile]
    else {
        unreachable!()
    };
    let Some(Descriptor::ServicesProfile(current_services)) =
        published.get(&previous.services_profile)
    else {
        return Err(invalid("missing current services"));
    };
    if !original_services
        .services
        .iter()
        .all(|entry| current_services.services.contains(entry))
    {
        return Err(invalid(
            "overlapping service closure changes require explicit review",
        ));
    }
    let Descriptor::ServicesProfile(mut services) =
        base.definitions[&extension.selection.services_profile].clone()
    else {
        unreachable!()
    };
    services.services.extend(
        current_services
            .services
            .iter()
            .filter(|entry| {
                !original_services
                    .services
                    .iter()
                    .any(|old| old.name == entry.name)
            })
            .cloned(),
    );
    services.services.sort_by(|a, b| a.name.cmp(&b.name));
    let mut catalog = base.clone();
    for (id, definition) in published {
        if definition.id()? != *id {
            return Err(invalid("published definition has a changed identity"));
        }
        catalog.definitions.insert(*id, definition.clone());
    }
    let services_id = catalog.publish(Descriptor::ServicesProfile(services))?;
    let old_bundle = previous
        .bindings
        .first()
        .ok_or_else(|| invalid("current native bindings are empty"))?
        .implementation;
    if previous.bindings.len() != base.baseline.bindings.len()
        || previous
            .bindings
            .iter()
            .zip(&base.baseline.bindings)
            .any(|(a, b)| {
                a.wire_id != b.wire_id
                    || a.interface != b.interface
                    || a.implementation != old_bundle
            })
    {
        return Err(invalid(
            "current interfaces are not the qualified service-only predecessor",
        ));
    }
    let Some(Descriptor::Implementation(current_bundle)) = published.get(&old_bundle) else {
        return Err(invalid("missing current native bundle"));
    };
    let Descriptor::Implementation(baseline_bundle) =
        &base.definitions[&base.baseline.bindings[0].implementation]
    else {
        unreachable!()
    };
    let mut normalized = current_bundle.clone();
    normalized.code = baseline_bundle.code.clone();
    normalized.services_profile = baseline_bundle.services_profile;
    if normalized != *baseline_bundle
        || current_bundle.code != super::catalog::lifecycle_implementation_code()
    {
        return Err(invalid(
            "current native bundle changes more than the service closure",
        ));
    }
    let Descriptor::Implementation(mut bundle) =
        base.definitions[&extension.selection.bindings[0].implementation].clone()
    else {
        unreachable!()
    };
    bundle.services_profile = services_id;
    bundle.code = super::catalog::lifecycle_implementation_code();
    let bundle_id = catalog.publish(Descriptor::Implementation(bundle))?;
    let mut next = previous.clone();
    next.services_profile = services_id;
    for field in PRIVACY_FIELDS {
        let position = next
            .installed_storage
            .iter()
            .position(|id| *id == base.world_storage[field])
            .expect("checked owner");
        next.installed_storage[position] = extension.world_storage[field];
    }
    next.installed_storage.sort_unstable();
    next.active
        .retain(|id| *id != old_bundle && base.baseline.active.binary_search(id).is_err());
    next.active.extend(
        extension
            .selection
            .active
            .iter()
            .copied()
            .filter(|id| *id != extension.selection.bindings[0].implementation),
    );
    next.active.push(bundle_id);
    next.active.sort_unstable();
    next.active.dedup();
    next.bindings = extension
        .selection
        .bindings
        .iter()
        .cloned()
        .map(|mut binding| {
            binding.implementation = bundle_id;
            binding
        })
        .collect();
    require_retained_restrictions(previous, &next)?;
    next.check_closure(&catalog.definitions)?;
    let publish = catalog
        .definitions
        .iter()
        .filter(|(id, _)| !published.contains_key(id))
        .map(|(_, value)| value.clone())
        .collect();
    selection_change_set(network, height, qualification, previous, &next, publish)
}

/// What a chain has once the certified SoraFS repair is selected: the compiled repair successor of
/// the frozen baseline, and every definition published by then. Nothing else can publish on such a
/// chain before this activation, and the activation boundary compares against its real State.
///
/// # Errors
/// The compiled repair transition cannot be constructed.
pub fn taira_repaired_chain(
    network: NetworkId,
) -> Result<(RootSelectionV1, BTreeMap<DefinitionId, Descriptor>), CatalogError> {
    let base = super::release_catalog()?;
    let mut published: BTreeMap<DefinitionId, Descriptor> = base
        .baseline_definitions
        .iter()
        .map(|id| (*id, base.definitions[id].clone()))
        .collect();
    let applied = base
        .sorafs_repair_activation(&base.baseline, &published, network, 2, [1; 32])?
        .apply(&base.baseline, &published)
        .map_err(|error| CatalogError::Parameters(error.to_string()))?;
    published.extend(applied.published);
    Ok((applied.selection, published))
}

/// Digest of exactly what `change` publishes, so a drafted plan can be compared with what real
/// State would publish.
///
/// # Errors
/// The descriptors cannot be encoded.
pub fn published_digest(change: &ChangeSetV1) -> Result<String, CatalogError> {
    let bytes = norito::encode_canonical(&change.publish)
        .map_err(|error| CatalogError::Parameters(error.to_string()))?;
    Ok(hex::encode(iroha_crypto::Hash::new(bytes).as_ref()))
}

/// The one owner-authorized Taira change set: from the compiled repair successor of the frozen
/// baseline to the compiled Time and Ironwood composition.
///
/// The successor keeps the repair kernel and otherwise equals the compiled composition, so its
/// services profile, implementation and storage set are the identities readers already know. The
/// spent repair instruction is unbound and deactivated. This constructs a change set and approves
/// nothing (`specs/runtime/taira_time_ironwood_v1.md`).
///
/// # Errors
/// `previous` is not the exact repaired selection, or the composition cannot be constructed.
pub fn taira_time_ironwood_change_set(
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
) -> Result<ChangeSetV1, CatalogError> {
    let invalid = |message: String| CatalogError::Parameters(message);
    let base = super::release_catalog()?;
    let extension = base.ironwood.as_ref().expect("compiled Ironwood");
    let (repaired, _) = taira_repaired_chain(network)?;
    if previous != &repaired {
        return Err(invalid(
            "Taira activation requires the exact repaired selection".into(),
        ));
    }
    let mut builder = base.clone();
    let mut next = builder.lifecycle_successor(&extension.selection)?;
    next.kernel = previous.kernel;
    let mut needed: std::collections::BTreeSet<DefinitionId> =
        next.active.iter().copied().collect();
    needed.extend(next.installed_storage.iter().copied());
    needed.insert(next.services_profile);
    needed.insert(extension.migration);
    let mut publish = Vec::new();
    for id in needed {
        if published.definition(&id).is_some() {
            continue;
        }
        let descriptor = builder
            .definitions
            .get(&id)
            .cloned()
            .ok_or_else(|| invalid(format!("composition definition {id} is not compiled")))?;
        publish.push(descriptor);
    }
    selection_change_set(network, height, qualification, previous, &next, publish)
}

/// Derive a compiled service-only release (including Time) from the CURRENT selection.
/// `reference` is the complete baseline-derived selection, whose extra services
/// are retained alongside Ironwood's grammar. The merged catalog must already implement it.
/// Restrictions on retained definitions remain exact. A restricted implementation or services
/// profile that would be replaced requires an explicit governed policy transition first; this
/// constructor neither moves its height nor silently removes it.
/// # Errors
/// Unsupported reference/current selection, non-additive service changes, missing originals,
/// a restricted definition requiring replacement, or an implementation that changes more than
/// the baseline service/code closure.
pub fn service_change_set_for_selection(
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    reference: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, Descriptor>,
) -> Result<ChangeSetV1, CatalogError> {
    let catalog = super::release_catalog()?;
    service_change_set_in_catalog(
        catalog,
        network,
        height,
        qualification,
        previous,
        reference,
        published,
    )
}

fn service_change_set_in_catalog(
    base: &ReleaseCatalog,
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
    previous: &RootSelectionV1,
    reference: &RootSelectionV1,
    published: &BTreeMap<DefinitionId, Descriptor>,
) -> Result<ChangeSetV1, CatalogError> {
    let invalid = |message: &str| CatalogError::Parameters(message.into());
    if base.unsupported(previous).is_some() {
        return Err(invalid("current service predecessor is not compiled"));
    }
    previous.check_closure(published)?;
    reference.check_closure(&base.definitions)?;
    let reference_id = reference
        .bindings
        .first()
        .ok_or_else(|| invalid("service reference requires an active native bundle"))?
        .implementation;
    let mut normalized_selection = reference.clone();
    normalized_selection.services_profile = base.baseline.services_profile;
    normalized_selection.active.retain(|id| *id != reference_id);
    normalized_selection
        .active
        .push(base.baseline.bindings[0].implementation);
    normalized_selection.active.sort_unstable();
    for binding in &mut normalized_selection.bindings {
        if binding.implementation != reference_id {
            return Err(invalid("reference is not a service-only selection"));
        }
        binding.implementation = base.baseline.bindings[0].implementation;
    }
    if normalized_selection != base.baseline {
        return Err(invalid("reference is not a service-only selection"));
    }
    let Some(Descriptor::Implementation(reference_bundle)) = base.definitions.get(&reference_id)
    else {
        return Err(invalid(
            "service reference is not a compiled implementation",
        ));
    };
    let Descriptor::Implementation(baseline_bundle) =
        &base.definitions[&base.baseline.bindings[0].implementation]
    else {
        unreachable!()
    };
    let mut normalized = reference_bundle.clone();
    normalized.code = baseline_bundle.code.clone();
    normalized.services_profile = baseline_bundle.services_profile;
    if normalized != *baseline_bundle
        || reference_bundle.code != super::catalog::lifecycle_implementation_code()
    {
        return Err(invalid("reference is not a service-only native successor"));
    }
    let Descriptor::ServicesProfile(original) = &base.definitions[&base.baseline.services_profile]
    else {
        unreachable!()
    };
    let Descriptor::ServicesProfile(reference_services) =
        &base.definitions[&reference_bundle.services_profile]
    else {
        unreachable!()
    };
    if !original
        .services
        .iter()
        .all(|entry| reference_services.services.contains(entry))
    {
        return Err(invalid("service successor overlaps the original closures"));
    }
    let Some(Descriptor::ServicesProfile(mut services)) =
        published.get(&previous.services_profile).cloned()
    else {
        return Err(invalid("missing current services"));
    };
    for entry in reference_services
        .services
        .iter()
        .filter(|entry| !original.services.iter().any(|old| old.name == entry.name))
    {
        if services.services.iter().any(|old| old.name == entry.name) {
            return Err(invalid(
                "service extension is already selected or conflicts",
            ));
        }
        services.services.push(entry.clone());
    }
    services.services.sort_by(|a, b| a.name.cmp(&b.name));
    let mut catalog = base.clone();
    let services = catalog.publish(Descriptor::ServicesProfile(services))?;
    let old_bundle = previous
        .bindings
        .first()
        .ok_or_else(|| invalid("service composition requires an active native bundle"))?
        .implementation;
    if previous
        .bindings
        .iter()
        .any(|binding| binding.implementation != old_bundle)
    {
        return Err(invalid(
            "service composition requires the qualified native bundle",
        ));
    }
    let Some(Descriptor::Implementation(mut bundle)) = published.get(&old_bundle).cloned() else {
        return Err(invalid("current bundle is unpublished"));
    };
    bundle.services_profile = services;
    bundle.code = super::catalog::lifecycle_implementation_code();
    let implementation = catalog.publish(Descriptor::Implementation(bundle))?;
    let mut next = previous.clone();
    next.services_profile = services;
    next.active.retain(|id| *id != old_bundle);
    next.active.push(implementation);
    next.active.sort_unstable();
    for binding in &mut next.bindings {
        binding.implementation = implementation;
    }
    require_retained_restrictions(previous, &next)?;
    next.check_closure(&catalog.definitions)?;
    if base.unsupported(&next).is_some() {
        return Err(invalid("service composition is not compiled"));
    }
    let publish = catalog
        .definitions
        .iter()
        .filter(|(id, _)| !published.contains_key(id))
        .map(|(_, definition)| definition.clone())
        .collect();
    selection_change_set(network, height, qualification, previous, &next, publish)
}

/// Register only explicitly qualified service-only release references. Native descriptors
/// elsewhere in the catalog are not qualification evidence. Unsupported references are
/// skipped, never advertised, and cannot fail construction of the unrelated release catalog.
#[cfg(test)]
pub(super) fn register_service_compositions(
    catalog: &mut ReleaseCatalog,
    references: &[RootSelectionV1],
) -> Result<(), CatalogError> {
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"catalog-only-composition"),
    ));
    for previous in references {
        if service_change_set_in_catalog(
            catalog,
            network,
            1,
            [1; 32],
            &catalog.baseline,
            previous,
            &catalog.definitions,
        )
        .is_err()
        {
            continue;
        }
        let Ok(change) = activation_change_set_in_catalog(
            catalog,
            network,
            1,
            [1; 32],
            previous,
            &catalog.definitions,
        ) else {
            continue;
        };
        for definition in change.publish {
            catalog.publish(definition)?;
        }
    }
    Ok(())
}

/// Install the exact candidate composition in an isolated component fixture.
/// This grants no governance, deployment or finality authority. Governed activation is tested
/// separately; production callers cannot access this helper without the explicit test feature.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn install_for_test(world: &mut crate::state::World) {
    let catalog = super::release_catalog().expect("catalog");
    let extension = catalog.ironwood.as_ref().expect("Ironwood");
    world.runtime_definitions = catalog
        .definitions
        .iter()
        .map(|(id, descriptor)| (*id, descriptor.clone()))
        .collect();
    world.runtime_selection = mv::cell::Cell::new(extension.selection.clone());
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod test_activation;
