//! Current SNS lease preimages authenticated by an independently selected native decision.
//!
//! The response supplies no checkpoint, schema trust or freshness policy. Its complete World
//! snapshot proves the exact native record at the caller's selected certified cut; local time
//! also bounds expiry. Historical certification alone never establishes current network liveness.

use super::{NameRecordV1, NameSelectorV1, NameStatus, record_storage_key};
use crate::{
    account::AccountId,
    id::NetworkId,
    sumeragi_finality::{FinalityError, VerifiedSumeragiBlock, WorldStateSnapshotV1},
};
use iroha_crypto::Hash;
use norito::codec::{Decode as _, Encode as _};

/// Aggregate canonical lease projection bound, independent of consensus validity.
pub const MAX_SNS_LEASE_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum retained original native SNS record bytes in one portable projection.
pub const MAX_SNS_LEASE_RECORD_BYTES_V1: usize = 256 * 1024;

/// Data-only current SNS lease projection; decoding grants no authority.
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
#[norito_schema(name = "iroha_data_model::sns::lease::SnsLeaseProofV1")]
pub struct SnsLeaseProofV1 {
    /// Complete canonical native World hash preimages at the selected certified decision.
    pub world: WorldStateSnapshotV1,
    /// Exact original native `NameRecordV1::encode` bytes, bound by the World table row.
    pub record: Vec<u8>,
}

mod borrowed {
    pub(super) struct Value<'a, T>(pub(super) &'a T);
    pub(super) struct Vec<'a, T>(pub(super) &'a std::vec::Vec<T>);
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

/// Borrowed encoder for the sole lease response layout, retaining original publication custody.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct SnsLeaseProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    record: borrowed::Vec<'a, u8>,
}
impl norito::NoritoSchema for SnsLeaseProofRefV1<'_> {
    fn nominal_name() -> String {
        <SnsLeaseProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <SnsLeaseProofV1 as norito::NoritoSchema>::frame_name()
    }
}
impl<'a> SnsLeaseProofRefV1<'a> {
    /// Borrow exact originals without copying the complete World graph.
    #[must_use]
    pub fn new(world: &'a WorldStateSnapshotV1, record: &'a Vec<u8>) -> Self {
        Self {
            world: borrowed::Value(world),
            record: borrowed::Vec(record),
        }
    }
}

/// Exact active lease authenticated at the caller-selected certified native World cut.
#[derive(Debug, Clone)]
pub struct VerifiedSnsLeaseV1 {
    network_id: NetworkId,
    height: u64,
    context_id: Hash,
    record: NameRecordV1,
}

fn invalid(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}

impl SnsLeaseProofV1 {
    /// Decode the sole canonical projection under finite byte, count and allocation limits.
    /// # Errors
    /// Oversized, noncanonical, malformed or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        if bytes.len() > MAX_SNS_LEASE_PROOF_BYTES_V1 {
            return Err(norito::core::Error::Message(
                "SNS lease projection exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                131_072,
                MAX_SNS_LEASE_PROOF_BYTES_V1,
                131_072,
                128 * 1024 * 1024,
                64,
            ),
        )
    }

    /// Authenticate the exact expected selector and owner at an independently selected cut.
    ///
    /// The caller obtains `native_schema` from qualified installation authority and establishes
    /// fresh parent quorum before choosing `block`. Neither value comes from this response.
    /// # Errors
    /// Wrong network/schema/cut, substituted selector/owner/record, inactive or expired lease,
    /// malformed native bytes or exceeded finite bounds.
    pub fn verify(
        &self,
        network: NetworkId,
        selector: &NameSelectorV1,
        owner: &AccountId,
        native_schema: Hash,
        block: &VerifiedSumeragiBlock,
        now_unix_ms: u64,
    ) -> Result<VerifiedSnsLeaseV1, FinalityError> {
        let canonical = NameSelectorV1::new(selector.suffix_id, &selector.label)
            .map_err(|_| invalid("SNS lease selector is invalid"))?;
        if canonical != *selector
            || !matches!(
                selector.suffix_id,
                super::ACCOUNT_ALIAS_SUFFIX_ID
                    | super::DOMAIN_NAME_SUFFIX_ID
                    | super::DATASPACE_ALIAS_SUFFIX_ID
            )
            || block.height() < 2
            || block.commitment().schedule.current.network_id != network
            || self.world.schema_hash != native_schema
            || now_unix_ms == 0
            || self.record.is_empty()
            || self.record.len() > MAX_SNS_LEASE_RECORD_BYTES_V1
            || norito::canonical_frame_len(self)
                .map_err(|_| invalid("SNS lease projection is invalid"))?
                > MAX_SNS_LEASE_PROOF_BYTES_V1
        {
            return Err(invalid(
                "SNS lease identity, schema or resource bound differs",
            ));
        }
        let world = self.world.authenticate(block)?;
        world.verify_table_value(
            "world.smart_contract_state",
            &record_storage_key(selector),
            &self.record,
        )?;
        let record = norito::with_decode_limits_scope(
            norito::DecodeLimits::new(
                16_384,
                MAX_SNS_LEASE_RECORD_BYTES_V1,
                16_384,
                2 * 1024 * 1024,
                32,
            ),
            || {
                let mut bytes = self.record.as_slice();
                let record = NameRecordV1::decode(&mut bytes)
                    .map_err(|_| invalid("SNS lease native record is invalid"))?;
                if !bytes.is_empty() || record.encode() != self.record {
                    return Err(invalid("SNS lease native record is not canonical"));
                }
                Ok(record)
            },
        )?;
        if record.selector != *selector
            || record.name_hash != selector.name_hash()
            || record.owner != *owner
            || record.ownership_generation == 0
            || !matches!(record.status, NameStatus::Active)
            || record.registered_at_ms > world.block_time_ms()
            || record.registered_at_ms > now_unix_ms
            || record.expires_at_ms <= now_unix_ms.max(world.block_time_ms())
            || record.expires_at_ms > record.grace_expires_at_ms
            || record.grace_expires_at_ms > record.redemption_expires_at_ms
        {
            return Err(invalid(
                "SNS lease differs from the exact current active owner",
            ));
        }
        Ok(VerifiedSnsLeaseV1 {
            network_id: network,
            height: block.height(),
            context_id: block.context_id(),
            record,
        })
    }
}
impl VerifiedSnsLeaseV1 {
    /// Independently selected native network.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Original certified World cut height.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Original native certified decision identity.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Exact independently authenticated active native ownership record.
    #[must_use]
    pub fn record(&self) -> &NameRecordV1 {
        &self.record
    }
}

#[cfg(test)]
mod tests;
