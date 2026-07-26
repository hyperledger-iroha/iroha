//! Deterministic tests for nested enum NCB pattern windows.

use norito::columnar as ncb;

#[derive(Debug, Clone, PartialEq, Eq)]
enum PayloadKind {
    Name(String),
    Code(u32),
}

#[derive(Debug, Clone)]
enum Pred {
    NameEq(String),
    CodeEq(u32),
    NameMatches(regex::Regex),
    CodeIn(core::ops::RangeInclusive<u32>),
    CodesNonDecreasing(usize),
}

fn p_name(value: &str) -> Pred {
    Pred::NameEq(value.to_owned())
}

fn p_name_re(value: &str) -> Pred {
    Pred::NameMatches(regex::Regex::new(value).unwrap())
}

fn p_code(value: u32) -> Pred {
    Pred::CodeEq(value)
}

fn p_code_in(start: u32, end: u32) -> Pred {
    Pred::CodeIn(start..=end)
}

fn p_codes_nd(len: usize) -> Pred {
    Pred::CodesNonDecreasing(len)
}

fn has_pattern_window_pred(seq: &[PayloadKind], pattern: &[Pred]) -> bool {
    if pattern.is_empty() || seq.len() < pattern.len() {
        return false;
    }
    'outer: for start in 0..=seq.len() - pattern.len() {
        for (idx, pred) in pattern.iter().enumerate() {
            let value = &seq[start + idx];
            let ok = match (value, pred) {
                (PayloadKind::Name(actual), Pred::NameEq(expected)) => actual == expected,
                (PayloadKind::Name(actual), Pred::NameMatches(regex)) => regex.is_match(actual),
                (PayloadKind::Code(actual), Pred::CodeEq(expected)) => actual == expected,
                (PayloadKind::Code(actual), Pred::CodeIn(range)) => range.contains(actual),
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

fn has_pattern_window_pred_adv(seq: &[PayloadKind], pattern: &[Pred]) -> bool {
    if pattern.is_empty() || seq.is_empty() {
        return false;
    }
    for start in 0..seq.len() {
        let mut seq_idx = start;
        let mut pred_idx = 0;
        while pred_idx < pattern.len() {
            match &pattern[pred_idx] {
                Pred::CodesNonDecreasing(len) => {
                    if *len == 0 || seq_idx + len > seq.len() {
                        break;
                    }
                    let mut last = None;
                    let mut ok = true;
                    for offset in 0..*len {
                        match seq[seq_idx + offset] {
                            PayloadKind::Code(value) => {
                                if let Some(prev) = last
                                    && value < prev
                                {
                                    ok = false;
                                    break;
                                }
                                last = Some(value);
                            }
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if !ok {
                        break;
                    }
                    seq_idx += len;
                    pred_idx += 1;
                }
                pred => {
                    if seq_idx >= seq.len()
                        || !has_pattern_window_pred(
                            &seq[seq_idx..=seq_idx],
                            std::slice::from_ref(pred),
                        )
                    {
                        break;
                    }
                    seq_idx += 1;
                    pred_idx += 1;
                }
            }
        }
        if pred_idx == pattern.len() {
            return true;
        }
    }
    false
}

fn collect_flag_true_payloads(view: &ncb::NcbU64EnumBoolView<'_>) -> Vec<PayloadKind> {
    let mut out = Vec::new();
    for idx in 0..view.len() {
        if view.flag(idx) {
            match view.payload(idx).expect("payload") {
                ncb::ColEnumRef::Name(value) => out.push(PayloadKind::Name(value.to_owned())),
                ncb::ColEnumRef::Code(value) => out.push(PayloadKind::Code(value)),
            }
        }
    }
    out
}

fn has_non_decreasing_code_window(seq: &[PayloadKind], len: usize) -> bool {
    if len == 0 || seq.len() < len {
        return false;
    }
    for start in 0..=seq.len() - len {
        let mut last = None;
        let mut ok = true;
        for offset in 0..len {
            match seq[start + offset] {
                PayloadKind::Code(value) => {
                    if let Some(prev) = last
                        && value < prev
                    {
                        ok = false;
                        break;
                    }
                    last = Some(value);
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

fn has_code_exact_window(seq: &[PayloadKind], codes: &[u32]) -> bool {
    if codes.is_empty() || seq.len() < codes.len() {
        return false;
    }
    'outer: for start in 0..=seq.len() - codes.len() {
        for (idx, expected) in codes.iter().enumerate() {
            match seq[start + idx] {
                PayloadKind::Code(actual) if actual == *expected => {}
                _ => continue 'outer,
            }
        }
        return true;
    }
    false
}

const NAMES_POOL: &[&str] = &[
    "aa", "bb", "cc", "dd", "ee", "ko", "kot", "koto", "koto2", "alpha", "abacus", "zz",
];

fn add_filler(rows: &mut Vec<(u64, ncb::EnumBorrow<'static>, bool)>, id: &mut u64, len: usize) {
    for idx in 0..len {
        if idx % 2 == 0 {
            let name = NAMES_POOL[idx % NAMES_POOL.len()];
            rows.push((*id, ncb::EnumBorrow::Name(name), (idx % 3) != 1));
        } else {
            let code = 45 + ((idx as u32) % 40);
            rows.push((*id, ncb::EnumBorrow::Code(code), (idx % 4) != 2));
        }
        *id += 1;
    }
}

fn build_rows_with_embedded_patterns(
    fill1: usize,
    fill2: usize,
    fill3: usize,
    fill4: usize,
) -> Vec<(u64, ncb::EnumBorrow<'static>, bool)> {
    let mut rows = Vec::new();
    let mut id = 1u64;

    add_filler(&mut rows, &mut id, fill1);
    rows.push((id, ncb::EnumBorrow::Name("aa"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(50), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(50), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("bb"), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill2);
    rows.push((id, ncb::EnumBorrow::Name("alpha"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(53), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill3);
    rows.push((id, ncb::EnumBorrow::Code(61), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("kotodama"), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill4);
    rows.push((id, ncb::EnumBorrow::Name("aa"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(60), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(61), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(61), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("cc"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(62), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("dd"), true));

    rows
}

fn assert_contains_multi_windows(dict: bool) {
    for (fill1, fill2, fill3, fill4) in [(0, 0, 0, 0), (1, 2, 3, 4), (4, 3, 2, 1)] {
        let rows = build_rows_with_embedded_patterns(fill1, fill2, fill3, fill4);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, dict, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        let w1 = vec![p_name("aa"), p_code(50), p_code(50), p_name("bb")];
        assert!(has_pattern_window_pred(&seq, &w1));

        let w2 = vec![p_name_re(r"^a.*"), p_code_in(53, 54)];
        assert!(has_pattern_window_pred(&seq, &w2));

        let w3 = vec![p_code_in(60, 61), p_name_re(r"^kot")];
        assert!(has_pattern_window_pred(&seq, &w3));

        let w4_deep = vec![
            p_name("aa"),
            p_code(60),
            p_code(61),
            p_code(61),
            p_name("cc"),
            p_code(62),
            p_name("dd"),
        ];
        assert!(has_pattern_window_pred(&seq, &w4_deep));

        let pat_nd = vec![p_name("aa"), p_codes_nd(3), p_name("cc")];
        assert!(has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

#[test]
fn offsets_contains_multi_windows() {
    assert_contains_multi_windows(false);
}

#[test]
fn dict_contains_multi_windows() {
    assert_contains_multi_windows(true);
}

fn decreasing_rows(prefix: usize, suffix: usize) -> Vec<(u64, ncb::EnumBorrow<'static>, bool)> {
    let mut rows = Vec::new();
    let mut id = 1u64;
    add_filler(&mut rows, &mut id, prefix);
    rows.push((id, ncb::EnumBorrow::Name("alpha"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(60), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(62), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(61), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("beta"), true));
    id += 1;
    add_filler(&mut rows, &mut id, suffix);
    rows
}

fn assert_negative_decreasing_codes_window(dict: bool) {
    for (prefix, suffix) in [(0, 0), (1, 2), (3, 1)] {
        let rows = decreasing_rows(prefix, suffix);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, dict, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        let exact = [60, 62, 61];
        assert!(has_code_exact_window(&seq, &exact));
        assert!(!has_non_decreasing_code_window(&seq, exact.len()));

        let pat_nd = vec![p_name("alpha"), p_codes_nd(3), p_name("beta")];
        assert!(!has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

#[test]
fn offsets_negative_decreasing_codes_window() {
    assert_negative_decreasing_codes_window(false);
}

#[test]
fn dict_negative_decreasing_codes_window() {
    assert_negative_decreasing_codes_window(true);
}

fn non_decreasing_rows(
    start: u32,
    steps: &[u8],
    prefix: usize,
    suffix: usize,
) -> (Vec<(u64, ncb::EnumBorrow<'static>, bool)>, Vec<u32>) {
    let mut rows = Vec::new();
    let mut id = 1u64;
    add_filler(&mut rows, &mut id, prefix);

    let mut codes = Vec::with_capacity(steps.len() + 1);
    let mut cur = start;
    codes.push(cur);
    for step in steps {
        cur = cur.saturating_add(*step as u32);
        codes.push(cur);
    }
    for &code in &codes {
        rows.push((id, ncb::EnumBorrow::Code(code), true));
        id += 1;
    }

    add_filler(&mut rows, &mut id, suffix);
    (rows, codes)
}

fn assert_non_decreasing_codes_window(dict: bool) {
    for (start, steps, prefix, suffix) in [
        (40_u32, &[0_u8, 1, 1][..], 0_usize, 0_usize),
        (60, &[3, 0, 2, 1][..], 1, 2),
        (80, &[0, 0, 0][..], 4, 1),
    ] {
        let (rows, codes) = non_decreasing_rows(start, steps, prefix, suffix);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, dict, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        assert!(has_non_decreasing_code_window(&seq, codes.len()));
        assert!(has_code_exact_window(&seq, &codes));
    }
}

#[test]
fn offsets_non_decreasing_codes_window() {
    assert_non_decreasing_codes_window(false);
}

#[test]
fn dict_non_decreasing_codes_window() {
    assert_non_decreasing_codes_window(true);
}
