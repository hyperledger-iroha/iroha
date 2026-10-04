//! One set of independent phase destinations sharing original canonical decode controls.

use super::*;
use common::DestinationError;
use iroha_data_model::consensus::GlobalThresholdBeaconDkgSessionV1;
use norito::core::{PreparedDecodeError, PreparedDecodeScopeError, PreparedDecodeWorkspace};
use snapshot::{Snapshot, SnapshotPhase};

/// Original preparation, decoder, or source-custody failure for one retained input.
#[derive(Debug, thiserror::Error)]
pub enum GlobalThresholdBeaconInputErrorV1 {
    /// Physical original-pool construction failed before any private attempt claim.
    #[error(transparent)]
    Preparation(#[from] crate::beacon::GlobalThresholdBeaconSessionError),
    /// The original canonical decoder or prepared destination refused its source.
    #[error(transparent)]
    Decode(#[from] PreparedDecodeError<DestinationError>),
    /// The original counter/control owner refused construction or reuse.
    #[error(transparent)]
    Scope(#[from] PreparedDecodeScopeError),
    /// A retry offered different bytes or another allocation in place of its source.
    #[error("prepared DKG input no longer has its original source")]
    SourceChanged,
    /// A destination was consumed, offered out of order, or disagreed with its plan.
    #[error("prepared DKG input is in another phase")]
    Phase,
    /// A canonically decoded phase does not bind the original authenticated attempt.
    #[error("prepared DKG input does not bind its original session and phase")]
    Binding,
}

struct Source {
    address: usize,
    length: usize,
    digest: iroha_crypto::Hash,
}
impl Source {
    fn new(bytes: &[u8]) -> Self {
        Self {
            address: bytes.as_ptr().addr(),
            length: bytes.len(),
            digest: iroha_crypto::Hash::new(bytes),
        }
    }
    fn matches(&self, bytes: &[u8]) -> bool {
        self.address == bytes.as_ptr().addr()
            && self.length == bytes.len()
            && self.digest == iroha_crypto::Hash::new(bytes)
    }
}
trait Destination: Sized {
    type Record;
    fn decode(
        &mut self,
        workspace: &mut PreparedDecodeWorkspace,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), PreparedDecodeError<DestinationError>>;
    fn finish(self) -> Result<RetainedPayload<Self::Record>, Self>;
}
impl Destination for Snapshot {
    type Record = crate::beacon::GlobalThresholdBeaconDkgSnapshotV1;
    fn decode(
        &mut self,
        workspace: &mut PreparedDecodeWorkspace,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), PreparedDecodeError<DestinationError>> {
        self.decode(workspace, bytes, limits)
    }
    fn finish(self) -> Result<RetainedPayload<Self::Record>, Self> {
        self.finish()
    }
}
impl Destination for session::Session {
    type Record = GlobalThresholdBeaconKeySessionV1;
    fn decode(
        &mut self,
        workspace: &mut PreparedDecodeWorkspace,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), PreparedDecodeError<DestinationError>> {
        self.decode(workspace, bytes, limits)
    }
    fn finish(self) -> Result<RetainedPayload<Self::Record>, Self> {
        self.finish()
    }
}
struct Bank<D: Destination> {
    prepared: Option<D>,
    retained: Option<RetainedPayload<D::Record>>,
    source: Option<Source>,
}
impl<D: Destination> Bank<D> {
    fn new(prepared: D) -> Self {
        Self {
            prepared: Some(prepared),
            retained: None,
            source: None,
        }
    }
    fn decode(
        &mut self,
        workspace: &mut PreparedDecodeWorkspace,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<&D::Record, GlobalThresholdBeaconInputErrorV1> {
        if let Some(source) = &self.source {
            if !source.matches(bytes) {
                return Err(GlobalThresholdBeaconInputErrorV1::SourceChanged);
            }
        } else {
            self.source = Some(Source::new(bytes));
        }
        if self.retained.is_none() {
            self.prepared
                .as_mut()
                .ok_or(GlobalThresholdBeaconInputErrorV1::Phase)?
                .decode(workspace, bytes, limits)?;
            let prepared = self
                .prepared
                .take()
                .ok_or(GlobalThresholdBeaconInputErrorV1::Phase)?;
            match prepared.finish() {
                Ok(retained) => self.retained = Some(retained),
                Err(prepared) => {
                    self.prepared = Some(prepared);
                    return Err(GlobalThresholdBeaconInputErrorV1::Phase);
                }
            }
        }
        self.retained
            .as_ref()
            .map(RetainedPayload::get)
            .ok_or(GlobalThresholdBeaconInputErrorV1::Phase)
    }
}

/// One original-pool canonical publication graph for checkpoint restoration.
///
/// Each phase has its own exact row storage and canonical decode controls,
/// prepared before the durable claim. Generation owns one recipient/dealer,
/// delivery owns the full roster and one dealer's `n` edges, and acceptance owns
/// all `n²` incoming edges plus one recipient's `n` acknowledgments. The original
/// session always binds the full ordered committee and its threshold. No reduced
/// committee, source replacement or phase-bank reset is permitted.
pub struct PreparedGlobalThresholdBeaconDkgPublicationV1 {
    publication: Bank<Snapshot>,
    workspace: PreparedDecodeWorkspace,
    session: GlobalThresholdBeaconDkgSessionV1,
    seat: u16,
    phase: SnapshotPhase,
    budget: AllocationBudget,
    bound: bool,
}
impl PreparedGlobalThresholdBeaconDkgPublicationV1 {
    /// Prepare exact singleton nested storage and original canonical controls.
    ///
    /// # Errors
    /// Returns the original geometry, admission or physical refusal before claim.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        seat: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        Self::for_phase(
            session,
            roster,
            seat,
            SnapshotPhase::SinglePublication { seat },
            budget,
        )
    }

    /// Prepare the complete roster and exactly this dealer's `n` original delivery rows.
    ///
    /// # Errors
    /// Preserves invalid geometry and original pool/allocator refusal before claim.
    pub fn new_delivery(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        seat: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        Self::for_phase(
            session,
            roster,
            seat,
            SnapshotPhase::LocalDeliveries { seat },
            budget,
        )
    }

    /// Prepare all `n²` original inbound edges and this recipient's `n` acknowledgments.
    ///
    /// # Errors
    /// Preserves invalid geometry and original pool/allocator refusal before claim.
    pub fn new_acceptance(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        seat: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        Self::for_phase(
            session,
            roster,
            seat,
            SnapshotPhase::LocalAcceptances { seat },
            budget,
        )
    }

    fn for_phase(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        seat: u16,
        phase: SnapshotPhase,
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        let publication = Bank::new(Snapshot::new(session, roster, phase, budget)?);
        let mut reservation = budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(crate::beacon::GlobalThresholdBeaconSessionError::from)?;
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        Ok(Self {
            publication,
            workspace,
            session,
            seat,
            phase,
            budget: budget.clone(),
            bound: false,
        })
    }

    /// Whether every retained publication and decode-control owner uses this pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget)
            && self.workspace.belongs_to(budget)
            && self
                .publication
                .retained
                .as_ref()
                .is_none_or(|row| row.belongs_to(budget))
    }

    /// Exact original prepared storage retired by the first successful extraction.
    #[cfg(test)]
    pub(in crate::beacon) fn decode_retirement_bytes(&self) -> Option<usize> {
        self.publication
            .prepared
            .as_ref()
            .map(Snapshot::extraction_scaffolding_bytes)
    }

    /// Fill the original bank once through the existing generated canonical walk.
    ///
    /// Signature/proof authentication remains the local restore owner's duty.
    ///
    /// # Errors
    /// Keeps source identity and every prepared prefix on the original decoder
    /// refusal; rejects changed source, wrong local row geometry, seat or phase.
    pub fn decode(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), GlobalThresholdBeaconInputErrorV1> {
        self.bound = false;
        let record = self
            .publication
            .decode(&mut self.workspace, bytes, limits)?;
        let n = usize::from(self.session.committee_size);
        let phase_matches = match self.phase {
            SnapshotPhase::SinglePublication { .. } => {
                record.last_updated_height == self.session.start_height
                    && record.recipient_keys.len() == 1
                    && record.dealer_commitments.len() == 1
                    && record.recipient_keys[0].recipient_index == self.seat
                    && record.dealer_commitments[0].dealer_index == self.seat
                    && record.encrypted_shares.is_empty()
                    && record.share_acceptances.is_empty()
            }
            SnapshotPhase::LocalDeliveries { .. } => {
                record.last_updated_height >= self.session.commitments_end_height
                    && record.last_updated_height < self.session.deliveries_end_height
                    && record.recipient_keys.len() == n
                    && record.dealer_commitments.len() == n
                    && record.encrypted_shares.len() == n
                    && record.share_acceptances.is_empty()
                    && record.encrypted_shares.iter().enumerate().all(|(i, edge)| {
                        edge.dealer_index == self.seat
                            && usize::from(edge.recipient_index) == i + 1
                            && edge.delivery_height == record.last_updated_height
                    })
            }
            SnapshotPhase::LocalAcceptances { .. } => {
                record.last_updated_height >= self.session.deliveries_end_height
                    && record.last_updated_height < self.session.acceptances_end_height
                    && record.recipient_keys.len() == n
                    && record.dealer_commitments.len() == n
                    && record.encrypted_shares.len() == n * n
                    && record.share_acceptances.len() == n
                    && record.share_acceptances.iter().enumerate().all(|(i, row)| {
                        row.recipient_index == self.seat
                            && usize::from(row.dealer_index) == i + 1
                            && row.accepted_height == record.last_updated_height
                    })
            }
            SnapshotPhase::Commitments | SnapshotPhase::Deliveries => false,
        };
        if record.session != self.session || !phase_matches {
            return Err(GlobalThresholdBeaconInputErrorV1::Binding);
        }
        self.bound = true;
        Ok(())
    }

    /// Borrow the complete canonical original publication without copying its rows.
    #[must_use]
    pub fn publication(&self) -> Option<&crate::beacon::GlobalThresholdBeaconDkgSnapshotV1> {
        self.bound
            .then(|| self.publication.retained.as_ref().map(RetainedPayload::get))
            .flatten()
    }
}

/// Complete original-pool input storage for one authenticated local DKG attempt.
///
/// The three banks are independently initialized before the durable attempt claim.
/// Successful canonical decoding moves exactly those allocations into immutable
/// retained records. Errors retain the source fingerprint and every destination;
/// no owned DTO fallback, late graph allocation or replacement source is allowed.
/// Source bytes and descriptor offsets remain owned by the surrounding attempt.
/// Decoding is not authentication: signatures, exact finality and the final seal
/// still use the existing local-seat and session verification owners.
pub struct PreparedGlobalThresholdBeaconDkgInputsV1 {
    commitments: Bank<Snapshot>,
    deliveries: Bank<Snapshot>,
    final_session: Bank<session::Session>,
    workspace: PreparedDecodeWorkspace,
    session: GlobalThresholdBeaconDkgSessionV1,
    budget: AllocationBudget,
    commitments_bound: bool,
    deliveries_bound: bool,
    final_bound: bool,
}
impl PreparedGlobalThresholdBeaconDkgInputsV1 {
    /// Physically prepare all complete input graphs from the authenticated roster.
    ///
    /// # Errors
    /// Every shape, pool, or physical refusal occurs before this owner is returned;
    /// the caller must not claim an attempt or invoke RNG until it succeeds.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        let commitments = Bank::new(Snapshot::new(
            session,
            roster,
            SnapshotPhase::Commitments,
            budget,
        )?);
        let deliveries = Bank::new(Snapshot::new(
            session,
            roster,
            SnapshotPhase::Deliveries,
            budget,
        )?);
        let final_session = Bank::new(session::Session::new(session, roster, budget)?);
        let mut reservation = budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(crate::beacon::GlobalThresholdBeaconSessionError::from)?;
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        Ok(Self {
            commitments,
            deliveries,
            final_session,
            workspace,
            session,
            budget: budget.clone(),
            commitments_bound: false,
            deliveries_bound: false,
            final_bound: false,
        })
    }
    /// Whether this prepared owner and its decode controls use the supplied source.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget) && self.workspace.belongs_to(budget)
    }

    /// Decode the complete commitments frame into its original destination once.
    ///
    /// # Errors
    /// Returns the unchanged decoder cause, original source mismatch or exact
    /// session/height mismatch without dropping either the bank or source binding.
    pub fn decode_commitments(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), GlobalThresholdBeaconInputErrorV1> {
        let record = self
            .commitments
            .decode(&mut self.workspace, bytes, limits)?;
        if record.session != self.session || record.last_updated_height != self.session.start_height
        {
            return Err(GlobalThresholdBeaconInputErrorV1::Binding);
        }
        self.commitments_bound = true;
        Ok(())
    }
    /// Borrow the original complete phase graph after its canonical binding checks.
    #[must_use]
    pub fn commitments(&self) -> Option<&crate::beacon::GlobalThresholdBeaconDkgSnapshotV1> {
        self.commitments_bound
            .then(|| self.commitments.retained.as_ref().map(RetainedPayload::get))
            .flatten()
    }
    /// Decode the independent all-edge frame after commitments input completes.
    ///
    /// # Errors
    /// Refusals keep original complete source and initialized destination custody.
    pub fn decode_deliveries(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), GlobalThresholdBeaconInputErrorV1> {
        if !self.commitments_bound {
            return Err(GlobalThresholdBeaconInputErrorV1::Phase);
        }
        let record = self.deliveries.decode(&mut self.workspace, bytes, limits)?;
        if record.session != self.session
            || record.last_updated_height != self.session.commitments_end_height
        {
            return Err(GlobalThresholdBeaconInputErrorV1::Binding);
        }
        self.deliveries_bound = true;
        Ok(())
    }
    /// Borrow the exact canonical all-edge phase graph, without cloning any row.
    #[must_use]
    pub fn deliveries(&self) -> Option<&crate::beacon::GlobalThresholdBeaconDkgSnapshotV1> {
        self.deliveries_bound
            .then(|| self.deliveries.retained.as_ref().map(RetainedPayload::get))
            .flatten()
    }
    /// Decode the final public session into its independent prepared graph.
    ///
    /// # Errors
    /// Rejects out-of-order input, changed source and original session/height
    /// mismatch; codec validity alone never marks the session authenticated.
    pub fn decode_final_session(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), GlobalThresholdBeaconInputErrorV1> {
        if !self.deliveries_bound {
            return Err(GlobalThresholdBeaconInputErrorV1::Phase);
        }
        let record = self
            .final_session
            .decode(&mut self.workspace, bytes, limits)?;
        if record.adaptive_dkg.session != self.session
            || record.adaptive_dkg.finalized_at_height != self.session.acceptances_end_height
        {
            return Err(GlobalThresholdBeaconInputErrorV1::Binding);
        }
        self.final_bound = true;
        Ok(())
    }
    /// Move the original complete graph once into the preclaimed session verifier.
    ///
    /// # Errors
    /// Until canonical binding succeeds, or after a prior move, returns only a
    /// phase refusal; no source graph is reconstructed or decoded a second time.
    pub fn take_final_session(
        &mut self,
    ) -> Result<RetainedPayload<GlobalThresholdBeaconKeySessionV1>, GlobalThresholdBeaconInputErrorV1>
    {
        if !self.final_bound {
            return Err(GlobalThresholdBeaconInputErrorV1::Phase);
        }
        self.final_session
            .retained
            .take()
            .ok_or(GlobalThresholdBeaconInputErrorV1::Phase)
    }
}

