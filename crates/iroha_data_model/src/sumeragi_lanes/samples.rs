//! Immutable autoscale samples retaining exact original backing and control custody.
use std::{alloc::Layout, fmt, ops::Deref};

use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedShared, PrepaidBufferError,
    PrepaidSharedError, shared::Shared,
};
use iroha_schema::{Declaration, IntoSchema, Metadata, NamedFieldsMeta};
use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
    json,
};

use super::{CustodySignersAdmissionError, SumeragiLaneSample};

/// Local refusal while admitting an immutable sample window.
#[derive(Debug)]
pub enum LaneSamplesAdmissionError {
    /// An existing admitted window belongs to a different original pool.
    ForeignBudget,
    /// Exact combined backing/control admission refused before materialization.
    Admission(AllocationRefusal),
    /// The physical allocator refused the original prepaid row backing.
    Backing(PrepaidBufferError),
    /// The physical allocator refused the original prepaid shared control.
    Control(PrepaidSharedError),
}
impl fmt::Display for LaneSamplesAdmissionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBudget => out.write_str("lane sample owner belongs to another pool"),
            Self::Admission(error) => error.fmt(out),
            Self::Backing(error) => error.fmt(out),
            Self::Control(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for LaneSamplesAdmissionError {}

/// Typed original-pool admission of the nested owners in one lane State generation.
#[derive(Debug)]
pub enum LaneStateAdmissionError {
    /// Original signer backing or control was refused or invalid.
    Signers(CustodySignersAdmissionError),
    /// Original sample backing or control was refused.
    Samples(LaneSamplesAdmissionError),
}
impl fmt::Display for LaneStateAdmissionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signers(error) => error.fmt(out),
            Self::Samples(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for LaneStateAdmissionError {}
impl From<CustodySignersAdmissionError> for LaneStateAdmissionError {
    fn from(error: CustodySignersAdmissionError) -> Self {
        Self::Signers(error)
    }
}
impl From<LaneSamplesAdmissionError> for LaneStateAdmissionError {
    fn from(error: LaneSamplesAdmissionError) -> Self {
        Self::Samples(error)
    }
}

/// Immutable recent samples with the canonical Vec wire representation.
///
/// Cloning shares the exact original allocation. Decoded data remains untrusted;
/// resource admission never grants historical or consensus authority. Outer lane
/// State vectors and Cell controls are separate accounting obligations.
#[derive(Clone, Default, IntoSchema)]
#[schema(transparent = "Vec<SumeragiLaneSample>")]
pub struct SumeragiLaneSamples(Storage);
#[derive(Clone, Default)]
enum Storage {
    #[default]
    Empty,
    Untrusted(Shared<Vec<SumeragiLaneSample>, ()>),
    Admitted(ChargedShared<ChargedBuffer<SumeragiLaneSample>>),
}
impl SumeragiLaneSamples {
    /// Borrow the unchanged original samples, oldest first.
    #[must_use]
    pub fn as_slice(&self) -> &[SumeragiLaneSample] {
        match &self.0 {
            Storage::Empty => &[],
            Storage::Untrusted(rows) => rows.as_slice(),
            Storage::Admitted(rows) => rows.as_slice(),
        }
    }
    /// Exact shared-control layout, separate from the fixed row backing.
    #[must_use]
    pub fn control_layout() -> Layout {
        ChargedShared::<ChargedBuffer<SumeragiLaneSample>>::allocation_layout()
    }
    /// Whether this window is empty or retains its original same-pool owners.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.0 {
            Storage::Empty => true,
            Storage::Untrusted(_) => false,
            Storage::Admitted(rows) => rows.belongs_to(budget),
        }
    }
    /// Admit decoded rows before copying, or retain the exact existing same-pool owner.
    ///
    /// # Errors
    /// Foreign owners, finite-pool refusal and physical allocation refusal remain typed.
    /// The borrowed source survives every failure; partial copies refund when dropped.
    pub fn admit(&self, budget: &AllocationBudget) -> Result<Self, LaneSamplesAdmissionError> {
        match &self.0 {
            Storage::Empty => Ok(Self::default()),
            Storage::Admitted(_) => self
                .admitted_to(budget)
                .then(|| self.clone())
                .ok_or(LaneSamplesAdmissionError::ForeignBudget),
            Storage::Untrusted(_) => Self::build(self.len(), budget, |rows| {
                rows.append(self.as_slice())
                    .expect("exact original sample capacity");
            }),
        }
    }
    /// Append one sample after retaining only the suffix that fits `max_samples`.
    ///
    /// Admission covers the exact resulting row count plus control before copying.
    /// A zero maximum produces an empty window; policy validity is checked separately.
    /// No configured maximum is substituted for the actual retained row count.
    ///
    /// # Errors
    /// Refuses a foreign admitted source or local capacity/allocation without changing it.
    pub fn retain_and_append(
        &self,
        sample: SumeragiLaneSample,
        max_samples: usize,
        budget: &AllocationBudget,
    ) -> Result<Self, LaneSamplesAdmissionError> {
        if matches!(self.0, Storage::Admitted(_)) && !self.admitted_to(budget) {
            return Err(LaneSamplesAdmissionError::ForeignBudget);
        }
        if max_samples == 0 {
            return Ok(Self::default());
        }
        let retained = self.len().min(max_samples - 1);
        let suffix = &self.as_slice()[self.len() - retained..];
        Self::build(retained + 1, budget, |rows| {
            rows.append(suffix)
                .expect("exact original sample suffix capacity");
            rows.push_reserved(sample);
        })
    }
    fn build(
        count: usize,
        budget: &AllocationBudget,
        fill: impl FnOnce(&mut ChargedBuffer<SumeragiLaneSample>),
    ) -> Result<Self, LaneSamplesAdmissionError> {
        if count == 0 {
            return Ok(Self::default());
        }
        let backing = Layout::array::<SumeragiLaneSample>(count)
            .map_err(|_| LaneSamplesAdmissionError::Admission(AllocationRefusal::DemandOverflow))?;
        let mut original = budget
            .try_reserve_layouts([backing, Self::control_layout()])
            .map_err(LaneSamplesAdmissionError::Admission)?;
        let mut rows = ChargedBuffer::from_reservation(count, &mut original)
            .map_err(LaneSamplesAdmissionError::Backing)?;
        fill(&mut rows);
        let shared =
            ChargedShared::from_reservation(rows, &mut original).map_err(|(rows, error)| {
                drop(rows);
                LaneSamplesAdmissionError::Control(error)
            })?;
        debug_assert_eq!(original.remaining_bytes(), 0);
        Ok(Self(Storage::Admitted(shared)))
    }
}
impl Deref for SumeragiLaneSamples {
    type Target = [SumeragiLaneSample];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl fmt::Debug for SumeragiLaneSamples {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_slice().fmt(out)
    }
}
impl PartialEq for SumeragiLaneSamples {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for SumeragiLaneSamples {}
impl TryFrom<Vec<SumeragiLaneSample>> for SumeragiLaneSamples {
    type Error = ncore::Error;
    fn try_from(values: Vec<SumeragiLaneSample>) -> Result<Self, Self::Error> {
        if values.is_empty() {
            return Ok(Self::default());
        }
        let bytes = Shared::<Vec<SumeragiLaneSample>, ()>::layout().size();
        ncore::reserve_decode_allocation(bytes)?;
        Shared::try_new(values, ())
            .map(|rows| Self(Storage::Untrusted(rows)))
            .map_err(|(_values, (), _)| ncore::Error::AllocationFailed {
                bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
            })
    }
}
impl norito::NoritoSchema for SumeragiLaneSamples {
    fn nominal_name() -> String {
        <Vec<SumeragiLaneSample> as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <Vec<SumeragiLaneSample> as norito::NoritoSchema>::frame_name()
    }
}
struct SampleSequence<'a>(&'a [SumeragiLaneSample]);
impl SerializePayload for SampleSequence<'_> {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_element_sequence::<SumeragiLaneSample, _>(writer, self.0.iter())
    }
}
impl SerializePayload for SumeragiLaneSamples {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        SampleSequence(self.as_slice()).serialize(writer)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        ncore::encoded_payload_len(&SampleSequence(self.as_slice())).ok()
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }
}
impl<'de> DeserializePayload<'de> for SumeragiLaneSamples {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("validated canonical lane sample archive")
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
impl<'a> DecodeFromSlice<'a> for SumeragiLaneSamples {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (rows, used) = ncore::decode_field_canonical::<Vec<SumeragiLaneSample>>(bytes)?;
        let value = Self::try_from(rows)?;
        ncore::note_payload_access(bytes, used);
        Ok((value, used))
    }
}
impl json::JsonSerialize for SumeragiLaneSamples {
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
        out.push('[')?;
        for (index, value) in self.as_slice().iter().enumerate() {
            if index != 0 {
                out.push(',')?;
            }
            value.json_serialize_to(out)?;
        }
        out.push(']')?;
        out.end_container();
        Ok(())
    }
}
impl json::JsonDeserialize for SumeragiLaneSamples {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        Self::try_from(Vec::<SumeragiLaneSample>::json_deserialize(parser)?).map_err(|error| {
            if error.is_decode_resource_limit() {
                json::Error::from_decode_resource(error)
            } else {
                json::Error::Message(error.to_string())
            }
        })
    }
}

