#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_CSHARP_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${PRIVACY_CSHARP_DOTNET_BIN:-dotnet}"

export DOTNET_CLI_TELEMETRY_OPTOUT="${DOTNET_CLI_TELEMETRY_OPTOUT:-1}"
export DOTNET_CLI_HOME="${DOTNET_CLI_HOME:-${TMPDIR:-/tmp}/iroha-dotnet-home}"

cd "${ROOT_DIR}"
DOTNET_VERSION="$("${DOTNET_BIN}" --version)"
printf '%s\n' "${DOTNET_VERSION}"
case "${DOTNET_VERSION}" in
  8.0.*) ;;
  *)
    echo "error: privacy C# SDK tests require .NET SDK 8.0.x; got ${DOTNET_VERSION}" >&2
    exit 1
    ;;
esac

for filter in \
  "FullyQualifiedName~PrivacyNativeTests" \
  "FullyQualifiedName~VerifyingKeyBackendTagTests"
do
  "${DOTNET_BIN}" test \
    csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    --filter "${filter}" \
    --logger "console;verbosity=minimal"
done
