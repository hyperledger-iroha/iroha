//! One reviewed hardware Advance, followed by independently verified exact completion.
use super::prepare::{model, original, put_original};
use super::worker::Phase;
use super::*;
use crate::kagemusha_wallet_state_v1::custody::SuffixTransitionOwnerV1;
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
};
use std::ops::ControlFlow;

const FROZEN_BOUND: usize = SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
    + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
    + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
    + 1024;
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    /// Consume one real Native review after hardware-backed user approval. Begin must already
    /// have direct ledger finality. The same frozen capsule is the only permitted Advance.
    pub fn execute_reviewed_suffix_unload(
        &mut self,
        review: ReviewedSuffixUnloadV1,
    ) -> Result<SuffixUnloadExecutionV1, ReviewedSuffixFailureV1> {
        // These are the only two pre-admission refusals that return the same capability.
        let wait = if matches!(self.status()?, SlotStatus::Pending(_)) {
            Some(ReviewMaintenanceWaitV1::SelectedPending)
        } else if self.scheduler.payment_pending() {
            Some(ReviewMaintenanceWaitV1::Busy)
        } else {
            None
        };
        if let Some(wait) = wait {
            return Err(ReviewedSuffixFailureV1 {
                error: match wait {
                    ReviewMaintenanceWaitV1::SelectedPending => Error::Pending,
                    _ => Error::ArchiveAdmission(ArchiveAdmissionErrorV1::Busy),
                },
                retained: Some(review),
                wait: Some(wait),
            });
        }
        self.require_fresh_work()?;
        let _payment = self.scheduler.payment();
        self.require_runtime_capacity(NativeAdmissionPhaseV1::BeforeCommit)?;
        self.sync_manifest()?;
        let selected = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        let record = self
            .suffix_request(&selected, &review.request.request_id)?
            .ok_or(Error::WitnessLost("suffix reviewed request"))?;
        if selected.suffix.as_ref().and_then(|s| s.active_request) != Some(record.request_id)
            || record.selection != Some(review.selection)
            || record.source_checkpoint != review.source_checkpoint
            || record.source_selected_generation != review.source_selected_generation
            || record.released.is_some()
        {
            return Err(Error::OperationConflict.into());
        }
        let source = self.require_source(&selected, &record)?;
        if selected.manifest.capsule != record.source_capsule
            || selected.manifest.indexed != Some(record.source_sequence)
        {
            return Err(Error::OperationConflict.into());
        }
        let selection: SuffixPublicationSelectionV1 =
            read_record(&mut self.archive, review.selection)?;
        selection.require((self.scheme_id, self.wallet_id))?;
        let body = review.frozen.capsule.body();
        let header = model(|| {
            KagemushaWalletSuffixHeaderV1::decode_canonical(&body.authority_header, &self.scheme_id)
        })?;
        if original(&mut self.archive, &selection.frozen_capsule, FROZEN_BOUND)?
            != review.frozen.to_canonical_bytes()?
            || selection.authority != review.projection.authority_digest
            || selection.authority != body.authority_digest
            || selection.operation_id != body.operation_id
            || selection.source_capsule != source.capsule_digest()
            || review.tip.point().0 != &source.capsule().body().successor_state
            || header.tip_adjusted != *review.tip.point().1
            || header.tip != model(|| source.capsule().body().successor_state.commitment())?
        {
            return Err(Error::OperationConflict.into());
        }
        // A passed HTTP call, transaction hash or host verdict cannot satisfy this gate.
        if !self.suffix_begin_accepted(&record.request_id, &selection.authority)? {
            return Err(Error::Invalid("suffix Begin finality required before Advance").into());
        }
        let next: preparation_custody::SourceCustodyV1 = archive::decode(&original(
            &mut self.archive,
            &selection.successor_custody,
            preparation_custody::SOURCE_CUSTODY_MAX_BYTES,
        )?)?;
        next.require(&mut self.archive, &body.successor_state)?;
        self.retain_source_custody(review.frozen.capsule.custody_digest(), &next)?;
        self.archive.put(
            ArchiveKey::Capsule(review.frozen.capsule.custody_digest()),
            &review.frozen.to_canonical_bytes()?,
        )?;
        let owner = SuffixTransitionOwnerV1::new(
            review.frozen.credential,
            archive::decode(&review.frozen.certificates)?,
        );
        let request = AdvanceRequest {
            expected: ExpectedHead::Released {
                sequence: record.source_sequence,
                head: body.statement.predecessor,
                capsule_digest: record.source_capsule,
            },
            operation_id: body.operation_id,
            new_head: body.statement.successor,
            proof_digest: body.authority_digest,
            capsule: review.frozen.capsule.clone(),
            growth_bytes: 0,
        };
        match self
            .custody
            .advance_suffix(&owner, &request)
            .map_err(Error::from)?
        {
            AdvanceOutcome::Released { .. } => {
                self.sync_manifest()?;
                // An authenticated selected seal is constructed only by checked_step.
                let released = self.checked_step(review.frozen.capsule.custody_digest())?;
                if released.operation_id() != body.operation_id {
                    return Err(Error::WitnessLost("suffix Advance completion identity").into());
                }
                self.proofs.suffix_pass = Some(LiveSuffixPass {
                    scheduler: self.scheduler.clone(),
                    binding: WorkBinding {
                        request_id: record.request_id,
                        source_capsule: record.source_capsule,
                        source_sequence: record.source_sequence,
                        source_selected_generation: record.source_selected_generation,
                        anchor_sequence: record.anchor_sequence,
                        turn: 0,
                    },
                    cursor: None,
                    tip: Some(review.tip),
                    inflight: None,
                });
                // Release verification executes outside the owner on the next bounded resume.
                Ok(SuffixUnloadExecutionV1::Pending)
            }
            AdvanceOutcome::Pending { .. } => Ok(SuffixUnloadExecutionV1::Pending),
            AdvanceOutcome::NotPerformed(NotPerformed::StaleHead) => {
                Err(Error::OperationConflict.into())
            }
            AdvanceOutcome::NotPerformed(NotPerformed::CapacityWait) => {
                Err(Error::NotEnoughStorage.into())
            }
            AdvanceOutcome::NotPerformed(NotPerformed::Invalid { field }) => {
                Err(Error::Invalid(field).into())
            }
            AdvanceOutcome::Archived(_) | AdvanceOutcome::DeliveryDataLoss { .. } => {
                Err(Error::WitnessLost("suffix selected completion unavailable").into())
            }
        }
    }
    pub(super) fn suffix_resume_selected(
        &mut self,
        _selected: SelectedCheckpoint,
        record: SuffixRequestRecordV1,
    ) -> Result<ControlFlow<SuffixUnloadDecisionV1, SuffixPreparationJobV1>, Error> {
        let key = record
            .selection
            .ok_or(Error::WitnessLost("suffix pending selection"))?;
        let selection: SuffixPublicationSelectionV1 = read_record(&mut self.archive, key)?;
        selection.require((self.scheme_id, self.wallet_id))?;
        let frozen = SuffixFrozenTransitionV1::decode_canonical(
            &original(&mut self.archive, &selection.frozen_capsule, FROZEN_BOUND)?,
            &self.scheme_id,
        )?;
        let SlotStatus::Pending(marker) = self.status()? else {
            return Err(Error::Pending);
        };
        if marker.head()
            != Some((
                frozen.capsule.body().statement.sequence,
                selection.operation_id,
                frozen.capsule.custody_digest(),
            ))
        {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Waiting(
                ReviewMaintenanceWaitV1::SelectedPending,
            )));
        }
        let _payment = self.scheduler.payment();
        match self.custody.recover_selected_suffix(&frozen)? {
            AdvanceOutcome::Pending { .. } => {
                return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Pending));
            }
            AdvanceOutcome::Released { .. } => {
                self.sync_manifest()?;
            }
            AdvanceOutcome::NotPerformed(NotPerformed::StaleHead) => {
                return Err(Error::OperationConflict);
            }
            AdvanceOutcome::NotPerformed(NotPerformed::CapacityWait) => {
                return Err(Error::NotEnoughStorage);
            }
            AdvanceOutcome::NotPerformed(NotPerformed::Invalid { field }) => {
                return Err(Error::Invalid(field));
            }
            AdvanceOutcome::Archived(_) | AdvanceOutcome::DeliveryDataLoss { .. } => {
                return Err(Error::WitnessLost(
                    "suffix recovered completion unavailable",
                ));
            }
        }
        drop(_payment);
        let selected = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        let request = archive::decode(&original(
            &mut self.archive,
            &record.request_original,
            65536,
        )?)?;
        self.suffix_turn(selected, record, request)
    }
    pub(super) fn suffix_release_phase(
        &mut self,
        selected: &SelectedCheckpoint,
        record: &SuffixRequestRecordV1,
        tip: SuffixTipV1,
    ) -> Result<Phase, Error> {
        let key = record
            .selection
            .ok_or(Error::WitnessLost("suffix release selection"))?;
        let selection: SuffixPublicationSelectionV1 = read_record(&mut self.archive, key)?;
        let frozen = SuffixFrozenTransitionV1::decode_canonical(
            &original(&mut self.archive, &selection.frozen_capsule, FROZEN_BOUND)?,
            &self.scheme_id,
        )?;
        let step =
            self.indexed_step(&selected.manifest, frozen.capsule.body().statement.sequence)?;
        let (actual, retained) = step
            .suffix()
            .ok_or(Error::WitnessLost("suffix release schema"))?;
        if actual != &frozen
            || selection.operation_id != step.operation_id()
            || tip.point().0.core.sequence != record.source_sequence
            || tip.steps().0 as u128 != selection.step_count
        {
            return Err(Error::WitnessLost("suffix release exact selected original"));
        }
        Ok(Phase::Release {
            tip,
            frozen,
            completion: retained.record.clone(),
        })
    }
    pub(super) fn suffix_released_projection(
        &mut self,
        record: &SuffixRequestRecordV1,
    ) -> Result<SuffixUnloadReleasedV1, Error> {
        let selection_key = record
            .selection
            .ok_or(Error::WitnessLost("suffix released selection"))?;
        let selection: SuffixPublicationSelectionV1 =
            read_record(&mut self.archive, selection_key)?;
        let released: SuffixReleasedOriginalV1 = read_record(
            &mut self.archive,
            record
                .released
                .ok_or(Error::WitnessLost("suffix released record"))?,
        )?;
        released.require((self.scheme_id, self.wallet_id))?;
        selection.require((self.scheme_id, self.wallet_id))?;
        if released.request_id != record.request_id
            || released.selection != selection_key
            || released.authority != selection.authority
            || released.operation_id != selection.operation_id
        {
            return Err(Error::WitnessLost("suffix released binding"));
        }
        let header_original = original(
            &mut self.archive,
            &selection.header_original,
            KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1,
        )?;
        let header = model(|| {
            KagemushaWalletSuffixHeaderV1::decode_canonical(&header_original, &self.scheme_id)
        })?;
        let result = SuffixUnloadReleasedV1 {
            format_version: 1,
            discriminator: 16,
            request_id: record.request_id,
            authority_digest: selection.authority,
            amount: model(|| header.payout_terms())?.amount,
            step_count: selection.step_count,
            chunk_count: selection.chunk_count,
            instruction_count: selection.chunk_count + 2,
        };
        model(|| result.validate())?;
        Ok(result)
    }
    pub(super) fn suffix_publish_release(
        &mut self,
        selected: &mut SelectedCheckpoint,
        record: &mut SuffixRequestRecordV1,
        operation: [u8; 32],
    ) -> Result<SuffixUnloadReleasedV1, Error> {
        let selection_key = record
            .selection
            .ok_or(Error::WitnessLost("suffix release selection"))?;
        let selection: SuffixPublicationSelectionV1 =
            read_record(&mut self.archive, selection_key)?;
        selection.require((self.scheme_id, self.wallet_id))?;
        if operation != selection.operation_id {
            return Err(Error::OperationConflict);
        }
        let frozen = SuffixFrozenTransitionV1::decode_canonical(
            &original(&mut self.archive, &selection.frozen_capsule, FROZEN_BOUND)?,
            &self.scheme_id,
        )?;
        let step =
            self.indexed_step(&selected.manifest, frozen.capsule.body().statement.sequence)?;
        let (actual, retained) = step
            .suffix()
            .ok_or(Error::WitnessLost("suffix released schema"))?;
        if actual != &frozen || step.operation_id() != operation {
            return Err(Error::WitnessLost("suffix released selected identity"));
        }
        if record.released.is_some() {
            return self.suffix_released_projection(record);
        }
        let body = frozen.capsule.body();
        let header = model(|| {
            KagemushaWalletSuffixHeaderV1::decode_canonical(&body.authority_header, &self.scheme_id)
        })?;
        let completion_original = put_original(
            &mut self.archive,
            step.completion_frame(),
            SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1,
        )?;
        let receipt_original =
            put_original(&mut self.archive, &archive::encode(step.receipt())?, 4096)?;
        let finalize_original = put_original(
            &mut self.archive,
            step.output(),
            KAGEMUSHA_WALLET_SUFFIX_UNLOAD_MAX_BYTES_V1,
        )?;
        let finalize = KagemushaWalletLedgerV1 {
            scheme: self.scheme_id,
            action: KagemushaWalletLedgerActionV1::SuffixUnloadFinalize {
                wallet: self.wallet_id,
                authority: selection.authority,
                account: header.account,
                ordinal: selection.chunk_count + 1,
                unload: step.output().to_vec(),
            },
        };
        let final_instruction = put_original(
            &mut self.archive,
            &archive::encode(&finalize)?,
            super::super::suffix_ledger::SUFFIX_INSTRUCTION_MAX_BYTES_V1,
        )?;
        let instructions = IndexRoot::default()
            .set(
                &mut self.archive,
                manifest::sequence_key(0),
                &selection.begin_instruction_original,
            )?
            .set(
                &mut self.archive,
                manifest::sequence_key(selection.chunk_count + 1),
                &final_instruction,
            )?;
        // Appends are deterministically materialized and retained one ordinal at a time by
        // the instruction owner; no unbounded suffix is copied while holding this owner.
        let released = SuffixReleasedOriginalV1 {
            version: 1,
            discriminator: 6,
            scheme_id: self.scheme_id,
            wallet_id: self.wallet_id,
            request_id: record.request_id,
            authority: selection.authority,
            selection: selection_key,
            operation_id: operation,
            selected_generation: step.selected_generation(),
            capsule_original: selection.frozen_capsule,
            capsule_custody_digest: step.capsule_digest(),
            completion_original,
            completion_digest: step.completion_digest(),
            receipt_original,
            finalize_original,
            instruction_originals: instructions,
            instruction_count: selection.chunk_count + 2,
        };
        model(|| retained.record.verify(&frozen.credential, &frozen.capsule))?;
        released.require((self.scheme_id, self.wallet_id))?;
        record.released = Some(write_record(&mut self.archive, &released)?);
        let request_key = write_record(&mut self.archive, record)?;
        let mut namespace = selected
            .suffix
            .clone()
            .ok_or(Error::WitnessLost("suffix released namespace"))?;
        namespace.requests =
            namespace
                .requests
                .set(&mut self.archive, record.request_id, &request_key)?;
        namespace.authorities = namespace.authorities.set(
            &mut self.archive,
            selection.authority,
            &record.request_id,
        )?;
        namespace.operations =
            namespace
                .operations
                .set(&mut self.archive, operation, &record.request_id)?;
        if namespace.active_request == Some(record.request_id) {
            namespace.active_request = None;
        }
        selected.digest =
            self.publish_suffix_checkpoint(selected.digest, &selected.manifest, &namespace)?;
        selected.suffix = Some(namespace);
        self.suffix_released_projection(record)
    }
}
