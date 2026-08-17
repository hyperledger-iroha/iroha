#!/usr/bin/env python3
"""Produce or reconstruct one authenticated Kagemusha V4 source projection.

Stable Cargo cannot emit its internal unit graph.  This producer therefore does
not approximate one from ``cargo metadata``.  It accepts a canonical normalized
Cargo unit-graph artifact plus an execution policy only when a separate SSH
controller has signed an exact authorization binding those artifacts to the
reviewed clean source closure.  Every source fact in the resulting projection
is still derived locally from the verified signed commit.
"""

from __future__ import annotations

import sys

# A clean Kagemusha checkout rejects Python cache files as untracked source.
sys.dont_write_bytecode = True

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
from typing import Any, Callable, Sequence

REPO_ROOT = Path(__file__).resolve().parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts import build_kagemusha_v4_candidate_bundle as builder
from scripts import kagemusha_source_tree_seal as source_seal


AUTHORIZATION_SCHEMA = (
    "iroha.kagemusha.source_seal_projection_controller_authorization.v1"
)
PRODUCTION_RECEIPT_SCHEMA = (
    "iroha.kagemusha.source_seal_projection_production_receipt.v1"
)
REQUEST_RECEIPT_SCHEMA = "iroha.kagemusha.source_seal_projection_request_receipt.v1"
SIGNATURE_NAMESPACE = "iroha-kagemusha-source-seal-projection-v1"
MAX_AUTHORIZATION_BYTES = 64 * 1024
MAX_EXECUTION_POLICY_BYTES = 16 * 1024 * 1024
MAX_UNIT_GRAPH_BYTES = 16 * 1024 * 1024
MAX_SIGNATURE_BYTES = 64 * 1024
MAX_GRAPH_STRING_BYTES = 4096
UNIT_GRAPH_KEYS = {"roots", "units", "version"}
UNIT_KEYS = {
    "dependencies",
    "features",
    "mode",
    "pkg_id",
    "platform",
    "profile",
    "target",
}
TARGET_BASE_KEYS = {
    "crate_types",
    "doc",
    "doctest",
    "edition",
    "kind",
    "name",
    "src_path",
    "test",
}
TARGET_OPTIONAL_KEYS = {"required-features"}
PROFILE_KEYS = {
    "codegen_backend",
    "codegen_units",
    "debug_assertions",
    "debuginfo",
    "incremental",
    "lto",
    "name",
    "opt_level",
    "overflow_checks",
    "panic",
    "rpath",
    "split_debuginfo",
    "strip",
}
DEPENDENCY_KEYS = {"extern_crate_name", "index", "noprelude", "public"}
ROOT_REQUIRED_FEATURES = (
    "dev-tools",
    "kagemusha-candidate-evidence-lab",
    "zk-halo2-ipa",
)
AUTHORIZATION_KEYS = {
    "execution_policy_sha256",
    "projection_schema",
    "reviewed_source_closure_sha256",
    "schema",
    "source_commit",
    "source_tree_sha256",
    "unit_graph",
}
UNIT_GRAPH_SUMMARY_KEYS = {
    "custom_build_packages",
    "custom_build_units",
    "iroha_core_units",
    "normalization",
    "packages",
    "sha256",
    "size_bytes",
    "units",
}


class ProjectionProductionError(RuntimeError):
    """Authenticated source-projection production failed closed."""


@dataclass(frozen=True)
class ControllerTrust:
    """Exact SSH controller identity and policy digests used for authorization."""

    principal: str
    public_key_sha256: str
    allowed_signers_sha256: str
    revocation_sha256: str
    signature_sha256: str


@dataclass(frozen=True)
class ProjectionProduction:
    """Deterministic projection bytes and their path-free production receipt."""

    projection: dict[str, Any]
    projection_bytes: bytes
    projection_sha256: str
    receipt: dict[str, Any]


@dataclass(frozen=True)
class DerivedProjectionInputs:
    """Locally reconstructed inputs for an authorization or projection."""

    identity: source_seal.SourceIdentity
    authorization: dict[str, Any]
    authorization_bytes: bytes
    execution_policy: bytes
    unit_graph: dict[str, Any]


