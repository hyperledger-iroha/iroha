//! Resumable, evidence-checked Musubi V1 publication workflow.
//!
//! The workflow deliberately keeps network authentication and signing outside
//! the persisted state.  Its journal contains only public request material,
//! signed public evidence, finalized records, and idempotency identifiers.  A
//! backend is therefore supplied at runtime and cannot smuggle provider URLs,
//! bearer credentials, private keys, or a retired public upload route into a
//! project or operation journal.

use std::{
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use iroha_data_model::{
    ChainId,
    account::AccountId,
    isi::musubi::PublishMusubiReleaseV1,
    musubi::{
        ArchiveId, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1,
        MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1, MusubiArchiveLocationStateV1,
        MusubiArchiveLocationV1, MusubiArchiveRecordV1, MusubiContentDigestV1,
        MusubiNamespaceDelegationV1, MusubiNamespaceV1, MusubiPackageScopeV1, MusubiPublicationV1,
        MusubiRegistrySnapshotV1, MusubiReleaseDigestV1, MusubiReleaseRecordV1,
        MusubiResolverReleaseRowV1, MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptV1,
        MusubiSemanticReleaseDigestV1, MusubiStorageAvailabilityV1, MusubiVerificationLockDigestV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, ReplicationOrderId},
    },
};
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};

use crate::atomic_io::{AtomicWriteError, AtomicWriteRoot};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};

const JOURNAL_SCHEMA: &str = "musubi-publication-journal";
const JOURNAL_VERSION: u8 = 1;
const JOURNAL_DIRECTORY: &str = "publication-v1";
const JOURNAL_EXTENSION: &str = "norito";
const JOURNAL_LOCK_EXTENSION: &str = "lock";
const STAGED_CAR_EXTENSION: &str = "car";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_JOURNAL_BYTES_USIZE: usize = 16 * 1024 * 1024;
const JOURNAL_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    1_048_576,
    MAX_JOURNAL_BYTES_USIZE,
    2_097_152,
    64 * 1024 * 1024,
    128,
);
const OPERATION_ID_DOMAIN: &[u8] = b"iroha.musubi.publication-operation.v1";
const PUBLISH_INSTRUCTION_DOMAIN: &[u8] = b"iroha.musubi.publish-instruction.v1";
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;
#[cfg(windows)]
const FILE_SHARE_WRITE: u32 = 0x0000_0002;

/// Stable identifier used to make every remote publication transition idempotent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct PublicationOperationIdV1([u8; 32]);

impl PublicationOperationIdV1 {
    /// Return the exact operation-id bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn from_request(request: &PublicationRequestV1) -> Self {
        Self(domain_hash(OPERATION_ID_DOMAIN, &request.encode()))
    }

    fn is_zero(self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

impl fmt::Display for PublicationOperationIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

/// Error returned when a detached publication operation id is not canonical hex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationOperationIdParseError;

impl fmt::Display for PublicationOperationIdParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("publication operation id must be 64 non-zero lowercase hex digits")
    }
}

impl Error for PublicationOperationIdParseError {}

impl FromStr for PublicationOperationIdV1 {
    type Err = PublicationOperationIdParseError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.len() != 64
            || raw
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(PublicationOperationIdParseError);
        }
        let decoded = hex::decode(raw).map_err(|_| PublicationOperationIdParseError)?;
        let bytes = <[u8; 32]>::try_from(decoded).map_err(|_| PublicationOperationIdParseError)?;
        let operation_id = Self(bytes);
        if operation_id.is_zero() {
            return Err(PublicationOperationIdParseError);
        }
        Ok(operation_id)
    }
}

/// Public inputs that remain stable across every retry of one publication.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationRequestV1 {
    /// Deployment-selected chain identity.
    pub chain_id: ChainId,
    /// Exact genesis block hash for the selected deployment.
    pub genesis_block_hash: [u8; 32],
    /// Account that will publish the release through Native AMX.
    pub publisher: AccountId,
    /// Authenticated seed-ingress broker expected to sign the staging receipt.
    pub ingress_broker: AccountId,
    /// Seed provider selected for the authenticated ingress request.
    pub seed_provider: ProviderId,
    /// Exact canonical namespace whose immutable binding authorizes the package claim.
    pub namespace: MusubiNamespaceV1,
    /// Immutable release and exact verification graph.
    pub publication: MusubiPublicationV1,
    /// Exact canonical CAR and parsed-bundle commitment.
    pub archive_commitment: MusubiArchiveCommitmentV1,
    /// Optional generation-bound authorization for the first package claim.
    pub namespace_delegation: Option<MusubiNamespaceDelegationV1>,
    /// Registry-policy revision used by archive and release admission.
    pub expected_policy_revision: u64,
    /// Existing package-governance revision, absent only for a first claim.
    pub expected_governance_revision: Option<u64>,
    /// Unpredictable public anti-replay nonce for this operation.
    pub nonce: [u8; 32],
}

impl PublicationRequestV1 {
    /// Validate all immutable publication, archive, deployment, and revision bindings.
    pub fn validate(&self) -> Result<(), PublicationError> {
        self.publication
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        self.archive_commitment
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        self.namespace
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        if let Some(delegation) = &self.namespace_delegation {
            delegation
                .validate()
                .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
            if delegation.payload.delegate != self.publisher {
                return Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Validation,
                    reason: "namespace delegation does not authorize the publisher".to_owned(),
                });
            }
        }
        if self.chain_id.as_str().is_empty()
            || self.genesis_block_hash.iter().all(|byte| *byte == 0)
            || self.seed_provider.as_bytes().iter().all(|byte| *byte == 0)
            || self.nonce.iter().all(|byte| *byte == 0)
            || self.expected_policy_revision == 0
            || self.expected_governance_revision == Some(0)
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "publication deployment, nonce, or revision binding is inert".to_owned(),
            });
        }
        let namespace_scope_matches = match (
            &self.publication.manifest.release.package.scope,
            self.namespace.domain_segment(),
        ) {
            (MusubiPackageScopeV1::DataspaceRoot, None) => true,
            (MusubiPackageScopeV1::Domain(domain), Some(segment)) => domain.as_ref() == segment,
            _ => false,
        };
        if !namespace_scope_matches {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "canonical namespace does not match the structural package scope"
                    .to_owned(),
            });
        }
        if self.publication.manifest.archive_id != self.archive_commitment.archive_id() {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "release manifest does not bind the supplied archive commitment".to_owned(),
            });
        }
        Ok(())
    }

    /// Derive the stable idempotency identifier for this exact request.
    #[must_use]
    pub fn operation_id(&self) -> PublicationOperationIdV1 {
        PublicationOperationIdV1::from_request(self)
    }

    fn receipt_binding(&self) -> MusubiSeedIngressReceiptBindingV1 {
        MusubiSeedIngressReceiptBindingV1 {
            chain_id: self.chain_id.clone(),
            genesis_block_hash: self.genesis_block_hash,
            publisher: self.publisher.clone(),
            ingress_broker: self.ingress_broker.clone(),
            seed_provider: self.seed_provider,
            semantic_release_manifest_digest: self.publication.manifest.semantic_digest(),
            archive_id: self.archive_commitment.archive_id(),
            car_body_digest: self.archive_commitment.car_digest,
            car_body_length: self.archive_commitment.car_size,
            nonce: self.nonce,
        }
    }

    fn publish_instruction(&self) -> PublishMusubiReleaseV1 {
        PublishMusubiReleaseV1::new(
            self.namespace.clone(),
            self.publication.clone(),
            self.namespace_delegation.clone(),
            self.expected_policy_revision,
            self.expected_governance_revision,
        )
    }
}

/// The seven production phases of Musubi V1 publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum PublicationPhaseV1 {
    /// Validate and compiler-check the clean package and exact proof.
    Validation,
    /// Stage the exact CAR through authenticated SoraFS seed ingress.
    SeedIngress,
    /// Idempotently register the archive and create its permanent pin/order.
    ArchiveRegistration,
    /// Wait for finalized, provider-verified replication quorum.
    Replication,
    /// Verify full readback through two distinct finalized providers.
    Readback,
    /// Submit the final package claim and release through Native AMX.
    ReleaseSubmission,
    /// Verify the exact finalized home record and universal resolver row.
    FinalVerification,
}

/// Secret-free proof that the clean package and exact verification graph were checked.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationValidationEvidenceV1 {
    /// Archive whose exact CAR bytes were checked.
    pub archive_id: ArchiveId,
    /// Semantic manifest digest embedded in the checked bundle.
    pub semantic_release_digest: MusubiSemanticReleaseDigestV1,
    /// Full registry release digest, including the archive identity.
    pub release_digest: MusubiReleaseDigestV1,
    /// Source-tree digest reproduced from the clean packaged tree.
    pub source_tree_digest: MusubiContentDigestV1,
    /// Typed artifact-descriptor digest reproduced from the bundle.
    pub descriptor_digest: MusubiContentDigestV1,
    /// Exact normalized verification-lock digest checked by the resolver.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
    /// Exact CAR digest read by the compiler-validation path.
    pub car_digest: MusubiContentDigestV1,
    /// Exact CAR length read by the compiler-validation path.
    pub car_size: u64,
    /// Deterministic digest of the successful compiler result.
    pub compiler_output_digest: MusubiContentDigestV1,
    /// Finalized registry snapshot against which the exact graph was checked.
    pub resolution_snapshot: MusubiRegistrySnapshotV1,
}

impl PublicationValidationEvidenceV1 {
    fn validate_for(&self, request: &PublicationRequestV1) -> Result<(), PublicationError> {
        let manifest = &request.publication.manifest;
        if self.archive_id != request.archive_commitment.archive_id()
            || self.semantic_release_digest != manifest.semantic_digest()
            || self.release_digest != manifest.release_digest()
            || self.source_tree_digest != request.archive_commitment.source_tree_digest
            || self.descriptor_digest != request.archive_commitment.descriptor_digest
            || self.verification_lock_digest != manifest.verification_lock_digest
            || self.car_digest != request.archive_commitment.car_digest
            || self.car_size != request.archive_commitment.car_size
            || self.compiler_output_digest.is_zero()
            || self.resolution_snapshot != request.publication.resolution.snapshot
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "clean-package validation evidence was substituted or incomplete"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Idempotent archive-registration and permanent pin/order result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveRegistrationV1 {
    /// Finalized archive retaining a verified receipt for this exact operation binding.
    pub archive: MusubiArchiveRecordV1,
    /// Stable location identity reserved for this publication.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Permanent registry-grade SoraFS pin manifest.
    pub pin_manifest: ManifestDigest,
    /// Replication order used for finalized provider completions.
    pub replication_order: ReplicationOrderId,
    /// Earliest renewal epoch requested for the location.
    pub renew_after_epoch: u64,
    /// Expiry epoch requested for the renewable location.
    pub expires_at_epoch: u64,
}

impl PublicationArchiveRegistrationV1 {
    fn validate_for(
        &self,
        request: &PublicationRequestV1,
        staging_receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<(), PublicationError> {
        self.archive
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        let registered_binding = &self.archive.staging_receipt.payload.binding;
        self.archive
            .staging_receipt
            .verify(
                registered_binding,
                self.archive.staging_receipt.payload.issued_at_ms,
            )
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        if self.archive.archive_id != request.archive_commitment.archive_id()
            || &self.archive.commitment != &request.archive_commitment
            || registered_binding != &staging_receipt.payload.binding
            || &registered_binding.chain_id != &request.chain_id
            || registered_binding.genesis_block_hash != request.genesis_block_hash
            || registered_binding.semantic_release_manifest_digest
                != request.publication.manifest.semantic_digest()
            || self.location_id.is_zero()
            || self.pin_manifest.as_bytes().iter().all(|byte| *byte == 0)
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.renew_after_epoch >= self.expires_at_epoch
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason:
                    "archive registration, operation-bound staging receipt, or permanent pin was substituted"
                        .to_owned(),
            });
        }
        Ok(())
    }
}

