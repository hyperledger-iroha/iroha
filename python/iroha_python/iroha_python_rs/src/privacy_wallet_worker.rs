//! Wallet-local privacy witness custody.
//!
//! This module deliberately exposes no operation that returns witness bytes.
//! A caller may import a credential file, inspect or cancel its opaque handle,
//! or consume it exactly once inside a native Rust closure. The pipe protocol
//! mirrors that boundary: proof execution remains unavailable until a native
//! prover callback is attached.

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rand_core_06::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_CREDENTIAL_BYTES: u64 = 8_388_608;
pub const MAX_CANONICAL_PUBLIC_INTENT_BYTES: usize = 524_288;
pub const MAX_HANDLES: usize = 1_024;
pub const MIN_TTL_MILLIS: u64 = 1_000;
pub const MAX_TTL_MILLIS: u64 = 15 * 60 * 1_000;

const MAGIC: &[u8; 4] = b"IPWW";
const AUTH_TAG_BYTES: usize = 32;
const HANDLE_BYTES: usize = 32;
const DIGEST_BYTES: usize = 32;
const NONCE_BYTES: usize = 32;
const MAX_CHAIN_ID_BYTES: usize = 512;
const MAX_SIGNER_BYTES: usize = 512;
const MAX_PROTOCOL_BYTES: usize = 96;
const MAX_PATH_BYTES: usize = 4_096;
const PUBLIC_INTENT_DIGEST_DOMAIN: &[u8] = b"iroha-privacy-wallet-binding-v1\0";
const COMPILED_PROFILE_DIGEST_DOMAIN: &[u8] =
    b"iroha-privacy-compiled-profile-binding-v1\0";

const SUPPORTED_PROTOCOLS: &[&str] = &[
    "zk-ace-pq-authorization-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "verange-transparent-range-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-ams-v1",
    "iroha-bootle-lantern-anoncred-v1",
];
const PUBLIC_INTENT_BASE_FIELDS: &[&str] = &[
    "algorithm_id",
    "protocol_id",
    "selected_features",
    "selected_criteria",
    "signer_wallet_id",
];
const PRIVACY_FEATURE_FIELDS: &[&str] = &[
    "hide_amount",
    "hide_sender",
    "hide_receiver",
    "hide_asset_type",
    "post_quantum",
];

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct WitnessHandle([u8; HANDLE_BYTES]);

impl WitnessHandle {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; HANDLE_BYTES]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; HANDLE_BYTES] {
        &self.0
    }

    #[must_use]
    pub fn encode_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn decode_hex(value: &str) -> Result<Self, WorkerError> {
        if value.len() != HANDLE_BYTES * 2
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(WorkerError::InvalidHandle);
        }
        let decoded = hex::decode(value).map_err(|_| WorkerError::InvalidHandle)?;
        let bytes = decoded
            .try_into()
            .map_err(|_| WorkerError::InvalidHandle)?;
        Ok(Self(bytes))
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct WitnessBinding {
    pub chain_id: String,
    pub genesis_digest: [u8; DIGEST_BYTES],
    pub signer: String,
    pub protocol: String,
    pub profile_digest: [u8; DIGEST_BYTES],
    pub public_intent_digest: [u8; DIGEST_BYTES],
    pub nonce: [u8; NONCE_BYTES],
    /// Digest of a separately verified signed authority, never the IPC key.
    pub signed_release_authority_digest: [u8; DIGEST_BYTES],
}

impl WitnessBinding {
    pub fn validate(&self) -> Result<(), WorkerError> {
        validate_text("chain id", &self.chain_id, MAX_CHAIN_ID_BYTES)?;
        validate_text("signer", &self.signer, MAX_SIGNER_BYTES)?;
        validate_text("protocol", &self.protocol, MAX_PROTOCOL_BYTES)?;
        if !SUPPORTED_PROTOCOLS.contains(&self.protocol.as_str()) {
            return Err(WorkerError::UnsupportedProtocol);
        }
        if self.genesis_digest == [0; DIGEST_BYTES]
            || self.profile_digest == [0; DIGEST_BYTES]
            || self.public_intent_digest == [0; DIGEST_BYTES]
            || self.nonce == [0; NONCE_BYTES]
            || self.signed_release_authority_digest == [0; DIGEST_BYTES]
        {
            return Err(WorkerError::InvalidBinding(
                "binding digests and nonce must be non-zero",
            ));
        }
        Ok(())
    }

