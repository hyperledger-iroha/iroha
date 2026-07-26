#!/usr/bin/env bash

# Publishes the Android SDK to a Maven repository, captures SBOM + dependency
# evidence, and optionally signs artefacts with Sigstore.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="${ROOT_DIR}/java/iroha_android"
GRADLE_WRAPPER="${ROOT_DIR}/examples/android/gradlew"
GRADLE_FALLBACK="${GRADLE:-gradle}"

JVM_ARTIFACT_ID="iroha-android"
ANDROID_ARTIFACT_ID="iroha-android"

VERSION="${ANDROID_PUBLISH_VERSION:-0.1.0-SNAPSHOT}"
REPO_DIR_DEFAULT="${ROOT_DIR}/artifacts/android/maven"
REPORT_DIR_BASE_DEFAULT="${ROOT_DIR}/artifacts/android/reports"
REPORT_DIR_BASE="${ANDROID_PUBLISH_REPORT_DIR:-$REPORT_DIR_BASE_DEFAULT}"
REPORT_DIR="${REPORT_DIR_BASE%/}/${VERSION}"
REPO_DIR="${ANDROID_PUBLISH_REPO_DIR:-$REPO_DIR_DEFAULT}"
REPO_URL="${ANDROID_PUBLISH_REPO_URL:-}"
REPO_USERNAME="${ANDROID_PUBLISH_REPO_USERNAME:-}"
REPO_PASSWORD="${ANDROID_PUBLISH_REPO_PASSWORD:-}"

SIGN_ARTIFACTS="${ANDROID_PUBLISH_SIGN:-0}"
COSIGN_BIN_DEFAULT="${COSIGN:-cosign}"
COSIGN_BIN="${ANDROID_PUBLISH_COSIGN_BIN:-$COSIGN_BIN_DEFAULT}"
SIGSTORE_TOKEN_ENV="${ANDROID_PUBLISH_SIGSTORE_TOKEN_ENV:-SIGSTORE_ID_TOKEN}"
SIGSTORE_TOKEN_VALUE="${!SIGSTORE_TOKEN_ENV-}"
SIG_DIR="${REPORT_DIR}/signatures"
CHECKSUM_FILE="${REPORT_DIR}/checksums.txt"
SUMMARY_FILE="${REPORT_DIR}/publish_summary.json"

REPORT_FILES=()
PUBLISH_SOURCES_JVM=()
PUBLISH_SOURCES_ANDROID=()
RECORD_LINES=()

hash_file() {
    local path="$1"
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$path" | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    else
        python3 - "$path" <<'PY'
import hashlib, sys
path = sys.argv[1]
hasher = hashlib.sha256()
with open(path, "rb") as fp:
    while True:
        chunk = fp.read(8192)
        if not chunk:
            break
        hasher.update(chunk)
print(hasher.hexdigest())
PY
    fi
}

sign_file() {
    local path="$1"
    local bundle="$2"
    mkdir -p "$(dirname "$bundle")"
    local args=(sign-blob --yes --bundle "$bundle" "$path")
    if [[ -n "$SIGSTORE_TOKEN_VALUE" ]]; then
        args+=(--identity-token "$SIGSTORE_TOKEN_VALUE")
    fi
    COSIGN_EXPERIMENTAL="${COSIGN_EXPERIMENTAL:-1}" "$COSIGN_BIN" "${args[@]}"
}

record_file() {
    local path="$1"
    local category="$2" # publish | report
    [[ -f "$path" ]] || return

    local rel="$path"
    if [[ "$path" == "$ROOT_DIR/"* ]]; then
        rel="${path#$ROOT_DIR/}"
    fi

    local sha
    sha="$(hash_file "$path")"
    local sig_rel=""

    if [[ "$SIGN_ARTIFACTS" == "1" ]]; then
        if ! command -v "$COSIGN_BIN" >/dev/null 2>&1; then
            echo "error: cosign binary '${COSIGN_BIN}' not found (set COSIGN or ANDROID_PUBLISH_COSIGN_BIN)" >&2
            exit 1
        fi
        local bundle="${SIG_DIR}/$(basename "$path").sigstore"
        sign_file "$path" "$bundle"
        if [[ "$bundle" == "$ROOT_DIR/"* ]]; then
            sig_rel="${bundle#$ROOT_DIR/}"
        else
            sig_rel="$bundle"
        fi
    fi

    echo "${sha}  ${rel}" >> "$CHECKSUM_FILE"
    RECORD_LINES+=("${rel}|${sha}|${sig_rel}|${category}")
}

