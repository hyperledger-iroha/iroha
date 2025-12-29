#![no_main]
use libfuzzer_sys::fuzz_target;

fn escape_as_json_string_content(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len());
    for &b in data {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x00..=0x1F => {
                out.push_str("\\u00");
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push(HEX[((b >> 4) & 0x0F) as usize] as char);
                out.push(HEX[(b & 0x0F) as usize] as char);
            }
            0x20..=0x7E => out.push(b as char),
            _ => {
                out.push_str("\\u00");
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push(HEX[((b >> 4) & 0x0F) as usize] as char);
                out.push(HEX[(b & 0x0F) as usize] as char);
            }
        }
    }
    out
}

fn expected_from_bytes(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len());
    for &b in data {
        match b {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            0x00..=0x1F => out.push(b as char),
            0x20..=0x7E => out.push(b as char),
            _ => out.push(char::from_u32((b as u32) & 0xFF).unwrap()),
        }
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let content = escape_as_json_string_content(data);
    let s = format!("\"{}\"", content);
    let expect = expected_from_bytes(data);
    let mut p = norito::json::Parser::new(&s);
    let mut arena = norito::json::Arena::new();
    match p.parse_string_ref(&mut arena) {
        Ok(sr) => {
            let got = match sr {
                norito::json::StrRef::Borrowed(b) => b.to_string(),
                norito::json::StrRef::Owned(o) => o.to_string(),
            };
            assert_eq!(got, expect);
        }
        Err(_e) => {}
    }
});
