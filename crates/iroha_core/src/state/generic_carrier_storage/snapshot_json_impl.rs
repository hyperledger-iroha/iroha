//! One exact binary current/undo envelope for retained physical carrier stores.

use super::*;
use norito::{
    codec::Encode,
    json::{self, JsonWriteSink},
};

fn container(
    out: &mut dyn JsonWriteSink,
    empty: bool,
    write: impl FnOnce(&mut dyn JsonWriteSink) -> Result<(), json::BoundedJsonError>,
) -> Result<(), json::BoundedJsonError> {
    if empty {
        out.begin_empty_container()?;
    } else {
        out.begin_container()?;
    }
    let result = write(out);
    out.end_container();
    result
}

fn binary(value: &impl Encode, out: &mut dyn JsonWriteSink) -> Result<(), json::BoundedJsonError> {
    struct Hex<'a> {
        out: &'a mut dyn JsonWriteSink,
        refused: Option<json::BoundedJsonError>,
    }
    impl std::io::Write for Hex<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for &byte in bytes {
                for nibble in [byte >> 4, byte & 15] {
                    if let Err(original) = self.out.push(char::from(HEX[usize::from(nibble)])) {
                        self.refused = Some(original);
                        return Err(std::io::Error::other(
                            "bounded carrier snapshot sink refused",
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
    container(out, false, |out| {
        out.push_str("{\"encoded_hex\":\"")?;
        let mut hex = Hex { out, refused: None };
        norito::codec::encode_adaptive_into(value, &mut hex).map_err(|original| {
            hex.refused.take().unwrap_or_else(|| {
                if original.decode_resource_error().is_some() {
                    json::BoundedJsonError::from_decode_resource(original)
                } else {
                    json::BoundedJsonError::LengthMismatch
                }
            })
        })?;
        out.push_str("\"}")
    })
}

fn bounded<'a, K: mv::Key + Encode, V: mv::Value + Encode>(
    undo: impl Iterator<Item = (&'a K, &'a Option<V>)>,
    current: impl Iterator<Item = (&'a K, &'a V)>,
    out: &mut dyn JsonWriteSink,
) -> Result<(), json::BoundedJsonError> {
    let mut undo = undo.peekable();
    let mut current = current.peekable();
    container(out, false, |out| {
        out.push_str("{\"revert\":")?;
        container(out, undo.peek().is_none(), |out| {
            out.push('[')?;
            for (index, (key, value)) in undo.enumerate() {
                if index != 0 {
                    out.push(',')?;
                }
                container(out, false, |out| {
                    out.push_str("{\"key\":")?;
                    binary(key, out)?;
                    out.push_str(",\"value\":")?;
                    match value {
                        Some(value) => binary(value, out)?,
                        None => out.push_str("null")?,
                    }
                    out.push('}')
                })?;
            }
            out.push(']')
        })?;
        out.push_str(",\"blocks\":")?;
        container(out, current.peek().is_none(), |out| {
            out.push('[')?;
            for (index, (key, value)) in current.enumerate() {
                if index != 0 {
                    out.push(',')?;
                }
                container(out, false, |out| {
                    out.push_str("{\"key\":")?;
                    binary(key, out)?;
                    out.push_str(",\"value\":")?;
                    binary(value, out)?;
                    out.push('}')
                })?;
            }
            out.push(']')
        })?;
        out.push('}')
    })
}

macro_rules! adapters {
    ($store:ident, $block:ident, $storage:ty, $key:ty, $mode:ty) => {
        pub(crate) mod $store {
            use super::*;
            pub fn serialize(value: &$storage, out: &mut String) {
                json::write_with_unbounded_sink(out, |out| serialize_bounded(value, out));
            }
            pub fn serialize_bounded(
                value: &$storage,
                out: &mut dyn JsonWriteSink,
            ) -> Result<(), json::BoundedJsonError> {
                use mv::storage::StorageReadOnly as _;
                let snapshot = value.snapshot();
                bounded(snapshot.revert_map().iter(), snapshot.current().iter(), out)
            }
        }
        pub(crate) mod $block {
            use super::*;
            type Field<'a> =
                crate::state::block_field::StorageField<'a, $key, RetainedCarrierBytes, $mode>;
            pub fn serialize(value: &Field<'_>, out: &mut String) {
                json::write_with_unbounded_sink(out, |out| serialize_bounded(value, out));
            }
            pub fn serialize_bounded(
                value: &Field<'_>,
                out: &mut dyn JsonWriteSink,
            ) -> Result<(), json::BoundedJsonError> {
                use mv::storage::StorageReadOnly as _;
                bounded(value.original_undo_entries(), value.iter(), out)
            }
        }
    };
}
adapters!(
    snapshot_json,
    snapshot_block_json,
    CarrierStorage,
    RetainedCarrierKey,
    CarrierStorageMode
);
adapters!(
    artifact_snapshot_json,
    artifact_snapshot_block_json,
    ArtifactStorage,
    iroha_data_model::smart_contract::ContractArtifactId,
    ArtifactStorageMode
);
