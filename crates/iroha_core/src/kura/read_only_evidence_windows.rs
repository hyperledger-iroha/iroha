// Windows Kura evidence uses the canonical retained native filesystem owner.
use super::{
    CanonicalKuraEvidenceError as Error, CanonicalKuraEvidenceLimits as Limits,
    CanonicalKuraEvidenceResult as Result, evidence_require as require,
};
use iroha_fs::{FileIdentity, ReaderDirectory, RetainedFile};
use std::{
    os::windows::ffi::OsStrExt as _,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub(super) struct Source {
    file: RetainedFile,
    size: u64,
}

impl Source {
    fn open(
        directory: &ReaderDirectory,
        name: &str,
        max_bytes: u64,
        hook: &mut impl FnMut(&Path),
    ) -> Result<Self> {
        directory.require_current_owner()?;
        let file = directory.open_retained_regular(name)?;
        file.require_current_owner()?;
        let size = file.snapshot()?.local_parts().2;
        require(
            size <= max_bytes,
            "bounded owned single-link regular source",
        )?;
        // The admitted original remains held across the hook. Native sharing denies
        // writes and deletion; neither a pathname reopen nor a writable handle escapes.
        hook(&directory.path().join(name));
        let source = Self { file, size };
        source.check()?;
        Ok(source)
    }

    pub(super) fn len(&self) -> u64 {
        self.size
    }

    pub(super) fn check(&self) -> Result<()> {
        self.file.require_current_owner()?;
        Ok(())
    }

    pub(super) fn check_marker_bytes(&self, expected: &[u8]) -> Result<()> {
        self.check()?;
        require(
            expected.len() <= super::MAX_BLOCK_COMMIT_MARKER_BYTES
                && expected.len() as u64 == self.len(),
            "published marker recheck bound",
        )?;
        let mut bytes = [0u8; super::MAX_BLOCK_COMMIT_MARKER_BYTES];
        let observed = &mut bytes[..expected.len()];
        iroha_fs::read_exact_at(self.file.file(), observed, 0)?;
        self.check()?;
        require(observed == expected, "published marker changed")
    }

    pub(super) fn read(&self, offset: u64, length: u64) -> Result<Vec<u8>> {
        self.read_after_check(offset, length, || {})
    }

    pub(super) fn read_after_check(
        &self,
        offset: u64,
        length: u64,
        after_check: impl FnOnce(),
    ) -> Result<Vec<u8>> {
        self.check()?;
        require(
            offset
                .checked_add(length)
                .is_some_and(|end| end <= self.len())
                && length <= 32 * 1024 * 1024,
            "bounded source read range",
        )?;
        let length = usize::try_from(length).map_err(|_| Error::Invalid("read size conversion"))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(Error::Allocator)?;
        bytes.resize(length, 0);
        after_check();
        iroha_fs::read_exact_at(self.file.file(), &mut bytes, offset)?;
        self.check()?;
        Ok(bytes)
    }
}

#[derive(Debug)]
pub(super) struct Sources {
    directory: ReaderDirectory,
    pub(super) data: Source,
    pub(super) index: Source,
    pub(super) hashes: Source,
    pub(super) marker: Source,
}

impl Sources {
    pub(super) fn ensure_publication_ancestry(&self, ancestry: &[FileIdentity]) -> Result<()> {
        self.check()?;
        let parent = self.directory.snapshot()?.local_parts().1;
        require(
            !ancestry.iter().any(|identity| {
                let (host, volume, object) = identity.local_parts();
                host == 2 && (volume, object) == parent
            }),
            "publication inside protected Core source namespace",
        )?;
        self.check()
    }

    pub(super) fn open(store: &Path, limits: Limits, mut hook: impl FnMut(&Path)) -> Result<Self> {
        require(
            store.is_absolute() && store.as_os_str().encode_wide().count() <= 4096,
            "absolute bounded source path",
        )?;
        let mut normalized = PathBuf::new();
        let mut components = 0;
        for component in store.components() {
            match component {
                Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                    normalized.push(component.as_os_str());
                    components += 1;
                }
                _ => return Err(Error::Invalid("normalized source path")),
            }
        }
        require(
            normalized.as_os_str() == store.as_os_str() && components <= 63,
            "bounded source components",
        )?;
        let directory = ReaderDirectory::open(store)?;
        directory.require_current_owner()?;
        let data = Source::open(
            &directory,
            super::DATA_FILE_NAME,
            limits.max_store_data_bytes,
            &mut hook,
        )?;
        let index = Source::open(
            &directory,
            super::INDEX_FILE_NAME,
            limits.max_committed_blocks * super::BlockIndex::SIZE,
            &mut hook,
        )?;
        let hashes = Source::open(
            &directory,
            super::HASHES_FILE_NAME,
            limits.max_committed_blocks * super::SIZE_OF_BLOCK_HASH,
            &mut hook,
        )?;
        let marker = Source::open(
            &directory,
            super::COUNT_FILE_NAME,
            super::MAX_BLOCK_COMMIT_MARKER_BYTES as u64,
            &mut hook,
        )?;
        let sources = Self {
            directory,
            data,
            index,
            hashes,
            marker,
        };
        let mut identities = std::collections::BTreeSet::new();
        for source in [
            &sources.data,
            &sources.index,
            &sources.hashes,
            &sources.marker,
        ] {
            require(
                identities.insert(source.file.identity()?.local_parts()),
                "aliased evidence sources",
            )?;
        }
        sources.check()?;
        Ok(sources)
    }

    pub(super) fn check(&self) -> Result<()> {
        self.directory.require_current_owner()?;
        for source in [&self.data, &self.index, &self.hashes, &self.marker] {
            source.check()?;
        }
        self.directory.require_current_owner()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateReadOnly as _;
    use iroha_fs::{PrivateDirectory, PublishMode};

    fn fixture() -> (tempfile::TempDir, PrivateDirectory, Limits) {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("store")).unwrap();
        for name in [
            super::super::DATA_FILE_NAME,
            super::super::INDEX_FILE_NAME,
            super::super::HASHES_FILE_NAME,
            super::super::COUNT_FILE_NAME,
        ] {
            directory
                .write_atomic(name, b"original", PublishMode::CreateNew)
                .unwrap();
        }
        let limits = Limits {
            first_height: 1,
            last_height: 1,
            max_committed_blocks: 1,
            max_store_data_bytes: 1024,
            max_carrier_bytes: 1024,
            max_output_bytes: 1024,
            max_decode_allocation_bytes: 4096,
        };
        (temporary, directory, limits)
    }

    #[test]
    fn retained_windows_sources_read_exact_ranges_and_marker_bytes() {
        let (_temporary, directory, limits) = fixture();
        let mut admitted = Vec::new();
        let sources = Sources::open(directory.path(), limits, |path| {
            admitted.push(path.to_owned())
        })
        .unwrap();
        assert_eq!(admitted.len(), 4);
        assert_eq!(sources.data.len(), 8);
        assert_eq!(sources.data.read(1, 3).unwrap(), b"rig");
        assert!(sources.data.read(8, 1).is_err());
        assert!(sources.data.read(u64::MAX, 2).is_err());
        sources.marker.check_marker_bytes(b"original").unwrap();
        assert!(sources.marker.check_marker_bytes(b"modified").is_err());
        assert!(sources.marker.check_marker_bytes(b"short").is_err());
        sources.check().unwrap();
    }

    #[test]
    fn retained_windows_sources_deny_writes_and_replacement_through_last_check() {
        let (_temporary, directory, limits) = fixture();
        let sources = Sources::open(directory.path(), limits, |_| {}).unwrap();
        let data = directory.path().join(super::super::DATA_FILE_NAME);
        let original = std::fs::read(&data).unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&data).is_err());
        assert!(std::fs::remove_file(&data).is_err());
        assert!(std::fs::rename(&data, directory.path().join("replaced")).is_err());
        let bytes = sources
            .data
            .read_after_check(0, 8, || {
                assert!(std::fs::write(&data, b"modified").is_err());
            })
            .unwrap();
        assert_eq!(bytes, original);
        sources.check().unwrap();
        drop(sources);
        std::fs::write(data, b"modified").unwrap();
    }

    #[test]
    fn retained_windows_sources_refuse_links_absence_bounds_and_path_aliases() {
        let (_temporary, directory, limits) = fixture();
        let mut too_small = limits;
        too_small.max_store_data_bytes = 7;
        assert!(Sources::open(directory.path(), too_small, |_| {}).is_err());
        for path in [
            PathBuf::from("relative"),
            directory.path().join("."),
            directory.path().join(".."),
            directory.path().join("missing"),
        ] {
            assert!(Sources::open(&path, limits, |_| {}).is_err());
        }
        let original = directory.path().join(super::super::DATA_FILE_NAME);
        std::fs::hard_link(&original, directory.path().join("alias")).unwrap();
        assert!(Sources::open(directory.path(), limits, |_| {}).is_err());
        std::fs::remove_file(directory.path().join("alias")).unwrap();
        Sources::open(directory.path(), limits, |_| {}).unwrap();
    }

    #[test]
    fn retained_windows_source_namespace_uses_full_native_identity() {
        let (_temporary, directory, limits) = fixture();
        let sources = Sources::open(directory.path(), limits, |_| {}).unwrap();
        assert!(
            sources
                .ensure_publication_ancestry(&[directory.identity().unwrap()])
                .is_err()
        );
        let outside =
            PrivateDirectory::open_or_create(directory.path().parent().unwrap().join("outside"))
                .unwrap();
        sources
            .ensure_publication_ancestry(&[outside.identity().unwrap()])
            .unwrap();
    }

    #[test]
    fn windows_reader_preserves_native_carriers_and_retains_sources_through_finish() {
        use crate::{
            kura::{BlockIndex, BlockStoreCommitMarker, CanonicalKuraEvidenceReader},
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        use std::num::NonZeroUsize;

        let (_temporary, directory, mut limits) = fixture();
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::default(), 1))
                .unwrap();
        chain.commit(Vec::new());
        let blocks = (1..=2)
            .map(|height| {
                chain
                    .kura()
                    .get_block(
                        NonZeroUsize::new(height).unwrap(),
                        &chain.state().view().execution_budget(),
                    )
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut data = Vec::new();
        let mut index = Vec::new();
        let mut hashes = Vec::new();
        for block in &blocks {
            let wire = block.encode_wire().unwrap();
            index.extend_from_slice(
                &BlockIndex {
                    start: data.len() as u64,
                    length: wire.len() as u64,
                }
                .encode(),
            );
            hashes.extend_from_slice(block.hash().as_ref());
            data.extend_from_slice(&wire);
        }
        let marker =
            norito::encode_canonical(&BlockStoreCommitMarker::new(2, Some(blocks[1].hash())))
                .unwrap();
        for (name, bytes) in [
            (super::super::DATA_FILE_NAME, data.as_slice()),
            (super::super::INDEX_FILE_NAME, index.as_slice()),
            (super::super::HASHES_FILE_NAME, hashes.as_slice()),
            (super::super::COUNT_FILE_NAME, marker.as_slice()),
        ] {
            directory
                .write_atomic(name, bytes, PublishMode::Replace)
                .unwrap();
        }
        limits.last_height = 2;
        limits.max_committed_blocks = 2;
        limits.max_store_data_bytes = data.len() as u64;
        limits.max_carrier_bytes = data.len();
        limits.max_output_bytes = data.len() as u64;
        limits.max_decode_allocation_bytes = 64 * 1024 * 1024;
        let mut reader = CanonicalKuraEvidenceReader::open(directory.path(), limits).unwrap();
        for (position, block) in blocks.iter().enumerate() {
            assert_eq!(
                reader.read_carrier(position as u64 + 1).unwrap(),
                block.encode_wire().unwrap()
            );
        }
        let complete = reader.finish().unwrap();
        assert_eq!(complete.committed_height(), 2);
        assert_eq!(complete.carrier_count(), 2);
        assert_eq!(complete.output_bytes(), data.len() as u64);
        complete.recheck_sources().unwrap();
        assert!(
            complete
                .ensure_publication_ancestry(&[directory.identity().unwrap()])
                .is_err()
        );
        assert!(complete.ensure_publication_ancestry(&[]).is_err());
        assert!(std::fs::remove_file(directory.path().join(super::super::DATA_FILE_NAME)).is_err());
        drop(complete);
        std::fs::remove_file(directory.path().join(super::super::DATA_FILE_NAME)).unwrap();
    }
}
