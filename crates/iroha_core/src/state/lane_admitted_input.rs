//! Immutable first-admission source authority, distinct from live lane eligibility.
//!
//! TODO: connect the recovery-required outcome to the existing bounded body
//! request/completion owner and retain custody through native RS16 materialization.

use std::num::NonZeroUsize;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, SignedBlock, consensus_v2 as wire, decode_framed_signed_block,
        lane_consensus::QueuePlanAdmissionPriorityV1,
    },
};
use iroha_model_base::topology::{DataSpaceId, LaneId};

use super::{State, VerifiedLaneContext, VerifiedLaneContexts};
use crate::{
    sumeragi::v2_transport::{
        AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse,
    },
    torii_proxy::{ValidatedLaneAdmittedInputV1, decode_and_validate_lane_admitted_input_v1},
};

/// Exact canonical historical source, privately linked to an authenticated lane head.
///
/// This immutable token can outlive lane closure. It proves neither current
/// membership nor all-route eligibility and cannot authorize signing or Apply.
#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedLaneAdmittedInputSourceV1 {
    network_id: NetworkId,
    priority: QueuePlanAdmissionPriorityV1,
    binding_hash: Hash,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    incarnation: Hash,
    finality: wire::finality::V2FinalityArtifact,
}

impl AuthenticatedLaneAdmittedInputSourceV1 {
    /// Exact historical context, CommitQC and PoPs for existing body recovery.
    pub(crate) fn finality(&self) -> &wire::finality::V2FinalityArtifact {
        &self.finality
    }

    /// Original State-owned position; never the binding's proposal height.
    pub(crate) fn priority(&self) -> QueuePlanAdmissionPriorityV1 {
        self.priority
    }

    /// Actual finalized first-carrier header hash.
    pub(crate) fn carrier_hash(&self) -> HashOf<BlockHeader> {
        self.finality.block_hash
    }

