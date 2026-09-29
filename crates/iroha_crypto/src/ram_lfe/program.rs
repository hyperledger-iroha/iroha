//! The sole immutable, clearing owner and canonical codec for hidden programs.

use super::{
    BFV_PROGRAM_DIGEST_DOMAIN, BFV_PROGRAM_MAX_INSTRUCTIONS, BFV_PROGRAM_REGISTER_COUNT_U16,
    BFV_PROGRAM_STATE_WIDTH_U16, Hash, HiddenRamFheInstruction, RamLfeError, invalid_program_error,
    policy_secret, validate_hidden_program,
};
use norito::core::{DecodeFromSlice, SerializePayload};
use std::{fmt, str::FromStr, sync::Arc};
use zeroize::{Zeroize, Zeroizing};

const WORDS_PER_INSTRUCTION: usize = 6;
const BYTES_PER_INSTRUCTION: usize = WORDS_PER_INSTRUCTION * 8;
const MAX_TAPE_BYTES: usize = BFV_PROGRAM_MAX_INSTRUCTIONS * BYTES_PER_INSTRUCTION;
/// Maximum canonical frame accepted for a hidden programmed RAM-LFE tape.
pub const RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES: usize = 40 + 2 + 3 + 3 + 2 + 8 + MAX_TAPE_BYTES;

#[derive(PartialEq, Eq)]
struct Tape {
    bytes: Zeroizing<Vec<u8>>,
    count: usize,
}

impl Tape {
    fn new() -> Result<Self, RamLfeError> {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.try_reserve_exact(MAX_TAPE_BYTES).map_err(|error| {
            invalid_program_error(&format!("hidden program allocation failed: {error}"))
        })?;
        bytes.resize(MAX_TAPE_BYTES, 0);
        Ok(Self { bytes, count: 0 })
    }

    fn initialized(&self) -> &[u8] {
        &self.bytes[..self.count * BYTES_PER_INSTRUCTION]
    }
}

impl Drop for Tape {
    fn drop(&mut self) {
        self.bytes.as_mut_slice().zeroize();
        self.count.zeroize();
        #[cfg(test)]
        observe_clear(&self.bytes);
    }
}

#[derive(PartialEq, Eq)]
struct Program {
    version: u8,
    register_count: u16,
    memory_lane_count: u16,
    tape: Tape,
}

/// Validated hidden program with a single shared, clearing instruction allocation.
///
/// Use [`Self::builder`] for typed instructions or [`Self::from_bytes`] for a
/// canonical private frame. Clones share the allocation; Debug never prints the
/// tape. Instructions read from the tape and explicitly serialized bytes remain
/// private material. Clearing covers the owned tape and byte buffers; typed
/// instruction values and compiler-created copies require separate care.
/// Generic archive decoders are deliberately unavailable because their scratch
/// ownership cannot provide this type's private-frame clearing guarantee.
///
/// ```
/// use iroha_crypto::{HiddenRamFheInstruction, HiddenRamFheProgram};
/// let mut tape = HiddenRamFheProgram::builder()?;
/// tape.push(HiddenRamFheInstruction::LoadInput(0, 0))?;
/// tape.push(HiddenRamFheInstruction::Output(0))?;
/// let program = tape.finish()?;
/// let private_frame = program.to_bytes()?;
/// assert_eq!(HiddenRamFheProgram::from_bytes(&private_frame)?, program);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ```compile_fail
/// let _: iroha_crypto::HiddenRamFheProgram = norito::decode_from_bytes(&[]).unwrap();
/// ```
#[derive(Clone, PartialEq, Eq, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::HiddenRamFheProgramV1",
    frame = "iroha_crypto::ram_lfe::HiddenRamFheProgramV1"
)]
pub struct HiddenRamFheProgram(Arc<Program>);

impl HiddenRamFheProgram {
    /// Start a bounded tape builder for the compiled first-release profile.
    ///
    /// # Errors
    /// Returns an allocation error before accepting any private instructions.
    pub fn builder() -> Result<HiddenRamFheProgramBuilder, RamLfeError> {
        Ok(HiddenRamFheProgramBuilder { tape: Tape::new()? })
    }

