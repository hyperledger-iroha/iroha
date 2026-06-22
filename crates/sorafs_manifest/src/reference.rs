//! Reference validation outcomes for SoraFS operator tooling.
//!
//! This module backs the first SF-11 validator slice. It reuses the canonical
//! SoraFS Norito models and reports deterministic, machine-readable outcomes
//! that can be consumed by CLIs, CI, and dashboards.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AdmissionRecord, AdvertValidationError, AuditorSignatureVerificationError,
    GovernanceDagBlockV1, GovernanceDagBlockValidationError, GovernanceDagChainValidationError,
    GovernanceDagHeadChainValidationError, GovernanceDagHeadV1, GovernanceDagHeadValidationError,
    GovernanceLogNodeV1, GovernanceLogPayloadV1, GovernanceLogSignatureVerificationError,
    GovernanceLogValidationError, GovernanceSignatureAlgorithm, OrderCancelReasonV1, OrderCancelV1,
    OrderRequestV1, OrderSideV1, OrderTierV1, OrderbookValidationError, PdpChallengeV1,
    PdpChallengeValidationError, PdpCommitmentV1, PdpCommitmentValidationError, PdpProofV1,
    PdpProofValidationError, PorChallengeV1, PorChallengeValidationError, PorProofV1,
    PorProofValidationError, PotrReceiptV1, PotrReceiptValidationError, ProofStreamTier,
    ProviderAdmissionEnvelopeError, ProviderAdmissionEnvelopeV1, ProviderAdmissionRenewalError,
    ProviderAdmissionRenewalV1, ProviderAdmissionRevocationError, ProviderAdmissionRevocationV1,
    ProviderAdmissionValidationError, ProviderAdvertV1, RepairAuditEventV1,
    RepairEscalationApprovalV1, RepairEscalationPolicyV1, RepairEvidenceV1, RepairReportV1,
    RepairSlashProposalV1, RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1,
    RepairValidationError, RepairWorkerActionV1, RepairWorkerSignaturePayloadV1,
    ReplicationOrderSignatureVerificationError, ReplicationOrderV1,
    ReplicationOrderValidationError, SettlementChannelStatusV1, SettlementChannelV1,
    SettlementReceiptV1, SignatureAlgorithm, SignedAuditorRequestPayloadV1, SignedAuditorRequestV1,
    SignedReplicationOrderV1, SignedReplicationOrderValidationError, TradeEventV1,
    validate_governance_dag_head_against_chain_v1, verify_envelope,
};

/// Current schema version for [`ValidationOutcomeV1`].
pub const VALIDATION_OUTCOME_VERSION_V1: u8 = 1;

/// Error-catalogue document referenced by emitted outcomes.
pub const REFERENCE_SDK_ERRORS_DOC_URL: &str = "docs/portal/docs/sorafs/reference-sdk/errors.md";

const STATUS_OK: &str = "Ok";
const STATUS_ERROR: &str = "Error";
const CATEGORY_VALIDATION: &str = "validation";
const CATEGORY_POLICY: &str = "policy";
const CATEGORY_SIGNATURE: &str = "signature";
const CATEGORY_NORITO: &str = "norito";
const CATEGORY_INTERNAL: &str = "internal";

/// Structured key/value context attached to validation outcomes.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ValidationContextFieldV1 {
    /// Context key.
    pub key: String,
    /// Context value.
    pub value: String,
}

impl ValidationContextFieldV1 {
    /// Creates a validation context field.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Input metadata attached to validation outcomes.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ValidationInputV1 {
    /// Input kind, for example `provider_advert`.
    pub kind: String,
    /// Human-readable input path or label.
    pub path: String,
}

impl ValidationInputV1 {
    /// Creates input metadata for an outcome.
    #[must_use]
    pub fn new(kind: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            path: path.into(),
        }
    }
}

/// Deterministic validation result for SF-11 reference tooling.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct ValidationOutcomeV1 {
    /// `Ok` for accepted payloads, `Error` for rejected payloads.
    pub status: String,
    /// Stable machine-readable outcome code.
    pub code: String,
    /// Error category such as `validation`, `policy`, `signature`, or `norito`.
    pub category: String,
    /// Human-readable diagnostic.
    pub message: String,
    /// Suggested operator action for rejected inputs.
    pub action: Option<String>,
    /// Documentation path for the code catalogue.
    pub docs_url: Option<String>,
    /// Labels suitable for telemetry dimensions.
    pub telemetry_tags: Vec<String>,
    /// Structured context fields.
    pub context: Vec<ValidationContextFieldV1>,
    /// Inputs considered by the validator.
    pub inputs: Vec<ValidationInputV1>,
    /// Outcome schema version.
    pub version: u8,
    /// UNIX timestamp when the outcome was generated.
    pub generated_at: u64,
}

impl ValidationOutcomeV1 {
    /// Creates a successful validation outcome.
    #[must_use]
    pub fn ok(
        code: impl Into<String>,
        message: impl Into<String>,
        telemetry_tags: Vec<String>,
        context: Vec<ValidationContextFieldV1>,
        inputs: Vec<ValidationInputV1>,
        generated_at: u64,
    ) -> Self {
        Self {
            status: STATUS_OK.to_owned(),
            code: code.into(),
            category: CATEGORY_VALIDATION.to_owned(),
            message: message.into(),
            action: None,
            docs_url: Some(REFERENCE_SDK_ERRORS_DOC_URL.to_owned()),
            telemetry_tags,
            context,
            inputs,
            version: VALIDATION_OUTCOME_VERSION_V1,
            generated_at,
        }
    }

    /// Creates a rejected validation outcome.
    #[must_use]
    pub fn error(
        code: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        action: impl Into<String>,
        telemetry_tags: Vec<String>,
        context: Vec<ValidationContextFieldV1>,
        inputs: Vec<ValidationInputV1>,
        generated_at: u64,
    ) -> Self {
        Self {
            status: STATUS_ERROR.to_owned(),
            code: code.into(),
            category: category.into(),
            message: message.into(),
            action: Some(action.into()),
            docs_url: Some(REFERENCE_SDK_ERRORS_DOC_URL.to_owned()),
            telemetry_tags,
            context,
            inputs,
            version: VALIDATION_OUTCOME_VERSION_V1,
            generated_at,
        }
    }

    /// Returns true when this outcome represents an accepted payload.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status == STATUS_OK
    }
}

/// Payload kinds accepted by the fixture bundle cross-link validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureBundlePayloadKindV1 {
    /// [`ProviderAdvertV1`] payload.
    ProviderAdvert,
    /// [`ProviderAdmissionEnvelopeV1`] payload.
    ProviderAdmissionEnvelope,
    /// [`ReplicationOrderV1`] payload.
    ReplicationOrder,
    /// [`PdpCommitmentV1`] payload.
    PdpCommitment,
    /// [`PdpChallengeV1`] payload.
    PdpChallenge,
    /// [`PdpProofV1`] payload.
    PdpProof,
    /// [`PorChallengeV1`] payload.
    PorChallenge,
    /// [`PorProofV1`] payload.
    PorProof,
    /// [`PotrReceiptV1`] payload.
    PotrReceipt,
    /// [`RepairEvidenceV1`] payload.
    RepairEvidence,
    /// [`RepairReportV1`] payload.
    RepairReport,
    /// [`RepairTaskRecordV1`] payload.
    RepairTaskRecord,
    /// [`RepairSlashProposalV1`] payload.
    RepairSlashProposal,
    /// [`RepairTaskEventV1`] payload.
    RepairTaskEvent,
    /// [`OrderRequestV1`] orderbook payload.
    OrderbookOrderRequest,
    /// [`OrderCancelV1`] orderbook payload.
    OrderbookOrderCancel,
    /// [`TradeEventV1`] orderbook payload.
    OrderbookTradeEvent,
    /// [`SettlementChannelV1`] orderbook payload.
    OrderbookSettlementChannel,
    /// [`SettlementReceiptV1`] orderbook payload.
    OrderbookSettlementReceipt,
}

impl FixtureBundlePayloadKindV1 {
    fn input_kind(self) -> &'static str {
        match self {
            Self::ProviderAdvert => "provider_advert",
            Self::ProviderAdmissionEnvelope => "provider_admission_envelope",
            Self::ReplicationOrder => "replication_order",
            Self::PdpCommitment => "pdp_commitment",
            Self::PdpChallenge => "pdp_challenge",
            Self::PdpProof => "pdp_proof",
            Self::PorChallenge => "por_challenge",
            Self::PorProof => "por_proof",
            Self::PotrReceipt => "potr_receipt",
            Self::RepairEvidence => "repair_evidence",
            Self::RepairReport => "repair_report",
            Self::RepairTaskRecord => "repair_task_record",
            Self::RepairSlashProposal => "repair_slash_proposal",
            Self::RepairTaskEvent => "repair_task_event",
            Self::OrderbookOrderRequest => "orderbook_order_request",
            Self::OrderbookOrderCancel => "orderbook_order_cancel",
            Self::OrderbookTradeEvent => "orderbook_trade_event",
            Self::OrderbookSettlementChannel => "settlement_channel",
            Self::OrderbookSettlementReceipt => "settlement_receipt",
        }
    }

    fn schema(self) -> &'static str {
        match self {
            Self::ProviderAdvert => "ProviderAdvertV1",
            Self::ProviderAdmissionEnvelope => "ProviderAdmissionEnvelopeV1",
            Self::ReplicationOrder => "ReplicationOrderV1",
            Self::PdpCommitment => "PdpCommitmentV1",
            Self::PdpChallenge => "PdpChallengeV1",
            Self::PdpProof => "PdpProofV1",
            Self::PorChallenge => "PorChallengeV1",
            Self::PorProof => "PorProofV1",
            Self::PotrReceipt => "PotrReceiptV1",
            Self::RepairEvidence => "RepairEvidenceV1",
            Self::RepairReport => "RepairReportV1",
            Self::RepairTaskRecord => "RepairTaskRecordV1",
            Self::RepairSlashProposal => "RepairSlashProposalV1",
            Self::RepairTaskEvent => "RepairTaskEventV1",
            Self::OrderbookOrderRequest => "OrderRequestV1",
            Self::OrderbookOrderCancel => "OrderCancelV1",
            Self::OrderbookTradeEvent => "TradeEventV1",
            Self::OrderbookSettlementChannel => "SettlementChannelV1",
            Self::OrderbookSettlementReceipt => "SettlementReceiptV1",
        }
    }
}

/// A Norito payload included in a fixture-directory bundle validation run.
pub struct FixtureBundlePayloadV1<'payload> {
    /// Payload kind.
    pub kind: FixtureBundlePayloadKindV1,
    /// Human-readable input path or label.
    pub label: String,
    /// Norito bytes for the payload.
    pub bytes: &'payload [u8],
}

impl<'payload> FixtureBundlePayloadV1<'payload> {
    /// Creates a fixture bundle payload reference.
    #[must_use]
    pub fn new(
        kind: FixtureBundlePayloadKindV1,
        label: impl Into<String>,
        bytes: &'payload [u8],
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            bytes,
        }
    }
}

#[derive(Debug, Default)]
struct FixtureBundleLinks {
    manifest_digest: Option<([u8; 32], String)>,
    order_assignment_providers: BTreeSet<[u8; 32]>,
    linked_providers: Vec<FixtureBundleProviderLink>,
    linkable_artifacts: BTreeSet<String>,
}

#[derive(Debug)]
struct FixtureBundleProviderLink {
    kind: &'static str,
    label: String,
    provider_id: [u8; 32],
    requires_order_assignment: bool,
}

impl FixtureBundleLinks {
    fn observe_manifest(
        &mut self,
        digest: [u8; 32],
        label: &str,
        inputs: Vec<ValidationInputV1>,
        generated_at: u64,
    ) -> Result<(), ValidationOutcomeV1> {
        self.linkable_artifacts.insert(label.to_owned());
        if let Some((expected, expected_label)) = &self.manifest_digest {
            if expected != &digest {
                return Err(bundle_error(
                    "SFS-BND-002",
                    CATEGORY_VALIDATION,
                    "fixture bundle manifest digest mismatch",
                    "Regenerate the bundle so replication orders, PoR/PoTR receipts, and repair payloads name the same manifest digest.",
                    vec![
                        ValidationContextFieldV1::new(
                            "expected_manifest_digest_hex",
                            hex::encode(expected),
                        ),
                        ValidationContextFieldV1::new(
                            "expected_manifest_source",
                            expected_label.clone(),
                        ),
                        ValidationContextFieldV1::new(
                            "actual_manifest_digest_hex",
                            hex::encode(digest),
                        ),
                        ValidationContextFieldV1::new("actual_manifest_source", label.to_owned()),
                    ],
                    inputs,
                    generated_at,
                ));
            }
        } else {
            self.manifest_digest = Some((digest, label.to_owned()));
        }
        Ok(())
    }

    fn observe_provider(
        &mut self,
        kind: &'static str,
        label: &str,
        provider_id: [u8; 32],
        requires_order_assignment: bool,
    ) {
        self.linkable_artifacts.insert(label.to_owned());
        self.linked_providers.push(FixtureBundleProviderLink {
            kind,
            label: label.to_owned(),
            provider_id,
            requires_order_assignment,
        });
    }

    fn finish(
        self,
        inputs: Vec<ValidationInputV1>,
        generated_at: u64,
    ) -> Result<Vec<ValidationContextFieldV1>, ValidationOutcomeV1> {
        let linkable_artifacts = self.linkable_artifacts.len();
        if linkable_artifacts < 2 {
            return Err(bundle_error(
                "SFS-BND-001",
                CATEGORY_VALIDATION,
                "fixture bundle must contain at least two linkable SoraFS artifacts",
                "Point --bundle at a fixture directory containing at least two known SoraFS artifacts such as an order plus PoR proof, or an advert plus admission envelope.",
                vec![ValidationContextFieldV1::new(
                    "linkable_artifacts",
                    linkable_artifacts.to_string(),
                )],
                inputs,
                generated_at,
            ));
        }

        if self.order_assignment_providers.is_empty() {
            if let Some(first) = self.linked_providers.first() {
                for link in self.linked_providers.iter().skip(1) {
                    if link.provider_id != first.provider_id {
                        return Err(provider_mismatch_error(
                            "provider_id does not match the first provider-bearing artifact",
                            first.provider_id,
                            &first.label,
                            link,
                            inputs,
                            generated_at,
                        ));
                    }
                }
            }
        } else {
            let mut first_provider_only_link: Option<&FixtureBundleProviderLink> = None;
            for link in &self.linked_providers {
                if link.requires_order_assignment
                    && !self.order_assignment_providers.contains(&link.provider_id)
                {
                    return Err(provider_mismatch_error(
                        "provider_id is not assigned by the replication order",
                        *self
                            .order_assignment_providers
                            .iter()
                            .next()
                            .expect("order assignment set is non-empty"),
                        "replication_order.assignments",
                        link,
                        inputs,
                        generated_at,
                    ));
                } else if !link.requires_order_assignment {
                    if let Some(first) = first_provider_only_link {
                        if link.provider_id != first.provider_id {
                            return Err(provider_mismatch_error(
                                "provider_id does not match the first provider-admission artifact",
                                first.provider_id,
                                &first.label,
                                link,
                                inputs,
                                generated_at,
                            ));
                        }
                    } else {
                        first_provider_only_link = Some(link);
                    }
                }
            }
        }

        let mut context = vec![
            ValidationContextFieldV1::new("artifact_count", inputs.len().to_string()),
            ValidationContextFieldV1::new("linkable_artifacts", linkable_artifacts.to_string()),
            ValidationContextFieldV1::new(
                "linked_provider_count",
                self.linked_providers.len().to_string(),
            ),
            ValidationContextFieldV1::new(
                "order_assignment_count",
                self.order_assignment_providers.len().to_string(),
            ),
        ];
        if let Some((manifest_digest, manifest_source)) = self.manifest_digest {
            context.push(ValidationContextFieldV1::new(
                "manifest_digest_hex",
                hex::encode(manifest_digest),
            ));
            context.push(ValidationContextFieldV1::new(
                "manifest_source",
                manifest_source,
            ));
        }
        if !self.order_assignment_providers.is_empty() {
            let providers = self
                .order_assignment_providers
                .iter()
                .map(hex::encode)
                .collect::<Vec<_>>()
                .join(",");
            context.push(ValidationContextFieldV1::new(
                "order_assignment_provider_ids_hex",
                providers,
            ));
        }
        Ok(context)
    }
}

