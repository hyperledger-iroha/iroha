//! Execution results and registration payloads of C10 executions.
//!
//! [`TransactionResultC10V1`] is the baseline `TransactionResult` with the C10 rejection family:
//! the outcome a block's execution output certifies for a transaction that contains a C10
//! execution. [`RegisterSmartContractCodeC10V1`] and [`RegisterTriggerC10V1`] are the payloads of
//! the two C10 registration interfaces; their wire IDs are new and the baseline
//! `RegisterSmartContractCode` and `Register<Trigger>` keep theirs.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};

use super::{
    TransactionRejectionReasonC10V1, manifest::ContractManifestC10V1, trigger::TriggerC10V1,
};
use crate::{
    block::consensus::NexusFeeReceipt,
    events::data::prelude::AssetBatchTransferOutcome,
    smart_contract::{ContractArtifactId, event::ContractEmissionsV1},
    trigger::DataTriggerSequence,
};

/// The outcome of a C10 execution: the data triggers it raised, or its C10 rejection.
pub type TransactionResultInnerC10V1 = Result<DataTriggerSequence, TransactionRejectionReasonC10V1>;

/// The certified outcome of a transaction that contains a C10 execution.
///
/// The baseline `TransactionResult` with the C10 rejection family; the other three members are
/// the baseline's own.
/// A certified C10 output binds this value to its signed input or trigger identity.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TransactionResultC10V1")]
pub struct TransactionResultC10V1(
    /// The data triggers raised, or the C10 rejection.
    pub TransactionResultInnerC10V1,
    /// Durable per-leg receipts emitted by an independently settled native transfer batch.
    pub Vec<AssetBatchTransferOutcome>,
    /// Exact actual Nexus charge; explicitly absent when no charge committed.
    pub Option<NexusFeeReceipt>,
    /// Actual native contract emissions in execution order, required even when empty.
    pub ContractEmissionsV1,
);

/// The baseline `TransactionResult`'s JSON object: `Ok` or `Err` (the rejection's base64
/// form), then `batch_transfer_outcomes`, `nexus_fee_receipt` and `contract_events`.
impl norito::json::JsonSerialize for TransactionResultC10V1 {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match &self.0 {
            Ok(sequence) => {
                out.push_str("\"Ok\":");
                norito::json::JsonSerialize::json_serialize(sequence, out);
            }
            Err(reason) => {
                out.push_str("\"Err\":");
                norito::json::JsonSerialize::json_serialize(reason, out);
            }
        }
        out.push_str(",\"batch_transfer_outcomes\":");
        norito::json::JsonSerialize::json_serialize(&self.1, out);
        out.push_str(",\"nexus_fee_receipt\":");
        norito::json::JsonSerialize::json_serialize(&self.2, out);
        out.push_str(",\"contract_events\":");
        norito::json::JsonSerialize::json_serialize(&self.3, out);
        out.push('}');
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        let result = (|| -> Result<(), norito::json::BoundedJsonError> {
            out.push('{')?;
            match &self.0 {
                Ok(sequence) => {
                    out.push_str("\"Ok\":")?;
                    norito::json::JsonSerialize::json_serialize_to(sequence, out)?;
                }
                Err(reason) => {
                    out.push_str("\"Err\":")?;
                    norito::json::JsonSerialize::json_serialize_to(reason, out)?;
                }
            }
            out.push_str(",\"batch_transfer_outcomes\":")?;
            norito::json::JsonSerialize::json_serialize_to(&self.1, out)?;
            out.push_str(",\"nexus_fee_receipt\":")?;
            norito::json::JsonSerialize::json_serialize_to(&self.2, out)?;
            out.push_str(",\"contract_events\":")?;
            norito::json::JsonSerialize::json_serialize_to(&self.3, out)?;
            out.push('}')?;
            Ok(())
        })();
        out.end_container();
        result
    }
}

