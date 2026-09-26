//! Independent observer signatures over fresh native current state and completed Check evidence.
use super::*;
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
        let signed = source
            .transactions
            .sign(instruction, true)
            .map_err(|_| Error::Refused)?;
        source
            .transactions
            .submit_and_wait(&signed)
            .map_err(|_| Error::Unavailable)?;
        Ok(signed.transaction)
    }
    fn observe(
        &self,
        request: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, Error> {
        request.encode_canonical().map_err(|_| Error::Refused)?;
        let binding_digest =
            stream_token_binding_digest_v1(&self.source.binding).map_err(|_| Error::Refused)?;
        let (control, anchor, subject) = match request.subject {
            Subject::CurrentCustody {
                binding_digest: expected,
            } => {
                if expected != binding_digest {
                    return Err(Error::Refused);
                }
                // Startup has no operation body and must not wait for consensus to start.
                // This actual same-State/Kura read qualifies custody only; it grants no signing.
                let current = self
                    .source
                    .capture([0; 32])
                    .map_err(|_| Error::Unavailable)?;
                (
                    current.control,
                    current.anchor,
                    SignerStreamTokenStateSubjectV1::CurrentCustody { binding_digest },
                )
            }
            Subject::CompletedOperation {
                binding_digest: expected,
                operation_id,
                signing_payload_digest,
                signing_payload_size,
                signatures_digest,
                ..
            } => {
                if expected != binding_digest {
                    return Err(Error::Refused);
                }
                let current = self
                    .source
                    .capture(operation_id)
                    .map_err(|_| Error::Unavailable)?;
                let row = current.operation.ok_or(Error::Refused)?.operation;
                let reviewed = row.operation.reviewed;
                if reviewed.request.signing_payload_digest != signing_payload_digest
                    || reviewed.request.signing_payload_size != signing_payload_size
                {
                    return Err(Error::Refused);
                }
                let phase = match request.phase {
                    ObservationPhase::AfterCommit => Phase::AfterCommit(row),
                    ObservationPhase::BeforeRelease => Phase::BeforeRelease(row),
                    _ => return Err(Error::Refused),
                };
                let check = self
                    .source
                    .checked(reviewed, phase)
                    .map_err(|_| Error::Unavailable)?;
                check.ensure_live().map_err(|_| Error::Unavailable)?;
                let completed = check
                    .snapshot()
                    .completed_operation()
                    .ok_or(Error::Refused)?;
                if completed.signatures_digest != signatures_digest {
                    return Err(Error::Refused);
                }
                (
                    check.snapshot().control().clone(),
                    check.snapshot().anchor(),
                    SignerStreamTokenStateSubjectV1::CompletedOperation {
                        binding_digest,
                        operation_id,
                        signing_payload_digest,
                        signing_payload_size,
                        completed_operation: Box::new(completed.clone()),
                    },
                )
            }
        };
        let source = &self.source;
        let context = source
            .context(&control, anchor)
            .map_err(|_| Error::Refused)?;
        // Authenticate the caller's lower bound independently against native retained history.
        let view = source.state.view();
        let floor =
            iroha_core::query::stream_token_custody::read_stream_token_custody_control_at_v1(
                &view,
                &source.binding,
                request.minimum_anchor.height,
            )
            .map_err(|_| Error::Refused)?
            .ok_or(Error::Refused)?;
        if floor.anchor != request.minimum_anchor || anchor.height < floor.anchor.height {
            return Err(Error::Refused);
        }
        iroha_core::query::signer_finality::verify_signer_finality_v1(
            &view,
            floor.anchor.height,
            floor.anchor.block_hash,
        )
        .map_err(|_| Error::Unavailable)?;
        let time = source.time().map_err(|_| Error::Unavailable)?;
        let observed = time.earliest_unix_ms + (time.latest_unix_ms - time.earliest_unix_ms) / 2;
        if observed < request.not_before_unix_ms
            || time.earliest_unix_ms < self.trust.active_from_unix_ms
            || time.latest_unix_ms >= self.trust.active_until_unix_ms
        {
            return Err(Error::Refused);
        }
        let expires = observed
            .checked_add(self.trust.max_state_age_ms)
            .ok_or(Error::Refused)?
            .min(self.trust.active_until_unix_ms);
        let body = SignerStreamTokenStateObservationBodyV1 {
            magic: SignerStreamTokenStateObservationBodyV1::magic(),
            request_digest: request.digest().map_err(|_| Error::Refused)?,
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
        };
        let signature = source
            .transactions
            .sign_observation(&body)
            .map_err(|_| Error::Unavailable)?;
        let observation = SignerStreamTokenStateObservationV1 { body, signature }
            .encode_canonical()
            .map_err(|_| Error::InvalidResponse)?;
        match request.subject {
            Subject::CurrentCustody { .. } => {
                StreamTokenObserverReplyV1::current(self.record.clone(), observation)
            }
            Subject::CompletedOperation { .. } => {
                StreamTokenObserverReplyV1::completed(observation)
            }
        }
    }
}
