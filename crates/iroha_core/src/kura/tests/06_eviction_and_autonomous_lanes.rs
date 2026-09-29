// Generic Kura fixtures retain complete original native execution and certificates.
type DefaultKuraFixture = (TempDir, KuraConfig, Arc<Kura>);
type ConfiguredKuraFixture = (TempDir, KuraConfig, RuntimeLaneConfig, Arc<Kura>);

struct NativeBlocks {
    chain: crate::sumeragi::test_chain::CertifiedTestChain,
    next_height: u64,
}
impl NativeBlocks {
    fn new() -> Self {
        use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
        Self {
            chain: CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
                .expect("original native fixture genesis"),
            next_height: 1,
        }
    }
    fn next(&mut self) -> Arc<SignedBlock> {
        while self.chain.height() < self.next_height {
            self.chain.commit(Vec::new());
        }
        let original = Arc::clone(self.chain.committed(self.next_height).block());
        self.next_height += 1;
        original
    }
    fn get(&self, index: usize) -> Option<Arc<SignedBlock>> {
        let height = u64::try_from(index).ok()?.checked_add(1)?;
        (height < self.next_height).then(|| Arc::clone(self.chain.committed(height).block()))
    }
}

#[test]
fn native_storage_generator_retains_original_execution_and_never_resigns_outputs() {
    let mut blocks = NativeBlocks::new();
    let originals = (0..3).map(|_| blocks.next()).collect::<Vec<_>>();
    for (index, original) in originals.iter().enumerate() {
        assert!(Arc::ptr_eq(original, &blocks.get(index).unwrap()));
        let receipt = blocks.chain.committed(index as u64 + 1);
        assert!(Arc::ptr_eq(original, receipt.block()));
        assert_eq!(
            original.commit_certificate(),
            receipt.block().commit_certificate()
        );
        assert_eq!(
            original.encode_wire().unwrap(),
            receipt.block().encode_wire().unwrap()
        );
    }
    assert!(blocks.get(3).is_none());
    let view = blocks.chain.state().view();
    let verifier = crate::sumeragi::certified_chain::CertifiedChain::new(&view).unwrap();
    for height in 1..=3 {
        verifier.authenticated_execution(height).unwrap();
    }
}

#[test]
fn deep_history_get_block_uses_cached_bytes() {
    const BLOCK_COUNT: usize = 192;
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, nonzero!(16_usize));
    let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
    drop(kura);
    let mut store = new_block_store(&temp_dir);
    store.create_files_if_they_do_not_exist().unwrap();
    let mut blocks = NativeBlocks::new();
    let mut expected_hashes = Vec::with_capacity(BLOCK_COUNT);
    for _ in 0..BLOCK_COUNT {
        let block = blocks.next();
        expected_hashes.push(block.hash());
        store.append_block_to_chain(block.as_ref()).unwrap();
    }
    drop(store);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let heights: Vec<_> = (1..=BLOCK_COUNT)
        .map(|height| NonZeroUsize::new(height).expect("nonzero height"))
        .collect();
    for _ in 0..3 {
        for (idx, height) in heights.iter().enumerate() {
            let block = kura
                .get_block(*height)
                .unwrap_or_else(|| panic!("block missing at height {height}"));
            assert_eq!(block.hash(), expected_hashes[idx]);
        }
    }
    let store_guard = kura.block_store.lock();
    let mirror = store_guard
        .data_mmap
        .as_ref()
        .expect("expected data mirror to be primed");
    assert_eq!(
        mirror.kind(),
        MemoryMirrorKind::MemoryMapped,
        "expected data mirror to use a memory-mapped backend"
    );
    let mapped_len = mirror.len();
    assert_eq!(
        u64::try_from(mapped_len).expect("mirror length fits in u64"),
        store_guard.data_mmap_len,
        "data mirror length should match recorded length"
    );
}

#[test]
fn debug_output_new_blocks_writes_jsonl() {
    let temp_dir = TempDir::new().expect("temp dir");
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.debug_output_new_blocks = true;
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let _handle = {
        let _rt_guard = rt.enter();
        Kura::start(kura.clone(), ShutdownSignal::new())
    };
    let block = NativeBlocks::new().next();
    kura.store_block(Arc::clone(&block)).expect("store block");
    wait_for_block_hash(&kura, 1, block.hash());
    let dump_path = Kura::canonical_storage_path(temp_dir.path()).join("blocks.jsonl");
    let contents = fs::read_to_string(&dump_path).expect("read debug block dump");
    let mut lines = contents.lines();
    let first = lines.next().expect("first JSON line");
    assert!(lines.next().is_none(), "expected one JSON line");
    let _: norito::json::Value =
        norito::json::from_slice(first.as_bytes()).expect("valid JSON line");
}

