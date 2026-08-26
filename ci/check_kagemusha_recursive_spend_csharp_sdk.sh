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
  -p:ProduceReferenceAssembly=false \
  -- \
  --filter-class \
    Hyperledger.Iroha.Sdk.Tests.KagemushaToriiTests \
    Hyperledger.Iroha.Sdk.Tests.VerifyingKeyBackendTagTests \
    Hyperledger.Iroha.Sdk.Tests.ToriiClientTests \
    Hyperledger.Iroha.Sdk.Tests.TransactionBuilderTests

client="${ROOT_DIR}/csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiKagemushaClient.cs"
models="${ROOT_DIR}/csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiKagemushaModels.cs"
test -f "${client}"
test -f "${models}"

grep -Fq 'KagemushaRequiredBridgeAbiVersion = ToriiKagemushaTransport.BridgeAbiVersion' "${client}"
grep -Fq 'internal const int BridgeAbiVersion = 23;' "${models}"
grep -Fq 'internal const int ManifestVersion = 4;' "${models}"
grep -Fq 'internal const int MaxTopUpNoritoRequestBytes = 512 * 1024;' "${models}"
grep -Fq 'internal const int MaxRedeemNoritoRequestBytes = 48 * 1024 * 1024;' "${models}"
grep -Fq '"/v1/offline/readiness"' "${client}"
grep -Fq '"/v1/offline/top-up"' "${client}"
grep -Fq '"/v1/offline/redeem"' "${client}"
grep -Fq '"/v1/offline/operations/' "${client}"

if grep -rEni --include='*.cs' --exclude-dir=bin --exclude-dir=obj \
  '(class|record|interface)[[:space:]]+[^[:space:]]*Kagemusha[^[:space:]]*Prover|DllImport.*Kagemusha|LibraryImport.*Kagemusha' \
  "${ROOT_DIR}/csharp/src"; then
  echo "error: C# must remain a Torii DTO/transport client and must not claim a Kagemusha native prover" >&2
  exit 1
fi

echo "Kagemusha C# boundary passed: ABI-21/V4 Torii DTOs are present without a native prover claim."