/// Exact public evidence returned after one provider readback.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReadbackEvidenceV1 {
    /// Provider through which the complete archive was read back.
    pub provider: ProviderId,
    /// Finalized location used for the readback.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Replication order whose completion authorized this provider.
    pub replication_order: ReplicationOrderId,
    /// Exact commitment reproduced by parsing and verifying the returned CAR.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Semantic release digest parsed from the returned canonical bundle.
    pub semantic_release_digest: MusubiSemanticReleaseDigestV1,
    /// Verification-lock digest parsed from the returned canonical bundle.
    pub verification_lock_digest: MusubiVerificationLockDigestV1,
}

impl PublicationReadbackEvidenceV1 {
    fn validate_for(
        &self,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        expected_provider: ProviderId,
    ) -> Result<(), PublicationError> {
        if self.provider != expected_provider
            || self.location_id != location.location_id
            || self.replication_order != location.replication_order
            || &self.commitment != &request.archive_commitment
            || self.semantic_release_digest != request.publication.manifest.semantic_digest()
            || self.verification_lock_digest
                != request.publication.manifest.verification_lock_digest
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Readback,
                reason: "provider readback evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }
}

/// Idempotent Native AMX submission result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationAmxSubmissionV1 {
    /// Operation identifier passed to the backend idempotency boundary.
    pub operation_id: PublicationOperationIdV1,
    /// Digest of the exact [`PublishMusubiReleaseV1`] instruction accepted by AMX.
    pub instruction_digest: [u8; 32],
    /// Submitted transaction hash.
    pub transaction_hash: [u8; 32],
}

impl PublicationAmxSubmissionV1 {
    /// Bind a committed transaction hash to the exact idempotent publish instruction.
    #[must_use]
    pub fn new(
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
        transaction_hash: [u8; 32],
    ) -> Self {
        Self {
            operation_id,
            instruction_digest: domain_hash(PUBLISH_INSTRUCTION_DOMAIN, &instruction.encode()),
            transaction_hash,
        }
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<(), PublicationError> {
        if self.operation_id != operation_id
            || self.instruction_digest
                != domain_hash(PUBLISH_INSTRUCTION_DOMAIN, &instruction.encode())
            || self.transaction_hash.iter().all(|byte| *byte == 0)
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "Native AMX submission evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }
}

/// Exact finalized home-dataspace and universal-index publication result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationFinalEvidenceV1 {
    /// Deployment-selected chain identity returned with the universal-index row.
    pub chain_id: ChainId,
    /// Exact genesis block hash returned with the universal-index row.
    pub genesis_block_hash: [u8; 32],
    /// Finalized universal registry snapshot used for the exact verification.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Exact authoritative release record in the stable home dataspace.
    pub home_release: MusubiReleaseRecordV1,
    /// Exact compact release row in the universal sparse index.
    pub universal_release: MusubiResolverReleaseRowV1,
}

impl PublicationFinalEvidenceV1 {
    fn validate_for(&self, request: &PublicationRequestV1) -> Result<(), PublicationError> {
        self.snapshot
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        self.home_release
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        self.universal_release
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        let manifest = &request.publication.manifest;
        let row = &self.universal_release;
        if self.chain_id != request.chain_id
            || self.genesis_block_hash != request.genesis_block_hash
            || self.snapshot.finalized_height
                < request.publication.resolution.snapshot.finalized_height
            || self.snapshot.index_revision < request.publication.resolution.snapshot.index_revision
            || &self.home_release.manifest != manifest
            || self.home_release.release_digest != manifest.release_digest()
            || &self.home_release.published_by != &request.publisher
            || self.home_release.published_at_height > self.snapshot.finalized_height
            || &row.release != &manifest.release
            || row.release_digest != manifest.release_digest()
            || row.archive_id != manifest.archive_id
            || row.source_digest != request.archive_commitment.source_tree_digest
            || row.interface_digest != manifest.interface_digest
            || row.abi != manifest.abi
            || &row.dependencies != &manifest.dependencies
            || row.index_revision != self.snapshot.index_revision
            || row.selection.storage.index_revision != row.index_revision
            || row.selection.storage.archive_id != manifest.archive_id
            || row.selection.storage.availability != MusubiStorageAvailabilityV1::Selectable
            || row.selection.storage.healthy_replicas < MUSUBI_MIN_HEALTHY_REPLICAS_V1
            || row.selection.storage.finalized_height > self.snapshot.finalized_height
            || row.selection.yank != self.home_release.yank
            || row.selection.governance != self.home_release.artifact_governance
            || !row.selection.fresh_selectable()
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::FinalVerification,
                reason: "finalized home record or universal resolver entry was substituted"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Completed publication result returned by ordinary publish or resume.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationResultV1 {
    /// Stable operation identifier.
    pub operation_id: PublicationOperationIdV1,
    /// Final Native AMX submission.
    pub submission: PublicationAmxSubmissionV1,
    /// Exact finalized registry evidence.
    pub final_evidence: PublicationFinalEvidenceV1,
}

/// Durable, secret-free operation journal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationJournalV1 {
    /// Fixed schema marker.
    pub schema: String,
    /// Fixed first-release schema version.
    pub version: u8,
    /// Monotonic local compare-and-set revision.
    pub revision: u64,
    /// Stable idempotency identifier derived from the request.
    pub operation_id: PublicationOperationIdV1,
    /// Immutable public request.
    pub request: PublicationRequestV1,
    /// Current phase; a completed journal remains at `FinalVerification`.
    pub phase: PublicationPhaseV1,
    /// Successful clean-package validation evidence.
    pub validation: Option<PublicationValidationEvidenceV1>,
    /// Authenticated seed-ingress receipt.
    pub staging_receipt: Option<MusubiSeedIngressReceiptV1>,
    /// Finalized archive record and permanent pin/order identifiers.
    pub archive_registration: Option<PublicationArchiveRegistrationV1>,
    /// Finalized healthy location with provider attestations.
    pub replication: Option<MusubiArchiveLocationV1>,
    /// Two distinct provider readback results.
    pub readbacks: Vec<PublicationReadbackEvidenceV1>,
    /// Idempotent Native AMX submission result.
    pub submission: Option<PublicationAmxSubmissionV1>,
    /// Present only after exact finalized home/index verification.
    pub completion: Option<PublicationFinalEvidenceV1>,
}

impl PublicationJournalV1 {
    fn new(request: PublicationRequestV1) -> Result<Self, PublicationError> {
        request.validate()?;
        let operation_id = request.operation_id();
        Ok(Self {
            schema: JOURNAL_SCHEMA.to_owned(),
            version: JOURNAL_VERSION,
            revision: 1,
            operation_id,
            request,
            phase: PublicationPhaseV1::Validation,
            validation: None,
            staging_receipt: None,
            archive_registration: None,
            replication: None,
            readbacks: Vec::new(),
            submission: None,
            completion: None,
        })
    }

    /// Validate schema, operation identity, exact evidence, and phase monotonicity.
    pub fn validate(&self) -> Result<(), PublicationError> {
        if self.schema != JOURNAL_SCHEMA
            || self.version != JOURNAL_VERSION
            || self.revision == 0
            || self.operation_id.is_zero()
        {
            return Err(PublicationError::InvalidJournal(
                "journal schema, version, revision, or operation id is invalid".to_owned(),
            ));
        }
        self.request.validate()?;
        if self.operation_id != self.request.operation_id() {
            return Err(PublicationError::InvalidJournal(
                "journal operation id does not bind its immutable request".to_owned(),
            ));
        }
        let required = self.phase as u8;
        validate_option(
            required >= PublicationPhaseV1::SeedIngress as u8,
            &self.validation,
        )?;
        validate_option(
            required >= PublicationPhaseV1::ArchiveRegistration as u8,
            &self.staging_receipt,
        )?;
        validate_option(
            required >= PublicationPhaseV1::Replication as u8,
            &self.archive_registration,
        )?;
        validate_option(
            required >= PublicationPhaseV1::Readback as u8,
            &self.replication,
        )?;
        let expects_readbacks = required >= PublicationPhaseV1::ReleaseSubmission as u8;
        if self.readbacks.len() != if expects_readbacks { 2 } else { 0 } {
            return Err(PublicationError::InvalidJournal(
                "journal readback count is inconsistent with its phase".to_owned(),
            ));
        }
        validate_option(
            required >= PublicationPhaseV1::FinalVerification as u8,
            &self.submission,
        )?;
        if self.completion.is_some() && self.phase != PublicationPhaseV1::FinalVerification {
            return Err(PublicationError::InvalidJournal(
                "journal completion is present before final verification".to_owned(),
            ));
        }

        if let Some(validation) = &self.validation {
            validation.validate_for(&self.request)?;
        }
        if let Some(receipt) = &self.staging_receipt {
            receipt
                .verify(
                    &self.request.receipt_binding(),
                    receipt.payload.issued_at_ms,
                )
                .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))?;
        }
        if let Some(registration) = &self.archive_registration {
            registration.validate_for(
                &self.request,
                self.staging_receipt.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "archive registration is missing its operation-bound staging receipt"
                            .to_owned(),
                    )
                })?,
            )?;
        }
        if let Some(location) = &self.replication {
            validate_replication(&self.request, self.registration()?, location)?;
        }
        if let Some(location) = &self.replication {
            for (readback, provider) in self.readbacks.iter().zip(location.providers.iter()) {
                readback.validate_for(&self.request, location, *provider)?;
            }
        }
        if let Some(submission) = &self.submission {
            submission.validate_for(self.operation_id, &self.request.publish_instruction())?;
        }
        if let Some(completion) = &self.completion {
            completion.validate_for(&self.request)?;
        }
        Ok(())
    }

    fn registration(&self) -> Result<&PublicationArchiveRegistrationV1, PublicationError> {
        self.archive_registration.as_ref().ok_or_else(|| {
            PublicationError::InvalidJournal("journal is missing archive registration".to_owned())
        })
    }

    /// Convert a completed journal into its stable publication result.
    pub fn result(&self) -> Option<PublicationResultV1> {
        Some(PublicationResultV1 {
            operation_id: self.operation_id,
            submission: self.submission?,
            final_evidence: self.completion.clone()?,
        })
    }
}

