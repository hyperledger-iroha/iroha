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