fn store_dummy_blocks(kura: &Arc<Kura>, count: usize) -> Vec<HashOf<BlockHeader>> {
    establish_dummy_store_primary_anchor(kura);
    let mut blocks = NativeBlocks::new();
    let mut hashes = Vec::with_capacity(count);
    for _ in 0..count {
        let block = blocks.next();
        let hash = block.hash();
        kura.store_block(block).expect("store block");
        hashes.push(hash);
    }
    hashes
}

fn read_block(store: &mut BlockStore, index: usize) -> eyre::Result<SignedBlock> {
    let BlockIndex { start, length } = store.read_block_index(index as u64)?;
    let len: usize = length.try_into().unwrap();
    let mut buff = vec![0_u8; len];
    store.read_block_data(start, &mut buff)?;
    let block = decode_versioned_signed_block(&buff).map_err(eyre::Report::new)?;
    Ok(block)
}

fn two_lane_runtime_config() -> RuntimeLaneConfig {
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane0, lane1]).expect("catalog");
    RuntimeLaneConfig::from_catalog(&catalog)
}

fn kura_storage_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
) -> (TempDir, KuraConfig) {
    let temp_dir = TempDir::new().expect(temp_context);
    let config = kura_config_for_dir(&temp_dir, blocks_in_memory);
    (temp_dir, config)
}

fn unwrapped_kura_storage_fixture(blocks_in_memory: NonZeroUsize) -> (TempDir, KuraConfig) {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, blocks_in_memory);
    (temp_dir, config)
}

fn expect_default_kura_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
    open_context: &str,
) -> DefaultKuraFixture {
    let (temp_dir, config) = kura_storage_fixture(temp_context, blocks_in_memory);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect(open_context);
    (temp_dir, config, kura)
}

fn kura_root_fixture(blocks_in_memory: NonZeroUsize) -> DefaultKuraFixture {
    expect_default_kura_fixture("create Kura root", blocks_in_memory, "open Kura")
}

