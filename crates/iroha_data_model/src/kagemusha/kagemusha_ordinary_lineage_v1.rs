//! First-release global ordinary financial-lineage CAS data.
//!
//! An ordinary platform key supplies no rollback-resistant local counter. These exact selectors
//! bind an operation to a globally exclusive DATA predecessor, separate from current FI status.
//! Decoding, hashing or an account signature never admits a State proof or lends a Native grant.
//! The producer must authenticate actual private DATA custody, exact ordinary whole proofs and
//! current FI/policy before issuing an anchored/ reserved/committed result. Native admission must
//! independently retain the installed issuer, current certified policy cut and exact pending WAL.
use super::{KagemushaOperationKindV1, KagemushaRetailEnrollmentOwnerV1};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Maximum canonical selector/request/result original; whole proof originals travel separately.
pub const KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1: usize = 32 * 1024;
/// Exact account request domain, distinct from read-only current FI status.
pub const KAGEMUSHA_ORDINARY_LINEAGE_REQUEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-lineage-cas-request\0";
/// Exact current issuer result domain. Issuance requires real durable DATA CAS and whole proof admission.
pub const KAGEMUSHA_ORDINARY_LINEAGE_RESULT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-lineage-cas-result\0";

/// Sole asset-owner metadata key selecting one authoritative DATA domain for a liability pool.
/// The complete value is this first-release type encoded through the maintained Norito JSON
/// `Json` wrapper, and its exact `AssetDefinition` preimage must be proved in current World.
pub const KAGEMUSHA_ORDINARY_LINEAGE_DATA_AUTHORITY_METADATA_KEY_V1: &str =
    "kagemusha_ordinary_lineage_data_authority_v1";

/// Independently governed exclusive DATA domain. This public type is data only: neither decoding
/// it nor learning an incarnation admits a service. Installed purpose, current asset-owner World
/// selection and the real Native DATA session must independently agree on the full original.
/// A replacement database cannot initialize another exclusion domain for the same liability pool.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageDataAuthorityV1")]
pub struct KagemushaOrdinaryLineageDataAuthorityV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact network/asset/incarnation liability pool, shared across FI lineages in that pool.
    pub liability_pool_id: [u8; 32],
    /// Purpose-bound exact installed client-profile, collection-schema and Kasumi release originals.
    /// The client profile independently pins the approved service/TLS endpoints and trust identities.
    pub service_identity_digest: [u8; 32],
    /// Exact immutable DATA incarnation identity, using the maintained current-control domain.
    pub data_incarnation_digest: [u8; 32],
    /// Exact logical dataspace, never an account alias or caller routing choice.
    pub dataspace: String,
    /// Exact authenticated DATA tenant.
    pub tenant: String,
    /// Exact independently delegated DATA principal.
    pub principal: String,
    /// Exact collection containing all lineage heads and source/nullifier exclusions.
    pub collection: String,
}
impl KagemushaOrdinaryLineageDataAuthorityV1 {
    /// Derive service identity only from independently retained complete original digests.
    /// This deterministic data helper does not establish custody or admit those originals.
    /// # Errors
    /// Refuses absent original identity.
    pub fn service_identity_digest_for_originals(
        client_profile: [u8; 32],
        collections: [u8; 32],
        kasumi_release: [u8; 32],
    ) -> Result<[u8; 32], String> {
        nonzero(&[client_profile, collections, kasumi_release])?;
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-lineage-data-service-originals\0");
        for original in [client_profile, collections, kasumi_release] {
            h.update(original);
        }
        Ok(h.finalize().into())
    }
    /// Shape check under the exact original financial asset scope; no financial grant is returned.
    /// # Errors
    /// Refuses another liability pool, absent service/incarnation or ambiguous namespace strings.
    pub fn validate_for_runtime(
        &self,
        runtime: &super::KagemushaRetailEnrollmentRuntimeV1,
    ) -> Result<(), String> {
        self.validate_for_pool(
            &runtime.network_id,
            &runtime.asset,
            runtime.asset_incarnation,
        )
    }
    /// Validate the pool selected by actual Node network and asset registry incarnation.
    /// This deterministic data check supplies no World permission or DATA session authority.
    /// # Errors
    /// Refuses malformed shape or substitution of the exclusive liability pool.
    pub fn validate_for_pool(
        &self,
        network: &crate::NetworkId,
        asset: &crate::asset::AssetDefinitionId,
        incarnation: crate::nexus::AxtAssetIncarnationV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        if self.liability_pool_id
            != super::kagemusha_liability_pool_id_v1(network, asset, incarnation)
                .map_err(|error| error.to_string())?
        {
            return Err("ordinary lineage authoritative DATA pool differs".into());
        }
        Ok(())
    }
    /// Validate finite exact authority data; installation and current World remain separate gates.
    /// # Errors
    /// Refuses unsupported version, missing original identity or ambiguous namespace spelling.
    pub fn validate_shape(&self) -> Result<(), String> {
        nonzero(&[
            self.liability_pool_id,
            self.service_identity_digest,
            self.data_incarnation_digest,
        ])?;
        let name = |s: &str| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        };
        if self.version != 1
            || ![
                &self.dataspace,
                &self.tenant,
                &self.principal,
                &self.collection,
            ]
            .into_iter()
            .all(|s| name(s))
        {
            return Err("ordinary lineage authoritative DATA domain differs".into());
        }
        Ok(())
    }
    /// Require this exact full value in the current certified `AssetDefinition` metadata.
    /// The caller must independently verify the complete `AssetDefinition` against actual World.
    /// No raw metadata value or supplied `AssetDefinition` can select service authority.
    /// # Errors
    /// Refuses absence, malformed value or any service/incarnation/namespace substitution.
    pub fn require_asset_definition_metadata(
        &self,
        definition: &crate::asset::AssetDefinition,
    ) -> Result<(), String> {
        let key = KAGEMUSHA_ORDINARY_LINEAGE_DATA_AUTHORITY_METADATA_KEY_V1
            .parse::<iroha_model_base::name::Name>()
            .map_err(|error| error.to_string())?;
        let raw = definition.metadata.get(&key).ok_or_else(|| {
            "ordinary lineage DATA authority is absent from current asset World".to_owned()
        })?;
        let actual: Self = raw
            .try_into_any_norito()
            .map_err(|error| error.to_string())?;
        if actual != *self {
            return Err("ordinary lineage DATA World authority was substituted".into());
        }
        Ok(())
    }
    /// Exact complete bounded original; a hash is never a substitute for installed/World custody.
    /// # Errors
    /// Refuses unsupported version, missing identity or unsupported canonical framing.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
}

