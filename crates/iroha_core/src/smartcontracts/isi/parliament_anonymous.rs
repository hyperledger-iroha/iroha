//! The interfaces of the anonymous Parliament family, the
//! `iroha.governance.parliament.anonymous.v1` addition (design §3.2, §8.1, §9 and §12.3).
//!
//! - [`CreateAnonymousParliamentAttemptV1`] creates an attempt for exact proposal content.
//!   Any signer may submit it while fewer than `K` anonymous attempts are nonterminal (the
//!   admission cap of design §9). Core selects the purpose, freezes the policy from committed
//!   parameters, and derives every identifier, binding and the compare-and-set head; the
//!   caller chooses none of them. Excess admission is refused before any State is written.
//! - [`SubmitAnonymousParliamentTransitionV1`] records one account-signed fact or intent of
//!   design §8.1. Risk escalation and the two early-close intents need `CanManageParliament`;
//!   the sortition registrations are permissionless and derive their snapshot from committed
//!   State and the containing height; a response, an absence or an endorsement is the signing
//!   authority's own. No transaction applies a lifecycle transition: the positioned
//!   block-start pass does (`governance::parliament_anonymous::world_pass`).
//! - [`RegisterParliamentCredentialV1`] executes only as the sole instruction of a direct
//!   signed input under the network's candidate-testing selection. The handler takes the
//!   input's one-use original, which locates the proof the credential record names, and lets
//!   the attempt's reducer accept the authority's binding seat with its verified credential.
//!   The reducer rebuilds the election's registration ledger from the credentials World keeps,
//!   so seat and commitment freshness is checked before the proof is verified. The attempt and
//!   the new credential are written together.
//!
//! A refusal the node cannot decide defers the input instead of rejecting it: an unavailable
//! verifier or local read, or stored credentials that are not the election's, which is
//! World's corruption and never the signer's fault.
//!
//! TODO(design §12.3, slice 5): production enactment needs the exact proposal, and only its
//! digest is kept. A qualified release must store the admitted proposal with its attempt (or
//! resolve it from an owner the family selects) before it can certify a production attempt;
//! this release creates candidate-testing attempts only.

use iroha_data_model::{
    account::AccountId,
    governance::{
        anonymous_ballot::{
            AnonymousParliamentTransitionV1, CreateAnonymousParliamentAttemptV1,
            FrozenParliamentAttemptPolicyV1, ParliamentBallotParametersV1,
            ParliamentBallotPurposeV1, ParliamentCandidateTestingPolicyV1,
            ParliamentLifecycleParametersV1, RegisterParliamentCredentialV1,
            SubmitAnonymousParliamentTransitionV1, anonymous_governance_attempt_id_v1,
            digests::proposal_digest_v1,
        },
        types::{GovernanceAttemptStatusV1, ParliamentBody, ProposalKind, RiskTierV1},
    },
    isi::governance::ParliamentInvitationDecisionV1,
    permission::Permission,
};
use iroha_executor_data_model::permission::governance::CanManageParliament;
use ivm::error::ExecutionDeferral;
use mv::storage::StorageReadOnly;

use crate::{
    execution_attempt::ExecutionAttemptError,
    governance::parliament_anonymous::{
        AnonymousAttemptOriginV1, AnonymousParliamentAttemptStateV1, AnonymousParliamentErrorV1,
        world_pass::casting_window_limit,
    },
    smartcontracts::{
        Execute,
        isi::{
            Error,
            world::isi::{
                has_exact_permission, parliament_expected_head_v1,
                parliament_sortition_pulse_epoch_length_v1,
            },
        },
    },
    state::{
        StateReadOnly, StateTransaction, WorldReadOnly,
        parliament_anonymous_tables::{
            election_credentials, eligible_citizens, nonterminal_attempts, store_attempt,
        },
    },
};

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(reason.to_string().into())
}

