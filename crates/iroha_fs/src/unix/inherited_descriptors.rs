//! Bounded original descriptor transfer without overwriting a child's occupied descriptors.

use std::{
    io,
    os::{
        fd::{AsRawFd as _, BorrowedFd, IntoRawFd as _, OwnedFd, RawFd},
        unix::process::CommandExt as _,
    },
    process::Command,
};

const MAX_MAPPINGS: usize = 64;
const MAX_DESTINATION: RawFd = 4095;

/// Keep independent originals alive and install fixed child descriptors at native launch.
///
/// Every source is duplicated with close-on-exec above every destination before this
/// function returns. The caller may then drop its original descriptors. This does not
/// change the command's executable, working directory, environment or standard streams.
///
/// A destination must be unused in the forked child. An occupied destination is refused
/// before installing any mapping, including one belonging to Rust's private exec-error
/// channel. Fixed protocol descriptors are never silently renumbered. A source which
/// itself occupies a destination must therefore be dropped by its caller before launch.
///
/// # Errors
/// Refuses more than 64 mappings, duplicate destinations, destinations outside 3..=4095,
/// source duplication failure, or an occupied child destination. Resource and exec errors
/// remain errors; this function does not authenticate the selected executable.
#[allow(
    unsafe_code,
    reason = "the private post-fork callback only probes and duplicates descriptors using async-signal-safe operations"
)]
pub fn inherit_descriptors(
    command: &mut Command,
    mappings: &[(BorrowedFd<'_>, RawFd)],
) -> io::Result<()> {
    if mappings.len() > MAX_MAPPINGS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many inherited descriptor mappings",
        ));
    }
    let mut maximum = 2;
    for (index, (_, target)) in mappings.iter().enumerate() {
        if !(3..=MAX_DESTINATION).contains(target)
            || mappings[..index].iter().any(|(_, other)| other == target)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "inherited descriptor destinations must be distinct and within 3..=4095",
            ));
        }
        maximum = maximum.max(*target);
    }
    let floor = maximum.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "descriptor duplication overflow",
        )
    })?;
    let mut originals = Vec::with_capacity(mappings.len());
    for (source, target) in mappings {
        originals.push((rustix::io::fcntl_dupfd_cloexec(source, floor)?, *target));
    }
    if originals.is_empty() {
        return Ok(());
    }

    // SAFETY: allocation and ownership acquisition finish above, in the parent. The
    // callback calls only POSIX descriptor operations and constructs raw OS errors.
    // A same-number dup2 is a nonmutating validity probe: it preserves CLOEXEC too.
    // No invalid BorrowedFd is constructed to probe an absent raw destination.
    unsafe {
        command.pre_exec(move || {
            for (_, target) in &originals {
                require_unused(*target)?;
            }
            for (source, target) in &originals {
                // Unlike dup2, F_DUPFD_CLOEXEC never closes a descriptor which became occupied
                // after the admission pass (for example in a native signal handler).
                // All originals are above all destinations, so the exact free target
                // is the lowest available descriptor at or above the requested floor.
                install_exact(source, *target)?;
            }
            // Parent-owned originals stay CLOEXEC; exec closes them in the child. No
            // captured owner or heap allocation is dropped in this callback.
            Ok(())
        });
    }
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "POSIX same-number dup2 is a nonmutating presence probe, including for a possibly absent raw descriptor"
)]
fn require_unused(target: RawFd) -> io::Result<()> {
    unsafe extern "C" {
        fn dup2(source: RawFd, destination: RawFd) -> RawFd;
    }
    // SAFETY: dup2 accepts any raw descriptor, reporting EBADF for an absent one.
    // Equal source/destination never closes, changes flags or changes file position.
    if unsafe { dup2(target, target) } >= 0 {
        return Err(io::Error::from_raw_os_error(
            rustix::io::Errno::BUSY.raw_os_error(),
        ));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(rustix::io::Errno::BADF.raw_os_error()) {
        return Err(error);
    }
    Ok(())
}

fn install_exact(source: &OwnedFd, target: RawFd) -> io::Result<()> {
    let installed = rustix::io::fcntl_dupfd_cloexec(source, target)?;
    if installed.as_raw_fd() != target {
        drop(installed); // Close only our newly allocated child descriptor.
        return Err(io::Error::from_raw_os_error(
            rustix::io::Errno::BUSY.raw_os_error(),
        ));
    }
    rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty())?;
    let _ = installed.into_raw_fd(); // The exec'd child now owns this descriptor.
    Ok(())
}

#[cfg(test)]
#[path = "inherited_descriptors_tests.rs"]
mod tests;
