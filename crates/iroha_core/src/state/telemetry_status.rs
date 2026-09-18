//! Bounded, generation-consistent journal witnesses for classified status.

use super::{BlockHeader, Hash, HashOf, State, is_stable_state_view_generation};
use iroha_config::parameters::actual::LaneRoutingPolicy;

/// Internal scheduling quantum; never a second memory pool or runtime mode.
pub(crate) const STATUS_CLASSIFICATION_BLOCKS: usize = 64;
const JOURNAL_DOMAIN: &[u8] = b"iroha:telemetry:classified-journal:v1\0";

/// One immutable applied target, including the routing policy from its generation.
#[derive(Clone, Debug)]
pub(crate) struct TelemetryStatusTarget {
    pub(crate) height: usize,
    pub(crate) tip: Option<HashOf<BlockHeader>>,
    pub(crate) routing_policy: LaneRoutingPolicy,
}

/// Fixed-size witness for one bounded suffix of the captured State journal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TelemetryJournalChunk {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) checkpoint: Option<HashOf<BlockHeader>>,
    pub(crate) tip: Option<HashOf<BlockHeader>>,
    pub(crate) digest: Hash,
}

/// Source failures never authorize partial or substituted classification.
#[derive(Clone, Copy, Debug, thiserror::Error)]
pub(crate) enum TelemetryStatusSourceError {
    #[error("State publication is busy")]
    Busy,
    #[error("captured State journal target changed")]
    TargetChanged,
    #[error("classified journal position is invalid")]
    InvalidPosition,
    #[error("journal witness encoding failed")]
    Encoding,
}

/// Stream an unambiguous domain, half-open zero-based range and ordered rows.
/// Each row contains its one-based fixed-width u64 height and 32-byte header hash.
pub(crate) fn write_telemetry_journal_prefix(
    writer: &mut dyn std::io::Write,
    start: usize,
    end: usize,
) -> std::io::Result<()> {
    let start = u64::try_from(start).map_err(std::io::Error::other)?;
    let end = u64::try_from(end).map_err(std::io::Error::other)?;
    writer.write_all(JOURNAL_DOMAIN)?;
    writer.write_all(&start.to_le_bytes())?;
    writer.write_all(&end.to_le_bytes())
}

/// Write one exact journal position; no native-endian or variable-width ambiguity.
pub(crate) fn write_telemetry_journal_row(
    writer: &mut dyn std::io::Write,
    height: usize,
    hash: HashOf<BlockHeader>,
) -> std::io::Result<()> {
    let height = u64::try_from(height).map_err(std::io::Error::other)?;
    writer.write_all(&height.to_le_bytes())?;
    writer.write_all(hash.as_ref())
}

impl State {
    /// Copy only one applied target and Nexus policy under the publication generation.
    pub(crate) fn telemetry_status_target(
        &self,
    ) -> Result<TelemetryStatusTarget, TelemetryStatusSourceError> {
        for _ in 0..super::DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS {
            let before = self.state_view_generation();
            if before % 2 != 0 {
                continue;
            }
            let Some(journal) = self.block_hashes.inner.try_read() else {
                return Err(TelemetryStatusSourceError::Busy);
            };
            let Some(nexus) = self.nexus.try_read() else {
                return Err(TelemetryStatusSourceError::Busy);
            };
            let hashes = journal.as_slice();
            let target = TelemetryStatusTarget {
                height: hashes.len(),
                tip: hashes.last().copied(),
                routing_policy: nexus.routing_policy.clone(),
            };
            let after = self.state_view_generation();
            drop(nexus);
            drop(journal);
            if is_stable_state_view_generation(before, after) {
                return Ok(target);
            }
        }
        Err(TelemetryStatusSourceError::Busy)
    }