    /// Return the fixed program format version.
    #[must_use]
    pub fn version(&self) -> u8 {
        self.0.version
    }

    /// Return the compiled register count.
    #[must_use]
    pub fn register_count(&self) -> u16 {
        self.0.register_count
    }

    /// Return the compiled persisted-memory lane count.
    #[must_use]
    pub fn memory_lane_count(&self) -> u16 {
        self.0.memory_lane_count
    }

    /// Return the number of private instructions in this tape.
    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.0.tape.count
    }

    /// Read a typed instruction. The returned value is private caller-owned material.
    #[must_use]
    pub fn instruction(&self, index: usize) -> Option<HiddenRamFheInstruction> {
        let start = index.checked_mul(BYTES_PER_INSTRUCTION)?;
        let end = start.checked_add(BYTES_PER_INSTRUCTION)?;
        let slot = self.0.tape.initialized().get(start..end)?;
        Some(decode_instruction(slot).expect("validated canonical private instruction"))
    }

    /// Iterate over private instructions without cloning the tape allocation.
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = HiddenRamFheInstruction> + '_ {
        (0..self.instruction_count()).map(|index| self.instruction(index).expect("bounded index"))
    }

    /// Encode into a clearing private byte owner using the canonical Norito frame.
    ///
    /// # Errors
    /// Returns an encoding or bounded-allocation error.
    pub fn to_bytes(&self) -> Result<Zeroizing<Vec<u8>>, norito::core::Error> {
        let length = norito::canonical_frame_len(self)?;
        if length > RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES {
            return Err(norito::core::Error::LengthMismatch);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(length)
            .map_err(|_| norito::core::Error::LengthMismatch)?;
        bytes.resize(length, 0);
        let mut writer = std::io::Cursor::new(bytes.as_mut_slice());
        norito::core::write_canonical_to_writer(self, &mut writer)?;
        if writer.position() != u64::try_from(length).expect("bounded frame") {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(bytes)
    }

    /// Decode one bounded canonical private frame directly into clearing storage.
    ///
    /// # Errors
    /// Rejects other frame identities/layouts, malformed fields and invalid programs.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RamLfeError> {
        if bytes.len() > RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES {
            return Err(invalid_program_error(
                "hidden program frame exceeds byte limit",
            ));
        }
        let view = norito::core::from_bytes_view(bytes).map_err(|error| codec_error(&error))?;
        if view.schema() != norito::schema::identity::frame_hash::<Self>() {
            return Err(codec_error(&norito::core::Error::SchemaMismatch));
        }
        // This owner's canonical frame is byte-aligned after the 40-byte header.
        // Reject surplus leading padding before the private allocation is made.
        if bytes.len() != norito::core::Header::SIZE + view.as_bytes().len() {
            return Err(codec_error(&norito::core::Error::LengthMismatch));
        }
        if view.flags() != norito::core::default_encode_flags() {
            return Err(codec_error(&norito::core::Error::NonCanonicalEncoding));
        }
        // `decode_unchecked` omits only schema checking, performed above. Norito
        // still scopes decode limits/flags and enforces full payload consumption.
        let DecodedOwner(program) = view
            .decode_unchecked::<DecodedOwner>()
            .map_err(|error| codec_error(&error))?;
        norito::verify_exact_canonical_frame(&program, bytes)
            .map_err(|error| codec_error(&error))?;
        Ok(program)
    }

    /// Return the public digest of the canonical hidden program commitment.
    ///
    /// # Errors
    /// Returns the underlying canonical encoding error.
    pub fn digest(&self) -> Result<Hash, norito::core::Error> {
        let commitment = policy_secret::commit_canonical(policy_secret::PROGRAM_CONTEXT, self)?;
        Ok(Hash::new_from_chunks(&[
            BFV_PROGRAM_DIGEST_DOMAIN,
            &commitment,
        ]))
    }

    fn encoding(&self) -> ProgramEncoding<'_> {
        ProgramEncoding {
            version: self.version(),
            register_count: self.register_count(),
            memory_lane_count: self.memory_lane_count(),
            tape: self.0.tape.initialized(),
        }
    }
}

