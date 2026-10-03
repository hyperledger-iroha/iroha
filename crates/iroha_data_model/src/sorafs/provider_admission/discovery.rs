//! Bounded current provider authority and advert evidence at one certified World cut.
//!
//! A historical admission write cannot prove that no later revocation occurred.
//! This projection therefore authenticates the complete current World snapshot,
//! then the exact head, predecessor and owner preimages. The recipient selects the
//! network, native schema and fresh certified decision independently. The response
//! carries no checkpoint and cannot choose its own trust root or freshness policy.

use super::{
    ProviderAdmissionCouncilPolicyV1,
    governance::{PROVIDER_ADMISSION_MAX_REVISIONS_V1, decode_frame},
    history::{AdmissionHistoryPathV1, AdmissionHistoryRecordV1, admission_history_path},
};
use crate::{
    account::AccountId,
    id::NetworkId,
    sorafs::capacity::ProviderId,
    sumeragi_finality::{
        FinalityError, VerifiedSumeragiBlock, VerifiedWorldStateSnapshotV1, WorldStateSnapshotV1,
    },
};
use iroha_crypto::Hash;
/// Exact current signer authority for the explicitly admitted account-read capability.
pub mod account_read;
/// Authenticated current signer control, including states awaiting enrollment.
pub mod stream_token_control;
use crate::sorafs::stream_token_custody::proof::{
    StreamTokenCustodyRecordProofRefV1, StreamTokenCustodyRecordProofV1, borrowed,
};
use sorafs_manifest::{
    AdmissionRecord, ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1,
    provider_admission::{
        ProviderAdmissionGenesisMaterialV1, compute_envelope_digest, verify_advert_against_record,
    },
    provider_advert::{ProviderAdvertV1, decode_provider_advert_v1},
};

/// Aggregate canonical response limit, independent of consensus validity.
pub const MAX_PROVIDER_DISCOVERY_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum original signed advert frame retained by this portable projection.
pub const MAX_PROVIDER_DISCOVERY_ADVERT_BYTES_V1: usize = 256 * 1024;

/// Exact current head and its immediate immutable predecessor, if one exists.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::discovery::ProviderAdmissionHeadProofV1"
)]
pub struct ProviderAdmissionHeadProofV1 {
    /// Original complete canonical `AdmissionHistoryRecordV1` bytes.
    pub head: Vec<u8>,
    /// Original preceding record bytes, absent only for revision one.
    #[norito(required)]
    pub predecessor: Option<Vec<u8>>,
}

/// Data-only provider discovery response; decoded values grant no authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_data_model::sorafs::provider_admission::discovery::ProviderDiscoveryProofV1"
)]
pub struct ProviderDiscoveryProofV1 {
    /// Complete canonical hash preimages of the same certified native World.
    pub world: WorldStateSnapshotV1,
    /// Current native council policy head and immediate predecessor.
    pub council: ProviderAdmissionHeadProofV1,
    /// Current native provider admission head and immediate predecessor.
    pub provider: ProviderAdmissionHeadProofV1,
    /// Exact native `world.provider_owners` row preimage.
    pub owner: AccountId,
    /// Complete canonical provider-signed advert, never an unauthenticated origin hint.
    pub advert: Vec<u8>,
    /// Current public native token custody; absence never authorizes account downloads.
    #[norito(required)]
    pub stream_token: Option<StreamTokenCustodyRecordProofV1>,
}

#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
struct HeadRef<'a> {
    head: borrowed::Vec<'a, u8>,
    predecessor: Option<borrowed::Vec<'a, u8>>,
}
impl norito::NoritoSchema for HeadRef<'_> {
    fn nominal_name() -> String {
        <ProviderAdmissionHeadProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <ProviderAdmissionHeadProofV1 as norito::NoritoSchema>::frame_name()
    }
}

/// Borrowed encoder for the sole discovery response layout, without cloning World graphs.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct ProviderDiscoveryProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    council: HeadRef<'a>,
    provider: HeadRef<'a>,
    owner: borrowed::Value<'a, AccountId>,
    advert: borrowed::Vec<'a, u8>,
    stream_token: Option<StreamTokenCustodyRecordProofRefV1<'a>>,
}
impl norito::NoritoSchema for ProviderDiscoveryProofRefV1<'_> {
    fn nominal_name() -> String {
        <ProviderDiscoveryProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <ProviderDiscoveryProofV1 as norito::NoritoSchema>::frame_name()
    }
}
impl<'a> ProviderDiscoveryProofRefV1<'a> {
    /// Borrow exact originals under the publisher's retained allocation budget.
    /// Construction grants no admission or finality authority.
    #[must_use]
    pub fn new(
        world: &'a WorldStateSnapshotV1,
        council: (&'a Vec<u8>, Option<&'a Vec<u8>>),
        provider: (&'a Vec<u8>, Option<&'a Vec<u8>>),
        owner: &'a AccountId,
        advert: &'a Vec<u8>,
        stream_token: Option<(&'a Vec<u8>, &'a Vec<u8>)>,
    ) -> Self {
        Self {
            world: borrowed::Value(world),
            council: HeadRef {
                head: borrowed::Vec(council.0),
                predecessor: council.1.map(borrowed::Vec),
            },
            provider: HeadRef {
                head: borrowed::Vec(provider.0),
                predecessor: provider.1.map(borrowed::Vec),
            },
            owner: borrowed::Value(owner),
            advert: borrowed::Vec(advert),
            stream_token: stream_token.map(|(head, record)| StreamTokenCustodyRecordProofRefV1 {
                head: borrowed::Vec(head),
                record: borrowed::Vec(record),
            }),
        }
    }
}