fn validate_option<T>(required: bool, value: &Option<T>) -> Result<(), PublicationError> {
    if required != value.is_some() {
        return Err(PublicationError::InvalidJournal(
            "journal evidence presence is inconsistent with its phase".to_owned(),
        ));
    }
    Ok(())
}

/// Runtime-only source of exact CAR bytes.
pub trait PublicationCarSource {
    /// Open a new reader at byte zero. Implementations must not persist their path or credentials.
    fn open_car(&self) -> io::Result<Box<dyn Read + '_>>;
}

/// Deterministic operation-local CAR source stored beside, but never inside, the secret-free journal.
#[derive(Clone, Debug)]
pub struct PublicationStagedCarSourceV1 {
    path: PathBuf,
    expected_size: u64,
}

impl PublicationStagedCarSourceV1 {
    /// Bind the immutable CAR location for one operation below an explicit user state root.
    #[must_use]
    pub fn new(
        user_state_root: &Path,
        operation_id: PublicationOperationIdV1,
        expected_size: u64,
    ) -> Self {
        Self {
            path: user_state_root.join(staged_car_relative_path(operation_id)),
            expected_size,
        }
    }

    /// Return the deterministic operation-local path without persisting it in the journal.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Durably stage exact CAR bytes below the private operation directory.
    ///
    /// Identical retries reuse an already verified regular file. A different body at the same
    /// operation id, an unsafe filesystem entry, or bytes that disagree with the request
    /// commitment fail closed without replacing the existing path.
    pub fn stage_bytes(
        user_state_root: &Path,
        operation_id: PublicationOperationIdV1,
        expected_size: u64,
        expected_digest: MusubiContentDigestV1,
        bytes: &[u8],
    ) -> Result<Self, PublicationError> {
        if expected_size == 0
            || expected_size > MUSUBI_MAX_CAR_BYTES_V1
            || u64::try_from(bytes.len()).ok() != Some(expected_size)
            || blake3::hash(bytes).as_bytes() != expected_digest.as_bytes()
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "staged CAR bytes do not match their bounded request commitment".to_owned(),
            });
        }
        let source = Self::new(user_state_root, operation_id, expected_size);
        match fs::symlink_metadata(source.path()) {
            Ok(_) => {
                source.verify_digest(expected_digest)?;
                return Ok(source);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(PublicationError::CarSource(error)),
        }
        let root = AtomicWriteRoot::new(user_state_root).map_err(PublicationError::JournalWrite)?;
        root.replace(&staged_car_relative_path(operation_id), bytes)
            .map_err(PublicationError::JournalWrite)?;
        source.verify_digest(expected_digest)?;
        Ok(source)
    }

    fn verify_digest(
        &self,
        expected_digest: MusubiContentDigestV1,
    ) -> Result<(), PublicationError> {
        let mut reader = self.open_car().map_err(PublicationError::CarSource)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut observed = 0_u64;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(PublicationError::CarSource)?;
            if read == 0 {
                break;
            }
            observed = observed
                .checked_add(u64::try_from(read).expect("read buffer length fits u64"))
                .ok_or_else(|| {
                    PublicationError::InvalidJournal("staged CAR length overflowed".to_owned())
                })?;
            if observed > self.expected_size {
                return Err(PublicationError::InvalidJournal(
                    "staged CAR grew while it was verified".to_owned(),
                ));
            }
            hasher.update(&buffer[..read]);
        }
        if observed != self.expected_size
            || hasher.finalize().as_bytes() != expected_digest.as_bytes()
        {
            return Err(PublicationError::InvalidJournal(
                "existing staged CAR differs from the immutable publication request".to_owned(),
            ));
        }
        Ok(())
    }
}

impl PublicationCarSource for PublicationStagedCarSourceV1 {
    fn open_car(&self) -> io::Result<Box<dyn Read + '_>> {
        let inspected = fs::symlink_metadata(&self.path)?;
        if self.expected_size == 0
            || self.expected_size > MUSUBI_MAX_CAR_BYTES_V1
            || !metadata_is_safe_regular_file(&inspected)
            || inspected.len() != self.expected_size
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged publication CAR is not the expected bounded regular file",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        options.share_mode(FILE_SHARE_READ);
        set_no_follow(&mut options);
        let file = options.open(&self.path)?;
        let opened = file.metadata()?;
        let linked_after = fs::symlink_metadata(&self.path)?;
        if !metadata_is_safe_regular_file(&opened)
            || !metadata_is_safe_regular_file(&linked_after)
            || !same_file_snapshot(&inspected, &opened)
            || !same_file_snapshot(&opened, &linked_after)
            || opened.len() != self.expected_size
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged publication CAR changed while it was opened",
            ));
        }
        Ok(Box::new(StableCarReader {
            path: self.path.clone(),
            file,
            initial: opened,
            expected_size: self.expected_size,
            observed: 0,
            complete: false,
        }))
    }
}

struct StableCarReader {
    path: PathBuf,
    file: File,
    initial: fs::Metadata,
    expected_size: u64,
    observed: u64,
    complete: bool,
}

impl StableCarReader {
    fn validate_complete_snapshot(&self) -> io::Result<()> {
        let opened = self.file.metadata()?;
        let linked = fs::symlink_metadata(&self.path)?;
        if !metadata_is_safe_regular_file(&opened)
            || !metadata_is_safe_regular_file(&linked)
            || !same_file_snapshot(&self.initial, &opened)
            || !same_file_snapshot(&opened, &linked)
            || opened.len() != self.expected_size
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged publication CAR changed while it was read",
            ));
        }
        Ok(())
    }
}

impl Read for StableCarReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.complete || buffer.is_empty() {
            return Ok(0);
        }
        let read = self.file.read(buffer)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "staged publication CAR ended before its committed length",
            ));
        }
        self.observed = self
            .observed
            .checked_add(u64::try_from(read).expect("read length fits u64"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CAR length overflowed"))?;
        if self.observed > self.expected_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staged publication CAR exceeded its committed length",
            ));
        }
        if self.observed == self.expected_size {
            self.validate_complete_snapshot()?;
            self.complete = true;
        }
        Ok(read)
    }
}

/// Classification for a transport or remote-state failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationBackendFailureClass {
    /// Retrying the same idempotent transition may succeed.
    Retryable,
    /// Retrying without changing external state cannot succeed.
    Permanent,
}

/// Redacted backend failure carrying only a stable public code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationBackendError {
    class: PublicationBackendFailureClass,
    code: String,
}

impl PublicationBackendError {
    /// Construct a retryable backend failure from a bounded stable code.
    #[must_use]
    pub fn retryable(code: impl Into<String>) -> Self {
        Self::new(PublicationBackendFailureClass::Retryable, code)
    }

    /// Construct a permanent backend failure from a bounded stable code.
    #[must_use]
    pub fn permanent(code: impl Into<String>) -> Self {
        Self::new(PublicationBackendFailureClass::Permanent, code)
    }

    fn new(class: PublicationBackendFailureClass, code: impl Into<String>) -> Self {
        let candidate = code.into();
        let code = if !candidate.is_empty()
            && candidate.len() <= 96
            && candidate
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        {
            candidate
        } else {
            "MUSUBI_BACKEND_FAILURE".to_owned()
        };
        Self { class, code }
    }

    /// Return whether the same idempotent call may be retried.
    #[must_use]
    pub const fn class(&self) -> PublicationBackendFailureClass {
        self.class
    }

    /// Return the stable, redacted failure code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }
}

impl fmt::Display for PublicationBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.code)
    }
}

impl Error for PublicationBackendError {}

/// Runtime publication adapter. Credentials and endpoints remain inside this object.
pub trait PublicationBackend {
    /// Return current Unix time in milliseconds for receipt validation.
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError>;

    /// Validate and compiler-check the clean CAR and exact dependency graph.
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError>;

