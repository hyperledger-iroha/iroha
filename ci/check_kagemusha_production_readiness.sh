#!/bin/bash
set -euo pipefail

SCRIPT_SOURCE_ORIGINAL="${BASH_SOURCE[0]}"
if [[ -z "${SCRIPT_SOURCE_ORIGINAL}" || -L "${SCRIPT_SOURCE_ORIGINAL}" ]]; then
  echo "Kagemusha readiness rejects missing or symlinked script invocation" >&2
  exit 2
fi
SCRIPT_SOURCE_NORMALIZED="${SCRIPT_SOURCE_ORIGINAL}"
while [[ "${SCRIPT_SOURCE_NORMALIZED}" == ./* ]]; do
  SCRIPT_SOURCE_NORMALIZED="${SCRIPT_SOURCE_NORMALIZED#./}"
done
if [[ -z "${SCRIPT_SOURCE_NORMALIZED}" ]]; then
  echo "Kagemusha readiness script invocation path is empty" >&2
  exit 2
fi

path_has_noncanonical_component() {
  local path_text="${1}"
  local remainder="${path_text#/}"
  local component
  while [[ -n "${remainder}" ]]; do
    if [[ "${remainder}" == */* ]]; then
      component="${remainder%%/*}"
      remainder="${remainder#*/}"
    else
      component="${remainder}"
      remainder=""
    fi
    if [[ -z "${component}" || "${component}" == "." || "${component}" == ".." ]]; then
      return 0
    fi
  done
  return 1
}

if path_has_noncanonical_component "${SCRIPT_SOURCE_NORMALIZED}"; then
  echo "Kagemusha readiness script invocation path is not canonical" >&2
  exit 2
fi
if [[ "${SCRIPT_SOURCE_NORMALIZED}" == /* ]]; then
  SCRIPT_PATH_LEXICAL="${SCRIPT_SOURCE_NORMALIZED}"
else
  SCRIPT_PATH_LEXICAL="$(builtin pwd -P)/${SCRIPT_SOURCE_NORMALIZED}"
fi
SCRIPT_BASENAME="${SCRIPT_PATH_LEXICAL##*/}"
SCRIPT_DIRECTORY_LEXICAL="${SCRIPT_PATH_LEXICAL%/*}"
SCRIPT_DIRECTORY_PHYSICAL="$(
  builtin cd -P -- "${SCRIPT_DIRECTORY_LEXICAL}" && builtin pwd -P
)"
if [[ "${SCRIPT_DIRECTORY_PHYSICAL}" != "${SCRIPT_DIRECTORY_LEXICAL}" \
  || "${SCRIPT_BASENAME}" != "check_kagemusha_production_readiness.sh" \
  || "${SCRIPT_DIRECTORY_PHYSICAL##*/}" != "ci" ]]; then
  echo "Kagemusha readiness rejects moved or symlink-traversing script invocation" >&2
  exit 2
fi
SCRIPT_PATH="${SCRIPT_DIRECTORY_PHYSICAL}/${SCRIPT_BASENAME}"
if [[ ! -f "${SCRIPT_PATH}" || -L "${SCRIPT_PATH}" \
  || ! "${SCRIPT_SOURCE_ORIGINAL}" -ef "${SCRIPT_PATH}" ]]; then
  echo "Kagemusha readiness script path changed during root derivation" >&2
  exit 2
fi
DERIVED_ROOT_DIR="${SCRIPT_DIRECTORY_PHYSICAL%/*}"
ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-${DERIVED_ROOT_DIR}}"
MODE="candidate"
SELF_TEST="false"

for argument in "$@"; do
  case "${argument}" in
    candidate|promotion) MODE="${argument}" ;;
    --self-test) SELF_TEST="true" ;;
    *)
      echo "usage: ci/check_kagemusha_production_readiness.sh [candidate|promotion] [--self-test]" >&2
      exit 2
      ;;
  esac
done

PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-python3}"
PYTHON_SHA256=""
PYTHON_PIN_FD="-1"
PYTHON_PATH_FINGERPRINT=""
GATE_SHA256=""
GATE_PIN_FD="-1"
GATE_PATH_FINGERPRINT=""

promotion_stat_fingerprint() {
  local target="${1}"
  local observed
  if observed="$(/usr/bin/stat -f '%u %Lp %d %i %l %z %m %c' -- "${target}" 2>/dev/null)"; then
    :
  elif observed="$(/usr/bin/stat -c '%u %a %d %i %h %s %Y %Z' -- "${target}" 2>/dev/null)"; then
    :
  else
    return 1
  fi
  printf '%s\n' "${observed}"
}