/// Original financial lineage, unchanged by refreshed FI nonce, PI lease or app session.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryFinancialLineageV1")]
pub struct KagemushaOrdinaryFinancialLineageV1 {
    /// Sole version.
    pub version: u16,
    /// Exact unchanged account/FI/network/asset incarnation/lane.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Original ordinary financial epoch identity, selected from the original admitted C.
    pub financial_epoch_id: [u8; 32],
    /// Original Native financial-secret commitment, never an app's offered key or current nonce.
    pub financial_authority_commitment: [u8; 32],
}
impl KagemushaOrdinaryFinancialLineageV1 {
    /// Validate data shape only; authority requires actual original credential/State admission.
    /// # Errors
    /// Refuses unsupported version or missing original selectors/scope.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.owner.enrollment_id().map_err(|e| e.to_string())?;
        if self.version != 1
            || self.financial_epoch_id == [0; 32]
            || self.financial_authority_commitment == [0; 32]
        {
            return Err("ordinary financial lineage shape differs".into());
        }
        Ok(())
    }
    /// Immutable key for the actual global DATA head. Refresh never changes this key.
    /// # Errors
    /// Refuses invalid original data or unsupported canonical encoding.
    pub fn id(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-financial-lineage\0", self)
    }
}
/// Complete financial head identity; the SHA binds the entire exact canonical State original.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryFinancialHeadV1")]
pub struct KagemushaOrdinaryFinancialHeadV1 {
    /// Actual State commitment verified in both ordinary field parities.
    pub state_commitment: [u8; 32],
    /// Actual State sequence.
    pub logical_sequence: u128,
    /// SHA of the complete exact State original, not an offered compact public projection.
    pub state_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryFinancialHeadV1 {
    /// Validate shape only.
    /// # Errors
    /// Refuses missing full-original commitment.
    pub fn validate_shape(&self) -> Result<(), String> {
        nonzero(&[self.state_commitment, self.state_original_sha256])
    }
}
/// Immutable pre-W2 selector. It excludes W2, prepared-ID, candidate and CAS-result digests,
/// so neither the local neutral reservation nor State preparation depends on its own receipt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageOperationSelectionV1")]
pub struct KagemushaOrdinaryLineageOperationSelectionV1 {
    /// Actual same immutable financial lineage.
    pub lineage: KagemushaOrdinaryFinancialLineageV1,
    /// Sole original Native operation identifier.
    pub operation_id: [u8; 32],
    /// Exact predecessor observed before preparation.
    pub predecessor: KagemushaOrdinaryFinancialHeadV1,
    /// Selected actual monetary operation; Bootstrap is forbidden.
    pub operation: KagemushaOperationKindV1,
    /// Actual amount; State arithmetic must independently verify this value.
    pub amount: u128,
    /// Exact asset scale from the original lineage.
    pub scale: u32,
    /// Full receiver request SHA for Send; non-Send uses the model's purpose-bound zero selector.
    pub receiver_request_original_sha256: [u8; 32],
    /// Complete pre-W2 output body SHA; committed by the actual selected State/Guard relation.
    pub output_body_original_sha256: [u8; 32],
    /// Full genuine Native neutral `OutboxReservation` original SHA.
    pub neutral_reservation_original_sha256: [u8; 32],
    /// Model-owned neutral reservation digest used by actual purpose2 S/W.
    pub neutral_reservation_digest: [u8; 32],
}
impl KagemushaOrdinaryLineageOperationSelectionV1 {
    /// Validate closed ordinary money selector data without admitting proofs.
    /// # Errors
    /// Refuses Bootstrap, wrong scale, missing selectors or operation-specific receiver slots.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.lineage.validate_shape()?;
        self.predecessor.validate_shape()?;
        nonzero(&[
            self.operation_id,
            self.output_body_original_sha256,
            self.neutral_reservation_original_sha256,
            self.neutral_reservation_digest,
        ])?;
        if self.amount == 0
            || self.scale != self.lineage.owner.runtime.scale
            || !matches!(
                self.operation,
                KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
            )
        {
            return Err("ordinary lineage operation/amount/scale differs".into());
        }
        if (self.operation == KagemushaOperationKindV1::SendSplit)
            != (self.receiver_request_original_sha256 != [0; 32])
        {
            return Err("ordinary lineage receiver selector differs".into());
        }
        Ok(())
    }
    /// Stable reservation selector fixed before W2. No receipt or proof digest is included.
    /// # Errors
    /// Refuses malformed selectors or noncanonical serialization.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(
            b"iroha:kagemusha:v1:ordinary-lineage-operation-selection\0",
            self,
        )
    }
}
/// Post-W2 ordinary preparation proof admission data. Native/ Core must verify every full
/// original and both field proofs; these digests alone do not construct an admitted candidate.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageReservationV1")]
pub struct KagemushaOrdinaryLineageReservationV1 {
    /// Immutable pre-W2 selector.
    pub selection: KagemushaOrdinaryLineageOperationSelectionV1,
    /// Exact successor independently joined by whole State preparation admission.
    pub successor: KagemushaOrdinaryFinancialHeadV1,
    /// Full purpose2 W original SHA.
    pub purpose2_approval_original_sha256: [u8; 32],
    /// Complete exact public State candidate projection original SHA; private balances are excluded.
    pub candidate_original_sha256: [u8; 32],
    /// Complete purpose-bound stateless predecessor/State/Guard proof-bundle SHA.
    pub proof_bundle_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryLineageReservationV1 {
    /// Validate data joins only; no CAS, proof or native loan is admitted here.
    /// # Errors
    /// Refuses missing originals, unchanged State or an invalid sequence edge.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.selection.validate_shape()?;
        self.successor.validate_shape()?;
        nonzero(&[
            self.purpose2_approval_original_sha256,
            self.candidate_original_sha256,
            self.proof_bundle_original_sha256,
        ])?;
        if self.successor.logical_sequence
            != self
                .selection
                .predecessor
                .logical_sequence
                .checked_add(1)
                .ok_or("ordinary lineage sequence overflow")?
            || self.successor.state_commitment == self.selection.predecessor.state_commitment
        {
            return Err("ordinary lineage successor differs".into());
        }
        Ok(())
    }
    /// Exact full reservation original identity.
    /// # Errors
    /// Refuses malformed data or canonical serialization failure.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-lineage-reservation\0", self)
    }
}
/// Whole terminal/public-envelope selector after purpose1 capture. The global commit atomically
/// advances the exact reserved head and consumes the immutable transition nullifier.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageCommitV1")]
pub struct KagemushaOrdinaryLineageCommitV1 {
    /// Full exact held reservation, including its immutable selection/successor.
    pub reservation: KagemushaOrdinaryLineageReservationV1,
    /// Actual predecessor conflict/nullifier from the ordinary whole Terminal relation.
    pub transition_nullifier: [u8; 32],
    /// Full purpose1 approval original SHA.
    pub purpose1_approval_original_sha256: [u8; 32],
    /// Full logical terminal record original SHA.
    pub terminal_record_original_sha256: [u8; 32],
    /// Complete original paired Terminal proofs SHA.
    pub terminal_proofs_original_sha256: [u8; 32],
    /// Complete original paired physical Wrapper proofs SHA.
    pub wrapper_proofs_original_sha256: [u8; 32],
    /// Complete selected pre-receipt output/envelope original SHA. The later global receipt is
    /// outside this selector; the envelope never commits to its own receipt.
    pub outgoing_original_sha256: [u8; 32],
    /// Exact acknowledged signed FI control original at W1 capture, separate from W2 capture.
    pub purpose1_financial_control_original_sha256: [u8; 32],
    /// Full immutable W1 Native interval clock context original SHA.
    pub purpose1_clock_context_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryLineageCommitV1 {
    /// Validate data shape only.
    /// # Errors
    /// Refuses malformed reservation or absent full original selectors.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.reservation.validate_shape()?;
        nonzero(&[
            self.transition_nullifier,
            self.purpose1_approval_original_sha256,
            self.terminal_record_original_sha256,
            self.terminal_proofs_original_sha256,
            self.wrapper_proofs_original_sha256,
            self.outgoing_original_sha256,
            self.purpose1_financial_control_original_sha256,
            self.purpose1_clock_context_original_sha256,
        ])
    }
    /// Complete canonical commit selector digest; supplied proof originals are checked separately.
    /// # Errors
    /// Refuses malformed data or canonical serialization failure.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-lineage-commit\0", self)
    }
}
/// Exact globally admitted zero-State/Guard anchor data. The actual paired proof admission is separate.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageAnchorV1")]
pub struct KagemushaOrdinaryLineageAnchorV1 {
    /// Exact immutable original financial lineage.
    pub lineage: KagemushaOrdinaryFinancialLineageV1,
    /// Actual whole-proof joined zero financial head.
    pub initial_head: KagemushaOrdinaryFinancialHeadV1,
    /// SHA of every full canonical State/Guard proof-bundle original.
    pub proof_bundle_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryLineageAnchorV1 {
    /// Validate data only; sequence0 is necessary and never sufficient for zero-State admission.
    /// # Errors
    /// Refuses missing proof original, malformed lineage or nonzero sequence.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.lineage.validate_shape()?;
        self.initial_head.validate_shape()?;
        nonzero(&[self.proof_bundle_original_sha256])?;
        if self.initial_head.logical_sequence != 0 {
            return Err("ordinary lineage anchor is not zero sequence".into());
        }
        Ok(())
    }
}
/// Data operation carried by a Native account-signed CAS request.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageRequestOperationV1")]
pub enum KagemushaOrdinaryLineageRequestOperationV1 {
    /// First globally admitted exact zero State/Guard anchor; sequence must be zero.
    Anchor(Box<KagemushaOrdinaryLineageAnchorV1>),
    /// Reserve the exact DATA predecessor after stateless genuine W2 candidate admission.
    Reserve(Box<KagemushaOrdinaryLineageReservationV1>),
    /// Commit whole terminal evidence and atomically advance the sole global head.
    Commit(Box<KagemushaOrdinaryLineageCommitV1>),
    /// Reserve an exact source credit and predecessor for ordinary Mint/Receive intake.
    /// Mint issuance requires a pre-debit pending selector and actual finalized funding admission.
    ReserveIncoming(Box<super::KagemushaOrdinaryIncomingReservationV1>),
    /// Commit actual ordinary incoming State/replay proof and atomically consume credit/advance head.
    CommitIncoming(Box<super::KagemushaOrdinaryIncomingCommitV1>),
}
impl KagemushaOrdinaryLineageRequestOperationV1 {
    /// Actual financial lineage referred to by every operation.
    #[must_use]
    pub fn lineage(&self) -> &KagemushaOrdinaryFinancialLineageV1 {
        match self {
            Self::Anchor(anchor) => &anchor.lineage,
            Self::Reserve(r) => &r.selection.lineage,
            Self::Commit(c) => &c.reservation.selection.lineage,
            Self::ReserveIncoming(r) => &r.selection.lineage,
            Self::CommitIncoming(c) => &c.reservation.selection.lineage,
        }
    }
    /// Validate data-only operation shape.
    /// # Errors
    /// Refuses nonzero initial sequence or malformed financial selectors.
    pub fn validate_shape(&self) -> Result<(), String> {
        match self {
            Self::Anchor(anchor) => anchor.validate_shape(),
            Self::Reserve(r) => r.validate_shape(),
            Self::Commit(c) => c.validate_shape(),
            Self::ReserveIncoming(r) => r.validate_shape(),
            Self::CommitIncoming(c) => c.validate_shape(),
        }
    }
}
/// Actual Native account-signed bounded request. Signature is consent only, not proof/CAS custody.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageRequestV1")]
pub struct KagemushaOrdinaryLineageRequestV1 {
    /// Sole version.
    pub version: u16,
    /// Fresh real Native nonce, retained with the full request before signing/HTTP dispatch.
    pub request_nonce: [u8; 32],
    /// Exact independently installed current issuer policy identity.
    pub issuer_policy_digest: [u8; 32],
    /// Complete immutable CAS operation.
    pub operation: KagemushaOrdinaryLineageRequestOperationV1,
}
impl KagemushaOrdinaryLineageRequestV1 {
    /// Canonical original with bounded first-release shape.
    /// # Errors
    /// Refuses unsupported version, missing nonce/policy or invalid selectors.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        if self.version != 1 {
            return Err("ordinary lineage request version differs".into());
        }
        nonzero(&[self.request_nonce, self.issuer_policy_digest])?;
        self.operation.validate_shape()?;
        bounded(self)
    }
    /// Purpose-bound account signing bytes; this is not a read-only FI request.
    /// # Errors
    /// Refuses invalid request shape.
    pub fn account_signing_message(&self) -> Result<Vec<u8>, String> {
        Ok(message(
            KAGEMUSHA_ORDINARY_LINEAGE_REQUEST_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
    /// Verify exact single-member Ed account consent, matching the actual Native wallet selection.
    /// # Errors
    /// Refuses multisig/weight/threshold/algorithm drift or another signing subject.
    pub fn verify_account_signature(&self, signature: &Signature) -> Result<(), String> {
        let owner = &self.operation.lineage().owner;
        let policy = owner
            .account_id
            .multisig_policy()
            .ok_or("ordinary lineage W unsupported")?;
        let member = policy
            .members()
            .first()
            .ok_or("ordinary lineage account signer absent")?;
        if policy.threshold() != 1
            || policy.members().len() != 1
            || member.weight() != 1
            || member.public_key().algorithm() != Algorithm::Ed25519
            || signature.payload().len() != 64
        {
            return Err("ordinary lineage account signature shape differs".into());
        }
        signature
            .verify(member.public_key(), &self.account_signing_message()?)
            .map_err(|_| "ordinary lineage account signature rejected".into())
    }
}
/// Current issuer assertion only after genuine durable global CAS under actual admitted DATA
/// custody. It is not a Taira DATA Merkle proof: the separate Native current policy cut and real
/// current receipt read authenticate its scope. Receiver admission must check the exact receipt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageResultSubjectV1")]
pub struct KagemushaOrdinaryLineageResultSubjectV1 {
    /// Exact unchanged Native request. The result signs all financial selectors and proof SHAs.
    pub request: KagemushaOrdinaryLineageRequestV1,
    /// Exact authenticated Native DATA incarnation digest.
    pub data_incarnation_digest: [u8; 32],
    /// Actual admitted DATA revision containing the immutable result record.
    pub data_revision: u64,
    /// Actual selected DATA policy generation.
    pub data_policy_epoch: u64,
    /// Actual selected DATA schema generation.
    pub data_schema_epoch: u64,
    /// Complete exact immutable DATA result-record original SHA.
    pub data_record_original_sha256: [u8; 32],
    /// Current independently certified Taira policy/release cut height.
    pub authority_height: u64,
    /// Exact certified current Taira context ID.
    pub authority_context_id: [u8; 32],
    /// Exact current Taira World root; authenticates current financial policy, not DATA documents.
    pub authority_world_root: [u8; 32],
    /// Exact independently installed public policy original admitting this signing purpose.
    pub cas_policy_digest: [u8; 32],
    /// Actual current admitting ordinary release.
    pub release_id: [u8; 32],
    /// Current issuer issue time after the durably observed DATA CAS.
    pub issued_at_ms: u64,
}
impl KagemushaOrdinaryLineageResultSubjectV1 {
    /// Exact result original binding, purpose-bound and independent of read-only FI status.
    /// # Errors
    /// Refuses missing real DATA/policy observations or malformed request.
    pub fn issuer_signing_message(&self) -> Result<Vec<u8>, String> {
        self.request.canonical_bytes()?;
        nonzero(&[
            self.data_incarnation_digest,
            self.data_record_original_sha256,
            self.authority_context_id,
            self.authority_world_root,
            self.cas_policy_digest,
            self.release_id,
        ])?;
        if [
            self.data_revision,
            self.data_policy_epoch,
            self.data_schema_epoch,
            self.authority_height,
            self.issued_at_ms,
        ]
        .contains(&0)
        {
            return Err("ordinary lineage result lacks actual committed context".into());
        }
        Ok(message(
            KAGEMUSHA_ORDINARY_LINEAGE_RESULT_DOMAIN_V1,
            &bounded(self)?,
        ))
    }
}
/// Complete purpose-bound result original. Public signature verification still supplies neither
/// actual pending Native reservation custody nor a new current monetary loan.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryLineageResultV1")]
pub struct KagemushaSignedOrdinaryLineageResultV1 {
    /// Complete exact subject.
    pub subject: KagemushaOrdinaryLineageResultSubjectV1,
    /// Actual original Ed issuer signature, distinct from app platform approval.
    pub signature: Signature,
}
impl KagemushaSignedOrdinaryLineageResultV1 {
    /// Verify exact request and purpose-bound issuer signature only.
    /// # Errors
    /// Refuses any substituted request/key/shape/signature; Native separately verifies context.
    pub fn verify_for_request(
        &self,
        expected: &KagemushaOrdinaryLineageRequestV1,
        key: &PublicKey,
    ) -> Result<(), String> {
        if self.subject.request != *expected
            || key.algorithm() != Algorithm::Ed25519
            || self.signature.payload().len() != 64
        {
            return Err("ordinary lineage result request/key differs".into());
        }
        self.signature
            .verify(key, &self.subject.issuer_signing_message()?)
            .map_err(|_| "ordinary lineage issuer signature rejected".into())
    }
    /// Sole complete canonical signed original.
    /// # Errors
    /// Refuses malformed original shape or oversized canonical bytes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.issuer_signing_message()?;
        if self.signature.payload().len() != 64 {
            return Err("ordinary lineage signature width differs".into());
        }
        bounded(self)
    }
}
fn nonzero(fields: &[[u8; 32]]) -> Result<(), String> {
    if fields.contains(&[0; 32]) {
        return Err("ordinary lineage original selector absent".into());
    }
    Ok(())
}
fn bounded<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1 {
        return Err("ordinary lineage original oversized".into());
    }
    Ok(bytes)
}
fn message(domain: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut out = domain.to_vec();
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}
fn digest<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> Result<[u8; 32], String> {
    Ok(Sha256::digest(message(domain, &bounded(value)?)).into())
}

