//! Canonical field cursors over membership-checked bytes of one result tape.
//!
//! Cursors retain source root/length and enclosing bounds. Conditional containers
//! use constrained presence bits and a fixed layout, including absent boundaries.
//! Every offset is derived from checked original byte lengths, never host decoding.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, Uint, UintChip, Word, WordHasher};

use super::{
    super::result::{CompactResultLength, RESULT_CODEC_ID},
    tape::{ResultTape, ResultTapeWitness},
};

/// A length-delimited original byte range, bound to one exact tape context.
#[derive(Clone, Debug)]
pub struct FieldSpan {
    root: Word<Fp>,
    frame_len: Uint<Fp, 32>,
    start: Uint<Fp, 32>,
    len: Uint<Fp, 32>,
    end: Uint<Fp, 32>,
    present: Bit<Fp>,
}

impl FieldSpan {
    /// Constrained payload start within original R (excluding hash domain).
    pub const fn start(&self) -> &Uint<Fp, 32> {
        &self.start
    }
    /// Constrained exact payload length.
    pub const fn len(&self) -> &Uint<Fp, 32> {
        &self.len
    }
    /// One past this payload's last byte.
    pub const fn end(&self) -> &Uint<Fp, 32> {
        &self.end
    }
    /// Presence inherited from its containing original option.
    pub const fn present(&self) -> &Bit<Fp> {
        &self.present
    }
}

/// A checked cursor inside a bounded original container.
#[derive(Clone, Debug)]
pub struct FieldCursor {
    container: FieldSpan,
    position: Uint<Fp, 32>,
}

/// Decoder view using an untrusted witness generator and constrained tape owner.
/// The tape itself must be authenticated by the complete native result hash scan.
#[derive(Debug)]
pub struct ScheduleReader<'a, 'chip, H: WordHasher<Fp>> {
    /// Original root and bounded frame length shared with the result hash scan.
    pub tape: &'a ResultTape,
    /// Supplies openings only; no decoded host fields influence constraints.
    pub witness: &'a ResultTapeWitness,
    /// Shared arithmetic and range chips.
    pub uint: &'a mut UintChip<'chip, Fp>,
    /// Shared constrained Poseidon chip.
    pub hash: &'a mut H,
}

impl<H: WordHasher<Fp>> ScheduleReader<'_, '_, H> {
    fn bind(&self, region: &mut Region<'_, Fp>, span: &FieldSpan) -> Result<(), Error> {
        GlueChip::assert_equal(region, &span.root, self.tape.root())?;
        GlueChip::assert_equal(region, span.frame_len.word(), self.tape.frame_len().word())
    }

    /// Require two retained descriptors to refer to the exact same source field.
    /// # Errors
    /// A source, range or presence substitution is unsatisfied.
    pub fn same_span(
        &self,
        region: &mut Region<'_, Fp>,
        a: &FieldSpan,
        b: &FieldSpan,
    ) -> Result<(), Error> {
        self.bind(region, a)?;
        self.bind(region, b)?;
        for (a, b) in [
            (a.start.word(), b.start.word()),
            (a.len.word(), b.len.word()),
            (a.end.word(), b.end.word()),
            (a.present.word(), b.present.word()),
        ] {
            GlueChip::assert_equal(region, a, b)?;
        }
        Ok(())
    }

    /// Require an equality exactly when the containing source field is present.
    /// # Errors
    /// Layout errors; a changed present field is unsatisfied.
    pub fn equal_when(
        &mut self,
        region: &mut Region<'_, Fp>,
        present: &Bit<Fp>,
        a: &Word<Fp>,
        b: &Word<Fp>,
    ) -> Result<(), Error> {
        let difference = self.uint.glue().sub(region, a, b)?;
        let active = self.uint.glue().mul(region, present.word(), &difference)?;
        GlueChip::assert_constant(region, &active, Fp::ZERO)
    }

    /// Require a fixed native framing byte/value on a present field.
    /// # Errors
    /// Layout errors; incorrect present framing is unsatisfied.
    pub fn constant_when(
        &mut self,
        region: &mut Region<'_, Fp>,
        present: &Bit<Fp>,
        value: &Word<Fp>,
        expected: u64,
    ) -> Result<(), Error> {
        let constant = self.uint.glue().constant(region, Fp::from(expected))?;
        self.equal_when(region, present, value, &constant)
    }

