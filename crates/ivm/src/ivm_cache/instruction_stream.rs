//! Borrowed canonical instruction geometry for allocation-free inspection.

use super::DecodedOp;

/// A validated immutable instruction slice whose iteration needs no decoded storage.
#[derive(Clone, Copy)]
pub struct ValidatedInstructions<'a> {
    code: &'a [u8],
}

impl<'a> ValidatedInstructions<'a> {
    /// Validate with the same fixed-width decoder as execution preparation.
    ///
    /// # Errors
    /// Returns the original stream-boundary or decoder error before any consumer runs.
    pub fn new(code: &'a [u8]) -> Result<Self, crate::VMError> {
        if (code.len() as u64) > crate::memory::Memory::HEAP_START {
            return Err(crate::VMError::MemoryOutOfBounds);
        }
        for index in 0..code.len().div_ceil(4) {
            crate::decoder::decode_slice(code, (index as u64) * 4)?;
        }
        Ok(Self::from_admitted(code))
    }

    /// Borrow the executable region of a contract already admitted and prepared under its profile.
    pub(crate) fn from_admitted(code: &'a [u8]) -> Self {
        debug_assert!(code.len().is_multiple_of(4));
        Self { code }
    }

    /// Return the word index of an aligned executable-relative PC inside the stream.
    pub(crate) fn index_of(self, pc: u64) -> Option<usize> {
        let offset = usize::try_from(pc).ok()?;
        (offset < self.code.len() && offset.is_multiple_of(4)).then_some(offset / 4)
    }

    /// Read one admitted word by index.
    pub(crate) fn get(self, index: usize) -> Option<DecodedOp> {
        let bytes = self.code.as_chunks::<4>().0.get(index)?;
        Some(DecodedOp {
            pc: index as u64 * 4,
            inst: u32::from_le_bytes(*bytes),
        })
    }

    /// Number of complete admitted words.
    #[must_use]
    pub fn len(self) -> usize {
        self.code.len() / 4
    }

    /// Whether the admitted stream contains no instructions.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.code.is_empty()
    }

    /// Read the immutable admitted words in executable-relative address order.
    pub fn iter(self) -> impl ExactSizeIterator<Item = DecodedOp> + Clone + 'a {
        self.code
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(index, bytes)| DecodedOp {
                pc: (index as u64) * 4,
                inst: u32::from_le_bytes(*bytes),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_random_access_agrees_with_iteration_and_refuses_non_boundaries() {
        let words = [0x11223344_u32, 0x88776655];
        let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        let stream = ValidatedInstructions::from_admitted(&bytes);
        for (index, op) in stream.iter().enumerate() {
            assert_eq!(stream.index_of(op.pc), Some(index));
            assert_eq!(stream.get(index), Some(op));
        }
        for pc in [1, 7, 8, u64::MAX] {
            assert_eq!(stream.index_of(pc), None);
        }
        assert_eq!(stream.get(2), None);
        assert_eq!(stream.get(usize::MAX), None);
    }

    #[test]
    fn borrowed_scan_matches_decoded_storage_and_all_boundary_failures() {
        let words = [crate::encoding::wide::encode_halt(), 0xff00ff00, 0];
        let code: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        for len in 0..=code.len() {
            let original = &code[..len];
            let scanned = ValidatedInstructions::new(original);
            let decoded = super::super::IvmCache::decode_stream(original);
            match (scanned, decoded) {
                (Ok(scan), Ok(decoded)) => {
                    assert_eq!(scan.code.as_ptr(), original.as_ptr());
                    assert_eq!(scan.len(), decoded.len());
                    assert_eq!(scan.is_empty(), decoded.is_empty());
                    assert!(scan.iter().eq(decoded.iter().copied()));
                }
                (
                    Err(crate::VMError::MemoryAccessViolation { addr: a, perm: p }),
                    Err(crate::VMError::MemoryAccessViolation { addr: b, perm: q }),
                ) => {
                    assert_eq!((a, p), (b, q));
                    assert_eq!(
                        (a, p),
                        (((len / 4) * 4) as u32, crate::error::Perm::EXECUTE)
                    );
                }
                _ => panic!("borrowed and owned decoding disagree at {len}"),
            }
        }
    }
}