/// One original canonical final-session graph for a completed aggregate prefix.
///
/// This distinct boundary prepares only the final public session destination and
/// its two decode controls. It shares the existing generated field walk, source
/// fingerprint, canonical comparison and move-only bank used by complete phase
/// inputs. It does not prepare commitments, deliveries or any private ceremony
/// owner. Codec validity grants no native authority: the caller must replay the
/// actual finalized proof and pass the retained graph to the existing verifier.
pub struct PreparedGlobalThresholdBeaconFinalSessionInputV1 {
    final_session: Bank<session::Session>,
    workspace: PreparedDecodeWorkspace,
    session: GlobalThresholdBeaconDkgSessionV1,
    budget: AllocationBudget,
    final_bound: bool,
}
impl PreparedGlobalThresholdBeaconFinalSessionInputV1 {
    /// Prepare exact final public geometry from the authenticated session and roster.
    ///
    /// # Errors
    /// Returns original shape, pool or physical allocation refusal before claim.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        budget: &AllocationBudget,
    ) -> Result<Self, GlobalThresholdBeaconInputErrorV1> {
        let final_session = Bank::new(session::Session::new(session, roster, budget)?);
        let mut reservation = budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(crate::beacon::GlobalThresholdBeaconSessionError::from)?;
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        Ok(Self {
            final_session,
            workspace,
            session,
            budget: budget.clone(),
            final_bound: false,
        })
    }

    /// Observe temporary layouts in the same original destination before its graph move.
    #[cfg(test)]
    pub(super) fn extraction_scaffolding_bytes(&self) -> usize {
        self.final_session
            .prepared
            .as_ref()
            .map(session::Session::extraction_scaffolding_bytes)
            .expect("the original final-session destination must still be prepared")
    }

    /// Whether the prepared graph and its controls retain the supplied original pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget) && self.workspace.belongs_to(budget)
    }

    /// Decode the complete original final frame into its prepaid destination once.
    ///
    /// A finalization can occur at or after the frozen acceptance boundary. Its
    /// exact native tip and cutoff are checked by aggregate authority; the
    /// enclosing daemon verifies the original intent's frozen same-boot expiry.
    /// The existing final verifier checks all signatures/proofs. Every retry requires the same
    /// original source address, length and bytes, including after a scope refusal.
    ///
    /// # Errors
    /// Preserves the original codec, source, session or finalization-height cause.
    pub fn decode_final_session(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), GlobalThresholdBeaconInputErrorV1> {
        let record = self
            .final_session
            .decode(&mut self.workspace, bytes, limits)?;
        if record.adaptive_dkg.session != self.session
            || record.adaptive_dkg.finalized_at_height < self.session.acceptances_end_height
        {
            return Err(GlobalThresholdBeaconInputErrorV1::Binding);
        }
        self.final_bound = true;
        Ok(())
    }

    /// Borrow the complete canonically bound graph without copying a row.
    #[must_use]
    pub fn final_session(&self) -> Option<&GlobalThresholdBeaconKeySessionV1> {
        self.final_bound
            .then(|| {
                self.final_session
                    .retained
                    .as_ref()
                    .map(RetainedPayload::get)
            })
            .flatten()
    }

    /// Move the original final graph once to the existing final session verifier.
    ///
    /// # Errors
    /// Returns a phase refusal before complete binding or after the original move.
    pub fn take_final_session(
        &mut self,
    ) -> Result<RetainedPayload<GlobalThresholdBeaconKeySessionV1>, GlobalThresholdBeaconInputErrorV1>
    {
        if !self.final_bound {
            return Err(GlobalThresholdBeaconInputErrorV1::Phase);
        }
        self.final_session
            .retained
            .take()
            .ok_or(GlobalThresholdBeaconInputErrorV1::Phase)
    }
}
