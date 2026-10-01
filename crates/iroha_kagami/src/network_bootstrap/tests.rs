//! Real native signature/checkpoint regressions over explicitly synthetic test networks.

use super::*;
use iroha_crypto::{Algorithm, SignatureOf};
use iroha_data_model::{
    block::{BlockSignature, SignedBlock},
    sumeragi_finality::test_fixtures::NativeFinalityFixture,
    transaction::TransactionSignature,
};

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

fn definition(fixture: &NativeFinalityFixture, authority: &KeyPair) -> Definition {
    let checkpoint = fixture.checkpoint();
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    Definition {
        schema_version: 1,
        network_name: "fixture".into(),
        serial: 1,
        generation: 1,
        network_id: checkpoint.network_id(),
        chain_id: checkpoint.chain_id().into(),
        account_chain_discriminant: 753,
        native_world_schema: Hash::new(b"independently selected synthetic World schema"),
        issued_at_ms: now - 1000,
        expires_at_ms: now + 60_000,
        torii_roots: vec!["https://torii.example/".into()],
        peers: checkpoint
            .tip()
            .committee
            .iter()
            .map(|member| PeerDefinition {
                node_id: PeerId::new(member.public_key.clone()),
                torii_root: "https://torii.example/".into(),
            })
            .collect(),
        faucet: None,
        build_registry: None,
        checkpoint_hash: Hash::new(checkpoint.encode_canonical().unwrap()),
        checkpoint_height: checkpoint.height(),
        checkpoint_block_hash: checkpoint.block_hash().into(),
        genesis_public_key: key(41).public_key().clone(),
        release_public_key: authority.public_key().clone(),
        minimum_serial: 1,
        checkpoint_url: "https://releases.example/fixture/network-checkpoint.nrt".into(),
    }
}

fn public_validation(
    definition: &Definition,
    fixture: &NativeFinalityFixture,
) -> color_eyre::Result<()> {
    definition.validate_public(
        &fixture.checkpoint().encode_canonical()?,
        &fixture.genesis().encode_wire()?,
    )?;
    Ok(())
}

#[test]
fn native_signed_genesis_intents_do_not_claim_committed_execution() {
    let fixture = NativeFinalityFixture::new();
    let genesis = fixture.genesis();
    assert!(genesis.is_resultless_proposal());
    let genesis_account = AccountId::new(key(41).public_key().clone());
    iroha_core::block::check_genesis_block_intents(genesis, &genesis_account).unwrap();
    let (hash, metadata) = iroha_core::release_identity::genesis_identity(
        &genesis.encode_wire().unwrap(),
        key(41).public_key(),
    )
    .unwrap();
    assert_eq!(hash, Hash::from(genesis.hash()));
    assert_eq!(
        metadata.sumeragi_context.root_scope,
        SumeragiRootScope::Global
    );
    assert!(iroha_core::validate_genesis_block(genesis, &genesis_account).is_err());
    public_validation(&definition(&fixture, &key(81)), &fixture).unwrap();
}

#[test]
fn release_genesis_identity_rejects_forged_block_and_transaction_signatures() {
    let fixture = NativeFinalityFixture::new();
    let genesis_key = key(41);
    let unrelated = key(82);
    let mut forged_block = fixture.genesis().clone();
    forged_block
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(unrelated.private_key(), forged_block.hash()).unwrap(),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();
    assert!(
        iroha_core::release_identity::genesis_identity(
            &forged_block.encode_wire().unwrap(),
            genesis_key.public_key(),
        )
        .is_err()
    );
    let mut transactions = fixture
        .genesis()
        .external_transactions()
        .cloned()
        .collect::<Vec<_>>();
    let forged_signature =
        SignatureOf::try_new(unrelated.private_key(), transactions[0].payload()).unwrap();
    transactions[0].set_signature(TransactionSignature(forged_signature));
    let forged_intent =
        SignedBlock::try_genesis(transactions, genesis_key.private_key(), None, None).unwrap();
    assert!(
        iroha_core::release_identity::genesis_identity(
            &forged_intent.encode_wire().unwrap(),
            genesis_key.public_key(),
        )
        .is_err()
    );
}

#[test]
fn independent_public_inputs_reject_wrong_network_key_checkpoint_and_scoped_parent() {
    let fixture = NativeFinalityFixture::new();
    let expected = definition(&fixture, &key(81));
    public_validation(&expected, &fixture).unwrap();
    let mut wrong = expected.clone();
    let other_network = NativeFinalityFixture::start_with_mode(
        "other",
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos,
    )
    .network_id();
    assert_ne!(other_network, expected.network_id);
    wrong.network_id = other_network;
    assert!(public_validation(&wrong, &fixture).is_err());
    wrong = expected.clone();
    wrong.genesis_public_key = key(82).public_key().clone();
    assert!(public_validation(&wrong, &fixture).is_err());
    wrong = expected.clone();
    wrong.checkpoint_hash = Hash::new(b"substituted checkpoint");
    assert!(public_validation(&wrong, &fixture).is_err());
    wrong = expected.clone();
    wrong.chain_id = "other".into();
    assert!(public_validation(&wrong, &fixture).is_err());
    let mut scoped = NativeFinalityFixture::start_with_scope(
        "private",
        SumeragiRootScope::Dataspace {
            parent_network_id: fixture.network_id(),
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(u64::MAX),
        },
    );
    scoped.certify(scoped.block_with_submitted_work(scoped.next_header()));
    assert!(public_validation(&definition(&scoped, &key(81)), &scoped).is_err());
}

