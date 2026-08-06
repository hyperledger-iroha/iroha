//! Peer-to-peer proxy envelopes for Torii ingress routing.

use std::fmt;

use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    ChainId,
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use norito::codec::{Decode, Encode};

/// Schema version for Torii proxy requests with generation-bound durable admission.
pub const TORII_PROXY_REQUEST_VERSION_V5: u16 = 5;
/// Schema version for peer-to-peer Torii proxy responses.
pub const TORII_PROXY_RESPONSE_VERSION_V1: u16 = 1;
/// Maximum participant routes admitted in one Native AMX routing-plan hint.
pub const TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1: usize =
    crate::native_amx::MAX_NATIVE_AMX_PARTICIPANT_LEGS;
/// Current first-release QueuePlan global-admission binding layout.
pub const QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2: u16 = 2;
/// Current first-release QueuePlan authority-attestation layout.
pub const QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2: u16 = 2;
/// Current first-release QueuePlan admission-certificate layout.
pub const QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2: u16 = 2;
/// Schema version for peer-to-peer publication of a certified QueuePlan admission.
pub const QUEUE_PLAN_ADMISSION_PUBLICATION_VERSION_V1: u16 = 1;

const QUEUE_PLAN_ADMISSION_CHAIN_DOMAIN_V2: &[u8] = b"iroha:torii:queue-plan-admission-chain:v2\0";
const QUEUE_PLAN_ADMISSION_BINDING_DOMAIN_V2: &[u8] =
    b"iroha:torii:queue-plan-admission-binding:v2\0";
const QUEUE_PLAN_ADMISSION_ATTESTATION_DOMAIN_V2: &[u8] =
    b"iroha:torii:queue-plan-admission-attestation:v2\0";
const QUEUE_PLAN_SYNCED_REQUEST_DOMAIN_V5: &str = "torii:proxy:queue-plan-synced:v5";

/// Return the chain identity carried by every QueuePlan admission binding.
#[must_use]
pub fn queue_plan_admission_chain_id_digest(chain_id: &ChainId) -> Hash {
    Hash::new_from_chunks(&[
        QUEUE_PLAN_ADMISSION_CHAIN_DOMAIN_V2,
        chain_id.as_str().as_bytes(),
    ])
}

/// Derive the deterministic QueuePlanSynced request identity shared by every ingress.
///
/// This pure kernel deliberately excludes connection/session identity. Every responsive ingress
/// therefore presents the same semantic request identity for one chain and entrypoint while
/// retaining its own process-local reply route.
#[must_use]
pub fn queue_plan_synced_request_id(
    chain_id: &ChainId,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Hash {
    queue_plan_synced_request_id_from_chain_digest(
        queue_plan_admission_chain_id_digest(chain_id),
        entrypoint_hash,
    )
}

/// Derive the deterministic QueuePlanSynced request identity from its durable projection.
///
/// Binding the request to the persisted chain digest lets journal replay and certificate
/// validation recompute the same semantic identity without recovering or trusting a raw chain
/// string. Delivery ordinals and connection tenures remain deliberately excluded.
#[must_use]
pub fn queue_plan_synced_request_id_from_chain_digest(
    chain_id_digest: Hash,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Hash {
    Hash::new(
        norito::encode_canonical(&(
            QUEUE_PLAN_SYNCED_REQUEST_DOMAIN_V5,
            chain_id_digest,
            entrypoint_hash,
        ))
        .expect("deterministic QueuePlanSynced request identity must encode"),
    )
}

/// Globally unique registry key for one transaction-entrypoint admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct QueuePlanAdmissionRegistryKeyV2 {
    /// Registry-key layout version.
    pub version: u16,
    /// Chain that owns the entrypoint.
    pub chain_id_digest: Hash,
    /// Typed canonical transaction-entrypoint identity.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
}

/// Immutable value claimed by a QueuePlan global-admission registry key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct QueuePlanAdmissionRegistryValueV2 {
    /// Registry-value layout version.
    pub version: u16,
    /// Domain-separated hash of the complete admission binding.
    pub binding_hash: Hash,
}

/// One exact queue-journal claim shared by every authority in an admission certificate.
///
/// The complete context carries ordered rosters for every coordinator/participant leg. The
/// journal digest covers the exact transaction wire, routing plan, context, canonical ingress
/// timestamp, chain digest, and deterministic request identity. Authorities never substitute a
/// locally sampled timestamp or independently reconstructed claim.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanAdmissionBindingV2 {
    /// Binding layout version.
    pub version: u16,
    /// Domain-separated chain identity.
    pub chain_id_digest: Hash,
    /// Deterministic QueuePlanSynced proxy request identity.
    pub request_id: Hash,
    /// Typed canonical transaction-entrypoint identity.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Real signed-transaction identity when the entrypoint contains one.
    pub signed_transaction_hash: Option<HashOf<SignedTransaction>>,
    /// Complete canonical routing-plan digest.
    pub routing_plan_digest: Hash,
    /// Exact lifecycle, incarnation, and ordered per-leg authority context.
    pub admission_context: crate::queue::QueuePlanAdmissionContextV2,
    /// Canonical ingress timestamp persisted identically by every authority.
    pub enqueue_timestamp_ms: u64,
    /// Exact queue-plan journal record layout.
    pub queue_plan_journal_version: u16,
    /// Exact durable-claim layout returned by queue admission.
    pub durable_admission_version: u16,
    /// Domain-separated digest of the exact canonical journal record.
    pub journal_record_digest: Hash,
}

impl QueuePlanAdmissionBindingV2 {
    /// Build the single exact binding an ingress node sends to every authority.
    ///
    /// # Errors
    /// Returns an error when the supplied context is not canonical for the routing plan or the
    /// exact version-4 journal record cannot be encoded.
    pub fn new(
        chain_id: &ChainId,
        transaction: &TransactionEntrypoint,
        routing_plan: &crate::queue::RoutingPlan,
        admission_context: crate::queue::QueuePlanAdmissionContextV2,
        enqueue_timestamp_ms: u64,
    ) -> Result<Self, String> {
        admission_context.validate_for_routing_plan(routing_plan)?;
        let chain_id_digest = queue_plan_admission_chain_id_digest(chain_id);
        let global_admission_identity = crate::queue::QueuePlanGlobalAdmissionIdentityV2 {
            version: crate::queue::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2,
            chain_id_digest,
            request_id: queue_plan_synced_request_id_from_chain_digest(
                chain_id_digest,
                transaction.hash(),
            ),
        };
        let journal_record_digest = crate::queue::queue_plan_journal_record_claim_digest(
            transaction.clone(),
            routing_plan.clone(),
            admission_context.clone(),
            enqueue_timestamp_ms,
            Some(global_admission_identity.clone()),
        )
        .map_err(|error| format!("QueuePlan journal claim cannot be encoded: {error}"))?;
        Ok(Self {
            version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            chain_id_digest: global_admission_identity.chain_id_digest,
            request_id: global_admission_identity.request_id,
            entrypoint_hash: transaction.hash(),
            signed_transaction_hash: signed_transaction_hash(transaction),
            routing_plan_digest: routing_plan.digest(),
            admission_context,
            enqueue_timestamp_ms,
            queue_plan_journal_version: crate::queue::QUEUE_PLAN_JOURNAL_VERSION,
            durable_admission_version: crate::queue::QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V2,
            journal_record_digest,
        })
    }

    /// Reconstruct a shared binding from one exact locally durable queue claim.
    ///
    /// # Errors
    /// Returns an error for ordinary claims without a global identity or for any inconsistent
    /// version, transaction, routing, context, or journal field.
    pub fn try_from_durable_admission(
        durable: &crate::queue::QueuePlanDurableAdmissionV2,
    ) -> Result<Self, String> {
        if durable.version != crate::queue::QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V2 {
            return Err("QueuePlan durable-admission version is unsupported".to_owned());
        }
        let identity = durable
            .global_admission_identity
            .as_ref()
            .ok_or_else(|| "QueuePlan durable admission has no global identity".to_owned())?;
        if identity.version != crate::queue::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2 {
            return Err("QueuePlan global-admission identity version is unsupported".to_owned());
        }
        let binding = Self {
            version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            chain_id_digest: identity.chain_id_digest,
            request_id: identity.request_id,
            entrypoint_hash: durable.entrypoint_hash.clone(),
            signed_transaction_hash: durable.signed_transaction_hash.clone(),
            routing_plan_digest: durable.routing_plan.digest(),
            admission_context: durable.context.clone(),
            enqueue_timestamp_ms: durable.enqueue_timestamp_ms,
            queue_plan_journal_version: crate::queue::QUEUE_PLAN_JOURNAL_VERSION,
            durable_admission_version: durable.version,
            journal_record_digest: durable.journal_record_digest,
        };
        binding.validate_structure()?;
        Ok(binding)
    }

    /// Return the global identity persisted inside the exact queue-plan journal record.
    #[must_use]
    pub fn global_admission_identity(&self) -> crate::queue::QueuePlanGlobalAdmissionIdentityV2 {
        crate::queue::QueuePlanGlobalAdmissionIdentityV2 {
            version: crate::queue::QUEUE_PLAN_GLOBAL_ADMISSION_IDENTITY_VERSION_V2,
            chain_id_digest: self.chain_id_digest,
            request_id: self.request_id,
        }
    }

    /// Return the canonical routing plan carried redundantly by the context.
    ///
    /// # Errors
    /// Returns an error when the context cannot encode a canonical routing plan or its advertised
    /// digest differs.
    pub fn routing_plan(&self) -> Result<crate::queue::RoutingPlan, String> {
        let routing_plan = self.admission_context.routing_plan()?;
        self.admission_context
            .validate_for_routing_plan(&routing_plan)?;
        if routing_plan.digest() != self.routing_plan_digest {
            return Err("QueuePlan binding routing digest differs from its context".to_owned());
        }
        Ok(routing_plan)
    }

    /// Validate all fields that do not require the exact transaction wire.
    ///
    /// # Errors
    /// Returns the first unsupported version, zero identity, context, routing, or journal failure.
    pub fn validate_structure(&self) -> Result<(), String> {
        if self.version != QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2 {
            return Err("QueuePlan admission-binding version is unsupported".to_owned());
        }
        if self.chain_id_digest == Hash::prehashed([0; Hash::LENGTH])
            || self.request_id == Hash::prehashed([0; Hash::LENGTH])
            || self.journal_record_digest == Hash::prehashed([0; Hash::LENGTH])
        {
            return Err("QueuePlan admission binding contains a zero identity hash".to_owned());
        }
        if self.request_id
            != queue_plan_synced_request_id_from_chain_digest(
                self.chain_id_digest,
                self.entrypoint_hash.clone(),
            )
        {
            return Err(
                "QueuePlan admission binding has a noncanonical semantic request identity"
                    .to_owned(),
            );
        }
        if self.admission_context.version != crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2 {
            return Err("QueuePlan admission-context version is unsupported".to_owned());
        }
        if self.queue_plan_journal_version != crate::queue::QUEUE_PLAN_JOURNAL_VERSION {
            return Err("QueuePlan journal version is unsupported".to_owned());
        }
        if self.durable_admission_version != crate::queue::QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V2 {
            return Err("QueuePlan durable-admission version is unsupported".to_owned());
        }
        self.routing_plan().map(|_| ())
    }

