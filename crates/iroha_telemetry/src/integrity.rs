//! Telemetry integrity helpers (hash chaining and optional keyed signatures).

use std::path::{Path, PathBuf};

use iroha_config::parameters::actual::TelemetryIntegrity as TelemetryIntegrityConfig;
use norito::json::{Map, Value};
use thiserror::Error;

const STATE_VERSION: u64 = 1;
const STATE_FILE_PREFIX: &str = "telemetry_integrity_";

#[derive(Debug, Clone)]
pub struct IntegrityConfig {
    pub enabled: bool,
    pub signing_key: Option<[u8; 32]>,
    pub signing_key_id: Option<String>,
}

impl From<TelemetryIntegrityConfig> for IntegrityConfig {
    fn from(config: TelemetryIntegrityConfig) -> Self {
        Self {
            enabled: config.enabled,
            signing_key: config.signing_key,
            signing_key_id: config.signing_key_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChainState {
    config: IntegrityConfig,
    seq: u64,
    prev_hash: [u8; 32],
    state_path: Option<PathBuf>,
}

impl ChainState {
    pub fn new_with_state_path(
        config: TelemetryIntegrityConfig,
        state_path: Option<PathBuf>,
    ) -> Self {
        let mut chain = Self::from_config(config.into(), state_path);
        chain.load_state();
        chain
    }

    pub fn new_with_kind(config: TelemetryIntegrityConfig, kind: &str) -> Self {
        let state_path = state_path_for(kind, config.state_dir.as_ref());
        Self::new_with_state_path(config, state_path)
    }

    pub fn from_config(config: IntegrityConfig, state_path: Option<PathBuf>) -> Self {
        Self {
            config,
            seq: 1,
            prev_hash: [0_u8; 32],
            state_path,
        }
    }

    pub fn attach_chain(&mut self, map: &mut Map) -> Result<(), IntegrityError> {
        if !self.config.enabled {
            return Ok(());
        }

        let payload = norito::json::to_vec(map)?;
        let seq = self.seq;
        let prev_hash = self.prev_hash;
        let hash = compute_hash(prev_hash, seq, &payload);
        let signature = self
            .config
            .signing_key
            .map(|key| blake3::keyed_hash(&key, &hash).as_bytes().to_owned());

        self.prev_hash = hash;
        self.seq = self.seq.wrapping_add(1);

        let mut chain = Map::new();
        chain.insert("seq".into(), Value::from(seq));
        chain.insert("prev_hash".into(), Value::from(hex::encode(prev_hash)));
        chain.insert("hash".into(), Value::from(hex::encode(hash)));
        if let Some(signature) = signature {
            chain.insert("signature".into(), Value::from(hex::encode(signature)));
            if let Some(key_id) = self.config.signing_key_id.clone() {
                chain.insert("key_id".into(), Value::from(key_id));
            }
        }

        map.insert("chain".into(), Value::Object(chain));
        self.persist_state();
        Ok(())
    }

    fn load_state(&mut self) {
        if !self.config.enabled {
            return;
        }
        let Some(path) = self.state_path.as_ref() else {
            return;
        };

        match load_state_snapshot(path) {
            Ok(Some(snapshot)) => {
                self.seq = snapshot.seq;
                self.prev_hash = snapshot.prev_hash;
            }
            Ok(None) => {}
            Err(message) => {
                iroha_logger::warn!(
                    path = %path.display(),
                    %message,
                    "failed to load telemetry integrity state; starting new chain"
                );
            }
        }
    }

    fn persist_state(&self) {
        let Some(path) = self.state_path.as_ref() else {
            return;
        };

        if let Err(message) = persist_state_snapshot(path, self.seq, self.prev_hash) {
            iroha_logger::warn!(
                path = %path.display(),
                %message,
                "failed to persist telemetry integrity state"
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum IntegrityError {
    #[error("failed to serialize telemetry payload for integrity hash: {0}")]
    Serialize(#[from] norito::json::Error),
}

#[derive(Debug, Clone)]
struct ChainStateSnapshot {
    seq: u64,
    prev_hash: [u8; 32],
}

fn compute_hash(prev_hash: [u8; 32], seq: u64, payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&prev_hash);
    hasher.update(&seq.to_be_bytes());
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

fn state_path_for(kind: &str, state_dir: Option<&PathBuf>) -> Option<PathBuf> {
    state_dir.map(|dir| dir.join(format!("{STATE_FILE_PREFIX}{kind}.json")))
}

fn load_state_snapshot(path: &Path) -> Result<Option<ChainStateSnapshot>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!("failed to read state file: {err}"));
        }
    };

    let value: Value = norito::json::from_slice(&bytes)
        .map_err(|err| format!("failed to decode state file: {err}"))?;
    let map = value
        .as_object()
        .ok_or_else(|| "state file payload is not an object".to_string())?;
    let version = map
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if version != STATE_VERSION {
        return Err(format!(
            "unsupported state file version {version}, expected {STATE_VERSION}"
        ));
    }
    let seq = map
        .get("seq")
        .and_then(Value::as_u64)
        .ok_or_else(|| "state file missing seq".to_string())?;
    if seq == 0 {
        return Err("state file seq must be >= 1".to_string());
    }
    let prev_hash_hex = map
        .get("prev_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "state file missing prev_hash".to_string())?;
    let decoded = hex::decode(prev_hash_hex)
        .map_err(|err| format!("state file prev_hash is not hex: {err}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "state file prev_hash must be 32 bytes (got {})",
            decoded.len()
        ));
    }
    let mut prev_hash = [0_u8; 32];
    prev_hash.copy_from_slice(&decoded);
    Ok(Some(ChainStateSnapshot { seq, prev_hash }))
}

fn persist_state_snapshot(path: &Path, seq: u64, prev_hash: [u8; 32]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create state directory: {err}"))?;
    }

    let mut map = Map::new();
    map.insert("version".into(), Value::from(STATE_VERSION));
    map.insert("seq".into(), Value::from(seq));
    map.insert("prev_hash".into(), Value::from(hex::encode(prev_hash)));
    let payload = norito::json::to_vec(&map)
        .map_err(|err| format!("failed to serialize state file: {err}"))?;

    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, &payload)
        .map_err(|err| format!("failed to write temp state file: {err}"))?;
    if let Err(err) = std::fs::rename(&tmp_path, path) {
        std::fs::write(path, &payload).map_err(|write_err| {
            format!(
                "failed to replace state file: rename failed ({err}); write failed ({write_err})"
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn chain_increments_with_prev_hash() {
        let config = TelemetryIntegrityConfig {
            enabled: true,
            state_dir: None,
            signing_key: None,
            signing_key_id: None,
        };
        let mut chain = ChainState::new_with_state_path(config, None);
        let mut map = Map::new();
        map.insert("msg".into(), Value::from("hello"));

        let payload = norito::json::to_vec(&map).expect("payload");
        let expected_hash = compute_hash([0_u8; 32], 1, &payload);

        chain.attach_chain(&mut map).expect("attach chain");
        let chain_map = map
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        assert_eq!(chain_map.get("seq").and_then(Value::as_u64), Some(1));
        assert_eq!(
            chain_map.get("prev_hash").and_then(Value::as_str),
            Some("0000000000000000000000000000000000000000000000000000000000000000")
        );
        let expected_hash_hex = hex::encode(expected_hash);
        assert_eq!(
            chain_map.get("hash").and_then(Value::as_str),
            Some(expected_hash_hex.as_str())
        );

        let mut second = Map::new();
        second.insert("msg".into(), Value::from("world"));
        let payload = norito::json::to_vec(&second).expect("payload");
        let expected_hash = compute_hash(expected_hash, 2, &payload);

        chain.attach_chain(&mut second).expect("attach chain");
        let chain_map = second
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        assert_eq!(chain_map.get("seq").and_then(Value::as_u64), Some(2));
        let expected_hash_hex = hex::encode(expected_hash);
        assert_eq!(
            chain_map.get("hash").and_then(Value::as_str),
            Some(expected_hash_hex.as_str())
        );
    }

    #[test]
    fn chain_includes_signature_when_keyed() {
        let key = [7_u8; 32];
        let config = TelemetryIntegrityConfig {
            enabled: true,
            state_dir: None,
            signing_key: Some(key),
            signing_key_id: Some("primary".to_string()),
        };
        let mut chain = ChainState::new_with_state_path(config, None);
        let mut map = Map::new();
        map.insert("msg".into(), Value::from("signed"));

        let payload = norito::json::to_vec(&map).expect("payload");
        let hash = compute_hash([0_u8; 32], 1, &payload);
        let signature = blake3::keyed_hash(&key, &hash);

        chain.attach_chain(&mut map).expect("attach chain");
        let chain_map = map
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        let signature_hex = hex::encode(signature.as_bytes());
        assert_eq!(
            chain_map.get("signature").and_then(Value::as_str),
            Some(signature_hex.as_str())
        );
        assert_eq!(
            chain_map.get("key_id").and_then(Value::as_str),
            Some("primary")
        );
    }

    #[test]
    fn chain_state_persists_across_restarts() {
        let dir = std::env::temp_dir().join(format!(
            "iroha-telemetry-integrity-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let state_path = dir.join("telemetry_integrity_ws.json");

        let config = TelemetryIntegrityConfig {
            enabled: true,
            state_dir: None,
            signing_key: None,
            signing_key_id: None,
        };

        let mut first = ChainState::new_with_state_path(config.clone(), Some(state_path.clone()));
        let mut map = Map::new();
        map.insert("msg".into(), Value::from("first"));
        let payload = norito::json::to_vec(&map).expect("payload");
        let expected_hash = compute_hash([0_u8; 32], 1, &payload);

        first.attach_chain(&mut map).expect("attach chain");
        assert!(state_path.exists());

        let mut second = ChainState::new_with_state_path(config, Some(state_path));
        let mut map = Map::new();
        map.insert("msg".into(), Value::from("second"));
        second.attach_chain(&mut map).expect("attach chain");
        let chain_map = map
            .get("chain")
            .and_then(Value::as_object)
            .expect("chain map");
        assert_eq!(chain_map.get("seq").and_then(Value::as_u64), Some(2));
        assert_eq!(
            chain_map.get("prev_hash").and_then(Value::as_str),
            Some(hex::encode(expected_hash).as_str())
        );
    }

    #[test]
    fn state_path_for_kind_uses_prefix() {
        let dir = PathBuf::from("telemetry-state");
        let path = state_path_for("ws", Some(&dir)).expect("state path");
        assert_eq!(path, dir.join("telemetry_integrity_ws.json"));
    }
}
