//! A physically prepared nested row bank and its separate canonical final backing.

use super::*;
use common::*;
use rows::Row;

/// The prepared row objects retain every nested key/signature/byte allocation.
/// Their canonical output backing also exists before any original input is read.
pub(super) struct Rows<D: Row> {
    pub(super) destinations: ChargedBuffer<D>,
    pub(super) canonical: ChargedBuffer<D::Wire>,
    spans: ChargedBuffer<SequenceSpan>,
    ready: bool,
}
impl<D: Row> Rows<D> {
    pub(super) fn new(
        count: usize,
        budget: &AllocationBudget,
        mut prepare: impl FnMut(usize) -> Result<D, SessionGraphError>,
    ) -> Result<Self, SessionGraphError> {
        let canonical = buffer(count, budget)?;
        let spans = spans(count, budget)?;
        let mut destinations = buffer(count, budget)?;
        for index in 0..count {
            destinations.push_reserved(prepare(index)?);
        }
        Ok(Self {
            destinations,
            canonical,
            spans,
            ready: false,
        })
    }
    pub(super) fn decode(&mut self, bytes: &[u8]) -> DecodeResult<()> {
        self.reset();
        exact_count(bytes, self.destinations.capacity())?;
        let plan =
            prepare_element_sequence(bytes, self.spans.as_mut_slice()).map_err(sequence_error)?;
        complete(plan.used(), bytes)?;
        plan.decode_elements::<D::Wire, DestinationError>(|index, field| {
            field.with_payload(|bytes| self.destinations.as_mut_slice()[index].decode(bytes))
        })?;
        self.ready = true;
        Ok(())
    }
    pub(super) fn reset(&mut self) {
        self.ready = false;
        for row in self.destinations.as_mut_slice() {
            row.reset();
        }
    }
    pub(super) fn ready(&self) -> bool {
        self.ready && self.destinations.as_slice().iter().all(Row::ready)
    }
}
impl<D: Row> SerializePayload for Rows<D> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        if !self.ready() {
            return Err(norito::Error::InvalidValue {
                context: "unfinished prepared DKG row sequence",
            });
        }
        norito::core::write_element_sequence::<PayloadRef<'_, D>, _>(
            writer,
            self.destinations.as_slice().iter().map(PayloadRef),
        )
    }
}
