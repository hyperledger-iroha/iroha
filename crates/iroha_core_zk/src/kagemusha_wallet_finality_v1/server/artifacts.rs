//! Exact server D/V/PK reads selected by the authenticated producer inventory.

use std::{collections::BTreeSet, io::Read as _, path::Path};

use iroha_kagemusha_proof::finality::{
    catalog::ArtifactRecord,
    continuity::{producer::Error, tree::OriginalBytes},
    native::{ArtifactId, ArtifactSource, ImportLimits},
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
    visited: BTreeSet<Vec<u8>>,
}
impl Originals {
    pub(super) fn open(
        inventory: &AuthenticatedProducerInventoryV1,
        path: &Path,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        Self::from_records(&inventory.inventory().finality.originals, path, limits)
    }

    // Offline qualification may inspect unadmitted records. This is only a byte
    // source: ServerFinalityV1::open authenticates its signed inventory separately.
    pub(super) fn from_records(
        records: &[ArtifactRecord],
        path: &Path,
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        if !(1..=65_536).contains(&limits.maximum_artifacts)
            || !(1..=1 << 30).contains(&limits.key.maximum_bytes)
            || limits.maximum_original_bytes == 0
            || limits.maximum_original_bytes == usize::MAX
            || records.is_empty()
            || records.len() > limits.maximum_artifacts
        {
            return Err(Error::Artifact);
        }
        let mut previous: Option<&[u8]> = None;
        let mut total = 0_usize;
        for record in records {
            record.validate_identity()?;
            if previous.is_some_and(|name| name >= record.name.as_slice()) {
                return Err(Error::Artifact);
            }
            previous = Some(&record.name);
            for (role, cap) in [
                DESCRIPTOR_MAX_BYTES_V1,
                VERIFYING_KEY_MAX_BYTES_V1,
                limits.key.maximum_bytes,
            ]
            .into_iter()
            .enumerate()
            {
                let size = usize::try_from(record.lengths[role]).map_err(|_| Error::Artifact)?;
                if size == 0 || size > cap || record.sha256[role] == [0; 32] {
                    return Err(Error::Artifact);
                }
                total = total
                    .checked_add(size)
                    .filter(|n| *n <= limits.maximum_original_bytes)
                    .ok_or(Error::Artifact)?;
            }
        }
        Ok(Self {
            source: DirectoryOriginalsV1::open_existing(
                path,
                limits.key.maximum_bytes.max(DESCRIPTOR_MAX_BYTES_V1),
            )
            .map_err(|_| Error::Artifact)?,
            records: records.to_vec(),
            maximum_key_bytes: limits.key.maximum_bytes,
            visited: BTreeSet::new(),
        })
    }

    pub(super) fn require_complete(&self) -> Result<(), Error> {
        if self.visited.len() != self.records.len() {
            return Err(Error::Artifact);
        }
        Ok(())
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
        let bytes = OriginalBytes {
            descriptor: self.original(record, 0, DESCRIPTOR_MAX_BYTES_V1)?,
            verifying_key: self.original(record, 1, VERIFYING_KEY_MAX_BYTES_V1)?,
            proving_key: self.original(record, 2, self.maximum_key_bytes)?,
        };
        self.visited.insert(record.name.clone());
        Ok(bytes)
    }
}
