//! Exact offline archive reads and authenticated bounded compiled PK regeneration.

use super::{
    Result, ServerFinalityErrorV1,
    cache::{Cache, read_original},
};
use crate::kagemusha_wallet_artifacts_v1::{DESCRIPTOR_MAX_BYTES_V1, VERIFYING_KEY_MAX_BYTES_V1};
use iroha_fs::PrivateDirectory;
use iroha_kagemusha_proof::finality::{
    catalog::{ArtifactRecord, ServerRecipes, VerifierBlobSource},
    continuity::{producer::Error, tree::OriginalBytes},
    native::{ArtifactId, ArtifactSource, ImportLimits},
};
use iroha_pasta::CancellationToken;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Cursor, Read},
    path::Path,
};

pub(super) struct VerifierOriginals {
    directory: PrivateDirectory,
    expected: BTreeMap<[u8; 32], u64>,
    cancellation: CancellationToken,
    failure: Option<ServerFinalityErrorV1>,
}
impl VerifierOriginals {
    pub(super) fn open(
        path: &Path,
        records: &[ArtifactRecord],
        cancellation: CancellationToken,
    ) -> Result<Self> {
        let mut expected = BTreeMap::new();
        for record in records {
            for (role, maximum) in [DESCRIPTOR_MAX_BYTES_V1, VERIFYING_KEY_MAX_BYTES_V1]
                .into_iter()
                .enumerate()
            {
                if record.lengths[role] == 0
                    || record.lengths[role] > maximum as u64
                    || record.sha256[role] == [0; 32]
                {
                    return Err(ServerFinalityErrorV1::Binding);
                }
                if expected
                    .insert(record.sha256[role], record.lengths[role])
                    .is_some_and(|old| old != record.lengths[role])
                {
                    return Err(ServerFinalityErrorV1::Binding);
                }
            }
        }
        Ok(Self {
            directory: PrivateDirectory::open_exact(path)?,
            expected,
            cancellation,
            failure: None,
        })
    }
    fn read(&self, hash: [u8; 32]) -> Result<Vec<u8>> {
        let length = *self
            .expected
            .get(&hash)
            .ok_or(ServerFinalityErrorV1::Binding)?;
        read_original(
            &self.directory,
            hash,
            length,
            DESCRIPTOR_MAX_BYTES_V1,
            Some(&self.cancellation),
        )?
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound).into())
    }
    pub(super) fn take_failure(&mut self) -> Option<ServerFinalityErrorV1> {
        self.failure.take()
    }
}
impl VerifierBlobSource for VerifierOriginals {
    fn open(&mut self, hash: &[u8; 32]) -> std::result::Result<Box<dyn Read + '_>, Error> {
        match self.read(*hash) {
            Ok(bytes) => Ok(Box::new(Cursor::new(bytes))),
            Err(error) => {
                let status = if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Artifact
                };
                self.failure = Some(error);
                Err(status)
            }
        }
    }
}

pub(super) struct Originals {
    source: VerifierOriginals,
    records: Vec<ArtifactRecord>,
    recipes: ServerRecipes,
    cache: Cache,
    failure: Option<ServerFinalityErrorV1>,
}
impl Originals {
    pub(super) fn new(
        source: VerifierOriginals,
        records: Vec<ArtifactRecord>,
        recipes: ServerRecipes,
        cache: Cache,
    ) -> Self {
        Self {
            source,
            records,
            recipes,
            cache,
            failure: None,
        }
    }
    pub(super) fn take_failure(&mut self) -> Option<ServerFinalityErrorV1> {
        self.failure.take()
    }
    fn original(&mut self, id: &ArtifactId) -> Result<OriginalBytes> {
        let mut selected = None;
        for record in &self.records {
            if record.matches_identity(id)? && selected.replace(record).is_some() {
                return Err(ServerFinalityErrorV1::Binding);
            }
        }
        let record = selected.ok_or(ServerFinalityErrorV1::Binding)?;
        let descriptor = self.source.read(record.sha256[0])?;
        let verifying_key = self.source.read(record.sha256[1])?;
        if let Some(proving_key) = self.cache.get(record, Some(&self.source.cancellation))? {
            return Ok(OriginalBytes {
                descriptor,
                verifying_key,
                proving_key,
            });
        }
        let original = self
            .recipes
            .regenerate(id, Some(&self.source.cancellation))?;
        if original.descriptor != descriptor || original.verifying_key != verifying_key {
            return Err(ServerFinalityErrorV1::Binding);
        }
        self.cache.put(
            record,
            &original.proving_key,
            Some(&self.source.cancellation),
        )?;
        // This remains DATA. The caller performs the exact source-specific strict import.
        Ok(original)
    }
}
impl ArtifactSource for Originals {
    fn load(&mut self, id: &ArtifactId) -> std::result::Result<OriginalBytes, Error> {
        match self.original(id) {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                let status = if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Artifact
                };
                self.failure = Some(error);
                Err(status)
            }
        }
    }
}

