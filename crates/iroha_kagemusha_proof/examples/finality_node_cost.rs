//! Gate-1 measurement: cost of one genuine proof node of each ordinary Load
//! finality class, and the fixed per-height traversal of the current catalog.
//!
//! Every node uses the crate's real circuits and the same code path as the
//! offline catalog compiler and the installed producer:
//! `keygen_pk_v2` with the compiler's key configuration for the source (Vesta)
//! and its one-key Omega wrapper (Pallas), canonical original serialization,
//! strict original import through `Prover::from_original_artifacts`, then
//! `Prover::prove` (source proof, fold, wrapper proof and its internal checks)
//! and `Prover::verify_evidence`. The witnesses come from the captured native
//! receipt fixture `fixtures/kagemusha/ordinary_load_receipt_v1.json` through
//! the same `prepare_*` functions that `InstalledFinality::append_block` uses.
//! A standalone source proof with the same key splits the node cost into its
//! source and wrapper parts. All keys are k16.
//!
//! The keys are generated locally for measurement. They are not an installed
//! catalog, the anchor is a test fixture and nothing here grants authority.
//!
//! Usage (release build only; run one class per process so `/usr/bin/time -l`
//! reports that class's peak RSS):
//!
//! ```text
//! cargo run --release --offline --locked -p iroha_kagemusha_proof \
//!     --example finality_node_cost -- counts
//! RAYON_NUM_THREADS=1 /usr/bin/time -l target/release/examples/finality_node_cost <class> [position]
//! ```
//!
//! Classes: `bls`, `aggregation`, `result`, `schedule`, `context`, `load`
//! (one leaf at `position`, default 0) and `merge` (two adjacent Context CRC
//! leaves, positions 2 and 3, then their real interval merge). The wrapper is
//! measured inside every class: its keygen, key sizes, the derived proving
//! share and the node verification are wrapper costs. `counts` prints the
//! fixed traversal; `footprint` prints the exact source PK length of every
//! unique leaf class from its real descriptor (verifying-key generation only);
//! `native` times the native verification of the same captured CommitQC
//! (committee PoP admission and the quorum aggregate BLS check) for comparison.

use std::{collections::BTreeMap, time::Instant};

use ff::{Field, PrimeField};
use iroha_kagemusha_proof::{
    finality::{
        LoadReceiptCells,
        aggregate::{prepare_aggregation, propose_aggregate_key},
        bls::prepare_bls_batches,
        continuity::{
            SourceMergeCircuit, SourceNodeEvidence, SourcePairPlan, leaf_frame_native,
            producer::{OriginalArtifact, Prover, SourceCircuit},
        },
        load_source::prepare_load_source,
        native::{Parameters, compiled_leaf_schedule_transcript},
        result_scan::prepare_result_batches,
        schedule::{context_hash::prepare_context_batches, source::prepare_schedule_source},
    },
    omega::{OmegaCircuit, OmegaPlan, OmegaWitness},
};
use iroha_pasta::{Fp, Fq, msm::MemoryBudget};
use iroha_plonk::{
    ProverConfig, ProverRandomness, Witness, create_proof_owned_with_claim,
    cs::InstanceType,
    frontend::Circuit,
    keys::{
        CosetCachePolicy, KeygenConfigV2, keygen_pk_v2, keygen_vk_with_binding_v2,
        pk::artifact::ReadConfig,
    },
    pcs::ipa::PinnedParams,
    verifier::verify_full,
};
use iroha_plonk_recursion::{FOLD_WITNESS_BYTES, FoldConfig};
use norito::json::Value;
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

const FIXTURE: &str = include_str!("../../../fixtures/kagemusha/ordinary_load_receipt_v1.json");

// ---------------------------------------------------------------------------
// Process probes: wall, process CPU, 1/5/15-minute load and current RSS.

fn cpu_seconds() -> f64 {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::ProcessCPUTime);
    #[allow(clippy::cast_precision_loss, reason = "exact below 2^53 ns")]
    let seconds = time.tv_sec as f64 + time.tv_nsec as f64 / 1e9;
    seconds
}

