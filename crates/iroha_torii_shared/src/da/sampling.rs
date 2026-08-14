//! Payload-size policy shared by DA and Taikai PDP commitments.
/// Compute how many samples should be taken for the given payload size.
///
/// The window scales with the payload size but is clamped to `[32, 256]`.
#[must_use]
pub fn compute_sample_window(total_size: u64) -> u16 {
    const CHUNK_UNIT: u64 = 64 * 1024 * 1024;
    const MIN_SAMPLES: u64 = 32;
    const MAX_SAMPLES: u64 = 256;
    if total_size == 0 {
        return u16::try_from(MIN_SAMPLES).expect("min samples fits in u16");
    }
    let buckets = total_size.div_ceil(CHUNK_UNIT);
    let clamped = buckets.clamp(MIN_SAMPLES, MAX_SAMPLES);
    u16::try_from(clamped).unwrap_or(u16::MAX)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_window_clamps_and_scales() {
        assert_eq!(compute_sample_window(0), 32);
        assert_eq!(compute_sample_window(1), 32);
        assert_eq!(compute_sample_window(64 * 1024 * 1024), 32);
        assert_eq!(compute_sample_window(65 * 1024 * 1024), 32);
        assert_eq!(compute_sample_window(1024 * 1024 * 1024 * 1024), 256);
    }
}
