//! Original evidence owner and explicit isolated Cargo invocation for the ignored rehearsal.

use super::*;
use norito::json::{JsonSerialize, Value};
use std::{
    fs::{self, File},
    io::{BufRead as _, BufReader},
    process::Command,
};

#[derive(JsonSerialize)]
struct Edit {
    path: &'static str,
    before: String,
    after: String,
}

#[derive(JsonSerialize)]
struct StageRequest {
    inputs: Vec<String>,
    pinned: BTreeMap<String, Value>,
    reviewed: BTreeMap<String, String>,
    omit: Vec<String>,
    edits: Vec<Edit>,
}

fn hooks() -> Vec<Edit> {
    let native =
        "    pub fn native_module(&self, implementation: &DefinitionId) -> Option<&str> {\n";
    let support = "    pub(crate) fn executes_definition(&self, id: &DefinitionId) -> bool {\n";
    let reject = |key: &str, result: &str| {
        format!(
            "        if matches!(self.definitions.get({key}),\n            Some(DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {{\n                code: ImplementationCodeV1::Native {{ module, .. }}, ..\n            }})) if module == \"iroha.account.signing_key.v1\") {{\n            return {result};\n        }}\n"
        )
    };
    vec![
        Edit { path: READER, before: "mod execute;\n".into(), after: String::new() },
        Edit { path: DISPATCH, before: "    dispatch_instruction::<iroha_data_model::isi::account_signing_key::SetAccountSigningKeyV1> => CoreAuthorized [asset_effect = NoNumericAssetEffect],\n".into(), after: String::new() },
        Edit { path: CATALOG, before: native.into(), after: format!("{native}{}", reject("implementation", "None")) },
        Edit { path: SUPPORT, before: support.into(), after: format!("{support}{}", reject("id", "false")) },
    ]
}

fn helper(root: &Path) -> Command {
    let mut command = Command::new("python3");
    command.arg(root.join("scripts/runtime_source_removal_candidate.py"));
    command
}

fn invoke(root: &Path, candidate: &Path, request: &Path, action: &str) {
    assert!(
        helper(root)
            .arg(action)
            .arg(root)
            .arg(candidate)
            .arg(request)
            .status()
            .unwrap()
            .success(),
        "exact source {action} refused"
    );
}

fn archive_sources(root: &Path, archive: &Path) {
    for (name, _, _) in REVIEWED {
        let target = archive.join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(root.join(name), target).unwrap();
    }
}

fn root_input(chain: &CertifiedTestChain) -> RootInput {
    let tip = chain.committed(chain.height());
    let floor = crate::runtime::checkpoint::history_boundary(
        &chain.state().world_view(),
        tip.block_time_ms(),
    )
    .expect("the actual committed effective floor exists");
    RootInput {
        network: chain.network_id(),
        floor: floor.height,
        tip: chain.height(),
        tip_hash: *tip.block_hash().as_ref(),
    }
}

