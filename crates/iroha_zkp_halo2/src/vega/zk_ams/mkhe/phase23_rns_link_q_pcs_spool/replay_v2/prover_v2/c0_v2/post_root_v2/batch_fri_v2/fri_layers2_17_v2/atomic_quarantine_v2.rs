//! Purpose-bound confidential quarantine for atomic canonical-proof release.

use std::path::Path;

use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use super::*;

#[path = "atomic_quarantine_v2/materialized_plaintext_v2.rs"]
mod materialized_plaintext_v2;
use materialized_plaintext_v2::AtomicProofQuarantinePlaintextV2;

const QUARANTINE_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.qpcs.canonical-proof-atomic-quarantine\0";
const QUARANTINE_REPLAY_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.qpcs.canonical-proof-atomic-quarantine.replay-permit\0";
const CANONICAL_QUARANTINE_CHUNK_BYTES_V2: u64 = 16_384;
const CANONICAL_QUARANTINE_TAG_BYTES_V2: u64 = 16;
const CANONICAL_QUARANTINE_MAX_EXACT_BYTES_V2: u64 = 29_245_792;
const CANONICAL_QUARANTINE_KAT_EXACT_BYTES_V2: u64 = 27_196_704;
const CANONICAL_QUARANTINE_KAT_SLOTS_V2: u64 = 1_660;
const CANONICAL_QUARANTINE_KAT_AUTHENTICATED_READS_V2: u64 = 3_320;
const CANONICAL_QUARANTINE_KAT_PADDING_BYTES_V2: u64 = 736;
const CANONICAL_QUARANTINE_KAT_FILE_BYTES_V2: u64 = 27_224_000;
pub(super) const CANONICAL_QUARANTINE_KAT_IO_BYTES_V2: u64 = 81_672_000;
const CANONICAL_QUARANTINE_MAX_SLOTS_V2: u64 = 1_786;
const CANONICAL_QUARANTINE_MAX_AUTHENTICATED_READS_V2: u64 = 3_572;
const CANONICAL_QUARANTINE_SNAPSHOT_HASHED_RECORDS_V2: [u64; 2] = [1_660, 1_786];
const CANONICAL_QUARANTINE_MAX_PADDING_BYTES_V2: u64 = 16_032;
pub(super) const CANONICAL_QUARANTINE_MAX_FILE_BYTES_V2: u64 = 29_290_400;
pub(super) const CANONICAL_QUARANTINE_MAX_IO_BYTES_V2: u64 = 87_871_200;
const CANONICAL_QUARANTINE_READ_AAD_HEAP_BYTES_V2: usize = 209;
const CANONICAL_QUARANTINE_KEY_HEAP_BYTES_V2: usize = 32;
pub(super) const CANONICAL_QUARANTINE_HEAP_BYTES_V2: usize = 29_262_417;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AtomicProofQuarantineGeometryV2 {
    exact_bytes: u64,
    slot_count: u64,
    plaintext_capacity: u64,
    file_bytes: u64,
    padding_bytes: u64,
}

struct AtomicProofQuarantineWriteLiveV2 {
    writer: ConfidentialSpoolWriterV1,
    chunk: Option<ConfidentialSpoolChunkV1>,
    geometry: AtomicProofQuarantineGeometryV2,
    context_digest: [u8; 32],
    written: u64,
    next_slot: u64,
    chunk_used: usize,
}

pub(super) struct AtomicProofQuarantineSinkV2 {
    master_binding: [u8; 32],
    live: Option<AtomicProofQuarantineWriteLiveV2>,
    begun: bool,
}

struct AtomicProofQuarantineReplayPermitV2 {
    digest: [u8; 32],
}

pub(super) struct AtomicProofQuarantineReadyV2 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    permit: Option<AtomicProofQuarantineReplayPermitV2>,
    geometry: AtomicProofQuarantineGeometryV2,
    master_binding: [u8; 32],
    context_digest: [u8; 32],
    snapshot_digest: [u8; 32],
}

