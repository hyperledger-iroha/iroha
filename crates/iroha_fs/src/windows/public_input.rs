//! Read-only public input custody with exact security and private-ancestor snapshots.

use super::*;
use directory_move::SecuritySnapshot;

/// Exact Windows public-input metadata and native security descriptor components.
///
/// These values describe one local original, not portable or permanent physical identity.
/// Protocol consumers supply their own canonical encoding and independently approved digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicInputSnapshot {
    metadata: Snapshot,
    security: SecuritySnapshot,
}

impl PublicInputSnapshot {
    /// Native Windows metadata in the documented [`crate::LocalFileSnapshot`] projection.
    #[must_use]
    pub fn metadata_parts(&self) -> crate::LocalFileSnapshot {
        FileSnapshot(self.metadata).local_parts()
    }

    /// Exact binary owner SID, optional primary-group SID, DACL and descriptor-control flags.
    ///
    /// These are native security data, not a second serialization codec. Callers encode the
    /// typed fields with their protocol codec; no pointer or writable capability is exposed.
    #[must_use]
    pub fn security_parts(&self) -> (&[u8], Option<&[u8]>, &[u8], u16) {
        self.security.parts()
    }
}

#[derive(Debug)]
struct ParentSnapshot {
    metadata: Snapshot,
    security: SecuritySnapshot,
    owner_only: bool,
}

/// A bounded public verification input retaining its original and every native ancestor.
///
/// The current-user single-link regular file excludes other data writers and deletion.
/// Every ancestor retains exact security/identity/attributes; owner-only ancestors also
/// retain full metadata. Unrelated namespace timestamps of shared ancestors may advance.
/// No creation, canonicalization, ACL repair or raw descriptor access is provided.
#[derive(Debug)]
pub struct StrictPublicInput {
    original: RetainedFile,
    directory: Directory,
    parents: Vec<ParentSnapshot>,
    before: PublicInputSnapshot,
}

fn exact_path(path: &Path) -> io::Result<()> {
    let text = path
        .to_str()
        .ok_or_else(|| invalid("public input path is not UTF-8"))?;
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(invalid(
            "public input requires an absolute local drive path",
        ));
    };
    if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        || !matches!(components.next(), Some(Component::RootDir))
    {
        return Err(invalid(
            "public input requires an absolute local drive path",
        ));
    }
    let prefix_len = prefix
        .as_os_str()
        .to_str()
        .ok_or_else(|| invalid("public input prefix is not UTF-8"))?
        .len();
    let tail = &text[prefix_len..];
    let mut spellings = tail.split(['\\', '/']);
    if spellings.next() != Some("")
        || spellings.any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid("public input requires an exact nontraversing path"));
    }
    let mut count = 0;
    for part in components {
        let Component::Normal(name) = part else {
            return Err(invalid("public input requires normal path components"));
        };
        checked_name(name)?;
        count += 1;
    }
    if count == 0 {
        return Err(invalid("public input has no file name"));
    }
    Ok(())
}

fn observe_file(original: &RetainedFile) -> io::Result<PublicInputSnapshot> {
    original.revalidate()?;
    validate_current_owner(original.file())?;
    let metadata = snapshot(original.file(), false, false)?;
    let security = SecuritySnapshot::of(original.file())?;
    if snapshot(original.file(), false, false)? != metadata
        || SecuritySnapshot::of(original.file())? != security
    {
        return Err(changed());
    }
    original.revalidate()?;
    Ok(PublicInputSnapshot { metadata, security })
}

fn check_parents(directory: &Directory, parents: &[ParentSnapshot]) -> io::Result<()> {
    let mut result = directory.revalidate();
    for (link, before) in directory.links.iter().zip(parents) {
        let check = (|| {
            let after = snapshot(&link.file, false, true)?;
            if SecuritySnapshot::of(&link.file)? != before.security
                || if before.owner_only {
                    after != before.metadata
                } else {
                    after.id != before.metadata.id || after.attributes != before.metadata.attributes
                }
            {
                return Err(changed());
            }
            Ok(())
        })();
        result = result.and(check);
    }
    let final_check = directory.revalidate();
    result.and(final_check)
}

