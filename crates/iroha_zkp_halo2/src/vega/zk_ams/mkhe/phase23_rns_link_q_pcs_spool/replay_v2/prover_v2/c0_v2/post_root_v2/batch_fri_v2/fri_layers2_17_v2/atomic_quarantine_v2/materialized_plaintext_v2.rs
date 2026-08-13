//! Exact zeroizing plaintext owner admitted before caller-sink release.
use zeroize::{Zeroize, Zeroizing};
use super::*;
pub(super) struct AtomicProofQuarantinePlaintextV2 {
    bytes: Zeroizing<Vec<u8>>,
    exact_bytes: usize,
    written: usize,
    next_slot: u64,
    complete: bool,
}
impl AtomicProofQuarantinePlaintextV2 {
    fn new_exact_v2(exact_bytes: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        let exact_u64 = u64::try_from(exact_bytes)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if exact_u64 == 0 || exact_u64 > CANONICAL_QUARANTINE_MAX_EXACT_BYTES_V2 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_bytes)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != exact_bytes {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(exact_bytes, 0);
        Ok(Self {
            bytes: Zeroizing::new(bytes),
            exact_bytes,
            written: 0,
            next_slot: 0,
            complete: false,
        })
    }
    fn push_slot_v2(
        &mut self,
        slot: u64,
        slot_count: u64,
        chunk: &ConfidentialSpoolChunkV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if self.complete
            || slot != self.next_slot
            || slot >= slot_count
            || chunk.len_v1() != CANONICAL_QUARANTINE_CHUNK_BYTES_V2
            || chunk.as_slice_v1().len() != CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let remaining = self
            .exact_bytes
            .checked_sub(self.written)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let emit = remaining.min(CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize);
        if emit == 0 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let next_slot = slot
            .checked_add(1)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        let is_last = next_slot == slot_count;
        if (!is_last && emit != CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize)
            || (is_last && chunk.as_slice_v1()[emit..].iter().any(|byte| *byte != 0))
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let next = self
            .written
            .checked_add(emit)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if next > self.exact_bytes || is_last != (next == self.exact_bytes) {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        self.bytes[self.written..next].copy_from_slice(&chunk.as_slice_v1()[..emit]);
        self.written = next;
        self.next_slot = next_slot;
        Ok(())
    }
    fn finish_v2(mut self, slot_count: u64) -> Result<Self, ProverPrerequisiteErrorV2> {
        if self.complete
            || self.next_slot != slot_count
            || self.written != self.exact_bytes
            || self.bytes.len() != self.exact_bytes
            || self.bytes.capacity() != self.exact_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        self.complete = true;
        Ok(self)
    }
    pub(super) fn emit_into_sink_v2<S: BatchFriCanonicalProofSinkV2>(
        self,
        mut sink: S,
    ) -> Result<S::Output, ProverPrerequisiteErrorV2> {
        if !self.complete
            || self.exact_bytes == 0
            || self.written != self.exact_bytes
            || self.bytes.len() != self.exact_bytes
            || self.bytes.capacity() != self.exact_bytes
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        sink.begin_exact_v2(self.exact_bytes)?;
        sink.write_next_v2(self.bytes.as_slice())?;
        sink.finish_exact_v2()
    }
}
impl Drop for AtomicProofQuarantinePlaintextV2 {
    fn drop(&mut self) {
        self.bytes.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.bytes);
        #[cfg(test)]
        MATERIALIZED_PLAINTEXT_DROPS_V2.with(|drops| {
            debug_assert!(self.bytes.iter().all(|byte| *byte == 0));
            drops.set(drops.get().saturating_add(1));
        });
    }
}
impl AtomicProofQuarantineReadyV2 {
    pub(super) fn materialize_v2(
        mut self,
    ) -> Result<AtomicProofQuarantinePlaintextV2, ProverPrerequisiteErrorV2> {
        let permit = self
            .permit
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let mut snapshot = self
            .snapshot
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        self.validate_v2(&snapshot, &permit)?;
        let exact = usize::try_from(self.geometry.exact_bytes)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if quarantine_geometry_v2(exact)? != self.geometry {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        #[cfg(test)]
        if take_materialization_test_fault_v2(MaterializationTestFaultV2::Allocation) {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        let mut plaintext = AtomicProofQuarantinePlaintextV2::new_exact_v2(exact)?;
        for slot in 0..self.geometry.slot_count {
            #[cfg(test)]
            if take_materialization_test_fault_v2(MaterializationTestFaultV2::Read(slot)) {
                return Err(
                    iroha_confidential_spool::ConfidentialSpoolErrorV1::FileOperation {
                        operation: "injected-late-materialization-read",
                        kind: std::io::ErrorKind::Other,
                    }
                    .into(),
                );
            }
            let chunk = snapshot.read_slot_v1(slot, self.context_digest)?;
            #[cfg(test)]
            let mut chunk = chunk;
            #[cfg(test)]
            MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(|reads| {
                reads.set(reads.get().saturating_add(1));
            });
            #[cfg(test)]
            if take_materialization_test_fault_v2(MaterializationTestFaultV2::Authentication(slot))
            {
                return Err(
                    iroha_confidential_spool::ConfidentialSpoolErrorV1::Authentication.into(),
                );
            }
            #[cfg(test)]
            if take_materialization_test_fault_v2(MaterializationTestFaultV2::Unwind(slot)) {
                panic!("injected late materialization unwind");
            }
            #[cfg(test)]
            if take_materialization_test_fault_v2(MaterializationTestFaultV2::ShortChunk(slot)) {
                chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(
                    CANONICAL_QUARANTINE_CHUNK_BYTES_V2 - 1,
                )?;
            }
            #[cfg(test)]
            if take_materialization_test_fault_v2(MaterializationTestFaultV2::Padding(slot)) {
                let used = usize::try_from(
                    CANONICAL_QUARANTINE_CHUNK_BYTES_V2 - self.geometry.padding_bytes,
                )
                .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
                if used >= chunk.as_slice_v1().len() {
                    return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
                }
                chunk.as_mut_slice_v1()[used] = 1;
            }
            plaintext.push_slot_v2(slot, self.geometry.slot_count, &chunk)?;
        }
        let plaintext = plaintext.finish_v2(self.geometry.slot_count)?;
        drop(snapshot);
        drop(permit);
        Ok(plaintext)
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterializationTestFaultV2 {
    Allocation,
    Read(u64),
    Authentication(u64),
    Unwind(u64),
    ShortChunk(u64),
    Padding(u64),
}
#[cfg(test)]
std::thread_local! {
    static MATERIALIZATION_TEST_FAULT_V2: core::cell::Cell<Option<MaterializationTestFaultV2>> =
        const { core::cell::Cell::new(None) };
    static MATERIALIZED_PLAINTEXT_DROPS_V2: core::cell::Cell<usize> =
        const { core::cell::Cell::new(0) };
    static MATERIALIZED_AUTHENTICATED_SLOTS_V2: core::cell::Cell<usize> =
        const { core::cell::Cell::new(0) };
}
#[cfg(test)]
fn take_materialization_test_fault_v2(expected: MaterializationTestFaultV2) -> bool {
    MATERIALIZATION_TEST_FAULT_V2.with(|fault| {
        if fault.get() != Some(expected) {
            return false;
        }
        fault.set(None);
        true
    })
}
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
    #[derive(Default)]
    struct SinkCountsV2 {
        begin: AtomicUsize,
        write: AtomicUsize,
        finish: AtomicUsize,
    }
    struct CountingSinkV2 {
        counts: Arc<SinkCountsV2>,
    }
    impl BatchFriCanonicalProofSinkV2 for CountingSinkV2 {
        type Output = ();
        fn begin_exact_v2(&mut self, _: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            self.counts.begin.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn write_next_v2(&mut self, _: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            self.counts.write.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            self.counts.finish.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    fn stage_bytes_v2(bytes: &[u8]) -> AtomicProofQuarantineReadyV2 {
        let directory = tempfile::tempdir().unwrap();
        let sink =
            AtomicProofQuarantineSinkV2::create_in_v2(directory.path(), [0x71; 32], bytes.len())
                .unwrap();
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(sink, bytes.len()).unwrap();
        writer.write_v2(bytes).unwrap();
        writer.finish_v2().unwrap()
    }
    fn reset_test_state_v2(fault: MaterializationTestFaultV2) {
        MATERIALIZATION_TEST_FAULT_V2.with(|state| state.set(Some(fault)));
        MATERIALIZED_PLAINTEXT_DROPS_V2.with(|drops| drops.set(0));
        MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(|reads| reads.set(0));
    }
    fn assert_zero_sink_v2(counts: &SinkCountsV2) {
        assert_eq!(counts.begin.load(Ordering::SeqCst), 0);
        assert_eq!(counts.write.load(Ordering::SeqCst), 0);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 0);
    }
    fn release_with_fault_v2(
        bytes: &[u8],
        fault: MaterializationTestFaultV2,
    ) -> (
        Result<((), ()), ProverPrerequisiteErrorV2>,
        Arc<SinkCountsV2>,
    ) {
        let ready = stage_bytes_v2(bytes);
        let counts = Arc::new(SinkCountsV2::default());
        reset_test_state_v2(fault);
        let result = release_after_atomic_quarantine_operation_v2(
            CountingSinkV2 {
                counts: counts.clone(),
            },
            || Ok((ready, ())),
            AtomicProofQuarantineReadyV2::materialize_v2,
        );
        (result, counts)
    }
    #[test]
    fn late_read_and_authentication_failures_never_reach_caller_sink() {
        let bytes = vec![0x81; 2 * CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize + 1];
        for (fault, authenticated_slots) in [
            (MaterializationTestFaultV2::Read(2), 2),
            (MaterializationTestFaultV2::Authentication(2), 3),
        ] {
            let (result, counts) = release_with_fault_v2(&bytes, fault);
            assert!(result.is_err());
            assert_zero_sink_v2(&counts);
            assert_eq!(
                MATERIALIZED_PLAINTEXT_DROPS_V2.with(core::cell::Cell::get),
                1
            );
            assert_eq!(
                MATERIALIZATION_TEST_FAULT_V2.with(core::cell::Cell::get),
                None
            );
            assert_eq!(
                MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(core::cell::Cell::get),
                authenticated_slots
            );
        }
    }
    #[test]
    fn allocation_failure_happens_before_reads_or_caller_sink() {
        let (result, counts) =
            release_with_fault_v2(&[0x82; 17], MaterializationTestFaultV2::Allocation);
        assert_eq!(result, Err(ProverPrerequisiteErrorV2::Allocation));
        assert_zero_sink_v2(&counts);
        assert_eq!(
            MATERIALIZED_PLAINTEXT_DROPS_V2.with(core::cell::Cell::get),
            0
        );
        assert_eq!(
            MATERIALIZATION_TEST_FAULT_V2.with(core::cell::Cell::get),
            None
        );
        assert_eq!(
            MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(core::cell::Cell::get),
            0
        );
    }
    #[test]
    fn late_materialization_unwind_drops_plaintext_before_caller_sink() {
        let bytes = vec![0x83; 2 * CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize + 1];
        let ready = stage_bytes_v2(&bytes);
        let counts = Arc::new(SinkCountsV2::default());
        reset_test_state_v2(MaterializationTestFaultV2::Unwind(2));
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = release_after_atomic_quarantine_operation_v2(
                CountingSinkV2 {
                    counts: counts.clone(),
                },
                || Ok((ready, ())),
                AtomicProofQuarantineReadyV2::materialize_v2,
            );
        }));
        assert!(unwind.is_err());
        assert_zero_sink_v2(&counts);
        assert_eq!(
            MATERIALIZED_PLAINTEXT_DROPS_V2.with(core::cell::Cell::get),
            1
        );
        assert_eq!(
            MATERIALIZATION_TEST_FAULT_V2.with(core::cell::Cell::get),
            None
        );
        assert_eq!(
            MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(core::cell::Cell::get),
            3
        );
    }
    #[test]
    fn malformed_chunk_and_nonzero_padding_fail_before_caller_sink() {
        let short_bytes = vec![0x84; CANONICAL_QUARANTINE_CHUNK_BYTES_V2 as usize + 1];
        for fault in [
            MaterializationTestFaultV2::ShortChunk(1),
            MaterializationTestFaultV2::Padding(1),
        ] {
            let (result, counts) = release_with_fault_v2(&short_bytes, fault);
            assert!(result.is_err());
            assert_zero_sink_v2(&counts);
            assert_eq!(
                MATERIALIZED_PLAINTEXT_DROPS_V2.with(core::cell::Cell::get),
                1
            );
            assert_eq!(
                MATERIALIZATION_TEST_FAULT_V2.with(core::cell::Cell::get),
                None
            );
            assert_eq!(
                MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(core::cell::Cell::get),
                2
            );
        }
    }
    #[test]
    fn successful_materialized_owner_is_emitted_once_then_zeroized() {
        let ready = stage_bytes_v2(&[0x85; 17]);
        let counts = Arc::new(SinkCountsV2::default());
        MATERIALIZATION_TEST_FAULT_V2.with(|state| state.set(None));
        MATERIALIZED_PLAINTEXT_DROPS_V2.with(|drops| drops.set(0));
        MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(|reads| reads.set(0));
        release_after_atomic_quarantine_operation_v2(
            CountingSinkV2 {
                counts: counts.clone(),
            },
            || Ok((ready, ())),
            AtomicProofQuarantineReadyV2::materialize_v2,
        )
        .unwrap();
        assert_eq!(counts.begin.load(Ordering::SeqCst), 1);
        assert_eq!(counts.write.load(Ordering::SeqCst), 1);
        assert_eq!(counts.finish.load(Ordering::SeqCst), 1);
        assert_eq!(
            MATERIALIZED_AUTHENTICATED_SLOTS_V2.with(core::cell::Cell::get),
            1
        );
        assert_eq!(
            MATERIALIZED_PLAINTEXT_DROPS_V2.with(core::cell::Cell::get),
            1
        );
    }
    #[test]
    fn source_guards_require_full_materialization_before_caller_begin() {
        let source = include_str!("materialized_plaintext_v2.rs");
        let parent = include_str!("../atomic_quarantine_v2.rs");
        assert!(source.lines().count() <= 500);
        assert!(source.contains("bytes: Zeroizing<Vec<u8>>"));
        assert!(source.contains("try_reserve_exact(exact_bytes)"));
        assert!(source.contains("bytes.capacity() != exact_bytes"));
        assert!(source.contains("snapshot.read_slot_v1(slot, self.context_digest)?"));
        assert!(source.contains("plaintext.push_slot_v2("));
        assert!(source.contains("drop(snapshot);\n        drop(permit);"));
        let emit = source
            .split_once("pub(super) fn emit_into_sink_v2")
            .unwrap()
            .1
            .split_once("impl Drop for AtomicProofQuarantinePlaintextV2")
            .unwrap()
            .0;
        assert!(!emit.contains("read_slot_v1"));
        assert!(!emit.contains("try_reserve"));
        assert!(emit.find("begin_exact_v2").unwrap() < emit.find("write_next_v2").unwrap());
        assert!(!source.contains("impl Clone for AtomicProofQuarantinePlaintextV2"));
        let release = parent
            .split_once("fn release_after_atomic_quarantine_operation_v2")
            .unwrap()
            .1
            .split_once("pub(super) fn release_after_atomic_quarantine_v2")
            .unwrap()
            .0;
        assert!(
            release.find("materialize(quarantine)?").unwrap()
                < release.find("emit_into_sink_v2(sink)").unwrap()
        );
    }
}
