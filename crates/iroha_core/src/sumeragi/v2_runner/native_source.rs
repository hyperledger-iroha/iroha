//! Exact historical source request ownership, independent of global heights.

use super::*;
use crate::sumeragi::v2_lane_driver::NativeLaneDriver;
use crate::sumeragi::v2_transport::{
    AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse,
    authenticate_certified_body_request_with_validator_pops,
};
use crate::sumeragi::v2_worker::NativeSourceCompletionAdmission;
use crate::{NetworkMessage, sumeragi::message::BlockMessageWire};
use iroha_p2p::{
    Priority,
    network::{NetworkActorAdmissionError, NetworkActorAdmissionTicket, message::Post},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) enum NativeSourceTarget {
    Instance(wire::HeightContextId),
    Candidate,
    Validation(wire::BlockSubject),
}

pub(super) struct NativeSourceRequest {
    source: Arc<crate::state::AuthenticatedLaneAdmittedInputSourceV1>,
    target: NativeSourceTarget,
    request: Option<AuthenticatedCertifiedBodyRequest>,
    response: Option<AuthenticatedCertifiedBodyResponse>,
    message: Arc<BlockMessageWire>,
    peers: Vec<PeerId>,
    cursor: usize,
    returned: Option<Post<NetworkMessage>>,
    ticket: Option<NetworkActorAdmissionTicket>,
    next_retry: Instant,
}

