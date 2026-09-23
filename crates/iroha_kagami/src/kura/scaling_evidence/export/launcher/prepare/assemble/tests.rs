//! Actual generated signed genesis joined to an explicit four-validator offline transcript.
//!
//! Genesis authority is really staged. The transcript uses the existing authenticated opaque
//! Native source fixture and public BlockStore framing; it does not claim runtime autonomous execution.

use super::*;
use crate::{
    RunArgs as _,
    kura::scaling_evidence::export::filesystem::{FactsInputBindings, ProofInputBinding},
};
use clap::Parser as _;
use iroha_crypto::{KeyPair, PrivateKey};
use std::{
    fs,
    io::{BufWriter, Write as _},
    path::PathBuf,
};
use zeroize::Zeroizing;
#[path = "../../../../fixture.rs"]
#[allow(dead_code, reason = "fixture is shared by focused test suites")]
mod transcript;

/// Run a full generated-genesis facts fixture on the bounded stack already used by
/// Kagami genesis staging. Keep every fixture owner and assertion on that worker;
/// carry the caller's account discriminant and propagate its original panic.
/// Ordinary CLI stack policy and production command implementations are unchanged.
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
pub(in crate::kura::scaling_evidence::export) fn with_facts_assembly_stack(
    body: impl FnOnce() + Send,
) {
    let discriminant = iroha_data_model::account::address::chain_discriminant();
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("kagami-facts-fixture".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, move || {
                let _discriminant = ChainDiscriminantGuard::enter(discriminant);
                body();
            })
            .expect("spawn bounded facts fixture thread");
        if let Err(payload) = worker.join() {
            std::panic::resume_unwind(payload);
        }
    });
}

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

