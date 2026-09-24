//! Phase-1 native enrollment adapter over explicitly supplied qualified dependencies.
//!
//! This adapter owns the original process-local selection and journal deadline. It reserves
//! phases 2, 3 and 5 durably before consuming their typed kernel states, reads the exact
//! published proof in phase 4, and revokes the ticket in phase 6. The qualified delegate must
//! authenticate the original raw
//! platform evidence, independent policies and release, and consume the enrollment kernel; the
//! adapter cannot do so from the phase-2 frame alone. The delegate receives the original
//! process-local live selection and its governed pins by Rust type, never through C/JNI fields.
//! No C/JNI caller can inject qualified dependencies or reconstruct a consumed kernel state.

use std::sync::{Arc, Mutex};

use super::{
    AcceptedIssuerChallengeV1, FreshIssuerAdmissionV1, INITIAL_ENROLLMENT_READ_SELECTION_V1,
    KagemushaCoreCoordinatorBackendErrorV1, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1, PreparedIssuerProofV1, archive_boundary,
    enrollment_attempt_journal::{
        KagemushaEnrollmentAttemptJournalV1, KagemushaEnrollmentJournalDispatchV1,
        KagemushaEnrollmentJournalErrorV1, KagemushaEnrollmentJournalPinsV1,
        KagemushaEnrollmentJournalSelectionV1, KagemushaEnrollmentLiveSelectionV1,
    },
    kagemusha_core_coordinator_decode_request_v1, kagemusha_core_coordinator_encode_response_v1,
    kagemusha_core_coordinator_validate_method_response_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};

#[derive(Default)]
struct PhaseOneOwnerV1 {
    attempted_open: bool,
    handle: Option<u64>,
    attempted_selection: bool,
    selection: Option<KagemushaEnrollmentJournalSelectionV1>,
    selection_response: Option<Vec<u8>>,
    accepted: Option<AcceptedIssuerChallengeV1>,
    challenge_id: Option<[u8; 32]>,
    prepared: Option<PreparedIssuerProofV1>,
    fresh_admission: Option<FreshIssuerAdmissionV1>,
    cancelled_ticket: Option<u64>,
}

/// Rust-only challenge and proof handoff to a separately qualified enrollment provider.
///
/// `live_selection` is the original process-local journal ticket. Its `require_live` result
/// supplies the native nonce, account and lane; `pins()` supplies the governed release, profile,
/// issuer and app-policy identities. A provider must use this value with
/// `PendingIssuerEnrollmentV1::begin_selected`, authenticate independently retained raw platform
/// evidence and policy/release objects, and return the resulting consuming challenge state.
/// The adapter retains it only after durable publication. The app frame supplies neither these
/// objects nor authority to replace them.
/// The returned challenge and proof are kernel-owned consuming states, not host-created bytes.
/// An unavailable or uncertain result leaves the relevant journal intent frozen.
pub trait KagemushaQualifiedEnrollmentDelegateV1: Send + Sync + 'static {
    /// Consume the exact retained selection and verify one bounded issuer challenge.
    fn accept_challenge(
        &self,
        handle: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        request_frame: &[u8],
    ) -> Result<AcceptedIssuerChallengeV1, KagemushaCoreCoordinatorBackendErrorV1>;

    /// Consume the original authenticated challenge once for the account and device proofs.
    ///
    /// The provider must call `AcceptedIssuerChallengeV1::prepare_proof` on this value. A
    /// substitute challenge or reconstructed host state cannot produce a valid result.
    fn prepare_proof(
        &self,
        handle: u64,
        live_selection: KagemushaEnrollmentLiveSelectionV1,
        accepted: AcceptedIssuerChallengeV1,
        raw_account_signature: &[u8],
        complete_device_response: &[u8],
    ) -> Result<PreparedIssuerProofV1, KagemushaCoreCoordinatorBackendErrorV1>;
}

/// Qualified-delegate adapter that supplies exactly one native phase-1 selection.
///
/// The caller must inject a separately qualified coordinator, authenticated monotonic journal
/// store, fixed storage path, and independently governed release/profile/issuer/app pins from
/// trusted Rust provisioning. The generic app ABI cannot construct or install this object.
/// The completed admission remains process-local and must be taken once by trusted Rust code.
/// This object alone cannot qualify hardware or authorize money.
pub struct KagemushaEnrollmentPhaseOneBackendV1 {
    inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
    qualified_enrollment: Option<Arc<dyn KagemushaQualifiedEnrollmentDelegateV1>>,
    journal: Arc<KagemushaEnrollmentAttemptJournalV1>,
    pins: KagemushaEnrollmentJournalPinsV1,
    storage_path: Box<str>,
    owner: Mutex<PhaseOneOwnerV1>,
}

