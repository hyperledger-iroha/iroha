//! Peer-authenticated Unix transport, SCM_RIGHTS framing, fixed client, and CLI.

use super::super::{SoftwareSignerWrappingKeyV1, load_software_signer_wrapping_key_from_fd_v1};
use super::{
    protocol::{
        AuthorityAdminCommandV1, AuthorityAdminRequestV1, AuthorizeRequestV1,
        FRAME_ADMIN_REQUEST_V1, FRAME_ADMIN_RESPONSE_V1, FRAME_AUTHORIZE_REQUEST_V1,
        FRAME_AUTHORIZE_RESPONSE_V1, FRAME_QUALIFY_REQUEST_V1, FRAME_QUALIFY_RESPONSE_V1,
        FRAME_VERIFY_REQUEST_V1, FRAME_VERIFY_RESPONSE_V1, OperationResponseV1, OperationStatusV1,
        QualifyRequestV1, QualifyResponseV1, TAIRA_AUTHORITY_MAX_ARTIFACTS_V1,
        TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1, TairaAuthorityPublicBindingV1, TairaAuthorityRoleV1,
        VerifyRequestV1, decode_body, decode_frame, encode_frame, qualify_response_digest,
    },
    service::{
        TairaAuthorityErrorV1, TairaAuthorityProvisioningV1, TairaAuthorityServiceV1,
        now_unix_millis, parse_digest, response_for_error,
    },
};
use crate::external_software_signer::{
    service::{verify_provenance, verify_response_attestation},
    unix::{BoundEndpointV1, bind_endpoint},
};
use clap::{Args, Parser, Subcommand};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use norito::{NoritoDeserialize, NoritoSerialize};
use rand::random;
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{IoSlice, IoSliceMut, Read as _, Write as _},
    mem::MaybeUninit,
    os::{
        fd::{AsFd as _, BorrowedFd, OwnedFd},
        unix::fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _},
        unix::net::UnixStream,
    },
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const IO_TIMEOUT_V1: Duration = Duration::from_secs(10);
const MAX_SESSIONS_V1: usize = 64;
const MAX_PUBLIC_BINDING_BYTES_V1: usize = 128 * 1024;

/// Exact service sockets and reviewed public binding used by a role.
#[derive(Clone, Debug)]
pub struct TairaAuthorityEndpointPolicyV1 {
    /// Request socket for the authorized client UID.
    pub request_socket: PathBuf,
    /// Administrator socket for the independent administrator UID.
    pub administrator_socket: PathBuf,
    /// Reviewed public role binding.
    pub binding: TairaAuthorityPublicBindingV1,
}

impl TairaAuthorityEndpointPolicyV1 {
    /// Validate endpoint paths and the complete public binding.
    pub fn try_new(
        request_socket: impl Into<PathBuf>,
        administrator_socket: impl Into<PathBuf>,
        binding: TairaAuthorityPublicBindingV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        let value = Self {
            request_socket: request_socket.into(),
            administrator_socket: administrator_socket.into(),
            binding,
        };
        value
            .binding
            .validate()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        if value.request_socket == value.administrator_socket
            || !absolute_normal(&value.request_socket)
            || !absolute_normal(&value.administrator_socket)
        {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        Ok(value)
    }
}

/// Fixed-endpoint native authority client.
#[derive(Clone, Debug)]
pub struct TairaAuthorityClientV1 {
    policy: TairaAuthorityEndpointPolicyV1,
}

impl TairaAuthorityClientV1 {
    /// Construct a client from an already authenticated installed policy.
    #[must_use]
    pub const fn new(policy: TairaAuthorityEndpointPolicyV1) -> Self {
        Self { policy }
    }

    /// Authenticate service availability before caller-controlled input is read.
    pub fn qualify(&self) -> Result<(), TairaAuthorityErrorV1> {
        self.qualify_status().map(drop)
    }

    /// Authenticate the request-side service and return its signed status object.
    pub fn status(&self) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        self.qualify_status()
    }

