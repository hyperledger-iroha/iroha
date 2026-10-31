//! Exact native mutable-output admission before intentional truncation.

use super::*;

impl Directory {
    pub(crate) fn open_output(&self, name: &OsStr) -> io::Result<RetainedFile> {
        self.open_output_observed(name, |_| Ok(()))
    }

    pub(crate) fn open_output_observed(
        &self,
        name: &OsStr,
        before_truncate: impl FnOnce(&RetainedFile) -> io::Result<()>,
    ) -> io::Result<RetainedFile> {
        self.revalidate()?;
        let result = (|| {
            let name = checked_name(name)?.to_owned();
            let path = self.path().join(&name);
            let (file, created) = match open_file(
                &path,
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ,
                CREATE_NEW,
                false,
                true,
            ) {
                Ok(file) => (file, true),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (
                    open_file(
                        &path,
                        GENERIC_READ | GENERIC_WRITE,
                        FILE_SHARE_READ,
                        OPEN_EXISTING,
                        false,
                        false,
                    )?,
                    false,
                ),
                Err(error) => return Err(error),
            };
            // No truncating creation disposition is used. Both handles must
            // pass the original regular-file and current-owner policy first.
            let before = snapshot(&file, created, false)?;
            validate_current_owner(&file)?;
            let original = RetainedFile {
                directory: self.clone(),
                name,
                file,
                before,
                links: LinkPolicy::Single,
                private: created,
                writable: true,
                read_only: false,
                executable: false,
                publishable: false,
                ancestry: AncestryCheck::EachCall,
            };
            original.require_current_owner()?;
            let observed = before_truncate(&original);
            let custody = original.require_current_owner();
            observed.and(custody)?;
            let truncated = original.file.set_len(0);
            let custody = original.require_current_owner();
            truncated.and(custody)?;
            Ok(original)
        })();
        let parent_custody = self.revalidate();
        result.and_then(|original| parent_custody.map(|()| original))
    }
}
