//! Resumable, evidence-checked Musubi V1 publication workflow.
//!
//! The workflow deliberately keeps network authentication and signing outside
//! the persisted state.  Its journal contains only public request material,
//! signed public evidence, finalized records, and idempotency identifiers.  A
//! backend is therefore supplied at runtime and cannot smuggle provider URLs,
//! bearer credentials, private keys, or a retired public upload route into a
//! project or operation journal.
//!
//! Archive registration has three durable checkpoints inside the public
//! `ArchiveRegistration` phase: a bounded append-only sequence of exact
//! fee-quoted signed transaction attempts, the finalized authoritative archive
//! record recovered from the registry, and only then permanent pin/order
//! coordination. An attempt is never replaced while its application state is
//! absent, unknown, or pending. A new receipt and transaction generation are
//! permitted only after durable terminal and finalized-absence evidence for the
//! preceding attempt. Archive locations have an independent bounded append-only
//! generation history: each exact signed CAS is persisted before submission,
//! every later generation requires authoritative terminal finalized state, and
//! retired stable location identities are never reused.
//! Release submission follows the same Phase-B boundary: clean readbacks and one
//! fee-quoted signature are persisted atomically before any send, every retry first
//! queries the exact transaction hash, and an absent transaction is submitted only
//! while the selected finalized location is byte-identical to the signed floor.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read},
    num::{NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    str::FromStr,
};

use iroha::musubi_runtime::{
    MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1, MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1,
    MusubiSeedIngressCarPlanV1,
};
use iroha_data_model::{
    ChainId, NetworkId,
    account::AccountId,
    block::BlockHeader,
    isi::{
        InstructionBox,
        musubi::{AddMusubiArchiveLocationV1, PublishMusubiReleaseV1, RegisterMusubiArchiveV1},
    },
    metadata::Metadata,
    musubi::{
        ArchiveId, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_LOCATION_PROVIDERS_V1,
        MUSUBI_MIN_HEALTHY_REPLICAS_V1, MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1,
        MusubiArchiveLocationPageV1, MusubiArchiveLocationStateV1, MusubiArchiveLocationV1,
        MusubiArchiveRecordV1, MusubiArchiveRetentionDecisionV1,
        MusubiArchiveRetentionDispositionV1, MusubiArchiveRetentionPageV1,
        MusubiArchiveRetentionQueryV1, MusubiContentDigestV1, MusubiExactReleaseSnapshotV1,
        MusubiNamespaceDelegationV1, MusubiNamespaceV1, MusubiPackageScopeV1,
        MusubiProviderBundleAttestationDigestV1, MusubiProviderBundleAttestationKeyV1,
        MusubiProviderBundleAttestationSetDigestV1, MusubiPublicationV1, MusubiRegistrySnapshotV1,
        MusubiReleaseDigestV1, MusubiReleaseIdV1, MusubiReleaseRecordV1, MusubiResolverIndexPageV1,
        MusubiResolverReleaseRowV1, MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptV1,
        MusubiSemanticReleaseDigestV1, MusubiVerificationLockDigestV1, MusubiVersionReqV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, ReplicationOrderId},
    },
    transaction::{
        Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionPayload,
        TransactionSignature, signed::MultisigSignatures,
    },
};
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
use sorafs_car::{CarBuildPlan, CarVerifier, ChunkStore};

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
const STAGED_PLAN_EXTENSION: &str = "plan.norito";
const STAGED_PLAN_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    16_384,
    MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1,
    2_097_152,
    128 * 1024 * 1024,
    128,
);
const STAGED_CAR_PLAN_HEAP_LIMIT_BYTES_V1: usize =
    sorafs_car::DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES;

#[cfg(all(test, unix))]
std::thread_local! {
    static TEST_PUBLICATION_READ_FIFO_SUBSTITUTIONS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// Maximum number of exact archive-registration transaction generations in one V1 operation.
pub const MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1: usize = 8;
/// Maximum number of append-only archive-location transaction generations in one V1 operation.
pub const MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1: usize = 8;
/// Maximum immutable signed registration attempts retained for one provider proof.
pub const MUSUBI_MAX_PROVIDER_REGISTRATION_ATTEMPTS_V1: u8 = 8;
/// Maximum number of exact release-submission transaction generations in one V1 operation.
pub const MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1: usize = 8;
/// Maximum authorization signatures retained by one compact release transaction envelope.
pub const MUSUBI_MAX_RELEASE_SIGNATURES_V1: usize = 16;
const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_JOURNAL_BYTES_USIZE: usize = 16 * 1024 * 1024;
// The persisted frame reserves half of its payload budget for immutable request/archive evidence
// and half for eight release attempts plus compact completion state. Standalone canonical
// component frames overcount their nested representation, while the remaining 1/64 covers outer
// framing and terminal fields.
const JOURNAL_CANONICAL_OVERHEAD_RESERVE_BYTES: usize = MAX_JOURNAL_BYTES_USIZE / 64;
const JOURNAL_COMPONENT_BUDGET_BYTES: usize =
    MAX_JOURNAL_BYTES_USIZE - JOURNAL_CANONICAL_OVERHEAD_RESERVE_BYTES;
const JOURNAL_NON_RELEASE_BUDGET_BYTES: usize = JOURNAL_COMPONENT_BUDGET_BYTES / 2;
const JOURNAL_RELEASE_HISTORY_BUDGET_BYTES: usize =
    JOURNAL_COMPONENT_BUDGET_BYTES - JOURNAL_NON_RELEASE_BUDGET_BYTES;
const MAX_RELEASE_SUBMISSION_CANONICAL_BYTES: usize = JOURNAL_CANONICAL_OVERHEAD_RESERVE_BYTES / 4;
const MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES: usize =
    JOURNAL_CANONICAL_OVERHEAD_RESERVE_BYTES / 4;
const JOURNAL_RELEASE_FINAL_STATE_BUDGET_BYTES: usize =
    MAX_RELEASE_SUBMISSION_CANONICAL_BYTES + MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES;
const JOURNAL_RELEASE_ATTEMPTS_BUDGET_BYTES: usize =
    JOURNAL_RELEASE_HISTORY_BUDGET_BYTES - JOURNAL_RELEASE_FINAL_STATE_BUDGET_BYTES;
// The exact paired query is bounded independently from the compact journal checkpoint. It may
// include consensus-legal post-publication controller projections that must not inflate the
// durable operation frame.
const MAX_RELEASE_FINAL_QUERY_EVIDENCE_CANONICAL_BYTES: usize = MAX_JOURNAL_BYTES_USIZE;
const MAX_RELEASE_ATTEMPT_CANONICAL_BYTES: usize =
    JOURNAL_RELEASE_ATTEMPTS_BUDGET_BYTES / MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1;
// Every retained attempt receives equal canonical budgets for its persist-before-send intent and
// its later applied/terminal outcome. Their standalone frames overcount the nested attempt bytes.
const MAX_RELEASE_INTENT_CANONICAL_BYTES: usize = MAX_RELEASE_ATTEMPT_CANONICAL_BYTES / 2;
const MAX_RELEASE_OUTCOME_CANONICAL_BYTES: usize =
    MAX_RELEASE_ATTEMPT_CANONICAL_BYTES - MAX_RELEASE_INTENT_CANONICAL_BYTES;
// TODO: Replace complete journaled location pages with digest-addressed, durably verified
// sidecars before admitting the full consensus-legal 4 locations x 64 providers x 64 approvals
// shape (especially ML-DSA approvals). Until then the derived canonical budgets reject it
// fail-closed rather than allowing a publication that cannot append its terminal evidence.
const JOURNAL_DECODE_LIMITS: DecodeLimits = DecodeLimits::new(
    1_048_576,
    MAX_JOURNAL_BYTES_USIZE,
    2_097_152,
    64 * 1024 * 1024,
    128,
);
const OPERATION_ID_DOMAIN: &[u8] = b"iroha.musubi.publication-operation.v1";
const ARCHIVE_REGISTRATION_INSTRUCTION_DOMAIN: &[u8] =
    b"iroha.musubi.archive-registration-instruction.v1";
const ARCHIVE_LOCATION_INSTRUCTION_DOMAIN: &[u8] = b"iroha.musubi.archive-location-instruction.v1";
const PUBLISH_INSTRUCTION_DOMAIN: &[u8] = b"iroha.musubi.publish-instruction.v1";
const RELEASE_SIGNED_TRANSACTION_DOMAIN: &[u8] = b"iroha.musubi.release-signed-transaction.v1";
const FINAL_HOME_RELEASE_DOMAIN: &[u8] = b"iroha.musubi.final-home-release.v1";
const FINAL_UNIVERSAL_RELEASE_DOMAIN: &[u8] = b"iroha.musubi.final-universal-release.v1";
const FINAL_CHECKPOINT_DOMAIN: &[u8] = b"iroha.musubi.final-checkpoint.v1";
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
    /// Return the exact network identity derived from this request's genesis commitment.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        NetworkId::from_genesis_hash(
            iroha::crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                iroha::crypto::Hash::prehashed(self.genesis_block_hash),
            ),
        )
    }

    /// Validate all immutable publication, archive, deployment, and revision bindings.
    ///
    /// # Errors
    ///
    /// Returns an invalid-evidence error when any request field, namespace binding, archive
    /// commitment, or policy revision is malformed or inconsistent.
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

    pub(crate) fn publish_instruction(&self) -> PublishMusubiReleaseV1 {
        PublishMusubiReleaseV1::new(
            self.namespace.clone(),
            self.publication.clone(),
            self.namespace_delegation.clone(),
            self.expected_policy_revision,
            self.expected_governance_revision,
        )
    }

    /// Construct the one canonical archive-registration instruction for this operation receipt.
    pub(crate) fn archive_registration_instruction(
        &self,
        staging_receipt: &MusubiSeedIngressReceiptV1,
    ) -> RegisterMusubiArchiveV1 {
        RegisterMusubiArchiveV1::new(
            self.archive_commitment.clone(),
            staging_receipt.clone(),
            self.expected_policy_revision,
        )
    }
}

/// The seven production phases of Musubi V1 publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum PublicationPhaseV1 {
    /// Validate and compiler-check the clean package and exact proof.
    Validation,
    /// Stage the exact CAR through authenticated `SoraFS` seed ingress.
    SeedIngress,
    /// Persist exact registration intent and finality before creating its permanent pin/order.
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

/// Durable exact transaction intent for immutable archive registration.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveRegistrationIntentV1 {
    /// Operation whose immutable request produced this intent.
    pub operation_id: PublicationOperationIdV1,
    /// Exact archive identity registered by the instruction.
    pub archive_id: ArchiveId,
    /// Exact seed-ingress receipt embedded in the registration instruction.
    pub staging_receipt: MusubiSeedIngressReceiptV1,
    /// Domain-separated digest of the canonical [`RegisterMusubiArchiveV1`] instruction.
    pub instruction_digest: [u8; 32],
    /// Exact fee-quoted and signed transaction to submit or recover.
    pub signed_transaction: SignedTransaction,
    /// Canonical transaction hash derived from `signed_transaction`.
    pub transaction_hash: [u8; 32],
}

impl PublicationArchiveRegistrationIntentV1 {
    /// Bind an exact prebuilt transaction to the immutable operation and registration instruction.
    #[must_use]
    pub fn new(
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        staging_receipt: MusubiSeedIngressReceiptV1,
        signed_transaction: SignedTransaction,
    ) -> Self {
        let instruction = request.archive_registration_instruction(&staging_receipt);
        let transaction_hash = *signed_transaction.hash().as_ref();
        Self {
            operation_id,
            archive_id: request.archive_commitment.archive_id(),
            staging_receipt,
            instruction_digest: archive_registration_instruction_digest(&instruction),
            signed_transaction,
            transaction_hash,
        }
    }

    pub(crate) fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        staging_receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<(), PublicationError> {
        let instruction = request.archive_registration_instruction(staging_receipt);
        let expected_instruction: InstructionBox = instruction.clone().into();
        let exact_instruction = matches!(
            self.signed_transaction.instructions(),
            Executable::Instructions(instructions)
                if instructions.len() == 1
                    && instructions.iter().next() == Some(&expected_instruction)
        );
        let signature_is_valid = self.signed_transaction.verify_signature().is_ok();
        let receipt_is_valid = staging_receipt
            .verify(
                &request.receipt_binding(),
                staging_receipt.payload.issued_at_ms,
            )
            .is_ok();
        if self.operation_id != operation_id
            || self.archive_id != request.archive_commitment.archive_id()
            || &self.staging_receipt != staging_receipt
            || self.instruction_digest != archive_registration_instruction_digest(&instruction)
            || self.transaction_hash != *self.signed_transaction.hash().as_ref()
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || self.signed_transaction.network_id().copied() != Some(request.network_id())
            || self.signed_transaction.authority() != &request.publisher
            || archive_registration_intent_valid_until_ms(self).is_none()
            || !exact_instruction
            || !signature_is_valid
            || !receipt_is_valid
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason: "archive registration intent or exact signed transaction was substituted"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Return the earliest consensus validity deadline shared by the signed transaction and receipt.
pub(crate) fn archive_registration_intent_valid_until_ms(
    intent: &PublicationArchiveRegistrationIntentV1,
) -> Option<u64> {
    let created_at_ms =
        u64::try_from(intent.signed_transaction.creation_time().as_millis()).ok()?;
    if created_at_ms < intent.staging_receipt.payload.issued_at_ms
        || created_at_ms > intent.staging_receipt.payload.expires_at_ms
    {
        return None;
    }
    let ttl_ms = u64::try_from(intent.signed_transaction.time_to_live()?.as_millis()).ok()?;
    let transaction_expires_at_ms = created_at_ms.checked_add(ttl_ms)?;
    Some(transaction_expires_at_ms.min(intent.staging_receipt.payload.expires_at_ms))
}

/// Exact finalized registry evidence that an archive identity remained absent.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveAbsenceEvidenceV1 {
    /// Deployment-selected chain identity returned by the finalized query.
    pub chain_id: ChainId,
    /// Exact genesis block hash returned by the finalized query.
    pub genesis_block_hash: [u8; 32],
    /// Finalized universal registry snapshot at which the archive was absent.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Consensus-committed creation time of the finalized block named by `snapshot`.
    pub finalized_time_ms: u64,
    /// Exact retention decision proving that the registry did not know the archive.
    pub decision: MusubiArchiveRetentionDecisionV1,
}

impl PublicationArchiveAbsenceEvidenceV1 {
    /// Validate that this evidence names the request's exact deployment and absent archive.
    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationError> {
        self.snapshot
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        self.decision
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        if self.chain_id != request.chain_id
            || self.genesis_block_hash != request.genesis_block_hash
            || self.decision.archive_id != request.archive_commitment.archive_id()
            || self.decision.disposition != MusubiArchiveRetentionDispositionV1::RetainUnknown
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason: "archive-registration absence evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }
}

/// Provable terminal state of one exact archive-registration transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PublicationArchiveRegistrationTerminalReasonV1 {
    /// Finalized transaction status expired the exact transaction.
    #[codec(index = 0)]
    RegistryExpired {
        /// Finalized block when supplied by the authoritative status record.
        block_height: Option<u64>,
    },
    /// A finalized block time passed the exact transaction/receipt validity window.
    #[codec(index = 1)]
    FinalizedValidityWindowElapsed {
        /// Consensus-committed time proving no later block may accept the intent.
        finalized_time_ms: u64,
    },
}

/// Durable terminal and finalized-absence evidence for one exact registration attempt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveRegistrationTerminalV1 {
    /// Exact transaction whose generation may no longer be submitted or retried.
    pub transaction_hash: [u8; 32],
    /// Why the exact transaction is provably terminal.
    pub reason: PublicationArchiveRegistrationTerminalReasonV1,
    /// Finalized exact-identity query proving that no conflicting archive was present.
    pub absence: PublicationArchiveAbsenceEvidenceV1,
}

impl PublicationArchiveRegistrationTerminalV1 {
    /// Construct terminal evidence for a finalized registry expiration.
    #[must_use]
    pub fn registry_expired(
        intent: &PublicationArchiveRegistrationIntentV1,
        block_height: Option<u64>,
        absence: PublicationArchiveAbsenceEvidenceV1,
    ) -> Self {
        Self {
            transaction_hash: intent.transaction_hash,
            reason: PublicationArchiveRegistrationTerminalReasonV1::RegistryExpired {
                block_height,
            },
            absence,
        }
    }

    /// Construct terminal evidence from finalized chain time and exact archive absence.
    #[must_use]
    pub fn finalized_validity_window_elapsed(
        intent: &PublicationArchiveRegistrationIntentV1,
        absence: PublicationArchiveAbsenceEvidenceV1,
    ) -> Self {
        Self {
            transaction_hash: intent.transaction_hash,
            reason:
                PublicationArchiveRegistrationTerminalReasonV1::FinalizedValidityWindowElapsed {
                    finalized_time_ms: absence.finalized_time_ms,
                },
            absence,
        }
    }

    /// Validate this terminal proof against the exact persisted registration intent.
    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<(), PublicationError> {
        self.absence.validate_for(request)?;
        let reason_is_valid = match self.reason {
            PublicationArchiveRegistrationTerminalReasonV1::RegistryExpired { block_height } => {
                block_height.is_none_or(|height| {
                    height > 0 && self.absence.snapshot.finalized_height >= height
                })
            }
            PublicationArchiveRegistrationTerminalReasonV1::FinalizedValidityWindowElapsed {
                finalized_time_ms,
            } => {
                finalized_time_ms == self.absence.finalized_time_ms
                    && archive_registration_intent_valid_until_ms(intent)
                        .is_some_and(|valid_until_ms| finalized_time_ms > valid_until_ms)
            }
        };
        if self.transaction_hash != intent.transaction_hash
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || !reason_is_valid
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason: "archive-registration terminal evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }
}

/// One immutable exact transaction generation and its optional terminal evidence.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveRegistrationAttemptV1 {
    /// One-based contiguous generation number within this operation journal.
    pub generation: u8,
    /// Exact fee-quoted signed transaction intent for this generation.
    pub intent: PublicationArchiveRegistrationIntentV1,
    /// Terminal and finalized-absence evidence, appended before a later generation is allowed.
    pub terminal: Option<PublicationArchiveRegistrationTerminalV1>,
}

impl PublicationArchiveRegistrationAttemptV1 {
    fn new(generation: u8, intent: PublicationArchiveRegistrationIntentV1) -> Self {
        Self {
            generation,
            intent,
            terminal: None,
        }
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationError> {
        self.intent
            .validate_for(operation_id, request, &self.intent.staging_receipt)?;
        if let Some(terminal) = &self.terminal {
            terminal.validate_for(request, &self.intent)?;
        }
        Ok(())
    }
}

/// Result of inspecting or submitting one exact archive-registration generation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the fixed advance protocol returns complete finalized evidence without heap-indirecting its stable API"
)]
pub enum PublicationArchiveRegistrationAdvanceV1 {
    /// The exact transaction remains absent/unknown or pending and must not be replaced.
    Pending,
    /// The exact authoritative archive was recovered from finalized registry state.
    Registered(PublicationRegisteredArchiveV1),
    /// The exact transaction is terminal and the archive is finalized absent/conflict-free.
    TerminalAbsent(PublicationArchiveRegistrationTerminalV1),
}

/// Finalized authoritative archive record recovered for one exact registration transaction.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationRegisteredArchiveV1 {
    /// Exact transaction identity whose finalized effect was recovered.
    pub finalized_transaction_hash: [u8; 32],
    /// Deployment-selected chain identity returned by the finalized query.
    pub chain_id: ChainId,
    /// Exact genesis block hash returned by the finalized query.
    pub genesis_block_hash: [u8; 32],
    /// Finalized registry snapshot that contains the authoritative archive record.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Authoritative archive record embedded in the finalized archive-location query page.
    pub archive: MusubiArchiveRecordV1,
}

impl PublicationRegisteredArchiveV1 {
    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<(), PublicationError> {
        self.archive
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        self.snapshot
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        let observed_finalized_transaction_hash = self.finalized_transaction_hash;
        let expected_intent_transaction_hash = intent.transaction_hash;
        if observed_finalized_transaction_hash != expected_intent_transaction_hash
            || self.chain_id != request.chain_id
            || self.genesis_block_hash != request.genesis_block_hash
            || self.archive.registered_at_height > self.snapshot.finalized_height
            || self.archive.archive_id != intent.archive_id
            || self.archive.archive_id != request.archive_commitment.archive_id()
            || self.archive.commitment != request.archive_commitment
            || self.archive.staging_receipt != intent.staging_receipt
            || self.archive.registered_by != request.publisher
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason: "finalized authoritative archive record conflicts with registration intent"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Main-journal identity of one immutable provider-registration transaction sidecar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationProviderRegistrationTransactionCheckpointV1 {
    /// One-based, provider-local immutable signing attempt.
    pub attempt: u8,
    /// Exact archive location revision carried by the signed registration instruction.
    pub expected_location_revision: u64,
    /// Immutable archive/order/provider identity of the registered proof.
    pub key: MusubiProviderBundleAttestationKeyV1,
    /// Digest of the complete provider attestation carried by the instruction.
    pub attestation_digest: MusubiProviderBundleAttestationDigestV1,
    /// Canonical Iroha hash of the exact signed transaction.
    pub transaction_hash: [u8; 32],
    /// Domain-separated hash of the complete immutable sidecar bytes.
    pub sidecar_hash: [u8; 32],
}

/// Compact main-journal anchor for one location generation's immutable provider sidecars.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationProviderRegistrationCheckpointV1 {
    /// One-based archive-location generation owning these proof registrations.
    pub generation: u8,
    /// Exact archive named by every provider proof.
    pub archive_id: ArchiveId,
    /// Exact replication order named by every provider proof.
    pub replication_order: ReplicationOrderId,
    /// Digest of the immutable sorted provider-proof set.
    pub provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1,
    /// Domain-separated hash of the complete immutable provider-set sidecar.
    pub set_sidecar_hash: [u8; 32],
    /// Append-only identities and hashes of every expected signed-transaction sidecar.
    pub transactions: Vec<PublicationProviderRegistrationTransactionCheckpointV1>,
}

impl PublicationProviderRegistrationCheckpointV1 {
    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
        expected_generation: u8,
    ) -> Result<(), PublicationError> {
        let maximum_transactions = MUSUBI_MAX_LOCATION_PROVIDERS_V1
            .checked_mul(usize::from(MUSUBI_MAX_PROVIDER_REGISTRATION_ATTEMPTS_V1))
            .expect("fixed provider checkpoint bounds fit usize");
        if self.generation != expected_generation
            || self.generation == 0
            || usize::from(self.generation) > MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1
            || self.archive_id != request.archive_commitment.archive_id()
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.provider_attestation_set_digest.is_zero()
            || self.set_sidecar_hash.iter().all(|byte| *byte == 0)
            || self.transactions.len() > maximum_transactions
        {
            return Err(PublicationError::InvalidJournal(
                "provider-registration checkpoint identity or bound is invalid".to_owned(),
            ));
        }

        let mut provider_attempts = BTreeMap::<ProviderId, (u8, u64)>::new();
        for transaction in &self.transactions {
            if transaction.key.validate().is_err()
                || transaction.key.archive_id != self.archive_id
                || transaction.key.replication_order != self.replication_order
                || transaction.attestation_digest.is_zero()
                || transaction.attempt == 0
                || transaction.attempt > MUSUBI_MAX_PROVIDER_REGISTRATION_ATTEMPTS_V1
                || transaction.transaction_hash.iter().all(|byte| *byte == 0)
                || transaction.sidecar_hash.iter().all(|byte| *byte == 0)
            {
                return Err(PublicationError::InvalidJournal(
                    "provider-registration transaction checkpoint is invalid".to_owned(),
                ));
            }
            match provider_attempts.get(&transaction.key.provider_id) {
                None if transaction.attempt == 1 => {}
                Some((prior_attempt, prior_revision))
                    if transaction.attempt == prior_attempt.saturating_add(1)
                        && transaction.expected_location_revision >= *prior_revision => {}
                _ => {
                    return Err(PublicationError::InvalidJournal(
                        "provider-registration attempts are not contiguous and monotonic"
                            .to_owned(),
                    ));
                }
            }
            provider_attempts.insert(
                transaction.key.provider_id,
                (transaction.attempt, transaction.expected_location_revision),
            );
        }
        Ok(())
    }
}

/// Result of checking provider registrations before a location transaction may be signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationProviderRegistrationCheckpointAdvanceV1 {
    /// Every exact provider proof is finalized and all expected sidecars were revalidated.
    Ready,
    /// Persist this append-only compact anchor before any referenced transaction is submitted.
    Updated(PublicationProviderRegistrationCheckpointV1),
}

/// Exact signed compare-and-set transaction prepared for one archive-location generation.
///
/// This value is persisted before submission. A crash can therefore replay or inspect the exact
/// transaction instead of rebuilding a new mutation across an unjournaled phase cut.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveLocationIntentV1 {
    /// Stable publication operation that owns the location generation.
    pub operation_id: PublicationOperationIdV1,
    /// One-based contiguous generation number.
    pub generation: u8,
    /// Finalized complete archive/location page used to choose the CAS revision.
    pub prepared_page: MusubiArchiveLocationPageV1,
    /// Never-before-used stable location identity returned by the coordinator.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Registry-grade pin manifest bound by the exact transaction.
    pub pin_manifest: ManifestDigest,
    /// Replication order bound by the exact transaction and provider-attestation set.
    pub replication_order: ReplicationOrderId,
    /// Digest of the sorted immutable provider-attestation records supplied to Core.
    pub provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1,
    /// Earliest renewal epoch selected by the coordinator.
    pub renew_after_epoch: u64,
    /// Expiry epoch selected by the coordinator.
    pub expires_at_epoch: u64,
    /// Exact archive location-set revision compared by Core.
    pub expected_location_revision: u64,
    /// Domain-separated digest of the canonical location instruction.
    pub instruction_digest: [u8; 32],
    /// Exact fee-quoted signed transaction, durably recorded before submission.
    pub signed_transaction: SignedTransaction,
    /// Canonical hash of `signed_transaction`.
    pub transaction_hash: [u8; 32],
}

impl PublicationArchiveLocationIntentV1 {
    /// Bind one exact signed transaction to its finalized preparation snapshot.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the fixed intent constructor consumes the prepared instruction and signed transaction as one immutable protocol step"
    )]
    pub fn new(
        operation_id: PublicationOperationIdV1,
        generation: u8,
        prepared_page: MusubiArchiveLocationPageV1,
        instruction: AddMusubiArchiveLocationV1,
        signed_transaction: SignedTransaction,
    ) -> Self {
        let transaction_hash = *signed_transaction.hash().as_ref();
        let instruction_digest = archive_location_instruction_digest(&instruction);
        Self {
            operation_id,
            generation,
            prepared_page,
            location_id: instruction.location_id,
            pin_manifest: instruction.pin_manifest,
            replication_order: instruction.replication_order,
            provider_attestation_set_digest: instruction.provider_attestation_set_digest,
            renew_after_epoch: instruction.renew_after_epoch,
            expires_at_epoch: instruction.expires_at_epoch,
            expected_location_revision: instruction.expected_location_revision,
            instruction_digest,
            signed_transaction,
            transaction_hash,
        }
    }

    /// Reconstruct the sole canonical instruction carried by this generation.
    #[must_use]
    pub fn instruction(&self) -> AddMusubiArchiveLocationV1 {
        AddMusubiArchiveLocationV1 {
            archive_id: self.prepared_page.archive.archive_id,
            location_id: self.location_id,
            pin_manifest: self.pin_manifest,
            replication_order: self.replication_order,
            provider_attestation_set_digest: self.provider_attestation_set_digest,
            renew_after_epoch: self.renew_after_epoch,
            expires_at_epoch: self.expires_at_epoch,
            expected_location_revision: self.expected_location_revision,
        }
    }

    pub(crate) fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<(), PublicationError> {
        validate_archive_location_page(request, registered, &self.prepared_page)?;
        let instruction = self.instruction();
        instruction
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
        let expected_instruction: InstructionBox = instruction.into();
        let exact_instruction = matches!(
            self.signed_transaction.instructions(),
            Executable::Instructions(instructions)
                if instructions.len() == 1
                    && instructions.iter().next() == Some(&expected_instruction)
        );
        let expected_generation = u8::try_from(prior_location_ids.len() + 1).ok();
        if self.operation_id != operation_id
            || Some(self.generation) != expected_generation
            || usize::from(self.generation) > MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1
            || self.location_id.is_zero()
            || prior_location_ids.contains(&self.location_id)
            || prior_location_ids.iter().any(|location_id| {
                self.prepared_page
                    .archive
                    .location_ids
                    .binary_search(location_id)
                    .is_ok()
                    || self
                        .prepared_page
                        .items
                        .binary_search_by_key(location_id, |location| location.location_id)
                        .is_ok()
            })
            || self
                .prepared_page
                .archive
                .location_ids
                .binary_search(&self.location_id)
                .is_ok()
            || self.expected_location_revision != self.prepared_page.archive.location_revision
            || self.expected_location_revision == u64::MAX
            || self.instruction_digest != archive_location_instruction_digest(&self.instruction())
            || self.pin_manifest.as_bytes().iter().all(|byte| *byte == 0)
            || self
                .replication_order
                .as_bytes()
                .iter()
                .all(|byte| *byte == 0)
            || self.renew_after_epoch >= self.expires_at_epoch
            || self.transaction_hash != *self.signed_transaction.hash().as_ref()
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || self.signed_transaction.network_id().copied() != Some(request.network_id())
            || self.signed_transaction.authority() != &request.publisher
            || self.signed_transaction.verify_signature().is_err()
            || !exact_instruction
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason:
                    "archive-location intent, CAS revision, or signed transaction was substituted"
                        .to_owned(),
            });
        }
        Ok(())
    }
}

/// Finalized application evidence for one exact archive-location transaction.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveRegistrationV1 {
    /// Exact signed transaction that was journaled before submission.
    pub intent: PublicationArchiveLocationIntentV1,
    /// Authoritative block height at which that exact transaction was applied.
    pub applied_height: u64,
    /// Complete finalized current archive/location page at or after application.
    pub finalized_page: MusubiArchiveLocationPageV1,
}

impl PublicationArchiveRegistrationV1 {
    /// Return the stable location identity for this generation.
    #[must_use]
    pub const fn location_id(&self) -> MusubiArchiveLocationIdV1 {
        self.intent.location_id
    }

    /// Return the exact location from the finalized page.
    pub(crate) fn location(&self) -> Result<&MusubiArchiveLocationV1, PublicationError> {
        self.finalized_page
            .items
            .binary_search_by_key(&self.intent.location_id, |location| location.location_id)
            .ok()
            .map(|index| &self.finalized_page.items[index])
            .ok_or_else(|| {
                PublicationError::InvalidJournal(
                    "finalized archive-location generation is missing its location".to_owned(),
                )
            })
    }

    pub(crate) fn validate_polled_page(
        &self,
        request: &PublicationRequestV1,
        page: &MusubiArchiveLocationPageV1,
    ) -> Result<(), PublicationError> {
        page.validate()
            .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
        let registered_location = self.location()?;
        let observed_location = page
            .items
            .binary_search_by_key(&self.intent.location_id, |location| location.location_id)
            .ok()
            .map(|index| &page.items[index]);
        let location_regressed = observed_location.is_some_and(|location| {
            !matches!(
                location_progress(registered_location, location),
                Ok(PublicationLocationProgressV1::Current)
            )
        });
        let observed_genesis_hash = page.genesis_hash;
        let expected_genesis_hash = request.genesis_block_hash;
        if page.chain_id != request.chain_id
            || observed_genesis_hash != expected_genesis_hash
            || page.archive.registration_projection()
                != self.finalized_page.archive.registration_projection()
            || page.snapshot.finalized_height < self.finalized_page.snapshot.finalized_height
            || page.snapshot.index_revision < self.finalized_page.snapshot.index_revision
            || (page.snapshot.finalized_height == self.finalized_page.snapshot.finalized_height
                && page.snapshot != self.finalized_page.snapshot)
            || page.archive.location_revision < self.finalized_page.archive.location_revision
            || (page.snapshot == self.finalized_page.snapshot
                && (page.archive != self.finalized_page.archive
                    || page.items != self.finalized_page.items))
            || (page.archive.location_revision == self.finalized_page.archive.location_revision
                && (page.archive != self.finalized_page.archive
                    || page.items != self.finalized_page.items))
            || page.next_cursor.is_some()
            || page.items.len() != page.archive.location_ids.len()
            || page
                .items
                .iter()
                .zip(&page.archive.location_ids)
                .any(|(location, location_id)| {
                    location.location_id != *location_id
                        || location.archive_id != page.archive.archive_id
                        || location.finalized_height > page.snapshot.finalized_height
                        || location.state == MusubiArchiveLocationStateV1::Retired
                        || location.validate().is_err()
                })
            || location_regressed
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "polled finalized archive-location page was incomplete or substituted"
                    .to_owned(),
            });
        }
        Ok(())
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<(), PublicationError> {
        self.intent
            .validate_for(operation_id, request, registered, prior_location_ids)?;
        validate_archive_location_page(request, registered, &self.finalized_page)?;
        let location = self.location()?;
        if self.applied_height <= self.intent.prepared_page.snapshot.finalized_height
            || self.applied_height > self.finalized_page.snapshot.finalized_height
            || self.finalized_page.snapshot.finalized_height
                < self.intent.prepared_page.snapshot.finalized_height
            || self.finalized_page.snapshot.index_revision
                < self.intent.prepared_page.snapshot.index_revision
            || (self.finalized_page.snapshot.finalized_height
                == self.intent.prepared_page.snapshot.finalized_height
                && self.finalized_page.snapshot != self.intent.prepared_page.snapshot)
            || self.finalized_page.archive.location_revision
                <= self.intent.expected_location_revision
            || location.revision <= self.intent.expected_location_revision
            || (location.revision == self.intent.expected_location_revision + 1
                && (location.finalized_height != self.applied_height
                    || location.state != MusubiArchiveLocationStateV1::Healthy
                    || location.pin_manifest != self.intent.pin_manifest
                    || location.replication_order != self.intent.replication_order
                    || location.provider_attestation_set_digest
                        != self.intent.provider_attestation_set_digest
                    || location.renew_after_epoch != self.intent.renew_after_epoch
                    || location.expires_at_epoch != self.intent.expires_at_epoch))
            || location.location_id != self.intent.location_id
            || location.archive_id != request.archive_commitment.archive_id()
            || location.finalized_height < self.applied_height
            || location.state == MusubiArchiveLocationStateV1::Retired
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                reason: "archive-location application or finalized state was substituted"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Why one exact archive-location generation can never become the active generation again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PublicationArchiveLocationTerminalReasonV1 {
    /// The exact CAS transaction was finalized rejected after another location mutation rebased it.
    #[codec(index = 0)]
    RejectedRebase {
        /// Finalized rejection height.
        block_height: u64,
    },
    /// The exact signed transaction expired without application.
    #[codec(index = 1)]
    RegistryExpired {
        /// Finalized height when one was supplied by the authoritative status record.
        block_height: Option<u64>,
    },
    /// The exact transaction applied, then the stable identity was retired before recovery.
    #[codec(index = 2)]
    AppliedThenRetired {
        /// Authoritative application height for the exact signed transaction.
        applied_height: u64,
    },
    /// A previously finalized active generation was later retired.
    #[codec(index = 3)]
    Retired,
}

/// Durable finalized floor against which one archive-location terminal was accepted.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[allow(
    clippy::large_enum_variant,
    reason = "the stable journal enum retains exact finalized replication evidence inline for deterministic recovery"
)]
pub enum PublicationArchiveLocationTerminalFloorV1 {
    /// The transaction never acquired finalized application evidence; use its prepared page.
    #[codec(index = 0)]
    Prepared,
    /// No healthy replication checkpoint existed; use the finalized application page.
    #[codec(index = 1)]
    Registered,
    /// A later healthy full-directory checkpoint existed and is retained exactly.
    #[codec(index = 2)]
    Replication(PublicationReplicationCheckpointV1),
}

/// Finalized full-directory evidence terminating one archive-location generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveLocationTerminalV1 {
    /// Exact signed transaction identity owned by the generation.
    pub transaction_hash: [u8; 32],
    /// Authoritative reason this generation cannot be retried.
    pub reason: PublicationArchiveLocationTerminalReasonV1,
    /// Complete finalized directory proving the generation's identity is absent.
    pub finalized_page: MusubiArchiveLocationPageV1,
}

