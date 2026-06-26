#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_FFI="${ROOT_DIR}/crates/sorafs_manifest/src/reference_ffi.rs"
HEADER="${ROOT_DIR}/crates/sorafs_manifest/include/sorafs_reference.h"
MODE="${1:-}"

usage() {
  cat >&2 <<'EOF'
usage: ci/check_sorafs_reference_ffi_header.sh [negative-control]

negative-control:
  --negative-control-missing-header
  --negative-control-bad-signature
  --negative-control-constant-drift
  --negative-control-missing-rust-export
EOF
}

run_contract_check() {
  local rust_ffi="${1}"
  local header="${2}"

  python3 - "$rust_ffi" "$header" <<'PY'
import re
import sys
from pathlib import Path

rust_ffi = Path(sys.argv[1])
header = Path(sys.argv[2])

rust_text = rust_ffi.read_text(encoding="utf-8")
header_text = header.read_text(encoding="utf-8")

rust_export_pattern = re.compile(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
    r'(sorafs_reference_[a-z0-9_]+)\s*\('
)
header_declaration_pattern = re.compile(
    r'(?:SorafsReferenceFfiBuffer|void)\s+'
    r'(sorafs_reference_[a-z0-9_]+)\s*\('
)
rust_const_pattern = re.compile(
    r'pub\s+const\s+(SORAFS_REFERENCE_[A-Z0-9_]+)\s*:\s*u32\s*=\s*([0-9]+)\s*;'
)
header_const_pattern = re.compile(
    r'#define\s+(SORAFS_REFERENCE_[A-Z0-9_]+)\s+([0-9]+)\b'
)

rust_exports = set(rust_export_pattern.findall(rust_text))
header_declarations = set(header_declaration_pattern.findall(header_text))
rust_constants = dict(rust_const_pattern.findall(rust_text))
header_constants = dict(header_const_pattern.findall(header_text))

def ptr(name, const=True):
    prefix = r"const\s+" if const else ""
    return rf"{prefix}uint8_t\s*\*\s*{name}"

def usize(name):
    return rf"size_t\s+{name}"

def u32(name):
    return rf"uint32_t\s+{name}"

def u64(name):
    return rf"uint64_t\s+{name}"

def buffer_signature(name, params):
    return (
        rf"SorafsReferenceFfiBuffer\s+{name}\s*\(\s*"
        + r"\s*,\s*".join(params)
        + r"\s*\)\s*;"
    )

def bytes_label_signature(name, extra_prefix=(), extra_suffix=()):
    return buffer_signature(
        name,
        [
            *extra_prefix,
            ptr("bytes_ptr"),
            usize("bytes_len"),
            ptr("label_ptr"),
            usize("label_len"),
            *extra_suffix,
            u64("generated_at"),
        ],
    )

def pair_signature(name, left, right):
    return buffer_signature(
        name,
        [
            ptr(f"{left}_ptr"),
            usize(f"{left}_len"),
            ptr(f"{left}_label_ptr"),
            usize(f"{left}_label_len"),
            ptr(f"{right}_ptr"),
            usize(f"{right}_len"),
            ptr(f"{right}_label_ptr"),
            usize(f"{right}_label_len"),
            u64("generated_at"),
        ],
    )

expected_signatures = {
    "sorafs_reference_free_buffer": (
        r"void\s+sorafs_reference_free_buffer\s*\(\s*"
        r"SorafsReferenceFfiBuffer\s+buffer\s*"
        r"\)\s*;"
    ),
    "sorafs_reference_validate_provider_advert_json": bytes_label_signature(
        "sorafs_reference_validate_provider_advert_json",
        extra_suffix=[u64("now")],
    ),
    "sorafs_reference_validate_provider_admission_json": bytes_label_signature(
        "sorafs_reference_validate_provider_admission_json"
    ),
    "sorafs_reference_validate_provider_admission_renewal_json": pair_signature(
        "sorafs_reference_validate_provider_admission_renewal_json",
        "envelope",
        "renewal",
    ),
    "sorafs_reference_validate_provider_admission_revocation_json": pair_signature(
        "sorafs_reference_validate_provider_admission_revocation_json",
        "envelope",
        "revocation",
    ),
    "sorafs_reference_validate_replication_order_json": bytes_label_signature(
        "sorafs_reference_validate_replication_order_json"
    ),
    "sorafs_reference_validate_signed_replication_order_json": bytes_label_signature(
        "sorafs_reference_validate_signed_replication_order_json"
    ),
    "sorafs_reference_validate_orderbook_json": bytes_label_signature(
        "sorafs_reference_validate_orderbook_json",
        extra_prefix=[u32("kind")],
    ),
    "sorafs_reference_validate_pop_json": bytes_label_signature(
        "sorafs_reference_validate_pop_json",
        extra_prefix=[u32("kind")],
    ),
    "sorafs_reference_validate_hedging_json": bytes_label_signature(
        "sorafs_reference_validate_hedging_json",
        extra_prefix=[u32("kind")],
    ),
    "sorafs_reference_validate_pdp_commitment_json": bytes_label_signature(
        "sorafs_reference_validate_pdp_commitment_json"
    ),
    "sorafs_reference_validate_pdp_challenge_json": bytes_label_signature(
        "sorafs_reference_validate_pdp_challenge_json"
    ),
    "sorafs_reference_validate_pdp_proof_json": bytes_label_signature(
        "sorafs_reference_validate_pdp_proof_json"
    ),
    "sorafs_reference_validate_pdp_commitment_challenge_json": pair_signature(
        "sorafs_reference_validate_pdp_commitment_challenge_json",
        "commitment",
        "challenge",
    ),
    "sorafs_reference_validate_pdp_challenge_proof_json": pair_signature(
        "sorafs_reference_validate_pdp_challenge_proof_json",
        "challenge",
        "proof",
    ),
    "sorafs_reference_validate_pdp_json": buffer_signature(
        "sorafs_reference_validate_pdp_json",
        [
            ptr("commitment_ptr"),
            usize("commitment_len"),
            ptr("commitment_label_ptr"),
            usize("commitment_label_len"),
            ptr("challenge_ptr"),
            usize("challenge_len"),
            ptr("challenge_label_ptr"),
            usize("challenge_label_len"),
            ptr("proof_ptr"),
            usize("proof_len"),
            ptr("proof_label_ptr"),
            usize("proof_label_len"),
            u64("generated_at"),
        ],
    ),
    "sorafs_reference_validate_por_json": pair_signature(
        "sorafs_reference_validate_por_json",
        "challenge",
        "proof",
    ),
    "sorafs_reference_validate_potr_json": bytes_label_signature(
        "sorafs_reference_validate_potr_json",
        extra_suffix=[u32("profile")],
    ),
    "sorafs_reference_validate_repair_json": bytes_label_signature(
        "sorafs_reference_validate_repair_json",
        extra_prefix=[u32("kind")],
    ),
    "sorafs_reference_validate_governance_json": buffer_signature(
        "sorafs_reference_validate_governance_json",
        [
            ptr("bytes_ptr"),
            usize("bytes_len"),
            ptr("label_ptr"),
            usize("label_len"),
            ptr("expected_cid_ptr"),
            usize("expected_cid_len"),
            u64("generated_at"),
        ],
    ),
    "sorafs_reference_validate_bundle_json": buffer_signature(
        "sorafs_reference_validate_bundle_json",
        [
            r"const\s+SorafsReferenceFfiBundlePayload\s*\*\s*payloads_ptr",
            usize("payloads_len"),
            u64("now"),
            u64("generated_at"),
        ],
    ),
}

errors = []

missing_header_declarations = sorted(rust_exports - header_declarations)
stale_header_declarations = sorted(header_declarations - rust_exports)
if missing_header_declarations:
    errors.append(
        "Rust SoraFS reference FFI exports missing from C header: "
        + ", ".join(missing_header_declarations)
    )
if stale_header_declarations:
    errors.append(
        "C header SoraFS reference declarations missing Rust exports: "
        + ", ".join(stale_header_declarations)
    )

missing_expected_exports = sorted(set(expected_signatures) - rust_exports)
if missing_expected_exports:
    errors.append(
        "missing required Rust SoraFS reference FFI exports: "
        + ", ".join(missing_expected_exports)
    )
for name, pattern in expected_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header SoraFS reference declaration has wrong signature: {name}")

missing_constants = sorted(set(rust_constants) - set(header_constants))
stale_constants = sorted(set(header_constants) - set(rust_constants))
if missing_constants:
    errors.append(
        "Rust SoraFS reference constants missing from C header: "
        + ", ".join(missing_constants)
    )
if stale_constants:
    errors.append(
        "C header SoraFS reference constants missing Rust constants: "
        + ", ".join(stale_constants)
    )
for name, value in sorted(rust_constants.items()):
    header_value = header_constants.get(name)
    if header_value is not None and header_value != value:
        errors.append(
            f"C header SoraFS reference constant drift: {name} is {header_value}, Rust is {value}"
        )

required_structs = [
    r"typedef\s+struct\s+SorafsReferenceFfiBuffer\s*\{\s*"
    r"uint8_t\s*\*\s*ptr\s*;\s*"
    r"size_t\s+len\s*;\s*"
    r"\}\s*SorafsReferenceFfiBuffer\s*;",
    r"typedef\s+struct\s+SorafsReferenceFfiBundlePayload\s*\{\s*"
    r"uint32_t\s+kind\s*;\s*"
    r"const\s+uint8_t\s*\*\s*bytes_ptr\s*;\s*"
    r"size_t\s+bytes_len\s*;\s*"
    r"const\s+uint8_t\s*\*\s*label_ptr\s*;\s*"
    r"size_t\s+label_len\s*;\s*"
    r"\}\s*SorafsReferenceFfiBundlePayload\s*;",
]
for pattern in required_structs:
    if re.search(pattern, header_text) is None:
        errors.append("C header SoraFS reference struct layout drift")

if errors:
    raise SystemExit("\n".join(errors))

print(
    "sorafs_reference.h declares "
    f"{len(rust_exports)} FFI symbols and {len(rust_constants)} selector constants"
)
PY
}

