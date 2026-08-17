//! Embeds source identity into `iroha_core` binaries.
//!
//! Ordinary builds retain the lightweight Git-commit marker used by the Sumeragi v2 adapter
//! build fingerprint. The opt-in Kagemusha
//! candidate-build feature additionally requires and verifies an independently pinned reviewed
//! clean signed source closure and the exact Cargo/rustc binaries supplied by the dedicated build
//! helper.
use std::{env, ffi::OsStr, fs, path::Path, process::Command};

const SEALED_FEATURE_ENV: &str = "CARGO_FEATURE_KAGEMUSHA_CANDIDATE_SOURCE_SEAL";
const AUTHORIZED_PARENT_COMMIT: &str = "5d41c784787ed496ccbd46379ee236cc992d9c65";
const AUTHORIZED_PARENT_TREE: &str = "f20ab04ddd65c2b7da71250e77e2cc1006aa38f2";
const AUTHORIZED_PARENT_EPOCH: u64 = 1_786_749_503;
const PROJECTION_SCHEMA: &str = "iroha.kagemusha.authenticated_source_seal_projection.v1";
const OBSERVED_SCHEMA: &str = "iroha.kagemusha.source_seal_build_script_observed.v1";
const OUTER_POLICY_SCHEMA: &str = "iroha.kagemusha.cprime_source_seal_outer_policy.v1";
const UNIT_GRAPH_NORMALIZATION: &str = "cargo-unit-graph-v1-package-root-relative-src-path-source-cache-placeholders-sorted-compact-lf-v1";
const EXPECTED_FEATURE_ENVS: &[&str] = &[
    "CARGO_FEATURE_BLS",
    "CARGO_FEATURE_CIRCUIT_PARAMS",
    "CARGO_FEATURE_DEFAULT",
    "CARGO_FEATURE_DEV_TOOLS",
    "CARGO_FEATURE_JSON",
    "CARGO_FEATURE_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB",
    "CARGO_FEATURE_KAGEMUSHA_CANDIDATE_SOURCE_SEAL",
    "CARGO_FEATURE_NODE",
    "CARGO_FEATURE_PROOFS_HALO2",
    "CARGO_FEATURE_PROOFS_STARK",
    "CARGO_FEATURE_RUNTIME",
    "CARGO_FEATURE_ZK_HALO2",
    "CARGO_FEATURE_ZK_HALO2_IPA",
    "CARGO_FEATURE_ZK_IPA_NATIVE",
    "CARGO_FEATURE_ZK_STARK",
];
const RESOLVED_FEATURES_JSON: &str = concat!(
    "[\"bls\",\"circuit-params\",\"default\",\"dev-tools\",\"json\",",
    "\"kagemusha-candidate-evidence-lab\",\"kagemusha-candidate-source-seal\",",
    "\"node\",\"proofs-halo2\",\"proofs-stark\",\"runtime\",\"zk-halo2\",",
    "\"zk-halo2-ipa\",\"zk-ipa-native\",\"zk-stark\"]"
);
const EXPLICIT_FEATURES_JSON: &str = concat!(
    "[\"iroha_core/dev-tools\",\"iroha_core/kagemusha-candidate-evidence-lab\",",
    "\"iroha_core/kagemusha-candidate-source-seal\"]"
);
const SEMANTIC_ARGV_JSON: &str = concat!(
    "[\"build\",\"--release\",\"--locked\",\"--target-dir\",",
    "\"<EXTERNAL_TARGET_DIR>\",\"-p\",\"iroha_core\",\"--features\",",
    "\"iroha_core/dev-tools,iroha_core/kagemusha-candidate-source-seal,",
    "iroha_core/kagemusha-candidate-evidence-lab\",\"--bin\",",
    "\"kagemusha_recursive_spend_v4_bundle\",\"--jobs\",\"1\",",
    "\"--message-format=json-render-diagnostics\"]"
);
fn main() {
    if env::var_os(SEALED_FEATURE_ENV).is_some() {
        embed_exact_kagemusha_source_seal();
        return;
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
    if let Some(commit) = env_commit_hash().or_else(git_commit_hash) {
        println!("cargo:rustc-env=GIT_COMMIT_HASH={commit}");
    } else {
        println!(
            "cargo:warning=iroha_core build.rs: unable to determine git commit hash; \
             the Sumeragi v2 build fingerprint will use the `unknown` source marker"
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the authenticated projection is intentionally constructed and verified in one auditable flow"
)]
fn embed_exact_kagemusha_source_seal() {
    validate_exact_sealed_build_context();
    let source_commit = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_COMMIT", 40);
    let source_git_tree = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_GIT_TREE", 40);
    let parent_commit = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_PARENT_COMMIT", 40);
    let parent_tree = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_PARENT_TREE", 40);
    assert!(
        parent_commit == AUTHORIZED_PARENT_COMMIT && parent_tree == AUTHORIZED_PARENT_TREE,
        "sealed Kagemusha source lineage does not descend from the authorized optimizations authority"
    );
    let commit_object_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SHA256", 64);
    let commit_object_size =
        required_decimal_env("KAGEMUSHA_BUILD_SOURCE_COMMIT_OBJECT_SIZE", 1, 4096);
    let ssh_signer_principal =
        required_portable_identifier_env("KAGEMUSHA_BUILD_SOURCE_SSH_SIGNER_PRINCIPAL", 1, 128);
    let ssh_public_key_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_SSH_PUBLIC_KEY_SHA256", 64);
    let ssh_allowed_signers_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_SSH_ALLOWED_SIGNERS_SHA256", 64);
    let ssh_revocation_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_SSH_REVOCATION_SHA256", 64);
    let source_tree_sha256 = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256", 64);
    let reviewed_cargo_binary_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256", 64);
    let reviewed_rustc_binary_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256", 64);
    require_actual_tool_digest("CARGO", &reviewed_cargo_binary_sha256, "Cargo");
    require_actual_tool_digest("RUSTC", &reviewed_rustc_binary_sha256, "rustc");
    let closure_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256", 64);
    let closure_hex =
        required_lower_hex_env_bounded("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_HEX", 2, 8192);
    let closure = decode_hex(&closure_hex, "reviewed source closure");
    validate_one_line_canonical_ascii(&closure, 4096, "reviewed source closure");
    assert_eq!(
        sha256_hex(&closure),
        closure_sha256,
        "reviewed source closure bytes differ from their SHA-256"
    );
    let source_date_epoch = required_decimal_env(
        "KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH",
        AUTHORIZED_PARENT_EPOCH + 1,
        i64::MAX as u64,
    );
    let source_date_epoch_text = source_date_epoch.to_string();
    assert_eq!(
        env::var("SOURCE_DATE_EPOCH").ok().as_deref(),
        Some(source_date_epoch_text.as_str()),
        "SOURCE_DATE_EPOCH must exactly equal the authenticated source epoch"
    );
    let execution_policy_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_EXECUTION_POLICY_SHA256", 64);
    let unit_graph_sha256 = required_lower_hex_env("KAGEMUSHA_BUILD_UNIT_GRAPH_SHA256", 64);
    let unit_graph_size =
        required_decimal_env("KAGEMUSHA_BUILD_UNIT_GRAPH_SIZE_BYTES", 1, 16 * 1024 * 1024);
    let unit_count = required_decimal_env("KAGEMUSHA_BUILD_UNIT_GRAPH_UNITS", 1, 100_000);
    let package_count = required_decimal_env("KAGEMUSHA_BUILD_UNIT_GRAPH_PACKAGES", 1, 100_000);
    let custom_build_units = required_decimal_env(
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_UNITS",
        0,
        unit_count,
    );
    let custom_build_packages = required_decimal_env(
        "KAGEMUSHA_BUILD_UNIT_GRAPH_CUSTOM_BUILD_PACKAGES",
        0,
        package_count,
    );
    let iroha_core_units =
        required_decimal_env("KAGEMUSHA_BUILD_UNIT_GRAPH_IROHA_CORE_UNITS", 1, unit_count);
    let projection = format!(
        concat!(
            "{{\"build_script_observed\":{{\"debug_assertions\":false,",
            "\"features\":{RESOLVED_FEATURES_JSON},\"host\":\"aarch64-apple-darwin\",",
            "\"num_jobs\":1,\"opt_level\":\"3\",\"profile\":\"release\",",
            "\"schema\":\"{OBSERVED_SCHEMA}\",\"target\":\"aarch64-apple-darwin\"}},",
            "\"outer_policy\":{{\"cargo\":{{\"binary\":\"kagemusha_recursive_spend_v4_bundle\",",
            "\"explicit_features\":{EXPLICIT_FEATURES_JSON},\"package\":\"iroha_core\",",
            "\"profile\":\"release\",\"semantic_argv\":{SEMANTIC_ARGV_JSON},",
            "\"target\":\"aarch64-apple-darwin\",\"unit_graph\":{{",
            "\"custom_build_packages\":{custom_build_packages},",
            "\"custom_build_units\":{custom_build_units},",
            "\"iroha_core_units\":{iroha_core_units},",
            "\"normalization\":\"{UNIT_GRAPH_NORMALIZATION}\",",
            "\"packages\":{package_count},\"sha256\":\"{unit_graph_sha256}\",",
            "\"size_bytes\":{unit_graph_size},\"units\":{unit_count}}}}},",
            "\"execution_policy_sha256\":\"{execution_policy_sha256}\",",
            "\"schema\":\"{OUTER_POLICY_SCHEMA}\"}},",
            "\"reviewed_source_closure_hex\":\"{closure_hex}\",",
            "\"reviewed_source_closure_sha256\":\"{closure_sha256}\",",
            "\"schema\":\"{PROJECTION_SCHEMA}\",\"source_authority\":{{",
            "\"commit\":\"{source_commit}\",",
            "\"commit_object_sha256\":\"{commit_object_sha256}\",",
            "\"commit_object_size\":{commit_object_size},",
            "\"committer_epoch\":{source_date_epoch},\"git_tree\":\"{source_git_tree}\",",
            "\"ordered_parents\":[\"{parent_commit}\"],",
            "\"parent_commit\":\"{parent_commit}\",\"parent_tree\":\"{parent_tree}\",",
            "\"signature\":{{\"allowed_signers_sha256\":\"{ssh_allowed_signers_sha256}\",",
            "\"mechanism\":\"git-commit-ssh-signature-v1\",",
            "\"principal\":\"{ssh_signer_principal}\",",
            "\"public_key_sha256\":\"{ssh_public_key_sha256}\",",
            "\"revocation_sha256\":\"{ssh_revocation_sha256}\",",
            "\"signature_namespace\":\"git\"}}}},",
            "\"source_commit\":\"{source_commit}\",\"source_date_epoch\":{source_date_epoch},",
            "\"source_repo_dirty\":false,\"source_tree_sha256\":\"{source_tree_sha256}\"}}\n"
        ),
        RESOLVED_FEATURES_JSON = RESOLVED_FEATURES_JSON,
        OBSERVED_SCHEMA = OBSERVED_SCHEMA,
        EXPLICIT_FEATURES_JSON = EXPLICIT_FEATURES_JSON,
        SEMANTIC_ARGV_JSON = SEMANTIC_ARGV_JSON,
        custom_build_packages = custom_build_packages,
        custom_build_units = custom_build_units,
        iroha_core_units = iroha_core_units,
        UNIT_GRAPH_NORMALIZATION = UNIT_GRAPH_NORMALIZATION,
        package_count = package_count,
        unit_graph_sha256 = unit_graph_sha256,
        unit_graph_size = unit_graph_size,
        unit_count = unit_count,
        execution_policy_sha256 = execution_policy_sha256,
        OUTER_POLICY_SCHEMA = OUTER_POLICY_SCHEMA,
        closure_hex = closure_hex,
        closure_sha256 = closure_sha256,
        PROJECTION_SCHEMA = PROJECTION_SCHEMA,
        source_commit = source_commit,
        commit_object_sha256 = commit_object_sha256,
        commit_object_size = commit_object_size,
        source_date_epoch = source_date_epoch,
        source_git_tree = source_git_tree,
        parent_commit = parent_commit,
        parent_tree = parent_tree,
        ssh_allowed_signers_sha256 = ssh_allowed_signers_sha256,
        ssh_signer_principal = ssh_signer_principal,
        ssh_public_key_sha256 = ssh_public_key_sha256,
        ssh_revocation_sha256 = ssh_revocation_sha256,
        source_tree_sha256 = source_tree_sha256,
    );
    let projection_hex = required_lower_hex_env_bounded(
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX",
        2,
        32_768,
    );
    let supplied_projection = decode_hex(&projection_hex, "authenticated source-seal projection");
    validate_one_line_canonical_ascii(
        &supplied_projection,
        16_384,
        "authenticated source-seal projection",
    );
    assert_eq!(
        supplied_projection,
        projection.as_bytes(),
        "authenticated source-seal projection is noncanonical or differs from its inputs"
    );
    let supplied_projection_sha256 = required_lower_hex_env(
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256",
        64,
    );
    let actual_projection_sha256 = sha256_hex(&supplied_projection);
    assert_eq!(
        supplied_projection_sha256, actual_projection_sha256,
        "authenticated source-seal projection SHA-256 differs"
    );
    println!("cargo:rustc-env=GIT_COMMIT_HASH={source_commit}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_COMMIT={source_commit}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_TREE_SHA256={source_tree_sha256}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_DATE_EPOCH={source_date_epoch}");
    println!(
        "cargo:rustc-env=KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_HEX={projection_hex}"
    );
    println!(
        "cargo:rustc-env=KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256={actual_projection_sha256}"
    );
    println!(
        "cargo:rustc-env=KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256={reviewed_cargo_binary_sha256}"
    );
    println!(
        "cargo:rustc-env=KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256={reviewed_rustc_binary_sha256}"
    );
}

fn require_actual_tool_digest(environment_name: &str, expected_sha256: &str, label: &str) {
    let path = env::var_os(environment_name)
        .unwrap_or_else(|| panic!("{environment_name} is required for a sealed build"));
    let path = Path::new(&path);
    assert!(path.is_absolute(), "sealed {label} path must be absolute");
    let canonical = fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("sealed {label} path is unavailable: {error}"));
    let metadata = fs::metadata(&canonical)
        .unwrap_or_else(|error| panic!("sealed {label} metadata is unavailable: {error}"));
    assert!(
        metadata.is_file() && metadata.len() > 0,
        "sealed {label} path is not a nonempty regular file"
    );
    let bytes = fs::read(&canonical)
        .unwrap_or_else(|error| panic!("sealed {label} bytes are unavailable: {error}"));
    assert_eq!(
        sha256_hex(&bytes),
        expected_sha256,
        "sealed {label} bytes differ from the reviewed SHA-256"
    );
}

fn validate_exact_sealed_build_context() {
    for (name, expected) in [
        ("CARGO_PKG_NAME", "iroha_core"),
        ("HOST", "aarch64-apple-darwin"),
        ("TARGET", "aarch64-apple-darwin"),
        ("PROFILE", "release"),
        ("OPT_LEVEL", "3"),
        ("DEBUG", "false"),
        ("NUM_JOBS", "1"),
    ] {
        assert_eq!(
            env::var(name).ok().as_deref(),
            Some(expected),
            "{name} differs from the exact sealed candidate build context"
        );
    }
    let mut observed = Vec::new();
    for (name, value) in env::vars_os() {
        if let Some(name) = cargo_feature_env_name(&name) {
            assert_eq!(value, OsStr::new("1"), "{name} must have the exact value 1");
            observed.push(name.to_owned());
        }
    }
    observed.sort();
    observed.dedup();
    if observed.len() != EXPECTED_FEATURE_ENVS.len()
        || !EXPECTED_FEATURE_ENVS
            .iter()
            .all(|expected| observed.iter().any(|actual| actual == expected))
    {
        panic!(
            "CARGO_FEATURE_* environment differs from the exact sealed candidate feature closure"
        );
    }
}

#[cfg(unix)]
fn cargo_feature_env_name(name: &OsStr) -> Option<&str> {
    use std::os::unix::ffi::OsStrExt as _;
    let bytes = name.as_bytes();
    if !bytes.starts_with(b"CARGO_FEATURE_") {
        return None;
    }
    Some(
        std::str::from_utf8(bytes)
            .unwrap_or_else(|_| panic!("non-UTF-8 CARGO_FEATURE_* environment name is forbidden")),
    )
}

#[cfg(not(unix))]
fn cargo_feature_env_name(name: &OsStr) -> Option<&str> {
    let name = name
        .to_str()
        .unwrap_or_else(|| panic!("non-Unicode environment name is forbidden"));
    name.starts_with("CARGO_FEATURE_").then_some(name)
}

fn required_lower_hex_env(name: &str, expected_len: usize) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if value.len() != expected_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        panic!("{name} is not canonical lower-case hexadecimal");
    }
    value
}