/// The rejection of a refused reducer step, or a deferral recorded in `state` for a refusal the
/// node cannot decide.
pub(crate) fn anonymous_refusal(
    error: &AnonymousParliamentErrorV1,
    state: &mut StateTransaction<'_, '_>,
) -> Error {
    let deferral = if error.is_local() {
        Some(ExecutionDeferral::VerifierArtifactsUnavailable)
    } else if matches!(error, AnonymousParliamentErrorV1::CredentialStore) {
        Some(ExecutionDeferral::LocalInvariantViolation)
    } else {
        None
    };
    match deferral {
        Some(reason) => {
            let _ = state.defer_execution(reason);
            refused("local anonymous Parliament step did not complete")
        }
        None => refused(format!("anonymous Parliament step refused: {error}")),
    }
}

/// The original refusal of a read that may be deferred.
fn attempt_refusal(
    state: &mut StateTransaction<'_, '_>,
    error: ExecutionAttemptError<Error>,
) -> Error {
    match error {
        ExecutionAttemptError::Rejected(error) => error,
        ExecutionAttemptError::Deferred(reason) => {
            let _ = state.defer_execution(reason);
            refused("local anonymous Parliament read did not complete")
        }
    }
}

/// The time of the executing block in milliseconds.
fn block_time_ms(state: &StateTransaction<'_, '_>) -> Result<u64, Error> {
    u64::try_from(state._curr_block.creation_time().as_millis())
        .map_err(|_| refused("the block time exceeds the Parliament time domain"))
}

fn require_parliament_manager(
    authority: &AccountId,
    state: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let required: Permission = CanManageParliament.into();
    if has_exact_permission(&state.world, authority, &required) {
        Ok(())
    } else {
        Err(refused("not permitted: CanManageParliament"))
    }
}

/// The purpose of an attempt created now (design §12.3): the release's qualification, or the
/// network's committed candidate-testing selection, or none.
fn committed_purpose(state: &StateTransaction<'_, '_>) -> Result<ParliamentBallotPurposeV1, Error> {
    let candidate = state
        .world
        .parameters()
        .custom()
        .get(ParliamentCandidateTestingPolicyV1::parameter_id())
        .map(ParliamentCandidateTestingPolicyV1::from_custom_parameter)
        .transpose()
        .map_err(|error| refused(format!("committed candidate-testing selection: {error}")))?;
    ParliamentBallotPurposeV1::select_v1(candidate.as_ref()).ok_or_else(|| {
        refused(
            "no anonymous Parliament attempt may be created: the release is unqualified and \
             genesis selected no candidate testing",
        )
    })
}

/// The protocol-defined pipeline of `proposal` in the anonymous family, with its risk tier: the
/// baseline's bodies for the proposal kind, followed by the conditional Confirmation Jury of a
/// binding pipeline. The caller cannot choose either.
fn anonymous_pipeline_v1(proposal: &ProposalKind) -> (RiskTierV1, Vec<ParliamentBody>) {
    let (risk_tier, required) =
        crate::governance::parliament::parliament_attempt_policy_v1(proposal);
    let mut bodies: Vec<ParliamentBody> = required.iter().map(|body| body.body).collect();
    if bodies.contains(&ParliamentBody::PolicyJury) {
        bodies.push(ParliamentBody::ConfirmationJury);
    }
    (risk_tier, bodies)
}

