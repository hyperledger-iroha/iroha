//! Original governed verifier registry pair, separate from local artifact availability.
//!
//! This owner captures canonical authority independently of the local artifact cache.
//! It does not authenticate Kura history, issue a root, or supply a publication permit.

use crate::state::{State, is_stable_state_view_generation};
use iroha_data_model::kagemusha::KagemushaGovernedVerifierRegistryV1;
use mv::{PublicationPreparationError, cell::CommittedCellBorrow};
use std::convert::Infallible;

/// Original source acquisition and complete registry shape refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::state) enum RegistrySourceError {
    /// The exact native current/undo or publication owner was unavailable.
    Original(PublicationPreparationError<Infallible>),
    /// The actual current or retained predecessor registry failed validation.
    Invalid {
        predecessor: bool,
        reason: &'static str,
    },
}

/// Complete original current and undo authority while its native writers stay held.
/// Local RejectAll/loaded/stale artifacts cannot change either canonical source.
pub(in crate::state) struct GovernedRegistrySource<'state> {
    state: &'state State,
    generation: u64,
    pair: CommittedCellBorrow<'state, KagemushaGovernedVerifierRegistryV1>,
}

impl<'state> GovernedRegistrySource<'state> {
    /// Retain and validate only the original State registry under a bounded native read.
    /// The registry validator itself enforces V1 release/policy cardinality ceilings.
    pub(in crate::state) fn try_capture(
        state: &'state State,
    ) -> Result<Option<Self>, RegistrySourceError> {
        let generation = state.state_view_generation();
        if generation & 1 != 0 {
            return Ok(None);
        }
        let pair = state
            .world
            .kagemusha_verifier_registry
            .try_committed_borrow()
            .map_err(RegistrySourceError::Original)?;
        pair.current()
            .validate()
            .map_err(|reason| RegistrySourceError::Invalid {
                predecessor: false,
                reason,
            })?;
        if let Some(original) = pair.undo() {
            original
                .validate()
                .map_err(|reason| RegistrySourceError::Invalid {
                    predecessor: true,
                    reason,
                })?;
        }
        let source = Self {
            state,
            generation,
            pair,
        };
        if !source.try_matches_current()? {
            return Ok(None);
        }
        Ok(Some(source))
    }
    /// Borrow the complete current governed policy and every release authority.
    pub(in crate::state) fn current(&self) -> &KagemushaGovernedVerifierRegistryV1 {
        self.pair.current()
    }
    /// Borrow exact native undo, preserving absence rather than inventing an empty registry.
    pub(in crate::state) fn predecessor(&self) -> Option<&KagemushaGovernedVerifierRegistryV1> {
        self.pair.undo().as_ref()
    }
    /// Check the retained original publication and enclosing State generation.
    pub(in crate::state) fn try_matches_current(&self) -> Result<bool, RegistrySourceError> {
        if !is_stable_state_view_generation(self.generation, self.state.state_view_generation()) {
            return Ok(false);
        }
        if !self
            .pair
            .try_matches_current()
            .map_err(RegistrySourceError::Original)?
        {
            return Ok(false);
        }
        Ok(is_stable_state_view_generation(
            self.generation,
            self.state.state_view_generation(),
        ))
    }
}

// TODO: consume the canonical registry from the original frozen publication journal
// with every other canonical cell/table, atomic predecessor custody and authenticated
// native recovery. Static inventory admission does not satisfy these owner gates.

use iroha_allocation::{AllocationRefusal, ChargedBuffer, ChargedBufferError};
use mv::cell::CommittedCellObservation;

/// Independent encoded-byte ceilings, checked before either buffer allocation.
#[derive(Clone, Copy)]
pub(in crate::state) struct RegistryCaptureLimits {
    pub(in crate::state) max_current_bytes: usize,
    pub(in crate::state) max_predecessor_bytes: usize,
    pub(in crate::state) max_total_bytes: usize,
}

/// Original resource or encoding refusal without allocated diagnostic text.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::state) enum RegistryCaptureError {
    Source(RegistrySourceError),
    ByteLimit,
    Admission(AllocationRefusal),
    Allocation,
    Encoding,
}

