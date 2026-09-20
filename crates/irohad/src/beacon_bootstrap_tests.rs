//! Real fresh-RNG bootstrap, native credential and exact-roster certificate tests.
use super::*;
use iroha_core::beacon::{
    GlobalThresholdBeaconPulseAggregatorV1, GlobalThresholdBeaconSessionBindingV1,
    validate_global_threshold_beacon_session_v1,
};
use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_data_model::{
    NetworkId, block::BlockHeader, consensus::GlobalThresholdBeaconChainAnchorV1,
};
use std::{
    cell::Cell,
    fs::OpenOptions,
    os::fd::AsFd as _,
    os::unix::fs::{PermissionsExt as _, symlink},
};

fn request_and_genesis() -> (Request, GenesisProof, Vec<KeyPair>) {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    iroha_genesis::init_instruction_registry();
    let mut keys = (0..4)
        .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by_key(|k| PeerId::new(k.public_key().clone()));
    let roster = keys
        .iter()
        .map(|k| PeerId::new(k.public_key().clone()))
        .collect::<Vec<_>>();
    let topology = keys
        .iter()
        .map(|k| {
            iroha_genesis::GenesisTopologyEntry::new(
                PeerId::new(k.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(k.private_key()).expect("fresh BLS PoP"),
            )
        })
        .collect();
    let mut npos = iroha_data_model::parameter::system::SumeragiNposParameters::default();
    npos.max_validators = 4;
    npos.epoch_length_blocks = std::num::NonZeroU64::new(16).unwrap();
    npos.evidence_horizon_blocks = 16;
    npos.slashing_delay_blocks = 1;
    npos.validate().expect("valid signed short-epoch fixture parameters");
    let manifest = crate::complete_test_genesis_builder_for_topology(
        iroha_genesis::GenesisBuilder::new_without_executor(
            iroha_model_base::chain::ChainId::from(crate::taira_runtime_signer::TAIRA_CHAIN_ID_V1),
            ".",
        ),
        topology,
    )
    .append_parameter(iroha_data_model::parameter::Parameter::Custom(
        npos.into_custom_parameter(),
    ))
    .build_raw()
    .expect("complete native genesis")
    .with_consensus_mode(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos)
    .with_consensus_meta();
    let key = KeyPair::random();
    let block = manifest
        .clone()
        .build_and_sign(&key)
        .expect("sign real native genesis");
    let network_id = NetworkId::from_genesis_hash(block.0.hash());
    let genesis = GenesisProof {
        manifest,
        signed_wire: block.0.encode_wire().expect("canonical genesis wire"),
        public_key: key.public_key().clone(),
    };
    let session_id: [u8; 32] =
        Hash::new(KeyPair::random().public_key().to_string().as_bytes()).into();
    let request = Request {
        schema: "iroha.global-beacon.bootstrap.request.v1".into(),
        dkg_session: GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id,
            session_id,
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            sharing_end_height: 2,
            complaints_end_height: 3,
            responses_end_height: 4,
        },
        target_roster: roster.clone(),
        authorization_roster: roster,
        provider_handles: (1..=4)
            .map(|i| format!("software://taira/global-beacon/validator-{i}"))
            .collect(),
        provider_revision: 1,
    };
    (request, genesis, keys)
}
fn complete(request: Request, genesis: GenesisProof) -> (PublicBundle, Vec<Zeroizing<Vec<u8>>>) {
    let mut heights = [2, 3, 4].into_iter();
    let progress = Cell::new(0);
    let result = ceremony(
        request,
        genesis,
        1,
        Instant::now() + Duration::from_secs(60),
        |snapshot| {
            assert_eq!(snapshot.last_updated_height, 1);
            assert_eq!(snapshot.dealer_commitments.len(), 4);
            assert!(snapshot.complaints.is_empty());
            progress.set(progress.get() + 1);
            Ok(())
        },
        |_| {
            assert_eq!(progress.get(), 1);
            heights.next().ok_or(Error::Height)
        },
    )
    .expect("fresh ceremony");
    assert_eq!(progress.get(), 1);
    assert!(heights.next().is_none());
    result
}