    #[must_use]
    fn digest(&self) -> [u8; DIGEST_BYTES] {
        let mut encoded = Vec::with_capacity(512);
        put_text(&mut encoded, &self.chain_id);
        encoded.extend_from_slice(&self.genesis_digest);
        put_text(&mut encoded, &self.signer);
        put_text(&mut encoded, &self.protocol);
        encoded.extend_from_slice(&self.profile_digest);
        encoded.extend_from_slice(&self.public_intent_digest);
        encoded.extend_from_slice(&self.nonce);
        encoded.extend_from_slice(&self.signed_release_authority_digest);
        sha256(&encoded)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CompiledProfileBinding {
    pub parameter_id: [u8; DIGEST_BYTES],
    pub parameter_digest: [u8; DIGEST_BYTES],
    pub verifier_digest: [u8; DIGEST_BYTES],
    pub statement_schema_digest: [u8; DIGEST_BYTES],
    pub engine_manifest_digest: [u8; DIGEST_BYTES],
}

impl CompiledProfileBinding {
    fn validate(&self) -> Result<(), WorkerError> {
        if [
            self.parameter_id,
            self.parameter_digest,
            self.verifier_digest,
            self.statement_schema_digest,
            self.engine_manifest_digest,
        ]
        .contains(&[0; DIGEST_BYTES])
        {
            return Err(WorkerError::InvalidBinding(
                "compiled profile digests must be non-zero",
            ));
        }
        Ok(())
    }
}

pub fn compiled_profile_digest(
    protocol: &str,
    binding: &CompiledProfileBinding,
) -> Result<[u8; DIGEST_BYTES], WorkerError> {
    validate_text("compiled profile protocol", protocol, MAX_PROTOCOL_BYTES)?;
    if !SUPPORTED_PROTOCOLS.contains(&protocol) {
        return Err(WorkerError::UnsupportedProtocol);
    }
    binding.validate()?;
    let mut digest = Sha256::new();
    digest.update(COMPILED_PROFILE_DIGEST_DOMAIN);
    let protocol_length =
        u16::try_from(protocol.len()).map_err(|_| WorkerError::InvalidBinding(
            "compiled profile protocol is too long",
        ))?;
    digest.update(protocol_length.to_be_bytes());
    digest.update(protocol.as_bytes());
    digest.update(binding.parameter_id);
    digest.update(binding.parameter_digest);
    digest.update(binding.verifier_digest);
    digest.update(binding.statement_schema_digest);
    digest.update(binding.engine_manifest_digest);
    Ok(digest.finalize().into())
}

#[must_use]
pub fn canonical_public_intent_digest(
    canonical_public_intent: &[u8],
) -> [u8; DIGEST_BYTES] {
    let mut digest = Sha256::new();
    digest.update(PUBLIC_INTENT_DIGEST_DOMAIN);
    digest.update(canonical_public_intent);
    digest.finalize().into()
}

pub struct ImportRequest {
    pub credential_path: PathBuf,
    pub binding: WitnessBinding,
    pub ttl_millis: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct WitnessLease {
    pub handle: WitnessHandle,
    pub expires_at_millis: u64,
}

struct StoredWitness {
    binding: WitnessBinding,
    binding_digest: [u8; DIGEST_BYTES],
    expires_at_millis: u64,
    material: Zeroizing<Vec<u8>>,
}

pub struct WitnessVault {
    entries: HashMap<WitnessHandle, StoredWitness>,
    maximum_handles: usize,
}

impl Default for WitnessVault {
    fn default() -> Self {
        Self::new(MAX_HANDLES)
    }
}

impl Drop for WitnessVault {
    fn drop(&mut self) {
        for witness in self.entries.values_mut() {
            witness.material.zeroize();
        }
        self.entries.clear();
    }
}

impl WitnessVault {
    #[must_use]
    pub fn new(maximum_handles: usize) -> Self {
        Self {
            entries: HashMap::new(),
            maximum_handles: maximum_handles.clamp(1, MAX_HANDLES),
        }
    }

    pub fn import_credential(
        &mut self,
        request: ImportRequest,
    ) -> Result<WitnessLease, WorkerError> {
        let now_millis = unix_time_millis()?;
        self.import_credential_at(request, now_millis)
    }

    fn import_credential_at(
        &mut self,
        request: ImportRequest,
        now_millis: u64,
    ) -> Result<WitnessLease, WorkerError> {
        request.binding.validate()?;
        validate_clock_and_ttl(now_millis, request.ttl_millis)?;
        self.purge_expired_at(now_millis);
        if self.entries.len() >= self.maximum_handles {
            return Err(WorkerError::CapacityExceeded);
        }
        let material = read_credential_file(&request.credential_path)?;
        let expires_at_millis = now_millis
            .checked_add(request.ttl_millis)
            .ok_or(WorkerError::InvalidTtl)?;
        let handle = self.unique_handle()?;
        let binding_digest = request.binding.digest();
        self.entries.insert(
            handle,
            StoredWitness {
                binding: request.binding,
                binding_digest,
                expires_at_millis,
                material,
            },
        );
        Ok(WitnessLease {
            handle,
            expires_at_millis,
        })
    }

    pub fn inspect(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
    ) -> Result<WitnessLease, WorkerError> {
        let now_millis = unix_time_millis()?;
        self.inspect_at(handle, expected_binding, now_millis)
    }

    fn inspect_at(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
        now_millis: u64,
    ) -> Result<WitnessLease, WorkerError> {
        self.purge_expired_at(now_millis);
        let stored = self.entries.get(&handle).ok_or(WorkerError::UnknownHandle)?;
        validate_expected_binding(stored, expected_binding)?;
        Ok(WitnessLease {
            handle,
            expires_at_millis: stored.expires_at_millis,
        })
    }

    pub fn cancel(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
    ) -> Result<(), WorkerError> {
        let now_millis = unix_time_millis()?;
        self.cancel_at(handle, expected_binding, now_millis)
    }

    fn cancel_at(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
        now_millis: u64,
    ) -> Result<(), WorkerError> {
        self.purge_expired_at(now_millis);
        {
            let stored = self.entries.get(&handle).ok_or(WorkerError::UnknownHandle)?;
            validate_expected_binding(stored, expected_binding)?;
        }
        let mut removed = self
            .entries
            .remove(&handle)
            .ok_or(WorkerError::UnknownHandle)?;
        removed.material.zeroize();
        Ok(())
    }

    /// Atomically removes a witness and lends it to one native operation.
    ///
    /// Removal happens before `operation` runs. Success, failure, and panic
    /// unwinding all drop a zeroizing buffer, so retrying a handle is
    /// impossible and callback errors never reinsert secret material.
    pub fn consume_with<T, E>(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
        operation: impl FnOnce(&mut [u8]) -> Result<T, E>,
    ) -> Result<T, ConsumeError<E>> {
        let now_millis = unix_time_millis().map_err(ConsumeError::Custody)?;
        self.consume_with_at(handle, expected_binding, now_millis, operation)
    }

    fn consume_with_at<T, E>(
        &mut self,
        handle: WitnessHandle,
        expected_binding: &WitnessBinding,
        now_millis: u64,
        operation: impl FnOnce(&mut [u8]) -> Result<T, E>,
    ) -> Result<T, ConsumeError<E>> {
        self.purge_expired_at(now_millis);
        {
            let stored = self
                .entries
                .get(&handle)
                .ok_or(ConsumeError::Custody(WorkerError::UnknownHandle))?;
            validate_expected_binding(stored, expected_binding)
                .map_err(ConsumeError::Custody)?;
        }
        let mut stored = self
            .entries
            .remove(&handle)
            .ok_or(ConsumeError::Custody(WorkerError::UnknownHandle))?;
        let result = operation(stored.material.as_mut_slice()).map_err(ConsumeError::Operation);
        stored.material.zeroize();
        result
    }

    pub fn purge_expired(&mut self) -> Result<usize, WorkerError> {
        Ok(self.purge_expired_at(unix_time_millis()?))
    }

    fn purge_expired_at(&mut self, now_millis: u64) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, witness| witness.expires_at_millis > now_millis);
        before - self.entries.len()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn unique_handle(&self) -> Result<WitnessHandle, WorkerError> {
        for _ in 0..32 {
            let mut bytes = [0_u8; HANDLE_BYTES];
            OsRng
                .try_fill_bytes(&mut bytes)
                .map_err(|_| WorkerError::EntropyUnavailable)?;
            if bytes != [0; HANDLE_BYTES] {
                let candidate = WitnessHandle(bytes);
                if !self.entries.contains_key(&candidate) {
                    return Ok(candidate);
                }
            }
        }
        Err(WorkerError::EntropyUnavailable)
    }
}

#[derive(Debug)]
pub enum ConsumeError<E> {
    Custody(WorkerError),
    Operation(E),
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum CommandKind {
    Ping = 1,
    Import = 2,
    Inspect = 3,
    Cancel = 4,
    Execute = 5,
}

impl TryFrom<u8> for CommandKind {
    type Error = WorkerError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Ping),
            2 => Ok(Self::Import),
            3 => Ok(Self::Inspect),
            4 => Ok(Self::Cancel),
            5 => Ok(Self::Execute),
            _ => Err(WorkerError::UnknownCommand),
        }
    }
}

