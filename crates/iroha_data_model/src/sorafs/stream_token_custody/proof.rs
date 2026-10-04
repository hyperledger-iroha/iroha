//! Native custody presence and absence at an independently selected certified World cut.
//!
//! This public evidence supports Configure and Enroll preparation before provider admission.
//! It carries no trust root and grants no enrollment, current-use, token or spending authority.

use super::{
    STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1, STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1,
    StreamTokenCustodyControlRecordV1,
    history::{StreamTokenCustodyControlIndexV1, head_key, height_key, record_key},
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
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::{SignerPurposeBindingV1, SignerRoleV1},
};

/// Aggregate original response bound; exceeding it is not a consensus invalidity verdict.
pub const MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;

fn invalid(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}
fn map_invalid(error: impl std::fmt::Display) -> FinalityError {
    FinalityError(error.to_string())
}

/// Exact original current native custody index and control record.
/// Token bodies and signer operation/audit journals are not part of this projection.
#[derive(
    Clone,
    Debug,
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
    name = "iroha_data_model::sorafs::stream_token_custody::proof::StreamTokenCustodyRecordProofV1"
)]
pub struct StreamTokenCustodyRecordProofV1 {
    /// Original complete canonical current-head index frame.
    pub head: Vec<u8>,
    /// Original complete canonical control record, including its enrolled public attestation.
    pub record: Vec<u8>,
}

// Preserve the syntactic Vec segment used by the packed sequence derive while
// forwarding each exact original. No second response schema or decoder exists.
pub(crate) mod borrowed {
    pub struct Value<'a, T>(pub(crate) &'a T);
    pub struct Vec<'a, T>(pub(crate) &'a std::vec::Vec<T>);
    macro_rules! forward {
        ($name:ident, $target:ty) => {
            impl<T> norito::core::SerializePayload for $name<'_, T>
            where
                $target: norito::core::SerializePayload,
            {
                fn serialize(
                    &self,
                    out: &mut norito::core::Encoder<'_>,
                ) -> Result<(), norito::Error> {
                    norito::core::SerializePayload::serialize(self.0, out)
                }
                fn encoded_len_hint(&self) -> Option<usize> {
                    norito::core::SerializePayload::encoded_len_hint(self.0)
                }
                fn encoded_len_exact(&self) -> Option<usize> {
                    norito::core::SerializePayload::encoded_len_exact(self.0)
                }
            }
            impl<T> norito::json::JsonSerialize for $name<'_, T>
            where
                $target: norito::json::JsonSerialize,
            {
                fn json_serialize(&self, out: &mut String) {
                    self.0.json_serialize(out);
                }
                fn json_serialize_to(
                    &self,
                    out: &mut dyn norito::json::JsonWriteSink,
                ) -> Result<(), norito::json::BoundedJsonError> {
                    self.0.json_serialize_to(out)
                }
            }
        };
    }
    forward!(Value, T);
    forward!(Vec, std::vec::Vec<T>);
}

#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub(crate) struct StreamTokenCustodyRecordProofRefV1<'a> {
    pub(crate) head: borrowed::Vec<'a, u8>,
    pub(crate) record: borrowed::Vec<'a, u8>,
}
impl norito::NoritoSchema for StreamTokenCustodyRecordProofRefV1<'_> {
    fn nominal_name() -> String {
        <StreamTokenCustodyRecordProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <StreamTokenCustodyRecordProofV1 as norito::NoritoSchema>::frame_name()
    }
}
fn decode<T>(bytes: &[u8]) -> Result<T, FinalityError>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1 {
        return Err(invalid("Token custody projection exceeds its bound"));
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            4096,
            STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1,
            STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1,
            256 * 1024,
            16,
        ),
    )
    .map_err(map_invalid)
}

