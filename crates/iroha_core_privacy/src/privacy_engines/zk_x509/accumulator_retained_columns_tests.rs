//! Original coefficient replay, bounded ownership and failure-path tests.

use super::super::super::private_table::inspection;
use super::*;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn native_v1() -> Vec<Vec<F>> {
    (0..ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
        .map(|column| {
            (0..ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1)
                .map(|row| F((3 + column * 11 + row * 7) as u64))
                .collect()
        })
        .collect()
}
fn evaluate_v1(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |value, coefficient| {
            value.mul(point).add(*coefficient)
        })
}

#[test]
fn retained_ca_coefficients_preserve_native_values_and_replay_the_same_mask() {
    let owner = CaOriginalMaskedColumnsV1::sample_v1(
        CaColumnFamilyV1::Auxiliary,
        native_v1(),
        &mut ChaCha12Rng::seed_from_u64(73),
    )
    .unwrap();
    assert_eq!(owner.family_v1(), CaColumnFamilyV1::Auxiliary);
    assert_eq!(CaColumnFamilyV1::Base.width_v1(), 695);
    assert_eq!(CaColumnFamilyV1::Auxiliary.width_v1(), 128);
    let n = ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1;
    let coefficients = n + CA_MASK_DEGREE_V1 + 1;
    assert_eq!(
        owner.allocated_payload_bytes_v1().unwrap(),
        128 * (core::mem::size_of::<Vec<F>>() + coefficients * 8)
    );
    assert_eq!(
        owner.allocated_payload_bytes_v1().unwrap(),
        CaOriginalMaskedColumnsV1::payload_bound_v1(CaColumnFamilyV1::Auxiliary).unwrap()
    );
    assert_eq!(
        CaOriginalMaskedColumnsV1::payload_bound_v1(CaColumnFamilyV1::Base).unwrap(),
        695 * (core::mem::size_of::<Vec<F>>() + coefficients * core::mem::size_of::<F>())
    );
    assert!(format!("{owner:?}").contains("<redacted>"));
    let root = goldilocks_primitive_root_v1(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1).unwrap();
    for column in [0, 96, 116, 120, 124, 127] {
        let values = owner.column_v1(column).unwrap();
        assert_eq!(values.len(), coefficients);
        for row in [0, 1, 12, 13, 103, n - 1] {
            assert_eq!(
                evaluate_v1(values, root.pow(row as u128)),
                F((3 + column * 11 + row * 7) as u64)
            );
        }
    }
    let original = owner.column_v1(116).unwrap().to_vec();
    let first = owner.column_lde_v1(116).unwrap();
    let second = owner.column_lde_v1(116).unwrap();
    assert_eq!(&*first, &*second);
    assert_eq!(owner.column_v1(116).unwrap(), original);
    let lde_root = goldilocks_primitive_root_v1(ZK_X509_CA_FRI_LDE_LOG2_V1).unwrap();
    for row in [0, 1, 17, 65535] {
        assert_eq!(
            first[row],
            evaluate_v1(
                &original,
                F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(row as u128))
            )
        );
    }
    let z = E::canonical([13, 17, 19, 23]).unwrap();
    let expected = original.iter().rev().fold(E::ZERO, |value, coefficient| {
        value.mul(z).add(E::from_base(*coefficient))
    });
    assert_eq!(owner.open_v1(116, z).unwrap(), expected);
    assert!(owner.open_v1(128, z).is_err());
    assert!(owner.column_v1(128).is_err());
    assert!(owner.column_lde_v1(128).is_err());
    assert!(owner.open_v1(0, E::ZERO).is_err());
    assert!(owner.open_v1(0, E::ONE).is_err());
    let local = owner.local_lde_v1().unwrap();
    assert_eq!(local.len(), 128);
    assert_eq!(local[116], *first);
    assert!(local.iter().all(|column| column.len() == 65_536));
    drop(local);
    let ((), observations) = inspection::observe_v1(|| drop(owner));
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
    assert_eq!(
        observations
            .iter()
            .filter(|item| item.cells == coefficients)
            .count(),
        128
    );
    // This is public fixture data. Production replay never creates this copy.
}

