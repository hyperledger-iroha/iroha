//! Source-selected suffix namespace, independent of every ordinary persisted wallet schema.
//!
//! The provider selects one checkpoint. Its declared schema either names the original
//! Manifest or a new envelope selecting that exact Manifest plus a suffix manifest. No failed
//! parse is retried as another type. Ordinary publications retain the selected suffix root.
//! Records contain DATA and original references only, never a serialized verified capability.

use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KAGEMUSHA_WALLET_ARCHIVE_MANIFEST_MAX_BYTES_V1,
    kagemusha_wallet_archive_checkpoint_digest_v1 as checkpoint_digest,
    kagemusha_wallet_archive_object_digest_v1 as object_digest,
};
use manifest::Manifest;

mod records;
pub(super) use records::*;

pub(super) const RECORD_BOUND: usize = 4096;

pub(super) struct SelectedCheckpoint {
    pub digest: [u8; 32],
    pub manifest: Manifest,
    pub suffix: Option<SuffixArchiveManifestV1>,
}

fn lost(field: &'static str) -> Error {
    Error::WitnessLost(field)
}

fn frame_schema(bytes: &[u8]) -> Result<[u8; 16], Error> {
    Ok(norito::core::Header::read(bytes)
        .map_err(|_| lost("suffix checkpoint schema"))?
        .schema)
}

pub(super) fn read_record<T>(store: &mut impl ObjectStore, address: [u8; 32]) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if address == [0; 32] {
        return Err(lost("suffix record zero address"));
    }
    let bytes = store.read_object(&address, RECORD_BOUND)?;
    if object_digest(&bytes) != address {
        return Err(lost("suffix record content identity"));
    }
    archive::decode(&bytes)
}

pub(super) fn write_record<T>(store: &mut impl ObjectStore, record: &T) -> Result<[u8; 32], Error>
where
    T: norito::NoritoSerialize,
{
    let bytes = archive::encode(record)?;
    if bytes.len() > RECORD_BOUND {
        return Err(Error::Invalid("suffix record bound"));
    }
    let expected = object_digest(&bytes);
    if store.write_object(&bytes, RECORD_BOUND)? != expected {
        return Err(lost("suffix record publication identity"));
    }
    Ok(expected)
}

pub(super) fn decode_checkpoint(
    archive: &mut impl ObjectStore,
    binding: ([u8; 32], [u8; 32]),
    digest: [u8; 32],
    bytes: &[u8],
) -> Result<SelectedCheckpoint, Error> {
    if bytes.len() > KAGEMUSHA_WALLET_ARCHIVE_MANIFEST_MAX_BYTES_V1
        || checkpoint_digest(bytes) != digest
    {
        return Err(lost("suffix checkpoint identity"));
    }
    let schema = frame_schema(bytes)?;
    let (manifest, suffix) = if schema == norito::schema::identity::frame_hash::<Manifest>() {
        (archive::decode(bytes)?, None)
    } else if schema == norito::schema::identity::frame_hash::<SuffixArchiveCheckpointV1>() {
        let envelope: SuffixArchiveCheckpointV1 = archive::decode(bytes)?;
        envelope.require(binding)?;
        let manifest: Manifest = read_record(archive, envelope.manifest_original)?;
        let suffix: SuffixArchiveManifestV1 = read_record(archive, envelope.suffix_manifest)?;
        suffix.require(binding)?;
        (manifest, Some(suffix))
    } else {
        return Err(lost("unknown wallet checkpoint schema"));
    };
    require_manifest(&manifest, binding)?;
    Ok(SelectedCheckpoint {
        digest,
        manifest,
        suffix,
    })
}

pub(super) fn require_manifest(
    manifest: &Manifest,
    binding: ([u8; 32], [u8; 32]),
) -> Result<(), Error> {
    if (manifest.scheme_id, manifest.wallet_id) != binding
        || manifest.indexed.is_none()
        || manifest
            .folded
            .zip(manifest.indexed)
            .is_some_and(|(a, b)| a > b)
        || (manifest.checkpoint_count == 0) != (manifest.checkpoint_digest == [0; 32])
    {
        return Err(lost("archive manifest binding"));
    }
    Ok(())
}

pub(super) fn checkpoint_bytes(
    archive: &mut impl ObjectStore,
    manifest: &Manifest,
    suffix: Option<&SuffixArchiveManifestV1>,
) -> Result<Vec<u8>, Error> {
    let binding = (manifest.scheme_id, manifest.wallet_id);
    require_manifest(manifest, binding)?;
    let Some(suffix) = suffix else {
        return archive::encode(manifest);
    };
    suffix.require(binding)?;
    // Every referenced record is staged before the root; provider publication drains them.
    let envelope = SuffixArchiveCheckpointV1 {
        version: 1,
        discriminator: 1,
        scheme_id: binding.0,
        wallet_id: binding.1,
        manifest_original: write_record(archive, manifest)?,
        suffix_manifest: write_record(archive, suffix)?,
    };
    let bytes = archive::encode(&envelope)?;
    if bytes.len() > KAGEMUSHA_WALLET_ARCHIVE_MANIFEST_MAX_BYTES_V1 {
        return Err(Error::Invalid("suffix checkpoint bound"));
    }
    Ok(bytes)
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn selected_suffix_checkpoint(
        &mut self,
    ) -> Result<Option<SelectedCheckpoint>, Error> {
        let Some((digest, bytes)) = self.custody.archive_checkpoint()? else {
            return Ok(None);
        };
        decode_checkpoint(
            &mut self.archive,
            (self.scheme_id, self.wallet_id),
            digest,
            &bytes,
        )
        .map(Some)
    }
    pub(super) fn publish_suffix_checkpoint(
        &mut self,
        expected: [u8; 32],
        manifest: &Manifest,
        suffix: &SuffixArchiveManifestV1,
    ) -> Result<[u8; 32], Error> {
        let bytes = checkpoint_bytes(&mut self.archive, manifest, Some(suffix))?;
        let digest = checkpoint_digest(&bytes);
        if self.custody.publish_archive_checkpoint(expected, &bytes)? != digest {
            return Err(lost("suffix checkpoint publication"));
        }
        Ok(digest)
    }
}

#[cfg(test)]
mod tests;