impl StreamTokenCustodyRecordProofV1 {
    pub(crate) fn verify_current(
        &self,
        world: &VerifiedWorldStateSnapshotV1,
        expected_chain: &str,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        block: &VerifiedSumeragiBlock,
    ) -> Result<VerifiedStreamTokenCustodyRecordV1, FinalityError> {
        let head: StreamTokenCustodyControlIndexV1 = decode(&self.head)?;
        let record: StreamTokenCustodyControlRecordV1 = decode(&self.record)?;
        let control: SignerCustodyControlStateV1 = decode(&record.control_state)?;
        control.validate().map_err(map_invalid)?;
        record
            .validate_active_enrollment(&control)
            .map_err(map_invalid)?;
        let binding = &control.policy.binding;
        if record.provider_id != expected_provider
            || record.revision == 0
            || record.revision > STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1
            || record.execution_height == 0
            || record.execution_height > world.height()
            || record.recorded_at_unix_ms == 0
            || record.recorded_at_unix_ms > world.block_time_ms()
            || head.revision != record.revision
            || head.height != record.execution_height
            || head.ordinal != record.ordinal
            || head.digest != record.canonical_digest().map_err(map_invalid)?
            || binding.chain_id != expected_chain
            || binding.network_id != *expected_network.as_bytes()
            || binding.role != SignerRoleV1::StreamToken
            || binding.purpose
                != (SignerPurposeBindingV1::StreamToken {
                    provider_id: *expected_provider.as_bytes(),
                })
        {
            return Err(invalid(
                "Token custody projection differs from its native provider scope",
            ));
        }
        world.verify_table_value(
            "world.smart_contract_state",
            &head_key(expected_provider),
            &self.head,
        )?;
        world.verify_table_value(
            "world.smart_contract_state",
            &record_key(expected_provider, record.revision),
            &self.record,
        )?;
        world.verify_table_value(
            "world.smart_contract_state",
            &height_key(expected_provider, head.height, head.ordinal),
            &self.head,
        )?;
        world.verify_smart_contract_state_absent(&record_key(
            expected_provider,
            record.revision + 1,
        ))?;
        Ok(VerifiedStreamTokenCustodyRecordV1 {
            record,
            control,
            anchor: SignerCustodyAnchorV1 {
                height: world.height(),
                block_hash: *block.header().hash().as_ref(),
                state_digest: head.digest,
            },
        })
    }
}

/// Data-only exact native custody projection, independent of admission or advertisements.
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
    name = "iroha_data_model::sorafs::stream_token_custody::proof::StreamTokenCustodyProofV1"
)]
pub struct StreamTokenCustodyProofV1 {
    /// Complete hash preimages at the original certified native execution cut.
    pub world: WorldStateSnapshotV1,
    /// Exact native provider owner preimage, checked against the caller's selected owner.
    pub owner: AccountId,
    /// Exact current index and immutable original; absence requires authenticated World absence.
    #[norito(required)]
    pub current: Option<StreamTokenCustodyRecordProofV1>,
}

/// Borrowed encoder for the sole canonical projection layout, retaining the original allocation owner.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct StreamTokenCustodyProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    owner: borrowed::Value<'a, AccountId>,
    current: Option<StreamTokenCustodyRecordProofRefV1<'a>>,
}
impl norito::NoritoSchema for StreamTokenCustodyProofRefV1<'_> {
    fn nominal_name() -> String {
        <StreamTokenCustodyProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <StreamTokenCustodyProofV1 as norito::NoritoSchema>::frame_name()
    }
}
impl<'a> StreamTokenCustodyProofRefV1<'a> {
    /// Borrow original native values without granting custody or finality authority.
    #[must_use]
    pub fn new(
        world: &'a WorldStateSnapshotV1,
        owner: &'a AccountId,
        current: Option<(&'a Vec<u8>, &'a Vec<u8>)>,
    ) -> Self {
        Self {
            world: borrowed::Value(world),
            owner: borrowed::Value(owner),
            current: current.map(|(head, record)| StreamTokenCustodyRecordProofRefV1 {
                head: borrowed::Vec(head),
                record: borrowed::Vec(record),
            }),
        }
    }
}

/// Exact native record authenticated at one independently selected certified cut.
/// Expired, revoked and unenrolled controls remain facts, not permission to sign or enroll.
#[derive(Clone, Debug)]
pub struct VerifiedStreamTokenCustodyRecordV1 {
    pub(crate) record: StreamTokenCustodyControlRecordV1,
    pub(crate) control: SignerCustodyControlStateV1,
    pub(crate) anchor: SignerCustodyAnchorV1,
}
impl VerifiedStreamTokenCustodyRecordV1 {
    /// Exact native immutable record, including revision, provenance and original enrollment.
    #[must_use]
    pub fn record(&self) -> &StreamTokenCustodyControlRecordV1 {
        &self.record
    }
    /// Exact canonical committed control; callers separately enforce policy and eligibility.
    #[must_use]
    pub fn control(&self) -> &SignerCustodyControlStateV1 {
        &self.control
    }
    /// Native approval anchor whose digest is the original control record commitment.
    #[must_use]
    pub fn anchor(&self) -> SignerCustodyAnchorV1 {
        self.anchor
    }
}

