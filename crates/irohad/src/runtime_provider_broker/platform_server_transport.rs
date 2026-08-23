fn broker_error_status(error: BrokerError) -> Option<(u8, bool)> {
    match error {
        BrokerError::Rejected => Some((STATUS_REJECTED_V1, false)),
        BrokerError::Conflict => Some((STATUS_CONFLICT_V1, false)),
        BrokerError::StaleOrRevoked => Some((STATUS_STALE_OR_REVOKED_V1, true)),
        BrokerError::Ambiguous => Some((STATUS_AMBIGUOUS_V1, true)),
        BrokerError::Unavailable => Some((STATUS_UNAVAILABLE_V1, true)),
        BrokerError::Protocol | BrokerError::BindingMismatch => None,
    }
}
fn source_deadline_remaining(deadline: std::time::Instant) -> Result<Duration, BrokerError> {
    deadline
        .checked_duration_since(std::time::Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(BrokerError::Unavailable)
}
fn apply_source_socket_deadline(
    stream: &UnixStream,
    deadline: std::time::Instant,
) -> Result<(), BrokerError> {
    let remaining = source_deadline_remaining(deadline)?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|_| BrokerError::Unavailable)?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(|_| BrokerError::Unavailable)
}
fn source_fetch_error(error: sorafs_node::ProviderIngestSourceFetchErrorV1) -> BrokerError {
    match error {
        sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable => BrokerError::Unavailable,
        sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected => BrokerError::Rejected,
        sorafs_node::ProviderIngestSourceFetchErrorV1::Rejected => BrokerError::StaleOrRevoked,
    }
}
fn fetch_provider_ingest_source(
    source: &dyn crate::sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1,
    request: sorafs_node::ProviderIngestSourceRequestV1,
    timeout: Duration,
) -> Result<crate::sorafs_provider_ingest_runtime::VerifiedProviderIngestPayloadV1, BrokerError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| BrokerError::Unavailable)?;
    runtime
        .block_on(tokio::time::timeout(
            timeout,
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(source, request),
        ))
        .map_err(|_| BrokerError::Unavailable)?
        .map_err(source_fetch_error)
}
fn write_source_trailer(
    stream: &mut UnixStream,
    trailer: ProviderIngestSourceTrailerWireV1,
    deadline: std::time::Instant,
) -> Result<(), BrokerError> {
    apply_source_socket_deadline(stream, deadline)?;
    let admission = DecodeResourceAdmissionV1::acquire(None, SOURCE_STREAM_FRAME_DECODE_POLICY_V1)?;
    let _scope = admission.enter();
    let frame = encode_frame(
        FRAME_KIND_PROVIDER_INGEST_SOURCE_TRAILER_V1,
        &trailer,
        MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
    )?;
    write_length_prefixed(
        stream,
        &frame,
        MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
    )
}
fn write_source_failure_trailer(
    stream: &mut UnixStream,
    status: u8,
    content_length: u64,
    frame_count: u64,
    transcript: &blake3::Hasher,
    provider_metadata_digest: [u8; 32],
    deadline: std::time::Instant,
) {
    let _ = write_source_trailer(
        stream,
        ProviderIngestSourceTrailerWireV1 {
            status,
            content_length,
            frame_count,
            payload_digest: [0; 32],
            transcript_digest: *transcript.clone().finalize().as_bytes(),
            provider_metadata_digest,
        },
        deadline,
    );
}
#[expect(
    clippy::too_many_lines,
    reason = "the authenticated source stream is one ordered transcript"
)]
fn serve_provider_ingest_source_fetch(
    mut stream: UnixStream,
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<(), BrokerError> {
    let limits = required_binding_value!(&request.binding, provider_ingest_source_limits);
    let deadline = std::time::Instant::now()
        .checked_add(Duration::from_millis(limits.operation_timeout_ms))
        .ok_or(BrokerError::Rejected)?;
    apply_source_socket_deadline(&stream, deadline)?;
    let configured =
        qualify_server_binding(state, &request.binding, request.provider_metadata_digest)?;
    let fetch = decode_canonical::<ProviderIngestSourceFetchRequestWireV2>(
        &request.payload,
        MAX_PROVIDER_INGEST_SOURCE_REQUEST_BYTES_V1,
    )?;
    validate_source_fetch_request(
        &fetch,
        &request.binding,
        Some(&configured.provider_ingest_source_provider_ids),
        &state.network_id,
    )?;
    let authorization = fetch.authorization.clone();
    let source_request = source_request_from_wire(fetch)?;
    let source = broker_backend!(state, provider_ingest_authenticated_source);
    let mut fetched = fetch_provider_ingest_source(
        source.as_ref(),
        source_request,
        source_deadline_remaining(deadline)?,
    )?;
    validate_source_payload_metadata(&authorization, &fetched.manifest, &fetched.plan)?;
    let _retained_memory = acquire_source_retained_memory(&fetched.plan)?;
    let content_length = authorization.content_length();
    let frame_count = source_stream_frame_count(content_length)?;
    let mut transcript = {
        let initial_admission = DecodeResourceAdmissionV1::acquire_operation(
            OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V2,
        )?;
        let _initial_scope = initial_admission.enter();
        let manifest = fetched
            .manifest
            .encode()
            .map_err(|_| BrokerError::Rejected)?;
        let plan = encode_source_plan(&fetched.plan)?;
        validate_source_metadata_lengths(manifest.len(), plan.len())?;
        if sorafs_manifest::decode_manifest_v1_canonical(&manifest)
            .map_err(|_| BrokerError::Rejected)?
            != fetched.manifest
        {
            return Err(BrokerError::Rejected);
        }
        let header = ProviderIngestSourceHeaderWireV1 {
            manifest,
            plan,
            content_length,
            frame_count,
        };
        let result = encode_canonical(&header, MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1)?;
        let response = make_operation_response(request, STATUS_OK_V1, result, &state.network_id)?;
        let response_frame = encode_frame(
            FRAME_KIND_OPERATION_RESPONSE_V1,
            &response,
            MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
        )?;
        apply_source_socket_deadline(&stream, deadline)?;
        write_length_prefixed(
            &mut stream,
            &response_frame,
            MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
        )?;
        source_stream_transcript(request, &response)
    };
    let mut payload_hasher = blake3::Hasher::new();
    let mut offset = 0_u64;
    for sequence in 0..frame_count {
        let remaining = content_length
            .checked_sub(offset)
            .ok_or(BrokerError::Protocol)?;
        let chunk_len = usize::try_from(
            remaining.min(
                u64::try_from(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
                    .map_err(|_| BrokerError::Protocol)?,
            ),
        )
        .map_err(|_| BrokerError::Protocol)?;
        let chunk_admission =
            DecodeResourceAdmissionV1::acquire(None, SOURCE_STREAM_FRAME_DECODE_POLICY_V1)?;
        chunk_admission
            .reserve_retained_bytes(chunk_len, MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1)?;
        let _chunk_scope = chunk_admission.enter();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(chunk_len)
            .map_err(|_| BrokerError::Unavailable)?;
        bytes.resize(chunk_len, 0);
        if source_deadline_remaining(deadline).is_err()
            || std::io::Read::read_exact(&mut fetched.reader, &mut bytes).is_err()
            || source_deadline_remaining(deadline).is_err()
        {
            write_source_failure_trailer(
                &mut stream,
                STATUS_REJECTED_V1,
                content_length,
                sequence,
                &transcript,
                request.provider_metadata_digest,
                deadline,
            );
            return Ok(());
        }
        let chunk = ProviderIngestSourceChunkWireV1 {
            sequence,
            offset,
            bytes,
        };
        payload_hasher.update(&chunk.bytes);
        update_source_stream_transcript(&mut transcript, &chunk);
        let frame = encode_frame(
            FRAME_KIND_PROVIDER_INGEST_SOURCE_CHUNK_V1,
            &chunk,
            MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
        )?;
        apply_source_socket_deadline(&stream, deadline)?;
        write_length_prefixed(
            &mut stream,
            &frame,
            MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
        )?;
        offset = offset
            .checked_add(u64::try_from(chunk_len).map_err(|_| BrokerError::Protocol)?)
            .ok_or(BrokerError::Protocol)?;
    }
    let mut trailing = [0_u8; 1];
    let trailing_result = std::io::Read::read(&mut fetched.reader, &mut trailing);
    let payload_digest = *payload_hasher.finalize().as_bytes();
    if source_deadline_remaining(deadline).is_err()
        || !matches!(trailing_result, Ok(0))
        || payload_digest != *fetched.plan.payload_digest.as_bytes()
    {
        write_source_failure_trailer(
            &mut stream,
            STATUS_REJECTED_V1,
            content_length,
            frame_count,
            &transcript,
            request.provider_metadata_digest,
            deadline,
        );
        return Ok(());
    }
    if qualify_server_binding(state, &request.binding, request.provider_metadata_digest).is_err() {
        write_source_failure_trailer(
            &mut stream,
            STATUS_STALE_OR_REVOKED_V1,
            content_length,
            frame_count,
            &transcript,
            request.provider_metadata_digest,
            deadline,
        );
        return Ok(());
    }
    write_source_trailer(
        &mut stream,
        ProviderIngestSourceTrailerWireV1 {
            status: STATUS_OK_V1,
            content_length,
            frame_count,
            payload_digest,
            transcript_digest: *transcript.finalize().as_bytes(),
            provider_metadata_digest: request.provider_metadata_digest,
        },
        deadline,
    )
}
#[expect(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "each blocking session owns its shared permits and ordered transcript"
)]
fn serve_client(
    mut stream: UnixStream,
    state: &BrokerServerStateV1,
    mut session_permit: Option<tokio::sync::OwnedSemaphorePermit>,
    source_stream_permits: Arc<tokio::sync::Semaphore>,
    inbound_operation_budget: Arc<tokio::sync::Semaphore>,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
) -> Result<(), BrokerError> {
    if lifecycle.shutdown_requested() {
        return Err(BrokerError::Unavailable);
    }
    stream
        .set_read_timeout(Some(BROKER_IO_TIMEOUT_V1))
        .map_err(|_| BrokerError::Unavailable)?;
    stream
        .set_write_timeout(Some(BROKER_IO_TIMEOUT_V1))
        .map_err(|_| BrokerError::Unavailable)?;
    let request_frame = read_length_prefixed(&mut stream, MAX_HANDSHAKE_FRAME_BYTES_V1)?;
    let handshake = decode_frame::<HandshakeRequestV1>(
        &request_frame,
        FRAME_KIND_HANDSHAKE_REQUEST_V1,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )?;
    validate_handshake_request(&handshake)?;
    if lifecycle.shutdown_requested() {
        return Ok(());
    }
    if handshake.chain_id != state.chain_id || handshake.network_id != state.network_id {
        return Err(BrokerError::BindingMismatch);
    }
    let live_observations = handshake
        .requested_catalog
        .iter()
        .map(|binding| {
            let Some(_qualification_permit) = lifecycle.try_begin_qualification() else {
                return Err(BrokerError::Unavailable);
            };
            let configured = configured_observation(state, binding)?;
            qualify_server_binding(state, binding, configured.metadata_digest)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if lifecycle.shutdown_requested() {
        return Ok(());
    }
    let mut session_id = [0_u8; 32];
    rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut session_id)
        .map_err(|_| BrokerError::Unavailable)?;
    if session_id == [0; 32] {
        return Err(BrokerError::Unavailable);
    }
    let response = make_handshake_response(&handshake, session_id, live_observations)?;
    let response_frame = encode_frame(
        FRAME_KIND_HANDSHAKE_RESPONSE_V1,
        &response,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )?;
    write_length_prefixed(&mut stream, &response_frame, MAX_HANDSHAKE_FRAME_BYTES_V1)?;
    let mut expected_request_id = 1_u64;
    let mut pop_session = PopBrokerServerSessionV1::default();
    loop {
        if lifecycle.shutdown_requested() {
            return Ok(());
        }
        let (announced_slot, announced_operation, request_frame, decode_admission) =
            read_operation_request_frame_with_budget(
                &mut stream,
                Arc::clone(&inbound_operation_budget),
            )?;
        let decode_scope = decode_admission.enter();
        let request = decode_operation_frame::<OperationRequestV1>(
            &request_frame,
            FRAME_KIND_OPERATION_REQUEST_V1,
            announced_operation,
        )?;
        validate_operation_request_for_session(&request, &state.chain_id, &state.network_id)?;
        if request.binding.slot != announced_slot
            || request.operation != announced_operation
            || request.session_id != session_id
            || request.request_id != expected_request_id
        {
            return Err(BrokerError::Protocol);
        }
        if !handshake
            .requested_catalog
            .iter()
            .any(|binding| binding == &request.binding)
        {
            return Err(BrokerError::BindingMismatch);
        }
        let configured = configured_observation(state, &request.binding)?;
        if request.provider_metadata_digest != configured.metadata_digest {
            return Err(BrokerError::BindingMismatch);
        }
        // Retire the request identifier before dispatch. A failed or
        // partially completed mutation can never be replayed in this
        // authenticated session.
        expected_request_id = expected_request_id
            .checked_add(1)
            .ok_or(BrokerError::Protocol)?;
        // This permit is the operation's admission boundary. The
        // lifecycle's second state check closes the race with the
        // shutdown transition; a request that loses that race never
        // reaches a deployment-owned provider method.
        let Some(_operation_permit) = lifecycle.try_begin_operation() else {
            return Ok(());
        };
        if request.binding.slot
            == IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource.wire_id()
            && request.operation == OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V2
        {
            let Ok(source_stream_permit) = Arc::clone(&source_stream_permits).try_acquire_owned()
            else {
                let response = make_operation_response(
                    &request,
                    STATUS_REJECTED_V1,
                    encode_canonical(&(), MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1)?,
                    &state.network_id,
                )?;
                let response_frame = encode_frame(
                    FRAME_KIND_OPERATION_RESPONSE_V1,
                    &response,
                    MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
                )?;
                write_length_prefixed(
                    &mut stream,
                    &response_frame,
                    MAX_PROVIDER_INGEST_SOURCE_INITIAL_FRAME_BYTES_V1,
                )?;
                return Ok(());
            };
            // A stalled source stream consumes only its independently
            // configured stream and retained-plan permits after the
            // authenticated request has been classified. Release both
            // the unary session and the full initial-frame admission
            // before entering the payload stream.
            drop(session_permit.take());
            drop(decode_scope);
            drop(decode_admission);
            drop(request_frame);
            let _source_stream_permit = source_stream_permit;
            return serve_provider_ingest_source_fetch(stream, state, &request);
        }
        let (status, result, terminate) =
            match dispatch_server_operation_with_session(state, &mut pop_session, &request) {
                Ok(result) => (STATUS_OK_V1, result, false),
                Err(error) => {
                    let Some((status, terminate)) = broker_error_status(error) else {
                        return Err(error);
                    };
                    (
                        status,
                        ScrubbedBytes::new(encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)?),
                        terminate,
                    )
                }
            };
        let response =
            make_operation_response_scrubbed(&request, status, result, &state.network_id)?;
        let response_frame_limit = operation_frame_limit(request.operation);
        let response_frame = encode_frame(
            FRAME_KIND_OPERATION_RESPONSE_V1,
            &response,
            response_frame_limit,
        )?;
        write_length_prefixed(&mut stream, &response_frame, response_frame_limit)?;
        if terminate {
            return Ok(());
        }
    }
}
struct BoundSocketGuard {
    parent_directory: fs::File,
    instance_lock: endpoint_recovery::InstanceLockGuard,
    socket_name: std::ffi::OsString,
    identity: SocketIdentity,
    expected_service_uid: u32,
    socket_mode: u32,
    armed: bool,
}
impl BoundSocketGuard {
    fn verify_entry(&self) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
        self.instance_lock.verify(&self.parent_directory)?;
        let metadata = rustix::fs::statat(
            &self.parent_directory,
            self.socket_name.as_os_str(),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
        if !endpoint_recovery::socket_metadata_is_exact(
            &metadata,
            self.expected_service_uid,
            self.socket_mode,
        ) || socket_identity_from_stat(&metadata) != self.identity
        {
            return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
        }
        Ok(())
    }
    fn promote(
        &mut self,
        canonical_name: std::ffi::OsString,
    ) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
        rustix::fs::renameat_with(
            &self.parent_directory,
            self.socket_name.as_os_str(),
            &self.parent_directory,
            canonical_name.as_os_str(),
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
        self.socket_name = canonical_name;
        self.verify_entry()
    }
    fn cleanup(mut self) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
        self.armed = false;
        self.instance_lock.verify(&self.parent_directory)?;
        endpoint_recovery::cleanup_socket_entry(
            &self.parent_directory,
            self.socket_name.as_os_str(),
            self.identity,
            self.expected_service_uid,
            self.socket_mode,
            &self.instance_lock,
        )
    }
}
impl Drop for BoundSocketGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        if self.instance_lock.verify(&self.parent_directory).is_err() {
            return;
        }
        let _ = endpoint_recovery::cleanup_socket_entry(
            &self.parent_directory,
            self.socket_name.as_os_str(),
            self.identity,
            self.expected_service_uid,
            self.socket_mode,
            &self.instance_lock,
        );
    }
}
#[cfg(target_os = "linux")]
const fn socket_device_identity_from_raw(device: rustix::fs::Dev) -> u64 {
    device
}
#[cfg(target_os = "macos")]
#[expect(
    clippy::cast_sign_loss,
    reason = "the filesystem identity preserves MetadataExt's signed dev_t bit pattern"
)]
const fn socket_device_identity_from_raw(device: rustix::fs::Dev) -> u64 {
    // `MetadataExt::dev()` exposes `dev_t` through Rust's unsigned
    // `u64` identity. On macOS `dev_t` is signed, so `as` preserves
    // the exact conversion used by `MetadataExt` even when its high
    // bit is set; a fallible numeric conversion would reject a valid
    // filesystem identity.
    device as u64
}
fn socket_identity_from_stat(metadata: &rustix::fs::Stat) -> SocketIdentity {
    SocketIdentity {
        device: socket_device_identity_from_raw(metadata.st_dev),
        inode: metadata.st_ino,
    }
}
fn finish_startup_failure<T>(
    listener: UnixListener,
    guard: BoundSocketGuard,
    error: RuntimeProviderBrokerServerErrorV1,
) -> Result<T, RuntimeProviderBrokerServerErrorV1> {
    drop(listener);
    match guard.cleanup() {
        Ok(()) => Err(error),
        Err(cleanup_error) => Err(cleanup_error),
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "socket creation and identity pinning form one audit sequence"
)]
fn bind_server_listener(
    policy: &EndpointPolicy,
) -> Result<(tokio::net::UnixListener, BoundSocketGuard), RuntimeProviderBrokerServerErrorV1> {
    let parent = policy
        .path
        .parent()
        .ok_or(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    verify_directory(parent, policy.expected_service_uid, false).map_err(server_error)?;
    if policy.verify_all_ancestors {
        for ancestor in parent.ancestors().skip(1) {
            verify_directory(ancestor, policy.expected_service_uid, true).map_err(server_error)?;
        }
    }
    let parent_directory = rustix::fs::open(
        parent,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    let parent_metadata = parent_directory
        .metadata()
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    if !parent_metadata.is_dir()
        || parent_metadata.uid() != policy.expected_service_uid
        || parent_metadata.mode() & 0o022 != 0
    {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    let socket_name = policy
        .path
        .file_name()
        .ok_or(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?
        .to_owned();
    let instance_lock = endpoint_recovery::prepare_endpoint(
        &parent_directory,
        socket_name.as_os_str(),
        policy.expected_service_uid,
        policy.socket_mode,
    )?;
    let staging_name = endpoint_recovery::staging_socket_name()?;
    let staging_path = parent.join(&staging_name);
    let listener = UnixListener::bind(&staging_path)
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    let Ok(bound_metadata) = rustix::fs::statat(
        &parent_directory,
        staging_name.as_os_str(),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    ) else {
        drop(listener);
        // V1 cleanup is identity-conservative: without an observed
        // device/inode identity, this pathname is never unlinked.
        // Closing the listener, failing startup, and retaining the
        // unpredictable staging entry for operator inspection is
        // safer than removing a same-UID substituted entry.
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed);
    };
    let bound_identity = socket_identity_from_stat(&bound_metadata);
    let mut guard = BoundSocketGuard {
        parent_directory,
        instance_lock,
        socket_name: staging_name,
        identity: bound_identity,
        expected_service_uid: policy.expected_service_uid,
        socket_mode: policy.socket_mode,
        armed: true,
    };
    if rustix::fs::FileType::from_raw_mode(bound_metadata.st_mode) != rustix::fs::FileType::Socket
        || bound_metadata.st_uid != policy.expected_service_uid
        || bound_metadata.st_nlink != 1
    {
        return finish_startup_failure(
            listener,
            guard,
            RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
        );
    }
    let socket_mode = match rustix::fs::RawMode::try_from(policy.socket_mode) {
        Ok(mode) => mode,
        Err(_) => {
            return finish_startup_failure(
                listener,
                guard,
                RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
            );
        }
    };
    #[cfg(target_os = "linux")]
    let chmod_flags = rustix::fs::AtFlags::empty();
    #[cfg(target_os = "macos")]
    let chmod_flags = rustix::fs::AtFlags::SYMLINK_NOFOLLOW;
    if rustix::fs::chmodat(
        &guard.parent_directory,
        guard.socket_name.as_os_str(),
        rustix::fs::Mode::from_raw_mode(socket_mode),
        chmod_flags,
    )
    .is_err()
    {
        return finish_startup_failure(
            listener,
            guard,
            RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
        );
    }
    if let Err(error) = guard.verify_entry() {
        return finish_startup_failure(listener, guard, error);
    }
    if let Err(error) = guard.promote(socket_name) {
        return finish_startup_failure(listener, guard, error);
    }
    let identity = match endpoint_identity(policy) {
        Ok(identity) if identity == guard.identity => identity,
        Ok(_) | Err(_) => {
            return finish_startup_failure(
                listener,
                guard,
                RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
            );
        }
    };
    debug_assert_eq!(identity, guard.identity);
    if listener.set_nonblocking(true).is_err() {
        return finish_startup_failure(
            listener,
            guard,
            RuntimeProviderBrokerServerErrorV1::EndpointUnavailable,
        );
    }
    let listener = match tokio::net::UnixListener::from_std(listener) {
        Ok(listener) => listener,
        Err(_) => {
            return match guard.cleanup() {
                Ok(()) => Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable),
                Err(cleanup_error) => Err(cleanup_error),
            };
        }
    };
    Ok((listener, guard))
}
fn verify_endpoint_is_pinned(
    policy: &EndpointPolicy,
    guard: &BoundSocketGuard,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    guard.verify_entry()?;
    if endpoint_identity(policy).map_err(server_error)? != guard.identity {
        return Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
    }
    Ok(())
}
struct BrokerServingLifecycleGuard {
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
}
impl Drop for BrokerServingLifecycleGuard {
    fn drop(&mut self) {
        self.lifecycle.request_shutdown();
    }
}
#[derive(Default)]
struct AcceptedSessionControlsV1 {
    controls: std::collections::BTreeMap<u64, UnixStream>,
}
impl AcceptedSessionControlsV1 {
    fn insert(&mut self, session_token: u64, control: UnixStream) {
        let replaced = self.controls.insert(session_token, control);
        debug_assert!(
            replaced.is_none(),
            "session tokens are monotonically allocated and never reused"
        );
    }
    fn remove(&mut self, session_token: u64) {
        let _ = self.controls.remove(&session_token);
    }
    fn shutdown_all(&self) {
        for control in self.controls.values() {
            let _ = control.shutdown(std::net::Shutdown::Both);
        }
    }
}
impl Drop for AcceptedSessionControlsV1 {
    fn drop(&mut self) {
        // A serving-thread panic or other unexpected unwind must wake
        // every accepted session before its `JoinSet` is dropped.
        // The control sockets are clones of the descriptors owned by
        // the blocking workers, so shutdown applies to both handles.
        self.shutdown_all();
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "the serving lifecycle is one ordered shutdown protocol"
)]
fn serve_with_policy_and_fallible_readiness<R>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    policy: &EndpointPolicy,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce() -> Result<(), RuntimeProviderBrokerReadinessErrorV1>,
{
    if lifecycle.shutdown_requested() {
        return Ok(());
    }
    let _serving_lifecycle = BrokerServingLifecycleGuard {
        lifecycle: Arc::clone(&lifecycle),
    };
    let state = match prepare_server_state_for_lifecycle(bindings, backends, &lifecycle) {
        Ok(state) => Arc::new(state),
        Err(StartupQualificationErrorV1::Cancelled) => return Ok(()),
        Err(StartupQualificationErrorV1::Failed(error)) => return Err(error),
    };
    if lifecycle.shutdown_requested() {
        return Ok(());
    }
    let source_stream_limit = state
        .catalog
        .iter()
        .find(|binding| {
            binding.slot == IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource.wire_id()
        })
        .and_then(|binding| binding.provider_ingest_source_limits)
        .map(|limits| {
            usize::try_from(limits.max_concurrent_streams)
                .map_err(|_| RuntimeProviderBrokerServerErrorV1::BindingMismatch)
        })
        .transpose()?
        .unwrap_or(0);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)?;
    runtime.block_on(async move {
        let (listener, guard) = bind_server_listener(policy)?;
        if lifecycle.shutdown_requested() {
            drop(listener);
            return match guard.cleanup() {
                Ok(()) => Ok(()),
                Err(cleanup_error) => Err(cleanup_error),
            };
        }
        if let Err(outcome) = requalify_server_state(&state, &lifecycle) {
            let result = match outcome {
                StartupQualificationErrorV1::Cancelled => Ok(()),
                StartupQualificationErrorV1::Failed(error) => Err(error),
            };
            drop(listener);
            return match guard.cleanup() {
                Ok(()) => result,
                Err(cleanup_error) => Err(cleanup_error),
            };
        }
        if let Err(error) = verify_endpoint_is_pinned(policy, &guard) {
            drop(listener);
            return match guard.cleanup() {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(cleanup_error),
            };
        }
        match lifecycle.publish_ready_fallible(on_ready) {
            Ok(true) => {}
            Ok(false) => {
                let result = if lifecycle.shutdown_requested() {
                    Ok(())
                } else {
                    Err(RuntimeProviderBrokerServerErrorV1::Protocol)
                };
                drop(listener);
                return match guard.cleanup() {
                    Ok(()) => result,
                    Err(cleanup_error) => Err(cleanup_error),
                };
            }
            Err(RuntimeProviderBrokerReadinessErrorV1) => {
                lifecycle.request_shutdown();
                drop(listener);
                return match guard.cleanup() {
                    Ok(()) => Err(RuntimeProviderBrokerServerErrorV1::ReadinessUnavailable),
                    Err(cleanup_error) => Err(cleanup_error),
                };
            }
        }
        let session_permits = Arc::new(tokio::sync::Semaphore::new(MAX_BROKER_SESSIONS_V1));
        // Hold a permit for every declared inbound operation byte until
        // the decoded request leaves its server-loop iteration. A
        // compromised same-UID peer can therefore occupy at most one
        // maximum-sized operation across all accepted sessions, and a
        // length prefix alone never triggers a maximum-sized allocation.
        let inbound_operation_budget = Arc::new(tokio::sync::Semaphore::new(
            MAX_BROKER_PROCESS_OPERATION_BYTES_V1
                .checked_sub(MAX_BROKER_SHARED_DECODE_BYTES_V1)
                .expect("combined broker memory ceiling includes raw frames"),
        ));
        let source_stream_permits = Arc::new(tokio::sync::Semaphore::new(source_stream_limit));
        let mut sessions = tokio::task::JoinSet::new();
        let mut session_controls = AcceptedSessionControlsV1::default();
        let mut next_session_token = 0_u64;
        let mut serve_result = 'serve: loop {
            while let Some(joined) = sessions.try_join_next() {
                match joined {
                    Ok((session_token, _session_result)) => {
                        session_controls.remove(session_token);
                    }
                    Err(_) => {
                        break 'serve Err(RuntimeProviderBrokerServerErrorV1::Protocol);
                    }
                }
            }
            if lifecycle.shutdown_requested() {
                break Ok(());
            }
            if let Err(error) = verify_endpoint_is_pinned(policy, &guard) {
                break Err(error);
            }
            let accepted = tokio::select! {
                accepted = listener.accept() => Some(accepted),
                () = tokio::time::sleep(Duration::from_millis(100)) => None,
            };
            let Some(accepted) = accepted else {
                if let Err(error) = verify_endpoint_is_pinned(policy, &guard) {
                    break Err(error);
                }
                continue;
            };
            let (stream, _) = match accepted {
                Ok(accepted) => accepted,
                Err(_) => {
                    break Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
                }
            };
            if lifecycle.shutdown_requested() {
                drop(stream);
                break Ok(());
            }
            if let Err(error) = verify_endpoint_is_pinned(policy, &guard) {
                drop(stream);
                break Err(error);
            }
            let credentials = match stream.peer_cred() {
                Ok(credentials) => credentials,
                Err(_) => {
                    break Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
                }
            };
            if let Err(error) = verify_peer_uid(credentials.uid(), policy.expected_service_uid)
                .map_err(server_error)
            {
                break Err(error);
            }
            let Ok(session_permit) = Arc::clone(&session_permits).try_acquire_owned() else {
                // Excess peers are closed immediately. They never
                // enter an unbounded task, thread, or request queue.
                drop(stream);
                continue;
            };
            let stream = match stream.into_std() {
                Ok(stream) => stream,
                Err(_) => {
                    break Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
                }
            };
            if stream.set_nonblocking(false).is_err() {
                break Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
            }
            let control = match stream.try_clone() {
                Ok(control) => control,
                Err(_) => {
                    break Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable);
                }
            };
            let Some(session_token) = next_session_token.checked_add(1) else {
                break Err(RuntimeProviderBrokerServerErrorV1::Protocol);
            };
            next_session_token = session_token;
            session_controls.insert(session_token, control);
            let session_state = Arc::clone(&state);
            let session_source_stream_permits = Arc::clone(&source_stream_permits);
            let session_inbound_operation_budget = Arc::clone(&inbound_operation_budget);
            let session_lifecycle = Arc::clone(&lifecycle);
            let _session_registration = sessions.spawn_blocking(move || {
                // A peer protocol error, timeout, disconnect, or
                // backend rejection terminates only this authenticated
                // session.
                let result = serve_client(
                    stream,
                    &session_state,
                    Some(session_permit),
                    session_source_stream_permits,
                    session_inbound_operation_budget,
                    session_lifecycle,
                );
                (session_token, result)
            });
            if let Err(error) = verify_endpoint_is_pinned(policy, &guard) {
                break Err(error);
            }
        };
        lifecycle.request_shutdown();
        drop(listener);
        session_controls.shutdown_all();
        // V1 shutdown is completion-safe, not time-bounded: transport
        // shutdown wakes idle sessions and prevents new operations,
        // then the broker joins every call admitted before the
        // lifecycle transition. Synchronous provider traits expose no
        // uniform cancellation primitive, so deployment adapters must
        // enforce their configured external-call deadlines. Detaching
        // a blocking worker here could let a mutating provider call
        // outlive endpoint cleanup and a subsequent broker restart.
        while let Some(joined) = sessions.join_next().await {
            match joined {
                Ok((session_token, _session_result)) => {
                    session_controls.remove(session_token);
                }
                Err(_) if serve_result.is_ok() => {
                    serve_result = Err(RuntimeProviderBrokerServerErrorV1::Protocol);
                }
                Err(_) => {}
            }
        }
        drop(session_controls);
        match guard.cleanup() {
            Ok(()) => serve_result,
            Err(cleanup_error) => Err(cleanup_error),
        }
    })
}
fn serve_with_policy_and_lifecycle<R>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    policy: &EndpointPolicy,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce(),
{
    serve_with_policy_and_fallible_readiness(bindings, backends, policy, lifecycle, || {
        on_ready();
        Ok(())
    })
}
fn serve_with_policy(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    policy: &EndpointPolicy,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    serve_with_policy_and_lifecycle(bindings, backends, policy, lifecycle, || {})
}
struct BrokerConnection {
    stream: UnixStream,
    session_id: [u8; 32],
    next_request_id: u64,
    poisoned: bool,
}
struct BrokerSession {
    connection: Mutex<BrokerConnection>,
    chain_id: String,
    network_id: NetworkId,
    endpoint: EndpointPolicy,
    requested_catalog: Vec<ProviderBindingWireV1>,
}
fn connect_broker_connection(
    policy: &EndpointPolicy,
    chain_id: &str,
    network_id: NetworkId,
    requested_catalog: Vec<ProviderBindingWireV1>,
    io_timeout: Option<Duration>,
) -> Result<(BrokerConnection, Vec<ProviderObservationWireV1>), BrokerError> {
    let mut stream = connect_verified(policy)?;
    if let Some(io_timeout) = io_timeout {
        if io_timeout.is_zero() {
            return Err(BrokerError::Unavailable);
        }
        stream
            .set_read_timeout(Some(io_timeout))
            .map_err(|_| BrokerError::Unavailable)?;
        stream
            .set_write_timeout(Some(io_timeout))
            .map_err(|_| BrokerError::Unavailable)?;
    }
    let mut client_nonce = [0_u8; 32];
    rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut client_nonce)
        .map_err(|_| BrokerError::Unavailable)?;
    if client_nonce == [0; 32] {
        return Err(BrokerError::Unavailable);
    }
    let request = make_handshake_request(chain_id, network_id, requested_catalog, client_nonce)?;
    let request_frame = encode_frame(
        FRAME_KIND_HANDSHAKE_REQUEST_V1,
        &request,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )?;
    write_length_prefixed(&mut stream, &request_frame, MAX_HANDSHAKE_FRAME_BYTES_V1)?;
    let response_frame = read_length_prefixed(&mut stream, MAX_HANDSHAKE_FRAME_BYTES_V1)?;
    let response = decode_frame::<HandshakeResponseV1>(
        &response_frame,
        FRAME_KIND_HANDSHAKE_RESPONSE_V1,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )?;
    validate_handshake_response(&request, &response)?;
    Ok((
        BrokerConnection {
            stream,
            session_id: response.session_id,
            next_request_id: 1,
            poisoned: false,
        },
        response.observations,
    ))
}
impl_broker_debug_fields!(BrokerSession as value {} => finish_non_exhaustive);
impl BrokerSession {
    fn connect(
        policy: &EndpointPolicy,
        chain_id: &str,
        network_id: NetworkId,
        requested_catalog: Vec<ProviderBindingWireV1>,
    ) -> Result<(Arc<Self>, Vec<ProviderObservationWireV1>), BrokerError> {
        let (connection, observations) = connect_broker_connection(
            policy,
            chain_id,
            network_id,
            requested_catalog.clone(),
            None,
        )?;
        Ok((
            Arc::new(Self {
                connection: Mutex::new(connection),
                chain_id: chain_id.to_owned(),
                network_id,
                endpoint: policy.clone(),
                requested_catalog,
            }),
            observations,
        ))
    }
    fn reconnect(&self) -> Result<(), BrokerError> {
        let (connection, _) = connect_broker_connection(
            &self.endpoint,
            &self.chain_id,
            self.network_id,
            self.requested_catalog.clone(),
            None,
        )?;
        let mut current = self
            .connection
            .lock()
            .map_err(|_| BrokerError::Unavailable)?;
        *current = connection;
        Ok(())
    }
    fn poison(&self) {
        if let Ok(mut connection) = self.connection.lock() {
            connection.poisoned = true;
        }
    }
    fn decode_result<T>(&self, bytes: &ScrubbedBytes) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let _scope = bytes.enter_decode_admission();
        decode_canonical::<T>(bytes, MAX_OPERATION_FRAME_BYTES_V1).inspect_err(|_| {
            self.poison();
        })
    }
    fn decode_operation_result<T>(
        &self,
        bytes: &ScrubbedBytes,
        operation: u16,
    ) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        if bytes
            .decode_admission
            .as_ref()
            .and_then(|admission| admission.operation)
            .is_some_and(|active| active != operation)
        {
            self.poison();
            return Err(BrokerError::Protocol);
        }
        let _scope = bytes.enter_decode_admission();
        decode_canonical_with_policy::<T>(
            bytes,
            operation_frame_limit(operation),
            operation_decode_policy(operation),
        )
        .inspect_err(|_| {
            self.poison();
        })
    }
    fn decode_nested_result<T>(&self, bytes: &ScrubbedBytes, limit: usize) -> Result<T, BrokerError>
    where
        T: NoritoSerialize,
        for<'de> T: NoritoDeserialize<'de>,
    {
        let _scope = bytes.enter_decode_admission();
        decode_nested_canonical::<T>(bytes, limit).inspect_err(|_| {
            self.poison();
        })
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one request owns its full authenticated exchange"
    )]
    fn call(
        &self,
        binding: &ProviderBindingWireV1,
        metadata_digest: [u8; 32],
        operation: u16,
        payload: Vec<u8>,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        let frame_limit = operation_frame_limit(operation);
        // Reserve the full audited operation ceiling before retaining
        // the caller's payload or constructing any canonical request
        // copies. The same reservation follows the response into the
        // caller's typed result decode.
        let decode_admission = DecodeResourceAdmissionV1::acquire_operation(operation)?;
        decode_admission.reserve_retained_bytes(payload.len(), frame_limit)?;
        let decode_scope = decode_admission.enter();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::Unavailable)?;
        if connection.poisoned {
            return Err(BrokerError::Unavailable);
        }
        let request_id = connection.next_request_id;
        let next_request_id = connection
            .next_request_id
            .checked_add(1)
            .ok_or(BrokerError::Unavailable)?;
        let request = make_operation_request(
            connection.session_id,
            request_id,
            binding.clone(),
            metadata_digest,
            operation,
            payload,
        )?;
        let request_frame = encode_frame(FRAME_KIND_OPERATION_REQUEST_V1, &request, frame_limit)?;
        // Retire the identifier before the first write so a partially
        // dispatched request can never be replayed with the same id.
        connection.next_request_id = next_request_id;
        if write_operation_request_frame(&mut connection.stream, &request, &request_frame).is_err()
        {
            connection.poisoned = true;
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::Unavailable
            });
        }
        drop(request_frame);
        let Ok(response_frame) = read_length_prefixed_with_decode_admission(
            &mut connection.stream,
            frame_limit,
            &decode_admission,
        ) else {
            connection.poisoned = true;
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::Unavailable
            });
        };
        let Ok(mut response) = decode_operation_frame::<OperationResponseV1>(
            &response_frame,
            FRAME_KIND_OPERATION_RESPONSE_V1,
            operation,
        ) else {
            connection.poisoned = true;
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::Protocol
            });
        };
        if let Err(error) =
            validate_operation_response_for_client(&request, &response, &self.network_id)
        {
            connection.poisoned = true;
            return Err(if mutating {
                BrokerError::Ambiguous
            } else {
                error
            });
        }
        match response.status {
            STATUS_OK_V1 => {
                let result = std::mem::take(&mut response.result);
                drop(decode_scope);
                Ok(ScrubbedBytes::with_decode_admission(
                    result,
                    decode_admission,
                ))
            }
            STATUS_REJECTED_V1 => Err(BrokerError::Rejected),
            STATUS_CONFLICT_V1 => Err(BrokerError::Conflict),
            STATUS_STALE_OR_REVOKED_V1 => {
                connection.poisoned = true;
                Err(BrokerError::StaleOrRevoked)
            }
            STATUS_AMBIGUOUS_V1 => {
                connection.poisoned = true;
                Err(BrokerError::Ambiguous)
            }
            STATUS_UNAVAILABLE_V1 => {
                connection.poisoned = true;
                Err(BrokerError::Unavailable)
            }
            _ => {
                connection.poisoned = true;
                Err(if mutating {
                    BrokerError::Ambiguous
                } else {
                    BrokerError::Protocol
                })
            }
        }
    }
    fn call_sensitive(
        &self,
        binding: &ProviderBindingWireV1,
        metadata_digest: [u8; 32],
        operation: u16,
        mut payload: ScrubbedBytes,
        mutating: bool,
    ) -> Result<ScrubbedBytes, BrokerError> {
        self.call(
            binding,
            metadata_digest,
            operation,
            payload.take(),
            mutating,
        )
    }
}