#[derive(Debug)]
struct EntropyFailure;
impl std::error::Error for EntropyFailure {}
impl core::fmt::Display for EntropyFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("held CA test entropy failure")
    }
}
struct PartialEntropy {
    remaining: usize,
    unwind: bool,
}
impl TryRng for PartialEntropy {
    type Error = EntropyFailure;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        self.try_next_u64()
            .map(|v| u32::try_from(v & u64::from(u32::MAX)).expect("masked random word"))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        if self.remaining == 0 {
            assert!(!self.unwind, "deliberate CA coefficient entropy unwind");
            return Err(EntropyFailure);
        }
        self.remaining -= 1;
        Ok(7 + self.remaining as u64)
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        for block in destination.chunks_mut(8) {
            let value = self.try_next_u64()?.to_le_bytes();
            block.copy_from_slice(&value[..block.len()]);
        }
        Ok(())
    }
}

#[test]
fn retained_ca_coefficients_clear_native_and_completed_columns_on_error_and_unwind() {
    for unwind in [false, true] {
        let (result, observations) = inspection::observe_v1(|| {
            catch_unwind(AssertUnwindSafe(|| {
                CaOriginalMaskedColumnsV1::sample_v1(
                    CaColumnFamilyV1::Auxiliary,
                    native_v1(),
                    &mut PartialEntropy {
                        remaining: CA_MASK_DEGREE_V1 + 4,
                        unwind,
                    },
                )
            }))
        });
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(_))));
        }
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
        assert!(
            observations
                .iter()
                .filter(|item| item.cells == ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1)
                .count()
                >= 128
        );
        assert!(observations.iter().any(|item| item.cells
            == ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 + CA_MASK_DEGREE_V1 + 1
            && item.nonzero_before > 0));
    }
}

#[test]
fn malformed_native_ca_input_is_cleared_before_any_entropy_is_read() {
    for variant in 0..3 {
        let mut native = native_v1();
        match variant {
            0 => {
                native.pop();
            }
            1 => {
                native[0].pop();
            }
            _ => native[0][0] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1),
        }
        let (result, observations) = inspection::observe_v1(|| {
            CaOriginalMaskedColumnsV1::sample_v1(
                CaColumnFamilyV1::Auxiliary,
                native,
                &mut PartialEntropy {
                    remaining: 0,
                    unwind: true,
                },
            )
        });
        assert!(result.is_err());
        assert!(observations.iter().any(|item| item.nonzero_before > 0));
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
    }
}

fn public_coefficients_v1() -> CaOriginalMaskedColumnsV1 {
    let count = ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 + CA_MASK_DEGREE_V1 + 1;
    CaOriginalMaskedColumnsV1 {
        family: CaColumnFamilyV1::Auxiliary,
        columns: PrivateTableV1::new(
            (0..CaColumnFamilyV1::Auxiliary.width_v1())
                .map(|column| {
                    (0..count)
                        .map(|degree| F::reduce((column * 17 + degree * degree + 3) as u128))
                        .collect()
                })
                .collect(),
            clear_columns_v1,
        ),
    }
}