    fn qualify_status(&self) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let nonce = random_nonzero();
        let response: QualifyResponseV1 = self.exchange_request(
            FRAME_QUALIFY_REQUEST_V1,
            FRAME_QUALIFY_RESPONSE_V1,
            &QualifyRequestV1 {
                binding_sha256: self
                    .policy
                    .binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
                client_nonce: nonce,
            },
            &[],
        )?;
        if response.client_nonce != nonce
            || response.server_nonce == [0; 32]
            || response.provenance.binding != self.policy.binding.signer
            || response.status_json.is_empty()
            || response.response_digest
                != qualify_response_digest(&response).map_err(|()| TairaAuthorityErrorV1::Crypto)?
        {
            return Err(TairaAuthorityErrorV1::Crypto);
        }
        verify_provenance(&response.provenance).map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        verify_response_attestation(
            &self.policy.binding.signer,
            response.response_digest,
            &response.response_attestation,
        )
        .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        Ok(response.status_json)
    }

    /// Send one canonical authorization package and ordered artifact descriptors.
    pub fn authorize(
        &self,
        request_json: Vec<u8>,
        descriptors: &[BorrowedFd<'_>],
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let response: OperationResponseV1 = self.exchange_request(
            FRAME_AUTHORIZE_REQUEST_V1,
            FRAME_AUTHORIZE_RESPONSE_V1,
            &AuthorizeRequestV1 {
                binding_sha256: self
                    .policy
                    .binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
                request_json,
            },
            descriptors,
        )?;
        accepted_result(response)
    }

    /// Perform non-mutating historical receipt verification.
    pub fn verify_receipt(
        &self,
        request_json: Vec<u8>,
        descriptors: &[BorrowedFd<'_>],
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let response: OperationResponseV1 = self.exchange_request(
            FRAME_VERIFY_REQUEST_V1,
            FRAME_VERIFY_RESPONSE_V1,
            &VerifyRequestV1 {
                binding_sha256: self
                    .policy
                    .binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
                request_json,
            },
            descriptors,
        )?;
        accepted_result(response)
    }

    fn exchange_request<Request, Response>(
        &self,
        request_kind: u8,
        response_kind: u8,
        request: &Request,
        descriptors: &[BorrowedFd<'_>],
    ) -> Result<Response, TairaAuthorityErrorV1>
    where
        Request: NoritoSerialize,
        Response: NoritoSerialize,
        for<'de> Response: NoritoDeserialize<'de>,
    {
        exchange(
            &self.policy.request_socket,
            self.policy.binding.signer.service_uid,
            request_kind,
            response_kind,
            request,
            descriptors,
        )
    }

    fn administer(
        &self,
        command: AuthorityAdminCommandV1,
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let response: OperationResponseV1 = exchange(
            &self.policy.administrator_socket,
            self.policy.binding.signer.service_uid,
            FRAME_ADMIN_REQUEST_V1,
            FRAME_ADMIN_RESPONSE_V1,
            &AuthorityAdminRequestV1 {
                binding_sha256: self
                    .policy
                    .binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
                command,
            },
            &[],
        )?;
        accepted_result(response)
    }
}

/// Bound server for one exact authority role.
pub struct TairaAuthorityServerV1 {
    service: Arc<TairaAuthorityServiceV1>,
    policy: TairaAuthorityEndpointPolicyV1,
}

impl TairaAuthorityServerV1 {
    /// Bind a service to its reviewed public identity and endpoint policy.
    pub fn try_new(
        service: Arc<TairaAuthorityServiceV1>,
        policy: TairaAuthorityEndpointPolicyV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        if service.public_binding()? != policy.binding
            || rustix::process::geteuid().as_raw() != policy.binding.signer.service_uid
        {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        Ok(Self { service, policy })
    }

    /// Serve authenticated one-request sessions until SIGINT or SIGTERM.
    pub fn serve(self) -> Result<(), TairaAuthorityErrorV1> {
        let (request_listener, request_guard) = bind_endpoint(
            &self.policy.request_socket,
            self.policy.binding.signer.service_uid,
        )
        .map_err(|_| TairaAuthorityErrorV1::State)?;
        let (administrator_listener, administrator_guard) = bind_endpoint(
            &self.policy.administrator_socket,
            self.policy.binding.signer.service_uid,
        )
        .map_err(|_| TairaAuthorityErrorV1::State)?;
        serve_listeners(
            request_listener,
            administrator_listener,
            request_guard,
            administrator_guard,
            self.service,
            self.policy,
        )
    }
}

fn serve_listeners(
    request_listener: std::os::unix::net::UnixListener,
    administrator_listener: std::os::unix::net::UnixListener,
    request_guard: BoundEndpointV1,
    administrator_guard: BoundEndpointV1,
    service: Arc<TairaAuthorityServiceV1>,
    policy: TairaAuthorityEndpointPolicyV1,
) -> Result<(), TairaAuthorityErrorV1> {
    request_listener
        .set_nonblocking(true)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    administrator_listener
        .set_nonblocking(true)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let result = runtime.block_on(async move {
        let request_listener = tokio::net::UnixListener::from_std(request_listener)
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let administrator_listener = tokio::net::UnixListener::from_std(administrator_listener)
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let permits = Arc::new(tokio::sync::Semaphore::new(MAX_SESSIONS_V1));
        let mut tasks = tokio::task::JoinSet::new();
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(|_| TairaAuthorityErrorV1::State)?;
        loop {
            tokio::select! {
                accepted = request_listener.accept() => {
                    let (stream, _) = accepted.map_err(|_| TairaAuthorityErrorV1::State)?;
                    admit_session(
                        stream,
                        policy.binding.signer.client_uid,
                        false,
                        Arc::clone(&service),
                        Arc::clone(&permits),
                        &mut tasks,
                    )?;
                }
                accepted = administrator_listener.accept() => {
                    let (stream, _) = accepted.map_err(|_| TairaAuthorityErrorV1::State)?;
                    admit_session(
                        stream,
                        policy.binding.signer.administrator_uid,
                        true,
                        Arc::clone(&service),
                        Arc::clone(&permits),
                        &mut tasks,
                    )?;
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|_| TairaAuthorityErrorV1::State)?;
                    break;
                }
                _ = terminate.recv() => break,
                completed = tasks.join_next(), if !tasks.is_empty() => {
                    if completed.is_some_and(|result| result.is_err()) {
                        return Err(TairaAuthorityErrorV1::State);
                    }
                }
            }
        }
        while let Some(completed) = tasks.join_next().await {
            completed.map_err(|_| TairaAuthorityErrorV1::State)?;
        }
        Ok(())
    });
    let request_cleanup = request_guard
        .cleanup()
        .map_err(|_| TairaAuthorityErrorV1::State);
    let admin_cleanup = administrator_guard
        .cleanup()
        .map_err(|_| TairaAuthorityErrorV1::State);
    result?;
    request_cleanup?;
    admin_cleanup
}

fn admit_session(
    stream: tokio::net::UnixStream,
    expected_uid: u32,
    administrator: bool,
    service: Arc<TairaAuthorityServiceV1>,
    permits: Arc<tokio::sync::Semaphore>,
    tasks: &mut tokio::task::JoinSet<()>,
) -> Result<(), TairaAuthorityErrorV1> {
    let credentials = stream
        .peer_cred()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if credentials.uid() != expected_uid {
        return Ok(());
    }
    let Ok(permit) = permits.try_acquire_owned() else {
        return Ok(());
    };
    let stream = stream
        .into_std()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    stream
        .set_nonblocking(false)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    tasks.spawn_blocking(move || {
        let _permit = permit;
        let _ = serve_one(stream, administrator, expected_uid, &service);
    });
    Ok(())
}

fn serve_one(
    mut stream: UnixStream,
    administrator: bool,
    authenticated_uid: u32,
    service: &TairaAuthorityServiceV1,
) -> Result<(), TairaAuthorityErrorV1> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT_V1))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT_V1)))
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let (frame, descriptors) = receive_frame(&mut stream)?;
    let binding_sha256 = service
        .public_binding()?
        .sha256()
        .map_err(|()| TairaAuthorityErrorV1::Binding)?;
    let (kind, response) = if administrator {
        if frame.kind != FRAME_ADMIN_REQUEST_V1 || !descriptors.is_empty() {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let request: AuthorityAdminRequestV1 =
            decode_body(&frame.body).map_err(|()| TairaAuthorityErrorV1::Rejected)?;
        if request.binding_sha256 != binding_sha256 {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let response = service
            .administer(request.command, now_unix_millis()?)
            .unwrap_or_else(response_for_error);
        (FRAME_ADMIN_RESPONSE_V1, response)
    } else {
        match frame.kind {
            FRAME_QUALIFY_REQUEST_V1 if descriptors.is_empty() => {
                let request: QualifyRequestV1 =
                    decode_body(&frame.body).map_err(|()| TairaAuthorityErrorV1::Rejected)?;
                if request.binding_sha256 != binding_sha256 || request.client_nonce == [0; 32] {
                    return Err(TairaAuthorityErrorV1::Binding);
                }
                let response = QualifyResponseV1 {
                    client_nonce: request.client_nonce,
                    server_nonce: random_nonzero(),
                    provenance: service.provenance()?,
                    status_json: service.status_json()?,
                    response_digest: [0; 32],
                    response_attestation: Vec::new(),
                };
                let mut response = response;
                response.response_digest = qualify_response_digest(&response)
                    .map_err(|()| TairaAuthorityErrorV1::Crypto)?;
                response.response_attestation =
                    service.attest_response(response.response_digest)?;
                let encoded = encode_frame(FRAME_QUALIFY_RESPONSE_V1, &response)
                    .map_err(|()| TairaAuthorityErrorV1::State)?;
                write_frame(&mut stream, &encoded)?;
                return Ok(());
            }
            FRAME_AUTHORIZE_REQUEST_V1 => {
                let request: AuthorizeRequestV1 =
                    decode_body(&frame.body).map_err(|()| TairaAuthorityErrorV1::Rejected)?;
                if request.binding_sha256 != binding_sha256 {
                    return Err(TairaAuthorityErrorV1::Binding);
                }
                let response = service
                    .authorize_json(
                        &request.request_json,
                        descriptors,
                        authenticated_uid,
                        now_unix_millis()?,
                    )
                    .unwrap_or_else(response_for_error);
                (FRAME_AUTHORIZE_RESPONSE_V1, response)
            }
            FRAME_VERIFY_REQUEST_V1 => {
                let request: VerifyRequestV1 =
                    decode_body(&frame.body).map_err(|()| TairaAuthorityErrorV1::Rejected)?;
                if request.binding_sha256 != binding_sha256 {
                    return Err(TairaAuthorityErrorV1::Binding);
                }
                let response = service
                    .verify_json(&request.request_json, descriptors, authenticated_uid)
                    .unwrap_or_else(response_for_error);
                (FRAME_VERIFY_RESPONSE_V1, response)
            }
            _ => return Err(TairaAuthorityErrorV1::Rejected),
        }
    };
    let encoded = encode_frame(kind, &response).map_err(|()| TairaAuthorityErrorV1::State)?;
    write_frame(&mut stream, &encoded)
}

fn exchange<Request, Response>(
    endpoint: &Path,
    service_uid: u32,
    request_kind: u8,
    response_kind: u8,
    request: &Request,
    descriptors: &[BorrowedFd<'_>],
) -> Result<Response, TairaAuthorityErrorV1>
where
    Request: NoritoSerialize,
    Response: NoritoSerialize,
    for<'de> Response: NoritoDeserialize<'de>,
{
    validate_socket(endpoint, service_uid)?;
    let mut stream = UnixStream::connect(endpoint).map_err(|_| TairaAuthorityErrorV1::State)?;
    validate_connected_peer(&stream, service_uid)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT_V1))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT_V1)))
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    validate_socket(endpoint, service_uid)?;
    let encoded = encode_frame(request_kind, request).map_err(|()| TairaAuthorityErrorV1::State)?;
    send_frame(&mut stream, &encoded, descriptors)?;
    let (frame, unexpected) = receive_frame(&mut stream)?;
    if frame.kind != response_kind || !unexpected.is_empty() {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    decode_body(&frame.body).map_err(|()| TairaAuthorityErrorV1::Rejected)
}

