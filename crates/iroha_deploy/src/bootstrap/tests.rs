//! Native identity pins, bounded checkpoint persistence and exclusive custody regressions.
//!
//! Genuine fixture certificates cover synthetic execution outputs; these tests do not qualify a
//! live network. Shared helpers preserve the same checkpoint verifier used by production stores.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

pub(crate) fn metadata(checkpoint: &SumeragiFinalityCheckpoint) -> NetworkMetadata {
    NetworkMetadata {
        network_name: "fixture".into(),
        network_id: checkpoint.network_id(),
        chain_id: checkpoint.chain_id().into(),
        torii_roots: vec!["https://torii.example/".into()],
        account_chain_discriminant: 753,
        native_world_schema: Hash::new(b"independently qualified fixture World schema"),
        peers: checkpoint
            .tip()
            .committee
            .iter()
            .map(|validator| BootstrapPeer {
                node_id: iroha_model_base::peer::PeerId::new(validator.public_key.clone()),
                torii_root: "https://torii.example/".into(),
            })
            .collect(),
        faucet: None,
        build_registry: None,
    }
}

pub(crate) fn authenticated(
    metadata: NetworkMetadata,
    checkpoint: &SumeragiFinalityCheckpoint,
) -> AuthenticatedBootstrap {
    let verifier = verify_checkpoint(&metadata, &checkpoint.encode_canonical().unwrap()).unwrap();
    AuthenticatedBootstrap { metadata, verifier }
}

fn profile(checkpoint: &SumeragiFinalityCheckpoint) -> InstalledNetworkProfile {
    InstalledNetworkProfile::new(
        "fixture".into(),
        "https://torii.example/",
        checkpoint.chain_id().into(),
        753,
    )
    .unwrap()
}

pub(crate) fn discovered(checkpoint: &SumeragiFinalityCheckpoint) -> NativeBootstrap {
    NativeBootstrap {
        network_id: checkpoint.network_id(),
        verifier: FinalityVerifier::from_checkpoint(
            checkpoint.clone(),
            checkpoint.network_id(),
            checkpoint.chain_id(),
        )
        .unwrap(),
        capabilities: iroha_torii_shared::account_capabilities::AccountCapabilitiesV1 {
            schema_version: 1,
            network_id: checkpoint.network_id(),
            network_prefix: 753,
            allowed_signing: vec!["ed25519".into()],
        },
        faucet: iroha_torii_shared::account_faucet_policy::AccountFaucetAdvertisement {
            schema_version: 1,
            network_id: checkpoint.network_id(),
            network_prefix: 753,
            authority: iroha_data_model::account::AccountId::new(key(81).public_key().clone()),
            asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
            amount: 25_000_u64.into(),
        },
    }
}

#[test]
fn native_bootstrap_pins_network_and_resumes_after_exclusive_reopen() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bootstrap");
    let store = BootstrapStore::open(&path).unwrap();
    assert!(store.load(&selected).unwrap().is_none());
    assert!(matches!(
        BootstrapStore::open(&path),
        Err(BootstrapError::Busy)
    ));
    let result = store.accept(&selected, discovered(&checkpoint)).unwrap();
    assert_eq!(result.metadata().network_id, checkpoint.network_id());
    assert_eq!(result.metadata().chain_id, selected.chain_id());
    assert_eq!(
        result.metadata().torii_roots,
        [selected.torii_root().to_string()]
    );
    assert_eq!(result.metadata().account_chain_discriminant, 753);
    assert!(result.metadata().peers.is_empty());
    assert_eq!(
        result.metadata().faucet.as_ref().unwrap().max_operation_fee,
        1000_u64.into()
    );
    assert_eq!(result.into_verifier().checkpoint(), &checkpoint);
    let bytes = std::fs::read(path.join("accepted.nrt")).unwrap();
    store.accept(&selected, discovered(&checkpoint)).unwrap();
    assert_eq!(std::fs::read(path.join("accepted.nrt")).unwrap(), bytes);
    drop(store);
    let store = BootstrapStore::open(&path).unwrap();
    assert_eq!(
        store
            .load(&selected)
            .unwrap()
            .unwrap()
            .into_verifier()
            .checkpoint(),
        &checkpoint
    );
}

