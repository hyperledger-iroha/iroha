// Durable discovery of outstanding Native publication, independent of the
// current canonical tip and of whether any route payload has been published.
const NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY: &str = "native_amx_publication_index";
const NATIVE_AMX_PUBLICATION_INDEX_TEMP_PREFIX: &str = ".native-amx-publication-";
const MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS: usize =
    2 * iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES;
const MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES: usize = 2 * MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS;
const NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES: usize = 4_096;
const MAX_NATIVE_AMX_PUBLICATION_INDEX_BYTES: usize =
    MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES * NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxPublicationIndexFileKind {
    Stable { height: u64 },
    Temporary,
}

/// Shared by authoritative inventory and physical/evidence classification.
/// Stable names use exactly twenty decimal digits and 64 lowercase hex digits.
fn native_amx_publication_index_file_kind(name: &str) -> Option<NativeAmxPublicationIndexFileKind> {
    if let Some(suffix) = name.strip_prefix(NATIVE_AMX_PUBLICATION_INDEX_TEMP_PREFIX) {
        return (!suffix.is_empty()
            && suffix.len() <= 64
            && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .then_some(NativeAmxPublicationIndexFileKind::Temporary);
    }
    let stem = name.strip_suffix(".norito")?;
    let (height, digest) = stem.split_once('-')?;
    if height.len() != 20
        || !height.bytes().all(|byte| byte.is_ascii_digit())
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let height = height.parse::<u64>().ok()?;
    (height > 0).then_some(NativeAmxPublicationIndexFileKind::Stable { height })
}

/// Why this durable publication locator was admitted. A completed repair is
/// already canonically applied and can never use an uncommitted-write rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::kura::NativeAmxPublicationIndexOriginV1")]
enum NativeAmxPublicationIndexOriginV1 {
    CanonicalWrite,
    CompletedRepair,
}

/// In-memory view. The existing carrier type remains the sole runtime identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeAmxPublicationIndexRecord {
    carrier: NativeAmxPublicationCarrier,
    merge_entry_hash: Option<HashOf<MergeLedgerEntry>>,
    selection_marker: BlockStoreCommitMarker,
    origin: NativeAmxPublicationIndexOriginV1,
    replaced: Option<NativeAmxPublicationCarrier>,
}

/// Fixed-field wire envelope. Neither block bodies nor unbounded collections
/// are admitted into the locator. Compact bodies bind their exact sidecar hash.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::kura::NativeAmxPublicationIndexRecordV1")]
struct NativeAmxPublicationIndexRecordV1 {
    version: u32,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    executed_wire_hash: Hash,
    merge_entry_hash: Option<HashOf<MergeLedgerEntry>>,
    selection_marker: BlockStoreCommitMarker,
    origin: NativeAmxPublicationIndexOriginV1,
    replaced: Option<(u64, HashOf<BlockHeader>, Hash)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeAmxPublicationIndexResolution {
    /// Exact selected header AND complete wire bytes match this record.
    Committed,
    /// The exact recorded pre-admission marker and old body were selected.
    ProvenUncommitted,
    /// A later replacement/prune or another mismatch needs its own authenticated
    /// retirement proof. Absence or a different hash alone never grants removal.
    RequiresRetirementProof,
}

impl NativeAmxPublicationIndexRecord {
    fn validate(&self) -> std::result::Result<(), &'static str> {
        let before = &self.selection_marker;
        if self.carrier.height == 0
            || before.version != BlockStoreCommitMarker::VERSION
            || (before.count == 0) != before.tip_hash.is_none()
        {
            return Err("invalid Native publication index height or pre-admission marker");
        }
        if self.origin == NativeAmxPublicationIndexOriginV1::CompletedRepair {
            if self.replaced.is_some()
                || self.carrier.height > before.count
                || (self.carrier.height == before.count
                    && Some(self.carrier.block_hash) != before.tip_hash)
            {
                return Err("Native completed repair differs from its selected canonical marker");
            }
            return Ok(());
        }
        match self.replaced {
            None if before.count.checked_add(1) == Some(self.carrier.height) => {}
            Some(old)
                if old.height == self.carrier.height
                    && old.height == before.count
                    && Some(old.block_hash) == before.tip_hash
                    && old != self.carrier => {}
            _ => {
                return Err(
                    "Native publication index is neither an exact append nor a tip replacement",
                );
            }
        }
        Ok(())
    }

    fn wire(&self) -> NativeAmxPublicationIndexRecordV1 {
        NativeAmxPublicationIndexRecordV1 {
            version: 1,
            height: self.carrier.height,
            block_hash: self.carrier.block_hash,
            executed_wire_hash: self.carrier.executed_wire_hash,
            merge_entry_hash: self.merge_entry_hash,
            selection_marker: self.selection_marker.clone(),
            origin: self.origin,
            replaced: self
                .replaced
                .map(|old| (old.height, old.block_hash, old.executed_wire_hash)),
        }
    }

