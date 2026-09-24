//! Worker-side preparation of one finality-bound canonical executed-body chunk.
//!
//! The source task retains the authenticated request occurrence while the
//! shared historical-body worker reads Kura, verifies State and finality, and
//! preencodes the exact response. The current runner does not yet route these
//! completions into exact output; that separate typed handoff must preserve the
//! task's ingress owner and retry an unchanged prepared frame on backpressure.

use std::sync::Arc;

use iroha_crypto::{HashOf, KeyPair};
use iroha_data_model::{NetworkId, block::consensus_v2 as wire};
use iroha_model_base::peer::PeerId;
use iroha_p2p::network::NetworkReplyRoutes;

use super::super::{
    FairV2IngressOwnershipEvidence, InboundBlockMessage,
    message::{
        BlockMessage, BlockMessageWire, LANE_HISTORICAL_RECOVERY_VERSION_V1,
        LaneHistoricalRecoveryKindV1, LaneHistoricalRecoveryPayloadV1,
        LaneHistoricalRecoveryRequestV1,
    },
    v2_lane_work::{
        CanonicalRecoveryReadError, V2LaneWorkLimits, build_canonical_executed_block_response,
        canonical_executed_block_request_fits_frame,
    },
};
use crate::{NetworkMessage, kura::Kura, state::State};

/// One exact authenticated request reserved for off-actor canonical source work.
pub(crate) struct CanonicalExecutedBodyServeTask {
    /// Exact request that must match the original fair-ingress occurrence.
    pub(crate) request: LaneHistoricalRecoveryRequestV1,
    /// Authenticated semantic requester and response target.
    pub(crate) recipient: PeerId,
    /// Original authenticated delivery routes.
    pub(crate) reply_routes: NetworkReplyRoutes,
    /// Original finite fair-ingress occurrence and route proof.
    pub(crate) ingress_ownership: FairV2IngressOwnershipEvidence,
    context: wire::HeightContext,
    state: Arc<State>,
    limits: V2LaneWorkLimits,
}

impl CanonicalExecutedBodyServeTask {
    fn validate_binding(
        request: &LaneHistoricalRecoveryRequestV1,
        recipient: &PeerId,
        authenticated_via: &PeerId,
        reply_routes: &NetworkReplyRoutes,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
        limits: V2LaneWorkLimits,
    ) -> Result<(), CanonicalRecoveryReadError> {
        if request.version != LANE_HISTORICAL_RECOVERY_VERSION_V1
            || &request.requester != recipient
            || request.certificate.is_some()
            || !request.signer_pops.is_empty()
            || !matches!(
                &request.kind,
                LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { .. }
            )
            || !canonical_executed_block_request_fits_frame(limits, request)
            || reply_routes.semantic_target() != recipient
            || !reply_routes
                .iter()
                .any(|route| route.is_authenticated_via(authenticated_via))
        {
            return Err(CanonicalRecoveryReadError::Rejected(
                "canonical executed-body request changed its kind, sender, route, or size"
                    .to_owned(),
            ));
        }
        // TODO: Fund this request clone and the nested ingress comparison from
        // the original physical admission owner before connecting live ingress.
        let exact_message = BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request.clone()));
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(&exact_message)
            || !ingress_ownership.matches_semantic_origin(recipient)
            || !ingress_ownership.matches_reply_routes(Some(reply_routes))
        {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical executed-body request lost its exact fair-ingress owner".to_owned(),
            ));
        }
        Ok(())
    }

    /// Consume exactly one authenticated fair-ingress carrier after validation.
    ///
    /// A rejected carrier is returned unchanged to its serialized ingress
    /// owner. The production caller must also retain a worker-refused task for
    /// local retry; neither ordinary nor terminal ingress is connected yet.
    #[allow(dead_code)] // TODO: Connect after physical charge and bounded retry ownership.
    pub(crate) fn from_authenticated_inbound(
        mut inbound: InboundBlockMessage,
        context: wire::HeightContext,
        state: Arc<State>,
        limits: V2LaneWorkLimits,
    ) -> Result<Self, (InboundBlockMessage, CanonicalRecoveryReadError)> {
        let BlockMessage::LaneHistoricalRecoveryRequest(request) = inbound.message() else {
            return Err((
                inbound,
                CanonicalRecoveryReadError::Rejected(
                    "canonical executed-body binding received another message kind".to_owned(),
                ),
            ));
        };
        let Some(reply_routes) = inbound.reply_routes() else {
            return Err((
                inbound,
                CanonicalRecoveryReadError::Rejected(
                    "canonical executed-body request has no authenticated reply route".to_owned(),
                ),
            ));
        };
        let Some(ingress_ownership) = inbound.ingress_ownership() else {
            return Err((
                inbound,
                CanonicalRecoveryReadError::LocalPersistence(
                    "canonical executed-body request has no fair-ingress owner".to_owned(),
                ),
            ));
        };
        if let Err(error) = Self::validate_binding(
            request,
            inbound.sender(),
            inbound.via(),
            reply_routes,
            ingress_ownership,
            limits,
        ) {
            return Err((inbound, error));
        }
        let ingress_ownership = inbound
            .take_ingress_ownership()
            .expect("validated fair-ingress owner remains attached");
        let (message, recipient, reply_routes) = inbound.into_message_sender_and_reply_routes();
        let BlockMessage::LaneHistoricalRecoveryRequest(request) = message else {
            unreachable!("validated canonical request retains its message kind")
        };
        Ok(Self {
            request: *request,
            recipient,
            reply_routes: reply_routes.expect("validated authenticated route remains attached"),
            ingress_ownership,
            context,
            state,
            limits,
        })
    }

    /// Borrow the original ingress owner for a future typed terminal handoff.
    #[allow(dead_code)] // TODO: Consume this owner in the typed canonical output post.
    pub(crate) fn ingress_ownership(&self) -> &FairV2IngressOwnershipEvidence {
        &self.ingress_ownership
    }
}