    /// Stage the CAR only through an admitted, authenticated seed-ingress service.
    fn stage_authenticated_seed_ingress(
        &mut self,
        operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError>;

    /// Idempotently register/reuse the archive and create/reuse its permanent pin and order.
    /// The returned archive must retain a verified receipt with the supplied operation binding.
    fn ensure_archive_and_permanent_pin(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError>;

    /// Poll finalized provider completions and return a healthy location at quorum.
    fn finalized_replication(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<Option<MusubiArchiveLocationV1>, PublicationBackendError>;

    /// Read and fully verify the archive through one selected finalized provider.
    fn readback_provider(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError>;

    /// Submit or recover the exact release claim through Native AMX.
    fn submit_release_native_amx(
        &mut self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError>;

    /// Poll finality and query both the exact home record and exact universal index row.
    fn finalized_release_and_index(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError>;
}

/// One step of a resumable publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationAdvanceV1 {
    /// A durable transition completed and the journal now names this phase.
    Progressed(PublicationPhaseV1),
    /// Finalized external state is not ready; retry from the unchanged phase.
    Pending(PublicationPhaseV1),
    /// Exact home and universal-index verification completed.
    Complete(PublicationResultV1),
}

/// Durable operation-journal storage below an explicit user-level state root.
#[derive(Debug)]
pub struct PublicationJournalStore {
    root: AtomicWriteRoot,
}

struct PublicationOperationLockV1 {
    file: File,
    path: PathBuf,
    identity: fs::Metadata,
    parent: File,
    parent_path: PathBuf,
    parent_identity: fs::Metadata,
}

impl PublicationOperationLockV1 {
    fn validate(&self) -> Result<(), PublicationError> {
        let opened = self.file.metadata().map_err(PublicationError::JournalIo)?;
        let named = fs::symlink_metadata(&self.path).map_err(PublicationError::JournalIo)?;
        let parent_opened = self
            .parent
            .metadata()
            .map_err(PublicationError::JournalIo)?;
        let parent_named =
            fs::symlink_metadata(&self.parent_path).map_err(PublicationError::JournalIo)?;
        if !operation_lock_metadata_is_safe(&opened, &self.parent_identity)
            || !operation_lock_metadata_is_safe(&named, &self.parent_identity)
            || !same_file_snapshot(&self.identity, &opened)
            || !same_file_snapshot(&opened, &named)
            || !same_directory(&self.parent_identity, &parent_opened)
            || !same_directory(&parent_opened, &parent_named)
        {
            return Err(PublicationError::InvalidJournal(
                "publication operation lock changed identity".to_owned(),
            ));
        }
        Ok(())
    }

    fn finish<T>(self, result: Result<T, PublicationError>) -> Result<T, PublicationError> {
        let unlock = File::unlock(&self.file).map_err(PublicationError::JournalIo);
        match (result, unlock) {
            (Err(error), _) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) => Err(error),
        }
    }
}

impl PublicationJournalStore {
    /// Open or create the private `publication-v1` journal directory.
    pub fn open(user_state_root: &Path) -> Result<Self, PublicationError> {
        let root = AtomicWriteRoot::new(user_state_root).map_err(PublicationError::JournalWrite)?;
        let journal_directory = root.path().join(JOURNAL_DIRECTORY);
        let created = match fs::create_dir(&journal_directory) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    fs::set_permissions(&journal_directory, fs::Permissions::from_mode(0o700))
                        .map_err(PublicationError::JournalIo)?;
                }
                true
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(PublicationError::JournalIo(error)),
        };
        let metadata =
            fs::symlink_metadata(&journal_directory).map_err(PublicationError::JournalIo)?;
        if !journal_directory_metadata_is_safe(&metadata) {
            return Err(PublicationError::InvalidJournal(
                "publication journal directory is not a private real directory".to_owned(),
            ));
        }
        if created {
            File::open(root.path())
                .and_then(|directory| directory.sync_all())
                .map_err(PublicationError::JournalIo)?;
        }
        Ok(Self { root })
    }

    /// Persist a new operation, or return the identical existing operation idempotently.
    pub fn create(
        &self,
        request: PublicationRequestV1,
    ) -> Result<PublicationJournalV1, PublicationError> {
        let journal = PublicationJournalV1::new(request)?;
        let operation_lock = self.lock_operation(journal.operation_id)?;
        let result = (|| {
            match self.load(journal.operation_id) {
                Ok(existing) if existing.request == journal.request => return Ok(existing),
                Ok(_) => {
                    return Err(PublicationError::InvalidJournal(
                        "operation id collision has different immutable request bytes".to_owned(),
                    ));
                }
                Err(PublicationError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
            operation_lock.validate()?;
            self.write(&journal)?;
            operation_lock.validate()?;
            Ok(journal)
        })();
        operation_lock.finish(result)
    }

    /// Load and fully validate one journal by typed operation id.
    pub fn load(
        &self,
        operation_id: PublicationOperationIdV1,
    ) -> Result<PublicationJournalV1, PublicationError> {
        let relative = journal_relative_path(operation_id);
        let path = self.root.path().join(relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(PublicationError::NotFound(operation_id));
            }
            Err(error) => return Err(PublicationError::JournalIo(error)),
        };
        if !metadata_is_safe_regular_file(&metadata) || metadata.len() > MAX_JOURNAL_BYTES {
            return Err(PublicationError::InvalidJournal(
                "journal is not a bounded regular file".to_owned(),
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        options.share_mode(FILE_SHARE_READ);
        set_no_follow(&mut options);
        let mut file = options.open(&path).map_err(PublicationError::JournalIo)?;
        let opened = file.metadata().map_err(PublicationError::JournalIo)?;
        if !metadata_is_safe_regular_file(&opened) || !same_file_snapshot(&metadata, &opened) {
            return Err(PublicationError::InvalidJournal(
                "journal changed while it was opened".to_owned(),
            ));
        }
        let capacity = usize::try_from(metadata.len()).map_err(|_| {
            PublicationError::InvalidJournal("journal length does not fit memory".to_owned())
        })?;
        let mut bytes = Vec::with_capacity(capacity);
        file.by_ref()
            .take(MAX_JOURNAL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(PublicationError::JournalIo)?;
        if bytes.len() > MAX_JOURNAL_BYTES_USIZE {
            return Err(PublicationError::InvalidJournal(
                "journal grew beyond its fixed size bound while it was read".to_owned(),
            ));
        }
        let opened_after = file.metadata().map_err(PublicationError::JournalIo)?;
        let linked_after = fs::symlink_metadata(&path).map_err(PublicationError::JournalIo)?;
        if bytes.len() as u64 != metadata.len()
            || !metadata_is_safe_regular_file(&opened_after)
            || !metadata_is_safe_regular_file(&linked_after)
            || !same_file_snapshot(&metadata, &opened_after)
            || !same_file_snapshot(&opened_after, &linked_after)
        {
            return Err(PublicationError::InvalidJournal(
                "journal length changed while it was read".to_owned(),
            ));
        }
        let journal = decode_publication_journal(&bytes)?;
        if journal.operation_id != operation_id {
            return Err(PublicationError::InvalidJournal(
                "journal filename and encoded operation id differ".to_owned(),
            ));
        }
        journal.validate()?;
        Ok(journal)
    }

    fn write(&self, journal: &PublicationJournalV1) -> Result<(), PublicationError> {
        journal.validate()?;
        let bytes = norito::encode_canonical(journal).map_err(|error| {
            PublicationError::InvalidJournal(format!(
                "journal could not be canonically encoded: {error}"
            ))
        })?;
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(PublicationError::InvalidJournal(
                "journal exceeds its fixed size bound".to_owned(),
            ));
        }
        self.root
            .replace(&journal_relative_path(journal.operation_id), &bytes)
            .map_err(PublicationError::JournalWrite)
    }

    fn transition(
        &self,
        previous: &PublicationJournalV1,
        mut next: PublicationJournalV1,
    ) -> Result<PublicationJournalV1, PublicationError> {
        let operation_lock = self.lock_operation(previous.operation_id)?;
        let result = (|| {
            let current = self.load(previous.operation_id)?;
            if current.revision != previous.revision || current != *previous {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            next.revision = previous.revision.checked_add(1).ok_or_else(|| {
                PublicationError::InvalidJournal("journal revision overflowed".to_owned())
            })?;
            operation_lock.validate()?;
            self.write(&next)?;
            operation_lock.validate()?;
            let persisted = self.load(next.operation_id)?;
            if persisted != next {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            Ok(persisted)
        })();
        operation_lock.finish(result)
    }

    fn lock_operation(
        &self,
        operation_id: PublicationOperationIdV1,
    ) -> Result<PublicationOperationLockV1, PublicationError> {
        let parent_path = self.root.path().join(JOURNAL_DIRECTORY);
        let parent_before =
            fs::symlink_metadata(&parent_path).map_err(PublicationError::JournalIo)?;
        if parent_before.file_type().is_symlink() || !parent_before.is_dir() {
            return Err(PublicationError::InvalidJournal(
                "publication journal directory is not a real directory".to_owned(),
            ));
        }
        let parent = File::open(&parent_path).map_err(PublicationError::JournalIo)?;
        let parent_opened = parent.metadata().map_err(PublicationError::JournalIo)?;
        let parent_named =
            fs::symlink_metadata(&parent_path).map_err(PublicationError::JournalIo)?;
        if !same_directory(&parent_before, &parent_opened)
            || !same_directory(&parent_opened, &parent_named)
        {
            return Err(PublicationError::InvalidJournal(
                "publication journal directory changed identity".to_owned(),
            ));
        }

        let path = self
            .root
            .path()
            .join(operation_lock_relative_path(operation_id));
        let before = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !operation_lock_metadata_is_safe(&metadata, &parent_opened) {
                    return Err(PublicationError::InvalidJournal(
                        "publication operation lock is not a private empty regular file".to_owned(),
                    ));
                }
                Some(metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(PublicationError::JournalIo(error)),
        };

        let (file, created) = match before.as_ref() {
            Some(_) => (open_existing_operation_lock(&path)?, false),
            None => match create_operation_lock(&path) {
                Ok(file) => (file, true),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    (open_existing_operation_lock(&path)?, false)
                }
                Err(error) => return Err(PublicationError::JournalIo(error)),
            },
        };
        if created {
            #[cfg(unix)]
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(PublicationError::JournalIo)?;
            file.sync_all().map_err(PublicationError::JournalIo)?;
            parent.sync_all().map_err(PublicationError::JournalIo)?;
        }
        let opened = file.metadata().map_err(PublicationError::JournalIo)?;
        let named = fs::symlink_metadata(&path).map_err(PublicationError::JournalIo)?;
        if !operation_lock_metadata_is_safe(&opened, &parent_opened)
            || !operation_lock_metadata_is_safe(&named, &parent_opened)
            || before
                .as_ref()
                .is_some_and(|metadata| !same_file_snapshot(metadata, &opened))
            || !same_file_snapshot(&opened, &named)
        {
            return Err(PublicationError::InvalidJournal(
                "publication operation lock changed while it was opened".to_owned(),
            ));
        }
        file.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => PublicationError::ConcurrentJournalUpdate,
            fs::TryLockError::Error(error) => PublicationError::JournalIo(error),
        })?;
        let operation_lock = PublicationOperationLockV1 {
            file,
            path,
            identity: opened,
            parent,
            parent_path,
            parent_identity: parent_opened,
        };
        operation_lock.validate()?;
        Ok(operation_lock)
    }
}

/// Resumable publication coordinator.
#[derive(Debug)]
pub struct PublicationEngine<'a> {
    store: &'a PublicationJournalStore,
}

impl<'a> PublicationEngine<'a> {
    /// Bind an engine to a durable user-level journal store.
    #[must_use]
    pub const fn new(store: &'a PublicationJournalStore) -> Self {
        Self { store }
    }

    /// Persist a detached operation and return its resumable identifier.
    pub fn begin_detached(
        &self,
        request: PublicationRequestV1,
    ) -> Result<PublicationOperationIdV1, PublicationError> {
        self.store
            .create(request)
            .map(|journal| journal.operation_id)
    }

    /// Start or idempotently recover an operation, running until finality or a pending poll.
    pub fn publish(
        &self,
        request: PublicationRequestV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let journal = self.store.create(request)?;
        self.run(journal.operation_id, source, backend)
    }

    /// Resume an operation by id and run until finality or a pending poll.
    pub fn resume(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        self.run(operation_id, source, backend)
    }

    /// Advance exactly one durable phase, making retries observable to callers.
    pub fn advance_once(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let journal = self.store.load(operation_id)?;
        if let Some(result) = journal.result() {
            return Ok(PublicationAdvanceV1::Complete(result));
        }
        let phase = journal.phase;
        let mut next = journal.clone();
        match phase {
            PublicationPhaseV1::Validation => {
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let evidence = backend
                    .validate_clean_package(operation_id, &journal.request, car.as_mut())
                    .map_err(PublicationError::Backend)?;
                evidence.validate_for(&journal.request)?;
                next.validation = Some(evidence);
                next.phase = PublicationPhaseV1::SeedIngress;
            }
            PublicationPhaseV1::SeedIngress => {
                let expected = journal.request.receipt_binding();
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let receipt = backend
                    .stage_authenticated_seed_ingress(operation_id, &expected, car.as_mut())
                    .map_err(PublicationError::Backend)?;
                let now = backend
                    .current_time_ms()
                    .map_err(PublicationError::Backend)?;
                receipt
                    .verify(&expected, now)
                    .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))?;
                next.staging_receipt = Some(receipt);
                next.phase = PublicationPhaseV1::ArchiveRegistration;
            }
            PublicationPhaseV1::ArchiveRegistration => {
                let receipt = journal.staging_receipt.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing staging receipt".to_owned())
                })?;
                let now = backend
                    .current_time_ms()
                    .map_err(PublicationError::Backend)?;
                if now < receipt.payload.issued_at_ms || now > receipt.payload.expires_at_ms {
                    next.staging_receipt = None;
                    next.phase = PublicationPhaseV1::SeedIngress;
                } else {
                    let registration = backend
                        .ensure_archive_and_permanent_pin(operation_id, &journal.request, receipt)
                        .map_err(PublicationError::Backend)?;
                    registration.validate_for(&journal.request, receipt)?;
                    next.archive_registration = Some(registration);
                    next.phase = PublicationPhaseV1::Replication;
                }
            }
            PublicationPhaseV1::Replication => {
                let registration = journal.registration()?;
                let Some(location) = backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                else {
                    return Ok(PublicationAdvanceV1::Pending(phase));
                };
                validate_replication(&journal.request, registration, &location)?;
                next.replication = Some(location);
                next.phase = PublicationPhaseV1::Readback;
            }
            PublicationPhaseV1::Readback => {
                let location = journal.replication.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing finalized replication".to_owned())
                })?;
                let providers = location.providers.get(..2).ok_or_else(|| {
                    PublicationError::InvalidEvidence {
                        phase,
                        reason: "fewer than two finalized providers are available".to_owned(),
                    }
                })?;
                let mut readbacks = Vec::with_capacity(2);
                for provider in providers {
                    let evidence = backend
                        .readback_provider(operation_id, &journal.request, location, *provider)
                        .map_err(PublicationError::Backend)?;
                    evidence.validate_for(&journal.request, location, *provider)?;
                    readbacks.push(evidence);
                }
                if readbacks[0].provider == readbacks[1].provider {
                    return Err(PublicationError::InvalidEvidence {
                        phase,
                        reason: "readbacks did not use two distinct providers".to_owned(),
                    });
                }
                next.readbacks = readbacks;
                next.phase = PublicationPhaseV1::ReleaseSubmission;
            }
            PublicationPhaseV1::ReleaseSubmission => {
                let instruction = journal.request.publish_instruction();
                let submission = backend
                    .submit_release_native_amx(operation_id, &instruction)
                    .map_err(PublicationError::Backend)?;
                submission.validate_for(operation_id, &instruction)?;
                next.submission = Some(submission);
                next.phase = PublicationPhaseV1::FinalVerification;
            }
            PublicationPhaseV1::FinalVerification => {
                let submission = journal.submission.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing Native AMX submission".to_owned())
                })?;
                let Some(final_evidence) = backend
                    .finalized_release_and_index(operation_id, &journal.request, submission)
                    .map_err(PublicationError::Backend)?
                else {
                    return Ok(PublicationAdvanceV1::Pending(phase));
                };
                final_evidence.validate_for(&journal.request)?;
                next.completion = Some(final_evidence);
            }
        }
        let persisted = self.store.transition(&journal, next)?;
        if let Some(result) = persisted.result() {
            Ok(PublicationAdvanceV1::Complete(result))
        } else {
            Ok(PublicationAdvanceV1::Progressed(persisted.phase))
        }
    }

    fn run(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        loop {
            match self.advance_once(operation_id, source, backend)? {
                PublicationAdvanceV1::Progressed(_) => {}
                terminal => return Ok(terminal),
            }
        }
    }
}

