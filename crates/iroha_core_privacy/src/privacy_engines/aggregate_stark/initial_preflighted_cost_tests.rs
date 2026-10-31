//! Complete sixteen-column hashing with retained canonical preflight at common22.
//!
//! Public fixtures and one reference cut remain equally resident in both arms.
//! Timers include construction, incoming preflight and the arm's remaining scans,
//! finalization, every cut-cell comparison and clearing the produced cut. They
//! exclude source/FFT evaluation; both use the same sixteen-column output bound;
//! only an end-to-end prover run can qualify their combined memory/time effect.

use super::*;
use std::time::{Duration, Instant};

const POOL_COLUMNS: usize = 16;
const BASE_GROUPS: [usize; 6] = [596, 30, 17, 430, 28, 2112];
const AUX_GROUPS: [usize; 6] = [204, 160, 36, 375, 39, 1804];

fn fixture_v1(rows: usize) -> Vec<ZeroizingFieldColumnV1> {
    (0..POOL_COLUMNS)
        .map(|column| {
            let mut values = Vec::new();
            values.try_reserve_exact(rows).unwrap();
            assert_eq!(values.capacity(), rows);
            values.extend((0..rows).map(|row| {
                // Entirely public, distinct canonical row/column input. This
                // bounded arithmetic also runs unchanged at common22.
                F(1 + (column as u64 + 1) * 1_000_000_007 + row as u64 * 97)
            }));
            ZeroizingFieldColumnV1(values)
        })
        .collect()
}

fn context_v1() -> TransparentStarkDigestContextV1 {
    TransparentStarkDigestContextV1::new(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        b"initial-preflighted-complete-cost-v1",
    )
}

fn make_v1(rows: usize, width: usize, indices: &[usize]) -> StreamingRowCommitmentV1 {
    StreamingRowCommitmentV1::new(
        context_v1(),
        b"joined-leaf",
        b"joined-node",
        usize::from(u16::MAX),
        rows,
        width,
        indices,
    )
    .unwrap()
}

/// Both arms perform the complete incoming-eight preflight. Only the candidate
/// retains that sealed capability instead of repeating the full field scan.
fn absorb_v1<const REUSE_PREFLIGHT: bool>(
    owner: &mut StreamingRowCommitmentV1,
    columns: &[&[F]],
) -> Result<(), AggregateStarkErrorV1> {
    let checked = joined_trace::preflight_test_batch_v1(columns)?;
    if REUSE_PREFLIGHT {
        owner.absorb_preflighted_initial_columns_v1(checked)
    } else {
        owner.absorb_initial_columns_v1(checked.columns_v1())
    }
}

fn feed_v1<const REUSE_PREFLIGHT: bool>(
    owner: &mut StreamingRowCommitmentV1,
    pool: &[ZeroizingFieldColumnV1],
    groups: &[usize],
) {
    const BATCH: usize = 16;
    assert_eq!(pool.len(), POOL_COLUMNS);
    let mut first = 0;
    for &width in groups {
        assert!(width > 0);
        for offset in (0..width).step_by(BATCH) {
            let count = BATCH.min(width - offset);
            let views: [&[F]; BATCH] = core::array::from_fn(|index| {
                if index < count {
                    pool[(first + offset + index) % POOL_COLUMNS].as_ref()
                } else {
                    &[]
                }
            });
            absorb_v1::<REUSE_PREFLIGHT>(owner, &views[..count]).unwrap();
        }
        first += width;
    }
    assert_eq!(first, owner.width);
}

