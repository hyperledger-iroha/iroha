#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Clone, Debug, Arbitrary)]
struct Row {
    id: u64,
    #[arbitrary(with = arbitrary_string_cap)]
    name: String,
    val: u32,
    flag: bool,
}

fn arbitrary_string_cap(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<String> {
    let n: usize = u.int_in_range(0..=24)?;
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        let c: u8 = u.int_in_range(0x20..=0x7e)?;
        s.push(c as char);
    }
    Ok(s)
}

fuzz_target!(|rows: Vec<Row>| {
    let rows = rows.into_iter().take(64).collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    let borrowed: Vec<(u64, &str, u32, bool)> = rows
        .iter()
        .map(|r| (r.id, r.name.as_str(), r.val, r.flag))
        .collect();
    // Encode via AoS and NCB and ensure both decoders roundtrip to equal owned rows
    let aos = norito::aos::encode_rows_u64_str_u32_bool(&borrowed);
    let ncb = norito::columnar::encode_ncb_u64_str_u32_bool(&borrowed);

    let mut out_aos = Vec::new();
    {
        let view = norito::columnar::view_aos_u64_str_u32_bool(&aos).expect("view aos");
        for i in 0..view.len() {
            out_aos.push((
                view.id(i),
                view.name(i).unwrap().to_string(),
                view.val(i),
                view.flag(i),
            ));
        }
    }
    let mut out_ncb = Vec::new();
    {
        let view = norito::columnar::view_ncb_u64_str_u32_bool(&ncb).expect("view ncb");
        for i in 0..view.len() {
            out_ncb.push((
                view.id(i),
                view.name(i).unwrap().to_string(),
                view.val(i),
                view.flag(i),
            ));
        }
    }
    assert_eq!(out_aos, out_ncb);
});
