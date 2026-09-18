//! Materialize one immutable input only when all of its routes pin the same group.
//!
//! The private output proves a checked observation and exact historical source,
//! not a lease. Every productive effect must reacquire the State publication
//! lease and recheck all slots before using it. No body readiness or signing is
//! granted here. LaneInstance's move-owned worker establishes physical RS16
//! storage/validation custody without resetting its pre-payload timer.
//! TODO: connect that owner to production worker admission and atomic group
//! application while retiring the old lane signer.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::block::{
    lane_admission::QueuePlanAdmissionBindingV1,
    lane_consensus::{FrozenLaneConsensusContextV1, LaneValueKindV1, QueuePlanAdmissionPriorityV1},
    lane_input::{
        LANE_INPUT_VERSION_V1, LaneInputDescriptorV1, LaneInputPayloadV1, LaneInputRouteSlotV1,
    },
};
use iroha_model_base::topology::{DataSpaceId, LaneId};

use super::{State, VerifiedFirstLaneAdmittedInputV1, VerifiedLaneContext, VerifiedLaneContexts};

/// An exact earlier route head blocking this group, in strictly decreasing rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaneInputDependencyV1 {
    /// Route whose prior obligation must finish first.
    pub(crate) route: (LaneId, DataSpaceId),
    /// Exact frozen instance owning the earlier obligation.
    pub(crate) instance_id: Hash,
    /// Exact earlier immutable group.
    pub(crate) binding_hash: Hash,
    /// Globally assigned rank; always strictly less than the blocked group's rank.
    pub(crate) priority: QueuePlanAdmissionPriorityV1,
}

/// Exact immutable bytes selected from an authenticated complete context set.
/// This object is not live authority and grants neither readiness nor a vote.
#[derive(Clone, Debug)]
pub(crate) struct VerifiedLaneInputBodyV1 {
    source: VerifiedFirstLaneAdmittedInputV1,
    payload: LaneInputPayloadV1,
    bytes: Vec<u8>,
    kind: LaneValueKindV1,
}

impl VerifiedLaneInputBodyV1 {
    /// Exact original global carrier evidence and complete control.
    pub(crate) fn source(&self) -> &VerifiedFirstLaneAdmittedInputV1 {
        &self.source
    }
    /// Same immutable input and route slots on every affected lane.
    pub(crate) fn payload(&self) -> &LaneInputPayloadV1 {
        &self.payload
    }
    /// Canonical bytes to transfer to mandatory RS16 storage/validation.
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Role derived from the immutable admission plan.
    pub(crate) fn kind(&self) -> LaneValueKindV1 {
        self.kind
    }
}

/// Materialization must distinguish an older obligation from stale observations.
#[derive(Debug)]
pub(crate) enum LaneInputBodyPreparationV1 {
    /// All exact route heads agree; body custody still must be established.
    Ready(VerifiedLaneInputBodyV1),
    /// Earlier owners must progress; a timer cannot revoke their work.
    BlockedByEarlierInputs(Vec<LaneInputDependencyV1>),
    /// Read a new authenticated complete set before proceeding.
    ObservationChanged,
    /// The supplied target instance has already closed or was never a member.
    InstanceNotCurrent,
}

// Pure structural selection returns no authenticated token. Only the State
// boundary below can turn it into a private materialized-body proof.
#[derive(Debug)]
pub(super) enum SlotSelection {
    Ready(Vec<LaneInputRouteSlotV1>),
    Blocked(Vec<LaneInputDependencyV1>),
}

