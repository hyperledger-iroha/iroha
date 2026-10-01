//! Genuine parent quorum plus deterministic wallet observations for resumable orchestration.
//! Synthetic execution fixtures test custody/control flow, not live fee or private execution.

use std::{
    cell::{Cell, RefCell},
    num::NonZeroU64,
};

use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    alias_setup::{
        AliasDataSpaceIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1, ResolvedDataSpaceV1,
    },
    block::consensus::SumeragiRootScope,
    isi::alias_setup::EnsureAlias,
    sns::{lease::SnsLeaseProofV1, record_storage_key},
    sumeragi::{SumeragiFootprint, SumeragiStatus},
    sumeragi_finality::{
        SumeragiFinalityAttestation, SumeragiFinalityAttestationBody, SumeragiFinalityProof,
        WorldStateElementKindV1, WorldStateSnapshotEntryV1, WorldStateSnapshotV1, genesis_epoch,
        test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
};
use iroha_model_base::{peer::PeerId, topology::DataSpaceId};
use norito::codec::{Decode as _, Encode as _};

use super::*;
use crate::bootstrap::{
    NetworkRelease, ReleaseCheckpointStore, ReleasePeer, ReleaseTrust, SignedNetworkCheckpoint,
};

struct Fixture {
    directory: tempfile::TempDir,
    parent: NativeFinalityFixture,
    child: Config,
    registration: PrivateDataspaceRegistration,
    bootstrap: AuthenticatedBootstrap,
    lease_proof: SnsLeaseProofV1,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut parent = NativeFinalityFixture::start("provisioning-parent");
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, "acme").unwrap();
        let owner = KeyPair::from_seed(vec![131; 32], Algorithm::Ed25519);
        let now = unix_ms().unwrap();
        let mut lease = NameRecordV1::new(
            selector.clone(),
            AccountId::new(owner.public_key().clone()),
            vec![],
            0,
            1,
            now + 600_000,
            now + 700_000,
            now + 800_000,
            Default::default(),
        );
        lease.ownership_generation = 7;
        let record = lease.encode();
        let lease_proof = SnsLeaseProofV1 {
            world: WorldStateSnapshotV1 {
                schema_hash: Hash::new(b"independently qualified fixture World schema"),
                entries: vec![WorldStateSnapshotEntryV1 {
                    field_id: "world.smart_contract_state".into(),
                    kind: WorldStateElementKindV1::Table,
                    key_hash: Some(
                        world_state_value_hash_v1(&record_storage_key(&selector)).unwrap(),
                    ),
                    value_hash: world_state_value_hash_v1(&record).unwrap(),
                }],
            },
            record,
        };
        let block = parent.block_with_submitted_work(parent.next_header());
        parent.certify_with_world_root(block, lease_proof.world.root().unwrap());
        let scope = SumeragiRootScope::Dataspace {
            parent_network_id: parent.network_id(),
            dataspace_id: DataSpaceId::from_hash(&selector.name_hash()),
        };
        let child_chain = NativeFinalityFixture::start_with_scope("provisioning-child", scope);
        let result = child_chain
            .verifier()
            .verify_retained_decision(child_chain.genesis_proof())
            .unwrap()
            .result()
            .0;
        let registration = PrivateDataspaceRegistration::new(
            scope,
            child_chain.chain_id().parse().unwrap(),
            child_chain.network_id(),
            result,
            genesis_epoch(child_chain.genesis()).unwrap(),
        )
        .unwrap();
        let mut child = Config::load_table(
            "fixture.toml",
            toml::toml! {
                chain = (child_chain.chain_id())
                network_id = (child_chain.network_id().to_string())
                torii_url = "http://127.0.0.1:8080/"
                [account]
                chain_discriminant = 753
                domain = "app.acme"
                public_key = (owner.public_key().to_string())
                private_key = (ExposedPrivateKey(owner.private_key().clone()).to_string())
            },
        )
        .unwrap();
        child.api_token = Some(iroha::secrecy::SecretString::new(
            "only-the-private-child".into(),
        ));
        let checkpoint = parent.checkpoint();
        let authority = KeyPair::from_seed(vec![132; 32], Algorithm::Ed25519);
        let release = NetworkRelease {
            network_name: "fixture".into(),
            serial: 1,
            generation: 1,
            network_id: parent.network_id(),
            chain_id: parent.chain_id().into(),
            issued_at_ms: now - 1000,
            expires_at_ms: now + 600_000,
            torii_roots: vec!["https://parent.example/".into()],
            account_chain_discriminant: 753,
            native_world_schema: lease_proof.world.schema_hash,
            peers: checkpoint
                .tip()
                .committee
                .iter()
                .map(|validator| ReleasePeer {
                    node_id: PeerId::new(validator.public_key.clone()),
                    torii_root: "https://parent.example/".into(),
                })
                .collect(),
            faucet: Some(ReleaseFaucet {
                torii_root: "https://parent.example/".into(),
                issuer: AccountId::new(authority.public_key().clone()),
                asset_definition_id: iroha_wallet::operations::XOR_ASSET_DEFINITION
                    .parse()
                    .unwrap(),
                amount: 1000_u64.into(),
                max_operation_fee: 2_u64.into(),
                max_namespace_rent: 100_u64.into(),
            }),
            build_registry: None,
            checkpoint_hash: Hash::new(checkpoint.encode_canonical().unwrap()),
            checkpoint_height: checkpoint.height(),
            checkpoint_block_hash: checkpoint.block_hash().into(),
        };
        let bytes = SignedNetworkCheckpoint::sign(release, &checkpoint, authority.private_key())
            .unwrap()
            .encode_canonical()
            .unwrap();
        let bootstrap = ReleaseCheckpointStore::open(&directory.path().join("release"))
            .unwrap()
            .authenticate(
                &ReleaseTrust::new("fixture".into(), authority.public_key().clone(), 1).unwrap(),
                &bytes,
                now,
            )
            .unwrap();
        Self {
            directory,
            parent,
            child,
            registration,
            bootstrap,
            lease_proof,
        }
    }

    fn open(&self) -> Result<RemoteProvisioning> {
        RemoteProvisioning::open_trusted(
            &self.directory.path().join("provisioning"),
            &self.bootstrap,
            self.child.clone(),
            "acme".into(),
            self.registration.clone(),
        )
    }
}

