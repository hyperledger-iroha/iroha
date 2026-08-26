/// Exact durable authority retained by the proactive advert refresher.
///
/// This token deliberately contains no block bytes.  Kura authenticates the
/// canonical index, retained finality, and deterministic keeper before minting
/// it, then repeats those checks before the token may authorize one body read
/// and signature.  Keeping this small token across Sumeragi height rollover
/// therefore does not trust volatile wire state or pin an exact-output slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KuraReplicaAdvertSourceV1 {
    network_id: NetworkId,
    height: u64,
    block_hash: HashOf<BlockHeader>,
    executed_block_wire_len: u64,
    executed_block_wire_hash: Hash,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    keeper_index: u32,
    keeper: PeerId,
}
impl KuraReplicaAdvertSourceV1 {
    pub(crate) const fn height(&self) -> u64 {
        self.height
    }
    #[cfg(test)]
    pub(crate) fn for_refresh_owner_test(height: u64, keeper: PeerId) -> Self {
        let test_hash = |domain: &[u8]| {
            let mut preimage = domain.to_vec();
            preimage.extend_from_slice(&height.to_le_bytes());
            Hash::new(preimage)
        };
        Self {
            network_id: crate::sumeragi::synthetic_network_id("kura-replica-refresh-owner-test"),
            height,
            block_hash: HashOf::from_untyped_unchecked(test_hash(b"kura-replica-refresh-block")),
            executed_block_wire_len: 1,
            executed_block_wire_hash: test_hash(b"kura-replica-refresh-wire"),
            finality_artifact_hash: HashOf::from_untyped_unchecked(test_hash(
                b"kura-replica-refresh-finality",
            )),
            keeper_index: 0,
            keeper,
        }
    }
    fn from_advert(advert: &KuraReplicaAdvertV1) -> Self {
        Self {
            network_id: advert.network_id,
            height: advert.height,
            block_hash: advert.block_hash,
            executed_block_wire_len: advert.executed_block_wire_len,
            executed_block_wire_hash: advert.executed_block_wire_hash,
            finality_artifact_hash: advert.finality_artifact_hash,
            keeper_index: advert.keeper_index,
            keeper: advert.keeper.clone(),
        }
    }
    fn unsigned_advert(&self) -> KuraReplicaAdvertV1 {
        KuraReplicaAdvertV1 {
            version: crate::sumeragi::message::KURA_REPLICA_ADVERT_VERSION_V1,
            network_id: self.network_id,
            height: self.height,
            block_hash: self.block_hash,
            executed_block_wire_len: self.executed_block_wire_len,
            executed_block_wire_hash: self.executed_block_wire_hash,
            finality_artifact_hash: self.finality_artifact_hash,
            keeper_index: self.keeper_index,
            keeper: self.keeper.clone(),
            signature: Vec::new(),
        }
    }
}
impl Kura {
    /// Return `true` when the block payload is available locally (in memory, `blocks.data`, or the
    /// local sidecar cache).
    #[cfg(test)]
    pub(crate) fn block_payload_available_by_hash(&self, hash: HashOf<BlockHeader>) -> bool {
        if self.canonical_storage_poisoned.load(Ordering::Acquire) {
            return false;
        }
        let Some(height) = self.get_block_height_by_hash(hash) else {
            return false;
        };
        self.block_payload_available_by_height(height)
    }
    #[cfg(test)]
    fn block_payload_available_by_height(&self, block_height: NonZeroUsize) -> bool {
        matches!(
            self.block_body_status_by_height(block_height),
            Some(BlockBodyStatus::Cached | BlockBodyStatus::Inline | BlockBodyStatus::LocalSidecar)
        )
    }
    /// Bind the immutable local transport identity before Kura's writer starts.
    ///
    /// Repeating the exact identity is idempotent; a different identity fails
    /// closed so body-keeper pinning cannot change during the process lifetime.
    pub fn bind_local_peer_id(&self, peer: PeerId) -> Result<()> {
        if let Some(bound) = self.local_peer_id.get() {
            return if bound == &peer {
                Ok(())
            } else {
                Err(Error::KuraReplicaLocalPeerConflict)
            };
        }
        match self.local_peer_id.set(peer) {
            Ok(()) => Ok(()),
            Err(peer) if self.local_peer_id.get() == Some(&peer) => Ok(()),
            Err(_) => Err(Error::KuraReplicaLocalPeerConflict),
        }
    }
    #[cfg(test)]
    fn kura_replica_advert_body_reads_for_tests(&self) -> usize {
        self.kura_replica_advert_body_reads.load(Ordering::Acquire)
    }
    /// Read the exact canonical durable tip used to anchor one proactive
    /// refresh cursor.  Height and hash are captured under the same prune and
    /// canonical-chain guards so same-height replacement cannot masquerade as
    /// an unchanged scan boundary.
    pub(crate) fn exact_kura_replica_advert_tip(
        &self,
    ) -> Result<Option<(u64, HashOf<BlockHeader>)>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let mut store = self.block_store.lock();
        let height = store.read_exact_durable_index_count()?;
        if height == 0 {
            return Ok(None);
        }
        let hash = Self::read_durable_hash_at_height(&mut store, height)?.ok_or(
            Error::BlockHeightGap {
                expected_next_height: height.saturating_add(1),
                actual_height: height,
            },
        )?;
        Ok(Some((height, hash)))
    }
    /// Probe one exact durable height without reading its block body.
    ///
    /// Heights without retained v2 finality and heights for which this node is
    /// not a deterministic CommitQC keeper are ordinary misses.  A hit is a
    /// small reconstructible token; the complete body is read only if the
    /// caller elects to publish that token.
    pub(crate) fn probe_kura_replica_advert_source(
        &self,
        height: u64,
        local_key: &KeyPair,
    ) -> Result<Option<KuraReplicaAdvertSourceV1>> {
        let keeper = PeerId::new(local_key.public_key().clone());
        if self.local_peer_id.get() != Some(&keeper) {
            return Err(Error::KuraReplicaLocalPeerConflict);
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        self.probe_kura_replica_advert_source_under_guards(height, &keeper)
    }
    /// Build and sign a local advert only while the exact canonical body is
    /// durably readable and the local peer is selected by its verified
    /// CommitQC keeper authority.
    #[cfg(test)]
    pub(crate) fn build_signed_kura_replica_advert(
        &self,
        height: u64,
        local_key: &KeyPair,
    ) -> Result<Option<KuraReplicaAdvertV1>> {
        let Some(source) = self.probe_kura_replica_advert_source(height, local_key)? else {
            return Ok(None);
        };
        self.build_signed_kura_replica_advert_from_source(&source, local_key)
            .map(Some)
    }
    /// Revalidate one retained source token, read its complete canonical body
    /// exactly once, and sign the resulting advert.
    pub(crate) fn build_signed_kura_replica_advert_from_source(
        &self,
        source: &KuraReplicaAdvertSourceV1,
        local_key: &KeyPair,
    ) -> Result<KuraReplicaAdvertV1> {
        let keeper = PeerId::new(local_key.public_key().clone());
        if source.keeper != keeper || self.local_peer_id.get() != Some(&keeper) {
            return Err(Error::KuraReplicaLocalPeerConflict);
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let current = self
            .probe_kura_replica_advert_source_under_guards(source.height, &keeper)?
            .ok_or_else(|| {
                Error::InvalidKuraReplicaAdvert(
                    "retained advert source lost its exact deterministic keeper authority"
                        .to_owned(),
                )
            })?;
        if &current != source {
            return Err(Error::InvalidKuraReplicaAdvert(
                "retained advert source changed its exact durable authority".to_owned(),
            ));
        }
        self.verify_complete_kura_replica_body_under_guards(source)?;
        let mut advert = source.unsigned_advert();
        advert.signature =
            iroha_crypto::Signature::try_new(local_key.private_key(), &advert.signature_preimage())
                .map_err(|error| {
                    Error::InvalidKuraReplicaAdvert(format!(
                        "failed to sign local Kura replica advert: {error}"
                    ))
                })?
                .payload()
                .to_vec();
        advert
            .verify_keeper_signature()
            .map_err(Error::InvalidKuraReplicaAdvert)?;
        Ok(advert)
    }
    /// Revalidate a retained exact-output advert before it crosses a height
    /// rollover.  This repeats body, finality, keeper, key, and signature
    /// checks instead of trusting the queued wire object.
    #[cfg(test)]
    pub(crate) fn revalidate_local_kura_replica_advert(
        &self,
        advert: &KuraReplicaAdvertV1,
        local_key: &KeyPair,
    ) -> Result<()> {
        advert
            .verify_keeper_signature()
            .map_err(Error::InvalidKuraReplicaAdvert)?;
        let local_peer = PeerId::new(local_key.public_key().clone());
        if advert.keeper != local_peer || self.local_peer_id.get() != Some(&local_peer) {
            return Err(Error::KuraReplicaLocalPeerConflict);
        }
        self.revalidate_kura_replica_advert_source(advert)
    }
    /// Revalidate a retained local advert from durable Kura state without
    /// re-signing it or publishing it into the remote-replica registry.
    pub(crate) fn revalidate_kura_replica_advert_source(
        &self,
        advert: &KuraReplicaAdvertV1,
    ) -> Result<()> {
        advert
            .verify_keeper_signature()
            .map_err(Error::InvalidKuraReplicaAdvert)?;
        if self.local_peer_id.get() != Some(&advert.keeper) {
            return Err(Error::KuraReplicaLocalPeerConflict);
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let source = KuraReplicaAdvertSourceV1::from_advert(advert);
        let current = self
            .probe_kura_replica_advert_source_under_guards(advert.height, &advert.keeper)?
            .ok_or_else(|| {
                Error::InvalidKuraReplicaAdvert(
                    "retained advert lost its exact deterministic keeper authority".to_owned(),
                )
            })?;
        if current != source {
            return Err(Error::InvalidKuraReplicaAdvert(
                "retained local advert differs from exact durable authority".to_owned(),
            ));
        }
        self.verify_complete_kura_replica_body_under_guards(&source)?;
        Ok(())
    }
    /// Authenticate the canonical index, retained finality, and local keeper
    /// while the caller owns the prune and canonical-chain guards.  This path
    /// never reads the encoded block body.
    fn probe_kura_replica_advert_source_under_guards(
        &self,
        height: u64,
        keeper: &PeerId,
    ) -> Result<Option<KuraReplicaAdvertSourceV1>> {
        let (canonical_hash, index, blocks_dir) = {
            let mut store = self.block_store.lock();
            let durable_count = store.read_exact_durable_index_count()?;
            if height == 0 || height > durable_count {
                return Ok(None);
            }
            let canonical_hash = Self::read_durable_hash_at_height(&mut store, height)?.ok_or(
                Error::BlockHeightGap {
                    expected_next_height: durable_count.saturating_add(1),
                    actual_height: height,
                },
            )?;
            let index = store.read_block_index(height.saturating_sub(1))?;
            (canonical_hash, index, store.path_to_blockchain.clone())
        };
        let Some(authority) =
            self.verified_kura_replica_authority_for_eviction(&blocks_dir, height, canonical_hash)?
        else {
            return Ok(None);
        };
        let Some(keeper_index) = authority
            .selected_keepers
            .iter()
            .find(|(_, selected)| selected == keeper)
            .map(|(index, _)| *index)
        else {
            return Ok(None);
        };
        if index.length == 0 || index.length > STRICT_INIT_MAX_BLOCK_BYTES {
            return Err(Error::InvalidKuraReplicaAdvert(
                "selected local keeper has an invalid canonical body index".to_owned(),
            ));
        }
        if authority.key.executed_block_wire_len != index.length {
            return Err(Error::InvalidKuraReplicaAdvert(
                "authenticated finality differs from the exact durable body index".to_owned(),
            ));
        }
        Ok(Some(KuraReplicaAdvertSourceV1 {
            network_id: authority.network_id,
            height,
            block_hash: canonical_hash,
            executed_block_wire_len: authority.key.executed_block_wire_len,
            executed_block_wire_hash: authority.key.executed_block_wire_hash,
            finality_artifact_hash: authority.key.finality_artifact_hash,
            keeper_index,
            keeper: keeper.clone(),
        }))
    }
    /// Read and validate the complete local body only after exact keeper
    /// authority has been authenticated under the enclosing guards.
    fn verify_complete_kura_replica_body_under_guards(
        &self,
        source: &KuraReplicaAdvertSourceV1,
    ) -> Result<()> {
        #[cfg(test)]
        self.kura_replica_advert_body_reads
            .fetch_add(1, Ordering::AcqRel);
        let (index, da_blocks_dir, da_path, inline_wire) = {
            let mut store = self.block_store.lock();
            let index = store.read_block_index(source.height.saturating_sub(1))?;
            if index.length != source.executed_block_wire_len
                || index.length == 0
                || index.length > STRICT_INIT_MAX_BLOCK_BYTES
            {
                return Err(Error::InvalidKuraReplicaAdvert(
                    "selected local keeper has a mismatched canonical body index".to_owned(),
                ));
            }
            let inline_wire = if index.is_evicted() {
                None
            } else {
                let mut bytes = vec![0_u8; usize::try_from(index.length)?];
                store.read_block_data(index.start, &mut bytes)?;
                Some(bytes)
            };
            (
                index,
                store.da_blocks_dir.clone(),
                store.da_block_path(source.height),
                inline_wire,
            )
        };
        let wire = if let Some(wire) = inline_wire {
            wire
        } else {
            self.read_regular_sidecar_bytes(
                &da_path,
                &da_blocks_dir,
                usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX),
            )?
            .ok_or_else(|| {
                Error::InvalidKuraReplicaAdvert(
                    "selected local keeper lost its complete canonical body".to_owned(),
                )
            })?
        };
        if u64::try_from(wire.len())? != index.length
            || Hash::new(&wire) != source.executed_block_wire_hash
        {
            return Err(Error::InvalidKuraReplicaAdvert(
                "selected local keeper has a corrupt complete canonical body".to_owned(),
            ));
        }
        let block = decode_framed_signed_block(&wire)?;
        if block.header().height().get() != source.height || block.hash() != source.block_hash {
            return Err(Error::InvalidKuraReplicaAdvert(
                "selected local keeper body differs from the canonical block identity".to_owned(),
            ));
        }
        Ok(())
    }
    /// Authenticate the canonical index, retained finality, and deterministic
    /// keeper while the caller owns the prune and canonical-chain guards.
    fn verified_kura_replica_advert_authority_under_guards(
        &self,
        advert: &KuraReplicaAdvertV1,
        blocks_dir: &Path,
    ) -> Result<(VerifiedKuraReplicaAuthority, u64)> {
        let (canonical_hash, index, durable_count) = {
            let mut store = self.block_store.lock();
            let durable_count = store.read_exact_durable_index_count()?;
            if advert.height == 0 || advert.height > durable_count {
                return Err(Error::InvalidKuraReplicaAdvert(
                    "advertised height is outside the exact durable chain".to_owned(),
                ));
            }
            let canonical_hash = Self::read_durable_hash_at_height(&mut store, advert.height)?
                .ok_or_else(|| {
                    Error::InvalidKuraReplicaAdvert(
                        "advertised height has no durable canonical hash".to_owned(),
                    )
                })?;
            let index = store.read_block_index(advert.height.saturating_sub(1))?;
            (canonical_hash, index, durable_count)
        };
        if canonical_hash != advert.block_hash
            || index.length == 0
            || index.length != advert.executed_block_wire_len
        {
            return Err(Error::InvalidKuraReplicaAdvert(
                "advert does not bind the exact durable block index".to_owned(),
            ));
        }
        let authority = self
            .verified_kura_replica_authority_for_eviction(
                blocks_dir,
                advert.height,
                canonical_hash,
            )?
            .ok_or_else(|| {
                Error::InvalidKuraReplicaAdvert(
                    "advertised block has no authenticated retained v2 finality".to_owned(),
                )
            })?;
        if advert.network_id != authority.network_id
            || advert.block_hash != authority.key.block_hash
            || advert.finality_artifact_hash != authority.key.finality_artifact_hash
            || advert.executed_block_wire_len != authority.key.executed_block_wire_len
            || advert.executed_block_wire_hash != authority.key.executed_block_wire_hash
        {
            return Err(Error::InvalidKuraReplicaAdvert(
                "advert differs from exact authenticated finality or executed wire".to_owned(),
            ));
        }
        if !authority
            .selected_keepers
            .iter()
            .any(|(index, peer)| *index == advert.keeper_index && peer == &advert.keeper)
        {
            return Err(Error::InvalidKuraReplicaAdvert(
                "advert signer is not an exact deterministic keeper".to_owned(),
            ));
        }
        Ok((authority, durable_count))
    }
    /// Authenticate and retain one direct keeper advert against exact durable
    /// canonical finality.  Structural/signature validation happens before
    /// any Kura lock; canonical identity and keeper selection are then checked
    /// under the prune and canonical-chain guards.
    pub(crate) fn admit_kura_replica_advert(&self, advert: &KuraReplicaAdvertV1) -> Result<()> {
        advert
            .verify_keeper_signature()
            .map_err(Error::InvalidKuraReplicaAdvert)?;
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        self.ensure_canonical_storage_not_poisoned()?;
        let blocks_dir = self.active_blocks_dir.lock().clone();
        let (authority, canonical_tip) =
            self.verified_kura_replica_advert_authority_under_guards(advert, &blocks_dir)?;
        let now = Instant::now();
        let mut registry = self.replica_registry.lock();
        let (minimum_height, maximum_height) = self.replica_registry_height_horizon(canonical_tip);
        if authority.key.height < minimum_height || authority.key.height > maximum_height {
            return Err(Error::InvalidKuraReplicaAdvert(format!(
                "advertised height {} is outside the active replica registry horizon {minimum_height}..={maximum_height}",
                authority.key.height,
            )));
        }
        let registry_cap = self.replica_registry_capacity();
        let fresh = |observation: &BlockReplicaAdvert| {
            now.saturating_duration_since(observation.observed_at) <= self.replica_advert_ttl
        };
        let surviving_other_keys = registry
            .iter()
            .filter(|(key, peers)| {
                key.height >= minimum_height
                    && key.height <= maximum_height
                    && key.height != authority.key.height
                    && peers.values().any(|observation| fresh(observation))
            })
            .count();
        let prospective_keys = surviving_other_keys.checked_add(1).ok_or_else(|| {
            Error::InvalidKuraReplicaAdvert(
                "replica registry key count exceeds the platform size representation".to_owned(),
            )
        })?;
        if prospective_keys > registry_cap {
            return Err(Error::InvalidKuraReplicaAdvert(
                "replica registry cannot admit the authenticated key within its active height horizon"
                    .to_owned(),
            ));
        }
        let exact_peers = registry.get(&authority.key);
        let fresh_exact_peers = exact_peers
            .map(|peers| {
                peers
                    .values()
                    .filter(|observation| fresh(observation))
                    .count()
            })
            .unwrap_or(0);
        let keeper_is_fresh = exact_peers
            .and_then(|peers| peers.get(&advert.keeper))
            .is_some_and(|observation| fresh(observation));
        let prospective_peers = fresh_exact_peers
            .checked_add(usize::from(!keeper_is_fresh))
            .ok_or_else(|| {
                Error::InvalidKuraReplicaAdvert(
                    "replica registry peer count exceeds the platform size representation"
                        .to_owned(),
                )
            })?;
        if prospective_peers
            > iroha_config::parameters::actual::KURA_REPLICA_ADVERT_KEEPERS_PER_KEY_LIMIT
        {
            return Err(Error::InvalidKuraReplicaAdvert(
                "replica registry peer count exceeds the protocol validator bound".to_owned(),
            ));
        }
        self.prune_replica_adverts_for_horizon(&mut registry, now, minimum_height, maximum_height);
        registry.retain(|key, _| key.height != authority.key.height || key == &authority.key);
        registry.entry(authority.key).or_default().insert(
            advert.keeper.clone(),
            BlockReplicaAdvert {
                keeper_index: advert.keeper_index,
                observed_at: now,
            },
        );
        Ok(())
    }
    /// Return local/remote body status for a canonical block hash known to Kura.
    #[cfg(test)]
    pub(crate) fn block_body_status_by_hash(
        &self,
        hash: HashOf<BlockHeader>,
    ) -> Option<BlockBodyStatus> {
        if self.canonical_storage_poisoned.load(Ordering::Acquire) {
            return None;
        }
        let height = self.get_block_height_by_hash(hash)?;
        self.block_body_status_by_height(height)
    }
    #[cfg(test)]
    fn block_body_status_by_height(&self, block_height: NonZeroUsize) -> Option<BlockBodyStatus> {
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        let (block_index, has_cached) = {
            let data = self.block_data.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            let idx = block_height.get().saturating_sub(1);
            if data.len() <= idx {
                return None;
            }
            (idx, data.cached_body(idx).is_some())
        };
        if has_cached {
            if self.prune_recovery_is_required() {
                return None;
            }
            return Some(BlockBodyStatus::Cached);
        }
        let (index, expected_hash, blocks_dir, da_blocks_dir, da_path, canonical_tip) = {
            let mut store = self.block_store.lock();
            if self.prune_recovery_is_required() {
                return None;
            }
            let index = match store.read_block_index(block_index as u64) {
                Ok(index) => index,
                Err(err) => {
                    warn!(
                        ?err,
                        block_index,
                        "failed to read block index while checking payload availability"
                    );
                    return Some(BlockBodyStatus::Missing);
                }
            };
            let expected_hash = match store.read_block_hashes(block_index as u64, 1) {
                Ok(hashes) => hashes.first().copied(),
                Err(err) => {
                    warn!(
                        ?err,
                        block_index,
                        "failed to read block hash while checking payload availability"
                    );
                    None
                }
            };
            let blocks_dir = store.path_to_blockchain.clone();
            let da_blocks_dir = store.da_blocks_dir.clone();
            let da_path = store.da_block_path(block_height.get() as u64);
            let canonical_tip = match store.read_exact_durable_index_count() {
                Ok(tip) => tip,
                Err(err) => {
                    warn!(
                        ?err,
                        "failed to read durable tip while checking payload availability"
                    );
                    return Some(BlockBodyStatus::Missing);
                }
            };
            (
                index,
                expected_hash,
                blocks_dir,
                da_blocks_dir,
                da_path,
                canonical_tip,
            )
        };
        if self.prune_recovery_is_required() {
            return None;
        }
        if index.length == 0 {
            return Some(BlockBodyStatus::Missing);
        }
        if !index.is_evicted() {
            return Some(if has_cached {
                BlockBodyStatus::Cached
            } else {
                BlockBodyStatus::Inline
            });
        }
        let Some(expected_hash) = expected_hash else {
            return Some(BlockBodyStatus::Missing);
        };
        let height = block_height.get() as u64;
        let has_local_candidate = std::fs::symlink_metadata(&da_path).is_ok_and(|metadata| {
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() == index.length
        });
        let authority = self
            .verified_kura_replica_authority_for_eviction(&blocks_dir, height, expected_hash)
            .ok()
            .flatten();
        if has_local_candidate
            && let Some(authority) = authority.as_ref()
            && authority.key.executed_block_wire_len == index.length
            && let Ok(Some(bytes)) = self.read_regular_sidecar_bytes(
                &da_path,
                &da_blocks_dir,
                usize::try_from(STRICT_INIT_MAX_BLOCK_BYTES).unwrap_or(usize::MAX),
            )
            && u64::try_from(bytes.len()).ok() == Some(index.length)
            && Hash::new(&bytes) == authority.key.executed_block_wire_hash
        {
            return Some(BlockBodyStatus::LocalSidecar);
        }
        let Some(authority) = authority else {
            return Some(BlockBodyStatus::Missing);
        };
        let replicas = self.matching_selected_keeper_count(&authority, canonical_tip);
        if self.prune_recovery_is_required()
            || self.canonical_storage_poisoned.load(Ordering::Acquire)
        {
            return None;
        }
        if self.has_all_selected_remote_keepers(&authority, canonical_tip) {
            Some(BlockBodyStatus::RemoteOnly { replicas })
        } else {
            Some(BlockBodyStatus::Missing)
        }
    }
    fn prune_replica_adverts_for_horizon(
        &self,
        registry: &mut BlockReplicaRegistry,
        now: Instant,
        minimum_height: u64,
        maximum_height: u64,
    ) {
        registry.retain(|_, peers| {
            peers.retain(|_, advert| {
                now.saturating_duration_since(advert.observed_at) <= self.replica_advert_ttl
            });
            !peers.is_empty()
        });
        registry.retain(|key, _| key.height >= minimum_height && key.height <= maximum_height);
    }
    fn replica_registry_capacity(&self) -> usize {
        self.replica_registry_key_capacity.get()
    }
    fn replica_registry_height_horizon(&self, canonical_tip: u64) -> (u64, u64) {
        let span = self
            .replica_registry_key_capacity
            .get()
            .checked_sub(1)
            .expect("non-zero replica registry capacity has a representable predecessor");
        let minimum_height = u64::try_from(span)
            .ok()
            .and_then(|span| canonical_tip.checked_sub(span))
            .unwrap_or(1)
            .max(1);
        (minimum_height, canonical_tip)
    }
    fn matching_selected_keeper_count(
        &self,
        authority: &VerifiedKuraReplicaAuthority,
        canonical_tip: u64,
    ) -> usize {
        let now = Instant::now();
        let mut registry = self.replica_registry.lock();
        let (minimum_height, maximum_height) = self.replica_registry_height_horizon(canonical_tip);
        self.prune_replica_adverts_for_horizon(&mut registry, now, minimum_height, maximum_height);
        registry
            .get(&authority.key)
            .map(|peers| {
                authority
                    .selected_keepers
                    .iter()
                    .filter(|(index, peer)| {
                        peers
                            .get(peer)
                            .is_some_and(|advert| advert.keeper_index == *index)
                    })
                    .count()
            })
            .unwrap_or(0)
    }
    fn has_all_selected_remote_keepers(
        &self,
        authority: &VerifiedKuraReplicaAuthority,
        canonical_tip: u64,
    ) -> bool {
        let Some(local_peer) = self.local_peer_id.get() else {
            return false;
        };
        if authority.selected_keepers.is_empty()
            || authority
                .selected_keepers
                .iter()
                .any(|(_, keeper)| keeper == local_peer)
        {
            return false;
        }
        self.matching_selected_keeper_count(authority, canonical_tip)
            == authority.selected_keepers.len()
    }
}