fn validate_connected_peer(
    stream: &UnixStream,
    service_uid: u32,
) -> Result<(), TairaAuthorityErrorV1> {
    let clone = stream
        .try_clone()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    clone
        .set_nonblocking(true)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let guard = runtime.enter();
    let peer = tokio::net::UnixStream::from_std(clone)
        .map_err(|_| TairaAuthorityErrorV1::State)?
        .peer_cred()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    drop(guard);
    stream
        .set_nonblocking(false)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if peer.uid() != service_uid {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(())
}

fn send_frame(
    stream: &mut UnixStream,
    frame: &[u8],
    descriptors: &[BorrowedFd<'_>],
) -> Result<(), TairaAuthorityErrorV1> {
    if descriptors.len() > TAIRA_AUTHORITY_MAX_ARTIFACTS_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let length = u32::try_from(frame.len()).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut packet = Vec::with_capacity(frame.len() + 4);
    packet.extend_from_slice(&length.to_be_bytes());
    packet.extend_from_slice(frame);
    let mut ancillary_space = vec![
        MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(TAIRA_AUTHORITY_MAX_ARTIFACTS_V1))
    ];
    let mut ancillary = SendAncillaryBuffer::new(&mut ancillary_space);
    if !descriptors.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(descriptors)) {
        return Err(TairaAuthorityErrorV1::State);
    }
    let sent = sendmsg(
        &*stream,
        &[IoSlice::new(&packet)],
        &mut ancillary,
        SendFlags::empty(),
    )
    .map_err(|_| TairaAuthorityErrorV1::State)?;
    if sent == 0 || sent > packet.len() {
        return Err(TairaAuthorityErrorV1::State);
    }
    stream
        .write_all(&packet[sent..])
        .and_then(|()| stream.flush())
        .map_err(|_| TairaAuthorityErrorV1::State)
}