pub struct AuthenticatedFrame {
    pub kind: CommandKind,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

pub fn encode_frame(
    frame: &AuthenticatedFrame,
    auth_key: &[u8; DIGEST_BYTES],
) -> Result<Vec<u8>, WorkerError> {
    if frame.sequence == 0 || frame.payload.len() > MAX_FRAME_BYTES {
        return Err(WorkerError::InvalidFrame);
    }
    let payload_len =
        u32::try_from(frame.payload.len()).map_err(|_| WorkerError::FrameTooLarge)?;
    let mut authenticated = Vec::with_capacity(18 + frame.payload.len());
    authenticated.extend_from_slice(MAGIC);
    authenticated.push(PROTOCOL_VERSION);
    authenticated.push(frame.kind as u8);
    authenticated.extend_from_slice(&frame.sequence.to_be_bytes());
    authenticated.extend_from_slice(&payload_len.to_be_bytes());
    authenticated.extend_from_slice(&frame.payload);
    let tag = hmac_sha256(auth_key, &authenticated);
    authenticated.extend_from_slice(&tag);
    if authenticated.len() > MAX_FRAME_BYTES {
        return Err(WorkerError::FrameTooLarge);
    }
    let frame_len =
        u32::try_from(authenticated.len()).map_err(|_| WorkerError::FrameTooLarge)?;
    let mut encoded = Vec::with_capacity(4 + authenticated.len());
    encoded.extend_from_slice(&frame_len.to_be_bytes());
    encoded.extend_from_slice(&authenticated);
    Ok(encoded)
}

pub fn decode_frame(
    encoded: &[u8],
    auth_key: &[u8; DIGEST_BYTES],
) -> Result<AuthenticatedFrame, WorkerError> {
    if encoded.len() < 4 {
        return Err(WorkerError::InvalidFrame);
    }
    let declared = u32::from_be_bytes(
        encoded[0..4]
            .try_into()
            .map_err(|_| WorkerError::InvalidFrame)?,
    ) as usize;
    if declared > MAX_FRAME_BYTES {
        return Err(WorkerError::FrameTooLarge);
    }
    if declared != encoded.len() - 4 || declared < 18 + AUTH_TAG_BYTES {
        return Err(WorkerError::InvalidFrame);
    }
    let authenticated_end = encoded.len() - AUTH_TAG_BYTES;
    let authenticated = &encoded[4..authenticated_end];
    let actual_tag = &encoded[authenticated_end..];
    let expected_tag = hmac_sha256(auth_key, authenticated);
    if !constant_time_eq(actual_tag, &expected_tag) {
        return Err(WorkerError::AuthenticationFailed);
    }
    if &authenticated[0..4] != MAGIC || authenticated[4] != PROTOCOL_VERSION {
        return Err(WorkerError::InvalidFrame);
    }
    let kind = CommandKind::try_from(authenticated[5])?;
    let sequence = u64::from_be_bytes(
        authenticated[6..14]
            .try_into()
            .map_err(|_| WorkerError::InvalidFrame)?,
    );
    if sequence == 0 {
        return Err(WorkerError::InvalidFrame);
    }
    let payload_len = u32::from_be_bytes(
        authenticated[14..18]
            .try_into()
            .map_err(|_| WorkerError::InvalidFrame)?,
    ) as usize;
    if payload_len != authenticated.len() - 18 {
        return Err(WorkerError::InvalidFrame);
    }
    Ok(AuthenticatedFrame {
        kind,
        sequence,
        payload: authenticated[18..].to_vec(),
    })
}

pub fn read_frame(
    reader: &mut impl Read,
    auth_key: &[u8; DIGEST_BYTES],
) -> Result<Option<AuthenticatedFrame>, WorkerError> {
    let mut length = [0_u8; 4];
    match reader.read(&mut length[0..1]) {
        Ok(0) => return Ok(None),
        Ok(1) => {}
        Ok(_) => unreachable!("one-byte read returned more than one byte"),
        Err(error) => return Err(WorkerError::Io(error.kind())),
    }
    reader
        .read_exact(&mut length[1..])
        .map_err(|error| WorkerError::Io(error.kind()))?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(WorkerError::FrameTooLarge);
    }
    if length < 18 + AUTH_TAG_BYTES {
        return Err(WorkerError::InvalidFrame);
    }
    let mut encoded = vec![0_u8; length + 4];
    encoded[0..4].copy_from_slice(&(length as u32).to_be_bytes());
    reader
        .read_exact(&mut encoded[4..])
        .map_err(|error| WorkerError::Io(error.kind()))?;
    decode_frame(&encoded, auth_key).map(Some)
}

pub fn write_frame(
    writer: &mut impl Write,
    frame: &AuthenticatedFrame,
    auth_key: &[u8; DIGEST_BYTES],
) -> Result<(), WorkerError> {
    let encoded = encode_frame(frame, auth_key)?;
    writer
        .write_all(&encoded)
        .map_err(|error| WorkerError::Io(error.kind()))?;
    writer
        .flush()
        .map_err(|error| WorkerError::Io(error.kind()))
}

pub fn run_pipe_session(
    reader: &mut impl Read,
    writer: &mut impl Write,
    auth_key: Zeroizing<[u8; DIGEST_BYTES]>,
) -> Result<(), WorkerError> {
    if auth_key.iter().all(|byte| *byte == 0) {
        return Err(WorkerError::AuthenticationFailed);
    }
    let mut vault = WitnessVault::default();
    let mut expected_sequence = 1_u64;
    while let Some(frame) = read_frame(reader, &auth_key)? {
        if frame.sequence != expected_sequence {
            return Err(WorkerError::ReplayOrOutOfOrder);
        }
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(WorkerError::ReplayOrOutOfOrder)?;
        let response = dispatch(&mut vault, frame.kind, &frame.payload);
        write_frame(
            writer,
            &AuthenticatedFrame {
                kind: frame.kind,
                sequence: frame.sequence,
                payload: encode_response(response),
            },
            &auth_key,
        )?;
    }
    Ok(())
}

enum CommandResponse {
    Pong,
    Lease(WitnessLease),
    Cancelled,
    Error(WorkerError),
}

fn dispatch(vault: &mut WitnessVault, kind: CommandKind, payload: &[u8]) -> CommandResponse {
    let result = match kind {
        CommandKind::Ping => {
            if payload.is_empty() {
                return CommandResponse::Pong;
            }
            Err(WorkerError::InvalidPayload)
        }
        CommandKind::Import => decode_import(payload)
            .and_then(|request| vault.import_credential(request))
            .map(CommandResponse::Lease),
        CommandKind::Inspect => decode_handle_request(payload).and_then(
            |(handle, binding)| {
                vault
                    .inspect(handle, &binding)
                    .map(CommandResponse::Lease)
            },
        ),
        CommandKind::Cancel => decode_handle_request(payload).and_then(
            |(handle, binding)| {
                vault
                    .cancel(handle, &binding)
                    .map(|()| CommandResponse::Cancelled)
            },
        ),
        // Deliberately do not consume the handle here. The only valid future
        // implementation is a direct native prover callback passed to
        // `consume_with`; returning witness bytes would violate the boundary.
        CommandKind::Execute => decode_execute_request(payload).and_then(
            |(handle, binding, canonical_public_intent)| {
                validate_canonical_public_intent(&canonical_public_intent, &binding)?;
                vault.inspect(handle, &binding)?;
                Err(WorkerError::NativeProverUnavailable)
            },
        ),
    };
    result.unwrap_or_else(CommandResponse::Error)
}

pub fn encode_import_payload(request: &ImportRequest) -> Result<Vec<u8>, WorkerError> {
    request.binding.validate()?;
    let path = request
        .credential_path
        .to_str()
        .ok_or(WorkerError::InvalidCredentialPath)?;
    validate_text("credential path", path, MAX_PATH_BYTES)?;
    let mut payload = Vec::with_capacity(768);
    put_text(&mut payload, path);
    encode_binding(&mut payload, &request.binding);
    payload.extend_from_slice(&request.ttl_millis.to_be_bytes());
    Ok(payload)
}

pub fn encode_handle_payload(
    handle: WitnessHandle,
    binding: &WitnessBinding,
) -> Result<Vec<u8>, WorkerError> {
    binding.validate()?;
    let mut payload = Vec::with_capacity(512);
    payload.extend_from_slice(handle.as_bytes());
    encode_binding(&mut payload, binding);
    Ok(payload)
}

pub fn encode_execute_payload(
    handle: WitnessHandle,
    binding: &WitnessBinding,
    canonical_public_intent: &[u8],
) -> Result<Vec<u8>, WorkerError> {
    binding.validate()?;
    validate_canonical_public_intent(canonical_public_intent, binding)?;
    let public_intent_length = u32::try_from(canonical_public_intent.len())
        .map_err(|_| WorkerError::InvalidPublicIntent)?;
    let mut payload = Vec::with_capacity(516 + canonical_public_intent.len());
    payload.extend_from_slice(handle.as_bytes());
    encode_binding(&mut payload, binding);
    payload.extend_from_slice(&public_intent_length.to_be_bytes());
    payload.extend_from_slice(canonical_public_intent);
    if payload.len() > MAX_FRAME_BYTES {
        return Err(WorkerError::FrameTooLarge);
    }
    Ok(payload)
}

fn decode_import(payload: &[u8]) -> Result<ImportRequest, WorkerError> {
    let mut cursor = Cursor::new(payload);
    let path = cursor.text(MAX_PATH_BYTES)?;
    let binding = decode_binding(&mut cursor)?;
    let ttl_millis = cursor.u64()?;
    cursor.finish()?;
    let credential_path = PathBuf::from(path);
    Ok(ImportRequest {
        credential_path,
        binding,
        ttl_millis,
    })
}

