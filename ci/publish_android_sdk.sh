#!/usr/bin/env bash
# Build and publish the Android SDK artefacts (AAR + JVM jar) with SBOMs and runtime metadata.
# Outputs are copied into artifacts/android/publish by default.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_ROOT="${ROOT_DIR}/java/iroha_android"
OUT_DIR="${ANDROID_PUBLISH_OUT:-${ROOT_DIR}/artifacts/android/publish}"
REPO_DIR="${IROHA_ANDROID_REPO_DIR:-${PROJECT_ROOT}/build/maven}"
VERSION="${IROHA_ANDROID_VERSION:-0.1.0-SNAPSHOT}"
GRADLE_BIN="${GRADLE_BIN:-}"
WRAPPER_FALLBACK="${ROOT_DIR}/examples/android/gradlew"

ensure_gradle() {
    if [[ -n "${GRADLE_BIN}" && -x "${GRADLE_BIN}" ]]; then
        echo "${GRADLE_BIN}"
        return
    fi
    if [[ -x "${WRAPPER_FALLBACK}" ]]; then
        echo "${WRAPPER_FALLBACK}"
        return
    fi
    if command -v gradle >/dev/null 2>&1; then
        echo "gradle"
        return
    fi
    echo "Gradle wrapper not found; set GRADLE_BIN or ensure ${WRAPPER_FALLBACK} exists." >&2
    exit 1
}

GRADLE="$(ensure_gradle)"

run_gradle() {
    "${GRADLE}" -p "${PROJECT_ROOT}" --no-daemon --stacktrace \
        -PirohaAndroidVersion="${VERSION}" \
        -PirohaAndroidRepoDir="${REPO_DIR}" \
        "$@"
}

# Build + publish artefacts with metadata.
run_gradle \
    :core:check \
    :android:publish \
    :android:cyclonedxBom \
    :jvm:publish \
    :jvm:cyclonedxBom

# Verify the sample against the published AAR unless explicitly skipped.
if [[ "${ANDROID_PUBLISH_SKIP_SAMPLE:-0}" != "1" ]]; then
    run_gradle \
        -PirohaAndroidUsePublished=true \
        :samples-android:assembleDebug
fi

mkdir -p "${OUT_DIR}"/{android,jvm}

python3 - "${ROOT_DIR}" "${OUT_DIR}" "${VERSION}" <<'PY'
import json
import os
import shutil
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
out_dir = Path(sys.argv[2]).resolve()
version = sys.argv[3]
project = root / "java" / "iroha_android"

targets = {
    "android": {
        "runtime": project
        / "android"
        / "build"
        / "reports"
        / "publishing"
        / "android-runtime-manifest.json",
        "dependency": project
        / "android"
        / "build"
        / "reports"
        / "publishing"
        / f"iroha-android-{version}-releaseRuntimeClasspath.json",
        "checksum": project / "android" / "build" / "reports" / "publishing" / "android-release.aar.sha256",
    },
    "jvm": {
        "dependency": project
        / "jvm"
        / "build"
        / "reports"
        / "publishing"
        / f"iroha-android-jvm-{version}-runtimeClasspath.json",
        "artifact": project / "jvm" / "build" / "libs" / f"iroha-android-jvm-{version}.jar",
        "sbom_dir": project / "jvm" / "build" / "reports" / "bom",
    },
}


def require(path: Path, label: str) -> Path:
    if not path.exists():
        sys.stderr.write(f"missing {label}: {path}\n")
        sys.exit(1)
    return path


for kind, cfg in targets.items():
    dest = out_dir / kind
    dest.mkdir(parents=True, exist_ok=True)

    dep_manifest = require(cfg["dependency"], f"{kind} dependency manifest")
    shutil.copy2(dep_manifest, dest / dep_manifest.name)

    if kind == "android":
        runtime_manifest = require(cfg["runtime"], f"{kind} runtime manifest")
        with runtime_manifest.open("r", encoding="utf-8") as handle:
            manifest = json.load(handle)

        for label, path in (
            ("runtime manifest", runtime_manifest),
            ("checksum", require(cfg["checksum"], f"{kind} checksum")),
        ):
            shutil.copy2(path, dest / path.name)

        artifact_path = require(root / manifest["artifact"]["path"], f"{kind} artefact")
        shutil.copy2(artifact_path, dest / artifact_path.name)

        sbom_path = manifest.get("sbom", {}).get("path")
        if sbom_path:
            sbom_full = root / sbom_path
            require(sbom_full, f"{kind} SBOM")
            shutil.copy2(sbom_full, dest / sbom_full.name)
    else:
        artefact = require(cfg["artifact"], f"{kind} artefact")
        shutil.copy2(artefact, dest / artefact.name)
        sbom_dir = cfg["sbom_dir"]
        if sbom_dir.exists():
            for bom in sbom_dir.glob("*.json"):
                shutil.copy2(bom, dest / bom.name)
            for bom_xml in sbom_dir.glob("*.xml"):
                shutil.copy2(bom_xml, dest / bom_xml.name)
        else:
            sys.stderr.write(f"missing {kind} SBOM directory {sbom_dir}\n")
            sys.exit(1)

print(f"Wrote publish artefacts to {out_dir}")
PY
