//! Bounded sparse signer bindings, with a count preflight before binary allocation.
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError, ChargedShared,
    PrepaidSharedError, shared::Shared,
};
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
    json,
};
use std::{alloc::Layout, fmt};

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

/// Sparse immutable original stake bindings in strict native signer order.
///
/// All clones retain the original backing. Decoding produces untrusted storage;
/// original-pool admission is a separate resource operation and grants no authority.
#[derive(Clone, Default)]
pub struct SumeragiLaneCustodySigners(Storage);
#[derive(Clone, Default)]
enum Storage {
    #[default]
    Empty,
    Untrusted(Shared<Vec<SumeragiLaneSignerCustody>, ()>),
    Admitted(ChargedShared<ChargedBuffer<SumeragiLaneSignerCustody>>),
}

/// Typed refusal before immutable signer storage can enter the original State pool.
#[derive(Debug)]
pub enum CustodySignersAdmissionError {
    /// Sparse signer order or bounds are invalid.
    Invalid,
    /// An admitted owner belongs to another pool, even if its contents match.
    ForeignBudget,
    /// Exact fixed backing admission or allocation failed locally.
    Backing(ChargedBufferError),
    /// Original-pool admission refused the shared control block.
    ControlAdmission(AllocationRefusal),
    /// The admitted shared control block could not be allocated.
    ControlAllocation(PrepaidSharedError),
}
impl fmt::Display for CustodySignersAdmissionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => out.write_str("invalid original lane custody signer order or bound"),
            Self::ForeignBudget => {
                out.write_str("lane custody signer owner belongs to another pool")
            }
            Self::Backing(error) => error.fmt(out),
            Self::ControlAdmission(error) => error.fmt(out),
            Self::ControlAllocation(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for CustodySignersAdmissionError {}
impl SumeragiLaneCustodySigners {
    /// Borrow retained bindings without copying their backing.
    #[must_use]
    pub fn as_slice(&self) -> &[SumeragiLaneSignerCustody] {
        match &self.0 {
            Storage::Empty => &[],
            Storage::Untrusted(rows) => rows.as_slice(),
            Storage::Admitted(rows) => rows.as_slice(),
        }
    }
    /// Exact control layout, separate from the fixed signer backing.
    #[must_use]
    pub fn control_layout() -> Layout {
        ChargedShared::<ChargedBuffer<SumeragiLaneSignerCustody>>::allocation_layout()
    }
    /// Whether this owner is empty or retains both original allocations in this pool.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.0 {
            Storage::Empty => true,
            Storage::Untrusted(_) => false,
            Storage::Admitted(rows) => rows.belongs_to(budget),
        }
    }
    /// Move the exact fixed backing into an immutable control from the same pool.
    ///
    /// # Errors
    /// Returns the original backing unchanged on every source, validation, admission
    /// or allocator refusal. Its original charge remains available for retry or drop.
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns the original prepaid backing without allocating an error owner"
    )]
    pub fn from_charged(
        rows: ChargedBuffer<SumeragiLaneSignerCustody>,
        budget: &AllocationBudget,
    ) -> Result<
        Self,
        (
            ChargedBuffer<SumeragiLaneSignerCustody>,
            CustodySignersAdmissionError,
        ),
    > {
        if !rows.belongs_to(budget) {
            return Err((rows, CustodySignersAdmissionError::ForeignBudget));
        }
        if Self::validate_rows(rows.as_slice()).is_err() {
            return Err((rows, CustodySignersAdmissionError::Invalid));
        }
        let mut reservation = match budget.try_reserve(Self::control_layout()) {
            Ok(reservation) => reservation,
            Err(error) => {
                return Err((rows, CustodySignersAdmissionError::ControlAdmission(error)));
            }
        };
        ChargedShared::from_reservation(rows, &mut reservation)
            .map(|owner| Self(Storage::Admitted(owner)))
            .map_err(|(rows, error)| (rows, CustodySignersAdmissionError::ControlAllocation(error)))
    }
    /// Admit a decoded source without replacing its pool or consuming it on refusal.
    ///
    /// The source remains intact while exact new backing/control are admitted before
    /// materialization. Existing same-pool owners share without allocation. Temporary
    /// partial copies refund on refusal; a retry reads the unchanged original source.
    /// # Errors
    /// Returns typed foreign-owner, shape, pool or allocator refusal.
    pub fn admit(&self, budget: &AllocationBudget) -> Result<Self, CustodySignersAdmissionError> {
        match &self.0 {
            Storage::Empty => return Ok(Self::default()),
            Storage::Admitted(_) => {
                return self
                    .admitted_to(budget)
                    .then(|| self.clone())
                    .ok_or(CustodySignersAdmissionError::ForeignBudget);
            }
            Storage::Untrusted(_) => {}
        }
        self.validate()
            .map_err(|_| CustodySignersAdmissionError::Invalid)?;
        let mut rows = ChargedBuffer::new(self.as_slice().len(), budget)
            .map_err(CustodySignersAdmissionError::Backing)?;
        rows.append(self.as_slice())
            .expect("exact signer capacity was admitted before copying");
        Self::from_charged(rows, budget).map_err(|(_rows, error)| error)
    }
    fn validate_rows(rows: &[SumeragiLaneSignerCustody]) -> Result<(), &'static str> {
        if rows.len() > MAX_LANE_CUSTODY_SIGNERS
            || rows.iter().any(|entry| {
                usize::try_from(entry.signer)
                    .ok()
                    .is_none_or(|index| index >= MAX_LANE_CUSTODY_SIGNERS)
            })
            || rows.windows(2).any(|pair| pair[0].signer >= pair[1].signer)
        {
            return Err("invalid original lane custody signer order or bound");
        }
        Ok(())
    }
    /// Check the native bound and strict index order.
    /// # Errors
    /// An index/count is too large or an index is repeated or out of order.
    pub fn validate(&self) -> Result<(), &'static str> {
        Self::validate_rows(self.as_slice())
    }
}
impl TryFrom<Vec<SumeragiLaneSignerCustody>> for SumeragiLaneCustodySigners {
    type Error = ncore::Error;
    fn try_from(values: Vec<SumeragiLaneSignerCustody>) -> Result<Self, Self::Error> {
        Self::validate_rows(&values).map_err(|message| ncore::Error::Message(message.into()))?;
        if values.is_empty() {
            return Ok(Self::default());
        }
        let bytes = Shared::<Vec<SumeragiLaneSignerCustody>, ()>::layout().size();
        ncore::reserve_decode_allocation(bytes)?;
        Shared::try_new(values, ())
            .map(|owner| Self(Storage::Untrusted(owner)))
            .map_err(|(_values, (), _error)| ncore::Error::AllocationFailed {
                bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
            })
    }
}
impl fmt::Debug for SumeragiLaneCustodySigners {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(out)
    }
}
impl PartialEq for SumeragiLaneCustodySigners {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for SumeragiLaneCustodySigners {}
impl TypeId for SumeragiLaneCustodySigners {
    fn id() -> String {
        "SumeragiLaneCustodySigners".into()
    }
}
impl IntoSchema for SumeragiLaneCustodySigners {
    fn type_name() -> String {
        "SumeragiLaneCustodySigners".into()
    }
    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta {
                types: vec![std::any::TypeId::of::<Vec<SumeragiLaneSignerCustody>>()],
            }));
            Vec::<SumeragiLaneSignerCustody>::update_schema_map(map);
        }
    }
}
impl norito::NoritoSchema for SumeragiLaneCustodySigners {
    fn nominal_name() -> String {
        Self::static_frame_name().expect("fixed identity").into()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_data_model::sumeragi_lanes::SumeragiLaneCustodySigners")
    }
}
struct SignerSequence<'a>(&'a [SumeragiLaneSignerCustody]);
impl SerializePayload for SignerSequence<'_> {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_element_sequence::<SumeragiLaneSignerCustody, _>(writer, self.0.iter())
    }
}
impl SerializePayload for SumeragiLaneCustodySigners {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        self.validate()
            .map_err(|message| ncore::Error::Message(message.into()))?;
        let sequence = SignerSequence(self.as_slice());
        let len = ncore::encoded_payload_len(&sequence)?;
        ncore::write_len(
            writer,
            u64::try_from(len).map_err(|_| ncore::Error::LengthMismatch)?,
        )?;
        sequence.serialize(writer)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let len = ncore::encoded_payload_len(&SignerSequence(self.as_slice())).ok()?;
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
        let value = Self::try_from(values)?;
        ncore::note_payload_access(bytes, end);
        Ok((value, end))
    }
}
impl json::JsonSerialize for SumeragiLaneCustodySigners {
    fn json_serialize(&self, out: &mut String) {
        out.push('[');
        for (index, value) in self.as_slice().iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            value.json_serialize(out);
        }
        out.push(']');
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.begin_container()?;
        let result = (|| -> Result<(), norito::json::BoundedJsonError> {
            out.push('[')?;
            for (index, value) in self.as_slice().iter().enumerate() {
                if index != 0 {
                    out.push(',')?;
                }
                value.json_serialize_to(out)?;
            }
            out.push(']')?;
            Ok(())
        })();
        out.end_container();
        result?;
        Ok(())
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
            values
                .try_reserve_exact(1)
                .map_err(|_| json::Error::AllocationFailed)?;
            values.push(value);
        }
        sequence.finish()?;
        Self::try_from(values).map_err(|error| {
            if error.is_decode_resource_limit() {
                json::Error::from_decode_resource(error)
            } else {
                json::Error::Message(error.to_string())
            }
        })
    }
}
