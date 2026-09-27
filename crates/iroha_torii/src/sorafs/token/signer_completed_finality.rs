//! Purpose-owned joins between private receipt claims and native finalized role-11 Checks.

use super::{
    StreamTokenIssuerError, StreamTokenStateObserverClientV1, signer_lifecycle::SignerClockV1,
};
use iroha_core::query::stream_token_authority::observation::{
    PendingStreamTokenCheckV1, StreamTokenCheckExpectedV1, StreamTokenEligibilityTimeIntervalV1,
    VerifiedStreamTokenCheckV1, begin_stream_token_check_v1, capture_stream_token_authority_v1,
};
use iroha_data_model::{
    account::AccountId,
    sorafs::stream_token_authority::{
        StreamTokenCheckPhaseV1, StreamTokenOutcomeV1, StreamTokenReviewedV1,
    },
};
use sorafs_manifest::signer::{
    protocol::signer_operation_signatures_digest_v1,
    stream_token::SignerStreamTokenReceiptV1,
    stream_token_evidence::{
        SignerStreamTokenObservationPhaseV1 as Phase, SignerStreamTokenStateObservationBodyV1,
        SignerStreamTokenStateSubjectV1,
    },
};
use std::time::Duration;

pub(super) enum PendingCompletedFinalityV1 {
    Native(Box<PendingStreamTokenCheckV1>),
    #[cfg(test)]
    Simulated,
}
pub(super) enum CompletedFinalityV1 {
    Native(Box<VerifiedStreamTokenCheckV1>),
    #[cfg(test)]
    Simulated,
}
fn unavailable() -> StreamTokenIssuerError {
    StreamTokenIssuerError::SignerFinalityUnavailable
}

impl PendingCompletedFinalityV1 {
    pub(super) fn verify(
        self,
        observation: &SignerStreamTokenStateObservationBodyV1,
        clock: &dyn SignerClockV1,
        uncertainty_ms: u64,
    ) -> Result<CompletedFinalityV1, StreamTokenIssuerError> {
        let proof = match self {
            Self::Native(pending) => {
                let verified = (*pending).verify_finalized(|| {
                    let now = clock.now_unix_ms().map_err(|_| iroha_core::query::stream_token_authority::observation::StreamTokenObservationErrorV1::Clock)?;
                    eligibility_interval(now, uncertainty_ms)
                }).map_err(|_| unavailable())?;
                CompletedFinalityV1::Native(Box::new(verified))
            }
            #[cfg(test)]
            Self::Simulated => CompletedFinalityV1::Simulated,
        };
        proof.validate(observation)?;
        Ok(proof)
    }
}
impl CompletedFinalityV1 {
    pub(super) fn validate(
        &self,
        observation: &SignerStreamTokenStateObservationBodyV1,
    ) -> Result<(), StreamTokenIssuerError> {
        match self {
            Self::Native(proof) => {
                proof.ensure_live().map_err(|_| unavailable())?;
                let snapshot = proof.snapshot();
                let SignerStreamTokenStateSubjectV1::CompletedOperation {
                    completed_operation: completed,
                    ..
                } = &observation.subject
                else {
                    return Err(unavailable());
                };
                if snapshot.completed_operation() != Some(completed.as_ref())
                    || snapshot.anchor() != observation.current_anchor
                    || snapshot.control().active_head != Some(observation.active_head)
                    || snapshot.control().signer_revoked != observation.signer_revoked
                    || snapshot.control().attester_revoked != observation.attester_revoked
                {
                    return Err(unavailable());
                }
                let iroha_data_model::sorafs::stream_token_authority::StreamTokenAuthorityActionV1::Check(check) = &proof.instruction().request.action else { return Err(unavailable()); };
                if !matches!(
                    (&check.phase, observation.phase),
                    (StreamTokenCheckPhaseV1::AfterCommit(_), Phase::AfterCommit)
                        | (
                            StreamTokenCheckPhaseV1::BeforeRelease(_),
                            Phase::BeforeRelease
                        )
                ) {
                    return Err(unavailable());
                }
                Ok(())
            }
            #[cfg(test)]
            Self::Simulated => Ok(()),
        }
    }
}

