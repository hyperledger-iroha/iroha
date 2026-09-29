//! Real-context internal-node roots, immutable attempt identity and cleanup controls.

use super::*;
use crate::backend::deep_striped_merkle::{StreamLimits, StripedMerklePlan};

fn payload(index: usize, output: &mut [u8]) {
    for (position, word) in output.chunks_exact_mut(8).enumerate() {
        word.copy_from_slice(&((index * 17 + position * 13 + 1) as u64).to_le_bytes());
    }
}
fn tree(binding: &Context) -> Vec<Vec<Digest>> {
    let oracle = Oracle::Fri(4);
    let mut levels = vec![
        (0..128)
            .map(|index| {
                let mut bytes = [0; 128];
                payload(index, &mut bytes);
                binding
                    .hash_leaf(oracle, u32::try_from(index).unwrap(), &bytes)
                    .unwrap()
            })
            .collect::<Vec<_>>(),
    ];
    while levels.last().unwrap().len() > 1 {
        let level = levels.len();
        let next = levels
            .last()
            .unwrap()
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| {
                binding
                    .hash_parent(
                        oracle,
                        u32::try_from(level).unwrap(),
                        u32::try_from(index).unwrap(),
                        pair[0],
                        pair[1],
                    )
                    .unwrap()
            })
            .collect();
        levels.push(next);
    }
    levels
}
fn completed(binding: &Context, levels: &[Vec<Digest>]) -> CompletedNodes {
    let mut pending = NodeCachePlan::new(Oracle::Fri(4))
        .unwrap()
        .start(binding)
        .unwrap();
    for (level, nodes) in levels.iter().enumerate().skip(1) {
        for (index, &value) in nodes.iter().enumerate() {
            pending.record(level, index, value).unwrap();
        }
    }
    pending.finish(levels.last().unwrap()[0]).unwrap()
}
#[allow(
    clippy::unnecessary_wraps,
    reason = "passed directly as the fallible regeneration callback of `open`"
)]
fn fill(indices: &[usize], output: &mut [u8]) -> Result<()> {
    for (&index, row) in indices.iter().zip(output.chunks_exact_mut(128)) {
        payload(index, row);
    }
    Ok(())
}

#[test]
fn fixed_cache_envelope_charges_every_node_coverage_and_opening_buffer() {
    let oracles = [
        Oracle::Row,
        Oracle::QuotientAndMask,
        Oracle::Fri(0),
        Oracle::Fri(1),
        Oracle::Fri(2),
        Oracle::Fri(3),
        Oracle::Fri(4),
    ];
    let mut sum = 0;
    for oracle in oracles {
        let plan = NodeCachePlan::new(oracle).unwrap();
        assert_eq!(plan.nodes, plan.leaves - 1);
        assert_eq!(
            plan.payload_bytes,
            plan.nodes * 48 + plan.nodes.div_ceil(64) * 8
        );
        assert!(
            opening_payload_bytes(oracle).unwrap() >= 3 * QUERY_COUNT * oracle.shape().unwrap().3
        );
        sum += plan.payload_bytes;
    }
    assert_eq!(sum, 834_439_424);
    assert!(NodeCachePlan::new(Oracle::Terminal).is_err());
    assert!(NodeCachePlan::with_shape(Oracle::Fri(4), 3).is_err());
    assert!(NodeCachePlan::with_shape(Oracle::Row, LDE_ROWS * 2).is_err());
}

#[test]
fn rejected_writes_poison_even_a_complete_cache_and_partial_owners_erase() {
    ERASURES.with(|v| v.set((0, 0)));
    let binding = Context::new(b"poisoned internal node owner").unwrap();
    let levels = tree(&binding);
    let root = levels.last().unwrap()[0];
    for rejected in [(0, 0), (1, 64), (8, 0), (1, 0)] {
        let mut pending = NodeCachePlan::new(Oracle::Fri(4))
            .unwrap()
            .start(&binding)
            .unwrap();
        for (level, nodes) in levels.iter().enumerate().skip(1) {
            for (index, &value) in nodes.iter().enumerate() {
                pending.record(level, index, value).unwrap();
            }
        }
        assert!(pending.record(rejected.0, rejected.1, root).is_err());
        assert!(pending.finish(root).is_err());
    }
    let mut pending = NodeCachePlan::new(Oracle::Fri(4))
        .unwrap()
        .start(&binding)
        .unwrap();
    pending.record(1, 0, levels[1][0]).unwrap();
    assert!(pending.finish(root).is_err());
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut pending = NodeCachePlan::new(Oracle::Fri(4))
            .unwrap()
            .start(&binding)
            .unwrap();
        pending.record(1, 0, levels[1][0]).unwrap();
        panic!("public injected cache failure");
    }));
    assert!(unwound.is_err());
    ERASURES.with(|v| assert_eq!(v.get(), (6 * 127, 0)));
}

