//! Native result/schedule container geometry and original successor slot fields.

use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, Uint, Word, WordHasher};

use super::decode::{FieldSpan, ScheduleReader};

/// The complete five-field schedule located inside the original result.
#[derive(Clone, Debug)]
pub struct ScheduleRanges {
    height: Uint<Fp, 64>,
    current: FieldSpan,
    boundary: FieldSpan,
    next: FieldSpan,
    after_next: FieldSpan,
}

impl ScheduleRanges {
    /// Consume exactly the native result and schedule fields and bind both heights.
    /// Skipped execution/beacon/lane payloads remain part of the complete R hash;
    /// they never supply independent authority through this decoder.
    /// # Errors
    /// Layout errors; substituted framing, omitted fields or different heights fail.
    pub fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Self, Error> {
        let mut result = reader.result_payload(region)?;
        let result_height = reader.field(region, &mut result)?;
        let _execution = reader.field(region, &mut result)?;
        let schedule = reader.field(region, &mut result)?;
        let _beacon = reader.field(region, &mut result)?;
        let _lanes = reader.field(region, &mut result)?;
        reader.finish(region, &result)?;
        let result_height = reader.scalar::<8>(region, &result_height)?;
        reader.uint.assert_nonzero(region, &result_height)?;
        let mut cursor = reader.enter(region, &schedule)?;
        let height = reader.field(region, &mut cursor)?;
        let current = reader.field(region, &mut cursor)?;
        let boundary = reader.field(region, &mut cursor)?;
        let next = reader.field(region, &mut cursor)?;
        let after_next = reader.field(region, &mut cursor)?;
        reader.finish(region, &cursor)?;
        let height = reader.scalar::<8>(region, &height)?;
        GlueChip::assert_equal(region, height.word(), result_height.word())?;
        Ok(Self {
            height,
            current,
            boundary,
            next,
            after_next,
        })
    }
    /// Exact executed and schedule height from the original result.
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Exact incumbent context payload range; its identity is separately hashed.
    pub const fn current(&self) -> &FieldSpan {
        &self.current
    }
    /// Original boundary Option field, including its discriminant.
    pub const fn boundary(&self) -> &FieldSpan {
        &self.boundary
    }
    /// Original h+1 compact slot, whose Ready epoch is derived from this schedule.
    pub const fn next(&self) -> &FieldSpan {
        &self.next
    }
    /// Original h+2 compact slot, retaining the native boundary barrier.
    pub const fn after_next(&self) -> &FieldSpan {
        &self.after_next
    }

    /// Bind both decoded slots to these exact source ranges and consecutive heights.
    /// # Errors
    /// Layout errors; slot substitution, height overflow or gap is unsatisfied.
    pub fn bind_slots<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        next: &SlotCells,
        after_next: &SlotCells,
    ) -> Result<(), Error> {
        reader.same_span(region, &self.next, &next.body)?;
        reader.same_span(region, &self.after_next, &after_next.body)?;
        let h1 = reader.uint.checked_add_constant(region, &self.height, 1)?;
        let h2 = reader.uint.checked_add_constant(region, &self.height, 2)?;
        GlueChip::assert_equal(region, h1.word(), next.height.word())?;
        GlueChip::assert_equal(region, h2.word(), after_next.height.word())
    }
}

/// Fields of the exact original optional boundary, including its next context.
#[derive(Clone, Debug)]
pub struct BoundaryRanges {
    body: FieldSpan,
    height: Uint<Fp, 64>,
    predecessor: [Word<Fp>; 32],
    selection_anchor: [Word<Fp>; 32],
    next: FieldSpan,
    preparation: FieldSpan,
}

impl BoundaryRanges {
    /// Decode Some/None and exactly six native boundary fields with fixed shape.
    /// # Errors
    /// Layout errors; wrong option/version, omitted/trailing fields or changed height fail.
    pub fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        schedule: &ScheduleRanges,
    ) -> Result<Self, Error> {
        let body = reader.option(region, &schedule.boundary)?;
        let mut cursor = reader.enter(region, &body)?;
        let version = reader.field(region, &mut cursor)?;
        let height = reader.field(region, &mut cursor)?;
        let predecessor = reader.field(region, &mut cursor)?;
        let selection_anchor = reader.field(region, &mut cursor)?;
        let next = reader.field(region, &mut cursor)?;
        let preparation = reader.field(region, &mut cursor)?;
        reader.finish(region, &cursor)?;
        let version = reader.scalar::<2>(region, &version)?;
        reader.constant_when(region, body.present(), version.word(), 1)?;
        let height = reader.scalar::<8>(region, &height)?;
        reader.equal_when(
            region,
            body.present(),
            height.word(),
            schedule.height.word(),
        )?;
        reader.exact_len(region, &predecessor, 32)?;
        let predecessor = reader.bytes::<32>(region, &predecessor, 0)?;
        reader.exact_len(region, &selection_anchor, 32)?;
        let selection_anchor = reader.bytes::<32>(region, &selection_anchor, 0)?;
        Ok(Self {
            body,
            height,
            predecessor,
            selection_anchor,
            next,
            preparation,
        })
    }
    /// Native boundary presence, decoded from the authenticated original Option.
    pub const fn present(&self) -> &Bit<Fp> {
        self.body.present()
    }
    /// The native boundary height, zero if absent.
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Exact predecessor context identity, to bind to the incumbent context hash.
    pub const fn predecessor(&self) -> &[Word<Fp>; 32] {
        &self.predecessor
    }
    /// Original committed B-1 application-header selection anchor.
    pub const fn selection_anchor(&self) -> &[Word<Fp>; 32] {
        &self.selection_anchor
    }
    /// Complete authorized successor context range, absent when there is no boundary.
    pub const fn next(&self) -> &FieldSpan {
        &self.next
    }
    /// Original E+2 preparation Option; quorum execution determines its contents.
    pub const fn preparation(&self) -> &FieldSpan {
        &self.preparation
    }

    /// Ready slots inherit boundary.next when present and current otherwise.
    /// # Errors
    /// Layout errors or a different original result source.
    pub fn authorized<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        schedule: &ScheduleRanges,
    ) -> Result<FieldSpan, Error> {
        reader.select_span(region, self.present(), &self.next, &schedule.current)
    }
}