fn required_portable_identifier_env(name: &str, minimum: usize, maximum: usize) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if value.len() < minimum
        || value.len() > maximum
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._@+-".contains(&byte))
    {
        panic!("{name} is not a bounded portable identifier");
    }
    value
}

fn required_lower_hex_env_bounded(name: &str, minimum: usize, maximum: usize) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if value.len() < minimum
        || value.len() > maximum
        || !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        panic!("{name} is not bounded canonical lower-case hexadecimal");
    }
    value
}

fn required_decimal_env(name: &str, minimum: u64, maximum: u64) -> u64 {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        panic!("{name} is not canonical unsigned decimal");
    }
    let parsed = value
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name} exceeds unsigned 64-bit decimal"));
    assert!(
        (minimum..=maximum).contains(&parsed) && parsed.to_string() == value,
        "{name} is outside its authenticated bound"
    );
    parsed
}

fn decode_hex(value: &str, label: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char)
                .to_digit(16)
                .unwrap_or_else(|| panic!("{label} hex is invalid"));
            let low = (pair[1] as char)
                .to_digit(16)
                .unwrap_or_else(|| panic!("{label} hex is invalid"));
            u8::try_from((high << 4) | low)
                .unwrap_or_else(|_| unreachable!("a hexadecimal byte always fits in u8"))
        })
        .collect()
}