/// Public signing-purpose original admitted by the independently installed runtime inventory.
/// It uses the existing FI/Core key and scope; it is neither a second FI certificate nor a
/// self-authorizing model factory. The Native installer must authenticate its retained original
/// descriptor under the independently held runtime owner before enabling any CAS endpoint.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryLineageIssuerPolicyV1")]
pub struct KagemushaOrdinaryLineageIssuerPolicyV1 {
    /// Sole version.
    pub version: u16,
    /// Exact existing independent FI policy original identity.
    pub issuer_policy_digest: [u8; 32],
    /// Exact existing FI issuer key; no app may select this key.
    pub issuer_public_key: PublicKey,
    /// Exact existing independently admitted FI/runtime/network/asset incarnation tuple.
    pub runtime: super::KagemushaRetailEnrollmentRuntimeV1,
    /// Purpose fingerprint for Anchor0/Reserve1/Commit2/ReserveIncoming3/CommitIncoming4
    /// plus their exact request/result domains. The first-release signed policy must admit all five.
    pub purpose_domain_digest: [u8; 32],
    /// Complete independently governed exclusive DATA domain for the original liability pool.
    pub data_authority: KagemushaOrdinaryLineageDataAuthorityV1,
    /// Explicit owner-admitted financial CAS authorization. Qualified source requires true.
    pub enabled: bool,
}
impl KagemushaOrdinaryLineageIssuerPolicyV1 {
    /// Exact purpose fingerprint; no wildcard or generic FI-status purpose is admitted.
    #[must_use]
    pub fn purpose_domain_digest() -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-lineage-cas-admitted-purposes\0");
        for bytes in [
            KAGEMUSHA_ORDINARY_LINEAGE_REQUEST_DOMAIN_V1,
            KAGEMUSHA_ORDINARY_LINEAGE_RESULT_DOMAIN_V1,
        ] {
            h.update((bytes.len() as u64).to_le_bytes());
            h.update(bytes);
        }
        h.update([0, 1, 2, 3, 4]);
        h.update(b"exclusive-data-authority-v1\0");
        h.finalize().into()
    }
    /// Check shape and exact role/scope joins to an independently selected original FI policy.
    /// Descriptor authentication under the installed owner is mandatory separately.
    /// # Errors
    /// Refuses disabled purpose, another key/scope/role or malformed issuer policy.
    pub fn validate_for_issuer(
        &self,
        issuer: &super::KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        issuer.validate().map_err(|error| error.to_string())?;
        self.data_authority.validate_for_runtime(&self.runtime)?;
        if self.version != 1
            || !self.enabled
            || self.issuer_public_key.algorithm() != Algorithm::Ed25519
            || self.issuer_public_key != issuer.issuer_public_key
            || self.runtime != issuer.runtime
            || self.issuer_policy_digest
                != super::kagemusha_ordinary_retail_issuer_policy_digest_v1(issuer)?
            || self.purpose_domain_digest != Self::purpose_domain_digest()
        {
            return Err("ordinary lineage signing purpose is not admitted".into());
        }
        Ok(())
    }
    /// Complete canonical public policy original. Identity alone establishes no installed authority.
    /// # Errors
    /// Refuses unsupported version or absent original purpose identity.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        if self.version != 1 || self.purpose_domain_digest != Self::purpose_domain_digest() {
            return Err("ordinary lineage purpose identity differs".into());
        }
        self.data_authority.validate_for_runtime(&self.runtime)?;
        nonzero(&[self.issuer_policy_digest])?;
        bounded(self)
    }
    /// Immutable exact full policy digest used in every signed CAS result.
    /// # Errors
    /// Refuses malformed public original.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(message(
            b"iroha:kagemusha:v1:ordinary-lineage-cas-policy\0",
            &self.canonical_bytes()?,
        ))
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{KeyPair, Signature};
    fn fixture() -> (Fixture, KagemushaOrdinaryFinancialLineageV1) {
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let line = KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: f.selection.owner.clone(),
            financial_epoch_id: super::super::kagemusha_ordinary_financial_epoch_id_v1(
                &f.selection.issuance.credential.subject,
            )
            .unwrap(),
            financial_authority_commitment: f
                .selection
                .issuance
                .credential
                .subject
                .financial_authority_commitment,
        };
        (f, line)
    }
    fn head(sequence: u128, tag: u8) -> KagemushaOrdinaryFinancialHeadV1 {
        KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: [tag; 32],
            logical_sequence: sequence,
            state_original_sha256: [tag + 1; 32],
        }
    }
    fn reserve(
        lineage: KagemushaOrdinaryFinancialLineageV1,
    ) -> KagemushaOrdinaryLineageReservationV1 {
        KagemushaOrdinaryLineageReservationV1 {
            selection: KagemushaOrdinaryLineageOperationSelectionV1 {
                scale: lineage.owner.runtime.scale,
                lineage,
                operation_id: [20; 32],
                predecessor: head(0, 21),
                operation: KagemushaOperationKindV1::SendSplit,
                amount: 17,
                receiver_request_original_sha256: [22; 32],
                output_body_original_sha256: [23; 32],
                neutral_reservation_original_sha256: [24; 32],
                neutral_reservation_digest: [25; 32],
            },
            successor: head(1, 26),
            purpose2_approval_original_sha256: [27; 32],
            candidate_original_sha256: [28; 32],
            proof_bundle_original_sha256: [29; 32],
        }
    }
    #[test]
    fn immutable_selector_excludes_later_w2_candidate_receipt_and_lineage_excludes_refresh_nonce() {
        let (_, line) = fixture();
        let mut r = reserve(line.clone());
        let selected = r.selection.digest().unwrap();
        let full = r.digest().unwrap();
        r.purpose2_approval_original_sha256[0] ^= 1;
        r.candidate_original_sha256[0] ^= 1;
        r.proof_bundle_original_sha256[0] ^= 1;
        assert_eq!(r.selection.digest().unwrap(), selected);
        assert_ne!(r.digest().unwrap(), full);
        let original = line.id().unwrap();
        r.selection.lineage.owner.runtime.scale += 1;
        assert_ne!(r.selection.lineage.id().unwrap(), original);
    }
    #[test]
    fn zero_anchor_and_reserved_head_edge_are_required_and_bootstrap_cannot_reserve_money() {
        let (_, line) = fixture();
        let mut anchor = KagemushaOrdinaryLineageAnchorV1 {
            lineage: line.clone(),
            initial_head: head(0, 30),
            proof_bundle_original_sha256: [31; 32],
        };
        anchor.validate_shape().unwrap();
        anchor.initial_head.logical_sequence = 1;
        assert!(anchor.validate_shape().is_err());
        let mut r = reserve(line);
        r.validate_shape().unwrap();
        r.successor.logical_sequence = 0;
        assert!(r.validate_shape().is_err());
        r.successor.logical_sequence = 1;
        r.selection.operation = KagemushaOperationKindV1::Bootstrap;
        assert!(r.validate_shape().is_err());
        r.selection.operation = KagemushaOperationKindV1::SendSplit;
        r.selection.receiver_request_original_sha256 = [0; 32];
        assert!(r.validate_shape().is_err());
        r.selection.operation = KagemushaOperationKindV1::RedeemSplit;
        r.validate_shape().unwrap();
        r.selection.amount = 0;
        assert!(r.validate_shape().is_err());
    }
    #[test]
    fn actual_account_consent_and_explicit_issuer_purpose_bind_all_original_data() {
        let (f, line) = fixture();
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        assert_eq!(issuer.public_key(), &f.issuer_policy.issuer_public_key);
        let policy = KagemushaOrdinaryLineageIssuerPolicyV1 {
            version: 1,
            issuer_policy_digest: super::super::kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.issuer_policy,
            )
            .unwrap(),
            issuer_public_key: issuer.public_key().clone(),
            runtime: f.issuer_policy.runtime.clone(),
            data_authority: KagemushaOrdinaryLineageDataAuthorityV1 {
                version: 1,
                liability_pool_id: super::super::kagemusha_liability_pool_id_v1(
                    &f.issuer_policy.runtime.network_id,
                    &f.issuer_policy.runtime.asset,
                    f.issuer_policy.runtime.asset_incarnation,
                )
                .unwrap(),
                service_identity_digest: [90; 32],
                data_incarnation_digest: [33; 32],
                dataspace: "mibank.bpng".into(),
                tenant: "mibank-core".into(),
                principal: "core-mibank".into(),
                collection: "retail_enrollments".into(),
            },
            purpose_domain_digest: KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            enabled: true,
        };
        policy.validate_for_issuer(&f.issuer_policy).unwrap();
        let mut disabled = policy.clone();
        disabled.enabled = false;
        assert!(disabled.validate_for_issuer(&f.issuer_policy).is_err());
        let mut request = KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [32; 32],
            issuer_policy_digest: policy.issuer_policy_digest,
            operation: KagemushaOrdinaryLineageRequestOperationV1::Reserve(Box::new(reserve(line))),
        };
        let signature = Signature::new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        );
        request.verify_account_signature(&signature).unwrap();
        let subject = KagemushaOrdinaryLineageResultSubjectV1 {
            request: request.clone(),
            data_incarnation_digest: [33; 32],
            data_revision: 12,
            data_policy_epoch: 1,
            data_schema_epoch: 1,
            data_record_original_sha256: [34; 32],
            authority_height: 13,
            authority_context_id: [35; 32],
            authority_world_root: [36; 32],
            cas_policy_digest: policy.digest().unwrap(),
            release_id: f.release.release_id(),
            issued_at_ms: 1400,
        };
        let signed = KagemushaSignedOrdinaryLineageResultV1 {
            signature: Signature::new(
                issuer.private_key(),
                &subject.issuer_signing_message().unwrap(),
            ),
            subject,
        };
        signed
            .verify_for_request(&request, issuer.public_key())
            .unwrap();
        let app_authority = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        assert_eq!(app_authority.public_key(), &f.app_authority.authority_key);
        let mut wrong_role = signed.clone();
        wrong_role.signature = Signature::new(
            app_authority.private_key(),
            &wrong_role.subject.issuer_signing_message().unwrap(),
        );
        assert!(
            wrong_role
                .verify_for_request(&request, &f.issuer_policy.issuer_public_key)
                .is_err()
        );
        let mut wrong_policy = policy.clone();
        wrong_policy.issuer_public_key = app_authority.public_key().clone();
        assert!(wrong_policy.validate_for_issuer(&f.issuer_policy).is_err());
        request.request_nonce[0] ^= 1;
        assert!(request.verify_account_signature(&signature).is_err());
        assert!(
            signed
                .verify_for_request(&request, issuer.public_key())
                .is_err()
        );
    }
    #[test]
    fn full_original_canonical_roundtrip_refuses_trailing_frame_and_key_role_change() {
        let (f, line) = fixture();
        let request = KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [32; 32],
            issuer_policy_digest: super::super::kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.issuer_policy,
            )
            .unwrap(),
            operation: KagemushaOrdinaryLineageRequestOperationV1::Reserve(Box::new(reserve(line))),
        };
        let bytes = request.canonical_bytes().unwrap();
        let actual: KagemushaOrdinaryLineageRequestV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .unwrap();
        assert_eq!(actual, request);
        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            norito::decode_canonical_with_limits::<KagemushaOrdinaryLineageRequestV1>(
                &trailing,
                norito::canonical_decode_limits(trailing.len())
            )
            .is_err()
        );
        let wrong = KeyPair::from_seed(vec![71; 32], Algorithm::Ed25519);
        assert!(
            request
                .verify_account_signature(&Signature::new(
                    wrong.private_key(),
                    &request.account_signing_message().unwrap()
                ))
                .is_err()
        );
    }
}

