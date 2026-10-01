//! Two-phase outbox release; the exact native command precedes hardware exposure.
use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderPublicInputsV1, SenderTerminalReceiptV1,
};

/// Original physical nonce and durable storage selected for an actual verified receiver ACK.
pub struct KagemushaNativeOutboxReleaseOriginalsV1 {
    /// Original private intent destination; never chosen by a C/JNI caller.
    pub intent_directory: PathBuf,
    /// Original hardware one-use challenge, retained unchanged for an uncertain command.
    pub hardware_one_use_nonce: [u8; 32],
    /// Exact native checkpoint destination fixed before device dispatch.
    pub destination: KagemushaNativeCorePublicationDestinationV1,
}
/// Independently installed originals for redemption finality. Mobile frames select no policy,
/// checkpoint, replay floor, clock, private destination, or authority owner through this intake.
/// Complete data archives remain untrusted until the actual native Core verifies them.
pub trait KagemushaNativeRedemptionFinalitySourceV1: Send + Sync + 'static {
    /// Recheck original signed policy, freshness custody and platform availability.
    fn recheck_originals(&self) -> Result<()>;
    /// Read the current independently trusted native time; a mobile clock is never accepted.
    fn current_trusted_native_time_ms(&self) -> Result<u64>;
    /// Read exact full status and threshold-signed bootstrap archives for this native voucher.
    fn original_finality(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
        operation_id: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>)>;
    /// Borrow original independently retained policy, scope, replay and time selections.
    fn bootstrap_pins<'a>(
        &'a self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
    ) -> Result<iroha_data_model::kagemusha::KagemushaMobileBootstrapPinsV1<'a>>;
    /// Read the exact original hardware challenge and native durable destinations only after
    /// full original finality has been authenticated under the actual installed Core owner.
    fn release_originals(
        &self,
        selection: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedRedemptionFinalitySelectionV1<'_>,
    ) -> Result<KagemushaNativeOutboxReleaseOriginalsV1>;
}
static FINALITY_SOURCE: OnceLock<Arc<dyn KagemushaNativeRedemptionFinalitySourceV1>> =
    OnceLock::new();
/// Register one immutable Rust-only original finality owner. Registration grants no monetary
/// authority; every archive and policy selection is verified again by the actual native Core.
pub fn register_kagemusha_native_redemption_finality_source_v1(
    source: Arc<dyn KagemushaNativeRedemptionFinalitySourceV1>,
) -> Result<()> {
    source.recheck_originals()?;
    FINALITY_SOURCE.set(source).map_err(|_| Error::Rejected)
}
/// Native original path hints for an already completed release. These values grant no authority;
/// every retry must prove the complete original private WAL under the installed native Core.
#[derive(Clone)]
pub struct KagemushaNativeCompletedOutboxReleaseLocatorV1 {
    /// Original private release WAL directory, selected by native custody.
    pub intent_directory: PathBuf,
    /// Original native publication path and immutable checkpoint identity.
    pub destination: KagemushaNativeCorePublicationDestinationV1,
}

