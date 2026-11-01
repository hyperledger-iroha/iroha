#!/usr/bin/env bash
# Run paid-pin tests using csharp/global.json; the native bridge must be available.
# The optional dotnet override selects the pinned SDK executable.
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_CSHARP_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${SORAFS_PIN_REGISTER_DOTNET_BIN:-dotnet}"

export DOTNET_CLI_TELEMETRY_OPTOUT="${DOTNET_CLI_TELEMETRY_OPTOUT:-1}"
export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-${TMPDIR:-/tmp}/iroha-dotnet-home}"

cd "${ROOT_DIR}/csharp"
DOTNET_VERSION="$("${DOTNET_BIN}" --version)"
printf '%s\n' "${DOTNET_VERSION}"
case "${DOTNET_VERSION}" in
  10.0.*) ;;
  *)
    echo "error: SoraFS pin-register C# SDK tests require .NET SDK 10.0.x; got ${DOTNET_VERSION}" >&2
    exit 1
    ;;
esac

"${DOTNET_BIN}" test \
  --project tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
  -- \
  --filter-method "*RegisterSoraFsPinManifestAsync*"
