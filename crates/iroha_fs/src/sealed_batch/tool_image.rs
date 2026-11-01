//! Anonymous kernel-sealed executable copies of admitted batch originals.

use super::{FlatBatchKind, SealedFlatBatch};
use crate::{FileIdentity, denied, invalid, platform};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, Read as _, Write as _},
    os::unix::{fs::MetadataExt as _, process::CommandExt as _},
    sync::Arc,
};

use rustix::fs::{MemfdFlags, Mode, SealFlags};

const IMAGE_NAME: &str = "iroha-retained-tool";
const IMAGE_ARGV0: &str = "/iroha-retained-tool";

fn required_seals() -> SealFlags {
    SealFlags::WRITE
        | SealFlags::FUTURE_WRITE
        | SealFlags::GROW
        | SealFlags::SHRINK
        | SealFlags::EXEC
        | SealFlags::SEAL
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Snapshot {
    identity: FileIdentity,
    length: u64,
    mode: u32,
    owner: u32,
    group: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn snapshot(file: &File) -> io::Result<Snapshot> {
    let metadata = crate::FileMetadata::of(file)?;
    if !metadata.is_file()
        || metadata.links() != 0
        || metadata.mode() & 0o7777 != 0o500
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || rustix::fs::fcntl_get_seals(file)? != required_seals()
    {
        return Err(denied(
            "executable memory image lost its required kernel seal",
        ));
    }
    Ok(Snapshot {
        identity: metadata.identity(),
        length: metadata.len(),
        mode: metadata.mode(),
        owner: metadata.uid(),
        group: metadata.gid(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Copied,
    Sealed,
    Compared,
}

/// One anonymous executable, kernel-sealed against content, extent and execute-mode changes.
///
/// Linux/Android create a distinct executable memfd and compare every byte with an
/// already admitted batch original. The diagnostic name has no filesystem leaf to
/// rename. Commands use the descriptor and a fixed non-multicall argv0; nested tool
/// bindings inherit another retained descriptor. No file or raw descriptor escapes.
/// The source batch remains held and rechecked through the caller's original child
/// lifetime. This owner authenticates neither source bytes nor dynamic dependencies.
/// It refuses missing executable-memfd or required-seal support and never changes
/// system security policy or falls back to pathname execution.
///
/// ```compile_fail
/// use iroha_fs::SealedToolImage;
/// fn escape(image: SealedToolImage<1>) { let file = image.file(); }
/// ```
#[derive(Debug)]
pub struct SealedToolImage<const N: usize> {
    source: Arc<SealedFlatBatch<N>>,
    file: File,
    before: Snapshot,
}

impl<const N: usize> SealedFlatBatch<N> {
    /// Copy an executable original into a distinct anonymous kernel-sealed image.
    ///
    /// The existing entry ceiling bounds memory use. This intentionally requires
    /// executable memfd policy and all six seals, including execute-mode sealing.
    /// The returned owner retains this complete batch and exposes no mutable handle.
    ///
    /// # Errors
    /// Refuses invalid/nonexecutable entries, missing native ELF magic, changed
    /// originals, copy/readback errors, resource exhaustion and unavailable or
    /// denied kernel executable-memory policy. There is no compatibility fallback.
    pub fn sealed_tool_image(
        self: &Arc<Self>,
        index: usize,
    ) -> io::Result<Arc<SealedToolImage<N>>> {
        SealedToolImage::copy_observed(self, index, |_| Ok(()))
    }
}

impl<const N: usize> SealedToolImage<N> {
    fn copy_observed(
        source: &Arc<SealedFlatBatch<N>>,
        index: usize,
        mut observe: impl FnMut(Step) -> io::Result<()>,
    ) -> io::Result<Arc<Self>> {
        source.revalidate()?;
        let result = (|| {
            let entry = source
                .entries
                .get(index)
                .ok_or_else(|| invalid("flat batch index is out of bounds"))?;
            if entry.kind != FlatBatchKind::Executable {
                return Err(denied(
                    "only an executable batch entry may create a tool image",
                ));
            }
            let source_before = source.entry_snapshot(index)?;
            let length = source_before.local_parts().2;
            if length < 4 || length > entry.maximum {
                return Err(invalid("executable memory image has an invalid extent"));
            }
            let mut reader = source.reader(index)?;
            let mut magic = [0_u8; 4];
            reader.read_exact(&mut magic)?;
            if magic != *b"\x7fELF" {
                return Err(invalid("executable memory image requires native ELF bytes"));
            }
            // Explicit EXEC honors the current namespace/LSM executable-memfd policy.
            // In particular, denial is never retried with weaker creation flags.
            let mut file = File::from(rustix::fs::memfd_create(
                IMAGE_NAME,
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
            )?);
            let mut reader = source.reader(index)?;
            let mut buffer = [0_u8; 16 * 1024];
            let mut copied = 0_u64;
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                copied = copied
                    .checked_add(count as u64)
                    .filter(|value| *value <= length)
                    .ok_or_else(|| invalid("executable memory copy exceeded original extent"))?;
                file.write_all(&buffer[..count])?;
            }
            if copied != length {
                return Err(invalid("executable memory copy ended early"));
            }
            observe(Step::Copied)?;
            rustix::fs::fchmod(&file, Mode::RUSR | Mode::XUSR)?;
            rustix::fs::fcntl_add_seals(&file, required_seals())?;
            let before = snapshot(&file)?;
            if before.length != length || before.identity == source.entry_identity(index)? {
                return Err(invalid(
                    "executable memory image is not a distinct exact original",
                ));
            }
            observe(Step::Sealed)?;
            let mut reader = source.reader(index)?;
            let mut other = [0_u8; 16 * 1024];
            let mut offset = 0_u64;
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                crate::read_exact_at(&file, &mut other[..count], offset)?;
                if buffer[..count] != other[..count] {
                    return Err(invalid("sealed executable memory bytes differ from source"));
                }
                offset = offset
                    .checked_add(count as u64)
                    .filter(|value| *value <= length)
                    .ok_or_else(|| invalid("executable readback exceeded original extent"))?;
            }
            if offset != length || crate::read_at(&file, &mut other[..1], offset)? != 0 {
                return Err(invalid("sealed executable memory readback extent differs"));
            }
            observe(Step::Compared)?;
            let image = Arc::new(Self {
                source: Arc::clone(source),
                file,
                before,
            });
            let image_check = image.revalidate();
            let source_check = source.entry_snapshot(index).and_then(|now| {
                if now == source_before {
                    Ok(())
                } else {
                    Err(invalid("tool image source changed during copy"))
                }
            });
            image_check?;
            source_check?;
            Ok(image)
        })();
        // Observe the retained source after every ordinary copy/seal/observer error.
        // The initiating error stays first; an unreturned anonymous image just closes.
        let final_source = source.revalidate();
        let image = result?;
        final_source?;
        Ok(image)
    }

    /// Return this distinct anonymous original's complete local identity.
    ///
    /// # Errors
    /// Refuses changed source custody, metadata or required kernel seals.
    pub fn identity(&self) -> io::Result<FileIdentity> {
        self.revalidate()?;
        Ok(self.before.identity)
    }

    /// Check both the original source batch and the complete anonymous image snapshot.
    ///
    /// # Errors
    /// Refuses source drift, image policy/identity changes and failed kernel queries.
    pub fn revalidate(&self) -> io::Result<()> {
        let source_check = self.source.revalidate();
        let image_check = snapshot(&self.file).and_then(|now| {
            if now == self.before {
                Ok(())
            } else {
                Err(invalid("sealed executable memory original changed"))
            }
        });
        source_check?;
        image_check
    }

    /// Construct a descriptor-selected native command with a fixed non-`cargo` argv0.
    ///
    /// The command retains this owner through spawn. The release caller retains its
    /// complete admission through original terminal observation and revalidates it.
    ///
    /// # Errors
    /// Refuses changed custody, unavailable descriptor projection or native errors.
    pub fn command(self: &Arc<Self>) -> io::Result<crate::ProcessCommand> {
        self.revalidate()?;
        let mut command = platform::flat_batch_command(&self.file, Arc::clone(self))?;
        command.arg0(IMAGE_ARGV0);
        self.revalidate()?;
        Ok(crate::ProcessCommand::from_retained_descriptor(command))
    }

    pub(crate) fn bind_environment(
        self: &Arc<Self>,
        command: &mut std::process::Command,
        key: &OsStr,
    ) -> io::Result<()> {
        self.revalidate()?;
        let result = platform::build_input_environment(command, key, &self.file, Arc::clone(self));
        let final_check = self.revalidate();
        result?;
        final_check
    }
}

#[cfg(test)]
#[path = "tool_image_tests.rs"]
mod tests;
