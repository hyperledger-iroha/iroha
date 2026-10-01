//! Original incoming work under the same installed concrete Core and qualified physical owner.
use super::*;
use iroha_core_zk::kagemusha_v1_state::{
    CreditIdV1, KagemushaAuthenticatedIncomingFoldV1, KagemushaTransitionKindV1,
    MintInboxReservationV1, MintReservationCertificateV1, MintStageCertificateV1,
    PaymentStageAuthorizationV1,
};
use iroha_data_model::kagemusha::{
    KagemushaCreditOpeningV1, KagemushaDeviceSignatureV1, KagemushaMintCreditV1,
    KagemushaPaymentRequestV1,
};

/// Independently selected originals; no variant represents already verified monetary authority.
pub enum KagemushaNativeIncomingStageOriginalsV1 {
    /// Exact physical reservation before an online mint debit.
    ReserveMint {
        reservation: MintInboxReservationV1,
        certificate: MintReservationCertificateV1,
    },
    /// Original governed mint and qualified staging certificate.
    StageMint {
        credit: KagemushaMintCreditV1,
        certificate: MintStageCertificateV1,
    },
    /// Receiver-private opening and exact original signed peer payment/staging evidence.
    StagePeer {
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        opening: KagemushaCreditOpeningV1,
        trusted_staged_at_ms: u64,
        authorization: PaymentStageAuthorizationV1,
    },
}

/// Native original path, nonce and independently authenticated instant for this exact fold.
pub struct KagemushaNativeIncomingFoldOriginalsV1 {
    /// Private intent path chosen by installed custody, never a frame.
    pub intent_directory: PathBuf,
    /// Fresh original hidden state nonce, never a mobile-selected value.
    pub successor_state_nonce: [u8; 32],
    /// Original native trusted instant, not a caller wall clock.
    pub trusted_time_ms: u64,
    /// Exact original destination fixed before requesting physical history work.
    pub destination: KagemushaNativeCorePublicationDestinationV1,
}

/// Physical original evidence needed by the distinct history root-selection operation.
/// The lifecycle op17 aggregate projection cannot implement this interface. Returned bytes
/// remain untrusted until the concrete native Guard, proof and actual device key verify them.
pub trait KagemushaNativeIncomingEvidenceSourceV1: Send + Sync + 'static {
    /// Recheck independently installed original hardware/key/request custody.
    fn recheck_originals(&self) -> Result<()>;
    /// Read the same actual original hardware result. This must not start another physical
    /// operation on retry or substitute Core signing for the selected device root signature.
    fn original_fold_evidence(
        &self,
        original: &KagemushaAuthenticatedIncomingFoldV1,
        paired_proof: &iroha_data_model::kagemusha::KagemushaPairedProofV1,
        original_request: &[Vec<u8>],
    ) -> Result<(HardwareTransitionCertificateV1, [u8; 64])>;
}
static EVIDENCE: OnceLock<Arc<dyn KagemushaNativeIncomingEvidenceSourceV1>> = OnceLock::new();
/// Rust-only immutable installation; C/JNI cannot register evidence or a verifying authority.
pub fn register_kagemusha_native_incoming_evidence_source_v1(
    source: Arc<dyn KagemushaNativeIncomingEvidenceSourceV1>,
) -> Result<()> {
    source.recheck_originals()?;
    EVIDENCE.set(source).map_err(|_| Error::Rejected)
}

pub(super) struct IncomingAttempt {
    pub(super) request: Vec<Vec<u8>>,
    response: Vec<Vec<u8>>,
    proof: Option<iroha_data_model::kagemusha::KagemushaPairedProofV1>,
    destination: KagemushaNativeCorePublicationDestinationV1,
    pub(super) completion: Option<Vec<Vec<u8>>>,
}