    /// Witness at most 64 hashes while preserving the original captured target.
    /// No journal, world or transaction guard escapes this synchronous accessor.
    pub(crate) fn telemetry_journal_chunk(
        &self,
        target: &TelemetryStatusTarget,
        start: usize,
    ) -> Result<TelemetryJournalChunk, TelemetryStatusSourceError> {
        if start > target.height {
            return Err(TelemetryStatusSourceError::InvalidPosition);
        }
        let end = start
            .saturating_add(STATUS_CLASSIFICATION_BLOCKS)
            .min(target.height);
        for _ in 0..super::DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS {
            let before = self.state_view_generation();
            if before % 2 != 0 {
                continue;
            }
            let Some(journal) = self.block_hashes.inner.try_read() else {
                return Err(TelemetryStatusSourceError::Busy);
            };
            let hashes = journal.as_slice();
            if hashes.len() < target.height
                || target
                    .height
                    .checked_sub(1)
                    .and_then(|i| hashes.get(i))
                    .copied()
                    != target.tip
            {
                return Err(TelemetryStatusSourceError::TargetChanged);
            }
            let digest = Hash::new_from_writer(|writer| {
                write_telemetry_journal_prefix(writer, start, end)?;
                for (offset, hash) in hashes[start..end].iter().copied().enumerate() {
                    write_telemetry_journal_row(writer, start + offset + 1, hash)?;
                }
                Ok(())
            })
            .map_err(|_| TelemetryStatusSourceError::Encoding)?;
            let chunk = TelemetryJournalChunk {
                start,
                end,
                checkpoint: start.checked_sub(1).and_then(|i| hashes.get(i)).copied(),
                tip: end.checked_sub(1).and_then(|i| hashes.get(i)).copied(),
                digest,
            };
            let after = self.state_view_generation();
            drop(journal);
            if is_stable_state_view_generation(before, after) {
                return Ok(chunk);
            }
        }
        Err(TelemetryStatusSourceError::Busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::World};
    fn state() -> State {
        State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }
    fn hash(value: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new([value]))
    }
    fn append(state: &State, hashes: impl IntoIterator<Item = HashOf<BlockHeader>>) {
        let _publication = state.begin_state_view_write();
        let mut journal = state.block_hashes.block();
        for hash in hashes {
            journal.push(hash);
        }
        journal.commit();
    }
    #[tokio::test]
    async fn status_source_busy_is_distinct_from_changed_or_invalid_journal() {
        use crate::telemetry::StatusSnapshotError;

        let state = state();
        append(&state, [hash(1)]);
        let target = state.telemetry_status_target().expect("first target");
        let publication = state.begin_state_view_write();
        assert!(matches!(
            state
                .telemetry_status_target()
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateBusy)
        ));
        assert!(matches!(
            state
                .telemetry_journal_chunk(&target, 0)
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateBusy)
        ));
        drop(publication);
        let recovered = state
            .telemetry_status_target()
            .expect("publication released");
        assert_eq!(
            (recovered.height, recovered.tip),
            (target.height, target.tip)
        );
        assert!(state.telemetry_journal_chunk(&target, 0).is_ok());
        let journal_write = state.block_hashes.inner.write();
        assert!(matches!(
            state
                .telemetry_status_target()
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateBusy)
        ));
        assert!(matches!(
            state
                .telemetry_journal_chunk(&target, 0)
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateBusy)
        ));
        drop(journal_write);
        assert!(state.telemetry_journal_chunk(&target, 0).is_ok());
        assert!(matches!(
            state
                .telemetry_journal_chunk(&target, target.height + 1)
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateUnavailable)
        ));
        let mut journal = state.block_hashes.block_and_revert();
        journal.push(hash(2));
        journal.commit();
        assert!(matches!(
            state
                .telemetry_journal_chunk(&target, 0)
                .map_err(StatusSnapshotError::from),
            Err(StatusSnapshotError::StateUnavailable)
        ));
        assert!(matches!(
            StatusSnapshotError::from(TelemetryStatusSourceError::Encoding),
            StatusSnapshotError::StateUnavailable
        ));
    }

    #[tokio::test]
    async fn target_binds_one_even_publication_and_releases_all_source_guards() {
        let state = state();
        append(&state, [hash(1)]);
        let old = state.telemetry_status_target().expect("first target");
        let publication = state.begin_state_view_write();
        assert!(matches!(
            state.telemetry_status_target(),
            Err(TelemetryStatusSourceError::Busy)
        ));
        state.nexus.write().routing_policy.rules.push(
            iroha_config::parameters::actual::LaneRoutingRule {
                lane: old.routing_policy.default_lane,
                dataspace: Some(old.routing_policy.default_dataspace),
                matcher: iroha_config::parameters::actual::LaneRoutingMatcher {
                    account: None,
                    instruction: Some("Log".into()),
                    description: Some("published marker".into()),
                },
            },
        );
        let mut journal = state.block_hashes.block();
        journal.push(hash(2));
        journal.commit();
        drop(publication);
        let new = state.telemetry_status_target().expect("new target");
        assert_eq!((old.height, old.tip), (1, Some(hash(1))));
        assert!(old.routing_policy.rules.is_empty());
        assert_eq!((new.height, new.tip), (2, Some(hash(2))));
        assert_eq!(new.routing_policy.rules.len(), 1);
        assert!(state.block_hashes.inner.try_write().is_some());
        assert!(state.nexus.try_write().is_some());
    }
    #[tokio::test]
    async fn journal_chunks_are_bounded_and_bind_checkpoint_and_original_target() {
        let state = state();
        append(&state, (1..=70).map(hash));
        let target = state.telemetry_status_target().unwrap();
        let first = state.telemetry_journal_chunk(&target, 0).unwrap();
        assert_eq!(
            (first.start, first.end, first.checkpoint, first.tip),
            (0, 64, None, Some(hash(64)))
        );
        append(&state, [hash(71)]);
        let second = state.telemetry_journal_chunk(&target, 64).unwrap();
        assert_eq!(
            (second.start, second.end, second.checkpoint, second.tip),
            (64, 70, Some(hash(64)), Some(hash(70)))
        );
        assert_ne!(first.digest, second.digest);
        assert!(state.block_hashes.inner.try_write().is_some());
        assert!(matches!(
            state.telemetry_journal_chunk(&target, 71),
            Err(TelemetryStatusSourceError::InvalidPosition)
        ));
        let mut wrong = target.clone();
        wrong.tip = Some(hash(72));
        assert!(matches!(
            state.telemetry_journal_chunk(&wrong, 64),
            Err(TelemetryStatusSourceError::TargetChanged)
        ));
        let next_publication = std::sync::Arc::new(super::super::BlockHashPublication);
        let mut journal = state.block_hashes.inner.write();
        let super::super::BlockHashStorage::Owned {
            hashes,
            publication,
        } = &mut *journal
        else {
            panic!("owned fixture journal")
        };
        hashes[63] = hash(73);
        *publication = next_publication;
        drop(journal);
        let changed = state.telemetry_journal_chunk(&target, 64).unwrap();
        assert_ne!(changed.checkpoint, second.checkpoint);
    }
    #[test]
    fn journal_encoding_domain_range_height_order_and_hash_are_unambiguous() {
        let encode = |rows: &[(usize, HashOf<BlockHeader>)]| {
            let mut bytes = Vec::new();
            write_telemetry_journal_prefix(&mut bytes, 0, 2).unwrap();
            for (height, hash) in rows {
                write_telemetry_journal_row(&mut bytes, *height, *hash).unwrap();
            }
            bytes
        };
        let rows = [(1, hash(1)), (2, hash(2))];
        let bytes = encode(&rows);
        assert!(bytes.starts_with(JOURNAL_DOMAIN));
        assert_eq!(
            bytes.len(),
            JOURNAL_DOMAIN.len() + 16 + 2 * (8 + Hash::LENGTH)
        );
        assert_eq!(
            &bytes[JOURNAL_DOMAIN.len()..JOURNAL_DOMAIN.len() + 8],
            &0_u64.to_le_bytes()
        );
        assert_eq!(
            &bytes[JOURNAL_DOMAIN.len() + 8..JOURNAL_DOMAIN.len() + 16],
            &2_u64.to_le_bytes()
        );
        let digest = Hash::new(&bytes);
        for other in [
            [rows[1], rows[0]],
            [(2, hash(1)), (1, hash(2))],
            [(1, hash(1)), (2, hash(3))],
        ] {
            assert_ne!(digest, Hash::new(encode(&other)));
        }
        let mut other = Vec::new();
        write_telemetry_journal_prefix(&mut other, 1, 3).unwrap();
        for (height, hash) in rows {
            write_telemetry_journal_row(&mut other, height, hash).unwrap();
        }
        assert_ne!(digest, Hash::new(other));
    }
}
