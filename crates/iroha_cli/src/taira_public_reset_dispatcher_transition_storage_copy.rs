//! Private copy prefixes acquire their final mode only after complete content verification.
use super::*;

/// Resume exact private prefixes or a complete copy interrupted after its final chmod.
pub(in super::super) fn copy_exact(source: &Path, target: &Path, pin: &Pin) -> Result<()> {
    if exists(target)? {
        checked(target, pin)?;
        return Ok(());
    }
    let parent = target
        .parent()
        .ok_or_else(|| eyre!("copy parent missing"))?;
    let held_parent = direct_directory(parent)?;
    let mut input = checked(source, pin)?;
    let staged = target.with_file_name(format!(
        ".{}.partial",
        target
            .file_name()
            .ok_or_else(|| eyre!("copy name missing"))?
            .to_string_lossy()
    ));
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    let mut output = match options.create_new(true).mode(0o600).open(&staged) {
        Ok(file) => {
            // Only the exclusively created descriptor may normalize a mode masked by umask.
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            file
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            options.create_new(false).open(&staged)?
        }
        Err(error) => return Err(error.into()),
    };
    let meta = output.metadata()?;
    let mode = meta.mode() & 0o7777;
    need(
        meta.is_file()
            && meta.nlink() == 1
            && meta.uid() == rustix::process::geteuid().as_raw()
            && meta.len() <= pin.size
            && (mode == 0o600 || (mode == pin.mode && meta.len() == pin.size)),
        "unsafe partial transition copy",
    )?;
    let (_, snapshot) = open_pinned_regular(&staged, "partial transition copy")?;
    ensure_pinned_unchanged(&staged, "partial transition copy", &output, &snapshot)?;
    let mut offset = 0u64;
    let mut left = [0u8; 64 * 1024];
    let mut right = [0u8; 64 * 1024];
    while offset < meta.len() {
        let n = usize::try_from((meta.len() - offset).min(left.len() as u64))?;
        input.read_exact(&mut left[..n])?;
        output.read_exact(&mut right[..n])?;
        need(left[..n] == right[..n], "partial copy prefix differs")?;
        offset += n as u64;
    }
    ensure_pinned_unchanged(&staged, "partial transition copy", &output, &snapshot)?;
    while offset < pin.size {
        let n = usize::try_from((pin.size - offset).min(left.len() as u64))?;
        input.read_exact(&mut left[..n])?;
        output.write_all(&left[..n])?;
        offset += n as u64;
    }
    output.sync_all()?;
    checked(source, pin)?;
    let staged_pin = Pin {
        mode,
        ..pin.clone()
    };
    let complete = checked(&staged, &staged_pin)?;
    let complete_meta = complete.metadata()?;
    need(
        complete_meta.dev() == meta.dev() && complete_meta.ino() == meta.ino(),
        "partial copy inode changed before mode publication",
    )?;
    check_directory(parent, &held_parent)?;
    output.set_permissions(fs::Permissions::from_mode(pin.mode))?;
    output.sync_all()?;
    let final_meta = checked(&staged, pin)?.metadata()?;
    need(
        final_meta.dev() == meta.dev() && final_meta.ino() == meta.ino(),
        "partial copy inode changed after mode publication",
    )?;
    check_directory(parent, &held_parent)?;
    move_exact(&staged, target, pin)
}
