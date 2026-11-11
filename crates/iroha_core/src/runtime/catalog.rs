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
//!
//! Its second part is what the release adds after the baseline ([`additions`]): instructions and
//! World fields compiled into this release but not into the baseline, each addition published as
//! its own native implementation, interfaces and storage. They are neither installed nor active
//! until a governed change set ([`ReleaseCatalog::activation`]) moves the root to them.

pub(crate) mod additions;
pub mod migrations;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use iroha_data_model::{
    NetworkId,
    runtime::{
        definition::{
            BindingV1, CommitmentProfileDescriptorV1, DefinitionDescriptorV1, DefinitionError,
            DefinitionId, DefinitionLookup, EntrypointV1, EventDescriptorV1,
            HostProfileDescriptorV1, ImplementationCodeV1, ImplementationDescriptorV1,
            InterfaceDescriptorV1, KernelDescriptorV1, MigrationDescriptorV1,
            ParameterDescriptorV1, PermissionDescriptorV1, ProjectionV1, ResourceLimitsV1,
            RestrictionEntryV1, RootSelectionV1, SchemaV1, SelectionError, SelectionId,
            ServiceEntryV1, ServicesProfileDescriptorV1, StorageDescriptorV1, StorageShapeV1,
        },
        lifecycle::{ChangeSetV1, RestrictionKindV1, RootScopeV1, SelectionTransitionV1},
    },
};

use self::{
    additions::{Addition, CellInstaller, StorageAddition},
    migrations::MigrationAddition,
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
/// Label of every reference closure and statement digest of an addition after the baseline,
/// followed by the addition's module.
const ADDITION_LABEL: &[u8] = b"iroha:runtime:addition:v1\0";
/// Native module of the baseline bundle.
pub(crate) const BASELINE_MODULE: &str = "iroha.baseline";
/// Successor native bundle: complete trigger lifecycle capture and V2 output.
pub(crate) const LIFECYCLE_MODULE: &str = iroha_data_model::runtime::TRIGGER_LIFECYCLE_MODULE;
/// The one native code identity used by standalone and composed lifecycle bundles.
/// The enclosing descriptor commits its exact services and dependencies, so an Ironwood
/// composition has a distinct implementation identity without inventing a second module.
pub(crate) fn lifecycle_implementation_code() -> ImplementationCodeV1 {
    ImplementationCodeV1::Native {
        module: LIFECYCLE_MODULE.into(),
        reference_closure: iroha_crypto::Hash::new(LIFECYCLE_MODULE.as_bytes()).into(),
    }
}

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
    /// Successor selection with complete native lifecycle output; never founded implicitly.
    pub lifecycle: RootSelectionV1,
    /// Identity of the successor selection, installed only by governed change sets.
    pub lifecycle_id: SelectionId,
    /// The storage definition each compiled canonical World field implements, by registry
    /// identity.
    pub world_storage: BTreeMap<&'static str, DefinitionId>,
    /// Existing State-level authority owners available to the keyed commitment profile.
    pub(crate) keyed_storage: Vec<DefinitionId>,
    /// What the release adds after the baseline, in activation order.
    pub additions: Vec<AdditionDefinitions>,
    /// How each cell an addition adds is installed, by storage definition.
    cell_installers: BTreeMap<DefinitionId, CellInstaller>,
    /// The native code of every migration definition this release compiles: one definition per
    /// migration and execution services profile.
    migrations: BTreeMap<DefinitionId, MigrationAddition>,
    /// Exact compiled predecessor implementations and their restricted-read successors.
    pub(super) restricted_read_compositions: BTreeMap<DefinitionId, AdditionDefinitions>,
    /// Additive Ironwood definitions and the governed successor.
    pub ironwood: Option<super::ironwood::IronwoodCatalog>,
}

/// The definitions one addition after the baseline publishes ([`additions`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdditionDefinitions {
    /// Native module of its implementation.
    pub module: &'static str,
    /// Its implementation.
    pub implementation: DefinitionId,
    /// The interfaces it adds, by wire ID.
    pub interfaces: BTreeMap<&'static str, DefinitionId>,
    /// The earlier interfaces it rebinds to itself, by wire ID.
    pub rebinds: BTreeMap<&'static str, DefinitionId>,
    /// The storage it adds, by World field.
    pub storage: BTreeMap<&'static str, DefinitionId>,
    /// A separately identified kernel published by a narrowly scoped recovery addition.
    /// Publishing it does not select it; the exact repair change set does.
    pub kernel: Option<DefinitionId>,
}

impl AdditionDefinitions {
    /// Every definition the addition publishes, sorted.
    #[must_use]
    pub fn definitions(&self) -> Vec<DefinitionId> {
        sorted(
            self.storage
                .values()
                .chain(self.interfaces.values())
                .copied()
                .chain([self.implementation])
                .chain(self.kernel)
                .collect(),
        )
    }
}

