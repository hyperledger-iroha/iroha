//! Native filesystem/control-flow tests over signed certificate fixtures; no parent business
//! rules or running network are inferred from these self-consistent authentication fixtures.

use super::*;
use crate::{
    attachment::tests::Fixture,
    bootstrap::{NetworkRelease, ReleaseCheckpointStore, ReleaseTrust, SignedNetworkCheckpoint},
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair};
use iroha_data_model::{
    sumeragi_finality::{SumeragiFinalityAttestation, SumeragiFinalityProof},
    transaction::FeePaymentIntent,
};
use iroha_model_base::peer::PeerId;
use iroha_primitives::numeric::Quantity;
use std::{cell::Cell, collections::BTreeMap, num::NonZeroU64, time::Duration};

fn configuration(fixture: &Fixture) -> Config {
    let key = KeyPair::from_seed(vec![150; 32], Algorithm::Ed25519);
    Config::load_table(
        "explicit-admin-fixture.toml",
        toml::toml! {
            chain = (fixture.parent.chain_id())
            network_id = (fixture.parent.network_id().to_string())
            torii_url = "https://parent.example/"
            [account]
            chain_discriminant = 753
            public_key = (key.public_key().to_string())
            private_key = (ExposedPrivateKey(key.private_key().clone()).to_string())
        },
    )
    .unwrap()
}
fn bootstrap(fixture: &Fixture, path: &Path) -> AuthenticatedBootstrap {
    let now = native_operation::now_ms().unwrap();
    let checkpoint = fixture.parent.checkpoint();
    let key = KeyPair::from_seed(vec![49; 32], Algorithm::Ed25519);
    let release = NetworkRelease {
        network_name: "fixture".into(),
        serial: 1,
        generation: 1,
        network_id: checkpoint.network_id(),
        chain_id: checkpoint.chain_id().into(),
        issued_at_ms: now - 1_000,
        expires_at_ms: now + 600_000,
        torii_roots: vec!["https://parent.example/".into()],
        account_chain_discriminant: 753,
        native_world_schema: Hash::new(b"independently qualified fixture World schema"),
        peers: checkpoint
            .tip()
            .committee
            .iter()
            .map(|v| crate::bootstrap::ReleasePeer {
                node_id: PeerId::new(v.public_key.clone()),
                torii_root: "https://parent.example/".into(),
            })
            .collect(),
        faucet: None,
        build_registry: None,
        checkpoint_hash: Hash::new(checkpoint.encode_canonical().unwrap()),
        checkpoint_height: checkpoint.height(),
        checkpoint_block_hash: checkpoint.block_hash().into(),
    };
    let bytes = SignedNetworkCheckpoint::sign(release, &checkpoint, key.private_key())
        .unwrap()
        .encode_canonical()
        .unwrap();
    let trust = ReleaseTrust::new("fixture".into(), key.public_key().clone(), 1).unwrap();
    ReleaseCheckpointStore::open(path)
        .unwrap()
        .authenticate(&trust, &bytes, now)
        .unwrap()
}
fn options() -> BoundedTransactionOptions {
    BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::from([(
            iroha_wallet::operations::XOR_ASSET_DEFINITION
                .parse()
                .unwrap(),
            Quantity::from(10_u32),
        )]),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}
struct Offline(Cell<usize>);
impl FinalitySource for Offline {
    type Error = std::io::Error;
    fn finality_proof(
        &self,
        _: NonZeroU64,
    ) -> std::result::Result<SumeragiFinalityProof, Self::Error> {
        self.0.set(self.0.get() + 1);
        Err(std::io::Error::other("offline"))
    }
    fn latest_attestation(
        &self,
        _: &PeerId,
        _: &[u8; 32],
    ) -> std::result::Result<SumeragiFinalityAttestation, Self::Error> {
        self.0.set(self.0.get() + 1);
        Err(std::io::Error::other("offline"))
    }
}
fn origin(
    fixture: &Fixture,
    config: &Config,
    utc: u64,
    options: &BoundedTransactionOptions,
) -> Origin {
    Origin {
        identity: fixture.identity.clone(),
        administrator: config.account.clone(),
        torii_root: config.torii_api_url.to_string(),
        chain_discriminant: config.account_chain_discriminant,
        terms: Terms::new(utc, options).unwrap(),
        checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
    }
}