fn quarantine_geometry_v2(
    exact_bytes: usize,
) -> Result<AtomicProofQuarantineGeometryV2, ProverPrerequisiteErrorV2> {
    let exact_bytes =
        u64::try_from(exact_bytes).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    if exact_bytes == 0 || exact_bytes > CANONICAL_QUARANTINE_MAX_EXACT_BYTES_V2 {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let slot_count = exact_bytes
        .checked_add(CANONICAL_QUARANTINE_CHUNK_BYTES_V2 - 1)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?
        / CANONICAL_QUARANTINE_CHUNK_BYTES_V2;
    let plaintext_capacity = slot_count
        .checked_mul(CANONICAL_QUARANTINE_CHUNK_BYTES_V2)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let file_bytes = slot_count
        .checked_mul(CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    let padding_bytes = plaintext_capacity
        .checked_sub(exact_bytes)
        .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
    Ok(AtomicProofQuarantineGeometryV2 {
        exact_bytes,
        slot_count,
        plaintext_capacity,
        file_bytes,
        padding_bytes,
    })
}

fn quarantine_context_digest_v2(
    master_binding: [u8; 32],
    geometry: AtomicProofQuarantineGeometryV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if master_binding == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut hash = Keccak256::new();
    hash.update(QUARANTINE_CONTEXT_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&master_binding);
    hash.update(&geometry.exact_bytes.to_be_bytes());
    hash.update(&geometry.slot_count.to_be_bytes());
    hash.update(&CANONICAL_QUARANTINE_CHUNK_BYTES_V2.to_be_bytes());
    hash.update(&geometry.plaintext_capacity.to_be_bytes());
    hash.update(&geometry.file_bytes.to_be_bytes());
    hash.update(&geometry.padding_bytes.to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    Ok(digest)
}

fn quarantine_replay_digest_v2(
    master_binding: [u8; 32],
    context_digest: [u8; 32],
    snapshot_digest: [u8; 32],
    geometry: AtomicProofQuarantineGeometryV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if master_binding == [0; 32] || context_digest == [0; 32] || snapshot_digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut hash = Keccak256::new();
    hash.update(QUARANTINE_REPLAY_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&master_binding);
    hash.update(&context_digest);
    hash.update(&snapshot_digest);
    hash.update(&geometry.exact_bytes.to_be_bytes());
    hash.update(&geometry.slot_count.to_be_bytes());
    hash.update(&CANONICAL_QUARANTINE_CHUNK_BYTES_V2.to_be_bytes());
    hash.update(&geometry.file_bytes.to_be_bytes());
    hash.update(&geometry.padding_bytes.to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    Ok(digest)
}

impl AtomicProofQuarantineSinkV2 {
    pub(super) fn create_in_v2(
        directory: &Path,
        master_binding: [u8; 32],
        exact_bytes: usize,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let geometry = quarantine_geometry_v2(exact_bytes)?;
        let context_digest = quarantine_context_digest_v2(master_binding, geometry)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            geometry.slot_count,
            CANONICAL_QUARANTINE_CHUNK_BYTES_V2,
            context_digest,
        )?;
        let writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)?;
        let chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(CANONICAL_QUARANTINE_CHUNK_BYTES_V2)?;
        Ok(Self {
            master_binding,
            live: Some(AtomicProofQuarantineWriteLiveV2 {
                writer,
                chunk: Some(chunk),
                geometry,
                context_digest,
                written: 0,
                next_slot: 0,
                chunk_used: 0,
            }),
            begun: false,
        })
    }
}

impl BatchFriCanonicalProofSinkV2 for AtomicProofQuarantineSinkV2 {
    type Output = AtomicProofQuarantineReadyV2;

    fn begin_exact_v2(&mut self, exact_bytes: usize) -> Result<(), ProverPrerequisiteErrorV2> {
        let live = self
            .live
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let exact_bytes = u64::try_from(exact_bytes)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if self.begun || exact_bytes != live.geometry.exact_bytes {
            return Err(ProverPrerequisiteErrorV2::Poisoned);
        }
        self.begun = true;
        Ok(())
    }

    fn write_next_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        if !self.begun {
            return Err(ProverPrerequisiteErrorV2::Poisoned);
        }
        let live = self
            .live
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let next = live
            .written
            .checked_add(
                u64::try_from(bytes.len())
                    .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if bytes.is_empty() || next > live.geometry.exact_bytes {
            return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
        }
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let mut offset = 0;
        while offset < bytes.len() {
            let available = CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize - live.chunk_used;
            let take = available.min(bytes.len() - offset);
            let chunk = live
                .chunk
                .as_mut()
                .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
            chunk.as_mut_slice_v1()[live.chunk_used..live.chunk_used + take]
                .copy_from_slice(&bytes[offset..offset + take]);
            live.chunk_used += take;
            live.written = live
                .written
                .checked_add(
                    u64::try_from(take)
                        .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
                )
                .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            offset += take;
            if live.chunk_used == CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize {
                let chunk = live
                    .chunk
                    .take()
                    .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
                live.writer.write_slot_v1(live.next_slot, chunk)?;
                live.next_slot = live
                    .next_slot
                    .checked_add(1)
                    .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
                live.chunk_used = 0;
                if live.written < live.geometry.exact_bytes {
                    live.chunk = Some(ConfidentialSpoolChunkV1::new_zeroed_v1(
                        CANONICAL_QUARANTINE_CHUNK_BYTES_V2,
                    )?);
                }
            }
        }
        self.live = Some(live);
        Ok(())
    }

    fn finish_exact_v2(mut self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
        if !self.begun {
            return Err(ProverPrerequisiteErrorV2::Poisoned);
        }
        let mut live = self
            .live
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if live.written != live.geometry.exact_bytes {
            return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
        }
        if let Some(chunk) = live.chunk.take() {
            if live.chunk_used == 0 || live.next_slot + 1 != live.geometry.slot_count {
                return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
            }
            live.writer.write_slot_v1(live.next_slot, chunk)?;
            live.next_slot += 1;
            live.chunk_used = 0;
        }
        if live.next_slot != live.geometry.slot_count || live.chunk_used != 0 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let snapshot = live.writer.seal_v1()?;
        if snapshot.slot_count_v1() != live.geometry.slot_count
            || snapshot.plaintext_len_v1() != CANONICAL_QUARANTINE_CHUNK_BYTES_V2
            || snapshot.ciphertext_record_len_v1()
                != CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2
            || snapshot.file_len_v1() != live.geometry.file_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let snapshot_digest = *snapshot.snapshot_digest_v1();
        let replay_digest = quarantine_replay_digest_v2(
            self.master_binding,
            live.context_digest,
            snapshot_digest,
            live.geometry,
        )?;
        Ok(AtomicProofQuarantineReadyV2 {
            snapshot: Some(snapshot),
            permit: Some(AtomicProofQuarantineReplayPermitV2 {
                digest: replay_digest,
            }),
            geometry: live.geometry,
            master_binding: self.master_binding,
            context_digest: live.context_digest,
            snapshot_digest,
        })
    }
}

impl AtomicProofQuarantineReadyV2 {
    fn validate_v2(
        &self,
        snapshot: &ConfidentialSpoolSnapshotV1,
        permit: &AtomicProofQuarantineReplayPermitV2,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if snapshot.slot_count_v1() != self.geometry.slot_count
            || snapshot.plaintext_len_v1() != CANONICAL_QUARANTINE_CHUNK_BYTES_V2
            || snapshot.ciphertext_record_len_v1()
                != CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2
            || snapshot.file_len_v1() != self.geometry.file_bytes
            || *snapshot.snapshot_digest_v1() != self.snapshot_digest
            || quarantine_context_digest_v2(self.master_binding, self.geometry)?
                != self.context_digest
            || quarantine_replay_digest_v2(
                self.master_binding,
                self.context_digest,
                self.snapshot_digest,
                self.geometry,
            )? != permit.digest
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        Ok(())
    }
}

fn release_after_atomic_quarantine_operation_v2<S, T>(
    sink: S,
    stage: impl FnOnce() -> Result<(AtomicProofQuarantineReadyV2, T), ProverPrerequisiteErrorV2>,
    materialize: impl FnOnce(
        AtomicProofQuarantineReadyV2,
    ) -> Result<AtomicProofQuarantinePlaintextV2, ProverPrerequisiteErrorV2>,
) -> Result<(S::Output, T), ProverPrerequisiteErrorV2>
where
    S: BatchFriCanonicalProofSinkV2,
{
    let (quarantine, retained) = stage()?;
    let plaintext = materialize(quarantine)?;
    Ok((plaintext.emit_into_sink_v2(sink)?, retained))
}

pub(super) fn release_after_atomic_quarantine_v2<S, T>(
    sink: S,
    stage: impl FnOnce() -> Result<(AtomicProofQuarantineReadyV2, T), ProverPrerequisiteErrorV2>,
) -> Result<(S::Output, T), ProverPrerequisiteErrorV2>
where
    S: BatchFriCanonicalProofSinkV2,
{
    release_after_atomic_quarantine_operation_v2(
        sink,
        stage,
        AtomicProofQuarantineReadyV2::materialize_v2,
    )
}

const _: () = {
    assert!(
        CANONICAL_QUARANTINE_KAT_AUTHENTICATED_READS_V2
            == CANONICAL_QUARANTINE_SNAPSHOT_HASHED_RECORDS_V2[0]
                + CANONICAL_QUARANTINE_KAT_SLOTS_V2
    );
    assert!(
        CANONICAL_QUARANTINE_KAT_SLOTS_V2 * CANONICAL_QUARANTINE_CHUNK_BYTES_V2
            == CANONICAL_QUARANTINE_KAT_EXACT_BYTES_V2 + CANONICAL_QUARANTINE_KAT_PADDING_BYTES_V2
    );
    assert!(
        CANONICAL_QUARANTINE_KAT_FILE_BYTES_V2
            == CANONICAL_QUARANTINE_KAT_SLOTS_V2
                * (CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2)
    );
    assert!(
        CANONICAL_QUARANTINE_KAT_IO_BYTES_V2
            == (CANONICAL_QUARANTINE_KAT_SLOTS_V2
                + CANONICAL_QUARANTINE_KAT_AUTHENTICATED_READS_V2)
                * (CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2)
    );
    assert!(
        CANONICAL_QUARANTINE_MAX_SLOTS_V2 * CANONICAL_QUARANTINE_CHUNK_BYTES_V2
            == CANONICAL_QUARANTINE_MAX_EXACT_BYTES_V2 + CANONICAL_QUARANTINE_MAX_PADDING_BYTES_V2
    );
    assert!(
        CANONICAL_QUARANTINE_MAX_AUTHENTICATED_READS_V2
            == CANONICAL_QUARANTINE_SNAPSHOT_HASHED_RECORDS_V2[1]
                + CANONICAL_QUARANTINE_MAX_SLOTS_V2
    );
    assert!(
        CANONICAL_QUARANTINE_MAX_FILE_BYTES_V2
            == CANONICAL_QUARANTINE_MAX_SLOTS_V2
                * (CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2)
    );
    assert!(
        CANONICAL_QUARANTINE_MAX_IO_BYTES_V2
            == (CANONICAL_QUARANTINE_MAX_SLOTS_V2
                + CANONICAL_QUARANTINE_MAX_AUTHENTICATED_READS_V2)
                * (CANONICAL_QUARANTINE_CHUNK_BYTES_V2 + CANONICAL_QUARANTINE_TAG_BYTES_V2)
    );
    assert!(
        CANONICAL_QUARANTINE_HEAP_BYTES_V2 as u64
            == CANONICAL_QUARANTINE_MAX_EXACT_BYTES_V2
                + CANONICAL_QUARANTINE_CHUNK_BYTES_V2
                + CANONICAL_QUARANTINE_READ_AAD_HEAP_BYTES_V2 as u64
                + CANONICAL_QUARANTINE_KEY_HEAP_BYTES_V2 as u64
    );
};

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;

    #[derive(Clone, Copy)]
    enum FailureV2 {
        None,
        Begin,
        Write,
        Finish,
        PanicWrite,
    }

    #[derive(Default)]
    struct SinkCountsV2 {
        begin: AtomicUsize,
        write: AtomicUsize,
        finish: AtomicUsize,
    }

    struct CountingSinkV2 {
        counts: Arc<SinkCountsV2>,
        failure: FailureV2,
        expected: usize,
        bytes: Vec<u8>,
    }

    impl CountingSinkV2 {
        fn new_v2(counts: Arc<SinkCountsV2>, failure: FailureV2) -> Self {
            Self {
                counts,
                failure,
                expected: 0,
                bytes: Vec::new(),
            }
        }
    }

    impl BatchFriCanonicalProofSinkV2 for CountingSinkV2 {
        type Output = Vec<u8>;

        fn begin_exact_v2(&mut self, exact_bytes: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            self.counts.begin.fetch_add(1, Ordering::SeqCst);
            if matches!(self.failure, FailureV2::Begin) {
                return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
            }
            self.expected = exact_bytes;
            Ok(())
        }

        fn write_next_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            self.counts.write.fetch_add(1, Ordering::SeqCst);
            match self.failure {
                FailureV2::Write => {
                    return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
                }
                FailureV2::PanicWrite => panic!("intentional caller-sink unwind"),
                _ => {}
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }

        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            self.counts.finish.fetch_add(1, Ordering::SeqCst);
            if matches!(self.failure, FailureV2::Finish) || self.bytes.len() != self.expected {
                return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
            }
            Ok(self.bytes)
        }
    }

    fn stage_bytes_v2(bytes: &[u8]) -> AtomicProofQuarantineReadyV2 {
        let directory = crate::testing::TestDirectory::new("atomic-proof-quarantine");
        let sink =
            AtomicProofQuarantineSinkV2::create_in_v2(directory.path(), [0x31; 32], bytes.len())
                .unwrap();
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(sink, bytes.len()).unwrap();
        writer.write_v2(bytes).unwrap();
        writer.finish_v2().unwrap()
    }

    #[test]
    fn transparent_kat_context_and_replay_permit_frames_are_frozen() {
        let geometry =
            quarantine_geometry_v2(CANONICAL_QUARANTINE_KAT_EXACT_BYTES_V2 as usize).unwrap();
        let context = quarantine_context_digest_v2([0x31; 32], geometry).unwrap();
        assert_eq!(
            context,
            [
                0xf6, 0xbe, 0x3c, 0xdd, 0x34, 0x43, 0x00, 0xa9, 0x64, 0xa0, 0xcb, 0xbc, 0x7e, 0x79,
                0x23, 0x2c, 0x8c, 0xf5, 0x23, 0x76, 0x2b, 0x3d, 0xc4, 0x1b, 0xee, 0x18, 0x00, 0x6a,
                0x33, 0x4d, 0xec, 0x30,
            ]
        );
        assert_eq!(
            quarantine_replay_digest_v2([0x31; 32], context, [0x32; 32], geometry).unwrap(),
            [
                0x9d, 0xc6, 0xe4, 0xa6, 0x66, 0x68, 0x56, 0xc4, 0x76, 0xd1, 0x6e, 0x69, 0x8d, 0x14,
                0x8f, 0x2c, 0x48, 0x5a, 0xdb, 0x9c, 0x14, 0xc8, 0xfb, 0x69, 0x43, 0x0a, 0x6a, 0x4b,
                0xb4, 0xea, 0x07, 0xcf,
            ]
        );
    }

    #[test]
    fn exact_authenticated_quarantine_replays_once_in_order() {
        let bytes = vec![0x51; CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize + 1];
        let ready = stage_bytes_v2(&bytes);
        let counts = Arc::new(SinkCountsV2::default());
        let (output, ()) = release_after_atomic_quarantine_v2(
            CountingSinkV2::new_v2(counts.clone(), FailureV2::None),
            || Ok((ready, ())),
        )
        .unwrap();
        assert_eq!(output, bytes);
        assert_eq!(counts.begin.load(Ordering::SeqCst), 1);
        assert_eq!(counts.write.load(Ordering::SeqCst), 1);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn bad_permit_and_pre_release_error_have_zero_caller_sink_effects() {
        let mut ready = stage_bytes_v2(&[0x52; 17]);
        ready.permit.as_mut().unwrap().digest[0] ^= 1;
        let counts = Arc::new(SinkCountsV2::default());
        let result: Result<(Vec<u8>, ()), _> = release_after_atomic_quarantine_v2(
            CountingSinkV2::new_v2(counts.clone(), FailureV2::None),
            || Ok((ready, ())),
        );
        assert!(result.is_err());
        assert_eq!(counts.begin.load(Ordering::SeqCst), 0);
        assert_eq!(counts.write.load(Ordering::SeqCst), 0);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 0);
        let counts = Arc::new(SinkCountsV2::default());
        let result: Result<(Vec<u8>, ()), _> = release_after_atomic_quarantine_v2(
            CountingSinkV2::new_v2(counts.clone(), FailureV2::None),
            || {
                let _fully_staged = stage_bytes_v2(&[0x55; 17]);
                Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot)
            },
        );
        assert!(result.is_err());
        assert_eq!(counts.begin.load(Ordering::SeqCst), 0);
        assert_eq!(counts.write.load(Ordering::SeqCst), 0);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn staging_unwind_cannot_reach_the_caller_sink() {
        let counts = Arc::new(SinkCountsV2::default());
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(Vec<u8>, ()), _> = release_after_atomic_quarantine_v2(
                CountingSinkV2::new_v2(counts.clone(), FailureV2::None),
                || {
                    let _fully_staged = stage_bytes_v2(&[0x56; 17]);
                    panic!("intentional proof-staging unwind")
                },
            );
        }));
        assert!(unwind.is_err());
        assert_eq!(counts.begin.load(Ordering::SeqCst), 0);
        assert_eq!(counts.write.load(Ordering::SeqCst), 0);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn caller_sink_failures_are_one_shot_after_validation() {
        for failure in [FailureV2::Begin, FailureV2::Write, FailureV2::Finish] {
            let ready = stage_bytes_v2(&[0x53; 17]);
            let counts = Arc::new(SinkCountsV2::default());
            let result: Result<(Vec<u8>, ()), _> = release_after_atomic_quarantine_v2(
                CountingSinkV2::new_v2(counts.clone(), failure),
                || Ok((ready, ())),
            );
            assert!(result.is_err());
            assert_eq!(counts.begin.load(Ordering::SeqCst), 1);
            assert_eq!(
                counts.write.load(Ordering::SeqCst),
                if matches!(failure, FailureV2::Begin) {
                    0
                } else {
                    1
                }
            );
            assert_eq!(
                counts.finish.load(Ordering::SeqCst),
                if matches!(failure, FailureV2::Finish) {
                    1
                } else {
                    0
                }
            );
        }
        let ready = stage_bytes_v2(&[0x54; 17]);
        let counts = Arc::new(SinkCountsV2::default());
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(Vec<u8>, ()), _> = release_after_atomic_quarantine_v2(
                CountingSinkV2::new_v2(counts.clone(), FailureV2::PanicWrite),
                || Ok((ready, ())),
            );
        }));
        assert!(unwind.is_err());
        assert_eq!(counts.begin.load(Ordering::SeqCst), 1);
        assert_eq!(counts.write.load(Ordering::SeqCst), 1);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 0);
    }
}