/// The attempt policy frozen at admission from committed parameters (design §8.1): the
/// committed `gov.parliament_lifecycle` and `gov.parliament_ballot` values, or their defaults
/// while none is committed, at the committed block cadence and the native pulse cadence.
fn frozen_policy(
    pipeline: &[ParliamentBody],
    state: &mut StateTransaction<'_, '_>,
) -> Result<FrozenParliamentAttemptPolicyV1, Error> {
    let pulse_cadence_blocks = parliament_sortition_pulse_epoch_length_v1(state)?;
    let parameters = state.world.parameters();
    let custom = parameters.custom();
    let lifecycle = custom
        .get(ParliamentLifecycleParametersV1::parameter_id())
        .map(ParliamentLifecycleParametersV1::from_custom_parameter)
        .transpose()
        .map_err(|error| refused(format!("committed gov.parliament_lifecycle: {error}")))?
        .unwrap_or_default();
    let ballot = custom
        .get(ParliamentBallotParametersV1::parameter_id())
        .map(ParliamentBallotParametersV1::from_custom_parameter)
        .transpose()
        .map_err(|error| refused(format!("committed gov.parliament_ballot: {error}")))?
        .unwrap_or_default();
    let block_cadence_ms = parameters.sumeragi().block_cadence_ms.get();
    FrozenParliamentAttemptPolicyV1::freeze_v1(
        pipeline,
        &lifecycle,
        Some(&ballot),
        block_cadence_ms,
        pulse_cadence_blocks,
    )
    .map_err(|error| refused(format!("anonymous Parliament attempt policy: {error}")))
}

impl Execute for CreateAnonymousParliamentAttemptV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if let Some(reason) = self.proposal.first_release_exact_json_u64_invariant_error() {
            return Err(refused(reason));
        }
        let purpose = committed_purpose(state)?;
        let content = self.proposal_content_id();
        let id = self.governance_attempt_id();
        let attempts = &state.world.parliament_anonymous_attempts;
        if attempts.get(&id).is_some() {
            return Err(refused("the anonymous Parliament attempt already exists"));
        }
        // A retry continues its terminal, non-enacted predecessor and inherits the redraw
        // units the proposal has consumed (design §8.1).
        let randomness_redraws_before_attempt = match self.attempt_sequence.checked_sub(1) {
            None => 0,
            Some(previous) => {
                let predecessor = attempts
                    .get(&anonymous_governance_attempt_id_v1(content, previous))
                    .ok_or_else(|| {
                        refused("an anonymous Parliament retry requires its exact predecessor")
                    })?;
                if !matches!(
                    predecessor.attempt().status,
                    GovernanceAttemptStatusV1::Rejected
                        | GovernanceAttemptStatusV1::Superseded
                        | GovernanceAttemptStatusV1::ExecutionFailed
                ) {
                    return Err(refused(
                        "an anonymous Parliament retry requires a terminal non-enacted predecessor",
                    ));
                }
                predecessor
                    .randomness_redraws_used()
                    .map_err(|error| refused(format!("predecessor redraw count: {error}")))?
            }
        };
        // The admission cap: one casting position per nonterminal attempt (design §9).
        let limit = casting_window_limit(&*state.world)
            .map_err(|error| refused(format!("admission cap: {error}")))?;
        if nonterminal_attempts(&state.world.parliament_anonymous_due) >= limit {
            return Err(refused(format!(
                "the anonymous Parliament admission cap of {limit} nonterminal attempts is reached"
            )));
        }
        let (risk_tier, pipeline) = anonymous_pipeline_v1(&self.proposal);
        let policy = frozen_policy(&pipeline, state)?;
        let proposal_digest = proposal_digest_v1(&self.proposal)
            .map_err(|error| refused(format!("proposal digest: {error}")))?;
        let effect_preimage_hash = self.proposal.effect_preimage_hash_v1();
        let expected_head = parliament_expected_head_v1(&self.proposal, state)
            .map_err(|error| attempt_refusal(state, error))?;
        let created_at_ms = block_time_ms(state)?;
        let attempt = AnonymousParliamentAttemptStateV1::try_new(
            purpose,
            self.canonical_attempt(risk_tier),
            policy,
            proposal_digest,
            effect_preimage_hash,
            expected_head,
            AnonymousAttemptOriginV1 {
                created_at_ms,
                randomness_redraws_before_attempt,
            },
        )
        .map_err(|error| anonymous_refusal(&error, state))?;
        store_attempt(&mut state.world, id, attempt);
        Ok(())
    }
}

