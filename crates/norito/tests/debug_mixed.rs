//! Debug print for Mixed encoding layout
#![cfg(feature = "json")]

use iroha_schema::IntoSchema;
use norito::core::to_bytes;

#[derive(IntoSchema, norito::derive::Encode, norito::derive::Decode, PartialEq, Debug)]
struct Mixed {
    name: String,
    nums: Vec<u32>,
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn print_mixed_layout() {
    if norito::core::default_encode_flags() == 0 {
        return;
    }
    let value = Mixed {
        name: "hi".into(),
        nums: vec![1, 2, 3],
    };
    let bytes = to_bytes(&value).expect("encode");
    eprintln!("header+payload len={}", bytes.len());
    // Print header, then first 32 payload bytes
    let header = &bytes[..norito::core::Header::SIZE];
    let payload = &bytes[norito::core::Header::SIZE..];
    eprintln!(
        "header={} flags={:02x}",
        hex(header),
        header[header.len() - 1]
    );
    let show = payload.len().min(64);
    eprintln!("payload[0..{show}]={}", hex(&payload[..show]));

    // Parse hybrid packed-struct: [bitset][sizes*][data...]
    let mut o = 0usize;
    let bitset = payload[o];
    o += 1;
    eprintln!("bitset={bitset:02x}");
    // Read one size (nums) because bit 1 is set
    let (sz, hdr) = if (bitset & 0x02) != 0 {
        let (sz, hdr) = norito::core::read_len_from_slice(&payload[o..]).expect("sz");
        eprintln!("sizes varint: sz={sz} hdr={hdr} bytes={:02x}", payload[o]);
        (sz, hdr)
    } else {
        (0, 0)
    };
    o += hdr;
    // Now data block starts at o
    let start_data = o;
    // name: expect inner header + bytes
    let (nlen, nh) = norito::core::read_len_from_slice(&payload[o..]).expect("nlen");
    eprintln!("name: nlen={nlen} nhdr={nh} first={:02x}", payload[o]);
    o += nh + nlen;
    // nums follows; its total size should be sz
    let nums_bytes = &payload[o..o + sz];
    eprintln!("nums[..]={}", hex(&nums_bytes[..nums_bytes.len().min(32)]));
    o += sz;
    eprintln!("consumed data bytes={}", o - start_data);
    // Decode to ensure it still decodes
    let dec: Mixed = norito::decode_from_bytes(&bytes).expect("decode");
    eprintln!("decoded={dec:?}");
}
