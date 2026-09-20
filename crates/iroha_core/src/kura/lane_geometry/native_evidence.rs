//! Pure Native geometry observation and explicit durability attestation.
//!
//! Observation authenticates bounded immutable evidence without syncing or
//! repairing files. The consuming attestation rechecks those exact identities.
//! Neither object is a geometry reservation or authority to publish State.

use super::*;
use crate::kura::StableSidecarMetadata;

type NativeGeometryEvidence = (
    BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1>,
    BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact>,
);

struct ObservedNativeAmxFile {
    path: PathBuf,
    metadata: StableSidecarMetadata,
    bytes_hash: Hash,
}

/// Read-only result bound to one Kura and one complete directory inventory.
/// Drop releases memory and the directory handle without performing effects.
pub(super) struct ObservedNativeAmxEvidence<'kura> {
    kura: &'kura Kura,
    directory: BoundProgressDirectory,
    inventory: &'kura BoundProgressDirectorySnapshot,
    manifests: BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1>,
    receipts: BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact>,
    files: Vec<ObservedNativeAmxFile>,
    payload_limit: usize,
    context: &'kura str,
}

impl Kura {
    /// Validate exact bounded Native evidence without recovery or durability writes.
    /// The caller retains the same sidecar/mutation guards as the geometry scan.
    pub(super) fn observe_geometry_native_amx_per_height_evidence<'observed>(
        &'observed self,
        lane_artifacts: &Path,
        artifact_snapshot: &'observed BoundProgressDirectorySnapshot,
        retained_record_limit: usize,
        context: &'observed str,
    ) -> Result<ObservedNativeAmxEvidence<'observed>> {
        let payload_limit = usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES)?;
        let mut manifests = BTreeMap::new();
        let mut receipts = BTreeMap::new();
        let mut evidence_bytes = 0_u64;
        let mut files = Vec::new();
        let directory = Self::open_bound_progress_directory(&self.store_root, lane_artifacts)?;
        if &self.geometry_bound_progress_directory_snapshot(
            &directory,
            artifact_snapshot.len(),
            context,
        )? != artifact_snapshot
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "Native AMX evidence namespace differs from its observed inventory",
            ));
        }
        for (raw_name, entry_snapshot) in artifact_snapshot {
            let path = lane_artifacts.join(raw_name);
            let Some((kind, lane_block_height, temporary)) =
                Self::parse_native_amx_evidence_path(&path)?
            else {
                continue;
            };
            if temporary {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence is still temporary"),
                    ),
                    path,
                ));
            }
            let retained_count = match kind {
                NativeAmxEvidenceKind::Manifest => manifests.len(),
                NativeAmxEvidenceKind::Receipt => receipts.len(),
            };
            if retained_count >= retained_record_limit {
                let evidence_kind = match kind {
                    NativeAmxEvidenceKind::Manifest => "manifest",
                    NativeAmxEvidenceKind::Receipt => "receipt",
                };
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "{context} Native AMX {evidence_kind} count exceeds configured retention"
                        ),
                    ),
                    path,
                ));
            }
            if entry_snapshot.kind != BoundProgressDirectoryEntryKind::File {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence is not a regular file"),
                    ),
                    path,
                ));
            }
            let metadata =
                Self::regular_sidecar_metadata_for(&self.store_root, &path, lane_artifacts)?
                    .ok_or_else(|| {
                        Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX evidence disappeared during validation"
                                ),
                            ),
                            path.clone(),
                        )
                    })?;
            let encoded_len = metadata.file.len();
            evidence_bytes = evidence_bytes.checked_add(encoded_len).ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence byte count overflows"),
                    ),
                    path.clone(),
                )
            })?;
            if evidence_bytes > self.native_amx_participant_evidence_file_bytes() {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "{context} Native AMX manifests and receipts exceed their shared aggregate byte bound"
                        ),
                    ),
                    path,
                ));
            }
            let before = self
                .read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!("{context} Native AMX evidence disappeared while reading"),
                        ),
                        path.clone(),
                    )
                })?;
            if !Self::stable_sidecar_metadata_unchanged(&metadata, &before.metadata) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed before decoding"),
                    ),
                    path,
                ));
            }
            match kind {
                NativeAmxEvidenceKind::Manifest => {
                    let artifact = norito::decode_from_bytes::<
                        NativeAmxParticipantApplicationManifestArtifactV1,
                    >(&before.bytes)
                    .map_err(Error::NoritoFrame)?;
                    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
                        || artifact.leaf.participant_height != lane_block_height
                        || Self::validate_native_amx_participant_application_manifest_artifact(
                            &artifact,
                        )
                        .is_err()
                        || manifests.insert(lane_block_height, artifact).is_some()
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX manifest is non-canonical, malformed, or duplicated"
                                ),
                            ),
                            path,
                        ));
                    }
                }
                NativeAmxEvidenceKind::Receipt => {
                    let artifact = norito::decode_from_bytes::<
                        NativeAmxParticipantApplicationReceiptArtifact,
                    >(&before.bytes)
                    .map_err(Error::NoritoFrame)?;
                    if norito::to_bytes(&artifact).map_err(Error::NoritoFrame)? != before.bytes
                        || artifact.participant_proposal.descriptor.lane_block_height
                            != lane_block_height
                        || Self::validate_native_amx_participant_application_receipt_artifact(
                            &artifact,
                        )
                        .is_err()
                        || receipts.insert(lane_block_height, artifact).is_some()
                    {
                        return Err(Error::IO(
                            std::io::Error::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "{context} Native AMX receipt is non-canonical, malformed, or duplicated"
                                ),
                            ),
                            path,
                        ));
                    }
                }
            }
            files.try_reserve(1).map_err(|_| {
                self.geometry_error(
                    ErrorKind::OutOfMemory,
                    "cannot retain bounded Native AMX evidence identities",
                )
            })?;
            files.push(ObservedNativeAmxFile {
                path,
                metadata: before.metadata,
                bytes_hash: before.bytes_hash,
            });
        }
        if &self.geometry_bound_progress_directory_snapshot(
            &directory,
            artifact_snapshot.len(),
            context,
        )? != artifact_snapshot
        {
            return Err(self.geometry_error(
                ErrorKind::InvalidData,
                "Native AMX evidence namespace changed while observing its inventory",
            ));
        }
        Ok(ObservedNativeAmxEvidence {
            kura: self,
            directory,
            inventory: artifact_snapshot,
            manifests,
            receipts,
            files,
            payload_limit,
            context,
        })
    }

    /// Explicit maintenance used by the current lock-scoped geometry path.
    /// TODO: join a full pure retirement observation and resource reservation
    /// before carrying admission across consensus or canonical block durability.
    pub(super) fn read_and_attest_geometry_native_amx_per_height_evidence(
        &self,
        lane_artifacts: &Path,
        artifact_snapshot: &BoundProgressDirectorySnapshot,
        retained_record_limit: usize,
        context: &str,
    ) -> Result<NativeGeometryEvidence> {
        self.observe_geometry_native_amx_per_height_evidence(
            lane_artifacts,
            artifact_snapshot,
            retained_record_limit,
            context,
        )?
        .attest()
    }
}

