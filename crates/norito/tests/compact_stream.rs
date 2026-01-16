use std::io::Cursor;

use norito::{
    core::{self, DecodeFlagsGuard, header_flags},
    stream_vec_collect_from_reader,
};

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
}

#[test]
fn write_len_varint_roundtrip() {
    core::reset_decode_state();
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

        let mut buf = Vec::new();
        core::write_len_to_vec(&mut buf, 300);
        assert_eq!(buf, vec![0xac, 0x02]);

        let (value, used) = core::read_len_from_slice(&buf).expect("decode varint length");
        assert_eq!(value, 300);
        assert_eq!(used, buf.len());
    }
    core::reset_decode_state();
}

#[test]
fn decode_packed_offsets_fixed() {
    core::reset_decode_state();
    let mut buf = Vec::new();
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&2u64.to_le_bytes());
    buf.extend_from_slice(&5u64.to_le_bytes());
    buf.extend_from_slice(&10u64.to_le_bytes());

    {
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let (offsets, used, data_len, tail_len) =
            core::decode_packed_offsets_slice(&buf, 3).expect("decode packed offsets");
        assert_eq!(offsets, vec![0, 2, 5, 10]);
        assert_eq!(used, buf.len());
        assert_eq!(data_len, 10);
        assert_eq!(tail_len, 0);
    }

    core::reset_decode_state();
}

#[test]
fn stream_vec_collect_accepts_compact_lengths() {
    let values = vec![10u32, 20, 30];

    let mut body = Vec::new();
    body.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for &value in &values {
        let bytes = value.to_le_bytes();
        push_varint(&mut body, bytes.len() as u64);
        body.extend_from_slice(&bytes);
    }

    let flags = header_flags::COMPACT_LEN;
    let framed =
        core::frame_bare_with_header_flags::<Vec<u32>>(&body, flags).expect("frame payload");

    let decoded =
        stream_vec_collect_from_reader::<_, u32>(Cursor::new(&framed)).expect("decode vec");
    assert_eq!(decoded, values);
}
