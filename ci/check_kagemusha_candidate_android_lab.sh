#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 - "$ROOT_DIR" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])

paths = {
    "settings": root / "kotlin/settings.gradle.kts",
    "gradle": root / "kotlin/kagemusha-candidate-evidence-lab/build.gradle.kts",
    "manifest": root / "kotlin/kagemusha-candidate-evidence-lab/src/main/AndroidManifest.xml",
    "native": root / "kotlin/kagemusha-candidate-evidence-lab/src/main/java/org/hyperledger/iroha/sdk/kagemusha/candidate/lab/KagemushaCandidateLabNative.kt",
    "harness": root / "kotlin/kagemusha-candidate-evidence-lab/src/androidTest/java/org/hyperledger/iroha/sdk/kagemusha/candidate/lab/CandidateLabHarness.kt",
    "lifecycle": root / "kotlin/kagemusha-candidate-evidence-lab/src/androidTest/java/org/hyperledger/iroha/sdk/kagemusha/candidate/lab/KagemushaCandidateLifecycleInstrumentedTest.kt",
    "export": root / "kotlin/kagemusha-candidate-evidence-lab/src/androidTest/java/org/hyperledger/iroha/sdk/kagemusha/candidate/lab/KagemushaCandidateArtifactExportInstrumentedTest.kt",
    "runner": root / "scripts/run_kagemusha_candidate_android_lab.sh",
    "native_builder": root / "scripts/build_kagemusha_candidate_android_native.sh",
    "artifact_stager": root / "scripts/stage_kagemusha_candidate_android_artifacts.py",
    "validator": root / "scripts/check_android_device_lab_slot.py",
    "compile_check": root / "ci/check_kagemusha_candidate_android_lab_compile.sh",
    "stager": root / "scripts/stage_kagemusha_candidate_android_lab.py",
    "staging_spec": root / "specs/sdk/android/readiness/kagemusha_candidate_lab_staging.md",
    "source_seal": root / "scripts/kagemusha_source_tree_seal.py",
    "packaging": root / "scripts/check_mobile_sdk_artifacts.sh",
    "rust": root / "crates/connect_norito_bridge/src/lib.rs",
    "cargo": root / "crates/connect_norito_bridge/Cargo.toml",
    "core_cargo": root / "crates/iroha_core/Cargo.toml",
    "header": root / "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "kagemusha_workflow": root / ".github/workflows/pr_kagemusha_payload_bench.yml",
    "release_workflow": root / ".github/workflows/mobile_sdk_artifacts.yml",
}

errors = []
text = {}
for label, path in paths.items():
    if not path.is_file():
        errors.append(f"missing {label}: {path.relative_to(root)}")
        text[label] = ""
    else:
        text[label] = path.read_text(encoding="utf-8")

def require(label: str, needle: str, message: str) -> None:
    if needle not in text[label]:
        errors.append(message)

legacy_stage_markers = (
    "candidate-stage-manifest-" + "v1",
    "android_candidate_stage_manifest." + "v1",
    "candidate-validation-" + "v1",
    "candidate_validation." + "v1",
    "validate_kagemusha_candidate_stage_manifest_" + "v1",
    "exactly " + "44 entries",
    "44 non-self " + "files",
)
for label in (
    "gradle", "harness", "runner", "native_builder", "artifact_stager",
    "validator", "compile_check", "stager", "staging_spec",
):
    for marker in legacy_stage_markers:
        if marker in text[label]:
            errors.append(f"{label} resurrects retired candidate stage contract: {marker}")

for label in ("gradle", "harness", "validator", "compile_check", "stager"):
    for required in (
        "candidate-stage-manifest-v2.json",
        "candidate-validation-v2.json",
        "recursive-step-two-qualification-v4.norito",
        "self-physical-footprint-v1",
    ):
        require(label, required, f"{label} is missing V2 candidate binding {required}")

for label in ("runner", "native_builder", "artifact_stager"):
    require(
        label,
        "validate_kagemusha_candidate_stage_manifest_v2",
        f"{label} is not wired to the sole V2 candidate-stage validator",
    )

marker = "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
package = "org.hyperledger.iroha.sdk.kagemusha.candidate.lab"
feature = "kagemusha-candidate-evidence-lab"

settings = text["settings"]
conditional = re.search(
    r'if\s*\(providers\.gradleProperty\("kagemushaCandidateEvidenceLab"\)\.orNull\s*==\s*"true"\)\s*\{(?P<body>.*?)\}',
    settings,
    re.DOTALL,
)
if conditional is None or 'include(":kagemusha-candidate-evidence-lab")' not in conditional.group("body"):
    errors.append("candidate lab must be included only by the explicit Gradle opt-in property")
if settings.count('include(":kagemusha-candidate-evidence-lab")') != 1:
    errors.append("candidate lab Gradle inclusion must occur exactly once")

