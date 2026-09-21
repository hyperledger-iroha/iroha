//! Prepaid fixed backing storage with no escaping uncharged allocation.
//!
//! Only the byte allocation is covered. Decoded objects, scratch storage and
//! budget control storage remain obligations of the integrating caller.

use std::alloc::Layout;

use super::{AllocationBudget, AllocationCharge, AllocationRefusal};

/// A fixed byte allocation retaining its original prepaid charge until deallocation.
///
/// The backing allocation cannot be extracted, replaced or grown. Bounded appends
/// initialize bytes before exposing them. Field drop order frees the backing Vec
/// before returning its credits. This owner accounts only its requested layout;
/// callers must separately admit any nested objects, scratch or other storage.
pub struct ChargedByteBuffer {
    bytes: Vec<u8>,
    _charge: AllocationCharge,
}

/// Local admission or allocator failure before byte storage is constructed.
#[derive(Debug)]
pub enum ChargedByteBufferError {
    /// Preserve the original finite pool's refusal and release observation.
    Admission(AllocationRefusal),
    /// The global allocator could not supply an already admitted layout.
    Allocator {
        /// Exact requested backing allocation size.
        requested_bytes: usize,
    },
}

impl std::fmt::Display for ChargedByteBufferError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(refusal) => refusal.fmt(formatter),
            Self::Allocator { requested_bytes } => write!(
                formatter,
                "failed to allocate {requested_bytes} admitted byte-buffer bytes"
            ),
        }
    }
}

impl std::error::Error for ChargedByteBufferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(refusal) => Some(refusal),
            Self::Allocator { .. } => None,
        }
    }
}

impl From<AllocationRefusal> for ChargedByteBufferError {
    fn from(refusal: AllocationRefusal) -> Self {
        Self::Admission(refusal)
    }
}

impl ChargedByteBuffer {
    /// Admit the exact backing layout before requesting it from the allocator.
    ///
    /// Zero capacity requests no backing allocation. On admission or allocator
    /// refusal, no allocation owner escapes and unused credits are returned.
    ///
    /// # Errors
    /// Returns the original pool refusal for overflow, oversize or capacity
    /// exhaustion, or an allocator failure for an admitted nonzero layout.
    pub fn new(capacity: usize, budget: &AllocationBudget) -> Result<Self, ChargedByteBufferError> {
        let layout =
            Layout::array::<u8>(capacity).map_err(|_| AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve(layout)?;
        let charge = reservation
            .try_split(layout)
            .expect("the exact backing layout was reserved above");
        let bytes = if capacity == 0 {
            Vec::new()
        } else {
            // SAFETY: layout is nonzero and checked above. The original credit
            // already covers exactly this global allocator request. A null
            // result installs no allocation and drops the unused charge.
            let pointer = unsafe { std::alloc::alloc(layout) };
            if pointer.is_null() {
                return Err(ChargedByteBufferError::Allocator {
                    requested_bytes: capacity,
                });
            }
            // SAFETY: pointer came from the global allocator with precisely
            // Layout::array::<u8>(capacity). Length zero exposes no uninitialized
            // bytes. Vec owns that same allocation and deallocates its original
            // layout. The only mutation below checks the fixed capacity first.
            unsafe { Vec::from_raw_parts(pointer, 0, capacity) }
        };
        Ok(Self {
            bytes,
            _charge: charge,
        })
    }

    /// Borrow initialized bytes without separating allocation and charge.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Fill already allocated capacity; never grow or replace the backing Vec.
    ///
    /// # Errors
    /// Returns [`std::io::ErrorKind::InvalidInput`] if the append would exceed
    /// the fixed capacity. Existing bytes and allocation ownership are unchanged.
    pub fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if bytes.len() > self.bytes.capacity() - self.bytes.len() {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}