impl NativeSourceRequest {
    pub(super) fn new(
        source: Arc<crate::state::AuthenticatedLaneAdmittedInputSourceV1>,
        target: NativeSourceTarget,
        local: &PeerId,
        key: &KeyPair,
        archives: Vec<PeerId>,
        now: Instant,
    ) -> Result<Self, V2RunnerError> {
        let artifact = source.finality();
        let mut request = wire::CertifiedBodyRequest {
            round: artifact.commit_qc.proposal_round,
            subject: artifact.subject,
            certificate: artifact.commit_qc.clone(),
            requester: local.clone(),
            signature: Vec::new(),
        };
        request.signature =
            iroha_crypto::Signature::try_new(key.private_key(), &request.signature_preimage())
                .map_err(|error| V2RunnerError::Service(error.to_string()))?
                .payload()
                .to_vec();
        let request = authenticate_certified_body_request_with_validator_pops(
            &artifact.height_context,
            &artifact.validator_set_pops,
            request,
            local,
        )
        .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        let message = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyRequest(
                request.request().clone(),
            )),
        )))
        .map(Arc::new)
        .map_err(|error| V2RunnerError::Service(error.to_string()))?;
        let peers = artifact
            .height_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .chain(archives)
            .filter(|peer| peer != local)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            source,
            target,
            request: Some(request),
            response: None,
            message,
            peers,
            cursor: 0,
            returned: None,
            ticket: None,
            next_retry: now,
        })
    }

    pub(super) fn admits_hash(&self, hash: HashOf<wire::CertifiedBodyRequest>) -> bool {
        self.response.is_none()
            && self
                .request
                .as_ref()
                .is_some_and(|request| request.request_hash() == hash)
    }

    pub(super) fn admits(&self, message: &BlockMessage) -> bool {
        self.response.is_none()
            && matches!(message,
            BlockMessage::V2(message) if matches!(&message.payload,
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                    if self.request.as_ref().is_some_and(|request| response.request_hash == request.request_hash())))
    }

    pub(super) fn accept(
        &mut self,
        response: wire::CertifiedBodyResponse,
        sender: &PeerId,
    ) -> Result<(), V2RunnerError> {
        let request = self.request.as_ref().ok_or_else(|| {
            V2RunnerError::Service("native source lost original signed request".into())
        })?;
        if response.request_hash != request.request_hash() || self.response.is_some() {
            return Err(V2RunnerError::Service(
                "native source selector changed exact response ownership".into(),
            ));
        }
        match request.authenticate_response(
            &self.source.finality().height_context,
            response,
            sender,
        ) {
            Ok(response) => self.response = Some(response),
            Err(error) => iroha_logger::debug!(%error, %sender, "rejected Native source response"),
        }
        Ok(())
    }

    pub(super) fn settle(
        &mut self,
        driver: &mut NativeLaneDriver,
        services: &mut ProductionV2Services,
        recovered: &mut BTreeMap<Hash, Arc<crate::state::VerifiedFirstLaneAdmittedInputV1>>,
    ) -> Result<bool, V2RunnerError> {
        if self.response.is_none() {
            return Ok(false);
        }
        match self.target {
            NativeSourceTarget::Candidate => {
                let input = self
                    .source
                    .complete_from_authenticated_response(
                        self.request.as_ref().expect("retained request"),
                        self.response.as_ref().expect("retained response"),
                    )
                    .map_err(V2RunnerError::Service)?;
                let binding = input.validated_input().certificate().binding_hash;
                recovered.insert(binding, Arc::new(input));
                Ok(true)
            }
            NativeSourceTarget::Instance(id) => {
                driver
                    .complete_source_recovery(
                        id,
                        self.request.as_ref().expect("retained request"),
                        self.response.as_ref().expect("retained response"),
                    )
                    .map_err(V2RunnerError::Service)?;
                Ok(true)
            }
            NativeSourceTarget::Validation(subject) => {
                let request = self.request.take().expect("retained request");
                let response = self.response.take().expect("retained response");
                match services.complete_native_source(subject, request, response) {
                    NativeSourceCompletionAdmission::Accepted => Ok(true),
                    NativeSourceCompletionAdmission::Retry { request, response } => {
                        self.request = Some(request);
                        self.response = Some(response);
                        Ok(false)
                    }
                    NativeSourceCompletionAdmission::Rejected {
                        request,
                        response,
                        reason,
                    } => {
                        self.request = Some(request);
                        self.response = Some(response);
                        Err(V2RunnerError::Service(reason))
                    }
                }
            }
        }
    }

    pub(super) fn poll(
        &mut self,
        network: &crate::IrohaNetwork,
        guard: &Arc<ConsensusOutputGuard>,
        now: Instant,
        retransmit: Duration,
    ) -> Result<(), V2RunnerError> {
        if self.response.is_some() || self.peers.is_empty() || now < self.next_retry {
            return Ok(());
        }
        let operation = guard
            .begin_fail_stop_operation()
            .ok_or(V2RunnerError::RestartRequired)?;
        let peer = &self.peers[self.cursor];
        let post = self.returned.take().unwrap_or_else(|| Post {
            data: NetworkMessage::SumeragiBlock(Arc::clone(&self.message)),
            peer_id: peer.clone(),
            priority: Priority::High,
        });
        let result = match network.post_recoverable(post, self.ticket.take()) {
            Ok(()) => {
                self.cursor = (self.cursor + 1) % self.peers.len();
                // One peer per control turn, then a bounded retransmission cadence.
                self.next_retry = if self.cursor == 0 {
                    deadline_after(now, retransmit)
                } else {
                    now
                };
                Ok(())
            }
            Err(error) => {
                let (post, ticket, reason) = match error {
                    NetworkActorAdmissionError::Backpressured {
                        message, ticket, ..
                    } => (message, ticket, None),
                    NetworkActorAdmissionError::Closed { message } => (
                        message,
                        None,
                        Some("Native source network actor closed".to_owned()),
                    ),
                    NetworkActorAdmissionError::Rejected { message, reason } => (
                        message,
                        None,
                        Some(format!("Native source frame rejected: {reason:?}")),
                    ),
                };
                let exact = post.peer_id == *peer
                    && post.priority == Priority::High
                    && matches!(&post.data, NetworkMessage::SumeragiBlock(message) if Arc::ptr_eq(message, &self.message));
                self.returned = Some(post);
                self.ticket = ticket;
                if !exact {
                    Err(V2RunnerError::Service(
                        "Native source actor returned another physical frame".into(),
                    ))
                } else if let Some(reason) = reason {
                    Err(V2RunnerError::Service(reason))
                } else {
                    Ok(())
                }
            }
        };
        if result.is_ok() {
            operation.complete();
        }
        result
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        (self.response.is_none() && !self.peers.is_empty()).then_some(self.next_retry)
    }
}