    fn encoded(&self) -> Result<Vec<u8>> {
        self.validate()
            .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
        let bytes = norito::encode_canonical(&self.wire())?;
        if bytes.len() > NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES {
            return Err(Error::PruneIntentConflict(
                "Native publication index record exceeds its byte bound".to_owned(),
            ));
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES {
            return Err(Error::PruneIntentConflict(
                "Native publication index frame exceeds its byte bound".to_owned(),
            ));
        }
        let limits = recovery_control_decode_limits_v1(u64::try_from(
            NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
        )?)?;
        let wire = norito::decode_canonical_with_limits::<NativeAmxPublicationIndexRecordV1>(
            bytes, limits,
        )
        .map_err(|error| Error::NoritoFrame(error.into()))?;
        if wire.version != 1 {
            return Err(Error::PruneIntentConflict(
                "unsupported Native publication index version".to_owned(),
            ));
        }
        let record = Self {
            carrier: NativeAmxPublicationCarrier {
                height: wire.height,
                block_hash: wire.block_hash,
                executed_wire_hash: wire.executed_wire_hash,
            },
            merge_entry_hash: wire.merge_entry_hash,
            selection_marker: wire.selection_marker,
            origin: wire.origin,
            replaced: wire
                .replaced
                .map(
                    |(height, block_hash, executed_wire_hash)| NativeAmxPublicationCarrier {
                        height,
                        block_hash,
                        executed_wire_hash,
                    },
                ),
        };
        if record.encoded()? != bytes {
            return Err(Error::PruneIntentConflict(
                "Native publication index frame is not exact canonical bytes".to_owned(),
            ));
        }
        Ok(record)
    }

    fn file_name(&self) -> Result<String> {
        let digest = Hash::new(self.encoded()?);
        let hex = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(format!("{:020}-{hex}.norito", self.carrier.height))
    }

    /// Caller supplies the selected resolved marker and exact selected body
    /// identity; this method never reads or promotes a canonical marker stage.
    fn classify_resolved_carrier(
        &self,
        selected_marker: &BlockStoreCommitMarker,
        selected_carrier: Option<NativeAmxPublicationCarrier>,
    ) -> Result<NativeAmxPublicationIndexResolution> {
        self.validate()
            .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
        validate_native_amx_publication_selected_marker(selected_marker)?;
        if selected_carrier.is_some_and(|selected| selected.height != self.carrier.height)
            || (self.carrier.height > selected_marker.count && selected_carrier.is_some())
            || (self.carrier.height == selected_marker.count
                && selected_carrier
                    .is_some_and(|selected| Some(selected.block_hash) != selected_marker.tip_hash))
        {
            return Err(Error::PruneIntentConflict(
                "selected Native publication carrier disagrees with resolved marker".to_owned(),
            ));
        }
        if selected_carrier == Some(self.carrier) {
            return Ok(NativeAmxPublicationIndexResolution::Committed);
        }
        if self.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite
            && selected_marker == &self.selection_marker
        {
            match self.replaced {
                None if selected_carrier.is_none() => {
                    return Ok(NativeAmxPublicationIndexResolution::ProvenUncommitted);
                }
                Some(old) if selected_carrier == Some(old) => {
                    return Ok(NativeAmxPublicationIndexResolution::ProvenUncommitted);
                }
                _ => {}
            }
        }
        Ok(NativeAmxPublicationIndexResolution::RequiresRetirementProof)
    }
}

fn validate_native_amx_publication_selected_marker(marker: &BlockStoreCommitMarker) -> Result<()> {
    if marker.version != BlockStoreCommitMarker::VERSION
        || (marker.count == 0) != marker.tip_hash.is_none()
    {
        return Err(Error::PruneIntentConflict(
            "Native publication classification received an invalid resolved marker".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct NativeAmxPublicationIndexFile {
    path: PathBuf,
    record: NativeAmxPublicationIndexRecord,
    temporary: bool,
    snapshot: StableSidecarRead,
}

/// A typed atomic-write temporary whose bounded bytes do not form a record.
/// It owns physical bytes only: never a carrier identity or a preservation pin.
#[derive(Debug)]
struct NativeAmxPublicationIndexOrphanTemporary {
    path: PathBuf,
    snapshot: StableSidecarRead,
}

#[derive(Debug, Default)]
struct NativeAmxPublicationIndexInventory {
    records: BTreeMap<NativeAmxPublicationCarrier, NativeAmxPublicationIndexRecord>,
    files: Vec<NativeAmxPublicationIndexFile>,
    orphan_temporaries: Vec<NativeAmxPublicationIndexOrphanTemporary>,
    physical_bytes: u64,
}

#[derive(Debug)]
struct NativeAmxPublicationIndexSelection {
    pins: BTreeMap<u64, HashOf<BlockHeader>>,
    /// Header-selected candidates still require complete wire/association checks.
    #[cfg(test)]
    selected_candidates: BTreeSet<NativeAmxPublicationCarrier>,
    /// Omitted pins are NOT retireable: only a separately proven old marker,
    /// exact replacement or completed prune may authorize their removal.
    #[cfg(test)]
    unresolved: BTreeSet<NativeAmxPublicationCarrier>,
}

impl NativeAmxPublicationIndexInventory {
    fn physical_file_count(&self) -> usize {
        self.files.len() + self.orphan_temporaries.len()
    }

    #[cfg(test)]
    fn carriers(&self) -> BTreeSet<NativeAmxPublicationCarrier> {
        self.records.keys().copied().collect()
    }

    /// A non-Native replacement still needs durable retirement ownership when
    /// it supersedes an indexed carrier. Ordinary appends never gain an index.
    /// This is only the locator eligibility check; prepare independently binds
    /// the old selected tip and its complete wire before creating a new record.
    fn permits_ordinary_replacement(
        &self,
        carrier: NativeAmxPublicationCarrier,
        replaced: Option<NativeAmxPublicationCarrier>,
    ) -> bool {
        self.records
            .get(&carrier)
            .is_some_and(|record| record.replaced.is_some())
            || replaced.is_some_and(|old| {
                old.height == carrier.height && old != carrier && self.records.contains_key(&old)
            })
    }

    /// Preserve only header identities selected by an independently resolved
    /// canonical marker/journal. Raw records, including uncommitted append and
    /// replacement candidates, remain in `records` for exact wire validation.
    /// This is a preservation pin set, never a completion/retirement authority.
    fn pins_for_resolved_marker<F>(
        &self,
        marker: &BlockStoreCommitMarker,
        selected_hash: F,
    ) -> Result<BTreeMap<u64, HashOf<BlockHeader>>>
    where
        F: FnMut(u64) -> Result<Option<HashOf<BlockHeader>>>,
    {
        Ok(self
            .selection_for_resolved_marker(marker, selected_hash)?
            .pins)
    }

    fn selection_for_resolved_marker<F>(
        &self,
        marker: &BlockStoreCommitMarker,
        mut selected_hash: F,
    ) -> Result<NativeAmxPublicationIndexSelection>
    where
        F: FnMut(u64) -> Result<Option<HashOf<BlockHeader>>>,
    {
        validate_native_amx_publication_selected_marker(marker)?;
        if marker.count > 0 && selected_hash(marker.count)? != marker.tip_hash {
            return Err(Error::PruneIntentConflict(
                "resolved Native publication pin marker differs from selected journal".to_owned(),
            ));
        }
        let mut pins = BTreeMap::new();
        #[cfg(test)]
        let mut selected_candidates = BTreeSet::new();
        #[cfg(test)]
        let mut unresolved = BTreeSet::new();
        for record in self.records.values() {
            let carrier = record.carrier;
            if carrier.height > marker.count {
                #[cfg(test)]
                unresolved.insert(carrier);
                continue;
            }
            let hash = selected_hash(carrier.height)?.ok_or_else(|| {
                Error::PruneIntentConflict(
                    "resolved canonical journal lacks a Native publication candidate height"
                        .to_owned(),
                )
            })?;
            if carrier.block_hash == hash {
                pins.insert(carrier.height, hash);
                #[cfg(test)]
                selected_candidates.insert(carrier);
            } else {
                #[cfg(test)]
                unresolved.insert(carrier);
            }
        }
        Ok(NativeAmxPublicationIndexSelection {
            pins,
            #[cfg(test)]
            selected_candidates,
            #[cfg(test)]
            unresolved,
        })
    }
}

#[derive(Debug)]
struct NativeAmxPublicationIndexPublication {
    record: NativeAmxPublicationIndexRecord,
    additional_bytes: u64,
    bytes: Vec<u8>,
}

impl Kura {
    fn native_amx_publication_index_directory_for(store_root: &Path) -> PathBuf {
        store_root.join(NATIVE_AMX_PUBLICATION_INDEX_DIRECTORY)
    }

    /// Strictly read-only. Complete owned temporaries participate in discovery
    /// but are never promoted, unlinked, or turned into unconditional body pins.
    /// Empty/partial owned temporaries remain raw physical inventory only.
    /// Cleanup requires the later committed-record reconstruction fence;
    /// malformed stable records and unsafe filesystem objects still fail.
    fn read_native_amx_publication_index_for_store(
        store_root: &Path,
    ) -> Result<NativeAmxPublicationIndexInventory> {
        let directory = Self::native_amx_publication_index_directory_for(store_root);
        let Some((_, before)) = Self::canonical_sidecar_directory_for(store_root, &directory)?
        else {
            return Ok(NativeAmxPublicationIndexInventory::default());
        };
        let mut inventory = NativeAmxPublicationIndexInventory::default();
        for entry in
            std::fs::read_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?
        {
            let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
            if inventory.physical_file_count() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "Native publication index exceeds its file bound",
                ));
            }
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_str().ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "Native publication index filename is not UTF-8",
                )
            })?;
            let kind = native_amx_publication_index_file_kind(name).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "Native publication index has an unowned filename",
                )
            })?;
            let temporary = kind == NativeAmxPublicationIndexFileKind::Temporary;
            let snapshot = Self::read_regular_sidecar_snapshot_for(
                store_root,
                &path,
                &directory,
                NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "Native publication index entry disappeared",
                )
            })?;
            inventory.physical_bytes = inventory
                .physical_bytes
                .checked_add(u64::try_from(snapshot.bytes.len())?)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.clone(),
                        "Native publication index byte sum overflows",
                    )
                })?;
            if inventory.physical_bytes > u64::try_from(MAX_NATIVE_AMX_PUBLICATION_INDEX_BYTES)? {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "Native publication index exceeds its total byte bound",
                ));
            }
            let record = match NativeAmxPublicationIndexRecord::decode(&snapshot.bytes) {
                Ok(record) => record,
                Err(_) if temporary => {
                    inventory
                        .orphan_temporaries
                        .push(NativeAmxPublicationIndexOrphanTemporary { path, snapshot });
                    continue;
                }
                Err(error) => return Err(error),
            };
            if !temporary && name != record.file_name()? {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "Native publication index filename differs from its exact record",
                ));
            }
            if let Some(existing) = inventory.records.get(&record.carrier) {
                if existing != &record {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "Native publication index duplicates a carrier with conflicting context",
                    ));
                }
            } else {
                if inventory.records.len() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "Native publication index exceeds its distinct carrier bound",
                    ));
                }
                inventory.records.insert(record.carrier, record.clone());
            }
            inventory.files.push(NativeAmxPublicationIndexFile {
                path,
                record,
                temporary,
                snapshot,
            });
        }
        let Some((_, after)) = Self::canonical_sidecar_directory_for(store_root, &directory)?
        else {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "Native publication index directory disappeared",
            ));
        };
        if !Self::sidecar_directory_metadata_unchanged(&before, &after) {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "Native publication index changed during inventory",
            ));
        }
        Ok(inventory)
    }

    /// Caller holds prune then canonical ownership. This captures the already
    /// resolved durable frontier before taking the lower-order sidecar lock.
    /// Exact retries reuse the original record, not the now-advanced marker.
    fn prepare_native_amx_publication_index(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        replaced: Option<&SignedBlock>,
    ) -> Result<NativeAmxPublicationIndexPublication> {
        self.prepare_native_amx_publication_index_with_origin(
            block,
            merge_entry,
            replaced,
            NativeAmxPublicationIndexOriginV1::CanonicalWrite,
        )
    }

    /// Caller holds prune and canonical serialization. Existing locators retain
    /// their original write identity. A new repair requires complete canonical
    /// wire authentication and the finalized checkpoint/manifest join; a lost
    /// index before WSV completion remains an error, never fresh admission.
    fn prepare_native_amx_repair_publication_index(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        evidence: &NativeAmxParticipantApplicationEvidencePlan,
        target_indices: &[usize],
    ) -> Result<NativeAmxPublicationIndexPublication> {
        let targets = target_indices.iter().copied().collect::<BTreeSet<_>>();
        if targets.is_empty()
            || targets.len() != target_indices.len()
            || targets
                .iter()
                .any(|index| *index >= evidence.artifacts.len())
        {
            return Err(Error::PruneIntentConflict(
                "Native completed repair has invalid exact target indices".to_owned(),
            ));
        }
        let carrier = Self::native_amx_publication_carrier(block)?;
        let retained_index = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        let retained_record = retained_index.records.get(&carrier);
        if retained_record.is_some_and(|record| {
            record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite
        }) {
            return self.prepare_native_amx_publication_index(block, merge_entry, None);
        }
        if let Some(record) = retained_record {
            record
                .validate()
                .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
            if record.carrier != carrier
                || record.merge_entry_hash != merge_entry.map(MergeLedgerEntry::canonical_hash)
                || record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair
            {
                return Err(Error::PruneIntentConflict(
                    "Native completed repair differs from its retained publication index"
                        .to_owned(),
                ));
            }
        }
        let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {
            Error::PruneIntentConflict("Native completed repair has zero height".to_owned())
        })?;
        let selected = self
            .read_block_body_under_prune_and_canonical_guards(height)?
            .ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native completed repair lacks its authenticated canonical body".to_owned(),
                )
            })?;
        if Self::native_amx_publication_carrier(&selected)? != carrier
            || evidence.application_block_height != carrier.height
            || evidence.application_block_hash != carrier.block_hash
            || evidence.executed_block_wire_hash != carrier.executed_wire_hash
            || evidence.artifacts.is_empty()
            || usize::try_from(evidence.manifest_leaf_count).ok() != Some(evidence.artifacts.len())
        {
            return Err(Error::PruneIntentConflict(
                "Native completed repair differs from its authenticated canonical source"
                    .to_owned(),
            ));
        }
        {
            let _geometry = self.lane_geometry_lock.lock();
            let _sidecar = self.sidecar_lock.lock();
            for (index, (manifest, receipt)) in evidence.artifacts.iter().enumerate() {
                if usize::try_from(manifest.leaf_index).ok() != Some(index)
                    || manifest.manifest_root != evidence.manifest_root
                    || manifest.manifest_leaf_count != evidence.manifest_leaf_count
                    || manifest.leaf.application_block_height != carrier.height
                    || manifest.leaf.application_block_hash != carrier.block_hash
                    || manifest.leaf.executed_block_wire_hash != carrier.executed_wire_hash
                    || HashOf::new(manifest) != receipt.manifest_artifact_hash
                    || manifest.finality_artifact_hash != evidence.finality_artifact_hash
                    || manifest.finality_artifact_hash != receipt.finality_artifact_hash
                    || Self::validate_native_amx_participant_application_receipt_artifact(receipt)
                        .is_err()
                    || !Self::native_amx_participant_receipt_matches_manifest_leaf(
                        receipt,
                        &manifest.leaf,
                    )
                    || !self
                        .native_amx_publication_wsv_join_is_complete_locked(manifest, receipt)?
                {
                    return Err(Error::PruneIntentConflict("Native completed repair lacks the exact finalized WSV checkpoint and manifest join".to_owned()));
                }
                // A partial State repair may inspect a non-target only through
                // the authenticated physical journal. It never relabels that
                // old route using the current live State geometry.
                let target = self.native_amx_reservation_physical_target_from_journal(
                    &receipt.participant_proposal.descriptor,
                )?;
                match self.native_amx_route_publication_capacity_at_target_locked(&target, manifest, receipt)? {
                    None if targets.contains(&index) => return Err(Error::PruneIntentConflict(
                        "Native completed repair target is older than an authenticated published frontier".to_owned(),
                    )),
                    // The existing planner returns None only after authenticating
                    // the later pair, finalized WSV join and completed cleanup.
                    None => {}
                    Some((_, capacity)) => {
                        self.require_native_amx_completed_repair_receipt_at_target_locked(
                            &target, receipt, retained_record.map(|record| (record, manifest)),
                        )?;
                        if !targets.contains(&index)
                            && (!capacity.outstanding_components.is_empty()
                                || capacity.prune_journal_bytes != 0
                                || capacity.physical_cleanup_pending)
                        {
                            return Err(Error::PruneIntentConflict(
                                "Native completed repair would omit another route's unfinished publication".to_owned(),
                            ));
                        }
                    }
                }
                self.require_native_amx_reservation_physical_target(&target)?;
            }
        }
        self.prepare_native_amx_publication_index_with_origin(
            block,
            merge_entry,
            None,
            NativeAmxPublicationIndexOriginV1::CompletedRepair,
        )
    }

    /// Callers hold prune, canonical, geometry and sidecar in order. WSV alone
    /// cannot replace an unfinished index; an exact stable receipt and latest
    /// pointer are retained proof of the prior completed route publication.
    /// Only an existing, independently authenticated repair locator can own
    /// exact publication temporaries; pristine admission remains temp-free.
    fn require_native_amx_completed_repair_receipt_at_target_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
        recovery: Option<(
            &NativeAmxPublicationIndexRecord,
            &NativeAmxParticipantApplicationManifestArtifactV1,
        )>,
    ) -> Result<()> {
        let descriptor = &receipt.participant_proposal.descriptor;
        self.require_active_lane_artifact(entry, descriptor)?;
        let namespace = self.native_amx_evidence_namespace_for_entry(entry)?;
        self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?;
        let inventory = self.inventory_native_amx_evidence_files_locked(&namespace, true)?;
        self.require_native_amx_completed_repair_receipt_with_inventory_locked(
            entry, receipt, recovery, &namespace, &inventory,
        )
    }
    fn require_native_amx_completed_repair_receipt_with_inventory_locked(
        &self,
        entry: &impl LaneArtifactStorageView,
        receipt: &NativeAmxParticipantApplicationReceiptArtifact,
        recovery: Option<(
            &NativeAmxPublicationIndexRecord,
            &NativeAmxParticipantApplicationManifestArtifactV1,
        )>,
        namespace: &BoundProgressNamespace,
        inventory: &NativeAmxEvidenceInventory,
    ) -> Result<()> {
        let descriptor = &receipt.participant_proposal.descriptor;
        let file = inventory.receipts.get(&descriptor.lane_block_height)
            .ok_or_else(|| Error::PruneIntentConflict("Native unfinished publication lacks its exact pending index and retained receipt".to_owned()))?;
        if self.decode_native_amx_receipt_file_locked(entry, &namespace, file)? != *receipt {
            return Err(Error::PruneIntentConflict(
                "Native completed repair lacks exact stable receipt custody".to_owned(),
            ));
        }
        if !inventory.temporaries.is_empty() {
            let Some((record, manifest)) = recovery else {
                return Err(Error::PruneIntentConflict(
                    "Native new completed repair cannot adopt unowned publication temporaries"
                        .to_owned(),
                ));
            };
            record
                .validate()
                .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
            if record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair
                || record.carrier.height != manifest.leaf.application_block_height
                || record.carrier.block_hash != manifest.leaf.application_block_hash
                || record.carrier.executed_wire_hash != manifest.leaf.executed_block_wire_hash
                || HashOf::new(manifest) != receipt.manifest_artifact_hash
                || manifest.finality_artifact_hash != receipt.finality_artifact_hash
                || !Self::native_amx_participant_receipt_matches_manifest_leaf(
                    receipt,
                    &manifest.leaf,
                )
            {
                return Err(Error::PruneIntentConflict(
                    "Native repair temporary lacks its exact retained index and artifact join"
                        .to_owned(),
                ));
            }
            for temporary in inventory.temporaries.values() {
                let expected = match temporary.kind {
                    NativeAmxEvidenceKind::Manifest => manifest.encode_framed()?,
                    NativeAmxEvidenceKind::Receipt => receipt.encode_framed()?,
                };
                // Inventory binds canonical filename, route and physical identity;
                // this read rechecks that same object, not a path-only replacement.
                if temporary.participant_height != descriptor.lane_block_height
                    || self.read_native_amx_evidence_file_bytes_locked(&namespace, temporary)?
                        != expected
                {
                    return Err(Error::PruneIntentConflict(
                        "Native repair temporary differs from its exact authenticated canonical artifact".to_owned(),
                    ));
                }
            }
        }
        let latest_path = Self::native_amx_participant_receipt_latest_index_path_for_entry(
            entry,
            &self.store_root,
        );
        self.require_native_amx_latest_index_temp_absent_locked(&namespace)?;
        let latest = self.decode_bound_native_amx_participant_receipt_latest_index_locked(
            entry,
            &latest_path,
            &namespace,
        )?;
        if latest
            != Some(NativeAmxParticipantReceiptLatestIndexV2::from_receipt(
                receipt,
            ))
        {
            return Err(Error::PruneIntentConflict(
                "Native completed repair lacks its exact published receipt pointer".to_owned(),
            ));
        }
        Ok(())
    }

    /// Reconstruct exact repair artifacts from the retained locator, selected
    /// canonical full wire, merge association and available finality. Neither
    /// temporary payloads nor live secondary State provide reconstruction authority.
    fn native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(
        &self,
        block: &SignedBlock,
        merge: Option<&MergeLedgerEntry>,
        record: &NativeAmxPublicationIndexRecord,
    ) -> Result<
        Vec<(
            NativeAmxParticipantApplicationManifestArtifactV1,
            NativeAmxParticipantApplicationReceiptArtifact,
        )>,
    > {
        let carrier = Self::native_amx_publication_carrier(block)?;
        record
            .validate()
            .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
        if record.origin != NativeAmxPublicationIndexOriginV1::CompletedRepair
            || record.carrier != carrier
            || record.merge_entry_hash != merge.map(MergeLedgerEntry::canonical_hash)
        {
            return Err(Error::PruneIntentConflict(
                "Native completed repair startup differs from its retained index".to_owned(),
            ));
        }
        let height = NonZeroUsize::new(usize::try_from(carrier.height)?).ok_or_else(|| {
            Error::PruneIntentConflict("Native completed repair has zero startup height".to_owned())
        })?;
        let selected = self
            .read_block_body_under_prune_and_canonical_guards(height)?
            .ok_or_else(|| {
                Error::PruneIntentConflict(
                    "Native completed repair startup lacks its authenticated canonical body"
                        .to_owned(),
                )
            })?;
        if Self::native_amx_publication_carrier(&selected)? != carrier {
            return Err(Error::PruneIntentConflict(
                "Native completed repair startup changed canonical executed wire".to_owned(),
            ));
        }
        let (_, finality, _) = self
            .v2_finality_artifact_with_archive_under_prune_and_canonical_guards(carrier.height)?
            .ok_or(Error::MissingV2FinalityArtifact {
                height: carrier.height,
            })?;
        let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, merge)
            .map_err(|error| Error::PruneIntentConflict(format!("Native completed repair startup manifest: {error}")))?;
        let artifacts =
            native_amx_participant_application_artifacts(&manifest, HashOf::new(&finality))
                .filter(|artifacts| !artifacts.is_empty())
                .ok_or_else(|| {
                    Error::PruneIntentConflict(
                        "Native completed repair startup has no exact artifact plan".to_owned(),
                    )
                })?;
        Ok(artifacts)
    }
    fn authenticate_native_amx_completed_repair_on_startup(
        &self,
        block: &SignedBlock,
        merge: Option<&MergeLedgerEntry>,
        record: &NativeAmxPublicationIndexRecord,
    ) -> Result<()> {
        let artifacts = self
            .native_amx_completed_repair_artifacts_under_prune_and_canonical_guards(
                block, merge, record,
            )?;
        let _geometry = self.lane_geometry_lock.lock();
        let _sidecar = self.sidecar_lock.lock();
        for (manifest, receipt) in artifacts {
            if !self.native_amx_publication_wsv_join_is_complete_locked(&manifest, &receipt)? {
                return Err(Error::PruneIntentConflict(
                    "Native completed repair startup lost its finalized WSV join".to_owned(),
                ));
            }
            let target = self.native_amx_reservation_physical_target_from_journal(
                &receipt.participant_proposal.descriptor,
            )?;
            // An independently authenticated later completed pointer is an
            // existing terminal proof, not authority to republish old evidence.
            if self
                .native_amx_route_publication_capacity_at_target_locked(
                    &target, &manifest, &receipt,
                )?
                .is_some()
            {
                self.require_native_amx_completed_repair_receipt_at_target_locked(
                    &target,
                    &receipt,
                    Some((record, &manifest)),
                )?;
            }
            self.require_native_amx_reservation_physical_target(&target)?;
        }
        Ok(())
    }

    fn prepare_native_amx_publication_index_with_origin(
        &self,
        block: &SignedBlock,
        merge_entry: Option<&MergeLedgerEntry>,
        replaced: Option<&SignedBlock>,
        origin: NativeAmxPublicationIndexOriginV1,
    ) -> Result<NativeAmxPublicationIndexPublication> {
        let carrier = Self::native_amx_publication_carrier(block)?;
        let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, merge_entry)
            .map_err(|error| Error::PruneIntentConflict(format!("Native publication index manifest is invalid: {error}")))?;
        let replaced_carrier = replaced
            .map(Self::native_amx_publication_carrier)
            .transpose()?;
        let merge_entry_hash = match (Self::block_merge_reference(block), merge_entry) {
            (Some(reference), Some(entry)) if reference.matches_entry(entry) => {
                Some(entry.canonical_hash())
            }
            (None, None) => None,
            _ => {
                return Err(Error::PruneIntentConflict(
                    "Native publication index compact association differs from carrier".to_owned(),
                ));
            }
        };
        let selection_marker = {
            let mut store = self.block_store.lock();
            let count = store.read_exact_durable_index_count()?;
            store.commit_marker_for_count(count)?
        };
        // Capture before the lower-order sidecar lock. A header match alone
        // cannot prove replacement of the exact old executed carrier.
        let selected_replacement_matches = if let Some(old) = replaced_carrier {
            if old.height != selection_marker.count
                || Some(old.block_hash) != selection_marker.tip_hash
            {
                false
            } else {
                let height = NonZeroUsize::new(usize::try_from(old.height)?).ok_or_else(|| {
                    Error::PruneIntentConflict("Native index replacement height is zero".to_owned())
                })?;
                let selected = self
                    .get_block_without_merge_sidecar(height)
                    .ok_or_else(|| {
                        Error::PruneIntentConflict(
                            "Native index replacement lacks its selected complete old body"
                                .to_owned(),
                        )
                    })?;
                Self::native_amx_publication_carrier(&selected)? == old
            }
        } else {
            false
        };
        let _sidecar = self.sidecar_lock.lock();
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        if manifest.count() == 0
            && !inventory.permits_ordinary_replacement(carrier, replaced_carrier)
        {
            return Err(Error::PruneIntentConflict("ordinary block has neither an exact pending Native replacement nor retirement retry".to_owned()));
        }
        let record = if let Some(existing) = inventory.records.get(&carrier) {
            if existing.merge_entry_hash != merge_entry_hash {
                return Err(Error::PruneIntentConflict(
                    "Native publication retry changed its compact association".to_owned(),
                ));
            }
            existing.clone()
        } else {
            if inventory.records.len() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS {
                return Err(Error::PruneIntentConflict(
                    "Native publication index carrier capacity is full".to_owned(),
                ));
            }
            if replaced_carrier.is_some() && !selected_replacement_matches {
                return Err(Error::PruneIntentConflict(
                    "Native publication replacement differs from the selected complete old tip"
                        .to_owned(),
                ));
            }
            let record = NativeAmxPublicationIndexRecord {
                carrier,
                merge_entry_hash,
                selection_marker,
                origin,
                replaced: replaced_carrier,
            };
            record
                .validate()
                .map_err(|message| Error::PruneIntentConflict(message.to_owned()))?;
            if record.origin == NativeAmxPublicationIndexOriginV1::CanonicalWrite
                && record.replaced.is_none()
                && block.header().prev_block_hash() != record.selection_marker.tip_hash
            {
                return Err(Error::PruneIntentConflict(
                    "Native publication append does not extend its exact pre-admission marker"
                        .to_owned(),
                ));
            }
            record
        };
        let bytes = record.encoded()?;
        let existing = inventory
            .files
            .iter()
            .any(|file| !file.temporary && file.record == record);
        if !existing && inventory.physical_file_count() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES {
            return Err(Error::PruneIntentConflict(
                "Native publication index has no physical stage slot".to_owned(),
            ));
        }
        Ok(NativeAmxPublicationIndexPublication {
            additional_bytes: if existing {
                0
            } else {
                u64::try_from(bytes.len())?
            },
            record,
            bytes,
        })
    }

    fn ensure_native_amx_publication_index_directory_locked(&self) -> Result<PathBuf> {
        let directory = Self::native_amx_publication_index_directory_for(&self.store_root);
        let (_, root_before) =
            Self::canonical_sidecar_directory_for(&self.store_root, &self.store_root)?.ok_or_else(
                || {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "Native publication index root is absent",
                    )
                },
            )?;
        if Self::canonical_sidecar_directory_for(&self.store_root, &directory)?.is_none() {
            std::fs::create_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
        }
        let (_, root_after) =
            Self::canonical_sidecar_directory_for(&self.store_root, &self.store_root)?.ok_or_else(
                || {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "Native publication index root disappeared",
                    )
                },
            )?;
        if !Self::sidecar_metadata_same_object(&root_before, &root_after) {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "Native publication index root changed during directory admission",
            ));
        }
        Self::canonical_sidecar_directory_for(&self.store_root, &directory)?.ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                directory.clone(),
                "Native publication index directory disappeared",
            )
        })?;
        // Repeat on an existing directory: creation may previously have reached
        // disk before its parent fsync failed.
        sync_dir(&self.store_root).map_err(|error| Error::IO(error, self.store_root.clone()))?;
        Ok(directory)
    }

    /// Marker bytes must already belong to the caller's capacity reservation.
    /// Caller disables automatic reservation rollback BEFORE calling; any error
    /// may leave a synced temporary or published record and keeps ownership.
    fn publish_native_amx_publication_index(
        &self,
        publication: &NativeAmxPublicationIndexPublication,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        let _sidecar = self.sidecar_lock.lock();
        if publication.bytes != publication.record.encoded()? {
            return Err(Error::PruneIntentConflict(
                "prepared Native publication index changed".to_owned(),
            ));
        }
        let resource_directory = Self::native_amx_publication_index_directory_for(&self.store_root);
        // This bounded namespace includes typed temporary paths whose names
        // are chosen inside the atomic writer, plus first directory creation.
        let accounting = self
            .begin_total_disk_usage_mutation()
            .with_startup_resource_tree(&resource_directory);
        let directory = self.ensure_native_amx_publication_index_directory_locked()?;
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        if inventory
            .records
            .get(&publication.record.carrier)
            .is_some_and(|record| record != &publication.record)
        {
            return Err(Error::PruneIntentConflict(
                "Native publication index context changed after preparation".to_owned(),
            ));
        }
        let path = directory.join(publication.record.file_name()?);
        let existing = inventory
            .files
            .iter()
            .find(|file| !file.temporary && file.path == path);
        let actual_addition = if existing.is_some() {
            0
        } else {
            u64::try_from(publication.bytes.len())?
        };
        if actual_addition > publication.additional_bytes
            || (existing.is_none()
                && inventory.physical_file_count() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES)
            || (!inventory.records.contains_key(&publication.record.carrier)
                && inventory.records.len() >= MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS)
        {
            return Err(Error::PruneIntentConflict(
                "Native publication index exceeds its prepared capacity".to_owned(),
            ));
        }
        if existing.is_none()
            && !self.write_atomic_synced_impl_with_prefix(
                &path,
                &publication.bytes,
                false,
                NATIVE_AMX_PUBLICATION_INDEX_TEMP_PREFIX,
            )?
        {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native publication index no-clobber publication raced",
            ));
        }
        let current = Self::read_regular_sidecar_snapshot_for(
            &self.store_root,
            &path,
            &directory,
            NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
        )?
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.clone(),
                "Native publication index disappeared after publication",
            )
        })?;
        if current.bytes != publication.bytes {
            return Err(Self::invalid_lane_artifact_error(
                path,
                "Native publication index publication has different bytes",
            ));
        }
        if let Some(existing) = existing {
            if !Self::stable_sidecar_file_binding_unchanged(
                &existing.snapshot.metadata,
                &current.metadata,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "Native publication exact retry changed its admitted file",
                ));
            }
        }
        self.sync_native_amx_publication_index_file_locked(&path, &directory, &current)?;
        self.update_disk_usage_delta(0, actual_addition);
        accounting.finish();
        Ok(())
    }

    fn sync_native_amx_publication_index_file_locked(
        &self,
        path: &Path,
        directory: &Path,
        expected: &StableSidecarRead,
    ) -> Result<()> {
        let file =
            std::fs::File::open(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let opened = secure_file_metadata::from_file(&file)
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        if !Self::sidecar_file_metadata_unchanged(&expected.metadata.file, &opened) {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "Native publication sync opened a different file",
            ));
        }
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        sync_dir(directory).map_err(|error| Error::IO(error, directory.to_path_buf()))?;
        let after = Self::read_regular_sidecar_snapshot_for(
            &self.store_root,
            path,
            directory,
            NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
        )?
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "Native publication record disappeared across durability barrier",
            )
        })?;
        if after.bytes != expected.bytes
            || !Self::stable_sidecar_file_binding_unchanged(&expected.metadata, &after.metadata)
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "Native publication record changed across durability barrier",
            ));
        }
        Ok(())
    }

    /// Caller holds prune/canonical and has reconstructed every committed
    /// record under the recovery fence. An interrupted write cannot authorize
    /// canonical mutation: only complete stable publication crosses that gate.
    /// This removes raw orphan temporaries only, without promotion or pins.
    fn cleanup_native_amx_publication_index_orphan_temporaries(
        &self,
        inventory: &NativeAmxPublicationIndexInventory,
    ) -> Result<()> {
        let _sidecar = self.sidecar_lock.lock();
        self.cleanup_native_amx_publication_index_orphan_temporaries_locked(inventory)
    }

    /// Same recovery-fence contract, with sidecar ownership already held.
    fn cleanup_native_amx_publication_index_orphan_temporaries_locked(
        &self,
        inventory: &NativeAmxPublicationIndexInventory,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        if inventory.physical_file_count() > MAX_NATIVE_AMX_PUBLICATION_INDEX_FILES {
            return Err(Error::PruneIntentConflict(
                "Native orphan cleanup exceeds its inventory bound".to_owned(),
            ));
        }
        if inventory.orphan_temporaries.is_empty() {
            return Ok(());
        }
        let directory = Self::native_amx_publication_index_directory_for(&self.store_root);
        let (_, directory_metadata) =
            Self::canonical_sidecar_directory_for(&self.store_root, &directory)?.ok_or_else(
                || {
                    Self::invalid_lane_artifact_error(
                        directory.clone(),
                        "Native orphan namespace disappeared",
                    )
                },
            )?;
        let mut present = Vec::new();
        let mut unique = BTreeSet::new();
        for orphan in &inventory.orphan_temporaries {
            let name = orphan.path.file_name().and_then(|name| name.to_str());
            if orphan.path.parent() != Some(directory.as_path())
                || name.and_then(native_amx_publication_index_file_kind)
                    != Some(NativeAmxPublicationIndexFileKind::Temporary)
                || !unique.insert(orphan.path.clone())
                || orphan.snapshot.bytes.len() > NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES
                || NativeAmxPublicationIndexRecord::decode(&orphan.snapshot.bytes).is_ok()
                || !Self::sidecar_metadata_same_object(
                    &orphan.snapshot.metadata.directory,
                    &directory_metadata,
                )
            {
                return Err(Self::invalid_lane_artifact_error(
                    orphan.path.clone(),
                    "Native orphan cleanup lacks an exact raw temporary binding",
                ));
            }
            let Some(current) = Self::read_regular_sidecar_snapshot_for(
                &self.store_root,
                &orphan.path,
                &directory,
                NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
            )?
            else {
                // A previous unlink may have reached disk before parent fsync
                // failed. Repeat the barrier below, without claiming healing
                // of a faulted physical inventory or subtracting bytes twice.
                continue;
            };
            if current.bytes != orphan.snapshot.bytes
                || !Self::stable_sidecar_file_binding_unchanged(
                    &orphan.snapshot.metadata,
                    &current.metadata,
                )
            {
                return Err(Self::invalid_lane_artifact_error(
                    orphan.path.clone(),
                    "Native orphan changed since read-only reconstruction",
                ));
            }
            present.push(orphan);
        }
        if present.is_empty() {
            sync_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
            return Ok(());
        }
        // The namespace has its own bounded inventory. A legitimate crash
        // population can exceed the generic 48-path mutation scope.
        let accounting = self
            .begin_total_disk_usage_mutation()
            .with_startup_resource_tree(&directory);
        let mut removed = 0_u64;
        for orphan in present {
            let current = Self::read_regular_sidecar_snapshot_for(
                &self.store_root,
                &orphan.path,
                &directory,
                NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    orphan.path.clone(),
                    "Native orphan disappeared before exact unlink",
                )
            })?;
            if current.bytes != orphan.snapshot.bytes
                || !Self::stable_sidecar_file_binding_unchanged(
                    &orphan.snapshot.metadata,
                    &current.metadata,
                )
            {
                return Err(Self::invalid_lane_artifact_error(
                    orphan.path.clone(),
                    "Native orphan exact unlink encountered a replaced object",
                ));
            }
            std::fs::remove_file(&orphan.path)
                .map_err(|error| Error::IO(error, orphan.path.clone()))?;
            sync_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
            let (_, after) = Self::canonical_sidecar_directory_for(&self.store_root, &directory)?
                .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "Native orphan cleanup namespace disappeared",
                )
            })?;
            if !Self::sidecar_metadata_same_object(&current.metadata.directory, &after) {
                return Err(Self::invalid_lane_artifact_error(
                    orphan.path.clone(),
                    "Native orphan cleanup namespace changed",
                ));
            }
            match std::fs::symlink_metadata(&orphan.path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, orphan.path.clone())),
                Ok(_) => {
                    return Err(Self::invalid_lane_artifact_error(
                        orphan.path.clone(),
                        "Native orphan unlink path was replaced",
                    ));
                }
            }
            removed = removed
                .checked_add(u64::try_from(current.bytes.len())?)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        orphan.path.clone(),
                        "Native orphan byte sum overflows",
                    )
                })?;
        }
        self.update_disk_usage_delta(removed, 0);
        accounting.finish();
        Ok(())
    }

    /// Caller holds prune/canonical and has ALREADY authenticated completion,
    /// exact uncommitted selection, or replacement/prune retirement. This
    /// primitive performs no semantic inference from missing files or hashes.
    /// It must finish successfully before releasing any resident reservation.
    fn remove_native_amx_publication_index_exact(
        &self,
        record: &NativeAmxPublicationIndexRecord,
    ) -> Result<()> {
        let _sidecar = self.sidecar_lock.lock();
        self.remove_native_amx_publication_index_exact_locked(record)
    }

    /// Same retirement contract; caller additionally already holds sidecar
    /// ownership (for post-WSV route completion). Never locks the Native map.
    fn remove_native_amx_publication_index_exact_locked(
        &self,
        record: &NativeAmxPublicationIndexRecord,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        let expected_bytes = record.encoded()?;
        let directory = Self::native_amx_publication_index_directory_for(&self.store_root);
        let inventory = Self::read_native_amx_publication_index_for_store(&self.store_root)?;
        if inventory
            .records
            .get(&record.carrier)
            .is_some_and(|stored| stored != record)
        {
            return Err(Error::PruneIntentConflict(
                "Native publication retirement changed the exact record context".to_owned(),
            ));
        }
        let files = inventory
            .files
            .iter()
            .filter(|file| &file.record == record)
            .collect::<Vec<_>>();
        if files.is_empty() {
            // A prior unlink may have succeeded but parent sync failed. Retry
            // the barrier even when no pathname remains; absence is no proof.
            // This does not heal a faulted physical inventory. Its independent
            // explicit epoch-bound reconciliation is still required.
            if Self::canonical_sidecar_directory_for(&self.store_root, &directory)?.is_some() {
                sync_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
            }
            sync_dir(&self.store_root)
                .map_err(|error| Error::IO(error, self.store_root.clone()))?;
            return Ok(());
        }
        let accounting = self
            .begin_total_disk_usage_mutation()
            .with_startup_resource_tree(&directory);
        let mut removed = 0_u64;
        for file in files {
            let current = Self::read_regular_sidecar_snapshot_for(
                &self.store_root,
                &file.path,
                &directory,
                NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES,
            )?
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    file.path.clone(),
                    "Native publication record disappeared before exact unlink",
                )
            })?;
            if current.bytes != expected_bytes
                || !Self::stable_sidecar_file_binding_unchanged(
                    &file.snapshot.metadata,
                    &current.metadata,
                )
            {
                return Err(Self::invalid_lane_artifact_error(
                    file.path.clone(),
                    "Native publication retirement encountered a replaced record",
                ));
            }
            std::fs::remove_file(&file.path)
                .map_err(|error| Error::IO(error, file.path.clone()))?;
            sync_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
            let (_, after_directory) =
                Self::canonical_sidecar_directory_for(&self.store_root, &directory)?.ok_or_else(
                    || {
                        Self::invalid_lane_artifact_error(
                            directory.clone(),
                            "Native publication retirement directory disappeared",
                        )
                    },
                )?;
            if !Self::sidecar_metadata_same_object(&current.metadata.directory, &after_directory) {
                return Err(Self::invalid_lane_artifact_error(
                    file.path.clone(),
                    "Native publication exact unlink changed namespace or was replaced",
                ));
            }
            match std::fs::symlink_metadata(&file.path) {
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::IO(error, file.path.clone())),
                Ok(_) => {
                    return Err(Self::invalid_lane_artifact_error(
                        file.path.clone(),
                        "Native publication exact unlink path was replaced",
                    ));
                }
            }
            removed = removed
                .checked_add(u64::try_from(current.bytes.len())?)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        file.path.clone(),
                        "Native publication retired byte sum overflows",
                    )
                })?;
        }
        self.update_disk_usage_delta(removed, 0);
        accounting.finish();
        Ok(())
    }
}

