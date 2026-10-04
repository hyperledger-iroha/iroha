//! One original prepared output owner with resumable exact-descriptor publication.
//!
//! The surrounding DKG attempt owns the directory path and pinned descriptor.
//! Preparation and publication borrow that exact source, so every refusal retains
//! the same phase owner, private bytes and completed-file progress.

use super::*;
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, PrepaidBufferError};
use iroha_core::beacon::credential::{
    GlobalBeaconCredentialEncodeErrorV1, PreparedGlobalBeaconCredentialV1,
};
use norito::json::{BoundedJsonError, JsonSerialize as _, JsonWriteSink};

/// Exact final-output source failure; no local I/O condition is protocol invalidity.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ExportError {
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    #[error(transparent)]
    Buffer(#[from] PrepaidBufferError),
    #[error(transparent)]
    Credential(#[from] GlobalBeaconCredentialEncodeErrorV1),
    #[error(transparent)]
    Encoding(#[from] norito::Error),
    #[error(transparent)]
    Json(#[from] BoundedJsonError),
    #[error("beacon export descriptor operation failed")]
    Io(#[source] std::io::Error),
    #[error("beacon export original directory or file identity changed")]
    Custody,
    #[error("beacon export prepared source or phase changed")]
    Phase,
}

struct ByteWriter<'a>(&'a mut ChargedBuffer<u8>);
impl std::io::Write for ByteWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct CountJson(usize);
impl JsonWriteSink for CountJson {
    fn push(&mut self, value: char) -> std::result::Result<(), BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0u8; 4]))
    }
    fn push_str(&mut self, value: &str) -> std::result::Result<(), BoundedJsonError> {
        self.0 = self
            .0
            .checked_add(value.len())
            .filter(|n| *n <= MAX_PUBLIC_BYTES)
            .ok_or(BoundedJsonError::BodyTooLarge)?;
        Ok(())
    }
}
struct WriteJson<'a>(&'a mut ChargedBuffer<u8>);
impl JsonWriteSink for WriteJson<'_> {
    fn push(&mut self, value: char) -> std::result::Result<(), BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0u8; 4]))
    }
    fn push_str(&mut self, value: &str) -> std::result::Result<(), BoundedJsonError> {
        self.0
            .append(value.as_bytes())
            .map_err(|_| BoundedJsonError::LengthMismatch)
    }
}
#[derive(JsonSerialize)]
struct ProviderRef<'a> {
    signer_index: u16,
    validator: &'a PeerId,
    handle: &'a str,
    revision: u64,
    policy_digest: [u8; 32],
}

#[derive(Default)]
pub(super) struct FileProgress {
    descriptor: Option<File>,
    offset: usize,
    synced: bool,
    complete: bool,
}
impl FileProgress {
    pub(super) fn complete(&self) -> bool {
        self.complete
    }
}
struct Outputs {
    credential: PreparedGlobalBeaconCredentialV1,
    public: ChargedBuffer<u8>,
    provider: ChargedBuffer<u8>,
    pending: Zeroizing<[u8; 96]>,
}
impl Outputs {
    fn bytes(&self, index: usize) -> std::result::Result<&[u8], ExportError> {
        match index {
            0 => self.credential.encoded().ok_or(ExportError::Phase),
            1 => Ok(self.pending.as_ref()),
            2 => Ok(self.public.as_slice()),
            3 => Ok(self.provider.as_slice()),
            _ => Err(ExportError::Phase),
        }
    }
}
const FILES: [(&str, bool); 4] = [
    (GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1, true),
    (ROTATION_PENDING_SHARE_NAME, true),
    ("public-session.norito", false),
    ("provider.json", false),
];