    /// The original result's canonical payload container, checking its fixed
    /// 40-byte header identity, flags and exact payload length on the same tape.
    /// # Errors
    /// Layout errors; a frame shorter than its fixed header is unsatisfied.
    pub fn result_payload(&mut self, region: &mut Region<'_, Fp>) -> Result<FieldCursor, Error> {
        let zero = self.uint.constant::<32>(region, 0)?;
        let header = self
            .witness
            .read::<32>(self.tape, self.uint, self.hash, region, &zero)?;
        for (i, expected) in b"NRT0\0\0".iter().chain(RESULT_CODEC_ID.iter()).enumerate() {
            GlueChip::assert_constant(region, &header[i], Fp::from(u64::from(*expected)))?;
        }
        GlueChip::assert_constant(region, &header[22], Fp::ZERO)?;
        let end_offset = self.uint.constant::<32>(region, 32)?;
        let end_header =
            self.witness
                .read::<8>(self.tape, self.uint, self.hash, region, &end_offset)?;
        GlueChip::assert_constant(region, &end_header[7], Fp::from(2))?;
        let payload_len = self.pack_le(region, &header[23..31])?;
        let payload_len = self.uint.range_check::<32>(region, payload_len.word())?;
        let original_len = self.uint.checked_add_constant(region, &payload_len, 40)?;
        GlueChip::assert_equal(region, original_len.word(), self.tape.frame_len().word())?;
        let start = self.uint.constant::<32>(region, 40)?;
        let len = self
            .uint
            .checked_sub(region, self.tape.frame_len(), &start)?;
        let present = self
            .uint
            .glue()
            .boolean(region, iroha_plonk::frontend::Value::known(true))?;
        GlueChip::assert_constant(region, present.word(), Fp::ONE)?;
        let span = FieldSpan {
            root: self.tape.root().clone(),
            frame_len: self.tape.frame_len().clone(),
            start: start.clone(),
            len,
            end: self.tape.frame_len().clone(),
            present,
        };
        Ok(FieldCursor {
            container: span,
            position: start,
        })
    }

    /// Enter one original field's payload as a nested struct container.
    /// # Errors
    /// A different tape context is unsatisfied.
    pub fn enter(
        &self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
    ) -> Result<FieldCursor, Error> {
        self.bind(region, span)?;
        Ok(FieldCursor {
            container: span.clone(),
            position: span.start.clone(),
        })
    }

    /// Require every byte of this struct container to be consumed exactly.
    /// # Errors
    /// Layout errors; omitted/trailing fields are unsatisfied when present.
    pub fn finish(
        &mut self,
        region: &mut Region<'_, Fp>,
        cursor: &FieldCursor,
    ) -> Result<(), Error> {
        self.bind(region, &cursor.container)?;
        self.equal_when(
            region,
            &cursor.container.present,
            cursor.position.word(),
            cursor.container.end.word(),
        )
    }

    /// Read the next canonical length-prefixed field and advance the cursor.
    /// # Errors
    /// Layout errors; malformed compact length or container overrun is unsatisfied.
    pub fn field(
        &mut self,
        region: &mut Region<'_, Fp>,
        cursor: &mut FieldCursor,
    ) -> Result<FieldSpan, Error> {
        self.bind(region, &cursor.container)?;
        let zero = self.uint.constant::<32>(region, 0)?;
        let safe = self.uint.glue().select(
            region,
            &cursor.container.present,
            cursor.position.word(),
            zero.word(),
        )?;
        let safe = self.uint.range_check::<32>(region, &safe)?;
        let source = self
            .witness
            .read::<3>(self.tape, self.uint, self.hash, region, &safe)?;
        let mut normalized = Vec::with_capacity(3);
        for byte in source {
            normalized.push(self.uint.glue().mul(
                region,
                cursor.container.present.word(),
                &byte,
            )?);
        }
        let normalized = normalized.try_into().map_err(|_| Error::Synthesis)?;
        let length = CompactResultLength::from_window(self.uint, region, &normalized)?;
        let width = UintChip::widen::<8, 32>(length.encoded_bytes());
        let start = self.uint.checked_add(region, &cursor.position, &width)?;
        let end = self.uint.checked_add(region, &start, length.value())?;
        let outside = self.uint.lt(region, &cursor.container.end, &end)?;
        let fits = self.uint.glue().not(region, &outside)?;
        self.constant_when(region, &cursor.container.present, fits.word(), 1)?;
        let start = self.uint.glue().select(
            region,
            &cursor.container.present,
            start.word(),
            zero.word(),
        )?;
        let end =
            self.uint
                .glue()
                .select(region, &cursor.container.present, end.word(), zero.word())?;
        let start = self.uint.range_check::<32>(region, &start)?;
        let end = self.uint.range_check::<32>(region, &end)?;
        cursor.position = end.clone();
        Ok(FieldSpan {
            root: cursor.container.root.clone(),
            frame_len: cursor.container.frame_len.clone(),
            start,
            len: length.value().clone(),
            end,
            present: cursor.container.present.clone(),
        })
    }