/// Exact six native timing/payload/epoch parameters, in declared field order.
#[derive(Clone, Debug)]
pub struct ParamsCells {
    values: [Uint<Fp, 64>; 6],
}

impl ParamsCells {
    fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
    ) -> Result<Self, Error> {
        reader.exact_len(region, span, 50)?;
        let first = reader.bytes::<32>(region, span, 0)?;
        let last = reader.bytes::<32>(region, span, 18)?;
        for i in 0..14 {
            GlueChip::assert_equal(region, &first[18 + i], &last[i])?;
        }
        let mut bytes = first.to_vec();
        bytes.extend_from_slice(&last[14..]);
        let mut values = Vec::with_capacity(6);
        for (position, width) in [(0, 8), (9, 8), (18, 8), (27, 8), (36, 4), (41, 8)] {
            reader.constant_when(region, span.present(), &bytes[position], width as u64)?;
            values.push(reader.pack_le(region, &bytes[position + 1..position + 1 + width])?);
        }
        Ok(Self {
            values: values.try_into().map_err(|_| Error::Synthesis)?,
        })
    }
    /// Block time, retry, execution budget, apply budget, max payload, epoch length.
    pub const fn values(&self) -> &[Uint<Fp, 64>; 6] {
        &self.values
    }
    /// Preserve all lag-two parameters exactly across authenticated successors.
    /// # Errors
    /// A changed parameter is unsatisfied.
    pub fn assert_equal(&self, region: &mut Region<'_, Fp>, other: &Self) -> Result<(), Error> {
        for (a, b) in self.values.iter().zip(&other.values) {
            GlueChip::assert_equal(region, a.word(), b.word())?;
        }
        Ok(())
    }
}

/// One exact compact Ready/PendingBoundary slot extracted from R.
#[derive(Clone, Debug)]
pub struct SlotCells {
    body: FieldSpan,
    pending: Bit<Fp>,
    height: Uint<Fp, 64>,
    boundary_height: Uint<Fp, 64>,
    predecessor: [Word<Fp>; 32],
    params: ParamsCells,
}

impl SlotCells {
    /// Decode the native compact slot (Ready64 bytes, Pending106 bytes).
    /// Named fields immediately follow the LE32 tag; there is no extra wrapper.
    /// # Errors
    /// Layout errors; wrong tags/lengths/field widths are unsatisfied.
    pub fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        body: &FieldSpan,
    ) -> Result<Self, Error> {
        let tag = reader.bytes::<4>(region, body, 0)?;
        let tag = reader.pack_le(region, &tag)?;
        let pending = reader
            .uint
            .glue()
            .boolean(region, tag.value().map(|tag| tag == 1))?;
        GlueChip::assert_equal(region, pending.word(), tag.word())?;
        let expected_len =
            reader
                .uint
                .glue()
                .linear(region, &[(Fp::from(42), pending.word())], Fp::from(64))?;
        reader.equal_when(region, body.present(), body.len().word(), &expected_len)?;
        let height = reader.bytes::<9>(region, body, 4)?;
        reader.constant_when(region, body.present(), &height[0], 8)?;
        let height = reader.pack_le(region, &height[1..])?;
        let pending_body = reader.only_when(region, body, &pending)?;
        let boundary = reader.bytes::<9>(region, &pending_body, 13)?;
        reader.constant_when(region, pending_body.present(), &boundary[0], 8)?;
        let boundary_height = reader.pack_le(region, &boundary[1..])?;
        let prefix = reader.bytes::<1>(region, &pending_body, 22)?;
        reader.constant_when(region, pending_body.present(), &prefix[0], 32)?;
        let predecessor = reader.bytes::<32>(region, &pending_body, 23)?;
        let offset =
            reader
                .uint
                .glue()
                .linear(region, &[(Fp::from(42), pending.word())], Fp::from(13))?;
        let offset = reader.uint.range_check::<32>(region, &offset)?;
        let size = reader.uint.constant::<32>(region, 51)?;
        let params_container = reader.subrange(region, body, &offset, &size)?;
        let mut cursor = reader.enter(region, &params_container)?;
        let params = reader.field(region, &mut cursor)?;
        reader.finish(region, &cursor)?;
        let params = ParamsCells::parse(reader, region, &params)?;
        Ok(Self {
            body: body.clone(),
            pending,
            height,
            boundary_height,
            predecessor,
            params,
        })
    }
    /// Exact native barrier flag; true slots confer no signing authority.
    pub const fn pending(&self) -> &Bit<Fp> {
        &self.pending
    }
    /// Governed height from the same original slot bytes.
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Original pending boundary height, zero for Ready.
    pub const fn boundary_height(&self) -> &Uint<Fp, 64> {
        &self.boundary_height
    }
    /// Original pending predecessor context, zero for Ready.
    pub const fn predecessor(&self) -> &[Word<Fp>; 32] {
        &self.predecessor
    }
    /// Exact native lag-two parameters.
    pub const fn params(&self) -> &ParamsCells {
        &self.params
    }
}