/// Move-only source for every byte and exact file descriptor of one final seat export.
pub(crate) struct PreparedSeatExport {
    directory_identity: DirectoryIdentity,
    public: ValidatedGlobalThresholdBeaconSessionV1,
    seat: u16,
    source: Option<RuntimeGlobalBeaconShareProvisioningV1>,
    outputs: Outputs,
    files: [FileProgress; 4],
    terminal_custody_failure: bool,
}
impl PreparedSeatExport {
    /// Prepare both public files and the secret output backing before extracting a share.
    pub(crate) fn new(
        directory: &Directory,
        public: &ValidatedGlobalThresholdBeaconSessionV1,
        seat: u16,
        handle: &str,
        revision: u64,
        budget: &AllocationBudget,
    ) -> std::result::Result<Self, ExportError> {
        let digest = global_beacon_partial_signer_public_inventory_digest_v1(
            public.network_id,
            &[(public.record(), seat)],
        )
        .map_err(GlobalBeaconCredentialEncodeErrorV1::from)?;
        let credential = PreparedGlobalBeaconCredentialV1::new(
            public.network_id,
            handle,
            revision,
            digest,
            [(public, seat)],
            budget,
        )?;
        let validator = &public.adaptive_dkg.recipient_keys[usize::from(seat - 1)].validator;
        let provider = ProviderRef {
            signer_index: seat,
            validator,
            handle,
            revision,
            policy_digest: digest,
        };
        let mut count = CountJson(0);
        provider.json_serialize_to(&mut count)?;
        let public_len = norito::canonical_frame_len(public.record())?;
        if count.0 == 0 || public_len == 0 || public_len > MAX_PUBLIC_BYTES {
            return Err(ExportError::Phase);
        }
        let total = count
            .0
            .checked_add(public_len)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let mut reservation = budget.try_reserve_bytes(total)?;
        let mut public_bytes = ChargedBuffer::from_reservation(public_len, &mut reservation)?;
        let mut provider_bytes = ChargedBuffer::from_reservation(count.0, &mut reservation)?;
        norito::core::write_canonical_to_writer(
            public.record(),
            &mut ByteWriter(&mut public_bytes),
        )?;
        provider.json_serialize_to(&mut WriteJson(&mut provider_bytes))?;
        if public_bytes.as_slice().len() != public_len || provider_bytes.as_slice().len() != count.0
        {
            return Err(ExportError::Phase);
        }
        let directory_identity = DirectoryIdentity::new(directory)?;
        Ok(Self {
            directory_identity,
            public: public.clone(),
            seat,
            source: None,
            outputs: Outputs {
                credential,
                public: public_bytes,
                provider: provider_bytes,
                pending: Zeroizing::new([0; 96]),
            },
            files: std::array::from_fn(|_| FileProgress::default()),
            terminal_custody_failure: false,
        })
    }