/// Current provider admission and signed advert authenticated at a caller-selected cut.
/// This does not grant stream-token issuance or authorize account spending.
#[derive(Debug, Clone)]
pub struct VerifiedProviderDiscoveryV1 {
    network_id: NetworkId,
    height: u64,
    context_id: Hash,
    owner: AccountId,
    admission: AdmissionRecord,
    advert: ProviderAdvertV1,
}

fn invalid(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}
fn map_invalid(_: impl std::fmt::Display) -> FinalityError {
    invalid("Provider discovery material is invalid")
}

impl ProviderAdmissionHeadProofV1 {
    fn verify(
        &self,
        world: &VerifiedWorldStateSnapshotV1,
        network: NetworkId,
        subject: Option<ProviderId>,
    ) -> Result<AdmissionHistoryRecordV1, FinalityError> {
        let head = AdmissionHistoryRecordV1::decode_frame(&self.head).map_err(map_invalid)?;
        if head.network_id != *network.as_bytes()
            || head.height == 0
            || head.height > world.height()
            || head.recorded_at_unix_ms == 0
            || head.recorded_at_unix_ms > world.block_time_ms()
            || head.revision == 0
            || head.revision > PROVIDER_ADMISSION_MAX_REVISIONS_V1
            || (head.revision == 1) != head.predecessor.is_none()
            || (head.revision == 1) != self.predecessor.is_none()
            || (head.genesis_origin.is_some() && (head.height != 1 || head.revision != 1))
        {
            return Err(invalid(
                "Provider discovery head is inconsistent with its certified cut",
            ));
        }
        world.verify_table_value(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Head),
            &self.head,
        )?;
        world.verify_table_value(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Revision(head.revision)),
            &self.head,
        )?;
        world.verify_smart_contract_state_absent(&admission_history_path(
            subject,
            AdmissionHistoryPathV1::Revision(head.revision + 1),
        ))?;
        if let Some(bytes) = &self.predecessor {
            let previous = AdmissionHistoryRecordV1::decode_frame(bytes).map_err(map_invalid)?;
            world.verify_table_value(
                "world.smart_contract_state",
                &admission_history_path(
                    subject,
                    AdmissionHistoryPathV1::Revision(head.revision - 1),
                ),
                bytes,
            )?;
            if Some(previous.canonical_digest().map_err(map_invalid)?) != head.predecessor
                || previous.revision.checked_add(1) != Some(head.revision)
                || previous.network_id != head.network_id
                || previous.height > head.height
                || previous.recorded_at_unix_ms > head.recorded_at_unix_ms
                || previous.revoked
            {
                return Err(invalid(
                    "Provider discovery predecessor differs from its native head",
                ));
            }
        }
        Ok(head)
    }
}