impl PublicationArchiveLocationTerminalV1 {
    #[allow(
        clippy::too_many_lines,
        reason = "the terminal validator keeps all finalized-floor, absence, and retirement invariants in one security-sensitive state check"
    )]
    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered_archive: &PublicationRegisteredArchiveV1,
        attempt: &PublicationArchiveLocationAttemptV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
        floor: &PublicationArchiveLocationTerminalFloorV1,
    ) -> Result<(), PublicationError> {
        attempt.intent.validate_for(
            operation_id,
            request,
            registered_archive,
            prior_location_ids,
        )?;
        validate_archive_location_page(request, registered_archive, &self.finalized_page)?;
        let floor_page = match (floor, attempt.registration.as_ref()) {
            (PublicationArchiveLocationTerminalFloorV1::Prepared, None) => {
                &attempt.intent.prepared_page
            }
            (PublicationArchiveLocationTerminalFloorV1::Registered, Some(registration)) => {
                &registration.finalized_page
            }
            (
                PublicationArchiveLocationTerminalFloorV1::Replication(checkpoint),
                Some(registration),
            ) => {
                checkpoint.validate_for(request, registration)?;
                &checkpoint.finalized_page
            }
            _ => {
                return Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Replication,
                    reason: "archive-location terminal used an invalid durable floor".to_owned(),
                });
            }
        };
        let requires_strict_revision = !matches!(
            self.reason,
            PublicationArchiveLocationTerminalReasonV1::RegistryExpired { .. }
        );
        if finalized_page_progress(floor_page, &self.finalized_page)?
            != PublicationLocationProgressV1::Current
            || (requires_strict_revision
                && self.finalized_page.archive.location_revision
                    <= floor_page.archive.location_revision)
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "archive-location terminal regressed its durable finalized floor"
                    .to_owned(),
            });
        }
        let absent = self
            .finalized_page
            .archive
            .location_ids
            .binary_search(&attempt.intent.location_id)
            .is_err()
            && self
                .finalized_page
                .items
                .binary_search_by_key(&attempt.intent.location_id, |location| location.location_id)
                .is_err();
        let reason_is_valid = match self.reason {
            PublicationArchiveLocationTerminalReasonV1::RejectedRebase { block_height } => {
                attempt.registration.is_none()
                    && block_height > attempt.intent.prepared_page.snapshot.finalized_height
                    && block_height <= self.finalized_page.snapshot.finalized_height
                    && self.finalized_page.archive.location_revision
                        > attempt.intent.expected_location_revision
            }
            PublicationArchiveLocationTerminalReasonV1::RegistryExpired { block_height } => {
                attempt.registration.is_none()
                    && block_height.is_none_or(|height| {
                        height > attempt.intent.prepared_page.snapshot.finalized_height
                            && height <= self.finalized_page.snapshot.finalized_height
                    })
                    && self.finalized_page.archive.location_revision
                        >= attempt.intent.expected_location_revision
            }
            PublicationArchiveLocationTerminalReasonV1::AppliedThenRetired { applied_height } => {
                attempt.registration.is_none()
                    && applied_height > attempt.intent.prepared_page.snapshot.finalized_height
                    && applied_height <= self.finalized_page.snapshot.finalized_height
                    && self.finalized_page.archive.location_revision
                        > attempt.intent.expected_location_revision.saturating_add(1)
            }
            PublicationArchiveLocationTerminalReasonV1::Retired => {
                attempt.registration.as_ref().is_some_and(|registration| {
                    self.finalized_page.snapshot.finalized_height
                        > registration.finalized_page.snapshot.finalized_height
                        && self.finalized_page.snapshot.index_revision
                            >= registration.finalized_page.snapshot.index_revision
                        && self.finalized_page.archive.location_revision
                            > registration.finalized_page.archive.location_revision
                })
            }
        };
        if self.transaction_hash != attempt.intent.transaction_hash
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || self.finalized_page.snapshot.finalized_height
                < attempt.intent.prepared_page.snapshot.finalized_height
            || self.finalized_page.snapshot.index_revision
                < attempt.intent.prepared_page.snapshot.index_revision
            || (self.finalized_page.snapshot.finalized_height
                == attempt.intent.prepared_page.snapshot.finalized_height
                && self.finalized_page.snapshot != attempt.intent.prepared_page.snapshot)
            || (self.finalized_page.snapshot == attempt.intent.prepared_page.snapshot
                && (self.finalized_page.archive != attempt.intent.prepared_page.archive
                    || self.finalized_page.items != attempt.intent.prepared_page.items))
            || (self.finalized_page.archive.location_revision
                == attempt.intent.prepared_page.archive.location_revision
                && (self.finalized_page.archive != attempt.intent.prepared_page.archive
                    || self.finalized_page.items != attempt.intent.prepared_page.items))
            || !absent
            || !reason_is_valid
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "archive-location terminal or retirement evidence was substituted"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// One bounded append-only archive-location transaction generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationArchiveLocationAttemptV1 {
    /// One-based contiguous generation number.
    pub generation: u8,
    /// Exact signed transaction persisted before its first submission.
    pub intent: PublicationArchiveLocationIntentV1,
    /// Finalized application evidence, appended after the transaction applies.
    pub registration: Option<PublicationArchiveRegistrationV1>,
    /// Finalized terminal evidence, appended before a later generation is allowed.
    pub terminal: Option<PublicationArchiveLocationTerminalV1>,
    /// Finalized floor against which `terminal` was accepted, retained for journal recovery.
    pub terminal_floor: Option<PublicationArchiveLocationTerminalFloorV1>,
}

impl PublicationArchiveLocationAttemptV1 {
    fn new(generation: u8, intent: PublicationArchiveLocationIntentV1) -> Self {
        Self {
            generation,
            intent,
            registration: None,
            terminal: None,
            terminal_floor: None,
        }
    }
}

/// Result of submitting or recovering one exact archive-location transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the fixed advance protocol returns complete finalized location evidence without changing its stable API"
)]
pub enum PublicationArchiveLocationAdvanceV1 {
    /// The exact transaction remains pending or not yet authoritatively terminal.
    Pending,
    /// The exact transaction and current same-ID location are finalized.
    Registered(PublicationArchiveRegistrationV1),
    /// The exact transaction is terminal and the complete finalized directory excludes its ID.
    Terminal(PublicationArchiveLocationTerminalV1),
}

/// Result of polling the current finalized state of the active location generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationReplicationAdvanceV1 {
    /// The location exists but is not currently healthy at quorum, or the query is not yet current.
    Pending,
    /// Current finalized healthy location and the complete directory snapshot that authenticated it.
    Healthy(PublicationReplicationCheckpointV1),
    /// Complete finalized proof that the stable location identity was retired.
    Retired(PublicationArchiveLocationTerminalV1),
}

/// Durable finalized replication floor for one active archive-location generation.
///
/// Retaining the complete directory page prevents a later lagging absence response from being
/// mistaken for retirement after the target location or another directory entry has advanced.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReplicationCheckpointV1 {
    /// Complete finalized archive-location directory containing the active healthy location.
    pub finalized_page: MusubiArchiveLocationPageV1,
}

impl PublicationReplicationCheckpointV1 {
    /// Return the active generation's exact location from this finalized page.
    pub(crate) fn location(
        &self,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<&MusubiArchiveLocationV1, PublicationError> {
        self.finalized_page
            .items
            .binary_search_by_key(&registration.location_id(), |location| location.location_id)
            .ok()
            .map(|index| &self.finalized_page.items[index])
            .ok_or_else(|| PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "finalized replication checkpoint is missing its active location"
                    .to_owned(),
            })
    }

    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<(), PublicationError> {
        self.finalized_page
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
        registration.validate_polled_page(request, &self.finalized_page)?;
        validate_replication(request, registration, self.location(registration)?)
    }
}

pub(crate) fn validate_archive_location_page(
    request: &PublicationRequestV1,
    registered: &PublicationRegisteredArchiveV1,
    page: &MusubiArchiveLocationPageV1,
) -> Result<(), PublicationError> {
    page.validate()
        .map_err(|error| invalid(PublicationPhaseV1::ArchiveRegistration, error))?;
    let observed_genesis_hash = page.genesis_hash;
    let expected_genesis_hash = request.genesis_block_hash;
    if page.chain_id != request.chain_id
        || observed_genesis_hash != expected_genesis_hash
        || page.archive.registration_projection() != registered.archive.registration_projection()
        || page.snapshot.finalized_height < registered.snapshot.finalized_height
        || page.snapshot.index_revision < registered.snapshot.index_revision
        || (page.snapshot.finalized_height == registered.snapshot.finalized_height
            && page.snapshot != registered.snapshot)
        || page.archive.location_revision < registered.archive.location_revision
        || (page.snapshot == registered.snapshot && page.archive != registered.archive)
        || page.next_cursor.is_some()
        || page.items.len() != page.archive.location_ids.len()
        || page
            .items
            .iter()
            .zip(&page.archive.location_ids)
            .any(|(location, location_id)| {
                location.location_id != *location_id
                    || location.archive_id != page.archive.archive_id
                    || location.finalized_height > page.snapshot.finalized_height
                    || location.state == MusubiArchiveLocationStateV1::Retired
                    || location.validate().is_err()
            })
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ArchiveRegistration,
            reason: "finalized archive-location page was incomplete or substituted".to_owned(),
        });
    }
    Ok(())
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
            || self.commitment != request.archive_commitment
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

fn validate_readback_subset(
    request: &PublicationRequestV1,
    location: &MusubiArchiveLocationV1,
    readbacks: &[PublicationReadbackEvidenceV1],
) -> Result<(), PublicationError> {
    let is_strictly_ordered = readbacks
        .windows(2)
        .all(|pair| pair[0].provider < pair[1].provider);
    let all_are_location_providers = readbacks
        .iter()
        .all(|readback| location.providers.binary_search(&readback.provider).is_ok());
    if readbacks.len() != 2 || !is_strictly_ordered || !all_are_location_providers {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Readback,
            reason:
                "provider readbacks were not a strictly ordered distinct location-provider subset"
                    .to_owned(),
        });
    }
    for readback in readbacks {
        readback.validate_for(request, location, readback.provider)?;
    }
    Ok(())
}

/// Finalized replication and readback floor that authorized one release signature.
///
/// The floor is retained inside each attempt rather than inferred from the journal head. A
/// location retirement can therefore move the workflow back to replication without erasing the
/// exact storage evidence under which an earlier, potentially submitted transaction was signed.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleasePreparationFloorV1 {
    /// One-based archive-location generation that produced the selected location.
    pub location_generation: u8,
    /// Exact healthy location selected from `replication`.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Complete finalized healthy-location directory used before signing.
    pub replication: PublicationReplicationCheckpointV1,
    /// Exact two-provider readbacks used before signing.
    pub readbacks: Vec<PublicationReadbackEvidenceV1>,
}

impl PublicationReleasePreparationFloorV1 {
    /// Construct and validate an independently auditable release-preparation floor.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence when the replication checkpoint, selected location generation, or
    /// two provider readbacks do not match the immutable publication request.
    pub fn try_new(
        location_generation: u8,
        replication: PublicationReplicationCheckpointV1,
        readbacks: Vec<PublicationReadbackEvidenceV1>,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<Self, PublicationError> {
        let floor = Self {
            location_generation,
            location_id: registration.location_id(),
            replication,
            readbacks,
        };
        floor.validate_for(request)?;
        floor.validate_for_registration(request, registration)?;
        Ok(floor)
    }

    fn validate_for(&self, request: &PublicationRequestV1) -> Result<(), PublicationError> {
        let page = &self.replication.finalized_page;
        page.validate()
            .map_err(|error| invalid(PublicationPhaseV1::ReleaseSubmission, error))?;
        let location = page
            .items
            .binary_search_by_key(&self.location_id, |location| location.location_id)
            .ok()
            .map(|index| &page.items[index]);
        let exact_archive = page.archive.archive_id == request.archive_commitment.archive_id()
            && page.archive.commitment == request.archive_commitment
            && page.archive.registered_by == request.publisher
            && page
                .archive
                .staging_receipt
                .verify(
                    &request.receipt_binding(),
                    page.archive.staging_receipt.payload.issued_at_ms,
                )
                .is_ok();
        let observed_genesis_hash = page.genesis_hash;
        let expected_genesis_hash = request.genesis_block_hash;
        if page.chain_id != request.chain_id
            || observed_genesis_hash != expected_genesis_hash
            || !exact_archive
            || page.next_cursor.is_some()
            || page.items.len() != page.archive.location_ids.len()
            || self.readbacks.len() != 2
            || location.is_none()
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "release preparation floor was incomplete or substituted".to_owned(),
            });
        }
        let location = location.expect("checked above");
        if location.state != MusubiArchiveLocationStateV1::Healthy
            || location.providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "release preparation floor was not healthy at replication quorum"
                    .to_owned(),
            });
        }
        validate_readback_subset(request, location, &self.readbacks)
    }

    fn location(&self) -> Option<&MusubiArchiveLocationV1> {
        self.replication
            .finalized_page
            .items
            .binary_search_by_key(&self.location_id, |location| location.location_id)
            .ok()
            .map(|index| &self.replication.finalized_page.items[index])
    }

    fn validate_for_registration(
        &self,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<(), PublicationError> {
        if self.location_generation == 0
            || self.location_generation != registration.intent.generation
            || self.location_id != registration.location_id()
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "release preparation floor names a different location generation"
                    .to_owned(),
            });
        }
        self.replication.validate_for(request, registration)?;
        let location = self.replication.location(registration)?;
        validate_readback_subset(request, location, &self.readbacks)
    }
}

/// Compact, reconstructable authorization envelope for an exact release transaction.
///
/// Immutable request fields and the sole publish instruction are deliberately not duplicated.
/// Metadata is always empty and proof attachments are always absent in V1.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleaseSignedEnvelopeV1 {
    /// Exact signature-bound creation time in Unix milliseconds.
    pub creation_time_ms: u64,
    /// Exact non-zero signature-bound transaction lifetime in milliseconds.
    pub time_to_live_ms: NonZeroU64,
    /// Optional exact signature-bound transaction nonce.
    pub nonce: Option<NonZeroU32>,
    /// Exact fee payer and charge bounds accepted by the signer.
    pub fee_payment: FeePaymentIntent,
    /// Primary transaction signature retained byte-for-byte.
    pub signature: TransactionSignature,
    /// Complete canonical multisig proof bundle, when the publisher is multisig.
    pub multisig_signatures: Option<MultisigSignatures>,
}

impl PublicationReleaseSignedEnvelopeV1 {
    /// Extract a compact envelope only from the exact canonical V1 release transaction shape.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence when the transaction shape is not the fixed release protocol or
    /// the compact envelope cannot reconstruct its exact signed bytes.
    pub fn try_from_signed_transaction(
        request: &PublicationRequestV1,
        signed_transaction: &SignedTransaction,
    ) -> Result<Self, PublicationError> {
        validate_release_signed_transaction_shape(request, signed_transaction)?;
        let payload = signed_transaction.payload();
        let envelope = Self {
            creation_time_ms: payload.creation_time_ms,
            time_to_live_ms: payload
                .time_to_live_ms
                .expect("release transaction shape requires a non-zero lifetime"),
            nonce: payload.nonce,
            fee_payment: payload.fee_payment.clone(),
            signature: signed_transaction.signature().clone(),
            multisig_signatures: signed_transaction.multisig_signatures().cloned(),
        };
        let reconstructed = envelope.reconstruct_signed_transaction(request)?;
        if &reconstructed != signed_transaction
            || release_signed_transaction_wire_v1(&reconstructed)?
                != release_signed_transaction_wire_v1(signed_transaction)?
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "compact release envelope did not reconstruct the exact signed transaction"
                    .to_owned(),
            });
        }
        Ok(envelope)
    }

    /// Reconstruct and validate the sole exact signed release transaction.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence when the compact payload, multisig bundle, or signature cannot
    /// reconstruct the fixed release transaction shape.
    pub fn reconstruct_signed_transaction(
        &self,
        request: &PublicationRequestV1,
    ) -> Result<SignedTransaction, PublicationError> {
        let payload = TransactionPayload {
            domain: iroha_data_model::transaction::TransactionDomain::Network(request.network_id()),
            authority: request.publisher.clone(),
            creation_time_ms: self.creation_time_ms,
            instructions: vec![InstructionBox::from(request.publish_instruction())].into(),
            time_to_live_ms: Some(self.time_to_live_ms),
            nonce: self.nonce,
            fee_payment: self.fee_payment.clone(),
            metadata: Metadata::default(),
            attachments: None,
        };
        let mut builder = TransactionBuilder::from_payload(payload).map_err(|error| {
            PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: format!("compact release envelope has an invalid payload: {error}"),
            }
        })?;
        if let Some(multisig_signatures) = &self.multisig_signatures {
            builder = builder.with_multisig_signatures(multisig_signatures.clone());
        }
        let signed_transaction = builder.build_with_signature(self.signature.payload().clone());
        validate_release_signed_transaction_shape(request, &signed_transaction)?;
        Ok(signed_transaction)
    }
}

/// Durable compact intent for one exact fee-quoted release transaction.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleaseSubmissionIntentV1 {
    /// Stable publication operation that owns this intent.
    pub operation_id: PublicationOperationIdV1,
    /// Exact finalized replication and two-provider readback floor used before signing.
    pub preparation: PublicationReleasePreparationFloorV1,
    /// Domain-separated digest of the sole [`PublishMusubiReleaseV1`] instruction.
    pub instruction_digest: [u8; 32],
    /// Local replay-integrity digest of the complete fixed-V1 signed transaction wire bytes.
    ///
    /// Authoritative transaction status is keyed by `transaction_hash` and does not attest
    /// this authorization-inclusive digest.
    // TODO: Bind this digest to authoritative committed-wire evidence when Torii exposes it.
    pub signed_transaction_digest: [u8; 32],
    /// Iroha transaction identity, which intentionally excludes authorization proofs.
    pub transaction_hash: [u8; 32],
    /// Minimal fields needed to reconstruct the exact signed transaction.
    pub envelope: PublicationReleaseSignedEnvelopeV1,
}

impl PublicationReleaseSubmissionIntentV1 {
    /// Extract, reconstruct, and bind one exact signed transaction before any submission.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence when the preparation floor or signed transaction is malformed,
    /// substituted, or exceeds the fixed release-intent budget.
    pub fn try_new(
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        preparation: PublicationReleasePreparationFloorV1,
        signed_transaction: &SignedTransaction,
    ) -> Result<Self, PublicationError> {
        preparation.validate_for(request)?;
        let envelope = PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
            request,
            signed_transaction,
        )?;
        let wire = release_signed_transaction_wire_v1(signed_transaction)?;
        let intent = Self {
            operation_id,
            preparation,
            instruction_digest: domain_hash(
                PUBLISH_INSTRUCTION_DOMAIN,
                &request.publish_instruction().encode(),
            ),
            signed_transaction_digest: domain_hash(RELEASE_SIGNED_TRANSACTION_DOMAIN, &wire),
            transaction_hash: *signed_transaction.hash().as_ref(),
            envelope,
        };
        intent.validate_for(operation_id, request)?;
        Ok(intent)
    }

    /// Reconstruct and validate the complete transaction represented by this compact intent.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence when the compact envelope, instruction digest, signed-wire
    /// digest, transaction hash, or operation binding differs.
    pub fn reconstruct_signed_transaction(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
    ) -> Result<SignedTransaction, PublicationError> {
        self.preparation.validate_for(request)?;
        let signed_transaction = self.envelope.reconstruct_signed_transaction(request)?;
        let wire = release_signed_transaction_wire_v1(&signed_transaction)?;
        if self.operation_id != operation_id
            || self.instruction_digest
                != domain_hash(
                    PUBLISH_INSTRUCTION_DOMAIN,
                    &request.publish_instruction().encode(),
                )
            || self.signed_transaction_digest
                != domain_hash(RELEASE_SIGNED_TRANSACTION_DOMAIN, &wire)
            || self.signed_transaction_digest.iter().all(|byte| *byte == 0)
            || self.transaction_hash != *signed_transaction.hash().as_ref()
            || self.transaction_hash.iter().all(|byte| *byte == 0)
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "compact release submission intent was substituted".to_owned(),
            });
        }
        Ok(signed_transaction)
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_INTENT_CANONICAL_BYTES,
            "release submission intent",
            PublicationPhaseV1::ReleaseSubmission,
        )?;
        self.reconstruct_signed_transaction(operation_id, request)
            .map(|_| ())
    }
}

/// Synchronized finalized proof that the exact release was absent at one consensus time.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleaseAbsenceEvidenceV1 {
    /// Complete empty universal resolver page for the exact package and `=VERSION` requirement.
    pub resolver_page: MusubiResolverIndexPageV1,
    /// Exact archive-retention request pinned to `resolver_page.snapshot`.
    pub retention_query: MusubiArchiveRetentionQueryV1,
    /// Complete same-snapshot retention response and consensus-committed block time.
    pub retention_page: MusubiArchiveRetentionPageV1,
}

impl PublicationReleaseAbsenceEvidenceV1 {
    pub(crate) fn validate_for(
        &self,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationError> {
        self.resolver_page
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ReleaseSubmission, error))?;
        self.retention_query
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ReleaseSubmission, error))?;
        self.retention_page
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::ReleaseSubmission, error))?;
        let exact_requirement = MusubiVersionReqV1::from_str(&format!(
            "={}",
            request.publication.manifest.release.version
        ))
        .map_err(|error| invalid(PublicationPhaseV1::ReleaseSubmission, error))?;
        let exact_retention_query = MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![request.archive_commitment.archive_id()],
            expected_snapshot: Some(self.resolver_page.snapshot),
        };
        let [retention] = self.retention_page.items.as_slice() else {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "finalized exact-release absence evidence had a partial retention page"
                    .to_owned(),
            });
        };
        if self.resolver_page.chain_id != request.chain_id
            || self.resolver_page.genesis_hash != request.genesis_block_hash
            || self.resolver_page.query.package != request.publication.manifest.release.package
            || self.resolver_page.query.requirement.as_ref() != Some(&exact_requirement)
            || self.resolver_page.query.page.cursor.is_some()
            || !self.resolver_page.items.is_empty()
            || self.resolver_page.next_cursor.is_some()
            || self.retention_query != exact_retention_query
            || self.retention_page.chain_id != request.chain_id
            || self.retention_page.genesis_hash != request.genesis_block_hash
            || self.retention_page.snapshot != self.resolver_page.snapshot
            || self.retention_page.finalized_time_ms == 0
            || retention.archive_id != request.archive_commitment.archive_id()
            || retention.disposition == MusubiArchiveRetentionDispositionV1::RetainUnknown
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "finalized exact-release absence evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }

    fn snapshot(&self) -> MusubiRegistrySnapshotV1 {
        self.resolver_page.snapshot
    }

    fn finalized_time_ms(&self) -> u64 {
        self.retention_page.finalized_time_ms
    }
}

/// Authoritative terminal classification for one exact release transaction generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum PublicationReleaseSubmissionTerminalReasonV1 {
    /// The exact transaction reached an authoritative finalized expired status.
    #[codec(index = 0)]
    RegistryExpired {
        /// Finalized block supplied by the authoritative status record.
        block_height: u64,
        /// Same-or-later synchronized finalized exact-release absence.
        absence: PublicationReleaseAbsenceEvidenceV1,
    },
    /// Consensus time passed the signature-bound deadline while the exact release stayed absent.
    #[codec(index = 1)]
    FinalizedValidityWindowElapsed {
        /// Synchronized exact-release absence and consensus-time evidence.
        absence: PublicationReleaseAbsenceEvidenceV1,
    },
    /// Core finalized a deterministic rejection while the exact release stayed absent.
    #[codec(index = 2)]
    RegistryRejected {
        /// Finalized rejection height.
        block_height: u64,
        /// Same-or-later synchronized finalized exact-release absence.
        absence: PublicationReleaseAbsenceEvidenceV1,
    },
}

/// Terminal evidence appended before a successor release signature may be persisted.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleaseSubmissionTerminalV1 {
    /// Iroha transaction identity of the exact terminal attempt.
    pub transaction_hash: [u8; 32],
    /// Full signed-wire digest, including every authorization proof byte.
    pub signed_transaction_digest: [u8; 32],
    /// Authoritative terminal classification.
    pub reason: PublicationReleaseSubmissionTerminalReasonV1,
}

impl PublicationReleaseSubmissionTerminalV1 {
    /// Bind an authoritative expired status to one exact compact intent.
    #[must_use]
    pub fn registry_expired(
        intent: &PublicationReleaseSubmissionIntentV1,
        block_height: u64,
        absence: PublicationReleaseAbsenceEvidenceV1,
    ) -> Self {
        Self {
            transaction_hash: intent.transaction_hash,
            signed_transaction_digest: intent.signed_transaction_digest,
            reason: PublicationReleaseSubmissionTerminalReasonV1::RegistryExpired {
                block_height,
                absence,
            },
        }
    }

    /// Bind finalized consensus-time expiry and exact-release absence to one intent.
    #[must_use]
    pub fn finalized_validity_window_elapsed(
        intent: &PublicationReleaseSubmissionIntentV1,
        absence: PublicationReleaseAbsenceEvidenceV1,
    ) -> Self {
        Self {
            transaction_hash: intent.transaction_hash,
            signed_transaction_digest: intent.signed_transaction_digest,
            reason: PublicationReleaseSubmissionTerminalReasonV1::FinalizedValidityWindowElapsed {
                absence,
            },
        }
    }

    /// Bind an authoritative finalized rejection to one exact compact intent.
    #[must_use]
    pub fn registry_rejected(
        intent: &PublicationReleaseSubmissionIntentV1,
        block_height: u64,
        absence: PublicationReleaseAbsenceEvidenceV1,
    ) -> Self {
        Self {
            transaction_hash: intent.transaction_hash,
            signed_transaction_digest: intent.signed_transaction_digest,
            reason: PublicationReleaseSubmissionTerminalReasonV1::RegistryRejected {
                block_height,
                absence,
            },
        }
    }

    fn validate_for(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationReleaseSubmissionIntentV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_OUTCOME_CANONICAL_BYTES,
            "release terminal evidence",
            PublicationPhaseV1::ReleaseSubmission,
        )?;
        let preparation_height = intent
            .preparation
            .replication
            .finalized_page
            .snapshot
            .finalized_height;
        let preparation_index_revision = intent
            .preparation
            .replication
            .finalized_page
            .snapshot
            .index_revision;
        let reason_is_valid = match &self.reason {
            PublicationReleaseSubmissionTerminalReasonV1::RegistryExpired {
                block_height,
                absence,
            }
            | PublicationReleaseSubmissionTerminalReasonV1::RegistryRejected {
                block_height,
                absence,
            } => {
                absence.validate_for(request)?;
                *block_height > preparation_height
                    && absence.snapshot().finalized_height >= *block_height
            }
            PublicationReleaseSubmissionTerminalReasonV1::FinalizedValidityWindowElapsed {
                absence,
            } => {
                absence.validate_for(request)?;
                absence.snapshot().finalized_height > preparation_height
                    && release_submission_valid_until_ms(intent)
                        .is_some_and(|deadline| absence.finalized_time_ms() > deadline)
            }
        };
        if self.transaction_hash != intent.transaction_hash
            || self.signed_transaction_digest != intent.signed_transaction_digest
            || self.absence().snapshot().index_revision < preparation_index_revision
            || !reason_is_valid
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                reason: "release submission terminal evidence was substituted".to_owned(),
            });
        }
        Ok(())
    }

    fn absence(&self) -> &PublicationReleaseAbsenceEvidenceV1 {
        match &self.reason {
            PublicationReleaseSubmissionTerminalReasonV1::RegistryExpired { absence, .. }
            | PublicationReleaseSubmissionTerminalReasonV1::FinalizedValidityWindowElapsed {
                absence,
            }
            | PublicationReleaseSubmissionTerminalReasonV1::RegistryRejected { absence, .. } => {
                absence
            }
        }
    }
}

/// Append-only authoritative outcome of one exact release transaction generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[allow(
    clippy::large_enum_variant,
    reason = "the stable wire enum retains complete applied or terminal evidence inline for deterministic journal recovery"
)]
pub enum PublicationReleaseSubmissionOutcomeV1 {
    /// The journaled payload transaction was finalized applied through Native AMX.
    #[codec(index = 0)]
    Applied {
        /// Existing application evidence retained for final verification.
        submission: PublicationAmxSubmissionV1,
        /// Local signed-wire replay binding retained alongside payload application evidence.
        signed_transaction_digest: [u8; 32],
    },
    /// The exact transaction became provably terminal without application.
    #[codec(index = 1)]
    Terminal(PublicationReleaseSubmissionTerminalV1),
}

/// Authoritative payload progress for one already-journaled release transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the fixed backend advance API returns complete authoritative evidence without heap indirection"
)]
pub enum PublicationReleaseSubmissionAdvanceV1 {
    /// The payload transaction is absent, live, pending, or not yet authoritatively resolved.
    Pending,
    /// The payload transaction was finalized as applied through Native AMX.
    Applied(PublicationAmxSubmissionV1),
    /// The exact transaction is terminal and synchronized exact-release absence was proven.
    Terminal(PublicationReleaseSubmissionTerminalV1),
}

impl PublicationReleaseSubmissionOutcomeV1 {
    /// Retain finalized payload application evidence with the intent's local wire binding.
    #[must_use]
    pub fn applied(
        intent: &PublicationReleaseSubmissionIntentV1,
        submission: PublicationAmxSubmissionV1,
    ) -> Self {
        Self::Applied {
            submission,
            signed_transaction_digest: intent.signed_transaction_digest,
        }
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationReleaseSubmissionIntentV1,
    ) -> Result<(), PublicationError> {
        match self {
            Self::Applied {
                submission,
                signed_transaction_digest,
            } => {
                submission.validate_for(operation_id, &request.publish_instruction())?;
                if submission.transaction_hash != intent.transaction_hash
                    || *signed_transaction_digest != intent.signed_transaction_digest
                    || submission.applied_height
                        <= intent
                            .preparation
                            .replication
                            .finalized_page
                            .snapshot
                            .finalized_height
                {
                    return Err(PublicationError::InvalidEvidence {
                        phase: PublicationPhaseV1::ReleaseSubmission,
                        reason: "applied release outcome names a different compact intent"
                            .to_owned(),
                    });
                }
                Ok(())
            }
            Self::Terminal(terminal) => terminal.validate_for(request, intent),
        }
    }
}

/// One bounded append-only release transaction generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationReleaseSubmissionAttemptV1 {
    /// One-based contiguous generation number.
    pub generation: u8,
    /// Exact compact signed transaction and the storage floor that authorized it.
    pub intent: PublicationReleaseSubmissionIntentV1,
    /// Applied or terminal evidence appended before any later journal transition.
    pub outcome: Option<PublicationReleaseSubmissionOutcomeV1>,
}

impl PublicationReleaseSubmissionAttemptV1 {
    /// Construct a live release generation that has not acquired terminal evidence.
    #[must_use]
    pub fn new(generation: u8, intent: PublicationReleaseSubmissionIntentV1) -> Self {
        Self {
            generation,
            intent,
            outcome: None,
        }
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_ATTEMPT_CANONICAL_BYTES,
            "release submission attempt",
            PublicationPhaseV1::ReleaseSubmission,
        )?;
        self.intent.validate_for(operation_id, request)?;
        if let Some(outcome) = &self.outcome {
            ensure_release_component_budget(
                outcome,
                MAX_RELEASE_OUTCOME_CANONICAL_BYTES,
                "release submission outcome",
                PublicationPhaseV1::ReleaseSubmission,
            )?;
            outcome.validate_for(operation_id, request, &self.intent)?;
        }
        Ok(())
    }
}

/// Return the signature-bound validity deadline for one release intent.
pub(crate) fn release_submission_valid_until_ms(
    intent: &PublicationReleaseSubmissionIntentV1,
) -> Option<u64> {
    intent
        .envelope
        .creation_time_ms
        .checked_add(intent.envelope.time_to_live_ms.get())
}

fn validate_release_signed_transaction_shape(
    request: &PublicationRequestV1,
    signed_transaction: &SignedTransaction,
) -> Result<(), PublicationError> {
    let expected_instruction = InstructionBox::from(request.publish_instruction());
    let exact_instruction = matches!(
        signed_transaction.instructions(),
        Executable::Instructions(instructions)
            if instructions.len() == 1
                && instructions.iter().next() == Some(&expected_instruction)
    );
    let payload = signed_transaction.payload();
    let lifetime_is_valid = payload.time_to_live_ms.is_some_and(|time_to_live_ms| {
        payload.creation_time_ms > 0
            && payload
                .creation_time_ms
                .checked_add(time_to_live_ms.get())
                .is_some()
    });
    let signature_count = signed_transaction.signature_count();
    let multisig_is_canonical = signed_transaction
        .multisig_signatures()
        .is_none_or(|signatures| signatures.validate_canonical().is_ok());
    if signed_transaction.network_id().copied() != Some(request.network_id())
        || signed_transaction.authority() != &request.publisher
        || !exact_instruction
        || !lifetime_is_valid
        || !signed_transaction.metadata().is_empty()
        || signed_transaction.attachments().is_some()
        || signed_transaction.fee_payment_intent().validate().is_err()
        || signature_count == 0
        || signature_count > MUSUBI_MAX_RELEASE_SIGNATURES_V1
        || !multisig_is_canonical
        || signed_transaction.verify_signature().is_err()
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ReleaseSubmission,
            reason: "release transaction was not the exact bounded canonical publish shape"
                .to_owned(),
        });
    }
    Ok(())
}

fn release_signed_transaction_wire_v1(
    signed_transaction: &SignedTransaction,
) -> Result<Vec<u8>, PublicationError> {
    signed_transaction
        .encode_wire_v1()
        .map_err(|error| PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ReleaseSubmission,
            reason: format!("release transaction V1 wire encoding failed: {error}"),
        })
}

/// Idempotent Native AMX submission and authoritative application evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationAmxSubmissionV1 {
    /// Operation identifier passed to the backend idempotency boundary.
    pub operation_id: PublicationOperationIdV1,
    /// Digest of the exact [`PublishMusubiReleaseV1`] instruction accepted by AMX.
    pub instruction_digest: [u8; 32],
    /// Submitted payload transaction hash.
    pub transaction_hash: [u8; 32],
    /// Authoritative block height at which the payload transaction was applied.
    pub applied_height: u64,
}

impl PublicationAmxSubmissionV1 {
    /// Bind an applied payload hash and height to the exact idempotent publish instruction.
    #[must_use]
    pub fn new(
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
        transaction_hash: [u8; 32],
        applied_height: u64,
    ) -> Self {
        Self {
            operation_id,
            instruction_digest: domain_hash(PUBLISH_INSTRUCTION_DOMAIN, &instruction.encode()),
            transaction_hash,
            applied_height,
        }
    }

    fn validate_for(
        &self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_SUBMISSION_CANONICAL_BYTES,
            "release submission evidence",
            PublicationPhaseV1::ReleaseSubmission,
        )?;
        if self.operation_id != operation_id
            || self.instruction_digest
                != domain_hash(PUBLISH_INSTRUCTION_DOMAIN, &instruction.encode())
            || self.transaction_hash.iter().all(|byte| *byte == 0)
            || self.applied_height == 0
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
    fn validate_for(
        &self,
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_FINAL_QUERY_EVIDENCE_CANONICAL_BYTES,
            "release final evidence",
            PublicationPhaseV1::FinalVerification,
        )?;
        self.snapshot
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        self.home_release
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        self.universal_release
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        MusubiExactReleaseSnapshotV1 {
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_block_hash,
            snapshot: self.snapshot,
            home_release: self.home_release.clone(),
            universal_release: self.universal_release.clone(),
        }
        .validate()
        .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        let manifest = &request.publication.manifest;
        let row = &self.universal_release;
        if self.chain_id != request.chain_id
            || self.genesis_block_hash != request.genesis_block_hash
            || self.snapshot.finalized_height
                < request.publication.resolution.snapshot.finalized_height
            || self.snapshot.finalized_height < submission.applied_height
            || self.snapshot.index_revision < request.publication.resolution.snapshot.index_revision
            || &self.home_release.manifest != manifest
            || self.home_release.release_digest != manifest.release_digest()
            || self.home_release.published_by != request.publisher
            || self.home_release.published_at_height > self.snapshot.finalized_height
            || row.release != manifest.release
            || row.release_digest != manifest.release_digest()
            || row.archive_id != manifest.archive_id
            || row.source_digest != request.archive_commitment.source_tree_digest
            || row.interface_digest != manifest.interface_digest
            || row.abi != manifest.abi
            || row.dependencies != manifest.dependencies
            || row.index_revision > self.snapshot.index_revision
            || row.selection.storage.index_revision > row.index_revision
            || row.selection.storage.archive_id != manifest.archive_id
            || row.selection.storage.finalized_height > self.snapshot.finalized_height
            || (row.selection.storage.finalized_height == self.snapshot.finalized_height
                && row.selection.storage.finalized_block_hash != self.snapshot.finalized_block_hash)
            || row.selection.yank != self.home_release.yank
            || row.selection.governance != self.home_release.artifact_governance
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

/// Compact local journal commitment to one fully verified paired release projection.
///
/// The checkpoint binds the immutable request and canonical projection digests, but its public
/// self-digest is not an authenticated finalized-query receipt. Completed resume therefore relies
/// on the publication journal's trusted private-storage boundary.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PublicationFinalCheckpointV1 {
    /// Stable request-derived operation identity, including the public anti-replay nonce.
    pub operation_id: PublicationOperationIdV1,
    /// Deployment-selected chain identity returned with the exact projections.
    pub chain_id: ChainId,
    /// Exact genesis block hash returned with the exact projections.
    pub genesis_block_hash: [u8; 32],
    /// Finalized registry snapshot at which both projections were verified.
    pub snapshot: MusubiRegistrySnapshotV1,
    /// Exact structural release identity.
    pub release: MusubiReleaseIdV1,
    /// Digest of the complete immutable release manifest, including its archive identity.
    pub release_digest: MusubiReleaseDigestV1,
    /// Exact immutable source archive identity.
    pub archive_id: ArchiveId,
    /// Domain-separated canonical digest of the verified home-dataspace record.
    pub home_release_digest: [u8; 32],
    /// Domain-separated canonical digest of the verified universal resolver row.
    pub universal_release_digest: [u8; 32],
    /// Integrity digest binding every compact checkpoint field and projection digest.
    pub checkpoint_digest: [u8; 32],
}