def _strict_canonical_json(payload: bytes, label: str) -> Any:
    """Decode one duplicate-free canonical ASCII JSON line."""

    try:
        value = json.loads(
            payload,
            object_pairs_hook=builder._reject_duplicate_json_members,
            parse_constant=builder._reject_nonfinite_json_number,
        )
    except builder.CandidateBuildError as error:
        raise ProjectionProductionError(str(error)) from error
    except (json.JSONDecodeError, UnicodeError, ValueError) as error:
        raise ProjectionProductionError(f"{label} is not strict JSON") from error
    try:
        canonical = builder._canonical_json_line(value)
    except builder.CandidateBuildError as error:
        raise ProjectionProductionError(str(error)) from error
    if canonical != payload:
        raise ProjectionProductionError(f"{label} is not canonical JSON")
    return value


def _digest(value: Any, length: int, label: str) -> str:
    try:
        return builder._nonzero_lower_hex(value, length, label)
    except builder.CandidateBuildError as error:
        raise ProjectionProductionError(str(error)) from error


def _object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    try:
        return builder._exact_object(value, keys, label)
    except builder.CandidateBuildError as error:
        raise ProjectionProductionError(str(error)) from error


def _read_pinned_file(
    path: Path,
    expected_sha256: str,
    label: str,
    maximum_bytes: int,
    *,
    allow_empty: bool,
) -> bytes:
    """Read one owner-controlled regular file and verify its external pin."""

    expected_sha256 = _digest(expected_sha256, 64, f"{label} SHA-256 pin")
    try:
        payload = source_seal._read_bounded_absolute_file(
            path,
            label,
            maximum_bytes,
            allow_empty=allow_empty,
            owner_controlled=True,
        )
    except source_seal.SourceSealError as error:
        raise ProjectionProductionError(str(error)) from error
    if hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise ProjectionProductionError(f"{label} digest differs from its pin")
    return payload


def _bounded_ascii(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > MAX_GRAPH_STRING_BYTES
        or not value.isascii()
        or value.strip() != value
    ):
        raise ProjectionProductionError(f"{label} is not bounded canonical ASCII")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise ProjectionProductionError(f"{label} contains a control character")
    return value


def _relative_source_path(value: Any, label: str) -> str:
    path = _bounded_ascii(value, label)
    if (
        path.startswith(("/", "\\"))
        or "\\" in path
        or ":" in path.split("/", 1)[0]
        or any(component in ("", ".", "..") for component in path.split("/"))
    ):
        raise ProjectionProductionError(
            f"{label} is not a package-root-relative normalized path"
        )
    return path


def _string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        raise ProjectionProductionError(f"{label} is not a JSON string array")
    result = [_bounded_ascii(item, f"{label} item") for item in value]
    if result != sorted(set(result)):
        raise ProjectionProductionError(f"{label} is not unique and sorted")
    return result


def _require_projection_byte_bound(payload: bytes) -> None:
    """Enforce the single 16 KiB producer, consumer, and promotion bound."""

    if not 1 <= len(payload) <= builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES:
        raise ProjectionProductionError("produced source projection exceeds its byte bound")