copy_required() {
    local src="$1"
    local dest="$2"
    local label="$3"
    if [[ ! -f "$src" ]]; then
        echo "error: missing ${label} at ${src}" >&2
        exit 1
    fi
    mkdir -p "$(dirname "$dest")"
    cp "$src" "$dest"
    REPORT_FILES+=("$dest")
}

copy_bom() {
    local src_dir="$1"
    local dest_dir="$2"
    local base_name="$3"
    local label="$4"

    local json_src=""
    for candidate in "${src_dir}/${base_name}.json" "${src_dir}/bom/${base_name}.json" "${src_dir}/bom.json" "${src_dir}/bom/bom.json"; do
        if [[ -f "$candidate" ]]; then
            json_src="$candidate"
            break
        fi
    done
    if [[ -z "$json_src" ]]; then
        json_src="$(find "$src_dir" -maxdepth 3 -name "*.json" | head -n1)"
    fi
    if [[ -z "$json_src" || ! -f "$json_src" ]]; then
        echo "error: missing ${label} CycloneDX JSON SBOM under ${src_dir}" >&2
        exit 1
    fi

    local xml_src=""
    for candidate in "${src_dir}/${base_name}.xml" "${src_dir}/bom/${base_name}.xml" "${src_dir}/bom.xml" "${src_dir}/bom/bom.xml"; do
        if [[ -f "$candidate" ]]; then
            xml_src="$candidate"
            break
        fi
    done
    if [[ -z "$xml_src" ]]; then
        xml_src="$(find "$src_dir" -maxdepth 3 -name "*.xml" | head -n1)"
    fi
    if [[ -z "$xml_src" || ! -f "$xml_src" ]]; then
        echo "error: missing ${label} CycloneDX XML SBOM under ${src_dir}" >&2
        exit 1
    fi

    mkdir -p "$dest_dir"
    copy_required "$json_src" "${dest_dir}/${label}.json" "${label} JSON SBOM"
    copy_required "$xml_src" "${dest_dir}/${label}.xml" "${label} XML SBOM"
}

mkdir -p "$REPO_DIR" "$REPORT_DIR"
: > "$CHECKSUM_FILE"

GRADLE_ARGS=(
    "-PirohaAndroidVersion=${VERSION}"
)

if [[ -n "$REPO_URL" ]]; then
    GRADLE_ARGS+=("-PirohaAndroidRepoUrl=${REPO_URL}")
else
    GRADLE_ARGS+=("-PirohaAndroidRepoDir=${REPO_DIR}")
fi

if [[ -n "$REPO_USERNAME" ]]; then
    GRADLE_ARGS+=("-PirohaAndroidRepoUsername=${REPO_USERNAME}")
fi

if [[ -n "$REPO_PASSWORD" ]]; then
    GRADLE_ARGS+=("-PirohaAndroidRepoPassword=${REPO_PASSWORD}")
fi

GRADLE_BIN="$GRADLE_WRAPPER"
if [[ ! -x "$GRADLE_BIN" ]]; then
    GRADLE_BIN="$GRADLE_FALLBACK"
fi

echo "==> Publishing Iroha Android SDK version ${VERSION}"
"$GRADLE_BIN" -p "$ANDROID_DIR" publish "${GRADLE_ARGS[@]}"

if [[ "${ANDROID_PUBLISH_SKIP_SAMPLE:-0}" != "1" ]]; then
    if [[ -n "$REPO_URL" ]]; then
        echo "==> Skipping sample app build because ANDROID_PUBLISH_REPO_URL is set (no local repo to resolve)"
    else
        echo "==> Building samples-android against the published AAR"
        "$GRADLE_BIN" -p "$ANDROID_DIR" :samples-android:assembleDebug \
            -PirohaAndroidSampleUsePublished=true \
            -PirohaAndroidSampleRepoDir="${REPO_DIR}" \
            -PirohaAndroidVersion="${VERSION}"
    fi
fi

JVM_REPORT_SRC="${ANDROID_DIR}/jvm/build/reports"
ANDROID_REPORT_SRC="${ANDROID_DIR}/android/build/reports"
REPORT_JVM_DIR="${REPORT_DIR}/jvm"
REPORT_ANDROID_DIR="${REPORT_DIR}/android"

