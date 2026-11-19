//! Retire immutable snapshot aliases only when every link is inside the stopped snapshot tree.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Default)]
pub(super) struct SnapshotLinks {
    // Native volume and object identify a single inode or file record across directory entries.
    directories: BTreeSet<(u64, [u8; 16])>,
    counts: BTreeMap<(u64, [u8; 16]), (u64, u64)>,
}

pub(super) enum Removal<'a> {
    SingleLink,
    Inventory(&'a mut SnapshotLinks),
    Retire(&'a mut SnapshotLinks),
}

impl Removal<'_> {
    pub(super) fn inspecting(&self) -> bool {
        matches!(self, Self::Inventory(_))
    }

    pub(super) fn visit_directory(&mut self, id: FileIdentity) -> io::Result<()> {
        let key = (id.volume, id.object);
        let expected = match self {
            Self::SingleLink => true,
            Self::Inventory(inventory) => inventory.directories.insert(key),
            Self::Retire(inventory) => inventory.directories.remove(&key),
        };
        if expected { Ok(()) } else { Err(changed()) }
    }

    pub(super) fn admit_file(&mut self, id: FileIdentity, links: u64) -> io::Result<()> {
        let key = (id.volume, id.object);
        match self {
            Self::SingleLink if links == 1 => Ok(()),
            Self::SingleLink => Err(denied("private removal requires exactly one link")),
            Self::Inventory(inventory) => {
                if links > 1 {
                    let (original, seen) = inventory.counts.entry(key).or_insert((links, 0));
                    if *original != links || *seen >= links {
                        return Err(changed());
                    }
                    *seen += 1;
                }
                Ok(())
            }
            Self::Retire(inventory) => {
                if let Some((_, remaining)) = inventory.counts.get_mut(&key) {
                    if *remaining != links || links == 0 {
                        return Err(changed());
                    }
                    *remaining -= 1;
                } else if links != 1 {
                    return Err(changed());
                }
                Ok(())
            }
        }
    }
}

impl PrivateDirectory {
    /// Clear one stopped snapshot subtree whose immutable files may have several local names.
    ///
    /// The caller holds the exclusive operation and runtime locks. Every alias of a shared
    /// file must be inside the named subtree. The first pass verifies this before unlinking
    /// anything. The second checks each remaining native link count at deletion. The ordinary
    /// private tree removal policy still applies everywhere else. A missing subtree is empty.
    ///
    /// # Errors
    /// Refuses empty or unsafe paths, external links, changed identities, oversized trees and
    /// native failures. A failure during removal can leave a partially cleared subtree.
    pub fn clear_snapshot_subtree(&self, components: &[&str]) -> io::Result<()> {
        if components.is_empty() {
            return Err(invalid("snapshot removal requires a named subtree"));
        }
        let mut directory = self.inner.clone();
        for component in components {
            let Some(child) =
                directory.child_owned_optional(checked_name(OsStr::new(component))?)?
            else {
                return Ok(());
            };
            directory = child;
        }
        let mut inventory = SnapshotLinks::default();
        directory.walk_snapshot(&mut Removal::Inventory(&mut inventory))?;
        if inventory.counts.values().any(|(links, seen)| links != seen) {
            return Err(denied(
                "snapshot file has links outside the stopped subtree",
            ));
        }
        directory.walk_snapshot(&mut Removal::Retire(&mut inventory))?;
        if !inventory.directories.is_empty()
            || inventory
                .counts
                .values()
                .any(|(_, remaining)| *remaining != 0)
        {
            return Err(changed());
        }
        directory.revalidate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn only_complete_local_snapshot_link_groups_can_be_retired() {
        let root = tempfile::tempdir().unwrap();
        let owner = PrivateDirectory::open_or_create(root.path().join("context")).unwrap();
        let snapshot = owner.create_child("snapshots").unwrap();
        let first = snapshot.create_child("first").unwrap();
        let second = snapshot.create_child("second").unwrap();
        let original = first.path().join("chunk");
        let alias = second.path().join("reused");
        let external = owner.path().join("external");
        fs::write(&original, b"immutable").unwrap();
        fs::hard_link(&original, &alias).unwrap();
        fs::hard_link(&original, &external).unwrap();
        drop((first, second));
        assert!(owner.clear_snapshot_subtree(&["snapshots"]).is_err());
        for path in [&original, &alias, &external] {
            assert_eq!(fs::read(path).unwrap(), b"immutable");
        }
        fs::remove_file(&external).unwrap();
        owner.clear_snapshot_subtree(&["snapshots"]).unwrap();
        assert!(snapshot.entries(1).unwrap().is_empty());
        assert!(owner.clear_snapshot_subtree(&[]).is_err());
        assert!(owner.clear_snapshot_subtree(&["absent"]).is_ok());
    }
}
