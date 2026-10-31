//! Child-only inheritance of a selected native build-tool descriptor.

use std::{
    ffi::OsStr, fs::File, io, os::unix::process::CommandExt as _, process::Command, sync::Arc,
};

#[allow(
    unsafe_code,
    reason = "pre_exec only changes CLOEXEC on its own preallocated descriptor using POSIX fcntl"
)]
pub fn build_input_environment<T: Send + Sync + 'static>(
    command: &mut Command,
    key: &OsStr,
    file: &File,
    owner: Arc<T>,
) -> io::Result<()> {
    // Allocation selects a genuinely free number and owns it before std prepares
    // any streams/error channel. No fixed descriptor is reserved or overwritten.
    let original = File::from(rustix::io::fcntl_dupfd_cloexec(file, 3)?);
    let projection = super::sealed_batch::flat_batch_descriptor_path(&original)?;
    command.env(key, projection);
    // SAFETY: original and owner were fully allocated in the parent. The callback
    // performs only POSIX fcntl on a still-owned descriptor, with no allocation,
    // locks or destructor. Parent flags remain CLOEXEC after fork. Existing fixed
    // descriptor mappings fail on this occupied number instead of replacing it.
    unsafe {
        command.pre_exec(move || {
            std::hint::black_box(&owner);
            let mut flags = rustix::io::fcntl_getfd(&original)?;
            flags.remove(rustix::io::FdFlags::CLOEXEC);
            rustix::io::fcntl_setfd(&original, flags)?;
            Ok(())
        });
    }
    Ok(())
}
