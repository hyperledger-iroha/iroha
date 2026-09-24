//! Phase-1 native enrollment adapter over explicitly supplied qualified dependencies.
//!
//! This adapter owns the original process-local selection and journal deadline. It does not
//! verify an issuer challenge, create a credential, or authorize money: phases 2 through 6 stay
//! unavailable until a qualified provider integrates the consuming enrollment kernel. No C/JNI
//! caller can inject the delegate, authenticated journal store, or governed pins.

use std::sync::{Arc, Mutex};

use super::{
    KagemushaCoreCoordinatorBackendErrorV1, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1, archive_boundary,
    enrollment_attempt_journal::{
        KagemushaEnrollmentAttemptJournalV1, KagemushaEnrollmentJournalErrorV1,
        KagemushaEnrollmentJournalPinsV1, KagemushaEnrollmentJournalSelectionV1,
    },
    kagemusha_core_coordinator_decode_request_v1,
    kagemusha_core_coordinator_validate_method_response_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};

#[derive(Default)]
struct PhaseOneOwnerV1 {
    attempted_open: bool,
    handle: Option<u64>,
    attempted_selection: bool,
    selection: Option<KagemushaEnrollmentJournalSelectionV1>,
}

/// Qualified-delegate adapter that supplies exactly one native phase-1 selection.
///
/// The caller must inject a separately qualified coordinator, authenticated monotonic journal
/// store, fixed storage path, and independently governed release/profile/issuer/app pins from
/// trusted Rust provisioning. The generic app ABI cannot construct or install this object.
/// Remaining enrollment phases fail closed; this object alone cannot enroll a wallet.
pub struct KagemushaEnrollmentPhaseOneBackendV1 {
    inner: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
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
            journal,
            pins,
            storage_path: storage_path.into(),
            owner: Mutex::new(PhaseOneOwnerV1::default()),
        })
    }
}

impl KagemushaCoreCoordinatorBackendV1 for KagemushaEnrollmentPhaseOneBackendV1 {
    fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        if owner.attempted_open || storage_path != self.storage_path.as_ref() {
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
        if fields[0].as_slice() != 1_u32.to_le_bytes() {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable);
        }
        if owner.attempted_selection {
            return Err(KagemushaCoreCoordinatorBackendErrorV1::Rejected);
        }
        // Consume the only selection attempt before durable I/O. An uncertain response cannot
        // mint another nonce, lane or deadline in this process.
        owner.attempted_selection = true;
        let (selection, response) = self
            .journal
            .select(request_frame, self.pins)
            .map_err(map_journal_error)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, request_frame, &response)
            .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)?;
        owner.selection = Some(selection);
        Ok(response)
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