#[test]
fn fresh_four_seat_bootstrap_roundtrips_native_custody_and_lifecycle_quorum() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (request, genesis, keys) = request_and_genesis();
    let (bundle, credentials) = complete(request, genesis);
    assert_eq!(bundle.finalized_observed_height, 4);
    assert_eq!(bundle.certificate.effective_height, 5);
    assert_eq!(bundle.record.activated_at_height, None);
    let record = &bundle.record.session;
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    };
    let session = validate_global_threshold_beacon_session_v1(record.clone(), &binding)
        .expect("native public validation");
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 5,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x75; 32])),
    };
    let mut aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(session.clone(), 6, anchor)
        .expect("exact test pulse");
    for (i, credential) in credentials.iter().enumerate() {
        let provider = &bundle.providers[i];
        let catalog = crate::IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "fresh-beacon-bootstrap",
            crate::IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            &provider.handle,
            provider.revision,
            provider.policy_digest,
        )
        .with_network_id_for_test(record.network_id);
        let configured = catalog.iter().next().expect("one bound runtime provider");
        let signer = crate::external_software_signer::decode_global_beacon_runtime_signer_v1(
            credential,
            &record.network_id,
            configured,
        )
        .expect("exact native credential import");
        let index = (i + 1) as u16;
        assert!(
            signer
                .attest_partial_signing_capability(&session, index)
                .expect("live imported seat")
                .matches(&session, index)
        );
        assert!(
            signer
                .attest_partial_signing_capability(&session, if index == 4 { 1 } else { index + 1 })
                .is_err()
        );
        let partial = signer
            .sign_partial(&session, aggregator.payload())
            .expect("fresh production signature");
        if i < 2 {
            aggregator
                .accept_partial(partial)
                .expect("proof-verified native share");
        }
        let mut wrong = Zeroizing::new(credential.to_vec());
        wrong[0] ^= 1;
        assert!(
            crate::external_software_signer::decode_global_beacon_runtime_signer_v1(
                &wrong,
                &record.network_id,
                configured
            )
            .is_err()
        );
    }
    aggregator
        .finalize()
        .expect("unique threshold group signature");
    let signatures = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, k)| sign_install(&bundle, i as u16, k).expect("exact-roster lifecycle signature"))
        .collect::<Vec<_>>();
    let certificate =
        assemble_install(&bundle, signatures.clone()).expect("native 3-of-4 lifecycle QC");
    assert_eq!(certificate.signatures.len(), 3);
    let instructions = vec![InstructionBox::from(
        ApplyThresholdKeyLifecycleCertificateV1 { certificate },
    )];
    let encoded = json_bytes(&instructions).expect("real native instruction JSON");
    let decoded: Vec<InstructionBox> =
        norito::json::from_slice(&encoded).expect("native instruction owner roundtrip");
    assert_eq!(decoded.len(), 1);
    assert!(assemble_install(&bundle, signatures[..2].to_vec()).is_err());
    let mut duplicate = signatures.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(assemble_install(&bundle, duplicate).is_err());
    let mut reordered = signatures;
    reordered.swap(0, 1);
    assert!(assemble_install(&bundle, reordered).is_err());
}

