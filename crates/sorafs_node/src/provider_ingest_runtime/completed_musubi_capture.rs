/// One unsealed projected assignment supplied to the capture scanner.
///
/// This value is deliberately opaque and has no wire codec. Constructing it
/// does not confer finalized authority: the scanner validates the complete
/// page, then uses its private claim factory to seal the raw archive binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureSourceRowV1 {
    pin: PinManifestFinalizedRecordV1,
    order: ReplicationOrderRecord,
    musubi_archive: Option<MusubiReplicationOrderArchiveBindingV1>,
    provider_owner: Option<AccountId>,
    completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
    completion_epoch: Option<u64>,
    committed_transaction_hash: Option<[u8; 32]>,
}
impl ProviderIngestCompletedMusubiCaptureSourceRowV1 {
    /// Package one untrusted archive projection for scanner-side validation.
    ///
    /// No field is validated here. This constructor intentionally returns no
    /// opaque claim and cannot be used as finalized evidence on its own.
    #[must_use]
    pub fn from_projected_fields(
        pin: PinManifestFinalizedRecordV1,
        order: ReplicationOrderRecord,
        musubi_archive: Option<MusubiReplicationOrderArchiveBindingV1>,
        provider_owner: Option<AccountId>,
        completion_authority: Option<ProviderIngestCompletionAuthorityV1>,
        completion_epoch: Option<u64>,
        committed_transaction_hash: Option<[u8; 32]>,
    ) -> Self {
        Self {
            pin,
            order,
            musubi_archive,
            provider_owner,
            completion_authority,
            completion_epoch,
            committed_transaction_hash,
        }
    }
}
/// One unsealed, replayable page supplied to the capture scanner.
///
/// The private fields and lack of a codec keep this projection distinct from opaque finalized
/// evidence. The scanner commits no cursor progress until it has checked the page identity, bounds,
/// ordering, every row, and every claim that it seals from the raw bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureSourcePageV1 {
    network_id: NetworkId,
    provider_id: [u8; 32],
    finalized_cursor: ProviderIngestFinalizedCursorV1,
    finalized_block_time_ms: u64,
    rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
    next_after_order_id: Option<[u8; 32]>,
}
impl ProviderIngestCompletedMusubiCaptureSourcePageV1 {
    /// Package one untrusted archive page for scanner-side validation.
    ///
    /// No field is validated here. In particular, this constructor cannot
    /// mint or carry a completed-row claim.
    #[must_use]
    pub fn from_projected_fields(
        network_id: NetworkId,
        provider_id: [u8; 32],
        finalized_cursor: ProviderIngestFinalizedCursorV1,
        finalized_block_time_ms: u64,
        rows: Vec<ProviderIngestCompletedMusubiCaptureSourceRowV1>,
        next_after_order_id: Option<[u8; 32]>,
    ) -> Self {
        Self {
            network_id,
            provider_id,
            finalized_cursor,
            finalized_block_time_ms,
            rows,
            next_after_order_id,
        }
    }
    /// Return the projected finalized cursor without conferring authority.
    #[must_use]
    pub const fn finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.finalized_cursor
    }
    /// Return the exclusive continuation boundary, when another page exists.
    #[must_use]
    pub const fn next_after_order_id(&self) -> Option<[u8; 32]> {
        self.next_after_order_id
    }
}
/// Immutable public verifier identity for one daemon-owned capture session.
///
/// This private-field value is untrusted transport data, has no wire codec, and confers no
/// finalized authority by itself. A capture scanner pins one exact binding before its first read
/// and verifies every response with the bound Ed25519 key. The reader generation is an immutable
/// epoch derived for the ephemeral signer session; it is distinct from both archive health and the
/// scanner-owned request generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureVerifierBindingV1 {
    session_id: [u8; 32],
    network_id: NetworkId,
    provider_id: [u8; 32],
    reader_generation: u64,
    public_key: [u8; 32],
}
impl ProviderIngestCompletedMusubiCaptureVerifierBindingV1 {
    /// Construct untrusted reader binding material for cross-crate transport.
    ///
    /// This helper validates structure and derives the session identifier, but does not prove that
    /// the supplied key belongs to an authoritative reader. Only the crate-private scanner
    /// construction path may decide which exact binding to pin.
    ///
    /// # Errors
    ///
    /// Rejects an invalid network, provider, generation, or Ed25519 key,
    /// or a canonical session commitment that cannot be encoded.
    #[doc(hidden)]
    pub fn try_from_untrusted_reader_parts(
        network_id: NetworkId,
        provider_id: [u8; 32],
        reader_generation: u64,
        public_key: [u8; 32],
    ) -> Result<Self, ProviderIngestFinalizedLedgerErrorV1> {
        let session_id = completed_musubi_capture_session_id(
            network_id,
            provider_id,
            reader_generation,
            public_key,
        )?;
        let binding = Self {
            session_id,
            network_id,
            provider_id,
            reader_generation,
            public_key,
        };
        binding.validate()?;
        Ok(binding)
    }
    fn validate(&self) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
        if self.network_id.as_bytes()[31] & 1 == 0
            || self.provider_id == [0; 32]
            || self.reader_generation == 0
            || self.public_key == [0; 32]
            || PublicKey::from_bytes(Algorithm::Ed25519, &self.public_key).is_err()
            || self.session_id
                != completed_musubi_capture_session_id(
                    self.network_id,
                    self.provider_id,
                    self.reader_generation,
                    self.public_key,
                )?
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        Ok(())
    }
    /// Exact genesis-derived network identity pinned by this session.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Exact local provider identity pinned by this session.
    #[must_use]
    pub const fn provider_id(&self) -> [u8; 32] {
        self.provider_id
    }
    /// Immutable generation of this ephemeral reader session.
    #[must_use]
    pub const fn reader_generation(&self) -> u64 {
        self.reader_generation
    }
    /// Derived identity of this ephemeral reader session.
    #[must_use]
    pub const fn session_id(&self) -> [u8; 32] {
        self.session_id
    }
    /// Exact Ed25519 verification key for this reader session.
    #[must_use]
    pub const fn public_key(&self) -> [u8; 32] {
        self.public_key
    }
}
/// Exact bounded read request authenticated by one signed capture response.
///
/// The scanner owns `generation`. It prepares the current non-zero value before awaiting the reader
/// and advances it only after signature and full page validation succeed. Thus cancellation or
/// validation failure retries the exact request, while an old successful response cannot be
/// replayed into a later scanner step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiCaptureRequestV1 {
    binding: ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
    at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    after_order_id: Option<[u8; 32]>,
    limit: u16,
    generation: u64,
}
impl ProviderIngestCompletedMusubiCaptureRequestV1 {
    fn new(
        binding: ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: u16,
        generation: u64,
    ) -> Result<Self, ProviderIngestFinalizedLedgerErrorV1> {
        let request = Self {
            binding,
            at_finalized_cursor,
            after_order_id,
            limit,
            generation,
        };
        request.validate()?;
        Ok(request)
    }
    /// Construct structurally checked but otherwise untrusted request parts.
    ///
    /// This helper exists for the private cross-crate daemon reader and its
    /// tests. It does not prove that the binding is authoritative, advance a
    /// scanner generation, or confer any claim-minting capability.
    ///
    /// # Errors
    ///
    /// Rejects malformed binding material, a zero or over-limit request, a
    /// mismatched cursor/continuation option pair, or a zero generation.
    #[doc(hidden)]
    pub fn try_from_untrusted_reader_parts(
        binding: ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: u16,
        generation: u64,
    ) -> Result<Self, ProviderIngestFinalizedLedgerErrorV1> {
        Self::new(
            binding,
            at_finalized_cursor,
            after_order_id,
            limit,
            generation,
        )
    }
    fn validate(&self) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
        self.binding.validate()?;
        if self.generation == 0
            || self.limit == 0
            || usize::from(self.limit) > PROVIDER_INGEST_STATUS_PAGE_MAX_V1
            || self.at_finalized_cursor.is_none() != self.after_order_id.is_none()
            || self
                .at_finalized_cursor
                .is_some_and(|cursor| cursor.height == 0 || cursor.block_hash == [0; 32])
            || self
                .after_order_id
                .is_some_and(|order_id| order_id == [0; 32])
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        Ok(())
    }
    /// Borrow the exact verifier binding repeated in this request.
    #[must_use]
    pub const fn binding(&self) -> &ProviderIngestCompletedMusubiCaptureVerifierBindingV1 {
        &self.binding
    }
    /// Exact immutable finalized cursor for a continuation, if any.
    #[must_use]
    pub const fn at_finalized_cursor(&self) -> Option<ProviderIngestFinalizedCursorV1> {
        self.at_finalized_cursor
    }
    /// Exact exclusive order boundary for a continuation, if any.
    #[must_use]
    pub const fn after_order_id(&self) -> Option<[u8; 32]> {
        self.after_order_id
    }
    /// Checked platform-independent row bound.
    #[must_use]
    pub const fn limit(&self) -> u16 {
        self.limit
    }
    /// Scanner-owned non-zero request generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
}
/// Untrusted signed response from one completed-Musubi capture reader.
///
/// The envelope and contained projection have no wire codec and no public raw page accessor.
/// Construction does not validate the signature or confer authority; the scanner verifies the exact
/// request-bound transcript against its pinned binding before inspecting or sealing the projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiSignedCapturePageV1 {
    request: ProviderIngestCompletedMusubiCaptureRequestV1,
    source_page: ProviderIngestCompletedMusubiCaptureSourcePageV1,
    signature: [u8; 64],
}
impl ProviderIngestCompletedMusubiSignedCapturePageV1 {
    /// Package untrusted cross-crate response parts without validating them.
    #[doc(hidden)]
    #[must_use]
    pub fn from_untrusted_reader_parts(
        request: ProviderIngestCompletedMusubiCaptureRequestV1,
        source_page: ProviderIngestCompletedMusubiCaptureSourcePageV1,
        signature: [u8; 64],
    ) -> Self {
        Self {
            request,
            source_page,
            signature,
        }
    }
}
/// Request-bound signed finalized-ledger boundary used only by the capture scanner.
///
/// Implementations receive no claim factory and return no opaque claim. An exact retry must
/// reconstruct and sign the same immutable page without consuming adapter-local continuation state.
/// The scanner pins [`Self::capture_verifier_binding`] once, verifies the canonical transcript,
/// validates every unsealed field, and only then seals completed-row claims.
pub trait ProviderIngestCompletedMusubiSignedCaptureLedgerV1: Send + Sync + 'static {
    /// Return this reader's immutable ephemeral session verifier binding.
    ///
    /// # Errors
    ///
    /// Returns a typed rejection or unavailability error when the exact
    /// private reader session cannot supply a valid binding.
    fn capture_verifier_binding(
        &self,
    ) -> Result<
        ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
        ProviderIngestFinalizedLedgerErrorV1,
    >;
    /// Reconstruct and sign one exact bounded unsealed page without consuming it.
    fn read_signed_completed_musubi_capture_page<'a>(
        &'a self,
        request: ProviderIngestCompletedMusubiCaptureRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            ProviderIngestCompletedMusubiSignedCapturePageV1,
            ProviderIngestFinalizedLedgerErrorV1,
        >,
    >;
}
#[derive(NoritoSerialize)]
struct ProviderIngestCompletedMusubiCaptureSessionMaterialV1 {
    version: u8,
    network_id: NetworkId,
    provider_id: [u8; 32],
    reader_generation: u64,
    public_key: [u8; 32],
}
#[derive(NoritoSerialize)]
struct ProviderIngestCompletedMusubiCaptureRequestMaterialV1 {
    version: u8,
    session_id: [u8; 32],
    network_id: NetworkId,
    provider_id: [u8; 32],
    reader_generation: u64,
    public_key: [u8; 32],
    at_finalized_height: Option<u64>,
    at_finalized_block_hash: Option<[u8; 32]>,
    after_order_id: Option<[u8; 32]>,
    limit: u16,
    generation: u64,
}
#[derive(NoritoSerialize)]
struct ProviderIngestCompletedMusubiCapturePageHeaderMaterialV1 {
    network_id: NetworkId,
    provider_id: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    finalized_block_time_ms: u64,
    row_count: u16,
}
fn completed_musubi_capture_session_id(
    network_id: NetworkId,
    provider_id: [u8; 32],
    reader_generation: u64,
    public_key: [u8; 32],
) -> Result<[u8; 32], ProviderIngestFinalizedLedgerErrorV1> {
    if network_id.as_bytes()[31] & 1 == 0
        || provider_id == [0; 32]
        || reader_generation == 0
        || public_key == [0; 32]
        || PublicKey::from_bytes(Algorithm::Ed25519, &public_key).is_err()
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let canonical =
        norito::encode_canonical(&ProviderIngestCompletedMusubiCaptureSessionMaterialV1 {
            version: PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_VERSION_V1,
            network_id,
            provider_id,
            reader_generation,
            public_key,
        })
        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let domain_len =
        u64::try_from(PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_SESSION_DOMAIN_V1.len())
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let canonical_len = u64::try_from(canonical.len())
        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&domain_len.to_be_bytes());
    hasher.update(PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_SESSION_DOMAIN_V1);
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}
fn completed_musubi_capture_request_material(
    request: &ProviderIngestCompletedMusubiCaptureRequestV1,
) -> ProviderIngestCompletedMusubiCaptureRequestMaterialV1 {
    let ProviderIngestCompletedMusubiCaptureRequestV1 {
        binding,
        at_finalized_cursor,
        after_order_id,
        limit,
        generation,
    } = request;
    let ProviderIngestCompletedMusubiCaptureVerifierBindingV1 {
        session_id,
        network_id,
        provider_id,
        reader_generation,
        public_key,
    } = binding;
    ProviderIngestCompletedMusubiCaptureRequestMaterialV1 {
        version: PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_VERSION_V1,
        session_id: *session_id,
        network_id: *network_id,
        provider_id: *provider_id,
        reader_generation: *reader_generation,
        public_key: *public_key,
        at_finalized_height: at_finalized_cursor.map(|cursor| cursor.height),
        at_finalized_block_hash: at_finalized_cursor.map(|cursor| cursor.block_hash),
        after_order_id: *after_order_id,
        limit: *limit,
        generation: *generation,
    }
}
fn completed_musubi_capture_page_header_material(
    page: &ProviderIngestCompletedMusubiCaptureSourcePageV1,
) -> Result<
    ProviderIngestCompletedMusubiCapturePageHeaderMaterialV1,
    ProviderIngestFinalizedLedgerErrorV1,
