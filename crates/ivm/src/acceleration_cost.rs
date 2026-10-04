//! Bounded interpolation of whole-operation costs within measured public geometry.

/// Preserve measured endpoints and either slope; never extrapolate a last sample.
pub(crate) fn interpolate<T>(
    samples: &[T],
    items: usize,
    position: impl Fn(&T) -> usize,
    costs: impl Fn(&T) -> (u64, u64),
) -> Option<(u64, u64)> {
    if items < position(samples.first()?)
        || items > position(samples.last()?)
        || samples
            .windows(2)
            .any(|pair| position(&pair[0]) >= position(&pair[1]))
    {
        return None;
    }
    let upper_index = samples.partition_point(|sample| position(sample) < items);
    let upper = samples.get(upper_index)?;
    if position(upper) == items {
        return Some(costs(upper));
    }
    let lower = samples.get(upper_index.checked_sub(1)?)?;
    let span = (position(upper) - position(lower)) as u128;
    let offset = (items - position(lower)) as u128;
    let mix = |low: u64, high: u64| {
        // usize is at most 64 bits on supported targets, and the two weights sum
        // to span. This weighted sum fits u128 even for maximum u64 timings.
        ((u128::from(low) * (span - offset) + u128::from(high) * offset) / span) as u64
    };
    let (low_cpu, low_gpu) = costs(lower);
    let (high_cpu, high_gpu) = costs(upper);
    Some((mix(low_cpu, high_cpu), mix(low_gpu, high_gpu)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(samples: &[(usize, u64, u64)], items: usize) -> Option<(u64, u64)> {
        interpolate(
            samples,
            items,
            |sample| sample.0,
            |sample| (sample.1, sample.2),
        )
    }

    #[test]
    fn preserves_endpoints_and_opposite_slopes_with_no_extrapolation() {
        let samples = [(32, 100, 90), (128, 40, 30), (512, 80, 50)];
        assert_eq!(cost(&samples, 31), None);
        assert_eq!(cost(&samples, 32), Some((100, 90)));
        assert_eq!(cost(&samples, 80), Some((70, 60)));
        assert_eq!(cost(&samples, 128), Some((40, 30)));
        assert_eq!(cost(&samples, 320), Some((60, 40)));
        assert_eq!(cost(&samples, 512), Some((80, 50)));
        assert_eq!(cost(&samples, 513), None);
        assert_eq!(cost(&samples, usize::MAX), None);
    }

    #[test]
    fn rejects_empty_repeated_or_unordered_geometries() {
        assert_eq!(cost(&[], 0), None);
        assert_eq!(cost(&[(5, 10, 2)], 5), Some((10, 2)));
        assert_eq!(cost(&[(5, 10, 2)], 6), None);
        assert_eq!(cost(&[(5, 10, 2), (5, 20, 3)], 5), None);
        assert_eq!(cost(&[(5, 10, 2), (4, 20, 3)], 5), None);
    }

    #[test]
    fn retains_full_width_timing_and_geometry_precision() {
        let samples = [(0, u64::MAX, 0), (usize::MAX, 0, u64::MAX)];
        let middle = usize::MAX / 2;
        let high = ((u128::from(u64::MAX) * middle as u128) / usize::MAX as u128) as u64;
        assert_eq!(cost(&samples, middle), Some((u64::MAX - high, high)));
        assert_eq!(cost(&samples, usize::MAX), Some((0, u64::MAX)));
    }
}