#[test]
fn bootstrap_rejects_foreign_genesis_rosters_transcripts_and_lifecycle_substitution() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (request, genesis, keys) = request_and_genesis();
    let core_roster =
        iroha_core::sumeragi::signed_genesis_voting_peers(&iroha_genesis::GenesisBlock(
            iroha_genesis::decode_signed_genesis(&genesis.signed_wire).expect("native wire"),
        ))
        .expect("native height-context roster");
    assert_eq!(request.authorization_roster, core_roster);
    // A valid signed manifest may list its topology in another insertion order;
    // only the native context's sorted voting identities own seat numbering.
    let mut raw = norito::json::to_value(&genesis.manifest).expect("public genesis JSON");
    let mut reversed = 0;
    for transaction in raw
        .get_mut("transactions")
        .and_then(norito::json::Value::as_array_mut)
        .expect("transactions")
    {
        if let Some(topology) = transaction
            .get_mut("topology")
            .and_then(norito::json::Value::as_array_mut)
        {
            if topology.len() == 4 {
                topology.reverse();
                reversed += 1;
            }
        }
    }
    assert_eq!(reversed, 1);
    let shuffled_manifest: iroha_genesis::RawGenesisTransaction =
        norito::json::from_value(raw).expect("shuffled public manifest");
    let fresh_genesis_key = KeyPair::random();
    let shuffled_block = shuffled_manifest
        .clone()
        .build_and_sign(&fresh_genesis_key)
        .expect("sign shuffled topology");
    let shuffled_proof = GenesisProof {
        manifest: shuffled_manifest,
        signed_wire: shuffled_block
            .0
            .encode_wire()
            .expect("native shuffled wire"),
        public_key: fresh_genesis_key.public_key().clone(),
    };
    let mut shuffled_request = request.clone();
    shuffled_request.dkg_session.network_id = NetworkId::from_genesis_hash(shuffled_block.0.hash());
    validate_genesis(&shuffled_request, &shuffled_proof)
        .expect("native roster ignores insertion order");
    let mut permissioned = genesis.clone();
    permissioned.manifest = permissioned.manifest.with_consensus_mode(
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned,
    );
    assert!(validate_genesis(&request, &permissioned).is_err());
    let mut late = request.clone();
    late.dkg_session.responses_end_height = 15;
    assert!(validate_genesis(&late, &genesis).is_err());
    let mut wrong = request.clone();
    wrong.authorization_roster.swap(0, 1);
    assert_eq!(validate_genesis(&wrong, &genesis), Err(Error::InvalidInput));
    wrong = request.clone();
    wrong.target_roster.swap(0, 1);
    assert!(validate_request(&wrong, 1).is_err());
    let mut foreign = genesis.clone();
    foreign.public_key = KeyPair::random().public_key().clone();
    assert!(validate_genesis(&request, &foreign).is_err());
    let mut corrupt = genesis.clone();
    corrupt.signed_wire[0] ^= 1;
    assert!(validate_genesis(&request, &corrupt).is_err());
    let (bundle, _) = complete(request, genesis);
    assert!(sign_install(&bundle, 0, &keys[1]).is_err());
    for kind in 0..6 {
        let mut changed = bundle.clone();
        match kind {
            0 => changed.certificate.effective_height += 1,
            1 => changed.certificate.expected_active_session_id = Some([0xAA; 32]),
            2 => changed.record.session.transcript_hash[0] ^= 1,
            3 => changed.providers[0].policy_digest[0] ^= 1,
            4 => changed.certificate.public_state.push(0),
            _ => changed.finalized_observed_height = 14,
        }
        assert!(validate_bundle(&changed).is_err());
    }
    let bytes = json_bytes(&bundle).expect("public bundle JSON");
    let decoded: PublicBundle = norito::json::from_slice(&bytes).expect("public bundle roundtrip");
    validate_bundle(&decoded).expect("all native authority checks after roundtrip");
}

#[test]
fn bootstrap_records_observed_height_jumps_and_rejects_pulse_collision() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (request, genesis, _) = request_and_genesis();
    // A queued canary can finish several real carriers after the faucet. The
    // ceremony must retain that observation, rather than inventing height four.
    for observed in [6, 14] {
        let mut heights = [2, 3, observed].into_iter();
        let outcome = ceremony(
            request.clone(),
            genesis.clone(),
            1,
            Instant::now() + Duration::from_secs(60),
            |_| Ok(()),
            |_| heights.next().ok_or(Error::Height),
        );
        assert!(heights.next().is_none());
        if observed == 6 {
            let (bundle, credentials) = outcome.expect("real observed jump fits signed epoch");
            assert_eq!(bundle.finalized_observed_height, 6);
            assert_eq!(bundle.record.session.adaptive_dkg.finalized_at_height, 6);
            assert_eq!(bundle.certificate.effective_height, 7);
            assert_eq!(credentials.len(), 4);
            validate_bundle(&bundle).expect("unmodified native bundle remains valid");
        } else {
            // This signed genesis requires a pulse at fifteen, before an
            // installation at fifteen could activate its session at sixteen.
            assert!(matches!(outcome, Err(Error::InvalidInput)));
        }
    }
}