fn decode_handle_request(
    payload: &[u8],
) -> Result<(WitnessHandle, WitnessBinding), WorkerError> {
    let mut cursor = Cursor::new(payload);
    let handle = WitnessHandle(cursor.array()?);
    let binding = decode_binding(&mut cursor)?;
    cursor.finish()?;
    Ok((handle, binding))
}

fn decode_execute_request(
    payload: &[u8],
) -> Result<(WitnessHandle, WitnessBinding, Vec<u8>), WorkerError> {
    let mut cursor = Cursor::new(payload);
    let handle = WitnessHandle(cursor.array()?);
    let binding = decode_binding(&mut cursor)?;
    let public_intent_length = usize::try_from(cursor.u32()?)
        .map_err(|_| WorkerError::InvalidPublicIntent)?;
    if public_intent_length == 0
        || public_intent_length > MAX_CANONICAL_PUBLIC_INTENT_BYTES
    {
        return Err(WorkerError::InvalidPublicIntent);
    }
    let canonical_public_intent = cursor.take(public_intent_length)?.to_vec();
    cursor.finish()?;
    Ok((handle, binding, canonical_public_intent))
}

fn encode_binding(output: &mut Vec<u8>, binding: &WitnessBinding) {
    put_text(output, &binding.chain_id);
    output.extend_from_slice(&binding.genesis_digest);
    put_text(output, &binding.signer);
    put_text(output, &binding.protocol);
    output.extend_from_slice(&binding.profile_digest);
    output.extend_from_slice(&binding.public_intent_digest);
    output.extend_from_slice(&binding.nonce);
    output.extend_from_slice(&binding.signed_release_authority_digest);
}

fn decode_binding(cursor: &mut Cursor<'_>) -> Result<WitnessBinding, WorkerError> {
    let binding = WitnessBinding {
        chain_id: cursor.text(MAX_CHAIN_ID_BYTES)?,
        genesis_digest: cursor.array()?,
        signer: cursor.text(MAX_SIGNER_BYTES)?,
        protocol: cursor.text(MAX_PROTOCOL_BYTES)?,
        profile_digest: cursor.array()?,
        public_intent_digest: cursor.array()?,
        nonce: cursor.array()?,
        signed_release_authority_digest: cursor.array()?,
    };
    binding.validate()?;
    Ok(binding)
}

fn encode_response(response: CommandResponse) -> Vec<u8> {
    let mut output = Vec::with_capacity(128);
    match response {
        CommandResponse::Pong => output.push(0),
        CommandResponse::Lease(lease) => {
            output.push(1);
            output.extend_from_slice(lease.handle.as_bytes());
            output.extend_from_slice(&lease.expires_at_millis.to_be_bytes());
        }
        CommandResponse::Cancelled => output.push(2),
        CommandResponse::Error(error) => {
            output.push(255);
            output.extend_from_slice(&error.code().to_be_bytes());
            let message = error.message();
            put_text(&mut output, message);
        }
    }
    output
}

struct Cursor<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(source: &'a [u8]) -> Self {
        Self { source, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], WorkerError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(WorkerError::InvalidPayload)?;
        let value = self
            .source
            .get(self.offset..end)
            .ok_or(WorkerError::InvalidPayload)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WorkerError> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerError::InvalidPayload)
    }

    fn u16(&mut self) -> Result<u16, WorkerError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn text(&mut self, maximum_bytes: usize) -> Result<String, WorkerError> {
        let length = usize::from(self.u16()?);
        if length == 0 || length > maximum_bytes {
            return Err(WorkerError::InvalidPayload);
        }
        let value = std::str::from_utf8(self.take(length)?)
            .map_err(|_| WorkerError::InvalidPayload)?;
        validate_text("payload text", value, maximum_bytes)?;
        Ok(value.to_owned())
    }

    fn finish(self) -> Result<(), WorkerError> {
        if self.offset == self.source.len() {
            Ok(())
        } else {
            Err(WorkerError::InvalidPayload)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerError {
    AuthenticationFailed,
    CapacityExceeded,
    ClockUnavailable,
    CredentialChangedDuringImport,
    CredentialFileEmpty,
    CredentialFileInsecure,
    CredentialFileTooLarge,
    EntropyUnavailable,
    Expired,
    FrameTooLarge,
    InvalidBinding(&'static str),
    InvalidCredentialPath,
    InvalidFrame,
    InvalidHandle,
    InvalidPayload,
    InvalidPublicIntent,
    InvalidTtl,
    Io(io::ErrorKind),
    NativeProverUnavailable,
    PublicIntentDigestMismatch,
    ReplayOrOutOfOrder,
    UnknownCommand,
    UnknownHandle,
    UnsupportedProtocol,
    WrongBinding,
}

impl WorkerError {
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::AuthenticationFailed => 1,
            Self::CapacityExceeded => 2,
            Self::ClockUnavailable => 25,
            Self::CredentialChangedDuringImport => 3,
            Self::CredentialFileEmpty => 4,
            Self::CredentialFileInsecure => 5,
            Self::CredentialFileTooLarge => 6,
            Self::EntropyUnavailable => 7,
            Self::Expired => 8,
            Self::FrameTooLarge => 9,
            Self::InvalidBinding(_) => 10,
            Self::InvalidCredentialPath => 11,
            Self::InvalidFrame => 12,
            Self::InvalidHandle => 13,
            Self::InvalidPayload => 14,
            Self::InvalidPublicIntent => 23,
            Self::InvalidTtl => 15,
            Self::Io(_) => 16,
            Self::NativeProverUnavailable => 17,
            Self::PublicIntentDigestMismatch => 24,
            Self::ReplayOrOutOfOrder => 18,
            Self::UnknownCommand => 19,
            Self::UnknownHandle => 20,
            Self::UnsupportedProtocol => 21,
            Self::WrongBinding => 22,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::AuthenticationFailed => "IPC authentication failed",
            Self::CapacityExceeded => "witness handle capacity exceeded",
            Self::ClockUnavailable => "native system clock is unavailable",
            Self::CredentialChangedDuringImport => "credential file changed during import",
            Self::CredentialFileEmpty => "credential file is empty",
            Self::CredentialFileInsecure => "credential file permissions are not owner-only",
            Self::CredentialFileTooLarge => "credential file exceeds the bounded size",
            Self::EntropyUnavailable => "secure operating-system entropy is unavailable",
            Self::Expired => "witness handle expired",
            Self::FrameTooLarge => "IPC frame exceeds the bounded size",
            Self::InvalidBinding(message) => message,
            Self::InvalidCredentialPath => "credential path is invalid",
            Self::InvalidFrame => "IPC frame is non-canonical",
            Self::InvalidHandle => "witness handle is invalid",
            Self::InvalidPayload => "IPC payload is non-canonical",
            Self::InvalidPublicIntent => "public intent bytes are not canonical typed JSON",
            Self::InvalidTtl => "witness handle TTL is invalid",
            Self::Io(_) => "local IPC or credential file I/O failed",
            Self::NativeProverUnavailable => {
                "native worker prover attachment is unavailable; witness was not released"
            }
            Self::PublicIntentDigestMismatch => {
                "public intent bytes do not match the witness binding digest"
            }
            Self::ReplayOrOutOfOrder => "IPC sequence replay or reordering detected",
            Self::UnknownCommand => "IPC command is unknown",
            Self::UnknownHandle => "witness handle is unknown",
            Self::UnsupportedProtocol => "privacy protocol is unsupported",
            Self::WrongBinding => "witness handle binding does not match the public intent",
        }
    }
}

fn unix_time_millis() -> Result<u64, WorkerError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WorkerError::ClockUnavailable)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| WorkerError::ClockUnavailable)
}

