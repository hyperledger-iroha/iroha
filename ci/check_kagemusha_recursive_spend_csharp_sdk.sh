#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${KAGEMUSHA_RECURSIVE_SPEND_DOTNET_BIN:-dotnet}"
BRIDGE_TARGET_DIR="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_BRIDGE_TARGET_DIR:-${TMPDIR:-/tmp}/iroha-kagemusha-csharp-native-target}"
export DOTNET_CLI_TELEMETRY_OPTOUT="${DOTNET_CLI_TELEMETRY_OPTOUT:-1}"
export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-${TMPDIR:-/tmp}/iroha-dotnet-home}"

cd "${ROOT_DIR}"
if ! command -v "${DOTNET_BIN}" >/dev/null 2>&1; then
  echo "error: Kagemusha recursive spend C# SDK tests require .NET SDK 8.0.x; '${DOTNET_BIN}' was not found" >&2
  exit 1
fi
DOTNET_VERSION="$("${DOTNET_BIN}" --version)"
printf '%s\n' "${DOTNET_VERSION}"
case "${DOTNET_VERSION}" in
  8.0.*) ;;
  *)
    echo "error: Kagemusha recursive spend C# SDK tests require .NET SDK 8.0.x; got ${DOTNET_VERSION}" >&2
    exit 1
    ;;
esac
printf 'dotnet --info:\n'
"${DOTNET_BIN}" --info

CARGO_TARGET_DIR="${BRIDGE_TARGET_DIR}" cargo build -p connect_norito_bridge
BRIDGE_LIBRARY_DIR="${BRIDGE_TARGET_DIR}/debug"
case "$(uname -s)" in
  Darwin)
    BRIDGE_LIBRARY_NAME="libconnect_norito_bridge.dylib"
    ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    BRIDGE_LIBRARY_NAME="connect_norito_bridge.dll"
    ;;
  *)
    BRIDGE_LIBRARY_NAME="libconnect_norito_bridge.so"
    ;;
esac
BRIDGE_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}/${BRIDGE_LIBRARY_NAME}"
if [[ ! -f "${BRIDGE_LIBRARY_PATH}" ]]; then
  echo "error: freshly built connect_norito_bridge native library was not found at ${BRIDGE_LIBRARY_PATH}" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  BRIDGE_LIBRARY_SHA256="$(sha256sum "${BRIDGE_LIBRARY_PATH}" | cut -d ' ' -f 1)"
elif command -v shasum >/dev/null 2>&1; then
  BRIDGE_LIBRARY_SHA256="$(shasum -a 256 "${BRIDGE_LIBRARY_PATH}" | cut -d ' ' -f 1)"
else
  echo "error: sha256sum or shasum is required to record the native bridge digest" >&2
  exit 1
fi
if [[ ! "${BRIDGE_LIBRARY_SHA256}" =~ ^[0-9a-fA-F]{64}$ ]]; then
  echo "error: failed to compute a valid SHA-256 for ${BRIDGE_LIBRARY_PATH}" >&2
  exit 1
fi
printf 'connect_norito_bridge native bridge: %s\n' "${BRIDGE_LIBRARY_PATH}"
printf 'connect_norito_bridge native bridge sha256: %s\n' "${BRIDGE_LIBRARY_SHA256}"
export DYLD_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
export LD_LIBRARY_PATH="${BRIDGE_LIBRARY_DIR}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export PATH="${BRIDGE_LIBRARY_DIR}:${PATH}"

"${DOTNET_BIN}" test \
  csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
  --filter "FullyQualifiedName~KagemushaRecursiveSpendNativeTests|FullyQualifiedName~PrivacyNativeTests|FullyQualifiedName~TransactionBuilderTests|FullyQualifiedName~CanonicalRequestTests|FullyQualifiedName~ToriiClientTests|FullyQualifiedName~SignedQueryBuilderTests|FullyQualifiedName~SignedIterableQueryBuilderTests|FullyQualifiedName~VerifyingKeyBackendTagTests" \
  --logger "console;verbosity=minimal"
