//! Tests for experimental Norito Column Blocks (NCB) prototype.
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::needless_range_loop)]

use norito::{
    columnar::{
        encode_ncb_u64_str_bool, encode_ncb_u64_str_bool_force_dict,
        encode_ncb_u64_str_bool_no_dict, materialize_ncb, should_use_columnar,
        view_ncb_u64_str_bool,
    },
    core,
};

#[test]
fn ncb_roundtrip_u64_str_bool() {
    let rows: Vec<(u64, String, bool)> = vec![
        (1, "alice".to_string(), true),
        (2, "bob".to_string(), false),
        (3, "carol".to_string(), true),
        (4, "dave".to_string(), false),
        (5, "eve".to_string(), true),
    ];
    let tuples: Vec<(u64, &str, bool)> =
        rows.iter().map(|(i, s, b)| (*i, s.as_str(), *b)).collect();

    // Encode NCB and create a view
    let bytes = encode_ncb_u64_str_bool(&tuples);
    let view = view_ncb_u64_str_bool(&bytes).expect("view");

    assert_eq!(view.len(), rows.len());
    for i in 0..rows.len() {
        assert_eq!(view.id(i), rows[i].0);
        assert_eq!(view.name(i).unwrap(), rows[i].1.as_str());
        assert_eq!(view.flag(i), rows[i].2);
    }

    // Materialize back into owned tuples
    let owned = materialize_ncb(view).expect("materialize");
    assert_eq!(owned, rows);
}

#[test]
fn ncb_threshold_heuristic() {
    let h = core::heuristics::get();
    assert!(!should_use_columnar(0));
    if h.aos_ncb_small_n > 0 {
        assert!(
            !should_use_columnar(h.aos_ncb_small_n - 1),
            "rows below threshold should stay AoS"
        );
    }
    assert!(
        !should_use_columnar(h.aos_ncb_small_n),
        "threshold row count should still use AoS/NCB two-pass"
    );
    assert!(
        should_use_columnar(h.aos_ncb_small_n.saturating_add(1)),
        "rows beyond threshold should enable columnar auto-selection"
    );
}

#[test]
fn ncb_column_iterators_offsets() {
    let rows: Vec<(u64, &str, bool)> = vec![
        (10, "alice", true),
        (20, "bob", false),
        (30, "carol", true),
        (40, "dave", false),
    ];
    let bytes = encode_ncb_u64_str_bool_no_dict(&rows);
    let view = view_ncb_u64_str_bool(&bytes).unwrap();
    // ids
    let ids: Vec<u64> = view.iter_ids().collect();
    assert_eq!(ids, rows.iter().map(|r| r.0).collect::<Vec<_>>());
    // names
    let names: Vec<&str> = view.iter_names().collect();
    assert_eq!(names, rows.iter().map(|r| r.1).collect::<Vec<_>>());
    // flags
    let flags: Vec<bool> = view.iter_flags().collect();
    assert_eq!(flags, rows.iter().map(|r| r.2).collect::<Vec<_>>());
}

#[test]
fn ncb_column_iterators_dict() {
    let rows: Vec<(u64, &str, bool)> = vec![
        (1, "x", true),
        (2, "y", false),
        (3, "x", true),
        (4, "y", false),
        (5, "x", true),
    ];
    let bytes = encode_ncb_u64_str_bool_force_dict(&rows);
    let view = view_ncb_u64_str_bool(&bytes).unwrap();
    let names: Vec<&str> = view.iter_names().collect();
    assert_eq!(names, rows.iter().map(|r| r.1).collect::<Vec<_>>());
}

#[test]
fn ncb_column_iterators_flag_true() {
    let rows: Vec<(u64, &str, bool)> = vec![
        (1, "a", true),
        (2, "b", false),
        (3, "cc", true),
        (4, "d", false),
        (5, "eee", true),
    ];
    let bytes = encode_ncb_u64_str_bool_no_dict(&rows);
    let view = view_ncb_u64_str_bool(&bytes).unwrap();
    let ids_dense: Vec<u64> = view.iter_ids_flag_true().collect();
    let ids_rowwise: Vec<u64> = (0..view.len())
        .filter(|&i| view.flag(i))
        .map(|i| view.id(i))
        .collect();
    assert_eq!(ids_dense, ids_rowwise);
    let names_dense: Vec<&str> = view.iter_names_flag_true().collect();
    let names_rowwise: Vec<&str> = (0..view.len())
        .filter(|&i| view.flag(i))
        .map(|i| view.name(i).unwrap())
        .collect();
    assert_eq!(names_dense, names_rowwise);
    let names_dense64: Vec<&str> = view.iter_names_flag_true_popcount64().collect();
    assert_eq!(names_dense64, names_rowwise);
    let pos_simple: Vec<usize> = view.iter_true_positions().collect();
    let d: Vec<usize> = view.iter_true_positions_popcount64_aligned().collect();
    assert_eq!(pos_simple, d);
    let names_dense64a: Vec<&str> = view.iter_names_flag_true_popcount64_aligned().collect();
    assert_eq!(names_dense64a, names_rowwise);
}

#[test]
fn ncb_true_positions_popcount_matches_simple() {
    // Construct rows with irregular flags to exercise both paths
    let rows: Vec<(u64, &str, bool)> = (0..65)
        .map(|i| {
            (
                i as u64,
                if i % 5 == 0 { "aaa" } else { "b" },
                (i % 3) == 0 || (i % 7) == 0,
            )
        })
        .collect();
    let bytes = encode_ncb_u64_str_bool_no_dict(&rows);
    let view = view_ncb_u64_str_bool(&bytes).unwrap();
    let a: Vec<usize> = view.iter_true_positions().collect();
    let b: Vec<usize> = view.iter_true_positions_popcount().collect();
    assert_eq!(a, b);
    let c: Vec<usize> = view.iter_true_positions_popcount64().collect();
    assert_eq!(a, c);
    // Names via popcount vs simple filtered
    let names_dense: Vec<&str> = view.iter_names_flag_true_popcount().collect();
    let names_rowwise: Vec<&str> = (0..view.len())
        .filter(|&i| view.flag(i))
        .map(|i| view.name(i).unwrap())
        .collect();
    assert_eq!(names_dense, names_rowwise);
    let names_dense64: Vec<&str> = view.iter_names_flag_true_popcount64().collect();
    assert_eq!(names_dense64, names_rowwise);
    let d: Vec<usize> = view.iter_true_positions_popcount64_aligned().collect();
    assert_eq!(a, d);
    let names_dense64a: Vec<&str> = view.iter_names_flag_true_popcount64_aligned().collect();
    assert_eq!(names_dense64a, names_rowwise);
}