/// Validates a fixture-directory payload set and checks manifest/provider links.
#[must_use]
pub fn validate_fixture_bundle_payloads(
    payloads: &[FixtureBundlePayloadV1<'_>],
    now: u64,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs: Vec<_> = payloads
        .iter()
        .map(|payload| ValidationInputV1::new(payload.kind.input_kind(), payload.label.clone()))
        .collect();
    let mut links = FixtureBundleLinks::default();

    for payload in payloads {
        if let Err(outcome) =
            validate_fixture_bundle_payload(payload, &mut links, now, generated_at)
        {
            return remap_bundle_payload_error(outcome, inputs, generated_at);
        }
    }

    if let (Some(challenge), Some(proof)) = (
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PdpChallenge),
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PdpProof),
    ) {
        let outcome = validate_pdp_challenge_proof_bytes(
            challenge.bytes,
            proof.bytes,
            challenge.label.clone(),
            proof.label.clone(),
            generated_at,
        );
        if !outcome.is_ok() {
            return remap_bundle_payload_error(outcome, inputs, generated_at);
        }
    }

    if let (Some(commitment), Some(challenge)) = (
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PdpCommitment),
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PdpChallenge),
    ) {
        let outcome = validate_pdp_commitment_challenge_bytes(
            commitment.bytes,
            challenge.bytes,
            commitment.label.clone(),
            challenge.label.clone(),
            generated_at,
        );
        if !outcome.is_ok() {
            return remap_bundle_payload_error(outcome, inputs, generated_at);
        }
    }

    if let (Some(challenge), Some(proof)) = (
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PorChallenge),
        payloads
            .iter()
            .find(|payload| payload.kind == FixtureBundlePayloadKindV1::PorProof),
    ) {
        let outcome = validate_por_challenge_proof_bytes(
            challenge.bytes,
            proof.bytes,
            challenge.label.clone(),
            proof.label.clone(),
            generated_at,
        );
        if !outcome.is_ok() {
            return remap_bundle_payload_error(outcome, inputs, generated_at);
        }
    }

    let context = match links.finish(inputs.clone(), generated_at) {
        Ok(context) => context,
        Err(outcome) => return outcome,
    };

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "fixture bundle cross-links accepted",
        vec![
            "sorafs.reference.bundle".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn validate_fixture_bundle_payload(
    payload: &FixtureBundlePayloadV1<'_>,
    links: &mut FixtureBundleLinks,
    now: u64,
    generated_at: u64,
) -> Result<(), ValidationOutcomeV1> {
    match payload.kind {
        FixtureBundlePayloadKindV1::ProviderAdvert => {
            let advert = decode_bundle_payload::<ProviderAdvertV1>(payload, generated_at)?;
            let mut context = advert_context(&advert);
            if let Err(error) = advert.validate_with_body(now) {
                let code = advert_validation_code(&error);
                let category = advert_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "validation_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("provider advert validation failed: {error}"),
                    "Regenerate the advert from governed provider metadata and retry validation.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            if let Err(error) = advert.verify_signature() {
                context.push(ValidationContextFieldV1::new(
                    "signature_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    "SFS-SIG-001",
                    CATEGORY_SIGNATURE,
                    format!("provider advert signature validation failed: {error}"),
                    "Resign the advert body with the governed provider Ed25519 key.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                advert.body.provider_id,
                false,
            );
        }
        FixtureBundlePayloadKindV1::ProviderAdmissionEnvelope => {
            let envelope =
                decode_bundle_payload::<ProviderAdmissionEnvelopeV1>(payload, generated_at)?;
            let mut context = provider_admission_context(&envelope);
            if let Err(error) = verify_envelope(&envelope) {
                let code = provider_admission_envelope_code(&error);
                let category = provider_admission_envelope_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "validation_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("provider admission envelope validation failed: {error}"),
                    "Regenerate the admission envelope from governed provider metadata and council signatures.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                envelope.proposal.provider_id,
                false,
            );
        }
        FixtureBundlePayloadKindV1::ReplicationOrder => {
            let order = decode_bundle_payload::<ReplicationOrderV1>(payload, generated_at)?;
            let mut context = replication_order_context(&order);
            if let Err(error) = order.validate() {
                let code = replication_order_validation_code(&error);
                let category = replication_order_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "validation_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("replication order validation failed: {error}"),
                    "Regenerate the replication order from governed manifest and provider assignments.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                order.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            for assignment in &order.assignments {
                links
                    .order_assignment_providers
                    .insert(assignment.provider_id);
            }
        }
        FixtureBundlePayloadKindV1::PdpCommitment => {
            let commitment = decode_bundle_payload::<PdpCommitmentV1>(payload, generated_at)?;
            let mut context = pdp_commitment_context(&commitment);
            if let Err(error) = commitment.validate() {
                let code = pdp_commitment_validation_code(&error);
                let category = pdp_commitment_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "commitment_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("PDP commitment validation failed: {error}"),
                    "Regenerate the PDP commitment from the canonical manifest chunk profile and commitment tree roots.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                commitment.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
        }
        FixtureBundlePayloadKindV1::PdpChallenge => {
            let challenge = decode_bundle_payload::<PdpChallengeV1>(payload, generated_at)?;
            let mut context = pdp_challenge_context(&challenge);
            if let Err(error) = challenge.validate() {
                let code = pdp_challenge_validation_code(&error);
                let category = pdp_challenge_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "challenge_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("PDP challenge validation failed: {error}"),
                    "Regenerate the PDP challenge from the canonical commitment, provider, epoch, and randomness inputs.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                challenge.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                challenge.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::PdpProof => {
            let proof = decode_bundle_payload::<PdpProofV1>(payload, generated_at)?;
            let mut context = pdp_proof_context(&proof);
            if let Err(error) = proof.validate() {
                let code = pdp_proof_validation_code(&error);
                let category = pdp_proof_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "proof_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("PDP proof validation failed: {error}"),
                    "Regenerate the PDP proof from the challenged segments, hot-leaf witnesses, and provider signature.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                proof.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                proof.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::PorChallenge => {
            let challenge = decode_bundle_payload::<PorChallengeV1>(payload, generated_at)?;
            let mut context = vec![
                ValidationContextFieldV1::new(
                    "challenge_id_hex",
                    hex::encode(challenge.challenge_id),
                ),
                ValidationContextFieldV1::new(
                    "manifest_digest_hex",
                    hex::encode(challenge.manifest_digest),
                ),
                ValidationContextFieldV1::new(
                    "provider_id_hex",
                    hex::encode(challenge.provider_id),
                ),
            ];
            if let Err(error) = challenge.validate() {
                let code = por_challenge_validation_code(&error);
                let category = por_challenge_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "challenge_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("PoR challenge validation failed: {error}"),
                    "Regenerate the PoR challenge from the canonical manifest, provider, epoch, and randomness inputs.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                challenge.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                challenge.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::PorProof => {
            let proof = decode_bundle_payload::<PorProofV1>(payload, generated_at)?;
            let mut context = vec![
                ValidationContextFieldV1::new("challenge_id_hex", hex::encode(proof.challenge_id)),
                ValidationContextFieldV1::new(
                    "manifest_digest_hex",
                    hex::encode(proof.manifest_digest),
                ),
                ValidationContextFieldV1::new("provider_id_hex", hex::encode(proof.provider_id)),
                ValidationContextFieldV1::new("proof_samples", proof.samples.len().to_string()),
            ];
            if let Err(error) = proof.validate() {
                let code = por_proof_validation_code(&error);
                context.push(ValidationContextFieldV1::new(
                    "proof_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    CATEGORY_VALIDATION,
                    format!("PoR proof validation failed: {error}"),
                    "Regenerate the PoR proof from the challenged chunks and canonical authentication path.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                proof.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                proof.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::PotrReceipt => {
            let receipt = decode_bundle_payload::<PotrReceiptV1>(payload, generated_at)?;
            let mut context = potr_context(&receipt);
            if let Err(error) = receipt.validate() {
                let code = potr_receipt_validation_code(&error);
                let category = potr_receipt_validation_category(&error);
                context.push(ValidationContextFieldV1::new(
                    "receipt_error",
                    error.to_string(),
                ));
                return Err(bundle_error(
                    code,
                    category,
                    format!("PoTR receipt validation failed: {error}"),
                    "Regenerate the PoTR receipt from the canonical timed retrieval observation and signer material.",
                    context,
                    vec![ValidationInputV1::new(
                        payload.kind.input_kind(),
                        payload.label.clone(),
                    )],
                    generated_at,
                ));
            }
            links.observe_manifest(
                receipt.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                receipt.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::RepairEvidence => {
            let evidence = decode_repair_bundle_payload::<RepairEvidenceV1>(payload, generated_at)?;
            validate_repair_bundle_value(evidence.validate(), &evidence, payload, generated_at)?;
            links.observe_manifest(
                evidence.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                evidence.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::RepairReport => {
            let report = decode_repair_bundle_payload::<RepairReportV1>(payload, generated_at)?;
            validate_repair_bundle_value(report.validate(), &report, payload, generated_at)?;
            links.observe_manifest(
                report.evidence.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                report.evidence.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::RepairTaskRecord => {
            let task = decode_repair_bundle_payload::<RepairTaskRecordV1>(payload, generated_at)?;
            validate_repair_bundle_value(task.validate(), &task, payload, generated_at)?;
            links.observe_manifest(
                task.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                task.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::RepairSlashProposal => {
            let proposal =
                decode_repair_bundle_payload::<RepairSlashProposalV1>(payload, generated_at)?;
            validate_repair_bundle_value(proposal.validate(), &proposal, payload, generated_at)?;
            links.observe_manifest(
                proposal.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                proposal.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::RepairTaskEvent => {
            let event = decode_repair_bundle_payload::<RepairTaskEventV1>(payload, generated_at)?;
            validate_repair_bundle_value(event.validate(), &event, payload, generated_at)?;
            links.observe_manifest(
                event.manifest_digest,
                &payload.label,
                vec![ValidationInputV1::new(
                    payload.kind.input_kind(),
                    payload.label.clone(),
                )],
                generated_at,
            )?;
            links.observe_provider(
                payload.kind.input_kind(),
                &payload.label,
                event.provider_id,
                true,
            );
        }
        FixtureBundlePayloadKindV1::OrderbookOrderRequest => validate_orderbook_bundle_payload(
            OrderbookValidationPayloadKindV1::OrderRequest,
            payload,
            generated_at,
        )?,
        FixtureBundlePayloadKindV1::OrderbookOrderCancel => validate_orderbook_bundle_payload(
            OrderbookValidationPayloadKindV1::OrderCancel,
            payload,
            generated_at,
        )?,
        FixtureBundlePayloadKindV1::OrderbookTradeEvent => validate_orderbook_bundle_payload(
            OrderbookValidationPayloadKindV1::TradeEvent,
            payload,
            generated_at,
        )?,
        FixtureBundlePayloadKindV1::OrderbookSettlementChannel => {
            validate_orderbook_bundle_payload(
                OrderbookValidationPayloadKindV1::SettlementChannel,
                payload,
                generated_at,
            )?
        }
        FixtureBundlePayloadKindV1::OrderbookSettlementReceipt => {
            validate_orderbook_bundle_payload(
                OrderbookValidationPayloadKindV1::SettlementReceipt,
                payload,
                generated_at,
            )?
        }
    }
    Ok(())
}

fn validate_orderbook_bundle_payload(
    kind: OrderbookValidationPayloadKindV1,
    payload: &FixtureBundlePayloadV1<'_>,
    generated_at: u64,
) -> Result<(), ValidationOutcomeV1> {
    let outcome =
        validate_orderbook_payload_bytes(kind, payload.bytes, payload.label.clone(), generated_at);
    if outcome.is_ok() {
        Ok(())
    } else {
        Err(outcome)
    }
}

fn validate_repair_bundle_value<T>(
    result: Result<(), RepairValidationError>,
    value: &T,
    payload: &FixtureBundlePayloadV1<'_>,
    generated_at: u64,
) -> Result<(), ValidationOutcomeV1>
where
    T: std::fmt::Debug,
{
    if let Err(error) = result {
        let code = repair_validation_code(&error);
        let category = repair_validation_category(&error);
        return Err(bundle_error(
            code,
            category,
            format!("repair payload validation failed: {error}"),
            "Regenerate the repair payload from canonical scheduler or auditor state.",
            vec![
                ValidationContextFieldV1::new("schema", payload.kind.schema()),
                ValidationContextFieldV1::new("validation_error", error.to_string()),
                ValidationContextFieldV1::new("payload", format!("{value:?}")),
            ],
            vec![ValidationInputV1::new(
                payload.kind.input_kind(),
                payload.label.clone(),
            )],
            generated_at,
        ));
    }
    Ok(())
}

fn decode_bundle_payload<T>(
    payload: &FixtureBundlePayloadV1<'_>,
    generated_at: u64,
) -> Result<T, ValidationOutcomeV1>
where
    T: for<'decode> norito::NoritoDeserialize<'decode>,
{
    norito::decode_from_bytes::<T>(payload.bytes).map_err(|error| {
        bundle_decode_error(
            payload.kind,
            &payload.label,
            error.to_string(),
            generated_at,
        )
    })
}

fn decode_repair_bundle_payload<T>(
    payload: &FixtureBundlePayloadV1<'_>,
    generated_at: u64,
) -> Result<T, ValidationOutcomeV1>
where
    T: for<'decode> norito::NoritoDeserialize<'decode>,
{
    decode_repair_archive_payload::<T>(payload.bytes).map_err(|error| {
        bundle_decode_error(
            payload.kind,
            &payload.label,
            error.to_string(),
            generated_at,
        )
    })
}

fn bundle_decode_error(
    kind: FixtureBundlePayloadKindV1,
    label: &str,
    error: String,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    bundle_error(
        "SFS-NORITO-001",
        CATEGORY_NORITO,
        format!("failed to decode {} Norito payload: {error}", kind.schema()),
        "Re-encode the fixture payload with the canonical SoraFS Norito schema.",
        vec![ValidationContextFieldV1::new("schema", kind.schema())],
        vec![ValidationInputV1::new(kind.input_kind(), label.to_owned())],
        generated_at,
    )
}

fn bundle_error(
    code: impl Into<String>,
    category: impl Into<String>,
    message: impl Into<String>,
    action: impl Into<String>,
    context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let code = code.into();
    let mut tags = vec!["sorafs.reference.bundle".to_owned()];
    tags.push(format!("sorafs.reference.code.{code}"));
    ValidationOutcomeV1::error(
        code,
        category,
        message,
        action,
        tags,
        context,
        inputs,
        generated_at,
    )
}

fn remap_bundle_payload_error(
    outcome: ValidationOutcomeV1,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    if outcome.code == "SFS-BND-002" || outcome.code == "SFS-BND-003" {
        return outcome;
    }
    let mut context = outcome.context;
    context.push(ValidationContextFieldV1::new(
        "payload_code",
        outcome.code.clone(),
    ));
    context.push(ValidationContextFieldV1::new(
        "payload_message",
        outcome.message,
    ));
    bundle_error(
        "SFS-BND-001",
        outcome.category,
        "fixture bundle payload validation failed",
        "Fix the invalid payload named in the bundle outcome before rerunning cross-link validation.",
        context,
        inputs,
        generated_at,
    )
}

fn provider_mismatch_error(
    reason: &str,
    expected_provider: [u8; 32],
    expected_source: &str,
    actual: &FixtureBundleProviderLink,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    bundle_error(
        "SFS-BND-003",
        CATEGORY_VALIDATION,
        "fixture bundle provider assignment mismatch",
        "Regenerate the bundle so provider-bearing artifacts either share the same provider or appear in the replication order assignments.",
        vec![
            ValidationContextFieldV1::new("provider_mismatch_reason", reason),
            ValidationContextFieldV1::new(
                "expected_provider_id_hex",
                hex::encode(expected_provider),
            ),
            ValidationContextFieldV1::new("expected_provider_source", expected_source),
            ValidationContextFieldV1::new(
                "actual_provider_id_hex",
                hex::encode(actual.provider_id),
            ),
            ValidationContextFieldV1::new("actual_provider_source", actual.label.clone()),
            ValidationContextFieldV1::new("actual_provider_kind", actual.kind),
        ],
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`GovernanceLogNodeV1`] and emits a reference outcome.
#[must_use]
pub fn validate_governance_log_node_bytes(
    bytes: &[u8],
    label: impl Into<String>,
    expected_node_cid: Option<&[u8]>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let label = label.into();
    let inputs = vec![ValidationInputV1::new("governance_log_node", label.clone())];
    let node = match norito::decode_from_bytes::<GovernanceLogNodeV1>(bytes) {
        Ok(node) => node,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode governance log node Norito payload: {error}"),
                "Re-encode the governance log node with the canonical SoraFS Norito schema.",
                vec![
                    "sorafs.reference.governance".to_owned(),
                    "sorafs.reference.code.SFS-NORITO-001".to_owned(),
                ],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "GovernanceLogNodeV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = governance_log_node_context(&node);
    if let Some(expected_node_cid) = expected_node_cid
        && node.node_cid.as_slice() != expected_node_cid
    {
        context.push(ValidationContextFieldV1::new(
            "expected_node_cid",
            String::from_utf8_lossy(expected_node_cid).to_string(),
        ));
        return ValidationOutcomeV1::error(
            "SFS-GOV-003",
            CATEGORY_VALIDATION,
            "governance log node CID does not match the expected CID",
            "Pass the node CID that belongs to this governance log node or regenerate the fixture from the canonical node payload.",
            vec![
                "sorafs.reference.governance".to_owned(),
                "sorafs.reference.code.SFS-GOV-003".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Err(error) = node.validate() {
        let code = governance_log_validation_code(&error);
        let category = governance_log_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("governance log node validation failed: {error}"),
            "Regenerate the governance log node from canonical payload bytes, publisher metadata, and governed signature material.",
            vec![
                "sorafs.reference.governance".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Err(error) = node.verify_publisher_signature() {
        context.push(ValidationContextFieldV1::new(
            "signature_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            governance_signature_verification_code(&error),
            governance_signature_verification_category(&error),
            format!("governance log publisher signature validation failed: {error}"),
            "Resign the governance log node with the governed publisher key and canonical node signing bytes.",
            vec![
                "sorafs.reference.governance".to_owned(),
                format!(
                    "sorafs.reference.code.{}",
                    governance_signature_verification_code(&error)
                ),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let GovernanceLogPayloadV1::ProviderAdvert(advert) = &node.payload
        && let Err(error) = advert.verify_signature()
    {
        context.push(ValidationContextFieldV1::new(
            "signature_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            "SFS-SIG-001",
            CATEGORY_SIGNATURE,
            format!("governance provider advert signature validation failed: {error}"),
            "Resign the embedded advert body with the governed provider Ed25519 key.",
            vec![
                "sorafs.reference.governance".to_owned(),
                "sorafs.reference.code.SFS-SIG-001".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "governance log node accepted",
        vec![
            "sorafs.reference.governance".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`GovernanceDagBlockV1`] and emits a reference outcome.
#[must_use]
pub fn validate_governance_dag_block_bytes(
    bytes: &[u8],
    label: impl Into<String>,
    expected_block_cid: Option<&[u8]>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let label = label.into();
    let inputs = vec![ValidationInputV1::new(
        "governance_dag_block",
        label.clone(),
    )];
    let block = match norito::decode_from_bytes::<GovernanceDagBlockV1>(bytes) {
        Ok(block) => block,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode governance DAG block Norito payload: {error}"),
                "Re-encode the governance DAG block with the canonical SoraFS Norito schema.",
                vec![
                    "sorafs.reference.governance_dag.block".to_owned(),
                    "sorafs.reference.code.SFS-NORITO-001".to_owned(),
                ],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "GovernanceDagBlockV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = governance_dag_block_context(&block);
    if let Some(expected_block_cid) = expected_block_cid
        && block.block_cid.as_slice() != expected_block_cid
    {
        context.push(ValidationContextFieldV1::new(
            "expected_block_cid",
            String::from_utf8_lossy(expected_block_cid).to_string(),
        ));
        context.push(ValidationContextFieldV1::new(
            "expected_block_cid_hex",
            hex::encode(expected_block_cid),
        ));
        return ValidationOutcomeV1::error(
            "SFS-GOV-004",
            CATEGORY_VALIDATION,
            "governance DAG block CID does not match the expected CID",
            "Pass the block CID that belongs to this governance DAG block or regenerate the block from canonical payload bytes.",
            vec![
                "sorafs.reference.governance_dag.block".to_owned(),
                "sorafs.reference.code.SFS-GOV-004".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Err(error) = block.validate() {
        let code = governance_dag_block_validation_code(&error);
        let category = governance_dag_block_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("governance DAG block validation failed: {error}"),
            "Regenerate the governance DAG block from canonical log-node bytes, parent linkage, publisher metadata, and governed signature material.",
            vec![
                "sorafs.reference.governance_dag.block".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "governance DAG block accepted",
        vec![
            "sorafs.reference.governance_dag.block".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a signed [`GovernanceDagHeadV1`] against Norito-encoded blocks.
#[must_use]
pub fn validate_governance_dag_head_chain_bytes(
    head_bytes: &[u8],
    head_label: impl Into<String>,
    block_payloads: &[(&[u8], String)],
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let head_label = head_label.into();
    let mut inputs = vec![ValidationInputV1::new(
        "governance_dag_head",
        head_label.clone(),
    )];
    for (_, label) in block_payloads {
        inputs.push(ValidationInputV1::new(
            "governance_dag_block",
            label.clone(),
        ));
    }

    let head = match norito::decode_from_bytes::<GovernanceDagHeadV1>(head_bytes) {
        Ok(head) => head,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode governance DAG head Norito payload: {error}"),
                "Re-encode the governance DAG head with the canonical SoraFS Norito schema.",
                vec![
                    "sorafs.reference.governance_dag.head".to_owned(),
                    "sorafs.reference.code.SFS-NORITO-001".to_owned(),
                ],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "GovernanceDagHeadV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut blocks = Vec::with_capacity(block_payloads.len());
    for (bytes, label) in block_payloads {
        match norito::decode_from_bytes::<GovernanceDagBlockV1>(bytes) {
            Ok(block) => blocks.push(block),
            Err(error) => {
                return ValidationOutcomeV1::error(
                    "SFS-NORITO-001",
                    CATEGORY_NORITO,
                    format!(
                        "failed to decode governance DAG block Norito payload `{label}`: {error}"
                    ),
                    "Re-encode every governance DAG block with the canonical SoraFS Norito schema before validating the head chain.",
                    vec![
                        "sorafs.reference.governance_dag.head".to_owned(),
                        "sorafs.reference.code.SFS-NORITO-001".to_owned(),
                    ],
                    vec![
                        ValidationContextFieldV1::new("schema", "GovernanceDagBlockV1"),
                        ValidationContextFieldV1::new("block_label", label.clone()),
                    ],
                    inputs,
                    generated_at,
                );
            }
        }
    }

    let mut context = governance_dag_head_context(&head);
    context.push(ValidationContextFieldV1::new(
        "block_payload_count",
        blocks.len().to_string(),
    ));
    for (index, block) in blocks.iter().enumerate() {
        context.push(ValidationContextFieldV1::new(
            format!("block_{index}_cid_hex"),
            hex::encode(&block.block_cid),
        ));
    }

    if let Err(error) = validate_governance_dag_head_against_chain_v1(&head, &blocks) {
        let code = governance_dag_head_chain_validation_code(&error);
        let category = governance_dag_head_chain_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("governance DAG head-chain validation failed: {error}"),
            "Regenerate the governance DAG blocks and signed head manifest from one canonical parent-linked chain.",
            vec![
                "sorafs.reference.governance_dag.head".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "governance DAG head accepted",
        vec![
            "sorafs.reference.governance_dag.head".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn governance_log_node_context(node: &GovernanceLogNodeV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "GovernanceLogNodeV1"),
        ValidationContextFieldV1::new("version", node.version.to_string()),
        ValidationContextFieldV1::new(
            "node_cid",
            String::from_utf8_lossy(&node.node_cid).to_string(),
        ),
        ValidationContextFieldV1::new("node_cid_hex", hex::encode(&node.node_cid)),
        ValidationContextFieldV1::new("timestamp", node.timestamp.to_string()),
        ValidationContextFieldV1::new(
            "publisher_peer_id",
            String::from_utf8_lossy(&node.publisher_peer_id).to_string(),
        ),
        ValidationContextFieldV1::new("payload_kind", governance_payload_kind(&node.payload)),
        ValidationContextFieldV1::new(
            "publisher_signature_algorithm",
            governance_signature_algorithm_label(node.publisher_signature.algorithm),
        ),
        ValidationContextFieldV1::new(
            "publisher_public_key_len",
            node.publisher_signature.public_key.len().to_string(),
        ),
        ValidationContextFieldV1::new(
            "publisher_signature_len",
            node.publisher_signature.signature.len().to_string(),
        ),
    ];
    if let Some(prev_cid) = &node.prev_cid {
        context.push(ValidationContextFieldV1::new(
            "prev_cid",
            String::from_utf8_lossy(prev_cid).to_string(),
        ));
        context.push(ValidationContextFieldV1::new(
            "prev_cid_hex",
            hex::encode(prev_cid),
        ));
    }
    context
}

fn governance_dag_block_context(block: &GovernanceDagBlockV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "GovernanceDagBlockV1"),
        ValidationContextFieldV1::new("version", block.version.to_string()),
        ValidationContextFieldV1::new("block_cid_hex", hex::encode(&block.block_cid)),
        ValidationContextFieldV1::new("sequence", block.sequence.to_string()),
        ValidationContextFieldV1::new("timestamp", block.timestamp.to_string()),
        ValidationContextFieldV1::new(
            "publisher_peer_id",
            String::from_utf8_lossy(&block.publisher_peer_id).to_string(),
        ),
        ValidationContextFieldV1::new("node_cid_hex", hex::encode(&block.node.node_cid)),
        ValidationContextFieldV1::new(
            "node_payload_kind",
            governance_payload_kind(&block.node.payload),
        ),
        ValidationContextFieldV1::new(
            "block_signature_algorithm",
            governance_signature_algorithm_label(block.block_signature.algorithm),
        ),
        ValidationContextFieldV1::new(
            "block_public_key_len",
            block.block_signature.public_key.len().to_string(),
        ),
        ValidationContextFieldV1::new(
            "block_signature_len",
            block.block_signature.signature.len().to_string(),
        ),
    ];
    if let Some(prev) = &block.prev_block_cid {
        context.push(ValidationContextFieldV1::new(
            "prev_block_cid_hex",
            hex::encode(prev),
        ));
    }
    context
}

fn governance_dag_head_context(head: &GovernanceDagHeadV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "GovernanceDagHeadV1"),
        ValidationContextFieldV1::new("version", head.version.to_string()),
        ValidationContextFieldV1::new("head_block_cid_hex", hex::encode(&head.head_block_cid)),
        ValidationContextFieldV1::new("block_count", head.block_count.to_string()),
        ValidationContextFieldV1::new("generated_at", head.generated_at.to_string()),
        ValidationContextFieldV1::new(
            "publisher_peer_id",
            String::from_utf8_lossy(&head.publisher_peer_id).to_string(),
        ),
        ValidationContextFieldV1::new(
            "head_signature_algorithm",
            governance_signature_algorithm_label(head.head_signature.algorithm),
        ),
        ValidationContextFieldV1::new(
            "head_public_key_len",
            head.head_signature.public_key.len().to_string(),
        ),
        ValidationContextFieldV1::new(
            "head_signature_len",
            head.head_signature.signature.len().to_string(),
        ),
    ];
    if let Some(checkpoint) = &head.checkpoint_cid {
        context.push(ValidationContextFieldV1::new(
            "checkpoint_cid_hex",
            hex::encode(checkpoint),
        ));
    }
    context
}

fn order_request_context(order: &OrderRequestV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "OrderRequestV1"),
        ValidationContextFieldV1::new("version", order.version.to_string()),
        ValidationContextFieldV1::new("order_id_hex", hex::encode(order.order_id)),
        ValidationContextFieldV1::new("side", order_side_label(order.side)),
        ValidationContextFieldV1::new("tier", order_tier_label(order.tier)),
        ValidationContextFieldV1::new(
            "price_per_gib_micro_xor",
            order.price_per_gib.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new("quantity_gib", order.quantity_gib.to_string()),
        ValidationContextFieldV1::new("remaining_gib", order.remaining_gib.to_string()),
        ValidationContextFieldV1::new("owner_account_len", order.owner_account.len().to_string()),
        ValidationContextFieldV1::new("expiry_unix", order.expiry_unix.to_string()),
        ValidationContextFieldV1::new("nonce", order.nonce.to_string()),
        ValidationContextFieldV1::new("maker_fee_bps", order.maker_fee_bps.to_string()),
        ValidationContextFieldV1::new("taker_fee_bps", order.taker_fee_bps.to_string()),
    ];
    append_orderbook_signature_context(&mut context, &order.signature);
    context
}

fn order_cancel_context(cancel: &OrderCancelV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "OrderCancelV1"),
        ValidationContextFieldV1::new("version", cancel.version.to_string()),
        ValidationContextFieldV1::new("order_id_hex", hex::encode(cancel.order_id)),
        ValidationContextFieldV1::new("owner_account_len", cancel.owner_account.len().to_string()),
        ValidationContextFieldV1::new("reason", order_cancel_reason_label(cancel.reason)),
        ValidationContextFieldV1::new("nonce", cancel.nonce.to_string()),
    ];
    append_orderbook_signature_context(&mut context, &cancel.signature);
    context
}

fn trade_event_context(trade: &TradeEventV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("schema", "TradeEventV1"),
        ValidationContextFieldV1::new("version", trade.version.to_string()),
        ValidationContextFieldV1::new("trade_id_hex", hex::encode(trade.trade_id)),
        ValidationContextFieldV1::new("maker_order_id_hex", hex::encode(trade.maker_order_id)),
        ValidationContextFieldV1::new("taker_order_id_hex", hex::encode(trade.taker_order_id)),
        ValidationContextFieldV1::new("tier", order_tier_label(trade.tier)),
        ValidationContextFieldV1::new(
            "price_per_gib_micro_xor",
            trade.price_per_gib.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new("filled_gib", trade.filled_gib.to_string()),
        ValidationContextFieldV1::new(
            "maker_fee_micro_xor",
            trade.maker_fee.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new(
            "taker_fee_micro_xor",
            trade.taker_fee.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new("timestamp_unix", trade.timestamp_unix.to_string()),
    ]
}

fn settlement_channel_context(channel: &SettlementChannelV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("schema", "SettlementChannelV1"),
        ValidationContextFieldV1::new("version", channel.version.to_string()),
        ValidationContextFieldV1::new("channel_id_hex", hex::encode(channel.channel_id)),
        ValidationContextFieldV1::new("trade_id_hex", hex::encode(channel.trade_id)),
        ValidationContextFieldV1::new("buyer_account_len", channel.buyer_account.len().to_string()),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(channel.provider_id)),
        ValidationContextFieldV1::new("total_bytes", channel.total_bytes.to_string()),
        ValidationContextFieldV1::new("remaining_bytes", channel.remaining_bytes.to_string()),
        ValidationContextFieldV1::new(
            "xor_locked_micro_xor",
            channel.xor_locked.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new("status", settlement_channel_status_label(channel.status)),
        ValidationContextFieldV1::new("opened_at_unix", channel.opened_at_unix.to_string()),
        ValidationContextFieldV1::new("updated_at_unix", channel.updated_at_unix.to_string()),
    ]
}

fn settlement_receipt_context(receipt: &SettlementReceiptV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("schema", "SettlementReceiptV1"),
        ValidationContextFieldV1::new("version", receipt.version.to_string()),
        ValidationContextFieldV1::new("receipt_id_hex", hex::encode(receipt.receipt_id)),
        ValidationContextFieldV1::new("channel_id_hex", hex::encode(receipt.channel_id)),
        ValidationContextFieldV1::new("trade_id_hex", hex::encode(receipt.trade_id)),
        ValidationContextFieldV1::new("range_start", receipt.range.start.to_string()),
        ValidationContextFieldV1::new("range_end", receipt.range.end.to_string()),
        ValidationContextFieldV1::new("chunk_hash_hex", hex::encode(receipt.chunk_hash)),
        ValidationContextFieldV1::new("bytes_delivered", receipt.bytes_delivered.to_string()),
        ValidationContextFieldV1::new(
            "xor_debited_micro_xor",
            receipt.xor_debited.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new(
            "provider_credit_micro_xor",
            receipt.provider_credit.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new(
            "fee_amount_micro_xor",
            receipt.fee_amount.as_micro().to_string(),
        ),
        ValidationContextFieldV1::new("issued_at_unix", receipt.issued_at_unix.to_string()),
    ];
    append_orderbook_signature_context(&mut context, &receipt.settlement_signature);
    context
}

fn append_orderbook_signature_context(
    context: &mut Vec<ValidationContextFieldV1>,
    signature: &crate::OrderbookSignatureV1,
) {
    context.push(ValidationContextFieldV1::new(
        "signature_algorithm",
        signature_algorithm_label(signature.algorithm),
    ));
    context.push(ValidationContextFieldV1::new(
        "signature_public_key_len",
        signature.public_key.len().to_string(),
    ));
    context.push(ValidationContextFieldV1::new(
        "signature_len",
        signature.signature.len().to_string(),
    ));
}

fn order_side_label(side: OrderSideV1) -> &'static str {
    match side {
        OrderSideV1::Bid => "bid",
        OrderSideV1::Ask => "ask",
    }
}

fn order_tier_label(tier: OrderTierV1) -> &'static str {
    match tier {
        OrderTierV1::Hot => "hot",
        OrderTierV1::Warm => "warm",
        OrderTierV1::Archive => "archive",
    }
}

fn order_cancel_reason_label(reason: OrderCancelReasonV1) -> &'static str {
    match reason {
        OrderCancelReasonV1::OwnerRequested => "owner_requested",
        OrderCancelReasonV1::Expired => "expired",
        OrderCancelReasonV1::Governance => "governance",
        OrderCancelReasonV1::Replaced => "replaced",
    }
}

fn settlement_channel_status_label(status: SettlementChannelStatusV1) -> &'static str {
    match status {
        SettlementChannelStatusV1::Open => "open",
        SettlementChannelStatusV1::Closing => "closing",
        SettlementChannelStatusV1::Closed => "closed",
        SettlementChannelStatusV1::Breached => "breached",
        SettlementChannelStatusV1::Refunded => "refunded",
    }
}

fn governance_payload_kind(payload: &GovernanceLogPayloadV1) -> &'static str {
    match payload {
        GovernanceLogPayloadV1::ProviderAdvert(_) => "provider_advert",
        GovernanceLogPayloadV1::ReplicationOrder(_) => "replication_order",
        GovernanceLogPayloadV1::PorChallenge(_) => "por_challenge",
        GovernanceLogPayloadV1::PorProof(_) => "por_proof",
        GovernanceLogPayloadV1::AuditVerdict(_) => "audit_verdict",
        GovernanceLogPayloadV1::DealSettlement(_) => "deal_settlement",
        GovernanceLogPayloadV1::ReputationSnapshot(_) => "reputation_snapshot",
    }
}

fn governance_signature_algorithm_label(algorithm: GovernanceSignatureAlgorithm) -> &'static str {
    match algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => "ed25519",
        GovernanceSignatureAlgorithm::Dilithium3 => "dilithium3",
    }
}

fn governance_log_validation_code(error: &GovernanceLogValidationError) -> &'static str {
    match error {
        GovernanceLogValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        GovernanceLogValidationError::InvalidSignature => "SFS-SIG-005",
        GovernanceLogValidationError::Advert(error) => advert_validation_code(error),
        GovernanceLogValidationError::ReplicationOrder(error) => {
            replication_order_validation_code(error)
        }
        GovernanceLogValidationError::PorChallenge(error) => por_challenge_validation_code(error),
        GovernanceLogValidationError::PorProof(error) => por_proof_validation_code(error),
        GovernanceLogValidationError::AuditVerdict(_)
        | GovernanceLogValidationError::DealSettlement(_)
        | GovernanceLogValidationError::ReputationSnapshot(_)
        | GovernanceLogValidationError::MissingNodeCid
        | GovernanceLogValidationError::InvalidPrevCid
        | GovernanceLogValidationError::MissingPublisherPeerId => "SFS-GOV-001",
    }
}

fn governance_log_validation_category(error: &GovernanceLogValidationError) -> &'static str {
    match error {
        GovernanceLogValidationError::UnsupportedVersion { .. } => CATEGORY_VALIDATION,
        GovernanceLogValidationError::InvalidSignature => CATEGORY_SIGNATURE,
        GovernanceLogValidationError::Advert(error) => advert_validation_category(error),
        GovernanceLogValidationError::ReplicationOrder(error) => {
            replication_order_validation_category(error)
        }
        GovernanceLogValidationError::PorChallenge(error) => {
            por_challenge_validation_category(error)
        }
        GovernanceLogValidationError::PorProof(_) => CATEGORY_VALIDATION,
        GovernanceLogValidationError::AuditVerdict(_)
        | GovernanceLogValidationError::DealSettlement(_)
        | GovernanceLogValidationError::ReputationSnapshot(_)
        | GovernanceLogValidationError::MissingNodeCid
        | GovernanceLogValidationError::InvalidPrevCid
        | GovernanceLogValidationError::MissingPublisherPeerId => CATEGORY_VALIDATION,
    }
}

fn governance_signature_verification_code(
    error: &GovernanceLogSignatureVerificationError,
) -> &'static str {
    match error {
        GovernanceLogSignatureVerificationError::PayloadEncoding { .. } => "SFS-INT-001",
        GovernanceLogSignatureVerificationError::UnsupportedAlgorithm(_)
        | GovernanceLogSignatureVerificationError::InvalidPublicKeyLength { .. }
        | GovernanceLogSignatureVerificationError::InvalidSignatureLength { .. }
        | GovernanceLogSignatureVerificationError::InvalidPublicKey { .. }
        | GovernanceLogSignatureVerificationError::Verification { .. } => "SFS-SIG-005",
    }
}

fn governance_signature_verification_category(
    error: &GovernanceLogSignatureVerificationError,
) -> &'static str {
    match error {
        GovernanceLogSignatureVerificationError::PayloadEncoding { .. } => CATEGORY_INTERNAL,
        _ => CATEGORY_SIGNATURE,
    }
}

fn governance_dag_block_validation_code(error: &GovernanceDagBlockValidationError) -> &'static str {
    match error {
        GovernanceDagBlockValidationError::UnsupportedVersion { .. }
        | GovernanceDagBlockValidationError::MissingBlockCid
        | GovernanceDagBlockValidationError::InvalidPrevBlockCid
        | GovernanceDagBlockValidationError::RootHasParent
        | GovernanceDagBlockValidationError::NonRootMissingParent
        | GovernanceDagBlockValidationError::MissingPublisherPeerId
        | GovernanceDagBlockValidationError::Node(_) => "SFS-GOV-005",
        GovernanceDagBlockValidationError::InvalidSignature
        | GovernanceDagBlockValidationError::NodeSignature(_)
        | GovernanceDagBlockValidationError::BlockSignature(_) => "SFS-SIG-006",
        GovernanceDagBlockValidationError::CidEncoding { .. } => "SFS-INT-001",
        GovernanceDagBlockValidationError::InvalidBlockCid => "SFS-GOV-004",
    }
}

fn governance_dag_block_validation_category(
    error: &GovernanceDagBlockValidationError,
) -> &'static str {
    match error {
        GovernanceDagBlockValidationError::InvalidSignature
        | GovernanceDagBlockValidationError::NodeSignature(_)
        | GovernanceDagBlockValidationError::BlockSignature(_) => CATEGORY_SIGNATURE,
        GovernanceDagBlockValidationError::CidEncoding { .. } => CATEGORY_INTERNAL,
        _ => CATEGORY_VALIDATION,
    }
}

fn governance_dag_head_validation_code(error: &GovernanceDagHeadValidationError) -> &'static str {
    match error {
        GovernanceDagHeadValidationError::UnsupportedVersion { .. }
        | GovernanceDagHeadValidationError::MissingHeadBlockCid
        | GovernanceDagHeadValidationError::EmptyBlockCount
        | GovernanceDagHeadValidationError::MissingPublisherPeerId
        | GovernanceDagHeadValidationError::InvalidCheckpointCid => "SFS-GOV-007",
        GovernanceDagHeadValidationError::InvalidSignature
        | GovernanceDagHeadValidationError::HeadSignature(_) => "SFS-SIG-007",
    }
}

fn governance_dag_head_validation_category(
    error: &GovernanceDagHeadValidationError,
) -> &'static str {
    match error {
        GovernanceDagHeadValidationError::InvalidSignature
        | GovernanceDagHeadValidationError::HeadSignature(_) => CATEGORY_SIGNATURE,
        _ => CATEGORY_VALIDATION,
    }
}

fn governance_dag_chain_validation_code(error: &GovernanceDagChainValidationError) -> &'static str {
    match error {
        GovernanceDagChainValidationError::InvalidBlock { source, .. } => {
            governance_dag_block_validation_code(source)
        }
        GovernanceDagChainValidationError::Empty
        | GovernanceDagChainValidationError::DuplicateBlockCid { .. }
        | GovernanceDagChainValidationError::MissingParent { .. }
        | GovernanceDagChainValidationError::SequenceGap { .. }
        | GovernanceDagChainValidationError::TimestampRegression { .. }
        | GovernanceDagChainValidationError::HeadCount { .. }
        | GovernanceDagChainValidationError::ExpectedHeadMismatch => "SFS-GOV-006",
    }
}

fn governance_dag_chain_validation_category(
    error: &GovernanceDagChainValidationError,
) -> &'static str {
    match error {
        GovernanceDagChainValidationError::InvalidBlock { source, .. } => {
            governance_dag_block_validation_category(source)
        }
        _ => CATEGORY_VALIDATION,
    }
}

fn governance_dag_head_chain_validation_code(
    error: &GovernanceDagHeadChainValidationError,
) -> &'static str {
    match error {
        GovernanceDagHeadChainValidationError::Head(error) => {
            governance_dag_head_validation_code(error)
        }
        GovernanceDagHeadChainValidationError::Chain(error) => {
            governance_dag_chain_validation_code(error)
        }
        GovernanceDagHeadChainValidationError::BlockCountMismatch { .. } => "SFS-GOV-008",
    }
}

fn governance_dag_head_chain_validation_category(
    error: &GovernanceDagHeadChainValidationError,
) -> &'static str {
    match error {
        GovernanceDagHeadChainValidationError::Head(error) => {
            governance_dag_head_validation_category(error)
        }
        GovernanceDagHeadChainValidationError::Chain(error) => {
            governance_dag_chain_validation_category(error)
        }
        GovernanceDagHeadChainValidationError::BlockCountMismatch { .. } => CATEGORY_VALIDATION,
    }
}

/// Repair payload kinds supported by the reference validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairValidationPayloadKindV1 {
    /// [`RepairEvidenceV1`] payload.
    Evidence,
    /// [`RepairReportV1`] payload.
    Report,
    /// [`RepairTaskRecordV1`] payload.
    TaskRecord,
    /// [`RepairSlashProposalV1`] payload.
    SlashProposal,
    /// [`RepairEscalationPolicyV1`] payload.
    EscalationPolicy,
    /// [`RepairEscalationApprovalV1`] payload.
    EscalationApproval,
    /// [`SignedAuditorRequestV1`] payload with signature verification.
    SignedAuditorRequest,
    /// [`RepairWorkerSignaturePayloadV1`] payload.
    WorkerSignaturePayload,
    /// [`RepairTaskEventV1`] payload.
    TaskEvent,
    /// [`RepairAuditEventV1`] payload.
    AuditEvent,
}

impl RepairValidationPayloadKindV1 {
    fn input_kind(self) -> &'static str {
        match self {
            Self::Evidence => "repair_evidence",
            Self::Report => "repair_report",
            Self::TaskRecord => "repair_task_record",
            Self::SlashProposal => "repair_slash_proposal",
            Self::EscalationPolicy => "repair_escalation_policy",
            Self::EscalationApproval => "repair_escalation_approval",
            Self::SignedAuditorRequest => "signed_auditor_request",
            Self::WorkerSignaturePayload => "repair_worker_signature_payload",
            Self::TaskEvent => "repair_task_event",
            Self::AuditEvent => "repair_audit_event",
        }
    }

    fn schema(self) -> &'static str {
        match self {
            Self::Evidence => "RepairEvidenceV1",
            Self::Report => "RepairReportV1",
            Self::TaskRecord => "RepairTaskRecordV1",
            Self::SlashProposal => "RepairSlashProposalV1",
            Self::EscalationPolicy => "RepairEscalationPolicyV1",
            Self::EscalationApproval => "RepairEscalationApprovalV1",
            Self::SignedAuditorRequest => "SignedAuditorRequestV1",
            Self::WorkerSignaturePayload => "RepairWorkerSignaturePayloadV1",
            Self::TaskEvent => "RepairTaskEventV1",
            Self::AuditEvent => "RepairAuditEventV1",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::Evidence => "repair evidence accepted",
            Self::Report => "repair report accepted",
            Self::TaskRecord => "repair task record accepted",
            Self::SlashProposal => "repair slash proposal accepted",
            Self::EscalationPolicy => "repair escalation policy accepted",
            Self::EscalationApproval => "repair escalation approval accepted",
            Self::SignedAuditorRequest => "signed auditor request accepted",
            Self::WorkerSignaturePayload => "repair worker signature payload accepted",
            Self::TaskEvent => "repair task event accepted",
            Self::AuditEvent => "repair audit event accepted",
        }
    }
}

/// Orderbook payload kinds supported by the reference validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderbookValidationPayloadKindV1 {
    /// [`OrderRequestV1`] payload.
    OrderRequest,
    /// [`OrderCancelV1`] payload.
    OrderCancel,
    /// [`TradeEventV1`] payload.
    TradeEvent,
    /// [`SettlementChannelV1`] payload.
    SettlementChannel,
    /// [`SettlementReceiptV1`] payload.
    SettlementReceipt,
}

impl OrderbookValidationPayloadKindV1 {
    fn input_kind(self) -> &'static str {
        match self {
            Self::OrderRequest => "orderbook_order_request",
            Self::OrderCancel => "orderbook_order_cancel",
            Self::TradeEvent => "orderbook_trade_event",
            Self::SettlementChannel => "settlement_channel",
            Self::SettlementReceipt => "settlement_receipt",
        }
    }

    fn schema(self) -> &'static str {
        match self {
            Self::OrderRequest => "OrderRequestV1",
            Self::OrderCancel => "OrderCancelV1",
            Self::TradeEvent => "TradeEventV1",
            Self::SettlementChannel => "SettlementChannelV1",
            Self::SettlementReceipt => "SettlementReceiptV1",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::OrderRequest => "order_request",
            Self::OrderCancel => "order_cancel",
            Self::TradeEvent => "trade_event",
            Self::SettlementChannel => "settlement_channel",
            Self::SettlementReceipt => "settlement_receipt",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::OrderRequest => "orderbook order request accepted",
            Self::OrderCancel => "orderbook cancel request accepted",
            Self::TradeEvent => "orderbook trade event accepted",
            Self::SettlementChannel => "settlement channel accepted",
            Self::SettlementReceipt => "settlement receipt accepted",
        }
    }
}

/// Validates a Norito-encoded orderbook payload and emits a reference outcome.
#[must_use]
pub fn validate_orderbook_payload_bytes(
    kind: OrderbookValidationPayloadKindV1,
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input_label = input_label.into();
    let inputs = vec![ValidationInputV1::new(
        kind.input_kind(),
        input_label.clone(),
    )];

    match kind {
        OrderbookValidationPayloadKindV1::OrderRequest => {
            match norito::decode_from_bytes::<OrderRequestV1>(bytes) {
                Ok(payload) => validate_orderbook_payload_value(
                    kind,
                    payload.validate(),
                    order_request_context(&payload),
                    inputs,
                    generated_at,
                ),
                Err(error) => orderbook_decode_error(kind, error.to_string(), inputs, generated_at),
            }
        }
        OrderbookValidationPayloadKindV1::OrderCancel => {
            match norito::decode_from_bytes::<OrderCancelV1>(bytes) {
                Ok(payload) => validate_orderbook_payload_value(
                    kind,
                    payload.validate(),
                    order_cancel_context(&payload),
                    inputs,
                    generated_at,
                ),
                Err(error) => orderbook_decode_error(kind, error.to_string(), inputs, generated_at),
            }
        }
        OrderbookValidationPayloadKindV1::TradeEvent => {
            match norito::decode_from_bytes::<TradeEventV1>(bytes) {
                Ok(payload) => validate_orderbook_payload_value(
                    kind,
                    payload.validate(),
                    trade_event_context(&payload),
                    inputs,
                    generated_at,
                ),
                Err(error) => orderbook_decode_error(kind, error.to_string(), inputs, generated_at),
            }
        }
        OrderbookValidationPayloadKindV1::SettlementChannel => {
            match norito::decode_from_bytes::<SettlementChannelV1>(bytes) {
                Ok(payload) => validate_orderbook_payload_value(
                    kind,
                    payload.validate(),
                    settlement_channel_context(&payload),
                    inputs,
                    generated_at,
                ),
                Err(error) => orderbook_decode_error(kind, error.to_string(), inputs, generated_at),
            }
        }
        OrderbookValidationPayloadKindV1::SettlementReceipt => {
            match norito::decode_from_bytes::<SettlementReceiptV1>(bytes) {
                Ok(payload) => validate_orderbook_payload_value(
                    kind,
                    payload.validate(),
                    settlement_receipt_context(&payload),
                    inputs,
                    generated_at,
                ),
                Err(error) => orderbook_decode_error(kind, error.to_string(), inputs, generated_at),
            }
        }
    }
}

fn validate_orderbook_payload_value(
    kind: OrderbookValidationPayloadKindV1,
    result: Result<(), OrderbookValidationError>,
    mut context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    if let Err(error) = result {
        let code = orderbook_validation_code(&error);
        let category = orderbook_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("{} validation failed: {error}", kind.schema()),
            "Regenerate the orderbook payload from canonical orderbook or settlement state and retry validation.",
            vec![
                "sorafs.reference.orderbook".to_owned(),
                format!("sorafs.reference.orderbook.{}", kind.tag()),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        kind.success_message(),
        vec![
            "sorafs.reference.orderbook".to_owned(),
            format!("sorafs.reference.orderbook.{}", kind.tag()),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn orderbook_decode_error(
    kind: OrderbookValidationPayloadKindV1,
    error: String,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ValidationOutcomeV1::error(
        "SFS-NORITO-001",
        CATEGORY_NORITO,
        format!("failed to decode {} Norito payload: {error}", kind.schema()),
        "Re-encode the orderbook payload with the canonical SoraFS Norito schema.",
        vec![
            "sorafs.reference.orderbook".to_owned(),
            format!("sorafs.reference.orderbook.{}", kind.tag()),
            "sorafs.reference.code.SFS-NORITO-001".to_owned(),
        ],
        vec![ValidationContextFieldV1::new("schema", kind.schema())],
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`ProviderAdvertV1`] and emits a reference outcome.
#[must_use]
pub fn validate_provider_advert_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    now: u64,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new("provider_advert", input_label);
    let inputs = vec![input];
    let advert = match norito::decode_from_bytes::<ProviderAdvertV1>(bytes) {
        Ok(advert) => advert,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ProviderAdvertV1 Norito payload: {error}"),
                "Re-encode the provider advert with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.advert".to_owned()],
                vec![ValidationContextFieldV1::new("schema", "ProviderAdvertV1")],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = advert_context(&advert);
    if let Err(error) = advert.validate_with_body(now) {
        let code = advert_validation_code(&error);
        let category = advert_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("provider advert validation failed: {error}"),
            "Regenerate the advert from governed provider metadata and retry validation.",
            vec![
                "sorafs.reference.advert".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Err(error) = advert.verify_signature() {
        context.push(ValidationContextFieldV1::new(
            "signature_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            "SFS-SIG-001",
            CATEGORY_SIGNATURE,
            format!("provider advert signature validation failed: {error}"),
            "Resign the advert body with the governed provider Ed25519 key.",
            vec![
                "sorafs.reference.advert".to_owned(),
                "sorafs.reference.code.SFS-SIG-001".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "provider advert accepted",
        vec![
            "sorafs.reference.advert".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded repair payload and emits a reference outcome.
#[must_use]
pub fn validate_repair_payload_bytes(
    kind: RepairValidationPayloadKindV1,
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new(kind.input_kind(), input_label);
    let inputs = vec![input];
    macro_rules! decode_repair_payload {
        ($payload_type:ty) => {
            match decode_repair_archive_payload::<$payload_type>(bytes) {
                Ok(payload) => payload,
                Err(error) => {
                    return ValidationOutcomeV1::error(
                        "SFS-NORITO-001",
                        CATEGORY_NORITO,
                        format!("failed to decode {} Norito payload: {error}", kind.schema()),
                        "Re-encode the repair payload with the canonical SoraFS Norito schema.",
                        vec!["sorafs.reference.repair".to_owned()],
                        vec![ValidationContextFieldV1::new("schema", kind.schema())],
                        inputs,
                        generated_at,
                    );
                }
            }
        };
    }

    let context = match kind {
        RepairValidationPayloadKindV1::Evidence => {
            let evidence = decode_repair_payload!(RepairEvidenceV1);
            let context = repair_evidence_context(&evidence);
            if let Err(error) = evidence.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::Report => {
            let report = decode_repair_payload!(RepairReportV1);
            let context = repair_report_context(&report);
            if let Err(error) = report.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::TaskRecord => {
            let task = decode_repair_payload!(RepairTaskRecordV1);
            let context = repair_task_context(&task);
            if let Err(error) = task.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::SlashProposal => {
            let proposal = decode_repair_payload!(RepairSlashProposalV1);
            let context = repair_slash_proposal_context(&proposal);
            if let Err(error) = proposal.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::EscalationPolicy => {
            let policy = decode_repair_payload!(RepairEscalationPolicyV1);
            let context = repair_escalation_policy_context(&policy);
            if let Err(error) = policy.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::EscalationApproval => {
            let approval = decode_repair_payload!(RepairEscalationApprovalV1);
            let context = repair_escalation_approval_context(&approval);
            if let Err(error) = approval.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::SignedAuditorRequest => {
            let request = decode_repair_payload!(SignedAuditorRequestV1);
            let mut context = signed_auditor_request_context(&request);
            match request.verify_signature() {
                Ok(public_key) => {
                    context.push(ValidationContextFieldV1::new(
                        "auditor_public_key",
                        public_key.to_string(),
                    ));
                    context
                }
                Err(error) => {
                    return auditor_signature_error_outcome(error, context, inputs, generated_at);
                }
            }
        }
        RepairValidationPayloadKindV1::WorkerSignaturePayload => {
            let payload = decode_repair_payload!(RepairWorkerSignaturePayloadV1);
            let context = repair_worker_signature_payload_context(&payload);
            if let Err(error) = payload.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::TaskEvent => {
            let event = decode_repair_payload!(RepairTaskEventV1);
            let context = repair_task_event_context(&event);
            if let Err(error) = event.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
        RepairValidationPayloadKindV1::AuditEvent => {
            let event = decode_repair_payload!(RepairAuditEventV1);
            let mut context = repair_task_event_context(&event.payload);
            context.push(ValidationContextFieldV1::new(
                "audit_sequence",
                event.header.sequence.to_string(),
            ));
            context.push(ValidationContextFieldV1::new(
                "audit_signer",
                event.header.signer.clone(),
            ));
            context.push(ValidationContextFieldV1::new(
                "payload_digest_hex",
                hex::encode(event.header.payload_digest),
            ));
            if let Err(error) = event.payload.validate() {
                return repair_validation_error_outcome(error, context, inputs, generated_at);
            }
            context
        }
    };

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        kind.success_message(),
        vec![
            "sorafs.reference.repair".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn decode_repair_archive_payload<T>(bytes: &[u8]) -> Result<T, norito::Error>
where
    T: for<'decode> norito::NoritoDeserialize<'decode>,
{
    match norito::decode_from_bytes::<T>(bytes) {
        Ok(value) => Ok(value),
        Err(primary_error) => {
            if let Ok(view) = norito::core::from_bytes_view(bytes) {
                let mut payload = view.as_bytes();
                if let Ok(value) = <T as norito::codec::Decode>::decode(&mut payload) {
                    return Ok(value);
                }
            }
            let mut input = bytes;
            <T as norito::codec::Decode>::decode(&mut input).map_err(|_| primary_error)
        }
    }
}

/// Validates a Norito-encoded [`ReplicationOrderV1`] and emits a reference outcome.
#[must_use]
pub fn validate_replication_order_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new("replication_order", input_label);
    let inputs = vec![input];
    let order = match norito::decode_from_bytes::<ReplicationOrderV1>(bytes) {
        Ok(order) => order,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ReplicationOrderV1 Norito payload: {error}"),
                "Re-encode the replication order with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.order".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ReplicationOrderV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = replication_order_context(&order);
    if let Err(error) = order.validate() {
        let code = replication_order_validation_code(&error);
        let category = replication_order_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("replication order validation failed: {error}"),
            "Regenerate the replication order from governed manifest and provider assignments.",
            vec![
                "sorafs.reference.order".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "replication order accepted",
        vec![
            "sorafs.reference.order".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`SignedReplicationOrderV1`] and emits a reference outcome.
#[must_use]
pub fn validate_signed_replication_order_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new("signed_replication_order", input_label);
    let inputs = vec![input];
    let envelope = match norito::decode_from_bytes::<SignedReplicationOrderV1>(bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode SignedReplicationOrderV1 Norito payload: {error}"),
                "Re-encode the signed replication order with the canonical SoraFS Norito schema.",
                vec![
                    "sorafs.reference.order".to_owned(),
                    "sorafs.reference.code.SFS-NORITO-001".to_owned(),
                ],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "SignedReplicationOrderV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = signed_replication_order_context(&envelope);
    if let Err(error) = envelope.validate() {
        let code = signed_replication_order_validation_code(&error);
        let category = signed_replication_order_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "validation_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("signed replication order validation failed: {error}"),
            "Regenerate the signed replication order from governed order bytes and signature material.",
            vec![
                "sorafs.reference.order".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Err(error) = envelope.verify_signature() {
        context.push(ValidationContextFieldV1::new(
            "signature_error",
            error.to_string(),
        ));
        let code = replication_order_signature_verification_code(&error);
        return ValidationOutcomeV1::error(
            code,
            replication_order_signature_verification_category(&error),
            format!("signed replication order signature validation failed: {error}"),
            "Resign the replication order with the governed issuer Ed25519 key and canonical order signing bytes.",
            vec![
                "sorafs.reference.order".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "signed replication order accepted",
        vec![
            "sorafs.reference.order".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`ProviderAdmissionEnvelopeV1`] and emits a reference outcome.
#[must_use]
pub fn validate_provider_admission_envelope_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new("provider_admission_envelope", input_label);
    let inputs = vec![input];
    let envelope = match norito::decode_from_bytes::<ProviderAdmissionEnvelopeV1>(bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ProviderAdmissionEnvelopeV1 Norito payload: {error}"),
                "Re-encode the provider admission envelope with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.admission".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ProviderAdmissionEnvelopeV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = provider_admission_context(&envelope);
    match verify_envelope(&envelope) {
        Ok(advert_digest) => {
            context.push(ValidationContextFieldV1::new(
                "verified_advert_body_digest_hex",
                hex::encode(advert_digest),
            ));
            ValidationOutcomeV1::ok(
                "SFS-OK-000",
                "provider admission envelope accepted",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    "sorafs.reference.code.SFS-OK-000".to_owned(),
                ],
                context,
                inputs,
                generated_at,
            )
        }
        Err(error) => {
            let code = provider_admission_envelope_code(&error);
            let category = provider_admission_envelope_category(&error);
            context.push(ValidationContextFieldV1::new(
                "validation_error",
                error.to_string(),
            ));
            ValidationOutcomeV1::error(
                code,
                category,
                format!("provider admission envelope validation failed: {error}"),
                "Regenerate the admission envelope from governed provider metadata and council signatures.",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    format!("sorafs.reference.code.{code}"),
                ],
                context,
                inputs,
                generated_at,
            )
        }
    }
}

/// Validates a Norito-encoded provider admission renewal against a previous envelope.
#[must_use]
pub fn validate_provider_admission_renewal_bytes(
    previous_envelope_bytes: &[u8],
    renewal_bytes: &[u8],
    previous_envelope_label: impl Into<String>,
    renewal_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs = vec![
        ValidationInputV1::new("provider_admission_envelope", previous_envelope_label),
        ValidationInputV1::new("provider_admission_renewal", renewal_label),
    ];
    let previous_envelope = match norito::decode_from_bytes::<ProviderAdmissionEnvelopeV1>(
        previous_envelope_bytes,
    ) {
        Ok(envelope) => envelope,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!(
                    "failed to decode previous ProviderAdmissionEnvelopeV1 Norito payload: {error}"
                ),
                "Re-encode the previous provider admission envelope with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.admission".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ProviderAdmissionEnvelopeV1",
                )],
                inputs,
                generated_at,
            );
        }
    };
    let renewal = match norito::decode_from_bytes::<ProviderAdmissionRenewalV1>(renewal_bytes) {
        Ok(renewal) => renewal,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ProviderAdmissionRenewalV1 Norito payload: {error}"),
                "Re-encode the provider admission renewal with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.admission".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ProviderAdmissionRenewalV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = provider_admission_renewal_context(&renewal);
    let previous_record = match AdmissionRecord::new(previous_envelope) {
        Ok(record) => record,
        Err(error) => {
            let code = provider_admission_envelope_code(&error);
            let category = provider_admission_envelope_category(&error);
            context.push(ValidationContextFieldV1::new(
                "previous_envelope_error",
                error.to_string(),
            ));
            return ValidationOutcomeV1::error(
                code,
                category,
                format!("previous provider admission envelope validation failed: {error}"),
                "Regenerate the previous admission envelope from governed provider metadata and council signatures.",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    format!("sorafs.reference.code.{code}"),
                ],
                context,
                inputs,
                generated_at,
            );
        }
    };

    match previous_record.apply_renewal(&renewal) {
        Ok(updated_record) => {
            context.push(ValidationContextFieldV1::new(
                "updated_advert_body_digest_hex",
                hex::encode(updated_record.advert_body_digest),
            ));
            ValidationOutcomeV1::ok(
                "SFS-OK-000",
                "provider admission renewal accepted",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    "sorafs.reference.code.SFS-OK-000".to_owned(),
                ],
                context,
                inputs,
                generated_at,
            )
        }
        Err(error) => {
            let code = provider_admission_renewal_code(&error);
            let category = provider_admission_renewal_category(&error);
            context.push(ValidationContextFieldV1::new(
                "renewal_error",
                error.to_string(),
            ));
            ValidationOutcomeV1::error(
                code,
                category,
                format!("provider admission renewal validation failed: {error}"),
                "Regenerate the renewal from the previous admission envelope, updated governed envelope, and council signatures.",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    format!("sorafs.reference.code.{code}"),
                ],
                context,
                inputs,
                generated_at,
            )
        }
    }
}

/// Validates a Norito-encoded provider admission revocation against an envelope.
#[must_use]
pub fn validate_provider_admission_revocation_bytes(
    envelope_bytes: &[u8],
    revocation_bytes: &[u8],
    envelope_label: impl Into<String>,
    revocation_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs = vec![
        ValidationInputV1::new("provider_admission_envelope", envelope_label),
        ValidationInputV1::new("provider_admission_revocation", revocation_label),
    ];
    let envelope = match norito::decode_from_bytes::<ProviderAdmissionEnvelopeV1>(envelope_bytes) {
        Ok(envelope) => envelope,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ProviderAdmissionEnvelopeV1 Norito payload: {error}"),
                "Re-encode the provider admission envelope with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.admission".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ProviderAdmissionEnvelopeV1",
                )],
                inputs,
                generated_at,
            );
        }
    };
    let revocation = match norito::decode_from_bytes::<ProviderAdmissionRevocationV1>(
        revocation_bytes,
    ) {
        Ok(revocation) => revocation,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode ProviderAdmissionRevocationV1 Norito payload: {error}"),
                "Re-encode the provider admission revocation with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.admission".to_owned()],
                vec![ValidationContextFieldV1::new(
                    "schema",
                    "ProviderAdmissionRevocationV1",
                )],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = provider_admission_revocation_context(&revocation);
    let record = match AdmissionRecord::new(envelope) {
        Ok(record) => record,
        Err(error) => {
            let code = provider_admission_envelope_code(&error);
            let category = provider_admission_envelope_category(&error);
            context.push(ValidationContextFieldV1::new(
                "envelope_error",
                error.to_string(),
            ));
            return ValidationOutcomeV1::error(
                code,
                category,
                format!("provider admission envelope validation failed: {error}"),
                "Regenerate the admission envelope from governed provider metadata and council signatures.",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    format!("sorafs.reference.code.{code}"),
                ],
                context,
                inputs,
                generated_at,
            );
        }
    };

    match record.verify_revocation(&revocation) {
        Ok(()) => ValidationOutcomeV1::ok(
            "SFS-OK-000",
            "provider admission revocation accepted",
            vec![
                "sorafs.reference.admission".to_owned(),
                "sorafs.reference.code.SFS-OK-000".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        ),
        Err(error) => {
            let code = provider_admission_revocation_code(&error);
            let category = provider_admission_revocation_category(&error);
            context.push(ValidationContextFieldV1::new(
                "revocation_error",
                error.to_string(),
            ));
            ValidationOutcomeV1::error(
                code,
                category,
                format!("provider admission revocation validation failed: {error}"),
                "Regenerate the revocation from the governed admission envelope digest and council signatures.",
                vec![
                    "sorafs.reference.admission".to_owned(),
                    format!("sorafs.reference.code.{code}"),
                ],
                context,
                inputs,
                generated_at,
            )
        }
    }
}

/// Validates a Norito-encoded [`PdpCommitmentV1`] and emits a reference outcome.
#[must_use]
pub fn validate_pdp_commitment_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input_label = input_label.into();
    let inputs = vec![ValidationInputV1::new(
        "pdp_commitment",
        input_label.clone(),
    )];
    let commitment = match norito::decode_from_bytes::<PdpCommitmentV1>(bytes) {
        Ok(commitment) => commitment,
        Err(error) => {
            return pdp_decode_error(
                "PdpCommitmentV1",
                "pdp commitment",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };

    let mut context = pdp_commitment_context(&commitment);
    if let Err(error) = commitment.validate() {
        let code = pdp_commitment_validation_code(&error);
        let category = pdp_commitment_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "commitment_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP commitment validation failed: {error}"),
            "Regenerate the PDP commitment from the canonical manifest chunk profile and commitment tree roots.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP commitment accepted",
        pdp_tags("SFS-OK-000"),
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`PdpChallengeV1`] and emits a reference outcome.
#[must_use]
pub fn validate_pdp_challenge_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input_label = input_label.into();
    let inputs = vec![ValidationInputV1::new("pdp_challenge", input_label.clone())];
    let challenge = match norito::decode_from_bytes::<PdpChallengeV1>(bytes) {
        Ok(challenge) => challenge,
        Err(error) => {
            return pdp_decode_error(
                "PdpChallengeV1",
                "pdp challenge",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };

    let mut context = pdp_challenge_context(&challenge);
    if let Err(error) = challenge.validate() {
        let code = pdp_challenge_validation_code(&error);
        let category = pdp_challenge_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "challenge_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP challenge validation failed: {error}"),
            "Regenerate the PDP challenge from the canonical commitment, provider, epoch, and randomness inputs.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP challenge accepted",
        pdp_tags("SFS-OK-000"),
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`PdpProofV1`] and emits a reference outcome.
#[must_use]
pub fn validate_pdp_proof_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input_label = input_label.into();
    let inputs = vec![ValidationInputV1::new("pdp_proof", input_label.clone())];
    let proof = match norito::decode_from_bytes::<PdpProofV1>(bytes) {
        Ok(proof) => proof,
        Err(error) => {
            return pdp_decode_error(
                "PdpProofV1",
                "pdp proof",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };

    let mut context = pdp_proof_context(&proof);
    if let Err(error) = proof.validate() {
        let code = pdp_proof_validation_code(&error);
        let category = pdp_proof_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "proof_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP proof validation failed: {error}"),
            "Regenerate the PDP proof from the challenged segments, hot-leaf witnesses, and provider signature.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP proof accepted",
        pdp_tags("SFS-OK-000"),
        context,
        inputs,
        generated_at,
    )
}

/// Validates Norito-encoded PDP commitment and challenge payload binding.
#[must_use]
pub fn validate_pdp_commitment_challenge_bytes(
    commitment_bytes: &[u8],
    challenge_bytes: &[u8],
    commitment_label: impl Into<String>,
    challenge_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs = vec![
        ValidationInputV1::new("pdp_commitment", commitment_label),
        ValidationInputV1::new("pdp_challenge", challenge_label),
    ];
    let commitment = match norito::decode_from_bytes::<PdpCommitmentV1>(commitment_bytes) {
        Ok(commitment) => commitment,
        Err(error) => {
            return pdp_decode_error(
                "PdpCommitmentV1",
                "pdp commitment",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };
    let challenge = match norito::decode_from_bytes::<PdpChallengeV1>(challenge_bytes) {
        Ok(challenge) => challenge,
        Err(error) => {
            return pdp_decode_error(
                "PdpChallengeV1",
                "pdp challenge",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };

    let mut context = pdp_commitment_challenge_context(&commitment, &challenge);
    if let Err(error) = commitment.validate() {
        let code = pdp_commitment_validation_code(&error);
        let category = pdp_commitment_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "commitment_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP commitment validation failed: {error}"),
            "Regenerate the PDP commitment from the canonical manifest chunk profile and commitment tree roots.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }
    if let Err(error) = challenge.validate() {
        let code = pdp_challenge_validation_code(&error);
        let category = pdp_challenge_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "challenge_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP challenge validation failed: {error}"),
            "Regenerate the PDP challenge from the canonical commitment, provider, epoch, and randomness inputs.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }
    if commitment.manifest_digest != challenge.manifest_digest {
        return pdp_binding_error(
            "PDP challenge manifest digest does not match commitment",
            "Regenerate the challenge for the exact manifest digest named by the commitment.",
            context,
            inputs,
            generated_at,
        );
    }
    if commitment.chunk_profile != challenge.chunk_profile {
        return pdp_binding_error(
            "PDP challenge chunk profile does not match commitment",
            "Regenerate the challenge using the chunk profile embedded in the commitment.",
            context,
            inputs,
            generated_at,
        );
    }
    if challenge.samples.len() > usize::from(commitment.sample_window) {
        context.push(ValidationContextFieldV1::new(
            "sample_window_error",
            "challenge samples exceed commitment sample_window",
        ));
        return ValidationOutcomeV1::error(
            "SFS-PDP-001",
            CATEGORY_VALIDATION,
            "PDP challenge sample window exceeds the commitment",
            "Regenerate the challenge with at most the commitment sample_window samples, or reseal the commitment with a larger governed sample window.",
            pdp_tags("SFS-PDP-001"),
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP commitment/challenge accepted",
        pdp_tags("SFS-OK-000"),
        context,
        inputs,
        generated_at,
    )
}

/// Validates Norito-encoded PDP commitment, challenge, and proof payloads.
#[must_use]
pub fn validate_pdp_commitment_challenge_proof_bytes(
    commitment_bytes: &[u8],
    challenge_bytes: &[u8],
    proof_bytes: &[u8],
    commitment_label: impl Into<String>,
    challenge_label: impl Into<String>,
    proof_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let commitment_label = commitment_label.into();
    let challenge_label = challenge_label.into();
    let proof_label = proof_label.into();
    let commitment_outcome = validate_pdp_commitment_challenge_bytes(
        commitment_bytes,
        challenge_bytes,
        commitment_label.clone(),
        challenge_label.clone(),
        generated_at,
    );
    if !commitment_outcome.is_ok() {
        return commitment_outcome;
    }

    let proof_outcome = validate_pdp_challenge_proof_bytes(
        challenge_bytes,
        proof_bytes,
        challenge_label.clone(),
        proof_label.clone(),
        generated_at,
    );
    if !proof_outcome.is_ok() {
        return proof_outcome;
    }

    let mut context = commitment_outcome.context;
    context.extend(proof_outcome.context);
    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP commitment/challenge/proof accepted",
        pdp_tags("SFS-OK-000"),
        context,
        vec![
            ValidationInputV1::new("pdp_commitment", commitment_label),
            ValidationInputV1::new("pdp_challenge", challenge_label),
            ValidationInputV1::new("pdp_proof", proof_label),
        ],
        generated_at,
    )
}

/// Validates Norito-encoded PDP challenge/proof payloads and their pair binding.
#[must_use]
pub fn validate_pdp_challenge_proof_bytes(
    challenge_bytes: &[u8],
    proof_bytes: &[u8],
    challenge_label: impl Into<String>,
    proof_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs = vec![
        ValidationInputV1::new("pdp_challenge", challenge_label),
        ValidationInputV1::new("pdp_proof", proof_label),
    ];
    let challenge = match norito::decode_from_bytes::<PdpChallengeV1>(challenge_bytes) {
        Ok(challenge) => challenge,
        Err(error) => {
            return pdp_decode_error(
                "PdpChallengeV1",
                "pdp challenge",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };
    let proof = match norito::decode_from_bytes::<PdpProofV1>(proof_bytes) {
        Ok(proof) => proof,
        Err(error) => {
            return pdp_decode_error(
                "PdpProofV1",
                "pdp proof",
                error.to_string(),
                inputs,
                generated_at,
            );
        }
    };

    let mut context = pdp_challenge_proof_context(&challenge, &proof);
    if let Err(error) = challenge.validate() {
        let code = pdp_challenge_validation_code(&error);
        let category = pdp_challenge_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "challenge_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP challenge validation failed: {error}"),
            "Regenerate the PDP challenge from the canonical commitment, provider, epoch, and randomness inputs.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }
    if let Err(error) = proof.validate() {
        let code = pdp_proof_validation_code(&error);
        let category = pdp_proof_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "proof_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PDP proof validation failed: {error}"),
            "Regenerate the PDP proof from the challenged segments, hot-leaf witnesses, and provider signature.",
            pdp_tags(code),
            context,
            inputs,
            generated_at,
        );
    }
    if proof.challenge_id != challenge.challenge_id {
        return pdp_binding_error(
            "PDP proof challenge_id does not match challenge",
            "Regenerate the proof for the exact challenge identifier being validated.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.manifest_digest != challenge.manifest_digest {
        return pdp_binding_error(
            "PDP proof manifest digest does not match challenge",
            "Regenerate the proof for the manifest digest named by the challenge.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.provider_id != challenge.provider_id {
        return pdp_binding_error(
            "PDP proof provider_id does not match challenge",
            "Regenerate the proof with the provider identity named by the challenge.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.epoch_id != challenge.epoch_id {
        return pdp_binding_error(
            "PDP proof epoch_id does not match challenge",
            "Regenerate the proof for the challenge epoch being validated.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.issued_at_unix > challenge.response_deadline_unix {
        context.push(ValidationContextFieldV1::new(
            "deadline_error",
            "proof issued after challenge response deadline",
        ));
        return ValidationOutcomeV1::error(
            "SFS-POL-002",
            CATEGORY_POLICY,
            "PDP proof missed the challenge response deadline",
            "Treat the proof as late unless a governed policy override explicitly extends the deadline.",
            pdp_tags("SFS-POL-002"),
            context,
            inputs,
            generated_at,
        );
    }
    if let Err(error) = validate_pdp_proof_coverage(&challenge, &proof) {
        context.push(ValidationContextFieldV1::new("coverage_error", error));
        return ValidationOutcomeV1::error(
            "SFS-PDP-001",
            CATEGORY_VALIDATION,
            "PDP proof sample coverage does not match the challenge",
            "Regenerate the proof with exactly the segment and hot-leaf witnesses requested by the challenge.",
            pdp_tags("SFS-PDP-001"),
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PDP challenge/proof accepted",
        pdp_tags("SFS-OK-000"),
        context,
        inputs,
        generated_at,
    )
}

/// Validates Norito-encoded PoR challenge/proof payloads and their pair binding.
#[must_use]
pub fn validate_por_challenge_proof_bytes(
    challenge_bytes: &[u8],
    proof_bytes: &[u8],
    challenge_label: impl Into<String>,
    proof_label: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let inputs = vec![
        ValidationInputV1::new("por_challenge", challenge_label),
        ValidationInputV1::new("por_proof", proof_label),
    ];
    let challenge = match norito::decode_from_bytes::<PorChallengeV1>(challenge_bytes) {
        Ok(challenge) => challenge,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode PorChallengeV1 Norito payload: {error}"),
                "Re-encode the PoR challenge with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.por".to_owned()],
                vec![ValidationContextFieldV1::new("schema", "PorChallengeV1")],
                inputs,
                generated_at,
            );
        }
    };
    let proof = match norito::decode_from_bytes::<PorProofV1>(proof_bytes) {
        Ok(proof) => proof,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode PorProofV1 Norito payload: {error}"),
                "Re-encode the PoR proof with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.por".to_owned()],
                vec![ValidationContextFieldV1::new("schema", "PorProofV1")],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = por_context(&challenge, &proof);
    if let Err(error) = challenge.validate() {
        let code = por_challenge_validation_code(&error);
        let category = por_challenge_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "challenge_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PoR challenge validation failed: {error}"),
            "Regenerate the PoR challenge from the canonical manifest, provider, epoch, and randomness inputs.",
            vec![
                "sorafs.reference.por".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }
    if let Err(error) = proof.validate() {
        let code = por_proof_validation_code(&error);
        context.push(ValidationContextFieldV1::new(
            "proof_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            CATEGORY_VALIDATION,
            format!("PoR proof validation failed: {error}"),
            "Regenerate the PoR proof from the challenged chunks and canonical authentication path.",
            vec![
                "sorafs.reference.por".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }
    if proof.challenge_id != challenge.challenge_id {
        return por_binding_error(
            "SFS-POR-003",
            "proof challenge_id does not match challenge",
            "Regenerate the proof for the exact challenge identifier being validated.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.manifest_digest != challenge.manifest_digest {
        return por_binding_error(
            "SFS-POR-003",
            "proof manifest digest does not match challenge",
            "Regenerate the proof for the manifest digest named by the challenge.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.provider_id != challenge.provider_id {
        return por_binding_error(
            "SFS-POR-003",
            "proof provider_id does not match challenge",
            "Regenerate the proof with the provider identity named by the challenge.",
            context,
            inputs,
            generated_at,
        );
    }
    if proof.submitted_at > challenge.deadline_at {
        context.push(ValidationContextFieldV1::new(
            "deadline_error",
            "proof submitted after challenge deadline",
        ));
        return ValidationOutcomeV1::error(
            "SFS-POL-002",
            CATEGORY_POLICY,
            "PoR proof missed the challenge deadline",
            "Treat the proof as late unless a governed policy override explicitly extends the deadline.",
            vec![
                "sorafs.reference.por".to_owned(),
                "sorafs.reference.code.SFS-POL-002".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }
    let challenge_indices: BTreeSet<u64> = challenge.sample_indices.iter().copied().collect();
    let proof_indices: BTreeSet<u64> = proof
        .samples
        .iter()
        .map(|sample| sample.sample_index)
        .collect();
    if challenge_indices.len() != challenge.sample_indices.len()
        || proof_indices.len() != proof.samples.len()
        || challenge_indices != proof_indices
    {
        context.push(ValidationContextFieldV1::new(
            "sample_coverage_error",
            "proof samples do not match challenge sample indices",
        ));
        return ValidationOutcomeV1::error(
            "SFS-POR-001",
            CATEGORY_VALIDATION,
            "PoR proof sample coverage does not match the challenge",
            "Regenerate the proof with exactly the sample indices requested by the challenge.",
            vec![
                "sorafs.reference.por".to_owned(),
                "sorafs.reference.code.SFS-POR-001".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    context.push(ValidationContextFieldV1::new(
        "proof_digest_hex",
        hex::encode(proof.proof_digest()),
    ));
    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PoR challenge/proof accepted",
        vec![
            "sorafs.reference.por".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

/// Validates a Norito-encoded [`PotrReceiptV1`] and emits a reference outcome.
#[must_use]
pub fn validate_potr_receipt_bytes(
    bytes: &[u8],
    input_label: impl Into<String>,
    expected_tier: Option<ProofStreamTier>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let input = ValidationInputV1::new("potr_receipt", input_label);
    let inputs = vec![input];
    let receipt = match norito::decode_from_bytes::<PotrReceiptV1>(bytes) {
        Ok(receipt) => receipt,
        Err(error) => {
            return ValidationOutcomeV1::error(
                "SFS-NORITO-001",
                CATEGORY_NORITO,
                format!("failed to decode PotrReceiptV1 Norito payload: {error}"),
                "Re-encode the PoTR receipt with the canonical SoraFS Norito schema.",
                vec!["sorafs.reference.potr".to_owned()],
                vec![ValidationContextFieldV1::new("schema", "PotrReceiptV1")],
                inputs,
                generated_at,
            );
        }
    };

    let mut context = potr_context(&receipt);
    if let Err(error) = receipt.validate() {
        let code = potr_receipt_validation_code(&error);
        let category = potr_receipt_validation_category(&error);
        context.push(ValidationContextFieldV1::new(
            "receipt_error",
            error.to_string(),
        ));
        return ValidationOutcomeV1::error(
            code,
            category,
            format!("PoTR receipt validation failed: {error}"),
            "Regenerate the PoTR receipt from the canonical timed retrieval observation and signer material.",
            vec![
                "sorafs.reference.potr".to_owned(),
                format!("sorafs.reference.code.{code}"),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    if let Some(expected_tier) = expected_tier
        && receipt.tier != expected_tier
    {
        context.push(ValidationContextFieldV1::new(
            "expected_tier",
            proof_stream_tier_label(expected_tier),
        ));
        context.push(ValidationContextFieldV1::new(
            "tier_error",
            "receipt tier does not match requested profile",
        ));
        return ValidationOutcomeV1::error(
            "SFS-POTR-002",
            CATEGORY_VALIDATION,
            "PoTR receipt tier does not match the requested profile",
            "Validate the receipt against the tier profile that was requested, or rerun the retrieval for the requested tier.",
            vec![
                "sorafs.reference.potr".to_owned(),
                "sorafs.reference.code.SFS-POTR-002".to_owned(),
            ],
            context,
            inputs,
            generated_at,
        );
    }

    ValidationOutcomeV1::ok(
        "SFS-OK-000",
        "PoTR receipt accepted",
        vec![
            "sorafs.reference.potr".to_owned(),
            "sorafs.reference.code.SFS-OK-000".to_owned(),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn advert_context(advert: &ProviderAdvertV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(advert.body.provider_id)),
        ValidationContextFieldV1::new("profile_id", advert.body.profile_id.clone()),
        ValidationContextFieldV1::new("issued_at", advert.issued_at.to_string()),
        ValidationContextFieldV1::new("expires_at", advert.expires_at.to_string()),
        ValidationContextFieldV1::new("ttl_secs", advert.ttl().to_string()),
        ValidationContextFieldV1::new(
            "signature_algorithm",
            format!("{:?}", advert.signature.algorithm),
        ),
    ]
}

fn pdp_tags(code: &str) -> Vec<String> {
    vec![
        "sorafs.reference.pdp".to_owned(),
        format!("sorafs.reference.code.{code}"),
    ]
}

fn pdp_decode_error(
    schema: &'static str,
    label: &'static str,
    error: String,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ValidationOutcomeV1::error(
        "SFS-NORITO-001",
        CATEGORY_NORITO,
        format!("failed to decode {schema} Norito payload: {error}"),
        format!("Re-encode the {label} with the canonical SoraFS Norito schema."),
        pdp_tags("SFS-NORITO-001"),
        vec![ValidationContextFieldV1::new("schema", schema)],
        inputs,
        generated_at,
    )
}

fn pdp_binding_error(
    message: impl Into<String>,
    action: impl Into<String>,
    context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ValidationOutcomeV1::error(
        "SFS-PDP-003",
        CATEGORY_VALIDATION,
        message,
        action,
        pdp_tags("SFS-PDP-003"),
        context,
        inputs,
        generated_at,
    )
}

fn pdp_commitment_context(commitment: &PdpCommitmentV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("schema", "PdpCommitmentV1"),
        ValidationContextFieldV1::new("version", commitment.version.to_string()),
        ValidationContextFieldV1::new(
            "manifest_digest_hex",
            hex::encode(commitment.manifest_digest),
        ),
        ValidationContextFieldV1::new(
            "chunk_profile",
            chunk_profile_label(&commitment.chunk_profile),
        ),
        ValidationContextFieldV1::new(
            "commitment_root_hot_hex",
            hex::encode(commitment.commitment_root_hot),
        ),
        ValidationContextFieldV1::new(
            "commitment_root_segment_hex",
            hex::encode(commitment.commitment_root_segment),
        ),
        ValidationContextFieldV1::new("hash_algorithm", commitment.hash_algorithm.as_str()),
        ValidationContextFieldV1::new("hot_tree_height", commitment.hot_tree_height.to_string()),
        ValidationContextFieldV1::new(
            "segment_tree_height",
            commitment.segment_tree_height.to_string(),
        ),
        ValidationContextFieldV1::new("sample_window", commitment.sample_window.to_string()),
        ValidationContextFieldV1::new("sealed_at", commitment.sealed_at.to_string()),
    ]
}

fn pdp_challenge_context(challenge: &PdpChallengeV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("schema", "PdpChallengeV1"),
        ValidationContextFieldV1::new("version", challenge.version.to_string()),
        ValidationContextFieldV1::new("challenge_id_hex", hex::encode(challenge.challenge_id)),
        ValidationContextFieldV1::new(
            "manifest_digest_hex",
            hex::encode(challenge.manifest_digest),
        ),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(challenge.provider_id)),
        ValidationContextFieldV1::new(
            "chunk_profile",
            chunk_profile_label(&challenge.chunk_profile),
        ),
        ValidationContextFieldV1::new("seed_hex", hex::encode(challenge.seed)),
        ValidationContextFieldV1::new("epoch_id", challenge.epoch_id.to_string()),
        ValidationContextFieldV1::new("drand_round", challenge.drand_round.to_string()),
        ValidationContextFieldV1::new(
            "response_deadline_unix",
            challenge.response_deadline_unix.to_string(),
        ),
        ValidationContextFieldV1::new("sample_count", challenge.samples.len().to_string()),
        ValidationContextFieldV1::new(
            "hot_leaf_count",
            challenge
                .samples
                .iter()
                .map(|sample| sample.hot_leaf_indices.len())
                .sum::<usize>()
                .to_string(),
        ),
    ]
}

fn pdp_proof_context(proof: &PdpProofV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("schema", "PdpProofV1"),
        ValidationContextFieldV1::new("version", proof.version.to_string()),
        ValidationContextFieldV1::new("challenge_id_hex", hex::encode(proof.challenge_id)),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(proof.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(proof.provider_id)),
        ValidationContextFieldV1::new("epoch_id", proof.epoch_id.to_string()),
        ValidationContextFieldV1::new("proof_leaf_count", proof.proof_leaves.len().to_string()),
        ValidationContextFieldV1::new(
            "hot_leaf_proof_count",
            proof
                .proof_leaves
                .iter()
                .map(|leaf| leaf.hot_leaves.len())
                .sum::<usize>()
                .to_string(),
        ),
        ValidationContextFieldV1::new("signature_len", proof.signature.len().to_string()),
        ValidationContextFieldV1::new("issued_at_unix", proof.issued_at_unix.to_string()),
    ]
}

fn pdp_commitment_challenge_context(
    commitment: &PdpCommitmentV1,
    challenge: &PdpChallengeV1,
) -> Vec<ValidationContextFieldV1> {
    let mut context = pdp_commitment_context(commitment);
    context.extend([
        ValidationContextFieldV1::new("challenge_id_hex", hex::encode(challenge.challenge_id)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(challenge.provider_id)),
        ValidationContextFieldV1::new(
            "challenge_manifest_digest_hex",
            hex::encode(challenge.manifest_digest),
        ),
        ValidationContextFieldV1::new(
            "challenge_chunk_profile",
            chunk_profile_label(&challenge.chunk_profile),
        ),
        ValidationContextFieldV1::new("challenge_epoch_id", challenge.epoch_id.to_string()),
        ValidationContextFieldV1::new(
            "challenge_sample_count",
            challenge.samples.len().to_string(),
        ),
    ]);
    context
}

fn pdp_challenge_proof_context(
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("challenge_id_hex", hex::encode(challenge.challenge_id)),
        ValidationContextFieldV1::new("proof_challenge_id_hex", hex::encode(proof.challenge_id)),
        ValidationContextFieldV1::new(
            "manifest_digest_hex",
            hex::encode(challenge.manifest_digest),
        ),
        ValidationContextFieldV1::new(
            "proof_manifest_digest_hex",
            hex::encode(proof.manifest_digest),
        ),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(challenge.provider_id)),
        ValidationContextFieldV1::new("proof_provider_id_hex", hex::encode(proof.provider_id)),
        ValidationContextFieldV1::new("epoch_id", challenge.epoch_id.to_string()),
        ValidationContextFieldV1::new("proof_epoch_id", proof.epoch_id.to_string()),
        ValidationContextFieldV1::new("sample_count", challenge.samples.len().to_string()),
        ValidationContextFieldV1::new("proof_leaf_count", proof.proof_leaves.len().to_string()),
        ValidationContextFieldV1::new(
            "response_deadline_unix",
            challenge.response_deadline_unix.to_string(),
        ),
        ValidationContextFieldV1::new("issued_at_unix", proof.issued_at_unix.to_string()),
    ]
}

fn validate_pdp_proof_coverage(
    challenge: &PdpChallengeV1,
    proof: &PdpProofV1,
) -> Result<(), String> {
    let mut expected = BTreeMap::new();
    for sample in &challenge.samples {
        let hot_leaves = sample
            .hot_leaf_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if hot_leaves.len() != sample.hot_leaf_indices.len() {
            return Err(format!(
                "challenge segment {} contains duplicate hot leaf indices",
                sample.segment_index
            ));
        }
        if expected
            .insert(sample.segment_index, (sample.segment_leaf_hash, hot_leaves))
            .is_some()
        {
            return Err(format!(
                "challenge contains duplicate segment {}",
                sample.segment_index
            ));
        }
    }

    let mut actual = BTreeMap::new();
    for leaf in &proof.proof_leaves {
        let hot_leaves = leaf
            .hot_leaves
            .iter()
            .map(|hot| hot.leaf_index)
            .collect::<BTreeSet<_>>();
        if hot_leaves.len() != leaf.hot_leaves.len() {
            return Err(format!(
                "proof segment {} contains duplicate hot leaf proofs",
                leaf.segment_index
            ));
        }
        if actual
            .insert(leaf.segment_index, (leaf.segment_hash, hot_leaves))
            .is_some()
        {
            return Err(format!(
                "proof contains duplicate segment {}",
                leaf.segment_index
            ));
        }
    }

    if expected.keys().copied().collect::<BTreeSet<_>>()
        != actual.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err("proof segment set does not match challenge segment set".to_owned());
    }

    for (segment_index, (expected_hash, expected_hot_leaves)) in expected {
        let Some((actual_hash, actual_hot_leaves)) = actual.get(&segment_index) else {
            return Err(format!("proof is missing segment {segment_index}"));
        };
        if expected_hash != *actual_hash {
            return Err(format!(
                "proof segment {segment_index} hash does not match challenge"
            ));
        }
        if &expected_hot_leaves != actual_hot_leaves {
            return Err(format!(
                "proof segment {segment_index} hot leaf set does not match challenge"
            ));
        }
    }

    Ok(())
}

fn chunk_profile_label(profile: &crate::ChunkingProfileV1) -> String {
    format!("{}.{}@{}", profile.namespace, profile.name, profile.semver)
}

fn por_context(challenge: &PorChallengeV1, proof: &PorProofV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("challenge_id_hex", hex::encode(challenge.challenge_id)),
        ValidationContextFieldV1::new(
            "manifest_digest_hex",
            hex::encode(challenge.manifest_digest),
        ),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(challenge.provider_id)),
        ValidationContextFieldV1::new("chunking_profile", challenge.chunking_profile.clone()),
        ValidationContextFieldV1::new("sample_tier", challenge.sample_tier.to_string()),
        ValidationContextFieldV1::new("sample_count", challenge.sample_count.to_string()),
        ValidationContextFieldV1::new("proof_samples", proof.samples.len().to_string()),
        ValidationContextFieldV1::new("issued_at", challenge.issued_at.to_string()),
        ValidationContextFieldV1::new("deadline_at", challenge.deadline_at.to_string()),
        ValidationContextFieldV1::new("submitted_at", proof.submitted_at.to_string()),
    ]
}

fn potr_context(receipt: &PotrReceiptV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(receipt.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(receipt.provider_id)),
        ValidationContextFieldV1::new("tier", proof_stream_tier_label(receipt.tier)),
        ValidationContextFieldV1::new("deadline_ms", receipt.deadline_ms.to_string()),
        ValidationContextFieldV1::new("latency_ms", receipt.latency_ms.to_string()),
        ValidationContextFieldV1::new("status", format!("{:?}", receipt.status)),
        ValidationContextFieldV1::new("requested_at_ms", receipt.requested_at_ms.to_string()),
        ValidationContextFieldV1::new("responded_at_ms", receipt.responded_at_ms.to_string()),
        ValidationContextFieldV1::new("recorded_at_ms", receipt.recorded_at_ms.to_string()),
        ValidationContextFieldV1::new("range_start", receipt.range_start.to_string()),
        ValidationContextFieldV1::new("range_end", receipt.range_end.to_string()),
    ]
}

fn proof_stream_tier_label(tier: ProofStreamTier) -> &'static str {
    match tier {
        ProofStreamTier::Hot => "hot",
        ProofStreamTier::Warm => "warm",
        ProofStreamTier::Archive => "archive",
    }
}

fn provider_admission_context(
    envelope: &ProviderAdmissionEnvelopeV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new(
            "provider_id_hex",
            hex::encode(envelope.proposal.provider_id),
        ),
        ValidationContextFieldV1::new("profile_id", envelope.proposal.profile_id.clone()),
        ValidationContextFieldV1::new("proposal_digest_hex", hex::encode(envelope.proposal_digest)),
        ValidationContextFieldV1::new(
            "advert_body_digest_hex",
            hex::encode(envelope.advert_body_digest),
        ),
        ValidationContextFieldV1::new("issued_at", envelope.issued_at.to_string()),
        ValidationContextFieldV1::new("retention_epoch", envelope.retention_epoch.to_string()),
        ValidationContextFieldV1::new(
            "council_signatures",
            envelope.council_signatures.len().to_string(),
        ),
        ValidationContextFieldV1::new("endpoints", envelope.proposal.endpoints.len().to_string()),
        ValidationContextFieldV1::new(
            "jurisdiction_code",
            envelope.proposal.jurisdiction_code.clone(),
        ),
    ]
}

fn provider_admission_renewal_context(
    renewal: &ProviderAdmissionRenewalV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(renewal.provider_id)),
        ValidationContextFieldV1::new(
            "previous_envelope_digest_hex",
            hex::encode(renewal.previous_envelope_digest),
        ),
        ValidationContextFieldV1::new(
            "renewal_envelope_digest_hex",
            hex::encode(renewal.envelope_digest),
        ),
        ValidationContextFieldV1::new("renewal_issued_at", renewal.envelope.issued_at.to_string()),
        ValidationContextFieldV1::new(
            "renewal_retention_epoch",
            renewal.envelope.retention_epoch.to_string(),
        ),
        ValidationContextFieldV1::new(
            "renewal_council_signatures",
            renewal.envelope.council_signatures.len().to_string(),
        ),
        ValidationContextFieldV1::new("notes_present", renewal.notes.is_some().to_string()),
    ]
}

fn provider_admission_revocation_context(
    revocation: &ProviderAdmissionRevocationV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(revocation.provider_id)),
        ValidationContextFieldV1::new(
            "envelope_digest_hex",
            hex::encode(revocation.envelope_digest),
        ),
        ValidationContextFieldV1::new("revoked_at", revocation.revoked_at.to_string()),
        ValidationContextFieldV1::new(
            "council_signatures",
            revocation.council_signatures.len().to_string(),
        ),
        ValidationContextFieldV1::new("reason", revocation.reason.clone()),
        ValidationContextFieldV1::new("notes_present", revocation.notes.is_some().to_string()),
    ]
}

fn replication_order_context(order: &ReplicationOrderV1) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("order_id_hex", hex::encode(order.order_id)),
        ValidationContextFieldV1::new("manifest_cid_hex", hex::encode(&order.manifest_cid)),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(order.manifest_digest)),
        ValidationContextFieldV1::new("chunking_profile", order.chunking_profile.clone()),
        ValidationContextFieldV1::new("target_replicas", order.target_replicas.to_string()),
        ValidationContextFieldV1::new("assignments", order.assignments.len().to_string()),
        ValidationContextFieldV1::new("issued_at", order.issued_at.to_string()),
        ValidationContextFieldV1::new("deadline_at", order.deadline_at.to_string()),
    ]
}

fn signed_replication_order_context(
    envelope: &SignedReplicationOrderV1,
) -> Vec<ValidationContextFieldV1> {
    let mut context = replication_order_context(&envelope.order);
    context.push(ValidationContextFieldV1::new(
        "schema",
        "SignedReplicationOrderV1",
    ));
    context.push(ValidationContextFieldV1::new(
        "signed_order_version",
        envelope.version.to_string(),
    ));
    context.push(ValidationContextFieldV1::new(
        "signature_algorithm",
        signature_algorithm_label(envelope.signature.algorithm),
    ));
    context.push(ValidationContextFieldV1::new(
        "signature_public_key_len",
        envelope.signature.public_key.len().to_string(),
    ));
    context.push(ValidationContextFieldV1::new(
        "signature_len",
        envelope.signature.signature.len().to_string(),
    ));
    context
}

fn signature_algorithm_label(algorithm: SignatureAlgorithm) -> &'static str {
    match algorithm {
        SignatureAlgorithm::Ed25519 => "ed25519",
        SignatureAlgorithm::MultiSig => "multi-sig",
    }
}

fn repair_evidence_context(evidence: &RepairEvidenceV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(evidence.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(evidence.provider_id)),
        ValidationContextFieldV1::new("cause", repair_cause_label(evidence)),
    ];
    if let Some(por_history_id) = evidence.por_history_id {
        context.push(ValidationContextFieldV1::new(
            "por_history_id",
            por_history_id.to_string(),
        ));
    }
    context
}

fn repair_report_context(report: &RepairReportV1) -> Vec<ValidationContextFieldV1> {
    let mut context = repair_evidence_context(&report.evidence);
    context.push(ValidationContextFieldV1::new(
        "ticket_id",
        report.ticket_id.to_string(),
    ));
    context.push(ValidationContextFieldV1::new(
        "auditor_account",
        report.auditor_account.clone(),
    ));
    context.push(ValidationContextFieldV1::new(
        "submitted_at_unix",
        report.submitted_at_unix.to_string(),
    ));
    context
}

fn repair_task_context(task: &RepairTaskRecordV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("ticket_id", task.ticket_id.to_string()),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(task.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(task.provider_id)),
        ValidationContextFieldV1::new("auditor_account", task.auditor_account.clone()),
        ValidationContextFieldV1::new("state", repair_task_state_label(&task.state)),
    ];
    if let Some(por_history_id) = task.por_history_id {
        context.push(ValidationContextFieldV1::new(
            "por_history_id",
            por_history_id.to_string(),
        ));
    }
    if let Some(deadline) = task.sla_deadline_unix {
        context.push(ValidationContextFieldV1::new(
            "sla_deadline_unix",
            deadline.to_string(),
        ));
    }
    context.push(ValidationContextFieldV1::new(
        "slash_proposal_digest_present",
        task.slash_proposal_digest.is_some().to_string(),
    ));
    context
}

fn repair_slash_proposal_context(
    proposal: &RepairSlashProposalV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("ticket_id", proposal.ticket_id.to_string()),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(proposal.provider_id)),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(proposal.manifest_digest)),
        ValidationContextFieldV1::new("auditor_account", proposal.auditor_account.clone()),
        ValidationContextFieldV1::new(
            "proposed_penalty_nano",
            proposal.proposed_penalty_nano.to_string(),
        ),
        ValidationContextFieldV1::new("submitted_at_unix", proposal.submitted_at_unix.to_string()),
        ValidationContextFieldV1::new("approval_present", proposal.approval.is_some().to_string()),
    ]
}

fn repair_escalation_policy_context(
    policy: &RepairEscalationPolicyV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("quorum_bps", policy.quorum_bps.to_string()),
        ValidationContextFieldV1::new("minimum_voters", policy.minimum_voters.to_string()),
        ValidationContextFieldV1::new(
            "dispute_window_secs",
            policy.dispute_window_secs.to_string(),
        ),
        ValidationContextFieldV1::new("appeal_window_secs", policy.appeal_window_secs.to_string()),
        ValidationContextFieldV1::new("max_penalty_nano", policy.max_penalty_nano.to_string()),
    ]
}

fn repair_escalation_approval_context(
    approval: &RepairEscalationApprovalV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("approve_votes", approval.approve_votes.to_string()),
        ValidationContextFieldV1::new("reject_votes", approval.reject_votes.to_string()),
        ValidationContextFieldV1::new("abstain_votes", approval.abstain_votes.to_string()),
        ValidationContextFieldV1::new("approved_at_unix", approval.approved_at_unix.to_string()),
        ValidationContextFieldV1::new("finalized_at_unix", approval.finalized_at_unix.to_string()),
    ]
}

fn signed_auditor_request_context(
    request: &SignedAuditorRequestV1,
) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("auditor_account", request.auditor_account.clone()),
        ValidationContextFieldV1::new("nonce", request.nonce.to_string()),
        ValidationContextFieldV1::new(
            "payload_kind",
            signed_auditor_payload_label(&request.payload),
        ),
        ValidationContextFieldV1::new(
            "signature_algorithm",
            format!("{:?}", request.signature.algorithm),
        ),
    ];
    match &request.payload {
        SignedAuditorRequestPayloadV1::RepairReport(report) => {
            context.push(ValidationContextFieldV1::new(
                "ticket_id",
                report.ticket_id.to_string(),
            ));
            context.push(ValidationContextFieldV1::new(
                "manifest_digest_hex",
                hex::encode(report.evidence.manifest_digest),
            ));
            context.push(ValidationContextFieldV1::new(
                "provider_id_hex",
                hex::encode(report.evidence.provider_id),
            ));
        }
        SignedAuditorRequestPayloadV1::SlashProposal(proposal) => {
            context.push(ValidationContextFieldV1::new(
                "ticket_id",
                proposal.ticket_id.to_string(),
            ));
            context.push(ValidationContextFieldV1::new(
                "manifest_digest_hex",
                hex::encode(proposal.manifest_digest),
            ));
            context.push(ValidationContextFieldV1::new(
                "provider_id_hex",
                hex::encode(proposal.provider_id),
            ));
        }
    }
    context
}

fn repair_worker_signature_payload_context(
    payload: &RepairWorkerSignaturePayloadV1,
) -> Vec<ValidationContextFieldV1> {
    vec![
        ValidationContextFieldV1::new("ticket_id", payload.ticket_id.to_string()),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(payload.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(payload.provider_id)),
        ValidationContextFieldV1::new("worker_id", payload.worker_id.clone()),
        ValidationContextFieldV1::new("idempotency_key", payload.idempotency_key.clone()),
        ValidationContextFieldV1::new("action", repair_worker_action_label(&payload.action)),
    ]
}

fn repair_task_event_context(event: &RepairTaskEventV1) -> Vec<ValidationContextFieldV1> {
    let mut context = vec![
        ValidationContextFieldV1::new("ticket_id", event.ticket_id.to_string()),
        ValidationContextFieldV1::new("manifest_digest_hex", hex::encode(event.manifest_digest)),
        ValidationContextFieldV1::new("provider_id_hex", hex::encode(event.provider_id)),
        ValidationContextFieldV1::new("status", event.status.to_string()),
        ValidationContextFieldV1::new("occurred_at_unix", event.occurred_at_unix.to_string()),
    ];
    if let Some(actor) = &event.actor {
        context.push(ValidationContextFieldV1::new("actor", actor.clone()));
    }
    context
}

fn repair_cause_label(evidence: &RepairEvidenceV1) -> String {
    match &evidence.cause {
        crate::RepairCauseV1::PorFailure(_) => "por_failure",
        crate::RepairCauseV1::LatencySla(_) => "latency_sla",
        crate::RepairCauseV1::ReplicaShortfall(_) => "replica_shortfall",
        crate::RepairCauseV1::Manual(_) => "manual",
    }
    .to_owned()
}

fn repair_task_state_label(state: &RepairTaskStateV1) -> String {
    match state {
        RepairTaskStateV1::Queued(_) => "queued",
        RepairTaskStateV1::InProgress(_) => "in_progress",
        RepairTaskStateV1::Completed(_) => "completed",
        RepairTaskStateV1::Failed(_) => "failed",
        RepairTaskStateV1::Escalated(_) => "escalated",
    }
    .to_owned()
}

fn signed_auditor_payload_label(payload: &SignedAuditorRequestPayloadV1) -> String {
    match payload {
        SignedAuditorRequestPayloadV1::RepairReport(_) => "repair_report",
        SignedAuditorRequestPayloadV1::SlashProposal(_) => "slash_proposal",
    }
    .to_owned()
}

fn repair_worker_action_label(action: &RepairWorkerActionV1) -> String {
    match action {
        RepairWorkerActionV1::Claim { .. } => "claim",
        RepairWorkerActionV1::Heartbeat { .. } => "heartbeat",
        RepairWorkerActionV1::Complete { .. } => "complete",
        RepairWorkerActionV1::Fail { .. } => "fail",
    }
    .to_owned()
}

fn por_binding_error(
    code: &'static str,
    message: &'static str,
    action: &'static str,
    mut context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    context.push(ValidationContextFieldV1::new("binding_error", message));
    ValidationOutcomeV1::error(
        code,
        CATEGORY_VALIDATION,
        format!("PoR challenge/proof binding failed: {message}"),
        action,
        vec![
            "sorafs.reference.por".to_owned(),
            format!("sorafs.reference.code.{code}"),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn repair_validation_error_outcome(
    error: RepairValidationError,
    mut context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let code = repair_validation_code(&error);
    let category = repair_validation_category(&error);
    context.push(ValidationContextFieldV1::new(
        "validation_error",
        error.to_string(),
    ));
    ValidationOutcomeV1::error(
        code,
        category,
        format!("repair payload validation failed: {error}"),
        repair_validation_action(code),
        vec![
            "sorafs.reference.repair".to_owned(),
            format!("sorafs.reference.code.{code}"),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn auditor_signature_error_outcome(
    error: AuditorSignatureVerificationError,
    mut context: Vec<ValidationContextFieldV1>,
    inputs: Vec<ValidationInputV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let code = auditor_signature_verification_code(&error);
    let category = auditor_signature_verification_category(&error);
    context.push(ValidationContextFieldV1::new(
        "signature_error",
        error.to_string(),
    ));
    ValidationOutcomeV1::error(
        code,
        category,
        format!("signed auditor request validation failed: {error}"),
        repair_validation_action(code),
        vec![
            "sorafs.reference.repair".to_owned(),
            format!("sorafs.reference.code.{code}"),
        ],
        context,
        inputs,
        generated_at,
    )
}

fn repair_validation_action(code: &str) -> &'static str {
    match code {
        "SFS-NORITO-001" => "Re-encode the repair payload with the canonical SoraFS Norito schema.",
        "SFS-VAL-001" => {
            "Regenerate the repair payload from a manifest/provider record with non-zero canonical digests."
        }
        "SFS-VAL-002" => "Regenerate the repair payload with the supported V1 schema.",
        "SFS-REP-001" => {
            "Attach complete repair evidence with non-zero PoR samples, latency, or replica shortfall details."
        }
        "SFS-REP-002" => {
            "Regenerate the repair task, report, event, or worker payload from canonical scheduler state."
        }
        "SFS-POL-005" => {
            "Correct the repair timestamps or SLA/deadline ordering before submitting the payload."
        }
        "SFS-GOV-002" => {
            "Regenerate the repair escalation or slash governance payload from the governed policy state."
        }
        "SFS-SIG-004" => {
            "Resign the auditor request with the governed auditor Ed25519 key and canonical payload bytes."
        }
        "SFS-INT-001" => {
            "Retry validation after checking the local validator build and Norito encoder."
        }
        _ => "Regenerate the repair payload from canonical SoraFS state and retry validation.",
    }
}

fn advert_validation_code(error: &AdvertValidationError) -> &'static str {
    match error {
        AdvertValidationError::UnsupportedVersion(_) => "SFS-VAL-002",
        AdvertValidationError::UnknownProfileHandle { .. }
        | AdvertValidationError::MissingProfileAliases
        | AdvertValidationError::InvalidProfileAlias
        | AdvertValidationError::MissingRequiredAlias { .. }
        | AdvertValidationError::DuplicateProfileAlias { .. } => "SFS-VAL-003",
        AdvertValidationError::InvalidTimestamps
        | AdvertValidationError::TtlOutOfRange { .. }
        | AdvertValidationError::Expired { .. } => "SFS-POL-001",
        _ => "SFS-VAL-004",
    }
}

fn orderbook_validation_code(error: &OrderbookValidationError) -> &'static str {
    match error {
        OrderbookValidationError::UnsupportedOrderVersion { .. }
        | OrderbookValidationError::UnsupportedCancelVersion { .. }
        | OrderbookValidationError::UnsupportedTradeVersion { .. }
        | OrderbookValidationError::UnsupportedChannelVersion { .. }
        | OrderbookValidationError::UnsupportedReceiptVersion { .. } => "SFS-VAL-002",
        OrderbookValidationError::InvalidOrderId
        | OrderbookValidationError::InvalidMakerOrderId
        | OrderbookValidationError::InvalidTakerOrderId
        | OrderbookValidationError::InvalidTradeId
        | OrderbookValidationError::InvalidChannelId
        | OrderbookValidationError::InvalidReceiptId
        | OrderbookValidationError::InvalidChunkHash
        | OrderbookValidationError::InvalidProviderId => "SFS-VAL-001",
        OrderbookValidationError::InvalidTimestamp
        | OrderbookValidationError::ZeroNonce
        | OrderbookValidationError::InvalidFeeBps { .. }
        | OrderbookValidationError::ExpiredOrder { .. }
        | OrderbookValidationError::SettlementChannelNotOpen { .. } => "SFS-POL-007",
        OrderbookValidationError::InvalidSignature
        | OrderbookValidationError::InvalidPublicKeyLength { .. }
        | OrderbookValidationError::InvalidSignatureLength { .. } => "SFS-SIG-007",
        OrderbookValidationError::SettlementImbalance { .. }
        | OrderbookValidationError::ReceiptExceedsChannelBytes { .. }
        | OrderbookValidationError::ReceiptExceedsRemainingBytes { .. }
        | OrderbookValidationError::ReceiptExceedsEscrow { .. }
        | OrderbookValidationError::Amount(_) => "SFS-OBK-002",
        _ => "SFS-OBK-001",
    }
}

fn repair_validation_code(error: &RepairValidationError) -> &'static str {
    match error {
        RepairValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        RepairValidationError::EmptyString {
            field: "manifest_digest" | "provider_id" | "challenge_id",
        } => "SFS-VAL-001",
        RepairValidationError::InvalidSamples
        | RepairValidationError::InvalidLatency
        | RepairValidationError::InvalidMissingChunks => "SFS-REP-001",
        RepairValidationError::InvalidTimestamp { .. }
        | RepairValidationError::InvalidTimestampOrder { .. } => "SFS-POL-005",
        RepairValidationError::InvalidPenalty
        | RepairValidationError::InvalidQuorumBps { .. }
        | RepairValidationError::InvalidMinimumVoters
        | RepairValidationError::InvalidVoteCount => "SFS-GOV-002",
        RepairValidationError::InvalidPublicKey | RepairValidationError::InvalidSignature => {
            "SFS-SIG-004"
        }
        _ => "SFS-REP-002",
    }
}

fn auditor_signature_verification_code(error: &AuditorSignatureVerificationError) -> &'static str {
    match error {
        AuditorSignatureVerificationError::Validation(error) => repair_validation_code(error),
        AuditorSignatureVerificationError::PayloadEncoding { .. } => "SFS-INT-001",
        AuditorSignatureVerificationError::UnsupportedAlgorithm(_)
        | AuditorSignatureVerificationError::InvalidPublicKeyLength { .. }
        | AuditorSignatureVerificationError::InvalidSignatureLength { .. }
        | AuditorSignatureVerificationError::InvalidPublicKey { .. }
        | AuditorSignatureVerificationError::Verification { .. } => "SFS-SIG-004",
    }
}

fn pdp_commitment_validation_code(error: &PdpCommitmentValidationError) -> &'static str {
    match error {
        PdpCommitmentValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PdpCommitmentValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PdpCommitmentValidationError::UnsupportedProfileMultihash { .. } => "SFS-VAL-003",
        PdpCommitmentValidationError::InvalidSampleWindow => "SFS-PDP-001",
        PdpCommitmentValidationError::InvalidSealedAt => "SFS-POL-002",
        PdpCommitmentValidationError::InvalidHotRoot
        | PdpCommitmentValidationError::InvalidSegmentRoot
        | PdpCommitmentValidationError::UnsupportedHashAlgorithm { .. }
        | PdpCommitmentValidationError::InvalidHotTreeHeight
        | PdpCommitmentValidationError::InvalidSegmentTreeHeight => "SFS-PDP-002",
    }
}

fn pdp_challenge_validation_code(error: &PdpChallengeValidationError) -> &'static str {
    match error {
        PdpChallengeValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PdpChallengeValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PdpChallengeValidationError::InvalidDeadline => "SFS-POL-002",
        PdpChallengeValidationError::EmptySampleSet
        | PdpChallengeValidationError::EmptyHotLeafSet { .. }
        | PdpChallengeValidationError::DuplicateHotLeafIndex { .. }
        | PdpChallengeValidationError::InvalidSegmentDigest { .. } => "SFS-PDP-001",
        PdpChallengeValidationError::InvalidChallengeId
        | PdpChallengeValidationError::InvalidProviderId
        | PdpChallengeValidationError::InvalidSeed => "SFS-PDP-002",
    }
}

fn pdp_proof_validation_code(error: &PdpProofValidationError) -> &'static str {
    match error {
        PdpProofValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PdpProofValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PdpProofValidationError::MissingSignature => "SFS-SIG-008",
        PdpProofValidationError::EmptyProofSet
        | PdpProofValidationError::InvalidSegmentDigest { .. }
        | PdpProofValidationError::MissingHotLeafProofs { .. }
        | PdpProofValidationError::InvalidLeafDigest { .. } => "SFS-PDP-001",
        PdpProofValidationError::InvalidChallengeId
        | PdpProofValidationError::InvalidProviderId
        | PdpProofValidationError::InvalidIssuedAt => "SFS-PDP-002",
    }
}

fn por_challenge_validation_code(error: &PorChallengeValidationError) -> &'static str {
    match error {
        PorChallengeValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PorChallengeValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PorChallengeValidationError::UnknownChunkerHandle { .. } => "SFS-VAL-003",
        PorChallengeValidationError::InvalidDeadline { .. } => "SFS-POL-002",
        PorChallengeValidationError::ZeroSampleCount
        | PorChallengeValidationError::SampleCountMismatch { .. } => "SFS-POR-001",
        _ => "SFS-VAL-008",
    }
}

fn por_proof_validation_code(error: &PorProofValidationError) -> &'static str {
    match error {
        PorProofValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PorProofValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PorProofValidationError::MissingSamples => "SFS-POR-001",
        _ => "SFS-VAL-009",
    }
}

fn potr_receipt_validation_code(error: &PotrReceiptValidationError) -> &'static str {
    match error {
        PotrReceiptValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        PotrReceiptValidationError::InvalidManifestDigest => "SFS-VAL-001",
        PotrReceiptValidationError::LatencyExceedsDeadline { .. } => "SFS-POTR-001",
        PotrReceiptValidationError::InvalidSignature { .. } => "SFS-SIG-003",
        _ => "SFS-VAL-010",
    }
}

fn provider_admission_envelope_code(error: &ProviderAdmissionEnvelopeError) -> &'static str {
    match error {
        ProviderAdmissionEnvelopeError::Validation(error) => {
            provider_admission_validation_code(error)
        }
        ProviderAdmissionEnvelopeError::Signature(_) => "SFS-SIG-002",
        ProviderAdmissionEnvelopeError::Serialization { .. } => "SFS-INT-001",
    }
}

fn provider_admission_renewal_code(error: &ProviderAdmissionRenewalError) -> &'static str {
    match error {
        ProviderAdmissionRenewalError::UnsupportedVersion { .. } => "SFS-VAL-002",
        ProviderAdmissionRenewalError::Envelope(error) => provider_admission_envelope_code(error),
        ProviderAdmissionRenewalError::EnvelopeDigestMismatch { .. }
        | ProviderAdmissionRenewalError::PreviousDigestMismatch { .. }
        | ProviderAdmissionRenewalError::ProviderMismatch { .. } => "SFS-VAL-007",
        ProviderAdmissionRenewalError::RetentionNotExtended { .. }
        | ProviderAdmissionRenewalError::IssuedAtRegression { .. } => "SFS-POL-004",
        ProviderAdmissionRenewalError::ProfileIdChanged { .. }
        | ProviderAdmissionRenewalError::ProfileAliasesChanged
        | ProviderAdmissionRenewalError::CapabilitiesChanged
        | ProviderAdmissionRenewalError::AdvertKeyChanged => "SFS-VAL-006",
    }
}

fn provider_admission_revocation_code(error: &ProviderAdmissionRevocationError) -> &'static str {
    match error {
        ProviderAdmissionRevocationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        ProviderAdmissionRevocationError::ProviderMismatch { .. }
        | ProviderAdmissionRevocationError::EnvelopeDigestMismatch { .. } => "SFS-VAL-007",
        ProviderAdmissionRevocationError::ReasonEmpty => "SFS-VAL-006",
        ProviderAdmissionRevocationError::MissingSignatures
        | ProviderAdmissionRevocationError::Signature(_) => "SFS-SIG-002",
        ProviderAdmissionRevocationError::Serialization(_) => "SFS-INT-001",
    }
}

fn provider_admission_validation_code(error: &ProviderAdmissionValidationError) -> &'static str {
    match error {
        ProviderAdmissionValidationError::UnsupportedProposalVersion { .. }
        | ProviderAdmissionValidationError::UnsupportedEnvelopeVersion { .. } => "SFS-VAL-002",
        ProviderAdmissionValidationError::UnknownChunkerHandle { .. }
        | ProviderAdmissionValidationError::NonCanonicalProfileHandle { .. }
        | ProviderAdmissionValidationError::MissingCanonicalAlias { .. }
        | ProviderAdmissionValidationError::InvalidProfileAliases
        | ProviderAdmissionValidationError::UnknownProfileAlias { .. } => "SFS-VAL-003",
        ProviderAdmissionValidationError::ProposalDigestMismatch
        | ProviderAdmissionValidationError::AdvertDigestMismatch => "SFS-VAL-007",
        ProviderAdmissionValidationError::MissingCouncilSignatures => "SFS-SIG-002",
        ProviderAdmissionValidationError::InvalidRetentionEpoch { .. } => "SFS-POL-004",
        _ => "SFS-VAL-006",
    }
}

fn replication_order_validation_code(error: &ReplicationOrderValidationError) -> &'static str {
    match error {
        ReplicationOrderValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        ReplicationOrderValidationError::InvalidManifestDigest => "SFS-VAL-001",
        ReplicationOrderValidationError::UnknownChunkerHandle { .. }
        | ReplicationOrderValidationError::NonCanonicalProfileHandle { .. } => "SFS-VAL-003",
        ReplicationOrderValidationError::InvalidDeadline
        | ReplicationOrderValidationError::SlaInvalid(_) => "SFS-POL-003",
        _ => "SFS-VAL-005",
    }
}

fn signed_replication_order_validation_code(
    error: &SignedReplicationOrderValidationError,
) -> &'static str {
    match error {
        SignedReplicationOrderValidationError::UnsupportedVersion { .. } => "SFS-VAL-002",
        SignedReplicationOrderValidationError::Order(error) => {
            replication_order_validation_code(error)
        }
        SignedReplicationOrderValidationError::InvalidSignature => "SFS-SIG-006",
    }
}

fn replication_order_signature_verification_code(
    error: &ReplicationOrderSignatureVerificationError,
) -> &'static str {
    match error {
        ReplicationOrderSignatureVerificationError::PayloadEncoding { .. } => "SFS-INT-001",
        ReplicationOrderSignatureVerificationError::UnsupportedAlgorithm(_)
        | ReplicationOrderSignatureVerificationError::InvalidPublicKeyLength { .. }
        | ReplicationOrderSignatureVerificationError::InvalidSignatureLength { .. }
        | ReplicationOrderSignatureVerificationError::InvalidPublicKey { .. }
        | ReplicationOrderSignatureVerificationError::Verification { .. } => "SFS-SIG-006",
    }
}

fn repair_validation_category(error: &RepairValidationError) -> &'static str {
    match error {
        RepairValidationError::InvalidTimestamp { .. }
        | RepairValidationError::InvalidTimestampOrder { .. }
        | RepairValidationError::InvalidPenalty
        | RepairValidationError::InvalidQuorumBps { .. }
        | RepairValidationError::InvalidMinimumVoters
        | RepairValidationError::InvalidVoteCount => CATEGORY_POLICY,
        RepairValidationError::InvalidPublicKey | RepairValidationError::InvalidSignature => {
            CATEGORY_SIGNATURE
        }
        _ => CATEGORY_VALIDATION,
    }
}

fn auditor_signature_verification_category(
    error: &AuditorSignatureVerificationError,
) -> &'static str {
    match error {
        AuditorSignatureVerificationError::Validation(error) => repair_validation_category(error),
        AuditorSignatureVerificationError::PayloadEncoding { .. } => CATEGORY_INTERNAL,
        _ => CATEGORY_SIGNATURE,
    }
}

fn pdp_commitment_validation_category(error: &PdpCommitmentValidationError) -> &'static str {
    match error {
        PdpCommitmentValidationError::InvalidSealedAt => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn pdp_challenge_validation_category(error: &PdpChallengeValidationError) -> &'static str {
    match error {
        PdpChallengeValidationError::InvalidDeadline => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn pdp_proof_validation_category(error: &PdpProofValidationError) -> &'static str {
    match error {
        PdpProofValidationError::MissingSignature => CATEGORY_SIGNATURE,
        _ => CATEGORY_VALIDATION,
    }
}

fn advert_validation_category(error: &AdvertValidationError) -> &'static str {
    match error {
        AdvertValidationError::InvalidTimestamps
        | AdvertValidationError::TtlOutOfRange { .. }
        | AdvertValidationError::Expired { .. } => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn orderbook_validation_category(error: &OrderbookValidationError) -> &'static str {
    match error {
        OrderbookValidationError::InvalidTimestamp
        | OrderbookValidationError::ZeroNonce
        | OrderbookValidationError::InvalidFeeBps { .. }
        | OrderbookValidationError::ExpiredOrder { .. }
        | OrderbookValidationError::SettlementChannelNotOpen { .. } => CATEGORY_POLICY,
        OrderbookValidationError::InvalidSignature
        | OrderbookValidationError::InvalidPublicKeyLength { .. }
        | OrderbookValidationError::InvalidSignatureLength { .. } => CATEGORY_SIGNATURE,
        _ => CATEGORY_VALIDATION,
    }
}

fn por_challenge_validation_category(error: &PorChallengeValidationError) -> &'static str {
    match error {
        PorChallengeValidationError::InvalidDeadline { .. } => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn potr_receipt_validation_category(error: &PotrReceiptValidationError) -> &'static str {
    match error {
        PotrReceiptValidationError::LatencyExceedsDeadline { .. } => CATEGORY_POLICY,
        PotrReceiptValidationError::InvalidSignature { .. } => CATEGORY_SIGNATURE,
        _ => CATEGORY_VALIDATION,
    }
}

fn provider_admission_envelope_category(error: &ProviderAdmissionEnvelopeError) -> &'static str {
    match error {
        ProviderAdmissionEnvelopeError::Validation(error) => {
            provider_admission_validation_category(error)
        }
        ProviderAdmissionEnvelopeError::Signature(_) => CATEGORY_SIGNATURE,
        ProviderAdmissionEnvelopeError::Serialization { .. } => CATEGORY_INTERNAL,
    }
}

fn provider_admission_renewal_category(error: &ProviderAdmissionRenewalError) -> &'static str {
    match error {
        ProviderAdmissionRenewalError::Envelope(error) => {
            provider_admission_envelope_category(error)
        }
        ProviderAdmissionRenewalError::RetentionNotExtended { .. }
        | ProviderAdmissionRenewalError::IssuedAtRegression { .. } => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn provider_admission_revocation_category(
    error: &ProviderAdmissionRevocationError,
) -> &'static str {
    match error {
        ProviderAdmissionRevocationError::MissingSignatures
        | ProviderAdmissionRevocationError::Signature(_) => CATEGORY_SIGNATURE,
        ProviderAdmissionRevocationError::Serialization(_) => CATEGORY_INTERNAL,
        _ => CATEGORY_VALIDATION,
    }
}

fn provider_admission_validation_category(
    error: &ProviderAdmissionValidationError,
) -> &'static str {
    match error {
        ProviderAdmissionValidationError::MissingCouncilSignatures => CATEGORY_SIGNATURE,
        ProviderAdmissionValidationError::InvalidRetentionEpoch { .. } => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn replication_order_validation_category(error: &ReplicationOrderValidationError) -> &'static str {
    match error {
        ReplicationOrderValidationError::InvalidDeadline
        | ReplicationOrderValidationError::SlaInvalid(_) => CATEGORY_POLICY,
        _ => CATEGORY_VALIDATION,
    }
}

fn signed_replication_order_validation_category(
    error: &SignedReplicationOrderValidationError,
) -> &'static str {
    match error {
        SignedReplicationOrderValidationError::UnsupportedVersion { .. } => CATEGORY_VALIDATION,
        SignedReplicationOrderValidationError::Order(error) => {
            replication_order_validation_category(error)
        }
        SignedReplicationOrderValidationError::InvalidSignature => CATEGORY_SIGNATURE,
    }
}

fn replication_order_signature_verification_category(
    error: &ReplicationOrderSignatureVerificationError,
) -> &'static str {
    match error {
        ReplicationOrderSignatureVerificationError::PayloadEncoding { .. } => CATEGORY_INTERNAL,
        _ => CATEGORY_SIGNATURE,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use norito::{decode_from_bytes, to_bytes};

    use super::*;
    use crate::repair::QueuedRepairStateV1;
    use crate::{
        AdvertEndpoint, AdvertSignature, AuditorSignatureV1, AvailabilityTier, CapabilityTlv,
        CapabilityType, CapacityMetadataEntry, EndpointKind, EndpointMetadata, EndpointMetadataKey,
        POTR_RECEIPT_VERSION_V1, PathDiversityPolicy, PotrSignatureAlgorithm, PotrSignatureV1,
        PotrStatus, ProviderAdvertBodyV1, ProviderCapabilityRangeV1, QosHints,
        REFRESH_RECOMMENDATION_SECS, REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1,
        REPAIR_TASK_VERSION_V1, REPLICATION_ORDER_VERSION_V1, RendezvousTopic, RepairCauseV1,
        RepairEvidenceV1, RepairPorFailureCauseV1, RepairReportV1, RepairTaskRecordV1,
        RepairTaskStateV1, RepairTicketId, ReplicationAssignmentV1, ReplicationOrderSlaV1,
        SIGNED_AUDITOR_REQUEST_VERSION_V1, SignatureAlgorithm, SignedAuditorRequestPayloadV1,
        SignedAuditorRequestV1, StakePointer,
    };

    fn workspace_fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }

    fn admission_envelope() -> ProviderAdmissionEnvelopeV1 {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/envelope_v1.to",
        ))
        .expect("read admission envelope fixture");
        decode_from_bytes(&bytes).expect("decode admission envelope fixture")
    }

    fn admission_renewal_bytes() -> Vec<u8> {
        fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/renewal_v1.to",
        ))
        .expect("read admission renewal fixture")
    }

    fn admission_revocation_bytes() -> Vec<u8> {
        fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/revocation_v1.to",
        ))
        .expect("read admission revocation fixture")
    }

    fn por_challenge() -> PorChallengeV1 {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/por/challenge_v1.to",
        ))
        .expect("read PoR challenge fixture");
        decode_from_bytes(&bytes).expect("decode PoR challenge fixture")
    }

    fn por_proof() -> PorProofV1 {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/por/proof_v1.to",
        ))
        .expect("read PoR proof fixture");
        decode_from_bytes(&bytes).expect("decode PoR proof fixture")
    }

    fn governance_node() -> GovernanceLogNodeV1 {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/governance/node_v1.to",
        ))
        .expect("read governance node fixture");
        decode_from_bytes(&bytes).expect("decode governance node fixture")
    }

    fn ed25519_signed_governance_node() -> GovernanceLogNodeV1 {
        let mut node = governance_node();
        let signing_key = SigningKey::from_bytes(&[0xA6; 32]);
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signature payload");
        let signature = signing_key.sign(&payload_bytes);
        node.publisher_signature = crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
        node
    }

    fn dilithium3_signed_governance_node() -> GovernanceLogNodeV1 {
        let mut node = governance_node();
        let key_pair = KeyPair::try_from_seed(vec![0xD6; 32], Algorithm::MlDsa)
            .expect("generate ML-DSA governance keypair");
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signature payload");
        let signature = IrohaSignature::try_new(key_pair.private_key(), &payload_bytes)
            .expect("sign governance payload with ML-DSA key");
        let (algorithm, public_key) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("encode ML-DSA public key");
        assert_eq!(algorithm, Algorithm::MlDsa);
        node.publisher_signature = crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: public_key.to_vec(),
            signature: signature.payload().to_vec(),
        };
        node
    }

    fn empty_ed25519_governance_signature() -> crate::GovernanceLogSignatureV1 {
        crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn sign_governance_dag_block(block: &mut GovernanceDagBlockV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = block
            .signature_payload_bytes()
            .expect("encode governance DAG block signing payload");
        let signature = signing_key.sign(&payload_bytes);
        block.block_signature = crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn sign_governance_dag_head(head: &mut GovernanceDagHeadV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = head
            .signature_payload_bytes()
            .expect("encode governance DAG head signing payload");
        let signature = signing_key.sign(&payload_bytes);
        head.head_signature = crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn signed_governance_dag_block(
        prev_block_cid: Option<Vec<u8>>,
        sequence: u64,
        timestamp: u64,
    ) -> GovernanceDagBlockV1 {
        let mut node = ed25519_signed_governance_node();
        node.node_cid = format!("bafygovernancelognode{sequence}").into_bytes();
        node.prev_cid = sequence
            .checked_sub(1)
            .map(|previous| format!("bafygovernancelognode{previous}").into_bytes());
        node.timestamp = timestamp;
        let signing_key = SigningKey::from_bytes(&[0xA6; 32]);
        let node_payload = node
            .signature_payload_bytes()
            .expect("encode governance node signing payload");
        let node_signature = signing_key.sign(&node_payload);
        node.publisher_signature = crate::GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: node_signature.to_bytes().to_vec(),
        };

        let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
        let block_cid = crate::governance_dag_block_cid_v1(
            prev_block_cid.as_deref(),
            sequence,
            timestamp + 10,
            &publisher_peer_id,
            &node,
        )
        .expect("derive governance DAG block CID");
        let mut block = GovernanceDagBlockV1 {
            version: crate::GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid,
            sequence,
            timestamp: timestamp + 10,
            publisher_peer_id,
            node,
            block_signature: empty_ed25519_governance_signature(),
        };
        sign_governance_dag_block(&mut block, &[0xC7; 32]);
        block
    }

    fn signed_governance_dag_chain() -> (Vec<GovernanceDagBlockV1>, GovernanceDagHeadV1) {
        let first = signed_governance_dag_block(None, 0, 1_700_000_300);
        let second = signed_governance_dag_block(Some(first.block_cid.clone()), 1, 1_700_000_360);
        let blocks = vec![first, second];
        let mut head = GovernanceDagHeadV1 {
            version: crate::GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: blocks
                .last()
                .expect("chain has a head block")
                .block_cid
                .clone(),
            block_count: blocks.len() as u64,
            generated_at: 1_700_001_000,
            publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
            checkpoint_cid: None,
            head_signature: empty_ed25519_governance_signature(),
        };
        sign_governance_dag_head(&mut head, &[0xD9; 32]);
        (blocks, head)
    }

    fn orderbook_signature() -> crate::OrderbookSignatureV1 {
        crate::OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![0xD7; PUBLIC_KEY_LENGTH],
            signature: vec![0x57; SIGNATURE_LENGTH],
        }
    }

    fn orderbook_order_request() -> OrderRequestV1 {
        OrderRequestV1 {
            version: crate::ORDERBOOK_ORDER_VERSION_V1,
            order_id: [0x71; 32],
            side: OrderSideV1::Bid,
            tier: OrderTierV1::Hot,
            price_per_gib: crate::XorAmount::from_micro(1_250_000),
            quantity_gib: 64,
            remaining_gib: 64,
            owner_account: b"buyer@sora".to_vec(),
            expiry_unix: 1_800_000_000,
            nonce: 7,
            maker_fee_bps: 10,
            taker_fee_bps: 15,
            signature: orderbook_signature(),
        }
    }

    fn orderbook_settlement_receipt() -> SettlementReceiptV1 {
        SettlementReceiptV1 {
            version: crate::SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x81; 32],
            channel_id: [0x82; 32],
            trade_id: [0x83; 32],
            range: crate::ByteRangeV1 {
                start: 128,
                end: 384,
            },
            chunk_hash: [0x84; 32],
            bytes_delivered: 256,
            xor_debited: crate::XorAmount::from_micro(100),
            provider_credit: crate::XorAmount::from_micro(90),
            fee_amount: crate::XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_010,
            settlement_signature: orderbook_signature(),
        }
    }

    fn potr_receipt() -> PotrReceiptV1 {
        PotrReceiptV1 {
            version: POTR_RECEIPT_VERSION_V1,
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            tier: ProofStreamTier::Hot,
            deadline_ms: 90_000,
            latency_ms: 42_000,
            status: PotrStatus::Success,
            requested_at_ms: 1_700_000_000_000,
            responded_at_ms: 1_700_000_042_000,
            recorded_at_ms: 1_700_000_042_100,
            range_start: 0,
            range_end: 1_048_575,
            request_id: Some([0x44; 16]),
            trace_id: Some([0x33; 16]),
            note: Some("ok".to_owned()),
            gateway_signature: None,
            provider_signature: None,
        }
    }

    fn repair_evidence() -> RepairEvidenceV1 {
        RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: [0x31; 32],
            provider_id: [0x32; 32],
            por_history_id: Some(7),
            cause: RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                challenge_id: [0x33; 32],
                failed_samples: 2,
                proof_digest: Some([0x34; 32]),
            }),
            evidence_json: None,
            notes: Some("auditor confirmed repair trigger".to_owned()),
        }
    }

    fn repair_report() -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".to_owned()),
            auditor_account: "auditor@sora".to_owned(),
            submitted_at_unix: 1_700_000_050,
            evidence: repair_evidence(),
            notes: Some("queued for repair".to_owned()),
        }
    }

    fn repair_task_record() -> RepairTaskRecordV1 {
        RepairTaskRecordV1 {
            version: REPAIR_TASK_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".to_owned()),
            manifest_digest: [0x31; 32],
            provider_id: [0x32; 32],
            auditor_account: "auditor@sora".to_owned(),
            state: RepairTaskStateV1::Queued(QueuedRepairStateV1 {
                queued_at_unix: 1_700_000_060,
                sla_deadline_unix: Some(1_700_086_400),
            }),
            por_history_id: Some(7),
            sla_deadline_unix: Some(1_700_086_400),
            scheduler_notes: Some("waiting for worker claim".to_owned()),
            slash_proposal_digest: None,
        }
    }

    fn signed_auditor_request() -> SignedAuditorRequestV1 {
        let signing_key = SigningKey::from_bytes(&[0xB6; 32]);
        let mut request = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "auditor@sora".to_owned(),
            nonce: 99,
            payload: SignedAuditorRequestPayloadV1::RepairReport(repair_report()),
            signature: AuditorSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; 64],
            },
        };
        let payload_bytes =
            norito::to_bytes(&request.signature_payload()).expect("encode auditor payload");
        let signature = signing_key.sign(&payload_bytes);
        request.signature.signature = signature.to_bytes().to_vec();
        request
    }

    fn signed_advert(now: u64) -> ProviderAdvertV1 {
        let body = ProviderAdvertBodyV1 {
            provider_id: [0x11; 32],
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
            stake: StakePointer {
                pool_id: [0x22; 32],
                stake_amount: 1_000_000,
            },
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1_500,
                max_concurrent_streams: 32,
            },
            capabilities: vec![
                CapabilityTlv {
                    cap_type: CapabilityType::ToriiGateway,
                    payload: Vec::new(),
                },
                CapabilityTlv {
                    cap_type: CapabilityType::ChunkRangeFetch,
                    payload: ProviderCapabilityRangeV1 {
                        max_chunk_span: 32,
                        min_granularity: 8,
                        supports_sparse_offsets: true,
                        requires_alignment: false,
                        supports_merkle_proof: true,
                    }
                    .to_bytes()
                    .expect("encode range capability"),
                },
            ],
            endpoints: vec![AdvertEndpoint {
                kind: EndpointKind::Torii,
                host_pattern: "storage.example.com".to_owned(),
                metadata: vec![EndpointMetadata {
                    key: EndpointMetadataKey::Region,
                    value: b"global".to_vec(),
                }],
            }],
            rendezvous_topics: vec![RendezvousTopic {
                topic: "sorafs.sf1.primary".to_owned(),
                region: "global".to_owned(),
            }],
            path_policy: PathDiversityPolicy {
                min_guard_weight: 10,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        };
        let signing_key = SigningKey::from_bytes(&[0xA5; 32]);
        let body_bytes = norito::to_bytes(&body).expect("encode advert body");
        let signature = signing_key.sign(&body_bytes);
        ProviderAdvertV1 {
            version: crate::PROVIDER_ADVERT_VERSION_V1,
            issued_at: now,
            expires_at: now + REFRESH_RECOMMENDATION_SECS,
            body,
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        }
    }

    fn replication_order() -> ReplicationOrderV1 {
        ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0xAB; 32],
            manifest_cid: b"bafyreplicaexamplecidroot".to_vec(),
            manifest_digest: [0x42; 32],
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 2,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: [0x10; 32],
                    slice_gib: 512,
                    lane: Some("lane-primary".to_owned()),
                },
                ReplicationAssignmentV1 {
                    provider_id: [0x11; 32],
                    slice_gib: 512,
                    lane: Some("lane-secondary".to_owned()),
                },
            ],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_086_400,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 86_400,
                min_availability_percent_milli: 99_500,
                min_por_success_percent_milli: 98_000,
            },
            metadata: vec![CapacityMetadataEntry {
                key: "governance.ticket".to_owned(),
                value: "ticket-sorafs-0001".to_owned(),
            }],
        }
    }

    fn signed_replication_order() -> SignedReplicationOrderV1 {
        let signing_key = SigningKey::from_bytes(&[0xA7; 32]);
        let mut envelope = SignedReplicationOrderV1 {
            version: crate::SIGNED_REPLICATION_ORDER_VERSION_V1,
            order: replication_order(),
            signature: crate::ReplicationOrderSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; 64],
            },
        };
        let payload_bytes = envelope
            .signature_payload_bytes()
            .expect("encode signed replication order payload");
        let signature = signing_key.sign(&payload_bytes);
        envelope.signature.signature = signature.to_bytes().to_vec();
        envelope
    }

    #[test]
    fn validation_context_field_new_sets_fields() {
        let field = ValidationContextFieldV1::new("provider", "alpha");
        assert_eq!(field.key, "provider");
        assert_eq!(field.value, "alpha");
    }

    #[test]
    fn validation_input_new_sets_fields() {
        let input = ValidationInputV1::new("provider_advert", "advert.to");
        assert_eq!(input.kind, "provider_advert");
        assert_eq!(input.path, "advert.to");
    }

    #[test]
    fn validation_outcome_ok_reports_success() {
        let outcome = ValidationOutcomeV1::ok(
            "SFS-OK-000",
            "accepted",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            42,
        );
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.version, VALIDATION_OUTCOME_VERSION_V1);
        assert_eq!(outcome.generated_at, 42);
    }

    #[test]
    fn validation_outcome_error_reports_failure() {
        let outcome = ValidationOutcomeV1::error(
            "SFS-VAL-004",
            "validation",
            "failed",
            "fix input",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            42,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.action.as_deref(), Some("fix input"));
    }

    #[test]
    fn validate_fixture_bundle_payloads_accepts_linked_advert_and_order() {
        let advert = signed_advert(1_700_000_000);
        let order = replication_order();
        let advert_bytes = to_bytes(&advert).expect("encode advert");
        let order_bytes = to_bytes(&order).expect("encode order");
        let payloads = [
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ProviderAdvert,
                "advert.to",
                &advert_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ReplicationOrder,
                "order.to",
                &order_bytes,
            ),
        ];

        let outcome = validate_fixture_bundle_payloads(&payloads, 1_700_000_001, 42);

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "linkable_artifacts" && field.value == "2"),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_fixture_bundle_payloads_accepts_orderbook_payloads_with_linked_artifacts() {
        let advert = signed_advert(1_700_000_000);
        let order = replication_order();
        let orderbook_order = orderbook_order_request();
        let receipt = orderbook_settlement_receipt();
        let advert_bytes = to_bytes(&advert).expect("encode advert");
        let order_bytes = to_bytes(&order).expect("encode order");
        let orderbook_order_bytes =
            to_bytes(&orderbook_order).expect("encode orderbook order request");
        let receipt_bytes = to_bytes(&receipt).expect("encode settlement receipt");
        let payloads = [
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ProviderAdvert,
                "advert.to",
                &advert_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ReplicationOrder,
                "order.to",
                &order_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::OrderbookOrderRequest,
                "orderbook/order_request_v1.to",
                &orderbook_order_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::OrderbookSettlementReceipt,
                "orderbook/settlement_receipt_v1.to",
                &receipt_bytes,
            ),
        ];

        let outcome = validate_fixture_bundle_payloads(&payloads, 1_700_000_001, 46);

        assert!(outcome.is_ok(), "{outcome:?}");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "artifact_count" && field.value == "4"),
            "{outcome:?}"
        );
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "linkable_artifacts" && field.value == "2"),
            "{outcome:?}"
        );
        assert!(
            outcome.inputs.iter().any(|input| {
                input.kind == "settlement_receipt"
                    && input.path == "orderbook/settlement_receipt_v1.to"
            }),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_fixture_bundle_payloads_rejects_single_linkable_artifact() {
        let order = replication_order();
        let order_bytes = to_bytes(&order).expect("encode order");
        let payloads = [FixtureBundlePayloadV1::new(
            FixtureBundlePayloadKindV1::ReplicationOrder,
            "order.to",
            &order_bytes,
        )];

        let outcome = validate_fixture_bundle_payloads(&payloads, 1_700_000_001, 43);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-BND-001");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_fixture_bundle_payloads_rejects_manifest_digest_mismatch() {
        let order = replication_order();
        let mut receipt = potr_receipt();
        receipt.manifest_digest = [0x99; 32];
        receipt.provider_id = [0x10; 32];
        let order_bytes = to_bytes(&order).expect("encode order");
        let receipt_bytes = to_bytes(&receipt).expect("encode receipt");
        let payloads = [
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ReplicationOrder,
                "order.to",
                &order_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::PotrReceipt,
                "receipt.to",
                &receipt_bytes,
            ),
        ];

        let outcome = validate_fixture_bundle_payloads(&payloads, 1_700_000_001, 44);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-BND-002");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_fixture_bundle_payloads_rejects_provider_assignment_mismatch() {
        let order = replication_order();
        let mut receipt = potr_receipt();
        receipt.manifest_digest = order.manifest_digest;
        receipt.provider_id = [0x99; 32];
        let order_bytes = to_bytes(&order).expect("encode order");
        let receipt_bytes = to_bytes(&receipt).expect("encode receipt");
        let payloads = [
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::ReplicationOrder,
                "order.to",
                &order_bytes,
            ),
            FixtureBundlePayloadV1::new(
                FixtureBundlePayloadKindV1::PotrReceipt,
                "receipt.to",
                &receipt_bytes,
            ),
        ];

        let outcome = validate_fixture_bundle_payloads(&payloads, 1_700_000_001, 45);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-BND-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_governance_log_node_bytes_accepts_fixture() {
        let node = governance_node();
        let bytes = to_bytes(&node).expect("encode governance node");
        let outcome = validate_governance_log_node_bytes(
            &bytes,
            "governance-node.to",
            Some(b"bafygovernancelognode"),
            46,
        );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "payload_kind" && field.value == "por_proof"),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_governance_log_node_bytes_verifies_ed25519_publisher_signature() {
        let node = ed25519_signed_governance_node();
        let bytes = to_bytes(&node).expect("encode governance node");
        let outcome = validate_governance_log_node_bytes(&bytes, "governance-node.to", None, 47);
        assert!(outcome.is_ok(), "{outcome:?}");

        let mut tampered = node;
        tampered.publisher_peer_id.extend_from_slice(b"-tampered");
        let bytes = to_bytes(&tampered).expect("encode tampered governance node");
        let outcome =
            validate_governance_log_node_bytes(&bytes, "tampered-governance-node.to", None, 48);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-005");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_governance_log_node_bytes_verifies_dilithium3_publisher_signature() {
        let node = dilithium3_signed_governance_node();
        let bytes = to_bytes(&node).expect("encode governance node");
        let outcome = validate_governance_log_node_bytes(&bytes, "governance-node.to", None, 47);
        assert!(outcome.is_ok(), "{outcome:?}");

        let mut tampered = node;
        tampered.timestamp += 1;
        let bytes = to_bytes(&tampered).expect("encode tampered governance node");
        let outcome =
            validate_governance_log_node_bytes(&bytes, "tampered-governance-node.to", None, 48);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-005");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_governance_log_node_bytes_rejects_malformed_norito() {
        let outcome = validate_governance_log_node_bytes(b"not norito", "bad-node.to", None, 47);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_governance_log_node_bytes_rejects_cid_mismatch() {
        let node = governance_node();
        let bytes = to_bytes(&node).expect("encode governance node");
        let outcome = validate_governance_log_node_bytes(
            &bytes,
            "governance-node.to",
            Some(b"bafywronggovernancenode"),
            48,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-GOV-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_governance_log_node_bytes_rejects_structural_failure() {
        let mut node = governance_node();
        node.node_cid.clear();
        let bytes = to_bytes(&node).expect("encode governance node");
        let outcome = validate_governance_log_node_bytes(&bytes, "bad-node.to", None, 49);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-GOV-001");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_governance_dag_block_bytes_accepts_signed_block() {
        let block = signed_governance_dag_block(None, 0, 1_700_000_300);
        let bytes = to_bytes(&block).expect("encode governance DAG block");
        let outcome = validate_governance_dag_block_bytes(
            &bytes,
            "governance-block.to",
            Some(&block.block_cid),
            50,
        );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "schema" && field.value == "GovernanceDagBlockV1"),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_governance_dag_block_bytes_rejects_cid_mismatch() {
        let block = signed_governance_dag_block(None, 0, 1_700_000_300);
        let bytes = to_bytes(&block).expect("encode governance DAG block");
        let outcome = validate_governance_dag_block_bytes(
            &bytes,
            "governance-block.to",
            Some(&[0xFF; 32]),
            51,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-GOV-004");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_governance_dag_head_chain_bytes_accepts_signed_head() {
        let (blocks, head) = signed_governance_dag_chain();
        let head_bytes = to_bytes(&head).expect("encode governance DAG head");
        let block_bytes: Vec<Vec<u8>> = blocks
            .iter()
            .map(|block| to_bytes(block).expect("encode governance DAG block"))
            .collect();
        let block_refs: Vec<(&[u8], String)> = block_bytes
            .iter()
            .enumerate()
            .map(|(index, bytes)| (bytes.as_slice(), format!("governance-block-{index}.to")))
            .collect();
        let outcome = validate_governance_dag_head_chain_bytes(
            &head_bytes,
            "governance-head.to",
            &block_refs,
            52,
        );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
    }

    #[test]
    fn validate_governance_dag_head_chain_bytes_rejects_block_count_mismatch() {
        let (blocks, mut head) = signed_governance_dag_chain();
        head.block_count += 1;
        sign_governance_dag_head(&mut head, &[0xD9; 32]);
        let head_bytes = to_bytes(&head).expect("encode governance DAG head");
        let block_bytes: Vec<Vec<u8>> = blocks
            .iter()
            .map(|block| to_bytes(block).expect("encode governance DAG block"))
            .collect();
        let block_refs: Vec<(&[u8], String)> = block_bytes
            .iter()
            .enumerate()
            .map(|(index, bytes)| (bytes.as_slice(), format!("governance-block-{index}.to")))
            .collect();
        let outcome = validate_governance_dag_head_chain_bytes(
            &head_bytes,
            "governance-head.to",
            &block_refs,
            53,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-GOV-008");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_provider_advert_bytes_accepts_signed_advert() {
        let advert = signed_advert(1_700_000_000);
        let bytes = to_bytes(&advert).expect("encode advert");
        let outcome =
            validate_provider_advert_bytes(&bytes, "advert.to", 1_700_000_001, 1_700_000_002);
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_provider_advert_bytes_rejects_malformed_norito() {
        let outcome = validate_provider_advert_bytes(b"not norito", "bad.to", 1, 2);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_provider_advert_bytes_rejects_policy_failure() {
        let mut advert = signed_advert(1_700_000_000);
        advert.expires_at = advert.issued_at + crate::MAX_ADVERT_TTL_SECS + 1;
        let bytes = to_bytes(&advert).expect("encode advert");
        let outcome = validate_provider_advert_bytes(&bytes, "expired.to", 1_700_000_001, 3);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POL-001");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_provider_advert_bytes_rejects_bad_signature() {
        let mut advert = signed_advert(1_700_000_000);
        advert.signature.signature[0] ^= 0x01;
        let bytes = to_bytes(&advert).expect("encode advert");
        let outcome = validate_provider_advert_bytes(&bytes, "bad-signature.to", 1_700_000_001, 4);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-001");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_accepts_fixture() {
        let envelope = admission_envelope();
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "envelope.to", 7);
        assert!(outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_provider_admission_renewal_bytes_accepts_fixture() {
        let envelope = admission_envelope();
        let envelope_bytes = to_bytes(&envelope).expect("encode envelope");
        let renewal_bytes = admission_renewal_bytes();
        let outcome = validate_provider_admission_renewal_bytes(
            &envelope_bytes,
            &renewal_bytes,
            "envelope.to",
            "renewal.to",
            7,
        );
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome.context.iter().any(|field| {
                field.key == "renewal_envelope_digest_hex" && field.value.len() == 64
            }),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_provider_admission_renewal_bytes_rejects_malformed_renewal() {
        let envelope = admission_envelope();
        let envelope_bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_renewal_bytes(
            &envelope_bytes,
            b"not norito",
            "envelope.to",
            "bad-renewal.to",
            8,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_provider_admission_revocation_bytes_accepts_fixture() {
        let envelope = admission_envelope();
        let envelope_bytes = to_bytes(&envelope).expect("encode envelope");
        let revocation_bytes = admission_revocation_bytes();
        let outcome = validate_provider_admission_revocation_bytes(
            &envelope_bytes,
            &revocation_bytes,
            "envelope.to",
            "revocation.to",
            9,
        );
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "envelope_digest_hex" && field.value.len() == 64),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_provider_admission_revocation_bytes_rejects_malformed_revocation() {
        let envelope = admission_envelope();
        let envelope_bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_revocation_bytes(
            &envelope_bytes,
            b"not norito",
            "envelope.to",
            "bad-revocation.to",
            10,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_malformed_norito() {
        let outcome =
            validate_provider_admission_envelope_bytes(b"not norito", "bad-envelope.to", 8);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_digest_mismatch() {
        let mut envelope = admission_envelope();
        envelope.proposal_digest[0] ^= 0x01;
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "bad-digest.to", 9);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-007");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_signature_failure() {
        let mut envelope = admission_envelope();
        envelope.council_signatures[0].signature[0] ^= 0x01;
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "bad-signature.to", 10);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-002");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_retention_policy() {
        let mut envelope = admission_envelope();
        envelope.issued_at = envelope.retention_epoch + 1;
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "bad-retention.to", 11);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POL-004");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_structural_failure() {
        let mut envelope = admission_envelope();
        envelope.proposal.provider_id = [0; 32];
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "bad-structure.to", 12);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-006");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_provider_admission_envelope_bytes_rejects_chunker_failure() {
        let mut envelope = admission_envelope();
        envelope.proposal.profile_id = "unknown.profile@1.0.0".to_owned();
        let bytes = to_bytes(&envelope).expect("encode envelope");
        let outcome = validate_provider_admission_envelope_bytes(&bytes, "bad-chunker.to", 13);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_accepts_fixtures() {
        let challenge = por_challenge();
        let proof = por_proof();
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "challenge.to",
            "proof.to",
            14,
        );
        assert!(outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_malformed_challenge() {
        let proof = por_proof();
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            b"not norito",
            &proof_bytes,
            "bad-challenge.to",
            "proof.to",
            15,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_binding_mismatch() {
        let challenge = por_challenge();
        let mut proof = por_proof();
        proof.challenge_id[0] ^= 0x01;
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "challenge.to",
            "bad-proof.to",
            16,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POR-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_late_proof() {
        let challenge = por_challenge();
        let mut proof = por_proof();
        proof.submitted_at = challenge.deadline_at + 1;
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "challenge.to",
            "late-proof.to",
            17,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POL-002");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_sample_coverage_mismatch() {
        let challenge = por_challenge();
        let mut proof = por_proof();
        proof.samples[0].sample_index = challenge.sample_indices[0] + 10_000;
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "challenge.to",
            "bad-coverage.to",
            18,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POR-001");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_challenge_structure_failure() {
        let mut challenge = por_challenge();
        let proof = por_proof();
        challenge.chunking_profile = "unknown.profile@1.0.0".to_owned();
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "bad-challenge.to",
            "proof.to",
            19,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_por_challenge_proof_bytes_rejects_proof_structure_failure() {
        let challenge = por_challenge();
        let mut proof = por_proof();
        proof.auth_path.clear();
        let challenge_bytes = to_bytes(&challenge).expect("encode challenge");
        let proof_bytes = to_bytes(&proof).expect("encode proof");
        let outcome = validate_por_challenge_proof_bytes(
            &challenge_bytes,
            &proof_bytes,
            "challenge.to",
            "bad-proof.to",
            20,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-009");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_potr_receipt_bytes_accepts_valid_receipt() {
        let receipt = potr_receipt();
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let outcome =
            validate_potr_receipt_bytes(&bytes, "receipt.to", Some(ProofStreamTier::Hot), 21);
        assert!(outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_potr_receipt_bytes_rejects_malformed_norito() {
        let outcome = validate_potr_receipt_bytes(b"not norito", "bad-receipt.to", None, 22);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_potr_receipt_bytes_rejects_late_success_receipt() {
        let mut receipt = potr_receipt();
        receipt.latency_ms = receipt.deadline_ms + 1;
        receipt.responded_at_ms = receipt.requested_at_ms + u64::from(receipt.latency_ms);
        receipt.recorded_at_ms = receipt.responded_at_ms + 100;
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let outcome = validate_potr_receipt_bytes(&bytes, "late-receipt.to", None, 23);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POTR-001");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_potr_receipt_bytes_rejects_invalid_signature() {
        let mut receipt = potr_receipt();
        receipt.gateway_signature = Some(PotrSignatureV1 {
            algorithm: PotrSignatureAlgorithm::Ed25519,
            public_key: vec![0u8; 16],
            signature: vec![0u8; 32],
        });
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let outcome = validate_potr_receipt_bytes(&bytes, "bad-signature.to", None, 24);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-003");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_potr_receipt_bytes_rejects_tier_mismatch() {
        let receipt = potr_receipt();
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let outcome =
            validate_potr_receipt_bytes(&bytes, "wrong-tier.to", Some(ProofStreamTier::Warm), 25);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POTR-002");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_potr_receipt_bytes_rejects_structural_failure() {
        let mut receipt = potr_receipt();
        receipt.range_start = receipt.range_end + 1;
        let bytes = to_bytes(&receipt).expect("encode receipt");
        let outcome = validate_potr_receipt_bytes(&bytes, "bad-range.to", None, 26);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-010");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_repair_payload_bytes_accepts_task_record() {
        let task = repair_task_record();
        let bytes = to_bytes(&task).expect("encode task");
        let outcome = validate_repair_payload_bytes(
            RepairValidationPayloadKindV1::TaskRecord,
            &bytes,
            "repair-task.to",
            27,
        );
        assert!(outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_repair_payload_bytes_accepts_signed_auditor_request() {
        let request = signed_auditor_request();
        let bytes = to_bytes(&request).expect("encode signed auditor request");
        let outcome = validate_repair_payload_bytes(
            RepairValidationPayloadKindV1::SignedAuditorRequest,
            &bytes,
            "signed-auditor.to",
            28,
        );
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
    }

    #[test]
    fn validate_repair_payload_bytes_rejects_malformed_norito() {
        let outcome = validate_repair_payload_bytes(
            RepairValidationPayloadKindV1::TaskRecord,
            b"not norito",
            "bad-repair-task.to",
            29,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_repair_payload_bytes_rejects_evidence_failure() {
        let mut evidence = repair_evidence();
        evidence.cause = RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
            challenge_id: [0x33; 32],
            failed_samples: 0,
            proof_digest: None,
        });
        let bytes = to_bytes(&evidence).expect("encode evidence");
        let outcome = validate_repair_payload_bytes(
            RepairValidationPayloadKindV1::Evidence,
            &bytes,
            "bad-evidence.to",
            30,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-REP-001", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_repair_payload_bytes_rejects_bad_auditor_signature() {
        let mut request = signed_auditor_request();
        request.signature.signature[0] ^= 0x01;
        let bytes = to_bytes(&request).expect("encode signed auditor request");
        let outcome = validate_repair_payload_bytes(
            RepairValidationPayloadKindV1::SignedAuditorRequest,
            &bytes,
            "bad-signed-auditor.to",
            31,
        );
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-004", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_orderbook_payload_bytes_accepts_order_request() {
        let order = orderbook_order_request();
        let bytes = to_bytes(&order).expect("encode orderbook order request");
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::OrderRequest,
            &bytes,
            "orderbook-order.to",
            32,
        );

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome.context.iter().any(|field| {
                field.key == "order_id_hex" && field.value == hex::encode(order.order_id)
            }),
            "{outcome:?}"
        );
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_orderbook_payload_bytes_rejects_malformed_norito() {
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::SettlementReceipt,
            b"not norito",
            "bad-receipt.to",
            33,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_orderbook_payload_bytes_rejects_policy_failure() {
        let mut order = orderbook_order_request();
        order.nonce = 0;
        let bytes = to_bytes(&order).expect("encode orderbook order request");
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::OrderRequest,
            &bytes,
            "bad-orderbook-order.to",
            34,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POL-007", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_orderbook_payload_bytes_rejects_signature_failure() {
        let mut order = orderbook_order_request();
        order.signature.signature.pop();
        let bytes = to_bytes(&order).expect("encode orderbook order request");
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::OrderRequest,
            &bytes,
            "bad-orderbook-signature.to",
            35,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-007", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_orderbook_payload_bytes_rejects_settlement_imbalance() {
        let mut receipt = orderbook_settlement_receipt();
        receipt.provider_credit = crate::XorAmount::from_micro(91);
        let bytes = to_bytes(&receipt).expect("encode orderbook settlement receipt");
        let outcome = validate_orderbook_payload_bytes(
            OrderbookValidationPayloadKindV1::SettlementReceipt,
            &bytes,
            "bad-orderbook-receipt.to",
            36,
        );

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OBK-002", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_replication_order_bytes_accepts_valid_order() {
        let order = replication_order();
        let bytes = to_bytes(&order).expect("encode order");
        let outcome = validate_replication_order_bytes(&bytes, "order.to", 1_700_000_002);
        assert!(outcome.is_ok());
        assert_eq!(outcome.code, "SFS-OK-000");
        let roundtrip: ValidationOutcomeV1 =
            decode_from_bytes(&to_bytes(&outcome).expect("encode outcome"))
                .expect("decode outcome");
        assert_eq!(roundtrip, outcome);
    }

    #[test]
    fn validate_signed_replication_order_bytes_accepts_signed_order() {
        let envelope = signed_replication_order();
        let bytes = to_bytes(&envelope).expect("encode signed order");
        let outcome = validate_signed_replication_order_bytes(&bytes, "signed-order.to", 7);

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(outcome.code, "SFS-OK-000");
        assert!(
            outcome
                .context
                .iter()
                .any(|field| field.key == "signature_algorithm" && field.value == "ed25519"),
            "{outcome:?}"
        );
    }

    #[test]
    fn validate_signed_replication_order_bytes_rejects_bad_signature() {
        let mut envelope = signed_replication_order();
        envelope.order.deadline_at += 1;
        let bytes = to_bytes(&envelope).expect("encode tampered signed order");
        let outcome = validate_signed_replication_order_bytes(&bytes, "bad-signed-order.to", 8);

        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-SIG-006", "{outcome:?}");
        assert_eq!(outcome.category, CATEGORY_SIGNATURE);
    }

    #[test]
    fn validate_replication_order_bytes_rejects_malformed_norito() {
        let outcome = validate_replication_order_bytes(b"not norito", "bad.to", 2);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-NORITO-001");
        assert_eq!(outcome.category, CATEGORY_NORITO);
    }

    #[test]
    fn validate_replication_order_bytes_rejects_manifest_digest_failure() {
        let mut order = replication_order();
        order.manifest_digest = [0; 32];
        let bytes = to_bytes(&order).expect("encode order");
        let outcome = validate_replication_order_bytes(&bytes, "bad-digest.to", 3);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-001");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_replication_order_bytes_rejects_chunker_failure() {
        let mut order = replication_order();
        order.chunking_profile = "sorafs-sf1".to_owned();
        let bytes = to_bytes(&order).expect("encode order");
        let outcome = validate_replication_order_bytes(&bytes, "bad-chunker.to", 4);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-003");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }

    #[test]
    fn validate_replication_order_bytes_rejects_policy_failure() {
        let mut order = replication_order();
        order.deadline_at = order.issued_at;
        let bytes = to_bytes(&order).expect("encode order");
        let outcome = validate_replication_order_bytes(&bytes, "bad-deadline.to", 5);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-POL-003");
        assert_eq!(outcome.category, CATEGORY_POLICY);
    }

    #[test]
    fn validate_replication_order_bytes_rejects_structural_failure() {
        let mut order = replication_order();
        order.assignments.clear();
        let bytes = to_bytes(&order).expect("encode order");
        let outcome = validate_replication_order_bytes(&bytes, "bad-assignments.to", 6);
        assert!(!outcome.is_ok());
        assert_eq!(outcome.code, "SFS-VAL-005");
        assert_eq!(outcome.category, CATEGORY_VALIDATION);
    }
}