fn receive_frame(
    stream: &mut UnixStream,
) -> Result<(super::protocol::AuthorityFrameV1, Vec<OwnedFd>), TairaAuthorityErrorV1> {
    let mut packet = vec![0_u8; TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 + 4];
    let mut ancillary_space = vec![
        MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(TAIRA_AUTHORITY_MAX_ARTIFACTS_V1))
    ];
    let mut ancillary = RecvAncillaryBuffer::new(&mut ancillary_space);
    let mut iov = [IoSliceMut::new(&mut packet)];
    #[cfg(target_os = "linux")]
    let flags = RecvFlags::CMSG_CLOEXEC;
    #[cfg(not(target_os = "linux"))]
    let flags = RecvFlags::empty();
    let received = recvmsg(&*stream, &mut iov, &mut ancillary, flags)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if received.bytes < 4
        || received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut descriptors = Vec::new();
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
            _ => return Err(TairaAuthorityErrorV1::Rejected),
        }
    }
    if descriptors.len() > TAIRA_AUTHORITY_MAX_ARTIFACTS_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let declared = u32::from_be_bytes(packet[..4].try_into().expect("four-byte prefix"));
    let declared = usize::try_from(declared).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if declared == 0 || declared > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let already = received.bytes - 4;
    if already > declared {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    stream
        .read_exact(&mut packet[received.bytes..4 + declared])
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let frame =
        decode_frame(&packet[4..4 + declared]).map_err(|()| TairaAuthorityErrorV1::Rejected)?;
    Ok((frame, descriptors))
}