pub(super) fn rehearse() {
    // Refuse before an expensive build on hosts without qualified original-descriptor
    // execution. Retaining a handle while reopening a pathname would not bind the child.
    Arc::new(iroha_fs::RetainedBuildInput::open(std::env::current_exe().unwrap()).unwrap())
        .command()
        .expect("native original-descriptor process launch is required; no pathname fallback");
    let root = source_root().canonicalize().unwrap();
    assert!(
        root.join(HANDLER).is_file(),
        "only the original build can authorize a candidate"
    );
    let directory = fixture_dir();
    assert!(!directory.starts_with(&root));
    fs::create_dir(&directory)
        .expect("rehearsal output must be new; old results never resume authority");
    let target = PathBuf::from(
        std::env::var_os("IROHA_N14_SOURCE_REMOVAL_TARGET_DIR")
            .expect("root-authorized fixed warm candidate build lane is required"),
    );
    assert!(target.is_absolute() && !target.starts_with(&directory) && !target.starts_with(&root));
    assert!(
        target.is_dir(),
        "use an explicitly provisioned stable lane, not an accidental fresh target"
    );
    let (owner, aliases) = reviewed_aliases();
    let (manifest, request) = manifest(owner);
    let sources = source_ownership::check_sources(
        &root,
        &manifest,
        source_ownership::manifest_digest(&manifest).unwrap(),
        &request,
        read_limits(),
    )
    .unwrap();
    let archive = directory.join("reviewed-originals");
    archive_sources(&root, &archive);
    let mut parent = CertifiedTestChain::start(parent_config()).unwrap();
    let home = crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap();
    let mut child_config = child_fixture::config_for_root(parent.network_id(), home);
    child_config.genesis_parameters.push(checkpoint_policy());
    let mut child = CertifiedTestChain::start(child_config).unwrap();
    let parent_change = retirement_change(&parent, &aliases);
    let child_change = retirement_change(&child, &aliases);
    activate_parent(&mut parent, parent_change);
    activate_child(&mut child, child_change);
    for name in ["parent", "child"] {
        fs::create_dir_all(directory.join(name).join("snapshots")).unwrap();
        fs::create_dir(directory.join(name).join("kura")).unwrap();
    }
    retire_with_checkpoint(&mut parent, &aliases, |chain| {
        keep(chain, &directory.join("parent/snapshots"))
    });
    retire_with_checkpoint(&mut child, &aliases, |chain| {
        keep(chain, &directory.join("child/snapshots"))
    });
    // Both stores retain a newer snapshot. Only the actual effective floor is acceptable.
    keep(&child, &directory.join("child/snapshots"));
    advance(&mut child);
    let registration = PrivateDataspaceRegistration::new(
        child.root_scope(),
        child.state().query_view().chain_id().clone(),
        child.network_id(),
        child.committed(1).result().0,
        authenticated_genesis(child.genesis())
            .unwrap()
            .into_parts()
            .0,
    )
    .unwrap();
    let mut instructions = vec![
        RegisterPrivateDataspace {
            alias: ALIAS.into(),
            expected_ownership_generation: 1,
            registration: norito::encode_canonical(&registration).unwrap(),
        }
        .into(),
    ];
    instructions.extend(
        (2..=child.height()).map(|height| anchor_instruction(&child, &registration, height)),
    );
    submit(&mut parent, instructions);
    keep(&parent, &directory.join("parent/snapshots"));
    advance(&mut parent);
    let inputs = Inputs {
        parent: root_input(&parent),
        child: root_input(&child),
    };
    fs::write(
        directory.join("roots.json"),
        norito::json::to_vec(&inputs).unwrap(),
    )
    .unwrap();
    let root_input_file = iroha_fs::RetainedBuildInput::open(directory.join("roots.json")).unwrap();
    let root_input_snapshot = root_input_file.snapshot().unwrap();
    for (name, chain) in [("parent", &parent), ("child", &child)] {
        copy_store_files(
            &chain.kura().store_root(),
            &directory.join(name).join("kura"),
        );
    }
    let parent_state = parent.state().clone();
    let child_state = child.state().clone();
    let parent_cut = parent_state.runtime_retirement_cut().unwrap();
    let child_cuts = [child_state.runtime_retirement_cut().unwrap()];
    let parent_root = original_root(parent_cut.view()).unwrap();
    let evidence = verify_source_candidate(&sources, &parent_cut, &child_cuts, &mut |id, root| {
        let state = if root == parent_root {
            &parent_state
        } else {
            assert_eq!(root, original_root(child_cuts[0].view()).unwrap());
            &child_state
        };
        Ok(crate::runtime::retirement::receipt_proof(&state.view(), id).unwrap())
    })
    .unwrap();
    assert_eq!(evidence.roots(), 2);

    let pins_path = directory.join("build-inputs.json");
    assert!(
        helper(&root)
            .arg("capture")
            .arg(&root)
            .arg(&pins_path)
            .status()
            .unwrap()
            .success()
    );
    let pinned: BTreeMap<String, Value> =
        norito::json::from_slice(&fs::read(&pins_path).unwrap()).unwrap();
    let staged = StageRequest {
        inputs: pinned.keys().cloned().collect(),
        pinned,
        reviewed: REVIEWED
            .iter()
            .map(|(name, _, hash)| ((*name).into(), (*hash).into()))
            .collect(),
        omit: request.paths.clone(),
        edits: hooks(),
    };
    let request_path = directory.join("stage-request.json");
    fs::write(&request_path, norito::json::to_vec(&staged).unwrap()).unwrap();
    let request_file = iroha_fs::RetainedBuildInput::open(&request_path).unwrap();
    let request_snapshot = request_file.snapshot().unwrap();
    let recheck_inputs = || {
        root_input_file.revalidate().unwrap();
        request_file.revalidate().unwrap();
        assert_eq!(root_input_file.snapshot().unwrap(), root_input_snapshot);
        assert_eq!(request_file.snapshot().unwrap(), request_snapshot);
    };
    let candidate = directory.join("candidate");
    recheck_inputs();
    invoke(&root, &candidate, &request_path, "stage");
    recheck_inputs();
    evidence.recheck_sources().unwrap();
    invoke(&root, &candidate, &request_path, "verify");

    let build_log = directory.join("candidate-build.jsonl");
    let stderr = File::create(directory.join("candidate-build.stderr")).unwrap();
    let status = Command::new(candidate.join("scripts/cargo_fast.sh"))
        .current_dir(&candidate)
        .args(["--stable-local-metadata", "--incremental", "--target-dir"])
        .arg(&target)
        .args([
            "--",
            "test",
            "--locked",
            "-p",
            "iroha_core",
            "--lib",
            "--no-run",
            "--message-format=json",
        ])
        .stdout(File::create(&build_log).unwrap())
        .stderr(stderr)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "isolated compilation failed; inspect retained build logs"
    );
    let mut executables = BTreeSet::new();
    for line in BufReader::new(File::open(&build_log).unwrap()).lines() {
        let line = line.unwrap();
        let Ok(value) = norito::json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("reason").and_then(Value::as_str) == Some("compiler-artifact")
            && value
                .get("target")
                .and_then(|target| target.get("name"))
                .and_then(Value::as_str)
                == Some("iroha_core")
            && let Some(path) = value.get("executable").and_then(Value::as_str)
        {
            executables.insert(PathBuf::from(path));
        }
    }
    assert_eq!(
        executables.len(),
        1,
        "retain the exact current Cargo test executable"
    );
    let executable = executables.into_iter().next().unwrap();
    assert!(executable.starts_with(&target));
    let original_binary = Arc::new(iroha_fs::RetainedBuildInput::open(&executable).unwrap());
    let binary_snapshot = original_binary.snapshot().unwrap();
    evidence.recheck_sources().unwrap();
    recheck_inputs();
    invoke(&root, &candidate, &request_path, "verify");
    let mut process = original_binary.command().unwrap();
    let candidate_directory = Arc::new(iroha_fs::ReaderDirectory::open(&candidate).unwrap());
    process.current_dir_retained(&candidate_directory).unwrap();
    process
        .args([
            CANDIDATE_TEST,
            "--ignored",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("IROHA_N14_SOURCE_REMOVAL_DIR", &directory)
        .stdout(iroha_fs::ProcessStdio::Inherit)
        .stderr(iroha_fs::ProcessStdio::Inherit);
    let status = process.status().unwrap();
    original_binary.revalidate().unwrap();
    assert_eq!(original_binary.snapshot().unwrap(), binary_snapshot);
    evidence.recheck_sources().unwrap();
    recheck_inputs();
    invoke(&root, &candidate, &request_path, "verify");
    assert!(
        status.success(),
        "candidate-linked oldest-floor replay failed"
    );
    // No persisted report, support flag or deletion permission is minted by this test.
}
