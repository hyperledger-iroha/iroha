fn test_network_id(seed: u8) -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        Hash::prehashed([seed; Hash::LENGTH])
    ))
}

#[test]
fn recovered_lifecycle_kura_binding_releases_paths_only_to_exact_kura() {
    let kura = Kura::blank_kura_for_testing();
    let foreign_kura = Kura::blank_kura_for_testing();
    let local_signer = KeyPair::random();
    let foreign_signer = KeyPair::random();
    let binding =
        RecoveredLifecycleOwnerKuraBindingV1::for_test(kura.as_ref(), Some(&local_signer));

    let storage_root = kura.sumeragi_v2_storage_root();
    let expected_chunk_root = storage_root.join("chunks");
    let paths = binding
        .storage_paths_for_launch(kura.as_ref())
        .expect("the exact Kura projects its sealed launch paths");
    assert_eq!(
        paths.wal_path(),
        storage_root.join("wal").join(format!("{:020}.wal", 1_u64))
    );
    assert_eq!(paths.chunk_root(), expected_chunk_root);
    assert!(binding.matches_launch_identity(kura.as_ref(), &local_signer));
    assert!(!binding.matches_launch_identity(kura.as_ref(), &foreign_signer));
    assert!(!binding.matches_launch_identity(foreign_kura.as_ref(), &local_signer));
    assert!(
        binding
            .storage_paths_for_launch(foreign_kura.as_ref())
            .is_none(),
        "a foreign Kura must not project launch storage paths"
    );
    assert_eq!(paths.into_chunk_root(), expected_chunk_root);
}

#[derive(Debug)]
struct TestAggregator;

impl SignatureAggregator for TestAggregator {
    fn aggregate(&self, signatures: &[&[u8]]) -> Result<Vec<u8>, String> {
        let mut aggregate = Vec::new();
        for signature in signatures {
            aggregate.extend_from_slice(
                &u32::try_from(signature.len())
                    .map_err(|error| error.to_string())?
                    .to_le_bytes(),
            );
            aggregate.extend_from_slice(signature);
        }
        Ok(aggregate)
    }
}

fn peer(seed: u8) -> PeerId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("deterministic peer key");
    PeerId::new(key.public_key().clone())
}

fn context() -> wire::HeightContext {
    let mut roster = (1_u8..=4)
        .map(|seed| wire::ValidatorPower {
            validator: peer(seed),
            power: 1,
        })
        .collect::<Vec<_>>();
    roster.sort();
    wire::HeightContext {
        network_id: test_network_id(0x61),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 1,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 512 * 1024,
            max_chunk_count: 1024,
        },
        leader_seed: [0xA5; 32],
    }
}

fn verified_genesis(context: wire::HeightContext) -> VerifiedHeightContext {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS-normal key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert!(
        keys.iter()
            .zip(&context.roster)
            .all(|(key, entry)| key.public_key() == entry.validator.public_key())
    );
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("BLS proof of possession")
        })
        .collect();
    VerifiedHeightContext::genesis(context, proofs).expect("verified genesis context")
}