fn write_frame(stream: &mut UnixStream, frame: &[u8]) -> Result<(), TairaAuthorityErrorV1> {
    let length = u32::try_from(frame.len()).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    stream
        .write_all(&length.to_be_bytes())
        .and_then(|()| stream.write_all(frame))
        .and_then(|()| stream.flush())
        .map_err(|_| TairaAuthorityErrorV1::State)
}

fn accepted_result(response: OperationResponseV1) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    match response.status {
        OperationStatusV1::Ok | OperationStatusV1::Replayed if !response.result_json.is_empty() => {
            Ok(response.result_json)
        }
        OperationStatusV1::Conflict => Err(TairaAuthorityErrorV1::Conflict),
        OperationStatusV1::Unavailable => Err(TairaAuthorityErrorV1::State),
        _ => Err(TairaAuthorityErrorV1::Rejected),
    }
}

fn validate_socket(path: &Path, service_uid: u32) -> Result<(), TairaAuthorityErrorV1> {
    if !absolute_normal(path) {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| TairaAuthorityErrorV1::State)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.uid() != service_uid
        || metadata.mode() & 0o7777 != 0o666
        || metadata.nlink() != 1
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(())
}

fn absolute_normal(path: &Path) -> bool {
    path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
}

fn random_nonzero() -> [u8; 32] {
    loop {
        let value = random::<[u8; 32]>();
        if value != [0; 32] {
            return value;
        }
    }
}

fn fixed_paths(role: TairaAuthorityRoleV1) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    #[cfg(target_os = "linux")]
    let (configuration, runtime, state) = (
        Path::new("/etc/iroha/taira-authorities/v1"),
        Path::new("/run/iroha/taira-authorities/v1"),
        Path::new("/var/lib/iroha/taira-authorities/v1"),
    );
    #[cfg(target_os = "macos")]
    let (configuration, runtime, state) = (
        Path::new("/private/etc/iroha/taira-authorities/v1"),
        Path::new("/private/var/run/iroha/taira-authorities/v1"),
        Path::new("/private/var/db/iroha/taira-authorities/v1"),
    );
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let (configuration, runtime, state) = (
        Path::new("/etc/iroha/taira-authorities/v1"),
        Path::new("/run/iroha/taira-authorities/v1"),
        Path::new("/var/lib/iroha/taira-authorities/v1"),
    );
    let role = role.as_str();
    (
        configuration.join(role).join("binding-v1.norito"),
        runtime.join(role).join("request-v1.sock"),
        runtime.join(role).join("administrator-v1.sock"),
        state.join(role),
    )
}

fn load_fixed_client(
    role: TairaAuthorityRoleV1,
) -> Result<TairaAuthorityClientV1, TairaAuthorityErrorV1> {
    let (binding_path, request_socket, administrator_socket, _) = fixed_paths(role);
    let binding = read_public_binding(&binding_path, BindingOwnerPolicyV1::Root)?;
    let expected_service_id = format!("taira-authority-{}-v1", role.as_str());
    let expected_administrator_id = format!("taira-authority-{}-administrator-v1", role.as_str());
    if binding.role != role
        || binding.signer.service_id != expected_service_id
        || binding.signer.administrator_id != expected_administrator_id
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(TairaAuthorityClientV1::new(
        TairaAuthorityEndpointPolicyV1::try_new(request_socket, administrator_socket, binding)?,
    ))
}

#[derive(Clone, Copy)]
enum BindingOwnerPolicyV1 {
    Root,
    RootOrEffectiveUser,
}