#[test]
fn bootstrap_rejects_profile_reset_policy_and_discovery_substitution() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bootstrap");
    let store = BootstrapStore::open(&path).unwrap();
    store.accept(&selected, discovered(&checkpoint)).unwrap();
    let bytes = std::fs::read(path.join("accepted.nrt")).unwrap();
    for changed in [
        InstalledNetworkProfile::new(
            "other".into(),
            selected.torii_root().as_str(),
            selected.chain_id().into(),
            753,
        )
        .unwrap(),
        InstalledNetworkProfile::new(
            "fixture".into(),
            "https://other.example/",
            selected.chain_id().into(),
            753,
        )
        .unwrap(),
        InstalledNetworkProfile::new(
            "fixture".into(),
            selected.torii_root().as_str(),
            "other-chain".into(),
            753,
        )
        .unwrap(),
        InstalledNetworkProfile::new(
            "fixture".into(),
            selected.torii_root().as_str(),
            selected.chain_id().into(),
            754,
        )
        .unwrap(),
    ] {
        assert!(store.load(&changed).is_err());
        assert!(store.accept(&changed, discovered(&checkpoint)).is_err());
    }
    let mut reset = NativeFinalityFixture::start_with_mode(
        checkpoint.chain_id(),
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    );
    reset.certify(reset.block_with_submitted_work(reset.next_header()));
    assert_ne!(reset.network_id(), checkpoint.network_id());
    assert!(
        store
            .accept(&selected, discovered(&reset.checkpoint()))
            .is_err()
    );
    let mut changed = discovered(&checkpoint);
    changed.faucet.amount = 50_000_u64.into();
    assert!(store.accept(&selected, changed).is_err());
    let mut changed = discovered(&checkpoint);
    changed.capabilities.network_prefix = 754;
    assert!(store.accept(&selected, changed).is_err());
    let mut changed = discovered(&checkpoint);
    changed.faucet.network_id = reset.network_id();
    assert!(store.accept(&selected, changed).is_err());
    assert_eq!(std::fs::read(path.join("accepted.nrt")).unwrap(), bytes);
}

#[test]
fn native_metadata_caps_spending_and_rejects_invalid_faucet() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let mut native = discovered(&checkpoint);
    native.faucet.amount = 25_u64.into();
    let metadata = native_metadata(&selected, &native).unwrap();
    let faucet = metadata.faucet.unwrap();
    assert_eq!(faucet.max_operation_fee, 25_u64.into());
    assert_eq!(faucet.max_namespace_rent, 25_u64.into());
    native.faucet.amount = 0_u64.into();
    assert!(native_metadata(&selected, &native).is_err());
}

#[test]
fn native_bootstrap_refuses_unsupported_managed_owner_before_acceptance() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let store = BootstrapStore::open(&directory.path().join("bootstrap")).unwrap();
    let mut native = discovered(&checkpoint);
    native.capabilities.allowed_signing = vec!["secp256k1".into()];
    assert!(store.accept(&selected, native).is_err());
    assert!(store.load(&selected).unwrap().is_none());
}