    /// Settle an existing authenticated historical proposal-body response.
    ///
    /// Both request and response constructors remain owned by v2_transport.
    /// The caller retains its exact request/completion custody until this token
    /// is transferred to the native input owner. No resultless body is written
    /// to Kura's executed-body cache and no global Ready/Apply is synthesized.
    pub(crate) fn complete_from_authenticated_response(
        &self,
        request: &AuthenticatedCertifiedBodyRequest,
        response: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<VerifiedFirstLaneAdmittedInputV1, String> {
        let request_wire = request.request();
        let response_wire = response.response();
        if request_wire.round != self.finality.commit_qc.proposal_round
            || request_wire.subject != self.finality.subject
            || request_wire.certificate != self.finality.commit_qc
            || response_wire.request_hash != request.request_hash()
            || Hash::new(&response_wire.body) != self.finality.subject.payload_hash
        {
            return Err(
                "admission source response differs from exact historical request/QC".into(),
            );
        }
        let block = decode_framed_signed_block(&response_wire.body)
            .map_err(|error| format!("admission proposal body is not canonical: {error}"))?;
        if !block.is_resultless_proposal() {
            return Err("admission source response contains execution results".into());
        }
        self.project_control(&block)
    }

    fn project_control(
        &self,
        block: &SignedBlock,
    ) -> Result<VerifiedFirstLaneAdmittedInputV1, String> {
        if block.header().height().get() != self.priority.carrier_height
            || block.hash() != self.carrier_hash()
            || block.header().prev_block_hash() != self.finality.subject.parent_block_hash
            || block
                .canonical_proposal_wire_hash()
                .map_err(|error| error.to_string())?
                != self.finality.subject.payload_hash
        {
            return Err("admission source body differs from exact finalized carrier".into());
        }
        let index =
            usize::try_from(self.priority.admission_index).map_err(|error| error.to_string())?;
        let bytes = block
            .execution_context()
            .and_then(|context| context.queue_plan_admissions.get(index))
            .ok_or_else(|| {
                "first-admission canonical index is absent from its carrier".to_owned()
            })?;
        let input = decode_and_validate_lane_admitted_input_v1(&self.network_id, bytes)?;
        let binding = &input.certificate().certificate.binding;
        if input.certificate().binding_hash != self.binding_hash
            || !binding
                .admission_context
                .route_incarnations
                .iter()
                .any(|route| {
                    route.leg.route.lane_id == self.lane_id
                        && route.leg.route.dataspace_id == self.dataspace_id
                        && route.lane_incarnation == self.incarnation
                })
        {
            return Err("first-admission input differs from the authenticated pinned head".into());
        }
        Ok(VerifiedFirstLaneAdmittedInputV1 {
            source: self.clone(),
            input,
            canonical_control_bytes: bytes.clone(),
        })
    }
}

/// Exact complete input proven to occupy the original finalized admission index.
/// The token is source evidence only; native Ready separately checks the current
/// full context set and all affected route slots/head identities.
#[derive(Clone, Debug)]
pub(crate) struct VerifiedFirstLaneAdmittedInputV1 {
    source: AuthenticatedLaneAdmittedInputSourceV1,
    input: ValidatedLaneAdmittedInputV1,
    canonical_control_bytes: Vec<u8>,
}

impl VerifiedFirstLaneAdmittedInputV1 {
    /// Exact immutable finality source, usable for idempotent source recovery.
    pub(crate) fn source(&self) -> &AuthenticatedLaneAdmittedInputSourceV1 {
        &self.source
    }
    /// Complete authenticated entrypoint/certificate without a reconstructed body.
    pub(crate) fn validated_input(&self) -> &ValidatedLaneAdmittedInputV1 {
        &self.input
    }
    /// Exact canonical bytes from the original carrier, including its quorum subset.
    pub(crate) fn canonical_control_bytes(&self) -> &[u8] {
        &self.canonical_control_bytes
    }
    /// Hash used by the native immutable-input descriptor.
    pub(crate) fn canonical_control_hash(&self) -> Hash {
        Hash::new(&self.canonical_control_bytes)
    }
    /// Original admission position, preserved on later duplicate publication.
    pub(crate) fn priority(&self) -> QueuePlanAdmissionPriorityV1 {
        self.source.priority()
    }
    /// Actual first carrier, independent of the opening carrier or current tip.
    pub(crate) fn carrier_hash(&self) -> HashOf<BlockHeader> {
        self.source.carrier_hash()
    }
}

/// Read outcomes keep stale authority and a physically owned recovery need distinct.
#[derive(Debug)]
pub(crate) enum FirstLaneAdmittedInputReadV1 {
    /// Complete source evidence is locally available.
    Ready(VerifiedFirstLaneAdmittedInputV1),
    /// Install/retain a bounded existing certified-proposal-body request owner.
    /// This is a work requirement, not permission to park without an owner.
    CanonicalBodyRecoveryRequired(AuthenticatedLaneAdmittedInputSourceV1),
    /// Reread the coherent finalized context set after publication completes.
    ObservationChanged,
    /// The supplied immutable instance is not a member of the observed current set.
    InstanceNotCurrent,
}

impl State {
    /// Read the exact first admitted input of one currently observed lane instance.
    ///
    /// Copy the canonical source hash under State guards, drop every guard before
    /// Kura I/O, then recheck the observation. Missing historical proof, malformed
    /// storage, wrong index or wrong binding is an error, never a generic retry.
    pub(crate) fn first_lane_admitted_input(
        &self,
        observed: &VerifiedLaneContexts,
        lane: &VerifiedLaneContext,
    ) -> Result<FirstLaneAdmittedInputReadV1, String> {
        if !observed.is_current(self) {
            return Ok(FirstLaneAdmittedInputReadV1::ObservationChanged);
        }
        if !observed.contexts().iter().any(|current| {
            current.instance_id() == lane.instance_id() && current.frozen() == lane.frozen()
        }) {
            return Ok(FirstLaneAdmittedInputReadV1::InstanceNotCurrent);
        }
        let frozen = lane.frozen();
        let rank = frozen.admission_priority;
        if rank.carrier_height > frozen.opening_global_height
            || rank.carrier_height > observed.carrier_height()
        {
            return Err("pinned first admission is newer than its authenticated opening".into());
        }
        let height = usize::try_from(rank.carrier_height).map_err(|error| error.to_string())?;
        let height =
            NonZeroUsize::new(height).ok_or_else(|| "zero first-admission height".to_owned())?;
        let expected_hash = {
            let view = self.view();
            view.block_hashes.get(height.get() - 1).copied()
        };
        if !observed.is_current(self) {
            return Ok(FirstLaneAdmittedInputReadV1::ObservationChanged);
        }
        let expected_hash = expected_hash.ok_or_else(|| {
            "first-admission carrier is absent from coherent State history".to_owned()
        })?;
        #[cfg(test)]
        super::lane_consensus_verified::io_observer::notify();
        let result = (|| {
            let read = self
                .kura
                .read_first_admission_carrier(height, expected_hash)
                .map_err(|error| error.to_string())?;
            if read.finality.height_context.network_id != frozen.network_id {
                return Err("first-admission finality belongs to another network".into());
            }
            let source = AuthenticatedLaneAdmittedInputSourceV1 {
                network_id: frozen.network_id,
                priority: rank,
                binding_hash: frozen.admitted_binding_hash,
                lane_id: frozen.lane_id,
                dataspace_id: frozen.dataspace_id,
                incarnation: frozen.lane_incarnation,
                finality: read.finality,
            };
            match read.body {
                Some(body) => source
                    .project_control(&body)
                    .map(FirstLaneAdmittedInputReadV1::Ready),
                None => Ok(FirstLaneAdmittedInputReadV1::CanonicalBodyRecoveryRequired(
                    source,
                )),
            }
        })();
        // A stale publication cannot authorize use of either success or error.
        if !observed.is_current(self) {
            return Ok(FirstLaneAdmittedInputReadV1::ObservationChanged);
        }
        result
    }
}
