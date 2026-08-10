//! Stateful software signer core independent of its Unix transport.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _};

use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::{
        AdvanceSorafsReserveLifecycle, ApplySorafsRepairTaskAction, ChargeSorafsReserveRent,
        DecideSorafsReserveAppeal, DecideSorafsReserveMovement, DrawSorafsReserveCredit,
        MaintainSorafsOrderbook, MatchSorafsOrderbook, RecordSorafsOrderbookSettlementReceipt,
        RegisterSorafsReserveAccount, RepaySorafsReserveCredit, RequestSorafsReserveMovement,
        SubmitSorafsProofOutcome, SubmitSorafsRepairAppeal, SubmitSorafsRepairTask,
        SubmitSorafsReserveAppeal,
    },
    transaction::{Executable, TransactionBuilder, TransactionPayload},
};

use super::{
    envelope::{
        SoftwareSignerKeyEnvelopeAadV1, SoftwareSignerKeyEnvelopeV1, SoftwareSignerWrappingKeyV1,
    },
    journal::{
        RecoveredAdminCommitV1, RecoveredJournalV1, RecoveredSignCommitV1,
        SoftwareSignerAuditEventV1, SoftwareSignerAuditJournalV1, SoftwareSignerJournalErrorV1,
        digest_parts_signature, sync_directory, validate_private_file,
    },
    protocol::{
        AdminCommandV1, AdminRequestV1, AdminResponseV1, AdminStatusV1, ExternalSignerBackendV1,
        SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1, SIGNER_PROTOCOL_VERSION_V1,
        SIGNER_PUBLIC_BINDING_MAGIC_V1, SignRequestV1, SignResponseV1, SignStatusV1,
        SoftwareSignerKeyAlgorithmV1, SoftwareSignerLiveProvenanceV1,
        SoftwareSignerPublicBindingV1, SoftwareSignerPurposeBindingV1, SoftwareSignerRoleV1,
        admin_request_digest, admin_response_digest, digest_canonical, payload_digest,
        public_key_digest, sign_request_digest, sign_response_digest, valid_identity,
        valid_software_signer_handle,
    },
};

const ACTIVE_ENVELOPE_NAME_V1: &str = "key-envelope-v1.norito";
const PENDING_ENVELOPE_NAME_V1: &str = ".key-envelope-v1.pending";
const ENVELOPE_FILE_MAX_BYTES_V1: usize = 32 * 1024;
const PROVENANCE_ATTESTATION_DOMAIN_V1: &[u8] = b"iroha.external-signer.provenance.v1";
const RESPONSE_ATTESTATION_DOMAIN_V1: &[u8] = b"iroha.external-signer.response-attestation.v1";

/// Public inputs used to create a fresh isolated software signer.
#[derive(Clone, Debug)]
pub struct SoftwareSignerProvisioningV1 {
    /// Stable production runtime-provider handle.
    pub handle: String,
    /// Stable service identity.
    pub service_id: String,
    /// Stable independently administered identity.
    pub administrator_id: String,
    /// Exact UID that will run the service.
    pub service_uid: u32,
    /// Exact runtime-provider broker UID.
    pub client_uid: u32,
    /// Exact administrator UID.
    pub administrator_uid: u32,
    /// Least-privilege SoraFS signing role.
    pub role: SoftwareSignerRoleV1,
    /// Exact public authority admitted for purpose-separated payloads.
    pub purpose_binding: SoftwareSignerPurposeBindingV1,
    /// Initial signature algorithm.
    pub algorithm: SoftwareSignerKeyAlgorithmV1,
    /// Initial monotonic key generation.
    pub key_revision: u64,
    /// Initial monotonic policy generation.
    pub policy_revision: u64,
    /// Initial public policy digest.
    pub policy_digest: [u8; 32],
    /// Maximum canonical transaction payload size.
    pub max_request_bytes: u32,
}

impl SoftwareSignerProvisioningV1 {
    fn validate(&self) -> Result<(), SoftwareSignerErrorV1> {
        if !valid_software_signer_handle(self.role, &self.handle)
            || !valid_identity(&self.service_id)
            || !valid_identity(&self.administrator_id)
            || self.service_id == self.administrator_id
            || self.service_uid == self.client_uid
            || self.service_uid == self.administrator_uid
            || self.client_uid == self.administrator_uid
            || !self.purpose_binding.validates_role(self.role)
            || self.key_revision == 0
            || self.policy_revision == 0
            || self.policy_digest == [0; 32]
            || self.max_request_bytes == 0
            || !self.role.allows_algorithm(self.algorithm)
            || usize::try_from(self.max_request_bytes)
                .ok()
                .is_none_or(|limit| limit > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1)
        {
            return Err(SoftwareSignerErrorV1::InvalidBinding);
        }
        #[cfg(unix)]
        if rustix::process::geteuid().as_raw() != self.service_uid {
            return Err(SoftwareSignerErrorV1::IdentityMismatch);
        }
        Ok(())
    }
}