    /// Validate this binding against the exact request transaction and routing plan.
    ///
    /// # Errors
    /// Returns an error when any typed transaction identity, plan/context field, chain identity,
    /// or canonical version-4 journal-record digest differs.
    pub fn validate_for_request(
        &self,
        chain_id: &ChainId,
        transaction: &TransactionEntrypoint,
        routing_plan: &crate::queue::RoutingPlan,
    ) -> Result<(), String> {
        if self.chain_id_digest != queue_plan_admission_chain_id_digest(chain_id) {
            return Err("QueuePlan admission binding belongs to another chain".to_owned());
        }
        if self.request_id != queue_plan_synced_request_id(chain_id, transaction.hash()) {
            return Err(
                "QueuePlan admission binding has a noncanonical semantic request identity"
                    .to_owned(),
            );
        }
        self.validate_for_transaction_and_plan(transaction, routing_plan)
    }

    /// Validate the complete durable identity that authorizes a lane reservation Commit.
    ///
    /// The compatibility queue hash is derived from the canonical entrypoint rather than trusted
    /// from the reservation key. The exact coordinator and its admitting incarnation are then
    /// recovered from this binding's immutable routing context. The binding hash authenticates the
    /// original admission height, while the reservation key's distinct proposal height identifies
    /// the later lane slot that actually took queue ownership.
    ///
    /// # Errors
    /// Returns an error for a malformed reservation key or any entrypoint, queue hash, plan,
    /// backdated reservation height, coordinator, incarnation, or canonical binding-hash
    /// mismatch.
    pub(crate) fn validate_for_lane_reservation_commit(
        &self,
        key: &crate::queue::LaneQueueReservationKeyV2,
    ) -> Result<(), String> {
        key.validate().map_err(str::to_owned)?;
        self.validate_structure()?;
        let compatibility_queue_hash =
            HashOf::from_untyped_unchecked(Hash::from(self.entrypoint_hash.clone()));
        if key.proposal_height < self.admission_context.proposal_height {
            return Err(
                "lane reservation proposal height precedes its durable QueuePlan admission"
                    .to_owned(),
            );
        }
        if self.entrypoint_hash != key.entrypoint_hash
            || compatibility_queue_hash != key.signed_transaction_hash
            || self.routing_plan_digest != key.routing_plan_digest
            || self.canonical_hash() != key.queue_plan_admission_binding_hash
        {
            return Err(
                "QueuePlan binding does not match the lane reservation transaction, plan, or binding identity"
                    .to_owned(),
            );
        }
        let routing_plan = self.routing_plan()?;
        let coordinator = self
            .admission_context
            .route_incarnations
            .first()
            .ok_or_else(|| "QueuePlan admission context has no coordinator".to_owned())?;
        if routing_plan.coordinator_leg() != key.coordinator_leg
            || coordinator.leg != key.coordinator_leg
            || coordinator.lane_incarnation != key.lane_incarnation
        {
            return Err(
                "QueuePlan binding does not match the lane reservation coordinator generation"
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Validate the exact transaction, routing plan, and journal record when the trusted caller
    /// has already established the chain identity.
    ///
    /// # Errors
    /// Returns an error for any typed transaction identity, plan/context, or journal mismatch.
    pub fn validate_for_transaction_and_plan(
        &self,
        transaction: &TransactionEntrypoint,
        routing_plan: &crate::queue::RoutingPlan,
    ) -> Result<(), String> {
        self.validate_structure()?;
        if self.entrypoint_hash != transaction.hash()
            || self.signed_transaction_hash != signed_transaction_hash(transaction)
        {
            return Err(
                "QueuePlan admission binding has a different transaction identity".to_owned(),
            );
        }
        if self.routing_plan_digest != routing_plan.digest() {
            return Err("QueuePlan admission binding has a different routing plan".to_owned());
        }
        self.admission_context
            .validate_for_routing_plan(routing_plan)?;
        let exact_digest = crate::queue::queue_plan_journal_record_claim_digest(
            transaction.clone(),
            routing_plan.clone(),
            self.admission_context.clone(),
            self.enqueue_timestamp_ms,
            Some(self.global_admission_identity()),
        )
        .map_err(|error| format!("QueuePlan journal claim cannot be encoded: {error}"))?;
        if exact_digest != self.journal_record_digest {
            return Err(
                "QueuePlan admission binding does not cover the exact journal record".to_owned(),
            );
        }
        Ok(())
    }

    /// Return the domain-separated hash attested by coordinator authorities.
    #[must_use]
    pub fn canonical_hash(&self) -> Hash {
        let bytes = norito::encode_canonical(self)
            .expect("QueuePlan admission binding must have a canonical Norito encoding");
        Hash::new_from_chunks(&[QUEUE_PLAN_ADMISSION_BINDING_DOMAIN_V2, bytes.as_slice()])
    }

    /// Return the immutable WSV registry key for this transaction entrypoint.
    #[must_use]
    pub fn registry_key(&self) -> QueuePlanAdmissionRegistryKeyV2 {
        QueuePlanAdmissionRegistryKeyV2 {
            version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            chain_id_digest: self.chain_id_digest,
            entrypoint_hash: self.entrypoint_hash.clone(),
        }
    }

    /// Return the immutable WSV registry value for this exact binding.
    #[must_use]
    pub fn registry_value(&self) -> QueuePlanAdmissionRegistryValueV2 {
        QueuePlanAdmissionRegistryValueV2 {
            version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            binding_hash: self.canonical_hash(),
        }
    }
}

fn signed_transaction_hash(
    entrypoint: &TransactionEntrypoint,
) -> Option<HashOf<SignedTransaction>> {
    match entrypoint {
        TransactionEntrypoint::External(signed) => Some(signed.hash()),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction().hash()),
        TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
    }
}

/// One compact signature over a shared QueuePlan admission binding.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanAdmissionAttestationV2 {
    /// Attestation layout version.
    pub version: u16,
    /// Signer's index in the exact ordered coordinator validator set.
    pub validator_index: u16,
    /// Signature over the binding hash and validator index.
    pub signature: Signature,
}

/// Coordinator-authority evidence that one exact QueuePlan journal claim is durably replicated.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanAdmissionCertificateV2 {
    /// Certificate layout version.
    pub version: u16,
    /// One canonical binding shared by every attestation.
    pub binding: QueuePlanAdmissionBindingV2,
    /// Strictly increasing validator-index attestations.
    pub attestations: Vec<QueuePlanAdmissionAttestationV2>,
}

/// Canonical QueuePlan certificate disseminated from an ingress aggregator to validators.
///
/// The certificate stays as exact Norito bytes so every receiving validator can enforce the
/// same bounded canonical-decoding boundary before publishing those bytes durably in Kura.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct QueuePlanAdmissionPublicationV1 {
    /// Publication envelope schema version.
    pub schema_version: u16,
    /// Exact canonical [`QueuePlanAdmissionCertificateV2`] bytes.
    pub certificate: Vec<u8>,
}

/// Strength required while validating a QueuePlan admission certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuePlanAdmissionCertificateStrengthV2 {
    /// A bounded nonempty subset, used for authenticated authority responses.
    Partial,
    /// Exactly the context's durability threshold, used by global admission controls.
    Quorum,
}

/// Fully authenticated QueuePlan admission certificate and its registry projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedQueuePlanAdmissionCertificateV2 {
    /// Exact decoded certificate.
    pub certificate: QueuePlanAdmissionCertificateV2,
    /// Domain-separated binding identity.
    pub binding_hash: Hash,
    /// Immutable WSV registry key.
    pub registry_key: QueuePlanAdmissionRegistryKeyV2,
    /// Immutable WSV registry value.
    pub registry_value: QueuePlanAdmissionRegistryValueV2,
    /// Exact coordinator route authorized by the binding.
    pub coordinator_route: crate::queue::RoutingDecision,
    /// Number of distinct attestations required for durable availability.
    pub durability_threshold: usize,
}

#[derive(Encode)]
struct QueuePlanAdmissionAttestationPayloadV2 {
    version: u16,
    binding_hash: Hash,
    validator_index: u16,
}

