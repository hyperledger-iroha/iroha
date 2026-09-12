#!/usr/bin/env python3
"""Check a public allocation plan before Taira deployment writes anything.

Run on each storage host: ``python3 scripts/taira_disk_capacity.py --plan plan.json``.
The JSON contains only paths, labels, and conservative *additional allocated*
bytes/inodes, including temporary copies and explicit headroom. No credentials,
SSH, log/config contents, cleanup, reservations, or environment overrides are used.
Repeated paths on one filesystem add together. Existing artifacts still occupy
space and must never be deducted unless their removal has already been verified.
For sparse guest disks, run another plan on the physical backing host as well.
A successful observation is not a reservation; recheck immediately before apply.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

PLAN_SCHEMA = "taira.disk-capacity.plan.v1"
RESULT_SCHEMA = "taira.disk-capacity.result.v1"
MAX_PLAN_BYTES = 1024 * 1024
MAX_ALLOCATIONS = 1024
MAX_COUNT = (1 << 63) - 1


class CapacityError(ValueError):
    """Invalid allocation plan or uninspectable storage path."""


def count(value: object, label: str) -> int:
    """Require an explicit nonnegative bounded integer, excluding booleans."""
    if type(value) is not int or not 0 <= value <= MAX_COUNT:
        raise CapacityError(f"{label} must be an integer in 0..{MAX_COUNT}")
    return value


def direct_path(value: object) -> Path:
    """Reject noncanonical spelling before walking components without symlinks."""
    if not isinstance(value, str) or not value.startswith("/") or "\x00" in value:
        raise CapacityError("allocation path must be an absolute path")
    path = Path(value)
    if str(path) != value or any(part in (".", "..") for part in value.split("/")):
        raise CapacityError("allocation path must use canonical absolute spelling")
    return path


def validate_plan(value: object) -> list[dict[str, object]]:
    """Validate the exact public plan shape; no implicit safety margin is invented."""
    if (
        not isinstance(value, dict)
        or set(value) != {"schema", "allocations"}
        or value["schema"] != PLAN_SCHEMA
    ):
        raise CapacityError("expected exact taira.disk-capacity.plan.v1 object")
    rows = value["allocations"]
    if not isinstance(rows, list) or not 1 <= len(rows) <= MAX_ALLOCATIONS:
        raise CapacityError("plan must contain 1..1024 allocations")
    for row in rows:
        if not isinstance(row, dict) or set(row) != {
            "path",
            "label",
            "bytes",
            "inodes",
        }:
            raise CapacityError(
                "allocation must contain exactly path, label, bytes, inodes"
            )
        direct_path(row["path"])
        label = row["label"]
        if (
            not isinstance(label, str)
            or not 1 <= len(label) <= 200
            or any(ord(c) < 32 for c in label)
        ):
            raise CapacityError(
                "allocation label must be printable and 1..200 characters"
            )
        count(row["bytes"], "allocation bytes")
        count(row["inodes"], "allocation inodes")
    return rows


def inspect_filesystem(path: Path) -> dict[str, object]:
    """Read the nearest existing directory through a no-follow descriptor walk."""
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open("/", flags)
    anchor = Path("/")
    try:
        for component in path.parts[1:]:
            try:
                next_fd = os.open(component, flags, dir_fd=fd)
            except FileNotFoundError:
                break
            os.close(fd)
            fd = next_fd
            anchor /= component
        before = os.fstat(fd)
        space = os.fstatvfs(fd)
        after = os.fstat(fd)
        if (before.st_dev, before.st_ino) != (
            after.st_dev,
            after.st_ino,
        ) or not stat.S_ISDIR(after.st_mode):
            raise CapacityError("storage anchor changed during inspection")
        fragment = count(space.f_frsize or space.f_bsize, "filesystem allocation unit")
        if fragment == 0:
            raise CapacityError("filesystem allocation unit is zero")
        available_bytes = count(space.f_bavail * fragment, "filesystem available bytes")
        available_inodes = count(space.f_favail, "filesystem available inodes")
        return {
            "device": before.st_dev,
            "anchor": str(anchor),
            "fragment_bytes": fragment,
            "available_bytes": available_bytes,
            "available_inodes": available_inodes,
        }
    except OSError as error:
        raise CapacityError(
            f"cannot inspect allocation directory {path}: {error.strerror}"
        ) from None
    finally:
        os.close(fd)


def evaluate(plan: object, *, inspect=inspect_filesystem) -> dict[str, object]:
    """Sum simultaneous allocations per actual filesystem and report both limits."""
    devices: dict[int, dict[str, object]] = {}
    for allocation in validate_plan(plan):
        observation = inspect(direct_path(allocation["path"]))
        device = observation["device"]
        row = devices.setdefault(
            device,
            {
                **observation,
                "required_bytes": 0,
                "required_inodes": 0,
                "allocations": [],
            },
        )
        row["available_bytes"] = min(
            row["available_bytes"], observation["available_bytes"]
        )
        row["available_inodes"] = min(
            row["available_inodes"], observation["available_inodes"]
        )
        row["required_bytes"] = count(
            row["required_bytes"] + allocation["bytes"], "aggregate bytes"
        )
        row["required_inodes"] = count(
            row["required_inodes"] + allocation["inodes"], "aggregate inodes"
        )
        row["allocations"].append(dict(allocation))
    errors = []
    for row in devices.values():
        row["passed"] = True
        for resource in ("bytes", "inodes"):
            available, required = (
                row[f"available_{resource}"],
                row[f"required_{resource}"],
            )
            if required > available:
                row["passed"] = False
                errors.append(
                    f"{row['anchor']}: need {required} additional {resource}, available {available}"
                )
    return {
        "schema": RESULT_SCHEMA,
        "observed_unix_ns": time.time_ns(),
        "passed": not errors,
        "reservation_created": False,
        "filesystems": list(devices.values()),
        "errors": errors,
    }


def allocation_bound(
    payload_bytes: int, file_count: int, directory_count: int, fragment_bytes: int
) -> dict[str, int]:
    """Bound allocation using payload bytes plus maximum per-file slack and directories.

    Include metadata and simultaneously live temporary files in the supplied
    payload/file counts. Sparse files are charged at their full materialized size.
    This is a byte/inode bound, not an ext4 metadata/journal reserve; add explicit
    filesystem headroom separately.
    """
    for value, label in (
        (payload_bytes, "payload bytes"),
        (file_count, "file count"),
        (directory_count, "directory count"),
        (fragment_bytes, "fragment bytes"),
    ):
        count(value, label)
    if fragment_bytes == 0 or (payload_bytes and not file_count):
        raise CapacityError(
            "positive allocation unit and file count for payload required"
        )
    return {
        "bytes": count(
            payload_bytes
            + file_count * (fragment_bytes - 1)
            + directory_count * fragment_bytes,
            "allocation bound",
        ),
        "inodes": count(file_count + directory_count, "inode bound"),
    }


def cohost_peak_plan(
    *,
    coordinator_path: str,
    upload_path: str,
    service_path: str,
    store_paths: list[str],
    runtime_paths: list[str],
    artifacts: dict[str, int],
    stage: dict[str, int],
    per_store: dict[str, int],
    per_replica_runtime: dict[str, int],
    headroom: list[dict[str, object]],
) -> dict[str, object]:
    """Describe the fresh four-validator rollout peak: 3A + 2S + 4P + 4R.

    A is all validator AND edge artifact sets (one set per role), S the full Inrou
    stage tree, P one complete store INCLUDING manifest/PoR/index metadata and
    temporary publication overhead. R includes runtime guest hydration, a separate
    writable root disk, lease/ephemeral storage, bundle extraction/cache/block
    copies, and publication overhead for one replica. Runtime footprints are
    required even though they are allocated after preseed. Inputs are allocated
    byte/inode bounds, not logical quota. Existing inputs are already charged by
    statvfs. Installed release reuse is deliberately not credited. The four store
    paths and the four runtime paths must each be distinct.
    """
    if len(store_paths) != 4 or len(set(store_paths)) != 4:
        raise CapacityError("exactly four distinct cohost store paths required")
    if len(runtime_paths) != 4 or len(set(runtime_paths)) != 4:
        raise CapacityError("exactly four distinct cohost runtime paths required")
    for value in (artifacts, stage, per_store, per_replica_runtime):
        if not isinstance(value, dict) or set(value) != {"bytes", "inodes"}:
            raise CapacityError("footprints must contain exactly bytes and inodes")
        for field in value:
            count(value[field], field)

    def row(path, label, footprint):
        return {"path": path, "label": label, **footprint}

    allocations = [
        row(coordinator_path, "coordinator artifact snapshot", artifacts),
        row(upload_path, "per-role artifact uploads", artifacts),
        row(service_path, "per-role installed artifacts", artifacts),
        row(coordinator_path, "coordinator Inrou stage snapshot", stage),
        row(upload_path, "host-scoped Inrou stage upload", stage),
    ]
    allocations.extend(
        row(path, f"preseed store {i + 1}", per_store)
        for i, path in enumerate(store_paths)
    )
    allocations.extend(
        row(path, f"runtime replica {i + 1}", per_replica_runtime)
        for i, path in enumerate(runtime_paths)
    )
    allocations.extend(dict(item) for item in headroom)
    plan = {"schema": PLAN_SCHEMA, "allocations": allocations}
    validate_plan(plan)
    return plan


MIB = 1024 * 1024
GIB = 1024 * MIB


def _require_derivation(ok, message):
    if not ok:
        raise CapacityError(message)


def _positive_derivation(value, label):
    result = count(value, label)
    _require_derivation(result > 0, label + " must be positive")
    return result


def derive_capacity(
    inputs,
    runtime,
    build,
    *,
    expected_commit,
    coordinator_path,
    upload_path,
    service_path,
    store_paths,
    runtime_paths,
    guest_headroom_path,
    backing_path,
):
    """Pure 3A+2S+4P+4R calculation from metadata, retained SF1 preparation only.

    Config contents are never inputs. Each freshly rebased config is charged at
    the native materializer's 1 MiB output limit, not its previous measured size.
    Unknown stage/service layouts fail rather than silently omit new allocations.
    Binary weights come from all 26 native roles, not four unique uploaded files.
    """
    _require_derivation(
        re.fullmatch("[0-9a-f]{40}", expected_commit or "") is not None,
        "actual full commit required",
    )
    _require_derivation(
        build.get("commit") == expected_commit
        and "exit_code" not in build
        and build.get("source_unchanged") is True
        and build.get("toolchain_unchanged") is True
        and build.get("target") == "aarch64-unknown-linux-gnu"
        and build.get("profile") == "release"
        and build.get("jobs") == 6
        and build.get("deployed") is False
        and build.get("release_qualified") is False,
        "completed exact-commit maintained build receipt required",
    )
    names = {"iroha", "iroha3d_taira", "sorafs-node", "kagami"}
    artifacts = build.get("artifacts", [])
    _require_derivation(
        len(artifacts) == 4 and {row["name"] for row in artifacts} == names,
        "four distinct actual built binaries required",
    )
    sizes = {
        row["name"]: _positive_derivation(row["size"], "binary size")
        for row in artifacts
    }
    _require_derivation(
        inputs.get("schema") == "taira.public-capacity-inputs.v1"
        and inputs.get("secret_contents_read") is False
        and runtime.get("schema") == "taira.public-runtime-capacity-inputs.v1"
        and runtime.get("secret_contents_read") is False,
        "metadata-only capacity and runtime input receipts required",
    )
    bindings = inputs.get("native_sf1_manifest_bindings")
    _require_derivation(
        isinstance(bindings, dict)
        and set(bindings) == {"bundle", "guest", "discovery"}
        and all(
            isinstance(value, str) and re.fullmatch("[0-9a-f]{64}", value)
            for value in bindings.values()
        ),
        "existing native SF1 admission must bind all three current manifests",
    )
    fragment = _positive_derivation(
        inputs["filesystem"]["fragment_bytes"], "guest allocation unit"
    )
    _require_derivation(
        fragment == 4096,
        "this retained SF1 filesystem plan requires 4096-byte fragments",
    )
    expected_roles = {
        (f"taira-validator-{i}", role)
        for i in range(1, 5)
        for role in (
            "iroha3d",
            "iroha_cli",
            "sorafs_node",
            "config",
            "genesis",
            "genesis_hash",
        )
    }
    expected_roles |= {("taira-edge", "iroha_cli"), ("taira-edge", "edge_config")}
    rows = inputs["artifacts"]
    _require_derivation(
        len(rows) == 26 and {(r["slug"], r["role"]) for r in rows} == expected_roles,
        "exact four-validator and edge role inventory required",
    )
    binary_roles = {
        "iroha3d": "iroha3d_taira",
        "iroha_cli": "iroha",
        "sorafs_node": "sorafs-node",
    }
    role_sizes = []
    for row in rows:
        observed = _positive_derivation(row["bytes"], "artifact metadata length")
        if row["role"] in binary_roles:
            size = sizes[binary_roles[row["role"]]]
        elif row["role"] == "config":
            _require_derivation(
                observed <= MIB, "config exceeds native 1 MiB materialization limit"
            )
            size = MIB
        else:
            size = observed
        role_sizes.append({"slug": row["slug"], "role": row["role"], "bytes": size})
    artifact_logical = sum(row["bytes"] for row in role_sizes)
    a = allocation_bound(artifact_logical, 26, 64, fragment)

    stage_root = direct_path(runtime["source_stage"])
    stage_rows = inputs["stage_files"]
    stage_files = {
        str(Path(row["path"]).relative_to(stage_root)): _positive_derivation(
            row["bytes"], "stage file size"
        )
        for row in stage_rows
    }
    expected_stage = {
        "receipt.json",
        "container.json",
        "service.json",
        "manifests/aarch64.to",
        "manifests/discovery.to",
        "manifests/bundle.to",
        "payloads/bundle.bin",
        "payloads/guest/aarch64/initrd.img",
        "payloads/guest/aarch64/vmlinux",
        "payloads/guest/aarch64/rootfs.ext4",
        "payloads/discovery/index.json",
    }
    _require_derivation(
        len(stage_rows) == 11 and set(stage_files) == expected_stage,
        "retained aarch64 SF1 stage must have exactly the known eleven files",
    )
    stage_logical = sum(stage_files.values())
    _require_derivation(
        stage_logical == inputs["inventory_inrou_stage_bytes"],
        "stage receipt byte total differs",
    )
    s = allocation_bound(stage_logical, 11, 22, fragment)
    payloads = {
        name: size for name, size in stage_files.items() if name.startswith("payloads/")
    }
    chunk_count = sum((size + 65535) // 65536 for size in payloads.values())
    # Index, per-manifest metadata, manifest and their atomic publication copies.
    store_metadata = 2 * (3 * (64 + 16) + 64) * MIB
    p = allocation_bound(
        stage_logical + store_metadata + MIB, chunk_count + 32, 64, fragment
    )

    _require_derivation(
        runtime["service_artifacts"] == [],
        "additional service artifacts require an explicit capacity model",
    )
    bundle = _positive_derivation(
        runtime["bundle_compressed_bytes"], "compressed bundle size"
    )
    _require_derivation(
        bundle == stage_files["payloads/bundle.bin"],
        "runtime bundle does not match measured stage",
    )
    members = runtime["bundle_members"]
    _require_derivation(
        isinstance(members, list)
        and members
        and len(members) <= 1024
        and len({row["path"] for row in members}) == len(members),
        "bounded distinct bundle members required",
    )
    _require_derivation(
        all(row["kind"] in ("file", "directory") for row in members),
        "unsupported archive member kind",
    )
    unpacked = sum(count(row["bytes"], "archive member size") for row in members)
    decoded = _positive_derivation(
        runtime["bundle_archive_decoded_bytes"], "decoded archive size"
    )
    _require_derivation(
        unpacked <= decoded, "archive member bytes exceed decoded archive"
    )
    leases = runtime["lease_volumes"]
    _require_derivation(
        len(leases) == 2
        and {row["volume_name"] for row in leases} == {"root_disk", "app_data"},
        "retained root and data lease geometry required",
    )
    lease_bytes = sum(
        _positive_derivation(row["max_total_bytes"], "lease maximum") for row in leases
    )
    root_lease = next(row for row in leases if row["volume_name"] == "root_disk")
    _require_derivation(
        root_lease["max_total_bytes"]
        >= stage_files["payloads/guest/aarch64/rootfs.ext4"],
        "root lease must cover hydrated rootfs",
    )
    ephemeral = count(
        runtime["container_resources"]["ephemeral_storage_bytes"], "ephemeral maximum"
    )
    guest = sum(
        size for name, size in payloads.items() if name.startswith("payloads/guest/")
    )
    # Cache and temporary, block image and temporary, extracted tree and temporary.
    runtime_logical = (
        guest
        + lease_bytes
        + ephemeral
        + 2 * bundle
        + 2 * ((bundle + 511) // 512 * 512)
        + 2 * unpacked
        + MIB
    )
    runtime_files = 73 + 2 * len(members)
    r = allocation_bound(
        runtime_logical,
        runtime_files,
        64 + 2 * sum(row["kind"] == "directory" for row in members),
        fragment,
    )
    guest_plan = cohost_peak_plan(
        coordinator_path=coordinator_path,
        upload_path=upload_path,
        service_path=service_path,
        store_paths=store_paths,
        runtime_paths=runtime_paths,
        artifacts=a,
        stage=s,
        per_store=p,
        per_replica_runtime=r,
        headroom=[
            {
                "path": guest_headroom_path,
                "label": "guest filesystem headroom",
                "bytes": 2 * GIB,
                "inodes": 16384,
            }
        ],
    )
    required_bytes = sum(row["bytes"] for row in guest_plan["allocations"])
    required_inodes = sum(row["inodes"] for row in guest_plan["allocations"])
    backing_plan = {
        "schema": PLAN_SCHEMA,
        "allocations": [
            {
                "path": backing_path,
                "label": "full additional guest allocation including guest reserve",
                "bytes": required_bytes,
                "inodes": 1,
            },
            {
                "path": backing_path,
                "label": "Mac physical backing headroom",
                "bytes": 2 * GIB,
                "inodes": 1024,
            },
        ],
    }
    validate_plan(backing_plan)
    derivation = {
        "schema": "taira.disk-derivation.v1",
        "commit": expected_commit,
        "formula": "3A + 2S + 4P + 4R + 2 GiB guest headroom",
        "artifacts": a,
        "stage": s,
        "per_store": p,
        "per_replica_runtime": r,
        "artifact_role_sizes": role_sizes,
        "artifact_logical_bytes": artifact_logical,
        "stage_logical_bytes": stage_logical,
        "chunk_count_upper_bound": chunk_count,
        "chunk_min_bytes": 65536,
        "native_sf1_manifest_bindings": dict(bindings),
        "store_metadata_and_temporary_bytes": store_metadata,
        "runtime_guest_bytes": guest,
        "runtime_all_lease_maxima_bytes": lease_bytes,
        "runtime_ephemeral_bytes": ephemeral,
        "runtime_bundle_unpacked_bytes": unpacked,
        "config_per_role_bytes_bound": MIB,
        "required_bytes": required_bytes,
        "required_inodes": required_inodes,
        "backing_required_bytes": required_bytes + 2 * GIB,
        "requires_completed_transfer_before_admission": True,
        "existing_inputs_credited": False,
        "secret_contents_read": False,
        "free_space_observed": False,
        "reservation_created": False,
    }
    return {
        "guest_plan": guest_plan,
        "backing_plan": backing_plan,
        "derivation": derivation,
    }


def read_plan(path: Path) -> object:
    """Read only the bounded public JSON plan and reject duplicate keys."""

    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise CapacityError("duplicate plan field")
            result[key] = value
        return result

    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode):
            raise CapacityError("plan must be a regular file")
        if before.st_size > MAX_PLAN_BYTES:
            raise CapacityError("plan exceeds 1 MiB")
        with os.fdopen(os.dup(fd), "rb") as source:
            data = source.read(MAX_PLAN_BYTES + 1)
        after = os.fstat(fd)
        named = path.stat(follow_symlinks=False)

        def identity(info):
            return (
                info.st_dev,
                info.st_ino,
                info.st_mode,
                info.st_nlink,
                info.st_size,
                info.st_mtime_ns,
                info.st_ctime_ns,
            )

        if identity(before) != identity(after) or identity(after) != identity(named):
            raise CapacityError("plan changed during inspection")
        if len(data) != before.st_size:
            raise CapacityError("plan size changed during inspection")
        return json.loads(data, object_pairs_hook=unique)
    finally:
        os.close(fd)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan", required=True, type=Path, help="public metadata allocation plan"
    )
    args = parser.parse_args()
    try:
        result = evaluate(read_plan(args.plan))
    except (CapacityError, OSError, ValueError) as error:
        print(
            json.dumps({"schema": RESULT_SCHEMA, "passed": False, "error": str(error)})
        )
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0 if result["passed"] else 2


if __name__ == "__main__":
    sys.exit(main())
