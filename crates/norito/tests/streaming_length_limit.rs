//! Streaming decode enforces the configured max archive length.
use norito::{Error, core, stream_vec_collect_from_reader};
use std::io::Cursor;
fn make_header<T: core::NoritoSerialize>(len: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(core::Header::SIZE);
    bytes.extend_from_slice(b"NRT0");
    bytes.push(core::VERSION_MAJOR);
    bytes.push(core::VERSION_MINOR);
    bytes.extend_from_slice(&norito::schema::identity::frame_hash::<T>());
    bytes.push(core::Compression::None as u8);
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes()); // checksum placeholder
    bytes.push(0); // flags
    bytes
}
#[test]
fn stream_vec_rejects_over_limit_payload() {
    // Do not lower the process-wide limit while other grouped tests decode.
    let limit = core::max_archive_len();
    let length = limit
        .checked_add(1)
        .expect("archive limit leaves an invalid length");
    let bytes = make_header::<Vec<u32>>(length);
    let err = stream_vec_collect_from_reader::<_, u32>(Cursor::new(bytes))
        .expect_err("over-limit payload must fail");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length: observed_length,
            limit: observed_limit
        } if observed_length == length && observed_limit == limit
    ));
}

#[test]
fn stream_vec_rejects_over_limit_before_reading_payload() {
    let limit = core::max_archive_len();
    let length = limit
        .checked_add(1)
        .expect("archive limit leaves an invalid length");
    let mut bytes = make_header::<Vec<u32>>(length);
    let payload = [0xa5; 8];
    bytes.extend_from_slice(&payload);
    let mut reader = Cursor::new(bytes);

    let err = stream_vec_collect_from_reader::<_, u32>(&mut reader)
        .expect_err("over-limit header must fail before payload reads");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length: observed_length,
            limit: observed_limit
        } if observed_length == length && observed_limit == limit
    ));
    assert_eq!(reader.position(), core::Header::SIZE as u64);
    assert_eq!(&reader.get_ref()[core::Header::SIZE..], &payload);
    assert_eq!(core::max_archive_len(), limit);
}
