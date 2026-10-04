//! One closed DKG phase owner retaining its original private work and input cursors.
//!
//! No refusal restarts the command, reacquires an inherited FIFO, selects another
//! pool or rerolls a consumed primitive. The current semantic phase advances
//! before publication. Finality retains the once-decoded original journal while
//! retrying the sole atomic NativeJournalCursor advancement.
//!
//! TODO: native journal decoded block/index preparation is still an independent
//! open funding boundary. The exact raw journal frame is funded here; keeping an
//! existing bounded canonical journal alive does not certify its heap admission.

use super::*;
use claim::{ClaimError, PreparedAttemptClaim};
use input::{FrameInput, FrameReadError};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, PrepaidBufferError, RetainedPayload,
};
use iroha_core::beacon::{
    AuthenticatedGlobalBeaconDkgAttemptV1, GlobalThresholdBeaconInputErrorV1,
    LocalGlobalThresholdBeaconDkgSeatV1, PreparedGlobalThresholdBeaconDkgInputsV1,
    PreparedGlobalThresholdBeaconDkgPublicationV1,
    PreparedGlobalThresholdBeaconSessionVerificationV1, VerifiedGlobalBeaconDkgCheckpointContextV1,
};
use publication::{PhaseFile, PhasePublication};

mod claim;
mod durable;
mod durable_deadline;
use durable::PreparedDurableDkg;
use durable_deadline::DurableDeadline;
mod finality;
mod input;
use finality::FinalityInput;
mod publication;

