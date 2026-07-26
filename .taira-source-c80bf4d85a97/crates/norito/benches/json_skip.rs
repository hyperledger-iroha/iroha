//! Measure skip-throughput using TapeWalker vs the native DOM walker.
#![cfg(feature = "json")]

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn make_blob(n_objs: usize, payload_size: usize) -> String {
    let mut s = String::with_capacity(n_objs * (payload_size + 64));
    s.push('[');
    for i in 0..n_objs {
        if i != 0 {
            s.push(',');
        }
        s.push('{');
        s.push_str("\"payload\":");
        s.push('"');
        for _ in 0..payload_size {
            s.push('x');
        }
        s.push('"');
        s.push_str(",\"other\":");
        s.push_str(&i.to_string());
        s.push('}');
    }
    s.push(']');
    s
}

fn bench_skip(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_skip");
    for &(n, sz) in &[(128usize, 256usize), (256, 512), (512, 1024)] {
        let blob = make_blob(n, sz);
        group.throughput(Throughput::Bytes(blob.len() as u64));

        // Norito tape + skip
        group.bench_function(BenchmarkId::new("norito_skip", format!("{n}x{sz}")), |b| {
            b.iter_batched(
                || blob.as_str(),
                |input| {
                    let mut w = norito::json::TapeWalker::new(input);
                    // Expect array
                    let _ = w.next_struct(); // [
                    for _ in 0..n {
                        let _ = w.next_struct(); // {
                        // key "payload"
                        let _ = w.read_key_hash().unwrap();
                        let _ = w.expect_colon();
                        // skip payload value
                        w.skip_value().unwrap();
                        // ,"other":<num>
                        let _ = w.consume_comma_if_present();
                        let _ = w.read_key_hash().unwrap();
                        let _ = w.expect_colon();
                        // parse number via small parser
                        let s = w.input();
                        let mut p = norito::json::Parser::new_at(s, w.raw_pos());
                        let _ = p.parse_u64().unwrap();
                        w.sync_to_raw(p.position());
                        // close object and maybe comma
                        let _ = w.next_struct(); // }
                        let _ = w.consume_comma_if_present();
                    }
                    let _ = w.next_struct(); // ]
                },
                BatchSize::SmallInput,
            )
        });

        // Norito DOM: parse then walk and ignore payloads
        group.bench_function(BenchmarkId::new("norito_dom", format!("{n}x{sz}")), |b| {
            b.iter_batched(
                || blob.as_str(),
                |input| {
                    let v = norito::json::parse_value(input).unwrap();
                    let arr = v.as_array().unwrap();
                    let mut sum = 0u64;
                    for obj in arr {
                        if let norito::json::Value::Object(map) = obj
                            && let Some(val) = map.get("other")
                        {
                            sum += val.as_u64().unwrap_or(0);
                        }
                    }
                    sum
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_skip);
criterion_main!(benches);