/// Return canonical domain-separated signing bytes for one authority index.
///
/// # Errors
/// Returns a Norito encoding error if the fixed attestation payload cannot be encoded.
pub fn queue_plan_admission_attestation_signing_bytes_v2(
    binding_hash: Hash,
    validator_index: u16,
) -> Result<Vec<u8>, norito::Error> {
    let payload = QueuePlanAdmissionAttestationPayloadV2 {
        version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
        binding_hash,
        validator_index,
    };
    let encoded = norito::encode_canonical(&payload)?;
    let mut bytes =
        Vec::with_capacity(QUEUE_PLAN_ADMISSION_ATTESTATION_DOMAIN_V2.len() + encoded.len());
    bytes.extend_from_slice(QUEUE_PLAN_ADMISSION_ATTESTATION_DOMAIN_V2);
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

/// Validate one already-decoded QueuePlan admission certificate.
///
/// # Errors
/// Returns the first structural, chain, roster, threshold, ordering, or signature failure.
pub fn validate_queue_plan_admission_certificate_v2(
    chain_id: &ChainId,
    certificate: QueuePlanAdmissionCertificateV2,
    strength: QueuePlanAdmissionCertificateStrengthV2,
) -> Result<ValidatedQueuePlanAdmissionCertificateV2, String> {
    validate_queue_plan_admission_certificate_for_chain_digest_v2(
        queue_plan_admission_chain_id_digest(chain_id),
        certificate,
        strength,
    )
}

/// Validate a QueuePlan certificate against a caller-authenticated chain digest.
///
/// Torii uses this after validating the request binding against its local chain and exact
/// transaction. Consensus-facing callers should prefer
/// [`validate_queue_plan_admission_certificate_v2`] with a trusted [`ChainId`].
///
/// # Errors
/// Returns the first structural, chain, roster, threshold, ordering, or signature failure.
pub fn validate_queue_plan_admission_certificate_for_chain_digest_v2(
    expected_chain_id_digest: Hash,
    certificate: QueuePlanAdmissionCertificateV2,
    strength: QueuePlanAdmissionCertificateStrengthV2,
) -> Result<ValidatedQueuePlanAdmissionCertificateV2, String> {
    if certificate.version != QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2 {
        return Err("QueuePlan admission-certificate version is unsupported".to_owned());
    }
    certificate.binding.validate_structure()?;
    if certificate.binding.chain_id_digest != expected_chain_id_digest {
        return Err("QueuePlan admission certificate belongs to another chain".to_owned());
    }
    let routing_plan = certificate.binding.routing_plan()?;
    let coordinator = certificate
        .binding
        .admission_context
        .route_incarnations
        .first()
        .ok_or_else(|| "QueuePlan admission context has no coordinator".to_owned())?;
    let durability_threshold = usize::from(coordinator.durability_threshold);
    let attestation_count = certificate.attestations.len();
    if attestation_count == 0 || attestation_count > durability_threshold {
        return Err(
            "QueuePlan admission certificate has an empty or oversized attestation set".to_owned(),
        );
    }
    if strength == QueuePlanAdmissionCertificateStrengthV2::Quorum
        && attestation_count != durability_threshold
    {
        return Err(
            "QueuePlan admission certificate does not contain the exact durability quorum"
                .to_owned(),
        );
    }
    let binding_hash = certificate.binding.canonical_hash();
    let mut previous_index = None;
    for attestation in &certificate.attestations {
        if attestation.version != QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2 {
            return Err("QueuePlan admission-attestation version is unsupported".to_owned());
        }
        if previous_index.is_some_and(|previous| previous >= attestation.validator_index) {
            return Err(
                "QueuePlan admission attestations are duplicated or not canonically ordered"
                    .to_owned(),
            );
        }
        previous_index = Some(attestation.validator_index);
        let validator = coordinator
            .validator_set
            .get(usize::from(attestation.validator_index))
            .ok_or_else(|| "QueuePlan admission attestation index is out of bounds".to_owned())?;
        let signing_bytes = queue_plan_admission_attestation_signing_bytes_v2(
            binding_hash,
            attestation.validator_index,
        )
        .map_err(|error| format!("QueuePlan attestation payload cannot be encoded: {error}"))?;
        attestation
            .signature
            .verify(validator.public_key(), &signing_bytes)
            .map_err(|error| format!("QueuePlan admission attestation is invalid: {error}"))?;
    }
    let registry_key = certificate.binding.registry_key();
    let registry_value = certificate.binding.registry_value();
    Ok(ValidatedQueuePlanAdmissionCertificateV2 {
        coordinator_route: routing_plan.coordinator_route(),
        certificate,
        binding_hash,
        registry_key,
        registry_value,
        durability_threshold,
    })
}

/// Decode canonical bounded certificate bytes and require an exact durability quorum.
///
/// This is the validation boundary used by merge-sidecar admission and WSV staging. Partial
/// authority responses must use [`validate_queue_plan_admission_certificate_v2`] directly.
///
/// # Errors
/// Returns an error for an empty/oversized body, bounded-decode failure, noncanonical Norito
/// bytes, or any structural, chain, quorum, roster, ordering, or signature mismatch.
pub fn decode_and_validate_queue_plan_admission_certificate_v2(
    chain_id: &ChainId,
    bytes: &[u8],
) -> Result<ValidatedQueuePlanAdmissionCertificateV2, String> {
    let max_bytes = iroha_data_model::merge::MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err("QueuePlan admission certificate is empty or oversized".to_owned());
    }
    let decode_limits = norito::DecodeLimits::new(
        iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES + 1,
        max_bytes,
        max_bytes,
        max_bytes.saturating_mul(4),
        64,
    );
    let certificate = norito::decode_canonical_with_limits::<QueuePlanAdmissionCertificateV2>(
        bytes,
        decode_limits,
    )
    .map_err(|error| format!("QueuePlan admission certificate cannot be decoded: {error}"))?;
    validate_queue_plan_admission_certificate_v2(
        chain_id,
        certificate,
        QueuePlanAdmissionCertificateStrengthV2::Quorum,
    )
}

/// Stable lane/dataspace assignment determined at ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiRouteHintV1 {
    /// Nexus lane selected for the request.
    pub lane_id: LaneId,
    /// Dataspace selected for the request.
    pub dataspace_id: DataSpaceId,
}

impl From<crate::queue::RoutingDecision> for ToriiRouteHintV1 {
    fn from(value: crate::queue::RoutingDecision) -> Self {
        Self {
            lane_id: value.lane_id,
            dataspace_id: value.dataspace_id,
        }
    }
}

impl From<ToriiRouteHintV1> for crate::queue::RoutingDecision {
    fn from(value: ToriiRouteHintV1) -> Self {
        Self::new(value.lane_id, value.dataspace_id)
    }
}

/// Role of one route in a Torii transaction routing plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiRouteLegRoleV1 {
    /// Coordinator route for final admission and commit ordering.
    Coordinator,
    /// Dataspace-local participant route.
    Participant,
}

impl From<crate::queue::RouteLegRole> for ToriiRouteLegRoleV1 {
    fn from(value: crate::queue::RouteLegRole) -> Self {
        match value {
            crate::queue::RouteLegRole::Coordinator => Self::Coordinator,
            crate::queue::RouteLegRole::Participant => Self::Participant,
        }
    }
}

impl From<ToriiRouteLegRoleV1> for crate::queue::RouteLegRole {
    fn from(value: ToriiRouteLegRoleV1) -> Self {
        match value {
            ToriiRouteLegRoleV1::Coordinator => Self::Coordinator,
            ToriiRouteLegRoleV1::Participant => Self::Participant,
        }
    }
}

/// One lane/dataspace leg in a Torii transaction routing plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiRouteLegHintV1 {
    /// Lane/dataspace route selected for this leg.
    pub route: ToriiRouteHintV1,
    /// Role assigned to this leg.
    pub role: ToriiRouteLegRoleV1,
}

impl From<crate::queue::RouteLeg> for ToriiRouteLegHintV1 {
    fn from(value: crate::queue::RouteLeg) -> Self {
        Self {
            route: value.route.into(),
            role: value.role.into(),
        }
    }
}

impl From<ToriiRouteLegHintV1> for crate::queue::RouteLeg {
    fn from(value: ToriiRouteLegHintV1) -> Self {
        Self::new(value.route.into(), value.role.into())
    }
}

/// Kind of validation failure in a Torii routing-plan hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToriiRoutingPlanHintErrorKind {
    /// A coordinator leg was encoded with a non-coordinator role.
    UnexpectedCoordinatorRole,
    /// A participant leg was encoded with a non-participant role.
    UnexpectedParticipantRole,
    /// A Native AMX hint contains more participant routes than the protocol permits.
    NativeAmxParticipantLimitExceeded,
    /// A Native AMX hint repeats one participant route.
    NativeAmxDuplicateParticipantRoute,
    /// Native AMX participant routes are not in canonical dataspace/lane order.
    NativeAmxParticipantsOutOfOrder,
    /// A Native AMX hint advertised a digest that does not match its route legs.
    NativeAmxPlanDigestMismatch,
}

/// Error returned when a Torii routing-plan hint is not internally canonical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToriiRoutingPlanHintError {
    kind: ToriiRoutingPlanHintErrorKind,
    leg_index: Option<usize>,
    actual_role: Option<ToriiRouteLegRoleV1>,
    participant_count: Option<usize>,
    participant_limit: Option<usize>,
    previous_route: Option<ToriiRouteHintV1>,
    actual_route: Option<ToriiRouteHintV1>,
    advertised_digest: Option<Hash>,
    computed_digest: Option<Hash>,
}

impl ToriiRoutingPlanHintError {
    /// Construct an error for a malformed coordinator leg role.
    #[must_use]
    pub const fn unexpected_coordinator_role(actual: ToriiRouteLegRoleV1) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::UnexpectedCoordinatorRole,
            leg_index: None,
            actual_role: Some(actual),
            participant_count: None,
            participant_limit: None,
            previous_route: None,
            actual_route: None,
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a malformed participant leg role.
    #[must_use]
    pub const fn unexpected_participant_role(index: usize, actual: ToriiRouteLegRoleV1) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::UnexpectedParticipantRole,
            leg_index: Some(index),
            actual_role: Some(actual),
            participant_count: None,
            participant_limit: None,
            previous_route: None,
            actual_route: None,
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a Native AMX participant vector above the protocol limit.
    #[must_use]
    pub const fn native_amx_participant_limit_exceeded(count: usize, limit: usize) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::NativeAmxParticipantLimitExceeded,
            leg_index: None,
            actual_role: None,
            participant_count: Some(count),
            participant_limit: Some(limit),
            previous_route: None,
            actual_route: None,
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a repeated Native AMX participant route.
    #[must_use]
    pub const fn native_amx_duplicate_participant_route(
        index: usize,
        route: ToriiRouteHintV1,
    ) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::NativeAmxDuplicateParticipantRoute,
            leg_index: Some(index),
            actual_role: None,
            participant_count: None,
            participant_limit: None,
            previous_route: Some(route),
            actual_route: Some(route),
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for noncanonical Native AMX participant ordering.
    #[must_use]
    pub const fn native_amx_participants_out_of_order(
        index: usize,
        previous: ToriiRouteHintV1,
        actual: ToriiRouteHintV1,
    ) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::NativeAmxParticipantsOutOfOrder,
            leg_index: Some(index),
            actual_role: None,
            participant_count: None,
            participant_limit: None,
            previous_route: Some(previous),
            actual_route: Some(actual),
            advertised_digest: None,
            computed_digest: None,
        }
    }

    /// Construct an error for a Native AMX digest that does not match the route legs.
    #[must_use]
    pub const fn native_amx_plan_digest_mismatch(advertised: Hash, computed: Hash) -> Self {
        Self {
            kind: ToriiRoutingPlanHintErrorKind::NativeAmxPlanDigestMismatch,
            leg_index: None,
            actual_role: None,
            participant_count: None,
            participant_limit: None,
            previous_route: None,
            actual_route: None,
            advertised_digest: Some(advertised),
            computed_digest: Some(computed),
        }
    }

    /// Return the failure kind.
    #[must_use]
    pub const fn kind(&self) -> ToriiRoutingPlanHintErrorKind {
        self.kind
    }

    /// Return the malformed leg role, when this error is role-related.
    #[must_use]
    pub const fn actual_role(&self) -> Option<ToriiRouteLegRoleV1> {
        self.actual_role
    }

    /// Return the malformed participant index, when this error identifies one route leg.
    #[must_use]
    pub const fn leg_index(&self) -> Option<usize> {
        self.leg_index
    }

    /// Return the advertised participant count, when this error is count-related.
    #[must_use]
    pub const fn participant_count(&self) -> Option<usize> {
        self.participant_count
    }

    /// Return the maximum participant count, when this error is count-related.
    #[must_use]
    pub const fn participant_limit(&self) -> Option<usize> {
        self.participant_limit
    }

    /// Return the preceding participant route, when this error is order-related.
    #[must_use]
    pub const fn previous_route(&self) -> Option<ToriiRouteHintV1> {
        self.previous_route
    }

    /// Return the malformed participant route, when this error is route-related.
    #[must_use]
    pub const fn actual_route(&self) -> Option<ToriiRouteHintV1> {
        self.actual_route
    }

    /// Return the advertised Native AMX plan digest, when this error is digest-related.
    #[must_use]
    pub const fn advertised_digest(&self) -> Option<Hash> {
        self.advertised_digest
    }

    /// Return the recomputed Native AMX plan digest, when this error is digest-related.
    #[must_use]
    pub const fn computed_digest(&self) -> Option<Hash> {
        self.computed_digest
    }
}