#[test]
fn same_decision_with_alternate_quorum_preserves_the_original_checkpoint_witness() {
    use iroha_data_model::block::{CommitCertificate, decode_framed_signed_block};
    use iroha_sumeragi::{
        message::Qc,
        types::{AggregateSignature, Bitmap},
    };

    let fixture = NativeFinalityFixture::new();
    let original = fixture.checkpoint();
    let mut proof = fixture.latest().clone();
    let mut block = decode_framed_signed_block(&proof.block_wire).unwrap();
    let certificate = block.commit_certificate().unwrap();
    let consensus_header = certificate.consensus_header().to_vec();
    let result_preimage = certificate.result_preimage().to_vec();
    let availability = certificate.availability().to_vec();
    let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    let mut keys: Vec<_> = (1..=4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
    assert!(
        keys.iter()
            .zip(&proof.committee)
            .all(|(key, member)| key.public_key() == &member.public_key)
    );
    let chosen = [1, 2, 3];
    qc.signers = Bitmap::from_indices(keys.len(), chosen).unwrap();
    let shares: Vec<_> = chosen
        .into_iter()
        .map(|index| {
            iroha_crypto::bls::consensus::sign_preimage(
                keys[index as usize].private_key(),
                &qc.preimage(),
            )
            .unwrap()
        })
        .collect();
    qc.agg_sig = AggregateSignature(iroha_crypto::bls::consensus::aggregate(&shares).unwrap());
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        consensus_header,
        norito::encode_canonical(&qc).unwrap(),
        result_preimage,
        availability,
    )));
    proof.block_wire = block.encode_wire().unwrap();
    fixture
        .verifier()
        .verify_same_decision(fixture.latest(), &proof)
        .unwrap();
    let alternate = fixture.verifier().export_checkpoint(&proof).unwrap();
    assert_ne!(
        original.encode_canonical().unwrap(),
        alternate.encode_canonical().unwrap()
    );
    let selected = profile(&original);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bootstrap");
    let store = BootstrapStore::open(&path).unwrap();
    store.accept(&selected, discovered(&original)).unwrap();
    let retained = std::fs::read(path.join("accepted.nrt")).unwrap();
    let accepted = store.accept(&selected, discovered(&alternate)).unwrap();
    assert_eq!(accepted.into_verifier().checkpoint(), &original);
    assert_eq!(std::fs::read(path.join("accepted.nrt")).unwrap(), retained);
}

#[test]
fn bootstrap_never_rewinds_or_replaces_a_same_height_decision() {
    let mut fixture = NativeFinalityFixture::new();
    let low = fixture.checkpoint();
    let selected = profile(&low);
    fixture.certify(fixture.block_with_submitted_work(fixture.next_header()));
    let high = fixture.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let store = BootstrapStore::open(&directory.path().join("bootstrap")).unwrap();
    store.accept(&selected, discovered(&low)).unwrap();
    store.accept(&selected, discovered(&high)).unwrap();
    assert!(store.accept(&selected, discovered(&low)).is_err());
    let mut fork = NativeFinalityFixture::new();
    let mut header = fork.next_header();
    header.creation_time_ms += 7;
    fork.certify(fork.block_with_submitted_work(header));
    assert_eq!(fork.network_id(), high.network_id());
    assert_ne!(fork.checkpoint().block_hash(), high.block_hash());
    assert!(
        store
            .accept(&selected, discovered(&fork.checkpoint()))
            .is_err()
    );
    assert_eq!(
        store
            .load(&selected)
            .unwrap()
            .unwrap()
            .into_verifier()
            .checkpoint(),
        &high
    );
}

#[test]
fn concurrent_acceptance_retains_the_highest_verified_checkpoint() {
    let mut fixture = NativeFinalityFixture::new();
    let low = fixture.checkpoint();
    let selected = profile(&low);
    fixture.certify(fixture.block_with_submitted_work(fixture.next_header()));
    let high = fixture.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let store = BootstrapStore::open(&directory.path().join("bootstrap")).unwrap();
    std::thread::scope(|scope| {
        let low_result = scope.spawn(|| store.accept(&selected, discovered(&low)));
        let high_result = scope.spawn(|| store.accept(&selected, discovered(&high)));
        let _ = low_result.join().unwrap();
        high_result.join().unwrap().unwrap();
    });
    assert_eq!(
        store
            .load(&selected)
            .unwrap()
            .unwrap()
            .into_verifier()
            .checkpoint(),
        &high
    );
}