impl super::signer_finality::CoreFinalityV1 {
    pub(super) fn prepare_native_completed_check(
        &self,
        receipt: &SignerStreamTokenReceiptV1,
        phase: Phase,
        observer: &dyn StreamTokenStateObserverClientV1,
    ) -> Result<PendingCompletedFinalityV1, StreamTokenIssuerError> {
        let snapshot = capture_stream_token_authority_v1(
            &self.state.view(),
            self.pins.binding(),
            receipt.request.operation_id,
        )
        .map_err(|_| unavailable())?;
        super::signer_finality::check_control_pins(&snapshot.control, &self.pins)?;
        let record = snapshot.operation.ok_or_else(unavailable)?;
        let reviewed = StreamTokenReviewedV1 {
            request: receipt.request,
            intent: receipt.intent,
        };
        let StreamTokenOutcomeV1::Completed(complete) = record.operation.operation.outcome else {
            return Err(unavailable());
        };
        if record.operation.operation.reviewed != reviewed
            || complete.reviewed != reviewed
            || complete.reservation != receipt.reservation
            || complete.commitment != receipt.commitment
            || complete.signatures_digest
                != signer_operation_signatures_digest_v1(&receipt.signatures)
                    .map_err(|_| unavailable())?
        {
            return Err(unavailable());
        }
        let native_phase = match phase {
            Phase::AfterCommit => StreamTokenCheckPhaseV1::AfterCommit(record.operation),
            Phase::BeforeRelease => StreamTokenCheckPhaseV1::BeforeRelease(record.operation),
            _ => return Err(unavailable()),
        };
        let expected = StreamTokenCheckExpectedV1 {
            binding: self.pins.binding().clone(),
            observer: AccountId::new(self.pins.observer_trust().public_key.clone()),
            expected_operator: snapshot.operator,
            control_revision: snapshot.control_revision,
            control_digest: snapshot.anchor.state_digest,
            reviewed,
            phase: native_phase,
            floor: snapshot.floor,
        };
        let prepared =
            begin_stream_token_check_v1(self.state.clone(), expected, Duration::from_secs(60))
                .map_err(|_| unavailable())?;
        let signed = observer
            .finalize_check(prepared.instruction())
            .map_err(|_| unavailable())?;
        let pending = prepared
            .bind_signed_transaction(signed)
            .map_err(|_| unavailable())?;
        Ok(PendingCompletedFinalityV1::Native(Box::new(pending)))
    }
}

fn eligibility_interval(
    now: u64,
    uncertainty: u64,
) -> Result<
    StreamTokenEligibilityTimeIntervalV1,
    iroha_core::query::stream_token_authority::observation::StreamTokenObservationErrorV1,
> {
    use iroha_core::query::stream_token_authority::observation::StreamTokenObservationErrorV1::Clock;
    let earliest_unix_ms = now
        .checked_sub(uncertainty)
        .filter(|time| *time > 0)
        .ok_or(Clock)?;
    let latest_unix_ms = now
        .checked_add(uncertainty)
        .filter(|time| *time < u64::MAX)
        .ok_or(Clock)?;
    Ok(StreamTokenEligibilityTimeIntervalV1 {
        earliest_unix_ms,
        latest_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eligibility_interval_checks_both_bounds_without_saturation() {
        let interval = eligibility_interval(10_000, 250).expect("bounded clock");
        assert_eq!(interval.earliest_unix_ms, 9_750);
        assert_eq!(interval.latest_unix_ms, 10_250);
        assert!(eligibility_interval(250, 250).is_err());
        assert!(eligibility_interval(u64::MAX - 250, 250).is_err());
    }
}