make_negative_workspace() {
  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sorafs-reference-header.XXXXXX")"
  cp "${RUST_FFI}" "${tmp}/reference_ffi.rs"
  cp "${HEADER}" "${tmp}/sorafs_reference.h"
  echo "${tmp}"
}

expect_contract_rejection() {
  local expected_fragment="${1}"
  local rust_ffi="${2}"
  local header="${3}"
  local output
  local status

  set +e
  output="$(run_contract_check "${rust_ffi}" "${header}" 2>&1)"
  status=$?
  set -e

  if [[ "${status}" -eq 0 ]]; then
    echo "[sorafs-reference-header] negative control unexpectedly passed" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected_fragment}"* ]]; then
    echo "[sorafs-reference-header] negative control failed for the wrong reason" >&2
    echo "[sorafs-reference-header] expected output fragment: ${expected_fragment}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "[sorafs-reference-header] negative control rejected expected drift: ${expected_fragment}"
}

if [[ "${MODE}" == --negative-control-* ]]; then
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT

  case "${MODE}" in
    --negative-control-missing-header)
      perl -0pi -e 's/SorafsReferenceFfiBuffer\s+sorafs_reference_validate_pdp_json\s*\([^;]*\);\n//s or die "missing PDP declaration target\n"' "${tmp}/sorafs_reference.h"
      expect_contract_rejection \
        "Rust SoraFS reference FFI exports missing from C header" \
        "${tmp}/reference_ffi.rs" \
        "${tmp}/sorafs_reference.h"
      ;;
    --negative-control-bad-signature)
      perl -0pi -e 's/(SorafsReferenceFfiBuffer\s+sorafs_reference_validate_pdp_json\s*\([^;]*?)const uint8_t \*commitment_ptr/$1uint8_t *commitment_ptr/s or die "missing PDP signature target\n"' "${tmp}/sorafs_reference.h"
      expect_contract_rejection \
        "C header SoraFS reference declaration has wrong signature: sorafs_reference_validate_pdp_json" \
        "${tmp}/reference_ffi.rs" \
        "${tmp}/sorafs_reference.h"
      ;;
    --negative-control-constant-drift)
      perl -0pi -e 's/#define SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF 19/#define SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF 99/s or die "missing PDP constant target\n"' "${tmp}/sorafs_reference.h"
      expect_contract_rejection \
        "C header SoraFS reference constant drift" \
        "${tmp}/reference_ffi.rs" \
        "${tmp}/sorafs_reference.h"
      ;;
    --negative-control-missing-rust-export)
      perl -0pi -e 's/pub\s+unsafe\s+extern\s+"C"\s+fn\s+sorafs_reference_validate_pdp_json\s*\(/pub unsafe extern "C" fn sorafs_reference_validate_pdp_json_removed(/s or die "missing Rust export target\n"' "${tmp}/reference_ffi.rs"
      expect_contract_rejection \
        "C header SoraFS reference declarations missing Rust exports" \
        "${tmp}/reference_ffi.rs" \
        "${tmp}/sorafs_reference.h"
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  exit 0
elif [[ -n "${MODE}" ]]; then
  usage
  exit 2
fi

run_contract_check "${RUST_FFI}" "${HEADER}"

if command -v "${CC:-cc}" >/dev/null 2>&1; then
  "${CC:-cc}" -fsyntax-only -x c -I"${ROOT_DIR}/crates/sorafs_manifest/include" "${HEADER}"
else
  echo "[sorafs-reference-header] skipping C syntax check: ${CC:-cc} not found" >&2
fi

if command -v "${CXX:-c++}" >/dev/null 2>&1; then
  "${CXX:-c++}" -fsyntax-only -x c++ -I"${ROOT_DIR}/crates/sorafs_manifest/include" "${HEADER}"
else
  echo "[sorafs-reference-header] skipping C++ syntax check: ${CXX:-c++} not found" >&2
fi
