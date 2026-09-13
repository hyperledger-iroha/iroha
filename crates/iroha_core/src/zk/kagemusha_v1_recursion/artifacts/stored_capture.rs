//! Bounded capture of the original release-authenticated proving-key stream.
//!
//! The captured bytes are authenticated, but have not been decoded as a canonical proving key.
//! TODO: integrate a bounded structured-key decoder and original-key polynomial access before
//! replacing the dense production loaders. Capturing and then using the dense loader would not
//! repair its peak memory. The default memory resolver still retains its complete allocation.
//!
//! Capture owns one live 8 KiB plaintext chunk; the directory resolver separately buffers 64 KiB.
//! Ciphertext/AAD/key allocations, spool metadata, decoder buffers and co-resident artifacts are
//! additional costs. No process RSS, crypto secret-lifecycle or production claim follows here.

use std::{
    io::{self, Read as _, SeekFrom},
    path::Path,
};

use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};
use sha2::{Digest as _, Sha256};

use super::{
    DigestV1, KagemushaArtifactBindingV1, KagemushaArtifactByteResolverV1,
    KagemushaArtifactDescriptorV1, KagemushaArtifactErrorV1, KagemushaArtifactKindV1,
    KagemushaArtifactRoleV1, KagemushaAuthenticatedArtifactSetV1, KagemushaRecursionArtifactsV1,
};

const CHUNK_BYTES: u64 = 8 * 1024;
const CAPTURE_CONTEXT_DOMAIN: &[u8] = b"iroha.kagemusha.original-pk-capture.v1\0";

/// Original release-derived identities; construction is private to authenticated capture.
struct OriginalArtifact {
    recursion: KagemushaRecursionArtifactsV1,
    native_profile_digest: DigestV1,
    provider_policy_root: DigestV1,
    suite_id: DigestV1,
    vk_set_digest: DigestV1,
    binding: KagemushaArtifactBindingV1,
}

impl OriginalArtifact {
    fn from_set<R: KagemushaArtifactByteResolverV1>(
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
        role: KagemushaArtifactRoleV1,
    ) -> Result<Self, KagemushaArtifactErrorV1> {
        let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
        let binding = artifacts.binding(role);
        descriptor.validate_binding(binding)?;
        if descriptor.kind != KagemushaArtifactKindV1::ProvingKey {
            return Err(KagemushaArtifactErrorV1::InvalidBinding(role));
        }
        Ok(Self {
            recursion: artifacts.recursion_artifacts(),
            native_profile_digest: artifacts.native_profile_digest(),
            provider_policy_root: artifacts.provider_policy_root(),
            suite_id: artifacts.suite_id(),
            vk_set_digest: artifacts.vk_set_digest(),
            binding,
        })
    }

    fn context(&self, slots: u64) -> Result<DigestV1, KagemushaArtifactErrorV1> {
        // Norito's explicit header carries the layout flags. The release ID binds the complete
        // protocol inventory; retain that inventory itself in OriginalArtifact as well.
        let identity = (
            1_u8,
            (
                self.recursion.release_id,
                self.recursion.profile_digest,
                self.recursion.artifact_manifest_digest,
                self.native_profile_digest,
                self.provider_policy_root,
                self.suite_id,
                self.vk_set_digest,
            ),
            self.binding,
            (
                CHUNK_BYTES,
                slots,
                (self.binding.byte_len - 1) % CHUNK_BYTES + 1,
            ),
        );
        let encoded = norito::to_bytes(&identity)
            .map_err(|_| artifact_read_error(self.binding.role, "capture identity encoding"))?;
        let mut digest = Sha256::new();
        digest.update(CAPTURE_CONTEXT_DOMAIN);
        digest.update(encoded);
        Ok(digest.finalize().into())
    }

    fn matches<R: KagemushaArtifactByteResolverV1>(
        &self,
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    ) -> bool {
        self.recursion == artifacts.recursion_artifacts()
            && self.native_profile_digest == artifacts.native_profile_digest()
            && self.provider_policy_root == artifacts.provider_policy_root()
            && self.suite_id == artifacts.suite_id()
            && self.vk_set_digest == artifacts.vk_set_digest()
            && self.binding == artifacts.binding(self.binding.role)
    }
}

