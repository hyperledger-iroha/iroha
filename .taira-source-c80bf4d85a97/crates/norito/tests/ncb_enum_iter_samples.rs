//! Iterator sample checks over NCB enum datasets for various toggle combos.

use norito::columnar as ncb;

#[test]
fn offsets_id_code_delta_iter_samples() {
    use ncb::EnumBorrow;
    // id-delta ON, code-delta ON, offsets-based names (no dict)
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("p"), true),
        (101, EnumBorrow::Code(7), false),
        (103, EnumBorrow::Code(9), true), // flag true code
        (104, EnumBorrow::Name("q"), true),
        (110, EnumBorrow::Code(9), false),
        (111, EnumBorrow::Name("r"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Spot-check ids and payloads
    assert_eq!(view.id(0), 100);
    assert_eq!(view.id(2), 103);
    match view.payload(0).expect("p0") {
        ncb::ColEnumRef::Name(s) => assert_eq!(s, "p"),
        _ => panic!("exp Name"),
    }
    match view.payload(2).expect("p2") {
        ncb::ColEnumRef::Code(v) => assert_eq!(v, 9),
        _ => panic!("exp Code"),
    }

    // Collect names and codes at flag=true via iterators, compare with naive
    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    let names_fast: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_fast: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(names_fast, names_true_naive);
    assert_eq!(codes_fast, codes_true_naive);
    // Indexed variants should match too
    let names_idx: Vec<String> = view
        .iter_names_flag_true_indexed()
        .map(|s| s.to_string())
        .collect();
    let codes_idx: Vec<u32> = view.iter_codes_flag_true_indexed().collect();
    assert_eq!(names_idx, names_true_naive);
    assert_eq!(codes_idx, codes_true_naive);
}

#[test]
fn dict_id_code_delta_iter_samples() {
    use ncb::EnumBorrow;
    // id-delta ON, dict ON, code-delta ON
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (3, EnumBorrow::Name("bb"), false),
        (4, EnumBorrow::Code(10), true),
        (10, EnumBorrow::Name("aa"), true),
        (11, EnumBorrow::Code(12), false),
        (20, EnumBorrow::Name("cc"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Spot-check names via dict and codes via delta
    match view.payload(0).expect("p0") {
        ncb::ColEnumRef::Name(s) => assert_eq!(s, "aa"),
        _ => panic!("exp Name"),
    }
    match view.payload(2).expect("p2") {
        ncb::ColEnumRef::Code(v) => assert_eq!(v, 10),
        _ => panic!("exp Code"),
    }
    match view.payload(3).expect("p3") {
        ncb::ColEnumRef::Name(s) => assert_eq!(s, "aa"),
        _ => panic!("exp Name"),
    }

    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    let names_fast: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_fast: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(names_fast, names_true_naive);
    assert_eq!(codes_fast, codes_true_naive);
    let names_idx: Vec<String> = view
        .iter_names_flag_true_indexed()
        .map(|s| s.to_string())
        .collect();
    let codes_idx: Vec<u32> = view.iter_codes_flag_true_indexed().collect();
    assert_eq!(names_idx, names_true_naive);
    assert_eq!(codes_idx, codes_true_naive);
}

#[test]
fn offsets_repeating_names_zero_delta_codes() {
    use ncb::EnumBorrow;
    // Repeating names and zero-delta code streaks; offsets, id-delta ON, code-delta ON
    // Names at positions 0,3,6,9 are "xx"; codes have streaks 200 (x4), then 201 (x3)
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("xx"), true),
        (101, EnumBorrow::Code(200), true),
        (102, EnumBorrow::Code(200), false),
        (103, EnumBorrow::Name("xx"), true),
        (104, EnumBorrow::Code(200), true),
        (105, EnumBorrow::Code(200), true),
        (106, EnumBorrow::Name("xx"), false),
        (107, EnumBorrow::Code(201), true),
        (108, EnumBorrow::Code(201), false),
        (109, EnumBorrow::Name("xx"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Naive collection for flag=true
    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    // Expect names at indices 0,3,9 (flags true) and codes from flagged indices (1,4,5,7)
    assert_eq!(names_true_naive, vec!["xx", "xx", "xx"]);
    assert_eq!(codes_true_naive, vec![200, 200, 200, 201]);

    // Iterators must match naive lists and subsequences
    let names_fast: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_fast: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(names_fast, names_true_naive);
    assert_eq!(codes_fast, codes_true_naive);
    let codes_slice: Vec<u32> = view.iter_codes_flag_true_fast().skip(1).take(2).collect();
    assert_eq!(codes_slice, vec![200, 200]);
}

#[test]
fn dict_repeating_names_zero_delta_codes() {
    use ncb::EnumBorrow;
    // Dict ON with repeating names; code streaks 50 (x3) then 51 (x2), various flags
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(50), true),
        (4, EnumBorrow::Name("bb"), false),
        (5, EnumBorrow::Code(50), true),
        (6, EnumBorrow::Name("aa"), true),
        (7, EnumBorrow::Code(51), false),
        (8, EnumBorrow::Code(51), true),
        (9, EnumBorrow::Name("bb"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Naive and iterator parity checks
    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    let names_fast: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_fast: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(names_fast, names_true_naive);
    assert_eq!(codes_fast, codes_true_naive);

    // Specific sample slices
    let names_slice: Vec<String> = view
        .iter_names_flag_true_fast()
        .take(2)
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names_slice, vec!["aa", "aa"]);
    let codes_slice: Vec<u32> = view.iter_codes_flag_true_fast().skip(2).take(1).collect();
    assert_eq!(codes_slice, vec![50]);
}

#[test]
fn offsets_long_repeats_zero_delta_codes() {
    use ncb::EnumBorrow;
    // Build 30 rows with long zero-delta code streaks and repeated names.
    // - id-delta ON, code-delta ON, offsets-based names (no dict)
    // - Codes: long run of 300, then long run of 301
    // - Names sprinkled at regular intervals with flag=true
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 100u64;
    for i in 0..30u64 {
        let flag = i % 3 != 1; // true for 2 out of 3 rows
        if i % 7 == 0 || i % 7 == 3 {
            // Place repeating names at regular positions
            let name = if i % 14 < 7 { "aa" } else { "bb" };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else if i < 18 {
            rows.push((id, EnumBorrow::Code(300), flag));
        } else {
            rows.push((id, EnumBorrow::Code(301), flag));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Collect iterator outputs
    let names_true: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_true: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    // Naive collection for parity
    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    assert_eq!(names_true, names_true_naive);
    assert_eq!(codes_true, codes_true_naive);

    // Windowed subsequence checks: ensure a window of three 300s exists and two 301s exist
    let has_three_300s = codes_true.windows(3).any(|w| w == [300, 300, 300]);
    assert!(
        has_three_300s,
        "expected a run of three 300s in codes_true: {codes_true:?}"
    );
    let has_two_301s = codes_true.windows(2).any(|w| w == [301, 301]);
    assert!(
        has_two_301s,
        "expected a run of two 301s in codes_true: {codes_true:?}"
    );

    // Reverse slice check: reverse codes and verify tail holds expected last repeated code
    if let Some(&last) = codes_true.last() {
        let rev_api: Vec<u32> = view.iter_codes_flag_true_fast_rev().collect();
        assert_eq!(rev_api[0], last);
        let rev_manual: Vec<u32> = codes_true.iter().copied().rev().collect();
        assert_eq!(rev_api, rev_manual);
    }
}

#[test]
fn dict_long_repeats_zero_delta_codes() {
    use ncb::EnumBorrow;
    // Similar to previous test but with dict ON; repeated names and long code streaks.
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 1u64;
    for i in 0..28u64 {
        let flag = i % 4 != 2; // true for 3 out of 4 rows
        if i % 6 == 0 || i % 6 == 4 {
            let name = if i % 12 < 6 { "aa" } else { "cc" };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else if i < 16 {
            rows.push((id, EnumBorrow::Code(50), flag));
        } else {
            rows.push((id, EnumBorrow::Code(51), flag));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    let names_true: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    let codes_true: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    let mut names_true_naive = Vec::new();
    let mut codes_true_naive = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => names_true_naive.push(s.to_string()),
                ncb::ColEnumRef::Code(v) => codes_true_naive.push(v),
            }
        }
    }
    assert_eq!(names_true, names_true_naive);
    assert_eq!(codes_true, codes_true_naive);

    // Windows: ensure at least three 50s and two 51s in a row somewhere
    assert!(codes_true.windows(3).any(|w| w == [50, 50, 50]));
    assert!(codes_true.windows(2).any(|w| w == [51, 51]));
    // Reverse names via API: first reversed equals last forward
    if let Some(last_name) = names_true.last() {
        let rev_api: Vec<String> = view.iter_names_flag_true_fast_rev().collect();
        assert_eq!(&rev_api[0], last_name);
        let rev_manual: Vec<String> = names_true.iter().cloned().rev().collect();
        assert_eq!(rev_api, rev_manual);
    }
}

#[test]
fn offsets_long_repeats_fixture() {
    use ncb::EnumBorrow;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 100u64;
    for i in 0..30u64 {
        let flag = i % 3 != 1;
        if i % 7 == 0 || i % 7 == 3 {
            let name = if i % 14 < 7 { "aa" } else { "bb" };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else if i < 18 {
            rows.push((id, EnumBorrow::Code(300), flag));
        } else {
            rows.push((id, EnumBorrow::Code(301), flag));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    // Compare to hex fixture
    use std::path::Path;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/enum_offsets_long_repeats_zero_delta_codes.hex");
    let hex = std::fs::read_to_string(&path).expect("read fixture");
    let hex = hex.trim();
    let r#gen = {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in &bytes {
            use core::fmt::Write as _;
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    };
    assert_eq!(r#gen, hex, "offsets long repeats fixture mismatch");
}

#[test]
fn dict_long_repeats_fixture() {
    use ncb::EnumBorrow;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 1u64;
    for i in 0..28u64 {
        let flag = i % 4 != 2;
        if i % 6 == 0 || i % 6 == 4 {
            let name = if i % 12 < 6 { "aa" } else { "cc" };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else if i < 16 {
            rows.push((id, EnumBorrow::Code(50), flag));
        } else {
            rows.push((id, EnumBorrow::Code(51), flag));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    use std::path::Path;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/enum_dict_long_repeats_zero_delta_codes.hex");
    let hex = std::fs::read_to_string(&path).expect("read fixture");
    let hex = hex.trim();
    let r#gen = {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in &bytes {
            use core::fmt::Write as _;
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    };
    assert_eq!(r#gen, hex, "dict long repeats fixture mismatch");
}

// ===== Helper predicates for regex-like pattern assertions over flag-true rows =====

#[derive(Debug, Clone, PartialEq, Eq)]
enum PayloadKind {
    Name(String),
    Code(u32),
}

fn collect_flag_true_payloads(view: &ncb::NcbU64EnumBoolView<'_>) -> Vec<PayloadKind> {
    let mut out = Vec::new();
    for i in 0..view.len() {
        if view.flag(i) {
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(s) => out.push(PayloadKind::Name(s.to_string())),
                ncb::ColEnumRef::Code(v) => out.push(PayloadKind::Code(v)),
            }
        }
    }
    out
}

fn has_alternating_name_code(
    seq: &[PayloadKind],
    expected_name: &str,
    expected_code: u32,
    min_pairs: usize,
) -> bool {
    if min_pairs == 0 {
        return true;
    }
    let n = seq.len();
    let window_len = min_pairs * 2;
    if n < window_len {
        return false;
    }
    for start in 0..=n - window_len {
        let mut ok = true;
        for k in 0..min_pairs {
            match &seq[start + 2 * k] {
                PayloadKind::Name(s) if s == expected_name => {}
                _ => {
                    ok = false;
                    break;
                }
            }
            match seq[start + 2 * k + 1] {
                PayloadKind::Code(v) if v == expected_code => {}
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return true;
        }
    }
    false
}

fn has_pattern_window(seq: &[PayloadKind], pattern: &[PayloadKind]) -> bool {
    if pattern.is_empty() || seq.len() < pattern.len() {
        return false;
    }
    for start in 0..=seq.len() - pattern.len() {
        let mut ok = true;
        for (j, p) in pattern.iter().enumerate() {
            let s = &seq[start + j];
            match (s, p) {
                (PayloadKind::Name(a), PayloadKind::Name(b)) => {
                    if a != b {
                        ok = false;
                        break;
                    }
                }
                (PayloadKind::Code(a), PayloadKind::Code(b)) => {
                    if a != b {
                        ok = false;
                        break;
                    }
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return true;
        }
    }
    false
}

// Predicate-based pattern matching
#[derive(Debug, Clone)]
enum Pred {
    NameEq(String),
    NameAny,
    CodeEq(u32),
    CodeGe(u32),
    // New richer variants for tests
    NameMatches(regex::Regex),
    CodeIn(core::ops::RangeInclusive<u32>),
}

fn has_pattern_window_pred(seq: &[PayloadKind], pattern: &[Pred]) -> bool {
    if pattern.is_empty() || seq.len() < pattern.len() {
        return false;
    }
    'outer: for start in 0..=seq.len() - pattern.len() {
        for (j, p) in pattern.iter().enumerate() {
            let s = &seq[start + j];
            let ok = match (s, p) {
                (PayloadKind::Name(a), Pred::NameEq(b)) => a == b,
                (PayloadKind::Name(_), Pred::NameAny) => true,
                (PayloadKind::Name(a), Pred::NameMatches(re)) => re.is_match(a),
                (PayloadKind::Code(a), Pred::CodeEq(b)) => a == b,
                (PayloadKind::Code(a), Pred::CodeGe(b)) => *a >= *b,
                (PayloadKind::Code(a), Pred::CodeIn(r)) => r.contains(a),
                _ => false,
            };
            if !ok {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

// Small builder helpers (function-based DSL)
fn p_name(s: &str) -> Pred {
    Pred::NameEq(s.to_string())
}
fn p_name_re(re: &str) -> Pred {
    Pred::NameMatches(regex::Regex::new(re).unwrap())
}
fn p_code(v: u32) -> Pred {
    Pred::CodeEq(v)
}
fn p_code_ge(v: u32) -> Pred {
    Pred::CodeGe(v)
}
fn p_code_in(start: u32, end: u32) -> Pred {
    Pred::CodeIn(start..=end)
}

// Helper to ensure a hex fixture exists and matches the generated bytes.
fn ensure_hex_fixture(rel_path: &str, bytes: &[u8]) {
    use std::{io::Write as _, path::Path};
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let generated = {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use core::fmt::Write as _;
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    };
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            let _ = writeln!(f, "{generated}");
            // If we just created it, consider it matching.
            return;
        }
    }
    let hex = std::fs::read_to_string(&path).expect("read fixture");
    let hex = hex.trim();
    assert_eq!(generated, hex, "fixture mismatch for {}", path.display());
}

#[test]
fn offsets_alternating_pattern_regex_like() {
    use ncb::EnumBorrow;
    // Build a pure alternating pattern: [Name("aa"), Code(300)] * 8 with all flags true.
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 10u64;
    for i in 0..16u64 {
        if i % 2 == 0 {
            rows.push((id, EnumBorrow::Name("aa"), true));
        } else {
            rows.push((id, EnumBorrow::Code(300), true));
        }
        id += 1;
    }
    // Turn on id+code deltas (names are offsets-based; no dict)
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);
    assert!(has_alternating_name_code(&seq, "aa", 300, 6)); // at least 6 pairs
}

#[test]
fn dict_alternating_pattern_regex_like() {
    use ncb::EnumBorrow;
    // Alternating with dict ON: [Name("zz"), Code(77)] * 5 over id-delta, code-delta, dict.
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::new();
    let mut id = 1u64;
    for i in 0..10u64 {
        if i % 2 == 0 {
            rows.push((id, EnumBorrow::Name("zz"), true));
        } else {
            rows.push((id, EnumBorrow::Code(77), true));
        }
        id += 1;
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);
    assert!(has_alternating_name_code(&seq, "zz", 77, 4));
}

#[test]
fn offsets_complex_window_pattern() {
    use ncb::EnumBorrow;
    // Construct a sequence containing the window [Name("aa"), Code(300), Code(300), Name("bb")]
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Code(299), false),
        (11, EnumBorrow::Name("aa"), true),
        (12, EnumBorrow::Code(300), true),
        (13, EnumBorrow::Code(300), true),
        (14, EnumBorrow::Name("bb"), true),
        (15, EnumBorrow::Code(301), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);
    let pat = [
        PayloadKind::Name("aa".to_string()),
        PayloadKind::Code(300),
        PayloadKind::Code(300),
        PayloadKind::Name("bb".to_string()),
    ];
    assert!(
        has_pattern_window(&seq, &pat),
        "sequence {seq:?} missing pattern"
    );
}

#[test]
fn offsets_nested_window_fixture() {
    use ncb::EnumBorrow;
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Code(299), false),
        (11, EnumBorrow::Name("aa"), true),
        (12, EnumBorrow::Code(300), true),
        (13, EnumBorrow::Code(300), true),
        (14, EnumBorrow::Name("bb"), true),
        (15, EnumBorrow::Code(300), true),
        (16, EnumBorrow::Code(301), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    use std::path::Path;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/enum_offsets_nested_window.hex");
    let hex = std::fs::read_to_string(&path).expect("read fixture");
    let hex = hex.trim();
    let r#gen = {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in &bytes {
            use core::fmt::Write as _;
            let _ = write!(&mut s, "{b:02x}");
        }
        s
    };
    assert_eq!(r#gen, hex, "offsets nested window fixture mismatch");
}

#[test]
fn dict_complex_window_pattern() {
    use ncb::EnumBorrow;
    // With dict ON, include window [Name("aa"), Code(50), Code(50), Name("bb")]
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(50), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Code(51), false),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);
    let pat = [
        PayloadKind::Name("aa".to_string()),
        PayloadKind::Code(50),
        PayloadKind::Code(50),
        PayloadKind::Name("bb".to_string()),
    ];
    assert!(
        has_pattern_window(&seq, &pat),
        "sequence {seq:?} missing pattern"
    );
}

#[test]
fn offsets_predicate_pattern_window() {
    use ncb::EnumBorrow;
    // Create sequence where a predicate pattern exists: Name("aa"), Code(>=300), Name(any)
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (10, EnumBorrow::Name("cc"), false),
        (11, EnumBorrow::Name("aa"), true),
        (12, EnumBorrow::Code(305), true),
        (13, EnumBorrow::Name("dd"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);
    let pat = [
        Pred::NameEq("aa".to_string()),
        Pred::CodeGe(300),
        Pred::NameAny,
    ];
    assert!(
        has_pattern_window_pred(&seq, &pat),
        "sequence {seq:?} missing pred pattern"
    );
}

#[test]
fn dict_mixed_nested_windows_pattern_dsl() {
    use ncb::EnumBorrow;
    // Mixed windows with dict-coded names and changing codes.
    // Sequence contains: [Name("aa"), Code(50), Code(51), Name("bb")] and
    // a later window matching [Name /^a.*/, Code in 53..=54].
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(51), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Name("alpha"), true),
        (6, EnumBorrow::Code(53), true),
        (7, EnumBorrow::Name("zz"), false),
        (8, EnumBorrow::Code(54), true),
        (9, EnumBorrow::Name("abacus"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    // Check for the complex nested window using the DSL
    let pat1 = vec![p_name("aa"), p_code(50), p_code(51), p_name("bb")];
    assert!(
        has_pattern_window_pred(&seq, &pat1),
        "missing pat1 in {seq:?}"
    );

    // Regex + range-based match
    let pat2 = vec![p_name_re(r"^a.*"), p_code_in(53, 54)];
    assert!(
        has_pattern_window_pred(&seq, &pat2),
        "missing pat2 in {seq:?}"
    );
}

#[test]
fn dict_mixed_nested_windows_fixture() {
    use ncb::EnumBorrow;
    // Same sequence as above; keep fixture stable for any codec/layout drift checks.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(51), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Name("alpha"), true),
        (6, EnumBorrow::Code(53), true),
        (7, EnumBorrow::Name("zz"), false),
        (8, EnumBorrow::Code(54), true),
        (9, EnumBorrow::Name("abacus"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    ensure_hex_fixture("tests/data/enum_dict_mixed_nested_windows.hex", &bytes);
}

#[test]
fn dict_mixed_overlapping_windows_pattern_dsl_and_fixture() {
    use ncb::EnumBorrow;
    // Overlapping windows with dict-coded names and code deltas.
    // Windows present:
    //  - [Name("ko"), Code(60), Name("ko")]
    //  - [Code in 60..=61, Name /^koto/]
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("ko"), true),
        (2, EnumBorrow::Code(60), true),
        (3, EnumBorrow::Name("ko"), true),
        (4, EnumBorrow::Code(60), true),
        (5, EnumBorrow::Code(61), true),
        (6, EnumBorrow::Name("kotodama"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    let pat1 = vec![p_name("ko"), p_code(60), p_name("ko")];
    assert!(
        has_pattern_window_pred(&seq, &pat1),
        "missing pat1 in {seq:?}"
    );

    let pat2 = vec![p_code_in(60, 61), p_name_re(r"^koto")];
    assert!(
        has_pattern_window_pred(&seq, &pat2),
        "missing pat2 in {seq:?}"
    );

    ensure_hex_fixture("tests/data/enum_dict_mixed_overlapping_windows.hex", &bytes);
}

#[test]
fn offsets_mixed_nested_windows_pattern_dsl() {
    use ncb::EnumBorrow;
    // Offsets-based names (no dict) variant of mixed nested windows.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(51), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Name("alpha"), true),
        (6, EnumBorrow::Code(53), true),
        (7, EnumBorrow::Name("zz"), false),
        (8, EnumBorrow::Code(54), true),
        (9, EnumBorrow::Name("abacus"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    let pat1 = vec![p_name("aa"), p_code(50), p_code(51), p_name("bb")];
    assert!(
        has_pattern_window_pred(&seq, &pat1),
        "missing pat1 in {seq:?}"
    );

    let pat2 = vec![p_name_re(r"^a.*"), p_code_in(53, 54)];
    assert!(
        has_pattern_window_pred(&seq, &pat2),
        "missing pat2 in {seq:?}"
    );
}

#[test]
fn offsets_mixed_nested_windows_fixture() {
    use ncb::EnumBorrow;
    // Fixture for offsets-based mixed nested windows.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(50), true),
        (3, EnumBorrow::Code(51), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Name("alpha"), true),
        (6, EnumBorrow::Code(53), true),
        (7, EnumBorrow::Name("zz"), false),
        (8, EnumBorrow::Code(54), true),
        (9, EnumBorrow::Name("abacus"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    ensure_hex_fixture("tests/data/enum_offsets_mixed_nested_windows.hex", &bytes);
}

#[test]
fn offsets_mixed_overlapping_windows_pattern_dsl_and_fixture() {
    use ncb::EnumBorrow;
    // Offsets-based names (no dict) overlapping windows variant.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("ko"), true),
        (2, EnumBorrow::Code(60), true),
        (3, EnumBorrow::Name("ko"), true),
        (4, EnumBorrow::Code(60), true),
        (5, EnumBorrow::Code(61), true),
        (6, EnumBorrow::Name("kotodama"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    let pat1 = vec![p_name("ko"), p_code(60), p_name("ko")];
    assert!(
        has_pattern_window_pred(&seq, &pat1),
        "missing pat1 in {seq:?}"
    );

    let pat2 = vec![p_code_in(60, 61), p_name_re(r"^koto")];
    assert!(
        has_pattern_window_pred(&seq, &pat2),
        "missing pat2 in {seq:?}"
    );

    ensure_hex_fixture(
        "tests/data/enum_offsets_mixed_overlapping_windows.hex",
        &bytes,
    );
}

#[test]
fn dict_long_nested_windows_a_pattern_and_fixture() {
    use ncb::EnumBorrow;
    // Longer nested windows with dict-coded names and varied code deltas (non-decreasing).
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(60), true),
        (3, EnumBorrow::Code(60), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Code(61), false),
        (6, EnumBorrow::Name("cc"), true),
        (7, EnumBorrow::Code(61), true),
        (8, EnumBorrow::Name("dd"), true),
        (9, EnumBorrow::Code(63), true),
        (10, EnumBorrow::Name("aa"), true),
        (11, EnumBorrow::Code(62), true),
        (12, EnumBorrow::Code(62), true),
        (13, EnumBorrow::Name("cc"), true),
        (14, EnumBorrow::Code(63), true),
        (15, EnumBorrow::Code(63), true),
        (16, EnumBorrow::Name("ee"), true),
        (17, EnumBorrow::Code(64), false),
        (18, EnumBorrow::Name("bb"), true),
        (19, EnumBorrow::Code(65), true),
        (20, EnumBorrow::Name("aa"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    // Windows to assert
    let p1 = vec![p_name("aa"), p_code(60), p_code(60), p_name("bb")];
    assert!(has_pattern_window_pred(&seq, &p1));

    let p2 = vec![p_name_re(r"^c"), p_code_in(61, 62)];
    assert!(has_pattern_window_pred(&seq, &p2));

    let p3 = vec![
        p_name("aa"),
        p_code_in(62, 63),
        p_code_in(62, 63),
        p_name_re(r"^c"),
    ];
    assert!(has_pattern_window_pred(&seq, &p3));

    let p4 = vec![p_code_in(63, 65), p_name_re(r"^e")];
    assert!(has_pattern_window_pred(&seq, &p4));

    ensure_hex_fixture("tests/data/enum_dict_long_nested_windows_a.hex", &bytes);
}

#[test]
fn dict_long_nested_windows_b_overlaps_and_fixture() {
    use ncb::EnumBorrow;
    // Even longer pattern-rich sequence with overlapping windows.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("ko"), true),
        (2, EnumBorrow::Code(70), true),
        (3, EnumBorrow::Code(70), true),
        (4, EnumBorrow::Name("ko"), true),
        (5, EnumBorrow::Name("koto"), true),
        (6, EnumBorrow::Code(71), true),
        (7, EnumBorrow::Name("koto2"), true),
        (8, EnumBorrow::Code(71), true),
        (9, EnumBorrow::Code(72), true),
        (10, EnumBorrow::Name("kot"), true),
        (11, EnumBorrow::Code(72), true),
        (12, EnumBorrow::Code(73), true),
        (13, EnumBorrow::Name("koto"), true),
        (14, EnumBorrow::Code(73), true),
        (15, EnumBorrow::Name("koto3"), false),
        (16, EnumBorrow::Name("kokoro"), true),
        (17, EnumBorrow::Code(74), true),
        (18, EnumBorrow::Name("koto"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    // Overlapping windows
    let w1 = vec![p_name("ko"), p_code(70), p_code(70), p_name("ko")];
    assert!(has_pattern_window_pred(&seq, &w1));

    let w2 = vec![p_name_re(r"^koto"), p_code_in(71, 73), p_name_re(r"^kot")];
    assert!(has_pattern_window_pred(&seq, &w2));

    let w3 = vec![p_code_in(72, 73), p_name_re(r"^koto")];
    assert!(has_pattern_window_pred(&seq, &w3));

    let w4 = vec![p_name_re(r"^koko?ro"), p_code_ge(74)];
    assert!(has_pattern_window_pred(&seq, &w4));

    ensure_hex_fixture("tests/data/enum_dict_long_nested_windows_b.hex", &bytes);
}

#[test]
fn offsets_long_nested_windows_a_pattern_and_fixture() {
    use ncb::EnumBorrow;
    // Offsets-based names (no dict) variant of the long nested windows sequence.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(60), true),
        (3, EnumBorrow::Code(60), true),
        (4, EnumBorrow::Name("bb"), true),
        (5, EnumBorrow::Code(61), false),
        (6, EnumBorrow::Name("cc"), true),
        (7, EnumBorrow::Code(61), true),
        (8, EnumBorrow::Name("dd"), true),
        (9, EnumBorrow::Code(63), true),
        (10, EnumBorrow::Name("aa"), true),
        (11, EnumBorrow::Code(62), true),
        (12, EnumBorrow::Code(62), true),
        (13, EnumBorrow::Name("cc"), true),
        (14, EnumBorrow::Code(63), true),
        (15, EnumBorrow::Code(63), true),
        (16, EnumBorrow::Name("ee"), true),
        (17, EnumBorrow::Code(64), false),
        (18, EnumBorrow::Name("bb"), true),
        (19, EnumBorrow::Code(65), true),
        (20, EnumBorrow::Name("aa"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    let p1 = vec![p_name("aa"), p_code(60), p_code(60), p_name("bb")];
    assert!(has_pattern_window_pred(&seq, &p1));

    let p2 = vec![p_name_re(r"^c"), p_code_in(61, 62)];
    assert!(has_pattern_window_pred(&seq, &p2));

    let p3 = vec![
        p_name("aa"),
        p_code_in(62, 63),
        p_code_in(62, 63),
        p_name_re(r"^c"),
    ];
    assert!(has_pattern_window_pred(&seq, &p3));

    let p4 = vec![p_code_in(63, 65), p_name_re(r"^e")];
    assert!(has_pattern_window_pred(&seq, &p4));

    ensure_hex_fixture("tests/data/enum_offsets_long_nested_windows_a.hex", &bytes);
}

#[test]
fn offsets_long_nested_windows_b_overlaps_and_fixture() {
    use ncb::EnumBorrow;
    // Offsets-based names (no dict) variant of the overlapping windows sequence.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("ko"), true),
        (2, EnumBorrow::Code(70), true),
        (3, EnumBorrow::Code(70), true),
        (4, EnumBorrow::Name("ko"), true),
        (5, EnumBorrow::Name("koto"), true),
        (6, EnumBorrow::Code(71), true),
        (7, EnumBorrow::Name("koto2"), true),
        (8, EnumBorrow::Code(71), true),
        (9, EnumBorrow::Code(72), true),
        (10, EnumBorrow::Name("kot"), true),
        (11, EnumBorrow::Code(72), true),
        (12, EnumBorrow::Code(73), true),
        (13, EnumBorrow::Name("koto"), true),
        (14, EnumBorrow::Code(73), true),
        (15, EnumBorrow::Name("koto3"), false),
        (16, EnumBorrow::Name("kokoro"), true),
        (17, EnumBorrow::Code(74), true),
        (18, EnumBorrow::Name("koto"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    let seq = collect_flag_true_payloads(&view);

    let w1 = vec![p_name("ko"), p_code(70), p_code(70), p_name("ko")];
    assert!(has_pattern_window_pred(&seq, &w1));

    let w2 = vec![p_name_re(r"^koto"), p_code_in(71, 73), p_name_re(r"^kot")];
    assert!(has_pattern_window_pred(&seq, &w2));

    let w3 = vec![p_code_in(72, 73), p_name_re(r"^koto")];
    assert!(has_pattern_window_pred(&seq, &w3));

    let w4 = vec![p_name_re(r"^koko?ro"), p_code_ge(74)];
    assert!(has_pattern_window_pred(&seq, &w4));

    ensure_hex_fixture("tests/data/enum_offsets_long_nested_windows_b.hex", &bytes);
}

#[test]
fn offsets_id_code_delta_alt_names_and_codes() {
    use ncb::EnumBorrow;
    // Alternating code deltas with repeated names; id-delta ON, code-delta ON, no dict
    // Rows layout (id increments small; codes repeat then bump):
    // 0: id=100, Name("aa"), flag=T
    // 1: id+1,  Code(100),    flag=F
    // 2: id+1,  Code(100),    flag=T
    // 3: id+2,  Name("aa"),   flag=T
    // 4: id+1,  Code(101),    flag=F
    // 5: id+1,  Code(101),    flag=T
    // 6: id+2,  Name("bb"),   flag=T
    // 7: id+1,  Code(102),    flag=F
    // 8: id+1,  Code(102),    flag=T
    // 9: id+2,  Name("bb"),   flag=T
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (100, EnumBorrow::Name("aa"), true),
        (101, EnumBorrow::Code(100), false),
        (102, EnumBorrow::Code(100), true),
        (104, EnumBorrow::Name("aa"), true),
        (105, EnumBorrow::Code(101), false),
        (106, EnumBorrow::Code(101), true),
        (108, EnumBorrow::Name("bb"), true),
        (109, EnumBorrow::Code(102), false),
        (110, EnumBorrow::Code(102), true),
        (112, EnumBorrow::Name("bb"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Validate specific subsequences via iterators
    let names_true: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names_true, vec!["aa", "aa", "bb", "bb"]);
    let codes_true: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(codes_true, vec![100, 101, 102]);
    // Check subsequence: skip first name, next two are ["aa", "bb"]
    let names_slice: Vec<String> = view
        .iter_names_flag_true_fast()
        .skip(1)
        .take(2)
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names_slice, vec!["aa", "bb"]);
    // Codes subsequence: skip one, expect [101, 102]
    let codes_slice: Vec<u32> = view.iter_codes_flag_true_fast().skip(1).take(2).collect();
    assert_eq!(codes_slice, vec![101, 102]);
}

#[test]
fn dict_id_code_delta_alt_names_and_codes() {
    use ncb::EnumBorrow;
    // Same alternating pattern, but with dict ON and id+code deltas ON.
    let rows: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("aa"), true),
        (2, EnumBorrow::Code(10), false),
        (3, EnumBorrow::Code(10), true),
        (5, EnumBorrow::Name("aa"), true),
        (6, EnumBorrow::Code(11), false),
        (7, EnumBorrow::Code(11), true),
        (9, EnumBorrow::Name("bb"), true),
        (10, EnumBorrow::Code(12), false),
        (11, EnumBorrow::Code(12), true),
        (13, EnumBorrow::Name("bb"), true),
    ];
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), rows.len());

    // Names via dict should resolve correctly
    let names_true: Vec<String> = view
        .iter_names_flag_true_fast()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names_true, vec!["aa", "aa", "bb", "bb"]);
    let codes_true: Vec<u32> = view.iter_codes_flag_true_fast().collect();
    assert_eq!(codes_true, vec![10, 11, 12]);
    // Specific slices
    let names_slice: Vec<String> = view
        .iter_names_flag_true_fast()
        .skip(2)
        .take(2)
        .map(|s| s.to_string())
        .collect();
    assert_eq!(names_slice, vec!["bb", "bb"]);
    let codes_slice: Vec<u32> = view.iter_codes_flag_true_fast().take(2).collect();
    assert_eq!(codes_slice, vec![10, 11]);
}