#[test]
fn cache_binds_original_attempt_oracle_root_and_unchanged_opening_bytes() {
    let binding = Context::new(b"per attempt cached frontier").unwrap();
    let other = Context::new(b"per attempt cached frontier").unwrap();
    let changed = Context::new(b"another immutable statement").unwrap();
    let clone = binding.clone();
    assert!(binding.same_attempt(&clone));
    assert!(!binding.same_attempt(&other));
    let levels = tree(&binding);
    let root = levels.last().unwrap()[0];
    for context in [&other, &changed] {
        assert!(
            completed(&binding, &levels)
                .bind(context, Oracle::Fri(4), root)
                .is_err()
        );
    }
    assert!(
        completed(&binding, &levels)
            .bind(&binding, Oracle::Row, root)
            .is_err()
    );
    assert!(
        completed(&binding, &levels)
            .bind(&binding, Oracle::Fri(4), Digest::default())
            .is_err()
    );
    for query in [
        vec![0],
        vec![127],
        vec![0, 1, 63, 64, 126, 127],
        (0..128).step_by(2).collect(),
    ] {
        let opened = completed(&binding, &levels)
            .bind(&clone, Oracle::Fri(4), root)
            .unwrap()
            .open(&query, DigestExecutionV1::Cpu, fill)
            .unwrap();
        let plan = MultiproofPlan::new(
            128,
            &query,
            MultiproofLimits {
                max_depth: 7,
                max_queried_leaves: 64,
                max_siblings: 448,
                max_parent_hashes: 448,
            },
        )
        .unwrap();
        let expected = plan
            .sibling_positions()
            .iter()
            .map(|p| levels[p.level][p.index])
            .collect::<Vec<_>>();
        assert_eq!(opened.root, root);
        assert_eq!(opened.siblings, expected);
        for (&index, bytes) in query.iter().zip(opened.values()) {
            let mut reference = [0; 128];
            payload(index, &mut reference);
            assert_eq!(bytes, reference);
        }
    }
}

#[test]
fn altered_regeneration_cache_nodes_and_partial_callbacks_cannot_publish() {
    let binding = Context::new(b"cached opening negative controls").unwrap();
    let levels = tree(&binding);
    let root = levels.last().unwrap()[0];
    let query = [0, 63, 127];
    assert!(
        completed(&binding, &levels)
            .bind(&binding, Oracle::Fri(4), root)
            .unwrap()
            .open(&query, DigestExecutionV1::Cpu, |indices, out| {
                fill(indices, out)?;
                out[0] ^= 1;
                Ok(())
            })
            .is_err()
    );
    let mut corrupted = completed(&binding, &levels);
    let plan = MultiproofPlan::new(
        128,
        &query,
        MultiproofLimits {
            max_depth: 7,
            max_queried_leaves: 64,
            max_siblings: 448,
            max_parent_hashes: 448,
        },
    )
    .unwrap();
    let sibling = plan
        .sibling_positions()
        .iter()
        .find(|p| p.level > 0)
        .unwrap();
    corrupted.nodes.0[corrupted.plan.slot(sibling.level, sibling.index).unwrap()] = [0; 6];
    assert!(
        corrupted
            .bind(&binding, Oracle::Fri(4), root)
            .unwrap()
            .open(&query, DigestExecutionV1::Cpu, fill)
            .is_err()
    );
    PAYLOAD_ERASURES.with(|v| v.set((0, 0)));
    for unwind in [false, true] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            completed(&binding, &levels)
                .bind(&binding, Oracle::Fri(4), root)
                .unwrap()
                .open(&query, DigestExecutionV1::Cpu, |_, out| {
                    out.fill(91);
                    assert!(!unwind, "public injected regeneration failure");
                    Err(invalid("injected callback failure"))
                })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
    }
    PAYLOAD_ERASURES.with(|v| {
        let (bytes, bad) = v.get();
        assert!(bytes >= 2 * query.len() * 128);
        assert_eq!(bad, 0);
    });
    assert!(
        completed(&binding, &levels)
            .bind(&binding, Oracle::Fri(4), root)
            .unwrap()
            .open(&[1, 1], DigestExecutionV1::Cpu, |_, _| panic!(
                "invalid query must precede callback"
            ))
            .is_err()
    );
}

