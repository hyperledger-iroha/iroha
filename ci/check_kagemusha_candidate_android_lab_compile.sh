#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON3_BINARY="$(command -v python3 2>/dev/null || true)"
[[ -x "$PYTHON3_BINARY" ]] || {
  echo "[kagemusha-candidate-compile] ERROR: python3 is required" >&2
  exit 69
}

FIXTURE_VALUES="$("$PYTHON3_BINARY" -I - "$ROOT_DIR" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import sys

root = Path(sys.argv[1]).resolve()
nonce = f"compile-only-{os.getpid()}"
candidate_payload = (nonce + ":candidate-v4\n").encode()
candidate_sha = hashlib.sha256(candidate_payload).hexdigest()
candidate_parent = root / "artifacts/kagemusha-candidate-evidence" / candidate_sha
pending = candidate_parent / f".compile-fixture-{os.getpid()}"
pending.mkdir(parents=True, mode=0o700)

artifact_names = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
scenario_names = (
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)

def publish(relative: str, payload: bytes) -> None:
    path = pending / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    os.chmod(path, 0o600)

manifest_payload = (nonce + ":manifest-v4\n").encode()
qualification_receipt = (nonce + ":recursive-step-two-qualification-v4\n").encode()
publish("evidence/candidate/candidate-v4.norito", candidate_payload)
publish("evidence/candidate/manifest-v4.norito", manifest_payload)
publish(
    "evidence/candidate/recursive-step-two-qualification-v4.norito",
    qualification_receipt,
)
artifact_reports = []
artifact_roles = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
for index, (role, name) in enumerate(zip(artifact_roles, artifact_names), start=1):
    payload = (nonce + ":" + name + "\n").encode()
    publish(f"evidence/candidate/artifacts/{name}", payload)
    artifact_reports.append({
        "role": role,
        "file_name": name,
        "framed_size_bytes": len(payload),
        "framed_sha256": hashlib.sha256(payload).hexdigest(),
        "payload_size_bytes": len(payload) - 1,
        "payload_sha256": hashlib.sha256(
            f"{nonce}:payload:{index}:{name}".encode()
        ).hexdigest(),
    })
receipt_sha = hashlib.sha256(qualification_receipt).hexdigest()
qualified = hashlib.sha256(
    b"iroha:kagemusha:recursive-spend-qualified-candidate:v4\0"
    + bytes.fromhex(candidate_sha)
    + bytes.fromhex(receipt_sha)
).hexdigest()
source_commit = "1" * 40
source_tree_sha = hashlib.sha256((nonce + ":source-tree").encode()).hexdigest()
validation_report = {
    "schema": "iroha.kagemusha.recursive_spend.candidate_validation.v2",
    "candidate_record_sha256": candidate_sha,
    "candidate_manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
    "qualification_receipt_file_name": "recursive-step-two-qualification-v4.norito",
    "qualification_receipt_sha256": receipt_sha,
    "qualified_candidate_sha256": qualified,
    "source_commit": source_commit,
    "source_tree_sha256": source_tree_sha,
    "source_repo_dirty": False,
    "reviewed_source_closure_descriptor_sha256": "6" * 64,
    "authenticated_source_seal_projection_sha256": "7" * 64,
    "reviewed_cargo_binary_sha256": "8" * 64,
    "reviewed_rustc_binary_sha256": "9" * 64,
    "generation": "compile-only",
    "generation_memory_limit_bytes": 6 * 1024 * 1024 * 1024,
    "generation_memory_enforcement_profile": "self-physical-footprint-v1",
    "bridge_abi_version": 22,
    "artifact_count": len(artifact_reports),
    "artifacts": artifact_reports,
    "topup_finality_roster_file_name": "topup-finality-roster-v4.norito",
    "topup_finality_roster_size_bytes": len(b"compile-roster"),
    "topup_finality_roster_sha256": hashlib.sha256(b"compile-roster").hexdigest(),
}
publish(
    "evidence/candidate/candidate-validation-v2.json",
    (json.dumps(validation_report, sort_keys=True, separators=(",", ":")) + "\n").encode(),
)
for name in scenario_names:
    publish(f"scenario/{name}", (nonce + ":" + name + "\n").encode())

