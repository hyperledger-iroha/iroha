//! Bounded sparse signer bindings, with a count preflight before binary allocation.
use iroha_schema::IntoSchema;
use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
    json,
};

use super::{MAX_LANE_CUSTODY_SIGNERS, SumeragiLaneStakeBinding};

/// One monetary signer in its pinned native committee; absent indices are forensic-only.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneSignerCustody")]
pub struct SumeragiLaneSignerCustody {
    /// Original canonical signer index, including indices above global election limits.
    pub signer: u32,
    /// Exact original staking tenure and escrow.
    pub binding: SumeragiLaneStakeBinding,
}

/// Sparse original stake bindings in strict native signer order, bounded before decoding.
#[derive(Clone, Debug, Default, PartialEq, Eq, IntoSchema, norito::NoritoSchema)]
#[norito(reuse_archived)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneCustodySigners")]
pub struct SumeragiLaneCustodySigners(Vec<SumeragiLaneSignerCustody>);
impl SumeragiLaneCustodySigners {
    /// Borrow retained bindings without copying their backing.
    #[must_use]
    pub fn as_slice(&self) -> &[SumeragiLaneSignerCustody] {
        &self.0
    }
    /// Check the native bound and strict index order.
    /// # Errors
    /// An index/count is too large or an index is repeated or out of order.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.0.len() > MAX_LANE_CUSTODY_SIGNERS
            || self.0.iter().any(|entry| {
                usize::try_from(entry.signer)
                    .ok()
                    .is_none_or(|index| index >= MAX_LANE_CUSTODY_SIGNERS)
            })
            || self
                .0
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return Err("invalid original lane custody signer order or bound");
        }
        Ok(())
    }
}
impl TryFrom<Vec<SumeragiLaneSignerCustody>> for SumeragiLaneCustodySigners {
    type Error = &'static str;
    fn try_from(values: Vec<SumeragiLaneSignerCustody>) -> Result<Self, Self::Error> {
        let result = Self(values);
        result.validate()?;
        Ok(result)
    }
}
impl SerializePayload for SumeragiLaneCustodySigners {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        self.validate()
            .map_err(|message| ncore::Error::Message(message.into()))?;
        let len = self
            .0
            .encoded_len_exact()
            .ok_or(ncore::Error::LengthMismatch)?;
        ncore::write_len(
            writer,
            u64::try_from(len).map_err(|_| ncore::Error::LengthMismatch)?,
        )?;
        self.0.serialize(writer)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let len = self.0.encoded_len_exact()?;
        ncore::len_prefix_len(len).checked_add(len)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }
}
impl<'de> DeserializePayload<'de> for SumeragiLaneCustodySigners {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("validated canonical lane custody signer archive")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = Self::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value)
    }
}
impl<'a> DecodeFromSlice<'a> for SumeragiLaneCustodySigners {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (len, prefix) = ncore::read_len_dyn_slice(bytes)?;
        let end = prefix
            .checked_add(len)
            .ok_or(ncore::Error::LengthMismatch)?;
        if end != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let field = bytes.get(prefix..end).ok_or(ncore::Error::LengthMismatch)?;
        let (count, _) = ncore::inspect_seq_len_slice(field)?;
        if count > MAX_LANE_CUSTODY_SIGNERS {
            return Err(ncore::Error::LengthMismatch);
        }
        let (values, used) =
            ncore::decode_field_canonical::<Vec<SumeragiLaneSignerCustody>>(field)?;
        if used != field.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let value =
            Self::try_from(values).map_err(|message| ncore::Error::Message(message.into()))?;
        ncore::note_payload_access(bytes, end);
        Ok((value, end))
    }
}
impl json::JsonSerialize for SumeragiLaneCustodySigners {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}
impl json::JsonDeserialize for SumeragiLaneCustodySigners {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut sequence = json::SeqVisitor::new(parser)?;
        let mut values = Vec::new();
        while !sequence.is_finished() {
            if values.len() == MAX_LANE_CUSTODY_SIGNERS {
                return Err(json::Error::Message(
                    "lane custody signer count exceeds native bound".into(),
                ));
            }
            let value = sequence
                .next_element()?
                .ok_or_else(|| json::Error::Message("missing signer".into()))?;
            values.try_reserve_exact(1).map_err(|_| {
                json::Error::Message("lane custody signer allocation failed".into())
            })?;
            values.push(value);
        }
        sequence.finish()?;
        Self::try_from(values).map_err(|message| json::Error::Message(message.into()))
    }
}