#[test]
fn parallel_ca_local_lde_matches_original_columns_and_exact_matrix_capacities() {
    let owner = public_coefficients_v1();
    let pointers = owner
        .columns
        .iter()
        .map(|column| column.as_ptr())
        .collect::<Vec<_>>();
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let actual = pool.install(|| owner.local_lde_v1()).unwrap();
        assert_eq!(actual.len(), owner.family.width_v1());
        assert_eq!(actual.capacity(), actual.len());
        for (index, column) in actual.iter().enumerate() {
            let reference = serial.install(|| owner.column_lde_v1(index)).unwrap();
            assert_eq!(column, &*reference);
            assert_eq!(column.len(), 1 << ZK_X509_CA_FRI_LDE_LOG2_V1);
            assert_eq!(column.capacity(), column.len());
        }
        assert_eq!(
            owner
                .columns
                .iter()
                .map(|column| column.as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
    }
}

#[test]
fn parallel_ca_local_lde_rejects_original_shape_errors_before_workers() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for malformed in 0..4 {
        let mut owner = public_coefficients_v1();
        let index = 3;
        match malformed {
            0 => owner.columns[index].clear(),
            1 => owner.columns[index].resize((1 << ZK_X509_CA_FRI_LDE_LOG2_V1) + 1, F::ONE),
            2 => owner.columns[index][0] = F(u64::MAX),
            _ => {
                owner.columns.truncate(index);
            }
        }
        let expected = owner.column_lde_v1(index).err().unwrap();
        let calls = AtomicUsize::new(0);
        let result = owner.local_lde_with_v1(|_, _, _, _, _| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });
        assert_eq!(result.err().unwrap(), expected);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn parallel_ca_local_lde_selects_first_error_and_clears_all_outputs_on_unwind() {
    let owner = public_coefficients_v1();
    let width = owner.family.width_v1();
    let rows = 1 << ZK_X509_CA_FRI_LDE_LOG2_V1;
    for unwind in [false, true] {
        let (result, cleared) = inspection::observe_v1(|| {
            catch_unwind(AssertUnwindSafe(|| {
                owner.local_lde_with_v1(|index, _, target, _, _| {
                    target.fill(F((index + 1) as u64));
                    if index == 9 {
                        assert!(!unwind, "injected CA local column panic");
                        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
                    }
                    if index == 13 {
                        return Err(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening);
                    }
                    Ok(())
                })
            }))
        });
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().err().unwrap(),
                ZkX509CaAccumulatorProofErrorV1::Resource
            );
        }
        assert_eq!(
            cleared.iter().filter(|entry| entry.cells == rows).count(),
            width
        );
        assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
        assert!(cleared.iter().any(|entry| entry.nonzero_before != 0));
        assert_eq!(owner.column_v1(0).unwrap()[0], F(3));
    }
    let ((), cleared) = inspection::observe_v1(|| {
        drop(owner.local_lde_v1().unwrap());
    });
    assert_eq!(
        cleared.iter().filter(|entry| entry.cells == rows).count(),
        width
    );
    assert!(cleared.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
#[ignore = "optimized native CA auxiliary full-matrix scheduling cost; preserves every output"]
fn ca_local_lde_parallel_column_cost_preserves_full_matrix() {
    use std::time::Instant;
    let owner = public_coefficients_v1();
    for round in 0..2 {
        let mut originals = None;
        let mut parallel = None;
        let mut timings = [std::time::Duration::ZERO; 2];
        for coarse in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let start = Instant::now();
            let result = if coarse {
                owner.local_lde_v1().unwrap()
            } else {
                let mut columns = PrivateTableV1::new(Vec::new(), clear_columns_v1);
                columns.try_reserve_exact(owner.family.width_v1()).unwrap();
                for index in 0..owner.family.width_v1() {
                    columns.push(owner.column_lde_v1(index).unwrap().into_vec());
                }
                columns
            };
            timings[usize::from(coarse)] = start.elapsed();
            if coarse {
                parallel = Some(result);
            } else {
                originals = Some(result);
            }
        }
        assert_eq!(
            &**originals.as_ref().unwrap(),
            &**parallel.as_ref().unwrap()
        );
        eprintln!(
            "ca_local_lde rows=65536 columns={} round={round} workers={} original_serial_columns_inner_parallel_ns={} parallel_columns_coarse_fft_ns={} parity=exact includes=allocation-scaling-fft excludes=source-sampling-drop",
            owner.family.width_v1(),
            rayon::current_num_threads(),
            timings[0].as_nanos(),
            timings[1].as_nanos()
        );
    }
}
