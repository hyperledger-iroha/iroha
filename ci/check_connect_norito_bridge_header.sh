#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIB="${ROOT_DIR}/crates/connect_norito_bridge/src/lib.rs"
HEADER="${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h"
UMBRELLA="${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h"
MODE="${1:-}"

usage() {
  cat >&2 <<'EOF'
usage: ci/check_connect_norito_bridge_header.sh [negative-control]

negative-control:
  --negative-control-missing-recursive-header
  --negative-control-bad-recursive-signature
  --negative-control-missing-rust-export
  --negative-control-missing-privacy-header
  --negative-control-bad-privacy-signature
  --negative-control-missing-privacy-rust-export
  --negative-control-umbrella-drift
EOF
}

run_contract_check() {
  local rust_lib="${1}"
  local header="${2}"
  local umbrella="${3}"

  python3 - "$rust_lib" "$header" "$umbrella" <<'PY'
import re
import sys
from pathlib import Path

rust_lib = Path(sys.argv[1])
header = Path(sys.argv[2])
umbrella = Path(sys.argv[3])

rust_text = rust_lib.read_text(encoding="utf-8")
header_text = header.read_text(encoding="utf-8")
umbrella_text = umbrella.read_text(encoding="utf-8")

recursive_export_pattern = re.compile(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
    r'(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\('
)
recursive_declaration_pattern = re.compile(
    r'int32_t\s+'
    r'(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+)\s*\('
)
privacy_export_pattern = re.compile(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
    r'(iroha_privacy_[a-z0-9_]+)\s*\('
)
privacy_declaration_pattern = re.compile(
    r'(?:int32_t|void)\s+'
    r'(iroha_privacy_[a-z0-9_]+)\s*\('
)

rust_exports = set(recursive_export_pattern.findall(rust_text))
header_declarations = set(recursive_declaration_pattern.findall(header_text))
rust_privacy_exports = set(privacy_export_pattern.findall(rust_text))
header_privacy_declarations = set(privacy_declaration_pattern.findall(header_text))
bridge_abi_export = re.search(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_bridge_abi_version\s*\(',
    rust_text,
) is not None
bridge_abi_declaration = re.search(
    r"uint32_t\s+connect_norito_bridge_abi_version\s*\(\s*void\s*\)\s*;",
    header_text,
) is not None

required_abi6 = {
    "connect_norito_kagemusha_recursive_spend_init",
    "connect_norito_kagemusha_recursive_spend_append",
    "connect_norito_kagemusha_recursive_spend_transition_profile_init",
    "connect_norito_kagemusha_recursive_spend_transition_profile_append",
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
    "connect_norito_kagemusha_recursive_spend_verify",
    "connect_norito_kagemusha_recursive_spend_redeem",
}

def const_u8_ptr(name):
    return rf"const\s+uint8_t\s*\*\s*{name}"

def u8_out_ptr(name):
    return rf"uint8_t\s*\*\*\s*{name}"

def ulong(name):
    return rf"unsigned\s+long\s+{name}"

def ulong_ptr(name):
    return rf"unsigned\s+long\s*\*\s*{name}"

def c_signature(return_type, name, params):
    return (
        rf"{return_type}\s+{name}\s*\(\s*"
        + r"\s*,\s*".join(params)
        + r"\s*\)\s*;"
    )

def recursive_request_signature(name, out_ptr_name, out_len_name):
    return c_signature(
        "int32_t",
        name,
        [
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            u8_out_ptr(out_ptr_name),
            ulong_ptr(out_len_name),
        ],
    )

expected_recursive_signatures = {
    "connect_norito_kagemusha_recursive_spend_init": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_init",
        "out_bundle_ptr",
        "out_bundle_len",
    ),
    "connect_norito_kagemusha_recursive_spend_append": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_append",
        "out_bundle_ptr",
        "out_bundle_len",
    ),
    "connect_norito_kagemusha_recursive_spend_transition_profile_init": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_transition_profile_init",
        "out_profile_ptr",
        "out_profile_len",
    ),
    "connect_norito_kagemusha_recursive_spend_transition_profile_append": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_transition_profile_append",
        "out_profile_ptr",
        "out_profile_len",
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        [
            const_u8_ptr("profile_norito_ptr"),
            ulong("profile_norito_len"),
            u8_out_ptr("out_boundary_ptr"),
            ulong_ptr("out_boundary_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
        [
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_witness_ptr"),
            ulong_ptr("out_witness_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
        [
            const_u8_ptr("previous_witness_norito_ptr"),
            ulong("previous_witness_norito_len"),
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_witness_ptr"),
            ulong_ptr("out_witness_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_verify": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_verify",
        "out_result_ptr",
        "out_result_len",
    ),
    "connect_norito_kagemusha_recursive_spend_redeem": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_redeem",
        "out_instruction_ptr",
        "out_instruction_len",
    ),
}
required_privacy_ffi = {
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "iroha_privacy_free_buffer",
}
expected_privacy_signatures = {
    "iroha_privacy_capabilities_v1": (
        r"int32_t\s+iroha_privacy_capabilities_v1\s*\(\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_proof_request_v1": (
        r"int32_t\s+iroha_privacy_proof_request_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*algorithm_id_ptr\s*,\s*"
        r"unsigned\s+long\s+algorithm_id_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*entrypoint_ptr\s*,\s*"
        r"unsigned\s+long\s+entrypoint_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*vk_ref_ptr\s*,\s*"
        r"unsigned\s+long\s+vk_ref_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*public_inputs_ptr\s*,\s*"
        r"unsigned\s+long\s+public_inputs_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*witness_ptr\s*,\s*"
        r"unsigned\s+long\s+witness_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*proof_ptr\s*,\s*"
        r"unsigned\s+long\s+proof_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_build_proof_v1": (
        r"int32_t\s+iroha_privacy_build_proof_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*request_ptr\s*,\s*"
        r"unsigned\s+long\s+request_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_verify_proof_v1": (
        r"int32_t\s+iroha_privacy_verify_proof_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*request_ptr\s*,\s*"
        r"unsigned\s+long\s+request_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_free_buffer": (
        r"void\s+iroha_privacy_free_buffer\s*\(\s*"
        r"uint8_t\s*\*\s*ptr\s*"
        r"\)\s*;"
    ),
}

missing_exports = sorted(required_abi6 - rust_exports)
missing_header_declarations = sorted(required_abi6 - header_declarations)
undeclared_exports = sorted(rust_exports - header_declarations)
stale_header_declarations = sorted(header_declarations - rust_exports)
missing_privacy_exports = sorted(required_privacy_ffi - rust_privacy_exports)
missing_privacy_header_declarations = sorted(
    required_privacy_ffi - header_privacy_declarations
)
undeclared_privacy_exports = sorted(rust_privacy_exports - header_privacy_declarations)
stale_privacy_header_declarations = sorted(
    header_privacy_declarations - rust_privacy_exports
)

errors = []
if missing_exports:
    errors.append(
        "missing required Rust ABI-6 exports: " + ", ".join(missing_exports)
    )
if missing_header_declarations:
    errors.append(
        "missing required C header declarations: "
        + ", ".join(missing_header_declarations)
    )
if undeclared_exports:
    errors.append(
        "Rust recursive-spend exports missing from C header: "
        + ", ".join(undeclared_exports)
    )
if stale_header_declarations:
    errors.append(
        "C header recursive-spend declarations missing Rust exports: "
        + ", ".join(stale_header_declarations)
    )
for name, pattern in expected_recursive_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header recursive-spend declaration has wrong signature: {name}")
if missing_privacy_exports:
    errors.append(
        "missing required Rust privacy FFI exports: "
        + ", ".join(missing_privacy_exports)
    )
if missing_privacy_header_declarations:
    errors.append(
        "missing required C header privacy declarations: "
        + ", ".join(missing_privacy_header_declarations)
    )
if undeclared_privacy_exports:
    errors.append(
        "Rust privacy FFI exports missing from C header: "
        + ", ".join(undeclared_privacy_exports)
    )
if stale_privacy_header_declarations:
    errors.append(
        "C header privacy declarations missing Rust exports: "
        + ", ".join(stale_privacy_header_declarations)
    )
for name, pattern in expected_privacy_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header privacy declaration has wrong signature: {name}")
if not bridge_abi_export:
    errors.append("missing Rust C export: connect_norito_bridge_abi_version")
if not bridge_abi_declaration:
    errors.append("missing C header declaration: connect_norito_bridge_abi_version")
if '#include "connect_norito_bridge.h"' not in umbrella_text:
    errors.append("NoritoBridge.h must include connect_norito_bridge.h")

if errors:
    raise SystemExit("\n".join(errors))

print(
    "connect_norito_bridge.h declares all "
    f"{len(required_abi6)} ABI-6 recursive spend symbols and "
    f"{len(required_privacy_ffi)} privacy FFI symbols"
)
PY
}

make_negative_workspace() {
  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-bridge-header.XXXXXX")"
  cp "${RUST_LIB}" "${tmp}/lib.rs"
  cp "${HEADER}" "${tmp}/connect_norito_bridge.h"
  cp "${UMBRELLA}" "${tmp}/NoritoBridge.h"
  echo "${tmp}"
}

expect_contract_rejection() {
  local expected_fragment="${1}"
  local rust_lib="${2}"
  local header="${3}"
  local umbrella="${4}"
  local output
  local status

  set +e
  output="$(run_contract_check "${rust_lib}" "${header}" "${umbrella}" 2>&1)"
  status=$?
  set -e

  if [[ "${status}" -eq 0 ]]; then
    echo "[bridge-header] negative control unexpectedly passed" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected_fragment}"* ]]; then
    echo "[bridge-header] negative control failed for the wrong reason" >&2
    echo "[bridge-header] expected output fragment: ${expected_fragment}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "[bridge-header] negative control rejected expected drift: ${expected_fragment}"
}

if [[ "${MODE}" == --negative-control-* ]]; then
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT

  case "${MODE}" in
    --negative-control-missing-recursive-header)
      perl -0pi -e 's/int32_t\s+connect_norito_kagemusha_recursive_spend_redeem\s*\([^;]*\);\n//s or die "missing recursive declaration target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required C header declarations" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-recursive-signature)
      perl -0pi -e 's/uint8_t\*\*\s+out_instruction_ptr/uint8_t* out_instruction_ptr/s or die "missing recursive signature target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "C header recursive-spend declaration has wrong signature: connect_norito_kagemusha_recursive_spend_redeem" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-rust-export)
      perl -0pi -e 's/pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_kagemusha_recursive_spend_lineage_append_boundary\s*\(/pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_lineage_append_boundary_removed(/s or die "missing Rust export target\n"' "${tmp}/lib.rs"
      expect_contract_rejection \
        "missing required Rust ABI-6 exports" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-privacy-header)
      perl -0pi -e 's/int32_t\s+iroha_privacy_build_proof_v1\s*\([^;]*\);\n//s or die "missing privacy declaration target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required C header privacy declarations" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-privacy-signature)
      perl -0pi -e 's/unsigned\s+long\s+request_len/unsigned long* request_len/s or die "missing privacy signature target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "C header privacy declaration has wrong signature: iroha_privacy_build_proof_v1" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-privacy-rust-export)
      perl -0pi -e 's/pub\s+unsafe\s+extern\s+"C"\s+fn\s+iroha_privacy_build_proof_v1\s*\(/pub unsafe extern "C" fn iroha_privacy_build_proof_v1_removed(/s or die "missing privacy Rust export target\n"' "${tmp}/lib.rs"
      expect_contract_rejection \
        "missing required Rust privacy FFI exports" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-umbrella-drift)
      perl -0pi -e 's/#include "connect_norito_bridge\.h"\n//s or die "missing umbrella include target\n"' "${tmp}/NoritoBridge.h"
      expect_contract_rejection \
        "NoritoBridge.h must include connect_norito_bridge.h" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
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

run_contract_check "${RUST_LIB}" "${HEADER}" "${UMBRELLA}"

if command -v "${CC:-cc}" >/dev/null 2>&1; then
  "${CC:-cc}" -fsyntax-only -x c -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${HEADER}"
else
  echo "[bridge-header] skipping C syntax check: ${CC:-cc} not found" >&2
fi

if command -v "${CXX:-c++}" >/dev/null 2>&1; then
  "${CXX:-c++}" -fsyntax-only -x c++ -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${UMBRELLA}"
else
  echo "[bridge-header] skipping C++ syntax check: ${CXX:-c++} not found" >&2
fi