#[test]
fn bootstrap_records_and_exact_lock_cannot_be_recreated_after_loss() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let directory = tempfile::tempdir().unwrap();
    for missing in ["bootstrap.lock", "accepted.nrt"] {
        let path = directory.path().join(missing.replace('.', "-"));
        let store = BootstrapStore::open(&path).unwrap();
        store.accept(&selected, discovered(&checkpoint)).unwrap();
        if missing == "bootstrap.lock" {
            // Windows retains exclusive deletion custody while the lock handle is open.
            drop(store);
            std::fs::remove_file(path.join(missing)).unwrap();
        } else {
            std::fs::remove_file(path.join(missing)).unwrap();
            assert!(store.load(&selected).is_err());
            assert!(store.accept(&selected, discovered(&checkpoint)).is_err());
            drop(store);
        }
        assert!(BootstrapStore::open(&path).is_err());
        assert!(!path.join(missing).exists());
    }
    #[cfg(unix)]
    {
        let path = directory.path().join("replaced-lock");
        let store = BootstrapStore::open(&path).unwrap();
        store.accept(&selected, discovered(&checkpoint)).unwrap();
        std::fs::rename(path.join("bootstrap.lock"), path.join("original.lock")).unwrap();
        drop(store.directory.create_lock("bootstrap.lock").unwrap());
        assert!(store.load(&selected).is_err());
        assert!(store.accept(&selected, discovered(&checkpoint)).is_err());
    }
}

#[test]
fn bootstrap_failed_publication_requires_reopening_original_custody() {
    let mut fixture = NativeFinalityFixture::new();
    let low = fixture.checkpoint();
    let selected = profile(&low);
    fixture.certify(fixture.block_with_submitted_work(fixture.next_header()));
    let high = fixture.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bootstrap");
    let store = BootstrapStore::open(&path).unwrap();
    store.accept(&selected, discovered(&low)).unwrap();
    std::fs::rename(path.join("accepted.nrt"), path.join("retained.nrt")).unwrap();
    std::fs::create_dir(path.join("accepted.nrt")).unwrap();
    let retained = RetainedBootstrap {
        profile: profile_image(&selected).unwrap(),
        metadata: native_metadata(&selected, &discovered(&high)).unwrap(),
        checkpoint: high.encode_canonical().unwrap(),
    };
    assert!(
        store
            .publish(&mut store.state.lock().unwrap(), retained)
            .is_err()
    );
    std::fs::remove_dir(path.join("accepted.nrt")).unwrap();
    std::fs::rename(path.join("retained.nrt"), path.join("accepted.nrt")).unwrap();
    assert!(store.load(&selected).is_err());
    assert!(store.accept(&selected, discovered(&high)).is_err());
    drop(store);
    let store = BootstrapStore::open(&path).unwrap();
    assert_eq!(
        store
            .load(&selected)
            .unwrap()
            .unwrap()
            .into_verifier()
            .checkpoint(),
        &low
    );
    store.accept(&selected, discovered(&high)).unwrap();
}

#[test]
fn bounded_records_and_native_checkpoints_reject_malformed_material() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let selected = profile(&checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bootstrap");
    let store = BootstrapStore::open(&path).unwrap();
    store.accept(&selected, discovered(&checkpoint)).unwrap();
    let retained = std::fs::read(path.join("accepted.nrt")).unwrap();
    assert!(decode::<BootstrapRecord>(&[], MAX_BOOTSTRAP_BYTES).is_err());
    assert!(decode::<BootstrapRecord>(&retained, retained.len() - 1).is_err());
    let mut trailing = retained.clone();
    trailing.push(0);
    assert!(decode::<BootstrapRecord>(&trailing, MAX_BOOTSTRAP_BYTES).is_err());
    let mut record = store.read_record().unwrap();
    record.accepted.as_mut().unwrap().checkpoint = b"invalid checkpoint".to_vec();
    store
        .directory
        .write_atomic(
            "accepted.nrt",
            &norito::encode_canonical(&record).unwrap(),
            PublishMode::Replace,
        )
        .unwrap();
    assert!(store.load(&selected).is_err());
    assert!(store.accept(&selected, discovered(&checkpoint)).is_err());
    let mut data = metadata(&checkpoint);
    data.torii_roots.push(data.torii_roots[0].clone());
    assert!(validate_metadata(&data).is_err());
    data = metadata(&checkpoint);
    data.network_id = NativeFinalityFixture::new_with_explicit_parameters().network_id();
    assert_ne!(data.network_id, checkpoint.network_id());
    assert!(verify_checkpoint(&data, &checkpoint.encode_canonical().unwrap()).is_err());
}

