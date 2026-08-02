#!/usr/bin/env python3
"""Run the exact macOS/arm64 Taira cohort and emit its canonical receipt.

This is a release-capture command for a dedicated disposable macOS runner. It
requires root to install the candidate binary and supervisor in root-controlled
content-addressed validation paths. It starts
four ordinary per-peer supervisors as the reset-bundle owner, proves common
consensus advancement, restarts every validator child in turn, and then
leaves the fresh deploy bundle unchanged before writing the receipt.

The original deploy bundle's storage is never used.  Each peer runs in an
owner-private shadow work directory with empty throw-away storage, while the
exact receipt-bound config bytes and genesis remain the process inputs.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import pwd
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn, Sequence

try:
    from . import deploy_taira_v21_reset as deploy
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_output_fd,
        stable_hash_path,
        stable_open_relative,
    )
except ImportError:
    import deploy_taira_v21_reset as deploy
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_output_fd,
        stable_hash_path,
        stable_open_relative,
    )


VALIDATION_BINARY_ROOT = deploy.INSTALL_ROOT / "release-validation" / "bin"
VALIDATION_SUPERVISOR_ROOT = deploy.INSTALL_ROOT / "release-validation" / "supervisor"
MAX_RECEIPT_LIFETIME_SECONDS = 45 * 60
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")


class MacosFourPeerCaptureError(RuntimeError):
    """The native cohort did not satisfy the first-release capture contract."""


@dataclass(frozen=True)
class RunningSupervisor:
    peer: deploy.PeerPlan
    process: subprocess.Popen[bytes]
    pid_file: Path
    terminal_file: Path
    workdir: Path
    storage: Path
    child_argv: tuple[str, ...]


def _fail(message: str) -> NoReturn:
    raise MacosFourPeerCaptureError(message)


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _commit(value: object) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail("source commit must be one full lowercase 40-hex commit")
    return value


def _canonical(path: Path, label: str, *, directory: bool = False) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as exc:
        raise MacosFourPeerCaptureError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(info.st_mode):
        _fail(f"{label} must be canonical and contain no symlink alias")
    expected = stat.S_ISDIR if directory else stat.S_ISREG
    if not expected(info.st_mode):
        _fail(f"{label} has the wrong file type")
    return path


def _root_controlled_supervisor(path: Path) -> str:
    path = _canonical(path, "supervisor source")
    try:
        deploy.require_root_controlled_file(path, executable=False)
    except deploy.DeploymentError as exc:
        raise MacosFourPeerCaptureError(
            "supervisor source must be one immutable root-controlled file"
        ) from exc
    return stable_hash_path(path, max_size=4 * 1024 * 1024).sha256


def _install_validation_file(
    source: Path,
    digest: str,
    *,
    install_root: Path,
    installed_name: str,
    maximum_bytes: int,
    label: str,
) -> tuple[Path, tuple[int, ...]]:
    deploy.ensure_root_directory(deploy.INSTALL_ROOT, 0o755)
    deploy.ensure_root_directory(deploy.INSTALL_ROOT / "release-validation", 0o755)
    deploy.ensure_root_directory(install_root, 0o755)
    destination_dir = install_root / digest
    if destination_dir.exists() or destination_dir.is_symlink():
        _fail(f"validation {label} destination already exists: {destination_dir}")
    destination_dir.mkdir(mode=0o755)
    os.chown(destination_dir, 0, 0)
    destination = destination_dir / installed_name
    source_info = stable_hash_path(source, max_size=maximum_bytes)
    if source_info.sha256 != digest:
        _fail(f"validation {label} changed before protected installation")
    try:
        with stable_open_relative(
            source.parent, source.name, expected=source_info
        ) as source_descriptor:
            descriptor = os.open(
                destination,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
                0o555,
            )
            try:
                while chunk := os.read(source_descriptor, 1024 * 1024):
                    view = memoryview(chunk)
                    while view:
                        written = os.write(descriptor, view)
                        if written <= 0:
                            _fail(f"short write installing validation {label}")
                        view = view[written:]
                os.fchmod(descriptor, 0o555)
                os.fchown(descriptor, 0, 0)
                os.fsync(descriptor)
                installed = os.fstat(descriptor)
            finally:
                os.close(descriptor)
        if stable_hash_path(destination).sha256 != digest:
            _fail(f"protected validation {label} differs after installation")
        deploy.require_root_controlled_file(destination, executable=True)
        deploy.fsync_directory(destination_dir)
    except BaseException:
        destination.unlink(missing_ok=True)
        destination_dir.rmdir()
        raise
    return destination, deploy.metadata_identity(installed)


def _remove_validation_file(
    path: Path,
    expected_identity: tuple[int, ...],
    install_root: Path,
    label: str,
) -> None:
    try:
        current = path.lstat()
    except FileNotFoundError as exc:
        raise MacosFourPeerCaptureError(
            f"validation {label} disappeared before cleanup"
        ) from exc
    if deploy.metadata_identity(current) != expected_identity:
        _fail(f"validation {label} changed before cleanup")
    path.unlink()
    path.parent.rmdir()
    deploy.fsync_directory(install_root)


def _copy_shadow_workdir(source: Path, destination: Path, uid: int, gid: int) -> None:
    destination.mkdir(mode=0o700)
    os.chown(destination, uid, gid)
    for entry in sorted(source.iterdir(), key=lambda value: value.name):
        if entry.name == "storage":
            continue
        info = entry.lstat()
        if stat.S_ISLNK(info.st_mode):
            _fail(f"validator workdir contains a symlink: {entry}")
        target = destination / entry.name
        if stat.S_ISDIR(info.st_mode):
            shutil.copytree(entry, target, symlinks=False)
            for current, directory_names, file_names in os.walk(target):
                os.chown(current, uid, gid)
                os.chmod(current, 0o700)
                for name in directory_names:
                    child = Path(current) / name
                    os.chown(child, uid, gid)
                    os.chmod(child, 0o700)
                for name in file_names:
                    child = Path(current) / name
                    child_info = child.lstat()
                    if stat.S_ISLNK(child_info.st_mode):
                        _fail(f"shadow workdir contains a symlink: {child}")
                    os.chown(child, uid, gid)
                    os.chmod(child, 0o600)
        elif stat.S_ISREG(info.st_mode) and info.st_nlink == 1:
            shutil.copyfile(entry, target)
            os.chown(target, uid, gid)
            os.chmod(target, 0o600)
        else:
            _fail(f"validator workdir contains an unsafe entry: {entry}")
    storage = destination / "storage"
    storage.mkdir(mode=0o700)
    os.chown(storage, uid, gid)
    for name in ("kura", "snapshot"):
        child = storage / name
        child.mkdir(mode=0o700)
        os.chown(child, uid, gid)


def _drop_privileges(uid: int, gid: int, username: str):
    def apply() -> None:
        os.initgroups(username, gid)
        os.setgid(gid)
        os.setuid(uid)
        os.umask(0o077)

    return apply


def _start_supervisor(
    *,
    peer: deploy.PeerPlan,
    bundle: deploy.BundlePlan,
    binary: Path,
    binary_digest: str,
    supervisor: Path,
    runtime_root: Path,
    restart_generation: str,
) -> RunningSupervisor:
    shadow = runtime_root / "peers" / peer.slug
    _copy_shadow_workdir(peer.workdir, shadow, bundle.owner_uid, bundle.owner_gid)
    storage = shadow / "storage"
    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    workdir_info = shadow.lstat()
    storage_info = storage.lstat()
    binary_info = binary.lstat()
    terminal_binding = deploy.supervisor_terminal_binding(
        binary_digest, binary_info, peer.config_sha256, restart_generation
    )
    terminal_file = (
        runtime_root
        / "terminal"
        / f"validator-{peer.number}-{terminal_binding}-terminal-unhealthy.json"
    )
    argv = [
        str(deploy.DEFAULT_SUPERVISOR_PYTHON),
        "-I",
        "-S",
        str(supervisor),
        "--binary",
        str(binary),
        "--binary-sha256",
        binary_digest,
        "--binary-device",
        str(binary_info.st_dev),
        "--binary-inode",
        str(binary_info.st_ino),
        "--binary-size",
        str(binary_info.st_size),
        "--binary-mtime-ns",
        str(binary_info.st_mtime_ns),
        "--binary-ctime-ns",
        str(binary_info.st_ctime_ns),
        "--config",
        str(peer.config),
        "--config-sha256",
        peer.config_sha256,
        "--workdir",
        str(shadow),
        "--workdir-device",
        str(workdir_info.st_dev),
        "--workdir-inode",
        str(workdir_info.st_ino),
        "--storage-dir",
        str(storage),
        "--storage-device",
        str(storage_info.st_dev),
        "--storage-inode",
        str(storage_info.st_ino),
        "--pid-file",
        str(pid_file),
        "--terminal-unhealthy-file",
        str(terminal_file),
        "--restart-generation",
        restart_generation,
        "--initial-backoff-seconds",
        "1.0",
        "--maximum-backoff-seconds",
        "30.0",
        "--stable-uptime-seconds",
        "120.0",
    ]
    account = pwd.getpwuid(bundle.owner_uid)
    environment = {
        "GENESIS": str(bundle.root / "genesis.signed.nrt"),
        "KURA_STORE_DIR": str(storage / "kura"),
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "RUST_LOG": "info",
        "SNAPSHOT_STORE_DIR": str(storage / "snapshot"),
        "ZK_HALO2_ENABLED": "true",
    }
    process = subprocess.Popen(
        argv,
        cwd=shadow,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=environment,
        preexec_fn=_drop_privileges(
            bundle.owner_uid, bundle.owner_gid, account.pw_name
        ),
    )
    return RunningSupervisor(
        peer,
        process,
        pid_file,
        terminal_file,
        shadow,
        storage,
        (str(binary), "--sora", "--config", str(peer.config)),
    )


def _terminal_check(running: Sequence[RunningSupervisor]) -> None:
    for item in running:
        if item.process.poll() is not None:
            _fail(f"{item.peer.label} supervisor exited during release capture")
        if item.terminal_file.exists() or item.terminal_file.is_symlink():
            _fail(f"{item.peer.label} entered terminal-unhealthy state")


def _read_child(
    item: RunningSupervisor,
    uid: int,
    gid: int,
    ops: deploy.SystemOps | None = None,
) -> int:
    """Return the one exact validator child owned by this known supervisor."""

    if item.process.poll() is not None:
        _fail(f"{item.peer.label} supervisor exited before child inspection")
    inspector = ops or deploy.SystemOps()
    pid = deploy.parse_pid_file(item.pid_file, uid, gid)
    process = inspector.inspect_process(pid)
    if (
        process.ppid != item.process.pid
        or process.uid != uid
        or process.argv != item.child_argv
    ):
        _fail(f"{item.peer.label} validator child identity differs")
    if inspector.child_pids(item.process.pid) != (pid,):
        _fail(f"{item.peer.label} supervisor does not own exactly one child")
    if deploy.parse_pid_file(item.pid_file, uid, gid) != pid:
        _fail(f"{item.peer.label} validator PID changed during inspection")
    repeated = inspector.inspect_process(pid)
    if repeated != process:
        _fail(f"{item.peer.label} validator child changed during inspection")
    return pid


def _wait_new_child(
    item: RunningSupervisor,
    old_pid: int,
    uid: int,
    gid: int,
    deadline: float,
    ops: deploy.SystemOps | None = None,
) -> int:
    inspector = ops or deploy.SystemOps()
    while time.monotonic() < deadline:
        _terminal_check([item])
        try:
            candidate = _read_child(item, uid, gid, inspector)
        except (FileNotFoundError, OSError, deploy.DeploymentError):
            time.sleep(0.25)
            continue
        if candidate != old_pid:
            try:
                os.kill(old_pid, 0)
            except ProcessLookupError:
                return candidate
        time.sleep(0.25)
    _fail(f"{item.peer.label} did not replace its terminated validator child")


def _request_child_restart(
    item: RunningSupervisor,
    uid: int,
    gid: int,
    ops: deploy.SystemOps | None = None,
) -> int:
    """Ask the known supervisor to restart its exact inspected child."""

    old_child = _read_child(item, uid, gid, ops)
    try:
        item.process.send_signal(signal.SIGUSR1)
    except ProcessLookupError as exc:
        raise MacosFourPeerCaptureError(
            f"{item.peer.label} supervisor exited before restart request"
        ) from exc
    return old_child


def _stop_supervisors(running: Sequence[RunningSupervisor]) -> None:
    for item in running:
        if item.process.poll() is None:
            item.process.send_signal(signal.SIGTERM)
    deadline = time.monotonic() + 60
    pending = list(running)
    while pending and time.monotonic() < deadline:
        pending = [item for item in pending if item.process.poll() is None]
        if pending:
            time.sleep(0.25)
    if pending:
        _fail(
            "release-capture supervisors did not stop cleanly: "
            + ", ".join(item.peer.label for item in pending)
        )


def _prepare_runtime_root(path: Path, uid: int, gid: int) -> Path:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        _fail("runtime root must be one fresh absolute path")
    parent = path.parent.resolve(strict=True)
    if parent != path.parent:
        _fail("runtime-root parent must be canonical")
    path.mkdir(mode=0o700)
    os.chown(path, uid, gid)
    for name in ("peers", "pids", "terminal"):
        child = path / name
        child.mkdir(mode=0o700)
        os.chown(child, uid, gid)
    return path


def _receipt(
    *,
    source: admission.SourceIdentity,
    bundle: deploy.BundlePlan,
    binary_sha256: str,
    artifact_handoff_sha256: str,
    supervisor_sha256: str,
    restart_generation: str,
    start: deploy.FleetSample,
    end: deploy.FleetSample,
    issued_at: int,
) -> dict[str, object]:
    config_digests = {peer.slug: peer.config_sha256 for peer in bundle.peers}
    body: dict[str, object] = {
        "artifact_handoff_sha256": artifact_handoff_sha256,
        "end": {"block_hash": end.block_hash, "height": end.height},
        "expires_at_unix": issued_at + MAX_RECEIPT_LIFETIME_SECONDS,
        "issued_at_unix": issued_at,
        "peer_count": admission.PEER_COUNT,
        "peers": [
            {
                "final_block_hash": end.block_hash,
                "final_height": end.height,
                "label": peer.label,
                "number": peer.number,
                "restart_proof": "passed",
                "source_commit": source.commit,
                "validator_binary_sha256": binary_sha256,
                "validator_config_sha256": peer.config_sha256,
            }
            for peer in bundle.peers
        ],
        "platform": {"arch": "arm64", "os": "macos"},
        "reset_manifest_sha256": bundle.manifest_sha256,
        "restart_generation": restart_generation,
        "schema": admission.MACOS_RECEIPT_SCHEMA,
        "schema_version": admission.MACOS_RECEIPT_SCHEMA_VERSION,
        "source": source.as_dict(),
        "start": {"block_hash": start.block_hash, "height": start.height},
        "supervisor_sha256": supervisor_sha256,
        "validator_binary_sha256": binary_sha256,
        "validator_config_sha256": config_digests,
    }
    return {**body, "receipt_id": admission.compute_macos_receipt_id(body)}


def capture(args: argparse.Namespace) -> dict[str, object]:
    if os.geteuid() != 0:
        _fail("native macOS four-peer release capture requires root")
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        _fail("native release capture requires macOS arm64")
    source = admission.SourceIdentity(
        _commit(args.source_commit),
        _commit(args.dpn_validator_release_commit),
        _sha256(args.cargo_lock_sha256, "Cargo.lock digest"),
        _sha256(args.workspace_source_manifest_sha256, "workspace source digest"),
    )
    restart_generation = _sha256(args.restart_generation, "restart generation")
    bundle_path = _canonical(args.reset_bundle, "reset bundle", directory=True)
    binary_source = _canonical(args.validator_binary, "validator binary")
    output = args.output
    if not output.is_absolute() or output.exists() or output.is_symlink():
        _fail("receipt output must be one new absolute path")
    if args.health_timeout_seconds <= 0:
        _fail("health timeout must be positive")
    binary_sha = stable_hash_path(
        binary_source, max_size=deploy.MAX_BINARY_BYTES
    ).sha256
    manifest_sha = stable_hash_path(
        bundle_path / "reset-manifest.json", max_size=deploy.MAX_MANIFEST_BYTES
    ).sha256
    bundle = deploy.validate_bundle(
        bundle_path,
        expected_reset_manifest_sha256=manifest_sha,
        expected_binary_sha256=binary_sha,
        expected_source_commit=source.commit,
        expected_dpn_validator_release_commit=(
            source.dpn_validator_release_commit
        ),
        minimum_free_bytes=0,
        maximum_fsync_latency_ms=10_000,
    )
    if len(bundle.peers) != admission.PEER_COUNT:
        _fail("reset bundle must contain exactly four validators")
    supervisor = _canonical(args.supervisor, "supervisor source")
    supervisor_sha = _root_controlled_supervisor(supervisor)
    deploy.validate_supervisor_python(deploy.DEFAULT_SUPERVISOR_PYTHON)

    installed_binary: Path | None = None
    installed_identity: tuple[int, ...] | None = None
    installed_supervisor: Path | None = None
    installed_supervisor_identity: tuple[int, ...] | None = None
    running: list[RunningSupervisor] = []
    runtime_root: Path | None = None
    captured_error: BaseException | None = None
    start: deploy.FleetSample | None = None
    end: deploy.FleetSample | None = None
    try:
        installed_binary, installed_identity = _install_validation_file(
            binary_source,
            binary_sha,
            install_root=VALIDATION_BINARY_ROOT,
            installed_name="irohad",
            maximum_bytes=deploy.MAX_BINARY_BYTES,
            label="binary",
        )
        installed_supervisor, installed_supervisor_identity = _install_validation_file(
            supervisor,
            supervisor_sha,
            install_root=VALIDATION_SUPERVISOR_ROOT,
            installed_name="taira_peer_supervisor.py",
            maximum_bytes=4 * 1024 * 1024,
            label="supervisor",
        )
        deploy.validate_installed_peer_configs(installed_binary, bundle)

        runtime_root = _prepare_runtime_root(
            args.runtime_root, bundle.owner_uid, bundle.owner_gid
        )
        for peer in bundle.peers:
            running.append(
                _start_supervisor(
                    peer=peer,
                    bundle=bundle,
                    binary=installed_binary,
                    binary_digest=binary_sha,
                    supervisor=installed_supervisor,
                    runtime_root=runtime_root,
                    restart_generation=restart_generation,
                )
            )
        deadline = time.monotonic() + args.health_timeout_seconds
        start = deploy.wait_for_fleet_sample(
            bundle,
            source.commit,
            source.dpn_validator_release_commit,
            deadline,
            terminal_checker=lambda: _terminal_check(running),
        )
        prior = start
        for item in running:
            _terminal_check(running)
            old_child = _request_child_restart(
                item,
                bundle.owner_uid,
                bundle.owner_gid,
            )
            restart_deadline = time.monotonic() + deploy.RESTART_PROOF_TIMEOUT_SECONDS
            _wait_new_child(
                item,
                old_child,
                bundle.owner_uid,
                bundle.owner_gid,
                restart_deadline,
            )
            prior = deploy.wait_for_advancement(
                bundle,
                source.commit,
                source.dpn_validator_release_commit,
                prior,
                restart_deadline,
                terminal_checker=lambda: _terminal_check(running),
            )
        end = prior
        if end.height <= start.height or end.block_hash == start.block_hash:
            _fail("four-peer cohort did not advance after all restart proofs")
    except BaseException as exc:
        captured_error = exc
    finally:
        cleanup_errors: list[str] = []
        supervisors_stopped = True
        if running:
            try:
                _stop_supervisors(running)
            except BaseException:
                cleanup_errors.append("supervisors")
                supervisors_stopped = False
        if runtime_root is not None and supervisors_stopped:
            try:
                shutil.rmtree(runtime_root)
            except BaseException:
                cleanup_errors.append("runtime-root")
        if (
            installed_supervisor is not None
            and installed_supervisor_identity is not None
            and supervisors_stopped
        ):
            try:
                _remove_validation_file(
                    installed_supervisor,
                    installed_supervisor_identity,
                    VALIDATION_SUPERVISOR_ROOT,
                    "supervisor",
                )
            except BaseException:
                cleanup_errors.append("validation-supervisor")
        if (
            installed_binary is not None
            and installed_identity is not None
            and supervisors_stopped
        ):
            try:
                _remove_validation_file(
                    installed_binary,
                    installed_identity,
                    VALIDATION_BINARY_ROOT,
                    "binary",
                )
            except BaseException:
                cleanup_errors.append("validation-binary")
        if supervisors_stopped:
            try:
                deploy.require_bundle_runtime_unchanged(bundle)
            except BaseException:
                cleanup_errors.append("deploy-bundle")
        if cleanup_errors:
            failure = MacosFourPeerCaptureError(
                "native release capture cleanup was incomplete: "
                + ", ".join(cleanup_errors)
            )
            if captured_error is not None and hasattr(failure, "add_note"):
                failure.add_note(
                    f"capture failure: {type(captured_error).__name__}: {captured_error}"
                )
            raise failure from captured_error
    if captured_error is not None:
        raise captured_error
    assert start is not None and end is not None
    if stable_hash_path(binary_source).sha256 != binary_sha:
        _fail("validator binary changed across native release capture")
    if stable_hash_path(supervisor).sha256 != supervisor_sha:
        _fail("supervisor changed across native release capture")
    receipt = _receipt(
        source=source,
        bundle=bundle,
        binary_sha256=binary_sha,
        artifact_handoff_sha256=_sha256(
            args.artifact_handoff_sha256, "macOS build handoff digest"
        ),
        supervisor_sha256=supervisor_sha,
        restart_generation=restart_generation,
        start=start,
        end=end,
        issued_at=int(time.time()),
    )
    payload = canonical_json_bytes(receipt)
    with exclusive_output_fd(output, mode=0o600) as descriptor:
        os.fchown(descriptor, bundle.owner_uid, bundle.owner_gid)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("short write publishing macOS four-peer receipt")
            view = view[written:]
        os.fsync(descriptor)
    return {
        "artifact_handoff_sha256": receipt["artifact_handoff_sha256"],
        "end_height": end.height,
        "output": str(output),
        "peer_count": admission.PEER_COUNT,
        "receipt_id": receipt["receipt_id"],
        "reset_manifest_sha256": bundle.manifest_sha256,
        "source": source.as_dict(),
        "validator_binary_sha256": binary_sha,
    }


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--reset-bundle", type=Path, required=True)
    parser.add_argument("--validator-binary", type=Path, required=True)
    parser.add_argument("--artifact-handoff-sha256", required=True)
    parser.add_argument("--supervisor", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--restart-generation", required=True)
    parser.add_argument("--runtime-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--health-timeout-seconds", type=int, default=900)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        result = capture(args)
    except (
        MacosFourPeerCaptureError,
        OSError,
        ReleaseArtifactError,
        deploy.DeploymentError,
    ) as exc:
        print(f"macOS four-peer release capture refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