impl fmt::Display for ToriiRoutingPlanHintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ToriiRoutingPlanHintErrorKind::UnexpectedCoordinatorRole => match self.actual_role {
                Some(actual) => write!(f, "unexpected coordinator role {actual:?}"),
                None => f.write_str("unexpected coordinator role"),
            },
            ToriiRoutingPlanHintErrorKind::UnexpectedParticipantRole => {
                match (self.leg_index, self.actual_role) {
                    (Some(index), Some(actual)) => {
                        write!(f, "unexpected participant role {actual:?} at index {index}")
                    }
                    _ => f.write_str("unexpected participant role"),
                }
            }
            ToriiRoutingPlanHintErrorKind::NativeAmxParticipantLimitExceeded => {
                match (self.participant_count, self.participant_limit) {
                    (Some(count), Some(limit)) => write!(
                        f,
                        "native AMX participant count {count} exceeds protocol limit {limit}"
                    ),
                    _ => f.write_str("native AMX participant count exceeds protocol limit"),
                }
            }
            ToriiRoutingPlanHintErrorKind::NativeAmxDuplicateParticipantRoute => {
                match (self.leg_index, self.actual_route) {
                    (Some(index), Some(route)) => write!(
                        f,
                        "duplicate native AMX participant route at index {index}: dataspace {}, lane {}",
                        route.dataspace_id.as_u64(),
                        route.lane_id.as_u32()
                    ),
                    _ => f.write_str("duplicate native AMX participant route"),
                }
            }
            ToriiRoutingPlanHintErrorKind::NativeAmxParticipantsOutOfOrder => {
                match (self.leg_index, self.previous_route, self.actual_route) {
                    (Some(index), Some(previous), Some(actual)) => write!(
                        f,
                        "native AMX participant routes are out of canonical (dataspace, lane) order \
                         at index {index}: previous ({}, {}), actual ({}, {})",
                        previous.dataspace_id.as_u64(),
                        previous.lane_id.as_u32(),
                        actual.dataspace_id.as_u64(),
                        actual.lane_id.as_u32()
                    ),
                    _ => f.write_str(
                        "native AMX participant routes are out of canonical (dataspace, lane) order",
                    ),
                }
            }
            ToriiRoutingPlanHintErrorKind::NativeAmxPlanDigestMismatch => {
                match (self.advertised_digest, self.computed_digest) {
                    (Some(advertised), Some(computed)) => write!(
                        f,
                        "native AMX plan digest mismatch: advertised {advertised}, computed {computed}"
                    ),
                    _ => f.write_str("native AMX plan digest mismatch"),
                }
            }
        }
    }
}

impl std::error::Error for ToriiRoutingPlanHintError {}

/// Stable full routing plan determined at ingress.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiRoutingPlanHintV1 {
    /// Single coordinator route.
    Single(ToriiRouteLegHintV1),
    /// Native AMX coordinator and participant route set.
    NativeAmx {
        /// Stable digest of the native AMX plan.
        plan_digest: Hash,
        /// Coordinator route for final ordering.
        coordinator: ToriiRouteLegHintV1,
        /// Dataspace-local participant routes.
        participants: Vec<ToriiRouteLegHintV1>,
    },
}

impl ToriiRoutingPlanHintV1 {
    /// Return the coordinator route for peer selection and diagnostics.
    #[must_use]
    pub fn coordinator_route(&self) -> ToriiRouteHintV1 {
        match self {
            Self::Single(leg) => leg.route,
            Self::NativeAmx { coordinator, .. } => coordinator.route,
        }
    }

    /// Convert this hint to a full routing plan after validating redundant wire fields.
    ///
    /// # Errors
    /// Returns an error when leg roles are not canonical, the participant vector exceeds the
    /// protocol limit or is not in strict canonical route order, or a Native AMX hint's advertised
    /// digest does not match the digest recomputed from its route legs.
    pub fn try_into_routing_plan(
        self,
    ) -> Result<crate::queue::RoutingPlan, ToriiRoutingPlanHintError> {
        match self {
            Self::Single(leg) => {
                if leg.role != ToriiRouteLegRoleV1::Coordinator {
                    return Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                        leg.role,
                    ));
                }
                Ok(crate::queue::RoutingPlan::single(
                    crate::queue::RouteLeg::from(leg).route,
                ))
            }
            Self::NativeAmx {
                plan_digest,
                coordinator,
                participants,
            } => {
                if coordinator.role != ToriiRouteLegRoleV1::Coordinator {
                    return Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                        coordinator.role,
                    ));
                }
                if participants.len() > TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1 {
                    return Err(
                        ToriiRoutingPlanHintError::native_amx_participant_limit_exceeded(
                            participants.len(),
                            TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1,
                        ),
                    );
                }

                let mut participant_legs = Vec::with_capacity(participants.len());
                let mut previous_route: Option<ToriiRouteHintV1> = None;
                for (index, leg) in participants.into_iter().enumerate() {
                    if leg.role != ToriiRouteLegRoleV1::Participant {
                        return Err(ToriiRoutingPlanHintError::unexpected_participant_role(
                            index, leg.role,
                        ));
                    }
                    if let Some(previous) = previous_route {
                        let previous_key = (previous.dataspace_id, previous.lane_id);
                        let actual_key = (leg.route.dataspace_id, leg.route.lane_id);
                        if actual_key == previous_key {
                            return Err(
                                ToriiRoutingPlanHintError::native_amx_duplicate_participant_route(
                                    index, leg.route,
                                ),
                            );
                        }
                        if actual_key < previous_key {
                            return Err(
                                ToriiRoutingPlanHintError::native_amx_participants_out_of_order(
                                    index, previous, leg.route,
                                ),
                            );
                        }
                    }
                    previous_route = Some(leg.route);
                    participant_legs.push(crate::queue::RouteLeg::from(leg));
                }

                let plan = crate::queue::RoutingPlan::native_amx(
                    crate::queue::RouteLeg::from(coordinator).route,
                    participant_legs,
                );
                let computed = plan.digest();
                if computed != plan_digest {
                    return Err(ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
                        plan_digest,
                        computed,
                    ));
                }
                Ok(plan)
            }
        }
    }
}

impl From<crate::queue::RoutingPlan> for ToriiRoutingPlanHintV1 {
    fn from(value: crate::queue::RoutingPlan) -> Self {
        match value {
            crate::queue::RoutingPlan::Single(leg) => Self::Single(leg.into()),
            crate::queue::RoutingPlan::NativeAmx(plan) => Self::NativeAmx {
                plan_digest: plan.plan_digest,
                coordinator: plan.coordinator.into(),
                participants: plan.participants.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl TryFrom<ToriiRoutingPlanHintV1> for crate::queue::RoutingPlan {
    type Error = ToriiRoutingPlanHintError;

    fn try_from(value: ToriiRoutingPlanHintV1) -> Result<Self, Self::Error> {
        value.try_into_routing_plan()
    }
}

/// Encoded response format requested by the ingress node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyResponseFormatV1 {
    /// Serialize the response body as Norito.
    Norito,
    /// Serialize the response body as JSON.
    Json,
}

/// Supported read endpoints forwarded over the Torii control plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadEndpointV1 {
    /// `GET /v1/accounts/{account_id}`
    AccountGet,
    /// `GET /v1/explorer/accounts/{account_id}`
    ExplorerAccountDetail,
    /// `GET /v1/accounts/{account_id}/assets`
    AccountAssetsGet,
    /// `POST /v1/accounts/{account_id}/assets/query`
    AccountAssetsQuery,
    /// `GET /v1/accounts/{account_id}/permissions`
    AccountPermissionsGet,
    /// `GET /v1/accounts/{account_id}/transactions`
    AccountTransactionsGet,
    /// `POST /v1/accounts/{account_id}/transactions/query`
    AccountTransactionsQuery,
    /// `POST /v1/transactions/query`
    TransactionsQuery,
    /// `GET /v1/pipeline/transactions/status`
    PipelineTransactionStatusGet,
    /// `GET /v1/proofs/{id}`
    ProofRecordGet,
    /// `GET /v1/accounts`
    AccountsList,
    /// `POST /v1/accounts/query`
    AccountsQuery,
    /// `GET /v1/accounts/{uaid}/portfolio`
    AccountsPortfolio,
    /// `GET /v1/assets/definitions`
    AssetDefinitionsList,
    /// `GET /v1/assets/definitions/{asset}`
    AssetDefinitionGet,
    /// `POST /v1/assets/definitions/query`
    AssetDefinitionsQuery,
    /// `GET /v1/assets/definitions/{asset}/holders`
    AssetHoldersGet,
    /// `POST /v1/assets/definitions/{asset}/holders/query`
    AssetHoldersQuery,
    /// `GET /v1/domains`
    DomainsList,
    /// `POST /v1/domains/query`
    DomainsQuery,
    /// `GET /v1/nfts`
    NftsList,
    /// `POST /v1/nfts/query`
    NftsQuery,
    /// `GET /v1/nexus/public-lanes/{lane_id}/validators`
    NexusPublicLaneValidators,
    /// `GET /v1/nexus/public-lanes/{lane_id}/stake`
    NexusPublicLaneStake,
    /// `GET /v1/nexus/public-lanes/{lane_id}/rewards/pending`
    NexusPublicLaneRewards,
    /// `GET /v1/nexus/dataspaces/accounts/{literal}/summary`
    NexusDataspacesAccountSummary,
    /// `GET /v1/space-directory/uaids/{uaid}`
    SpaceDirectoryBindingsGet,
    /// `GET /v1/space-directory/uaids/{uaid}/manifests`
    SpaceDirectoryManifestsGet,
    /// `GET /v1/rwas`
    RwasList,
    /// `POST /v1/rwas/query`
    RwasQuery,
    /// `POST /v1/aliases/resolve`
    AliasResolve,
    /// `POST /v1/aliases/resolve-index`
    AliasResolveIndex,
    /// `POST /v1/aliases/by-account`
    AliasLookupByAccount,
    /// `GET /v1/explorer/asset-definitions/{id}`
    ExplorerAssetDefinitionDetail,
    /// `GET /v1/explorer/asset-definitions/{id}/econometrics`
    ExplorerAssetDefinitionEconometrics,
    /// `GET /v1/explorer/asset-definitions/{id}/snapshot`
    ExplorerAssetDefinitionSnapshot,
    /// `POST /v1/contracts/aliases/resolve`
    ContractAliasResolve,
    /// `GET /v1/contracts/state`
    ContractStateGet,
    /// `POST /v1/contracts/view`
    ContractViewPost,
    /// `POST /v1/contracts/view/batch`
    ContractViewBatchPost,
    /// `GET /v1/accounts/{account_id}/history`
    AccountHistoryGet,
    /// `GET /v1/internal/accounts/{account_id}`
    InternalAccountGet,
    /// `GET /v1/internal/accounts/{account_id}/transactions/{entrypoint_hash}`
    InternalAccountTransactionGet,
    /// `GET /v1/internal/accounts/{account_id}/assets/{asset_definition_id}?scope=...`
    InternalAccountAssetGet,
    /// `POST /v1/contracts/deployment-state`
    ContractDeploymentState,
}

/// Canonical routed read executed on an authoritative Torii peer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiReadProxyRequestV1 {
    /// Supported read endpoint identifier.
    pub endpoint: ToriiReadEndpointV1,
    /// Stable route resolved by the ingress node.
    pub expected_route: ToriiRouteHintV1,
    /// String path arguments in endpoint-specific order.
    pub path_args: Vec<String>,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// Raw JSON body for POST-style read endpoints.
    pub body: Vec<u8>,
    /// Response encoding negotiated by the ingress node.
    pub response_format: ToriiProxyResponseFormatV1,
}

/// Route set Nexus should recompute for a coordinated fanout request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiFanoutRouteScopeV1 {
    /// Fan out across all configured dataspace routes.
    AllDataspaces,
    /// Fan out across the dataspaces that may own the target account.
    TargetAccount {
        /// Canonical target account id literal.
        account_id: String,
    },
    /// Fan out across public routes plus caller-visible private dataspaces.
    VisibleAccount {
        /// Optional canonical caller account id literal.
        caller_account_id: Option<String>,
    },
}