#[test]
fn parent_bootstrap_requires_a_certified_successor_and_global_scope() {
    let genesis = NativeFinalityFixture::start("genesis-only").checkpoint();
    assert!(verify_checkpoint(&metadata(&genesis), &genesis.encode_canonical().unwrap()).is_err());
    let global = NativeFinalityFixture::new();
    let mut private = NativeFinalityFixture::start_with_scope(
        "private-root",
        SumeragiRootScope::Dataspace {
            parent_network_id: global.network_id(),
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(7),
        },
    );
    private.certify(private.block_with_submitted_work(private.next_header()));
    let checkpoint = private.checkpoint();
    assert!(
        verify_checkpoint(
            &metadata(&checkpoint),
            &checkpoint.encode_canonical().unwrap()
        )
        .is_err()
    );
}

#[test]
fn network_metadata_admits_numeric_loopback_development_but_refuses_insecure_remote_roots() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    for root in ["http://127.0.0.1:18080/", "http://[::1]:18080/"] {
        let mut data = metadata(&checkpoint);
        data.torii_roots = vec![root.into()];
        data.peers.clear();
        validate_metadata(&data).unwrap();
    }
    for root in [
        "http://localhost/",
        "http://192.168.1.2/",
        "http://torii.example/",
    ] {
        let mut data = metadata(&checkpoint);
        data.torii_roots = vec![root.into()];
        data.peers.clear();
        assert!(validate_metadata(&data).is_err());
    }
}

#[test]
fn authenticated_bootstrap_only_authorizes_its_exact_http_endpoints() {
    let checkpoint = NativeFinalityFixture::new().checkpoint();
    let key = key(88);
    let bootstrap = authenticated(metadata(&checkpoint), &checkpoint);
    let client = |endpoint: &str| {
        let table = toml::toml! {
            chain = (checkpoint.chain_id())
            network_id = (checkpoint.network_id().to_string())
            torii_url = endpoint
            [account]
            chain_discriminant = 753
            public_key = (key.public_key().to_string())
            private_key = (iroha_crypto::ExposedPrivateKey(key.private_key().clone()).to_string())
        };
        iroha::client::Client::builder(
            iroha::config::Config::load_table("bootstrap-tests.toml", table).unwrap(),
        )
        .build()
        .unwrap()
    };
    let peer =
        iroha_model_base::peer::PeerId::new(checkpoint.tip().committee[0].public_key.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    bootstrap
        .http_source(
            vec![client("https://torii.example/")],
            vec![(peer.clone(), client("https://torii.example/"))],
            deadline,
        )
        .unwrap();
    for endpoint in [
        "https://torii.example/other/",
        "https://foreign.example/",
        "http://127.0.0.1:18080/",
    ] {
        assert!(
            bootstrap
                .http_source(
                    vec![client(endpoint)],
                    vec![(peer.clone(), client("https://torii.example/"))],
                    deadline
                )
                .is_err()
        );
        assert!(
            bootstrap
                .http_source(
                    vec![client("https://torii.example/")],
                    vec![(peer.clone(), client(endpoint))],
                    deadline
                )
                .is_err()
        );
    }
}

struct RuntimeSource {
    proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof,
}
impl crate::verify::finality::FinalitySource for RuntimeSource {
    type Error = std::io::Error;
    fn finality_proof(
        &self,
        height: std::num::NonZeroU64,
    ) -> std::io::Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        if height.get() == self.proof.height() {
            Ok(self.proof.clone())
        } else {
            Err(std::io::Error::other("fixture height unavailable"))
        }
    }
    fn latest_attestation(
        &self,
        _: &iroha_model_base::peer::PeerId,
        _: &[u8; 32],
    ) -> std::io::Result<crate::verify::finality::FinalityAttestation> {
        Err(std::io::Error::other("fixture peer offline"))
    }
    fn latest_finality_proof(
        &self,
        _: &iroha_model_base::peer::PeerId,
        _: &[u8; 32],
    ) -> std::io::Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        Ok(self.proof.clone())
    }
}

