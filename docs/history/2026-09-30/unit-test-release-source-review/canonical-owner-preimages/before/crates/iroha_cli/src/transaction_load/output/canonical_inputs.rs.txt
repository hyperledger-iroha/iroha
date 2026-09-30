// Raw file identities for retained load outputs. This owner grants no finality proof.
use super::*;
use sha2::{Digest as _, Sha256};
use std::os::unix::fs::FileExt as _;

/// A raw file identity, distinct from Iroha's typed hash and from proof authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawFileIdentity {
    pub(crate) raw_sha256: [u8; 32],
    pub(crate) byte_length: u64,
}

fn read_at(file: &File, mut bytes: &mut [u8], mut offset: u64) -> Result<()> {
    while !bytes.is_empty() {
        let n = file.read_at(bytes, offset)?;
        ensure!(n > 0, "retained file became short");
        offset = offset
            .checked_add(u64::try_from(n)?)
            .ok_or_else(|| eyre!("read offset overflow"))?;
        bytes = &mut bytes[n..];
    }
    Ok(())
}
/// Hash exactly `length` retained bytes and reject a file that grew past them.
pub(super) fn raw_digest(file: &File, length: u64) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < length {
        let count = usize::try_from((length - offset).min(buffer.len() as u64))?;
        read_at(file, &mut buffer[..count], offset)?;
        hasher.update(&buffer[..count]);
        offset += count as u64;
    }
    ensure!(
        file.read_at(&mut buffer[..1], length)? == 0,
        "retained file gained trailing bytes"
    );
    Ok(hasher.finalize().into())
}
