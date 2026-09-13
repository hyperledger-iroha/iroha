//! Logical-entry reservations over complete ordered transcript bundles.
//!
//! Every entry hash owns E=1 across physical execution and fee fragments. Its
//! transcript counts and sole public statement are replaced from the complete
//! finalized bundle, never summed from independently prepared occurrences.
//! State still owns authenticated entry creation, WSV/source publication, and
//! matching rollback. This component neither enables quotas nor grants finality.

use iroha_crypto::Hash;
use iroha_data_model::fastpq::TransferTranscript;

use super::{
    BTreeMap, EntryOwner, OccurrenceSlot, OccurrenceUsage, ReservationCheckpoint,
    ReservationContext, ReservationError, ReservationInvariant, ReservationLedger,
    ReservationPolicy, ReservationTransaction, SourceDimension, SourceUsage,
};
use crate::fastpq::source_capture::{
    FastpqSourceStatementBuildLimits, FastpqSourceTranscriptUsage,
    measure_fastpq_source_entry_bundle_usage,
};

/// Failure before complete-entry replacement; every variant leaves usage intact.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EntryBundleReservationError {
    /// Canonical public preparation, input shape, or its explicit local bound failed.
    /// This must not be interpreted as ordinary transaction rejection or deferral.
    Preparation(String),
    /// The complete measured entry violates capacity or capability invariants.
    Reservation(ReservationError),
}

impl From<ReservationError> for EntryBundleReservationError {
    fn from(error: ReservationError) -> Self {
        Self::Reservation(error)
    }
}

impl std::fmt::Display for EntryBundleReservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preparation(error) => {
                write!(formatter, "FASTPQ complete-entry preparation: {error}")
            }
            Self::Reservation(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for EntryBundleReservationError {}

/// Opaque capability binding one logical execution identity to this block ledger.
#[derive(Clone, Debug)]
pub(crate) struct EntryBundleOwner {
    entry_hash: Hash,
    inner: EntryOwner,
}

/// Block accounting with one public-statement contribution per logical entry.
/// Explicit construction bounds limit preparation and do not select runtime policy.
pub(crate) struct EntryBundleReservationLedger {
    inner: ReservationLedger,
    entries: BTreeMap<Hash, EntryOwner>,
    construction_limits: FastpqSourceStatementBuildLimits,
}

impl EntryBundleReservationLedger {
    /// Create an empty ledger under explicit frozen context, ceilings and scratch bounds.
    pub(crate) fn new(
        context: ReservationContext,
        policy: ReservationPolicy,
        construction_limits: FastpqSourceStatementBuildLimits,
    ) -> Result<Self, ReservationError> {
        Ok(Self {
            inner: ReservationLedger::new(context, policy)?,
            entries: BTreeMap::new(),
            construction_limits,
        })
    }

    /// Start a physical scope; dropping it restores accounting and identity bindings.
    pub(crate) fn transaction(
        &mut self,
        context: ReservationContext,
    ) -> Result<EntryBundleReservationTransaction<'_>, ReservationError> {
        Ok(EntryBundleReservationTransaction {
            inner: self.inner.transaction(context)?,
            bindings: PendingBindings {
                entries: &mut self.entries,
                created: Vec::new(),
                committed: false,
            },
            construction_limits: self.construction_limits,
        })
    }

    /// Exact committed E/T/D/I/M/S usage.
    pub(crate) fn usage(&self) -> SourceUsage {
        self.inner.usage()
    }
}

struct PendingBindings<'a> {
    entries: &'a mut BTreeMap<Hash, EntryOwner>,
    created: Vec<Hash>,
    committed: bool,
}

impl PendingBindings<'_> {
    fn rollback_to(&mut self, offset: usize) {
        while self.created.len() > offset {
            if let Some(hash) = self.created.pop() {
                self.entries.remove(&hash);
            }
        }
    }
}

impl Drop for PendingBindings<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.rollback_to(0);
        }
    }
}

/// Exclusive physical fragment using the journaled reservation ledger.
pub(crate) struct EntryBundleReservationTransaction<'a> {
    inner: ReservationTransaction<'a>,
    bindings: PendingBindings<'a>,
    construction_limits: FastpqSourceStatementBuildLimits,
}

/// Journal checkpoint covering both usage and newly owned entry identities.
pub(crate) struct EntryBundleReservationCheckpoint {
    inner: ReservationCheckpoint,
    created_entries: usize,
}