fn validate_clock_and_ttl(now_millis: u64, ttl_millis: u64) -> Result<(), WorkerError> {
    if now_millis == 0
        || !(MIN_TTL_MILLIS..=MAX_TTL_MILLIS).contains(&ttl_millis)
        || now_millis.checked_add(ttl_millis).is_none()
    {
        return Err(WorkerError::InvalidTtl);
    }
    Ok(())
}

fn validate_expected_binding(
    stored: &StoredWitness,
    expected: &WitnessBinding,
) -> Result<(), WorkerError> {
    expected.validate()?;
    let expected_digest = expected.digest();
    if !constant_time_eq(&stored.binding_digest, &expected_digest)
        || stored.binding != *expected
    {
        return Err(WorkerError::WrongBinding);
    }
    Ok(())
}

fn public_intent_protocol_fields(protocol: &str) -> Result<&'static [&'static str], WorkerError> {
    match protocol {
        "zk-ace-pq-authorization-v0" => Ok(&[
            "zk_ace_policy_archive_hex",
            "zk_ace_source_account_id",
            "zk_ace_destination_account_id",
            "zk_ace_amount_decimal",
            "zk_ace_identity_root_hex",
        ]),
        "iroha-jindo-polynomial-commitment-v0" => Ok(&[
            "jindo_evaluation_point_hex",
        ]),
        "verange-transparent-range-v1" => Ok(&[
            "verange_asset_definition_id",
            "verange_policy_id_hex",
            "verange_bit_length",
        ]),
        "vega-existing-credential-zk-v0" => Ok(&[
            "vega_issuer_id_hex",
            "vega_issuer_record_epoch",
            "vega_issuer_record_digest_hex",
            "vega_issuer_public_key_hex",
            "vega_presentation_date",
            "vega_minimum_age_years",
            "vega_reader_challenge_hex",
            "vega_session_transcript_digest_hex",
        ]),
        "iroha-zk-ams-v1" => Ok(&[
            "zk_ams_action",
            "zk_ams_issuer_id_hex",
            "zk_ams_issuer_public_key_hex",
            "zk_ams_issuer_policy_record_digest_hex",
            "zk_ams_registry_id_hex",
            "zk_ams_registry_record_digest_hex",
            "zk_ams_policy_id_hex",
            "zk_ams_policy_digest_hex",
            "zk_ams_account_registry_root_hex",
            "zk_ams_account_registry_root_epoch",
            "zk_ams_subject_commitments_hex",
            "zk_ams_credential_nonces_hex",
            "zk_ams_admitted_seed_key_ring_hex",
            "zk_ams_account_id",
        ]),
        "iroha-bootle-lantern-anoncred-v1" => Ok(&[
            "bootle_lantern_policy_archive_hex",
            "bootle_lantern_disclosure_indices",
        ]),
        _ => Err(WorkerError::UnsupportedProtocol),
    }
}

fn expected_feature_flag(protocol: &str, field: &str) -> Option<bool> {
    if !PRIVACY_FEATURE_FIELDS.contains(&field) {
        return None;
    }
    Some(match field {
        "hide_amount" => protocol == "verange-transparent-range-v1",
        "hide_sender" => matches!(
            protocol,
            "vega-existing-credential-zk-v0"
                | "iroha-zk-ams-v1"
                | "iroha-bootle-lantern-anoncred-v1"
        ),
        "hide_receiver" | "hide_asset_type" | "post_quantum" => false,
        _ => return None,
    })
}

fn validate_feature_contract(
    value: Option<&norito::json::Value>,
    protocol: &str,
) -> Result<(), WorkerError> {
    let Some(norito::json::Value::Object(flags)) = value else {
        return Err(WorkerError::InvalidPublicIntent);
    };
    if flags.len() != PRIVACY_FEATURE_FIELDS.len() {
        return Err(WorkerError::InvalidPublicIntent);
    }
    for field in PRIVACY_FEATURE_FIELDS {
        let expected =
            expected_feature_flag(protocol, field).ok_or(WorkerError::InvalidPublicIntent)?;
        if flags.get(*field).and_then(norito::json::Value::as_bool) != Some(expected) {
            return Err(WorkerError::InvalidPublicIntent);
        }
    }
    Ok(())
}

/// Validate the transport representation before any native prover callback.
///
/// The eventual callback must still decode the protocol-specific fields into
/// its exact Rust action type before calling `WitnessVault::consume_with`.
fn validate_canonical_public_intent(
    canonical_public_intent: &[u8],
    binding: &WitnessBinding,
) -> Result<(), WorkerError> {
    if canonical_public_intent.is_empty()
        || canonical_public_intent.len() > MAX_CANONICAL_PUBLIC_INTENT_BYTES
        || canonical_public_intent.contains(&0)
    {
        return Err(WorkerError::InvalidPublicIntent);
    }
    let text = std::str::from_utf8(canonical_public_intent)
        .map_err(|_| WorkerError::InvalidPublicIntent)?;
    let value =
        norito::json::parse_value(text).map_err(|_| WorkerError::InvalidPublicIntent)?;
    let canonical =
        norito::json::to_json(&value).map_err(|_| WorkerError::InvalidPublicIntent)?;
    if canonical.as_bytes() != canonical_public_intent {
        return Err(WorkerError::InvalidPublicIntent);
    }
    let Some(object) = value.as_object() else {
        return Err(WorkerError::InvalidPublicIntent);
    };
    let protocol_fields = public_intent_protocol_fields(&binding.protocol)?;
    if object.len() != PUBLIC_INTENT_BASE_FIELDS.len() + protocol_fields.len()
        || !PUBLIC_INTENT_BASE_FIELDS
            .iter()
            .chain(protocol_fields.iter())
            .all(|field| object.contains_key(*field))
        || object.get("algorithm_id").and_then(norito::json::Value::as_str)
            != Some(binding.protocol.as_str())
        || object.get("protocol_id").and_then(norito::json::Value::as_str)
            != Some(binding.protocol.as_str())
        || object
            .get("signer_wallet_id")
            .and_then(norito::json::Value::as_str)
            != Some(binding.signer.as_str())
    {
        return Err(WorkerError::InvalidPublicIntent);
    }
    validate_feature_contract(object.get("selected_features"), &binding.protocol)?;
    validate_feature_contract(object.get("selected_criteria"), &binding.protocol)?;
    let actual_digest = canonical_public_intent_digest(canonical_public_intent);
    if !constant_time_eq(&actual_digest, &binding.public_intent_digest) {
        return Err(WorkerError::PublicIntentDigestMismatch);
    }
    Ok(())
}

fn validate_text(
    _label: &'static str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), WorkerError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.trim() != value
        || value
            .chars()
            .any(|character| character.is_control() || character == '\0')
    {
        return Err(WorkerError::InvalidBinding("binding text is invalid"));
    }
    Ok(())
}

