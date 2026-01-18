use ivm::{ProgramMetadata, encoding, ivm_cache};

fn simple_prog(nops: usize) -> Vec<u8> {
    let meta = ProgramMetadata::default().encode();
    let mut bytes = meta;
    for _ in 0..nops {
        // encode a cheap arithmetic op (wide `ADDI x1, x1, 0`)
        let addi = ivm::kotodama::compiler::encode_addi(1, 1, 0)
            .expect("encode addi");
        bytes.extend_from_slice(&addi.to_le_bytes());
    }
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn global_cache_hits_increase_on_repeat() {
    let prog = simple_prog(64);
    let parsed = ivm::ProgramMetadata::parse(&prog).unwrap();

    let meta = parsed.metadata;

    let off = parsed.code_offset;
    let code = &prog[off..];
    // Reset global counters by reading before
    let _ = ivm_cache::global_counters();
    let (_h0, m0, _e0) = ivm_cache::global_counters();
    // First predecode: miss
    let _ = ivm_cache::global_get_with_meta(code, &meta).expect("predecode ok");
    let (h1, m1, e1) = ivm_cache::global_counters();
    assert!(m1 > m0, "expected at least one miss");
    // Second predecode: hit
    let _ = ivm_cache::global_get_with_meta(code, &meta).expect("predecode hit");
    let (h2, m2, e2) = ivm_cache::global_counters();
    assert!(h2 > h1, "expected at least one hit");
    assert!(m2 >= m1, "misses should not decrease");
    assert!(e2 >= e1, "evictions monotonic");
}