impl norito::json::JsonDeserialize for TransactionResultC10V1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let mut inner = None;
        let mut batch_transfer_outcomes = None;
        let mut nexus_fee_receipt = None;
        let mut contract_events = None;
        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }
            let key = parser.parse_key()?;
            match key.as_str() {
                "Ok" | "Err" if inner.is_some() => {
                    return Err(norito::json::Error::duplicate_field(key.as_str()));
                }
                "Ok" => {
                    inner = Some(Ok(DataTriggerSequence::json_deserialize(parser)?));
                }
                "Err" => {
                    inner = Some(Err(TransactionRejectionReasonC10V1::json_deserialize(
                        parser,
                    )?));
                }
                "batch_transfer_outcomes" => {
                    if batch_transfer_outcomes.is_some() {
                        return Err(norito::json::Error::duplicate_field(
                            "batch_transfer_outcomes",
                        ));
                    }
                    batch_transfer_outcomes =
                        Some(Vec::<AssetBatchTransferOutcome>::json_deserialize(parser)?);
                }
                "nexus_fee_receipt" => {
                    if nexus_fee_receipt.is_some() {
                        return Err(norito::json::Error::duplicate_field("nexus_fee_receipt"));
                    }
                    nexus_fee_receipt = Some(Option::<NexusFeeReceipt>::json_deserialize(parser)?);
                }
                "contract_events" => {
                    if contract_events.is_some() {
                        return Err(norito::json::Error::duplicate_field("contract_events"));
                    }
                    contract_events = Some(ContractEmissionsV1::json_deserialize(parser)?);
                }
                other => return Err(norito::json::Error::unknown_field(other.to_owned())),
            }
            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            parser.consume_char(b'}')?;
            break;
        }
        Ok(Self(
            inner.ok_or_else(|| norito::json::Error::missing_field("Ok or Err"))?,
            batch_transfer_outcomes.unwrap_or_default(),
            nexus_fee_receipt
                .ok_or_else(|| norito::json::Error::missing_field("nexus_fee_receipt"))?,
            contract_events.ok_or_else(|| norito::json::Error::missing_field("contract_events"))?,
        ))
    }
}

/// Register a C10 contract manifest under its exact data space artifact identity.
///
/// The C10 counterpart of `RegisterSmartContractCode`: the manifest is a
/// [`ContractManifestC10V1`] and its `abi_hash` names the C10 host profile.
/// The governed C10 addition claims its tolerant wire ID.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::ivm_c10::RegisterSmartContractCodeC10V1")]
pub struct RegisterSmartContractCodeC10V1 {
    /// Exact artifact data space and hash; the manifest must carry the same hash.
    pub artifact_id: ContractArtifactId,
    /// The C10 manifest, containing `code_hash` and the C10 `abi_hash`.
    pub manifest: ContractManifestC10V1,
}

impl crate::seal::Instruction for RegisterSmartContractCodeC10V1 {}

impl RegisterSmartContractCodeC10V1 {
    /// Stable wire identity of the C10 contract registration interface.
    pub const WIRE_ID: &'static str = "iroha.smart_contract.code.c10.register.v1";
}

/// Register a C10 trigger.
///
/// The C10 counterpart of `Register<Trigger>`: the trigger's filter carries the C10 status
/// family and its executable runs on the C10 profile.
/// The governed C10 addition selects this registration interface.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::ivm_c10::RegisterTriggerC10V1")]
pub struct RegisterTriggerC10V1 {
    /// The trigger to register (the baseline `Register<Trigger>` names its member `object`).
    pub object: TriggerC10V1,
}

impl crate::seal::Instruction for RegisterTriggerC10V1 {}

impl RegisterTriggerC10V1 {
    /// Stable wire identity of the C10 trigger registration interface.
    pub const WIRE_ID: &'static str = "iroha.trigger.c10.register.v1";
}

// Canonical field decoding with no trailing bytes, exactly as the baseline
// `RegisterSmartContractCode` and `Register<Trigger>` decode inside an instruction frame.
impl<'a> norito::core::DecodeFromSlice<'a> for RegisterSmartContractCodeC10V1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        let mut offset = 0usize;
        let artifact_id = crate::isi::decode_aos_canonical_field::<ContractArtifactId>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let manifest = crate::isi::decode_aos_canonical_field::<ContractManifestC10V1>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                artifact_id,
                manifest,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterTriggerC10V1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        let mut offset = 0usize;
        let object = crate::isi::decode_aos_canonical_field::<TriggerC10V1>(
            crate::isi::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { object }, offset))
    }
}
