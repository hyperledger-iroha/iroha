//! Clearing owners and bounded snapshots for the sole programmed interpreter.

use super::program::instruction_fields;
use super::{
    BFV_PROGRAM_MAX_INSTRUCTIONS, BFV_PROGRAM_REGISTER_COUNT, BFV_PROGRAM_STATE_WIDTH,
    BfvCiphertext, HiddenRamFheInstruction, RamLfeError, invalid_program_error,
};
use std::{
    fmt,
    ops::{Deref, DerefMut},
};
use zeroize::{Zeroize, Zeroizing};

const COEFFICIENTS_PER_CIPHERTEXT: usize = 128;
const SNAPSHOT_WIDTH: usize =
    (BFV_PROGRAM_REGISTER_COUNT + BFV_PROGRAM_STATE_WIDTH) * COEFFICIENTS_PER_CIPHERTEXT;
const INSTRUCTION_WIDTH: usize = 6;

pub(super) fn clear_ciphertext(value: &mut BfvCiphertext) {
    value.c0.as_mut_slice().zeroize();
    value.c1.as_mut_slice().zeroize();
    #[cfg(test)]
    observe_clear(value.c0.iter().chain(&value.c1).copied());
}

pub(super) struct OwnedCiphertext(pub(super) BfvCiphertext);

impl OwnedCiphertext {
    pub(super) fn copy(value: &BfvCiphertext) -> Result<Self, RamLfeError> {
        let mut result = Self(BfvCiphertext {
            c0: Vec::new(),
            c1: Vec::new(),
        });
        result
            .0
            .c0
            .try_reserve_exact(value.c0.len())
            .map_err(|error| allocation_error(&error))?;
        result
            .0
            .c1
            .try_reserve_exact(value.c1.len())
            .map_err(|error| allocation_error(&error))?;
        result.0.c0.extend_from_slice(&value.c0);
        result.0.c1.extend_from_slice(&value.c1);
        Ok(result)
    }

    fn into_inner(mut self) -> BfvCiphertext {
        BfvCiphertext {
            c0: std::mem::take(&mut self.0.c0),
            c1: std::mem::take(&mut self.0.c1),
        }
    }
}

impl Deref for OwnedCiphertext {
    type Target = BfvCiphertext;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for OwnedCiphertext {
    fn drop(&mut self) {
        clear_ciphertext(&mut self.0);
    }
}

pub(super) struct OwnedCiphertexts(Vec<BfvCiphertext>);

impl OwnedCiphertexts {
    pub(super) fn with_capacity(count: usize) -> Result<Self, RamLfeError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|error| allocation_error(&error))?;
        Ok(Self(values))
    }

    pub(super) fn from_vec(values: Vec<BfvCiphertext>) -> Self {
        Self(values)
    }

    pub(super) fn push(&mut self, value: OwnedCiphertext) -> Result<(), RamLfeError> {
        if self.0.len() == self.0.capacity() {
            return Err(invalid_program_error(
                "private ciphertext owner capacity exceeded",
            ));
        }
        self.0.push(value.into_inner());
        Ok(())
    }

    pub(super) fn replace(
        &mut self,
        index: usize,
        value: OwnedCiphertext,
    ) -> Result<(), RamLfeError> {
        let slot = self
            .0
            .get_mut(index)
            .ok_or_else(|| invalid_program_error("private ciphertext index out of bounds"))?;
        clear_ciphertext(slot);
        *slot = value.into_inner();
        Ok(())
    }

    pub(super) fn encode_output(mut self) -> Result<Vec<u8>, RamLfeError> {
        struct Output(super::super::BfvIdentifierCiphertext);
        impl Drop for Output {
            fn drop(&mut self) {
                for value in &mut self.0.slots {
                    clear_ciphertext(value);
                }
            }
        }
        let output = Output(super::super::BfvIdentifierCiphertext {
            slots: std::mem::take(&mut self.0),
        });
        let length = norito::canonical_frame_len(&output.0)
            .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))?;
        if length > super::MAX_INPUT_BYTES {
            return Err(invalid_program_error(
                "encoded output exceeds interpreter byte limit",
            ));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        bytes
            .try_reserve_exact(length)
            .map_err(|error| allocation_error(&error))?;
        bytes.resize(length, 0);
        let mut writer = std::io::Cursor::new(bytes.as_mut_slice());
        norito::core::write_canonical_to_writer(&output.0, &mut writer)
            .map_err(|error| RamLfeError::TranscriptEncoding(error.to_string()))?;
        if writer.position() != u64::try_from(length).expect("bounded output length") {
            return Err(invalid_program_error("encoded output length changed"));
        }
        // Only the completed, canonical public output leaves the clearing owner.
        Ok(std::mem::take(&mut *bytes))
    }
}

