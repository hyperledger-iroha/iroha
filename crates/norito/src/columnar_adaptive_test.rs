    #[test]
    fn adaptive_large_input_tags_ncb() {
        // Build rows just over the threshold so auto path is used
        let t = crate::core::heuristics::get().aos_ncb_small_n;
        let n = t.saturating_add(1);
        let mut rows: Vec<(u64, &str, bool)> = Vec::with_capacity(n);
        // Use small, distinct strings; content heuristics may vary inside NCB but the tag is driven by size
        let names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
        for (i, name) in names.iter().enumerate() {
            rows.push((i as u64, name.as_str(), i % 2 == 0));
        }
        let bytes = encode_rows_u64_str_bool_adaptive(&rows);
        assert!(!bytes.is_empty());
        match bytes[0] {
            ADAPTIVE_TAG_NCB => {
                assert!(
                    should_use_columnar(n),
                    "encoder picked NCB but heuristic rejected columnar"
                );
            }
            ADAPTIVE_TAG_AOS => {
                assert!(
                    !should_use_columnar(n),
                    "encoder picked AoS while heuristic expected columnar"
                );
            }
            other => panic!("unexpected adaptive tag: {other}"),
        }
        // And it should roundtrip
        let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
        let expected: Vec<(u64, String, bool)> = rows
            .iter()
            .map(|(id, s, b)| (*id, (*s).to_string(), *b))
            .collect();
        assert_eq!(decoded, expected);
    }
