use std::iter;

use criterion::{Criterion, criterion_group, criterion_main};
use norito::streaming::{
    BundleAcceleration, EntropyMode,
    codec::{BaselineEncoder, BaselineEncoderConfig, FrameDimensions, RawFrame},
};

fn bench_bundled_scalar(c: &mut Criterion) {
    let dimensions = FrameDimensions::new(32, 32);
    let frames: Vec<RawFrame> = iter::repeat_with(|| {
        RawFrame::new(dimensions, vec![0x55; dimensions.pixel_count()]).expect("frame")
    })
    .take(5)
    .collect();

    let config = BaselineEncoderConfig {
        frame_dimensions: dimensions,
        frames_per_segment: frames.len() as u16,
        frame_duration_ns: 16_666_667,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 3,
        ..BaselineEncoderConfig::default()
    };

    c.bench_function("streaming_rans_bundled_scalar", |b| {
        b.iter(|| {
            let mut encoder = BaselineEncoder::new(config.clone());
            encoder
                .encode_segment(17, 2_000_000, 9, &frames, None)
                .expect("encode");
        })
    });
}

fn bench_bundled_simd(c: &mut Criterion) {
    #[allow(unreachable_code)]
    let simd_available = {
        #[cfg(target_arch = "x86_64")]
        {
            std::arch::is_x86_feature_detected!("avx2")
        }
        #[cfg(target_arch = "aarch64")]
        {
            std::arch::is_aarch64_feature_detected!("neon")
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            false
        }
    };

    if !simd_available {
        return;
    }

    let dimensions = FrameDimensions::new(32, 32);
    let frames: Vec<RawFrame> = iter::repeat_with(|| {
        RawFrame::new(dimensions, vec![0x33; dimensions.pixel_count()]).expect("frame")
    })
    .take(5)
    .collect();

    let config = BaselineEncoderConfig {
        frame_dimensions: dimensions,
        frames_per_segment: frames.len() as u16,
        frame_duration_ns: 16_666_667,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width: 3,
        bundle_acceleration: BundleAcceleration::CpuSimd,
        bundle_prefetch_distance: 4,
        ..BaselineEncoderConfig::default()
    };

    c.bench_function("streaming_rans_bundled_simd", |b| {
        b.iter(|| {
            let mut encoder = BaselineEncoder::new(config.clone());
            encoder
                .encode_segment(23, 2_000_000, 12, &frames, None)
                .expect("encode");
        })
    });
}

criterion_group!(streaming_bundled, bench_bundled_scalar, bench_bundled_simd);
criterion_main!(streaming_bundled);
