#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/android_sbom_provenance.sh <sdk-version>

Generates CycloneDX SBOMs for the Android SDK + sample apps, signs each SBOM
with cosign (keyless), and writes checksums + bundles under
artifacts/android/sbom/<sdk-version>/.

Prerequisites:
  - cosign available on PATH (COSIGN environment variable overrides binary)
  - Java/Android toolchain configured for the workspace Gradle wrappers
  - Tests succeed (run via run_tests.sh + check_android_samples.sh)
USAGE
}

if [[ $# -lt 1 ]]; then
  usage >&2
  exit 1
fi

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [[ -z "${REPO_ROOT}" ]]; then
  echo "error: must run inside the repository" >&2
  exit 1
fi

SDK_VERSION=$1
DEST="${REPO_ROOT}/artifacts/android/sbom/${SDK_VERSION}"
SDK_GRADLE_WRAPPER="${REPO_ROOT}/examples/android/gradlew"
SDK_GRADLE_FALLBACK="${GRADLE:-gradle}"
SAMPLES_GRADLE_WRAPPER="${REPO_ROOT}/examples/android/gradlew"
COSIGN_BIN=${COSIGN:-cosign}

if [[ ! -x "${SDK_GRADLE_WRAPPER}" ]]; then
  if ! command -v "${SDK_GRADLE_FALLBACK}" >/dev/null 2>&1; then
    echo "error: expected Gradle wrapper at ${SDK_GRADLE_WRAPPER} or a gradle binary on PATH" >&2
    exit 1
  fi
  SDK_GRADLE_WRAPPER="${SDK_GRADLE_FALLBACK}"
fi

if [[ ! -x "${SAMPLES_GRADLE_WRAPPER}" ]]; then
  echo "error: expected Gradle wrapper at ${SAMPLES_GRADLE_WRAPPER}" >&2
  exit 1
fi

if ! command -v "${COSIGN_BIN}" >/dev/null 2>&1; then
  echo "error: cosign is required (set COSIGN env var to override binary)" >&2
  exit 1
fi

echo "==> Running Android SDK tests"
("${REPO_ROOT}/java/iroha_android/run_tests.sh")
("${REPO_ROOT}/scripts/check_android_samples.sh")

echo "==> Generating CycloneDX SBOMs"
(
  cd "${REPO_ROOT}"
  "${SDK_GRADLE_WRAPPER}" -p java/iroha_android \
    -PirohaAndroidVersion="${SDK_VERSION}" \
    :jvm:cyclonedxBom \
    :android:cyclonedxBom
  "${SAMPLES_GRADLE_WRAPPER}" \
    :operator-console:cyclonedxBom \
    :retail-wallet:cyclonedxBom \
    -PversionName="${SDK_VERSION}"
)

mkdir -p "${DEST}"

declare -A OUTPUT_MAP=(
  ["java/iroha_android/jvm"]="iroha-android-jvm.cyclonedx.json"
  ["java/iroha_android/android"]="iroha-android.cyclonedx.json"
  ["examples/android/operator-console"]="operator-console.cyclonedx.json"
  ["examples/android/retail-wallet"]="retail-wallet.cyclonedx.json"
)

echo "==> Collecting SBOM reports"
while IFS= read -r -d '' bom; do
  module_root=$(dirname "$(dirname "$(dirname "${bom}")")")
  rel_root=${module_root#"${REPO_ROOT}/"}
  filename=${OUTPUT_MAP[${rel_root}]:-$(basename "${bom}")}
  cp "${bom}" "${DEST}/${filename}"
done < <(find "${REPO_ROOT}/java/iroha_android/jvm" "${REPO_ROOT}/java/iroha_android/android" "${REPO_ROOT}/examples/android" \
            -path '*/build/reports/bom/*.json' -print0)

CHECKSUM_FILE="${DEST}/checksums.txt"
: > "${CHECKSUM_FILE}"

echo "==> Signing SBOMs with cosign"
for bom in "${DEST}"/*.json; do
  [[ -e "${bom}" ]] || continue
  "${COSIGN_BIN}" sign-blob --yes --bundle "${bom}.sigstore" "${bom}"
  shasum -a 256 "${bom}" >> "${CHECKSUM_FILE}"
done

echo "SBOMs + signatures stored under ${DEST}"