#[test]
fn bootstrap_phase_eof_and_deadline_abort_without_fabricated_height() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (request, genesis, _) = request_and_genesis();
    assert!(validate_request(&request, 0).is_err());
    assert!(validate_request(&request, 2).is_err());
    let outcome = ceremony(
        request.clone(),
        genesis.clone(),
        1,
        Instant::now(),
        |_| panic!("expired before sharing"),
        |_| panic!("expired before height read"),
    );
    assert!(matches!(outcome, Err(Error::Deadline)));
    let mut count = 0;
    let outcome = ceremony(
        request.clone(),
        genesis.clone(),
        1,
        Instant::now() + Duration::from_secs(60),
        |_| {
            count += 1;
            Ok(())
        },
        |_| Err(Error::Height),
    );
    assert!(matches!(outcome, Err(Error::Height)));
    assert_eq!(count, 1);
    let outcome = ceremony(
        request,
        genesis,
        1,
        Instant::now() + Duration::from_secs(60),
        |_| Ok(()),
        |_| Ok(1),
    );
    assert!(matches!(outcome, Err(Error::Height)));
    let (read, write) = rustix::pipe::pipe().expect("public height pipe");
    rustix::io::write(&write, b"4\n").expect("observed public height");
    assert_eq!(
        read_observed_height(read.as_fd(), Instant::now() + Duration::from_secs(1)),
        Ok(4)
    );
    rustix::io::write(&write, b"04\n").expect("malformed public height");
    assert_eq!(
        read_observed_height(read.as_fd(), Instant::now() + Duration::from_secs(1)),
        Err(Error::Height)
    );
    drop(write);
    assert_eq!(
        read_observed_height(read.as_fd(), Instant::now() + Duration::from_secs(1)),
        Err(Error::Height)
    );
}

#[test]
fn bootstrap_output_custody_is_exclusive_and_lifecycle_key_is_consumed() {
    let temp = tempfile::Builder::new()
        .prefix(".beacon-bootstrap-test-")
        .tempdir_in(std::env::current_dir().expect("checkout"))
        .expect("owned fixture parent");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("owner mode");
    let root = fs::canonicalize(temp.path()).expect("canonical fixture root");
    let output = root.join("output");
    create_private_directory(&output).expect("new owner-only directory");
    assert!(create_private_directory(&output).is_err());
    let secret = output.join("credential");
    write_new(&secret, b"private runtime test record", true).expect("new private file");
    assert_eq!(
        fs::metadata(&secret).expect("metadata").mode() & 0o777,
        0o600
    );
    assert!(write_new(&secret, b"replacement", true).is_err());
    let link = root.join("link");
    symlink(&secret, &link).expect("adversarial symlink");
    assert!(write_new(&link, b"replacement", true).is_err());
    assert!(read_public_bytes(&link).is_err());
    let public = output.join("public.json");
    write_new(&public, b"{}", false).expect("new public result");
    assert_eq!(
        read_public_bytes(&public).expect("stable bounded public read"),
        b"{}"
    );
    // The held output directory cannot be redirected by replacing its pathname.
    let bound = Directory::open(&output).expect("bound output directory");
    let moved = root.join("old-output");
    fs::rename(&output, &moved).expect("adversarial replacement");
    create_private_directory(&output).expect("new unrelated output inode");
    assert!(
        bound
            .write_new(std::ffi::OsStr::new("not-written"), b"secret", true)
            .is_err()
    );
    assert!(!output.join("not-written").exists());
    assert!(!moved.join("not-written").exists());
    let hard = root.join("hard");
    fs::hard_link(moved.join("public.json"), &hard).expect("adversarial extra link");
    assert!(read_public_bytes(&hard).is_err());
    let unsafe_parent = root.join("unsafe");
    create_private_directory(&unsafe_parent).expect("owned initial directory");
    fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o777)).expect("unsafe mode");
    assert!(write_new(&unsafe_parent.join("not-written"), b"secret", true).is_err());
    let key = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let mut literal = Zeroizing::new(
        ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .expect("canonical BLS private literal"),
    );
    literal.push('\n');
    assert_eq!(literal.len(), 71);
    let key_path = output.join("consumed-key");
    write_new(&key_path, literal.as_bytes(), true).expect("supervisor fixture key copy");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&key_path)
        .expect("owned inherited-file analogue");
    let loaded = load_lifecycle_key(file).expect("same native descriptor loader");
    assert_eq!(loaded.public_key(), key.public_key());
    assert_eq!(fs::metadata(&key_path).expect("consumed metadata").len(), 0);
    assert!(
        Args::try_parse_from([
            "beacon-bootstrap",
            "sign-install",
            "--key-fd",
            "198",
            "--bundle",
            "/public",
            "--signer-index",
            "0",
            "--output",
            "/signed",
            "--private-key",
            "retired"
        ])
        .is_err()
    );
    assert!(
        sign_command(
            Path::new("/not-read"),
            0,
            Some(199),
            None,
            Path::new("/not-written")
        )
        .is_err()
    );
    for fd in [0, 198, 199, 200] {
        assert!(
            provision_command(
                Path::new("/not-read"),
                Path::new("/not-read"),
                Path::new("/not-read"),
                Path::new("/not-read"),
                1,
                fd,
                Path::new("/not-written"),
                1000
            )
            .is_err()
        );
    }
}