fn read_public_binding(
    path: &Path,
    owner_policy: BindingOwnerPolicyV1,
) -> Result<TairaAuthorityPublicBindingV1, TairaAuthorityErrorV1> {
    if !absolute_normal(path) {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let before = file.metadata().map_err(|_| TairaAuthorityErrorV1::State)?;
    let valid_owner = match owner_policy {
        BindingOwnerPolicyV1::Root => before.uid() == 0,
        BindingOwnerPolicyV1::RootOrEffectiveUser => {
            before.uid() == 0 || before.uid() == rustix::process::geteuid().as_raw()
        }
    };
    if !before.is_file()
        || !valid_owner
        || before.mode() & 0o022 != 0
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > u64::try_from(MAX_PUBLIC_BINDING_BYTES_V1).unwrap_or(u64::MAX)
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(u64::try_from(MAX_PUBLIC_BINDING_BYTES_V1 + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let after = file.metadata().map_err(|_| TairaAuthorityErrorV1::State)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let binding: TairaAuthorityPublicBindingV1 =
        norito::decode_canonical(&bytes).map_err(|_| TairaAuthorityErrorV1::Binding)?;
    binding
        .validate()
        .map_err(|()| TairaAuthorityErrorV1::Binding)?;
    Ok(binding)
}

#[derive(Debug, Parser)]
#[command(name = "taira_release_authority", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Provision(ProvisionArgs),
    Serve(ServeArgs),
    AssignRun(RoleArgs),
    Authorize(ArtifactArgs),
    Recover(RecoverArgs),
    VerifyReceipt(ArtifactArgs),
    Status(RoleArgs),
    Rotate(RotateArgs),
    Revoke(RevokeArgs),
}

#[derive(Debug, Args)]
struct RoleArgs {
    #[arg(long)]
    role: TairaAuthorityRoleV1,
}

#[derive(Debug, Args)]
struct ArtifactArgs {
    #[arg(long)]
    role: TairaAuthorityRoleV1,
    #[arg(long = "artifact-fd", value_name = "FD")]
    artifact_fds: Vec<i32>,
}

#[derive(Debug, Args)]
struct ProvisionArgs {
    #[arg(long)]
    role: TairaAuthorityRoleV1,
    #[arg(long)]
    state_directory: PathBuf,
    #[arg(long)]
    binding_out: PathBuf,
    #[arg(long)]
    service_id: String,
    #[arg(long)]
    administrator_id: String,
    #[arg(long)]
    service_uid: u32,
    #[arg(long)]
    client_uid: u32,
    #[arg(long)]
    administrator_uid: u32,
    #[arg(long, default_value_t = 1)]
    key_revision: u64,
    #[arg(long, default_value_t = 1)]
    policy_revision: u64,
    #[arg(long)]
    policy_sha256: String,
    #[arg(long, default_value_t = 33_554_432)]
    max_request_bytes: u32,
    #[arg(long)]
    wrapping_key_fd: i32,
    #[arg(long)]
    retained_genesis_key_fd: Option<i32>,
    #[arg(long)]
    observation_binding_fd: Option<i32>,
}

#[derive(Debug, Args)]
struct ServeArgs {
    #[arg(long)]
    state_directory: PathBuf,
    #[arg(long)]
    binding: PathBuf,
    #[arg(long)]
    request_socket: PathBuf,
    #[arg(long)]
    administrator_socket: PathBuf,
    #[arg(long)]
    wrapping_key_fd: i32,
}

#[derive(Debug, Args)]
struct RecoverArgs {
    #[arg(long)]
    state_directory: PathBuf,
    #[arg(long)]
    binding: PathBuf,
    #[arg(long)]
    wrapping_key_fd: i32,
}

#[derive(Debug, Args)]
struct RotateArgs {
    #[arg(long)]
    role: TairaAuthorityRoleV1,
    #[arg(long)]
    operation_id: String,
    #[arg(long)]
    expected_audit_head: String,
    #[arg(long)]
    expected_key_revision: u64,
    #[arg(long)]
    new_key_revision: u64,
    #[arg(long)]
    new_policy_revision: u64,
    #[arg(long)]
    new_policy_sha256: String,
}

#[derive(Debug, Args)]
struct RevokeArgs {
    #[arg(long)]
    role: TairaAuthorityRoleV1,
    #[arg(long)]
    operation_id: String,
    #[arg(long)]
    expected_audit_head: String,
    #[arg(long)]
    expected_key_revision: u64,
    #[arg(long)]
    reason_sha256: String,
}

pub(super) fn run_cli() -> Result<(), &'static str> {
    match Cli::parse().command {
        Command::Provision(args) => cli_provision(args),
        Command::Serve(args) => cli_serve(args),
        Command::AssignRun(args) => {
            let client = load_fixed_client(args.role).map_err(cli_error)?;
            client
                .administer(AuthorityAdminCommandV1::Status)
                .map_err(cli_error)?;
            let input = read_stdin_bounded().map_err(cli_error)?;
            let output = client
                .administer(AuthorityAdminCommandV1::AssignRun {
                    assignment_json: input,
                })
                .map_err(cli_error)?;
            write_stdout(&output)
        }
        Command::Authorize(args) => cli_artifact_operation(args, false),
        Command::VerifyReceipt(args) => cli_artifact_operation(args, true),
        Command::Recover(args) => cli_recover(args),
        Command::Status(args) => {
            let client = load_fixed_client(args.role).map_err(cli_error)?;
            let output = client.status().map_err(cli_error)?;
            write_stdout(&output)
        }
        Command::Rotate(args) => {
            let client = load_fixed_client(args.role).map_err(cli_error)?;
            client
                .administer(AuthorityAdminCommandV1::Status)
                .map_err(cli_error)?;
            let output = client
                .administer(AuthorityAdminCommandV1::Rotate {
                    operation_id: parse_digest(&args.operation_id).map_err(cli_error)?,
                    expected_audit_head: parse_digest(&args.expected_audit_head)
                        .map_err(cli_error)?,
                    expected_key_revision: args.expected_key_revision,
                    new_key_revision: args.new_key_revision,
                    new_policy_revision: args.new_policy_revision,
                    new_policy_digest: parse_digest(&args.new_policy_sha256).map_err(cli_error)?,
                })
                .map_err(cli_error)?;
            write_stdout(&output)
        }
        Command::Revoke(args) => {
            let client = load_fixed_client(args.role).map_err(cli_error)?;
            client
                .administer(AuthorityAdminCommandV1::Status)
                .map_err(cli_error)?;
            let output = client
                .administer(AuthorityAdminCommandV1::Revoke {
                    operation_id: parse_digest(&args.operation_id).map_err(cli_error)?,
                    expected_audit_head: parse_digest(&args.expected_audit_head)
                        .map_err(cli_error)?,
                    expected_key_revision: args.expected_key_revision,
                    reason_digest: parse_digest(&args.reason_sha256).map_err(cli_error)?,
                })
                .map_err(cli_error)?;
            write_stdout(&output)
        }
    }
}

fn cli_provision(args: ProvisionArgs) -> Result<(), &'static str> {
    let wrapping_key = load_wrapping_key(args.wrapping_key_fd).map_err(cli_error)?;
    let provisioning = TairaAuthorityProvisioningV1 {
        role: args.role,
        service_id: args.service_id,
        administrator_id: args.administrator_id,
        service_uid: args.service_uid,
        client_uid: args.client_uid,
        administrator_uid: args.administrator_uid,
        key_revision: args.key_revision,
        policy_revision: args.policy_revision,
        policy_digest: parse_digest(&args.policy_sha256).map_err(cli_error)?,
        max_request_bytes: args.max_request_bytes,
    };
    let service = match (
        args.role,
        args.retained_genesis_key_fd,
        args.observation_binding_fd,
    ) {
        (TairaAuthorityRoleV1::PrivacyGovernance, Some(descriptor), None) => {
            let retained_key = load_retained_genesis_key(descriptor).map_err(cli_error)?;
            TairaAuthorityServiceV1::provision_with_retained_genesis_key(
                args.state_directory,
                provisioning,
                wrapping_key,
                retained_key,
            )
        }
        (TairaAuthorityRoleV1::PublicSoakReplayAdmission, None, Some(descriptor)) => {
            let observation = load_inherited_public_binding(descriptor).map_err(cli_error)?;
            TairaAuthorityServiceV1::provision_with_public_soak_observation_binding(
                args.state_directory,
                provisioning,
                wrapping_key,
                observation,
            )
        }
        (TairaAuthorityRoleV1::PrivacyGovernance, _, _)
        | (TairaAuthorityRoleV1::PublicSoakReplayAdmission, _, _)
        | (_, Some(_), _)
        | (_, _, Some(_)) => {
            return Err("authority command failed");
        }
        (_, None, None) => {
            TairaAuthorityServiceV1::provision(args.state_directory, provisioning, wrapping_key)
        }
    }
    .map_err(cli_error)?;
    write_binding_new(
        &args.binding_out,
        &service.public_binding().map_err(cli_error)?,
    )
}