impl PublicationFinalCheckpointV1 {
    fn from_verified(
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
        evidence: &PublicationFinalEvidenceV1,
    ) -> Result<Self, PublicationError> {
        evidence.validate_for(request, submission)?;
        let home_release = norito::encode_canonical(&evidence.home_release).map_err(|error| {
            PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::FinalVerification,
                reason: format!("home release could not be canonically encoded: {error}"),
            }
        })?;
        let universal_release =
            norito::encode_canonical(&evidence.universal_release).map_err(|error| {
                PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::FinalVerification,
                    reason: format!(
                        "universal release row could not be canonically encoded: {error}"
                    ),
                }
            })?;
        let mut checkpoint = Self {
            operation_id: request.operation_id(),
            chain_id: evidence.chain_id.clone(),
            genesis_block_hash: evidence.genesis_block_hash,
            snapshot: evidence.snapshot,
            release: evidence.home_release.manifest.release.clone(),
            release_digest: evidence.home_release.release_digest,
            archive_id: evidence.universal_release.archive_id,
            home_release_digest: domain_hash(FINAL_HOME_RELEASE_DOMAIN, &home_release),
            universal_release_digest: domain_hash(
                FINAL_UNIVERSAL_RELEASE_DOMAIN,
                &universal_release,
            ),
            checkpoint_digest: [0; 32],
        };
        checkpoint.checkpoint_digest = checkpoint.digest()?;
        checkpoint.validate_for(request, submission)?;
        Ok(checkpoint)
    }

    fn digest(&self) -> Result<[u8; 32], PublicationError> {
        let mut payload = self.clone();
        payload.checkpoint_digest = [0; 32];
        let canonical = norito::encode_canonical(&payload).map_err(|error| {
            PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::FinalVerification,
                reason: format!("final checkpoint could not be canonically encoded: {error}"),
            }
        })?;
        Ok(domain_hash(FINAL_CHECKPOINT_DOMAIN, &canonical))
    }

    fn validate_for(
        &self,
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
    ) -> Result<(), PublicationError> {
        ensure_release_component_budget(
            self,
            MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES,
            "release final checkpoint",
            PublicationPhaseV1::FinalVerification,
        )?;
        self.snapshot
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        self.release
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::FinalVerification, error))?;
        let manifest = &request.publication.manifest;
        if self.operation_id != request.operation_id()
            || self.operation_id != submission.operation_id
            || self.chain_id != request.chain_id
            || self.genesis_block_hash != request.genesis_block_hash
            || self.snapshot.finalized_height
                < request.publication.resolution.snapshot.finalized_height
            || self.snapshot.finalized_height < submission.applied_height
            || self.snapshot.index_revision < request.publication.resolution.snapshot.index_revision
            || self.release != manifest.release
            || self.release_digest != manifest.release_digest()
            || self.archive_id != manifest.archive_id
            || self.home_release_digest.iter().all(|byte| *byte == 0)
            || self.universal_release_digest.iter().all(|byte| *byte == 0)
            || self.checkpoint_digest.iter().all(|byte| *byte == 0)
            || self.checkpoint_digest != self.digest()?
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::FinalVerification,
                reason: "compact final publication checkpoint was substituted".to_owned(),
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
    /// Compact local commitment to the verified exact registry projections.
    pub final_checkpoint: PublicationFinalCheckpointV1,
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
    /// Bounded append-only exact registration transaction generations.
    pub archive_registration_attempts: Vec<PublicationArchiveRegistrationAttemptV1>,
    /// Finalized authoritative archive record persisted before storage coordination.
    pub registered_archive: Option<PublicationRegisteredArchiveV1>,
    /// Compact append-only anchors for immutable provider set and transaction sidecars.
    pub provider_registration_checkpoints: Vec<PublicationProviderRegistrationCheckpointV1>,
    /// Bounded append-only signed archive-location transaction generations.
    pub archive_location_attempts: Vec<PublicationArchiveLocationAttemptV1>,
    /// Complete finalized page containing the healthy active location and compact proof-set digest.
    pub replication: Option<PublicationReplicationCheckpointV1>,
    /// Two distinct provider readback results.
    pub readbacks: Vec<PublicationReadbackEvidenceV1>,
    /// Bounded append-only compact release transaction generations.
    pub release_submission_attempts: Vec<PublicationReleaseSubmissionAttemptV1>,
    /// Idempotent Native AMX submission result.
    pub submission: Option<PublicationAmxSubmissionV1>,
    /// Present only after exact finalized home/index verification and compaction.
    pub completion: Option<PublicationFinalCheckpointV1>,
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
            archive_registration_attempts: Vec::new(),
            registered_archive: None,
            provider_registration_checkpoints: Vec::new(),
            archive_location_attempts: Vec::new(),
            replication: None,
            readbacks: Vec::new(),
            release_submission_attempts: Vec::new(),
            submission: None,
            completion: None,
        })
    }

    /// Validate schema, operation identity, exact evidence, and phase consistency.
    ///
    /// # Errors
    ///
    /// Returns an invalid-journal or invalid-evidence error when any persisted phase, append-only
    /// history, canonical budget, replay binding, or finalized checkpoint is inconsistent.
    #[allow(
        clippy::too_many_lines,
        reason = "the journal validator keeps the complete append-only security state machine and all phase invariants adjacent"
    )]
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
        self.validate_canonical_budgets()?;
        self.request.validate()?;
        if self.operation_id != self.request.operation_id() {
            return Err(PublicationError::InvalidJournal(
                "journal operation id does not bind its immutable request".to_owned(),
            ));
        }
        let required = self.phase as u8;
        validate_option(
            required >= PublicationPhaseV1::SeedIngress as u8,
            self.validation.as_ref(),
        )?;
        validate_option(
            required >= PublicationPhaseV1::ArchiveRegistration as u8,
            self.staging_receipt.as_ref(),
        )?;
        if self.archive_registration_attempts.len() > MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1 {
            return Err(PublicationError::InvalidJournal(
                "journal exceeds the archive-registration attempt bound".to_owned(),
            ));
        }
        for (index, attempt) in self.archive_registration_attempts.iter().enumerate() {
            let generation = u8::try_from(index + 1).expect("attempt bound fits u8");
            if attempt.generation != generation
                || (index + 1 < self.archive_registration_attempts.len()
                    && attempt.terminal.is_none())
            {
                return Err(PublicationError::InvalidJournal(
                    "archive-registration attempts are not contiguous and append-only".to_owned(),
                ));
            }
            attempt.validate_for(self.operation_id, &self.request)?;
        }
        let active_attempt = self
            .archive_registration_attempts
            .last()
            .filter(|attempt| attempt.terminal.is_none());
        let registration_complete = required >= PublicationPhaseV1::Replication as u8;
        if registration_complete {
            if active_attempt.is_none() {
                return Err(PublicationError::InvalidJournal(
                    "completed archive registration is missing its active exact attempt".to_owned(),
                ));
            }
            validate_option(true, self.registered_archive.as_ref())?;
        } else if required < PublicationPhaseV1::ArchiveRegistration as u8 {
            validate_option(false, self.registered_archive.as_ref())?;
            if !self.archive_location_attempts.is_empty() {
                return Err(PublicationError::InvalidJournal(
                    "archive-location attempts are present before archive finality".to_owned(),
                ));
            }
            if active_attempt.is_some()
                || (self.phase == PublicationPhaseV1::Validation
                    && !self.archive_registration_attempts.is_empty())
            {
                return Err(PublicationError::InvalidJournal(
                    "a live archive-registration attempt is present outside its phase".to_owned(),
                ));
            }
        } else if self.registered_archive.is_some() && active_attempt.is_none() {
            return Err(PublicationError::InvalidJournal(
                "archive registration checkpoints are not monotonic".to_owned(),
            ));
        }
        if self.provider_registration_checkpoints.len() > MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1
            || self.provider_registration_checkpoints.len()
                > self.archive_location_attempts.len().saturating_add(1)
            || (required < PublicationPhaseV1::ArchiveRegistration as u8
                && !self.provider_registration_checkpoints.is_empty())
        {
            return Err(PublicationError::InvalidJournal(
                "provider-registration checkpoint history is out of phase or over bound".to_owned(),
            ));
        }
        for (index, checkpoint) in self.provider_registration_checkpoints.iter().enumerate() {
            let generation = u8::try_from(index + 1).expect("location-attempt bound fits u8");
            checkpoint.validate_for(&self.request, generation)?;
            if let Some(attempt) = self.archive_location_attempts.get(index)
                && (attempt.intent.generation != checkpoint.generation
                    || attempt.intent.replication_order != checkpoint.replication_order
                    || attempt.intent.provider_attestation_set_digest
                        != checkpoint.provider_attestation_set_digest)
            {
                return Err(PublicationError::InvalidJournal(
                    "location intent does not bind its provider-registration checkpoint".to_owned(),
                ));
            }
        }
        if self.archive_location_attempts.len() > MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1 {
            return Err(PublicationError::InvalidJournal(
                "journal exceeds the archive-location attempt bound".to_owned(),
            ));
        }
        let mut prior_location_ids = Vec::with_capacity(self.archive_location_attempts.len());
        for (index, attempt) in self.archive_location_attempts.iter().enumerate() {
            let generation = u8::try_from(index + 1).expect("location-attempt bound fits u8");
            if attempt.generation != generation
                || attempt.intent.generation != generation
                || (index + 1 < self.archive_location_attempts.len() && attempt.terminal.is_none())
            {
                return Err(PublicationError::InvalidJournal(
                    "archive-location attempts are not contiguous and append-only".to_owned(),
                ));
            }
            let registered = self.registered_archive.as_ref().ok_or_else(|| {
                PublicationError::InvalidJournal(
                    "archive-location attempt is missing authoritative archive finality".to_owned(),
                )
            })?;
            attempt.intent.validate_for(
                self.operation_id,
                &self.request,
                registered,
                &prior_location_ids,
            )?;
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|previous| self.archive_location_attempts.get(previous))
            {
                let terminal = previous.terminal.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "a replacement archive location lacks prior terminal finality".to_owned(),
                    )
                })?;
                let prepared = &attempt.intent.prepared_page;
                let prior_finalized = &terminal.finalized_page;
                if prepared.snapshot.finalized_height < prior_finalized.snapshot.finalized_height
                    || prepared.snapshot.index_revision < prior_finalized.snapshot.index_revision
                    || (prepared.snapshot.finalized_height
                        == prior_finalized.snapshot.finalized_height
                        && prepared != prior_finalized)
                    || (prepared.snapshot == prior_finalized.snapshot
                        && (prepared.archive != prior_finalized.archive
                            || prepared.items != prior_finalized.items))
                    || prepared.archive.location_revision
                        < prior_finalized.archive.location_revision
                    || (prepared.archive.location_revision
                        == prior_finalized.archive.location_revision
                        && (prepared.archive != prior_finalized.archive
                            || prepared.items != prior_finalized.items))
                {
                    return Err(PublicationError::InvalidJournal(
                        "a replacement archive location regressed prior terminal finality"
                            .to_owned(),
                    ));
                }
            }
            if let Some(registration) = &attempt.registration {
                registration.validate_for(
                    self.operation_id,
                    &self.request,
                    registered,
                    &prior_location_ids,
                )?;
                if registration.intent != attempt.intent {
                    return Err(PublicationError::InvalidJournal(
                        "archive-location finality names a different signed intent".to_owned(),
                    ));
                }
            }
            if attempt.terminal.is_some() != attempt.terminal_floor.is_some() {
                return Err(PublicationError::InvalidJournal(
                    "archive-location terminal evidence is missing its durable floor".to_owned(),
                ));
            }
            if let (Some(terminal), Some(floor)) = (&attempt.terminal, &attempt.terminal_floor) {
                terminal.validate_for(
                    self.operation_id,
                    &self.request,
                    registered,
                    attempt,
                    &prior_location_ids,
                    floor,
                )?;
            }
            prior_location_ids.push(attempt.intent.location_id);
        }
        let active_location_attempt = self
            .archive_location_attempts
            .last()
            .filter(|attempt| attempt.terminal.is_none());
        if registration_complete
            && active_location_attempt
                .and_then(|attempt| attempt.registration.as_ref())
                .is_none()
        {
            return Err(PublicationError::InvalidJournal(
                "replication phase is missing a finalized active location generation".to_owned(),
            ));
        }
        validate_option(
            required >= PublicationPhaseV1::Readback as u8,
            self.replication.as_ref(),
        )?;
        let expects_readbacks = required >= PublicationPhaseV1::ReleaseSubmission as u8;
        if self.readbacks.len() != if expects_readbacks { 2 } else { 0 } {
            return Err(PublicationError::InvalidJournal(
                "journal readback count is inconsistent with its phase".to_owned(),
            ));
        }
        if self.release_submission_attempts.len() > MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1 {
            return Err(PublicationError::InvalidJournal(
                "journal exceeds the release-submission attempt bound".to_owned(),
            ));
        }
        if required < PublicationPhaseV1::ArchiveRegistration as u8
            && !self.release_submission_attempts.is_empty()
        {
            return Err(PublicationError::InvalidJournal(
                "release-submission attempts are present before archive registration".to_owned(),
            ));
        }
        let mut prior_release_hashes = Vec::with_capacity(self.release_submission_attempts.len());
        let mut prior_release_digests = Vec::with_capacity(self.release_submission_attempts.len());
        for (index, attempt) in self.release_submission_attempts.iter().enumerate() {
            let generation = u8::try_from(index + 1).expect("release-attempt bound fits u8");
            if attempt.generation != generation
                || (index + 1 < self.release_submission_attempts.len()
                    && !matches!(
                        &attempt.outcome,
                        Some(PublicationReleaseSubmissionOutcomeV1::Terminal(_))
                    ))
                || prior_release_hashes.contains(&attempt.intent.transaction_hash)
                || prior_release_digests.contains(&attempt.intent.signed_transaction_digest)
            {
                return Err(PublicationError::InvalidJournal(
                    "release-submission attempts are not unique, contiguous, and append-only"
                        .to_owned(),
                ));
            }
            attempt.validate_for(self.operation_id, &self.request)?;
            let location_attempt = attempt
                .intent
                .preparation
                .location_generation
                .checked_sub(1)
                .map(usize::from)
                .and_then(|index| self.archive_location_attempts.get(index))
                .filter(|location_attempt| {
                    location_attempt.generation == attempt.intent.preparation.location_generation
                })
                .ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "release intent is missing its journaled archive-location generation"
                            .to_owned(),
                    )
                })?;
            let location_registration =
                location_attempt.registration.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "release intent location generation lacks finalized registration"
                            .to_owned(),
                    )
                })?;
            attempt
                .intent
                .preparation
                .validate_for_registration(&self.request, location_registration)?;
            if let Some((previous, previous_terminal)) = index
                .checked_sub(1)
                .and_then(|previous| self.release_submission_attempts.get(previous))
                .and_then(|previous| match &previous.outcome {
                    Some(PublicationReleaseSubmissionOutcomeV1::Terminal(terminal)) => {
                        Some((previous, terminal))
                    }
                    _ => None,
                })
            {
                let prior_snapshot = previous_terminal.absence().snapshot();
                let next_snapshot = &attempt
                    .intent
                    .preparation
                    .replication
                    .finalized_page
                    .snapshot;
                if next_snapshot.finalized_height < prior_snapshot.finalized_height
                    || next_snapshot.index_revision < prior_snapshot.index_revision
                    || (next_snapshot.finalized_height == prior_snapshot.finalized_height
                        && next_snapshot.finalized_block_hash
                            != prior_snapshot.finalized_block_hash)
                {
                    return Err(PublicationError::InvalidJournal(
                        "successor release intent does not cover prior terminal finality"
                            .to_owned(),
                    ));
                }
                if let PublicationReleaseSubmissionTerminalReasonV1::RegistryRejected {
                    block_height,
                    ..
                } = &previous_terminal.reason
                {
                    let prior_location = previous
                        .intent
                        .preparation
                        .location()
                        .expect("validated release preparation contains its location");
                    let next_location = attempt
                        .intent
                        .preparation
                        .location()
                        .expect("validated release preparation contains its location");
                    let higher_same_location = attempt.intent.preparation.location_generation
                        == previous.intent.preparation.location_generation
                        && next_location.location_id == prior_location.location_id
                        && next_location.revision > prior_location.revision
                        && next_location.finalized_height >= *block_height;
                    let prior_location_terminal = previous
                        .intent
                        .preparation
                        .location_generation
                        .checked_sub(1)
                        .map(usize::from)
                        .and_then(|index| self.archive_location_attempts.get(index))
                        .and_then(|attempt| attempt.terminal.as_ref());
                    let retired_then_replaced = attempt.intent.preparation.location_generation
                        > previous.intent.preparation.location_generation
                        && prior_location_terminal.is_some_and(|terminal| {
                            matches!(
                                terminal.reason,
                                PublicationArchiveLocationTerminalReasonV1::Retired
                            ) && terminal.finalized_page.snapshot.finalized_height >= *block_height
                        });
                    if !higher_same_location && !retired_then_replaced {
                        return Err(PublicationError::InvalidJournal(
                            "rejected release successor did not refresh or replace its location"
                                .to_owned(),
                        ));
                    }
                }
            }
            match &attempt.outcome {
                None => {
                    if self.phase != PublicationPhaseV1::ReleaseSubmission
                        || index + 1 != self.release_submission_attempts.len()
                        || location_attempt.terminal.is_some()
                        || self.replication.as_ref()
                            != Some(&attempt.intent.preparation.replication)
                        || self.readbacks != attempt.intent.preparation.readbacks
                    {
                        return Err(PublicationError::InvalidJournal(
                            "live release intent is outside its exact current preparation floor"
                                .to_owned(),
                        ));
                    }
                }
                Some(PublicationReleaseSubmissionOutcomeV1::Applied { .. }) => {
                    if self.phase != PublicationPhaseV1::FinalVerification
                        || index + 1 != self.release_submission_attempts.len()
                    {
                        return Err(PublicationError::InvalidJournal(
                            "applied release outcome is outside final verification".to_owned(),
                        ));
                    }
                }
                Some(PublicationReleaseSubmissionOutcomeV1::Terminal(_)) => {}
            }
            prior_release_hashes.push(attempt.intent.transaction_hash);
            prior_release_digests.push(attempt.intent.signed_transaction_digest);
        }
        if required >= PublicationPhaseV1::ReleaseSubmission as u8
            && self.release_submission_attempts.is_empty()
        {
            return Err(PublicationError::InvalidJournal(
                "release submission is missing its persist-before-send exact intent".to_owned(),
            ));
        }
        if self.phase == PublicationPhaseV1::FinalVerification
            && !matches!(
                self.release_submission_attempts
                    .last()
                    .and_then(|attempt| attempt.outcome.as_ref()),
                Some(PublicationReleaseSubmissionOutcomeV1::Applied { .. })
            )
        {
            return Err(PublicationError::InvalidJournal(
                "final verification is missing the exact applied release outcome".to_owned(),
            ));
        }
        validate_option(
            required >= PublicationPhaseV1::FinalVerification as u8,
            self.submission.as_ref(),
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
        if let Some(attempt) = active_attempt
            && self.staging_receipt.as_ref() != Some(&attempt.intent.staging_receipt)
        {
            return Err(PublicationError::InvalidJournal(
                "active archive-registration attempt is missing its exact staging receipt"
                    .to_owned(),
            ));
        }
        if let Some(registered) = &self.registered_archive {
            registered.validate_for(
                &self.request,
                active_attempt
                    .map(|attempt| &attempt.intent)
                    .ok_or_else(|| {
                        PublicationError::InvalidJournal(
                            "authoritative archive is missing its registration intent".to_owned(),
                        )
                    })?,
            )?;
        }
        if let Some(checkpoint) = &self.replication {
            checkpoint.validate_for(&self.request, self.registration()?)?;
        }
        if let Some(checkpoint) = &self.replication {
            let location = checkpoint.location(self.registration()?)?;
            if !self.readbacks.is_empty() {
                validate_readback_subset(&self.request, location, &self.readbacks)?;
            }
        }
        if let Some(submission) = &self.submission {
            submission.validate_for(self.operation_id, &self.request.publish_instruction())?;
            if let Some(last_attempt) = self.release_submission_attempts.last() {
                let applied_submission = match &last_attempt.outcome {
                    Some(PublicationReleaseSubmissionOutcomeV1::Applied { submission, .. }) => {
                        Some(submission)
                    }
                    _ => None,
                };
                if applied_submission != Some(submission) {
                    return Err(PublicationError::InvalidJournal(
                        "Native AMX submission does not equal the append-only applied outcome"
                            .to_owned(),
                    ));
                }
            }
        }
        if let Some(completion) = &self.completion {
            completion.validate_for(
                &self.request,
                self.submission.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "final checkpoint is missing its Native AMX submission".to_owned(),
                    )
                })?,
            )?;
        }
        Ok(())
    }

    fn validate_canonical_budgets(&self) -> Result<(), PublicationError> {
        let full_size = canonical_encoded_len(self).map_err(PublicationError::InvalidJournal)?;
        if full_size > MAX_JOURNAL_BYTES_USIZE {
            return Err(PublicationError::InvalidJournal(format!(
                "journal canonical size {full_size} exceeds {MAX_JOURNAL_BYTES_USIZE} bytes"
            )));
        }

        let mut non_release = self.clone();
        non_release.release_submission_attempts.clear();
        non_release.submission = None;
        non_release.completion = None;
        let non_release_size =
            canonical_encoded_len(&non_release).map_err(PublicationError::InvalidJournal)?;
        if non_release_size > JOURNAL_NON_RELEASE_BUDGET_BYTES {
            return Err(PublicationError::InvalidJournal(format!(
                "non-release journal state canonical size {non_release_size} exceeds its derived \
                 {JOURNAL_NON_RELEASE_BUDGET_BYTES}-byte budget"
            )));
        }

        let mut release_attempts_size = 0_usize;
        for attempt in &self.release_submission_attempts {
            let attempt_size =
                canonical_encoded_len(attempt).map_err(PublicationError::InvalidJournal)?;
            let intent_size =
                canonical_encoded_len(&attempt.intent).map_err(PublicationError::InvalidJournal)?;
            let outcome_size = attempt
                .outcome
                .as_ref()
                .map(canonical_encoded_len)
                .transpose()
                .map_err(PublicationError::InvalidJournal)?
                .unwrap_or_default();
            if attempt_size > MAX_RELEASE_ATTEMPT_CANONICAL_BYTES
                || intent_size > MAX_RELEASE_INTENT_CANONICAL_BYTES
                || outcome_size > MAX_RELEASE_OUTCOME_CANONICAL_BYTES
            {
                return Err(PublicationError::InvalidJournal(
                    "release attempt exceeds its multiplicity-derived canonical budget".to_owned(),
                ));
            }
            release_attempts_size =
                release_attempts_size
                    .checked_add(attempt_size)
                    .ok_or_else(|| {
                        PublicationError::InvalidJournal(
                            "release history canonical size overflowed".to_owned(),
                        )
                    })?;
        }
        if release_attempts_size > JOURNAL_RELEASE_ATTEMPTS_BUDGET_BYTES {
            return Err(PublicationError::InvalidJournal(format!(
                "release attempts canonical size {release_attempts_size} exceeds their derived \
                 {JOURNAL_RELEASE_ATTEMPTS_BUDGET_BYTES}-byte budget"
            )));
        }

        let submission_size = self
            .submission
            .as_ref()
            .map(canonical_encoded_len)
            .transpose()
            .map_err(PublicationError::InvalidJournal)?;
        let completion_size = self
            .completion
            .as_ref()
            .map(canonical_encoded_len)
            .transpose()
            .map_err(PublicationError::InvalidJournal)?;
        if submission_size.is_some_and(|size| size > MAX_RELEASE_SUBMISSION_CANONICAL_BYTES)
            || completion_size
                .is_some_and(|size| size > MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES)
        {
            return Err(PublicationError::InvalidJournal(
                "release submission or final checkpoint exceeds its reserved canonical budget"
                    .to_owned(),
            ));
        }
        let mut final_state_size = 0_usize;
        for component_size in [submission_size, completion_size].into_iter().flatten() {
            final_state_size = final_state_size
                .checked_add(component_size)
                .ok_or_else(|| {
                    PublicationError::InvalidJournal(
                        "release history canonical size overflowed".to_owned(),
                    )
                })?;
        }
        if final_state_size > JOURNAL_RELEASE_FINAL_STATE_BUDGET_BYTES {
            return Err(PublicationError::InvalidJournal(format!(
                "release final state canonical size {final_state_size} exceeds its reserved \
                 {JOURNAL_RELEASE_FINAL_STATE_BUDGET_BYTES}-byte budget"
            )));
        }
        Ok(())
    }

    fn registration(&self) -> Result<&PublicationArchiveRegistrationV1, PublicationError> {
        self.archive_location_attempts
            .last()
            .filter(|attempt| attempt.terminal.is_none())
            .and_then(|attempt| attempt.registration.as_ref())
            .ok_or_else(|| {
                PublicationError::InvalidJournal(
                    "journal is missing an active finalized archive location".to_owned(),
                )
            })
    }

    /// Convert a completed journal into its stable publication result.
    pub fn result(&self) -> Option<PublicationResultV1> {
        Some(PublicationResultV1 {
            operation_id: self.operation_id,
            submission: self.submission?,
            final_checkpoint: self.completion.clone()?,
        })
    }
}

fn validate_option<T>(required: bool, value: Option<&T>) -> Result<(), PublicationError> {
    if required != value.is_some() {
        return Err(PublicationError::InvalidJournal(
            "journal evidence presence is inconsistent with its phase".to_owned(),
        ));
    }
    Ok(())
}

fn archive_registration_attempts_are_append_only(
    previous: &[PublicationArchiveRegistrationAttemptV1],
    next: &[PublicationArchiveRegistrationAttemptV1],
) -> bool {
    if previous.len() > next.len() || next.len() > previous.len().saturating_add(1) {
        return false;
    }
    if previous == next.get(..previous.len()).unwrap_or_default() {
        return true;
    }
    if previous.len() != next.len() || previous.is_empty() {
        return false;
    }
    let last = previous.len() - 1;
    previous[..last] == next[..last]
        && previous[last].generation == next[last].generation
        && previous[last].intent == next[last].intent
        && previous[last].terminal.is_none()
        && next[last].terminal.is_some()
}

fn provider_registration_checkpoints_are_append_only(
    previous: &[PublicationProviderRegistrationCheckpointV1],
    next: &[PublicationProviderRegistrationCheckpointV1],
) -> bool {
    if previous == next {
        return true;
    }
    if next.len() == previous.len().saturating_add(1) {
        return previous == &next[..previous.len()]
            && next.last().is_some_and(|checkpoint| {
                usize::from(checkpoint.generation) == next.len()
                    && checkpoint.transactions.is_empty()
            });
    }
    if previous.len() != next.len() || previous.is_empty() {
        return false;
    }
    let last = previous.len() - 1;
    let before = &previous[last];
    let after = &next[last];
    previous[..last] == next[..last]
        && before.generation == after.generation
        && before.archive_id == after.archive_id
        && before.replication_order == after.replication_order
        && before.provider_attestation_set_digest == after.provider_attestation_set_digest
        && before.set_sidecar_hash == after.set_sidecar_hash
        && after.transactions.len() == before.transactions.len().saturating_add(1)
        && before.transactions == after.transactions[..before.transactions.len()]
}

fn archive_location_attempts_are_append_only(
    previous: &[PublicationArchiveLocationAttemptV1],
    next: &[PublicationArchiveLocationAttemptV1],
) -> bool {
    if previous.len() > next.len() || next.len() > previous.len().saturating_add(1) {
        return false;
    }
    if previous == next.get(..previous.len()).unwrap_or_default() {
        return true;
    }
    if previous.len() != next.len() || previous.is_empty() {
        return false;
    }
    let last = previous.len() - 1;
    if previous[..last] != next[..last]
        || previous[last].generation != next[last].generation
        || previous[last].intent != next[last].intent
    {
        return false;
    }
    let before = &previous[last];
    let after = &next[last];
    let registration_was_absent = before.registration.is_none();
    let registration_is_present = after.registration.is_some();
    let registration_appended = registration_was_absent
        && registration_is_present
        && before.terminal == after.terminal
        && before.terminal_floor == after.terminal_floor;
    let terminal_was_absent = before.terminal.is_none();
    let terminal_is_present = after.terminal.is_some();
    let terminal_appended = before.registration == after.registration
        && terminal_was_absent
        && terminal_is_present
        && before.terminal_floor.is_none()
        && after.terminal_floor.is_some();
    registration_appended || terminal_appended
}

fn release_submission_attempts_are_append_only(
    previous: &[PublicationReleaseSubmissionAttemptV1],
    next: &[PublicationReleaseSubmissionAttemptV1],
) -> bool {
    if previous.len() > next.len() || next.len() > previous.len().saturating_add(1) {
        return false;
    }
    if next.len() == previous.len().saturating_add(1) {
        let Some(appended) = next.last() else {
            return false;
        };
        return previous == &next[..previous.len()]
            && appended.outcome.is_none()
            && usize::from(appended.generation) == next.len()
            && previous.last().is_none_or(|attempt| {
                matches!(
                    attempt.outcome,
                    Some(PublicationReleaseSubmissionOutcomeV1::Terminal(_))
                )
            });
    }
    if previous == next {
        return true;
    }
    if previous.is_empty() {
        return false;
    }
    let last = previous.len() - 1;
    previous[..last] == next[..last]
        && previous[last].generation == next[last].generation
        && previous[last].intent == next[last].intent
        && previous[last].outcome.is_none()
        && next[last].outcome.is_some()
}

/// Runtime-only source of exact CAR bytes and their canonical build-plan witness.
pub trait PublicationCarSource {
    /// Open a new reader at byte zero. Implementations must not persist their path or credentials.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the exact bounded CAR cannot be opened safely at byte zero.
    fn open_car(&self) -> io::Result<Box<dyn Read + '_>>;

    /// Return the exact validated seed-ingress wire plan for this CAR.
    ///
    /// The plan is runtime-only publication input. Implementations must not put its bytes or a
    /// filesystem path into the secret-free publication journal.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the immutable plan cannot be reopened, decoded canonically, or
    /// validated against the complete archive commitment.
    fn car_plan(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> io::Result<MusubiSeedIngressCarPlanV1>;
}

/// Deterministic operation-local CAR and plan stored beside, but never inside, the journal.
#[derive(Clone, Debug)]
pub struct PublicationStagedCarSourceV1 {
    root: PathBuf,
    path: PathBuf,
    plan_path: PathBuf,
    expected_size: u64,
}

impl PublicationStagedCarSourceV1 {
    /// Bind immutable CAR and plan locations for one operation below an explicit state root.
    #[must_use]
    pub fn new(
        user_state_root: &Path,
        operation_id: PublicationOperationIdV1,
        expected_size: u64,
    ) -> Self {
        Self {
            root: user_state_root.to_path_buf(),
            path: user_state_root.join(staged_car_relative_path(operation_id)),
            plan_path: user_state_root.join(staged_plan_relative_path(operation_id)),
            expected_size,
        }
    }

    /// Return the deterministic operation-local path without persisting it in the journal.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the deterministic immutable plan-sidecar path outside the journal.
    #[must_use]
    pub fn plan_path(&self) -> &Path {
        &self.plan_path
    }

    /// Durably stage exact CAR bytes and their canonical plan below the private operation directory.
    ///
    /// Identical retries reuse both verified regular files. A different body or plan at the same
    /// operation id, an unsafe filesystem entry, or bytes that disagree with the request
    /// commitment fail closed without replacing either existing path.
    ///
    /// # Errors
    ///
    /// Returns invalid evidence, invalid journal state, or an I/O error when the CAR or plan is
    /// unbounded, differs from its commitment, or cannot be staged and revalidated safely.
    pub fn stage_bytes(
        user_state_root: &Path,
        operation_id: PublicationOperationIdV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        bytes: &[u8],
    ) -> Result<Self, PublicationError> {
        commitment
            .validate()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        let wire_plan = MusubiSeedIngressCarPlanV1::from_car_build_plan(plan, commitment)
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        let canonical_digest = wire_plan
            .canonical_digest()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        let plan_bytes = wire_plan
            .canonical_bytes()
            .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
        if commitment.car_size == 0
            || commitment.car_size > MUSUBI_MAX_CAR_BYTES_V1
            || u64::try_from(bytes.len()).ok() != Some(commitment.car_size)
            || blake3::hash(bytes).as_bytes() != commitment.car_digest.as_bytes()
            || plan_bytes.is_empty()
            || plan_bytes.len() > MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1
            || canonical_digest.is_zero()
        {
            return Err(invalid_staged_car_plan());
        }
        if !staged_car_plan_matches_commitment(commitment, plan, bytes) {
            return Err(invalid_staged_car_plan());
        }
        let source = Self::new(user_state_root, operation_id, commitment.car_size);
        let root = AtomicWriteRoot::new(user_state_root).map_err(PublicationError::JournalWrite)?;
        root.install_immutable(&staged_plan_relative_path(operation_id), &plan_bytes)
            .map_err(PublicationError::JournalWrite)?;
        root.install_immutable(&staged_car_relative_path(operation_id), bytes)
            .map_err(PublicationError::JournalWrite)?;
        source.verify_digest(commitment.car_digest)?;
        let reopened = source
            .car_plan(commitment)
            .map_err(PublicationError::CarSource)?;
        if reopened != wire_plan {
            return Err(PublicationError::InvalidJournal(
                "existing staged plan differs from the immutable publication request".to_owned(),
            ));
        }
        Ok(source)
    }

