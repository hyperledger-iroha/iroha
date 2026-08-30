#!/usr/bin/env python3
"""Render a sealed, unsigned public bundle for the Taira reset17 controller.

The preparation spec contains public source paths and private-file *paths* but
never private bytes.  This renderer copies only authenticated public inputs,
generates the exact four LaunchAgents, and writes canonical ``manifest.json``.
Signing is deliberately a separate operator action with ``ssh-keygen -Y sign``.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import stat
import sys
from typing import Any, Mapping, Sequence

import testnet_reset17_authenticated_reset as reset17


SPEC_SCHEMA = "inori.taira.reset17-preparation-spec.v1"
FIXED_INSTALLS = {
    "iroha3d_taira": Path("bin/iroha3d_taira"),
    "kagami": Path("bin/kagami"),
    "iroha": Path("bin/iroha"),
    "taira_operator_status": Path("bin/taira_operator_status"),
    "fd198_supervisor": Path("libexec/taira_fd198_supervisor.py"),
    "genesis_manifest": Path("config/genesis.manifest.json"),
    "signed_genesis": Path("config/genesis.signed.nrt"),
    "preparation_tool": Path("libexec/prepare_reset17_candidate.py"),
}
EXECUTABLE_ARTIFACTS = {
    "iroha3d_taira",
    "kagami",
    "iroha",
    "taira_operator_status",
}


def _exact_keys(value: Mapping[str, Any], keys: Sequence[str], label: str) -> None:
    if set(value) != set(keys):
        raise reset17.Reset17Error(f"{label} has missing or unexpected fields")


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise reset17.Reset17Error(f"{label} must be an object")
    return value


def _absolute(value: Any, label: str) -> Path:
    return reset17._absolute_path(value, label)


def _mkdir(path: Path, mode: int = 0o700) -> None:
    try:
        os.mkdir(path, mode)
    except FileExistsError:
        metadata = os.lstat(path)
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != mode
        ):
            raise reset17.Reset17Error("preparation directory is untrusted")


def _ensure_parents(root: Path, relative: Path) -> Path:
    current = root
    for component in relative.parts:
        current /= component
        _mkdir(current)
    return current


def _copy_public(source: Path, destination: Path, mode: int) -> tuple[str, int]:
    reset17._reject_symlink_components(source, "preparation public source")
    descriptor, payload = reset17._open_and_read_bounded(
        source, reset17.MAX_PUBLIC_FILE_BYTES, "preparation public source"
    )
    try:
        metadata = os.fstat(descriptor)
        if metadata.st_uid not in (0, os.geteuid()):
            raise reset17.Reset17Error("preparation public source owner is foreign")
    finally:
        os.close(descriptor)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    target = os.open(destination, flags, 0o600)
    complete = False
    try:
        view = memoryview(payload)
        offset = 0
        while offset < len(payload):
            count = os.write(target, view[offset:])
            if count <= 0:
                raise reset17.Reset17Error("short reset17 preparation write")
            offset += count
        view.release()
        os.fchmod(target, mode)
        os.fsync(target)
        complete = True
    finally:
        os.close(target)
        if not complete:
            try:
                os.unlink(destination)
            except FileNotFoundError:
                pass
    return hashlib.sha256(payload).hexdigest(), len(payload)


def _write_public(destination: Path, payload: bytes, mode: int) -> tuple[str, int]:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(destination, flags, 0o600)
    complete = False
    try:
        view = memoryview(payload)
        offset = 0
        while offset < len(payload):
            count = os.write(descriptor, view[offset:])
            if count <= 0:
                raise reset17.Reset17Error("short generated-public-file write")
            offset += count
        view.release()
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        complete = True
    finally:
        os.close(descriptor)
        if not complete:
            try:
                os.unlink(destination)
            except FileNotFoundError:
                pass
    return hashlib.sha256(payload).hexdigest(), len(payload)


def _record(path: Path, install: Path, digest: str, size: int, mode: int) -> dict[str, Any]:
    return {
        "path": str(path),
        "sha256": digest,
        "size": size,
        "mode": mode,
        "install_relative": str(install),
    }


def _launch_agent(
    *,
    label: str,
    python_path: Path,
    release_dir: Path,
    binary_install: Path,
    supervisor_install: Path,
    config_install: Path,
    genesis_install: Path,
    signer_source: Path,
    signer_launch: Path,
    control_root: Path,
) -> bytes:
    value = {
        "Label": label,
        "Program": str(python_path),
        "ProgramArguments": [
            str(python_path),
            "-I",
            "-B",
            str(release_dir / supervisor_install),
            "run",
            "--binary",
            str(release_dir / binary_install),
            "--config",
            str(release_dir / config_install),
            "--genesis-manifest",
            str(release_dir / genesis_install),
            "--signer-source",
            str(signer_source),
            "--signer-launch",
            str(signer_launch),
        ],
        "RunAtLoad": True,
        "KeepAlive": True,
        "WorkingDirectory": str(release_dir),
        "ThrottleInterval": 10,
        "SoftResourceLimits": {"NumberOfFiles": 4096},
        "HardResourceLimits": {"NumberOfFiles": 8192},
        "EnvironmentVariables": {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        "Umask": 0o077,
        "StandardOutPath": str(control_root / "logs" / f"{label}.stdout.log"),
        "StandardErrorPath": str(control_root / "logs" / f"{label}.stderr.log"),
    }
    return plistlib.dumps(value, fmt=plistlib.FMT_XML, sort_keys=True)


def render(spec_path: Path, output: Path) -> tuple[Path, str]:
    spec = _object(reset17.read_json(spec_path), "preparation spec")
    _exact_keys(
        spec,
        (
            "schema",
            "release_id",
            "network_id",
            "source",
            "protocols",
            "bpng",
            "deployment",
            "artifacts",
            "validators",
        ),
        "preparation spec",
    )
    if spec["schema"] != SPEC_SCHEMA:
        raise reset17.Reset17Error("preparation spec schema is foreign")
    release_id = reset17._require_string(spec["release_id"], "release id")
    if reset17.RUN_ID_RE.fullmatch(release_id) is None or not release_id.startswith(
        "reset17-"
    ):
        raise reset17.Reset17Error("preparation release id is not canonical")
    network_id = reset17._require_string(spec["network_id"], "NetworkId")
    if reset17.NETWORK_ID_RE.fullmatch(network_id) is None:
        raise reset17.Reset17Error("preparation NetworkId is malformed")
    source = _object(spec["source"], "source")
    reset17._parse_source(source, source.get("commit", ""))
    reset17._validate_protocols(spec["protocols"])
    reset17._validate_bpng(spec["bpng"])

    deployment_input = _object(spec["deployment"], "deployment")
    _exact_keys(
        deployment_input,
        (
            "uid",
            "launch_domain",
            "python_path",
            "release_dir",
            "launch_agents_dir",
            "control_root",
            "require_single_data_volume",
            "free_reserve_bytes",
            "free_reserve_bps",
        ),
        "deployment",
    )
    python_path = _absolute(deployment_input["python_path"], "isolated Python path")
    python_fd, python_payload = reset17._open_and_read_bounded(
        python_path, reset17.MAX_PUBLIC_FILE_BYTES, "isolated Python"
    )
    os.close(python_fd)
    deployment = {
        key: value for key, value in deployment_input.items() if key != "python_path"
    }
    deployment["python"] = {
        "path": str(python_path),
        "sha256": hashlib.sha256(python_payload).hexdigest(),
        "size": len(python_payload),
    }
    parsed_deployment = reset17._parse_deployment(deployment, release_id)

    if not output.is_absolute() or str(output) != os.path.normpath(str(output)):
        raise reset17.Reset17Error("bundle output must be normalized and absolute")
    if output.exists() or output.is_symlink():
        raise reset17.Reset17Error("bundle output already exists")
    reset17._validate_existing_directory(output.parent, "bundle output parent")
    _mkdir(output)
    payload_root = output / "payload"
    _mkdir(payload_root)

    artifacts_input = _object(spec["artifacts"], "artifact source inventory")
    if set(artifacts_input) != reset17.REQUIRED_ARTIFACTS:
        raise reset17.Reset17Error("artifact source inventory is not exact")
    artifact_records: dict[str, Any] = {}
    for name in sorted(reset17.REQUIRED_ARTIFACTS):
        source_path = _absolute(artifacts_input[name], f"{name} source")
        install = FIXED_INSTALLS[name]
        destination_parent = _ensure_parents(payload_root, install.parent)
        destination = destination_parent / install.name
        mode = 0o555 if name in EXECUTABLE_ARTIFACTS else 0o444
        digest, size = _copy_public(source_path, destination, mode)
        artifact_records[name] = _record(
            Path("payload") / install, install, digest, size, mode
        )

    validators_input = spec["validators"]
    if not isinstance(validators_input, list) or len(validators_input) != 4:
        raise reset17.Reset17Error("preparation spec must contain four validators")
    validators: list[dict[str, Any]] = []
    for expected_index, item in enumerate(validators_input, start=1):
        validator = _object(item, f"validator {expected_index}")
        _exact_keys(
            validator,
            (
                "index",
                "label",
                "data_root",
                "torii_url",
                "p2p_port",
                "config_source",
                "private_files",
                "runtime_signer",
            ),
            f"validator {expected_index}",
        )
        if validator["index"] != expected_index:
            raise reset17.Reset17Error("preparation validators are not ordered")
        label = f"org.sora.taira.user.validator-{expected_index}"
        if validator["label"] != label:
            raise reset17.Reset17Error("preparation validator label is foreign")
        config_install = Path("config") / f"validator-{expected_index}.toml"
        config_destination = (
            _ensure_parents(payload_root, config_install.parent) / config_install.name
        )
        config_digest, config_size = _copy_public(
            _absolute(validator["config_source"], "validator config source"),
            config_destination,
            0o444,
        )
        config_record = _record(
            Path("payload") / config_install,
            config_install,
            config_digest,
            config_size,
            0o444,
        )
        private_files = _object(validator["private_files"], "private-file inventory")
        runtime_signer = _object(validator["runtime_signer"], "runtime signer")
        signer_source = _absolute(
            private_files.get("soracloud_runtime_signer"), "runtime signer source"
        )
        signer_launch = _absolute(
            runtime_signer.get("launch_path"), "runtime signer launch"
        )
        launch_install = Path("launch-agents") / f"{label}.plist"
        launch_destination = (
            _ensure_parents(payload_root, launch_install.parent) / launch_install.name
        )
        launch_payload = _launch_agent(
            label=label,
            python_path=python_path,
            release_dir=parsed_deployment["release_dir"],
            binary_install=FIXED_INSTALLS["iroha3d_taira"],
            supervisor_install=FIXED_INSTALLS["fd198_supervisor"],
            config_install=config_install,
            genesis_install=FIXED_INSTALLS["genesis_manifest"],
            signer_source=signer_source,
            signer_launch=signer_launch,
            control_root=parsed_deployment["control_root"],
        )
        launch_digest, launch_size = _write_public(
            launch_destination, launch_payload, 0o444
        )
        validators.append(
            {
                "index": expected_index,
                "label": label,
                "data_root": validator["data_root"],
                "torii_url": validator["torii_url"],
                "p2p_port": validator["p2p_port"],
                "config": config_record,
                "launch_agent": _record(
                    Path("payload") / launch_install,
                    launch_install,
                    launch_digest,
                    launch_size,
                    0o444,
                ),
                "private_files": private_files,
                "runtime_signer": runtime_signer,
            }
        )

    manifest = {
        "schema": reset17.MANIFEST_SCHEMA,
        "generation": reset17.GENERATION,
        "release_id": release_id,
        "network_id": network_id,
        "source": source,
        "protocols": spec["protocols"],
        "bpng": spec["bpng"],
        "deployment": deployment,
        "artifacts": artifact_records,
        "validators": validators,
    }
    manifest_path = output / "manifest.json"
    manifest_payload = reset17.canonical_json_bytes(manifest)
    _write_public(manifest_path, manifest_payload, 0o444)
    for path in sorted(
        (item for item in output.rglob("*") if item.is_dir()),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        os.chmod(path, 0o555)
    os.chmod(output, 0o555)
    return manifest_path, hashlib.sha256(manifest_payload).hexdigest()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", required=True, type=Path)
    parser.add_argument("--out-bundle", required=True, type=Path)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        manifest_path, digest = render(args.spec, args.out_bundle)
    except reset17.Reset17Error as error:
        print(f"taira-reset17-prepare: {error}", file=sys.stderr)
        return 1
    print(f"{manifest_path}\t{digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
