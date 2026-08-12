use iroha_primitives::numeric::Numeric;
use norito::codec::Encode;

use super::*;

fn numeric(values: &[u32]) -> QueryOutputBatchBox {
    QueryOutputBatchBox::Numeric(values.iter().copied().map(Numeric::from).collect())
}

#[test]
fn construction_rejects_missing_columns() {
    assert_eq!(
        QueryOutputBatchBoxTuple::new(Vec::new()),
        Err(QueryOutputBatchBoxTupleError::NoColumns)
    );
}

#[test]
fn construction_rejects_every_unequal_column_position() {
    for (expected_column, tuple) in [
        (1, vec![numeric(&[1]), numeric(&[])]),
        (2, vec![numeric(&[1]), numeric(&[2]), numeric(&[])]),
        (2, vec![numeric(&[1]), numeric(&[2]), numeric(&[3, 4])]),
    ] {
        assert!(matches!(
            QueryOutputBatchBoxTuple::new(tuple),
            Err(QueryOutputBatchBoxTupleError::ColumnLengthMismatch {
                column,
                ..
            }) if column == expected_column
        ));
    }
}

#[test]
fn construction_allows_equal_zero_row_columns() {
    let batch = QueryOutputBatchBoxTuple::new(vec![numeric(&[]), numeric(&[])])
        .expect("equal empty columns are a valid zero-row page");
    assert_eq!(batch.column_count(), 2);
    assert_eq!(batch.len(), 0);
    assert!(batch.is_empty());
}