impl KagemushaEnrollmentPhaseOneBackendV1 {
    /// Construct one process-lifetime phase-1 adapter without installing it globally.
    ///
    /// The supplied store must already be authenticated, rollback-checked and tied to this
    /// exact storage path. Invalid path or reserved policy pins are rejected before opening
    /// the platform coordinator.
    ///
    /// # Errors
    ///
    /// Returns `Rejected` if the native path or independent policy pins are malformed.
    pub fn new(
        inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
        journal: Arc<KagemushaEnrollmentAttemptJournalV1>,
        pins: KagemushaEnrollmentJournalPinsV1,
        storage_path: &str,
    ) -> Result<Self, KagemushaCoreCoordinatorBackendErrorV1> {
        kagemusha_core_coordinator_validate_storage_path_v1(storage_path.as_bytes())
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        pins.validate()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        Ok(Self {
            inner,
            qualified_enrollment: None,
            journal,
            pins,
            storage_path: storage_path.into(),
            owner: Mutex::new(PhaseOneOwnerV1::default()),
        })
    }

    /// Construct an adapter with a Rust-only qualified challenge and proof provider.
    ///
    /// The generic coordinator backend is not treated as a phase-2 provider. This provider must
    /// retain the original raw platform evidence and independent issuer, app and release objects.
    /// It receives the original live journal selection at dispatch; no app frame can supply one.
    ///
    /// # Errors
    ///
    /// Returns `Rejected` if the native path or independent policy pins are malformed.
    pub fn new_with_qualified_enrollment(
        inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
        qualified_enrollment: Arc<dyn KagemushaQualifiedEnrollmentDelegateV1>,
        journal: Arc<KagemushaEnrollmentAttemptJournalV1>,
        pins: KagemushaEnrollmentJournalPinsV1,
        storage_path: &str,
    ) -> Result<Self, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut backend = Self::new(inner, journal, pins, storage_path)?;
        backend.qualified_enrollment = Some(qualified_enrollment);
        Ok(backend)
    }

    /// Take the one freshly authenticated issuer admission after durable phase-5 publication.
    ///
    /// This Rust-only handoff rechecks the original ticket, governed pins and continuous
    /// deadline. It cannot reconstruct an admission from a certificate, app response or journal
    /// snapshot after restart. The caller must still perform the separate enrolled-wallet open.
    ///
    /// # Errors
    ///
    /// Returns `Rejected` if the handle, selection or one-use admission is absent or stale.
    pub fn take_fresh_admission(
        &self,
        handle: u64,
    ) -> Result<FreshIssuerAdmissionV1, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        let selection = owner
            .selection
            .as_ref()
            .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        self.journal
            .retain_live(selection.clone(), self.pins)
            .map_err(map_journal_error)?;
        let admission = owner
            .fresh_admission
            .take()
            .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        admission
            .require_live()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        Ok(admission)
    }
}

