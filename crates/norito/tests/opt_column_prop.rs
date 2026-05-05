//! Deterministic tests for optional columns and NCB invariants.

use norito::columnar::*;

#[test]
fn optstr_invariants() {
    let cases: Vec<Vec<(u64, Option<&str>, bool)>> = vec![
        Vec::new(),
        vec![(0, None, false)],
        vec![(1, Some(""), true), (2, None, false), (3, Some("aa"), true)],
        vec![
            (10, Some("alpha"), true),
            (10, Some("beta"), false),
            (12, None, true),
            (14, Some("gamma"), false),
        ],
    ];

    for rows in cases {
        let ncb = encode_ncb_u64_optstr_bool(&rows);
        let view = view_ncb_u64_optstr_bool(&ncb).expect("view optstr");
        assert_eq!(view.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(view.id(idx), row.0);
            assert_eq!(view.name(idx).expect("name call ok"), row.1);
            assert_eq!(view.flag(idx), row.2);
        }

        let mut prefixed = Vec::with_capacity(ncb.len() + 1);
        prefixed.push(0xCC);
        prefixed.extend_from_slice(&ncb);
        let misaligned = view_ncb_u64_optstr_bool(&prefixed[1..]).expect("view misaligned optstr");
        assert_eq!(misaligned.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(misaligned.id(idx), row.0);
            assert_eq!(misaligned.name(idx).expect("name call ok"), row.1);
            assert_eq!(misaligned.flag(idx), row.2);
        }
    }
}

#[test]
fn optu32_invariants() {
    let cases: Vec<Vec<(u64, Option<u32>, bool)>> = vec![
        Vec::new(),
        vec![(0, None, false)],
        vec![
            (1, Some(0), true),
            (2, None, false),
            (3, Some(u32::MAX), true),
        ],
        vec![
            (10, Some(42), true),
            (10, Some(43), false),
            (12, None, true),
            (14, Some(44), false),
        ],
    ];

    for rows in cases {
        let ncb = encode_ncb_u64_optu32_bool(&rows);
        let view = view_ncb_u64_optu32_bool(&ncb).expect("view optu32");
        assert_eq!(view.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(view.id(idx), row.0);
            assert_eq!(view.val(idx), row.1);
            assert_eq!(view.flag(idx), row.2);
        }

        let mut prefixed = Vec::with_capacity(ncb.len() + 2);
        prefixed.extend_from_slice(&[0xAA, 0xBB]);
        prefixed.extend_from_slice(&ncb);
        let misaligned = view_ncb_u64_optu32_bool(&prefixed[2..]).expect("view misaligned optu32");
        assert_eq!(misaligned.len(), rows.len());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(misaligned.id(idx), row.0);
            assert_eq!(misaligned.val(idx), row.1);
            assert_eq!(misaligned.flag(idx), row.2);
        }
    }
}