fn put_text(output: &mut Vec<u8>, value: &str) {
    let length = u16::try_from(value.len()).expect("validated text length fits u16");
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn read_credential_file(path: &Path) -> Result<Zeroizing<Vec<u8>>, WorkerError> {
    if !path.is_absolute() {
        return Err(WorkerError::InvalidCredentialPath);
    }
    let before = fs::symlink_metadata(path).map_err(|error| WorkerError::Io(error.kind()))?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(WorkerError::InvalidCredentialPath);
    }
    validate_file_security(&before)?;
    if before.len() == 0 {
        return Err(WorkerError::CredentialFileEmpty);
    }
    if before.len() > MAX_CREDENTIAL_BYTES {
        return Err(WorkerError::CredentialFileTooLarge);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .truncate(false)
        .open(path)
        .map_err(|error| WorkerError::Io(error.kind()))?;
    let opened = file
        .metadata()
        .map_err(|error| WorkerError::Io(error.kind()))?;
    if !same_file_snapshot(&before, &opened) {
        return Err(WorkerError::CredentialChangedDuringImport);
    }
    let mut material = Zeroizing::new(Vec::with_capacity(
        usize::try_from(before.len()).map_err(|_| WorkerError::CredentialFileTooLarge)?,
    ));
    file.by_ref()
        .take(MAX_CREDENTIAL_BYTES + 1)
        .read_to_end(&mut material)
        .map_err(|error| WorkerError::Io(error.kind()))?;
    if material.is_empty() {
        return Err(WorkerError::CredentialFileEmpty);
    }
    if material.len() as u64 > MAX_CREDENTIAL_BYTES {
        return Err(WorkerError::CredentialFileTooLarge);
    }
    let after = file
        .metadata()
        .map_err(|error| WorkerError::Io(error.kind()))?;
    if !same_file_snapshot(&opened, &after) || material.len() as u64 != after.len()
    {
        material.zeroize();
        return Err(WorkerError::CredentialChangedDuringImport);
    }
    let material_digest = Zeroizing::new(sha256(&material));
    file.seek(SeekFrom::Start(0))
        .map_err(|error| WorkerError::Io(error.kind()))?;
    let mut verification_digest = Sha256::new();
    let mut verification_buffer = Zeroizing::new([0_u8; 8_192]);
    let mut verified_bytes = 0_u64;
    loop {
        let read = file
            .read(&mut verification_buffer[..])
            .map_err(|error| WorkerError::Io(error.kind()))?;
        if read == 0 {
            break;
        }
        verified_bytes = verified_bytes
            .checked_add(read as u64)
            .ok_or(WorkerError::CredentialFileTooLarge)?;
        if verified_bytes > MAX_CREDENTIAL_BYTES {
            material.zeroize();
            return Err(WorkerError::CredentialFileTooLarge);
        }
        verification_digest.update(&verification_buffer[..read]);
        verification_buffer[..read].zeroize();
    }
    let verified_digest_array: [u8; DIGEST_BYTES] = verification_digest.finalize().into();
    let verified_digest = Zeroizing::new(verified_digest_array);
    let verified = file
        .metadata()
        .map_err(|error| WorkerError::Io(error.kind()))?;
    if !same_file_snapshot(&after, &verified)
        || verified_bytes != material.len() as u64
        || !constant_time_eq(&material_digest, &verified_digest)
    {
        material.zeroize();
        return Err(WorkerError::CredentialChangedDuringImport);
    }
    Ok(material)
}

#[cfg(unix)]
fn validate_file_security(metadata: &fs::Metadata) -> Result<(), WorkerError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(WorkerError::CredentialFileInsecure);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_file_security(_metadata: &fs::Metadata) -> Result<(), WorkerError> {
    Err(WorkerError::CredentialFileInsecure)
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
        && left.mode() == right.mode()
}

#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.created().ok() == right.created().ok()
        && left.modified().ok() == right.modified().ok()
}

fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

fn sha256(input: &[u8]) -> [u8; DIGEST_BYTES] {
    Sha256::digest(input).into()
}

