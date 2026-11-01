//! Original-object cross-parent publication with exact native security custody.

use super::*;
use windows_sys::Win32::Security::{
    GROUP_SECURITY_INFORMATION, GetLengthSid, IsValidAcl, IsValidSid,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SecuritySnapshot {
    owner: Vec<u8>,
    group: Option<Vec<u8>>,
    dacl: Vec<u8>,
    control: u16,
}

impl SecuritySnapshot {
    pub(super) fn parts(&self) -> (&[u8], Option<&[u8]>, &[u8], u16) {
        (&self.owner, self.group.as_deref(), &self.dacl, self.control)
    }

    // Classify only an already captured/admitted security descriptor. Query failures
    // propagate; a failed private-policy query never selects a weaker reader policy.
    pub(super) fn current_owner_only(&self) -> io::Result<bool> {
        let user = UserSid::current()?;
        if self.owner != copy_sid(user.pointer())? {
            return Ok(false);
        }
        if self.dacl.len() < 8 {
            return Err(invalid("native ACL header is truncated"));
        }
        let count = u16::from_le_bytes([self.dacl[4], self.dacl[5]]);
        let mut offset = 8_usize;
        for _ in 0..count {
            let header = self
                .dacl
                .get(offset..offset + 4)
                .ok_or_else(|| invalid("native ACL entry header is truncated"))?;
            let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
            if length < 4 {
                return Err(invalid("native ACL entry extent is invalid"));
            }
            let ace = self
                .dacl
                .get(offset..offset + length)
                .ok_or_else(|| invalid("native ACL entry is truncated"))?;
            offset += length;
            if header[1] & 8 != 0 || header[0] == 1 {
                continue;
            }
            if header[0] != 0 || ace.len() < 12 {
                return Err(invalid("unsupported native public-input ACE"));
            }
            if ace[8..] != self.owner {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn same_principals_after_dacl_seal(&self, other: &Self) -> bool {
        // SetSecurityInfo marks its authored DACL as auto-inherited even when the
        // exact replacement remains protected. Native creator/seal controls prove
        // this single added bit; owner, group and every other control bit stay exact.
        self.owner == other.owner
            && self.group == other.group
            && other.control == self.control | windows_sys::Win32::Security::SE_DACL_AUTO_INHERITED
    }

    pub(super) fn of(file: &File) -> io::Result<Self> {
        let mut owner = null_mut();
        let mut group = null_mut();
        let mut dacl: *mut ACL = null_mut();
        let mut descriptor = null_mut();
        // SAFETY: all outputs are valid and remain inside the returned owned descriptor.
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &raw mut owner,
                &raw mut group,
                &raw mut dacl,
                null_mut(),
                &raw mut descriptor,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
        let _allocation = LocalAllocation(descriptor);
        // SAFETY: ACL belongs to the live descriptor. Validate before reading its byte length.
        if dacl.is_null() || unsafe { IsValidAcl(dacl) } == 0 {
            return Err(denied("directory move requires a valid native DACL"));
        }
        let mut control = 0;
        let mut revision = 0;
        // SAFETY: descriptor and both scalar output pointers remain valid.
        unsafe {
            win_ok(GetSecurityDescriptorControl(
                descriptor,
                &raw mut control,
                &raw mut revision,
            ))?;
        }
        // SAFETY: validated native ACL's AclSize covers its header and all ACE bytes.
        let dacl =
            unsafe { std::slice::from_raw_parts(dacl.cast::<u8>(), usize::from((*dacl).AclSize)) }
                .to_vec();
        Ok(Self {
            owner: copy_sid(owner)?,
            group: if group.is_null() {
                None
            } else {
                Some(copy_sid(group)?)
            },
            dacl,
            control,
        })
    }
}

fn copy_sid(sid: PSID) -> io::Result<Vec<u8>> {
    // SAFETY: caller provides a SID pointer inside the retained GetSecurityInfo descriptor.
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(denied(
            "directory move requires valid native security identifiers",
        ));
    }
    // SAFETY: valid SID reports its complete initialized byte length.
    let length = unsafe { GetLengthSid(sid) } as usize;
    // SAFETY: SID remains alive in its descriptor for the entire copy.
    Ok(unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), length) }.to_vec())
}

pub struct PreparedDirectoryMove {
    directory: Directory,
    before: Snapshot,
    security: SecuritySnapshot,
}