> {
    let ProviderIngestCompletedMusubiCaptureSourcePageV1 {
        network_id,
        provider_id,
        finalized_cursor,
        finalized_block_time_ms,
        rows,
        next_after_order_id: _,
    } = page;
    Ok(ProviderIngestCompletedMusubiCapturePageHeaderMaterialV1 {
        network_id: *network_id,
        provider_id: *provider_id,
        finalized_height: finalized_cursor.height,
        finalized_block_hash: finalized_cursor.block_hash,
        finalized_block_time_ms: *finalized_block_time_ms,
        row_count: u16::try_from(rows.len())
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?,
    })
}
fn update_completed_musubi_capture_transcript_field<T: norito::core::NoritoSerialize>(
    hasher: &mut blake3::Hasher,
    total_bytes: &mut usize,
    field_tag: u16,
    row_index: u32,
    value: &T,
) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
    const FRAME_BYTES: usize =
        std::mem::size_of::<u16>() + std::mem::size_of::<u32>() + std::mem::size_of::<u64>();
    let predicted_len = {
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::core::encoded_frame_len(value)
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?
    };
    if predicted_len > PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_ROW_MAX_CANONICAL_BYTES_V1 {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let predicted_framed_len = FRAME_BYTES
        .checked_add(predicted_len)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let predicted_total = total_bytes
        .checked_add(predicted_framed_len)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    if predicted_total > PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_MAX_CANONICAL_BYTES_V1
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let canonical = norito::encode_canonical(value)
        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    if canonical.len() != predicted_len
        || canonical.len() > PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_ROW_MAX_CANONICAL_BYTES_V1
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let framed_len = FRAME_BYTES
        .checked_add(canonical.len())
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    *total_bytes = total_bytes
        .checked_add(framed_len)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    if *total_bytes > PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_MAX_CANONICAL_BYTES_V1 {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let canonical_len = u64::try_from(canonical.len())
        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    hasher.update(&field_tag.to_be_bytes());
    hasher.update(&row_index.to_be_bytes());
    hasher.update(&canonical_len.to_be_bytes());
    hasher.update(&canonical);
    Ok(())
}
fn update_completed_musubi_capture_transcript_row(
    hasher: &mut blake3::Hasher,
    total_bytes: &mut usize,
    row_index: u32,
    row: &ProviderIngestCompletedMusubiCaptureSourceRowV1,
) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
    let ProviderIngestCompletedMusubiCaptureSourceRowV1 {
        pin,
        order,
        musubi_archive,
        provider_owner,
        completion_authority,
        completion_epoch,
        committed_transaction_hash,
    } = row;
    update_completed_musubi_capture_transcript_field(hasher, total_bytes, 10, row_index, pin)?;
    update_completed_musubi_capture_transcript_field(hasher, total_bytes, 11, row_index, order)?;
    update_completed_musubi_capture_transcript_field(
        hasher,
        total_bytes,
        12,
        row_index,
        musubi_archive,
    )?;
    update_completed_musubi_capture_transcript_field(
        hasher,
        total_bytes,
        13,
        row_index,
        provider_owner,
    )?;
    update_completed_musubi_capture_transcript_field(
        hasher,
        total_bytes,
        14,
        row_index,
        completion_authority,
    )?;
    update_completed_musubi_capture_transcript_field(
        hasher,
        total_bytes,
        15,
        row_index,
        completion_epoch,
    )?;
    update_completed_musubi_capture_transcript_field(
        hasher,
        total_bytes,
        16,
        row_index,
        committed_transaction_hash,
    )?;
    Ok(())
}
fn validate_completed_musubi_capture_transcript_bounds(
    request: &ProviderIngestCompletedMusubiCaptureRequestV1,
    page: &ProviderIngestCompletedMusubiCaptureSourcePageV1,
) -> Result<(), ProviderIngestFinalizedLedgerErrorV1> {
    request.validate()?;
    if page.network_id.as_bytes()[31] & 1 == 0
        || page.provider_id == [0; 32]
        || page.finalized_cursor.height == 0
        || page.finalized_cursor.block_hash == [0; 32]
        || page.finalized_block_time_ms == 0
        || page.rows.len() > usize::from(request.limit)
        || page.rows.len() > PROVIDER_INGEST_STATUS_PAGE_MAX_V1
        || page.rows.iter().any(|row| {
            row.order.canonical_order.is_empty()
                || row.order.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
        })
    {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let mut canonical_order_bytes = 0_usize;
    for row in &page.rows {
        canonical_order_bytes = canonical_order_bytes
            .checked_add(row.order.canonical_order.len())
            .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        if canonical_order_bytes
            > PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_MAX_CANONICAL_BYTES_V1
            || row.order.provider_completions.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS
            || row
                .musubi_archive
                .as_ref()
                .is_some_and(|binding| binding.validate().is_err())
            || row
                .completion_authority
                .as_ref()
                .is_some_and(|authority| !authority.is_valid())
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
    }
    Ok(())
}
/// Compute the exact canonical domain-separated message signed by a capture reader.
///
/// This cross-crate helper treats both inputs as untrusted and grants no
/// authority. It exists solely so the private daemon reader can sign the same
/// bounded transcript that the scanner independently reconstructs.
///
/// # Errors
///
/// Rejects malformed or over-limit request/page material, noncanonical nested values, length
/// overflow, or any component that cannot be canonically encoded within the V1 memory bound.
#[doc(hidden)]
pub fn provider_ingest_completed_musubi_capture_transcript_digest_v1(
    request: &ProviderIngestCompletedMusubiCaptureRequestV1,
    page: &ProviderIngestCompletedMusubiCaptureSourcePageV1,
) -> Result<[u8; 32], ProviderIngestFinalizedLedgerErrorV1> {
    validate_completed_musubi_capture_transcript_bounds(request, page)?;
    let ProviderIngestCompletedMusubiCaptureSourcePageV1 {
        network_id: _,
        provider_id: _,
        finalized_cursor: _,
        finalized_block_time_ms: _,
        rows,
        next_after_order_id,
    } = page;
    let mut hasher = blake3::Hasher::new();
    hasher.update(PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_DOMAIN_V1);
    let mut total_bytes = PROVIDER_INGEST_COMPLETED_MUSUBI_CAPTURE_TRANSCRIPT_DOMAIN_V1.len();
    const NOT_A_ROW: u32 = u32::MAX;
    update_completed_musubi_capture_transcript_field(
        &mut hasher,
        &mut total_bytes,
        1,
        NOT_A_ROW,
        &completed_musubi_capture_request_material(request),
    )?;
    update_completed_musubi_capture_transcript_field(
        &mut hasher,
        &mut total_bytes,
        2,
        NOT_A_ROW,
        &completed_musubi_capture_page_header_material(page)?,
    )?;
    for (index, row) in rows.iter().enumerate() {
        let row_index =
            u32::try_from(index).map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        update_completed_musubi_capture_transcript_row(
            &mut hasher,
            &mut total_bytes,
            row_index,
            row,
        )?;
    }
    update_completed_musubi_capture_transcript_field(
        &mut hasher,
        &mut total_bytes,
        20,
        NOT_A_ROW,
        next_after_order_id,
    )?;
    Ok(*hasher.finalize().as_bytes())
}
/// One validated completed-Musubi candidate emitted by the capture scanner.
///
/// The value has no public constructor or wire codec. Its authorization and opaque completed-row
/// claim were derived together from one validated row of a single finalized page. Equality is
/// semantic and excludes the private process-local authority marker; reconciliation performs a
/// separate exact-instance check.
#[derive(Debug, Clone)]
pub(crate) struct ProviderIngestCompletedMusubiCaptureCandidateV1 {
    authorization: FinalizedProviderIngestAuthorizationV1,
    completed_claim: ProviderIngestFinalizedMusubiCompletionClaimV1,
    completed_musubi_store_instance: CompletedMusubiStoreInstanceV1,
}
impl PartialEq for ProviderIngestCompletedMusubiCaptureCandidateV1 {
    fn eq(&self, other: &Self) -> bool {
        self.authorization == other.authorization && self.completed_claim == other.completed_claim
    }
}
impl Eq for ProviderIngestCompletedMusubiCaptureCandidateV1 {}
impl ProviderIngestCompletedMusubiCaptureCandidateV1 {
    /// Borrow the exact finalized provider-ingest authorization.
    #[must_use]
    pub(crate) const fn authorization(&self) -> &FinalizedProviderIngestAuthorizationV1 {
        &self.authorization
    }
    /// Borrow the opaque local-provider completed-row claim.
    #[must_use]
    pub(crate) const fn completed_claim(&self) -> &ProviderIngestFinalizedMusubiCompletionClaimV1 {
        &self.completed_claim
    }
    pub(crate) fn matches_completed_musubi_store_instance(
        &self,
        expected: &CompletedMusubiStoreInstanceV1,
    ) -> bool {
        self.completed_musubi_store_instance.matches(expected)
            && self
                .completed_claim
                .matches_completed_musubi_store_instance(expected)
    }
}
/// One bounded page emitted by the completed-Musubi capture scanner.
///
/// `scan_complete` means this page exhausted the exact finalized snapshot. The next scanner call
/// then starts a new snapshot and may observe a later finalized head. When that probe still
/// resolves to the last completely scanned head, every bounded row is revalidated but candidates
/// are suppressed and an empty terminal page is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderIngestCompletedMusubiCapturePageV1 {
    finalized_cursor: ProviderIngestFinalizedCursorV1,
    candidates: Vec<ProviderIngestCompletedMusubiCaptureCandidateV1>,
    scan_complete: bool,
}
impl ProviderIngestCompletedMusubiCapturePageV1 {
    /// Return the exact finalized cursor shared by every validated source row.
    #[must_use]
    pub(crate) const fn finalized_cursor(&self) -> ProviderIngestFinalizedCursorV1 {
        self.finalized_cursor
    }
    /// Borrow the completed-Musubi candidates selected from this page.
    #[must_use]
    pub(crate) fn candidates(&self) -> &[ProviderIngestCompletedMusubiCaptureCandidateV1] {
        &self.candidates
    }
    /// Return whether this page exhausted the pinned finalized snapshot.
    #[must_use]
    pub(crate) const fn scan_complete(&self) -> bool {
        self.scan_complete
    }
}
/// Result of verifying and durably enqueuing one bounded capture page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderIngestCompletedMusubiReconcileOutcomeV1 {
    /// Exact finalized cursor shared by the reconciled source rows.
    pub finalized_cursor: ProviderIngestFinalizedCursorV1,
    /// Number of completed-Musubi candidates reverified under storage leases.
    pub candidates: usize,
    /// Number of newly inserted approval intents.
    pub inserted: usize,
    /// Number of exact approval intents already retained by the journal.
    pub existing: usize,
    /// Number of exact payloads already retained by authenticated inventory.
    pub inventory_suppressed: usize,
    /// Whether this page exhausted its immutable finalized snapshot.
    pub scan_complete: bool,
}
/// Path-free failure while reconciling one completed-Musubi capture page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum ProviderIngestCompletedMusubiReconcileErrorV1 {
    /// The replay-safe scanner could not reach its finalized reader.
    #[error("completed-Musubi finalized capture is unavailable")]
    CaptureUnavailable,
    /// The replay-safe scanner or its projected page failed validation.
    #[error("completed-Musubi finalized capture failed")]
    Capture,
    /// The admitted manifest or its exact stored CAR plan was unavailable.
    #[error("completed-Musubi admitted payload plan is unavailable")]
    AdmittedPlanUnavailable,
    /// Lifecycle-leased bundle verification rejected or could not read the payload.
    #[error("completed-Musubi admitted payload verification failed")]
    VerificationFailed,
    /// The admitted payload could not be read under its lifecycle lease.
    #[error("completed-Musubi admitted payload is temporarily unavailable")]
    VerificationUnavailable,
    /// A different retained intent occupies the immutable attestation key.
    #[error("completed-Musubi approval admission conflicts with retained intent")]
    AdmissionConflict,
    /// The authenticated coordinator inventory was unavailable or timed out.
    #[error("completed-Musubi provider-attestation inventory is unavailable")]
    InventoryUnavailable,
    /// Inventory qualification or the exact readback was rejected.
    #[error("completed-Musubi provider-attestation inventory rejected admission")]
    InventoryRejected,
    /// The durable approval-intent journal rejected or could not persist the request.
    #[error("completed-Musubi approval intent could not be enqueued")]
    JournalUnavailable,
    /// The bounded durable journal has no room for another identity.
    #[error("completed-Musubi approval journal capacity is exhausted")]
    CapacityExceeded,
}
/// Take-once daemon tenure for completed-Musubi finalized capture.
///
/// [`crate::NodeHandle`] reserves this opaque value once for one exact process-local storage/outbox
/// incarnation. Construction retains one erased signed reader without consulting it, so a valid
/// height-zero bootstrap can remain pending until genesis exists. Lazy binding always retries that
/// same retained reader; dropping this value never resets the shared reservation.
///
/// This first-release slice deliberately exposes no page, claim, approval, or effect-driving
/// operation. The private daemon composer may retain the tenure, but cannot use it to start
/// signing, journal mutation, inventory mutation, transaction submission, or registry mutation.
#[doc(hidden)]
#[must_use = "dropping the capture tenure permanently consumes this store instance's reservation"]
pub struct ProviderIngestCompletedMusubiCaptureCoordinatorV1 {
    state: ProviderIngestCompletedMusubiCaptureCoordinatorStateV1,
}
#[expect(clippy::large_enum_variant, reason = "by-value take-once state")]
enum ProviderIngestCompletedMusubiCaptureCoordinatorStateV1 {
    Pending(ProviderIngestCompletedMusubiCapturePendingV1),
    Active(
        ProviderIngestCompletedMusubiCaptureScannerV1<
            dyn ProviderIngestCompletedMusubiSignedCaptureLedgerV1,
        >,
    ),
}
struct ProviderIngestCompletedMusubiCapturePendingV1 {
    completed_musubi_store_instance: CompletedMusubiStoreInstanceV1,
    provider_id: [u8; 32],
    network_id: NetworkId,
    max_page_rows: usize,
    ledger: Arc<dyn ProviderIngestCompletedMusubiSignedCaptureLedgerV1>,
}
impl fmt::Debug for ProviderIngestCompletedMusubiCaptureCoordinatorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match &self.state {
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Pending(_) => "pending",
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Active(_) => "active",
        };
        formatter
            .debug_struct("ProviderIngestCompletedMusubiCaptureCoordinatorV1")
            .field("state", &state)
            .finish_non_exhaustive()
    }
}
impl ProviderIngestCompletedMusubiCaptureCoordinatorV1 {
    pub(crate) fn new_pending(
        completed_musubi_store_instance: CompletedMusubiStoreInstanceV1,
        provider_id: [u8; 32],
        network_id: NetworkId,
        max_page_rows: usize,
        ledger: Arc<dyn ProviderIngestCompletedMusubiSignedCaptureLedgerV1>,
    ) -> Result<Self, ProviderIngestRuntimeErrorV1> {
        validate_completed_musubi_capture_scanner_identity(provider_id, network_id, max_page_rows)?;
        Ok(Self {
            state: ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Pending(
                ProviderIngestCompletedMusubiCapturePendingV1 {
                    completed_musubi_store_instance,
                    provider_id,
                    network_id,
                    max_page_rows,
                    ledger,
                },
            ),
        })
    }
    /// Try to bind the retained reader without permitting reader substitution.
    ///
    /// Unavailability leaves the exact pending reader and all static identity material in place for
    /// a later retry. This method remains crate-private until the qualified journal/inventory
    /// coordinator can consume scanner output without exposing claims or requests.
    // The opaque attestation driver is the sole production caller and invokes
    // this after binding its journal, signer, and inventory. Stock daemon
    // startup retains that driver inert until supervision is qualified.
    #[allow(
        dead_code,
        reason = "activation stays closed until the qualified effect coordinator is complete"
    )]
    pub(crate) fn try_activate(&mut self) -> Result<(), ProviderIngestRuntimeErrorV1> {
        let ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Pending(pending) = &self.state
        else {
            return Ok(());
        };
        let scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            pending.completed_musubi_store_instance.clone(),
            pending.provider_id,
            pending.network_id,
            pending.max_page_rows,
            Arc::clone(&pending.ledger),
        )?;
        self.state = ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Active(scanner);
        Ok(())
    }
    pub(crate) fn active_scanner_mut(
        &mut self,
    ) -> Option<
        &mut ProviderIngestCompletedMusubiCaptureScannerV1<
            dyn ProviderIngestCompletedMusubiSignedCaptureLedgerV1,
        >,
    > {
        match &mut self.state {
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Pending(_) => None,
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Active(scanner) => {
                Some(scanner)
            }
        }
    }
    fn binding(&self) -> (NetworkId, [u8; 32], usize, &CompletedMusubiStoreInstanceV1) {
        match &self.state {
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Pending(pending) => (
                pending.network_id,
                pending.provider_id,
                pending.max_page_rows,
                &pending.completed_musubi_store_instance,
            ),
            ProviderIngestCompletedMusubiCaptureCoordinatorStateV1::Active(scanner) => (
                scanner.network_id,
                scanner.provider_id,
                scanner.max_page_rows,
                &scanner.completed_musubi_store_instance,
            ),
        }
    }
}
fn validate_completed_musubi_capture_scanner_identity(
    provider_id: [u8; 32],
    network_id: NetworkId,
    max_page_rows: usize,
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if provider_id == [0; 32] {
        return Err(ProviderIngestRuntimeErrorV1::InvalidProviderId);
    }
    if network_id.as_bytes()[31] & 1 == 0 {
        return Err(ProviderIngestRuntimeErrorV1::InvalidNetworkId);
    }
    if max_page_rows == 0 || max_page_rows > PROVIDER_INGEST_STATUS_PAGE_MAX_V1 {
        return Err(ProviderIngestRuntimeErrorV1::InvalidPolicy);
    }
    Ok(())
}
/// Opaque bounded scanner for finalized local-provider Musubi completions.
///
/// Only [`crate::NodeHandle`] can construct this scanner. The signed reader never receives a claim
/// factory or opaque claim. The scanner first verifies the request-bound transcript under its
/// pinned session key, then validates the complete unsealed projection, privately creates the
/// factory, seals and revalidates every assignment, owns its continuation cursor, and exposes only
/// the authorization plus opaque completed claim needed by a later capture-only verifier. It
/// performs no storage, signing, journal, inventory, or registry mutation. After a complete scan it
/// performs only one bounded validating probe at an unchanged finalized head and resumes ordinary
/// paging once the head advances.
pub(crate) struct ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    completed_musubi_store_instance: CompletedMusubiStoreInstanceV1,
    provider_id: [u8; 32],
    network_id: NetworkId,
    max_page_rows: usize,
    ledger: Arc<Ledger>,
    verifier_binding: ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
    request_generation: u64,
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    last_completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderIngestCompletedMusubiCaptureProgressV1 {
    request_generation: u64,
    scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    scan_after_order_id: Option<[u8; 32]>,
    last_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
    last_completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}
