//! Fallible reconstruction of the host INPUT cursor from the complete valid prefix.

use super::{IVM, Memory, VMError};

impl IVM {
    /// Reconstruct the cursor without publishing a partially observed prefix.
    /// Malformed envelopes stop the scan; local resource refusal aborts loading.
    pub(super) fn recompute_input_bump_from_memory(&mut self) -> Result<(), VMError> {
        let mut offset = 0_u64;
        loop {
            if offset + 7 > Memory::INPUT_SIZE {
                break;
            }
            let Some(header) = self.read_input_cursor_range(offset, 7)? else {
                break;
            };
            let length = u64::from(u32::from_be_bytes([
                header[3], header[4], header[5], header[6],
            ]));
            let total = 7_u64.saturating_add(length).saturating_add(32);
            if offset + total > Memory::INPUT_SIZE {
                break;
            }
            let Some(payload) = self.read_input_cursor_range(offset + 7, length)? else {
                break;
            };
            let Some(hash) = self.read_input_cursor_range(offset + 7 + length, 32)? else {
                break;
            };
            if iroha_crypto::Hash::new(payload).as_ref() != hash {
                break;
            }
            // Preserve the existing eight-byte host-slot convention, including
            // empty payloads and a final aligned slot at the region boundary.
            offset = (offset + total).next_multiple_of(8);
        }
        self.input_bump_next = offset;
        Ok(())
    }

    fn read_input_cursor_range(&self, offset: u64, len: u64) -> Result<Option<&[u8]>, VMError> {
        match self.memory.load_region(Memory::INPUT_START + offset, len) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.execution_deferral().is_some() => Err(error),
            Err(_) => Ok(None),
        }
    }
}