#[test]
fn closed_policy_rejects_zero_unbounded_and_inconsistent_allowances() {
    let fixture = NativeFinalityFixture::new();
    let mut policy = definition(&fixture, &key(81));
    policy.faucet = Some(FaucetDefinition {
        torii_root: policy.torii_roots[0].clone(),
        issuer: AccountId::new(key(83).public_key().clone()),
        asset_definition_id: AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::parse_fully_qualified("app.universal").unwrap(),
            "gas".parse().unwrap(),
        ),
        amount: 10_u32.into(),
        max_operation_fee: 2_u32.into(),
        max_namespace_rent: 3_u32.into(),
    });
    public_validation(&policy, &fixture).unwrap();
    for field in [0, 1, 2] {
        let mut wrong = policy.clone();
        let faucet = wrong.faucet.as_mut().unwrap();
        match field {
            0 => faucet.amount = Quantity::zero(),
            1 => faucet.max_operation_fee = Quantity::zero(),
            _ => faucet.max_namespace_rent = Quantity::zero(),
        }
        assert!(public_validation(&wrong, &fixture).is_err());
    }
    let mut wrong = policy.clone();
    wrong.faucet.as_mut().unwrap().max_operation_fee = 11_u32.into();
    assert!(public_validation(&wrong, &fixture).is_err());
    wrong = policy.clone();
    wrong.expires_at_ms = wrong.issued_at_ms + iroha_deploy::bootstrap::MAX_RELEASE_VALIDITY_MS + 1;
    assert!(public_validation(&wrong, &fixture).is_err());
    wrong = policy.clone();
    wrong.minimum_serial = 0;
    assert!(public_validation(&wrong, &fixture).is_err());
    let mut json = norito::json::to_value(&policy).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("unbounded_fees".into(), norito::json::Value::Bool(true));
    assert!(norito::json::value::from_value::<Definition>(json).is_err());
}

#[test]
fn publisher_is_fresh_atomic_and_never_publishes_private_key() {
    let fixture = NativeFinalityFixture::new();
    let authority = key(81);
    let directory = tempfile::tempdir().unwrap();
    let owner = OwnerDirectory::open_or_create(directory.path()).unwrap();
    let key_record = ExposedPrivateKey(authority.private_key().clone())
        .try_to_multihash_string()
        .unwrap();
    let policy = norito::json::to_vec(&definition(&fixture, &authority)).unwrap();
    let inputs = owner
        .publish_private_child(
            "inputs",
            &[
                ("definition.json", &policy),
                (
                    "checkpoint.nrt",
                    &fixture.checkpoint().encode_canonical().unwrap(),
                ),
                ("genesis.nrt", &fixture.genesis().encode_wire().unwrap()),
                ("release.key", key_record.as_bytes()),
            ],
        )
        .unwrap();
    let args = || Args {
        definition: inputs.path().join("definition.json"),
        checkpoint: inputs.path().join("checkpoint.nrt"),
        genesis: inputs.path().join("genesis.nrt"),
        release_key: inputs.path().join("release.key"),
        out_dir: directory.path().join("published"),
    };
    args().run(&mut BufWriter::new(Vec::new())).unwrap();
    let published = iroha_fs::PrivateDirectory::open(directory.path().join("published")).unwrap();
    assert_eq!(published.entries(10).unwrap().len(), 5);
    let profiles = InstalledNetworkProfiles::from_installation_bytes(
        &published
            .read(NETWORK_PROFILES_FILENAME, 128 * 1024)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        profiles.select("fixture").unwrap().release_public_key(),
        authority.public_key()
    );
    for file in published.entries(10).unwrap() {
        assert!(
            !published
                .read(file, MAX_FINALITY_CHECKPOINT_BYTES + 32 * 1024)
                .unwrap()
                .windows(key_record.len())
                .any(|window| window == key_record.as_bytes())
        );
    }
    assert!(args().run(&mut BufWriter::new(Vec::new())).is_err());
    let mut wrong = args();
    wrong.out_dir = directory.path().join("wrong-key");
    let bad = owner
        .publish_private_child(
            "other-key",
            &[(
                "key",
                ExposedPrivateKey(key(82).private_key().clone())
                    .try_to_multihash_string()
                    .unwrap()
                    .as_bytes(),
            )],
        )
        .unwrap();
    wrong.release_key = bad.path().join("key");
    assert!(wrong.run(&mut BufWriter::new(Vec::new())).is_err());
    assert!(!directory.path().join("wrong-key").exists());
}
