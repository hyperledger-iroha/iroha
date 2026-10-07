//! One original certified carrier and archive acquisition, retained across local refusal.
//!
//! The portable proof's complete backing is prepaid before copying its authenticated fields.
//! Native prefix verification remains the certified-chain reader's distinct custody boundary.

use std::{fmt, ops::Range};

use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, PrepaidBufferError, RetainedPayload,
};
use iroha_data_model::{
    block::consensus::ExecWitness,
    sumeragi_amx::{AllocatedAmxRecordProofV1, AmxProofAllocationErrorV1, AmxRecordKind},
    sumeragi_finality::{NativeLaneStateProof, NativeLaneStateProofError},
};

use super::ordinary_writes::{self, CastingOwner, WriteDecodeError};
use crate::{
    execution_attempt::ExecutionAttemptError,
    query::native_context_archive::{
        NativeContextArchive, NativeContextArchiveError, NativeContextRead,
    },
    state::StateReadOnly,
    sumeragi::certified_chain::{CertifiedBlock, CertifiedChain, ChainReadError},
};

/// Exact source, canonical or physical cause; none becomes record absence.
#[derive(Debug, thiserror::Error)]
pub enum NativeAmxRecordProofErrorV1 {
    /// Original native certificate or prefix verification refused.
    #[error(transparent)]
    Chain(#[from] ExecutionAttemptError<ChainReadError>),
    /// Retained descriptor, namespace, I/O or original-pool archive read refused.
    #[error(transparent)]
    Archive(#[from] NativeContextArchiveError),
    /// Original complete write graph demand was not admitted.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// A prepaid original write allocation refused materialization.
    #[error(transparent)]
    Materialization(#[from] PrepaidBufferError),
    /// Canonical frame or the caller's unchanged cumulative allowance refused.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Complete lane commitment or its actual same-pool scratch refused.
    #[error(transparent)]
    Lane(#[from] NativeLaneStateProofError),
    /// Exact portable proof fields or same-pool tree scratch refused.
    #[error(transparent)]
    Proof(#[from] AmxProofAllocationErrorV1),
    /// The immutable carrier, source or private allocation plan is inconsistent.
    #[error("native AMX proof source: {0}")]
    Source(&'static str),
}
impl From<WriteDecodeError> for NativeAmxRecordProofErrorV1 {
    fn from(cause: WriteDecodeError) -> Self {
        match cause {
            WriteDecodeError::ForeignPool => Self::Source("foreign original write pool"),
            WriteDecodeError::PlanChanged => Self::Source("original write allocation plan changed"),
            WriteDecodeError::Codec(cause) => Self::Codec(cause),
            WriteDecodeError::Admission(cause) => {
                #[cfg(all(test, sumeragi_core_mutation = "HC147"))]
                {
                    let _ = cause;
                    Self::Source("mutated original write admission")
                }
                #[cfg(not(all(test, sumeragi_core_mutation = "HC147")))]
                {
                    Self::Admission(cause)
                }
            }
            WriteDecodeError::Materialization(cause) => Self::Materialization(cause),
        }
    }
}

/// Bounded acquisition is distinct from authenticated absence or a completed proof.
#[derive(Debug)]
pub enum NativeAmxRecordProofPollV1 {
    /// The original descriptor acquired at most one 4096-byte prefix.
    Pending,
    /// All carrier/archive checks completed; `None` means authenticated absence.
    Complete(Option<AllocatedAmxRecordProofV1>),
}

struct Projection {
    lane_range: Range<usize>,
    witness: RetainedPayload<ExecWitness>,
    _casting: CastingOwner,
}

/// Original State cut, certified carrier and partial archive owners retained on refusal.
/// No Clone, replacement-source installation or payload extraction exists.
#[must_use = "dropping this owner abandons the exact pending native AMX proof"]
pub struct NativeAmxRecordProofReadV1<'v, V: StateReadOnly> {
    view: &'v V,
    height: u64,
    kind: AmxRecordKind,
    transaction: [u8; 32],
    budget: AllocationBudget,
    chain: Option<CertifiedChain<'v, V>>,
    certified: Option<CertifiedBlock>,
    read: Option<NativeContextRead>,
    bytes: Option<ChargedBuffer<u8>>,
    projection: Option<Projection>,
    authenticated: bool,
    completed: bool,
}
impl<V: StateReadOnly> fmt::Debug for NativeAmxRecordProofReadV1<'_, V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeAmxRecordProofReadV1")
            .field("height", &self.height)
            .field("kind", &self.kind)
            .field("transaction", &self.transaction)
            .field("authenticated", &self.authenticated)
            .field("completed", &self.completed)
            .finish_non_exhaustive()
    }
}

impl<'v, V: StateReadOnly> NativeAmxRecordProofReadV1<'v, V> {
    pub(super) fn new(
        view: &'v V,
        height: u64,
        kind: AmxRecordKind,
        transaction: [u8; 32],
    ) -> Self {
        Self {
            view,
            height,
            kind,
            transaction,
            budget: view.prepared_contract_cache().execution_budget().clone(),
            chain: None,
            certified: None,
            read: None,
            bytes: None,
            projection: None,
            authenticated: false,
            completed: false,
        }
    }

    /// Resume the same source, retaining acquired bytes through every failed decode.
    ///
    /// # Errors
    /// Original source/resource refusal; all pending owners remain with this mutable borrow.
    pub fn poll(&mut self) -> Result<NativeAmxRecordProofPollV1, NativeAmxRecordProofErrorV1> {
        use NativeAmxRecordProofErrorV1 as Error;
        if self.completed {
            return Err(Error::Source("native AMX proof already completed"));
        }
        if self.height < 2 {
            return Err(Error::Source(
                "AMX proof requires a certified non-genesis execution",
            ));
        }
        if self.chain.is_none() {
            self.chain = Some(CertifiedChain::new(self.view)?);
        }
        if self.certified.is_none() {
            self.certified = Some(
                self.chain
                    .as_ref()
                    .expect("original native reader")
                    .certified(self.height)?,
            );
        }
        let certified = self.certified.as_ref().expect("verified original carrier");
        if self.read.is_none() {
            self.read = Some(
                NativeContextArchive::open_existing(
                    self.view.kura(),
                    self.budget.clone(),
                    self.view.kura().native_context_archive_max_bytes(),
                )?
                .read_job(self.height, certified.block_hash()),
            );
        }
        let read = self
            .read
            .as_mut()
            .expect("original namespace and descriptor");
        read.recheck_namespace()?;
        if self.bytes.is_none() {
            match read.poll()? {
                None => return Ok(NativeAmxRecordProofPollV1::Pending),
                Some(bytes) => {
                    self.bytes = Some(bytes);
                    // Yield the completed source before decoding/tree construction. This keeps
                    // acquisition independently resumable under the caller's next local budget.
                    return Ok(NativeAmxRecordProofPollV1::Pending);
                }
            }
        }
        let bytes = self.bytes.as_ref().expect("original complete frame");
        if self.projection.is_none() {
            let projection = match ordinary_writes::decode_projection(bytes, &self.budget) {
                Ok(projection) => projection,
                Err(cause) => {
                    #[cfg(all(test, sumeragi_core_mutation = "HC148"))]
                    {
                        self.bytes = None;
                    }
                    return Err(cause.into());
                }
            };
            if projection.carrier_height != self.height
                || projection.carrier_hash != certified.block_hash()
                || !projection.witness.belongs_to(&self.budget)
                || !projection.casting_bindings.belongs_to(&self.budget)
            {
                return Err(Error::Source(
                    "archive differs from the certified carrier or original pool",
                ));
            }
            let start = (projection.lane_payload.as_ptr() as usize)
                .checked_sub(bytes.as_slice().as_ptr() as usize)
                .ok_or(Error::Source("original lane range"))?;
            let end = start
                .checked_add(projection.lane_payload.len())
                .ok_or(Error::Source("original lane range"))?;
            if bytes.as_slice().get(start..end) != Some(projection.lane_payload) {
                return Err(Error::Source(
                    "lane field does not borrow the original archive frame",
                ));
            }
            self.projection = Some(Projection {
                lane_range: start..end,
                witness: projection.witness,
                _casting: projection.casting_bindings,
            });
        }
        let projection = self
            .projection
            .as_ref()
            .expect("complete prepaid original writes");
        let root = certified.commitment().execution.ordinary_writes_root;
        if !self.authenticated {
            let path = NativeLaneStateProof::from_witness(projection.witness.get(), &self.budget)?;
            if !path.verify(*self.view.network_id(), self.height, root)
                || !path.matches_state_payload(
                    *self.view.network_id(),
                    self.height,
                    &bytes.as_slice()[projection.lane_range.clone()],
                )?
            {
                return Err(Error::Source(
                    "archive differs from complete certified execution",
                ));
            }
            self.authenticated = true;
        }
        read.recheck_namespace()?;
        let certificate = certified
            .certificate()
            .ok_or(Error::Source("original carrier has no native certificate"))?;
        let proof = AllocatedAmxRecordProofV1::from_original_witness(
            certificate,
            projection.witness.get(),
            self.kind,
            self.transaction,
            root,
            &self.budget,
        )?;
        read.recheck_namespace()?;
        self.completed = true;
        Ok(NativeAmxRecordProofPollV1::Complete(proof))
    }

    /// Attempt completion without replacing the source on refusal.
    ///
    /// # Errors
    /// Original source/resource refusal. This borrowed job retains all completed fields,
    /// descriptor and partial prefix for the caller's genuine poll/complete retry.
    pub fn complete(
        &mut self,
    ) -> Result<Option<AllocatedAmxRecordProofV1>, NativeAmxRecordProofErrorV1> {
        loop {
            match self.poll()? {
                NativeAmxRecordProofPollV1::Pending => {}
                NativeAmxRecordProofPollV1::Complete(proof) => return Ok(proof),
            }
        }
    }
}