#[test]
fn original_amx_administrator_child_fees_time_and_checkpoint_survive_reopen_and_refuse_substitution()
 {
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let root = tempfile::tempdir().unwrap();
    let config = configuration(&fixture);
    let options = options();
    assert_ne!(config.account, fixture.identity.owner);
    let utc = native_operation::now_ms().unwrap() + 60_000;
    let original = origin(&fixture, &config, utc, &options);
    original
        .validate(&fixture.identity, &config, utc, &options)
        .unwrap();
    let bytes = encode_bounded(&original, MAX_RECORD_BYTES).unwrap();
    let path = root.path().join("attachment");
    let store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    let saved = store
        .directory
        .publish_private_child(DIRECTORY, &[(ORIGIN, &bytes)])
        .unwrap();
    let identity = saved.identity().unwrap();
    drop(saved);
    drop(store);
    let store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    let saved = store.directory.open_child(DIRECTORY).unwrap();
    assert_eq!(saved.identity().unwrap(), identity);
    let retained: Origin = norito::decode_canonical_with_limits(
        &saved.read(ORIGIN, MAX_RECORD_BYTES).unwrap(),
        norito::canonical_decode_limits(bytes.len()),
    )
    .unwrap();
    retained
        .validate(&fixture.identity, &config, utc, &options)
        .unwrap();
    assert_eq!(
        saved.read(ORIGIN, MAX_RECORD_BYTES).unwrap().as_slice(),
        bytes
    );
    let mut changed_signer = config.clone();
    let key = KeyPair::from_seed(vec![151; 32], Algorithm::Ed25519);
    changed_signer.account = AccountId::new(key.public_key().clone());
    changed_signer.key_pair = key;
    assert!(
        retained
            .validate(&fixture.identity, &changed_signer, utc, &options)
            .is_err(),
        "original AMX administrator must remain bound after reopen"
    );
    let mut changed_child = fixture.identity.clone();
    changed_child.registration.instance[0] ^= 1;
    assert!(
        retained
            .validate(&changed_child, &config, utc, &options)
            .is_err()
    );
    let mut changed_fee = options.clone();
    changed_fee
        .max_total_fees
        .values_mut()
        .for_each(|v| *v = Quantity::from(11_u32));
    assert!(
        retained
            .validate(&fixture.identity, &config, utc, &changed_fee)
            .is_err()
    );
    assert!(
        retained
            .validate(&fixture.identity, &config, utc + 1, &options)
            .is_err()
    );
    let mut changed = retained.clone();
    changed.checkpoint = fixture.child.checkpoint().encode_canonical().unwrap();
    assert!(
        changed
            .validate(&fixture.identity, &config, utc, &options)
            .is_err()
    );
    assert_eq!(
        saved.read(ORIGIN, MAX_RECORD_BYTES).unwrap().as_slice(),
        bytes
    );
}

#[test]
fn administrative_amx_partial_or_changed_origin_is_no_http_and_no_repair() {
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let root = tempfile::tempdir().unwrap();
    let config = configuration(&fixture);
    let options = options();
    let utc = native_operation::now_ms().unwrap() + 60_000;
    let bootstrap = bootstrap(&fixture, &root.path().join("release"));
    let mut parent = ParentFinalityStore::open(&root.path().join("parent"), &bootstrap).unwrap();
    let mut store =
        AttachmentStore::open(&root.path().join("attachment"), fixture.identity.clone()).unwrap();
    store
        .validate_amx_parent(&config, &bootstrap, &parent)
        .unwrap();
    let saved = store
        .directory
        .publish_private_child(
            DIRECTORY,
            &[("partial", b"original incomplete publication")],
        )
        .unwrap();
    let source = Offline(Cell::new(0));
    assert!(
        store
            .advance_amx_registration(&config, &bootstrap, &mut parent, &source, utc, &options)
            .is_err()
    );
    assert_eq!(source.0.get(), 0);
    assert!(!saved.path().join(ORIGIN).exists());
    assert!(!saved.path().join("transaction").exists());
    // A separate complete origin proves changed authorization refusal rather than partial inventory.
    let mut complete =
        AttachmentStore::open(&root.path().join("complete"), fixture.identity.clone()).unwrap();
    let bytes =
        encode_bounded(&origin(&fixture, &config, utc, &options), MAX_RECORD_BYTES).unwrap();
    let retained = complete
        .directory
        .publish_private_child(DIRECTORY, &[(ORIGIN, &bytes)])
        .unwrap();
    validate_inventory(&retained).unwrap();
    assert!(
        complete
            .advance_amx_registration(&config, &bootstrap, &mut parent, &source, utc + 1, &options)
            .is_err()
    );
    assert_eq!(source.0.get(), 0);
    assert!(!retained.path().join("transaction").exists());
    assert_eq!(
        retained.read(ORIGIN, MAX_RECORD_BYTES).unwrap().as_slice(),
        bytes.as_slice()
    );
    retained
        .write_atomic("foreign", b"foreign", PublishMode::CreateNew)
        .unwrap();
    assert!(validate_inventory(&retained).is_err());
}

#[test]
fn administrative_amx_requires_fresh_original_parent_before_signing_and_cancellation_is_no_work() {
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let root = tempfile::tempdir().unwrap();
    let config = configuration(&fixture);
    let options = options();
    let utc = native_operation::now_ms().unwrap() + 60_000;
    let bootstrap = bootstrap(&fixture, &root.path().join("release"));
    let mut parent = ParentFinalityStore::open(&root.path().join("parent"), &bootstrap).unwrap();
    let mut store =
        AttachmentStore::open(&root.path().join("attachment"), fixture.identity.clone()).unwrap();
    let source = Offline(Cell::new(0));
    assert!(
        store
            .advance_amx_registration(&config, &bootstrap, &mut parent, &source, utc, &options)
            .is_err()
    );
    assert!(source.0.get() >= 3);
    assert!(!store.directory.path().join(DIRECTORY).exists());
    let signal = Arc::new(std::sync::atomic::AtomicBool::new(true));
    store.bind_cancellation(signal).unwrap();
    let before = source.0.get();
    assert!(matches!(
        store.advance_amx_registration(&config, &bootstrap, &mut parent, &source, utc, &options),
        Err(AttachmentError::Cancelled)
    ));
    assert_eq!(source.0.get(), before);
    assert!(matches!(
        native::<()>(Err(crate::managed::Error::NativeDeadline)),
        Err(AttachmentError::NativeOperation(
            crate::managed::Error::NativeDeadline
        ))
    ));
}
