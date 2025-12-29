//! Ensure oversized code buffers are rejected during predecode.

use ivm::{Memory, VMError, ivm_cache::IvmCache};

#[test]
fn decode_stream_rejects_oversize_code() {
    let oversized = vec![0u8; Memory::HEAP_START as usize + 1];
    let res = IvmCache::decode_stream(&oversized);
    assert!(matches!(res, Err(VMError::MemoryOutOfBounds)));
}