fn load_averages() -> String {
    if cfg!(target_os = "linux") {
        return std::fs::read_to_string("/proc/loadavg")
            .map(|text| text.split_whitespace().take(3).collect::<Vec<_>>().join("/"))
            .unwrap_or_else(|_| "unavailable".to_owned());
    }
    std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .filter(|word| *word != "{" && *word != "}")
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_else(|_| "unavailable".to_owned())
}

fn rss_kib() -> String {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|_| "unavailable".to_owned())
}

/// Measured phases of one class, printed as they complete.
struct Report {
    class: String,
    values: BTreeMap<String, String>,
}
impl Report {
    fn new(class: &str) -> Self {
        Self {
            class: class.to_owned(),
            values: BTreeMap::new(),
        }
    }
    fn phase<T>(&mut self, name: &str, run: impl FnOnce() -> T) -> T {
        let load_start = load_averages();
        let cpu = cpu_seconds();
        let start = Instant::now();
        let value = run();
        let wall = start.elapsed().as_secs_f64();
        let cpu = cpu_seconds() - cpu;
        let load_end = load_averages();
        let rss = rss_kib();
        println!(
            "FINALITY_NODE_PHASE class={} phase={name} wall_s={wall:.3} cpu_s={cpu:.3} rss_now_kib={rss} load_start={load_start} load_end={load_end}",
            self.class
        );
        self.values.insert(format!("{name}_wall_s"), format!("{wall:.3}"));
        self.values.insert(format!("{name}_cpu_s"), format!("{cpu:.3}"));
        value
    }
    fn value(&mut self, name: &str, value: impl ToString) {
        self.values.insert(name.to_owned(), value.to_string());
    }
    fn wall(&self, name: &str) -> f64 {
        self.values[&format!("{name}_wall_s")].parse().unwrap()
    }
    fn finish(&mut self) {
        let derived = self.wall("node_prove") - self.wall("source_prove_standalone");
        self.value("wrapper_and_checks_derived_s", format!("{derived:.3}"));
        let fields = self
            .values
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "FINALITY_NODE_COST class={} threads={} {fields}",
            self.class,
            rayon::current_num_threads()
        );
    }
}

fn ok<T, E: core::fmt::Debug>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|error| panic!("{what}: {error:?}"))
}

// ---------------------------------------------------------------------------
// Captured native fixture, converted exactly as the native session does.

fn hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "odd hex length");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}
fn field<'a>(json: &'a Value, name: &str) -> &'a Value {
    json.get(name)
        .unwrap_or_else(|| panic!("fixture field {name}"))
}
fn bytes(json: &Value, name: &str) -> Vec<u8> {
    hex(field(json, name).as_str().expect("hex string"))
}
fn fixed<const N: usize>(json: &Value, name: &str) -> [u8; N] {
    bytes(json, name).try_into().expect("fixed length")
}

struct Block {
    frame: Vec<u8>,
    message: [u8; 165],
    roster: Vec<[u8; 48]>,
    bitmap: Vec<u8>,
    signature: [u8; 96],
    context: [u8; 32],
    receipt: [u8; LoadReceiptCells::BYTES],
    event_root: [u8; 32],
}
fn block() -> Block {
    let json: Value = ok(norito::json::from_str(FIXTURE), "fixture JSON");
    let schedule = field(&json, "authenticated_schedule");
    assert!(
        field(schedule, "boundary").is_null(),
        "fixture has no boundary"
    );
    let context = fixed(field(schedule, "current"), "context_id_hex");
    let roster = field(&json, "committee_public_keys_hex")
        .as_array()
        .expect("roster")
        .iter()
        .map(|key| {
            hex(key.as_str().expect("key"))
                .try_into()
                .expect("48-byte key")
        })
        .collect();
    Block {
        frame: bytes(&json, "result_preimage_hex"),
        message: fixed(&json, "commit_vote_preimage_hex"),
        roster,
        bitmap: bytes(&json, "qc_bitmap_hex"),
        signature: fixed(&json, "qc_aggregate_signature_hex"),
        context,
        receipt: fixed(&json, "receipt_transcript_hex"),
        event_root: fixed(&json, "event_commitment_root_hex"),
    }
}