pub(super) fn select_input_slots<'a>(
    binding: &QueuePlanAdmissionBindingV1,
    priority: QueuePlanAdmissionPriorityV1,
    contexts: impl IntoIterator<Item = (&'a FrozenLaneConsensusContextV1, Hash)>,
) -> Result<SlotSelection, String> {
    let binding_hash = binding.canonical_hash();
    let mut routes = BTreeMap::new();
    for bound in &binding.admission_context.route_incarnations {
        let key = (bound.leg.route.lane_id, bound.leg.route.dataspace_id);
        if let Some((_, previous)) = routes.insert(key, (bound.leg.route, bound.lane_incarnation)) {
            if previous != bound.lane_incarnation {
                return Err("one admitted route has conflicting incarnations".into());
            }
        }
    }
    let mut by_route = BTreeMap::new();
    for (frozen, instance) in contexts {
        if by_route
            .insert((frozen.lane_id, frozen.dataspace_id), (frozen, instance))
            .is_some()
        {
            return Err("complete lane context set repeats a route".into());
        }
    }
    let mut slots = Vec::with_capacity(routes.len());
    let mut dependencies = Vec::new();
    for (key, (route, incarnation)) in routes {
        let (frozen, instance_id) = by_route.get(&key).copied().ok_or_else(|| {
            "admitted group lost an affected route from the complete context set".to_owned()
        })?;
        if frozen.lane_incarnation != incarnation {
            return Err("admitted group route incarnation differs from its current owner".into());
        }
        if frozen.admitted_binding_hash != binding_hash {
            if frozen.admission_priority >= priority {
                return Err("admitted group is blocked by a non-earlier route head".into());
            }
            dependencies.push(LaneInputDependencyV1 {
                route: key,
                instance_id,
                binding_hash: frozen.admitted_binding_hash,
                priority: frozen.admission_priority,
            });
            continue;
        }
        if frozen.admission_priority != priority {
            return Err("same admitted group has inconsistent first-carrier rank".into());
        }
        slots.push(LaneInputRouteSlotV1 {
            route,
            lane_incarnation: incarnation,
            instance_id,
            lane_height: frozen.next_lane_height,
        });
    }
    Ok(if dependencies.is_empty() {
        SlotSelection::Ready(slots)
    } else {
        SlotSelection::Blocked(dependencies)
    })
}

impl State {
    /// Join an exact original input to all currently observed route slots.
    ///
    /// No State guard, disk I/O, queue mutation or side effect occurs here. A
    /// missing/inconsistent affected route is a contradiction, never generic
    /// Pending. All legitimate dependencies strictly decrease global admission
    /// rank, so they cannot introduce an AMX route-order cycle.
    pub(crate) fn prepare_lane_input_body(
        &self,
        observed: &VerifiedLaneContexts,
        lane: &VerifiedLaneContext,
        source: &VerifiedFirstLaneAdmittedInputV1,
    ) -> Result<LaneInputBodyPreparationV1, String> {
        if !observed.is_current(self) {
            return Ok(LaneInputBodyPreparationV1::ObservationChanged);
        }
        if !observed.contexts().iter().any(|current| {
            current.instance_id() == lane.instance_id() && current.frozen() == lane.frozen()
        }) {
            return Ok(LaneInputBodyPreparationV1::InstanceNotCurrent);
        }
        let frozen = lane.frozen();
        let binding = &source.validated_input().input().certificate.binding;
        if source.priority() != frozen.admission_priority
            || binding.canonical_hash() != frozen.admitted_binding_hash
            || binding.network_id_digest
                != crate::torii_proxy::queue_plan_admission_network_id_digest(self.network_id_ref())
            || !binding
                .admission_context
                .route_incarnations
                .iter()
                .any(|bound| {
                    bound.leg.route.lane_id == frozen.lane_id
                        && bound.leg.route.dataspace_id == frozen.dataspace_id
                        && bound.lane_incarnation == frozen.lane_incarnation
                })
        {
            return Err("historical complete input differs from the target lane head".into());
        }
        let selection = select_input_slots(
            binding,
            source.priority(),
            observed
                .contexts()
                .iter()
                .map(|context| (context.frozen(), Hash::from(context.instance_id().0))),
        )?;
        let result = match selection {
            SlotSelection::Blocked(dependencies) => {
                LaneInputBodyPreparationV1::BlockedByEarlierInputs(dependencies)
            }
            SlotSelection::Ready(slots) => {
                let payload = LaneInputPayloadV1 {
                    descriptor: LaneInputDescriptorV1 {
                        version: LANE_INPUT_VERSION_V1,
                        admission_priority: source.priority(),
                        admission_carrier_hash: source.carrier_hash(),
                        admitted_input_hash: source.canonical_control_hash(),
                        slots,
                    },
                    input: source.validated_input().input().clone(),
                };
                let kind = payload.validate_structure()?;
                let bytes =
                    norito::encode_canonical(&payload).map_err(|error| error.to_string())?;
                LaneInputBodyPreparationV1::Ready(VerifiedLaneInputBodyV1 {
                    source: source.clone(),
                    payload,
                    bytes,
                    kind,
                })
            }
        };
        if !observed.is_current(self) {
            return Ok(LaneInputBodyPreparationV1::ObservationChanged);
        }
        Ok(result)
    }
}
