fn prepare_sumeragi_lane_relay_item(
    context: SumeragiRelayBuildContext,
    message: iroha_core::NetworkMessage,
) -> PrepareSumeragiRelayResult {
    use iroha_core::NetworkMessage::{
        CertifiedMergeSidecar, LaneDrainVote, LaneRelay, MergeCommitteeSignature, NativeAmx,
        QueuePlanAdmissionCertificate,
    };
    let peer_id = context.peer.id().clone();
    let item = match message {
        LaneRelay(envelope) => LaneRelayMessage::Envelope(*envelope),
        MergeCommitteeSignature(signature) => {
            LaneRelayMessage::MergeSignature(Arc::unwrap_or_clone(signature))
        }
        CertifiedMergeSidecar(message) => {
            let reply_route = certified_merge_sidecar_ingress_reply_route(
                message.as_ref(),
                context.reply_route.clone(),
            );
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: peer_id.clone(),
                reply_route: Some(reply_route),
                message: Arc::unwrap_or_clone(message),
            }
        }
        NativeAmx(message) => LaneRelayMessage::NativeAmx {
            sender: peer_id.clone(),
            reply_route: Some(context.reply_route.clone()),
            message: Arc::unwrap_or_clone(message),
        },
        QueuePlanAdmissionCertificate(certificate) => {
            LaneRelayMessage::QueuePlanAdmissionCertificate {
                sender: peer_id.clone(),
                certificate,
            }
        }
        LaneDrainVote(vote) => {
            let vote = *vote;
            if vote.signer != peer_id {
                iroha_logger::debug!(
                    peer = %context.peer,
                    signer = %vote.signer,
                    "rejecting lane-drain vote whose signed identity differs from its authenticated sender"
                );
                return context.terminal(SumeragiRelayTerminalOutcome::Failed);
            }
            LaneRelayMessage::DrainVote {
                sender: peer_id,
                vote,
            }
        }
        _ => {
            iroha_logger::error!(
                peer = %context.peer,
                "non-Sumeragi message reached the retained Sumeragi dispatcher"
            );
            return context.terminal(SumeragiRelayTerminalOutcome::Failed);
        }
    };
    context.prepared(
        SumeragiRelayClass::Lane,
        PreparedSumeragiRelayItem::Lane(Box::new(item)),
    )
}
