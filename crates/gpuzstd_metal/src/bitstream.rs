#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BitstreamError {
    InvalidBits,
    ValueOverflow,
    NoSpace,
    UnexpectedEof,
}

const MAX_BITS: u32 = 56;

pub(crate) struct BitWriter {
    buffer: u64,
    bit_count: u32,
    out: Vec<u8>,
    max_bytes: usize,
}

impl BitWriter {
    pub(crate) fn with_capacity(max_bytes: usize) -> Self {
        Self {
            buffer: 0,
            bit_count: 0,
            out: Vec::with_capacity(max_bytes.min(64)),
            max_bytes,
        }
    }

    pub(crate) fn write_bits(&mut self, value: u64, bits: u32) -> Result<(), BitstreamError> {
        if bits > MAX_BITS {
            return Err(BitstreamError::InvalidBits);
        }
        if bits > 0 && bits < 64 && (value >> bits) != 0 {
            return Err(BitstreamError::ValueOverflow);
        }
        if bits == 0 {
            return Ok(());
        }
        self.buffer |= value << self.bit_count;
        self.bit_count += bits;
        while self.bit_count >= 8 {
            self.push_byte()?;
        }
        Ok(())
    }

    pub(crate) fn flush_byte_aligned(&mut self) -> Result<(), BitstreamError> {
        if self.bit_count == 0 {
            return Ok(());
        }
        if self.out.len() >= self.max_bytes {
            return Err(BitstreamError::NoSpace);
        }
        self.out.push((self.buffer & 0xff) as u8);
        self.buffer = 0;
        self.bit_count = 0;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<Vec<u8>, BitstreamError> {
        self.flush_byte_aligned()?;
        Ok(self.out)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.out
    }

    pub(crate) fn bit_count(&self) -> u32 {
        self.bit_count
    }

    fn push_byte(&mut self) -> Result<(), BitstreamError> {
        if self.out.len() >= self.max_bytes {
            return Err(BitstreamError::NoSpace);
        }
        self.out.push((self.buffer & 0xff) as u8);
        self.buffer >>= 8;
        self.bit_count -= 8;
        Ok(())
    }
}

pub(crate) struct BitReader<'a> {
    buffer: u64,
    bit_count: u32,
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            buffer: 0,
            bit_count: 0,
            data,
            pos: 0,
        }
    }

    pub(crate) fn read_bits(&mut self, bits: u32) -> Result<u64, BitstreamError> {
        if bits > MAX_BITS {
            return Err(BitstreamError::InvalidBits);
        }
        if bits == 0 {
            return Ok(0);
        }
        while self.bit_count < bits {
            if self.pos >= self.data.len() {
                return Err(BitstreamError::UnexpectedEof);
            }
            self.buffer |= (self.data[self.pos] as u64) << self.bit_count;
            self.bit_count += 8;
            self.pos += 1;
        }
        let mask = (1u64 << bits) - 1;
        let value = self.buffer & mask;
        self.buffer >>= bits;
        self.bit_count -= bits;
        Ok(value)
    }

    pub(crate) fn align_to_byte(&mut self) {
        let skip = self.bit_count & 7;
        self.buffer >>= skip;
        self.bit_count -= skip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_STEPS: &[(u64, u32)] = &[
        (0b101, 3),
        (0b10011, 5),
        (0b1110001, 7),
        (0b10, 2),
        (0b101111000011, 12),
        (0b111111, 6),
    ];
    const GOLDEN_BYTES: [u8; 5] = [0x9d, 0x71, 0x87, 0xf7, 0x07];

    #[test]
    fn bitwriter_packs_lsb_first_golden() {
        let mut writer = BitWriter::with_capacity(16);
        for (value, bits) in GOLDEN_STEPS {
            writer.write_bits(*value, *bits).expect("write bits");
        }
        let bytes = writer.finish().expect("finish writer");
        assert_eq!(bytes, GOLDEN_BYTES);
    }

    #[test]
    fn bitreader_roundtrips_golden() {
        let mut reader = BitReader::new(&GOLDEN_BYTES);
        for (value, bits) in GOLDEN_STEPS {
            let got = reader.read_bits(*bits).expect("read bits");
            assert_eq!(got, *value);
        }
        reader.align_to_byte();
        assert_eq!(reader.read_bits(1), Err(BitstreamError::UnexpectedEof));
    }

    #[test]
    fn bitwriter_flush_aligns_to_byte() {
        let mut writer = BitWriter::with_capacity(4);
        writer.write_bits(0b101, 3).expect("write bits");
        assert_eq!(writer.bit_count(), 3);
        writer.flush_byte_aligned().expect("flush");
        assert_eq!(writer.bytes(), &[0b101]);
        assert_eq!(writer.bit_count(), 0);
    }

    #[test]
    fn bitwriter_rejects_overflow_and_invalid_bits() {
        let mut writer = BitWriter::with_capacity(4);
        assert_eq!(
            writer.write_bits(0b10, 1),
            Err(BitstreamError::ValueOverflow)
        );
        assert_eq!(
            writer.write_bits(0, MAX_BITS + 1),
            Err(BitstreamError::InvalidBits)
        );
    }

    #[test]
    fn bitwriter_reports_no_space() {
        let mut writer = BitWriter::with_capacity(1);
        assert_eq!(writer.write_bits(0xffff, 16), Err(BitstreamError::NoSpace));
    }

    #[test]
    fn bitreader_reports_unexpected_eof() {
        let mut reader = BitReader::new(&[0xaa]);
        assert_eq!(reader.read_bits(9), Err(BitstreamError::UnexpectedEof));
    }
}
