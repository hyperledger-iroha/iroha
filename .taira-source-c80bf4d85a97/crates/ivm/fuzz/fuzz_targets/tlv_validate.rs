#![no_main]

use ivm::pointer_abi::validate_tlv_bytes;
use libfuzzer_sys::fuzz_target;

const MAX_LEN: usize = 0x8000; // Match INPUT region size.

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let len = data.len().min(MAX_LEN);
    let envelope = &data[..len];

    let _ = validate_tlv_bytes(envelope);

    if len > 1 {
        let _ = validate_tlv_bytes(&envelope[1..]);
    }
});