impl KagemushaCoreCoordinatorBackendV1 for KagemushaEnrollmentPhaseOneBackendV1 {
    fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if owner.attempted_open {
            // A second open can represent an account switch. Revoke the old UI handle and
            // original enrollment ticket even though this process cannot open another one.
            if let Some(handle) = owner.handle.take() {
                let _selected = owner.selection.take();
                owner.selection_response = None;
                let _accepted = owner.accepted.take();
                let _prepared = owner.prepared.take();
                let _admission = owner.fresh_admission.take();
                owner.challenge_id = None;
                owner.cancelled_ticket = None;
                let _ = self.journal.revoke_all();
                let _ = self.inner.close(handle);
            }
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        if storage_path != self.storage_path.as_ref() {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        owner.attempted_open = true;
        let handle = self.inner.open(storage_path)?;
        if handle == 0 {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        owner.handle = Some(handle);
        Ok(handle)
    }

    fn invoke(
        &self,
        handle: u64,
        method: KagemushaCoreCoordinatorMethodV1,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        if matches!(
            method,
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment
                | KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest
                | KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof
        ) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner.invoke(handle, method, request_frame)
    }

    fn invoke_initial_enrollment(
        &self,
        handle: u64,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        archive_boundary::validate_request(method, request_frame)
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        let fields = kagemusha_core_coordinator_decode_request_v1(request_frame)
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        match fields[0].as_slice() {
            phase if phase == 1_u32.to_le_bytes() => {
                if owner.attempted_selection {
                    return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
                }
                // Consume the only selection attempt before durable I/O. An uncertain response
                // cannot mint another nonce, lane or deadline in this process.
                owner.attempted_selection = true;
                let (selection, response) = self
                    .journal
                    .select(request_frame, self.pins)
                    .map_err(map_journal_error)?;
                kagemusha_core_coordinator_validate_method_response_v1(
                    method,
                    request_frame,
                    &response,
                )
                .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                owner.selection = Some(selection);
                owner.selection_response = Some(response.clone());
                Ok(response)
            }
            phase if phase == INITIAL_ENROLLMENT_READ_SELECTION_V1.to_le_bytes() => {
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                if fields[1].as_slice() != selection.account_i105.as_bytes() {
                    return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
                }
                self.journal
                    .retain_live(selection.clone(), self.pins)
                    .map_err(map_journal_error)?;
                owner
                    .selection_response
                    .clone()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)
            }
            phase if phase == 2_u32.to_le_bytes() => {
                let qualified = self
                    .qualified_enrollment
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)?;
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                    .clone();
                // Recheck the original process-local ticket, policy pins and native deadline
                // before returning even a previously retained response.
                let live_selection = self
                    .journal
                    .retain_live(selection.clone(), self.pins)
                    .map_err(map_journal_error)?;
                match self
                    .journal
                    .reserve(&selection, request_frame)
                    .map_err(map_journal_error)?
                {
                    KagemushaEnrollmentJournalDispatchV1::Retained(response) => Ok(response),
                    KagemushaEnrollmentJournalDispatchV1::Execute(reservation) => {
                        // The delegate is the explicitly qualified policy/evidence owner. The
                        // persisted intent prevents a second verifier/device action if its
                        // result is lost, rejected, or cannot be published durably.
                        let accepted =
                            qualified.accept_challenge(handle, live_selection, request_frame)?;
                        let challenge_id = accepted
                            .device_request_id()
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let response = kagemusha_core_coordinator_encode_response_v1(&[
                            selection.ticket.to_le_bytes().to_vec(),
                            accepted
                                .account_signing_message()
                                .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                                .to_vec(),
                            challenge_id.to_vec(),
                            accepted
                                .canonical_device_command()
                                .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                                .to_vec(),
                        ])
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        archive_boundary::validate_response(method, request_frame, &response)
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let published = self
                            .journal
                            .publish(reservation, &response)
                            .map_err(map_journal_error)?;
                        // Only the successfully published challenge can authorize phase 3.
                        owner.accepted = Some(accepted);
                        owner.challenge_id = Some(challenge_id);
                        Ok(published)
                    }
                }
            }
            phase if phase == 3_u32.to_le_bytes() => {
                let qualified = self
                    .qualified_enrollment
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)?;
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                    .clone();
                let live_selection = self
                    .journal
                    .retain_live(selection.clone(), self.pins)
                    .map_err(map_journal_error)?;
                match self
                    .journal
                    .reserve(&selection, request_frame)
                    .map_err(map_journal_error)?
                {
                    KagemushaEnrollmentJournalDispatchV1::Retained(response) => Ok(response),
                    KagemushaEnrollmentJournalDispatchV1::Execute(reservation) => {
                        // Taking the one authenticated challenge after durable intent makes
                        // every uncertain result terminal for this proof attempt.
                        let accepted = owner
                            .accepted
                            .take()
                            .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let prepared = qualified.prepare_proof(
                            handle,
                            live_selection,
                            accepted,
                            &fields[2],
                            &fields[3],
                        )?;
                        let challenge_id = prepared
                            .challenge_id()
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        if owner.challenge_id != Some(challenge_id) {
                            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
                        }
                        let response = kagemusha_core_coordinator_encode_response_v1(&[
                            selection.ticket.to_le_bytes().to_vec(),
                            challenge_id.to_vec(),
                            prepared
                                .canonical_proof()
                                .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                                .to_vec(),
                        ])
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        archive_boundary::validate_response(method, request_frame, &response)
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let published = self
                            .journal
                            .publish(reservation, &response)
                            .map_err(map_journal_error)?;
                        owner.prepared = Some(prepared);
                        Ok(published)
                    }
                }
            }
            phase if phase == 4_u32.to_le_bytes() => {
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                self.journal
                    .retain_live(selection.clone(), self.pins)
                    .map_err(map_journal_error)?;
                self.journal
                    .read_proof(selection, request_frame)
                    .map_err(map_journal_error)
            }
            phase if phase == 5_u32.to_le_bytes() => {
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?
                    .clone();
                self.journal
                    .retain_live(selection.clone(), self.pins)
                    .map_err(map_journal_error)?;
                match self
                    .journal
                    .reserve(&selection, request_frame)
                    .map_err(map_journal_error)?
                {
                    KagemushaEnrollmentJournalDispatchV1::Retained(response) => Ok(response),
                    KagemushaEnrollmentJournalDispatchV1::Execute(reservation) => {
                        let prepared = owner
                            .prepared
                            .take()
                            .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        // The issuer certificate is checked only against the exact retained
                        // account/device proof and the original issuer policy/release.
                        let admission = prepared
                            .complete(&fields[2])
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let response = kagemusha_core_coordinator_encode_response_v1(&[
                            selection.ticket.to_le_bytes().to_vec(),
                            admission.enrollment_binding().enrollment_id.to_vec(),
                        ])
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        archive_boundary::validate_response(method, request_frame, &response)
                            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                        let published = self
                            .journal
                            .publish(reservation, &response)
                            .map_err(map_journal_error)?;
                        owner.fresh_admission = Some(admission);
                        Ok(published)
                    }
                }
            }
            phase if phase == 6_u32.to_le_bytes() => {
                let ticket = u64::from_le_bytes(
                    fields[1]
                        .as_slice()
                        .try_into()
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?,
                );
                if owner.cancelled_ticket == Some(ticket) {
                    return kagemusha_core_coordinator_encode_response_v1(&[])
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected);
                }
                let selection = owner
                    .selection
                    .as_ref()
                    .ok_or(KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
                if selection.ticket != ticket {
                    return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
                }
                let selection = selection.clone();
                let _selected = owner.selection.take();
                owner.selection_response = None;
                let _accepted = owner.accepted.take();
                let _prepared = owner.prepared.take();
                let _admission = owner.fresh_admission.take();
                owner.challenge_id = None;
                self.journal
                    .cancel(&selection, request_frame)
                    .map_err(map_journal_error)?;
                owner.cancelled_ticket = Some(ticket);
                kagemusha_core_coordinator_encode_response_v1(&[])
                    .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
            }
            _ => Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable),
        }
    }

    fn acknowledge_committed_app_attest(
        &self,
        handle: u64,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        let owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner
            .acknowledge_committed_app_attest(handle, request_frame)
    }

    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        operation_id: [u8; 32],
    ) -> Result<
        iroha_core::zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1,
        KagemushaCoreCoordinatorBackendErrorV1,
    > {
        let owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        self.inner.export_outgoing_state_proof(handle, operation_id)
    }

    fn close(&self, handle: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if handle == 0 || owner.handle != Some(handle) {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        owner.handle = None;
        let _selected = owner.selection.take();
        owner.selection_response = None;
        let _accepted = owner.accepted.take();
        let _prepared = owner.prepared.take();
        let _admission = owner.fresh_admission.take();
        owner.challenge_id = None;
        owner.cancelled_ticket = None;
        let journal_result = self.journal.revoke_all();
        let close_result = self.inner.close(handle);
        journal_result.map_err(map_journal_error)?;
        close_result
    }
}

fn map_journal_error(
    error: KagemushaEnrollmentJournalErrorV1,
) -> KagemushaCoreCoordinatorBackendErrorV1 {
    match error {
        KagemushaEnrollmentJournalErrorV1::Unavailable => {
            KagemushaCoreCoordinatorBackendErrorV1::Unavailable
        }
        _ => KagemushaCoreCoordinatorBackendErrorV1::Rejected,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "enrollment_phase_one_backend/phase_two_tests.rs"]
mod phase_two_tests;