def _validate_normalized_unit_graph(payload: bytes) -> dict[str, Any]:
    """Validate structural facts available from a genuine normalized graph.

    Cargo documents ``pkg_id`` as opaque.  The exact selected root nevertheless
    identifies the ``iroha_core`` package instance because the sealed semantic
    command selects that package and binary, so its other unit instances can be
    counted by opaque-id equality without parsing or fabricating package facts.
    """

    graph = _object(
        _strict_canonical_json(payload, "normalized Cargo unit graph"),
        UNIT_GRAPH_KEYS,
        "normalized Cargo unit graph",
    )
    if graph["version"] != 1:
        raise ProjectionProductionError("normalized Cargo unit-graph version is not 1")
    units = graph["units"]
    if not isinstance(units, list) or not 1 <= len(units) <= 100_000:
        raise ProjectionProductionError("normalized Cargo unit count is outside its bound")
    roots = graph["roots"]
    if not isinstance(roots, list) or not roots:
        raise ProjectionProductionError("normalized Cargo unit graph has no root")
    if any(type(index) is not int or not 0 <= index < len(units) for index in roots):
        raise ProjectionProductionError("normalized Cargo root index is invalid")
    if roots != sorted(set(roots)):
        raise ProjectionProductionError("normalized Cargo roots are not unique and sorted")
    if len(roots) != 1:
        raise ProjectionProductionError("Kagemusha Cargo unit graph must have one root")

    packages: set[str] = set()
    custom_build_packages: set[str] = set()
    custom_build_units = 0
    edges: list[list[int]] = []
    parsed_units: list[dict[str, Any]] = []
    for unit_index, raw_unit in enumerate(units):
        if not isinstance(raw_unit, dict) or set(raw_unit) != UNIT_KEYS:
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} fields are not exact"
            )
        pkg_id = _bounded_ascii(raw_unit["pkg_id"], f"unit {unit_index} pkg_id")
        packages.add(pkg_id)
        target = raw_unit["target"]
        if not isinstance(target, dict) or not (
            TARGET_BASE_KEYS <= set(target)
            and set(target) <= TARGET_BASE_KEYS | TARGET_OPTIONAL_KEYS
        ):
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} target fields are not exact"
            )
        kinds = _string_list(target["kind"], f"unit {unit_index} target kinds")
        _string_list(
            target["crate_types"], f"unit {unit_index} target crate types"
        )
        target_name = _bounded_ascii(
            target["name"], f"unit {unit_index} target name"
        )
        _relative_source_path(
            target["src_path"], f"unit {unit_index} target source path"
        )
        _bounded_ascii(target["edition"], f"unit {unit_index} target edition")
        if any(
            type(target[field]) is not bool for field in ("doc", "test", "doctest")
        ):
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} target booleans are invalid"
            )
        required_features = (
            _string_list(
                target["required-features"],
                f"unit {unit_index} target required features",
            )
            if "required-features" in target
            else None
        )
        profile = raw_unit["profile"]
        if not isinstance(profile, dict) or set(profile) != PROFILE_KEYS:
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} profile fields are not exact"
            )
        _bounded_ascii(profile["name"], f"unit {unit_index} profile name")
        _bounded_ascii(profile["opt_level"], f"unit {unit_index} profile opt level")
        _bounded_ascii(profile["lto"], f"unit {unit_index} profile LTO")
        _bounded_ascii(profile["panic"], f"unit {unit_index} profile panic")
        _bounded_ascii(profile["strip"]["resolved"]["Named"], f"unit {unit_index} profile strip") if (
            isinstance(profile["strip"], dict)
            and set(profile["strip"]) == {"resolved"}
            and isinstance(profile["strip"]["resolved"], dict)
            and set(profile["strip"]["resolved"]) == {"Named"}
        ) else (_ for _ in ()).throw(
            ProjectionProductionError(
                f"normalized Cargo unit {unit_index} profile strip is invalid"
            )
        )
        if profile["strip"]["resolved"]["Named"] not in (
            "none",
            "debuginfo",
            "symbols",
        ):
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} profile strip is invalid"
            )
        for optional_string in ("codegen_backend", "split_debuginfo"):
            value = profile[optional_string]
            if value is not None:
                _bounded_ascii(
                    value, f"unit {unit_index} profile {optional_string}"
                )
        for optional_integer in ("codegen_units", "debuginfo"):
            value = profile[optional_integer]
            if value is not None and (type(value) is not int or not 0 <= value <= 2**31 - 1):
                raise ProjectionProductionError(
                    f"normalized Cargo unit {unit_index} profile {optional_integer} is invalid"
                )
        for boolean in (
            "debug_assertions",
            "overflow_checks",
            "rpath",
            "incremental",
        ):
            if type(profile[boolean]) is not bool:
                raise ProjectionProductionError(
                    f"normalized Cargo unit {unit_index} profile {boolean} is invalid"
                )
        mode = _bounded_ascii(raw_unit["mode"], f"unit {unit_index} mode")
        if mode not in ("build", "run-custom-build"):
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} mode is not admitted"
            )
        _string_list(raw_unit["features"], f"unit {unit_index} features")
        platform = raw_unit["platform"]
        if platform is not None:
            platform = _bounded_ascii(platform, f"unit {unit_index} platform")

        dependencies = raw_unit["dependencies"]
        if not isinstance(dependencies, list):
            raise ProjectionProductionError(
                f"normalized Cargo unit {unit_index} dependencies are not an array"
            )
        dependency_indices: list[int] = []
        dependency_order: list[tuple[int, str]] = []
        for dependency_index, dependency in enumerate(dependencies):
            if not isinstance(dependency, dict) or set(dependency) != DEPENDENCY_KEYS:
                raise ProjectionProductionError(
                    f"unit {unit_index} dependency {dependency_index} fields are not exact"
                )
            target_index = dependency["index"]
            if (
                type(target_index) is not int
                or not 0 <= target_index < len(units)
                or target_index == unit_index
            ):
                raise ProjectionProductionError(
                    f"unit {unit_index} dependency {dependency_index} index is invalid"
                )
            extern_name = _bounded_ascii(
                dependency["extern_crate_name"],
                f"unit {unit_index} dependency {dependency_index} extern name",
            )
            for boolean in ("noprelude", "public"):
                if type(dependency[boolean]) is not bool:
                    raise ProjectionProductionError(
                        f"unit {unit_index} dependency {dependency_index} {boolean} is not boolean"
                    )
            dependency_indices.append(target_index)
            dependency_order.append((target_index, extern_name))
        if dependency_order != sorted(set(dependency_order)):
            raise ProjectionProductionError(
                f"unit {unit_index} dependencies are not unique and sorted"
            )
        edges.append(dependency_indices)
        parsed_units.append(
            {
                "features": raw_unit["features"],
                "kinds": kinds,
                "mode": mode,
                "pkg_id": pkg_id,
                "platform": platform,
                "profile": profile,
                "required_features": required_features,
                "target_name": target_name,
            }
        )
        if "custom-build" in kinds or mode == "run-custom-build":
            custom_build_units += 1
            custom_build_packages.add(pkg_id)

    reachable: set[int] = set()
    pending = list(roots)
    while pending:
        index = pending.pop()
        if index in reachable:
            continue
        reachable.add(index)
        pending.extend(edges[index])
    if len(reachable) != len(units):
        raise ProjectionProductionError("normalized Cargo unit graph contains unreachable units")

    root = parsed_units[roots[0]]
    if (
        root["target_name"] != builder.BINARY_NAME
        or root["kinds"] != ["bin"]
        or root["mode"] != "build"
        or root["platform"] != builder.SOURCE_SEAL_TARGET
        or root["profile"].get("name") != "release"
        or root["profile"].get("opt_level") != "3"
        or root["profile"].get("debug_assertions") is not False
        or root["required_features"] != list(ROOT_REQUIRED_FEATURES)
        or root["features"] != list(builder.SOURCE_SEAL_RESOLVED_FEATURES)
    ):
        raise ProjectionProductionError(
            "normalized Cargo root is not the exact Kagemusha release binary unit"
        )

    return {
        "custom_build_packages": len(custom_build_packages),
        "custom_build_units": custom_build_units,
        "iroha_core_units": sum(
            unit["pkg_id"] == root["pkg_id"] for unit in parsed_units
        ),
        "normalization": builder.SOURCE_SEAL_UNIT_GRAPH_NORMALIZATION,
        "packages": len(packages),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "size_bytes": len(payload),
        "units": len(units),
    }


