#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use fastpq_prover::{OperationKind, StateTransition, TransitionBatch, build_trace};
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

const MAX_TRANSITIONS: usize = 32;
const MAX_KEY_BYTES: usize = 64;
const MAX_VALUE_BYTES: usize = 16;
const MAX_METADATA_ENTRIES: usize = 8;
const MAX_METADATA_KEY_LEN: usize = 32;
const MAX_METADATA_VALUE_LEN: usize = 64;

#[derive(Debug)]
struct BatchSeed {
    parameter: String,
    transitions: Vec<TransitionSeed>,
    metadata: Vec<MetadataSeed>,
    dsid: Option<Vec<u8>>,
    slot: Option<Vec<u8>>,
}

#[derive(Debug)]
struct TransitionSeed {
    key: Vec<u8>,
    pre_value: Vec<u8>,
    post_value: Vec<u8>,
    operation: OperationSeed,
}

#[derive(Debug)]
enum OperationSeed {
    Transfer,
    Mint,
    Burn,
    RoleGrant {
        role_id: Vec<u8>,
        permission_id: Vec<u8>,
        epoch: u64,
    },
    RoleRevoke {
        role_id: Vec<u8>,
        permission_id: Vec<u8>,
        epoch: u64,
    },
    MetaSet,
}

#[derive(Debug)]
struct MetadataSeed {
    key: String,
    value: Vec<u8>,
}

impl<'a> Arbitrary<'a> for BatchSeed {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let parameter = ascii_string(u, MAX_METADATA_KEY_LEN)?;

        let transition_len =
            usize::try_from(u.int_in_range(0..=MAX_TRANSITIONS as u32)?).expect("len fits usize");
        let mut transitions = Vec::with_capacity(transition_len);
        for _ in 0..transition_len {
            transitions.push(TransitionSeed::arbitrary(u)?);
        }

        let metadata_len =
            usize::try_from(u.int_in_range(0..=MAX_METADATA_ENTRIES as u32)?).expect("len fits");
        let mut metadata = Vec::with_capacity(metadata_len);
        for _ in 0..metadata_len {
            metadata.push(MetadataSeed::arbitrary(u)?);
        }

        let dsid = if u.arbitrary()? {
            Some(bounded_vec(u, MAX_METADATA_VALUE_LEN)?)
        } else {
            None
        };
        let slot = if u.arbitrary()? {
            Some(bounded_vec(u, 8)?)
        } else {
            None
        };

        Ok(Self {
            parameter,
            transitions,
            metadata,
            dsid,
            slot,
        })
    }
}

impl<'a> Arbitrary<'a> for TransitionSeed {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            key: bounded_vec(u, MAX_KEY_BYTES)?,
            pre_value: bounded_vec(u, MAX_VALUE_BYTES)?,
            post_value: bounded_vec(u, MAX_VALUE_BYTES)?,
            operation: OperationSeed::arbitrary(u)?,
        })
    }
}

impl<'a> Arbitrary<'a> for OperationSeed {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=5)? {
            0 => Ok(Self::Transfer),
            1 => Ok(Self::Mint),
            2 => Ok(Self::Burn),
            3 => Ok(Self::RoleGrant {
                role_id: bounded_vec(u, MAX_KEY_BYTES)?,
                permission_id: bounded_vec(u, MAX_KEY_BYTES)?,
                epoch: u.arbitrary()?,
            }),
            4 => Ok(Self::RoleRevoke {
                role_id: bounded_vec(u, MAX_KEY_BYTES)?,
                permission_id: bounded_vec(u, MAX_KEY_BYTES)?,
                epoch: u.arbitrary()?,
            }),
            _ => Ok(Self::MetaSet),
        }
    }
}

impl<'a> Arbitrary<'a> for MetadataSeed {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            key: ascii_string(u, MAX_METADATA_KEY_LEN)?,
            value: bounded_vec(u, MAX_METADATA_VALUE_LEN)?,
        })
    }
}

impl BatchSeed {
    fn into_batch(self) -> TransitionBatch {
        let mut batch = TransitionBatch::new(self.parameter, fastpq_prover::PublicInputs::default());
        let mut metadata = BTreeMap::new();
        for entry in self.metadata {
            metadata.insert(entry.key, entry.value);
        }
        if let Some(dsid) = self.dsid {
            let mut bytes = [0u8; 16];
            let len = dsid.len().min(bytes.len());
            bytes[..len].copy_from_slice(&dsid[..len]);
            batch.public_inputs.dsid = bytes;
        }
        if let Some(slot) = self.slot {
            let mut buf = [0u8; 8];
            let len = slot.len().min(buf.len());
            buf[..len].copy_from_slice(&slot[..len]);
            batch.public_inputs.slot = u64::from_le_bytes(buf);
        }
        batch.metadata = metadata;

        for transition in self.transitions {
            batch.push(transition.into_transition());
        }
        batch
    }
}

impl TransitionSeed {
    fn into_transition(self) -> StateTransition {
        StateTransition::new(
            self.key,
            self.pre_value,
            self.post_value,
            self.operation.into_kind(),
        )
    }
}

impl OperationSeed {
    fn into_kind(self) -> OperationKind {
        match self {
            OperationSeed::Transfer => OperationKind::Transfer,
            OperationSeed::Mint => OperationKind::Mint,
            OperationSeed::Burn => OperationKind::Burn,
            OperationSeed::RoleGrant {
                role_id,
                permission_id,
                epoch,
            } => OperationKind::RoleGrant {
                role_id,
                permission_id,
                epoch,
            },
            OperationSeed::RoleRevoke {
                role_id,
                permission_id,
                epoch,
            } => OperationKind::RoleRevoke {
                role_id,
                permission_id,
                epoch,
            },
            OperationSeed::MetaSet => OperationKind::MetaSet,
        }
    }
}

fn ascii_string(u: &mut Unstructured<'_>, max_len: usize) -> arbitrary::Result<String> {
    let len = usize::try_from(u.int_in_range(0..=max_len as u32)?).expect("len fits usize");
    let bytes = u.bytes(len)?;
    let string = bytes
        .iter()
        .map(|byte| (b'a' + (byte % 26)) as char)
        .collect();
    Ok(string)
}

fn bounded_vec(u: &mut Unstructured<'_>, max_len: usize) -> arbitrary::Result<Vec<u8>> {
    let len = usize::try_from(u.int_in_range(0..=max_len as u32)?).expect("len fits usize");
    Ok(u.bytes(len)?.to_vec())
}

fn process_seed(seed: BatchSeed) {
    let batch = seed.into_batch();
    if let Ok(trace) = build_trace(&batch) {
        assert!(trace.padded_len.is_power_of_two());
        for column in &trace.columns {
            assert_eq!(
                column.values.len(),
                trace.padded_len,
                "column length should match padded_len"
            );
        }
    }
}

fuzz_target!(|seed: BatchSeed| {
    process_seed(seed);
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_smoke_runs_without_fuzzer() {
        // Deterministic seeds keep this harness stable on non-fuzzer, stable toolchains.
        let fixtures: &[&[u8]] = &[
            b"",
            b"trace-seed-1",
            b"trace-seed-2-with-more-bytes",
            &[0u8; 64],
        ];
        for raw in fixtures {
            let mut u = Unstructured::new(raw);
            if let Ok(seed) = BatchSeed::arbitrary(&mut u) {
                process_seed(seed);
            }
        }
    }
}
