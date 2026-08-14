// Fallible exact retained storage for owned strings decoded from Norito.
#[allow(unsafe_code)]
fn try_copy_string_for_decode(bytes: &[u8]) -> Result<String, Error> {
    std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
    if bytes.is_empty() {
        return Ok(String::new());
    }
    let layout = Layout::array::<u8>(bytes.len()).map_err(|_| Error::LengthMismatch)?;
    // The length reader already charged these retained bytes. SAFETY: the
    // layout describes exactly `bytes.len()` non-zero bytes.
    let allocation = unsafe { std::alloc::alloc(layout) };
    if allocation.is_null() {
        return Err(Error::AllocationFailed {
            bytes: limit_to_u64(bytes.len()),
        });
    }
    // SAFETY: source and destination are valid, exact, non-overlapping buffers.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), allocation, bytes.len()) };
    let slice = std::ptr::slice_from_raw_parts_mut(allocation, bytes.len());
    // SAFETY: the exact allocation is initialized and UTF-8 was checked above.
    let owned = unsafe { Box::from_raw(slice) }.into_vec();
    Ok(unsafe { String::from_utf8_unchecked(owned) })
}
