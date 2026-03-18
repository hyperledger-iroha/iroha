//! Property tests for optional columns and NCB invariants.

use norito::columnar::*;
use proptest::prelude::*;

// Strategy helpers
fn small_string() -> impl Strategy<Value = String> {
    // ASCII lowercase letters, including empty string
    prop::collection::vec(any::<u8>(), 0..20).prop_map(|bs| {
        bs.into_iter()
            .map(|b| ((b % 26) + b'a') as char)
            .collect::<String>()
    })
}

proptest! {
    // Random presence distributions and misalignment stress for Option<&str>
    #[test]
    fn prop_optstr_invariants(
        n in 0usize..180,
        // Nearly 50% presence; distribution varied
        present in prop::collection::vec(any::<bool>(), 0..180),
        names in prop::collection::vec(small_string(), 0..180),
        flags in prop::collection::vec(any::<bool>(), 0..180),
        step in 0u8..3, // id step for monotone drift
    ) {
        let n = n.min(180);
        let mut ids = Vec::with_capacity(n);
        let mut cur: u64 = 0;
        for _ in 0..n { cur = cur.wrapping_add(step as u64); ids.push(cur); }

        let mut rows_owned: Vec<Option<String>> = Vec::with_capacity(n);
        let mut idx_name = 0usize;
        for i in 0..n {
            let p = present.get(i).copied().unwrap_or(false);
            let s = if p {
                let nm = names.get(idx_name).cloned().unwrap_or_default();
                idx_name += 1;
                Some(nm)
            } else { None };
            rows_owned.push(s);
        }
        let mut rows_flags: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n { rows_flags.push(flags.get(i).copied().unwrap_or(false)); }

        // Build borrowed rows
        let mut borrowed: Vec<(u64, Option<&str>, bool)> = Vec::with_capacity(n);
        for i in 0..n {
            let sref = rows_owned[i].as_deref();
            borrowed.push((ids[i], sref, rows_flags[i]));
        }

        // Encode NCB directly and view
        let ncb = encode_ncb_u64_optstr_bool(&borrowed);
        let view = view_ncb_u64_optstr_bool(&ncb).expect("view optstr");
        prop_assert_eq!(view.len(), n);
        // Check roundtrip row-wise and rank mapping consistency
        for i in 0..n {
            prop_assert_eq!(view.id(i), ids[i]);
            let got = view.name(i).expect("name call ok");
            let exp = borrowed[i].1;
            match (got, exp) {
                (Some(gs), Some(es)) => prop_assert_eq!(gs, es),
                (None, None) => {},
                _ => prop_assert!(false, "presence mismatch at {i}"),
            }
            prop_assert_eq!(view.flag(i), rows_flags[i]);
        }

        // Misalignment stress: prefix with 1 byte and parse again
        let mut pref = Vec::with_capacity(ncb.len() + 1);
        pref.push(0xCC);
        pref.extend_from_slice(&ncb);
        let view2 = view_ncb_u64_optstr_bool(&pref[1..]).expect("view misaligned optstr");
        prop_assert_eq!(view2.len(), n);
        for i in 0..n {
            // Accessors must still agree
            prop_assert_eq!(view2.id(i), ids[i]);
            let got = view2.name(i).expect("name call ok");
            let exp = borrowed[i].1;
            match (got, exp) {
                (Some(gs), Some(es)) => prop_assert_eq!(gs, es),
                (None, None) => {},
                _ => prop_assert!(false, "presence mismatch (misaligned) at {i}"),
            }
            prop_assert_eq!(view2.flag(i), rows_flags[i]);
        }
    }

    // Random presence distributions and misalignment stress for Option<u32>
    #[test]
    fn prop_optu32_invariants(
        n in 0usize..180,
        present in prop::collection::vec(any::<bool>(), 0..180),
        values in prop::collection::vec(any::<u32>(), 0..180),
        flags in prop::collection::vec(any::<bool>(), 0..180),
        step in 0u8..3,
    ) {
        let n = n.min(180);
        let mut ids = Vec::with_capacity(n);
        let mut cur: u64 = 0;
        for _ in 0..n { cur = cur.wrapping_add(step as u64); ids.push(cur); }

        let mut rows_vals: Vec<Option<u32>> = Vec::with_capacity(n);
        let mut idx = 0usize;
        for i in 0..n {
            let p = present.get(i).copied().unwrap_or(false);
            let v = if p { Some(values.get(idx).copied().unwrap_or(0)) } else { None };
            if p { idx += 1; }
            rows_vals.push(v);
        }
        let mut rows_flags: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n { rows_flags.push(flags.get(i).copied().unwrap_or(false)); }

        // Encode NCB directly and view
        let rows: Vec<(u64, Option<u32>, bool)> = (0..n).map(|i| (ids[i], rows_vals[i], rows_flags[i])).collect();
        let ncb = encode_ncb_u64_optu32_bool(&rows);
        let view = view_ncb_u64_optu32_bool(&ncb).expect("view optu32");
        prop_assert_eq!(view.len(), n);
        for i in 0..n {
            prop_assert_eq!(view.id(i), ids[i]);
            prop_assert_eq!(view.val(i), rows_vals[i]);
            prop_assert_eq!(view.flag(i), rows_flags[i]);
        }

        // Misalignment stress
        let mut pref = Vec::with_capacity(ncb.len() + 2);
        pref.extend_from_slice(&[0xAA, 0xBB]);
        pref.extend_from_slice(&ncb);
        let view2 = view_ncb_u64_optu32_bool(&pref[2..]).expect("view misaligned optu32");
        prop_assert_eq!(view2.len(), n);
        for i in 0..n {
            prop_assert_eq!(view2.id(i), ids[i]);
            prop_assert_eq!(view2.val(i), rows_vals[i]);
            prop_assert_eq!(view2.flag(i), rows_flags[i]);
        }
    }
}
