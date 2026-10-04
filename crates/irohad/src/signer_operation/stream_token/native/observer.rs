//! Independent observer signatures over fresh native current state and completed Check evidence.
use super::*;
use iroha_core::{
    execution_attempt::ExecutionAttemptError, query::signer_finality::SignerFinalityErrorV1,
};
use sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenEvidenceAdmissionErrorV1 as Admission;

mod completed;
#[cfg(test)]
pub(super) use completed::test_hooks;
use iroha_torii::sorafs::{
    StreamTokenObserverReplyV1, StreamTokenSignerCallErrorV1 as Error,
    StreamTokenStateObserverClientV1,
};
use sorafs_manifest::signer::{
    state_observation::SignerStateObserverTrustV1,
    stream_token_evidence::{
        SignerStreamTokenObservationPhaseV1 as ObservationPhase,
        SignerStreamTokenObservationRequestSubjectV1 as Subject,
        SignerStreamTokenObservationRequestV1, SignerStreamTokenStateObservationBodyV1,
        SignerStreamTokenStateObservationV1, SignerStreamTokenStateSubjectV1,
    },
};

pub(super) struct NativeStreamTokenObserverV1 {
    pub(super) source: Arc<NativeStreamTokenSourceV1>,
    pub(super) handle: String,
    pub(super) trust: SignerStateObserverTrustV1,
    pub(super) record: Vec<u8>,
}
impl StreamTokenStateObserverClientV1 for NativeStreamTokenObserverV1 {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn finalize_check(
        &self,
        instruction: &MutateSorafsStreamTokenAuthority,
    ) -> Result<iroha_data_model::transaction::SignedTransaction, Error> {
        let source = &self.source;
        let sorafs_manifest::signer::protocol::SignerPurposeBindingV1::StreamToken { provider_id } =
            source.binding.purpose
        else {
            return Err(Error::Refused);
        };
        if instruction.request.provider_id != ProviderId::new(provider_id) {
            return Err(Error::Refused);
        }
        // TODO: the transport must carry the caller's original Check deadline and separate
        // sign-only custody from submission before this service can bind before publication.
        let deadline = source
            .transactions
            .start_deadline()
            .map_err(|_| Error::Unavailable)?;
        let signed = source
            .transactions
            .sign(instruction, true, deadline)
            .map_err(|_| Error::Refused)?;
        source
            .transactions
            .submit_and_wait(&signed, deadline)
            .map_err(|_| Error::Unavailable)?;
        Ok(signed)
    }
    fn observe(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, Error> {
        request
            .encode_canonical()
            .map_err(|error| evidence_call_error(&error, Error::Refused))?;
        let binding_digest =
            stream_token_binding_digest_v1(&self.source.binding).map_err(|_| Error::Refused)?;
        let Subject::CurrentCustody {
            binding_digest: expected,
        } = request.subject
        else {
            return completed::observe(self, request, binding_digest);
        };
        if expected != binding_digest {
            return Err(Error::Refused);
        }
        // Startup has no operation body and must not wait for consensus to start.
        // This same-State/Kura read qualifies custody only; it grants no signing.
        let current = self
            .source
            .capture([0; 32])
            .map_err(|_| Error::Unavailable)?;
        let body = self
            .prepare_body(
                request,
                &current.control,
                current.anchor,
                SignerStreamTokenStateSubjectV1::CurrentCustody { binding_digest },
            )
            .map_err(|error| error.service_error())?;
        let payload = body
            .signing_payload()
            .map_err(|error| evidence_call_error(&error, Error::Unavailable))?;
        let signature = self
            .source
            .transactions
            .sign_observation_payload(&payload)
            .map_err(|_| Error::Unavailable)?;
        let observation = SignerStreamTokenStateObservationV1 { body, signature }
            .encode_canonical()
            .map_err(|error| evidence_call_error(&error, Error::InvalidResponse))?;
        StreamTokenObserverReplyV1::current(self.record.clone(), observation)
    }
}

// Original native/codec causes remain owned until the local continuation ends.
// Nested custody-row codecs still project their own refusals; that separate owner
// must be migrated before this boundary can retain those original errors.
enum AttemptError {
    Evidence(Admission, Error),
    Native(ExecutionAttemptError<SignerFinalityErrorV1>),
    Terminal(Error),
}
impl AttemptError {
    fn retryable(&self) -> bool {
        match self {
            Self::Evidence(error, _) => error.is_retryable(),
            Self::Native(error) => matches!(error, ExecutionAttemptError::Deferred(_)),
            Self::Terminal(_) => false,
        }
    }
    fn service_error(&self) -> Error {
        match self {
            Self::Evidence(error, rejected) => evidence_call_error(error, *rejected),
            Self::Native(_) => Error::Unavailable,
            Self::Terminal(error) => *error,
        }
    }
}

impl NativeStreamTokenObserverV1 {
    fn prepare_body(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
        control: &sorafs_manifest::signer::custody_control::SignerCustodyControlStateV1,
        anchor: sorafs_manifest::signer::custody::SignerCustodyAnchorV1,
        subject: SignerStreamTokenStateSubjectV1,
    ) -> Result<SignerStreamTokenStateObservationBodyV1, AttemptError> {
        let source = &self.source;
        let context = source
            .context(control, anchor)
            .map_err(|_| AttemptError::Terminal(Error::Refused))?;
        self.validate_floor(request, anchor)?;
        let time = source
            .time()
            .map_err(|_| AttemptError::Terminal(Error::Unavailable))?;
        let observed = time.earliest_unix_ms + (time.latest_unix_ms - time.earliest_unix_ms) / 2;
        if observed < request.not_before_unix_ms
            || time.earliest_unix_ms < self.trust.active_from_unix_ms
            || time.latest_unix_ms >= self.trust.active_until_unix_ms
        {
            return Err(AttemptError::Terminal(Error::Refused));
        }
        let expires = observed
            .checked_add(self.trust.max_state_age_ms)
            .ok_or(AttemptError::Terminal(Error::Refused))?
            .min(self.trust.active_until_unix_ms);
        Ok(SignerStreamTokenStateObservationBodyV1 {
            magic: SignerStreamTokenStateObservationBodyV1::magic(),
            request_digest: request
                .digest()
                .map_err(|error| AttemptError::Evidence(error, Error::Refused))?,
            phase: request.phase,
            subject,
            authority: self.trust.authority.clone(),
            chain_id: source.binding.chain_id.clone(),
            network_id: source.binding.network_id,
            observed_at_unix_ms: observed,
            expires_at_unix_ms: expires,
            current_anchor: anchor,
            active_head: context.active_head,
            signer_revoked: context.signer_revoked,
            attester_revoked: context.attester_revoked,
        })
    }

    fn validate_floor(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
        anchor: sorafs_manifest::signer::custody::SignerCustodyAnchorV1,
    ) -> Result<(), AttemptError> {
        // This fresh original view retires before a caller can wait on any refusal.
        let source = &self.source;
        let view = source.state.view();
        let floor =
            iroha_core::query::stream_token_custody::read_stream_token_custody_control_at_v1(
                &view,
                &source.binding,
                request.minimum_anchor.height,
            )
            .map_err(|_| AttemptError::Terminal(Error::Refused))?
            .ok_or(AttemptError::Terminal(Error::Refused))?;
        if floor.anchor != request.minimum_anchor || anchor.height < floor.anchor.height {
            return Err(AttemptError::Terminal(Error::Refused));
        }
        iroha_core::query::signer_finality::verify_signer_finality_v1(
            &view,
            floor.anchor.height,
            floor.anchor.block_hash,
        )
        .map_err(AttemptError::Native)?;
        Ok(())
    }
}

fn evidence_call_error(
    error: &sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenEvidenceAdmissionErrorV1,
    rejected: Error,
) -> Error {
    if error.is_retryable() {
        Error::Unavailable
    } else {
        rejected
    }
}
