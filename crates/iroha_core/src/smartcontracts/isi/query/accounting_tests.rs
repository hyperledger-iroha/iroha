//! Source rows, retained keys and pagination share one exact work budget.

use iroha_data_model::domain::Domain;
use iroha_test_samples::ALICE_ID;

use super::*;

fn domains() -> Vec<Domain> {
    [Some(3), None, Some(1), Some(2)]
        .into_iter()
        .enumerate()
        .map(|(id, rank)| {
            let mut domain =
                Domain::new(DomainId::try_new(format!("accounting{id}"), "universal").unwrap())
                    .build(&ALICE_ID);
            if let Some(rank) = rank {
                domain
                    .metadata_mut()
                    .insert("rank".parse().unwrap(), Json::new(rank));
            }
            domain
        })
        .collect()
}

#[test]
fn sorted_paths_charge_each_source_once_and_retain_exact_key_byte_costs() {
    let values = domains();
    let key: Name = "rank".parse().unwrap();
    let bytes: u64 = values
        .iter()
        .map(|value| {
            bounded_bare_encoded_len(value, u64::MAX).unwrap()
                + value
                    .get_metadata_sorting_key(&key)
                    .map_or(0, |key| bounded_bare_encoded_len(key, u64::MAX).unwrap())
                + value.bounded_tiebreak_key_len(u64::MAX).unwrap()
        })
        .sum();
    // Offset + fetch selects the heap path at 3 and materialized path at 4,097.
    for fetch in [2, STREAMING_SORTED_PREFIX_LIMIT as u64] {
        let fetch = NonZeroU64::new(fetch).unwrap();
        let params = QueryParams {
            pagination: Pagination::new(Some(fetch), 1),
            sorting: iroha_data_model::query::parameters::Sorting::by_metadata_key(key.clone()),
            fetch_size: FetchSize::new(Some(fetch)),
        };
        let exact_units = 4 * 7 + bytes * 3;
        let run = |units| {
            apply_query_postprocessing_ephemeral_with_budget(
                values.clone().into_iter(),
                SelectorTuple::<Domain>::default(),
                &params,
                QueryLimits::new(fetch.get()),
                Some(QueryExecutionBudget::from_weighted_limit(units, 7, 3)),
            )
        };
        let (output, stats) = run(exact_units).expect("exact source and key work fits");
        assert_eq!(stats.processed_items(), 4);
        assert_eq!(stats.processed_bytes(), bytes);
        let expected = [
            values[3].id.clone(),
            values[0].id.clone(),
            values[1].id.clone(),
        ];
        let QueryOutputBatchBox::Domain(batch) = output.batch.into_iter().next().unwrap() else {
            panic!("expected domain batch");
        };
        assert_eq!(
            batch
                .into_iter()
                .map(|domain| domain.id)
                .collect::<Vec<_>>(),
            expected[..usize::try_from(fetch.get()).unwrap().min(3)]
        );
        assert!(matches!(
            run(exact_units - 1),
            Err(Error::GasBudgetExceeded)
        ));
    }
}

#[test]
fn bounded_offset_charges_skipped_rows_and_the_probe_once() {
    let values = domains();
    let bytes = values
        .iter()
        .map(|value| bounded_bare_encoded_len(value, u64::MAX).unwrap())
        .sum::<u64>();
    let fetch = NonZeroU64::new(2).unwrap();
    let params = QueryParams {
        pagination: Pagination::new(None, 1),
        fetch_size: FetchSize::new(Some(fetch)),
        ..QueryParams::default()
    };
    let run = |units| {
        apply_query_postprocessing_ephemeral_with_budget(
            values.clone().into_iter(),
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::new(2).with_count_mode(QueryCountMode::Bounded),
            Some(QueryExecutionBudget::from_weighted_limit(units, 7, 3)),
        )
    };
    let exact_units = 4 * 7 + bytes * 3;
    let (output, stats) = run(exact_units).expect("offset, page and probe fit exactly");
    assert_eq!(stats.processed_items(), 4);
    assert_eq!(stats.processed_bytes(), bytes);
    assert_eq!(output.remaining_items, None);
    assert!(output.has_more);
    assert!(matches!(
        run(exact_units - 1),
        Err(Error::GasBudgetExceeded)
    ));
}