// ---------------------------------------------------------------------------
// Key configuration identical to `finality::catalog::key_config`.

fn key_config(types: Vec<InstanceType>, source: bool) -> KeygenConfigV2 {
    let mut config = KeygenConfigV2::pipa_r(types);
    config.compress_selectors = !source;
    config.coset_cache = CosetCachePolicy::OnDemand;
    config.msm_budget = MemoryBudget::DEFAULT;
    config
}

fn randomness(seed: u8) -> ProverRandomness<'static> {
    ProverRandomness::recovery(move |_context: &[u8; 32]| {
        Ok::<_, core::convert::Infallible>(ChaCha20Rng::from_seed([seed; 32]))
    })
}

struct Original {
    descriptor: Vec<u8>,
    verifying_key: Vec<u8>,
    proving_key: Vec<u8>,
}
impl Original {
    fn borrowed(&self) -> OriginalArtifact<'_> {
        OriginalArtifact {
            descriptor: &self.descriptor,
            verifying_key: &self.verifying_key,
            proving_key: &self.proving_key,
        }
    }
}

/// Compile, import, prove and verify one genuine node; return its producer and evidence.
fn measure<C: SourceCircuit>(
    report: &mut Report,
    params: &Parameters,
    live: &C,
    frame: &[Fp; 69],
    seed: u8,
) -> (Prover<C>, SourceNodeEvidence) {
    let budget = MemoryBudget::DEFAULT;
    // Source key, as `Compiler::source` generates it.
    let source_key = report.phase("source_keygen", || {
        ok(
            keygen_pk_v2(
                &params.vesta,
                &live.without_witnesses(),
                &key_config(vec![InstanceType::Bounded], true),
            ),
            "source keygen",
        )
    });
    let source = report.phase("source_serialize", || Original {
        descriptor: source_key.binding().encoded().to_vec(),
        verifying_key: source_key.vk().to_bytes().to_vec(),
        proving_key: ok(source_key.artifact_bytes_v2(), "source original"),
    });
    let descriptor = source_key.binding().descriptor();
    report.value("source_fixed_columns", descriptor.num_fixed_columns);
    report.value("source_advice_columns", descriptor.num_advice_columns);
    report.value("source_descriptor_bytes", source.descriptor.len());
    report.value("source_vk_bytes", source.verifying_key.len());
    report.value("source_pk_bytes", source.proving_key.len());
    // The same source proof that `Prover::prove` makes first, alone.
    let public = vec![frame.to_vec()];
    let output = report.phase("source_prove_standalone", || {
        let witness = ok(
            Witness::from_circuit(&source_key, live, &public),
            "source witness",
        );
        ok(
            create_proof_owned_with_claim(
                &params.vesta,
                &source_key,
                witness,
                randomness(seed),
                ProverConfig::default(),
            ),
            "source proof",
        )
    });
    report.value("source_proof_bytes", output.proof.len());
    report.phase("source_verify_standalone", || {
        ok(
            verify_full(
                &params.vesta,
                source_key.binding(),
                source_key.vk(),
                &public,
                &output.proof,
                budget,
            ),
            "source verify",
        );
        ok(
            output.opening.decide(&params.vesta, budget),
            "source opening",
        );
    });
    // One-key Omega wrapper, as `catalog::wrapper` builds it.
    let blank = {
        let digest = ok(
            source_key.vk().kagemusha_digest(source_key.binding()),
            "source key digest",
        );
        let plan = ok(
            OmegaPlan::new(
                source_key.binding().clone(),
                params.vesta.clone(),
                vec![digest],
            ),
            "wrapper plan",
        );
        let plan = ok(
            plan.with_key_catalog(vec![source_key.vk().clone()]),
            "wrapper catalog",
        );
        let length = plan.verifier().proof_length();
        ok(
            OmegaCircuit::new(
                plan,
                OmegaWitness {
                    key: source_key.vk().clone(),
                    instances: vec![Fp::ZERO; 69],
                    proof: vec![0; length],
                    length: u32::try_from(length).unwrap(),
                    fold: [0; FOLD_WITNESS_BYTES],
                },
            ),
            "wrapper layout",
        )
        .without_witnesses()
    };
    drop(source_key);
    let wrapper_key = report.phase("wrapper_keygen", || {
        ok(
            keygen_pk_v2(
                &params.pallas,
                &blank,
                &key_config(OmegaPlan::instance_types().to_vec(), false),
            ),
            "wrapper keygen",
        )
    });
    let wrapper = report.phase("wrapper_serialize", || Original {
        descriptor: wrapper_key.binding().encoded().to_vec(),
        verifying_key: wrapper_key.vk().to_bytes().to_vec(),
        proving_key: ok(wrapper_key.artifact_bytes_v2(), "wrapper original"),
    });
    let descriptor = wrapper_key.binding().descriptor();
    report.value("wrapper_fixed_columns", descriptor.num_fixed_columns);
    report.value("wrapper_advice_columns", descriptor.num_advice_columns);
    report.value("wrapper_descriptor_bytes", wrapper.descriptor.len());
    report.value("wrapper_vk_bytes", wrapper.verifying_key.len());
    report.value("wrapper_pk_bytes", wrapper.proving_key.len());
    drop(wrapper_key);
    drop(blank);
    // Strict original import, which `Mounted::prove_with` repeats for every node.
    let config = ReadConfig {
        maximum_bytes: source.proving_key.len().max(wrapper.proving_key.len()),
        maximum_rows: 1 << 16,
        coset_cache: CosetCachePolicy::OnDemand,
        msm_budget: budget,
    };
    let prover = report.phase("strict_import", || {
        ok(
            Prover::from_original_artifacts(
                live,
                source.borrowed(),
                wrapper.borrowed(),
                params.pallas.clone(),
                params.vesta.clone(),
                config,
            ),
            "strict import",
        )
    });
    drop((source, wrapper));
    let evidence = report.phase("node_prove", || {
        ok(
            prover.prove(
                live,
                Fq::from(u64::from(seed) + 11).to_repr(),
                randomness(seed.wrapping_add(1)),
                randomness(seed.wrapping_add(2)),
                ProverConfig::default(),
            ),
            "node prove",
        )
    });
    report.value("node_proof_bytes", evidence.proof.len());
    report.phase("node_verify", || {
        ok(prover.verify_evidence(&evidence, budget), "node verify");
    });
    (prover, evidence)
}