    /// Read a fixed-size window within a source field, without host offset choices.
    /// Absent fields return constrained zero bytes using safe dummy source reads.
    /// # Errors
    /// Layout errors; an active out-of-container read is unsatisfied.
    pub fn bytes<const N: usize>(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
        relative: u32,
    ) -> Result<[Word<Fp>; N], Error> {
        self.bind(region, span)?;
        let relative_end = self
            .uint
            .constant::<32>(region, u128::from(relative) + N as u128)?;
        let outside = self.uint.lt(region, &span.len, &relative_end)?;
        let fits = self.uint.glue().not(region, &outside)?;
        self.constant_when(region, &span.present, fits.word(), 1)?;
        let offset = self
            .uint
            .checked_add_constant(region, &span.start, u128::from(relative))?;
        let zero = self.uint.constant::<32>(region, 0)?;
        let offset = self
            .uint
            .glue()
            .select(region, &span.present, offset.word(), zero.word())?;
        let offset = self.uint.range_check::<32>(region, &offset)?;
        let source = self
            .witness
            .read::<N>(self.tape, self.uint, self.hash, region, &offset)?;
        let mut result = Vec::with_capacity(N);
        for byte in source {
            result.push(self.uint.glue().mul(region, span.present.word(), &byte)?);
        }
        result.try_into().map_err(|_| Error::Synthesis)
    }

    /// Require an exact native payload width, conditional on source presence.
    /// # Errors
    /// Layout errors; a different present field width is unsatisfied.
    pub fn exact_len(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
        expected: u32,
    ) -> Result<(), Error> {
        self.bind(region, span)?;
        self.constant_when(region, &span.present, span.len.word(), u64::from(expected))
    }

    /// Decode a canonical Option tag and its single length-delimited Some body.
    /// # Errors
    /// Layout errors; nonboolean tags, trailing None/Some bytes are unsatisfied.
    pub fn option(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
    ) -> Result<FieldSpan, Error> {
        self.tagged_body::<1>(region, span)
    }

    /// Decode a two-variant tag with an empty first variant and one enclosed
    /// body in the second. Native Option uses1 byte, beacon enum uses4.
    /// # Errors
    /// Layout errors; invalid tags or trailing variant data are unsatisfied.
    pub fn tagged_body<const TAG_BYTES: usize>(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
    ) -> Result<FieldSpan, Error> {
        let tag = self.bytes::<TAG_BYTES>(region, span, 0)?;
        let tag = self.pack_le(region, &tag)?;
        let some = self
            .uint
            .glue()
            .boolean(region, tag.value().map(|tag| tag == 1))?;
        GlueChip::assert_equal(region, some.word(), tag.word())?;
        let none = self.uint.glue().not(region, &some)?;
        let none = self.uint.glue().and(region, &span.present, &none)?;
        self.constant_when(region, &none, span.len.word(), TAG_BYTES as u64)?;
        let start = self
            .uint
            .checked_add_constant(region, &span.start, TAG_BYTES as u128)?;
        let mut cursor = FieldCursor {
            container: FieldSpan {
                present: some,
                ..span.clone()
            },
            position: start,
        };
        let payload = self.field(region, &mut cursor)?;
        self.finish(region, &cursor)?;
        Ok(payload)
    }