impl StrictPublicInput {
    /// Validate the caller's exact absolute local-drive spelling without filesystem I/O.
    /// # Errors
    /// Refuses nonlocal prefixes, traversal, aliases and invalid native member spellings.
    pub fn validate_path(path: impl AsRef<Path>) -> io::Result<()> {
        exact_path(path.as_ref())
    }

    /// Admit one exact absolute native path without following reparses or creating objects.
    ///
    /// # Errors
    /// Refuses ambiguous/nonlocal paths, unsafe custody, multiple links, native query errors
    /// and any identity/security/private-ancestor change during admission.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        exact_path(path)?;
        let name = checked_name(
            path.file_name()
                .ok_or_else(|| invalid("public input has no leaf"))?,
        )?;
        let directory = Directory::open_reader(
            path.parent()
                .ok_or_else(|| invalid("public input has no parent"))?,
        )?;
        let mut parents = Vec::new();
        let mut original = None;
        let mut before = None;
        let result = (|| {
            for link in &directory.links {
                let metadata = snapshot(&link.file, false, true)?;
                let security = SecuritySnapshot::of(&link.file)?;
                let owner_only = security.current_owner_only()?;
                parents.push(ParentSnapshot {
                    metadata,
                    security,
                    owner_only,
                });
            }
            original = Some(directory.open_retained(name, false, false)?);
            before = Some(observe_file(original.as_ref().ok_or_else(changed)?)?);
            Ok(())
        })();
        let original_check = original.as_ref().map_or(Ok(()), |original| {
            let actual = observe_file(original)?;
            if before.as_ref().is_some_and(|expected| *expected != actual) {
                return Err(changed());
            }
            Ok(())
        });
        let parent_check = check_parents(&directory, &parents);
        result.and(original_check).and(parent_check)?;
        Ok(Self {
            original: original.ok_or_else(changed)?,
            directory,
            parents,
            before: before.ok_or_else(changed)?,
        })
    }

    /// Revalidate every original, exact native security and private-ancestor metadata.
    /// # Errors
    /// Returns the first genuine custody failure after independently checking all originals.
    pub fn revalidate(&self) -> io::Result<()> {
        let original = observe_file(&self.original).and_then(|after| {
            if after == self.before {
                Ok(())
            } else {
                Err(changed())
            }
        });
        let parents = check_parents(&self.directory, &self.parents);
        original.and(parents)
    }

    /// Return the original's exact native metadata after fresh full custody checks.
    /// # Errors
    /// Refuses changed originals or native observations; no current pathname becomes authority.
    pub fn snapshot(&self) -> io::Result<PublicInputSnapshot> {
        self.revalidate()?;
        Ok(self.before.clone())
    }

    /// Read the complete original within the caller's bound, checking exact extent and EOF.
    /// # Errors
    /// Refuses oversized inputs, allocation/I/O failures or final custody/metadata drift.
    pub fn read(&self, maximum: usize) -> io::Result<Vec<u8>> {
        let result = (|| {
            self.revalidate()?;
            let length = usize::try_from(self.before.metadata.length)
                .map_err(|_| invalid("public input extent does not fit"))?;
            if length > maximum {
                return Err(invalid("public input exceeds its byte bound"));
            }
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(io::Error::other)?;
            bytes.resize(length, 0);
            crate::read_exact_at(self.original.file(), &mut bytes, 0)?;
            let mut eof = [0_u8; 1];
            if crate::read_at(self.original.file(), &mut eof, self.before.metadata.length)? != 0 {
                return Err(changed());
            }
            Ok(bytes)
        })();
        let final_check = self.revalidate();
        result.and_then(|bytes| final_check.map(|()| bytes))
    }
}

#[cfg(test)]
mod tests;