/// Whether a wire ID belongs to the immutable foundation roster.
pub(crate) fn is_baseline_instruction(wire_id: &str) -> bool {
    use iroha_data_model::isi::{
        privacy::{
            BootstrapPrivacyIronwoodPoolV1, SchedulePrivacyIronwoodLimitsUpdateV1,
            SubmitPrivacyIronwoodProofV1,
        },
        taira_activation::ScheduleTairaTimeIronwoodV1,
    };
    !matches!(
        wire_id,
        BootstrapPrivacyIronwoodPoolV1::WIRE_ID
            | SubmitPrivacyIronwoodProofV1::WIRE_ID
            | SchedulePrivacyIronwoodLimitsUpdateV1::WIRE_ID
            | ScheduleTairaTimeIronwoodV1::WIRE_ID
    )
}

/// One governed step of a migration (§5.4), each its own change set
/// ([`ReleaseCatalog::migration_step`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationStep {
    /// Install the successor storage and activate the migration.
    Start,
    /// Make the successor authoritative and start draining the old storage.
    CutOver,
    /// Retire the drained old storage and deactivate the migration.
    Retire,
}

/// A catalog that cannot be built: a build defect the catalog tests catch.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    /// An addition after the baseline is malformed.
    #[error("addition {module}: {reason}")]
    Addition {
        /// Its module.
        module: String,
        /// What is wrong.
        reason: String,
    },
    /// The selected services have no matching executable semantics in this release.
    #[error("runtime services profile {0} is unsupported")]
    UnsupportedServices(DefinitionId),
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
    #[cfg(test)]
    if let Some(catalog) = test_catalog::current() {
        return Ok(catalog);
    }
    CATALOG
        .get_or_init(ReleaseCatalog::build)
        .as_ref()
        .map_err(Clone::clone)
}

/// A catalog with other additions, standing in for [`release_catalog`] on one test thread.
#[cfg(test)]
pub(crate) mod test_catalog {
    use std::cell::Cell;

    use super::{Addition, ReleaseCatalog};

    thread_local! {
        static CURRENT: Cell<Option<&'static ReleaseCatalog>> = const { Cell::new(None) };
    }

    /// This thread's stand-in catalog, if any.
    pub(super) fn current() -> Option<&'static ReleaseCatalog> {
        CURRENT.with(Cell::get)
    }

    /// Run `body` with the catalog of a release whose additions are `additions` as this thread's
    /// release catalog. Code that runs on other threads still sees the real catalog.
    pub fn with_additions<T>(additions: &[Addition], body: impl FnOnce() -> T) -> T {
        with_release(additions, &[], body)
    }

    /// Run `body` with the catalog of a release that adds `additions` and compiles `migrations`
    /// as this thread's release catalog.
    pub fn with_release<T>(
        additions: &[Addition],
        migrations: &[super::MigrationAddition],
        body: impl FnOnce() -> T,
    ) -> T {
        struct Restore(Option<&'static ReleaseCatalog>);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT.with(|current| current.set(self.0));
            }
        }
        let catalog: &'static ReleaseCatalog = Box::leak(Box::new(
            ReleaseCatalog::build_with(additions, migrations).expect("test catalog"),
        ));
        let _restore = Restore(CURRENT.with(|current| current.replace(Some(catalog))));
        body()
    }
}

/// `H(BASELINE_LABEL ‖ subject)`: the fixed digest a baseline descriptor binds for `subject`.
fn baseline_digest(subject: &[&[u8]]) -> [u8; 32] {
    let mut chunks = Vec::with_capacity(subject.len() + 1);
    chunks.push(BASELINE_LABEL);
    chunks.extend_from_slice(subject);
    iroha_crypto::Hash::new_from_chunks(&chunks).into()
}