def _verify_controller_authorization(
    authorization: bytes,
    signature: bytes,
    allowed_signers: bytes,
    revocation: bytes,
) -> ControllerTrust:
    """Verify one detached SSH authorization under an exact single-key policy."""

    if not source_seal.SSH_KEYGEN.is_file() or source_seal.SSH_KEYGEN.is_symlink():
        raise ProjectionProductionError("pinned /usr/bin/ssh-keygen is unavailable")
    try:
        principal, public_key_sha256 = source_seal._validate_allowed_signers(
            allowed_signers
        )
    except source_seal.SourceSealError as error:
        raise ProjectionProductionError(str(error)) from error
    if (
        signature.count(b"-----BEGIN SSH SIGNATURE-----") != 1
        or signature.count(b"-----END SSH SIGNATURE-----") != 1
    ):
        raise ProjectionProductionError("controller signature is not one SSH signature")
    try:
        with tempfile.TemporaryDirectory(
            prefix="iroha-kagemusha-projection-controller-"
        ) as temporary:
            directory = Path(temporary)
            allowed_path = source_seal._write_private_policy_snapshot(
                directory, "allowed-signers", allowed_signers
            )
            revocation_path = source_seal._write_private_policy_snapshot(
                directory, "revocation", revocation
            )
            signature_path = source_seal._write_private_policy_snapshot(
                directory, "authorization.sig", signature
            )
            completed = subprocess.run(
                [
                    os.fspath(source_seal.SSH_KEYGEN),
                    "-Y",
                    "verify",
                    "-f",
                    os.fspath(allowed_path),
                    "-I",
                    principal,
                    "-n",
                    SIGNATURE_NAMESPACE,
                    "-s",
                    os.fspath(signature_path),
                    "-r",
                    os.fspath(revocation_path),
                ],
                input=authorization,
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                env=source_seal._git_environment(),
            )
    except (OSError, source_seal.SourceSealError) as error:
        raise ProjectionProductionError(
            "could not verify the controller authorization"
        ) from error
    if completed.returncode != 0:
        raise ProjectionProductionError(
            "controller authorization does not have one trusted SSH signature"
        )
    return ControllerTrust(
        principal=principal,
        public_key_sha256=public_key_sha256,
        allowed_signers_sha256=hashlib.sha256(allowed_signers).hexdigest(),
        revocation_sha256=hashlib.sha256(revocation).hexdigest(),
        signature_sha256=hashlib.sha256(signature).hexdigest(),
    )


