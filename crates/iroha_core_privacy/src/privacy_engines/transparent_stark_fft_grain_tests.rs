//! Controlled CPU FFT task-grain parity and scheduling cost experiments.
//!
//! Production keeps its original grain. Timers reuse one in-place work buffer;
//! fixtures, expected outputs, pool construction, reset and comparison are outside.

use super::{GoldilocksFieldV1 as F, GoldilocksFp4V1 as E, *};

fn base_with_grain_and_powers_v1<const GRAIN: usize>(
    values: &mut [F],
    root: F,
    powers: Option<&GoldilocksFftPowersV1>,
) -> Result<(), TransparentStarkErrorV1> {
    goldilocks_fft_with_stage_v1(values, root, powers, |values, width, step, powers| {
        goldilocks_fft_stage_with_grain_v1::<_, true, GRAIN>(
            values,
            width,
            step,
            powers,
            |even, odd, twiddle| {
                let scaled_odd = (*odd).mul(twiddle);
                let original_even = *even;
                *even = original_even.add(scaled_odd);
                *odd = original_even.sub(scaled_odd);
            },
        );
    })
}

fn base_with_grain_v1<const GRAIN: usize>(
    values: &mut [F],
    root: F,
) -> Result<(), TransparentStarkErrorV1> {
    base_with_grain_and_powers_v1::<GRAIN>(values, root, None)
}

fn extension_with_grain_v1<const GRAIN: usize>(
    values: &mut [E],
    root: F,
) -> Result<(), TransparentStarkErrorV1> {
    goldilocks_fp4_fft_with_stage_v1(values, root, |values, width, step| {
        goldilocks_fft_stage_with_grain_v1::<_, true, GRAIN>(
            values,
            width,
            step,
            None,
            |even, odd, twiddle| {
                let scaled_odd = (*odd).mul_base(twiddle);
                let original_even = *even;
                *even = original_even.add(scaled_odd);
                *odd = original_even.sub(scaled_odd);
            },
        );
    })
}

fn base_value_v1(index: usize) -> F {
    F(GOLDILOCKS_MODULUS_V1 - 1 - ((index as u64 * 37) % 65521))
}

fn extension_value_v1(index: usize) -> E {
    E::from_coefficients(core::array::from_fn(|lane| {
        base_value_v1(index + lane * 97)
    }))
    .unwrap()
}

fn check_grain_v1<const GRAIN: usize>(log: u8) {
    let size = 1usize << log;
    let root = goldilocks_primitive_root_v1(log).unwrap();
    let inverse_root = root.inv().unwrap();
    let inverse_size = F::reduce(size as u128).inv().unwrap();
    let original = zeroize::Zeroizing::new((0..size).map(base_value_v1).collect::<Vec<_>>());
    let mut expected = zeroize::Zeroizing::new(original.to_vec());
    goldilocks_fft_v1(&mut expected, root).unwrap();
    let mut actual = zeroize::Zeroizing::new(original.to_vec());
    let address = actual.as_ptr();
    let capacity = actual.capacity();
    base_with_grain_v1::<GRAIN>(&mut actual, root).unwrap();
    assert_eq!(&*actual, &*expected, "base log={log} grain={GRAIN}");
    let powers = GoldilocksFftPowersV1::new_v1(
        size,
        root,
        GoldilocksFftPowersV1::required_payload_bytes_v1(size).unwrap(),
    )
    .unwrap();
    actual.copy_from_slice(&original);
    base_with_grain_and_powers_v1::<GRAIN>(&mut actual, root, Some(&powers)).unwrap();
    assert_eq!(&*actual, &*expected, "powers log={log} grain={GRAIN}");
    base_with_grain_v1::<GRAIN>(&mut actual, inverse_root).unwrap();
    for value in actual.iter_mut() {
        *value = value.mul(inverse_size);
    }
    goldilocks_ifft_v1(&mut expected, root).unwrap();
    assert_eq!(&*actual, &*expected);
    assert_eq!(&*actual, &*original);
    assert_eq!(actual.as_ptr(), address);
    assert_eq!(actual.capacity(), capacity);

    let original = zeroize::Zeroizing::new((0..size).map(extension_value_v1).collect::<Vec<_>>());
    let mut expected = zeroize::Zeroizing::new(original.to_vec());
    goldilocks_fp4_fft_v1(&mut expected, root).unwrap();
    let mut actual = zeroize::Zeroizing::new(original.to_vec());
    let address = actual.as_ptr();
    let capacity = actual.capacity();
    extension_with_grain_v1::<GRAIN>(&mut actual, root).unwrap();
    assert_eq!(&*actual, &*expected, "fp4 log={log} grain={GRAIN}");
    extension_with_grain_v1::<GRAIN>(&mut actual, inverse_root).unwrap();
    for value in actual.iter_mut() {
        *value = value.mul_base(inverse_size);
    }
    goldilocks_fp4_ifft_v1(&mut expected, root).unwrap();
    assert_eq!(&*actual, &*expected);
    assert_eq!(&*actual, &*original);
    assert_eq!(actual.as_ptr(), address);
    assert_eq!(actual.capacity(), capacity);
}

#[test]
fn task_grains_preserve_full_forward_inverse_and_boundary_outputs() {
    assert_eq!(FFT_BUTTERFLIES_PER_TASK_V1, 1024);
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            // Below/at/above each grain boundary and the original parallel cutoff.
            for log in [0, 1, 3, 10, 11, 12, 13, 14, 15, 16] {
                check_grain_v1::<1024>(log);
                check_grain_v1::<4096>(log);
                check_grain_v1::<16384>(log);
            }
        });
    }
}