    fn reopen_plan(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> io::Result<MusubiSeedIngressCarPlanV1> {
        let root = AtomicWriteRoot::new(&self.root)
            .map_err(|_| invalid_plan_source("staged publication plan root is unsafe"))?;
        let bytes = root
            .load_immutable(
                self.plan_path
                    .strip_prefix(&self.root)
                    .map_err(|_| invalid_plan_source("staged publication plan escaped its root"))?,
                MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1,
            )
            .map_err(|_| invalid_plan_source("staged publication plan could not be opened safely"))?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "staged publication plan sidecar is missing",
                )
            })?;
        let plan: MusubiSeedIngressCarPlanV1 =
            norito::decode_canonical_with_limits(&bytes, STAGED_PLAN_DECODE_LIMITS).map_err(
                |_| invalid_plan_source("staged publication plan is not canonical Norito"),
            )?;
        let canonical_digest = plan
            .canonical_digest()
            .map_err(|_| invalid_plan_source("staged publication plan digest is invalid"))?;
        let canonical = plan.canonical_bytes().map_err(|_| {
            invalid_plan_source("staged publication plan cannot be encoded canonically")
        })?;
        if canonical != bytes || canonical_digest.is_zero() {
            return Err(invalid_plan_source(
                "staged publication plan has a noncanonical representation",
            ));
        }
        plan.validate(commitment).map_err(|_| {
            invalid_plan_source("staged publication plan differs from the commitment")
        })?;
        let car_plan = plan
            .to_car_build_plan(commitment)
            .map_err(|_| invalid_plan_source("staged publication plan cannot be reconstructed"))?;
        let car = self.load_car_bytes_for_plan()?;
        if !staged_car_plan_matches_commitment(commitment, &car_plan, &car) {
            return Err(invalid_plan_source(
                "staged publication CAR and plan do not reproduce their archive commitment",
            ));
        }
        Ok(plan)
    }

    fn load_car_bytes_for_plan(&self) -> io::Result<Vec<u8>> {
        let size = usize::try_from(self.expected_size).map_err(|_| {
            invalid_plan_source("staged publication CAR length does not fit this platform")
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|_| {
            invalid_plan_source("staged publication CAR buffer could not be allocated")
        })?;
        bytes.resize(size, 0);
        let mut reader = self.open_car()?;
        reader.read_exact(&mut bytes)?;
        let mut trailing = [0_u8; 1];
        if reader.read(&mut trailing)? != 0 {
            return Err(invalid_plan_source(
                "staged publication CAR exceeds its committed length",
            ));
        }
        Ok(bytes)
    }

    fn verify_digest(
        &self,
        expected_digest: MusubiContentDigestV1,
    ) -> Result<(), PublicationError> {
        let mut reader = self.open_car().map_err(PublicationError::CarSource)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 64 * 1024];
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

fn invalid_staged_car_plan() -> PublicationError {
    PublicationError::InvalidEvidence {
        phase: PublicationPhaseV1::Validation,
        reason: "staged CAR and plan do not reproduce their bounded archive commitment".to_owned(),
    }
}

fn staged_car_plan_matches_commitment(
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
    bytes: &[u8],
) -> bool {
    let Ok(verified) = CarVerifier::verify_canonical_car_with_plan_retained(plan, bytes) else {
        return false;
    };
    let stats = verified.stats();
    let observed_car_digest = stats.car_archive_digest.as_bytes();
    let expected_car_digest = commitment.car_digest.as_bytes();
    let observed_content_length = stats.payload_bytes;
    let expected_content_length = commitment.content_length;
    let observed_root_cid = stats.root_cids.first().map(Vec::as_slice);
    let expected_root_cid: &[u8] = commitment.root_cid.as_bytes();
    if stats.car_size != commitment.car_size
        || observed_car_digest != expected_car_digest
        || observed_content_length != expected_content_length
        || stats.chunk_count != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
        || stats.chunk_profile != plan.chunk_profile
        || stats.root_cids.len() != 1
        || observed_root_cid != Some(expected_root_cid)
    {
        return false;
    }
    let Ok(mut chunk_store) = ChunkStore::with_profile_and_heap_limit(
        plan.chunk_profile,
        STAGED_CAR_PLAN_HEAP_LIMIT_BYTES_V1,
    ) else {
        return false;
    };
    let mut payload_reader = verified.payload_reader();
    if chunk_store
        .ingest_plan_stream(plan, &mut payload_reader)
        .is_err()
    {
        return false;
    }
    chunk_store.payload_digest() == &plan.payload_digest
        && chunk_store.por_tree().root() == commitment.por_root.as_bytes()
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
        set_no_follow_nonblocking(&mut options);
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

    fn car_plan(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> io::Result<MusubiSeedIngressCarPlanV1> {
        self.reopen_plan(commitment)
    }
}

fn invalid_plan_source(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
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
    ///
    /// # Errors
    ///
    /// Returns a backend error when the trusted publication clock cannot be sampled.
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError>;

    /// Validate and compiler-check the clean CAR and exact dependency graph.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the exact CAR cannot be read, validated, or compiler-checked.
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError>;

    /// Stage the CAR only through an admitted, authenticated seed-ingress service.
    ///
    /// # Errors
    ///
    /// Returns a backend error when authenticated staging or receipt verification cannot complete.
    fn stage_authenticated_seed_ingress(
        &mut self,
        operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
        car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError>;

    /// Prebuild, fee-quote, and sign the exact archive-registration transaction without submitting it.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the exact registration intent cannot be built or signed.
    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError>;

    /// Submit or recover the exact durable registration transaction and authoritative archive.
    ///
    /// # Errors
    ///
    /// Returns a backend error when transaction status, submission, or finalized archive recovery
    /// is unavailable or invalid.
    fn submit_or_recover_archive_registration(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError>;

    /// Revalidate or extend the compact provider-sidecar anchor before any proof submission.
    ///
    /// Backends that do not use detached provider-registration sidecars may retain this ready
    /// default. Production Musubi returns `Updated` after installing each new immutable sidecar;
    /// the engine journals that anchor and stops before the sidecar transaction can be submitted.
    ///
    /// # Errors
    ///
    /// Returns a backend error when provider sidecars cannot be revalidated or extended safely.
    fn checkpoint_archive_location_provider_registrations(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
        _checkpoint: Option<&PublicationProviderRegistrationCheckpointV1>,
    ) -> Result<PublicationProviderRegistrationCheckpointAdvanceV1, PublicationBackendError> {
        Ok(PublicationProviderRegistrationCheckpointAdvanceV1::Ready)
    }

    /// Coordinate and sign one exact location CAS without submitting it.
    ///
    /// # Errors
    ///
    /// Returns a backend error when storage coordination, fee quotation, or exact CAS signing fails.
    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError>;

    /// Submit or recover the exact journaled location CAS and finalized directory state.
    ///
    /// # Errors
    ///
    /// Returns a backend error when location submission, status recovery, or finalized directory
    /// retrieval fails.
    fn submit_or_recover_archive_location(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError>;

    /// Poll finalized provider completions and return a healthy location at quorum.
    ///
    /// # Errors
    ///
    /// Returns a backend error when finalized replication state cannot be queried or verified.
    fn finalized_replication(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError>;

    /// Read and fully verify the archive through one selected finalized provider.
    ///
    /// # Errors
    ///
    /// Returns a backend error when provider readback or complete archive verification fails.
    fn readback_provider(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError>;

    /// Prebuild, fee-quote, and sign one exact release transaction without submitting it.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the exact release transaction cannot be built, quoted, or
    /// signed against the supplied preparation floor.
    fn prepare_release_submission_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        preparation: &PublicationReleasePreparationFloorV1,
    ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError>;

    /// Query status first, then submit or replay the exact journaled release transaction.
    ///
    /// When `allow_absent_submission` is false, an absent transaction must remain unsent; exact
    /// pending, applied, and terminal status recovery remains available.
    ///
    /// # Errors
    ///
    /// Returns a backend error when authoritative status recovery or permitted exact submission
    /// cannot complete.
    fn submit_or_recover_release_submission(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationReleaseSubmissionIntentV1,
        allow_absent_submission: bool,
    ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError>;

    /// Poll finality and query both the exact home record and exact universal index row.
    ///
    /// # Errors
    ///
    /// Returns a backend error when synchronized finalized release projections cannot be queried.
    fn finalized_release_and_index(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError>;
}

/// One step of a resumable publication.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the fixed engine result returns complete publication evidence inline without changing its stable API"
)]
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
        match result {
            Err(error) => Err(error),
            Ok(value) => unlock.map(|()| value),
        }
    }
}

impl PublicationJournalStore {
    /// Open or create the private `publication-v1` journal directory.
    ///
    /// # Errors
    ///
    /// Returns a journal error when the state root or publication directory cannot be opened,
    /// created, synchronized, or proven to be a private real directory.
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
            open_read_only_no_follow_nonblocking(root.path())
                .and_then(|directory| directory.sync_all())
                .map_err(PublicationError::JournalIo)?;
        }
        Ok(Self { root })
    }

    /// Persist a new operation, or return the identical existing operation idempotently.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the request is invalid, the operation identity collides,
    /// the operation lock cannot be acquired, or the journal cannot be persisted safely.
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
    ///
    /// # Errors
    ///
    /// Returns not-found, journal I/O, or invalid-journal errors when the bounded journal cannot be
    /// opened as the same safe file, decoded canonically, or fully validated.
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
        #[cfg(all(test, unix))]
        substitute_publication_read_target_with_fifo_for_test(&path)
            .map_err(PublicationError::JournalIo)?;
        set_no_follow_nonblocking(&mut options);
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
            if !archive_registration_attempts_are_append_only(
                &previous.archive_registration_attempts,
                &next.archive_registration_attempts,
            ) {
                return Err(PublicationError::InvalidJournal(
                    "archive-registration attempt history is not append-only".to_owned(),
                ));
            }
            if !archive_location_attempts_are_append_only(
                &previous.archive_location_attempts,
                &next.archive_location_attempts,
            ) {
                return Err(PublicationError::InvalidJournal(
                    "archive-location attempt history is not append-only".to_owned(),
                ));
            }
            if !provider_registration_checkpoints_are_append_only(
                &previous.provider_registration_checkpoints,
                &next.provider_registration_checkpoints,
            ) {
                return Err(PublicationError::InvalidJournal(
                    "provider-registration checkpoint history is not append-only".to_owned(),
                ));
            }
            if !release_submission_attempts_are_append_only(
                &previous.release_submission_attempts,
                &next.release_submission_attempts,
            ) {
                return Err(PublicationError::InvalidJournal(
                    "release-submission attempt history is not append-only".to_owned(),
                ));
            }
            if previous.completion.is_some() && previous.completion != next.completion {
                return Err(PublicationError::InvalidJournal(
                    "compact final checkpoint is not append-only".to_owned(),
                ));
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
        let parent = open_read_only_no_follow_nonblocking(&parent_path)
            .map_err(PublicationError::JournalIo)?;
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
            Some(_) => (
                open_existing_operation_lock(&path).map_err(PublicationError::JournalIo)?,
                false,
            ),
            None => match create_operation_lock(&path) {
                Ok(file) => (file, true),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (
                    open_existing_operation_lock(&path).map_err(PublicationError::JournalIo)?,
                    false,
                ),
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

fn release_successor_preparation_is_ready(
    journal: &PublicationJournalV1,
    preparation: &PublicationReleasePreparationFloorV1,
) -> Result<bool, PublicationError> {
    let Some(previous) = journal.release_submission_attempts.last() else {
        return Ok(true);
    };
    let Some(PublicationReleaseSubmissionOutcomeV1::Terminal(terminal)) = &previous.outcome else {
        return Err(PublicationError::InvalidJournal(
            "a new release intent requires terminal evidence for the preceding attempt".to_owned(),
        ));
    };
    let prior_snapshot = terminal.absence().snapshot();
    let next_snapshot = preparation.replication.finalized_page.snapshot;
    if next_snapshot.finalized_height < prior_snapshot.finalized_height
        || next_snapshot.index_revision < prior_snapshot.index_revision
        || (next_snapshot.finalized_height == prior_snapshot.finalized_height
            && next_snapshot.finalized_block_hash != prior_snapshot.finalized_block_hash)
    {
        return Ok(false);
    }
    let PublicationReleaseSubmissionTerminalReasonV1::RegistryRejected { block_height, .. } =
        &terminal.reason
    else {
        return Ok(true);
    };
    let prior_location = previous.intent.preparation.location().ok_or_else(|| {
        PublicationError::InvalidJournal(
            "release terminal is missing its exact preparation location".to_owned(),
        )
    })?;
    let next_location = preparation.location().ok_or_else(|| {
        PublicationError::InvalidJournal(
            "release successor is missing its exact preparation location".to_owned(),
        )
    })?;
    let higher_same_location = preparation.location_generation
        == previous.intent.preparation.location_generation
        && next_location.location_id == prior_location.location_id
        && next_location.revision > prior_location.revision
        && next_location.finalized_height >= *block_height;
    let retired_then_replaced = preparation.location_generation
        > previous.intent.preparation.location_generation
        && previous
            .intent
            .preparation
            .location_generation
            .checked_sub(1)
            .map(usize::from)
            .and_then(|index| journal.archive_location_attempts.get(index))
            .and_then(|attempt| attempt.terminal.as_ref())
            .is_some_and(|location_terminal| {
                matches!(
                    location_terminal.reason,
                    PublicationArchiveLocationTerminalReasonV1::Retired
                ) && location_terminal.finalized_page.snapshot.finalized_height >= *block_height
            });
    Ok(higher_same_location || retired_then_replaced)
}

fn prepare_release_submission_attempt(
    journal: &PublicationJournalV1,
    registration: &PublicationArchiveRegistrationV1,
    replication: PublicationReplicationCheckpointV1,
    readbacks: Vec<PublicationReadbackEvidenceV1>,
    backend: &mut dyn PublicationBackend,
) -> Result<Option<PublicationReleaseSubmissionAttemptV1>, PublicationError> {
    if journal.release_submission_attempts.len() >= MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1 {
        return Err(PublicationError::Backend(
            PublicationBackendError::permanent("RELEASE_SUBMISSION_ATTEMPT_LIMIT_REACHED"),
        ));
    }
    let preparation = PublicationReleasePreparationFloorV1::try_new(
        registration.intent.generation,
        replication,
        readbacks,
        &journal.request,
        registration,
    )?;
    if !release_successor_preparation_is_ready(journal, &preparation)? {
        return Ok(None);
    }
    let intent = backend
        .prepare_release_submission_intent(journal.operation_id, &journal.request, &preparation)
        .map_err(PublicationError::Backend)?;
    intent.validate_for(journal.operation_id, &journal.request)?;
    if intent.preparation != preparation {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ReleaseSubmission,
            reason: "prepared release intent changed its exact storage/readback floor".to_owned(),
        });
    }
    let generation = u8::try_from(journal.release_submission_attempts.len() + 1)
        .expect("release-submission attempt bound fits u8");
    Ok(Some(PublicationReleaseSubmissionAttemptV1::new(
        generation, intent,
    )))
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
    ///
    /// # Errors
    ///
    /// Returns a publication error when request validation, operation locking, or initial journal
    /// persistence fails.
    pub fn begin_detached(
        &self,
        request: PublicationRequestV1,
    ) -> Result<PublicationOperationIdV1, PublicationError> {
        self.store
            .create(request)
            .map(|journal| journal.operation_id)
    }

    /// Repair the immutable CAR and plan for one pristine pre-ingress journal.
    ///
    /// The caller may rebuild the clean package outside the operation lock, but must supply the
    /// exact journal image it used. This method then compares that complete image under the
    /// per-operation lock, proves that the rebuilt publication and archive commitment equal the
    /// immutable request, and installs only absent or byte-identical sidecars. The journal is not
    /// advanced or rewritten.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the journal is not the initial validation revision, a
    /// concurrent transition changed it, the rebuilt content differs from the immutable request,
    /// or either exact sidecar cannot be verified and durably installed.
    pub(crate) fn recover_pre_ingress_sidecars(
        &self,
        expected: &PublicationJournalV1,
        rebuilt_publication: &MusubiPublicationV1,
        rebuilt_commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<PublicationStagedCarSourceV1, PublicationError> {
        expected.validate()?;
        if expected.phase != PublicationPhaseV1::Validation || expected.revision != 1 {
            return Err(PublicationError::InvalidJournal(
                "pre-ingress sidecar recovery requires the pristine validation revision".to_owned(),
            ));
        }
        if rebuilt_publication != &expected.request.publication
            || rebuilt_commitment != &expected.request.archive_commitment
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "rebuilt publication content differs from the immutable recovery request"
                    .to_owned(),
            });
        }

        let operation_id = expected.operation_id;
        let operation_lock = self.store.lock_operation(operation_id)?;
        let result = (|| {
            let current = self.store.load(operation_id)?;
            if current != *expected {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            if current.phase != PublicationPhaseV1::Validation || current.revision != 1 {
                return Err(PublicationError::InvalidJournal(
                    "pre-ingress sidecar recovery requires the pristine validation revision"
                        .to_owned(),
                ));
            }
            if rebuilt_publication != &current.request.publication
                || rebuilt_commitment != &current.request.archive_commitment
            {
                return Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Validation,
                    reason:
                        "rebuilt publication content differs from the immutable recovery request"
                            .to_owned(),
                });
            }

            operation_lock.validate()?;
            let source = PublicationStagedCarSourceV1::stage_bytes(
                self.store.root.path(),
                operation_id,
                rebuilt_commitment,
                plan,
                car,
            )?;
            operation_lock.validate()?;
            if self.store.load(operation_id)? != current {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            Ok(source)
        })();
        operation_lock.finish(result)
    }

    /// Persist a secret-free operation journal before installing its immutable CAR and plan.
    ///
    /// This ordering leaves a small authoritative recovery anchor if power is lost or local
    /// storage fails during either sidecar install; it never leaves an unindexed CAR behind. An
    /// identical call against the pristine journal reuses both sidecars idempotently.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the request cannot be journaled first, or when its exact
    /// canonical CAR/plan pair cannot then be verified and durably installed.
    pub fn begin_detached_with_car(
        &self,
        request: PublicationRequestV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<(PublicationOperationIdV1, PublicationStagedCarSourceV1), PublicationError> {
        let operation_id = request.operation_id();
        let publication = request.publication.clone();
        let commitment = request.archive_commitment.clone();
        let journal = self.store.create(request)?;
        if journal.operation_id != operation_id {
            return Err(PublicationError::InvalidJournal(
                "persisted publication operation identity changed".to_owned(),
            ));
        }
        let source =
            self.recover_pre_ingress_sidecars(&journal, &publication, &commitment, plan, car)?;
        Ok((operation_id, source))
    }

    /// Start or idempotently recover an operation, running until finality or a pending poll.
    ///
    /// # Errors
    ///
    /// Returns a publication error when journal recovery, CAR access, backend execution, evidence
    /// validation, or a durable transition fails.
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
    ///
    /// # Errors
    ///
    /// Returns a publication error when the journal cannot be loaded or a subsequent backend,
    /// validation, or durable transition fails.
    pub fn resume(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        self.run(operation_id, source, backend)
    }

    /// Advance exactly one durable phase, making retries observable to callers.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the current journal, CAR, backend evidence, append-only
    /// transition, or persistent state fails validation.
    #[allow(
        clippy::too_many_lines,
        reason = "the publication engine keeps the complete persist-before-send security state machine explicit in one fixed-protocol transition"
    )]
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
                let plan = source
                    .car_plan(&journal.request.archive_commitment)
                    .map_err(PublicationError::CarSource)?;
                plan.validate(&journal.request.archive_commitment)
                    .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let evidence = backend
                    .validate_clean_package(operation_id, &journal.request, car.as_mut())
                    .map_err(PublicationError::Backend)?;
                evidence.validate_for(&journal.request)?;
                next.validation = Some(evidence);
                next.phase = PublicationPhaseV1::SeedIngress;
            }
            PublicationPhaseV1::SeedIngress => {
                if journal.archive_registration_attempts.len()
                    >= MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1
                {
                    return Err(PublicationError::Backend(
                        PublicationBackendError::permanent(
                            "ARCHIVE_REGISTRATION_ATTEMPT_LIMIT_REACHED",
                        ),
                    ));
                }
                let expected = journal.request.receipt_binding();
                let plan = source
                    .car_plan(&journal.request.archive_commitment)
                    .map_err(PublicationError::CarSource)?;
                plan.validate(&journal.request.archive_commitment)
                    .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))?;
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let receipt = backend
                    .stage_authenticated_seed_ingress(
                        operation_id,
                        &expected,
                        &journal.request.archive_commitment,
                        &plan,
                        car.as_mut(),
                    )
                    .map_err(PublicationError::Backend)?;
                let now = backend
                    .current_time_ms()
                    .map_err(PublicationError::Backend)?;
                verify_seed_ingress_receipt_with_bounded_service_lead(&receipt, &expected, now)?;
                next.staging_receipt = Some(receipt);
                next.phase = PublicationPhaseV1::ArchiveRegistration;
            }
            PublicationPhaseV1::ArchiveRegistration => {
                let receipt = journal.staging_receipt.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing staging receipt".to_owned())
                })?;
                let active_attempt = journal
                    .archive_registration_attempts
                    .last()
                    .filter(|attempt| attempt.terminal.is_none());
                if active_attempt.is_none() {
                    let now = backend
                        .current_time_ms()
                        .map_err(PublicationError::Backend)?;
                    if now > receipt.payload.expires_at_ms {
                        next.staging_receipt = None;
                        next.phase = PublicationPhaseV1::SeedIngress;
                    } else {
                        verify_seed_ingress_receipt_with_bounded_service_lead(
                            receipt,
                            &journal.request.receipt_binding(),
                            now,
                        )?;
                        if now < receipt.payload.issued_at_ms {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        let intent = backend
                            .prepare_archive_registration_intent(
                                operation_id,
                                &journal.request,
                                receipt,
                            )
                            .map_err(PublicationError::Backend)?;
                        intent.validate_for(operation_id, &journal.request, receipt)?;
                        let generation =
                            u8::try_from(journal.archive_registration_attempts.len() + 1)
                                .expect("archive-registration attempt bound fits u8");
                        next.archive_registration_attempts.push(
                            PublicationArchiveRegistrationAttemptV1::new(generation, intent),
                        );
                    }
                } else if let Some(attempt) = active_attempt
                    && journal.registered_archive.is_none()
                {
                    match backend
                        .submit_or_recover_archive_registration(
                            operation_id,
                            &journal.request,
                            &attempt.intent,
                        )
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationArchiveRegistrationAdvanceV1::Pending => {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        PublicationArchiveRegistrationAdvanceV1::Registered(registered) => {
                            registered.validate_for(&journal.request, &attempt.intent)?;
                            next.registered_archive = Some(registered);
                        }
                        PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(terminal) => {
                            terminal.validate_for(&journal.request, &attempt.intent)?;
                            next.archive_registration_attempts
                                .last_mut()
                                .expect("active registration attempt exists")
                                .terminal = Some(terminal);
                            next.staging_receipt = None;
                            next.phase = PublicationPhaseV1::SeedIngress;
                        }
                    }
                } else {
                    let registered = journal
                        .registered_archive
                        .as_ref()
                        .expect("checked authoritative archive");
                    let active_location_attempt = journal
                        .archive_location_attempts
                        .last()
                        .filter(|attempt| attempt.terminal.is_none());
                    if active_location_attempt.is_none() {
                        if journal.archive_location_attempts.len()
                            >= MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1
                        {
                            return Err(PublicationError::Backend(
                                PublicationBackendError::permanent(
                                    "ARCHIVE_LOCATION_ATTEMPT_LIMIT_REACHED",
                                ),
                            ));
                        }
                        let generation = u8::try_from(journal.archive_location_attempts.len() + 1)
                            .expect("archive-location attempt bound fits u8");
                        let prior_location_ids = journal
                            .archive_location_attempts
                            .iter()
                            .map(|attempt| attempt.intent.location_id)
                            .collect::<Vec<_>>();
                        let provider_checkpoint = generation
                            .checked_sub(1)
                            .map(usize::from)
                            .and_then(|index| journal.provider_registration_checkpoints.get(index));
                        match backend
                            .checkpoint_archive_location_provider_registrations(
                                operation_id,
                                &journal.request,
                                registered,
                                generation,
                                &prior_location_ids,
                                provider_checkpoint,
                            )
                            .map_err(PublicationError::Backend)?
                        {
                            PublicationProviderRegistrationCheckpointAdvanceV1::Ready => {
                                let intent = backend
                                    .prepare_archive_location_intent(
                                        operation_id,
                                        &journal.request,
                                        registered,
                                        generation,
                                        &prior_location_ids,
                                    )
                                    .map_err(PublicationError::Backend)?;
                                intent.validate_for(
                                    operation_id,
                                    &journal.request,
                                    registered,
                                    &prior_location_ids,
                                )?;
                                next.archive_location_attempts.push(
                                    PublicationArchiveLocationAttemptV1::new(generation, intent),
                                );
                            }
                            PublicationProviderRegistrationCheckpointAdvanceV1::Updated(
                                checkpoint,
                            ) => {
                                checkpoint.validate_for(&journal.request, generation)?;
                                if provider_checkpoint == Some(&checkpoint) {
                                    return Err(PublicationError::InvalidEvidence {
                                        phase,
                                        reason: "provider-registration checkpoint update made no append-only progress"
                                            .to_owned(),
                                    });
                                }
                                if let Some(existing) = next
                                    .provider_registration_checkpoints
                                    .get_mut(usize::from(generation - 1))
                                {
                                    *existing = checkpoint;
                                } else if next.provider_registration_checkpoints.len()
                                    == usize::from(generation - 1)
                                {
                                    next.provider_registration_checkpoints.push(checkpoint);
                                } else {
                                    return Err(PublicationError::InvalidJournal(
                                        "provider-registration checkpoint generations are not contiguous"
                                            .to_owned(),
                                    ));
                                }
                            }
                        }
                    } else if let Some(attempt) = active_location_attempt {
                        if attempt.registration.is_some() {
                            next.phase = PublicationPhaseV1::Replication;
                        } else {
                            let prior_location_ids = journal.archive_location_attempts
                                [..journal.archive_location_attempts.len() - 1]
                                .iter()
                                .map(|prior| prior.intent.location_id)
                                .collect::<Vec<_>>();
                            match backend
                                .submit_or_recover_archive_location(
                                    operation_id,
                                    &journal.request,
                                    registered,
                                    &attempt.intent,
                                    &prior_location_ids,
                                )
                                .map_err(PublicationError::Backend)?
                            {
                                PublicationArchiveLocationAdvanceV1::Pending => {
                                    return Ok(PublicationAdvanceV1::Pending(phase));
                                }
                                PublicationArchiveLocationAdvanceV1::Registered(registration) => {
                                    registration.validate_for(
                                        operation_id,
                                        &journal.request,
                                        registered,
                                        &prior_location_ids,
                                    )?;
                                    if registration.intent != attempt.intent {
                                        return Err(PublicationError::InvalidEvidence {
                                            phase,
                                            reason: "archive-location finality changed its exact signed intent"
                                                .to_owned(),
                                        });
                                    }
                                    next.archive_location_attempts
                                        .last_mut()
                                        .expect("active location attempt exists")
                                        .registration = Some(registration);
                                    next.phase = PublicationPhaseV1::Replication;
                                }
                                PublicationArchiveLocationAdvanceV1::Terminal(terminal) => {
                                    append_location_terminal(&journal, &mut next, terminal)?;
                                }
                            }
                        }
                    }
                }
            }
            PublicationPhaseV1::Replication => {
                let registration = journal.registration()?;
                match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                        checkpoint.validate_for(&journal.request, registration)?;
                        next.replication = Some(checkpoint);
                        next.phase = PublicationPhaseV1::Readback;
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        append_location_terminal(&journal, &mut next, terminal)?;
                    }
                }
            }
            PublicationPhaseV1::Readback => {
                let registration = journal.registration()?;
                let journaled_checkpoint = journal.replication.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing finalized replication".to_owned())
                })?;
                let checkpoint = match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        if retirement_checkpoint_progress(
                            &journal,
                            journaled_checkpoint,
                            &terminal,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        append_location_terminal(&journal, &mut next, terminal)?;
                        return self.persist_advance(&journal, next);
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => checkpoint,
                };
                if replication_checkpoint_progress(
                    &journal.request,
                    registration,
                    journaled_checkpoint,
                    &checkpoint,
                )? == PublicationLocationProgressV1::Stale
                {
                    return Ok(PublicationAdvanceV1::Pending(phase));
                }
                if &checkpoint != journaled_checkpoint {
                    next.replication = Some(checkpoint.clone());
                }
                let location = checkpoint.location(registration)?;
                let mut readbacks = Vec::with_capacity(2);
                let mut first_permanent_failure = None;
                let mut first_retryable_failure = None;
                let mut first_invalid_evidence = None;
                for provider in &location.providers {
                    let evidence = match backend.readback_provider(
                        operation_id,
                        &journal.request,
                        location,
                        *provider,
                    ) {
                        Ok(evidence) => evidence,
                        Err(error) => {
                            match error.class() {
                                PublicationBackendFailureClass::Permanent => {
                                    if first_permanent_failure.is_none() {
                                        first_permanent_failure = Some(error);
                                    }
                                }
                                PublicationBackendFailureClass::Retryable => {
                                    if first_retryable_failure.is_none() {
                                        first_retryable_failure = Some(error);
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    if let Err(error) = evidence.validate_for(&journal.request, location, *provider)
                    {
                        if first_invalid_evidence.is_none() {
                            first_invalid_evidence = Some(error);
                        }
                        continue;
                    }
                    readbacks.push(evidence);
                    if readbacks.len() == 2 {
                        break;
                    }
                }
                if readbacks.len() != 2 {
                    if let Some(error) = first_permanent_failure.or(first_retryable_failure) {
                        return Err(PublicationError::Backend(error));
                    }
                    if let Some(error) = first_invalid_evidence {
                        return Err(error);
                    }
                    return Err(PublicationError::Backend(
                        PublicationBackendError::retryable("PROVIDER_READBACK_QUORUM_UNAVAILABLE"),
                    ));
                }
                validate_readback_subset(&journal.request, location, &readbacks)?;
                let Some(attempt) = prepare_release_submission_attempt(
                    &journal,
                    registration,
                    checkpoint.clone(),
                    readbacks,
                    backend,
                )?
                else {
                    if &checkpoint != journaled_checkpoint {
                        next.replication = Some(checkpoint);
                        return self.persist_advance(&journal, next);
                    }
                    return Ok(PublicationAdvanceV1::Pending(phase));
                };
                next.replication = Some(attempt.intent.preparation.replication.clone());
                next.readbacks
                    .clone_from(&attempt.intent.preparation.readbacks);
                next.release_submission_attempts.push(attempt);
                next.phase = PublicationPhaseV1::ReleaseSubmission;
            }
            PublicationPhaseV1::ReleaseSubmission => {
                let registration = journal.registration()?;
                let journaled_checkpoint = journal.replication.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing finalized replication".to_owned())
                })?;
                if let Some(active_attempt) = journal
                    .release_submission_attempts
                    .last()
                    .filter(|attempt| attempt.outcome.is_none())
                {
                    let allow_absent_submission = match backend
                        .finalized_replication(operation_id, &journal.request, registration)
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationReplicationAdvanceV1::Pending => false,
                        PublicationReplicationAdvanceV1::Retired(terminal) => {
                            retirement_checkpoint_progress(
                                &journal,
                                journaled_checkpoint,
                                &terminal,
                            )?;
                            false
                        }
                        PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                            let progress = replication_checkpoint_progress(
                                &journal.request,
                                registration,
                                journaled_checkpoint,
                                &checkpoint,
                            )?;
                            let current_location = checkpoint.location(registration)?;
                            let signed_location = active_attempt
                                .intent
                                .preparation
                                .location()
                                .ok_or_else(|| {
                                    PublicationError::InvalidJournal(
                                        "release intent is missing its signed location".to_owned(),
                                    )
                                })?;
                            progress == PublicationLocationProgressV1::Current
                                && current_location == signed_location
                        }
                    };
                    match backend
                        .submit_or_recover_release_submission(
                            operation_id,
                            &journal.request,
                            &active_attempt.intent,
                            allow_absent_submission,
                        )
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationReleaseSubmissionAdvanceV1::Pending => {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        PublicationReleaseSubmissionAdvanceV1::Applied(submission) => {
                            let outcome = PublicationReleaseSubmissionOutcomeV1::applied(
                                &active_attempt.intent,
                                submission,
                            );
                            outcome.validate_for(
                                operation_id,
                                &journal.request,
                                &active_attempt.intent,
                            )?;
                            next.release_submission_attempts
                                .last_mut()
                                .expect("active release attempt exists")
                                .outcome = Some(outcome);
                            next.submission = Some(submission);
                            next.phase = PublicationPhaseV1::FinalVerification;
                        }
                        PublicationReleaseSubmissionAdvanceV1::Terminal(terminal) => {
                            terminal.validate_for(&journal.request, &active_attempt.intent)?;
                            next.release_submission_attempts
                                .last_mut()
                                .expect("active release attempt exists")
                                .outcome =
                                Some(PublicationReleaseSubmissionOutcomeV1::Terminal(terminal));
                        }
                    }
                    return self.persist_advance(&journal, next);
                }
                let journaled_location = journaled_checkpoint.location(registration)?;
                match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        if retirement_checkpoint_progress(
                            &journal,
                            journaled_checkpoint,
                            &terminal,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        append_location_terminal(&journal, &mut next, terminal)?;
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                        if replication_checkpoint_progress(
                            &journal.request,
                            registration,
                            journaled_checkpoint,
                            &checkpoint,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        let location = checkpoint.location(registration)?;
                        if &checkpoint != journaled_checkpoint {
                            let target_changed = location != journaled_location;
                            next.replication = Some(checkpoint.clone());
                            if target_changed {
                                next.readbacks.clear();
                                next.phase = PublicationPhaseV1::Readback;
                                return self.persist_advance(&journal, next);
                            }
                        }
                        let Some(attempt) = prepare_release_submission_attempt(
                            &journal,
                            registration,
                            checkpoint,
                            journal.readbacks.clone(),
                            backend,
                        )?
                        else {
                            if next.replication != journal.replication {
                                return self.persist_advance(&journal, next);
                            }
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        };
                        next.replication = Some(attempt.intent.preparation.replication.clone());
                        next.readbacks
                            .clone_from(&attempt.intent.preparation.readbacks);
                        next.release_submission_attempts.push(attempt);
                    }
                }
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
                next.completion = Some(PublicationFinalCheckpointV1::from_verified(
                    &journal.request,
                    submission,
                    &final_evidence,
                )?);
            }
        }
        self.persist_advance(&journal, next)
    }

    fn persist_advance(
        &self,
        journal: &PublicationJournalV1,
        next: PublicationJournalV1,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let persisted = self.store.transition(journal, next)?;
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

fn verify_seed_ingress_receipt_with_bounded_service_lead(
    receipt: &MusubiSeedIngressReceiptV1,
    expected: &MusubiSeedIngressReceiptBindingV1,
    current_time_ms: u64,
) -> Result<(), PublicationError> {
    let latest_accepted_issue_time = current_time_ms
        .checked_add(MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1)
        .ok_or_else(|| PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            reason: "seed-ingress receipt clock bound overflowed".to_owned(),
        })?;
    if current_time_ms == 0 || receipt.payload.issued_at_ms > latest_accepted_issue_time {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            reason: "seed-ingress receipt exceeds the bounded service clock lead".to_owned(),
        });
    }
    receipt
        .verify(expected, current_time_ms.max(receipt.payload.issued_at_ms))
        .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))
}

fn append_location_terminal(
    journal: &PublicationJournalV1,
    next: &mut PublicationJournalV1,
    terminal: PublicationArchiveLocationTerminalV1,
) -> Result<(), PublicationError> {
    let floor = location_terminal_floor(journal)?;
    validate_location_terminal(journal, &terminal, &floor)?;
    let attempt = next
        .archive_location_attempts
        .last_mut()
        .expect("active location attempt exists");
    attempt.terminal = Some(terminal);
    attempt.terminal_floor = Some(floor);
    next.replication = None;
    next.readbacks.clear();
    next.phase = PublicationPhaseV1::ArchiveRegistration;
    Ok(())
}

fn location_terminal_floor(
    journal: &PublicationJournalV1,
) -> Result<PublicationArchiveLocationTerminalFloorV1, PublicationError> {
    let attempt = journal
        .archive_location_attempts
        .last()
        .filter(|attempt| attempt.terminal.is_none())
        .ok_or_else(|| {
            PublicationError::InvalidJournal(
                "archive-location terminal evidence has no active generation".to_owned(),
            )
        })?;
    Ok(journal.replication.as_ref().map_or_else(
        || {
            if attempt.registration.is_some() {
                PublicationArchiveLocationTerminalFloorV1::Registered
            } else {
                PublicationArchiveLocationTerminalFloorV1::Prepared
            }
        },
        |checkpoint| PublicationArchiveLocationTerminalFloorV1::Replication(checkpoint.clone()),
    ))
}

fn validate_location_terminal(
    journal: &PublicationJournalV1,
    terminal: &PublicationArchiveLocationTerminalV1,
    floor: &PublicationArchiveLocationTerminalFloorV1,
) -> Result<(), PublicationError> {
    let attempt = journal
        .archive_location_attempts
        .last()
        .filter(|attempt| attempt.terminal.is_none())
        .ok_or_else(|| {
            PublicationError::InvalidJournal(
                "archive-location terminal evidence has no active generation".to_owned(),
            )
        })?;
    let registered = journal.registered_archive.as_ref().ok_or_else(|| {
        PublicationError::InvalidJournal(
            "archive-location terminal evidence is missing archive finality".to_owned(),
        )
    })?;
    let prior_location_ids = journal.archive_location_attempts
        [..journal.archive_location_attempts.len() - 1]
        .iter()
        .map(|prior| prior.intent.location_id)
        .collect::<Vec<_>>();
    terminal.validate_for(
        journal.operation_id,
        &journal.request,
        registered,
        attempt,
        &prior_location_ids,
        floor,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublicationLocationProgressV1 {
    Stale,
    Current,
}

fn finalized_page_progress(
    previous: &MusubiArchiveLocationPageV1,
    current: &MusubiArchiveLocationPageV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    if (current.snapshot.finalized_height == previous.snapshot.finalized_height
        && current.snapshot != previous.snapshot)
        || (current.snapshot == previous.snapshot
            && (current.archive != previous.archive || current.items != previous.items))
        || (current.archive.location_revision == previous.archive.location_revision
            && (current.archive != previous.archive || current.items != previous.items))
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "equal finalized archive-location checkpoints carried different state"
                .to_owned(),
        });
    }
    if current.snapshot.finalized_height < previous.snapshot.finalized_height
        || current.snapshot.index_revision < previous.snapshot.index_revision
        || current.archive.location_revision < previous.archive.location_revision
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    Ok(PublicationLocationProgressV1::Current)
}

fn replication_checkpoint_progress(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    previous: &PublicationReplicationCheckpointV1,
    current: &PublicationReplicationCheckpointV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    previous.validate_for(request, registration)?;
    current.validate_for(request, registration)?;
    if finalized_page_progress(&previous.finalized_page, &current.finalized_page)?
        == PublicationLocationProgressV1::Stale
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    location_progress(
        previous.location(registration)?,
        current.location(registration)?,
    )
}

fn retirement_checkpoint_progress(
    journal: &PublicationJournalV1,
    checkpoint: &PublicationReplicationCheckpointV1,
    terminal: &PublicationArchiveLocationTerminalV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    let registration = journal.registration()?;
    checkpoint.validate_for(&journal.request, registration)?;
    validate_location_terminal(
        journal,
        terminal,
        &PublicationArchiveLocationTerminalFloorV1::Registered,
    )?;
    if finalized_page_progress(&checkpoint.finalized_page, &terminal.finalized_page)?
        == PublicationLocationProgressV1::Stale
        || terminal.finalized_page.archive.location_revision
            <= checkpoint.finalized_page.archive.location_revision
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    let floor = location_terminal_floor(journal)?;
    validate_location_terminal(journal, terminal, &floor)?;
    Ok(PublicationLocationProgressV1::Current)
}

fn location_progress(
    previous: &MusubiArchiveLocationV1,
    current: &MusubiArchiveLocationV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    if current.archive_id != previous.archive_id || current.location_id != previous.location_id {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "finalized archive location changed its stable identity".to_owned(),
        });
    }
    if current.revision < previous.revision {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    if current.revision == previous.revision {
        return if current == previous {
            Ok(PublicationLocationProgressV1::Current)
        } else {
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "equal archive-location revisions carried different records".to_owned(),
            })
        };
    }
    if current.finalized_height < previous.finalized_height {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "archive-location revision advanced while finality regressed".to_owned(),
        });
    }
    Ok(PublicationLocationProgressV1::Current)
}

/// Validate a current healthy location for the immutable publication archive.
///
/// The stable location identity cannot be reused after retirement. Its renewable pin, order,
/// provider set, and epochs may legitimately advance after the coordinator checkpoint. Core
/// exact-resolves the location's aggregate digest to immutable provider proofs before publishing
/// this compact finalized state, while the publisher independently verifies archive bytes through
/// two selected providers before release submission.
pub(crate) fn validate_replication(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    location: &MusubiArchiveLocationV1,
) -> Result<(), PublicationError> {
    location
        .validate()
        .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
    let registered_location = registration.location()?;
    if location.archive_id != request.archive_commitment.archive_id()
        || location.location_id != registration.location_id()
        || location.state != MusubiArchiveLocationStateV1::Healthy
        || location.providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
        || location.finalized_height < registration.applied_height
        || location_progress(registered_location, location)?
            != PublicationLocationProgressV1::Current
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "finalized archive location, pin, order, or quorum was substituted".to_owned(),
        });
    }
    Ok(())
}

/// Publication workflow error with retry class preserved for backend failures.
#[derive(Debug)]
pub enum PublicationError {
    /// The immutable CAR or canonical plan sidecar could not be reopened or read.
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
            Self::CarSource(error) => {
                write!(
                    formatter,
                    "failed to open publication CAR or plan sidecar: {error}"
                )
            }
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
    // First-release reset semantics are fail-closed: there is no parser or field synthesis for
    // any pre-release journal layout.
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

fn staged_plan_relative_path(operation_id: PublicationOperationIdV1) -> PathBuf {
    PathBuf::from(JOURNAL_DIRECTORY).join(format!("{operation_id}.{STAGED_PLAN_EXTENSION}"))
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

fn canonical_encoded_len<T: Encode>(value: &T) -> Result<usize, String> {
    norito::encode_canonical(value)
        .map(|bytes| bytes.len())
        .map_err(|error| format!("canonical Norito encoding failed: {error}"))
}

fn ensure_release_component_budget<T: Encode>(
    value: &T,
    maximum: usize,
    component: &str,
    phase: PublicationPhaseV1,
) -> Result<(), PublicationError> {
    let observed = canonical_encoded_len(value)
        .map_err(|reason| PublicationError::InvalidEvidence { phase, reason })?;
    if observed > maximum {
        return Err(PublicationError::InvalidEvidence {
            phase,
            reason: format!("{component} canonical size {observed} exceeds the limit {maximum}"),
        });
    }
    Ok(())
}

fn archive_registration_instruction_digest(instruction: &RegisterMusubiArchiveV1) -> [u8; 32] {
    let canonical = norito::encode_canonical(instruction)
        .expect("typed archive registration instruction has a canonical Norito encoding");
    domain_hash(ARCHIVE_REGISTRATION_INSTRUCTION_DOMAIN, &canonical)
}

fn archive_location_instruction_digest(instruction: &AddMusubiArchiveLocationV1) -> [u8; 32] {
    let canonical = norito::encode_canonical(instruction)
        .expect("typed archive location instruction has a canonical Norito encoding");
    domain_hash(ARCHIVE_LOCATION_INSTRUCTION_DOMAIN, &canonical)
}

fn open_existing_operation_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    set_no_follow_nonblocking(&mut options);
    #[cfg(windows)]
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    options.open(path)
}

fn create_operation_lock(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    set_no_follow_nonblocking(&mut options);
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

fn open_read_only_no_follow_nonblocking(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_nonblocking(&mut options);
    options.open(path)
}

fn set_no_follow_nonblocking(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        // A substituted FIFO or device must never block before descriptor metadata rejects it.
        options.custom_flags(platform_no_follow_flag() | platform_nonblocking_flag());
    }
    #[cfg(windows)]
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    #[cfg(not(any(unix, windows)))]
    let _ = options;
}

#[cfg(all(test, unix))]
fn substitute_publication_read_target_with_fifo_for_test(path: &Path) -> io::Result<()> {
    let substitute = TEST_PUBLICATION_READ_FIFO_SUBSTITUTIONS.with(|remaining| {
        let count = remaining.get();
        remaining.set(count.saturating_sub(1));
        count != 0
    });
    if !substitute {
        return Ok(());
    }
    fs::remove_file(path)?;
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "test fixture could not substitute a FIFO publication input",
        ))
    }
}

#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("Musubi publication file reads are not qualified for this Android architecture");

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
compile_error!("Musubi publication file reads are not qualified for this Unix target");

#[cfg(all(target_os = "android", target_arch = "riscv64"))]
const fn platform_no_follow_flag() -> i32 {
    0x400000
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}

#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
}

#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x8000
}

#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0x20000
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
    target_os = "linux",
    any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x80
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "sparc", target_arch = "sparc64")
))]
const fn platform_nonblocking_flag() -> i32 {
    0x4000
}