/// Merge behavior requested for an App API read fanout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiReadFanoutMergeV1 {
    /// Merge JSON list-style responses.
    List,
    /// Merge JSON singleton responses.
    Singleton,
    /// Merge account-detail responses while preserving the requested response format.
    Account,
    /// Merge account-history responses with global ordering and pagination.
    AccountHistory,
    /// Merge account portfolio responses.
    Portfolio,
    /// Merge dataspace account summary responses.
    DataspaceSummary,
    /// Merge space-directory bindings responses.
    SpaceDirectoryBindings,
    /// Merge space-directory manifest responses.
    SpaceDirectoryManifests {
        /// Client pagination offset to apply after merged deduplication.
        page_offset: u64,
        /// Client pagination limit to apply after merged deduplication.
        page_limit: Option<u64>,
    },
}

/// App API read fanout coordinated by the Nexus/default route.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiReadFanoutProxyRequestV1 {
    /// Supported read endpoint identifier.
    pub endpoint: ToriiReadEndpointV1,
    /// Route scope that Nexus must recompute from its local catalog/world.
    pub route_scope: ToriiFanoutRouteScopeV1,
    /// Merge behavior for the endpoint response.
    pub merge: ToriiReadFanoutMergeV1,
    /// String path arguments in endpoint-specific order.
    pub path_args: Vec<String>,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// Raw JSON body for POST-style read endpoints.
    pub body: Vec<u8>,
    /// Response encoding negotiated by the ingress node.
    pub response_format: ToriiProxyResponseFormatV1,
}

/// Hosted HTTP request forwarded to a peer that may own a healthy Inrou target.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiHostedHttpProxyRequestV1 {
    /// Soracloud service name already resolved from the public route.
    pub service_name: String,
    /// Exact service revision selected by the ingress node.
    pub service_version: String,
    /// Exact authoritative replica slot selected by the ingress node.
    pub replica_slot: u16,
    /// Request path relative to the admitted public route prefix.
    pub request_path: String,
    /// Original client HTTP method.
    pub method: String,
    /// Raw query string without the leading `?`.
    pub query_string: Option<String>,
    /// Original request headers preserved by ingress.
    pub headers: Vec<ToriiProxyHeaderV1>,
    /// Raw request body bytes.
    pub body: Vec<u8>,
    /// Original client IP address when known, used for deterministic canary selection.
    pub remote_ip: Option<String>,
}

/// First-release queue admission contract for a proxied transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyTransactionAdmissionV2 {
    /// Acknowledge only after the exact QueuePlan certificate is globally committed.
    ///
    /// Index two deliberately leaves both retired V1 tags invalid, so neither
    /// ordinary deferred admission nor its pre-global "synced" sibling can be
    /// reinterpreted under the first-release contract.
    #[codec(index = 2)]
    QueuePlanSynced,
}

/// Canonical version-4 Torii request body forwarded over the P2P control plane.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ToriiProxyRequestKindV4 {
    /// Submit a signed transaction to the authoritative lane validator.
    #[codec(index = 0)]
    SubmitTransaction {
        /// Original transaction entrypoint from the client.
        transaction: TransactionEntrypoint,
        /// Full routing plan resolved by the ingress node.
        expected_plan: ToriiRoutingPlanHintV1,
        /// Durability boundary the route-owning peer must satisfy before acknowledging.
        admission: ToriiProxyTransactionAdmissionV2,
        /// Exact shared journal binding required for a durable admission.
        ///
        /// This must be present only for `QueuePlanSynced` and is revalidated by
        /// every forwarding and admitting authority.
        admission_binding: Option<QueuePlanAdmissionBindingV2>,
    },
    /// Execute a signed query on the authoritative lane validator.
    #[codec(index = 1)]
    SignedQuery {
        /// Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Exhaust a client-signed query on one exact authoritative route.
    #[codec(index = 2)]
    SignedQueryRouteScan {
        /// Original versioned Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Route resolved by the ingress node.
        expected_route: ToriiRouteHintV1,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a client-signed query fanout coordinated by the Nexus/default route.
    #[codec(index = 3)]
    SignedQueryFanout {
        /// Original versioned Norito-encoded signed query from the client.
        query_bytes: Vec<u8>,
        /// Response encoding negotiated by the ingress node.
        response_format: ToriiProxyResponseFormatV1,
    },
    /// Execute a routed Torii read endpoint on the authoritative peer.
    #[codec(index = 4)]
    Read(ToriiReadProxyRequestV1),
    /// Execute an App API read fanout coordinated by the Nexus/default route.
    #[codec(index = 5)]
    ReadFanout(ToriiReadFanoutProxyRequestV1),
    /// Proxy a Soracloud public hosted-HTTP request to a peer with a local healthy Inrou target.
    #[codec(index = 6)]
    HostedHttp(ToriiHostedHttpProxyRequestV1),
}

/// Version-5 P2P Torii proxy request sent from ingress to an authoritative peer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyRequestV5 {
    /// Version of the proxy request envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Current forwarding depth observed by this hop.
    pub hop_count: u8,
    /// Maximum number of hops allowed before the request is rejected.
    pub max_hops: u8,
    /// Peer ids already traversed by the request to prevent proxy loops.
    pub visited_peer_ids: Vec<PeerId>,
    /// Canonical request to execute on the authoritative peer.
    pub request: ToriiProxyRequestKindV4,
}

/// One HTTP header preserved across the Torii proxy response snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyHeaderV1 {
    /// Lower- or mixed-case header name as received from the responder.
    pub name: String,
    /// Raw header value bytes.
    pub value: Vec<u8>,
}

/// Serialized HTTP response sent back to the ingress node.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyHttpResponseV1 {
    /// HTTP status code returned by the authoritative responder.
    pub status_code: u16,
    /// HTTP headers returned by the authoritative responder.
    pub headers: Vec<ToriiProxyHeaderV1>,
    /// Raw response body bytes returned by the authoritative responder.
    pub body: Vec<u8>,
}