impl NativeCoreWorkOwnerV1 {
    pub(in super::super) fn from_pending_incoming(
        path: String,
        cap: KagemushaAuthenticatedIncomingFoldV1,
    ) -> Result<Self> {
        let selected = cap
            .current_recovery_selection()
            .map_err(|_| Error::Rejected)?;
        let original_enrollment = selected.enrollment_binding().clone();
        let selected_source = super::super::enrolled_open::authenticated_recovery_source(&selected)
            .map_err(|_| Error::Rejected)?;
        let release = cap.authenticated_release().map_err(|_| Error::Rejected)?;
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let destination = source.incoming_recovery_destination(&cap)?;
        require_child(Path::new(&path), &destination.directory)?;
        if destination.checkpoint_operation_id == [0; 32] {
            return Err(Error::Rejected);
        }
        let kind: u32 = match cap.transition().hardware_statement.kind {
            KagemushaTransitionKindV1::MintFold => 0,
            KagemushaTransitionKindV1::ReceiveFold => 1,
            _ => return Err(Error::Rejected),
        };
        let request = vec![kind.to_le_bytes().to_vec(), cap.credit_id().0.to_vec()];
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        Ok(Self {
            path,
            stage: Stage::Incoming(Box::new(cap)),
            original_enrollment,
            selected_source,
            release,
            terminal_attempt: None,
            incoming_attempt: Some(IncomingAttempt {
                request,
                response: Vec::new(),
                proof: None,
                destination,
                completion: None,
            }),
            stage_request: None,
            release_attempt: None,
            completed_release_locators: std::collections::BTreeMap::new(),
        })
    }
    pub(in super::super) fn stage_incoming_original(
        &mut self,
        fields: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        let (kind, credit) = selection(fields, 2)?;
        if let Some(previous) = &self.stage_request {
            if previous == fields {
                self.resume_publication()?;
                self.recheck_originals()?;
                return Ok(vec![credit.to_vec()]);
            }
            if !matches!(self.stage, Stage::Selected(_)) {
                return Err(Error::Rejected);
            }
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let original = source.incoming_stage_originals(self.selected()?, kind, credit)?;
        let destination =
            source.publication_destination(self.selected()?, credit, 10 + kind as u8)?;
        self.destination(&destination)?;
        match &original {
            KagemushaNativeIncomingStageOriginalsV1::ReserveMint { reservation, .. }
                if kind == 0 && reservation.credit_id() == CreditIdV1(credit) =>
            {
                ()
            }
            KagemushaNativeIncomingStageOriginalsV1::StageMint {
                credit: original, ..
            } if kind == 1 && original.statement.lifecycle.credit_id == credit => (),
            KagemushaNativeIncomingStageOriginalsV1::StagePeer {
                payment,
                trusted_staged_at_ms,
                ..
            } if kind == 2 && payment.output.credit_id == credit && *trusted_staged_at_ms != 0 => {
                ()
            }
            _ => return Err(Error::Rejected),
        }
        source.recheck_originals()?;
        self.stage_request = Some(fields.to_vec());
        let owner = self.take_selected()?;
        let pending = match original {
            KagemushaNativeIncomingStageOriginalsV1::ReserveMint {
                reservation,
                certificate,
            } => owner.stage_incoming_mint_reservation(
                &destination.directory,
                destination.checkpoint_operation_id,
                reservation,
                certificate,
            ),
            KagemushaNativeIncomingStageOriginalsV1::StageMint {
                credit,
                certificate,
            } => owner.stage_incoming_mint_credit(
                &destination.directory,
                destination.checkpoint_operation_id,
                credit,
                certificate,
            ),
            KagemushaNativeIncomingStageOriginalsV1::StagePeer {
                request,
                payment,
                opening,
                trusted_staged_at_ms,
                authorization,
            } => owner.stage_incoming_peer_payment(
                &destination.directory,
                destination.checkpoint_operation_id,
                request,
                payment,
                opening,
                trusted_staged_at_ms,
                authorization,
            ),
        }
        .map_err(|_| Error::Rejected)?;
        self.publish(pending)?;
        source.recheck_originals()?;
        self.recheck_originals()?;
        Ok(vec![credit.to_vec()])
    }

    pub(in super::super) fn prepare_incoming(
        &mut self,
        fields: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        let (kind, credit) = selection(fields, 1)?;
        if let Some(attempt) = &self.incoming_attempt {
            if attempt.request != fields || attempt.completion.is_some() {
                return Err(Error::Rejected);
            }
            if let Stage::Incoming(cap) = &self.stage {
                cap.recheck_originals().map_err(|_| Error::Rejected)?;
                if !attempt.response.is_empty() {
                    return Ok(attempt.response.clone());
                }
            } else {
                return Err(Error::Rejected);
            }
        } else {
            let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
            source.recheck_originals()?;
            let originals = source.incoming_fold_originals(self.selected()?, kind, credit)?;
            require_child(Path::new(&self.path), &originals.intent_directory)?;
            self.destination(&originals.destination)?;
            if originals.successor_state_nonce == [0; 32] || originals.trusted_time_ms == 0 {
                return Err(Error::Rejected);
            }
            source.recheck_originals()?;
            let owner = self.take_selected()?;
            let cap = match kind {
                0 => owner.prepare_incoming_mint_fold(
                    &originals.intent_directory,
                    CreditIdV1(credit),
                    originals.successor_state_nonce,
                    originals.trusted_time_ms,
                ),
                1 => owner.prepare_incoming_receive_fold(
                    &originals.intent_directory,
                    CreditIdV1(credit),
                    originals.successor_state_nonce,
                    originals.trusted_time_ms,
                ),
                _ => return Err(Error::Rejected),
            }
            .map_err(|_| Error::Rejected)?;
            self.stage = Stage::Incoming(Box::new(cap));
            self.incoming_attempt = Some(IncomingAttempt {
                request: fields.to_vec(),
                response: Vec::new(),
                proof: None,
                destination: originals.destination,
                completion: None,
            });
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        let Stage::Incoming(cap) = &mut self.stage else {
            return Err(Error::Rejected);
        };
        let proof = match cap.original_proof().map_err(|_| Error::Rejected)? {
            Some(proof) => proof.clone(),
            None => {
                let generated = {
                    let selection = cap.proving_selection().map_err(|_| Error::Rejected)?;
                    let (profile, resolver) = source.incoming_prover_originals(&selection)?;
                    let prover = KagemushaProductionProverV1::load_incoming(
                        &selection,
                        profile,
                        Resolver(resolver),
                    )
                    .map_err(|_| Error::Rejected)?;
                    let claim = prover
                        .prove_incoming_state_hash_claim(&selection)
                        .map_err(|_| Error::Rejected)?;
                    prover
                        .prove_incoming_state(&selection, &claim)
                        .map_err(|_| Error::Rejected)?
                        .proof
                };
                // The exact original pair is synced before any public/device work is exposed.
                cap.retain_original_proof(&generated)
                    .map_err(|_| Error::Rejected)?;
                generated
            }
        };
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        let transition = cap.transition();
        let hardware = norito::encode_canonical(&transition.hardware_statement)
            .map_err(|_| Error::Rejected)?;
        let proof_statement = transition
            .proof_statement
            .digest()
            .map_err(|_| Error::Rejected)?;
        let normalized = transition
            .normalized_guard_statement
            .canonical_digest()
            .map_err(|_| Error::Rejected)?;
        let signing = cap
            .root_selection_signing_bytes()
            .map_err(|_| Error::Rejected)?;
        let archive = norito::encode_canonical(&proof).map_err(|_| Error::Rejected)?;
        let (binding, epoch) = cap.device_binding();
        if hardware.len() > 8192 || signing.len() > 32768 || archive.len() > 8192 {
            return Err(Error::Rejected);
        }
        let response = vec![
            cap.history_operation_id().to_vec(),
            cap.credit_id().0.to_vec(),
            hardware,
            proof_statement.to_vec(),
            normalized.to_vec(),
            signing,
            binding.device_key_reference.to_vec(),
            epoch.generation.to_le_bytes().to_vec(),
            epoch.epoch_id.to_vec(),
            archive,
        ];
        // The normal shared encoder enforces the complete 128KiB bound as well.
        super::super::kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        cap.recheck_originals().map_err(|_| Error::Rejected)?;
        let attempt = self.incoming_attempt.as_mut().ok_or(Error::Rejected)?;
        attempt.proof = Some(proof);
        attempt.response = response.clone();
        Ok(response)
    }

    pub(in super::super) fn complete_incoming(
        &mut self,
        fields: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        if fields.len() != 4 {
            return Err(Error::Rejected);
        }
        let attempt = self.incoming_attempt.as_mut().ok_or(Error::Rejected)?;
        if attempt.response.len() != 10
            || fields[0] != attempt.response[0]
            || fields[1] != attempt.response[9]
        {
            return Err(Error::Rejected);
        }
        if let Some(previous) = &attempt.completion {
            if previous != fields {
                return Err(Error::Rejected);
            }
        } else {
            attempt.completion = Some(fields.to_vec());
        }
        let response = vec![fields[0].clone()];
        if matches!(self.stage, Stage::Publication(_)) {
            self.resume_publication()?;
        }
        if matches!(self.stage, Stage::Selected(_)) {
            self.recheck_originals()?;
            return Ok(response);
        }
        let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
        let evidence = EVIDENCE.get().ok_or(Error::Unavailable)?.clone();
        source.recheck_originals()?;
        evidence.recheck_originals()?;
        let attempt = self.incoming_attempt.as_ref().ok_or(Error::Rejected)?;
        let proof = attempt.proof.as_ref().ok_or(Error::Rejected)?.clone();
        let destination = attempt.destination.clone();
        let Stage::Incoming(cap) = &self.stage else {
            return Err(Error::Rejected);
        };
        cap.recheck_retry_originals().map_err(|_| Error::Rejected)?;
        let (certificate, root_signature) = evidence.original_fold_evidence(cap, &proof, fields)?;
        if norito::encode_canonical(&certificate).map_err(|_| Error::Rejected)? != fields[2]
            || root_signature.as_slice() != fields[3]
        {
            return Err(Error::Rejected);
        }
        evidence.recheck_originals()?;
        source.recheck_originals()?;
        let Stage::Incoming(cap) = std::mem::replace(&mut self.stage, Stage::Frozen) else {
            return Err(Error::Rejected);
        };
        match cap.complete_or_retain(
            &destination.directory,
            destination.checkpoint_operation_id,
            TransitionAuthorizationV1::new(certificate, proof),
            KagemushaDeviceSignatureV1::from_raw_bytes(&root_signature)
                .map_err(|_| Error::Rejected)?,
        ) {
            Ok(pending) => self.publish(pending)?,
            Err((cap, _)) => {
                self.stage = Stage::Incoming(cap);
                return Err(Error::Unavailable);
            }
        }
        evidence.recheck_originals()?;
        source.recheck_originals()?;
        self.recheck_originals()?;
        Ok(response)
    }
}

fn selection(fields: &[Vec<u8>], max_kind: u32) -> Result<(u32, [u8; 32])> {
    if fields.len() != 2 {
        return Err(Error::Rejected);
    }
    let kind = u32::from_le_bytes(
        fields[0]
            .as_slice()
            .try_into()
            .map_err(|_| Error::Rejected)?,
    );
    let credit = fields[1]
        .as_slice()
        .try_into()
        .map_err(|_| Error::Rejected)?;
    if kind > max_kind || credit == [0; 32] {
        return Err(Error::Rejected);
    }
    Ok((kind, credit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_selector_keeps_typed_credit_identity_and_refuses_malformed_fields() {
        for kind in 0_u32..=2 {
            let credit = [kind as u8 + 1; 32];
            let fields = vec![kind.to_le_bytes().to_vec(), credit.to_vec()];
            let selected = selection(&fields, 2).unwrap();
            assert_eq!(selected, (kind, CreditIdV1(credit).0));
            assert_eq!(selected.1.to_vec(), fields[1]);
            if kind == 2 {
                assert!(selection(&fields, 1).is_err());
            }
        }
        for fields in [
            vec![],
            vec![0_u32.to_le_bytes().to_vec()],
            vec![vec![0; 3], vec![1; 32]],
            vec![0_u32.to_le_bytes().to_vec(), vec![1; 31]],
            vec![0_u32.to_le_bytes().to_vec(), vec![0; 32]],
            vec![3_u32.to_le_bytes().to_vec(), vec![1; 32]],
            vec![0_u32.to_le_bytes().to_vec(), vec![1; 32], vec![]],
        ] {
            assert!(selection(&fields, 2).is_err());
        }
    }
}