promotion_assert_root_custody() {
  local target="${1}"
  local label="${2}"
  local remainder="${target#/}"
  local component
  local current="/"
  local fingerprint
  local owner_uid
  local mode_text
  local ignored
  local mode_value
  if [[ "${target}" != /* || "${target}" == "/" ]] \
    || path_has_noncanonical_component "${target}"; then
    echo "${label} must be one canonical absolute non-root path" >&2
    return 1
  fi
  fingerprint="$(promotion_stat_fingerprint "/")" || {
    echo "${label} root metadata is unavailable" >&2
    return 1
  }
  read -r owner_uid mode_text ignored <<<"${fingerprint}"
  if [[ "${owner_uid}" != "0" || ! "${mode_text}" =~ ^[0-7]{3,4}$ ]]; then
    echo "${label} filesystem root is not root-owned with a canonical mode" >&2
    return 1
  fi
  mode_value=$((8#${mode_text}))
  if (( mode_value & 0022 )); then
    echo "${label} filesystem root is group/world writable" >&2
    return 1
  fi
  while [[ -n "${remainder}" ]]; do
    if [[ "${remainder}" == */* ]]; then
      component="${remainder%%/*}"
      remainder="${remainder#*/}"
    else
      component="${remainder}"
      remainder=""
    fi
    current="${current%/}/${component}"
    if [[ -L "${current}" || ! -e "${current}" ]]; then
      echo "${label} traverses a missing or symlinked path component" >&2
      return 1
    fi
    fingerprint="$(promotion_stat_fingerprint "${current}")" || {
      echo "${label} metadata is unavailable" >&2
      return 1
    }
    read -r owner_uid mode_text ignored <<<"${fingerprint}"
    if [[ "${owner_uid}" != "0" || ! "${mode_text}" =~ ^[0-7]{3,4}$ ]]; then
      echo "${label} path component is not root-owned with a canonical mode" >&2
      return 1
    fi
    mode_value=$((8#${mode_text}))
    if (( mode_value & 0022 )); then
      echo "${label} path component is group/world writable" >&2
      return 1
    fi
  done
}

if [[ "${MODE}" == "promotion" ]]; then
  if [[ -n "${KAGEMUSHA_PRODUCTION_READINESS_ROOT+x}" ]]; then
    echo "promotion rejects KAGEMUSHA_PRODUCTION_READINESS_ROOT; run the checked-in gate in place" >&2
    exit 2
  fi
  # Bootstrap contract: an independently authenticated controller must install
  # the reviewed checkout below this root-owned, non-group/world-writable path
  # and verify the reviewed gate digest before invoking this exact path.  The
  # digest environment value repeats that launcher's decision; it cannot replace
  # the external pre-exec check because a substituted script could omit its own
  # checks or simply exit zero.
  promotion_assert_root_custody "${DERIVED_ROOT_DIR}" "promotion readiness checkout" || exit 2
  promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate" || exit 2
  if (( EUID != 0 )); then
    echo "production promotion must enter the digest-pinned interpreter as root" >&2
    exit 2
  fi
  GATE_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256:-}"
  if [[ ! "${GATE_SHA256}" =~ ^[0-9a-f]{64}$ || "${GATE_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "promotion requires KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256 from an independently authenticated launcher/controller" >&2
    exit 2
  fi
  GATE_PATH_FINGERPRINT="$(promotion_stat_fingerprint "${SCRIPT_PATH}")" || {
    echo "promotion readiness gate metadata is unavailable" >&2
    exit 2
  }
  exec 8<"${SCRIPT_PATH}"
  GATE_PIN_FD="8"
  if [[ -x /usr/bin/shasum ]]; then
    OBSERVED_GATE_SHA256="$(/usr/bin/shasum -a 256 -- "/dev/fd/${GATE_PIN_FD}")"
  elif [[ -x /usr/bin/sha256sum ]]; then
    OBSERVED_GATE_SHA256="$(/usr/bin/sha256sum -- "/dev/fd/${GATE_PIN_FD}")"
  else
    echo "promotion requires root-installed /usr/bin/shasum or /usr/bin/sha256sum" >&2
    exit 2
  fi
  OBSERVED_GATE_SHA256="${OBSERVED_GATE_SHA256%% *}"
  if [[ "${OBSERVED_GATE_SHA256}" != "${GATE_SHA256}" ]]; then
    echo "promotion readiness gate differs from its independently reviewed SHA-256" >&2
    exit 2
  fi
  if [[ "$(promotion_stat_fingerprint "${SCRIPT_PATH}")" != "${GATE_PATH_FINGERPRINT}" ]]; then
    echo "promotion readiness gate changed during bootstrap" >&2
    exit 2
  fi
  promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate" || exit 2
  PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-}"
  PYTHON_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256:-}"
  if [[ "${PYTHON_BIN}" != /* || ! -f "${PYTHON_BIN}" || -L "${PYTHON_BIN}" || ! -x "${PYTHON_BIN}" || ! "${PYTHON_SHA256}" =~ ^[0-9a-f]{64}$ || "${PYTHON_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "promotion requires a canonical absolute digest-pinned Python interpreter" >&2
    exit 2
  fi
  promotion_assert_root_custody "${PYTHON_BIN}" "promotion Python interpreter" || exit 2
  PYTHON_PATH_FINGERPRINT="$(promotion_stat_fingerprint "${PYTHON_BIN}")" || {
    echo "promotion Python interpreter metadata is unavailable" >&2
    exit 2
  }
  exec 9<"${PYTHON_BIN}"
  PYTHON_PIN_FD="9"
  if [[ -x /usr/bin/shasum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/shasum -a 256 -- "/dev/fd/${PYTHON_PIN_FD}")"
  elif [[ -x /usr/bin/sha256sum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/sha256sum -- "/dev/fd/${PYTHON_PIN_FD}")"
  else
    echo "promotion requires root-installed /usr/bin/shasum or /usr/bin/sha256sum" >&2
    exit 2
  fi
  OBSERVED_PYTHON_SHA256="${OBSERVED_PYTHON_SHA256%% *}"
  if [[ "${OBSERVED_PYTHON_SHA256}" != "${PYTHON_SHA256}" ]]; then
    echo "promotion Python interpreter differs from its trusted SHA-256" >&2
    exit 2
  fi
  if [[ "$(promotion_stat_fingerprint "${PYTHON_BIN}")" != "${PYTHON_PATH_FINGERPRINT}" ]]; then
    echo "promotion Python interpreter changed before execution" >&2
    exit 2
  fi
  promotion_assert_root_custody "${PYTHON_BIN}" "promotion Python interpreter" || exit 2
fi

if /usr/bin/git -C "${ROOT_DIR}" diff --quiet --diff-filter=U --; then
  :
else
  INDEX_STATUS=$?
  if [[ "${INDEX_STATUS}" -eq 1 ]]; then
    echo "Kagemusha readiness rejects unresolved Git index entries" >&2
  else
    echo "Kagemusha readiness could not inspect the Git index" >&2
  fi
  exit 2
fi

# Candidate and promotion use Python 3.10 features. Reject an older ambient
# interpreter before the embedded gate can fail with a misleading traceback.
if ! "${PYTHON_BIN}" -I -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo "Kagemusha production readiness requires Python 3.10 or newer" >&2
  exit 2
fi

# Portable Bash still enters Python by pathname.  At this point every path
# component and the file are root-owned and non-writable by group/other, so no
# principal outside the documented root trust boundary can win the remaining
# execve lookup.  Inherited fd 8 binds the gate to the controller's reviewed
# digest, and inherited fd 9 binds the running image back to the exact pre-exec
# bytes and metadata before any promotion decision is made.
"${PYTHON_BIN}" -I - "${ROOT_DIR}" "${MODE}" "${SELF_TEST}" "${PYTHON_SHA256}" \
  "${PYTHON_PIN_FD}" "${PYTHON_PATH_FINGERPRINT}" "${GATE_SHA256}" \
  "${GATE_PIN_FD}" "${GATE_PATH_FINGERPRINT}" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import types
from collections.abc import Callable
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
self_test = sys.argv[3] == "true"
trusted_python_sha256 = sys.argv[4]
try:
    trusted_python_fd = int(sys.argv[5])
except ValueError:
    trusted_python_fd = -2
trusted_python_path_fingerprint = sys.argv[6]
trusted_gate_sha256 = sys.argv[7]
try:
    trusted_gate_fd = int(sys.argv[8])
except ValueError:
    trusted_gate_fd = -2
trusted_gate_path_fingerprint = sys.argv[9]

READINESS = "ci/check_kagemusha_production_readiness.sh"
MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_INCLUDE = 'include!("kagemusha_model.rs");'
MODEL_VERIFIER_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_release_verifier.rs"
MODEL_VERIFIER_MODULE = "mod kagemusha_release_verifier;"
PRIVACY = "crates/iroha_data_model/src/privacy.rs"
PRIVACY_PROTOCOL = "crates/iroha_data_model/src/privacy/protocol.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
CATALOG_COMPONENT = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4_release_catalog_impl.rs"
CATALOG_INCLUDE = 'include!("kagemusha_terminal_registry_v4_release_catalog_impl.rs");\n'
CORE = "crates/iroha_core/src/smartcontracts/isi/offline.rs"
STEP_TRANSITION = "crates/iroha_core/src/zk/kagemusha_step_transition.rs"
RECURSIVE_BACKEND = "crates/iroha_core/src/zk/kagemusha_v2.rs"
RECURSION_ADAPTER = "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs"
VALUE_CONTRACT = "crates/iroha_data_model/tests/kagemusha_value_contract.rs"
SCHEMA_GOLDEN = "crates/iroha_data_model/tests/offline_public_schema_golden.rs"
CONFIG = "crates/iroha_config/src/parameters/user.rs"
NODE = "crates/irohad/src/main.rs"
KAGAMI = "crates/iroha_kagami/src/kagemusha.rs"
BUNDLE = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs"
ROUTES = "crates/iroha_torii_shared/src/route_catalog.rs"
WORKFLOW = ".github/workflows/pr_kagemusha_payload_bench.yml"
IOS_EVIDENCE_MODULE = "scripts/kagemusha_candidate_ios_evidence.py"
PRODUCTION_IOS_EVIDENCE_MODULE = "scripts/kagemusha_production_ios_evidence.py"
SOURCE_TREE_SEAL = "scripts/kagemusha_source_tree_seal.py"
SOURCE_GIT = Path("/usr/bin/git")
SOURCE_ALLOWED_SIGNERS_PATH_ENV = (
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH"
)
SOURCE_ALLOWED_SIGNERS_SHA256_ENV = (
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256"
)
SOURCE_REVOCATION_PATH_ENV = "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH"
SOURCE_REVOCATION_SHA256_ENV = (
    "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256"
)
SOURCE_SEAL_PROJECTION_PATH_ENV = (
    "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION"
)
SOURCE_SEAL_PROJECTION_SHA256_ENV = (
    "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256"
)
PROMOTION_STAGING_PARENT = Path(
    "/private/var/db/iroha-kagemusha-readiness-v1"
    if sys.platform == "darwin"
    else "/var/lib/iroha/kagemusha-readiness-v1"
)

ARTIFACTS = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
REPORT_ARTIFACT_PURPOSES = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
FINAL_METADATA = (
    "topup-finality-roster-v4.norito",
    "manifest.norito",
    "manifest.norito.sha256",
    "manifest.json",
    "release-attestation-v4.norito",
    "physical-device-benchmark.evidence",
    "cryptographic-review.evidence",
    "recursive-step-two-qualification-v4.norito",
    "promotion-record-v4.norito",
)
MAX_RELEASE_DIRECTORIES = 16
MAX_RELEASE_INVENTORY_ENTRIES = len(ARTIFACTS + FINAL_METADATA)
MAX_MANIFEST_BYTES = 32 * 1024 * 1024
MAX_DIGEST_SIDECAR_BYTES = 65
MAX_RELEASE_ATTESTATION_BYTES = 1024 * 1024
MAX_BENCHMARK_EVIDENCE_BYTES = 16 * 1024 * 1024
MAX_CRYPTOGRAPHIC_REVIEW_BYTES = 1024 * 1024
MAX_QUALIFICATION_RECEIPT_BYTES = 2 * 384 * 1024 + 16 * 1024
MAX_PROMOTION_RECORD_BYTES = 1024 * 1024
MAX_KAGAMI_VERIFIER_BYTES = 512 * 1024 * 1024
MAX_READINESS_GATE_BYTES = 8 * 1024 * 1024
MAX_REVIEWED_SOURCE_CLOSURE_BYTES = 16 * 1024 * 1024
MAX_REVIEWED_HELPER_BYTES = 4 * 1024 * 1024
MAX_SOURCE_ALLOWED_SIGNERS_BYTES = 64 * 1024
MAX_SOURCE_REVOCATION_BYTES = 16 * 1024 * 1024
MAX_SOURCE_SEAL_PROJECTION_BYTES = 16 * 1024
MAX_DECLARED_ARTIFACT_FILE_BYTES = 5 * 1024 * 1024 * 1024
MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES = 10 * 1024 * 1024 * 1024
MAX_CATALOG_AGGREGATE_BYTES = 12 * 1024 * 1024 * 1024
BOUNDED_AUTHENTICATED_METADATA = (
    ("release-attestation-v4.norito", MAX_RELEASE_ATTESTATION_BYTES),
    ("cryptographic-review.evidence", MAX_CRYPTOGRAPHIC_REVIEW_BYTES),
    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),
)
KAGAMI_VERIFIER_PATH_ENV = "KAGEMUSHA_V4_KAGAMI_BIN"
KAGAMI_VERIFIER_SHA256_ENV = "KAGEMUSHA_V4_KAGAMI_SHA256"
SANITIZED_VERIFIER_ENV = {
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "TMPDIR": str(PROMOTION_STAGING_PARENT),
}
READ_CHUNK_BYTES = 1024 * 1024

# Promotion trust boundary: the controller-authenticated, root-installed gate,
# its digest-pinned Python interpreter, and the independently reviewed source
# closure are trusted code.
# Every operator-supplied production input is required to live below a
# symlink-free, root-owned path chain that is not writable by group or other.
# Root itself remains the production trust principal; this gate does not try to
# defend against a malicious caller that can replace the gate while it runs.
PRODUCTION_TRUSTED_UID = 0
REVIEWED_SOURCE_CLOSURE_KEYS = {
    "schema",
    "base_commit",
    "source_commit",
    "source_repo_dirty",
    "source_tree_sha256",
    "tracked_binary_diff_sha256",
    "untracked_file_count",
    "untracked_path_mode_blob_oid_manifest",
    "untracked_path_mode_blob_oid_manifest_sha256",
    "ignored_cargo_lock_size_bytes",
    "ignored_cargo_lock_sha256",
    "combined_source_fingerprint_sha256",
}
SOURCE_SEAL_PROJECTION_KEYS = {
    "build_script_observed",
    "outer_policy",
    "reviewed_source_closure_hex",
    "reviewed_source_closure_sha256",
    "schema",
    "source_authority",
    "source_commit",
    "source_date_epoch",
    "source_repo_dirty",
    "source_tree_sha256",
}
SOURCE_AUTHORITY_KEYS = {
    "commit",
    "commit_object_sha256",
    "commit_object_size",
    "committer_epoch",
    "git_tree",
    "ordered_parents",
    "parent_commit",
    "parent_tree",
    "signature",
}
SOURCE_SIGNATURE_KEYS = {
    "allowed_signers_sha256",
    "mechanism",
    "principal",
    "public_key_sha256",
    "revocation_sha256",
    "signature_namespace",
}
ROUTE_LITERALS = (
    "/v1/offline/readiness",
    "/v1/offline/top-up",
    "/v1/offline/redeem",
    "/v1/offline/operations/{operation_id}",
)
RETIRED_RECURSIVE_LIFECYCLE_TYPES = (
    "KagemushaRecursiveSpendInitRequestV2",
    "KagemushaRecursiveSpendInitResultV2",
    "KagemushaRecursiveSpendTopUpUnsignedV2",
    "KagemushaRecursiveSpendTopUpRequestV2",
    "KagemushaRecursiveSpendTopUpAnchorV2",
    "KagemushaRecursiveSpendAppendInputV2",
    "KagemushaRecursiveSpendSplitIntentBuildRequestV2",
    "KagemushaRecursiveSpendSplitIntentV2",
    "KagemushaRecursiveSpendAppendRequestV2",
    "KagemushaRecursiveSpendRedeemBuildRequestV2",
    "KagemushaRecursiveSpendRedeemBuildResultV2",
    "KagemushaRecursiveSpendRedemptionIntentV2",
    "KagemushaRecursiveSpendRedemptionIntentBuildRequestV2",
    "KagemushaRecursiveSpendPeerSplitTransitionV2",
    "KagemushaRecursiveSpendRedemptionChangeTransitionV2",
    "KagemushaRecursiveSpendPublicStatementV2",
    "KagemushaRecursiveSpendProofV2",
    "KagemushaRecursiveSpendBundleV2",
    "KagemushaRecursiveSpendRedeemChangeBranchV2",
    "KagemushaRecursiveSpendSplitResultV2",
    "KagemushaRecursiveSpendPeerPaymentV2",
    "KagemushaRecursiveSpendTopUpFinalityEvidenceV2",
    "KagemushaRecursiveSpendVerifyRequestV2",
    "KagemushaRecursiveSpendBundleSummaryV2",
    "KagemushaRecursiveSpendVerifyResultV2",
    "KagemushaRecursiveSpendRedeemResultV2",
    "KagemushaRecursiveSpendRedeemUnsignedV2",
    "KagemushaRecursiveSpendRedeemRequestV2",
    "KagemushaRecursiveSpendTransitionV2",
    "KagemushaRecursiveSpendTransitionValuesV2",
    "KagemushaRecursiveSpendTransitionConfigV2",
    "KagemushaRecursiveSpendTransitionCircuitV2",
    "KagemushaRecursiveSpendTransitionEqCircuitV2",
    "KagemushaRecursiveSpendTransitionEpCircuitV2",
    "kagemusha_recursive_spend_transition_instance_columns_v2",
    "KAGEMUSHA_RECURSIVE_SPEND_V2_PUBLIC_INPUTS_SCHEMA",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V2",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1",
    "KagemushaRecursiveSpendArtifactManifestV3",
    "KagemushaRecursiveSpendPromotedReleaseV3",
    "KagemushaRecursiveSpendArtifactBindingV3",
)
RETIRED_RECURSIVE_V3_MARKERS = (
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V3",
    "KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V3",
    "KAGEMUSHA_VERIFIER_PURPOSE_STEP_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROOF_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3",
    "is_kagemusha_v3_",
    "V3 artifact release",
)


def read(relative: str, errors: list[str]) -> str:
    path = root / relative
    if not path.is_file():
        errors.append(f"missing corridor file: {relative}")
        return ""
    return path.read_text(encoding="utf-8")


def read_reviewed_model(errors: list[str], overrides: dict[str, str]) -> str:
    """Read the parent and both authenticated model components as one source."""

    # Preserve the existing negative-test API: a MODEL override is already a
    # complete logical source, while MODEL_COMPONENT can exercise the split.
    if MODEL in overrides:
        return overrides[MODEL]
    parent = read(MODEL, errors)
    component = (
        overrides[MODEL_COMPONENT]
        if MODEL_COMPONENT in overrides
        else read(MODEL_COMPONENT, errors)
    )
    verifier = (
        overrides[MODEL_VERIFIER_COMPONENT]
        if MODEL_VERIFIER_COMPONENT in overrides
        else read(MODEL_VERIFIER_COMPONENT, errors)
    )
    if parent.count(MODEL_INCLUDE) != 1:
        errors.append(
            f"{MODEL}: expected exactly one reviewed {Path(MODEL_COMPONENT).name} include"
        )
        return parent
    parent = parent.replace(MODEL_INCLUDE, component, 1)
    if parent.count(MODEL_VERIFIER_MODULE) != 1:
        errors.append(
            f"{MODEL}: expected exactly one reviewed {Path(MODEL_VERIFIER_COMPONENT).name} module"
        )
        return parent
    for marker in (
        "const VERIFIER_IDENTITY_SCHEMA_V4",
        "pub fn kagemusha_recursive_spend_verifier_key_id_v4",
    ):
        if verifier.count(marker) != 1:
            errors.append(
                f"{MODEL_VERIFIER_COMPONENT}: expected exactly one {marker!r}"
            )
    return parent.replace(
        MODEL_VERIFIER_MODULE,
        "mod kagemusha_release_verifier {\n" + verifier + "\n}",
        1,
    )


def read_reviewed_catalog(errors: list[str], overrides: dict[str, str]) -> str:
    """Read the terminal registry and release-catalog implementation as one source."""

    if CATALOG in overrides:
        return overrides[CATALOG]
    parent = read(CATALOG, errors)
    component = (
        overrides[CATALOG_COMPONENT]
        if CATALOG_COMPONENT in overrides
        else read(CATALOG_COMPONENT, errors)
    )
    if parent.count(CATALOG_INCLUDE) != 1:
        errors.append(
            f"{CATALOG}: expected exactly one reviewed {Path(CATALOG_COMPONENT).name} include"
        )
        return parent
    return parent.replace(CATALOG_INCLUDE, component, 1)


def pin_regular_metadata(
    path: Path,
    label: str,
    *,
    require_single_link: bool = True,
    allow_empty: bool = False,
) -> tuple[int, tuple[int, ...]]:
    """Open and retain one exact regular-file metadata identity.

    Operator-controlled release inputs must be singly linked.  Fixed system
    executables may have several links on sealed operating-system volumes; for
    those callers the root-owned, non-writable path chain is the custody
    boundary and every observed link-count change remains part of the pin.
    """

    before = path.lstat()
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or (require_single_link and before.st_nlink != 1)
        or before.st_size < 0
        or (not allow_empty and before.st_size == 0)
    ):
        qualification = "singly-linked " if require_single_link else ""
        content = "" if allow_empty else "nonempty "
        raise ValueError(f"{label} must be a {content}{qualification}regular file")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fingerprint = (
            before.st_dev,
            before.st_ino,
            before.st_nlink,
            before.st_mode,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
            before.st_uid,
            before.st_gid,
        )
        if not os.path.samestat(before, opened) or fingerprint != (
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mode,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            opened.st_uid,
            opened.st_gid,
        ):
            raise ValueError(f"{label} changed while it was pinned")
        return descriptor, fingerprint
    except BaseException:
        os.close(descriptor)
        raise


def pin_directory_metadata(path: Path, label: str) -> tuple[int, tuple[int, ...]]:
    """Open and retain one exact real-directory metadata identity."""

    before = path.lstat()
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise ValueError(f"{label} must be a non-symlink directory")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        fingerprint = (
            before.st_dev,
            before.st_ino,
            before.st_nlink,
            before.st_mode,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
            before.st_uid,
            before.st_gid,
        )
        if not os.path.samestat(before, opened) or fingerprint != (
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mode,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            opened.st_uid,
            opened.st_gid,
        ):
            raise ValueError(f"{label} changed while it was pinned")
        return descriptor, fingerprint
    except BaseException:
        os.close(descriptor)
        raise


def absolute_directory_chain(path: Path) -> list[Path]:
    """Return every directory from the filesystem root through an absolute path."""

    if not path.is_absolute():
        raise ValueError("catalog path must be absolute")
    if any(part in {".", ".."} for part in path.parts[1:]):
        raise ValueError("catalog path must contain only normal absolute components")
    chain = [Path(path.anchor)]
    current = chain[0]
    for part in path.parts[1:]:
        current /= part
        chain.append(current)
    return chain


def revalidate_pinned_metadata(
    path: Path, descriptor: int, fingerprint: tuple[int, ...], label: str
) -> None:
    """Prove a retained descriptor and its pathname still name the pinned file."""

    opened = os.fstat(descriptor)
    after_path = path.lstat()
    observed_open = (
        opened.st_dev,
        opened.st_ino,
        opened.st_nlink,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_uid,
        opened.st_gid,
    )
    observed_path = (
        after_path.st_dev,
        after_path.st_ino,
        after_path.st_nlink,
        after_path.st_mode,
        after_path.st_size,
        after_path.st_mtime_ns,
        after_path.st_ctime_ns,
        after_path.st_uid,
        after_path.st_gid,
    )
    if observed_open != fingerprint or observed_path != fingerprint:
        raise ValueError(f"{label} changed during authenticated catalog verification")


def hash_pinned_descriptor(
    descriptor: int,
    fingerprint: tuple[int, ...],
    maximum_bytes: int,
    label: str,
) -> str:
    """Hash exact bytes through a retained descriptor without reopening its path."""

    size = fingerprint[4]
    if size <= 0 or size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise ValueError(f"{label} became truncated while it was hashed")
        digest.update(chunk)
        offset += len(chunk)
    if os.fstat(descriptor).st_size != size:
        raise ValueError(f"{label} changed while it was hashed")
    return digest.hexdigest()


def validate_inherited_promotion_python() -> None:
    """Bind the running interpreter to the descriptor opened before shell exec."""

    if trusted_python_fd != 9:
        raise ValueError("promotion Python descriptor handoff is missing")
    try:
        opened = os.fstat(trusted_python_fd)
    except OSError as error:
        raise ValueError("promotion Python descriptor handoff is closed") from error
    if (
        not stat.S_ISREG(opened.st_mode)
        or opened.st_uid != PRODUCTION_TRUSTED_UID
        or stat.S_IMODE(opened.st_mode) & 0o022
        or opened.st_nlink < 1
        or not stat.S_IMODE(opened.st_mode) & 0o111
        or opened.st_size <= 0
        or opened.st_size > MAX_KAGAMI_VERIFIER_BYTES
    ):
        raise ValueError("promotion Python descriptor does not have production custody")
    observed_fingerprint = " ".join(
        (
            str(opened.st_uid),
            format(stat.S_IMODE(opened.st_mode), "o"),
            str(opened.st_dev),
            str(opened.st_ino),
            str(opened.st_nlink),
            str(opened.st_size),
            str(int(opened.st_mtime)),
            str(int(opened.st_ctime)),
        )
    )
    if observed_fingerprint != trusted_python_path_fingerprint:
        raise ValueError("promotion Python descriptor differs from its pre-exec path pin")
    descriptor_fingerprint = (
        opened.st_dev,
        opened.st_ino,
        opened.st_nlink,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_uid,
        opened.st_gid,
    )
    if (
        hash_pinned_descriptor(
            trusted_python_fd,
            descriptor_fingerprint,
            MAX_KAGAMI_VERIFIER_BYTES,
            "inherited promotion Python",
        )
        != trusted_python_sha256
    ):
        raise ValueError("inherited promotion Python differs from its trusted SHA-256")
    runtime = Path(sys.executable)
    runtime_metadata = runtime.lstat()
    if runtime.is_symlink() or not os.path.samestat(opened, runtime_metadata):
        raise ValueError("running promotion Python differs from its inherited descriptor")


def validate_inherited_promotion_gate() -> None:
    """Bind this gate to the independently reviewed launcher digest and path."""

    if trusted_gate_fd != 8:
        raise ValueError("promotion gate descriptor handoff is missing")
    try:
        opened = os.fstat(trusted_gate_fd)
    except OSError as error:
        raise ValueError("promotion gate descriptor handoff is closed") from error
    if (
        not stat.S_ISREG(opened.st_mode)
        or opened.st_uid != PRODUCTION_TRUSTED_UID
        or stat.S_IMODE(opened.st_mode) & 0o022
        or opened.st_nlink != 1
        or not stat.S_IMODE(opened.st_mode) & 0o111
        or opened.st_size <= 0
        or opened.st_size > MAX_READINESS_GATE_BYTES
    ):
        raise ValueError("promotion gate descriptor does not have production custody")
    observed_fingerprint = " ".join(
        (
            str(opened.st_uid),
            format(stat.S_IMODE(opened.st_mode), "o"),
            str(opened.st_dev),
            str(opened.st_ino),
            str(opened.st_nlink),
            str(opened.st_size),
            str(int(opened.st_mtime)),
            str(int(opened.st_ctime)),
        )
    )
    if observed_fingerprint != trusted_gate_path_fingerprint:
        raise ValueError("promotion gate differs from its pre-exec path pin")
    descriptor_fingerprint = (
        opened.st_dev,
        opened.st_ino,
        opened.st_nlink,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_uid,
        opened.st_gid,
    )
    if (
        hash_pinned_descriptor(
            trusted_gate_fd,
            descriptor_fingerprint,
            MAX_READINESS_GATE_BYTES,
            "inherited promotion gate",
        )
        != trusted_gate_sha256
    ):
        raise ValueError("inherited promotion gate differs from its reviewed SHA-256")
    gate_path = root / READINESS
    gate_metadata = gate_path.lstat()
    if gate_path.is_symlink() or not os.path.samestat(opened, gate_metadata):
        raise ValueError("running promotion gate differs from its inherited descriptor")


def read_pinned_descriptor(
    descriptor: int,
    fingerprint: tuple[int, ...],
    maximum_bytes: int,
    label: str,
    *,
    allow_empty: bool = False,
) -> bytes:
    """Read exact bounded bytes through a retained descriptor."""

    size = fingerprint[4]
    if size < 0 or (not allow_empty and size == 0) or size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    chunks: list[bytes] = []
    offset = 0
    while offset < size:
        chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
        if not chunk:
            raise ValueError(f"{label} became truncated while it was read")
        chunks.append(chunk)
        offset += len(chunk)
    if os.fstat(descriptor).st_size != size:
        raise ValueError(f"{label} changed while it was read")
    return b"".join(chunks)


def inspect_pinned_prefix(
    descriptor: int,
    fingerprint: tuple[int, ...],
    expected_bytes: int,
    maximum_bytes: int,
    prefix_bytes: int,
    label: str,
) -> bytes:
    """Inspect a prefix through the descriptor retained for the whole decision."""

    if (
        expected_bytes <= 0
        or expected_bytes > maximum_bytes
        or fingerprint[4] != expected_bytes
        or prefix_bytes <= 0
        or prefix_bytes > expected_bytes
    ):
        raise ValueError(f"{label} does not match its bounded declared size")
    prefix = os.pread(descriptor, prefix_bytes, 0)
    if len(prefix) != prefix_bytes or os.fstat(descriptor).st_size != expected_bytes:
        raise ValueError(f"{label} changed while it was inspected")
    return prefix


def require_production_root_custody(descriptor: int, label: str) -> None:
    """Mirror runtime root ownership and group/world write rejection."""

    metadata = os.fstat(descriptor)
    if metadata.st_uid != PRODUCTION_TRUSTED_UID:
        raise ValueError(f"{label} must be owned by root")
    if stat.S_IMODE(metadata.st_mode) & 0o022:
        raise ValueError(f"{label} must not be group/world writable")


def snapshot_private_bytes(
    payload: bytes,
    file_name: str,
    label: str,
    staging_parent: Path,
    *,
    allow_empty: bool = False,
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Put already-pinned bounded bytes in one owner-private validation slot."""

    if not payload and not allow_empty:
        raise ValueError(f"{label} snapshot payload is empty")
    temporary = tempfile.TemporaryDirectory(
        prefix="kagemusha-pinned-input-", dir=staging_parent
    )
    temporary_path = Path(temporary.name).resolve(strict=True)
    os.chmod(temporary_path, 0o700)
    temporary_metadata = temporary_path.lstat()
    if (
        not stat.S_ISDIR(temporary_metadata.st_mode)
        or temporary_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(temporary_metadata.st_mode) != 0o700
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot directory is not owner-private")
    target = temporary_path / file_name
    descriptor = os.open(
        target,
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError(f"could not snapshot {label}")
            view = view[written:]
        os.fchmod(descriptor, 0o600)
        os.fsync(descriptor)
        observed = bytearray()
        offset = 0
        while offset < len(payload):
            chunk = os.pread(
                descriptor, min(READ_CHUNK_BYTES, len(payload) - offset), offset
            )
            if not chunk:
                raise ValueError(f"{label} snapshot became truncated")
            observed.extend(chunk)
            offset += len(chunk)
        opened = os.fstat(descriptor)
        path_metadata = target.lstat()
        if (
            bytes(observed) != payload
            or opened.st_size != len(payload)
            or not os.path.samestat(opened, path_metadata)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o600
        ):
            raise ValueError(f"{label} snapshot bytes changed")
    except BaseException:
        os.close(descriptor)
        temporary.cleanup()
        raise
    os.close(descriptor)
    return temporary, target


def evidence_bytes_are_non_placeholder(payload: bytes) -> bool:
    """Reject bounded signed-evidence slots containing placeholder markers."""

    if len(payload) < 64 or len(payload) > MAX_BENCHMARK_EVIDENCE_BYTES:
        return False
    return (
        re.search(
            rb"(?:placeholder|synthetic|dummy|todo|not[ -]?reviewed)",
            payload,
            flags=re.IGNORECASE,
        )
        is None
    )


def snapshot_pinned_executable(
    descriptor: int,
    fingerprint: tuple[int, ...],
    label: str,
    staging_parent: Path,
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Materialize exact already-hashed executable bytes in an owner-private directory."""

    temporary = tempfile.TemporaryDirectory(
        prefix="kagemusha-kagami-verifier-", dir=staging_parent
    )
    temporary_path = Path(temporary.name).resolve(strict=True)
    os.chmod(temporary_path, 0o700)
    temporary_metadata = temporary_path.lstat()
    if (
        not stat.S_ISDIR(temporary_metadata.st_mode)
        or temporary_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(temporary_metadata.st_mode) != 0o700
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot directory is not owner-private")
    target = temporary_path / "kagami"
    target_descriptor = os.open(
        target,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o500,
    )
    try:
        offset = 0
        size = fingerprint[4]
        while offset < size:
            chunk = os.pread(descriptor, min(READ_CHUNK_BYTES, size - offset), offset)
            if not chunk:
                raise ValueError(f"{label} became truncated while it was snapshotted")
            view = memoryview(chunk)
            while view:
                written = os.write(target_descriptor, view)
                if written <= 0:
                    raise OSError(f"could not snapshot {label}")
                view = view[written:]
            offset += len(chunk)
        os.fchmod(target_descriptor, 0o500)
        os.fsync(target_descriptor)
    except BaseException:
        os.close(target_descriptor)
        temporary.cleanup()
        raise
    os.close(target_descriptor)
    target_metadata = target.lstat()
    if (
        not stat.S_ISREG(target_metadata.st_mode)
        or target_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(target_metadata.st_mode) != 0o500
        or target_metadata.st_size != fingerprint[4]
    ):
        temporary.cleanup()
        raise ValueError(f"{label} snapshot does not have exact private metadata")
    return temporary, target


def canonical_nonzero_sha256(value: object, label: str) -> str:
    """Return one canonical nonzero lowercase SHA-256 string."""

    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{64}", value) is None
        or value == "0" * 64
    ):
        raise ValueError(f"{label} is not a canonical nonzero SHA-256")
    return value


def checked_declared_artifact_total(declared_artifacts: dict[str, int]) -> int:
    """Validate each exact artifact size and its aggregate release inventory."""

    total = 0
    for name in ARTIFACTS:
        size_bytes = declared_artifacts[name]
        if size_bytes <= 0 or size_bytes > MAX_DECLARED_ARTIFACT_FILE_BYTES:
            raise ValueError(
                f"artifact {name} violates its "
                f"{MAX_DECLARED_ARTIFACT_FILE_BYTES}-byte size limit"
            )
        total += size_bytes
        if total > MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES:
            raise ValueError(
                "declared artifacts exceed the "
                f"{MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES}-byte aggregate limit"
            )
    return total


def checked_catalog_aggregate_total(current: int, release_bytes: int) -> int:
    """Mirror the runtime's non-raiseable whole-catalog byte ceiling."""

    if current < 0 or release_bytes < 0:
        raise ValueError("catalog aggregate byte accounting is negative")
    total = current + release_bytes
    if total > MAX_CATALOG_AGGREGATE_BYTES:
        raise ValueError(
            "artifact catalog exceeds the runtime aggregate byte limit of "
            f"{MAX_CATALOG_AGGREGATE_BYTES}"
        )
    return total


def require(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {needle!r}")


def require_pattern(
    text: str,
    relative: str,
    errors: list[str],
    pattern: str,
    description: str,
) -> None:
    if re.search(pattern, text, flags=re.DOTALL) is None:
        errors.append(f"{relative}: missing {description}")


def forbid(text: str, relative: str, errors: list[str], *needles: str) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: retired corridor remains: {needle!r}")


def forbid_merge_conflict_markers(
    text: str, relative: str, errors: list[str]
) -> None:
    """Reject unresolved Git conflict markers in every reviewed source."""

    if re.search(r"(?m)^(?:<<<<<<<(?: .*)?|=======|>>>>>>>(?: .*)?)$", text):
        errors.append(f"{relative}: unresolved Git merge conflict marker")


def static_errors(overrides: dict[str, str] | None = None) -> list[str]:
    errors: list[str] = []
    overrides = overrides or {}
    texts = {
        path: overrides.get(path, read(path, errors))
        for path in (
            READINESS,
            PRIVACY,
            PRIVACY_PROTOCOL,
            BRIDGE,
            HEADER,
            CORE,
            STEP_TRANSITION,
            RECURSIVE_BACKEND,
            RECURSION_ADAPTER,
            VALUE_CONTRACT,
            SCHEMA_GOLDEN,
            CONFIG,
            NODE,
            KAGAMI,
            BUNDLE,
            ROUTES,
            WORKFLOW,
            IOS_EVIDENCE_MODULE,
            PRODUCTION_IOS_EVIDENCE_MODULE,
        )
    }
    texts[MODEL] = read_reviewed_model(errors, overrides)
    texts[CATALOG] = read_reviewed_catalog(errors, overrides)
    for relative, text in texts.items():
        forbid_merge_conflict_markers(text, relative, errors)
    model = texts[MODEL]
    require(
        model,
        MODEL,
        errors,
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v4"',
        '"iroha.reviewed-source-closure.v1"',
        "reviewed_source_closure_descriptor_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4: [&str; 8]",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
        "pub enum KagemushaPastaCycleArtifactKindV4",
        "ParamsIpa",
        "BootstrapWitness",
        "KagemushaRecursiveSpendReleaseActivationV4",
        "kagemusha_recursive_spend_verifier_key_id_v4",
    )
    forbid(
        model,
        MODEL,
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    forbid(
        "\n".join(
            texts[path]
            for path in (
                BRIDGE,
                CORE,
                STEP_TRANSITION,
                RECURSIVE_BACKEND,
                VALUE_CONTRACT,
                SCHEMA_GOLDEN,
            )
        ),
        "Rust ABI-21/V4 corridor",
        errors,
        *RETIRED_RECURSIVE_LIFECYCLE_TYPES,
        *RETIRED_RECURSIVE_V3_MARKERS,
    )
    for artifact in ARTIFACTS:
        if model.count(f'"{artifact}"') != 1:
            errors.append(f"{MODEL}: exact-eight artifact {artifact!r} must be declared once")
    availability = re.search(
        r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE:\s*bool\s*=\s*"
        r'cfg!\(feature\s*=\s*"kagemusha-production-enabled"\)\s*;',
        model,
    )
    if availability is None:
        errors.append(
            f"{MODEL}: production availability must be controlled only by the "
            "kagemusha-production-enabled feature"
        )

    require(
        texts[PRIVACY],
        PRIVACY,
        errors,
        'include!("privacy/protocol.rs");',
    )
    require(
        texts[PRIVACY_PROTOCOL],
        PRIVACY_PROTOCOL,
        errors,
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;",
    )
    require(
        texts[BRIDGE],
        BRIDGE,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "promotion_record_norito_ptr",
        "KagemushaRecursiveSpendReleaseRecordV4",
        ".authenticate(&trusted_policy)",
        "self.promotion_record",
        "validate_against_authenticated_release",
        "require_kagemusha_recursive_spend_production_promotion_v4()?",
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
        "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
        "installed.validate_live_inventory()?",
        "KagemushaQualifiedArtifactSourceV4",
        "qualify_kagemusha_authenticated_artifact_source_v4(",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source(",
        "KagemushaPastaCycleOpaqueProverV4::from_qualified_artifact_source(",
        "from_candidate_artifact_spool_loader(",
        "fn candidate_proving_key_spool(",
        "fn runtime_verifier(",
        "fn runtime_prover(",
        "recursive_spend_v4_prover_and_terminal_verifier_lifetimes_do_not_overlap",
        '"authenticated-v4-artifact-installation"',
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "KagemushaRecursiveSpendRedemptionChangePrepareRequestV4",
        "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
    )
    require(
        texts[HEADER],
        HEADER,
        errors,
        "CONNECT_NORITO_BRIDGE_ABI_VERSION 22",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "promotion_record_norito_ptr",
    )
    forbid(
        texts[BRIDGE] + texts[HEADER],
        f"{BRIDGE} / {HEADER}",
        errors,
        "kagemusha_recursive_spend_artifact_begin_v3",
        "kagemusha_recursive_spend_artifact_set_install_v3",
        "kagemusha_recursive_spend_init_v3",
        "kagemusha_recursive_spend_append_v3",
    )

    require(
        texts[CATALOG],
        CATALOG,
        errors,
        "pub struct KagemushaReleaseCatalogV4",
        "pub fn load(policy_path: &Path, artifact_dir: &Path)",
        "exactly eight artifacts",
        "KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source",
        "DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4",
        "const MAX_CATALOG_AGGREGATE_BYTES_V4: u64 = 12 * 1024 * 1024 * 1024;",
        "KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4",
    )
    runtime_profile_validation = texts[RECURSION_ADAPTER].split(
        "fn validate_kagemusha_profile_protocol_v4<C>(", 1
    )[-1].split("fn terminal_validate_kagemusha_eq_bootstrap_v4(", 1)[0]
    forbid(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "keygen_vk",
        "kagemusha_bootstrap_verifying_key_v1",
        "validate_bootstrap_protocol",
    )
    require(
        runtime_profile_validation,
        "runtime Kagemusha protocol validation",
        errors,
        "kagemusha_compiled_protocol_structure_sha256",
        "KagemushaStepBootstrapV4::decode_authenticated",
    )
    require_pattern(
        texts[CATALOG],
        CATALOG,
        errors,
        (
            r"const\s+KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4:\s*usize\s*=\s*"
            r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4\.len\(\)\s*;\s*"
            r"[\s\S]*?"
            r"if\s+manifest\s*"
            r"\.profiles\s*\.iter\(\)\s*"
            r"\.map\(\|profile\|\s*profile\.artifacts\.len\(\)\)\s*"
            r"\.sum::<usize>\(\)\s*"
            r"!=\s*KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4\s*\{"
        ),
        "exact-eight manifest inventory check",
    )
    forbid(
        texts[CATALOG] + texts[CORE] + texts[NODE] + texts[KAGAMI],
        "configured V4 runtime",
        errors,
        "IROHA_KAGEMUSHA_RELEASE_TRUST_ROOT_NORITO_HEX",
        "kagemusha_enabled",
    )
    require(
        texts[KAGAMI],
        KAGAMI,
        errors,
        "fn configured_policy_bytes(path: &Path)",
        'decode_canonical_norito(&configured, "configured Kagemusha V4 release policy")',
        "KagemushaAuthenticatedReleaseV4::verify",
        "KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4",
        "if expected.len() != 17",
        "ActivateKagemushaRecursiveReleaseV4::new(activation, policy)",
        r'instruction_count\":1',
    )
    require_pattern(
        texts[KAGAMI],
        KAGAMI,
        errors,
        (
            r"fn verify_exact_inventory_v4\(.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"if expected\.len\(\) != 17.*?"
            r"fn recursive_step_verifier_commitment_v4\("
        ),
        "function-scoped 17-file verifier inventory including the qualification receipt",
    )
    require(
        texts[BUNDLE],
        BUNDLE,
        errors,
        "const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 17;",
        "fn final_release_inventory_v4() -> BTreeSet<String>",
        "KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4",
        "if expected.len() != FINAL_RELEASE_INVENTORY_COUNT_V4",
        "fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()",
    )
    require_pattern(
        texts[BUNDLE],
        BUNDLE,
        errors,
        (
            r"fn final_release_inventory_v4\(\).*?\.chain\(\[.*?"
            r"KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.*?"
            r"\]\).*?\.collect\(\).*?impl PublicationDirectory"
        ),
        "function-scoped 17-file producer inventory including the qualification receipt",
    )
    require_pattern(
        texts[MODEL],
        MODEL,
        errors,
        (
            r"pub const KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4: usize\s*=\s*"
            r"2 \* KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize "
            r"\+ 16 \* 1024;"
        ),
        "qualification receipt bound derived from two absolute proof pairs plus framing",
    )
    require_pattern(
        texts[MODEL],
        MODEL,
        errors,
        (
            r"pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32\s*=\s*"
            r"384 \* 1024;"
        ),
        "384 KiB absolute V4 proof-pair bound",
    )
    opaque_metadata_section = texts[READINESS].split(
        "BOUNDED_AUTHENTICATED_METADATA = (", 1
    )[-1].split("READ_CHUNK_BYTES =", 1)[0]
    if "recursive-step-two-qualification-v4.norito" in opaque_metadata_section:
        errors.append(
            f"{READINESS}: opaque qualification receipt is routed through textual evidence scanning"
        )
    verifier_function = texts[READINESS].rsplit(
        "def release_verifier_command(", 1
    )[-1].split("def validate_kagami_verification_report(", 1)[0]
    require(
        texts[READINESS],
        READINESS,
        errors,
        'KAGAMI_VERIFIER_PATH_ENV = "KAGEMUSHA_V4_KAGAMI_BIN"',
        'KAGAMI_VERIFIER_SHA256_ENV = "KAGEMUSHA_V4_KAGAMI_SHA256"',
        "hash_pinned_descriptor(",
        "def validate_kagami_verification_report(",
        "env=SANITIZED_VERIFIER_ENV",
        'cwd=Path("/")',
        "validate_kagami_verification_report(",
        "promotion requires signed physical-iOS raw evidence",
        "def load_ios_evidence_validator(",
        "read_pinned_descriptor(",
        "PRODUCTION_TRUSTED_UID = 0",
        "def require_production_root_custody(",
        "production promotion must run as root",
        "def snapshot_private_bytes(",
        "evidence_bytes_are_non_placeholder(",
        "trusted_python_sha256 = sys.argv[4]",
        "running promotion Python differs from its trusted SHA-256",
        "def validate_inherited_promotion_python(",
        "inherited promotion Python differs from its trusted SHA-256",
        "def validate_inherited_promotion_gate(",
        "inherited promotion gate differs from its reviewed SHA-256",
        'KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256',
        "PROMOTION_STAGING_PARENT = Path(",
        "authenticate_reviewed_source_file(",
        "validate_source_trust_projection(",
        "isolated_source_trust_git_config(",
        "SOURCE_ALLOWED_SIGNERS_PATH_ENV",
        "SOURCE_REVOCATION_PATH_ENV",
        "SOURCE_SEAL_PROJECTION_PATH_ENV",
        "trusted_source_helper_snapshot",
        "trusted_ios_validator_snapshot",
        "PRODUCTION_IOS_EVIDENCE_MODULE",
        "validate_production_signed_evidence",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY",
        "static candidate corridor passed;",
        "production promotion was not evaluated.",
    )
    require(
        texts[PRODUCTION_IOS_EVIDENCE_MODULE],
        PRODUCTION_IOS_EVIDENCE_MODULE,
        errors,
        "iroha.kagemusha.ios_device_lab.production_signed_evidence.v1",
        "iroha.kagemusha.ios.production_device_policy.v1",
        "def validate_production_signed_evidence(",
        "PLATFORM_TRUST_BLOCKER",
        "Apple X.509 chain",
        "freshness/replay state",
    )
    production_ios_validation = texts[PRODUCTION_IOS_EVIDENCE_MODULE].rsplit(
        "def validate_production_signed_evidence(", 1
    )[-1]
    require_pattern(
        production_ios_validation,
        PRODUCTION_IOS_EVIDENCE_MODULE,
        errors,
        (
            r"errors\.append\(PLATFORM_TRUST_BLOCKER\)\s*"
            r"return errors\s*$"
        ),
        "unconditional production App Attest trust blocker",
    )
    shell_bootstrap = texts[READINESS].split("<<'PY'", 1)[0]
    require(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        'SCRIPT_SOURCE_ORIGINAL="${BASH_SOURCE[0]}"',
        "builtin pwd -P",
        "promotion_assert_root_custody",
        'promotion_assert_root_custody "${DERIVED_ROOT_DIR}" "promotion readiness checkout"',
        'promotion_assert_root_custody "${SCRIPT_PATH}" "promotion readiness gate"',
        'exec 8<"${SCRIPT_PATH}"',
        '"/dev/fd/${GATE_PIN_FD}"',
        'exec 9<"${PYTHON_BIN}"',
        '"/dev/fd/${PYTHON_PIN_FD}"',
        '"${PYTHON_PIN_FD}" "${PYTHON_PATH_FINGERPRINT}"',
        "promotion Python interpreter changed before execution",
        "rejects missing or symlinked script invocation",
        "from an independently authenticated launcher/controller",
        'PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-python3}"',
        "sys.version_info >= (3, 10)",
        "requires Python 3.10 or newer",
        '/usr/bin/git -C "${ROOT_DIR}" diff --quiet --diff-filter=U --',
        "readiness rejects unresolved Git index entries",
    )
    forbid(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        "$(dirname ",
        "`dirname ",
        "readlink ",
    )
    require_pattern(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        (
            r"promotion_assert_root_custody \"\$\{PYTHON_BIN\}\""
            r".*?PYTHON_PATH_FINGERPRINT=.*?exec 9<\"\$\{PYTHON_BIN\}\""
            r".*?/dev/fd/\$\{PYTHON_PIN_FD\}.*?"
            r"promotion Python interpreter changed before execution.*?"
            r"promotion_assert_root_custody \"\$\{PYTHON_BIN\}\""
        ),
        "pre-exec Python descriptor custody",
    )
    require_pattern(
        shell_bootstrap,
        "promotion shell bootstrap",
        errors,
        (
            r"promotion_assert_root_custody \"\$\{DERIVED_ROOT_DIR\}\""
            r".*?promotion_assert_root_custody \"\$\{SCRIPT_PATH\}\""
            r".*?KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256"
            r".*?exec 8<\"\$\{SCRIPT_PATH\}\""
            r".*?/dev/fd/\$\{GATE_PIN_FD\}.*?OBSERVED_GATE_SHA256"
            r".*?!=.*?GATE_SHA256.*?"
            r"differs from its independently reviewed SHA-256.*?"
            r"promotion_assert_root_custody \"\$\{SCRIPT_PATH\}\""
        ),
        "independently pinned root-custodied gate bootstrap",
    )
    forbid(
        verifier_function,
        "promotion verifier command",
        errors,
        '"cargo"',
        '"run"',
    )
    ios_validator_function = texts[READINESS].rsplit(
        "def verify_ios_evidence(", 1
    )[-1].split("def promotion_errors(", 1)[0]
    require_pattern(
        ios_validator_function,
        "physical-iOS evidence verification",
        errors,
        (
            r"validation_errors\s*=\s*validator\(\s*evidence_snapshot_path,"
            r".*?trusted_public_key_snapshot,\s*"
            r"trusted_production_policy_snapshot,\s*\).*?"
            r"evidence\s*=\s*strict_json_bytes\(\s*evidence_bytes,"
        ),
        "same pinned evidence, trusted key, and production policy snapshots for validation and digest binding",
    )
    forbid(
        ios_validator_function,
        "physical-iOS evidence verification",
        errors,
        "subprocess.run",
        "sys.executable",
        "check_kagemusha_candidate_ios_evidence.py",
        "validator(evidence_path",
    )
    promotion_function = texts[READINESS].rsplit("def promotion_errors(", 1)[-1].split(
        "errors = static_errors()", 1
    )[0]
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"ios_configuration\s*=\s*ios_evidence_configuration\(errors\)"
            r".*?authenticate_reviewed_source_file\(\s*PRODUCTION_IOS_EVIDENCE_MODULE,"
            r".*?load_ios_evidence_validator\(\s*validator_bytes,"
        ),
        "fail-closed production iOS evidence validator path",
    )
    ios_loader_function = texts[READINESS].rsplit(
        "def load_ios_evidence_validator(", 1
    )[-1].split("def verify_ios_evidence(", 1)[0]
    require_pattern(
        ios_loader_function,
        READINESS,
        errors,
        (
            r"production_validator\s*=\s*production_module\.__dict__\.get\(\s*"
            r'"validate_production_signed_evidence"\s*\)'
        ),
        "production-only iOS evidence validator entrypoint",
    )
    if promotion_function.count("require_production_root_custody(") < 17:
        errors.append(
            f"{READINESS}: promotion does not root-custody every production trust class"
        )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"production_roots\s*=\s*\[.*?PROMOTION_STAGING_PARENT.*?\]"
            r".*?snapshot_pinned_executable\(.*?PROMOTION_STAGING_PARENT"
        ),
        "fixed pinned production staging parent",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"production_roots\s*=\s*\[\s*root,\s*source_helper_path\.parent,"
            r"\s*ios_validator_path\.parent,.*?"
            r"reviewed promotion readiness gate.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"MAX_READINESS_GATE_BYTES.*?trusted_gate_sha256"
        ),
        "retained root-custodied reviewed gate and checkout",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"authenticate_reviewed_source_file\(\s*SOURCE_TREE_SEAL,"
            r".*?snapshot_private_bytes\(\s*source_helper_bytes,"
            r".*?str\(trusted_source_helper_snapshot\)"
        ),
        "source-closure-authenticated source-tree helper snapshot",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"source SSH allowed-signers policy.*?"
            r"require_production_root_custody\(descriptor, label\).*?"
            r"allowed_signers_sha256.*?snapshot_private_bytes\(.*?"
            r"source SSH revocation policy.*?allow_empty=True.*?"
            r"revocation_sha256.*?snapshot_private_bytes\(.*?"
            r"authenticated source-seal projection.*?"
            r"source_projection_sha256.*?validate_source_trust_projection\(.*?"
            r"isolated_source_trust_git_config\(.*?\.gitconfig.*?"
            r'"HOME": str\(trusted_source_trust_home\)'
        ),
        "closure-bound snapshotted source SSH trust policies",
    )
    forbid(
        promotion_function,
        "promotion source SSH trust bootstrap",
        errors,
        '"HOME": "/var/empty"',
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"authenticate_reviewed_source_file\(\s*IOS_EVIDENCE_MODULE,"
            r".*?snapshot_private_bytes\(\s*validator_bytes,"
            r".*?authenticate_reviewed_source_file\(\s*PRODUCTION_IOS_EVIDENCE_MODULE,"
            r".*?snapshot_private_bytes\(\s*production_validator_bytes,"
            r".*?load_ios_evidence_validator\(\s*validator_bytes,"
            r"\s*trusted_ios_validator_snapshot,\s*production_validator_bytes,"
        ),
        "source-closure-authenticated candidate and production iOS validator snapshots",
    )
    snapshot_functions = texts[READINESS].split(
        "def snapshot_private_bytes(", 1
    )[-1].split("def canonical_nonzero_sha256(", 1)[0]
    if snapshot_functions.count("dir=staging_parent") != 2:
        errors.append(
            f"{READINESS}: promotion snapshots do not use only their explicit staging parent"
        )
    verifier_environment = texts[READINESS].split(
        "SANITIZED_VERIFIER_ENV = {", 1
    )[-1].split("READ_CHUNK_BYTES =", 1)[0]
    if (
        verifier_environment.count('"TMPDIR": str(PROMOTION_STAGING_PARENT),')
        != 1
        or promotion_function.count(
            '"TMPDIR": str(PROMOTION_STAGING_PARENT),'
        )
        != 1
    ):
        errors.append(
            f"{READINESS}: promotion subprocesses do not use only the fixed staging parent"
        )
    forbid(
        promotion_function,
        "promotion catalog byte custody",
        errors,
        "read_regular_bounded(",
        "inspect_regular_prefix(",
        "strict_json(",
        ".read_bytes()",
    )
    require_pattern(
        promotion_function,
        READINESS,
        errors,
        (
            r"promotion Python runtime.*?hash_pinned_descriptor\(.*?\)"
            r"\s*!=\s*trusted_python_sha256"
        ),
        "running promotion interpreter digest revalidation",
    )
    require(
        texts[CONFIG] + texts[NODE] + texts[CORE],
        "configured V4 runtime",
        errors,
        "kagemusha_release_policy_path",
        "kagemusha_artifact_dir",
        "KagemushaReleaseCatalogV4::load",
        "ensure_kagemusha_active_release_material_v4",
    )
    require(
        texts[CORE],
        CORE,
        errors,
        "impl Execute for ActivateKagemushaRecursiveReleaseV4",
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
        "validate_offline_attestation_policy_for_release_activation",
        "self.device_attestation_policy",
        "impl Execute for TopUpKagemushaRecursiveV4",
        "impl Execute for RedeemKagemushaRecursiveV4",
        "issuance_active_at",
    )
    require_pattern(
        texts[CORE],
        CORE,
        errors,
        (
            r"let\s+change_release\s*=\s*request\s*\.offline_change\s*\.as_ref\(\)"
            r".*?\.transpose\(\)\?\s*;\s*"
            r"if\s+change_release\.as_ref\(\)\.is_some_and\(\|release\|\s*\{\s*"
            r"!\s*release\s*\.cached\s*"
            r"\.issuance_active_at\(state_transaction\.block_height\(\)\)"
        ),
        "offline-change withdrawal-height issuance check",
    )
    for route in ROUTE_LITERALS:
        if route not in texts[ROUTES]:
            errors.append(f"{ROUTES}: stable route changed or disappeared: {route}")
    require(
        texts[WORKFLOW],
        WORKFLOW,
        errors,
        "check_kagemusha_production_readiness.sh candidate",
        "check_kagemusha_production_readiness.sh candidate --self-test",
        "ci/check_kagemusha_recursive_spend_python_sdk.sh --self-test",
        "check_kagemusha_recursive_spend_v4_sdk_contract.sh",
        '"crates/iroha_core/src/smartcontracts/isi/offline/**"',
        '"specs/sdk/swift/readiness/*kagemusha*.md"',
        "scripts/tests/build_kagemusha_v4_candidate_bundle_test.py",
        "scripts/tests/check_kagemusha_candidate_ios_evidence_test.py",
        "scripts/tests/kagemusha_source_tree_seal_test.py",
        "scripts/tests/kagemusha_staged_resource_guard_test.py",
        "scripts/tests/stage_kagemusha_candidate_android_artifacts_test.py",
        "scripts/tests/stage_kagemusha_candidate_android_lab_test.py",
        "pytests/scripts/run_kagemusha_v4_generation_test.py",
        "pytests/scripts/run_kagemusha_v4_generation_benchmark_test.py",
        "cargo test -p iroha_core kagemusha_v4 --lib",
        "cargo test -p iroha_core offline_device_attestation_policy --lib",
        "cargo test -p iroha_core device_registration_ --lib",
        "cargo test -p iroha_core --features \"dev-tools,zk-halo2-ipa,kagemusha-candidate-evidence-lab\" --bin kagemusha_recursive_spend_v4_bundle final_release_inventory_is_exact_and_includes_recursive_qualification_receipt",
        "cargo test -p iroha_core sparse_confidential_subtree_roots_match_dense_reference --lib",
        "cargo test -p iroha_core next_zero_confidential_path_matches_padded_tree_path --lib",
        "cargo test -p iroha_core sequential_append_paths --lib",
        "cargo test -p iroha_core recursive_state_vector_is_exact_and_zero_padded --lib",
        "cargo test -p iroha_core output_membership --lib",
        "cargo test -p iroha_core v4_eq_frontier_copy_constraints --lib",
        "cargo test -p iroha_core v4_manifest_preserves_exact_little_endian_state_limbs --lib",
        "cargo test -p iroha_core v4_eq_and_ep_public_columns_share_the_v2_result_frontier_limb --lib",
        "cargo test -p iroha_core kagemusha_terminal_registry_v4 --lib",
        "cargo test -p iroha_kagami --bin kagami harden_private_tree",
        "cargo test -p iroha_kagami --bin kagami private_custody_readme_invokes_non_executable_scripts_through_bash",
        "cargo test -p iroha_kagami --bin kagami raw_npos_genesis_receives_the_chain_bound_localnet_epoch_seed",
        "cargo test -p iroha_kagami --bin kagami atomic_activation_policy_",
        "cargo test -p iroha_kagami --bin kagami atomic_activation_rejects_noncanonical_app_policy_text",
        "cargo test -p iroha_torii readiness_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii offline_commands --lib -- --nocapture",
        "cargo test -p iroha_config settlement_offline_tests -- --nocapture",
        "cargo test -p iroha_config torii_kagemusha_commands_tests -- --nocapture",
        "cargo test -p connect_norito_bridge recursive_spend_v4",
        "cargo test -p connect_norito_bridge output_membership_local_carrier --lib",
    )
    return errors


def strict_json_bytes(payload: bytes, label: str) -> dict[str, object]:
    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    value = json.loads(
        payload.decode("utf-8"),
        object_pairs_hook=object_pairs,
        parse_constant=lambda value: (_ for _ in ()).throw(
            ValueError(f"{label} contains non-finite value {value!r}")
        ),
    )
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def reviewed_source_commit_from_closure(payload: bytes) -> tuple[dict[str, object], str]:
    """Parse the independently pinned closure enough to authenticate its helpers."""

    closure = strict_json_bytes(payload, "reviewed source closure")
    if set(closure) != REVIEWED_SOURCE_CLOSURE_KEYS:
        raise ValueError("reviewed source closure fields are not exact")
    if (
        closure.get("schema") != "iroha.reviewed-source-closure.v1"
        or closure.get("source_repo_dirty") is not False
    ):
        raise ValueError("reviewed source closure is not one clean V1 closure")
    source_commit = closure.get("source_commit")
    if (
        not isinstance(source_commit, str)
        or re.fullmatch(r"[0-9a-f]{40}", source_commit) is None
        or source_commit == "0" * 40
    ):
        raise ValueError("reviewed source closure commit is not canonical")
    return closure, source_commit


def validate_source_trust_projection(
    payload: bytes,
    reviewed_closure_bytes: bytes,
    reviewed_closure_sha256: str,
    source_commit: str,
    allowed_signers_sha256: str,
    revocation_sha256: str,
) -> None:
    """Bind the exact promotion SSH policies to the reviewed source closure."""

    projection = strict_json_bytes(payload, "authenticated source-seal projection")
    if set(projection) != SOURCE_SEAL_PROJECTION_KEYS:
        raise ValueError("authenticated source-seal projection fields are not exact")
    if (
        projection.get("schema")
        != "iroha.kagemusha.authenticated_source_seal_projection.v1"
        or projection.get("source_repo_dirty") is not False
        or projection.get("source_commit") != source_commit
        or projection.get("reviewed_source_closure_sha256")
        != reviewed_closure_sha256
        or projection.get("reviewed_source_closure_hex")
        != reviewed_closure_bytes.hex()
    ):
        raise ValueError(
            "authenticated source-seal projection differs from the reviewed closure"
        )
    reviewed_closure, _ = reviewed_source_commit_from_closure(
        reviewed_closure_bytes
    )
    if projection.get("source_tree_sha256") != reviewed_closure.get(
        "source_tree_sha256"
    ):
        raise ValueError(
            "authenticated source-seal projection tree differs from the reviewed closure"
        )
    authority = projection.get("source_authority")
    if not isinstance(authority, dict) or set(authority) != SOURCE_AUTHORITY_KEYS:
        raise ValueError("authenticated source authority fields are not exact")
    signature = authority.get("signature")
    if not isinstance(signature, dict) or set(signature) != SOURCE_SIGNATURE_KEYS:
        raise ValueError("authenticated source signature fields are not exact")
    if (
        authority.get("commit") != source_commit
        or signature.get("mechanism") != "git-commit-ssh-signature-v1"
        or signature.get("signature_namespace") != "git"
        or signature.get("allowed_signers_sha256") != allowed_signers_sha256
        or signature.get("revocation_sha256") != revocation_sha256
    ):
        raise ValueError(
            "promotion SSH trust-policy digests differ from the reviewed source closure projection"
        )
    canonical_nonzero_sha256(
        signature.get("public_key_sha256"),
        "authenticated source SSH public key",
    )


def isolated_source_trust_git_config(
    allowed_signers: Path, revocation: Path
) -> bytes:
    """Create the sole global Git config visible to the reviewed source helper."""

    for path, label in (
        (allowed_signers, "allowed-signers snapshot"),
        (revocation, "revocation snapshot"),
    ):
        path_text = str(path)
        if (
            not path.is_absolute()
            or path.resolve(strict=False) != path
            or re.fullmatch(r"/[A-Za-z0-9._/-]+", path_text) is None
        ):
            raise ValueError(f"{label} path is not safe for isolated Git config")
    return (
        '[gpg "ssh"]\n'
        f"\tallowedSignersFile = {allowed_signers}\n"
        f"\trevocationFile = {revocation}\n"
    ).encode("utf-8")


def source_git_environment() -> dict[str, str]:
    """Return the fixed environment for closure-commit blob authentication."""

    return {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_LITERAL_PATHSPECS": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_PAGER": "cat",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PAGER": "cat",
        "PATH": "/usr/bin:/bin",
        "TZ": "UTC",
    }


def authenticate_reviewed_source_file(
    relative: str,
    observed_bytes: bytes,
    source_commit: str,
    maximum_bytes: int,
) -> None:
    """Match one pinned worktree helper to its exact closure-commit blob."""

    if (
        not relative
        or relative.startswith("/")
        or "\\" in relative
        or any(component in {"", ".", ".."} for component in relative.split("/"))
    ):
        raise ValueError("reviewed helper path is not canonical")
    authenticated = subprocess.run(
        [
            str(SOURCE_GIT),
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.excludesFile=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
            str(root),
            "cat-file",
            "blob",
            f"{source_commit}:{relative}",
        ],
        cwd=Path("/"),
        env=source_git_environment(),
        stdin=subprocess.DEVNULL,
        check=False,
        capture_output=True,
        close_fds=True,
    )
    if (
        authenticated.returncode != 0
        or authenticated.stderr
        or not authenticated.stdout
        or len(authenticated.stdout) > maximum_bytes
    ):
        raise ValueError(f"could not authenticate reviewed helper {relative}")
    if authenticated.stdout != observed_bytes:
        raise ValueError(f"reviewed helper {relative} differs from the source closure")


def release_verifier_command(verifier: Path, directory: Path, policy: Path) -> list[str]:
    """Use one explicitly digest-pinned Kagami verifier for promotion decisions."""
    return [
        str(verifier),
        "kagemusha",
        "verify-release-v4",
        "--bundle-dir",
        str(directory),
        "--release-policy",
        str(policy),
        "--benchmark-evidence",
        str(directory / "physical-device-benchmark.evidence"),
        "--cryptographic-review",
        str(directory / "cryptographic-review.evidence"),
    ]


def validate_kagami_verification_report(
    report: dict[str, object],
    *,
    directory: Path,
    manifest: dict[str, object],
    policy_sha256: str,
    promotion_record_sha256: str,
    qualification_receipt_sha256: str,
    ios_candidate_sha256: str,
) -> None:
    """Authenticate the complete machine report emitted by the pinned verifier."""

    exact_keys = {
        "status",
        "envelope_sha256",
        "manifest_body_sha256",
        "candidate_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "promotion_record_sha256",
        "release_policy_sha256",
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "network_id",
        "asset_definition_id",
        "asset_scale",
        "bridge_abi_version",
        "recursive_step_verifier_commitment",
        "artifacts",
    }
    if set(report) != exact_keys:
        raise ValueError("Kagami verification report fields are not exact")
    if report.get("status") != "verified" or report.get("bridge_abi_version") != 22:
        raise ValueError("Kagami did not report one verified native-ABI-22 release")
    if report.get("envelope_sha256") != directory.name:
        raise ValueError("Kagami manifest envelope differs from the release directory")
    if report.get("release_policy_sha256") != policy_sha256:
        raise ValueError("Kagami verified a different release policy")
    if report.get("promotion_record_sha256") != promotion_record_sha256:
        raise ValueError("Kagami reconstructed a different promotion record")
    if report.get("qualification_receipt_sha256") != qualification_receipt_sha256:
        raise ValueError("Kagami verified a different recursive qualification receipt")
    if report.get("candidate_sha256") != ios_candidate_sha256:
        raise ValueError(
            "signed physical-iOS candidate differs from Kagami's reconstructed candidate"
        )
    for field in (
        "manifest_body_sha256",
        "candidate_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "promotion_record_sha256",
        "release_policy_sha256",
        "recursive_step_verifier_commitment",
    ):
        canonical_nonzero_sha256(report.get(field), f"Kagami report {field}")
    manifest_equal_fields = (
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
        "network_id",
        "asset_scale",
    )
    for field in manifest_equal_fields:
        if report.get(field) != manifest.get(field):
            raise ValueError(f"Kagami report {field} differs from the manifest")
    manifest_asset = manifest.get("asset")
    if isinstance(manifest_asset, str) and report.get("asset_definition_id") != manifest_asset:
        raise ValueError("Kagami report asset differs from the manifest")
    if report.get("qualified_candidate_sha256") != manifest.get(
        "qualified_candidate_sha256"
    ):
        raise ValueError("Kagami qualified candidate differs from the manifest")

    expected_artifacts: list[dict[str, object]] = []
    profiles = manifest.get("profiles")
    if not isinstance(profiles, list):
        raise ValueError("manifest profiles are not an array")
    flattened: list[dict[str, object]] = []
    for profile in profiles:
        if not isinstance(profile, dict) or not isinstance(profile.get("artifacts"), list):
            raise ValueError("manifest proof profile is malformed")
        for artifact in profile["artifacts"]:
            if not isinstance(artifact, dict):
                raise ValueError("manifest artifact is malformed")
            flattened.append(artifact)
    if len(flattened) != len(REPORT_ARTIFACT_PURPOSES):
        raise ValueError("manifest does not contain the exact report artifact set")
    for purpose, artifact in zip(REPORT_ARTIFACT_PURPOSES, flattened, strict=True):
        expected_artifacts.append(
            {
                "purpose": purpose,
                "file_name": artifact.get("file_name"),
                "size_bytes": artifact.get("size_bytes"),
                "sha256": artifact.get("sha256"),
                "payload_size_bytes": artifact.get("payload_size_bytes"),
                "payload_sha256": artifact.get("payload_sha256"),
            }
        )
    roster = manifest.get("topup_finality_roster_artifact")
    if not isinstance(roster, dict):
        raise ValueError("manifest top-up finality roster binding is malformed")
    expected_artifacts.append(
        {
            "purpose": "topup_finality_roster",
            "file_name": roster.get("file_name"),
            "size_bytes": roster.get("size_bytes"),
            "sha256": roster.get("sha256"),
            "payload_size_bytes": None,
            "payload_sha256": None,
        }
    )
    artifacts = report.get("artifacts")
    if artifacts != expected_artifacts:
        raise ValueError("Kagami report artifact inventory differs from the manifest")


def ios_evidence_configuration(
    errors: list[str],
) -> tuple[Path, str, Path, Path] | None:
    """Return the complete opt-in physical-iOS evidence configuration."""

    root_text = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT", "")
    key_id = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID", "")
    public_key_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY", ""
    )
    production_policy_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY", ""
    )
    present = tuple(
        bool(value)
        for value in (
            root_text,
            key_id,
            public_key_text,
            production_policy_text,
        )
    )
    if not any(present):
        errors.append(
            "promotion requires signed production physical-iOS raw evidence, trusted key id, "
            "public key, and production policy"
        )
        return None
    if not all(present):
        errors.append(
            "physical-iOS evidence requires KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT, "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY together"
        )
        return None
    ios_root = Path(root_text)
    public_key = Path(public_key_text)
    production_policy = Path(production_policy_text)
    if (
        not ios_root.is_absolute()
        or ios_root.resolve(strict=False) != ios_root
        or not ios_root.is_dir()
        or ios_root.is_symlink()
    ):
        errors.append("physical-iOS evidence root must be a canonical absolute real directory")
        return None
    if (
        not public_key.is_absolute()
        or public_key.resolve(strict=False) != public_key
        or not public_key.is_file()
        or public_key.is_symlink()
    ):
        errors.append("physical-iOS trusted public key must be a canonical absolute regular file")
        return None
    if (
        not production_policy.is_absolute()
        or production_policy.resolve(strict=False) != production_policy
        or not production_policy.is_file()
        or production_policy.is_symlink()
        or production_policy.stat().st_size == 0
        or production_policy.stat().st_size > 1024 * 1024
    ):
        errors.append(
            "physical-iOS production policy must be a canonical absolute bounded regular file"
        )
        return None
    return ios_root, key_id, public_key, production_policy


def load_ios_evidence_validator(
    candidate_module_bytes: bytes,
    candidate_module_path: Path,
    production_module_bytes: bytes,
    production_module_path: Path,
) -> Callable[[Path, Path, str, Path, Path], list[str]]:
    """Load both reviewed validators from already pinned source bytes."""

    module_name = "_iroha_pinned_kagemusha_candidate_ios_evidence"
    module = types.ModuleType(module_name)
    module.__file__ = str(candidate_module_path)
    module.__package__ = ""
    sys.modules[module_name] = module
    production_name = "_iroha_pinned_kagemusha_production_ios_evidence"
    production_module = types.ModuleType(production_name)
    production_module.__file__ = str(production_module_path)
    production_module.__package__ = ""
    sys.modules[production_name] = production_module
    try:
        code = compile(
            candidate_module_bytes,
            str(candidate_module_path),
            "exec",
            dont_inherit=True,
        )
        exec(code, module.__dict__)
        production_code = compile(
            production_module_bytes,
            str(production_module_path),
            "exec",
            dont_inherit=True,
        )
        exec(production_code, production_module.__dict__)
        production_validator = production_module.__dict__.get(
            "validate_production_signed_evidence"
        )
        if not callable(production_validator):
            raise ValueError(
                "pinned production physical-iOS validator has no maintained entrypoint"
            )

        def validate(
            evidence_path: Path,
            artifact_root: Path,
            trusted_key_id: str,
            trusted_public_key_path: Path,
            production_policy_path: Path,
        ) -> list[str]:
            return production_validator(
                evidence_path,
                artifact_root,
                trusted_key_id,
                trusted_public_key_path,
                production_policy_path,
                module,
            )

        return validate
    except BaseException:
        sys.modules.pop(module_name, None)
        sys.modules.pop(production_name, None)
        raise


def verify_ios_evidence(
    directory: Path,
    ios_configuration: tuple[Path, str, Path, Path],
    validator: Callable[[Path, Path, str, Path, Path], list[str]],
    evidence_bytes: bytes,
    trusted_public_key_snapshot: Path,
    trusted_production_policy_snapshot: Path,
    directory_pins: list[tuple[Path, int, tuple[int, ...], str]],
    staging_parent: Path,
) -> tuple[str | None, str | None]:
    """Verify one signed raw slot from the exact bytes used for its candidate digest."""

    ios_root, key_id, _, _ = ios_configuration
    release_root = ios_root / directory.name
    raw_root = release_root / "raw"
    if (
        not release_root.is_dir()
        or release_root.is_symlink()
        or not raw_root.is_dir()
        or raw_root.is_symlink()
    ):
        return None, (
            f"{directory.name}: physical-iOS evidence must use "
            f"{ios_root}/<manifest-sha256>/raw"
        )
    try:
        # Retain the externally named roots through the whole promotion.  The
        # reviewed validator snapshots every exact raw file, validates it, and
        # performs a full identity/content rescan before returning.  Root-only
        # custody then protects that authenticated inventory after the rescan.
        for path, label in (
            (release_root, f"physical-iOS release directory {release_root}"),
            (raw_root, f"physical-iOS raw directory {raw_root}"),
        ):
            descriptor, fingerprint = pin_directory_metadata(path, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            directory_pins.append((path, descriptor, fingerprint, label))
        evidence_snapshot, evidence_snapshot_path = snapshot_private_bytes(
            evidence_bytes,
            "physical-device-benchmark.evidence",
            "signed physical-iOS evidence",
            staging_parent,
        )
        try:
            validation_errors = validator(
                evidence_snapshot_path,
                raw_root,
                key_id,
                trusted_public_key_snapshot,
                trusted_production_policy_snapshot,
            )
        finally:
            evidence_snapshot.cleanup()
        if validation_errors:
            return None, (
                f"{directory.name}: physical-iOS evidence verification failed: "
                f"{validation_errors[-1]}"
            )
        evidence = strict_json_bytes(
            evidence_bytes,
            "signed physical-iOS evidence",
        )
        artifact_digests = evidence.get("artifact_digests")
        if not isinstance(artifact_digests, dict):
            raise ValueError("artifact_digests is not an object")
        candidate = artifact_digests.get("input/candidate-v4.norito")
        if not isinstance(candidate, dict):
            raise ValueError("candidate artifact binding is missing")
        candidate_sha256 = candidate.get("sha256")
        if (
            not isinstance(candidate_sha256, str)
            or re.fullmatch(r"[0-9a-f]{64}", candidate_sha256) is None
            or candidate_sha256 == "0" * 64
        ):
            raise ValueError("candidate artifact digest is not canonical")
        if evidence.get("release_manifest_sha256") != directory.name:
            raise ValueError(
                "production iOS evidence release manifest digest does not match catalog"
            )
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        return None, f"{directory.name}: invalid signed physical-iOS evidence: {error}"
    return candidate_sha256, None


def promotion_errors() -> list[str]:
    errors: list[str] = []
    if os.geteuid() != PRODUCTION_TRUSTED_UID:
        return [
            "production promotion must run as root so its custody policy matches the runtime"
        ]
    try:
        validate_inherited_promotion_gate()
    except (OSError, ValueError) as error:
        return [f"promotion gate bootstrap custody failed: {error}"]
    try:
        validate_inherited_promotion_python()
    except (OSError, ValueError) as error:
        return [f"promotion Python pre-exec custody failed: {error}"]
    policy_text = os.environ.get("KAGEMUSHA_V4_RELEASE_POLICY_PATH", "")
    artifact_text = os.environ.get("KAGEMUSHA_V4_ARTIFACT_ROOT", "")
    if not policy_text or not artifact_text:
        return [
            "promotion requires KAGEMUSHA_V4_RELEASE_POLICY_PATH and KAGEMUSHA_V4_ARTIFACT_ROOT"
        ]
    policy = Path(policy_text)
    artifact_root = Path(artifact_text)
    verifier_text = os.environ.get(KAGAMI_VERIFIER_PATH_ENV, "")
    verifier_sha256 = os.environ.get(KAGAMI_VERIFIER_SHA256_ENV, "")
    verifier = Path(verifier_text) if verifier_text else None
    if (
        not policy.is_absolute()
        or not artifact_root.is_absolute()
        or policy.resolve(strict=False) != policy
        or artifact_root.resolve(strict=False) != artifact_root
    ):
        errors.append("promotion policy and artifact root must be canonical absolute paths")
    if (
        not policy.is_file()
        or policy.is_symlink()
        or policy.stat().st_size == 0
        or policy.stat().st_size > 64 * 1024
    ):
        errors.append("promotion policy must be a nonempty regular file")
    if not artifact_root.is_dir() or artifact_root.is_symlink():
        errors.append("promotion artifact root must be a real directory")
        return errors
    if (
        verifier is None
        or not verifier.is_absolute()
        or verifier.resolve(strict=False) != verifier
        or not verifier.is_file()
        or verifier.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", verifier_sha256) is None
        or verifier_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires a canonical absolute digest-pinned Kagami executable via "
            f"{KAGAMI_VERIFIER_PATH_ENV} and {KAGAMI_VERIFIER_SHA256_ENV}"
        )
        return errors
    ios_configuration = ios_evidence_configuration(errors)

    source_identity: dict[str, object] | None = None
    reviewed_closure_text = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE", ""
    )
    reviewed_closure_sha256 = os.environ.get(
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256", ""
    )
    reviewed_closure = Path(reviewed_closure_text) if reviewed_closure_text else None
    if (
        not reviewed_closure_text
        or reviewed_closure is None
        or not reviewed_closure.is_absolute()
        or reviewed_closure.resolve(strict=False) != reviewed_closure
        or not reviewed_closure.is_file()
        or reviewed_closure.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", reviewed_closure_sha256) is None
        or reviewed_closure_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires a canonical absolute independently pinned reviewed "
            "source-closure path and SHA-256"
        )
    allowed_signers_text = os.environ.get(SOURCE_ALLOWED_SIGNERS_PATH_ENV, "")
    allowed_signers_sha256 = os.environ.get(SOURCE_ALLOWED_SIGNERS_SHA256_ENV, "")
    revocation_text = os.environ.get(SOURCE_REVOCATION_PATH_ENV, "")
    revocation_sha256 = os.environ.get(SOURCE_REVOCATION_SHA256_ENV, "")
    source_projection_text = os.environ.get(SOURCE_SEAL_PROJECTION_PATH_ENV, "")
    source_projection_sha256 = os.environ.get(
        SOURCE_SEAL_PROJECTION_SHA256_ENV, ""
    )
    allowed_signers = Path(allowed_signers_text) if allowed_signers_text else None
    revocation = Path(revocation_text) if revocation_text else None
    source_projection = (
        Path(source_projection_text) if source_projection_text else None
    )
    if (
        allowed_signers is None
        or not allowed_signers.is_absolute()
        or allowed_signers.resolve(strict=False) != allowed_signers
        or not allowed_signers.is_file()
        or allowed_signers.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", allowed_signers_sha256) is None
        or allowed_signers_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires one canonical root-custodied digest-pinned SSH allowed-signers policy"
        )
    if (
        revocation is None
        or not revocation.is_absolute()
        or revocation.resolve(strict=False) != revocation
        or not revocation.is_file()
        or revocation.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", revocation_sha256) is None
        or revocation_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires one canonical root-custodied digest-pinned SSH revocation policy (an explicitly pinned empty file means no revocations)"
        )
    if (
        source_projection is None
        or not source_projection.is_absolute()
        or source_projection.resolve(strict=False) != source_projection
        or not source_projection.is_file()
        or source_projection.is_symlink()
        or re.fullmatch(r"[0-9a-f]{64}", source_projection_sha256) is None
        or source_projection_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires the canonical independently digest-pinned authenticated source-seal projection"
        )

    catalog_directory_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    trusted_file_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    policy_sha256 = ""
    ios_validator: Callable[[Path, Path, str, Path, Path], list[str]] | None = None
    ios_validator_path = root / IOS_EVIDENCE_MODULE
    production_ios_validator_path = root / PRODUCTION_IOS_EVIDENCE_MODULE
    source_helper_path = root / SOURCE_TREE_SEAL
    verifier_snapshot: tempfile.TemporaryDirectory[str] | None = None
    public_key_snapshot: tempfile.TemporaryDirectory[str] | None = None
    ios_policy_snapshot: tempfile.TemporaryDirectory[str] | None = None
    closure_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_projection_snapshot: tempfile.TemporaryDirectory[str] | None = None
    allowed_signers_snapshot: tempfile.TemporaryDirectory[str] | None = None
    revocation_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_trust_config_snapshot: tempfile.TemporaryDirectory[str] | None = None
    source_helper_snapshot: tempfile.TemporaryDirectory[str] | None = None
    ios_validator_snapshot: tempfile.TemporaryDirectory[str] | None = None
    production_ios_validator_snapshot: tempfile.TemporaryDirectory[str] | None = None
    trusted_public_key_snapshot: Path | None = None
    trusted_ios_policy_snapshot: Path | None = None
    trusted_closure_snapshot: Path | None = None
    trusted_allowed_signers_snapshot: Path | None = None
    trusted_revocation_snapshot: Path | None = None
    trusted_source_trust_home: Path | None = None
    trusted_source_helper_snapshot: Path | None = None
    verifier_exec = verifier

    def cleanup_private_snapshots() -> None:
        if production_ios_validator_snapshot is not None:
            production_ios_validator_snapshot.cleanup()
        if ios_validator_snapshot is not None:
            ios_validator_snapshot.cleanup()
        if source_helper_snapshot is not None:
            source_helper_snapshot.cleanup()
        if source_trust_config_snapshot is not None:
            source_trust_config_snapshot.cleanup()
        if revocation_snapshot is not None:
            revocation_snapshot.cleanup()
        if allowed_signers_snapshot is not None:
            allowed_signers_snapshot.cleanup()
        if source_projection_snapshot is not None:
            source_projection_snapshot.cleanup()
        if closure_snapshot is not None:
            closure_snapshot.cleanup()
        if public_key_snapshot is not None:
            public_key_snapshot.cleanup()
        if ios_policy_snapshot is not None:
            ios_policy_snapshot.cleanup()
        if verifier_snapshot is not None:
            verifier_snapshot.cleanup()

    try:
        python_runtime = Path(sys.executable)
        if (
            not python_runtime.is_absolute()
            or python_runtime.resolve(strict=False) != python_runtime
            or not python_runtime.is_file()
            or python_runtime.is_symlink()
        ):
            raise ValueError("promotion Python runtime path is not canonical")
        if (
            not PROMOTION_STAGING_PARENT.is_absolute()
            or PROMOTION_STAGING_PARENT.resolve(strict=False)
            != PROMOTION_STAGING_PARENT
            or not PROMOTION_STAGING_PARENT.is_dir()
            or PROMOTION_STAGING_PARENT.is_symlink()
        ):
            raise ValueError(
                f"fixed promotion staging parent is unavailable: {PROMOTION_STAGING_PARENT}"
            )
        if not SOURCE_GIT.is_file() or SOURCE_GIT.is_symlink():
            raise ValueError("fixed source-authentication Git is unavailable")
        seen_directories: set[Path] = set()
        production_roots = [
            root,
            source_helper_path.parent,
            ios_validator_path.parent,
            production_ios_validator_path.parent,
            artifact_root,
            policy.parent,
            verifier.parent,
            python_runtime.parent,
            PROMOTION_STAGING_PARENT,
            SOURCE_GIT.parent,
        ]
        if reviewed_closure is not None:
            production_roots.append(reviewed_closure.parent)
        if source_projection is not None:
            production_roots.append(source_projection.parent)
        if allowed_signers is not None:
            production_roots.append(allowed_signers.parent)
        if revocation is not None:
            production_roots.append(revocation.parent)
        if ios_configuration is not None:
            production_roots.extend(
                [
                    ios_configuration[0],
                    ios_configuration[2].parent,
                    ios_configuration[3].parent,
                ]
            )
        production_directory_paths = {
            path
            for trusted_root in production_roots
            for path in absolute_directory_chain(trusted_root)
        }
        trusted_roots = [
            *production_roots,
            source_helper_path.parent,
            ios_validator_path.parent,
            production_ios_validator_path.parent,
        ]
        for trusted_root in trusted_roots:
            for path in absolute_directory_chain(trusted_root):
                if path in seen_directories:
                    continue
                seen_directories.add(path)
                label = f"trusted release path component {path}"
                descriptor, fingerprint = pin_directory_metadata(path, label)
                if path in production_directory_paths:
                    try:
                        require_production_root_custody(descriptor, label)
                    except BaseException:
                        os.close(descriptor)
                        raise
                catalog_directory_pins.append((path, descriptor, fingerprint, label))
        gate_path = root / READINESS
        label = f"reviewed promotion readiness gate {gate_path}"
        descriptor, fingerprint = pin_regular_metadata(gate_path, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_READINESS_GATE_BYTES, label
            )
            != trusted_gate_sha256
        ):
            os.close(descriptor)
            raise ValueError("promotion gate differs from its reviewed SHA-256")
        trusted_file_pins.append((gate_path, descriptor, fingerprint, label))
        label = f"release policy {policy}"
        descriptor, fingerprint = pin_regular_metadata(policy, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        trusted_file_pins.append((policy, descriptor, fingerprint, label))
        policy_sha256 = hash_pinned_descriptor(
            descriptor, fingerprint, 64 * 1024, label
        )
        label = f"Kagami release verifier {verifier}"
        descriptor, fingerprint = pin_regular_metadata(verifier, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if fingerprint[4] > MAX_KAGAMI_VERIFIER_BYTES or not fingerprint[3] & 0o111:
            os.close(descriptor)
            raise ValueError(
                "Kagami release verifier must be executable and within its size limit"
            )
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label
            )
            != verifier_sha256
        ):
            os.close(descriptor)
            raise ValueError("Kagami release verifier differs from its trusted SHA-256")
        trusted_file_pins.append((verifier, descriptor, fingerprint, label))
        verifier_snapshot, verifier_exec = snapshot_pinned_executable(
            descriptor, fingerprint, label, PROMOTION_STAGING_PARENT
        )
        snapshot_root = verifier_exec.parent
        snapshot_label = f"private Kagami verifier snapshot directory {snapshot_root}"
        snapshot_descriptor, snapshot_fingerprint = pin_directory_metadata(
            snapshot_root, snapshot_label
        )
        catalog_directory_pins.append(
            (
                snapshot_root,
                snapshot_descriptor,
                snapshot_fingerprint,
                snapshot_label,
            )
        )
        snapshot_label = f"private Kagami verifier snapshot {verifier_exec}"
        snapshot_descriptor, snapshot_fingerprint = pin_regular_metadata(
            verifier_exec, snapshot_label
        )
        if (
            hash_pinned_descriptor(
                snapshot_descriptor,
                snapshot_fingerprint,
                MAX_KAGAMI_VERIFIER_BYTES,
                snapshot_label,
            )
            != verifier_sha256
        ):
            os.close(snapshot_descriptor)
            raise ValueError("private Kagami verifier snapshot digest changed")
        trusted_file_pins.append(
            (
                verifier_exec,
                snapshot_descriptor,
                snapshot_fingerprint,
                snapshot_label,
            )
        )
        label = f"promotion Python runtime {python_runtime}"
        descriptor, fingerprint = pin_regular_metadata(
            python_runtime, label, require_single_link=False
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if (
            hash_pinned_descriptor(
                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label
            )
            != trusted_python_sha256
        ):
            os.close(descriptor)
            raise ValueError(
                "running promotion Python differs from its trusted SHA-256"
            )
        trusted_file_pins.append((python_runtime, descriptor, fingerprint, label))
        label = f"source-authentication Git {SOURCE_GIT}"
        descriptor, fingerprint = pin_regular_metadata(
            SOURCE_GIT, label, require_single_link=False
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        if not fingerprint[3] & 0o111:
            os.close(descriptor)
            raise ValueError("source-authentication Git is not executable")
        trusted_file_pins.append((SOURCE_GIT, descriptor, fingerprint, label))
        reviewed_closure_bytes: bytes | None = None
        reviewed_source_commit: str | None = None
        if reviewed_closure is not None:
            label = f"reviewed source closure {reviewed_closure}"
            descriptor, fingerprint = pin_regular_metadata(reviewed_closure, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            reviewed_closure_bytes = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_REVIEWED_SOURCE_CLOSURE_BYTES,
                label,
            )
            if hashlib.sha256(reviewed_closure_bytes).hexdigest() != reviewed_closure_sha256:
                os.close(descriptor)
                raise ValueError("reviewed source closure differs from its trusted SHA-256")
            trusted_file_pins.append(
                (reviewed_closure, descriptor, fingerprint, label)
            )
            _, reviewed_source_commit = reviewed_source_commit_from_closure(
                reviewed_closure_bytes
            )
            closure_snapshot, trusted_closure_snapshot = snapshot_private_bytes(
                reviewed_closure_bytes,
                "reviewed-source-closure.json",
                "reviewed source closure",
                PROMOTION_STAGING_PARENT,
            )
        if (
            reviewed_closure_bytes is None
            or reviewed_source_commit is None
            or allowed_signers is None
            or revocation is None
            or source_projection is None
        ):
            raise ValueError("reviewed source trust inputs are incomplete")
        label = f"source SSH allowed-signers policy {allowed_signers}"
        descriptor, fingerprint = pin_regular_metadata(allowed_signers, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        allowed_signers_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_ALLOWED_SIGNERS_BYTES,
            label,
        )
        if hashlib.sha256(allowed_signers_bytes).hexdigest() != allowed_signers_sha256:
            os.close(descriptor)
            raise ValueError("SSH allowed-signers policy differs from its trusted SHA-256")
        trusted_file_pins.append((allowed_signers, descriptor, fingerprint, label))
        allowed_signers_snapshot, trusted_allowed_signers_snapshot = (
            snapshot_private_bytes(
                allowed_signers_bytes,
                "allowed-signers",
                "source SSH allowed-signers policy",
                PROMOTION_STAGING_PARENT,
            )
        )
        label = f"source SSH revocation policy {revocation}"
        descriptor, fingerprint = pin_regular_metadata(
            revocation, label, allow_empty=True
        )
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        revocation_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_REVOCATION_BYTES,
            label,
            allow_empty=True,
        )
        if hashlib.sha256(revocation_bytes).hexdigest() != revocation_sha256:
            os.close(descriptor)
            raise ValueError("SSH revocation policy differs from its trusted SHA-256")
        trusted_file_pins.append((revocation, descriptor, fingerprint, label))
        revocation_snapshot, trusted_revocation_snapshot = snapshot_private_bytes(
            revocation_bytes,
            "revocation",
            "source SSH revocation policy",
            PROMOTION_STAGING_PARENT,
            allow_empty=True,
        )
        label = f"authenticated source-seal projection {source_projection}"
        descriptor, fingerprint = pin_regular_metadata(source_projection, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        source_projection_bytes = read_pinned_descriptor(
            descriptor,
            fingerprint,
            MAX_SOURCE_SEAL_PROJECTION_BYTES,
            label,
        )
        if hashlib.sha256(source_projection_bytes).hexdigest() != source_projection_sha256:
            os.close(descriptor)
            raise ValueError(
                "authenticated source-seal projection differs from its trusted SHA-256"
            )
        trusted_file_pins.append((source_projection, descriptor, fingerprint, label))
        source_projection_snapshot, _ = snapshot_private_bytes(
            source_projection_bytes,
            "authenticated-source-seal-projection.json",
            "authenticated source-seal projection",
            PROMOTION_STAGING_PARENT,
        )
        validate_source_trust_projection(
            source_projection_bytes,
            reviewed_closure_bytes,
            reviewed_closure_sha256,
            reviewed_source_commit,
            allowed_signers_sha256,
            revocation_sha256,
        )
        if (
            trusted_allowed_signers_snapshot is None
            or trusted_revocation_snapshot is None
        ):
            raise ValueError("source SSH trust-policy snapshots are incomplete")
        source_trust_config = isolated_source_trust_git_config(
            trusted_allowed_signers_snapshot,
            trusted_revocation_snapshot,
        )
        source_trust_config_snapshot, trusted_source_trust_config = (
            snapshot_private_bytes(
                source_trust_config,
                ".gitconfig",
                "isolated source SSH trust Git config",
                PROMOTION_STAGING_PARENT,
            )
        )
        trusted_source_trust_home = trusted_source_trust_config.parent
        label = f"reviewed source-tree seal helper {source_helper_path}"
        descriptor, fingerprint = pin_regular_metadata(source_helper_path, label)
        try:
            require_production_root_custody(descriptor, label)
        except BaseException:
            os.close(descriptor)
            raise
        source_helper_bytes = read_pinned_descriptor(
            descriptor, fingerprint, MAX_REVIEWED_HELPER_BYTES, label
        )
        trusted_file_pins.append((source_helper_path, descriptor, fingerprint, label))
        if reviewed_source_commit is None:
            raise ValueError("reviewed source closure cannot authenticate its helper")
        authenticate_reviewed_source_file(
            SOURCE_TREE_SEAL,
            source_helper_bytes,
            reviewed_source_commit,
            MAX_REVIEWED_HELPER_BYTES,
        )
        source_helper_snapshot, trusted_source_helper_snapshot = snapshot_private_bytes(
            source_helper_bytes,
            "kagemusha_source_tree_seal.py",
            "reviewed source-tree seal helper",
            PROMOTION_STAGING_PARENT,
        )
        if (
            trusted_closure_snapshot is None
            or trusted_source_helper_snapshot is None
            or trusted_source_trust_home is None
        ):
            raise ValueError("reviewed source helper snapshots are incomplete")
        source_identity_result = subprocess.run(
            [
                str(python_runtime),
                "-I",
                str(trusted_source_helper_snapshot),
                "identity",
                "--root",
                str(root),
                "--reviewed-source-closure",
                str(trusted_closure_snapshot),
                "--reviewed-source-closure-sha256",
                reviewed_closure_sha256,
            ],
            cwd=Path("/"),
            env={
                "HOME": str(trusted_source_trust_home),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TMPDIR": str(PROMOTION_STAGING_PARENT),
                "TZ": "UTC",
            },
            stdin=subprocess.DEVNULL,
            check=False,
            capture_output=True,
            close_fds=True,
        )
        if source_identity_result.returncode != 0:
            raise ValueError(
                "promotion source differs from the independently pinned reviewed closure"
            )
        parsed_identity = strict_json_bytes(
            source_identity_result.stdout, "promotion reviewed source identity"
        )
        if (
            set(parsed_identity)
            != {
                "reviewed_source_closure",
                "reviewed_source_closure_descriptor_sha256",
                "schema",
                "source_commit",
                "source_repo_dirty",
                "source_tree_sha256",
            }
            or parsed_identity.get("schema")
            != "iroha.kagemusha.reviewed_source_tree_identity.v1"
            or parsed_identity.get("source_repo_dirty") is not False
            or parsed_identity.get("source_commit") != reviewed_source_commit
            or parsed_identity.get("reviewed_source_closure_descriptor_sha256")
            != reviewed_closure_sha256
            or parsed_identity.get("reviewed_source_closure")
            != strict_json_bytes(
                reviewed_closure_bytes, "reviewed source closure"
            )
        ):
            raise ValueError("promotion reviewed source identity is not exact")
        source_identity = parsed_identity
        if ios_configuration is not None:
            public_key = ios_configuration[2]
            label = f"physical-iOS trusted public key {public_key}"
            descriptor, fingerprint = pin_regular_metadata(public_key, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            if fingerprint[4] > 64 * 1024:
                os.close(descriptor)
                raise ValueError("physical-iOS trusted public key is oversized")
            public_key_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 64 * 1024, label
            )
            trusted_file_pins.append((public_key, descriptor, fingerprint, label))
            public_key_snapshot, trusted_public_key_snapshot = snapshot_private_bytes(
                public_key_bytes,
                "trusted-physical-ios-public-key.pem",
                "physical-iOS trusted public key",
                PROMOTION_STAGING_PARENT,
            )
            production_ios_policy = ios_configuration[3]
            label = f"physical-iOS production policy {production_ios_policy}"
            descriptor, fingerprint = pin_regular_metadata(
                production_ios_policy, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            production_ios_policy_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 1024 * 1024, label
            )
            trusted_file_pins.append(
                (production_ios_policy, descriptor, fingerprint, label)
            )
            ios_policy_snapshot, trusted_ios_policy_snapshot = snapshot_private_bytes(
                production_ios_policy_bytes,
                "production-ios-policy-v1.json",
                "physical-iOS production policy",
                PROMOTION_STAGING_PARENT,
            )
            label = f"reviewed physical-iOS evidence validator {ios_validator_path}"
            descriptor, fingerprint = pin_regular_metadata(ios_validator_path, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            validator_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 4 * 1024 * 1024, label
            )
            trusted_file_pins.append(
                (ios_validator_path, descriptor, fingerprint, label)
            )
            authenticate_reviewed_source_file(
                IOS_EVIDENCE_MODULE,
                validator_bytes,
                reviewed_source_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
            ios_validator_snapshot, trusted_ios_validator_snapshot = (
                snapshot_private_bytes(
                    validator_bytes,
                    "kagemusha_candidate_ios_evidence.py",
                    "reviewed physical-iOS evidence validator",
                    PROMOTION_STAGING_PARENT,
                )
            )
            label = (
                "reviewed production physical-iOS evidence validator "
                f"{production_ios_validator_path}"
            )
            descriptor, fingerprint = pin_regular_metadata(
                production_ios_validator_path, label
            )
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            production_validator_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 4 * 1024 * 1024, label
            )
            trusted_file_pins.append(
                (production_ios_validator_path, descriptor, fingerprint, label)
            )
            authenticate_reviewed_source_file(
                PRODUCTION_IOS_EVIDENCE_MODULE,
                production_validator_bytes,
                reviewed_source_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
            (
                production_ios_validator_snapshot,
                trusted_production_ios_validator_snapshot,
            ) = snapshot_private_bytes(
                production_validator_bytes,
                "kagemusha_production_ios_evidence.py",
                "reviewed production physical-iOS evidence validator",
                PROMOTION_STAGING_PARENT,
            )
            ios_validator = load_ios_evidence_validator(
                validator_bytes,
                trusted_ios_validator_snapshot,
                production_validator_bytes,
                trusted_production_ios_validator_snapshot,
            )
    except Exception as error:
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        cleanup_private_snapshots()
        errors.append(f"promotion release trust path is not pinned: {error}")
        return errors

    authenticated_verification_allowed = not errors
    directories = []
    for path in artifact_root.iterdir():
        directories.append(path)
        if len(directories) > MAX_RELEASE_DIRECTORIES:
            errors.append(
                f"promotion artifact root exceeds {MAX_RELEASE_DIRECTORIES} releases"
            )
            for _, descriptor, _, _ in catalog_directory_pins:
                os.close(descriptor)
            for _, descriptor, _, _ in trusted_file_pins:
                os.close(descriptor)
            cleanup_private_snapshots()
            return errors
    directories.sort()
    if not directories:
        errors.append("promotion artifact root contains no manifest-digest releases")
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        cleanup_private_snapshots()
        return errors
    if ios_configuration is not None:
        ios_root = ios_configuration[0]
        ios_directories = []
        for path in ios_root.iterdir():
            ios_directories.append(path)
            if len(ios_directories) > MAX_RELEASE_DIRECTORIES:
                errors.append(
                    "physical-iOS evidence root exceeds "
                    f"{MAX_RELEASE_DIRECTORIES} releases"
                )
                for _, descriptor, _, _ in catalog_directory_pins:
                    os.close(descriptor)
                for _, descriptor, _, _ in trusted_file_pins:
                    os.close(descriptor)
                cleanup_private_snapshots()
                return errors
        if {path.name for path in ios_directories} != {
            path.name for path in directories
        }:
            errors.append(
                "physical-iOS evidence root must contain exactly one "
                "manifest-digest directory for every promoted release"
            )
            for _, descriptor, _, _ in catalog_directory_pins:
                os.close(descriptor)
            for _, descriptor, _, _ in trusted_file_pins:
                os.close(descriptor)
            cleanup_private_snapshots()
            return errors
    expected_inventory = set(ARTIFACTS + FINAL_METADATA)
    catalog_aggregate_bytes = 0
    catalog_pins = trusted_file_pins
    for directory in directories:
        directory_error_count = len(errors)
        ios_candidate_sha256: str | None = None
        promotion_record_sha256: str | None = None
        qualification_receipt_sha256: str | None = None
        if not directory.is_dir() or directory.is_symlink() or not re.fullmatch(r"[0-9a-f]{64}", directory.name):
            errors.append(f"noncanonical release entry: {directory.name}")
            continue
        try:
            label = f"release directory {directory}"
            descriptor, fingerprint = pin_directory_metadata(directory, label)
            try:
                require_production_root_custody(descriptor, label)
            except BaseException:
                os.close(descriptor)
                raise
            catalog_directory_pins.append((directory, descriptor, fingerprint, label))
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: release directory is not pinned: {error}")
            continue
        actual = set()
        for path in directory.iterdir():
            actual.add(path.name)
            if len(actual) > MAX_RELEASE_INVENTORY_ENTRIES:
                errors.append(f"{directory.name}: final release inventory is oversized")
                break
        if actual != expected_inventory:
            errors.append(f"{directory.name}: final release inventory is not exact")
            continue
        new_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
        release_pins: dict[str, tuple[int, tuple[int, ...], str]] = {}
        try:
            release_bytes = 0
            for name in expected_inventory:
                path = directory / name
                label = f"{directory.name}/{name}"
                descriptor, fingerprint = pin_regular_metadata(path, label)
                try:
                    require_production_root_custody(descriptor, label)
                except BaseException:
                    os.close(descriptor)
                    raise
                new_pins.append((path, descriptor, fingerprint, label))
                release_pins[name] = (descriptor, fingerprint, label)
                release_bytes += fingerprint[4]
            catalog_aggregate_bytes = checked_catalog_aggregate_total(
                catalog_aggregate_bytes, release_bytes
            )
        except (OSError, ValueError) as error:
            for _, descriptor, _, _ in new_pins:
                os.close(descriptor)
            errors.append(f"{directory.name}: invalid catalog byte inventory: {error}")
            continue
        catalog_pins.extend(new_pins)
        try:
            descriptor, fingerprint, label = release_pins["manifest.norito"]
            manifest_bytes = read_pinned_descriptor(
                descriptor, fingerprint, MAX_MANIFEST_BYTES, label
            )
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest.norito: {error}")
            continue
        digest = hashlib.sha256(manifest_bytes).hexdigest()
        if digest != directory.name:
            errors.append(f"{directory.name}: directory does not equal manifest SHA-256")
        try:
            descriptor, fingerprint, label = release_pins[
                "manifest.norito.sha256"
            ]
            sidecar = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_DIGEST_SIDECAR_BYTES,
                label,
            )
            if sidecar != f"{digest}\n".encode("ascii"):
                errors.append(f"{directory.name}: manifest digest sidecar is not canonical")
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest digest sidecar: {error}")
        try:
            descriptor, fingerprint, label = release_pins["manifest.json"]
            manifest = strict_json_bytes(
                read_pinned_descriptor(
                    descriptor, fingerprint, MAX_MANIFEST_BYTES, label
                ),
                label,
            )
        except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"{directory.name}: invalid manifest JSON: {error}")
            continue
        if manifest.get("schema") != "kagemusha.offline.recursive_spend.artifact_manifest.v4":
            errors.append(f"{directory.name}: manifest schema is not V4")
        if manifest.get("bridge_abi_version") != 22 or manifest.get("source_repo_dirty") is not False:
            errors.append(f"{directory.name}: ABI/source-tree promotion binding is invalid")
        for field in (
            "authenticated_source_seal_projection_sha256",
            "reviewed_cargo_binary_sha256",
            "reviewed_rustc_binary_sha256",
        ):
            try:
                canonical_nonzero_sha256(
                    manifest.get(field), f"{directory.name} manifest {field}"
                )
            except ValueError as error:
                errors.append(str(error))
        if source_identity is not None and (
            manifest.get("authenticated_source_seal_projection_sha256")
            != source_projection_sha256
        ):
            errors.append(
                f"{directory.name}: manifest differs from the authenticated source-seal projection"
            )
        if source_identity is not None and (
            manifest.get("source_commit") != source_identity.get("source_commit")
            or manifest.get("source_tree_sha256")
            != source_identity.get("source_tree_sha256")
            or manifest.get("reviewed_source_closure")
            != source_identity.get("reviewed_source_closure")
            or manifest.get("reviewed_source_closure_descriptor_sha256")
            != source_identity.get("reviewed_source_closure_descriptor_sha256")
        ):
            errors.append(
                f"{directory.name}: manifest differs from the pinned reviewed source closure"
            )
        profiles = manifest.get("profiles")
        roles = []
        if isinstance(profiles, list):
            for profile in profiles:
                if isinstance(profile, dict) and isinstance(profile.get("artifacts"), list):
                    roles.extend(profile["artifacts"])
        if len(roles) != 8:
            errors.append(f"{directory.name}: manifest does not bind exactly eight artifacts")
        declared_artifacts: dict[str, int] = {}
        for role in roles:
            if not isinstance(role, dict):
                continue
            name = role.get("file_name")
            size_bytes = role.get("size_bytes")
            if isinstance(name, str) and isinstance(size_bytes, int) and not isinstance(size_bytes, bool):
                declared_artifacts[name] = size_bytes
        if set(declared_artifacts) != set(ARTIFACTS):
            errors.append(f"{directory.name}: manifest artifact names are not exact")
        else:
            try:
                checked_declared_artifact_total(declared_artifacts)
            except ValueError as error:
                errors.append(f"{directory.name}: {error}")
            else:
                for name in ARTIFACTS:
                    try:
                        descriptor, fingerprint, label = release_pins[name]
                        prefix = inspect_pinned_prefix(
                            descriptor,
                            fingerprint,
                            declared_artifacts[name],
                            MAX_DECLARED_ARTIFACT_FILE_BYTES,
                            8,
                            label,
                        )
                        if prefix != b"KRV4KEY\0":
                            errors.append(f"{directory.name}/{name}: invalid KRV4 framing")
                    except (OSError, ValueError) as error:
                        errors.append(f"{directory.name}/{name}: invalid artifact: {error}")
        for name, maximum in BOUNDED_AUTHENTICATED_METADATA:
            try:
                descriptor, fingerprint, label = release_pins[name]
                payload = read_pinned_descriptor(
                    descriptor, fingerprint, maximum, label
                )
                if name == "promotion-record-v4.norito":
                    promotion_record_sha256 = hashlib.sha256(payload).hexdigest()
            except (OSError, ValueError) as error:
                errors.append(f"{directory.name}/{name}: invalid evidence: {error}")
        evidence_bytes: bytes | None = None
        try:
            descriptor, fingerprint, label = release_pins[
                "physical-device-benchmark.evidence"
            ]
            evidence_bytes = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_BENCHMARK_EVIDENCE_BYTES,
                label,
            )
            if not evidence_bytes_are_non_placeholder(evidence_bytes):
                errors.append(
                    f"{directory.name}/physical-device-benchmark.evidence: "
                    "missing non-placeholder evidence bytes"
                )
        except (OSError, ValueError) as error:
            errors.append(
                f"{directory.name}/physical-device-benchmark.evidence: "
                f"invalid evidence: {error}"
            )
        try:
            # This is opaque proof-bearing Norito, not human-authored evidence.
            # Bound and pin it here; Kagami performs canonical authentication.
            descriptor, fingerprint, label = release_pins[
                "recursive-step-two-qualification-v4.norito"
            ]
            receipt = read_pinned_descriptor(
                descriptor,
                fingerprint,
                MAX_QUALIFICATION_RECEIPT_BYTES,
                label,
            )
            qualification_receipt_sha256 = hashlib.sha256(receipt).hexdigest()
        except (OSError, ValueError) as error:
            errors.append(
                f"{directory.name}/recursive-step-two-qualification-v4.norito: "
                f"invalid qualification receipt: {error}"
            )
        if (
            ios_configuration is not None
            and ios_validator is not None
            and evidence_bytes is not None
            and trusted_public_key_snapshot is not None
            and trusted_ios_policy_snapshot is not None
            and len(errors) == directory_error_count
        ):
            ios_candidate_sha256, ios_error = verify_ios_evidence(
                directory,
                ios_configuration,
                ios_validator,
                evidence_bytes,
                trusted_public_key_snapshot,
                trusted_ios_policy_snapshot,
                catalog_directory_pins,
                PROMOTION_STAGING_PARENT,
            )
            if ios_error is not None:
                errors.append(ios_error)
        if authenticated_verification_allowed and len(errors) == directory_error_count:
            if (
                ios_candidate_sha256 is None
                or promotion_record_sha256 is None
                or qualification_receipt_sha256 is None
            ):
                errors.append(
                    f"{directory.name}: authenticated verification inputs are incomplete"
                )
                continue
            command = release_verifier_command(verifier_exec, directory, policy)
            verified = subprocess.run(
                command,
                cwd=Path("/"),
                env=SANITIZED_VERIFIER_ENV,
                stdin=subprocess.DEVNULL,
                check=False,
                capture_output=True,
                text=True,
                close_fds=True,
            )
            if verified.returncode != 0:
                detail = (verified.stderr or verified.stdout).strip().splitlines()
                suffix = f": {detail[-1]}" if detail else ""
                errors.append(
                    f"{directory.name}: authenticated V4 release verification failed{suffix}"
                )
            else:
                try:
                    report = strict_json_bytes(
                        verified.stdout.encode("utf-8"),
                        "Kagami V4 verification report",
                    )
                    validate_kagami_verification_report(
                        report,
                        directory=directory,
                        manifest=manifest,
                        policy_sha256=policy_sha256,
                        promotion_record_sha256=promotion_record_sha256,
                        qualification_receipt_sha256=qualification_receipt_sha256,
                        ios_candidate_sha256=ios_candidate_sha256,
                    )
                except (UnicodeError, ValueError, json.JSONDecodeError) as error:
                    errors.append(
                        f"{directory.name}: authenticated verifier report is invalid: {error}"
                    )
    for path, descriptor, fingerprint, label in catalog_pins:
        try:
            revalidate_pinned_metadata(path, descriptor, fingerprint, label)
        except (OSError, ValueError) as error:
            errors.append(f"invalid catalog byte inventory: {error}")
        finally:
            os.close(descriptor)
    for path, descriptor, fingerprint, label in reversed(catalog_directory_pins):
        try:
            revalidate_pinned_metadata(path, descriptor, fingerprint, label)
        except (OSError, ValueError) as error:
            errors.append(f"invalid catalog directory inventory: {error}")
        finally:
            os.close(descriptor)
    cleanup_private_snapshots()
    return errors


errors = static_errors()
if mode == "promotion":
    errors.extend(promotion_errors())

if self_test:
    try:
        with tempfile.TemporaryDirectory(
            prefix="kagemusha-symlink-invocation-self-test-"
        ) as temporary:
            invocation = Path(temporary) / "readiness-symlink"
            invocation.symlink_to(root / READINESS)
            rejected = subprocess.run(
                ["/bin/bash", str(invocation), "candidate"],
                cwd=Path("/"),
                env={"LANG": "C", "LC_ALL": "C", "PATH": "/untrusted/bin"},
                stdin=subprocess.DEVNULL,
                check=False,
                capture_output=True,
                text=True,
                close_fds=True,
            )
            if (
                rejected.returncode != 2
                or "rejects missing or symlinked script invocation"
                not in rejected.stderr
            ):
                errors.append("self-test failed to reject symlinked gate invocation")
    except OSError as error:
        errors.append(f"symlink invocation self-test failed unexpectedly: {error}")
    try:
        with tempfile.TemporaryDirectory(
            prefix="kagemusha-untrusted-gate-self-test-"
        ) as temporary:
            untrusted_checkout = (
                Path(temporary).resolve(strict=True) / "untrusted-checkout"
            )
            untrusted_ci = untrusted_checkout / "ci"
            untrusted_ci.mkdir(parents=True)
            untrusted_gate = untrusted_ci / Path(READINESS).name
            untrusted_gate.write_bytes((root / READINESS).read_bytes())
            untrusted_gate.chmod(0o700)
            untrusted_ci.chmod(0o700)
            # This stays invalid even when the self-test itself runs as root.
            untrusted_checkout.chmod(0o770)
            rejected = subprocess.run(
                ["/bin/bash", str(untrusted_gate), "promotion"],
                cwd=Path("/"),
                env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                stdin=subprocess.DEVNULL,
                check=False,
                capture_output=True,
                text=True,
                close_fds=True,
            )
            if (
                rejected.returncode != 2
                or "promotion readiness checkout" not in rejected.stderr
                or not any(
                    marker in rejected.stderr
                    for marker in ("not root-owned", "group/world writable")
                )
            ):
                errors.append(
                    "self-test failed to reject a user-controlled promotion gate checkout"
                )
    except OSError as error:
        errors.append(f"untrusted gate self-test failed unexpectedly: {error}")
    try:
        head_result = subprocess.run(
            [str(SOURCE_GIT), "-C", str(root), "rev-parse", "--verify", "HEAD"],
            cwd=Path("/"),
            env=source_git_environment(),
            stdin=subprocess.DEVNULL,
            check=False,
            capture_output=True,
            text=True,
            close_fds=True,
        )
        head_commit = head_result.stdout.strip()
        if (
            head_result.returncode != 0
            or head_result.stderr
            or re.fullmatch(r"[0-9a-f]{40}", head_commit) is None
        ):
            raise ValueError("could not resolve self-test HEAD")
        try:
            authenticate_reviewed_source_file(
                SOURCE_TREE_SEAL,
                b"mutated helper bytes",
                head_commit,
                MAX_REVIEWED_HELPER_BYTES,
            )
        except ValueError as error:
            if "differs from the source closure" not in str(error):
                raise
        else:
            errors.append("self-test failed to reject a source-helper byte mutation")
    except (OSError, ValueError) as error:
        errors.append(f"source-helper authentication self-test failed unexpectedly: {error}")
    system_git_descriptor = -1
    try:
        system_git_descriptor, system_git_fingerprint = pin_regular_metadata(
            SOURCE_GIT,
            "self-test source-authentication Git",
            require_single_link=False,
        )
        require_production_root_custody(
            system_git_descriptor, "self-test source-authentication Git"
        )
        if not system_git_fingerprint[3] & 0o111:
            raise ValueError("fixed source-authentication Git is not executable")
        revalidate_pinned_metadata(
            SOURCE_GIT,
            system_git_descriptor,
            system_git_fingerprint,
            "self-test source-authentication Git",
        )
    except (OSError, ValueError) as error:
        errors.append(f"fixed Git custody self-test failed unexpectedly: {error}")
    finally:
        if system_git_descriptor >= 0:
            os.close(system_git_descriptor)
    try:
        source_commit = "1" * 40
        source_tree_sha256 = "2" * 64
        closure_value: dict[str, object] = {
            "schema": "iroha.reviewed-source-closure.v1",
            "base_commit": source_commit,
            "source_commit": source_commit,
            "source_repo_dirty": False,
            "source_tree_sha256": source_tree_sha256,
            "tracked_binary_diff_sha256": "3" * 64,
            "untracked_file_count": 0,
            "untracked_path_mode_blob_oid_manifest": [],
            "untracked_path_mode_blob_oid_manifest_sha256": "4" * 64,
            "ignored_cargo_lock_size_bytes": 1,
            "ignored_cargo_lock_sha256": "5" * 64,
            "combined_source_fingerprint_sha256": "6" * 64,
        }
        closure_bytes = (
            json.dumps(closure_value, sort_keys=True, separators=(",", ":")) + "\n"
        ).encode("utf-8")
        closure_sha256 = hashlib.sha256(closure_bytes).hexdigest()
        allowed_sha256 = "7" * 64
        revocation_sha256 = "8" * 64
        projection = {
            "build_script_observed": {},
            "outer_policy": {},
            "reviewed_source_closure_hex": closure_bytes.hex(),
            "reviewed_source_closure_sha256": closure_sha256,
            "schema": "iroha.kagemusha.authenticated_source_seal_projection.v1",
            "source_authority": {
                "commit": source_commit,
                "commit_object_sha256": "9" * 64,
                "commit_object_size": 1,
                "committer_epoch": 1,
                "git_tree": "a" * 40,
                "ordered_parents": ["b" * 40],
                "parent_commit": "b" * 40,
                "parent_tree": "c" * 40,
                "signature": {
                    "allowed_signers_sha256": allowed_sha256,
                    "mechanism": "git-commit-ssh-signature-v1",
                    "principal": "reviewer@example.test",
                    "public_key_sha256": "d" * 64,
                    "revocation_sha256": revocation_sha256,
                    "signature_namespace": "git",
                },
            },
            "source_commit": source_commit,
            "source_date_epoch": 1,
            "source_repo_dirty": False,
            "source_tree_sha256": source_tree_sha256,
        }
        projection_bytes = (
            json.dumps(projection, sort_keys=True, separators=(",", ":")) + "\n"
        ).encode("utf-8")
        validate_source_trust_projection(
            projection_bytes,
            closure_bytes,
            closure_sha256,
            source_commit,
            allowed_sha256,
            revocation_sha256,
        )
        try:
            validate_source_trust_projection(
                projection_bytes,
                closure_bytes,
                closure_sha256,
                source_commit,
                "e" * 64,
                revocation_sha256,
            )
        except ValueError as error:
            if "trust-policy digests" not in str(error):
                raise
        else:
            errors.append(
                "self-test failed to bind SSH trust-policy digests to the reviewed closure"
            )
    except (UnicodeError, ValueError, json.JSONDecodeError) as error:
        errors.append(f"source trust-projection self-test failed unexpectedly: {error}")
    try:
        with tempfile.TemporaryDirectory(
            prefix="kagemusha-source-projection-bound-self-test-"
        ) as temporary:
            boundary_root = Path(temporary)
            exact = boundary_root / "exact-projection"
            exact.write_bytes(b"x" * MAX_SOURCE_SEAL_PROJECTION_BYTES)
            descriptor, fingerprint = pin_regular_metadata(
                exact, "self-test exact source projection"
            )
            try:
                payload = read_pinned_descriptor(
                    descriptor,
                    fingerprint,
                    MAX_SOURCE_SEAL_PROJECTION_BYTES,
                    "self-test exact source projection",
                )
            finally:
                os.close(descriptor)
            if len(payload) != MAX_SOURCE_SEAL_PROJECTION_BYTES:
                errors.append("self-test failed at the exact source-projection byte bound")
            oversized = boundary_root / "oversized-projection"
            oversized.write_bytes(b"x" * (MAX_SOURCE_SEAL_PROJECTION_BYTES + 1))
            descriptor, fingerprint = pin_regular_metadata(
                oversized, "self-test oversized source projection"
            )
            try:
                try:
                    read_pinned_descriptor(
                        descriptor,
                        fingerprint,
                        MAX_SOURCE_SEAL_PROJECTION_BYTES,
                        "self-test oversized source projection",
                    )
                except ValueError as error:
                    if "16384-byte size limit" not in str(error):
                        raise
                else:
                    errors.append("self-test accepted an oversized source projection")
            finally:
                os.close(descriptor)
    except (OSError, ValueError) as error:
        errors.append(f"source-projection bound self-test failed unexpectedly: {error}")
    if (
        "recursive-step-two-qualification-v4.norito" not in FINAL_METADATA
        or MAX_RELEASE_INVENTORY_ENTRIES != 17
        or MAX_QUALIFICATION_RECEIPT_BYTES != 802_816
    ):
        errors.append(
            "self-test failed to pin the final recursive qualification receipt inventory"
        )
    for invalid_catalog_path in (
        Path("relative/catalog"),
        Path("/trusted/staging/../catalog"),
    ):
        try:
            absolute_directory_chain(invalid_catalog_path)
        except ValueError:
            pass
        else:
            errors.append(
                "self-test failed to reject a noncanonical catalog path chain"
            )
    aggregate_boundary = 0
    try:
        for release_bytes in (
            MAX_CATALOG_AGGREGATE_BYTES // 2,
            MAX_CATALOG_AGGREGATE_BYTES // 2,
        ):
            aggregate_boundary = checked_catalog_aggregate_total(
                aggregate_boundary, release_bytes
            )
        checked_catalog_aggregate_total(aggregate_boundary, 1)
    except ValueError:
        if aggregate_boundary != MAX_CATALOG_AGGREGATE_BYTES:
            errors.append("self-test failed at the whole-catalog byte boundary")
    else:
        errors.append("self-test failed to reject an oversized multi-release catalog")
    try:
        with tempfile.TemporaryDirectory(
            prefix="kagemusha-self-test-staging-parent-"
        ) as staging_text, tempfile.TemporaryDirectory(
            prefix="kagemusha-self-test-attacker-tmpdir-"
        ) as attacker_tmpdir:
            staging_parent = Path(staging_text).resolve(strict=True)
            prior_tmpdir = os.environ.get("TMPDIR")
            os.environ["TMPDIR"] = attacker_tmpdir
            try:
                snapshot, snapshot_path = snapshot_private_bytes(
                    b"authenticated physical-iOS evidence bytes",
                    "evidence.json",
                    "self-test evidence",
                    staging_parent,
                )
                try:
                    snapshot_metadata = snapshot_path.lstat()
                    if (
                        snapshot_path.read_bytes()
                        != b"authenticated physical-iOS evidence bytes"
                        or snapshot_path.parent.parent != staging_parent
                        or Path(attacker_tmpdir).resolve(strict=True)
                        in snapshot_path.parents
                        or stat.S_IMODE(snapshot_metadata.st_mode) != 0o600
                        or stat.S_IMODE(snapshot_path.parent.lstat().st_mode) != 0o700
                    ):
                        errors.append(
                            "self-test failed to create an exact fixed-parent evidence snapshot"
                        )
                finally:
                    snapshot.cleanup()
                empty_snapshot, empty_snapshot_path = snapshot_private_bytes(
                    b"",
                    "revocation",
                    "self-test empty SSH revocation policy",
                    staging_parent,
                    allow_empty=True,
                )
                try:
                    if (
                        empty_snapshot_path.stat().st_size != 0
                        or hashlib.sha256(empty_snapshot_path.read_bytes()).hexdigest()
                        != hashlib.sha256(b"").hexdigest()
                    ):
                        errors.append(
                            "self-test failed to preserve an explicitly pinned empty revocation policy"
                        )
                finally:
                    empty_snapshot.cleanup()
                allowed_snapshot, allowed_path = snapshot_private_bytes(
                    b"reviewer@example.test ssh-ed25519 AAAA\n",
                    "allowed-signers",
                    "self-test SSH allowed-signers policy",
                    staging_parent,
                )
                revocation_snapshot, revocation_path = snapshot_private_bytes(
                    b"",
                    "revocation",
                    "self-test SSH revocation policy",
                    staging_parent,
                    allow_empty=True,
                )
                config_snapshot: tempfile.TemporaryDirectory[str] | None = None
                try:
                    config_payload = isolated_source_trust_git_config(
                        allowed_path, revocation_path
                    )
                    config_snapshot, config_path = snapshot_private_bytes(
                        config_payload,
                        ".gitconfig",
                        "self-test isolated source SSH Git config",
                        staging_parent,
                    )
                    config_environment = source_git_environment()
                    config_environment.pop("GIT_CONFIG_GLOBAL", None)
                    config_environment["HOME"] = str(config_path.parent)
                    for key, expected in (
                        ("gpg.ssh.allowedSignersFile", allowed_path),
                        ("gpg.ssh.revocationFile", revocation_path),
                    ):
                        configured = subprocess.run(
                            [
                                str(SOURCE_GIT),
                                "config",
                                "--global",
                                "--path",
                                "--get",
                                key,
                            ],
                            cwd=Path("/"),
                            env=config_environment,
                            stdin=subprocess.DEVNULL,
                            check=False,
                            capture_output=True,
                            text=True,
                            close_fds=True,
                        )
                        if (
                            configured.returncode != 0
                            or configured.stderr
                            or configured.stdout != f"{expected}\n"
                        ):
                            errors.append(
                                "self-test failed to expose only the snapshotted source SSH trust policy"
                            )
                finally:
                    if config_snapshot is not None:
                        config_snapshot.cleanup()
                    revocation_snapshot.cleanup()
                    allowed_snapshot.cleanup()
            finally:
                if prior_tmpdir is None:
                    os.environ.pop("TMPDIR", None)
                else:
                    os.environ["TMPDIR"] = prior_tmpdir
    except (OSError, ValueError) as error:
        errors.append(f"private evidence snapshot self-test failed unexpectedly: {error}")
    try:
        with tempfile.TemporaryDirectory(prefix="kagemusha-custody-self-test-") as temporary:
            writable = Path(temporary) / "writable"
            writable.write_bytes(b"untrusted")
            writable.chmod(0o622)
            descriptor, _ = pin_regular_metadata(writable, "self-test writable file")
            try:
                try:
                    require_production_root_custody(
                        descriptor, "self-test writable file"
                    )
                except ValueError:
                    pass
                else:
                    errors.append(
                        "self-test failed to reject a caller-writable production input"
                    )
            finally:
                os.close(descriptor)
    except (OSError, ValueError) as error:
        errors.append(f"production custody self-test failed unexpectedly: {error}")
    report_manifest_artifacts = [
        {
            "file_name": name,
            "size_bytes": index + 1,
            "sha256": f"{index + 1:x}" * 64,
            "payload_size_bytes": index + 2,
            "payload_sha256": f"{index + 2:x}" * 64,
        }
        for index, name in enumerate(ARTIFACTS)
    ]
    report_manifest = {
        "generation": "self-test",
        "generation_memory_limit_bytes": 1,
        "generation_memory_enforcement_profile": "self-test-profile",
        "network_id": "self-test-network",
        "asset": "self-test-asset",
        "asset_scale": 2,
        "authenticated_source_seal_projection_sha256": "b" * 64,
        "reviewed_cargo_binary_sha256": "c" * 64,
        "reviewed_rustc_binary_sha256": "d" * 64,
        "qualified_candidate_sha256": "7" * 64,
        "profiles": [
            {"artifacts": report_manifest_artifacts[:4]},
            {"artifacts": report_manifest_artifacts[4:]},
        ],
        "topup_finality_roster_artifact": {
            "file_name": "topup-finality-roster-v4.norito",
            "size_bytes": 17,
            "sha256": "a" * 64,
        },
    }
    report_artifacts = [
        {
            "purpose": purpose,
            "file_name": artifact["file_name"],
            "size_bytes": artifact["size_bytes"],
            "sha256": artifact["sha256"],
            "payload_size_bytes": artifact["payload_size_bytes"],
            "payload_sha256": artifact["payload_sha256"],
        }
        for purpose, artifact in zip(
            REPORT_ARTIFACT_PURPOSES, report_manifest_artifacts, strict=True
        )
    ]
    report_artifacts.append(
        {
            "purpose": "topup_finality_roster",
            "file_name": "topup-finality-roster-v4.norito",
            "size_bytes": 17,
            "sha256": "a" * 64,
            "payload_size_bytes": None,
            "payload_sha256": None,
        }
    )
    verifier_report = {
        "status": "verified",
        "envelope_sha256": "1" * 64,
        "manifest_body_sha256": "2" * 64,
        "candidate_sha256": "3" * 64,
        "qualification_receipt_sha256": "4" * 64,
        "qualified_candidate_sha256": "7" * 64,
        "authenticated_source_seal_projection_sha256": "b" * 64,
        "reviewed_cargo_binary_sha256": "c" * 64,
        "reviewed_rustc_binary_sha256": "d" * 64,
        "promotion_record_sha256": "6" * 64,
        "release_policy_sha256": "5" * 64,
        "generation": "self-test",
        "generation_memory_limit_bytes": 1,
        "generation_memory_enforcement_profile": "self-test-profile",
        "network_id": "self-test-network",
        "asset_definition_id": "self-test-asset",
        "asset_scale": 2,
        "bridge_abi_version": 22,
        "recursive_step_verifier_commitment": "9" * 64,
        "artifacts": report_artifacts,
    }
    try:
        validate_kagami_verification_report(
            verifier_report,
            directory=Path("/release") / ("1" * 64),
            manifest=report_manifest,
            policy_sha256="5" * 64,
            promotion_record_sha256="6" * 64,
            qualification_receipt_sha256="4" * 64,
            ios_candidate_sha256="3" * 64,
        )
        invalid_report = dict(verifier_report)
        invalid_report["status"] = "unverified"
        validate_kagami_verification_report(
            invalid_report,
            directory=Path("/release") / ("1" * 64),
            manifest=report_manifest,
            policy_sha256="5" * 64,
            promotion_record_sha256="6" * 64,
            qualification_receipt_sha256="4" * 64,
            ios_candidate_sha256="3" * 64,
        )
    except ValueError as error:
        if "did not report one verified" not in str(error):
            errors.append(f"authenticated report self-test failed unexpectedly: {error}")
    else:
        errors.append("self-test failed to reject an unverified Kagami report")
    for field in (
        "authenticated_source_seal_projection_sha256",
        "reviewed_cargo_binary_sha256",
        "reviewed_rustc_binary_sha256",
    ):
        mismatched_report = dict(verifier_report)
        mismatched_report[field] = "e" * 64
        try:
            validate_kagami_verification_report(
                mismatched_report,
                directory=Path("/release") / ("1" * 64),
                manifest=report_manifest,
                policy_sha256="5" * 64,
                promotion_record_sha256="6" * 64,
                qualification_receipt_sha256="4" * 64,
                ios_candidate_sha256="3" * 64,
            )
        except ValueError as error:
            if "differs from the manifest" not in str(error):
                errors.append(f"authenticated report {field} self-test failed unexpectedly: {error}")
        else:
            errors.append(f"self-test failed to reject a mismatched Kagami report {field}")
    try:
        with tempfile.TemporaryDirectory(prefix="kagemusha-catalog-pin-self-test-") as temporary:
            catalog_root = Path(temporary).resolve(strict=True)
            release = catalog_root / "release"
            replacement = catalog_root / "replacement"
            release.mkdir()
            replacement.mkdir()
            release_file = release / "artifact"
            release_file.write_bytes(b"pinned release artifact")
            (replacement / "artifact").write_bytes(b"substituted release artifact")
            pins: list[tuple[Path, int, tuple[int, ...], str]] = []
            try:
                for component in absolute_directory_chain(catalog_root):
                    label = f"self-test catalog path component {component}"
                    descriptor, fingerprint = pin_directory_metadata(component, label)
                    pins.append((component, descriptor, fingerprint, label))
                release_label = "self-test release directory"
                release_descriptor, release_fingerprint = pin_directory_metadata(
                    release, release_label
                )
                pins.append(
                    (release, release_descriptor, release_fingerprint, release_label)
                )
                file_label = "self-test release file"
                file_descriptor, file_fingerprint = pin_regular_metadata(
                    release_file, file_label
                )
                pins.append((release_file, file_descriptor, file_fingerprint, file_label))
                for path, descriptor, fingerprint, label in pins:
                    revalidate_pinned_metadata(path, descriptor, fingerprint, label)

                displaced = catalog_root / "displaced"
                release.rename(displaced)
                replacement.rename(release)
                try:
                    revalidate_pinned_metadata(
                        release,
                        release_descriptor,
                        release_fingerprint,
                        release_label,
                    )
                except (OSError, ValueError):
                    pass
                else:
                    errors.append(
                        "self-test failed to reject a substituted release directory"
                    )
            finally:
                for _, descriptor, _, _ in reversed(pins):
                    os.close(descriptor)
    except (OSError, ValueError) as error:
        errors.append(f"catalog pin self-test failed unexpectedly: {error}")
    baseline = {
        READINESS: read(READINESS, []),
        MODEL: read_reviewed_model([], {}),
        MODEL_COMPONENT: read(MODEL_COMPONENT, []),
        MODEL_VERIFIER_COMPONENT: read(MODEL_VERIFIER_COMPONENT, []),
        PRIVACY: read(PRIVACY, []),
        PRIVACY_PROTOCOL: read(PRIVACY_PROTOCOL, []),
        CATALOG: read_reviewed_catalog([], {}),
        CORE: read(CORE, []),
        KAGAMI: read(KAGAMI, []),
        BUNDLE: read(BUNDLE, []),
        WORKFLOW: read(WORKFLOW, []),
        PRODUCTION_IOS_EVIDENCE_MODULE: read(
            PRODUCTION_IOS_EVIDENCE_MODULE, []
        ),
    }
    conflicted_model = (
        baseline[MODEL]
        + "\n<<<<<<< HEAD\nreviewed-side\n=======\nincoming-side\n>>>>>>> origin/reviewed\n"
    )
    conflict_errors = static_errors({MODEL: conflicted_model})
    if not any("unresolved Git merge conflict marker" in error for error in conflict_errors):
        errors.append("self-test failed to reject a reviewed merge conflict")
    missing_python_version_check = baseline[READINESS].replace(
        "sys.version_info >= (3, 10)", "True", 1
    )
    python_version_errors = static_errors(
        {READINESS: missing_python_version_check}
    )
    if not any("sys.version_info >= (3, 10)" in error for error in python_version_errors):
        errors.append("self-test failed to reject a missing Python version preflight")
    missing_index_check = baseline[READINESS].replace(
        "--diff-filter=U", "--diff-filter=M", 1
    )
    index_check_errors = static_errors({READINESS: missing_index_check})
    if not any("--diff-filter=U" in error for error in index_check_errors):
        errors.append("self-test failed to reject a missing unresolved-index preflight")
    bypassed_production_ios_blocker = baseline[
        PRODUCTION_IOS_EVIDENCE_MODULE
    ].replace(
        "    errors.append(PLATFORM_TRUST_BLOCKER)\n    return errors\n",
        "    return errors\n",
        1,
    )
    bypassed_production_ios_errors = static_errors(
        {PRODUCTION_IOS_EVIDENCE_MODULE: bypassed_production_ios_blocker}
    )
    if not any(
        "unconditional production App Attest trust blocker" in error
        for error in bypassed_production_ios_errors
    ):
        errors.append(
            "self-test failed to reject removal of the production App Attest trust blocker"
        )
    mutated = baseline[MODEL].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 21",
    )
    if not static_errors({MODEL: mutated}):
        errors.append("self-test failed to reject ABI-21 substitution")
    detached_model_component = baseline[MODEL_COMPONENT].replace(
        "pub enum KagemushaPastaCycleArtifactKindV4",
        "pub enum DetachedKagemushaPastaCycleArtifactKindV4",
        1,
    )
    if not static_errors({MODEL_COMPONENT: detached_model_component}):
        errors.append("self-test failed to authenticate the split model component")
    detached_verifier_component = baseline[MODEL_VERIFIER_COMPONENT].replace(
        "const VERIFIER_IDENTITY_SCHEMA_V4",
        "const DETACHED_VERIFIER_IDENTITY_SCHEMA_V4",
        1,
    )
    if not static_errors({MODEL_VERIFIER_COMPONENT: detached_verifier_component}):
        errors.append("self-test failed to authenticate the release-verifier component")
    sixteen_file_verifier = baseline[KAGAMI].replace(
        "if expected.len() != 17",
        "if expected.len() != 16",
        1,
    )
    if not static_errors({KAGAMI: sixteen_file_verifier}):
        errors.append(
            "self-test failed to reject a sixteen-file final release verifier"
        )
    verifier_without_receipt = baseline[KAGAMI].replace(
        """        (
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
            "qualification receipt",
        ),
""",
        "",
        1,
    )
    verifier_without_receipt_errors = static_errors(
        {KAGAMI: verifier_without_receipt}
    )
    if not any(
        "function-scoped 17-file verifier inventory" in error
        for error in verifier_without_receipt_errors
    ):
        errors.append(
            "self-test failed to reject a verifier inventory without the qualification receipt"
        )
    sixteen_file_finalizer = baseline[BUNDLE].replace(
        "const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 17;",
        "const FINAL_RELEASE_INVENTORY_COUNT_V4: usize = 16;",
        1,
    )
    if not static_errors({BUNDLE: sixteen_file_finalizer}):
        errors.append(
            "self-test failed to reject a sixteen-file final release producer"
        )
    producer_without_receipt = baseline[BUNDLE].replace(
        """            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
            PROMOTION_RECORD_FILE_NAME_V4,
""",
        """            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
            PROMOTION_RECORD_FILE_NAME_V4,
""",
        1,
    )
    producer_without_receipt_errors = static_errors(
        {BUNDLE: producer_without_receipt}
    )
    if not any(
        "function-scoped 17-file producer inventory" in error
        for error in producer_without_receipt_errors
    ):
        errors.append(
            "self-test failed to reject a producer inventory without the qualification receipt"
        )
    renamed_inventory_test = baseline[BUNDLE].replace(
        "fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()",
        "fn retired_final_release_inventory_test()",
        1,
    )
    renamed_inventory_test_errors = static_errors({BUNDLE: renamed_inventory_test})
    if not any(
        "fn final_release_inventory_is_exact_and_includes_recursive_qualification_receipt()"
        in error
        for error in renamed_inventory_test_errors
    ):
        errors.append("self-test failed to reject a missing producer inventory test")
    receipt_bound_drift = baseline[MODEL].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 384 * 1024;",
        "KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4: u32 = 385 * 1024;",
        1,
    )
    receipt_bound_drift_errors = static_errors({MODEL: receipt_bound_drift})
    if not any(
        "384 KiB absolute V4 proof-pair bound" in error
        for error in receipt_bound_drift_errors
    ):
        errors.append("self-test failed to reject qualification receipt bound drift")
    receipt_text_scan = baseline[READINESS].replace(
        """    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),
)""",
        """    ("promotion-record-v4.norito", MAX_PROMOTION_RECORD_BYTES),
    ("recursive-step-two-qualification-v4.norito", MAX_QUALIFICATION_RECEIPT_BYTES),
)""",
        1,
    )
    receipt_text_scan_errors = static_errors({READINESS: receipt_text_scan})
    if not any(
        "opaque qualification receipt is routed through textual evidence scanning" in error
        for error in receipt_text_scan_errors
    ):
        errors.append("self-test failed to reject textual scanning of an opaque receipt")
    shared_bridge_abi_drift = baseline[PRIVACY_PROTOCOL].replace(
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;",
        "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 21;",
        1,
    )
    if not static_errors({PRIVACY_PROTOCOL: shared_bridge_abi_drift}):
        errors.append("self-test failed to reject shared bridge ABI-21 substitution")
    detached_protocol_surface = baseline[PRIVACY].replace(
        'include!("privacy/protocol.rs");',
        "// protocol include removed",
        1,
    )
    if not static_errors({PRIVACY: detached_protocol_surface}):
        errors.append("self-test failed to reject detached privacy protocol surface")
    flipped_availability = baseline[MODEL].replace(
        'cfg!(feature = "kagemusha-production-enabled")',
        "true",
        1,
    )
    if not static_errors({MODEL: flipped_availability}):
        errors.append("self-test failed to reject an invalid availability state")
    seven_artifacts = baseline[CATALOG].replace(
        "KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len();",
        "7;",
        1,
    )
    seven_artifact_errors = static_errors({CATALOG: seven_artifacts})
    if not any("exact-eight manifest inventory check" in error for error in seven_artifact_errors):
        errors.append("self-test failed to reject a seven-artifact manifest check")
    unguarded_change = baseline[CORE].replace(
        "change_release.as_ref().is_some_and(|release|",
        "change_release.as_ref().is_none_or(|release|",
        1,
    )
    unguarded_change_errors = static_errors({CORE: unguarded_change})
    if not any(
        "offline-change withdrawal-height issuance check" in error
        for error in unguarded_change_errors
    ):
        errors.append("self-test failed to reject an unguarded offline-change issuance path")
    missing_frontier_filter = baseline[WORKFLOW].replace(
        "cargo test -p iroha_core output_membership --lib",
        "cargo test -p iroha_core retired_output_membership_filter --lib",
        1,
    )
    missing_frontier_filter_errors = static_errors({WORKFLOW: missing_frontier_filter})
    if not any(
        "cargo test -p iroha_core output_membership --lib" in error
        for error in missing_frontier_filter_errors
    ):
        errors.append("self-test failed to reject a missing frontier-test workflow filter")
    boundary_artifacts = {
        name: MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES // len(ARTIFACTS)
        for name in ARTIFACTS
    }
    if (
        checked_declared_artifact_total(boundary_artifacts)
        != MAX_DECLARED_ARTIFACT_AGGREGATE_BYTES
    ):
        errors.append("self-test failed to accept the exact artifact aggregate limit")
    exact_file_artifacts = {name: 1 for name in ARTIFACTS}
    exact_file_artifacts[ARTIFACTS[0]] = MAX_DECLARED_ARTIFACT_FILE_BYTES
    if (
        checked_declared_artifact_total(exact_file_artifacts)
        != MAX_DECLARED_ARTIFACT_FILE_BYTES + len(ARTIFACTS) - 1
    ):
        errors.append("self-test failed to accept the exact artifact file limit")
    oversized_file_artifacts = dict(boundary_artifacts)
    oversized_file_artifacts[ARTIFACTS[0]] = MAX_DECLARED_ARTIFACT_FILE_BYTES + 1
    try:
        checked_declared_artifact_total(oversized_file_artifacts)
    except ValueError:
        pass
    else:
        errors.append("self-test failed to reject an oversized artifact file")
    oversized_aggregate_artifacts = dict(boundary_artifacts)
    oversized_aggregate_artifacts[ARTIFACTS[0]] += 1
    try:
        checked_declared_artifact_total(oversized_aggregate_artifacts)
    except ValueError:
        pass
    else:
        errors.append("self-test failed to reject an oversized artifact aggregate")
    verifier_command = release_verifier_command(
        Path("/trusted/kagami"), Path("/release"), Path("/policy.norito")
    )
    if verifier_command[:3] != [
        "/trusted/kagami",
        "kagemusha",
        "verify-release-v4",
    ]:
        errors.append("self-test failed to pin the explicit Kagami release verifier")
    cargo_verifier = baseline[READINESS].replace(
        "        str(verifier),\n        \"kagemusha\",",
        "        \"cargo\",\n        \"run\",",
        1,
    )
    cargo_verifier_errors = static_errors({READINESS: cargo_verifier})
    if not any(
        "promotion verifier command" in error for error in cargo_verifier_errors
    ):
        errors.append("self-test failed to reject a PATH-resolved Cargo verifier")
    reopened_ios_evidence = baseline[READINESS].replace(
        "                evidence_snapshot_path,\n                raw_root,",
        "                directory / \"physical-device-benchmark.evidence\",\n"
        "                raw_root,",
        1,
    )
    reopened_ios_evidence_errors = static_errors(
        {READINESS: reopened_ios_evidence}
    )
    if not any(
        "same pinned evidence, trusted key, and production policy snapshots" in error
        for error in reopened_ios_evidence_errors
    ):
        errors.append(
            "self-test failed to reject reopening physical-iOS evidence for validation"
        )
    reopened_ios_key = baseline[READINESS].replace(
        "                trusted_public_key_snapshot,\n"
        "                trusted_production_policy_snapshot,\n",
        "                ios_configuration[2],\n"
        "                trusted_production_policy_snapshot,\n",
        1,
    )
    reopened_ios_key_errors = static_errors({READINESS: reopened_ios_key})
    if not any(
        "same pinned evidence, trusted key, and production policy snapshots" in error
        for error in reopened_ios_key_errors
    ):
        errors.append("self-test failed to reject reopening the physical-iOS trust key")
    reopened_ios_policy = baseline[READINESS].replace(
        "                trusted_production_policy_snapshot,\n",
        "                ios_configuration[3],\n",
        1,
    )
    reopened_ios_policy_errors = static_errors({READINESS: reopened_ios_policy})
    if not any(
        "same pinned evidence, trusted key, and production policy snapshots" in error
        for error in reopened_ios_policy_errors
    ):
        errors.append("self-test failed to reject reopening the physical-iOS production policy")
    accepted_testnet_ios_evidence = baseline[READINESS].replace(
        'production_module.__dict__.get(\n            "validate_production_signed_evidence"\n        )',
        'production_module.__dict__.get("validate_signed_evidence")',
        1,
    )
    accepted_testnet_ios_errors = static_errors({READINESS: accepted_testnet_ios_evidence})
    if not any(
        "production-only iOS evidence validator entrypoint" in error
        for error in accepted_testnet_ios_errors
    ):
        errors.append(
            "self-test failed to reject the testnet-only iOS validator in promotion"
        )
    missing_root_custody = baseline[READINESS].replace(
        "                        require_production_root_custody(descriptor, label)",
        "                        # production root custody removed",
        1,
    )
    missing_root_custody_errors = static_errors({READINESS: missing_root_custody})
    if not any(
        "root-custody every production trust class" in error
        for error in missing_root_custody_errors
    ):
        errors.append("self-test failed to reject a missing production custody check")
    unpinned_running_python = baseline[READINESS].replace(
        "            != trusted_python_sha256",
        "            != hash_pinned_descriptor(\n"
        "                descriptor, fingerprint, MAX_KAGAMI_VERIFIER_BYTES, label\n"
        "            )",
        1,
    )
    unpinned_running_python_errors = static_errors(
        {READINESS: unpinned_running_python}
    )
    if not any(
        "running promotion interpreter digest revalidation" in error
        for error in unpinned_running_python_errors
    ):
        errors.append("self-test failed to reject an unpinned running Python runtime")
    path_reopened_python = baseline[READINESS].replace(
        '"/dev/fd/${PYTHON_PIN_FD}"',
        '"${PYTHON_BIN}"',
    )
    path_reopened_python_errors = static_errors({READINESS: path_reopened_python})
    if not any(
        "pre-exec Python descriptor custody" in error
        for error in path_reopened_python_errors
    ):
        errors.append("self-test failed to reject path-reopened pre-exec Python")
    ambient_snapshot_parent = baseline[READINESS].replace(
        'prefix="kagemusha-pinned-input-", dir=staging_parent',
        'prefix="kagemusha-pinned-input-"',
        1,
    )
    ambient_snapshot_parent_errors = static_errors(
        {READINESS: ambient_snapshot_parent}
    )
    if not any(
        "explicit staging parent" in error
        for error in ambient_snapshot_parent_errors
    ):
        errors.append("self-test failed to reject ambient-TMPDIR promotion staging")
    ambient_source_helper_tmpdir = baseline[READINESS].replace(
        '"TMPDIR": str(PROMOTION_STAGING_PARENT),',
        '"TMPDIR": "/tmp",',
        1,
    )
    ambient_source_helper_tmpdir_errors = static_errors(
        {READINESS: ambient_source_helper_tmpdir}
    )
    if not any(
        "fixed staging parent" in error
        for error in ambient_source_helper_tmpdir_errors
    ):
        errors.append(
            "self-test failed to reject ambient-TMPDIR promotion subprocess staging"
        )
    path_executed_source_helper = baseline[READINESS].replace(
        "                str(trusted_source_helper_snapshot),",
        "                str(source_helper_path),",
        1,
    )
    path_executed_source_helper_errors = static_errors(
        {READINESS: path_executed_source_helper}
    )
    if not any(
        "source-closure-authenticated source-tree helper snapshot" in error
        for error in path_executed_source_helper_errors
    ):
        errors.append("self-test failed to reject a path-executed source helper")
    path_loaded_ios_validator = baseline[READINESS].replace(
        "                validator_bytes,\n"
        "                trusted_ios_validator_snapshot,",
        "                validator_bytes,\n"
        "                ios_validator_path,",
        1,
    )
    path_loaded_ios_validator_errors = static_errors(
        {READINESS: path_loaded_ios_validator}
    )
    if not any(
        "source-closure-authenticated candidate and production iOS validator snapshots" in error
        for error in path_loaded_ios_validator_errors
    ):
        errors.append("self-test failed to reject a path-loaded iOS validator")
    ambient_source_trust_home = baseline[READINESS].replace(
        '"HOME": str(trusted_source_trust_home),',
        '"HOME": "/var/empty",',
        1,
    )
    ambient_source_trust_home_errors = static_errors(
        {READINESS: ambient_source_trust_home}
    )
    if not any(
        "source SSH trust" in error
        or "closure-bound snapshotted" in error
        for error in ambient_source_trust_home_errors
    ):
        errors.append("self-test failed to reject an unconfigured source trust HOME")
    unbound_source_trust_projection = baseline[READINESS].replace(
        "        validate_source_trust_projection(\n"
        "            source_projection_bytes,",
        "        bypass_source_trust_projection(\n"
        "            source_projection_bytes,",
        1,
    )
    unbound_source_trust_projection_errors = static_errors(
        {READINESS: unbound_source_trust_projection}
    )
    if not any(
        "closure-bound snapshotted source SSH trust policies" in error
        for error in unbound_source_trust_projection_errors
    ):
        errors.append("self-test failed to reject unbound source SSH trust policies")
    missing_gate_checkout_custody = baseline[READINESS].replace(
        '  promotion_assert_root_custody "${DERIVED_ROOT_DIR}" '
        '"promotion readiness checkout" || exit 2',
        "  # promotion checkout custody removed",
        1,
    )
    missing_gate_checkout_custody_errors = static_errors(
        {READINESS: missing_gate_checkout_custody}
    )
    if not any(
        "root-custodied gate bootstrap" in error
        or "promotion_assert_root_custody" in error
        for error in missing_gate_checkout_custody_errors
    ):
        errors.append("self-test failed to reject an untrusted gate checkout")
    bypassed_gate_digest = baseline[READINESS].replace(
        '  if [[ "${OBSERVED_GATE_SHA256}" != "${GATE_SHA256}" ]]; then',
        "  if false; then",
        1,
    )
    bypassed_gate_digest_errors = static_errors(
        {READINESS: bypassed_gate_digest}
    )
    if not any(
        "root-custodied gate bootstrap" in error
        for error in bypassed_gate_digest_errors
    ):
        errors.append("self-test failed to reject a bypassed reviewed gate digest")
    forged_dirname_root = baseline[READINESS].replace(
        'SCRIPT_DIRECTORY_LEXICAL="${SCRIPT_PATH_LEXICAL%/*}"',
        'SCRIPT_DIRECTORY_LEXICAL="$(dirname "${SCRIPT_PATH_LEXICAL}")"',
        1,
    )
    forged_dirname_root_errors = static_errors({READINESS: forged_dirname_root})
    if not any(
        "promotion shell bootstrap" in error
        for error in forged_dirname_root_errors
    ):
        errors.append("self-test failed to reject PATH-resolved root derivation")

if errors:
    print(
        f"Kagemusha ABI-21/V4 (native bridge ABI 22) {mode} corridor failed:",
        file=sys.stderr,
    )
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
if mode == "candidate":
    print(
        "Kagemusha ABI-21/V4 (native bridge ABI 22) static candidate corridor passed; "
        "production promotion was not evaluated."
    )
else:
    print("Kagemusha ABI-21/V4 (native bridge ABI 22) production promotion corridor passed.")
PY