fn validate_replication(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    location: &MusubiArchiveLocationV1,
) -> Result<(), PublicationError> {
    location
        .validate()
        .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
    if location.archive_id != request.archive_commitment.archive_id()
        || location.location_id != registration.location_id
        || location.pin_manifest != registration.pin_manifest
        || location.replication_order != registration.replication_order
        || location.renew_after_epoch != registration.renew_after_epoch
        || location.expires_at_epoch != registration.expires_at_epoch
        || location.state != MusubiArchiveLocationStateV1::Healthy
        || location.providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
        || location.finalized_height < registration.archive.registered_at_height
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "finalized archive location, pin, order, or quorum was substituted".to_owned(),
        });
    }
    let manifest = &request.publication.manifest;
    for attestation in &location.provider_attestations {
        let binding = &attestation.payload.binding;
        if &binding.chain_id != &request.chain_id
            || binding.genesis_block_hash != request.genesis_block_hash
            || binding.archive_id != request.archive_commitment.archive_id()
            || binding.bundle_digest != request.archive_commitment.bundle_digest
            || binding.descriptor_digest != request.archive_commitment.descriptor_digest
            || binding.semantic_release_manifest_digest != manifest.semantic_digest()
            || binding.verification_lock_digest != manifest.verification_lock_digest
            || binding.source_tree_digest != request.archive_commitment.source_tree_digest
            || binding.finalized_anchor.height > location.finalized_height
            || binding.completion_epoch >= location.expires_at_epoch
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "provider bundle attestation was substituted".to_owned(),
            });
        }
        attestation
            .verify(binding)
            .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
    }
    Ok(())
}

/// Publication workflow error with retry class preserved for backend failures.
#[derive(Debug)]
pub enum PublicationError {
    /// The CAR source could not be reopened or read.
    CarSource(io::Error),
    /// A backend transition failed without persisting secrets in the journal.
    Backend(PublicationBackendError),
    /// Signed, finalized, compiler, or readback evidence did not exactly match the request.
    InvalidEvidence {
        /// Phase that rejected the evidence.
        phase: PublicationPhaseV1,
        /// Public non-secret failure reason.
        reason: String,
    },
    /// A journal was malformed, inconsistent, unsafe, or noncanonical.
    InvalidJournal(String),
    /// Atomic durable journal replacement failed.
    JournalWrite(AtomicWriteError),
    /// A journal filesystem operation failed.
    JournalIo(io::Error),
    /// No journal exists for this typed operation id.
    NotFound(PublicationOperationIdV1),
    /// Another resume changed the journal between load and durable transition.
    ConcurrentJournalUpdate,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CarSource(error) => write!(formatter, "failed to open publication CAR: {error}"),
            Self::Backend(error) => write!(formatter, "publication backend failed: {error}"),
            Self::InvalidEvidence { phase, reason } => {
                write!(formatter, "invalid {phase:?} evidence: {reason}")
            }
            Self::InvalidJournal(reason) => {
                write!(formatter, "invalid publication journal: {reason}")
            }
            Self::JournalWrite(error) => {
                write!(formatter, "failed to write publication journal: {error}")
            }
            Self::JournalIo(error) => write!(formatter, "publication journal I/O failed: {error}"),
            Self::NotFound(operation_id) => {
                write!(
                    formatter,
                    "publication operation `{operation_id}` was not found"
                )
            }
            Self::ConcurrentJournalUpdate => {
                formatter.write_str("publication journal changed during a resumable transition")
            }
        }
    }
}

impl Error for PublicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CarSource(error) | Self::JournalIo(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::JournalWrite(error) => Some(error),
            Self::InvalidEvidence { .. }
            | Self::InvalidJournal(_)
            | Self::NotFound(_)
            | Self::ConcurrentJournalUpdate => None,
        }
    }
}

fn invalid(phase: PublicationPhaseV1, error: impl fmt::Display) -> PublicationError {
    PublicationError::InvalidEvidence {
        phase,
        reason: error.to_string(),
    }
}

fn decode_publication_journal(bytes: &[u8]) -> Result<PublicationJournalV1, PublicationError> {
    if bytes.is_empty() || bytes.len() > MAX_JOURNAL_BYTES_USIZE {
        return Err(PublicationError::InvalidJournal(
            "journal exceeds its fixed canonical frame bound".to_owned(),
        ));
    }
    norito::decode_canonical_with_limits(bytes, JOURNAL_DECODE_LIMITS).map_err(|error| {
        PublicationError::InvalidJournal(format!("journal is not canonical Norito: {error}"))
    })
}

fn journal_relative_path(operation_id: PublicationOperationIdV1) -> PathBuf {
    Path::new(JOURNAL_DIRECTORY).join(format!("{operation_id}.{JOURNAL_EXTENSION}"))
}

fn operation_lock_relative_path(operation_id: PublicationOperationIdV1) -> PathBuf {
    Path::new(JOURNAL_DIRECTORY).join(format!("{operation_id}.{JOURNAL_LOCK_EXTENSION}"))
}