#[test]
fn bootstrap_config_descriptor_uses_only_exact_native_consensus_identity() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let (request, _, keys) = request_and_genesis();
    let key = &keys[0];
    let literal = Zeroizing::new(
        ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .unwrap(),
    );
    let mut config = Zeroizing::new(format!(
        "chain = \"fc56984b-2be7-431d-840e-21514d1883f0\"\nchain_discriminant = 369\npublic_key = \"{}\"\nprivate_key = \"{}\"\n[genesis]\nexpected_hash = \"{}\"\n[torii.faucet]\nprivate_key_file = \"/must-not-read/faucet\"\n[torii.account_onboarding]\nprivate_key_file = \"/must-not-read/onboarding\"\n[streaming.codec]\nrans_tables_path = \"/must-not-read/tables\"\n[nexus.registry]\nmanifest_directory = \"/must-not-read/registry\"\n",
        key.public_key(),
        literal.as_str(),
        request.dkg_session.network_id
    ));
    let network = request.dkg_session.network_id;
    assert_eq!(
        lifecycle_key_from_config(config.as_bytes(), &network)
            .unwrap()
            .public_key(),
        key.public_key()
    );
    let foreign = iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"foreign"),
    ));
    assert!(lifecycle_key_from_config(config.as_bytes(), &foreign).is_err());
    for (from, to) in [
        ("chain_discriminant = 369", "chain_discriminant = 1"),
        ("chain_discriminant = 369\n", ""),
        (
            "[genesis]",
            "private_key_file = \"/must-not-read/consensus\"\n[genesis]",
        ),
        ("[genesis]", "extends = \"/must-not-read/base\"\n[genesis]"),
        (
            "[genesis]",
            "[genesis]\nexpected_hash_file = \"/must-not-read/identity\"",
        ),
    ] {
        let bad = Zeroizing::new(config.replace(from, to));
        assert!(lifecycle_key_from_config(bad.as_bytes(), &network).is_err());
    }
    let other = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let bad = Zeroizing::new(config.replace(
        &key.public_key().to_string(),
        &other.public_key().to_string(),
    ));
    assert!(lifecycle_key_from_config(bad.as_bytes(), &network).is_err());
    let temp = tempfile::Builder::new()
        .prefix(".beacon-config-test-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    for (name, bytes, success) in [
        ("valid", config.as_bytes(), true),
        ("malformed", b"not TOML".as_slice(), false),
    ] {
        let path = root.join(name);
        write_new(&path, bytes, true).unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert_eq!(load_lifecycle_config(file, &network).is_ok(), success);
        assert_eq!(fs::metadata(path).unwrap().len(), 0);
    }
    config.zeroize();
    assert!(
        Args::try_parse_from([
            "beacon-bootstrap",
            "sign-install",
            "--config-fd",
            "198",
            "--bundle",
            "/public",
            "--signer-index",
            "0",
            "--output",
            "/signed"
        ])
        .is_ok()
    );
    assert!(
        Args::try_parse_from([
            "beacon-bootstrap",
            "sign-install",
            "--config-fd",
            "198",
            "--key-fd",
            "198",
            "--bundle",
            "/public",
            "--signer-index",
            "0",
            "--output",
            "/signed"
        ])
        .is_err()
    );
    assert!(
        sign_command(
            Path::new("/not-read"),
            0,
            None,
            Some(200),
            Path::new("/not-written")
        )
        .is_err()
    );
}