gradle = text["gradle"]
for needle, message in (
    ("alias(libs.plugins.android.application)", "candidate lab must be an Android application"),
    ('beforeVariants(selector().withBuildType("release"))', "candidate lab release variant must be disabled"),
    ("variant.enable = false", "candidate lab release variant must be disabled"),
    ('abiFilters += "arm64-v8a"', "candidate lab must be ARM64-only"),
    (marker, "candidate lab Gradle contract is missing its DO-NOT-SHIP marker"),
    ("artifacts/kagemusha-candidate-evidence/", "candidate lab build must be candidate-rooted"),
    ("evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so", "candidate lab native input path drifted"),
    ("kagemusha-candidate-evidence-lab-DO-NOT-SHIP-", "candidate lab APK name must be unmistakably non-shipping"),
    ('rename { "libconnect_norito_bridge_candidate_lab.so" }', "candidate lab native library must be renamed inside the APK"),
    ('google()', "candidate lab must declare the Google Android repository"),
    ('mavenCentral()', "candidate lab must declare Maven Central"),
    ('kagemushaCandidateCompileOnly', "candidate lab must expose the guarded compile-only contract"),
    ('compileDebugAndroidTestKotlin', "compile-only contract must compile androidTest Kotlin"),
    ('addGeneratedSourceDirectory', "candidate lab generated inputs must use the AGP Variant API"),
    ('androidTestImplementation(project(":core-jvm"))', "candidate lab must declare its AccountAddress dependency"),
    ('stageCandidateLabTestApk', "candidate lab must retain the exact androidTest APK"),
    ('candidate-stage-manifest-v2.json', "candidate lab must bind the exact stage manifest"),
):
    if needle not in gradle:
        errors.append(message)
for forbidden in (
    "maven-publish", "publishing {", "kagemusha-wallet-release.apk",
    "output.outputFileName", "variant.outputFileName",
    "assets.srcDir(generatedAssets)", "jniLibs.srcDir(generatedJni)",
):
    if forbidden in gradle:
        errors.append(f"candidate lab Gradle file must not contain {forbidden}")

manifest = text["manifest"]
if 'android.permission.INTERNET' in manifest:
    errors.append("candidate lab must not request INTERNET permission")
for needle in (marker, 'android:allowBackup="false"'):
    if needle not in manifest:
        errors.append(f"candidate lab manifest is missing {needle}")

kotlin_methods = {
    "nativeBridgeAbiVersion",
    "nativeProductionCapabilityObservedV4",
    "nativeArtifactBeginV4",
    "nativeArtifactWriteV4",
    "nativeArtifactFinalizeV4",
    "nativeArtifactCancelV4",
    "nativeArtifactSetInstallV4",
    "nativeArtifactSetIsInstalledV4",
    "nativeAcceptedIdentityV4",
    "nativeArtifactSetUninstallV4",
    "nativeValidateBranchV4",
    "nativeBuildInitRequestV4",
    "nativeBuildAppendRequestV4",
    "nativeBuildDuplicateInputAppendRequestV4",
    "nativeBuildVerifyRequestV4",
    "nativeBuildRedeemRequestV5",
    "nativeInitV4",
    "nativeAppendV4",
    "nativeVerifyV4",
    "nativeRedeemV4",
    "nativeProjectInitResultV4",
    "nativeProjectSplitResultV4",
    "nativeProjectVerifyResultV4",
    "nativeProjectRedeemResultV4",
}
actual_kotlin_methods = set(re.findall(r'external\s+fun\s+(native[A-Za-z0-9]+)\s*\(', text["native"]))
if actual_kotlin_methods != kotlin_methods:
    errors.append(
        "candidate lab Kotlin JNI surface drifted: "
        f"missing={sorted(kotlin_methods - actual_kotlin_methods)} "
        f"extra={sorted(actual_kotlin_methods - kotlin_methods)}"
    )
for needle in (f"package {package}", 'LIBRARY_NAME: String = "connect_norito_bridge_candidate_lab"', "REQUIRED_BRIDGE_ABI: Int = 23"):
    if needle not in text["native"]:
        errors.append(f"candidate lab Kotlin JNI contract is missing {needle}")

jni_prefix = "Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_"
rust = text["rust"]

def immediate_cfg_before(declaration_start: int):
    """Return the cfg immediately attached to a Rust declaration, if any."""
    prefix_start = max(0, declaration_start - 1_000)
    prefix = rust[prefix_start:declaration_start]
    cfgs = list(re.finditer(r'#\[cfg\((?P<cfg>[^\]]+)\)\]', prefix, re.DOTALL))
    if not cfgs:
        return None
    cfg = cfgs[-1]
    attributes_after_cfg = prefix[cfg.end():]
    if re.fullmatch(r'\s*(?:#\[[^\]]+\]\s*)*', attributes_after_cfg) is None:
        return None
    return cfg.group("cfg"), prefix_start + cfg.start()