def _source_authority_projection(
    authority: source_seal.SourceAuthority,
) -> dict[str, Any]:
    if len(authority.ordered_parents) != 1 or len(authority.ordered_parent_trees) != 1:
        raise ProjectionProductionError(
            "Kagemusha projection requires exactly one authorized source parent"
        )
    return {
        "commit": authority.commit,
        "commit_object_sha256": authority.commit_object_sha256,
        "commit_object_size": authority.commit_object_size,
        "committer_epoch": authority.committer_epoch,
        "git_tree": authority.git_tree,
        "ordered_parents": list(authority.ordered_parents),
        "parent_commit": authority.ordered_parents[0],
        "parent_tree": authority.ordered_parent_trees[0],
        "signature": {
            "allowed_signers_sha256": authority.signature.allowed_signers_sha256,
            "mechanism": "git-commit-ssh-signature-v1",
            "principal": authority.signature.principal,
            "public_key_sha256": authority.signature.public_key_sha256,
            "revocation_sha256": authority.signature.revocation_sha256,
            "signature_namespace": "git",
        },
    }


def _derive_projection_inputs(
    root: Path,
    *,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    execution_policy_path: Path,
    execution_policy_sha256: str,
    unit_graph_path: Path,
    unit_graph_sha256: str,
    identity_reader: Callable[
        [Path, str, str], source_seal.SourceIdentity
    ] = source_seal.compute_identity,
) -> DerivedProjectionInputs:
    """Derive the exact unsigned controller request from pinned local inputs."""

    root = root.resolve(strict=True)
    reviewed_source_closure = reviewed_source_closure.resolve(strict=True)
    identity = identity_reader(
        root,
        str(reviewed_source_closure),
        reviewed_source_closure_sha256,
    )
    if identity.source_repo_dirty:
        raise ProjectionProductionError("source projection requires a clean source identity")
    execution_policy = _read_pinned_file(
        execution_policy_path,
        execution_policy_sha256,
        "projection execution policy",
        MAX_EXECUTION_POLICY_BYTES,
        allow_empty=False,
    )
    _strict_canonical_json(execution_policy, "projection execution policy")
    unit_graph_bytes = _read_pinned_file(
        unit_graph_path,
        unit_graph_sha256,
        "normalized Cargo unit graph",
        MAX_UNIT_GRAPH_BYTES,
        allow_empty=False,
    )
    unit_graph = _validate_normalized_unit_graph(unit_graph_bytes)
    closure_bytes = builder._canonical_json_line(identity.reviewed_source_closure)
    authorization = {
        "execution_policy_sha256": hashlib.sha256(execution_policy).hexdigest(),
        "projection_schema": builder.AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA,
        "reviewed_source_closure_sha256": hashlib.sha256(closure_bytes).hexdigest(),
        "schema": AUTHORIZATION_SCHEMA,
        "source_commit": identity.source_commit,
        "source_tree_sha256": identity.source_tree_sha256,
        "unit_graph": unit_graph,
    }
    return DerivedProjectionInputs(
        identity=identity,
        authorization=authorization,
        authorization_bytes=builder._canonical_json_line(authorization),
        execution_policy=execution_policy,
        unit_graph=unit_graph,
    )


def construct_authorization_request(
    root: Path,
    *,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    execution_policy_path: Path,
    execution_policy_sha256: str,
    unit_graph_path: Path,
    unit_graph_sha256: str,
    identity_reader: Callable[
        [Path, str, str], source_seal.SourceIdentity
    ] = source_seal.compute_identity,
) -> tuple[dict[str, Any], bytes]:
    """Return the canonical request that the external controller must sign."""

    derived = _derive_projection_inputs(
        root,
        reviewed_source_closure=reviewed_source_closure,
        reviewed_source_closure_sha256=reviewed_source_closure_sha256,
        execution_policy_path=execution_policy_path,
        execution_policy_sha256=execution_policy_sha256,
        unit_graph_path=unit_graph_path,
        unit_graph_sha256=unit_graph_sha256,
        identity_reader=identity_reader,
    )
    return derived.authorization, derived.authorization_bytes