#[test]
fn runtime_checkpoint_persists_verified_progress_without_bootstrap_rewind_or_readiness_reuse() {
    let mut native = NativeFinalityFixture::new();
    let checkpoint = native.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let bootstrap = authenticated(metadata(&checkpoint), &checkpoint);
    let runtime_path = directory.path().join("runtime");
    let mut runtime = ParentFinalityStore::open(&runtime_path, &bootstrap).unwrap();
    assert!(ParentFinalityStore::open(&runtime_path, &bootstrap).is_err());
    let source = RuntimeSource {
        proof: native.certify(native.block_with_submitted_work(native.next_header())),
    };
    assert_eq!(
        runtime
            .catch_up(&source, std::num::NonZeroU64::new(3).unwrap())
            .unwrap(),
        3
    );
    let before = runtime.verifier().checkpoint().clone();
    let bytes = std::fs::read(runtime_path.join("verified.nrt")).unwrap();
    assert!(runtime.observe(&source, &[7; 32]).is_err());
    assert_eq!(runtime.verifier().checkpoint(), &before);
    assert_eq!(
        std::fs::read(runtime_path.join("verified.nrt")).unwrap(),
        bytes
    );
    assert!(
        runtime
            .catch_up(&source, std::num::NonZeroU64::new(4).unwrap())
            .is_err()
    );
    assert_eq!(runtime.verifier().checkpoint(), &before);
    drop(runtime);
    let runtime = ParentFinalityStore::open(&runtime_path, &bootstrap).unwrap();
    assert_eq!(
        runtime.verifier().checkpoint(),
        &before,
        "older bootstrap never rewinds runtime"
    );
    assert_eq!(runtime.verifier().verified_tip().unwrap().height(), 3);
    drop(runtime);
    // The same certified decision can also reopen from a matching native bootstrap.
    let next = authenticated(metadata(&native.checkpoint()), &native.checkpoint());
    assert_eq!(
        ParentFinalityStore::open(&runtime_path, &next)
            .unwrap()
            .verifier()
            .checkpoint()
            .height(),
        3
    );
}

#[test]
fn runtime_checkpoint_rejects_reset_and_incomplete_or_replaced_custody() {
    let native = NativeFinalityFixture::new();
    let checkpoint = native.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let bootstrap = authenticated(metadata(&checkpoint), &checkpoint);
    for missing in ["lock", "verified.nrt"] {
        let path = directory.path().join(missing.replace('.', "-"));
        drop(ParentFinalityStore::open(&path, &bootstrap).unwrap());
        std::fs::remove_file(path.join(missing)).unwrap();
        assert!(ParentFinalityStore::open(&path, &bootstrap).is_err());
        assert!(!path.join(missing).exists());
    }
    let path = directory.path().join("corrupt");
    drop(ParentFinalityStore::open(&path, &bootstrap).unwrap());
    std::fs::write(path.join("verified.nrt"), b"partial").unwrap();
    assert!(ParentFinalityStore::open(&path, &bootstrap).is_err());
    assert_eq!(
        std::fs::read(path.join("verified.nrt")).unwrap(),
        b"partial"
    );
    let path = directory.path().join("changed");
    let mut runtime = ParentFinalityStore::open(&path, &bootstrap).unwrap();
    #[cfg(unix)]
    {
        std::fs::rename(path.join("lock"), path.join("original-lock")).unwrap();
        let private = PrivateDirectory::open(&path).unwrap();
        private.create_lock("lock").unwrap();
        let before = runtime.verifier().checkpoint().clone();
        let source = RuntimeSource {
            proof: native.latest().clone(),
        };
        assert!(
            runtime
                .catch_up(&source, std::num::NonZeroU64::new(2).unwrap())
                .is_err()
        );
        assert!(runtime.observe(&source, &[7; 32]).is_err());
        assert_eq!(runtime.verifier().checkpoint(), &before);
    }
    #[cfg(windows)]
    {
        // The native Windows lock handle forbids replacement while owned.
        assert!(std::fs::rename(path.join("lock"), path.join("original-lock")).is_err());
        assert!(ParentFinalityStore::open(&path, &bootstrap).is_err());
    }
    drop(runtime);
    let path = directory.path().join("old-network");
    drop(ParentFinalityStore::open(&path, &bootstrap).unwrap());
    let mut reset = NativeFinalityFixture::start_with_mode(
        "new-runtime-generation",
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    );
    reset.certify(reset.block_with_submitted_work(reset.next_header()));
    let checkpoint = reset.checkpoint();
    let reset = authenticated(metadata(&checkpoint), &checkpoint);
    assert!(ParentFinalityStore::open(&path, &reset).is_err());
    ParentFinalityStore::open(&directory.path().join("new-network"), &reset).unwrap();
}

