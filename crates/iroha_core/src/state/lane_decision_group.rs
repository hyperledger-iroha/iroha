//! Join exact native Decisions for every route of one immutable admitted input.
//!
//! This is a read-only consumer boundary. Individual Decisions remain owned by
//! their instance until the entire group can enter global execution. It grants
//! no Ready, application completion, historical authority or publication lease.
//! TODO: replace the old MergeLaneExecution source consumer and atomically retire
//! independent economic frontier writers before activating this group join.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_data_model::block::{
    lane_consensus::LaneDecisionV1,
    lane_input::{LaneDecisionGroupV1, MAX_LANE_INPUT_ROUTE_SLOTS},
};

use super::{
    AuthenticatedLaneAdmittedInputSourceV1, FirstLaneAdmittedInputReadV1,
    LaneInputBodyPreparationV1, LaneInputDependencyV1, State, VerifiedFirstLaneAdmittedInputV1,
    VerifiedLaneContext, VerifiedLaneContexts, VerifiedLaneInputBodyV1,
};
use crate::sumeragi::{
    v2_lane_payload::verify_lane_input_manifest, v2_lane_wire::LaneAuthenticator,
};

/// All exact route Decisions for a single canonical input, in route-slot order.
/// The private construction authenticates an observation, never a live lease.
#[derive(Clone, Debug)]
pub(crate) struct VerifiedLaneDecisionGroupV1 {
    body: VerifiedLaneInputBodyV1,
    decisions: Vec<LaneDecisionV1>,
    contexts: Vec<VerifiedLaneContext>,
}
impl VerifiedLaneDecisionGroupV1 {
    /// One immutable entrypoint and its authenticated first-carrier source.
    pub(crate) fn body(&self) -> &VerifiedLaneInputBodyV1 {
        &self.body
    }
    /// One native CommitQC per distinct route, in the body's canonical slot order.
    pub(crate) fn decisions(&self) -> &[LaneDecisionV1] {
        &self.decisions
    }
    /// Export one complete source with the entrypoint represented exactly once.
    /// The wire value carries evidence, not this private verification or a lease.
    pub(crate) fn to_wire(&self) -> LaneDecisionGroupV1 {
        LaneDecisionGroupV1 {
            payload: self.body.payload().clone(),
            decisions: self.decisions.clone(),
        }
    }
    /// Exact independently authenticated contexts aligned with the route slots.
    /// A pre-carrier consumer must compare them with its own current snapshot.
    pub(super) fn contexts(&self) -> &[VerifiedLaneContext] {
        &self.contexts
    }
}

/// A group wait always names exact missing instances or earlier admitted work.
#[derive(Debug)]
pub(crate) enum LaneDecisionGroupPreparationV1 {
    /// Every affected instance decided this input; economic execution is separate.
    Ready(VerifiedLaneDecisionGroupV1),
    /// These earlier immutable groups still own affected route heads.
    BlockedByEarlierInputs(Vec<LaneInputDependencyV1>),
    /// Retain received Decisions and await these exact still-open instances.
    MissingDecisions(Vec<Hash>),
    /// Recover this exact first global carrier through its existing body owner.
    /// The caller retains the incoming group until recovery and revalidation.
    CanonicalBodyRecoveryRequired(AuthenticatedLaneAdmittedInputSourceV1),
    /// Refresh the authenticated complete set before retrying this observation.
    ObservationChanged,
    /// The requested target is absent from the complete current set.
    InstanceNotCurrent,
}

fn authenticate_decision_group(
    observed: &VerifiedLaneContexts,
    body: VerifiedLaneInputBodyV1,
    decisions: &[LaneDecisionV1],
) -> Result<LaneDecisionGroupPreparationV1, String> {
    let slots = &body.payload().descriptor.slots;
    if decisions.len() > slots.len() {
        return Err("native decision group exceeds its exact route count".into());
    }
    let contexts = observed
        .contexts()
        .iter()
        .map(|lane| (Hash::from(lane.instance_id().0), lane))
        .collect::<BTreeMap<_, _>>();
    let slots_by_instance = slots
        .iter()
        .map(|slot| (slot.instance_id, slot))
        .collect::<BTreeMap<_, _>>();
    let mut checked = BTreeMap::new();
    for decision in decisions {
        let instance = decision.value().instance_id;
        let slot = slots_by_instance
            .get(&instance)
            .ok_or_else(|| "native Decision belongs to another input slot".to_owned())?;
        if checked.contains_key(&instance) {
            return Err("native decision group repeats an instance".into());
        }
        let lane = contexts
            .get(&instance)
            .ok_or_else(|| "native Decision lost its authenticated current instance".to_owned())?;
        let frozen = lane.frozen();
        if (
            slot.route.lane_id,
            slot.route.dataspace_id,
            slot.lane_incarnation,
            slot.lane_height,
        ) != (
            frozen.lane_id,
            frozen.dataspace_id,
            frozen.lane_incarnation,
            frozen.next_lane_height,
        ) {
            return Err("native Decision route differs from its exact input slot".into());
        }
        LaneAuthenticator::new(lane)
            .decision_certificate(decision)
            .map_err(|error| error.to_string())?;
        // The valid QC alone does not prove that its descriptor, input or RS16
        // commitment is the immutable group selected from the first carrier.
        verify_lane_input_manifest(lane, &body, &decision.manifest, body.canonical_bytes())?;
        checked.insert(instance, decision);
    }
    let missing = slots
        .iter()
        .filter_map(|slot| (!checked.contains_key(&slot.instance_id)).then_some(slot.instance_id))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Ok(LaneDecisionGroupPreparationV1::MissingDecisions(missing));
    }
    let ordered = slots
        .iter()
        .map(|slot| (*checked[&slot.instance_id]).clone())
        .collect();
    let contexts = slots
        .iter()
        .map(|slot| (*contexts[&slot.instance_id]).clone())
        .collect();
    Ok(LaneDecisionGroupPreparationV1::Ready(
        VerifiedLaneDecisionGroupV1 {
            body,
            decisions: ordered,
            contexts,
        },
    ))
}