fn pick<T>(items: Vec<T>, position: usize, what: &str) -> T {
    let length = items.len();
    items
        .into_iter()
        .nth(position)
        .unwrap_or_else(|| panic!("{what} position {position} >= {length}"))
}

fn leaf(class: &str, position: usize) {
    let mut report = Report::new(class);
    report.value("position", position);
    report.value("load_at_start", load_averages());
    let params = report.phase("parameters_k16", || Parameters {
        pallas: ok(PinnedParams::derive(16), "pallas parameters"),
        vesta: ok(PinnedParams::derive(16), "vesta parameters"),
    });
    let block = block();
    match class {
        "aggregation" => {
            let leaves = report.phase("witness_program", || {
                let key = ok(
                    propose_aggregate_key(&block.roster, &block.bitmap),
                    "aggregate key",
                );
                ok(
                    prepare_aggregation(&block.roster, &block.bitmap, key),
                    "aggregation",
                )
            });
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 41);
        }
        "bls" => {
            let leaves = report.phase("witness_program", || {
                let key = ok(
                    propose_aggregate_key(&block.roster, &block.bitmap),
                    "aggregate key",
                );
                ok(
                    prepare_bls_batches(block.message, key, block.signature),
                    "bls",
                )
            });
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 43);
        }
        "result" => {
            let leaves = report.phase("witness_program", || {
                let expected = core::array::from_fn(|i| block.message[133 + i]);
                ok(prepare_result_batches(&block.frame, expected), "result")
            });
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 47);
        }
        "schedule" => {
            let leaves = report.phase("witness_program", || {
                ok(
                    prepare_schedule_source(block.frame.clone(), false, block.context),
                    "schedule",
                )
            });
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 53);
        }
        "context" => {
            let leaves = report.phase("witness_program", || context_leaves(&block));
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 59);
        }
        "load" => {
            let leaves = report.phase("witness_program", || {
                ok(
                    prepare_load_source(
                        &block.frame,
                        &block.receipt,
                        block.event_root,
                        1,
                        0,
                        &[[0; 32]; 32],
                    ),
                    "load",
                )
            });
            report.value("program_leaves", leaves.len());
            let live = pick(leaves, position, class);
            let frame = ok(leaf_frame_native(live.endpoints()), "frame");
            measure(&mut report, &params, &live, &frame, 61);
        }
        other => panic!("unknown class {other}"),
    }
    report.value("load_at_end", load_averages());
    report.finish();
}

