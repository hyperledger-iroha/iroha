//! Role service, run assignment, descriptor validation, replay, and receipts.

use super::super::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerProvisioningV1, SoftwareSignerPurposeBindingV1,
    SoftwareSignerRoleV1, SoftwareSignerRotationSuccessorV1, SoftwareSignerServiceV1,
    SoftwareSignerSignatureReceiptV1, SoftwareSignerWrappingKeyV1,
    verify_software_signer_rotation_successor,
};
use super::{
    protocol::{
        AuthorityAdminCommandV1, FinalizePrivacyGenesisV1, OperationResponseV1, OperationStatusV1,
        ReplayConsumptionV1, RunAssignmentV1, StoredAuthorizationV1,
        StoredDeploymentFinalizationV1, StoredRotationHandoffV1, StoredRunAssignmentV1,
        TAIRA_AUTHORITY_BINDING_MAGIC_V1, TAIRA_AUTHORITY_MAX_ARTIFACT_BYTES_V1,
        TAIRA_AUTHORITY_MAX_ARTIFACTS_V1, TAIRA_AUTHORITY_MAX_JSON_BYTES_V1,
        TAIRA_AUTHORITY_MAX_TOTAL_ARTIFACT_BYTES_V1, TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
        TairaAuthorityArtifactManifestEntryV1, TairaAuthorityPublicBindingV1, TairaAuthorityRoleV1,
        sha256,
    },
    store::{
        create_private_subdirectory, directory_contains_only_records, load_canonical_records,
        persist_canonical_once, validate_private_directory,
    },
};
use crate::external_software_signer::privacy_governance;
use crate::external_software_signer::protocol::{
    AdminCommandV1, AdminRequestV1, AdminStatusV1, SignResponseV1, SignStatusV1,
    TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1, admin_request_digest,
};
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::transaction::TransactionBuilder;
use iroha_version::codec::EncodeVersioned as _;
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom},
    os::{
        fd::OwnedFd,
        unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    },
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const ASSIGNMENTS_DIRECTORY_V1: &str = "authority-run-assignments-v1";
const CONSUMPTIONS_DIRECTORY_V1: &str = "authority-replay-consumptions-v1";
const RECEIPTS_DIRECTORY_V1: &str = "authority-receipts-v1";
const DEPLOYMENT_FINALIZATIONS_DIRECTORY_V1: &str = "authority-deployment-finalizations-v1";
const ROTATION_HANDOFFS_DIRECTORY_V1: &str = "authority-rotation-handoffs-v1";
const PRIVACY_GENESIS_FINALIZATION_DIRECTORY_V1: &str = "privacy-genesis-finalization-v1";
const PUBLIC_SOAK_OBSERVATION_BINDING_DIRECTORY_V1: &str = "public-soak-observation-binding-v1";
const MAX_RUN_LIFETIME_MILLIS_V1: u64 = 24 * 60 * 60 * 1_000;
const ASSIGNMENT_SIGNING_DOMAIN_V1: &[u8] = b"iroha:taira:run-assignment:v1\0";
const RECEIPT_OPERATION_DOMAIN_V1: &[u8] = b"iroha:taira:durable-receipt-operation:v1\0";
const DURABLE_RECEIPT_SIGNING_DOMAIN_V1: &[u8] = b"iroha:taira:durable-receipt:v1\0";
const RUN_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-run-id:v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"iroha:taira:authority-operation-id:v1\0";
const DEPLOYMENT_FINALIZATION_OPERATION_DOMAIN_V1: &[u8] =
    b"iroha:taira:deployment-finalization-operation:v1\0";
const PRIVACY_GENESIS_FINALIZATION_OPERATION_DOMAIN_V1: &[u8] =
    b"iroha:taira:finalize-privacy-genesis:v1\0";
const PUBLIC_SOAK_SUBJECT_DOMAIN_V1: &[u8] =
    b"iroha.taira.public-v2-24h-soak.authority-subject.v1\0";
const PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.taira.public-v2-24h-soak.authority-envelope-signature.v1\0";
const PUBLIC_SOAK_BROKER_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha.taira.public-v2-24h-soak.durable-admission-signature.v1\0";
const PUBLIC_SOAK_REPLAY_NAMESPACE_V1: &str = "iroha.taira.public-v2-24h-soak-authority-replay.v1";
const PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1: u64 = 15 * 60 * 1_000;

/// Public inputs used to provision one isolated authority role.
#[derive(Clone, Debug)]
pub struct TairaAuthorityProvisioningV1 {
    /// Exact role owned by this process and key.
    pub role: TairaAuthorityRoleV1,
    /// Stable service identity.
    pub service_id: String,
    /// Stable independent administrator identity.
    pub administrator_id: String,
    /// Exact service UID.
    pub service_uid: u32,
    /// Exact authorized client UID.
    pub client_uid: u32,
    /// Exact administrator UID.
    pub administrator_uid: u32,
    /// Initial positive key revision.
    pub key_revision: u64,
    /// Initial positive policy revision.
    pub policy_revision: u64,
    /// SHA-256 of reviewed role policy.
    pub policy_digest: [u8; 32],
    /// Maximum canonical authority request bytes.
    pub max_request_bytes: u32,
}

/// Redaction-safe authority failure classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TairaAuthorityErrorV1 {
    /// A binding, identity, role, or policy was invalid.
    Binding,
    /// An authenticated request did not match its assignment or schema.
    Rejected,
    /// A run or operation identifier was reused for different bytes.
    Conflict,
    /// Durable state was incomplete, corrupt, or unavailable.
    State,
    /// A signature or receipt could not be produced or verified.
    Crypto,
}

impl std::fmt::Display for TairaAuthorityErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Binding => "authority binding was rejected",
            Self::Rejected => "authority request was rejected",
            Self::Conflict => "authority replay conflict",
            Self::State => "authority state is unavailable",
            Self::Crypto => "authority authentication failed",
        })
    }
}

impl std::error::Error for TairaAuthorityErrorV1 {}

