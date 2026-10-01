// Native authenticated reads retain the original executed graph and refuse unpaid I/O.
// Exact prepaid length and occupied-frame substitution are covered alongside
// native_execution_reads; these checks add DA-file bounds and State query charging.

#[test]
fn canonical_cold_read_budget_refusal_preserves_storage_and_retry() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let kura = chain.kura();
    let height = nonzero!(1_usize);
    let expected = chain.committed(1).block().clone();
    kura.block_data.lock()[0].1 = None;
    let before = kura.canonical_block_wire_bytes_for_testing(height).unwrap();
    let limits = norito::DecodeLimits::new(1, 1, 1, 1, 1);
    let refused = norito::with_decode_limits_scope(limits, || kura.get_block(height));
    assert!(
        refused.is_none(),
        "cold read must respect its caller's budget"
    );
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
    assert!(kura.block_data.lock().cached_body(0).is_none());
    assert_eq!(
        kura.canonical_block_wire_bytes_for_testing(height).unwrap(),
        before
    );
    assert_eq!(kura.get_block(height).unwrap().as_ref(), expected.as_ref());

    // A real stored-wire fault must still close admission; a budget refusal is
    // not a license to ignore independently detected canonical corruption.
    kura.corrupt_native_frame_for_test(height);
    kura.block_data.lock()[0].1 = None;
    assert!(kura.get_block(height).is_none());
    assert!(kura.canonical_storage_poisoned.load(Ordering::Acquire));
}

#[test]
fn authenticated_da_body_read_refuses_oversized_occupied_file_without_repair() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    let receipt = CertifiedChain::new(&view)
        .unwrap()
        .authenticated_execution(2)
        .unwrap();
    let original = receipt.block();
    let wire = original.encode_wire().unwrap();
    let wire_len = wire.len() as u64;
    let blocks = (1..=3)
        .map(|height| Arc::clone(chain.committed(height).block()))
        .collect::<Vec<_>>();
    let kura = chain.kura();
    canonical_physical_seed_da_suffix(kura, &blocks);
    let (retained, bytes) = kura
        .read_authenticated_execution_wire(&receipt, wire_len)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&retained, original));
    assert_eq!(bytes, wire);
    let path = kura.block_store.lock().da_block_path(2);
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&[0]).unwrap();
    file.sync_all().unwrap();
    assert_eq!(fs::metadata(&path).unwrap().len(), wire_len + 1);
    assert!(
        kura.read_authenticated_execution_wire(&receipt, wire_len)
            .is_err()
    );
    assert_eq!(
        fs::metadata(path).unwrap().len(),
        wire_len + 1,
        "authenticated reads refuse occupied corruption without repairing it"
    );
}

#[test]
fn executed_history_denial_precedes_cold_body_decode_and_projection() {
    use crate::{
        state::{StateReadOnly as _, World},
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_data_model::query::error::QueryExecutionFail;
    for corrupt in [false, true] {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        chain.commit(Vec::new());
        let kura = chain.kura();
        let expected = chain.committed(2).block().clone();
        let tip = chain.committed(3).block().clone();
        let target_len = expected.encode_wire().unwrap().len() as u64;
        let tip_len = tip.encode_wire().unwrap().len() as u64;
        if corrupt {
            kura.corrupt_native_frame_for_test(nonzero!(3_usize));
        }
        kura.mark_transaction_entrypoint_index_incomplete(2, 3);
        kura.block_data.lock()[1].1 = None;
        kura.block_data.lock()[2].1 = None;
        let index_before = format!("{:?}", *kura.transaction_entrypoint_index.lock());
        let raw_before = {
            let mut store = kura.block_store.lock();
            let slot = store.read_block_index(2).unwrap();
            let mut bytes = vec![0; usize::try_from(slot.length).unwrap()];
            store.read_block_data(slot.start, &mut bytes).unwrap();
            bytes
        };
        kura.reset_canonical_query_reads_for_test();
        let view = chain.state().view();
        let source = view.canonical_history();
        let mut charged = Vec::new();
        let denied = source.executed_block(nonzero!(2_usize), |blocks, bytes| {
            charged.push((blocks, bytes));
            assert_eq!((blocks, bytes), (1, tip_len));
            Err(QueryExecutionFail::GasBudgetExceeded)
        });
        assert!(matches!(denied, Err(QueryExecutionFail::GasBudgetExceeded)));
        assert_eq!(charged, vec![(1, tip_len)]);
        assert_eq!(
            kura.canonical_query_reads_for_test(),
            (0, 0),
            "denial precedes any source-body read, including corrupt occupied frames"
        );
        assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
        assert!(kura.block_data.lock().cached_body(1).is_none());
        assert!(kura.block_data.lock().cached_body(2).is_none());
        assert_eq!(
            format!("{:?}", *kura.transaction_entrypoint_index.lock()),
            index_before
        );

        charged.clear();
        let admitted = source.executed_block(nonzero!(2_usize), |blocks, bytes| {
            charged.push((blocks, bytes));
            Ok(())
        });
        if corrupt {
            assert!(
                admitted.is_err(),
                "occupied corruption cannot authenticate the State tip"
            );
            assert_eq!(charged, vec![(1, tip_len)]);
            assert_eq!(kura.canonical_query_reads_for_test(), (1, tip_len));
        } else {
            assert_eq!(admitted.unwrap().as_ref(), expected.as_ref());
            assert_eq!(charged, vec![(1, tip_len), (1, target_len)]);
            assert_eq!(
                kura.canonical_query_reads_for_test(),
                (2, tip_len + target_len)
            );
        }
        assert!(
            kura.block_data.lock().cached_body(1).is_none(),
            "historical authentication does not publish caches"
        );
        assert!(kura.block_data.lock().cached_body(2).is_none());
        assert_eq!(
            format!("{:?}", *kura.transaction_entrypoint_index.lock()),
            index_before
        );
        let raw_after = {
            let mut store = kura.block_store.lock();
            let slot = store.read_block_index(2).unwrap();
            let mut bytes = vec![0; usize::try_from(slot.length).unwrap()];
            store.read_block_data(slot.start, &mut bytes).unwrap();
            bytes
        };
        assert_eq!(
            raw_after, raw_before,
            "queries never repair occupied source bytes"
        );
    }
}
