#[cfg(test)]
mod scheduler_variant_tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::transaction::signed::TransactionEntrypoint;

    fn make_hash(v: u8) -> HashOf<TransactionEntrypoint> {
        let mut b = [0u8; Hash::LENGTH];
        b[0] = v;
        b[Hash::LENGTH - 1] |= 1; // keep LSB set as per Hash invariant
        HashOf::from_untyped_unchecked(Hash::prehashed(b))
    }

    // Build a small CSR graph by hand for testing
    // adj: 0 -> [2]; 1 -> [2,3]; 2 -> []; 3 -> [4]; 4 -> []
    fn sample_graph() -> (
        Vec<usize>,
        Vec<usize>,
        Vec<usize>,
        Vec<HashOf<TransactionEntrypoint>>,
    ) {
        let row_offsets = vec![0, 1, 3, 3, 4, 4];
        let cols = vec![2, 2, 3, 4];
        let indeg = vec![0, 0, 2, 1, 1];
        let call_hashes = vec![
            make_hash(10),
            make_hash(5),
            make_hash(30),
            make_hash(7),
            make_hash(8),
        ];
        (row_offsets, cols, indeg, call_hashes)
    }

    #[test]
    fn per_wave_scheduler_deterministic_order() {
        let (row_offsets, cols, indeg, call_hashes) = sample_graph();
        // Implement per-wave scheduling locally for test
        let n = indeg.len();
        let mut indeg_s = indeg.clone();
        let mut ready = Vec::new();
        for (i, &deg) in indeg_s.iter().enumerate() {
            if deg == 0 {
                ready.push(i);
            }
        }
        let mut order = Vec::with_capacity(n);
        while !ready.is_empty() {
            ready.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            let current = ready.split_off(0);
            for &i in &current {
                order.push(i);
                let (start, end) = (row_offsets[i], row_offsets[i + 1]);
                for &v in &cols[start..end] {
                    indeg_s[v] = indeg_s[v].saturating_sub(1);
                    if indeg_s[v] == 0 {
                        ready.push(v);
                    }
                }
            }
        }
        assert_eq!(order, vec![1, 0, 3, 2, 4]);
    }

    #[test]
    fn ready_heap_scheduler_topo_order() {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let (row_offsets, cols, indeg, call_hashes) = sample_graph();
        let n = indeg.len();
        let mut indeg_s = indeg.clone();
        let mut heap: BinaryHeap<Reverse<(HashOf<TransactionEntrypoint>, usize)>> =
            BinaryHeap::with_capacity(n);
        for i in 0..n {
            if indeg_s[i] == 0 {
                heap.push(Reverse((call_hashes[i], i)));
            }
        }
        let mut order = Vec::with_capacity(n);
        while let Some(Reverse((_h, i))) = heap.pop() {
            order.push(i);
            let (start, end) = (row_offsets[i], row_offsets[i + 1]);
            for &v in &cols[start..end] {
                indeg_s[v] = indeg_s[v].saturating_sub(1);
                if indeg_s[v] == 0 {
                    heap.push(Reverse((call_hashes[v], v)));
                }
            }
        }

        // Valid deterministic topological order
        assert_eq!(order, vec![1, 3, 4, 0, 2]);
    }

    #[test]
    fn component_scheduler_orders_components_contiguously() {
        let components = vec![vec![2, 3, 4], vec![0, 1]];
        let row_offsets = vec![0, 1, 1, 2, 3, 3];
        let cols = vec![1, 3, 4];
        let indeg = vec![0, 1, 0, 1, 1];
        let call_hashes = vec![
            make_hash(10),
            make_hash(12),
            make_hash(5),
            make_hash(40),
            make_hash(50),
        ];

        let wave = super::schedule_components_wave(&components, &row_offsets, &cols, &call_hashes)
            .expect("component scheduling must succeed (wave)");
        assert_eq!(wave, vec![2, 3, 4, 0, 1]);

        let heap =
            super::schedule_components_ready_heap(&components, &row_offsets, &cols, &call_hashes)
                .expect("component scheduling must succeed (heap)");
        assert_eq!(heap, vec![2, 3, 4, 0, 1]);

        let global_wave = super::schedule_wave_global(&row_offsets, &cols, &indeg, &call_hashes);
        assert_eq!(global_wave, vec![2, 0, 1, 3, 4]);

        let global_heap =
            super::schedule_ready_heap_global(&row_offsets, &cols, &indeg, &call_hashes);
        assert_eq!(global_heap, vec![2, 0, 1, 3, 4]);
    }

    #[test]
    fn conflict_free_layers_merge_singletons_into_one_wave() {
        let components = vec![vec![3], vec![1], vec![0], vec![2]];
        let row_offsets = vec![0, 0, 0, 0, 0];
        let cols = Vec::new();
        let call_hashes = vec![make_hash(40), make_hash(10), make_hash(30), make_hash(20)];

        let layers =
            super::conflict_free_component_layers(&components, &row_offsets, &cols, &call_hashes)
                .expect("singleton components must schedule");

        assert_eq!(layers, vec![vec![1, 3, 2, 0]]);
    }

    #[test]
    fn conflict_free_layers_preserve_component_depths() {
        let components = vec![vec![2, 0, 1], vec![4, 3]];
        let row_offsets = vec![0, 1, 2, 2, 3, 3];
        let cols = vec![1, 2, 4];
        let call_hashes = vec![
            make_hash(20),
            make_hash(10),
            make_hash(30),
            make_hash(15),
            make_hash(5),
        ];

        let layers =
            super::conflict_free_component_layers(&components, &row_offsets, &cols, &call_hashes)
                .expect("component-local chains must schedule");

        assert_eq!(layers, vec![vec![3, 0], vec![4, 1], vec![2]]);
    }
}
