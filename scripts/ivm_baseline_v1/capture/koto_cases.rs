//! Kotodama programs compiled with the anchor's own compiler.
//!
//! Sources come from the anchor export (its samples, examples and interpreter fixtures) and from
//! the capture-owned programs under `scripts/ivm_baseline_v1/kotodama`. Each compiled artifact
//! yields one case per public entrypoint; `runs.json` adds cases that pass arguments or use a
//! particular host.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use iroha_data_model::smart_contract::manifest::EntryPointKind;
use iroha_primitives::json::Json;
use ivm::{AssetDefinitionId, PointerType, ProgramMetadata};
use kotodama_lang::compiler::{Compiler, CompilerOptions};
use norito::json::Value;

use crate::oracle::{Case, Gas, HostKind, Policy, Step, fixture_account, tlv};

/// Anchor directories whose `.ko` files join the corpus, with the identifier prefix of each.
const ANCHOR_SOURCE_DIRS: &[(&str, &str)] = &[
    ("crates/kotodama_lang/src/samples", "samples"),
    ("crates/ivm/tests/data", "ivm_tests_data"),
    ("crates/ivm/fixtures/koto_v1/kotodama", "koto_v1/kotodama"),
    (
        "crates/ivm/fixtures/koto_v1/kotodama_lists",
        "koto_v1/lists",
    ),
    (
        "crates/ivm/fixtures/koto_v1/kotodama_state_aggregate_literal_runtime",
        "koto_v1/state_aggregate_literal_runtime",
    ),
    (
        "crates/ivm/fixtures/koto_v1/kotodama_state_name_map_runtime",
        "koto_v1/state_name_map_runtime",
    ),
    (
        "crates/ivm/fixtures/koto_v1/kotodama_state_scalar",
        "koto_v1/state_scalar",
    ),
    (
        "crates/ivm/fixtures/koto_v1/kotodama_v1_runtime_acceptance",
        "koto_v1/runtime_acceptance",
    ),
    ("examples/hello", "examples"),
    ("examples/map", "examples"),
    ("examples/nft", "examples"),
    ("examples/transfer", "examples"),
    ("examples/coffee-club/contracts", "examples"),
];
/// Sources also compiled with `CompilerOptions::force_zk`, which sets the header's ZK mode bit;
/// the resulting artifacts are identified as `zk/<source id>`.
const ZK_VARIANTS: &[&str] = &[
    "samples/zk_vote_ballot",
    "samples/tuple_return_demo",
    "koto_v1/runtime_acceptance/001",
    "baseline/numeric",
];
/// Identifier prefixes whose entrypoints also run under the ledger mock host.
const LEDGER_PREFIXES: &[&str] = &["samples/", "examples/"];

/// One Kotodama source of the corpus.
pub struct Source {
    /// Corpus identifier, also the fixture file stem.
    pub id: String,
    /// Where the text came from (anchor-relative path or `capture:<file>`).
    pub origin: String,
    /// Source text.
    pub text: String,
}

/// A source the anchor compiler refuses.
pub struct Rejection {
    /// Corpus identifier.
    pub id: String,
    /// The compiler's diagnostic.
    pub error: String,
}

/// A compiled source.
pub struct Compiled {
    /// Corpus identifier.
    pub id: String,
    /// Complete artifact.
    pub artifact: Vec<u8>,
}

fn files_with_suffix(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(suffix))
        })
        .collect();
    files.sort();
    files
}