#[test]
fn initial_preflighted_complete_roots_cuts_openings_and_group_tails_match_original() {
    for rows in [2, 32, 4096] {
        let pool = fixture_v1(rows);
        for groups in [
            vec![1],
            vec![17],
            vec![31],
            vec![32],
            vec![33],
            vec![65, 33, 31, 1],
        ] {
            let width = groups.iter().sum();
            let indices = [0, rows - 1];
            let mut before = make_v1(rows, width, &indices);
            let mut after = make_v1(rows, width, &indices);
            feed_v1::<false>(&mut before, &pool, &groups);
            feed_v1::<true>(&mut after, &pool, &groups);
            let before = before.finish().unwrap();
            let after = after.finish().unwrap();
            assert_eq!(before, after);
            for &row in &indices {
                assert_eq!(
                    after.opened_rows[&row],
                    (0..width)
                        .map(|column| pool[column % POOL_COLUMNS][row])
                        .collect::<Vec<_>>()
                );
            }
            // Independent complete materialized row tree, not another call to
            // either paired streaming specialization.
            let materialized = (0..width)
                .map(|column| pool[column % POOL_COLUMNS].to_vec())
                .collect::<Vec<_>>();
            let tree = row_tree_v1(
                context_v1(),
                b"joined-leaf",
                b"joined-node",
                usize::from(u16::MAX),
                &materialized,
                rows,
            )
            .unwrap();
            assert_eq!(after.commitment.root, tree.root());
            assert_eq!(
                after.commitment.frontier,
                canonical_multiproof_frontier_v1(&tree, rows, &indices).unwrap()
            );
            let mut before = make_v1(rows, width, &[]);
            let mut after = make_v1(rows, width, &[]);
            feed_v1::<false>(&mut before, &pool, &groups);
            feed_v1::<true>(&mut after, &pool, &groups);
            let (before, before_cut) = before.finish_retaining_cut_v1().unwrap();
            let (after, after_cut) = after.finish_retaining_cut_v1().unwrap();
            assert_eq!(before, after);
            assert_eq!(after.commitment.root, tree.root());
            assert_eq!(before_cut.roots, after_cut.roots);
            assert_eq!(before_cut.root, after_cut.root);
            assert_eq!(before_cut.rows, after_cut.rows);
        }
    }
}