impl ObservedNativeAmxEvidence<'_> {
    /// Consume authenticated read evidence without asserting filesystem durability.
    /// This projection cannot authorize retirement or survive a mutation boundary.
    pub(super) fn into_observed(self) -> Result<NativeGeometryEvidence> {
        for observed in &self.files {
            let after = self
                .kura
                .read_regular_sidecar_snapshot(
                    &observed.path,
                    &self.directory.expected_path,
                    self.payload_limit,
                )?
                .ok_or_else(|| {
                    self.kura.geometry_error(
                        ErrorKind::InvalidData,
                        "Native AMX evidence disappeared after observation",
                    )
                })?;
            if after.bytes_hash != observed.bytes_hash
                || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata)
            {
                return Err(self.kura.geometry_error(
                    ErrorKind::InvalidData,
                    "Native AMX evidence changed after observation",
                ));
            }
        }
        if &self.kura.geometry_bound_progress_directory_snapshot(
            &self.directory,
            self.inventory.len(),
            self.context,
        )? != self.inventory
        {
            return Err(self.kura.geometry_error(
                ErrorKind::InvalidData,
                "Native AMX evidence namespace changed after observation",
            ));
        }
        Ok((self.manifests, self.receipts))
    }

    #[cfg(test)]
    pub(super) fn manifests(
        &self,
    ) -> &BTreeMap<u64, NativeAmxParticipantApplicationManifestArtifactV1> {
        &self.manifests
    }

    #[cfg(test)]
    pub(super) fn receipts(
        &self,
    ) -> &BTreeMap<u64, NativeAmxParticipantApplicationReceiptArtifact> {
        &self.receipts
    }

    /// Sync only the captured objects and reject any intervening substitution.
    /// No decoded evidence escapes as durable until the complete barrier succeeds.
    pub(super) fn attest(self) -> Result<NativeGeometryEvidence> {
        let Self {
            kura,
            directory,
            inventory,
            manifests,
            receipts,
            files,
            payload_limit,
            context,
        } = self;
        let lane_artifacts = &directory.expected_path;
        if &kura.geometry_bound_progress_directory_snapshot(&directory, inventory.len(), context)?
            != inventory
        {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "Native AMX evidence namespace changed before durability attestation",
            ));
        }
        for observed in files {
            let path = observed.path;
            let file = OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            let opened_metadata = secure_file_metadata::from_file(&file)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if !Kura::sidecar_file_metadata_unchanged(&observed.metadata.file, &opened_metadata) {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed before durability sync"),
                    ),
                    path,
                ));
            }
            file.sync_all()
                .map_err(|error| Error::IO(error, path.clone()))?;
            let after = kura
                .read_regular_sidecar_snapshot(&path, lane_artifacts, payload_limit)?
                .ok_or_else(|| {
                    Error::IO(
                        std::io::Error::new(
                            ErrorKind::InvalidData,
                            format!(
                                "{context} Native AMX evidence disappeared after durability sync"
                            ),
                        ),
                        path.clone(),
                    )
                })?;
            if after.bytes_hash != observed.bytes_hash
                || !Kura::stable_sidecar_metadata_unchanged(&observed.metadata, &after.metadata)
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("{context} Native AMX evidence changed during durability sync"),
                    ),
                    path,
                ));
            }
        }
        sync_dir(lane_artifacts).map_err(|error| Error::IO(error, lane_artifacts.to_path_buf()))?;
        if &kura.geometry_bound_progress_directory_snapshot(&directory, inventory.len(), context)?
            != inventory
        {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "Native AMX evidence namespace changed during durability attestation",
            ));
        }
        Ok((manifests, receipts))
    }
}