fn staged_car_relative_path(operation_id: PublicationOperationIdV1) -> PathBuf {
    PathBuf::from(JOURNAL_DIRECTORY).join(format!("{operation_id}.{STAGED_CAR_EXTENSION}"))
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(
        &u64::try_from(domain.len())
            .expect("publication domain length fits u64")
            .to_le_bytes(),
    );
    hasher.update(domain);
    hasher.update(
        &u64::try_from(bytes.len())
            .expect("bounded publication payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn open_existing_operation_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    set_no_follow(&mut options);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    options.open(path)
}

fn create_operation_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    set_no_follow(&mut options);
    #[cfg(unix)]
    options.mode(0o600);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    options.open(path)
}

#[cfg(unix)]
fn operation_lock_metadata_is_safe(metadata: &fs::Metadata, parent: &fs::Metadata) -> bool {
    metadata_is_safe_regular_file(metadata)
        && metadata.len() == 0
        && metadata.permissions().mode() & 0o7777 == 0o600
        && metadata.uid() == parent.uid()
}

#[cfg(windows)]
fn operation_lock_metadata_is_safe(metadata: &fs::Metadata, _parent: &fs::Metadata) -> bool {
    metadata_is_safe_regular_file(metadata) && metadata.len() == 0
}

#[cfg(not(any(unix, windows)))]
const fn operation_lock_metadata_is_safe(_metadata: &fs::Metadata, _parent: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn journal_directory_metadata_is_safe(metadata: &fs::Metadata) -> bool {
    metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.permissions().mode() & 0o7777 == 0o700
}

#[cfg(windows)]
fn journal_directory_metadata_is_safe(metadata: &fs::Metadata) -> bool {
    metadata.is_dir() && !metadata_is_windows_reparse_point(metadata)
}

#[cfg(not(any(unix, windows)))]
const fn journal_directory_metadata_is_safe(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_directory(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    journal_directory_metadata_is_safe(left)
        && journal_directory_metadata_is_safe(right)
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.uid() == right.uid()
}

#[cfg(windows)]
fn same_directory(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    journal_directory_metadata_is_safe(left)
        && journal_directory_metadata_is_safe(right)
        && left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(not(any(unix, windows)))]
const fn same_directory(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn metadata_is_safe_regular_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && !metadata.file_type().is_symlink()
        && !metadata_is_windows_reparse_point(metadata)
        && metadata_has_one_hard_link(metadata)
}

#[cfg(windows)]
fn metadata_is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
const fn metadata_is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn metadata_has_one_hard_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    metadata.nlink() == 1
}

#[cfg(windows)]
fn metadata_has_one_hard_link(metadata: &fs::Metadata) -> bool {
    metadata.number_of_links() == Some(1)
}

#[cfg(not(any(unix, windows)))]
const fn metadata_has_one_hard_link(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.nlink() == 1
        && right.nlink() == 1
}

#[cfg(windows)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.file_type() == right.file_type()
        && left.file_attributes() == right.file_attributes()
        && left.file_size() == right.file_size()
        && left.creation_time() == right.creation_time()
        && left.last_write_time() == right.last_write_time()
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
}

#[cfg(not(any(unix, windows)))]
const fn same_file_snapshot(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn set_no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(platform_no_follow_flag());
    }
    #[cfg(windows)]
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    #[cfg(not(any(unix, windows)))]
    let _ = options;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[cfg(unix)]
    use std::io::Write as _;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;

    use iroha::{
        crypto::{Algorithm, KeyPair, SignatureOf},
        data_model::{
            musubi::{
                MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiArchiveAvailabilityV1,
                MusubiArtifactGovernanceStateV1, MusubiKotodamaEditionV1, MusubiPackageIdV1,
                MusubiPackageScopeV1, MusubiProviderBundleVerificationApprovalV1,
                MusubiProviderBundleVerificationAttestationV1,
                MusubiProviderBundleVerificationBindingV1,
                MusubiProviderBundleVerificationPayloadV1, MusubiReasonV1, MusubiReleaseIdV1,
                MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiReleaseRevisionsV1,
                MusubiReleaseSelectionStateV1, MusubiReleaseYankV1, MusubiResolutionProofV1,
                MusubiSeedIngressReceiptApprovalV1, MusubiSeedIngressReceiptPayloadV1,
                MusubiVerificationLockV1, MusubiVersionV1,
            },
            nexus::DataSpaceId,
            sorafs::pin_registry::{
                ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionAuthorityV1,
                ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
            },
        },
    };
    use tempfile::tempdir;

    use super::*;

    struct BytesSource(Vec<u8>);

    impl PublicationCarSource for BytesSource {
        fn open_car(&self) -> io::Result<Box<dyn Read + '_>> {
            Ok(Box::new(Cursor::new(self.0.as_slice())))
        }
    }

    #[test]
    fn staged_car_source_reopens_only_the_exact_operation_file() {
        let state = tempdir().expect("state root");
        fs::create_dir(state.path().join(JOURNAL_DIRECTORY)).expect("publication directory");
        let operation_id = "0101010101010101010101010101010101010101010101010101010101010101"
            .parse()
            .expect("operation id");
        let source = PublicationStagedCarSourceV1::new(state.path(), operation_id, 4);
        fs::write(source.path(), b"car!").expect("stage fixture CAR");

        let mut bytes = Vec::new();
        source
            .open_car()
            .expect("open exact CAR")
            .read_to_end(&mut bytes)
            .expect("read exact CAR");
        assert_eq!(bytes, b"car!");

        let wrong_size = PublicationStagedCarSourceV1::new(state.path(), operation_id, 5);
        assert_eq!(
            wrong_size
                .open_car()
                .err()
                .expect("wrong length must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(unix)]
    #[test]
    fn staged_car_reader_rejects_hard_links_and_in_place_growth() {
        let state = tempdir().expect("state root");
        fs::create_dir(state.path().join(JOURNAL_DIRECTORY)).expect("publication directory");
        let operation_id = "0404040404040404040404040404040404040404040404040404040404040404"
            .parse()
            .expect("operation id");
        let source = PublicationStagedCarSourceV1::new(state.path(), operation_id, 4);
        fs::write(source.path(), b"car!").expect("stage fixture CAR");
        let linked = state.path().join("linked.car");
        fs::hard_link(source.path(), &linked).expect("create hard link");
        assert_eq!(
            source
                .open_car()
                .err()
                .expect("hard-linked source rejected")
                .kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(linked).expect("remove fixture hard link");

        let mut reader = source.open_car().expect("open exact CAR");
        let mut prefix = [0_u8; 2];
        reader.read_exact(&mut prefix).expect("read prefix");
        OpenOptions::new()
            .append(true)
            .open(source.path())
            .expect("open source for mutation")
            .write_all(b"x")
            .expect("grow source");
        let mut remainder = Vec::new();
        let error = reader
            .read_to_end(&mut remainder)
            .expect_err("in-place growth rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn staged_car_bytes_are_commitment_checked_and_idempotent() {
        let state = tempdir().expect("state root");
        let _store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let operation_id = "0202020202020202020202020202020202020202020202020202020202020202"
            .parse()
            .expect("operation id");
        let bytes = b"canonical-car";
        let expected_size = u64::try_from(bytes.len()).expect("fixture length fits u64");
        let digest = MusubiContentDigestV1::new(*blake3::hash(bytes).as_bytes());
        let source = PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            expected_size,
            digest,
            bytes,
        )
        .expect("stage committed CAR");
        PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            expected_size,
            digest,
            bytes,
        )
        .expect("identical retry reuses staged CAR");

        fs::write(source.path(), b"substituted!!").expect("substitute same-length fixture");
        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                operation_id,
                expected_size,
                digest,
                bytes,
            ),
            Err(PublicationError::InvalidJournal(_))
        ));

        let other_id = "0303030303030303030303030303030303030303030303030303030303030303"
            .parse()
            .expect("other operation id");
        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                other_id,
                expected_size,
                MusubiContentDigestV1::new([9; 32]),
                bytes,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        assert!(
            !PublicationStagedCarSourceV1::new(state.path(), other_id, expected_size)
                .path()
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn journal_decode_rejects_trailing_bare_and_oversized_frames() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, _) = request();
        let operation_id = request.operation_id();
        let journal = store.create(request).expect("create canonical journal");
        let path = state.path().join(journal_relative_path(operation_id));
        let canonical = fs::read(&path).expect("read canonical journal");
        assert_eq!(
            decode_publication_journal(&canonical).expect("decode canonical journal"),
            journal
        );

        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open journal for trailing-byte injection")
            .write_all(&[0])
            .expect("append trailing byte");
        assert!(matches!(
            store.load(operation_id),
            Err(PublicationError::InvalidJournal(_))
        ));

        store
            .root
            .replace(&journal_relative_path(operation_id), &journal.encode())
            .expect("replace with legacy bare encoding");
        assert!(matches!(
            store.load(operation_id),
            Err(PublicationError::InvalidJournal(_))
        ));

        let oversized = vec![0_u8; MAX_JOURNAL_BYTES_USIZE + 1];
        assert!(matches!(
            decode_publication_journal(&oversized),
            Err(PublicationError::InvalidJournal(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn operation_lock_is_private_exclusive_and_rejects_hard_links() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, _) = request();
        let operation_id = request.operation_id();
        store.create(request).expect("create journal");
        let lock_path = state
            .path()
            .join(operation_lock_relative_path(operation_id));
        let metadata = fs::symlink_metadata(&lock_path).expect("operation lock metadata");
        assert_eq!(metadata.len(), 0);
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);

        let held = store
            .lock_operation(operation_id)
            .expect("hold operation lock");
        let second = PublicationJournalStore::open(state.path()).expect("second journal store");
        assert!(matches!(
            second.lock_operation(operation_id),
            Err(PublicationError::ConcurrentJournalUpdate)
        ));
        held.finish(Ok(())).expect("release operation lock");

        let hard_link = state.path().join("operation-lock-hard-link");
        fs::hard_link(&lock_path, &hard_link).expect("link operation lock");
        assert!(matches!(
            store.lock_operation(operation_id),
            Err(PublicationError::InvalidJournal(_))
        ));
        fs::remove_file(hard_link).expect("remove fixture hard link");

        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o4600))
            .expect("add set-user-ID bit to operation lock");
        assert!(matches!(
            store.lock_operation(operation_id),
            Err(PublicationError::InvalidJournal(_))
        ));
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600))
            .expect("restore operation lock permissions");
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_transition_cas_has_exactly_one_winner() {
        use std::sync::{Arc, Barrier};

        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, _) = request();
        let operation_id = request.operation_id();
        let previous = store.create(request).expect("create journal");
        let barrier = Arc::new(Barrier::new(2));
        let root = state.path().to_path_buf();
        let workers = (0..2)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let root = root.clone();
                let previous = previous.clone();
                std::thread::spawn(move || {
                    let store = PublicationJournalStore::open(&root).expect("worker journal store");
                    barrier.wait();
                    store.transition(&previous, previous.clone())
                })
            })
            .collect::<Vec<_>>();
        let results = workers
            .into_iter()
            .map(|worker| worker.join().expect("transition worker"))
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(PublicationError::ConcurrentJournalUpdate)))
                .count(),
            1
        );
        assert_eq!(
            store.load(operation_id).expect("winning journal").revision,
            2
        );
    }

    struct EarlyBackend {
        broker: KeyPair,
        fail_validation_once: bool,
        substitute_receipt: bool,
    }

    struct CompleteBackend {
        broker: KeyPair,
        replication_pending_once: bool,
        finality_pending_once: bool,
        substitute_readback: bool,
        submissions: usize,
    }

    struct ArchiveRegistrationCrashBackend {
        complete: CompleteBackend,
        now_ms: u64,
        archive_committed: bool,
        registration_attempts: usize,
        staged_receipts: Vec<MusubiSeedIngressReceiptV1>,
    }

    impl EarlyBackend {
        fn unsupported() -> PublicationBackendError {
            PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
        }
    }

    impl PublicationBackend for EarlyBackend {
        fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
            Ok(1_500)
        }

        fn validate_clean_package(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            car: &mut dyn Read,
        ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
            if self.fail_validation_once {
                self.fail_validation_once = false;
                return Err(PublicationBackendError::retryable(
                    "COMPILER_TEMPORARILY_UNAVAILABLE",
                ));
            }
            let mut consumed = Vec::new();
            car.read_to_end(&mut consumed)
                .map_err(|_| PublicationBackendError::permanent("CAR_READ_FAILED"))?;
            if consumed.is_empty() {
                return Err(PublicationBackendError::permanent("EMPTY_TEST_CAR"));
            }
            Ok(validation_evidence(request))
        }

        fn stage_authenticated_seed_ingress(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            expected: &MusubiSeedIngressReceiptBindingV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            let mut receipt = signed_receipt(expected, &self.broker);
            if self.substitute_receipt {
                receipt.payload.binding.archive_id = ArchiveId::new([0xEE; 32]);
            }
            Ok(receipt)
        }

        fn ensure_archive_and_permanent_pin(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _registration: &PublicationArchiveRegistrationV1,
        ) -> Result<Option<MusubiArchiveLocationV1>, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn readback_provider(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _location: &MusubiArchiveLocationV1,
            _provider: ProviderId,
        ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn submit_release_native_amx(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _instruction: &PublishMusubiReleaseV1,
        ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn finalized_release_and_index(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _submission: &PublicationAmxSubmissionV1,
        ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
            Err(Self::unsupported())
        }
    }

    impl PublicationBackend for CompleteBackend {
        fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
            Ok(1_500)
        }

        fn validate_clean_package(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _car: &mut dyn Read,
        ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
            Ok(validation_evidence(request))
        }

        fn stage_authenticated_seed_ingress(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            expected: &MusubiSeedIngressReceiptBindingV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            Ok(signed_receipt(expected, &self.broker))
        }

        fn ensure_archive_and_permanent_pin(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
            Ok(registration(request, &self.broker))
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registration: &PublicationArchiveRegistrationV1,
        ) -> Result<Option<MusubiArchiveLocationV1>, PublicationBackendError> {
            if self.replication_pending_once {
                self.replication_pending_once = false;
                return Ok(None);
            }
            Ok(Some(location(request, registration, 3)))
        }

        fn readback_provider(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            location: &MusubiArchiveLocationV1,
            provider: ProviderId,
        ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
            let mut evidence = PublicationReadbackEvidenceV1 {
                provider,
                location_id: location.location_id,
                replication_order: location.replication_order,
                commitment: request.archive_commitment.clone(),
                semantic_release_digest: request.publication.manifest.semantic_digest(),
                verification_lock_digest: request.publication.manifest.verification_lock_digest,
            };
            if self.substitute_readback && provider == location.providers[0] {
                evidence.commitment.car_digest = MusubiContentDigestV1::new([0xEE; 32]);
            }
            Ok(evidence)
        }

        fn submit_release_native_amx(
            &mut self,
            operation_id: PublicationOperationIdV1,
            instruction: &PublishMusubiReleaseV1,
        ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
            self.submissions += 1;
            Ok(PublicationAmxSubmissionV1::new(
                operation_id,
                instruction,
                [0x71; 32],
            ))
        }

        fn finalized_release_and_index(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _submission: &PublicationAmxSubmissionV1,
        ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
            if self.finality_pending_once {
                self.finality_pending_once = false;
                return Ok(None);
            }
            Ok(Some(final_evidence(request)))
        }
    }

    impl PublicationBackend for ArchiveRegistrationCrashBackend {
        fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
            Ok(self.now_ms)
        }

        fn validate_clean_package(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            car: &mut dyn Read,
        ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
            self.complete
                .validate_clean_package(operation_id, request, car)
        }

        fn stage_authenticated_seed_ingress(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            expected: &MusubiSeedIngressReceiptBindingV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            let receipt = signed_receipt_at(
                expected,
                &self.complete.broker,
                self.now_ms,
                self.now_ms + 1_000,
            );
            self.staged_receipts.push(receipt.clone());
            Ok(receipt)
        }

        fn ensure_archive_and_permanent_pin(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
            self.registration_attempts += 1;
            if !self.archive_committed {
                self.archive_committed = true;
                self.now_ms = receipt.payload.expires_at_ms + 1;
                return Err(PublicationBackendError::retryable(
                    "CRASH_AFTER_ARCHIVE_COMMIT",
                ));
            }
            let registration = registration(request, &self.complete.broker);
            assert_eq!(
                registration.archive.staging_receipt,
                self.staged_receipts[0]
            );
            assert_ne!(registration.archive.staging_receipt, *receipt);
            Ok(registration)
        }

        fn finalized_replication(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registration: &PublicationArchiveRegistrationV1,
        ) -> Result<Option<MusubiArchiveLocationV1>, PublicationBackendError> {
            self.complete
                .finalized_replication(operation_id, request, registration)
        }

        fn readback_provider(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            location: &MusubiArchiveLocationV1,
            provider: ProviderId,
        ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
            self.complete
                .readback_provider(operation_id, request, location, provider)
        }

        fn submit_release_native_amx(
            &mut self,
            operation_id: PublicationOperationIdV1,
            instruction: &PublishMusubiReleaseV1,
        ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
            self.complete
                .submit_release_native_amx(operation_id, instruction)
        }

        fn finalized_release_and_index(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            submission: &PublicationAmxSubmissionV1,
        ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
            self.complete
                .finalized_release_and_index(operation_id, request, submission)
        }
    }

    fn account(seed: u8) -> (AccountId, KeyPair) {
        let keypair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture keypair");
        (AccountId::new(keypair.public_key().clone()), keypair)
    }

    fn snapshot() -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: 42,
            finalized_block_hash: [0x42; 32],
            index_revision: 3,
        }
    }

    fn archive_commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
            por_root: MusubiContentDigestV1::new([3; 32]),
            content_length: 1_024,
            car_digest: MusubiContentDigestV1::new([4; 32]),
            car_size: 2_048,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: 2,
            chunk_count: 4,
        }
    }

    fn request() -> (PublicationRequestV1, KeyPair) {
        let commitment = archive_commitment();
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package"),
        );
        let release = MusubiReleaseIdV1::new(
            package,
            "1.0.0".parse::<MusubiVersionV1>().expect("release version"),
        );
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let manifest = MusubiReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([9; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id: commitment.archive_id(),
            verification_lock_digest: lock.digest(),
        };
        let (publisher, _) = account(20);
        let (broker, broker_keypair) = account(21);
        (
            PublicationRequestV1 {
                chain_id: ChainId::from("musubi-publish-test"),
                genesis_block_hash: [0x15; 32],
                publisher,
                ingress_broker: broker,
                seed_provider: ProviderId::new([0x16; 32]),
                namespace: MusubiNamespaceV1::new("dex").expect("namespace"),
                publication: MusubiPublicationV1 {
                    manifest,
                    resolution: MusubiResolutionProofV1 {
                        snapshot: snapshot(),
                        lock,
                    },
                },
                archive_commitment: commitment,
                namespace_delegation: None,
                expected_policy_revision: 1,
                expected_governance_revision: None,
                nonce: [0x18; 32],
            },
            broker_keypair,
        )
    }

    fn validation_evidence(request: &PublicationRequestV1) -> PublicationValidationEvidenceV1 {
        PublicationValidationEvidenceV1 {
            archive_id: request.archive_commitment.archive_id(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            release_digest: request.publication.manifest.release_digest(),
            source_tree_digest: request.archive_commitment.source_tree_digest,
            descriptor_digest: request.archive_commitment.descriptor_digest,
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
            car_digest: request.archive_commitment.car_digest,
            car_size: request.archive_commitment.car_size,
            compiler_output_digest: MusubiContentDigestV1::new([0x63; 32]),
            resolution_snapshot: request.publication.resolution.snapshot,
        }
    }

    fn signed_receipt(
        binding: &MusubiSeedIngressReceiptBindingV1,
        broker: &KeyPair,
    ) -> MusubiSeedIngressReceiptV1 {
        signed_receipt_at(binding, broker, 1_000, 2_000)
    }

    fn signed_receipt_at(
        binding: &MusubiSeedIngressReceiptBindingV1,
        broker: &KeyPair,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> MusubiSeedIngressReceiptV1 {
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: binding.clone(),
            issued_at_ms,
            expires_at_ms,
        };
        MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker.public_key().clone(),
                signature: SignatureOf::try_from_hash(broker.private_key(), payload.signing_hash())
                    .expect("sign fixture ingress receipt"),
            }],
            payload,
        }
    }

    fn registration(
        request: &PublicationRequestV1,
        broker: &KeyPair,
    ) -> PublicationArchiveRegistrationV1 {
        PublicationArchiveRegistrationV1 {
            archive: MusubiArchiveRecordV1 {
                archive_id: request.archive_commitment.archive_id(),
                commitment: request.archive_commitment.clone(),
                staging_receipt: signed_receipt(&request.receipt_binding(), broker),
                registered_by: request.publisher.clone(),
                registered_at_height: 50,
                location_revision: 1,
                location_ids: Vec::new(),
            },
            location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
            pin_manifest: ManifestDigest::new([0x32; 32]),
            replication_order: ReplicationOrderId::new([0x33; 32]),
            renew_after_epoch: 10,
            expires_at_epoch: 20,
        }
    }

    fn provider_attestation(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        provider_byte: u8,
    ) -> MusubiProviderBundleVerificationAttestationV1 {
        let (owner, keypair) = account(provider_byte.saturating_add(60));
        let binding = MusubiProviderBundleVerificationBindingV1 {
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            provider_id: ProviderId::new([provider_byte; 32]),
            completed_by: owner.clone(),
            completion_authority: ProviderIngestCompletionAuthorityV1::new(
                owner,
                ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [provider_byte.saturating_add(20); 32],
                    revision: 1,
                    predecessor_digest: None,
                    policy_digest: [provider_byte.saturating_add(30); 32],
                },
            ),
            replication_order: registration.replication_order,
            assignment_revision: 1,
            completion_epoch: 12,
            finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                height: 60,
                block_hash: [provider_byte.saturating_add(40); 32],
            },
            archive_id: request.archive_commitment.archive_id(),
            bundle_digest: request.archive_commitment.bundle_digest,
            descriptor_digest: request.archive_commitment.descriptor_digest,
            semantic_release_manifest_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
            source_tree_digest: request.archive_commitment.source_tree_digest,
        };
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding,
        };
        MusubiProviderBundleVerificationAttestationV1 {
            approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                public_key: keypair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    keypair.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign provider fixture attestation"),
            }],
            payload,
        }
    }

    fn location(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        provider_count: u8,
    ) -> MusubiArchiveLocationV1 {
        let attestations = (1..=provider_count)
            .map(|provider| provider_attestation(request, registration, provider))
            .collect::<Vec<_>>();
        MusubiArchiveLocationV1 {
            location_id: registration.location_id,
            archive_id: request.archive_commitment.archive_id(),
            pin_manifest: registration.pin_manifest,
            replication_order: registration.replication_order,
            providers: attestations
                .iter()
                .map(|attestation| attestation.payload.binding.provider_id)
                .collect(),
            provider_attestations: attestations,
            renew_after_epoch: registration.renew_after_epoch,
            expires_at_epoch: registration.expires_at_epoch,
            finalized_height: 70,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        }
    }

    fn final_evidence(request: &PublicationRequestV1) -> PublicationFinalEvidenceV1 {
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 100,
            finalized_block_hash: [0x64; 32],
            index_revision: 4,
        };
        let yank = MusubiReleaseYankV1 {
            release: request.publication.manifest.release.clone(),
            yanked: false,
            reason: MusubiReasonV1::new("initial publication").expect("reason"),
            changed_by: request.publisher.clone(),
            changed_at_height: 80,
            revision: 1,
        };
        let governance = MusubiArtifactGovernanceStateV1::Available;
        let home_release = MusubiReleaseRecordV1 {
            manifest: request.publication.manifest.clone(),
            release_digest: request.publication.manifest.release_digest(),
            published_by: request.publisher.clone(),
            published_at_height: 80,
            yank: yank.clone(),
            artifact_governance: governance.clone(),
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
        };
        let universal_release = MusubiResolverReleaseRowV1 {
            release: request.publication.manifest.release.clone(),
            release_digest: request.publication.manifest.release_digest(),
            archive_id: request.archive_commitment.archive_id(),
            source_digest: request.archive_commitment.source_tree_digest,
            interface_digest: request.publication.manifest.interface_digest,
            abi: request.publication.manifest.abi,
            dependencies: request.publication.manifest.dependencies.clone(),
            selection: MusubiReleaseSelectionStateV1 {
                yank,
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id: request.archive_commitment.archive_id(),
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: 3,
                    active_locations: 1,
                    finalized_height: 70,
                    finalized_block_hash: [0x46; 32],
                    index_revision: snapshot.index_revision,
                },
                governance,
            },
            index_revision: snapshot.index_revision,
        };
        PublicationFinalEvidenceV1 {
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot,
            home_release,
            universal_release,
        }
    }

    #[cfg(unix)]
    #[test]
    fn retry_and_receipt_substitution_never_advance_the_journal() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"runtime-only-car-secret".to_vec());
        let journal_bytes = fs::read(
            temp.path()
                .join(JOURNAL_DIRECTORY)
                .join(format!("{operation_id}.{JOURNAL_EXTENSION}")),
        )
        .expect("read journal");
        assert!(
            !journal_bytes
                .windows(b"runtime-only-car-secret".len())
                .any(|window| window == b"runtime-only-car-secret")
        );

        let mut backend = EarlyBackend {
            broker,
            fail_validation_once: true,
            substitute_receipt: true,
        };
        let error = engine
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("first validation is retryable");
        let PublicationError::Backend(error) = error else {
            panic!("expected backend failure");
        };
        assert_eq!(error.class(), PublicationBackendFailureClass::Retryable);
        let unchanged = store.load(operation_id).expect("unchanged journal");
        assert_eq!(unchanged.phase, PublicationPhaseV1::Validation);
        assert_eq!(unchanged.revision, 1);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("retry validation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        );
        let error = engine
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("substituted receipt must fail");
        assert!(matches!(
            error,
            PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::SeedIngress,
                ..
            }
        ));
        let unchanged = store
            .load(operation_id)
            .expect("unchanged after substitution");
        assert_eq!(unchanged.phase, PublicationPhaseV1::SeedIngress);
        assert_eq!(unchanged.revision, 2);
        assert!(unchanged.staging_receipt.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn detached_resume_crosses_all_seven_phases_and_reuses_amx_submission() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = CompleteBackend {
            broker,
            replication_pending_once: true,
            finality_pending_once: true,
            substitute_readback: false,
            submissions: 0,
        };

        assert_eq!(
            engine
                .publish(request, &source, &mut backend)
                .expect("start publication"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::Replication)
        );
        let replication_wait = store.load(operation_id).expect("replication journal");
        assert_eq!(replication_wait.phase, PublicationPhaseV1::Replication);
        assert_eq!(replication_wait.revision, 4);

        assert_eq!(
            engine
                .resume(operation_id, &source, &mut backend)
                .expect("resume through AMX"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::FinalVerification)
        );
        assert_eq!(backend.submissions, 1);
        let finality_wait = store.load(operation_id).expect("finality journal");
        assert_eq!(finality_wait.phase, PublicationPhaseV1::FinalVerification);
        assert_eq!(finality_wait.revision, 7);

        let completed = engine
            .resume(operation_id, &source, &mut backend)
            .expect("complete final verification");
        let PublicationAdvanceV1::Complete(result) = completed else {
            panic!("publication should be complete");
        };
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(backend.submissions, 1);
        assert!(matches!(
            engine
                .resume(operation_id, &source, &mut backend)
                .expect("idempotent completed resume"),
            PublicationAdvanceV1::Complete(_)
        ));
        assert_eq!(backend.submissions, 1);
    }

    #[cfg(unix)]
    #[test]
    fn archive_commit_crash_resumes_after_receipt_refresh_without_changing_operation() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRegistrationCrashBackend {
            complete: CompleteBackend {
                broker,
                replication_pending_once: false,
                finality_pending_once: false,
                substitute_readback: false,
                submissions: 0,
            },
            now_ms: 1_000,
            archive_committed: false,
            registration_attempts: 0,
            staged_receipts: Vec::new(),
        };

        let error = engine
            .publish(request, &source, &mut backend)
            .expect_err("simulated crash loses the first registration response");
        assert!(matches!(
            error,
            PublicationError::Backend(ref backend_error)
                if backend_error.code() == "CRASH_AFTER_ARCHIVE_COMMIT"
        ));
        let interrupted = store.load(operation_id).expect("interrupted journal");
        assert_eq!(
            interrupted.phase,
            PublicationPhaseV1::ArchiveRegistration
        );
        let registered_receipt = interrupted
            .staging_receipt
            .clone()
            .expect("original receipt remains durable");

        let completed = engine
            .resume(operation_id, &source, &mut backend)
            .expect("resume with a refreshed operation-bound receipt");
        assert!(matches!(completed, PublicationAdvanceV1::Complete(_)));
        assert_eq!(backend.registration_attempts, 2);
        assert_eq!(backend.staged_receipts.len(), 2);
        assert_eq!(backend.staged_receipts[0], registered_receipt);
        assert_ne!(backend.staged_receipts[0], backend.staged_receipts[1]);
        assert_eq!(
            backend.staged_receipts[0].payload.binding,
            backend.staged_receipts[1].payload.binding
        );
        let persisted = store.load(operation_id).expect("completed journal");
        assert_eq!(
            persisted
                .archive_registration
                .expect("archive registration evidence")
                .archive
                .staging_receipt,
            registered_receipt,
            "authoritative archive retains its first receipt"
        );
    }

    #[cfg(unix)]
    #[test]
    fn trait_backed_readback_substitution_stops_before_amx() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = CompleteBackend {
            broker,
            replication_pending_once: false,
            finality_pending_once: false,
            substitute_readback: true,
            submissions: 0,
        };
        assert!(matches!(
            engine.publish(request, &source, &mut backend),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Readback,
                ..
            })
        ));
        assert_eq!(backend.submissions, 0);
        let journal = store.load(operation_id).expect("readback journal");
        assert_eq!(journal.phase, PublicationPhaseV1::Readback);
        assert!(journal.readbacks.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn journal_rejects_missing_phase_evidence_and_tampered_receipt_signature() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");

        let mut missing = store.load(operation_id).expect("load journal");
        missing.phase = PublicationPhaseV1::Replication;
        assert!(matches!(
            missing.validate(),
            Err(PublicationError::InvalidJournal(_))
        ));

        let source = BytesSource(b"car".to_vec());
        let mut backend = EarlyBackend {
            broker,
            fail_validation_once: false,
            substitute_receipt: false,
        };
        assert!(matches!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("validate"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        ));
        assert!(matches!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stage"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        ));
        let mut tampered = store.load(operation_id).expect("load staged journal");
        let (_, attacker) = account(99);
        tampered
            .staging_receipt
            .as_mut()
            .expect("receipt")
            .approvals[0]
            .public_key = attacker.public_key().clone();
        let tampered = norito::encode_canonical(&tampered).expect("encode tampered journal");
        store
            .root
            .replace(&journal_relative_path(operation_id), &tampered)
            .expect("simulate durable disk substitution");
        assert!(matches!(
            store.load(operation_id),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::SeedIngress,
                ..
            })
        ));
    }

    #[test]
    fn journal_rejects_archive_registration_receipt_replay_from_another_nonce() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let expected_receipt = registration.archive.staging_receipt.clone();
        let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
        journal.validation = Some(validation_evidence(&request));
        journal.staging_receipt = Some(expected_receipt);
        journal.archive_registration = Some(registration);
        journal.phase = PublicationPhaseV1::Replication;
        journal
            .validate()
            .expect("registration must retain the exact staged receipt");

        let mut replayed_binding = request.receipt_binding();
        replayed_binding.nonce = [0xEE; 32];
        journal
            .archive_registration
            .as_mut()
            .expect("archive registration")
            .archive
            .staging_receipt = signed_receipt(&replayed_binding, &broker);
        assert!(matches!(
            journal.validate(),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                ..
            })
        ));
    }

    #[test]
    fn journal_accepts_a_refreshed_receipt_for_the_registered_operation_binding() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let registered_receipt = registration.archive.staging_receipt.clone();
        let refreshed_receipt = signed_receipt_at(
            &registered_receipt.payload.binding,
            &broker,
            registered_receipt.payload.expires_at_ms + 1,
            registered_receipt.payload.expires_at_ms + 1_001,
        );
        assert_ne!(registered_receipt, refreshed_receipt);

        let mut journal = PublicationJournalV1::new(request).expect("publication journal");
        journal.validation = Some(validation_evidence(&journal.request));
        journal.staging_receipt = Some(refreshed_receipt);
        journal.archive_registration = Some(registration);
        journal.phase = PublicationPhaseV1::Replication;
        journal
            .validate()
            .expect("receipt refresh with the exact operation binding must resume");
    }

    #[test]
    fn replication_requires_three_exact_finalized_provider_attestations() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        registration
            .validate_for(&request, &registration.archive.staging_receipt)
            .expect("valid archive registration");

        let below_quorum = location(&request, &registration, 2);
        assert!(matches!(
            validate_replication(&request, &registration, &below_quorum),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));

        let exact = location(&request, &registration, 3);
        validate_replication(&request, &registration, &exact).expect("three-provider quorum");
        let mut substituted = exact;
        substituted.provider_attestations[1]
            .payload
            .binding
            .semantic_release_manifest_digest = MusubiSemanticReleaseDigestV1::new([0xEE; 32]);
        assert!(matches!(
            validate_replication(&request, &registration, &substituted),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));
    }

    #[test]
    fn readback_rejects_provider_or_commitment_substitution() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let location = location(&request, &registration, 3);
        let provider = location.providers[0];
        let exact = PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        };
        exact
            .validate_for(&request, &location, provider)
            .expect("exact readback");

        let mut wrong_provider = exact.clone();
        wrong_provider.provider = location.providers[1];
        assert!(
            wrong_provider
                .validate_for(&request, &location, provider)
                .is_err()
        );
        let mut wrong_car = exact;
        wrong_car.commitment.car_digest = MusubiContentDigestV1::new([0xEF; 32]);
        assert!(
            wrong_car
                .validate_for(&request, &location, provider)
                .is_err()
        );
    }

    #[test]
    fn amx_and_final_index_evidence_bind_the_exact_release() {
        let (request, _) = request();
        let operation_id = request.operation_id();
        let instruction = request.publish_instruction();
        let exact_submission =
            PublicationAmxSubmissionV1::new(operation_id, &instruction, [0x71; 32]);
        exact_submission
            .validate_for(operation_id, &instruction)
            .expect("exact AMX submission");
        let mut substituted_submission = exact_submission;
        substituted_submission.instruction_digest = [0x72; 32];
        assert!(
            substituted_submission
                .validate_for(operation_id, &instruction)
                .is_err()
        );

        let exact_final = final_evidence(&request);
        exact_final
            .validate_for(&request)
            .expect("exact finalized home and universal records");
        let mut wrong_chain = exact_final.clone();
        wrong_chain.chain_id = ChainId::from("another-musubi-chain");
        assert!(wrong_chain.validate_for(&request).is_err());
        let mut wrong_genesis = exact_final.clone();
        wrong_genesis.genesis_block_hash = [0x74; 32];
        assert!(wrong_genesis.validate_for(&request).is_err());
        let mut substituted_index = exact_final;
        substituted_index.universal_release.source_digest = MusubiContentDigestV1::new([0x73; 32]);
        assert!(substituted_index.validate_for(&request).is_err());
    }

    #[test]
    fn detached_operation_ids_are_canonical_nonzero_lowercase_hex() {
        let (request, _) = request();
        let operation_id = request.operation_id();
        assert_eq!(operation_id.to_string().parse(), Ok(operation_id));
        assert!("00".repeat(32).parse::<PublicationOperationIdV1>().is_err());
        let canonical = operation_id.to_string();
        assert!(
            format!("A{}", &canonical[1..])
                .parse::<PublicationOperationIdV1>()
                .is_err()
        );
    }

    #[test]
    fn publication_request_rejects_an_empty_chain_identity() {
        let (mut request, _) = request();
        request.chain_id = ChainId::from("");
        assert!(matches!(
            request.validate(),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
    }
}
