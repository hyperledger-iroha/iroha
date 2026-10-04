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
    GlobalThresholdBeaconInputErrorV1, LocalGlobalThresholdBeaconDkgSeatV1,
    PreparedGlobalThresholdBeaconDkgInputsV1, PreparedGlobalThresholdBeaconSessionVerificationV1,
};
use norito::json::{BoundedJsonError, JsonSerialize as _, JsonWriteSink};
use publication::{PhaseFile, PhasePublication};

mod claim;
mod finality;
mod input;
use finality::FinalityInput;
mod publication;

#[cfg(test)]
mod test_pipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Prepared,
    Claimed,
    JournalDurable,
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
    Json(#[from] BoundedJsonError),
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
    fn terminal(&self) -> bool {
        use iroha_core::sumeragi::native_journal::NativeJournalError;
        use iroha_data_model::sumeragi::finality::{
            NativeFinalityDecodeError, PreparedNativeFinalityError,
        };
        match self {
            Self::Phase | Self::Height | Self::Deadline | Self::Binding => true,
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
                | ClaimError::Deadline,
            ) => true,
            Self::Export(seat_export::ExportError::Custody | seat_export::ExportError::Phase) => {
                true
            }
            Self::Session(GlobalThresholdBeaconSessionError::Invalid(_)) => true,
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

#[derive(JsonSerialize)]
struct Journal<'a> {
    schema: &'static str,
    session: GlobalThresholdBeaconDkgSessionV1,
    signer_index: u16,
    chain_id: &'a ChainId,
    network_id: NetworkId,
    native_source: Option<NativeSource>,
}
#[derive(JsonSerialize)]
struct NativeSource {
    height: u64,
    block_hash: Hash,
    consensus_hash: [u8; 32],
    result: [u8; 32],
}
struct Count(usize);
impl JsonWriteSink for Count {
    fn push(&mut self, value: char) -> std::result::Result<(), BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
    fn push_str(&mut self, value: &str) -> std::result::Result<(), BoundedJsonError> {
        self.0 = self
            .0
            .checked_add(value.len())
            .filter(|n| *n <= MAX_PUBLIC_BYTES)
            .ok_or(BoundedJsonError::BodyTooLarge)?;
        Ok(())
    }
}
struct WriteJson<'a>(&'a mut ChargedBuffer<u8>);
impl JsonWriteSink for WriteJson<'_> {
    fn push(&mut self, value: char) -> std::result::Result<(), BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
    fn push_str(&mut self, value: &str) -> std::result::Result<(), BoundedJsonError> {
        self.0
            .append(value.as_bytes())
            .map_err(|_| BoundedJsonError::LengthMismatch)
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
    attempt_journal: ChargedBuffer<u8>,
    publications: [PhasePublication; 4],
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
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        signer_index: u16,
        signer: KeyPair,
        public_input: File,
        finality_input: File,
        clock: NativeJournalCursor,
        cutoff: u64,
        provider_handle: &str,
        provider_revision: u64,
        attempt_root: &Path,
        deadline: Instant,
        budget: &AllocationBudget,
    ) -> std::result::Result<Self, AttemptError> {
        if Instant::now() >= deadline {
            return Err(AttemptError::Deadline);
        }
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
        let claim = PreparedAttemptClaim::new(
            attempt_root,
            &session.attempt_id,
            signer_index,
            deadline,
            budget,
        )?;
        let journal = Journal {
            schema: "iroha.global-beacon.dkg-seat-attempt.v1",
            session,
            signer_index,
            chain_id: clock.chain_id(),
            network_id: clock.network_id(),
            native_source: clock.tip().map(|tip| NativeSource {
                height: tip.height(),
                block_hash: tip.block_hash().into(),
                consensus_hash: tip.core_hash().0,
                result: tip.result().0,
            }),
        };
        let mut count = Count(0);
        journal.json_serialize_to(&mut count)?;
        let total = count
            .0
            .checked_add(provider_handle.len())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        let mut attempt_journal = ChargedBuffer::from_reservation(count.0, &mut reservation)?;
        journal.json_serialize_to(&mut WriteJson(&mut attempt_journal))?;
        let mut handle = ChargedBuffer::from_reservation(provider_handle.len(), &mut reservation)?;
        handle
            .append(provider_handle.as_bytes())
            .map_err(|_| AttemptError::Phase)?;
        if attempt_journal.as_slice().len() != count.0 || reservation.remaining_bytes() != 0 {
            return Err(AttemptError::Phase);
        }
        Ok(Self {
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
            attempt_journal,
            publications: [
                PhasePublication::new(PhaseFile::Journal),
                PhasePublication::new(PhaseFile::Publication),
                PhasePublication::new(PhaseFile::Deliveries),
                PhasePublication::new(PhaseFile::Acceptances),
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
        })
    }
    fn publish_phase(&mut self, index: usize) -> std::result::Result<(), AttemptError> {
        let directory = self.claim.directory().ok_or(AttemptError::Phase)?;
        let bytes = if index == 0 {
            self.attempt_journal.as_slice()
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
        if Instant::now() >= self.deadline {
            return Err(AttemptError::Deadline);
        }
        match self.phase {
            Phase::Prepared => {
                self.claim.make_durable()?;
                self.phase = Phase::Claimed;
            }
            Phase::Claimed => {
                self.publish_phase(0)?;
                self.phase = Phase::JournalDurable;
            }
            Phase::JournalDurable => {
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
                self.publish_phase(1)?;
                self.phase = Phase::PublicationDurable;
            }
            Phase::PublicationDurable => {
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_commitments(frame, norito::canonical_decode_limits(frame.len()))?;
                self.phase = Phase::CommitmentsDecoded;
            }
            Phase::CommitmentsDecoded => {
                self.finality
                    .advance_to(self.session.commitments_end_height, self.cutoff)?;
                self.phase = Phase::CommitmentsFinalized;
            }
            Phase::CommitmentsFinalized => {
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
                self.publish_phase(2)?;
                self.phase = Phase::DeliveriesDurable;
            }
            Phase::DeliveriesDurable => {
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_deliveries(frame, norito::canonical_decode_limits(frame.len()))?;
                self.phase = Phase::EdgesDecoded;
            }
            Phase::EdgesDecoded => {
                self.finality
                    .advance_to(self.session.deliveries_end_height, self.cutoff)?;
                self.phase = Phase::EdgesFinalized;
            }
            Phase::EdgesFinalized => {
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
                self.publish_phase(3)?;
                self.phase = Phase::AcceptancesDurable;
            }
            Phase::AcceptancesDurable => {
                self.public_input.read_until_complete()?;
                let frame = self.public_input.frame().ok_or(AttemptError::Phase)?;
                self.inputs
                    .decode_final_session(frame, norito::canonical_decode_limits(frame.len()))?;
                self.final_graph = Some(self.inputs.take_final_session()?);
                self.phase = Phase::SessionDecoded;
            }
            Phase::SessionDecoded => {
                self.finality
                    .advance_to(self.session.acceptances_end_height, self.cutoff)?;
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
    pub(crate) fn resume(mut self) -> std::result::Result<(), PendingSeatDkgAttempt> {
        while self.phase != Phase::Complete {
            if let Err(cause) = self.step() {
                if cause.terminal() {
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
    pub(crate) owner: SeatDkgAttempt,
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