JVM_MANIFEST_NAME="${JVM_ARTIFACT_ID}-${VERSION}-runtimeClasspath.json"
JVM_RUNTIME_MANIFEST_NAME="${JVM_ARTIFACT_ID}-jvm-runtime-manifest.json"
JVM_RUNTIME_CHECKSUM_NAME="${JVM_ARTIFACT_ID}-jvm-${VERSION}-runtime.sha256"
ANDROID_MANIFEST_NAME="${ANDROID_ARTIFACT_ID}-${VERSION}-releaseRuntimeClasspath.json"
JVM_MANIFEST_SRC="${JVM_REPORT_SRC}/publishing/${JVM_MANIFEST_NAME}"
JVM_RUNTIME_MANIFEST_SRC="${JVM_REPORT_SRC}/publishing/${JVM_RUNTIME_MANIFEST_NAME}"
JVM_RUNTIME_CHECKSUM_SRC="${JVM_REPORT_SRC}/publishing/${JVM_RUNTIME_CHECKSUM_NAME}"
ANDROID_MANIFEST_SRC="${ANDROID_REPORT_SRC}/publishing/${ANDROID_MANIFEST_NAME}"
ANDROID_RUNTIME_MANIFEST_SRC="${ANDROID_REPORT_SRC}/publishing/${ANDROID_ARTIFACT_ID}-runtime-manifest.json"
ANDROID_RUNTIME_CHECKSUM_SRC="${ANDROID_REPORT_SRC}/publishing/${ANDROID_ARTIFACT_ID}-${VERSION}-runtime.sha256"

copy_required "$JVM_MANIFEST_SRC" "${REPORT_JVM_DIR}/${JVM_MANIFEST_NAME}" "JVM dependency manifest"
copy_required "$ANDROID_MANIFEST_SRC" "${REPORT_ANDROID_DIR}/${ANDROID_MANIFEST_NAME}" "Android dependency manifest"
copy_required "$JVM_RUNTIME_MANIFEST_SRC" "${REPORT_JVM_DIR}/$(basename "$JVM_RUNTIME_MANIFEST_SRC")" "JVM runtime manifest"
copy_required "$JVM_RUNTIME_CHECKSUM_SRC" "${REPORT_JVM_DIR}/$(basename "$JVM_RUNTIME_CHECKSUM_SRC")" "JVM runtime checksum"
copy_required "$ANDROID_RUNTIME_MANIFEST_SRC" "${REPORT_ANDROID_DIR}/$(basename "$ANDROID_RUNTIME_MANIFEST_SRC")" "Android runtime manifest"
copy_required "$ANDROID_RUNTIME_CHECKSUM_SRC" "${REPORT_ANDROID_DIR}/$(basename "$ANDROID_RUNTIME_CHECKSUM_SRC")" "Android runtime checksum"

copy_bom "$JVM_REPORT_SRC" "$REPORT_JVM_DIR" "bom" "bom-jvm"
copy_bom "$ANDROID_REPORT_SRC" "$REPORT_ANDROID_DIR" "bom" "bom-android"

JVM_COORD_DIR="${REPO_DIR}/org/hyperledger/iroha/${JVM_ARTIFACT_ID}/${VERSION}"
if [[ -d "$JVM_COORD_DIR" ]]; then
    for candidate in "${JVM_COORD_DIR}"/${JVM_ARTIFACT_ID}-"${VERSION}"{.jar,-sources.jar,-javadoc.jar,.pom,.module}; do
        [[ -f "$candidate" ]] && PUBLISH_SOURCES_JVM+=("$candidate")
    done
fi

ANDROID_COORD_DIR="${REPO_DIR}/org/hyperledger/iroha/${ANDROID_ARTIFACT_ID}/${VERSION}"
if [[ -d "$ANDROID_COORD_DIR" ]]; then
    for candidate in "${ANDROID_COORD_DIR}"/${ANDROID_ARTIFACT_ID}-"${VERSION}"{.aar,-sources.jar,.pom,.module}; do
        [[ -f "$candidate" ]] && PUBLISH_SOURCES_ANDROID+=("$candidate")
    done
fi