// Runtime ownership does not change the existing State field schema: samples
// remain the same canonical Vec projection as before original-pool admission.
impl iroha_schema::TypeId for super::SumeragiLaneState {
    fn id() -> String {
        "SumeragiLaneState".into()
    }
}
impl IntoSchema for super::SumeragiLaneState {
    fn type_name() -> String {
        "SumeragiLaneState".into()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
            declarations: [
                (
                    "lanes",
                    std::any::TypeId::of::<Vec<super::SumeragiLaneRecord>>(),
                ),
                (
                    "custody",
                    std::any::TypeId::of::<Vec<super::SumeragiLaneCustody>>(),
                ),
                ("samples", std::any::TypeId::of::<Vec<SumeragiLaneSample>>()),
                ("last_transition", std::any::TypeId::of::<u64>()),
                ("incarnations", std::any::TypeId::of::<u64>()),
            ]
            .into_iter()
            .map(|(name, ty)| Declaration {
                name: name.into(),
                ty,
            })
            .collect(),
        }));
        Vec::<super::SumeragiLaneRecord>::update_schema_map(map);
        Vec::<super::SumeragiLaneCustody>::update_schema_map(map);
        Vec::<SumeragiLaneSample>::update_schema_map(map);
        u64::update_schema_map(map);
    }
}