fn unwrapped_kura_fixture() -> DefaultKuraFixture {
    let (temp_dir, config) = unwrapped_kura_storage_fixture(BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    (temp_dir, config, kura)
}

fn unwrapped_inline_kura_fixture_with_fsync(fsync_mode: FsyncMode) -> (TempDir, Arc<Kura>) {
    let temp_dir = TempDir::new().unwrap();
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.fsync_mode = fsync_mode;
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    (temp_dir, kura)
}

fn expect_configured_kura_fixture(
    temp_context: &str,
    blocks_in_memory: NonZeroUsize,
    open_context: &str,
) -> ConfiguredKuraFixture {
    let (temp_dir, config) = kura_storage_fixture(temp_context, blocks_in_memory);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect(open_context);
    establish_configured_lane_markers_for_test(&kura, &lane_config);
    (temp_dir, config, lane_config, kura)
}

fn temporary_kura_fixture() -> ConfiguredKuraFixture {
    expect_configured_kura_fixture(
        "temporary Kura directory",
        BLOCKS_IN_MEMORY,
        "initialize Kura",
    )
}

fn expect_two_lane_storage_fixture(temp_context: &str) -> (TempDir, KuraConfig, RuntimeLaneConfig) {
    let (temp_dir, config) = kura_storage_fixture(temp_context, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    (temp_dir, config, lane_config)
}

fn two_lane_storage_fixture() -> (TempDir, KuraConfig, RuntimeLaneConfig) {
    expect_two_lane_storage_fixture("create temp dir")
}

fn blank_kura_with_next_block() -> (Arc<Kura>, Arc<SignedBlock>) {
    let kura = Kura::blank_kura_for_testing();
    let block = NativeBlocks::new().next();
    (kura, block)
}

fn blank_kura_with_blocks() -> (Arc<Kura>, NativeBlocks) {
    let catalog = LaneCatalog::default();
    let config = kura_config_for_path(Path::new("isolated-kura-fixture"), BLOCKS_IN_MEMORY);
    let kura = Kura::new_temporary_with_configured_lane_catalog(
        &config,
        &RuntimeLaneConfig::from_catalog(&catalog),
        &catalog,
    )
    .expect("open an isolated configured empty store");
    establish_dummy_store_primary_anchor(&kura);
    let blocks = NativeBlocks::new();
    (kura, blocks)
}

fn default_pipeline_sidecar_fixture() -> (
    TempDir,
    KuraConfig,
    Arc<Kura>,
    HashOf<BlockHeader>,
    PipelineRecoverySidecar,
) {
    let (temp_dir, config) = unwrapped_kura_storage_fixture(BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let block_hash = store_dummy_blocks(&kura, 1)[0];
    let sidecar = PipelineRecoverySidecar::new(
        1,
        block_hash,
        PipelineDagSnapshot {
            fingerprint: [0u8; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    (temp_dir, config, kura, block_hash, sidecar)
}

fn test_kura_with_default_lane_markers(
    config: &Config,
    lane_config: &RuntimeLaneConfig,
) -> (Arc<Kura>, BlockCount) {
    let (kura, block_count) = Kura::open_test_kura_with_configured_lane_config(config, lane_config)
        .expect("init Kura test fixture");
    establish_configured_lane_markers_for_test(&kura, lane_config);
    (kura, block_count)
}

fn reopen_test_kura_with_default_lane_geometry(
    config: &Config,
    lane_config: &RuntimeLaneConfig,
) -> Result<(Arc<Kura>, BlockCount)> {
    let (kura, count) = Kura::open_test_kura_with_configured_lane_config(config, lane_config)?;
    kura.restore_published_lane_geometry_for_test(lane_config)?;
    Ok((kura, count))
}

fn establish_configured_lane_markers_for_test(kura: &Kura, lane_config: &RuntimeLaneConfig) {
    publish_initial_configured_lane_geometry_for_test(kura, lane_config, &BTreeMap::new());
    kura.restore_published_lane_geometry_for_test(lane_config)
        .expect("restore the actual published structural fixture geometry");
}

fn publish_initial_configured_lane_geometry_for_test(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    requested_incarnations: &BTreeMap<LaneId, Hash>,
) {
    if kura.lane_storage_network.lock().is_none() {
        kura.bind_lane_storage_network(native_storage_network_id())
            .expect("bind the original native fixture genesis network");
    }
    let (baseline, phases, _) = kura
        .lane_geometry_journal_state_for_test()
        .expect("inspect exact fixture geometry journal");
    if !phases.is_empty() {
        return;
    }
    let baseline = baseline.expect("authenticated fixture catalog baseline");
    let mut incarnations = BTreeMap::new();
    let mut activations = BTreeMap::new();
    for entry in lane_config.entries() {
        let incarnation = if let Some(incarnation) = requested_incarnations.get(&entry.lane_id) {
            *incarnation
        } else {
            match kura
                .lane_storage_entry(entry.lane_id)
                .and_then(|stored| kura.active_lane_incarnation_marker(&stored))
            {
                Ok((incarnation, activation)) => {
                    assert_eq!(
                        activation, 0,
                        "initial fixture geometry activates at genesis"
                    );
                    incarnation
                }
                Err(Error::IO(error, _)) if error.kind() == ErrorKind::NotFound => Hash::new(
                    format!(
                        "kura-lane-incarnation:{}:{}",
                        entry.lane_id.as_u32(),
                        entry.dataspace_id.as_u64()
                    )
                    .as_bytes(),
                ),
                Err(error) => panic!("existing fixture lane marker is invalid: {error}"),
            }
        };
        incarnations.insert(entry.lane_id, incarnation);
        activations.insert(entry.lane_id, 0);
    }
    let primary_incarnation = incarnations[&LaneId::SINGLE];
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        primary_incarnation,
        baseline,
    )
    .expect("anchor the fixture's configured primary");
    if lane_config.entries().len() == 1 {
        return;
    }
    let initial = RuntimeLaneConfig::default();
    assert_eq!(
        initial.primary(),
        lane_config.primary(),
        "this fixture publishes secondary lanes from the canonical primary segment"
    );
    // Secondary targets must still be absent: the real transition owns their
    // first files and marker publication, including crash recovery provenance.
    kura.apply_lane_geometry_transition(
        &initial,
        lane_config,
        &BTreeMap::from([(LaneId::SINGLE, primary_incarnation)]),
        &incarnations,
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
        &activations,
        &BTreeSet::new(),
    )
    .expect("durably apply the fixture's secondary lane geometry");
    kura.mark_lane_geometry_catalog_published(
        lane_config,
        &incarnations,
        &activations,
        Some(baseline),
    )
    .expect("publish the fixture's exact lane geometry for restart");
}

fn active_fixture_geometry_maps(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
) -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    lane_config
        .entries()
        .iter()
        .map(|entry| {
            let (incarnation, activation) = kura
                .active_lane_incarnation_marker(
                    &kura
                        .lane_storage_entry(entry.lane_id)
                        .expect("exact active identity"),
                )
                .expect("authenticate each original journal-published route");
            ((entry.lane_id, incarnation), (entry.lane_id, activation))
        })
        .unzip()
}

fn populate_strict_kura_store(dir: &TempDir, count: usize) {
    let config = kura_config_for_dir(dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    let _ = store_dummy_blocks(&kura, count);
}