    /// Move extracted components into this original owner exactly once, then encode by borrow.
    /// The source remains owned here on any local primitive or encoding failure.
    pub(crate) fn accept(
        &mut self,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> std::result::Result<(), (Zeroizing<[[u8; 32]; 3]>, ExportError)> {
        if self.source.is_some() {
            return Err((components, ExportError::Phase));
        }
        for (destination, component) in self
            .outputs
            .pending
            .chunks_exact_mut(32)
            .zip(components.iter())
        {
            destination.copy_from_slice(component);
        }
        self.source = Some(RuntimeGlobalBeaconShareProvisioningV1::new(
            self.public.clone(),
            self.seat,
            components,
        ));
        Ok(())
    }

    /// Complete encoding and resume publication through the original descriptors and offsets.
    /// No failure consumes this owner or opens an existing output as a replacement destination.
    pub(crate) fn publish(
        &mut self,
        directory: &Directory,
    ) -> std::result::Result<(), ExportError> {
        if self.terminal_custody_failure {
            return Err(ExportError::Custody);
        }
        if !self.directory_identity.matches(directory)? {
            self.terminal_custody_failure = true;
            return Err(ExportError::Custody);
        }
        if self.outputs.credential.encoded().is_none() {
            let source = self.source.as_ref().ok_or(ExportError::Phase)?;
            encode_global_beacon_partial_signer_credential_v1(
                &mut self.outputs.credential,
                std::iter::once(source),
            )?;
        }
        for (index, (name, private)) in FILES.iter().copied().enumerate() {
            let result = publish_file(
                directory,
                name,
                private,
                self.outputs.bytes(index)?,
                &mut self.files[index],
            );
            if matches!(result, Err(ExportError::Custody)) {
                self.terminal_custody_failure = true;
            }
            result?;
        }
        Ok(())
    }

    /// Whether all four exact outputs have been durably verified under their original names.
    pub(crate) fn complete(&self) -> bool {
        self.files.iter().all(|file| file.complete)
    }
}

// The surrounding attempt owns the exact charged directory path and descriptor.
// Export preparation borrows it so a failed constructor cannot consume that owner.
struct DirectoryIdentity {
    metadata: std::fs::Metadata,
    path: Hash,
}
impl DirectoryIdentity {
    fn new(directory: &Directory) -> std::result::Result<Self, ExportError> {
        use std::os::unix::ffi::OsStrExt as _;
        revalidate_directory(directory)?;
        Ok(Self {
            metadata: directory.file.metadata().map_err(ExportError::Io)?,
            path: Hash::new(directory.path.as_os_str().as_bytes()),
        })
    }
    fn matches(&self, directory: &Directory) -> std::result::Result<bool, ExportError> {
        use std::os::unix::ffi::OsStrExt as _;
        let now = directory.file.metadata().map_err(ExportError::Io)?;
        Ok(self.metadata.dev() == now.dev()
            && self.metadata.ino() == now.ino()
            && self.metadata.uid() == now.uid()
            && self.metadata.gid() == now.gid()
            && self.metadata.mode() == now.mode()
            && self.path == Hash::new(directory.path.as_os_str().as_bytes()))
    }
}

// Recheck every canonical ancestor and the held inode. This performs no pathname
// allocation and never substitutes a freshly opened directory for the original writer.
pub(super) fn revalidate_directory(directory: &Directory) -> std::result::Result<(), ExportError> {
    use rustix::fs::{Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut current = File::from(
        rustix::fs::open("/", flags, Mode::empty()).map_err(|e| ExportError::Io(e.into()))?,
    );
    validate_directory(&current.metadata().map_err(ExportError::Io)?)
        .map_err(|_| ExportError::Custody)?;
    for part in directory.path.components() {
        match part {
            Component::RootDir => {}
            Component::Normal(name) => {
                current = File::from(
                    rustix::fs::openat(&current, name, flags, Mode::empty())
                        .map_err(|e| ExportError::Io(e.into()))?,
                );
                validate_directory(&current.metadata().map_err(ExportError::Io)?)
                    .map_err(|_| ExportError::Custody)?;
            }
            _ => return Err(ExportError::Custody),
        }
    }
    let held = directory.file.metadata().map_err(ExportError::Io)?;
    let named = current.metadata().map_err(ExportError::Io)?;
    validate_directory(&held).map_err(|_| ExportError::Custody)?;
    if held.dev() != named.dev()
        || held.ino() != named.ino()
        || held.uid() != named.uid()
        || held.gid() != named.gid()
        || held.mode() != named.mode()
    {
        return Err(ExportError::Custody);
    }
    Ok(())
}
fn validate_output(
    file: &File,
    private: bool,
    expected_length: usize,
) -> std::result::Result<(), ExportError> {
    let m = file.metadata().map_err(ExportError::Io)?;
    if !m.is_file()
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.nlink() != 1
        || (private && m.mode() & 0o7777 != 0o600)
        || m.len() != u64::try_from(expected_length).map_err(|_| ExportError::Phase)?
    {
        return Err(ExportError::Custody);
    }
    Ok(())
}
pub(super) fn publish_file(
    directory: &Directory,
    name: &str,
    private: bool,
    bytes: &[u8],
    progress: &mut FileProgress,
) -> std::result::Result<(), ExportError> {
    use rustix::fs::{Mode, OFlags};
    revalidate_directory(directory)?;
    if progress.descriptor.is_none() {
        let file = File::from(
            rustix::fs::openat(
                &directory.file,
                name,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(if private { 0o600 } else { 0o644 }),
            )
            .map_err(|e| ExportError::Io(e.into()))?,
        );
        // Install custody before metadata/fsync/write can refuse: never recreate this file.
        progress.descriptor = Some(file);
    }
    let file = progress
        .descriptor
        .as_mut()
        .expect("original file retained");
    validate_output(file, private, progress.offset)?;
    verify_prefix(file, &bytes[..progress.offset])?;
    write_remaining(file, bytes, &mut progress.offset, |file, bytes, offset| {
        std::os::unix::fs::FileExt::write_at(file, bytes, offset)
    })?;
    if !progress.synced {
        file.sync_all().map_err(ExportError::Io)?;
        progress.synced = true;
    }
    validate_output(file, private, bytes.len())?;
    verify_prefix(file, bytes)?;
    let named = File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| ExportError::Io(e.into()))?,
    );
    if !same_file(
        &file.metadata().map_err(ExportError::Io)?,
        &named.metadata().map_err(ExportError::Io)?,
    ) {
        return Err(ExportError::Custody);
    }
    directory.file.sync_all().map_err(ExportError::Io)?;
    revalidate_directory(directory)?;
    progress.complete = true;
    Ok(())
}

fn write_remaining(
    file: &File,
    bytes: &[u8],
    offset: &mut usize,
    mut write: impl FnMut(&File, &[u8], u64) -> std::io::Result<usize>,
) -> std::result::Result<(), ExportError> {
    while *offset < bytes.len() {
        let at = u64::try_from(*offset).map_err(|_| ExportError::Phase)?;
        let wrote = write(file, &bytes[*offset..], at).map_err(ExportError::Io)?;
        if wrote == 0 {
            return Err(ExportError::Io(std::io::ErrorKind::WriteZero.into()));
        }
        if wrote > bytes.len() - *offset {
            return Err(ExportError::Phase);
        }
        *offset += wrote;
    }
    Ok(())
}
fn verify_prefix(file: &File, expected: &[u8]) -> std::result::Result<(), ExportError> {
    use std::os::unix::fs::FileExt as _;
    let mut scratch = Zeroizing::new([0u8; 4096]);
    let mut offset = 0;
    while offset < expected.len() {
        let length = (expected.len() - offset).min(scratch.len());
        let read = file
            .read_at(
                &mut scratch[..length],
                u64::try_from(offset).map_err(|_| ExportError::Phase)?,
            )
            .map_err(ExportError::Io)?;
        if read == 0 || scratch[..read] != expected[offset..offset + read] {
            return Err(ExportError::Custody);
        }
        offset += read;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