fn context_leaves(
    block: &Block,
) -> Vec<iroha_kagemusha_proof::finality::schedule::context_hash::ContextHashBatchCircuit> {
    let parser = ok(
        prepare_schedule_source(block.frame.clone(), false, block.context),
        "schedule",
    );
    let input = *parser.first().expect("schedule leaf").input();
    ok(
        prepare_context_batches(
            block.frame.clone(),
            input.epoch_hash.payload_start,
            input.epoch_hash.payload_len,
            block.context,
        ),
        "context",
    )
}

/// Two genuine adjacent Context CRC leaves (one shared class), then the real
/// first-level interval merge `IntervalTree` makes for positions 2 and 3.
fn merge() {
    let mut children = Report::new("merge_children");
    let params = children.phase("parameters_k16", || Parameters {
        pallas: ok(PinnedParams::derive(16), "pallas parameters"),
        vesta: ok(PinnedParams::derive(16), "vesta parameters"),
    });
    let block = block();
    let mut leaves = children
        .phase("witness_program", || context_leaves(&block))
        .into_iter()
        .skip(2);
    let left = leaves.next().expect("left leaf");
    let right = leaves.next().expect("right leaf");
    drop(leaves);
    let frame = ok(leaf_frame_native(left.endpoints()), "frame");
    let (prover, left_proof) = measure(&mut children, &params, &left, &frame, 67);
    let right_proof = children.phase("second_leaf_node_prove", || {
        ok(
            prover.prove(
                &right,
                Fq::from(79).to_repr(),
                randomness(80),
                randomness(81),
                ProverConfig::default(),
            ),
            "second leaf",
        )
    });
    children.finish();
    let child = ok(prover.qualified_source(), "child source");
    drop(prover);

    let mut report = Report::new("merge");
    report.value("load_at_start", load_averages());
    let plan = ok(
        SourcePairPlan::new([child.clone(), child], &params.pallas),
        "pair plan",
    );
    let live = report.phase("merge_prepare_native_child_verify_and_fold", || {
        ok(
            SourceMergeCircuit::prepare(
                plan,
                [left_proof, right_proof],
                &params.vesta,
                Fp::from(83),
                &FoldConfig::default(),
            ),
            "merge prepare",
        )
    });
    let frame = *live.instances();
    // `measure` generates keys from the witnessless layout, as the compiler does
    // from `SourceMergeCircuit::for_source`; the import checks the live layout.
    measure(&mut report, &params, &live, &frame, 89);
    report.value("load_at_end", load_averages());
    report.finish();
}

// ---------------------------------------------------------------------------
// Fixed traversal from the compiled leaf schedule, and per-height counts.

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.finality.compiled_leaf_program.v1")]
struct ProgramRecord {
    program: u64,
    semantic_end: u32,
    classes: Vec<u32>,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.finality.compiled_leaf_programs.v1")]
struct Policy {
    version: u16,
    programs: Vec<ProgramRecord>,
}

/// Count distinct keys of the balanced tree `IntervalTree` builds: a merge's
/// key is fixed by its ordered pair of child keys.
fn symbolic_nodes(classes: &[u32]) -> (usize, usize) {
    let mut ids = BTreeMap::<(u64, u64), u64>::new();
    let mut level: Vec<u64> = classes.iter().map(|class| u64::from(*class)).collect();
    let leaf_classes = classes
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut next_id = 1_u64 << 32;
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|pair| {
                if let [left, right] = pair {
                    *ids.entry((*left, *right)).or_insert_with(|| {
                        next_id += 1;
                        next_id
                    })
                } else {
                    pair[0]
                }
            })
            .collect();
    }
    (leaf_classes, ids.len())
}

