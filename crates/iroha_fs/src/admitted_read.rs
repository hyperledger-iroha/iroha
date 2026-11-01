//! One original native read algorithm, with explicit caller-owned buffer cleanup.

use super::{FileSnapshot, PrivateReadScope, checked_name};
use std::{
    ffi::OsStr,
    io::{self, Read as _},
};
use zeroize::Zeroizing;

// Sealed by module privacy: callers cannot add a buffer policy or change native checks.
trait AdmittedReadBuffer: AsRef<[u8]> + AsMut<[u8]> {
    type Tail: AsMut<[u8]>;
    fn tail() -> Self::Tail;
}

impl AdmittedReadBuffer for Vec<u8> {
    type Tail = [u8; 1];
    fn tail() -> Self::Tail {
        [0; 1]
    }
}

impl AdmittedReadBuffer for Zeroizing<Vec<u8>> {
    type Tail = Zeroizing<[u8; 1]>;
    fn tail() -> Self::Tail {
        Zeroizing::new([0; 1])
    }
}

impl PrivateReadScope<'_> {
    /// Read one admitted record into the caller's original owned allocation.
    ///
    /// The admission tuple converts the fresh length then allocates exactly that many bytes;
    /// both retain caller error types and allocation charging. The maximum independently bounds
    /// the admitted length before allocation. `mid` runs after exact read/EOF/snapshot checks and
    /// before the final named snapshot. The error tuple maps only initial-open errors and constructs
    /// original changed-read (`false`) or changed-namespace (`true`) errors. These factories cannot
    /// accept a failed native check. Only the first raw native open can yield `None`.
    ///
    /// This method returns the same single `Vec`, with no copy or zeroization change. The caller
    /// must close per-record ownership before decoding these bytes. Enclosing directory scope
    /// closes every ordinary result. No file, pathname, descriptor or writer is returned; callers
    /// keep batches read-only, without signing, publication, network effects or a validation cache.
    ///
    /// # Errors
    /// Preserves admission, allocation, original native I/O and caller custody errors. Refuses an
    /// inadmissible length/buffer, truncation, extension, changed snapshots or changed named objects.
    pub fn read_admitted<E>(
        &mut self,
        name: impl AsRef<OsStr>,
        maximum: usize,
        admission: (
            impl FnOnce(u64) -> Result<usize, E>,
            impl FnOnce(usize) -> Result<Vec<u8>, E>,
        ),
        mid: impl FnOnce(&Self) -> Result<(), E>,
        errors: (impl FnOnce(io::Error) -> E, impl FnOnce(bool) -> E),
    ) -> Result<Option<Vec<u8>>, E>
    where
        E: From<io::Error>,
    {
        self.read_admitted_owned(name, maximum, admission, mid, errors)
    }

    /// Read one admitted record into its original zeroizing allocation.
    ///
    /// Uses the same extent admission, exact read/EOF, original-file snapshot and final named
    /// snapshot as [`Self::read_admitted`]. The caller allocates a `Zeroizing<Vec<u8>>` once;
    /// every failure after allocation drops that owner and wipes its buffer. The EOF scratch
    /// byte is zeroized as well. A successful owner remains zeroizing while the enclosing
    /// directory scope checks custody, including when an exit refusal drops its body result.
    ///
    /// Move successful bytes to a plain `Vec` only after the enclosing read scope returns,
    /// using `std::mem::take(&mut *bytes)` when that is the caller's explicit output policy.
    /// Taking or copying bytes inside the scope would surrender this clearing guarantee.
    /// Initial absence alone returns `None`; all admission and allocation errors retain `E`.
    ///
    /// # Errors
    /// Refuses the same invalid names, extents, read/EOF errors and custody changes as
    /// [`Self::read_admitted`], preserving the caller's original typed error factories.
    pub fn read_admitted_zeroizing<E>(
        &mut self,
        name: impl AsRef<OsStr>,
        maximum: usize,
        admission: (
            impl FnOnce(u64) -> Result<usize, E>,
            impl FnOnce(usize) -> Result<Zeroizing<Vec<u8>>, E>,
        ),
        mid: impl FnOnce(&Self) -> Result<(), E>,
        errors: (impl FnOnce(io::Error) -> E, impl FnOnce(bool) -> E),
    ) -> Result<Option<Zeroizing<Vec<u8>>>, E>
    where
        E: From<io::Error>,
    {
        self.read_admitted_owned(name, maximum, admission, mid, errors)
    }

    fn read_admitted_owned<E, B>(
        &mut self,
        name: impl AsRef<OsStr>,
        maximum: usize,
        admission: (
            impl FnOnce(u64) -> Result<usize, E>,
            impl FnOnce(usize) -> Result<B, E>,
        ),
        mid: impl FnOnce(&Self) -> Result<(), E>,
        errors: (impl FnOnce(io::Error) -> E, impl FnOnce(bool) -> E),
    ) -> Result<Option<B>, E>
    where
        E: From<io::Error>,
        B: AdmittedReadBuffer,
    {
        let (initial_error, changed_error) = errors;
        let mut file = match checked_name(name.as_ref())
            .and_then(|name| self.directory.open_readonly_native(name, true))
            .map_err(initial_error)?
        {
            Some(file) => file,
            None => return Ok(None),
        };
        let before = FileSnapshot::of(&file, true)?;
        let extent = file.metadata()?.len();
        let (admit, allocate) = admission;
        let length = admit(extent)?;
        if length > maximum || u64::try_from(length).ok() != Some(extent) {
            return Err(
                io::Error::other("record length admission differs from its native extent").into(),
            );
        }
        let mut bytes = allocate(length)?;
        if bytes.as_ref().len() != length {
            return Err(
                io::Error::other("record allocation differs from its admitted length").into(),
            );
        }
        file.read_exact(bytes.as_mut())?;
        let mut tail = B::tail();
        if !(file.read(tail.as_mut())? == 0 && FileSnapshot::of(&file, true)? == before) {
            return Err(changed_error(false));
        }
        mid(self)?;
        let current = self
            .directory
            .open_readonly_native(checked_name(name.as_ref())?, false)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        if FileSnapshot::of(&current, true)? != before {
            return Err(changed_error(true));
        }
        Ok(Some(bytes))
    }
}