/// Public custody presence or absence authenticated under the caller's independent trust inputs.
/// This result is not an enrollment, current-use, provider admission or transaction grant.
#[derive(Clone, Debug)]
pub struct VerifiedStreamTokenCustodyStateV1 {
    network_id: NetworkId,
    provider_id: ProviderId,
    owner: AccountId,
    height: u64,
    context_id: Hash,
    current: Option<VerifiedStreamTokenCustodyRecordV1>,
}
impl VerifiedStreamTokenCustodyStateV1 {
    /// Independently selected network authenticated by the certified decision.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Independently selected provider whose native owner and custody paths were authenticated.
    #[must_use]
    pub fn provider_id(&self) -> ProviderId {
        self.provider_id
    }
    /// Exact selected owner authenticated by the native provider-owner row.
    #[must_use]
    pub fn owner(&self) -> &AccountId {
        &self.owner
    }
    /// Height of the certified execution cut; freshness must be enforced by the caller.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Exact certified consensus context joining this evidence to the independently selected cut.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Exact current custody, or authenticated absence of both native head and first record.
    #[must_use]
    pub fn current(&self) -> Option<&VerifiedStreamTokenCustodyRecordV1> {
        self.current.as_ref()
    }
}
impl StreamTokenCustodyProofV1 {
    /// Decode the sole canonical data frame under finite transport and allocation bounds.
    /// # Errors
    /// Oversized, malformed, noncanonical or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        if bytes.len() > MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1 {
            return Err(norito::core::Error::Message(
                "custody proof exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                131_072,
                MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1,
                131_072,
                128 * 1024 * 1024,
                64,
            ),
        )
    }

    /// Authenticate exact presence or absence without requiring provider admission or an advert.
    ///
    /// The network, provider, owner, full signer binding, qualified native schema and certified
    /// decision must be selected independently. The caller enforces its own finality freshness
    /// policy. No response value selects trust, authorizes enrollment, or proves current use.
    /// # Errors
    /// Wrong independent scope, owner or schema, changed World/preimages, concealed native
    /// custody, malformed or noncurrent records, or exceeded finite bounds.
    pub fn verify(
        &self,
        expected_network: NetworkId,
        expected_provider: ProviderId,
        expected_owner: &AccountId,
        expected_binding: &SignerCustodyBindingV1,
        expected_schema: Hash,
        block: &VerifiedSumeragiBlock,
    ) -> Result<VerifiedStreamTokenCustodyStateV1, FinalityError> {
        expected_binding.validate().map_err(map_invalid)?;
        block.verify_global_scope(expected_network, &expected_binding.chain_id)?;
        if expected_provider.as_bytes() == &[0; 32]
            || block.commitment().schedule.current.network_id != expected_network
            || self.world.schema_hash != expected_schema
            || self.owner != *expected_owner
            || expected_binding.network_id != *expected_network.as_bytes()
            || expected_binding.role != SignerRoleV1::StreamToken
            || expected_binding.purpose
                != (SignerPurposeBindingV1::StreamToken {
                    provider_id: *expected_provider.as_bytes(),
                })
            || norito::canonical_frame_len(self).map_err(map_invalid)?
                > MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1
        {
            return Err(invalid(
                "Custody proof differs from its independent scope or bound",
            ));
        }
        let world = self.world.authenticate(block)?;
        world.verify_table_value("world.provider_owners", &expected_provider, expected_owner)?;
        let current = if let Some(proof) = &self.current {
            let current = proof.verify_current(
                &world,
                &expected_binding.chain_id,
                expected_network,
                expected_provider,
                block,
            )?;
            if current.control.policy.binding != *expected_binding {
                return Err(invalid(
                    "Custody proof differs from the independent complete signer binding",
                ));
            }
            Some(current)
        } else {
            world.verify_smart_contract_state_absent(&head_key(expected_provider))?;
            world.verify_smart_contract_state_absent(&record_key(expected_provider, 1))?;
            None
        };
        Ok(VerifiedStreamTokenCustodyStateV1 {
            network_id: expected_network,
            provider_id: expected_provider,
            owner: self.owner.clone(),
            height: world.height(),
            context_id: world.context_id(),
            current,
        })
    }
}

#[cfg(test)]
mod tests;