/// Private process-local attestation to a worker-checked Kura source and frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalExecutedBodyDurableSourceProof {
    network_id: NetworkId,
    responder: PeerId,
    source_height: u64,
    request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
    chunk_index: u32,
    exact_output_hash: HashOf<NetworkMessage>,
}

impl CanonicalExecutedBodyDurableSourceProof {
    fn mint(
        network_id: NetworkId,
        responder: PeerId,
        request: &LaneHistoricalRecoveryRequestV1,
        message: &NetworkMessage,
    ) -> Result<Self, CanonicalRecoveryReadError> {
        let LaneHistoricalRecoveryKindV1::CanonicalExecutedBlock { need, chunk_index } =
            &request.kind
        else {
            return Err(CanonicalRecoveryReadError::Rejected(
                "canonical source proof received another request kind".to_owned(),
            ));
        };
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical source proof lost Sumeragi block traffic".to_owned(),
            ));
        };
        let BlockMessage::LaneHistoricalRecoveryResponse(response) = envelope.as_message() else {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical source proof lost its response kind".to_owned(),
            ));
        };
        let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
            finality_artifact,
            wire_len,
            chunk_index: response_chunk_index,
            bytes,
            ..
        } = &response.payload
        else {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical source proof lost its chunk payload".to_owned(),
            ));
        };
        if response.version != LANE_HISTORICAL_RECOVERY_VERSION_V1
            || response.request_hash != HashOf::new(request)
            || *response_chunk_index != *chunk_index
            || finality_artifact.height != need.height
            || HashOf::new(finality_artifact) != need.finality_artifact_hash
            || finality_artifact.block_hash != need.block_hash
            || finality_artifact.commit_qc.execution_commitment != need.execution_commitment
            || *wire_len != need.executed_block_wire_len
            || bytes.is_empty()
        {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical source proof changed its finality, request, or chunk".to_owned(),
            ));
        }
        Ok(Self {
            network_id,
            responder,
            source_height: need.height,
            request_hash: response.request_hash,
            chunk_index: *chunk_index,
            exact_output_hash: message.exact_output_hash(),
        })
    }

    /// Match only the exact worker-warmed frame under the expected authority.
    pub(crate) fn covers_message(
        &self,
        network_id: &NetworkId,
        responder: &PeerId,
        message: &NetworkMessage,
    ) -> bool {
        if &self.network_id != network_id || &self.responder != responder {
            return false;
        }
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return false;
        };
        let BlockMessage::LaneHistoricalRecoveryResponse(response) = envelope.as_message() else {
            return false;
        };
        let LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
            finality_artifact,
            chunk_index,
            ..
        } = &response.payload
        else {
            return false;
        };
        response.request_hash == self.request_hash
            && finality_artifact.height == self.source_height
            && *chunk_index == self.chunk_index
            && message.cached_exact_output_hash() == Some(self.exact_output_hash)
    }

    /// Height of the immutable finality and body source.
    #[allow(dead_code)] // TODO: Validate this height in the typed rollover claim.
    pub(crate) const fn source_height(&self) -> u64 {
        self.source_height
    }
}