#[test]
fn extend_is_atomic_on_count_or_type_mismatch() {
    let original = QueryOutputBatchBoxTuple::from_batch(numeric(&[1]));

    let mut count_mismatch = original.clone();
    let two_columns = QueryOutputBatchBoxTuple::new(vec![numeric(&[2]), numeric(&[3])])
        .expect("equal column lengths");
    assert_eq!(
        count_mismatch.extend(two_columns),
        Err(QueryOutputBatchBoxTupleError::ColumnCountMismatch {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(count_mismatch, original);

    let mut type_mismatch = original.clone();
    let strings =
        QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(vec!["two".to_owned()]));
    assert_eq!(
        type_mismatch.extend(strings),
        Err(QueryOutputBatchBoxTupleError::ColumnTypeMismatch { column: 0 })
    );
    assert_eq!(type_mismatch, original);

    let mut late_mismatch = QueryOutputBatchBoxTuple::new(vec![
        numeric(&[1]),
        QueryOutputBatchBox::String(vec!["one".to_owned()]),
    ])
    .expect("equal column lengths");
    let late_snapshot = late_mismatch.clone();
    let appended = QueryOutputBatchBoxTuple::new(vec![numeric(&[2]), numeric(&[3])])
        .expect("equal column lengths");
    assert_eq!(
        late_mismatch.extend(appended),
        Err(QueryOutputBatchBoxTupleError::ColumnTypeMismatch { column: 1 })
    );
    assert_eq!(late_mismatch, late_snapshot, "preflight must be atomic");
}

#[test]
fn extend_preserves_equal_column_lengths() {
    let mut left = QueryOutputBatchBoxTuple::new(vec![numeric(&[1]), numeric(&[10])])
        .expect("equal column lengths");
    let right = QueryOutputBatchBoxTuple::new(vec![numeric(&[2, 3]), numeric(&[20, 30])])
        .expect("equal column lengths");

    left.extend(right).expect("matching tuples extend");

    assert_eq!(left.len(), 3);
    assert!(left.iter().all(|column| column.len() == 3));
}

#[test]
fn erased_batch_extend_is_total_and_supports_json() {
    assert!(QueryOutputBatchBox::Json(Vec::new()).is_empty());
    let mut left = QueryOutputBatchBox::Json(vec![Json::from(norito::json!({ "a": 1 }))]);
    left.extend(QueryOutputBatchBox::Json(vec![Json::from(
        norito::json!({ "b": 2 }),
    )]))
    .expect("matching JSON batches extend");
    assert_eq!(left.len(), 2);
    assert!(!left.is_empty());

    let snapshot = left.clone();
    assert_eq!(
        left.extend(numeric(&[3])),
        Err(QueryOutputBatchBoxTypeMismatch)
    );
    assert_eq!(left, snapshot, "type mismatch must not mutate the batch");
}

#[test]
fn norito_decode_rejects_missing_and_unequal_columns() {
    for candidate in [
        QueryOutputBatchBoxTupleCandidate { tuple: Vec::new() },
        QueryOutputBatchBoxTupleCandidate {
            tuple: vec![numeric(&[1]), numeric(&[])],
        },
        QueryOutputBatchBoxTupleCandidate {
            tuple: vec![numeric(&[]), numeric(&[1])],
        },
    ] {
        let encoded = candidate.encode();
        let error = norito::codec::decode_adaptive::<QueryOutputBatchBoxTuple>(&encoded)
            .expect_err("malformed columns must be rejected during decode");
        assert!(
            matches!(error, norito::core::Error::Message(_)),
            "unexpected error: {error:?}"
        );
        let exact_error =
            norito::codec::decode_exact_from_slice::<QueryOutputBatchBoxTuple>(&encoded)
                .expect_err("exact slice decode must enforce column invariants");
        assert!(
            matches!(exact_error, norito::core::Error::Message(_)),
            "unexpected exact-decode error: {exact_error:?}"
        );
    }
}

#[test]
fn exact_slice_decode_roundtrips_valid_columns() {
    let batch = QueryOutputBatchBoxTuple::new(vec![numeric(&[1, 2]), numeric(&[3, 4])])
        .expect("equal column lengths");
    let encoded = batch.encode();
    let decoded = norito::codec::decode_exact_from_slice::<QueryOutputBatchBoxTuple>(&encoded)
        .expect("valid exact slice decode");
    assert_eq!(decoded, batch);
}

#[test]
fn query_output_norito_decode_rejects_hostile_nested_batch() {
    #[derive(Encode)]
    struct QueryOutputCandidate {
        batch: QueryOutputBatchBoxTupleCandidate,
        remaining_items: Option<u64>,
        has_more: bool,
        continue_cursor: Option<ForwardCursor>,
    }

    let hostile = QueryOutputCandidate {
        batch: QueryOutputBatchBoxTupleCandidate {
            tuple: vec![numeric(&[1]), numeric(&[])],
        },
        remaining_items: Some(0),
        has_more: false,
        continue_cursor: None,
    };
    let error = norito::codec::decode_adaptive::<QueryOutput>(&hostile.encode())
        .expect_err("hostile nested batch must fail query-output decode");
    assert!(error.to_string().to_lowercase().contains("column"));
}

#[test]
fn norito_decode_rejects_every_truncated_prefix_and_trailing_data() {
    let batch = QueryOutputBatchBoxTuple::new(vec![numeric(&[1, 2]), numeric(&[3, 4])])
        .expect("equal column lengths");
    let encoded = batch.encode();

    for cut in 0..encoded.len() {
        assert!(
            norito::codec::decode_adaptive::<QueryOutputBatchBoxTuple>(&encoded[..cut]).is_err(),
            "truncated payload of {cut}/{} bytes decoded successfully",
            encoded.len()
        );
    }

    let mut with_trailing_data = encoded;
    with_trailing_data.extend_from_slice(&[0xA5, 0x5A]);
    assert!(
        norito::codec::decode_adaptive::<QueryOutputBatchBoxTuple>(&with_trailing_data).is_err(),
        "trailing bytes must not be accepted"
    );
}

#[cfg(feature = "json")]
#[test]
fn json_decode_rejects_hostile_column_shapes() {
    let nonempty = norito::json::to_value(&numeric(&[1])).expect("serialize batch column");
    let empty = norito::json::to_value(&numeric(&[])).expect("serialize empty batch column");
    for value in [
        norito::json!({ "tuple": [] }),
        norito::json!({ "tuple": [nonempty, empty] }),
    ] {
        let error = norito::json::from_value::<QueryOutputBatchBoxTuple>(value)
            .expect_err("hostile JSON response must be rejected");
        assert!(error.to_string().to_lowercase().contains("column"));
    }
}

#[cfg(feature = "json")]
#[test]
fn query_response_json_rejects_hostile_batch_before_iteration() {
    let nonempty = norito::json::to_value(&numeric(&[1])).expect("serialize batch column");
    let empty = norito::json::to_value(&numeric(&[])).expect("serialize empty batch column");
    let response = norito::json!({
        "kind": "Iterable",
        "content": {
            "batch": { "tuple": [nonempty, empty] },
            "remaining_items": 0,
            "has_more": false,
            "continue_cursor": null
        }
    });

    let error = norito::json::from_value::<QueryResponse>(response)
        .expect_err("hostile query response must fail at its decode boundary");
    assert!(error.to_string().to_lowercase().contains("column"));
}