    /// Parse one exact unsigned little-endian scalar (1..=8 bytes).
    /// # Errors
    /// Layout errors; field width or byte range changes are unsatisfied.
    pub fn scalar<const N: usize>(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
    ) -> Result<Uint<Fp, 64>, Error> {
        if !(1..=8).contains(&N) {
            return Err(Error::Synthesis);
        }
        self.exact_len(region, span, N as u32)?;
        let bytes = self.bytes::<N>(region, span, 0)?;
        self.pack_le(region, &bytes)
    }

    /// Pack source-bound bytes into an exact little-endian u64.
    /// # Errors
    /// Layout errors or an input width outside one through eight bytes.
    pub fn pack_le(
        &mut self,
        region: &mut Region<'_, Fp>,
        bytes: &[Word<Fp>],
    ) -> Result<Uint<Fp, 64>, Error> {
        if bytes.is_empty() || bytes.len() > 8 {
            return Err(Error::Synthesis);
        }
        let mut packed = self.uint.glue().constant(region, Fp::ZERO)?;
        for byte in bytes.iter().rev() {
            self.uint.range_check::<8>(region, byte)?;
            packed = self.uint.glue().linear(
                region,
                &[(Fp::from(256), &packed), (Fp::ONE, byte)],
                Fp::ZERO,
            )?;
        }
        self.uint.range_check::<64>(region, &packed)
    }

    /// Derive a nested byte range without trusting a host offset.
    /// # Errors
    /// Layout errors; an active child outside its original container is unsatisfied.
    pub fn subrange(
        &mut self,
        region: &mut Region<'_, Fp>,
        parent: &FieldSpan,
        relative: &Uint<Fp, 32>,
        len: &Uint<Fp, 32>,
    ) -> Result<FieldSpan, Error> {
        self.bind(region, parent)?;
        let relative_end = self.uint.checked_add(region, relative, len)?;
        let outside = self.uint.lt(region, &parent.len, &relative_end)?;
        self.constant_when(region, &parent.present, outside.word(), 0)?;
        let start = self.uint.checked_add(region, &parent.start, relative)?;
        let end = self.uint.checked_add(region, &start, len)?;
        Ok(FieldSpan {
            root: parent.root.clone(),
            frame_len: parent.frame_len.clone(),
            start,
            len: len.clone(),
            end,
            present: parent.present.clone(),
        })
    }

    /// Select between two already-derived spans of the same original tape.
    /// # Errors
    /// Layout errors; crossing source contexts is unsatisfied.
    pub fn select_span(
        &mut self,
        region: &mut Region<'_, Fp>,
        select: &Bit<Fp>,
        yes: &FieldSpan,
        no: &FieldSpan,
    ) -> Result<FieldSpan, Error> {
        self.bind(region, yes)?;
        self.bind(region, no)?;
        let start = self
            .uint
            .glue()
            .select(region, select, yes.start.word(), no.start.word())?;
        let len = self
            .uint
            .glue()
            .select(region, select, yes.len.word(), no.len.word())?;
        let end = self
            .uint
            .glue()
            .select(region, select, yes.end.word(), no.end.word())?;
        let present =
            self.uint
                .glue()
                .select(region, select, yes.present.word(), no.present.word())?;
        let present_bit = self
            .uint
            .glue()
            .boolean(region, present.value().map(|value| value == Fp::ONE))?;
        GlueChip::assert_equal(region, &present, present_bit.word())?;
        Ok(FieldSpan {
            root: self.tape.root().clone(),
            frame_len: self.tape.frame_len().clone(),
            start: self.uint.range_check::<32>(region, &start)?,
            len: self.uint.range_check::<32>(region, &len)?,
            end: self.uint.range_check::<32>(region, &end)?,
            present: present_bit,
        })
    }

    /// Restrict a derived span to a constrained active branch or roster seat.
    /// # Errors
    /// Layout errors or a different source context.
    pub fn only_when(
        &mut self,
        region: &mut Region<'_, Fp>,
        span: &FieldSpan,
        active: &Bit<Fp>,
    ) -> Result<FieldSpan, Error> {
        self.bind(region, span)?;
        Ok(FieldSpan {
            present: self.uint.glue().and(region, &span.present, active)?,
            ..span.clone()
        })
    }
}
