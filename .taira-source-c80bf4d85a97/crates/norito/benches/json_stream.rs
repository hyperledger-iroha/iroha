//! Compare token streaming over Norito's TapeWalker vs native DOM traversal.
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

fn bench_stream(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_stream");
    for &(n, sz) in &[(64usize, 256usize), (128, 512), (256, 1024)] {
        let blob = make_blob(n, sz);
        group.throughput(Throughput::Bytes(blob.len() as u64));

        group.bench_function(
            BenchmarkId::new("norito_stream", format!("{n}x{sz}")),
            |b| {
                b.iter_batched(
                    || blob.as_str(),
                    |input| {
                        let mut rdr = norito::json::Reader::new(input);
                        let mut count = 0usize;
                        for tok in rdr.tokens() {
                            match tok.unwrap() {
                                norito::json::Token::KeyBorrowed(_)
                                | norito::json::Token::StringBorrowed(_)
                                | norito::json::Token::Number(_)
                                | norito::json::Token::Bool(_)
                                | norito::json::Token::Null
                                | norito::json::Token::StartObject
                                | norito::json::Token::EndObject
                                | norito::json::Token::StartArray
                                | norito::json::Token::EndArray => {
                                    count += 1;
                                }
                            }
                        }
                        count
                    },
                    BatchSize::SmallInput,
                )
            },
        );

        group.bench_function(BenchmarkId::new("norito_dom", format!("{n}x{sz}")), |b| {
            b.iter_batched(
                || blob.as_str(),
                |input| {
                    let v = norito::json::parse_value(input).unwrap();
                    fn walk(v: &norito::json::Value, c: &mut usize) {
                        match v {
                            norito::json::Value::Null => *c += 1,
                            norito::json::Value::Bool(_) => *c += 1,
                            norito::json::Value::Number(_) => *c += 1,
                            norito::json::Value::String(_) => *c += 1,
                            norito::json::Value::Array(a) => {
                                *c += 1; // start
                                for x in a {
                                    walk(x, c);
                                }
                                *c += 1; // end
                            }
                            norito::json::Value::Object(m) => {
                                *c += 1; // start
                                for v in m.values() {
                                    *c += 1;
                                    walk(v, c);
                                }
                                *c += 1; // end
                            }
                        }
                    }
                    let mut count = 0usize;
                    walk(&v, &mut count);
                    count
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_stream);
criterion_main!(benches);