#[test]
fn striped_cache_capture_matches_every_materialized_internal_coordinate() {
    let binding = Context::new(b"striped cache capture parity").unwrap();
    let levels = tree(&binding);
    let root = levels.last().unwrap()[0];
    for stripes in [1, 2, 8, 64, 128] {
        let limits = StreamLimits {
            digest_execution: DigestExecutionV1::Cpu,
            max_payload_bytes: usize::MAX,
            max_hashes: usize::MAX,
        };
        let pending = NodeCachePlan::new(Oracle::Fri(4))
            .unwrap()
            .start(&binding)
            .unwrap();
        let mut stream = StripedMerklePlan::new(128, stripes, &[], limits)
            .unwrap()
            .start_cached(pending)
            .unwrap();
        let hash = |level: usize, index: usize, left, right| {
            binding
                .hash_parent_at(Oracle::Fri(4), level, index, left, right)
                .map_err(|error| binding_error(&error))
        };
        for stripe in 0..stripes {
            for start in (0..128 / stripes).step_by(3) {
                let indices = (start..(start + 3).min(128 / stripes))
                    .map(|row| stripe + row * stripes)
                    .collect::<Vec<_>>();
                let mut values = SecretPolynomial::zeroed(indices.len()).unwrap();
                for (&i, v) in indices.iter().zip(values.iter_mut()) {
                    *v = levels[0][i].words();
                }
                stream
                    .push_batch(
                        &indices,
                        &mut values,
                        |level, indices, left, right| {
                            for ((&index, &l), r) in indices.iter().zip(left).zip(right) {
                                *r = hash(level, index, digest(l), digest(*r))?.words();
                            }
                            Ok(())
                        },
                        hash,
                    )
                    .unwrap();
            }
        }
        let result = stream.finish(hash).unwrap();
        assert_eq!(result.root, root);
        let cache = result.cache.unwrap();
        for (level, nodes) in levels.iter().enumerate().skip(1) {
            for (index, &expected) in nodes.iter().enumerate() {
                assert_eq!(
                    cache.nodes.0[cache.plan.slot(level, index).unwrap()],
                    expected.words()
                );
            }
        }
        let opened = cache
            .bind(&binding, Oracle::Fri(4), root)
            .unwrap()
            .open(&[0, 1, 63, 64, 127], DigestExecutionV1::Cpu, fill)
            .unwrap();
        assert_eq!(opened.root, root);
    }
}

#[test]
fn in_memory_parent_coordinates_match_u32_hashing_and_never_truncate() {
    let binding = Context::new(b"in-memory parent coordinates").unwrap();
    let levels = tree(&binding);
    for (level, pair) in levels.windows(2).enumerate() {
        for (index, (children, &expected)) in pair[0].chunks_exact(2).zip(&pair[1]).enumerate() {
            assert_eq!(
                binding
                    .hash_parent_at(Oracle::Fri(4), level + 1, index, children[0], children[1])
                    .unwrap(),
                expected
            );
        }
    }
    // A coordinate beyond `u32` is rejected, never truncated onto a valid node
    // such as level 1, index 0 (`1 << 32` on 64-bit targets).
    let (left, right) = (levels[0][0], levels[0][1]);
    let mut beyond_u32 = vec![usize::MAX];
    if let Ok(truncating) = usize::try_from(u64::from(u32::MAX) + 1) {
        beyond_u32.push(truncating);
    }
    for coordinate in beyond_u32 {
        for (level, index) in [(coordinate, 0), (1, coordinate)] {
            assert!(matches!(
                binding.hash_parent_at(Oracle::Fri(4), level, index, left, right),
                Err(crate::backend::deep_binding::BindingError::Shape)
            ));
        }
    }
}