/// P2P Torii proxy response sent from the authoritative peer back to ingress.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ToriiProxyResponseV1 {
    /// Version of the proxy response envelope.
    pub schema_version: u16,
    /// Correlation id selected by the ingress node.
    pub request_id: Hash,
    /// Serialized HTTP response from the authoritative peer.
    pub response: ToriiProxyHttpResponseV1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};

    const LEGACY_TORII_PROXY_REQUEST_VERSION_V2: u16 = 2;

    /// Frozen test-only copy of the checked-in V2 Submit body.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    enum HistoricalToriiProxyRequestKindV1 {
        #[codec(index = 0)]
        SubmitTransaction {
            transaction: TransactionEntrypoint,
            expected_plan: ToriiRoutingPlanHintV1,
        },
    }

    /// Frozen test-only copy of the checked-in V2 request envelope.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    #[norito(schema_name = "iroha_core::torii_proxy::ToriiProxyRequestV2")]
    struct HistoricalToriiProxyRequestV2 {
        schema_version: u16,
        request_id: Hash,
        hop_count: u8,
        max_hops: u8,
        visited_peer_ids: Vec<PeerId>,
        request: HistoricalToriiProxyRequestKindV1,
    }

    /// Frozen test-only carrier for the checked-in V2 request at the original
    /// `NetworkMessage::ToriiProxyRequest` discriminant.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    #[norito(schema_name = "iroha_core::NetworkMessage")]
    enum HistoricalNetworkMessage {
        #[codec(index = 19)]
        ToriiProxyRequest(Box<HistoricalToriiProxyRequestV2>),
    }

    /// Frozen test-only copy of the transient V2-plus-boolean Submit body.
    ///
    /// This shape never was the checked-in V2 contract, but keeping it as an
    /// additional negative control proves that a one-byte boolean cannot be
    /// interpreted as the four-byte V3 admission discriminant.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    enum TransientBoolToriiProxyRequestKindV1 {
        #[codec(index = 0)]
        SubmitTransaction {
            transaction: TransactionEntrypoint,
            expected_plan: ToriiRoutingPlanHintV1,
            strict_durable: bool,
        },
    }

    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    #[norito(schema_name = "iroha_core::torii_proxy::ToriiProxyRequestV2")]
    struct TransientBoolToriiProxyRequestV2 {
        schema_version: u16,
        request_id: Hash,
        hop_count: u8,
        max_hops: u8,
        visited_peer_ids: Vec<PeerId>,
        request: TransientBoolToriiProxyRequestKindV1,
    }

    /// Frozen admission discriminants from the retired pre-global layout.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
    enum ObsoleteToriiProxyTransactionAdmissionV1 {
        #[codec(index = 0)]
        Deferred,
        #[codec(index = 1)]
        QueuePlanSynced,
    }

    fn torii_read_endpoint_wire_index(endpoint: ToriiReadEndpointV1) -> u32 {
        let encoded = norito::codec::Encode::encode(&endpoint);
        assert_eq!(
            encoded.len(),
            4,
            "ToriiReadEndpointV1 should encode as a u32 variant index"
        );
        u32::from_le_bytes(encoded.try_into().expect("four-byte variant index"))
    }

    fn torii_transaction_admission_wire_index(admission: ToriiProxyTransactionAdmissionV2) -> u32 {
        let encoded = norito::codec::Encode::encode(&admission);
        assert_eq!(
            encoded.len(),
            4,
            "ToriiProxyTransactionAdmissionV2 should encode as a u32 variant index"
        );
        u32::from_le_bytes(encoded.try_into().expect("four-byte variant index"))
    }

    #[test]
    fn queue_plan_synced_request_identity_is_semantic_and_source_bound() {
        let chain_a: ChainId = "queue-plan-request-chain-a".parse().expect("parse chain A");
        let chain_b: ChainId = "queue-plan-request-chain-b".parse().expect("parse chain B");
        let entrypoint_a = HashOf::from_untyped_unchecked(Hash::new(b"queue-plan-entrypoint-a"));
        let entrypoint_b = HashOf::from_untyped_unchecked(Hash::new(b"queue-plan-entrypoint-b"));
        let chain_a_digest = queue_plan_admission_chain_id_digest(&chain_a);

        let request = queue_plan_synced_request_id(&chain_a, entrypoint_a.clone());
        assert_eq!(
            request,
            Hash::new(
                norito::encode_canonical(&(
                    "torii:proxy:queue-plan-synced:v5",
                    chain_a_digest,
                    entrypoint_a.clone(),
                ))
                .expect("encode frozen request projection")
            ),
            "the shared kernel must retain the exact production domain and field projection"
        );
        assert_eq!(
            request,
            queue_plan_synced_request_id_from_chain_digest(chain_a_digest, entrypoint_a.clone(),),
            "the raw-chain wrapper and durable replay kernel must be identical"
        );
        assert_eq!(
            request,
            queue_plan_synced_request_id(&chain_a, entrypoint_a.clone()),
            "connection tenure and delivery ordinal are intentionally absent"
        );
        assert_ne!(
            request,
            queue_plan_synced_request_id(&chain_b, entrypoint_b.clone())
        );
        assert_ne!(
            request,
            queue_plan_synced_request_id(&chain_a, entrypoint_b)
        );
        {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                queue_plan_synced_request_id_from_chain_digest(
                    chain_a_digest,
                    entrypoint_a.clone()
                ),
                request,
                "semantic request identity must ignore the caller's ambient Norito layout"
            );
        }
    }

    #[test]
    fn torii_transaction_admission_wire_indexes_are_stable() {
        assert_eq!(
            torii_transaction_admission_wire_index(
                ToriiProxyTransactionAdmissionV2::QueuePlanSynced
            ),
            2
        );
    }

    #[test]
    fn obsolete_transaction_admission_tags_fail_closed() {
        for obsolete in [
            ObsoleteToriiProxyTransactionAdmissionV1::Deferred,
            ObsoleteToriiProxyTransactionAdmissionV1::QueuePlanSynced,
        ] {
            let encoded = norito::codec::Encode::encode(&obsolete);
            assert!(
                ToriiProxyTransactionAdmissionV2::decode(&mut encoded.as_slice()).is_err(),
                "retired admission tag {obsolete:?} must not decode under the V2 contract"
            );
        }
    }

    fn single_route_admission_fixture() -> (
        ChainId,
        TransactionEntrypoint,
        RoutingPlan,
        QueuePlanAdmissionBindingV2,
        iroha_crypto::KeyPair,
    ) {
        let chain_id: ChainId = "queue-plan-semantic-certificate"
            .parse()
            .expect("fixture chain id");
        let transaction_signer =
            iroha_crypto::KeyPair::from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519);
        let validator =
            iroha_crypto::KeyPair::from_seed(vec![0x72; 32], iroha_crypto::Algorithm::Ed25519);
        let transaction = TransactionEntrypoint::External(
            iroha_data_model::transaction::TransactionBuilder::new(
                chain_id.clone(),
                iroha_data_model::account::AccountId::new(transaction_signer.public_key().clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .sign(transaction_signer.private_key()),
        );
        let route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(5));
        let routing_plan = RoutingPlan::single(route);
        let validator_set = vec![PeerId::new(validator.public_key().clone())];
        let context = crate::queue::QueuePlanAdmissionContextV2 {
            version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2,
            authority_height: 0,
            proposal_height: 1,
            predecessor_block_hash: None,
            routing_plan_digest: routing_plan.digest(),
            route_incarnations: vec![crate::queue::QueuePlanRouteIncarnationV2 {
                leg: RouteLeg::new(route, RouteLegRole::Coordinator),
                lane_incarnation: Hash::new(b"semantic certificate incarnation"),
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                durability_threshold: 1,
            }],
        };
        let binding =
            QueuePlanAdmissionBindingV2::new(&chain_id, &transaction, &routing_plan, context, 73)
                .expect("construct canonical admission binding");
        (chain_id, transaction, routing_plan, binding, validator)
    }

    #[test]
    fn lane_reservation_commit_separates_admission_and_reservation_heights() {
        let (chain_id, transaction, routing_plan, binding, _) = single_route_admission_fixture();
        let coordinator = binding
            .admission_context
            .route_incarnations
            .first()
            .expect("single-route fixture has a coordinator");
        let key = crate::queue::LaneQueueReservationKeyV2 {
            version: crate::queue::LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: HashOf::from_untyped_unchecked(Hash::from(
                binding.entrypoint_hash.clone(),
            )),
            entrypoint_hash: binding.entrypoint_hash.clone(),
            queue_plan_admission_binding_hash: binding.canonical_hash(),
            routing_plan_digest: binding.routing_plan_digest,
            coordinator_leg: coordinator.leg,
            lane_id: coordinator.leg.route.lane_id,
            dataspace_id: coordinator.leg.route.dataspace_id,
            lane_incarnation: coordinator.lane_incarnation,
            proposal_height: binding.admission_context.proposal_height,
            lane_block_height: 1,
            lane_block_view: 0,
            reservation_owner_hash: Hash::new(b"lane-reservation-owner"),
            proposal_identity_hash: Hash::new(b"lane-reservation-proposal"),
        };

        binding
            .validate_for_lane_reservation_commit(&key)
            .expect("matching admission and reservation heights must validate");

        let mut later_reservation = key;
        later_reservation.proposal_height += 1;
        binding
            .validate_for_lane_reservation_commit(&later_reservation)
            .expect("a later reservation slot retains the exact earlier admission binding");
        assert_ne!(
            key.digest(),
            later_reservation.digest(),
            "the distinct reservation height remains covered by the exact reservation identity"
        );

        let mut conflicting_binding = later_reservation;
        conflicting_binding.queue_plan_admission_binding_hash =
            Hash::new(b"different QueuePlan admission binding");
        let error = binding
            .validate_for_lane_reservation_commit(&conflicting_binding)
            .expect_err("a different admission binding must be rejected");
        assert!(
            error.contains("binding identity"),
            "unexpected rejection: {error}"
        );

        let mut height_two_context = binding.admission_context.clone();
        height_two_context.authority_height = 1;
        height_two_context.proposal_height = 2;
        height_two_context.predecessor_block_hash = Some(HashOf::from_untyped_unchecked(
            Hash::new(b"height-one committed predecessor"),
        ));
        let height_two_binding = QueuePlanAdmissionBindingV2::new(
            &chain_id,
            &transaction,
            &routing_plan,
            height_two_context,
            74,
        )
        .expect("construct a canonical height-two admission binding");
        let mut backdated_reservation = key;
        backdated_reservation.queue_plan_admission_binding_hash =
            height_two_binding.canonical_hash();
        let error = height_two_binding
            .validate_for_lane_reservation_commit(&backdated_reservation)
            .expect_err("a reservation slot before durable admission must be rejected");
        assert!(
            error.contains("precedes its durable QueuePlan admission"),
            "unexpected rejection: {error}"
        );
    }

    #[test]
    fn queue_plan_certificate_rejects_noncanonical_semantic_request_identity() {
        let (chain_id, transaction, routing_plan, mut forged, validator) =
            single_route_admission_fixture();
        forged.request_id = Hash::new(b"forged self-consistent certificate request identity");
        forged.journal_record_digest = crate::queue::queue_plan_journal_record_claim_digest(
            transaction,
            routing_plan,
            forged.admission_context.clone(),
            forged.enqueue_timestamp_ms,
            Some(forged.global_admission_identity()),
        )
        .expect("rebind forged semantic identity into its journal digest");
        let signing_bytes =
            queue_plan_admission_attestation_signing_bytes_v2(forged.canonical_hash(), 0)
                .expect("encode forged certificate preimage");
        let certificate = QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding: forged,
            attestations: vec![QueuePlanAdmissionAttestationV2 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
                validator_index: 0,
                signature: Signature::try_new(validator.private_key(), &signing_bytes)
                    .expect("sign internally consistent forged certificate"),
            }],
        };

        let error = validate_queue_plan_admission_certificate_v2(
            &chain_id,
            certificate,
            QueuePlanAdmissionCertificateStrengthV2::Quorum,
        )
        .expect_err("digest-valid signatures cannot authorize a noncanonical request identity");
        assert!(
            error.contains("noncanonical semantic request identity"),
            "unexpected rejection: {error}"
        );
    }

    #[test]
    fn queue_plan_certificate_boundary_is_canonical_and_ambient_independent() {
        let (chain_id, _, _, binding, validator) = single_route_admission_fixture();
        let binding_hash = binding.canonical_hash();
        let signing_bytes = queue_plan_admission_attestation_signing_bytes_v2(binding_hash, 0)
            .expect("encode canonical attestation preimage");
        let certificate = QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding,
            attestations: vec![QueuePlanAdmissionAttestationV2 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
                validator_index: 0,
                signature: Signature::try_new(validator.private_key(), &signing_bytes)
                    .expect("sign canonical admission certificate"),
            }],
        };
        let canonical =
            norito::encode_canonical(&certificate).expect("encode canonical admission certificate");
        decode_and_validate_queue_plan_admission_certificate_v2(&chain_id, &canonical)
            .expect("canonical admission certificate validates");

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&certificate).expect("encode alternate-layout admission certificate")
        };
        assert_ne!(
            alternate, canonical,
            "fixture must exercise a distinct non-canonical certificate layout"
        );
        assert!(
            decode_and_validate_queue_plan_admission_certificate_v2(&chain_id, &alternate).is_err(),
            "alternate-layout certificate must fail closed"
        );
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                certificate.binding.canonical_hash(),
                binding_hash,
                "binding identity must ignore the caller's ambient Norito layout"
            );
            assert_eq!(
                queue_plan_admission_attestation_signing_bytes_v2(binding_hash, 0)
                    .expect("encode attestation under alternate ambient layout"),
                signing_bytes
            );
            decode_and_validate_queue_plan_admission_certificate_v2(&chain_id, &canonical)
                .expect("canonical certificate must validate under alternate ambient layout");
        }
    }

    #[test]
    fn native_admission_binds_participants_but_uses_only_coordinator_quorum() {
        let chain_id: ChainId = "queue-plan-native-coordinator-quorum"
            .parse()
            .expect("fixture chain id");
        let transaction_signer =
            iroha_crypto::KeyPair::from_seed(vec![0x81; 32], iroha_crypto::Algorithm::Ed25519);
        let authority =
            iroha_data_model::account::AccountId::new(transaction_signer.public_key().clone());
        let transaction = TransactionEntrypoint::External(
            iroha_data_model::transaction::TransactionBuilder::new(
                chain_id.clone(),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .sign(transaction_signer.private_key()),
        );
        let coordinator_route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(7));
        let participant_route = RoutingDecision::new(LaneId::new(4), DataSpaceId::new(8));
        let routing_plan = RoutingPlan::native_amx(
            coordinator_route,
            vec![RouteLeg::new(participant_route, RouteLegRole::Participant)],
        );
        let coordinator_signers = (0_u8..4)
            .map(|seed| {
                iroha_crypto::KeyPair::from_seed(
                    vec![seed.saturating_add(0x90); 32],
                    iroha_crypto::Algorithm::Ed25519,
                )
            })
            .collect::<Vec<_>>();
        let participant_signer =
            iroha_crypto::KeyPair::from_seed(vec![0xA0; 32], iroha_crypto::Algorithm::Ed25519);
        let coordinator_roster = coordinator_signers
            .iter()
            .map(|signer| PeerId::new(signer.public_key().clone()))
            .collect::<Vec<_>>();
        let participant_roster = vec![PeerId::new(participant_signer.public_key().clone())];
        let context = crate::queue::QueuePlanAdmissionContextV2 {
            version: crate::queue::QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V2,
            authority_height: 0,
            proposal_height: 1,
            predecessor_block_hash: None,
            routing_plan_digest: routing_plan.digest(),
            route_incarnations: vec![
                crate::queue::QueuePlanRouteIncarnationV2 {
                    leg: RouteLeg::new(coordinator_route, RouteLegRole::Coordinator),
                    lane_incarnation: Hash::new(b"native-admission-coordinator-incarnation"),
                    validator_set_hash_version:
                        iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&coordinator_roster),
                    validator_set: coordinator_roster,
                    validator_count: 4,
                    durability_threshold: 2,
                },
                crate::queue::QueuePlanRouteIncarnationV2 {
                    leg: RouteLeg::new(participant_route, RouteLegRole::Participant),
                    lane_incarnation: Hash::new(b"native-admission-participant-incarnation"),
                    validator_set_hash_version:
                        iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&participant_roster),
                    validator_set: participant_roster,
                    validator_count: 1,
                    durability_threshold: 1,
                },
            ],
        };
        let binding =
            QueuePlanAdmissionBindingV2::new(&chain_id, &transaction, &routing_plan, context, 42)
                .expect("build Native admission binding");
        assert_eq!(
            binding.routing_plan().expect("bound Native routing plan"),
            routing_plan,
            "participant routing evidence must remain inside the shared binding"
        );
        let sign = |validator_index: u16, signer: &iroha_crypto::KeyPair| {
            let signing_bytes = queue_plan_admission_attestation_signing_bytes_v2(
                binding.canonical_hash(),
                validator_index,
            )
            .expect("encode attestation preimage");
            QueuePlanAdmissionAttestationV2 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
                validator_index,
                signature: Signature::try_new(signer.private_key(), &signing_bytes)
                    .expect("sign admission attestation"),
            }
        };
        let certificate = QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding: binding.clone(),
            attestations: vec![
                sign(0, &coordinator_signers[0]),
                sign(1, &coordinator_signers[1]),
            ],
        };
        let validated = validate_queue_plan_admission_certificate_v2(
            &chain_id,
            certificate,
            QueuePlanAdmissionCertificateStrengthV2::Quorum,
        )
        .expect("coordinator quorum certifies the participant-bound plan");
        assert_eq!(validated.coordinator_route, coordinator_route);
        assert_eq!(validated.durability_threshold, 2);

        let participant_attestations = vec![
            sign(0, &participant_signer),
            sign(1, &coordinator_signers[1]),
        ];
        let participant_certificate = QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding,
            attestations: participant_attestations,
        };
        assert!(
            validate_queue_plan_admission_certificate_v2(
                &chain_id,
                participant_certificate,
                QueuePlanAdmissionCertificateStrengthV2::Quorum,
            )
            .is_err(),
            "participant authority must not substitute for coordinator durability quorum"
        );
    }

    #[test]
    fn torii_proxy_v5_envelope_roundtrips_exact_request() {
        let request = ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"torii-proxy-v5-roundtrip"),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: ToriiProxyRequestKindV4::Read(ToriiReadProxyRequestV1 {
                endpoint: ToriiReadEndpointV1::AccountsList,
                expected_route: ToriiRouteHintV1 {
                    lane_id: LaneId::new(3),
                    dataspace_id: DataSpaceId::new(9),
                },
                path_args: Vec::new(),
                query_string: None,
                body: Vec::new(),
                response_format: ToriiProxyResponseFormatV1::Json,
            }),
        };

        let encoded = norito::to_bytes(&request).expect("encode V5 Torii proxy request");
        let decoded = norito::decode_from_bytes::<ToriiProxyRequestV5>(&encoded)
            .expect("decode V5 Torii proxy request");
        assert_eq!(decoded, request);
    }

    fn historical_v2_submit_fixture() -> HistoricalToriiProxyRequestV2 {
        let keypair =
            iroha_crypto::KeyPair::from_seed(vec![0x70; 32], iroha_crypto::Algorithm::Ed25519);
        let authority = iroha_data_model::account::AccountId::new(keypair.public_key().clone());
        let mut transaction_builder =
            iroha_data_model::transaction::signed::TransactionBuilder::new(
                "torii-proxy-historical-v2-fixture"
                    .parse()
                    .expect("fixture chain id"),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
        transaction_builder.set_creation_time(std::time::Duration::from_millis(41));
        let transaction =
            TransactionEntrypoint::External(transaction_builder.sign(keypair.private_key()));
        HistoricalToriiProxyRequestV2 {
            schema_version: LEGACY_TORII_PROXY_REQUEST_VERSION_V2,
            request_id: Hash::new(b"historical-v2-submit"),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: Vec::new(),
            request: HistoricalToriiProxyRequestKindV1::SubmitTransaction {
                transaction,
                expected_plan: ToriiRoutingPlanHintV1::from(RoutingPlan::single(
                    RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                )),
            },
        }
    }

    #[test]
    fn historical_v2_submit_and_network_carrier_cannot_be_accepted_as_v5() {
        let historical = historical_v2_submit_fixture();
        let historical_bytes =
            norito::to_bytes(&historical).expect("encode checked-in historical V2 Submit fixture");
        assert_eq!(
            &historical_bytes[6..22],
            norito::core::schema_hash_for_name("iroha_core::torii_proxy::ToriiProxyRequestV2")
                .as_slice(),
            "fixture must carry the checked-in V2 envelope schema"
        );
        assert_eq!(
            norito::decode_from_bytes::<HistoricalToriiProxyRequestV2>(&historical_bytes)
                .expect("decode checked-in historical V2 Submit fixture"),
            historical
        );
        assert!(
            norito::decode_from_bytes::<ToriiProxyRequestV5>(&historical_bytes).is_err(),
            "the checked-in V2 request must not decode as a V5 request"
        );

        let historical_network = HistoricalNetworkMessage::ToriiProxyRequest(Box::new(historical));
        let historical_network_bytes = norito::to_bytes(&historical_network)
            .expect("encode checked-in historical V2 network carrier");
        assert_eq!(
            &historical_network_bytes[6..22],
            <crate::NetworkMessage as norito::NoritoSerialize>::schema_hash().as_slice(),
            "the frozen carrier must use the production NetworkMessage schema"
        );
        assert_eq!(
            norito::decode_from_bytes::<HistoricalNetworkMessage>(&historical_network_bytes)
                .expect("decode checked-in historical V2 network carrier"),
            historical_network
        );
        assert!(
            norito::decode_from_bytes::<crate::NetworkMessage>(&historical_network_bytes).is_err(),
            "the live NetworkMessage decoder must reject a nested V2 request before dispatch"
        );
    }

    #[test]
    fn legacy_v2_submit_bool_wire_cannot_be_accepted_as_v5() {
        let keypair =
            iroha_crypto::KeyPair::from_seed(vec![0x71; 32], iroha_crypto::Algorithm::Ed25519);
        let authority = iroha_data_model::account::AccountId::new(keypair.public_key().clone());
        let mut transaction_builder =
            iroha_data_model::transaction::signed::TransactionBuilder::new(
                "torii-proxy-legacy-v2-fixture"
                    .parse()
                    .expect("fixture chain id"),
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
        transaction_builder.set_creation_time(std::time::Duration::from_millis(42));
        let transaction =
            TransactionEntrypoint::External(transaction_builder.sign(keypair.private_key()));
        let expected_plan = ToriiRoutingPlanHintV1::from(RoutingPlan::single(
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        ));

        for strict_durable in [false, true] {
            let legacy = TransientBoolToriiProxyRequestV2 {
                schema_version: LEGACY_TORII_PROXY_REQUEST_VERSION_V2,
                request_id: Hash::new(if strict_durable {
                    &b"legacy-v2-strict-submit"[..]
                } else {
                    &b"legacy-v2-deferred-submit"[..]
                }),
                hop_count: 1,
                max_hops: 3,
                visited_peer_ids: Vec::new(),
                request: TransientBoolToriiProxyRequestKindV1::SubmitTransaction {
                    transaction: transaction.clone(),
                    expected_plan: expected_plan.clone(),
                    strict_durable,
                },
            };
            let legacy_bytes =
                norito::to_bytes(&legacy).expect("encode frozen legacy V2 Submit fixture");
            assert_eq!(
                &legacy_bytes[6..22],
                norito::core::schema_hash_for_name("iroha_core::torii_proxy::ToriiProxyRequestV2")
                    .as_slice(),
                "fixture must carry the historical V2 envelope schema"
            );
            assert_eq!(
                legacy_bytes.last().copied(),
                Some(u8::from(strict_durable)),
                "historical Submit must end in its one-byte strict_durable field"
            );
            assert_eq!(
                norito::decode_from_bytes::<TransientBoolToriiProxyRequestV2>(&legacy_bytes)
                    .expect("decode frozen legacy V2 Submit fixture"),
                legacy
            );
            assert!(
                norito::decode_from_bytes::<ToriiProxyRequestV5>(&legacy_bytes).is_err(),
                "the genuine V2 frame must not decode as a V5 request"
            );

            let mut relabeled_as_v5 = legacy_bytes;
            relabeled_as_v5[6..22]
                .copy_from_slice(&<ToriiProxyRequestV5 as norito::NoritoSerialize>::schema_hash());
            assert!(
                norito::decode_from_bytes::<ToriiProxyRequestV5>(&relabeled_as_v5).is_err(),
                "even a V5 schema label must not turn the legacy one-byte bool payload into the \
                 V5 admission-binding shape; no compatibility fallback is permitted"
            );
        }
    }

    #[test]
    fn torii_read_endpoint_wire_indexes_match_first_release_schema() {
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountTransactionsGet),
            5
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountTransactionsQuery),
            6
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::AccountHistoryGet),
            40
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountGet),
            41
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountTransactionGet),
            42
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::InternalAccountAssetGet),
            43
        );
        assert_eq!(
            torii_read_endpoint_wire_index(ToriiReadEndpointV1::ContractDeploymentState),
            44
        );
    }

    fn native_amx_participant_legs(count: usize) -> Vec<RouteLeg> {
        (0..count)
            .map(|index| {
                let ordinal = u32::try_from(index + 1).expect("participant fixture index fits u32");
                RouteLeg::new(
                    RoutingDecision::new(
                        LaneId::new(ordinal),
                        DataSpaceId::new(u64::from(ordinal)),
                    ),
                    RouteLegRole::Participant,
                )
            })
            .collect()
    }

    #[test]
    fn torii_routing_plan_hint_roundtrips_single_and_native_amx_plans() {
        let single_route = RoutingDecision::new(LaneId::new(4), DataSpaceId::new(9));
        let single_hint = ToriiRoutingPlanHintV1::from(RoutingPlan::single(single_route));

        assert_eq!(
            single_hint.coordinator_route(),
            ToriiRouteHintV1 {
                lane_id: single_route.lane_id,
                dataspace_id: single_route.dataspace_id,
            }
        );
        assert_eq!(
            single_hint
                .clone()
                .try_into_routing_plan()
                .expect("canonical single-route hint should validate"),
            RoutingPlan::single(single_route)
        );
        assert_eq!(
            RoutingPlan::try_from(single_hint).expect("single routing hint should validate"),
            RoutingPlan::single(single_route)
        );

        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let native_plan = RoutingPlan::native_amx(
            coordinator,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
                    RouteLegRole::Coordinator,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7)),
                    RouteLegRole::Coordinator,
                ),
            ],
        );
        let native_hint = ToriiRoutingPlanHintV1::from(native_plan.clone());

        assert_eq!(
            native_hint.coordinator_route(),
            ToriiRouteHintV1 {
                lane_id: coordinator.lane_id,
                dataspace_id: coordinator.dataspace_id,
            }
        );
        let ToriiRoutingPlanHintV1::NativeAmx {
            plan_digest,
            participants,
            ..
        } = &native_hint
        else {
            panic!("expected native AMX routing plan hint");
        };
        assert_eq!(*plan_digest, native_plan.digest());
        assert!(
            participants
                .iter()
                .all(|leg| leg.role == ToriiRouteLegRoleV1::Participant)
        );
        assert_eq!(
            native_hint
                .clone()
                .try_into_routing_plan()
                .expect("canonical native AMX hint should validate"),
            native_plan
        );
        assert_eq!(
            RoutingPlan::try_from(native_hint).expect("native AMX routing hint should validate"),
            native_plan
        );
    }

    #[test]
    fn torii_routing_plan_hint_enforces_native_amx_participant_limit() {
        assert_eq!(
            TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1, 255,
            "Torii hint bound must remain source-bound to the Native AMX protocol cap"
        );
        let coordinator = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let maximum_plan = RoutingPlan::native_amx(
            coordinator,
            native_amx_participant_legs(TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1),
        );
        assert_eq!(
            ToriiRoutingPlanHintV1::from(maximum_plan.clone()).try_into_routing_plan(),
            Ok(maximum_plan),
            "the exact participant limit must remain admissible"
        );

        let oversized_count = TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1 + 1;
        let oversized_hint = ToriiRoutingPlanHintV1::from(RoutingPlan::native_amx(
            coordinator,
            native_amx_participant_legs(oversized_count),
        ));
        let error = oversized_hint
            .try_into_routing_plan()
            .expect_err("a participant vector above the protocol cap must fail closed");
        assert_eq!(
            error,
            ToriiRoutingPlanHintError::native_amx_participant_limit_exceeded(
                oversized_count,
                TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1,
            )
        );
        assert_eq!(
            error.kind(),
            ToriiRoutingPlanHintErrorKind::NativeAmxParticipantLimitExceeded
        );
        assert_eq!(error.participant_count(), Some(oversized_count));
        assert_eq!(
            error.participant_limit(),
            Some(TORII_ROUTING_PLAN_MAX_NATIVE_AMX_PARTICIPANTS_V1)
        );
        assert_eq!(
            error.to_string(),
            "native AMX participant count 256 exceeds protocol limit 255"
        );
    }

    #[test]
    fn torii_routing_plan_hint_rejects_duplicate_participants_without_deduplication() {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let plan = RoutingPlan::native_amx(coordinator, native_amx_participant_legs(2));
        let mut hint = ToriiRoutingPlanHintV1::from(plan);
        let ToriiRoutingPlanHintV1::NativeAmx { participants, .. } = &mut hint else {
            panic!("expected native AMX hint");
        };
        let duplicate = participants[0];
        participants.insert(1, duplicate);

        let error = hint
            .try_into_routing_plan()
            .expect_err("duplicate participant hints must not be silently deduplicated");
        assert_eq!(
            error,
            ToriiRoutingPlanHintError::native_amx_duplicate_participant_route(1, duplicate.route)
        );
        assert_eq!(
            error.kind(),
            ToriiRoutingPlanHintErrorKind::NativeAmxDuplicateParticipantRoute
        );
        assert_eq!(error.leg_index(), Some(1));
        assert_eq!(error.previous_route(), Some(duplicate.route));
        assert_eq!(error.actual_route(), Some(duplicate.route));
        assert_eq!(
            error.to_string(),
            "duplicate native AMX participant route at index 1: dataspace 1, lane 1"
        );
    }

    #[test]
    fn torii_routing_plan_hint_rejects_out_of_order_participants_without_sorting() {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let plan = RoutingPlan::native_amx(coordinator, native_amx_participant_legs(2));
        let mut hint = ToriiRoutingPlanHintV1::from(plan);
        let ToriiRoutingPlanHintV1::NativeAmx { participants, .. } = &mut hint else {
            panic!("expected native AMX hint");
        };
        participants.swap(0, 1);
        let previous = participants[0].route;
        let actual = participants[1].route;

        let error = hint
            .try_into_routing_plan()
            .expect_err("out-of-order participant hints must not be silently sorted");
        assert_eq!(
            error,
            ToriiRoutingPlanHintError::native_amx_participants_out_of_order(1, previous, actual,)
        );
        assert_eq!(
            error.kind(),
            ToriiRoutingPlanHintErrorKind::NativeAmxParticipantsOutOfOrder
        );
        assert_eq!(error.leg_index(), Some(1));
        assert_eq!(error.previous_route(), Some(previous));
        assert_eq!(error.actual_route(), Some(actual));
        assert_eq!(
            error.to_string(),
            "native AMX participant routes are out of canonical (dataspace, lane) order at index \
             1: previous (2, 2), actual (1, 1)"
        );
    }

    #[test]
    fn torii_routing_plan_hint_rejects_forged_digest_and_roles() {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let native_plan = RoutingPlan::native_amx(
            coordinator,
            vec![
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
                    RouteLegRole::Participant,
                ),
                RouteLeg::new(
                    RoutingDecision::new(LaneId::new(3), DataSpaceId::new(9)),
                    RouteLegRole::Participant,
                ),
            ],
        );

        let mut forged_digest = ToriiRoutingPlanHintV1::from(native_plan.clone());
        let advertised = Hash::new(b"forged-native-amx-plan-digest");
        let ToriiRoutingPlanHintV1::NativeAmx { plan_digest, .. } = &mut forged_digest else {
            panic!("expected native AMX hint");
        };
        *plan_digest = advertised;
        assert_eq!(
            forged_digest.clone().try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
                advertised,
                native_plan.digest()
            ))
        );
        assert_eq!(
            RoutingPlan::try_from(forged_digest),
            Err(ToriiRoutingPlanHintError::native_amx_plan_digest_mismatch(
                advertised,
                native_plan.digest()
            ))
        );

        let wrong_single_role = ToriiRoutingPlanHintV1::Single(ToriiRouteLegHintV1 {
            route: ToriiRouteHintV1::from(coordinator),
            role: ToriiRouteLegRoleV1::Participant,
        });
        assert_eq!(
            wrong_single_role.try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::unexpected_coordinator_role(
                ToriiRouteLegRoleV1::Participant
            ))
        );

        let mut wrong_participant_role = ToriiRoutingPlanHintV1::from(native_plan);
        let ToriiRoutingPlanHintV1::NativeAmx { participants, .. } = &mut wrong_participant_role
        else {
            panic!("expected native AMX hint");
        };
        participants[1].role = ToriiRouteLegRoleV1::Coordinator;
        assert_eq!(
            wrong_participant_role.try_into_routing_plan(),
            Err(ToriiRoutingPlanHintError::unexpected_participant_role(
                1,
                ToriiRouteLegRoleV1::Coordinator
            ))
        );
    }
}
