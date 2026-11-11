//! Short source-selection/publication turns. Cryptographic work is returned to the caller.
use super::worker::{Phase, Verified};
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    kagemusha_wallet_archive_object_digest_v1 as object_digest,
    kagemusha_wallet_provider_digest_v1 as provider_digest,
};
use crate::kagemusha_wallet_intake_v1 as intake;
use crate::kagemusha_wallet_state_v1::preparation_custody::SourceCustodyV1;
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
};
use std::ops::ControlFlow;

// The observer must surround the actual decoder, including nested model validation.
// Preserve ordinary valid() classifications while keeping enclosing resource refusals retryable.
pub(super) fn model<T>(
    read: impl FnOnce() -> Result<T, KagemushaWalletValidationErrorV1>,
) -> Result<T, Error> {
    match norito::core::classify_decode_attempt(|| match read() {
        Err(KagemushaWalletValidationErrorV1::Codec(error)) => Err(error),
        result => Ok(result),
    }) {
        Ok(result) => valid(result),
        Err(error) => Err(decode_attempt_error(error, Error::Invalid("G1 validation"))),
    }
}

const ORIGINAL_BOUND: usize = 64 * 1024;
pub(super) fn address(bytes: &[u8]) -> Result<[u8; 32], Error> {
    let value = bytes
        .try_into()
        .map_err(|_| Error::WitnessLost("suffix object address"))?;
    if value == [0; 32] {
        return Err(Error::WitnessLost("suffix object zero address"));
    }
    Ok(value)
}
pub(super) fn original(
    store: &mut impl ObjectStore,
    key: &[u8; 32],
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let bytes = store.read_object(key, maximum)?;
    if bytes.is_empty() || bytes.len() > maximum || object_digest(&bytes) != *key {
        return Err(Error::WitnessLost("suffix object identity"));
    }
    Ok(bytes)
}
pub(super) fn put_original(
    store: &mut impl ObjectStore,
    bytes: &[u8],
    maximum: usize,
) -> Result<[u8; 32], Error> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error::Invalid("suffix original bound"));
    }
    let key = store.write_object(bytes, maximum)?;
    if key != object_digest(bytes) {
        return Err(Error::WitnessLost("suffix original publication"));
    }
    Ok(key)
}
fn pin_key(role: u8, sequence: u128) -> [u8; 32] {
    let mut key = manifest::sequence_key(sequence);
    key[0] = role;
    key
}
fn empty_namespace(scheme_id: [u8; 32], wallet_id: [u8; 32]) -> SuffixArchiveManifestV1 {
    SuffixArchiveManifestV1 {
        version: 1,
        discriminator: 2,
        scheme_id,
        wallet_id,
        requests: IndexRoot::default(),
        authorities: IndexRoot::default(),
        operations: IndexRoot::default(),
        source_pins: IndexRoot::default(),
        ledger_attempts: IndexRoot::default(),
        ledger_outcomes: IndexRoot::default(),
        ledger_stages: IndexRoot::default(),
        active_request: None,
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    pub(super) fn save_request(
        &mut self,
        selected: &mut SelectedCheckpoint,
        record: &SuffixRequestRecordV1,
    ) -> Result<(), Error> {
        record.require((self.scheme_id, self.wallet_id))?;
        let key = write_record(&mut self.archive, record)?;
        let mut namespace = selected
            .suffix
            .clone()
            .unwrap_or_else(|| empty_namespace(self.scheme_id, self.wallet_id));
        namespace.requests = namespace
            .requests
            .set(&mut self.archive, record.request_id, &key)?;
        namespace.active_request = Some(record.request_id);
        selected.digest =
            self.publish_suffix_checkpoint(selected.digest, &selected.manifest, &namespace)?;
        selected.suffix = Some(namespace);
        Ok(())
    }
    pub(super) fn require_source(
        &mut self,
        selected: &SelectedCheckpoint,
        record: &SuffixRequestRecordV1,
    ) -> Result<ReleasedStep, Error> {
        if selected.manifest.indexed != Some(record.source_sequence)
            || selected.manifest.capsule != record.source_capsule
        {
            let selection_key = record.selection.ok_or(Error::OperationConflict)?;
            let selection: SuffixPublicationSelectionV1 =
                read_record(&mut self.archive, selection_key)?;
            selection.require((self.scheme_id, self.wallet_id))?;
            if selection.request_id != record.request_id
                || selection.source_capsule != record.source_capsule
                || selection.source_operation != record.source_operation
                || selection.source_sequence != record.source_sequence
                || selection.source_checkpoint != record.source_checkpoint
            {
                return Err(Error::WitnessLost("suffix source selection binding"));
            }
            let next = self.step_entry(
                &selected.manifest,
                record
                    .source_sequence
                    .checked_add(1)
                    .ok_or(Error::OperationConflict)?,
            )?;
            let frozen = SuffixFrozenTransitionV1::decode_canonical(
                &original(
                    &mut self.archive,
                    &selection.frozen_capsule,
                    SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                        + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                        + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                        + 1024,
                )?,
                &self.scheme_id,
            )?;
            if next.operation != selection.operation_id
                || next.capsule != frozen.capsule.custody_digest()
            {
                return Err(Error::OperationConflict);
            }
        }
        let step = self.indexed_step(&selected.manifest, record.source_sequence)?;
        if step.capsule_digest() != record.source_capsule
            || step.operation_id() != record.source_operation
            || step.selected_generation() != record.source_selected_generation
        {
            return Err(Error::OperationConflict);
        }
        Ok(step)
    }
    fn pin_step(
        &mut self,
        record: &mut SuffixRequestRecordV1,
        step: &ReleasedStep,
    ) -> Result<(), Error> {
        let sequence = step.capsule().body().statement.sequence;
        let (frozen, frozen_bound) = if let Some((suffix, _)) = step.suffix() {
            (
                suffix.to_canonical_bytes()?,
                SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                    + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                    + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                    + 1024,
            )
        } else {
            (
                archive::encode(step.ordinary_frozen()?)?,
                KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1
                    + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                    + archive::METADATA_BOUND,
            )
        };
        for (role, bytes, bound) in [
            (2, frozen, frozen_bound),
            (
                4,
                step.completion_frame().to_vec(),
                SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1,
            ),
        ] {
            let key = put_original(&mut self.archive, &bytes, bound)?;
            record.pins = record
                .pins
                .set(&mut self.archive, pin_key(role, sequence), &key)?;
        }
        Ok(())
    }
    /// Select an immutable source and perform one bounded preparation turn.
    /// # Errors
    /// Invalid or changed request terms, source conflict, unavailable originals or resource refusal.
    pub fn suffix_unload_prepare(
        &mut self,
        request: SuffixUnloadRequestV1,
    ) -> Result<ControlFlow<SuffixUnloadDecisionV1, SuffixPreparationJobV1>, Error> {
        request.validate()?;
        if self.scheduler.payment_pending() {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Waiting(
                ReviewMaintenanceWaitV1::Busy,
            )));
        }
        if matches!(self.status()?, SlotStatus::Pending(_)) {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Waiting(
                ReviewMaintenanceWaitV1::SelectedPending,
            )));
        }
        self.sync_manifest()?;
        let mut selected = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        if let Some(record) = self.suffix_request(&selected, &request.request_id)? {
            let retained: SuffixUnloadRequestV1 = archive::decode(&original(
                &mut self.archive,
                &record.request_original,
                ORIGINAL_BOUND,
            )?)?;
            if retained != request {
                return Err(Error::OperationConflict);
            }
            if record.released.is_some() {
                return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Released(
                    self.suffix_released_projection(&record)?,
                )));
            }
            return self.suffix_turn(selected, record, request);
        }
        // Ordinary monetary work may supersede an unselected review. Retire only that
        // stale scheduling owner; exact selected suffix recovery remains active.
        if let Some(active) = selected.suffix.as_ref().and_then(|s| s.active_request) {
            let held = self
                .suffix_request(&selected, &active)?
                .ok_or(Error::WitnessLost("suffix active request"))?;
            if selected.manifest.capsule != held.source_capsule {
                let selected_suffix = if let Some(key) = held.selection {
                    let selection: SuffixPublicationSelectionV1 =
                        read_record(&mut self.archive, key)?;
                    let next = self.step_entry(
                        &selected.manifest,
                        held.source_sequence
                            .checked_add(1)
                            .ok_or(Error::OperationConflict)?,
                    )?;
                    next.operation == selection.operation_id
                } else {
                    false
                };
                if !selected_suffix {
                    let mut namespace = selected
                        .suffix
                        .take()
                        .ok_or(Error::WitnessLost("suffix active namespace"))?;
                    namespace.active_request = None;
                    selected.digest = self.publish_suffix_checkpoint(
                        selected.digest,
                        &selected.manifest,
                        &namespace,
                    )?;
                    selected.suffix = Some(namespace);
                    self.proofs.suffix_pass = None;
                }
            }
        }
        if let Some(active) = selected.suffix.as_ref().and_then(|s| s.active_request) {
            let held = self
                .suffix_request(&selected, &active)?
                .ok_or(Error::WitnessLost("suffix active request"))?;
            if selected.manifest.capsule != held.source_capsule {
                return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Waiting(
                    ReviewMaintenanceWaitV1::Busy,
                )));
            }
            // A new explicit request may replace an unsigned review. The old token and any
            // completing worker fail their active-request fence; Begin itself moves no value.
            let mut namespace = selected
                .suffix
                .take()
                .ok_or(Error::WitnessLost("suffix active namespace"))?;
            namespace.active_request = None;
            selected.digest =
                self.publish_suffix_checkpoint(selected.digest, &selected.manifest, &namespace)?;
            selected.suffix = Some(namespace);
            self.proofs.suffix_pass = None;
        }
        let Some(anchor_sequence) = selected.manifest.folded else {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::NotApplicable(
                SuffixNotApplicableV1::NoAnchor,
            )));
        };
        let source_sequence = selected.manifest.indexed.ok_or(Error::NoHead)?;
        if anchor_sequence == source_sequence {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::NotApplicable(
                SuffixNotApplicableV1::NoSuffix,
            )));
        }
        let count = source_sequence
            .checked_sub(anchor_sequence)
            .ok_or(Error::WitnessLost("suffix range"))?;
        if count > 4096 {
            return Err(Error::Invalid("suffix step count bound"));
        }
        if selected
            .manifest
            .collection
            .as_ref()
            .is_some_and(|i| (anchor_sequence..=source_sequence).contains(&i.sequence))
        {
            return Err(Error::Collected);
        }
        let anchor_step = self.indexed_step(&selected.manifest, anchor_sequence)?;
        let source = self.indexed_step(&selected.manifest, source_sequence)?;
        let fold_bytes = self
            .archive
            .get(
                ArchiveKey::Fold(anchor_sequence),
                KAGEMUSHA_WALLET_FOLD_RECORD_MAX_BYTES_V1 + archive::METADATA_BOUND,
            )?
            .ok_or(Error::WitnessLost("suffix anchor fold"))?;
        let fold_binding = selected
            .manifest
            .folds
            .get(&mut self.archive, &manifest::sequence_key(anchor_sequence))?
            .ok_or(Error::WitnessLost("suffix anchor index"))?;
        if fold_binding != provider_digest("wallet-recorded-fold", &fold_bytes) {
            return Err(Error::WitnessLost("suffix anchor original identity"));
        }
        let fold: RecordedFold = archive::decode(&fold_bytes)?;
        if fold.record.sequence != anchor_sequence
            || fold.record.capsule_digest != anchor_step.capsule_digest()
            || fold.record.head != anchor_step.capsule().body().statement.successor
            || selected.manifest.credit_tree.root() != fold.record.lineage.public.credit_digest_root
        {
            return Err(Error::WitnessLost("suffix anchor source binding"));
        }
        let anchor_source = self.source_custody(&selected.manifest, &anchor_step)?;
        let state = anchor_step.capsule().body().successor_state;
        let certificates: KagemushaWalletCertificateSetV1 = archive::decode(
            &anchor_source
                .original(
                    &mut self.archive,
                    &state,
                    PreparationOriginalV1::EnrollmentCertificates,
                )?
                .ok_or(Error::WitnessLost("suffix anchor certificates"))?,
        )?;
        let issuer = model(|| {
            certificates.certificate(
                &anchor_step.credential().body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            )
        })?
        .clone();
        let anchor = KagemushaWalletSuffixAnchorV1 {
            version: 1,
            credential: *anchor_step.credential(),
            certificates: KagemushaWalletCertificateSetV1 {
                certificates: vec![issuer],
            },
            lineage: fold.record.lineage,
            state,
        };
        let anchor_original = put_original(
            &mut self.archive,
            &model(|| anchor.to_canonical_bytes())?,
            KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
        )?;
        let anchor_credit_tree = put_original(
            &mut self.archive,
            &archive::encode(&selected.manifest.credit_tree)?,
            4096,
        )?;
        let anchor_pending = address(
            &selected
                .manifest
                .fold_pending
                .get(&mut self.archive, &manifest::sequence_key(anchor_sequence))?
                .ok_or(Error::WitnessLost("suffix anchor pending descriptor"))?,
        )?;
        let source_custody = self.source_custody(&selected.manifest, &source)?;
        let source_custody = put_original(
            &mut self.archive,
            &archive::encode(&source_custody)?,
            preparation_custody::SOURCE_CUSTODY_MAX_BYTES,
        )?;
        let request_original = put_original(
            &mut self.archive,
            &archive::encode(&request)?,
            ORIGINAL_BOUND,
        )?;
        let mut record = SuffixRequestRecordV1 {
            version: 1,
            discriminator: 3,
            scheme_id: self.scheme_id,
            wallet_id: self.wallet_id,
            request_id: request.request_id,
            request_original,
            source_checkpoint: selected.digest,
            source_capsule: source.capsule_digest(),
            source_operation: source.operation_id(),
            source_sequence,
            source_selected_generation: source.selected_generation(),
            source_custody,
            anchor_original,
            anchor_credit_tree,
            anchor_pending,
            anchor_sequence,
            pins: IndexRoot::default(),
            progress: None,
            selection: None,
            released: None,
        };
        self.pin_step(&mut record, &anchor_step)?;
        self.pin_step(&mut record, &source)?;
        self.save_request(&mut selected, &record)?;
        self.suffix_turn(selected, record, request)
    }
    /// Resume from exact selected originals. No progress record grants proof authority.
    /// An authenticated request-index miss returns `NotStarted` DATA for this exact request.
    /// # Errors
    /// Invalid identity, unavailable checkpoint, malformed or missing retained originals, or conflict.
    pub fn suffix_unload_resume(
        &mut self,
        request_id: &[u8; 32],
    ) -> Result<ControlFlow<SuffixUnloadDecisionV1, SuffixPreparationJobV1>, Error> {
        if *request_id == [0; 32] {
            return Err(Error::Invalid("suffix request identity"));
        }
        // A crash may occur after provider release and before StepEntry publication. Adopt
        // only the exact provider-selected tail before deciding whether to review or finish.
        if matches!(self.status()?, SlotStatus::Released(_)) {
            self.sync_manifest()?;
        }
        let selected = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        let record = match self.suffix_resume_observation(&selected, request_id)? {
            ControlFlow::Break(absent) => {
                return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::NotStarted(
                    absent,
                )));
            }
            ControlFlow::Continue(record) => record,
        };
        let request: SuffixUnloadRequestV1 = archive::decode(&original(
            &mut self.archive,
            &record.request_original,
            ORIGINAL_BOUND,
        )?)?;
        request.validate()?;
        if request.request_id != *request_id {
            return Err(Error::WitnessLost("suffix retained request"));
        }
        if record.released.is_some() {
            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Released(
                self.suffix_released_projection(&record)?,
            )));
        }
        if matches!(self.status()?, SlotStatus::Pending(_)) {
            return self.suffix_resume_selected(selected, record);
        }
        self.suffix_turn(selected, record, request)
    }
    pub(super) fn suffix_turn(
        &mut self,
        mut selected: SelectedCheckpoint,
        mut record: SuffixRequestRecordV1,
        request: SuffixUnloadRequestV1,
    ) -> Result<ControlFlow<SuffixUnloadDecisionV1, SuffixPreparationJobV1>, Error> {
        if selected.suffix.as_ref().and_then(|s| s.active_request) != Some(record.request_id) {
            return Err(Error::OperationConflict);
        }
        self.require_source(&selected, &record)?;
        let total = record.source_sequence - record.anchor_sequence;
        if let Some(pass) = &self.proofs.suffix_pass {
            if pass.binding.request_id != record.request_id
                || pass.binding.source_capsule != record.source_capsule
            {
                return Err(Error::OperationConflict);
            }
            if let Some(inflight) = &pass.inflight {
                if !inflight.finished() {
                    return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::Waiting(
                        ReviewMaintenanceWaitV1::Busy,
                    )));
                }
                // An un-published result or failed/cancelled worker lost its live cursor.
                self.proofs.suffix_pass = None;
            }
        }
        let mut pass = self.proofs.suffix_pass.take().unwrap_or(LiveSuffixPass {
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
            tip: None,
            inflight: None,
        });
        self.proofs.suffix_serial = self
            .proofs
            .suffix_serial
            .checked_add(1)
            .ok_or(Error::Invalid("suffix turn overflow"))?;
        pass.binding.turn = self.proofs.suffix_serial;
        let phase = if let Some(tip) = pass.tip.take() {
            self.suffix_release_phase(&selected, &record, tip)?
        } else if let Some(cursor) = pass.cursor.take() {
            let done = cursor.point().0.core.sequence - record.anchor_sequence;
            if done == total {
                let tip = worker::suffix(cursor.into_tip())?;
                if selected
                    .manifest
                    .indexed
                    .is_some_and(|s| s > record.source_sequence)
                {
                    self.suffix_release_phase(&selected, &record, tip)?
                } else {
                    let input = self.suffix_finish_inputs(&selected, &record, &request, &tip)?;
                    Phase::Finish { tip, input }
                }
            } else {
                let mut progress = if let Some(key) = record.progress {
                    read_record::<SuffixPreparationProgressV1>(&mut self.archive, key)?
                } else {
                    SuffixPreparationProgressV1 {
                        version: 1,
                        discriminator: 9,
                        scheme_id: self.scheme_id,
                        wallet_id: self.wallet_id,
                        request_id: record.request_id,
                        source_checkpoint: record.source_checkpoint,
                        pinned_steps: 0,
                        processed_steps: 0,
                        emitted_chunks: 0,
                        emitted_stream_bytes: 0,
                        chunk_digests: IndexRoot::default(),
                        chunk_originals: IndexRoot::default(),
                        credit_tree: archive::decode(&original(
                            &mut self.archive,
                            &record.anchor_credit_tree,
                            4096,
                        )?)?,
                        partial_step_original: None,
                        partial_step_length: 0,
                    }
                };
                progress.require((self.scheme_id, self.wallet_id))?;
                let end = (done + 8).min(total);
                let mut steps = Vec::new();
                let mut before = *cursor.point().0;
                for offset in done..end {
                    let sequence = record.anchor_sequence + offset + 1;
                    let key = pin_key(10, sequence);
                    let bytes = if offset < progress.pinned_steps {
                        let key = address(
                            &record
                                .pins
                                .get(&mut self.archive, &key)?
                                .ok_or(Error::WitnessLost("suffix pinned step"))?,
                        )?;
                        original(
                            &mut self.archive,
                            &key,
                            KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1,
                        )?
                    } else {
                        let step = self.indexed_step(&selected.manifest, sequence)?;
                        if !matches!(
                            step.capsule().body().kind,
                            KagemushaWalletOperationKindV1::Receive
                                | KagemushaWalletOperationKindV1::Unload
                        ) {
                            let mut namespace = selected
                                .suffix
                                .take()
                                .ok_or(Error::WitnessLost("suffix namespace"))?;
                            namespace.active_request = None;
                            self.publish_suffix_checkpoint(
                                selected.digest,
                                &selected.manifest,
                                &namespace,
                            )?;
                            return Ok(ControlFlow::Break(SuffixUnloadDecisionV1::NotApplicable(
                                SuffixNotApplicableV1::UnsupportedStep,
                            )));
                        }
                        self.pin_step(&mut record, &step)?;
                        let charge = self.suffix_step_charge(&selected.manifest, &step)?;
                        let bytes = source_steps::step_original(
                            &mut self.archive,
                            &step,
                            &before,
                            &selected.manifest.issued_requests,
                            charge.as_ref(),
                            self.proofs.installed.verifier().scheme(),
                            &mut progress.credit_tree,
                        )?;
                        let original = put_original(
                            &mut self.archive,
                            &bytes,
                            KAGEMUSHA_WALLET_SUFFIX_STEP_MAX_BYTES_V1,
                        )?;
                        record.pins = record.pins.set(&mut self.archive, key, &original)?;
                        progress.pinned_steps = offset + 1;
                        bytes
                    };
                    let parsed = model(|| {
                        KagemushaWalletSuffixStepV1::decode_canonical(&bytes, &self.scheme_id)
                    })?;
                    before = parsed.after;
                    steps.push(bytes);
                }
                // A single turn retains at most eight new step originals and their map paths.
                record.progress = Some(write_record(&mut self.archive, &progress)?);
                self.save_request(&mut selected, &record)?;
                let chunk_index = done / 8;
                let stream_offset = if chunk_index > 0 {
                    // Fixed-size progress lookup: each previous chunk commits its end offset.
                    let previous_key = address(
                        &progress
                            .chunk_originals
                            .get(&mut self.archive, &manifest::sequence_key(chunk_index - 1))?
                            .ok_or(Error::WitnessLost("suffix preceding chunk"))?,
                    )?;
                    let previous_original = original(
                        &mut self.archive,
                        &previous_key,
                        KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
                    )?;
                    let previous = model(|| {
                        KagemushaWalletSuffixChunkV1::decode_canonical(
                            &previous_original,
                            &self.scheme_id,
                        )
                    })?;
                    previous.stream_offset + previous.data.len() as u128
                } else {
                    0
                };
                Phase::Chunk {
                    cursor,
                    chunk_index,
                    stream_offset,
                    steps,
                }
            }
        } else {
            let anchor_original = original(
                &mut self.archive,
                &record.anchor_original,
                KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
            )?;
            Phase::Anchor(model(|| {
                KagemushaWalletSuffixAnchorV1::decode_canonical(&anchor_original, &self.scheme_id)
            })?)
        };
        let owner = Speculation::new();
        let job = owner.job();
        pass.inflight = Some(owner);
        let binding = pass.binding;
        self.proofs.suffix_pass = Some(pass);
        Ok(ControlFlow::Continue(SuffixPreparationJobV1 {
            binding,
            installed: Arc::clone(&self.proofs.installed),
            sources: Arc::clone(&self.proofs.sources),
            budget: self.proofs.budget,
            read: self.proofs.read,
            scheduler: self.scheduler.clone(),
            _lifetime: SuffixJobLifetime(job.clone()),
            job,
            phase,
        }))
    }
    fn suffix_finish_inputs(
        &mut self,
        selected: &SelectedCheckpoint,
        record: &SuffixRequestRecordV1,
        request: &SuffixUnloadRequestV1,
        tip: &SuffixTipV1,
    ) -> Result<FinishInputs, Error> {
        let source = self.require_source(selected, record)?;
        if tip.point().0 != &source.capsule().body().successor_state {
            return Err(Error::OperationConflict);
        }
        let progress: SuffixPreparationProgressV1 = read_record(
            &mut self.archive,
            record
                .progress
                .ok_or(Error::WitnessLost("suffix complete progress"))?,
        )?;
        progress.require((self.scheme_id, self.wallet_id))?;
        if progress.pinned_steps != record.source_sequence - record.anchor_sequence
            || progress.processed_steps != progress.pinned_steps
            || progress.emitted_chunks != (progress.processed_steps + 7) / 8
        {
            return Err(Error::WitnessLost("suffix complete counts"));
        }
        let mut chunk_digests = Vec::new();
        for ordinal in 0..progress.emitted_chunks {
            chunk_digests.push(address(
                &progress
                    .chunk_digests
                    .get(&mut self.archive, &manifest::sequence_key(ordinal))?
                    .ok_or(Error::WitnessLost("suffix chunk digest index"))?,
            )?);
        }
        let mut custody: SourceCustodyV1 = archive::decode(&original(
            &mut self.archive,
            &record.source_custody,
            preparation_custody::SOURCE_CUSTODY_MAX_BYTES,
        )?)?;
        let before = tip.point().0;
        custody.require(&mut self.archive, before)?;
        let certificates: KagemushaWalletCertificateSetV1 = archive::decode(
            &custody
                .original(
                    &mut self.archive,
                    before,
                    PreparationOriginalV1::EnrollmentCertificates,
                )?
                .ok_or(Error::WitnessLost("suffix source certificates"))?,
        )?;
        let mut certificates = vec![
            model(|| {
                certificates.certificate(
                    &source.credential().body.issuer_certificate,
                    KagemushaWalletSignerRoleV1::Enrollment,
                )
            })?
            .clone(),
        ];
        let mut online_charge = 0;
        if let Some(charge) = &request.charge {
            let quote = model(|| {
                KagemushaWalletChargeQuoteV1::decode_canonical(&charge.quote, &self.scheme_id)
            })?;
            let beneficiary = request
                .beneficiary
                .as_deref()
                .ok_or(Error::Invalid("suffix charge beneficiary"))?;
            intake::account(beneficiary, &quote.body.beneficiary_account_digest)
                .map_err(|e| e.into_state_error(Error::Invalid("suffix beneficiary binding")))?;
            online_charge = quote.body.online_charge;
            let set: KagemushaWalletCertificateSetV1 = archive::decode(&charge.certificates)?;
            certificates.push(
                model(|| {
                    set.certificate(
                        &quote.body.signer_certificate,
                        KagemushaWalletSignerRoleV1::RegulatoryPolicy,
                    )
                })?
                .clone(),
            );
        }
        certificates.sort_by_key(KagemushaWalletSignerCertificateV1::certificate_digest);
        certificates.dedup_by_key(|c| c.certificate_digest());
        let certificates = KagemushaWalletCertificateSetV1 { certificates };
        model(|| certificates.validate())?;
        let mut adjusted = *before;
        adjusted.core.pending_outgoing_root = tip.point().1.pending_outgoing_root;
        if custody.maps.pending().root() != adjusted.core.pending_outgoing_root {
            let pending: map_tree::PersistentMapV1 =
                archive::decode(&original(&mut self.archive, &record.anchor_pending, 2048)?)?;
            if pending.root() != adjusted.core.pending_outgoing_root {
                return Err(Error::WitnessLost("suffix adjusted pending map"));
            }
            custody.maps.replace_pending(pending);
        }
        let leaf = KagemushaWalletRedeemLeafV1 {
            ordinal: before.core.next_redeem,
            nullifier: kagemusha_wallet_unload_nullifier_v1(
                &self.scheme_id,
                &self.wallet_id,
                before.core.next_redeem,
            ),
            amount: request.amount,
            online_charge,
        };
        let mut maps = PreparationMapsV1::new(
            &mut self.archive,
            &custody.maps,
            &adjusted,
            KagemushaWalletOperationKindV1::Unload,
            None,
        )?;
        let recovery = maps.insert(
            PreparationMapV1::Recovery,
            leaf.key(),
            model(|| leaf.leaf_value())?,
        )?;
        custody.maps = maps.snapshot();
        drop(maps);
        let existing = record
            .selection
            .map(|key| -> Result<_, Error> {
                let selection: SuffixPublicationSelectionV1 = read_record(&mut self.archive, key)?;
                selection.require((self.scheme_id, self.wallet_id))?;
                SuffixFrozenTransitionV1::decode_canonical(
                    &original(
                        &mut self.archive,
                        &selection.frozen_capsule,
                        SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                            + 1024,
                    )?,
                    &self.scheme_id,
                )
            })
            .transpose()?;
        let nonce = if let Some(frozen) = &existing {
            frozen.capsule.body().successor_state.core.state_nonce
        } else {
            NativeChoicesV1::fresh_nonce(&before.core.state_nonce)?
        };
        if existing.is_none() {
            self.require_runtime_capacity(NativeAdmissionPhaseV1::BeforeProve)?;
            let mut originals = self
                .proofs
                .originals
                .lock()
                .map_err(|_| Error::ArtifactsUnavailable("suffix original store lock"))?;
            self.proofs
                .sources
                .revalidate_sigma_cancellable(8, &mut *originals, self.proofs.read, None)
                .map_err(|e| {
                    if e.is_cancelled() {
                        Error::Cancelled
                    } else if e.is_unavailable() {
                        Error::ArtifactsUnavailable("suffix sigma originals")
                    } else {
                        Error::Proof("suffix sigma original binding")
                    }
                })?;
        }
        Ok(FinishInputs {
            request: request.clone(),
            credential: *source.credential(),
            certificates,
            account: self.proofs.account.clone(),
            account_original: self.proofs.account_original.clone(),
            anchor_digest: kagemusha_wallet_suffix_digest_v1(
                KagemushaWalletSuffixDigestRoleV1::Anchor,
                &original(
                    &mut self.archive,
                    &record.anchor_original,
                    KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
                )?,
            ),
            chunk_digests,
            stream_bytes: progress.emitted_stream_bytes,
            source_capsule: record.source_capsule,
            source_operation: record.source_operation,
            pins: record.pins,
            recovery,
            nonce,
            existing,
            successor_custody: custody,
        })
    }
    /// Publish one authenticated worker result after rechecking its exact selected source.
    /// # Errors
    /// Cancelled or superseded work, changed source, invalid verified result or storage refusal.
    pub fn suffix_unload_publish(
        &mut self,
        proved: ProvedSuffixPreparationV1,
    ) -> Result<SuffixUnloadDecisionV1, Error> {
        let mut pass = self.proofs.suffix_pass.take().ok_or(Error::Cancelled)?;
        if pass.binding != proved.binding {
            return Err(Error::OperationConflict);
        }
        if pass.inflight.take().and_then(Speculation::take).is_none() {
            return Err(Error::Cancelled);
        }
        let mut selected = self.selected_suffix_checkpoint()?.ok_or(Error::NoHead)?;
        let mut record = self
            .suffix_request(&selected, &proved.binding.request_id)?
            .ok_or(Error::WitnessLost("suffix worker request"))?;
        if selected.suffix.as_ref().and_then(|s| s.active_request) != Some(record.request_id) {
            return Err(Error::OperationConflict);
        }
        self.require_source(&selected, &record)?;
        let total = record.source_sequence - record.anchor_sequence;
        match proved.verified {
            Verified::Anchor(cursor) => {
                pass.cursor = Some(cursor);
                self.proofs.suffix_pass = Some(pass);
                Ok(SuffixUnloadDecisionV1::Verifying(SuffixUnloadProgressV1 {
                    format_version: 1,
                    discriminator: 14,
                    request_id: record.request_id,
                    completed_steps: 0,
                    total_steps: total,
                }))
            }
            Verified::Chunk {
                cursor,
                original: bytes,
            } => {
                let chunk = model(|| {
                    KagemushaWalletSuffixChunkV1::decode_canonical(&bytes, &self.scheme_id)
                })?;
                let mut progress: SuffixPreparationProgressV1 = read_record(
                    &mut self.archive,
                    record
                        .progress
                        .ok_or(Error::WitnessLost("suffix pinned progress"))?,
                )?;
                progress.require((self.scheme_id, self.wallet_id))?;
                let key = put_original(
                    &mut self.archive,
                    &bytes,
                    KAGEMUSHA_WALLET_SUFFIX_CHUNK_MAX_BYTES_V1,
                )?;
                let ordinal = manifest::sequence_key(chunk.chunk_index);
                if let Some(previous) = progress.chunk_originals.get(&mut self.archive, &ordinal)? {
                    if previous != key {
                        return Err(Error::WitnessLost("suffix replay changed chunk"));
                    }
                } else {
                    if progress.emitted_chunks != chunk.chunk_index
                        || progress.emitted_stream_bytes != chunk.stream_offset
                    {
                        return Err(Error::WitnessLost("suffix chunk publication order"));
                    }
                    progress.chunk_originals =
                        progress
                            .chunk_originals
                            .set(&mut self.archive, ordinal, &key)?;
                    progress.chunk_digests = progress.chunk_digests.set(
                        &mut self.archive,
                        ordinal,
                        &kagemusha_wallet_suffix_digest_v1(
                            KagemushaWalletSuffixDigestRoleV1::Chunk,
                            &bytes,
                        ),
                    )?;
                    progress.emitted_chunks += 1;
                    progress.emitted_stream_bytes += chunk.data.len() as u128;
                }
                let completed = cursor.point().0.core.sequence - record.anchor_sequence;
                progress.processed_steps = progress.processed_steps.max(completed);
                record.progress = Some(write_record(&mut self.archive, &progress)?);
                self.save_request(&mut selected, &record)?;
                pass.cursor = Some(cursor);
                self.proofs.suffix_pass = Some(pass);
                Ok(SuffixUnloadDecisionV1::Verifying(SuffixUnloadProgressV1 {
                    format_version: 1,
                    discriminator: 14,
                    request_id: record.request_id,
                    completed_steps: completed,
                    total_steps: total,
                }))
            }
            Verified::Release { operation } => self
                .suffix_publish_release(&mut selected, &mut record, operation)
                .map(SuffixUnloadDecisionV1::Released),
            Verified::Finish {
                tip,
                frozen,
                projection,
                successor_custody,
            } => {
                successor_custody
                    .require(&mut self.archive, &frozen.capsule.body().successor_state)?;
                let request: SuffixUnloadRequestV1 = archive::decode(&original(
                    &mut self.archive,
                    &record.request_original,
                    ORIGINAL_BOUND,
                )?)?;
                let selection = if let Some(key) = record.selection {
                    let selection: SuffixPublicationSelectionV1 =
                        read_record(&mut self.archive, key)?;
                    if original(
                        &mut self.archive,
                        &selection.frozen_capsule,
                        SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                            + 1024,
                    )? != frozen.to_canonical_bytes()?
                    {
                        return Err(Error::WitnessLost("suffix selected retry changed capsule"));
                    }
                    key
                } else {
                    let body = frozen.capsule.body();
                    let header = model(|| {
                        KagemushaWalletSuffixHeaderV1::decode_canonical(
                            &body.authority_header,
                            &self.scheme_id,
                        )
                    })?;
                    let header_original = put_original(
                        &mut self.archive,
                        &body.authority_header,
                        KAGEMUSHA_WALLET_SUFFIX_HEADER_MAX_BYTES_V1,
                    )?;
                    let anchor = original(
                        &mut self.archive,
                        &record.anchor_original,
                        KAGEMUSHA_WALLET_SUFFIX_ANCHOR_MAX_BYTES_V1,
                    )?;
                    let begin = KagemushaWalletLedgerV1 {
                        scheme: self.scheme_id,
                        action: KagemushaWalletLedgerActionV1::SuffixUnloadBegin {
                            wallet: self.wallet_id,
                            authority: body.authority_digest,
                            account: self.proofs.account.clone(),
                            ordinal: 0,
                            header: body.authority_header.clone(),
                            anchor,
                        },
                    };
                    let begin_instruction_original = put_original(
                        &mut self.archive,
                        &archive::encode(&begin)?,
                        super::super::suffix_ledger::SUFFIX_INSTRUCTION_MAX_BYTES_V1,
                    )?;
                    let frozen_capsule = put_original(
                        &mut self.archive,
                        &frozen.to_canonical_bytes()?,
                        SUFFIX_RECOVERY_FRAME_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
                            + KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
                            + 1024,
                    )?;
                    let successor_custody = put_original(
                        &mut self.archive,
                        &archive::encode(&successor_custody)?,
                        preparation_custody::SOURCE_CUSTODY_MAX_BYTES,
                    )?;
                    let progress: SuffixPreparationProgressV1 = read_record(
                        &mut self.archive,
                        record
                            .progress
                            .ok_or(Error::WitnessLost("suffix selected progress"))?,
                    )?;
                    let selection = SuffixPublicationSelectionV1 {
                        version: 1,
                        discriminator: 5,
                        scheme_id: self.scheme_id,
                        wallet_id: self.wallet_id,
                        request_id: record.request_id,
                        authority: body.authority_digest,
                        source_checkpoint: record.source_checkpoint,
                        source_capsule: record.source_capsule,
                        source_operation: record.source_operation,
                        source_sequence: record.source_sequence,
                        anchor_original: record.anchor_original,
                        header_original,
                        begin_instruction_original,
                        chunk_originals: progress.chunk_originals,
                        chunk_count: header.chunk_count,
                        stream_bytes: header.stream_bytes,
                        step_count: header.step_count,
                        pins: record.pins,
                        native_choices: frozen_capsule,
                        frozen_capsule,
                        successor_custody,
                        operation_id: body.operation_id,
                    };
                    selection.require((self.scheme_id, self.wallet_id))?;
                    let key = write_record(&mut self.archive, &selection)?;
                    record.selection = Some(key);
                    self.save_request(&mut selected, &record)?;
                    key
                };
                Ok(SuffixUnloadDecisionV1::Ready(ReviewedSuffixUnloadV1 {
                    request,
                    projection,
                    tip,
                    frozen,
                    selection,
                    source_checkpoint: record.source_checkpoint,
                    source_selected_generation: record.source_selected_generation,
                }))
            }
        }
    }
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::kagemusha_wallet_state_v1::tests::fixture;

    #[test]
    fn model_decoder_preserves_enclosing_refusal_and_exact_retry() {
        let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
        let original = payment.to_canonical_bytes().unwrap();
        let scheme = payment.send.statement.scheme_id;
        let read = || model(|| KagemushaWalletPaymentV1::decode_canonical(&original, &scheme));
        let limits = norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64);
        let error = norito::with_decode_limits_scope(limits, read).unwrap_err();
        assert!(matches!(error, Error::DecodeResource(ref cause)
            if cause.kind() == norito::core::DecodeLocalRefusalKind::EnclosingLimit));
        assert_eq!(read().unwrap().to_canonical_bytes().unwrap(), original);
        assert!(matches!(
            model(|| KagemushaWalletPaymentV1::decode_canonical(&[0], &scheme)),
            Err(Error::Invalid(_))
        ));
    }
}
