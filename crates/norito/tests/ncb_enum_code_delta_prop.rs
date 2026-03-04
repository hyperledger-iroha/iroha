//! Property tests for enum NCB with code-delta sequences (alternating/wrap patterns).

use norito::columnar as ncb;
use proptest::prelude::*;

fn small_name() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<u8>(), 0..8).prop_map(|bs| {
        bs.into_iter()
            .map(|b| ((b % 26) + b'a') as char)
            .collect::<String>()
    })
}

proptest! {
    // Alternating Name/Code, small deltas, stable decode under code-delta
    #[test]
    fn prop_enum_code_delta_alternating(
        n in 1usize..128,
        name in small_name(),
        base in any::<u32>(),
    ) {
        let n = n.min(128);
        let mut rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = Vec::with_capacity(n);
        let mut expected: Vec<(u64, ncb::RowEnumOwned, bool)> = Vec::with_capacity(n);
        let mut cur = base;
        for i in 0..n {
            let id = (i as u64) * 7 + 1;
            if i % 2 == 0 {
                rows.push((id, ncb::EnumBorrow::Name(&name), i % 3 == 0));
                expected.push((id, ncb::RowEnumOwned::Name(name.clone()), i % 3 == 0));
            } else {
                cur = cur.wrapping_add(1);
                rows.push((id, ncb::EnumBorrow::Code(cur), i % 3 == 0));
                expected.push((id, ncb::RowEnumOwned::Code(cur), i % 3 == 0));
            }
        }
        // Encode with code-delta forced ON for Code variants
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, /*id-delta*/ false, /*dict*/ false, /*code-delta*/ true);
        // Misalignment stress
        let mut pref = vec![0xEE, 0xEE, 0xEE];
        pref.extend_from_slice(&bytes);
        let view = ncb::view_ncb_u64_enum_bool(&pref[3..]).expect("view enum ncb");
        prop_assert_eq!(view.len(), n);
        for (i, _) in expected.iter().enumerate().take(n) {
            let (eid, ep, ef) = (&expected[i].0, &expected[i].1, &expected[i].2);
            prop_assert_eq!(view.id(i), *eid);
            prop_assert_eq!(view.flag(i), *ef);
            match ep {
                ncb::RowEnumOwned::Name(s) => {
                    prop_assert_eq!(view.tag(i), 0);
                    match view.payload(i).unwrap() {
                        ncb::ColEnumRef::Name(ns) => prop_assert_eq!(ns, s.as_str()),
                        _ => prop_assert!(false, "expected name payload"),
                    }
                }
                ncb::RowEnumOwned::Code(v) => {
                    prop_assert_eq!(view.tag(i), 1);
                    match view.payload(i).unwrap() {
                        ncb::ColEnumRef::Code(cv) => prop_assert_eq!(cv, *v),
                        _ => prop_assert!(false, "expected code payload"),
                    }
                }
            }
        }
    }

    // Code-only with wrap-around patterns
    #[test]
    fn prop_enum_code_delta_wrap(
        n in 1usize..256,
        base in any::<u32>(),
        step in any::<u16>(),
    ) {
        let n = n.min(128);
        let step: u32 = (step as u32) | 1; // ensure movement; keep deltas small
        let mut rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = Vec::with_capacity(n);
        let mut expected: Vec<(u64, ncb::RowEnumOwned, bool)> = Vec::with_capacity(n);
        let mut cur = base;
        for i in 0..n {
            let id = (i as u64) * 11 + 5;
            cur = cur.wrapping_add(step);
            rows.push((id, ncb::EnumBorrow::Code(cur), i % 5 == 0));
            expected.push((id, ncb::RowEnumOwned::Code(cur), i % 5 == 0));
        }
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, false, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view wrap");
        prop_assert_eq!(view.len(), n);
        for (i, _) in expected.iter().enumerate().take(n) {
            prop_assert_eq!(view.id(i), expected[i].0);
            prop_assert_eq!(view.flag(i), expected[i].2);
            prop_assert_eq!(view.tag(i), 1);
            match view.payload(i).unwrap() {
                ncb::ColEnumRef::Code(cv) => {
                    let ev = base.wrapping_add(step.wrapping_mul((i as u32).wrapping_add(1)));
                    prop_assert_eq!(cv, ev);
                }
                _ => prop_assert!(false, "expected code payload"),
            }
        }
    }
}

proptest! {
    // Combined deltas: id-delta + code-delta, no dict names; validate view decode
    #[test]
    fn prop_enum_code_and_id_delta(
        n in 2usize..128,
        base in any::<u32>(),
    ) {
        let n = n.min(64);
        // Build rows: every 3rd is Name("aa") (len<8 to avoid dict); others are Code with +1 deltas
        let mut rows: Vec<(u64, ncb::EnumBorrow<'_>, bool)> = Vec::with_capacity(n);
        let mut expected: Vec<(u64, ncb::RowEnumOwned, bool)> = Vec::with_capacity(n);
        let mut code = base;
        for i in 0..n {
            let id = (i as u64) * 2 + 9; // monotonic to trigger id-delta
            if i % 3 == 0 {
                rows.push((id, ncb::EnumBorrow::Name("aa"), i % 2 == 0));
                expected.push((id, ncb::RowEnumOwned::Name("aa".to_string()), i % 2 == 0));
            } else {
                code = code.wrapping_add(1);
                rows.push((id, ncb::EnumBorrow::Code(code), i % 2 == 0));
                expected.push((id, ncb::RowEnumOwned::Code(code), i % 2 == 0));
            }
        }
        // Force both deltas; names not dict-coded by construction
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view enum id+code delta");
        prop_assert_eq!(view.len(), n);
        for (i, _) in expected.iter().enumerate().take(n) {
            prop_assert_eq!(view.id(i), expected[i].0);
            prop_assert_eq!(view.flag(i), expected[i].2);
            match &expected[i].1 {
                ncb::RowEnumOwned::Name(s) => {
                    prop_assert_eq!(view.tag(i), 0);
                    match view.payload(i).unwrap() {
                        ncb::ColEnumRef::Name(ns) => prop_assert_eq!(ns, s.as_str()),
                        _ => prop_assert!(false, "expected name payload"),
                    }
                }
                ncb::RowEnumOwned::Code(v) => {
                    prop_assert_eq!(view.tag(i), 1);
                    match view.payload(i).unwrap() {
                        ncb::ColEnumRef::Code(cv) => prop_assert_eq!(cv, *v),
                        _ => prop_assert!(false, "expected code payload"),
                    }
                }
            }
        }
    }
}
//
