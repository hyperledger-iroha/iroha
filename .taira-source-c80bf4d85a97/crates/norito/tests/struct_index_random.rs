//! Randomized/adversarial corpus comparing NEON/fast path to scalar reference.
#![cfg(feature = "json")]

use norito::json::{build_struct_index, build_struct_index_scalar_test};

fn make_mix(seed: u64, kib: usize) -> String {
    // Deterministic generator: alternate patterns and inject spaces to shift alignment.
    let mut rng = seed;
    let pats = [
        r#"{"k":"a\"b"}"#,
        r#"{"k":"\u0041"}"#,
        r#"{"arr":[1,2,3]}"#,
        r#"{"o":{}}"#,
        r#"{"s":"\\\\"}"#,
    ];
    let mut s = String::with_capacity(kib * 1024 + 2);
    s.push('[');
    let mut first = true;
    let mut written = 1usize;
    while written + 64 + 2 < kib * 1024 {
        // leave headroom for commas and closing
        // pick a pattern
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let idx = (rng >> 33) as usize % pats.len();
        let pat = pats[idx];
        // pad left with some spaces to vary 16B alignment
        let pad = (rng >> 5) as usize % 16;
        if !first {
            s.push(',');
            written += 1;
        }
        first = false;
        for _ in 0..pad {
            s.push(' ');
        }
        s.push_str(pat);
        written += pad + pat.len();
    }
    s.push(']');
    s
}

#[test]
fn random_adversarial_parity() {
    for kib in [32usize, 128, 512] {
        for seed in [1u64, 0xDEADBEEF, 0xCAFEBABE, 0xA5A5A5A5A5A5A5A5] {
            let doc = make_mix(seed, kib);
            let scalar = build_struct_index_scalar_test(&doc);
            let fast = build_struct_index(&doc);
            assert_eq!(scalar.offsets, fast.offsets, "seed={seed}, kib={kib}");
        }
    }
}
