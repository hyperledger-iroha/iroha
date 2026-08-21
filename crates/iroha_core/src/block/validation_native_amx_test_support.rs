#[test]
fn da_proof_policy_sidecar_hash_mismatch_reports_both_hashes() {
    let expected = Some(HashOf::<DaProofPolicyBundle>::from_untyped_unchecked(
        Hash::prehashed([0x11; Hash::LENGTH]),
    ));
    let actual = Some(HashOf::<DaProofPolicyBundle>::from_untyped_unchecked(
        Hash::prehashed([0x22; Hash::LENGTH]),
    ));
    let message =
        BlockValidationError::DaProofPolicySidecarHashMismatch { expected, actual }.to_string();
    assert!(message.contains(&format!("{expected:?}")));
    assert!(message.contains(&format!("{actual:?}")));
}
fn install_test_lane_manifests(state: &State) {
    let statuses = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| {
            let status = LaneManifestStatus {
                lane: lane.id,
                alias: lane.alias.clone(),
                dataspace: lane.dataspace_id,
                visibility: lane.visibility,
                storage: lane.storage,
                governance: None,
                manifest_path: None,
                governance_rules: None,
                privacy_commitments: Vec::new(),
            };
            (lane.id, status)
        })
        .collect();
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
}
fn test_confidential_features(state: &State, height: u64) -> Option<ConfidentialFeatureDigest> {
    let view = state.query_view();
    let digest =
        compute_confidential_feature_digest(view.world(), view.zk(), view.sccp_registry(), height);
    (!digest.is_empty()).then_some(digest)
}
fn test_world_with_assets<D, A, Ad, As, N>(
    domains: D,
    accounts: A,
    asset_definitions: Ad,
    assets: As,
    nfts: N,
) -> World
where
    D: IntoIterator<Item = Domain>,
    A: IntoIterator<Item = Account>,
    Ad: IntoIterator<Item = AssetDefinition>,
    As: IntoIterator<Item = Asset>,
    N: IntoIterator<Item = Nft>,
{
    let mut asset_definitions = asset_definitions.into_iter().collect::<Vec<_>>();
    let assets = assets.into_iter().collect::<Vec<_>>();
    let mut totals = BTreeMap::<AssetDefinitionId, Quantity>::new();
    for asset in &assets {
        let total = totals
            .entry(asset.id.definition().clone())
            .or_insert_with(Quantity::zero);
        *total = total
            .checked_add(&asset.value)
            .expect("test asset total must remain in the quantity domain");
    }
    for definition in &mut asset_definitions {
        definition.total_quantity = totals
            .remove(definition.id())
            .unwrap_or_else(Quantity::zero);
    }
    World::with_assets(domains, accounts, asset_definitions, assets, nfts)
}
fn accept_transaction_at_mock_time(
    transaction: SignedTransaction,
    network_id: &NetworkId,
    max_clock_drift: Duration,
    limits: TransactionParameters,
    crypto: &iroha_config::parameters::actual::Crypto,
    now: Duration,
) -> Result<AcceptedTransaction<'static>, crate::tx::AcceptTransactionFail> {
    let (_time_handle, time_source) = TimeSource::new_mock(now);
    AcceptedTransaction::accept_with_time_source(
        transaction,
        network_id,
        max_clock_drift,
        limits,
        crypto,
        &time_source,
    )
}
fn decode_stored_state_int(stored: &[u8]) -> i64 {
    let record: ivm::state_value::StateValueRecordV1 =
        norito::decode_from_bytes(stored).expect("decode canonical durable-state record");
    assert_eq!(
        norito::to_bytes(&record).expect("re-encode durable-state record"),
        stored,
        "durable-state records must use canonical Norito encoding"
    );
    let [ivm::state_value::StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
        panic!("stored Int state must contain one pointer atom");
    };
    let tlv = ivm::pointer_abi::validate_tlv_bytes(envelope)
        .expect("stored Int atom uses a canonical pointer-ABI envelope");
    assert_eq!(tlv.type_id, ivm::PointerType::Int);
    iroha_primitives::numeric_abi::IntValueV1::decode_frame(tlv.payload)
        .expect("decode persisted Int atom")
        .into_int()
        .try_to_i64()
        .expect("stored Int fits i64")
}
fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
    let (account_id, keypair) = gen_account_in("dummy");
    let mut builder = TransactionBuilder::new(
        deterministic_test_network_id(0x07),
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(0));
    let tx = builder
        .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
        .sign(keypair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}
