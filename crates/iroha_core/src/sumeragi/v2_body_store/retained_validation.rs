//! Exact marker writes joined to the original retained candidate.

use super::*;
use crate::sumeragi::v2_apply::validation_custody::{
    CarrierMarkerPreparation, CarrierValidator, RetainedBodyValidationService,
};

impl V2BodyStore {
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
        let envelope = self.load_validation_envelope(&durable, expected_manifest_hash)?;
        let key = (durable.round(), durable.subject());
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
