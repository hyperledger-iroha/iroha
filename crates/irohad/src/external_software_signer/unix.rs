//! Peer-credential-authenticated Unix transport and runtime credential loading.
use super::{
    SoftwareSignerWrappingKeyV1,
    protocol::{
        AdminCommandV1, AdminRequestV1, AdminResponseV1, AdminStatusV1, QualifyRequestV1,
        QualifyResponseV1, SIGNER_FRAME_ADMIN_REQUEST_V1, SIGNER_FRAME_ADMIN_RESPONSE_V1,
        SIGNER_FRAME_QUALIFY_REQUEST_V1, SIGNER_FRAME_QUALIFY_RESPONSE_V1,
        SIGNER_FRAME_SIGN_REQUEST_V1, SIGNER_FRAME_SIGN_RESPONSE_V1, SIGNER_MAX_FRAME_BYTES_V1,
        SIGNER_PROTOCOL_MAGIC_V1, SIGNER_PROTOCOL_VERSION_V1, SignRequestV1, SignResponseV1,
        SignStatusV1, SoftwareSignerFrameV1, SoftwareSignerKeyAlgorithmV1,
        SoftwareSignerLiveProvenanceV1, SoftwareSignerPublicBindingV1, SoftwareSignerRoleV1,
        admin_request_digest, admin_response_digest, payload_digest, qualify_response_digest,
        scrub, sign_request_digest, sign_response_digest,
    },
    service::{
        SoftwareSignerServiceV1, native_payload_matches_role, taira_authority_signing_message,
        verify_provenance, verify_response_attestation,
    },
};
use iroha_crypto::Signature;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::{
        fd::{AsRawFd as _, OwnedFd},
        unix::{
            fs::{FileTypeExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
            net::UnixStream,
        },
    },
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
const SOCKET_MODE_V1: u32 = 0o666;
const RUNTIME_DIRECTORY_MODE_V1: u32 = 0o711;
const IO_TIMEOUT_V1: Duration = Duration::from_secs(10);
const MAX_SESSIONS_V1: usize = 64;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Exact request/admin endpoint paths and expected software-signer identity.
#[derive(Clone, Debug)]
pub struct SoftwareSignerEndpointPolicyV1 {
    /// Client-signing Unix socket.
    pub request_socket: PathBuf,
    /// Independently administered Unix socket.
    pub administrator_socket: PathBuf,
    /// Exact public service binding pinned by callers and the server launcher.
    pub expected_binding: SoftwareSignerPublicBindingV1,
}
impl SoftwareSignerEndpointPolicyV1 {
    /// Construct and validate an endpoint policy without accessing either socket.
    ///
    /// # Errors
    ///
    /// Rejects relative/non-normal paths, a shared request/admin path, distinct
    /// runtime directories, or an invalid public binding.
    pub fn try_new(
        request_socket: impl Into<PathBuf>,
        administrator_socket: impl Into<PathBuf>,
        expected_binding: SoftwareSignerPublicBindingV1,
    ) -> Result<Self, SoftwareSignerServerErrorV1> {
        let policy = Self {
            request_socket: request_socket.into(),
            administrator_socket: administrator_socket.into(),
            expected_binding,
        };
        policy.validate_paths()?;
        policy
            .expected_binding
            .validate()
            .map_err(|()| SoftwareSignerServerErrorV1::BindingMismatch)?;
        Ok(policy)
    }
    fn validate_paths(&self) -> Result<(), SoftwareSignerServerErrorV1> {
        validate_absolute_normal_path(&self.request_socket)?;
        validate_absolute_normal_path(&self.administrator_socket)?;
        if self.request_socket == self.administrator_socket
            || self.request_socket.parent().is_none()
            || self.request_socket.parent() != self.administrator_socket.parent()
        {
            return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
        }
        Ok(())
    }
}
/// Public, payload-free evidence returned for one durably committed signature.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SoftwareSignerSignatureReceiptV1 {
    /// Caller-selected replay/idempotency identifier.
    pub operation_id: [u8; 32],
    /// Canonical request-metadata digest.
    pub request_digest: [u8; 32],
    /// Domain-separated digest of the exact supplied payload bytes.
    pub payload_digest: [u8; 32],
    /// Exact supplied payload byte length.
    pub payload_length: u64,
    /// Raw Ed25519 or ML-DSA signature payload.
    pub signature: Vec<u8>,
    /// Sequence of the immutable audit commit containing this signature.
    pub commit_sequence: u64,
    /// Hash-chain head at the immutable audit commit.
    pub commit_audit_head: [u8; 32],
    /// `true` when the exact prior committed result was replayed.
    pub replayed: bool,
    /// Active software backend, identities, role, policy, key, and audit head.
    pub provenance: SoftwareSignerLiveProvenanceV1,
    /// Canonical digest of the complete correlated response fields.
    pub response_digest: [u8; 32],
    /// Active-key signature binding the response digest to live provenance.
    pub response_attestation: Vec<u8>,
}
fn valid_receipt_commit_position(
    role: SoftwareSignerRoleV1,
    replayed: bool,
    commit_sequence: u64,
    commit_audit_head: [u8; 32],
    provenance: &SoftwareSignerLiveProvenanceV1,
) -> bool {
    if commit_sequence == 0
        || commit_audit_head == [0; 32]
        || commit_sequence > provenance.audit_sequence
    {
        return false;
    }
    let live =
        commit_sequence == provenance.audit_sequence && commit_audit_head == provenance.audit_head;
    if live {
        return true;
    }
    replayed
        && role != SoftwareSignerRoleV1::Promotion
        && commit_sequence < provenance.audit_sequence
        && commit_audit_head != provenance.audit_head
}
impl SoftwareSignerSignatureReceiptV1 {
    /// Verify a public receipt without contacting the signer service.
    ///
    /// This binds the reviewed service identity, exact payload bytes, detached signature, operation
    /// identifier, durable audit commit, live provenance, response digest, and both active-key
    /// attestations. Revoked provenance is never accepted.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure class for any mismatch or invalid signature.
    pub fn verify_offline(
        &self,
        expected_binding: &SoftwareSignerPublicBindingV1,
        expected_operation_id: [u8; 32],
        payload: &[u8],
        detached_signature: &[u8],
    ) -> Result<(), ExternalSoftwareSignerClientErrorV1> {
        expected_binding
            .validate()
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        if expected_operation_id == [0; 32]
            || self.operation_id != expected_operation_id
            || self.provenance.binding != *expected_binding
            || self.provenance.revoked
            || payload.is_empty()
            || payload.len()
                > usize::try_from(expected_binding.max_request_bytes)
                    .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected)?
            || self.payload_length
                != u64::try_from(payload.len())
                    .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected)?
            || self.payload_digest != payload_digest(payload)
            || self.signature != detached_signature
            || self.signature.len()
                != expected_binding
                    .key_algorithm
                    .algorithm()
                    .signature_payload_len()
            || !valid_receipt_commit_position(
                expected_binding.role,
                self.replayed,
                self.commit_sequence,
                self.commit_audit_head,
                &self.provenance,
            )
        {
            return Err(ExternalSoftwareSignerClientErrorV1::BindingMismatch);
        }
        let request = SignRequestV1 {
            binding_digest: expected_binding
                .digest()
                .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?,
            operation_id: expected_operation_id,
            expected_key_revision: expected_binding.key_revision,
            expected_policy_revision: expected_binding.policy_revision,
            expected_policy_digest: expected_binding.policy_digest,
            payload_digest: self.payload_digest,
            payload: payload.to_vec(),
            request_digest: [0; 32],
        };
        if self.request_digest
            != sign_request_digest(&request)
                .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        verify_live_provenance(expected_binding, &self.provenance)?;
        let response = SignResponseV1 {
            operation_id: self.operation_id,
            request_digest: self.request_digest,
            payload_digest: self.payload_digest,
            status: if self.replayed {
                SignStatusV1::Replayed
            } else {
                SignStatusV1::Ok
            },
            signature: self.signature.clone(),
            commit_sequence: self.commit_sequence,
            commit_audit_head: self.commit_audit_head,
            provenance: self.provenance.clone(),
            response_digest: [0; 32],
            response_attestation: Vec::new(),
        };
        if self.response_digest
            != sign_response_digest(&response)
                .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        verify_response_attestation(
            expected_binding,
            self.response_digest,
            &self.response_attestation,
        )
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        verify_payload_signature(expected_binding, payload, detached_signature)
    }
}
/// Synchronous request-side client used by irohad runtime-provider adapters.
#[derive(Clone, Debug)]
pub struct SoftwareSignerClientV1 {
    policy: SoftwareSignerEndpointPolicyV1,
    #[cfg(test)]
    direct_service: Option<Arc<SoftwareSignerServiceV1>>,
}
impl SoftwareSignerClientV1 {
    /// Pin a client to an exact public signer binding and endpoint identity.
    pub fn new(policy: SoftwareSignerEndpointPolicyV1) -> Self {
        Self {
            policy,
            #[cfg(test)]
            direct_service: None,
        }
    }
    #[cfg(test)]
    pub(super) fn new_direct(
        service: Arc<SoftwareSignerServiceV1>,
    ) -> Result<Self, ExternalSoftwareSignerClientErrorV1> {
        let expected_binding = service
            .public_binding()
            .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
        let policy = SoftwareSignerEndpointPolicyV1::try_new(
            PathBuf::from("/tmp/iroha-software-signer-direct/request.sock"),
            PathBuf::from("/tmp/iroha-software-signer-direct/administrator.sock"),
            expected_binding,
        )
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        Ok(Self {
            policy,
            direct_service: Some(service),
        })
    }
    /// Return the expected immutable binding.
    #[must_use]
    pub const fn expected_binding(&self) -> &SoftwareSignerPublicBindingV1 {
        &self.policy.expected_binding
    }
    /// Perform a nonce-bound public qualification probe.
    ///
    /// # Errors
    ///
    /// Rejects endpoint substitution, wrong service UID, stale/revoked
    /// provenance, noncanonical frames, or an invalid response attestation.
    pub fn qualify(
        &self,
    ) -> Result<SoftwareSignerLiveProvenanceV1, ExternalSoftwareSignerClientErrorV1> {
        #[cfg(test)]
        if let Some(service) = &self.direct_service {
            let provenance = service
                .provenance()
                .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
            verify_live_provenance(&self.policy.expected_binding, &provenance)?;
            return Ok(provenance);
        }
        let binding_digest = self
            .policy
            .expected_binding
            .digest()
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        let client_nonce = random_nonzero();
        let request = QualifyRequestV1 {
            binding_digest,
            client_nonce,
        };
        let response: QualifyResponseV1 = exchange(
            &self.policy.request_socket,
            self.policy.expected_binding.service_uid,
            SIGNER_FRAME_QUALIFY_REQUEST_V1,
            &request,
            SIGNER_FRAME_QUALIFY_RESPONSE_V1,
        )?;
        if response.client_nonce != client_nonce
            || response.server_nonce == [0; 32]
            || response.response_digest
                != qualify_response_digest(&response)
                    .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        verify_live_provenance(&self.policy.expected_binding, &response.provenance)?;
        verify_response_attestation(
            &response.provenance.binding,
            response.response_digest,
            &response.response_attestation,
        )
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        Ok(response.provenance)
    }
    /// Sign exact role-scoped bytes with replay-safe idempotency.
    ///
    /// # Errors
    ///
    /// Returns a payload-free class for malformed, equivocated, stale,
    /// revoked, substituted, or unavailable operations.
    pub fn sign(
        &self,
        operation_id: [u8; 32],
        payload: &[u8],
    ) -> Result<SoftwareSignerSignatureReceiptV1, ExternalSoftwareSignerClientErrorV1> {
        if operation_id == [0; 32]
            || payload.is_empty()
            || payload.len()
                > usize::try_from(self.policy.expected_binding.max_request_bytes)
                    .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
        }
        let mut request = SignRequestV1 {
            binding_digest: self
                .policy
                .expected_binding
                .digest()
                .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?,
            operation_id,
            expected_key_revision: self.policy.expected_binding.key_revision,
            expected_policy_revision: self.policy.expected_binding.policy_revision,
            expected_policy_digest: self.policy.expected_binding.policy_digest,
            payload_digest: payload_digest(payload),
            payload: payload.to_vec(),
            request_digest: [0; 32],
        };
        request.request_digest = sign_request_digest(&request)
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?;
        let response = self.exchange_sign_request(&request)?;
        if response.operation_id != operation_id
            || response.request_digest != request.request_digest
            || response.payload_digest != request.payload_digest
            || response.response_digest
                != sign_response_digest(&response)
                    .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        verify_live_provenance(&self.policy.expected_binding, &response.provenance)?;
        verify_response_attestation(
            &response.provenance.binding,
            response.response_digest,
            &response.response_attestation,
        )
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        match response.status {
            SignStatusV1::Ok | SignStatusV1::Replayed => {}
            SignStatusV1::Rejected => return Err(ExternalSoftwareSignerClientErrorV1::Rejected),
            SignStatusV1::Equivocation => {
                return Err(ExternalSoftwareSignerClientErrorV1::Equivocation);
            }
            SignStatusV1::StaleOrRevoked => {
                return Err(ExternalSoftwareSignerClientErrorV1::StaleOrRevoked);
            }
            SignStatusV1::Unavailable => {
                return Err(ExternalSoftwareSignerClientErrorV1::Unavailable);
            }
        }
        let replayed = response.status == SignStatusV1::Replayed;
        if response.signature.is_empty()
            || !valid_receipt_commit_position(
                self.policy.expected_binding.role,
                replayed,
                response.commit_sequence,
                response.commit_audit_head,
                &response.provenance,
            )
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        verify_payload_signature(&self.policy.expected_binding, payload, &response.signature)?;
        Ok(SoftwareSignerSignatureReceiptV1 {
            operation_id,
            request_digest: response.request_digest,
            payload_digest: response.payload_digest,
            payload_length: u64::try_from(payload.len())
                .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected)?,
            signature: response.signature,
            commit_sequence: response.commit_sequence,
            commit_audit_head: response.commit_audit_head,
            replayed,
            provenance: response.provenance,
            response_digest: response.response_digest,
            response_attestation: response.response_attestation,
        })
    }
    fn exchange_sign_request(
        &self,
        request: &SignRequestV1,
    ) -> Result<SignResponseV1, ExternalSoftwareSignerClientErrorV1> {
        #[cfg(test)]
        if let Some(service) = &self.direct_service {
            return service
                .handle_sign_request(request)
                .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected);
        }
        exchange(
            &self.policy.request_socket,
            self.policy.expected_binding.service_uid,
            SIGNER_FRAME_SIGN_REQUEST_V1,
            request,
            SIGNER_FRAME_SIGN_RESPONSE_V1,
        )
    }
}
/// Administrator-side client confined to the administrator Unix socket.
#[derive(Clone, Debug)]
pub struct SoftwareSignerAdministratorClientV1 {
    policy: SoftwareSignerEndpointPolicyV1,
}
/// Complete predecessor-bound request for one monotonic signer rotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftwareSignerRotationRequestV1 {
    /// Stable non-zero administrator idempotency key.
    pub operation_id: [u8; 32],
    /// Exact signed audit head that must immediately precede the rotation.
    pub expected_audit_head: [u8; 32],
    /// Exact active key revision being superseded.
    pub expected_key_revision: u64,
    /// Strictly greater successor key revision.
    pub new_key_revision: u64,
    /// Strictly greater successor policy revision.
    pub new_policy_revision: u64,
    /// SHA-256 digest of the reviewed successor policy bytes.
    pub new_policy_digest: [u8; 32],
    /// Signature algorithm for the successor key.
    pub algorithm: SoftwareSignerKeyAlgorithmV1,
}
impl SoftwareSignerAdministratorClientV1 {
    /// Pin an administrator client to the current reviewed binding.
    pub fn new(policy: SoftwareSignerEndpointPolicyV1) -> Self {
        Self { policy }
    }
    /// Read signed live state without mutating the journal.
    ///
    /// # Errors
    ///
    /// Rejects unauthenticated endpoints, substituted bindings, invalid signed
    /// provenance, or an unavailable signer.
    pub fn status(
        &self,
    ) -> Result<SoftwareSignerLiveProvenanceV1, ExternalSoftwareSignerClientErrorV1> {
        self.command(AdminCommandV1::Status, None)
    }
    /// Rotate to a monotonic successor key and public policy.
    ///
    /// # Errors
    ///
    /// Rejects stale predecessors, non-monotonic revisions, unauthorized algorithms, idempotency
    /// conflicts, substituted bindings, or unavailable durable persistence.
    pub fn rotate(
        &self,
        request: SoftwareSignerRotationRequestV1,
    ) -> Result<SoftwareSignerLiveProvenanceV1, ExternalSoftwareSignerClientErrorV1> {
        if !self
            .policy
            .expected_binding
            .role
            .allows_algorithm(request.algorithm)
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
        }
        self.command(
            AdminCommandV1::Rotate {
                operation_id: request.operation_id,
                expected_audit_head: request.expected_audit_head,
                expected_key_revision: request.expected_key_revision,
                new_key_revision: request.new_key_revision,
                new_policy_revision: request.new_policy_revision,
                new_policy_digest: request.new_policy_digest,
                algorithm: request.algorithm,
            },
            Some((
                request.new_key_revision,
                request.new_policy_revision,
                request.new_policy_digest,
                request.algorithm,
            )),
        )
    }
    /// Irreversibly revoke the active key generation.
    ///
    /// # Errors
    ///
    /// Rejects stale predecessors, malformed reasons, idempotency conflicts,
    /// substituted bindings, or unavailable durable persistence.
    pub fn revoke(
        &self,
        operation_id: [u8; 32],
        expected_audit_head: [u8; 32],
        expected_key_revision: u64,
        reason_digest: [u8; 32],
    ) -> Result<SoftwareSignerLiveProvenanceV1, ExternalSoftwareSignerClientErrorV1> {
        let provenance = self.command(
            AdminCommandV1::Revoke {
                operation_id,
                expected_audit_head,
                expected_key_revision,
                reason_digest,
            },
            None,
        )?;
        if !provenance.revoked {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        Ok(provenance)
    }
    fn command(
        &self,
        command: AdminCommandV1,
        expected_rotation: Option<(u64, u64, [u8; 32], SoftwareSignerKeyAlgorithmV1)>,
    ) -> Result<SoftwareSignerLiveProvenanceV1, ExternalSoftwareSignerClientErrorV1> {
        let binding_digest = self
            .policy
            .expected_binding
            .digest()
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        let request_digest = admin_request_digest(binding_digest, &command)
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?;
        let request = AdminRequestV1 {
            binding_digest,
            command,
            request_digest,
        };
        let response: AdminResponseV1 = exchange(
            &self.policy.administrator_socket,
            self.policy.expected_binding.service_uid,
            SIGNER_FRAME_ADMIN_REQUEST_V1,
            &request,
            SIGNER_FRAME_ADMIN_RESPONSE_V1,
        )?;
        if response.request_digest != request_digest
            || response.response_digest
                != admin_response_digest(&response)
                    .map_err(|()| ExternalSoftwareSignerClientErrorV1::Protocol)?
        {
            return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
        }
        match response.status {
            AdminStatusV1::Ok | AdminStatusV1::Replayed => {}
            AdminStatusV1::Rejected => {
                return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
            }
            AdminStatusV1::Conflict => {
                return Err(ExternalSoftwareSignerClientErrorV1::Equivocation);
            }
            AdminStatusV1::Unavailable => {
                return Err(ExternalSoftwareSignerClientErrorV1::Unavailable);
            }
        }
        response
            .provenance
            .binding
            .validate()
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        verify_provenance(&response.provenance)
            .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        verify_response_attestation(
            &response.provenance.binding,
            response.response_digest,
            &response.response_attestation,
        )
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)?;
        validate_stable_service_identity(
            &self.policy.expected_binding,
            &response.provenance.binding,
        )?;
        if let Some((key_revision, policy_revision, policy_digest, algorithm)) = expected_rotation {
            let binding = &response.provenance.binding;
            if binding.key_revision != key_revision
                || binding.policy_revision != policy_revision
                || binding.policy_digest != policy_digest
                || binding.key_algorithm != algorithm
                || response.provenance.revoked
            {
                return Err(ExternalSoftwareSignerClientErrorV1::BindingMismatch);
            }
        } else if response.provenance.binding != self.policy.expected_binding {
            return Err(ExternalSoftwareSignerClientErrorV1::BindingMismatch);
        }
        Ok(response.provenance)
    }
}
/// Bound external software signer server.
pub struct SoftwareSignerServerV1 {
    service: Arc<SoftwareSignerServiceV1>,
    policy: SoftwareSignerEndpointPolicyV1,
}
impl SoftwareSignerServerV1 {
    /// Bind a service implementation to its exact public endpoint policy.
    ///
    /// # Errors
    ///
    /// Rejects service/binding substitution before creating sockets.
    pub fn try_new(
        service: Arc<SoftwareSignerServiceV1>,
        policy: SoftwareSignerEndpointPolicyV1,
    ) -> Result<Self, SoftwareSignerServerErrorV1> {
        policy.validate_paths()?;
        if service
            .public_binding()
            .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?
            != policy.expected_binding
            || rustix::process::geteuid().as_raw() != policy.expected_binding.service_uid
        {
            return Err(SoftwareSignerServerErrorV1::BindingMismatch);
        }
        Ok(Self { service, policy })
    }
    /// Serve authenticated one-request sessions until SIGINT or SIGTERM.
    ///
    /// # Errors
    ///
    /// Fails closed for endpoint substitution, listener failure, runtime
    /// failure, or incomplete endpoint cleanup.
    pub fn serve(self) -> Result<(), SoftwareSignerServerErrorV1> {
        let (request_listener, request_guard) = bind_endpoint(
            &self.policy.request_socket,
            self.policy.expected_binding.service_uid,
        )?;
        let (administrator_listener, administrator_guard) = bind_endpoint(
            &self.policy.administrator_socket,
            self.policy.expected_binding.service_uid,
        )?;
        request_listener
            .set_nonblocking(true)
            .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
        administrator_listener
            .set_nonblocking(true)
            .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
        let result = runtime.block_on(async move {
            let request_listener = tokio::net::UnixListener::from_std(request_listener)
                .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
            let administrator_listener =
                tokio::net::UnixListener::from_std(administrator_listener)
                    .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
            let permits = Arc::new(tokio::sync::Semaphore::new(MAX_SESSIONS_V1));
            let mut tasks = tokio::task::JoinSet::new();
            let mut terminate = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate(),
            )
            .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
            loop {
                tokio::select! {
                    accepted = request_listener.accept() => {
                        let (stream, _) = accepted.map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
                        admit_session(
                            stream,
                            self.policy.expected_binding.client_uid,
                            false,
                            Arc::clone(&self.service),
                            Arc::clone(&permits),
                            &mut tasks,
                        )?;
                    }
                    accepted = administrator_listener.accept() => {
                        let (stream, _) = accepted.map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
                        admit_session(
                            stream,
                            self.policy.expected_binding.administrator_uid,
                            true,
                            Arc::clone(&self.service),
                            Arc::clone(&permits),
                            &mut tasks,
                        )?;
                    }
                    signal = tokio::signal::ctrl_c() => {
                        signal.map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
                        break;
                    }
                    _ = terminate.recv() => break,
                    completed = tasks.join_next(), if !tasks.is_empty() => {
                        if completed.is_some_and(|result| result.is_err()) {
                            return Err(SoftwareSignerServerErrorV1::Unavailable);
                        }
                    }
                }
            }
            while let Some(completed) = tasks.join_next().await {
                completed.map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
            }
            Ok(())
        });
        let request_cleanup = request_guard.cleanup();
        let administrator_cleanup = administrator_guard.cleanup();
        result?;
        request_cleanup?;
        administrator_cleanup
    }
}
fn admit_session(
    stream: tokio::net::UnixStream,
    expected_uid: u32,
    administrator: bool,
    service: Arc<SoftwareSignerServiceV1>,
    permits: Arc<tokio::sync::Semaphore>,
    tasks: &mut tokio::task::JoinSet<()>,
) -> Result<(), SoftwareSignerServerErrorV1> {
    let credentials = stream
        .peer_cred()
        .map_err(|_| SoftwareSignerServerErrorV1::Authentication)?;
    if !peer_uid_is_authorized(credentials.uid(), expected_uid) {
        return Ok(());
    }
    let Ok(permit) = permits.try_acquire_owned() else {
        return Ok(());
    };
    let stream = stream
        .into_std()
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    stream
        .set_nonblocking(false)
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    tasks.spawn_blocking(move || {
        let _permit = permit;
        let _ = serve_one(stream, administrator, &service);
    });
    Ok(())
}
pub(super) const fn peer_uid_is_authorized(observed_uid: u32, expected_uid: u32) -> bool {
    observed_uid == expected_uid
}
fn serve_one(
    mut stream: UnixStream,
    administrator: bool,
    service: &SoftwareSignerServiceV1,
) -> Result<(), SoftwareSignerServerErrorV1> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT_V1))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT_V1)))
        .map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    let frame = read_frame(&mut stream)?;
    let response = if administrator {
        if frame.kind != SIGNER_FRAME_ADMIN_REQUEST_V1 {
            return Err(SoftwareSignerServerErrorV1::Authentication);
        }
        let request: AdminRequestV1 = decode_body(&frame.body)?;
        let response = service
            .handle_admin_request(&request)
            .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
        encode_frame(SIGNER_FRAME_ADMIN_RESPONSE_V1, &response)?
    } else {
        match frame.kind {
            SIGNER_FRAME_QUALIFY_REQUEST_V1 => {
                let request: QualifyRequestV1 = decode_body(&frame.body)?;
                let binding = service
                    .public_binding()
                    .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
                if request.client_nonce == [0; 32]
                    || request.binding_digest
                        != binding
                            .digest()
                            .map_err(|()| SoftwareSignerServerErrorV1::BindingMismatch)?
                {
                    return Err(SoftwareSignerServerErrorV1::BindingMismatch);
                }
                let mut response = QualifyResponseV1 {
                    client_nonce: request.client_nonce,
                    server_nonce: random_nonzero(),
                    provenance: service
                        .provenance()
                        .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?,
                    response_digest: [0; 32],
                    response_attestation: Vec::new(),
                };
                response.response_digest = qualify_response_digest(&response)
                    .map_err(|()| SoftwareSignerServerErrorV1::Protocol)?;
                response.response_attestation = service
                    .attest_protocol_response(response.response_digest)
                    .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
                encode_frame(SIGNER_FRAME_QUALIFY_RESPONSE_V1, &response)?
            }
            SIGNER_FRAME_SIGN_REQUEST_V1 => {
                let request: SignRequestV1 = decode_body(&frame.body)?;
                let response = service
                    .handle_sign_request(&request)
                    .map_err(|_| SoftwareSignerServerErrorV1::Unavailable)?;
                encode_frame(SIGNER_FRAME_SIGN_RESPONSE_V1, &response)?
            }
            _ => return Err(SoftwareSignerServerErrorV1::Authentication),
        }
    };
    write_length_prefixed(&mut stream, &response)
}
fn exchange<Request, Response>(
    endpoint: &Path,
    service_uid: u32,
    request_kind: u8,
    request: &Request,
    response_kind: u8,
) -> Result<Response, ExternalSoftwareSignerClientErrorV1>
where
    Request: NoritoSerialize,
    Response: NoritoSerialize,
    for<'de> Response: NoritoDeserialize<'de>,
{
    let before = endpoint_identity(endpoint, service_uid)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    let asynchronous = runtime
        .block_on(tokio::net::UnixStream::connect(endpoint))
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    if asynchronous
        .peer_cred()
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?
        .uid()
        != service_uid
        || endpoint_identity(endpoint, service_uid)? != before
    {
        return Err(ExternalSoftwareSignerClientErrorV1::Authentication);
    }
    let mut stream = asynchronous
        .into_std()
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_read_timeout(Some(IO_TIMEOUT_V1)))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT_V1)))
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    let mut frame = encode_frame(request_kind, request)
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Protocol)?;
    let write_result = write_length_prefixed(&mut stream, &frame);
    scrub(&mut frame);
    write_result.map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    let response =
        read_frame(&mut stream).map_err(|_| ExternalSoftwareSignerClientErrorV1::Protocol)?;
    if response.kind != response_kind {
        return Err(ExternalSoftwareSignerClientErrorV1::Protocol);
    }
    decode_body(&response.body).map_err(|_| ExternalSoftwareSignerClientErrorV1::Protocol)
}
fn encode_frame<T: NoritoSerialize>(
    kind: u8,
    value: &T,
) -> Result<Vec<u8>, SoftwareSignerServerErrorV1> {
    let body =
        norito::encode_canonical(value).map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    let frame = SoftwareSignerFrameV1 {
        magic: SIGNER_PROTOCOL_MAGIC_V1,
        version: SIGNER_PROTOCOL_VERSION_V1,
        kind,
        body,
    };
    let encoded =
        norito::encode_canonical(&frame).map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    if encoded.is_empty() || encoded.len() > SIGNER_MAX_FRAME_BYTES_V1 {
        return Err(SoftwareSignerServerErrorV1::Protocol);
    }
    Ok(encoded)
}
fn read_frame(
    stream: &mut UnixStream,
) -> Result<SoftwareSignerFrameV1, SoftwareSignerServerErrorV1> {
    let mut length = [0_u8; 4];
    stream
        .read_exact(&mut length)
        .map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    let length = usize::try_from(u32::from_be_bytes(length))
        .map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    if length == 0 || length > SIGNER_MAX_FRAME_BYTES_V1 {
        return Err(SoftwareSignerServerErrorV1::Protocol);
    }
    let mut encoded = vec![0; length];
    stream
        .read_exact(&mut encoded)
        .map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    let frame: SoftwareSignerFrameV1 =
        norito::decode_canonical(&encoded).map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    scrub(&mut encoded);
    if frame.magic != SIGNER_PROTOCOL_MAGIC_V1
        || frame.version != SIGNER_PROTOCOL_VERSION_V1
        || frame.body.is_empty()
        || frame.body.len() > SIGNER_MAX_FRAME_BYTES_V1
    {
        return Err(SoftwareSignerServerErrorV1::Protocol);
    }
    Ok(frame)
}
fn decode_body<T>(bytes: &[u8]) -> Result<T, SoftwareSignerServerErrorV1>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    norito::decode_canonical(bytes).map_err(|_| SoftwareSignerServerErrorV1::Protocol)
}
fn write_length_prefixed(
    stream: &mut UnixStream,
    bytes: &[u8],
) -> Result<(), SoftwareSignerServerErrorV1> {
    let length = u32::try_from(bytes.len()).map_err(|_| SoftwareSignerServerErrorV1::Protocol)?;
    stream
        .write_all(&length.to_be_bytes())
        .and_then(|()| stream.write_all(bytes))
        .and_then(|()| stream.flush())
        .map_err(|_| SoftwareSignerServerErrorV1::Protocol)
}
fn verify_live_provenance(
    expected: &SoftwareSignerPublicBindingV1,
    provenance: &SoftwareSignerLiveProvenanceV1,
) -> Result<(), ExternalSoftwareSignerClientErrorV1> {
    if provenance.binding != *expected || provenance.revoked {
        return Err(ExternalSoftwareSignerClientErrorV1::StaleOrRevoked);
    }
    verify_provenance(provenance).map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)
}
fn verify_payload_signature(
    binding: &SoftwareSignerPublicBindingV1,
    payload: &[u8],
    signature: &[u8],
) -> Result<(), ExternalSoftwareSignerClientErrorV1> {
    let message = match binding.role {
        SoftwareSignerRoleV1::Promotion => {
            let Some(json) =
                payload.strip_prefix(super::protocol::SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1)
            else {
                return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
            };
            if json.is_empty()
                || json.first() != Some(&b'{')
                || json.last() != Some(&b'}')
                || json.contains(&0)
                || std::str::from_utf8(json).is_err()
            {
                return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
            }
            payload.to_vec()
        }
        role if role.native_role().is_some() => {
            let builder =
                iroha_data_model::transaction::TransactionBuilder::decode_payload(payload)
                    .map_err(|_| ExternalSoftwareSignerClientErrorV1::Rejected)?;
            if builder.payload().authority()
                != &iroha_data_model::account::AccountId::new(binding.public_key.clone())
                || !native_payload_matches_role(binding.role, builder.payload())
            {
                return Err(ExternalSoftwareSignerClientErrorV1::Rejected);
            }
            builder.payload_hash_bytes().to_vec()
        }
        SoftwareSignerRoleV1::TairaAuthority => {
            taira_authority_signing_message(binding, payload)
                .ok_or(ExternalSoftwareSignerClientErrorV1::Rejected)?
        }
        _ => super::typed_payload::validated_typed_signing_message(binding, payload)
            .map_err(|()| ExternalSoftwareSignerClientErrorV1::Rejected)?,
    };
    let signature = Signature::try_from_bytes(signature)
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::Protocol)?;
    signature
        .verify(&binding.public_key, &message)
        .map_err(|_| ExternalSoftwareSignerClientErrorV1::BindingMismatch)
}
fn validate_stable_service_identity(
    before: &SoftwareSignerPublicBindingV1,
    after: &SoftwareSignerPublicBindingV1,
) -> Result<(), ExternalSoftwareSignerClientErrorV1> {
    if before.magic != after.magic
        || before.version != after.version
        || before.backend != after.backend
        || before.handle != after.handle
        || before.service_id != after.service_id
        || before.administrator_id != after.administrator_id
        || before.service_uid != after.service_uid
        || before.client_uid != after.client_uid
        || before.administrator_uid != after.administrator_uid
        || before.role != after.role
        || before.domain != after.domain
        || before.audit_genesis_digest != after.audit_genesis_digest
        || before.max_request_bytes != after.max_request_bytes
    {
        return Err(ExternalSoftwareSignerClientErrorV1::BindingMismatch);
    }
    Ok(())
}
fn random_nonzero() -> [u8; 32] {
    loop {
        let value = rand::random::<[u8; 32]>();
        if value != [0; 32] {
            return value;
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SocketIdentityV1 {
    device: u64,
    inode: u64,
}
fn endpoint_identity(
    path: &Path,
    service_uid: u32,
) -> Result<SocketIdentityV1, ExternalSoftwareSignerClientErrorV1> {
    validate_runtime_directory(
        path.parent()
            .ok_or(ExternalSoftwareSignerClientErrorV1::Unavailable)?,
        service_uid,
    )
    .map_err(|_| ExternalSoftwareSignerClientErrorV1::Authentication)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|_| ExternalSoftwareSignerClientErrorV1::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.uid() != service_uid
        || metadata.mode() & 0o7777 != SOCKET_MODE_V1
        || metadata.nlink() != 1
    {
        return Err(ExternalSoftwareSignerClientErrorV1::Authentication);
    }
    Ok(SocketIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
pub(super) struct BoundEndpointV1 {
    parent: File,
    name: OsString,
    identity: SocketIdentityV1,
    service_uid: u32,
    armed: bool,
}
impl BoundEndpointV1 {
    fn verify(&self) -> Result<(), SoftwareSignerServerErrorV1> {
        let metadata = rustix::fs::statat(
            &self.parent,
            &self.name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
        if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::Socket
            || metadata.st_uid != self.service_uid
            || metadata.st_nlink != 1
            || u32::from(metadata.st_mode & 0o7777) != SOCKET_MODE_V1
            || socket_identity_from_stat(&metadata) != self.identity
        {
            return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
        }
        Ok(())
    }
    pub(super) fn cleanup(mut self) -> Result<(), SoftwareSignerServerErrorV1> {
        self.verify()?;
        let quarantine = OsString::from(format!(
            ".signer-cleanup-{}-{}",
            std::process::id(),
            STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        rustix::fs::renameat_with(
            &self.parent,
            &self.name,
            &self.parent,
            &quarantine,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointCleanup)?;
        let metadata = rustix::fs::statat(
            &self.parent,
            &quarantine,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointCleanup)?;
        if socket_identity_from_stat(&metadata) != self.identity {
            let _ = rustix::fs::renameat_with(
                &self.parent,
                &quarantine,
                &self.parent,
                &self.name,
                rustix::fs::RenameFlags::NOREPLACE,
            );
            return Err(SoftwareSignerServerErrorV1::EndpointCleanup);
        }
        rustix::fs::unlinkat(&self.parent, &quarantine, rustix::fs::AtFlags::empty())
            .map_err(|_| SoftwareSignerServerErrorV1::EndpointCleanup)?;
        self.parent
            .sync_all()
            .map_err(|_| SoftwareSignerServerErrorV1::EndpointCleanup)?;
        self.armed = false;
        Ok(())
    }
}
impl Drop for BoundEndpointV1 {
    fn drop(&mut self) {
        if self.armed && self.verify().is_ok() {
            let _ = rustix::fs::unlinkat(&self.parent, &self.name, rustix::fs::AtFlags::empty());
            let _ = self.parent.sync_all();
        }
    }
}
pub(super) fn bind_endpoint(
    path: &Path,
    service_uid: u32,
) -> Result<(std::os::unix::net::UnixListener, BoundEndpointV1), SoftwareSignerServerErrorV1> {
    validate_absolute_normal_path(path)?;
    let parent_path = path
        .parent()
        .ok_or(SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    validate_runtime_directory(parent_path, service_uid)?;
    if rustix::process::geteuid().as_raw() != service_uid {
        return Err(SoftwareSignerServerErrorV1::Authentication);
    }
    let parent = rustix::fs::open(
        parent_path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    let name = path
        .file_name()
        .ok_or(SoftwareSignerServerErrorV1::EndpointUnavailable)?
        .to_owned();
    if rustix::fs::statat(&parent, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW).is_ok() {
        return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
    }
    let staging = OsString::from(format!(
        ".signer-staging-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let staging_path = parent_path.join(&staging);
    let listener = std::os::unix::net::UnixListener::bind(&staging_path)
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    fs::set_permissions(&staging_path, fs::Permissions::from_mode(SOCKET_MODE_V1))
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    let metadata = fs::symlink_metadata(&staging_path)
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    let identity = SocketIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    if metadata.uid() != service_uid
        || !metadata.file_type().is_socket()
        || metadata.mode() & 0o7777 != SOCKET_MODE_V1
        || metadata.nlink() != 1
    {
        return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
    }
    rustix::fs::renameat_with(
        &parent,
        &staging,
        &parent,
        &name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    parent
        .sync_all()
        .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    let guard = BoundEndpointV1 {
        parent,
        name,
        identity,
        service_uid,
        armed: true,
    };
    guard.verify()?;
    Ok((listener, guard))
}
#[cfg(target_os = "linux")]
const fn socket_identity_from_stat(metadata: &rustix::fs::Stat) -> SocketIdentityV1 {
    SocketIdentityV1 {
        device: metadata.st_dev,
        inode: metadata.st_ino,
    }
}
#[cfg(target_os = "macos")]
fn socket_identity_from_stat(metadata: &rustix::fs::Stat) -> SocketIdentityV1 {
    SocketIdentityV1 {
        device: u64::try_from(metadata.st_dev).unwrap_or(u64::MAX),
        inode: metadata.st_ino,
    }
}
fn validate_runtime_directory(
    path: &Path,
    service_uid: u32,
) -> Result<(), SoftwareSignerServerErrorV1> {
    validate_absolute_normal_path(path)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != service_uid
        || metadata.mode() & 0o7777 != RUNTIME_DIRECTORY_MODE_V1
    {
        return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
    }
    for ancestor in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|_| SoftwareSignerServerErrorV1::EndpointUnavailable)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || (metadata.uid() != 0 && metadata.uid() != service_uid)
            || metadata.mode() & 0o022 != 0
        {
            return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
        }
    }
    Ok(())
}
fn validate_absolute_normal_path(path: &Path) -> Result<(), SoftwareSignerServerErrorV1> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(SoftwareSignerServerErrorV1::EndpointUnavailable);
    }
    Ok(())
}
/// Load exactly 32 wrapping-key bytes from a consumed inherited descriptor.
///
/// The descriptor number, not its content, may be passed through argv. The
/// descriptor is closed after this call and its temporary buffer is scrubbed.
///
/// # Errors
///
/// Rejects standard descriptors, untrusted regular-file metadata, malformed
/// sources, read failures, and values that are not exactly 32 non-zero bytes.
pub fn load_software_signer_wrapping_key_from_fd_v1(
    descriptor: OwnedFd,
) -> Result<SoftwareSignerWrappingKeyV1, SoftwareSignerCredentialErrorV1> {
    if descriptor.as_raw_fd() <= 2 {
        return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
    }
    let file = File::from(descriptor);
    let metadata = file
        .metadata()
        .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    let euid = rustix::process::geteuid().as_raw();
    if metadata.is_dir()
        || (metadata.is_file()
            && ((metadata.uid() != 0 && metadata.uid() != euid)
                || metadata.mode() & 0o077 != 0
                || metadata.nlink() != 1))
    {
        return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
    }
    let mut bytes = Vec::with_capacity(33);
    file.take(33)
        .read_to_end(&mut bytes)
        .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    if bytes.len() != 32 {
        scrub(&mut bytes);
        return Err(SoftwareSignerCredentialErrorV1::InvalidLength);
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    scrub(&mut bytes);
    let result = SoftwareSignerWrappingKeyV1::try_from_bytes(key)
        .map_err(|_| SoftwareSignerCredentialErrorV1::InvalidSource);
    scrub(&mut key);
    result
}
/// Load exactly 32 wrapping-key bytes from a systemd credential file.
///
/// The path must be absolute, symlink-free, single-link, owned by root or the
/// service UID, and inaccessible to group/other users.
///
/// # Errors
///
/// Rejects insecure paths or metadata, path substitution, read failures, and
/// values that are not exactly 32 non-zero bytes.
pub fn load_software_signer_wrapping_key_from_credential_v1(
    path: &Path,
) -> Result<SoftwareSignerWrappingKeyV1, SoftwareSignerCredentialErrorV1> {
    let expected_identity = validate_credential_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(
        rustix::fs::OFlags::NOFOLLOW
            .bits()
            .try_into()
            .map_err(|_| SoftwareSignerCredentialErrorV1::InvalidSource)?,
    );
    let descriptor = options
        .open(path)
        .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    let opened = descriptor
        .metadata()
        .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    if (opened.dev(), opened.ino()) != expected_identity {
        return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
    }
    let mut bytes = Vec::with_capacity(33);
    descriptor
        .take(33)
        .read_to_end(&mut bytes)
        .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    if bytes.len() != 32 {
        scrub(&mut bytes);
        return Err(SoftwareSignerCredentialErrorV1::InvalidLength);
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(&bytes);
    scrub(&mut bytes);
    let result = SoftwareSignerWrappingKeyV1::try_from_bytes(key)
        .map_err(|_| SoftwareSignerCredentialErrorV1::InvalidSource);
    scrub(&mut key);
    result
}
fn validate_credential_path(path: &Path) -> Result<(u64, u64), SoftwareSignerCredentialErrorV1> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
    }
    let euid = rustix::process::geteuid().as_raw();
    let metadata =
        fs::symlink_metadata(path).map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || (metadata.uid() != 0 && metadata.uid() != euid)
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != 1
    {
        return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
    }
    for ancestor in path
        .parent()
        .ok_or(SoftwareSignerCredentialErrorV1::InvalidSource)?
        .ancestors()
    {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|_| SoftwareSignerCredentialErrorV1::Unavailable)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || (metadata.uid() != 0 && metadata.uid() != euid)
            || metadata.mode() & 0o022 != 0
        {
            return Err(SoftwareSignerCredentialErrorV1::InvalidSource);
        }
    }
    Ok((metadata.dev(), metadata.ino()))
}
/// Payload-free client failure classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalSoftwareSignerClientErrorV1 {
    /// Socket or signer is unavailable.
    Unavailable,
    /// Filesystem or peer credentials do not match the pinned service.
    Authentication,
    /// Canonical framing or response correlation failed.
    Protocol,
    /// Public service/key/policy identity was substituted.
    BindingMismatch,
    /// Request bytes are outside the role or public bounds.
    Rejected,
    /// An operation identifier was reused for different bytes.
    Equivocation,
    /// Key or policy is stale or revoked.
    StaleOrRevoked,
}
/// Payload-free server startup or transport failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerServerErrorV1 {
    /// Bound service identity differs from the reviewed public binding.
    BindingMismatch,
    /// Runtime directory or socket could not be securely created or verified.
    EndpointUnavailable,
    /// Peer UID did not match the endpoint role.
    Authentication,
    /// Canonical framing or request correlation failed.
    Protocol,
    /// Runtime, cryptographic service, or task admission was unavailable.
    Unavailable,
    /// Exact endpoint identity could not be safely removed.
    EndpointCleanup,
}
/// Payload-free runtime wrapping-key source failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerCredentialErrorV1 {
    /// Descriptor or credential path/metadata is not trusted.
    InvalidSource,
    /// Credential is not exactly 32 bytes.
    InvalidLength,
    /// Credential could not be read.
    Unavailable,
}
