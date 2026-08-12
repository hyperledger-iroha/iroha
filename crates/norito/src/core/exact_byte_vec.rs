// Exact-layout destination used by the counted bounded frame encoder.

fn exact_byte_vec_with_capacity(bytes: usize) -> Result<Vec<u8>, BoundedEncodeError> {
    if bytes == 0 {
        return Ok(Vec::new());
    }
    let layout = std::alloc::Layout::array::<u8>(bytes)
        .map_err(|_| BoundedEncodeError::AllocationFailed { bytes })?;
    // SAFETY: `layout` is non-zero and describes exactly `bytes` byte slots.
    // Null is rejected before ownership; `Vec` deallocates the same layout.
    let allocation = unsafe { std::alloc::alloc(layout) };
    let allocation = core::ptr::NonNull::new(allocation)
        .ok_or(BoundedEncodeError::AllocationFailed { bytes })?;
    // SAFETY: the allocation owns `bytes` uninitialized byte slots.
    Ok(unsafe { Vec::from_raw_parts(allocation.as_ptr(), 0, bytes) })
}
