//! Bounded comparison of an interrupted compaction against authenticated state.

use std::io::{self, Read};

const SCRATCH_BYTES: usize = 4096;

/// Verify exactly the admitted prefix and EOF on the retained temp-file handle.
///
/// A growing temp must not allocate or read beyond its original length plus one
/// detection byte. File identity, link, and cleanup authority remain with the
/// reservation journal; this function never opens, seeks, or removes a path.
pub(super) fn verify_prefix(
    reader: &mut impl Read,
    expected: &[u8],
    admitted_len: u64,
) -> io::Result<()> {
    let admitted_len = usize::try_from(admitted_len).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "lane reservation compaction temp exceeds usize",
        )
    })?;
    let prefix = expected.get(..admitted_len).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "lane reservation compaction temp exceeds the authenticated compacted state",
        )
    })?;
    let mut scratch = [0_u8; SCRATCH_BYTES];
    let mut matches = true;
    for chunk in prefix.chunks(SCRATCH_BYTES) {
        let actual = &mut scratch[..chunk.len()];
        reader.read_exact(actual)?;
        matches &= actual == chunk;
    }
    loop {
        match reader.read(&mut scratch[..1]) {
            Ok(0) => break,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "lane reservation compaction temp identity or length changed during reconciliation",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    if !matches {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lane reservation compaction temp is not an authenticated prefix of canonical state",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "reservation_compaction_prefix_tests.rs"]
mod tests;