#[test]
fn uncertain_checkpoint_publication_requires_reopen_and_never_rewinds_on_retry() {
    let mut native = NativeFinalityFixture::new();
    let checkpoint = native.checkpoint();
    let directory = tempfile::tempdir().unwrap();
    let bootstrap = authenticated(metadata(&checkpoint), &checkpoint);
    let path = directory.path().join("runtime");
    let mut runtime = ParentFinalityStore::open(&path, &bootstrap).unwrap();
    let source = RuntimeSource {
        proof: native.certify(native.block_with_submitted_work(native.next_header())),
    };
    let before = runtime.verifier().checkpoint().clone();
    std::fs::rename(path.join("verified.nrt"), path.join("retained.nrt")).unwrap();
    std::fs::create_dir(path.join("verified.nrt")).unwrap();
    assert!(
        runtime
            .catch_up(&source, std::num::NonZeroU64::new(3).unwrap())
            .is_err()
    );
    assert_eq!(runtime.verifier().checkpoint(), &before);
    std::fs::remove_dir(path.join("verified.nrt")).unwrap();
    std::fs::rename(path.join("retained.nrt"), path.join("verified.nrt")).unwrap();
    assert!(
        runtime
            .catch_up(&source, std::num::NonZeroU64::new(2).unwrap())
            .is_err()
    );
    assert!(runtime.observe(&source, &[7; 32]).is_err());
    drop(runtime);
    let mut reopened = ParentFinalityStore::open(&path, &bootstrap).unwrap();
    assert_eq!(reopened.verifier().checkpoint(), &before);
    assert_eq!(
        reopened
            .catch_up(&source, std::num::NonZeroU64::new(3).unwrap())
            .unwrap(),
        3
    );
}

#[test]
fn certified_refresh_uses_native_proofs_without_claiming_peer_readiness() {
    let mut fixture = NativeFinalityFixture::new();
    let checkpoint = fixture.checkpoint();
    let bootstrap = authenticated(metadata(&checkpoint), &checkpoint);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("runtime");
    let mut runtime = ParentFinalityStore::open(&path, &bootstrap).unwrap();
    let current = fixture.certify(fixture.block_with_submitted_work(fixture.next_header()));
    let source = RuntimeSource {
        proof: current.clone(),
    };
    // This source deliberately has no attestations. Only its native certified cut is usable.
    assert!(runtime.observe(&source, &[9; 32]).is_err());
    assert_eq!(runtime.refresh_certified(&source).unwrap(), 3);
    let retained = std::fs::read(path.join("verified.nrt")).unwrap();
    let mut invalid = current;
    invalid.block_wire[0] ^= 1;
    let mut fork = NativeFinalityFixture::new();
    let mut header = fork.next_header();
    header.creation_time_ms += 7;
    let fork = fork.certify(fork.block_with_submitted_work(header));
    for proof in [invalid, fork, checkpoint.tip().clone()] {
        assert!(runtime.refresh_certified(&RuntimeSource { proof }).is_err());
        assert_eq!(runtime.verifier().checkpoint().height(), 3);
        assert_eq!(std::fs::read(path.join("verified.nrt")).unwrap(), retained);
    }
    drop(runtime);
    assert_eq!(
        ParentFinalityStore::open(&path, &bootstrap)
            .unwrap()
            .verifier()
            .checkpoint()
            .height(),
        3
    );
}
