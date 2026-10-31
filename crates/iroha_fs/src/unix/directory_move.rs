//! Descriptor-relative cross-parent directory publication.

use super::*;

pub struct PreparedDirectoryMove {
    directory: Directory,
    before: FileSnapshot,
}

#[derive(Debug)]
pub struct MovedDirectory {
    directory: Directory,
    after: FileSnapshot,
}

impl Directory {
    pub(crate) fn revalidate_move_parent(&self) -> io::Result<()> {
        custody_transaction::revalidate_name(self)
    }

    pub(crate) fn prepare_directory_move(
        self,
        destination: &Self,
    ) -> io::Result<PreparedDirectoryMove> {
        custody_transaction::revalidate_name(&self)?;
        custody_transaction::revalidate_name(destination)?;
        if self.links.len() < 2 || Arc::strong_count(&self.links[self.links.len() - 1]) != 1 {
            return Err(denied(
                "directory move requires an unshared non-root original",
            ));
        }
        let before = self.snapshot_directory()?;
        if before.owner != rustix::process::geteuid().as_raw() {
            return Err(denied("directory move requires current ownership"));
        }
        if destination
            .links
            .iter()
            .any(|link| link.identity == before.identity)
        {
            return Err(denied("directory move destination is inside the original"));
        }
        if destination.identity()?.volume != before.identity.volume {
            return Err(invalid("directory move requires the same filesystem"));
        }
        Ok(PreparedDirectoryMove {
            directory: self,
            before,
        })
    }
}

impl PreparedDirectoryMove {
    pub(crate) fn source_parent(&self) -> Directory {
        Directory {
            links: self.directory.links[..self.directory.links.len() - 1].to_vec(),
        }
    }

    pub(crate) fn publish(
        mut self,
        destination: &Directory,
        name: &OsStr,
        published: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<MovedDirectory> {
        custody_transaction::revalidate_name(&self.directory)?;
        custody_transaction::revalidate_name(destination)?;
        if self.directory.snapshot_directory()? != self.before {
            return Err(changed());
        }
        let current = self.directory.links.pop().ok_or_else(changed)?;
        let mut current = Arc::try_unwrap(current).map_err(|_| changed())?;
        let parent = self.directory.current();
        let source_name = current.path.file_name().ok_or_else(changed)?;
        // Directory moves cannot use link/unlink emulation: publication is one native call.
        file_move::rename_no_replace(&parent.file, source_name, &destination.current().file, name)?;
        published()?;
        let after = FileSnapshot::from_metadata(&current.file.metadata()?);
        self.before.changed = after.changed;
        if after != self.before {
            return Err(changed());
        }
        self.directory.revalidate()?;
        self.directory.require_absent(source_name)?;
        current.path = destination.path().join(name);
        let mut links = destination.links.clone();
        links.push(Arc::new(current));
        let moved = MovedDirectory {
            directory: Directory { links },
            after,
        };
        moved.revalidate()?;
        Ok(moved)
    }
}

impl MovedDirectory {
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        custody_transaction::revalidate_name(&self.directory)?;
        if self.directory.snapshot_directory()? != self.after {
            return Err(changed());
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        Ok(self.after.identity)
    }
    pub(crate) fn into_directory(self) -> Directory {
        self.directory
    }
}