#[cfg(test)]
mod test_pipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Prepared,
    RestoringGeneration,
    RestoringDeliveries,
    RestoringAcceptances,
    Claiming,
    Claimed,
    GenerationIntentDurable,
    Generated,
    PublicationEncoded,
    PublicationDurable,
    CommitmentsDecoded,
    CommitmentsFinalized,
    DeliveriesSigned,
    DeliveriesEncoded,
    DeliveriesDurable,
    EdgesDecoded,
    EdgesFinalized,
    AcceptancesSigned,
    AcceptancesEncoded,
    AcceptancesDurable,
    SessionDecoded,
    SessionFinalized,
    SessionSealed,
    ExportPrepared,
    ShareExtracted,
    Complete,
    Terminal,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AttemptError {
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    #[error(transparent)]
    Backing(#[from] PrepaidBufferError),
    #[error(transparent)]
    DurableDecode(norito::core::PreparedDecodeError<std::convert::Infallible>),
    #[error(transparent)]
    DurableScope(norito::core::PreparedDecodeScopeError),
    #[error(transparent)]
    Input(#[from] GlobalThresholdBeaconInputErrorV1),
    #[error(transparent)]
    Frame(#[from] FrameReadError),
    #[error(transparent)]
    Claim(#[from] ClaimError),
    #[error(transparent)]
    Export(#[from] seat_export::ExportError),
    #[error(transparent)]
    Local(#[from] LocalGlobalThresholdBeaconDkgErrorV1),
    #[error(transparent)]
    Session(#[from] GlobalThresholdBeaconSessionError),
    #[error(transparent)]
    Journal(#[from] iroha_core::sumeragi::native_journal::NativeJournalError),
    #[error(transparent)]
    JournalSource(#[from] iroha_data_model::sumeragi::finality::PreparedNativeFinalityError),
    #[error("the original DKG attempt is in another phase")]
    Phase,
    #[error("the original DKG finality height is invalid or closed")]
    Height,
    #[error("the original DKG attempt deadline elapsed")]
    Deadline,
    #[error("the prepared DKG attempt has invalid bindings")]
    Binding,
}
impl AttemptError {
    fn terminal(&self, phase: Phase) -> bool {
        use iroha_core::sumeragi::native_journal::NativeJournalError;
        use iroha_data_model::sumeragi::finality::{
            NativeFinalityDecodeError, PreparedNativeFinalityError,
        };
        match self {
            Self::Phase | Self::Height | Self::Deadline | Self::Binding => true,
            Self::DurableDecode(norito::core::PreparedDecodeError::Codec(error)) => {
                error.kind() == norito::core::DecodeAttemptErrorKind::Invalid
            }
            Self::Local(
                LocalGlobalThresholdBeaconDkgErrorV1::Invalid(_)
                | LocalGlobalThresholdBeaconDkgErrorV1::Session(
                    GlobalThresholdBeaconSessionError::Invalid(_)
                    | GlobalThresholdBeaconSessionError::ForeignReservation
                    | GlobalThresholdBeaconSessionError::PlanChanged,
                ),
            ) => true,
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Checkpoint(
                iroha_crypto::threshold_bls::checkpoint::DkgCheckpointErrorV1::Binding
                | iroha_crypto::threshold_bls::checkpoint::DkgCheckpointErrorV1::Terminal,
            )) => true,
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Checkpoint(
                iroha_crypto::threshold_bls::checkpoint::DkgCheckpointErrorV1::Decode(
                    norito::core::PreparedDecodeError::Codec(error),
                ),
            )) => error.kind() == norito::core::DecodeAttemptErrorKind::Invalid,
            // A restored source/context is immutable and this phase uses no RNG.
            // Retrying a deterministic secret/proof failure cannot repair it.
            // Keep the original physical, scope and entropy causes local even
            // here; sealing phases retain their existing retry classification.
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Checkpoint(error))
                if matches!(
                    phase,
                    Phase::RestoringGeneration
                        | Phase::RestoringDeliveries
                        | Phase::RestoringAcceptances
                ) =>
            {
                use iroha_crypto::{
                    encryption::Error as EncryptionError,
                    hybrid::HybridError,
                    threshold_bls::{ThresholdBlsError, checkpoint::DkgCheckpointErrorV1},
                };
                match error {
                    DkgCheckpointErrorV1::Encryption(
                        EncryptionError::NonceGeneration(_) | EncryptionError::InertNonce,
                    )
                    | DkgCheckpointErrorV1::Hybrid(HybridError::RandomBytes { .. })
                    | DkgCheckpointErrorV1::Threshold(
                        ThresholdBlsError::RandomnessUnavailable
                        | ThresholdBlsError::InertRandomness,
                    ) => false,
                    DkgCheckpointErrorV1::Encryption(_)
                    | DkgCheckpointErrorV1::Hybrid(_)
                    | DkgCheckpointErrorV1::Threshold(_)
                    | DkgCheckpointErrorV1::Encoding(norito::Error::NonCanonicalEncoding) => true,
                    _ => false,
                }
            }
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Hybrid(error))
                if matches!(
                    phase,
                    Phase::RestoringGeneration
                        | Phase::RestoringDeliveries
                        | Phase::RestoringAcceptances
                ) =>
            {
                !matches!(error, iroha_crypto::hybrid::HybridError::RandomBytes { .. })
            }
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Threshold(error))
                if matches!(
                    phase,
                    Phase::RestoringGeneration
                        | Phase::RestoringDeliveries
                        | Phase::RestoringAcceptances
                ) =>
            {
                !matches!(
                    error,
                    iroha_crypto::threshold_bls::ThresholdBlsError::RandomnessUnavailable
                        | iroha_crypto::threshold_bls::ThresholdBlsError::InertRandomness
                )
            }
            Self::Local(LocalGlobalThresholdBeaconDkgErrorV1::Session(
                GlobalThresholdBeaconSessionError::Encoding(norito::Error::NonCanonicalEncoding),
            )) if matches!(
                phase,
                Phase::RestoringGeneration
                    | Phase::RestoringDeliveries
                    | Phase::RestoringAcceptances
            ) =>
            {
                true
            }
            Self::JournalSource(
                PreparedNativeFinalityError::Invalid(_)
                | PreparedNativeFinalityError::SourceChanged
                | PreparedNativeFinalityError::NotDecoded,
            ) => true,
            Self::JournalSource(PreparedNativeFinalityError::Decode(
                norito::core::PreparedDecodeError::Codec(error),
            )) => error.kind() == norito::core::DecodeAttemptErrorKind::Invalid,
            Self::Input(
                GlobalThresholdBeaconInputErrorV1::Binding
                | GlobalThresholdBeaconInputErrorV1::Phase
                | GlobalThresholdBeaconInputErrorV1::SourceChanged,
            ) => true,
            Self::Input(GlobalThresholdBeaconInputErrorV1::Decode(
                norito::core::PreparedDecodeError::Codec(error),
            )) => error.kind() == norito::core::DecodeAttemptErrorKind::Invalid,
            Self::Frame(
                FrameReadError::Deadline
                | FrameReadError::Length
                | FrameReadError::EndOfStream
                | FrameReadError::Custody
                | FrameReadError::Phase,
            ) => true,
            Self::Claim(
                ClaimError::AmbiguousCreation(_)
                | ClaimError::AlreadyClaimed(_)
                | ClaimError::Custody
                | ClaimError::Phase
                | ClaimError::Deadline
                | ClaimError::Directory(
                    seat_export::ExportError::Custody | seat_export::ExportError::Phase,
                ),
            ) => true,
            Self::Export(seat_export::ExportError::Custody | seat_export::ExportError::Phase) => {
                true
            }
            Self::Session(
                GlobalThresholdBeaconSessionError::Invalid(_)
                | GlobalThresholdBeaconSessionError::ForeignReservation
                | GlobalThresholdBeaconSessionError::PlanChanged,
            ) => true,
            Self::Journal(
                NativeJournalError::Invalid(_)
                | NativeJournalError::Decode(
                    NativeFinalityDecodeError::Invalid(_) | NativeFinalityDecodeError::Malformed(_),
                ),
            ) => true,
            _ => false,
        }
    }
}

/// Every phase's actual owner remains in this receiver until success or explicit drop.
pub(crate) struct SeatDkgAttempt {
    prepared: Option<PreparedLocalGlobalThresholdBeaconDkgSeatV1>,
    local: Option<LocalGlobalThresholdBeaconDkgSeatV1>,
    inputs: PreparedGlobalThresholdBeaconDkgInputsV1,
    final_graph: Option<RetainedPayload<GlobalThresholdBeaconKeySessionV1>>,
    verifier: Option<PreparedGlobalThresholdBeaconSessionVerificationV1>,
    sealed: Option<ValidatedGlobalThresholdBeaconSessionV1>,
    export: Option<seat_export::PreparedSeatExport>,
    rejected_components: Option<Zeroizing<[[u8; 32]; 3]>>,
    public_input: FrameInput,
    finality: FinalityInput,
    claim: PreparedAttemptClaim,
    authority: AuthenticatedGlobalBeaconDkgAttemptV1,
    durable: PreparedDurableDkg,
    original_publications: [Option<PreparedGlobalThresholdBeaconDkgPublicationV1>; 3],
    restore_target: Option<u16>,
    restored_private_phase: u16,
    restored_dealer_retired: bool,
    publications: [PhasePublication; 4],
    source_publications: [PhasePublication; 6],
    provider_handle: ChargedBuffer<u8>,
    provider_revision: u64,
    signer: KeyPair,
    signer_index: u16,
    session: GlobalThresholdBeaconDkgSessionV1,
    input_bounds: [usize; 3],
    cutoff: u64,
    deadline: Instant,
    budget: AllocationBudget,
    phase: Phase,
}
impl SeatDkgAttempt {
    /// All known preparation succeeds before the first mkdir, RNG or signature.
    pub(super) fn new(
        authority: AuthenticatedGlobalBeaconDkgAttemptV1,
        roster: &[PeerId],
        signer_index: u16,
        signer: KeyPair,
        public_input: File,
        finality_input: File,
        clock: NativeJournalCursor,
        provider_handle: &str,
        provider_revision: u64,
        attempt_root: &Path,
        deadline: Instant,
        budget: &AllocationBudget,
    ) -> std::result::Result<SeatDkgAttemptOwner, AttemptError> {
        if Instant::now() >= deadline {
            return Err(AttemptError::Deadline);
        }
        let session = authority.session();
        let cutoff = authority.cutoff();
        if provider_revision == 0
            || iroha_config::parameters::validate_production_runtime_handle(provider_handle)
                .is_err()
            || !clock.allocation_budget().same_pool(budget)
            || clock.network_id() != session.network_id
            || clock.tip().map(|tip| tip.height()).unwrap_or(1) != session.start_height
            || session.acceptances_end_height >= cutoff
        {
            return Err(AttemptError::Binding);
        }
        let prepared = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
            session,
            roster,
            signer_index,
            &signer,
            budget,
        )?;
        let input_bounds = prepared.input_frame_bounds();
        if input_bounds
            .iter()
            .any(|size| *size == 0 || *size > MAX_PUBLIC_BYTES)
        {
            return Err(AttemptError::Binding);
        }
        let verifier = prepared.prepare_final_session_verifier()?;
        let inputs = PreparedGlobalThresholdBeaconDkgInputsV1::new(session, roster, budget)?;
        let public_input = FrameInput::new(public_input, input_bounds[0], deadline, budget)
            .map_err(|(_source, error)| error)?;
        let finality_input = FrameInput::new(
            finality_input,
            clock.limits().journal_bytes,
            deadline,
            budget,
        )
        .map_err(|(_source, error)| error)?;
        public_input.require_distinct_source(&finality_input)?;
        let mut claim = PreparedAttemptClaim::new(
            attempt_root,
            &session.attempt_id,
            signer_index,
            deadline,
            budget,
        )?;
        let expiry = DurableDeadline::freeze(deadline)?;
        // Reload decoders are independent original-source banks, needed only
        // for an existing claimed prefix. Metadata grants bounded preparation,
        // never source or phase authority. Pin the owner-private child first;
        // all banks precede any private restoration or publication adoption.
        let (original_publications, restore_target) = if claim.existing()? {
            claim.open_existing()?;
            let phase = PreparedDurableDkg::restore_phase_hint(
                claim.read_directory().ok_or(AttemptError::Phase)?,
            )?;
            (
                [
                    Some(PreparedGlobalThresholdBeaconDkgPublicationV1::new(
                        session,
                        roster,
                        signer_index,
                        budget,
                    )?),
                    if phase >= 2 {
                        Some(PreparedGlobalThresholdBeaconDkgPublicationV1::new_delivery(
                            session,
                            roster,
                            signer_index,
                            budget,
                        )?)
                    } else {
                        None
                    },
                    if phase >= 3 {
                        Some(
                            PreparedGlobalThresholdBeaconDkgPublicationV1::new_acceptance(
                                session,
                                roster,
                                signer_index,
                                budget,
                            )?,
                        )
                    } else {
                        None
                    },
                ],
                Some(phase),
            )
        } else {
            ([None, None, None], None)
        };
        let durable = PreparedDurableDkg::new(
            expiry,
            input_bounds[0],
            prepared.private_checkpoint_bytes(),
            budget,
        )?;
        let total = provider_handle
            .len()
            .checked_add(std::mem::size_of::<SeatDkgAttempt>())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        let mut owner = ChargedBuffer::from_reservation(1, &mut reservation)?;
        let mut handle = ChargedBuffer::from_reservation(provider_handle.len(), &mut reservation)?;
        handle
            .append(provider_handle.as_bytes())
            .map_err(|_| AttemptError::Phase)?;
        if reservation.remaining_bytes() != 0 {
            return Err(AttemptError::Phase);
        }
        owner.push_reserved(Self {
            prepared: Some(prepared),
            local: None,
            inputs,
            final_graph: None,
            verifier: Some(verifier),
            sealed: None,
            export: None,
            rejected_components: None,
            public_input,
            finality: FinalityInput::new(finality_input, clock, session.start_height)?,
            claim,
            authority,
            durable,
            original_publications,
            restore_target,
            restored_private_phase: 0,
            restored_dealer_retired: false,
            publications: [
                PhasePublication::new(PhaseFile::GenerationIntent),
                PhasePublication::new(PhaseFile::Publication),
                PhasePublication::new(PhaseFile::Deliveries),
                PhasePublication::new(PhaseFile::Acceptances),
            ],
            source_publications: [
                PhasePublication::new(PhaseFile::CommitmentsInput),
                PhasePublication::new(PhaseFile::DeliveriesInput),
                PhasePublication::new(PhaseFile::SessionInput),
                PhasePublication::new(PhaseFile::CommitmentsProof),
                PhasePublication::new(PhaseFile::DeliveriesProof),
                PhasePublication::new(PhaseFile::SessionProof),
            ],
            provider_handle: handle,
            provider_revision,
            signer,
            signer_index,
            session,
            input_bounds,
            cutoff,
            deadline,
            budget: budget.clone(),
            phase: Phase::Prepared,
        });
        Ok(SeatDkgAttemptOwner { receiver: owner })
    }
    fn phase_input_hash(&self, phase: u16) -> std::result::Result<[u8; 32], AttemptError> {
        if phase == 1 {
            return Ok([0; 32]);
        }
        let source = match phase {
            2 => self.inputs.commitments(),
            3 => self.inputs.deliveries(),
            _ => None,
        }
        .ok_or(AttemptError::Phase)?;
        Ok(self
            .local
            .as_ref()
            .ok_or(AttemptError::Phase)?
            .checkpoint_input_hash(phase, source)?)
    }
    fn context(
        &self,
        phase: u16,
        public_hash: [u8; 32],
        intent_hash: [u8; 32],
    ) -> std::result::Result<VerifiedGlobalBeaconDkgCheckpointContextV1, AttemptError> {
        let handle = std::str::from_utf8(self.provider_handle.as_slice())
            .map_err(|_| AttemptError::Binding)?;
        Ok(self.authority.checkpoint_context(
            self.finality.clock(),
            phase,
            self.signer_index,
            &self.signer,
            handle,
            self.provider_revision,
            public_hash,
            self.phase_input_hash(phase)?,
            self.durable.previous_checkpoint_hash(phase)?,
            intent_hash,
        )?)
    }
    fn claim_and_fifo_identity(
        &self,
    ) -> std::result::Result<([u64; 4], [u8; 32], [u64; 4]), AttemptError> {
        let (claim, path) = self.claim.identity()?;
        let public = self.public_input.source_identity()?;
        let finality = self.finality.source_identity()?;
        Ok((
            claim,
            path,
            [public[0], public[1], finality[0], finality[1]],
        ))
    }
    fn original_source_hashes(
        &self,
        phase: u16,
    ) -> std::result::Result<[[u8; 32]; 2], AttemptError> {
        if phase == 1 {
            return Ok([[0; 32]; 2]);
        }
        let index = usize::from(phase.checked_sub(2).ok_or(AttemptError::Phase)?);
        if index >= 2 {
            return Err(AttemptError::Phase);
        }
        Ok([
            self.source_publications[index].complete_hash()?,
            self.source_publications[index + 3].complete_hash()?,
        ])
    }
    fn stream_generations(&self) -> [u64; 2] {
        [self.public_input.generation(), self.finality.generation()]
    }
    fn prepare_intent(&mut self, phase: u16) -> std::result::Result<(), AttemptError> {
        let context = self.context(phase, [0; 32], [0; 32])?;
        let (claim, path, fifos) = self.claim_and_fifo_identity()?;
        let sources = self.original_source_hashes(phase)?;
        let generations = self.stream_generations();
        self.durable.prepare_intent(
            phase,
            context.binding(),
            claim,
            path,
            fifos,
            None,
            sources,
            generations,
        )
    }
    fn close_previous_restore_before_input(
        &mut self,
        phase: u16,
    ) -> std::result::Result<(), AttemptError> {
        if !(1..=3).contains(&phase) {
            return Err(AttemptError::Phase);
        }
        let context = *self.durable.latest_context()?;
        if context.phase != phase {
            return Err(AttemptError::Binding);
        }
        let (claim, path, fifos) = self.claim_and_fifo_identity()?;
        let generations = self.stream_generations();
        self.durable.prepare_intent(
            phase + 4,
            &context,
            claim,
            path,
            fifos,
            None,
            [[0; 32]; 2],
            generations,
        )?;
        self.durable.publish_intent(
            self.claim.directory().ok_or(AttemptError::Phase)?,
            phase + 4,
        )
    }
    fn close_generation_restore_before_input(&mut self) -> std::result::Result<(), AttemptError> {
        self.close_previous_restore_before_input(1)
    }
    fn publish_public_source(&mut self, index: usize) -> std::result::Result<(), AttemptError> {
        let bytes = self.public_input.frame().ok_or(AttemptError::Phase)?;
        if index >= 3 || bytes.len() > self.input_bounds[index] {
            return Err(AttemptError::Binding);
        }
        self.source_publications[index]
            .publish(self.claim.directory().ok_or(AttemptError::Phase)?, bytes)?;
        Ok(())
    }
    fn advance_original_finality(
        &mut self,
        index: usize,
        target: u64,
    ) -> std::result::Result<(), AttemptError> {
        if index >= 3 {
            return Err(AttemptError::Phase);
        }
        let directory = self.claim.directory().ok_or(AttemptError::Phase)?;
        let bound = self.finality.clock().limits().journal_bytes;
        let publication = &mut self.source_publications[index + 3];
        self.finality.advance_to(target, self.cutoff, |frame| {
            if frame.len() > bound {
                return Err(AttemptError::Binding);
            }
            publication.publish(directory, frame)?;
            Ok(())
        })
    }
    fn prepare_extraction_intent(&mut self) -> std::result::Result<(), AttemptError> {
        let sealed = self.sealed.as_ref().ok_or(AttemptError::Phase)?;
        let source = self
            .authority
            .finalized_source(self.finality.clock(), sealed)?;
        let continuation = sealed.record().transcript_hash;
        let context = *self.durable.latest_context()?;
        let (claim, path, fifos) = self.claim_and_fifo_identity()?;
        let source_hashes = [
            self.source_publications[2].complete_hash()?,
            self.source_publications[5].complete_hash()?,
        ];
        let generations = self.stream_generations();
        self.durable.prepare_intent(
            4,
            &context,
            claim,
            path,
            fifos,
            Some((continuation, source)),
            source_hashes,
            generations,
        )?;
        self.durable
            .publish_intent(self.claim.directory().ok_or(AttemptError::Phase)?, 4)
    }
    fn seal_and_publish_checkpoint(&mut self, phase: u16) -> std::result::Result<(), AttemptError> {
        let public_hash = Hash::new(
            self.local
                .as_ref()
                .ok_or(AttemptError::Phase)?
                .encoded_public_frame(),
        )
        .into();
        let context = self.context(phase, public_hash, self.durable.intent_hash(phase)?)?;
        let encrypted = self
            .local
            .as_mut()
            .ok_or(AttemptError::Phase)?
            .seal_private_checkpoint(&context, &self.signer)?;
        self.durable.publish_checkpoint(
            self.claim.directory().ok_or(AttemptError::Phase)?,
            phase,
            encrypted,
        )
    }
    fn publish_checkpoint_head(&mut self, phase: u16) -> std::result::Result<(), AttemptError> {
        let public_hash = Hash::new(
            self.local
                .as_ref()
                .ok_or(AttemptError::Phase)?
                .encoded_public_frame(),
        )
        .into();
        let context = self.context(phase, public_hash, self.durable.intent_hash(phase)?)?;
        let encrypted = self
            .local
            .as_mut()
            .ok_or(AttemptError::Phase)?
            .seal_private_checkpoint(&context, &self.signer)?;
        self.durable.publish_head(
            self.claim.directory().ok_or(AttemptError::Phase)?,
            context.binding(),
            Hash::new(encrypted).into(),
        )
    }
    fn restore_generation(&mut self) -> std::result::Result<(), AttemptError> {
        if self.original_publications[0].is_none() {
            // A claim appearing after fresh preparation cannot manufacture a late
            // decoder or be adopted by the fresh producer owner.
            return Err(AttemptError::Binding);
        }
        self.claim.open_existing()?;
        let directory = self.claim.read_directory().ok_or(AttemptError::Phase)?;
        let target = self.durable.prepare_restore(
            directory,
            self.input_bounds,
            self.finality.clock().limits().journal_bytes,
        )?;
        if self.restore_target.is_some_and(|old| old != target) {
            return Err(AttemptError::Binding);
        }
        if self.original_publications[..usize::from(target)]
            .iter()
            .any(Option::is_none)
        {
            return Err(AttemptError::Binding);
        }
        self.restore_target = Some(target);
        let (intent, head) = self.durable.load_generation_through(directory, target)?;
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        let (claim, path, fifos) = self.claim_and_fifo_identity()?;
        #[cfg(all(test, sumeragi_daemon_mutation = "HC106"))]
        let fifos = intent.fifo_identity;
        if (claim, path, fifos)
            != (
                intent.claim_identity,
                intent.claim_path_hash,
                intent.fifo_identity,
            )
        {
            return Err(AttemptError::Binding);
        }
        let handle = std::str::from_utf8(self.provider_handle.as_slice())
            .map_err(|_| AttemptError::Binding)?;
        let context = self.authority.checkpoint_context(
            self.finality.clock(),
            1,
            self.signer_index,
            &self.signer,
            handle,
            self.provider_revision,
            head.context.public_output_hash,
            [0; 32],
            [0; 32],
            head.context.producer_intent_hash,
        )?;
        if context.binding() != &head.context {
            return Err(AttemptError::Binding);
        }
        if self.local.is_none() {
            let bank = self.original_publications[0]
                .as_mut()
                .ok_or(AttemptError::Phase)?;
            bank.decode(
                self.durable.public_source(),
                norito::canonical_decode_limits(self.durable.public_source().len()),
            )?;
            let publication = bank.publication().ok_or(AttemptError::Phase)?;
            let prepared = self.prepared.take().ok_or(AttemptError::Phase)?;
            match prepared.restore_generated(
                &context,
                publication,
                self.durable.public_source(),
                self.durable.private_source(),
                &self.signer,
                norito::canonical_decode_limits(self.durable.private_source().len()),
            ) {
                Ok(local) => {
                    self.local = Some(local);
                    self.restored_private_phase = 1;
                }
                Err((prepared, cause)) => {
                    self.prepared = Some(prepared);
                    return Err(cause.into());
                }
            }
        }
        self.durable.retain_restored_generation(intent, head)?;
        let directory = self.claim.read_directory().ok_or(AttemptError::Phase)?;
        self.publications[0].restore_complete(directory, self.durable.intent_bytes(1)?)?;
        self.publications[1].restore_complete(
            directory,
            self.local
                .as_ref()
                .ok_or(AttemptError::Phase)?
                .encoded_public_frame(),
        )?;
        self.durable.sync_restored_generation(directory)?;
        if target > 1 {
            self.phase = Phase::RestoringDeliveries;
            return Ok(());
        }
        // Suspend-inclusive time spent in read/proof/fsync may never grant extra time.
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        self.claim.authenticate_restored(claim, path, deadline)?;
        // Claim ancestry fsync is another blocking boundary, including suspension.
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        self.claim.tighten_deadline(deadline);
        self.phase = Phase::PublicationDurable;
        Ok(())
    }

    /// Replay a complete original later phase without reading/reopening the FIFO,
    /// generating another private primitive or importing a recorded tip hash.
    fn restore_later(&mut self, phase: u16) -> std::result::Result<(), AttemptError> {
        let directory = self.claim.read_directory().ok_or(AttemptError::Phase)?;
        let (intent, head) = self.durable.load_later(directory, phase)?;
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        let (claim, path, fifos) = self.claim_and_fifo_identity()?;
        if (claim, path, fifos)
            != (
                intent.claim_identity,
                intent.claim_path_hash,
                intent.fifo_identity,
            )
        {
            return Err(AttemptError::Binding);
        }
        let target = match phase {
            2 => self.session.commitments_end_height,
            3 => self.session.deliveries_end_height,
            _ => return Err(AttemptError::Phase),
        };
        let (_, _, input, proof) = self.durable.later_sources(phase)?;
        if phase == 2 {
            self.inputs
                .decode_commitments(input, norito::canonical_decode_limits(input.len()))?;
        } else {
            self.inputs
                .decode_deliveries(input, norito::canonical_decode_limits(input.len()))?;
        }
        self.finality
            .restore_target_from_original_frame(proof, target, self.cutoff)?;
        let context = self.context(
            phase,
            head.context.public_output_hash,
            head.context.producer_intent_hash,
        )?;
        if context.binding() != &head.context {
            return Err(AttemptError::Binding);
        }
        if self.restored_private_phase < phase {
            let (output, private, _, _) = self.durable.later_sources(phase)?;
            let bank = self.original_publications[usize::from(phase - 1)]
                .as_mut()
                .ok_or(AttemptError::Phase)?;
            bank.decode(output, norito::canonical_decode_limits(output.len()))?;
            let publication = bank.publication().ok_or(AttemptError::Phase)?;
            let phase_input = if phase == 2 {
                self.inputs.commitments()
            } else {
                self.inputs.deliveries()
            }
            .ok_or(AttemptError::Phase)?;
            let local = self.local.take().ok_or(AttemptError::Phase)?;
            let restored = if phase == 2 {
                local.restore_delivered(
                    &context,
                    phase_input,
                    publication,
                    output,
                    private,
                    &self.signer,
                    norito::canonical_decode_limits(private.len()),
                )
            } else {
                local.restore_accepted(
                    &context,
                    phase_input,
                    publication,
                    output,
                    private,
                    &self.signer,
                    norito::canonical_decode_limits(private.len()),
                )
            };
            match restored {
                Ok(local) => {
                    self.local = Some(local);
                    self.restored_private_phase = phase;
                }
                Err((local, cause)) => {
                    self.local = Some(local);
                    return Err(cause.into());
                }
            }
        }
        let directory = self.claim.read_directory().ok_or(AttemptError::Phase)?;
        let index = usize::from(phase - 2);
        let (_, _, input, proof) = self.durable.later_sources(phase)?;
        self.source_publications[index].restore_complete(directory, input)?;
        self.source_publications[index + 3].restore_complete(directory, proof)?;
        self.publications[usize::from(phase)].restore_complete(
            directory,
            self.local
                .as_ref()
                .ok_or(AttemptError::Phase)?
                .encoded_public_frame(),
        )?;
        self.durable.sync_restored_later(directory, phase)?;
        self.durable.prepare_restore(
            directory,
            self.input_bounds,
            self.finality.clock().limits().journal_bytes,
        )?;
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        self.durable.retain_restored_later(phase, intent, head)?;
        // Retirement follows the real original checkpoint/output/head barriers.
        // Retrying the same phase never reconstructs or re-signs the polynomial.
        if phase == 2 && !self.restored_dealer_retired {
            self.local
                .as_mut()
                .ok_or(AttemptError::Phase)?
                .retire_durably_published_dealer()?;
            self.restored_dealer_retired = true;
        }
        if phase < self.restore_target.ok_or(AttemptError::Phase)? {
            self.phase = Phase::RestoringAcceptances;
            return Ok(());
        }
        self.finish_restored_claim(intent, claim, path, phase)?;
        Ok(())
    }
    fn finish_restored_claim(
        &mut self,
        intent: durable::Intent,
        claim: [u64; 4],
        path: [u8; 32],
        phase: u16,
    ) -> std::result::Result<(), AttemptError> {
        let public_generation = intent.stream_generations[0]
            .checked_add(1)
            .ok_or(AttemptError::Binding)?;
        self.public_input.restore_empty_cursor(
            public_generation,
            self.input_bounds[usize::from(phase - 1)],
            [intent.fifo_identity[0], intent.fifo_identity[1]],
        )?;
        self.finality.restore_stream_generation(
            intent.stream_generations[1],
            [intent.fifo_identity[2], intent.fifo_identity[3]],
        )?;
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        self.claim.authenticate_restored(claim, path, deadline)?;
        let deadline = intent.expiry.restore(self.deadline)?;
        self.deadline = self.deadline.min(deadline);
        self.public_input.tighten_deadline(deadline);
        self.finality.tighten_deadline(deadline);
        self.claim.tighten_deadline(deadline);
        self.phase = match phase {
            2 => Phase::DeliveriesDurable,
            3 => Phase::AcceptancesDurable,
            _ => return Err(AttemptError::Phase),
        };
        Ok(())
    }

    fn publish_phase(&mut self, index: usize) -> std::result::Result<(), AttemptError> {
        let directory = self.claim.directory().ok_or(AttemptError::Phase)?;
        let bytes = if index == 0 {
            self.durable.intent_bytes(1)?
        } else {
            self.local
                .as_ref()
                .ok_or(AttemptError::Phase)?
                .encoded_public_frame()
        };
        self.publications[index].publish(directory, bytes)?;
        if !self.publications[index].complete() {
            return Err(AttemptError::Phase);
        }
        Ok(())
    }
    fn next_public_input(&mut self, bound: usize) -> std::result::Result<(), AttemptError> {
        // A signed phase may retry encoding/publication, but consumes its input only
        // once. After consumption the next phase's complete input is still absent.
        if self.public_input.frame().is_some() {
            self.public_input.consume_verified_frame()?;
        }
        self.public_input.set_next_maximum(bound)?;
        Ok(())
    }
    fn step(&mut self) -> std::result::Result<(), AttemptError> {
        self.deadline = self.durable.tightened_deadline(self.deadline)?;
        self.public_input.tighten_deadline(self.deadline);
        self.finality.tighten_deadline(self.deadline);
        self.claim.tighten_deadline(self.deadline);
        if Instant::now() >= self.deadline {
            return Err(AttemptError::Deadline);
        }
        match self.phase {
            Phase::Prepared => {
                if self.claim.existing()? {
                    self.phase = Phase::RestoringGeneration;
                    self.restore_generation()?;
                } else {
                    self.phase = Phase::Claiming;
                    self.claim.make_durable()?;
                    self.phase = Phase::Claimed;
                }
            }
            Phase::Claiming => {
                self.claim.make_durable()?;
                self.phase = Phase::Claimed;
            }
            Phase::RestoringGeneration => {
                self.restore_generation()?;
            }
            Phase::RestoringDeliveries => {
                self.restore_later(2)?;
            }
            Phase::RestoringAcceptances => {
                self.restore_later(3)?;
            }

            Phase::Claimed => {
                self.prepare_intent(1)?;
                self.publish_phase(0)?;
                self.phase = Phase::GenerationIntentDurable;
            }
            Phase::GenerationIntentDurable => {
                let prepared = self.prepared.take().ok_or(AttemptError::Phase)?;
                self.phase = Phase::Terminal;
                self.local = Some(prepared.generate(&self.signer)?);
                self.phase = Phase::Generated;
            }
            Phase::Generated => {
                self.local
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .publication_frame()?;
                self.phase = Phase::PublicationEncoded;
            }
            Phase::PublicationEncoded => {
                self.seal_and_publish_checkpoint(1)?;
                self.publish_phase(1)?;
                self.publish_checkpoint_head(1)?;
                self.phase = Phase::PublicationDurable;
            }
            Phase::PublicationDurable => {
                self.close_generation_restore_before_input()?;
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_commitments(frame, norito::canonical_decode_limits(frame.len()))?;
                self.phase = Phase::CommitmentsDecoded;
            }
            Phase::CommitmentsDecoded => {
                self.publish_public_source(0)?;
                self.advance_original_finality(0, self.session.commitments_end_height)?;
                self.phase = Phase::CommitmentsFinalized;
            }
            Phase::CommitmentsFinalized => {
                self.prepare_intent(2)?;
                self.durable
                    .publish_intent(self.claim.directory().ok_or(AttemptError::Phase)?, 2)?;
                let source = self.inputs.commitments().ok_or(AttemptError::Phase)?;
                self.phase = Phase::Terminal;
                let _ = self.local.as_mut().ok_or(AttemptError::Phase)?.deliver(
                    &source.recipient_keys,
                    &source.dealer_commitments,
                    self.finality.height(),
                    &self.signer,
                )?;
                self.phase = Phase::DeliveriesSigned;
            }
            Phase::DeliveriesSigned => {
                self.next_public_input(self.input_bounds[1])?;
                self.local
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .delivery_frame(self.inputs.commitments().ok_or(AttemptError::Phase)?)?;
                self.phase = Phase::DeliveriesEncoded;
            }
            Phase::DeliveriesEncoded => {
                self.seal_and_publish_checkpoint(2)?;
                self.publish_phase(2)?;
                self.publish_checkpoint_head(2)?;
                // Only the original complete file+directory publication retires
                // the original runtime polynomial. Writer refusal retains it.
                self.local
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .retire_durably_published_dealer()?;
                self.phase = Phase::DeliveriesDurable;
            }
            Phase::DeliveriesDurable => {
                self.close_previous_restore_before_input(2)?;
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_deliveries(frame, norito::canonical_decode_limits(frame.len()))?;
                self.phase = Phase::EdgesDecoded;
            }
            Phase::EdgesDecoded => {
                self.publish_public_source(1)?;
                self.advance_original_finality(1, self.session.deliveries_end_height)?;
                self.phase = Phase::EdgesFinalized;
            }
            Phase::EdgesFinalized => {
                self.prepare_intent(3)?;
                self.durable
                    .publish_intent(self.claim.directory().ok_or(AttemptError::Phase)?, 3)?;
                let source = self.inputs.deliveries().ok_or(AttemptError::Phase)?;
                self.phase = Phase::Terminal;
                let _ = self.local.as_mut().ok_or(AttemptError::Phase)?.accept(
                    source,
                    self.finality.height(),
                    &self.signer,
                )?;
                self.phase = Phase::AcceptancesSigned;
            }
            Phase::AcceptancesSigned => {
                self.next_public_input(self.input_bounds[2])?;
                self.local
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .acceptance_frame(self.inputs.deliveries().ok_or(AttemptError::Phase)?)?;
                self.phase = Phase::AcceptancesEncoded;
            }
            Phase::AcceptancesEncoded => {
                self.seal_and_publish_checkpoint(3)?;
                self.publish_phase(3)?;
                self.publish_checkpoint_head(3)?;
                self.phase = Phase::AcceptancesDurable;
            }
            Phase::AcceptancesDurable => {
                self.close_previous_restore_before_input(3)?;
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_final_session(frame, norito::canonical_decode_limits(frame.len()))?;
                self.final_graph = Some(self.inputs.take_final_session()?);
                self.phase = Phase::SessionDecoded;
            }
            Phase::SessionDecoded => {
                self.publish_public_source(2)?;
                self.advance_original_finality(2, self.session.acceptances_end_height)?;
                self.phase = Phase::SessionFinalized;
            }
            Phase::SessionFinalized => {
                let graph = self.final_graph.take().ok_or(AttemptError::Phase)?;
                let record = graph.get();
                let binding = GlobalThresholdBeaconSessionBindingV1 {
                    network_id: self.session.network_id,
                    session_id: self.session.session_id,
                    roster_hash: self.session.roster_hash,
                    transcript_hash: record.transcript_hash,
                };
                let verifier = self.verifier.take().ok_or(AttemptError::Phase)?;
                match verifier.seal(graph, &binding) {
                    Ok(sealed) => {
                        self.sealed = Some(sealed);
                        self.phase = Phase::SessionSealed;
                    }
                    Err((verifier, graph, cause)) => {
                        self.verifier = Some(verifier);
                        self.final_graph = Some(graph);
                        return Err(cause.into());
                    }
                }
            }
            Phase::SessionSealed => {
                let directory = self.claim.directory().ok_or(AttemptError::Phase)?;
                let handle = std::str::from_utf8(self.provider_handle.as_slice())
                    .map_err(|_| AttemptError::Phase)?;
                self.export = Some(seat_export::PreparedSeatExport::new(
                    directory,
                    self.sealed.as_ref().ok_or(AttemptError::Phase)?,
                    self.signer_index,
                    handle,
                    self.provider_revision,
                    &self.budget,
                )?);
                self.phase = Phase::ExportPrepared;
            }
            Phase::ExportPrepared => {
                self.prepare_extraction_intent()?;
                self.phase = Phase::Terminal;
                let components = self
                    .local
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .finalize_private_share(self.sealed.as_ref().ok_or(AttemptError::Phase)?)?;
                if let Err((components, cause)) = self
                    .export
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .accept(components)
                {
                    self.rejected_components = Some(components);
                    return Err(cause.into());
                }
                self.phase = Phase::ShareExtracted;
            }
            Phase::ShareExtracted => {
                self.export
                    .as_mut()
                    .ok_or(AttemptError::Phase)?
                    .publish(self.claim.directory().ok_or(AttemptError::Phase)?)?;
                if !self.export.as_ref().ok_or(AttemptError::Phase)?.complete() {
                    return Err(AttemptError::Phase);
                }
                self.phase = Phase::Complete;
            }
            Phase::Complete => {}
            Phase::Terminal => return Err(AttemptError::Phase),
        }
        Ok(())
    }
}
/// Sole mutable receiver in its original prepaid singleton allocation.
///
/// Resume and failure move only this handle. The private graph stays at the same
/// address, and the physical receiver backing refunds only after its graph drops.
pub(crate) struct SeatDkgAttemptOwner {
    receiver: ChargedBuffer<SeatDkgAttempt>,
}
impl std::ops::Deref for SeatDkgAttemptOwner {
    type Target = SeatDkgAttempt;
    fn deref(&self) -> &Self::Target {
        &self.receiver.as_slice()[0]
    }
}
impl std::ops::DerefMut for SeatDkgAttemptOwner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.receiver.as_mut_slice()[0]
    }
}
impl SeatDkgAttemptOwner {
    pub(crate) fn resume(mut self) -> std::result::Result<(), PendingSeatDkgAttempt> {
        while self.phase != Phase::Complete {
            if let Err(cause) = self.step() {
                if cause.terminal(self.phase) {
                    self.phase = Phase::Terminal;
                }
                return Err(PendingSeatDkgAttempt { owner: self, cause });
            }
        }
        Ok(())
    }
}
/// Original complete failed attempt; debug/display never traverse private custody.
pub(crate) struct PendingSeatDkgAttempt {
    pub(crate) owner: SeatDkgAttemptOwner,
    pub(crate) cause: AttemptError,
}
impl std::fmt::Debug for PendingSeatDkgAttempt {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("PendingSeatDkgAttempt")
            .field("phase", &self.owner.phase)
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for PendingSeatDkgAttempt {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.cause, out)
    }
}
impl std::error::Error for PendingSeatDkgAttempt {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod durable_tests;

#[cfg(test)]
mod restore_error_tests;

#[cfg(test)]
mod later_restore_tests;