struct Source<'a> {
    chain: &'a NativeFinalityFixture,
    offline: bool,
    reads: Cell<usize>,
}
impl FinalitySource for Source<'_> {
    type Error = std::io::Error;
    fn finality_proof(
        &self,
        height: NonZeroU64,
    ) -> std::result::Result<SumeragiFinalityProof, Self::Error> {
        if height.get() == self.chain.checkpoint().height() {
            Ok(self.chain.latest().clone())
        } else {
            Err(std::io::Error::other("unserved fixture height"))
        }
    }
    fn latest_attestation(
        &self,
        peer: &PeerId,
        challenge: &[u8; 32],
    ) -> std::result::Result<SumeragiFinalityAttestation, Self::Error> {
        self.reads.set(self.reads.get() + 1);
        if self.offline {
            return Err(std::io::Error::other("offline"));
        }
        let key = (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .find(|key| key.public_key() == peer.public_key())
            .unwrap();
        let height = self.chain.checkpoint().height();
        let config_fingerprint = Hash::new(b"fixture config");
        let body = SumeragiFinalityAttestationBody {
            challenge: *challenge,
            network_id: self.chain.network_id(),
            node_id: peer.clone(),
            node_fingerprint: Hash::new(peer.encode()),
            build_fingerprint: Hash::new(b"fixture build"),
            config_fingerprint,
            genesis_block_hash: self.chain.genesis().hash(),
            genesis_finality_proof: self.chain.genesis_proof().clone(),
            status: SumeragiStatus {
                protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
                config_fingerprint,
                beacon_horizon: None,
                instance: self.chain.verifier().instance().0,
                height: height + 1,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: height,
                applied_height: height,
                awaiting: false,
                signer: Some(key.public_key().clone()),
                unanchored: false,
                abstaining: false,
                halted: None,
                footprint: SumeragiFootprint::default(),
            },
            finality_proof: self.chain.latest().clone(),
        };
        Ok(SumeragiFinalityAttestation {
            signature: SignatureOf::try_from_hash(key.private_key(), body.signing_hash()).unwrap(),
            body,
        })
    }
}

