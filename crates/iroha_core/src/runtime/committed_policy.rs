//! The committed content-bundle limits, settlement router inputs and uploaded-model and DA ingest
//! quotas of a root (`specs/genesis_bound_node_configuration.md`, "Frozen-baseline constraint on
//! G2"; `specs/live_runtime_upgrades.md` §4, §6, §7.1).
//!
//! The typed custom parameters `content_policy_v1` ([`ContentPolicyV1`]),
//! `settlement_router_policy_v1` ([`SettlementRouterPolicyV1`]) and `nexus_quota_policy_v1`
//! ([`NexusQuotaPolicyV1`]) are behaviour this release adds after its frozen baseline. A root
//! recognizes one only while its selection activates the
//! Parameter definition this release publishes for it ([`super::catalog::parameters`]) and no
//! `Disabled` restriction names that definition. Recognition is never stored: every reader derives
//! it from the exact selection and parameters version it reads, so a block, a view and both sides
//! of a State cell agree with the selection they belong to.
//!
//! - **Not recognized** (every root until a governed change set activates the definition): the
//!   identifier is an ordinary custom parameter, stored without interpretation exactly as on the
//!   frozen baseline. No reader decodes it, `SetParameter` and executor reconciliation treat it
//!   like any other custom parameter, and every reader applies the protocol constants of this
//!   release, which equal the protocol defaults of the typed policies.
//! - **Becoming recognized** (activation, or lifting the definition's `Disabled` restriction)
//!   writes the definition's initial value, the protocol default, over whatever the root stored
//!   under the identifier while it was opaque ([`install`]), so an opaque value never becomes
//!   policy.
//! - **Recognized**: `SetParameter` admits only a valid policy ([`admit_parameter`]), and executor
//!   reconciliation can neither declare, replace nor retire the parameter ([`consensus_owned`]).
//!   Like every committed policy, a value committed in block `N` governs block `N + 1` on: block
//!   construction freezes its predecessor's value.
//! - **Becoming unrecognized** (a `Disabled` restriction or deactivation) is admitted only while
//!   the committed value equals the initial value; otherwise the change set lapses
//!   ([`check_retirement`]). The block that applies it was constructed under its predecessor's
//!   value, so this keeps that block, like every later one, at the protocol constants that the
//!   successor selection requires (§6, "Scheduling"). Governance first sets the value back.
//!
//! §7.1 defines `Deprecated`, `AdmissionClosed` and `CompletionOnly` for interfaces and
//! implementations only; on a Parameter definition they change nothing.

use std::sync::LazyLock;

use iroha_config::parameters::actual::{Content, Router, Settlement};
use iroha_data_model::{
    content_policy::ContentPolicyV1,
    da::prelude::DaStripeLayout,
    nexus_quota_policy::NexusQuotaPolicyV1,
    parameter::{CustomParameter, CustomParameterId, Parameters},
    runtime::definition::{DefinitionDescriptorV1, DefinitionId, RestrictionV1, RootSelectionV1},
    settlement_router_policy::SettlementRouterPolicyV1,
};
use mv::storage::StorageReadOnly as _;

use super::catalog::parameters::{
    CONTENT_POLICY, NEXUS_QUOTA_POLICY, ParameterAddition, SETTLEMENT_ROUTER_POLICY,
};
use crate::{
    pipeline_gas_policy::{ParameterIngressError, ParameterRetentionError},
    state::{StateStorageAdmissionError, StateTransaction, WorldReadOnly, WorldTransaction},
};

/// One typed committed parameter with the identities its readers look up.
struct Recognized {
    addition: &'static ParameterAddition,
    definition: Option<DefinitionId>,
    id: Option<CustomParameterId>,
}

impl Recognized {
    fn of(addition: &'static ParameterAddition) -> Self {
        Self {
            addition,
            // A descriptor that cannot be built is a build defect the catalog reports when it is
            // built; such a parameter is never recognized.
            definition: addition.definition_id().ok(),
            id: addition.name.parse().ok(),
        }
    }

