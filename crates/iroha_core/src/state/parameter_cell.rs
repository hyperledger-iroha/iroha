//! Direct restoration of both parameter cuts into fully funded original Cells.
//!
//! Parsing uses MV's sole current/explicit-undo grammar. The enclosing snapshot
//! owner retains the original source and performs the existing State validation.

use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer};
use mv::cell::{Cell, CellInitialization, CellInitializationError};
use norito::json::{self, JsonWriteSink};

use crate::pipeline_gas_policy::{
    ParameterIngressAttemptError, ParameterIngressError, RetainedStateParameters,
};

/// Original parameter Cell control or complete graph restore refusal.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ParameterCellRestoreError {
    /// Original seven physical Cell/control backings refused before parsing.
    #[error(transparent)]
    Initialization(#[from] CellInitializationError),
    /// Actual canonical/projection/comparison operation and original provenance.
    #[error(transparent)]
    Ingress(#[from] ParameterIngressAttemptError),
}

struct KeyBuffer(ChargedBuffer<u8>);
impl AsRef<[u8]> for KeyBuffer {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl AsMut<[u8]> for KeyBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}
fn key_buffer(length: usize, pool: &AllocationBudget) -> Result<KeyBuffer, ParameterIngressError> {
    let mut bytes = ChargedBuffer::new(length, pool)?;
    for _ in 0..length {
        bytes.push_reserved(0);
    }
    Ok(KeyBuffer(bytes))
}

/// Restore both exact cuts without allocating an ordinary graph or canonical copy.
///
/// Complete initial Cell controls and the two graph shells precede their values.
/// Canonical comparison runs before initializing any EBR generation. A failed
/// attempt drops private values and backing immediately and publishes nothing.
fn restore_inner<const CANONICAL: bool>(
    source: &str,
    pool: &AllocationBudget,
) -> Result<Cell<RetainedStateParameters, AllocationCharge>, ParameterCellRestoreError> {
    let controls = CellInitialization::<RetainedStateParameters>::try_reserve(pool)?;
    let (current, undo) = ParameterIngressAttemptError::observe(|| {
        let mut parser = json::Parser::new(source);
        parser.preflight_document()?;
        let (current, undo) = mv::json::parse_cell_values_with(
            &mut parser,
            |parser| RetainedStateParameters::try_decode_json(parser, pool),
            |length| key_buffer(length, pool),
        )?;
        parser.finish_document()?;
        if CANONICAL {
            let mut comparison = Comparison {
                remaining: source.as_bytes(),
            };
            let canonical = match mv::json::write_cell_values_to(&undo, &current, &mut comparison) {
                Ok(()) => comparison.remaining.is_empty(),
                Err(json::BoundedJsonError::LengthMismatch) => false,
                Err(error) => return Err(error.into_core_error().into()),
            };
            if !canonical {
                return Err(json::Error::WithPos {
                    msg: "parameters snapshot field is not canonically encoded",
                    byte: 0,
                    line: 1,
                    col: 1,
                }
                .into());
            }
        }
        Ok((current, undo))
    })?;
    Ok(controls.initialize(current, undo))
}

/// Require the exact original canonical snapshot bytes in production.
pub(super) fn restore(
    source: &str,
    pool: &AllocationBudget,
) -> Result<Cell<RetainedStateParameters, AllocationCharge>, ParameterCellRestoreError> {
    restore_inner::<true>(source, pool)
}
/// Test-owned DOM fixtures have intentionally discarded original field order.
#[cfg(test)]
pub(super) fn restore_authored_fixture(
    source: &str,
    pool: &AllocationBudget,
) -> Result<Cell<RetainedStateParameters, AllocationCharge>, ParameterCellRestoreError> {
    restore_inner::<false>(source, pool)
}

struct Comparison<'a> {
    remaining: &'a [u8],
}
impl JsonWriteSink for Comparison<'_> {
    fn push(&mut self, value: char) -> Result<(), json::BoundedJsonError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
    fn push_str(&mut self, value: &str) -> Result<(), json::BoundedJsonError> {
        let Some(rest) = self.remaining.strip_prefix(value.as_bytes()) else {
            return Err(json::BoundedJsonError::LengthMismatch);
        };
        self.remaining = rest;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