struct Operations {
    binding: Binding,
    calls: RefCell<Vec<&'static str>>,
    fund_status: Cell<OperationStatus>,
    reserve_status: Cell<OperationStatus>,
    lease_proof: SnsLeaseProofV1,
    forged_generation: Cell<bool>,
}
impl Operations {
    fn new(store: &RemoteProvisioning, fixture: &Fixture) -> Self {
        Self {
            binding: store.record.binding.clone(),
            calls: RefCell::new(vec![]),
            fund_status: Cell::new(OperationStatus::Applied),
            reserve_status: Cell::new(OperationStatus::Pending),
            lease_proof: fixture.lease_proof.clone(),
            forged_generation: Cell::new(false),
        }
    }
    fn request(&self) -> AliasSetupPlanRequestV1 {
        AliasSetupPlanRequestV1::new(vec![EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace: ResolvedDataSpaceV1::new(
                    "acme".parse().unwrap(),
                    self.binding.registration.scope.dataspace_id(),
                ),
                owner: self.binding.owner.clone(),
            }),
            AliasLeaseAcquisitionV1::new(1, Some(0)),
            AliasQuoteGuardV1 {
                expected_policy_version: 1,
                expected_payment_asset: self.binding.faucet.asset_definition_id.clone(),
                max_amount: 10_u64.into(),
                valid_until_ms: unix_ms().unwrap() + 120_000,
            },
        )])
    }
}
impl ProvisioningOperations for Operations {
    fn fund(
        &self,
        config: &Config,
        request: &FaucetRequest,
        _: &Path,
        deadline: Instant,
    ) -> Result<OperationStatus> {
        assert!(deadline > Instant::now());
        assert!(config.api_token.is_none());
        assert_eq!(request.amount, self.binding.faucet.amount);
        assert_eq!(request.fee_payment.charge_limits().len(), 1);
        assert_eq!(
            request.fee_payment.charge_limits()[0].max_amount(),
            &self.binding.faucet.max_operation_fee
        );
        self.calls.borrow_mut().push("fund");
        Ok(self.fund_status.get())
    }
    fn namespace_request(
        &self,
        _: &Config,
        _: &str,
        _: Instant,
    ) -> Result<AliasSetupPlanRequestV1> {
        self.calls.borrow_mut().push("quote");
        Ok(self.request())
    }
    fn reserve(
        &self,
        _: &Config,
        request: &AliasSetupPlanRequestV1,
        options: &BoundedTransactionOptions,
        journal: &Path,
    ) -> Result<OperationStatus> {
        self.calls.borrow_mut().push("reserve");
        assert_eq!(
            options.max_total_fees[&self.binding.faucet.asset_definition_id],
            self.binding.faucet.max_operation_fee
        );
        if !path_exists(journal)? {
            let _held = iroha_wallet::operation_journal::Journal::create_prepared(journal, request)
                .unwrap();
        } else {
            let held = iroha_wallet::operation_journal::Journal::open(journal).unwrap();
            assert_eq!(
                &held.read_operation::<AliasSetupPlanRequestV1>().unwrap(),
                request
            );
        }
        Ok(self.reserve_status.get())
    }
    fn lease(&self, config: &Config, read: &LeaseRead<'_>) -> Result<VerifiedSnsLeaseV1> {
        self.calls.borrow_mut().push("lease");
        assert!(read.deadline > Instant::now());
        let mut proof = self.lease_proof.clone();
        if self.forged_generation.get() {
            let mut record = NameRecordV1::decode(&mut proof.record.as_slice()).unwrap();
            record.ownership_generation = 8;
            proof.record = record.encode();
        }
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, read.alias).unwrap();
        proof
            .verify(
                config.network_id,
                &selector,
                read.owner,
                read.native_schema,
                read.block,
                unix_ms()?,
            )
            .map_err(|_| ProvisioningError::NamespaceObservation)
    }
}

