//! Native signed-fixture collection and retained publication rejection controls.
//!
//! Disk projections copy exact values and frames from the executed native fixture.
//! These tests do not stand in for runtime archive capture or network qualification.

use super::*;
use crate::kura::scaling_evidence::{export::NativeHeightEvidenceV1, fixture};
use iroha_core::kura::BlockStore;
use iroha_data_model::query::CommittedTransaction;
use std::{
    fs,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink},
};

struct Disk {
    _temp: tempfile::TempDir,
    store: PathBuf,
    merge: PathBuf,
    genesis: PathBuf,
    context: PathBuf,
    carrier: PathBuf,
    queries: PathBuf,
    fixture: fixture::Fixture,
}
fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
impl Disk {
    fn new(lanes: usize) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let store = root.join("store");
        let output = root.join("output");
        let original = root.join("original");
        for directory in [&store, &output, &original] {
            fs::create_dir(directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let fixture = fixture::Fixture::new(lanes);
        let mut disk = BlockStore::new(&store);
        disk.create_files_if_they_do_not_exist().unwrap();
        for height in &fixture.heights {
            disk.append_block_to_chain(&height.block).unwrap();
        }
        drop(disk);
        let merge = store.join("merge.log");
        write(&merge, &[]);
        let archive = store.join("native-contexts");
        fs::create_dir(&archive).unwrap();
        fs::set_permissions(&archive, fs::Permissions::from_mode(0o700)).unwrap();
        for height in &fixture.heights {
            let name = format!(
                "{:020}-{}.nrt",
                height.block.header().height().get(),
                hex::encode(height.block.hash().as_ref())
            );
            write(
                &archive.join(name),
                &norito::encode_canonical(&height.lane_evidence.state).unwrap(),
            );
            for frame in &height.lane_evidence.frames {
                let entry =
                    crate::kura::scaling_evidence::lane_proof::decode_frame(&frame.frame).unwrap();
                let directory = store
                    .join("lanes")
                    .join(hex::encode(entry.block.header.instance.0));
                fs::create_dir_all(&directory).unwrap();
                fs::set_permissions(
                    directory.parent().unwrap(),
                    fs::Permissions::from_mode(0o700),
                )
                .unwrap();
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
                write(
                    &directory.join(format!("{:020}.frame", entry.block.header.height)),
                    &frame.frame,
                );
            }
        }
        let genesis = original.join("genesis.nrt");
        write(
            &genesis,
            &fixture.heights[0]
                .block
                .canonical_resultless_proposal()
                .encode_wire()
                .unwrap(),
        );
        let epoch =
            iroha_data_model::sumeragi_finality::genesis_epoch(&fixture.heights[0].block).unwrap();
        let context = original.join("epoch.nrt");
        write(&context, &norito::encode_canonical(&epoch).unwrap());
        Self {
            _temp: temp,
            store,
            merge,
            genesis,
            context,
            carrier: output.join("carrier.nrt"),
            queries: output.join("queries.nrt"),
            fixture,
        }
    }
    fn binding(&self, path: &Path) -> ProofInputBinding {
        let bytes = fs::read(path).unwrap();
        ProofInputBinding {
            path: path.to_owned(),
            sha256: iroha_crypto::sha256(&bytes),
            max_bytes: 1024 * 1024,
        }
    }
    fn limits(&self) -> CollectionLimits {
        CollectionLimits {
            carrier_bytes: 8 * 1024 * 1024,
            query_bytes: 8 * 1024 * 1024,
            total_bytes: 32 * 1024 * 1024,
            context_bytes: 1024 * 1024,
            queries: 1024,
            leaves_per_carrier: 1024,
        }
    }
    fn reader(&self) -> CanonicalKuraEvidenceLimits {
        CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: self.fixture.heights.len() as u64,
            max_committed_blocks: 1024,
            max_store_data_bytes: 16 * 1024 * 1024,
            max_carrier_bytes: 1024 * 1024,
            max_merge_log_bytes: 1024 * 1024,
            max_merge_frames: 1024,
            max_output_bytes: 16 * 1024 * 1024,
            max_decode_allocation_bytes: 32 * 1024 * 1024,
            owner_uid: fs::metadata(&self.store).unwrap().uid(),
        }
    }
    fn collect(
        &self,
        hook: impl FnMut(PrepareRole, Phase) -> Result<()>,
    ) -> Result<CollectedLaunch> {
        let plan = self.fixture.plan();
        let limits = self.limits();
        collect_with_hook(
            self.binding(&self.genesis),
            self.binding(&self.context),
            plan.chain_id,
            plan.network_id,
            plan.genesis_epoch_context_id,
            &self.store,
            &self.merge,
            self.reader(),
            limits,
            CollectedOutputPair::admit(&self.carrier, &self.queries, limits)?,
            hook,
        )
    }
    fn record(&self, index: usize) -> PathBuf {
        let block = &self.fixture.heights[index].block;
        self.store.join("native-contexts").join(format!(
            "{:020}-{}.nrt",
            block.header().height().get(),
            hex::encode(block.hash().as_ref())
        ))
    }
}

