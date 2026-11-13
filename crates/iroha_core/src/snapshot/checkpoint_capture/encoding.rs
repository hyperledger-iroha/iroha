//! Checked canonical encoding of the finalized checkpoint overlay.

use norito::{
    codec::Encode,
    json::{self, JsonSerialize, JsonWriteSink},
};

use crate::snapshot::SnapshotCaptureError;
use crate::state::{StateBlock, WorldReadOnly, snapshot_storage};

/// A fallible destination whose backing bytes never exceed the configured snapshot cap.
pub(crate) struct Writer {
    output: String,
    maximum: usize,
}

impl Writer {
    fn new(maximum: usize) -> Self {
        Self {
            output: String::new(),
            maximum,
        }
    }

    fn admit(&mut self, bytes: usize) -> Result<(), json::BoundedJsonError> {
        if bytes > self.maximum.saturating_sub(self.output.len()) {
            return Err(json::BoundedJsonError::BodyTooLarge);
        }
        self.output
            .try_reserve_exact(bytes)
            .map_err(|_| json::BoundedJsonError::AllocationFailed)
    }

    /// Write a typed value through its checked Norito serializer.
    pub(crate) fn value(
        &mut self,
        value: &impl JsonSerialize,
    ) -> Result<(), json::BoundedJsonError> {
        value.json_serialize_to(self)
    }

    /// Write the canonical binary blob wrapper without any unbounded hex buffer.
    pub(crate) fn binary(&mut self, value: &impl Encode) -> Result<(), json::BoundedJsonError> {
        let encoded_len = norito::codec::encode_adaptive_into(value, &mut std::io::sink())
            .map_err(|_| json::BoundedJsonError::LengthMismatch)?;
        let json_len = encoded_len
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(18))
            .ok_or(json::BoundedJsonError::BodyTooLarge)?;
        self.admit(json_len)?;
        self.push_str("{\"encoded_hex\":\"")?;
        // Enforce the cap on every byte of the second pass too. Even a drifting
        // serializer cannot grow a temporary Vec or overrun the admitted output.
        let mut hex = HexWriter {
            out: self,
            error: None,
        };
        let actual = norito::codec::encode_adaptive_into(value, &mut hex).map_err(|_| {
            hex.error
                .take()
                .unwrap_or(json::BoundedJsonError::LengthMismatch)
        })?;
        if actual != encoded_len {
            return Err(json::BoundedJsonError::LengthMismatch);
        }
        self.push_str("\"}")
    }
}