/// `H(ADDITION_LABEL ‖ module ‖ 0 ‖ subject)`: the fixed digest a descriptor of the addition
/// `module` binds for `subject`.
fn addition_digest(module: &str, subject: &[&[u8]]) -> [u8; 32] {
    let mut chunks = Vec::with_capacity(subject.len() + 3);
    chunks.push(ADDITION_LABEL);
    chunks.push(module.as_bytes());
    chunks.push(&[0]);
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

/// Baseline definitions every addition shares: like the baseline bundle, an addition emits the
/// data events, checks the baseline permissions, reads the chain parameters and runs under the
/// baseline services.
struct Shared {
    event: DefinitionId,
    parameter: DefinitionId,
    permissions: Vec<DefinitionId>,
    services_profile: DefinitionId,
    layout: String,
}

/// The compiled instructions and World fields the additions claim, which the baseline leaves out.
struct Claimed {
    instructions: BTreeSet<&'static str>,
    fields: BTreeSet<&'static str>,
}

impl Claimed {
    /// What `additions` claim.
    ///
    /// # Errors
    /// A module is the baseline's or repeated, an addition answers nothing, or two additions
    /// claim the same instruction or field.
    fn of(additions: &[Addition]) -> Result<Self, CatalogError> {
        let mut modules = BTreeSet::new();
        let mut claimed = Self {
            instructions: BTreeSet::new(),
            fields: BTreeSet::new(),
        };
        for addition in additions {
            let fail = |reason: String| CatalogError::Addition {
                module: addition.module.to_owned(),
                reason,
            };
            if addition.module == BASELINE_MODULE || !modules.insert(addition.module) {
                return Err(fail("its module is the baseline's or repeated".to_owned()));
            }
            if addition.instructions.is_empty() && addition.rebinds.is_empty() {
                return Err(fail("it answers no instruction".to_owned()));
            }
            for wire_id in addition.instructions {
                if !claimed.instructions.insert(*wire_id) {
                    return Err(fail(format!("{wire_id} is claimed twice")));
                }
            }
            for entry in addition.storage {
                if !claimed.fields.insert(entry.field()) {
                    return Err(fail(format!("{} is claimed twice", entry.field())));
                }
            }
        }
        Ok(claimed)
    }
}

impl ReleaseCatalog {
    pub(super) fn publish(
        &mut self,
        descriptor: DefinitionDescriptorV1,
    ) -> Result<DefinitionId, CatalogError> {
        let id = descriptor.id()?;
        self.definitions.insert(id, descriptor);
        Ok(id)
    }

    fn build() -> Result<Self, CatalogError> {
        Self::build_with(additions::ADDITIONS, migrations::MIGRATIONS)
    }

    /// The catalog of a release that adds `additions` after the baseline and compiles no
    /// migration ([`Self::build_with`]).
    ///
    /// # Errors
    /// See [`Self::build_with`].
    #[cfg(test)]
    pub(crate) fn build_from(additions: &[Addition]) -> Result<Self, CatalogError> {
        Self::build_with(additions, &[])
    }

    /// The catalog of a release that adds `additions` after the baseline and compiles
    /// `migrations`: everything compiled that no addition claims is the baseline.
    ///
    /// # Errors
    /// A definition is invalid, the baseline is not closed, an addition claims something that
    /// is not compiled, claims it twice, rebinds a wire ID nothing before it answers or reuses a
    /// module, or a migration is malformed.
    pub(crate) fn build_with(
        additions: &[Addition],
        migrations: &[MigrationAddition],
    ) -> Result<Self, CatalogError> {
        let claimed = Claimed::of(additions)?;
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
            lifecycle: RootSelectionV1 {
                kernel: DefinitionId([0; 32]),
                commitment_profile: DefinitionId([0; 32]),
                services_profile: DefinitionId([0; 32]),
                installed_storage: Vec::new(),
                active: Vec::new(),
                bindings: Vec::new(),
                restrictions: Vec::new(),
                completion_closures: Vec::new(),
            },
            lifecycle_id: SelectionId([0; 32]),
            world_storage: BTreeMap::new(),
            keyed_storage: Vec::new(),
            additions: Vec::new(),
            cell_installers: BTreeMap::new(),
            migrations: BTreeMap::new(),
            restricted_read_compositions: BTreeMap::new(),
            ironwood: None,
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
        let mut canonical_fields = BTreeMap::new();
        for field in fields {
            if let Some(storage) = world_storage(field)? {
                canonical_fields.insert(field.id, (field, storage.clone()));
                if claimed.fields.contains(field.id) {
                    continue;
                }
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
        let mut payloads = BTreeMap::new();
        for instruction in iroha_data_model::isi::registry::built_in_instruction_schemas() {
            payloads.insert(instruction.wire_id, (instruction.payload)());
            if claimed.instructions.contains(instruction.wire_id)
                || !is_baseline_instruction(instruction.wire_id)
            {
                continue;
            }
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
        active.extend(permissions.iter().copied());
        active.extend(interface_ids);
        catalog.baseline = RootSelectionV1 {
            kernel,
            commitment_profile,
            services_profile,
            installed_storage: installed,
            active: sorted(active),
            bindings: interfaces
                .iter()
                .map(|(wire_id, interface)| BindingV1 {
                    wire_id: (*wire_id).to_owned(),
                    interface: *interface,
                    implementation: bundle,
                })
                .collect(),
            restrictions: Vec::new(),
            completion_closures: Vec::new(),
        };
        catalog.baseline.check_closure(&catalog.definitions)?;
        catalog.baseline_id = catalog.baseline.id()?;
        // Everything published so far is the baseline; what the release adds follows it.
        catalog.baseline_definitions = catalog.definitions.keys().copied().collect();

        let shared = Shared {
            event,
            parameter,
            permissions,
            services_profile,
            layout,
        };
        let baseline_storage = catalog.baseline.installed_storage.clone();
        let mut storage_so_far = baseline_storage.clone();
        let mut interfaces_so_far = interfaces;
        for addition in additions {
            let independent_interfaces;
            let (storage_dependencies, interface_dependencies) = if addition.module
                == additions::SORAFS_REPAIR.module
                || addition.module == additions::SORAFS_AUTHORITY.module
                || addition.module == additions::RESTRICTED_DATASPACE_READ.module
                || addition.module == additions::RUNTIME_LIFECYCLE.module
            {
                independent_interfaces = catalog
                    .baseline
                    .bindings
                    .iter()
                    .map(|binding| {
                        let wire = payloads
                            .keys()
                            .copied()
                            .find(|wire| *wire == binding.wire_id)
                            .expect("baseline binding has a registered wire ID");
                        (wire, binding.interface)
                    })
                    .collect();
                (&baseline_storage, &independent_interfaces)
            } else {
                (&storage_so_far, &interfaces_so_far)
            };
            let published = catalog.publish_addition(
                addition,
                &shared,
                &canonical_fields,
                &payloads,
                storage_dependencies,
                interface_dependencies,
            )?;
            storage_so_far.extend(published.storage.values().copied());
            storage_so_far.sort_unstable();
            interfaces_so_far.extend(published.interfaces.iter().map(|(id, def)| (*id, *def)));
            catalog.additions.push(published);
        }
        catalog.ironwood = Some(super::ironwood::build_catalog(&mut catalog)?);
        let baseline = catalog.baseline.clone();
        catalog.lifecycle = catalog.lifecycle_successor(&baseline)?;
        catalog.lifecycle_id = catalog.lifecycle.id()?;
        catalog.compile_lifecycle_compositions()?;
        super::sorafs_authority::compile(&mut catalog)?;
        super::restricted_dataspace_read::compile(&mut catalog)?;
        let mut state_fields = Vec::new();
        flatten(
            crate::state::authority_registry::STATE_FIELDS,
            &mut state_fields,
        );
        for field in state_fields {
            if (field.id.starts_with("state.") || field.id.starts_with("runtime."))
                && let Some(storage) = world_storage(field)?
            {
                let id = catalog.publish(DefinitionDescriptorV1::Storage(storage))?;
                catalog.keyed_storage.push(id);
            }
        }
        catalog.keyed_storage.sort_unstable();
        catalog.publish(iroha_data_model::runtime::keyed_state::adoption_kernel(
            &catalog.keyed_storage,
        ))?;
        catalog.publish(super::keyed_state::descriptor())?;
        catalog.publish(super::state_row_permission::descriptor())?;
        catalog.publish_migrations(migrations)?;

        Ok(catalog)
    }

    /// Publish one migration definition per migration of `migrations` and execution services
    /// profile this catalog compiles: a migration relies on the services profile of the root
    /// that runs it (§3.3), so a root running any compiled profile can activate it.
    fn publish_migrations(&mut self, migrations: &[MigrationAddition]) -> Result<(), CatalogError> {
        let services: Vec<DefinitionId> = self
            .definitions
            .iter()
            .filter(|(_, descriptor)| {
                matches!(descriptor, DefinitionDescriptorV1::ServicesProfile(_))
            })
            .map(|(id, _)| *id)
            .collect();
        let mut modules = BTreeSet::new();
        for migration in migrations {
            let module = migration.module;
            let fail = |reason: String| CatalogError::Addition {
                module: module.to_owned(),
                reason,
            };
            if module == BASELINE_MODULE
                || !modules.insert(module)
                || self
                    .additions
                    .iter()
                    .any(|addition| addition.module == module)
            {
                return Err(fail("its module is the baseline's or repeated".to_owned()));
            }
            let storage = |fields: &[&'static str]| {
                fields
                    .iter()
                    .map(|field| {
                        self.world_storage.get(field).copied().ok_or_else(|| {
                            fail(format!("{field} is not a canonical World table or cell"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(sorted)
            };
            let (from, to) = (storage(migration.from)?, storage(migration.to)?);
            if from.len() != migration.from.len()
                || to.len() != migration.to.len()
                || from.is_empty()
                || to.is_empty()
                || from.iter().any(|storage| to.binary_search(storage).is_ok())
            {
                return Err(fail(
                    "its old and successor storage must be distinct, non-empty and disjoint"
                        .to_owned(),
                ));
            }
            if migration.rows_per_stage == 0 {
                return Err(fail("a stage must move at least one row".to_owned()));
            }
            for services_profile in &services {
                let id =
                    self.publish(DefinitionDescriptorV1::Migration(MigrationDescriptorV1 {
                        code: ImplementationCodeV1::Native {
                            module: module.into(),
                            reference_closure: addition_digest(module, &[b"migration"]),
                        },
                        from: from.clone(),
                        to: to.clone(),
                        cursor: SchemaV1::Codec(migration.cursor.to_owned()),
                        host_profile: None,
                        services_profile: *services_profile,
                        limits: ResourceLimitsV1 {
                            max_gas: u64::MAX,
                            max_effects: migration.rows_per_stage,
                            max_call_depth: 1,
                        },
                    }))?;
                self.migrations.insert(id, *migration);
            }
        }
        Ok(())
    }

    /// The native code of the migration definition `id`, if this release compiles it.
    #[must_use]
    pub(crate) fn migration(&self, id: &DefinitionId) -> Option<&MigrationAddition> {
        self.migrations.get(id)
    }

    /// The definition of the migration `module` a root running `selection` activates: the one
    /// that relies on the root's execution services profile.
    ///
    /// # Errors
    /// `module` is not a migration of this release, or this release does not compile it for
    /// the root's services profile.
    pub fn migration_definition(
        &self,
        module: &str,
        selection: &RootSelectionV1,
    ) -> Result<(DefinitionId, &MigrationDescriptorV1), CatalogError> {
        self.migrations
            .iter()
            .filter(|(_, migration)| migration.module == module)
            .find_map(|(id, _)| match self.definitions.get(id) {
                Some(DefinitionDescriptorV1::Migration(descriptor))
                    if descriptor.services_profile == selection.services_profile =>
                {
                    Some((*id, descriptor))
                }
                _ => None,
            })
            .ok_or_else(|| CatalogError::Addition {
                module: module.to_owned(),
                reason: "not a migration this release compiles for the root's services profile"
                    .to_owned(),
            })
    }

    /// Draft the change set of one step of the migration `module` on a root running `current`,
    /// effective at `effective_height` (§5.4):
    ///
    /// - [`MigrationStep::Start`] publishes what the root lacks, installs the successor storage
    ///   and activates the migration, whose stages then copy the old storage;
    /// - [`MigrationStep::CutOver`] restricts the migration to completion only: from that
    ///   height the successor is authoritative and stages drain the old storage. A release adds
    ///   to this change set the transitions that move its readers and writers to the successor;
    /// - [`MigrationStep::Retire`] retires the drained old storage and deactivates the
    ///   migration.
    ///
    /// # Errors
    /// `module` is not a migration of this release, or the step does not apply to `current`.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is one independent input of the drafted change set"
    )]
    pub fn migration_step(
        &self,
        module: &str,
        step: MigrationStep,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        let fail = |reason: String| CatalogError::Addition {
            module: module.to_owned(),
            reason,
        };
        let (migration, descriptor) = self.migration_definition(module, current)?;
        let mut successor = current.clone();
        let mut publish = Vec::new();
        let mut transitions = Vec::new();
        match step {
            MigrationStep::Start => {
                let install: Vec<DefinitionId> = descriptor
                    .to
                    .iter()
                    .copied()
                    .filter(|storage| current.installed_storage.binary_search(storage).is_err())
                    .collect();
                publish = sorted(
                    install
                        .iter()
                        .copied()
                        .chain([migration])
                        .filter(|id| published.definition(id).is_none())
                        .collect(),
                )
                .into_iter()
                .map(|id| self.definitions[&id].clone())
                .collect();
                transitions.extend(
                    install
                        .iter()
                        .map(|storage| SelectionTransitionV1::InstallStorage(*storage)),
                );
                transitions.push(SelectionTransitionV1::Activate(migration));
                successor.installed_storage.extend(install);
                successor.installed_storage = sorted(successor.installed_storage);
                successor.active.push(migration);
                successor.active = sorted(successor.active);
            }
            MigrationStep::CutOver => {
                transitions.push(SelectionTransitionV1::Restrict {
                    definition: migration,
                    restriction: RestrictionKindV1::CompletionOnly,
                });
                successor
                    .restrictions
                    .retain(|entry| entry.definition != migration);
                successor.restrictions.push(RestrictionEntryV1 {
                    definition: migration,
                    restriction: RestrictionKindV1::CompletionOnly.at(effective_height),
                });
                successor.restrictions.sort_by_key(|entry| entry.definition);
            }
            MigrationStep::Retire => {
                transitions.extend(
                    descriptor
                        .from
                        .iter()
                        .map(|storage| SelectionTransitionV1::RetireStorage(*storage)),
                );
                transitions.push(SelectionTransitionV1::Restrict {
                    definition: migration,
                    restriction: RestrictionKindV1::Disabled,
                });
                transitions.push(SelectionTransitionV1::Deactivate(migration));
                successor
                    .installed_storage
                    .retain(|storage| descriptor.from.binary_search(storage).is_err());
                successor.active.retain(|id| *id != migration);
                successor
                    .restrictions
                    .retain(|entry| entry.definition != migration);
            }
        }
        let change_set = ChangeSetV1 {
            network,
            root: RootScopeV1::Universal,
            predecessor: current.id()?,
            successor: successor.id()?,
            effective_height,
            publish,
            transitions,
            qualification,
        };
        change_set
            .apply(current, published)
            .map_err(|error| fail(error.to_string()))?;
        Ok(change_set)
    }

    /// Publish `addition`'s storage, interfaces and implementation after the baseline and every
    /// earlier addition, whose storage is `storage_so_far` and whose interfaces are
    /// `interfaces_so_far`.
    fn publish_addition(
        &mut self,
        addition: &Addition,
        shared: &Shared,
        canonical_fields: &BTreeMap<&'static str, (&'static Field, StorageDescriptorV1)>,
        payloads: &BTreeMap<&'static str, std::borrow::Cow<'static, str>>,
        storage_so_far: &[DefinitionId],
        interfaces_so_far: &BTreeMap<&'static str, DefinitionId>,
    ) -> Result<AdditionDefinitions, CatalogError> {
        let module = addition.module;
        let fail = |reason: String| CatalogError::Addition {
            module: module.to_owned(),
            reason,
        };
        let mut storage = BTreeMap::new();
        for entry in addition.storage {
            let field = entry.field();
            let Some((_, descriptor)) = canonical_fields.get(field) else {
                return Err(fail(format!(
                    "{field} is not a canonical World table or cell"
                )));
            };
            // Snapshots leave an uninstalled field out as a top-level World member, or as a
            // member of the nested owner that holds it (§5.5).
            if crate::runtime::world_snapshot_member(field).is_none() {
                return Err(fail(format!(
                    "{field} has no snapshot member a selection can leave out"
                )));
            }
            let mut descriptor = descriptor.clone();
            descriptor.invariants = addition_digest(module, &[b"storage", field.as_bytes()]);
            match (entry, &descriptor.shape) {
                (StorageAddition::Table(_), StorageShapeV1::Table { .. }) => {}
                (StorageAddition::Cell(cell), StorageShapeV1::Cell) => {
                    descriptor.initial = Some(
                        (cell.initial)()
                            .map_err(|error| fail(format!("initial value of {field}: {error}")))?,
                    );
                }
                _ => return Err(fail(format!("{field} is declared with the wrong shape"))),
            }
            let id = self.publish(DefinitionDescriptorV1::Storage(descriptor))?;
            self.world_storage.insert(field, id);
            if let StorageAddition::Cell(cell) = entry {
                self.cell_installers.insert(id, cell.install);
            }
            storage.insert(field, id);
        }

        let mut interfaces = BTreeMap::new();
        for wire_id in addition.instructions {
            let Some(payload) = payloads.get(wire_id) else {
                return Err(fail(format!("{wire_id} is not a compiled instruction")));
            };
            let interface =
                self.publish(DefinitionDescriptorV1::Interface(InterfaceDescriptorV1 {
                    wire_id: (*wire_id).to_owned(),
                    payload: SchemaV1::Codec(format!("norito:{payload}@{}", shared.layout)),
                    rejection_codes: Vec::new(),
                    events: vec![shared.event],
                    metering_class: BASELINE_METERING.into(),
                    contract_statement: addition_digest(
                        module,
                        &[b"interface", wire_id.as_bytes()],
                    ),
                }))?;
            interfaces.insert(*wire_id, interface);
        }
        let mut rebinds = BTreeMap::new();
        for wire_id in addition.rebinds {
            let Some(interface) = interfaces_so_far.get(wire_id) else {
                return Err(fail(format!(
                    "it rebinds {wire_id}, which nothing before it answers"
                )));
            };
            rebinds.insert(*wire_id, *interface);
        }

        let touched = sorted(
            storage_so_far
                .iter()
                .chain(storage.values())
                .copied()
                .collect(),
        );
        let implementation = self.publish(DefinitionDescriptorV1::Implementation(
            ImplementationDescriptorV1 {
                entrypoint: EntrypointV1::Instructions(sorted(
                    interfaces
                        .values()
                        .chain(rebinds.values())
                        .copied()
                        .collect(),
                )),
                code: ImplementationCodeV1::Native {
                    module: module.into(),
                    reference_closure: addition_digest(module, &[b"implementation"]),
                },
                reads: touched.clone(),
                writes: touched.clone(),
                scans: touched,
                events: vec![shared.event],
                permissions: shared.permissions.clone(),
                parameters: vec![shared.parameter],
                invokes: sorted(
                    interfaces_so_far
                        .values()
                        .chain(interfaces.values())
                        .copied()
                        .collect(),
                ),
                host_profile: None,
                services_profile: shared.services_profile,
                limits: ResourceLimitsV1 {
                    max_gas: u64::MAX,
                    max_effects: u32::MAX,
                    max_call_depth: u16::MAX,
                },
            },
        ))?;
        let kernel = if module == additions::SORAFS_REPAIR.module {
            Some(self.publish(Self::sorafs_repair_kernel_descriptor())?)
        } else {
            None
        };
        Ok(AdditionDefinitions {
            module,
            implementation,
            interfaces,
            rebinds,
            storage,
            kernel,
        })
    }

    /// Native module of `implementation`, if this release implements it natively.
    #[must_use]
    pub fn native_module(&self, implementation: &DefinitionId) -> Option<&str> {
        match self.definitions.get(implementation) {
            Some(DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
                code: ImplementationCodeV1::Native { module, .. },
                ..
            })) => Some(module),
            _ => None,
        }
    }

    /// How this release installs the cell an addition adds as `storage`, if it adds one.
    #[must_use]
    pub(crate) fn cell_installer(&self, storage: &DefinitionId) -> Option<CellInstaller> {
        self.cell_installers.get(storage).copied()
    }

    /// The change set that moves a root from `current` to the addition `module` (§4, "Add an
    /// instruction"): it publishes what `published` does not hold yet, installs the addition's
    /// storage, activates its interfaces and implementation and binds its wire IDs and rebinds to
    /// it, effective at `effective_height`.
    ///
    /// # Errors
    /// `module` is not an addition of this release, or the change set does not apply to
    /// `current`; for example an earlier addition is not active yet, or the addition already is.
    pub fn activation(
        &self,
        module: &str,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        if module == additions::SORAFS_AUTHORITY.module {
            return super::sorafs_authority::draft(
                self,
                current,
                published,
                network,
                effective_height,
                qualification,
            );
        }
        let fail = |reason: String| CatalogError::Addition {
            module: module.to_owned(),
            reason,
        };
        let selected_addition;
        let addition = self
            .additions
            .iter()
            .find(|addition| addition.module == module)
            .ok_or_else(|| fail("not an addition of this release".to_owned()))?;
        let addition = if module == additions::RESTRICTED_DATASPACE_READ.module {
            selected_addition = super::restricted_dataspace_read::for_selection(self, current)?;
            &selected_addition
        } else {
            addition
        };
        let publish = addition
            .definitions()
            .into_iter()
            .filter(|id| published.definition(id).is_none())
            .map(|id| self.definitions[&id].clone())
            .collect();
        let mut transitions: Vec<SelectionTransitionV1> = addition
            .storage
            .values()
            .map(|id| SelectionTransitionV1::InstallStorage(*id))
            .collect();
        transitions.extend(
            addition
                .interfaces
                .values()
                .chain([&addition.implementation])
                .map(|id| SelectionTransitionV1::Activate(*id)),
        );
        let bindings: BTreeMap<&str, BindingV1> = addition
            .interfaces
            .iter()
            .chain(&addition.rebinds)
            .map(|(wire_id, interface)| {
                (
                    *wire_id,
                    BindingV1 {
                        wire_id: (*wire_id).to_owned(),
                        interface: *interface,
                        implementation: addition.implementation,
                    },
                )
            })
            .collect();
        transitions.extend(bindings.values().cloned().map(SelectionTransitionV1::Bind));

        let mut successor = current.clone();
        successor
            .installed_storage
            .extend(addition.storage.values().copied());
        successor.installed_storage = sorted(successor.installed_storage);
        successor.active.extend(
            addition
                .interfaces
                .values()
                .chain([&addition.implementation]),
        );
        successor.active = sorted(successor.active);
        successor
            .bindings
            .retain(|binding| !bindings.contains_key(binding.wire_id.as_str()));
        successor.bindings.extend(bindings.into_values());
        successor
            .bindings
            .sort_by(|left, right| left.wire_id.cmp(&right.wire_id));

        if module == additions::RESTRICTED_DATASPACE_READ.module
            && let Some(restriction) =
                super::restricted_dataspace_read::inherited_restriction(current, effective_height)?
        {
            transitions.push(SelectionTransitionV1::Restrict {
                definition: addition.implementation,
                restriction,
            });
            successor.restrictions.push(
                iroha_data_model::runtime::definition::RestrictionEntryV1 {
                    definition: addition.implementation,
                    restriction: restriction.at(effective_height),
                },
            );
            successor.restrictions.sort_by_key(|entry| entry.definition);
        }

        let change_set = ChangeSetV1 {
            network,
            root: RootScopeV1::Universal,
            predecessor: current.id()?,
            successor: successor.id()?,
            effective_height,
            publish,
            transitions,
            qualification,
        };
        change_set
            .apply(current, published)
            .map_err(|error| fail(error.to_string()))?;
        Ok(change_set)
    }

    fn sorafs_repair_kernel_descriptor() -> DefinitionDescriptorV1 {
        DefinitionDescriptorV1::Kernel(KernelDescriptorV1 {
            name: "iroha.kernel.taira_sorafs_repair.v1".into(),
            reference_closure: addition_digest(additions::SORAFS_REPAIR.module, &[b"kernel"]),
        })
    }

    /// Draft the sole transition authorized by Taira's all-four repair certificate.
    /// No earlier addition is installed and no existing binding or profile is changed.
    /// The new kernel retains ordinary execution and refuses any second repair.
    ///
    /// # Errors
    /// The predecessor is not the frozen kernel, the repair is already selected, or the
    /// precisely scoped successor is not closed over the published definitions.
    pub fn sorafs_repair_activation(
        &self,
        current: &RootSelectionV1,
        published: &impl DefinitionLookup,
        network: NetworkId,
        effective_height: u64,
        qualification: [u8; 32],
    ) -> Result<ChangeSetV1, CatalogError> {
        let fail = |reason: String| CatalogError::Addition {
            module: additions::SORAFS_REPAIR.module.into(),
            reason,
        };
        if current.kernel != self.baseline.kernel {
            return Err(fail("repair requires the exact predecessor kernel".into()));
        }
        let mut change = self.activation(
            additions::SORAFS_REPAIR.module,
            current,
            published,
            network,
            effective_height,
            qualification,
        )?;
        let mut successor = change
            .apply(current, published)
            .map_err(|error| fail(error.to_string()))?
            .selection;
        let kernel = Self::sorafs_repair_kernel_descriptor();
        let kernel_id = kernel.id()?;
        if published.definition(&kernel_id).is_some() {
            return Err(fail("repair kernel has already been published".into()));
        }
        debug_assert!(
            change
                .publish
                .iter()
                .any(|descriptor| descriptor == &kernel)
        );
        change
            .transitions
            .push(SelectionTransitionV1::SetKernel(kernel_id));
        successor.kernel = kernel_id;
        change.successor = successor.id()?;
        change
            .apply(current, published)
            .map_err(|error| fail(error.to_string()))?;
        Ok(change)
    }

    /// Compile only the explicitly supported Ironwood plus lifecycle composition.
    /// Baseline lifecycle is built separately. Published native descriptors never
    /// advertise additional executable combinations merely by appearing in a catalog.
    pub(crate) fn compile_lifecycle_compositions(&mut self) -> Result<(), CatalogError> {
        if let Some(ironwood) = &self.ironwood {
            let current = ironwood.selection.clone();
            self.lifecycle_successor(&current)?;
        }
        Ok(())
    }

    /// Derive lifecycle capture from the currently committed selection.
    ///
    /// Installed storage, interfaces, events, host profiles, restrictions and all
    /// existing service entries are retained. Other native owner cutovers (such
    /// as Ironwood) must already be supported by this release catalog. This
    /// publishes descriptors in this in-memory builder only, never in State.
    /// # Errors
    /// An unsupported predecessor, invalid service profile or closure.
    pub fn lifecycle_successor(
        &mut self,
        current: &RootSelectionV1,
    ) -> Result<RootSelectionV1, CatalogError> {
        current.check_closure(&self.definitions)?;
        if let Some(id) = self.unsupported(current) {
            return Err(CatalogError::UnsupportedServices(id));
        }
        let Some(DefinitionDescriptorV1::ServicesProfile(mut services)) =
            self.definitions.get(&current.services_profile).cloned()
        else {
            return Err(CatalogError::UnsupportedServices(current.services_profile));
        };
        if services
            .services
            .iter()
            .any(|service| service.name == "trigger_lifecycle")
        {
            return Err(CatalogError::UnsupportedServices(current.services_profile));
        }
        services.services.push(ServiceEntryV1 {
            name: "trigger_lifecycle".into(),
            reference_closure: iroha_crypto::Hash::new(
                iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
            )
            .into(),
        });
        services.services.sort_by(|a, b| a.name.cmp(&b.name));
        let services = self.publish(DefinitionDescriptorV1::ServicesProfile(services))?;
        let mut successor = current.clone();
        successor.services_profile = services;
        let mut replacements = BTreeMap::new();
        for binding in &current.bindings {
            if replacements.contains_key(&binding.implementation) {
                continue;
            }
            let Some(DefinitionDescriptorV1::Implementation(mut implementation)) =
                self.definitions.get(&binding.implementation).cloned()
            else {
                return Err(CatalogError::UnsupportedServices(binding.implementation));
            };
            if !matches!(implementation.code, ImplementationCodeV1::Native { .. }) {
                return Err(CatalogError::UnsupportedServices(binding.implementation));
            }
            // Additions keep their dispatch module, including any rebound handlers.
            // Lifecycle changes their services dependency, not their instruction code.
            let module = match &implementation.code {
                ImplementationCodeV1::Native { module, .. } => module.as_str(),
                _ => unreachable!("native code checked above"),
            };
            if module == BASELINE_MODULE || module == "iroha.ironwood.v1" {
                implementation.code = lifecycle_implementation_code();
            }
            implementation.services_profile = services;
            let id = self.publish(DefinitionDescriptorV1::Implementation(implementation))?;
            replacements.insert(binding.implementation, id);
        }
        for active in &mut successor.active {
            if let Some(replacement) = replacements.get(active) {
                *active = *replacement;
            }
        }
        successor.active = sorted(successor.active);
        // ChangeSet::Deactivate consumes restrictions on the retired bundle;
        // restrictions on retained interfaces/owners remain unchanged.
        successor
            .restrictions
            .retain(|entry| !replacements.contains_key(&entry.definition));
        for binding in &mut successor.bindings {
            binding.implementation = replacements[&binding.implementation];
        }
        successor.check_closure(&self.definitions)?;
        Ok(successor)
    }

    /// Select one service's semantics from an exact compiled services profile.
    ///
    /// Dispatch and node-side support checks must share this predicate when a service is
    /// composed with independently governed extensions. A supported profile without the
    /// service returns false; a matching name with a different closure is never treated as
    /// absence. This consults only the immutable compiled catalog, not untrusted published
    /// descriptors. Ordinary selection closure/support validation remains required separately.
    /// The lookup costs O(log definitions + log services), without scanning storage owners.
    ///
    /// # Errors
    /// The complete profile is not compiled, is not a services descriptor, or selects a
    /// different reference closure for the requested service.
    pub fn service_enabled(
        &self,
        profile: DefinitionId,
        service: &str,
        reference_closure: [u8; 32],
    ) -> Result<bool, CatalogError> {
        let Some(DefinitionDescriptorV1::ServicesProfile(descriptor)) =
            self.definitions.get(&profile)
        else {
            return Err(CatalogError::UnsupportedServices(profile));
        };
        match descriptor
            .services
            .binary_search_by(|entry| entry.name.as_str().cmp(service))
        {
            Err(_) => Ok(false),
            Ok(index) if descriptor.services[index].reference_closure == reference_closure => {
                Ok(true)
            }
            Ok(_) => Err(CatalogError::UnsupportedServices(profile)),
        }
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
pub(crate) mod tests;