fn check_refusals_v1<const GRAIN: usize>() {
    let root = goldilocks_primitive_root_v1(3).unwrap();
    for (length, root, invalid) in [
        (0, root, false),
        (3, root, false),
        (8, F::ZERO, false),
        (8, F::ONE, false),
        (8, F(GOLDILOCKS_MODULUS_V1), false),
        (8, F::ONE, true),
        (8, root, true),
    ] {
        let mut original = (0..length).map(base_value_v1).collect::<Vec<_>>();
        if invalid {
            original[length - 1] = F(GOLDILOCKS_MODULUS_V1);
        }
        let mut expected = original.clone();
        let mut actual = original.clone();
        assert_eq!(
            base_with_grain_v1::<GRAIN>(&mut actual, root),
            goldilocks_fft_v1(&mut expected, root)
        );
        assert_eq!(actual, original);
        let original = original
            .iter()
            .map(|&value| E::from_raw_coefficients_for_testing([value, F::ZERO, F::ZERO, F::ZERO]))
            .collect::<Vec<_>>();
        let mut expected = original.clone();
        let mut actual = original.clone();
        assert_eq!(
            extension_with_grain_v1::<GRAIN>(&mut actual, root),
            goldilocks_fp4_fft_v1(&mut expected, root)
        );
        assert_eq!(actual, original);
    }
    let mut original = (0..8).map(base_value_v1).collect::<Vec<_>>();
    let mut expected = original.clone();
    let mut powers = GoldilocksFftPowersV1::new_v1(
        8,
        root,
        GoldilocksFftPowersV1::required_payload_bytes_v1(8).unwrap(),
    )
    .unwrap();
    powers.root = F::ONE;
    assert_eq!(
        base_with_grain_and_powers_v1::<GRAIN>(&mut original, root, Some(&powers)),
        goldilocks_fft_with_powers_v1::<true>(&mut expected, root, Some(&powers))
    );
    assert_eq!(original, expected);
}

#[test]
fn task_grains_keep_original_domain_canonicality_and_power_refusals() {
    check_refusals_v1::<1024>();
    check_refusals_v1::<4096>();
    check_refusals_v1::<16384>();
}

type TransformV1<T> = fn(&mut [T], F) -> Result<(), TransparentStarkErrorV1>;

fn measure_v1<T>(
    log: u8,
    field: &str,
    value: impl Fn(usize) -> T,
    reference: TransformV1<T>,
    transforms: [TransformV1<T>; 3],
) where
    T: Copy + Send + Sync + PartialEq + core::fmt::Debug + zeroize::Zeroize,
{
    assert!(!cfg!(debug_assertions), "use an optimized build");
    let size = 1usize << log;
    let root = goldilocks_primitive_root_v1(log).unwrap();
    let original = zeroize::Zeroizing::new((0..size).map(value).collect::<Vec<_>>());
    let mut expected = zeroize::Zeroizing::new(original.to_vec());
    let mut work = zeroize::Zeroizing::new(original.to_vec());
    let address = work.as_ptr();
    let capacity = work.capacity();
    for workers in [4, 16, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        expected.copy_from_slice(&original);
        pool.install(|| reference(&mut expected, root)).unwrap();
        for round in 0..3 {
            // Rotate the public order so every grain occupies each position once.
            for offset in 0..3 {
                let index = (round + offset) % 3;
                let grain = [1024, 4096, 16384][index];
                work.copy_from_slice(&original);
                let start = std::time::Instant::now();
                pool.install(|| transforms[index](std::hint::black_box(work.as_mut_slice()), root))
                    .unwrap();
                let elapsed = start.elapsed();
                assert_eq!(&*work, &*expected);
                assert_eq!(work.as_ptr(), address);
                assert_eq!(work.capacity(), capacity);
                eprintln!(
                    "x509_cpu_fft_grain field={field} direction=forward rows={size} workers={workers} grain={grain} round={round} seconds={:.6} parity=exact in_place=true",
                    elapsed.as_secs_f64()
                );
            }
        }
    }
}

#[test]
#[ignore = "controlled CPU FFT grain cost at 65536 base-field rows"]
fn cpu_fft_grain_cost_base_log16() {
    measure_v1(
        16,
        "F",
        base_value_v1,
        goldilocks_fft_v1,
        [
            base_with_grain_v1::<1024>,
            base_with_grain_v1::<4096>,
            base_with_grain_v1::<16384>,
        ],
    );
}

#[test]
#[ignore = "controlled CPU FFT grain cost at 524288 base-field rows"]
fn cpu_fft_grain_cost_base_log19() {
    measure_v1(
        19,
        "F",
        base_value_v1,
        goldilocks_fft_v1,
        [
            base_with_grain_v1::<1024>,
            base_with_grain_v1::<4096>,
            base_with_grain_v1::<16384>,
        ],
    );
}

#[test]
#[ignore = "controlled CPU FFT grain cost at 65536 quartic-extension rows"]
fn cpu_fft_grain_cost_extension_log16() {
    measure_v1(
        16,
        "Fp4",
        extension_value_v1,
        goldilocks_fp4_fft_v1,
        [
            extension_with_grain_v1::<1024>,
            extension_with_grain_v1::<4096>,
            extension_with_grain_v1::<16384>,
        ],
    );
}

#[test]
#[ignore = "controlled CPU FFT grain cost at 524288 quartic-extension rows"]
fn cpu_fft_grain_cost_extension_log19() {
    measure_v1(
        19,
        "Fp4",
        extension_value_v1,
        goldilocks_fp4_fft_v1,
        [
            extension_with_grain_v1::<1024>,
            extension_with_grain_v1::<4096>,
            extension_with_grain_v1::<16384>,
        ],
    );
}