impl Execute for SubmitAnonymousParliamentTransitionV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        use AnonymousParliamentTransitionV1 as Transition;

        self.validate_static().map_err(refused)?;
        let id = self.governance_attempt_id;
        if matches!(
            self.transition,
            Transition::EscalateRisk(_)
                | Transition::CloseQualificationEarly
                | Transition::ClosePhaseEarly(_)
        ) {
            require_parliament_manager(authority, state)?;
        }
        let now_ms = block_time_ms(state)?;
        let height = state.block_height();
        let network_id = *state.network_id();
        let mut attempt = state
            .world
            .parliament_anonymous_attempts
            .get(&id)
            .cloned()
            .ok_or_else(|| refused("unknown anonymous Parliament attempt"))?;
        let snapshot = |state: &StateTransaction<'_, '_>,
                        attempt: &AnonymousParliamentAttemptStateV1| {
            eligible_citizens(
                &state.world.citizens,
                &attempt.policy().sortition_policy.minimum_citizenship_bond,
            )
        };
        let result = match self.transition {
            Transition::EscalateRisk(payload) => attempt.escalate_risk(id, payload.target),
            Transition::CloseQualificationEarly => attempt.close_qualification_early(id, now_ms),
            Transition::RegisterInitialSortition => {
                let candidates = snapshot(state, &attempt);
                attempt
                    .register_initial_sortition(id, &network_id, height, now_ms, candidates)
                    .map(|_| ())
            }
            Transition::RegisterSortitionRetry => {
                let eligible = snapshot(state, &attempt);
                attempt
                    .register_sortition_retry(id, &network_id, height, now_ms, eligible)
                    .map(|_| ())
            }
            Transition::RecordInvitationResponse(payload) => attempt
                .record_invitation_response(
                    id,
                    payload.election_attempt_id,
                    payload.body,
                    authority,
                    payload.decision == ParliamentInvitationDecisionV1::Accept,
                    now_ms,
                )
                .map(|_| ()),
            Transition::ClosePhaseEarly(intent) => attempt.close_phase_early(id, intent, now_ms),
            Transition::RecordAttemptAbsence(payload) => attempt.record_absence(
                id,
                payload.body_instance_id,
                payload.assignment_id,
                authority,
                now_ms,
            ),
            Transition::EndorsePublicFinding(payload) => attempt.endorse_public_finding(
                id,
                payload.body_instance_id,
                payload.result_root,
                authority,
                now_ms,
            ),
        };
        result.map_err(|error| anonymous_refusal(&error, state))?;
        store_attempt(&mut state.world, id, attempt);
        Ok(())
    }
}

impl Execute for RegisterParliamentCredentialV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let source = match state.consume_direct_parliament_registration() {
            Ok(source) => source,
            Err(error) => {
                return Err(match error.into_deferral() {
                    Ok(reason) => {
                        let _ = state.defer_execution(reason);
                        refused("local credential registration did not complete")
                    }
                    Err(error) => refused(error),
                });
            }
        };
        let now_ms = block_time_ms(state)?;
        let network_id = *state.network_id();
        let mut attempt = state
            .world
            .parliament_anonymous_attempts
            .get(&self.governance_attempt_id)
            .cloned()
            .ok_or_else(|| refused("unknown anonymous Parliament attempt"))?;
        // An election the attempt does not hold has no credentials; the reducer refuses it.
        let records = attempt
            .accepted_assignments(self.election_attempt_id)
            .map(|accepted| {
                election_credentials(&state.world.parliament_anonymous_registrations, accepted)
            })
            .unwrap_or_default();
        match attempt.register_credential(&network_id, authority, &self, &records, source, now_ms) {
            Ok((assignment, record)) => {
                store_attempt(&mut state.world, self.governance_attempt_id, attempt);
                state
                    .world
                    .parliament_anonymous_registrations
                    .insert(assignment, record);
                Ok(())
            }
            Err(error) => Err(anonymous_refusal(&error, state)),
        }
    }
}
