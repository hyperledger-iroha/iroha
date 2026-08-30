//! Core-owned confidential-spool adapter for the replacement MKHE RNS source.
//!
//! Keeping this adapter in `iroha_core` preserves dependency direction: the
//! proof crate defines a backend-neutral move-only interface, while Core owns
//! both the proof crate and `iroha_crypto`'s already-locked XChaCha20-Poly1305
//! spool implementation.  No pathname, key, raw snapshot, or detached digest
//! constructor crosses the boundary.

use std::path::PathBuf;

use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolErrorV1, ConfidentialSpoolLayoutV1,
    ConfidentialSpoolSnapshotV1, ConfidentialSpoolWriterV1,
};
use iroha_zkp_halo2::vega::{
    ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSecretChunkV1,
    ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceErrorV1,
    ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceProviderV1,
    ZkAmsMkheRnsNativeSourceSnapshotV1, ZkAmsMkheRnsNativeSourceWriterV1,
};

/// Core-owned factory for unlinked authenticated RNS-native source arenas.
///
/// This owner deliberately implements neither `Clone` nor `Debug`: even a
/// temporary directory path is operational information, not proof metadata.
pub struct CoreZkAmsMkheRnsNativeSourceProviderV1 {
    directory: PathBuf,
}

impl CoreZkAmsMkheRnsNativeSourceProviderV1 {
    /// Bind the provider to a caller-selected private spool directory.
    ///
    /// Directory and Unix descriptor checks occur atomically when
    /// [`ZkAmsMkheRnsNativeSourceProviderV1::create`] is invoked.
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
}

/// Core-owned zeroizing secret record.
///
/// The inner owner is consumed directly by the crypto spool; no second
/// plaintext allocation is introduced by this adapter.
pub struct CoreZkAmsMkheRnsNativeSecretChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    inner: Option<ConfidentialSpoolChunkV1>,
}

impl CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn new(
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeSourceErrorV1> {
        let inner = ConfidentialSpoolChunkV1::new_zeroed_v1(arena.plaintext_bytes())
            .map_err(map_spool_error_v1)?;
        Ok(Self {
            arena,
            inner: Some(inner),
        })
    }

    fn into_inner(mut self) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheRnsNativeSourceErrorV1> {
        self.inner
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)
    }
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        self.inner
            .as_ref()
            .expect("live Core RNS-native secret chunk")
            .as_slice_v1()
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner
            .as_mut()
            .expect("live Core RNS-native secret chunk")
            .as_mut_slice_v1()
    }
}

impl Drop for CoreZkAmsMkheRnsNativeSecretChunkV1 {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.as_mut() {
            inner.as_mut_slice_v1().fill(0);
        }
    }
}

/// Core-owned pair of strict sequential confidential-spool writers.
pub struct CoreZkAmsMkheRnsNativeSourceWriterV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    main: Option<ConfidentialSpoolWriterV1>,
    nonce: Option<ConfidentialSpoolWriterV1>,
}

/// Core-owned pair of immutable authenticated confidential-spool snapshots.
pub struct CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    main: ConfidentialSpoolSnapshotV1,
    nonce: ConfidentialSpoolSnapshotV1,
}

impl ZkAmsMkheRnsNativeSourceProviderV1 for CoreZkAmsMkheRnsNativeSourceProviderV1 {
    type Writer = CoreZkAmsMkheRnsNativeSourceWriterV1;

    fn create(
        &self,
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    ) -> Result<Self::Writer, ZkAmsMkheRnsNativeSourceErrorV1> {
        layout.validate()?;
        let main_layout = spool_layout_v1(layout, ZkAmsMkheRnsNativeSourceArenaV1::Main)?;
        let nonce_layout = spool_layout_v1(layout, ZkAmsMkheRnsNativeSourceArenaV1::Nonce)?;
        let main = ConfidentialSpoolWriterV1::create_in_v1(&self.directory, main_layout)
            .map_err(map_spool_error_v1)?;
        let nonce = ConfidentialSpoolWriterV1::create_in_v1(&self.directory, nonce_layout)
            .map_err(map_spool_error_v1)?;
        Ok(CoreZkAmsMkheRnsNativeSourceWriterV1 {
            layout,
            main: Some(main),
            nonce: Some(nonce),
        })
    }
}

