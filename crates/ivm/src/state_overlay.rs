use std::{collections::BTreeMap, fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use norito::json::{self, Map, Value};

use crate::VMError;

/// Durable host-backed state overlay used by dev/test hosts.
///
/// Values are stored as raw TLV envelopes keyed by a canonical state path.
/// When a persistence path is provided, the overlay writes a Norito JSON map
/// `{ path: base64(tlv_bytes) }` so test runs can survive VM restarts.
#[derive(Clone, Debug, Default)]
pub struct DurableStateOverlay {
    persist_path: Option<PathBuf>,
    data: BTreeMap<String, Vec<u8>>,
}

/// Snapshot of a durable state overlay.
#[derive(Clone, Debug, Default)]
pub struct DurableStateSnapshot {
    data: BTreeMap<String, Vec<u8>>,
}

impl DurableStateOverlay {
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

    /// Return a copy of the TLV envelope for a path, if present.
    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.data.get(path).cloned()
    }

    /// Insert or replace the TLV envelope for the provided path.
    pub fn set(&mut self, path: &str, value: Vec<u8>) -> Result<(), VMError> {
        let key = path.to_string();
        let prev = self.data.insert(key.clone(), value);
        if let Err(err) = self.flush() {
            match prev {
                Some(old) => {
                    self.data.insert(key, old);
                }
                None => {
                    self.data.remove(path);
                }
            }
            return Err(err);
        }
        Ok(())
    }

    /// Iterate over the stored state paths.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    /// Delete the TLV envelope for the provided path.
    pub fn del(&mut self, path: &str) -> Result<(), VMError> {
        let prev = self.data.remove(path);
        if let Err(err) = self.flush() {
            if let Some(old) = prev {
                self.data.insert(path.to_string(), old);
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
            map.insert(k.clone(), Value::String(B64_STANDARD.encode(v)));
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
        let val: Value = json::from_slice(&bytes).map_err(|_| VMError::NoritoInvalid)?;
        let obj = val.as_object().ok_or(VMError::NoritoInvalid)?;
        let mut map = BTreeMap::new();
        for (k, v) in obj {
            let s = v.as_str().ok_or(VMError::NoritoInvalid)?.trim().to_string();
            let decoded = B64_STANDARD
                .decode(s.as_bytes())
                .map_err(|_| VMError::NoritoInvalid)?;
            map.insert(k.clone(), decoded);
        }
        self.data = map;
        Ok(())
    }
}

impl DurableStateSnapshot {
    #[must_use]
    pub fn new(data: BTreeMap<String, Vec<u8>>) -> Self {
        Self { data }
    }
}