impl fmt::Debug for HiddenRamFheProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED hidden RAM-FHE program]")
    }
}

/// Bounded typed builder whose private tape clears on failure or abandonment.
///
/// Instructions are written directly into the final tape allocation. This does
/// not take ownership of any copies the caller retained before calling `push`.
pub struct HiddenRamFheProgramBuilder {
    tape: Tape,
}

impl HiddenRamFheProgramBuilder {
    /// Append a typed instruction without reallocating private tape storage.
    ///
    /// # Errors
    /// Rejects the 257th instruction before writing it; `finish` validates semantics.
    pub fn push(&mut self, instruction: HiddenRamFheInstruction) -> Result<(), RamLfeError> {
        if self.tape.count == BFV_PROGRAM_MAX_INSTRUCTIONS {
            return Err(invalid_program_error(
                "program instruction tape exceeds maximum 256 instructions",
            ));
        }
        let fields = Zeroizing::new(instruction_fields(instruction));
        let start = self.tape.count * BYTES_PER_INSTRUCTION;
        for (target, word) in self.tape.bytes[start..start + BYTES_PER_INSTRUCTION]
            .chunks_exact_mut(8)
            .zip(fields.iter())
        {
            let encoded = Zeroizing::new(word.to_le_bytes());
            target.copy_from_slice(&*encoded);
        }
        self.tape.count += 1;
        Ok(())
    }

    /// Validate the complete tape and transfer its sole allocation into a shared owner.
    ///
    /// # Errors
    /// Rejects empty/no-output tapes, invalid indexes/immediates and depth/output excess.
    pub fn finish(self) -> Result<HiddenRamFheProgram, RamLfeError> {
        let program = HiddenRamFheProgram(Arc::new(Program {
            version: 1,
            register_count: BFV_PROGRAM_REGISTER_COUNT_U16,
            memory_lane_count: BFV_PROGRAM_STATE_WIDTH_U16,
            tape: self.tape,
        }));
        validate_hidden_program(&program)?;
        Ok(program)
    }
}

impl fmt::Debug for HiddenRamFheProgramBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED hidden RAM-FHE program builder]")
    }
}

#[derive(norito::SerializePayload)]
struct ProgramEncoding<'a> {
    version: u8,
    register_count: u16,
    memory_lane_count: u16,
    tape: &'a [u8],
}

impl SerializePayload for HiddenRamFheProgram {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        self.encoding().serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoding().encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoding().encoded_len_exact()
    }
}

struct DecodedOwner(HiddenRamFheProgram);
impl<'a> DecodeFromSlice<'a> for DecodedOwner {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_payload(bytes).map(|(value, used)| (Self(value), used))
    }
}