/// One opened signer service. Debug output deliberately excludes key material.
pub struct SoftwareSignerServiceV1 {
    state_directory: PathBuf,
    state: Mutex<SoftwareSignerStateV1>,
}

struct SoftwareSignerStateV1 {
    binding: SoftwareSignerPublicBindingV1,
    envelope: SoftwareSignerKeyEnvelopeV1,
    wrapping_key: SoftwareSignerWrappingKeyV1,
    keypair: KeyPair,
    journal: SoftwareSignerAuditJournalV1,
    revoked: bool,
    poisoned: bool,
    sign_commits: BTreeMap<[u8; 32], RecoveredSignCommitV1>,
    admin_commits: BTreeMap<[u8; 32], RecoveredAdminCommitV1>,
}

impl std::fmt::Debug for SoftwareSignerServiceV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lock().map_err(|_| std::fmt::Error)?;
        formatter
            .debug_struct("SoftwareSignerServiceV1")
            .field("service_id", &state.binding.service_id)
            .field("administrator_id", &state.binding.administrator_id)
            .field("role", &state.binding.role)
            .field("key_revision", &state.binding.key_revision)
            .field("audit_sequence", &state.journal.sequence())
            .field("revoked", &state.revoked)
            .field("key_material", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl SoftwareSignerServiceV1 {
    /// Generate and durably provision one new software signer.
    ///
    /// The state directory must not exist. Its parent chain must be absolute,
    /// symlink-free, and not writable by group or other users.
    ///
    /// # Errors
    ///
    /// Fails closed for invalid identities/policy, insecure paths, unavailable
    /// randomness, encryption failure, or incomplete persistence.
    pub fn provision(
        state_directory: impl Into<PathBuf>,
        provisioning: SoftwareSignerProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, SoftwareSignerErrorV1> {
        provisioning.validate()?;
        let state_directory = state_directory.into();
        validate_absolute_normal_path(&state_directory)?;
        validate_secure_ancestors(
            state_directory
                .parent()
                .ok_or(SoftwareSignerErrorV1::UntrustedPath)?,
        )?;
        create_state_directory(&state_directory)?;
        let keypair = KeyPair::try_random_with_algorithm(provisioning.algorithm.algorithm())
            .map_err(|_| SoftwareSignerErrorV1::CryptoUnavailable)?;
        let aad = SoftwareSignerKeyEnvelopeAadV1 {
            backend: ExternalSignerBackendV1::Software,
            handle: provisioning.handle,
            service_id: provisioning.service_id,
            administrator_id: provisioning.administrator_id,
            service_uid: provisioning.service_uid,
            client_uid: provisioning.client_uid,
            administrator_uid: provisioning.administrator_uid,
            role: provisioning.role,
            purpose_binding: provisioning.purpose_binding,
            domain: provisioning.role.domain().to_owned(),
            algorithm: provisioning.algorithm,
            key_revision: provisioning.key_revision,
            policy_revision: provisioning.policy_revision,
            policy_digest: provisioning.policy_digest,
            public_key: keypair.public_key().clone(),
            public_key_digest: public_key_digest(keypair.public_key())
                .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?,
            max_request_bytes: provisioning.max_request_bytes,
        };
        let envelope = SoftwareSignerKeyEnvelopeV1::create(aad.clone(), &keypair, &wrapping_key)
            .map_err(SoftwareSignerErrorV1::Envelope)?;
        persist_initial_envelope(&state_directory, &envelope)?;
        let (journal, recovered) = SoftwareSignerAuditJournalV1::create(
            &state_directory,
            aad,
            envelope.envelope_digest,
            &keypair,
        )
        .map_err(SoftwareSignerErrorV1::Journal)?;
        let binding = binding_from_recovered(&recovered)?;
        Ok(Self {
            state_directory,
            state: Mutex::new(SoftwareSignerStateV1 {
                binding,
                envelope,
                wrapping_key,
                keypair,
                journal,
                revoked: false,
                poisoned: false,
                sign_commits: BTreeMap::new(),
                admin_commits: BTreeMap::new(),
            }),
        })
    }

    /// Open and fully recover one provisioned signer.
    ///
    /// # Errors
    ///
    /// Rejects envelopes stale relative to the journal, corrupt audit records,
    /// incomplete rotations, wrong AEAD keys/AAD, insecure paths, and identity
    /// substitution. The server launcher additionally pins the independently
    /// reviewed successor binding, which rejects a coherent whole-state rollback.
    pub fn open(
        state_directory: impl Into<PathBuf>,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, SoftwareSignerErrorV1> {
        let state_directory = state_directory.into();
        validate_absolute_normal_path(&state_directory)?;
        validate_secure_ancestors(&state_directory)?;
        validate_state_directory(&state_directory)?;
        let (journal, recovered) = SoftwareSignerAuditJournalV1::open(&state_directory)
            .map_err(SoftwareSignerErrorV1::Journal)?;
        recover_envelope_transition(&state_directory, &recovered)?;
        let envelope = read_envelope(&state_directory.join(ACTIVE_ENVELOPE_NAME_V1))?;
        if envelope.envelope_digest != recovered.active_envelope_digest
            || envelope.aad() != &recovered.active_key
        {
            return Err(SoftwareSignerErrorV1::RollbackOrSubstitution);
        }
        let keypair = envelope
            .open(&wrapping_key)
            .map_err(SoftwareSignerErrorV1::Envelope)?;
        let binding = binding_from_recovered(&recovered)?;
        #[cfg(unix)]
        if binding.service_uid != rustix::process::geteuid().as_raw() {
            return Err(SoftwareSignerErrorV1::IdentityMismatch);
        }
        Ok(Self {
            state_directory,
            state: Mutex::new(SoftwareSignerStateV1 {
                binding,
                envelope,
                wrapping_key,
                keypair,
                journal,
                revoked: recovered.revoked,
                poisoned: false,
                sign_commits: recovered.sign_commits,
                admin_commits: recovered.admin_commits,
            }),
        })
    }

    /// Return the immutable public binding for the active generation.
    ///
    /// # Errors
    ///
    /// Returns unavailable if service state is poisoned.
    pub fn public_binding(&self) -> Result<SoftwareSignerPublicBindingV1, SoftwareSignerErrorV1> {
        let state = self.lock_state()?;
        state.ensure_available()?;
        Ok(state.binding.clone())
    }

    /// Return signed live qualification and current audit head.
    ///
    /// # Errors
    ///
    /// Returns unavailable if state is poisoned or attestation signing fails.
    pub fn provenance(&self) -> Result<SoftwareSignerLiveProvenanceV1, SoftwareSignerErrorV1> {
        let state = self.lock_state()?;
        state.ensure_available()?;
        state.provenance()
    }

    pub(super) fn handle_sign_request(
        &self,
        request: &SignRequestV1,
    ) -> Result<SignResponseV1, SoftwareSignerErrorV1> {
        let mut state = self.lock_state()?;
        state.ensure_available()?;
        let binding_digest = state
            .binding
            .digest()
            .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?;
        if request.binding_digest != binding_digest
            || request.operation_id == [0; 32]
            || request.payload_digest != payload_digest(&request.payload)
            || request.request_digest
                != sign_request_digest(request).map_err(|_| SoftwareSignerErrorV1::Rejected)?
            || request.expected_key_revision != state.binding.key_revision
            || request.expected_policy_revision != state.binding.policy_revision
            || request.expected_policy_digest != state.binding.policy_digest
        {
            return state.sign_error_response(request, SignStatusV1::StaleOrRevoked);
        }
        if state.revoked {
            return state.sign_error_response(request, SignStatusV1::StaleOrRevoked);
        }
        if request.payload.is_empty()
            || request.payload.len()
                > usize::try_from(state.binding.max_request_bytes)
                    .map_err(|_| SoftwareSignerErrorV1::Rejected)?
        {
            return state.sign_error_response(request, SignStatusV1::Rejected);
        }
        if let Some(commit) = state.sign_commits.get(&request.operation_id).cloned() {
            if commit.request_digest != request.request_digest
                || commit.payload_digest != request.payload_digest
            {
                let accepted_request_digest = commit.request_digest;
                state.append_audit(SoftwareSignerAuditEventV1::EquivocationRejected {
                    operation_id: request.operation_id,
                    accepted_request_digest,
                    rejected_request_digest: request.request_digest,
                })?;
                return state.sign_error_response(request, SignStatusV1::Equivocation);
            }
            return state.sign_success_response(request, SignStatusV1::Replayed, commit);
        }
        let signing_message = match state.binding.role {
            SoftwareSignerRoleV1::Promotion => {
                if state.binding.key_algorithm != SoftwareSignerKeyAlgorithmV1::Ed25519
                    || !valid_promotion_payload(&request.payload)
                {
                    return state.sign_error_response(request, SignStatusV1::Rejected);
                }
                request.payload.clone()
            }
            role if role.native_role().is_some() => {
                let builder = TransactionBuilder::decode_payload(&request.payload)
                    .map_err(|_| SoftwareSignerErrorV1::Rejected)?;
                let expected_authority = AccountId::new(state.binding.public_key.clone());
                if builder.payload().authority != expected_authority
                    || !native_payload_matches_role(state.binding.role, builder.payload())
                {
                    return state.sign_error_response(request, SignStatusV1::Rejected);
                }
                builder.payload_hash_bytes().to_vec()
            }
            _ => super::typed_payload::validated_typed_signing_message(
                &state.binding,
                &request.payload,
            )
            .map_err(|_| SoftwareSignerErrorV1::Rejected)?,
        };
        let pre_sign = state.active_snapshot();
        let signature = Signature::try_new(state.keypair.private_key(), &signing_message)
            .map_err(|_| SoftwareSignerErrorV1::CryptoUnavailable)?;
        if state.active_snapshot() != pre_sign || state.revoked {
            return state.sign_error_response(request, SignStatusV1::StaleOrRevoked);
        }
        signature
            .verify(&state.binding.public_key, &signing_message)
            .map_err(|_| SoftwareSignerErrorV1::Rejected)?;
        let signature_bytes = signature.payload().to_vec();
        let audit_head = state.append_audit(SoftwareSignerAuditEventV1::SignCommitted {
            operation_id: request.operation_id,
            request_digest: request.request_digest,
            payload_digest: request.payload_digest,
            signature: signature_bytes.clone(),
            signature_digest: digest_parts_signature(&signature_bytes),
        })?;
        let commit = RecoveredSignCommitV1 {
            request_digest: request.request_digest,
            payload_digest: request.payload_digest,
            signature: signature_bytes,
            sequence: state.journal.sequence(),
            audit_head,
        };
        state
            .sign_commits
            .insert(request.operation_id, commit.clone());
        state.sign_success_response(request, SignStatusV1::Ok, commit)
    }

    pub(super) fn attest_protocol_response(
        &self,
        response_digest: [u8; 32],
    ) -> Result<Vec<u8>, SoftwareSignerErrorV1> {
        let state = self.lock_state()?;
        state.ensure_available()?;
        state.attest_response(response_digest)
    }

    pub(super) fn handle_admin_request(
        &self,
        request: &AdminRequestV1,
    ) -> Result<AdminResponseV1, SoftwareSignerAdminErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
        state
            .ensure_available()
            .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
        if request.request_digest
            != admin_request_digest(request.binding_digest, &request.command)
                .map_err(|_| SoftwareSignerAdminErrorV1::Rejected)?
        {
            return state.admin_response(request.request_digest, AdminStatusV1::Rejected);
        }
        if let Some(operation_id) = admin_operation_id(&request.command) {
            if let Some(existing) = state.admin_commits.get(&operation_id) {
                return state.admin_response(
                    request.request_digest,
                    if existing.request_digest == request.request_digest {
                        AdminStatusV1::Replayed
                    } else {
                        AdminStatusV1::Conflict
                    },
                );
            }
        }
        if request.binding_digest
            != state
                .binding
                .digest()
                .map_err(|_| SoftwareSignerAdminErrorV1::Rejected)?
        {
            return state.admin_response(request.request_digest, AdminStatusV1::Rejected);
        }
        match &request.command {
            AdminCommandV1::Status => {
                state.admin_response(request.request_digest, AdminStatusV1::Ok)
            }
            AdminCommandV1::Rotate {
                operation_id,
                expected_audit_head,
                expected_key_revision,
                new_key_revision,
                new_policy_revision,
                new_policy_digest,
                algorithm,
            } => {
                if state.revoked
                    || *operation_id == [0; 32]
                    || *expected_audit_head != state.journal.audit_head()
                    || *expected_key_revision != state.binding.key_revision
                    || *new_key_revision <= state.binding.key_revision
                    || *new_policy_revision <= state.binding.policy_revision
                    || *new_policy_digest == [0; 32]
                {
                    return state.admin_response(request.request_digest, AdminStatusV1::Conflict);
                }
                let new_keypair = KeyPair::try_random_with_algorithm(algorithm.algorithm())
                    .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
                let mut new_aad = state.envelope.aad().clone();
                new_aad.algorithm = *algorithm;
                new_aad.key_revision = *new_key_revision;
                new_aad.policy_revision = *new_policy_revision;
                new_aad.policy_digest = *new_policy_digest;
                new_aad.public_key = new_keypair.public_key().clone();
                new_aad.public_key_digest = public_key_digest(new_keypair.public_key())
                    .map_err(|_| SoftwareSignerAdminErrorV1::Rejected)?;
                let new_envelope = SoftwareSignerKeyEnvelopeV1::create(
                    new_aad.clone(),
                    &new_keypair,
                    &state.wrapping_key,
                )
                .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
                persist_pending_envelope(&self.state_directory, &new_envelope)
                    .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
                let previous_key_revision = state.binding.key_revision;
                let previous_policy_revision = state.binding.policy_revision;
                if state
                    .append_audit(SoftwareSignerAuditEventV1::Rotated {
                        operation_id: *operation_id,
                        request_digest: request.request_digest,
                        previous_key_revision,
                        previous_policy_revision,
                        new_key: new_aad.clone(),
                        new_envelope_digest: new_envelope.envelope_digest,
                    })
                    .is_err()
                {
                    return Err(SoftwareSignerAdminErrorV1::Unavailable);
                }
                if promote_pending_envelope(&self.state_directory).is_err() {
                    state.poisoned = true;
                    return Err(SoftwareSignerAdminErrorV1::Unavailable);
                }
                state.envelope = new_envelope;
                state.keypair = new_keypair;
                let audit_genesis_digest = state.binding.audit_genesis_digest;
                state.binding = binding_from_aad(new_aad, audit_genesis_digest)
                    .map_err(|_| SoftwareSignerAdminErrorV1::Rejected)?;
                state.admin_commits.insert(
                    *operation_id,
                    RecoveredAdminCommitV1 {
                        request_digest: request.request_digest,
                    },
                );
                state.admin_response(request.request_digest, AdminStatusV1::Ok)
            }
            AdminCommandV1::Revoke {
                operation_id,
                expected_audit_head,
                expected_key_revision,
                reason_digest,
            } => {
                if state.revoked
                    || *operation_id == [0; 32]
                    || *expected_audit_head != state.journal.audit_head()
                    || *expected_key_revision != state.binding.key_revision
                    || *reason_digest == [0; 32]
                {
                    return state.admin_response(request.request_digest, AdminStatusV1::Conflict);
                }
                let key_revision = state.binding.key_revision;
                let policy_revision = state.binding.policy_revision;
                state
                    .append_audit(SoftwareSignerAuditEventV1::Revoked {
                        operation_id: *operation_id,
                        request_digest: request.request_digest,
                        key_revision,
                        policy_revision,
                        reason_digest: *reason_digest,
                    })
                    .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
                state.revoked = true;
                state.admin_commits.insert(
                    *operation_id,
                    RecoveredAdminCommitV1 {
                        request_digest: request.request_digest,
                    },
                );
                state.admin_response(request.request_digest, AdminStatusV1::Ok)
            }
        }
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, SoftwareSignerStateV1>, SoftwareSignerErrorV1> {
        self.state
            .lock()
            .map_err(|_| SoftwareSignerErrorV1::Unavailable)
    }
}

fn admin_operation_id(command: &AdminCommandV1) -> Option<[u8; 32]> {
    match command {
        AdminCommandV1::Status => None,
        AdminCommandV1::Rotate { operation_id, .. }
        | AdminCommandV1::Revoke { operation_id, .. } => Some(*operation_id),
    }
}

impl SoftwareSignerStateV1 {
    fn ensure_available(&self) -> Result<(), SoftwareSignerErrorV1> {
        if self.poisoned {
            Err(SoftwareSignerErrorV1::Unavailable)
        } else {
            Ok(())
        }
    }

    fn active_snapshot(&self) -> (u64, u64, [u8; 32], [u8; 32], bool) {
        (
            self.binding.key_revision,
            self.binding.policy_revision,
            self.binding.policy_digest,
            self.binding.public_key_digest,
            self.revoked,
        )
    }

    fn append_audit(
        &mut self,
        event: SoftwareSignerAuditEventV1,
    ) -> Result<[u8; 32], SoftwareSignerErrorV1> {
        self.journal
            .append(event, &self.keypair)
            .map_err(SoftwareSignerErrorV1::Journal)
    }

    fn provenance(&self) -> Result<SoftwareSignerLiveProvenanceV1, SoftwareSignerErrorV1> {
        let mut provenance = SoftwareSignerLiveProvenanceV1 {
            binding: self.binding.clone(),
            audit_sequence: self.journal.sequence(),
            audit_head: self.journal.audit_head(),
            revoked: self.revoked,
            attestation: Vec::new(),
        };
        let body_digest = provenance_body_digest(&provenance)?;
        provenance.attestation = Signature::try_new(self.keypair.private_key(), &body_digest)
            .map_err(|_| SoftwareSignerErrorV1::CryptoUnavailable)?
            .payload()
            .to_vec();
        Ok(provenance)
    }

    fn sign_success_response(
        &self,
        request: &SignRequestV1,
        status: SignStatusV1,
        commit: RecoveredSignCommitV1,
    ) -> Result<SignResponseV1, SoftwareSignerErrorV1> {
        let mut response = SignResponseV1 {
            operation_id: request.operation_id,
            request_digest: request.request_digest,
            payload_digest: request.payload_digest,
            status,
            signature: commit.signature,
            commit_sequence: commit.sequence,
            commit_audit_head: commit.audit_head,
            provenance: self.provenance()?,
            response_digest: [0; 32],
            response_attestation: Vec::new(),
        };
        response.response_digest =
            sign_response_digest(&response).map_err(|_| SoftwareSignerErrorV1::Unavailable)?;
        response.response_attestation = self.attest_response(response.response_digest)?;
        Ok(response)
    }

    fn sign_error_response(
        &self,
        request: &SignRequestV1,
        status: SignStatusV1,
    ) -> Result<SignResponseV1, SoftwareSignerErrorV1> {
        let mut response = SignResponseV1 {
            operation_id: request.operation_id,
            request_digest: request.request_digest,
            payload_digest: request.payload_digest,
            status,
            signature: Vec::new(),
            commit_sequence: self.journal.sequence(),
            commit_audit_head: self.journal.audit_head(),
            provenance: self.provenance()?,
            response_digest: [0; 32],
            response_attestation: Vec::new(),
        };
        response.response_digest =
            sign_response_digest(&response).map_err(|_| SoftwareSignerErrorV1::Unavailable)?;
        response.response_attestation = self.attest_response(response.response_digest)?;
        Ok(response)
    }

    fn admin_response(
        &self,
        request_digest: [u8; 32],
        status: AdminStatusV1,
    ) -> Result<AdminResponseV1, SoftwareSignerAdminErrorV1> {
        let mut response = AdminResponseV1 {
            request_digest,
            status,
            provenance: self
                .provenance()
                .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?,
            response_digest: [0; 32],
            response_attestation: Vec::new(),
        };
        response.response_digest = admin_response_digest(&response)
            .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
        response.response_attestation = self
            .attest_response(response.response_digest)
            .map_err(|_| SoftwareSignerAdminErrorV1::Unavailable)?;
        Ok(response)
    }

    fn attest_response(&self, response_digest: [u8; 32]) -> Result<Vec<u8>, SoftwareSignerErrorV1> {
        let message =
            super::protocol::digest_parts(RESPONSE_ATTESTATION_DOMAIN_V1, &[&response_digest]);
        Signature::try_new(self.keypair.private_key(), &message)
            .map(|signature| signature.payload().to_vec())
            .map_err(|_| SoftwareSignerErrorV1::CryptoUnavailable)
    }
}

pub(super) fn verify_response_attestation(
    binding: &SoftwareSignerPublicBindingV1,
    response_digest: [u8; 32],
    attestation: &[u8],
) -> Result<(), SoftwareSignerErrorV1> {
    if attestation.len() != binding.key_algorithm.algorithm().signature_payload_len() {
        return Err(SoftwareSignerErrorV1::InvalidBinding);
    }
    let signature = Signature::try_from_bytes(attestation)
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?;
    let message =
        super::protocol::digest_parts(RESPONSE_ATTESTATION_DOMAIN_V1, &[&response_digest]);
    signature
        .verify(&binding.public_key, &message)
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)
}

pub(super) fn verify_provenance(
    provenance: &SoftwareSignerLiveProvenanceV1,
) -> Result<(), SoftwareSignerErrorV1> {
    provenance
        .binding
        .validate()
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?;
    if provenance.audit_sequence == 0
        || provenance.audit_head == [0; 32]
        || provenance.attestation.len()
            != provenance
                .binding
                .key_algorithm
                .algorithm()
                .signature_payload_len()
    {
        return Err(SoftwareSignerErrorV1::InvalidBinding);
    }
    let signature = Signature::try_from_bytes(&provenance.attestation)
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?;
    signature
        .verify(
            &provenance.binding.public_key,
            &provenance_body_digest(provenance)?,
        )
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)
}

fn provenance_body_digest(
    provenance: &SoftwareSignerLiveProvenanceV1,
) -> Result<[u8; 32], SoftwareSignerErrorV1> {
    digest_canonical(
        PROVENANCE_ATTESTATION_DOMAIN_V1,
        &(
            provenance.binding.clone(),
            provenance.audit_sequence,
            provenance.audit_head,
            provenance.revoked,
        ),
    )
    .map_err(|_| SoftwareSignerErrorV1::Unavailable)
}

fn valid_promotion_payload(payload: &[u8]) -> bool {
    let Some(json) = payload.strip_prefix(super::protocol::SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1)
    else {
        return false;
    };
    !json.is_empty()
        && json.first() == Some(&b'{')
        && json.last() == Some(&b'}')
        && !json.contains(&0)
        && std::str::from_utf8(json).is_ok()
}

pub(super) fn native_payload_matches_role(
    role: SoftwareSignerRoleV1,
    payload: &TransactionPayload,
) -> bool {
    let Executable::Instructions(instructions) = payload.instructions() else {
        return false;
    };
    let [instruction] = instructions.as_ref() else {
        return false;
    };
    let instruction = instruction.as_any();
    match role {
        SoftwareSignerRoleV1::ProofOutcome => instruction
            .downcast_ref::<SubmitSorafsProofOutcome>()
            .is_some(),
        SoftwareSignerRoleV1::Repair => {
            instruction
                .downcast_ref::<SubmitSorafsRepairTask>()
                .is_some()
                || instruction
                    .downcast_ref::<ApplySorafsRepairTaskAction>()
                    .is_some()
                || instruction
                    .downcast_ref::<SubmitSorafsRepairAppeal>()
                    .is_some()
        }
        SoftwareSignerRoleV1::Reserve => {
            instruction
                .downcast_ref::<RegisterSorafsReserveAccount>()
                .is_some()
                || instruction
                    .downcast_ref::<RequestSorafsReserveMovement>()
                    .is_some()
                || instruction
                    .downcast_ref::<DecideSorafsReserveMovement>()
                    .is_some()
                || instruction
                    .downcast_ref::<ChargeSorafsReserveRent>()
                    .is_some()
                || instruction
                    .downcast_ref::<AdvanceSorafsReserveLifecycle>()
                    .is_some()
                || instruction
                    .downcast_ref::<DrawSorafsReserveCredit>()
                    .is_some()
                || instruction
                    .downcast_ref::<RepaySorafsReserveCredit>()
                    .is_some()
                || instruction
                    .downcast_ref::<SubmitSorafsReserveAppeal>()
                    .is_some()
                || instruction
                    .downcast_ref::<DecideSorafsReserveAppeal>()
                    .is_some()
        }
        SoftwareSignerRoleV1::Orderbook => {
            instruction.downcast_ref::<MatchSorafsOrderbook>().is_some()
                || instruction
                    .downcast_ref::<MaintainSorafsOrderbook>()
                    .is_some()
                || instruction
                    .downcast_ref::<RecordSorafsOrderbookSettlementReceipt>()
                    .is_some()
        }
        SoftwareSignerRoleV1::Promotion
        | SoftwareSignerRoleV1::GovernanceDag
        | SoftwareSignerRoleV1::PotrGateway
        | SoftwareSignerRoleV1::PotrProvider
        | SoftwareSignerRoleV1::BillingStatement
        | SoftwareSignerRoleV1::EvidenceViewer
        | SoftwareSignerRoleV1::StreamToken
        | SoftwareSignerRoleV1::PopCredentials => false,
    }
}

fn binding_from_recovered(
    recovered: &RecoveredJournalV1,
) -> Result<SoftwareSignerPublicBindingV1, SoftwareSignerErrorV1> {
    binding_from_aad(recovered.active_key.clone(), recovered.audit_genesis_digest)
}

fn binding_from_aad(
    aad: SoftwareSignerKeyEnvelopeAadV1,
    audit_genesis_digest: [u8; 32],
) -> Result<SoftwareSignerPublicBindingV1, SoftwareSignerErrorV1> {
    let binding = SoftwareSignerPublicBindingV1 {
        magic: SIGNER_PUBLIC_BINDING_MAGIC_V1,
        version: SIGNER_PROTOCOL_VERSION_V1,
        backend: aad.backend,
        handle: aad.handle,
        service_id: aad.service_id,
        administrator_id: aad.administrator_id,
        service_uid: aad.service_uid,
        client_uid: aad.client_uid,
        administrator_uid: aad.administrator_uid,
        role: aad.role,
        purpose_binding: aad.purpose_binding,
        domain: aad.domain,
        key_algorithm: aad.algorithm,
        key_revision: aad.key_revision,
        policy_revision: aad.policy_revision,
        policy_digest: aad.policy_digest,
        public_key: aad.public_key,
        public_key_digest: aad.public_key_digest,
        audit_genesis_digest,
        max_request_bytes: aad.max_request_bytes,
    };
    binding
        .validate()
        .map_err(|_| SoftwareSignerErrorV1::InvalidBinding)?;
    Ok(binding)
}

fn persist_initial_envelope(
    state_directory: &Path,
    envelope: &SoftwareSignerKeyEnvelopeV1,
) -> Result<(), SoftwareSignerErrorV1> {
    persist_pending_envelope(state_directory, envelope)?;
    promote_pending_envelope(state_directory)
}

fn persist_pending_envelope(
    state_directory: &Path,
    envelope: &SoftwareSignerKeyEnvelopeV1,
) -> Result<(), SoftwareSignerErrorV1> {
    envelope
        .validate_public()
        .map_err(SoftwareSignerErrorV1::Envelope)?;
    let bytes =
        norito::encode_canonical(envelope).map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    if bytes.is_empty() || bytes.len() > ENVELOPE_FILE_MAX_BYTES_V1 {
        return Err(SoftwareSignerErrorV1::Persistence);
    }
    let pending = state_directory.join(PENDING_ENVELOPE_NAME_V1);
    if pending.exists() {
        return Err(SoftwareSignerErrorV1::Persistence);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&pending)
        .map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    validate_private_file(&pending).map_err(SoftwareSignerErrorV1::Journal)?;
    sync_directory(state_directory).map_err(SoftwareSignerErrorV1::Journal)
}

fn promote_pending_envelope(state_directory: &Path) -> Result<(), SoftwareSignerErrorV1> {
    let pending = state_directory.join(PENDING_ENVELOPE_NAME_V1);
    let active = state_directory.join(ACTIVE_ENVELOPE_NAME_V1);
    validate_private_file(&pending).map_err(SoftwareSignerErrorV1::Journal)?;
    fs::rename(&pending, &active).map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    sync_directory(state_directory).map_err(SoftwareSignerErrorV1::Journal)?;
    validate_private_file(&active).map_err(SoftwareSignerErrorV1::Journal)
}

fn recover_envelope_transition(
    state_directory: &Path,
    recovered: &RecoveredJournalV1,
) -> Result<(), SoftwareSignerErrorV1> {
    let active_path = state_directory.join(ACTIVE_ENVELOPE_NAME_V1);
    let pending_path = state_directory.join(PENDING_ENVELOPE_NAME_V1);
    let active = active_path
        .exists()
        .then(|| read_envelope(&active_path))
        .transpose()?;
    let pending = pending_path
        .exists()
        .then(|| read_envelope(&pending_path))
        .transpose()?;
    match (active, pending) {
        (Some(active), None) if active.envelope_digest == recovered.active_envelope_digest => {
            Ok(())
        }
        (Some(active), Some(pending))
            if pending.envelope_digest == recovered.active_envelope_digest
                && active.envelope_digest != recovered.active_envelope_digest =>
        {
            promote_pending_envelope(state_directory)
        }
        (Some(active), Some(pending))
            if active.envelope_digest == recovered.active_envelope_digest
                && pending.envelope_digest != recovered.active_envelope_digest =>
        {
            validate_private_file(&pending_path).map_err(SoftwareSignerErrorV1::Journal)?;
            fs::remove_file(&pending_path).map_err(|_| SoftwareSignerErrorV1::Persistence)?;
            sync_directory(state_directory).map_err(SoftwareSignerErrorV1::Journal)
        }
        _ => Err(SoftwareSignerErrorV1::RollbackOrSubstitution),
    }
}

fn read_envelope(path: &Path) -> Result<SoftwareSignerKeyEnvelopeV1, SoftwareSignerErrorV1> {
    validate_private_file(path).map_err(SoftwareSignerErrorV1::Journal)?;
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(
                u64::try_from(ENVELOPE_FILE_MAX_BYTES_V1 + 1)
                    .map_err(|_| std::io::Error::other("envelope bound"))?,
            )
            .read_to_end(&mut bytes)
        })
        .map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    if bytes.is_empty() || bytes.len() > ENVELOPE_FILE_MAX_BYTES_V1 {
        return Err(SoftwareSignerErrorV1::Persistence);
    }
    let envelope: SoftwareSignerKeyEnvelopeV1 =
        norito::decode_canonical(&bytes).map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    if norito::encode_canonical(&envelope).map_err(|_| SoftwareSignerErrorV1::Persistence)? != bytes
    {
        return Err(SoftwareSignerErrorV1::Persistence);
    }
    envelope
        .validate_public()
        .map_err(SoftwareSignerErrorV1::Envelope)?;
    Ok(envelope)
}