fn load_inherited_public_binding(
    fd: i32,
) -> Result<TairaAuthorityPublicBindingV1, TairaAuthorityErrorV1> {
    let descriptor = duplicate_inherited_descriptor(fd)?;
    let mut bytes = Vec::new();
    File::from(descriptor)
        .take(u64::try_from(MAX_PUBLIC_BINDING_BYTES_V1 + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if bytes.is_empty() || bytes.len() > MAX_PUBLIC_BINDING_BYTES_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let binding: TairaAuthorityPublicBindingV1 =
        norito::decode_canonical(&bytes).map_err(|_| TairaAuthorityErrorV1::Binding)?;
    binding
        .validate()
        .map_err(|()| TairaAuthorityErrorV1::Binding)?;
    Ok(binding)
}

fn load_retained_genesis_key(fd: i32) -> Result<KeyPair, TairaAuthorityErrorV1> {
    let descriptor = duplicate_inherited_descriptor(fd)?;
    let mut bytes = Vec::new();
    File::from(descriptor)
        .take(33)
        .read_to_end(&mut bytes)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if bytes.len() != 32 {
        bytes.fill(0);
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let private = PrivateKey::from_bytes(Algorithm::Ed25519, &bytes)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    bytes.fill(0);
    KeyPair::from_private_key(private).map_err(|_| TairaAuthorityErrorV1::Rejected)
}

fn cli_serve(args: ServeArgs) -> Result<(), &'static str> {
    let binding = read_public_binding(&args.binding, BindingOwnerPolicyV1::RootOrEffectiveUser)
        .map_err(cli_error)?;
    let wrapping_key = load_wrapping_key(args.wrapping_key_fd).map_err(cli_error)?;
    let service = Arc::new(
        TairaAuthorityServiceV1::open(args.state_directory, wrapping_key).map_err(cli_error)?,
    );
    let policy = TairaAuthorityEndpointPolicyV1::try_new(
        args.request_socket,
        args.administrator_socket,
        binding,
    )
    .map_err(cli_error)?;
    TairaAuthorityServerV1::try_new(service, policy)
        .and_then(TairaAuthorityServerV1::serve)
        .map_err(cli_error)
}

fn cli_recover(args: RecoverArgs) -> Result<(), &'static str> {
    let binding = read_public_binding(&args.binding, BindingOwnerPolicyV1::RootOrEffectiveUser)
        .map_err(cli_error)?;
    let wrapping_key = load_wrapping_key(args.wrapping_key_fd).map_err(cli_error)?;
    let service =
        TairaAuthorityServiceV1::open(args.state_directory, wrapping_key).map_err(cli_error)?;
    if service.public_binding().map_err(cli_error)? != binding {
        return Err("authority command failed");
    }
    write_stdout(&service.status_json().map_err(cli_error)?)
}

fn cli_artifact_operation(args: ArtifactArgs, verify: bool) -> Result<(), &'static str> {
    let client = load_fixed_client(args.role).map_err(cli_error)?;
    client.qualify().map_err(cli_error)?;
    let input = read_stdin_bounded().map_err(cli_error)?;
    let descriptors = duplicate_inherited_descriptors(&args.artifact_fds).map_err(cli_error)?;
    let borrowed = descriptors
        .iter()
        .map(|descriptor| descriptor.as_fd())
        .collect::<Vec<_>>();
    let output = if verify {
        client.verify_receipt(input, &borrowed)
    } else {
        client.authorize(input, &borrowed)
    }
    .map_err(cli_error)?;
    write_stdout(&output)
}

