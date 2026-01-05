//! Varint helper coverage for compact length prefixes.

use norito::core;

#[test]
fn varint_len_prefix_len_counts_bytes() {
    assert_eq!(core::varint_len_prefix_len(0), 1);
    assert_eq!(core::varint_len_prefix_len(0x7f), 1);
    assert_eq!(core::varint_len_prefix_len(0x80), 2);
    assert_eq!(core::varint_len_prefix_len(0x3fff), 2);
    assert_eq!(core::varint_len_prefix_len(0x4000), 3);
}

#[test]
fn write_varint_len_helpers_encode_expected() {
    let mut buf = Vec::new();
    core::write_varint_len(&mut buf, 300).expect("write varint len");
    assert_eq!(buf, vec![0xac, 0x02]);

    let mut buf2 = Vec::new();
    core::write_varint_len_to_vec(&mut buf2, 300);
    assert_eq!(buf2, vec![0xac, 0x02]);
}
