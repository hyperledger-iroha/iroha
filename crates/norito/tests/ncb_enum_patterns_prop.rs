//! Property tests generating sequences that guarantee multiple nested windows.
//! Uses a tiny predicate DSL to assert presence of complex windows.

use norito::columnar as ncb;
use proptest::{collection::vec as pvec, prelude::*};

// Local copy of the payload view used by the matcher
#[derive(Debug, Clone, PartialEq, Eq)]
enum PayloadKind {
    Name(String),
    Code(u32),
}

// Predicates and small DSL (mirrors helpers in ncb_enum_iter_samples.rs)
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Pred {
    NameEq(String),
    NameAny,
    CodeEq(u32),
    CodeGe(u32),
    CodeAny,
    NameMatches(regex::Regex),
    CodeIn(core::ops::RangeInclusive<u32>),
    CodesNonDecreasing(usize),
}

// Builder helpers (function-based DSL to avoid macro parsing conflicts in proptest contexts)
fn p_name(s: &str) -> Pred {
    Pred::NameEq(s.to_string())
}
#[allow(dead_code)]
fn p_name_any() -> Pred {
    Pred::NameAny
}
fn p_name_re(re: &str) -> Pred {
    Pred::NameMatches(regex::Regex::new(re).unwrap())
}
fn p_code(v: u32) -> Pred {
    Pred::CodeEq(v)
}
#[allow(dead_code)]
fn p_code_ge(v: u32) -> Pred {
    Pred::CodeGe(v)
}
#[allow(dead_code)]
fn p_code_any() -> Pred {
    Pred::CodeAny
}
fn p_code_in(start: u32, end: u32) -> Pred {
    Pred::CodeIn(start..=end)
}
fn p_codes_nd(n: usize) -> Pred {
    Pred::CodesNonDecreasing(n)
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
                (PayloadKind::Code(_), Pred::CodeAny) => true,
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

// Advanced matcher: supports variable-width predicates like `CodesNonDecreasing(n)`
fn has_pattern_window_pred_adv(seq: &[PayloadKind], pattern: &[Pred]) -> bool {
    if pattern.is_empty() || seq.is_empty() {
        return false;
    }
    for start in 0..seq.len() {
        let mut i = start; // index in seq
        let mut j = 0; // index in pattern
        while j < pattern.len() {
            if i > seq.len() {
                break;
            }
            match pattern[j] {
                Pred::CodesNonDecreasing(n) => {
                    if n == 0 || i + n > seq.len() {
                        break;
                    }
                    let mut ok = true;
                    let mut last: Option<u32> = None;
                    for k in 0..n {
                        match seq[i + k] {
                            PayloadKind::Code(v) => {
                                if let Some(prev) = last
                                    && v < prev
                                {
                                    ok = false;
                                    break;
                                }
                                last = Some(v);
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
                    i += n;
                    j += 1;
                }
                Pred::NameEq(ref s) => match seq.get(i) {
                    Some(PayloadKind::Name(a)) if a == s => {
                        i += 1;
                        j += 1;
                    }
                    _ => break,
                },
                Pred::NameAny => {
                    if i < seq.len() {
                        if matches!(seq[i], PayloadKind::Name(_)) {
                            i += 1;
                            j += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Pred::NameMatches(ref re) => match seq.get(i) {
                    Some(PayloadKind::Name(a)) if re.is_match(a) => {
                        i += 1;
                        j += 1;
                    }
                    _ => break,
                },
                Pred::CodeEq(v) => match seq.get(i) {
                    Some(PayloadKind::Code(a)) if *a == v => {
                        i += 1;
                        j += 1;
                    }
                    _ => break,
                },
                Pred::CodeGe(v) => match seq.get(i) {
                    Some(PayloadKind::Code(a)) if *a >= v => {
                        i += 1;
                        j += 1;
                    }
                    _ => break,
                },
                Pred::CodeAny => {
                    if i < seq.len() {
                        if matches!(seq[i], PayloadKind::Code(_)) {
                            i += 1;
                            j += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Pred::CodeIn(ref r) => match seq.get(i) {
                    Some(PayloadKind::Code(a)) if r.contains(a) => {
                        i += 1;
                        j += 1;
                    }
                    _ => break,
                },
            }
        }
        if j == pattern.len() {
            return true;
        }
    }
    false
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

fn has_non_decreasing_code_window(seq: &[PayloadKind], len: usize) -> bool {
    if len == 0 || seq.len() < len {
        return false;
    }
    for start in 0..=seq.len() - len {
        let mut ok = true;
        let mut last: Option<u32> = None;
        for j in 0..len {
            match seq[start + j] {
                PayloadKind::Code(v) => {
                    if let Some(prev) = last
                        && v < prev
                    {
                        ok = false;
                        break;
                    }
                    last = Some(v);
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
        for (j, &cv) in codes.iter().enumerate() {
            match seq[start + j] {
                PayloadKind::Code(v) if v == cv => {}
                _ => continue 'outer,
            }
        }
        return true;
    }
    false
}

// Static pools for filler generation
const NAMES_POOL: &[&str] = &[
    "aa", "bb", "cc", "dd", "ee", "ko", "kot", "koto", "koto2", "alpha", "abacus", "zz",
];

fn build_rows_with_embedded_patterns(
    fill1: usize,
    fill2: usize,
    fill3: usize,
    fill4: usize,
) -> Vec<(u64, ncb::EnumBorrow<'static>, bool)> {
    let mut rows: Vec<(u64, ncb::EnumBorrow<'static>, bool)> = Vec::new();
    let mut id = 1u64;
    // Filler generator: alternate name/code, vary flags
    fn add_filler(rows: &mut Vec<(u64, ncb::EnumBorrow<'static>, bool)>, id: &mut u64, len: usize) {
        for k in 0..len {
            if k % 2 == 0 {
                let name = NAMES_POOL[k % NAMES_POOL.len()];
                rows.push((*id, ncb::EnumBorrow::Name(name), (k % 3) != 1));
            } else {
                let code = 45 + ((k as u32) % 40); // 45..84
                rows.push((*id, ncb::EnumBorrow::Code(code), (k % 4) != 2));
            }
            *id += 1;
        }
    }

    add_filler(&mut rows, &mut id, fill1);
    // Window W1: [Name("aa"), Code(50), Code(50), Name("bb")]
    rows.push((id, ncb::EnumBorrow::Name("aa"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(50), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(50), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("bb"), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill2);
    // Window W2: [Name /^a.*/, Code in 53..=54]
    rows.push((id, ncb::EnumBorrow::Name("alpha"), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Code(53), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill3);
    // Window W3: [Code in 60..=61, Name /^kot/]
    rows.push((id, ncb::EnumBorrow::Code(61), true));
    id += 1;
    rows.push((id, ncb::EnumBorrow::Name("kotodama"), true));
    id += 1;

    add_filler(&mut rows, &mut id, fill4);
    // Deep Window W4: [Name("aa"), Code(60), Code(61), Code(61), Name("cc"), Code(62), Name("dd")]
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn offsets_prop_contains_multi_windows(
        fill1 in 0usize..5,
        fill2 in 0usize..5,
        fill3 in 0usize..5,
        fill4 in 0usize..5,
    ) {
        let rows = build_rows_with_embedded_patterns(fill1, fill2, fill3, fill4);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        let w1 = vec![ p_name("aa"), p_code(50), p_code(50), p_name("bb") ];
        prop_assert!(has_pattern_window_pred(&seq, &w1));

        let w2 = vec![ p_name_re(r"^a.*"), p_code_in(53, 54) ];
        prop_assert!(has_pattern_window_pred(&seq, &w2));

        let w3 = vec![ p_code_in(60, 61), p_name_re(r"^kot") ];
        prop_assert!(has_pattern_window_pred(&seq, &w3));

        let w4_deep = vec![ p_name("aa"), p_code(60), p_code(61), p_code(61), p_name("cc"), p_code(62), p_name("dd") ];
        prop_assert!(has_pattern_window_pred(&seq, &w4_deep));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dict_prop_contains_multi_windows(
        fill1 in 0usize..6,
        fill2 in 0usize..6,
        fill3 in 0usize..6,
        fill4 in 0usize..6,
    ) {
        let rows = build_rows_with_embedded_patterns(fill1, fill2, fill3, fill4);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        let w1 = vec![ p_name("aa"), p_code(50), p_code(50), p_name("bb") ];
        prop_assert!(has_pattern_window_pred(&seq, &w1));

        let w2 = vec![ p_name_re(r"^a.*"), p_code_in(53, 54) ];
        prop_assert!(has_pattern_window_pred(&seq, &w2));

        let w3 = vec![ p_code_in(60, 61), p_name_re(r"^kot") ];
        prop_assert!(has_pattern_window_pred(&seq, &w3));

        let w4_deep = vec![ p_name("aa"), p_code(60), p_code(61), p_code(61), p_name("cc"), p_code(62), p_name("dd") ];
        prop_assert!(has_pattern_window_pred(&seq, &w4_deep));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn offsets_prop_nd_dsl_window(
        f1 in 0usize..5, f2 in 0usize..5, f3 in 0usize..5, f4 in 0usize..5,
    ) {
        let rows = build_rows_with_embedded_patterns(f1, f2, f3, f4);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);
        // W4 deep contains three non-decreasing codes (60,61,61) between names aa and cc
        let pat_nd = vec![ p_name("aa"), p_codes_nd(3), p_name("cc") ];
        prop_assert!(has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dict_prop_nd_dsl_window(
        f1 in 0usize..6, f2 in 0usize..6, f3 in 0usize..6, f4 in 0usize..6,
    ) {
        let rows = build_rows_with_embedded_patterns(f1, f2, f3, f4);
        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);
        let pat_nd = vec![ p_name("aa"), p_codes_nd(3), p_name("cc") ];
        prop_assert!(has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn offsets_prop_negative_decreasing_codes_window(
        prefix in 0usize..=3,
        suffix in 0usize..=3,
    ) {
        let mut rows: Vec<(u64, ncb::EnumBorrow<'static>, bool)> = Vec::new();
        let mut id = 1u64;
        for k in 0..prefix {
            if k % 2 == 0 { rows.push((id, ncb::EnumBorrow::Name("zz"), true)); }
            else { rows.push((id, ncb::EnumBorrow::Code(30 + k as u32), true)); }
            id += 1;
        }
        // Intentionally decreasing codes: 60, 62, 61
        rows.push((id, ncb::EnumBorrow::Name("alpha"), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(60), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(62), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(61), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Name("beta"), true)); id += 1;
        for k in 0..suffix {
            if k % 2 == 0 { rows.push((id, ncb::EnumBorrow::Name("yy"), true)); }
            else { rows.push((id, ncb::EnumBorrow::Code(80 + k as u32), true)); }
            id += 1;
        }

        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        // The exact codes window exists but is not non-decreasing
        let exact = [60, 62, 61];
        prop_assert!(has_code_exact_window(&seq, &exact));
        prop_assert!(!has_non_decreasing_code_window(&seq, exact.len()));

        // Advanced DSL should not match a non-decreasing codes window between names
        let pat_nd = vec![ p_name("alpha"), p_codes_nd(3), p_name("beta") ];
        prop_assert!(!has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dict_prop_negative_decreasing_codes_window(
        prefix in 0usize..=3,
        suffix in 0usize..=3,
    ) {
        let mut rows: Vec<(u64, ncb::EnumBorrow<'static>, bool)> = Vec::new();
        let mut id = 1u64;
        for k in 0..prefix {
            if k % 2 == 0 { rows.push((id, ncb::EnumBorrow::Name("zz"), true)); }
            else { rows.push((id, ncb::EnumBorrow::Code(30 + k as u32), true)); }
            id += 1;
        }
        rows.push((id, ncb::EnumBorrow::Name("alpha"), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(60), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(62), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Code(61), true)); id += 1;
        rows.push((id, ncb::EnumBorrow::Name("beta"), true)); id += 1;
        for k in 0..suffix {
            if k % 2 == 0 { rows.push((id, ncb::EnumBorrow::Name("yy"), true)); }
            else { rows.push((id, ncb::EnumBorrow::Code(80 + k as u32), true)); }
            id += 1;
        }

        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        let exact = [60, 62, 61];
        prop_assert!(has_code_exact_window(&seq, &exact));
        prop_assert!(!has_non_decreasing_code_window(&seq, exact.len()));

        let pat_nd = vec![ p_name("alpha"), p_codes_nd(3), p_name("beta") ];
        prop_assert!(!has_pattern_window_pred_adv(&seq, &pat_nd));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn offsets_prop_random_non_decreasing_codes_window(
        start in 40u32..=80,
        steps in pvec(0u8..=3, 3..=8),
        prefix in 0usize..=4,
        suffix in 0usize..=4,
    ) {
        let mut rows: Vec<(u64, ncb::EnumBorrow<'static>, bool)> = Vec::new();
        let mut id = 1u64;

        // prefix filler (names/codes mixed; arbitrary flags)
        for k in 0..prefix {
            if k % 2 == 0 {
                let name = NAMES_POOL[k % NAMES_POOL.len()];
                rows.push((id, ncb::EnumBorrow::Name(name), (k % 3) != 1));
            } else {
                let code = 30 + ((k as u32) % 20);
                rows.push((id, ncb::EnumBorrow::Code(code), (k % 4) != 2));
            }
            id += 1;
        }

        // Construct non-decreasing code block
        let mut codes: Vec<u32> = Vec::with_capacity(steps.len() + 1);
        let mut cur = start;
        codes.push(cur);
        for s in &steps { cur = cur.saturating_add(*s as u32); codes.push(cur); }
        for &v in &codes { rows.push((id, ncb::EnumBorrow::Code(v), true)); id += 1; }

        // suffix filler
        for k in 0..suffix {
            if k % 2 == 0 {
                let name = NAMES_POOL[(k + 7) % NAMES_POOL.len()];
                rows.push((id, ncb::EnumBorrow::Name(name), (k % 3) != 1));
            } else {
                let code = 80 + ((k as u32) % 20);
                rows.push((id, ncb::EnumBorrow::Code(code), (k % 4) != 2));
            }
            id += 1;
        }

        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        prop_assert!(has_non_decreasing_code_window(&seq, codes.len()));
        prop_assert!(has_code_exact_window(&seq, &codes));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn dict_prop_random_non_decreasing_codes_window(
        start in 40u32..=80,
        steps in pvec(0u8..=3, 3..=8),
        prefix in 0usize..=4,
        suffix in 0usize..=4,
    ) {
        let mut rows: Vec<(u64, ncb::EnumBorrow<'static>, bool)> = Vec::new();
        let mut id = 1u64;

        for k in 0..prefix {
            if k % 2 == 0 {
                let name = NAMES_POOL[k % NAMES_POOL.len()];
                rows.push((id, ncb::EnumBorrow::Name(name), (k % 3) != 1));
            } else {
                let code = 30 + ((k as u32) % 20);
                rows.push((id, ncb::EnumBorrow::Code(code), (k % 4) != 2));
            }
            id += 1;
        }

        let mut codes: Vec<u32> = Vec::with_capacity(steps.len() + 1);
        let mut cur = start;
        codes.push(cur);
        for s in &steps { cur = cur.saturating_add(*s as u32); codes.push(cur); }
        for &v in &codes { rows.push((id, ncb::EnumBorrow::Code(v), true)); id += 1; }

        for k in 0..suffix {
            if k % 2 == 0 {
                let name = NAMES_POOL[(k + 7) % NAMES_POOL.len()];
                rows.push((id, ncb::EnumBorrow::Name(name), (k % 3) != 1));
            } else {
                let code = 80 + ((k as u32) % 20);
                rows.push((id, ncb::EnumBorrow::Code(code), (k % 4) != 2));
            }
            id += 1;
        }

        let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, true, true);
        let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
        let seq = collect_flag_true_payloads(&view);

        prop_assert!(has_non_decreasing_code_window(&seq, codes.len()));
        prop_assert!(has_code_exact_window(&seq, &codes));
    }
}
