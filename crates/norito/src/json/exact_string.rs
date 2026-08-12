//! Exact-layout storage for measured JSON sequence and string decoding.

use super::Error;

pub(super) fn allocate<T>(length: usize) -> Result<Vec<T>, Error> {
    if length == 0 || core::mem::size_of::<T>() == 0 {
        return Ok(Vec::new());
    }
    let layout = std::alloc::Layout::array::<T>(length).map_err(|_| Error::AllocationFailed)?;
    // SAFETY: `layout` is non-zero and exact. The caller charges the complete
    // `length * size_of::<T>()` layout before entry; null is rejected before ownership.
    let allocation = unsafe { std::alloc::alloc(layout) };
    let allocation = core::ptr::NonNull::new(allocation).ok_or(Error::AllocationFailed)?;
    Ok(unsafe { Vec::from_raw_parts(allocation.as_ptr().cast::<T>(), 0, length) })
}