/// Collect the corpus sources in a stable order.
pub fn discover(anchor_root: &Path, capture_dir: &Path) -> Vec<Source> {
    let mut sources = Vec::new();
    for (dir, prefix) in ANCHOR_SOURCE_DIRS {
        for path in files_with_suffix(&anchor_root.join(dir), ".ko") {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("UTF-8 source file name");
            sources.push(Source {
                id: format!("{prefix}/{stem}"),
                origin: format!(
                    "{dir}/{}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .expect("UTF-8 source file name")
                ),
                text: fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
            });
        }
    }
    // Capture-owned sources are stored as `.ko.txt`: the live repository formats and compiles
    // every tracked `.ko` file with the current compiler, and these target the anchor's.
    for path in files_with_suffix(capture_dir, ".ko.txt") {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("UTF-8 source file name");
        let stem = name.strip_suffix(".ko.txt").expect("a .ko.txt file");
        sources.push(Source {
            id: format!("baseline/{stem}"),
            origin: format!("capture:{name}"),
            text: fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    for source in &sources {
        assert!(
            seen.insert(source.id.clone()),
            "duplicate source id {}",
            source.id
        );
    }
    sources
}

/// Compile every source twice with the anchor compiler and require identical bytes.
pub fn compile(sources: &[Source]) -> (Vec<Compiled>, Vec<Rejection>) {
    let mut compiled = Vec::new();
    let mut rejected = Vec::new();
    let mut record = |id: String, compiler: &dyn Fn() -> Compiler, text: &str| {
        let first = compiler().compile_source(text);
        let second = compiler().compile_source(text);
        assert_eq!(
            first, second,
            "the anchor compiler is not deterministic for {id}"
        );
        match first {
            Ok(artifact) => compiled.push(Compiled { id, artifact }),
            Err(error) => rejected.push(Rejection { id, error }),
        }
    };
    for source in sources {
        record(source.id.clone(), &Compiler::new, &source.text);
        if ZK_VARIANTS.contains(&source.id.as_str()) {
            record(
                format!("zk/{}", source.id),
                &|| {
                    Compiler::new_with_options(CompilerOptions {
                        force_zk: true,
                        ..CompilerOptions::default()
                    })
                },
                &source.text,
            );
        }
    }
    (compiled, rejected)
}

fn artifact_path(id: &str) -> String {
    format!("artifacts/kotodama/{id}.to.bin")
}

fn base_case(compiled: &Compiled, entrypoint: Option<&str>, host: HostKind, suffix: &str) -> Case {
    let host_name = match host {
        HostKind::Default => "default",
        HostKind::Core => "core",
        HostKind::Wsv => "wsv",
        HostKind::WsvContract => "wsv_contract",
    };
    let selector = entrypoint.unwrap_or("<generic>");
    let mut id = format!("kotodama/{}#{selector}@{host_name}", compiled.id);
    if !suffix.is_empty() {
        id.push('+');
        id.push_str(suffix);
    }
    Case {
        id,
        artifact: artifact_path(&compiled.id),
        program: compiled.artifact.clone(),
        covers: vec!["kotodama".to_owned()],
        gas: Gas::FromHeader,
        host,
        public_inputs: Vec::new(),
        arguments_json: None,
        prelude: Vec::new(),
        steps: entrypoint
            .map(|name| Step::SelectEntrypoint {
                name: name.to_owned(),
            })
            .into_iter()
            .collect(),
        policy: Policy::anchor_default(),
        cycle_budget: None,
        traced: true,
    }
}

fn host_kind(name: &str) -> HostKind {
    match name {
        "default" => HostKind::Default,
        "core" => HostKind::Core,
        "wsv" => HostKind::Wsv,
        "wsv_contract" => HostKind::WsvContract,
        other => panic!("unknown host `{other}` in runs.json"),
    }
}

fn text<'a>(run: &'a norito::json::Map, key: &str) -> Option<&'a str> {
    run.get(key).and_then(Value::as_str)
}

/// A deterministic argument of the named Kotodama parameter type, varied by position so that
/// generated operands differ, or `None` for a type the capture has no default for.
fn default_argument(type_name: &str, position: usize) -> Option<Value> {
    const INTS: [&str; 6] = ["7", "3", "2", "5", "11", "13"];
    const QUANTITIES: [&str; 6] = ["5", "3", "2", "8", "10", "4"];
    const DECIMALS: [&str; 4] = ["1.5", "2.25", "0.5", "1.25"];
    let text = |value: &str| Value::String(value.to_owned());
    Some(match type_name {
        "int" => text(INTS[position % INTS.len()]),
        "quantity" => text(QUANTITIES[position % QUANTITIES.len()]),
        "decimal" => text(DECIMALS[position % DECIMALS.len()]),
        "bool" => Value::Bool(true),
        "Name" => text("alpha"),
        "bytes" => text("0x0102030405060708"),
        "DataSpaceId" => Value::Number(norito::json::Number::U64(0)),
        "AccountId" => Value::String(fixture_account().to_string()),
        "AssetDefinitionId" => {
            // A version-4 UUID that differs per parameter position.
            let mut uuid = [0x11_u8; 16];
            uuid[0] = u8::try_from(position).ok()?.checked_add(1)?;
            uuid[6] = 0x42;
            uuid[8] = 0x83;
            Value::String(
                AssetDefinitionId::from_uuid_bytes(uuid)
                    .ok()?
                    .canonical_address(),
            )
        }
        _ => return None,
    })
}

/// Canonical JSON and argument record of generated arguments for `entry`, if all its parameter
/// types have defaults and the anchor encoder accepts them.
fn default_arguments(entry: &ivm::EmbeddedEntrypointDescriptor) -> Option<(String, Vec<u8>)> {
    let schema = entry.argument_schema.as_ref()?;
    let mut arguments = norito::json::Map::new();
    for (position, parameter) in entry.params.iter().enumerate() {
        arguments.insert(
            parameter.name.clone(),
            default_argument(&parameter.type_name, position)?,
        );
    }
    let canonical = norito::json::to_string(&Value::Object(arguments)).ok()?;
    let payload = Json::from_str_norito(&canonical).ok()?;
    let record = ivm_abi::arguments::encode_argument_record_from_json(schema, &payload).ok()?;
    Some((canonical, record))
}

/// Serve `record` to the case as the entrypoint argument record public input.
fn attach_arguments(case: &mut Case, canonical: String, record: &[u8]) {
    case.public_inputs = vec![(
        ivm_abi::arguments::ENTRYPOINT_ARGUMENT_RECORD_PUBLIC_INPUT_KEY.to_owned(),
        tlv(PointerType::NoritoBytes, record),
    )];
    case.arguments_json = Some(canonical);
    if !case.covers.iter().any(|tag| tag == "arguments:record") {
        case.covers.push("arguments:record".to_owned());
    }
}

/// Cases of every compiled artifact plus the explicit runs of `runs.json`.
pub fn build(compiled: &[Compiled], runs_file: &Path) -> Vec<Case> {
    let mut cases = Vec::new();
    for artifact in compiled {
        let parsed = ProgramMetadata::parse(&artifact.artifact)
            .unwrap_or_else(|error| panic!("parse {}: {error:?}", artifact.id));
        let Some(interface) = parsed.contract_interface else {
            cases.push(base_case(artifact, None, HostKind::Core, ""));
            continue;
        };
        let initialiser = interface
            .entrypoints
            .iter()
            .find(|entry| entry.kind == EntryPointKind::Hajimari)
            .map(|entry| entry.name.clone());
        let ledger = LEDGER_PREFIXES
            .iter()
            .any(|prefix| artifact.id.starts_with(prefix));
        for entry in &interface.entrypoints {
            if entry.argument_schema.is_some() {
                // A parameterised entrypoint runs with generated arguments where every parameter
                // type has a default; otherwise its root call fails for want of a record.
                if let Some((canonical, record)) = default_arguments(entry) {
                    let host = if ledger {
                        HostKind::Wsv
                    } else {
                        HostKind::Default
                    };
                    let mut case =
                        base_case(artifact, Some(&entry.name), host, "default_arguments");
                    attach_arguments(&mut case, canonical, &record);
                    cases.push(case);
                } else {
                    cases.push(base_case(
                        artifact,
                        Some(&entry.name),
                        HostKind::Core,
                        "without_arguments",
                    ));
                }
                continue;
            }
            cases.push(base_case(artifact, Some(&entry.name), HostKind::Core, ""));
            if ledger {
                cases.push(base_case(artifact, Some(&entry.name), HostKind::Wsv, ""));
            }
            // State initialised by `hajimari` persists in the host across invocations.
            if let Some(initialiser) = &initialiser
                && *initialiser != entry.name
            {
                let mut case = base_case(
                    artifact,
                    Some(&entry.name),
                    HostKind::Core,
                    "after_hajimari",
                );
                case.prelude = vec![initialiser.clone()];
                cases.push(case);
            }
        }
    }
    let by_id: BTreeMap<&str, &Compiled> = compiled
        .iter()
        .map(|artifact| (artifact.id.as_str(), artifact))
        .collect();
    let runs_text = fs::read_to_string(runs_file)
        .unwrap_or_else(|error| panic!("read {}: {error}", runs_file.display()));
    let runs: Value = norito::json::from_str(&runs_text).expect("runs.json is Norito JSON");
    for run in runs.as_array().expect("runs.json is an array") {
        let run = run.as_object().expect("a run is an object");
        let source = text(run, "source").expect("run.source");
        let entrypoint = text(run, "entrypoint").expect("run.entrypoint");
        let suffix = text(run, "id").expect("run.id");
        let host = host_kind(text(run, "host").expect("run.host"));
        let artifact = by_id
            .get(source)
            .unwrap_or_else(|| panic!("runs.json names `{source}`, which did not compile"));
        let mut case = base_case(artifact, Some(entrypoint), host, suffix);
        if let Some(tags) = run.get("covers").and_then(Value::as_array) {
            case.covers
                .extend(tags.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        if let Some(arguments) = run.get("arguments") {
            let interface = ProgramMetadata::parse(&artifact.artifact)
                .expect("compiled artifact parses")
                .contract_interface
                .expect("compiled contract interface");
            let entry = interface
                .entrypoints
                .iter()
                .find(|entry| entry.name == entrypoint)
                .unwrap_or_else(|| panic!("`{source}` has no entrypoint `{entrypoint}`"));
            let schema = entry
                .argument_schema
                .as_ref()
                .unwrap_or_else(|| panic!("`{source}::{entrypoint}` takes no arguments"));
            let canonical = norito::json::to_string(arguments).expect("canonical arguments JSON");
            let payload = Json::from_str_norito(&canonical)
                .unwrap_or_else(|error| panic!("arguments of {}: {error}", case.id));
            let record = ivm_abi::arguments::encode_argument_record_from_json(schema, &payload)
                .unwrap_or_else(|error| panic!("argument record of {}: {error:?}", case.id));
            attach_arguments(&mut case, canonical, &record);
        }
        cases.push(case);
    }
    cases
}