fn load_wrapping_key(fd: i32) -> Result<SoftwareSignerWrappingKeyV1, TairaAuthorityErrorV1> {
    let descriptor = duplicate_inherited_descriptor(fd)?;
    load_software_signer_wrapping_key_from_fd_v1(descriptor)
        .map_err(|_| TairaAuthorityErrorV1::State)
}

fn duplicate_inherited_descriptors(values: &[i32]) -> Result<Vec<OwnedFd>, TairaAuthorityErrorV1> {
    if values.len() > TAIRA_AUTHORITY_MAX_ARTIFACTS_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    values
        .iter()
        .map(|value| duplicate_inherited_descriptor(*value))
        .collect()
}

fn duplicate_inherited_descriptor(value: i32) -> Result<OwnedFd, TairaAuthorityErrorV1> {
    if value <= 2 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let namespace = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    File::open(format!("{namespace}/{value}"))
        .map(OwnedFd::from)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)
}

fn read_stdin_bounded() -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(
            u64::try_from(super::protocol::TAIRA_AUTHORITY_MAX_JSON_BYTES_V1 + 1)
                .unwrap_or(u64::MAX),
        )
        .read_to_end(&mut bytes)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if bytes.is_empty() || bytes.len() > super::protocol::TAIRA_AUTHORITY_MAX_JSON_BYTES_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(bytes)
}

fn write_stdout(bytes: &[u8]) -> Result<(), &'static str> {
    std::io::stdout()
        .write_all(bytes)
        .and_then(|()| std::io::stdout().flush())
        .map_err(|_| "authority output failed")
}

fn write_binding_new(
    path: &Path,
    binding: &TairaAuthorityPublicBindingV1,
) -> Result<(), &'static str> {
    if !absolute_normal(path) {
        return Err("authority output failed");
    }
    let bytes = norito::encode_canonical(binding).map_err(|_| "authority output failed")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)
        .map_err(|_| "authority output failed")?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "authority output failed")
}

const fn cli_error(_: TairaAuthorityErrorV1) -> &'static str {
    "Taira release authority failed closed"
}
