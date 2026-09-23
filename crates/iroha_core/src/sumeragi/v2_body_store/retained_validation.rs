//! Exact marker writes joined to the original retained candidate.

use super::*;
use crate::sumeragi::v2_apply::validation_custody::{
    CarrierMarkerPreparation, CarrierValidator, RetainedBodyValidationService,
};

impl V2BodyStore {
    /// Reproduce quarantined marker outcomes using actual retained candidate custody.
    /// Every round of one exact body shares the same original executed owner.
    pub(crate) fn revalidate_retained_markers<P: CarrierValidator>(
        &mut self,
        service: &mut RetainedBodyValidationService<P>,
    ) -> Result<(), V2BodyStoreError> {
        let receipts: Vec<_> = self
            .pending_revalidation
            .values()
            .map(|row| row.durable.clone())
            .collect();
        for durable in receipts {
            let result = self.execute_retained_durable_validation(
                durable.clone(),
                durable.manifest_hash(),
                service,
            )?;
            if result.validated_receipt().is_none() && result.rejection_identity().is_none() {
                return Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch);
            }
        }
        self.ensure_recovered_markers_revalidated()
    }

    /// Recheck the exact successful live marker and durable frame before selecting its owner.
    pub(crate) fn verify_validated_receipt(
        &self,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<(), V2BodyStoreError> {
        let durable = receipt.durable();
        if self.validated.get(&(durable.round(), durable.subject())) != Some(receipt) {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        self.load_validation_envelope(durable, durable.manifest_hash())
            .map(|_| ())
    }

    /// Retain the exact cold terminal success for recovered Apply only.
    /// The lifecycle ledger and semantic replay already selected this inert
    /// terminal result. Keeping it separate from `validated` prevents the
    /// worker handoff from accidentally restoring any Vote authority.
    pub(in crate::sumeragi) fn authorize_recovered_terminal_apply(
        &mut self,
        terminal: &super::super::v2_lifecycle_coordinator::ResolvedLifecycleValidateOutcomeV1,
    ) -> Result<(), V2BodyStoreError> {
        let receipt = terminal
            .validated_receipt()
            .ok_or(V2BodyStoreError::ReceiptMismatch)?;
        let durable = receipt.durable();
        let key = (durable.round(), durable.subject());
        if self.recovered_terminal_apply_receipt.is_some()
            || self.validated.contains_key(&key)
            || terminal.key() != key
            || self.entries.get(&key) != Some(durable)
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        self.load_validation_envelope(durable, durable.manifest_hash())?;
        self.recovered_terminal_apply_receipt = Some(receipt.clone());
        Ok(())
    }

    /// Recovered Apply, including ordinary Decision Apply, may consume the one
    /// selected terminal result without exposing it to voting.
    pub(crate) fn verify_recovered_apply_validated_receipt(
        &self,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<(), V2BodyStoreError> {
        if self.recovered_terminal_apply_receipt.as_ref() == Some(receipt) {
            let durable = receipt.durable();
            return self
                .load_validation_envelope(durable, durable.manifest_hash())
                .map(|_| ());
        }
        self.verify_validated_receipt(receipt)
    }

    /// Plan the exact retained-service descriptor allocations before construction.
    /// Candidate execution and nested journals require separate admission.
    pub(crate) fn retained_validation_descriptor_bytes<P: CarrierValidator>(
        &self,
    ) -> Result<usize, V2BodyStoreError> {
        RetainedBodyValidationService::<P>::descriptor_bytes(self.capacity.max_body_entries)
            .map_err(super::super::v2_apply::validation_custody::CarrierCustodyError::from)
            .map_err(V2BodyStoreError::from)
    }

    /// Reserve bounded service descriptors from this exact open height store.
    /// This is not admission for the payload retained by the associated owner.
    pub(crate) fn retained_validation_service<P: CarrierValidator>(
        &self,
        validator: P,
        budget: &mv::allocation::AllocationBudget,
    ) -> Result<RetainedBodyValidationService<P>, V2BodyStoreError> {
        Ok(RetainedBodyValidationService::new(
            validator,
            self.instance_identity(),
            self.capacity.max_body_entries,
            budget,
        )?)
    }

    /// Validate with complete custody installed before the success marker write.
    /// Cache and reproposal paths require the same executed owner. A local write
    /// refusal leaves its pending receipt and every older confirmed receipt.
    /// TODO: replace live scalar validation only with a real reserved publisher.
    pub(crate) fn execute_retained_durable_validation<P: CarrierValidator>(
        &mut self,
        durable: DurableBodyReceipt,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
        service: &mut RetainedBodyValidationService<P>,
    ) -> Result<DurableBodyValidationOutcome, V2BodyStoreError> {
        if !service.matches_store(&self.instance_identity()) {
            return Err(
                super::super::v2_apply::validation_custody::CarrierCustodyError::Identity.into(),
            );
        }
        let key = (durable.round(), durable.subject());
        // A retained rejection needs no candidate descriptor. Otherwise refuse
        // local capacity before frame I/O and its decoded allocations; this
        // read-only projection cannot authorize any validation marker.
        if !self.rejected.contains_key(&key) {
            service.preflight_marker(&durable)?;
        }
        let envelope = self.load_validation_envelope(&durable, expected_manifest_hash)?;
        if let Some(rejected) = self.rejected.get(&key) {
            if rejected.durable != durable {
                return Err(V2BodyStoreError::ReceiptMismatch);
            }
            return Ok(rejected.sealed_outcome());
        }
        let already_validated = self.validated.get(&key).cloned();
        if already_validated
            .as_ref()
            .is_some_and(|receipt| receipt.durable() != &durable)
        {
            return Err(V2BodyStoreError::ReceiptMismatch);
        }
        let reused = self.reusable_validated_commitment_for_exact_body(&durable, &envelope)?;
        let block = decode_framed_signed_block(&envelope.canonical_wire)
            .map_err(|error| V2BodyStoreError::BlockDecode(error.to_string()))?;
        match service.prepare_marker(
            &self.context,
            &block,
            &durable,
            already_validated.is_some() || reused.is_some(),
        )? {
            CarrierMarkerPreparation::Ready(commitment) => {
                if already_validated
                    .as_ref()
                    .is_some_and(|receipt| receipt.execution_commitment() != commitment)
                    || reused.is_some_and(|expected| expected != commitment)
                {
                    return Err(V2BodyStoreError::ConflictingValidationCommitment);
                }
                let validated = self.persist_validated_receipt(&durable, commitment)?;
                service.confirm(&validated)?;
                Ok(DurableBodyValidationOutcome(
                    DurableBodyValidationOutcomeBody::Validated(validated),
                ))
            }
            CarrierMarkerPreparation::Deferred(refusal) => {
                Err(V2BodyStoreError::LocalValidation(refusal))
            }
            CarrierMarkerPreparation::ValidationError(error) => {
                if let Some(refusal) = error.local_refusal() {
                    return Err(V2BodyStoreError::LocalValidation(refusal));
                }
                if let Some(reference) = error.missing_certified_merge_sidecar() {
                    return Ok(DurableBodyValidationOutcome(
                        DurableBodyValidationOutcomeBody::DeferredMergeSidecar {
                            durable,
                            reference: reference.clone(),
                        },
                    ));
                }
                let rejected = self.persist_rejected_outcome(
                    &durable,
                    error
                        .rejection_identity()
                        .ok_or_else(|| {
                            V2BodyStoreError::LocalValidation(
                                LocalValidationRefusal::RecoveryRequired(error.to_string()),
                            )
                        })?
                        .canonical_code(),
                    error.to_string(),
                )?;
                Ok(rejected.sealed_outcome())
            }
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FAIL_MARKER_FILE_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_MARKER_DIRECTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Inject only the next validation marker file sync on the calling test thread.
#[cfg(test)]
pub(crate) fn fail_next_marker_file_sync() {
    FAIL_MARKER_FILE_SYNC.set(true);
}

#[cfg(test)]
pub(crate) fn fail_next_marker_directory_sync() {
    FAIL_MARKER_DIRECTORY_SYNC.set(true);
}

#[cfg(test)]
pub(super) fn marker_directory_sync_fault(kind: FramePayloadKind) -> std::io::Result<()> {
    if matches!(kind, FramePayloadKind::ValidationOutcomeMarker)
        && FAIL_MARKER_DIRECTORY_SYNC.replace(false)
    {
        return Err(std::io::Error::other(
            "injected validation marker directory-sync refusal",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn marker_file_sync_fault(kind: FramePayloadKind) -> std::io::Result<()> {
    if matches!(kind, FramePayloadKind::ValidationOutcomeMarker)
        && FAIL_MARKER_FILE_SYNC.replace(false)
    {
        return Err(std::io::Error::other(
            "injected validation marker file-sync refusal",
        ));
    }
    Ok(())
}
