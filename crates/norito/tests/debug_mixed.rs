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
    let flags = header[header.len() - 1];
    let show = payload.len().min(64);
    eprintln!("payload[0..{show}]={}", hex(&payload[..show]));

    if (flags & norito::core::header_flags::PACKED_STRUCT) == 0 {
        let mut o = 0usize;
        let (name_field_len, name_field_header) =
            norito::core::read_len_from_slice(&payload[o..]).expect("name field length");
        o += name_field_header;
        assert!(o + name_field_len <= payload.len());
        let name_end = o + name_field_len;
        let (name_len, name_header) =
            norito::core::read_len_from_slice(&payload[o..name_end]).expect("name length");
        assert_eq!(&payload[o + name_header..name_end], b"hi");
        assert_eq!(name_len, 2);
        o = name_end;

        let (nums_field_len, nums_field_header) =
            norito::core::read_len_from_slice(&payload[o..]).expect("nums field length");
        o += nums_field_header;
        assert!(o + nums_field_len <= payload.len());
        o += nums_field_len;
        assert_eq!(o, payload.len());

        let dec: Mixed = norito::decode_from_bytes(&bytes).expect("decode");
        eprintln!("decoded={dec:?}");
        return;
    }

    // Parse hybrid packed-struct: [bitset][sizes*][data...]
    let mut o = 0usize;
    let bitset = payload[o];
    o += 1;
    eprintln!("bitset={bitset:02x}");
    let mut sizes = [0usize; 2];
    for (idx, size) in sizes.iter_mut().enumerate() {
        if (bitset & (1u8 << idx)) != 0 {
            let (field_size, hdr) = norito::core::read_len_from_slice(&payload[o..]).expect("sz");
            eprintln!(
                "size[{idx}]={field_size} hdr={hdr} first={:02x}",
                payload[o]
            );
            *size = field_size;
            o += hdr;
        }
    }
    // Now data block starts at o
    let start_data = o;
    assert!(o + sizes[0] <= payload.len());
    o += sizes[0];
    assert!(o + sizes[1] <= payload.len());
    let nums_bytes = &payload[o..o + sizes[1]];
    eprintln!("nums[..]={}", hex(&nums_bytes[..nums_bytes.len().min(32)]));
    o += sizes[1];
    eprintln!("consumed data bytes={}", o - start_data);
    // Decode to ensure it still decodes
    let dec: Mixed = norito::decode_from_bytes(&bytes).expect("decode");
    eprintln!("decoded={dec:?}");
}
