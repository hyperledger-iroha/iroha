use rand::{SeedableRng as _, rngs::StdRng};

use super::*;

#[test]
fn retained_polynomial_batches_are_byte_exact_across_thread_counts() {
    let native_columns = (0_u64..17)
        .map(|column| {
            (0_u64..8)
                .map(|row| F::reduce(u128::from(column + 23) * u128::from(row + 41) + 17))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let indices = [0, 7, 31, 63];
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("bounded test pool")
            .install(|| {
                let mut rng = StdRng::from_seed([0xD7; 32]);
                let (commitment, polynomials) = commit_masked_trace_polynomial_columns_v1(
                    b"aggregate-retained-batch-leaf",
                    b"aggregate-retained-batch-node",
                    4,
                    3,
                    6,
                    native_columns.len(),
                    3,
                    &indices,
                    &mut rng,
                    |column| Ok(native_columns[column].clone()),
                )
                .expect("batched retained commitment");
                let replay = replay_masked_trace_polynomial_columns_v1(
                    b"aggregate-retained-batch-leaf",
                    b"aggregate-retained-batch-node",
                    4,
                    &polynomials,
                    &indices,
                )
                .expect("batched retained replay");
                (commitment, replay)
            })
    };

    let single = run(1);
    let release = run(4);
    assert_eq!(single, release);
    assert_eq!(single.0, single.1);
}

#[test]
fn retained_polynomial_commitment_rejects_before_witness_source_work() {
    fn reject_before_source(
        leaf_domain: &[u8],
        node_domain: &'static [u8],
        group: usize,
        native_log2: u8,
        lde_log2: u8,
        width: usize,
        mask_degree: usize,
        indices: &[usize],
    ) {
        let calls = std::cell::Cell::new(0_usize);
        let mut rng = StdRng::from_seed([0xD2; 32]);
        assert_eq!(
            commit_masked_trace_polynomial_columns_v1(
                leaf_domain,
                node_domain,
                group,
                native_log2,
                lde_log2,
                width,
                mask_degree,
                indices,
                &mut rng,
                |_| {
                    calls.set(calls.get() + 1);
                    Ok(vec![F::ZERO; 8])
                },
            )
            .map(|_| ()),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(
            calls.get(),
            0,
            "shape and commitment allocation preflight precede source work"
        );
    }

    reject_before_source(b"", b"node", 0, 3, 6, 2, 3, &[]);
    reject_before_source(b"leaf", b"", 0, 3, 6, 2, 3, &[]);
    reject_before_source(b"leaf", b"node", 0, 3, 3, 2, 3, &[]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, 0, 3, &[]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, 2, 56, &[]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, 2, 3, &[1, 1]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, 2, 3, &[2, 1]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, 2, 3, &[64]);
    reject_before_source(b"leaf", b"node", usize::from(u16::MAX) + 1, 3, 6, 2, 3, &[]);
    reject_before_source(b"leaf", b"node", 0, 3, 6, usize::from(u16::MAX) + 1, 3, &[]);

    let calls = std::cell::Cell::new(0_usize);
    let mut rng = StdRng::from_seed([0xD3; 32]);
    assert_eq!(
        commit_masked_trace_polynomial_columns_v1(
            b"leaf",
            b"node",
            0,
            3,
            6,
            2,
            3,
            &[],
            &mut rng,
            |_| {
                calls.set(calls.get() + 1);
                Ok(vec![F::ZERO; 7])
            },
        )
        .map(|_| ()),
        Err(AggregateStarkErrorV1::InvalidLayout)
    );
    assert_eq!(calls.get(), 1, "malformed first column stops immediately");
}
