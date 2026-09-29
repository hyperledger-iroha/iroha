//! Read-only exact lane frames under retained source-directory descriptors.

use super::*;
use std::collections::BTreeMap;

struct Captured {
    instance: [u8; 32],
    name: OsString,
    state: FileState,
    digest: [u8; 32],
}

/// Bounded source custody for original lane frames; never opens a writable LaneStore.
pub(in crate::kura::scaling_evidence::export) struct LaneFrameReader {
    root: PathBuf,
    parents: BTreeMap<[u8; 32], Parent>,
    captured: Vec<Captured>,
    maximum: usize,
}
impl LaneFrameReader {
    pub(in crate::kura::scaling_evidence::export) fn new(
        root: &Path,
        maximum: usize,
    ) -> Result<Self> {
        ensure!(
            root.is_absolute() && maximum > 0 && maximum <= 1_000_000,
            "invalid original lane source admission"
        );
        Ok(Self {
            root: root.to_owned(),
            parents: BTreeMap::new(),
            captured: Vec::new(),
            maximum,
        })
    }

    /// Acquire one exact original frame. No missing directory or file is created.
    pub(in crate::kura::scaling_evidence::export) fn read(
        &mut self,
        instance: [u8; 32],
        height: u64,
        maximum: usize,
    ) -> Result<Vec<u8>> {
        ensure!(
            height > 0
                && maximum > 0
                && maximum <= 32 * 1024 * 1024
                && self.captured.len() < self.maximum,
            "lane source frame bound exceeded"
        );
        let name = OsString::from(format!("{height:020}.frame"));
        if !self.parents.contains_key(&instance) {
            ensure!(
                self.parents.len() < 64,
                "lane source namespace bound exceeded"
            );
            let path = self
                .root
                .join("lanes")
                .join(hex::encode(instance))
                .join(&name);
            let (parent, leaf) = Parent::capture(&path, &mut |_| Ok(()))?;
            ensure!(leaf == name, "original lane source leaf changed");
            self.parents.insert(instance, parent);
        }
        let parent = &self.parents[&instance];
        parent.check()?;
        let before = named(parent.file(), &name)?;
        ensure!(
            before.owned_regular(parent.uid)
                && before.size > 0
                && before.size <= u64::try_from(maximum)?,
            "lane frame is not a bounded original regular file"
        );
        let mut file = File::from(rustix::fs::openat(
            parent.file(),
            &name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        ensure!(
            held(&file)? == before && named(parent.file(), &name)? == before,
            "lane frame changed during original acquisition"
        );
        let length = usize::try_from(before.size)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        (&mut file).take(before.size + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() == length
                && held(&file)? == before
                && named(parent.file(), &name)? == before,
            "lane frame changed during original read"
        );
        parent.check()?;
        self.captured.push(Captured {
            instance,
            name,
            state: before,
            digest: iroha_crypto::sha256(&bytes),
        });
        Ok(bytes)
    }

    /// Recheck every original namespace and exact file identity/content through final reply.
    pub(in crate::kura::scaling_evidence::export) fn recheck_sources(&self) -> Result<()> {
        for parent in self.parents.values() {
            parent.check()?;
        }
        for captured in &self.captured {
            let parent = &self.parents[&captured.instance];
            ensure!(
                named(parent.file(), &captured.name)? == captured.state,
                "original lane source changed"
            );
            let mut file = File::from(rustix::fs::openat(
                parent.file(),
                &captured.name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )?);
            ensure!(
                held(&file)? == captured.state,
                "original lane frame was replaced"
            );
            let (digest, length) = iroha_crypto::sha256_reader_bounded(
                (&mut file).take(captured.state.size + 1),
                captured.state.size,
            )?;
            ensure!(
                digest == captured.digest
                    && length == captured.state.size
                    && held(&file)? == captured.state
                    && named(parent.file(), &captured.name)? == captured.state,
                "original lane frame content changed"
            );
        }
        for parent in self.parents.values() {
            parent.check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("lanes").join(hex::encode([7; 32]));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("00000000000000000001.frame");
        std::fs::write(&file, [1, 2, 3]).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        (root, file)
    }
    #[test]
    fn original_lane_bytes_remain_bound_through_source_recheck() {
        let (root, file) = fixture();
        let mut reader = LaneFrameReader::new(&root.path().canonicalize().unwrap(), 4).unwrap();
        assert_eq!(reader.read([7; 32], 1, 3).unwrap(), vec![1, 2, 3]);
        reader.recheck_sources().unwrap();
        std::fs::write(file, [3, 2, 1]).unwrap();
        assert!(reader.recheck_sources().is_err());
    }
    #[test]
    fn missing_bounds_and_symlinked_lane_frames_fail_without_creation() {
        let root = tempfile::tempdir().unwrap();
        let mut reader = LaneFrameReader::new(&root.path().canonicalize().unwrap(), 1).unwrap();
        assert!(reader.read([7; 32], 1, 3).is_err());
        assert!(!root.path().join("lanes").exists());
        let (root, file) = fixture();
        let mut reader = LaneFrameReader::new(&root.path().canonicalize().unwrap(), 1).unwrap();
        assert!(reader.read([7; 32], 1, 2).is_err());
        let other = file.with_extension("original");
        std::fs::rename(&file, &other).unwrap();
        symlink(&other, &file).unwrap();
        assert!(reader.read([7; 32], 1, 3).is_err());
    }
    #[test]
    fn renamed_lane_namespace_invalidates_original_custody() {
        let (root, _) = fixture();
        let mut reader = LaneFrameReader::new(&root.path().canonicalize().unwrap(), 1).unwrap();
        reader.read([7; 32], 1, 3).unwrap();
        let dir = root.path().join("lanes");
        std::fs::rename(&dir, root.path().join("moved")).unwrap();
        std::fs::create_dir(&dir).unwrap();
        assert!(reader.recheck_sources().is_err());
    }
}