fn artifact_read_error(role: KagemushaArtifactRoleV1, reason: &str) -> KagemushaArtifactErrorV1 {
    KagemushaArtifactErrorV1::Read {
        role,
        reason: reason.to_owned(),
    }
}

fn spool_error(
    role: KagemushaArtifactRoleV1,
    error: ConfidentialSpoolErrorV1,
) -> KagemushaArtifactErrorV1 {
    // The existing spool error deliberately excludes directory paths and underlying OS text.
    artifact_read_error(role, &format!("captured artifact spool: {error}"))
}

/// Move-only authenticated bytes and their exact original release and role identity.
///
/// This is not a proving key. No constructor accepts a caller-supplied binding or file handle.
/// The resolver is not retained; an existing memory resolver's own allocation remains external.
pub(crate) struct CapturedProvingKeyArtifactV1 {
    original: OriginalArtifact,
    context: DigestV1,
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaAuthenticatedArtifactSetV1<R> {
    /// Capture one original proving-key stream without retaining a full encoded-file buffer.
    ///
    /// All bytes pass through the existing exact-length, EOF and digest authentication gate.
    /// The private directory and existing spool platform/lifecycle requirements still apply.
    ///
    /// # Errors
    /// Rejects a non-proving-key role, invalid geometry, source authentication failure or spool
    /// failure. Any error or unwind destroys the private capture before it can escape.
    pub(crate) fn capture_proving_key(
        &self,
        role: KagemushaArtifactRoleV1,
        directory: &Path,
    ) -> Result<CapturedProvingKeyArtifactV1, KagemushaArtifactErrorV1> {
        let original = OriginalArtifact::from_set(self, role)?;
        let length = original.binding.byte_len;
        let slots = length.div_ceil(CHUNK_BYTES);
        let context = original.context(slots)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(slots, CHUNK_BYTES, context)
            .map_err(|error| spool_error(role, error))?;
        let snapshot = self.read_verified(role, |reader| {
            let mut writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)
                .map_err(|error| spool_error(role, error))?;
            let mut remaining = length;
            for slot in 0..slots {
                // write_slot consumes this chunk and encrypts it in place. The next zeroed
                // chunk is allocated only after that previous owner has been destroyed.
                let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(CHUNK_BYTES)
                    .map_err(|error| spool_error(role, error))?;
                let count = usize::try_from(remaining.min(CHUNK_BYTES))
                    .map_err(|_| artifact_read_error(role, "capture chunk length overflow"))?;
                reader
                    .read_exact(&mut chunk.as_mut_slice_v1()[..count])
                    .map_err(|_| artifact_read_error(role, "capture source read failed"))?;
                writer
                    .write_slot_v1(slot, chunk)
                    .map_err(|error| spool_error(role, error))?;
                remaining -= u64::try_from(count)
                    .map_err(|_| artifact_read_error(role, "capture byte count overflow"))?;
            }
            writer.seal_v1().map_err(|error| spool_error(role, error))
        })?;
        Ok(CapturedProvingKeyArtifactV1 {
            original,
            context,
            snapshot: Some(snapshot),
        })
    }
}

impl CapturedProvingKeyArtifactV1 {
    /// Return the exact original authenticated binding; it does not establish key canonicality.
    pub(crate) const fn binding(&self) -> KagemushaArtifactBindingV1 {
        self.original.binding
    }

    /// Check the entire retained release-derived identity against the original artifact set.
    pub(crate) fn matches_artifact_set<R: KagemushaArtifactByteResolverV1>(
        &self,
        artifacts: &KagemushaAuthenticatedArtifactSetV1<R>,
    ) -> bool {
        self.snapshot.is_some() && self.original.matches(artifacts)
    }