#[test]
fn complete_one_and_four_lane_collection_preserves_every_original_carrier_and_actual_query() {
    for lanes in [1, 4] {
        let disk = Disk::new(lanes);
        let original = fs::read(&disk.genesis).unwrap();
        let complete = disk.collect(|_, _| Ok(())).unwrap();
        let carriers: Vec<NativeHeightEvidenceV1> =
            norito::decode_canonical(&fs::read(&disk.carrier).unwrap()).unwrap();
        let queries: Vec<CommittedTransaction> =
            norito::decode_canonical(&fs::read(&disk.queries).unwrap()).unwrap();
        assert_eq!(carriers.len(), disk.fixture.heights.len());
        let mut expected = Vec::new();
        for (collected, height) in carriers.iter().zip(&disk.fixture.heights) {
            assert_eq!(collected.carrier, height.block.encode_wire().unwrap());
            assert_eq!(
                norito::encode_canonical(&collected.lane_evidence).unwrap(),
                height.evidence
            );
            expected.extend(height.queries());
        }
        assert_eq!(queries.len(), expected.len());
        assert_eq!(
            queries.len(),
            10,
            "two genuine prefix clock inputs plus all eight workload inputs"
        );
        assert_eq!(
            queries
                .iter()
                .filter(|query| disk
                    .fixture
                    .requests
                    .iter()
                    .any(|row| row.1.hash_as_entrypoint() == query.entrypoint_hash))
                .count(),
            8
        );
        for (query, bytes) in queries.iter().zip(expected) {
            assert_eq!(norito::encode_canonical(query).unwrap(), bytes);
            let carrier = disk
                .fixture
                .heights
                .iter()
                .find(|height| height.block.hash() == query.block_hash)
                .unwrap();
            assert!(query.verify_inclusion_in_block(&carrier.block));
        }
        let receipt = complete
            .finish_reply(|identity| {
                assert_eq!(identity.genesis.raw_sha256, iroha_crypto::sha256(&original));
                assert_eq!(
                    identity.carrier.raw_sha256,
                    iroha_crypto::sha256(fs::read(&disk.carrier).unwrap())
                );
                assert_eq!(
                    identity.queries.raw_sha256,
                    iroha_crypto::sha256(fs::read(&disk.queries).unwrap())
                );
                assert_eq!(identity.query_count, 10);
                Ok(())
            })
            .unwrap();
        assert_eq!(receipt.committed_height, carriers.len() as u64);
        assert_eq!(fs::read(&disk.genesis).unwrap(), original);
        assert!(
            !disk
                .carrier
                .with_file_name("carrier.nrt.publishing")
                .exists()
        );
    }
}

