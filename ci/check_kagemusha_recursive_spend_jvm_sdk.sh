#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_JVM_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
JAVA_OUT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-java-artifacts.XXXXXX")"
trap 'rm -rf "${JAVA_OUT}"' EXIT

java_version="$(java -version 2>&1 | head -n 1)"
if [[ ! "${java_version}" =~ version[[:space:]]+\"21([.\"]|$) ]]; then
  echo "error: Kagemusha JVM artifact checks require JDK 21; got ${java_version}" >&2
  exit 1
fi

cd "${ROOT_DIR}/kotlin"
./gradlew --no-daemon --max-workers=1 -q :core-jvm:test \
  --tests org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProverTest

cd "${ROOT_DIR}"
javac \
  -sourcepath "java/iroha_android/src/main/java:java/iroha_android/src/test/java" \
  -d "${JAVA_OUT}" \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java
java -ea -cp "${JAVA_OUT}" \
  org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest

python3 - "${ROOT_DIR}" <<'PY'
from pathlib import Path
import sys

root = Path(sys.argv[1])
allowed = {
    root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
}
actual = {
    path
    for package in (
        root / "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline",
        root / "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline",
    )
    if package.exists()
    for path in package.iterdir()
    if path.is_file()
}
unexpected = sorted(str(path.relative_to(root)) for path in actual - allowed)
if unexpected:
    raise SystemExit("unexpected JVM offline lifecycle files: " + ", ".join(unexpected))
PY

echo "Kagemusha JVM boundary passed: ABI-19 artifact streaming only."
