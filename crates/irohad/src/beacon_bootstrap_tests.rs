//! Per-seat beacon DKG command, phase-proof pipe and private custody tests.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_data_model::{NetworkId, block::BlockHeader};
use std::{
    fs::OpenOptions, os::fd::AsFd as _, os::unix::fs::PermissionsExt as _,
    os::unix::net::UnixStream,
};

fn test_network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"rotation-seat-command-network",
    )))
}

fn current_phase_fixture() -> (
    iroha_core::sumeragi::test_chain::CertifiedTestChain,
    NativeJournalCursor,
) {
    use iroha_core::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    let config = TestChainConfig::new(World::default(), 10_000);
    let chain_id = config.chain_id.clone();
    let chain = CertifiedTestChain::start(config).expect("current signed genesis");
    let verifier = NativeJournalCursor::new(
        chain_id,
        chain.network_id(),
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        test_finality_limits().checked().unwrap(),
    )
    .unwrap();
    assert!(
        verifier.tip().is_none(),
        "signed genesis alone is not a finalized execution"
    );
    (chain, verifier)
}

#[test]
fn current_phase_pipe_accepts_real_work_and_rejects_replay() {
    let (mut chain, mut verifier) = current_phase_fixture();
    chain.commit_at(20_000, Vec::new()); // Fixture adds a signed clock transaction.
    let limits = test_finality_limits().checked().unwrap();
    let journal = NativeFinalityJournal {
        blocks: (1..=2)
            .map(|height| {
                iroha_data_model::sumeragi::finality::NativeFinalityArtifact::from_block(
                    chain.committed(height).block(),
                    limits,
                )
                .unwrap()
            })
            .collect(),
    };
    let bytes = norito::encode_canonical(&journal).expect("canonical complete native journal");
    let (mut writer, reader) = UnixStream::pair().expect("local proof stream");
    for _ in 0..2 {
        writer
            .write_all(&u32::try_from(bytes.len()).unwrap().to_be_bytes())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    drop(writer);
    let mut last_height = 1;
    assert_eq!(
        read_rotation_phase_height(
            reader.as_fd(),
            Instant::now() + Duration::from_secs(1),
            &mut verifier,
            &mut last_height,
            4
        ),
        Ok(2)
    );
    assert_eq!(
        read_rotation_phase_height(
            reader.as_fd(),
            Instant::now() + Duration::from_secs(1),
            &mut verifier,
            &mut last_height,
            4
        ),
        Err(Error::Height)
    );
    assert_eq!(last_height, 2);
}

#[test]
fn rotation_rejects_unadmitted_evidence_before_opening_custody() {
    let mut limits = test_finality_limits();
    limits.finality_allocated_bytes = 0;
    let proof = RotationProofArgs {
        selection_evidence: PathBuf::from("/must-not-open/unadmitted-evidence"),
        network_id: test_network(),
        chain_id: ChainId::from("rotation-phase-test"),
        finality_limits: limits,
        target_epoch: 2,
        transition_id: Hash::new(b"unproved-transition"),
    };
    assert!(matches!(
        read_verified_rotation_selection(&proof),
        Err(Error::InvalidInput)
    ));
}

#[test]
fn rotation_phase_pipe_rejects_truncated_oversized_and_noncanonical_proofs() {
    for frame in [
        0_u32.to_be_bytes().to_vec(),
        u32::try_from(MAX_ROTATION_PHASE_PROOF_BYTES + 1)
            .expect("bounded proof size")
            .to_be_bytes()
            .to_vec(),
        vec![0, 0, 0, 1, 0],
        vec![0, 0, 0, 2, 0],
    ] {
        let (mut writer, reader) = UnixStream::pair().expect("local proof stream");
        writer.write_all(&frame).expect("write malformed frame");
        drop(writer);
        let mut verifier = NativeJournalCursor::new(
            ChainId::from("rotation-phase-test"),
            test_network(),
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
            test_finality_limits().checked().expect("limits"),
        )
        .expect("independent cursor");
        let mut last_height = 10;
        assert!(
            read_rotation_phase_height(
                reader.as_fd(),
                Instant::now() + Duration::from_secs(1),
                &mut verifier,
                &mut last_height,
                20,
            )
            .is_err()
        );
        assert_eq!(last_height, 10);
    }
}

fn test_finality_limits() -> FinalityLimitsArgs {
    FinalityLimitsArgs {
        finality_block_bytes: NATIVE_FINALITY_MAX_BLOCK_BYTES,
        finality_journal_bytes: NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        finality_block_count: NATIVE_FINALITY_MAX_BLOCK_COUNT,
        finality_allocated_bytes: 128 * 1024 * 1024,
    }
}

#[test]
fn finality_admission_rejects_zero_contradictory_and_over_protocol_limits() {
    assert!(test_finality_limits().checked().is_ok());
    let mut limits = test_finality_limits();
    limits.finality_allocated_bytes = 0;
    assert_eq!(limits.checked(), Err(Error::InvalidInput));
    let mut limits = test_finality_limits();
    limits.finality_journal_bytes = limits.finality_block_bytes - 1;
    assert_eq!(limits.checked(), Err(Error::InvalidInput));
    let mut limits = test_finality_limits();
    limits.finality_block_count = NATIVE_FINALITY_MAX_BLOCK_COUNT + 1;
    assert_eq!(limits.checked(), Err(Error::InvalidInput));
}

#[test]
fn rotation_phase_rejects_replay_gap_and_cutoff() {
    assert!(check_rotation_phase_height(10, 11, 14).is_ok());
    assert_eq!(check_rotation_phase_height(11, 11, 14), Err(Error::Height));
    assert_eq!(check_rotation_phase_height(11, 13, 14), Err(Error::Height));
    assert_eq!(check_rotation_phase_height(13, 14, 14), Err(Error::Height));
    assert_eq!(
        check_rotation_phase_height(u64::MAX, 0, u64::MAX),
        Err(Error::Height)
    );
}

#[test]
fn claimed_journal_count_never_substitutes_for_actual_native_source() {
    use iroha_data_model::sumeragi::finality::NativeFinalityArtifact;
    let mut cursor = NativeJournalCursor::new(
        ChainId::from("rotation-phase-test"),
        test_network(),
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        test_finality_limits().checked().expect("limits"),
    )
    .expect("cursor");
    let journal = NativeFinalityJournal {
        blocks: vec![
            NativeFinalityArtifact {
                block_wire: vec![0]
            };
            2
        ],
    };
    let mut height = 1;
    assert_eq!(
        advance_phase_journal(&mut cursor, &journal, &mut height, 10),
        Err(Error::Crypto)
    );
    assert_eq!(height, 1);
    assert!(cursor.tip().is_none());
    // An independently tracked phase clock cannot replace the retained native receipt.
    let mut height = 2;
    let journal = NativeFinalityJournal {
        blocks: vec![
            NativeFinalityArtifact {
                block_wire: vec![0]
            };
            3
        ],
    };
    assert_eq!(
        advance_phase_journal(&mut cursor, &journal, &mut height, 10),
        Err(Error::Height)
    );
    assert_eq!(height, 2);
    assert!(cursor.tip().is_none());
}

fn without_argument(args: &[String], name: &str) -> Vec<String> {
    let mut result = args.to_vec();
    let position = result
        .iter()
        .position(|arg| arg == name)
        .expect("argument exists");
    result.drain(position..position + 2);
    result
}

#[test]
fn bounded_phase_reader_consumes_exact_frame_without_advancing_next_frame() {
    let (mut writer, reader) = UnixStream::pair().expect("local proof stream");
    writer.write_all(b"firstsecond").expect("write both frames");
    let mut first = [0_u8; 5];
    read_exact_until(
        reader.as_fd(),
        Instant::now() + Duration::from_secs(1),
        &mut first,
    )
    .expect("first frame");
    assert_eq!(&first, b"first");
    let mut second = [0_u8; 6];
    read_exact_until(
        reader.as_fd(),
        Instant::now() + Duration::from_secs(1),
        &mut second,
    )
    .expect("second frame");
    assert_eq!(&second, b"second");
}

#[test]
fn rotation_seat_parser_requires_independent_pins_and_private_identity() {
    let transition = Hash::new(b"trusted-rotation-attempt").to_string();
    let base = vec![
        "beacon-bootstrap".to_owned(),
        "provision-rotation-seat".to_owned(),
        "--selection-evidence".to_owned(),
        "selection.norito".to_owned(),
        "--network-id".to_owned(),
        test_network().to_string(),
        "--chain-id".to_owned(),
        "rotation-test".to_owned(),
        "--target-epoch".to_owned(),
        "2".to_owned(),
        "--transition-id".to_owned(),
        transition,
    ];
    let mut seat = base.clone();
    seat.extend([
        "--signer-index".into(),
        "1".into(),
        "--key-fd".into(),
        "198".into(),
        "--public-fd".into(),
        "73".into(),
        "--finality-fd".into(),
        "74".into(),
        "--provider-handle".into(),
        "software://taira/global-beacon/rotation-seat-1".into(),
        "--provider-revision".into(),
        "1".into(),
        "--attempt-root".into(),
        "/tmp/owner-journal".into(),
    ]);
    assert!(Args::try_parse_from(&seat).is_ok());
    assert!(Args::try_parse_from(without_argument(&seat, "--chain-id")).is_err());
    let mut retired_pin = seat.clone();
    retired_pin.extend([
        "--trusted-context-id".into(),
        Hash::new(b"retired").to_string(),
    ]);
    assert!(Args::try_parse_from(retired_pin).is_err());
    let mut retired_height = seat.clone();
    retired_height.extend(["--anchor-height".into(), "10".into()]);
    assert!(Args::try_parse_from(retired_height).is_err());
    assert!(Args::try_parse_from(without_argument(&seat, "--key-fd")).is_err());
    let mut both_keys = seat.clone();
    both_keys.extend(["--config-fd".into(), "198".into()]);
    assert!(Args::try_parse_from(both_keys).is_err());
    let mut retired = seat.clone();
    retired[1] = "provision-rotation".into();
    assert!(Args::try_parse_from(retired).is_err());

    let mut assemble_dkg = base.clone();
    assemble_dkg[1] = "assemble-rotation-dkg".into();
    assemble_dkg.extend([
        "--public-session".into(),
        "session.norito".into(),
        "--phase-proof".into(),
        "phase-1.norito".into(),
        "--provider".into(),
        "seat-1-provider.json".into(),
        "--certificate-height".into(),
        "18".into(),
        "--output".into(),
        "rotation-bundle.json".into(),
    ]);
    assert!(Args::try_parse_from(&assemble_dkg).is_ok());
    assert!(Args::try_parse_from(without_argument(&assemble_dkg, "--provider")).is_err());

    let mut sign = base.clone();
    sign[1] = "sign-rotation".into();
    sign.extend([
        "--bundle".into(),
        "rotation-bundle.json".into(),
        "--signer-index".into(),
        "0".into(),
        "--key-fd".into(),
        "198".into(),
        "--output".into(),
        "signature.json".into(),
    ]);
    assert!(Args::try_parse_from(&sign).is_ok());
    assert!(Args::try_parse_from(without_argument(&sign, "--key-fd")).is_err());
}

#[test]
fn genesis_seat_parser_requires_signed_anchor_and_one_private_identity() {
    let base = vec![
        "beacon-bootstrap".to_owned(),
        "provision-genesis-seat".to_owned(),
        "--network-id".to_owned(),
        test_network().to_string(),
        "--chain-discriminant".to_owned(),
        "753".to_owned(),
        "--request".to_owned(),
        "request.json".to_owned(),
        "--genesis-manifest".to_owned(),
        "genesis-manifest.json".to_owned(),
        "--genesis-signed".to_owned(),
        "genesis.signed.nrt".to_owned(),
        "--genesis-public-key".to_owned(),
        "genesis.public-key".to_owned(),
        "--chain-id".to_owned(),
        "genesis-test".to_owned(),
    ];
    let mut seat = base.clone();
    seat.extend([
        "--signer-index".into(),
        "1".into(),
        "--key-fd".into(),
        "198".into(),
        "--public-fd".into(),
        "73".into(),
        "--finality-fd".into(),
        "74".into(),
        "--attempt-root".into(),
        "/tmp/owner-journal".into(),
    ]);
    assert!(Args::try_parse_from(&seat).is_ok());
    assert!(Args::try_parse_from(without_argument(&seat, "--genesis-signed")).is_err());
    assert!(Args::try_parse_from(without_argument(&seat, "--chain-id")).is_err());
    let mut retired = seat.clone();
    retired.extend(["--genesis-finality".into(), "fake-h1-qc.norito".into()]);
    assert!(Args::try_parse_from(retired).is_err());
    assert!(Args::try_parse_from(without_argument(&seat, "--key-fd")).is_err());
    let mut both = seat;
    both.extend(["--config-fd".into(), "198".into()]);
    assert!(Args::try_parse_from(both).is_err());
    let mut retired = base;
    retired[1] = "provision".into();
    assert!(Args::try_parse_from(retired).is_err());
}

#[test]
fn genesis_session_rejects_mutated_identity_under_same_attempt() {
    let network = test_network();
    let mut session = GlobalThresholdBeaconDkgSessionV1 {
        version: 1,
        network_id: network,
        session_id: genesis_seat::canonical_genesis_session_id(network),
        attempt_id: genesis_seat::canonical_genesis_attempt_id(network),
        authority_generation: 0,
        roster_hash: Hash::new(b"signed-genesis-roster").into(),
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        commitments_end_height: 2,
        deliveries_end_height: 3,
        acceptances_end_height: 4,
    };
    assert!(genesis_seat::genesis_session_identity_is_canonical(
        network, session
    ));
    session.session_id = Hash::new(b"rerolled-session").into();
    assert!(!genesis_seat::genesis_session_identity_is_canonical(
        network, session
    ));
    session.session_id = genesis_seat::canonical_genesis_session_id(network);
    session.attempt_id = Hash::new(b"rerolled-attempt").into();
    assert!(!genesis_seat::genesis_session_identity_is_canonical(
        network, session
    ));
}

#[test]
fn genesis_public_assembly_requires_exact_proof_and_provider_inputs() {
    let base = vec![
        "beacon-bootstrap".to_owned(),
        "assemble-genesis-dkg".to_owned(),
        "--network-id".to_owned(),
        test_network().to_string(),
        "--chain-discriminant".to_owned(),
        "753".to_owned(),
        "--request".to_owned(),
        "request.json".to_owned(),
        "--genesis-manifest".to_owned(),
        "genesis-manifest.json".to_owned(),
        "--genesis-signed".to_owned(),
        "genesis.signed.nrt".to_owned(),
        "--genesis-public-key".to_owned(),
        "genesis.public-key".to_owned(),
        "--chain-id".to_owned(),
        "genesis-test".to_owned(),
        "--phase-proof".to_owned(),
        "phase-2.norito".to_owned(),
        "--public-session".to_owned(),
        "public-session.norito".to_owned(),
        "--provider".to_owned(),
        "seat-1-provider.json".to_owned(),
        "--certificate-height".to_owned(),
        "7".to_owned(),
        "--output".to_owned(),
        "public-bundle.json".to_owned(),
    ];
    assert!(Args::try_parse_from(&base).is_ok());
    assert!(Args::try_parse_from(without_argument(&base, "--phase-proof")).is_err());
    assert!(Args::try_parse_from(without_argument(&base, "--provider")).is_err());
}

#[test]
fn one_shot_attempt_directory_cannot_reroll_after_restart() {
    let root = tempfile::Builder::new()
        .prefix(".beacon-seat-journal-")
        .tempdir_in(std::env::current_dir().expect("cwd"))
        .expect("private test root");
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).expect("private root");
    let path = fs::canonicalize(root.path()).expect("canonical root");
    let session = GlobalThresholdBeaconDkgSessionV1 {
        version: 1,
        network_id: test_network(),
        session_id: Hash::new(b"one-shot-session").into(),
        attempt_id: Hash::new(b"one-shot-attempt").into(),
        authority_generation: 1,
        roster_hash: Hash::new(b"one-shot-roster").into(),
        committee_size: 4,
        threshold: 2,
        start_height: 10,
        commitments_end_height: 11,
        deliveries_end_height: 12,
        acceptances_end_height: 13,
    };
    let first = rotation_seat::claim_attempt_directory(&path, &session, 1)
        .expect("claim one exact attempt and seat");
    assert_eq!(
        first.path.file_name().expect("attempt name"),
        std::ffi::OsStr::new(&rotation_seat::attempt_child_name(&session, 1))
    );
    first
        .write_new(std::ffi::OsStr::new("attempt-journal.json"), b"{}", true)
        .expect("durable attempt marker");
    drop(first);
    assert!(
        rotation_seat::claim_attempt_directory(&path, &session, 1).is_err(),
        "the same attempt must not generate fresh randomness after restart"
    );
    assert!(rotation_seat::claim_attempt_directory(&path, &session, 2).is_ok());
}

