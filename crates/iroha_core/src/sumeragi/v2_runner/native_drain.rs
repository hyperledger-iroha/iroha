//! Process-owned collector for authenticated autoscale drain votes.
//! TODO: connect guarded local issuance, retained fanout and direct carrier
//! selection before this collector can complete lane retirement.

use super::*;
use crate::lane_consensus::{LaneDrainVoteState, LaneDrainVoteV1, aggregate_lane_drain_votes};
#[cfg(test)]
use iroha_data_model::merge::LaneDrainCertificateV1;

/// Retain one pending close across global-height changes without consulting a
/// receiver's asynchronous Queue or Kura inventory for remote vote admission.
pub(crate) struct NativeDrainOwner {
    votes: LaneDrainVoteState,
}

impl NativeDrainOwner {
    /// Construct the sole drain-vote collector for a Native runner process.
    pub(crate) fn new() -> Self {
        Self {
            votes: LaneDrainVoteState::new(),
        }
    }

    /// Authenticate one transport-bound vote against the committed parent.
    /// Remote-invalid or not-yet-current votes are ignored; a failure to
    /// aggregate already authenticated exact-quorum votes fails the owner.
    pub(crate) fn accept_remote_vote(
        &mut self,
        state: &State,
        sender: PeerId,
        vote: LaneDrainVoteV1,
        now: Instant,
    ) -> Result<bool, V2LaneWorkError> {
        if sender != vote.signer || vote.validate_ingress().is_err() {
            return Ok(false);
        }
        let Ok(Some((body, committee))) =
            state.committed_autoscale_lane_drain_body_for_frontier(vote.body.final_frontier)
        else {
            return Ok(false);
        };
        if vote.body != body || !committee.contains(&vote.signer) {
            return Ok(false);
        }
        self.votes.retain_body(Some(body.clone()));
        let inserted = match self.votes.insert_vote(vote, now) {
            Ok(inserted) => inserted,
            Err(_) => return Ok(false),
        };
        if self.votes.certificate().is_none()
            && self.votes.votes().len()
                >= usize::try_from(body.intent.min_quorum).unwrap_or(usize::MAX)
        {
            let votes = self.votes.votes().values().cloned().collect::<Vec<_>>();
            let certificate =
                aggregate_lane_drain_votes(body, committee, &votes).map_err(|error| {
                    V2LaneWorkError::InvalidContext(format!(
                        "authenticated Native drain quorum could not aggregate: {error}"
                    ))
                })?;
            self.votes.set_certificate(certificate);
        }
        Ok(inserted)
    }

    /// Inspect the quorum produced by this collector in focused tests.
    #[cfg(test)]
    pub(crate) fn certificate(&self) -> Option<&LaneDrainCertificateV1> {
        self.votes.certificate()
    }
}
