if globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_CONTEXT_V1") is not True:
    raise RuntimeError("detached readiness source contract")
_source_contract_source = globals().get("_KAGEMUSHA_READINESS_SOURCE_CONTRACT_SOURCE_V1")
if not isinstance(_source_contract_source, str) or not _source_contract_source:
    raise RuntimeError("missing readiness source bytes")
_source_support_source = globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1")
if not isinstance(_source_support_source, str) or not _source_support_source:
    raise RuntimeError("missing support bytes")
_source_support_pipeline_errors = globals().get("source_provider_pipeline_errors")
if not callable(_source_support_pipeline_errors):
    raise RuntimeError("missing support checks")
_runtime_projection_source_errors = globals().get("runtime_projection_source_errors")
if not callable(_runtime_projection_source_errors):
    raise RuntimeError("missing runtime checks")
_canary_source_errors = globals().get("canary_source_errors")
if not callable(_canary_source_errors):
    raise RuntimeError("missing canary checks")
_recursion_source_contract_evaluator = globals().get("_KAGEMUSHA_RECURSION_SOURCE_CONTRACT_EVALUATOR_V1")
if not callable(_recursion_source_contract_evaluator):
    raise RuntimeError("missing recursion checks")
_lifecycle_source_contract_source = globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_SOURCE_V1")
if not isinstance(_lifecycle_source_contract_source, str) or not _lifecycle_source_contract_source:
    raise RuntimeError("missing lifecycle bytes")
_lifecycle_source_contract_evaluator = globals().get("_KAGEMUSHA_LIFECYCLE_SOURCE_CONTRACT_EVALUATOR_V1")
if not callable(_lifecycle_source_contract_evaluator):
    raise RuntimeError("missing lifecycle checks")

def read_override(path: str, errors: list[str], overrides: dict[str, str]) -> str:
    return overrides[path] if path in overrides else read(path, errors)

def read_reviewed_model(errors: list[str], overrides: dict[str, str]) -> str:
    if MODEL in overrides:
        return overrides[MODEL]
    parent = read(MODEL, errors)
    component = read_override(MODEL_COMPONENT, errors, overrides)
    verifier = read_override(MODEL_VERIFIER_COMPONENT, errors, overrides)
    if parent.count(MODEL_INCLUDE) != 1:
        errors.append(f"{MODEL}: expected exactly one reviewed {Path(MODEL_COMPONENT).name} include")
        return parent
    parent = parent.replace(MODEL_INCLUDE, component, 1)
    for marker in ("const VERIFIER_IDENTITY_SCHEMA_V4", "pub fn kagemusha_recursive_spend_verifier_key_id_v4"):
        if verifier.count(marker) != 1:
            errors.append(f"{MODEL_VERIFIER_COMPONENT}: expected exactly one {marker!r}")
    for module, relative in (
        (MODEL_VERIFIER_MODULE, MODEL_VERIFIER_COMPONENT),
        (MODEL_PROMOTION_RECEIPT_MODULE, MODEL_PROMOTION_RECEIPT_COMPONENT),
        (
            MODEL_INTERNAL_VALIDATION_RECEIPT_MODULE,
            MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        ),
        (MODEL_CANARY_EVIDENCE_MODULE, MODEL_CANARY_EVIDENCE_COMPONENT),
        (MODEL_CANARY_LIVENESS_MODULE, MODEL_CANARY_LIVENESS_COMPONENT),
    ):
        if parent.count(module) != 1:
            errors.append(f"{MODEL}: expected exactly one reviewed {Path(relative).name} module")
            continue
        name = module.removeprefix("mod ").removesuffix(";")
        source = verifier if relative == MODEL_VERIFIER_COMPONENT else read_override(relative, errors, overrides)
        parent = parent.replace(module, f"mod {name} {{\n{source}\n}}", 1)
    return parent

def read_reviewed_catalog(errors: list[str], overrides: dict[str, str]) -> str:
    if CATALOG in overrides:
        return overrides[CATALOG]
    parent = read(CATALOG, errors)
    if parent.count(CATALOG_INCLUDE) != 1:
        errors.append(f"{CATALOG}: expected exactly one reviewed {Path(CATALOG_COMPONENT).name} include")
        return parent
    parent = parent.replace(CATALOG_INCLUDE, read_override(CATALOG_COMPONENT, errors, overrides), 1)
    if parent.count(CATALOG_VALIDATOR_QUALIFICATION_INCLUDE) != 1:
        errors.append(f"{CATALOG}: expected exactly one reviewed {Path(CATALOG_VALIDATOR_QUALIFICATION_COMPONENT).name} include")
        return parent
    return parent.replace(CATALOG_VALIDATOR_QUALIFICATION_INCLUDE, read_override(CATALOG_VALIDATOR_QUALIFICATION_COMPONENT, errors, overrides), 1)

def read_reviewed_core(errors: list[str], overrides: dict[str, str]) -> str:
    if CORE in overrides:
        return overrides[CORE]
    parent = read(CORE, errors)
    components = (
        (CORE_RUNTIME_EFFECTIVE_CONFIG_MODULE, CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT, "mod kagemusha_runtime_effective_config {\n", "\n}"),
        (CORE_KAGEMUSHA_ACTIVATION_INCLUDE, CORE_KAGEMUSHA_ACTIVATION_COMPONENT, "", ""),
        (CORE_KAGEMUSHA_CANARY_INCLUDE, CORE_KAGEMUSHA_CANARY_COMPONENT, "", ""),
    )
    for marker, relative, prefix, suffix in components:
        if parent.count(marker) != 1:
            errors.append(f"{CORE}: expected exactly one reviewed {Path(relative).name} component")
            continue
        parent = parent.replace(marker, prefix + read_override(relative, errors, overrides) + suffix, 1)
    return parent

def read_reviewed_node(errors: list[str], overrides: dict[str, str]) -> str:
    if NODE in overrides:
        return overrides[NODE]
    parent = read(NODE, errors)
    components = (
        (NODE_VALIDATOR_QUALIFICATION_MODULE, NODE_VALIDATOR_QUALIFICATION_COMPONENT),
        (NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_MODULE, NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT),
        (NODE_VALIDATOR_QUALIFICATION_COMMAND_MODULE, NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT),
        (NODE_ROOT_OWNED_PUBLICATION_MODULE, NODE_ROOT_OWNED_PUBLICATION_COMPONENT),
    )
    for module, relative in components:
        if parent.count(module) != 1:
            errors.append(f"{NODE}: expected exactly one reviewed {Path(relative).name} module")
            continue
        module_name = module.rsplit("mod ", 1)[-1].split(maxsplit=1)[0].rstrip(";{")
        parent = parent.replace(module, f"mod {module_name} {{\n{read_override(relative, errors, overrides)}\n}}", 1)
    return parent

def read_reviewed_authenticated_tool_controller(errors: list[str], overrides: dict[str, str]) -> str:
    parent = read_override(AUTHENTICATED_TOOL_CONTROLLER, errors, overrides)
    components = (
        (KAGEMUSHA_PROMOTION_PUBLISHER_MODULE, KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT),
        (KAGEMUSHA_PYTHON_LAUNCHER_MODULE, KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT),
    )
    for module, relative in components:
        if parent.count(module) != 1:
            errors.append(f"{AUTHENTICATED_TOOL_CONTROLLER}: expected exactly one reviewed {Path(relative).name} module")
            continue
        module_name = module.rsplit("mod ", 1)[-1].rstrip(";")
        parent = parent.replace(module, f"mod {module_name} {{\n{read_override(relative, errors, overrides)}\n}}", 1)
    return parent

def read_reviewed_offline_cli(errors: list[str], overrides: dict[str, str]) -> str:
    if OFFLINE_CLI in overrides:
        return overrides[OFFLINE_CLI]
    parent = read(OFFLINE_CLI, errors)
    if parent.count(KAGEMUSHA_ROLLOUT_MODULE) != 1:
        errors.append(f"{OFFLINE_CLI}: expected exactly one reviewed {Path(KAGEMUSHA_ROLLOUT_COMPONENT).name} module")
        return parent
    return parent.replace(KAGEMUSHA_ROLLOUT_MODULE, "mod kagemusha_rollout {\n" + read_override(KAGEMUSHA_ROLLOUT_COMPONENT, errors, overrides) + "\n}", 1)

