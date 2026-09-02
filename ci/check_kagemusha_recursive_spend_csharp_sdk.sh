#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
DOTNET_BIN="${KAGEMUSHA_RECURSIVE_SPEND_CSHARP_DOTNET_BIN:-dotnet}"
ARTIFACTS="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-csharp.XXXXXX")"
trap 'rm -rf "${ARTIFACTS}"' EXIT

version="$(
  cd "${ROOT_DIR}/csharp"
  "${DOTNET_BIN}" --version
)"
if [[ ! "${version}" =~ ^8\. ]]; then
  echo "error: C# SDK checks require .NET 8; got ${version}" >&2
  exit 1
fi

(
  cd "${ROOT_DIR}/csharp"
  # The full C# workflow stages the native bridge; this DTO lane intentionally does not.
  "${DOTNET_BIN}" test \
    tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    --artifacts-path "${ARTIFACTS}" \
    -p:ProduceReferenceAssembly=false \
    -- \
    --filter-class \
      Hyperledger.Iroha.Sdk.Tests.KagemushaToriiTests \
      Hyperledger.Iroha.Sdk.Tests.VerifyingKeyBackendTagTests \
      Hyperledger.Iroha.Sdk.Tests.ToriiClientTests \
      Hyperledger.Iroha.Sdk.Tests.TransactionBuilderTests \
    --filter-not-method \
      Hyperledger.Iroha.Sdk.Tests.ToriiClientTests.HijiriQuoteNativeBridgeEncodesFreesAndRejectsMalformedResponse
)

client="${ROOT_DIR}/csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiKagemushaClient.cs"
models="${ROOT_DIR}/csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiKagemushaModels.cs"
validator="${ROOT_DIR}/csharp/src/Hyperledger.Iroha.Sdk/Torii/ToriiKagemushaOperationStatusValidator.cs"
test -f "${client}"
test -f "${models}"
test -f "${validator}"

grep -Fq 'KagemushaRequiredBridgeAbiVersion = ToriiKagemushaTransport.BridgeAbiVersion' "${client}"
grep -Fq 'internal const int BridgeAbiVersion = 23;' "${models}"
grep -Fq 'internal const int ManifestVersion = 4;' "${models}"
grep -Fq 'internal const int MaxReadinessJsonResponseBytes = 4 * 1024;' "${models}"
grep -Fq 'internal const int MaxOperationReferenceJsonResponseBytes = 4 * 1024;' "${models}"
grep -Fq 'internal const int MaxOperationStatusJsonResponseBytes = 16 * 1024 * 1024;' "${models}"
grep -Fq 'internal const int MaxTopUpNoritoRequestBytes = 512 * 1024;' "${models}"
grep -Fq 'internal const int MaxRedeemNoritoRequestBytes = 48 * 1024 * 1024;' "${models}"
grep -Fq 'iroha.torii.v1.offline.top_up.request' "${models}"
grep -Fq 'iroha.torii.v1.offline.redeem.request' "${models}"
grep -Fq '"/v1/offline/readiness"' "${client}"
grep -Fq '"/v1/offline/top-up"' "${client}"
grep -Fq '"/v1/offline/redeem"' "${client}"
grep -Fq '"/v1/offline/operations/' "${client}"
grep -Fq 'RequireKagemushaRetryAfter(response);' "${client}"
grep -Fq 'result.anchor.topup_operation_id' "${client}"
grep -Fq 'result.finality_proof.anchor.topup_operation_id' "${client}"
grep -Fq 'internal const uint RequiredNativeContractRevision = 1;' "${validator}"
grep -Fq '"connect_norito_kagemusha_native_contract_revision";' "${validator}"
grep -Fq '"connect_norito_kagemusha_offline_operation_status_json_validate_v2";' "${validator}"
grep -Fq 'bridgeAbiVersion == RequiredBridgeAbiVersion' "${validator}"
grep -Fq 'nativeContractRevision == RequiredNativeContractRevision' "${validator}"

python3 - "${ROOT_DIR}/csharp/src" "${validator}" <<'PY'
import pathlib
import re
import sys

source_root = pathlib.Path(sys.argv[1]).resolve()
validator_path = pathlib.Path(sys.argv[2]).resolve()
allowed_kagemusha_imports = {
    "connect_norito_kagemusha_native_contract_revision",
    "connect_norito_kagemusha_offline_operation_status_json_validate_v2",
}
type_pattern = re.compile(
    r"\b(?:class|record(?:\s+(?:class|struct))?|interface)\s+"
    r"[^\s<{;]*Kagemusha[^\s<{;]*Prover\b",
    re.IGNORECASE,
)
constant_pattern = re.compile(
    r"\bconst\s+string\s+(?P<name>[A-Za-z_]\w*)\s*=\s*"
    r'"(?P<value>[^"]+)"\s*;',
)
import_pattern = re.compile(
    r"\[(?:DllImport|LibraryImport)(?:Attribute)?\s*\("
    r"(?P<arguments>.*?)\)\s*\]\s*(?P<declaration>[^;]+;)",
    re.IGNORECASE | re.DOTALL,
)
entry_point_pattern = re.compile(
    r"\bEntryPoint\s*=\s*(?P<value>\"[^\"]+\"|[A-Za-z_]\w*)",
)
method_pattern = re.compile(
    r"\b(?:extern|partial)\s+[A-Za-z_]\w*(?:<[^;()]+>)?(?:\[\])?\s+"
    r"(?P<name>[A-Za-z_]\w*)\s*\(",
    re.DOTALL,
)

errors: list[str] = []
allowed_import_counts = {symbol: 0 for symbol in allowed_kagemusha_imports}
for path in source_root.rglob("*.cs"):
    if any(part in {"bin", "obj"} for part in path.parts):
        continue
    source = path.read_text(encoding="utf-8")
    if match := type_pattern.search(source):
        errors.append(f"{path}: native prover type claim `{match.group(0)}`")

    constants = {
        match.group("name"): match.group("value")
        for match in constant_pattern.finditer(source)
    }
    for native_import in import_pattern.finditer(source):
        arguments = native_import.group("arguments")
        declaration = native_import.group("declaration")
        entry_match = entry_point_pattern.search(arguments)
        method_match = method_pattern.search(declaration)
        entry_point = None
        if entry_match:
            token = entry_match.group("value")
            entry_point = token[1:-1] if token.startswith('"') else constants.get(token)
        method_name = method_match.group("name") if method_match else ""
        mentions_kagemusha = "kagemusha" in " ".join(
            (arguments, declaration, entry_point or "")
        ).lower()
        if not mentions_kagemusha:
            continue
        if path.resolve() != validator_path or entry_point not in allowed_kagemusha_imports:
            shown = entry_point or method_name or "unresolved native import"
            errors.append(f"{path}: forbidden Kagemusha native import `{shown}`")
            continue
        allowed_import_counts[entry_point] += 1

expected_counts = {
    "connect_norito_kagemusha_native_contract_revision": 1,
    "connect_norito_kagemusha_offline_operation_status_json_validate_v2": 2,
}
for symbol, expected in expected_counts.items():
    actual = allowed_import_counts[symbol]
    if actual != expected:
        errors.append(
            f"{validator_path}: expected {expected} imports of `{symbol}`, found {actual}"
        )

if errors:
    print(
        "error: C# must remain a Torii DTO/transport client and may import only "
        "the pinned Kagemusha native validators:",
        file=sys.stderr,
    )
    for error in errors:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(1)
PY

echo "Kagemusha C# boundary passed: ABI-23/V4 Torii DTOs pin native contract revision 1 and the V2 status validator without a native prover claim."