#[test]
fn missing_changed_or_noncanonical_original_projection_cannot_publish_any_vector() {
    for case in ["missing", "corrupt", "trailing", "foreign", "symlink"] {
        let disk = Disk::new(4);
        let path = disk.record(1);
        match case {
            "missing" => fs::remove_file(&path).unwrap(),
            "corrupt" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes[0] ^= 1;
                write(&path, &bytes);
            }
            "trailing" => {
                let mut bytes = fs::read(&path).unwrap();
                bytes.push(0);
                write(&path, &bytes);
            }
            "foreign" => write(&path, &fs::read(disk.record(0)).unwrap()),
            "symlink" => {
                fs::remove_file(&path).unwrap();
                symlink(disk.record(0), &path).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(disk.collect(|_, _| Ok(())).is_err(), "{case}");
        assert!(!disk.carrier.exists() && !disk.queries.exists());
    }
}

#[test]
fn collection_rejects_independent_epoch_network_original_genesis_and_all_work_bounds() {
    for case in [
        "epoch",
        "network",
        "genesis",
        "genesis_only",
        "carrier",
        "queries",
        "total",
        "context",
        "leaves",
    ] {
        let disk = Disk::new(1);
        let plan = disk.fixture.plan();
        let mut limits = disk.limits();
        let mut reader = disk.reader();
        let mut genesis = fs::read(&disk.genesis).unwrap();
        let mut epoch = plan.genesis_epoch_context_id;
        let mut network = plan.network_id;
        match case {
            "epoch" => epoch[0] ^= 1,
            "network" => {
                network = NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign")),
                )
            }
            "genesis" => genesis.push(0),
            "genesis_only" => reader.last_height = 1,
            "carrier" => limits.carrier_bytes = 1,
            "queries" => limits.query_bytes = 1,
            "total" => limits.total_bytes = 1,
            "context" => limits.context_bytes = 1,
            "leaves" => {
                limits.queries = 1;
                limits.leaves_per_carrier = 1;
            }
            _ => unreachable!(),
        }
        assert!(
            collect_native_inputs(
                plan.chain_id,
                network,
                epoch,
                &genesis,
                &disk.store,
                &disk.merge,
                reader,
                limits
            )
            .is_err(),
            "{case}"
        );
        assert!(!disk.carrier.exists() && !disk.queries.exists());
    }
}

#[test]
fn native_collected_pair_never_replaces_existing_destinations_or_aliases_originals() {
    let disk = Disk::new(1);
    assert!(CollectedOutputPair::admit(&disk.carrier, &disk.carrier, disk.limits()).is_err());
    assert!(CollectedOutputPair::admit(&disk.genesis, &disk.queries, disk.limits()).is_err());
    write(&disk.carrier, b"original output");
    assert!(disk.collect(|_, _| Ok(())).is_err());
    assert_eq!(fs::read(&disk.carrier).unwrap(), b"original output");
    assert!(!disk.queries.exists());
}

#[test]
fn native_collection_rechecks_original_inputs_and_store_at_publication_and_reply() {
    for case in ["genesis", "store", "output", "reply"] {
        let disk = Disk::new(4);
        if case == "reply" {
            let complete = disk.collect(|_, _| Ok(())).unwrap();
            assert!(
                complete
                    .finish_reply(|_| {
                        write(&disk.genesis, b"changed");
                        Ok(())
                    })
                    .is_err()
            );
            continue;
        }
        let mut fired = false;
        let result = disk.collect(|role, phase| {
            if !fired && role == PrepareRole::Bundle && phase == Phase::AfterWrite {
                fired = true;
                match case {
                    "genesis" => write(&disk.genesis, b"changed"),
                    "store" => write(&disk.store.join("blocks.hashes"), b"changed"),
                    "output" => write(
                        &disk.carrier.with_file_name("carrier.nrt.publishing"),
                        b"changed",
                    ),
                    _ => unreachable!(),
                }
            }
            Ok(())
        });
        assert!(fired && result.is_err(), "{case}");
    }
}