fn decode_payload(bytes: &[u8]) -> Result<(HiddenRamFheProgram, usize), norito::core::Error> {
    use norito::core::Error;
    if bytes.len() > RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES
        || norito::core::effective_decode_flags() != Some(norito::core::default_encode_flags())
    {
        return Err(Error::NonCanonicalEncoding);
    }
    let mut remaining = bytes;
    let mut field = || -> Result<&[u8], Error> {
        let (length, prefix) = norito::core::inspect_len_from_slice(remaining)?;
        let end = prefix.checked_add(length).ok_or(Error::LengthMismatch)?;
        let value = remaining.get(prefix..end).ok_or(Error::LengthMismatch)?;
        remaining = remaining.get(end..).ok_or(Error::LengthMismatch)?;
        Ok(value)
    };
    let version = field()?;
    let registers = field()?;
    let lanes = field()?;
    let tape = field()?;
    if version != [1]
        || registers != BFV_PROGRAM_REGISTER_COUNT_U16.to_le_bytes()
        || lanes != BFV_PROGRAM_STATE_WIDTH_U16.to_le_bytes()
    {
        return Err(Error::Message(
            "hidden program metadata does not match compiled profile".into(),
        ));
    }
    if !remaining.is_empty() || tape.len() < 8 {
        return Err(Error::LengthMismatch);
    }
    let length = usize::try_from(u64::from_le_bytes(
        tape[..8].try_into().expect("checked length"),
    ))
    .map_err(|_| Error::LengthMismatch)?;
    let private = &tape[8..];
    if length != private.len()
        || length == 0
        || length > MAX_TAPE_BYTES
        || !length.is_multiple_of(BYTES_PER_INSTRUCTION)
    {
        return Err(Error::LengthMismatch);
    }
    // Check every tag and unused word before allocating or copying a private tape.
    for slot in private.chunks_exact(BYTES_PER_INSTRUCTION) {
        decode_instruction(slot)?;
    }
    // Account for the fixed tape, shared owner and semantic-validation scratch
    // in every active Norito budget before the private copy is allocated.
    let allocation = MAX_TAPE_BYTES
        + std::mem::size_of::<Program>()
        + 2 * std::mem::size_of::<usize>()
        + usize::from(BFV_PROGRAM_REGISTER_COUNT_U16 + BFV_PROGRAM_STATE_WIDTH_U16)
            * std::mem::size_of::<u16>();
    norito::core::reserve_decode_allocation(allocation)?;
    let mut builder =
        HiddenRamFheProgram::builder().map_err(|error| program_codec_error(&error))?;
    builder.tape.bytes[..length].copy_from_slice(private);
    builder.tape.count = length / BYTES_PER_INSTRUCTION;
    let program = builder
        .finish()
        .map_err(|error| program_codec_error(&error))?;
    norito::core::note_payload_access(bytes, bytes.len());
    Ok((program, bytes.len()))
}

pub(super) fn instruction_fields(
    instruction: HiddenRamFheInstruction,
) -> [u64; WORDS_PER_INSTRUCTION] {
    use HiddenRamFheInstruction::*;
    match instruction {
        LoadInput(a, b) => [0, a.into(), b.into(), 0, 0, 0],
        LoadState(a, b) => [1, a.into(), b.into(), 0, 0, 0],
        StoreState(a, b) => [2, a.into(), b.into(), 0, 0, 0],
        LoadConst(a, b) => [3, a.into(), b, 0, 0, 0],
        Add(a, b, c) => [4, a.into(), b.into(), c.into(), 0, 0],
        AddPlain(a, b, c) => [5, a.into(), b.into(), c, 0, 0],
        SubPlain(a, b, c) => [6, a.into(), b.into(), c, 0, 0],
        MulPlain(a, b, c) => [7, a.into(), b.into(), c, 0, 0],
        Mul(a, b, c) => [8, a.into(), b.into(), c.into(), 0, 0],
        SelectEqZero(a, b, c, d) => [9, a.into(), b.into(), c.into(), d.into(), 0],
        Output(a) => [10, a.into(), 0, 0, 0, 0],
    }
}

