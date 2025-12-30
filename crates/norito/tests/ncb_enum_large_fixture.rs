//! NCB enum large dataset fixture and view checks.
#![cfg(feature = "json")]

use norito::columnar as ncb;

#[test]
fn ncb_enum_offsets_id_code_delta_large_fixture() {
    use ncb::EnumBorrow;
    // Recreate the dataset used by the fixture
    let n = 300usize;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::with_capacity(n);
    let mut id = 1u64;
    let mut code = 100u32;
    for i in 0..n {
        let flag = i % 5 == 0;
        if i % 3 == 0 {
            let name = match i % 4 {
                0 => "aa",
                1 => "bbb",
                2 => "cccc",
                _ => "d",
            };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else {
            rows.push((id, EnumBorrow::Code(code), flag));
            code = code.wrapping_add((i % 7 + 1) as u32);
        }
        id = id.wrapping_add((i % 9 + 1) as u64);
    }

    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    // Path checked/created inside ensure_hex_fixture
    // Generate-or-verify fixture behavior
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
                return;
            }
        }
        let hex = std::fs::read_to_string(&path).expect("read fixture");
        let hex = hex.trim();
        if generated != hex {
            let gl = generated.len();
            let hl = hex.len();
            let min = gl.min(hl);
            let mut idx = None;
            for i in 0..min {
                if generated.as_bytes()[i] != hex.as_bytes()[i] {
                    idx = Some(i);
                    break;
                }
            }
            eprintln!("fixture mismatch: gen_len={gl}, hex_len={hl}, first_diff={idx:?}");
            if let Some(i) = idx {
                let lo = i.saturating_sub(32);
                let hi = (i + 32).min(min);
                eprintln!("generated[..]: {}", &generated[lo..hi]);
                eprintln!("hex[..]: {}", &hex[lo..hi]);
            }
            assert_eq!(generated, hex, "fixture bytes mismatch");
        }
    }
    ensure_hex_fixture("tests/data/enum_offsets_id_code_delta_large.hex", &bytes);

    // View-level checks across chunk boundary (256) and ends
    let view = ncb::view_ncb_u64_enum_bool(&bytes).expect("view");
    assert_eq!(view.len(), n);
    for &(i, expect_flag) in &[
        (0usize, true),
        (1, false),
        (255, true),
        (256, false),
        (299, false),
    ] {
        assert_eq!(view.flag(i), expect_flag, "flag mismatch at {i}");
    }
    // Spot-check names and codes around chunk edges
    match view.payload(0).expect("payload 0") {
        ncb::ColEnumRef::Name(s) => assert_eq!(s, "aa"),
        _ => panic!("expected Name at 0"),
    }
    match view.payload(1).expect("payload 1") {
        ncb::ColEnumRef::Code(v) => assert!(v >= 100),
        _ => panic!("expected Code at 1"),
    }
    match view.payload(255).expect("payload 255") {
        ncb::ColEnumRef::Name(s) => assert!(!s.is_empty()),
        _ => panic!("expected Name at 255"),
    }
    match view.payload(256).expect("payload 256") {
        ncb::ColEnumRef::Code(v) => assert!(v > 0),
        _ => panic!("expected Code at 256"),
    }

    // Iterator parity: fast vs indexed and vs naive scanning counts
    let mut naive_true = 0usize;
    let mut naive_names_true = 0usize;
    let mut naive_codes_true = 0usize;
    for i in 0..view.len() {
        if view.flag(i) {
            naive_true += 1;
            match view.payload(i).expect("payload") {
                ncb::ColEnumRef::Name(_) => naive_names_true += 1,
                ncb::ColEnumRef::Code(_) => naive_codes_true += 1,
            }
        }
    }
    let fast_ids = view.iter_ids_flag_true_fast().count();
    assert_eq!(fast_ids, naive_true, "ids flag-true count");
    let fast_names = view.iter_names_flag_true_fast().count();
    assert_eq!(fast_names, naive_names_true, "names flag-true count");
    let fast_codes = view.iter_codes_flag_true_fast().count();
    assert_eq!(fast_codes, naive_codes_true, "codes flag-true count");
    // Indexed variants should match too
    let idx_names = view.iter_names_flag_true_indexed().count();
    let idx_codes = view.iter_codes_flag_true_indexed().count();
    assert_eq!(idx_names, naive_names_true, "idx names count");
    assert_eq!(idx_codes, naive_codes_true, "idx codes count");
}
