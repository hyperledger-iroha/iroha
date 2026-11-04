//! Early same-image handoff from stdio to the native ceremony's fixed descriptors.
//!
//! The supervisor's first exec carries only ordinary stdio. Its unrelated CLOEXEC
//! network descriptors and Rust's private exec-error channel have therefore closed
//! before this module reserves any protocol slot. We neither close an occupied
//! slot nor launch a second image: the existing typed command continues in this PID.

use super::{Directory, Error, Result};
use std::{
    ffi::OsStr,
    fs::File,
    io,
    os::{
        fd::{AsRawFd as _, BorrowedFd, IntoRawFd as _, OwnedFd},
        unix::fs::MetadataExt as _,
    },
    path::Path,
};

/// Dispatch the narrow internal transport before runtime initialization or logging.
pub(crate) fn dispatch_if_requested() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new("beacon-descriptor-relay")) {
        return false;
    }
    match args.next().as_deref() {
        Some(command) if command == "beacon-bootstrap" => super::dispatch_arguments(args, true),
        Some(command) if command == "beacon-prepare-custody" => {
            crate::external_software_signer::dispatch_beacon_custody_arguments(args, true)
        }
        // stderr can still be a public-input FIFO. Do not write diagnostics to it.
        _ => std::process::exit(2),
    }
    true
}

/// Claim exact unused slots without ever replacing an existing descriptor.
fn reserve(mappings: &[(BorrowedFd<'_>, i32)]) -> io::Result<Vec<OwnedFd>> {
    let mut slots = Vec::with_capacity(mappings.len());
    for (source, target) in mappings {
        let slot = rustix::io::fcntl_dupfd_cloexec(source, *target)?;
        if slot.as_raw_fd() != *target {
            // Only our new duplicate and earlier reservations are dropped. The
            // unrelated occupied target retains its bytes, flags and ownership.
            return Err(rustix::io::Errno::BUSY.into());
        }
        slots.push(slot);
    }
    Ok(slots)
}

/// The canonical native decoders become the sole owners of the fixed slots.
fn transfer(slots: Vec<OwnedFd>) {
    for slot in slots {
        let _ = slot.into_raw_fd();
    }
}

/// Hand one native key/config record to the unchanged FD 198 decoder.
pub(crate) fn prepare_key() -> io::Result<()> {
    let slots = reserve(&[(rustix::stdio::stdin(), 198)])?;
    let null = File::open("/dev/null")?;
    rustix::stdio::dup2_stdin(&null)?;
    transfer(slots);
    Ok(())
}

/// Hand the pending share and optional incumbent record to their native decoders.
pub(crate) fn prepare_pending(current: bool) -> io::Result<()> {
    let mut mappings = vec![(rustix::stdio::stdin(), 198)];
    if current {
        mappings.push((rustix::stdio::stdout(), 200));
    }
    let slots = reserve(&mappings)?;
    let null = File::options().read(true).write(true).open("/dev/null")?;
    rustix::stdio::dup2_stdin(&null)?;
    if current {
        rustix::stdio::dup2_stdout(&null)?;
    }
    transfer(slots);
    Ok(())
}

/// Restore owner-private seat logs only after all three fixed slots are reserved.
pub(crate) fn prepare_seat(root: &Path) -> Result<()> {
    let slots = reserve(&[
        (rustix::stdio::stdin(), 198),
        (rustix::stdio::stdout(), 201),
        (rustix::stdio::stderr(), 202),
    ])
    .map_err(|_| Error::Io)?;
    let directory = Directory::open(root)?;
    let metadata = directory.file.metadata().map_err(|_| Error::Io)?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o7777 != 0o700 {
        return Err(Error::InvalidCustody);
    }
    let stdout = private_log(&directory, "stdout.log")?;
    let stderr = private_log(&directory, "stderr.log")?;
    let null = File::open("/dev/null").map_err(|_| Error::Io)?;
    rustix::stdio::dup2_stdin(&null).map_err(|_| Error::Io)?;
    rustix::stdio::dup2_stdout(&stdout).map_err(|_| Error::Io)?;
    rustix::stdio::dup2_stderr(&stderr).map_err(|_| Error::Io)?;
    directory.revalidate()?;
    transfer(slots);
    Ok(())
}

fn private_log(directory: &Directory, name: &str) -> Result<File> {
    use rustix::fs::{Mode, OFlags};
    directory.revalidate()?;
    let file = File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| Error::Io)?,
    );
    let metadata = file.metadata().map_err(|_| Error::Io)?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(Error::InvalidCustody);
    }
    directory.revalidate()?;
    Ok(file)
}

#[cfg(test)]
#[path = "descriptor_relay_tests.rs"]
mod tests;