fn validate_absolute_normal_path(path: &Path) -> Result<(), SoftwareSignerErrorV1> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir | Component::ParentDir | Component::Prefix(_)
            )
        })
    {
        return Err(SoftwareSignerErrorV1::UntrustedPath);
    }
    Ok(())
}

fn validate_secure_ancestors(path: &Path) -> Result<(), SoftwareSignerErrorV1> {
    for ancestor in path.ancestors() {
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| SoftwareSignerErrorV1::UntrustedPath)?;
        #[cfg(unix)]
        {
            let euid = rustix::process::geteuid().as_raw();
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || (metadata.uid() != 0 && metadata.uid() != euid)
                || metadata.mode() & 0o022 != 0
            {
                return Err(SoftwareSignerErrorV1::UntrustedPath);
            }
        }
        #[cfg(not(unix))]
        if !metadata.is_dir() {
            return Err(SoftwareSignerErrorV1::UntrustedPath);
        }
    }
    Ok(())
}

fn create_state_directory(path: &Path) -> Result<(), SoftwareSignerErrorV1> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|_| SoftwareSignerErrorV1::Persistence)?;
    validate_state_directory(path)
}

fn validate_state_directory(path: &Path) -> Result<(), SoftwareSignerErrorV1> {
    let metadata = fs::symlink_metadata(path).map_err(|_| SoftwareSignerErrorV1::UntrustedPath)?;
    #[cfg(unix)]
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(SoftwareSignerErrorV1::UntrustedPath);
    }
    #[cfg(not(unix))]
    if !metadata.is_dir() {
        return Err(SoftwareSignerErrorV1::UntrustedPath);
    }
    Ok(())
}

/// Payload-free signer service failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerErrorV1 {
    /// Public identity or policy binding is invalid.
    InvalidBinding,
    /// Effective service identity differs from the governed binding.
    IdentityMismatch,
    /// State or runtime filesystem path is not trustworthy.
    UntrustedPath,
    /// Encrypted key envelope failed validation or opening.
    Envelope(super::envelope::SoftwareSignerEnvelopeErrorV1),
    /// Audit journal failed validation or durable mutation.
    Journal(SoftwareSignerJournalErrorV1),
    /// Active envelope or journal was rolled back or substituted.
    RollbackOrSubstitution,
    /// Request was malformed, unauthorized for the role, or noncanonical.
    Rejected,
    /// Cryptographic randomness or signing was unavailable.
    CryptoUnavailable,
    /// Durable state could not be committed.
    Persistence,
    /// Service state is unavailable or poisoned.
    Unavailable,
}

/// Payload-free administrator operation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareSignerAdminErrorV1 {
    /// Request or transition is invalid.
    Rejected,
    /// Durable mutation or key generation was unavailable.
    Unavailable,
}
