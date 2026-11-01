//! Bounded test-only AVX2 experiment on the actual registered fixed/native FFT shapes.

use super::*;
use crate::privacy_engines::transparent_stark::avx2_test_kernel;

#[test]
#[ignore = "mandatory AVX2 registered paired timing diagnostic; no production dispatch change"]
fn avx2_registered_fixed_native_fft_reports_exact_parity_and_paired_cost() {
    use fastpq_prover::goldilocks_transform::transform_goldilocks_columns_v1;
    assert!(std::is_x86_feature_detected!("avx2"));
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let native_logs = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(native_logs, [5, 8, 15, 16, 18, 19].into_iter().collect());
    let mut stripe_logs = std::collections::BTreeSet::new();
    let mut transition_logs = std::collections::BTreeSet::new();
    for registration in &layout.registered_segments {
        let plan = registered_retained_prover_plan_v1(registration.segment, layout.common_lde_log2)
            .unwrap();
        let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
            registration.segment.trace_log2,
            plan.quotient_coset_log2,
            0,
        )
        .unwrap();
        stripe_logs.insert(u8::try_from(stripe.rows.ilog2()).unwrap());
        if stripe.count > 1 {
            transition_logs.insert((
                u8::try_from(stripe.rows.ilog2()).unwrap(),
                plan.quotient_coset_log2,
            ));
        }
    }
    let cases = native_logs
        .into_iter()
        .map(|log| (0, log, log))
        .chain(stripe_logs.into_iter().map(|log| (1, log, log)))
        .chain(
            transition_logs
                .into_iter()
                .map(|(log, full)| (2, log, full)),
        )
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 13);
    println!(
        "avx2_fft_header cases=13 widths=1,8 rounds=3 workers=8,16 expected_records=312 table_construction_drop_timed=true private_copy_oracle_erase_in_transform_timing=false private_erase_timed_separately=true wholeproof_claim=false"
    );
    let mut records = 0;
    let mut all_blocks = 0;
    for workers in [8, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            assert_eq!(rayon::current_num_threads(), workers);
            for (case, &(phase, log, full)) in cases.iter().enumerate() {
                let rows = 1usize << log;
                let root = goldilocks_primitive_root_v1(log).unwrap();
                let diagonal = goldilocks_primitive_root_v1(full).unwrap();
                for width in [1, 8] {
                    // Admit the public shape before any input/oracle/output
                    // allocation: four matrices fit below 129 MiB, with the
                    // separately funded public tables below another 5 MiB.
                    assert!(rows <= 1 << main_quotient_stripes::MAIN_QUOTIENT_STRIPE_LOG2_V1 && width <= 8);
                    let source = columns(rows, width);
                    let mut expected = Words::new(source.iter()
                        .map(|column| column.iter().map(|value| value.0).collect()).collect(), erase_words_v1);
                    if phase != 1 {
                        assert_eq!(transform_goldilocks_columns_v1(&mut expected, root.0,
                            Direction::Inverse, fastpq_prover::ExecutionMode::Cpu).unwrap(), Backend::Cpu);
                    }
                    if phase == 2 {
                        for column in &mut *expected {
                            let mut power = F::ONE;
                            for value in column {
                                *value = F(*value).mul(power).0;
                                power = power.mul(diagonal);
                            }
                        }
                    }
                    if phase != 0 {
                        assert_eq!(transform_goldilocks_columns_v1(&mut expected, root.0,
                            Direction::Forward, fastpq_prover::ExecutionMode::Cpu).unwrap(), Backend::Cpu);
                    }
                    let mut scalar = PrivateTableV1::new(source.to_vec(), zeroize_field_rows_v1);
                    let mut candidate = PrivateTableV1::new(source.to_vec(), zeroize_field_rows_v1);
                    let private_payload = [&*source, &*scalar, &*candidate].iter().map(|columns| {
                        columns.capacity() * core::mem::size_of::<Vec<F>>()
                            + columns.iter().map(|column| column.capacity() * core::mem::size_of::<F>()).sum::<usize>()
                    }).sum::<usize>() + expected.capacity() * core::mem::size_of::<Vec<u64>>()
                        + expected.iter().map(|column| column.capacity() * core::mem::size_of::<u64>()).sum::<usize>()
                        + 3 * core::mem::size_of::<PrivateTableV1<Vec<F>>>() + core::mem::size_of::<Words>();
                    assert!(private_payload + SHARED_POWERS_ALLOWANCE_V1 < 256 * 1024 * 1024);
                    for round in 0..3 {
                        for vector in if round % 2 == 0 { [false, true] } else { [true, false] } {
                            let target = if vector { &mut candidate } else { &mut scalar };
                            for (target, original) in target.iter_mut().zip(source.iter()) {
                                target.copy_from_slice(original);
                            }
                            let started = std::time::Instant::now();
                            let (forward, inverse) = if phase == 0 {
                                (None, policy().cpu_powers_v1(rows, root.inv().unwrap()).unwrap())
                            } else { policy().cpu_power_pair_v1(rows, root, phase == 2).unwrap() };
                            let construction_ns = started.elapsed().as_nanos();
                            let table_payload = forward.as_ref().map_or(0, |table| table.allocated_payload_bytes_v1().unwrap())
                                + inverse.as_ref().map_or(0, |table| table.allocated_payload_bytes_v1().unwrap());
                            assert!(table_payload <= SHARED_POWERS_ALLOWANCE_V1);
                            let vector_blocks = target.par_iter_mut().map(|column| {
                                let mut blocks = 0;
                                if phase != 1 {
                                    if let Some(table) = &inverse {
                                        if vector { blocks += avx2_test_kernel::transform(column, root, table, true).unwrap(); }
                                        else { goldilocks_ifft_coarse_with_powers_v1(column, root, table).unwrap(); }
                                    } else { goldilocks_ifft_coarse_v1(column, root).unwrap(); }
                                }
                                if phase == 2 {
                                    let mut power = F::ONE;
                                    for value in column.iter_mut() {
                                        *value = value.mul(power);
                                        power = power.mul(diagonal);
                                    }
                                }
                                if phase != 0 {
                                    if let Some(table) = &forward {
                                        if vector { blocks += avx2_test_kernel::transform(column, root, table, false).unwrap(); }
                                        else { goldilocks_fft_coarse_with_powers_v1(column, root, table).unwrap(); }
                                    } else { goldilocks_fft_coarse_v1(column, root).unwrap(); }
                                }
                                blocks
                            }).sum::<usize>();
                            drop(forward);
                            drop(inverse);
                            let elapsed_ns = started.elapsed().as_nanos();
                            assert_eq!(vector_blocks > 0, vector && rows >= SHARED_POWERS_MIN_ROWS_V1);
                            for (actual, oracle) in target.iter().zip(expected.iter()) {
                                assert!(actual.iter().zip(oracle).all(|(a, b)| a.0 == *b));
                            }
                            all_blocks += vector_blocks;
                            records += 1;
                            println!("avx2_fft_cost case={case} phase={phase} log={log} full_log={full} width={width} workers={workers} round={round} vector={vector} vector_blocks={vector_blocks} construction_ns={construction_ns} elapsed_ns={elapsed_ns} table_payload_bytes={table_payload} private_payload_bytes={private_payload} parity=exact");
                        }
                    }
                    let clear_started = std::time::Instant::now();
                    drop(candidate);
                    drop(scalar);
                    drop(expected);
                    drop(source);
                    println!("avx2_fft_clear case={case} width={width} workers={workers} private_erase_drop_ns={}", clear_started.elapsed().as_nanos());
                }
            }
        });
    }
    assert_eq!(records, 312);
    assert!(all_blocks > 0);
    println!("avx2_fft_complete records={records} vector_blocks={all_blocks} parity=exact");
}