struct HexWriter<'a> {
    out: &'a mut Writer,
    error: Option<json::BoundedJsonError>,
}
impl std::io::Write for HexWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &byte in bytes {
            for nibble in [byte >> 4, byte & 15] {
                if let Err(error) = self.out.push(char::from(HEX[usize::from(nibble)])) {
                    self.error = Some(error);
                    return Err(std::io::Error::other(
                        "bounded checkpoint hex sink refused output",
                    ));
                }
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl JsonWriteSink for Writer {
    fn push(&mut self, value: char) -> Result<(), json::BoundedJsonError> {
        self.admit(value.len_utf8())?;
        self.output.push(value);
        Ok(())
    }
    fn push_str(&mut self, value: &str) -> Result<(), json::BoundedJsonError> {
        self.admit(value.len())?;
        self.output.push_str(value);
        Ok(())
    }
}

/// Encode the original frozen block, including exact undo, without consulting a newer State.
pub(crate) fn finalized(
    state: &StateBlock<'_>,
    maximum: usize,
) -> Result<String, SnapshotCaptureError> {
    let mut out = Writer::new(maximum);
    encode(state, &mut out).map_err(SnapshotCaptureError::BoundedEncoding)?;
    Ok(out.output)
}

fn encode(state: &StateBlock<'_>, out: &mut Writer) -> Result<(), json::BoundedJsonError> {
    let world = state.world();
    out.push_str("{\"chain_id\":")?;
    out.value(&state.chain_id)?;
    out.push_str(",\"network_id\":")?;
    out.value(&state.network_id)?;
    out.push_str(",\"world\":")?;
    let omitted = crate::runtime::uninstalled_world_members(world.runtime_selection())
        .map_err(|_| json::BoundedJsonError::Unsupported)?;
    // The full native World is itself bounded before parsing its fixed member inventory.
    let full = json::to_json_bounded(world, out.maximum.saturating_sub(out.output.len()))?;
    let members = canonical_world_members(&full)?;
    out.push('{')?;
    let mut first = true;
    for (key, raw) in members {
        if omitted.members.contains(&key) {
            continue;
        }
        if !first {
            out.push(',')?;
        }
        first = false;
        out.value(&key)?;
        out.push(':')?;
        if key == "external_event_buf" {
            state.json_serialize_committed_external_event_buffer_to(out)?;
            continue;
        }
        let raw = raw.ok_or(json::BoundedJsonError::Unsupported)?;
        if omitted.nested_in(key).is_empty() {
            out.push_str(raw)?;
        } else {
            let nested = crate::state::deserialize::json_object_members(raw)
                .map_err(|_| json::BoundedJsonError::Unsupported)?;
            out.push('{')?;
            let mut first = true;
            for (nested_key, nested_raw) in nested {
                if omitted.nested_in(key).contains(&nested_key) {
                    continue;
                }
                if !first {
                    out.push(',')?;
                }
                first = false;
                out.value(&nested_key)?;
                out.push(':')?;
                out.push_str(nested_raw)?;
            }
            out.push('}')?;
        }
    }
    out.push_str("},\"nexus_runtime\":")?;
    out.value(&state.canonical_runtime)?;
    out.push_str(",\"native_execution_tip\":")?;
    out.value(
        &crate::state::native_execution_tip::NativeExecutionTipSnapshot::from_original(
            *state.native_execution_tip.get(),
            *state.native_execution_tip.original_undo(),
        ),
    )?;
    out.push_str(",\"block_hashes\":[")?;
    for (index, hash) in state.block_hashes().iter().enumerate() {
        if index != 0 {
            out.push(',')?;
        }
        out.value(hash)?;
    }
    out.push_str("],\"transactions\":")?;
    out.value(&state.transactions)?;
    macro_rules! storage {
        ($($field:ident),*) => {$(
            out.push_str(concat!(",\"", stringify!($field), "\":"))?;
            snapshot_storage::serialize_block_bounded(&world.$field, out)?;
        )*};
    }
    storage!(
        public_lane_validators,
        public_lane_stake_shares,
        public_lane_stake_custody,
        public_lane_stake_reserves,
        space_directory_manifests
    );
    crate::state::snapshot_service_state::serialize_block_bounded(world, out)?;
    crate::state::sccp_snapshot_state::serialize_block_bounded(world, out)?;
    out.push_str(",\"commit_topology\":")?;
    out.value(&state.commit_topology)?;
    out.push_str(",\"prev_commit_topology\":")?;
    out.value(&state.prev_commit_topology)?;
    out.push('}')
}

/// Overlay declaration order is independent of the one committed World snapshot schema.
/// Reorder only borrowed values from that exact fixed inventory, before storage omission.
/// The transient block event Vec is deliberately skipped; its `None` entry selects the
/// committed Cell owner, which publication leaves unchanged, rather than synthetic bytes.
fn canonical_world_members(
    full: &str,
) -> Result<Vec<(&str, Option<&str>)>, json::BoundedJsonError> {
    let order = <crate::state::World as json::FastJsonWrite>::json_object_field_order()
        .ok_or(json::BoundedJsonError::Unsupported)?;
    let mut members: Vec<_> = crate::state::deserialize::json_object_members(full)
        .map_err(|_| json::BoundedJsonError::Unsupported)?
        .into_iter()
        .map(|(key, raw)| (key, Some(raw)))
        .collect();
    members.push(("external_event_buf", None));
    members.sort_unstable_by_key(|(key, _)| {
        order
            .iter()
            .position(|expected| expected == key)
            .unwrap_or(usize::MAX)
    });
    if !members
        .iter()
        .map(|(key, _)| *key)
        .eq(order.iter().copied())
    {
        return Err(json::BoundedJsonError::Unsupported);
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_world_members_use_the_exact_committed_schema_order() {
        let order =
            <crate::state::World as json::FastJsonWrite>::json_object_field_order().unwrap();
        let reversed = format!(
            "{{{}}}",
            order
                .iter()
                .rev()
                .filter(|key| **key != "external_event_buf")
                .map(|key| format!("\"{key}\":null"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let members = canonical_world_members(&reversed).unwrap();
        assert!(
            members
                .iter()
                .map(|(key, _)| *key)
                .eq(order.iter().copied())
        );
        assert_eq!(
            members.iter().find(|(key, _)| *key == "external_event_buf"),
            Some(&("external_event_buf", None))
        );
        assert!(canonical_world_members("{}").is_err());
        assert!(canonical_world_members("{\"unknown\":null}").is_err());
    }

    #[test]
    fn native_storage_keys_preserve_checked_bytes_and_refuse_the_exact_cap() {
        use crate::state::{
            MusubiResolverIndexRevisionV1, SmartContractCodeUploadChunkKey,
            SmartContractCodeUploadKey,
        };
        use iroha_data_model::{account::AccountId, smart_contract::ContractArtifactId};
        fn check<T: json::JsonKeyCodec>(key: &T) {
            let mut expected = String::new();
            key.encode_json_key(&mut expected);
            let mut out = Writer::new(expected.len());
            key.encode_json_key_to(&mut out).unwrap();
            assert_eq!(out.output, expected);
            let mut short = Writer::new(expected.len() - 1);
            assert_eq!(
                key.encode_json_key_to(&mut short),
                Err(json::BoundedJsonError::BodyTooLarge)
            );
            assert!(short.output.len() < expected.len());
        }
        let owner = AccountId::new(iroha_crypto::KeyPair::random().public_key().clone());
        let upload = SmartContractCodeUploadKey::new(
            owner,
            ContractArtifactId::new(
                iroha_model_base::topology::DataSpaceId::new(u64::MAX),
                iroha_crypto::Hash::new(b"populated upload"),
            ),
        );
        check(&upload);
        check(&SmartContractCodeUploadChunkKey::new(upload, u32::MAX));
        check(&MusubiResolverIndexRevisionV1::new(u64::MAX).unwrap());
        #[cfg(feature = "json")]
        check(&crate::oracle::ObservationWindowKey::new(
            "feed".parse().unwrap(),
            iroha_data_model::oracle::FeedConfigVersion(u32::MAX),
            u64::MAX,
            iroha_crypto::Hash::new(b"populated observation"),
        ));
    }

    #[test]
    fn populated_governance_labels_have_identical_bounded_json() {
        use crate::state::{
            GovernanceProposalStatus as Proposal, GovernanceReferendumMode as Mode,
            GovernanceReferendumStatus as Status,
        };
        fn check<T: json::JsonSerialize>(value: &T) {
            let expected = json::to_json(value).unwrap();
            assert_eq!(
                json::to_json_bounded(value, expected.len()).unwrap(),
                expected
            );
            assert_eq!(
                json::to_json_bounded(value, expected.len() - 1),
                Err(json::BoundedJsonError::BodyTooLarge)
            );
        }
        for value in [
            Proposal::Proposed,
            Proposal::Rejected,
            Proposal::Enacted,
            Proposal::Superseded,
            Proposal::ExecutionFailed,
        ] {
            check(&value);
        }
        for value in [Status::Proposed, Status::Open, Status::Closed] {
            check(&value);
        }
        for value in [Mode::Zk, Mode::Plain] {
            check(&value);
        }
    }

    #[test]
    fn configured_limit_applies_before_growing_output_or_hex_buffers() {
        let mut out = Writer::new(6);
        out.push_str("12345").unwrap();
        assert_eq!(
            out.push_str("67"),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
        assert_eq!(out.output, "12345");
        assert_eq!(
            out.binary(&vec![0_u8; 32]),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
        assert_eq!(out.output, "12345");
    }

    #[test]
    fn a_drifting_binary_serializer_cannot_overrun_the_checked_destination() {
        struct Drifting(std::cell::Cell<bool>);
        impl norito::core::SerializePayload for Drifting {
            fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
                if self.0.replace(true) {
                    out.write_all(&[0; 32])?;
                } else {
                    out.write_all(&[0])?;
                }
                Ok(())
            }
        }
        let mut out = Writer::new(20);
        assert_eq!(
            out.binary(&Drifting(std::cell::Cell::new(false))),
            Err(json::BoundedJsonError::BodyTooLarge)
        );
        assert!(out.output.len() <= 20);
    }

    #[test]
    fn bounded_binary_maps_preserve_current_and_undo_bytes_after_freezing() {
        use crate::state::block_field::BlockField;
        let storage: mv::storage::Storage<(u64, u64), Vec<u8>> =
            [((1, 2), vec![0, 255]), ((2, 3), vec![7])]
                .into_iter()
                .collect();
        let mut block = BlockField::new(storage.block());
        block.insert((1, 2), vec![4, 5]);
        block.remove((2, 3));
        block.insert((3, 4), vec![6, 7, 8]);
        block.remove((4, 5));
        let mut expected = String::new();
        snapshot_storage::serialize_block(&block, &mut expected);
        block.begin_freeze();
        block.try_finish_freeze(|_| Ok::<_, ()>(())).unwrap();
        let mut out = Writer::new(expected.len());
        snapshot_storage::serialize_block_bounded(&block, &mut out).unwrap();
        assert_eq!(out.output, expected);
        let mut too_small = Writer::new(expected.len() - 1);
        assert_eq!(
            snapshot_storage::serialize_block_bounded(&block, &mut too_small),
            Err(json::BoundedJsonError::BodyTooLarge),
        );
    }
}