def cfg_enables_feature(cfg: str, expected_feature: str) -> bool:
    compact = re.sub(r"\s+", "", cfg)
    feature_clause = f'feature="{expected_feature}"'
    return compact == feature_clause or compact.startswith(f"all({feature_clause},")

jni_module_declarations = list(re.finditer(
    r'^mod\s+kagemusha_candidate_lab_jni\s*\{',
    rust,
    re.MULTILINE,
))
jni_module_body = ""
jni_module_start = -1
jni_module_end = -1
if len(jni_module_declarations) != 1:
    errors.append("candidate-lab JNI module declaration is missing its compile-time guard")
else:
    jni_module_declaration = jni_module_declarations[0]
    module_guard = immediate_cfg_before(jni_module_declaration.start())
if len(jni_module_declarations) == 1 and (
    module_guard is None or not cfg_enables_feature(module_guard[0], feature)
):
    errors.append("candidate-lab JNI module is not feature-gated")
elif len(jni_module_declarations) == 1:
    jni_module_start = jni_module_declaration.end()
    reexports = list(re.finditer(
        r'^pub\s+use\s+kagemusha_candidate_lab_jni::\*\s*;',
        rust[jni_module_start:],
        re.MULTILINE,
    ))
    if len(reexports) != 1:
        errors.append("candidate-lab JNI re-export is missing its compile-time guard")
    else:
        reexport = reexports[0]
        absolute_reexport_start = jni_module_start + reexport.start()
        reexport_guard = immediate_cfg_before(absolute_reexport_start)
        if reexport_guard is None or not cfg_enables_feature(reexport_guard[0], feature):
            errors.append("candidate-lab JNI re-export is not feature-gated")
        else:
            guarded_module = rust[jni_module_start:reexport_guard[1]].rstrip()
            if not guarded_module.endswith("}"):
                errors.append("candidate-lab JNI module does not close before its re-export")
            else:
                jni_module_body = guarded_module[:-1]
                jni_module_end = jni_module_start + len(guarded_module)

for macro_name, end_marker in (
    ("candidate_lab_jni_export", "macro_rules! candidate_lab_jni_forwarders"),
    ("candidate_lab_jni_forwarders", "candidate_lab_jni_export!"),
):
    start_marker = f"macro_rules! {macro_name}"
    if start_marker not in jni_module_body or end_marker not in jni_module_body:
        errors.append(f"candidate-lab JNI generator {macro_name} is missing")
        continue
    generator = jni_module_body.split(start_marker, 1)[1].split(end_marker, 1)[0]
    if "#[unsafe(no_mangle)]" not in generator:
        errors.append(f"candidate-lab JNI generator {macro_name} does not export its symbols")
    if 'pub unsafe extern "system" fn $name(' not in generator:
        errors.append(f"candidate-lab JNI generator {macro_name} is not a JNI function generator")

generated_jni_exports = {}
generated_jni_pattern = re.compile(
    rf'(?P<symbol>{re.escape(jni_prefix)}native[A-Za-z0-9]+)\s*'
    rf'\((?P<params>.*?)\)\s*->\s*'
    rf'(?P<return>\(\)|[A-Za-z_][A-Za-z0-9_:<>\'\?]*)\s*(?:\{{|=>)',
    re.DOTALL,
)
for match in generated_jni_pattern.finditer(jni_module_body):
    symbol = match.group("symbol")
    if symbol in generated_jni_exports:
        errors.append(f"candidate-lab JNI export is generated more than once: {symbol}")
    generated_jni_exports[symbol] = (match.group("params"), match.group("return"))

direct_jni_pattern = re.compile(
    rf'pub\s+unsafe\s+extern\s+"system"\s+fn\s+'
    rf'(?P<symbol>{re.escape(jni_prefix)}native[A-Za-z0-9]+)\s*'
    rf'\((?P<params>.*?)\)\s*->\s*(?P<return>[^\s{{]+)',
    re.DOTALL,
)
direct_jni_exports = {}
for match in direct_jni_pattern.finditer(rust):
    symbol = match.group("symbol")
    if symbol in direct_jni_exports:
        errors.append(f"candidate-lab JNI export is defined more than once: {symbol}")
    direct_jni_exports[symbol] = (match.group("params"), match.group("return"), match.start())
    inside_guarded_module = jni_module_start <= match.start() < jni_module_end
    if not inside_guarded_module:
        guard = immediate_cfg_before(match.start())
        if guard is None or not cfg_enables_feature(guard[0], feature):
            errors.append(f"candidate-lab JNI export {symbol} is not feature-gated")

duplicate_jni_exports = set(generated_jni_exports) & set(direct_jni_exports)
if duplicate_jni_exports:
    errors.append(
        "candidate-lab JNI exports are both direct and macro-generated: "
        f"{sorted(duplicate_jni_exports)}"
    )
