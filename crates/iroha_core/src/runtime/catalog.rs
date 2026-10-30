//! Definition catalog of this release (`specs/live_runtime_upgrades.md` §3.1).
//!
//! The catalog lists every definition this release can execute or install. Its first part is the
//! baseline: the engine of the foundation release registered as ordinary immutable definitions,
//! and the selection a new root starts from.
//!
//! - one storage definition per canonical World field of the authority registry;
//! - one interface per built-in instruction wire ID;
//! - one native bundle implementation that answers every baseline interface. Its handlers cannot
//!   yet be separated, so its dependencies are a conservative superset: all baseline storage,
//!   events, permissions and parameters, and every interface it may invoke. Decomposition later
//!   publishes new definitions; it never reinterprets the bundle;
//! - one event definition for the data events, one permission definition per Initial-executor
//!   permission name and one parameter definition for the chain parameters;
//! - the IVM host profile, the execution services profile, the commitment profile and the kernel.
//!
//! The baseline is pinned by `specs/runtime_baseline_catalog.json`. Once a running network has
//! installed it, it never changes: new behaviour is a new definition of the release catalog,
//! published and activated by a change set, and a changed World type, gas schedule or ABI is a new
//! storage or host-profile definition. Every identity depends only on declared schemas and fixed
//! labels, never on the build, so every build of the same source produces the same catalog.

use std::{collections::BTreeMap, sync::OnceLock};

use iroha_data_model::runtime::definition::{
    BindingV1, CommitmentProfileDescriptorV1, DefinitionDescriptorV1, DefinitionError,
    DefinitionId, EntrypointV1, EventDescriptorV1, HostProfileDescriptorV1, ImplementationCodeV1,
    ImplementationDescriptorV1, InterfaceDescriptorV1, KernelDescriptorV1, ParameterDescriptorV1,
    PermissionDescriptorV1, ProjectionV1, ResourceLimitsV1, RootSelectionV1, SchemaV1,
    SelectionError, SelectionId, ServiceEntryV1, ServicesProfileDescriptorV1, StorageDescriptorV1,
    StorageShapeV1,
};

use crate::state::authority_registry::{
    Canonical, CanonicalLayout, Field, Role, Schema, WORLD_FIELDS,
};

/// Label of every baseline reference closure and statement digest.
///
/// A native definition's reference closure is the archived source snapshot it was activated with
/// (§3.1). The release that founds or adopts a root attests which archive each label denotes; the
/// label keeps the identity independent of the build.
const BASELINE_LABEL: &[u8] = b"iroha:runtime:baseline:v1\0";
/// Native module of the baseline bundle.
pub(crate) const BASELINE_MODULE: &str = "iroha.baseline";
/// Commitment scheme of the baseline commitment profile.
pub(crate) const BASELINE_COMMITMENT_SCHEME: &str = "iroha.world.lthash16.v1";
/// Metering class of every baseline interface: the engine's native gas schedule.
const BASELINE_METERING: &str = "native.baseline";
/// Root-wide services of the baseline engine, sorted.
const BASELINE_SERVICES: &[&str] = &[
    "authorization",
    "events",
    "fees",
    "hooks",
    "metering",
    "queries",
    "recursion",
];

/// The definitions of this release and the baseline selection.
#[derive(Clone, Debug)]
pub struct ReleaseCatalog {
    /// Every definition of this release by identity.
    pub definitions: BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    /// The baseline's definitions, sorted: what a founded root publishes.
    pub baseline_definitions: Vec<DefinitionId>,
    /// The selection that founds a root.
    pub baseline: RootSelectionV1,
    /// Identity of [`Self::baseline`].
    pub baseline_id: SelectionId,
    /// The storage definition each compiled canonical World field implements, by registry
    /// identity.
    pub world_storage: BTreeMap<&'static str, DefinitionId>,
}

