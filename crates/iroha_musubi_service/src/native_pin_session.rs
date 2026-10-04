//! Canonical local native-pin session selection and fresh offline custody initialization.
//!
//! This record is an original operator selection, not native authority, finality or permission.
//! Runtime State/Queue binding and complete operation/slot census remain in the daemon owner.

use std::path::Path;

use eyre::{Result, ensure};
use iroha_data_model::{NetworkId, account::AccountId, sorafs::pin_registry::StorageClass};
use iroha_wallet::operation_journal::Journal;
use norito::json::{JsonDeserialize, JsonSerialize};

/// Exact immutable local session original, shared by fresh generation and runtime ordinary open.
/// Decoding or constructing this selection grants no signing credential or native Check evidence.
#[derive(Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct NativeMusubiPinSessionV1 {
    /// Original genesis-derived native Global network.
    pub network: NetworkId,
    /// Original independent pin transaction authority.
    pub authority: AccountId,
    /// Nonzero original session selected once before publication of the generated profile.
    pub session: [u8; 32],
    /// Exact storage class: 0 Hot, 1 Warm, 2 Cold.
    pub storage_class: u8,
    /// Original finite positive retention horizon; runtime validates its native configured bounds.
    pub retention_horizon_secs: u64,
}

impl NativeMusubiPinSessionV1 {
    /// Construct the original local session from explicit typed policy inputs.
    ///
    /// # Errors
    /// Refuses a non-Global network, zero session, indirect authority or invalid/oversized original.
    pub fn new(
        network: NetworkId,
        authority: AccountId,
        session: [u8; 32],
        storage_class: StorageClass,
        retention_horizon_secs: u64,
    ) -> Result<Self> {
        let selected = Self {
            network,
            authority,
            session,
            storage_class: match storage_class {
                StorageClass::Hot => 0,
                StorageClass::Warm => 1,
                StorageClass::Cold => 2,
            },
            retention_horizon_secs,
        };
        selected.validate()?;
        Ok(selected)
    }

    /// Validate the complete bounded local original without reading live custody or network state.
    ///
    /// # Errors
    /// Refuses invalid selected identities, unsupported class, empty horizon or codec resources.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.network.as_bytes()[31] & 1 == 1
                && self.session != [0; 32]
                && self.authority.try_signatory().is_some()
                && self.storage_class <= 2
                && self.retention_horizon_secs > 0,
            "native pin session original is invalid"
        );
        norito::json::to_json_bounded_boxed(self, 8192)?;
        Ok(())
    }

    /// Atomically initialize fresh private session custody using the sole wallet journal owner.
    /// The returned journal retains its exclusive lock; generation drops it before publication.
    /// Existing or partially published custody is never adopted or repaired by this method.
    ///
    /// # Errors
    /// Refuses invalid selection, occupied namespace, unsafe private custody or failed durability.
    pub fn initialize_private_journal(&self, path: &Path) -> Result<Journal> {
        self.validate()?;
        Journal::create_prepared(path, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    fn selection() -> NativeMusubiPinSessionV1 {
        let key = KeyPair::from_seed(vec![0xB1; 32], Algorithm::Ed25519);
        NativeMusubiPinSessionV1::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"local pin selection",
            ))),
            AccountId::new(key.public_key().clone()),
            [0xB2; 32],
            StorageClass::Warm,
            90 * 24 * 60 * 60,
        )
        .unwrap()
    }

    #[test]
    fn offline_initialization_is_atomic_exact_and_never_adopts_existing_history() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pin-session");
        let selected = selection();
        let held = selected.initialize_private_journal(&path).unwrap();
        assert_eq!(
            held.read_operation::<NativeMusubiPinSessionV1>().unwrap(),
            selected
        );
        assert!(Journal::open(&path).is_err(), "original lock is still held");
        assert!(selected.initialize_private_journal(&path).is_err());
        drop(held);
        let original = std::fs::read(path.join("operation.json")).unwrap();
        let held = Journal::open(&path).unwrap();
        assert_eq!(
            held.read_operation::<NativeMusubiPinSessionV1>().unwrap(),
            selected
        );
        drop(held);
        assert_eq!(
            std::fs::read(path.join("operation.json")).unwrap(),
            original
        );
        std::fs::remove_file(path.join("operation.json")).unwrap();
        assert!(selected.initialize_private_journal(&path).is_err());
        assert!(!path.join("operation.json").exists());
    }

    #[test]
    fn canonical_original_rejects_missing_extra_and_invalid_identity_fields() {
        let selected = selection();
        let json = norito::json::to_json(&selected).unwrap();
        assert_eq!(
            norito::json::from_str::<NativeMusubiPinSessionV1>(&json).unwrap(),
            selected
        );
        let extra = json.replacen('{', "{\"provider\":1,", 1);
        assert!(norito::json::from_str::<NativeMusubiPinSessionV1>(&extra).is_err());
        let missing = json.replace("\"storage_class\":1,", "");
        assert_ne!(missing, json);
        assert!(norito::json::from_str::<NativeMusubiPinSessionV1>(&missing).is_err());
        for case in 0..3 {
            let mut invalid = selection();
            match case {
                0 => invalid.session = [0; 32],
                1 => invalid.storage_class = 3,
                2 => invalid.retention_horizon_secs = 0,
                _ => unreachable!(),
            }
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("invalid");
            assert!(invalid.initialize_private_journal(&path).is_err());
            assert!(!path.exists());
        }
    }
}
