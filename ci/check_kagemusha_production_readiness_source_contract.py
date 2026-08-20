"""Always-enforced static source contract for Kagemusha production readiness."""

if globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1") is not True:
    raise RuntimeError(
        "readiness source-contract provider must run inside the authenticated gate"
    )
_source_contract_source = globals().get(
    "_KAGEMUSHA_READINESS_SOURCE_CONTRACT_SOURCE_V1"
)
if not isinstance(_source_contract_source, str) or not _source_contract_source:
    raise RuntimeError(
        "readiness source-contract provider requires its exact loaded source bytes"
    )

def static_errors(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []
    overrides = overrides or {}
    texts = {
        path: overrides.get(path, read(path, errors))
        for path in (
            READINESS,
            READINESS_SELF_TEST,
            PRIVACY,
            PRIVACY_PROTOCOL,
            BRIDGE,
            HEADER,
            CORE,
            STEP_TRANSITION,
            RECURSIVE_BACKEND,
            RECURSION_ADAPTER,
            VALUE_CONTRACT,
            SCHEMA_GOLDEN,
            CONFIG,
            NODE,
            KAGAMI,
            BUNDLE,
            BUNDLE_SOURCE_SEAL_INPUTS,
            ROUTES,
            WORKFLOW,
            PROMOTION_WORKFLOW,
            IOS_EVIDENCE_MODULE,
            PRODUCTION_IOS_EVIDENCE_MODULE,
        )
    }
    texts[READINESS_SOURCE_CONTRACT] = overrides.get(
        READINESS_SOURCE_CONTRACT, _source_contract_source
    )
    texts[MODEL] = read_reviewed_model(errors, overrides)
    texts[CATALOG] = read_reviewed_catalog(errors, overrides)
    bundle_inputs_include = 'include!("kagemusha_recursive_spend_v4_bundle/source_seal_build_inputs.rs");'
    if texts[BUNDLE].count(bundle_inputs_include) != 1:
        errors.append(f"{BUNDLE}: expected exactly one reviewed source-seal input include")
    else:
        texts[BUNDLE] = texts[BUNDLE].replace(
            bundle_inputs_include, texts[BUNDLE_SOURCE_SEAL_INPUTS], 1
        )
    for relative, text in texts.items():
        forbid_merge_conflict_markers(text, relative, errors)
    require(
        texts[READINESS_SOURCE_CONTRACT],
        READINESS_SOURCE_CONTRACT,
        errors,
        'globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1") is not True',
        '"_KAGEMUSHA_READINESS_SOURCE_CONTRACT_SOURCE_V1"',
        "def static_errors(",
    )
    require(
        texts[READINESS_SELF_TEST],
        READINESS_SELF_TEST,
        errors,
        'globals().get("_KAGEMUSHA_READINESS_SELF_TEST_CONTEXT_V1") is not True',
        "def expect_value_error(",
        "def expect_static_mutation(",
        "run_bounded_authenticated_process(",
        "expect_static_mutation(READINESS, *mutation)",
        "validate_native_build_launch_binding(",
        "validate_native_builder_entrypoint_binding(",
    )
    model = texts[MODEL]
    require(
        model,
        MODEL,
        errors,
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v4"',
        '"iroha.reviewed-source-closure.v1"',
        "reviewed_source_closure_descriptor_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "generator_binary_sha256",
        "sealed_candidate_build_report_sha256",
        "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4: [&str; 8]",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
        "pub enum KagemushaPastaCycleArtifactKindV4",
        "ParamsIpa",
        "BootstrapWitness",
        "KagemushaRecursiveSpendReleaseActivationV4",
        "kagemusha_recursive_spend_verifier_key_id_v4",
    )
    forbid(
        model,
        MODEL,
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    forbid(
        "\n".join(
            texts[path]
            for path in (
                BRIDGE,
                CORE,
                STEP_TRANSITION,
                RECURSIVE_BACKEND,
                VALUE_CONTRACT,
                SCHEMA_GOLDEN,
            )
        ),
        "Rust ABI-21/V4 corridor",
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    for artifact in ARTIFACTS:
        if model.count(f'"{artifact}"') != 1:
            errors.append(f"{MODEL}: exact-eight artifact {artifact!r} must be declared once")
    availability = re.search(
        r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*"
        r'cfg!\(feature\s*=\s*"kagemusha-production-enabled"\)\s*;',
        model,
    )
    if availability is None:
        errors.append(
            f"{MODEL}: production availability must be controlled only by the "
            "kagemusha-production-enabled feature"
        )
    require(
        texts[PRIVACY],
        PRIVACY,
        errors,
        'include!("privacy/protocol.rs");',
    )
    require(
        texts[PRIVACY_PROTOCOL],
        PRIVACY_PROTOCOL,
        errors,
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;",
    )
    require(
        texts[BRIDGE],
        BRIDGE,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "promotion_record_norito_ptr",
        "KagemushaRecursiveSpendReleaseRecordV4",
        ".authenticate(&trusted_policy)",
        "self.promotion_record",
        "validate_against_authenticated_release",
        "require_kagemusha_recursive_spend_production_promotion_v4()?",
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
        "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
        "installed.validate_live_inventory()?",
        "KagemushaQualifiedArtifactSourceV4",
        "qualify_kagemusha_authenticated_artifact_source_v4(",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source(",
        "KagemushaPastaCycleOpaqueProverV4::from_qualified_artifact_source(",
        "from_candidate_artifact_spool_loader(",
        "fn candidate_proving_key_spool(",
        "fn runtime_verifier(",
        "fn runtime_prover(",
        "recursive_spend_v4_prover_and_terminal_verifier_lifetimes_do_not_overlap",
        '"authenticated-v4-artifact-installation"',
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "KagemushaRecursiveSpendRedemptionChangePrepareRequestV4",
        "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
    )
    require(
        texts[HEADER],
        HEADER,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION 22",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "promotion_record_norito_ptr",
    )
    forbid(
        texts[BRIDGE] + texts[HEADER],
        f"{BRIDGE} / {HEADER}",
        errors,
        "kagemusha_recursive_spend_artifact_begin_v3",
        "kagemusha_recursive_spend_artifact_set_install_v3",
        "kagemusha_recursive_spend_init_v3",
        "kagemusha_recursive_spend_append_v3",
    )
    require(
        texts[CATALOG],
        CATALOG,
        errors,
        "pub struct KagemushaReleaseCatalogV4",
        "pub fn load(policy_path: &Path, artifact_dir: &Path)",
        "exactly eight artifacts",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source",
        "DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4",
        "const MAX_CATALOG_AGGREGATE_BYTES_V4: u64 = 12 * 1024 * 1024 * 1024;",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
    )
    runtime_profile_validation = texts[RECURSION_ADAPTER].split(
        "fn validate_kagemusha_profile_protocol_v4<C>(", 1
    )[-1].split("fn terminal_validate_kagemusha_eq_bootstrap_v4(", 1)[0]
    forbid(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "keygen_vk",
        "kagemusha_bootstrap_verifying_key_v1",
        "validate_bootstrap_protocol",
    )
    require(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "kagemusha_compiled_protocol_structure_sha256",
        "KagemushaStepBootstrapV4::decode_authenticated",
    )
    require_pattern(
        texts[CATALOG],
        CATALOG,
        errors,
        (
            r"const\s+KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4:\s*usize\s*=\s*"
            r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4\.len\(\)\s*;\s*"
            r"[\s\S]*?"
            r"if\s+manifest\s*"
            r"\.profiles\s*\.iter\(\)\s*"
            r"\.map\(\|profile\|\s*profile\.artifacts\.len\(\)\)\s*"
            r"\.sum::<usize>\(\)\s*"
            r"!=\s*KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4\s*\{"
        ),
        "exact-eight manifest inventory check",
    )
    forbid(
        texts[CATALOG] + texts[CORE] + texts[NODE] + texts[KAGAMI],
        "configured V4 runtime",
        errors,
        "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX",
        "kagemusha_enabled",
    )
    require(
        texts[KAGAMI],
        KAGAMI,
        errors,
        "fn configured_policy_bytes(path: &Path)",
        'decode_canonical_norito(&configured, "configured Kagemusha V4 release policy")',
        "KagemushaAuthenticatedReleaseV4::verify",
        "KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4",
        "if expected.len() != 17",
        "ActivateKagemushaRecursiveReleaseV4::new(activation, policy)",
        r'instruction_count\":1',
    )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"fn verify_exact_inventory_v4\(.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"if expected\.len\(\) != 17.*?"
            r"fn recursive_step_verifier_commitment_v4\("
        ),
        "function-scoped 17-file verifier inventory including the qualification receipt",
    )
    require(
        texts[BUNDLE],
        BUNDLE,
        errors,
        "const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 17;",
        "fn final_release_inventory_v4() -> BTreeSet<String>",
        "KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4",
        "if expected.len() != FINAL_RELEASE_INVENTORY_COUNT_V4",
        "fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()",
    )
    require_pattern(
        texts[BUNDLE],
        BUNDLE,
        errors,
        (
            r"fn final_release_inventory_v4\(\).*?\.chain\(\[.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"\]\).*?\.collect\(\).*?impl PublicationDirectory"
        ),
        "function-scoped 17-file producer inventory including the qualification receipt",
    )
    require_pattern(
        texts[MODEL],
        MODEL,
        errors,
        (
            r"pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4: usize\s*=\s*"
            r"2 \* KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize "
            r"\+ 16 \* 1024;"
        ),
        "qualification receipt bound derived from two absolute proof pairs plus framing",
    )
    require_pattern(
        texts[MODEL],
        MODEL,
        errors,
        (
            r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32\s*=\s*"
            r"384 \* 1024;"
        ),
        "384 KiB absolute V4 proof-pair bound",
    )
    opaque_metadata_section = texts[READINESS].split(
        "BOUNDED_AUTHENTICATED_METADATA = (", 1
    )[-1].split("READ_CHUNK_BYTES =", 1)[0]
    if "recursive-step-two-qualification-v4.norito" in opaque_metadata_section:
        errors.append(
            f"{READINESS}: opaque qualification receipt is routed through textual evidence scanning"
        )
    verifier_function = texts[READINESS].rsplit(
        "def release_verifier_command(", 1
    )[-1].split("def validate_kagami_verification_report(", 1)[0]
    require(
        texts[READINESS],
        READINESS,
        errors,
        'KAGAMI_VERIFIER_PATH_ENV = "KAGEMUSHA_V4_KAGAMI_BIN"',
        'KAGAMI_VERIFIER_SHA256_ENV = "KAGEMUSHA_V4_KAGAMI_SHA256"',
        '"KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"',
        '"KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256"',
        'AUTHENTICATED_TOOL_CONTROLLER_CONTRACT = "iroha.authenticated-tool-os-isolation.v1"',
        "hash_pinned_descriptor(",
        "def validate_kagami_verification_report(",
        "environment=SANITIZED_VERIFIER_ENV",
        'cwd=Path("/")',
        "validate_kagami_verification_report(",
        "def load_ios_evidence_validator(",
        "read_pinned_descriptor(",
        "PRODUCTION_TRUSTED_UID = 0",
        "def require_production_root_custody(",
        "def require_no_macos_extended_acl(",
        "MACOS_LIBC.acl_get_fd_np",
        'require_no_macos_extended_acl(trusted_python_fd, "inherited promotion Python")',
        'require_no_macos_extended_acl(trusted_gate_fd, "inherited promotion gate")',
        "production promotion must run as root",
        "def snapshot_private_bytes(",
        "evidence_bytes_are_non_placeholder(",
        "trusted_python_sha256 = sys.argv[4]",
        "running promotion Python differs from its trusted SHA-256",
        "def validate_inherited_promotion_python(",
        "inherited promotion Python differs from its trusted SHA-256",
        "def validate_inherited_promotion_gate(",
        "inherited promotion gate differs from its reviewed SHA-256",
        'KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256',
        'READINESS_SOURCE_CONTRACT = (',
        "MAX_READINESS_SOURCE_CONTRACT_BYTES = 128 * 1024",
        "authenticated_readiness_source_contract_bytes: bytes | None = None",
        "PROMOTION_STAGING_PARENT = Path(",
        "authenticate_reviewed_source_file(",
        "validate_source_trust_projection(",
        "isolated_source_trust_git_config(",
        "SOURCE_ALLOWED_SIGNERS_PATH_ENV",
        "SOURCE_REVOCATION_PATH_ENV",
        "SOURCE_SEAL_PROJECTION_PATH_ENV",
        "SOURCE_SEAL_VERIFICATION_INPUTS",
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH"',
        '"KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH"',
        "trusted_source_helper_snapshot",
        "trusted_ios_validator_snapshot",
        "PRODUCTION_IOS_EVIDENCE_MODULE",
        "validate_production_signed_evidence",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY",
        "online-freshness-consumption-receipt-v1.json",
        "promotion Python runtime closure changed during the production gate",
        "static candidate corridor passed;",
        "production promotion was not evaluated.",
    )
    require(
        texts[PRODUCTION_IOS_EVIDENCE_MODULE],
        PRODUCTION_IOS_EVIDENCE_MODULE,
        errors,
        "iroha.kagemusha.ios_device_lab.production_signed_evidence.v1",
        "iroha.kagemusha.ios.production_device_policy.v1",
        "def validate_production_signed_evidence(",
        "def build_production_signed_evidence(",
        "def _parse_x509_certificate(",
        "def _validate_attestation_certificate_chain(",
        "OID_APP_ATTEST_NONCE",
        "iroha.kagemusha.ios.app_attest_online_freshness_consumption_receipt.v1",
        "def _validate_online_freshness_receipt(",
        '"previous_assertion_counter"',
        '"consumption_id"',
    )
    shell_bootstrap = texts[READINESS].split("<<'PY'", 1)[0]
    require(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        'SCRIPT_EXECUTION_SOURCE="${BASH_SOURCE[0]}"',
        'SCRIPT_SOURCE_ORIGINAL="${GATE_LAUNCH_SOURCE_PATH}"',
        'SCRIPT_SOURCE_ORIGINAL="${SCRIPT_EXECUTION_SOURCE}"',
        "builtin pwd -P",
        "promotion_assert_root_custody",
        "promotion_assert_no_extended_acl",
        "/bin/ls -lde",
        "/usr/bin/xattr",
        "promotion_root_tree_sha256",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
        "/private/var/db/iroha-kagemusha-python-runtime-v1",
        "/usr/bin/find -s",
        "-print0",
        "promotion Python runtime closure differs from its trusted tree SHA-256",
        "promotion Python runtime closure changed before interpreter execution",
        "changed during ACL validation",
        'promotion_assert_root_custody "${DERIVED_ROOT_DIR}" "promotion readiness checkout"',
        'promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate"',
        'exec 8<"${SCRIPT_PATH}"',
        '"/dev/fd/${GATE_PIN_FD}"',
        'KAGEMUSHA_PRODUCTION_READINESS_PYTHON_PIN_FD',
        '"/dev/fd/${PYTHON_PIN_FD}"',
        "promotion Python inherited descriptor differs from its path",
        '"${PYTHON_PIN_FD}" "${PYTHON_PATH_FINGERPRINT}"',
        "promotion Python interpreter changed before execution",
        "rejects missing or symlinked script invocation",
        "from an independently authenticated launcher/controller",
        'PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-python3}"',
        "sys.version_info >= (3, 10)",
        "requires Python 3.10 or newer",
        'kagemusha_isolated_git diff --cached --quiet --no-ext-diff --no-textconv --diff-filter=U --',
        '--work-tree="${ROOT_DIR}"',
        'GIT_CONFIG_COUNT=0',
        '-c core.attributesFile=/dev/null',
        '-c core.excludesFile=/dev/null',
        '-c core.fsmonitor=false',
        '-c core.hooksPath=/dev/null',
        '-c core.preloadIndex=false',
        '-c core.untrackedCache=false',
        '-c submodule.recurse=false',
        'config --get-all core.worktree',
        'rejects a Git core.worktree redirect outside the repository root',
        'if [[ "${CONFIGURED_CORE_WORKTREE}" != "${ROOT_DIR}" ]]; then',
        'if [[ "${CONFIGURED_CORE_WORKTREE_STATUS}" -ne 1 ]]; then',
        "readiness rejects unresolved Git index entries",
    )
    shell_custody_function = shell_bootstrap.split(
        "promotion_assert_root_custody() {", 1
    )[-1].split("\n}\npromotion_root_tree_sha256() {", 1)[0]
    shell_acl_function = shell_bootstrap.split(
        "promotion_assert_no_extended_acl() {", 1
    )[-1].split("\n}\n\npromotion_assert_root_custody() {", 1)[0]
    require_pattern(
        shell_acl_function,
        "promotion shell ACL custody",
        errors,
        (
            r'\[\[ "\$\{OSTYPE\}" != darwin\* \]\].*?'
            r'/bin/ls -lde -- "\$\{target\}".*?'
            r'\[\[ "\$\{mode_marker\}" == \*\+ \]\].*?has an extended ACL.*?'
            r'/usr/bin/xattr "\$\{target\}".*?has unbound extended attributes'
        ),
        "fail-closed macOS extended-ACL inspection",
    )
    if shell_custody_function.count("promotion_assert_no_extended_acl") != 2:
        errors.append(
            f"{READINESS}: promotion shell custody does not reject ACLs on the root and every path component"
        )
    descriptor_custody_functions = texts[READINESS].split(
        "def require_no_macos_extended_acl(", 1
    )[-1].split("def snapshot_private_bytes(", 1)[0]
    require_pattern(
        descriptor_custody_functions,
        READINESS,
        errors,
        (
            r"MACOS_LIBC\.acl_get_fd_np\(descriptor, MACOS_ACL_TYPE_EXTENDED\).*?"
            r"entry_status\s*=\s*MACOS_LIBC\.acl_get_entry\(.*?"
            r"if entry_status == 0:.*?must not have an extended ACL.*?"
            r"MACOS_LIBC\.flistxattr\(descriptor, None, 0, 0\).*?"
            r"must not have unbound extended attributes.*?"
            r"def require_production_root_custody\(.*?"
            r"require_no_macos_extended_acl\(descriptor, label\)"
        ),
        "descriptor-exact macOS ACL rejection in production custody",
    )
    forbid(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        "$(dirname ",
        "`dirname ",
        "readlink ",
    )
    require_pattern(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        (
            r"promotion_assert_root_custody \"\$\{PYTHON_BIN\}\""
            r".*?PYTHON_PATH_FINGERPRINT=.*?"
            r".*?/dev/fd/\$\{PYTHON_PIN_FD\}.*?"
            r"promotion Python inherited descriptor differs from its path.*?"
            r"promotion Python interpreter changed before execution.*?"
            r"promotion_assert_root_custody \"\$\{PYTHON_BIN\}\""
        ),
        "pre-exec Python descriptor custody",
    )
    require_pattern(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        (
            r"promotion_assert_root_custody \"\$\{DERIVED_ROOT_DIR\}\""
            r".*?promotion_assert_root_custody \"\$\{SCRIPT_PATH\}\""
            r".*?KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256"
            r".*?exec 8<\"\$\{SCRIPT_PATH\}\""
            r".*?/dev/fd/\$\{GATE_PIN_FD\}.*?OBSERVED_GATE_SHA256"
            r".*?!=.*?GATE_SHA256.*?"
            r"differs from its independently reviewed SHA-256.*?"
            r"promotion_assert_root_custody \"\$\{SCRIPT_PATH\}\""
        ),
        "independently pinned root-custodied gate bootstrap",
    )
    forbid(
        verifier_function,
        "promotion verifier command",
        errors,
        '"cargo"',
        '"run"',
    )
    verifier_execution = texts[READINESS].rsplit(
        "def terminate_authenticated_verifier_process_group(", 1
    )[-1].split("def release_verifier_command(", 1)[0]
    require(
        verifier_execution,
        "authenticated Kagami verifier execution",
        errors,
        "selectors.DefaultSelector()",
        "preexec_fn=os.setpgrp",
        "time.monotonic() + timeout_seconds",
        "os.killpg(observed_process_group, signal.SIGTERM)",
        "os.killpg(observed_process_group, signal.SIGKILL)",
        "len(capture) > limit",
        "KAGAMI_VERIFIER_TIMEOUT_SECONDS",
        "MAX_KAGAMI_VERIFIER_STDOUT_BYTES",
        "MAX_KAGAMI_VERIFIER_STDERR_BYTES",
        "environment=SANITIZED_VERIFIER_ENV",
        "authenticated_verifier_exit_diagnostic",
        "authenticated_verifier_exited_without_reaping(process)",
        "authenticated_verifier_controller_command(controller, command)",
        '"--use-attested-runtime-identity"',
        '"--expected-runtime-uid"',
        '"--expected-runtime-gid"',
        '"--no-new-privileges"',
        '"--close-inherited-fds"',
        '"--forward-tool-exit-status"',
        '"--exact-tool-stdio"',
        '"--deny-network"',
        '"--deny-tool-process-spawn"',
        '"--deny-read-outside-allowlist"',
        '"--readable-file"',
        '"--readable-directory"',
        '"--deny-all-writes"',
        '"--require-empty-process-tree"',
        '"--account-unlinked-write-bytes"',
        '"--cumulative-write-limit-bytes"',
        '"--maximum-live-write-root-bytes"',
    )
    require_pattern(
        verifier_execution,
        "authenticated Kagami verifier execution",
        errors,
        (
            r"if authenticated_verifier_exited_without_reaping\(process\):"
            r".*?terminate_authenticated_verifier_process_group\(\s*"
            r"process,\s*leader_exit_observed=True,\s*\)"
            r".*?returncode\s*=\s*process\.returncode"
        ),
        "unconditional success-path verifier process-group sweep before leader reap",
    )
    forbid(
        verifier_execution,
        "authenticated Kagami verifier execution",
        errors,
        "capture_output=True",
        "text=True",
        "shell=True",
    )
    ios_validator_function = texts[READINESS].rsplit(
        "def verify_ios_evidence(", 1
    )[-1].split("def promotion_errors(", 1)[0]
    require_pattern(
        ios_validator_function,
        "physical-iOS evidence verification",
        errors,
        (
            r"validation_errors\s*=\s*validator\(\s*evidence_snapshot_path,"
            r".*?trusted_public_key_snapshot,\s*"
            r"trusted_production_policy_snapshot,\s*"
            r"freshness_snapshot_path,\s*"
            r"freshness_key_id,\s*"
            r"trusted_freshness_public_key_snapshot,\s*\).*?"
            r"evidence\s*=\s*strict_json_bytes\(\s*evidence_bytes,"
        ),
        "same pinned evidence, trusted key, and production policy snapshots for validation and digest binding",
    )
    forbid(
        ios_validator_function,
        "physical-iOS evidence verification",
        errors,
        "subprocess.run",
        "sys.executable",
        "check_kagemusha_candidate_ios_evidence.py",
        "validator(evidence_path",
    )
    promotion_function = texts[READINESS].rsplit("def promotion_errors(", 1)[-1].split(
        "source_contract_errors: list[str] = []", 1
    )[0]
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"tool_controller_text\s*=\s*os\.environ\.get\(.*?"
            r"tool_controller_sha256\s*=\s*os\.environ\.get\(.*?"
            r"tool_controller_sha256\s*==\s*verifier_sha256.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"!=\s*tool_controller_sha256.*?"
            r"tool_controller_snapshot, tool_controller_exec\s*=\s*"
            r"snapshot_pinned_executable\(.*?"
            r"run_authenticated_verifier\(command, tool_controller_exec\)"
        ),
        "distinct digest-pinned authenticated tool controller and snapshot execution",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"release_verifier_command\(verifier_exec, directory, policy\).*?"
            r"run_authenticated_verifier\(command, tool_controller_exec\).*?"
            r"authenticated_verifier_exit_diagnostic\(verified\.returncode\).*?"
            r"strict_json_bytes\(\s*verified\.stdout,"
        ),
        "bounded authenticated verifier execution and deterministic diagnostics",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"ios_configuration\s*=\s*ios_evidence_configuration\(errors\)"
            r".*?authenticate_reviewed_source_file\(\s*PRODUCTION_IOS_EVIDENCE_MODULE,"
            r".*?load_ios_evidence_validator\(\s*validator_bytes,"
        ),
        "fail-closed production iOS evidence validator path",
    )
    ios_loader_function = texts[READINESS].rsplit(
        "def load_ios_evidence_validator(", 1
    )[-1].split("def verify_ios_evidence(", 1)[0]
    require_pattern(
        ios_loader_function,
        READINESS,
        errors,
        (
            r"production_validator\s*=\s*production_module\.__dict__\.get\(\s*"
            r'"validate_production_signed_evidence"\s*\)'
        ),
        "production-only iOS evidence validator entrypoint",
    )
    if promotion_function.count("require_production_root_custody(") < 17:
        errors.append(
            f"{READINESS}: promotion does not root-custody every production trust class"
        )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"if path in production_directory_paths:\s*try:\s*"
            r"require_production_root_custody\(descriptor, label\)"
        ),
        "root-custody the complete production path-component set",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"production_roots\s*=\s*\[.*?PROMOTION_STAGING_PARENT.*?\]"
            r".*?snapshot_pinned_executable\(.*?PROMOTION_STAGING_PARENT"
        ),
        "fixed pinned production staging parent",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"production_roots\s*=\s*\[\s*root,\s*source_helper_path\.parent,"
            r"\s*ios_validator_path\.parent,.*?"
            r"reviewed promotion readiness gate.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"MAX_READINESS_GATE_BYTES.*?trusted_gate_sha256"
        ),
        "retained root-custodied reviewed gate and checkout",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"authenticate_reviewed_source_file\(\s*SOURCE_TREE_SEAL,"
            r".*?snapshot_private_bytes\(\s*source_helper_bytes,"
            r".*?str\(trusted_source_helper_snapshot\)"
        ),
        "source-closure-authenticated source-tree helper snapshot",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"label = f\"reviewed readiness source-contract provider.*?"
            r"pin_regular_metadata\(source_contract_path, label\).*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"read_pinned_descriptor\(\s*descriptor,\s*fingerprint,\s*"
            r"MAX_READINESS_SOURCE_CONTRACT_BYTES,\s*label,\s*\).*?"
            r"authenticate_reviewed_source_file\(\s*READINESS_SOURCE_CONTRACT,\s*"
            r"source_contract_bytes,\s*reviewed_source_commit,\s*"
            r"MAX_READINESS_SOURCE_CONTRACT_BYTES,\s*\).*?"
            r"authenticated_readiness_source_contract_bytes = source_contract_bytes"
        ),
        "root-custodied source-closure-authenticated source-contract bytes",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"if self_test:\s*"
            r"label = f\"reviewed readiness self-test helper.*?"
            r"pin_regular_metadata\(\s*readiness_self_test_path, label\s*\).*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"read_pinned_descriptor\(\s*descriptor,\s*fingerprint,\s*"
            r"MAX_READINESS_SELF_TEST_BYTES,\s*label,\s*\).*?"
            r"authenticate_reviewed_source_file\(\s*READINESS_SELF_TEST,\s*"
            r"readiness_self_test_bytes,\s*reviewed_source_commit,\s*"
            r"MAX_READINESS_SELF_TEST_BYTES,\s*\).*?"
            r"authenticated_readiness_self_test_bytes = readiness_self_test_bytes"
        ),
        "root-custodied source-closure-authenticated readiness self-test bytes",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"source SSH allowed-signers policy.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"allowed_signers_sha256.*?snapshot_private_bytes\(.*?"
            r"source SSH revocation policy.*?allow_empty=True.*?"
            r"revocation_sha256.*?snapshot_private_bytes\(.*?"
            r"authenticated source-seal projection.*?"
            r"source_projection_sha256.*?validate_source_trust_projection\(.*?"
            r"isolated_source_trust_git_config\(.*?\.gitconfig.*?"
            r'"HOME": str\(trusted_source_trust_home\)'
        ),
        "closure-bound snapshotted source SSH trust policies",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"source_identity = parsed_identity.*?"
            r"for relative in SOURCE_PROJECTION_PRODUCER_CLOSURE.*?snapshot_private_python_package.*?"
            r'"verify".*?"--authorization".*?"--controller-signature".*?'
            r'"--controller-allowed-signers".*?"--controller-revocation".*?'
            r'"--execution-policy".*?"--raw-unit-graph".*?"--unit-graph".*?'
            r'"--projection".*?source-projection reconstruction receipt.*?'
            r"manifest build tools differ from the authenticated execution policy"
        ),
        "signed source-projection reconstruction and build-tool cross-binding",
    )
    forbid(
        promotion_function,
        "promotion source SSH trust bootstrap",
        errors,
        '"HOME": "/var/empty"',
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"authenticate_reviewed_source_file\(\s*IOS_EVIDENCE_MODULE,"
            r".*?snapshot_private_bytes\(\s*validator_bytes,"
            r".*?authenticate_reviewed_source_file\(\s*PRODUCTION_IOS_EVIDENCE_MODULE,"
            r".*?snapshot_private_bytes\(\s*production_validator_bytes,"
            r".*?load_ios_evidence_validator\(\s*validator_bytes,"
            r"\s*trusted_ios_validator_snapshot,\s*production_validator_bytes,"
        ),
        "source-closure-authenticated candidate and production iOS validator snapshots",
    )
    snapshot_functions = texts[READINESS].split(
        "def snapshot_private_bytes(", 1
    )[-1].split("def canonical_nonzero_sha256(", 1)[0]
    if snapshot_functions.count("dir=staging_parent") != 3:
        errors.append(
            f"{READINESS}: promotion snapshots do not use only their explicit staging parent"
        )
    verifier_environment = texts[READINESS].split(
        "SANITIZED_VERIFIER_ENV = {", 1
    )[-1].split("READ_CHUNK_BYTES =", 1)[0]
    if (
        verifier_environment.count('"TMPDIR": str(PROMOTION_STAGING_PARENT),')
        != 1
        or promotion_function.count(
            '"TMPDIR": str(PROMOTION_STAGING_PARENT),'
        )
        != 2
    ):
        errors.append(
            f"{READINESS}: promotion subprocesses do not use only the fixed staging parent"
        )
    forbid(
        promotion_function,
        "promotion catalog byte custody",
        errors,
        "read_regular_bounded(",
        "inspect_regular_prefix(",
        "strict_json(",
        ".read_bytes()",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"promotion Python runtime.*?hash_pinned_descriptor\(.*?\)"
            r"\s*!=\s*trusted_python_sha256"
        ),
        "running promotion interpreter digest revalidation",
    )
    require(
        texts[CONFIG] + texts[NODE] + texts[CORE],
        "configured V4 runtime",
        errors,
        "kagemusha_release_policy_path",
        "kagemusha_artifact_dir",
        "KagemushaReleaseCatalogV4::load",
        "ensure_kagemusha_active_release_material_v4",
    )
    require(
        texts[CORE],
        CORE,
        errors,
        "impl Execute for ActivateKagemushaRecursiveReleaseV4",
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
        "validate_offline_attestation_policy_for_release_activation",
        "self.device_attestation_policy",
        "impl Execute for TopUpKagemushaRecursiveV4",
        "impl Execute for RedeemKagemushaRecursiveV4",
        "issuance_active_at",
    )
    require_pattern(
        texts[CORE],
        CORE,
        errors,
        (
            r"let\s+change_release\s*=\s*request\s*\.offline_change\s*\.as_ref\(\)"
            r".*?\.transpose\(\)\?\s*;\s*"
            r"if\s+change_release\.as_ref\(\)\.is_some_and\(\|release\|\s*\{\s*"
            r"!\s*release\s*\.cached\s*"
            r"\.issuance_active_at\(state_transaction\.block_height\(\)\)"
        ),
        "offline-change withdrawal-height issuance check",
    )
    for route in ROUTE_LITERALS:
        if route not in texts[ROUTES]:
            errors.append(f"{ROUTES}: stable route changed or disappeared: {route}")
    require(
        texts[WORKFLOW],
        WORKFLOW,
        errors,
        "check_kagemusha_production_readiness.sh candidate",
        "check_kagemusha_production_readiness.sh candidate --self-test",
        "ci/check_kagemusha_production_readiness_source_contract.py",
        "ci/check_kagemusha_production_readiness_self_test.py",
        "ci/check_kagemusha_recursive_spend_python_sdk.sh --self-test",
        "check_kagemusha_recursive_spend_v4_sdk_contract.sh",
        '"crates/iroha_core/src/smartcontracts/isi/offline/**"',
        '"crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle/**"',
        '"specs/sdk/swift/readiness/*kagemusha*.md"',
        "scripts/tests/build_kagemusha_v4_candidate_bundle_test.py",
        "scripts/tests/build_kagemusha_production_ios_policy_test.py",
        "scripts/tests/check_kagemusha_candidate_ios_evidence_test.py",
        "scripts/tests/kagemusha_app_attest_freshness_authority_test.py",
        "scripts/tests/kagemusha_production_app_attest_lab_source_test.py",
        "scripts/tests/measure_kagemusha_production_app_attest_bundle_test.py",
        "scripts/tests/sign_kagemusha_production_ios_evidence_test.py",
        "scripts/tests/kagemusha_source_tree_seal_test.py",
        "scripts/tests/produce_kagemusha_v4_source_seal_projection_test.py",
        "scripts/tests/kagemusha_staged_resource_guard_test.py",
        "scripts/tests/stage_kagemusha_candidate_android_artifacts_test.py",
        "scripts/tests/stage_kagemusha_candidate_android_lab_test.py",
        "pytests/scripts/run_kagemusha_v4_generation_test.py",
        "pytests/scripts/run_kagemusha_v4_generation_benchmark_test.py",
        "cargo test -p iroha_data_model receiver_snapshot --lib",
        "cargo test -p iroha_core kagemusha_v4 --lib",
        "cargo test -p iroha_core offline_device_attestation_policy --lib",
        "cargo test -p iroha_core device_registration_ --lib",
        "cargo test -p iroha_core kagemusha_online_registration_ --lib",
        "cargo test -p iroha_core active_receiver_snapshot_ --lib",
        "cargo test -p iroha_core --features \"dev-tools,zk-halo2-ipa,kagemusha-candidate-evidence-lab\" --bin kagemusha_recursive_spend_v4_bundle final_release_inventory_is_exact_and_includes_recursive_qualification_receipt",
        "cargo test -p iroha_core sparse_confidential_subtree_roots_match_dense_reference --lib",
        "cargo test -p iroha_core next_zero_confidential_path_matches_padded_tree_path --lib",
        "cargo test -p iroha_core sequential_append_paths --lib",
        "cargo test -p iroha_core recursive_state_vector_is_exact_and_zero_padded --lib",
        "cargo test -p iroha_core output_membership --lib",
        "cargo test -p iroha_core v4_eq_frontier_copy_constraints --lib",
        "cargo test -p iroha_core v4_manifest_preserves_exact_little_endian_state_limbs --lib",
        "cargo test -p iroha_core v4_eq_and_ep_public_columns_share_the_v2_result_frontier_limb --lib",
        "cargo test -p iroha_core kagemusha_terminal_registry_v4 --lib",
        "cargo test -p iroha_kagami --bin kagami harden_private_tree",
        "cargo test -p iroha_kagami --bin kagami private_custody_readme_invokes_non_executable_scripts_through_bash",
        "cargo test -p iroha_kagami --bin kagami raw_npos_genesis_receives_the_chain_bound_localnet_epoch_seed",
        "cargo test -p iroha_kagami --bin kagami atomic_activation_",
        "cargo test -p iroha_kagami --bin kagami backing_",
        "cargo test -p iroha_torii readiness_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii offline_commands --lib -- --nocapture",
        "cargo test -p iroha_config settlement_offline_tests -- --nocapture",
        "cargo test -p iroha_config torii_kagemusha_commands_tests -- --nocapture",
        "cargo test -p connect_norito_bridge recursive_spend_v4",
        "cargo test -p connect_norito_bridge output_membership_local_carrier --lib",
    )
    require(texts[PROMOTION_WORKFLOW], PROMOTION_WORKFLOW, errors,
        'gate_snapshot="$gate_launch_dir/check_kagemusha_production_readiness.sh"',
        'KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH="$KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH"',
        'KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD="$KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD"',
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"',
        "launch-kagemusha-readiness-v1",
        '--gate-snapshot "$gate_snapshot"',
        '--gate-source "$KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH"',
        '--python-runtime-tree-sha256 "$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256"')
    runtime_dispatch = texts[READINESS].rsplit(
        "\nsource_contract_errors: list[str] = []\n", 1
    )[-1]
    source_contract_dispatch, dispatch_separator, self_test_dispatch = (
        runtime_dispatch.partition("\nerrors = source_contract_errors\n")
    )
    if not dispatch_separator:
        errors.append(f"{READINESS}: source-contract dispatch boundary is missing")
    require_pattern(
        source_contract_dispatch,
        READINESS,
        errors,
        (
            r"if mode == \"promotion\":\s*"
            r"source_contract_errors\.extend\(promotion_errors\(\)\)\s*"
            r"source_contract_bytes = authenticated_readiness_source_contract_bytes.*?"
            r"else:\s*try:\s*source_contract_bytes = "
            r"\(root / READINESS_SOURCE_CONTRACT\)\.read_bytes\(\).*?"
            r"len\(source_contract_bytes\) > MAX_READINESS_SOURCE_CONTRACT_BYTES.*?"
            r"source_contract_source = source_contract_bytes\.decode\(\"utf-8\"\).*?"
            r"_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1.*?"
            r"_KAGEMUSHA_READINESS_SOURCE_CONTRACT_SOURCE_V1.*?"
            r"compile\(\s*source_contract_bytes,\s*READINESS_SOURCE_CONTRACT,\s*"
            r"\"exec\",?\s*\).*?"
            r"exec\(code, source_contract_context, source_contract_context\).*?"
            r"source_contract_context\.get\(\"static_errors\"\).*?"
            r"callable\(source_contract_evaluator\).*?"
            r"source_contract_errors\.extend\(source_contract_evaluator\(\)\)"
        ),
        "authenticated byte-only readiness source-contract dispatch",
    )
    if source_contract_dispatch.count(
        "(root / READINESS_SOURCE_CONTRACT).read_bytes()"
    ) != 1:
        errors.append(
            f"{READINESS}: source-contract provider must have exactly one candidate-only path read"
        )
    promotion_source_contract_dispatch = source_contract_dispatch.split(
        "\nelse:\n", 1
    )[0]
    forbid(
        promotion_source_contract_dispatch,
        "promotion source-contract provider dispatch",
        errors,
        "read_bytes",
        "compile(",
        "exec(",
        "runpy.run_path",
        "import_module",
    )
    forbid(
        source_contract_dispatch,
        "readiness source-contract provider dispatch",
        errors,
        "runpy.run_path",
        "import_module",
    )
    require_pattern(
        self_test_dispatch,
        READINESS,
        errors,
        (
            r"if mode == \"promotion\":\s*"
            r"readiness_self_test_bytes = authenticated_readiness_self_test_bytes.*?"
            r"else:\s*try:\s*readiness_self_test_bytes = "
            r"\(root / READINESS_SELF_TEST\)\.read_bytes\(\).*?"
            r"len\(readiness_self_test_bytes\) > MAX_READINESS_SELF_TEST_BYTES.*?"
            r"compile\(\s*readiness_self_test_bytes,\s*READINESS_SELF_TEST,\s*"
            r"\"exec\",?\s*\).*?exec\(code, self_test_context, self_test_context\)"
        ),
        "authenticated byte-only readiness self-test dispatch",
    )
    forbid(
        self_test_dispatch,
        "readiness self-test dispatch",
        errors,
        "runpy.run_path",
        "import_module",
    )
    return errors
