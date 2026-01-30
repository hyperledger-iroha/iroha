//! Gas schedule exposure helpers for host/runtime enforcement.

use iroha_crypto::Hash;

use crate::gas;

/// Single opcode entry in the canonical gas schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GasScheduleEntry {
    /// Primary opcode byte.
    pub opcode: u8,
    /// Gas cost charged for the opcode in its canonical form.
    pub cost: u64,
}

/// Return the canonical gas schedule entries in opcode order.
#[must_use]
pub fn gas_schedule_entries() -> Vec<GasScheduleEntry> {
    gas::SCHEDULE_OPCODES
        .iter()
        .map(|&opcode| GasScheduleEntry {
            opcode,
            cost: gas::cost_of((opcode as u32) << 24),
        })
        .collect()
}

/// Deterministic digest of the canonical gas schedule.
#[must_use]
pub fn schedule_hash() -> Hash {
    gas::schedule_hash()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_schedule_entries_match_opcodes() {
        let entries = gas_schedule_entries();
        assert_eq!(entries.len(), gas::SCHEDULE_OPCODES.len());
        for (entry, &opcode) in entries.iter().zip(gas::SCHEDULE_OPCODES) {
            assert_eq!(entry.opcode, opcode);
            let expected = gas::cost_of((opcode as u32) << 24);
            assert_eq!(entry.cost, expected);
        }
    }

    #[test]
    fn schedule_hash_matches_gas_hash() {
        assert_eq!(schedule_hash(), gas::schedule_hash());
    }
}