#[cfg(test)]
mod incoming_purpose_tests {
    use super::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    #[test]
    fn ordinary_lineage_signing_purpose_requires_all_five_exact_first_release_operations() {
        let calculate = |tags: &[u8]| {
            let mut h = Sha256::new();
            h.update(b"iroha:kagemusha:v1:ordinary-lineage-cas-admitted-purposes\0");
            for domain in [
                KAGEMUSHA_ORDINARY_LINEAGE_REQUEST_DOMAIN_V1,
                KAGEMUSHA_ORDINARY_LINEAGE_RESULT_DOMAIN_V1,
            ] {
                h.update((domain.len() as u64).to_le_bytes());
                h.update(domain);
            }
            h.update(tags);
            h.update(b"exclusive-data-authority-v1\0");
            <[u8; 32]>::from(h.finalize())
        };
        assert_eq!(
            KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            calculate(&[0, 1, 2, 3, 4])
        );
        assert_ne!(
            KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            calculate(&[0, 1, 2])
        );
        assert_ne!(
            KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            calculate(&[0, 1, 2, 4, 3])
        );
        // The former complete five-purpose fingerprint still lacks the new independently
        // governed exclusive DATA domain and must not admit the current signing policy.
        let mut previous = Sha256::new();
        previous.update(b"iroha:kagemusha:v1:ordinary-lineage-cas-admitted-purposes\0");
        for domain in [
            KAGEMUSHA_ORDINARY_LINEAGE_REQUEST_DOMAIN_V1,
            KAGEMUSHA_ORDINARY_LINEAGE_RESULT_DOMAIN_V1,
        ] {
            previous.update((domain.len() as u64).to_le_bytes());
            previous.update(domain);
        }
        previous.update([0, 1, 2, 3, 4]);
        assert_ne!(
            KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            <[u8; 32]>::from(previous.finalize())
        );
    }
    #[test]
    fn exclusive_data_authority_full_original_roundtrips_and_every_coordinate_changes_policy() {
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let data = KagemushaOrdinaryLineageDataAuthorityV1 {
            version: 1,
            liability_pool_id: super::super::kagemusha_liability_pool_id_v1(
                &f.issuer_policy.runtime.network_id,
                &f.issuer_policy.runtime.asset,
                f.issuer_policy.runtime.asset_incarnation,
            )
            .unwrap(),
            service_identity_digest: [90; 32],
            data_incarnation_digest: [91; 32],
            dataspace: "mibank.bpng".into(),
            tenant: "mibank-core".into(),
            principal: "core-mibank".into(),
            collection: "retail_enrollments".into(),
        };
        let raw = data.canonical_bytes().unwrap();
        let decoded: KagemushaOrdinaryLineageDataAuthorityV1 =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        assert_eq!(decoded, data);
        let policy = KagemushaOrdinaryLineageIssuerPolicyV1 {
            version: 1,
            issuer_policy_digest: super::super::kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &f.issuer_policy,
            )
            .unwrap(),
            issuer_public_key: f.issuer_policy.issuer_public_key.clone(),
            runtime: f.issuer_policy.runtime.clone(),
            purpose_domain_digest: KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            data_authority: data.clone(),
            enabled: true,
        };
        policy.validate_for_issuer(&f.issuer_policy).unwrap();
        let original_digest = policy.digest().unwrap();
        for coordinate in 0..7 {
            let mut changed = policy.clone();
            let changed_data = &mut changed.data_authority;
            match coordinate {
                0 => changed_data.service_identity_digest[0] ^= 1,
                1 => changed_data.data_incarnation_digest[0] ^= 1,
                2 => changed_data.dataspace.push('x'),
                3 => changed_data.tenant.push('x'),
                4 => changed_data.principal.push('x'),
                5 => changed_data.collection.push('x'),
                _ => changed_data.liability_pool_id[0] ^= 1,
            }
            if coordinate == 6 {
                assert!(changed.validate_for_issuer(&f.issuer_policy).is_err());
            } else {
                assert_ne!(changed.digest().unwrap(), original_digest);
            }
        }
        let service =
            KagemushaOrdinaryLineageDataAuthorityV1::service_identity_digest_for_originals(
                [1; 32], [2; 32], [3; 32],
            )
            .unwrap();
        assert_ne!(
            service,
            KagemushaOrdinaryLineageDataAuthorityV1::service_identity_digest_for_originals(
                [2; 32], [1; 32], [3; 32]
            )
            .unwrap()
        );
        assert!(
            KagemushaOrdinaryLineageDataAuthorityV1::service_identity_digest_for_originals(
                [0; 32], [2; 32], [3; 32]
            )
            .is_err()
        );
        for invalid in [
            "",
            "../elsewhere",
            "tenant/name",
            "tenant%2ename",
            "tenant\n",
        ] {
            let mut changed = data.clone();
            changed.tenant = invalid.into();
            assert!(changed.canonical_bytes().is_err());
        }
    }
    #[test]
    fn exclusive_data_authority_requires_the_exact_typed_network_asset_and_incarnation() {
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let runtime = &f.issuer_policy.runtime;
        let data = KagemushaOrdinaryLineageDataAuthorityV1 {
            version: 1,
            liability_pool_id: super::super::kagemusha_liability_pool_id_v1(
                &runtime.network_id,
                &runtime.asset,
                runtime.asset_incarnation,
            )
            .unwrap(),
            service_identity_digest: [90; 32],
            data_incarnation_digest: [91; 32],
            dataspace: "mibank.bpng".into(),
            tenant: "mibank-core".into(),
            principal: "core-mibank".into(),
            collection: "retail_enrollments".into(),
        };
        data.validate_for_runtime(runtime).unwrap();
        data.validate_for_pool(
            &runtime.network_id,
            &runtime.asset,
            runtime.asset_incarnation,
        )
        .unwrap();
        let other_network =
            crate::NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"different-lineage-authority-genesis"),
            ));
        let other_asset = crate::asset::AssetDefinitionId::from_uuid_bytes([
            0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
            0xcd, 0x30,
        ])
        .unwrap();
        let other_incarnation = crate::nexus::AxtAssetIncarnationV1::try_from_bytes(
            *iroha_crypto::Hash::new(b"different-lineage-authority-incarnation").as_ref(),
        )
        .unwrap();
        for (network, asset, incarnation) in [
            (&other_network, &runtime.asset, runtime.asset_incarnation),
            (&runtime.network_id, &other_asset, runtime.asset_incarnation),
            (&runtime.network_id, &runtime.asset, other_incarnation),
        ] {
            assert_ne!(
                super::super::kagemusha_liability_pool_id_v1(network, asset, incarnation).unwrap(),
                data.liability_pool_id,
            );
            assert_eq!(
                data.validate_for_pool(network, asset, incarnation)
                    .unwrap_err(),
                "ordinary lineage authoritative DATA pool differs",
            );
        }
        let mut malformed = data;
        malformed.service_identity_digest = [0; 32];
        assert_eq!(
            malformed.validate_for_runtime(runtime).unwrap_err(),
            "ordinary lineage original selector absent",
        );
    }
}
