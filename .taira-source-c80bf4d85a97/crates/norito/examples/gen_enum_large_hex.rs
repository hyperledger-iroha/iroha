//! Generate the enum_offsets_id_code_delta_large hex to stdout.
use norito::columnar as ncb;

fn main() {
    use ncb::EnumBorrow;
    let n = 300usize;
    let mut rows: Vec<(u64, EnumBorrow<'_>, bool)> = Vec::with_capacity(n);
    let mut id = 1u64;
    let mut code = 100u32;
    for i in 0..n {
        let flag = i % 5 == 0;
        if i % 3 == 0 {
            let name = match i % 4 {
                0 => "aa",
                1 => "bbb",
                2 => "cccc",
                _ => "d",
            };
            rows.push((id, EnumBorrow::Name(name), flag));
        } else {
            rows.push((id, EnumBorrow::Code(code), flag));
            code = code.wrapping_add((i % 7 + 1) as u32);
        }
        id = id.wrapping_add((i % 9 + 1) as u64);
    }
    let bytes = ncb::encode_ncb_u64_enum_bool(&rows, true, false, true);
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in &bytes {
        use core::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    println!("{s}");
}