#[cfg(any(
    target_os = "android",
    all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6",
            target_arch = "sparc",
            target_arch = "sparc64"
        ))
    )
))]
const fn platform_nonblocking_flag() -> i32 {
    0x800
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
const fn platform_nonblocking_flag() -> i32 {
    0x4
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io::Cursor};

    #[cfg(unix)]
    use std::io::Write as _;
    #[cfg(unix)]
    use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};

    use iroha::{
        crypto::{Algorithm, KeyPair, SignatureOf},
        data_model::{
            account::{MultisigMember, MultisigPolicy},
            musubi::{
                MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1, MUSUBI_MAX_ARCHIVE_LOCATIONS_V1,
                MUSUBI_MAX_LOCATION_PROVIDERS_V1, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1,
                MusubiArchiveAvailabilityV1, MusubiArtifactGovernanceStateV1,
                MusubiArtifactTakedownV1, MusubiGovernanceActionDigestV1, MusubiKotodamaEditionV1,
                MusubiPackageIdV1, MusubiPackageScopeV1, MusubiPageRequestV1,
                MusubiProviderBundleVerificationApprovalV1,
                MusubiProviderBundleVerificationAttestationV1,
                MusubiProviderBundleVerificationBindingV1,
                MusubiProviderBundleVerificationPayloadV1, MusubiReasonV1, MusubiReleaseIdV1,
                MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiReleaseRevisionsV1,
                MusubiReleaseSelectionStateV1, MusubiReleaseYankV1, MusubiResolutionProofV1,
                MusubiResolverIndexQueryV1, MusubiSeedIngressReceiptApprovalV1,
                MusubiSeedIngressReceiptPayloadV1, MusubiStorageAvailabilityV1,
                MusubiVerificationLockV1, MusubiVersionV1,
                musubi_provider_bundle_attestation_set_digest_v1, validate_musubi_account_id_v1,
            },
            nexus::DataSpaceId,
            proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
            sorafs::pin_registry::{
                ChunkerProfileHandle, ManifestRootCid, ProviderIngestCompletionAuthorityV1,
                ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
            },
            transaction::{FeePaymentIntent, TransactionBuilder},
        },
    };
    use tempfile::tempdir;

    use super::*;

    struct BytesSource(Vec<u8>);

    impl PublicationCarSource for BytesSource {
        fn open_car(&self) -> io::Result<Box<dyn Read + '_>> {
            Ok(Box::new(Cursor::new(self.0.as_slice())))
        }

        fn car_plan(
            &self,
            commitment: &MusubiArchiveCommitmentV1,
        ) -> io::Result<MusubiSeedIngressCarPlanV1> {
            MusubiSeedIngressCarPlanV1::from_car_build_plan(
                &publication_fixture_car_plan(),
                commitment,
            )
            .map_err(|_| invalid_plan_source("test publication plan differs from the commitment"))
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
        let (plan, bytes, commitment) = publication_fixture_canonical_car();
        let source = PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            &commitment,
            &plan,
            &bytes,
        )
        .expect("stage committed CAR");
        let car_before = fs::metadata(source.path()).expect("staged CAR metadata");
        let plan_before = fs::metadata(source.plan_path()).expect("staged plan metadata");
        PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            &commitment,
            &plan,
            &bytes,
        )
        .expect("identical retry reuses staged CAR and plan");
        assert!(same_file_snapshot(
            &car_before,
            &fs::metadata(source.path()).expect("reused CAR metadata")
        ));
        assert!(same_file_snapshot(
            &plan_before,
            &fs::metadata(source.plan_path()).expect("reused plan metadata")
        ));
        assert_eq!(
            source.car_plan(&commitment).expect("reopen exact plan"),
            MusubiSeedIngressCarPlanV1::from_car_build_plan(&plan, &commitment).expect("wire plan")
        );

        fs::write(source.path(), vec![0xA5; bytes.len()]).expect("substitute same-length fixture");
        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                operation_id,
                &commitment,
                &plan,
                &bytes,
            ),
            Err(PublicationError::JournalWrite(ref error))
                if error.code() == crate::atomic_io::AtomicWriteErrorCode::ImmutableConflict
        ));

        let other_id = "0303030303030303030303030303030303030303030303030303030303030303"
            .parse()
            .expect("other operation id");
        let mut wrong_commitment = commitment.clone();
        wrong_commitment.car_digest = MusubiContentDigestV1::new([9; 32]);
        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                other_id,
                &wrong_commitment,
                &plan,
                &bytes,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        assert!(
            !PublicationStagedCarSourceV1::new(state.path(), other_id, commitment.car_size)
                .path()
                .exists()
        );
    }

    #[test]
    fn staged_car_rejects_a_different_file_inventory_before_install() {
        let state = tempdir().expect("state root");
        let _store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let operation_id = "2929292929292929292929292929292929292929292929292929292929292929"
            .parse()
            .expect("operation id");
        let (mut substituted_plan, bytes, commitment) = publication_fixture_canonical_car();
        let source_file = substituted_plan
            .files
            .iter_mut()
            .find(|file| file.path.iter().map(String::as_str).eq(["src", "lib.ko"]))
            .expect("fixture source file");
        source_file.path = vec!["src".to_owned(), "renamed.ko".to_owned()];
        substituted_plan
            .validate()
            .expect("substituted inventory remains a valid SoraFS plan");
        MusubiSeedIngressCarPlanV1::from_car_build_plan(&substituted_plan, &commitment)
            .expect("scalar commitment fields do not bind the file inventory");

        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                operation_id,
                &commitment,
                &substituted_plan,
                &bytes,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        let source =
            PublicationStagedCarSourceV1::new(state.path(), operation_id, commitment.car_size);
        assert!(!source.path().exists());
        assert!(!source.plan_path().exists());
    }

    #[test]
    fn detached_begin_persists_the_recovery_anchor_before_sidecar_failure() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (request, _) = request();
        let operation_id = request.operation_id();
        let expected_size = request.archive_commitment.car_size;

        assert!(matches!(
            engine.begin_detached_with_car(
                request.clone(),
                &publication_fixture_car_plan(),
                b"not the committed canonical CAR",
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        assert_eq!(
            store
                .load(operation_id)
                .expect("durable recovery anchor")
                .request,
            request
        );
        let source = PublicationStagedCarSourceV1::new(state.path(), operation_id, expected_size);
        assert!(!source.path().exists());
        assert!(!source.plan_path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn detached_begin_idempotently_reuses_sidecars_while_the_journal_is_pristine() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment);
        let expected_operation_id = request.operation_id();

        let (operation_id, source) = engine
            .begin_detached_with_car(request.clone(), &plan, &car)
            .expect("begin detached publication");
        assert_eq!(operation_id, expected_operation_id);
        let journal_before = store.load(operation_id).expect("pristine journal");
        assert_eq!(journal_before.phase, PublicationPhaseV1::Validation);
        assert_eq!(journal_before.revision, 1);
        let car_before = fs::metadata(source.path()).expect("staged CAR metadata");
        let plan_before = fs::metadata(source.plan_path()).expect("staged plan metadata");

        let (retried_operation_id, retried_source) = engine
            .begin_detached_with_car(request, &plan, &car)
            .expect("idempotently recover pristine detached publication");
        assert_eq!(retried_operation_id, operation_id);
        assert!(same_file_snapshot(
            &car_before,
            &fs::metadata(retried_source.path()).expect("reused CAR metadata")
        ));
        assert!(same_file_snapshot(
            &plan_before,
            &fs::metadata(retried_source.plan_path()).expect("reused plan metadata")
        ));
        assert_eq!(
            store
                .load(operation_id)
                .expect("unchanged pristine journal"),
            journal_before
        );
    }

    #[test]
    fn detached_begin_rejects_an_advanced_journal_that_must_resume() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment);
        let (operation_id, source) = engine
            .begin_detached_with_car(request.clone(), &plan, &car)
            .expect("begin detached publication");
        let pristine = store.load(operation_id).expect("pristine journal");
        let mut next = pristine.clone();
        next.validation = Some(validation_evidence(&request));
        next.phase = PublicationPhaseV1::SeedIngress;
        let advanced = store
            .transition(&pristine, next)
            .expect("advance fixture journal");
        let car_before = fs::read(source.path()).expect("read staged CAR");
        let plan_before = fs::read(source.plan_path()).expect("read staged plan");

        assert!(matches!(
            engine.begin_detached_with_car(request, &plan, &car),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("pristine validation revision")
        ));
        assert_eq!(
            store
                .load(operation_id)
                .expect("unchanged advanced journal"),
            advanced
        );
        assert_eq!(
            fs::read(source.path()).expect("reread staged CAR"),
            car_before
        );
        assert_eq!(
            fs::read(source.plan_path()).expect("reread staged plan"),
            plan_before
        );
    }

    #[cfg(unix)]
    #[test]
    fn pristine_pre_ingress_recovery_installs_and_idempotently_reuses_exact_sidecars() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment.clone());
        let journal = store
            .create(request.clone())
            .expect("persist pristine recovery anchor");
        let journal_path = state
            .path()
            .join(journal_relative_path(journal.operation_id));
        let journal_before = fs::read(&journal_path).expect("read pristine journal");

        let source = engine
            .recover_pre_ingress_sidecars(&journal, &request.publication, &commitment, &plan, &car)
            .expect("recover exact sidecars");
        let car_before = fs::metadata(source.path()).expect("recovered CAR metadata");
        let plan_before = fs::metadata(source.plan_path()).expect("recovered plan metadata");
        assert_eq!(
            store.load(journal.operation_id).expect("unchanged journal"),
            journal
        );
        assert_eq!(
            source.car_plan(&commitment).expect("reopen recovered plan"),
            MusubiSeedIngressCarPlanV1::from_car_build_plan(&plan, &commitment)
                .expect("canonical wire plan")
        );

        let retried = engine
            .recover_pre_ingress_sidecars(&journal, &request.publication, &commitment, &plan, &car)
            .expect("idempotently recover exact sidecars");
        assert!(same_file_snapshot(
            &car_before,
            &fs::metadata(retried.path()).expect("reused CAR metadata")
        ));
        assert!(same_file_snapshot(
            &plan_before,
            &fs::metadata(retried.plan_path()).expect("reused plan metadata")
        ));
        assert_eq!(
            fs::read(journal_path).expect("reread pristine journal"),
            journal_before
        );
    }

    #[cfg(unix)]
    #[test]
    fn pristine_pre_ingress_recovery_repairs_a_car_only_partial_install() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment.clone());
        let journal = store
            .create(request.clone())
            .expect("persist pristine recovery anchor");
        store
            .root
            .install_immutable(&staged_car_relative_path(journal.operation_id), &car)
            .expect("install exact CAR-only crash fixture");
        let source = PublicationStagedCarSourceV1::new(
            state.path(),
            journal.operation_id,
            commitment.car_size,
        );
        let car_before = fs::metadata(source.path()).expect("partial CAR metadata");
        assert!(!source.plan_path().exists());

        let repaired = engine
            .recover_pre_ingress_sidecars(&journal, &request.publication, &commitment, &plan, &car)
            .expect("repair missing plan sidecar");
        assert!(same_file_snapshot(
            &car_before,
            &fs::metadata(repaired.path()).expect("reused partial CAR metadata")
        ));
        assert!(repaired.plan_path().exists());
        assert_eq!(
            store.load(journal.operation_id).expect("unchanged journal"),
            journal
        );
    }

    #[cfg(unix)]
    #[test]
    fn pristine_pre_ingress_recovery_repairs_a_plan_only_partial_install() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment.clone());
        let journal = store
            .create(request.clone())
            .expect("persist pristine recovery anchor");
        let plan_bytes = MusubiSeedIngressCarPlanV1::from_car_build_plan(&plan, &commitment)
            .and_then(|plan| plan.canonical_bytes())
            .expect("canonical plan sidecar");
        store
            .root
            .install_immutable(
                &staged_plan_relative_path(journal.operation_id),
                &plan_bytes,
            )
            .expect("install exact plan-only crash fixture");
        let source = PublicationStagedCarSourceV1::new(
            state.path(),
            journal.operation_id,
            commitment.car_size,
        );
        let plan_before = fs::metadata(source.plan_path()).expect("partial plan metadata");
        assert!(!source.path().exists());

        let repaired = engine
            .recover_pre_ingress_sidecars(&journal, &request.publication, &commitment, &plan, &car)
            .expect("repair missing CAR sidecar");
        assert!(same_file_snapshot(
            &plan_before,
            &fs::metadata(repaired.plan_path()).expect("reused partial plan metadata")
        ));
        assert!(repaired.path().exists());
        assert_eq!(
            store.load(journal.operation_id).expect("unchanged journal"),
            journal
        );
    }

    #[cfg(unix)]
    #[test]
    fn pre_ingress_recovery_rejects_mismatch_stale_and_advanced_journals_before_install() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (plan, car, commitment) = publication_fixture_canonical_car();
        let (request, _) = request_with_archive_commitment(commitment.clone());
        let journal = store
            .create(request.clone())
            .expect("persist pristine recovery anchor");
        let source = PublicationStagedCarSourceV1::new(
            state.path(),
            journal.operation_id,
            commitment.car_size,
        );

        let mut substituted_publication = request.publication.clone();
        substituted_publication.manifest.interface_digest = MusubiContentDigestV1::new([0xA5; 32]);
        assert!(matches!(
            engine.recover_pre_ingress_sidecars(
                &journal,
                &substituted_publication,
                &commitment,
                &plan,
                &car,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        assert!(!source.path().exists());
        assert!(!source.plan_path().exists());

        let mut next = journal.clone();
        next.validation = Some(validation_evidence(&request));
        next.phase = PublicationPhaseV1::SeedIngress;
        let advanced = store
            .transition(&journal, next)
            .expect("advance fixture journal");
        assert!(matches!(
            engine.recover_pre_ingress_sidecars(
                &journal,
                &request.publication,
                &commitment,
                &plan,
                &car,
            ),
            Err(PublicationError::ConcurrentJournalUpdate)
        ));
        assert!(matches!(
            engine.recover_pre_ingress_sidecars(
                &advanced,
                &request.publication,
                &commitment,
                &plan,
                &car,
            ),
            Err(PublicationError::InvalidJournal(_))
        ));
        assert!(!source.path().exists());
        assert!(!source.plan_path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn validation_requires_the_exact_plan_before_calling_the_backend() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let engine = PublicationEngine::new(&store);
        let (_plan, car, commitment) = publication_fixture_canonical_car();
        let (request, broker) = request_with_archive_commitment(commitment.clone());
        let journal = store
            .create(request.clone())
            .expect("persist pristine recovery anchor");
        store
            .root
            .install_immutable(&staged_car_relative_path(journal.operation_id), &car)
            .expect("install exact CAR-only crash fixture");
        let source = PublicationStagedCarSourceV1::new(
            state.path(),
            journal.operation_id,
            commitment.car_size,
        );
        let mut backend = EarlyBackend {
            broker,
            fail_validation_once: true,
            substitute_receipt: false,
            now_ms: 1_500,
            receipt_window: None,
            prepare_calls: 0,
        };

        assert!(matches!(
            engine.advance_once(journal.operation_id, &source, &mut backend),
            Err(PublicationError::CarSource(_))
        ));
        assert!(
            backend.fail_validation_once,
            "backend validation was not called"
        );
        assert_eq!(
            store.load(journal.operation_id).expect("unchanged journal"),
            journal
        );
    }

    #[cfg(unix)]
    #[test]
    fn staged_plan_missing_corrupt_or_hard_linked_fails_closed() {
        let state = tempdir().expect("state root");
        let _store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let (plan, bytes, commitment) = publication_fixture_canonical_car();
        for (id_byte, mutation) in [(0x31, "missing"), (0x32, "corrupt"), (0x33, "linked")] {
            let operation_id = PublicationOperationIdV1::from_str(&hex::encode([id_byte; 32]))
                .expect("operation id");
            let source = PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                operation_id,
                &commitment,
                &plan,
                &bytes,
            )
            .expect("stage fixture");
            let linked = state.path().join(format!("{mutation}.plan"));
            match mutation {
                "missing" => fs::remove_file(source.plan_path()).expect("remove plan sidecar"),
                "corrupt" => {
                    let mut noncanonical =
                        fs::read(source.plan_path()).expect("read canonical sidecar");
                    noncanonical.push(0);
                    fs::write(source.plan_path(), noncanonical)
                        .expect("append trailing sidecar byte");
                }
                "linked" => fs::hard_link(source.plan_path(), linked).expect("hard-link sidecar"),
                _ => unreachable!("closed fixture mutation"),
            }
            assert!(source.car_plan(&commitment).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn staged_plan_substitution_fails_commitment_validation() {
        let state = tempdir().expect("state root");
        let _store = PublicationJournalStore::open(state.path()).expect("private journal store");
        let operation_id = "3434343434343434343434343434343434343434343434343434343434343434"
            .parse()
            .expect("operation id");
        let (expected_plan, bytes, expected_commitment) = publication_fixture_canonical_car();
        let source = PublicationStagedCarSourceV1::stage_bytes(
            state.path(),
            operation_id,
            &expected_commitment,
            &expected_plan,
            &bytes,
        )
        .expect("stage fixture");

        let mut substituted_plan = expected_plan.clone();
        let source_file = substituted_plan
            .files
            .iter_mut()
            .find(|file| file.path.iter().map(String::as_str).eq(["src", "lib.ko"]))
            .expect("fixture source file");
        source_file.path = vec!["src".to_owned(), "renamed.ko".to_owned()];
        substituted_plan
            .validate()
            .expect("substituted inventory remains structurally valid");
        let substituted_commitment = expected_commitment.clone();
        let substituted_wire = MusubiSeedIngressCarPlanV1::from_car_build_plan(
            &substituted_plan,
            &substituted_commitment,
        )
        .expect("substituted wire plan");
        assert!(matches!(
            PublicationStagedCarSourceV1::stage_bytes(
                state.path(),
                operation_id,
                &substituted_commitment,
                &substituted_plan,
                &bytes,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                ..
            })
        ));
        assert!(source.path().exists());
        assert!(source.plan_path().exists());
        fs::write(
            source.plan_path(),
            substituted_wire
                .canonical_bytes()
                .expect("encode substituted plan"),
        )
        .expect("substitute sidecar bytes");

        assert_eq!(
            source
                .car_plan(&expected_commitment)
                .expect_err("substituted plan must fail")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(unix)]
    #[test]
    fn journal_load_rejects_a_fifo_substitution_without_blocking() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, _) = request();
        let operation_id = request.operation_id();
        store.create(request).expect("create canonical journal");

        TEST_PUBLICATION_READ_FIFO_SUBSTITUTIONS.with(|remaining| remaining.set(1));
        assert!(matches!(
            store.load(operation_id),
            Err(PublicationError::InvalidJournal(_))
        ));
        let path = state.path().join(journal_relative_path(operation_id));
        assert!(
            fs::symlink_metadata(path)
                .expect("substituted FIFO metadata")
                .file_type()
                .is_fifo()
        );
        TEST_PUBLICATION_READ_FIFO_SUBSTITUTIONS.with(|remaining| assert_eq!(remaining.get(), 0));
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

    #[test]
    fn compact_release_envelope_reconstructs_exact_wire_and_detects_proof_substitution() {
        let (request, broker) = request();
        let (_, preparation) = release_preparation_fixture(&request, &broker);
        let signed = signed_release_transaction(&request, 1);
        let envelope =
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(&request, &signed)
                .expect("extract compact release envelope");
        let reconstructed = envelope
            .reconstruct_signed_transaction(&request)
            .expect("reconstruct compact release transaction");
        assert_eq!(reconstructed, signed);

        let wire = release_signed_transaction_wire_v1(&signed).expect("release V1 wire");
        assert_eq!(
            wire,
            signed
                .encode_wire_v1()
                .expect("data-model fixed V1 transaction wire")
        );

        let intent = PublicationReleaseSubmissionIntentV1::try_new(
            request.operation_id(),
            &request,
            preparation,
            &signed,
        )
        .expect("compact release intent");
        let encoded = norito::encode_canonical(&intent).expect("encode compact release intent");
        let decoded: PublicationReleaseSubmissionIntentV1 =
            norito::decode_canonical(&encoded).expect("decode compact release intent");
        assert_eq!(decoded, intent);
        assert_eq!(
            decoded
                .reconstruct_signed_transaction(request.operation_id(), &request)
                .expect("validate decoded compact intent"),
            signed
        );

        let other = signed_release_transaction(&request, 2);
        let forged = TransactionBuilder::from_payload(signed.payload().clone())
            .expect("valid original payload")
            .build_with_signature(other.signature().payload().clone());
        assert_eq!(forged.hash(), signed.hash());
        let forged_wire =
            release_signed_transaction_wire_v1(&forged).expect("forged release V1 wire");
        assert_ne!(forged_wire, wire);
        assert_ne!(
            domain_hash(RELEASE_SIGNED_TRANSACTION_DOMAIN, &forged_wire),
            intent.signed_transaction_digest
        );
        let mut substituted = intent;
        substituted.envelope.signature = forged.signature().clone();
        assert!(matches!(
            substituted.validate_for(request.operation_id(), &request),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                ..
            })
        ));
    }

    #[test]
    fn compact_release_envelope_distinguishes_valid_authorization_bundles() {
        let (mut request, _) = request();
        let signer_a = KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519)
            .expect("first multisig fixture key");
        let signer_b = KeyPair::try_from_seed(vec![0xB2; 32], Algorithm::Ed25519)
            .expect("second multisig fixture key");
        request.publisher = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![
                    MultisigMember::new(signer_a.public_key().clone(), 1)
                        .expect("first multisig member"),
                    MultisigMember::new(signer_b.public_key().clone(), 1)
                        .expect("second multisig member"),
                ],
            )
            .expect("one-of-two multisig policy"),
        );
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.publish_instruction()]);
        builder.set_creation_time(std::time::Duration::from_millis(2_500));
        builder.set_nonce(NonZeroU32::new(1).expect("fixture nonce"));
        let transaction_a = builder.clone().sign_multisig([signer_a.private_key()]);
        let transaction_b = builder.sign_multisig([signer_b.private_key()]);

        assert_eq!(transaction_a.payload(), transaction_b.payload());
        assert_eq!(transaction_a.hash(), transaction_b.hash());
        transaction_a.verify_signature().expect("first valid proof");
        transaction_b
            .verify_signature()
            .expect("second valid proof");
        let wire_a = release_signed_transaction_wire_v1(&transaction_a).expect("first exact wire");
        let wire_b = release_signed_transaction_wire_v1(&transaction_b).expect("second exact wire");
        assert_ne!(wire_a, wire_b);
        assert_ne!(
            domain_hash(RELEASE_SIGNED_TRANSACTION_DOMAIN, &wire_a),
            domain_hash(RELEASE_SIGNED_TRANSACTION_DOMAIN, &wire_b)
        );

        let envelope_a = PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
            &request,
            &transaction_a,
        )
        .expect("first compact authorization");
        let envelope_b = PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
            &request,
            &transaction_b,
        )
        .expect("second compact authorization");
        assert_ne!(envelope_a, envelope_b);
        assert_eq!(
            envelope_a
                .reconstruct_signed_transaction(&request)
                .expect("first reconstruction"),
            transaction_a
        );
        assert_eq!(
            envelope_b
                .reconstruct_signed_transaction(&request)
                .expect("second reconstruction"),
            transaction_b
        );
    }

    #[test]
    fn compact_release_envelope_rejects_omitted_and_noncanonical_payload_fields() {
        let (request, _) = request();
        let signed = signed_release_transaction(&request, 1);
        let (_, publisher_keypair) = account(20);

        let mut metadata_payload = signed.payload().clone();
        metadata_payload
            .metadata
            .insert("unexpected".parse().expect("metadata key"), "not allowed");
        let metadata_transaction = TransactionBuilder::from_payload(metadata_payload)
            .expect("metadata fixture payload")
            .sign(publisher_keypair.private_key());
        assert!(
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
                &request,
                &metadata_transaction
            )
            .is_err()
        );

        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "release-vk"),
        );
        let attachments =
            ProofAttachmentList::try_from(vec![attachment]).expect("bounded proof attachment");
        let attachment_transaction = TransactionBuilder::from_payload(signed.payload().clone())
            .expect("attachment fixture payload")
            .with_attachments(attachments)
            .sign(publisher_keypair.private_key());
        assert!(
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
                &request,
                &attachment_transaction
            )
            .is_err()
        );

        let mut overflow_payload = signed.payload().clone();
        overflow_payload.creation_time_ms = u64::MAX;
        overflow_payload.time_to_live_ms = NonZeroU64::new(1);
        let overflow_transaction = TransactionBuilder::from_payload(overflow_payload)
            .expect("overflow fixture payload")
            .sign(publisher_keypair.private_key());
        assert!(
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
                &request,
                &overflow_transaction
            )
            .is_err()
        );

        let mut wrong_network_payload = signed.payload().clone();
        wrong_network_payload.domain = iroha_data_model::transaction::TransactionDomain::Network(
            NetworkId::from_genesis_hash(
                iroha::crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                    iroha::crypto::Hash::prehashed([0xFF; 32]),
                ),
            ),
        );
        let wrong_network_transaction = TransactionBuilder::from_payload(wrong_network_payload)
            .expect("wrong-network fixture payload")
            .sign(publisher_keypair.private_key());
        assert!(
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
                &request,
                &wrong_network_transaction
            )
            .is_err()
        );
    }

    #[test]
    fn compact_release_envelope_preserves_maximum_canonical_multisig_bundle() {
        let (request, _) = request();
        let (request, signed) = maximum_multisig_release_transaction(request);
        assert_eq!(signed.signature_count(), MUSUBI_MAX_RELEASE_SIGNATURES_V1);
        let envelope =
            PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(&request, &signed)
                .expect("maximum canonical multisig envelope");
        assert_eq!(
            envelope
                .reconstruct_signed_transaction(&request)
                .expect("maximum multisig reconstruction"),
            signed
        );
        let mut reordered = envelope;
        reordered
            .multisig_signatures
            .as_mut()
            .expect("multisig bundle")
            .signatures
            .swap(0, 1);
        assert!(reordered.reconstruct_signed_transaction(&request).is_err());
    }

    #[test]
    fn compact_final_checkpoint_covers_the_maximum_admitted_release_signers() {
        let (request, _) = request();
        let (request, _) = maximum_multisig_release_transaction(request);
        let evidence = final_evidence(&request);
        let submission = PublicationAmxSubmissionV1::new(
            request.operation_id(),
            &request.publish_instruction(),
            [0xA5; 32],
            evidence.snapshot.finalized_height,
        );
        let checkpoint =
            PublicationFinalCheckpointV1::from_verified(&request, &submission, &evidence)
                .expect("compact verified final checkpoint");
        ensure_release_component_budget(
            &checkpoint,
            MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES,
            "maximum-admitted-signer final checkpoint",
            PublicationPhaseV1::FinalVerification,
        )
        .expect("maximum admitted release signers fit the compact checkpoint reserve");
        assert!(
            canonical_encoded_len(&checkpoint).expect("encode compact final checkpoint")
                < canonical_encoded_len(&evidence).expect("encode full final evidence")
        );
        assert_eq!(checkpoint.release, request.publication.manifest.release);
        assert_ne!(checkpoint.home_release_digest, [0; 32]);
        assert_ne!(checkpoint.universal_release_digest, [0; 32]);

        let encoded = norito::encode_canonical(&checkpoint).expect("encode final checkpoint");
        let decoded: PublicationFinalCheckpointV1 =
            norito::decode_canonical(&encoded).expect("decode final checkpoint");
        assert_eq!(decoded, checkpoint);
        assert_eq!(
            decoded.checkpoint_digest,
            decoded.digest().expect("checkpoint digest")
        );

        let mut different_operation = request.clone();
        different_operation.nonce[0] ^= 1;
        assert_ne!(different_operation.operation_id(), request.operation_id());
        assert!(
            checkpoint
                .validate_for(&different_operation, &submission)
                .is_err()
        );
        let mut substituted_submission = submission;
        substituted_submission.operation_id = different_operation.operation_id();
        assert!(
            checkpoint
                .validate_for(&request, &substituted_submission)
                .is_err()
        );

        let mut substituted = checkpoint;
        substituted.home_release_digest[0] ^= 1;
        assert!(substituted.validate_for(&request, &submission).is_err());
    }

    #[test]
    fn compact_final_checkpoint_accepts_later_paired_yank_and_storage_projection() {
        let (request, _) = request();
        let mut evidence = final_evidence(&request);
        let (changed_by, _) = account(0xD1);
        let yank = MusubiReleaseYankV1 {
            release: request.publication.manifest.release.clone(),
            yanked: true,
            reason: MusubiReasonV1::new("post-publication policy change").expect("reason"),
            changed_by,
            changed_at_height: 90,
            revision: 2,
        };
        evidence.home_release.yank = yank.clone();
        evidence.home_release.revisions.yank = yank.revision;
        evidence.universal_release.selection.yank = yank;
        let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([0xD3; 32]),
            reason: MusubiReasonV1::new("post-publication governed takedown").expect("reason"),
            applied_at_height: 91,
        });
        evidence.home_release.artifact_governance = governance.clone();
        evidence.home_release.revisions.artifact_governance = 2;
        evidence.universal_release.selection.governance = governance;
        evidence.universal_release.selection.storage.availability =
            MusubiStorageAvailabilityV1::BelowQuorum;
        evidence
            .universal_release
            .selection
            .storage
            .healthy_replicas = 1;
        assert!(!evidence.universal_release.selection.fresh_selectable());

        let submission = PublicationAmxSubmissionV1::new(
            request.operation_id(),
            &request.publish_instruction(),
            [0xD2; 32],
            81,
        );
        let checkpoint =
            PublicationFinalCheckpointV1::from_verified(&request, &submission, &evidence)
                .expect("later paired projections still prove the immutable release claim");
        checkpoint
            .validate_for(&request, &submission)
            .expect("compact checkpoint remains request-bound");
    }

    #[test]
    fn compact_final_checkpoint_decouples_near_limit_governance_account() {
        let (request, _) = request();
        let submission = PublicationAmxSubmissionV1::new(
            request.operation_id(),
            &request.publish_instruction(),
            [0xD4; 32],
            81,
        );
        let ordinary_evidence = final_evidence(&request);
        let ordinary_checkpoint =
            PublicationFinalCheckpointV1::from_verified(&request, &submission, &ordinary_evidence)
                .expect("ordinary compact checkpoint");

        let changed_by = maximum_legal_musubi_account();
        let changed_by_size = norito::to_bytes(&changed_by)
            .expect("near-limit account has canonical Norito bytes")
            .len();
        assert!(changed_by_size <= MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1);
        assert!(changed_by_size > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 - 256);
        validate_musubi_account_id_v1(&changed_by).expect("near-limit account is legal in Musubi");

        let mut large_evidence = ordinary_evidence.clone();
        let yank = MusubiReleaseYankV1 {
            release: request.publication.manifest.release.clone(),
            yanked: true,
            reason: MusubiReasonV1::new("post-publication owner change").expect("reason"),
            changed_by,
            changed_at_height: 90,
            revision: 2,
        };
        large_evidence.home_release.yank = yank.clone();
        large_evidence.home_release.revisions.yank = yank.revision;
        large_evidence.universal_release.selection.yank = yank;

        let large_checkpoint =
            PublicationFinalCheckpointV1::from_verified(&request, &submission, &large_evidence)
                .expect("near-limit governance projection compacts");
        let ordinary_evidence_size =
            canonical_encoded_len(&ordinary_evidence).expect("ordinary final evidence size");
        let large_evidence_size =
            canonical_encoded_len(&large_evidence).expect("large final evidence size");
        let ordinary_checkpoint_size =
            canonical_encoded_len(&ordinary_checkpoint).expect("ordinary final checkpoint size");
        let large_checkpoint_size =
            canonical_encoded_len(&large_checkpoint).expect("large final checkpoint size");

        assert!(large_evidence_size > ordinary_evidence_size + changed_by_size);
        assert_eq!(large_checkpoint_size, ordinary_checkpoint_size);
        assert!(large_checkpoint_size <= MAX_RELEASE_FINAL_CHECKPOINT_CANONICAL_BYTES);
        assert_ne!(
            large_checkpoint.home_release_digest,
            ordinary_checkpoint.home_release_digest
        );
        assert_ne!(
            large_checkpoint.universal_release_digest,
            ordinary_checkpoint.universal_release_digest
        );
    }

    #[test]
    fn release_component_canonical_budget_accepts_boundary_and_rejects_plus_one() {
        fn bytes_with_canonical_size(target: usize) -> Vec<u8> {
            let mut lower = 0_usize;
            let mut upper = target;
            while lower <= upper {
                let length = lower + (upper - lower) / 2;
                let value = vec![0_u8; length];
                match norito::encode_canonical(&value)
                    .expect("encode boundary fixture")
                    .len()
                    .cmp(&target)
                {
                    std::cmp::Ordering::Less => lower = length + 1,
                    std::cmp::Ordering::Equal => return value,
                    std::cmp::Ordering::Greater => {
                        let Some(next) = length.checked_sub(1) else {
                            break;
                        };
                        upper = next;
                    }
                }
            }
            panic!("canonical byte-vector encoding could not represent exact size {target}");
        }

        let at_limit = bytes_with_canonical_size(MAX_RELEASE_INTENT_CANONICAL_BYTES);
        ensure_release_component_budget(
            &at_limit,
            MAX_RELEASE_INTENT_CANONICAL_BYTES,
            "boundary fixture",
            PublicationPhaseV1::ReleaseSubmission,
        )
        .expect("exact canonical boundary is admitted");

        let above_limit = bytes_with_canonical_size(MAX_RELEASE_INTENT_CANONICAL_BYTES + 1);
        assert!(matches!(
            ensure_release_component_budget(
                &above_limit,
                MAX_RELEASE_INTENT_CANONICAL_BYTES,
                "boundary fixture",
                PublicationPhaseV1::ReleaseSubmission,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ReleaseSubmission,
                ..
            })
        ));
        assert!(matches!(
            ensure_release_component_budget(
                &[0_u8],
                0,
                "final verification fixture",
                PublicationPhaseV1::FinalVerification,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::FinalVerification,
                ..
            })
        ));
    }

    #[test]
    fn release_absence_requires_exact_empty_same_snapshot_retention_evidence() {
        let (request, broker) = request();
        let (_, floor) = release_preparation_fixture(&request, &broker);
        let signed = signed_release_transaction(&request, 1);
        let intent = PublicationReleaseSubmissionIntentV1::try_new(
            request.operation_id(),
            &request,
            floor,
            &signed,
        )
        .expect("release intent");
        let deadline = release_submission_valid_until_ms(&intent).expect("release deadline");
        let absence = release_absence_evidence(&request, 80, deadline + 1);
        absence
            .validate_for(&request)
            .expect("exact synchronized absence");
        PublicationReleaseSubmissionTerminalV1::finalized_validity_window_elapsed(
            &intent,
            absence.clone(),
        )
        .validate_for(&request, &intent)
        .expect("consensus-time terminal proof");

        let mut unknown = absence.clone();
        unknown.retention_page.items[0].disposition =
            MusubiArchiveRetentionDispositionV1::RetainUnknown;
        unknown.retention_page.items[0].storage = None;
        assert!(unknown.validate_for(&request).is_err());

        let mut wrong_snapshot = absence.clone();
        wrong_snapshot.retention_page.snapshot.finalized_height += 1;
        wrong_snapshot.retention_page.snapshot.finalized_block_hash = [0xD2; 32];
        assert!(wrong_snapshot.validate_for(&request).is_err());

        let mut future_storage = absence.clone();
        future_storage.retention_page.items[0]
            .storage
            .as_mut()
            .expect("known archive storage")
            .finalized_height = future_storage.retention_page.snapshot.finalized_height + 1;
        assert!(future_storage.validate_for(&request).is_err());

        let mut non_exact = absence.clone();
        non_exact.resolver_page.query.requirement = Some("*".parse().expect("wildcard"));
        assert!(non_exact.validate_for(&request).is_err());

        let at_deadline = release_absence_evidence(&request, 80, deadline);
        assert!(
            PublicationReleaseSubmissionTerminalV1::finalized_validity_window_elapsed(
                &intent,
                at_deadline,
            )
            .validate_for(&request, &intent)
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test verifies every boundary and append-only mutation of the bounded release journal"
    )]
    fn release_attempt_journal_is_append_only_bounded_and_durable() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let mut journal = release_ready_journal(&request, &broker);
        let mut attempts = Vec::new();
        for generation in 1..=MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1 {
            let offset = u64::try_from((generation - 1) * 3).expect("fixture offset");
            let registration = registration(&request, &broker);
            let replication =
                replication_checkpoint_with_journal_max_shape(&request, &registration, offset);
            let floor = release_preparation_for_registration(&request, &registration, replication);
            let signed = signed_release_transaction(
                &request,
                u32::try_from(generation).expect("fixture nonce"),
            );
            let intent = PublicationReleaseSubmissionIntentV1::try_new(
                operation_id,
                &request,
                floor,
                &signed,
            )
            .expect("bounded release intent");
            let mut attempt = PublicationReleaseSubmissionAttemptV1::new(
                u8::try_from(generation).expect("fixture generation"),
                intent,
            );
            if generation < MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1 {
                let preparation_height = attempt
                    .intent
                    .preparation
                    .replication
                    .finalized_page
                    .snapshot
                    .finalized_height;
                let absence = release_absence_evidence(
                    &request,
                    preparation_height + 1,
                    release_submission_valid_until_ms(&attempt.intent).expect("release deadline")
                        + 1,
                );
                attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::Terminal(
                    PublicationReleaseSubmissionTerminalV1::registry_expired(
                        &attempt.intent,
                        preparation_height + 1,
                        absence,
                    ),
                ));
            }
            attempts.push(attempt);
        }
        let live_floor = attempts
            .last()
            .expect("live bounded attempt")
            .intent
            .preparation
            .clone();
        journal.replication = Some(live_floor.replication);
        journal.readbacks = live_floor.readbacks;
        journal.release_submission_attempts = attempts;
        journal.validate().expect("maximum legal release history");
        let encoded = norito::encode_canonical(&journal).expect("encode maximum release history");
        let (maximum_authority_request, maximum_signed) =
            maximum_multisig_release_transaction(request.clone());
        let maximum_envelope = PublicationReleaseSignedEnvelopeV1::try_from_signed_transaction(
            &maximum_authority_request,
            &maximum_signed,
        )
        .expect("maximum authorization envelope");
        let maximum_envelope_size =
            canonical_encoded_len(&maximum_envelope).expect("encode maximum envelope");
        let conservative_authorization_projection = encoded
            .len()
            .checked_add(
                maximum_envelope_size
                    .checked_mul(MUSUBI_MAX_RELEASE_SUBMISSION_ATTEMPTS_V1)
                    .expect("bounded envelope projection"),
            )
            .expect("bounded journal projection");
        assert!(conservative_authorization_projection <= MAX_JOURNAL_BYTES_USIZE);
        assert!(encoded.len() <= MAX_JOURNAL_BYTES_USIZE);
        store
            .write(&journal)
            .expect("persist maximum release history");
        let persisted_len = fs::metadata(state.path().join(journal_relative_path(operation_id)))
            .expect("maximum release journal metadata")
            .len();
        assert_eq!(
            persisted_len,
            u64::try_from(encoded.len()).expect("length fits u64")
        );
        assert!(persisted_len <= MAX_JOURNAL_BYTES);
        assert_eq!(
            store.load(operation_id).expect("reload release history"),
            journal
        );

        let mut completed = journal.clone();
        let last_attempt = completed
            .release_submission_attempts
            .last_mut()
            .expect("eighth live release attempt");
        let applied_height = last_attempt
            .intent
            .preparation
            .replication
            .finalized_page
            .snapshot
            .finalized_height
            + 1;
        let submission = PublicationAmxSubmissionV1::new(
            operation_id,
            &request.publish_instruction(),
            last_attempt.intent.transaction_hash,
            applied_height,
        );
        last_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::applied(
            &last_attempt.intent,
            submission,
        ));
        completed.phase = PublicationPhaseV1::FinalVerification;
        completed.submission = Some(submission);
        completed.completion = Some(
            PublicationFinalCheckpointV1::from_verified(
                &request,
                &submission,
                &final_evidence(&request),
            )
            .expect("compact exact final checkpoint"),
        );
        let completed = store
            .transition(&journal, completed)
            .expect("persist applied outcome and compact final checkpoint with maximum history");
        let completed_len = fs::metadata(state.path().join(journal_relative_path(operation_id)))
            .expect("completed maximum release journal metadata")
            .len();
        assert!(completed_len <= MAX_JOURNAL_BYTES);
        assert_eq!(
            store
                .load(operation_id)
                .expect("reload completed maximum release history"),
            completed
        );
        let mut rewritten_completion = completed.clone();
        let rewritten_checkpoint = rewritten_completion
            .completion
            .as_mut()
            .expect("completed journal checkpoint");
        rewritten_checkpoint.home_release_digest[0] ^= 1;
        rewritten_checkpoint.checkpoint_digest = rewritten_checkpoint
            .digest()
            .expect("alternate checkpoint digest");
        rewritten_checkpoint
            .validate_for(&request, &submission)
            .expect("self-consistent alternate checkpoint shape");
        assert!(matches!(
            store.transition(&completed, rewritten_completion),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("compact final checkpoint is not append-only")
        ));

        let mut immutable_rewrite = journal.clone();
        let first_terminal = match immutable_rewrite.release_submission_attempts[0]
            .outcome
            .as_mut()
            .expect("first terminal outcome")
        {
            PublicationReleaseSubmissionOutcomeV1::Terminal(terminal) => terminal,
            PublicationReleaseSubmissionOutcomeV1::Applied { .. } => {
                panic!("first outcome must be terminal")
            }
        };
        first_terminal.signed_transaction_digest[0] ^= 1;
        assert!(!release_submission_attempts_are_append_only(
            &journal.release_submission_attempts,
            &immutable_rewrite.release_submission_attempts,
        ));

        let mut ninth = journal;
        let last = ninth
            .release_submission_attempts
            .last_mut()
            .expect("eighth release attempt");
        let preparation_height = last
            .intent
            .preparation
            .replication
            .finalized_page
            .snapshot
            .finalized_height;
        last.outcome = Some(PublicationReleaseSubmissionOutcomeV1::Terminal(
            PublicationReleaseSubmissionTerminalV1::registry_expired(
                &last.intent,
                preparation_height + 1,
                release_absence_evidence(
                    &request,
                    preparation_height + 1,
                    release_submission_valid_until_ms(&last.intent).expect("release deadline") + 1,
                ),
            ),
        ));
        let (_, ninth_floor) = release_preparation_fixture_with_offset(&request, &broker, 24);
        let ninth_signed = signed_release_transaction(&request, 9);
        let ninth_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            ninth_floor.clone(),
            &ninth_signed,
        )
        .expect("ninth release intent shape");
        ninth
            .release_submission_attempts
            .push(PublicationReleaseSubmissionAttemptV1::new(9, ninth_intent));
        ninth.replication = Some(ninth_floor.replication);
        ninth.readbacks = ninth_floor.readbacks;
        assert!(matches!(
            ninth.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("release-submission attempt bound")
        ));
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test exercises each forbidden direct release-attempt transition in one coherent state history"
    )]
    fn release_attempt_transition_persists_live_intent_before_any_outcome() {
        let state = tempdir().expect("state root");
        let store = PublicationJournalStore::open(state.path()).expect("journal store");
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let mut previous = release_ready_journal(&request, &broker);
        let mut illegal_empty_submission = previous.clone();
        illegal_empty_submission.release_submission_attempts.clear();
        assert!(matches!(
            illegal_empty_submission.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("persist-before-send exact intent")
        ));
        previous.phase = PublicationPhaseV1::Readback;
        previous.readbacks.clear();
        previous.release_submission_attempts.clear();
        previous.validate().expect("pre-intent readback journal");
        store
            .write(&previous)
            .expect("persist release-ready journal");

        let (_, floor) = release_preparation_fixture(&request, &broker);
        let signed = signed_release_transaction(&request, 1);
        let intent =
            PublicationReleaseSubmissionIntentV1::try_new(operation_id, &request, floor, &signed)
                .expect("first release intent");
        let submission = PublicationAmxSubmissionV1::new(
            operation_id,
            &request.publish_instruction(),
            intent.transaction_hash,
            80,
        );

        let mut direct_applied = previous.clone();
        let mut applied_attempt = PublicationReleaseSubmissionAttemptV1::new(1, intent.clone());
        applied_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::applied(
            &applied_attempt.intent,
            submission,
        ));
        direct_applied.phase = PublicationPhaseV1::FinalVerification;
        direct_applied.readbacks = intent.preparation.readbacks.clone();
        direct_applied.release_submission_attempts = vec![applied_attempt];
        direct_applied.submission = Some(submission);
        assert!(matches!(
            store.transition(&previous, direct_applied),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("release-submission attempt history is not append-only")
        ));

        let preparation_height = intent
            .preparation
            .replication
            .finalized_page
            .snapshot
            .finalized_height;
        let absence = release_absence_evidence(
            &request,
            preparation_height + 1,
            release_submission_valid_until_ms(&intent).expect("release deadline") + 1,
        );
        let terminal = PublicationReleaseSubmissionTerminalV1::registry_expired(
            &intent,
            preparation_height + 1,
            absence,
        );
        let mut direct_terminal = previous.clone();
        direct_terminal.phase = PublicationPhaseV1::ReleaseSubmission;
        direct_terminal.readbacks = intent.preparation.readbacks.clone();
        let mut terminal_attempt = PublicationReleaseSubmissionAttemptV1::new(1, intent.clone());
        terminal_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::Terminal(
            terminal.clone(),
        ));
        direct_terminal.release_submission_attempts = vec![terminal_attempt];
        assert!(matches!(
            store.transition(&previous, direct_terminal),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("release-submission attempt history is not append-only")
        ));

        let mut live = previous.clone();
        live.phase = PublicationPhaseV1::ReleaseSubmission;
        live.readbacks = intent.preparation.readbacks.clone();
        live.release_submission_attempts =
            vec![PublicationReleaseSubmissionAttemptV1::new(1, intent)];
        let live = store
            .transition(&previous, live)
            .expect("persist first live intent");
        let mut terminal_history = live.clone();
        terminal_history.release_submission_attempts[0].outcome =
            Some(PublicationReleaseSubmissionOutcomeV1::Terminal(terminal));
        let terminal_history = store
            .transition(&live, terminal_history)
            .expect("append terminal outcome separately");

        let (_, refreshed_floor) = release_preparation_fixture_with_offset(&request, &broker, 1);
        let refreshed_signed = signed_release_transaction(&request, 2);
        let refreshed_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            refreshed_floor,
            &refreshed_signed,
        )
        .expect("second release intent");
        let mut direct_successor_outcome = terminal_history.clone();
        let mut second_attempt = PublicationReleaseSubmissionAttemptV1::new(2, refreshed_intent);
        second_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::Terminal(
            PublicationReleaseSubmissionTerminalV1::registry_expired(
                &second_attempt.intent,
                second_attempt
                    .intent
                    .preparation
                    .replication
                    .finalized_page
                    .snapshot
                    .finalized_height
                    + 1,
                release_absence_evidence(
                    &request,
                    second_attempt
                        .intent
                        .preparation
                        .replication
                        .finalized_page
                        .snapshot
                        .finalized_height
                        + 1,
                    release_submission_valid_until_ms(&second_attempt.intent)
                        .expect("second deadline")
                        + 1,
                ),
            ),
        ));
        direct_successor_outcome
            .release_submission_attempts
            .push(second_attempt);
        assert!(matches!(
            store.transition(&terminal_history, direct_successor_outcome),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("release-submission attempt history is not append-only")
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test keeps applied binding and rejected-successor invariants in one exact release history"
    )]
    fn release_attempt_applied_binding_and_rejected_successor_are_exact() {
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let mut applied_journal = release_ready_journal(&request, &broker);
        let (_, applied_floor) = release_preparation_fixture(&request, &broker);
        let applied_signed = signed_release_transaction(&request, 1);
        let applied_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            applied_floor,
            &applied_signed,
        )
        .expect("applied release intent");
        let submission = PublicationAmxSubmissionV1::new(
            operation_id,
            &request.publish_instruction(),
            applied_intent.transaction_hash,
            80,
        );
        let mut applied_attempt = PublicationReleaseSubmissionAttemptV1::new(1, applied_intent);
        applied_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::applied(
            &applied_attempt.intent,
            submission,
        ));
        applied_journal.phase = PublicationPhaseV1::FinalVerification;
        applied_journal.release_submission_attempts = vec![applied_attempt];
        applied_journal.submission = Some(submission);
        applied_journal.validate().expect("exact applied binding");
        applied_journal
            .submission
            .as_mut()
            .expect("submission")
            .transaction_hash[0] ^= 1;
        assert!(applied_journal.validate().is_err());

        let mut successor_journal = release_ready_journal(&request, &broker);
        let (first_registration, first_floor) = release_preparation_fixture(&request, &broker);
        let first_signed = signed_release_transaction(&request, 1);
        let first_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            first_floor.clone(),
            &first_signed,
        )
        .expect("first rejected intent");
        let (_, refreshed_floor) = release_preparation_fixture_with_offset(&request, &broker, 1);
        let covering_snapshot = refreshed_floor.replication.finalized_page.snapshot;
        let mut rejection_absence = release_absence_evidence(
            &request,
            covering_snapshot.finalized_height,
            release_submission_valid_until_ms(&first_intent).expect("deadline") + 1,
        );
        rejection_absence.resolver_page.snapshot = covering_snapshot;
        rejection_absence.retention_query.expected_snapshot = Some(covering_snapshot);
        rejection_absence.retention_page.snapshot = covering_snapshot;
        let mut first_attempt = PublicationReleaseSubmissionAttemptV1::new(1, first_intent);
        first_attempt.outcome = Some(PublicationReleaseSubmissionOutcomeV1::Terminal(
            PublicationReleaseSubmissionTerminalV1::registry_rejected(
                &first_attempt.intent,
                covering_snapshot.finalized_height,
                rejection_absence,
            ),
        ));
        let second_signed = signed_release_transaction(&request, 2);
        let second_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            refreshed_floor.clone(),
            &second_signed,
        )
        .expect("refreshed release intent");
        successor_journal.release_submission_attempts = vec![
            first_attempt.clone(),
            PublicationReleaseSubmissionAttemptV1::new(2, second_intent),
        ];
        successor_journal.replication = Some(refreshed_floor.replication);
        successor_journal.readbacks = refreshed_floor.readbacks;
        successor_journal
            .validate()
            .expect("higher same-location revision permits rejected successor");

        let mut covering_replication =
            replication_checkpoint_with_directory_advance(&request, &first_registration);
        covering_replication.finalized_page.snapshot = covering_snapshot;
        let covering_floor = release_preparation_for_registration(
            &request,
            &first_registration,
            covering_replication,
        );
        let unchanged_signed = signed_release_transaction(&request, 2);
        let unchanged_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            covering_floor.clone(),
            &unchanged_signed,
        )
        .expect("covering unchanged-location successor shape");
        successor_journal.release_submission_attempts = vec![
            first_attempt.clone(),
            PublicationReleaseSubmissionAttemptV1::new(2, unchanged_intent),
        ];
        successor_journal.replication = Some(covering_floor.replication);
        successor_journal.readbacks = covering_floor.readbacks;
        assert!(matches!(
            successor_journal.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("did not refresh or replace its location")
        ));

        let mut replacement_journal = release_ready_journal(&request, &broker);
        let registered = replacement_journal
            .registered_archive
            .as_ref()
            .expect("registered archive")
            .clone();
        let retirement = retired_location_terminal(&first_registration);
        assert!(
            retirement.finalized_page.snapshot.finalized_height
                >= covering_snapshot.finalized_height
        );
        let mut second_registration =
            location_registration_generation(operation_id, &request, &registered, 2);
        second_registration.intent.prepared_page = retirement.finalized_page.clone();
        second_registration =
            finalized_location_registration(&request, &second_registration.intent);
        let second_replication = replication_checkpoint(&request, &second_registration, 3);
        let second_floor = release_preparation_for_registration(
            &request,
            &second_registration,
            second_replication,
        );
        let replacement_signed = signed_release_transaction(&request, 2);
        let replacement_intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            &request,
            second_floor.clone(),
            &replacement_signed,
        )
        .expect("replacement-location release intent");
        let mut retired_first_location = replacement_journal.archive_location_attempts[0].clone();
        retired_first_location.terminal = Some(retirement);
        retired_first_location.terminal_floor = Some(
            PublicationArchiveLocationTerminalFloorV1::Replication(first_floor.replication.clone()),
        );
        replacement_journal.archive_location_attempts = vec![
            retired_first_location,
            PublicationArchiveLocationAttemptV1 {
                generation: 2,
                intent: second_registration.intent.clone(),
                registration: Some(second_registration),
                terminal: None,
                terminal_floor: None,
            },
        ];
        replacement_journal.release_submission_attempts = vec![
            first_attempt,
            PublicationReleaseSubmissionAttemptV1::new(2, replacement_intent),
        ];
        replacement_journal.replication = Some(second_floor.replication);
        replacement_journal.readbacks = second_floor.readbacks;
        replacement_journal
            .validate()
            .expect("retirement covering rejection permits a later location generation");
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
        let workers = [(), ()].map(|()| {
            let barrier = Arc::clone(&barrier);
            let root = root.clone();
            let previous = previous.clone();
            std::thread::spawn(move || {
                let store = PublicationJournalStore::open(&root).expect("worker journal store");
                barrier.wait();
                store.transition(&previous, previous.clone())
            })
        });
        let results = workers.map(|worker| worker.join().expect("transition worker"));
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
        now_ms: u64,
        receipt_window: Option<(u64, u64)>,
        prepare_calls: usize,
    }

    #[allow(
        clippy::struct_excessive_bools,
        reason = "independent fault-injection switches make each publication phase explicit in tests"
    )]
    struct CompleteBackend {
        broker: KeyPair,
        replication_pending_once: bool,
        finality_pending_once: bool,
        substitute_readback: bool,
        substitute_all_readbacks: bool,
        readback_backend_failure: Option<(ProviderId, PublicationBackendError)>,
        readback_providers: Vec<ProviderId>,
        submissions: usize,
    }

    struct ArchiveRecoveryBackend {
        broker: KeyPair,
        now_ms: u64,
        staged_receipts: Vec<MusubiSeedIngressReceiptV1>,
        prepare_calls: usize,
        registration_calls: usize,
        pin_calls: usize,
        archive_committed: bool,
        drop_commit_response_once: bool,
        return_conflicting_archive: bool,
        registration_mode: ArchiveRecoveryMode,
    }

    #[allow(
        clippy::struct_excessive_bools,
        reason = "independent crash and rejection switches model distinct recovery cuts in tests"
    )]
    struct LocationRecoveryBackend {
        broker: KeyPair,
        replication_script: VecDeque<LocationPollV1>,
        prepared_generations: Vec<(u8, Vec<MusubiArchiveLocationIdV1>)>,
        applied_generations: Vec<u8>,
        drop_location_response_once: bool,
        reject_release: bool,
        release_preparations: usize,
        release_submissions: usize,
        release_intents: Vec<[u8; 32]>,
        release_pending_responses: usize,
        drop_release_response_once: bool,
        release_applied: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ArchiveRecoveryMode {
        Commit,
        Pending,
        ExpiredAbsent,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LocationPollV1 {
        Healthy,
        HealthyRevisionOffset(u64),
        HealthyDirectoryAdvance,
        Retired,
        RetiredRevisionOffset(u64),
    }

    impl EarlyBackend {
        fn unsupported() -> PublicationBackendError {
            PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
        }
    }

    fn validate_seed_stage_fixture(
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
    ) -> Result<(), PublicationBackendError> {
        if expected.archive_id != commitment.archive_id()
            || expected.car_body_digest != commitment.car_digest
            || expected.car_body_length != commitment.car_size
        {
            return Err(PublicationBackendError::permanent(
                "TEST_SEED_COMMITMENT_INVALID",
            ));
        }
        plan.validate(commitment)
            .map_err(|_| PublicationBackendError::permanent("TEST_SEED_PLAN_INVALID"))
    }

    impl PublicationBackend for EarlyBackend {
        fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
            Ok(self.now_ms)
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
            commitment: &MusubiArchiveCommitmentV1,
            plan: &MusubiSeedIngressCarPlanV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            validate_seed_stage_fixture(expected, commitment, plan)?;
            let mut receipt = self.receipt_window.map_or_else(
                || signed_receipt(expected, &self.broker),
                |(issued_at_ms, expires_at_ms)| {
                    signed_receipt_at(expected, &self.broker, issued_at_ms, expires_at_ms)
                },
            );
            if self.substitute_receipt {
                receipt.payload.binding.archive_id = ArchiveId::new([0xEE; 32]);
            }
            Ok(receipt)
        }

        fn prepare_archive_registration_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
            self.prepare_calls += 1;
            Ok(registration_intent(operation_id, request, receipt.clone()))
        }

        fn submit_or_recover_archive_registration(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _intent: &PublicationArchiveRegistrationIntentV1,
        ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn prepare_archive_location_intent(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            _generation: u8,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn submit_or_recover_archive_location(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            _intent: &PublicationArchiveLocationIntentV1,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _registration: &PublicationArchiveRegistrationV1,
        ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
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

        fn prepare_release_submission_intent(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _preparation: &PublicationReleasePreparationFloorV1,
        ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn submit_or_recover_release_submission(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _intent: &PublicationReleaseSubmissionIntentV1,
            _allow_absent_submission: bool,
        ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
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
            commitment: &MusubiArchiveCommitmentV1,
            plan: &MusubiSeedIngressCarPlanV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            validate_seed_stage_fixture(expected, commitment, plan)?;
            Ok(signed_receipt(expected, &self.broker))
        }

        fn prepare_archive_registration_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
            Ok(registration_intent(operation_id, request, receipt.clone()))
        }

        fn submit_or_recover_archive_registration(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            intent: &PublicationArchiveRegistrationIntentV1,
        ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
            Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                registered_archive(request, &self.broker, intent),
            ))
        }

        fn prepare_archive_location_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            generation: u8,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
            let mut result = registration(request, &self.broker).intent;
            result.operation_id = operation_id;
            result.generation = generation;
            Ok(result)
        }

        fn submit_or_recover_archive_location(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            intent: &PublicationArchiveLocationIntentV1,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
            let mut result = registration(request, &self.broker);
            result.intent = intent.clone();
            Ok(PublicationArchiveLocationAdvanceV1::Registered(result))
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registration: &PublicationArchiveRegistrationV1,
        ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
            if self.replication_pending_once {
                self.replication_pending_once = false;
                return Ok(PublicationReplicationAdvanceV1::Pending);
            }
            Ok(PublicationReplicationAdvanceV1::Healthy(
                replication_checkpoint(request, registration, 3),
            ))
        }

        fn readback_provider(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            location: &MusubiArchiveLocationV1,
            provider: ProviderId,
        ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
            self.readback_providers.push(provider);
            if let Some((failed_provider, error)) = &self.readback_backend_failure
                && *failed_provider == provider
            {
                return Err(error.clone());
            }
            let mut evidence = PublicationReadbackEvidenceV1 {
                provider,
                location_id: location.location_id,
                replication_order: location.replication_order,
                commitment: request.archive_commitment.clone(),
                semantic_release_digest: request.publication.manifest.semantic_digest(),
                verification_lock_digest: request.publication.manifest.verification_lock_digest,
            };
            if self.substitute_all_readbacks
                || (self.substitute_readback && provider == location.providers[0])
            {
                evidence.commitment.car_digest = MusubiContentDigestV1::new([0xEE; 32]);
            }
            Ok(evidence)
        }

        fn prepare_release_submission_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            preparation: &PublicationReleasePreparationFloorV1,
        ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
            let nonce =
                u32::try_from(self.submissions + 1).expect("test submission count fits u32");
            PublicationReleaseSubmissionIntentV1::try_new(
                operation_id,
                request,
                preparation.clone(),
                &signed_release_transaction(request, nonce),
            )
            .map_err(|_| PublicationBackendError::permanent("TEST_RELEASE_INTENT_INVALID"))
        }

        fn submit_or_recover_release_submission(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            intent: &PublicationReleaseSubmissionIntentV1,
            allow_absent_submission: bool,
        ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
            if !allow_absent_submission {
                return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
            }
            self.submissions += 1;
            Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
                PublicationAmxSubmissionV1::new(
                    operation_id,
                    &request.publish_instruction(),
                    intent.transaction_hash,
                    80,
                ),
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

    impl ArchiveRecoveryBackend {
        fn unsupported() -> PublicationBackendError {
            PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
        }
    }

    impl PublicationBackend for ArchiveRecoveryBackend {
        fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
            Ok(self.now_ms)
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
            commitment: &MusubiArchiveCommitmentV1,
            plan: &MusubiSeedIngressCarPlanV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            validate_seed_stage_fixture(expected, commitment, plan)?;
            let receipt = signed_receipt_at(expected, &self.broker, self.now_ms, self.now_ms + 100);
            self.staged_receipts.push(receipt.clone());
            Ok(receipt)
        }

        fn prepare_archive_registration_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
            self.prepare_calls += 1;
            Ok(registration_intent(operation_id, request, receipt.clone()))
        }

        fn submit_or_recover_archive_registration(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            intent: &PublicationArchiveRegistrationIntentV1,
        ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
            self.registration_calls += 1;
            match self.registration_mode {
                ArchiveRecoveryMode::Pending => {
                    return Ok(PublicationArchiveRegistrationAdvanceV1::Pending);
                }
                ArchiveRecoveryMode::ExpiredAbsent => {
                    self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
                    return Ok(PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(
                        PublicationArchiveRegistrationTerminalV1::registry_expired(
                            intent,
                            Some(60),
                            archive_absence_evidence(request, 60),
                        ),
                    ));
                }
                ArchiveRecoveryMode::Commit => {}
            }
            if !self.archive_committed {
                self.archive_committed = true;
                self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
                if self.drop_commit_response_once {
                    self.drop_commit_response_once = false;
                    return Err(PublicationBackendError::retryable(
                        "ARCHIVE_COMMIT_RESPONSE_DROPPED",
                    ));
                }
            }
            let mut recovered = registered_archive(request, &self.broker, intent);
            if self.return_conflicting_archive {
                recovered.archive.registered_by = account(99).0;
            }
            Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                recovered,
            ))
        }

        fn prepare_archive_location_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registered: &PublicationRegisteredArchiveV1,
            generation: u8,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
            self.pin_calls += 1;
            Ok(
                location_registration_generation(operation_id, request, registered, generation)
                    .intent,
            )
        }

        fn submit_or_recover_archive_location(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            intent: &PublicationArchiveLocationIntentV1,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
            Ok(PublicationArchiveLocationAdvanceV1::Registered(
                finalized_location_registration(request, intent),
            ))
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _registration: &PublicationArchiveRegistrationV1,
        ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
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

        fn prepare_release_submission_intent(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _preparation: &PublicationReleasePreparationFloorV1,
        ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
            Err(Self::unsupported())
        }

        fn submit_or_recover_release_submission(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            _request: &PublicationRequestV1,
            _intent: &PublicationReleaseSubmissionIntentV1,
            _allow_absent_submission: bool,
        ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
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

    impl LocationRecoveryBackend {
        fn new(
            broker: KeyPair,
            replication_script: impl IntoIterator<Item = LocationPollV1>,
        ) -> Self {
            Self {
                broker,
                replication_script: replication_script.into_iter().collect(),
                prepared_generations: Vec::new(),
                applied_generations: Vec::new(),
                drop_location_response_once: false,
                reject_release: false,
                release_preparations: 0,
                release_submissions: 0,
                release_intents: Vec::new(),
                release_pending_responses: 0,
                drop_release_response_once: false,
                release_applied: false,
            }
        }
    }

    impl PublicationBackend for LocationRecoveryBackend {
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
            commitment: &MusubiArchiveCommitmentV1,
            plan: &MusubiSeedIngressCarPlanV1,
            _car: &mut dyn Read,
        ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
            validate_seed_stage_fixture(expected, commitment, plan)?;
            Ok(signed_receipt(expected, &self.broker))
        }

        fn prepare_archive_registration_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            receipt: &MusubiSeedIngressReceiptV1,
        ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
            Ok(registration_intent(operation_id, request, receipt.clone()))
        }

        fn submit_or_recover_archive_registration(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            intent: &PublicationArchiveRegistrationIntentV1,
        ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
            Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                registered_archive(request, &self.broker, intent),
            ))
        }

        fn prepare_archive_location_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registered: &PublicationRegisteredArchiveV1,
            generation: u8,
            prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
            self.prepared_generations
                .push((generation, prior_location_ids.to_vec()));
            Ok(
                location_registration_generation(operation_id, request, registered, generation)
                    .intent,
            )
        }

        fn submit_or_recover_archive_location(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _registered: &PublicationRegisteredArchiveV1,
            intent: &PublicationArchiveLocationIntentV1,
            _prior_location_ids: &[MusubiArchiveLocationIdV1],
        ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
            if !self.applied_generations.contains(&intent.generation) {
                self.applied_generations.push(intent.generation);
                if self.drop_location_response_once {
                    self.drop_location_response_once = false;
                    return Err(PublicationBackendError::retryable(
                        "ARCHIVE_LOCATION_COMMIT_RESPONSE_DROPPED",
                    ));
                }
            }
            Ok(PublicationArchiveLocationAdvanceV1::Registered(
                finalized_location_registration(request, intent),
            ))
        }

        fn finalized_replication(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            registration: &PublicationArchiveRegistrationV1,
        ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
            match self
                .replication_script
                .pop_front()
                .unwrap_or(LocationPollV1::Healthy)
            {
                LocationPollV1::Healthy => Ok(PublicationReplicationAdvanceV1::Healthy(
                    replication_checkpoint(request, registration, 3),
                )),
                LocationPollV1::HealthyRevisionOffset(offset) => {
                    Ok(PublicationReplicationAdvanceV1::Healthy(
                        replication_checkpoint_with_revision_offset(request, registration, offset),
                    ))
                }
                LocationPollV1::HealthyDirectoryAdvance => {
                    Ok(PublicationReplicationAdvanceV1::Healthy(
                        replication_checkpoint_with_directory_advance(request, registration),
                    ))
                }
                LocationPollV1::Retired => Ok(PublicationReplicationAdvanceV1::Retired(
                    retired_location_terminal(registration),
                )),
                LocationPollV1::RetiredRevisionOffset(offset) => {
                    Ok(PublicationReplicationAdvanceV1::Retired(
                        retired_location_terminal_with_revision_offset(registration, offset),
                    ))
                }
            }
        }

        fn readback_provider(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            location: &MusubiArchiveLocationV1,
            provider: ProviderId,
        ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
            Ok(PublicationReadbackEvidenceV1 {
                provider,
                location_id: location.location_id,
                replication_order: location.replication_order,
                commitment: request.archive_commitment.clone(),
                semantic_release_digest: request.publication.manifest.semantic_digest(),
                verification_lock_digest: request.publication.manifest.verification_lock_digest,
            })
        }

        fn prepare_release_submission_intent(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            preparation: &PublicationReleasePreparationFloorV1,
        ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
            self.release_preparations += 1;
            let nonce = u32::try_from(self.release_preparations)
                .expect("test release submission count fits u32");
            PublicationReleaseSubmissionIntentV1::try_new(
                operation_id,
                request,
                preparation.clone(),
                &signed_release_transaction(request, nonce),
            )
            .map_err(|_| PublicationBackendError::permanent("TEST_RELEASE_INTENT_INVALID"))
        }

        fn submit_or_recover_release_submission(
            &mut self,
            operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            intent: &PublicationReleaseSubmissionIntentV1,
            allow_absent_submission: bool,
        ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
            self.release_intents.push(intent.signed_transaction_digest);
            if self.release_applied {
                return Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
                    PublicationAmxSubmissionV1::new(
                        operation_id,
                        &request.publish_instruction(),
                        intent.transaction_hash,
                        80,
                    ),
                ));
            }
            if self.release_pending_responses > 0 {
                self.release_pending_responses -= 1;
                return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
            }
            if !allow_absent_submission {
                return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
            }
            self.release_submissions += 1;
            if self.reject_release {
                let block_height = intent
                    .preparation
                    .replication
                    .finalized_page
                    .snapshot
                    .finalized_height
                    .saturating_add(1);
                let finalized_time_ms = release_submission_valid_until_ms(intent)
                    .expect("test release deadline")
                    .saturating_add(1);
                let mut absence =
                    release_absence_evidence(request, block_height, finalized_time_ms);
                let preparation_revision = intent
                    .preparation
                    .replication
                    .finalized_page
                    .snapshot
                    .index_revision;
                absence.resolver_page.snapshot.index_revision = preparation_revision;
                absence.retention_query.expected_snapshot = Some(absence.resolver_page.snapshot);
                absence.retention_page.snapshot.index_revision = preparation_revision;
                absence.retention_page.items[0]
                    .storage
                    .as_mut()
                    .expect("test archive remains known")
                    .index_revision = preparation_revision;
                return Ok(PublicationReleaseSubmissionAdvanceV1::Terminal(
                    PublicationReleaseSubmissionTerminalV1::registry_rejected(
                        intent,
                        block_height,
                        absence,
                    ),
                ));
            }
            self.release_applied = true;
            if self.drop_release_response_once {
                self.drop_release_response_once = false;
                return Err(PublicationBackendError::retryable(
                    "RELEASE_COMMIT_RESPONSE_DROPPED",
                ));
            }
            Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
                PublicationAmxSubmissionV1::new(
                    operation_id,
                    &request.publish_instruction(),
                    intent.transaction_hash,
                    80,
                ),
            ))
        }

        fn finalized_release_and_index(
            &mut self,
            _operation_id: PublicationOperationIdV1,
            request: &PublicationRequestV1,
            _submission: &PublicationAmxSubmissionV1,
        ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
            Ok(Some(final_evidence(request)))
        }
    }

    fn account(seed: u8) -> (AccountId, KeyPair) {
        let keypair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture keypair");
        (AccountId::new(keypair.public_key().clone()), keypair)
    }

    fn maximum_legal_musubi_account() -> AccountId {
        let members = (0_u16..256)
            .map(|index| {
                let mut seed = [0xC4; 32];
                seed[..2].copy_from_slice(&index.to_le_bytes());
                let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                    .expect("near-limit account keypair");
                MultisigMember::new(keypair.public_key().clone(), 1)
                    .expect("near-limit account member")
            })
            .collect::<Vec<_>>();

        for count in (1..=members.len()).rev() {
            let policy = MultisigPolicy::new(1, members[..count].to_vec())
                .expect("near-limit account policy");
            let account = AccountId::new_multisig(policy);
            let size = norito::to_bytes(&account)
                .expect("near-limit account canonical bytes")
                .len();
            if size <= MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 {
                assert!(count < members.len(), "fixture must cross the Musubi bound");
                let larger = AccountId::new_multisig(
                    MultisigPolicy::new(1, members[..=count].to_vec())
                        .expect("one-member-larger account policy"),
                );
                assert!(
                    norito::to_bytes(&larger)
                        .expect("one-member-larger account canonical bytes")
                        .len()
                        > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1,
                    "selected account must be the largest legal member prefix"
                );
                return account;
            }
        }
        panic!("at least one multisig member must fit the Musubi account bound");
    }

    fn snapshot() -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: 42,
            finalized_block_hash: [0x42; 32],
            index_revision: 3,
        }
    }

    const PUBLICATION_FIXTURE_PLAN_PAYLOAD: &[u8] = b"canonical publication source payload";
    const PUBLICATION_FIXTURE_CAR_BODY: &[u8] = b"canonical publication CAR body";

    fn publication_fixture_car_plan() -> CarBuildPlan {
        publication_fixture_car_plan_with_source(PUBLICATION_FIXTURE_PLAN_PAYLOAD)
    }

    fn publication_fixture_car_plan_with_source(source: &[u8]) -> CarBuildPlan {
        publication_fixture_car_plan_and_payload_with_source(source).0
    }

    fn publication_fixture_car_plan_and_payload_with_source(
        source: &[u8],
    ) -> (CarBuildPlan, Vec<u8>) {
        let entries = [
            sorafs_car::FileEntry {
                path: vec!["src".to_owned(), "lib.ko".to_owned()],
                data: source.to_vec(),
            },
            sorafs_car::FileEntry {
                path: vec![".musubi".to_owned(), "semantic-release.norito".to_owned()],
                data: b"semantic release".to_vec(),
            },
            sorafs_car::FileEntry {
                path: vec![
                    ".musubi".to_owned(),
                    "artifact-descriptor.norito".to_owned(),
                ],
                data: b"artifact descriptor".to_vec(),
            },
            sorafs_car::FileEntry {
                path: vec![".musubi".to_owned(), "verification-lock.norito".to_owned()],
                data: b"verification lock".to_vec(),
            },
        ];
        CarBuildPlan::from_files(entries.into_iter().collect()).expect("fixture CAR plan")
    }

    fn publication_fixture_canonical_car() -> (CarBuildPlan, Vec<u8>, MusubiArchiveCommitmentV1) {
        let (plan, payload) =
            publication_fixture_car_plan_and_payload_with_source(PUBLICATION_FIXTURE_PLAN_PAYLOAD);
        let mut car = Vec::new();
        let stats = sorafs_car::CarWriter::new(&plan, &payload)
            .expect("fixture CAR writer")
            .write_to(&mut car)
            .expect("canonical fixture CAR");
        let descriptor = sorafs_car::chunker_registry::default_descriptor();
        assert_eq!(descriptor.profile, plan.chunk_profile);
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::try_from(
                stats.root_cids.first().expect("fixture CAR root").clone(),
            )
            .expect("canonical fixture root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: descriptor.id.0,
                namespace: descriptor.namespace.to_owned(),
                name: descriptor.name.to_owned(),
                semver: descriptor.semver.to_owned(),
                multihash_code: descriptor.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(
                sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks),
            ),
            por_root: MusubiContentDigestV1::new(
                sorafs_car::compute_por_root(&payload, &plan).expect("fixture PoR root"),
            ),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
            car_size: stats.car_size,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: u32::try_from(
                plan.files
                    .len()
                    .checked_sub(3)
                    .expect("fixture contains the mandatory bundle entries"),
            )
            .expect("fixture source file count fits u32"),
            chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count fits u32"),
        };
        commitment.validate().expect("fixture archive commitment");
        (plan, car, commitment)
    }

    fn publication_fixture_commitment_for_car(car: &[u8]) -> MusubiArchiveCommitmentV1 {
        publication_fixture_commitment_for_plan(car, &publication_fixture_car_plan())
    }

    fn publication_fixture_commitment_for_plan(
        car: &[u8],
        plan: &CarBuildPlan,
    ) -> MusubiArchiveCommitmentV1 {
        let descriptor = sorafs_car::chunker_registry::default_descriptor();
        assert_eq!(descriptor.profile, plan.chunk_profile);
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: descriptor.id.0,
                namespace: descriptor.namespace.to_owned(),
                name: descriptor.name.to_owned(),
                semver: descriptor.semver.to_owned(),
                multihash_code: descriptor.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(
                sorafs_car::compute_chunk_plan_digest_sha3(&plan.chunks),
            ),
            por_root: MusubiContentDigestV1::new([3; 32]),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*blake3::hash(car).as_bytes()),
            car_size: u64::try_from(car.len()).expect("fixture CAR length fits u64"),
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: u32::try_from(
                plan.files
                    .len()
                    .checked_sub(3)
                    .expect("fixture contains the mandatory bundle entries"),
            )
            .expect("fixture source file count fits u32"),
            chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count fits u32"),
        }
    }

    fn archive_commitment() -> MusubiArchiveCommitmentV1 {
        publication_fixture_commitment_for_car(PUBLICATION_FIXTURE_CAR_BODY)
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

    fn request_with_archive_commitment(
        commitment: MusubiArchiveCommitmentV1,
    ) -> (PublicationRequestV1, KeyPair) {
        let (mut request, broker) = request();
        request.publication.manifest.archive_id = commitment.archive_id();
        request.archive_commitment = commitment;
        request.validate().expect("canonical CAR request");
        (request, broker)
    }

    fn signed_release_transaction(request: &PublicationRequestV1, nonce: u32) -> SignedTransaction {
        let (publisher, publisher_keypair) = account(20);
        assert_eq!(publisher, request.publisher);
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.publish_instruction()]);
        builder.set_creation_time(std::time::Duration::from_millis(2_000 + u64::from(nonce)));
        builder.set_nonce(NonZeroU32::new(nonce).expect("release fixture nonce is non-zero"));
        builder.sign(publisher_keypair.private_key())
    }

    fn maximum_multisig_release_transaction(
        mut request: PublicationRequestV1,
    ) -> (PublicationRequestV1, SignedTransaction) {
        let signers = (0..MUSUBI_MAX_RELEASE_SIGNATURES_V1)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![u8::try_from(index + 100).expect("fixture seed"); 32],
                    Algorithm::Ed25519,
                )
                .expect("multisig fixture key")
            })
            .collect::<Vec<_>>();
        let members = signers
            .iter()
            .map(|signer| {
                MultisigMember::new(signer.public_key().clone(), 1)
                    .expect("multisig fixture member")
            })
            .collect::<Vec<_>>();
        request.publisher = AccountId::new_multisig(
            MultisigPolicy::new(
                u16::try_from(MUSUBI_MAX_RELEASE_SIGNATURES_V1)
                    .expect("signature maximum fits u16"),
                members,
            )
            .expect("multisig fixture policy"),
        );
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.publish_instruction()]);
        builder.set_creation_time(std::time::Duration::from_millis(2_000));
        let signed = builder.sign_multisig(signers.iter().map(KeyPair::private_key));
        (request, signed)
    }

    fn release_preparation_fixture(
        request: &PublicationRequestV1,
        broker: &KeyPair,
    ) -> (
        PublicationArchiveRegistrationV1,
        PublicationReleasePreparationFloorV1,
    ) {
        release_preparation_fixture_with_offset(request, broker, 0)
    }

    fn release_preparation_fixture_with_offset(
        request: &PublicationRequestV1,
        broker: &KeyPair,
        offset: u64,
    ) -> (
        PublicationArchiveRegistrationV1,
        PublicationReleasePreparationFloorV1,
    ) {
        let registration = registration(request, broker);
        let replication =
            replication_checkpoint_with_revision_offset(request, &registration, offset);
        let floor = release_preparation_for_registration(request, &registration, replication);
        (registration, floor)
    }

    fn release_preparation_for_registration(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        replication: PublicationReplicationCheckpointV1,
    ) -> PublicationReleasePreparationFloorV1 {
        let location = replication
            .location(registration)
            .expect("release fixture location");
        let readbacks = location
            .providers
            .iter()
            .take(2)
            .map(|provider| PublicationReadbackEvidenceV1 {
                provider: *provider,
                location_id: location.location_id,
                replication_order: location.replication_order,
                commitment: request.archive_commitment.clone(),
                semantic_release_digest: request.publication.manifest.semantic_digest(),
                verification_lock_digest: request.publication.manifest.verification_lock_digest,
            })
            .collect::<Vec<_>>();
        PublicationReleasePreparationFloorV1::try_new(
            registration.intent.generation,
            replication,
            readbacks,
            request,
            registration,
        )
        .expect("release preparation floor")
    }

    fn release_ready_journal(
        request: &PublicationRequestV1,
        broker: &KeyPair,
    ) -> PublicationJournalV1 {
        let operation_id = request.operation_id();
        let (location_registration, floor) = release_preparation_fixture(request, broker);
        let receipt = location_registration
            .intent
            .prepared_page
            .archive
            .staging_receipt
            .clone();
        let archive_intent = registration_intent(operation_id, request, receipt.clone());
        let registered = registered_archive(request, broker, &archive_intent);
        let mut journal = PublicationJournalV1::new(request.clone()).expect("release journal");
        journal.phase = PublicationPhaseV1::ReleaseSubmission;
        journal.validation = Some(validation_evidence(request));
        journal.staging_receipt = Some(receipt);
        journal.archive_registration_attempts = vec![PublicationArchiveRegistrationAttemptV1::new(
            1,
            archive_intent,
        )];
        journal.registered_archive = Some(registered);
        journal.archive_location_attempts = vec![PublicationArchiveLocationAttemptV1 {
            generation: 1,
            intent: location_registration.intent.clone(),
            registration: Some(location_registration),
            terminal: None,
            terminal_floor: None,
        }];
        journal.replication = Some(floor.replication.clone());
        journal.readbacks = floor.readbacks.clone();
        let intent = PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            request,
            floor,
            &signed_release_transaction(request, 1),
        )
        .expect("release-ready exact intent");
        journal.release_submission_attempts =
            vec![PublicationReleaseSubmissionAttemptV1::new(1, intent)];
        journal.validate().expect("release-ready journal");
        journal
    }

    fn release_absence_evidence(
        request: &PublicationRequestV1,
        finalized_height: u64,
        finalized_time_ms: u64,
    ) -> PublicationReleaseAbsenceEvidenceV1 {
        assert!(finalized_height > 1);
        let index_revision = finalized_height.saturating_sub(68).max(1);
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height,
            finalized_block_hash: [0xD1; 32],
            index_revision,
        };
        let retention = MusubiArchiveRetentionDecisionV1 {
            archive_id: request.archive_commitment.archive_id(),
            disposition: MusubiArchiveRetentionDispositionV1::PruneUnreferenced,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: Some(MusubiArchiveAvailabilityV1 {
                archive_id: request.archive_commitment.archive_id(),
                availability: MusubiStorageAvailabilityV1::Selectable,
                healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                active_locations: 1,
                finalized_height: finalized_height - 1,
                finalized_block_hash: [0xD0; 32],
                index_revision,
            }),
        };
        PublicationReleaseAbsenceEvidenceV1 {
            resolver_page: MusubiResolverIndexPageV1 {
                query: MusubiResolverIndexQueryV1 {
                    package: request.publication.manifest.release.package.clone(),
                    requirement: Some(
                        format!("={}", request.publication.manifest.release.version)
                            .parse()
                            .expect("exact fixture requirement"),
                    ),
                    page: MusubiPageRequestV1 {
                        limit: 1,
                        cursor: None,
                    },
                },
                chain_id: request.chain_id.clone(),
                genesis_hash: request.genesis_block_hash,
                items: Vec::new(),
                next_cursor: None,
                snapshot,
            },
            retention_query: MusubiArchiveRetentionQueryV1 {
                archive_ids: vec![request.archive_commitment.archive_id()],
                expected_snapshot: Some(snapshot),
            },
            retention_page: MusubiArchiveRetentionPageV1 {
                chain_id: request.chain_id.clone(),
                genesis_hash: request.genesis_block_hash,
                items: vec![retention],
                snapshot,
                finalized_time_ms,
            },
        }
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

    fn archive_absence_evidence(
        request: &PublicationRequestV1,
        finalized_height: u64,
    ) -> PublicationArchiveAbsenceEvidenceV1 {
        PublicationArchiveAbsenceEvidenceV1 {
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height,
                finalized_block_hash: [0xA5; 32],
                index_revision: finalized_height,
            },
            finalized_time_ms: 1_700_000_000_000,
            decision: MusubiArchiveRetentionDecisionV1 {
                archive_id: request.archive_commitment.archive_id(),
                disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
                active_releases: 0,
                yanked_releases: 0,
                taken_down_releases: 0,
                storage: None,
            },
        }
    }

    fn registration_intent(
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: MusubiSeedIngressReceiptV1,
    ) -> PublicationArchiveRegistrationIntentV1 {
        let (publisher, publisher_keypair) = account(20);
        assert_eq!(publisher, request.publisher);
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.archive_registration_instruction(&receipt)]);
        builder.set_creation_time(std::time::Duration::from_millis(
            receipt.payload.issued_at_ms,
        ));
        let signed_transaction = builder.sign(publisher_keypair.private_key());
        PublicationArchiveRegistrationIntentV1::new(
            operation_id,
            request,
            receipt,
            signed_transaction,
        )
    }

    fn registered_archive(
        request: &PublicationRequestV1,
        broker: &KeyPair,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> PublicationRegisteredArchiveV1 {
        let mut archive = archive_record(request, broker);
        archive.staging_receipt = intent.staging_receipt.clone();
        PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: intent.transaction_hash,
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3C; 32],
                index_revision: 2,
            },
            archive,
        }
    }

    fn archive_record(request: &PublicationRequestV1, broker: &KeyPair) -> MusubiArchiveRecordV1 {
        MusubiArchiveRecordV1 {
            archive_id: request.archive_commitment.archive_id(),
            commitment: request.archive_commitment.clone(),
            staging_receipt: signed_receipt(&request.receipt_binding(), broker),
            registered_by: request.publisher.clone(),
            registered_at_height: 50,
            location_revision: 1,
            location_ids: Vec::new(),
        }
    }

    fn registration(
        request: &PublicationRequestV1,
        broker: &KeyPair,
    ) -> PublicationArchiveRegistrationV1 {
        let archive = archive_record(request, broker);
        let prepared_page = MusubiArchiveLocationPageV1 {
            chain_id: request.chain_id.clone(),
            genesis_hash: request.genesis_block_hash,
            archive: archive.clone(),
            items: Vec::new(),
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3C; 32],
                index_revision: 2,
            },
        };
        let replication_order = ReplicationOrderId::new([0x33; 32]);
        let provider_attestations = (1..=3)
            .map(|provider| provider_attestation(request, replication_order, provider))
            .collect::<Vec<_>>();
        let provider_attestation_set_digest = provider_attestation_set_digest(
            request.archive_commitment.archive_id(),
            replication_order,
            &provider_attestations,
        );
        let instruction = AddMusubiArchiveLocationV1 {
            archive_id: request.archive_commitment.archive_id(),
            location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
            pin_manifest: ManifestDigest::new([0x32; 32]),
            replication_order,
            provider_attestation_set_digest,
            renew_after_epoch: 10,
            expires_at_epoch: 20,
            expected_location_revision: archive.location_revision,
        };
        let (_, publisher_keypair) = account(20);
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction.clone()]);
        builder.set_creation_time(std::time::Duration::from_millis(1_000));
        let intent = PublicationArchiveLocationIntentV1::new(
            request.operation_id(),
            1,
            prepared_page,
            instruction,
            builder.sign(publisher_keypair.private_key()),
        );
        let location = MusubiArchiveLocationV1 {
            location_id: intent.location_id,
            archive_id: request.archive_commitment.archive_id(),
            pin_manifest: intent.pin_manifest,
            replication_order: intent.replication_order,
            providers: provider_attestations
                .iter()
                .map(|attestation| attestation.payload.binding.provider_id)
                .collect(),
            provider_attestation_set_digest,
            renew_after_epoch: intent.renew_after_epoch,
            expires_at_epoch: intent.expires_at_epoch,
            finalized_height: 70,
            revision: 2,
            state: MusubiArchiveLocationStateV1::Healthy,
        };
        let mut finalized_archive = archive;
        finalized_archive.location_revision = 2;
        finalized_archive.location_ids = vec![intent.location_id];
        PublicationArchiveRegistrationV1 {
            intent,
            applied_height: 70,
            finalized_page: MusubiArchiveLocationPageV1 {
                chain_id: request.chain_id.clone(),
                genesis_hash: request.genesis_block_hash,
                archive: finalized_archive,
                items: vec![location],
                next_cursor: None,
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height: 70,
                    finalized_block_hash: [0x46; 32],
                    index_revision: 3,
                },
            },
        }
    }

    fn location_registration_generation(
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
    ) -> PublicationArchiveRegistrationV1 {
        assert!(generation > 0);
        let generation_u64 = u64::from(generation);
        let completed_generations = generation_u64 - 1;
        let prepared_height = registered.snapshot.finalized_height + completed_generations * 2;
        let prepared_revision = registered.archive.location_revision + completed_generations * 2;
        let prepared_snapshot = if generation == 1 {
            registered.snapshot
        } else {
            MusubiRegistrySnapshotV1 {
                finalized_height: prepared_height,
                finalized_block_hash: [0x6F_u8.saturating_add(generation); 32],
                index_revision: registered.snapshot.index_revision + completed_generations * 2,
            }
        };
        let mut prepared_archive = registered.archive.clone();
        prepared_archive.location_revision = prepared_revision;
        prepared_archive.location_ids.clear();
        let prepared_page = MusubiArchiveLocationPageV1 {
            chain_id: request.chain_id.clone(),
            genesis_hash: request.genesis_block_hash,
            archive: prepared_archive,
            items: Vec::new(),
            next_cursor: None,
            snapshot: prepared_snapshot,
        };
        let replication_order = ReplicationOrderId::new([0x40_u8.saturating_add(generation); 32]);
        let provider_attestations = (1..=3)
            .map(|provider| provider_attestation(request, replication_order, provider))
            .collect::<Vec<_>>();
        let provider_attestation_set_digest = provider_attestation_set_digest(
            request.archive_commitment.archive_id(),
            replication_order,
            &provider_attestations,
        );
        let instruction = AddMusubiArchiveLocationV1 {
            archive_id: request.archive_commitment.archive_id(),
            location_id: MusubiArchiveLocationIdV1::new([0x30_u8.saturating_add(generation); 32]),
            pin_manifest: ManifestDigest::new([0x50_u8.saturating_add(generation); 32]),
            replication_order,
            provider_attestation_set_digest,
            renew_after_epoch: 20 + generation_u64,
            expires_at_epoch: 40 + generation_u64,
            expected_location_revision: prepared_revision,
        };
        let (_, publisher_keypair) = account(20);
        let mut builder = TransactionBuilder::new(
            request.network_id(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction.clone()]);
        builder.set_creation_time(std::time::Duration::from_millis(1_000 + generation_u64));
        let intent = PublicationArchiveLocationIntentV1::new(
            operation_id,
            generation,
            prepared_page,
            instruction,
            builder.sign(publisher_keypair.private_key()),
        );
        finalized_location_registration(request, &intent)
    }

    fn finalized_location_registration(
        request: &PublicationRequestV1,
        intent: &PublicationArchiveLocationIntentV1,
    ) -> PublicationArchiveRegistrationV1 {
        let finalized_height = intent.prepared_page.snapshot.finalized_height + 1;
        let finalized_revision = intent.expected_location_revision + 1;
        let provider_attestations = (1..=3)
            .map(|provider| provider_attestation(request, intent.replication_order, provider))
            .collect::<Vec<_>>();
        let providers = provider_attestations
            .iter()
            .map(|attestation| attestation.payload.binding.provider_id)
            .collect::<Vec<_>>();
        let location = MusubiArchiveLocationV1 {
            location_id: intent.location_id,
            archive_id: request.archive_commitment.archive_id(),
            pin_manifest: intent.pin_manifest,
            replication_order: intent.replication_order,
            providers,
            provider_attestation_set_digest: intent.provider_attestation_set_digest,
            renew_after_epoch: intent.renew_after_epoch,
            expires_at_epoch: intent.expires_at_epoch,
            finalized_height,
            revision: finalized_revision,
            state: MusubiArchiveLocationStateV1::Healthy,
        };
        let mut finalized_archive = intent.prepared_page.archive.clone();
        finalized_archive.location_revision = finalized_revision;
        finalized_archive.location_ids = vec![intent.location_id];
        PublicationArchiveRegistrationV1 {
            intent: intent.clone(),
            applied_height: finalized_height,
            finalized_page: MusubiArchiveLocationPageV1 {
                chain_id: request.chain_id.clone(),
                genesis_hash: request.genesis_block_hash,
                archive: finalized_archive,
                items: vec![location],
                next_cursor: None,
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height,
                    finalized_block_hash: [0x60_u8.saturating_add(intent.generation); 32],
                    index_revision: intent.prepared_page.snapshot.index_revision + 1,
                },
            },
        }
    }

    fn retired_location_terminal(
        registration: &PublicationArchiveRegistrationV1,
    ) -> PublicationArchiveLocationTerminalV1 {
        retired_location_terminal_with_revision_offset(registration, 0)
    }

    fn retired_location_terminal_with_revision_offset(
        registration: &PublicationArchiveRegistrationV1,
        offset: u64,
    ) -> PublicationArchiveLocationTerminalV1 {
        let mut finalized_page = registration.finalized_page.clone();
        finalized_page.archive.location_revision += 1 + offset;
        finalized_page.archive.location_ids.clear();
        finalized_page.items.clear();
        finalized_page.snapshot.finalized_height += 1 + offset;
        finalized_page.snapshot.finalized_block_hash = [0x70_u8
            .saturating_add(registration.intent.generation)
            .saturating_add(u8::try_from(offset).unwrap_or(u8::MAX));
            32];
        finalized_page.snapshot.index_revision += 1 + offset;
        PublicationArchiveLocationTerminalV1 {
            transaction_hash: registration.intent.transaction_hash,
            reason: PublicationArchiveLocationTerminalReasonV1::Retired,
            finalized_page,
        }
    }

    fn provider_attestation(
        request: &PublicationRequestV1,
        replication_order: ReplicationOrderId,
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
            replication_order,
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

    fn provider_attestation_set_digest(
        archive_id: ArchiveId,
        replication_order: ReplicationOrderId,
        attestations: &[MusubiProviderBundleVerificationAttestationV1],
    ) -> MusubiProviderBundleAttestationSetDigestV1 {
        let references = attestations
            .iter()
            .map(MusubiProviderBundleVerificationAttestationV1::reference)
            .collect::<Vec<_>>();
        musubi_provider_bundle_attestation_set_digest_v1(archive_id, replication_order, &references)
            .expect("canonical provider attestation set")
    }

    fn location(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        provider_count: u8,
    ) -> MusubiArchiveLocationV1 {
        let registered_location = registration
            .location()
            .expect("registered fixture location");
        let attestations = (1..=provider_count)
            .map(|provider| {
                provider_attestation(request, registration.intent.replication_order, provider)
            })
            .collect::<Vec<_>>();
        let provider_attestation_set_digest = provider_attestation_set_digest(
            request.archive_commitment.archive_id(),
            registration.intent.replication_order,
            &attestations,
        );
        MusubiArchiveLocationV1 {
            location_id: registration.location_id(),
            archive_id: request.archive_commitment.archive_id(),
            pin_manifest: registration.intent.pin_manifest,
            replication_order: registration.intent.replication_order,
            providers: attestations
                .iter()
                .map(|attestation| attestation.payload.binding.provider_id)
                .collect(),
            provider_attestation_set_digest,
            renew_after_epoch: registration.intent.renew_after_epoch,
            expires_at_epoch: registration.intent.expires_at_epoch,
            finalized_height: registered_location.finalized_height,
            revision: registered_location.revision,
            state: MusubiArchiveLocationStateV1::Healthy,
        }
    }

    fn replication_checkpoint(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        provider_count: u8,
    ) -> PublicationReplicationCheckpointV1 {
        let mut finalized_page = registration.finalized_page.clone();
        let index = finalized_page
            .items
            .binary_search_by_key(&registration.location_id(), |location| location.location_id)
            .expect("registered fixture location is present");
        finalized_page.items[index] = location(request, registration, provider_count);
        PublicationReplicationCheckpointV1 { finalized_page }
    }

    fn replication_checkpoint_with_revision_offset(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        offset: u64,
    ) -> PublicationReplicationCheckpointV1 {
        let mut checkpoint = replication_checkpoint(request, registration, 3);
        if offset == 0 {
            return checkpoint;
        }
        let location = checkpoint
            .finalized_page
            .items
            .iter_mut()
            .find(|location| location.location_id == registration.location_id())
            .expect("registered fixture location is present");
        location.revision += offset;
        location.finalized_height += offset;
        checkpoint.finalized_page.archive.location_revision += offset;
        checkpoint.finalized_page.snapshot.finalized_height += offset;
        checkpoint.finalized_page.snapshot.finalized_block_hash =
            [0x80_u8.saturating_add(u8::try_from(offset).unwrap_or(u8::MAX)); 32];
        checkpoint.finalized_page.snapshot.index_revision += offset;
        checkpoint
    }

    fn replication_checkpoint_with_journal_max_shape(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
        offset: u64,
    ) -> PublicationReplicationCheckpointV1 {
        let provider_count = u8::try_from(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
            .expect("provider maximum fits the fixture counter");
        let mut checkpoint = replication_checkpoint(request, registration, provider_count);
        let target = checkpoint
            .finalized_page
            .items
            .iter_mut()
            .find(|location| location.location_id == registration.location_id())
            .expect("registered fixture location is present");
        target.revision = registration
            .location()
            .expect("registered fixture location")
            .revision
            + 1
            + offset;
        target.finalized_height =
            registration.finalized_page.snapshot.finalized_height + 1 + offset;
        let target = target.clone();

        checkpoint.finalized_page.archive.location_revision = target.revision + 3;
        checkpoint.finalized_page.snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: target.finalized_height + 3,
            finalized_block_hash: [0xA0_u8.saturating_add(u8::try_from(offset).unwrap_or(u8::MAX));
                32],
            index_revision: registration.finalized_page.snapshot.index_revision + 4 + offset,
        };
        for index in 1..MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
            let mut location = target.clone();
            let index_u8 = u8::try_from(index).expect("location maximum fits u8");
            location.location_id =
                MusubiArchiveLocationIdV1::new([0xA0_u8.saturating_add(index_u8); 32]);
            location.revision = target.revision + u64::from(index_u8);
            location.finalized_height = target.finalized_height + u64::from(index_u8);
            checkpoint
                .finalized_page
                .archive
                .location_ids
                .push(location.location_id);
            checkpoint.finalized_page.items.push(location);
        }
        checkpoint
    }

    fn replication_checkpoint_with_directory_advance(
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> PublicationReplicationCheckpointV1 {
        let mut checkpoint = replication_checkpoint(request, registration, 3);
        checkpoint.finalized_page.archive.location_revision += 1;
        checkpoint.finalized_page.snapshot.finalized_height += 1;
        checkpoint.finalized_page.snapshot.finalized_block_hash = [0x91; 32];

        let mut unrelated = checkpoint
            .location(registration)
            .expect("registered fixture location")
            .clone();
        unrelated.location_id = MusubiArchiveLocationIdV1::new([0xF0; 32]);
        unrelated.revision = checkpoint.finalized_page.archive.location_revision;
        unrelated.finalized_height = checkpoint.finalized_page.snapshot.finalized_height;
        checkpoint
            .finalized_page
            .archive
            .location_ids
            .push(unrelated.location_id);
        checkpoint.finalized_page.items.push(unrelated);
        checkpoint
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

    #[test]
    fn finalized_chain_time_must_strictly_pass_the_exact_registration_deadline() {
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let receipt = signed_receipt(&request.receipt_binding(), &broker);
        let intent = registration_intent(operation_id, &request, receipt);
        let valid_until_ms = archive_registration_intent_valid_until_ms(&intent)
            .expect("exact registration deadline");

        let mut at_deadline = archive_absence_evidence(&request, 60);
        at_deadline.finalized_time_ms = valid_until_ms;
        let terminal = PublicationArchiveRegistrationTerminalV1::finalized_validity_window_elapsed(
            &intent,
            at_deadline,
        );
        assert!(terminal.validate_for(&request, &intent).is_err());

        let mut after_deadline = archive_absence_evidence(&request, 61);
        after_deadline.finalized_time_ms = valid_until_ms + 1;
        let terminal = PublicationArchiveRegistrationTerminalV1::finalized_validity_window_elapsed(
            &intent,
            after_deadline,
        );
        terminal
            .validate_for(&request, &intent)
            .expect("finalized time after the exact deadline is terminal");

        let mut substituted = terminal;
        substituted.reason =
            PublicationArchiveRegistrationTerminalReasonV1::FinalizedValidityWindowElapsed {
                finalized_time_ms: valid_until_ms + 2,
            };
        assert!(substituted.validate_for(&request, &intent).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn retry_and_receipt_substitution_never_advance_the_journal() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let source = BytesSource(b"runtime-only-car-secret".to_vec());
        let plan_bytes = source
            .car_plan(&request.archive_commitment)
            .expect("fixture wire plan")
            .canonical_bytes()
            .expect("canonical fixture plan");
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
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
        assert!(
            !journal_bytes
                .windows(plan_bytes.len())
                .any(|window| window == plan_bytes.as_slice())
        );

        let mut backend = EarlyBackend {
            broker,
            fail_validation_once: true,
            substitute_receipt: true,
            now_ms: 1_500,
            receipt_window: None,
            prepare_calls: 0,
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
    fn future_issued_receipt_waits_within_service_skew_before_registration() {
        let within = tempdir().expect("within-skew state root");
        let within_store =
            PublicationJournalStore::open(within.path()).expect("within-skew journal store");
        let within_engine = PublicationEngine::new(&within_store);
        let (within_request, within_broker) = request();
        let within_operation = within_engine
            .begin_detached(within_request)
            .expect("persist within-skew operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let now_ms = 1_000;
        let issued_at_ms = now_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1;
        let mut within_backend = EarlyBackend {
            broker: within_broker,
            fail_validation_once: false,
            substitute_receipt: false,
            now_ms,
            receipt_window: Some((issued_at_ms, issued_at_ms + 100)),
            prepare_calls: 0,
        };

        assert_eq!(
            within_engine
                .advance_once(within_operation, &source, &mut within_backend)
                .expect("validate within-skew operation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        );
        assert_eq!(
            within_engine
                .advance_once(within_operation, &source, &mut within_backend)
                .expect("accept bounded future-issued receipt"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let waiting = within_store
            .load(within_operation)
            .expect("future-issued receipt journal");
        assert_eq!(
            within_engine
                .advance_once(within_operation, &source, &mut within_backend)
                .expect("wait for receipt issue time"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(
            within_store
                .load(within_operation)
                .expect("unchanged waiting journal"),
            waiting
        );
        assert_eq!(within_backend.prepare_calls, 0);

        within_backend.now_ms = issued_at_ms;
        assert_eq!(
            within_engine
                .advance_once(within_operation, &source, &mut within_backend)
                .expect("prepare at the inclusive issue time"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(within_backend.prepare_calls, 1);
        assert_eq!(
            within_store
                .load(within_operation)
                .expect("prepared registration journal")
                .archive_registration_attempts
                .len(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn future_issued_receipt_beyond_service_skew_is_rejected_before_persistence() {
        let beyond = tempdir().expect("beyond-skew state root");
        let beyond_store =
            PublicationJournalStore::open(beyond.path()).expect("beyond-skew journal store");
        let beyond_engine = PublicationEngine::new(&beyond_store);
        let (beyond_request, beyond_broker) = request();
        let beyond_operation = beyond_engine
            .begin_detached(beyond_request)
            .expect("persist beyond-skew operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let now_ms = 1_000;
        let beyond_issue = now_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1 + 1;
        let mut beyond_backend = EarlyBackend {
            broker: beyond_broker,
            fail_validation_once: false,
            substitute_receipt: false,
            now_ms,
            receipt_window: Some((beyond_issue, beyond_issue + 100)),
            prepare_calls: 0,
        };
        beyond_engine
            .advance_once(beyond_operation, &source, &mut beyond_backend)
            .expect("validate beyond-skew operation");
        assert!(matches!(
            beyond_engine.advance_once(beyond_operation, &source, &mut beyond_backend),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::SeedIngress,
                ..
            })
        ));
        let rejected = beyond_store
            .load(beyond_operation)
            .expect("unchanged beyond-skew journal");
        assert_eq!(rejected.phase, PublicationPhaseV1::SeedIngress);
        assert!(rejected.staging_receipt.is_none());
        assert!(rejected.archive_registration_attempts.is_empty());
        assert_eq!(beyond_backend.prepare_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test preserves and checks both crash boundaries around one exact transaction"
    )]
    fn registration_intent_recovers_a_dropped_commit_response_after_expiry_and_restart() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 1_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: true,
            return_conflicting_archive: false,
            registration_mode: ArchiveRecoveryMode::Commit,
        };

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("validate"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stage"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist exact registration intent"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let intent_journal = store.load(operation_id).expect("intent journal");
        let durable_intent = intent_journal
            .archive_registration_attempts
            .last()
            .expect("exact signed transaction attempt")
            .intent
            .clone();
        assert!(intent_journal.registered_archive.is_none());
        assert_eq!(backend.prepare_calls, 1);
        assert_eq!(backend.registration_calls, 0);

        let reopened = PublicationJournalStore::open(temp.path()).expect("reopen journal store");
        let resumed = PublicationEngine::new(&reopened);
        let error = resumed
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("simulate response loss after finalized archive commit");
        assert!(matches!(
            error,
            PublicationError::Backend(ref backend_error)
                if backend_error.code() == "ARCHIVE_COMMIT_RESPONSE_DROPPED"
        ));
        let interrupted = reopened.load(operation_id).expect("interrupted journal");
        assert_eq!(
            interrupted
                .archive_registration_attempts
                .last()
                .map(|attempt| &attempt.intent),
            Some(&durable_intent)
        );
        assert!(interrupted.registered_archive.is_none());
        assert!(backend.now_ms > durable_intent.staging_receipt.payload.expires_at_ms);

        assert_eq!(
            resumed
                .advance_once(operation_id, &source, &mut backend)
                .expect("recover authoritative archive after receipt expiry"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let recovered = reopened
            .load(operation_id)
            .expect("recovered archive journal");
        assert_eq!(
            recovered
                .registered_archive
                .as_ref()
                .expect("authoritative archive")
                .finalized_transaction_hash,
            durable_intent.transaction_hash
        );
        assert!(recovered.archive_location_attempts.is_empty());
        assert_eq!(backend.staged_receipts.len(), 1);
        assert_eq!(backend.prepare_calls, 1);
        assert_eq!(backend.registration_calls, 2);
        assert_eq!(backend.pin_calls, 0);

        let pin_store = PublicationJournalStore::open(temp.path()).expect("reopen before pin");
        let pin_resume = PublicationEngine::new(&pin_store);
        assert_eq!(
            pin_resume
                .advance_once(operation_id, &source, &mut backend)
                .expect("coordinate storage after durable archive recovery"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(backend.pin_calls, 1);
        assert_eq!(
            pin_resume
                .advance_once(operation_id, &source, &mut backend)
                .expect("finalize exact journaled location transaction"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
        );
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test covers every durable cut of one location replacement"
    )]
    fn archive_location_generation_recovers_prepared_submitted_applied_and_retired_cuts() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(broker, [LocationPollV1::Retired]);
        backend.drop_location_response_once = true;

        for step in 0..5 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("prepare location step {step} failed: {error}"));
        }
        let prepared = store.load(operation_id).expect("prepared location journal");
        assert_eq!(prepared.phase, PublicationPhaseV1::ArchiveRegistration);
        assert_eq!(prepared.archive_location_attempts.len(), 1);
        assert!(prepared.archive_location_attempts[0].registration.is_none());
        assert!(prepared.archive_location_attempts[0].terminal.is_none());
        let first_intent = prepared.archive_location_attempts[0].intent.clone();

        let submitted_store =
            PublicationJournalStore::open(temp.path()).expect("reopen after preparation");
        let submitted_engine = PublicationEngine::new(&submitted_store);
        let error = submitted_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("simulate loss after the exact transaction applied");
        assert!(matches!(
            error,
            PublicationError::Backend(ref error)
                if error.code() == "ARCHIVE_LOCATION_COMMIT_RESPONSE_DROPPED"
        ));
        assert_eq!(
            submitted_store
                .load(operation_id)
                .expect("unchanged submitted journal"),
            prepared
        );
        assert_eq!(backend.applied_generations, vec![1]);

        let applied_store =
            PublicationJournalStore::open(temp.path()).expect("reopen after applied cut");
        let applied_engine = PublicationEngine::new(&applied_store);
        assert_eq!(
            applied_engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("recover exact finalized application"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
        );
        let applied = applied_store
            .load(operation_id)
            .expect("applied location journal");
        let applied_attempt = applied.archive_location_attempts[0].clone();
        assert_eq!(applied_attempt.intent, first_intent);
        assert!(applied_attempt.registration.is_some());
        assert!(applied_attempt.terminal.is_none());

        assert_eq!(
            applied_engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist authoritative retirement"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let retired = applied_store
            .load(operation_id)
            .expect("retired location journal");
        assert_eq!(retired.archive_location_attempts.len(), 1);
        assert_eq!(
            retired.archive_location_attempts[0].intent,
            applied_attempt.intent
        );
        assert_eq!(
            retired.archive_location_attempts[0].registration,
            applied_attempt.registration
        );
        assert!(retired.archive_location_attempts[0].terminal.is_some());
        assert!(retired.replication.is_none());
        assert!(retired.readbacks.is_empty());

        let replacement_store =
            PublicationJournalStore::open(temp.path()).expect("reopen after retirement");
        let replacement_engine = PublicationEngine::new(&replacement_store);
        assert_eq!(
            replacement_engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist replacement exact intent"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let replacement_prepared = replacement_store
            .load(operation_id)
            .expect("replacement intent journal");
        assert_eq!(replacement_prepared.archive_location_attempts.len(), 2);
        assert_eq!(
            replacement_prepared.archive_location_attempts[0],
            retired.archive_location_attempts[0]
        );
        let second = &replacement_prepared.archive_location_attempts[1];
        assert_eq!(second.generation, 2);
        assert_ne!(second.intent.location_id, first_intent.location_id);
        assert_ne!(
            second.intent.transaction_hash,
            first_intent.transaction_hash
        );
        assert!(second.registration.is_none());
        assert!(second.terminal.is_none());
        assert_eq!(
            backend.prepared_generations,
            vec![(1, Vec::new()), (2, vec![first_intent.location_id])]
        );

        assert_eq!(
            replacement_engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("finalize replacement location"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
        );
        let replacement = replacement_store
            .load(operation_id)
            .expect("replacement finalized journal");
        assert_eq!(
            replacement.archive_location_attempts[0],
            retired.archive_location_attempts[0]
        );
        assert!(
            replacement.archive_location_attempts[1]
                .registration
                .is_some()
        );
        assert!(replacement.archive_location_attempts[1].terminal.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn retirement_is_rechecked_before_replication_and_readback() {
        for (script, expected_phase) in [
            (
                vec![LocationPollV1::Retired],
                PublicationPhaseV1::Replication,
            ),
            (
                vec![LocationPollV1::Healthy, LocationPollV1::Retired],
                PublicationPhaseV1::Readback,
            ),
        ] {
            let temp = tempdir().expect("state root");
            let store = PublicationJournalStore::open(temp.path()).expect("journal store");
            let engine = PublicationEngine::new(&store);
            let (request, broker) = request();
            let operation_id = engine
                .begin_detached(request)
                .expect("persist detached operation");
            let source = BytesSource(b"canonical-car".to_vec());
            let mut backend = LocationRecoveryBackend::new(broker, script);
            for step in 0..6 {
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
            }
            while store.load(operation_id).expect("phase journal").phase != expected_phase {
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("advance to guarded phase");
            }
            assert_eq!(
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("authoritative retirement rotates the location"),
                PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
            );
            let retired = store.load(operation_id).expect("retired journal");
            assert!(retired.archive_location_attempts[0].terminal.is_some());
            assert!(retired.replication.is_none());
            assert!(retired.readbacks.is_empty());
            assert!(retired.submission.is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn selected_location_renewal_requires_terminal_rotation_and_fresh_readbacks() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::HealthyRevisionOffset(1),
                LocationPollV1::Healthy,
                LocationPollV1::HealthyRevisionOffset(1),
                LocationPollV1::HealthyRevisionOffset(2),
            ],
        );

        for step in 0..6 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
        }
        assert_eq!(
            store.load(operation_id).expect("replication journal").phase,
            PublicationPhaseV1::Replication
        );

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist renewed healthy location"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::Readback)
        );
        let renewed = store.load(operation_id).expect("renewed journal");
        assert_eq!(
            renewed
                .replication
                .as_ref()
                .expect("renewed replication")
                .finalized_page
                .items[0]
                .revision,
            3
        );

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stale finalized poll remains retryable"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::Readback)
        );
        assert_eq!(
            store.load(operation_id).expect("unchanged stale journal"),
            renewed
        );

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("exact journaled revision resumes readback"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("changed selected location keeps the stale-readback intent unsent"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        let guarded = store.load(operation_id).expect("guarded journal");
        assert_eq!(
            guarded.release_submission_attempts[0]
                .intent
                .preparation
                .replication
                .finalized_page
                .items[0]
                .revision,
            3
        );
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_submissions, 0);
    }

    #[cfg(unix)]
    #[test]
    fn stale_pre_send_poll_preserves_the_live_intent_and_replays_identical_bytes() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(1),
                LocationPollV1::HealthyRevisionOffset(2),
            ],
        );

        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        let guarded = store.load(operation_id).expect("release journal");
        assert_eq!(guarded.phase, PublicationPhaseV1::ReleaseSubmission);
        assert_eq!(guarded.readbacks.len(), 2);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stale healthy page remains retryable"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            store.load(operation_id).expect("unchanged journal"),
            guarded
        );
        assert_eq!(backend.release_submissions, 0);
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_intents.len(), 1);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("exact checkpoint permits submission"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
        );
        assert_eq!(backend.release_submissions, 1);
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_intents.len(), 2);
        assert_eq!(backend.release_intents[0], backend.release_intents[1]);
    }

    #[cfg(unix)]
    #[test]
    fn authoritative_pending_status_never_resigns_or_replaces_the_live_transaction() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
            ],
        );
        backend.release_pending_responses = 2;
        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        let live = store.load(operation_id).expect("live release journal");
        let digest = live.release_submission_attempts[0]
            .intent
            .signed_transaction_digest;

        for _ in 0..2 {
            assert_eq!(
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("pending exact status remains retryable"),
                PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
            );
            assert_eq!(store.load(operation_id).expect("unchanged journal"), live);
        }
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_submissions, 0);
        assert_eq!(backend.release_intents, vec![digest, digest]);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("the same exact transaction is eventually applied"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
        );
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_submissions, 1);
        assert_eq!(backend.release_intents, vec![digest, digest, digest]);
    }

    #[cfg(unix)]
    #[test]
    fn lost_release_response_restarts_from_the_same_journaled_transaction() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::HealthyDirectoryAdvance,
                LocationPollV1::HealthyDirectoryAdvance,
            ],
        );

        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        let before = store.load(operation_id).expect("release journal");
        assert_eq!(before.phase, PublicationPhaseV1::ReleaseSubmission);
        let exact_digest = before.release_submission_attempts[0]
            .intent
            .signed_transaction_digest;
        backend.drop_release_response_once = true;

        let error = engine
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("a lost response leaves the exact live intent durable");
        assert!(matches!(
            error,
            PublicationError::Backend(ref error)
                if error.code() == "RELEASE_COMMIT_RESPONSE_DROPPED"
        ));
        assert_eq!(store.load(operation_id).expect("unchanged journal"), before);
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_submissions, 1);
        let reopened_store =
            PublicationJournalStore::open(temp.path()).expect("reopen publication journal");
        let reopened_engine = PublicationEngine::new(&reopened_store);
        assert_eq!(
            reopened_engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("status-first recovery observes the exact applied transaction"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
        );
        assert_eq!(backend.release_submissions, 1);
        assert_eq!(backend.release_preparations, 1);
        assert_eq!(backend.release_intents, vec![exact_digest, exact_digest]);
    }

    #[cfg(unix)]
    #[test]
    fn stale_retirement_is_pending_in_readback() {
        for (script, guarded_phase) in [(
            vec![
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::Retired,
                LocationPollV1::RetiredRevisionOffset(2),
            ],
            PublicationPhaseV1::Readback,
        )] {
            let temp = tempdir().expect("state root");
            let store = PublicationJournalStore::open(temp.path()).expect("journal store");
            let engine = PublicationEngine::new(&store);
            let (request, broker) = request();
            let operation_id = engine
                .begin_detached(request)
                .expect("persist detached operation");
            let source = BytesSource(b"canonical-car".to_vec());
            let mut backend = LocationRecoveryBackend::new(broker, script);
            for step in 0..6 {
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
            }
            while store.load(operation_id).expect("phase journal").phase != guarded_phase {
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("advance to guarded phase");
            }
            let guarded = store.load(operation_id).expect("guarded journal");

            assert_eq!(
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("stale retirement remains retryable"),
                PublicationAdvanceV1::Pending(guarded_phase)
            );
            assert_eq!(
                store.load(operation_id).expect("unchanged journal"),
                guarded
            );

            assert_eq!(
                engine
                    .advance_once(operation_id, &source, &mut backend)
                    .expect("strictly later retirement permits rotation"),
                PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
            );
            let retired = store.load(operation_id).expect("retired journal");
            let attempt = &retired.archive_location_attempts[0];
            assert!(attempt.terminal.is_some());
            assert!(matches!(
                &attempt.terminal_floor,
                Some(PublicationArchiveLocationTerminalFloorV1::Replication(_))
            ));
            let reopened = PublicationJournalStore::open(temp.path())
                .expect("reopen journal store")
                .load(operation_id)
                .expect("revalidate terminal against durable replication floor");
            assert_eq!(reopened, retired);

            let mut regressed = retired;
            let registration = regressed.archive_location_attempts[0]
                .registration
                .as_ref()
                .expect("finalized registration");
            let stale_terminal = retired_location_terminal(registration);
            regressed.archive_location_attempts[0]
                .terminal
                .as_mut()
                .expect("persisted terminal")
                .finalized_page = stale_terminal.finalized_page;
            assert!(matches!(
                regressed.validate(),
                Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Replication,
                    ..
                })
            ));
        }
    }

    #[cfg(unix)]
    #[test]
    fn stale_post_rejection_retirement_preserves_the_latest_checkpoint() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::Retired,
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::RetiredRevisionOffset(2),
            ],
        );
        backend.reject_release = true;
        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        let guarded = store.load(operation_id).expect("release journal");

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist the exact rejected transaction outcome"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
        );
        let terminal = store.load(operation_id).expect("terminal release journal");
        assert_ne!(terminal, guarded);
        assert!(matches!(
            terminal.release_submission_attempts[0].outcome,
            Some(PublicationReleaseSubmissionOutcomeV1::Terminal(_))
        ));
        assert_eq!(backend.release_submissions, 1);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stale post-rejection retirement remains retryable"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            store.load(operation_id).expect("unchanged journal"),
            terminal
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("unchanged location cannot authorize a new signature"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("later post-rejection retirement permits rotation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(backend.release_submissions, 1);
    }

    #[cfg(unix)]
    #[test]
    fn rejected_release_never_resigns_against_stale_or_unchanged_location_state() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(1),
                LocationPollV1::HealthyRevisionOffset(2),
                LocationPollV1::HealthyRevisionOffset(2),
            ],
        );
        backend.reject_release = true;
        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        let guarded = store.load(operation_id).expect("release journal");

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist the authoritative rejection and exact absence"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
        );
        let terminal = store.load(operation_id).expect("terminal release journal");
        assert_ne!(terminal, guarded);
        assert_eq!(backend.release_submissions, 1);
        assert_eq!(backend.release_preparations, 1);

        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stale post-rejection page remains retryable"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            store.load(operation_id).expect("unchanged journal"),
            terminal
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("unchanged current location cannot authorize a successor"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(backend.release_submissions, 1);
        assert_eq!(backend.release_preparations, 1);
    }

    #[test]
    fn checkpoint_allows_higher_target_revision_at_equal_location_height_on_a_newer_page() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let previous = replication_checkpoint(&request, &registration, 3);
        let mut current = replication_checkpoint_with_revision_offset(&request, &registration, 1);
        current
            .finalized_page
            .items
            .iter_mut()
            .find(|location| location.location_id == registration.location_id())
            .expect("registered fixture location")
            .finalized_height = previous
            .location(&registration)
            .expect("previous fixture location")
            .finalized_height;

        assert_eq!(
            replication_checkpoint_progress(&request, &registration, &previous, &current)
                .expect("newer full page authenticates the higher local revision"),
            PublicationLocationProgressV1::Current
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejected_release_rotates_only_after_post_rejection_retirement_evidence() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(
            broker,
            [
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::Healthy,
                LocationPollV1::Retired,
            ],
        );
        backend.reject_release = true;
        for step in 0..8 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
        }
        assert_eq!(
            store.load(operation_id).expect("release journal").phase,
            PublicationPhaseV1::ReleaseSubmission
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist exact rejection before any location rotation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("post-rejection retirement permits rotation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let retired = store.load(operation_id).expect("post-rejection journal");
        assert!(retired.archive_location_attempts[0].terminal.is_some());
        assert!(retired.submission.is_none());
        assert!(retired.readbacks.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn expired_receipt_is_refreshed_only_before_registration_intent() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 1_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: false,
            return_conflicting_archive: false,
            registration_mode: ArchiveRecoveryMode::Commit,
        };

        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage first receipt");
        backend.now_ms = 1_101;
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("discard expired receipt before intent"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        );
        let reset = store.load(operation_id).expect("receipt reset journal");
        assert!(reset.staging_receipt.is_none());
        assert!(reset.archive_registration_attempts.is_empty());

        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage fresh receipt");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist intent for fresh receipt");
        assert_eq!(backend.staged_receipts.len(), 2);
        assert_eq!(backend.prepare_calls, 1);
    }

    #[cfg(unix)]
    #[test]
    fn expired_unsubmitted_intent_rotates_only_after_authoritative_terminal_absence() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 1_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: false,
            return_conflicting_archive: false,
            registration_mode: ArchiveRecoveryMode::ExpiredAbsent,
        };

        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage first receipt");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist first exact intent");
        let before_crash = store.load(operation_id).expect("first intent journal");
        let first_attempt = before_crash.archive_registration_attempts[0].clone();
        assert_eq!(backend.registration_calls, 0);

        backend.now_ms = first_attempt.intent.staging_receipt.payload.expires_at_ms + 1;
        let reopened = PublicationJournalStore::open(temp.path()).expect("reopen journal store");
        let resumed = PublicationEngine::new(&reopened);
        assert_eq!(
            resumed
                .advance_once(operation_id, &source, &mut backend)
                .expect("persist authoritative expiration and absence"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
        );
        let terminal = reopened
            .load(operation_id)
            .expect("terminal first generation");
        assert_eq!(terminal.archive_registration_attempts.len(), 1);
        assert_eq!(
            terminal.archive_registration_attempts[0].intent,
            first_attempt.intent
        );
        assert!(terminal.archive_registration_attempts[0].terminal.is_some());
        assert!(terminal.staging_receipt.is_none());

        resumed
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage replacement receipt");
        resumed
            .advance_once(operation_id, &source, &mut backend)
            .expect("append replacement exact intent");
        let replacement = reopened
            .load(operation_id)
            .expect("replacement generation journal");
        assert_eq!(replacement.archive_registration_attempts.len(), 2);
        assert_eq!(
            replacement.archive_registration_attempts[0],
            terminal.archive_registration_attempts[0]
        );
        assert!(
            replacement.archive_registration_attempts[1]
                .terminal
                .is_none()
        );
        assert_ne!(
            replacement.archive_registration_attempts[0]
                .intent
                .transaction_hash,
            replacement.archive_registration_attempts[1]
                .intent
                .transaction_hash
        );
        assert_eq!(backend.staged_receipts.len(), 2);
        assert_eq!(backend.prepare_calls, 2);
    }

    #[cfg(unix)]
    #[test]
    fn unknown_or_pending_application_state_never_rotates_the_exact_intent() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 1_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: false,
            return_conflicting_archive: false,
            registration_mode: ArchiveRecoveryMode::Pending,
        };

        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist exact intent");
        let before = store.load(operation_id).expect("intent journal");
        backend.now_ms = 10_000;
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("unknown application state remains pending"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ArchiveRegistration)
        );
        assert_eq!(store.load(operation_id).expect("unchanged journal"), before);
        assert_eq!(backend.staged_receipts.len(), 1);
        assert_eq!(backend.prepare_calls, 1);
    }

    #[cfg(unix)]
    #[test]
    fn archive_registration_attempt_generation_is_strictly_bounded() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
        journal.validation = Some(validation_evidence(&request));
        journal.phase = PublicationPhaseV1::SeedIngress;
        for generation in 1..=MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1 {
            let generation_u64 = u64::try_from(generation).expect("generation fits u64");
            let issued_at_ms = 1_000 + generation_u64 * 1_000;
            let receipt = signed_receipt_at(
                &request.receipt_binding(),
                &broker,
                issued_at_ms,
                issued_at_ms + 100,
            );
            let intent = registration_intent(operation_id, &request, receipt);
            let finalized_height = 60 + generation_u64;
            let terminal = PublicationArchiveRegistrationTerminalV1::registry_expired(
                &intent,
                Some(finalized_height),
                archive_absence_evidence(&request, finalized_height),
            );
            journal
                .archive_registration_attempts
                .push(PublicationArchiveRegistrationAttemptV1 {
                    generation: u8::try_from(generation).expect("generation fits u8"),
                    intent,
                    terminal: Some(terminal),
                });
        }
        journal
            .validate()
            .expect("maximum attempt generation is valid");
        store
            .write(&journal)
            .expect("persist maximum-generation journal");

        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 100_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: false,
            return_conflicting_archive: false,
            registration_mode: ArchiveRecoveryMode::Commit,
        };
        let error = PublicationEngine::new(&store)
            .advance_once(operation_id, &source, &mut backend)
            .expect_err("a ninth generation must not be staged");
        assert!(matches!(
            error,
            PublicationError::Backend(ref error)
                if error.code() == "ARCHIVE_REGISTRATION_ATTEMPT_LIMIT_REACHED"
                    && error.class() == PublicationBackendFailureClass::Permanent
        ));
        assert!(backend.staged_receipts.is_empty());

        let mut oversized = journal;
        let previous = oversized
            .archive_registration_attempts
            .last()
            .expect("maximum generation")
            .clone();
        let mut ninth = previous;
        ninth.generation = u8::try_from(MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1 + 1)
            .expect("ninth generation fits u8");
        oversized.archive_registration_attempts.push(ninth);
        assert!(matches!(
            oversized.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("attempt bound")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn archive_location_attempt_generation_is_bounded_and_encoded_below_journal_limit() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let receipt = signed_receipt(&request.receipt_binding(), &broker);
        let archive_intent = registration_intent(operation_id, &request, receipt.clone());
        let registered = registered_archive(&request, &broker, &archive_intent);
        let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
        journal.validation = Some(validation_evidence(&request));
        journal.staging_receipt = Some(receipt);
        journal
            .archive_registration_attempts
            .push(PublicationArchiveRegistrationAttemptV1::new(
                1,
                archive_intent,
            ));
        journal.registered_archive = Some(registered.clone());
        journal.phase = PublicationPhaseV1::ArchiveRegistration;

        for generation in 1..=MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1 {
            let generation = u8::try_from(generation).expect("generation fits u8");
            let registration =
                location_registration_generation(operation_id, &request, &registered, generation);
            let replication = replication_checkpoint(&request, &registration, 3);
            let terminal = retired_location_terminal(&registration);
            journal
                .archive_location_attempts
                .push(PublicationArchiveLocationAttemptV1 {
                    generation,
                    intent: registration.intent.clone(),
                    registration: Some(registration),
                    terminal: Some(terminal),
                    terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Replication(
                        replication,
                    )),
                });
        }
        journal
            .validate()
            .expect("maximum location history is valid");
        let encoded = norito::encode_canonical(&journal).expect("encode bounded journal");
        assert!(encoded.len() <= MAX_JOURNAL_BYTES_USIZE);
        store.write(&journal).expect("persist bounded journal");
        let persisted_bytes = fs::metadata(temp.path().join(journal_relative_path(operation_id)))
            .expect("bounded journal metadata")
            .len();
        assert!(persisted_bytes <= MAX_JOURNAL_BYTES);
        assert_eq!(
            store.load(operation_id).expect("reload bounded journal"),
            journal
        );

        let mut rewritten = journal.clone();
        rewritten.archive_location_attempts[0]
            .terminal
            .as_mut()
            .expect("first terminal")
            .transaction_hash = [0xee; 32];
        assert!(!archive_location_attempts_are_append_only(
            &journal.archive_location_attempts,
            &rewritten.archive_location_attempts,
        ));

        let generation =
            u8::try_from(MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1 + 1).expect("ninth fits u8");
        let registration =
            location_registration_generation(operation_id, &request, &registered, generation);
        let replication = replication_checkpoint(&request, &registration, 3);
        let terminal = retired_location_terminal(&registration);
        let mut oversized = journal;
        oversized
            .archive_location_attempts
            .push(PublicationArchiveLocationAttemptV1 {
                generation,
                intent: registration.intent.clone(),
                registration: Some(registration),
                terminal: Some(terminal),
                terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Replication(
                    replication,
                )),
            });
        assert!(matches!(
            oversized.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("archive-location attempt bound")
        ));
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test covers substitution at each terminal-to-replacement snapshot boundary"
    )]
    fn terminal_and_replacement_pages_reject_same_snapshot_or_revision_substitution() {
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let receipt = signed_receipt(&request.receipt_binding(), &broker);
        let archive_intent = registration_intent(operation_id, &request, receipt.clone());
        let registered = registered_archive(&request, &broker, &archive_intent);
        let first = location_registration_generation(operation_id, &request, &registered, 1);
        let first_terminal = retired_location_terminal(&first);
        let second = location_registration_generation(operation_id, &request, &registered, 2);
        let second_attempt = PublicationArchiveLocationAttemptV1 {
            generation: 2,
            intent: second.intent.clone(),
            registration: None,
            terminal: None,
            terminal_floor: None,
        };
        let prior_location_ids = [first.location_id()];
        let active_second_attempt = PublicationArchiveLocationAttemptV1 {
            generation: 2,
            intent: second.intent.clone(),
            registration: Some(second.clone()),
            terminal: None,
            terminal_floor: None,
        };
        let mut equal_index_retirement = retired_location_terminal(&second);
        equal_index_retirement
            .finalized_page
            .snapshot
            .index_revision = second.finalized_page.snapshot.index_revision;
        equal_index_retirement
            .validate_for(
                operation_id,
                &request,
                &registered,
                &active_second_attempt,
                &prior_location_ids,
                &PublicationArchiveLocationTerminalFloorV1::Registered,
            )
            .expect("retirement may preserve the resolver index revision");
        let mut lower_index_retirement = equal_index_retirement;
        lower_index_retirement
            .finalized_page
            .snapshot
            .index_revision -= 1;
        assert!(
            lower_index_retirement
                .validate_for(
                    operation_id,
                    &request,
                    &registered,
                    &active_second_attempt,
                    &prior_location_ids,
                    &PublicationArchiveLocationTerminalFloorV1::Registered,
                )
                .is_err()
        );

        let exact_expiry = PublicationArchiveLocationTerminalV1 {
            transaction_hash: second.intent.transaction_hash,
            reason: PublicationArchiveLocationTerminalReasonV1::RegistryExpired {
                block_height: None,
            },
            finalized_page: second.intent.prepared_page.clone(),
        };
        exact_expiry
            .validate_for(
                operation_id,
                &request,
                &registered,
                &second_attempt,
                &prior_location_ids,
                &PublicationArchiveLocationTerminalFloorV1::Prepared,
            )
            .expect("unchanged prepared snapshot proves exact expiry absence");

        let mut same_snapshot_substituted = exact_expiry.clone();
        let mut unrelated = second.location().expect("second location fixture").clone();
        unrelated.location_id = MusubiArchiveLocationIdV1::new([0xe5; 32]);
        unrelated.finalized_height = same_snapshot_substituted
            .finalized_page
            .snapshot
            .finalized_height;
        unrelated.revision = same_snapshot_substituted
            .finalized_page
            .archive
            .location_revision
            + 1;
        same_snapshot_substituted
            .finalized_page
            .archive
            .location_revision += 1;
        same_snapshot_substituted
            .finalized_page
            .archive
            .location_ids = vec![unrelated.location_id];
        same_snapshot_substituted.finalized_page.items = vec![unrelated.clone()];
        assert!(
            same_snapshot_substituted
                .validate_for(
                    operation_id,
                    &request,
                    &registered,
                    &second_attempt,
                    &prior_location_ids,
                    &PublicationArchiveLocationTerminalFloorV1::Prepared,
                )
                .is_err()
        );

        let mut same_revision_substituted = exact_expiry;
        same_revision_substituted
            .finalized_page
            .snapshot
            .finalized_height += 1;
        same_revision_substituted
            .finalized_page
            .snapshot
            .finalized_block_hash = [0xe6; 32];
        unrelated.revision = same_revision_substituted
            .finalized_page
            .archive
            .location_revision;
        unrelated.finalized_height = same_revision_substituted
            .finalized_page
            .snapshot
            .finalized_height;
        same_revision_substituted
            .finalized_page
            .archive
            .location_ids = vec![unrelated.location_id];
        same_revision_substituted.finalized_page.items = vec![unrelated.clone()];
        assert!(
            same_revision_substituted
                .validate_for(
                    operation_id,
                    &request,
                    &registered,
                    &second_attempt,
                    &prior_location_ids,
                    &PublicationArchiveLocationTerminalFloorV1::Prepared,
                )
                .is_err()
        );

        let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
        journal.validation = Some(validation_evidence(&request));
        journal.staging_receipt = Some(receipt);
        journal
            .archive_registration_attempts
            .push(PublicationArchiveRegistrationAttemptV1::new(
                1,
                archive_intent,
            ));
        journal.registered_archive = Some(registered);
        journal.archive_location_attempts = vec![
            PublicationArchiveLocationAttemptV1 {
                generation: 1,
                intent: first.intent.clone(),
                registration: Some(first),
                terminal: Some(first_terminal),
                terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Registered),
            },
            second_attempt,
        ];
        journal.phase = PublicationPhaseV1::ArchiveRegistration;
        journal
            .validate()
            .expect("exact terminal-to-prepared checkpoint");

        let mut replacement_substituted = journal;
        let prepared = &mut replacement_substituted.archive_location_attempts[1]
            .intent
            .prepared_page;
        prepared.snapshot.finalized_height += 1;
        prepared.snapshot.finalized_block_hash = [0xe7; 32];
        unrelated.revision = prepared.archive.location_revision;
        unrelated.finalized_height = prepared.snapshot.finalized_height;
        prepared.archive.location_ids = vec![unrelated.location_id];
        prepared.items = vec![unrelated];
        assert!(matches!(
            replacement_substituted.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("regressed prior terminal finality")
        ));
        let encoded =
            norito::encode_canonical(&replacement_substituted).expect("encode substituted journal");
        let temp = tempdir().expect("substituted journal root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        store
            .root
            .replace(&journal_relative_path(operation_id), &encoded)
            .expect("persist substituted restart image");
        assert!(matches!(
            store.load(operation_id),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("regressed prior terminal finality")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn conflicting_authoritative_archive_never_reaches_pin_coordination() {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = ArchiveRecoveryBackend {
            broker,
            now_ms: 1_000,
            staged_receipts: Vec::new(),
            prepare_calls: 0,
            registration_calls: 0,
            pin_calls: 0,
            archive_committed: false,
            drop_commit_response_once: false,
            return_conflicting_archive: true,
            registration_mode: ArchiveRecoveryMode::Commit,
        };

        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage");
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist intent");
        assert!(matches!(
            engine.advance_once(operation_id, &source, &mut backend),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::ArchiveRegistration,
                ..
            })
        ));
        let unchanged = store.load(operation_id).expect("unchanged intent journal");
        assert_eq!(unchanged.archive_registration_attempts.len(), 1);
        assert!(
            unchanged.archive_registration_attempts[0]
                .terminal
                .is_none()
        );
        assert!(unchanged.registered_archive.is_none());
        assert_eq!(backend.pin_calls, 0);
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
            substitute_all_readbacks: false,
            readback_backend_failure: None,
            readback_providers: Vec::new(),
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
        assert_eq!(replication_wait.revision, 7);

        assert_eq!(
            engine
                .resume(operation_id, &source, &mut backend)
                .expect("resume through AMX"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::FinalVerification)
        );
        assert_eq!(backend.submissions, 1);
        let finality_wait = store.load(operation_id).expect("finality journal");
        assert_eq!(finality_wait.phase, PublicationPhaseV1::FinalVerification);
        assert_eq!(finality_wait.revision, 10);

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
    fn trait_backed_readback_skips_corrupt_provider_and_uses_later_quorum() {
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
            substitute_all_readbacks: false,
            readback_backend_failure: None,
            readback_providers: Vec::new(),
            submissions: 0,
        };
        assert!(matches!(
            engine
                .publish(request, &source, &mut backend)
                .expect("later providers satisfy the readback floor"),
            PublicationAdvanceV1::Complete(_)
        ));
        assert_eq!(backend.submissions, 1);
        assert_eq!(
            backend.readback_providers,
            vec![
                ProviderId::new([1; 32]),
                ProviderId::new([2; 32]),
                ProviderId::new([3; 32]),
            ]
        );
        let journal = store.load(operation_id).expect("completed journal");
        assert_eq!(
            journal
                .readbacks
                .iter()
                .map(|readback| readback.provider)
                .collect::<Vec<_>>(),
            vec![ProviderId::new([2; 32]), ProviderId::new([3; 32])]
        );
        journal.validate().expect("fallback journal remains valid");
    }

    #[cfg(unix)]
    #[test]
    fn trait_backed_invalid_readback_quorum_stops_before_amx_without_journal_mutation() {
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
            substitute_readback: false,
            substitute_all_readbacks: true,
            readback_backend_failure: None,
            readback_providers: Vec::new(),
            submissions: 0,
        };

        let error = engine
            .publish(request, &source, &mut backend)
            .expect_err("invalid providers cannot authorize AMX submission");
        let PublicationError::InvalidEvidence { phase, reason } = error else {
            panic!("substituted provider evidence must retain its integrity classification");
        };
        assert_eq!(phase, PublicationPhaseV1::Readback);
        assert_eq!(reason, "provider readback evidence was substituted");
        assert_eq!(backend.submissions, 0);
        assert_eq!(
            backend.readback_providers,
            vec![
                ProviderId::new([1; 32]),
                ProviderId::new([2; 32]),
                ProviderId::new([3; 32]),
            ]
        );
        let unchanged = store.load(operation_id).expect("readback journal");
        assert_eq!(unchanged.phase, PublicationPhaseV1::Readback);
        assert!(unchanged.readbacks.is_empty());
        assert!(unchanged.release_submission_attempts.is_empty());
        assert!(unchanged.submission.is_none());
        unchanged
            .validate()
            .expect("failed readbacks leave a valid journal");

        let error = engine
            .resume(operation_id, &source, &mut backend)
            .expect_err("retry still lacks two valid providers");
        assert!(matches!(
            error,
            PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Readback,
                ..
            }
        ));
        assert_eq!(
            store.load(operation_id).expect("retried readback journal"),
            unchanged
        );
        assert_eq!(backend.submissions, 0);
    }

    #[cfg(unix)]
    #[test]
    fn trait_backed_readback_exhaustion_preserves_backend_failure_class_and_code() {
        for (class, code) in [
            (
                PublicationBackendFailureClass::Permanent,
                "READBACK_AUTHENTICATION_FAILED",
            ),
            (
                PublicationBackendFailureClass::Retryable,
                "READBACK_PROVIDER_TIMEOUT",
            ),
        ] {
            let temp = tempdir().expect("state root");
            let store = PublicationJournalStore::open(temp.path()).expect("journal store");
            let engine = PublicationEngine::new(&store);
            let (request, broker) = request();
            let operation_id = request.operation_id();
            let source = BytesSource(b"canonical-car".to_vec());
            let failure = match class {
                PublicationBackendFailureClass::Retryable => {
                    PublicationBackendError::retryable(code)
                }
                PublicationBackendFailureClass::Permanent => {
                    PublicationBackendError::permanent(code)
                }
            };
            let mut backend = CompleteBackend {
                broker,
                replication_pending_once: false,
                finality_pending_once: false,
                substitute_readback: false,
                substitute_all_readbacks: true,
                readback_backend_failure: Some((ProviderId::new([1; 32]), failure)),
                readback_providers: Vec::new(),
                submissions: 0,
            };

            let error = engine
                .publish(request, &source, &mut backend)
                .expect_err("one backend failure plus invalid evidence cannot authorize AMX");
            let PublicationError::Backend(error) = error else {
                panic!("backend failure must retain its redacted classification");
            };
            assert_eq!(error.class(), class);
            assert_eq!(error.code(), code);
            assert_eq!(backend.submissions, 0);
            assert_eq!(
                backend.readback_providers,
                vec![
                    ProviderId::new([1; 32]),
                    ProviderId::new([2; 32]),
                    ProviderId::new([3; 32]),
                ]
            );
            let unchanged = store.load(operation_id).expect("readback journal");
            assert_eq!(unchanged.phase, PublicationPhaseV1::Readback);
            assert!(unchanged.readbacks.is_empty());
            assert!(unchanged.release_submission_attempts.is_empty());
            assert!(unchanged.submission.is_none());
            unchanged
                .validate()
                .expect("failed readbacks leave a valid journal");
        }
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
            now_ms: 1_500,
            receipt_window: None,
            prepare_calls: 0,
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
        let expected_receipt = registration
            .intent
            .prepared_page
            .archive
            .staging_receipt
            .clone();
        let intent =
            registration_intent(request.operation_id(), &request, expected_receipt.clone());
        let registered = PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: intent.transaction_hash,
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3C; 32],
                index_revision: 2,
            },
            archive: registration.intent.prepared_page.archive.clone(),
        };
        let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
        journal.validation = Some(validation_evidence(&request));
        journal.staging_receipt = Some(expected_receipt);
        journal
            .archive_registration_attempts
            .push(PublicationArchiveRegistrationAttemptV1::new(1, intent));
        journal.registered_archive = Some(registered);
        journal
            .archive_location_attempts
            .push(PublicationArchiveLocationAttemptV1 {
                generation: 1,
                intent: registration.intent.clone(),
                registration: Some(registration),
                terminal: None,
                terminal_floor: None,
            });
        journal.phase = PublicationPhaseV1::Replication;
        journal
            .validate()
            .expect("registration must retain the exact staged receipt");

        let mut replayed_binding = request.receipt_binding();
        replayed_binding.nonce = [0xEE; 32];
        journal
            .archive_location_attempts
            .last_mut()
            .and_then(|attempt| attempt.registration.as_mut())
            .expect("archive registration")
            .finalized_page
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
    fn journal_rejects_a_refreshed_receipt_after_archive_registration() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let registered_receipt = registration
            .intent
            .prepared_page
            .archive
            .staging_receipt
            .clone();
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
        let intent =
            registration_intent(journal.operation_id, &journal.request, registered_receipt);
        journal.registered_archive = Some(PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: intent.transaction_hash,
            chain_id: journal.request.chain_id.clone(),
            genesis_block_hash: journal.request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3C; 32],
                index_revision: 2,
            },
            archive: registration.intent.prepared_page.archive.clone(),
        });
        journal
            .archive_registration_attempts
            .push(PublicationArchiveRegistrationAttemptV1::new(1, intent));
        journal
            .archive_location_attempts
            .push(PublicationArchiveLocationAttemptV1 {
                generation: 1,
                intent: registration.intent.clone(),
                registration: Some(registration),
                terminal: None,
                terminal_floor: None,
            });
        journal.phase = PublicationPhaseV1::Replication;
        assert!(matches!(
            journal.validate(),
            Err(PublicationError::InvalidJournal(ref reason))
                if reason.contains("exact staging receipt")
        ));
    }

    #[test]
    fn replication_requires_three_exact_finalized_providers() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let intent = registration_intent(
            request.operation_id(),
            &request,
            registration
                .intent
                .prepared_page
                .archive
                .staging_receipt
                .clone(),
        );
        let registered = PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: intent.transaction_hash,
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3C; 32],
                index_revision: 2,
            },
            archive: registration.intent.prepared_page.archive.clone(),
        };
        registration
            .validate_for(request.operation_id(), &request, &registered, &[])
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

        let mut stale = exact.clone();
        stale.revision -= 1;
        assert!(matches!(
            validate_replication(&request, &registration, &stale),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));

        let mut equal_revision_substitution = exact.clone();
        equal_revision_substitution.renew_after_epoch += 1;
        assert!(matches!(
            validate_replication(&request, &registration, &equal_revision_substitution),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));

        let mut renewed_registration = registration.clone();
        renewed_registration.intent.pin_manifest = ManifestDigest::new([0xD1; 32]);
        renewed_registration.intent.replication_order = ReplicationOrderId::new([0xD2; 32]);
        renewed_registration.intent.renew_after_epoch = 15;
        renewed_registration.intent.expires_at_epoch = 30;
        let mut renewed = location(&request, &renewed_registration, 3);
        renewed.revision = 3;
        renewed.finalized_height = 71;
        validate_replication(&request, &registration, &renewed)
            .expect("same stable location may carry an authenticated finalized renewal");
        assert_eq!(
            location_progress(&renewed, &renewed).expect("exact renewal is current"),
            PublicationLocationProgressV1::Current
        );
        let mut newer = renewed.clone();
        newer.revision += 1;
        newer.finalized_height += 1;
        assert_eq!(
            location_progress(&renewed, &newer).expect("newer renewal is current"),
            PublicationLocationProgressV1::Current
        );
        validate_replication(&request, &registration, &newer)
            .expect("newer authenticated renewal remains selectable");
        let mut higher_revision_lower_height = newer;
        higher_revision_lower_height.finalized_height = renewed.finalized_height - 1;
        assert!(matches!(
            location_progress(&renewed, &higher_revision_lower_height),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));

        let mut substituted = exact;
        substituted.provider_attestation_set_digest =
            MusubiProviderBundleAttestationSetDigestV1::new([0xEE; 32]);
        assert!(matches!(
            validate_replication(&request, &registration, &substituted),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test checks the complete revision and snapshot substitution matrix for location checkpoints"
    )]
    fn archive_location_checkpoints_reject_revision_and_snapshot_substitution() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let archive_intent = registration_intent(
            request.operation_id(),
            &request,
            registration
                .intent
                .prepared_page
                .archive
                .staging_receipt
                .clone(),
        );
        let registered = PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: archive_intent.transaction_hash,
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: registration.intent.prepared_page.snapshot,
            archive: registration.intent.prepared_page.archive.clone(),
        };
        registration
            .validate_for(request.operation_id(), &request, &registered, &[])
            .expect("baseline archive-location application");
        registration
            .validate_polled_page(&request, &registration.finalized_page)
            .expect("baseline finalized location page");

        let mut target_revision_regressed = registration.clone();
        target_revision_regressed.finalized_page.items[0].revision =
            registration.intent.expected_location_revision;
        assert!(
            target_revision_regressed
                .validate_for(request.operation_id(), &request, &registered, &[])
                .is_err()
        );

        let mut first_application_substituted = registration.clone();
        first_application_substituted.finalized_page.items[0].pin_manifest =
            ManifestDigest::new([0xe1; 32]);
        assert!(
            first_application_substituted
                .validate_for(request.operation_id(), &request, &registered, &[])
                .is_err()
        );

        let mut first_application_not_healthy = registration.clone();
        first_application_not_healthy.finalized_page.items[0].state =
            MusubiArchiveLocationStateV1::Degraded;
        assert!(
            first_application_not_healthy
                .validate_for(request.operation_id(), &request, &registered, &[])
                .is_err()
        );

        let mut first_application_wrong_height = registration.clone();
        first_application_wrong_height.applied_height -= 1;
        assert!(
            first_application_wrong_height
                .validate_for(request.operation_id(), &request, &registered, &[])
                .is_err()
        );

        let mut archive_revision_regressed = registration.finalized_page.clone();
        archive_revision_regressed.snapshot.finalized_height += 1;
        archive_revision_regressed.snapshot.finalized_block_hash = [0xe2; 32];
        archive_revision_regressed.archive.location_revision -= 1;
        archive_revision_regressed.archive.location_ids.clear();
        archive_revision_regressed.items.clear();
        assert!(
            registration
                .validate_polled_page(&request, &archive_revision_regressed)
                .is_err()
        );

        let mut equal_archive_revision_substituted = registration.finalized_page.clone();
        equal_archive_revision_substituted.snapshot.finalized_height += 1;
        equal_archive_revision_substituted
            .snapshot
            .finalized_block_hash = [0xe3; 32];
        equal_archive_revision_substituted
            .archive
            .location_ids
            .clear();
        equal_archive_revision_substituted.items.clear();
        assert!(
            registration
                .validate_polled_page(&request, &equal_archive_revision_substituted)
                .is_err()
        );

        let mut same_snapshot_higher_revision = registration.finalized_page.clone();
        same_snapshot_higher_revision.archive.location_revision += 1;
        same_snapshot_higher_revision.items[0].revision += 1;
        assert!(
            registration
                .validate_polled_page(&request, &same_snapshot_higher_revision)
                .is_err()
        );

        let mut item_ahead_of_archive = registration.finalized_page.clone();
        item_ahead_of_archive.items[0].revision += 1;
        assert!(
            registration
                .validate_polled_page(&request, &item_ahead_of_archive)
                .is_err()
        );

        let mut same_snapshot_archive_substitution = registration.intent.prepared_page.clone();
        same_snapshot_archive_substitution.archive.location_revision += 1;
        assert!(
            validate_archive_location_page(
                &request,
                &registered,
                &same_snapshot_archive_substitution,
            )
            .is_err()
        );

        let mut registered_current = registered;
        registered_current.snapshot = registration.finalized_page.snapshot;
        registered_current.archive = registration.finalized_page.archive.clone();
        let mut later_archive_revision_regression = registration.finalized_page.clone();
        later_archive_revision_regression.snapshot.finalized_height += 1;
        later_archive_revision_regression
            .snapshot
            .finalized_block_hash = [0xe4; 32];
        later_archive_revision_regression.archive.location_revision -= 1;
        later_archive_revision_regression
            .archive
            .location_ids
            .clear();
        later_archive_revision_regression.items.clear();
        assert!(
            validate_archive_location_page(
                &request,
                &registered_current,
                &later_archive_revision_regression,
            )
            .is_err()
        );
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
    fn release_preparation_requires_a_sorted_distinct_location_provider_subset() {
        let (request, broker) = request();
        let registration = registration(&request, &broker);
        let replication = replication_checkpoint(&request, &registration, 3);
        let location = replication
            .location(&registration)
            .expect("fixture location");
        let readback_for = |provider| PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        };
        let later_subset = vec![
            readback_for(location.providers[1]),
            readback_for(location.providers[2]),
        ];
        PublicationReleasePreparationFloorV1::try_new(
            registration.intent.generation,
            replication.clone(),
            later_subset.clone(),
            &request,
            &registration,
        )
        .expect("any sorted two-provider location subset is valid");

        let assert_rejected = |readbacks| {
            assert!(matches!(
                PublicationReleasePreparationFloorV1::try_new(
                    registration.intent.generation,
                    replication.clone(),
                    readbacks,
                    &request,
                    &registration,
                ),
                Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Readback,
                    ref reason,
                }) if reason
                    == "provider readbacks were not a strictly ordered distinct location-provider subset"
            ));
        };

        let mut duplicate = later_subset.clone();
        duplicate[1] = duplicate[0].clone();
        assert_rejected(duplicate);

        let mut unsorted = later_subset.clone();
        unsorted.swap(0, 1);
        assert_rejected(unsorted);

        let mut nonmember = later_subset;
        nonmember[1].provider = ProviderId::new([0xFE; 32]);
        assert_rejected(nonmember);
    }

    #[test]
    fn amx_and_final_index_evidence_bind_the_exact_release() {
        let (request, _) = request();
        let operation_id = request.operation_id();
        let instruction = request.publish_instruction();
        let exact_submission =
            PublicationAmxSubmissionV1::new(operation_id, &instruction, [0x71; 32], 80);
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
        let mut heightless_submission = exact_submission;
        heightless_submission.applied_height = 0;
        assert!(
            heightless_submission
                .validate_for(operation_id, &instruction)
                .is_err()
        );

        let exact_final = final_evidence(&request);
        exact_final
            .validate_for(&request, &exact_submission)
            .expect("exact finalized home and universal records");
        let mut later_unrelated_snapshot = exact_final.clone();
        later_unrelated_snapshot.snapshot.finalized_height += 1;
        later_unrelated_snapshot.snapshot.finalized_block_hash = [0x75; 32];
        later_unrelated_snapshot.snapshot.index_revision += 1;
        later_unrelated_snapshot
            .validate_for(&request, &exact_submission)
            .expect("an unrelated later registry revision must not invalidate the exact row");
        let mut older_storage_projection = exact_final.clone();
        older_storage_projection
            .universal_release
            .selection
            .storage
            .index_revision -= 1;
        older_storage_projection
            .validate_for(&request, &exact_submission)
            .expect("a row may retain an older valid storage projection");
        let mut future_storage_projection = exact_final.clone();
        future_storage_projection
            .universal_release
            .selection
            .storage
            .index_revision += 1;
        assert!(
            future_storage_projection
                .validate_for(&request, &exact_submission)
                .is_err()
        );
        let mut mismatched_tip_storage = exact_final.clone();
        mismatched_tip_storage
            .universal_release
            .selection
            .storage
            .finalized_height = mismatched_tip_storage.snapshot.finalized_height;
        mismatched_tip_storage
            .universal_release
            .selection
            .storage
            .finalized_block_hash = [0x77; 32];
        assert!(
            mismatched_tip_storage
                .validate_for(&request, &exact_submission)
                .is_err()
        );
        let mut pre_application_snapshot = exact_final.clone();
        pre_application_snapshot.snapshot.finalized_height = exact_submission.applied_height - 1;
        pre_application_snapshot.snapshot.finalized_block_hash = [0x76; 32];
        assert!(
            pre_application_snapshot
                .validate_for(&request, &exact_submission)
                .is_err()
        );
        let mut wrong_chain = exact_final.clone();
        wrong_chain.chain_id = ChainId::from("another-musubi-chain");
        assert!(
            wrong_chain
                .validate_for(&request, &exact_submission)
                .is_err()
        );
        let mut wrong_genesis = exact_final.clone();
        wrong_genesis.genesis_block_hash = [0x74; 32];
        assert!(
            wrong_genesis
                .validate_for(&request, &exact_submission)
                .is_err()
        );
        let mut substituted_index = exact_final;
        substituted_index.universal_release.source_digest = MusubiContentDigestV1::new([0x73; 32]);
        assert!(
            substituted_index
                .validate_for(&request, &exact_submission)
                .is_err()
        );
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
    fn empty_chain_identity_is_rejected_before_publication_request_construction() {
        assert!(ChainId::try_from(String::new()).is_err());
    }
}