#[test]
fn genuine_private_genesis_owner_is_imported_into_only_its_bound_parent_wallet() {
    let _guard = crate::managed::native_test_guard();
    let fixture = Fixture::new();
    let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, "acme").unwrap();
    let spec = crate::localnet::PrivateRootSpec {
        parent_network_id: fixture.parent.network_id(),
        dataspace_id: DataSpaceId::from_hash(&selector.name_hash()),
        dataspace_alias: "acme".into(),
    };
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_private_root(
        "private",
        &fixture.directory.path().join("private"),
        &ports,
        &spec,
    )
    .unwrap();
    let path = fixture.directory.path().join("provisioning");
    assert!(RemoteProvisioning::load_parent_config(&path, &fixture.bootstrap, &prepared).is_err());
    assert!(!path.exists());
    let service = RemoteProvisioning::open(&path, &fixture.bootstrap, &prepared).unwrap();
    let child = prepared.context.load_client_config().unwrap();
    let parent =
        RemoteProvisioning::load_parent_config(&path, &fixture.bootstrap, &prepared).unwrap();
    assert_eq!(parent.key_pair, child.key_pair);
    assert_eq!(parent.account, child.account);
    assert_eq!(parent.network_id, fixture.parent.network_id());
    assert_ne!(parent.network_id, child.network_id);
    assert!(child.api_token.is_some());
    assert!(parent.api_token.is_none());
    assert!(parent.basic_auth.is_none());
    assert_eq!(
        service.record.binding.registration,
        prepared.load_private_registration().unwrap()
    );
    let mut substituted = prepared.clone();
    substituted.context.dataspace_alias = "foreign".into();
    assert!(
        RemoteProvisioning::load_parent_config(&path, &fixture.bootstrap, &substituted).is_err()
    );
    drop(service);
    assert_eq!(
        RemoteProvisioning::open(&path, &fixture.bootstrap, &prepared)
            .unwrap()
            .parent
            .key_pair,
        parent.key_pair
    );
}

#[test]
fn exact_owner_import_and_exclusive_custody_survive_reopen_but_not_child_replacement() {
    let fixture = Fixture::new();
    let store = fixture.open().unwrap();
    assert_eq!(store.parent.account, fixture.child.account);
    assert!(store.parent.api_token.is_none());
    assert_eq!(store.parent.network_id, fixture.parent.network_id());
    assert_ne!(store.parent.network_id, store.child.network_id);
    assert_eq!(store.progress().stage, ProvisioningStage::Funding);
    let short_deadline = Instant::now() + Duration::from_secs(1);
    assert_eq!(
        store
            .turn_deadline(&fixture.bootstrap, short_deadline)
            .unwrap(),
        short_deadline
    );
    assert!(
        store
            .turn_deadline(
                &fixture.bootstrap,
                Instant::now() + Duration::from_secs(3600)
            )
            .unwrap()
            < Instant::now() + Duration::from_secs(601)
    );
    assert!(fixture.open().is_err());
    let read_parent = |child: &Config| {
        RemoteProvisioning::load_parent_config_trusted(
            &fixture.directory.path().join("provisioning"),
            &fixture.bootstrap,
            child,
            "acme",
            &fixture.registration,
        )
    };
    let parent = read_parent(&fixture.child).unwrap();
    assert_eq!(parent.account, store.parent.account);
    assert_eq!(parent.network_id, store.parent.network_id);
    assert_eq!(parent.key_pair, store.parent.key_pair);
    assert!(parent.api_token.is_none());
    let mut changed = fixture.child.clone();
    changed.account = fixture
        .bootstrap
        .release()
        .faucet
        .as_ref()
        .unwrap()
        .issuer
        .clone();
    assert!(read_parent(&changed).is_err());
    let bytes = store
        .directory
        .read("provisioning.nrt", MAX_RECORD_BYTES)
        .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("only-the-private-child"));
    assert!(decode_record(&[bytes.as_slice(), b"suffix"].concat()).is_err());
    drop(store);
    let store = fixture.open().unwrap();
    assert_eq!(store.progress().stage.as_str(), "funding");
    drop(store);
    let mut changed = fixture.child.clone();
    changed.account = fixture
        .bootstrap
        .release()
        .faucet
        .as_ref()
        .unwrap()
        .issuer
        .clone();
    assert!(
        RemoteProvisioning::open_trusted(
            &fixture.directory.path().join("provisioning"),
            &fixture.bootstrap,
            changed,
            "acme".into(),
            fixture.registration.clone()
        )
        .is_err()
    );
    let path = fixture
        .directory
        .path()
        .join("provisioning/provisioning.nrt");
    std::fs::remove_file(path).unwrap();
    assert!(fixture.open().is_err());
    assert!(read_parent(&fixture.child).is_err());
}