impl ZkAmsMkheRnsNativeSourceWriterV1 for CoreZkAmsMkheRnsNativeSourceWriterV1 {
    type Chunk = CoreZkAmsMkheRnsNativeSecretChunkV1;
    type Snapshot = CoreZkAmsMkheRnsNativeSourceSnapshotV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn allocate_chunk(
        &self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.main.is_none() || self.nonce.is_none() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        CoreZkAmsMkheRnsNativeSecretChunkV1::new(arena)
    }

    fn write_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
        chunk: Self::Chunk,
    ) -> Result<(), ZkAmsMkheRnsNativeSourceErrorV1> {
        if chunk.arena() != arena || chunk.as_slice().len() as u64 != arena.plaintext_bytes() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        let inner = chunk.into_inner()?;
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self
                .main
                .as_mut()
                .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
                .write_slot_v1(slot, inner)
                .map_err(map_spool_error_v1),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self
                .nonce
                .as_mut()
                .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
                .write_slot_v1(slot, inner)
                .map_err(map_spool_error_v1),
        }
    }

    fn seal(mut self) -> Result<Self::Snapshot, ZkAmsMkheRnsNativeSourceErrorV1> {
        let main = self
            .main
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
            .seal_v1()
            .map_err(map_spool_error_v1)?;
        let nonce = self
            .nonce
            .take()
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned)?
            .seal_v1()
            .map_err(map_spool_error_v1)?;
        Ok(CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
            layout: self.layout,
            main,
            nonce,
        })
    }
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for CoreZkAmsMkheRnsNativeSourceSnapshotV1 {
    type Chunk = CoreZkAmsMkheRnsNativeSecretChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => *self.main.snapshot_digest_v1(),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => *self.nonce.snapshot_digest_v1(),
        }
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        let expected_context = self.layout.arena_context_digest(arena);
        let inner = match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => self
                .main
                .read_slot_v1(slot, expected_context)
                .map_err(map_spool_error_v1)?,
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self
                .nonce
                .read_slot_v1(slot, expected_context)
                .map_err(map_spool_error_v1)?,
        };
        Ok(CoreZkAmsMkheRnsNativeSecretChunkV1 {
            arena,
            inner: Some(inner),
        })
    }
}

// The underlying confidential-spool snapshot is a seekable authenticated
// owner. Its focused tests exercise same-slot repeatability, descending reads,
// immutable snapshot identity, pure preflight errors, and permanent poisoning
// after every operational error or unwind.
impl ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1 for CoreZkAmsMkheRnsNativeSourceSnapshotV1 {}

fn spool_layout_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
) -> Result<ConfidentialSpoolLayoutV1, ZkAmsMkheRnsNativeSourceErrorV1> {
    ConfidentialSpoolLayoutV1::new_v1(
        arena.slot_count(),
        arena.plaintext_bytes(),
        layout.arena_context_digest(arena),
    )
    .map_err(map_spool_error_v1)
}

