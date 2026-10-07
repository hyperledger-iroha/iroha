//! Exact server D/V/PK reads selected by the authenticated producer inventory.

use std::{io::Read as _, path::Path};

use iroha_kagemusha_proof::finality::{
    catalog::ArtifactRecord,
    continuity::{producer::Error, tree::OriginalBytes},
    native::{ArtifactId, ArtifactSource},
};
use sha2::{Digest as _, Sha256};

use crate::kagemusha_wallet_artifacts_v1::{
    DESCRIPTOR_MAX_BYTES_V1, VERIFYING_KEY_MAX_BYTES_V1,
    producer_inventory::{AuthenticatedProducerInventoryV1, DirectoryOriginalsV1},
};

pub(super) struct Originals {
    source: DirectoryOriginalsV1,
    records: Vec<ArtifactRecord>,
    maximum_key_bytes: usize,
}
impl Originals {
    pub(super) fn open(
        inventory: &AuthenticatedProducerInventoryV1,
        path: &Path,
        maximum_key_bytes: usize,
    ) -> Result<Self, Error> {
        Ok(Self {
            source: DirectoryOriginalsV1::open_existing(path, maximum_key_bytes)
                .map_err(|_| Error::Artifact)?,
            records: inventory.inventory().finality.originals.clone(),
            maximum_key_bytes,
        })
    }

    fn original(&self, record: &ArtifactRecord, role: usize, cap: usize) -> Result<Vec<u8>, Error> {
        let length = usize::try_from(record.lengths[role]).map_err(|_| Error::Artifact)?;
        if length == 0 || length > cap || record.sha256[role] == [0; 32] {
            return Err(Error::Artifact);
        }
        let mut reader = self
            .source
            .open_original(record.sha256[role])
            .map_err(|_| Error::Artifact)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::Artifact)?;
        reader
            .by_ref()
            .take(length as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Artifact)?;
        if bytes.len() != length || <[u8; 32]>::from(Sha256::digest(&bytes)) != record.sha256[role]
        {
            return Err(Error::Artifact);
        }
        Ok(bytes)
    }
}
impl ArtifactSource for Originals {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        let mut selected = None;
        for record in &self.records {
            if record.matches_identity(id)? {
                if selected.replace(record).is_some() {
                    return Err(Error::Artifact);
                }
            }
        }
        let record = selected.ok_or(Error::Artifact)?;
        Ok(OriginalBytes {
            descriptor: self.original(record, 0, DESCRIPTOR_MAX_BYTES_V1)?,
            verifying_key: self.original(record, 1, VERIFYING_KEY_MAX_BYTES_V1)?,
            proving_key: self.original(record, 2, self.maximum_key_bytes)?,
        })
    }
}