impl ProviderDiscoveryProofV1 {
    /// Decode the sole canonical response under fixed byte and allocation limits.
    /// # Errors
    /// Oversized, malformed, noncanonical or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        if bytes.len() > MAX_PROVIDER_DISCOVERY_BYTES_V1 {
            return Err(norito::core::Error::Message(
                "provider discovery exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                131_072,
                MAX_PROVIDER_DISCOVERY_BYTES_V1,
                131_072,
                128 * 1024 * 1024,
                64,
            ),
        )
    }

    /// Authenticate current authority and the exact signed advert at an independently selected cut.
    ///
    /// `expected_schema` must come from the installed qualified native registry,
    /// never from this response. The caller must first enforce its own finality
    /// freshness policy; an old certified block remains historical evidence.
    /// # Errors
    /// Wrong network, provider, schema or state; revoked/paused/expired admission;
    /// malformed lineage; altered advert, key, signature or resource bounds.
    pub fn verify(
        &self,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        expected_schema: Hash,
        block: &VerifiedSumeragiBlock,
        now_unix_seconds: u64,
    ) -> Result<VerifiedProviderDiscoveryV1, FinalityError> {
        if block.commitment().schedule.current.network_id != expected_network
            || self.world.schema_hash != expected_schema
            || self.advert.is_empty()
            || self.advert.len() > MAX_PROVIDER_DISCOVERY_ADVERT_BYTES_V1
            || norito::canonical_frame_len(self).map_err(map_invalid)?
                > MAX_PROVIDER_DISCOVERY_BYTES_V1
        {
            return Err(invalid(
                "Provider discovery network, schema or resource bound differs",
            ));
        }
        let world = self.world.authenticate(block)?;
        let council = self.council.verify(&world, expected_network, None)?;
        let provider = self
            .provider
            .verify(&world, expected_network, Some(expected_provider))?;
        let policy: ProviderAdmissionCouncilPolicyV1 =
            decode_frame(&council.material).map_err(map_invalid)?;
        policy.validate().map_err(map_invalid)?;
        if let Some(previous) = &self.council.predecessor {
            let previous = AdmissionHistoryRecordV1::decode_frame(previous).map_err(map_invalid)?;
            let previous: ProviderAdmissionCouncilPolicyV1 =
                decode_frame(&previous.material).map_err(map_invalid)?;
            policy.validate_successor(&previous).map_err(map_invalid)?;
        }
        if council.revoked
            || council.owner.is_some()
            || policy.paused
            || policy.network_id != council.network_id
            || policy.revision != council.revision
            || provider.revoked
            || provider.owner.as_ref() != Some(&self.owner)
        {
            return Err(invalid(
                "Provider discovery admission is revoked, paused or inconsistent",
            ));
        }
        world.verify_table_value("world.provider_owners", &expected_provider, &self.owner)?;
        let envelope: ProviderAdmissionEnvelopeV1 =
            decode_frame(&provider.material).map_err(map_invalid)?;
        if envelope.proposal.provider_id != *expected_provider.as_bytes()
            || envelope.admission_revision != provider.revision
            || now_unix_seconds < envelope.issued_at
            || now_unix_seconds.max(world.block_time_ms() / 1000) >= envelope.retention_epoch
        {
            return Err(invalid(
                "Provider discovery admission identity or validity differs",
            ));
        }
        if let Some(previous) = &self.provider.predecessor {
            let previous = AdmissionHistoryRecordV1::decode_frame(previous).map_err(map_invalid)?;
            let previous: ProviderAdmissionEnvelopeV1 =
                decode_frame(&previous.material).map_err(map_invalid)?;
            if envelope.expected_current_event_digest
                != Some(compute_envelope_digest(&previous).map_err(map_invalid)?)
            {
                return Err(invalid(
                    "Provider discovery renewal does not bind its predecessor",
                ));
            }
        }
        let admission = if provider.genesis_origin.is_some() {
            // The exact native head was authenticated above under the qualified
            // schema and original genesis-bound certified execution. Reconstruct
            // only its canonical genesis material; never claim council signatures.
            let material = ProviderAdmissionGenesisMaterialV1 {
                proposal: envelope.proposal.clone(),
                advert_body: envelope.advert_body.clone(),
                issued_at: envelope.issued_at,
                retention_epoch: envelope.retention_epoch,
            };
            let record = AdmissionRecord::from_genesis_material(
                &material,
                policy.network_id,
                policy.policy_id,
                policy.canonical_digest().map_err(map_invalid)?,
            )
            .map_err(map_invalid)?;
            if record.envelope() != &envelope || policy.revision != 1 {
                return Err(invalid(
                    "Provider discovery genesis material differs from its native head",
                ));
            }
            record
        } else {
            policy
                .verify_envelope_policy_claim(&envelope, now_unix_seconds)
                .map_err(map_invalid)?;
            let council = ProviderAdmissionCouncilPolicy::new(
                policy.trusted_signers.iter().copied(),
                usize::from(policy.signature_threshold),
            )
            .map_err(map_invalid)?;
            let digest = compute_envelope_digest(&envelope).map_err(map_invalid)?;
            AdmissionRecord::from_retained_envelope(envelope, &council, digest)
                .map_err(map_invalid)?
        };
        let advert = decode_provider_advert_v1(&self.advert).map_err(map_invalid)?;
        if !advert.signature_strict {
            return Err(invalid("Provider discovery requires a signed advert"));
        }
        advert
            .validate_with_body(now_unix_seconds.max(world.block_time_ms() / 1000))
            .map_err(map_invalid)?;
        advert.verify_signature().map_err(map_invalid)?;
        verify_advert_against_record(&advert, &admission).map_err(map_invalid)?;
        Ok(VerifiedProviderDiscoveryV1 {
            network_id: expected_network,
            height: block.height(),
            context_id: block.context_id(),
            owner: self.owner.clone(),
            admission,
            advert,
        })
    }
}

impl VerifiedProviderDiscoveryV1 {
    /// Independently selected genesis-bound network.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Height of the exact authenticated current-state projection.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Native certified decision identity at that height.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Exact current native provider owner.
    #[must_use]
    pub fn owner(&self) -> &AccountId {
        &self.owner
    }
    /// Exact current admission envelope, including its authorized advert key/body.
    #[must_use]
    pub fn admission(&self) -> &AdmissionRecord {
        &self.admission
    }
    /// Provider-signed advert bound to the same authenticated admission.
    #[must_use]
    pub fn advert(&self) -> &ProviderAdvertV1 {
        &self.advert
    }
}

#[cfg(test)]
mod tests;
