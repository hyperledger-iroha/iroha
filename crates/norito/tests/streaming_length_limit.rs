//! Streaming decode enforces the configured max archive length.

use std::io::Cursor;

use norito::{Error, core, stream_vec_collect_from_reader};

struct MaxArchiveGuard {
    prev: u64,
}

impl MaxArchiveGuard {
    fn new(limit: u64) -> Self {
        let prev = core::max_archive_len();
        core::set_max_archive_len(limit);
        Self { prev }
    }
}

impl Drop for MaxArchiveGuard {
    fn drop(&mut self) {
        core::set_max_archive_len(self.prev);
    }
}

fn make_header<T: core::NoritoSerialize>(len: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(core::Header::SIZE);
    bytes.extend_from_slice(b"NRT0");
    bytes.push(core::VERSION_MAJOR);
    bytes.push(core::VERSION_MINOR);
    bytes.extend_from_slice(&T::schema_hash());
    bytes.push(core::Compression::None as u8);
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes()); // checksum placeholder
    bytes.push(0); // flags
    bytes
}

#[test]
fn stream_vec_rejects_over_limit_payload() {
    let _guard = MaxArchiveGuard::new(8);
    let bytes = make_header::<Vec<u32>>(16);

    let err = stream_vec_collect_from_reader::<_, u32>(Cursor::new(bytes))
        .expect_err("over-limit payload must fail");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length: 16,
            limit: 8
        }
    ));
}