def construct_projection(
    root: Path,
    *,
    reviewed_source_closure: Path,
    reviewed_source_closure_sha256: str,
    authorization_path: Path,
    authorization_sha256: str,
    controller_signature_path: Path,
    controller_signature_sha256: str,
    controller_allowed_signers_path: Path,
    controller_allowed_signers_sha256: str,
    controller_revocation_path: Path,
    controller_revocation_sha256: str,
    execution_policy_path: Path,
    execution_policy_sha256: str,
    unit_graph_path: Path,
    unit_graph_sha256: str,
    identity_reader: Callable[
        [Path, str, str], source_seal.SourceIdentity
    ] = source_seal.compute_identity,
    signature_verifier: Callable[
        [bytes, bytes, bytes, bytes], ControllerTrust
    ] = _verify_controller_authorization,
) -> ProjectionProduction:
    """Construct the exact consumer-facing projection from authenticated inputs."""

    derived = _derive_projection_inputs(
        root,
        reviewed_source_closure=reviewed_source_closure,
        reviewed_source_closure_sha256=reviewed_source_closure_sha256,
        execution_policy_path=execution_policy_path,
        execution_policy_sha256=execution_policy_sha256,
        unit_graph_path=unit_graph_path,
        unit_graph_sha256=unit_graph_sha256,
        identity_reader=identity_reader,
    )
    identity = derived.identity

    authorization_bytes = _read_pinned_file(
        authorization_path,
        authorization_sha256,
        "projection controller authorization",
        MAX_AUTHORIZATION_BYTES,
        allow_empty=False,
    )
    signature = _read_pinned_file(
        controller_signature_path,
        controller_signature_sha256,
        "projection controller signature",
        MAX_SIGNATURE_BYTES,
        allow_empty=False,
    )
    allowed_signers = _read_pinned_file(
        controller_allowed_signers_path,
        controller_allowed_signers_sha256,
        "projection controller allowed-signers policy",
        source_seal.MAX_ALLOWED_SIGNERS_BYTES,
        allow_empty=False,
    )
    revocation = _read_pinned_file(
        controller_revocation_path,
        controller_revocation_sha256,
        "projection controller revocation policy",
        source_seal.MAX_REVOCATION_BYTES,
        allow_empty=True,
    )
    controller = signature_verifier(
        authorization_bytes,
        signature,
        allowed_signers,
        revocation,
    )
    authorization = _object(
        _strict_canonical_json(
            authorization_bytes, "projection controller authorization"
        ),
        AUTHORIZATION_KEYS,
        "projection controller authorization",
    )
    authorized_graph = _object(
        authorization["unit_graph"],
        UNIT_GRAPH_SUMMARY_KEYS,
        "authorized Cargo unit graph",
    )
    complete_graph = derived.unit_graph
    if authorized_graph != complete_graph:
        raise ProjectionProductionError(
            "controller-authorized Cargo unit graph differs from the supplied graph"
        )

    closure_bytes = builder._canonical_json_line(identity.reviewed_source_closure)
    closure_sha256 = hashlib.sha256(closure_bytes).hexdigest()
    expected_authorization = derived.authorization
    if authorization != expected_authorization:
        raise ProjectionProductionError(
            "controller authorization differs from the verified source and policy inputs"
        )

    projection = {
        "build_script_observed": {
            "debug_assertions": False,
            "features": list(builder.SOURCE_SEAL_RESOLVED_FEATURES),
            "host": builder.SOURCE_SEAL_TARGET,
            "num_jobs": 1,
            "opt_level": "3",
            "profile": "release",
            "schema": builder.SOURCE_SEAL_BUILD_SCRIPT_OBSERVED_SCHEMA,
            "target": builder.SOURCE_SEAL_TARGET,
        },
        "outer_policy": {
            "cargo": {
                "binary": builder.BINARY_NAME,
                "explicit_features": list(builder.SOURCE_SEAL_EXPLICIT_FEATURES),
                "package": "iroha_core",
                "profile": "release",
                "semantic_argv": list(builder.SOURCE_SEAL_SEMANTIC_ARGV),
                "target": builder.SOURCE_SEAL_TARGET,
                "unit_graph": complete_graph,
            },
            "execution_policy_sha256": expected_authorization[
                "execution_policy_sha256"
            ],
            "schema": builder.SOURCE_SEAL_OUTER_POLICY_SCHEMA,
        },
        "reviewed_source_closure_hex": closure_bytes.hex(),
        "reviewed_source_closure_sha256": closure_sha256,
        "schema": builder.AUTHENTICATED_SOURCE_SEAL_PROJECTION_SCHEMA,
        "source_authority": _source_authority_projection(identity.source_authority),
        "source_commit": identity.source_commit,
        "source_date_epoch": identity.source_authority.committer_epoch,
        "source_repo_dirty": False,
        "source_tree_sha256": identity.source_tree_sha256,
    }
    projection_bytes = builder._canonical_json_line(projection)
    _require_projection_byte_bound(projection_bytes)
    projection_sha256 = hashlib.sha256(projection_bytes).hexdigest()
    try:
        builder._projection_build_environment(
            projection,
            projection_bytes,
            projection_sha256,
            identity,
        )
    except builder.CandidateBuildError as error:
        raise ProjectionProductionError(
            f"produced projection fails the candidate consumer: {error}"
        ) from error

    receipt = {
        "authorization_sha256": hashlib.sha256(authorization_bytes).hexdigest(),
        "controller_allowed_signers_sha256": controller.allowed_signers_sha256,
        "controller_principal": controller.principal,
        "controller_public_key_sha256": controller.public_key_sha256,
        "controller_revocation_sha256": controller.revocation_sha256,
        "controller_signature_sha256": controller.signature_sha256,
        "execution_policy_sha256": expected_authorization[
            "execution_policy_sha256"
        ],
        "projection_hex": projection_bytes.hex(),
        "projection_sha256": projection_sha256,
        "schema": PRODUCTION_RECEIPT_SCHEMA,
        "source_commit": identity.source_commit,
        "source_tree_sha256": identity.source_tree_sha256,
        "unit_graph_sha256": complete_graph["sha256"],
    }
    return ProjectionProduction(
        projection=projection,
        projection_bytes=projection_bytes,
        projection_sha256=projection_sha256,
        receipt=receipt,
    )