/// Offline byte source for complete immutable compiler archives. Serving separately
/// authenticates its selected verifier graph and uses bounded compiled regeneration.
pub(super) struct ArchiveOriginals {
    directory: PrivateDirectory,
    records: Vec<ArtifactRecord>,
    maximum_key_bytes: usize,
    visited: BTreeSet<Vec<u8>>,
    failure: Option<ServerFinalityErrorV1>,
}
impl ArchiveOriginals {
    pub(super) fn from_records(
        records: &[ArtifactRecord],
        path: &Path,
        limits: ImportLimits,
    ) -> Result<Self> {
        if !(1..=65_536).contains(&limits.maximum_artifacts)
            || !(1..=1 << 30).contains(&limits.key.maximum_bytes)
            || limits.maximum_original_bytes == 0
            || limits.maximum_original_bytes == usize::MAX
            || records.is_empty()
            || records.len() > limits.maximum_artifacts
        {
            return Err(ServerFinalityErrorV1::Binding);
        }
        let mut previous: Option<&[u8]> = None;
        let mut total = 0_usize;
        for record in records {
            record.validate_identity()?;
            if previous.is_some_and(|name| name >= record.name.as_slice()) {
                return Err(ServerFinalityErrorV1::Binding);
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
                let size = usize::try_from(record.lengths[role])
                    .map_err(|_| ServerFinalityErrorV1::Binding)?;
                if size == 0 || size > cap || record.sha256[role] == [0; 32] {
                    return Err(ServerFinalityErrorV1::Binding);
                }
                total = total
                    .checked_add(size)
                    .filter(|n| *n <= limits.maximum_original_bytes)
                    .ok_or(ServerFinalityErrorV1::Binding)?;
            }
        }
        Ok(Self {
            directory: PrivateDirectory::open_exact(path)?,
            records: records.to_vec(),
            maximum_key_bytes: limits.key.maximum_bytes,
            visited: BTreeSet::new(),
            failure: None,
        })
    }

    pub(super) fn require_complete(&self) -> Result<()> {
        if self.visited.len() != self.records.len() {
            return Err(ServerFinalityErrorV1::Binding);
        }
        Ok(())
    }

    pub(super) fn take_failure(&mut self) -> Option<ServerFinalityErrorV1> {
        self.failure.take()
    }

    fn original(&self, record: &ArtifactRecord, role: usize, cap: usize) -> Result<Vec<u8>> {
        read_original(
            &self.directory,
            record.sha256[role],
            record.lengths[role],
            cap,
            None,
        )?
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound).into())
    }

    fn load_originals(&mut self, id: &ArtifactId) -> Result<OriginalBytes> {
        let mut selected = None;
        for record in &self.records {
            if record.matches_identity(id)? && selected.replace(record).is_some() {
                return Err(ServerFinalityErrorV1::Binding);
            }
        }
        let record = selected.ok_or(ServerFinalityErrorV1::Binding)?;
        let bytes = OriginalBytes {
            descriptor: self.original(record, 0, DESCRIPTOR_MAX_BYTES_V1)?,
            verifying_key: self.original(record, 1, VERIFYING_KEY_MAX_BYTES_V1)?,
            proving_key: self.original(record, 2, self.maximum_key_bytes)?,
        };
        self.visited.insert(record.name.clone());
        Ok(bytes)
    }
}
impl ArtifactSource for ArchiveOriginals {
    fn load(&mut self, id: &ArtifactId) -> std::result::Result<OriginalBytes, Error> {
        self.load_originals(id).map_err(|error| {
            self.failure = Some(error);
            Error::Artifact
        })
    }
}
