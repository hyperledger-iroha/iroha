//! Borrowed exact JSON envelopes restored directly into original prepaid carrier maps.
//!
//! No decoded JSON row/string graph is materialized. Only bounded binary scratch and the
//! final immutable payloads are allocated, all through the supplied original State pool.

use super::*;
use crate::state::generic_carrier_storage::{
    self as custody, ArtifactStorage, CarrierStorage, RetainedCarrierBytes, RetainedCarrierKey,
};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_data_model::runtime::carrier::{
    MAX_CARRIER_FRAME_BYTES, MAX_CARRIER_KEY_BYTES, borrow_carrier_storage_key_v1,
    borrow_carrier_storage_value_v1,
};
use iroha_data_model::smart_contract::ContractArtifactId;

fn invalid(message: &'static str) -> StateRestoreError {
    json::Error::InvalidField {
        field: "runtime_generic_storage".into(),
        message: message.into(),
    }
    .into()
}
fn payload_error(error: custody::Error) -> StateRestoreError {
    match error {
        custody::Error::Invalid(message) => invalid(message),
        custody::Error::Admission(original) => {
            StateRestoreError::ExecutionDeferred(original.into())
        }
        custody::Error::Allocator(original) => {
            StateRestoreError::ExecutionDeferred(original.into())
        }
    }
}
fn codec_error(original: norito::core::DecodeAttemptError) -> StateRestoreError {
    match crate::execution_attempt::canonical_decode_attempt_error(original, |_| {
        invalid("noncanonical carrier storage bytes")
    }) {
        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
        crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
            StateRestoreError::ExecutionDeferred(original)
        }
    }
}
fn binary(
    hex: &str,
    maximum: usize,
    budget: &AllocationBudget,
) -> Result<ChargedBuffer<u8>, StateRestoreError> {
    if hex.len() % 2 != 0
        || hex.len() / 2 > maximum
        || hex
            .bytes()
            .any(|b| !(b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    {
        return Err(invalid("noncanonical or oversized binary snapshot field"));
    }
    let mut bytes =
        ChargedBuffer::new(hex.len() / 2, budget).map_err(|original| match original {
            ChargedBufferError::Admission(original) => {
                StateRestoreError::ExecutionDeferred(original.into())
            }
            original @ ChargedBufferError::Allocator { .. } => {
                StateRestoreError::ExecutionDeferred(
                    iroha_allocation::AllocatorRefusal::try_from(original)
                        .expect("matched allocator refusal")
                        .into(),
                )
            }
        })?;
    // Capacity was funded before any bytes are initialized or decoded.
    for _ in 0..hex.len() / 2 {
        bytes.push_reserved(0);
    }
    hex::decode_to_slice(hex, bytes.as_mut_slice())
        .map_err(|_| invalid("invalid binary snapshot field"))?;
    Ok(bytes)
}
fn carrier_key(
    hex: &str,
    budget: &AllocationBudget,
) -> Result<RetainedCarrierKey, StateRestoreError> {
    let bytes = binary(hex, MAX_CARRIER_KEY_BYTES + 128, budget)?;
    let (storage, key) = borrow_carrier_storage_key_v1(bytes.as_slice()).map_err(codec_error)?;
    RetainedCarrierKey::admit_parts(storage, key, budget).map_err(payload_error)
}
fn record(
    hex: &str,
    maximum: usize,
    budget: &AllocationBudget,
) -> Result<RetainedCarrierBytes, StateRestoreError> {
    let bytes = binary(hex, maximum + 8, budget)?;
    let frame = borrow_carrier_storage_value_v1(bytes.as_slice()).map_err(codec_error)?;
    if frame.len() > maximum {
        return Err(invalid("record exceeds its owner bound"));
    }
    RetainedCarrierBytes::admit(frame, budget).map_err(payload_error)
}
fn artifact_key(
    hex: &str,
    budget: &AllocationBudget,
) -> Result<ContractArtifactId, StateRestoreError> {
    let bytes = binary(hex, 128, budget)?;
    norito::core::classify_decode_attempt(|| {
        let key = ContractArtifactId::decode_all(&mut bytes.as_slice())?;
        norito::codec::verify_exact_payload(&key, bytes.as_slice())?;
        Ok(key)
    })
    .map_err(codec_error)
}

/// Cursor for the single exact envelope emitted by `snapshot_storage::serialize_maps`.
struct Cursor<'a>(&'a str);
impl<'a> Cursor<'a> {
    fn literal(&mut self, literal: &str) -> Result<(), StateRestoreError> {
        self.0 = self
            .0
            .strip_prefix(literal)
            .ok_or_else(|| invalid("noncanonical carrier snapshot envelope"))?;
        Ok(())
    }
    fn blob(&mut self) -> Result<&'a str, StateRestoreError> {
        self.literal("{\"encoded_hex\":\"")?;
        let end = self
            .0
            .find('"')
            .ok_or_else(|| invalid("unterminated binary snapshot field"))?;
        let hex = &self.0[..end];
        self.0 = &self.0[end..];
        self.literal("\"}")?;
        Ok(hex)
    }
    fn rows(
        &mut self,
        undo: bool,
        mut visit: impl FnMut(&str, Option<&str>) -> Result<(), StateRestoreError>,
    ) -> Result<(), StateRestoreError> {
        if self.0.starts_with(']') {
            return self.literal("]");
        }
        loop {
            self.literal("{\"key\":")?;
            let key = self.blob()?;
            self.literal(",\"value\":")?;
            let value = if undo && self.0.starts_with("null") {
                self.literal("null")?;
                None
            } else {
                Some(self.blob()?)
            };
            self.literal("}")?;
            visit(key, value)?;
            if self.0.starts_with(']') {
                return self.literal("]");
            }
            self.literal(",")?;
        }
    }
}