#[derive(Debug)]
pub struct MovedDirectory {
    directory: Directory,
    after: Snapshot,
    security: SecuritySnapshot,
}

impl Directory {
    pub(crate) fn revalidate_move_parent(&self) -> io::Result<()> {
        self.revalidate()
    }

    pub(crate) fn prepare_directory_move(
        self,
        destination: &Self,
    ) -> io::Result<PreparedDirectoryMove> {
        self.revalidate()?;
        destination.revalidate()?;
        if self.links.len() < 2 || Arc::strong_count(&self.links[self.links.len() - 1]) != 1 {
            return Err(denied(
                "directory move requires an unshared non-root original",
            ));
        }
        validate_current_owner(&self.current().file)?;
        let before = snapshot(&self.current().file, self.current().private, true)?;
        for ancestor in &destination.links {
            if identity(&ancestor.file)? == before.id {
                return Err(denied("directory move destination is inside the original"));
            }
        }
        if destination.identity()?.volume != before.id.volume {
            return Err(invalid("directory move requires the same filesystem"));
        }
        let security = SecuritySnapshot::of(&self.current().file)?;
        Ok(PreparedDirectoryMove {
            directory: self,
            before,
            security,
        })
    }
}

impl PreparedDirectoryMove {
    pub(crate) fn source_parent(&self) -> Directory {
        Directory {
            links: self.directory.links[..self.directory.links.len() - 1].to_vec(),
        }
    }

    fn require_original(&self, file: &File, private: bool, renamed: bool) -> io::Result<Snapshot> {
        let after = snapshot(file, private, true)?;
        let mut expected = self.before;
        if renamed {
            // A native rename may advance change time, but cannot authorize altered ACLs.
            expected.changed = after.changed;
        }
        if after != expected || SecuritySnapshot::of(file)? != self.security {
            return Err(changed());
        }
        Ok(after)
    }

    pub(crate) fn publish(
        mut self,
        destination: &Directory,
        name: &OsStr,
        published: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<MovedDirectory> {
        self.directory.revalidate()?;
        destination.revalidate()?;
        self.require_original(
            &self.directory.current().file,
            self.directory.current().private,
            false,
        )?;
        let current = self.directory.links.pop().ok_or_else(changed)?;
        let current = Arc::try_unwrap(current).map_err(|_| changed())?;
        let retained = open_file(
            &current.path,
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            OPEN_EXISTING,
            true,
            false,
        )?;
        self.require_original(&retained, current.private, false)?;
        let Link {
            sealed,
            path,
            file,
            private,
        } = current;
        // The metadata observer keeps the admitted original alive across both authority
        // transitions. Every new handle must match its exact original security and metadata.
        drop(file);
        let publishing = open_file(
            &path,
            FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            OPEN_EXISTING,
            true,
            false,
        )?;
        self.require_original(&publishing, private, false)?;
        self.directory.revalidate()?;
        destination.revalidate()?;
        let destination_path = destination.path().join(name);
        rename_handle(&publishing, &destination_path, PublishMode::CreateNew)?;
        published()?;
        let after = self.require_original(&publishing, private, true)?;
        self.require_original(&retained, private, true)?;
        self.directory.revalidate()?;
        self.directory
            .require_absent(path.file_name().ok_or_else(changed)?)?;
        drop(publishing);
        let pinned = open_file(
            &destination_path,
            FILE_READ_ATTRIBUTES | FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            OPEN_EXISTING,
            true,
            false,
        )?;
        if self.require_original(&pinned, private, true)? != after {
            return Err(changed());
        }
        let mut links = destination.links.clone();
        links.push(Arc::new(Link {
            sealed,
            path: destination_path,
            file: pinned,
            private,
        }));
        let moved = MovedDirectory {
            directory: Directory { links },
            after,
            security: self.security,
        };
        moved.revalidate()?;
        Ok(moved)
    }
}

impl MovedDirectory {
    pub(crate) fn revalidate(&self) -> io::Result<()> {
        self.directory.revalidate()?;
        let current = self.directory.current();
        if snapshot(&current.file, current.private, true)? != self.after
            || SecuritySnapshot::of(&current.file)? != self.security
        {
            return Err(changed());
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        Ok(self.after.id)
    }
    pub(crate) fn into_directory(self) -> Directory {
        self.directory
    }
}