fn decode_instruction(slot: &[u8]) -> Result<HiddenRamFheInstruction, norito::core::Error> {
    use HiddenRamFheInstruction::*;
    use norito::core::Error;
    if slot.len() != BYTES_PER_INSTRUCTION {
        return Err(Error::LengthMismatch);
    }
    let mut words = Zeroizing::new([0_u64; WORDS_PER_INSTRUCTION]);
    for (value, bytes) in words.iter_mut().zip(slot.chunks_exact(8)) {
        *value = u64::from_le_bytes(bytes.try_into().expect("fixed word"));
    }
    let index =
        |offset: usize| u16::try_from(words[offset]).map_err(|_| Error::NonCanonicalEncoding);
    let (value, used) = match words[0] {
        0 => (LoadInput(index(1)?, index(2)?), 3),
        1 => (LoadState(index(1)?, index(2)?), 3),
        2 => (StoreState(index(1)?, index(2)?), 3),
        3 => (LoadConst(index(1)?, words[2]), 3),
        4 => (Add(index(1)?, index(2)?, index(3)?), 4),
        5 => (AddPlain(index(1)?, index(2)?, words[3]), 4),
        6 => (SubPlain(index(1)?, index(2)?, words[3]), 4),
        7 => (MulPlain(index(1)?, index(2)?, words[3]), 4),
        8 => (Mul(index(1)?, index(2)?, index(3)?), 4),
        9 => (SelectEqZero(index(1)?, index(2)?, index(3)?, index(4)?), 5),
        10 => (Output(index(1)?), 2),
        _ => return Err(Error::NonCanonicalEncoding),
    };
    if words[used..].iter().any(|&word| word != 0) {
        return Err(Error::NonCanonicalEncoding);
    }
    Ok(value)
}

fn codec_error(error: &norito::core::Error) -> RamLfeError {
    RamLfeError::TranscriptEncoding(error.to_string())
}
fn program_codec_error(error: &RamLfeError) -> norito::core::Error {
    norito::core::Error::Message(error.to_string())
}

impl FromStr for HiddenRamFheProgram {
    type Err = RamLfeError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let literal = value.strip_prefix("0x").ok_or_else(|| {
            invalid_program_error("hidden program must be exact 0x-prefixed lowercase hex")
        })?;
        if literal.is_empty()
            || literal.len() > RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES * 2
            || !literal.len().is_multiple_of(2)
            || !literal
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid_program_error(
                "hidden program requires bounded non-empty lowercase hex",
            ));
        }
        norito::core::reserve_decode_allocation(literal.len() / 2)
            .map_err(|error| codec_error(&error))?;
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(literal.len() / 2)
            .map_err(|_| invalid_program_error("hidden program byte allocation failed"))?;
        bytes.resize(literal.len() / 2, 0);
        hex::decode_to_slice(literal, &mut bytes)
            .map_err(|_| invalid_program_error("hidden program hexadecimal decode failed"))?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for HiddenRamFheProgram {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        // Hex has no characters requiring JSON escapes. Borrow its one canonical
        // spelling, avoiding a generic string decoder's private allocation.
        let raw = parser.raw_value_slice()?;
        let text = raw
            .strip_prefix('"')
            .and_then(|raw| raw.strip_suffix('"'))
            .ok_or_else(|| {
                norito::json::Error::Message("hidden program must be a lowercase hex string".into())
            })?;
        text.parse()
            .map_err(|error: RamLfeError| norito::json::Error::Message(error.to_string()))
    }
}

#[cfg(test)]
thread_local! {
    static CLEARED: std::cell::RefCell<Option<usize>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
fn observe_clear(bytes: &[u8]) {
    CLEARED.with_borrow_mut(|value| {
        if let Some(count) = value {
            assert!(bytes.iter().all(|&byte| byte == 0));
            *count += bytes.len();
        }
    });
}

/// Build deliberately invalid state from fixed public adversarial test inputs.
#[cfg(test)]
pub(super) fn from_public_test_parts(
    version: u8,
    register_count: u16,
    memory_lane_count: u16,
    instructions: &[HiddenRamFheInstruction],
) -> HiddenRamFheProgram {
    let mut bytes = Zeroizing::new(vec![0; instructions.len() * BYTES_PER_INSTRUCTION]);
    for (slot, instruction) in bytes
        .chunks_exact_mut(BYTES_PER_INSTRUCTION)
        .zip(instructions)
    {
        let words = Zeroizing::new(instruction_fields(*instruction));
        for (word, target) in words.iter().zip(slot.chunks_exact_mut(8)) {
            target.copy_from_slice(&word.to_le_bytes());
        }
    }
    HiddenRamFheProgram(Arc::new(Program {
        version,
        register_count,
        memory_lane_count,
        tape: Tape {
            bytes,
            count: instructions.len(),
        },
    }))
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