actual_rust_jni_methods = {
    symbol.removeprefix(jni_prefix)
    for symbol in set(generated_jni_exports) | set(direct_jni_exports)
}
if actual_rust_jni_methods != kotlin_methods:
    errors.append(
        "candidate-lab Rust JNI surface drifted: "
        f"missing={sorted(kotlin_methods - actual_rust_jni_methods)} "
        f"extra={sorted(actual_rust_jni_methods - kotlin_methods)}"
    )

continuity_signatures = {
    "nativeValidateBranchV4": (
        ["ByteArray", "ByteArray", "ByteArray", "ByteArray", "Long"],
        "ByteArray",
    ),
    "nativeBuildInitRequestV4": (
        ["ByteArray", "ByteArray", "ByteArray", "ByteArray", "ByteArray"],
        "ByteArray",
    ),
    "nativeBuildAppendRequestV4": (
        ["Array<ByteArray>"] * 4 + ["ByteArray"] * 4 + ["Long"],
        "ByteArray",
    ),
    "nativeBuildDuplicateInputAppendRequestV4": (
        ["Array<ByteArray>"] * 4 + ["ByteArray"] * 4 + ["Long"],
        "ByteArray",
    ),
    "nativeBuildVerifyRequestV4": (
        ["ByteArray", "ByteArray", "ByteArray", "Int", "Long", "Long"],
        "ByteArray",
    ),
    "nativeBuildRedeemRequestV5": (
        ["ByteArray"] * 5 + ["Int", "ByteArray", "Int"] + ["ByteArray"] * 4 + ["Long"],
        "ByteArray",
    ),
}
kotlin_to_jni = {
    "ByteArray": "JByteArray",
    "Array<ByteArray>": "JObjectArray",
    "Int": "jint",
    "Long": "jlong",
}
for method, (expected_params, expected_return) in continuity_signatures.items():
    declaration = re.search(
        rf'external\s+fun\s+{method}\s*\((?P<params>.*?)\)\s*:\s*(?P<return>[A-Za-z0-9<>]+)',
        text["native"],
        re.DOTALL,
    )
    if declaration is None:
        errors.append(f"Kotlin continuity JNI declaration is missing for {method}")
        continue
    kotlin_params = []
    for raw in declaration.group("params").split(","):
        raw = raw.strip()
        if not raw:
            continue
        kotlin_params.append(re.sub(r"\s+", "", raw.split(":", 1)[1]))
    kotlin_return = re.sub(r"\s+", "", declaration.group("return"))
    if kotlin_params != expected_params or kotlin_return != expected_return:
        errors.append(
            f"Kotlin continuity JNI signature drifted for {method}: "
            f"params={kotlin_params} return={kotlin_return}"
        )
        continue

    symbol = jni_prefix + method
    if symbol in generated_jni_exports:
        params, rust_return = generated_jni_exports[symbol]
        rust_types = ["JNIEnv", "JClass"]
    elif symbol in direct_jni_exports:
        params, rust_return, _ = direct_jni_exports[symbol]
        rust_types = []
    else:
        continue
    rust_types.extend(re.findall(
        r'^\s*(?:mut\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:\s*([^,\n]+),',
        params,
        re.MULTILINE,
    ))
    rust_types = [value.rsplit("::", 1)[-1].replace("<'_>", "") for value in rust_types]
    expected_rust = ["JNIEnv", "JClass"] + [kotlin_to_jni[value] for value in expected_params]
    rust_return = rust_return.rsplit("::", 1)[-1]
    if rust_types != expected_rust or rust_return != "jbyteArray":
        errors.append(
            f"Rust continuity JNI signature drifted for {method}: "
            f"params={rust_types} return={rust_return}"
        )

c_suffixes = (
    "artifact_begin_v4",
    "artifact_write_v4",
    "artifact_finalize_v4",
    "artifact_cancel_v4",
    "artifact_set_install_v4",
    "artifact_set_is_installed_v4",
    "accepted_identity_v4",
    "artifact_set_uninstall_v4",
    "init_v4",
    "append_v4",
    "verify_v4",
    "redeem_v4",
)
c_prefix = "connect_norito_kagemusha_recursive_spend_candidate_lab_"
c_lifecycle_generator_start = rust.find("macro_rules! kagemusha_recursive_spend_lifecycle_exports")
c_lifecycle_first_invocation = rust.find(
    "kagemusha_recursive_spend_lifecycle_exports! {",
    c_lifecycle_generator_start,
)
if c_lifecycle_generator_start < 0 or c_lifecycle_first_invocation < 0:
    errors.append("candidate-lab C lifecycle export generator is missing")
else:
    c_lifecycle_generator = rust[
        c_lifecycle_generator_start:c_lifecycle_first_invocation
    ]
    for role in ("init", "append", "verify", "redeem"):
        if re.search(
            rf'#\[unsafe\(no_mangle\)\]\s*'
            rf'pub\s+unsafe\s+extern\s+"C"\s+fn\s+\${role}_name\s*\(',
            c_lifecycle_generator,
        ) is None:
            errors.append(
                f"candidate-lab C lifecycle generator is missing exported {role} function"
            )

