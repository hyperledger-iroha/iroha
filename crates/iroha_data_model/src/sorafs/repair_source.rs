//! Bounded account-authenticated chunk requests under an exact native repair lease.

use super::moderation_ledger::RepairFinalizedCursorV1;

/// Maximum canonical repair-source request size.
pub const REPAIR_SOURCE_REQUEST_MAX_BYTES_V1: usize = 4096;
/// Maximum chunk returned by a repair source.
pub const REPAIR_SOURCE_CHUNK_MAX_BYTES_V1: usize = 4 * 1024 * 1024;

/// Claims authenticated by the canonical network HTTP signature and checked against native State.
/// A decoded request never establishes a lease, finality, provider admission or read permission.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sorafs::repair_source::RepairSourceRequestV1")]
pub struct RepairSourceRequestV1 {
    /// Independently retained finalized task cut, which must remain an ancestor of current State.
    pub floor: RepairFinalizedCursorV1,
    /// Exact native task identity.
    pub task_id: [u8; 32],
    /// Bounded canonical ticket identifying the native task.
    pub ticket_id: String,
    /// Exact task revision admitted by the worker.
    pub task_revision: u64,
    /// Exclusive lease generation admitted by the worker.
    pub lease_generation: u64,
    /// Provider whose replica the task repairs.
    pub target_provider: [u8; 32],
    /// Provider asked to supply a verified chunk.
    pub source_provider: [u8; 32],
    /// Exact approved manifest identity.
    pub manifest_digest: [u8; 32],
    /// Exact BLAKE3 chunk commitment within that manifest.
    pub chunk_digest: [u8; 32],
    /// Exact chunk length, also checked against stored canonical metadata.
    pub chunk_length: u32,
}

impl RepairSourceRequestV1 {
    /// Check bounded canonical claims before consulting authority or allocating payload bytes.
    /// This does not authenticate any claim.
    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        self.ticket_id.len() <= 128
            && sorafs_manifest::repair::RepairTicketId(self.ticket_id.clone())
                .validate()
                .is_ok()
            && self.chunk_length > 0
            && self.chunk_length as usize <= REPAIR_SOURCE_CHUNK_MAX_BYTES_V1
            && self.target_provider != self.source_provider
            && self.floor.height > 0
            && self.task_revision > 0
            && self.lease_generation > 0
            && [
                self.floor.block_hash,
                self.task_id,
                self.target_provider,
                self.source_provider,
                self.manifest_digest,
                self.chunk_digest,
            ]
            .iter()
            .all(|digest| *digest != [0; 32])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_source_request_roundtrip_is_canonical() {
        let request = RepairSourceRequestV1 {
            floor: RepairFinalizedCursorV1 {
                height: 3,
                block_hash: [1; 32],
            },
            task_id: [2; 32],
            ticket_id: "REP-TEST".to_owned(),
            task_revision: 4,
            lease_generation: 1,
            target_provider: [3; 32],
            source_provider: [4; 32],
            manifest_digest: [5; 32],
            chunk_digest: [6; 32],
            chunk_length: 1024,
        };
        let bytes = norito::encode_canonical(&request).unwrap();
        assert!(request.has_valid_shape());
        assert!(bytes.len() < REPAIR_SOURCE_REQUEST_MAX_BYTES_V1);
        assert_eq!(
            norito::decode_from_bytes::<RepairSourceRequestV1>(&bytes).unwrap(),
            request
        );
        for mutation in 0..5 {
            let mut changed = request.clone();
            match mutation {
                0 => {
                    changed.chunk_length =
                        u32::try_from(REPAIR_SOURCE_CHUNK_MAX_BYTES_V1 + 1).unwrap()
                }
                1 => changed.task_revision = 0,
                2 => changed.lease_generation = 0,
                3 => changed.source_provider = changed.target_provider,
                _ => changed.ticket_id = "a".repeat(129),
            }
            assert!(!changed.has_valid_shape());
        }
    }
}
