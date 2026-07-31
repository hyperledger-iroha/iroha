use std::{collections::BTreeMap, fs, ops::Bound, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use iroha_data_model::state_path::StatePath;
use norito::json::{self, Map, Value};

use crate::VMError;

/// Durable host-backed state overlay used by dev/test hosts.
///
/// Values are stored as raw `NoritoBytes` payloads keyed by a canonical state
/// path. Pointer-ABI envelopes exist only at the guest/host boundary.
/// When a persistence path is provided, the overlay writes a Norito JSON map
/// `{ path: base64(raw_payload_bytes) }` so test runs can survive VM restarts.
#[derive(Clone, Debug, Default)]
pub struct DurableStateOverlay {
    persist_path: Option<PathBuf>,
    data: BTreeMap<StatePath, Vec<u8>>,
}

/// Snapshot of a durable state overlay.
#[derive(Clone, Debug, Default)]
pub struct DurableStateSnapshot {
    data: BTreeMap<StatePath, Vec<u8>>,
}

impl DurableStateOverlay {
    fn validate_entry(path: &StatePath, value: &[u8]) -> Result<(), VMError> {
        crate::host::validate_state_path(path)?;
        crate::host::validate_state_value_payload_len(value.len())
    }

    /// Create an in-memory overlay with no persistence.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            persist_path: None,
            data: BTreeMap::new(),
        }
    }

    /// Create an overlay that persists to the given path.
    pub fn with_persist_path(path: PathBuf) -> Result<Self, VMError> {
        let mut overlay = Self {
            persist_path: Some(path),
            data: BTreeMap::new(),
        };
        overlay.reload_from_disk()?;
        Ok(overlay)
    }

    /// Return a copy of the raw `NoritoBytes` payload for a path, if present.
    pub fn get(&self, path: &StatePath) -> Option<Vec<u8>> {
        self.data.get(path).cloned()
    }

    /// Borrow a stored raw payload without materializing a copy.
    ///
    /// Stateful syscall handlers use this to inspect the response length and
    /// prove affordability before allocating or copying guest-visible output.
    pub fn get_ref(&self, path: &StatePath) -> Option<&[u8]> {
        self.data.get(path).map(Vec::as_slice)
    }

    /// Borrow a validated state-value payload.
    ///
    /// `set`, `restore`, and persisted-state loading enforce the payload-size
    /// bound before the value enters `data`. `STATE_LEN` can therefore read the
    /// stored vector length in constant time, independent of value size.
    pub(crate) fn value_payload_ref(&self, path: &StatePath) -> Result<Option<&[u8]>, VMError> {
        Ok(self.get_ref(path))
    }

    /// Insert or replace the raw payload for the provided path.
    pub fn set(&mut self, path: &StatePath, value: Vec<u8>) -> Result<(), VMError> {
        Self::validate_entry(path, &value)?;
        let key = path.clone();
        let prev = self.data.insert(key.clone(), value);
        if let Err(err) = self.flush() {
            match prev {
                Some(old) => {
                    self.data.insert(key, old);
                }
                None => {
                    self.data.remove(&key);
                }
            }
            return Err(err);
        }
        Ok(())
    }

    /// Iterate over the stored state paths.
    pub fn keys(&self) -> impl Iterator<Item = &StatePath> {
        self.data.keys()
    }

    /// Visit only keys in the ordered range sharing `prefix` as text.
    ///
    /// Callers apply any stricter path-segment rule. Starting at the ordered
    /// lower bound prevents an attacker-selected state prefix from charging or
    /// examining unrelated keys that sort before it.
    pub fn keys_with_text_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = &'a StatePath> + 'a {
        self.data
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .map(|(key, _)| key)
            .take_while(move |key| (*key).as_ref().starts_with(prefix))
    }

    /// Delete the raw payload for the provided path.
    pub fn del(&mut self, path: &StatePath) -> Result<(), VMError> {
        let prev = self.data.remove(path);
        if let Err(err) = self.flush() {
            if let Some(old) = prev {
                self.data.insert(path.clone(), old);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Take a snapshot of the current overlay contents.
    #[must_use]
    pub fn checkpoint(&self) -> DurableStateSnapshot {
        DurableStateSnapshot {
            data: self.data.clone(),
        }
    }

    /// Restore from a previously taken snapshot and persist to disk if needed.
    pub fn restore(&mut self, snapshot: &DurableStateSnapshot) -> Result<(), VMError> {
        for (path, value) in &snapshot.data {
            Self::validate_entry(path, value)?;
        }
        self.data = snapshot.data.clone();
        self.flush()
    }

    /// Force a flush of the current in-memory overlay to disk (no-op if no path).
    pub fn flush(&self) -> Result<(), VMError> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| VMError::NoritoInvalid)?;
        }
        let mut map = Map::new();
        for (k, v) in &self.data {
            map.insert(k.as_ref().to_owned(), Value::String(B64_STANDARD.encode(v)));
        }
        let serialized = json::to_vec(&Value::Object(map)).map_err(|_| VMError::NoritoInvalid)?;
        fs::write(path, serialized).map_err(|_| VMError::NoritoInvalid)?;
        Ok(())
    }

    fn reload_from_disk(&mut self) -> Result<(), VMError> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let Ok(bytes) = fs::read(path) else {
            // Nothing to load yet; treat as empty.
            return Ok(());
        };
        self.data = Self::decode_persisted(&bytes)?;
        Ok(())
    }

    fn decode_persisted(bytes: &[u8]) -> Result<BTreeMap<StatePath, Vec<u8>>, VMError> {
        let val: Value = json::from_slice(&bytes).map_err(|_| VMError::NoritoInvalid)?;
        let obj = val.as_object().ok_or(VMError::NoritoInvalid)?;
        let mut map = BTreeMap::new();
        for (k, v) in obj {
            // Reject an oversized persisted key before normalization allocates.
            if k.len() > crate::syscalls::STATE_MAX_PATH_BYTES {
                return Err(VMError::NoritoInvalid);
            }
            let path: StatePath = k.parse().map_err(|_| VMError::NoritoInvalid)?;
            let s = v.as_str().ok_or(VMError::NoritoInvalid)?.trim().to_string();
            let decoded = B64_STANDARD
                .decode(s.as_bytes())
                .map_err(|_| VMError::NoritoInvalid)?;
            Self::validate_entry(&path, &decoded)?;
            if map.insert(path, decoded).is_some() {
                return Err(VMError::NoritoInvalid);
            }
        }
        Ok(map)
    }
}

