//! Exact canonical prefix codec and poisoned streaming sink wrapper.
use super::*;
const CANONICAL_HEADER_BYTES_V2: usize = 512;
const CANONICAL_EVALUATION_BYTES_V2: usize = 3_040;
const CANONICAL_TERMINAL_BYTES_V2: usize = 12_160;
const CANONICAL_FIXED_PREFIX_BYTES_V2: usize = 16_320;
pub(in super::super::super::super) trait BatchFriCanonicalProofSinkV2:
    Sized
{
    type Output;
    fn begin_exact_v2(&mut self, exact_bytes: usize) -> Result<(), ProverPrerequisiteErrorV2>;
    fn write_next_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2>;
    fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2>;
}
pub(super) struct CanonicalProofSinkWriterV2<S: BatchFriCanonicalProofSinkV2> {
    sink: Option<S>,
    exact_bytes: usize,
    written: usize,
}
impl<S: BatchFriCanonicalProofSinkV2> CanonicalProofSinkWriterV2<S> {
    pub(super) fn begin_v2(sink: S, exact_bytes: usize) -> Result<Self, ProverPrerequisiteErrorV2> {
        if exact_bytes == 0 {
            return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
        }
        let mut writer = Self {
            sink: Some(sink),
            exact_bytes,
            written: 0,
        };
        let mut sink = writer
            .sink
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        sink.begin_exact_v2(exact_bytes)?;
        writer.sink = Some(sink);
        Ok(writer)
    }
    pub(super) fn write_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut sink = self
            .sink
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        let next = self
            .written
            .checked_add(bytes.len())
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if bytes.is_empty() || next > self.exact_bytes {
            return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
        }
        sink.write_next_v2(bytes)?;
        self.written = next;
        self.sink = Some(sink);
        Ok(())
    }
    pub(super) fn finish_v2(mut self) -> Result<S::Output, ProverPrerequisiteErrorV2> {
        let sink = self
            .sink
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        if self.written != self.exact_bytes {
            return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
        }
        sink.finish_exact_v2()
    }
    #[cfg(test)]
    const fn written_v2(&self) -> usize {
        self.written
    }
}
fn canonical_header_v2(
    parameter_digest: [u8; 32],
    context: PublicSpoolContextV2,
    initial_root: [u8; 32],
) -> Result<[u8; CANONICAL_HEADER_BYTES_V2], ProverPrerequisiteErrorV2> {
    context.validate_v2()?;
    if parameter_digest == [0; 32] || initial_root == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut header = [0_u8; CANONICAL_HEADER_BYTES_V2];
    header[..16].copy_from_slice(b"IROHA-QPCSV2\0\0\0\0");
    header[16..24].copy_from_slice(&[2, 17, 19, 38, 5, 10, 18, 2]);
    header[24..28].copy_from_slice(&131_072_u32.to_be_bytes());
    header[28..32].copy_from_slice(&524_288_u32.to_be_bytes());
    header[32..34].copy_from_slice(&160_u16.to_be_bytes());
    header[34..36].copy_from_slice(&320_u16.to_be_bytes());
    header[36..40].copy_from_slice(&4_028_u32.to_be_bytes());
    header[40..44].copy_from_slice(&3_392_u32.to_be_bytes());
    header[44..48].copy_from_slice(&20_030_u32.to_be_bytes());
    header[48..52].copy_from_slice(&6_080_u32.to_be_bytes());
    header[52..56].copy_from_slice(&16_u32.to_be_bytes());
    header[56..64].copy_from_slice(&29_245_792_u64.to_be_bytes());
    header[64..96].copy_from_slice(&parameter_digest);
    header[96..128].copy_from_slice(&context.sealed_source_transcript_digest);
    header[128..160].copy_from_slice(&context.source_algebra_binding_digest);
    header[160..192].copy_from_slice(&initial_root);
    Ok(header)
}
pub(super) fn write_canonical_prefix_v2<S: BatchFriCanonicalProofSinkV2>(
    writer: &mut CanonicalProofSinkWriterV2<S>,
    binding: &CanonicalProofReplayBindingV2,
    evaluations: &[u8; CANONICAL_EVALUATION_BYTES_V2],
    terminal: &[u8; CANONICAL_TERMINAL_BYTES_V2],
) -> Result<(), ProverPrerequisiteErrorV2> {
    if binding.terminal_digest != keccak256(terminal) {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let header = canonical_header_v2(
        binding.parameter_digest,
        binding.context,
        binding.initial_root,
    )?;
    writer.write_v2(&header)?;
    writer.write_v2(evaluations)?;
    writer.write_v2(&binding.quotient_root)?;
    for root in binding.fri_roots {
        writer.write_v2(&root)?;
    }
    writer.write_v2(terminal)?;
    Ok(())
}
const _: () = {
    assert!(
        CANONICAL_FIXED_PREFIX_BYTES_V2
            == CANONICAL_HEADER_BYTES_V2
                + CANONICAL_EVALUATION_BYTES_V2
                + 32
                + 18 * 32
                + CANONICAL_TERMINAL_BYTES_V2
    );
};
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::atomic::{AtomicUsize, Ordering},
    };
    static PANIC_SINK_DROPS_V2: AtomicUsize = AtomicUsize::new(0);
    struct HashSinkV2 {
        hash: Keccak256,
        expected: usize,
        written: usize,
    }
    impl BatchFriCanonicalProofSinkV2 for HashSinkV2 {
        type Output = [u8; 32];
        fn begin_exact_v2(&mut self, exact_bytes: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            self.expected = exact_bytes;
            Ok(())
        }
        fn write_next_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            self.hash.update(bytes);
            self.written += bytes.len();
            Ok(())
        }
        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            if self.written != self.expected {
                return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
            }
            Ok(self.hash.finalize())
        }
    }
    struct PanicSinkV2;
    struct ErrorSinkV2;
    impl BatchFriCanonicalProofSinkV2 for ErrorSinkV2 {
        type Output = ();
        fn begin_exact_v2(&mut self, _: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            Ok(())
        }
        fn write_next_v2(&mut self, _: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            Err(ProverPrerequisiteErrorV2::CanonicalProofSink)
        }
        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            Ok(())
        }
    }
    impl Drop for PanicSinkV2 {
        fn drop(&mut self) {
            PANIC_SINK_DROPS_V2.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl BatchFriCanonicalProofSinkV2 for PanicSinkV2 {
        type Output = ();
        fn begin_exact_v2(&mut self, _: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            Ok(())
        }
        fn write_next_v2(&mut self, _: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            panic!("intentional canonical sink unwind")
        }
        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            Ok(())
        }
    }
    #[test]
    fn synthetic_header_and_prefix_kats_pin_the_actual_codec() {
        let context = PublicSpoolContextV2 {
            sealed_source_transcript_digest: [0x12; 32],
            source_algebra_binding_digest: [0x13; 32],
        };
        let header = canonical_header_v2([0x11; 32], context, [0x14; 32]).unwrap();
        assert_eq!(
            keccak256(&header),
            [
                0xa4, 0xab, 0x3e, 0x9f, 0x8a, 0x03, 0x26, 0x75, 0x50, 0xda, 0xc9, 0x56, 0xed, 0x60,
                0xb0, 0x0a, 0xd6, 0xdd, 0x18, 0xe1, 0x83, 0x4b, 0x60, 0x9f, 0xbe, 0x4e, 0x45, 0x86,
                0x22, 0x28, 0xd7, 0x41,
            ]
        );
        let binding = CanonicalProofReplayBindingV2 {
            parameter_digest: [0x11; 32],
            context,
            initial_root: [0x14; 32],
            quotient_root: [0x15; 32],
            fri_roots: core::array::from_fn(|index| [0x20 + index as u8; 32]),
            terminal_digest: keccak256(&[0x51; 12_160]),
        };
        let sink = HashSinkV2 {
            hash: Keccak256::new(),
            expected: 0,
            written: 0,
        };
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(sink, 16_320).unwrap();
        write_canonical_prefix_v2(&mut writer, &binding, &[0x50; 3_040], &[0x51; 12_160]).unwrap();
        assert_eq!(writer.written_v2(), 16_320);
        assert_eq!(
            writer.finish_v2().unwrap(),
            [
                0x6b, 0x1f, 0x2b, 0x99, 0xb7, 0x8f, 0xd9, 0x6b, 0xaa, 0xa3, 0x37, 0xde, 0xc3, 0x7d,
                0xa7, 0x00, 0xc5, 0xa6, 0xd1, 0xf5, 0x14, 0xe0, 0x72, 0xbd, 0x28, 0xef, 0xb4, 0x5d,
                0x8e, 0xaf, 0xab, 0x7,
            ]
        );
    }
    #[test]
    fn sink_error_or_panic_poisoning_never_restores_the_taken_sink() {
        let mut error_writer = CanonicalProofSinkWriterV2::begin_v2(ErrorSinkV2, 1).unwrap();
        assert!(matches!(
            error_writer.write_v2(&[1]),
            Err(ProverPrerequisiteErrorV2::CanonicalProofSink)
        ));
        assert!(matches!(
            error_writer.write_v2(&[1]),
            Err(ProverPrerequisiteErrorV2::Poisoned)
        ));
        PANIC_SINK_DROPS_V2.store(0, Ordering::SeqCst);
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(PanicSinkV2, 1).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| writer.write_v2(&[1]))).is_err());
        assert_eq!(PANIC_SINK_DROPS_V2.load(Ordering::SeqCst), 1);
        assert!(matches!(
            writer.write_v2(&[1]),
            Err(ProverPrerequisiteErrorV2::Poisoned)
        ));
    }
}
