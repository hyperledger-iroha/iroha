// Assignment-bound publisher bytes are isolated from the admitted manifest index. Only the native
// provider ingest worker may consume them, after obtaining its independent finalized authorization.

use sorafs_car::publisher::{
    PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1, PublisherSourceChunkRequestV1, PublisherSourceHeaderV1,
};
const PUBLISHER_SOURCE_DIR_V1: &str = ".publisher-sources";
const PUBLISHER_SOURCE_METADATA_V1: &str = "source.to";
const PUBLISHER_SOURCE_CACHE_BYTES_V1: u64 = 16 * 1024 * 1024;
const PUBLISHER_SOURCE_CACHE_ENTRIES_V1: usize = 32;

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::store::StoredPublisherSourceV1")]
struct StoredPublisherSourceV1 {
    version: u8,
    deadline_epoch: u64,
    header: PublisherSourceHeaderV1,
}

fn publisher_source_rejected() -> StorageError {
    StorageError::Io(io::Error::new(
        io::ErrorKind::InvalidData,
        "publisher source rejected",
    ))
}

#[derive(Debug)]
struct VerifiedPublisherSourceV1 {
    source: StoredPublisherSourceV1,
    manifest: ManifestV1,
    plan: CarBuildPlan,
    header_digest: [u8; 32],
    manifest_digest: [u8; 32],
    identity: fs::Metadata,
    encoded_bytes: u64,
}

#[derive(Debug, Default)]
struct PublisherSourceCacheV1 {
    entries: BTreeMap<[u8; 32], (u64, Arc<VerifiedPublisherSourceV1>)>,
    encoded_bytes: u64,
    clock: u64,
    #[cfg(test)]
    loads: usize,
}
impl PublisherSourceCacheV1 {
    fn remove(&mut self, order: &[u8; 32]) {
        if let Some((_, entry)) = self.entries.remove(order) {
            self.encoded_bytes = self.encoded_bytes.saturating_sub(entry.encoded_bytes);
        }
    }
    fn retain(&mut self, source: Arc<VerifiedPublisherSourceV1>, config: &StorageConfig) {
        let limit = config
            .max_capacity_bytes()
            .0
            .min(PUBLISHER_SOURCE_CACHE_BYTES_V1);
        let entries = config.max_pins().min(PUBLISHER_SOURCE_CACHE_ENTRIES_V1);
        self.remove(&source.source.header.order_id);
        if source.encoded_bytes > limit || entries == 0 {
            return;
        }
        while self.entries.len() >= entries || self.encoded_bytes + source.encoded_bytes > limit {
            let Some(order) = self
                .entries
                .iter()
                .min_by_key(|(_, (age, _))| *age)
                .map(|(order, _)| *order)
            else {
                break;
            };
            self.remove(&order);
        }
        self.clock = self.clock.saturating_add(1);
        self.encoded_bytes += source.encoded_bytes;
        self.entries
            .insert(source.source.header.order_id, (self.clock, source));
    }
    fn load(
        &mut self,
        path: &Path,
        order: [u8; 32],
        config: &StorageConfig,
    ) -> Result<Arc<VerifiedPublisherSourceV1>, StorageError> {
        if let Some((_, entry)) = self.entries.get(&order) {
            if let Err(error) = entry.check_identity(path) {
                self.remove(&order);
                return Err(error);
            }
            let result = Arc::clone(entry);
            self.clock = self.clock.saturating_add(1);
            if let Some((age, _)) = self.entries.get_mut(&order) {
                *age = self.clock;
            }
            return Ok(result);
        }
        let entry = Arc::new(read_publisher_source(path)?);
        if entry.source.header.order_id != order {
            return Err(publisher_source_rejected());
        }
        #[cfg(test)]
        {
            self.loads += 1;
        }
        self.retain(Arc::clone(&entry), config);
        Ok(entry)
    }
}
impl VerifiedPublisherSourceV1 {
    fn check_identity(&self, path: &Path) -> Result<(), StorageError> {
        validate_real_directory(path.parent().ok_or_else(publisher_source_rejected)?)?;
        validate_real_directory(path)?;
        let file = path.join(PUBLISHER_SOURCE_METADATA_V1);
        let current = fs::symlink_metadata(&file)?;
        validate_bounded_file_metadata(
            &file,
            &current,
            (PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1 + 1024) as u64,
        )?;
        if !metadata_stable_during_read(&self.identity, &current) {
            return Err(publisher_source_rejected());
        }
        Ok(())
    }
}
fn read_publisher_source(path: &Path) -> Result<VerifiedPublisherSourceV1, StorageError> {
    validate_real_directory(path.parent().ok_or_else(publisher_source_rejected)?)?;
    validate_real_directory(path)?;
    let metadata_path = path.join(PUBLISHER_SOURCE_METADATA_V1);
    let before = fs::symlink_metadata(&metadata_path)?;
    let bytes = read_bounded_regular_file(
        &path.join(PUBLISHER_SOURCE_METADATA_V1),
        (PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1 + 1024) as u64,
    )?;
    let metadata: StoredPublisherSourceV1 = norito::decode_canonical_with_limits(
        &bytes,
        storage_decode_limits((PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1 + 1024) as u64),
    )?;
    if metadata.version != 1 || metadata.deadline_epoch == 0 {
        return Err(publisher_source_rejected());
    }
    let (manifest, plan) = metadata
        .header
        .verify()
        .map_err(|_| publisher_source_rejected())?;
    let header_digest = metadata
        .header
        .canonical_digest()
        .map_err(|_| publisher_source_rejected())?;
    let manifest_digest = *manifest.digest()?.as_bytes();
    let identity = fs::symlink_metadata(&metadata_path)?;
    if !metadata_stable_during_read(&before, &identity) {
        return Err(publisher_source_rejected());
    }
    Ok(VerifiedPublisherSourceV1 {
        source: metadata,
        manifest,
        plan,
        header_digest,
        manifest_digest,
        identity,
        encoded_bytes: bytes.len() as u64,
    })
}