fn signed_transaction_with_quarantine_marker(marker: Json) -> SignedTransaction {
    let (account_id, keypair) = gen_account_in("quarantine");
    let mut metadata = Metadata::default();
    metadata.insert(
        QUARANTINE_METADATA_KEY
            .parse()
            .expect("canonical quarantine metadata key"),
        marker,
    );
    TransactionBuilder::new(
        deterministic_test_network_id(0x08),
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "quarantine marker".to_owned())])
    .with_metadata(metadata)
    .sign(keypair.private_key())
}
#[test]
fn quarantine_classifier_accepts_only_exact_signed_boolean_true() {
    assert!(is_quarantine_transaction(
        &signed_transaction_with_quarantine_marker(Json::new(true))
    ));
    assert!(!is_quarantine_transaction(
        &signed_transaction_with_quarantine_marker(Json::new(false))
    ));
    assert!(!is_quarantine_transaction(
        &signed_transaction_with_quarantine_marker(Json::new("true"))
    ));
    assert!(!is_quarantine_transaction(
        &signed_transaction_with_quarantine_marker(Json::new(1_u64))
    ));
}
#[test]
fn legacy_taira_confidential_policy_hash_is_rejected() {
    let historical_policy_hash = [
        6, 56, 47, 173, 129, 176, 103, 189, 91, 113, 130, 211, 80, 254, 226, 208, 22, 148, 210,
        194, 47, 87, 152, 25, 162, 34, 156, 2, 45, 189, 111, 213,
    ];
    let expected = ConfidentialFeatureDigest::new(
        Some([0x3A; 32]),
        Some(1),
        Some(2),
        Some(1),
        Some([0x7F; 32]),
    );
    let actual = ConfidentialFeatureDigest::new(
        expected.vk_set_hash,
        expected.poseidon_params_id,
        expected.pedersen_params_id,
        expected.conf_rules_version,
        Some(historical_policy_hash),
    );
    assert!(matches!(
        ensure_confidential_features_match(Some(expected), Some(actual)),
        Err(BlockValidationError::ConfidentialFeaturesMismatch {
            expected: Some(reported_expected),
            actual: Some(reported_actual),
        }) if reported_expected == expected && reported_actual == actual
    ));
    assert!(ensure_confidential_features_match(Some(expected), Some(expected)).is_ok());
}
#[test]
fn confidential_feature_presence_is_exact() {
    let digest =
        ConfidentialFeatureDigest::new(Some([0x01; 32]), None, None, Some(1), Some([0x02; 32]));
    assert!(ensure_confidential_features_match(None, None).is_ok());
    assert!(matches!(
        ensure_confidential_features_match(Some(digest), None),
        Err(BlockValidationError::ConfidentialFeaturesMismatch {
            expected: Some(reported),
            actual: None,
        }) if reported == digest
    ));
    assert!(matches!(
        ensure_confidential_features_match(None, Some(digest)),
        Err(BlockValidationError::ConfidentialFeaturesMismatch {
            expected: None,
            actual: Some(reported),
        }) if reported == digest
    ));
}
fn native_amx_test_catalog(
    paynet: DataSpaceId,
    cbuae: DataSpaceId,
) -> iroha_data_model::nexus::DataSpaceCatalog {
    iroha_data_model::nexus::DataSpaceCatalog::new(vec![
        iroha_data_model::nexus::DataSpaceMetadata::default(),
        iroha_data_model::nexus::DataSpaceMetadata {
            id: paynet,
            alias: "paynet".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
        iroha_data_model::nexus::DataSpaceMetadata {
            id: cbuae,
            alias: "cbuae".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog")
}
fn native_amx_test_network_id() -> iroha_data_model::NetworkId {
    crate::sumeragi::synthetic_network_id("native-amx-test-genesis")
}
fn native_amx_test_world_with_keys() -> (World, Vec<KeyPair>) {
    let world = World::new();
    let keypairs = (0..4)
        .map(|_| crate::block::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let mut world_block = world.block();
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for keypair in &keypairs {
            peers.push(PeerId::new(keypair.public_key().clone()));
        }
        peers.apply();
    }
    for keypair in &keypairs {
        let pop = iroha_crypto::bls_normal_pop_prove(keypair.private_key())
            .expect("generate BLS proof-of-possession");
        let id = crate::state::derive_validator_key_id(keypair.public_key());
        let record = iroha_data_model::consensus::ConsensusKeyRecord {
            id: id.clone(),
            public_key: keypair.public_key().clone(),
            pop: Some(pop),
            activation_height: 0,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: iroha_data_model::consensus::ConsensusKeyStatus::Active,
        };
        world_block
            .consensus_keys
            .insert(id.clone(), record.clone());
        let pk = record.public_key.to_string();
        let mut by_pk = world_block
            .consensus_keys_by_pk
            .get(&pk)
            .cloned()
            .unwrap_or_default();
        if !by_pk.contains(&id) {
            by_pk.push(id);
        }
        world_block.consensus_keys_by_pk.insert(pk, by_pk);
    }
    world_block.commit();
    (world, keypairs)
}
struct NativeAmxTestAuthority {
    world: World,
    committee: Vec<PeerId>,
}
impl NativeAmxAuthorityContext for NativeAmxTestAuthority {
    fn route_active_at_height(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        height: u64,
    ) -> bool {
        height == 42 && matches!((lane_id.as_u32(), dataspace_id.as_u64()), (1, 7) | (2, 8))
    }
    fn lane_incarnation_at_height(&self, lane_id: LaneId, height: u64) -> Option<Hash> {
        (height == 42).then(|| Hash::new(lane_id.as_u32().to_be_bytes()))
    }
    fn resolve_lane_committee_at_height(
        &self,
        route: crate::state::LaneAuthorityRoute,
        height: u64,
    ) -> Result<Vec<PeerId>, crate::state::LaneAuthorityError> {
        if height == 42
            && self.route_active_at_height(route.lane_id(), route.dataspace_id(), height)
        {
            return Ok(self.committee.clone());
        }
        Err(crate::state::LaneAuthorityError::InactiveRoute {
            lane_id: route.lane_id(),
            dataspace_id: route.dataspace_id(),
            authority_height: height,
        })
    }
    fn consensus_pop_matches_authority(
        &self,
        _lane_id: LaneId,
        peer: &PeerId,
        height: u64,
        presented_pop: &[u8],
    ) -> bool {
        crate::state::live_consensus_key_pop_for_peer(&self.world.view(), peer, height)
            .is_none_or(|live_pop| live_pop == presented_pop)
    }
    fn native_amx_participant_predecessor_is_current(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        descriptor.previous_lane_block_height.checked_add(1) == Some(descriptor.lane_block_height)
            && (descriptor.previous_lane_block_height == 0)
                == descriptor.previous_lane_block_descriptor_hash.is_none()
    }
}
struct NativeAmxStalePredecessorTestAuthority<'a> {
    inner: &'a NativeAmxTestAuthority,
    stale_lane_id: LaneId,
}
impl NativeAmxAuthorityContext for NativeAmxStalePredecessorTestAuthority<'_> {
    fn route_active_at_height(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        height: u64,
    ) -> bool {
        self.inner
            .route_active_at_height(lane_id, dataspace_id, height)
    }
    fn lane_incarnation_at_height(&self, lane_id: LaneId, height: u64) -> Option<Hash> {
        self.inner.lane_incarnation_at_height(lane_id, height)
    }
    fn resolve_lane_committee_at_height(
        &self,
        route: crate::state::LaneAuthorityRoute,
        height: u64,
    ) -> Result<Vec<PeerId>, crate::state::LaneAuthorityError> {
        self.inner.resolve_lane_committee_at_height(route, height)
    }
    fn consensus_pop_matches_authority(
        &self,
        lane_id: LaneId,
        peer: &PeerId,
        height: u64,
        presented_pop: &[u8],
    ) -> bool {
        self.inner
            .consensus_pop_matches_authority(lane_id, peer, height, presented_pop)
    }
    fn native_amx_participant_predecessor_is_current(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        proposal.descriptor.lane_id != self.stale_lane_id
            && self
                .inner
                .native_amx_participant_predecessor_is_current(proposal)
    }
}
struct NativeAmxDriftedParticipantTestAuthority<'a> {
    inner: &'a NativeAmxTestAuthority,
    participant_lane_id: LaneId,
    participant_incarnation: Option<Hash>,
    participant_predecessor_is_current: bool,
}
impl NativeAmxAuthorityContext for NativeAmxDriftedParticipantTestAuthority<'_> {
    fn route_active_at_height(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        height: u64,
    ) -> bool {
        if lane_id == self.participant_lane_id {
            height == 42
                && dataspace_id == DataSpaceId::new(8)
                && self.participant_incarnation.is_some()
        } else {
            self.inner
                .route_active_at_height(lane_id, dataspace_id, height)
        }
    }
    fn lane_incarnation_at_height(&self, lane_id: LaneId, height: u64) -> Option<Hash> {
        if lane_id == self.participant_lane_id && height == 42 {
            self.participant_incarnation
        } else {
            self.inner.lane_incarnation_at_height(lane_id, height)
        }
    }
    fn resolve_lane_committee_at_height(
        &self,
        route: crate::state::LaneAuthorityRoute,
        height: u64,
    ) -> Result<Vec<PeerId>, crate::state::LaneAuthorityError> {
        self.inner.resolve_lane_committee_at_height(route, height)
    }
    fn consensus_pop_matches_authority(
        &self,
        lane_id: LaneId,
        peer: &PeerId,
        height: u64,
        presented_pop: &[u8],
    ) -> bool {
        self.inner
            .consensus_pop_matches_authority(lane_id, peer, height, presented_pop)
    }
    fn native_amx_participant_predecessor_is_current(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        if proposal.descriptor.lane_id == self.participant_lane_id {
            self.participant_predecessor_is_current
        } else {
            self.inner
                .native_amx_participant_predecessor_is_current(proposal)
        }
    }
}
fn native_amx_test_authority(world: World, keypairs: &[KeyPair]) -> NativeAmxTestAuthority {
    let mut committee = keypairs
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    committee.sort();
    committee.dedup();
    NativeAmxTestAuthority { world, committee }
}
fn historical_native_amx_test_active_lanes(
    coordinator_proposal: &LaneBlockProposalV1,
    receipt: &NativeAmxReceipt,
) -> Vec<MergeLaneBinding> {
    let mut routes = BTreeMap::new();
    let coordinator = &coordinator_proposal.descriptor;
    routes.insert(
        coordinator.lane_id,
        (
            coordinator.dataspace_id,
            coordinator.lane_incarnation,
            coordinator.proposal_height,
        ),
    );
    for leg in &receipt.legs {
        let descriptor = &leg.participant_proposal.descriptor;
        routes.insert(
            descriptor.lane_id,
            (
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
            ),
        );
    }
    routes
        .into_iter()
        .map(
            |(lane_id, (dataspace_id, incarnation, proposal_height))| MergeLaneBinding {
                lane_id,
                dataspace_id,
                lane_config_hash: Hash::new(
                    format!("historical-native-amx-lane-{}", lane_id.as_u32()).as_bytes(),
                ),
                incarnation,
                activation_height: proposal_height.saturating_sub(1),
            },
        )
        .collect()
}
fn checked_signature(private_key: &iroha_crypto::PrivateKey, payload: &[u8]) -> Signature {
    Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
}
fn expected_native_amx_test_context(block_height: u64) -> ExpectedNativeAmxV2Context {
    ExpectedNativeAmxV2Context {
        round: iroha_data_model::block::consensus_v2::ConsensusRound {
            context_id: iroha_data_model::block::consensus_v2::HeightContextId(
                HashOf::from_untyped_unchecked(Hash::new(b"native-amx-block-test-context")),
            ),
            height: block_height,
            view: 0,
        },
        epoch: 0,
    }
}
fn native_amx_test_validator_set(keypairs: &[KeyPair]) -> Vec<PeerId> {
    let mut validators = keypairs
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    validators.sort();
    validators
}
fn native_amx_test_coordinator_proposal(
    coordinator: crate::queue::RoutingDecision,
    tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
    authority_context_height: u64,
    keypairs: &[KeyPair],
) -> iroha_data_model::block::consensus::LaneBlockProposalV1 {
    native_amx_test_coordinator_proposal_at_view(
        coordinator,
        tx_entrypoint_hash,
        authority_context_height,
        2,
        keypairs,
    )
}
fn native_amx_test_coordinator_proposal_at_view(
    coordinator: crate::queue::RoutingDecision,
    tx_entrypoint_hash: HashOf<TransactionEntrypoint>,
    authority_context_height: u64,
    lane_block_view: u64,
    keypairs: &[KeyPair],
) -> iroha_data_model::block::consensus::LaneBlockProposalV1 {
    let validator_set = native_amx_test_validator_set(keypairs);
    let mut descriptor = iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
        lane_id: coordinator.lane_id,
        dataspace_id: coordinator.dataspace_id,
        lane_incarnation: Hash::new(coordinator.lane_id.as_u32().to_be_bytes()),
        proposal_height: authority_context_height,
        previous_lane_block_height: 6,
        previous_lane_block_descriptor_hash: Some(Hash::new(b"native-amx-test-previous")),
        lane_block_height: 7,
        lane_block_view,
        subject_hash: Hash::new(b"native-amx-test-subject"),
        payload_ownership_hash: Hash::new(b"native-amx-test-ownership"),
        rbc_instance_hash: Hash::new(b"native-amx-test-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(tx_entrypoint_hash)],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_count: u32::try_from(validator_set.len()).expect("fixture validator count"),
        min_quorum: u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
        )
        .expect("fixture quorum"),
        validator_set,
        qc_mode_tag: "native-amx:test-coordinator".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = iroha_data_model::block::consensus::LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}