#[test]
fn fresh_quorum_precedes_funding_and_pending_namespace_resumes_exact_original_request() {
    let fixture = Fixture::new();
    let mut store = fixture.open().unwrap();
    let ops = Operations::new(&store, &fixture);
    let source = Source {
        chain: &fixture.parent,
        offline: true,
        reads: Cell::new(0),
    };
    let deadline = || Instant::now() + Duration::from_secs(30);
    assert!(
        store
            .provision_once(&fixture.bootstrap, Instant::now())
            .is_err()
    );
    assert!(
        store
            .provision_with(&fixture.bootstrap, deadline(), &source, &ops)
            .is_err()
    );
    assert!(source.reads.get() >= 3);
    assert!(ops.calls.borrow().is_empty());
    let source = Source {
        offline: false,
        ..source
    };
    assert!(
        store
            .provision_with(&fixture.bootstrap, Instant::now(), &source, &ops)
            .is_err()
    );
    ops.fund_status.set(OperationStatus::Pending);
    let progress = store
        .provision_with(&fixture.bootstrap, deadline(), &source, &ops)
        .unwrap();
    assert_eq!(progress.stage, ProvisioningStage::Funding);
    assert_eq!(progress.wallet_status, Some(OperationStatus::Pending));
    ops.fund_status.set(OperationStatus::Applied);
    ops.calls.borrow_mut().clear();
    let progress = store
        .provision_with(&fixture.bootstrap, deadline(), &source, &ops)
        .unwrap();
    assert_eq!(progress.stage, ProvisioningStage::Namespace);
    assert!(progress.confirmed.is_none());
    assert_eq!(*ops.calls.borrow(), vec!["fund", "quote", "reserve"]);
    let request = store.record.namespace.clone().unwrap();
    drop(store);
    let mut store = fixture.open().unwrap();
    ops.calls.borrow_mut().clear();
    ops.reserve_status.set(OperationStatus::Applied);
    ops.forged_generation.set(true);
    assert!(
        store
            .provision_with(&fixture.bootstrap, deadline(), &source, &ops)
            .is_err()
    );
    assert_eq!(store.record.lease_generation, None);
    assert_eq!(store.progress().stage, ProvisioningStage::Namespace);
    assert_eq!(*ops.calls.borrow(), vec!["reserve", "lease"]);
    ops.calls.borrow_mut().clear();
    ops.forged_generation.set(false);
    let progress = store
        .provision_with(&fixture.bootstrap, deadline(), &source, &ops)
        .unwrap();
    assert_eq!(*ops.calls.borrow(), vec!["reserve", "lease"]);
    assert_eq!(store.record.namespace.as_ref(), Some(&request));
    assert_eq!(progress.stage, ProvisioningStage::Registering);
    assert!(progress.confirmed.is_none());
    assert_eq!(store.record.lease_generation, Some(7));
    assert!(
        store
            .relay_once(&fixture.bootstrap, Instant::now())
            .is_err()
    );
    drop(store);
    let store = fixture.open().unwrap();
    assert_eq!(store.progress().stage.as_str(), "registering");
}