#[test]
fn each_seat_credential_binds_exact_public_session_and_private_share() {
    for seats in [4_u16, 7] {
        let network = test_network();
        let session_id = Hash::new_from_chunks(&[b"daemon-seat-credential", &seats.to_be_bytes()]);
        let (public, components) = iroha_core::beacon::complete_beacon_dkg_fixture_for_seat_v1(
            network,
            session_id.into(),
            seats,
            seats,
        );
        let (digest, credential) = rotation_seat::encode_local_seat_credential(
            public.clone(),
            seats,
            components,
            "software://taira/global-beacon/exact-seat",
            1,
        )
        .expect("exact private seat credential");
        assert!(!credential.is_empty());
        assert_eq!(
            digest,
            global_beacon_partial_signer_public_inventory_digest_v1(network, &[(public, seats)],)
                .expect("exact public policy digest")
        );
    }
}

#[test]
fn rotation_config_descriptor_uses_only_exact_native_consensus_identity() {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let key = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let network = test_network();
    let literal = Zeroizing::new(
        ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .expect("canonical private key"),
    );
    let mut config = Zeroizing::new(format!(
        "chain = \"fc56984b-2be7-431d-840e-21514d1883f0\"\nchain_discriminant = 369\npublic_key = \"{}\"\nprivate_key = \"{}\"\n[genesis]\nexpected_hash = \"{}\"\n[torii.faucet]\nprivate_key_file = \"/must-not-read/faucet\"\n[torii.account_onboarding]\nprivate_key_file = \"/must-not-read/onboarding\"\n",
        key.public_key(),
        literal.as_str(),
        network
    ));
    assert_eq!(
        lifecycle_key_from_config(config.as_bytes(), &network)
            .expect("exact key")
            .public_key(),
        key.public_key()
    );
    let foreign =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
    assert!(lifecycle_key_from_config(config.as_bytes(), &foreign).is_err());
    for (from, to) in [
        ("chain_discriminant = 369", "chain_discriminant = 1"),
        ("[genesis]", "extends = \"/must-not-read/base\"\n[genesis]"),
        (
            "[genesis]",
            "[genesis]\nexpected_hash_file = \"/must-not-read/identity\"",
        ),
    ] {
        let bad = Zeroizing::new(config.replace(from, to));
        assert!(lifecycle_key_from_config(bad.as_bytes(), &network).is_err());
    }
    let root = tempfile::Builder::new()
        .prefix(".beacon-config-test-")
        .tempdir_in(std::env::current_dir().expect("cwd"))
        .expect("private test root");
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).expect("private root");
    let path = fs::canonicalize(root.path()).expect("canonical root");
    for (name, bytes, success) in [
        ("valid", config.as_bytes(), true),
        ("malformed", b"not TOML".as_slice(), false),
    ] {
        let file_path = path.join(name);
        write_new(&file_path, bytes, true).expect("private config");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .expect("open config");
        assert_eq!(load_lifecycle_config(file, &network).is_ok(), success);
        assert_eq!(fs::metadata(file_path).expect("truncated config").len(), 0);
    }
    config.zeroize();
}