def _write_new_private_file(path: Path, payload: bytes) -> None:
    """Publish one new projection without following or replacing a path."""

    path_text = os.fspath(path)
    if not path.is_absolute() or os.path.normpath(path_text) != path_text:
        raise ProjectionProductionError("projection output path must be absolute and normalized")
    parent = path.parent
    try:
        parent_metadata = parent.lstat()
        if (
            not stat.S_ISDIR(parent_metadata.st_mode)
            or stat.S_ISLNK(parent_metadata.st_mode)
            or parent.resolve(strict=True) != parent
            or parent_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(parent_metadata.st_mode) & 0o077
        ):
            raise ProjectionProductionError(
                "projection output parent must be an owner-private canonical directory"
            )
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        descriptor = os.open(path, flags, 0o600)
        try:
            offset = 0
            while offset < len(payload):
                written = os.write(descriptor, payload[offset:])
                if written <= 0:
                    raise ProjectionProductionError("could not publish source projection")
                offset += written
            os.fsync(descriptor)
            metadata = os.fstat(descriptor)
            if (
                not stat.S_ISREG(metadata.st_mode)
                or metadata.st_nlink != 1
                or metadata.st_uid != os.geteuid()
                or stat.S_IMODE(metadata.st_mode) != 0o600
                or metadata.st_size != len(payload)
            ):
                raise ProjectionProductionError(
                    "published source projection has unsafe metadata"
                )
        finally:
            os.close(descriptor)
    except ProjectionProductionError:
        raise
    except OSError as error:
        raise ProjectionProductionError(
            "projection output is unavailable or already exists"
        ) from error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("request", "produce", "verify"))
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--reviewed-source-closure", type=Path, required=True)
    parser.add_argument("--reviewed-source-closure-sha256", required=True)
    parser.add_argument("--authorization", type=Path)
    parser.add_argument("--authorization-sha256")
    parser.add_argument("--controller-signature", type=Path)
    parser.add_argument("--controller-signature-sha256")
    parser.add_argument("--controller-allowed-signers", type=Path)
    parser.add_argument("--controller-allowed-signers-sha256")
    parser.add_argument("--controller-revocation", type=Path)
    parser.add_argument("--controller-revocation-sha256")
    parser.add_argument("--execution-policy", type=Path, required=True)
    parser.add_argument("--execution-policy-sha256", required=True)
    parser.add_argument("--unit-graph", type=Path, required=True)
    parser.add_argument("--unit-graph-sha256", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--projection", type=Path)
    parser.add_argument("--projection-sha256")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Produce or independently reconstruct the exact projection bytes."""

    args = _parser().parse_args(argv)
    controller_arguments = (
        args.authorization,
        args.authorization_sha256,
        args.controller_signature,
        args.controller_signature_sha256,
        args.controller_allowed_signers,
        args.controller_allowed_signers_sha256,
        args.controller_revocation,
        args.controller_revocation_sha256,
    )
    if args.mode == "request":
        if (
            args.output is None
            or args.projection is not None
            or args.projection_sha256
            or any(value is not None for value in controller_arguments)
        ):
            raise ProjectionProductionError(
                "request requires --output and forbids controller and projection inputs"
            )
        _, request_bytes = construct_authorization_request(
            args.root,
            reviewed_source_closure=args.reviewed_source_closure,
            reviewed_source_closure_sha256=args.reviewed_source_closure_sha256,
            execution_policy_path=args.execution_policy,
            execution_policy_sha256=args.execution_policy_sha256,
            unit_graph_path=args.unit_graph,
            unit_graph_sha256=args.unit_graph_sha256,
        )
        _write_new_private_file(args.output, request_bytes)
        sys.stdout.buffer.write(
            builder._canonical_json_line(
                {
                    "authorization_sha256": hashlib.sha256(request_bytes).hexdigest(),
                    "schema": REQUEST_RECEIPT_SCHEMA,
                }
            )
        )
        return 0
    if any(value is None for value in controller_arguments):
        raise ProjectionProductionError(
            "produce and verify require every controller authorization input and pin"
        )
    if args.mode == "produce":
        if args.output is None or args.projection is not None or args.projection_sha256:
            raise ProjectionProductionError(
                "produce requires --output and forbids verify projection inputs"
            )
    elif args.output is not None or args.projection is None or not args.projection_sha256:
        raise ProjectionProductionError(
            "verify requires --projection and --projection-sha256 and forbids --output"
        )
    assert args.authorization is not None
    assert args.authorization_sha256 is not None
    assert args.controller_signature is not None
    assert args.controller_signature_sha256 is not None
    assert args.controller_allowed_signers is not None
    assert args.controller_allowed_signers_sha256 is not None
    assert args.controller_revocation is not None
    assert args.controller_revocation_sha256 is not None
    production = construct_projection(
        args.root,
        reviewed_source_closure=args.reviewed_source_closure,
        reviewed_source_closure_sha256=args.reviewed_source_closure_sha256,
        authorization_path=args.authorization,
        authorization_sha256=args.authorization_sha256,
        controller_signature_path=args.controller_signature,
        controller_signature_sha256=args.controller_signature_sha256,
        controller_allowed_signers_path=args.controller_allowed_signers,
        controller_allowed_signers_sha256=args.controller_allowed_signers_sha256,
        controller_revocation_path=args.controller_revocation,
        controller_revocation_sha256=args.controller_revocation_sha256,
        execution_policy_path=args.execution_policy,
        execution_policy_sha256=args.execution_policy_sha256,
        unit_graph_path=args.unit_graph,
        unit_graph_sha256=args.unit_graph_sha256,
    )
    if args.mode == "produce":
        assert args.output is not None
        _write_new_private_file(args.output, production.projection_bytes)
    else:
        assert args.projection is not None and args.projection_sha256 is not None
        supplied = _read_pinned_file(
            args.projection,
            args.projection_sha256,
            "authenticated source-seal projection",
            builder.MAX_AUTHENTICATED_SOURCE_SEAL_PROJECTION_BYTES,
            allow_empty=False,
        )
        if supplied != production.projection_bytes:
            raise ProjectionProductionError(
                "supplied source projection differs from deterministic reconstruction"
            )
    sys.stdout.buffer.write(builder._canonical_json_line(production.receipt))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        OSError,
        ProjectionProductionError,
        builder.CandidateBuildError,
        source_seal.SourceSealError,
    ) as error:
        print(f"Kagemusha source-projection production failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