fn occurrence_usage(
    measured: FastpqSourceTranscriptUsage,
) -> Result<OccurrenceUsage, ReservationError> {
    let empty = measured.transcripts == 0;
    if measured.max_statement_bytes != measured.total_statement_bytes
        || (empty
            && (measured.deltas != 0
                || measured.input_transcript_bytes != 0
                || measured.total_statement_bytes != 0))
        || (!empty
            && (measured.deltas < measured.transcripts
                || measured.input_transcript_bytes == 0
                || measured.total_statement_bytes == 0))
    {
        return Err(ReservationError::Invariant(
            ReservationInvariant::InvalidEntryBundleMeasurement,
        ));
    }
    let checked = |count: usize, dimension| {
        u64::try_from(count).map_err(|_| {
            ReservationError::Invariant(ReservationInvariant::UsageOverflow { dimension })
        })
    };
    Ok(OccurrenceUsage {
        transcripts: checked(measured.transcripts, SourceDimension::Transcripts)?,
        deltas: checked(measured.deltas, SourceDimension::Deltas)?,
        input_bytes: checked(
            measured.input_transcript_bytes,
            SourceDimension::InputTranscriptBytes,
        )?,
        statement_bytes: checked(
            measured.total_statement_bytes,
            SourceDimension::TotalStatementBytes,
        )?,
    })
}

impl EntryBundleReservationTransaction<'_> {
    /// Retain E=1 for this identity, including transcript-free/rejected entries.
    /// Reopening it in a fee or later physical fragment reuses the same owner.
    /// State must choose the identity from its owned complete execution archive.
    pub(crate) fn open_entry(
        &mut self,
        entry_hash: Hash,
    ) -> Result<EntryBundleOwner, ReservationError> {
        let inner = if let Some(owner) = self.bindings.entries.get(&entry_hash) {
            self.inner.validate_owner(owner)?;
            owner.clone()
        } else {
            let owner = self.inner.open_owner()?;
            self.bindings.entries.insert(entry_hash, owner.clone());
            self.bindings.created.push(entry_hash);
            owner
        };
        Ok(EntryBundleOwner { entry_hash, inner })
    }

    /// Atomically replace T/D/I and M=S from the complete finalized entry bundle.
    ///
    /// Empty input clears its source contribution while retaining E=1. This measures
    /// every original occurrence together, including repeated keys and common scales.
    /// The caller must publish this exact bundle and matching WSV changes atomically.
    /// No input is mutated, no private tree is built, and failure changes no accounting.
    pub(crate) fn replace_bundle(
        &mut self,
        owner: &EntryBundleOwner,
        bundle: &[TransferTranscript],
    ) -> Result<SourceUsage, EntryBundleReservationError> {
        self.inner.validate_owner(&owner.inner)?;
        let measured = measure_fastpq_source_entry_bundle_usage(
            &owner.entry_hash,
            bundle,
            self.construction_limits,
        )
        .map_err(EntryBundleReservationError::Preparation)?;
        let usage = occurrence_usage(measured)?;
        let state = self.inner.validate_owner(&owner.inner)?;
        // Only this wrapper can mint its owners. It keeps one replaceable slot;
        // slots and the raw occurrence API are never exposed by the wrapper.
        if let Some((&id, slot)) = state.slots.first_key_value() {
            let previous = OccurrenceSlot {
                owner: owner.inner.binding.clone(),
                id,
                generation: slot.generation,
            };
            self.inner.replace(&owner.inner, &previous, usage)?;
        } else {
            self.inner.reserve(&owner.inner, usage)?;
        }
        self.inner.owner_usage(&owner.inner).map_err(Into::into)
    }

    /// Exact usage for the complete logical entry across committed/staged fragments.
    pub(crate) fn owner_usage(
        &self,
        owner: &EntryBundleOwner,
    ) -> Result<SourceUsage, ReservationError> {
        self.inner.owner_usage(&owner.inner)
    }

    /// Exact block usage including this physical fragment's pending replacements.
    pub(crate) fn usage(&self) -> SourceUsage {
        self.inner.usage()
    }

    /// Save an ancestor-safe accounting and identity-binding boundary.
    pub(crate) fn checkpoint(&self) -> EntryBundleReservationCheckpoint {
        EntryBundleReservationCheckpoint {
            inner: self.inner.checkpoint(),
            created_entries: self.bindings.created.len(),
        }
    }

    /// Undo a retained boundary; invalid checkpoints leave both ledgers intact.
    pub(crate) fn rollback(
        &mut self,
        checkpoint: EntryBundleReservationCheckpoint,
    ) -> Result<SourceUsage, ReservationError> {
        let usage = self.inner.rollback(checkpoint.inner)?;
        self.bindings.rollback_to(checkpoint.created_entries);
        Ok(usage)
    }

    /// Retain usage and entry bindings with the matching State/source commit.
    pub(crate) fn commit(mut self) -> SourceUsage {
        self.bindings.committed = true;
        self.inner.commit()
    }
}

#[cfg(test)]
mod tests;