fn validate_one_line_canonical_ascii(value: &[u8], maximum: usize, label: &str) {
    if value.is_empty()
        || value.len() > maximum
        || value.last() != Some(&b'\n')
        || value[..value.len() - 1].contains(&b'\n')
        || value.contains(&b'\r')
        || !value[..value.len() - 1]
            .iter()
            .all(|byte| matches!(byte, 0x20..=0x7e))
    {
        panic!("{label} is not one bounded canonical ASCII JSON line plus LF");
    }
}

fn sha256_hex(value: &[u8]) -> String {
    #[rustfmt::skip]
    const ROUND_CONSTANTS: [u32; 64] = [
        0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4,
        0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe,
        0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f,
        0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da, 0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7,
        0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc,
        0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
        0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070, 0x19a4_c116,
        0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3,
        0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut state = [
        0x6a09_e667_u32,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_len = (value.len() as u64)
        .checked_mul(8)
        .unwrap_or_else(|| panic!("SHA-256 input length overflow"));
    let mut padded = value.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for block in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(block[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [
            mut state_a,
            mut state_b,
            mut state_c,
            mut state_d,
            mut state_e,
            mut state_f,
            mut state_g,
            mut state_h,
        ] = state;
        for index in 0..64 {
            let sum1 =
                state_e.rotate_right(6) ^ state_e.rotate_right(11) ^ state_e.rotate_right(25);
            let choose = (state_e & state_f) ^ ((!state_e) & state_g);
            let temp1 = state_h
                .wrapping_add(sum1)
                .wrapping_add(choose)
                .wrapping_add(ROUND_CONSTANTS[index])
                .wrapping_add(words[index]);
            let sum0 =
                state_a.rotate_right(2) ^ state_a.rotate_right(13) ^ state_a.rotate_right(22);
            let majority = (state_a & state_b) ^ (state_a & state_c) ^ (state_b & state_c);
            let temp2 = sum0.wrapping_add(majority);
            state_h = state_g;
            state_g = state_f;
            state_f = state_e;
            state_e = state_d.wrapping_add(temp1);
            state_d = state_c;
            state_c = state_b;
            state_b = state_a;
            state_a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([
            state_a, state_b, state_c, state_d, state_e, state_f, state_g, state_h,
        ]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = String::with_capacity(64);
    for byte in state.into_iter().flat_map(u32::to_be_bytes) {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").unwrap();
    }
    out
}
fn env_commit_hash() -> Option<String> {
    let commit = env::var("IROHA_GIT_COMMIT_HASH").ok()?;
    let trimmed = commit.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
fn git_commit_hash() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = Command::new("git")
        .args(["-C", &manifest_dir, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let trimmed = hash.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