/// Cancellation-safe rollback guard for one reconciled capture page.
///
/// The scanner commits a fully authenticated page before inventory and journal admission begin.
/// Retaining this guard across those later awaits restores the exact prior generation and cursors
/// whenever the reconciliation future is dropped or returns early. Call [`Self::commit`] only after
/// every candidate has been durably reconciled.
#[must_use = "dropping the guard restores the scanner's prior progress"]
pub(crate) struct ProviderIngestCompletedMusubiCaptureProgressRollbackV1<'a, Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    scanner: Option<&'a mut ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>>,
    progress: ProviderIngestCompletedMusubiCaptureProgressV1,
}
impl<Ledger> ProviderIngestCompletedMusubiCaptureProgressRollbackV1<'_, Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    /// Keep the scanner's newly committed page progress.
    pub(crate) fn commit(mut self) {
        self.scanner = None;
    }
}
impl<Ledger> Drop for ProviderIngestCompletedMusubiCaptureProgressRollbackV1<'_, Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    fn drop(&mut self) {
        if let Some(scanner) = self.scanner.take() {
            scanner.restore_progress(self.progress);
        }
    }
}
struct ValidatedCompletedMusubiCaptureScanPageV1 {
    finalized_cursor: ProviderIngestFinalizedCursorV1,
    candidates: Option<Vec<ProviderIngestCompletedMusubiCaptureCandidateV1>>,
    next_after_order_id: Option<[u8; 32]>,
}
impl<Ledger> fmt::Debug for ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestCompletedMusubiCaptureScannerV1")
            .field("provider_id", &self.provider_id)
            .field("network_id", &self.network_id)
            .field("max_page_rows", &self.max_page_rows)
            .field("capture_session_id", &self.verifier_binding.session_id)
            .field("request_generation", &self.request_generation)
            .field("scan_cursor", &self.scan_cursor)
            .field("scan_after_order_id", &self.scan_after_order_id)
            .field("last_finalized_cursor", &self.last_finalized_cursor)
            .field("last_completed_cursor", &self.last_completed_cursor)
            .finish_non_exhaustive()
    }
}
impl<Ledger> ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    pub(crate) fn new(
        completed_musubi_store_instance: CompletedMusubiStoreInstanceV1,
        provider_id: [u8; 32],
        network_id: NetworkId,
        max_page_rows: usize,
        ledger: Arc<Ledger>,
    ) -> Result<Self, ProviderIngestRuntimeErrorV1> {
        validate_completed_musubi_capture_scanner_identity(provider_id, network_id, max_page_rows)?;
        let verifier_binding = ledger
            .capture_verifier_binding()
            .map_err(map_capture_ledger_error)?;
        verifier_binding
            .validate()
            .map_err(map_capture_ledger_error)?;
        if verifier_binding.provider_id != provider_id || verifier_binding.network_id != network_id
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
        Ok(Self {
            completed_musubi_store_instance,
            provider_id,
            network_id,
            max_page_rows,
            ledger,
            verifier_binding,
            request_generation: 1,
            scan_cursor: None,
            scan_after_order_id: None,
            last_finalized_cursor: None,
            last_completed_cursor: None,
        })
    }
    pub(crate) const fn progress(&self) -> ProviderIngestCompletedMusubiCaptureProgressV1 {
        ProviderIngestCompletedMusubiCaptureProgressV1 {
            request_generation: self.request_generation,
            scan_cursor: self.scan_cursor,
            scan_after_order_id: self.scan_after_order_id,
            last_finalized_cursor: self.last_finalized_cursor,
            last_completed_cursor: self.last_completed_cursor,
        }
    }
    pub(crate) fn matches_completed_musubi_store_instance(
        &self,
        expected: &CompletedMusubiStoreInstanceV1,
    ) -> bool {
        self.completed_musubi_store_instance.matches(expected)
    }
    pub(crate) fn restore_progress(
        &mut self,
        progress: ProviderIngestCompletedMusubiCaptureProgressV1,
    ) {
        self.request_generation = progress.request_generation;
        self.scan_cursor = progress.scan_cursor;
        self.scan_after_order_id = progress.scan_after_order_id;
        self.last_finalized_cursor = progress.last_finalized_cursor;
        self.last_completed_cursor = progress.last_completed_cursor;
    }
    /// Restore `progress` unless the returned guard is explicitly committed.
    pub(crate) fn restore_progress_on_drop(
        &mut self,
        progress: ProviderIngestCompletedMusubiCaptureProgressV1,
    ) -> ProviderIngestCompletedMusubiCaptureProgressRollbackV1<'_, Ledger> {
        ProviderIngestCompletedMusubiCaptureProgressRollbackV1 {
            scanner: Some(self),
            progress,
        }
    }
    fn validate_and_seal_source_page(
        &self,
        source_page: ProviderIngestCompletedMusubiCaptureSourcePageV1,
        after_order_id: Option<[u8; 32]>,
        expected_cursor: Option<ProviderIngestFinalizedCursorV1>,
    ) -> Result<ValidatedCompletedMusubiCaptureScanPageV1, ProviderIngestRuntimeErrorV1> {
        validate_completed_musubi_capture_source_page(
            &source_page,
            after_order_id,
            expected_cursor,
            self.max_page_rows,
            self.network_id,
            self.provider_id,
        )?;
        let page = seal_completed_musubi_capture_source_page(
            source_page,
            &ProviderIngestFinalizedClaimFactoryV1::new_completed_musubi_capture(
                self.network_id,
                self.provider_id,
                self.completed_musubi_store_instance.clone(),
            ),
            self.network_id,
            self.provider_id,
        )?;
        if after_order_id.is_some()
            && expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
        }
        let finalized_cursor = expected_cursor.unwrap_or(page.finalized_cursor);
        validate_page(&page, after_order_id, finalized_cursor, self.max_page_rows)?;
        validate_monotonic_finalized_cursor(self.last_finalized_cursor, finalized_cursor)?;
        let suppress_unchanged_head =
            expected_cursor.is_none() && self.last_completed_cursor == Some(finalized_cursor);
        let mut candidates = Vec::new();
        candidates
            .try_reserve(page.rows.len())
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
        for row in &page.rows {
            let validated = validate_assignment_with_source_bound(
                row,
                finalized_cursor,
                self.provider_id,
                &self.network_id,
                MAX_REPLICATION_ORDER_ASSIGNMENTS,
            )?;
            if let Some(completed_claim) = row.completed_musubi_archive.as_ref() {
                if !completed_claim
                    .matches_completed_musubi_store_instance(&self.completed_musubi_store_instance)
                    || !completed_claim.matches_authorization(&validated.authorization)
                {
                    return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
                }
                candidates.push(ProviderIngestCompletedMusubiCaptureCandidateV1 {
                    authorization: validated.authorization,
                    completed_claim: completed_claim.clone(),
                    completed_musubi_store_instance: self.completed_musubi_store_instance.clone(),
                });
            }
        }
        Ok(ValidatedCompletedMusubiCaptureScanPageV1 {
            finalized_cursor,
            candidates: (!suppress_unchanged_head).then_some(candidates),
            next_after_order_id: page.next_after_order_id,
        })
    }
    /// Read and validate the next bounded page from the pinned finalized scan.
    ///
    /// A terminal page resets only the private continuation state. The monotonic finalized
    /// high-water remains retained, so the next fresh scan may stay at the same head or advance but
    /// can never regress or switch an equal-height hash.
    ///
    /// # Errors
    ///
    /// Returns an error when the finalized reader is unavailable or rejects
    /// the private capability, or when page bounds, cursor lineage, an
    /// assignment, or a sealed claim is malformed or substituted.
    pub async fn next_page(
        &mut self,
    ) -> Result<ProviderIngestCompletedMusubiCapturePageV1, ProviderIngestRuntimeErrorV1> {
        let expected_cursor = self.scan_cursor;
        let after_order_id = self.scan_after_order_id;
        let limit = u16::try_from(self.max_page_rows)
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidPolicy)?;
        let request = ProviderIngestCompletedMusubiCaptureRequestV1::new(
            self.verifier_binding.clone(),
            expected_cursor,
            after_order_id,
            limit,
            self.request_generation,
        )
        .map_err(map_capture_ledger_error)?;
        let signed_page = self
            .ledger
            .read_signed_completed_musubi_capture_page(request.clone())
            .await
            .map_err(map_capture_ledger_error)?;
        let source_page = verify_completed_musubi_signed_capture_page(
            signed_page,
            &request,
            &self.verifier_binding,
        )?;
        let validation =
            self.validate_and_seal_source_page(source_page, after_order_id, expected_cursor);
        let (finalized_cursor, candidates, next_after_order_id) = match validation {
            Ok(ValidatedCompletedMusubiCaptureScanPageV1 {
                finalized_cursor,
                candidates: Some(candidates),
                next_after_order_id,
            }) => (finalized_cursor, candidates, next_after_order_id),
            Ok(ValidatedCompletedMusubiCaptureScanPageV1 {
                finalized_cursor,
                candidates: None,
                ..
            }) => {
                self.request_generation = self
                    .request_generation
                    .checked_add(1)
                    .ok_or(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
                self.last_finalized_cursor = Some(finalized_cursor);
                return Ok(ProviderIngestCompletedMusubiCapturePageV1 {
                    finalized_cursor,
                    candidates: Vec::new(),
                    scan_complete: true,
                });
            }
            Err(validation_error) => return Err(validation_error),
        };
        let next_request_generation = self
            .request_generation
            .checked_add(1)
            .ok_or(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
        let scan_complete = next_after_order_id.is_none();
        self.last_finalized_cursor = Some(finalized_cursor);
        if scan_complete {
            self.scan_cursor = None;
            self.scan_after_order_id = None;
            self.last_completed_cursor = Some(finalized_cursor);
        } else {
            self.scan_cursor = Some(finalized_cursor);
            self.scan_after_order_id = next_after_order_id;
        }
        self.request_generation = next_request_generation;
        Ok(ProviderIngestCompletedMusubiCapturePageV1 {
            finalized_cursor,
            candidates,
            scan_complete,
        })
    }
}
fn verify_completed_musubi_signed_capture_page(
    signed: ProviderIngestCompletedMusubiSignedCapturePageV1,
    expected_request: &ProviderIngestCompletedMusubiCaptureRequestV1,
    expected_binding: &ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
) -> Result<ProviderIngestCompletedMusubiCaptureSourcePageV1, ProviderIngestRuntimeErrorV1> {
    if &signed.request != expected_request
        || signed.request.binding != *expected_binding
        || signed.request.validate().is_err()
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let digest = provider_ingest_completed_musubi_capture_transcript_digest_v1(
        &signed.request,
        &signed.source_page,
    )
    .map_err(map_capture_ledger_error)?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &expected_binding.public_key)
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    let signature = IrohaSignature::try_from_bytes(&signed.signature)
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    signature
        .verify(&public_key, &digest)
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
    Ok(signed.source_page)
}
fn validate_completed_musubi_capture_source_page(
    page: &ProviderIngestCompletedMusubiCaptureSourcePageV1,
    after_order_id: Option<[u8; 32]>,
    expected_cursor: Option<ProviderIngestFinalizedCursorV1>,
    limit: usize,
    expected_network_id: NetworkId,
    expected_provider_id: [u8; 32],
) -> Result<(), ProviderIngestRuntimeErrorV1> {
    if page.network_id != expected_network_id
        || page.provider_id != expected_provider_id
        || page.finalized_cursor.height == 0
        || page.finalized_cursor.block_hash == [0; 32]
        || page.finalized_block_time_ms == 0
        || expected_cursor.is_some_and(|cursor| cursor != page.finalized_cursor)
        || page.rows.len() > limit
        || page.next_after_order_id.is_some() && page.rows.is_empty()
        || page.next_after_order_id.is_some() && page.rows.len() != limit
    {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut previous = after_order_id;
    for row in &page.rows {
        let order_id = *row.order.order_id.as_bytes();
        if previous.is_some_and(|previous| previous >= order_id)
            || row.pin.finalized_cursor.height != page.finalized_cursor.height
            || row.pin.finalized_cursor.block_hash != page.finalized_cursor.block_hash
            || row.order.assignment_revision == 0
            || row.order.canonical_order.is_empty()
            || row.order.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
            || row
                .committed_transaction_hash
                .is_some_and(|hash| hash == [0; 32])
            || row.completion_authority.as_ref().is_some_and(|authority| {
                !authority.is_valid()
                    || row.provider_owner.as_ref() != Some(&authority.provider_owner)
            })
            || row.provider_owner.is_none() && row.completion_authority.is_some()
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
        let canonical_order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &row.order.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        canonical_order
            .validate()
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        let canonical_bytes = norito::encode_canonical(&canonical_order)
            .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)?;
        if canonical_bytes != row.order.canonical_order
            || canonical_order.order_id != order_id
            || canonical_order.manifest_digest != *row.order.manifest_digest.as_bytes()
            || canonical_order.manifest_cid.as_slice() != row.order.manifest_root_cid.as_bytes()
            || row.pin.manifest.digest != row.order.manifest_digest
            || row.pin.manifest.root_cid != row.order.manifest_root_cid
            || row.pin.manifest.chunker.to_handle() != canonical_order.chunking_profile
            || !canonical_order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == expected_provider_id)
        {
            return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
        }
        match (row.order.musubi_archive, row.musubi_archive.as_ref()) {
            (None, None) => {}
            (Some(archive_id), Some(binding)) => {
                let commitment = &binding.commitment;
                if binding.validate().is_err()
                    || binding.replication_order.as_bytes() != &order_id
                    || binding.archive_id != archive_id
                    || commitment.archive_id() != archive_id
                    || commitment.root_cid != row.pin.manifest.root_cid
                    || commitment.chunker != row.pin.manifest.chunker
                    || commitment.chunk_plan_digest.as_bytes()
                        != &row.pin.manifest.chunk_digest_sha3_256
                    || commitment.por_root.as_bytes() != &row.pin.manifest.por_root
                    || commitment.content_length != row.pin.manifest.content_length
                {
                    return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
                }
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding);
            }
        }
        previous = Some(order_id);
    }
    if page.next_after_order_id.is_some() && page.next_after_order_id != previous {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    Ok(())
}
fn seal_completed_musubi_capture_source_page(
    source: ProviderIngestCompletedMusubiCaptureSourcePageV1,
    claim_factory: &ProviderIngestFinalizedClaimFactoryV1,
    expected_network_id: NetworkId,
    expected_provider_id: [u8; 32],
) -> Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestRuntimeErrorV1> {
    if source.network_id != expected_network_id || source.provider_id != expected_provider_id {
        return Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage);
    }
    let mut rows = Vec::new();
    rows.try_reserve(source.rows.len())
        .map_err(|_| ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)?;
    for source_row in source.rows {
        let order_id = *source_row.order.order_id.as_bytes();
        let musubi_archive = source_row
            .musubi_archive
            .as_ref()
            .map(|binding| {
                claim_factory.seal_musubi_archive(
                    &source.network_id,
                    source.finalized_cursor,
                    order_id,
                    &source_row.pin.manifest,
                    binding.clone(),
                )
            })
            .transpose()
            .map_err(map_capture_ledger_error)?;
        let completed_musubi_archive = match source_row.musubi_archive {
            Some(binding)
                if source_row
                    .order
                    .provider_completion(ProviderId::new(expected_provider_id))
                    .is_some() =>
            {
                Some(
                    claim_factory
                        .seal_completed_musubi_archive(
                            &source.network_id,
                            source.finalized_cursor,
                            ProviderId::new(expected_provider_id),
                            &source_row.order,
                            &source_row.pin.manifest,
                            binding,
                        )
                        .map_err(map_capture_ledger_error)?,
                )
            }
            Some(_) | None => None,
        };
        rows.push(ProviderIngestFinalizedAssignmentV1 {
            pin: source_row.pin,
            order: source_row.order,
            musubi_archive,
            completed_musubi_archive,
            provider_owner: source_row.provider_owner,
            completion_authority: source_row.completion_authority,
            completion_epoch: source_row.completion_epoch,
            committed_transaction_hash: source_row.committed_transaction_hash,
        });
    }
    Ok(ProviderIngestFinalizedAssignmentPageV1 {
        finalized_cursor: source.finalized_cursor,
        finalized_block_time_ms: source.finalized_block_time_ms,
        rows,
        next_after_order_id: source.next_after_order_id,
    })
}
fn map_capture_ledger_error(
    error: ProviderIngestFinalizedLedgerErrorV1,
) -> ProviderIngestRuntimeErrorV1 {
    match error {
        ProviderIngestFinalizedLedgerErrorV1::Unavailable => {
            ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable
        }
        ProviderIngestFinalizedLedgerErrorV1::Rejected => {
            ProviderIngestRuntimeErrorV1::InvalidFinalizedPage
        }
    }
}
use crate::provider_attestation_journal::{
    MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1, MusubiProviderAttestationApprovalIdV1,
    MusubiProviderAttestationClaimOwnerV1, MusubiProviderAttestationDeliveryOutcomeV1,
    MusubiProviderAttestationEnqueueOutcomeV1, MusubiProviderAttestationFailureClassV1,
    MusubiProviderAttestationInventoryRuntimeV1, MusubiProviderAttestationJournalErrorV1,
    MusubiProviderAttestationJournalPolicyV1, MusubiProviderAttestationJournalRuntimeV1,
    MusubiProviderAttestationJournalStageV1, MusubiProviderAttestationJournalV1,
    MusubiProviderAttestationPreEnqueueProbeV1, MusubiProviderAttestationRetryOutcomeV1,
    MusubiProviderAttestationSignerV1, musubi_provider_attestation_approval_id_v1,
};
struct ProviderIngestCompletedMusubiPreparedApprovalV1 {
    approval_id: MusubiProviderAttestationApprovalIdV1,
    request: ProviderIngestMusubiAttestationApprovalRequestV1,
}
/// One page transaction whose drop restores the fresh capture request.
///
/// The guard remains live through approval signing and its durable CAS. It may commit scanner
/// progress only after every retained request is approved or is already in a post-approval state.
struct ProviderIngestCompletedMusubiPreparedPageV1<'a, Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    rollback: ProviderIngestCompletedMusubiCaptureProgressRollbackV1<'a, Ledger>,
    approvals: Vec<ProviderIngestCompletedMusubiPreparedApprovalV1>,
    outcome: ProviderIngestCompletedMusubiReconcileOutcomeV1,
}
impl<Ledger> ProviderIngestCompletedMusubiPreparedPageV1<'_, Ledger>
where
    Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
{
    fn commit(self) -> ProviderIngestCompletedMusubiReconcileOutcomeV1 {
        let Self {
            rollback, outcome, ..
        } = self;
        rollback.commit();
        outcome
    }
}
impl crate::NodeHandle {
    async fn prepare_provider_ingest_completed_musubi_capture_page<'scanner, Ledger, Inventory>(
        &self,
        scanner: &'scanner mut ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>,
        journal: &MusubiProviderAttestationJournalV1,
        inventory: &Inventory,
    ) -> Result<
        ProviderIngestCompletedMusubiPreparedPageV1<'scanner, Ledger>,
        ProviderIngestCompletedMusubiReconcileErrorV1,
    >
    where
        Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
        Inventory: MusubiProviderAttestationInventoryRuntimeV1 + ?Sized,
    {
        let Some(completed_musubi_store_instance) = self.completed_musubi_store_instance.as_ref()
        else {
            return Err(ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed);
        };
        if !scanner.matches_completed_musubi_store_instance(completed_musubi_store_instance) {
            return Err(ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed);
        }
        let progress = scanner.progress();
        let page = scanner.next_page().await.map_err(|error| match error {
            ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable => {
                ProviderIngestCompletedMusubiReconcileErrorV1::CaptureUnavailable
            }
            _ => ProviderIngestCompletedMusubiReconcileErrorV1::Capture,
        })?;
        let rollback = scanner.restore_progress_on_drop(progress);
        let mut approvals = Vec::new();
        approvals
            .try_reserve(page.candidates().len())
            .map_err(|_| ProviderIngestCompletedMusubiReconcileErrorV1::JournalUnavailable)?;
        let mut inserted = 0_usize;
        let mut existing = 0_usize;
        let mut inventory_suppressed = 0_usize;
        for candidate in page.candidates() {
            let authorization = candidate.authorization();
            let stored = self
                .manifest_metadata_by_digest(&authorization.manifest_digest())
                .map_err(|_| {
                    ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable
                })?;
            let profile =
                sorafs_car::chunker_registry::lookup_by_handle(stored.chunk_profile_handle())
                    .map(|descriptor| descriptor.profile)
                    .ok_or(
                        ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable,
                    )?;
            let plan = stored.try_to_car_plan(profile).map_err(|_| {
                ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable
            })?;
            if !candidate.matches_completed_musubi_store_instance(completed_musubi_store_instance) {
                return Err(ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed);
            }
            let request = self
                .verify_provider_ingest_completed_musubi_capture_bundle(
                    &plan,
                    authorization,
                    candidate.completed_claim(),
                )
                .map_err(|error| match error {
                    ProviderIngestLocalStorageErrorV1::Retryable => {
                        ProviderIngestCompletedMusubiReconcileErrorV1::VerificationUnavailable
                    }
                    ProviderIngestLocalStorageErrorV1::Permanent
                    | ProviderIngestLocalStorageErrorV1::Quarantined => {
                        ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed
                    }
                })?;
            match journal
                .probe_pre_enqueue_with_inventory(&request, inventory)
                .await
            {
                Ok(MusubiProviderAttestationPreEnqueueProbeV1::RetainedExact) => {
                    let approval_id = musubi_provider_attestation_approval_id_v1(&request)
                        .map_err(map_completed_musubi_admission_error)?;
                    existing = existing.saturating_add(1);
                    approvals.push(ProviderIngestCompletedMusubiPreparedApprovalV1 {
                        approval_id,
                        request,
                    });
                    continue;
                }
                Ok(MusubiProviderAttestationPreEnqueueProbeV1::InventoryExact) => {
                    inventory_suppressed = inventory_suppressed.saturating_add(1);
                    continue;
                }
                Ok(MusubiProviderAttestationPreEnqueueProbeV1::Absent) => {}
                Err(error) => return Err(map_completed_musubi_admission_error(error)),
            }
            let enqueue = journal
                .enqueue(&request)
                .await
                .map_err(map_completed_musubi_admission_error)?;
            match enqueue {
                MusubiProviderAttestationEnqueueOutcomeV1::Inserted { .. } => {
                    inserted = inserted.saturating_add(1);
                }
                MusubiProviderAttestationEnqueueOutcomeV1::Existing { .. } => {
                    existing = existing.saturating_add(1);
                }
            }
            approvals.push(ProviderIngestCompletedMusubiPreparedApprovalV1 {
                approval_id: enqueue.approval_id(),
                request,
            });
        }
        let outcome = ProviderIngestCompletedMusubiReconcileOutcomeV1 {
            finalized_cursor: page.finalized_cursor(),
            candidates: page.candidates().len(),
            inserted,
            existing,
            inventory_suppressed,
            scan_complete: page.scan_complete(),
        };
        Ok(ProviderIngestCompletedMusubiPreparedPageV1 {
            rollback,
            approvals,
            outcome,
        })
    }
    #[cfg(test)]
    pub(crate) async fn reconcile_provider_ingest_completed_musubi_capture_page<Ledger, Inventory>(
        &self,
        scanner: &mut ProviderIngestCompletedMusubiCaptureScannerV1<Ledger>,
        journal: &MusubiProviderAttestationJournalV1,
        inventory: &Inventory,
    ) -> Result<
        ProviderIngestCompletedMusubiReconcileOutcomeV1,
        ProviderIngestCompletedMusubiReconcileErrorV1,
    >
    where
        Ledger: ProviderIngestCompletedMusubiSignedCaptureLedgerV1 + ?Sized,
        Inventory: MusubiProviderAttestationInventoryRuntimeV1 + ?Sized,
    {
        Ok(self
            .prepare_provider_ingest_completed_musubi_capture_page(scanner, journal, inventory)
            .await?
            .commit())
    }
}
fn map_completed_musubi_admission_error(
    error: MusubiProviderAttestationJournalErrorV1,
) -> ProviderIngestCompletedMusubiReconcileErrorV1 {
    match error {
        MusubiProviderAttestationJournalErrorV1::InvalidIntent => {
            ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed
        }
        MusubiProviderAttestationJournalErrorV1::IntentConflict => {
            ProviderIngestCompletedMusubiReconcileErrorV1::AdmissionConflict
        }
        MusubiProviderAttestationJournalErrorV1::InventoryUnavailable => {
            ProviderIngestCompletedMusubiReconcileErrorV1::InventoryUnavailable
        }
        MusubiProviderAttestationJournalErrorV1::InventoryRejected
        | MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt => {
            ProviderIngestCompletedMusubiReconcileErrorV1::InventoryRejected
        }
        MusubiProviderAttestationJournalErrorV1::CapacityExceeded => {
            ProviderIngestCompletedMusubiReconcileErrorV1::CapacityExceeded
        }
        MusubiProviderAttestationJournalErrorV1::ClockUnavailable
        | MusubiProviderAttestationJournalErrorV1::StoreUnavailable
        | MusubiProviderAttestationJournalErrorV1::CasRetryExhausted => {
            ProviderIngestCompletedMusubiReconcileErrorV1::JournalUnavailable
        }
        MusubiProviderAttestationJournalErrorV1::InvalidPolicy
        | MusubiProviderAttestationJournalErrorV1::InvalidClaimOwner
        | MusubiProviderAttestationJournalErrorV1::InvalidPageLimit
        | MusubiProviderAttestationJournalErrorV1::ClockRollback
        | MusubiProviderAttestationJournalErrorV1::ClockSealRejected
        | MusubiProviderAttestationJournalErrorV1::NotFound
        | MusubiProviderAttestationJournalErrorV1::StaleClaim
        | MusubiProviderAttestationJournalErrorV1::InvalidAttestation
        | MusubiProviderAttestationJournalErrorV1::AttestationConflict
        | MusubiProviderAttestationJournalErrorV1::SignerUnavailable
        | MusubiProviderAttestationJournalErrorV1::SignerRejected
        | MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint
        | MusubiProviderAttestationJournalErrorV1::StoreRejected
        | MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow => {
            ProviderIngestCompletedMusubiReconcileErrorV1::JournalUnavailable
        }
    }
}
/// Count-only result of one bounded handoff-first effect-pump step.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderIngestCompletedMusubiAttestationDriveOutcomeV1 {
    /// Handoff claims that reached durable delivered state.
    pub handoffs_delivered: usize,
    /// Handoff claims scheduled for retry.
    pub handoffs_retried: usize,
    /// Handoff claims moved to a terminal dead letter.
    pub handoffs_dead_lettered: usize,
    /// Fresh completed rows reverified in the capture page.
    pub capture_candidates: usize,
    /// Newly inserted approval intents.
    pub approvals_inserted: usize,
    /// Exact approval intents already retained.
    pub approvals_existing: usize,
    /// Exact entries already present in authenticated inventory.
    pub inventory_suppressed: usize,
    /// Approval intents made durably post-approval in this step.
    pub approvals_stored: usize,
    /// Whether the capture page exhausted its immutable finalized snapshot.
    pub scan_complete: bool,
}
/// Redacted driver failure with stable retry/operator classification.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderIngestCompletedMusubiAttestationDriveErrorV1 {
    /// The supplied node, capture tenure, or private journal scope differs.
    #[error("completed-Musubi attestation driver binding was rejected")]
    BindingRejected,
    /// The finalized capture reader is temporarily unavailable.
    #[error("completed-Musubi finalized capture is unavailable")]
    CaptureUnavailable,
    /// Admitted payload bytes are temporarily unavailable.
    #[error("completed-Musubi admitted payload is unavailable")]
    PayloadUnavailable,
    /// A sealed clock, journal store, signer, or inventory effect is retryable.
    #[error("completed-Musubi attestation effect is temporarily unavailable")]
    EffectUnavailable,
    /// The bounded journal requires operator capacity intervention.
    #[error("completed-Musubi attestation journal capacity is exhausted")]
    CapacityBlocked,
    /// Approval is terminal without a durable approved attestation.
    #[error("completed-Musubi approval is terminal before durable approval")]
    ApprovalBlocked,
    /// Authenticated input, policy, effect, or durable state contradicted its binding.
    #[error("completed-Musubi attestation integrity check was rejected")]
    IntegrityRejected,
}
impl ProviderIngestCompletedMusubiAttestationDriveErrorV1 {
    /// Return whether an unchanged deployment may safely retry this step.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::CaptureUnavailable | Self::PayloadUnavailable | Self::EffectUnavailable
        )
    }
}
/// Opaque take-once capture, signing, and inventory effect pump.
///
/// This type exposes no requests, claims, journal entries, signers, or inventory effects. One call
/// drains at most one bounded handoff page before preparing one bounded capture page. Scanner
/// progress remains rollback-armed until every fresh request has reached durable approval.
#[doc(hidden)]
pub struct ProviderIngestCompletedMusubiAttestationDriverV1 {
    node: crate::NodeHandle,
    coordinator: ProviderIngestCompletedMusubiCaptureCoordinatorV1,
    journal: Arc<MusubiProviderAttestationJournalRuntimeV1>,
    claim_owner: MusubiProviderAttestationClaimOwnerV1,
    signer: Arc<dyn MusubiProviderAttestationSignerV1>,
    inventory: Arc<dyn MusubiProviderAttestationInventoryRuntimeV1>,
    page_limit: usize,
}
impl fmt::Debug for ProviderIngestCompletedMusubiAttestationDriverV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestCompletedMusubiAttestationDriverV1")
            .field("page_limit", &self.page_limit)
            .finish_non_exhaustive()
    }
}
impl crate::NodeHandle {
    /// Bind the opaque effect pump to one exact node/capture/journal scope.
    ///
    /// # Errors
    ///
    /// Rejects a foreign storage incarnation or any network, provider, policy,
    /// clock-scope, or checkpoint-scope mismatch.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn bind_provider_ingest_completed_musubi_attestation_driver_v1(
        &self,
        coordinator: ProviderIngestCompletedMusubiCaptureCoordinatorV1,
        journal: Arc<MusubiProviderAttestationJournalRuntimeV1>,
        claim_owner: MusubiProviderAttestationClaimOwnerV1,
        expected_policy: MusubiProviderAttestationJournalPolicyV1,
        signer: Arc<dyn MusubiProviderAttestationSignerV1>,
        inventory: Arc<dyn MusubiProviderAttestationInventoryRuntimeV1>,
    ) -> Result<
        ProviderIngestCompletedMusubiAttestationDriverV1,
        ProviderIngestCompletedMusubiAttestationDriveErrorV1,
    > {
        let Some(node_instance) = self.completed_musubi_store_instance.as_ref() else {
            return Err(ProviderIngestCompletedMusubiAttestationDriveErrorV1::BindingRejected);
        };
        let (network_id, provider_bytes, max_page_rows, capture_instance) = coordinator.binding();
        let provider_id = ProviderId::new(provider_bytes);
        if !capture_instance.matches(node_instance)
            || self.config.provider_id() != Some(provider_id)
            || max_page_rows == 0
            || max_page_rows > MUSUBI_PROVIDER_ATTESTATION_READY_PAGE_MAX_V1
            || !journal.matches_binding(network_id, provider_id, expected_policy)
        {
            return Err(ProviderIngestCompletedMusubiAttestationDriveErrorV1::BindingRejected);
        }
        Ok(ProviderIngestCompletedMusubiAttestationDriverV1 {
            node: self.clone(),
            coordinator,
            journal,
            claim_owner,
            signer,
            inventory,
            page_limit: max_page_rows,
        })
    }
}
impl ProviderIngestCompletedMusubiAttestationDriverV1 {
    /// Drain one bounded handoff page, then prepare/sign one capture page.
    ///
    /// # Errors
    ///
    /// Returns only redacted retry, operator-blocked, or integrity classes.
    pub async fn drive_one_bounded_page(
        &mut self,
    ) -> Result<
        ProviderIngestCompletedMusubiAttestationDriveOutcomeV1,
        ProviderIngestCompletedMusubiAttestationDriveErrorV1,
    > {
        let Self {
            node,
            coordinator,
            journal,
            claim_owner,
            signer,
            inventory,
            page_limit,
        } = self;
        let mut outcome = ProviderIngestCompletedMusubiAttestationDriveOutcomeV1::default();
        let ready = journal
            .ready_handoff_page(None, *page_limit)
            .await
            .map_err(map_completed_musubi_driver_journal_error)?;
        for key in ready {
            let Some(claim) = journal
                .claim_handoff(key.approval_id(), *claim_owner)
                .await
                .map_err(map_completed_musubi_driver_journal_error)?
            else {
                continue;
            };
            match journal
                .handoff_claim_with_inventory(&claim, inventory.as_ref())
                .await
            {
                Ok(MusubiProviderAttestationDeliveryOutcomeV1::Delivered)
                | Ok(MusubiProviderAttestationDeliveryOutcomeV1::Existing) => {
                    outcome.handoffs_delivered = outcome.handoffs_delivered.saturating_add(1);
                }
                Err(MusubiProviderAttestationJournalErrorV1::InventoryUnavailable) => match journal
                    .record_handoff_failure(
                        &claim,
                        MusubiProviderAttestationFailureClassV1::Retryable,
                    )
                    .await
                    .map_err(map_completed_musubi_driver_journal_error)?
                {
                    MusubiProviderAttestationRetryOutcomeV1::RetryScheduled => {
                        outcome.handoffs_retried = outcome.handoffs_retried.saturating_add(1);
                    }
                    MusubiProviderAttestationRetryOutcomeV1::DeadLettered => {
                        outcome.handoffs_dead_lettered =
                            outcome.handoffs_dead_lettered.saturating_add(1);
                    }
                },
                Err(
                    MusubiProviderAttestationJournalErrorV1::InventoryRejected
                    | MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt,
                ) => {
                    journal
                        .record_handoff_failure(
                            &claim,
                            MusubiProviderAttestationFailureClassV1::Permanent,
                        )
                        .await
                        .map_err(map_completed_musubi_driver_journal_error)?;
                    outcome.handoffs_dead_lettered =
                        outcome.handoffs_dead_lettered.saturating_add(1);
                }
                Err(
                    MusubiProviderAttestationJournalErrorV1::NotFound
                    | MusubiProviderAttestationJournalErrorV1::StaleClaim,
                ) => {}
                Err(error) => return Err(map_completed_musubi_driver_journal_error(error)),
            }
        }
        coordinator
            .try_activate()
            .map_err(map_completed_musubi_driver_capture_error)?;
        let scanner = coordinator
            .active_scanner_mut()
            .ok_or(ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected)?;
        let prepared = node
            .prepare_provider_ingest_completed_musubi_capture_page(
                scanner,
                journal.raw_journal(),
                inventory.as_ref(),
            )
            .await
            .map_err(map_completed_musubi_driver_reconcile_error)?;
        for approval in &prepared.approvals {
            let Some(claim) = journal
                .claim_approval(approval.approval_id, *claim_owner)
                .await
                .map_err(map_completed_musubi_driver_journal_error)?
            else {
                let status = journal
                    .status(approval.approval_id)
                    .await
                    .map_err(map_completed_musubi_driver_journal_error)?;
                match status {
                    Some(status)
                        if matches!(
                            status.stage,
                            MusubiProviderAttestationJournalStageV1::ApprovedPendingHandoff
                                | MusubiProviderAttestationJournalStageV1::HandoffClaimed
                                | MusubiProviderAttestationJournalStageV1::Delivered
                        ) || status.stage
                            == MusubiProviderAttestationJournalStageV1::DeadLetter
                            && status.dead_letter_has_approved_attestation =>
                    {
                        continue;
                    }
                    Some(status)
                        if status.stage == MusubiProviderAttestationJournalStageV1::DeadLetter =>
                    {
                        return Err(
                            ProviderIngestCompletedMusubiAttestationDriveErrorV1::ApprovalBlocked,
                        );
                    }
                    Some(_) => {
                        return Err(
                            ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable,
                        );
                    }
                    None => {
                        return Err(
                            ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected,
                        );
                    }
                }
            };
            match journal
                .approve_claim_with_signer(&claim, &approval.request, signer.as_ref())
                .await
            {
                Ok(_) => {
                    outcome.approvals_stored = outcome.approvals_stored.saturating_add(1);
                }
                Err(MusubiProviderAttestationJournalErrorV1::SignerUnavailable) => {
                    let retry = journal
                        .record_approval_failure(
                            &claim,
                            MusubiProviderAttestationFailureClassV1::Retryable,
                        )
                        .await
                        .map_err(map_completed_musubi_driver_journal_error)?;
                    return Err(match retry {
                        MusubiProviderAttestationRetryOutcomeV1::RetryScheduled => {
                            ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable
                        }
                        MusubiProviderAttestationRetryOutcomeV1::DeadLettered => {
                            ProviderIngestCompletedMusubiAttestationDriveErrorV1::ApprovalBlocked
                        }
                    });
                }
                Err(
                    MusubiProviderAttestationJournalErrorV1::SignerRejected
                    | MusubiProviderAttestationJournalErrorV1::InvalidAttestation,
                ) => {
                    journal
                        .record_approval_failure(
                            &claim,
                            MusubiProviderAttestationFailureClassV1::Permanent,
                        )
                        .await
                        .map_err(map_completed_musubi_driver_journal_error)?;
                    return Err(
                        ProviderIngestCompletedMusubiAttestationDriveErrorV1::ApprovalBlocked,
                    );
                }
                Err(
                    MusubiProviderAttestationJournalErrorV1::NotFound
                    | MusubiProviderAttestationJournalErrorV1::StaleClaim,
                ) => {
                    return Err(
                        ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable,
                    );
                }
                Err(error) => return Err(map_completed_musubi_driver_journal_error(error)),
            }
        }
        let reconciled = prepared.commit();
        outcome.capture_candidates = reconciled.candidates;
        outcome.approvals_inserted = reconciled.inserted;
        outcome.approvals_existing = reconciled.existing;
        outcome.inventory_suppressed = reconciled.inventory_suppressed;
        outcome.scan_complete = reconciled.scan_complete;
        Ok(outcome)
    }
}
fn map_completed_musubi_driver_capture_error(
    error: ProviderIngestRuntimeErrorV1,
) -> ProviderIngestCompletedMusubiAttestationDriveErrorV1 {
    match error {
        ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::CaptureUnavailable
        }
        _ => ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected,
    }
}
fn map_completed_musubi_driver_reconcile_error(
    error: ProviderIngestCompletedMusubiReconcileErrorV1,
) -> ProviderIngestCompletedMusubiAttestationDriveErrorV1 {
    match error {
        ProviderIngestCompletedMusubiReconcileErrorV1::CaptureUnavailable => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::CaptureUnavailable
        }
        ProviderIngestCompletedMusubiReconcileErrorV1::VerificationUnavailable => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::PayloadUnavailable
        }
        ProviderIngestCompletedMusubiReconcileErrorV1::InventoryUnavailable
        | ProviderIngestCompletedMusubiReconcileErrorV1::JournalUnavailable => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable
        }
        ProviderIngestCompletedMusubiReconcileErrorV1::CapacityExceeded => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::CapacityBlocked
        }
        ProviderIngestCompletedMusubiReconcileErrorV1::Capture
        | ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable
        | ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed
        | ProviderIngestCompletedMusubiReconcileErrorV1::AdmissionConflict
        | ProviderIngestCompletedMusubiReconcileErrorV1::InventoryRejected => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected
        }
    }
}
fn map_completed_musubi_driver_journal_error(
    error: MusubiProviderAttestationJournalErrorV1,
) -> ProviderIngestCompletedMusubiAttestationDriveErrorV1 {
    match error {
        MusubiProviderAttestationJournalErrorV1::ClockUnavailable
        | MusubiProviderAttestationJournalErrorV1::StoreUnavailable
        | MusubiProviderAttestationJournalErrorV1::CasRetryExhausted
        | MusubiProviderAttestationJournalErrorV1::SignerUnavailable
        | MusubiProviderAttestationJournalErrorV1::InventoryUnavailable
        | MusubiProviderAttestationJournalErrorV1::NotFound
        | MusubiProviderAttestationJournalErrorV1::StaleClaim => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable
        }
        MusubiProviderAttestationJournalErrorV1::CapacityExceeded => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::CapacityBlocked
        }
        MusubiProviderAttestationJournalErrorV1::InvalidPolicy
        | MusubiProviderAttestationJournalErrorV1::InvalidIntent
        | MusubiProviderAttestationJournalErrorV1::InvalidClaimOwner
        | MusubiProviderAttestationJournalErrorV1::InvalidPageLimit
        | MusubiProviderAttestationJournalErrorV1::ClockRollback
        | MusubiProviderAttestationJournalErrorV1::ClockSealRejected
        | MusubiProviderAttestationJournalErrorV1::IntentConflict
        | MusubiProviderAttestationJournalErrorV1::InvalidAttestation
        | MusubiProviderAttestationJournalErrorV1::AttestationConflict
        | MusubiProviderAttestationJournalErrorV1::SignerRejected
        | MusubiProviderAttestationJournalErrorV1::InvalidInventoryReceipt
        | MusubiProviderAttestationJournalErrorV1::InventoryRejected
        | MusubiProviderAttestationJournalErrorV1::CorruptCheckpoint
        | MusubiProviderAttestationJournalErrorV1::StoreRejected
        | MusubiProviderAttestationJournalErrorV1::ArithmeticOverflow => {
            ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected
        }
    }
}