    /// Whether the root running `selection` recognizes the parameter.
    fn by(&self, selection: &RootSelectionV1) -> bool {
        self.definition.as_ref().is_some_and(|definition| {
            selection.active.binary_search(definition).is_ok()
                && !selection
                    .restrictions
                    .binary_search_by(|entry| entry.definition.cmp(definition))
                    .is_ok_and(|index| {
                        matches!(
                            selection.restrictions[index].restriction,
                            RestrictionV1::Disabled(_)
                        )
                    })
        })
    }

    /// The custom parameter stored under the identifier, whether or not a root recognizes it.
    fn stored<'a>(&self, parameters: &'a Parameters) -> Option<&'a CustomParameter> {
        self.id.as_ref().and_then(|id| parameters.custom().get(id))
    }
}

static CONTENT: LazyLock<Recognized> = LazyLock::new(|| Recognized::of(&CONTENT_POLICY));
static SETTLEMENT_ROUTER: LazyLock<Recognized> =
    LazyLock::new(|| Recognized::of(&SETTLEMENT_ROUTER_POLICY));
static NEXUS_QUOTAS: LazyLock<Recognized> = LazyLock::new(|| Recognized::of(&NEXUS_QUOTA_POLICY));

fn all() -> [&'static Recognized; 3] {
    [&*CONTENT, &*SETTLEMENT_ROUTER, &*NEXUS_QUOTAS]
}

/// The readers' identities of the typed parameter named `name`, if this release adds it.
fn named(name: &str) -> Option<&'static Recognized> {
    all()
        .into_iter()
        .find(|recognized| recognized.addition.name == name)
}

/// Whether the root running `selection` recognizes `content_policy_v1`.
#[must_use]
pub fn content_recognized(selection: &RootSelectionV1) -> bool {
    CONTENT.by(selection)
}

/// Whether the root running `selection` recognizes `settlement_router_policy_v1`.
#[must_use]
pub fn settlement_router_recognized(selection: &RootSelectionV1) -> bool {
    SETTLEMENT_ROUTER.by(selection)
}

/// Whether the root running `selection` recognizes `nexus_quota_policy_v1`.
#[must_use]
pub fn nexus_quotas_recognized(selection: &RootSelectionV1) -> bool {
    NEXUS_QUOTAS.by(selection)
}