#[test]
fn initial_preflighted_rejects_every_bad_tail_and_preserves_shape_and_poison_checks() {
    let mut pool = fixture_v1(4);
    for bad in 0..16 {
        let original = pool[bad].0[3];
        pool[bad].0[3] = F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1);
        let views = pool.iter().map(AsRef::as_ref).collect::<Vec<&[F]>>();
        assert!(matches!(
            joined_trace::preflight_test_batch_v1(&views),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        ));
        drop(views);
        pool[bad].0[3] = original;
    }
    let views = pool.iter().map(AsRef::as_ref).collect::<Vec<&[F]>>();
    for (rows, width) in [(8, 16), (4, 15)] {
        let mut owner = make_v1(rows, width, &[]);
        let checked = joined_trace::preflight_test_batch_v1(&views).unwrap();
        assert_eq!(
            owner.absorb_preflighted_initial_columns_v1(checked),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(owner.received_columns, 0);
        assert!(!owner.failed);
    }
    assert!(matches!(
        joined_trace::preflight_test_batch_v1(&[]),
        Err(AggregateStarkErrorV1::InvalidLayout)
    ));
    let mut over = views.clone();
    over.push(views[0]);
    assert!(matches!(
        joined_trace::preflight_test_batch_v1(&over),
        Err(AggregateStarkErrorV1::InvalidLayout)
    ));
    let mut wrong_extent = views.clone();
    wrong_extent[15] = &views[15][..3];
    assert!(matches!(
        joined_trace::preflight_test_batch_v1(&wrong_extent),
        Err(AggregateStarkErrorV1::InvalidLayout)
    ));
    for reuse in [false, true] {
        let mut owner = make_v1(4, 16, &[]);
        owner.digest_streams.corrupt_remaining_v1(3, 16 * 8);
        let result = if reuse {
            absorb_v1::<true>(&mut owner, &views)
        } else {
            absorb_v1::<false>(&mut owner, &views)
        };
        assert!(result.is_err());
        assert!(owner.failed);
        assert_eq!(owner.received_columns, 0);
        let checked = joined_trace::preflight_test_batch_v1(&views).unwrap();
        assert_eq!(
            owner.absorb_preflighted_initial_columns_v1(checked),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert!(owner.finish_retaining_cut_v1().is_err());
    }
}

struct SampleV1 {
    total: Duration,
    initialize: Duration,
    absorb: Duration,
    finish: Duration,
    compare: Duration,
    clear: Duration,
}

fn timed_v1<const REUSE_PREFLIGHT: bool>(
    pool: &[ZeroizingFieldColumnV1],
    groups: &[usize],
    reference: &RetainedMerkleCutV1,
) -> SampleV1 {
    let start = Instant::now();
    let mut owner = make_v1(pool[0].len(), groups.iter().sum(), &[]);
    let initialize = start.elapsed();
    let phase = Instant::now();
    feed_v1::<REUSE_PREFLIGHT>(&mut owner, pool, groups);
    let absorb = phase.elapsed();
    let phase = Instant::now();
    let (mut result, cut) = owner.finish_retaining_cut_v1().unwrap();
    let finish = phase.elapsed();
    let phase = Instant::now();
    assert_eq!(cut.rows, reference.rows);
    assert_eq!(cut.roots, reference.roots);
    assert_eq!(cut.root, reference.root);
    assert_eq!(result.commitment.root, reference.root);
    assert!(result.commitment.frontier.is_empty());
    assert!(result.opened_rows.is_empty());
    let compare = phase.elapsed();
    let phase = Instant::now();
    drop(cut); // Full retained allocation is cleared and freed within the timer.
    result.commitment.root.zeroize_v1();
    drop(result);
    let clear = phase.elapsed();
    SampleV1 {
        total: start.elapsed(),
        initialize,
        absorb,
        finish,
        compare,
        clear,
    }
}

fn benchmark_v1(scenarios: &[(&str, &[usize])], repeats: usize) {
    const ROWS: usize = 1 << 22;
    assert!(matches!(repeats, 4 | 8));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(20)
        .build()
        .unwrap();
    pool.install(|| {
        assert_eq!(rayon::current_num_threads(), 20);
        let fixture = fixture_v1(ROWS);
        assert_eq!(fixture.iter().map(|column| column.0.capacity() * size_of::<F>()).sum::<usize>(), 536_870_912);
        assert_eq!(ROWS / 2 * size_of::<crate::privacy_engines::privacy_outer_hash::PrivacyOuterLastFieldPairV1>(), 889_192_448);
        for &(scenario, groups) in scenarios {
            let width = groups.iter().sum();
            // Untimed oracle and warmup; reference remains live in every arm.
            let mut oracle = make_v1(ROWS, width, &[]);
            feed_v1::<false>(&mut oracle, &fixture, groups);
            let (mut result, reference) = oracle.finish_retaining_cut_v1().unwrap();
            assert_eq!(reference.roots.capacity() * size_of::<PrivacyOuterDigestV1>(), 100_663_296);
            result.commitment.root.zeroize_v1();
            drop(result);
            for reuse in [true, false] {
                let warmup = if reuse { timed_v1::<true>(&fixture, groups, &reference) } else { timed_v1::<false>(&fixture, groups, &reference) };
                std::hint::black_box(warmup);
            }
            eprintln!("initial_preflighted_scope scenario={scenario} rows={ROWS} columns={width} workers=20 fixture_bytes=536870912 reference_cut_bytes=100663296 paired_state_bytes=889192448 scope=construct_preflight_absorb_finalize_compare_clear source_fft_and_fixture_setup=excluded");
            let mut durations = [Vec::new(), Vec::new()];
            // ABBA then BAAB balances both positions and nearest neighbours.
            for (trial, reuse) in [false, true, true, false, true, false, false, true].into_iter().take(repeats).enumerate() {
                let sample = if reuse { timed_v1::<true>(&fixture, groups, &reference) } else { timed_v1::<false>(&fixture, groups, &reference) };
                let arm = if reuse { "preflighted" } else { "original" };
                durations[usize::from(reuse)].push(sample.total.as_secs_f64());
                eprintln!("initial_preflighted_cost scenario={scenario} trial={trial} arm={arm} batch=16 total={:.9} initialize={:.9} absorb={:.9} finish={:.9} compare={:.9} clear={:.9} complete_cut_parity=true", sample.total.as_secs_f64(), sample.initialize.as_secs_f64(), sample.absorb.as_secs_f64(), sample.finish.as_secs_f64(), sample.compare.as_secs_f64(), sample.clear.as_secs_f64());
            }
            let medians = durations.map(|mut values| { values.sort_by(f64::total_cmp); (values[(values.len()-1)/2] + values[values.len()/2]) / 2.0 });
            eprintln!("initial_preflighted_median scenario={scenario} original={:.9} preflighted={:.9} ratio={:.6}", medians[0], medians[1], medians[0]/medians[1]);
            drop(reference);
        }
        drop(fixture);
    });
}

#[test]
#[ignore = "common22 paired hashing screen: full common22 public/reference/state working set"]
fn initial_preflighted_common22_complete_cost_screen() {
    benchmark_v1(&[("screen256", &[256])], 4);
}

#[test]
#[ignore = "complete real base/aux schedules at common22; balanced eight trials per role"]
fn initial_preflighted_common22_complete_cost_full_profile() {
    benchmark_v1(&[("base3213", &BASE_GROUPS), ("aux2618", &AUX_GROUPS)], 8);
}
