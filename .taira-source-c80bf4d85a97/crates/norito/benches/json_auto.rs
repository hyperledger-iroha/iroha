//! Compare decode speeds for from_json, from_json_fast, and from_json_auto.
#![cfg(feature = "json")]

use criterion::{Criterion, criterion_group, criterion_main};

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Inner {
    value: f64,
    flags: Vec<bool>,
    tags: Vec<String>,
    opt: Option<String>,
}

#[derive(Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Outer {
    id: u64,
    inner: Inner,
    items: Vec<Inner>,
}

impl norito::json::JsonDeserialize for Inner {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut value = None;
        let mut flags: Option<Vec<bool>> = None;
        let mut tags: Option<Vec<String>> = None;
        let mut opt: Option<Option<String>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "value" => value = Some(p.parse_f64()?),
                "flags" => flags = Some(p.parse_array::<bool>()?),
                "tags" => tags = Some(p.parse_array::<String>()?),
                "opt" => {
                    p.skip_ws();
                    if p.try_consume_null()? {
                        opt = Some(None);
                    } else {
                        opt = Some(Some(p.parse_string()?));
                    }
                }
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Inner {
            value: value.unwrap(),
            flags: flags.unwrap(),
            tags: tags.unwrap(),
            opt: opt.unwrap(),
        })
    }
}

impl norito::json::JsonDeserialize for Outer {
    fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut inner = None;
        let mut items: Option<Vec<Inner>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "inner" => inner = Some(Inner::json_deserialize(p)?),
                "items" => items = Some(p.parse_array::<Inner>()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Outer {
            id: id.unwrap(),
            inner: inner.unwrap(),
            items: items.unwrap(),
        })
    }
}

fn sample_large_json() -> String {
    let mut items = String::from("[");
    for i in 0..128 {
        if i > 0 {
            items.push(',');
        }
        let tag = format!("tag{i}");
        let val = 1.0 + (i as f64) * 0.25;
        let opt = if i % 3 == 0 {
            format!("\"o{i}\"")
        } else {
            "null".to_string()
        };
        items.push_str(&format!(
            "{{\"value\":{val},\"flags\":[true,false,true],\"tags\":[\"{tag}\",\"{tag}\"],\"opt\":{opt}}}"
        ));
    }
    items.push(']');
    format!(
        "{{\"id\":777,\"inner\":{{\"value\":-1.25e2,\"flags\":[false,true,false],\"tags\":[\"p\",\"q\"],\"opt\":\"z\"}},\"items\":{items}}}"
    )
}

fn bench_json_auto(c: &mut Criterion) {
    let small = r#"{"id":1,"inner":{"value":3.14,"flags":[true,false],"tags":["a"],"opt":null},"items":[{"value":1.0,"flags":[true],"tags":["x"],"opt":"y"}]}"#;
    let large = sample_large_json();

    c.bench_function("decode_small_from_json", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json(std::hint::black_box(small)).unwrap();
            std::hint::black_box(v)
        })
    });
    c.bench_function("decode_small_from_json_fast", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json_fast(std::hint::black_box(small)).unwrap();
            std::hint::black_box(v)
        })
    });
    c.bench_function("decode_small_from_json_auto", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json_auto(std::hint::black_box(small)).unwrap();
            std::hint::black_box(v)
        })
    });

    c.bench_function("decode_large_from_json", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json(std::hint::black_box(&large)).unwrap();
            std::hint::black_box(v)
        })
    });
    c.bench_function("decode_large_from_json_fast", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json_fast(std::hint::black_box(&large)).unwrap();
            std::hint::black_box(v)
        })
    });
    c.bench_function("decode_large_from_json_auto", |b| {
        b.iter(|| {
            let v: Outer = norito::json::from_json_auto(std::hint::black_box(&large)).unwrap();
            std::hint::black_box(v)
        })
    });
}

criterion_group!(benches, bench_json_auto);
criterion_main!(benches);