/// The committed content-bundle limits of a root running `selection` with `parameters`, if it
/// recognizes them.
///
/// An absent or malformed value reads as the protocol default. Neither occurs on a chain: the
/// change set that makes the parameter recognized writes a valid value, and `SetParameter`
/// admits only valid ones while it is recognized.
#[must_use]
pub fn content_policy(
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Option<ContentPolicyV1> {
    CONTENT.by(selection).then(|| {
        CONTENT
            .stored(parameters)
            .and_then(|custom| ContentPolicyV1::from_custom_parameter(custom).and_then(Result::ok))
            .unwrap_or_else(ContentPolicyV1::protocol_default)
    })
}

/// The committed settlement router inputs of a root running `selection` with `parameters`, if
/// it recognizes them; absent or malformed values read as in [`content_policy`].
#[must_use]
pub fn settlement_router_policy(
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Option<SettlementRouterPolicyV1> {
    SETTLEMENT_ROUTER.by(selection).then(|| {
        SETTLEMENT_ROUTER
            .stored(parameters)
            .and_then(|custom| {
                SettlementRouterPolicyV1::from_custom_parameter(custom).and_then(Result::ok)
            })
            .unwrap_or_else(SettlementRouterPolicyV1::protocol_default)
    })
}

/// The committed uploaded-model and DA ingest quotas of a root running `selection` with
/// `parameters`, if it recognizes them; absent or malformed values read as in
/// [`content_policy`].
#[must_use]
pub fn nexus_quota_policy(
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Option<NexusQuotaPolicyV1> {
    NEXUS_QUOTAS.by(selection).then(|| {
        NEXUS_QUOTAS
            .stored(parameters)
            .and_then(|custom| {
                NexusQuotaPolicyV1::from_custom_parameter(custom).and_then(Result::ok)
            })
            .unwrap_or_else(NexusQuotaPolicyV1::protocol_default)
    })
}

/// The uploaded-model and DA ingest quotas a root running `selection` with `parameters`
/// applies: the committed quotas when the root recognizes them, the protocol default otherwise.
///
/// No node setting contributes. A block applies the value of its predecessor, frozen when the
/// block is constructed ([`crate::state::StaticExecutionPolicy::nexus_quotas`]); a view applies
/// the value of the State it reads.
#[must_use]
pub fn effective_nexus_quotas(
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> NexusQuotaPolicyV1 {
    nexus_quota_policy(selection, parameters).unwrap_or_else(NexusQuotaPolicyV1::protocol_default)
}

/// The content settings a root running `selection` with `parameters` applies: `installed`, the
/// node's gateway resources over the protocol limits
/// ([`crate::state::State::install_node_content`]), with the committed limits laid over it when
/// the root recognizes them.
#[must_use]
pub fn effective_content(
    installed: &Content,
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Content {
    let mut content = installed.clone();
    if let Some(policy) = content_policy(selection, parameters) {
        apply_content(&policy, &mut content);
    }
    content
}

/// The settlement settings a root running `selection` with `parameters` applies: the committed
/// router inputs when the root recognizes them, `installed` (the protocol constants) otherwise.
#[must_use]
pub fn effective_settlement(
    installed: &Settlement,
    selection: &RootSelectionV1,
    parameters: &Parameters,
) -> Settlement {
    settlement_router_policy(selection, parameters)
        .map_or(*installed, |policy| settlement_of(&policy))
}

/// Replace the committed limits of `content` with `policy`, keeping its gateway resources.
///
/// Every field is named, so a field added to the settings does not compile until it is given a
/// source.
fn apply_content(policy: &ContentPolicyV1, content: &mut Content) {
    let Content {
        max_bundle_bytes,
        max_files,
        max_path_len,
        max_retention_blocks,
        chunk_size_bytes,
        limits: _,
        default_cache_max_age_secs,
        max_cache_max_age_secs,
        immutable_bundles,
        slo: _,
        pow: _,
        stripe_layout,
    } = content;
    *max_bundle_bytes = policy.max_bundle_bytes;
    *max_files = policy.max_files;
    *max_path_len = policy.max_path_len;
    *max_retention_blocks = policy.max_retention_blocks;
    *chunk_size_bytes = policy.chunk_size_bytes;
    *default_cache_max_age_secs = policy.default_cache_max_age_secs;
    *max_cache_max_age_secs = policy.max_cache_max_age_secs;
    *immutable_bundles = policy.immutable_bundles;
    *stripe_layout = policy.stripe_layout();
}

/// The committed limits `content` applies. Gateway resources are not part of the policy.
#[must_use]
pub fn content_policy_of(content: &Content) -> ContentPolicyV1 {
    let Content {
        max_bundle_bytes,
        max_files,
        max_path_len,
        max_retention_blocks,
        chunk_size_bytes,
        limits: _,
        default_cache_max_age_secs,
        max_cache_max_age_secs,
        immutable_bundles,
        slo: _,
        pow: _,
        stripe_layout:
            DaStripeLayout {
                total_stripes,
                shards_per_stripe,
                row_parity_stripes,
            },
    } = content;
    ContentPolicyV1 {
        max_bundle_bytes: *max_bundle_bytes,
        max_files: *max_files,
        max_path_len: *max_path_len,
        max_retention_blocks: *max_retention_blocks,
        chunk_size_bytes: *chunk_size_bytes,
        default_cache_max_age_secs: *default_cache_max_age_secs,
        max_cache_max_age_secs: *max_cache_max_age_secs,
        immutable_bundles: *immutable_bundles,
        stripe_total_stripes: *total_stripes,
        stripe_shards_per_stripe: *shards_per_stripe,
        stripe_row_parity_stripes: *row_parity_stripes,
    }
}

/// The settlement settings of `policy`, one exhaustive literal.
#[must_use]
pub fn settlement_of(policy: &SettlementRouterPolicyV1) -> Settlement {
    Settlement {
        router: Router {
            twap_window: std::time::Duration::from_secs(policy.twap_window_seconds),
            epsilon_bps: policy.epsilon_bps,
            buffer_alert_pct: policy.buffer_alert_pct,
            buffer_throttle_pct: policy.buffer_throttle_pct,
            buffer_xor_only_pct: policy.buffer_xor_only_pct,
            buffer_halt_pct: policy.buffer_halt_pct,
            buffer_horizon_hours: policy.buffer_horizon_hours,
        },
    }
}

/// The router inputs `settlement` applies. A sub-second part of the TWAP window is not
/// representable in the policy; no settings value of this release holds one.
#[must_use]
pub fn settlement_router_policy_of(settlement: &Settlement) -> SettlementRouterPolicyV1 {
    let Settlement {
        router:
            Router {
                twap_window,
                epsilon_bps,
                buffer_alert_pct,
                buffer_throttle_pct,
                buffer_xor_only_pct,
                buffer_halt_pct,
                buffer_horizon_hours,
            },
    } = settlement;
    SettlementRouterPolicyV1 {
        twap_window_seconds: twap_window.as_secs(),
        epsilon_bps: *epsilon_bps,
        buffer_alert_pct: *buffer_alert_pct,
        buffer_throttle_pct: *buffer_throttle_pct,
        buffer_xor_only_pct: *buffer_xor_only_pct,
        buffer_halt_pct: *buffer_halt_pct,
        buffer_horizon_hours: *buffer_horizon_hours,
    }
}

/// Check a custom parameter `SetParameter` is about to commit, if the root recognizes its
/// identifier as one of these typed parameters: it must be a valid policy. Every other
/// parameter, and every parameter of a root that does not recognize it, passes unchanged, so that
/// root executes `SetParameter` exactly as the frozen baseline does.
///
/// # Errors
/// The rule the policy violates.
pub(crate) fn admit_parameter(
    state_transaction: &StateTransaction<'_, '_>,
    parameter: &CustomParameter,
) -> Result<(), String> {
    match named(parameter.id().name().as_ref()) {
        Some(recognized) if recognized.by(state_transaction.world.runtime_selection.get()) => {
            (recognized.addition.canonical)(parameter).map(|_| ())
        }
        _ => Ok(()),
    }
}

/// Whether the root running `selection` recognizes `id` as one of these typed parameters. Only a
/// validated `SetParameter` writes such a parameter: an executor data model can neither declare,
/// replace nor retire it.
#[must_use]
pub(crate) fn consensus_owned(selection: &RootSelectionV1, id: &CustomParameterId) -> bool {
    named(id.name().as_ref()).is_some_and(|recognized| recognized.by(selection))
}

/// Check a change set that moves the root of `world` from `current` to `next`: it may stop
/// recognizing one of these parameters only while the committed value equals the definition's
/// initial value (or is absent, which reads as it). Activation calls this before applying the
/// change set; a refusal lapses it (§6).
///
/// # Errors
/// The parameter whose committed value is not the initial value.
pub(crate) fn check_retirement(
    world: &impl WorldReadOnly,
    current: &RootSelectionV1,
    next: &RootSelectionV1,
) -> Result<(), String> {
    for recognized in all() {
        if !recognized.by(current) || recognized.by(next) {
            continue;
        }
        let Some(committed) = recognized.stored(world.parameters()) else {
            continue;
        };
        let name = recognized.addition.name;
        let initial = (recognized.addition.initial)()
            .map_err(|error| format!("initial value of {name}: {error}"))?;
        if (recognized.addition.canonical)(committed).ok() != Some(initial) {
            return Err(format!(
                "committed parameter {name} must hold its initial value before it stops being \
                 recognized"
            ));
        }
    }
    Ok(())
}

/// Write the initial value of every one of these parameters that the change set moving the root
/// of `wtx` to `next` makes recognized (§4): the bytes its published Parameter definition
/// declares, decoded by this release, over whatever the root stored under the identifier while
/// it was opaque. Activation calls this before it moves the selection.
///
/// The write goes through the funded parameter owner, like every other parameters write. A
/// finite-pool refusal stays a local storage refusal the caller may retry; any other refusal is a
/// defect of this release, which stops the node at `height` as for a cell it cannot install. A
/// local refusal is never a function of committed State, so it never turns an activation into a
/// lapse.
///
/// # Errors
/// The release's installer refuses the published bytes, or the parameter owner refuses the write.
pub(super) fn install(
    wtx: &mut WorldTransaction<'_, '_>,
    next: &RootSelectionV1,
    catalog: &super::ReleaseCatalog,
    height: u64,
) -> Result<(), StateStorageAdmissionError> {
    for recognized in all() {
        if recognized.by(wtx.runtime_selection.get()) || !recognized.by(next) {
            continue;
        }
        let name = recognized.addition.name;
        let defect = |reason: String| StateStorageAdmissionError::UnsupportedRuntime {
            height,
            reason: format!("installing committed parameter {name} failed: {reason}"),
        };
        let Some(definition) = recognized.definition else {
            return Err(defect(
                "its Parameter definition has no identity".to_owned(),
            ));
        };
        let Some(addition) = catalog.parameter_addition(&definition) else {
            return Err(defect("this release does not add it".to_owned()));
        };
        let Some(DefinitionDescriptorV1::Parameter(descriptor)) =
            wtx.runtime_definitions.get(&definition)
        else {
            return Err(defect(
                "its Parameter definition is not published".to_owned(),
            ));
        };
        let parameter = (addition.install)(&descriptor.initial).map_err(defect)?;
        let pool = wtx.sccp_committee_generations.allocation_budget().clone();
        let refused = |error: ParameterIngressError| match error {
            ParameterIngressError::Allocation(iroha_allocation::ChargedBufferError::Admission(
                refusal,
            ))
            | ParameterIngressError::Retention(ParameterRetentionError::Admission(refusal)) => {
                StateStorageAdmissionError::World(mv::storage::AdmittedStorageError::Allocation(
                    refusal,
                ))
            }
            error => defect(error.to_string()),
        };
        let prepared = wtx
            .parameters
            .get()
            .prepare_custom_parameter(&parameter, &pool)
            .map_err(refused)?;
        let successor = prepared
            .into_successor(wtx.parameters.get(), &pool)
            .map_err(|(_prepared, error)| refused(error.into()))?;
        *wtx.parameters.get_mut() = successor;
    }
    Ok(())
}

/// Make the universal root of `world` recognize every typed committed parameter of this release,
/// as the change set drafted under [`super::catalog::parameters::MODULE`] does at block start:
/// publish the Parameter definitions, write their initial values and move the selection.
///
/// Fixtures only. A network recognizes a parameter only through a governed change set.
///
/// # Panics
/// The release catalog cannot be built, the root already recognizes every parameter, or the
/// activation does not apply to `world`'s selection.
#[cfg(test)]
pub(crate) fn activate_for_testing(world: &crate::state::World) {
    let catalog = super::release_catalog().expect("release catalog");
    // A change set's network scope does not shape the selection it produces.
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0; 32])),
    );
    let mut block = world.block();
    let mut wtx = block
        .transaction_without_telemetry(iroha_config::parameters::actual::LaneConfig::default(), 0);
    let current = wtx.runtime_selection.get().clone();
    let applied = {
        let published = super::change_set::PublishedDefinitions(&wtx.runtime_definitions);
        catalog
            .parameter_activation(&current, &published, network, 1, [0; 32])
            .expect("draft the parameter activation")
            .apply(&current, &published)
            .expect("the parameter activation applies")
    };
    for (definition, descriptor) in applied.published {
        wtx.runtime_definitions.insert(definition, descriptor);
    }
    install(&mut wtx, &applied.selection, catalog, 1).expect("the initial values install");
    *wtx.runtime_selection.get_mut() = applied.selection;
    wtx.apply();
    block.commit();
}

#[cfg(test)]
mod tests;
