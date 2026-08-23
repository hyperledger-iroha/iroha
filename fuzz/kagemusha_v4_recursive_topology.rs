#![no_main]

use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4, KagemushaRecursiveSpendBundleV4,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4 {
        return;
    }
    if let Ok(bundle) = norito::decode_canonical_with_limits::<KagemushaRecursiveSpendBundleV4>(
        data,
        norito::canonical_decode_limits(data.len()),
    ) {
        let _ = bundle.validate_public_binding();
    }
});
