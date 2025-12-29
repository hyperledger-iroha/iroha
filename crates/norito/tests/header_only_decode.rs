//! Header-based decoders must reject bare Norito payloads.

use std::io::Cursor;

use norito::{
    Error, codec::Encode, decode_from_reader, from_bytes, stream_vec_collect_from_reader,
};

#[test]
fn headerless_payloads_are_rejected() {
    let values = vec![1u32, 2, 3];
    let bare = values.encode();

    let err = match from_bytes::<Vec<u32>>(&bare) {
        Ok(_) => panic!("from_bytes accepted bare payload"),
        Err(err) => err,
    };
    assert!(matches!(err, Error::InvalidMagic));

    let err = decode_from_reader::<_, Vec<u32>>(Cursor::new(&bare))
        .expect_err("decode_from_reader must reject bare payloads");
    assert!(matches!(err, Error::InvalidMagic));

    let err = stream_vec_collect_from_reader::<_, u32>(Cursor::new(&bare))
        .expect_err("streaming decode must reject bare payloads");
    assert!(matches!(err, Error::InvalidMagic));
}