// Deterministic test seed derived from the original fixture label, carried only over a pipe.
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
const FIXTURE_SEED: &str = "fda714de4dfe08bf8768f76aff80177d365f00290c8259f2501bcaec7a1ee60e";

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
#[allow(
    unsafe_code,
    reason = "test-only anonymous seed pipe transfers its original read end"
)]
fn fixed_scaling_seed_pipe() -> fs::File {
    use std::os::fd::FromRawFd as _;

    let mut descriptors = [-1; 2];
    // SAFETY: the array has room for both newly owned descriptors on success.
    assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
    // SAFETY: each fresh descriptor is converted into exactly one owning File.
    let (read, mut write) = unsafe {
        (
            fs::File::from_raw_fd(descriptors[0]),
            fs::File::from_raw_fd(descriptors[1]),
        )
    };
    let flags = rustix::fs::fcntl_getfl(&read).unwrap();
    rustix::fs::fcntl_setfl(&read, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
    write.write_all(FIXTURE_SEED.as_bytes()).unwrap();
    drop(write);
    read
}

/// Fixed original-role fixture shared only by sibling export tests.
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
pub(in crate::kura::scaling_evidence::export) struct Fixture {
    _temp: tempfile::TempDir,
    paths: [PathBuf; 10],
    originals: Vec<Zeroizing<Vec<u8>>>,
    pins: [[u8; 32]; 10],
    root: PathBuf,
    log: PathBuf,
    output: PathBuf,
    chain_id: ChainId,
    network_id: NetworkId,
    discriminant: u16,
    signer: PublicKey,
    validators: [PublicKey; 4],
    accounts: Vec<AccountId>,
    lanes: usize,
    transcript: transcript::Fixture,
}
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
impl Fixture {
    pub(in crate::kura::scaling_evidence::export) fn new(lanes: usize) -> Self {
        use std::os::fd::{AsRawFd as _, IntoRawFd as _};

        assert!(matches!(lanes, 1 | 4));
        let _fixture_discriminant = ChainDiscriminantGuard::enter(777);
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().canonicalize().unwrap();
        let generated = home.join("generated");
        // Keep the seed read end owned if parsing fails, then transfer it exactly once
        // to the production CLI reader. Generated private buffers are erased on drop.
        let seed_input = fixed_scaling_seed_pipe();
        let seed_fd = seed_input.as_raw_fd().to_string();
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "--out-dir",
            generated.to_str().unwrap(),
            "--seed-fd",
            &seed_fd,
            "--scaling-lanes",
            &lanes.to_string(),
            "--scaling-accounts",
            "4",
        ])
        .unwrap();
        let mut reply = BufWriter::new(Vec::new());
        // Generic development localnets use the configured default discriminant.
        // The outer nondefault scope still checks that generation restores its caller.
        let default_discriminant = iroha_config::parameters::defaults::common::chain_discriminant();
        let generation_discriminant = ChainDiscriminantGuard::enter(default_discriminant);
        let _transferred_seed = seed_input.into_raw_fd();
        cli.command.run(&mut reply).unwrap();
        drop(generation_discriminant);
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            777
        );
        reply.flush().unwrap();
        let reply = String::from_utf8(reply.into_inner().unwrap()).unwrap();
        let receipt: norito::json::Value = norito::json::from_str(&reply).unwrap();
        assert_eq!(receipt["consensus_mode"].as_str(), Some("npos"));
        assert!(!reply.contains(FIXTURE_SEED));
        assert!(!reply.contains("facts-assembler-private-fixture-seed"));
        let first_paths = [
            generated.join("genesis.json"),
            generated.join("genesis.signed.nrt"),
            generated.join("peer0.toml"),
            generated.join("peer1.toml"),
            generated.join("peer2.toml"),
            generated.join("peer3.toml"),
        ];
        let first_bytes = first_paths
            .iter()
            .map(|path| Zeroizing::new(fs::read(path).unwrap()))
            .collect::<Vec<_>>();
        let manifest = parse_manifest(original(&first_paths[0], &first_bytes[0])).unwrap();
        let discriminant = manifest.chain_discriminant();
        assert_eq!(discriminant, default_discriminant);
        let _discriminant = ChainDiscriminantGuard::enter(discriminant);
        let configs: Vec<_> = (0..4)
            .map(|index| {
                parse_config(original(&first_paths[index + 2], &first_bytes[index + 2])).unwrap()
            })
            .collect();
        let keys = configs
            .iter()
            .map(|config| config.common.key_pair.clone())
            .collect::<Vec<_>>();
        let signer = configs[0].genesis.public_key.clone();
        let network_id = NetworkId::from_genesis_hash(configs[0].genesis.expected_hash);
        let validators: [PublicKey; 4] =
            std::array::from_fn(|index| configs[index].common.key_pair.public_key().clone());
        let authority = crate::genesis::staged_signed_genesis_merge_authority(
            &manifest,
            &first_bytes[1],
            &configs[0],
        )
        .unwrap();
        let validated = iroha_genesis::validate_prepared_genesis_bundle(
            &first_bytes[1],
            &manifest,
            &signer,
            configs[0].genesis.expected_hash,
        )
        .unwrap();
        let account_keys = (0..4)
            .map(|index| {
                let raw = Zeroizing::new(
                    fs::read_to_string(generated.join(format!("workload-account-{index:02}.toml")))
                        .unwrap(),
                );
                let table = crate::secret_toml::Table::new(
                    crate::secret_toml::parse_table(&raw, "generated workload account fixture")
                        .unwrap(),
                );
                let account = table["account"].as_table().unwrap();
                KeyPair::new(
                    account["public_key"]
                        .as_str()
                        .unwrap()
                        .parse::<PublicKey>()
                        .unwrap(),
                    account["private_key"]
                        .as_str()
                        .unwrap()
                        .parse::<PrivateKey>()
                        .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let accounts = account_keys
            .iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let journal = journal::tests::generated_original(network_id, &account_keys, lanes, 2);
        let policy = journal::tests::generated_expectations(network_id, &accounts, lanes);
        let (scheduled, _, _) = journal::read_original_journal(
            &journal,
            iroha_crypto::sha256(&journal),
            4 * 1024 * 1024,
            policy,
        )
        .unwrap()
        .into_parts();
        let transcript = transcript::Fixture::from_generated_genesis(
            keys,
            validated.block().clone(),
            &authority,
            &scheduled,
        );
        let context = norito::encode_canonical(authority.context()).unwrap();
        let finality = norito::encode_canonical(&finalized_contexts(&transcript)).unwrap();
        let queries = transcript
            .heights
            .iter()
            .flat_map(|height| height.queries())
            .map(|bytes| canonical::<CommittedTransaction>(&bytes).unwrap())
            .collect::<Vec<_>>();
        let queries = norito::encode_canonical(&queries).unwrap();
        let mut observed_heights = BTreeMap::new();
        for height in &transcript.heights {
            for raw in height.queries() {
                let query: CommittedTransaction = canonical(&raw).unwrap();
                let TransactionEntrypoint::External(signed) = query.entrypoint else {
                    unreachable!()
                };
                observed_heights.insert(
                    signed.hash().to_string(),
                    height.block.header().height().get(),
                );
            }
        }
        let mut updated_journal = Vec::new();
        for line in journal
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let mut row: norito::json::Value = norito::json::from_slice(line).unwrap();
            let hash = row
                .get("hash")
                .or_else(|| row.get("expected_hash"))
                .and_then(norito::json::Value::as_str);
            let height = hash.and_then(|hash| observed_heights.get(hash)).copied();
            for field in ["block_height", "local_block_height"] {
                if let Some(value) = row.get_mut(field) {
                    *value = norito::json!(height.expect("observed original request"));
                }
            }
            updated_journal.extend(norito::json::to_vec(&row).unwrap());
            updated_journal.push(b'\n');
        }
        let journal = updated_journal;
        let input = home.join("originals");
        fs::create_dir(&input).unwrap();
        let names = [
            "genesis.json",
            "genesis.signed.nrt",
            "peer0.toml",
            "peer1.toml",
            "peer2.toml",
            "peer3.toml",
            "context.nrt",
            "journal.jsonl",
            "finality.nrt",
            "queries.nrt",
        ];
        let paths = std::array::from_fn(|index| input.join(names[index]));
        let mut originals = first_bytes;
        originals.extend(
            [context, journal, finality, queries]
                .into_iter()
                .map(Zeroizing::new),
        );
        for (path, bytes) in paths.iter().zip(&originals) {
            write_new(path, bytes);
        }
        let pins = std::array::from_fn(|index| iroha_crypto::sha256(&originals[index]));
        let root = home.join("kura");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = iroha_core::kura::BlockStore::new(&root);
        store.create_files_if_they_do_not_exist().unwrap();
        for height in &transcript.heights {
            store.append_block_to_chain(&height.block).unwrap();
        }
        drop(store);
        let log = root.join("merge.log");
        write_new(&log, &[]);
        for name in [
            "blocks.data",
            "blocks.index",
            "blocks.hashes",
            "blocks.count.norito",
            "merge.log",
        ] {
            let path = root.join(name);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::File::open(path).unwrap().sync_all().unwrap();
        }
        let publication = home.join("publication");
        fs::create_dir(&publication).unwrap();
        let output = publication.join("facts.nrt");
        Self {
            _temp: temp,
            paths,
            originals,
            pins,
            root,
            log,
            output,
            chain_id: manifest.chain_id().clone(),
            network_id,
            discriminant,
            signer,
            validators,
            accounts,
            lanes,
            transcript,
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn bindings(&self) -> FactsInputBindings {
        let binding = |index: usize| ProofInputBinding {
            path: self.paths[index].clone(),
            sha256: self.pins[index],
            max_bytes: 4 * 1024 * 1024,
        };
        FactsInputBindings {
            manifest: binding(0),
            signed_genesis: binding(1),
            peer_configs: std::array::from_fn(|i| binding(i + 2)),
            context: binding(6),
            journal: binding(7),
            finality: binding(8),
            queries: binding(9),
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn genesis(&self) -> GenesisExpectations {
        GenesisExpectations {
            chain_id: self.chain_id.clone(),
            network_id: self.network_id,
            chain_discriminant: self.discriminant,
            genesis_public_key: self.signer.clone(),
            validators: self.validators.clone(),
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn journal(&self) -> JournalExpectations {
        journal::tests::generated_expectations(self.network_id, &self.accounts, self.lanes)
    }
    pub(in crate::kura::scaling_evidence::export) fn verification_limits(
        &self,
    ) -> VerificationLimits {
        VerificationLimits {
            admitted_proof_bytes: 64 * 1024 * 1024,
            input_bytes: 48 * 1024 * 1024,
            output_bytes: 16 * 1024 * 1024,
            heights: 16,
            requests: 8,
            leaves_per_carrier: 8,
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn reader_limits(
        &self,
    ) -> CanonicalKuraEvidenceLimits {
        CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: self.transcript.heights.len() as u64,
            max_committed_blocks: 16,
            max_store_data_bytes: 16 * 1024 * 1024,
            max_carrier_bytes: 8 * 1024 * 1024,
            max_merge_log_bytes: 16 * 1024 * 1024,
            max_merge_frames: 8,
            max_output_bytes: 16 * 1024 * 1024,
            max_decode_allocation_bytes: 128 * 1024 * 1024,
            owner_uid: fs::metadata(&self.root).unwrap().uid(),
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn caps(&self) -> FactsAssemblyCaps {
        FactsAssemblyCaps {
            input_bytes: 64 * 1024 * 1024,
            facts_bytes: 32 * 1024 * 1024,
            total_bytes: 96 * 1024 * 1024,
            decode_bytes: 128 * 1024 * 1024,
        }
    }
    pub(in crate::kura::scaling_evidence::export) fn block_store(&self) -> &Path {
        &self.root
    }
    pub(in crate::kura::scaling_evidence::export) fn merge_log(&self) -> &Path {
        &self.log
    }
    pub(in crate::kura::scaling_evidence::export) fn output_path(&self) -> PathBuf {
        self.output.clone()
    }
    pub(in crate::kura::scaling_evidence::export) fn original_paths(&self) -> Vec<PathBuf> {
        self.paths.to_vec()
    }
    fn originals(&self) -> FactsOriginals<'_> {
        self.with_bytes(&self.originals)
    }
    fn with_bytes<'a>(&'a self, bytes: &'a [Zeroizing<Vec<u8>>]) -> FactsOriginals<'a> {
        let fact = |index: usize| OriginalFact {
            path: &self.paths[index],
            bytes: &bytes[index],
            expected_raw_sha256: iroha_crypto::sha256(&bytes[index]),
            max_bytes: 4 * 1024 * 1024,
        };
        FactsOriginals {
            manifest: fact(0),
            signed_genesis: fact(1),
            peer_configs: std::array::from_fn(|i| fact(i + 2)),
            context: fact(6),
            journal: fact(7),
            finality: fact(8),
            queries: fact(9),
        }
    }
    fn assemble(&self) -> Result<AssembledFacts> {
        assemble(
            self.originals(),
            self.genesis(),
            self.journal(),
            self.verification_limits(),
            &self.root,
            &self.log,
            self.reader_limits(),
            self.caps(),
        )
    }
    fn parse_configs(&self) -> [actual::Root; 4] {
        std::array::from_fn(|i| {
            parse_config(original(&self.paths[i + 2], &self.originals[i + 2])).unwrap()
        })
    }
}
fn original<'a>(path: &'a Path, bytes: &'a [u8]) -> OriginalFact<'a> {
    OriginalFact {
        path,
        bytes,
        expected_raw_sha256: iroha_crypto::sha256(bytes),
        max_bytes: 4 * 1024 * 1024,
    }
}
#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn fixed_original_admission_checks_every_raw_pin_path_and_reservation_before_parse() {
    let bytes: [Vec<u8>; 10] = std::array::from_fn(|i| vec![i as u8]);
    let paths: [PathBuf; 10] =
        std::array::from_fn(|i| PathBuf::from(format!("/facts-preflight/input-{i}")));
    let make = || {
        let f = |i: usize| OriginalFact {
            path: &paths[i],
            bytes: &bytes[i],
            expected_raw_sha256: iroha_crypto::sha256(&bytes[i]),
            max_bytes: 1,
        };
        FactsOriginals {
            manifest: f(0),
            signed_genesis: f(1),
            peer_configs: std::array::from_fn(|i| f(i + 2)),
            context: f(6),
            journal: f(7),
            finality: f(8),
            queries: f(9),
        }
    };
    let caps = FactsAssemblyCaps {
        input_bytes: 10,
        facts_bytes: 1,
        total_bytes: 11,
        decode_bytes: 1024,
    };
    make().admit(caps).unwrap();
    for index in 0..10 {
        let mut originals = make();
        let field = match index {
            0 => &mut originals.manifest,
            1 => &mut originals.signed_genesis,
            2..=5 => &mut originals.peer_configs[index - 2],
            6 => &mut originals.context,
            7 => &mut originals.journal,
            8 => &mut originals.finality,
            _ => &mut originals.queries,
        };
        field.expected_raw_sha256[0] ^= 1;
        assert!(
            originals
                .admit(caps)
                .unwrap_err()
                .to_string()
                .contains("raw digest")
        );
    }
    let mut duplicate = make();
    duplicate.queries.path = duplicate.manifest.path;
    assert!(duplicate.admit(caps).is_err());
    let mut relative = make();
    relative.context.path = Path::new("relative");
    assert!(relative.admit(caps).is_err());
    let mut context = make();
    context.context.max_bytes = 8 * 1024 * 1024 + 1;
    assert!(
        context
            .admit(caps)
            .unwrap_err()
            .to_string()
            .contains("context reservation")
    );
    let mut short = caps;
    short.total_bytes = 10;
    assert!(
        make()
            .admit(short)
            .unwrap_err()
            .to_string()
            .contains("aggregate reservation")
    );
    let mut short = caps;
    short.input_bytes = 9;
    assert!(
        make()
            .admit(short)
            .unwrap_err()
            .to_string()
            .contains("input cap")
    );
}

#[test]
fn every_parse_time_external_read_surface_is_rejected_even_empty_or_disabled() {
    for path in FORBIDDEN_CONFIG {
        let mut table = toml::Table::new();
        let mut leaf = &mut table;
        for part in &path[..path.len() - 1] {
            leaf = leaf
                .entry((*part).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .unwrap();
        }
        let final_part = path.last().unwrap();
        let value = if matches!(*final_part, "account_onboarding" | "faucet" | "codec") {
            toml::Value::Table(toml::Table::new())
        } else {
            toml::Value::String("/facts-must-never-open/missing".to_owned())
        };
        leaf.insert((*final_part).to_owned(), value);
        let text = Zeroizing::new(toml::to_string(&table).unwrap());
        let error = parse_config(original(Path::new("/original/peer.toml"), text.as_bytes()))
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("forbidden external-read"),
            "{path:?}: {error}"
        );
    }
}

#[test]
fn manifest_external_executor_and_trigger_references_fail_before_typed_staging() {
    for value in [
        norito::json!({"executor":"/must-not-open/executor.to", "transactions":[]}),
        norito::json!({"transactions":[{"ivm_triggers":[{"action":"/must-not-open/trigger.to"}]}]}),
    ] {
        let bytes = norito::json::to_vec(&value).unwrap();
        let error =
            parse_manifest(original(Path::new("/original/genesis.json"), &bytes)).unwrap_err();
        assert!(error.to_string().contains("cannot load"));
    }
}

#[test]
fn staged_route_requires_exact_single_coordinator_and_never_uses_fallback() {
    use iroha_core::queue::{RouteLeg, RouteLegRole};
    let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::UNIVERSAL);
    check_route(RoutingPlan::single(route), route).unwrap();
    for actual in [
        RoutingPlan::Single(RouteLeg::new(route, RouteLegRole::Participant)),
        RoutingPlan::single(RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)),
        RoutingPlan::native_amx(
            route,
            vec![RouteLeg::new(
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::new(9)),
                RouteLegRole::Participant,
            )],
        ),
    ] {
        assert!(check_route(actual, route).is_err());
    }
}

#[test]
fn staged_request_slots_charge_all_four_projections_and_reject_one_byte_less() {
    let count = 8;
    let slots = count
        * (std::mem::size_of::<DecodedRequest>() + 4 * std::mem::size_of::<RequestRoute>())
        + 4 * std::mem::size_of::<Vec<RequestRoute>>();
    assert_eq!(request_slot_bytes(count, count).unwrap(), slots);
    assert!(request_slot_bytes(0, count).is_err());
    assert!(request_slot_bytes(count, count - 1).is_err());
    assert!(request_slot_bytes(1, MAX_REQUESTS + 1).is_err());
    let standard = norito::canonical_decode_limits(1024 * 1024);
    for (cap, succeeds) in [(slots, true), (slots - 1, false)] {
        let limits = norito::DecodeLimits::new(
            standard.max_sequence_elements(),
            standard.max_field_bytes(),
            standard.max_total_elements(),
            cap,
            128,
        );
        let result = norito::with_decode_limits_scope(limits, || {
            norito::core::reserve_decode_allocation(request_slot_bytes(count, count).unwrap())
        });
        assert_eq!(result.is_ok(), succeeds);
    }
}

#[test]
fn routing_decode_rejects_finite_work_before_any_signed_frame_decode() {
    let scheduled = vec![ScheduledRequest {
        logical_id: "invalid-original".to_owned(),
        phase: WorkloadPhase::Measurement,
        signed_transaction: vec![0],
        route: RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
    }];
    let error = decode_requests(&scheduled, 0).err().unwrap();
    assert!(error.to_string().contains("independent work bound"));
    assert!(decode_requests(&scheduled, 1).is_err());
}

fn finalized_contexts(fixture: &transcript::Fixture) -> Vec<FinalizedNativeContextV1> {
    fixture
        .heights
        .iter()
        .map(|height| FinalizedNativeContextV1 {
            finality: height.proof.clone(),
            contexts: canonical(&height.evidence).unwrap(),
        })
        .collect()
}

#[test]
fn five_query_group_reserves_exact_slots_and_frames_before_allocation() {
    let fixture = transcript::Fixture::new(4);
    let finality = finalized_contexts(&fixture);
    let queries = fixture
        .heights
        .iter()
        .flat_map(|height| height.queries())
        .take(5)
        .map(|raw| canonical::<CommittedTransaction>(&raw).unwrap())
        .collect::<Vec<_>>();
    let count = finality
        .iter()
        .map(|value| {
            norito::canonical_frame_len(&value.finality).unwrap()
                + norito::canonical_frame_len(&value.contexts).unwrap()
        })
        .sum::<usize>()
        + queries
            .iter()
            .map(|value| norito::canonical_frame_len(value).unwrap())
            .sum::<usize>()
        + finality.len() * std::mem::size_of::<SuppliedEvidenceHeightV1>()
        + 5 * std::mem::size_of::<Vec<u8>>();
    let last = finality.len() as u64;
    let mut limits = transcript::limits();
    limits.input_bytes = count as u64;
    let rows = group_supplied(finality.clone(), queries.clone(), last, limits).unwrap();
    assert_eq!(rows.len(), finality.len());
    assert_eq!(rows[0].queries.len(), 0);
    assert_eq!(rows[1].queries.len(), 4);
    assert_eq!(rows[2].queries.len(), 1);
    for (raw, original) in rows.iter().flat_map(|row| &row.queries).zip(&queries) {
        assert_eq!(raw, &norito::encode_canonical(original).unwrap());
    }
    for (row, original) in rows.iter().zip(&finality) {
        assert_eq!(
            row.contexts,
            norito::encode_canonical(&original.contexts).unwrap()
        );
    }
    limits.input_bytes -= 1;
    assert!(group_supplied(finality, queries, last, limits).is_err());
}

#[test]
fn query_grouping_preserves_all_rows_and_rejects_height_carrier_or_leaf_reordering() {
    let fixture = transcript::Fixture::new(4);
    let proofs = finalized_contexts(&fixture);
    let queries = fixture
        .heights
        .iter()
        .flat_map(|height| height.queries())
        .map(|raw| canonical::<CommittedTransaction>(&raw).unwrap())
        .collect::<Vec<_>>();
    let last = proofs.len() as u64;
    let rows = group_supplied(proofs.clone(), queries.clone(), last, transcript::limits()).unwrap();
    assert_eq!(
        rows.iter().map(|row| row.queries.len()).sum::<usize>(),
        queries.len()
    );
    let mut reversed = proofs.clone();
    reversed.reverse();
    assert!(group_supplied(reversed, queries.clone(), last, transcript::limits()).is_err());
    let mut wrong_leaf = queries.clone();
    wrong_leaf.swap(0, 1);
    assert!(group_supplied(proofs.clone(), wrong_leaf, last, transcript::limits()).is_err());
    let mut unknown = queries.clone();
    unknown[0].block_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong carrier"));
    assert!(group_supplied(proofs.clone(), unknown, last, transcript::limits()).is_err());
    let mut extra = queries.clone();
    extra.push(queries[0].clone());
    assert!(group_supplied(proofs.clone(), extra, last, transcript::limits()).is_err());
    assert!(group_supplied(vec![proofs[0].clone()], queries, last, transcript::limits()).is_err());
}

#[cfg(all(
    unix,
    any(target_vendor = "apple", target_os = "linux", target_os = "android")
))]
mod generated {
    use super::*;
    fn failure<T>(result: Result<T>, reason: &str) {
        let error = result.err().expect("changed original must fail");
        assert!(
            error.to_string().contains(reason),
            "{error}; expected {reason}"
        );
    }
    #[test]
    fn actual_fixed_one_and_four_genesis_journal_and_full_transcript_prepare_and_replay() {
        with_facts_assembly_stack(|| {
            for lanes in [1, 4] {
                let fixture = Fixture::new(lanes);
                let facts = fixture.assemble().unwrap();
                facts.recheck_sources().unwrap();
                let decoded: super::super::super::PrepareFactsV1 =
                    canonical(facts.canonical_bytes()).unwrap();
                assert_eq!(decoded.version, 1);
                assert_eq!(decoded.plan.scheduled.len(), 8);
                assert_eq!(decoded.plan.active_lanes.len(), lanes);
                assert_eq!(decoded.plan.network_id, fixture.network_id);
                assert_eq!(decoded.heights.len(), fixture.transcript.heights.len());
                assert_eq!(facts.verified.rows().len(), 8);
                // Canonical account metadata requests really require state; no default route
                // can stand in for the authenticated world projection that made these facts.
                use iroha_core::queue::{ConfigLaneRouter, LaneRouter as _};
                let configs = fixture.parse_configs();
                let router = ConfigLaneRouter::new(
                    configs[0].nexus.routing_policy.clone(),
                    configs[0].nexus.dataspace_catalog.clone(),
                    configs[0].nexus.lane_catalog.clone(),
                );
                for request in &decoded.plan.scheduled {
                    let signed: SignedTransaction = canonical(&request.signed_transaction).unwrap();
                    assert!(
                        router
                            .try_route_plan_without_state(signed.payload())
                            .unwrap()
                            .is_none()
                    );
                }
                // Use the same wire-to-runtime schedule conversion as production preparation.
                let (projected_plan, _, _) = super::super::super::super::RequestV1 {
                    version: decoded.version,
                    plan: decoded.plan,
                    limits: decoded.limits,
                    bindings: Vec::new(),
                }
                .into_parts()
                .unwrap();
                for authority in &facts._authorities {
                    assert_eq!(authority.genesis.context().network_id, fixture.network_id);
                    check_projected_routes(&authority.routes, &projected_plan.scheduled).unwrap();
                    let mut changed = authority.routes.clone();
                    changed.swap(0, 1);
                    assert!(check_projected_routes(&changed, &projected_plan.scheduled).is_err());
                    let mut changed = authority.routes.clone();
                    changed[0].signed_sha256[0] ^= 1;
                    assert!(check_projected_routes(&changed, &projected_plan.scheduled).is_err());
                    let mut changed = authority.routes.clone();
                    changed[0].route = RoutingDecision::new(LaneId::new(9), DataSpaceId::UNIVERSAL);
                    assert!(check_projected_routes(&changed, &projected_plan.scheduled).is_err());
                    assert!(
                        check_projected_routes(&authority.routes[1..], &projected_plan.scheduled)
                            .is_err()
                    );
                }
                for row in facts.verified.rows() {
                    let original = fixture
                        .transcript
                        .heights
                        .iter()
                        .find(|height| height.block.hash() == row.request.carrier_hash)
                        .unwrap();
                    assert_eq!(
                        row.request.carrier_height,
                        original.block.header().height().get()
                    );
                    assert!(row.request.carrier_height >= 3);
                    assert_eq!(row.request.dataspace_id, DataSpaceId::UNIVERSAL);
                }
                let output = super::super::super::prepare(
                    facts.canonical_bytes(),
                    iroha_crypto::sha256(facts.canonical_bytes()),
                    fixture.caps().facts_bytes,
                    super::super::super::PrepareOutputCaps {
                        request_bytes: 32 * 1024 * 1024,
                        bundle_bytes: 32 * 1024 * 1024,
                        total_bytes: 96 * 1024 * 1024,
                    },
                )
                .unwrap();
                let (request, bundle) = output.into_buffers();
                let request = crate::kura::scaling_evidence::export::launcher::decode(
                    &request,
                    iroha_crypto::sha256(&request),
                    32 * 1024 * 1024,
                )
                .unwrap();
                let (plan, limits, bindings) = request.into_parts();
                let bundle: SuppliedEvidenceBundleV1 = canonical(&bundle).unwrap();
                assert_eq!(bundle.heights.len(), fixture.transcript.heights.len());
                let replay = crate::kura::scaling_evidence::export::replay_export(
                    plan,
                    limits,
                    &bindings,
                    Hash::new(facts.verified.canonical_bytes()),
                    facts.verified.canonical_bytes(),
                )
                .unwrap();
                assert_eq!(replay.canonical_bytes(), facts.verified.canonical_bytes());
                facts.recheck_sources().unwrap();
                assert!(!fixture.output_path().exists());
            }
        });
    }
    #[test]
    fn live_genesis_route_projection_failure_never_returns_its_authority() {
        with_facts_assembly_stack(|| {
            let fixture = Fixture::new(4);
            let _guard = ChainDiscriminantGuard::enter(fixture.discriminant);
            let manifest =
                parse_manifest(original(&fixture.paths[0], &fixture.originals[0])).unwrap();
            let configs = fixture.parse_configs();
            let (scheduled, _, _) = journal::read_original_journal(
                &fixture.originals[7],
                fixture.pins[7],
                4 * 1024 * 1024,
                fixture.journal(),
            )
            .unwrap()
            .into_parts();
            let mut requests =
                decode_requests(&scheduled, fixture.verification_limits().requests).unwrap();
            requests[0].identity.route =
                RoutingDecision::new(LaneId::new(9), DataSpaceId::UNIVERSAL);
            let result = crate::genesis::staged_signed_genesis_with_projection(
                &manifest,
                &fixture.originals[1],
                &configs[0],
                |genesis, staged| project_request_routes(genesis, staged, &requests),
            );
            failure(result, "exact staged configured route");
        });
    }
    #[test]
    fn all_four_original_peer_identities_routing_and_sumeragi_limits_are_checked_before_staging() {
        with_facts_assembly_stack(|| {
            let fixture = Fixture::new(4);
            let _guard = ChainDiscriminantGuard::enter(fixture.discriminant);
            let manifest =
                parse_manifest(original(&fixture.paths[0], &fixture.originals[0])).unwrap();
            let check = |configs: &[actual::Root; 4],
                         expected: GenesisExpectations,
                         journal: JournalExpectations| {
                admit_genesis(&manifest, configs, &expected, &journal)
            };
            let mut configs = fixture.parse_configs();
            configs[3].sumeragi.block.max_transactions =
                std::num::NonZeroUsize::new(configs[3].sumeragi.block.max_transactions.get() + 1)
                    .unwrap();
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "Sumeragi v2 configuration",
            );
            let mut configs = fixture.parse_configs();
            configs[3].sumeragi.queues.commands =
                std::num::NonZeroUsize::new(configs[3].sumeragi.queues.commands.get() + 1).unwrap();
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "Sumeragi v2 configuration",
            );
            let mut configs = fixture.parse_configs();
            configs[3].sumeragi.role = actual::NodeRole::Observer;
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "Sumeragi v2 configuration",
            );
            let mut configs = fixture.parse_configs();
            configs[3].nexus.routing_policy.rules[0].lane = LaneId::new(3);
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "routing rule",
            );
            let mut configs = fixture.parse_configs();
            configs[3].nexus.autoscale.enabled = true;
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "fixed one/four",
            );
            let configs = fixture.parse_configs();
            let mut expected = fixture.genesis();
            expected.validators.swap(0, 1);
            failure(
                check(&configs, expected, fixture.journal()),
                "peer launch identity",
            );
            let mut expected = fixture.genesis();
            expected.validators[3] = expected.validators[0].clone();
            failure(
                check(&configs, expected, fixture.journal()),
                "distinct original",
            );
            let mut journal = fixture.journal();
            journal.accounts.swap(0, 1);
            failure(
                check(&configs, fixture.genesis(), journal),
                "independent fixed route",
            );
            // A valid generic catalog can contain 0,1,2,4. Install that same
            // catalog on every peer and in both effective/configured views:
            // matching peer counts and internally consistent catalogs cannot
            // replace the fixed 0..3 lane identity required in every trial.
            let mut configs = fixture.parse_configs();
            let mut lanes = configs[0].nexus.lane_catalog.lanes().to_vec();
            lanes[3].id = LaneId::new(4);
            let changed = iroha_data_model::nexus::LaneCatalog::new(
                std::num::NonZeroU32::new(5).unwrap(),
                lanes,
            )
            .unwrap();
            assert_eq!(changed.lanes().len(), 4);
            for config in &mut configs {
                config.nexus.lane_catalog = changed.clone();
                config.nexus.configured_lane_catalog = changed.clone();
            }
            failure(
                check(&configs, fixture.genesis(), fixture.journal()),
                "facts lane geometry mismatch",
            );
            // Rewriting the workload and all four routing policies to agree
            // with that foreign identity still cannot change the fixed plan.
            let mut journal = fixture.journal();
            for account in &mut journal.accounts {
                if account.route.lane_id == LaneId::new(3) {
                    account.route.lane_id = LaneId::new(4);
                }
            }
            for config in &mut configs {
                for rule in &mut config.nexus.routing_policy.rules {
                    if rule.lane == LaneId::new(3) {
                        rule.lane = LaneId::new(4);
                    }
                }
            }
            failure(
                check(&configs, fixture.genesis(), journal),
                "independent fixed route",
            );
            let configs = fixture.parse_configs();
            let mut journal = fixture.journal();
            journal.accounts[0].authority = AccountId::new(KeyPair::random().public_key().clone());
            failure(
                check(&configs, fixture.genesis(), journal),
                "not registered",
            );
        });
    }
    #[test]
    fn direct_work_bounds_reject_zero_unbounded_or_excess_reader_before_genesis() {
        let limits = transcript::limits();
        let reader = CanonicalKuraEvidenceLimits {
            first_height: 1,
            last_height: 2,
            max_committed_blocks: 8,
            max_store_data_bytes: 2 * 1024 * 1024,
            max_carrier_bytes: 1024 * 1024,
            max_merge_log_bytes: 2 * 1024 * 1024,
            max_merge_frames: 8,
            max_output_bytes: 1024 * 1024,
            max_decode_allocation_bytes: 8 * 1024 * 1024,
            owner_uid: 0,
        };
        admit_work(limits, reader).unwrap();
        let mut changed_limits = limits;
        changed_limits.heights = u64::MAX;
        failure(admit_work(changed_limits, reader), "proof work");
        let mut changed_limits = limits;
        changed_limits.requests = 0;
        failure(admit_work(changed_limits, reader), "proof work");
        let mut changed_reader = reader;
        changed_reader.max_decode_allocation_bytes = 0;
        failure(admit_work(limits, changed_reader), "reader work");
        let mut changed_reader = reader;
        changed_reader.first_height = 2;
        failure(admit_work(limits, changed_reader), "reader work");
        let mut changed_reader = reader;
        changed_reader.max_store_data_bytes = limits.input_bytes + 1;
        failure(admit_work(limits, changed_reader), "reader work");
    }
    #[test]
    fn actual_journal_claimed_applied_height_must_equal_the_verified_carrier() {
        with_facts_assembly_stack(|| {
            let fixture = Fixture::new(1);
            let mut bytes = fixture.originals.clone();
            let mut changed = Vec::new();
            for line in bytes[7]
                .split(|b| *b == b'\n')
                .filter(|line| !line.is_empty())
            {
                let mut row: norito::json::Value = norito::json::from_slice(line).unwrap();
                for field in ["block_height", "local_block_height"] {
                    if let Some(value) = row.get_mut(field) {
                        *value = norito::json!(3);
                    }
                }
                changed.extend(norito::json::to_vec(&row).unwrap());
                changed.push(b'\n');
            }
            bytes[7] = Zeroizing::new(changed);
            // This is a coherent original-journal observation, not a broken parser witness.
            let complete = journal::read_original_journal(
                &bytes[7],
                iroha_crypto::sha256(&bytes[7]),
                4 * 1024 * 1024,
                fixture.journal(),
            )
            .unwrap();
            assert!(
                complete
                    .into_parts()
                    .1
                    .iter()
                    .all(|row| row.block_height == 3)
            );
            failure(
                assemble(
                    fixture.with_bytes(&bytes),
                    fixture.genesis(),
                    fixture.journal(),
                    fixture.verification_limits(),
                    fixture.block_store(),
                    fixture.merge_log(),
                    fixture.reader_limits(),
                    fixture.caps(),
                ),
                "journal observation differs",
            );
            assert!(!fixture.output_path().exists());
        });
    }
    #[test]
    fn actual_complete_tip_cannot_be_replaced_by_a_successful_shorter_interval() {
        with_facts_assembly_stack(|| {
            use iroha_data_model::block::builder::BlockBuilder;
            let fixture = Fixture::new(1);
            let header = BlockHeader::new(
                std::num::NonZeroU64::new(fixture.transcript.heights.len() as u64 + 1).unwrap(),
                Some(fixture.transcript.heights.last().unwrap().block.hash()),
                None,
                200,
                0,
            );
            let third = BlockBuilder::new(header)
                .build_with_signature(0, fixture.transcript.keys[0].private_key());
            let mut store = iroha_core::kura::BlockStore::new(fixture.block_store());
            store.append_block_to_chain(&third).unwrap();
            drop(store);
            failure(fixture.assemble(), "actual stopped durable tip");
            assert!(!fixture.output_path().exists());
        });
    }
    #[test]
    fn retained_core_mutation_poisons_the_actual_assembled_owner() {
        with_facts_assembly_stack(|| {
            let fixture = Fixture::new(1);
            let facts = fixture.assemble().unwrap();
            facts.recheck_sources().unwrap();
            let path = fixture.block_store().join("blocks.count.norito");
            let replacement = fixture.block_store().join("replacement.norito");
            write_new(&replacement, &fs::read(&path).unwrap());
            fs::rename(replacement, path).unwrap();
            assert!(facts.recheck_sources().is_err());
            failure(facts.recheck_sources(), "poisoned");
            failure(facts.ensure_publication_ancestry(&[]), "poisoned");
        });
    }
    #[test]
    fn retained_core_namespace_cannot_be_used_for_facts_publication_or_retried() {
        with_facts_assembly_stack(|| {
            let fixture = Fixture::new(1);
            let facts = fixture.assemble().unwrap();
            let metadata = fs::metadata(fixture.block_store()).unwrap();
            assert!(
                facts
                    .ensure_publication_ancestry(&[(metadata.dev(), metadata.ino())])
                    .is_err()
            );
            failure(facts.ensure_publication_ancestry(&[]), "poisoned");
        });
    }
}