/// A catalog that cannot be built: a build defect the catalog tests catch.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// A canonical World field still has an unresolved schema.
    #[error("canonical World field {0} has no resolved schema")]
    UnresolvedSchema(&'static str),
    /// A parameter's initial value cannot be encoded.
    #[error("baseline parameters cannot be encoded: {0}")]
    Parameters(String),
    /// A descriptor is invalid.
    #[error(transparent)]
    Definition(#[from] DefinitionError),
    /// The baseline selection is not closed.
    #[error(transparent)]
    Selection(#[from] SelectionError),
}

/// The catalog of this release, built once.
///
/// # Errors
/// The catalog cannot be built (a build defect).
pub fn release_catalog() -> Result<&'static ReleaseCatalog, CatalogError> {
    static CATALOG: OnceLock<Result<ReleaseCatalog, CatalogError>> = OnceLock::new();
    CATALOG
        .get_or_init(ReleaseCatalog::build)
        .as_ref()
        .map_err(Clone::clone)
}

/// `H(BASELINE_LABEL ‖ subject)`: the fixed digest a baseline descriptor binds for `subject`.
fn baseline_digest(subject: &[&[u8]]) -> [u8; 32] {
    let mut chunks = Vec::with_capacity(subject.len() + 1);
    chunks.push(BASELINE_LABEL);
    chunks.extend_from_slice(subject);
    iroha_crypto::Hash::new_from_chunks(&chunks).into()
}

fn layout_suffix(layout: CanonicalLayout) -> String {
    format!("{}.{}.{}", layout.major, layout.minor, layout.flags)
}

/// `norito:<nominal>@<layout>` or `semantic:<identity>@<layout>` (`sumeragi.md` §16.1).
fn codec(schema: Schema, field: &'static str) -> Result<SchemaV1, CatalogError> {
    match schema {
        Schema::Norito {
            nominal_name,
            layout,
        } => Ok(SchemaV1::Codec(format!(
            "norito:{}@{}",
            nominal_name(),
            layout_suffix(layout)
        ))),
        Schema::Semantic {
            identity, layout, ..
        } => Ok(SchemaV1::Codec(format!(
            "semantic:{identity}@{}",
            layout_suffix(layout)
        ))),
        Schema::Required { .. } => Err(CatalogError::UnresolvedSchema(field)),
    }
}

fn projection(schema: Schema) -> ProjectionV1 {
    match schema {
        Schema::Semantic { identity, .. } => ProjectionV1::Semantic(identity.to_owned()),
        Schema::Norito { .. } | Schema::Required { .. } => ProjectionV1::BareNorito,
    }
}

/// The storage definition a canonical World field implements, if it is canonical.
fn world_storage(field: &'static Field) -> Result<Option<StorageDescriptorV1>, CatalogError> {
    let (shape, value) = match field.role {
        Role::Canonical(Canonical::Table { key, value }) => (
            StorageShapeV1::Table {
                key: codec(key, field.id)?,
            },
            value,
        ),
        Role::Canonical(Canonical::Cell(value)) => (StorageShapeV1::Cell, value),
        Role::Canonical(Canonical::Owner(_))
        | Role::Derived { .. }
        | Role::History { .. }
        | Role::Local(_) => return Ok(None),
    };
    Ok(Some(StorageDescriptorV1 {
        namespace: field.id.to_owned(),
        shape,
        value: codec(value, field.id)?,
        // A baseline cell holds what the genesis or adoption that founds the root wrote.
        initial: None,
        invariants: baseline_digest(&[b"storage", field.id.as_bytes()]),
        projection: projection(value),
    }))
}

fn flatten(fields: &'static [Field], out: &mut Vec<&'static Field>) {
    for field in fields {
        out.push(field);
        if let Role::Canonical(Canonical::Owner(children)) = field.role {
            flatten(children, out);
        }
    }
}

fn sorted(mut ids: Vec<DefinitionId>) -> Vec<DefinitionId> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

impl ReleaseCatalog {
    fn publish(
        &mut self,
        descriptor: DefinitionDescriptorV1,
    ) -> Result<DefinitionId, CatalogError> {
        let id = descriptor.id()?;
        self.definitions.insert(id, descriptor);
        Ok(id)
    }

    fn build() -> Result<Self, CatalogError> {
        let mut catalog = Self {
            definitions: BTreeMap::new(),
            baseline_definitions: Vec::new(),
            baseline: RootSelectionV1 {
                kernel: DefinitionId([0; 32]),
                commitment_profile: DefinitionId([0; 32]),
                services_profile: DefinitionId([0; 32]),
                installed_storage: Vec::new(),
                active: Vec::new(),
                bindings: Vec::new(),
                restrictions: Vec::new(),
                completion_closures: Vec::new(),
            },
            baseline_id: SelectionId([0; 32]),
            world_storage: BTreeMap::new(),
        };

        let kernel = catalog.publish(DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
            name: "iroha.kernel.v1".into(),
            reference_closure: baseline_digest(&[b"kernel"]),
        }))?;
        let commitment_profile = catalog.publish(DefinitionDescriptorV1::CommitmentProfile(
            CommitmentProfileDescriptorV1 {
                scheme: BASELINE_COMMITMENT_SCHEME.into(),
                parameters: crate::state::world_projection::world_state_accumulator::commitment_parameters(),
            },
        ))?;
        let services_profile = catalog.publish(DefinitionDescriptorV1::ServicesProfile(
            ServicesProfileDescriptorV1 {
                services: BASELINE_SERVICES
                    .iter()
                    .map(|name| ServiceEntryV1 {
                        name: (*name).to_owned(),
                        reference_closure: baseline_digest(&[b"service", name.as_bytes()]),
                    })
                    .collect(),
            },
        ))?;
        let host_profile = catalog.publish(DefinitionDescriptorV1::HostProfile(
            HostProfileDescriptorV1 {
                abi_version: 1,
                abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
                metering: ivm::gas::schedule_hash().into(),
            },
        ))?;

        let mut fields = Vec::new();
        flatten(WORLD_FIELDS, &mut fields);
        let mut installed = Vec::new();
        for field in fields {
            if let Some(storage) = world_storage(field)? {
                let id = catalog.publish(DefinitionDescriptorV1::Storage(storage))?;
                catalog.world_storage.insert(field.id, id);
                installed.push(id);
            }
        }
        let installed = sorted(installed);

        let layout = layout_suffix(crate::state::authority_registry::V1_LAYOUT);
        let event = catalog.publish(DefinitionDescriptorV1::Event(EventDescriptorV1 {
            name: "iroha.events.data.v1".into(),
            schema: SchemaV1::Codec(format!(
                "norito:{}@{layout}",
                norito::schema::identity::nominal_name::<iroha_data_model::events::data::DataEvent>(
                )
            )),
        }))?;
        let permission_payload = SchemaV1::Codec(format!(
            "norito:{}@{layout}",
            norito::schema::identity::nominal_name::<iroha_primitives::json::Json>()
        ));
        let mut permissions = Vec::new();
        for name in crate::executor::INITIAL_EXECUTOR_PERMISSION_NAMES {
            permissions.push(catalog.publish(DefinitionDescriptorV1::Permission(
                PermissionDescriptorV1 {
                    name: (*name).to_owned(),
                    payload: permission_payload.clone(),
                },
            ))?);
        }
        let permissions = sorted(permissions);
        let parameter =
            catalog.publish(DefinitionDescriptorV1::Parameter(ParameterDescriptorV1 {
                name: "iroha.parameters.v1".into(),
                schema: SchemaV1::Codec(format!(
                    "norito:{}@{layout}",
                    norito::schema::identity::nominal_name::<
                        iroha_data_model::parameter::system::Parameters,
                    >()
                )),
                initial: norito::encode_canonical(
                    &iroha_data_model::parameter::system::Parameters::default(),
                )
                .map_err(|error| CatalogError::Parameters(error.to_string()))?,
            }))?;

        let mut interfaces = BTreeMap::new();
        for instruction in iroha_data_model::isi::registry::built_in_instruction_schemas() {
            let interface =
                catalog.publish(DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
                    wire_id: instruction.wire_id.to_owned(),
                    payload: SchemaV1::Codec(format!(
                        "norito:{}@{layout}",
                        (instruction.payload)()
                    )),
                    rejection_codes: Vec::new(),
                    events: vec![event],
                    metering_class: BASELINE_METERING.into(),
                    contract_statement: baseline_digest(&[
                        b"interface",
                        instruction.wire_id.as_bytes(),
                    ]),
                }))?;
            interfaces.insert(instruction.wire_id, interface);
        }
        let interface_ids = sorted(interfaces.values().copied().collect());

        let bundle = catalog.publish(DefinitionDescriptorV1::Implementation(
            ImplementationDescriptorV1 {
                entrypoint: EntrypointV1::Instructions(interface_ids.clone()),
                code: ImplementationCodeV1::Native {
                    module: BASELINE_MODULE.into(),
                    reference_closure: baseline_digest(&[
                        b"implementation",
                        BASELINE_MODULE.as_bytes(),
                    ]),
                },
                reads: installed.clone(),
                writes: installed.clone(),
                scans: installed.clone(),
                events: vec![event],
                permissions: permissions.clone(),
                parameters: vec![parameter],
                invokes: interface_ids.clone(),
                host_profile: None,
                services_profile,
                limits: ResourceLimitsV1 {
                    max_gas: u64::MAX,
                    max_effects: u32::MAX,
                    max_call_depth: u16::MAX,
                },
            },
        ))?;

        let mut active = vec![event, parameter, host_profile, bundle];
        active.extend(permissions);
        active.extend(interface_ids);
        catalog.baseline = RootSelectionV1 {
            kernel,
            commitment_profile,
            services_profile,
            installed_storage: installed,
            active: sorted(active),
            bindings: interfaces
                .into_iter()
                .map(|(wire_id, interface)| BindingV1 {
                    wire_id: wire_id.to_owned(),
                    interface,
                    implementation: bundle,
                })
                .collect(),
            restrictions: Vec::new(),
            completion_closures: Vec::new(),
        };
        catalog.baseline.check_closure(&catalog.definitions)?;
        catalog.baseline_id = catalog.baseline.id()?;
        // Everything published so far is the baseline; later release definitions follow it.
        catalog.baseline_definitions = catalog.definitions.keys().copied().collect();
        Ok(catalog)
    }

    /// The first definition `selection` relies on that this release cannot execute, if any.
    ///
    /// Definitions are content-addressed, so a release executes a definition exactly when its
    /// catalog holds that identity. A node whose release does not support the selection at a
    /// height stops before that height instead of executing it (§6, "Stop the unsupported").
    #[must_use]
    pub fn unsupported(&self, selection: &RootSelectionV1) -> Option<DefinitionId> {
        [
            selection.kernel,
            selection.commitment_profile,
            selection.services_profile,
        ]
        .into_iter()
        .chain(selection.installed_storage.iter().copied())
        .chain(selection.active.iter().copied())
        .find(|id| !self.definitions.contains_key(id))
    }
}

#[cfg(test)]
mod tests;