    /// Read original bytes through a consuming callback, retaining at most one cached chunk.
    ///
    /// A successful callback returns this same owner. It must still separately establish all
    /// structured-key canonicality and original VK/profile checks before returning a key.
    /// A partial or random-access read authenticates only those bytes; it is not a full decoder.
    ///
    /// # Errors
    /// Callback errors, operational read errors and unwinds destroy the original capture. A
    /// callback cannot hide a fatal read error and recover an owner by returning success.
    pub(crate) fn with_reader<T, E>(
        mut self,
        consume: impl FnOnce(&mut CapturedProvingKeyReaderV1<'_>) -> Result<T, E>,
    ) -> Result<(Self, T), E>
    where
        E: From<KagemushaArtifactErrorV1>,
    {
        if self.snapshot.is_none() {
            return Err(E::from(artifact_read_error(
                self.original.binding.role,
                "captured artifact is poisoned",
            )));
        }
        let value = {
            let mut reader = CapturedProvingKeyReaderV1 {
                snapshot: &mut self.snapshot,
                context: self.context,
                binding: self.original.binding,
                position: 0,
                cached: None,
                failure: None,
            };
            let value = consume(&mut reader)?;
            if let Some(error) = reader.failure.take() {
                return Err(E::from(error));
            }
            if reader.snapshot.is_none() {
                return Err(E::from(artifact_read_error(
                    reader.binding.role,
                    "captured artifact is poisoned",
                )));
            }
            value
        };
        Ok((self, value))
    }
}

/// Bounded original-byte reader used only inside the captured owner's consuming callback.
///
/// Seeking may address the exact EOF but cannot expose zero padding in the final storage slot.
/// Pure invalid seeks do not change position. Operational failures latch and drop the snapshot.
pub(crate) struct CapturedProvingKeyReaderV1<'a> {
    snapshot: &'a mut Option<ConfidentialSpoolSnapshotV1>,
    context: DigestV1,
    binding: KagemushaArtifactBindingV1,
    position: u64,
    cached: Option<(u64, ConfidentialSpoolChunkV1)>,
    failure: Option<KagemushaArtifactErrorV1>,
}

impl CapturedProvingKeyReaderV1<'_> {
    fn poison(&mut self, error: KagemushaArtifactErrorV1) -> io::Error {
        self.cached = None;
        self.snapshot.take();
        self.failure.get_or_insert(error);
        io::Error::other("captured artifact read failed")
    }

    fn check_live(&self) -> io::Result<()> {
        if self.failure.is_some() || self.snapshot.is_none() {
            return Err(io::Error::other("captured artifact is poisoned"));
        }
        Ok(())
    }
}

impl io::Read for CapturedProvingKeyReaderV1<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.check_live()?;
        if output.is_empty() || self.position == self.binding.byte_len {
            return Ok(0);
        }
        let slot = self.position / CHUNK_BYTES;
        if self
            .cached
            .as_ref()
            .is_none_or(|(cached, _)| *cached != slot)
        {
            // Drop the old plaintext before allocating/authenticating a replacement chunk.
            self.cached = None;
            // Take the complete snapshot before the external read. A callback that catches
            // a read unwind still cannot recover an owner through the final presence gate.
            let mut snapshot = self
                .snapshot
                .take()
                .ok_or_else(|| io::Error::other("captured artifact is poisoned"))?;
            let result = snapshot.read_slot_v1(slot, self.context);
            let chunk =
                result.map_err(|error| self.poison(spool_error(self.binding.role, error)))?;
            *self.snapshot = Some(snapshot);
            self.cached = Some((slot, chunk));
        }
        let offset = usize::try_from(self.position % CHUNK_BYTES)
            .map_err(|_| io::Error::other("captured artifact chunk offset overflow"))?;
        let remaining = usize::try_from((self.binding.byte_len - self.position).min(CHUNK_BYTES))
            .map_err(|_| io::Error::other("captured artifact byte count overflow"))?;
        let count = output
            .len()
            .min(CHUNK_BYTES as usize - offset)
            .min(remaining);
        let chunk = &self
            .cached
            .as_ref()
            .ok_or_else(|| io::Error::other("captured artifact cache is absent"))?
            .1;
        output[..count].copy_from_slice(&chunk.as_slice_v1()[offset..offset + count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl io::Seek for CapturedProvingKeyReaderV1<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.check_live()?;
        let position = match position {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::End(offset) => i128::from(self.binding.byte_len) + i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
        };
        let position = u64::try_from(position)
            .ok()
            .filter(|position| *position <= self.binding.byte_len)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "capture seek out of range")
            })?;
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
#[path = "stored_capture_tests.rs"]
mod tests;
