//! Immutable journal values admitted before a geometry operation changes files.
//!
//! The existing caller retains prune, canonical-chain, geometry and sidecar
//! guards throughout this owner. This is not yet a reservation that may cross
//! canonical block durability: retirement admission and the journal predecessor
//! still need their own retained authority before that handoff is safe.

use super::retained_journal::RetainedGeometryJournal;
use super::*;

// Compare complete canonical encodings to establish these differences. This
// bound is independent of any assumed Norito enum offset or checksum layout.
const MAX_PHASE_CHANGED_BYTES: usize = 256;
const PHASES: [LaneGeometryPhase; 4] = [
    LaneGeometryPhase::Intent,
    LaneGeometryPhase::FilesApplied,
    LaneGeometryPhase::CatalogPublished,
    LaneGeometryPhase::RolledBack,
];

struct PhaseByteChange {
    offset: usize,
    before: u8,
    after: u8,
}

/// One exact encoding and bounded phase changes, with no mutable journal escape.
/// File operations and retries reuse the same admitted operations and buffer.
pub(super) struct PreparedGeometryJournalTransition {
    operations: Box<[LaneGeometryOperation]>,
    encoded: Box<[u8]>,
    encoded_phase: LaneGeometryPhase,
    changes: [Box<[PhaseByteChange]>; 4],
    writer: Option<Box<RetainedGeometryJournal>>,
}

impl PreparedGeometryJournalTransition {
    pub(super) fn prepare(
        kura: &Kura,
        journal: LaneGeometryJournal,
        record_index: usize,
    ) -> Result<Self> {
        // Authenticate retained checkpoint/evidence before any transition write.
        // Changing the selected phase below cannot introduce different evidence.
        kura.validate_lane_geometry_journal(&journal)?;
        let mut writer = Some(RetainedGeometryJournal::capture(
            kura,
            journal.encode().len(),
        )?);
        Self::prepare_with_retained_writer(kura, journal, record_index, &mut writer)
    }

