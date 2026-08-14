//! Private confidential-spool ownership for the Phase-23 RNS-Link source.
//!
//! This adapter owns the two live writers and later the two immutable
//! snapshots.  It exposes neither file paths nor keys.  All identities below
//! are non-secret transcript bindings; only possession of a snapshot permits
//! authenticated reads.
use super::{
    SECRET_MAIN_FILE_BYTES_V1, SECRET_MAIN_PLAINTEXT_BYTES_V1, SECRET_MAIN_SLOT_COUNT_V1,
    SECRET_NONCE_FILE_BYTES_V1, SECRET_NONCE_PLAINTEXT_BYTES_V1, SECRET_NONCE_SLOT_COUNT_V1,
    SOURCE_VERSION_V1, ZkAmsMkheErrorV1,
};
use crate::vega::sponge::Keccak256;
use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};
use std::path::Path;
const WRITER_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source.spool-writer-identity";
const PROVIDER_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source.spool-provider-identity";
const SNAPSHOT_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source.spool-snapshot-identity";
const PUBLICATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.secret-source.spool-publication-identity";
fn nonzero_digest_v1(domain: &[u8], frames: &[[u8; 32]]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[SOURCE_VERSION_V1]);
    for frame in frames {
        hash.update(frame);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}
fn layout_frame_v1(layout: ConfidentialSpoolLayoutV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(&layout.slot_count_v1().to_be_bytes());
    hash.update(&layout.plaintext_len_v1().to_be_bytes());
    hash.update(&layout.ciphertext_record_len_v1().to_be_bytes());
    hash.update(&layout.file_len_v1().to_be_bytes());
    hash.finalize()
}
fn map_spool_error_v1(_: iroha_confidential_spool::ConfidentialSpoolErrorV1) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidPhase23Fold
}
/// Move-only dual writer.  Every write takes both owners before leaf preflight
/// or I/O and restores them only after complete success.
#[must_use = "dropping this writer discards both confidential spools"]
pub(super) struct RnsLinkSecretSpoolWriterV1 {
    live: Option<LiveRnsLinkSecretSpoolWriterV1>,
    context_digest: [u8; 32],
    geometry_digest: [u8; 32],
    mapping_digest: [u8; 32],
    main_context_digest: [u8; 32],
    nonce_context_digest: [u8; 32],
    writer_identity: [u8; 32],
}
struct LiveRnsLinkSecretSpoolWriterV1 {
    main: ConfidentialSpoolWriterV1,
    nonce: ConfidentialSpoolWriterV1,
}
impl RnsLinkSecretSpoolWriterV1 {
    pub(super) fn create_v1(
        directory: &Path,
        context_digest: [u8; 32],
        geometry_digest: [u8; 32],
        mapping_digest: [u8; 32],
        main_context_digest: [u8; 32],
        nonce_context_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let main_layout =
            ConfidentialSpoolLayoutV1::phase23_rns_link_secret_main_v1(main_context_digest)
                .map_err(map_spool_error_v1)?;
        let nonce_layout =
            ConfidentialSpoolLayoutV1::phase23_rns_link_secret_nonce_v1(nonce_context_digest)
                .map_err(map_spool_error_v1)?;
        if main_layout.slot_count_v1() != SECRET_MAIN_SLOT_COUNT_V1
            || main_layout.plaintext_len_v1() != SECRET_MAIN_PLAINTEXT_BYTES_V1
            || main_layout.file_len_v1() != SECRET_MAIN_FILE_BYTES_V1
            || nonce_layout.slot_count_v1() != SECRET_NONCE_SLOT_COUNT_V1
            || nonce_layout.plaintext_len_v1() != SECRET_NONCE_PLAINTEXT_BYTES_V1
            || nonce_layout.file_len_v1() != SECRET_NONCE_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Self::create_with_layouts_v1(
            directory,
            main_layout,
            nonce_layout,
            context_digest,
            geometry_digest,
            mapping_digest,
            main_context_digest,
            nonce_context_digest,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn create_with_layouts_v1(
        directory: &Path,
        main_layout: ConfidentialSpoolLayoutV1,
        nonce_layout: ConfidentialSpoolLayoutV1,
        context_digest: [u8; 32],
        geometry_digest: [u8; 32],
        mapping_digest: [u8; 32],
        main_context_digest: [u8; 32],
        nonce_context_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if [
            context_digest,
            geometry_digest,
            mapping_digest,
            main_context_digest,
            nonce_context_digest,
        ]
        .contains(&[0; 32])
            || main_context_digest == nonce_context_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let main_layout_frame = layout_frame_v1(main_layout);
        let nonce_layout_frame = layout_frame_v1(nonce_layout);
        let writer_identity = nonzero_digest_v1(
            WRITER_IDENTITY_DOMAIN_V1,
            &[
                context_digest,
                geometry_digest,
                mapping_digest,
                main_context_digest,
                nonce_context_digest,
                main_layout_frame,
                nonce_layout_frame,
            ],
        )?;
        let main = ConfidentialSpoolWriterV1::create_in_v1(directory, main_layout)
            .map_err(map_spool_error_v1)?;
        let nonce = ConfidentialSpoolWriterV1::create_in_v1(directory, nonce_layout)
            .map_err(map_spool_error_v1)?;
        Ok(Self {
            live: Some(LiveRnsLinkSecretSpoolWriterV1 { main, nonce }),
            context_digest,
            geometry_digest,
            mapping_digest,
            main_context_digest,
            nonce_context_digest,
            writer_identity,
        })
    }
    pub(super) const fn writer_identity_v1(&self) -> [u8; 32] {
        self.writer_identity
    }
    pub(super) fn write_main_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.main
            .write_slot_v1(slot, chunk)
            .map_err(map_spool_error_v1)?;
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn write_nonce_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let mut live = self
            .live
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        live.nonce
            .write_slot_v1(slot, chunk)
            .map_err(map_spool_error_v1)?;
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn seal_v1(
        self,
        ordered_record_topology_root: [u8; 32],
    ) -> Result<RnsLinkSecretSpoolSnapshotsV1, ZkAmsMkheErrorV1> {
        if ordered_record_topology_root == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let live = self.live.ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let main = live.main.seal_v1().map_err(map_spool_error_v1)?;
        let nonce = live.nonce.seal_v1().map_err(map_spool_error_v1)?;
        let main_snapshot_digest = *main.snapshot_digest_v1();
        let nonce_snapshot_digest = *nonce.snapshot_digest_v1();
        if main_snapshot_digest == [0; 32]
            || nonce_snapshot_digest == [0; 32]
            || main_snapshot_digest == nonce_snapshot_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let snapshot_identity = nonzero_digest_v1(
            SNAPSHOT_IDENTITY_DOMAIN_V1,
            &[
                self.writer_identity,
                self.main_context_digest,
                self.nonce_context_digest,
                main_snapshot_digest,
                nonce_snapshot_digest,
            ],
        )?;
        let provider_identity = nonzero_digest_v1(
            PROVIDER_IDENTITY_DOMAIN_V1,
            &[
                self.writer_identity,
                self.context_digest,
                self.mapping_digest,
                snapshot_identity,
            ],
        )?;
        let publication_identity = nonzero_digest_v1(
            PUBLICATION_IDENTITY_DOMAIN_V1,
            &[
                provider_identity,
                snapshot_identity,
                self.context_digest,
                self.geometry_digest,
                self.mapping_digest,
                ordered_record_topology_root,
            ],
        )?;
        let identities = [
            self.writer_identity,
            provider_identity,
            snapshot_identity,
            publication_identity,
        ];
        for (index, identity) in identities.iter().enumerate() {
            if *identity == [0; 32] || identities[..index].contains(identity) {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
        }
        Ok(RnsLinkSecretSpoolSnapshotsV1 {
            main,
            nonce,
            main_context_digest: self.main_context_digest,
            nonce_context_digest: self.nonce_context_digest,
            writer_identity: self.writer_identity,
            provider_identity,
            snapshot_identity,
            publication_identity,
            main_snapshot_digest,
            nonce_snapshot_digest,
        })
    }
}
/// Move-only owner of both immutable authenticated snapshots.
#[must_use = "dropping these snapshots closes both confidential sources"]
pub(super) struct RnsLinkSecretSpoolSnapshotsV1 {
    main: ConfidentialSpoolSnapshotV1,
    nonce: ConfidentialSpoolSnapshotV1,
    main_context_digest: [u8; 32],
    nonce_context_digest: [u8; 32],
    writer_identity: [u8; 32],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    publication_identity: [u8; 32],
    main_snapshot_digest: [u8; 32],
    nonce_snapshot_digest: [u8; 32],
}
impl RnsLinkSecretSpoolSnapshotsV1 {
    pub(super) const fn writer_identity_v1(&self) -> [u8; 32] {
        self.writer_identity
    }
    pub(super) const fn provider_identity_v1(&self) -> [u8; 32] {
        self.provider_identity
    }
    pub(super) const fn snapshot_identity_v1(&self) -> [u8; 32] {
        self.snapshot_identity
    }
    pub(super) const fn publication_identity_v1(&self) -> [u8; 32] {
        self.publication_identity
    }
    pub(super) const fn main_snapshot_digest_v1(&self) -> [u8; 32] {
        self.main_snapshot_digest
    }
    pub(super) const fn nonce_snapshot_digest_v1(&self) -> [u8; 32] {
        self.nonce_snapshot_digest
    }
    pub(super) const fn main_file_bytes_v1(&self) -> u64 {
        self.main.file_len_v1()
    }
    pub(super) const fn nonce_file_bytes_v1(&self) -> u64 {
        self.nonce.file_len_v1()
    }
    pub(super) fn read_main_v1(
        &mut self,
        slot: u64,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        self.main
            .read_slot_v1(slot, self.main_context_digest)
            .map_err(map_spool_error_v1)
    }
    pub(super) fn read_nonce_v1(
        &mut self,
        slot: u64,
    ) -> Result<ConfidentialSpoolChunkV1, ZkAmsMkheErrorV1> {
        self.nonce
            .read_slot_v1(slot, self.nonce_context_digest)
            .map_err(map_spool_error_v1)
    }
}
#[cfg(test)]
#[path = "phase23_rns_link_external_spool_tests.rs"]
mod tests;
