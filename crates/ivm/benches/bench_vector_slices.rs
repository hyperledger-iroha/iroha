//! Benchmarks for vector slice helpers (scalar vs auto-accelerated).
use criterion::Criterion;
use ivm::{simd_lanes, vadd32_auto, vadd64_auto, vand_auto, vor_auto, vrot32_auto, vxor_auto};

fn make_inputs(len: usize) -> (Vec<u32>, Vec<u32>) {
    let mut a = Vec::with_capacity(len);
    let mut b = Vec::with_capacity(len);
    // Deterministic patterns with mixed bits
    for i in 0..len as u32 {
        a.push(i.wrapping_mul(1664525).wrapping_add(1013904223));
        b.push(i.rotate_left(13) ^ 0xA5A5_A5A5);
    }
    (a, b)
}

fn bench_op(
    c: &mut Criterion,
    name_scalar: &str,
    name_auto: &str,
    mut scalar: impl FnMut(&[u32], &[u32], &mut [u32]),
    mut auto: impl FnMut(&[u32], &[u32], &mut [u32]),
) {
    let lanes = simd_lanes();
    let blocks = 8192usize; // total elements = lanes * blocks (~32K elems on NEON)
    let len = lanes * blocks;
    let (a, b) = make_inputs(len);
    let mut out = vec![0u32; len];

    c.bench_function(name_scalar, |bch| {
        bch.iter(|| {
            scalar(
                std::hint::black_box(&a),
                std::hint::black_box(&b),
                std::hint::black_box(&mut out),
            );
            std::hint::black_box(&out);
        })
    });

    c.bench_function(name_auto, |bch| {
        bch.iter(|| {
            auto(
                std::hint::black_box(&a),
                std::hint::black_box(&b),
                std::hint::black_box(&mut out),
            );
            std::hint::black_box(&out);
        })
    });
}

fn scalar_add32(a: &[u32], b: &[u32], out: &mut [u32]) {
    for i in 0..a.len() {
        out[i] = a[i].wrapping_add(b[i]);
    }
}

fn auto_add32(a: &[u32], b: &[u32], out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, (aa, bb)) in a.chunks(lanes).zip(b.chunks(lanes)).enumerate() {
        let r = vadd32_auto(aa, bb);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn scalar_and(a: &[u32], b: &[u32], out: &mut [u32]) {
    for i in 0..a.len() {
        out[i] = a[i] & b[i];
    }
}

fn auto_and(a: &[u32], b: &[u32], out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, (aa, bb)) in a.chunks(lanes).zip(b.chunks(lanes)).enumerate() {
        let r = vand_auto(aa, bb);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn scalar_xor(a: &[u32], b: &[u32], out: &mut [u32]) {
    for i in 0..a.len() {
        out[i] = a[i] ^ b[i];
    }
}

fn auto_xor(a: &[u32], b: &[u32], out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, (aa, bb)) in a.chunks(lanes).zip(b.chunks(lanes)).enumerate() {
        let r = vxor_auto(aa, bb);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn scalar_or(a: &[u32], b: &[u32], out: &mut [u32]) {
    for i in 0..a.len() {
        out[i] = a[i] | b[i];
    }
}

fn auto_or(a: &[u32], b: &[u32], out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, (aa, bb)) in a.chunks(lanes).zip(b.chunks(lanes)).enumerate() {
        let r = vor_auto(aa, bb);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn scalar_add64(a: &[u32], b: &[u32], out: &mut [u32]) {
    for i in (0..a.len()).step_by(2) {
        let a0 = (a[i] as u64) | ((a[i + 1] as u64) << 32);
        let b0 = (b[i] as u64) | ((b[i + 1] as u64) << 32);
        let r = a0.wrapping_add(b0);
        out[i] = (r & 0xffff_ffff) as u32;
        out[i + 1] = (r >> 32) as u32;
    }
}

fn auto_add64(a: &[u32], b: &[u32], out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, (aa, bb)) in a.chunks(lanes).zip(b.chunks(lanes)).enumerate() {
        let r = vadd64_auto(aa, bb);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn scalar_rot32(a: &[u32], k: u32, out: &mut [u32]) {
    for (o, &x) in out.iter_mut().zip(a) {
        *o = x.rotate_left(k);
    }
}

fn auto_rot32(a: &[u32], k: u32, out: &mut [u32]) {
    let lanes = simd_lanes();
    for (i, chunk) in a.chunks(lanes).enumerate() {
        let r = vrot32_auto(chunk, k);
        out[i * lanes..i * lanes + lanes].copy_from_slice(&r);
    }
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_op(
        &mut c,
        "vadd32_scalar",
        "vadd32_auto",
        scalar_add32,
        auto_add32,
    );
    bench_op(&mut c, "vand_scalar", "vand_auto", scalar_and, auto_and);
    bench_op(&mut c, "vxor_scalar", "vxor_auto", scalar_xor, auto_xor);
    bench_op(&mut c, "vor_scalar", "vor_auto", scalar_or, auto_or);
    bench_op(
        &mut c,
        "vadd64_scalar",
        "vadd64_auto",
        scalar_add64,
        auto_add64,
    );
    // Rotation: compare scalar loop vs accelerated slice path directly
    bench_rot32(&mut c);
    c.final_summary();
}

fn bench_rot32(c: &mut Criterion) {
    let lanes = simd_lanes();
    let blocks = 8192usize;
    let len = lanes * blocks;
    let (a, _b) = make_inputs(len); // reuse generator; ignore b
    let mut out = vec![0u32; len];
    let k = 13u32;

    c.bench_function("vrot32_scalar", |bch| {
        bch.iter(|| {
            scalar_rot32(&a, k, &mut out);
            std::hint::black_box(&out);
        })
    });

    c.bench_function("vrot32_auto", |bch| {
        bch.iter(|| {
            auto_rot32(&a, k, &mut out);
            std::hint::black_box(&out);
        })
    });
}