    pub(super) fn prepare_with_retained_writer(
        kura: &Kura,
        journal: LaneGeometryJournal,
        record_index: usize,
        retained: &mut Option<RetainedGeometryJournal>,
    ) -> Result<Self> {
        kura.validate_lane_geometry_journal(&journal)?;
        let writer = retained.as_mut().ok_or_else(|| {
            kura.geometry_error(
                ErrorKind::InvalidData,
                "prepared geometry transition lost its retained descriptor",
            )
        })?;
        writer.prepare_next_write(journal.encode().len())?;
        let predecessor = match writer.predecessor() {
            Some(bytes) => {
                decode_exact::<LaneGeometryJournal>(bytes).map_err(Error::NoritoFrame)?
            }
            None => LaneGeometryJournal::default(),
        };
        // The observed file must be exactly this retry or this new record's
        // complete predecessor. Do not bless a different journal merely because
        // the requested transition remains individually well formed.
        if predecessor != journal {
            let mut prior = journal.clone();
            if record_index + 1 != prior.records.len() {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "prepared geometry journal differs from its retained predecessor",
                ));
            }
            prior.records.pop();
            if prior != predecessor {
                return Err(kura.geometry_error(
                    ErrorKind::InvalidData,
                    "prepared geometry journal differs from its retained predecessor",
                ));
            }
        }
        let mut prepared = Self::prepare_phases(
            &kura.store_root,
            journal,
            record_index,
            MAX_GEOMETRY_JOURNAL_BYTES,
        )?;
        prepared.writer = retained.take().map(Box::new);
        // TODO: admit the complete retained memory/descriptor owner before
        // voting. Report both buffers without shrinking the existing encoding
        // limit into a new route-dependent local acceptance rule.
        Ok(prepared)
    }

    pub(super) fn prepare_phases(
        root: &Path,
        mut journal: LaneGeometryJournal,
        record_index: usize,
        max_bytes: u64,
    ) -> Result<Self> {
        let record = journal.records.get_mut(record_index).ok_or_else(|| {
            lane_geometry_journal_structure_error(
                root,
                ErrorKind::InvalidInput,
                "prepared lane geometry journal has no selected transition",
            )
        })?;
        record.phase = LaneGeometryPhase::Intent;
        validate_lane_geometry_journal_structure(root, &journal)?;
        let encoded = journal.encode().into_boxed_slice();
        let capacity_error = || {
            lane_geometry_journal_structure_error(
                root,
                ErrorKind::InvalidInput,
                "prepared lane geometry journal exceeds its aggregate retained byte limit",
            )
        };
        if u64::try_from(encoded.len()).unwrap_or(u64::MAX) > max_bytes {
            return Err(capacity_error());
        }
        let mut changes = std::array::from_fn(|_| Box::<[PhaseByteChange]>::default());
        for (phase_index, phase) in PHASES.iter().enumerate().skip(1) {
            journal.records[record_index].phase = *phase;
            validate_lane_geometry_journal_structure(root, &journal)?;
            // At most one comparison encoding exists temporarily. Drop it at
            // each iteration; persistence never allocates a full-size copy.
            let phase_bytes = journal.encode();
            if phase_bytes.len() != encoded.len() {
                return Err(lane_geometry_journal_structure_error(
                    root,
                    ErrorKind::InvalidData,
                    "canonical geometry phase encodings have different lengths",
                ));
            }
            let mut delta = Vec::new();
            for (offset, (&before, &after)) in encoded.iter().zip(&phase_bytes).enumerate() {
                if before != after {
                    if delta.len() == MAX_PHASE_CHANGED_BYTES {
                        return Err(lane_geometry_journal_structure_error(
                            root,
                            ErrorKind::InvalidData,
                            "canonical geometry phase differences exceed their admitted bound",
                        ));
                    }
                    delta.push(PhaseByteChange {
                        offset,
                        before,
                        after,
                    });
                }
            }
            changes[phase_index] = delta.into_boxed_slice();
        }
        // The remaining decoded journal is not retained. Only the exact file
        // operations accompany the one encoding and its small verified deltas.
        let operations =
            std::mem::take(&mut journal.records[record_index].operations).into_boxed_slice();
        let prepared = Self {
            operations,
            encoded,
            encoded_phase: LaneGeometryPhase::Intent,
            changes,
            writer: None,
        };
        if prepared
            .retained_allocation_bytes()
            .is_none_or(|bytes| bytes > max_bytes)
        {
            return Err(capacity_error());
        }
        Ok(prepared)
    }

    /// Account for every allocation this owner retains, including operation paths.
    pub(super) fn retained_allocation_bytes(&self) -> Option<u64> {
        let mut bytes = std::mem::size_of::<Self>().checked_add(self.encoded.len())?;
        bytes = bytes.checked_add(
            self.operations
                .len()
                .checked_mul(std::mem::size_of::<LaneGeometryOperation>())?,
        )?;
        for changes in &self.changes {
            bytes = bytes.checked_add(
                changes
                    .len()
                    .checked_mul(std::mem::size_of::<PhaseByteChange>())?,
            )?;
        }
        if let Some(writer) = &self.writer {
            bytes = bytes.checked_add(writer.retained_allocation_bytes()?)?;
        }
        for operation in &self.operations {
            for path in [
                &operation.archived_blocks_path,
                &operation.archived_merge_path,
                &operation.unpublished_blocks_path,
                &operation.unpublished_merge_path,
            ] {
                bytes = bytes.checked_add(path.capacity())?;
            }
            for binding in operation.previous.iter().chain(operation.updated.iter()) {
                bytes = bytes.checked_add(binding.blocks_path.capacity())?;
                bytes = bytes.checked_add(binding.merge_path.capacity())?;
            }
        }
        u64::try_from(bytes).ok()
    }

    pub(super) fn operations(&self) -> &[LaneGeometryOperation] {
        &self.operations
    }

    fn phase_index(phase: LaneGeometryPhase) -> usize {
        match phase {
            LaneGeometryPhase::Intent => 0,
            LaneGeometryPhase::FilesApplied => 1,
            LaneGeometryPhase::CatalogPublished => 2,
            LaneGeometryPhase::RolledBack => 3,
        }
    }

    pub(super) fn bytes(&mut self, phase: LaneGeometryPhase) -> &[u8] {
        if self.encoded_phase != phase {
            for change in &self.changes[Self::phase_index(self.encoded_phase)] {
                self.encoded[change.offset] = change.before;
            }
            for change in &self.changes[Self::phase_index(phase)] {
                self.encoded[change.offset] = change.after;
            }
            self.encoded_phase = phase;
        }
        &self.encoded
    }

    pub(super) fn persist(&mut self, kura: &Kura, phase: LaneGeometryPhase) -> Result<()> {
        self.bytes(phase);
        let writer = self.writer.as_mut().ok_or_else(|| {
            kura.geometry_error(
                ErrorKind::InvalidInput,
                "geometry phase bytes have no retained physical journal owner",
            )
        })?;
        writer.persist(kura, phase, &self.encoded)
    }

    /// Recheck completed original phase custody without encoding or writing again.
    pub(super) fn reauthenticate_completed(
        &self,
        kura: &Kura,
        phase: LaneGeometryPhase,
    ) -> Result<()> {
        if self.encoded_phase != phase {
            return Err(kura.geometry_error(
                ErrorKind::InvalidInput,
                "prepared geometry completion differs from its retained phase",
            ));
        }
        self.writer
            .as_ref()
            .ok_or_else(|| {
                kura.geometry_error(
                    ErrorKind::InvalidInput,
                    "prepared geometry completion lost its original journal writer",
                )
            })?
            .reauthenticate_completed(kura, phase, &self.encoded)
    }
}