fn counts() {
    let names = ["Bls", "Aggregation", "Result", "Schedule", "Context", "Load"];
    let bytes = ok(compiled_leaf_schedule_transcript(), "leaf transcript");
    let policy: Policy = ok(norito::decode_canonical(&bytes), "decode leaf transcript");
    assert_eq!(policy.version, 1);
    assert_eq!(policy.programs.len(), names.len());
    let mut nodes = BTreeMap::new();
    let mut unique_total = 0;
    let mut unique_leaf_total = 0;
    for (name, record) in names.iter().zip(&policy.programs) {
        let leaves = record.classes.len();
        let (leaf_classes, merges) = symbolic_nodes(&record.classes);
        unique_total += leaf_classes + merges;
        unique_leaf_total += leaf_classes;
        nodes.insert(*name, 2 * leaves - 1);
        println!(
            "FINALITY_TRAVERSAL program={name} program_id={:#x} semantic_end={} leaves={leaves} uniqueLeafClasses={leaf_classes} uniqueMerges={merges} symbolicSourceNodes={} provedNodesPerUse={} sourceAndWrapperProofsPerUse={}",
            record.program,
            record.semantic_end,
            leaf_classes + merges,
            2 * leaves - 1,
            2 * (2 * leaves - 1),
        );
    }
    // `InstalledFinality::append_block`: certify_result proves Aggregation, BLS,
    // Certificate, Result and CertifiedResult; prove_schedule runs twice (parser,
    // Context, Schedule composition), the second reusing the identical Context
    // proof when no epoch boundary changes its input; then ScheduledResult,
    // HistoryStep and the history Append wrapper.
    let fixed_per_height = 7;
    let normal = nodes["Aggregation"]
        + nodes["Bls"]
        + nodes["Result"]
        + 2 * nodes["Schedule"]
        + nodes["Context"]
        + fixed_per_height;
    let boundary = normal + nodes["Context"];
    // `prove_receipt`: the Load program and the Receipt composition.
    let receipt = nodes["Load"] + 1;
    println!(
        "FINALITY_PER_HEIGHT normal_height_nodes={normal} normal_height_k16_proofs={} boundary_height_nodes={boundary} boundary_height_k16_proofs={} receipt_nodes={receipt} receipt_k16_proofs={} genesis_nodes=1",
        2 * normal,
        2 * boundary,
        2 * receipt,
    );
    // Six compositions, Genesis and Append; Genesis/Append share one history wrapper.
    let compositions = 6;
    let catalog_nodes = unique_total + compositions + 2;
    let catalog_artifacts = 2 * (unique_total + compositions) + 3;
    println!(
        "FINALITY_CATALOG unique_leaf_classes={unique_leaf_total} unique_program_nodes={unique_total} unique_nodes_with_compositions_and_history={catalog_nodes} original_artifacts={catalog_artifacts}"
    );
}

/// Exact original source PK length of one leaf class from its real descriptor:
/// `PIPAPK01 || digest || LE32 || vk || copy digest || (fixed + permutation) * n * 32`.
/// `keygen_vk_with_binding_v2` yields the same descriptor as `keygen_pk_v2`
/// without building proving tables.
fn source_pk_bytes<C: Circuit<Fp>>(params: &Parameters, live: &C) -> (usize, u32, usize) {
    let (binding, key) = ok(
        keygen_vk_with_binding_v2(
            &params.vesta,
            &live.without_witnesses(),
            &key_config(vec![InstanceType::Bounded], true),
        ),
        "source descriptor",
    );
    let descriptor = binding.descriptor();
    let columns = descriptor.num_fixed_columns as usize + descriptor.permutation.len();
    let bytes = 8 + 32 + 4 + key.to_bytes().len() + 32 + columns * binding.n() * 32;
    (bytes, descriptor.num_fixed_columns, descriptor.permutation.len())
}