impl DurableStateSnapshot {
    #[must_use]
    pub fn new(data: BTreeMap<StatePath, Vec<u8>>) -> Self {
        Self { data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> StatePath {
        value.parse().expect("canonical state path")
    }

    #[test]
    fn raw_payload_ingress_enforces_value_bound() {
        let mut overlay = DurableStateOverlay::in_memory();
        let maximum = vec![0xabu8; crate::syscalls::STATE_MAX_VALUE_BYTES];
        let bounded = path("bounded");
        overlay
            .set(&bounded, maximum.clone())
            .expect("maximum raw payload must fit");
        assert_eq!(
            overlay
                .value_payload_ref(&bounded)
                .expect("valid overlay")
                .expect("stored payload"),
            maximum
        );

        let oversized = vec![0xcdu8; crate::syscalls::STATE_MAX_VALUE_BYTES + 1];
        let oversized_path = path("oversized");
        assert_eq!(
            overlay.set(&oversized_path, oversized),
            Err(VMError::NoritoInvalid)
        );
        assert!(overlay.get_ref(&oversized_path).is_none());
    }

    #[test]
    fn restore_validates_all_entries_before_replacing_state() {
        let mut overlay = DurableStateOverlay::in_memory();
        let existing = path("existing");
        overlay
            .set(&existing, b"kept".to_vec())
            .expect("seed overlay");
        let snapshot = DurableStateSnapshot::new(BTreeMap::from([
            (path("valid"), b"value".to_vec()),
            (
                path("oversized"),
                vec![0u8; crate::syscalls::STATE_MAX_VALUE_BYTES + 1],
            ),
        ]));

        assert_eq!(overlay.restore(&snapshot), Err(VMError::NoritoInvalid));
        assert_eq!(overlay.get_ref(&existing), Some(b"kept".as_slice()));
        assert!(overlay.get_ref(&path("valid")).is_none());
    }

    #[test]
    fn text_prefix_iterator_starts_at_the_ordered_lower_bound() {
        let mut overlay = DurableStateOverlay::in_memory();
        for path in ["accounts/1", "orders/1", "orders/2", "payments/1"] {
            overlay
                .set(&path.parse().expect("canonical state path"), Vec::new())
                .expect("insert bounded entry");
        }
        assert_eq!(
            overlay
                .keys_with_text_prefix("orders")
                .map(AsRef::<str>::as_ref)
                .collect::<Vec<_>>(),
            vec!["orders/1", "orders/2"]
        );
    }

    #[test]
    fn persisted_overlay_rejects_canonically_duplicate_state_paths() {
        let duplicate = r#"{"root/e\u0301":"AQ==","root/é":"Ag=="}"#;
        assert_eq!(
            DurableStateOverlay::decode_persisted(duplicate.as_bytes()),
            Err(VMError::NoritoInvalid)
        );
    }
}