fn map_spool_error_v1(error: ConfidentialSpoolErrorV1) -> ZkAmsMkheRnsNativeSourceErrorV1 {
    match error {
        ConfidentialSpoolErrorV1::EmptyLayout
        | ConfidentialSpoolErrorV1::EmptyChunk
        | ConfidentialSpoolErrorV1::InertContextDigest => {
            ZkAmsMkheRnsNativeSourceErrorV1::InvalidLayout
        }
        ConfidentialSpoolErrorV1::GeometryOverflow
        | ConfidentialSpoolErrorV1::AddressSpaceExceeded
        | ConfidentialSpoolErrorV1::LimitExceeded(_)
        | ConfidentialSpoolErrorV1::CipherMessageLimit => {
            ZkAmsMkheRnsNativeSourceErrorV1::ResourceCeilingExceeded
        }
        ConfidentialSpoolErrorV1::Allocation(_) => ZkAmsMkheRnsNativeSourceErrorV1::Allocation,
        ConfidentialSpoolErrorV1::UnsupportedPlatform
        | ConfidentialSpoolErrorV1::EntropyUnavailable
        | ConfidentialSpoolErrorV1::WeakEntropy(_) => {
            ZkAmsMkheRnsNativeSourceErrorV1::BackendUnavailable
        }
        ConfidentialSpoolErrorV1::FileOperation { .. }
        | ConfidentialSpoolErrorV1::UnsafeTemporaryFile
        | ConfidentialSpoolErrorV1::TemporaryFileIdentityMismatch
        | ConfidentialSpoolErrorV1::UnsafeDetachedFile
        | ConfidentialSpoolErrorV1::FileLength { .. } => ZkAmsMkheRnsNativeSourceErrorV1::Storage,
        ConfidentialSpoolErrorV1::SlotOutOfRange { .. }
        | ConfidentialSpoolErrorV1::UnexpectedWriteSlot { .. }
        | ConfidentialSpoolErrorV1::ChunkLength { .. } => {
            ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite
        }
        ConfidentialSpoolErrorV1::ContextDigestMismatch
        | ConfidentialSpoolErrorV1::Encryption
        | ConfidentialSpoolErrorV1::Authentication => {
            ZkAmsMkheRnsNativeSourceErrorV1::Authentication
        }
        ConfidentialSpoolErrorV1::Incomplete { .. } => ZkAmsMkheRnsNativeSourceErrorV1::Incomplete,
        ConfidentialSpoolErrorV1::Poisoned => ZkAmsMkheRnsNativeSourceErrorV1::Poisoned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_zkp_halo2::vega::{
        zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_topology_v1,
    };

    fn source_layout() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
        let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            profile.profile_digest,
            topology.topology_digest,
            [0x51; 32],
            [0x52; 32],
            [0x53; 32],
        )
        .expect("source layout")
    }

    #[cfg(unix)]
    #[test]
    fn provider_uses_exact_arenas_and_rejects_cross_arena_chunks() {
        let directory = tempfile::tempdir().expect("private temporary directory");
        let provider = CoreZkAmsMkheRnsNativeSourceProviderV1::new(directory.path());
        let mut writer = provider.create(source_layout()).expect("source writer");

        let mut main = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
            .expect("main chunk");
        main.as_mut_slice()[0] = 7;
        writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0, main)
            .expect("first main write");

        let nonce = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
            .expect("nonce chunk");
        assert_eq!(
            writer.write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, nonce),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite)
        );
        let nonce = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
            .expect("replacement nonce chunk");
        writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0, nonce)
            .expect("first nonce write");
        assert!(matches!(
            writer.seal(),
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Incomplete)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn provider_preserves_exact_next_slot_and_path_free_errors() {
        let directory = tempfile::tempdir().expect("private temporary directory");
        let provider = CoreZkAmsMkheRnsNativeSourceProviderV1::new(directory.path());
        let mut writer = provider.create(source_layout()).expect("source writer");
        let chunk = writer
            .allocate_chunk(ZkAmsMkheRnsNativeSourceArenaV1::Main)
            .expect("main chunk");
        let error = writer
            .write_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 1, chunk)
            .expect_err("out-of-order write must fail");
        assert_eq!(error, ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        assert!(
            !error
                .to_string()
                .contains(directory.path().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn core_snapshot_exposes_the_repeatable_source_capability() {
        fn require_repeatable<S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1>() {}
        require_repeatable::<CoreZkAmsMkheRnsNativeSourceSnapshotV1>();
    }
}