fn native_amx_test_participant_proposal(
    body: &NativeAmxAttestationBodyV2,
    validator_set: Vec<PeerId>,
    coordinator_proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
) -> iroha_data_model::block::consensus::LaneBlockProposalV1 {
    if body.participant_lane_id == body.coordinator_lane_id
        && body.participant_dataspace_id == body.coordinator_dataspace_id
        && body.participant_lane_incarnation == body.coordinator_lane_incarnation
    {
        return coordinator_proposal.clone();
    }
    let mut descriptor = iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
        lane_id: body.participant_lane_id,
        dataspace_id: body.participant_dataspace_id,
        lane_incarnation: body.participant_lane_incarnation,
        proposal_height: body.authority_context_height,
        previous_lane_block_height: body.participant_previous_block_height,
        previous_lane_block_descriptor_hash: body.participant_previous_block_descriptor_hash,
        lane_block_height: body.participant_lane_block_height,
        lane_block_view: body.participant_lane_block_view,
        subject_hash: Hash::new(b"native-amx-test-participant-subject"),
        payload_ownership_hash: Hash::new(b"native-amx-test-participant-ownership"),
        rbc_instance_hash: Hash::new(b"native-amx-test-participant-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_count: body.participant_validator_count,
        min_quorum: body.participant_min_quorum,
        validator_set,
        qc_mode_tag: "native-amx:test-participant".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = iroha_data_model::block::consensus::LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}