impl StorageBackend {
    /// Reserve restart-safe staging for one publisher-authenticated finalized assignment.
    ///
    /// The trusted caller must authenticate the request's publisher, exact provider/order/revision,
    /// current approved pin and inclusive order deadline in one finalized State view. These values
    /// are informational bindings, not authority capabilities. Staging never serves or proves bytes.
    /// Total reserved staging bytes, including metadata, are bounded by `max_capacity_bytes`, with
    /// at most `max_pins` sessions, independently of the admitted-storage budget.
    /// Verified metadata is cached for at most 32 sessions and 16 MiB of canonical metadata, further
    /// limited by the configured session and capacity quotas. Decoded metadata is bounded by these
    /// limits and the per-header decode limits; the byte counter does not measure allocator RSS.
    pub fn stage_publisher_source(
        &self,
        header: &PublisherSourceHeaderV1,
        deadline_epoch: u64,
        finalized_epoch: u64,
    ) -> Result<[u8; 32], StorageError> {
        let wall_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| publisher_source_rejected())?
            .as_secs();
        let (manifest, plan) = header.verify().map_err(|_| publisher_source_rejected())?;
        let header_digest = header
            .canonical_digest()
            .map_err(|_| publisher_source_rejected())?;
        if finalized_epoch == 0
            || deadline_epoch < finalized_epoch
            || deadline_epoch < wall_epoch
            || deadline_epoch > manifest.pin_policy.retention_epoch
            || self.config.provider_id().map(|id| *id.as_bytes()) != Some(header.provider_id)
        {
            return Err(publisher_source_rejected());
        }
        let mut cache = self
            .publisher_sources
            .lock()
            .map_err(|_| publisher_source_rejected())?;
        let root = self.root_dir.join(PUBLISHER_SOURCE_DIR_V1);
        fs::create_dir_all(&root)?;
        validate_real_directory(&root)?;
        let target = root.join(hex::encode(header.order_id));
        let metadata = StoredPublisherSourceV1 {
            version: 1,
            deadline_epoch,
            header: header.clone(),
        };
        let encoded = norito::encode_canonical(&metadata)?;
        let required = manifest
            .content_length
            .checked_add(encoded.len() as u64)
            .ok_or_else(publisher_source_rejected)?;
        let mut reserved = 0_u64;
        let mut sessions = 0_usize;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if !entry
                .file_name()
                .to_str()
                .is_some_and(is_canonical_manifest_id)
            {
                return Err(publisher_source_rejected());
            }
            let path = entry.path();
            let order: [u8; 32] = hex::decode(
                entry
                    .file_name()
                    .to_str()
                    .ok_or_else(publisher_source_rejected)?,
            )
            .map_err(|_| publisher_source_rejected())?
            .try_into()
            .map_err(|_| publisher_source_rejected())?;
            validate_real_directory(&path)?;
            // A failed initial metadata publication contains no accepted chunks and is discardable.
            if !path.join(PUBLISHER_SOURCE_METADATA_V1).try_exists()? {
                remove_transaction_directory(&path)?;
                cache.remove(&order);
                continue;
            }
            let existing = cache.load(&path, order, &self.config)?;
            if existing.source.deadline_epoch < finalized_epoch.max(wall_epoch) {
                remove_transaction_directory(&path)?;
                cache.remove(&order);
                continue;
            }
            if path == target {
                if existing.source.header == *header
                    && existing.source.deadline_epoch == deadline_epoch
                {
                    return Ok(existing.header_digest);
                }
                if existing.source.header.assignment_revision >= header.assignment_revision {
                    return Err(publisher_source_rejected());
                }
                // The trusted caller rechecked the new finalized revision. Old bytes cannot cross
                // that boundary, even when the manifest happens to be identical.
                remove_transaction_directory(&path)?;
                cache.remove(&order);
                continue;
            }
            reserved = reserved
                .checked_add(existing.manifest.content_length)
                .and_then(|total| total.checked_add(existing.encoded_bytes))
                .ok_or_else(publisher_source_rejected)?;
            sessions += 1;
            if sessions >= self.config.max_pins() {
                return Err(StorageError::PinLimitReached {
                    limit: self.config.max_pins(),
                });
            }
        }
        let available = self.config.max_capacity_bytes().0.saturating_sub(reserved);
        if required > available {
            return Err(StorageError::CapacityExceeded {
                required,
                available,
            });
        }
        fs::create_dir(&target)?;
        sync_directory(&root)?;
        write_atomic(&target.join(PUBLISHER_SOURCE_METADATA_V1), &encoded)?;
        let identity = fs::symlink_metadata(target.join(PUBLISHER_SOURCE_METADATA_V1))?;
        cache.retain(
            Arc::new(VerifiedPublisherSourceV1 {
                manifest_digest: *manifest.digest()?.as_bytes(),
                source: metadata,
                manifest,
                plan,
                header_digest,
                identity,
                encoded_bytes: encoded.len() as u64,
            }),
            &self.config,
        );
        Ok(header_digest)
    }

    /// Persist one exact signed-request chunk for the currently authorized staging session.
    /// The trusted caller must repeat the same finalized publisher/assignment checks on every call.
    pub fn stage_publisher_source_chunk(
        &self,
        request: &PublisherSourceChunkRequestV1,
        finalized_epoch: u64,
    ) -> Result<(), StorageError> {
        let mut cache = self
            .publisher_sources
            .lock()
            .map_err(|_| publisher_source_rejected())?;
        let path = self
            .root_dir
            .join(PUBLISHER_SOURCE_DIR_V1)
            .join(hex::encode(request.order_id));
        let source = cache.load(&path, request.order_id, &self.config)?;
        let wall_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| publisher_source_rejected())?
            .as_secs();
        if source.source.header.provider_id != request.provider_id
            || self.config.provider_id().map(|id| *id.as_bytes()) != Some(request.provider_id)
            || source.source.header.assignment_revision != request.assignment_revision
            || source.manifest_digest != request.manifest_digest
            || source.source.deadline_epoch < finalized_epoch
            || source.source.deadline_epoch < wall_epoch
            || finalized_epoch == 0
            || source.header_digest != request.header_digest
        {
            return Err(publisher_source_rejected());
        }
        let chunk = source
            .source
            .header
            .chunks
            .get(request.upload.index as usize)
            .ok_or_else(publisher_source_rejected)?;
        let bytes = &request.upload.bytes;
        if bytes.len() != chunk.length as usize || blake3::hash(bytes).as_bytes() != &chunk.digest {
            return Err(publisher_source_rejected());
        }
        write_atomic(
            &path.join(format!("chunk-{:08x}", request.upload.index)),
            bytes,
        )?;
        source.check_identity(&path)?;
        Ok(())
    }

    /// Consume complete staged bytes only after the native worker obtained current finality.
    /// A missing or incomplete session returns `None`, allowing ordinary governed source fetch.
    /// Full storage ingest revalidates every digest, PoR, canonical CAR and quota before admission.
    /// The caller must revalidate live authority in the callback, which runs for each buffered read
    /// and EOF, immediately before storage publication, and before returning success. Callback
    /// failures remain sticky. The callback must not reenter the storage backend.
    pub fn ingest_staged_publisher_source(
        &self,
        authorization: &crate::FinalizedProviderIngestAuthorizationV1,
        expected_assignment_revision: u64,
        current_authority: &mut impl FnMut() -> Result<(), StorageError>,
    ) -> Result<Option<String>, StorageError> {
        authorization
            .validate()
            .map_err(|_| publisher_source_rejected())?;
        current_authority()?;
        let mut cache = self
            .publisher_sources
            .lock()
            .map_err(|_| publisher_source_rejected())?;
        let path = self
            .root_dir
            .join(PUBLISHER_SOURCE_DIR_V1)
            .join(hex::encode(authorization.order_id()));
        if !path.try_exists()? {
            return Ok(None);
        }
        let source = cache.load(&path, authorization.order_id(), &self.config)?;
        let wall_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| publisher_source_rejected())?
            .as_secs();
        if source.source.deadline_epoch < wall_epoch {
            remove_transaction_directory(&path)?;
            cache.remove(&authorization.order_id());
            return Ok(None);
        }
        let manifest = &source.manifest;
        let plan = &source.plan;
        if expected_assignment_revision == 0
            || source.source.header.assignment_revision != expected_assignment_revision
            || source.source.header.provider_id != authorization.provider_id()
            || self.config.provider_id().map(|id| *id.as_bytes())
                != Some(authorization.provider_id())
            || source.source.header.order_id != authorization.order_id()
            || source.manifest_digest != authorization.manifest_digest()
            || manifest.root_cid != authorization.manifest_cid()
            || manifest.content_length != authorization.content_length()
            || manifest.chunk_digest_sha3_256 != authorization.chunk_digest_sha3_256()
            || manifest.por_root != authorization.por_root()
            || canonical_profile_handle(manifest) != authorization.chunker_handle()
        {
            return Err(publisher_source_rejected());
        }
        for index in 0..plan.chunks.len() {
            if !path.join(format!("chunk-{index:08x}")).try_exists()? {
                return Ok(None);
            }
        }
        let mut reader = PublisherSourceReaderV1 {
            path: &path,
            plan,
            index: 0,
            chunk: io::Cursor::new(Vec::new()),
            deadline: source.source.deadline_epoch,
            current_authority,
            failed: false,
        };
        let manifest_id = self.ingest_manifest_guarded(
            manifest,
            plan,
            &mut reader,
            (None, None),
            &mut |reader| {
                reader.check().map_err(StorageError::Io)?;
                source.check_identity(&path)
            },
        )?;
        reader.check().map_err(StorageError::Io)?;
        source.check_identity(&path)?;
        remove_transaction_directory(&path)?;
        cache.remove(&authorization.order_id());
        reader.check().map_err(StorageError::Io)?;
        Ok(Some(manifest_id))
    }
}