/// Charged exact native registry frames with their original publication identity.
/// The native writers have already released. This is one cell owner, not State finality.
pub(in crate::state) struct CapturedGovernedRegistry<'state> {
    state: &'state State,
    generation: u64,
    original: CommittedCellObservation<'state, KagemushaGovernedVerifierRegistryV1>,
    current: ChargedBuffer<u8>,
    predecessor: Option<ChargedBuffer<u8>>,
}

fn allocation_error(error: ChargedBufferError) -> RegistryCaptureError {
    match error {
        ChargedBufferError::Admission(refusal) => RegistryCaptureError::Admission(refusal),
        ChargedBufferError::Allocator { .. } => RegistryCaptureError::Allocation,
    }
}

fn encode_original(
    value: &KagemushaGovernedVerifierRegistryV1,
    length: usize,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<ChargedBuffer<u8>, RegistryCaptureError> {
    struct Writer<'a>(&'a mut ChargedBuffer<u8>);
    impl std::io::Write for Writer<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.append(bytes)?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = ChargedBuffer::new(length, budget).map_err(allocation_error)?;
    norito::core::write_canonical_to_writer(value, &mut Writer(&mut bytes))
        .map_err(|_| RegistryCaptureError::Encoding)?;
    if bytes.as_slice().len() != length {
        return Err(RegistryCaptureError::Encoding);
    }
    Ok(bytes)
}

impl<'state> CapturedGovernedRegistry<'state> {
    /// Capture from the original State and execution pool with no caller-supplied source.
    /// Every byte buffer remains charged after the physical guards release.
    pub(in crate::state) fn try_capture(
        state: &'state State,
        limits: RegistryCaptureLimits,
    ) -> Result<Option<Self>, RegistryCaptureError> {
        let budget = state.ivm_execution_budget();
        let Some(source) =
            GovernedRegistrySource::try_capture(state).map_err(RegistryCaptureError::Source)?
        else {
            return Ok(None);
        };
        let current_len = norito::canonical_frame_len(source.current())
            .map_err(|_| RegistryCaptureError::Encoding)?;
        let predecessor_len = source
            .predecessor()
            .map(norito::canonical_frame_len)
            .transpose()
            .map_err(|_| RegistryCaptureError::Encoding)?;
        let total = current_len
            .checked_add(predecessor_len.unwrap_or(0))
            .ok_or(RegistryCaptureError::ByteLimit)?;
        if current_len > limits.max_current_bytes
            || predecessor_len.is_some_and(|length| length > limits.max_predecessor_bytes)
            || total > limits.max_total_bytes
        {
            return Err(RegistryCaptureError::ByteLimit);
        }
        let current = encode_original(source.current(), current_len, &budget)?;
        let predecessor = source
            .predecessor()
            .zip(predecessor_len)
            .map(|(value, len)| encode_original(value, len, &budget))
            .transpose()?;
        if !source
            .try_matches_current()
            .map_err(RegistryCaptureError::Source)?
        {
            return Ok(None);
        }
        let generation = source.generation;
        let original = source
            .pair
            .release_observation()
            .map_err(|e| RegistryCaptureError::Source(RegistrySourceError::Original(e)))?;
        let captured = Self {
            state,
            generation,
            original,
            current,
            predecessor,
        };
        if !captured
            .try_matches_current()
            .map_err(RegistryCaptureError::Source)?
        {
            return Ok(None);
        }
        Ok(Some(captured))
    }
    /// Exact canonical current frame, independent of local runtime availability.
    pub(in crate::state) fn current_bytes(&self) -> &[u8] {
        self.current.as_slice()
    }
    /// Exact canonical retained predecessor, preserving absent undo as None.
    pub(in crate::state) fn predecessor_bytes(&self) -> Option<&[u8]> {
        self.predecessor.as_ref().map(ChargedBuffer::as_slice)
    }
    /// Recheck the original cell and State generation before a future aggregate consumes it.
    pub(in crate::state) fn try_matches_current(&self) -> Result<bool, RegistrySourceError> {
        if !is_stable_state_view_generation(self.generation, self.state.state_view_generation()) {
            return Ok(false);
        }
        if !self
            .original
            .try_matches_current()
            .map_err(RegistrySourceError::Original)?
        {
            return Ok(false);
        }
        Ok(is_stable_state_view_generation(
            self.generation,
            self.state.state_view_generation(),
        ))
    }
}
