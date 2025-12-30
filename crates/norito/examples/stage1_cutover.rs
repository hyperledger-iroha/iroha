//! Quick-and-deterministic benchmark helper for Norito JSON Stage-1 backends.
//!
//! Enable the `bench-internal` feature to compare the public entry point
//! (`build_struct_index`) against the scalar reference implementation and gather
//! per-byte timings across representative input sizes. Intended usage:
//! `cargo run -p norito --example stage1_cutover --release --features bench-internal`

use norito::core::read_len_dyn_slice;
#[cfg(feature = "bench-internal")]
use norito::json::{build_struct_index, build_struct_index_scalar_bench};

#[cfg(feature = "bench-internal")]
fn synthesize_json(target_bytes: usize) -> String {
    // Reuse a moderately nested object to exercise Stage-1 scanning without
    // spending time on string/number generation.
    let chunk = r#"{"foo":1234,"bar":"baz","ary":[1,2,3,4,5],"bool":true,"null":null}"#;
    let mut out = String::with_capacity(target_bytes + chunk.len());
    while out.len() < target_bytes {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(chunk);
        out.push_str(chunk);
    }
    out
}

#[cfg(feature = "bench-internal")]
fn time_ns<F: FnMut()>(iters: usize, mut f: F) -> u128 {
    let start = std::time::Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_nanos()
}

#[cfg(feature = "bench-internal")]
fn main() {
    // Small sample set; fast enough to run in seconds.
    let sizes = [
        512usize,
        1024,
        2048,
        4096,
        8192,
        16 * 1024,
        32 * 1024,
        64 * 1024,
        128 * 1024,
        192 * 1024,
        256 * 1024,
    ];

    println!("bytes,scalar_ns,kernel_ns,ns_per_byte_scalar,ns_per_byte_kernel");
    for &size in &sizes {
        let json = synthesize_json(size);
        let scalar_ns = time_ns(200, || {
            // Bypass heuristic selection to force the scalar backend.
            let _ = build_struct_index_scalar_bench(&json);
        });
        let kernel_ns = time_ns(200, || {
            // Force the public entry point to exercise the selected backend.
            let _ = build_struct_index(&json);
        });
        let scalar_per_byte = scalar_ns as f64 / json.len() as f64;
        let kernel_per_byte = kernel_ns as f64 / json.len() as f64;
        // `read_len_dyn_slice` is referenced to keep linkage with
        // the crate and silence unused warnings when builds omit JSON support.
        let _ = read_len_dyn_slice;
        println!(
            "{},{},{},{:.3},{:.3}",
            json.len(),
            scalar_ns / 200,
            kernel_ns / 200,
            scalar_per_byte,
            kernel_per_byte
        );
    }
}

#[cfg(not(feature = "bench-internal"))]
fn main() {
    eprintln!("Enable --features bench-internal to run this benchmark (see stage1_cutover.rs).");
    let _ = read_len_dyn_slice;
}
