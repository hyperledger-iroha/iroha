#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealToolIdentityV1 {
    binary_sha256: String,
    binary_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealToolchainV1 {
    cargo: SourceSealToolIdentityV1,
    rustc: SourceSealToolIdentityV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputTreeV1 {
    bytes: u64,
    files: u64,
    records: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputCargoHomeV1 {
    roots: Vec<String>,
    tree: SourceSealBuildInputTreeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputCargoToolchainV1 {
    cargo_relative_path: String,
    tree: SourceSealBuildInputTreeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputRustToolchainV1 {
    rustc_relative_path: String,
    tree: SourceSealBuildInputTreeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputPathTreeV1 {
    path: String,
    tree: SourceSealBuildInputTreeV1,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputPythonRuntimeV1 {
    interpreter_path: String,
    interpreter_sha256: String,
    root: String,
    tree_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputRuntimeIdentityV1 {
    account_name: String,
    gid: u64,
    group_name: String,
    policy: String,
    uid: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputHostToolV1 {
    binary_sha256: String,
    binary_size_bytes: u64,
    path: String,
    resolved_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputSandboxV1 {
    backend: String,
    os_build: String,
    profile_schema: String,
    qualification: Vec<String>,
    xcode_build: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct SourceSealBuildInputClosureV1 {
    cargo_home: SourceSealBuildInputCargoHomeV1,
    cargo_toolchain: SourceSealBuildInputCargoToolchainV1,
    developer_dir: SourceSealBuildInputPathTreeV1,
    host_tools: Vec<SourceSealBuildInputHostToolV1>,
    platform: String,
    python_runtime: SourceSealBuildInputPythonRuntimeV1,
    rust_toolchain: SourceSealBuildInputRustToolchainV1,
    runtime_identity: SourceSealBuildInputRuntimeIdentityV1,
    sandbox: SourceSealBuildInputSandboxV1,
    schema: String,
    sdkroot: SourceSealBuildInputPathTreeV1,
}

fn canonical_absolute_source_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 4096
        || !value
            .as_bytes()
            .iter()
            .all(|byte| (0x20..=0x7e).contains(byte))
        || !value.starts_with('/')
    {
        return false;
    }
    let components = value.split('/').skip(1).collect::<Vec<_>>();
    components
        .iter()
        .all(|component| !component.is_empty() && *component != "." && *component != "..")
}

fn exact_source_seal_build_input_tree(tree: &SourceSealBuildInputTreeV1) -> bool {
    tree.records > 0
        && tree.records <= 250_000
        && tree.files > 0
        && tree.files <= tree.records
        && tree.bytes >= tree.files
        && tree.bytes <= 64 * 1024 * 1024 * 1024
        && is_nonzero_lower_hex(&tree.sha256, 64)
}

fn exact_source_seal_build_inputs(inputs: &SourceSealBuildInputClosureV1) -> bool {
    let expected_qualification = [
        "deny-ambient-read-v1",
        "deny-ambient-write-v1",
        "deny-network-v1",
        "deny-unlisted-exec-v1",
        "fresh-cargo-rustc-link-v1",
    ];
    inputs.schema == SOURCE_SEAL_BUILD_INPUT_CLOSURE_SCHEMA
        && inputs.platform == "darwin"
        && inputs
            .cargo_home
            .roots
            .iter()
            .map(String::as_str)
            .eq(["git", "registry"])
        && exact_source_seal_build_input_tree(&inputs.cargo_home.tree)
        && inputs.cargo_toolchain.cargo_relative_path == "bin/cargo"
        && exact_source_seal_build_input_tree(&inputs.cargo_toolchain.tree)
        && inputs.rust_toolchain.rustc_relative_path == "bin/rustc"
        && exact_source_seal_build_input_tree(&inputs.rust_toolchain.tree)
        && canonical_absolute_source_path(&inputs.python_runtime.root)
        && canonical_absolute_source_path(&inputs.python_runtime.interpreter_path)
        && inputs
            .python_runtime
            .root
            .starts_with("/private/var/db/iroha-kagemusha-python-runtime-v1/")
        && inputs.python_runtime.root.matches('/').count() == 5
        && inputs.python_runtime.interpreter_path
            == format!("{}/bin/python3", inputs.python_runtime.root)
        && is_nonzero_lower_hex(&inputs.python_runtime.interpreter_sha256, 64)
        && is_nonzero_lower_hex(&inputs.python_runtime.tree_sha256, 64)
        && inputs.runtime_identity.account_name == "_iroha_kagemusha_build"
        && inputs.runtime_identity.group_name == "_iroha_kagemusha_build"
        && inputs.runtime_identity.policy == "dedicated-nologin-no-concurrent-process-v1"
        && (1..=i32::MAX as u64).contains(&inputs.runtime_identity.uid)
        && (1..=i32::MAX as u64).contains(&inputs.runtime_identity.gid)
        && inputs.developer_dir.path == "/private/var/db/kagemusha/Xcode/Developer"
        && exact_source_seal_build_input_tree(&inputs.developer_dir.tree)
        && inputs.sdkroot.path
            == "/private/var/db/kagemusha/Xcode/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.2.sdk"
        && exact_source_seal_build_input_tree(&inputs.sdkroot.tree)
        && inputs.host_tools.len() == SOURCE_SEAL_REQUIRED_HOST_TOOLS.len()
        && inputs
            .host_tools
            .iter()
            .zip(SOURCE_SEAL_REQUIRED_HOST_TOOLS)
            .all(|(tool, expected_path)| {
                tool.path == *expected_path
                    && canonical_absolute_source_path(&tool.path)
                    && canonical_absolute_source_path(&tool.resolved_path)
                    && is_nonzero_lower_hex(&tool.binary_sha256, 64)
                    && tool.binary_size_bytes > 0
                    && tool.binary_size_bytes <= 512 * 1024 * 1024
            })
        && inputs.sandbox.backend == "macos-seatbelt-v1"
        && inputs.sandbox.profile_schema == "iroha.kagemusha.sealed_candidate_build_seatbelt.v1"
        && inputs
            .sandbox
            .qualification
            .iter()
            .map(String::as_str)
            .eq(expected_qualification)
        && portable_source_identifier(&inputs.sandbox.os_build, 64)
        && portable_source_identifier(&inputs.sandbox.xcode_build, 64)
}