/// Exact canonical frame and its still-owned request occurrence.
#[allow(dead_code)] // TODO: Transfer these fields through a typed SourceRetained output handoff.
pub(crate) struct PreparedCanonicalExecutedBodyOutput {
    /// Source request and still-owned ingress capability.
    pub(crate) task: CanonicalExecutedBodyServeTask,
    /// Worker-encoded exact response frame.
    pub(crate) message: NetworkMessage,
    /// Process-local proof sealing the validated source to that frame.
    pub(crate) proof: CanonicalExecutedBodyDurableSourceProof,
}

/// Worker result. A rejection retains the task for future typed retirement.
#[allow(dead_code)] // TODO: Settle each result through the canonical ingress/output owner.
pub(crate) enum CanonicalExecutedBodyServeCompletion {
    /// The exact frame and source proof are ready for a typed output owner.
    Prepared(PreparedCanonicalExecutedBodyOutput),
    /// Remote shape/authority failure or local persistence failure.
    Failed(CanonicalExecutedBodyServeTask, CanonicalRecoveryReadError),
}

impl CanonicalExecutedBodyServeCompletion {
    /// Borrow the original task for its resource-reservation release.
    pub(crate) fn task(&self) -> &CanonicalExecutedBodyServeTask {
        match self {
            Self::Prepared(output) => &output.task,
            Self::Failed(task, _) => task,
        }
    }
}

/// Read and prepare one exact chunk on the isolated worker, never on the actor.
pub(super) fn prepare_canonical_executed_body_output(
    network_id: NetworkId,
    kura: &Kura,
    responder_key: &KeyPair,
    task: CanonicalExecutedBodyServeTask,
) -> CanonicalExecutedBodyServeCompletion {
    let prepared = (|| {
        if task.context.network_id != network_id {
            return Err(CanonicalRecoveryReadError::Rejected(
                "canonical source task changed the worker network".to_owned(),
            ));
        }
        let response = build_canonical_executed_block_response(
            &task.context,
            task.state.as_ref(),
            kura,
            task.limits,
            &task.request,
            &task.recipient,
        )?;
        let wire = BlockMessageWire::try_preencoded(Arc::new(
            BlockMessage::LaneHistoricalRecoveryResponse(Box::new(response)),
        ))
        .map_err(|error| CanonicalRecoveryReadError::LocalPersistence(error.to_string()))?;
        if wire.encoded_capacity().is_none_or(|capacity| {
            capacity > crate::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES
        }) {
            return Err(CanonicalRecoveryReadError::Rejected(
                "canonical executed-body response exceeds the worker's charged frame bound"
                    .to_owned(),
            ));
        }
        let message = NetworkMessage::SumeragiBlock(Arc::new(wire));
        let responder = PeerId::new(responder_key.public_key().clone());
        let proof = CanonicalExecutedBodyDurableSourceProof::mint(
            network_id,
            responder.clone(),
            &task.request,
            &message,
        )?;
        if !proof.covers_message(&network_id, &responder, &message) {
            return Err(CanonicalRecoveryReadError::LocalPersistence(
                "canonical executed-body worker did not seal its exact frame".to_owned(),
            ));
        }
        Ok((message, proof))
    })();
    match prepared {
        Ok((message, proof)) => {
            CanonicalExecutedBodyServeCompletion::Prepared(PreparedCanonicalExecutedBodyOutput {
                task,
                message,
                proof,
            })
        }
        Err(error) => CanonicalExecutedBodyServeCompletion::Failed(task, error),
    }
}