impl State {
    /// Import one untrusted source for global execution without changing custody.
    ///
    /// A complete transaction and a valid native QC are insufficient: reconstruct
    /// the original first-carrier input and all current route slots, then require
    /// byte-for-byte payload equality. Missing historical body has a typed recovery
    /// owner; it cannot be replaced with the sender's otherwise signed input.
    pub(crate) fn import_lane_decision_group(
        &self,
        observed: &VerifiedLaneContexts,
        source: &LaneDecisionGroupV1,
    ) -> Result<LaneDecisionGroupPreparationV1, String> {
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        source.validate_structure()?;
        let instance = source.payload.descriptor.slots[0].instance_id;
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| Hash::from(lane.instance_id().0) == instance)
        else {
            return Ok(LaneDecisionGroupPreparationV1::InstanceNotCurrent);
        };
        let result = (|| {
            let original = match self.first_lane_admitted_input(observed, lane)? {
                FirstLaneAdmittedInputReadV1::Ready(original) => original,
                FirstLaneAdmittedInputReadV1::CanonicalBodyRecoveryRequired(required) => {
                    return Ok(
                        LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(required),
                    );
                }
                FirstLaneAdmittedInputReadV1::ObservationChanged => {
                    return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
                }
                FirstLaneAdmittedInputReadV1::InstanceNotCurrent => {
                    return Ok(LaneDecisionGroupPreparationV1::InstanceNotCurrent);
                }
            };
            self.import_recovered_lane_decision_group(observed, source, &original)
        })();
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        result
    }

    /// Resume the same source import after authenticated first-carrier recovery.
    ///
    /// The recovered token must come from the existing exact global body
    /// response boundary. It does not skip wire-payload equality or current
    /// all-route checks and does not acknowledge the caller's recovery owner.
    pub(crate) fn import_recovered_lane_decision_group(
        &self,
        observed: &VerifiedLaneContexts,
        source: &LaneDecisionGroupV1,
        original: &VerifiedFirstLaneAdmittedInputV1,
    ) -> Result<LaneDecisionGroupPreparationV1, String> {
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        source.validate_structure()?;
        let instance = source.payload.descriptor.slots[0].instance_id;
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| Hash::from(lane.instance_id().0) == instance)
        else {
            return Ok(LaneDecisionGroupPreparationV1::InstanceNotCurrent);
        };
        let result = (|| {
            let result =
                self.prepare_lane_decision_group(observed, lane, original, &source.decisions)?;
            if let LaneDecisionGroupPreparationV1::Ready(group) = &result {
                let supplied =
                    norito::encode_canonical(&source.payload).map_err(|error| error.to_string())?;
                if supplied != group.body().canonical_bytes() {
                    return Err("native decision source is not the exact first-carrier input and current slots".into());
                }
            }
            Ok(result)
        })();
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        result
    }

    /// Authenticate a complete group using current frozen instances and its
    /// exact first-admission input. Arrival order and different per-lane voting
    /// views do not affect membership or introduce a shared view barrier.
    /// No source/Decision custody is consumed and no State or disk mutation occurs.
    pub(crate) fn prepare_lane_decision_group(
        &self,
        observed: &VerifiedLaneContexts,
        lane: &VerifiedLaneContext,
        source: &VerifiedFirstLaneAdmittedInputV1,
        decisions: &[LaneDecisionV1],
    ) -> Result<LaneDecisionGroupPreparationV1, String> {
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        if decisions.len() > MAX_LANE_INPUT_ROUTE_SLOTS {
            return Err("native decision group exceeds the bounded route inventory".into());
        }
        let result = (|| match self.prepare_lane_input_body(observed, lane, source)? {
            LaneInputBodyPreparationV1::Ready(body) => {
                authenticate_decision_group(observed, body, decisions)
            }
            LaneInputBodyPreparationV1::BlockedByEarlierInputs(dependencies) => Ok(
                LaneDecisionGroupPreparationV1::BlockedByEarlierInputs(dependencies),
            ),
            LaneInputBodyPreparationV1::ObservationChanged => {
                Ok(LaneDecisionGroupPreparationV1::ObservationChanged)
            }
            LaneInputBodyPreparationV1::InstanceNotCurrent => {
                Ok(LaneDecisionGroupPreparationV1::InstanceNotCurrent)
            }
        })();
        // Expensive native signature/codeword verification never extends a lease.
        if !observed.is_current(self) {
            return Ok(LaneDecisionGroupPreparationV1::ObservationChanged);
        }
        result
    }
}
