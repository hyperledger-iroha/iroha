//! Benchmarks for vectorized SIMD helpers in IVM.
use criterion::Criterion;
use ivm::{simd_lanes, vadd32, vadd64_auto, vand_auto, vor_auto, vrot32, vxor_auto};

fn bench_vadd32(c: &mut Criterion) {
    let a = [1u32, 2, 3, 4];
    let b = [4u32, 3, 2, 1];
    c.bench_function("vadd32", |bch| {
        bch.iter(|| {
            std::hint::black_box(vadd32(a, b));
        })
    });
}

fn bench_vrot32(c: &mut Criterion) {
    let a = [0x1111_2222u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    c.bench_function("vrot32", |bch| {
        bch.iter(|| {
            std::hint::black_box(vrot32(a, 8));
        })
    });
}

fn bench_vadd64_auto(c: &mut Criterion) {
    let lanes = simd_lanes();
    let a: Vec<u32> = (0..lanes as u32).collect();
    let b: Vec<u32> = (0..lanes as u32).rev().collect();
    c.bench_function("vadd64_auto", |bch| {
        bch.iter(|| {
            std::hint::black_box(vadd64_auto(&a, &b));
        })
    });
}

fn bench_vand_auto(c: &mut Criterion) {
    let lanes = simd_lanes();
    let a: Vec<u32> = vec![0xFFFF_FFFF; lanes];
    let b: Vec<u32> = vec![0x0F0F_0F0F; lanes];
    c.bench_function("vand_auto", |bch| {
        bch.iter(|| {
            std::hint::black_box(vand_auto(&a, &b));
        })
    });
}

fn bench_vxor_auto(c: &mut Criterion) {
    let lanes = simd_lanes();
    let a: Vec<u32> = vec![0xAAAA_AAAA; lanes];
    let b: Vec<u32> = vec![0x5555_5555; lanes];
    c.bench_function("vxor_auto", |bch| {
        bch.iter(|| {
            std::hint::black_box(vxor_auto(&a, &b));
        })
    });
}

fn bench_vor_auto(c: &mut Criterion) {
    let lanes = simd_lanes();
    let a: Vec<u32> = vec![0xF0F0_F0F0; lanes];
    let b: Vec<u32> = vec![0x0F0F_0F0F; lanes];
    c.bench_function("vor_auto", |bch| {
        bch.iter(|| {
            std::hint::black_box(vor_auto(&a, &b));
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_vadd32(&mut c);
    bench_vrot32(&mut c);
    bench_vadd64_auto(&mut c);
    bench_vand_auto(&mut c);
    bench_vxor_auto(&mut c);
    bench_vor_auto(&mut c);
    c.final_summary();
}
