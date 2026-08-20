#[derive(Clone, Debug, PartialEq, Eq)]
enum ExactOutputRolloverClaim {
    /// Manually assembled output has no semantic rollover authority.
    Exact,
    GlobalV2(ExactOutputCreationScope),
    /// Canonical payload chunks emitted in a separate fanout from their
    /// proposal. The exact manifest is retained so finality handoff can
    /// revalidate every chunk and signature against the applied context.
    PayloadChunks {
        scope: ExactOutputCreationScope,
        manifest: wire::PayloadManifest,
    },
    Lane(ExactOutputCreationScope),
    /// Kura-backed autonomous payload or NewView output that the successor
    /// rehydrates and deterministically retransmits with the same local
    /// authority.
    AutonomousLane {
        scope: ExactOutputCreationScope,
        local_peer: PeerId,
        proposal_height: u64,
    },
    /// Authenticated lane payload/NewView traffic with exact live ownership.
    ///
    /// These messages are admissible lane transport, but they have no
    /// independent applied-height reconstruction authority. They must leave
    /// the exact-output corridor before height handoff can complete.
    NonRetireableLaneTransport {
        target: PeerId,
        message_hash: HashOf<BlockMessage>,
    },
    DurableCommitCertificateResponse {
        scope: ExactOutputCreationScope,
        target: PeerId,
        responder: PeerId,
        source_height: wire::Height,
        source_context_id: wire::HeightContextId,
        response_hash: HashOf<wire::CommitCertificateResponse>,
    },
    DurableCertifiedBodyResponse {
        scope: ExactOutputCreationScope,
        target: PeerId,
        responder: PeerId,
        source_round: wire::ConsensusRound,
        source_subject: wire::BlockSubject,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    },
    DurableLaneCertificateResponse {
        scope: ExactOutputCreationScope,
        target: PeerId,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_height: u64,
        proposal_hash: Hash,
        certificate_hash: HashOf<LaneBlockCertificateV1>,
    },
    HistoricalLaneRecoveryRequest {
        scope: ExactOutputCreationScope,
        target: PeerId,
        request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
    },
    HistoricalLaneRecoveryResponse {
        scope: ExactOutputCreationScope,
        target: PeerId,
        request_hash: HashOf<LaneHistoricalRecoveryRequestV1>,
        response_hash: HashOf<LaneHistoricalRecoveryResponseV1>,
    },
    HistoricalLaneCertification {
        scope: ExactOutputCreationScope,
        target: PeerId,
        source_height: u64,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_hash: Hash,
        message_hash: HashOf<BlockMessage>,
    },
    DurableKuraReplicaAdvert {
        scope: ExactOutputCreationScope,
        source_height: u64,
        advert_hash: HashOf<KuraReplicaAdvertV1>,
    },
    NativeAmx {
        scope: ExactOutputCreationScope,
        round: wire::ConsensusRound,
        message_hash: HashOf<NativeAmxMessage>,
    },
    LaneDrainVote {
        scope: ExactOutputCreationScope,
        target: PeerId,
        vote_hash: HashOf<LaneDrainVoteV1>,
    },
    MergeShare {
        scope: ExactOutputCreationScope,
        share_hash: HashOf<MergeCommitteeSignature>,
    },
    QueuePlanAdmission {
        scope: ExactOutputCreationScope,
        target: PeerId,
        view: wire::View,
        certificate_hash: Hash,
    },
    CertifiedSidecarRequest {
        scope: ExactOutputCreationScope,
        target: PeerId,
        transfer: CertifiedSidecarTransferIdentity,
        request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    },
    CertifiedSidecarControl {
        scope: ExactOutputCreationScope,
        target: PeerId,
        message_hash: HashOf<CertifiedMergeSidecarMessage>,
    },
    CertifiedSidecarChunk {
        scope: ExactOutputCreationScope,
        target: PeerId,
        transfer: CertifiedSidecarTransferIdentity,
        chunk_index: u32,
        chunk_count: u32,
        response_hash: HashOf<CertifiedMergeSidecarChunkV1>,
    },
}
fn native_amx_message_body(
    message: &NativeAmxMessage,
) -> Result<&NativeAmxAttestationBodyV2, String> {
    let (body, expected_phase) = match message {
        NativeAmxMessage::PrepareRequest(request) => (&request.body, NativeAmxPhase::Prepare),
        NativeAmxMessage::PrepareVote(vote) => (&vote.body, NativeAmxPhase::Prepare),
        NativeAmxMessage::CommitRequest(request) => {
            request
                .validate_shape()
                .map_err(|error| error.to_string())?;
            (&request.request.body, NativeAmxPhase::Commit)
        }
        NativeAmxMessage::CommitVote(vote) => (&vote.body, NativeAmxPhase::Commit),
    };
    if body.phase != expected_phase || body.authority_context_height != body.round.height {
        return Err("Native AMX output has an invalid embedded round".to_owned());
    }
    Ok(body)
}
impl ExactOutputRolloverClaim {
    const fn accepts_superseded_reply_delivery(&self) -> bool {
        matches!(
            self,
            Self::DurableCommitCertificateResponse { .. }
                | Self::DurableCertifiedBodyResponse { .. }
        )
    }
    fn scope(&self) -> Option<ExactOutputCreationScope> {
        match self {
            Self::Exact | Self::NonRetireableLaneTransport { .. } => None,
            Self::GlobalV2(scope) | Self::Lane(scope) => Some(*scope),
            Self::PayloadChunks { scope, .. } => Some(*scope),
            Self::AutonomousLane { scope, .. } => Some(*scope),
            Self::DurableCommitCertificateResponse { scope, .. }
            | Self::DurableCertifiedBodyResponse { scope, .. }
            | Self::DurableLaneCertificateResponse { scope, .. }
            | Self::HistoricalLaneRecoveryRequest { scope, .. }
            | Self::HistoricalLaneRecoveryResponse { scope, .. }
            | Self::HistoricalLaneCertification { scope, .. }
            | Self::DurableKuraReplicaAdvert { scope, .. }
            | Self::NativeAmx { scope, .. }
            | Self::LaneDrainVote { scope, .. }
            | Self::MergeShare { scope, .. }
            | Self::QueuePlanAdmission { scope, .. }
            | Self::CertifiedSidecarRequest { scope, .. }
            | Self::CertifiedSidecarControl { scope, .. }
            | Self::CertifiedSidecarChunk { scope, .. } => Some(*scope),
        }
    }
    fn validate_non_retireable_lane_transport_fanout(
        messages: &[NetworkMessage],
        peers: &[PeerId],
        target: &PeerId,
        message_hash: HashOf<BlockMessage>,
    ) -> Result<(), String> {
        let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
            return Err(
                "non-retireable lane transport claim requires one exact message".to_owned(),
            );
        };
        let message = envelope.as_message();
        if peers != std::slice::from_ref(target)
            || !matches!(
                message,
                BlockMessage::LaneExecutablePayload(_)
                    | BlockMessage::LaneBlockNewViewVote(_)
                    | BlockMessage::LaneBlockNewViewCertificate(_)
            )
            || HashOf::new(message) != message_hash
        {
            return Err("non-retireable lane transport claim changed identity".to_owned());
        }
        Ok(())
    }
    fn validate_fanout(&self, messages: &[NetworkMessage], peers: &[PeerId]) -> Result<(), String> {
        match self {
            Self::Exact => Ok(()),
            Self::GlobalV2(_) => {
                if messages.iter().all(|message| {
                    matches!(
                        message,
                        NetworkMessage::SumeragiBlock(envelope)
                            if matches!(envelope.as_message(), BlockMessage::V2(_))
                    )
                }) {
                    Ok(())
                } else {
                    Err("global-v2 rollover claim covers a different output kind".to_owned())
                }
            }
            Self::PayloadChunks { manifest, .. } => {
                let manifest_hash = HashOf::new(manifest);
                if messages.len() != manifest.chunk_hashes.len() {
                    return Err(
                        "payload-chunk rollover claim changed the exact chunk count".to_owned()
                    );
                }
                for (expected_index, message) in messages.iter().enumerate() {
                    let NetworkMessage::SumeragiBlock(envelope) = message else {
                        return Err(
                            "payload-chunk rollover claim covers a non-Sumeragi message".to_owned()
                        );
                    };
                    let BlockMessage::V2(message) = envelope.as_message() else {
                        return Err("payload-chunk rollover claim covers a lane message".to_owned());
                    };
                    message
                        .validate_version()
                        .map_err(|error| error.to_string())?;
                    let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload
                    else {
                        return Err(
                            "payload-chunk rollover claim covers another v2 payload".to_owned()
                        );
                    };
                    if chunk.manifest_hash != manifest_hash
                        || usize::try_from(chunk.index).ok() != Some(expected_index)
                    {
                        return Err(
                            "payload-chunk rollover claim changed exact manifest coordinates"
                                .to_owned(),
                        );
                    }
                }
                Ok(())
            }
            Self::Lane(_) => {
                if messages.iter().all(|message| {
                    matches!(
                        message,
                        NetworkMessage::SumeragiBlock(envelope)
                            if matches!(
                                envelope.as_message(),
                                BlockMessage::LaneBlockProposal(_)
                                    | BlockMessage::LaneBlockVote(_)
                                    | BlockMessage::LaneBlockQc(_)
                                    | BlockMessage::LaneBlockCertificate(_)
                            )
                    )
                }) {
                    Ok(())
                } else {
                    Err("lane rollover claim covers a different output kind".to_owned())
                }
            }
            Self::AutonomousLane { .. } => {
                if messages.iter().all(|message| {
                    matches!(
                        message,
                        NetworkMessage::SumeragiBlock(envelope)
                            if matches!(
                                envelope.as_message(),
                                BlockMessage::LaneExecutablePayload(_)
                                    | BlockMessage::LaneBlockNewViewVote(_)
                                    | BlockMessage::LaneBlockNewViewCertificate(_)
                            )
                    )
                }) {
                    Ok(())
                } else {
                    Err("autonomous-lane rollover claim covers a different output kind".to_owned())
                }
            }
            Self::NonRetireableLaneTransport {
                target,
                message_hash,
            } => Self::validate_non_retireable_lane_transport_fanout(
                messages,
                peers,
                target,
                *message_hash,
            ),
            Self::DurableCommitCertificateResponse {
                target,
                responder,
                source_height,
                source_context_id,
                response_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "durable CommitQC response claim requires one exact message".to_owned()
                    );
                };
                let BlockMessage::V2(message) = envelope.as_message() else {
                    return Err("durable CommitQC response claim covers a lane message".to_owned());
                };
                let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) =
                    &message.payload
                else {
                    return Err(
                        "durable CommitQC response claim covers another v2 payload".to_owned()
                    );
                };
                if peers != std::slice::from_ref(target)
                    || &response.responder != responder
                    || response.certificate.round.height != *source_height
                    || response.certificate.round.context_id != *source_context_id
                    || HashOf::new(response) != *response_hash
                {
                    return Err("durable CommitQC response claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::DurableCertifiedBodyResponse {
                target,
                source_round,
                source_subject,
                response_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err("durable body response claim requires one exact message".to_owned());
                };
                let BlockMessage::V2(message) = envelope.as_message() else {
                    return Err("durable body response claim covers a lane message".to_owned());
                };
                let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) =
                    &message.payload
                else {
                    return Err("durable body response claim covers another v2 payload".to_owned());
                };
                if peers != std::slice::from_ref(target)
                    || response.manifest.round != *source_round
                    || response.manifest.subject != *source_subject
                    || HashOf::new(response) != *response_hash
                {
                    return Err("durable body response claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::DurableLaneCertificateResponse {
                target,
                lane_id,
                lane_block_height,
                proposal_height,
                proposal_hash,
                certificate_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "durable lane-certificate claim requires one exact message".to_owned()
                    );
                };
                let BlockMessage::LaneBlockCertificate(certificate) = envelope.as_message() else {
                    return Err(
                        "durable lane-certificate claim covers another block payload".to_owned(),
                    );
                };
                let descriptor = &certificate.proposal.descriptor;
                if peers != std::slice::from_ref(target)
                    || descriptor.lane_id != *lane_id
                    || descriptor.lane_block_height != *lane_block_height
                    || descriptor.proposal_height != *proposal_height
                    || certificate.proposal.proposal_hash != *proposal_hash
                    || HashOf::new(certificate.as_ref()) != *certificate_hash
                {
                    return Err("durable lane-certificate claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::HistoricalLaneRecoveryRequest {
                target,
                request_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "historical lane recovery request claim requires one exact message"
                            .to_owned(),
                    );
                };
                let BlockMessage::LaneHistoricalRecoveryRequest(request) = envelope.as_message()
                else {
                    return Err(
                        "historical lane recovery request claim covers another block payload"
                            .to_owned(),
                    );
                };
                if peers != std::slice::from_ref(target)
                    || HashOf::new(request.as_ref()) != *request_hash
                {
                    return Err(
                        "historical lane recovery request claim changed identity".to_owned()
                    );
                }
                Ok(())
            }
            Self::HistoricalLaneRecoveryResponse {
                target,
                request_hash,
                response_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "historical lane recovery response claim requires one exact message"
                            .to_owned(),
                    );
                };
                let BlockMessage::LaneHistoricalRecoveryResponse(response) = envelope.as_message()
                else {
                    return Err(
                        "historical lane recovery response claim covers another block payload"
                            .to_owned(),
                    );
                };
                if peers != std::slice::from_ref(target)
                    || response.request_hash != *request_hash
                    || HashOf::new(response.as_ref()) != *response_hash
                {
                    return Err(
                        "historical lane recovery response claim changed identity".to_owned()
                    );
                }
                Ok(())
            }
            Self::HistoricalLaneCertification {
                target,
                source_height,
                lane_id,
                lane_block_height,
                proposal_hash,
                message_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "historical lane certification claim requires one exact message"
                            .to_owned(),
                    );
                };
                let message = envelope.as_message();
                let identity = match message {
                    BlockMessage::LaneBlockProposal(proposal) => Some((
                        proposal.descriptor.proposal_height,
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                        proposal.proposal_hash,
                    )),
                    BlockMessage::LaneBlockVote(vote) => Some((
                        vote.body.proposal_height,
                        vote.body.lane_id,
                        vote.body.lane_block_height,
                        vote.body.proposal_hash,
                    )),
                    BlockMessage::LaneBlockQc(qc) => Some((
                        qc.body.proposal_height,
                        qc.body.lane_id,
                        qc.body.lane_block_height,
                        qc.body.proposal_hash,
                    )),
                    BlockMessage::LaneBlockCertificate(certificate) => Some((
                        certificate.proposal.descriptor.proposal_height,
                        certificate.proposal.descriptor.lane_id,
                        certificate.proposal.descriptor.lane_block_height,
                        certificate.proposal.proposal_hash,
                    )),
                    _ => None,
                };
                if peers != std::slice::from_ref(target)
                    || identity
                        != Some((*source_height, *lane_id, *lane_block_height, *proposal_hash))
                    || HashOf::new(message) != *message_hash
                {
                    return Err(
                        "historical lane certification claim changed identity".to_owned()
                    );
                }
                Ok(())
            }
            Self::DurableKuraReplicaAdvert {
                source_height,
                advert_hash,
                ..
            } => {
                let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
                    return Err(
                        "durable Kura replica advert claim requires one exact message".to_owned(),
                    );
                };
                let BlockMessage::KuraReplicaAdvert(advert) = envelope.as_message() else {
                    return Err(
                        "durable Kura replica advert claim covers another block payload".to_owned(),
                    );
                };
                if advert.height != *source_height
                    || HashOf::new(advert) != *advert_hash
                    || peers.iter().any(|peer| peer == &advert.keeper)
                    || peers.iter().collect::<BTreeSet<_>>().len() != peers.len()
                {
                    return Err("durable Kura replica advert claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::NativeAmx {
                scope,
                round,
                message_hash,
            } => {
                let [NetworkMessage::NativeAmx(message)] = messages else {
                    return Err("Native AMX rollover claim requires one exact message".to_owned());
                };
                let body = native_amx_message_body(message)?;
                if body.round != *round
                    || round.context_id != scope.context_id
                    || round.height != scope.height
                    || HashOf::new(message.as_ref()) != *message_hash
                {
                    return Err("Native AMX rollover claim changed semantic identity".to_owned());
                }
                Ok(())
            }
            Self::LaneDrainVote {
                target, vote_hash, ..
            } => {
                let [NetworkMessage::LaneDrainVote(vote)] = messages else {
                    return Err("lane-drain rollover claim requires one exact vote".to_owned());
                };
                vote.validate_ingress()
                    .map_err(|error| format!("lane-drain rollover claim is invalid: {error}"))?;
                if peers != std::slice::from_ref(target) || HashOf::new(vote.as_ref()) != *vote_hash
                {
                    return Err("lane-drain rollover claim changed semantic identity".to_owned());
                }
                Ok(())
            }
            Self::MergeShare { share_hash, .. } => {
                let [NetworkMessage::MergeCommitteeSignature(signature)] = messages else {
                    return Err("merge-share rollover claim requires one exact share".to_owned());
                };
                if HashOf::new(signature.as_ref()) != *share_hash {
                    return Err("merge-share rollover claim changed semantic identity".to_owned());
                }
                Ok(())
            }
            Self::QueuePlanAdmission {
                target,
                certificate_hash,
                ..
            } => {
                let [NetworkMessage::QueuePlanAdmissionCertificate(certificate)] = messages else {
                    return Err(
                        "QueuePlan admission rollover claim requires one exact certificate"
                            .to_owned(),
                    );
                };
                if peers != std::slice::from_ref(target)
                    || certificate.is_empty()
                    || certificate.len()
                        > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES
                    || Hash::new(certificate.as_slice()) != *certificate_hash
                {
                    return Err(
                        "QueuePlan admission rollover claim changed semantic identity".to_owned(),
                    );
                }
                Ok(())
            }
            Self::CertifiedSidecarRequest {
                target,
                transfer,
                request_hash,
                ..
            } => {
                let [NetworkMessage::CertifiedMergeSidecar(message)] = messages else {
                    return Err(
                        "sidecar-request rollover claim requires one exact request".to_owned()
                    );
                };
                let CertifiedMergeSidecarMessage::Request(request) = message.as_ref() else {
                    return Err("sidecar-request rollover claim covers a chunk".to_owned());
                };
                if peers != std::slice::from_ref(target)
                    || CertifiedSidecarTransferIdentity::from_request(request) != *transfer
                    || HashOf::new(request) != *request_hash
                {
                    return Err("sidecar-request rollover claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::CertifiedSidecarControl {
                target,
                message_hash,
                ..
            } => {
                let [NetworkMessage::CertifiedMergeSidecar(message)] = messages else {
                    return Err(
                        "sidecar-control rollover claim requires one exact message".to_owned()
                    );
                };
                if !matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::Close(_)
                        | CertifiedMergeSidecarMessage::CloseAck(_)
                        | CertifiedMergeSidecarMessage::GenerationHint(_)
                ) {
                    return Err("sidecar-control rollover claim covers a data transfer".to_owned());
                }
                if peers != std::slice::from_ref(target)
                    || HashOf::new(message.as_ref()) != *message_hash
                {
                    return Err("sidecar-control rollover claim changed identity".to_owned());
                }
                Ok(())
            }
            Self::CertifiedSidecarChunk {
                target,
                transfer,
                chunk_index,
                chunk_count,
                response_hash,
                ..
            } => {
                let [NetworkMessage::CertifiedMergeSidecar(message)] = messages else {
                    return Err(
                        "sidecar-chunk rollover claim requires one exact response".to_owned()
                    );
                };
                let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {
                    return Err("sidecar-chunk rollover claim covers a request".to_owned());
                };
                if peers != std::slice::from_ref(target)
                    || CertifiedSidecarTransferIdentity::from_chunk(chunk) != *transfer
                    || chunk.chunk_index != *chunk_index
                    || chunk.chunk_count != *chunk_count
                    || HashOf::new(chunk) != *response_hash
                {
                    return Err("sidecar-chunk rollover claim changed identity".to_owned());
                }
                Ok(())
            }
        }
    }
}