struct PublisherSourceReaderV1<'a, F: FnMut() -> Result<(), StorageError>> {
    path: &'a Path,
    plan: &'a CarBuildPlan,
    index: usize,
    chunk: io::Cursor<Vec<u8>>,
    deadline: u64,
    current_authority: &'a mut F,
    failed: bool,
}
impl<F: FnMut() -> Result<(), StorageError>> PublisherSourceReaderV1<'_, F> {
    fn check(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("publisher source authority unavailable"));
        }
        let result = (self.current_authority)().and_then(|()| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| publisher_source_rejected())?
                .as_secs();
            if now > self.deadline {
                return Err(publisher_source_rejected());
            }
            Ok(())
        });
        if let Err(error) = result {
            self.failed = true;
            return Err(io::Error::other(error));
        }
        Ok(())
    }
}
impl<F: FnMut() -> Result<(), StorageError>> Read for PublisherSourceReaderV1<'_, F> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.chunk.read(output)?;
            if read != 0 {
                return Ok(read);
            }
            let Some(chunk) = self.plan.chunks.get(self.index) else {
                return Ok(0);
            };
            // Release the exhausted allocation before reading the next bounded chunk.
            self.chunk = io::Cursor::new(Vec::new());
            let bytes = read_bounded_regular_file(
                &self.path.join(format!("chunk-{:08x}", self.index)),
                u64::from(chunk.length),
            )
            .map_err(io::Error::other)?;
            if bytes.len() != chunk.length as usize
                || blake3::hash(&bytes).as_bytes() != &chunk.digest
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "publisher source chunk rejected",
                ));
            }
            self.index += 1;
            self.chunk = io::Cursor::new(bytes);
            self.check()?;
        }
    }
}