impl Deref for OwnedCiphertexts {
    type Target = [BfvCiphertext];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OwnedCiphertexts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for OwnedCiphertexts {
    fn drop(&mut self) {
        for value in &mut self.0 {
            clear_ciphertext(value);
        }
    }
}

/// Private, clearing snapshots emitted by the existing RAM-LFE interpreter.
///
/// This material is neither a proof nor a serialized receipt. It exposes private
/// witness values to its caller and must not be logged or persisted. Owning buffers
/// are cleared on drop; this does not guarantee erasure of caller or compiler copies.
pub struct RamLfeProgramExecutionTrace {
    snapshots: Zeroizing<Vec<u64>>,
    instructions: Zeroizing<Vec<u64>>,
    steps: usize,
    outputs: usize,
}

impl RamLfeProgramExecutionTrace {
    pub(super) fn new() -> Result<Self, RamLfeError> {
        Ok(Self {
            snapshots: zeroed((BFV_PROGRAM_MAX_INSTRUCTIONS + 1) * SNAPSHOT_WIDTH)?,
            instructions: zeroed(BFV_PROGRAM_MAX_INSTRUCTIONS * INSTRUCTION_WIDTH)?,
            steps: 0,
            outputs: 0,
        })
    }

    /// Number of instructions executed successfully.
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.steps
    }

    /// Number of output ciphertexts emitted in instruction order.
    #[must_use]
    pub fn output_count(&self) -> usize {
        self.outputs
    }

    /// Borrow a private initial/post-instruction snapshot, registers then memory.
    ///
    /// Each ciphertext contributes its 64 `c0` coefficients followed by 64 `c1`
    /// coefficients. Row zero is the initial state; row `i+1` follows instruction `i`.
    #[must_use]
    pub fn snapshot(&self, row: usize) -> Option<&[u64]> {
        if row > self.steps {
            return None;
        }
        let start = row.checked_mul(SNAPSHOT_WIDTH)?;
        self.snapshots
            .get(start..start.checked_add(SNAPSHOT_WIDTH)?)
    }

    pub(super) fn record(
        &mut self,
        instruction: Option<HiddenRamFheInstruction>,
        registers: &[BfvCiphertext],
        memory: &[BfvCiphertext],
        outputs: usize,
    ) -> Result<(), RamLfeError> {
        if registers.len() != BFV_PROGRAM_REGISTER_COUNT || memory.len() != BFV_PROGRAM_STATE_WIDTH
        {
            return Err(invalid_program_error("trace machine shape mismatch"));
        }
        if let Some(instruction) = instruction {
            if self.steps == BFV_PROGRAM_MAX_INSTRUCTIONS {
                return Err(invalid_program_error("trace instruction capacity exceeded"));
            }
            let start = self.steps * INSTRUCTION_WIDTH;
            let fields = Zeroizing::new(instruction_fields(instruction));
            self.instructions[start..start + INSTRUCTION_WIDTH].copy_from_slice(&*fields);
            self.steps += 1;
        }
        let mut target = self.snapshots
            [self.steps * SNAPSHOT_WIDTH..(self.steps + 1) * SNAPSHOT_WIDTH]
            .chunks_exact_mut(COEFFICIENTS_PER_CIPHERTEXT);
        for value in registers.iter().chain(memory) {
            if value.c0.len() != 64 || value.c1.len() != 64 {
                return Err(invalid_program_error("trace ciphertext shape mismatch"));
            }
            let row = target.next().expect("validated exact machine shape");
            row[..64].copy_from_slice(&value.c0);
            row[64..].copy_from_slice(&value.c1);
        }
        self.outputs = outputs;
        Ok(())
    }
}

impl fmt::Debug for RamLfeProgramExecutionTrace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED RAM-LFE execution trace]")
    }
}

impl Drop for RamLfeProgramExecutionTrace {
    fn drop(&mut self) {
        self.snapshots.as_mut_slice().zeroize();
        self.instructions.as_mut_slice().zeroize();
        self.steps.zeroize();
        self.outputs.zeroize();
        #[cfg(test)]
        observe_clear(
            self.snapshots
                .iter()
                .chain(self.instructions.iter())
                .copied(),
        );
    }
}

fn zeroed(count: usize) -> Result<Zeroizing<Vec<u64>>, RamLfeError> {
    let mut values = Zeroizing::new(Vec::new());
    values
        .try_reserve_exact(count)
        .map_err(|error| allocation_error(&error))?;
    values.resize(count, 0);
    Ok(values)
}

fn allocation_error(error: &std::collections::TryReserveError) -> RamLfeError {
    invalid_program_error(&format!("private interpreter allocation failed: {error}"))
}

#[cfg(test)]
thread_local! {
    static CLEARED: std::cell::RefCell<Option<usize>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn observe_clear(cells: impl Iterator<Item = u64>) {
    CLEARED.with_borrow_mut(|observed| {
        if let Some(count) = observed {
            for value in cells {
                assert_eq!(value, 0);
                *count += 1;
            }
        }
    });
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
