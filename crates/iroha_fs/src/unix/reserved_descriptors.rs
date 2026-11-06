//! Parent-owned slots for fixed protocol descriptors in native children.

use std::{
    io,
    os::{
        fd::{AsRawFd as _, BorrowedFd, OwnedFd, RawFd},
        unix::process::CommandExt as _,
    },
    process::Command,
    sync::{Arc, Condvar, Mutex, MutexGuard, Weak},
};

use rustix::{fs, io as fd};

/// Exclusive parent custody of fixed child descriptor numbers. Only inert,
/// read-only close-on-exec placeholders occupy these slots in the parent.
/// Concurrent commands retain this owner through fork and independently install
/// their own inputs in their own children. Dropping the last owner frees the slots.
#[derive(Debug)]
pub struct ReservedProcessDescriptors {
    slots: Vec<OwnedFd>,
    shared: Option<&'static SharedProcessDescriptorReservations>,
}

impl ReservedProcessDescriptors {
    /// Reserve a nonempty set of distinct, currently unused numbers in `3..=4095`.
    ///
    /// # Errors
    /// Invalid sets, resource exhaustion and occupied destinations fail before
    /// custody is returned. An occupied slot is never closed or replaced. Any
    /// partially acquired reservations are released on error.
    pub fn new(destinations: &[RawFd]) -> io::Result<Self> {
        if destinations.is_empty() || destinations.len() > 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid reserved descriptor count",
            ));
        }
        for (i, target) in destinations.iter().enumerate() {
            if !(3..=4095).contains(target) || destinations[..i].contains(target) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid reserved descriptor destination",
                ));
            }
        }
        let floor = 1 + *destinations.iter().max().expect("nonempty reservation");
        let null = fs::open(
            "/dev/null",
            fs::OFlags::RDONLY | fs::OFlags::CLOEXEC,
            fs::Mode::empty(),
        )?;
        let marker = fd::fcntl_dupfd_cloexec(&null, floor)?;
        // Opening /dev/null may itself have allocated one requested number.
        drop(null);
        let mut slots = Vec::with_capacity(destinations.len());
        for &target in destinations {
            let slot = fd::fcntl_dupfd_cloexec(&marker, target)?;
            if slot.as_raw_fd() != target {
                return Err(fd::Errno::BUSY.into());
            }
            slots.push(slot);
        }
        Ok(Self {
            slots,
            shared: None,
        })
    }

    /// Only the sealed native-command wrapper can register this exact mapping.
    #[allow(
        unsafe_code,
        reason = "the child uses only descriptor operations on already-owned slots and preallocated originals after fork"
    )]
    pub(crate) fn configure(
        self: &Arc<Self>,
        command: &mut Command,
        mappings: &[(BorrowedFd<'_>, RawFd)],
    ) -> io::Result<()> {
        if mappings.len() > self.slots.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too many reserved descriptor mappings",
            ));
        }
        let floor = 1 + self
            .slots
            .iter()
            .map(|slot| slot.as_raw_fd())
            .max()
            .expect("nonempty reservation");
        let mut originals = Vec::with_capacity(mappings.len());
        for (i, (source, target)) in mappings.iter().enumerate() {
            let index = self
                .slots
                .iter()
                .position(|slot| slot.as_raw_fd() == *target)
                .filter(|_| !mappings[..i].iter().any(|(_, previous)| previous == target))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unreserved or duplicate descriptor mapping",
                    )
                })?;
            let expected = fs::fstat(&self.slots[index])?;
            originals.push((
                fd::fcntl_dupfd_cloexec(source, floor)?,
                index,
                expected.st_dev,
                expected.st_ino,
                expected.st_rdev,
            ));
        }
        if originals.is_empty() {
            return Ok(());
        }
        let owner = Arc::clone(self);
        // SAFETY: the parent retains all exact slots and duplicated sources in the closure.
        // Fork copies their descriptor table. The Rust exec-error pipe and standard stream
        // preparation cannot allocate an occupied slot. No allocation or destructor runs
        // in the child callback. All slots are checked before replacing the first one.
        unsafe {
            command.pre_exec(move || {
                for (_, index, dev, ino, rdev) in &originals {
                    let slot = &owner.slots[*index];
                    let actual = fs::fstat(slot)?;
                    if !fd::fcntl_getfd(slot)?.contains(fd::FdFlags::CLOEXEC)
                        || actual.st_dev != *dev
                        || actual.st_ino != *ino
                        || actual.st_rdev != *rdev
                    {
                        return Err(fd::Errno::BUSY.into());
                    }
                }
                for (source, index, ..) in &originals {
                    // dup2 clears CLOEXEC on the installed child mapping. Only this child's
                    // copy of an exact owned placeholder may be replaced.
                    if dup2(source.as_raw_fd(), owner.slots[*index].as_raw_fd()) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        Ok(())
    }
}

type SharedEntry = Option<Weak<ReservedProcessDescriptors>>;

/// Scoped sharing of one fixed set of process descriptor slots. A registry holds
/// a weak entry only while parent or command owners still use that generation.
#[derive(Debug)]
pub struct SharedProcessDescriptorReservations {
    destinations: &'static [RawFd],
    entry: Mutex<SharedEntry>,
    retired: Condvar,
}

impl SharedProcessDescriptorReservations {
    /// Describe the slots. No process resources are acquired until `acquire`.
    pub const fn new(destinations: &'static [RawFd]) -> Self {
        Self {
            destinations,
            entry: Mutex::new(None),
            retired: Condvar::new(),
        }
    }

    /// Share a current reservation or acquire the exact slots once its predecessor has closed them.
    ///
    /// # Errors
    /// Returns an error for a poisoned registry, invalid slots, genuine prior
    /// occupancy or exhaustion. Only retirement of this registry's own generation is awaited.
    pub fn acquire(&'static self) -> io::Result<Arc<ReservedProcessDescriptors>> {
        let (mut entry, current) = self.ready()?;
        if let Some(current) = current {
            return Ok(current);
        }
        let mut reservation = ReservedProcessDescriptors::new(self.destinations)?;
        reservation.shared = Some(self);
        let current = Arc::new(reservation);
        *entry = Some(Arc::downgrade(&current));
        Ok(current)
    }

    /// Borrow the live reservation, or observe vacancy after all previous slots are closed.
    ///
    /// # Errors
    /// Returns an error if the registry is poisoned.
    pub fn active(&'static self) -> io::Result<Option<Arc<ReservedProcessDescriptors>>> {
        let (_entry, current) = self.ready()?;
        Ok(current)
    }

    fn ready(
        &self,
    ) -> io::Result<(
        MutexGuard<'_, SharedEntry>,
        Option<Arc<ReservedProcessDescriptors>>,
    )> {
        let poisoned = |_| io::Error::other("process descriptor reservation registry poisoned");
        let mut entry = self.entry.lock().map_err(poisoned)?;
        loop {
            match entry.as_ref() {
                None => return Ok((entry, None)),
                Some(previous) => {
                    if let Some(current) = previous.upgrade() {
                        return Ok((entry, Some(current)));
                    }
                }
            }
            // Weak upgrade already fails while the last owner's destructor is
            // still pending. The destructor records vacancy only after closing every slot.
            #[cfg(test)]
            tests::at_retirement_wait(self);
            entry = self.retired.wait(entry).map_err(poisoned)?;
        }
    }
}

impl Drop for ReservedProcessDescriptors {
    fn drop(&mut self) {
        if let Some(shared) = self.shared {
            #[cfg(test)]
            tests::before_slot_retirement(shared);
            let mut entry = shared
                .entry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.slots.clear();
            *entry = None;
            shared.retired.notify_all();
        }
    }
}

#[allow(
    unsafe_code,
    reason = "POSIX dup2 is async-signal-safe and used only on the child's reserved targets"
)]
unsafe extern "C" {
    fn dup2(source: RawFd, destination: RawFd) -> RawFd;
}

#[cfg(test)]
#[path = "reserved_descriptors_tests.rs"]
mod tests;