fn hmac_sha256(key: &[u8; DIGEST_BYTES], message: &[u8]) -> [u8; AUTH_TAG_BYTES] {
    const BLOCK_BYTES: usize = 64;
    let mut inner_key = Zeroizing::new([0x36_u8; BLOCK_BYTES]);
    let mut outer_key = Zeroizing::new([0x5c_u8; BLOCK_BYTES]);
    for (index, byte) in key.iter().enumerate() {
        inner_key[index] ^= byte;
        outer_key[index] ^= byte;
    }
    let mut inner = Sha256::new();
    inner.update(&inner_key[..]);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&outer_key[..]);
    outer.update(inner_digest);
    outer.finalize().into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use std::{
        io::Cursor as IoCursor,
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };

    use tempfile::TempDir;

    use super::*;

    const NOW: u64 = 1_800_000_000_000;
    const KEY: [u8; 32] = [0x51; 32];
    const CANONICAL_JINDO_PUBLIC_INTENT: &[u8] = br#"{"algorithm_id":"iroha-jindo-polynomial-commitment-v0","jindo_evaluation_point_hex":"0000000000000000000000000000000000000000000000000000000000000000","protocol_id":"iroha-jindo-polynomial-commitment-v0","selected_criteria":{"hide_amount":false,"hide_asset_type":false,"hide_receiver":false,"hide_sender":false,"post_quantum":false},"selected_features":{"hide_amount":false,"hide_asset_type":false,"hide_receiver":false,"hide_sender":false,"post_quantum":false},"signer_wallet_id":"alice@wonderland"}"#;

    fn binding() -> WitnessBinding {
        WitnessBinding {
            chain_id: "taira-testnet".to_owned(),
            genesis_digest: [1; 32],
            signer: "alice@wonderland".to_owned(),
            protocol: "iroha-jindo-polynomial-commitment-v0".to_owned(),
            profile_digest: [2; 32],
            public_intent_digest:
                canonical_public_intent_digest(CANONICAL_JINDO_PUBLIC_INTENT),
            nonce: [4; 32],
            signed_release_authority_digest: [5; 32],
        }
    }

    fn credential_file(directory: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = directory.path().join(name);
        fs::write(&path, bytes).expect("write credential");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("secure permissions");
        }
        path
    }

    fn import(vault: &mut WitnessVault, path: &Path) -> WitnessLease {
        vault
            .import_credential_at(
                ImportRequest {
                    credential_path: path.to_owned(),
                    binding: binding(),
                    ttl_millis: 30_000,
                },
                NOW,
            )
            .expect("import")
    }

    #[test]
    fn handles_are_random_opaque_and_single_use() {
        let directory = TempDir::new().expect("temp dir");
        let first = credential_file(&directory, "first", b"first secret");
        let second = credential_file(&directory, "second", b"second secret");
        let mut vault = WitnessVault::default();
        let first_lease = import(&mut vault, &first);
        let second_lease = import(&mut vault, &second);
        assert!(first_lease.handle != second_lease.handle);
        assert_ne!(first_lease.handle.as_bytes(), &[0; 32]);
        let observed = vault
            .consume_with_at(first_lease.handle, &binding(), NOW + 1, |secret| {
                Ok::<_, ()>(secret.to_vec())
            })
            .expect("consume");
        assert_eq!(observed, b"first secret");
        assert!(matches!(
            vault.consume_with_at(first_lease.handle, &binding(), NOW + 2, |_| Ok::<_, ()>(())),
            Err(ConsumeError::Custody(WorkerError::UnknownHandle))
        ));
    }

    #[test]
    fn callback_failure_still_consumes_the_handle() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        let mut vault = WitnessVault::default();
        let lease = import(&mut vault, &path);
        let result = vault.consume_with_at(
            lease.handle,
            &binding(),
            NOW + 1,
            |_| Err::<(), _>("prover failure"),
        );
        assert!(matches!(result, Err(ConsumeError::Operation("prover failure"))));
        assert!(matches!(
            vault.inspect_at(lease.handle, &binding(), NOW + 2),
            Err(WorkerError::UnknownHandle)
        ));
    }

    #[test]
    fn panic_unwind_removes_handle_without_reinsertion() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        let mut vault = WitnessVault::default();
        let lease = import(&mut vault, &path);
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ConsumeError<()>> =
                vault.consume_with_at(lease.handle, &binding(), NOW + 1, |_| panic!("stop"));
        }));
        assert!(panicked.is_err());
        assert!(matches!(
            vault.inspect_at(lease.handle, &binding(), NOW + 2),
            Err(WorkerError::UnknownHandle)
        ));
    }

    #[test]
    fn all_binding_dimensions_are_enforced() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        let mut mutations: Vec<Box<dyn Fn(&mut WitnessBinding)>> = vec![
            Box::new(|value| value.chain_id.push_str("-other")),
            Box::new(|value| value.genesis_digest[0] ^= 1),
            Box::new(|value| value.signer.push_str("-other")),
            Box::new(|value| value.protocol = "verange-transparent-range-v1".to_owned()),
            Box::new(|value| value.profile_digest[0] ^= 1),
            Box::new(|value| value.public_intent_digest[0] ^= 1),
            Box::new(|value| value.nonce[0] ^= 1),
            Box::new(|value| value.signed_release_authority_digest[0] ^= 1),
        ];
        for mutate in mutations.drain(..) {
            let mut vault = WitnessVault::default();
            let lease = import(&mut vault, &path);
            let mut wrong = binding();
            mutate(&mut wrong);
            assert!(matches!(
                vault.inspect_at(lease.handle, &wrong, NOW + 1),
                Err(WorkerError::WrongBinding)
            ));
            assert_eq!(vault.len(), 1, "mismatch must not consume the handle");
        }
    }

    #[test]
    fn expiry_and_cancel_are_atomic_terminal_states() {
        let directory = TempDir::new().expect("temp dir");
        let first = credential_file(&directory, "first", b"one");
        let second = credential_file(&directory, "second", b"two");
        let mut vault = WitnessVault::default();
        let expired = import(&mut vault, &first);
        assert!(matches!(
            vault.inspect_at(expired.handle, &binding(), expired.expires_at_millis),
            Err(WorkerError::UnknownHandle)
        ));
        let cancelled = import(&mut vault, &second);
        vault
            .cancel_at(cancelled.handle, &binding(), NOW + 1)
            .expect("cancel");
        assert!(matches!(
            vault.inspect_at(cancelled.handle, &binding(), NOW + 2),
            Err(WorkerError::UnknownHandle)
        ));
    }

    #[test]
    fn capacity_is_bounded_and_expiry_releases_capacity() {
        let directory = TempDir::new().expect("temp dir");
        let first = credential_file(&directory, "first", b"one");
        let second = credential_file(&directory, "second", b"two");
        let mut vault = WitnessVault::new(1);
        let lease = import(&mut vault, &first);
        assert!(matches!(
            vault.import_credential_at(
                ImportRequest {
                    credential_path: second.clone(),
                    binding: binding(),
                    ttl_millis: 30_000,
                },
                NOW + 1,
            ),
            Err(WorkerError::CapacityExceeded)
        ));
        assert_eq!(vault.purge_expired_at(lease.expires_at_millis), 1);
        assert!(vault
            .import_credential_at(
                ImportRequest {
                    credential_path: second,
                    binding: binding(),
                    ttl_millis: 30_000,
                },
                lease.expires_at_millis,
            )
            .is_ok());
    }

    #[test]
    fn ttl_and_binding_validation_fail_closed() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        for ttl in [0, MIN_TTL_MILLIS - 1, MAX_TTL_MILLIS + 1] {
            assert!(matches!(
                WitnessVault::default().import_credential_at(
                    ImportRequest {
                        credential_path: path.clone(),
                        binding: binding(),
                        ttl_millis: ttl,
                    },
                    NOW,
                ),
                Err(WorkerError::InvalidTtl)
            ));
        }
        let mut invalid = binding();
        invalid.nonce = [0; 32];
        assert!(matches!(
            WitnessVault::default().import_credential_at(
                ImportRequest {
                    credential_path: path,
                    binding: invalid,
                    ttl_millis: MIN_TTL_MILLIS,
                },
                NOW,
            ),
            Err(WorkerError::InvalidBinding(_))
        ));
    }

    #[test]
    fn relative_symlink_empty_and_oversized_files_are_rejected() {
        let directory = TempDir::new().expect("temp dir");
        let empty = credential_file(&directory, "empty", b"");
        assert!(matches!(
            read_credential_file(Path::new("relative")),
            Err(WorkerError::InvalidCredentialPath)
        ));
        assert!(matches!(
            read_credential_file(&empty),
            Err(WorkerError::CredentialFileEmpty)
        ));
        let oversized = credential_file(&directory, "oversized", b"x");
        fs::File::options()
            .write(true)
            .open(&oversized)
            .expect("open oversized")
            .set_len(MAX_CREDENTIAL_BYTES + 1)
            .expect("extend");
        assert!(matches!(
            read_credential_file(&oversized),
            Err(WorkerError::CredentialFileTooLarge)
        ));
        #[cfg(unix)]
        {
            let target = credential_file(&directory, "target", b"secret");
            let link = directory.path().join("link");
            std::os::unix::fs::symlink(target, &link).expect("symlink");
            assert!(matches!(
                read_credential_file(&link),
                Err(WorkerError::InvalidCredentialPath)
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn group_or_world_readable_files_are_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("change permissions");
        assert!(matches!(
            read_credential_file(&path),
            Err(WorkerError::CredentialFileInsecure)
        ));
    }

    #[test]
    fn authenticated_frame_round_trip_is_canonical() {
        let frame = AuthenticatedFrame {
            kind: CommandKind::Ping,
            sequence: 1,
            payload: vec![],
        };
        let encoded = encode_frame(&frame, &KEY).expect("encode");
        let decoded = decode_frame(&encoded, &KEY).expect("decode");
        assert_eq!(decoded.kind as u8, CommandKind::Ping as u8);
        assert_eq!(decoded.sequence, 1);
        assert!(decoded.payload.is_empty());
        assert_eq!(encode_frame(&decoded, &KEY).expect("re-encode"), encoded);
    }

    #[test]
    fn tampering_wrong_key_trailing_bytes_and_bad_lengths_are_rejected() {
        let frame = AuthenticatedFrame {
            kind: CommandKind::Ping,
            sequence: 1,
            payload: vec![],
        };
        let encoded = encode_frame(&frame, &KEY).expect("encode");
        let mut tampered = encoded.clone();
        tampered[10] ^= 1;
        assert!(matches!(
            decode_frame(&tampered, &KEY),
            Err(WorkerError::AuthenticationFailed)
        ));
        assert!(matches!(
            decode_frame(&encoded, &[0x52; 32]),
            Err(WorkerError::AuthenticationFailed)
        ));
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(matches!(
            decode_frame(&trailing, &KEY),
            Err(WorkerError::InvalidFrame)
        ));
        let mut short = encoded.clone();
        short[0..4].copy_from_slice(&1_u32.to_be_bytes());
        assert!(matches!(
            decode_frame(&short, &KEY),
            Err(WorkerError::InvalidFrame)
        ));
        let mut unknown = encoded;
        unknown[9] = 99;
        let tag_offset = unknown.len() - AUTH_TAG_BYTES;
        let tag = hmac_sha256(&KEY, &unknown[4..tag_offset]);
        unknown[tag_offset..].copy_from_slice(&tag);
        assert!(matches!(
            decode_frame(&unknown, &KEY),
            Err(WorkerError::UnknownCommand)
        ));
    }

    #[test]
    fn oversized_frames_are_rejected_before_allocation() {
        let mut input = IoCursor::new(((MAX_FRAME_BYTES as u32) + 1).to_be_bytes());
        assert!(matches!(
            read_frame(&mut input, &KEY),
            Err(WorkerError::FrameTooLarge)
        ));
        let frame = AuthenticatedFrame {
            kind: CommandKind::Ping,
            sequence: 1,
            payload: vec![0; MAX_FRAME_BYTES],
        };
        assert!(matches!(
            encode_frame(&frame, &KEY),
            Err(WorkerError::FrameTooLarge)
        ));
    }

    #[test]
    fn truncated_prefix_and_zero_session_key_fail_closed() {
        let mut truncated = IoCursor::new(vec![0_u8, 0]);
        assert!(matches!(
            read_frame(&mut truncated, &KEY),
            Err(WorkerError::Io(io::ErrorKind::UnexpectedEof))
        ));
        let mut empty = IoCursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        assert!(matches!(
            run_pipe_session(
                &mut empty,
                &mut output,
                Zeroizing::new([0; DIGEST_BYTES]),
            ),
            Err(WorkerError::AuthenticationFailed)
        ));
    }

    #[test]
    fn duplicate_and_out_of_order_sequences_terminate_the_session() {
        let ping = |sequence| {
            encode_frame(
                &AuthenticatedFrame {
                    kind: CommandKind::Ping,
                    sequence,
                    payload: vec![],
                },
                &KEY,
            )
            .expect("encode")
        };
        for input in [[ping(1), ping(1)].concat(), ping(2)] {
            let mut reader = IoCursor::new(input);
            let mut writer = Vec::new();
            assert!(matches!(
                run_pipe_session(&mut reader, &mut writer, Zeroizing::new(KEY)),
                Err(WorkerError::ReplayOrOutOfOrder)
            ));
        }
    }

    #[test]
    fn execute_ipc_fails_closed_without_consuming_or_exporting_secret() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"never export this");
        let mut vault = WitnessVault::default();
        let now = unix_time_millis().expect("native clock");
        let lease = vault
            .import_credential_at(
                ImportRequest {
                    credential_path: path,
                    binding: binding(),
                    ttl_millis: 30_000,
                },
                now,
            )
            .expect("import");
        let payload = encode_execute_payload(
            lease.handle,
            &binding(),
            CANONICAL_JINDO_PUBLIC_INTENT,
        )
        .expect("payload");
        let response = encode_response(dispatch(&mut vault, CommandKind::Execute, &payload));
        assert_eq!(response[0], 255);
        assert!(!response
            .windows(b"never export this".len())
            .any(|window| window == b"never export this"));
        assert!(vault
            .inspect_at(lease.handle, &binding(), now + 2)
            .is_ok());
    }

    #[test]
    fn forged_ipc_time_and_tampered_intent_never_release_or_consume() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        let mut vault = WitnessVault::default();
        let now = unix_time_millis().expect("native clock");
        let lease = vault
            .import_credential_at(
                ImportRequest {
                    credential_path: path,
                    binding: binding(),
                    ttl_millis: 30_000,
                },
                now,
            )
            .expect("import");

        let mut forged_time =
            encode_handle_payload(lease.handle, &binding()).expect("handle payload");
        forged_time.extend_from_slice(&1_u64.to_be_bytes());
        let response =
            encode_response(dispatch(&mut vault, CommandKind::Inspect, &forged_time));
        assert_eq!(response[0], 255);
        assert!(vault
            .inspect_at(lease.handle, &binding(), now + 1)
            .is_ok());

        let altered = std::str::from_utf8(CANONICAL_JINDO_PUBLIC_INTENT)
            .expect("utf8")
            .replace(
                "0000000000000000000000000000000000000000000000000000000000000000",
                "0100000000000000000000000000000000000000000000000000000000000000",
            );
        let mut tampered =
            encode_handle_payload(lease.handle, &binding()).expect("handle payload");
        tampered.extend_from_slice(&(altered.len() as u32).to_be_bytes());
        tampered.extend_from_slice(altered.as_bytes());
        let response =
            encode_response(dispatch(&mut vault, CommandKind::Execute, &tampered));
        assert_eq!(response[0], 255);
        assert!(vault
            .inspect_at(lease.handle, &binding(), now + 2)
            .is_ok());
    }

    #[test]
    fn execute_recomputes_canonical_public_intent_before_native_attachment() {
        assert_eq!(
            hex::encode(canonical_public_intent_digest(
                CANONICAL_JINDO_PUBLIC_INTENT,
            )),
            "0e6c727696f0fdcdf41f86e0062196acf882dea6b3b5a84934e37956acf80c41"
        );
        validate_canonical_public_intent(CANONICAL_JINDO_PUBLIC_INTENT, &binding())
            .expect("canonical typed transport intent");

        let noncanonical = [
            b" ".as_slice(),
            CANONICAL_JINDO_PUBLIC_INTENT,
        ]
        .concat();
        assert!(matches!(
            validate_canonical_public_intent(&noncanonical, &binding()),
            Err(WorkerError::InvalidPublicIntent)
        ));

        let altered = std::str::from_utf8(CANONICAL_JINDO_PUBLIC_INTENT)
            .expect("utf8")
            .replace(
                "0000000000000000000000000000000000000000000000000000000000000000",
                "0100000000000000000000000000000000000000000000000000000000000000",
            );
        assert!(matches!(
            validate_canonical_public_intent(altered.as_bytes(), &binding()),
            Err(WorkerError::PublicIntentDigestMismatch)
        ));

        let wrong_signer = std::str::from_utf8(CANONICAL_JINDO_PUBLIC_INTENT)
            .expect("utf8")
            .replace("alice@wonderland", "mallory@wonderland");
        assert!(matches!(
            validate_canonical_public_intent(wrong_signer.as_bytes(), &binding()),
            Err(WorkerError::InvalidPublicIntent)
        ));
    }

    #[test]
    fn compiled_profile_digest_binds_protocol_and_all_five_digests() {
        let profile = CompiledProfileBinding {
            parameter_id: [0x11; 32],
            parameter_digest: [0x22; 32],
            verifier_digest: [0x33; 32],
            statement_schema_digest: [0x44; 32],
            engine_manifest_digest: [0x55; 32],
        };
        let baseline = compiled_profile_digest(
            "iroha-jindo-polynomial-commitment-v0",
            &profile,
        )
        .expect("profile digest");
        assert_eq!(
            hex::encode(baseline),
            "296f382fbb00ee5646328337e71b69ecfe3551a1e896746fe916f7ad40074a2d"
        );
        let mut mutations = [
            profile.parameter_id,
            profile.parameter_digest,
            profile.verifier_digest,
            profile.statement_schema_digest,
            profile.engine_manifest_digest,
        ];
        for index in 0..mutations.len() {
            mutations[index][0] ^= 1;
            let changed = CompiledProfileBinding {
                parameter_id: mutations[0],
                parameter_digest: mutations[1],
                verifier_digest: mutations[2],
                statement_schema_digest: mutations[3],
                engine_manifest_digest: mutations[4],
            };
            assert_ne!(
                baseline,
                compiled_profile_digest(
                    "iroha-jindo-polynomial-commitment-v0",
                    &changed,
                )
                .expect("changed profile digest")
            );
            mutations[index][0] ^= 1;
        }
        assert_ne!(
            baseline,
            compiled_profile_digest("verange-transparent-range-v1", &profile)
                .expect("changed protocol digest")
        );
    }

    #[test]
    fn drop_path_is_exercised_without_cloning_or_debugging_secret_storage() {
        let directory = TempDir::new().expect("temp dir");
        let path = credential_file(&directory, "credential", b"secret");
        let dropped = Arc::new(AtomicBool::new(false));
        struct DropMarker(Arc<AtomicBool>);
        impl Drop for DropMarker {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let marker = DropMarker(Arc::clone(&dropped));
        let mut vault = WitnessVault::default();
        let lease = import(&mut vault, &path);
        let _: Result<(), ConsumeError<()>> =
            vault.consume_with_at(lease.handle, &binding(), NOW + 1, |_| {
                drop(marker);
                Ok(())
            });
        assert!(dropped.load(Ordering::SeqCst));
        assert!(vault.is_empty());
    }

}