fn class_sizes<C: Circuit<Fp> + Sync>(
    name: &str,
    params: &Parameters,
    classes: &[u32],
    leaves: &[C],
) -> usize {
    use rayon::prelude::*;
    let unique: Vec<u32> = classes
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let sizes: Vec<(u32, (usize, u32, usize))> = unique
        .par_iter()
        .map(|class| {
            (
                *class,
                source_pk_bytes(params, &leaves[usize::try_from(*class).unwrap()]),
            )
        })
        .collect();
    let mut shapes = BTreeMap::<(u32, usize, usize), usize>::new();
    for (_, (bytes, fixed, permutation)) in &sizes {
        *shapes.entry((*fixed, *permutation, *bytes)).or_default() += 1;
    }
    let total: usize = sizes.iter().map(|(_, (bytes, _, _))| bytes).sum();
    for ((fixed, permutation, bytes), count) in shapes {
        println!(
            "FINALITY_LEAF_SHAPE program={name} classes={count} fixed={fixed} permutation={permutation} source_pk_bytes={bytes}"
        );
    }
    println!(
        "FINALITY_LEAF_FOOTPRINT program={name} unique_leaf_classes={} source_pk_bytes_total={total}",
        sizes.len()
    );
    total
}

/// Exact source PK bytes of every unique leaf class of the six programs.
fn footprint() {
    let params = Parameters {
        pallas: ok(PinnedParams::derive(16), "pallas parameters"),
        vesta: ok(PinnedParams::derive(16), "vesta parameters"),
    };
    let bytes = ok(compiled_leaf_schedule_transcript(), "leaf transcript");
    let policy: Policy = ok(norito::decode_canonical(&bytes), "decode leaf transcript");
    let block = block();
    let key = ok(
        propose_aggregate_key(&block.roster, &block.bitmap),
        "aggregate key",
    );
    let classes = |index: usize| policy.programs[index].classes.as_slice();
    let expected = core::array::from_fn(|i| block.message[133 + i]);
    let mut total = 0;
    total += class_sizes(
        "Bls",
        &params,
        classes(0),
        &ok(
            prepare_bls_batches(block.message, key, block.signature),
            "bls",
        ),
    );
    total += class_sizes(
        "Aggregation",
        &params,
        classes(1),
        &ok(
            prepare_aggregation(&block.roster, &block.bitmap, key),
            "aggregation",
        ),
    );
    total += class_sizes(
        "Result",
        &params,
        classes(2),
        &ok(prepare_result_batches(&block.frame, expected), "result"),
    );
    total += class_sizes(
        "Schedule",
        &params,
        classes(3),
        &ok(
            prepare_schedule_source(block.frame.clone(), false, block.context),
            "schedule",
        ),
    );
    total += class_sizes("Context", &params, classes(4), &context_leaves(&block));
    total += class_sizes(
        "Load",
        &params,
        classes(5),
        &ok(
            prepare_load_source(
                &block.frame,
                &block.receipt,
                block.event_root,
                1,
                0,
                &[[0; 32]; 32],
            ),
            "load",
        ),
    );
    println!("FINALITY_LEAF_FOOTPRINT program=all source_pk_bytes_total={total}");
}

