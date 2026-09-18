// Private result ownership checks. Economic rows are additionally exercised by
// the real State/Kura independent-batch controls in ordinary_common_tail_tests.

fn tail_receipt_fixture() -> iroha_data_model::events::data::prelude::AssetBatchTransferOutcome {
    use iroha_data_model::events::data::prelude::{
        AssetBatchTransferLegStatus, AssetBatchTransferOutcome,
    };
    let (authority, _) = gen_account_in("tail");
    let definition = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("tail", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "owned".into(),
        asset: iroha_data_model::asset::AssetId::new(definition, authority.clone()),
        destination: authority,
        amount: 1u32.into(),
        status: AssetBatchTransferLegStatus::Applied,
    }
}

#[test]
fn common_tail_receipt_join_preserves_full_prefix_and_rejects_competing_rows_atomically() {
    use iroha_data_model::transaction::signed::TransactionResult;
    let first = HashOf::from_untyped_unchecked(Hash::new(b"first executed call"));
    let second = HashOf::from_untyped_unchecked(Hash::new(b"second executed call"));
    let owners = BTreeMap::from([(first, 0), (second, 1)]);
    let receipt = tail_receipt_fixture();
    let mut prefix = vec![TransactionResult::new(Ok(Default::default())); 2];
    prefix[0].set_batch_transfer_outcomes(vec![receipt.clone()]);
    let retained = prefix[0].clone();
    assign_tail_batch_outcomes(&mut prefix, &owners, BTreeMap::new()).unwrap();
    assert_eq!(prefix[0], retained);
    let before = prefix.clone();
    assert!(
        assign_tail_batch_outcomes(
            &mut prefix,
            &owners,
            BTreeMap::from([
                (first, vec![receipt.clone()]),
                (second, vec![receipt.clone()]),
            ])
        )
        .is_err()
    );
    assert_eq!(
        prefix, before,
        "a later failure cannot attach any earlier row"
    );
    let mut empty = vec![TransactionResult::new(Ok(Default::default()))];
    assert!(
        assign_tail_batch_outcomes(
            &mut empty,
            &BTreeMap::from([(first, 0), (second, 0)]),
            BTreeMap::from([
                (first, vec![receipt.clone()]),
                (second, vec![receipt.clone()])
            ])
        )
        .is_err()
    );
    assert!(empty[0].batch_transfer_outcomes().is_empty());
    assign_tail_batch_outcomes(
        &mut prefix,
        &owners,
        BTreeMap::from([(second, vec![receipt.clone()])]),
    )
    .unwrap();
    assert_eq!(prefix[0], retained);
    assert_eq!(prefix[1].batch_transfer_outcomes(), &[receipt]);
}

#[test]
fn common_tail_receipt_join_rejects_unknown_empty_and_out_of_range_owners() {
    use iroha_data_model::transaction::signed::TransactionResult;
    let owner = HashOf::from_untyped_unchecked(Hash::new(b"executed owner"));
    let receipt = tail_receipt_fixture();
    for (owners, outcomes) in [
        (
            BTreeMap::new(),
            BTreeMap::from([(owner, vec![receipt.clone()])]),
        ),
        (
            BTreeMap::from([(owner, 0)]),
            BTreeMap::from([(owner, Vec::new())]),
        ),
        (
            BTreeMap::from([(owner, 1)]),
            BTreeMap::from([(owner, vec![receipt])]),
        ),
    ] {
        let mut results = vec![TransactionResult::new(Ok(Default::default()))];
        let before = results.clone();
        assert!(assign_tail_batch_outcomes(&mut results, &owners, outcomes).is_err());
        assert_eq!(results, before);
    }
}

#[test]
fn common_tail_time_join_uses_returned_calls_not_repeated_display_hashes() {
    let (authority, _) = gen_account_in("tail");
    let time = iroha_data_model::trigger::TimeTriggerEntrypoint {
        id: "repeat".parse().unwrap(),
        instructions: iroha_data_model::transaction::ExecutionStep(
            iroha_primitives::const_vec::ConstVec::new_empty(),
        ),
        authority,
    };
    let entries = vec![time.clone(), time.clone()];
    let displays = vec![time.hash_as_entrypoint(); 2];
    let calls = [
        Hash::new(b"actual invocation 0"),
        Hash::new(b"actual invocation 1"),
    ];
    let first = tail_receipt_fixture();
    let mut second = first.clone();
    second.leg_id = "second invocation".into();
    let rows = BTreeMap::from([
        (
            HashOf::from_untyped_unchecked(calls[0]),
            vec![first.clone()],
        ),
        (
            HashOf::from_untyped_unchecked(calls[1]),
            vec![second.clone()],
        ),
    ]);
    let inner = vec![Ok(Default::default()); 2];
    let full = join_time_tail_receipts(
        &BTreeMap::new(),
        &entries,
        &displays,
        inner.clone(),
        &calls,
        rows.clone(),
    )
    .unwrap();
    assert_eq!(full[0].batch_transfer_outcomes(), &[first]);
    assert_eq!(full[1].batch_transfer_outcomes(), &[second]);
    assert_eq!(entries[0], entries[1]);
    assert_ne!(calls[0], calls[1]);
    assert!(
        join_time_tail_receipts(
            &BTreeMap::new(),
            &entries,
            &displays[..1],
            inner.clone(),
            &calls,
            rows.clone()
        )
        .is_err()
    );
    let mut wrong_display = displays.clone();
    wrong_display[0] = HashOf::from_untyped_unchecked(Hash::new(b"not that Time entry"));
    assert!(
        join_time_tail_receipts(
            &BTreeMap::new(),
            &entries,
            &wrong_display,
            inner.clone(),
            &calls,
            rows.clone()
        )
        .is_err()
    );
    assert!(
        join_time_tail_receipts(
            &BTreeMap::new(),
            &entries,
            &displays,
            inner.clone(),
            &[calls[0]; 2],
            rows.clone()
        )
        .is_err()
    );
    let prefix = BTreeMap::from([(HashOf::from_untyped_unchecked(calls[0]), 0)]);
    assert!(
        join_time_tail_receipts(
            &prefix,
            &entries,
            &displays,
            inner.clone(),
            &calls,
            rows.clone()
        )
        .is_err()
    );
    let mut leftovers = rows;
    leftovers.insert(
        HashOf::from_untyped_unchecked(Hash::new(b"unowned callback")),
        vec![tail_receipt_fixture()],
    );
    assert!(
        join_time_tail_receipts(
            &BTreeMap::new(),
            &entries,
            &displays,
            inner,
            &calls,
            leftovers
        )
        .is_err()
    );
}

#[test]
fn common_tail_prefix_receipt_aliases_are_one_to_one() {
    use iroha_data_model::transaction::signed::SealedTransactionReveal;
    let (authority, key) = gen_account_in("tail");
    let signed = iroha_data_model::transaction::TransactionBuilder::new(
        iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"tail network",
        ))),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([iroha_data_model::isi::Log::new(
        iroha_data_model::Level::INFO,
        "tail".into(),
    )])
    .sign(key.private_key());
    let external = TransactionEntrypoint::External(signed.clone());
    let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"commitment"),
        signed,
        [0; 32],
    ));
    assert_eq!(
        prefix_tail_receipt_owners(std::slice::from_ref(&reveal))
            .unwrap()
            .get(&external.execution_call_hash()),
        Some(&0)
    );
    assert!(prefix_tail_receipt_owners(&[external.clone(), external.clone()]).is_err());
    assert!(prefix_tail_receipt_owners(&[external, reveal]).is_err());
}