def static_errors(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []
    overrides = overrides or {}
    try:
        errors += _recursion_source_contract_evaluator(
            root, overrides, require_shipping_backend=mode == "promotion"
        )
    except Exception as error:
        errors.append(f"recursion source contract failed: {error}")
    try:
        errors += _lifecycle_source_contract_evaluator(root, overrides)
    except Exception as error:
        errors.append(f"lifecycle source contract failed: {error}")
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
            CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT,
            CORE_KAGEMUSHA_ACTIVATION_COMPONENT,
            CORE_KAGEMUSHA_CANARY_COMPONENT,
            CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT,
            *DEVICE_ATTESTATION_SOURCE_PATHS,
            CORE_TX, CORE_STATE, CORE_COMMITTED_TX_CONTEXT,
            CORE_BLOCK, CORE_EXECUTOR,
            CORE_ISI_MOD,
            STEP_TRANSITION,
            RECURSIVE_BACKEND,
            RECURSION_ADAPTER,
            VALUE_CONTRACT,
            SCHEMA_GOLDEN,
            CONFIG,
            NODE,
            NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT,
            KAGAMI,
            AUTHENTICATED_TOOL_CONTROLLER,
            KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
            KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
            OFFLINE_CLI,
            KAGEMUSHA_ROLLOUT_COMPONENT,
            KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT,
            MODEL_PROMOTION_RECEIPT_COMPONENT,
            MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
            MODEL_CANARY_EVIDENCE_COMPONENT,
            MODEL_CANARY_LIVENESS_COMPONENT,
            MODEL_ISI_OFFLINE,
            MODEL_ISI_MOD,
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
    texts[READINESS_SOURCE_SUPPORT] = overrides.get(
        READINESS_SOURCE_SUPPORT, _source_support_source
    )
    texts[READINESS_LIFECYCLE_SOURCE_CONTRACT] = overrides.get(
        READINESS_LIFECYCLE_SOURCE_CONTRACT, _lifecycle_source_contract_source
    )
    errors += _source_support_pipeline_errors(texts[READINESS])
    texts[MODEL] = read_reviewed_model(errors, overrides)
    texts[CATALOG] = read_reviewed_catalog(errors, overrides)
    texts[CORE] = read_reviewed_core(errors, overrides)
    texts[NODE] = read_reviewed_node(errors, overrides)
    texts[AUTHENTICATED_TOOL_CONTROLLER] = (
        read_reviewed_authenticated_tool_controller(errors, overrides)
    )
    texts[OFFLINE_CLI] = read_reviewed_offline_cli(errors, overrides)
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
        "def read_reviewed_model(",
        "def read_reviewed_catalog(",
        "def read_reviewed_core(",
        "def read_reviewed_node(",
        "def read_reviewed_authenticated_tool_controller(",
        "def read_reviewed_offline_cli(",
        "def static_errors(",
        'require_shipping_backend=mode == "promotion"',
        "_lifecycle_source_contract_evaluator(root, overrides)",
    )
    require(
        texts[READINESS_SOURCE_SUPPORT],
        READINESS_SOURCE_SUPPORT,
        errors,
        'globals().get("_KAGEMUSHA_READINESS_SOURCE_SUPPORT_CONTEXT_V1") is not True',
        '"_KAGEMUSHA_READINESS_SOURCE_SUPPORT_SOURCE_V1"',
        "def source_provider_pipeline_errors(",
        'MODEL = "crates/iroha_data_model/src/offline/mod.rs"',
        "MODEL_PROMOTION_RECEIPT_COMPONENT",
        "MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT",
        "MODEL_CANARY_EVIDENCE_COMPONENT",
        "MODEL_CANARY_LIVENESS_COMPONENT",
        "MODEL_ISI_OFFLINE",
        "MODEL_ISI_MOD",
        "CATALOG_VALIDATOR_QUALIFICATION_COMPONENT",
        "CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT",
        "CORE_KAGEMUSHA_ACTIVATION_COMPONENT",
        "CORE_KAGEMUSHA_CANARY_COMPONENT",
        "CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT",
        "POLICY_TESTS", "POLICY_TESTS_INCLUDE", "QUAL_TESTS",
        "ANDROID_AUTH", "ANDROID_AUTH_INCLUDE",
        "ANDROID_CERT", "ANDROID_CERT_FIX", "ANDROID_CERT_TEST",
        "DEVICE_ATTESTATION_SOURCE_PATHS",
        "device_attestation_governance_source_errors",
        "CORE_TX", "CORE_STATE", "CORE_STATE_TESTS", "CORE_COMMITTED_TX_CONTEXT",
        "CORE_BLOCK", "CORE_EXECUTOR",
        "CORE_ISI_MOD",
        "NODE_VALIDATOR_QUALIFICATION_COMPONENT",
        "NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT",
        "NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_MODULE",
        "runtime_projection_source_errors",
        "canary_source_errors",
        "AUTHENTICATED_TOOL_CONTROLLER",
        "KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT",
        "KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT",
        "KAGEMUSHA_ROLLOUT_COMPONENT",
        "KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT",
        "KAGEMUSHA_RELEASE_PYTHON_TEST_PATHS",
    )
    recursion_bootstrap = texts[READINESS_SOURCE_CONTRACT].split(
        "def static_errors(", 1
    )[-1].split("    texts = {", 1)[0]
    forbid(
        recursion_bootstrap,
        "readiness recursion source-contract bootstrap",
        errors,
        "read(",
        "read_override(",
        "compile(",
        "exec(",
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
        "MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT: read(",
        "MODEL_CANARY_EVIDENCE_COMPONENT: read(",
        "MODEL_CANARY_LIVENESS_COMPONENT: read(",
        "MODEL_ISI_OFFLINE: read(",
        "READINESS_LIFECYCLE_SOURCE_CONTRACT: read(",
        "LIFECYCLE_SOURCE_PATHS",
        "CORE_REDEMPTION_POLICY_TESTS",
        "authenticated lifecycle source-provider boundary",
        "CORE_KAGEMUSHA_CANARY_COMPONENT: read(",
        "CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT: read(",
        "*ATTESTATION_STATIC_MUTATIONS",
        "CORE_TX: read(", "CORE_STATE: read(", "CORE_STATE_TESTS: read(",
        "CORE_COMMITTED_TX_CONTEXT: read(",
        "CORE_BLOCK: read(", "CORE_EXECUTOR: read(",
        "CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT: read(",
        "NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT: read(",
        "KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT: read(",
        "KAGEMUSHA_ROLLOUT_COMPONENT: read(",
        "KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT: read(",
        "D_CANARY_MARKER",
        "D_EXPECTATIONS",
        "D_WIRE_BOUNDARY",
        "ambient Client enters direct validator collection",
        "direct validator collection transport isolation",
        "configured-or-60s direct client with non-expanding status timeout",
        "direct validator status bounded exact canonical scalar",
        "bounded identity-encoded direct validator status response",
        "direct validator attestation requires exact three protocol headers",
        "authenticate the promotion Python-launcher module wiring",
        "reject raw runtime projections at the production signer",
        "reject replace-capable canary publication",
        "reject blocking promotion inventory opens",
        "reject a report scalar detached from the canonical manifest",
        "reject replace-capable promotion-record publication",
        "reject a preliminary committed-record stdout line",
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
    internal_validation = texts[MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT]
    require_pattern(
        internal_validation,
        MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        errors,
        (
            r"pub fn decode_canonical\(.*?decode_canonical_with_limits\(bytes, limits\).*?"
            r"receipt\.validate\(\)\?;.*?if canonical != bytes.*?Ok\(receipt\).*?"
            r"pub fn validate\(&self\).*?self\.body\.validate\(\)\?;.*?"
            r"self\.signature\s*\.verify\(&self\.body\.validation_runner_public_key, "
            r"&self\.body\).*?InvalidSignature"
        ),
        "internal-validation receipt canonical signature/body validation",
    )
    require_pattern(
        internal_validation,
        MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        errors,
        (
            r"impl KagemushaRecursiveSpendInternalValidationReceiptBodyV1.*?"
            r"pub fn validate\(&self\).*?self\.validate_identity\(\)\?;.*?"
            r"self\.validate_tools\(\)\?;.*?self\.validate_commands\(\).*?"
            r"fn validate_commands\(&self\).*?self\.commands\.len\(\).*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_REQUIRED_COMMANDS_V1\.len\(\).*?"
            r"command\.command_id != spec\.command_id.*?!argv_matches.*?"
            r"command\.exit_code != 0.*?command\.termination_signal\.is_some\(\).*?"
            r"command\.timed_out.*?fuzz_targets != \[true, true\]"
        ),
        "internal-validation exact command outcomes",
    )
    require_pattern(
        internal_validation,
        MODEL_INTERNAL_VALIDATION_RECEIPT_COMPONENT,
        errors,
        (
            r'command_id: "core-final-release-inventory".*?program: CARGO,.*?'
            r'argv: &\[\s*"test",\s*"--locked",\s*"-p",\s*"iroha_core",\s*'
            r'"--features",\s*"dev-tools,zk-halo2-ipa,kagemusha-candidate-evidence-lab",\s*'
            r'"--bin",\s*"kagemusha_recursive_spend_v4_bundle",\s*'
            r'"final_release_inventory_is_exact_and_includes_both_receipts",\s*\],\s*'
            r"fuzz_target: None"
        ),
        "exact internal-validation final-inventory command",
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
    promotion_reservation_impl = model.split(
        "impl KagemushaV4PromotionReservationV1 {", 1
    )[-1].split("/// Shared controller, reservation, release, policy", 1)[0]
    require_pattern(
        promotion_reservation_impl,
        MODEL_PROMOTION_RECEIPT_COMPONENT,
        errors,
        (
            r"pub fn decode_canonical\(.*?"
            r"check_artifact_input_size\(\s*bytes,\s*"
            r"KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,\s*"
            r"ArtifactKind::PromotionReservation,\s*\)\?;.*?"
            r"norito::decode_canonical_with_limits\(\s*bytes,\s*"
            r"artifact_decode_limits\(\s*"
            r"KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,\s*bytes\.len\(\),?\s*"
            r"\),?\s*\).*?reservation\.body\.validate_structure\(\)\?;.*?"
            r"pub fn decode_and_verify_canonical\(.*?"
            r"Self::decode_canonical\(bytes\)\?;.*?"
            r"reservation\.verify\(pinned_controller\)\?;"
        ),
        "reservation decode",
    )
    receipt_validation = texts[CATALOG].split(
        "fn validate_exact_catalog_revalidation_receipt_v1(", 1
    )[-1].split("fn validate_validator_qualification_freshness_at_v1(", 1)[0]
    require_pattern(
        receipt_validation,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"exact_receipt_json\.is_empty\(\).*?"
            r"exact_receipt_json\.len\(\)\s*>\s*"
            r"KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES.*?"
            r"catalog_revalidation_receipt_json\s*\.matches_bytes\("
            r"exact_receipt_json\).*?"
            r"norito::json::from_slice\(exact_receipt_json\).*?"
            r"canonical_json_bytes_v1\(&value\)\?\s*!=\s*exact_receipt_json.*?"
            r"json_object_has_exact_fields_v1\(\s*object,\s*"
            r"&KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_FIELDS_V1,\s*\)"
        ),
        "catalog receipt decode",
    )
    receipt_schema = texts[CATALOG].split(
        "const KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_SCHEMA_V1", 1
    )[-1].split("#[derive(Clone, Debug, PartialEq, Eq)]", 1)[0]
    require_pattern(
        receipt_schema,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"ED25519_SUBJECT_PUBLIC_KEY_INFO_DER_PREFIX_V1:\s*\[u8;\s*12\]\s*=\s*\[\s*"
            r"0x30,\s*0x2a,\s*0x30,\s*0x05,\s*0x06,\s*0x03,\s*"
            r"0x2b,\s*0x65,\s*0x70,\s*0x03,\s*0x21,\s*0x00,\s*\];.*?"
            r"KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_FIELDS_V1:\s*"
            r"\[&str;\s*14\]\s*=\s*\[\s*"
            r'"catalog_sha256",\s*"expires_at_unix_ms",\s*'
            r'"issued_at_unix_ms",\s*"promotion_id",\s*"receipt_id",\s*'
            r'"release_statuses",\s*"schema",\s*"signature",\s*'
            r'"signature_algorithm",\s*"signature_payload_sha256",\s*'
            r'"signer_key_id",\s*"signer_public_key_sha256",\s*'
            r'"status",\s*"version",\s*\];'
        ),
        "receipt fields/SPKI",
    )
    authority_helpers = texts[CATALOG].split(
        "fn valid_catalog_revalidation_key_id_v1(", 1
    )[-1].split("fn canonical_json_bytes_v1(", 1)[0]
    require_pattern(
        authority_helpers,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"bytes\.len\(\)\s*<=\s*128.*?"
            r"bytes\[0\]\.is_ascii_alphanumeric\(\).*?"
            r"matches!\(\*byte,\s*b'\.'\s*\|\s*b'_'\s*\|\s*b'-'\).*?"
            r"fn catalog_revalidation_authority_spki_sha256_v1\(.*?"
            r"algorithm\s*!=\s*Algorithm::Ed25519\s*\|\|\s*"
            r"raw_public_key\.len\(\)\s*!=\s*32.*?"
            r"hasher\.update\(ED25519_SUBJECT_PUBLIC_KEY_INFO_DER_PREFIX_V1\).*?"
            r"hasher\.update\(raw_public_key\)"
        ),
        "authority id/SPKI digest",
    )
    require_pattern(
        authority_helpers,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"json_required_string_v1\(object,\s*\"signer_key_id\"\)\?\s*"
            r"!=\s*trusted_authority_key_id.*?"
            r"catalog_revalidation_authority_spki_sha256_v1\("
            r"trusted_authority_public_key\)\?;.*?"
            r"json_required_sha256_v1\(object,\s*\"signer_public_key_sha256\"\)\?\s*"
            r"!=\s*expected_spki_sha256.*?"
            r"json_required_string_v1\(object,\s*\"signature_algorithm\"\)\?\s*"
            r"!=\s*\"ed25519\".*?signature_text\.len\(\)\s*!=\s*128.*?"
            r"hex::decode_to_slice\(signature_text,\s*&mut signature_bytes\).*?"
            r"iroha_crypto::ed25519_parse_signature\(&signature_bytes\).*?"
            r"\.verify\(trusted_authority_public_key,\s*signature_payload\)"
        ),
        "catalog authority signature",
    )
    require_pattern(
        texts[CATALOG],
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"fn catalog_revalidation_signature_payload_v1\(.*?"
            r"let mut unsigned\s*=\s*object\.clone\(\);\s*"
            r'unsigned\.remove\("signature"\);\s*'
            r'unsigned\.remove\("signature_payload_sha256"\);.*?'
            r"norito::json::to_string\(&norito::json::Value::Object\(unsigned\)\).*?"
            r"\.map\(String::into_bytes\).*?"
            r"fn canonical_json_bytes_v1\(.*?"
            r"norito::json::to_string\(value\).*?\.into_bytes\(\);\s*"
            r"canonical\.push\(b'\\n'\);\s*Ok\(canonical\)"
        ),
        "canonical receipt payload",
    )
    require_pattern(
        receipt_validation,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"signature_payload_sha256\s*=\s*"
            r"json_required_sha256_v1\(object,\s*\"signature_payload_sha256\"\)\?;.*?"
            r"catalog_revalidation_signature_payload_v1\(object\)\?;.*?"
            r"Sha256::digest\(&signature_payload\).*?"
            r"validate_catalog_revalidation_authority_v1\(\s*object,\s*"
            r"&signature_payload,\s*trusted_authority_key_id,\s*"
            r"trusted_authority_public_key,\s*\)\?;"
        ),
        "payload digest ordering",
    )
    signing_boundary = texts[CATALOG].split(
        "fn build_and_sign_validator_qualification_seal_v1(", 1
    )[-1].split(
        "pub fn build_and_sign_validator_qualification_from_reservation_v1(", 1
    )[0]
    require_pattern(
        signing_boundary,
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        (
            r"build_validator_qualification_body_from_verified_release_v1\(.*?"
            r"validate_validator_qualification_body_matches_reservation_v1\("
            r"&body,\s*reservation\)\?;.*?"
            r"self\.seal\s*\.validate_for_configured_runtime\("
            r"&self\.policy_path,\s*&self\.artifact_dir\)\?;.*?"
            r"verify_kagemusha_catalog_sealed_paths_v1\(&self\.seal\.paths,\s*0\)\?;.*?"
            r"current_unix_time_ms_v1\(\)\?;.*?"
            r"validate_validator_qualification_freshness_at_v1\("
            r"subject,\s*current_time_ms\)\?;.*?"
            r"KagemushaV4ValidatorQualificationSealV1::try_sign\(body,\s*validator_signer\)"
        ),
        "final signing recheck",
    )
    offline_parse = texts[CONFIG].split("impl Offline {", 1)[-1].split(
        "impl Router {", 1
    )[0]
    require_pattern(
        offline_parse,
        CONFIG,
        errors,
        (
            r"let validator_qualification_inputs\s*=\s*\[\s*"
            r"kagemusha_promotion_controller_public_key\.is_some\(\),\s*"
            r"kagemusha_catalog_revalidation_authority_key_id\.is_some\(\),\s*"
            r"kagemusha_catalog_revalidation_authority_public_key\.is_some\(\),\s*"
            r"kagemusha_promotion_reservation_path\.is_some\(\),\s*"
            r"kagemusha_validator_qualification_seal_path\.is_some\(\),\s*"
            r"\].*?\.count\(\);.*?"
            r"if validator_qualification_inputs\s*!=\s*0\s*&&\s*"
            r"validator_qualification_inputs\s*!=\s*5.*?"
            r"if validator_qualification_inputs\s*!=\s*0\s*&&\s*"
            r"kagemusha_catalog_qualification_seal_path\.is_none\(\)"
        ),
        "qualification config completeness",
    )
    require_pattern(
        offline_parse,
        CONFIG,
        errors,
        (
            r"kagemusha_catalog_revalidation_authority_public_key.*?"
            r"is_some_and\(\|key\|\s*!matches!\("
            r"key\.try_algorithm\(\),\s*Ok\(Algorithm::Ed25519\)\)\).*?"
            r"kagemusha_catalog_revalidation_authority_key_id.*?"
            r"bytes\.is_empty\(\).*?bytes\.len\(\)\s*>\s*128.*?"
            r"!bytes\[0\]\.is_ascii_alphanumeric\(\).*?"
            r"matches!\(\*byte,\s*b'\.'\s*\|\s*b'_'\s*\|\s*b'-'\)"
        ),
        "authority key/id shape",
    )
    qualification_source = texts[NODE].split(
        "mod kagemusha_validator_qualification {", 1
    )[-1].split("\n}\n/// Root-custodied inputs", 1)[0]
    qualification_command = texts[NODE].split(
        "mod kagemusha_validator_qualification_command {", 1
    )[-1].split("\n}\n/// Deployment-injected factory", 1)[0]
    publication_source = texts[NODE].split(
        "mod root_owned_artifact_publication {", 1
    )[-1].split("\n}\n/// Platform-fixed local runtime-provider", 1)[0]
    qcomp = NODE_VALIDATOR_QUALIFICATION_COMPONENT
    qccomp = NODE_VALIDATOR_QUALIFICATION_COMMAND_COMPONENT
    pcomp = NODE_ROOT_OWNED_PUBLICATION_COMPONENT
    require_pattern(
        publication_source,
        pcomp,
        errors,
        r"flistxattr\(\s*opened\.as_raw_fd\(\),\s*std::ptr::null_mut\(\),\s*0,\s*MACOS_XATTR_SHOWCOMPRESSION",
        "macOS hidden-xattr query",
    )
    require(
        texts[CATALOG],
        CATALOG_VALIDATOR_QUALIFICATION_COMPONENT,
        errors,
        "metadata.mode != SumeragiConsensusMode::Permissioned",
    )
    errors += _runtime_projection_source_errors(
        texts[CORE_RUNTIME_EFFECTIVE_CONFIG_COMPONENT],
        texts[NODE_RUNTIME_EFFECTIVE_CONFIG_PROJECTION_COMPONENT],
        texts[NODE], texts[CATALOG], texts[MODEL],
    )
    errors += _canary_source_errors(
        texts[MODEL_CANARY_EVIDENCE_COMPONENT], texts[MODEL_CANARY_LIVENESS_COMPONENT],
        texts[KAGEMUSHA_ROLLOUT_COMPONENT], texts[KAGEMUSHA_ROLLOUT_LIVENESS_COMPONENT],
        texts[MODEL_PROMOTION_RECEIPT_COMPONENT], texts[MODEL_ISI_OFFLINE],
        texts[MODEL_ISI_MOD], texts[CORE], texts[CORE_KAGEMUSHA_CANARY_COMPONENT],
        texts[CORE_ATTESTATION_CERTIFICATE_VALIDATION_COMPONENT], texts[CORE_ISI_MOD],
        texts[CORE_TX], texts[CORE_STATE], texts[CORE_COMMITTED_TX_CONTEXT],
        texts[CORE_BLOCK], texts[CORE_EXECUTOR],
    )
    errors += device_attestation_governance_source_errors(texts)
    errors += release_closure_source_errors(
        texts[CORE], texts[SCHEMA_GOLDEN], texts[WORKFLOW], overrides
    )
    require_pattern(
        qualification_command,
        qccomp,
        errors,
        (
            r"let exact_bytes\s*=\s*read\(\s*path,\s*"
            r"KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,.*?\)\?;.*?"
            r"KagemushaV4PromotionReservationV1::decode_and_verify_canonical\("
            r"&exact_bytes,\s*controller\).*?"
            r"kagemusha_catalog_revalidation_receipt_path_v1\("
            r"reservation\.body\.promotion_id\).*?"
            r"let catalog_revalidation_receipt_json\s*=\s*read\(\s*&receipt_path,\s*"
            r"KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES,.*?\)\?;.*?"
            r"catalog_revalidation_receipt_json\s*\.matches_bytes\("
            r"&catalog_revalidation_receipt_json\)"
        ),
        "reservation/receipt custody",
    )
    require_pattern(
        qualification_command,
        qccomp,
        errors,
        (
            r"KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1:\s*&str\s*=\s*"
            r'"/Library/SORA/Kagemusha/catalog-revalidation";.*?'
            r'#\[cfg\(target_os\s*=\s*"macos"\)\].*?'
            r"read_configured_kagemusha_promotion_reservation\(.*?"
            r"RootOwnedNoReplaceArtifactPublicationTarget::read_root_owned_bounded\("
            r"path,\s*maximum,\s*label\).*?"
            r'#\[cfg\(not\(target_os\s*=\s*"macos"\)\)\].*?'
            r"read_configured_kagemusha_promotion_reservation\(.*?"
            r"unsupported outside macOS until a platform-specific root-custody path is reviewed.*?"
            r"fn kagemusha_catalog_revalidation_receipt_path_v1\(.*?"
            r"Path::new\(KAGEMUSHA_CATALOG_REVALIDATION_RECEIPT_ROOT_V1\).*?"
            r'\.join\(format!\("\{\}\.json",\s*hex::encode\(promotion_id\)\)\)'
        ),
        "fixed receipt path/platform gate",
    )
    require_pattern(
        qualification_source,
        qcomp,
        errors,
        (
            r"struct KagemushaTrustedPromotionInputsV1.*?"
            r"pinned_controller:\s*&'a PublicKey,.*?"
            r"exact_reservation_bytes:\s*&'a \[u8\],.*?"
            r"catalog_revalidation_receipt_json:\s*&'a \[u8\],.*?"
            r"catalog_revalidation_authority_key_id:\s*&'a str,.*?"
            r"catalog_revalidation_authority_public_key:\s*&'a PublicKey,.*?"
            r"build_and_sign_validator_qualification_from_reservation_v1\(\s*"
            r"promotion\.exact_reservation_bytes,\s*promotion\.pinned_controller,\s*"
            r"promotion\.catalog_revalidation_receipt_json,\s*"
            r"promotion\.catalog_revalidation_authority_key_id,\s*"
            r"promotion\.catalog_revalidation_authority_public_key,"
        ),
        "trusted promotion forwarding",
    )
    require_pattern(
        qualification_source,
        qcomp,
        errors,
        (
            r"fn evaluate_stock_launcher_unavailable_v1\(.*?"
            r"try_build_kagemusha_validator_qualification_v1\(\s*"
            r"sources,\s*None,\s*None,\s*genesis,\s*None,\s*validator_id,\s*"
            r"Some\(validator_signer\),\s*\)\?.*?"
            r"KagemushaValidatorQualificationOutcomeV1::Unavailable\(reason\)\s*=>\s*\{\s*"
            r"require_expected_stock_launcher_unavailable_reason_v1\(reason\)\s*\}.*?"
            r"KagemushaValidatorQualificationOutcomeV1::Signed\(_\)\s*=>\s*Err\(\s*"
            r'"stock launcher unexpectedly signed a Kagemusha validator qualification '
            r'without trusted promotion inputs"\s*\.to_owned\(\),\s*\),.*?'
            r"fn require_expected_stock_launcher_unavailable_reason_v1\(.*?"
            r"match reason\s*\{\s*"
            r"KagemushaValidatorQualificationUnavailableV1::SnapshotBootstrap\s*\|\s*"
            r"KagemushaValidatorQualificationUnavailableV1::MissingTrustedPromotionReservation\s*"
            r"=>\s*\{\s*Ok\(\(\)\)\s*\}\s*"
            r"reason\s*=>\s*Err\(format!\(\s*"
            r'"stock launcher returned an unexpected Kagemusha qualification outcome: '
            r'\{reason:\?\}"\s*\)\),'
        ),
        "stock-launcher fail-closed qualification outcome",
    )
    check_config_branch = texts[NODE].split("if args.startup.check_config {", 1)[
        -1
    ].split("// Resolve deployment-owned executable providers", 1)[0]
    require_pattern(
        check_config_branch,
        NODE,
        errors,
        (
            r"KagemushaValidatorSealPublicationTarget::prepare\(.*?"
            r"read_configured_kagemusha_promotion_reservation\(.*?"
            r"validate_config_for_check_mode\(&config,\s*genesis\.as_ref\(\),\s*mode\)\?;.*?"
            r"if let Some\(target\)\s*=\s*validator_seal_target\s*\{.*?"
            r"target\.publish_and_verify\(&seal\)"
        ),
        "seal action ordering",
    )
    require_pattern(
        qualification_command,
        qccomp,
        errors,
        (
            r"pub\(super\) fn publish_and_verify\(.*?seal\.verify\(\).*?"
            r"norito::encode_canonical\(seal\).*?"
            r"self\.inner\s*\.publish_bytes_and_verify\(&canonical,"
        ),
        "seal verification before publish",
    )
    require_pattern(
        publication_source,
        pcomp,
        errors,
        (
            r"pub\(super\) fn publish_bytes_and_verify\(.*?"
            r"write_all\(canonical_bytes\).*?"
            r"if readback\s*!=\s*canonical_bytes.*?"
            r"rustix::fs::renameat_with\(\s*&self\.parent,\s*&staging_name,\s*"
            r"&self\.parent,\s*&self\.file_name,\s*"
            r"rustix::fs::RenameFlags::NOREPLACE,\s*\).*?"
            r"irreversible commit boundary.*?verify_final\(&self\.path\)\?;"
        ),
        "no-replace commit protocol",
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
        "Self::Candidate => 17",
        "Self::Promoted => 18",
        "KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1",
        "if inventory_state.includes_promotion_record() && expected.len() != 18",
        "ActivateKagemushaRecursiveReleaseV4::new(",
        "args.runtime_effective_config_sha256",
        r'instruction_count\":1',
    )
    require(
        texts[KAGAMI],
        KAGAMI,
        errors,
        '#[command(name = "prepare-enable-issuance-v4")]',
        '#[command(name = "prepare-cancel-release-v4")]',
        '#[command(name = "prepare-deactivate-issuance-v4")]',
        "fn lifecycle_terminal_commands_publish_exact_typed_instructions_and_reports()",
        "fn lifecycle_commands_reject_tampered_noncanonical_oversized_and_malformed_inputs()",
        "fn lifecycle_command_refuses_to_replace_existing_output()",
        "assert_eq!(\n            report_lines.len(),",
        '"durability record and preparation report"',
        'let error = outcome.expect_err("existing lifecycle output must never be replaced");',
        'b"operator-reviewed sentinel"',
    )
    for command, source, maximum, model, constructor in (
        (
            "PrepareEnableIssuanceV4",
            "enable_witness",
            "KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1",
            "KagemushaV4IssuanceEnableWitnessV1",
            "EnableKagemushaRecursiveIssuanceV4",
        ),
        (
            "PrepareCancelReleaseV4",
            "cancellation",
            "KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1",
            "KagemushaV4ReleaseCancellationV1",
            "CancelKagemushaRecursiveReleaseV4",
        ),
        (
            "PrepareDeactivateIssuanceV4",
            "deactivation",
            "KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1",
            "KagemushaV4ReleaseDeactivationV1",
            "DeactivateKagemushaRecursiveIssuanceV4",
        ),
    ):
        require_pattern(
            texts[KAGAMI],
            KAGAMI,
            errors,
            (
                rf"Command::{command}\(args\)\s*=>\s*\{{.*?"
                rf"read_external_bounded\(\s*&args\.{source},\s*{maximum},.*?"
                rf"{model}::decode_canonical\(&bytes\).*?"
                rf"prepare_lifecycle_instruction_v4\(.*?"
                rf"InstructionBox::from\({constructor}::new\("
            ),
            f"bounded canonical {command} lifecycle preparation",
        )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"fn prepare_lifecycle_instruction_v4<.*?"
            r"let instructions = vec!\[instruction\];.*?"
            r"let instructions_hash = HashOf::new\(&instructions\);.*?"
            r"norito::json::to_string\(&instructions\).*?"
            r"publish_new_durable_file\(writer, output, instruction_json\.as_bytes\(\)\)\?;.*?"
            r'instruction_count\\":1.*?input_sha256'
        ),
        "one-instruction no-replace lifecycle publication",
    )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"fn verify_exact_inventory_v4\(.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1.*?"
            r"if inventory_state\.includes_promotion_record\(\) && expected\.len\(\) != 18.*?"
            r"fn recursive_step_verifier_commitment_v4\("
        ),
        "18-file verifier inventory with both validation receipts",
    )
    authenticated_controller = texts[AUTHENTICATED_TOOL_CONTROLLER]
    promotion_publisher = texts[KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT]
    python_launcher = texts[KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT]
    require_pattern(
        authenticated_controller,
        AUTHENTICATED_TOOL_CONTROLLER,
        errors,
        (
            r"fn entrypoint\(arguments: Vec<OsString>\).*?"
            r'match arguments\[1\]\.to_str\(\).*?'
            r'Some\("promote-kagemusha-release-v4"\)\s*=>\s*\{\s*'
            r"kagemusha_promotion_publisher::promote\(&arguments\[2\.\.\]\)\s*"
            r"\}.*?_\s*=>\s*Err\(ControllerError::policy\("
            r'"unsupported subcommand"\)\)'
        ),
        "exact authenticated promotion-publisher controller dispatch",
    )
    publisher_parse = promotion_publisher.split(
        "fn parse_request(arguments: &[OsString])", 1
    )[-1].split("fn normalized_absolute_path(", 1)[0]
    require_pattern(
        publisher_parse,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"const OPTIONS: \[&str; 5\] = \[\s*"
            r'"--expected-macos-build",\s*"--kagami",\s*'
            r'"--kagami-sha256",\s*"--bundle-dir",\s*'
            r'"--release-policy",\s*\];.*?'
            r"if arguments\.len\(\) != OPTIONS\.len\(\) \* 2.*?"
            r"for \(index, option\) in OPTIONS\.into_iter\(\)\.enumerate\(\).*?"
            r"if arguments\[index \* 2\] != option.*?"
            r"let value = arguments\[index \* 2 \+ 1\].*?"
            r"expected_macos_build: expected_macos_build\.to_owned\(\),.*?"
            r"kagami: normalized_absolute_path\(values\[\"--kagami\"\]\)\?,.*?"
            r"kagami_sha256: parse_sha256\(values\[\"--kagami-sha256\"\],"
            r'\s*"Kagami SHA-256"\)\?,.*?'
            r"bundle_dir: normalized_absolute_path\(values\[\"--bundle-dir\"\]\)\?,.*?"
            r"release_policy: normalized_absolute_path\(values\[\"--release-policy\"\]\)\?"
        ),
        "exact ordered promotion-controller argument contract",
    )
    root_identity = python_launcher.split(
        "pub(crate) fn validate_root_launch_identity()", 1
    )[-1].split("pub(super) fn observed_macos_build()", 1)[0]
    require_pattern(
        root_identity,
        KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        errors,
        (
            r"effective_uid\(\) != 0\s*\|\|\s*effective_gid\(\) != 0\s*"
            r"\|\|\s*unsafe \{ getuid\(\) \} != 0\s*"
            r"\|\|\s*unsafe \{ getgid\(\) \} != 0\s*"
            r"\|\|\s*unsafe \{ issetugid\(\) \} != 0.*?"
            r"validate_no_inherited_fds\(\)\?;"
        ),
        "real and effective non-set-id root identity",
    )
    macos_tcb = python_launcher.split(
        "pub(crate) fn require_macos_tcb(expected: &str)", 1
    )[-1].split("pub(crate) fn require_root_custody(", 1)[0]
    require_pattern(
        macos_tcb,
        KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        errors,
        (
            r"observed_macos_build\(\)\? != expected.*?"
            r"for root in OS_LIBRARY_ROOTS\s*\{\s*"
            r"require_root_custody\(Path::new\(root\), true\)\?;\s*\}.*?"
            r'require_root_custody\(Path::new\("/bin/bash"\), false\)\?;.*?'
            r"Ok\(os_tcb_digest\(expected\)\)"
        ),
        "exact macOS build and OS-library TCB custody",
    )
    require(
        promotion_publisher,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        'const SNAPSHOT_PARENT: &str = "/private/var/db/iroha-kagemusha-promotion-v1";',
        'const FINAL_NAME: &str = "promotion-record-v4.norito";',
        'const TEMP_PREFIX: &str = ".promotion-record-v4.norito.tmp.";',
        "const COMMIT_UNCERTAIN_EXIT: u8 = 75;",
        "const CANDIDATE_FILES: [CandidateFileSpec; 17] = [",
        "fn canonical_report(stdout: &[u8], expected: &CanonicalReportV4)",
        "fn identity_from_file_checked(",
        "fn open_member(",
        "struct PinnedInput",
        "struct CandidateSnapshot",
        "struct ExecutableSnapshot",
        "fn sandbox_profile(",
        "fn promote_macos(request: PromotionRequest)",
    )
    snapshot_source = promotion_publisher.split(
        "struct ExecutableSnapshot", 1
    )[-1].split("fn seatbelt_literal(", 1)[0]
    require_pattern(
        snapshot_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"fn create\(parent_path: &Path\).*?"
            r"let parent = open_directory\(parent_path\)\?;.*?"
            r'let directory = parent_path\.join\("active"\);.*?'
            r"fs::DirBuilder::new\(\)\s*\.mode\(0o700\)\s*\.create\(&directory\).*?"
            r'path: directory\.join\("kagami"\).*?'
            r"fn create\(\s*source: &mut kagemusha_python_launcher::PinnedFile,\s*"
            r"expected_sha256: \[u8; 32\],.*?"
            r"require_root_custody\(parent_path, true\)\?;.*?"
            r"fs::set_permissions\(&staging\.directory, fs::Permissions::from_mode\(0o700\)\).*?"
            r"OFlags::WRONLY\s*\|\s*OFlags::CREATE\s*\|\s*OFlags::EXCL\s*"
            r"\|\s*OFlags::NOFOLLOW\s*\|\s*OFlags::CLOEXEC.*?"
            r"output\s*\.sync_all\(\).*?run\.sync_all\(\).*?"
            r"staging\s*\.parent.*?\.sync_all\(\).*?"
            r"pin_regular\(&staging\.path, expected_sha256\)\?;.*?"
            r"validate_pinned\(source\)\?;.*?"
            r"fn cleanup\(mut self\).*?self\.verify\(\)\?;.*?"
            r"fs::remove_file\(&self\.path\).*?sync_all\(\).*?"
            r"fs::remove_dir\(&self\.directory\).*?self\.parent\.sync_all\(\)"
        ),
        "fixed owner-private pinned Kagami executable snapshot",
    )
    pinned_file_access = python_launcher.split(
        "#[derive(Debug)]\n    pub(crate) struct PinnedFile", 1
    )[-1].split("#[derive(Clone, Copy, Debug, Eq, PartialEq)]", 1)[0]
    require_pattern(
        pinned_file_access,
        KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        errors,
        (
            r"\{\s*path: PathBuf,\s*file: File,\s*stable: StableMetadata,\s*"
            r"sha256: \[u8; 32\],\s*\}\s*"
            r"impl PinnedFile\s*\{\s*"
            r"pub\(crate\) fn file_mut\(&mut self\) -> &mut File\s*\{\s*"
            r"&mut self\.file\s*\}"
        ),
        "private pinned-file state with the sole mutable descriptor accessor",
    )
    require_pattern(
        snapshot_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let source_size = source\s*\.file_mut\(\)\s*\.metadata\(\).*?"
            r"source\s*\.file_mut\(\)\s*\.seek\(SeekFrom::Start\(0\)\).*?"
            r"Read::take\(source\.file_mut\(\), MAX_KAGAMI_BYTES \+ 1\).*?"
            r"source\s*\.file_mut\(\)\s*\.seek\(SeekFrom::Start\(0\)\)"
        ),
        "accessor-confined bounded Kagami snapshot copy",
    )
    sandbox_source = promotion_publisher.split("fn sandbox_profile(", 1)[-1].split(
        "struct Captured", 1
    )[0]
    require(
        sandbox_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        '"(version 1)\\n(deny default)',
        '(allow process-exec (literal {executable}))',
        "(deny network*)",
        "(deny process-fork)",
        "(deny file-link)",
        "(deny file-clone)",
        "(allow file-read* (literal {executable}) (literal {policy}) (literal {bundle_literal}) (subpath {bundle_literal}))",
        "(allow file-write* (require-all (vnode-type REGULAR-FILE) (regex {temporary})))",
        "(allow file-write-create (require-all (vnode-type REGULAR-FILE) (literal {final_leaf})))",
    )
    bounded_identity = promotion_publisher.split(
        "fn identity_from_file_checked(", 1
    )[-1].split("fn sha256_bytes(", 1)[0]
    require_pattern(
        bounded_identity,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let before = file\s*\.metadata\(\).*?"
            r"let before_identity = identity_from_metadata\(&before, \[0; 32\]\);.*?"
            r"if let Some\(\(name, bounds\)\) = validation\s*\{.*?"
            r"validate_bounded_identity\(name, &before_identity, bounds\)\?;\s*\}.*?"
            r"file\.seek\(SeekFrom::Start\(0\)\).*?"
            r"before_identity\.size\.saturating_add\(1\).*?"
            r"let after = file\s*\.metadata\(\).*?"
            r"validate_bounded_identity\(name, &after, bounds\)\?;.*?"
            r"if before != after \|\| !hashed_length_matches"
        ),
        "bounded pre-read and post-read descriptor validation",
    )
    open_member_source = promotion_publisher.split("fn open_member(", 1)[-1].split(
        "struct PinnedInput", 1
    )[0]
    require_pattern(
        open_member_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"statat\(directory, name, AtFlags::SYMLINK_NOFOLLOW\).*?"
            r"FileType::from_raw_mode\(named\.st_mode\) != FileType::RegularFile.*?"
            r"openat\(\s*directory,\s*name,\s*"
            r"OFlags::RDONLY \| OFlags::NOFOLLOW \| OFlags::NONBLOCK \| OFlags::CLOEXEC.*?"
            r"let opened = file\s*\.metadata\(\).*?"
            r"validate_bounded_identity\(name, &identity_from_metadata\(&opened, \[0; 32\]\), bounds\)\?;.*?"
            r"validate_open_file_custody\(&file, Path::new\(name\)\)\?;.*?"
            r"identity_from_file_checked\(&mut file, hash_contents, Some\(\(name, bounds\)\)\)\?"
        ),
        "nonblocking regular-only descriptor open before bounded inspection",
    )
    pinned_input_source = promotion_publisher.split("struct PinnedInput", 1)[-1].split(
        "struct CandidateSnapshot", 1
    )[0]
    require_pattern(
        pinned_input_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"fn open\(path: &Path, maximum: u64\).*?"
            r"let mut parent = open_directory\(parent_path\)\?;.*?"
            r"let parent_identity = identity_from_file\(&mut parent, false\)\?;.*?"
            r"open_member\(&parent, name, bounds, true\)\?;.*?"
            r"fn verify\(&mut self\).*?"
            r"identity_from_file_checked\(&mut self\.file, true, Some\(\(name, self\.bounds\)\)\)\?.*?"
            r"require_root_custody\(&self\.path, false\)\?;.*?"
            r"let mut fresh_parent = open_directory\(parent_path\)\?;.*?"
            r"open_member\(&fresh_parent, name, self\.bounds, true\)\?;.*?"
            r"if fresh != self\.identity"
        ),
        "pinned policy pre/post metadata, digest, parent, and pathname validation",
    )
    candidate_declarations = promotion_publisher.split(
        "const CANDIDATE_FILES: [CandidateFileSpec; 17] = [", 1
    )[-1].split("const REPORT_ARTIFACTS:", 1)[0]
    if candidate_declarations.count("CandidateFileSpec {") != 17:
        errors.append(
            f"{KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT}: candidate inventory declaration is not exact seventeen"
        )
    candidate_entries = (
        *(f'name: "{name}"' for name in ARTIFACTS),
        'name: "topup-finality-roster-v4.norito"',
        'name: "manifest.norito"',
        'name: "manifest.norito.sha256"',
        'name: "manifest.json"',
        'name: "release-attestation-v4.norito"',
        "name: BENCHMARK_NAME",
        "name: REVIEW_NAME",
        'name: "recursive-step-two-qualification-v4.norito"',
        "name: KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1",
    )
    for entry in candidate_entries:
        if candidate_declarations.count(entry) != 1:
            errors.append(
                f"{KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT}: exact candidate inventory entry changed: {entry}"
            )
    inventory_classifier = promotion_publisher.split("fn classify_inventory(", 1)[-1].split(
        "fn commit_uncertain(", 1
    )[0]
    require_pattern(
        inventory_classifier,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"if initial\.len\(\) != CANDIDATE_FILES\.len\(\).*?"
            r"for \(name, expected\) in initial.*?"
            r"if !stable_candidate_identity\(name, expected, observed\).*?"
            r"let additions = current.*?"
            r"match additions\.as_slice\(\)\s*\{\s*"
            r"\[\] => Ok\(PublicationPhase::Candidate\),\s*"
            r"\[\(name, identity\)\] if name\.as_str\(\) == FINAL_NAME => \{\s*"
            r"validate_regular_identity\(name, identity, MAX_PROMOTION_BYTES, false\)\?;\s*"
            r"Ok\(PublicationPhase::Committed\)\s*\}\s*"
            r"\[\(name, identity\)\] if valid_temp_name\(name\) => \{\s*"
            r"validate_regular_identity\(name, identity, MAX_PROMOTION_BYTES, true\)\?;\s*"
            r"Ok\(PublicationPhase::Staging\)"
        ),
        "exact candidate, one-temporary, or one-final inventory state machine",
    )
    candidate_snapshot = promotion_publisher.split("struct CandidateSnapshot", 1)[-1].split(
        "struct ExecutableSnapshot", 1
    )[0]
    require_pattern(
        candidate_snapshot,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"fn open\(path: &Path\).*?"
            r"let expected = CANDIDATE_FILES.*?\.collect::<BTreeSet<_>>\(\);.*?"
            r"if inventory_names\(&directory\)\? != expected.*?"
            r"for spec in CANDIDATE_FILES.*?"
            r"require_root_custody\(&path\.join\(spec\.name\), false\)\?;.*?"
            r"open_member\(&directory, spec\.name, spec\.bounds\(\), true\)\?;.*?"
            r"if !inodes\.insert\(\(identity\.device, identity\.inode\)\).*?"
            r"fn phase\(&mut self\).*?"
            r"stable_directory_identity\(.*?classify_inventory\("
            r"&self\.initial_identities\(\), &self\.current_identities\(\)\?\).*?"
            r"fn verify_committed\(&mut self\).*?"
            r"if self\.phase\(\)\? != PublicationPhase::Committed.*?"
            r"exact eighteen-file post-state.*?"
            r"identity_from_file_checked\(held, true, Some\(\(name, bounds\)\)\)\?.*?"
            r"require_root_custody\(&self\.path\.join\(name\), false\)\?;.*?"
            r"open_member\(&self\.directory, name, bounds, true\)\?;.*?"
            r"require_root_custody\(&self\.path\.join\(FINAL_NAME\), false\)\?;.*?"
            r"open_member\(&self\.directory, FINAL_NAME, final_bounds, true\)\?"
        ),
        "held and reopened exact seventeen-to-eighteen candidate validation",
    )
    require_pattern(
        candidate_snapshot,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"fn current_identities_once\(&self\).*?"
            r"let names = inventory_names\(&self\.directory\)\?;.*?"
            r"for name in names\s*\{\s*"
            r"let bounds = inventoried_member_bounds\(&name\)\?;\s*"
            r"let hash_contents\s*=\s*"
            r"name == KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1;\s*"
            r"let \(_, identity\) = open_member\(&self\.directory, &name, bounds, hash_contents\)\?;\s*"
            r"current\.insert\(name, identity\);"
        ),
        "regular-only bounded temporary and final inventory inspection",
    )
    canonical_report_source = promotion_publisher.split(
        "fn canonical_report(stdout: &[u8], expected: &CanonicalReportV4)", 1
    )[-1].split("fn canonical_existing(", 1)[0]
    require_pattern(
        canonical_report_source,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"stdout\.last\(\) != Some\(&b'\\n'\).*?"
            r"stdout\[\.\.stdout\.len\(\) - 1\].*?"
            r"any\(\|byte\| matches!\(byte, b'\\n' \| b'\\r' \| 0\)\).*?"
            r"let payload = &stdout\[\.\.stdout\.len\(\) - 1\];.*?"
            r"let canonical = norito::json::to_json\(expected\).*?"
            r"if payload != canonical\.as_bytes\(\)"
        ),
        "single exact canonical typed promotion-report JSON line",
    )
    report_projection = promotion_publisher.split("fn report_expectation(", 1)[-1].split(
        "fn current_identities_once(", 1
    )[0]
    require_pattern(
        report_projection,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let envelope_sha256 = parse_sha256\(bundle_leaf,.*?"
            r"self\.read_held\(\"manifest\.norito\"\)\?;.*?"
            r"decode_canonical_norito\(&manifest_bytes, \"Kagemusha V4 manifest\"\)\?;.*?"
            r"manifest\s*\.validate\(\).*?"
            r"manifest_identity\.sha256 != envelope_sha256.*?"
            r"manifest\s*\.canonical_sha256\(\).*?!= envelope_sha256.*?"
            r"self\.read_held\(\"manifest\.norito\.sha256\"\)\?;.*?"
            r"manifest_sidecar != format!\(\"\{\}\\n\", hex\(&envelope_sha256\)\)\.as_bytes\(\).*?"
            r"self\.read_held\(\"manifest\.json\"\)\?;.*?"
            r"norito::json::from_slice\(&manifest_json\).*?"
            r"norito::json::to_string_pretty\(&manifest\).*?"
            r"manifest_from_json != manifest \|\| manifest_json != canonical_manifest_json\.as_bytes\(\)"
        ),
        "canonical manifest envelope, sidecar, and typed JSON cross-binding",
    )
    require_pattern(
        report_projection,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let candidate = manifest\.immutable_candidate\(\).*?"
            r"let candidate_sha256 = candidate\s*\.sha256\(\).*?"
            r"decode_canonical_against_candidate\(\s*&qualification_bytes,\s*&candidate,\s*\).*?"
            r"canonical_sha256_against_candidate\(&candidate\).*?"
            r"qualified_candidate_sha256\(&candidate\).*?"
            r"require_candidate_binding\(\s*&identities,\s*"
            r'"recursive-step-two-qualification-v4\.norito",\s*None,\s*qualification_sha256,\s*\)\?;.*?'
            r"qualification_sha256 != manifest\.qualification_receipt_sha256.*?"
            r"qualified_candidate_sha256 != manifest\.qualified_candidate_sha256.*?"
            r"require_candidate_binding\(\s*&identities,\s*BENCHMARK_NAME,\s*None,\s*"
            r"manifest\.benchmark_evidence_sha256,\s*\)\?;.*?"
            r"require_candidate_binding\(\s*&identities,\s*REVIEW_NAME,\s*None,\s*"
            r"manifest\.cryptographic_review_sha256,\s*\)\?;.*?"
            r"decode_canonical_norito\(&attestation_bytes, \"Kagemusha V4 release attestation\"\)\?;.*?"
            r"let subject = manifest\.release_attestation_subject\(\).*?"
            r"if attestation\.subject != subject.*?"
            r"manifest\.release_attestation_sha256"
        ),
        "independently decoded candidate, qualification, evidence, and attestation bindings",
    )
    require_pattern(
        report_projection,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let descriptors = manifest\s*\.profiles\s*\.iter\(\).*?"
            r"if descriptors\.len\(\) \+ 1 != REPORT_ARTIFACTS\.len\(\).*?"
            r"descriptors\.iter\(\)\.zip\(REPORT_ARTIFACTS\[\.\.8\]\.iter\(\)\).*?"
            r"if descriptor\.file_name != name.*?"
            r"require_candidate_binding\(\s*&identities,\s*name,\s*"
            r"Some\(descriptor\.size_bytes\),\s*descriptor\.sha256,\s*\)\?;.*?"
            r"payload_size_bytes: Some\(descriptor\.payload_size_bytes\),\s*"
            r"payload_sha256: Some\(hex\(&descriptor\.payload_sha256\)\),.*?"
            r"let roster = &manifest\.topup_finality_roster_artifact;.*?"
            r"require_candidate_binding\(\s*&identities,\s*roster_name,\s*"
            r"Some\(roster\.size_bytes\),\s*roster\.sha256,\s*\)\?;"
        ),
        "exact ordered manifest artifact and payload projection",
    )
    require_pattern(
        report_projection,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"Ok\(CanonicalReportV4 \{\s*"
            r'status: "verified"\.to_owned\(\),\s*'
            r"envelope_sha256: hex\(&envelope_sha256\),\s*"
            r"manifest_body_sha256: hex\(&subject\.manifest_subject_sha256\),\s*"
            r"candidate_sha256: hex\(&candidate_sha256\),\s*"
            r"qualification_receipt_sha256: hex\(&qualification_sha256\),\s*"
            r"qualified_candidate_sha256: hex\(&qualified_candidate_sha256\),\s*"
            r"internal_validation_receipt_sha256: hex\(&internal_validation_sha256\),\s*"
            r"promotion_record_sha256: hex\(&promotion_sha256\),\s*"
            r"release_policy_sha256: hex\(&policy_sha256\),\s*"
            r"authenticated_source_seal_projection_sha256: hex\(\s*"
            r"&manifest\.authenticated_source_seal_projection_sha256\s*\),\s*"
            r"reviewed_cargo_binary_sha256: hex\(&manifest\.reviewed_cargo_binary_sha256\),\s*"
            r"reviewed_rustc_binary_sha256: hex\(&manifest\.reviewed_rustc_binary_sha256\),\s*"
            r"generator_binary_sha256: hex\(&manifest\.generator_binary_sha256\),\s*"
            r"sealed_candidate_build_report_sha256: hex\(\s*"
            r"&manifest\.sealed_candidate_build_report_sha256\s*\),\s*"
            r"generation: manifest\.generation\.clone\(\),\s*"
            r"generation_memory_limit_bytes: manifest\.generation_memory_limit_bytes,\s*"
            r"generation_memory_enforcement_profile: manifest\s*"
            r"\.generation_memory_enforcement_profile\s*\.clone\(\),\s*"
            r"network_id: manifest\.network_id\.to_string\(\),\s*"
            r"asset_definition_id: manifest\.asset\.to_string\(\),\s*"
            r"asset_scale: manifest\.asset_scale,\s*"
            r"bridge_abi_version: manifest\.bridge_abi_version,\s*"
            r"recursive_step_verifier_commitment: hex\(&recursive_step_verifier_commitment_v4\(\s*"
            r"&manifest,\s*\)\?\),\s*artifacts,\s*\}\)"
        ),
        "fully candidate-bound canonical report field projection",
    )
    launcher_pin = python_launcher.split("pub(crate) fn pin_regular(", 1)[-1].split(
        "pub(crate) fn validate_pinned(", 1
    )[0]
    require_pattern(
        launcher_pin,
        KAGEMUSHA_PYTHON_LAUNCHER_COMPONENT,
        errors,
        (
            r"require_root_custody\(path, false\)\?;.*?"
            r"let mut file = open_nofollow\(path, true\)\?;.*?"
            r"metadata\.uid\(\) != 0.*?metadata\.nlink\(\) != 1.*?"
            r"metadata\.permissions\(\)\.mode\(\) & 0o022 != 0.*?"
            r"metadata\.size\(\) == 0.*?metadata\.size\(\) > MAX_RUNTIME_FILE_BYTES.*?"
            r"require_no_xattrs\(&file, path\)\?;.*?"
            r"let sha256 = hash_open_file\(&mut file\)\?;.*?"
            r"if sha256 != expected"
        ),
        "root-custodied descriptor-pinned executable digest",
    )
    publisher_run = promotion_publisher.split(
        "fn promote_macos(request: PromotionRequest)", 1
    )[-1].split("#[cfg(test)]", 1)[0]
    require_pattern(
        publisher_run,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"kagemusha_python_launcher::validate_root_launch_identity\(\)\?;\s*"
            r"kagemusha_python_launcher::require_macos_tcb\(&request\.expected_macos_build\)\?;\s*"
            r"kagemusha_python_launcher::require_root_custody\(Path::new\(SANDBOX_EXEC\), false\)\?;.*?"
            r"let controller = std::env::current_exe\(\)\s*"
            r"\.and_then\(fs::canonicalize\).*?"
            r"require_root_custody\(&controller, false\)\?;.*?"
            r"let kagami = canonical_existing\(&request\.kagami, false, \"Kagami executable\"\)\?;.*?"
            r"require_root_custody\(&kagami, false\)\?;.*?"
            r"let mut kagami_pin = kagemusha_python_launcher::pin_regular\("
            r"&kagami, request\.kagami_sha256\)\?;.*?"
            r"let mut candidate = CandidateSnapshot::open\(&bundle\)\?;.*?"
            r"let mut policy_pin = PinnedInput::open\(&policy, MAX_POLICY_BYTES\)\?;.*?"
            r"let mut snapshot = ExecutableSnapshot::create\(&mut kagami_pin, request\.kagami_sha256\)\?;.*?"
            r"let profile = sandbox_profile\(&snapshot\.path, &bundle, &policy\)\?;\s*"
            r"super::validate_sandbox_profile\(&profile\)\?;.*?"
            r"snapshot\.verify\(\)\?;\s*"
            r"kagemusha_python_launcher::validate_pinned\(&mut kagami_pin\)\?;"
        ),
        "root-TCB pinned controller, Kagami, policy, candidate, and snapshot launch",
    )
    require_pattern(
        publisher_run,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let mut command = Command::new\(SANDBOX_EXEC\);\s*"
            r"command\s*\.arg\(\"-p\"\)\s*\.arg\(profile\)\s*"
            r"\.arg\(&snapshot\.path\)\s*\.arg\(\"kagemusha\"\)\s*"
            r"\.arg\(\"promote-release-v4\"\)\s*"
            r"\.arg\(\"--bundle-dir\"\)\s*\.arg\(&bundle\)\s*"
            r"\.arg\(\"--release-policy\"\)\s*\.arg\(&policy\)\s*"
            r"\.arg\(\"--promotion-record\"\)\s*\.arg\(&final_path\)\s*"
            r"\.arg\(\"--benchmark-evidence\"\)\s*\.arg\(&benchmark\)\s*"
            r"\.arg\(\"--cryptographic-review\"\)\s*\.arg\(&review\)\s*"
            r"\.current_dir\(\"/\"\)\s*\.env_clear\(\).*?"
            r"let execution = run_sandboxed\(command, &mut candidate\);"
        ),
        "exact sandboxed Kagami promote-release-v4 argument order",
    )
    require_pattern(
        publisher_run,
        KAGEMUSHA_PROMOTION_PUBLISHER_COMPONENT,
        errors,
        (
            r"let phase = candidate\.phase\(\);.*?"
            r"let committed_or_ambiguous = !matches!\(&phase, Ok\(PublicationPhase::Candidate\)\);.*?"
            r"if committed_or_ambiguous \|\| cleanup\.is_err\(\)\s*\{\s*"
            r"return Err\(commit_uncertain\(error\.message\)\);.*?"
            r"if exit != 0\s*\{\s*return failed_child_result\(exit, phase\.ok\(\), cleanup\.is_ok\(\)\);.*?"
            r"Ok\(PublicationPhase::Committed\) => \{\}.*?"
            r"Ok\(PublicationPhase::Staging\) \| Err\(_\) => \{\s*"
            r"return Err\(commit_uncertain\(.*?"
            r"let final_identity = candidate\.verify_committed\(\)\?;\s*"
            r"policy_pin\.verify\(\)\?;\s*"
            r"kagemusha_python_launcher::validate_pinned\(&mut kagami_pin\)\?;\s*"
            r"let expected_report = candidate\.report_expectation\(\s*"
            r"bundle_leaf,\s*policy_pin\.identity\.sha256,\s*final_identity\.sha256,\s*\)\?;\s*"
            r"canonical_report\(&captured\.stdout, &expected_report\)\?;.*?"
            r"if let Err\(error\) = verification\s*\{\s*"
            r"return Err\(commit_uncertain\(error\.message\)\);\s*\}.*?"
            r"io::stdout\(\)\s*\.write_all\(&captured\.stdout\)\s*"
            r"\.and_then\(\|_\| io::stdout\(\)\.flush\(\)\)"
        ),
        "commit-uncertain classification and single verified stdout forwarding",
    )
    offline_cli = texts[OFFLINE_CLI]
    rollout = texts[KAGEMUSHA_ROLLOUT_COMPONENT]
    require_pattern(
        offline_cli,
        OFFLINE_CLI,
        errors,
        (
            r"mod kagemusha_rollout\s*\{.*?"
            r"pub\(crate\) enum KagemushaCommand\s*\{.*?"
            r'#\[command\(name = "rollout-v4"\)\]\s*'
            r"RolloutV4\(kagemusha_rollout::Args\),.*?"
            r"impl KagemushaCommand\s*\{.*?"
            r"matches!\(\s*self,\s*Self::RolloutV4\(args\) if "
            r"args\.allows_fallback_config\(\)\s*\).*?"
            r"impl Run for KagemushaCommand.*?"
            r"Self::RolloutV4\(args\) => args\.run\(context\),"
        ),
        "rollout-v4 offline CLI wiring and credential separation",
    )
    require(
        rollout,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        'const EXPECTATIONS_FILE_NAME: &str = "activation-expectations-v1.norito";',
        'const SUBMISSION_JOURNAL_FILE_NAME: &str = "activation-submission-journal-v1.norito";',
        'const RECEIPT_FILE_NAME: &str = "activation-finality-receipt-v1.norito";',
        'const ROLLOUT_STATE_ROOT: &str = "/var/lib/iroha/kagemusha-rollout-v1";',
        'const ROLLOUT_STATE_ROOT: &str = "/private/var/db/iroha-kagemusha-rollout-v1";',
        "const SEAL_MAX_BYTES: usize = 1024 * 1024;",
        "const FINALITY_PROOF_MAX_BYTES: usize = 8 * 1024 * 1024;",
        "const TRANSACTION_MAX_BYTES: usize = 64 * 1024 * 1024;",
    )
    rollout_paths = rollout.split("fn rollout_state_path(", 1)[-1].split(
        "fn parse_public_key(", 1
    )[0]
    require_pattern(
        rollout_paths,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"promotion_id: \[u8; 32\], file_name: &str.*?"
            r"Path::new\(ROLLOUT_STATE_ROOT\)\s*"
            r"\.join\(hex::encode\(promotion_id\)\)\s*\.join\(file_name\).*?"
            r"fn require_rollout_state_path\(.*?"
            r"if path != rollout_state_path\(promotion_id, file_name\)\?\s*\{\s*"
            r"bail!\(\"rollout artifact path must be the exact promotion-keyed "
            r"`\{file_name\}` state path\"\);"
        ),
        "canonical promotion-keyed fixed rollout state paths",
    )
    private_key_source = rollout.split("fn inspect_root_private_key(", 1)[-1].split(
        "fn read_root_owned(", 1
    )[0]
    require_pattern(
        private_key_source,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"require_root\(\)\?;.*?"
            r"!path\.is_absolute\(\) \|\| fs::canonicalize\(path\)\? != path.*?"
            r"let ancestry = validate_owned_ancestry\(parent, 0, label\)\?;.*?"
            r"let named = fs::symlink_metadata\(path\)\?;.*?"
            r"metadata\.file_type\(\)\.is_file\(\).*?metadata\.uid\(\) == 0.*?"
            r"metadata\.nlink\(\) == 1.*?metadata\.mode\(\) & 0o7777 == 0o600.*?"
            r"metadata\.len\(\) > 0.*?metadata\.len\(\) <= 4 \* 1024.*?"
            r"OFlags::RDONLY \| rustix::fs::OFlags::NOFOLLOW \| "
            r"rustix::fs::OFlags::CLOEXEC.*?"
            r"metadata_identity\(&named\) != metadata_identity\(&metadata\).*?"
            r"ancestry != validate_owned_ancestry\(parent, 0, label\)\?.*?"
            r"require_no_xattrs\(&opened, label\)\?;\s*"
            r"require_no_macos_acl\(&opened, label\)\?;.*?"
            r"fn load_root_custodied_key\(.*?"
            r"let before = inspect_root_private_key\(path, label\)\?;.*?"
            r"let key = load_operator_key_pair\(path\).*?"
            r"let after = inspect_root_private_key\(path, label\)\?;.*?"
            r"if metadata_identity\(&before\) != metadata_identity\(&after\)"
        ),
        "stable root-owned mode-0600 private-key custody",
    )
    stable_read = rollout.split("fn read_owned_with_policy(", 1)[-1].split(
        "fn metadata_identity(", 1
    )[0]
    require_pattern(
        stable_read,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"if !path\.is_absolute\(\).*?"
            r"let ancestry = validate_owned_ancestry\(parent, uid, label\)\?;.*?"
            r"if fs::canonicalize\(path\)\? != path.*?"
            r"let before = fs::symlink_metadata\(path\).*?"
            r"metadata\.file_type\(\)\.is_file\(\).*?metadata\.nlink\(\) == 1.*?"
            r"metadata\.uid\(\) == uid.*?metadata\.mode\(\) & 0o7777 == expected_mode.*?"
            r"metadata\.len\(\) <= u64::try_from\(maximum\).*?"
            r"OFlags::RDONLY \| rustix::fs::OFlags::CLOEXEC \| "
            r"rustix::fs::OFlags::NOFOLLOW.*?"
            r"metadata_identity\(&before\) != metadata_identity\(&opened\).*?"
            r"require_no_xattrs\(&file, label\)\?;\s*"
            r"require_no_macos_acl\(&file, label\)\?;.*?"
            r"\.take\(u64::try_from\(maximum\)\?\.saturating_add\(1\)\).*?"
            r"metadata_identity\(&opened\) != metadata_identity\(&after\).*?"
            r"metadata_identity\(&after\) != metadata_identity\(&named_after\).*?"
            r"ancestry != validate_owned_ancestry\(parent, uid, label\)\?.*?"
            r"fs::canonicalize\(path\)\? != path.*?"
            r"require_no_xattrs\(&file, label\)\?;\s*"
            r"require_no_macos_acl\(&file, label\)\?;"
        ),
        "bounded no-follow root-owned read with stable metadata and ACL/xattr checks",
    )
    private_read = rollout.split("fn read_root_private_artifact(", 1)[-1].split(
        "fn metadata_identity(", 1
    )[0]
    require_pattern(
        private_read,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"read_owned_with_policy\(path, maximum, label, 0, 0o400, true\).*?"
            r"fn read_owned\(.*?read_owned_with_policy\(path, maximum, label, uid, 0o444, false\).*?"
            r"if require_private_parent.*?ancestry\s*\.last\(\).*?"
            r"identity\.2 & 0o7777 != 0o700 \|\| identity\.3 != uid.*?"
            r"metadata\.mode\(\) & 0o7777 == expected_mode"
        ),
        "root-private mode-0400 artifacts below owner-held mode-0700 parents",
    )
    ancestry_source = rollout.split("fn validate_owned_ancestry(", 1)[-1].split(
        "fn require_no_xattrs(", 1
    )[0]
    require_pattern(
        ancestry_source,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"!path\.is_absolute\(\) \|\| fs::canonicalize\(path\)\? != path.*?"
            r"let metadata = fs::symlink_metadata\(&entry\)\?;.*?"
            r"!metadata\.file_type\(\)\.is_dir\(\).*?"
            r"\(metadata\.uid\(\) != 0 && metadata\.uid\(\) != owner\).*?"
            r"metadata\.mode\(\) & 0o022 != 0.*?"
            r"OFlags::DIRECTORY.*?OFlags::NOFOLLOW.*?"
            r"custody_identity\(&opened\.metadata\(\)\?\) != custody_identity\(&metadata\).*?"
            r"require_no_xattrs\(&opened, label\)\?;\s*"
            r"require_no_macos_acl\(&opened, label\)\?;"
        ),
        "canonical no-follow root-owned ancestry with ACL/xattr checks",
    )
    publication = rollout.split("enum PublicationError", 1)[-1].split(
        "#[cfg(test)]", 1
    )[0]
    require_pattern(
        publication,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"CleanupUncertain \{ path: PathBuf, detail: String \}.*?"
            r"CommitUncertain \{ path: PathBuf, detail: String \}.*?"
            r"fn open_owned_publication_destination\(.*?"
            r"validate_owned_ancestry\(parent_path, uid, \"publication destination\"\).*?"
            r"parent_meta\.mode\(\) & 0o7777 != 0o700.*?"
            r"AtFlags::SYMLINK_NOFOLLOW.*?destination already exists and will not be replaced.*?"
            r"fn publish_owned_with\(.*?"
            r"OFlags::CREATE.*?OFlags::EXCL.*?OFlags::NOFOLLOW.*?"
            r"Mode::from_raw_mode\(0o600\).*?"
            r"staging\s*\.write_all\(bytes\).*?staging\.sync_all\(\).*?"
            r"fchmod\(&staging, rustix::fs::Mode::from_raw_mode\(0o400\)\).*?"
            r"staging\.sync_all\(\).*?require_no_xattrs\(&staging.*?"
            r"require_no_macos_acl\(&staging.*?parent\.sync_all\(\).*?"
            r"RenameFlags::NOREPLACE.*?after_commit\(\).*?"
            r"u32::from\(named\.st_mode\) & 0o7777 != 0o400.*?"
            r"parent\.sync_all\(\).*?if readback != bytes.*?verify\(&readback\)\?;.*?"
            r"metadata_identity\(&opened\) != metadata_identity\(&path_metadata\).*?"
            r"opened\.mode\(\) & 0o7777 != 0o400.*?"
            r"ancestry\s*!= validate_owned_ancestry\(&parent_path, uid, \"published artifact\"\).*?"
            r"require_no_xattrs\(&staging.*?require_no_macos_acl\(&staging.*?"
            r"post\.map_err\(\|detail\| PublicationError::CommitUncertain"
        ),
        "root-owned no-replace fsync publication with commit uncertainty",
    )
    create_expectations = rollout.split("impl CreateExpectations", 1)[-1].split(
        "struct Submit", 1
    )[0]
    require_pattern(
        create_expectations,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"require_root\(\)\?;.*?"
            r"KagemushaV4PromotionReservationV1::decode_and_verify_canonical\(.*?"
            r"require_rollout_state_path\(\s*&self\.output,\s*"
            r"reservation\.body\.promotion_id,\s*EXPECTATIONS_FILE_NAME,\s*\)\?;.*?"
            r"verify_kagemusha_v4_validator_qualification_seals\(.*?"
            r"SignedTransaction::decode_all_versioned\(&transaction_bytes\).*?"
            r"\.encode_wire_v1\(\).*?!= transaction_bytes.*?"
            r"let body = KagemushaV4ActivationReceiptExpectationsBodyV1 \{.*?\};\s*"
            r"let artifact = \{\s*let controller_key = load_root_custodied_key\(.*?"
            r"if controller_key\.public_key\(\) != &controller.*?"
            r"KagemushaV4ActivationReceiptExpectationsArtifactV1::try_sign\(body, &controller_key\).*?"
            r"let bytes = norito::encode_canonical\(&artifact\).*?"
            r"artifact\s*\.verify_exact\(&bytes, &controller, &reservation_bytes\).*?"
            r"publish_root_owned\(&self\.output, &bytes, \|published\| \{\s*"
            r"KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_and_verify_canonical\(\s*"
            r"published,\s*&controller,\s*&reservation_bytes,"
        ),
        "deferred signing and exact reverification before expectations publication",
    )
    require_pattern(
        create_expectations,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"publish_root_owned\(&self\.output, &bytes,.*?"
            r"context\.print_data\(&report\)\.map_err\(\|error\| \{\s*"
            r"eyre!\(PublicationError::CommitUncertain \{\s*"
            r"path: self\.output,\s*"
            r'detail: format!\("published expectations report failed: \{error\}"\),'
        ),
        "commit-uncertain expectations publication reporting",
    )
    journal_source = rollout.split("fn verify_submission_journal_bytes(", 1)[-1].split(
        "fn require_status_response_hash(", 1
    )[0]
    require_pattern(
        journal_source,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"if bytes != loaded\.exact_bytes.*?"
            r"KagemushaV4ActivationReceiptExpectationsArtifactV1::decode_canonical\(bytes\).*?"
            r"\.verify_exact\(bytes, &loaded\.controller, &loaded\.reservation_bytes\).*?"
            r"fn inspect_submission_journal\(.*?"
            r"ErrorKind::NotFound.*?SubmissionJournalObservation::Absent.*?"
            r"read_root_private_artifact\(\s*path,\s*"
            r"KAGEMUSHA_V4_ACTIVATION_EXPECTATIONS_MAX_BYTES,\s*"
            r'"submission journal",\s*\)\?.*?'
            r"if bytes != loaded\.exact_bytes.*?SubmissionJournalObservation::Mismatched.*?"
            r"verify_submission_journal_bytes\(&bytes, loaded\)\?;.*?"
            r"SubmissionJournalObservation::Matching.*?"
            r"fn publish_submission_journal\(.*?"
            r"publish_root_owned\(path, &loaded\.exact_bytes, \|published\| \{\s*"
            r"verify_submission_journal_bytes\(published, loaded\)"
        ),
        "exact signed expectations journal read, publication, and reverification",
    )
    journal_decision = rollout.split("fn decide_submission_journal(", 1)[-1].split(
        "struct SubmissionUncertain", 1
    )[0]
    require_pattern(
        journal_decision,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"\(SubmissionJournalObservation::Mismatched, _\) =>.*?"
            r"SubmissionJournalDecisionError::Mismatch.*?"
            r"\(SubmissionJournalObservation::Absent, true\) =>.*?"
            r"SubmissionJournalDecisionError::Retrospective.*?"
            r"\(SubmissionJournalObservation::Absent, false\) => "
            r"Ok\(SubmissionJournalAction::Publish\).*?"
            r"\(SubmissionJournalObservation::Matching, _\) => "
            r"Ok\(SubmissionJournalAction::Resume\)"
        ),
        "retrospective refusal and matching-journal safe-resume decision",
    )
    submit_run = rollout.split("impl Submit", 1)[-1].split(
        "struct FinalizeReceipt", 1
    )[0]
    require_pattern(
        submit_run,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"if !self\.write_authorized\s*\{\s*"
            r'bail!\("--write-authorized is required for governed activation submission"\);\s*'
            r"\}\s*require_root\(\)\?;\s*"
            r"let loaded = load_verified_expectations\(&self\.trusted\)\?;"
        ),
        "explicit write authorization before rollout submission state access",
    )
    submit_declaration = rollout.split("struct Submit {", 1)[-1].split(
        "enum SubmissionJournalObservation", 1
    )[0]
    require_pattern(
        submit_declaration,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"#\[arg\(long, required = true, action = clap::ArgAction::SetTrue\)\]\s*"
            r"write_authorized: bool,"
        ),
        "required activation --write-authorized CLI flag",
    )
    require_pattern(
        submit_run,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"let journal_path = rollout_state_path\(\s*"
            r"loaded\.verified\.binding\(\)\.promotion_id,\s*"
            r"SUBMISSION_JOURNAL_FILE_NAME,\s*\)\?;\s*"
            r"let journal_observation = inspect_submission_journal\(&journal_path, &loaded\)\?;.*?"
            r"get_transaction_status_response_auto\(hash\).*?"
            r"decide_submission_journal\(journal_observation, initial_status\.is_some\(\)\).*?"
            r"if journal_action == SubmissionJournalAction::Publish\s*\{\s*"
            r"publish_submission_journal\(&journal_path, &loaded\)\?;\s*"
            r"if inspect_submission_journal\(&journal_path, &loaded\)\?\s*"
            r"!= SubmissionJournalObservation::Matching.*?"
            r"get_transaction_status_response_auto\(hash\).*?"
            r"stage: \"post-journal pre-POST reconciliation\".*?"
            r"\}\s*.*?submit_prepared_transaction_payload\(&prepared\)"
        ),
        "durable exact journal and status reconciliation before every POST",
    )
    status_source = rollout.split("fn classify_reconciled_submission_status(", 1)[-1].split(
        "impl Submit", 1
    )[0]
    require_pattern(
        status_source,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r'Some\("Applied"\) => ReconciledSubmissionStatus::Applied,\s*'
            r'Some\("Rejected"\) => ReconciledSubmissionStatus::Rejected,\s*'
            r'Some\("Expired"\) => ReconciledSubmissionStatus::Expired,\s*'
            r"Some\(_\) \| None => ReconciledSubmissionStatus::Unresolved,"
        ),
        "closed terminal status classification",
    )
    status_identity = rollout.split("fn require_status_response_hash(", 1)[-1].split(
        "fn applied_carrier_height(", 1
    )[0]
    require_pattern(status_identity, KAGEMUSHA_ROLLOUT_COMPONENT, errors,
        (r"fn require_journal_bound_status_response\(.*?"
         r"require_status_response_hash\(status, transaction\)\.map_err\(\|error\| \{\s*"
         r"eyre!\(SubmissionUncertain \{.*?journal: journal_path\.to_path_buf\(\).*?"
         r"fn require_journal_bound_status_hash\(.*?if observed_hash != expected.*?"
         r"SubmissionUncertain \{.*?journal: journal_path\.to_path_buf\(\).*?"
         r"fn require_journal_bound_wait_outcome\(.*?"
         r"require_journal_bound_status_hash\(.*?require_journal_bound_status_response\(.*?"
         r"outcome\.terminal_kind != outcome\.r#final\.status\.kind.*?SubmissionUncertain"),
        "journal-bound status identity uncertainty")
    if submit_run.count("require_journal_bound_status_response(") != 3:
        errors.append(f"{KAGEMUSHA_ROLLOUT_COMPONENT}: every submit status identity path must be journal-bound")
    configured_wait = rollout.split("fn wait_for_activation_terminal_status(", 1)[-1].split(
        "fn finish_waited_submission", 1
    )[0]
    require_pattern(
        configured_wait,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"client\.wait_for_transaction_terminal_status\(\s*hash,\s*"
            r"TransactionWaitOptions \{\s*timeout: client\.transaction_status_timeout,\s*"
            r"terminal_statuses: vec!\[\s*"
            r"TransactionWaitTerminalStatus::Applied,\s*"
            r"TransactionWaitTerminalStatus::Rejected,\s*"
            r"TransactionWaitTerminalStatus::Expired,"
        ),
        "configured status timeout and exact terminal status set",
    )
    if rollout.count('stage: "proof-anchored Applied result reporting"') != 4:
        errors.append(
            f"{KAGEMUSHA_ROLLOUT_COMPONENT}: every activation and canary Applied reporting path must be submission-uncertain"
        )
    ambiguous_post = submit_run.split(
        "else if let Err(post_error) = client.submit_prepared_transaction_payload(&prepared)", 1
    )[-1].split("let outcome = match wait_for_activation_terminal_status(&client, hash)", 1)[0]
    require_pattern(
        ambiguous_post,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"get_transaction_status_response_auto\(hash\).*?"
            r"ReconciledSubmissionStatus::Applied => \{\s*"
            r"let evidence = collect_finalized_activation_evidence\(.*?"
            r"stage: \"ambiguous POST proof reconciliation\".*?"
            r"context\.print_data\(&report\)\.map_err\(\|error\| \{\s*"
            r"eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"proof-anchored Applied result reporting\""
        ),
        "proof reconciliation and explicit uncertainty after ambiguous POST",
    )
    finality_context = rollout.split("fn require_qualified_finality_context(", 1)[-1].split(
        "fn collect_finalized_activation_evidence(", 1
    )[0]
    require_pattern(
        finality_context,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"context\.network_id != expectations\.binding\(\)\.network_id.*?"
            r"context\.mode != ConsensusMode::Permissioned.*?"
            r"context\.nexus_amx_context_hash\s*"
            r"!= Hash::prehashed\(runtime\.genesis_context\.nexus_amx_context_hash\).*?"
            r"context\.execution_policy_hash != expectations\.binding\(\)\.execution_policy_hash.*?"
            r"context\.da_layout != runtime\.genesis_context\.da_layout.*?"
            r"context\.snapshot_bootstrap\.is_some\(\).*?"
            r"context\.roster\.len\(\) != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT.*?"
            r"validator_set_pops\.len\(\) != runtime\.validators\.len\(\).*?"
            r"\.zip\(expectations\.validator_bodies\(\)\).*?"
            r"member\.power != 1 \|\| member\.validator != body\.validator_id.*?"
            r"validator_set_pops\s*\.iter\(\)\s*\.zip\(&runtime\.validators\).*?"
            r"actual != &expected\.bls_pop"
        ),
        "exact four-validator DA Nexus PoP and execution-policy corridor",
    )
    finality_evidence = rollout.split("fn collect_finalized_activation_evidence(", 1)[-1].split(
        "fn collect_finalized_canary_evidence(", 1
    )[0]
    require_pattern(
        finality_evidence,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"get_successful_transaction_details\(transaction\.hash_as_entrypoint\(\)\).*?"
            r"require_committed_entrypoint_wire\(&committed\.entrypoint, exact_wire\).*?"
            r"checked_sub\(anchor_height\).*?proof_count == 0.*?"
            r"KAGEMUSHA_V4_ACTIVATION_FINALITY_PROOF_MAX_COUNT_V1.*?"
            r"BridgeFinalityVerifier::with_context\(.*?"
            r"require_qualified_finality_context\(anchor, expectations\)\?;.*?"
            r"verifier\s*\.verify\(anchor\).*?"
            r"for height in first_successor\.\.=carrier_height\.get\(\).*?"
            r"if height == carrier_height\s*\{\s*"
            r"client\.get_bridge_finality_proof\(\s*height,\s*"
            r"committed\.block_hash\(\)\.clone\(\),\s*&mut verifier,\s*\).*?"
            r"client\.get_next_bridge_finality_proof\(height, &mut verifier\).*?"
            r"require_qualified_finality_context\(&proof, expectations\)\?;.*?"
            r"proof_bytes > KAGEMUSHA_V4_ACTIVATION_RECEIPT_MAX_BYTES.*?"
            r"get_canonical_executed_block_wire\(carrier_height, &committed\).*?"
            r"decode_framed_signed_block\(&block_bytes\).*?"
            r"block\s*\.encode_wire\(\).*?!= block_bytes.*?"
            r"entrypoint_hashes\(\).*?\.count\(\)\s*!= 1.*?"
            r"TrustedBlockProofAnchor::from_untrusted_finality_artifact\(\s*"
            r"&block,\s*finality_artifact,\s*committed\.entrypoint_hash\(\),\s*\).*?"
            r"let entry_index = usize::try_from\(proof_anchor\.entry_index\(\)\)\?;.*?"
            r"entrypoints_cloned\(\)\s*\.nth\(entry_index\).*?"
            r"require_committed_entrypoint_wire\(&block_entrypoint, exact_wire\)"
        ),
        "bounded full finality chain and trusted canonical block entrypoint proof",
    )
    exact_entrypoint = rollout.split("fn require_committed_entrypoint_wire(", 1)[-1].split(
        "fn require_root()", 1
    )[0]
    require_pattern(
        exact_entrypoint,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"let TransactionEntrypoint::External\(committed\) = entrypoint.*?"
            r"let committed_wire = committed\s*\.encode_wire_v1\(\).*?"
            r"if committed_wire != exact_wire"
        ),
        "exact authorization-bearing external transaction wire comparison",
    )
    finish_wait = rollout.split("fn finish_waited_submission", 1)[-1].split(
        "fn reconcile_after_failed_wait", 1
    )[0]
    require_pattern(
        finish_wait,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"require_journal_bound_wait_outcome\(.*?"
            r"ReconciledSubmissionStatus::Applied => \{.*?"
            r"collect_finalized_activation_evidence\(.*?"
            r"stage: \"Applied proof collection\".*?"
            r"context\.print_data\(&outcome\)\.map_err\(\|error\| \{\s*"
            r"eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"proof-anchored Applied result reporting\".*?"
            r"ReconciledSubmissionStatus::Rejected \| ReconciledSubmissionStatus::Expired.*?"
            r"ReconciledSubmissionStatus::Unresolved => Err\(eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"configured terminal wait\""
        ),
        "explicit submission uncertainty after configured wait ambiguity",
    )
    failed_wait = rollout.split("fn reconcile_after_failed_wait", 1)[-1].split(
        "struct FinalizedActivationEvidence", 1
    )[0]
    require_pattern(
        failed_wait,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"require_journal_bound_status_response\(.*?"
            r"ReconciledSubmissionStatus::Applied => \{.*?"
            r"collect_finalized_activation_evidence\(.*?"
            r"stage: \"failed wait proof reconciliation\".*?"
            r"context\.print_data\(&report\)\.map_err\(\|error\| \{\s*"
            r"eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"proof-anchored Applied result reporting\".*?"
            r"ReconciledSubmissionStatus::Unresolved => Err\(eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"failed wait status reconciliation\".*?"
            r"Ok\(None\) => Err\(eyre!\(SubmissionUncertain \{.*?"
            r"Err\(status_error\) => Err\(eyre!\(SubmissionUncertain \{.*?"
            r"stage: \"failed wait transport reconciliation\""
        ),
        "proof or explicit submission uncertainty after failed wait reconciliation",
    )
    finalize_receipt = rollout.split("impl FinalizeReceipt", 1)[-1].split(
        "struct LoadedVerifiedExpectations", 1
    )[0]
    require_pattern(
        finalize_receipt,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"require_rollout_state_path\(\s*&self\.output,\s*"
            r"expectations\.binding\(\)\.promotion_id,\s*RECEIPT_FILE_NAME,\s*\)\?;.*?"
            r"let journal_path = rollout_state_path\(\s*"
            r"expectations\.binding\(\)\.promotion_id,\s*SUBMISSION_JOURNAL_FILE_NAME,\s*\)\?;\s*"
            r"require_matching_submission_journal\(inspect_submission_journal\(\s*"
            r"&journal_path,\s*&loaded\s*\)\?\)\?;.*?"
            r"require_journal_bound_status_response\(\s*&status,\s*transaction,\s*"
            r"&journal_path,\s*\"finalize status identity reconciliation\",\s*\)\?;\s*"
            r"match classify_reconciled_submission_status\(Some\(&status\.status\.kind\)\).*?"
            r"ReconciledSubmissionStatus::Applied => \{\}.*?"
            r"ReconciledSubmissionStatus::Rejected \| ReconciledSubmissionStatus::Expired.*?"
            r"ReconciledSubmissionStatus::Unresolved => \{\s*"
            r"return Err\(eyre!\(SubmissionUncertain \{.*?"
            r"let carrier_height = applied_carrier_height_for_submission\(.*?"
            r"let evidence = collect_finalized_activation_evidence\(.*?"
            r"let body = KagemushaV4ActivationFinalityReceiptBodyV1 \{.*?\};\s*"
            r"let issuer_key =\s*load_root_custodied_key\(.*?"
            r"if issuer_key\.public_key\(\) != expectations\.receipt_issuer\(\).*?"
            r"KagemushaV4ActivationFinalityReceiptV1::try_sign\(body, &issuer_key\).*?"
            r"receipt\s*\.verify\(expectations\).*?"
            r"publish_root_owned\(&self\.output, &bytes, \|published\| \{\s*"
            r"let receipt = KagemushaV4ActivationFinalityReceiptV1::decode_canonical\(published\).*?"
            r"receipt\s*\.verify\(expectations\)"
        ),
        "deferred issuer signing and proof-verified no-replace final receipt",
    )
    require_pattern(
        finalize_receipt,
        KAGEMUSHA_ROLLOUT_COMPONENT,
        errors,
        (
            r"publish_root_owned\(&self\.output, &bytes,.*?"
            r"context\.print_data\(&report\)\.map_err\(\|error\| \{\s*"
            r"eyre!\(PublicationError::CommitUncertain \{\s*"
            r"path: self\.output,\s*"
            r'detail: format!\("published receipt report failed: \{error\}"\),'
        ),
        "commit-uncertain final-receipt publication reporting",
    )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"Command::PromoteReleaseV4\(args\) => \{.*?"
            r"verify_publish_verify_release_v4\(.*?"
            r"publish_new_durable_file\(\s*&mut std::io::sink\(\),\s*"
            r"&args\.promotion_record,\s*&record_bytes,\s*\).*?"
            r"writeln!\(\s*writer,\s*\"\{\}\",\s*"
            r"verified\.verification_report\(\)\?\.canonical_json\(\)\?\s*\)\?;"
        ),
        "verify-publish-verify promotion with one final canonical stdout JSON",
    )
    durable_publication = texts[KAGAMI].split(
        "fn write_new_durable_file_with_hooks_v1", 1
    )[-1].split("fn release_circuit_params_file_snapshot_matches_stat_v1", 1)[0]
    require_pattern(
        durable_publication,
        KAGAMI,
        errors,
        (
            r"statat\(&parent\.file, target_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
            r"refusing to overwrite or alias an existing promotion record.*?"
            r"OFlags::WRONLY \| OFlags::CREATE \| OFlags::EXCL \| OFlags::NOFOLLOW \| OFlags::CLOEXEC.*?"
            r"Mode::from_raw_mode\(0o600\).*?"
            r"temporary\s*\.write_all\(bytes\).*?temporary\s*\.sync_all\(\).*?"
            r"before_publish\(\)\.and_then\(\|\(\)\| parent\.verify_path_identity\(\)\).*?"
            r"renameat_with\(\s*&parent\.file,\s*&temporary_name,\s*"
            r"&parent\.file,\s*target_name,\s*RenameFlags::NOREPLACE,\s*\).*?"
            r"statat\(&parent\.file, target_name, AtFlags::SYMLINK_NOFOLLOW\).*?"
            r"sync_parent\(&parent\.file\).*?parent\.verify_path_identity\(\)\?;.*?"
            r"DurableFilePublicationOutcomeV1::Committed.*?"
            r"DurableFilePublicationOutcomeV1::CommitUncertain"
        ),
        "private no-replace durable promotion-record publication",
    )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"const DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE: u8 = 75;.*?"
            r"fn publish_new_durable_file.*?"
            r"DurableFilePublicationOutcomeV1::CommitUncertain.*?"
            r"ExplicitExitError::new\(\s*"
            r"DURABLE_FILE_COMMIT_UNCERTAIN_EXIT_CODE,\s*uncertain\.operator_record\(\),"
        ),
        "durability-uncertain Kagami exit 75",
    )
    require(
        texts[BUNDLE],
        BUNDLE,
        errors,
        "const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 18;",
        "fn final_release_inventory_v4() -> BTreeSet<String>",
        "KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1",
        "KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4",
        "if expected.len() != FINAL_RELEASE_INVENTORY_COUNT_V4",
        "fn final_release_inventory_is_exact_and_includes_both_receipts()",
    )
    require_pattern(
        texts[BUNDLE],
        BUNDLE,
        errors,
        (
            r"fn final_release_inventory_v4\(\).*?\.chain\(\[.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"\]\).*?\.collect\(\).*?impl PublicationDirectory"
        ),
        "function-scoped 18-file producer inventory including both validation receipts",
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
    require_pattern(
        texts[READINESS],
        READINESS,
        errors,
        (
            r"FINAL_METADATA = \(.*?"
            r'"internal-validation-receipt-v1\.norito",.*?'
            r'"recursive-step-two-qualification-v4\.norito",.*?'
            r'"promotion-record-v4\.norito",.*?'
            r"\).*?MAX_RELEASE_INVENTORY_ENTRIES = len\(ARTIFACTS \+ FINAL_METADATA\).*?"
            r"MAX_INTERNAL_VALIDATION_RECEIPT_BYTES = 1024 \* 1024"
        ),
        "18-file readiness inventory with bounded internal-validation receipt",
    )
    require_pattern(
        texts[READINESS],
        READINESS,
        errors,
        (
            r"BOUNDED_AUTHENTICATED_METADATA = \(.*?"
            r'"internal-validation-receipt-v1\.norito",\s*'
            r"MAX_INTERNAL_VALIDATION_RECEIPT_BYTES,.*?"
            r'\("promotion-record-v4\.norito", MAX_PROMOTION_RECORD_BYTES\),'
        ),
        "bounded opaque internal-validation receipt staging",
    )
    require_pattern(
        texts[READINESS],
        READINESS,
        errors,
        (
            r"def validate_kagami_verification_report\(.*?"
            r"internal_validation_receipt_sha256: str,.*?"
            r'"internal_validation_receipt_sha256",.*?'
            r"report\.get\(\"internal_validation_receipt_sha256\"\)\s*"
            r"!= internal_validation_receipt_sha256.*?"
            r'raise ValueError\(\"Kagami verified a different internal-validation receipt\"\)'
        ),
        "internal-validation report digest binding",
    )
    require_pattern(
        texts[READINESS],
        READINESS,
        errors,
        (
            r"internal_validation_receipt_sha256: str \| None = None.*?"
            r'elif name == "internal-validation-receipt-v1\.norito":\s*'
            r"internal_validation_receipt_sha256 = hashlib\.sha256\(payload\)\.hexdigest\(\).*?"
            r"or internal_validation_receipt_sha256 is None.*?"
            r"internal_validation_receipt_sha256=internal_validation_receipt_sha256,"
        ),
        "internal-validation staged-byte digest forwarding",
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
        "MAX_READINESS_SOURCE_CONTRACT_BYTES = 140 * 1024",
        "authenticated_readiness_source_contract_bytes: dict[str, bytes] = {}",
        "READINESS_SOURCE_PROVIDERS = (",
        "def pin_authenticated_reviewed_source_file(",
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
        "KAGEMUSHA_V4_PROMOTION_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT",
        "online-freshness-consumption-receipt-v1.json",
        "catalog-revalidation-receipt-v1.json",
        "promotion Python runtime closure changed during the production gate",
        "static candidate corridor passed;",
        "production promotion was not evaluated.",
    )
    require(
        texts[IOS_EVIDENCE_MODULE],
        IOS_EVIDENCE_MODULE,
        errors,
        'CANDIDATE_XCODE_VERSION = "Xcode 26.6"',
        "xcode_version must be exact Xcode 26.6 with one canonical build-version line",
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
        "iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1",
        "def catalog_revalidation_digest(",
        "def validate_catalog_revalidation_receipt(",
        "def validate_historical_production_evidence_for_catalog_revalidation(",
        '"previous_assertion_counter"',
        '"consumption_id"',
    )
    require_pattern(
        texts[PRODUCTION_IOS_EVIDENCE_MODULE],
        PRODUCTION_IOS_EVIDENCE_MODULE,
        errors,
        (
            r"def validate_production_signed_evidence\(.*?"
            r"return _validate_production_signed_evidence\(.*?"
            r"require_current_freshness_receipt=True,\s*\).*?"
            r"def validate_historical_production_evidence_for_catalog_revalidation\("
            r".*?return _validate_production_signed_evidence\(.*?"
            r"require_current_freshness_receipt=False,\s*\)"
        ),
        "separate current and historical freshness validator wrappers",
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
    require_pattern(
        descriptor_custody_functions,
        READINESS,
        errors,
        (
            r"def require_production_root_custody\(.*?"
            r"metadata\.st_uid != PRODUCTION_TRUSTED_UID.*?"
            r"stat\.S_IMODE\(metadata\.st_mode\) & 0o022.*?"
            r"require_no_macos_extended_acl\(descriptor, label\)"
        ),
        "root-owned non-group/world-writable production custody",
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
            r"ios_root,\s*key_id,\s*_,\s*_,\s*freshness_key_id,\s*_,\s*_,\s*_\s*"
            r"=\s*ios_configuration"
        ),
        "exact eight-field physical-iOS configuration unpack",
    )
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
    require_pattern(
        ios_loader_function,
        READINESS,
        errors,
        (
            r"historical_validator\s*=\s*production_module\.__dict__\.get\(\s*"
            r'"validate_historical_production_evidence_for_catalog_revalidation"'
            r"\s*\).*?catalog_validator\s*=\s*"
            r"production_module\.__dict__\.get\(\s*"
            r'"validate_catalog_revalidation_receipt"\s*\).*?'
            r"return historical_validator\(.*?"
            r"return validate, validate_catalog"
        ),
        "historical consumption plus separate current catalog validator boundary",
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
            r"if sealed_build_report is not None:\s*"
            r"production_roots\.append\(sealed_build_report\.parent\).*?"
            r"production_directory_paths\s*=\s*\{.*?"
            r"for trusted_root in production_roots.*?"
            r"if path in production_directory_paths:\s*try:\s*"
            r"require_production_root_custody\(descriptor, label\)"
        ),
        "sealed-build-report ancestor root custody",
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
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"physical-iOS catalog revalidation receipt.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"catalog_revalidation_receipt_snapshot,\s*"
            r"trusted_catalog_revalidation_receipt_snapshot.*?"
            r"ios_catalog_bindings\.append\(ios_catalog_binding\).*?"
            r"ios_catalog_validator\(\s*"
            r"trusted_catalog_revalidation_receipt_snapshot,.*?"
            r"ios_configuration\[6\],\s*ios_catalog_bindings,"
        ),
        "current promotion-scoped exact-catalog App Attest revalidation",
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
        texts[CONFIG] + texts[NODE] + texts[CORE] + texts[CATALOG],
        "configured V4 runtime",
        errors,
        "kagemusha_release_policy_path",
        "kagemusha_artifact_dir",
        "KagemushaReleaseCatalogV4::load",
        "ensure_kagemusha_active_release_material_v4",
        "KAGEMUSHA_V4_PROMOTION_ID_DOMAIN",
        "fn plan_v4_promotion_id(",
    )
    require(
        texts[CORE],
        CORE,
        errors,
        "impl Execute for ActivateKagemushaRecursiveReleaseV4",
        "CanActivateKagemushaRecursiveReleaseV4",
        "plan_kagemusha_v4_activation_binding(",
        "commit_v4_promotion_id",
        "CanManageOfflineDeviceAttestationPolicy",
        "validate_offline_attestation_policy_for_release_activation",
        "self.device_attestation_policy",
        "impl Execute for TopUpKagemushaRecursiveV4",
        "impl Execute for RedeemKagemushaRecursiveV4",
        "release.issuance_active",
    )
    require_pattern(
        texts[CORE],
        CORE,
        errors,
        (
            r"let\s+change_release\s*=.*?\.transpose\(\)\?;.*?"
            r"is_some_and\(\|release\|\s*!release\.issuance_active\)"
        ),
        "offline-change issuance window",
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
        "ci/check_kagemusha_production_readiness_source_support.py",
        "ci/check_kagemusha_recursion_source_contract.py",
        "ci/check_kagemusha_lifecycle_source_contract.py",
        "ci/check_kagemusha_production_readiness_self_test.py",
        "ci/check_kagemusha_recursive_spend_python_sdk.sh --self-test",
        "check_kagemusha_recursive_spend_v4_sdk_contract.sh",
        '"crates/iroha_core/src/smartcontracts/isi/offline/**"',
        '"crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle/**"',
        '"specs/sdk/swift/readiness/*kagemusha*.md"',
        "cargo test -p iroha_data_model receiver_snapshot --lib",
        "cargo test -p iroha_core kagemusha_v4 --lib",
        "cargo test -p iroha_core offline_device_attestation_policy --lib",
        "cargo test -p iroha_core device_registration_ --lib",
        "cargo test -p iroha_core kagemusha_online_registration_ --lib",
        "cargo test -p iroha_core active_receiver_snapshot_ --lib",
        "cargo test -p iroha_core --features \"dev-tools,zk-halo2-ipa,kagemusha-candidate-evidence-lab\" --bin kagemusha_recursive_spend_v4_bundle final_release_inventory_is_exact_and_includes_both_receipts",
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
        "name: Verify Kagemusha V4 production readiness (publication blocked)",
        "name: Verify reviewed production inputs (does not publish or activate)",
        "ref: ${{ github.workflow_sha }}",
        "PROMOTION_GITHUB_EVENT_NAME: ${{ github.event_name }}",
        "PROMOTION_GITHUB_REF: ${{ github.ref }}",
        "PROMOTION_GITHUB_REF_PROTECTED: ${{ github.ref_protected }}",
        "PROMOTION_GITHUB_REPOSITORY: ${{ github.repository }}",
        "PROMOTION_GITHUB_RUN_ATTEMPT: ${{ github.run_attempt }}",
        "PROMOTION_GITHUB_RUN_ID: ${{ github.run_id }}",
        "PROMOTION_GITHUB_SHA: ${{ github.sha }}",
        "PROMOTION_GITHUB_WORKFLOW_REF: ${{ github.workflow_ref }}",
        "PROMOTION_GITHUB_WORKFLOW_SHA: ${{ github.workflow_sha }}",
        "PROMOTION_GITHUB_WORKSPACE: ${{ github.workspace }}",
        'test "$PROMOTION_GITHUB_EVENT_NAME" = workflow_dispatch',
        'test "$PROMOTION_GITHUB_REPOSITORY" = hyperledger-iroha/iroha',
        'test "$PROMOTION_GITHUB_REF_PROTECTED" = true',
        '"hyperledger-iroha/iroha/.github/workflows/promote_kagemusha_v4.yml@$PROMOTION_GITHUB_REF"',
        'test "$PROMOTION_GITHUB_WORKFLOW_SHA" = "$PROMOTION_GITHUB_SHA"',
        'test "$workflow_checkout_head" = "$PROMOTION_GITHUB_SHA"',
        'test "$reviewed_checkout_head" = "$PROMOTION_GITHUB_SHA"',
        '-c safe.directory="$reviewed_checkout"',
        'readonly promotion_identity_domain="iroha.kagemusha.github-promotion-run.v1"',
        'readonly catalog_revalidation_receipt_root="/Library/SORA/Kagemusha/catalog-revalidation"',
        'KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT="$catalog_revalidation_receipt_root/$KAGEMUSHA_V4_PROMOTION_ID.json"',
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN: /Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller",
        'gate_snapshot="$gate_launch_dir/check_kagemusha_production_readiness.sh"',
        'KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH="$KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH"',
        'KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD="$KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD"',
        'KAGEMUSHA_V4_PROMOTION_ID="$KAGEMUSHA_V4_PROMOTION_ID"',
        'KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT="$KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT"',
        '/usr/bin/sudo -n /usr/bin/env -i',
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"',
        "launch-kagemusha-readiness-v1",
        '--gate-snapshot "$gate_snapshot"',
        '--gate-source "$KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH"',
        '--python-runtime-tree-sha256 "$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256"')
    if (
        texts[PROMOTION_WORKFLOW].count(
            '-c safe.directory="$reviewed_checkout"'
        )
        != 1
        or "safe.directory=*" in texts[PROMOTION_WORKFLOW]
    ):
        errors.append(
            f"{PROMOTION_WORKFLOW}: reviewed checkout requires one exact "
            "command-scoped safe.directory"
        )
    require_pattern(
        texts[PROMOTION_WORKFLOW],
        PROMOTION_WORKFLOW,
        errors,
        (
            r'reviewed_checkout_head="\$\(.*?'
            r'/usr/bin/git\s+-c\s+safe\.directory="\$reviewed_checkout"\s+\\\s*'
            r'-C\s+"\$reviewed_checkout"\s+\\\s*'
            r"rev-parse\s+--verify\s+'HEAD\^\{commit\}'"
        ),
        "exact reviewed-checkout-only Git ownership exception",
    )
    if re.search(
        r"(?<![/A-Za-z0-9_])sudo(?=\s)", texts[PROMOTION_WORKFLOW]
    ):
        errors.append(
            f"{PROMOTION_WORKFLOW}: privileged commands must use exact /usr/bin/sudo"
        )
    runtime_dispatch = texts[READINESS].rsplit(
        "\nsource_contract_errors: list[str] = []\n", 1
    )[-1]
    source_contract_dispatch, dispatch_separator, self_test_dispatch = (
        runtime_dispatch.partition("\nerrors = source_contract_errors\n")
    )
    if not dispatch_separator:
        errors.append(f"{READINESS}: source-contract dispatch boundary is missing")
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
