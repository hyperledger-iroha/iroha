#![no_main]

use iroha_data_model::offline::{
    KagemushaRecursiveSpendCandidateV4, KagemushaRecursiveSpendReleaseRecordV4,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    let Some((&kind, archive)) = data.split_first() else {
        return;
    };
    if archive.is_empty() || archive.len() > MAX_INPUT_BYTES {
        return;
    }
    let limits = norito::canonical_decode_limits(archive.len());
    match kind & 1 {
        0 => {
            if let Ok(candidate) = norito::decode_canonical_with_limits::<
                KagemushaRecursiveSpendCandidateV4,
            >(archive, limits)
            {
                let _ = candidate.validate();
            }
        }
        _ => {
            if let Ok(release) = norito::decode_canonical_with_limits::<
                KagemushaRecursiveSpendReleaseRecordV4,
            >(archive, limits)
            {
                let _ = release.validate_structure();
            }
        }
    }
});
