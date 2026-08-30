pub(crate) const fn wire_payload_is_certified_fence_escape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    matches!(
        payload,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                phase: wire::GlobalPhase::Commit,
                ..
            })
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                wire::CommitCertificateResponse {
                    certificate: wire::QuorumCertificate {
                        phase: wire::GlobalPhase::Commit,
                        ..
                    },
                    ..
                }
            )
    )
}
/// Whether a direct productive certificate can cross the absolute timeout cut
/// after retaining an older durable leader-wire owner.
///
/// TimeoutVotes use their separate finite producer episode and gain neither
/// certified capacity nor signature-fence authority. Commit-certificate
/// discovery responses own no productive leader-wire token and therefore use
/// their ordinary projected CommitQC lifecycle instead.
const fn wire_payload_is_direct_certificate_recovery_shape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    matches!(
        payload,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                phase: wire::GlobalPhase::Commit,
                ..
            })
    )
}
fn wire_payload_matches_current_strict_timeout_recovery_round(
    payload: &wire::ConsensusMessageV2Payload,
    context: &wire::HeightContext,
    tag: EventTag,
) -> bool {
    let round = match payload {
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => vote.round,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => certificate.round,
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)
            if certificate.phase == wire::GlobalPhase::Commit =>
        {
            certificate.round
        }
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return false,
    };
    round.context_id == context.id() && round.height == tag.height() && round.view == tag.view()
}
/// Whether one authenticated Progress payload can make a retained future
/// PrepareQC runnable or terminally supersede it.
///
/// This predicate is deliberately separate from the strict current-round
/// timeout-recovery cut above. Adjacent TimeoutVotes belong to the reducer's
/// bounded catch-up window, an at-or-ahead TC installs a newer view, and any
/// CommitQC for this exact height is already a terminal decision. Full
/// certificate and signature validation remains downstream in the adapter and
/// reducer.
fn wire_payload_advances_or_supersedes_future_prepare_qc_fifo_block(
    payload: &wire::ConsensusMessageV2Payload,
    context: &wire::HeightContext,
    tag: EventTag,
) -> bool {
    let round = match payload {
        wire::ConsensusMessageV2Payload::TimeoutVote(vote)
            if timeout_vote_view_is_admissible(tag.view(), vote.round.view) =>
        {
            vote.round
        }
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate)
            if certificate.round.view >= tag.view() =>
        {
            certificate.round
        }
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)
            if certificate.phase == wire::GlobalPhase::Commit =>
        {
            certificate.round
        }
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => return false,
    };
    round.context_id == context.id()
        && round.height == context.height
        && round.height == tag.height()
}
fn network_command_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_) => Some(CommandClass::Progress),
        wire::ConsensusMessageV2Payload::Proposal(_) | wire::ConsensusMessageV2Payload::Vote(_) => {
            Some(CommandClass::Normal)
        }
        wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => None,
    }
}
fn classify_reducer_network_ingress(
    fail_closed: bool,
    payload: &wire::ConsensusMessageV2Payload,
) -> Result<CommandClass, NetworkIngressError> {
    if fail_closed {
        return Err(NetworkIngressError::FailClosed);
    }
    network_command_class(payload).ok_or(NetworkIngressError::TransportPayload)
}
#[cfg(test)]
fn network_admission_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        // The transport wrapper is authenticated against an outstanding
        // request, then unwrapped into the embedded CommitQC and admitted to
        // the same Progress prefix before discovery state is retired.
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
            Some(CommandClass::Progress)
        }
        _ => network_command_class(payload),
    }
}