fn validate_qualification_service_identity(
    role: TairaAuthorityRoleV1,
    service_uid: u32,
) -> Result<(), TairaAuthorityErrorV1> {
    if role != TairaAuthorityRoleV1::Qualification {
        return Ok(());
    }
    if service_uid != 0 {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    // Unit tests exercise the service directly without opening production
    // sockets.  Every production build must provision, recover, and report
    // readiness for qualification only from a real root service process so
    // its sandbox can drop to the unrelated host nobody identity.
    #[cfg(not(test))]
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(())
}

struct AuthorityStateV1 {
    assignments: BTreeMap<[u8; 32], StoredRunAssignmentV1>,
    consumptions: BTreeMap<[u8; 32], ReplayConsumptionV1>,
    authorizations: BTreeMap<[u8; 32], StoredAuthorizationV1>,
    deployment_finalizations: BTreeMap<[u8; 32], StoredDeploymentFinalizationV1>,
    rotation_handoffs: BTreeMap<[u8; 32], StoredRotationHandoffV1>,
}

/// One opened authority role backed by the existing encrypted signer and audit journal.
pub struct TairaAuthorityServiceV1 {
    state_directory: PathBuf,
    role: TairaAuthorityRoleV1,
    signer: Arc<SoftwareSignerServiceV1>,
    state: Mutex<AuthorityStateV1>,
    privacy_genesis_finalized: bool,
    public_soak_observation_binding: Option<TairaAuthorityPublicBindingV1>,
}

impl std::fmt::Debug for TairaAuthorityServiceV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TairaAuthorityServiceV1")
            .field("role", &self.role)
            .field("state_directory", &self.state_directory)
            .field("private_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl TairaAuthorityServiceV1 {
    /// Provision a new role directory, encrypted Ed25519 key, and genesis audit record.
    pub fn provision(
        state_directory: impl Into<PathBuf>,
        provisioning: TairaAuthorityProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        Self::provision_inner(
            state_directory.into(),
            provisioning,
            wrapping_key,
            None,
            None,
        )
    }

    pub(super) fn provision_with_retained_genesis_key(
        state_directory: impl Into<PathBuf>,
        provisioning: TairaAuthorityProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
        retained_genesis_key: KeyPair,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        Self::provision_inner(
            state_directory.into(),
            provisioning,
            wrapping_key,
            Some(retained_genesis_key),
            None,
        )
    }

    pub(super) fn provision_with_public_soak_observation_binding(
        state_directory: impl Into<PathBuf>,
        provisioning: TairaAuthorityProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
        observation_binding: TairaAuthorityPublicBindingV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        Self::provision_inner(
            state_directory.into(),
            provisioning,
            wrapping_key,
            None,
            Some(observation_binding),
        )
    }

    fn provision_inner(
        state_directory: PathBuf,
        provisioning: TairaAuthorityProvisioningV1,
        wrapping_key: SoftwareSignerWrappingKeyV1,
        retained_genesis_key: Option<KeyPair>,
        observation_binding: Option<TairaAuthorityPublicBindingV1>,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        validate_qualification_service_identity(provisioning.role, provisioning.service_uid)?;
        if (provisioning.role == TairaAuthorityRoleV1::PrivacyGovernance)
            != retained_genesis_key.is_some()
        {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        if (provisioning.role == TairaAuthorityRoleV1::PublicSoakReplayAdmission)
            != observation_binding.is_some()
        {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let signer_provisioning = SoftwareSignerProvisioningV1 {
            handle: format!(
                "software://sorafs/taira-authority/{}",
                provisioning.role.as_str()
            ),
            service_id: provisioning.service_id,
            administrator_id: provisioning.administrator_id,
            service_uid: provisioning.service_uid,
            client_uid: provisioning.client_uid,
            administrator_uid: provisioning.administrator_uid,
            role: SoftwareSignerRoleV1::TairaAuthority,
            purpose_binding: SoftwareSignerPurposeBindingV1::TairaAuthority {
                role: provisioning.role.as_str().to_owned(),
            },
            algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
            key_revision: provisioning.key_revision,
            policy_revision: provisioning.policy_revision,
            policy_digest: provisioning.policy_digest,
            max_request_bytes: provisioning.max_request_bytes,
        };
        let signer = if let Some(retained_genesis_key) = retained_genesis_key {
            SoftwareSignerServiceV1::provision_with_keypair(
                &state_directory,
                signer_provisioning,
                wrapping_key,
                retained_genesis_key,
            )
        } else {
            SoftwareSignerServiceV1::provision(&state_directory, signer_provisioning, wrapping_key)
        }
        .map_err(|_| TairaAuthorityErrorV1::State)?;
        for name in [
            ASSIGNMENTS_DIRECTORY_V1,
            CONSUMPTIONS_DIRECTORY_V1,
            RECEIPTS_DIRECTORY_V1,
            DEPLOYMENT_FINALIZATIONS_DIRECTORY_V1,
            ROTATION_HANDOFFS_DIRECTORY_V1,
            PRIVACY_GENESIS_FINALIZATION_DIRECTORY_V1,
            PUBLIC_SOAK_OBSERVATION_BINDING_DIRECTORY_V1,
        ] {
            create_private_subdirectory(&state_directory.join(name))
                .map_err(|()| TairaAuthorityErrorV1::State)?;
        }
        let service = Self {
            state_directory,
            role: provisioning.role,
            signer: Arc::new(signer),
            state: Mutex::new(AuthorityStateV1 {
                assignments: BTreeMap::new(),
                consumptions: BTreeMap::new(),
                authorizations: BTreeMap::new(),
                deployment_finalizations: BTreeMap::new(),
                rotation_handoffs: BTreeMap::new(),
            }),
            privacy_genesis_finalized: provisioning.role == TairaAuthorityRoleV1::PrivacyGovernance,
            public_soak_observation_binding: observation_binding,
        };
        if let Some(observation) = &service.public_soak_observation_binding {
            service.validate_public_soak_observation_binding(observation)?;
            let key = observation
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?;
            persist_canonical_once(
                &service
                    .state_directory
                    .join(PUBLIC_SOAK_OBSERVATION_BINDING_DIRECTORY_V1),
                key,
                observation,
            )
            .map_err(|()| TairaAuthorityErrorV1::State)?;
        }
        if service.privacy_genesis_finalized {
            service.ensure_privacy_genesis_finalized()?;
        }
        service.public_binding()?;
        Ok(service)
    }

    /// Open and fully recover one role directory and all immutable ledgers.
    pub fn open(
        state_directory: impl Into<PathBuf>,
        wrapping_key: SoftwareSignerWrappingKeyV1,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        let state_directory = state_directory.into();
        let signer = Arc::new(
            SoftwareSignerServiceV1::open(&state_directory, wrapping_key)
                .map_err(|_| TairaAuthorityErrorV1::State)?,
        );
        let signer_binding = signer
            .public_binding()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let SoftwareSignerPurposeBindingV1::TairaAuthority { role } =
            &signer_binding.purpose_binding
        else {
            return Err(TairaAuthorityErrorV1::Binding);
        };
        let role = role.parse().map_err(|()| TairaAuthorityErrorV1::Binding)?;
        validate_qualification_service_identity(role, signer_binding.service_uid)?;
        let assignments_path = state_directory.join(ASSIGNMENTS_DIRECTORY_V1);
        let consumptions_path = state_directory.join(CONSUMPTIONS_DIRECTORY_V1);
        let receipts_path = state_directory.join(RECEIPTS_DIRECTORY_V1);
        let deployment_finalizations_path =
            state_directory.join(DEPLOYMENT_FINALIZATIONS_DIRECTORY_V1);
        let rotation_handoffs_path = state_directory.join(ROTATION_HANDOFFS_DIRECTORY_V1);
        let privacy_finalization_path =
            state_directory.join(PRIVACY_GENESIS_FINALIZATION_DIRECTORY_V1);
        let observation_binding_path =
            state_directory.join(PUBLIC_SOAK_OBSERVATION_BINDING_DIRECTORY_V1);
        for path in [
            &assignments_path,
            &consumptions_path,
            &receipts_path,
            &deployment_finalizations_path,
            &rotation_handoffs_path,
            &privacy_finalization_path,
            &observation_binding_path,
        ] {
            validate_private_directory(path).map_err(|()| TairaAuthorityErrorV1::State)?;
            directory_contains_only_records(path).map_err(|()| TairaAuthorityErrorV1::State)?;
        }
        let assignments =
            load_canonical_records(&assignments_path).map_err(|()| TairaAuthorityErrorV1::State)?;
        let consumptions = load_canonical_records(&consumptions_path)
            .map_err(|()| TairaAuthorityErrorV1::State)?;
        let authorizations =
            load_canonical_records(&receipts_path).map_err(|()| TairaAuthorityErrorV1::State)?;
        let deployment_finalizations = load_canonical_records(&deployment_finalizations_path)
            .map_err(|()| TairaAuthorityErrorV1::State)?;
        let rotation_handoffs = load_canonical_records(&rotation_handoffs_path)
            .map_err(|()| TairaAuthorityErrorV1::State)?;
        let privacy_finalizations: BTreeMap<[u8; 32], FinalizePrivacyGenesisV1> =
            load_canonical_records(&privacy_finalization_path)
                .map_err(|()| TairaAuthorityErrorV1::State)?;
        let observation_bindings: BTreeMap<[u8; 32], TairaAuthorityPublicBindingV1> =
            load_canonical_records(&observation_binding_path)
                .map_err(|()| TairaAuthorityErrorV1::State)?;
        if privacy_finalizations.len() > 1
            || (role != TairaAuthorityRoleV1::PrivacyGovernance
                && !privacy_finalizations.is_empty())
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        if observation_bindings.len() > 1
            || (role == TairaAuthorityRoleV1::PublicSoakReplayAdmission)
                != (observation_bindings.len() == 1)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let public_soak_observation_binding = observation_bindings.values().next().cloned();
        validate_recovered_state(
            role,
            &signer,
            &assignments,
            &consumptions,
            &authorizations,
            &deployment_finalizations,
            &rotation_handoffs,
        )?;
        let service = Self {
            state_directory,
            role,
            signer,
            state: Mutex::new(AuthorityStateV1 {
                assignments,
                consumptions,
                authorizations,
                deployment_finalizations,
                rotation_handoffs,
            }),
            privacy_genesis_finalized: role == TairaAuthorityRoleV1::PrivacyGovernance,
            public_soak_observation_binding,
        };
        if let Some(observation) = &service.public_soak_observation_binding {
            service.validate_public_soak_observation_binding(observation)?;
        }
        if service.privacy_genesis_finalized && privacy_finalizations.is_empty() {
            service.ensure_privacy_genesis_finalized()?;
        } else if service.privacy_genesis_finalized {
            service.verify_privacy_genesis_finalization(&privacy_finalizations)?;
        }
        service.public_binding()?;
        Ok(service)
    }

    /// Return the authenticated public binding for the active key generation.
    pub fn public_binding(&self) -> Result<TairaAuthorityPublicBindingV1, TairaAuthorityErrorV1> {
        let binding = TairaAuthorityPublicBindingV1 {
            magic: TAIRA_AUTHORITY_BINDING_MAGIC_V1,
            version: TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
            role: self.role,
            signer: self
                .signer
                .public_binding()
                .map_err(|_| TairaAuthorityErrorV1::State)?,
        };
        binding
            .validate()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        Ok(binding)
    }

    pub(super) fn provenance(
        &self,
    ) -> Result<super::super::SoftwareSignerLiveProvenanceV1, TairaAuthorityErrorV1> {
        self.signer
            .provenance()
            .map_err(|_| TairaAuthorityErrorV1::State)
    }

    pub(super) fn attest_response(
        &self,
        response_digest: [u8; 32],
    ) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        self.signer
            .attest_protocol_response(response_digest)
            .map_err(|_| TairaAuthorityErrorV1::Crypto)
    }

    pub(super) fn recover_rotation_handoff_from_predecessor(
        &self,
        previous: &TairaAuthorityPublicBindingV1,
    ) -> Result<(), TairaAuthorityErrorV1> {
        previous
            .validate()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        let current = self.public_binding()?;
        if previous == &current {
            return Ok(());
        }
        if previous.role != self.role {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let successor = self
            .signer
            .taira_current_rotation_successor(&previous.signer)
            .map_err(|_| TairaAuthorityErrorV1::Binding)?;
        let stored = stored_rotation_handoff(previous.clone(), successor)?;
        let operation_id = rotation_operation_id(&stored.command)?;
        persist_canonical_once(
            &self.state_directory.join(ROTATION_HANDOFFS_DIRECTORY_V1),
            operation_id,
            &stored,
        )
        .map_err(|()| TairaAuthorityErrorV1::State)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if let Some(existing) = state.rotation_handoffs.get(&operation_id) {
            if existing != &stored {
                return Err(TairaAuthorityErrorV1::Conflict);
            }
        } else {
            state.rotation_handoffs.insert(operation_id, stored);
        }
        Ok(())
    }

    pub(super) fn binding_for_admin_request(
        &self,
        digest: [u8; 32],
        command: &AuthorityAdminCommandV1,
    ) -> Result<TairaAuthorityPublicBindingV1, TairaAuthorityErrorV1> {
        let current = self.public_binding()?;
        if current
            .sha256()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?
            == digest
        {
            return Ok(current);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        state
            .rotation_handoffs
            .values()
            .find(|handoff| {
                handoff.previous_binding.sha256().ok() == Some(digest)
                    && handoff.successor_binding == current
                    && (matches!(command, AuthorityAdminCommandV1::Status)
                        || command == &handoff.command)
            })
            .map(|handoff| handoff.previous_binding.clone())
            .ok_or(TairaAuthorityErrorV1::Binding)
    }

    fn validate_public_soak_observation_binding(
        &self,
        observation: &TairaAuthorityPublicBindingV1,
    ) -> Result<(), TairaAuthorityErrorV1> {
        let replay = self.public_binding()?;
        if self.role != TairaAuthorityRoleV1::PublicSoakReplayAdmission
            || observation.role != TairaAuthorityRoleV1::PublicSoakObservation
            || observation.validate().is_err()
            || observation.signer.handle == replay.signer.handle
            || observation.signer.service_id == replay.signer.service_id
            || observation.signer.administrator_id == replay.signer.administrator_id
            || observation.signer.public_key_digest == replay.signer.public_key_digest
            || [
                observation.signer.service_uid,
                observation.signer.client_uid,
                observation.signer.administrator_uid,
            ]
            .into_iter()
            .any(|uid| {
                [
                    replay.signer.service_uid,
                    replay.signer.client_uid,
                    replay.signer.administrator_uid,
                ]
                .contains(&uid)
            })
        {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        Ok(())
    }

    fn ensure_privacy_genesis_finalized(&self) -> Result<(), TairaAuthorityErrorV1> {
        if self.role != TairaAuthorityRoleV1::PrivacyGovernance {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let binding = self.public_binding()?;
        let binding_sha256 = binding
            .sha256()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        let provenance = self.provenance()?;
        if provenance.audit_sequence > 2
            || (provenance.audit_sequence == 1
                && provenance.audit_head != binding.signer.audit_genesis_digest)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let mut transition = Map::new();
        transition.insert(
            "schema".into(),
            Value::from("iroha.taira.FinalizePrivacyGenesisV1"),
        );
        transition.insert("role".into(), Value::from("privacy-governance"));
        transition.insert(
            "binding_sha256".into(),
            Value::from(hex::encode(binding_sha256)),
        );
        let transition = canonical_json_line(&Value::Object(transition))?;
        let signing_payload = taira_signing_payload(&transition)?;
        let operation_id = digest_parts_sha256(
            PRIVACY_GENESIS_FINALIZATION_OPERATION_DOMAIN_V1,
            &[&binding_sha256],
        );
        let receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(operation_id, signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &signing_payload,
        )?;
        if receipt.commit_sequence != 2
            || receipt.commit_audit_head == binding.signer.audit_genesis_digest
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let finalized = FinalizePrivacyGenesisV1 {
            operation_id,
            binding_sha256,
            previous_audit_head: binding.signer.audit_genesis_digest,
            signing_payload,
            receipt,
        };
        persist_canonical_once(
            &self
                .state_directory
                .join(PRIVACY_GENESIS_FINALIZATION_DIRECTORY_V1),
            operation_id,
            &finalized,
        )
        .map_err(|()| TairaAuthorityErrorV1::State)?;
        let records = BTreeMap::from([(operation_id, finalized)]);
        self.verify_privacy_genesis_finalization(&records)
    }

    fn verify_privacy_genesis_finalization(
        &self,
        records: &BTreeMap<[u8; 32], FinalizePrivacyGenesisV1>,
    ) -> Result<(), TairaAuthorityErrorV1> {
        let [record] = records.values().collect::<Vec<_>>().as_slice() else {
            return Err(TairaAuthorityErrorV1::State);
        };
        let binding = self.public_binding()?;
        let binding_sha256 = binding
            .sha256()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        if record.operation_id
            != digest_parts_sha256(
                PRIVACY_GENESIS_FINALIZATION_OPERATION_DOMAIN_V1,
                &[&binding_sha256],
            )
            || record.binding_sha256 != binding_sha256
            || record.previous_audit_head != binding.signer.audit_genesis_digest
            || record.receipt.commit_sequence != 2
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        record
            .receipt
            .verify_offline(
                &record.receipt.provenance.binding,
                record.operation_id,
                &record.signing_payload,
                &record.receipt.signature,
            )
            .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        self.signer
            .verify_taira_journal_commit(
                record.operation_id,
                &record.signing_payload,
                &record.receipt,
            )
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if self
            .signer
            .taira_journal_commit_predecessor(
                record.operation_id,
                &record.signing_payload,
                &record.receipt,
            )
            .map_err(|_| TairaAuthorityErrorV1::State)?
            != record.previous_audit_head
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let live = self.provenance()?;
        if live.audit_sequence < record.receipt.commit_sequence
            || (live.audit_sequence == record.receipt.commit_sequence
                && live.audit_head != record.receipt.commit_audit_head)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        Ok(())
    }

    pub(super) fn assign_run_json(
        &self,
        assignment_json: &[u8],
        now_unix_millis: u64,
    ) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
        let assignment = parse_assignment(assignment_json, self.role)?;
        if assignment.issued_at_unix_millis > now_unix_millis
            || assignment.not_before_unix_millis < assignment.issued_at_unix_millis
            || assignment.expires_at_unix_millis <= assignment.not_before_unix_millis
            || assignment.expires_at_unix_millis - assignment.issued_at_unix_millis
                > MAX_RUN_LIFETIME_MILLIS_V1
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let binding = self.public_binding()?;
        if assignment.key_revision != binding.signer.key_revision
            || assignment.policy_revision != binding.signer.policy_revision
            || assignment.policy_digest != binding.signer.policy_digest
        {
            return Err(TairaAuthorityErrorV1::Conflict);
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if let Some(existing) = state.assignments.get(&assignment.run_id) {
            return if existing.assignment == assignment
                && existing.assignment_json == assignment_json
            {
                Ok(OperationResponseV1 {
                    status: OperationStatusV1::Replayed,
                    result_json: assignment_result_json(existing, true)?,
                })
            } else {
                Err(TairaAuthorityErrorV1::Conflict)
            };
        }
        let operation_id = digest_parts_sha256(ASSIGNMENT_SIGNING_DOMAIN_V1, &[&assignment.run_id]);
        let signing_payload = taira_signing_payload(assignment_json)?;
        let receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(operation_id, signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &signing_payload,
        )?;
        let stored = StoredRunAssignmentV1 {
            assignment,
            assignment_json: assignment_json.to_vec(),
            signing_payload,
            receipt,
        };
        persist_canonical_once(
            &self.state_directory.join(ASSIGNMENTS_DIRECTORY_V1),
            stored.assignment.run_id,
            &stored,
        )
        .map_err(|()| TairaAuthorityErrorV1::State)?;
        let result_json = assignment_result_json(&stored, false)?;
        state.assignments.insert(stored.assignment.run_id, stored);
        Ok(OperationResponseV1 {
            status: OperationStatusV1::Ok,
            result_json,
        })
    }

    pub(super) fn authorize_json(
        &self,
        request_json: &[u8],
        descriptors: Vec<OwnedFd>,
        authenticated_uid: u32,
        now_unix_millis: u64,
    ) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
        let request = parse_client_request(request_json, self.role)?;
        if request.deploy_disposition == Some(DeployDispositionV1::Finalize) {
            if !descriptors.is_empty() {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            return self.finalize_deployment(&request, now_unix_millis);
        }
        let governance_validation = if self.role == TairaAuthorityRoleV1::PrivacyGovernance {
            let subject = canonical_json_line(&request.subject)?;
            Some(
                privacy_governance::validate_assigned_privacy_governance_request_v1(
                    &subject,
                    authenticated_uid,
                    now_unix_millis,
                )
                .map_err(|_| TairaAuthorityErrorV1::Rejected)?,
            )
        } else {
            None
        };
        let mut artifacts =
            ValidatedArtifactsV1::new(descriptors, &request.manifest, authenticated_uid)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if let Some(existing) = state.authorizations.get(&request.operation_id) {
            if existing.consumption.request_sha256 != request.request_sha256
                || existing.consumption.run_id != request.run_id
            {
                return Err(TairaAuthorityErrorV1::Conflict);
            }
            artifacts.revalidate()?;
            verify_stored_authorization(&self.signer, self.role, existing)?;
            return Ok(OperationResponseV1 {
                status: OperationStatusV1::Replayed,
                result_json: authorization_result_json(existing, self.role, true)?,
            });
        }
        let assignment = state
            .assignments
            .get(&request.run_id)
            .ok_or(TairaAuthorityErrorV1::Rejected)?
            .assignment
            .clone();
        let binding = self.public_binding()?;
        if now_unix_millis < assignment.not_before_unix_millis
            || now_unix_millis >= assignment.expires_at_unix_millis
            || request.subject_sha256 != assignment.subject_sha256
            || request.manifest_sha256 != assignment.artifact_manifest_sha256
            || assignment.key_revision != binding.signer.key_revision
            || assignment.policy_revision != binding.signer.policy_revision
            || assignment.policy_digest != binding.signer.policy_digest
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        if request.deploy_disposition == Some(DeployDispositionV1::DryRun) {
            artifacts.revalidate()?;
            return Ok(OperationResponseV1 {
                status: OperationStatusV1::Ok,
                result_json: dry_run_result_json(&request)?,
            });
        }
        let role_result = match self.role {
            TairaAuthorityRoleV1::NativeEvidence => {
                Some(super::native_evidence::validate_native_evidence_v1(
                    &request.subject,
                    &request.manifest,
                    artifacts.files_mut(),
                )?)
            }
            TairaAuthorityRoleV1::Qualification => Some(
                super::sandbox::run_qualification_probes(artifacts.files_mut(), &request.manifest)?
                    .to_json_value(),
            ),
            _ => None,
        };
        let candidate_consumption = ReplayConsumptionV1 {
            run_id: request.run_id,
            operation_id: request.operation_id,
            request_sha256: request.request_sha256,
            subject_sha256: request.subject_sha256,
            artifact_manifest_sha256: request.manifest_sha256,
            consumed_at_unix_millis: now_unix_millis,
        };
        let existing_consumption = state.consumptions.get(&request.run_id).cloned();
        if let Some(existing) = &existing_consumption {
            if !same_consumption_request(existing, &candidate_consumption) {
                return Err(TairaAuthorityErrorV1::Conflict);
            }
        }
        let admitted_at_unix_millis = existing_consumption
            .as_ref()
            .map_or(now_unix_millis, |existing| existing.consumed_at_unix_millis);
        if self.role == TairaAuthorityRoleV1::PublicSoakReplayAdmission {
            // The broker must authenticate the independently signed observation
            // before the replay identifier is consumed durably.  Crash recovery
            // checks it at the already-recorded admission time.
            self.verify_public_soak_replay_observation(&request, admitted_at_unix_millis)?;
        }
        let consumption = if let Some(existing) = existing_consumption {
            existing
        } else {
            persist_canonical_once(
                &self.state_directory.join(CONSUMPTIONS_DIRECTORY_V1),
                request.run_id,
                &candidate_consumption,
            )
            .map_err(|()| TairaAuthorityErrorV1::State)?;
            state
                .consumptions
                .insert(request.run_id, candidate_consumption.clone());
            candidate_consumption
        };
        if let Some(validated) = governance_validation {
            let stored = self.issue_governance_transaction(
                &request,
                validated,
                consumption,
                authenticated_uid,
            )?;
            artifacts.revalidate()?;
            persist_canonical_once(
                &self.state_directory.join(RECEIPTS_DIRECTORY_V1),
                request.operation_id,
                &stored,
            )
            .map_err(|()| TairaAuthorityErrorV1::State)?;
            let result_json = authorization_result_json(&stored, self.role, false)?;
            state.authorizations.insert(request.operation_id, stored);
            return Ok(OperationResponseV1 {
                status: OperationStatusV1::Ok,
                result_json,
            });
        }
        if matches!(
            self.role,
            TairaAuthorityRoleV1::PublicSoakObservation
                | TairaAuthorityRoleV1::PublicSoakReplayAdmission
        ) {
            let stored = match self.role {
                TairaAuthorityRoleV1::PublicSoakObservation => self.issue_public_soak_observation(
                    &request,
                    consumption,
                    admitted_at_unix_millis,
                    assignment.expires_at_unix_millis,
                )?,
                TairaAuthorityRoleV1::PublicSoakReplayAdmission => self
                    .issue_public_soak_replay_admission(
                        &request,
                        consumption,
                        admitted_at_unix_millis,
                    )?,
                _ => unreachable!("matched public-soak roles"),
            };
            artifacts.revalidate()?;
            verify_stored_authorization(&self.signer, self.role, &stored)?;
            persist_canonical_once(
                &self.state_directory.join(RECEIPTS_DIRECTORY_V1),
                request.operation_id,
                &stored,
            )
            .map_err(|()| TairaAuthorityErrorV1::State)?;
            let result_json = authorization_result_json(&stored, self.role, false)?;
            state.authorizations.insert(request.operation_id, stored);
            return Ok(OperationResponseV1 {
                status: OperationStatusV1::Ok,
                result_json,
            });
        }
        let claims = envelope_claims_json(
            self.role,
            &request,
            assignment.issued_at_unix_millis,
            assignment.expires_at_unix_millis,
            role_result,
        )?;
        let envelope_signing_payload = taira_signing_payload(&claims)?;
        let envelope_receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(request.operation_id, envelope_signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &envelope_signing_payload,
        )?;
        let authority_envelope_json = authority_envelope_json(
            self.role,
            &claims,
            &envelope_receipt,
            &self.public_binding()?,
        )?;
        let receipt_claims = durable_receipt_claims_json(
            self.role,
            &request,
            &authority_envelope_json,
            admitted_at_unix_millis,
            &envelope_receipt,
        )?;
        let receipt_signing_payload = durable_receipt_signing_payload(&receipt_claims)?;
        let receipt_operation = digest_parts_sha256(
            RECEIPT_OPERATION_DOMAIN_V1,
            &[&request.operation_id, &request.run_id],
        );
        let durable_receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(receipt_operation, receipt_signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &receipt_signing_payload,
        )?;
        let durable_receipt_json = durable_receipt_json(
            self.role,
            &receipt_claims,
            &durable_receipt,
            &self.public_binding()?,
        )?;
        artifacts.revalidate()?;
        let stored = StoredAuthorizationV1 {
            consumption,
            request_json: request.canonical_request_json.clone(),
            admitted_at_unix_millis,
            authority_envelope_json,
            durable_receipt_json,
            envelope_signing_payload,
            envelope_receipt,
            receipt_signing_payload,
            durable_receipt,
        };
        verify_stored_authorization(&self.signer, self.role, &stored)?;
        persist_canonical_once(
            &self.state_directory.join(RECEIPTS_DIRECTORY_V1),
            request.operation_id,
            &stored,
        )
        .map_err(|()| TairaAuthorityErrorV1::State)?;
        let result_json = authorization_result_json(&stored, self.role, false)?;
        state.authorizations.insert(request.operation_id, stored);
        Ok(OperationResponseV1 {
            status: OperationStatusV1::Ok,
            result_json,
        })
    }

    fn finalize_deployment(
        &self,
        request: &ParsedClientRequestV1,
        now_unix_millis: u64,
    ) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
        if self.role != TairaAuthorityRoleV1::DeployIssuance {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let deployment_result = request
            .deployment_result
            .as_ref()
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let apply_envelope_json = {
            let applied = state
                .authorizations
                .get(&request.operation_id)
                .ok_or(TairaAuthorityErrorV1::Rejected)?;
            if applied.consumption.run_id != request.run_id
                || applied.consumption.request_sha256 != request.request_sha256
                || applied.consumption.subject_sha256 != request.subject_sha256
                || applied.consumption.artifact_manifest_sha256 != request.manifest_sha256
            {
                return Err(TairaAuthorityErrorV1::Conflict);
            }
            applied.authority_envelope_json.clone()
        };
        if let Some(existing) = state.deployment_finalizations.get(&request.operation_id) {
            return if existing.finalization_request_sha256 == request.wire_request_sha256
                && existing.outcome == deployment_result.outcome
                && existing.result_sha256 == deployment_result.result_sha256
            {
                Ok(OperationResponseV1 {
                    status: OperationStatusV1::Replayed,
                    result_json: replayed_finalization_result_json(existing)?,
                })
            } else {
                Err(TairaAuthorityErrorV1::Conflict)
            };
        }
        let signing_claims =
            deployment_finalization_claims_json(request, deployment_result, now_unix_millis)?;
        let signing_payload = taira_signing_payload(&signing_claims)?;
        let signing_operation = digest_parts_sha256(
            DEPLOYMENT_FINALIZATION_OPERATION_DOMAIN_V1,
            &[&request.operation_id],
        );
        let receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(signing_operation, signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &signing_payload,
        )?;
        let durable_receipt_json = durable_receipt_json(
            self.role,
            &signing_claims,
            &receipt,
            &self.public_binding()?,
        )?;
        let result_json = deployment_finalization_result_json(
            request.operation_id,
            &apply_envelope_json,
            &durable_receipt_json,
            false,
        )?;
        let stored = StoredDeploymentFinalizationV1 {
            operation_id: request.operation_id,
            apply_request_sha256: request.request_sha256,
            finalization_request_sha256: request.wire_request_sha256,
            outcome: deployment_result.outcome.clone(),
            result_sha256: deployment_result.result_sha256,
            finalized_at_unix_millis: now_unix_millis,
            signing_payload,
            receipt,
            result_json,
        };
        persist_canonical_once(
            &self
                .state_directory
                .join(DEPLOYMENT_FINALIZATIONS_DIRECTORY_V1),
            request.operation_id,
            &stored,
        )
        .map_err(|()| TairaAuthorityErrorV1::State)?;
        let result_json = stored.result_json.clone();
        state
            .deployment_finalizations
            .insert(request.operation_id, stored);
        Ok(OperationResponseV1 {
            status: OperationStatusV1::Ok,
            result_json,
        })
    }

    fn issue_governance_transaction(
        &self,
        request: &ParsedClientRequestV1,
        validated: privacy_governance::ValidatedPrivacyGovernanceRequestV1,
        consumption: ReplayConsumptionV1,
        authenticated_uid: u32,
    ) -> Result<StoredAuthorizationV1, TairaAuthorityErrorV1> {
        let subject = request
            .subject
            .as_object()
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let transaction = subject
            .get("transaction")
            .and_then(Value::as_object)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let genesis = subject
            .get("genesis")
            .and_then(Value::as_object)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let transaction_payload =
            decode_base64_standard(required_str(transaction, "payload_norito_base64")?)?;
        if sha256(&transaction_payload) != validated.transaction_payload_sha256 {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let previous = self.provenance()?;
        let envelope_receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(request.operation_id, transaction_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &transaction_payload,
        )?;
        if envelope_receipt.commit_sequence != previous.audit_sequence.saturating_add(1)
            || envelope_receipt.commit_audit_head == previous.audit_head
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let signature = Signature::try_from_bytes(&envelope_receipt.signature)
            .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        let signed = TransactionBuilder::decode_payload(&transaction_payload)
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?
            .build_with_signature(signature);
        signed
            .verify_signature()
            .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        let signed_transaction = signed.encode_versioned();
        let binding = self.public_binding()?;
        let authority_account_id = required_str(genesis, "authority_account_id")?;
        let authority_public_key = required_str(genesis, "public_key")?;
        if binding.signer.public_key.to_string() != authority_public_key {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let mut receipt = Map::new();
        receipt.insert(
            "schema".into(),
            Value::from("iroha.taira.privacy_governance_authority_receipt"),
        );
        receipt.insert("schema_version".into(), Value::from(1_u64));
        receipt.insert(
            "authority_envelope_schema".into(),
            Value::from("iroha.taira.privacy_governance_authority.v1"),
        );
        receipt.insert(
            "binding_schema".into(),
            Value::from("iroha.taira.privacy_governance_authority_binding"),
        );
        receipt.insert("status".into(), Value::from("signed"));
        receipt.insert(
            "request_id".into(),
            Value::from(hex::encode(validated.request_id)),
        );
        receipt.insert(
            "request_sha256".into(),
            Value::from(hex::encode(validated.request_sha256)),
        );
        receipt.insert(
            "replay_namespace".into(),
            Value::from("iroha.taira.privacy_governance_authority_replay.v1"),
        );
        receipt.insert(
            "administrator_uid".into(),
            Value::from(binding.signer.administrator_uid),
        );
        receipt.insert(
            "service_uid".into(),
            Value::from(binding.signer.service_uid),
        );
        receipt.insert("kernel_peer_uid".into(), Value::from(authenticated_uid));
        receipt.insert(
            "audit_sequence".into(),
            Value::from(envelope_receipt.commit_sequence),
        );
        receipt.insert(
            "key_revision".into(),
            Value::from(binding.signer.key_revision),
        );
        receipt.insert(
            "policy_revision".into(),
            Value::from(binding.signer.policy_revision),
        );
        receipt.insert(
            "audit_previous_head_sha256".into(),
            Value::from(hex::encode(previous.audit_head)),
        );
        receipt.insert(
            "audit_committed_head_sha256".into(),
            Value::from(hex::encode(envelope_receipt.commit_audit_head)),
        );
        receipt.insert(
            "audit_live_head_sha256".into(),
            Value::from(hex::encode(envelope_receipt.commit_audit_head)),
        );
        receipt.insert(
            "binding_sha256".into(),
            Value::from(hex::encode(
                binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
            )),
        );
        receipt.insert(
            "broker_binary_sha256".into(),
            Value::from(hex::encode(current_executable_sha256()?)),
        );
        receipt.insert(
            "operation_id".into(),
            Value::from(hex::encode(validated.operation_id)),
        );
        receipt.insert(
            "policy_sha256".into(),
            Value::from(hex::encode(binding.signer.policy_digest)),
        );
        receipt.insert(
            "authority_account_id".into(),
            Value::from(authority_account_id),
        );
        receipt.insert(
            "authority_public_key".into(),
            Value::from(authority_public_key),
        );
        receipt.insert(
            "service_id".into(),
            Value::from("taira-authority-privacy-governance-v1"),
        );
        receipt.insert("signer_role".into(), Value::from("privacy-governance"));
        receipt.insert(
            "signed_transaction_norito_base64".into(),
            Value::from(encode_base64_standard(&signed_transaction)),
        );
        receipt.insert(
            "signed_transaction_sha256".into(),
            Value::from(hex::encode(sha256(&signed_transaction))),
        );
        receipt.insert(
            "transaction_hash_hex".into(),
            Value::from(hex::encode(signed.hash().as_ref())),
        );
        receipt.insert(
            "response_attestation_base64".into(),
            Value::from(encode_base64_standard(
                &envelope_receipt.response_attestation,
            )),
        );
        receipt.insert(
            "response_attestation_sha256".into(),
            Value::from(hex::encode(sha256(&envelope_receipt.response_attestation))),
        );
        let durable_receipt_json = canonical_json_line(&Value::Object(receipt))?;
        let mut envelope_claims = Map::new();
        envelope_claims.insert(
            "request_id".into(),
            Value::from(hex::encode(validated.request_id)),
        );
        envelope_claims.insert(
            "transaction_payload_sha256".into(),
            Value::from(hex::encode(validated.transaction_payload_sha256)),
        );
        envelope_claims.insert(
            "signed_transaction_sha256".into(),
            Value::from(hex::encode(sha256(&signed_transaction))),
        );
        let mut envelope = Map::new();
        envelope.insert(
            "schema".into(),
            Value::from("iroha.taira.privacy_governance_authority.v1"),
        );
        envelope.insert("schema_version".into(), Value::from(1_u64));
        envelope.insert("role".into(), Value::from("privacy-governance"));
        envelope.insert("claims".into(), Value::Object(envelope_claims));
        envelope.insert("signature_algorithm".into(), Value::from("ed25519"));
        envelope.insert(
            "signature".into(),
            Value::from(hex::encode(&envelope_receipt.signature)),
        );
        envelope.insert(
            "audit_sequence".into(),
            Value::from(envelope_receipt.commit_sequence),
        );
        envelope.insert(
            "audit_head".into(),
            Value::from(hex::encode(envelope_receipt.commit_audit_head)),
        );
        let authority_envelope_json = canonical_json_line(&Value::Object(envelope))?;
        Ok(StoredAuthorizationV1 {
            consumption,
            request_json: request.canonical_request_json.clone(),
            admitted_at_unix_millis: validated.issued_at_unix_millis,
            authority_envelope_json,
            durable_receipt_json,
            envelope_signing_payload: transaction_payload,
            envelope_receipt: envelope_receipt.clone(),
            receipt_signing_payload: Vec::new(),
            durable_receipt: envelope_receipt,
        })
    }

    fn issue_public_soak_observation(
        &self,
        request: &ParsedClientRequestV1,
        consumption: ReplayConsumptionV1,
        issued_at_unix_millis: u64,
        assignment_expires_at_unix_millis: u64,
    ) -> Result<StoredAuthorizationV1, TairaAuthorityErrorV1> {
        let (completed_at, subject_digest) = validate_public_soak_observation_subject(request)?;
        if issued_at_unix_millis < completed_at
            || issued_at_unix_millis - completed_at > PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let expires_at = issued_at_unix_millis
            .checked_add(PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?
            .min(assignment_expires_at_unix_millis);
        if expires_at <= issued_at_unix_millis {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let binding = self.public_binding()?;
        let authority_key_id = hex::encode(binding.signer.public_key_digest);
        let mut claims = Map::new();
        claims.insert(
            "schema".into(),
            Value::from("iroha.taira.public-v2-24h-soak-authority-claims.v1"),
        );
        claims.insert(
            "subject_digest".into(),
            Value::from(hex::encode(subject_digest)),
        );
        claims.insert(
            "replay_namespace".into(),
            Value::from(PUBLIC_SOAK_REPLAY_NAMESPACE_V1),
        );
        claims.insert("replay_id".into(), Value::from(hex::encode(request.run_id)));
        claims.insert(
            "issued_at_unix_ms".into(),
            Value::from(issued_at_unix_millis),
        );
        claims.insert("expires_at_unix_ms".into(), Value::from(expires_at));
        let mut envelope = Map::new();
        envelope.insert(
            "schema".into(),
            Value::from("iroha.taira.public-v2-24h-soak-authority-envelope.v1"),
        );
        envelope.insert("schema_version".into(), Value::from(1_u64));
        envelope.insert("authority_key_id".into(), Value::from(authority_key_id));
        envelope.insert("signature_algorithm".into(), Value::from("ed25519"));
        envelope.insert("claims".into(), Value::Object(claims));
        let unsigned = canonical_json_line(&Value::Object(envelope.clone()))?;
        let mut message =
            Vec::with_capacity(PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1.len() + unsigned.len());
        message.extend_from_slice(PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&unsigned);
        let signing_payload = taira_validated_message_payload(
            self.role,
            "public-soak-observation-envelope-v1",
            &message,
        )?;
        let envelope_receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(request.operation_id, signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &signing_payload,
        )?;
        envelope.insert(
            "signature".into(),
            Value::from(hex::encode(&envelope_receipt.signature)),
        );
        let authority_envelope_json = canonical_json_line(&Value::Object(envelope))?;
        Ok(StoredAuthorizationV1 {
            consumption,
            request_json: request.canonical_request_json.clone(),
            admitted_at_unix_millis: issued_at_unix_millis,
            authority_envelope_json,
            durable_receipt_json: b"{}\n".to_vec(),
            envelope_signing_payload: signing_payload,
            envelope_receipt: envelope_receipt.clone(),
            receipt_signing_payload: Vec::new(),
            durable_receipt: envelope_receipt,
        })
    }

    fn issue_public_soak_replay_admission(
        &self,
        request: &ParsedClientRequestV1,
        consumption: ReplayConsumptionV1,
        admitted_at_unix_millis: u64,
    ) -> Result<StoredAuthorizationV1, TairaAuthorityErrorV1> {
        let observation_binding = self
            .public_soak_observation_binding
            .as_ref()
            .ok_or(TairaAuthorityErrorV1::Binding)?;
        let subject = request
            .subject
            .as_object()
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        if subject.len() != 6
            || [
                "authority_envelope",
                "authority_envelope_sha256",
                "completed_at_unix_ms",
                "replay_namespace",
                "subject",
                "subject_digest",
            ]
            .into_iter()
            .any(|field| !subject.contains_key(field))
            || required_str(subject, "replay_namespace")? != PUBLIC_SOAK_REPLAY_NAMESPACE_V1
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let envelope = subject
            .get("authority_envelope")
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let envelope_json = canonical_json_line(envelope)?;
        if sha256(&envelope_json) != required_digest(subject, "authority_envelope_sha256")? {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let completed_at = required_u64(subject, "completed_at_unix_ms")?;
        let subject_core = subject
            .get("subject")
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let subject_digest = validate_public_soak_subject_core(subject_core)?;
        if subject_digest != required_digest(subject, "subject_digest")? {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let observation = validate_public_soak_envelope(
            envelope,
            observation_binding,
            subject_digest,
            completed_at,
            admitted_at_unix_millis,
        )?;
        let binding = self.public_binding()?;
        if binding.signer.public_key_digest == observation_binding.signer.public_key_digest {
            return Err(TairaAuthorityErrorV1::Binding);
        }
        let receipt_id = digest_parts_sha256(
            b"iroha:taira:public-soak-durable-receipt-id:v1\0",
            &[&request.operation_id, &sha256(&envelope_json)],
        );
        let mut claims = Map::new();
        claims.insert(
            "schema".into(),
            Value::from("iroha.taira.public-v2-24h-soak-durable-admission-claims.v1"),
        );
        claims.insert("decision".into(), Value::from("admitted"));
        claims.insert("receipt_id".into(), Value::from(hex::encode(receipt_id)));
        claims.insert(
            "subject_digest".into(),
            Value::from(hex::encode(subject_digest)),
        );
        claims.insert(
            "authority_envelope_sha256".into(),
            Value::from(hex::encode(sha256(&envelope_json))),
        );
        claims.insert(
            "authority_key_id".into(),
            Value::from(observation.authority_key_id),
        );
        claims.insert(
            "replay_namespace".into(),
            Value::from(PUBLIC_SOAK_REPLAY_NAMESPACE_V1),
        );
        claims.insert("replay_id".into(), Value::from(observation.replay_id));
        claims.insert(
            "admitted_at_unix_ms".into(),
            Value::from(admitted_at_unix_millis),
        );
        let mut durable = Map::new();
        durable.insert(
            "schema".into(),
            Value::from("iroha.taira.public-v2-24h-soak-durable-admission-receipt.v1"),
        );
        durable.insert("schema_version".into(), Value::from(1_u64));
        durable.insert(
            "broker_key_id".into(),
            Value::from(hex::encode(binding.signer.public_key_digest)),
        );
        durable.insert("signature_algorithm".into(), Value::from("ed25519"));
        durable.insert("claims".into(), Value::Object(claims));
        let unsigned = canonical_json_line(&Value::Object(durable.clone()))?;
        let mut message =
            Vec::with_capacity(PUBLIC_SOAK_BROKER_SIGNATURE_DOMAIN_V1.len() + unsigned.len());
        message.extend_from_slice(PUBLIC_SOAK_BROKER_SIGNATURE_DOMAIN_V1);
        message.extend_from_slice(&unsigned);
        let signing_payload = taira_validated_message_payload(
            self.role,
            "public-soak-durable-admission-v1",
            &message,
        )?;
        let receipt = receipt_from_response(
            self.signer
                .sign_taira_payload(request.operation_id, signing_payload.clone())
                .map_err(|_| TairaAuthorityErrorV1::Crypto)?,
            &signing_payload,
        )?;
        durable.insert(
            "signature".into(),
            Value::from(hex::encode(&receipt.signature)),
        );
        Ok(StoredAuthorizationV1 {
            consumption,
            request_json: request.canonical_request_json.clone(),
            admitted_at_unix_millis,
            authority_envelope_json: envelope_json,
            durable_receipt_json: canonical_json_line(&Value::Object(durable))?,
            envelope_signing_payload: signing_payload,
            envelope_receipt: receipt.clone(),
            receipt_signing_payload: Vec::new(),
            durable_receipt: receipt,
        })
    }

    fn verify_public_soak_replay_observation(
        &self,
        request: &ParsedClientRequestV1,
        admitted_at_unix_millis: u64,
    ) -> Result<(), TairaAuthorityErrorV1> {
        let observation_binding = self
            .public_soak_observation_binding
            .as_ref()
            .ok_or(TairaAuthorityErrorV1::Binding)?;
        let subject = request
            .subject
            .as_object()
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        if subject.len() != 6
            || required_str(subject, "replay_namespace")? != PUBLIC_SOAK_REPLAY_NAMESPACE_V1
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let envelope = subject
            .get("authority_envelope")
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        let envelope_json = canonical_json_line(envelope)?;
        if sha256(&envelope_json) != required_digest(subject, "authority_envelope_sha256")? {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let subject_digest = validate_public_soak_subject_core(
            subject
                .get("subject")
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        )?;
        if subject_digest != required_digest(subject, "subject_digest")? {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        validate_public_soak_envelope(
            envelope,
            observation_binding,
            subject_digest,
            required_u64(subject, "completed_at_unix_ms")?,
            admitted_at_unix_millis,
        )?;
        Ok(())
    }

    pub(super) fn verify_json(
        &self,
        verification_json: &[u8],
        descriptors: Vec<OwnedFd>,
        authenticated_uid: u32,
    ) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
        let request = parse_verification_request(verification_json, self.role)?;
        let mut artifacts =
            ValidatedArtifactsV1::new(descriptors, &request.base.manifest, authenticated_uid)?;
        let state = self
            .state
            .lock()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let stored = state
            .authorizations
            .get(&request.base.operation_id)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        if stored.consumption.request_sha256 != request.base.request_sha256
            || stored.authority_envelope_json != request.authority_envelope_json
            || stored.durable_receipt_json != request.durable_receipt_json
        {
            return Err(TairaAuthorityErrorV1::Conflict);
        }
        if self.role == TairaAuthorityRoleV1::PublicSoakReplayAdmission {
            // Historical verification revalidates both signatures at the
            // recorded admission time and deliberately does not mutate replay
            // state.
            self.verify_public_soak_replay_observation(
                &request.base,
                stored.admitted_at_unix_millis,
            )?;
        }
        verify_stored_authorization(&self.signer, self.role, stored)?;
        artifacts.revalidate()?;
        Ok(OperationResponseV1 {
            status: OperationStatusV1::Ok,
            result_json: verification_result_json(stored, self.role)?,
        })
    }

    pub(super) fn administer(
        &self,
        command: AuthorityAdminCommandV1,
        now_unix_millis: u64,
    ) -> Result<OperationResponseV1, TairaAuthorityErrorV1> {
        match command {
            AuthorityAdminCommandV1::AssignRun { assignment_json } => {
                self.assign_run_json(&assignment_json, now_unix_millis)
            }
            AuthorityAdminCommandV1::Status => Ok(OperationResponseV1 {
                status: OperationStatusV1::Ok,
                result_json: self.status_json()?,
            }),
            AuthorityAdminCommandV1::Rotate {
                operation_id,
                expected_audit_head,
                expected_key_revision,
                new_key_revision,
                new_policy_revision,
                new_policy_digest,
            } => {
                if self.privacy_genesis_finalized {
                    return Err(TairaAuthorityErrorV1::Conflict);
                }
                let command = AuthorityAdminCommandV1::Rotate {
                    operation_id,
                    expected_audit_head,
                    expected_key_revision,
                    new_key_revision,
                    new_policy_revision,
                    new_policy_digest,
                };
                if let Some(existing) = self
                    .state
                    .lock()
                    .map_err(|_| TairaAuthorityErrorV1::State)?
                    .rotation_handoffs
                    .get(&operation_id)
                    .cloned()
                {
                    if existing.command != command {
                        return Err(TairaAuthorityErrorV1::Conflict);
                    }
                    return Ok(OperationResponseV1 {
                        status: OperationStatusV1::Replayed,
                        result_json: existing.result_json,
                    });
                }
                let previous_binding = self.public_binding()?;
                let signer_command = AdminCommandV1::Rotate {
                    operation_id,
                    expected_audit_head,
                    expected_key_revision,
                    new_key_revision,
                    new_policy_revision,
                    new_policy_digest,
                    algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
                };
                self.forward_admin(signer_command)?;
                let successor = self
                    .signer
                    .taira_rotation_successor(&previous_binding.signer, operation_id)
                    .map_err(|_| TairaAuthorityErrorV1::State)?;
                let stored = stored_rotation_handoff(previous_binding, successor)?;
                if stored.command != command {
                    return Err(TairaAuthorityErrorV1::State);
                }
                persist_canonical_once(
                    &self.state_directory.join(ROTATION_HANDOFFS_DIRECTORY_V1),
                    operation_id,
                    &stored,
                )
                .map_err(|()| TairaAuthorityErrorV1::State)?;
                let result_json = stored.result_json.clone();
                self.state
                    .lock()
                    .map_err(|_| TairaAuthorityErrorV1::State)?
                    .rotation_handoffs
                    .insert(operation_id, stored);
                Ok(OperationResponseV1 {
                    status: OperationStatusV1::Ok,
                    result_json,
                })
            }
            AuthorityAdminCommandV1::Revoke {
                operation_id,
                expected_audit_head,
                expected_key_revision,
                reason_digest,
            } => {
                self.forward_admin(AdminCommandV1::Revoke {
                    operation_id,
                    expected_audit_head,
                    expected_key_revision,
                    reason_digest,
                })?;
                Ok(OperationResponseV1 {
                    status: OperationStatusV1::Ok,
                    result_json: self.status_json()?,
                })
            }
        }
    }

    pub(super) fn status_json(&self) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
        let binding = self.public_binding()?;
        validate_qualification_service_identity(self.role, binding.signer.service_uid)?;
        let provenance = self.provenance()?;
        let mut object = Map::new();
        object.insert(
            "schema".into(),
            Value::from("iroha.taira.authority-client-status.v1"),
        );
        object.insert("role".into(), Value::from(self.role.as_str()));
        object.insert(
            "service_id".into(),
            Value::from(binding.signer.service_id.clone()),
        );
        object.insert(
            "administrator_id".into(),
            Value::from(binding.signer.administrator_id.clone()),
        );
        object.insert(
            "status".into(),
            Value::from(if provenance.revoked {
                "revoked"
            } else {
                "ready"
            }),
        );
        object.insert(
            "binding_sha256".into(),
            Value::from(hex::encode(
                binding
                    .sha256()
                    .map_err(|()| TairaAuthorityErrorV1::Binding)?,
            )),
        );
        object.insert(
            "key_revision".into(),
            Value::from(binding.signer.key_revision),
        );
        object.insert(
            "policy_revision".into(),
            Value::from(binding.signer.policy_revision),
        );
        object.insert(
            "audit_sequence".into(),
            Value::from(provenance.audit_sequence),
        );
        object.insert(
            "audit_head".into(),
            Value::from(hex::encode(provenance.audit_head)),
        );
        object.insert("revoked".into(), Value::from(provenance.revoked));
        canonical_json_line(&Value::Object(object))
    }

    fn forward_admin(&self, command: AdminCommandV1) -> Result<(), TairaAuthorityErrorV1> {
        let binding = self
            .signer
            .public_binding()
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        let binding_digest = binding
            .digest()
            .map_err(|()| TairaAuthorityErrorV1::Binding)?;
        let request_digest = admin_request_digest(binding_digest, &command)
            .map_err(|()| TairaAuthorityErrorV1::Rejected)?;
        let response = self
            .signer
            .handle_admin_request(&AdminRequestV1 {
                binding_digest,
                command,
                request_digest,
            })
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if !matches!(response.status, AdminStatusV1::Ok | AdminStatusV1::Replayed) {
            return Err(TairaAuthorityErrorV1::Conflict);
        }
        Ok(())
    }
}

fn rotation_operation_id(
    command: &AuthorityAdminCommandV1,
) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    match command {
        AuthorityAdminCommandV1::Rotate { operation_id, .. } => Ok(*operation_id),
        _ => Err(TairaAuthorityErrorV1::State),
    }
}

fn stored_rotation_handoff(
    previous_binding: TairaAuthorityPublicBindingV1,
    successor: SoftwareSignerRotationSuccessorV1,
) -> Result<StoredRotationHandoffV1, TairaAuthorityErrorV1> {
    let successor_binding = TairaAuthorityPublicBindingV1 {
        magic: TAIRA_AUTHORITY_BINDING_MAGIC_V1,
        version: TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
        role: previous_binding.role,
        signer: successor.successor.clone(),
    };
    previous_binding
        .validate()
        .and_then(|()| successor_binding.validate())
        .map_err(|()| TairaAuthorityErrorV1::Binding)?;
    let command = AuthorityAdminCommandV1::Rotate {
        operation_id: successor.operation_id,
        expected_audit_head: successor.predecessor_audit_head,
        expected_key_revision: previous_binding.signer.key_revision,
        new_key_revision: successor_binding.signer.key_revision,
        new_policy_revision: successor_binding.signer.policy_revision,
        new_policy_digest: successor_binding.signer.policy_digest,
    };
    let signer_command = AdminCommandV1::Rotate {
        operation_id: successor.operation_id,
        expected_audit_head: successor.predecessor_audit_head,
        expected_key_revision: previous_binding.signer.key_revision,
        new_key_revision: successor_binding.signer.key_revision,
        new_policy_revision: successor_binding.signer.policy_revision,
        new_policy_digest: successor_binding.signer.policy_digest,
        algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
    };
    if successor.request_digest
        != admin_request_digest(
            previous_binding
                .signer
                .digest()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
            &signer_command,
        )
        .map_err(|()| TairaAuthorityErrorV1::Binding)?
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let successor_binding_bytes =
        norito::encode_canonical(&successor_binding).map_err(|_| TairaAuthorityErrorV1::State)?;
    let mut result = Map::new();
    result.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-rotation-handoff.v1"),
    );
    result.insert("role".into(), Value::from(previous_binding.role.as_str()));
    result.insert("status".into(), Value::from("successor-ready"));
    result.insert(
        "operation_id".into(),
        Value::from(hex::encode(successor.operation_id)),
    );
    result.insert(
        "expected_audit_head".into(),
        Value::from(hex::encode(successor.predecessor_audit_head)),
    );
    result.insert(
        "expected_key_revision".into(),
        Value::from(previous_binding.signer.key_revision),
    );
    result.insert(
        "new_key_revision".into(),
        Value::from(successor_binding.signer.key_revision),
    );
    result.insert(
        "new_policy_revision".into(),
        Value::from(successor_binding.signer.policy_revision),
    );
    result.insert(
        "new_policy_sha256".into(),
        Value::from(hex::encode(successor_binding.signer.policy_digest)),
    );
    result.insert(
        "previous_binding_sha256".into(),
        Value::from(hex::encode(
            previous_binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    result.insert(
        "successor_binding_sha256".into(),
        Value::from(hex::encode(
            successor_binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    result.insert(
        "successor_binding_norito_base64".into(),
        Value::from(encode_base64_standard(&successor_binding_bytes)),
    );
    result.insert(
        "journal_record_norito_base64".into(),
        Value::from(encode_base64_standard(&successor.journal_record)),
    );
    result.insert("audit_sequence".into(), Value::from(successor.sequence));
    result.insert(
        "committed_audit_head".into(),
        Value::from(hex::encode(successor.audit_head)),
    );
    Ok(StoredRotationHandoffV1 {
        command,
        previous_binding,
        successor_binding,
        journal_record: successor.journal_record,
        result_json: canonical_json_line(&Value::Object(result))?,
    })
}

pub(super) fn verify_rotation_handoff_json(
    previous_binding: &TairaAuthorityPublicBindingV1,
    handoff_json: &[u8],
) -> Result<TairaAuthorityPublicBindingV1, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(handoff_json)?;
    let object = exact_object(
        &value,
        &[
            "audit_sequence",
            "committed_audit_head",
            "expected_audit_head",
            "expected_key_revision",
            "journal_record_norito_base64",
            "new_key_revision",
            "new_policy_revision",
            "new_policy_sha256",
            "operation_id",
            "previous_binding_sha256",
            "role",
            "schema",
            "status",
            "successor_binding_norito_base64",
            "successor_binding_sha256",
        ],
    )?;
    let journal_record =
        decode_base64_standard(required_str(object, "journal_record_norito_base64")?)?;
    let successor =
        verify_software_signer_rotation_successor(&previous_binding.signer, &journal_record)
            .map_err(|_| TairaAuthorityErrorV1::Binding)?;
    let expected = stored_rotation_handoff(previous_binding.clone(), successor)?;
    if expected.result_json != handoff_json {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let encoded_successor =
        decode_base64_standard(required_str(object, "successor_binding_norito_base64")?)?;
    let decoded_successor: TairaAuthorityPublicBindingV1 =
        norito::decode_canonical(&encoded_successor).map_err(|_| TairaAuthorityErrorV1::Binding)?;
    if norito::encode_canonical(&decoded_successor).map_err(|_| TairaAuthorityErrorV1::Binding)?
        != encoded_successor
        || decoded_successor != expected.successor_binding
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(decoded_successor)
}

pub(super) fn rotation_handoff_matches_installed_successor(
    handoff_json: &[u8],
    installed: &TairaAuthorityPublicBindingV1,
) -> bool {
    let Ok(value) = parse_canonical_json(handoff_json) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    let Some(encoded) = object
        .get("successor_binding_norito_base64")
        .and_then(Value::as_str)
        .and_then(|value| decode_base64_standard(value).ok())
    else {
        return false;
    };
    let Ok(decoded) = norito::decode_canonical::<TairaAuthorityPublicBindingV1>(&encoded) else {
        return false;
    };
    norito::encode_canonical(&decoded).ok().as_deref() == Some(encoded.as_slice())
        && &decoded == installed
        && object.get("role").and_then(Value::as_str) == Some(installed.role.as_str())
        && object.get("status").and_then(Value::as_str) == Some("successor-ready")
        && object
            .get("successor_binding_sha256")
            .and_then(Value::as_str)
            == installed.sha256().ok().map(hex::encode).as_deref()
}

pub(super) fn now_unix_millis() -> Result<u64, TairaAuthorityErrorV1> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TairaAuthorityErrorV1::State)?
        .as_millis();
    u64::try_from(millis).map_err(|_| TairaAuthorityErrorV1::State)
}

fn current_executable_sha256() -> Result<[u8; 32], TairaAuthorityErrorV1> {
    const MAX_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
    let path = std::env::current_exe().map_err(|_| TairaAuthorityErrorV1::State)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let before = file.metadata().map_err(|_| TairaAuthorityErrorV1::State)?;
    if !before.is_file()
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > MAX_EXECUTABLE_BYTES
        || before.mode() & 0o022 != 0
        || (before.uid() != 0 && before.uid() != rustix::process::geteuid().as_raw())
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    let identity = artifact_identity(&before);
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest).map_err(|_| TairaAuthorityErrorV1::State)?;
    let after = file.metadata().map_err(|_| TairaAuthorityErrorV1::State)?;
    if artifact_identity(&after) != identity {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    Ok(digest.finalize().into())
}

fn encode_base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from((first & 0x03) << 4 | second >> 4)],
        ));
        output.push(if chunk.len() > 1 {
            char::from(ALPHABET[usize::from((second & 0x0f) << 2 | third >> 6)])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(ALPHABET[usize::from(third & 0x3f)])
        } else {
            '='
        });
    }
    output
}

fn decode_base64_standard(value: &str) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    if value.is_empty() || value.len() % 4 != 0 || !value.is_ascii() {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let decode = |byte: u8| -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    for (index, chunk) in value.as_bytes().chunks_exact(4).enumerate() {
        let final_chunk = index + 1 == value.len() / 4;
        let a = decode(chunk[0]).ok_or(TairaAuthorityErrorV1::Rejected)?;
        let b = decode(chunk[1]).ok_or(TairaAuthorityErrorV1::Rejected)?;
        let c = if chunk[2] == b'=' {
            if !final_chunk || chunk[3] != b'=' {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            0
        } else {
            decode(chunk[2]).ok_or(TairaAuthorityErrorV1::Rejected)?
        };
        let d = if chunk[3] == b'=' {
            if !final_chunk {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            0
        } else {
            decode(chunk[3]).ok_or(TairaAuthorityErrorV1::Rejected)?
        };
        output.push(a << 2 | b >> 4);
        if chunk[2] != b'=' {
            output.push(b << 4 | c >> 2);
        }
        if chunk[3] != b'=' {
            output.push(c << 6 | d);
        }
    }
    if encode_base64_standard(&output) != value {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(output)
}

fn receipt_from_response(
    response: SignResponseV1,
    payload: &[u8],
) -> Result<SoftwareSignerSignatureReceiptV1, TairaAuthorityErrorV1> {
    let replayed = match response.status {
        SignStatusV1::Ok => false,
        SignStatusV1::Replayed => true,
        _ => return Err(TairaAuthorityErrorV1::Conflict),
    };
    Ok(SoftwareSignerSignatureReceiptV1 {
        operation_id: response.operation_id,
        request_digest: response.request_digest,
        payload_digest: response.payload_digest,
        payload_length: u64::try_from(payload.len())
            .map_err(|_| TairaAuthorityErrorV1::Rejected)?,
        signature: response.signature,
        commit_sequence: response.commit_sequence,
        commit_audit_head: response.commit_audit_head,
        replayed,
        provenance: response.provenance,
        response_digest: response.response_digest,
        response_attestation: response.response_attestation,
    })
}

fn verify_stored_authorization(
    signer: &SoftwareSignerServiceV1,
    expected_role: TairaAuthorityRoleV1,
    stored: &StoredAuthorizationV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let request = parse_client_request(&stored.request_json, expected_role)?;
    if request.operation_id != stored.consumption.operation_id
        || request.run_id != stored.consumption.run_id
        || request.request_sha256 != stored.consumption.request_sha256
        || request.subject_sha256 != stored.consumption.subject_sha256
        || request.manifest_sha256 != stored.consumption.artifact_manifest_sha256
        || (expected_role == TairaAuthorityRoleV1::DeployIssuance
            && request.deploy_disposition != Some(DeployDispositionV1::Apply))
    {
        return Err(TairaAuthorityErrorV1::State);
    }
    let SoftwareSignerPurposeBindingV1::TairaAuthority { role } =
        &stored.envelope_receipt.provenance.binding.purpose_binding
    else {
        return Err(TairaAuthorityErrorV1::Binding);
    };
    if role != expected_role.as_str()
        || stored.envelope_receipt.provenance.binding.role != SoftwareSignerRoleV1::TairaAuthority
        || stored.durable_receipt.provenance.binding != stored.envelope_receipt.provenance.binding
    {
        return Err(TairaAuthorityErrorV1::Binding);
    }
    stored
        .envelope_receipt
        .verify_offline(
            &stored.envelope_receipt.provenance.binding,
            stored.consumption.operation_id,
            &stored.envelope_signing_payload,
            &stored.envelope_receipt.signature,
        )
        .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
    signer
        .verify_taira_journal_commit(
            stored.consumption.operation_id,
            &stored.envelope_signing_payload,
            &stored.envelope_receipt,
        )
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if matches!(
        role.as_str(),
        "privacy-governance" | "public-soak-observation" | "public-soak-replay-admission"
    ) {
        if !stored.receipt_signing_payload.is_empty()
            || stored.durable_receipt != stored.envelope_receipt
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        if expected_role == TairaAuthorityRoleV1::PrivacyGovernance {
            verify_governance_single_commit_sidecars(signer, &request, stored)?;
        } else {
            verify_public_soak_single_commit_sidecars(expected_role, &request, stored)?;
        }
        return Ok(());
    }
    let receipt_operation = digest_parts_sha256(
        RECEIPT_OPERATION_DOMAIN_V1,
        &[&stored.consumption.operation_id, &stored.consumption.run_id],
    );
    stored
        .durable_receipt
        .verify_offline(
            &stored.durable_receipt.provenance.binding,
            receipt_operation,
            &stored.receipt_signing_payload,
            &stored.durable_receipt.signature,
        )
        .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
    signer
        .verify_taira_journal_commit(
            receipt_operation,
            &stored.receipt_signing_payload,
            &stored.durable_receipt,
        )
        .map_err(|_| TairaAuthorityErrorV1::State)
}

fn verify_governance_single_commit_sidecars(
    signer: &SoftwareSignerServiceV1,
    request: &ParsedClientRequestV1,
    stored: &StoredAuthorizationV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let binding = &stored.envelope_receipt.provenance.binding;
    let subject_json = canonical_json_line(&request.subject)?;
    let validated = privacy_governance::validate_assigned_privacy_governance_request_v1(
        &subject_json,
        binding.client_uid,
        stored.admitted_at_unix_millis,
    )
    .map_err(|_| TairaAuthorityErrorV1::State)?;
    if sha256(&stored.envelope_signing_payload) != validated.transaction_payload_sha256
        || stored.admitted_at_unix_millis != validated.issued_at_unix_millis
    {
        return Err(TairaAuthorityErrorV1::State);
    }

    let builder = TransactionBuilder::decode_payload(&stored.envelope_signing_payload)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let authority_account_id = builder.payload().authority.to_string();
    let signature = Signature::try_from_bytes(&stored.envelope_receipt.signature)
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let signed = builder.build_with_signature(signature);
    signed
        .verify_signature()
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    let signed_transaction = signed.encode_versioned();
    let signed_transaction_sha256 = sha256(&signed_transaction);

    let receipt_value = parse_canonical_json(&stored.durable_receipt_json)?;
    let receipt = exact_object(
        &receipt_value,
        &[
            "administrator_uid",
            "audit_committed_head_sha256",
            "audit_live_head_sha256",
            "audit_previous_head_sha256",
            "audit_sequence",
            "authority_envelope_schema",
            "authority_account_id",
            "authority_public_key",
            "binding_schema",
            "binding_sha256",
            "broker_binary_sha256",
            "kernel_peer_uid",
            "key_revision",
            "operation_id",
            "policy_revision",
            "policy_sha256",
            "replay_namespace",
            "request_id",
            "request_sha256",
            "response_attestation_base64",
            "response_attestation_sha256",
            "schema",
            "schema_version",
            "service_id",
            "service_uid",
            "signer_role",
            "signed_transaction_norito_base64",
            "signed_transaction_sha256",
            "status",
            "transaction_hash_hex",
        ],
    )?;
    let authority_binding = TairaAuthorityPublicBindingV1 {
        magic: TAIRA_AUTHORITY_BINDING_MAGIC_V1,
        version: TAIRA_AUTHORITY_PROTOCOL_VERSION_V1,
        role: TairaAuthorityRoleV1::PrivacyGovernance,
        signer: binding.clone(),
    };
    let previous_audit_head = signer
        .taira_journal_commit_predecessor(
            stored.consumption.operation_id,
            &stored.envelope_signing_payload,
            &stored.envelope_receipt,
        )
        .map_err(|_| TairaAuthorityErrorV1::State)?;
    if required_str(receipt, "schema")? != "iroha.taira.privacy_governance_authority_receipt"
        || required_u64(receipt, "schema_version")? != 1
        || required_str(receipt, "authority_envelope_schema")?
            != "iroha.taira.privacy_governance_authority.v1"
        || required_str(receipt, "binding_schema")?
            != "iroha.taira.privacy_governance_authority_binding"
        || required_str(receipt, "status")? != "signed"
        || required_str(receipt, "replay_namespace")?
            != "iroha.taira.privacy_governance_authority_replay.v1"
        || required_str(receipt, "service_id")? != "taira-authority-privacy-governance-v1"
        || required_str(receipt, "signer_role")? != "privacy-governance"
        || required_digest(receipt, "request_id")? != validated.request_id
        || required_digest(receipt, "request_sha256")? != validated.request_sha256
        || required_digest(receipt, "operation_id")? != validated.operation_id
        || required_digest(receipt, "signed_transaction_sha256")? != signed_transaction_sha256
        || required_digest(receipt, "policy_sha256")? != binding.policy_digest
        || required_digest(receipt, "binding_sha256")?
            != authority_binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::State)?
        || required_digest(receipt, "audit_previous_head_sha256")? != previous_audit_head
        || required_digest(receipt, "audit_committed_head_sha256")?
            != stored.envelope_receipt.commit_audit_head
        || required_digest(receipt, "audit_live_head_sha256")?
            != stored.envelope_receipt.commit_audit_head
        || required_u64(receipt, "audit_sequence")? != stored.envelope_receipt.commit_sequence
        || required_u64(receipt, "key_revision")? != binding.key_revision
        || required_u64(receipt, "policy_revision")? != binding.policy_revision
        || required_u64(receipt, "service_uid")? != u64::from(binding.service_uid)
        || required_u64(receipt, "administrator_uid")? != u64::from(binding.administrator_uid)
        || required_nonnegative_u64(receipt, "kernel_peer_uid")? != u64::from(binding.client_uid)
        || required_str(receipt, "authority_account_id")? != authority_account_id
        || required_str(receipt, "authority_public_key")? != binding.public_key.to_string()
        || required_str(receipt, "signed_transaction_norito_base64")?
            != encode_base64_standard(&signed_transaction)
        || required_str(receipt, "transaction_hash_hex")? != hex::encode(signed.hash().as_ref())
        || required_str(receipt, "response_attestation_base64")?
            != encode_base64_standard(&stored.envelope_receipt.response_attestation)
        || required_digest(receipt, "response_attestation_sha256")?
            != sha256(&stored.envelope_receipt.response_attestation)
    {
        return Err(TairaAuthorityErrorV1::State);
    }

    let envelope_value = parse_canonical_json(&stored.authority_envelope_json)?;
    let envelope = exact_object(
        &envelope_value,
        &[
            "audit_head",
            "audit_sequence",
            "claims",
            "role",
            "schema",
            "schema_version",
            "signature",
            "signature_algorithm",
        ],
    )?;
    let claims = exact_object(
        envelope.get("claims").ok_or(TairaAuthorityErrorV1::State)?,
        &[
            "request_id",
            "signed_transaction_sha256",
            "transaction_payload_sha256",
        ],
    )?;
    if required_str(envelope, "schema")? != "iroha.taira.privacy_governance_authority.v1"
        || required_u64(envelope, "schema_version")? != 1
        || required_str(envelope, "role")? != "privacy-governance"
        || required_str(envelope, "signature_algorithm")? != "ed25519"
        || required_str(envelope, "signature")? != hex::encode(&stored.envelope_receipt.signature)
        || required_u64(envelope, "audit_sequence")? != stored.envelope_receipt.commit_sequence
        || required_digest(envelope, "audit_head")? != stored.envelope_receipt.commit_audit_head
        || required_digest(claims, "request_id")? != validated.request_id
        || required_digest(claims, "transaction_payload_sha256")?
            != validated.transaction_payload_sha256
        || required_digest(claims, "signed_transaction_sha256")? != signed_transaction_sha256
    {
        return Err(TairaAuthorityErrorV1::State);
    }
    // The executable digest is not a caller-selected authority, but it must
    // remain a syntactically valid nonzero commitment in historical receipts.
    required_digest(receipt, "broker_binary_sha256")?;
    Ok(())
}

fn verify_public_soak_single_commit_sidecars(
    role: TairaAuthorityRoleV1,
    request: &ParsedClientRequestV1,
    stored: &StoredAuthorizationV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let (signed_document, signature_purpose, signature_domain) = match role {
        TairaAuthorityRoleV1::PublicSoakObservation => {
            if stored.durable_receipt_json != b"{}\n" {
                return Err(TairaAuthorityErrorV1::State);
            }
            (
                parse_canonical_json(&stored.authority_envelope_json)?,
                "public-soak-observation-envelope-v1",
                PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1,
            )
        }
        TairaAuthorityRoleV1::PublicSoakReplayAdmission => (
            parse_canonical_json(&stored.durable_receipt_json)?,
            "public-soak-durable-admission-v1",
            PUBLIC_SOAK_BROKER_SIGNATURE_DOMAIN_V1,
        ),
        _ => return Err(TairaAuthorityErrorV1::State),
    };
    let mut signed_object = signed_document
        .as_object()
        .ok_or(TairaAuthorityErrorV1::State)?
        .clone();
    let signature = signed_object
        .remove("signature")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(TairaAuthorityErrorV1::State)?;
    if signature != hex::encode(&stored.envelope_receipt.signature) {
        return Err(TairaAuthorityErrorV1::State);
    }
    let unsigned = canonical_json_line(&Value::Object(signed_object))?;
    let mut message = Vec::with_capacity(signature_domain.len() + unsigned.len());
    message.extend_from_slice(signature_domain);
    message.extend_from_slice(&unsigned);
    if stored.envelope_signing_payload
        != taira_validated_message_payload(role, signature_purpose, &message)?
    {
        return Err(TairaAuthorityErrorV1::State);
    }
    if role == TairaAuthorityRoleV1::PublicSoakReplayAdmission {
        let request_subject = request
            .subject
            .as_object()
            .ok_or(TairaAuthorityErrorV1::State)?;
        let stored_envelope = parse_canonical_json(&stored.authority_envelope_json)?;
        if request_subject.get("authority_envelope") != Some(&stored_envelope)
            || required_digest(request_subject, "authority_envelope_sha256")?
                != sha256(&stored.authority_envelope_json)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let durable = signed_document
            .as_object()
            .ok_or(TairaAuthorityErrorV1::State)?;
        let durable_claims = durable
            .get("claims")
            .and_then(Value::as_object)
            .ok_or(TairaAuthorityErrorV1::State)?;
        if required_digest(durable_claims, "authority_envelope_sha256")?
            != sha256(&stored.authority_envelope_json)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        let envelope = parse_canonical_json(&stored.authority_envelope_json)?;
        let envelope = envelope.as_object().ok_or(TairaAuthorityErrorV1::State)?;
        let envelope_claims = envelope
            .get("claims")
            .and_then(Value::as_object)
            .ok_or(TairaAuthorityErrorV1::State)?;
        for field in ["authority_key_id", "replay_id", "subject_digest"] {
            let envelope_value = if field == "authority_key_id" {
                envelope.get(field)
            } else {
                envelope_claims.get(field)
            };
            if durable_claims.get(field) != envelope_value {
                return Err(TairaAuthorityErrorV1::State);
            }
        }
        if required_u64(durable_claims, "admitted_at_unix_ms")? != stored.admitted_at_unix_millis {
            return Err(TairaAuthorityErrorV1::State);
        }
    } else {
        let (_, subject_digest) = validate_public_soak_observation_subject(request)?;
        let envelope = signed_document
            .as_object()
            .ok_or(TairaAuthorityErrorV1::State)?;
        let claims = envelope
            .get("claims")
            .and_then(Value::as_object)
            .ok_or(TairaAuthorityErrorV1::State)?;
        if required_digest(claims, "subject_digest")? != subject_digest
            || required_u64(claims, "issued_at_unix_ms")? != stored.admitted_at_unix_millis
        {
            return Err(TairaAuthorityErrorV1::State);
        }
    }
    Ok(())
}

fn same_consumption_request(left: &ReplayConsumptionV1, right: &ReplayConsumptionV1) -> bool {
    left.run_id == right.run_id
        && left.operation_id == right.operation_id
        && left.request_sha256 == right.request_sha256
        && left.subject_sha256 == right.subject_sha256
        && left.artifact_manifest_sha256 == right.artifact_manifest_sha256
}

fn validate_recovered_state(
    role: TairaAuthorityRoleV1,
    signer: &SoftwareSignerServiceV1,
    assignments: &BTreeMap<[u8; 32], StoredRunAssignmentV1>,
    consumptions: &BTreeMap<[u8; 32], ReplayConsumptionV1>,
    authorizations: &BTreeMap<[u8; 32], StoredAuthorizationV1>,
    deployment_finalizations: &BTreeMap<[u8; 32], StoredDeploymentFinalizationV1>,
    rotation_handoffs: &BTreeMap<[u8; 32], StoredRotationHandoffV1>,
) -> Result<(), TairaAuthorityErrorV1> {
    for (run_id, assignment) in assignments {
        if assignment.assignment.role != role || assignment.assignment.run_id != *run_id {
            return Err(TairaAuthorityErrorV1::State);
        }
        assignment
            .receipt
            .verify_offline(
                &assignment.receipt.provenance.binding,
                digest_parts_sha256(ASSIGNMENT_SIGNING_DOMAIN_V1, &[run_id]),
                &assignment.signing_payload,
                &assignment.receipt.signature,
            )
            .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        signer
            .verify_taira_journal_commit(
                digest_parts_sha256(ASSIGNMENT_SIGNING_DOMAIN_V1, &[run_id]),
                &assignment.signing_payload,
                &assignment.receipt,
            )
            .map_err(|_| TairaAuthorityErrorV1::State)?;
    }
    for (run_id, consumption) in consumptions {
        if consumption.run_id != *run_id
            || consumption.consumed_at_unix_millis == 0
            || !assignments.contains_key(run_id)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
    }
    for (operation_id, authorization) in authorizations {
        if authorization.consumption.operation_id != *operation_id
            || consumptions.get(&authorization.consumption.run_id)
                != Some(&authorization.consumption)
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        verify_stored_authorization(signer, role, authorization)?;
    }
    if role != TairaAuthorityRoleV1::DeployIssuance && !deployment_finalizations.is_empty() {
        return Err(TairaAuthorityErrorV1::State);
    }
    for (operation_id, finalization) in deployment_finalizations {
        let authorization = authorizations
            .get(operation_id)
            .ok_or(TairaAuthorityErrorV1::State)?;
        if finalization.operation_id != *operation_id
            || finalization.apply_request_sha256 != authorization.consumption.request_sha256
            || finalization.finalization_request_sha256 == [0; 32]
            || finalization.result_sha256 == [0; 32]
            || !matches!(
                finalization.outcome.as_str(),
                "success" | "rolled-back" | "rollback-failed"
            )
        {
            return Err(TairaAuthorityErrorV1::State);
        }
        finalization
            .receipt
            .verify_offline(
                &finalization.receipt.provenance.binding,
                digest_parts_sha256(DEPLOYMENT_FINALIZATION_OPERATION_DOMAIN_V1, &[operation_id]),
                &finalization.signing_payload,
                &finalization.receipt.signature,
            )
            .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
        signer
            .verify_taira_journal_commit(
                digest_parts_sha256(DEPLOYMENT_FINALIZATION_OPERATION_DOMAIN_V1, &[operation_id]),
                &finalization.signing_payload,
                &finalization.receipt,
            )
            .map_err(|_| TairaAuthorityErrorV1::State)?;
        if parse_canonical_json(&finalization.result_json).is_err() {
            return Err(TairaAuthorityErrorV1::State);
        }
    }
    if role == TairaAuthorityRoleV1::PrivacyGovernance && !rotation_handoffs.is_empty() {
        return Err(TairaAuthorityErrorV1::State);
    }
    for (operation_id, handoff) in rotation_handoffs {
        if rotation_operation_id(&handoff.command)? != *operation_id
            || handoff.previous_binding.role != role
            || handoff.successor_binding.role != role
            || verify_rotation_handoff_json(&handoff.previous_binding, &handoff.result_json)?
                != handoff.successor_binding
        {
            return Err(TairaAuthorityErrorV1::State);
        }
    }
    Ok(())
}

struct ParsedClientRequestV1 {
    operation_id: [u8; 32],
    run_id: [u8; 32],
    request_sha256: [u8; 32],
    subject: Value,
    subject_sha256: [u8; 32],
    manifest_value: Value,
    manifest_sha256: [u8; 32],
    manifest: Vec<TairaAuthorityArtifactManifestEntryV1>,
    canonical_request_json: Vec<u8>,
    deploy_disposition: Option<DeployDispositionV1>,
    deployment_result: Option<DeploymentResultV1>,
    wire_request_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeployDispositionV1 {
    DryRun,
    Apply,
    Finalize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DeploymentResultV1 {
    outcome: String,
    result_sha256: [u8; 32],
}

struct ParsedVerificationRequestV1 {
    base: ParsedClientRequestV1,
    authority_envelope_json: Vec<u8>,
    durable_receipt_json: Vec<u8>,
}

fn parse_client_request(
    bytes: &[u8],
    role: TairaAuthorityRoleV1,
) -> Result<ParsedClientRequestV1, TairaAuthorityErrorV1> {
    parse_client_request_with_schema(bytes, role, "iroha.taira.authority-client-request.v1")
}

fn parse_client_request_with_schema(
    bytes: &[u8],
    role: TairaAuthorityRoleV1,
    expected_schema: &str,
) -> Result<ParsedClientRequestV1, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(bytes)?;
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    let common_fields = [
        "artifact_manifest",
        "operation_id",
        "role",
        "run_id",
        "schema",
        "subject",
    ];
    let (deploy_disposition, deployment_result) = if role == TairaAuthorityRoleV1::DeployIssuance {
        let disposition = match required_str(object, "disposition")? {
            "dry-run" => DeployDispositionV1::DryRun,
            "apply" => DeployDispositionV1::Apply,
            "finalize" => DeployDispositionV1::Finalize,
            _ => return Err(TairaAuthorityErrorV1::Rejected),
        };
        let expected_length = if disposition == DeployDispositionV1::Finalize {
            8
        } else {
            7
        };
        if object.len() != expected_length
            || common_fields
                .iter()
                .any(|field| !object.contains_key(*field))
            || !object.contains_key("disposition")
            || (disposition == DeployDispositionV1::Finalize)
                != object.contains_key("deployment_result")
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let result = if disposition == DeployDispositionV1::Finalize {
            let result = exact_object(
                object
                    .get("deployment_result")
                    .ok_or(TairaAuthorityErrorV1::Rejected)?,
                &["outcome", "result_sha256"],
            )?;
            let outcome = required_str(result, "outcome")?.to_owned();
            if !matches!(
                outcome.as_str(),
                "success" | "rolled-back" | "rollback-failed"
            ) {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            Some(DeploymentResultV1 {
                outcome,
                result_sha256: required_digest(result, "result_sha256")?,
            })
        } else {
            None
        };
        (Some(disposition), result)
    } else {
        if object.len() != common_fields.len()
            || common_fields
                .iter()
                .any(|field| !object.contains_key(*field))
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        (None, None)
    };
    if required_str(object, "schema")? != expected_schema
        || required_str(object, "role")? != role.as_str()
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let operation_id = required_digest(object, "operation_id")?;
    let run_id = required_digest(object, "run_id")?;
    let subject = object
        .get("subject")
        .cloned()
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if !subject.is_object() {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let subject_bytes = canonical_json_core(&subject)?;
    let manifest_value = object
        .get("artifact_manifest")
        .cloned()
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let manifest = parse_manifest(&manifest_value)?;
    let manifest_bytes = canonical_json_core(&manifest_value)?;
    let expected_run_id = digest_parts_sha256(
        RUN_ID_DOMAIN_V1,
        &[role.as_str().as_bytes(), &sha256(&subject_bytes)],
    );
    let expected_operation_id = digest_parts_sha256(
        OPERATION_ID_DOMAIN_V1,
        &[
            role.as_str().as_bytes(),
            &expected_run_id,
            &sha256(&subject_bytes),
            &sha256(&manifest_bytes),
        ],
    );
    if run_id != expected_run_id || operation_id != expected_operation_id {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut identity = Map::new();
    for field in common_fields {
        identity.insert(
            field.to_owned(),
            object
                .get(field)
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        );
    }
    let canonical_request = canonical_json_line(&Value::Object(identity))?;
    let wire_request = canonical_json_line(&value)?;
    let wire_request_sha256 = sha256(&wire_request);
    Ok(ParsedClientRequestV1 {
        operation_id,
        run_id,
        request_sha256: sha256(&canonical_request),
        subject,
        subject_sha256: sha256(&subject_bytes),
        manifest_value,
        manifest_sha256: sha256(&manifest_bytes),
        manifest,
        canonical_request_json: wire_request,
        deploy_disposition,
        deployment_result,
        wire_request_sha256,
    })
}

fn parse_verification_request(
    bytes: &[u8],
    role: TairaAuthorityRoleV1,
) -> Result<ParsedVerificationRequestV1, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(bytes)?;
    let object = exact_object(
        &value,
        &[
            "artifact_manifest",
            "authority_envelope",
            "durable_receipt",
            "operation_id",
            "role",
            "run_id",
            "schema",
            "subject",
        ],
    )?;
    if required_str(object, "schema")? != "iroha.taira.authority-client-verification.v1"
        || required_str(object, "role")? != role.as_str()
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut base_object = Map::new();
    for field in [
        "artifact_manifest",
        "operation_id",
        "role",
        "run_id",
        "subject",
    ] {
        base_object.insert(
            field.to_owned(),
            object
                .get(field)
                .cloned()
                .ok_or(TairaAuthorityErrorV1::Rejected)?,
        );
    }
    base_object.insert(
        "schema".to_owned(),
        Value::from("iroha.taira.authority-client-request.v1"),
    );
    if role == TairaAuthorityRoleV1::DeployIssuance {
        base_object.insert("disposition".to_owned(), Value::from("apply"));
    }
    let base_bytes = canonical_json_line(&Value::Object(base_object))?;
    let base = parse_client_request_with_schema(
        &base_bytes,
        role,
        "iroha.taira.authority-client-request.v1",
    )?;
    let authority_envelope_json = canonical_json_line(
        object
            .get("authority_envelope")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    let durable_receipt_json = canonical_json_line(
        object
            .get("durable_receipt")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    Ok(ParsedVerificationRequestV1 {
        base,
        authority_envelope_json,
        durable_receipt_json,
    })
}

fn parse_assignment(
    bytes: &[u8],
    role: TairaAuthorityRoleV1,
) -> Result<RunAssignmentV1, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(bytes)?;
    let object = exact_object(
        &value,
        &[
            "artifact_manifest_sha256",
            "expires_at_unix_millis",
            "issued_at_unix_millis",
            "key_revision",
            "not_before_unix_millis",
            "policy_revision",
            "policy_sha256",
            "role",
            "run_id",
            "schema",
            "subject_sha256",
        ],
    )?;
    if required_str(object, "schema")? != "iroha.taira.authority-run-assignment.v1"
        || required_str(object, "role")? != role.as_str()
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(RunAssignmentV1 {
        role,
        run_id: required_digest(object, "run_id")?,
        subject_sha256: required_digest(object, "subject_sha256")?,
        artifact_manifest_sha256: required_digest(object, "artifact_manifest_sha256")?,
        issued_at_unix_millis: required_u64(object, "issued_at_unix_millis")?,
        not_before_unix_millis: required_u64(object, "not_before_unix_millis")?,
        expires_at_unix_millis: required_u64(object, "expires_at_unix_millis")?,
        key_revision: required_u64(object, "key_revision")?,
        policy_revision: required_u64(object, "policy_revision")?,
        policy_digest: required_digest(object, "policy_sha256")?,
    })
}

fn parse_manifest(
    value: &Value,
) -> Result<Vec<TairaAuthorityArtifactManifestEntryV1>, TairaAuthorityErrorV1> {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() > TAIRA_AUTHORITY_MAX_ARTIFACTS_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    let mut manifest = Vec::with_capacity(rows.len());
    for (ordinal, row) in rows.iter().enumerate() {
        let row = exact_object(row, &["name", "ordinal", "sha256", "size"])?;
        let observed_ordinal = required_nonnegative_u64(row, "ordinal")?;
        if observed_ordinal
            != u64::try_from(ordinal).map_err(|_| TairaAuthorityErrorV1::Rejected)?
        {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let name = required_str(row, "name")?.to_owned();
        if !valid_manifest_name(&name) || !names.insert(name.clone()) {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let size = required_u64(row, "size")?;
        if size == 0 || size > TAIRA_AUTHORITY_MAX_ARTIFACT_BYTES_V1 {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        total = total
            .checked_add(size)
            .filter(|total| *total <= TAIRA_AUTHORITY_MAX_TOTAL_ARTIFACT_BYTES_V1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?;
        manifest.push(TairaAuthorityArtifactManifestEntryV1 {
            ordinal: u16::try_from(ordinal).map_err(|_| TairaAuthorityErrorV1::Rejected)?,
            name,
            size,
            sha256: required_digest(row, "sha256")?,
        });
    }
    Ok(manifest)
}

fn valid_manifest_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && !value.ends_with('/')
        && value.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
}

struct ValidatedArtifactsV1 {
    files: Vec<File>,
    identities: Vec<ArtifactIdentityV1>,
    expected: Vec<TairaAuthorityArtifactManifestEntryV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArtifactIdentityV1 {
    device: u64,
    inode: u64,
    length: u64,
    links: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ValidatedArtifactsV1 {
    fn new(
        descriptors: Vec<OwnedFd>,
        manifest: &[TairaAuthorityArtifactManifestEntryV1],
        authenticated_uid: u32,
    ) -> Result<Self, TairaAuthorityErrorV1> {
        if descriptors.len() != manifest.len() {
            return Err(TairaAuthorityErrorV1::Rejected);
        }
        let mut files = Vec::with_capacity(descriptors.len());
        let mut identities = Vec::with_capacity(descriptors.len());
        let mut expected_entries = Vec::with_capacity(descriptors.len());
        let mut file_identities = BTreeSet::new();
        for (descriptor, expected) in descriptors.into_iter().zip(manifest) {
            let mut file = File::from(descriptor);
            let flags =
                rustix::fs::fcntl_getfl(&file).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
            if flags.intersects(rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::RDWR) {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            let metadata = file
                .metadata()
                .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || metadata.len() != expected.size
                || (metadata.uid() != 0 && metadata.uid() != authenticated_uid)
                || metadata.mode() & 0o022 != 0
            {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            let identity = artifact_identity(&metadata);
            if !file_identities.insert((identity.device, identity.inode)) {
                return Err(TairaAuthorityErrorV1::Rejected);
            }
            verify_artifact(&mut file, identity, expected)?;
            files.push(file);
            identities.push(identity);
            expected_entries.push(expected.clone());
        }
        Ok(Self {
            files,
            identities,
            expected: expected_entries,
        })
    }

    fn revalidate(&mut self) -> Result<(), TairaAuthorityErrorV1> {
        for ((file, identity), expected) in self
            .files
            .iter_mut()
            .zip(self.identities.iter().copied())
            .zip(&self.expected)
        {
            verify_artifact(file, identity, expected)?;
        }
        Ok(())
    }

    fn files_mut(&mut self) -> &mut [File] {
        &mut self.files
    }
}

fn verify_artifact(
    file: &mut File,
    identity: ArtifactIdentityV1,
    expected: &TairaAuthorityArtifactManifestEntryV1,
) -> Result<(), TairaAuthorityErrorV1> {
    let before = file
        .metadata()
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if artifact_identity(&before) != identity {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut hasher = Sha256::new();
    let mut limited = (&mut *file).take(
        expected
            .size
            .checked_add(1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    );
    let copied =
        std::io::copy(&mut limited, &mut hasher).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let after = file
        .metadata()
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let digest: [u8; 32] = hasher.finalize().into();
    if copied != expected.size || digest != expected.sha256 || artifact_identity(&after) != identity
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(())
}

fn artifact_identity(metadata: &std::fs::Metadata) -> ArtifactIdentityV1 {
    ArtifactIdentityV1 {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        links: metadata.nlink(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    }
}

struct ValidatedPublicSoakObservationV1 {
    authority_key_id: String,
    replay_id: String,
}

fn taira_validated_message_payload(
    role: TairaAuthorityRoleV1,
    purpose: &str,
    signing_message: &[u8],
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut wrapper = Map::new();
    wrapper.insert("role".into(), Value::from(role.as_str()));
    wrapper.insert("purpose".into(), Value::from(purpose));
    wrapper.insert(
        "signing_message_hex".into(),
        Value::from(hex::encode(signing_message)),
    );
    let wrapper = canonical_json_core(&Value::Object(wrapper))?;
    let mut payload =
        Vec::with_capacity(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1.len() + wrapper.len());
    payload.extend_from_slice(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1);
    payload.extend_from_slice(&wrapper);
    Ok(payload)
}

fn validate_public_soak_observation_subject(
    request: &ParsedClientRequestV1,
) -> Result<(u64, [u8; 32]), TairaAuthorityErrorV1> {
    let subject = exact_object(
        &request.subject,
        &["completed_at_unix_ms", "subject", "subject_digest"],
    )?;
    let completed_at = required_u64(subject, "completed_at_unix_ms")?;
    let digest = validate_public_soak_subject_core(
        subject
            .get("subject")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    if digest != required_digest(subject, "subject_digest")? {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok((completed_at, digest))
}

fn validate_public_soak_subject_core(value: &Value) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    let subject = exact_object(
        value,
        &[
            "anchor",
            "applied_statuses",
            "blocks",
            "lifecycle",
            "native_verifier",
            "prerequisites",
            "receipt",
            "samples",
            "schema",
            "source",
            "submission_receipts",
            "workload",
        ],
    )?;
    if required_str(subject, "schema")? != "iroha.taira.public-v2-24h-soak-authority-subject.v1" {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let receipt = exact_object(
        subject
            .get("receipt")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &["sha256", "size_bytes"],
    )?;
    required_digest(receipt, "sha256")?;
    required_u64(receipt, "size_bytes")?;
    let source = exact_object(
        subject
            .get("source")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &["tuple_sha256"],
    )?;
    required_digest(source, "tuple_sha256")?;
    let prerequisites = exact_object(
        subject
            .get("prerequisites")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &[
            "candidate_handoff_sha256",
            "deploy_handoff_sha256",
            "publication_handoff_sha256",
        ],
    )?;
    let prerequisite_digests = [
        required_digest(prerequisites, "candidate_handoff_sha256")?,
        required_digest(prerequisites, "deploy_handoff_sha256")?,
        required_digest(prerequisites, "publication_handoff_sha256")?,
    ];
    if prerequisite_digests
        .into_iter()
        .collect::<BTreeSet<_>>()
        .len()
        != 3
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    validate_digest_count_object(subject, "anchor", "sha256", "validator_count")?;
    validate_digest_count_object(subject, "samples", "sha256", "count")?;
    for field in [
        "workload",
        "submission_receipts",
        "applied_statuses",
        "blocks",
    ] {
        let inventory = exact_object(
            subject.get(field).ok_or(TairaAuthorityErrorV1::Rejected)?,
            &["artifact_sha256", "record_count", "records_sha256"],
        )?;
        required_digest(inventory, "artifact_sha256")?;
        required_digest(inventory, "records_sha256")?;
        required_u64(inventory, "record_count")?;
    }
    let lifecycle = exact_object(
        subject
            .get("lifecycle")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &[
            "artifact_sha256",
            "identity_sha256",
            "journal_artifact_sha256",
            "journal_record_count",
            "journal_records_sha256",
            "native_verifier_receipt_sha256",
            "window_sha256",
        ],
    )?;
    for field in [
        "artifact_sha256",
        "identity_sha256",
        "journal_artifact_sha256",
        "journal_records_sha256",
        "native_verifier_receipt_sha256",
        "window_sha256",
    ] {
        required_digest(lifecycle, field)?;
    }
    required_u64(lifecycle, "journal_record_count")?;
    let verifier = exact_object(
        subject
            .get("native_verifier")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &["binary_sha256", "source_sha256"],
    )?;
    required_digest(verifier, "binary_sha256")?;
    required_digest(verifier, "source_sha256")?;
    let core = canonical_json_line(value)?;
    let mut digest = Sha256::new();
    digest.update(PUBLIC_SOAK_SUBJECT_DOMAIN_V1);
    digest.update(core);
    Ok(digest.finalize().into())
}

fn validate_digest_count_object(
    parent: &Map,
    field: &str,
    digest_field: &str,
    count_field: &str,
) -> Result<(), TairaAuthorityErrorV1> {
    let object = exact_object(
        parent.get(field).ok_or(TairaAuthorityErrorV1::Rejected)?,
        &[digest_field, count_field],
    )?;
    required_digest(object, digest_field)?;
    required_u64(object, count_field)?;
    Ok(())
}

fn validate_public_soak_envelope(
    value: &Value,
    binding: &TairaAuthorityPublicBindingV1,
    expected_subject_digest: [u8; 32],
    completed_at_unix_millis: u64,
    admitted_at_unix_millis: u64,
) -> Result<ValidatedPublicSoakObservationV1, TairaAuthorityErrorV1> {
    let envelope = exact_object(
        value,
        &[
            "authority_key_id",
            "claims",
            "schema",
            "schema_version",
            "signature",
            "signature_algorithm",
        ],
    )?;
    let expected_key_id = hex::encode(binding.signer.public_key_digest);
    if required_str(envelope, "schema")? != "iroha.taira.public-v2-24h-soak-authority-envelope.v1"
        || required_u64(envelope, "schema_version")? != 1
        || required_str(envelope, "signature_algorithm")? != "ed25519"
        || required_str(envelope, "authority_key_id")? != expected_key_id
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let claims = exact_object(
        envelope
            .get("claims")
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
        &[
            "expires_at_unix_ms",
            "issued_at_unix_ms",
            "replay_id",
            "replay_namespace",
            "schema",
            "subject_digest",
        ],
    )?;
    if required_str(claims, "schema")? != "iroha.taira.public-v2-24h-soak-authority-claims.v1"
        || required_str(claims, "replay_namespace")? != PUBLIC_SOAK_REPLAY_NAMESPACE_V1
        || required_digest(claims, "subject_digest")? != expected_subject_digest
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let issued = required_u64(claims, "issued_at_unix_ms")?;
    let expires = required_u64(claims, "expires_at_unix_ms")?;
    if issued < completed_at_unix_millis
        || issued - completed_at_unix_millis > PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1
        || expires <= issued
        || expires - issued > PUBLIC_SOAK_MAX_AUTHORITY_LIFETIME_MILLIS_V1
        || admitted_at_unix_millis < issued
        || admitted_at_unix_millis > expires
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let replay_id = required_str(claims, "replay_id")?.to_owned();
    parse_digest(&replay_id)?;
    let signature_bytes = hex::decode(required_str(envelope, "signature")?)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let signature =
        Signature::try_from_bytes(&signature_bytes).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    let mut unsigned = envelope.clone();
    unsigned.remove("signature");
    let unsigned = canonical_json_line(&Value::Object(unsigned))?;
    let mut signing_message =
        Vec::with_capacity(PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1.len() + unsigned.len());
    signing_message.extend_from_slice(PUBLIC_SOAK_OBSERVATION_SIGNATURE_DOMAIN_V1);
    signing_message.extend_from_slice(&unsigned);
    signature
        .verify(&binding.signer.public_key, &signing_message)
        .map_err(|_| TairaAuthorityErrorV1::Crypto)?;
    Ok(ValidatedPublicSoakObservationV1 {
        authority_key_id: expected_key_id,
        replay_id,
    })
}

fn envelope_claims_json(
    role: TairaAuthorityRoleV1,
    request: &ParsedClientRequestV1,
    issued_at: u64,
    expires_at: u64,
    qualification_probe_results: Option<Value>,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-envelope-claims.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "replay_namespace".into(),
        Value::from(role.replay_namespace()),
    );
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(request.run_id)));
    object.insert(
        "subject_sha256".into(),
        Value::from(hex::encode(request.subject_sha256)),
    );
    object.insert(
        "artifact_manifest_sha256".into(),
        Value::from(hex::encode(request.manifest_sha256)),
    );
    object.insert("issued_at_unix_millis".into(), Value::from(issued_at));
    object.insert("expires_at_unix_millis".into(), Value::from(expires_at));
    object.insert("subject".into(), request.subject.clone());
    object.insert("artifact_manifest".into(), request.manifest_value.clone());
    if let Some(probe_results) = qualification_probe_results {
        let mut role_result = Map::new();
        role_result.insert("probe_results".into(), probe_results);
        object.insert("role_result".into(), Value::Object(role_result));
    }
    canonical_json_line(&Value::Object(object))
}

fn authority_envelope_json(
    role: TairaAuthorityRoleV1,
    claims_json: &[u8],
    receipt: &SoftwareSignerSignatureReceiptV1,
    binding: &TairaAuthorityPublicBindingV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let claims = parse_canonical_json(claims_json)?;
    let mut object = Map::new();
    object.insert("schema".into(), Value::from(role.envelope_schema()));
    object.insert("schema_version".into(), Value::from(1_u64));
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("claims".into(), claims);
    object.insert("signature_algorithm".into(), Value::from("ed25519"));
    object.insert(
        "binding_sha256".into(),
        Value::from(hex::encode(
            binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn durable_receipt_claims_json(
    role: TairaAuthorityRoleV1,
    request: &ParsedClientRequestV1,
    envelope_json: &[u8],
    admitted_at: u64,
    envelope_receipt: &SoftwareSignerSignatureReceiptV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-durable-receipt-claims.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("decision".into(), Value::from("admitted"));
    object.insert(
        "replay_namespace".into(),
        Value::from(role.replay_namespace()),
    );
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(request.run_id)));
    object.insert(
        "subject_sha256".into(),
        Value::from(hex::encode(request.subject_sha256)),
    );
    object.insert(
        "authority_envelope_sha256".into(),
        Value::from(hex::encode(sha256(envelope_json))),
    );
    object.insert("admitted_at_unix_millis".into(), Value::from(admitted_at));
    object.insert(
        "authority_audit_sequence".into(),
        Value::from(envelope_receipt.commit_sequence),
    );
    object.insert(
        "authority_audit_head".into(),
        Value::from(hex::encode(envelope_receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn durable_receipt_json(
    role: TairaAuthorityRoleV1,
    claims_json: &[u8],
    receipt: &SoftwareSignerSignatureReceiptV1,
    binding: &TairaAuthorityPublicBindingV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let claims = parse_canonical_json(claims_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-durable-receipt.v1"),
    );
    object.insert("schema_version".into(), Value::from(1_u64));
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert("claims".into(), claims);
    object.insert("signature_algorithm".into(), Value::from("ed25519"));
    object.insert(
        "binding_sha256".into(),
        Value::from(hex::encode(
            binding
                .sha256()
                .map_err(|()| TairaAuthorityErrorV1::Binding)?,
        )),
    );
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn assignment_result_json(
    stored: &StoredRunAssignmentV1,
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let assignment = parse_canonical_json(&stored.assignment_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-run-assignment-result.v1"),
    );
    object.insert("role".into(), Value::from(stored.assignment.role.as_str()));
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "assigned" }),
    );
    object.insert("assignment".into(), assignment);
    object.insert(
        "signature".into(),
        Value::from(hex::encode(&stored.receipt.signature)),
    );
    object.insert(
        "audit_sequence".into(),
        Value::from(stored.receipt.commit_sequence),
    );
    object.insert(
        "audit_head".into(),
        Value::from(hex::encode(stored.receipt.commit_audit_head)),
    );
    canonical_json_line(&Value::Object(object))
}

fn dry_run_result_json(request: &ParsedClientRequestV1) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("status".into(), Value::from("verified"));
    object.insert("authority_envelope".into(), Value::Object(Map::new()));
    object.insert("durable_receipt".into(), Value::Object(Map::new()));
    canonical_json_line(&Value::Object(object))
}

fn deployment_finalization_claims_json(
    request: &ParsedClientRequestV1,
    result: &DeploymentResultV1,
    finalized_at_unix_millis: u64,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.deployment-finalization-claims.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(request.operation_id)),
    );
    object.insert("run_id".into(), Value::from(hex::encode(request.run_id)));
    object.insert("outcome".into(), Value::from(result.outcome.clone()));
    object.insert(
        "result_sha256".into(),
        Value::from(hex::encode(result.result_sha256)),
    );
    object.insert(
        "finalized_at_unix_millis".into(),
        Value::from(finalized_at_unix_millis),
    );
    canonical_json_line(&Value::Object(object))
}

fn deployment_finalization_result_json(
    operation_id: [u8; 32],
    authority_envelope_json: &[u8],
    durable_receipt_json: &[u8],
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from("deploy-issuance"));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(operation_id)),
    );
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "finalized" }),
    );
    object.insert(
        "authority_envelope".into(),
        parse_canonical_json(authority_envelope_json)?,
    );
    object.insert(
        "durable_receipt".into(),
        parse_canonical_json(durable_receipt_json)?,
    );
    canonical_json_line(&Value::Object(object))
}

fn replayed_finalization_result_json(
    stored: &StoredDeploymentFinalizationV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(&stored.result_json)?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(TairaAuthorityErrorV1::State)?;
    object.insert("status".into(), Value::from("replayed"));
    canonical_json_line(&Value::Object(object))
}

fn authorization_result_json(
    stored: &StoredAuthorizationV1,
    role: TairaAuthorityRoleV1,
    replayed: bool,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let envelope = parse_canonical_json(&stored.authority_envelope_json)?;
    let receipt = parse_canonical_json(&stored.durable_receipt_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(stored.consumption.operation_id)),
    );
    object.insert(
        "status".into(),
        Value::from(if replayed { "replayed" } else { "authorized" }),
    );
    object.insert("authority_envelope".into(), envelope);
    object.insert("durable_receipt".into(), receipt);
    canonical_json_line(&Value::Object(object))
}

fn verification_result_json(
    stored: &StoredAuthorizationV1,
    role: TairaAuthorityRoleV1,
) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let envelope = parse_canonical_json(&stored.authority_envelope_json)?;
    let receipt = parse_canonical_json(&stored.durable_receipt_json)?;
    let mut object = Map::new();
    object.insert(
        "schema".into(),
        Value::from("iroha.taira.authority-client-result.v1"),
    );
    object.insert("role".into(), Value::from(role.as_str()));
    object.insert(
        "operation_id".into(),
        Value::from(hex::encode(stored.consumption.operation_id)),
    );
    object.insert("status".into(), Value::from("valid"));
    object.insert("authority_envelope".into(), envelope);
    object.insert("durable_receipt".into(), receipt);
    canonical_json_line(&Value::Object(object))
}

fn taira_signing_payload(json: &[u8]) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(json)?;
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if !object.contains_key("role") {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let canonical = canonical_json_core(&value)?;
    let mut payload =
        Vec::with_capacity(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1.len() + canonical.len());
    payload.extend_from_slice(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1);
    payload.extend_from_slice(&canonical);
    Ok(payload)
}

fn durable_receipt_signing_payload(json: &[u8]) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let value = parse_canonical_json(json)?;
    let mut wrapped = Map::new();
    wrapped.insert(
        "role".into(),
        value
            .get("role")
            .cloned()
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    );
    wrapped.insert(
        "signing_domain".into(),
        Value::from(String::from_utf8_lossy(DURABLE_RECEIPT_SIGNING_DOMAIN_V1).into_owned()),
    );
    wrapped.insert("receipt_claims".into(), value);
    let wrapped = canonical_json_core(&Value::Object(wrapped))?;
    let mut payload =
        Vec::with_capacity(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1.len() + wrapped.len());
    payload.extend_from_slice(TAIRA_RELEASE_AUTHORITY_SIGNING_DOMAIN_V1);
    payload.extend_from_slice(&wrapped);
    Ok(payload)
}

fn parse_canonical_json(bytes: &[u8]) -> Result<Value, TairaAuthorityErrorV1> {
    if bytes.is_empty() || bytes.len() > TAIRA_AUTHORITY_MAX_JSON_BYTES_V1 {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let core = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if core.is_empty() || core.ends_with(b"\n") {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let value: Value =
        norito::json::from_slice(core).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if canonical_json_core(&value)? != core {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(value)
}

fn canonical_json_core(value: &Value) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    norito::json::to_vec(value).map_err(|_| TairaAuthorityErrorV1::Rejected)
}

fn canonical_json_line(value: &Value) -> Result<Vec<u8>, TairaAuthorityErrorV1> {
    let mut bytes = canonical_json_core(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map, TairaAuthorityErrorV1> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(object)
}

fn required_str<'a>(object: &'a Map, field: &str) -> Result<&'a str, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_u64(object: &Map, field: &str) -> Result<u64, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_nonnegative_u64(object: &Map, field: &str) -> Result<u64, TairaAuthorityErrorV1> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_digest(object: &Map, field: &str) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    parse_digest(required_str(object, field)?)
}

pub(super) fn parse_digest(value: &str) -> Result<[u8; 32], TairaAuthorityErrorV1> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    let digest: [u8; 32] = hex::decode(value)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        .try_into()
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if digest == [0; 32] {
        return Err(TairaAuthorityErrorV1::Rejected);
    }
    Ok(digest)
}

pub(super) fn digest_parts_sha256(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

pub(super) fn response_for_error(error: TairaAuthorityErrorV1) -> OperationResponseV1 {
    OperationResponseV1 {
        status: match error {
            TairaAuthorityErrorV1::Conflict => OperationStatusV1::Conflict,
            TairaAuthorityErrorV1::State => OperationStatusV1::Unavailable,
            _ => OperationStatusV1::Rejected,
        },
        result_json: Vec::new(),
    }
}
