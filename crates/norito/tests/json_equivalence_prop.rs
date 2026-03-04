#![cfg(feature = "json")]
//! Property and equivalence tests for Norito's JSON fast path.

use norito::json::{JsonDeserialize, Parser, from_json_fast};
use proptest::prelude::*;

// A nested structure to exercise nested objects, options, vectors, and mixed types.
#[derive(Clone, Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Inner {
    count: u32,
    title: String,
}

impl JsonDeserialize for Inner {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut count = None;
        let mut title = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "count" => count = Some(p.parse_u64().map(|v| v as u32)?),
                "title" => title = Some(p.parse_string()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Inner {
            count: count.unwrap(),
            title: title.unwrap(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, norito::derive::FastJson, norito::derive::FastJsonWrite)]
struct Outer {
    id: u64,
    name: String,
    active: bool,
    ratio: f64,
    tags: Vec<String>,
    nested: Option<Inner>,
    nums: Vec<u64>,
}

impl JsonDeserialize for Outer {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, norito::json::Error> {
        p.skip_ws();
        p.consume_char(b'{')?;
        let mut id = None;
        let mut name = None;
        let mut active = None;
        let mut ratio = None;
        let mut tags: Option<Vec<String>> = None;
        let mut nested: Option<Option<Inner>> = None;
        let mut nums: Option<Vec<u64>> = None;
        loop {
            p.skip_ws();
            if p.try_consume_char(b'}')? {
                break;
            }
            let k = p.parse_key()?;
            match k.as_ref() {
                "id" => id = Some(p.parse_u64()?),
                "name" => name = Some(p.parse_string()?),
                "active" => active = Some(p.parse_bool()?),
                "ratio" => ratio = Some(p.parse_f64()?),
                "tags" => tags = Some(p.parse_array::<String>()?),
                "nested" => {
                    p.skip_ws();
                    if p.try_consume_null()? {
                        nested = Some(None);
                    } else {
                        nested = Some(Some(Inner::json_deserialize(p)?));
                    }
                }
                "nums" => nums = Some(p.parse_array::<u64>()?),
                _ => return Err(norito::json::Error::Message("unexpected key".into())),
            }
            let _ = p.consume_comma_if_present()?;
        }
        Ok(Outer {
            id: id.unwrap(),
            name: name.unwrap(),
            active: active.unwrap(),
            ratio: ratio.unwrap(),
            tags: tags.unwrap_or_default(),
            nested: nested.unwrap_or(None),
            nums: nums.unwrap_or_default(),
        })
    }
}

// Strategy for finite f64 values to avoid JSON NaN/Inf issues.
fn finite_f64() -> impl Strategy<Value = f64> {
    any::<f64>().prop_filter("finite", |f| f.is_finite())
}

fn arb_inner() -> impl Strategy<Value = Inner> {
    (
        any::<u32>(),
        // Keep strings small for test speed; JSON quoting handles escapes.
        "(?s).{0,32}".prop_map(|s| s),
    )
        .prop_map(|(count, title)| Inner { count, title })
}

fn arb_outer() -> impl Strategy<Value = Outer> {
    (
        any::<u64>(),
        "(?s).{0,32}".prop_map(|s| s),
        any::<bool>(),
        finite_f64(),
        prop::collection::vec("(?s).{0,16}".prop_map(|s| s), 0..8),
        prop::option::of(arb_inner()),
        prop::collection::vec(any::<u64>(), 0..16),
    )
        .prop_map(|(id, name, active, ratio, tags, nested, nums)| Outer {
            id,
            name,
            active,
            ratio,
            tags,
            nested,
            nums,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, .. ProptestConfig::default() })]
    #[test]
    fn from_json_fast_matches_generic_for_outer(ref o in arb_outer()) {
        let json = norito::json::to_json(o).expect("serialize json");
        let slow: Outer = norito::json::from_json(&json).expect("generic parse");
        let fast: Outer = from_json_fast(&json).expect("fast parse");
        prop_assert_eq!(slow, fast);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]
    #[test]
    fn tape_parse_string_ref_inline_matches_input(ref s in "(?s).{0,64}") {
        let quoted = norito::json::to_json(&norito::json::Value::String(s.clone())).unwrap();
        let mut w = norito::json::TapeWalker::new(&quoted);
        let mut arena = norito::json::Arena::new();
        let out = w.parse_string_ref_inline(&mut arena).expect("string ref");
        let got = match out { norito::json::StrRef::Borrowed(b) => b.to_string(), norito::json::StrRef::Owned(o) => o.to_string() };
        prop_assert_eq!(got, s.as_str());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]
    #[test]
    fn tape_skip_value_advances_to_second_string(ref a in "(?s).{0,64}", ref b in "(?s).{0,64}") {
        let qa = norito::json::to_json(&norito::json::Value::String(a.clone())).unwrap();
        let qb = norito::json::to_json(&norito::json::Value::String(b.clone())).unwrap();
        let s = format!("[{qa},{qb}]");
        let mut w = norito::json::TapeWalker::new(&s);
        // Move past '[' using the structural tape
        if let Some((off, ch)) = w.peek_struct() { assert_eq!(ch, b'['); let _ = w.next_struct(); w.sync_to_raw(off + 1); }
        // Skip first value of arbitrary shape (here: guaranteed to be a string)
        w.skip_value().expect("skip first");
        // Consume comma
        let _ = w.consume_comma_if_present().expect("comma");
        // Parse the second value
        let mut arena = norito::json::Arena::new();
        let sr = w.parse_string_ref_inline(&mut arena).expect("second string");
        let got = match sr { norito::json::StrRef::Borrowed(b) => b.to_string(), norito::json::StrRef::Owned(o) => o.to_string() };
        prop_assert_eq!(got, b.as_str());
    }
}