c_lifecycle_invocation = re.search(
    rf'#\[cfg\(feature\s*=\s*"{re.escape(feature)}"\)\]\s*'
    rf'kagemusha_recursive_spend_lifecycle_exports!\s*\{{(?P<body>.*?)^\}}',
    rust,
    re.MULTILINE | re.DOTALL,
)
generated_c_exports = set()
if c_lifecycle_invocation is None:
    errors.append("candidate-lab C lifecycle inventory is missing its exact feature guard")
else:
    c_lifecycle_body = c_lifecycle_invocation.group("body")
    if "resolver = require_kagemusha_candidate_evidence_lab_artifact_binding_v4;" not in c_lifecycle_body:
        errors.append("candidate-lab C lifecycle inventory does not use the candidate registry")
    if "verify_precheck = false;" not in c_lifecycle_body:
        errors.append("candidate-lab C lifecycle inventory has drifted from its reviewed precheck policy")
    generated_c_exports = set(re.findall(
        rf'=>\s*({re.escape(c_prefix)}(?:init|append|verify|redeem)_v4)\s*,',
        c_lifecycle_body,
    ))
    expected_generated_c_exports = {
        c_prefix + role + "_v4" for role in ("init", "append", "verify", "redeem")
    }
    if generated_c_exports != expected_generated_c_exports:
        errors.append(
            "candidate-lab generated C lifecycle surface drifted: "
            f"missing={sorted(expected_generated_c_exports - generated_c_exports)} "
            f"extra={sorted(generated_c_exports - expected_generated_c_exports)}"
        )

