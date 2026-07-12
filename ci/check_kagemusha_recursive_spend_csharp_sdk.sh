#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_DOTNET_BIN:-dotnet}"
ARTIFACTS="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-csharp.XXXXXX")"
trap 'rm -rf "${ARTIFACTS}"' EXIT

version="$(${DOTNET_BIN} --version)"
if [[ ! "${version}" =~ ^8\. ]]; then
  echo "error: C# SDK checks require .NET 8; got ${version}" >&2
  exit 1
fi

"${DOTNET_BIN}" test \
  "${ROOT_DIR}/csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj" \
  --artifacts-path "${ARTIFACTS}" \
  --filter "FullyQualifiedName~VerifyingKeyBackendTagTests|FullyQualifiedName~ToriiClientTests|FullyQualifiedName~TransactionBuilderTests" \
  -p:ProduceReferenceAssembly=false \
  --logger "console;verbosity=minimal"

if find "${ROOT_DIR}/csharp/src" -type f -path '*/Offline/*' -print -quit | grep -q .; then
  echo "error: C# publishes an offline lifecycle; Swift is the sole lifecycle SDK" >&2
  exit 1
fi

echo "Kagemusha C# boundary passed: no offline lifecycle is published."