#[test]
fn namespace_allowance_and_authenticated_lease_bind_exact_identity_and_rent() {
    let fixture = Fixture::new();
    let store = fixture.open().unwrap();
    let ops = Operations::new(&store, &fixture);
    let binding = &store.record.binding;
    let original = ops.request();
    binding.validate_namespace(&original).unwrap();
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.intents[0].quote_guard.max_amount = 101_u64.into(),
            1 => changed.intents[0].acquisition.term_years = 2,
            2 => changed.intents.push(changed.intents[0].clone()),
            3 => changed.schema_version = 2,
            _ => {
                if let AliasIntentV1::Dataspace(value) = &mut changed.intents[0].intent {
                    value.owner = binding.faucet.issuer.clone();
                }
            }
        }
        assert!(binding.validate_namespace(&changed).is_err());
    }
    let tip = store.finality.verifier().verified_tip().unwrap();
    let read = LeaseRead {
        alias: "acme",
        owner: &binding.owner,
        native_schema: fixture.bootstrap.release().native_world_schema,
        block: &tip,
        deadline: Instant::now() + Duration::from_secs(1),
    };
    let lease = ops.lease(&store.parent, &read).unwrap();
    let now = unix_ms().unwrap();
    assert_eq!(binding.lease_generation(lease.record(), now).unwrap(), 7);
    for mutation in 0..4 {
        let mut changed = lease.record().clone();
        match mutation {
            0 => changed.ownership_generation = 0,
            1 => changed.owner = binding.faucet.issuer.clone(),
            2 => changed.expires_at_ms = now,
            _ => changed.name_hash = [0; 32],
        };
        assert!(binding.lease_generation(&changed, now).is_err());
    }
    let mut record = store.record.clone();
    record.namespace = Some(original);
    assert!(record.validate().is_err());
    let mut config = store.parent.clone();
    config.api_token = fixture.child.api_token.clone();
    assert!(binding.validate_config(&config).is_err());
    assert!(
        NativeOperations
            .fund(
                &store.parent,
                &binding.faucet_request(),
                &store.directory.path().join("expired"),
                Instant::now()
            )
            .is_err()
    );
    assert!(
        NativeOperations
            .namespace_request(&store.parent, "acme", Instant::now())
            .is_err()
    );
    assert!(
        NativeOperations
            .reserve(
                &store.parent,
                &ops.request(),
                &binding.options(Instant::now()),
                &store.directory.path().join("expired")
            )
            .is_err()
    );
    assert!(
        NativeOperations
            .lease(
                &store.parent,
                &LeaseRead {
                    deadline: Instant::now(),
                    ..read
                }
            )
            .is_err()
    );
}

#[test]
fn uncertain_publication_retains_previous_record_and_requires_exclusive_reopen() {
    let fixture = Fixture::new();
    let mut store = fixture.open().unwrap();
    let path = store.directory.path().join("provisioning.nrt");
    let saved = std::fs::read(&path).unwrap();
    std::fs::rename(&path, path.with_extension("held")).unwrap();
    std::fs::create_dir(&path).unwrap();
    let mut next = store.record.clone();
    next.faucet_observed = true;
    assert!(store.publish(next).is_err());
    assert_eq!(store.progress().stage, ProvisioningStage::Funding);
    assert!(store.revalidate().is_err());
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(path.with_extension("held"), &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(store.revalidate().is_err());
    drop(store);
    let store = fixture.open().unwrap();
    assert_eq!(store.progress().stage, ProvisioningStage::Funding);
    assert_eq!(ProvisioningStage::Namespace.as_str(), "namespace");
    assert_eq!(ProvisioningStage::Attached.as_str(), "attached");
}