for suffix in c_suffixes:
    symbol = c_prefix + suffix
    match = re.search(rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(symbol)}\s*\(', rust)
    if match is not None and symbol in generated_c_exports:
        errors.append(f"candidate-lab C export is both direct and macro-generated: {symbol}")
        continue
    if match is None and symbol not in generated_c_exports:
        errors.append(f"Rust bridge is missing candidate-lab C export {symbol}")
        continue
    if match is not None:
        guard = immediate_cfg_before(match.start())
        if guard is None or not cfg_enables_feature(guard[0], feature):
            errors.append(f"candidate-lab C export {symbol} is not feature-gated")

if f'{feature} = ["iroha_core/{feature}"]' not in text["cargo"]:
    errors.append("bridge candidate-lab Cargo feature must forward only to iroha_core")
if f"{feature} = []" not in text["core_cargo"]:
    errors.append("iroha_core candidate-lab Cargo feature is missing")
header_guard = re.search(
    r'#ifdef CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB(?P<body>.*?)#endif',
    text["header"],
    re.DOTALL,
)
if header_guard is None:
    errors.append("candidate-lab C declarations must be hidden behind their header macro")
else:
    body = header_guard.group("body")
    for suffix in c_suffixes:
        if c_prefix + suffix not in body:
            errors.append(f"candidate-lab header is missing {c_prefix + suffix}")
    if marker not in body:
        errors.append("candidate-lab header guard is missing its exported marker declaration")

harness = text["harness"]
transcript_fields = {
    "schema", "slot_id", "candidate_record_sha256", "candidate_manifest_sha256",
    "candidate_stage_manifest_path", "candidate_stage_manifest_sha256",
    "candidate_inventory_sha256", "source_commit", "source_tree_sha256",
    "source_repo_dirty", "generation", "bridge_abi_version",
    "production_capability_observed", "initial_atomic", "first_recipient_atomic",
    "second_recipient_atomic", "sender_change_atomic", "redeemed_atomic",
    "final_unspent_atomic", "proof_hops", "init_proof_verified",
    "first_spend_verified", "multi_hop_proof_verified",
    "independent_branch_redemption_verified", "duplicate_rejected",
    "restart_recovered", "network_requests_during_peer_transfers",
    "attestation_challenge_sha256", "attestation_certificate_chain_sha256",
    "app_signing_certificate_sha256", "strongbox_attestation",
    "physical_device_attestation", "causal_events",
}
binding_fields = {
    "schema", "candidate_record_path", "candidate_record_sha256",
    "candidate_manifest_path", "candidate_manifest_sha256",
    "candidate_stage_manifest_path", "candidate_stage_manifest_sha256", "source_commit",
    "source_tree_sha256", "source_repo_dirty", "generation", "bridge_abi_version",
    "lab_native_library_path", "lab_native_library_sha256", "lab_apk_path",
    "lab_apk_sha256", "lab_apk_signing_cert_sha256", "lab_test_apk_path",
    "lab_test_apk_sha256", "lab_test_apk_signing_cert_sha256",
    "production_capability_observed",
    "native_accepted_candidate_record_sha256",
    "native_accepted_candidate_manifest_sha256", "native_accepted_source_commit",
    "native_accepted_source_tree_sha256", "native_accepted_source_repo_dirty",
    "native_accepted_generation", "native_accepted_bridge_abi_version",
    "native_accepted_inventory_sha256", "lifecycle_transcript_path",
    "lifecycle_transcript_sha256", "artifact_inventory",
}
transcript_block = re.search(r'val transcript = JSONObject\(\)(?P<body>.*?)\n\s*val evidenceDir', harness, re.DOTALL)
binding_block = re.search(r'return JSONObject\(\)(?P<body>.*?)\n\s*}', harness, re.DOTALL)
for label, block, expected in (
    ("transcript", transcript_block, transcript_fields),
    ("binding", binding_block, binding_fields),
):
    if block is None:
        errors.append(f"candidate {label} JSON construction is missing")
        continue
    actual = set(re.findall(r'\.put\(\s*"([a-z0-9_]+)"', block.group("body")))
    if actual != expected:
        errors.append(
            f"candidate {label} closed field set drifted: "
            f"missing={sorted(expected - actual)} extra={sorted(actual - expected)}"
        )

for needle, message in (
    ("ACCEPTED_IDENTITY_FIELD_COUNT = 49", "native identity must have exact 49-field arity"),
    ("nativeInitV4", "candidate lifecycle must call native init"),
    ("nativeAppendV4", "candidate lifecycle must call native append"),
    ("nativeVerifyV4", "candidate lifecycle must call native verify"),
    ("nativeRedeemV4", "candidate lifecycle must call native redeem"),
    ("nativeBuildInitRequestV4", "candidate lifecycle must build init on-device"),
    ("nativeBuildAppendRequestV4", "candidate lifecycle must build appends from observed branches"),
    ("nativeBuildVerifyRequestV4", "candidate lifecycle must build verify requests from observed branches"),
    ("nativeBuildRedeemRequestV5", "candidate lifecycle must build redemptions from observed branches"),
    ("nativeBuildDuplicateInputAppendRequestV4", "duplicate test must derive from an observed branch"),
    ("nativeValidateBranchV4", "candidate lifecycle must independently validate observed branches"),
    ("val bundle: ByteArray", "branch projection must retain the exact native bundle"),
    ("val topUpProvenance: ByteArray", "branch projection must retain exact provenance"),
    ("val membershipWitness: ByteArray", "branch projection must retain the exact witness"),
    ("context.noBackupFilesDir", "private lifecycle state must stay in no-backup app storage"),
    ("writePrivateAtomic", "private lifecycle openings must be persisted with restricted access"),
    ("firstPid != secondPid", "candidate lifecycle must prove process restart"),
    ("network_requests_during_peer_transfers", "candidate lifecycle must record its offline transfer count"),
    ('"candidate-binding-v2.json"', "candidate binding export is missing"),
    ('"lifecycle-transcript-v2.json"', "candidate transcript export is missing"),
    ("CausalEvents", "candidate lifecycle must emit the exact causal event sequence"),
    ("candidate_stage_manifest_sha256", "candidate lifecycle must bind the stage manifest"),
    ("kagemushaAttestationChallengeHex", "candidate lifecycle must consume the exact attestation challenge"),
    ("lab_test_apk_signing_cert_sha256", "candidate binding must retain the test APK signer"),
):
    if needle not in harness:
        errors.append(message)
if harness.count("verifyBranch(") != 3:
    errors.append("candidate lifecycle must define and invoke exactly two recipient verify flows")
if harness.count("redeemBranch(") != 4:
    errors.append("candidate lifecycle must define and invoke exactly three native redeem scenarios")
if harness.count("validateBranch(") != 6:
    errors.append("candidate lifecycle must define and invoke exactly five independent branch validations")
if harness.count("KagemushaCandidateLabNative.nativeBuildAppendRequestV4(") != 2:
    errors.append("candidate lifecycle must build exactly two appends from exact projected branches")
if harness.count("KagemushaCandidateLabNative.nativeBuildDuplicateInputAppendRequestV4(") != 1:
    errors.append("candidate lifecycle must build one duplicate-input request from an exact projected branch")

for forbidden in (
    "append-hop-01-request-v4.norito",
    "append-hop-02-request-v4.norito",
    "verify-init-request-v4.norito",
    "verify-hop-01-recipient-request-v4.norito",
    "verify-hop-02-recipient-request-v4.norito",
    "redeem-hop-01-recipient-request-v4.norito",
    "redeem-hop-02-recipient-request-v4.norito",
    "redeem-sender-change-request-v4.norito",
    "duplicate-input-append-request-v4.norito",
):
    if forbidden in harness or forbidden in gradle:
        errors.append(f"candidate lifecycle must not package precomputed request {forbidden}")

for label, class_name in (
    ("lifecycle", "KagemushaCandidateLifecycleInstrumentedTest"),
    ("export", "KagemushaCandidateArtifactExportInstrumentedTest"),
):
    if f"class {class_name}" not in text[label] or f"package {package}" not in text[label]:
        errors.append(f"candidate lab is missing exact instrumentation class {class_name}")

runner = text["runner"]
for needle in (
    "--build-only",
    "--stage-sha256",
    "--trusted-signer-public-key",
    "--android-attestation-trust-root-sha256",
    "--android-attestation-revocation-status-sha256",
    "--android-attestation-status-capture-receipt",
    "--android-attestation-status-capture-receipt-sha256",
    "--java-sha256",
    "--apksigner-jar-sha256",
    "--openssl-sha256",
    "kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$CANDIDATE_SHA256-debug.apk",
    "kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$CANDIDATE_SHA256-debug-androidTest.apk",
    '"$LIFECYCLE_CLASS"',
    '"$EXPORT_CLASS"',
    'shell am instrument -w -r',
    'check_android_device_lab_slot.py"',
    "--require-kagemusha-production-evidence",
    "TRUSTED_SLOT_SUMMARY",
    "verify_attestation_authority_inputs",
    "--confirmation-reference-slot",
    "--confirmation-binding",
    "--confirmation-lifecycle",
    "--confirmation-json-out",
    "candidate-confirmation-comparison-v1.json",
    "candidate-full-run-receipt-v1.json",
    '"confirmation_candidate_binding"',
    '"confirmation_lifecycle_transcript"',
    '"confirmation_semantic_comparison"',
    '"confirmation_comparator"',
    '"executed_commands"',
    '"android_attestation_status_capture_receipt"',
    '"android_status_snapshot"',
    '"attestation_status_capture_receipt_sha256"',
    '"<pinned-status-capture-receipt>"',
    "cleanup_device_state",
):
    if needle not in runner:
        errors.append(f"candidate lab runner is missing {needle}")
if runner.index('"$LIFECYCLE_CLASS"') > runner.index('"$EXPORT_CLASS"'):
    errors.append("candidate lifecycle instrumentation must run before restart/export")
if "kagemushaCandidateCompileOnly" in runner:
    errors.append("physical evidence runner must never enable the compile-only fixture contract")
for corridor_marker in (
    'AUTHORITY_STATUS_CAPTURE_RECEIPT=""',
    'AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256=""',
    '|| -n "$AUTHORITY_STATUS_CAPTURE_RECEIPT"',
    '&& -n "$AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256"',
    'verify_pinned_file \\\n    "$AUTHORITY_STATUS_CAPTURE_RECEIPT"',
    '--android-attestation-status-capture-receipt "$AUTHORITY_STATUS_CAPTURE_RECEIPT"',
    '--android-attestation-status-capture-receipt-sha256 "$AUTHORITY_STATUS_CAPTURE_RECEIPT_SHA256"',
    'kagemusha.get("authority_tools") != expected_authority',
    'summary_kagemusha.get("authority_tools") != expected_authority',
    'report.get("authority_tools") != expected_authority',
):
    if corridor_marker not in runner:
        errors.append(
            "candidate status-capture authority corridor is missing "
            f"{corridor_marker!r}"
        )
if runner.count("--android-attestation-status-capture-receipt-sha256") < 6:
    errors.append(
        "candidate status-capture receipt pin must reach usage, parsing, validation, "
        "both validator calls, and both redacted command templates"
    )
if runner.count('"${AUTHORITY_VALIDATOR_ARGS[@]}"') != 2:
    errors.append(
        "the complete pinned authority corridor must reach validation and confirmation"
    )
for label in ("runner", "validator"):
    if '"--apksigner-sha256"' in text[label]:
        errors.append(
            f"{label} must pin Java and apksigner.jar, not the dependency-loading shell launcher"
        )

builder = text["native_builder"]
for needle in (
    "kagemusha_source_tree_seal.py",
    "source_fingerprint",
    "--reviewed-source-closure",
    "--reviewed-source-closure-sha256",
    "source changed during the native build",
    '"$CARGO_NDK_BINARY" -t arm64-v8a',
    "build --locked --offline --release",
    "CARGO_NET_OFFLINE=true",
    "--no-default-features",
    "--features kagemusha-candidate-evidence-lab",
    "CARGO_TARGET_DIR=\"$CARGO_TARGET_DIR\"",
    "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
    "Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_",
    "rename_no_replace",
    "mode-0555 non-hard-linked regular file",
):
    if needle not in builder:
        errors.append(f"candidate native source-sealed build is missing {needle}")
if "build_kagemusha_candidate_android_native.sh" not in runner:
    errors.append("candidate device runner must build the lab bridge from current source")
for needle in ("--reviewed-source-closure", "--reviewed-source-closure-sha256"):
    if needle not in runner:
        errors.append(f"candidate device runner does not thread {needle}")
if "norito_bridge_source_seal.py" in builder:
    errors.append("candidate Android build must not use the Apple-only bridge-closure source seal")

validator = text["validator"]
for needle in (
    "KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND",
    "KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND",
    "KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND",
    "KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND",
    "derive_kagemusha_strongbox_challenge_v1",
    "extract_apk_signing_certificate_sha256",
    '"-jar"',
    '"apksigner_jar"',
    "validate_kagemusha_candidate_stage_manifest_v2",
    "validate_kagemusha_android_confirmation",
    "KAGEMUSHA_ANDROID_CONFIRMATION_COMPARISON_SCHEMA_V1",
    "--trusted-signer-public-key",
    "--android-attestation-trust-root-sha256",
    "--android-attestation-revocation-status-sha256",
    "--android-attestation-status-capture-receipt",
    "--android-attestation-status-capture-receipt-sha256",
    '"attestation_status_capture_receipt_sha256"',
    '"android_status_snapshot"',
    "--confirmation-reference-slot",
    "--confirmation-binding",
    "--confirmation-lifecycle",
    "--confirmation-json-out",
    "only_duration_nanos_may_differ",
):
    if needle not in validator:
        errors.append(f"authoritative candidate slot validator is missing {needle}")

compile_check = text["compile_check"]
for needle in (
    "--warm-dependencies",
    "-PkagemushaCandidateCompileOnly=true",
    ":kagemusha-candidate-evidence-lab:compileDebugKotlin",
    ":kagemusha-candidate-evidence-lab:compileDebugAndroidTestKotlin",
    "--offline",
    "--rerun-tasks",
    "--max-workers=2",
    "GRADLE_RO_DEP_CACHE",
    "compile-gradle-read-only-cache",
    "chmod -R u+w",
):
    if needle not in compile_check:
        errors.append(f"actual AGP/Kotlin compile-only check is missing {needle}")

warm_command = "ci/check_kagemusha_candidate_android_lab_compile.sh --warm-dependencies"
offline_command = "run: ci/check_kagemusha_candidate_android_lab_compile.sh\n"
for workflow_label in ("kagemusha_workflow", "release_workflow"):
    workflow = text[workflow_label]
    if workflow.count(warm_command) != 1:
        errors.append(
            f"{workflow_label} must warm the exact candidate Android dependency graph once"
        )
    if workflow.count(offline_command) != 1:
        errors.append(
            f"{workflow_label} must prove candidate Android compilation offline once"
        )
    if (
        workflow.count(warm_command) == 1
        and workflow.count(offline_command) == 1
        and workflow.index(warm_command) > workflow.index(offline_command)
    ):
        errors.append(
            f"{workflow_label} must warm Android dependencies before the offline proof"
        )
    if ":offline-wallet-android:compileDebugAndroidTestKotlin" in workflow:
        errors.append(
            f"{workflow_label} must compile the exact candidate app, not a proxy module"
        )

source_seal = text["source_seal"]
for needle in (
    "iroha.kagemusha.full-source-tree-sha256.v4",
    '"status",',
    '"--porcelain=v1",',
    '"-z",',
    '"--untracked-files=all",',
    "ALLOWED_INDEX_MODES",
    "ALLOWED_UNTRACKED_MODES",
    "head_before = _head(root)",
    "_index_entries(root)",
):
    if needle not in source_seal:
        errors.append(f"canonical Kagemusha full-source-tree seal is missing {needle}")

for needle in (marker, "kagemusha_recursive_spend_candidate_lab_"):
    if needle not in text["packaging"]:
        errors.append(f"production mobile packaging does not reject {needle}")
if ":kagemusha-candidate-evidence-lab" in text["release_workflow"]:
    errors.append("production mobile workflow must never build or publish the candidate lab")

if errors:
    for error in errors:
        print(f"[kagemusha-candidate-android-lab] ERROR: {error}", file=sys.stderr)
    raise SystemExit(1)
print("[kagemusha-candidate-android-lab] static contract passed")
PY

bash -n \
  "$ROOT_DIR/scripts/run_kagemusha_candidate_android_lab.sh" \
  "$ROOT_DIR/scripts/build_kagemusha_candidate_android_native.sh" \
  "$ROOT_DIR/ci/check_kagemusha_candidate_android_lab_compile.sh"
python3 -m py_compile \
  "$ROOT_DIR/scripts/check_android_device_lab_slot.py" \
  "$ROOT_DIR/scripts/stage_kagemusha_candidate_android_lab.py" \
  "$ROOT_DIR/scripts/kagemusha_source_tree_seal.py"
python3 "$ROOT_DIR/scripts/tests/kagemusha_source_tree_seal_test.py"
python3 "$ROOT_DIR/scripts/tests/stage_kagemusha_candidate_android_lab_test.py"

if [[ "${KAGEMUSHA_ANDROID_COMPILE_CHECK:-0}" == "1" ]]; then
  "$ROOT_DIR/ci/check_kagemusha_candidate_android_lab_compile.sh"
fi