pub(crate) fn restore_carriers(
    raw: &str,
    budget: &AllocationBudget,
) -> Result<CarrierStorage, StateRestoreError> {
    let mut cursor = Cursor(raw);
    cursor.literal("{\"revert\":[")?;
    CarrierStorage::try_restore_admitted(budget.clone(), |current, undo| {
        let mut previous = None;
        cursor.rows(true, |key, value| {
            let key = carrier_key(key, budget)?;
            if previous.as_ref().is_some_and(|p| p >= &key) {
                return Err(invalid("undo carrier keys are not strictly ordered"));
            }
            previous = Some(key.clone());
            let value = value
                .map(|hex| record(hex, MAX_CARRIER_FRAME_BYTES, budget))
                .transpose()?;
            undo(key, value)?;
            Ok(())
        })?;
        cursor.literal(",\"blocks\":[")?;
        previous = None;
        cursor.rows(false, |key, value| {
            let key = carrier_key(key, budget)?;
            if previous.as_ref().is_some_and(|p| p >= &key) {
                return Err(invalid("current carrier keys are not strictly ordered"));
            }
            previous = Some(key.clone());
            let value = record(
                value.expect("current values cannot be null"),
                MAX_CARRIER_FRAME_BYTES,
                budget,
            )?;
            current(key, value)?;
            Ok(())
        })?;
        cursor.literal("}")?;
        if !cursor.0.is_empty() {
            return Err(invalid("trailing carrier snapshot bytes"));
        }
        Ok::<_, StateRestoreError>(())
    })
    .map_err(|error| match error {
        mv::storage::AdmittedBlockError::Admission(original) => original.into(),
        mv::storage::AdmittedBlockError::Callback(original) => original,
    })
}

pub(crate) fn restore_artifacts(
    raw: &str,
    budget: &AllocationBudget,
) -> Result<ArtifactStorage, StateRestoreError> {
    const MAX: usize = iroha_data_model::isi::runtime_carrier::MAX_CARRIER_ARTIFACT_BYTES_V1;
    let mut cursor = Cursor(raw);
    cursor.literal("{\"revert\":[")?;
    ArtifactStorage::try_restore_admitted(budget.clone(), |current, undo| {
        let mut previous = None;
        cursor.rows(true, |key, value| {
            let key = artifact_key(key, budget)?;
            if previous.is_some_and(|p| p >= key) {
                return Err(invalid("undo artifact keys are not strictly ordered"));
            }
            previous = Some(key);
            undo(key, value.map(|hex| record(hex, MAX, budget)).transpose()?)?;
            Ok(())
        })?;
        cursor.literal(",\"blocks\":[")?;
        previous = None;
        cursor.rows(false, |key, value| {
            let key = artifact_key(key, budget)?;
            if previous.is_some_and(|p| p >= key) {
                return Err(invalid("current artifact keys are not strictly ordered"));
            }
            previous = Some(key);
            current(
                key,
                record(value.expect("current values cannot be null"), MAX, budget)?,
            )?;
            Ok(())
        })?;
        cursor.literal("}")?;
        if !cursor.0.is_empty() {
            return Err(invalid("trailing artifact snapshot bytes"));
        }
        Ok::<_, StateRestoreError>(())
    })
    .map_err(|error| match error {
        mv::storage::AdmittedBlockError::Admission(original) => original.into(),
        mv::storage::AdmittedBlockError::Callback(original) => original,
    })
}
