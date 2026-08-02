#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_CSHARP_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${PRIVACY_CSHARP_DOTNET_BIN:-dotnet}"

export DOTNET_CLI_TELEMETRY_OPTOUT="${DOTNET_CLI_TELEMETRY_OPTOUT:-1}"
export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-${TMPDIR:-/tmp}/iroha-dotnet-home}"

cd "${ROOT_DIR}"
DOTNET_VERSION="$("${DOTNET_BIN}" --version)"
printf '%s\n' "${DOTNET_VERSION}"
if [[ ! "${DOTNET_VERSION}" =~ ^8\.0\.[1-9][0-9]*$ ]]; then
  echo "error: privacy C# SDK tests require a stable canonical .NET SDK 8.0.x with a non-zero patch; got ${DOTNET_VERSION}" >&2
  exit 1
fi

PRIVACY_DOTNET_BIN_PATH="$(command -v "${DOTNET_BIN}")"
PRIVACY_DOTNET_ROOT_CANDIDATE="$(
  cd "$(dirname "${PRIVACY_DOTNET_BIN_PATH}")" && pwd -P
)"
if [[ -z "${DOTNET_ROOT:-}" && -d "${PRIVACY_DOTNET_ROOT_CANDIDATE}/host/fxr" ]]; then
  export DOTNET_ROOT="${PRIVACY_DOTNET_ROOT_CANDIDATE}"
fi

for test_class in \
  "Hyperledger.Iroha.Sdk.Tests.PrivacyNativeTests" \
  "Hyperledger.Iroha.Sdk.Tests.PrivacyExact12FixtureCodecV1Tests" \
  "Hyperledger.Iroha.Sdk.Tests.VerifyingKeyBackendTagTests"
do
  "${DOTNET_BIN}" test \
    csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    -- \
    --filter-class "${test_class}"
done
