//! Private no-sharing archive lock originals; ordinary lock sharing is unchanged.

use super::*;

impl Directory {
    pub(crate) fn open_exclusive_private_lock(&self, name: &OsStr) -> io::Result<File> {
        self.revalidate()?;
        let original = open_file(
            &self.path().join(name),
            GENERIC_READ | GENERIC_WRITE,
            0,
            OPEN_ALWAYS,
            false,
            true,
        )?;
        // Keep the created/opened original through every fallible validation and barrier.
        let mut admitted = None;
        let mut named = None;
        let result = (|| {
            let before = snapshot(&original, true, false)?;
            admitted = Some(before);
            named = Some(open_file(
                &self.path().join(name),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                OPEN_EXISTING,
                false,
                false,
            )?);
            let named = named.as_ref().ok_or_else(changed)?;
            if snapshot(named, true, false)? != before {
                return Err(changed());
            }
            original.sync_all()?;
            self.sync()?;
            if snapshot(&original, true, false)? != before
                || snapshot(named, true, false)? != before
            {
                return Err(changed());
            }
            Ok(())
        })();
        let original_check = snapshot(&original, true, false).and_then(|after| {
            if admitted.is_some_and(|before| before != after) {
                Err(changed())
            } else {
                Ok(())
            }
        });
        let named_check = named.as_ref().map_or(Ok(()), |named| {
            snapshot(named, true, false).and_then(|after| {
                if admitted.is_some_and(|before| before != after) {
                    Err(changed())
                } else {
                    Ok(())
                }
            })
        });
        let final_check = self.revalidate();
        result
            .and(original_check)
            .and(named_check)
            .and(final_check)?;
        Ok(original)
    }
}
