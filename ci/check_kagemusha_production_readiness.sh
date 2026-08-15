#!/bin/bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_PRODUCTION_READINESS_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
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

PYTHON_BIN="python3"
if [[ "${MODE}" == "promotion" ]]; then
  if [[ -n "${KAGEMUSHA_PRODUCTION_READINESS_ROOT+x}" ]]; then
    echo "promotion rejects KAGEMUSHA_PRODUCTION_READINESS_ROOT; run the checked-in gate in place" >&2
    exit 2
  fi
  PYTHON_BIN="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON:-}"
  PYTHON_SHA256="${KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256:-}"
  if [[ "${PYTHON_BIN}" != /* || ! -f "${PYTHON_BIN}" || -L "${PYTHON_BIN}" || ! -x "${PYTHON_BIN}" || ! "${PYTHON_SHA256}" =~ ^[0-9a-f]{64}$ || "${PYTHON_SHA256}" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "promotion requires a canonical absolute digest-pinned Python interpreter" >&2
    exit 2
  fi
  if [[ -x /usr/bin/shasum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/shasum -a 256 -- "${PYTHON_BIN}" | /usr/bin/awk '{print $1}')"
  elif [[ -x /usr/bin/sha256sum ]]; then
    OBSERVED_PYTHON_SHA256="$(/usr/bin/sha256sum -- "${PYTHON_BIN}" | /usr/bin/awk '{print $1}')"
  else
    echo "promotion requires root-installed /usr/bin/shasum or /usr/bin/sha256sum" >&2
    exit 2
  fi
  if [[ "${OBSERVED_PYTHON_SHA256}" != "${PYTHON_SHA256}" ]]; then
    echo "promotion Python interpreter differs from its trusted SHA-256" >&2
    exit 2
  fi
fi

"${PYTHON_BIN}" -I - "${ROOT_DIR}" "${MODE}" "${SELF_TEST}" <<'PY'
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

READINESS = "ci/check_kagemusha_production_readiness.sh"
MODEL = "crates/iroha_data_model/src/offline/mod.rs"
MODEL_COMPONENT = "crates/iroha_data_model/src/offline/kagemusha_model.rs"
MODEL_INCLUDE = 'include!("kagemusha_model.rs");'
PRIVACY = "crates/iroha_data_model/src/privacy.rs"
PRIVACY_PROTOCOL = "crates/iroha_data_model/src/privacy/protocol.rs"
BRIDGE = "crates/connect_norito_bridge/src/lib.rs"
HEADER = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
CATALOG = "crates/iroha_core/src/smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs"
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
}
READ_CHUNK_BYTES = 1024 * 1024
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
    """Read the parent and its authenticated model component as one source."""

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
    if parent.count(MODEL_INCLUDE) != 1:
        errors.append(
            f"{MODEL}: expected exactly one reviewed {Path(MODEL_COMPONENT).name} include"
        )
        return parent
    return parent.replace(MODEL_INCLUDE, component, 1)


def read_regular_bounded(path: Path, maximum_bytes: int, label: str) -> bytes:
    """Read one pinned regular file without trusting path metadata as an allocation size."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if before.st_size <= 0 or before.st_size > maximum_bytes:
        raise ValueError(f"{label} violates its {maximum_bytes}-byte size limit")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    chunks: list[bytes] = []
    try:
        opened = os.fstat(descriptor)
        identity = (before.st_dev, before.st_ino)
        if (
            not os.path.samestat(before, opened)
            or opened.st_nlink != 1
            or opened.st_size != before.st_size
        ):
            raise ValueError(f"{label} changed while it was opened")
        size = 0
        while True:
            chunk = os.read(descriptor, min(READ_CHUNK_BYTES, maximum_bytes + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_bytes:
                raise ValueError(f"{label} exceeds its size limit")
            chunks.append(chunk)
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            (after_path.st_dev, after_path.st_ino) != identity
            or not os.path.samestat(before, after_open)
            or size != before.st_size
            or after_path.st_size != size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    return b"".join(chunks)


def inspect_regular_prefix(
    path: Path,
    expected_bytes: int,
    maximum_bytes: int,
    prefix_bytes: int,
    label: str,
) -> bytes:
    """Inspect only a bounded prefix while pinning the complete file's identity and size."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if expected_bytes <= 0 or expected_bytes > maximum_bytes or before.st_size != expected_bytes:
        raise ValueError(f"{label} does not match its bounded declared size")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if not os.path.samestat(before, opened) or opened.st_nlink != 1:
            raise ValueError(f"{label} changed while it was opened")
        prefix = os.read(descriptor, prefix_bytes)
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            len(prefix) != prefix_bytes
            or not os.path.samestat(before, after_open)
            or not os.path.samestat(before, after_path)
            or after_path.st_size != before.st_size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was inspected")
        return prefix
    finally:
        os.close(descriptor)


def pin_regular_metadata(path: Path, label: str) -> tuple[int, tuple[int, ...]]:
    """Open and retain one exact regular-file metadata identity."""

    before = path.lstat()
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
    ):
        raise ValueError(f"{label} must be a nonempty singly-linked regular file")
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
        )
        if not os.path.samestat(before, opened) or fingerprint != (
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mode,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
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
        )
        if not os.path.samestat(before, opened) or fingerprint != (
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mode,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
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
    )
    observed_path = (
        after_path.st_dev,
        after_path.st_ino,
        after_path.st_nlink,
        after_path.st_mode,
        after_path.st_size,
        after_path.st_mtime_ns,
        after_path.st_ctime_ns,
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


def read_pinned_descriptor(
    descriptor: int,
    fingerprint: tuple[int, ...],
    maximum_bytes: int,
    label: str,
) -> bytes:
    """Read exact bounded bytes through a retained descriptor."""

    size = fingerprint[4]
    if size <= 0 or size > maximum_bytes:
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


def snapshot_pinned_executable(
    descriptor: int, fingerprint: tuple[int, ...], label: str
) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    """Materialize exact already-hashed executable bytes in an owner-private directory."""

    temporary = tempfile.TemporaryDirectory(prefix="kagemusha-kagami-verifier-")
    temporary_path = Path(temporary.name).resolve(strict=True)
    os.chmod(temporary_path, 0o700)
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
    if target.lstat().st_size != fingerprint[4]:
        temporary.cleanup()
        raise ValueError(f"{label} snapshot has the wrong size")
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


def evidence_is_non_placeholder(path: Path, maximum_bytes: int, label: str) -> bool:
    """Scan bounded evidence without retaining the complete file in memory."""

    before = path.lstat()
    if path.is_symlink() or not path.is_file() or before.st_nlink != 1:
        raise ValueError(f"{label} must be a singly-linked non-symlink regular file")
    if before.st_size < 64 or before.st_size > maximum_bytes:
        return False
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    placeholder = re.compile(rb"(?:placeholder|synthetic|dummy|todo|not[ -]?reviewed)", re.I)
    tail = b""
    size = 0
    found = False
    try:
        opened = os.fstat(descriptor)
        if not os.path.samestat(before, opened) or opened.st_nlink != 1:
            raise ValueError(f"{label} changed while it was opened")
        while True:
            chunk = os.read(descriptor, min(READ_CHUNK_BYTES, maximum_bytes + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_bytes:
                return False
            scan = tail + chunk
            found = found or placeholder.search(scan) is not None
            tail = scan[-64:]
        after_open = os.fstat(descriptor)
        after_path = path.lstat()
        if (
            not os.path.samestat(before, after_open)
            or not os.path.samestat(before, after_path)
            or size != before.st_size
            or after_path.st_size != size
            or after_path.st_mtime_ns != before.st_mtime_ns
            or after_path.st_ctime_ns != before.st_ctime_ns
        ):
            raise ValueError(f"{label} changed while it was scanned")
    finally:
        os.close(descriptor)
    return not found


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
            CATALOG,
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
        )
    }
    texts[MODEL] = read_reviewed_model(errors, overrides)
    model = texts[MODEL]
    require(
        model,
        MODEL,
        errors,
        "KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4: u32 = 22",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v4"',
        '"iroha.reviewed-source-closure.v1"',
        "reviewed_source_closure_descriptor_sha256",
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
    forbid(
        ios_validator_function,
        "physical-iOS evidence verification",
        errors,
        "subprocess.run",
        "sys.executable",
        "check_kagemusha_candidate_ios_evidence.py",
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
        "check_kagemusha_recursive_spend_v4_sdk_contract.sh",
        '"crates/iroha_core/src/smartcontracts/isi/offline/**"',
        "cargo test -p iroha_core kagemusha_v4 --lib",
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
        "cargo test -p iroha_torii readiness_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p iroha_torii v4_snapshot_admission_authenticates_exact_release_without_global_backend_flag",
        "cargo test -p connect_norito_bridge recursive_spend_v4",
        "cargo test -p connect_norito_bridge output_membership_local_carrier --lib",
    )
    return errors


def strict_json(path: Path) -> dict[str, object]:
    return strict_json_bytes(
        read_regular_bounded(path, MAX_MANIFEST_BYTES, "manifest JSON"),
        "manifest JSON",
    )


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
        "promotion_record_sha256",
        "release_policy_sha256",
        "recursive_step_verifier_commitment",
    ):
        canonical_nonzero_sha256(report.get(field), f"Kagami report {field}")
    manifest_equal_fields = (
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
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
) -> tuple[Path, str, Path] | None:
    """Return the complete opt-in physical-iOS evidence configuration."""

    root_text = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT", "")
    key_id = os.environ.get("KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID", "")
    public_key_text = os.environ.get(
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY", ""
    )
    present = tuple(bool(value) for value in (root_text, key_id, public_key_text))
    if not any(present):
        errors.append(
            "promotion requires signed physical-iOS raw evidence, trusted key id, and public key"
        )
        return None
    if not all(present):
        errors.append(
            "physical-iOS evidence requires KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT, "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID, and "
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY together"
        )
        return None
    ios_root = Path(root_text)
    public_key = Path(public_key_text)
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
    return ios_root, key_id, public_key


def load_ios_evidence_validator(
    module_bytes: bytes, module_path: Path
) -> Callable[[Path, Path, str, Path], list[str]]:
    """Load the reviewed validator from already pinned source bytes."""

    module_name = "_iroha_pinned_kagemusha_candidate_ios_evidence"
    module = types.ModuleType(module_name)
    module.__file__ = str(module_path)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        code = compile(module_bytes, str(module_path), "exec", dont_inherit=True)
        exec(code, module.__dict__)
        validator = module.__dict__.get("validate_signed_evidence")
        if not callable(validator):
            raise ValueError("pinned physical-iOS validator has no maintained entrypoint")
        return validator
    except BaseException:
        sys.modules.pop(module_name, None)
        raise


def verify_ios_evidence(
    directory: Path,
    ios_configuration: tuple[Path, str, Path],
    validator: Callable[[Path, Path, str, Path], list[str]],
) -> tuple[str | None, str | None]:
    """Verify one signed raw physical-iOS slot and return its candidate digest."""

    ios_root, key_id, public_key = ios_configuration
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
    evidence_path = directory / "physical-device-benchmark.evidence"
    validation_errors = validator(evidence_path, raw_root, key_id, public_key)
    if validation_errors:
        return None, (
            f"{directory.name}: physical-iOS evidence verification failed: "
            f"{validation_errors[-1]}"
        )
    try:
        evidence = strict_json_bytes(
            read_regular_bounded(
                evidence_path,
                MAX_BENCHMARK_EVIDENCE_BYTES,
                "signed physical-iOS evidence",
            ),
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
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        return None, f"{directory.name}: invalid signed physical-iOS evidence: {error}"
    return candidate_sha256, None


def promotion_errors() -> list[str]:
    errors: list[str] = []
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
    if (
        not reviewed_closure_text
        or re.fullmatch(r"[0-9a-f]{64}", reviewed_closure_sha256) is None
        or reviewed_closure_sha256 == "0" * 64
    ):
        errors.append(
            "promotion requires the independently pinned reviewed source-closure path and SHA-256"
        )
    else:
        source_identity_result = subprocess.run(
            [
                sys.executable,
                "-I",
                str(root / "scripts/kagemusha_source_tree_seal.py"),
                "identity",
                "--root",
                str(root),
                "--reviewed-source-closure",
                reviewed_closure_text,
                "--reviewed-source-closure-sha256",
                reviewed_closure_sha256,
            ],
            cwd=root,
            check=False,
            capture_output=True,
        )
        if source_identity_result.returncode != 0:
            errors.append(
                "promotion source differs from the independently pinned reviewed closure"
            )
        else:
            try:
                parsed_identity = json.loads(source_identity_result.stdout)
                if (
                    not isinstance(parsed_identity, dict)
                    or parsed_identity.get("schema")
                    != "iroha.kagemusha.reviewed_source_tree_identity.v1"
                    or parsed_identity.get("source_repo_dirty") is not False
                    or parsed_identity.get(
                        "reviewed_source_closure_descriptor_sha256"
                    )
                    != reviewed_closure_sha256
                    or not isinstance(
                        parsed_identity.get("reviewed_source_closure"), dict
                    )
                ):
                    raise ValueError("reviewed source identity is not exact")
                source_identity = parsed_identity
            except (UnicodeError, ValueError, json.JSONDecodeError):
                errors.append("promotion reviewed source identity is malformed")
    authenticated_verification_allowed = not errors

    catalog_directory_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    trusted_file_pins: list[tuple[Path, int, tuple[int, ...], str]] = []
    policy_sha256 = ""
    ios_validator: Callable[[Path, Path, str, Path], list[str]] | None = None
    ios_validator_path = root / IOS_EVIDENCE_MODULE
    verifier_snapshot: tempfile.TemporaryDirectory[str] | None = None
    verifier_exec = verifier
    try:
        seen_directories: set[Path] = set()
        trusted_roots = [artifact_root, policy.parent, verifier.parent]
        if ios_configuration is not None:
            trusted_roots.extend(
                [
                    ios_configuration[0],
                    ios_configuration[2].parent,
                    ios_validator_path.parent,
                ]
            )
        for trusted_root in trusted_roots:
            for path in absolute_directory_chain(trusted_root):
                if path in seen_directories:
                    continue
                seen_directories.add(path)
                label = f"trusted release path component {path}"
                descriptor, fingerprint = pin_directory_metadata(path, label)
                catalog_directory_pins.append((path, descriptor, fingerprint, label))
        label = f"release policy {policy}"
        descriptor, fingerprint = pin_regular_metadata(policy, label)
        trusted_file_pins.append((policy, descriptor, fingerprint, label))
        policy_sha256 = hash_pinned_descriptor(
            descriptor, fingerprint, 64 * 1024, label
        )
        label = f"Kagami release verifier {verifier}"
        descriptor, fingerprint = pin_regular_metadata(verifier, label)
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
            descriptor, fingerprint, label
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
        if ios_configuration is not None:
            public_key = ios_configuration[2]
            label = f"physical-iOS trusted public key {public_key}"
            descriptor, fingerprint = pin_regular_metadata(public_key, label)
            if fingerprint[4] > 64 * 1024:
                os.close(descriptor)
                raise ValueError("physical-iOS trusted public key is oversized")
            trusted_file_pins.append((public_key, descriptor, fingerprint, label))
            label = f"reviewed physical-iOS evidence validator {ios_validator_path}"
            descriptor, fingerprint = pin_regular_metadata(ios_validator_path, label)
            validator_bytes = read_pinned_descriptor(
                descriptor, fingerprint, 4 * 1024 * 1024, label
            )
            trusted_file_pins.append(
                (ios_validator_path, descriptor, fingerprint, label)
            )
            ios_validator = load_ios_evidence_validator(
                validator_bytes, ios_validator_path
            )
    except Exception as error:
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        if verifier_snapshot is not None:
            verifier_snapshot.cleanup()
        errors.append(f"promotion release trust path is not pinned: {error}")
        return errors

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
            if verifier_snapshot is not None:
                verifier_snapshot.cleanup()
            return errors
    directories.sort()
    if not directories:
        errors.append("promotion artifact root contains no manifest-digest releases")
        for _, descriptor, _, _ in catalog_directory_pins:
            os.close(descriptor)
        for _, descriptor, _, _ in trusted_file_pins:
            os.close(descriptor)
        if verifier_snapshot is not None:
            verifier_snapshot.cleanup()
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
                if verifier_snapshot is not None:
                    verifier_snapshot.cleanup()
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
            if verifier_snapshot is not None:
                verifier_snapshot.cleanup()
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
        try:
            release_bytes = 0
            for name in expected_inventory:
                path = directory / name
                label = f"{directory.name}/{name}"
                descriptor, fingerprint = pin_regular_metadata(path, label)
                new_pins.append((path, descriptor, fingerprint, label))
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
            manifest_bytes = read_regular_bounded(
                directory / "manifest.norito", MAX_MANIFEST_BYTES, "manifest.norito"
            )
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest.norito: {error}")
            continue
        digest = hashlib.sha256(manifest_bytes).hexdigest()
        if digest != directory.name:
            errors.append(f"{directory.name}: directory does not equal manifest SHA-256")
        try:
            sidecar = read_regular_bounded(
                directory / "manifest.norito.sha256",
                MAX_DIGEST_SIDECAR_BYTES,
                "manifest digest sidecar",
            )
            if sidecar != f"{digest}\n".encode("ascii"):
                errors.append(f"{directory.name}: manifest digest sidecar is not canonical")
        except (OSError, ValueError) as error:
            errors.append(f"{directory.name}: invalid manifest digest sidecar: {error}")
        try:
            manifest = strict_json(directory / "manifest.json")
        except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
            errors.append(f"{directory.name}: invalid manifest JSON: {error}")
            continue
        if manifest.get("schema") != "kagemusha.offline.recursive_spend.artifact_manifest.v4":
            errors.append(f"{directory.name}: manifest schema is not V4")
        if manifest.get("bridge_abi_version") != 22 or manifest.get("source_repo_dirty") is not False:
            errors.append(f"{directory.name}: ABI/source-tree promotion binding is invalid")
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
                        prefix = inspect_regular_prefix(
                            directory / name,
                            declared_artifacts[name],
                            MAX_DECLARED_ARTIFACT_FILE_BYTES,
                            8,
                            f"artifact {name}",
                        )
                        if prefix != b"KRV4KEY\0":
                            errors.append(f"{directory.name}/{name}: invalid KRV4 framing")
                    except (OSError, ValueError) as error:
                        errors.append(f"{directory.name}/{name}: invalid artifact: {error}")
        for name, maximum in BOUNDED_AUTHENTICATED_METADATA:
            try:
                payload = read_regular_bounded(directory / name, maximum, name)
                if name == "promotion-record-v4.norito":
                    promotion_record_sha256 = hashlib.sha256(payload).hexdigest()
            except (OSError, ValueError) as error:
                errors.append(f"{directory.name}/{name}: invalid evidence: {error}")
        try:
            if not evidence_is_non_placeholder(
                directory / "physical-device-benchmark.evidence",
                MAX_BENCHMARK_EVIDENCE_BYTES,
                "physical-device-benchmark.evidence",
            ):
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
            receipt = read_regular_bounded(
                directory / "recursive-step-two-qualification-v4.norito",
                MAX_QUALIFICATION_RECEIPT_BYTES,
                "recursive qualification receipt",
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
            and len(errors) == directory_error_count
        ):
            ios_candidate_sha256, ios_error = verify_ios_evidence(
                directory, ios_configuration, ios_validator
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
    if verifier_snapshot is not None:
        verifier_snapshot.cleanup()
    return errors


errors = static_errors()
if mode == "promotion":
    errors.extend(promotion_errors())

if self_test:
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
        PRIVACY: read(PRIVACY, []),
        PRIVACY_PROTOCOL: read(PRIVACY_PROTOCOL, []),
        CATALOG: read(CATALOG, []),
        CORE: read(CORE, []),
        KAGAMI: read(KAGAMI, []),
        BUNDLE: read(BUNDLE, []),
        WORKFLOW: read(WORKFLOW, []),
    }
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

if errors:
    print(
        f"Kagemusha ABI-21/V4 (native bridge ABI 22) {mode} corridor failed:",
        file=sys.stderr,
    )
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print(f"Kagemusha ABI-21/V4 (native bridge ABI 22) {mode} corridor passed.")
PY