relative_paths = sorted(
    (
        "evidence/candidate/candidate-v4.norito",
        "evidence/candidate/manifest-v4.norito",
        "evidence/candidate/candidate-validation-v2.json",
        "evidence/candidate/recursive-step-two-qualification-v4.norito",
        *(f"evidence/candidate/artifacts/{name}" for name in artifact_names),
        *(f"scenario/{name}" for name in scenario_names),
    ),
    key=lambda value: value.encode("utf-8"),
)
entries = []
for relative in relative_paths:
    payload = (pending / relative).read_bytes()
    entries.append({
        "path": relative,
        "mode": "0600",
        "size_bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    })

scenario_entries = [entry for entry in entries if entry["path"].startswith("scenario/")]
scenario_digest = hashlib.sha256()
scenario_digest.update(b"iroha.kagemusha.android-candidate-scenario-inventory.v1\0")
scenario_digest.update(len(scenario_entries).to_bytes(4, "big"))
for entry in scenario_entries:
    path_bytes = entry["path"].encode("utf-8")
    scenario_digest.update(len(path_bytes).to_bytes(4, "big"))
    scenario_digest.update(path_bytes)
    scenario_digest.update(entry["size_bytes"].to_bytes(8, "big"))
    scenario_digest.update(bytes.fromhex(entry["sha256"]))

manifest = {
    "schema": "iroha.kagemusha.android_candidate_stage_manifest.v2",
    "version": 2,
    "stage_manifest_path": "candidate-stage-manifest-v2.json",
    "stage_manifest_mode": "0600",
    "stage_manifest_size_bytes": 0,
    "candidate_record_sha256": candidate_sha,
    "candidate_manifest_sha256": hashlib.sha256(
        (pending / "evidence/candidate/manifest-v4.norito").read_bytes()
    ).hexdigest(),
    "candidate_validation_report_sha256": hashlib.sha256(
        (pending / "evidence/candidate/candidate-validation-v2.json").read_bytes()
    ).hexdigest(),
    "qualification_receipt_sha256": receipt_sha,
    "qualified_candidate_sha256": qualified,
    "scenario_inventory_sha256": scenario_digest.hexdigest(),
    "source_commit": source_commit,
    "source_tree_sha256": source_tree_sha,
    "source_repo_dirty": False,
    "validator": {
        "schema": "iroha.kagemusha.android_candidate_validator.v1",
        "candidate_binary_name": "kagemusha_recursive_spend_v4_bundle",
        "candidate_binary_sha256": "2" * 64,
        "scenario_binary_name": "kagemusha_candidate_scenario_validator",
        "scenario_binary_sha256": "3" * 64,
        "cargo_binary_sha256": "4" * 64,
        "cargo_version_verbose": "cargo compile-only fixture\n",
        "rustc_binary_sha256": "5" * 64,
        "rustc_version_verbose": "rustc compile-only fixture\n",
        "locked": True,
        "offline": True,
        "isolated_target": True,
        "build_jobs": 2,
        "candidate_package": "iroha_core",
        "scenario_package": "connect_norito_bridge",
        "features": ["kagemusha-candidate-evidence-lab"],
        "profile": "debug",
    },
    "entry_count": len(entries),
    "scenario_entry_count": len(scenario_entries),
    "entries": entries,
}
while True:
    encoded = (json.dumps(manifest, sort_keys=True, separators=(",", ":"),
                          ensure_ascii=True) + "\n").encode()
    if manifest["stage_manifest_size_bytes"] == len(encoded):
        break
    manifest["stage_manifest_size_bytes"] = len(encoded)
publish("candidate-stage-manifest-v2.json", encoded)
stage_sha = hashlib.sha256(encoded).hexdigest()
evidence_root = candidate_parent / stage_sha
pending.rename(evidence_root)
native = evidence_root / "evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so"
native.parent.mkdir(parents=True, exist_ok=True)
native.write_bytes(
    b"KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2\n"
    b"Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_\n"
)
os.chmod(native, 0o600)
print("|".join((candidate_sha, stage_sha, source_commit, source_tree_sha,
                str(evidence_root), str(native))))
PY
)"
IFS='|' read -r CANDIDATE_SHA256 STAGE_SHA256 SOURCE_COMMIT SOURCE_TREE_SHA256 \
  EVIDENCE_ROOT NATIVE_LIBRARY <<<"$FIXTURE_VALUES"
[[ -n "$NATIVE_LIBRARY" && "$EVIDENCE_ROOT" == "$ROOT_DIR"/* ]] || {
  echo "[kagemusha-candidate-compile] ERROR: fixture construction failed" >&2
  exit 1
}
FIXTURE_CANDIDATE_ROOT="$ROOT_DIR/artifacts/kagemusha-candidate-evidence/$CANDIDATE_SHA256"
EXTERNAL_BUILD_ROOT=""
cleanup() {
  if [[ -d "$FIXTURE_CANDIDATE_ROOT" && ! -L "$FIXTURE_CANDIDATE_ROOT" ]]; then
    chmod -R u+w "$FIXTURE_CANDIDATE_ROOT" 2>/dev/null || true
  fi
  rm -rf "$FIXTURE_CANDIDATE_ROOT"
  if [[ -n "$EXTERNAL_BUILD_ROOT" && -d "$EXTERNAL_BUILD_ROOT" && ! -L "$EXTERNAL_BUILD_ROOT" ]]; then
    rm -rf "$EXTERNAL_BUILD_ROOT"
  fi
}
trap cleanup EXIT

JAVA_HOME_RESOLVED="${JAVA_HOME:-}"
if [[ -z "$JAVA_HOME_RESOLVED" && -x /usr/libexec/java_home ]]; then
  JAVA_HOME_RESOLVED="$(/usr/libexec/java_home -v 21)"
fi
[[ -x "$JAVA_HOME_RESOLVED/bin/java" ]] || {
  echo "[kagemusha-candidate-compile] ERROR: JDK 21 is required" >&2
  exit 69
}
ANDROID_SDK_RESOLVED="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
[[ -d "$ANDROID_SDK_RESOLVED" ]] || {
  echo "[kagemusha-candidate-compile] ERROR: Android SDK is required" >&2
  exit 69
}
PRIVATE_GRADLE_HOME="$EVIDENCE_ROOT/compile-gradle-user-home"
mkdir -p "$PRIVATE_GRADLE_HOME/caches" "$PRIVATE_GRADLE_HOME/wrapper"
chmod 0700 "$PRIVATE_GRADLE_HOME" "$PRIVATE_GRADLE_HOME/caches" "$PRIVATE_GRADLE_HOME/wrapper"
EXTERNAL_BUILD_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-candidate-compile.XXXXXXXX")"
EXTERNAL_BUILD_ROOT="$("$PYTHON3_BINARY" -I -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$EXTERNAL_BUILD_ROOT")"
WARM_ARTIFACT_ROOT="$EXTERNAL_BUILD_ROOT/warm-gradle-artifacts"
FINAL_ARTIFACT_ROOT="$EXTERNAL_BUILD_ROOT/final-gradle-artifacts"
mkdir -m 0700 "$WARM_ARTIFACT_ROOT" "$FINAL_ARTIFACT_ROOT"
SOURCE_GRADLE_HOME="${GRADLE_USER_HOME:-$HOME/.gradle}"
SOURCE_DEPENDENCY_CACHE="$SOURCE_GRADLE_HOME/caches/modules-2"
CANDIDATE_IDENTITY_PROPERTIES=(
  -PkagemushaCandidateEvidenceLab=true
  -PkagemushaCandidateSha256="$CANDIDATE_SHA256"
  -PkagemushaCandidateStageSha256="$STAGE_SHA256"
  -PkagemushaCandidateEvidenceRoot="$EVIDENCE_ROOT"
  -PkagemushaCandidateSourceCommit="$SOURCE_COMMIT"
  -PkagemushaCandidateSourceTreeSha256="$SOURCE_TREE_SHA256"
  -PkagemushaCandidateGeneration=compile-only
  -PkagemushaCandidateSlotId=compile-only
  -PkagemushaCandidateLabNativeLibrary="$NATIVE_LIBRARY"
)
case "${KAGEMUSHA_CANDIDATE_COMPILE_WARM_GRADLE_CACHE:-0}" in
  0) ;;
  1)
    mkdir -p "$SOURCE_GRADLE_HOME"
    chmod 0700 "$SOURCE_GRADLE_HOME"
    (
      cd "$ROOT_DIR/kotlin"
      /usr/bin/env -i \
        HOME="$HOME" \
        PATH="$JAVA_HOME_RESOLVED/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
        TMPDIR="${TMPDIR:-/tmp}" \
        LANG="${LANG:-C.UTF-8}" \
        JAVA_HOME="$JAVA_HOME_RESOLVED" \
        ANDROID_HOME="$ANDROID_SDK_RESOLVED" \
        ANDROID_SDK_ROOT="$ANDROID_SDK_RESOLVED" \
        GRADLE_USER_HOME="$SOURCE_GRADLE_HOME" \
        MOBILE_SDK_ANDROID_ARTIFACT_DIR="$WARM_ARTIFACT_ROOT" \
        ./gradlew --no-daemon --max-workers=2 \
        --project-cache-dir "$EVIDENCE_ROOT/warm-gradle-project-cache" \
        -Pkotlin.compiler.execution.strategy=in-process \
        -PkagemushaCandidateCompileOnly=true \
        "${CANDIDATE_IDENTITY_PROPERTIES[@]}" \
        :kagemusha-candidate-evidence-lab:compileDebugKotlin \
        :kagemusha-candidate-evidence-lab:compileDebugAndroidTestKotlin
    )
    chmod -R u+w "$EVIDENCE_ROOT/gradle" "$EVIDENCE_ROOT/warm-gradle-project-cache" 2>/dev/null || true
    rm -rf -- "$EVIDENCE_ROOT/gradle" "$EVIDENCE_ROOT/warm-gradle-project-cache"
    rm -rf -- "$WARM_ARTIFACT_ROOT"
    ;;
  *)
    echo "[kagemusha-candidate-compile] ERROR: KAGEMUSHA_CANDIDATE_COMPILE_WARM_GRADLE_CACHE must be exactly 0 or 1" >&2
    exit 64
    ;;
esac
[[ -d "$SOURCE_DEPENDENCY_CACHE" ]] || {
  echo "[kagemusha-candidate-compile] ERROR: warmed Gradle dependency cache is required" >&2
  exit 69
}
READ_ONLY_DEPENDENCY_CACHE="$EVIDENCE_ROOT/compile-gradle-read-only-cache"
mkdir -m 0700 "$READ_ONLY_DEPENDENCY_CACHE"
case "$(uname -s)" in
  Darwin)
    /bin/cp -cR "$SOURCE_DEPENDENCY_CACHE" "$READ_ONLY_DEPENDENCY_CACHE/modules-2"
    ;;
  Linux)
    cp -a --reflink=auto \
      "$SOURCE_DEPENDENCY_CACHE" "$READ_ONLY_DEPENDENCY_CACHE/modules-2"
    ;;
  *)
    echo "[kagemusha-candidate-compile] ERROR: unsupported Gradle cache snapshot host" >&2
    exit 69
    ;;
esac
rm -f \
  "$READ_ONLY_DEPENDENCY_CACHE/modules-2/modules-2.lock" \
  "$READ_ONLY_DEPENDENCY_CACHE/modules-2/gc.properties"
chmod -R a-w "$READ_ONLY_DEPENDENCY_CACHE"
if [[ -d "$SOURCE_GRADLE_HOME/wrapper/dists" ]]; then
  ln -s "$SOURCE_GRADLE_HOME/wrapper/dists" "$PRIVATE_GRADLE_HOME/wrapper/dists"
fi

(
  cd "$ROOT_DIR/kotlin"
  /usr/bin/env -i \
    HOME="$HOME" \
    PATH="$JAVA_HOME_RESOLVED/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    TMPDIR="${TMPDIR:-/tmp}" \
    LANG="${LANG:-C.UTF-8}" \
    JAVA_HOME="$JAVA_HOME_RESOLVED" \
    ANDROID_HOME="$ANDROID_SDK_RESOLVED" \
    ANDROID_SDK_ROOT="$ANDROID_SDK_RESOLVED" \
    GRADLE_USER_HOME="$PRIVATE_GRADLE_HOME" \
    GRADLE_RO_DEP_CACHE="$READ_ONLY_DEPENDENCY_CACHE" \
    MOBILE_SDK_ANDROID_ARTIFACT_DIR="$FINAL_ARTIFACT_ROOT" \
    ./gradlew --no-daemon --offline --max-workers=2 \
    --project-cache-dir "$EVIDENCE_ROOT/compile-gradle-project-cache" \
    -Pkotlin.compiler.execution.strategy=in-process \
    -PkagemushaCandidateCompileOnly=true \
    "${CANDIDATE_IDENTITY_PROPERTIES[@]}" \
    :kagemusha-candidate-evidence-lab:compileDebugKotlin \
    :kagemusha-candidate-evidence-lab:compileDebugAndroidTestKotlin
)

echo "[kagemusha-candidate-compile] actual AGP/Kotlin main+androidTest compilation passed"