/// Native comparison on the same captured CommitQC: committee proof-of-possession
/// admission, the quorum aggregate BLS verification the native light client
/// performs (`ProofCrypto::verify_aggregate_multi`) and the result-frame hash.
fn native() {
    use iroha_crypto::{
        Algorithm, BlsNormalPopVerifiedKey, Hash, PublicKey,
        bls_normal_verify_preaggregated_multi_message,
    };
    let json: Value = ok(norito::json::from_str(FIXTURE), "fixture JSON");
    let block = block();
    let pops: Vec<Vec<u8>> = field(&json, "committee_proofs_of_possession_hex")
        .as_array()
        .expect("PoPs")
        .iter()
        .map(|pop| hex(pop.as_str().expect("PoP")))
        .collect();
    let keys: Vec<PublicKey> = block
        .roster
        .iter()
        .map(|key| ok(PublicKey::from_bytes(Algorithm::BlsNormal, key.as_slice()), "BLS key"))
        .collect();
    let runs = 200_u32;
    let timed = |name: &str, run: &mut dyn FnMut()| {
        run();
        let start = Instant::now();
        for _ in 0..runs {
            run();
        }
        let each = start.elapsed().as_secs_f64() / f64::from(runs);
        println!(
            "FINALITY_NATIVE op={name} mean_ms={:.3} runs={runs} threads={}",
            each * 1e3,
            rayon::current_num_threads()
        );
    };
    let load_start = load_averages();
    timed("pop_admit_committee", &mut || {
        for (key, pop) in keys.iter().zip(&pops) {
            ok(BlsNormalPopVerifiedKey::new(key, pop), "PoP");
        }
    });
    let admitted: Vec<BlsNormalPopVerifiedKey> = keys
        .iter()
        .zip(&pops)
        .map(|(key, pop)| ok(BlsNormalPopVerifiedKey::new(key, pop), "PoP"))
        .collect();
    let signers: Vec<&BlsNormalPopVerifiedKey> = admitted
        .iter()
        .enumerate()
        .filter(|(index, _)| (block.bitmap[index / 8] >> (index % 8)) & 1 == 1)
        .map(|(_, key)| key)
        .collect();
    timed("commit_qc_aggregate_verify", &mut || {
        ok(
            bls_normal_verify_preaggregated_multi_message(
                &[(signers.as_slice(), block.message.as_slice())],
                &block.signature,
            ),
            "CommitQC",
        );
    });
    timed("result_frame_blake2b", &mut || {
        let _digest = Hash::new(&block.frame);
    });
    // The light-client path phones already use for committed-transaction inclusion:
    // import a pinned checkpoint, re-verifying its tip proof (committee PoPs,
    // CommitQC, schedule, availability) through `SumeragiFinalityVerifier`.
    {
        use iroha_data_model::sumeragi_finality::{
            SumeragiFinalityCheckpoint, SumeragiFinalityVerifier,
        };
        const CHECKPOINT: &[u8] =
            include_bytes!("../../../fixtures/sumeragi/native-finality/height-2-checkpoint.nrt");
        let checkpoint = ok(
            SumeragiFinalityCheckpoint::decode_canonical(CHECKPOINT),
            "checkpoint",
        );
        let network = checkpoint.network_id();
        let chain = checkpoint.chain_id().to_owned();
        let proof = ok(norito::to_bytes(checkpoint.tip()), "tip proof bytes");
        println!(
            "FINALITY_NATIVE checkpoint_bytes={} tip_height={} tip_proof_bytes={} tip_block_wire_bytes={} committee={}",
            CHECKPOINT.len(),
            checkpoint.height(),
            proof.len(),
            checkpoint.tip().block_wire.len(),
            checkpoint.tip().committee.len()
        );
        timed("checkpoint_import_with_tip_reverification", &mut || {
            ok(
                SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
                    &checkpoint,
                    &network,
                    &chain,
                ),
                "checkpoint import",
            );
        });
    }
    let sizes = [
        "consensus_header_frame_hex",
        "commit_qc_frame_hex",
        "result_preimage_hex",
        "receipt_frame_hex",
        "event_box_frame_hex",
        "signed_genesis_wire_hex",
    ]
    .map(|name| format!("{name}={}", bytes(&json, name).len()));
    println!(
        "FINALITY_NATIVE signers={} committee={} {} load_start={load_start} load_end={}",
        signers.len(),
        keys.len(),
        sizes.join(" "),
        load_averages()
    );
}

fn main() {
    assert!(
        !cfg!(debug_assertions),
        "finality_node_cost measures only optimized builds; use --release"
    );
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let class = arguments.first().map_or("counts", String::as_str);
    let position = arguments
        .get(1)
        .map_or(0, |text| text.parse().expect("leaf position"));
    match class {
        "counts" => counts(),
        "footprint" => footprint(),
        "native" => native(),
        "merge" => merge(),
        _ => leaf(class, position),
    }
}