#[cfg(test)]
mod native_amx_publication_index_tests {
    use super::*;

    fn carrier(height: u64, header: u8, wire: u8) -> NativeAmxPublicationCarrier {
        NativeAmxPublicationCarrier {
            height,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([header])),
            executed_wire_hash: Hash::new([wire]),
        }
    }

    fn initial() -> NativeAmxPublicationIndexRecord {
        NativeAmxPublicationIndexRecord {
            carrier: carrier(1, 11, 12),
            merge_entry_hash: None,
            selection_marker: BlockStoreCommitMarker::new(0, None),
            origin: NativeAmxPublicationIndexOriginV1::CanonicalWrite,
            replaced: None,
        }
    }

    fn inventory(
        records: Vec<NativeAmxPublicationIndexRecord>,
    ) -> NativeAmxPublicationIndexInventory {
        NativeAmxPublicationIndexInventory {
            records: records
                .into_iter()
                .map(|record| (record.carrier, record))
                .collect(),
            ..NativeAmxPublicationIndexInventory::default()
        }
    }

    #[test]
    fn exact_wire_roundtrip_rejects_extra_bytes_and_version() {
        let mut record = initial();
        record.merge_entry_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"compact association",
        )));
        let bytes = record.encoded().expect("bounded index frame");
        assert_eq!(
            NativeAmxPublicationIndexRecord::decode(&bytes).expect("exact frame"),
            record
        );
        let mut extra = bytes;
        extra.push(0);
        assert!(NativeAmxPublicationIndexRecord::decode(&extra).is_err());
        let mut wire = record.wire();
        wire.version = 2;
        assert!(
            NativeAmxPublicationIndexRecord::decode(
                &norito::encode_canonical(&wire).expect("wrong version frame")
            )
            .is_err()
        );
    }

    #[test]
    fn record_requires_exact_append_or_tip_replacement() {
        let mut record = initial();
        record.carrier.height = 2;
        assert!(record.validate().is_err());
        record.selection_marker =
            BlockStoreCommitMarker::new(1, Some(carrier(1, 11, 12).block_hash));
        assert!(record.validate().is_ok());
        record.selection_marker =
            BlockStoreCommitMarker::new(u64::MAX, Some(record.carrier.block_hash));
        assert!(record.validate().is_err());
        let old = carrier(1, 11, 12);
        record.carrier = carrier(1, 21, 22);
        record.selection_marker = BlockStoreCommitMarker::new(1, Some(old.block_hash));
        record.replaced = Some(old);
        assert!(record.validate().is_ok());
        record.replaced = Some(record.carrier);
        assert!(record.validate().is_err());
    }

    #[test]
    fn completed_repair_roundtrip_preserves_earlier_carrier_below_ordinary_tip() {
        let native = carrier(1, 11, 12);
        let later = carrier(2, 31, 32);
        let marker = BlockStoreCommitMarker::new(2, Some(later.block_hash));
        let record = NativeAmxPublicationIndexRecord {
            selection_marker: marker.clone(),
            origin: NativeAmxPublicationIndexOriginV1::CompletedRepair,
            ..initial()
        };
        let bytes = record
            .encoded()
            .expect("committed repair has an exact bounded frame");
        assert_eq!(
            NativeAmxPublicationIndexRecord::decode(&bytes).unwrap(),
            record
        );
        let selected = inventory(vec![record.clone()])
            .selection_for_resolved_marker(&marker, |height| {
                Ok(match height {
                    1 => Some(native.block_hash),
                    2 => Some(later.block_hash),
                    _ => None,
                })
            })
            .expect("ordinary successor preserves committed repair ownership");
        assert_eq!(selected.pins, BTreeMap::from([(1, native.block_hash)]));
        assert_eq!(selected.selected_candidates, BTreeSet::from([native]));
        assert_eq!(
            record
                .classify_resolved_carrier(&marker, Some(native))
                .unwrap(),
            NativeAmxPublicationIndexResolution::Committed
        );
        for selected in [None, Some(carrier(1, 11, 99)), Some(carrier(1, 71, 72))] {
            assert_eq!(
                record.classify_resolved_carrier(&marker, selected).unwrap(),
                NativeAmxPublicationIndexResolution::RequiresRetirementProof,
                "a repair's pre-admission frontier cannot prove it uncommitted"
            );
        }
        assert_eq!(
            record
                .classify_resolved_carrier(&BlockStoreCommitMarker::new(0, None), None)
                .unwrap(),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
    }

    #[test]
    fn completed_repair_rejects_future_height_tip_mismatch_and_replacement() {
        let mut record = NativeAmxPublicationIndexRecord {
            origin: NativeAmxPublicationIndexOriginV1::CompletedRepair,
            ..initial()
        };
        assert!(
            record.validate().is_err(),
            "an uncommitted carrier is not a repair"
        );
        record.selection_marker = BlockStoreCommitMarker::new(1, Some(record.carrier.block_hash));
        assert!(record.validate().is_ok());
        record.selection_marker.tip_hash = Some(carrier(1, 71, 72).block_hash);
        assert!(record.validate().is_err(), "tip identity must be exact");
        record.selection_marker.tip_hash = Some(record.carrier.block_hash);
        record.replaced = Some(carrier(1, 11, 99));
        assert!(
            record.validate().is_err(),
            "repair never grants replacement authority"
        );
    }

    #[test]
    fn later_ordinary_tip_preserves_earlier_native_carrier_pin() {
        let record = initial();
        let native = record.carrier;
        let later = carrier(2, 31, 32);
        let marker = BlockStoreCommitMarker::new(2, Some(later.block_hash));
        let inventory = inventory(vec![record.clone()]);
        let selected = inventory
            .selection_for_resolved_marker(&marker, |height| {
                Ok(match height {
                    1 => Some(native.block_hash),
                    2 => Some(later.block_hash),
                    _ => None,
                })
            })
            .expect("exact resolved journal");
        assert_eq!(selected.pins, BTreeMap::from([(1, native.block_hash)]));
        assert_eq!(selected.selected_candidates, BTreeSet::from([native]));
        assert!(selected.unresolved.is_empty());
        assert_eq!(
            record
                .classify_resolved_carrier(&marker, Some(native))
                .expect("exact body"),
            NativeAmxPublicationIndexResolution::Committed
        );
    }

    #[test]
    fn uncommitted_append_is_not_a_pin_or_implicit_retirement() {
        let record = initial();
        let empty = BlockStoreCommitMarker::new(0, None);
        let inventory = inventory(vec![record.clone()]);
        let selected = inventory
            .selection_for_resolved_marker(&empty, |_| panic!("empty journal must not be read"))
            .expect("uncommitted candidate stays owned");
        assert!(selected.pins.is_empty());
        assert_eq!(selected.unresolved, BTreeSet::from([record.carrier]));
        assert_eq!(
            record
                .classify_resolved_carrier(&empty, None)
                .expect("exact original marker"),
            NativeAmxPublicationIndexResolution::ProvenUncommitted
        );
        let other = carrier(1, 91, 92);
        let later = BlockStoreCommitMarker::new(1, Some(other.block_hash));
        assert_eq!(
            record
                .classify_resolved_carrier(&later, Some(other))
                .expect("different committed carrier"),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
    }

    #[test]
    fn same_header_replacement_requires_complete_wire_selection() {
        let old_record = initial();
        let old = old_record.carrier;
        let replacement = NativeAmxPublicationIndexRecord {
            carrier: carrier(1, 11, 99),
            selection_marker: BlockStoreCommitMarker::new(1, Some(old.block_hash)),
            replaced: Some(old),
            ..old_record.clone()
        };
        assert!(replacement.validate().is_ok());
        let marker = replacement.selection_marker.clone();
        let inventory = inventory(vec![old_record.clone(), replacement.clone()]);
        let selected = inventory
            .selection_for_resolved_marker(&marker, |_| Ok(Some(old.block_hash)))
            .expect("same header candidates");
        assert_eq!(selected.pins.len(), 1);
        assert_eq!(selected.selected_candidates.len(), 2);
        assert_eq!(
            replacement
                .classify_resolved_carrier(&marker, Some(old))
                .expect("old wire selected"),
            NativeAmxPublicationIndexResolution::ProvenUncommitted
        );
        assert_eq!(
            replacement
                .classify_resolved_carrier(&marker, Some(replacement.carrier))
                .expect("new wire selected"),
            NativeAmxPublicationIndexResolution::Committed
        );
        assert_eq!(
            old_record
                .classify_resolved_carrier(&marker, Some(replacement.carrier))
                .expect("old record remains separately owned"),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
    }

    #[test]
    fn filename_classifier_rejects_aliases_and_unowned_temporaries() {
        let name = initial().file_name().expect("canonical name");
        assert_eq!(
            native_amx_publication_index_file_kind(&name),
            Some(NativeAmxPublicationIndexFileKind::Stable { height: 1 })
        );
        assert_eq!(
            native_amx_publication_index_file_kind(".native-amx-publication-Ab09Zx"),
            Some(NativeAmxPublicationIndexFileKind::Temporary)
        );
        for invalid in [
            name.replacen("00000000000000000001", "1", 1),
            name.replacen("00000000000000000001", "00000000000000000000", 1),
            name.to_uppercase(),
            ".native-amx-publication-".to_owned(),
            ".native-amx-publication-../../x".to_owned(),
            ".kura-sidecar-Ab09Zx".to_owned(),
        ] {
            assert_eq!(
                native_amx_publication_index_file_kind(&invalid),
                None,
                "{invalid}"
            );
        }
    }

    #[test]
    fn selected_marker_must_match_its_journal_and_body_height() {
        let record = initial();
        let marker = BlockStoreCommitMarker::new(1, Some(record.carrier.block_hash));
        assert!(
            inventory(vec![record.clone()])
                .pins_for_resolved_marker(&marker, |_| Ok(None))
                .is_err()
        );
        assert!(
            record
                .classify_resolved_carrier(&marker, Some(carrier(2, 11, 12)))
                .is_err()
        );
        assert!(
            NativeAmxPublicationIndexRecord::decode(&vec![
                0;
                NATIVE_AMX_PUBLICATION_INDEX_MAX_RECORD_BYTES
                    + 1
            ])
            .is_err()
        );
        assert_eq!(
            MAX_NATIVE_AMX_PUBLICATION_INDEX_RECORDS,
            2 * iroha_data_model::nexus::MAX_ACTIVE_EXECUTION_LANES
        );
    }

    #[test]
    fn ordinary_replacement_requires_exact_pending_record_or_retirement_retry() {
        let old_record = initial();
        let old = old_record.carrier;
        let new = carrier(1, 71, 72);
        let held = inventory(vec![old_record.clone()]);
        assert!(held.permits_ordinary_replacement(new, Some(old)));
        assert!(!held.permits_ordinary_replacement(new, None));
        assert!(!held.permits_ordinary_replacement(new, Some(carrier(1, 11, 99))));
        assert!(!held.permits_ordinary_replacement(carrier(2, 81, 82), Some(old)));
        let retirement = NativeAmxPublicationIndexRecord {
            carrier: new,
            selection_marker: BlockStoreCommitMarker::new(1, Some(old.block_hash)),
            replaced: Some(old),
            ..old_record
        };
        assert!(retirement.validate().is_ok());
        assert!(inventory(vec![retirement]).permits_ordinary_replacement(new, None));
        assert!(!inventory(Vec::new()).permits_ordinary_replacement(new, Some(old)));
        assert!(!inventory(vec![initial()]).permits_ordinary_replacement(old, None));
    }

    #[test]
    fn partial_and_empty_temporaries_cleanup_preserves_stable_record_and_physical_accounting() {
        let kura = Kura::blank_kura_for_testing();
        let directory = Kura::native_amx_publication_index_directory_for(&kura.store_root);
        std::fs::create_dir(&directory).expect("isolated index namespace");
        let record = initial();
        let bytes = record.encoded().expect("stable fixture frame");
        let stable = directory.join(record.file_name().expect("stable fixture name"));
        let empty = directory.join(".native-amx-publication-Empty1");
        let partial = directory.join(".native-amx-publication-Partial1");
        std::fs::write(&stable, &bytes).expect("neighboring stable fixture");
        std::fs::write(&empty, []).expect("crash immediately after temporary creation");
        std::fs::write(&partial, [0x42]).expect("crash during temporary write");
        // More than the generic per-mutation path cap; the owned bounded tree
        // must account for every zero-byte temporary as a physical entry.
        for ordinal in 0..49 {
            std::fs::write(
                directory.join(format!(".native-amx-publication-EmptyExtra{ordinal}")),
                [],
            )
            .unwrap();
        }
        let observed = Kura::read_native_amx_publication_index_for_store(&kura.store_root)
            .expect("bounded read-only crash inventory");
        assert_eq!(observed.records.len(), 1);
        assert_eq!(observed.orphan_temporaries.len(), 51);
        assert_eq!(observed.physical_file_count(), 52);
        assert_eq!(
            observed.physical_bytes,
            u64::try_from(bytes.len()).unwrap() + 1
        );
        assert_eq!(observed.carriers(), BTreeSet::from([record.carrier]));
        assert!(
            observed
                .pins_for_resolved_marker(&BlockStoreCommitMarker::new(0, None), |_| Ok(None))
                .expect("uncommitted stable fixture has no selected pin")
                .is_empty()
        );

        // Fixture setup alone initializes actual physical counts. The production
        // cleanup must publish its own exact deltas without a rescan afterwards.
        let counts = kura
            .physical_resource_scope()
            .unwrap()
            .observe(kura.evidence_resource_limits())
            .expect("actual fixture inventory");
        kura.resource_inventory
            .initialize(
                kura.resource_inventory.reconciliation_generation().unwrap(),
                &PHYSICAL_RESOURCE_FAMILIES
                    .iter()
                    .map(|family| (*family, counts[*family as usize]))
                    .collect::<Vec<_>>(),
            )
            .expect("initialize actual physical families");
        kura.refresh_disk_usage_bytes()
            .expect("initialize actual byte caches");
        let evidence_before = kura
            .resource_inventory
            .component_usage_for_tests(resource_inventory::Family::EvidenceKeyRecords)
            .unwrap();
        let storage_before = kura
            .resource_inventory
            .component_usage_for_tests(resource_inventory::Family::StorageBytes)
            .unwrap();
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        kura.cleanup_native_amx_publication_index_orphan_temporaries(&observed)
            .expect("exact orphan cleanup after fixture reconstruction fence");
        assert_eq!(std::fs::read(&stable).unwrap(), bytes);
        assert!(!empty.exists());
        assert!(!partial.exists());
        let after = Kura::read_native_amx_publication_index_for_store(&kura.store_root).unwrap();
        assert_eq!(after.records, observed.records);
        assert!(after.orphan_temporaries.is_empty());
        assert_eq!(after.physical_file_count(), 1);
        let evidence_after = kura
            .resource_inventory
            .component_usage_for_tests(resource_inventory::Family::EvidenceKeyRecords)
            .unwrap();
        let storage_after = kura
            .resource_inventory
            .component_usage_for_tests(resource_inventory::Family::StorageBytes)
            .unwrap();
        assert_eq!(
            evidence_before.persisted_entries - evidence_after.persisted_entries,
            51
        );
        assert_eq!(
            evidence_before.temporary_index_bytes - evidence_after.temporary_index_bytes,
            1
        );
        assert_eq!(evidence_before.index_bytes, evidence_after.index_bytes);
        assert_eq!(
            storage_before.storage_bytes - storage_after.storage_bytes,
            1
        );
        kura.cleanup_native_amx_publication_index_orphan_temporaries(&observed)
            .expect("absent retry repeats durability without a second debit");
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(resource_inventory::Family::StorageBytes)
                .unwrap(),
            storage_after
        );
        std::fs::write(&stable, [0x42]).expect("corrupt stable fixture");
        assert!(Kura::read_native_amx_publication_index_for_store(&kura.store_root).is_err());
    }

    #[test]
    fn orphan_cleanup_rejects_same_byte_inode_replacement() {
        let kura = Kura::blank_kura_for_testing();
        let directory = Kura::native_amx_publication_index_directory_for(&kura.store_root);
        std::fs::create_dir(&directory).expect("isolated index namespace");
        let record = initial();
        let stable = directory.join(record.file_name().unwrap());
        let stable_bytes = record.encoded().unwrap();
        std::fs::write(&stable, &stable_bytes).unwrap();
        let temporary = directory.join(".native-amx-publication-Partial1");
        std::fs::write(&temporary, [0x42]).unwrap();
        let observed = Kura::read_native_amx_publication_index_for_store(&kura.store_root).unwrap();
        let retained_old_inode = std::fs::File::open(&temporary).unwrap();
        std::fs::remove_file(&temporary).unwrap();
        std::fs::write(&temporary, [0x42]).unwrap();
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        assert!(
            kura.cleanup_native_amx_publication_index_orphan_temporaries(&observed)
                .is_err()
        );
        assert_eq!(std::fs::read(&temporary).unwrap(), [0x42]);
        assert_eq!(std::fs::read(&stable).unwrap(), stable_bytes);
        drop(retained_old_inode);
    }
    #[test]
    fn completed_repair_roundtrip_never_authorizes_uncommitted_rollback() {
        let mut repair = initial();
        repair.origin = NativeAmxPublicationIndexOriginV1::CompletedRepair;
        repair.selection_marker = BlockStoreCommitMarker::new(1, Some(repair.carrier.block_hash));
        let bytes = repair.encoded().expect("exact completed repair locator");
        assert_eq!(
            NativeAmxPublicationIndexRecord::decode(&bytes).unwrap(),
            repair
        );
        assert_eq!(
            repair
                .classify_resolved_carrier(&repair.selection_marker, Some(repair.carrier))
                .unwrap(),
            NativeAmxPublicationIndexResolution::Committed
        );
        let empty = BlockStoreCommitMarker::new(0, None);
        assert_eq!(
            repair.classify_resolved_carrier(&empty, None).unwrap(),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
        assert_eq!(
            repair
                .classify_resolved_carrier(&repair.selection_marker, None)
                .unwrap(),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
        let replaced_wire = carrier(1, 11, 99);
        assert_eq!(
            repair
                .classify_resolved_carrier(&repair.selection_marker, Some(replaced_wire))
                .unwrap(),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
        let replaced_header = carrier(1, 21, 22);
        let new_marker = BlockStoreCommitMarker::new(1, Some(replaced_header.block_hash));
        assert_eq!(
            repair
                .classify_resolved_carrier(&new_marker, Some(replaced_header))
                .unwrap(),
            NativeAmxPublicationIndexResolution::RequiresRetirementProof
        );
        let later = carrier(2, 31, 32);
        let later_marker = BlockStoreCommitMarker::new(2, Some(later.block_hash));
        assert_eq!(
            inventory(vec![repair.clone()])
                .pins_for_resolved_marker(&later_marker, |height| Ok(Some(if height == 1 {
                    repair.carrier.block_hash
                } else {
                    later.block_hash
                })))
                .unwrap(),
            BTreeMap::from([(1, repair.carrier.block_hash)])
        );
        repair.selection_marker = later_marker;
        assert!(
            repair.validate().is_ok(),
            "historical completed repair may start below a later tip"
        );
        repair.replaced = Some(replaced_wire);
        assert!(repair.validate().is_err());
        repair.replaced = None;
        repair.selection_marker = empty;
        assert!(repair.validate().is_err());
    }

    #[test]
    fn publication_index_rejects_frame_without_explicit_origin() {
        #[derive(Encode, norito::NoritoSchema)]
        #[norito_schema(name = "iroha_core::kura::NativeAmxPublicationIndexRecordV1")]
        struct MissingOrigin {
            version: u32,
            height: u64,
            block_hash: HashOf<BlockHeader>,
            executed_wire_hash: Hash,
            merge_entry_hash: Option<HashOf<MergeLedgerEntry>>,
            before_marker: BlockStoreCommitMarker,
            replaced: Option<(u64, HashOf<BlockHeader>, Hash)>,
        }
        let record = initial();
        let missing = MissingOrigin {
            version: 1,
            height: record.carrier.height,
            block_hash: record.carrier.block_hash,
            executed_wire_hash: record.carrier.executed_wire_hash,
            merge_entry_hash: None,
            before_marker: record.selection_marker,
            replaced: None,
        };
        let bytes = norito::encode_canonical(&missing).expect("prior shape for rejection only");
        assert!(NativeAmxPublicationIndexRecord::decode(&bytes).is_err());
    }
}