#[derive(Clone)]
pub(super) struct ReleaseAttempt {
    operation_id: [u8; 32],
    original_command: Vec<u8>,
    intent_directory: PathBuf,
    request: Vec<Vec<u8>>,
    response: Vec<Vec<u8>>,
    destination: KagemushaNativeCorePublicationDestinationV1,
    pub(super) completion: Option<Vec<u8>>,
}
impl ReleaseAttempt {
    // A pending original can be prepared again only for exactly the same public request.
    // A retained response closes preparation; only its original completion may resolve it.
    fn retry_response(&self, request: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        if self.request != request || self.completion.is_some() {
            return Err(Error::Rejected);
        }
        Ok(self.response.clone())
    }
    fn require_completion_tuple(
        &self,
        operation: [u8; 32],
        command: &[u8],
        response: &[u8],
    ) -> Result<()> {
        if self.operation_id != operation
            || self.original_command != command
            || self
                .completion
                .as_ref()
                .is_some_and(|original| original != response)
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
}

fn release_retirement_ready(attempt: &ReleaseAttempt, selected: bool) -> bool {
    selected && attempt.completion.is_some()
}

const MAX_COMPLETED_RELEASE_LOCATORS: usize = 16;

fn retain_completed_locator(
    locators: &mut std::collections::BTreeMap<
        [u8; 32],
        KagemushaNativeCompletedOutboxReleaseLocatorV1,
    >,
    operation: [u8; 32],
    locator: KagemushaNativeCompletedOutboxReleaseLocatorV1,
) {
    if !locators.contains_key(&operation) && locators.len() >= MAX_COMPLETED_RELEASE_LOCATORS {
        // Retiring a hint never removes the original WAL. Native source lookup remains mandatory
        // for an evicted operation; absent genuine lookup support fails closed.
        if let Some(retired_key) = locators.keys().next().copied() {
            locators.remove(&retired_key);
        }
    }
    locators.insert(operation, locator);
}

impl NativeCoreWorkOwnerV1 {
    // This is a read-only original completion verification. It cannot resume or replace a pending
    // different operation, and the locator map supplies no evidence by itself.
    fn completed_release_retry(
        &self,
        operation: [u8; 32],
        command: &[u8],
        response: &[u8],
    ) -> Result<bool> {
        let Stage::Selected(owner) = &self.stage else {
            return Ok(false);
        };
        let Some(record) = owner
            .outgoing_record_for_operation(operation)
            .map_err(|_| Error::Rejected)?
        else {
            return Ok(false);
        };
        if record.phase != KagemushaOutgoingOperationPhaseV1::Released {
            return Ok(false);
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let locator = match self.completed_release_locators.get(&operation) {
            Some(locator) => locator.clone(),
            None => source.completed_outbox_release_locator(owner, operation)?,
        };
        self.verify_completed_release_locator(operation, command, response, &locator)?;
        source.recheck_originals()?;
        Ok(true)
    }

    fn verify_completed_release_locator(
        &self,
        operation: [u8; 32],
        command: &[u8],
        response: &[u8],
        locator: &KagemushaNativeCompletedOutboxReleaseLocatorV1,
    ) -> Result<()> {
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        require_child(Path::new(&self.path), &locator.intent_directory)?;
        self.destination(&locator.destination)?;
        self.selected()?
            .verify_completed_outbox_release_existing(
                &locator.intent_directory,
                &locator.destination.directory,
                locator.destination.checkpoint_operation_id,
                operation,
                command,
                response,
            )
            .map_err(|_| Error::Rejected)?;
        self.recheck_originals()?;
        source.recheck_originals()
    }

    // An exact retry of a previously authenticated response stays on the genuine exclusive
    // native attempt. This correlation grants no completion: Core re-verifies its signed frame
    // and original WAL on every retry, including when no transient token slot was available.
    pub(in super::super) fn has_retained_release_completion(
        &self,
        operation: [u8; 32],
        command: &[u8],
        response: &[u8],
    ) -> Result<bool> {
        let Some(attempt) = &self.release_attempt else {
            return Ok(false);
        };
        if attempt.operation_id != operation || attempt.completion.is_none() {
            return Ok(false);
        }
        attempt.require_completion_tuple(operation, command, response)?;
        self.recheck_originals()?;
        Ok(true)
    }

    // The only retirement gate is a successfully published owner plus freshly verified original
    // completed WAL and native Released tombstone. Publication/response uncertainty retains all.
    pub(in super::super) fn retire_completed_release_attempt(
        &mut self,
    ) -> Result<Option<[u8; 32]>> {
        let Some(attempt) = &self.release_attempt else {
            return Ok(None);
        };
        if !release_retirement_ready(attempt, matches!(self.stage, Stage::Selected(_))) {
            return Ok(None);
        }
        let response = attempt.completion.as_ref().ok_or(Error::Rejected)?;
        let operation = attempt.operation_id;
        let locator = KagemushaNativeCompletedOutboxReleaseLocatorV1 {
            intent_directory: attempt.intent_directory.clone(),
            destination: attempt.destination.clone(),
        };
        self.verify_completed_release_locator(
            operation,
            &attempt.original_command,
            response,
            &locator,
        )?;
        retain_completed_locator(&mut self.completed_release_locators, operation, locator);
        if self
            .terminal_attempt
            .as_ref()
            .is_some_and(|original| original.operation_id == operation)
        {
            self.terminal_attempt = None;
        }
        self.release_attempt = None;
        Ok(Some(operation))
    }

    pub(in super::super) fn completed_release_destination(
        path: &str,
        cap: &iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutboxReleaseV1,
        original_directory: &str,
        original_checkpoint_id: [u8; 32],
    ) -> Result<KagemushaNativeCorePublicationDestinationV1> {
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        let destination = source.outbox_release_recovery_destination(cap)?;
        require_child(Path::new(path), &destination.directory)?;
        if destination.checkpoint_operation_id == [0; 32]
            || destination.checkpoint_operation_id != original_checkpoint_id
            || destination.directory.to_str() != Some(original_directory)
        {
            return Err(Error::Rejected);
        }
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        Ok(destination)
    }
    pub(in super::super) fn from_pending_outbox_release(
        path: String,
        cap: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutboxReleaseV1,
    ) -> Result<Self> {
        let selected = cap
            .current_recovery_selection()
            .map_err(|_| Error::Rejected)?;
        let original_enrollment = selected.enrollment_binding().clone();
        let selected_source = super::super::enrolled_open::authenticated_recovery_source(&selected)
            .map_err(|_| Error::Rejected)?;
        let release = cap.authenticated_release().map_err(|_| Error::Rejected)?;
        let operation = cap.operation_id().map_err(|_| Error::Rejected)?;
        let intent_directory = cap
            .original_intent_directory()
            .map_err(|_| Error::Rejected)?;
        require_child(Path::new(&path), &intent_directory)?;
        let original_command = cap
            .original_command()
            .map_err(|_| Error::Rejected)?
            .to_vec();
        let command = SenderCommandV1::decode_canonical_exact(12, operation, &original_command)
            .map_err(|_| Error::Rejected)?;
        let SenderCommandBodyV1::Release {
            inputs_digest,
            envelope_digest,
            inputs,
            envelope,
            terminal_receipt,
            hardware_authorization,
        } = command.body
        else {
            return Err(Error::Rejected);
        };
        let authorization = crate::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationV1::decode_canonical_exact(&hardware_authorization)
            .map_err(|_| Error::Rejected)?;
        let preparation = KagemushaCoreSenderPreparationArchiveV1 {
            version: 1,
            operation_id: operation,
            context: command.context,
            inputs_digest,
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        let mut fields = vec![authorization.outcome_id.to_vec()];
        match (inputs, terminal_receipt) {
            (
                SenderPublicInputsV1::SendSplit { request },
                SenderTerminalReceiptV1::PaymentAcknowledgement(ack),
            ) => {
                fields.extend([0_u32.to_le_bytes().to_vec(), request, envelope.clone()]);
                let mut receipt = 0_u32.to_le_bytes().to_vec();
                receipt.extend_from_slice(&ack);
                fields.push(receipt);
            }
            (
                SenderPublicInputsV1::RedeemSplit {
                    amount,
                    beneficiary,
                },
                SenderTerminalReceiptV1::RedemptionSettlement(receipt),
            ) => {
                use norito::codec::Encode;
                fields.extend([
                    1_u32.to_le_bytes().to_vec(),
                    amount.to_le_bytes().to_vec(),
                    beneficiary.encode(),
                    envelope.clone(),
                ]);
                let mut tagged = 1_u32.to_le_bytes().to_vec();
                tagged.extend(norito::encode_canonical(&receipt).map_err(|_| Error::Rejected)?);
                fields.push(tagged);
            }
            _ => return Err(Error::Rejected),
        }
        let response = vec![
            operation.to_vec(),
            preparation,
            envelope_digest.to_vec(),
            envelope,
            hardware_authorization,
        ];
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let destination = source.outbox_release_recovery_destination(&cap)?;
        require_child(Path::new(&path), &destination.directory)?;
        if destination.checkpoint_operation_id == [0; 32] {
            return Err(Error::Rejected);
        }
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        Ok(Self {
            path,
            stage: Stage::OutboxRelease(Box::new(cap)),
            original_enrollment,
            selected_source,
            release,
            terminal_attempt: None,
            incoming_attempt: None,
            stage_request: None,
            completed_release_locators: std::collections::BTreeMap::new(),
            release_attempt: Some(ReleaseAttempt {
                operation_id: operation,
                original_command,
                intent_directory,
                request: fields,
                response,
                destination,
                completion: None,
            }),
        })
    }
    pub(in super::super) fn prepare_payment_release(
        &mut self,
        fields: &[Vec<u8>],
        signer: &RetainedCoreAuthorizationSignerV1,
    ) -> Result<Vec<Vec<u8>>> {
        if fields.len() == 11 && fields[1] == 1_u32.to_le_bytes() {
            return self.prepare_redemption_release(fields, signer);
        }
        if fields.len() != 10
            || fields[1] != 0_u32.to_le_bytes()
            || fields[4].len() <= 4
            || fields[4][..4] != 0_u32.to_le_bytes()
        {
            return Err(Error::Rejected);
        }
        if let Some(attempt) = &self.release_attempt {
            let response = attempt.retry_response(&fields[..5])?;
            self.recheck_originals()?;
            if !matches!(self.stage, Stage::OutboxRelease(_)) {
                return Err(Error::Rejected);
            }
            return Ok(response);
        }
        let terminal_id: [u8; 32] = fields[0]
            .as_slice()
            .try_into()
            .map_err(|_| Error::Rejected)?;
        let owner = self.selected()?;
        let record = owner
            .outgoing_record_for_terminal(terminal_id)
            .map_err(|_| Error::Rejected)?
            .ok_or(Error::Rejected)?;
        if record.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || record.operation_kind
                != iroha_data_model::kagemusha::KagemushaOperationKindV1::SendSplit
        {
            return Err(Error::Rejected);
        }
        let Some(iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1::SendSplit {
            request,
        }) = &record.inputs
        else {
            return Err(Error::Rejected);
        };
        if request != &fields[2] {
            return Err(Error::Rejected);
        }
        let selection = owner
            .payment_release_selection(record.operation_id, &fields[4][4..])
            .map_err(|_| Error::Rejected)?;
        let envelope = selection
            .canonical_envelope()
            .map_err(|_| Error::Rejected)?;
        if envelope != fields[3] {
            return Err(Error::Rejected);
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let original = source.payment_release_originals(&selection)?;
        require_child(Path::new(&self.path), &original.intent_directory)?;
        self.destination(&original.destination)?;
        let authorization =
            signer.sign_payment_release(&selection, original.hardware_one_use_nonce)?;
        let command = SenderCommandV1 {
            version: 1,
            operation: 12,
            operation_id: record.operation_id,
            context: record.context.clone(),
            body: SenderCommandBodyV1::Release {
                inputs_digest: record.inputs_digest,
                envelope_digest: record.envelope_digest.ok_or(Error::Rejected)?,
                inputs: SenderPublicInputsV1::SendSplit {
                    request: request.clone(),
                },
                envelope: envelope.clone(),
                terminal_receipt: SenderTerminalReceiptV1::PaymentAcknowledgement(
                    fields[4][4..].to_vec(),
                ),
                hardware_authorization: authorization.clone(),
            },
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        let preparation = KagemushaCoreSenderPreparationArchiveV1 {
            version: 1,
            operation_id: record.operation_id,
            context: record.context.clone(),
            inputs_digest: record.inputs_digest,
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        let response = vec![
            record.operation_id.to_vec(),
            preparation,
            record.envelope_digest.ok_or(Error::Rejected)?.to_vec(),
            envelope,
            authorization,
        ];
        source.recheck_originals()?;
        let owner = self.take_selected()?;
        // Intake independently verifies the ACK/Core signature and fsyncs the complete command.
        let cap = owner
            .prepare_payment_release(&original.intent_directory, &command)
            .map_err(|_| Error::Rejected)?;
        self.stage = Stage::OutboxRelease(Box::new(cap));
        self.release_attempt = Some(ReleaseAttempt {
            operation_id: response[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
            original_command: command,
            intent_directory: original.intent_directory,
            request: fields[..5].to_vec(),
            response: response.clone(),
            destination: original.destination,
            completion: None,
        });
        self.recheck_originals()?;
        source.recheck_originals()?;
        Ok(response)
    }

    fn prepare_redemption_release(
        &mut self,
        fields: &[Vec<u8>],
        signer: &RetainedCoreAuthorizationSignerV1,
    ) -> Result<Vec<Vec<u8>>> {
        use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1;
        use norito::codec::Encode;
        if fields.len() != 11
            || fields[1] != 1_u32.to_le_bytes()
            || fields[5].len() <= 4
            || fields[5][..4] != 1_u32.to_le_bytes()
        {
            return Err(Error::Rejected);
        }
        if let Some(attempt) = &self.release_attempt {
            let response = attempt.retry_response(&fields[..6])?;
            self.recheck_originals()?;
            if !matches!(self.stage, Stage::OutboxRelease(_)) {
                return Err(Error::Rejected);
            }
            return Ok(response);
        }
        let terminal_id: [u8; 32] = fields[0]
            .as_slice()
            .try_into()
            .map_err(|_| Error::Rejected)?;
        let owner = self.selected()?;
        let record = owner
            .outgoing_record_for_terminal(terminal_id)
            .map_err(|_| Error::Rejected)?
            .ok_or(Error::Rejected)?;
        if record.phase != KagemushaOutgoingOperationPhaseV1::Installed {
            return Err(Error::Rejected);
        }
        let Some(KagemushaOutgoingPublicInputsV1::RedeemSplit {
            amount,
            beneficiary,
        }) = &record.inputs
        else {
            return Err(Error::Rejected);
        };
        if fields[2] != amount.to_le_bytes() || fields[3] != beneficiary.encode() {
            return Err(Error::Rejected);
        }
        let source = FINALITY_SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let (status, bootstrap) = source.original_finality(owner, record.operation_id)?;
        let mut pins = source.bootstrap_pins(owner)?;
        let now = source.current_trusted_native_time_ms()?;
        if now < pins.trusted_now_ms {
            return Err(Error::Rejected);
        }
        pins.trusted_now_ms = now;
        let selection = owner
            .redemption_finality_selection(record.operation_id, &status, &bootstrap, pins)
            .map_err(|_| Error::Rejected)?;
        let envelope = selection
            .canonical_envelope_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        let receipt = selection
            .terminal_receipt_at_trusted_time(now)
            .map_err(|_| Error::Rejected)?;
        if fields[4] != envelope
            || fields[5][4..] != norito::encode_canonical(&receipt).map_err(|_| Error::Rejected)?
        {
            return Err(Error::Rejected);
        }
        let original = source.release_originals(&selection)?;
        require_child(Path::new(&self.path), &original.intent_directory)?;
        self.destination(&original.destination)?;
        let authorization = signer.sign_redemption_release(
            &selection,
            original.hardware_one_use_nonce,
            source.as_ref(),
        )?;
        let command = SenderCommandV1 {
            version: 1,
            operation: 12,
            operation_id: record.operation_id,
            context: record.context.clone(),
            body: SenderCommandBodyV1::Release {
                inputs_digest: record.inputs_digest,
                envelope_digest: record.envelope_digest.ok_or(Error::Rejected)?,
                inputs: SenderPublicInputsV1::RedeemSplit {
                    amount: *amount,
                    beneficiary: beneficiary.clone(),
                },
                envelope: envelope.clone(),
                terminal_receipt: SenderTerminalReceiptV1::RedemptionSettlement(receipt),
                hardware_authorization: authorization.clone(),
            },
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        let preparation = KagemushaCoreSenderPreparationArchiveV1 {
            version: 1,
            operation_id: record.operation_id,
            context: record.context.clone(),
            inputs_digest: record.inputs_digest,
        }
        .encode_canonical()
        .map_err(|_| Error::Rejected)?;
        let response = vec![
            record.operation_id.to_vec(),
            preparation,
            record.envelope_digest.ok_or(Error::Rejected)?.to_vec(),
            envelope,
            authorization,
        ];
        let after = source.current_trusted_native_time_ms()?;
        selection
            .recheck_at_trusted_time(after)
            .map_err(|_| Error::Rejected)?;
        if after < now {
            return Err(Error::Rejected);
        }
        source.recheck_originals()?;
        pins.trusted_now_ms = after;
        let owner = self.take_selected()?;
        // Full finality originals and exact signed command are verified and fsynced before exposure.
        let cap = owner
            .prepare_redemption_release(
                &original.intent_directory,
                &command,
                &status,
                &bootstrap,
                pins,
            )
            .map_err(|_| Error::Rejected)?;
        self.stage = Stage::OutboxRelease(Box::new(cap));
        self.release_attempt = Some(ReleaseAttempt {
            operation_id: response[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
            original_command: command,
            intent_directory: original.intent_directory,
            request: fields[..6].to_vec(),
            response: response.clone(),
            destination: original.destination,
            completion: None,
        });
        self.recheck_originals()?;
        source.recheck_originals()?;
        Ok(response)
    }

    pub(in super::super) fn complete_payment_release(
        &mut self,
        operation_id: [u8; 32],
        original_command: &[u8],
        full_original_response: &[u8],
    ) -> Result<()> {
        let Some(attempt) = &self.release_attempt else {
            return if self.completed_release_retry(
                operation_id,
                original_command,
                full_original_response,
            )? {
                Ok(())
            } else {
                Err(Error::Rejected)
            };
        };
        // Correlate the original ID and command before setting completion or resuming publication.
        // A response for another operation cannot poison or advance this uncertain attempt.
        attempt.require_completion_tuple(operation_id, original_command, full_original_response)?;
        let destination = attempt.destination.clone();
        if let Stage::OutboxRelease(cap) = &self.stage {
            if cap.operation_id().map_err(|_| Error::Rejected)? != operation_id {
                return Err(Error::Rejected);
            }
            if cap
                .completed_original()
                .map_err(|_| Error::Rejected)?
                .is_none()
                && cap.original_command().map_err(|_| Error::Rejected)? != original_command
            {
                return Err(Error::Rejected);
            }
        } else if !matches!(self.stage, Stage::Publication(_) | Stage::Selected(_)) {
            return Err(Error::Rejected);
        }
        self.release_attempt
            .as_mut()
            .ok_or(Error::Rejected)?
            .completion = Some(full_original_response.to_vec());
        if matches!(self.stage, Stage::Publication(_)) {
            self.resume_publication()?;
        }
        if matches!(self.stage, Stage::Selected(_)) {
            return if self.retire_completed_release_attempt()? == Some(operation_id) {
                Ok(())
            } else {
                Err(Error::Rejected)
            };
        }
        let Stage::OutboxRelease(cap) = std::mem::replace(&mut self.stage, Stage::Frozen) else {
            return Err(Error::Rejected);
        };
        match (*cap).complete_or_retain(
            full_original_response,
            &destination.directory,
            destination.checkpoint_operation_id,
        ) {
            Ok(pending) => {
                self.publish(pending)?;
                if self.retire_completed_release_attempt()? != Some(operation_id) {
                    return Err(Error::Rejected);
                }
                Ok(())
            }
            Err((cap, _)) => {
                self.stage = Stage::OutboxRelease(Box::new(cap));
                Err(Error::Rejected)
            }
        }
    }
}

#[cfg(test)]
#[path = "native_payment_release/tests.rs"]
mod tests;