if [[ ${#PUBLISH_SOURCES_JVM[@]} -eq 0 ]]; then
    JVM_LIB_DIR="${ANDROID_DIR}/jvm/build/libs"
    JVM_PUB_DIR="${ANDROID_DIR}/jvm/build/publications/androidSdkJvm"
    for candidate in \
        "${JVM_LIB_DIR}/${JVM_ARTIFACT_ID}-${VERSION}.jar" \
        "${JVM_LIB_DIR}/${JVM_ARTIFACT_ID}-${VERSION}-sources.jar" \
        "${JVM_LIB_DIR}/${JVM_ARTIFACT_ID}-${VERSION}-javadoc.jar" \
        "${JVM_PUB_DIR}/pom-default.xml" \
        "${JVM_PUB_DIR}/module.json"
    do
        [[ -f "$candidate" ]] && PUBLISH_SOURCES_JVM+=("$candidate")
    done
fi

if [[ ${#PUBLISH_SOURCES_ANDROID[@]} -eq 0 ]]; then
    AND_LIB_DIR="${ANDROID_DIR}/android/build/outputs/aar"
    AND_PUB_DIR="${ANDROID_DIR}/android/build/publications/androidSdk"
    for candidate in \
        "${AND_LIB_DIR}/${ANDROID_ARTIFACT_ID}-${VERSION}.aar" \
        "${AND_LIB_DIR}/${ANDROID_ARTIFACT_ID}-${VERSION}-sources.jar" \
        "${AND_LIB_DIR}/android-release.aar" \
        "${AND_LIB_DIR}/android-release-sources.jar" \
        "${AND_PUB_DIR}/pom-default.xml" \
        "${AND_PUB_DIR}/module.json"
    do
        [[ -f "$candidate" ]] && PUBLISH_SOURCES_ANDROID+=("$candidate")
    done
fi

PUBLISHED_COPY_DIR="${REPORT_DIR}/artifacts"
PUBLISHED_COPY_DIR_JVM="${PUBLISHED_COPY_DIR}/jvm"
PUBLISHED_COPY_DIR_ANDROID="${PUBLISHED_COPY_DIR}/android"
mkdir -p "$PUBLISHED_COPY_DIR_JVM" "$PUBLISHED_COPY_DIR_ANDROID"

for path in "${REPORT_FILES[@]}"; do
    record_file "$path" "report"
done

if [[ ${#PUBLISH_SOURCES_JVM[@]} -eq 0 ]]; then
    echo "warning: no JVM published artefacts were located; skipping JVM publish evidence copy" >&2
else
    for src in "${PUBLISH_SOURCES_JVM[@]}"; do
        dest="${PUBLISHED_COPY_DIR_JVM}/$(basename "$src")"
        cp "$src" "$dest"
        record_file "$dest" "publish"
    done
fi

if [[ ${#PUBLISH_SOURCES_ANDROID[@]} -eq 0 ]]; then
    echo "warning: no Android published artefacts were located; skipping Android publish evidence copy" >&2
else
    for src in "${PUBLISH_SOURCES_ANDROID[@]}"; do
        dest="${PUBLISHED_COPY_DIR_ANDROID}/$(basename "$src")"
        cp "$src" "$dest"
        record_file "$dest" "publish"
    done
fi

RECORD_ENV="$(printf '%s\n' "${RECORD_LINES[@]}")"
export ROOT_DIR VERSION REPO_URL REPO_DIR RECORD_ENV SUMMARY_FILE
python3 <<'PY' > "${SUMMARY_FILE}"
import json
import os
from datetime import datetime, timezone

def parse_records(raw: str):
    publish, reports = [], []
    for line in raw.splitlines():
        if not line.strip():
            continue
        parts = line.split("|")
        if len(parts) != 4:
            continue
        path, sha, sig, category = parts
        entry = {"path": path, "sha256": sha}
        if sig:
            entry["sigstore_bundle"] = sig
        (publish if category == "publish" else reports).append(entry)
    return publish, reports

publish, reports = parse_records(os.environ.get("RECORD_ENV", ""))
summary = {
    "version": os.environ["VERSION"],
    "generated_at": datetime.now(tz=timezone.utc).isoformat(),
    "repository_target": os.environ.get("REPO_URL") or os.environ.get("REPO_DIR"),
    "publish_artifacts": publish,
    "report_artifacts": reports,
}
with open(os.environ["SUMMARY_FILE"], "w", encoding="utf-8") as fp:
    json.dump(summary, fp, indent=2)
PY

cat > "${REPORT_DIR}/README.md" <<EOF
# Android SDK publishing artefacts

- Version: ${VERSION}
- Generated: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- Repository destination: ${REPO_URL:-$REPO_DIR}
- Reports source: JVM ${JVM_REPORT_SRC}; Android ${ANDROID_REPORT_SRC}
- Signing: $( [[ "$SIGN_ARTIFACTS" == "1" ]] && echo "Sigstore bundles under signatures/" || echo "disabled" )

Artifacts captured in this directory:
- Published artefacts copy (\`artifacts/jvm/\`, \`artifacts/android/\`)
- CycloneDX SBOMs (JSON + XML under \`jvm/\` and \`android/\`)
- Runtime dependency manifests (\`jvm/${JVM_MANIFEST_NAME}\`, \`android/${ANDROID_MANIFEST_NAME}\`)
- Android runtime manifest/checksum (\`android/$(basename "$ANDROID_RUNTIME_MANIFEST_SRC")\`, \`android/$(basename "$ANDROID_RUNTIME_CHECKSUM_SRC")\` when present)
- Checksums (\`checksums.txt\`)
- Evidence summary (\`publish_summary.json\`)
$( [[ "$SIGN_ARTIFACTS" == "1" ]] && echo "- Sigstore bundles (\`signatures/\`)" )
EOF

echo "==> Reports copied to ${REPORT_DIR}"
