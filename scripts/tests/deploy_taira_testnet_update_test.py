"""Focused tests for the state-preserving Taira testnet updater."""

from __future__ import annotations

import os
import plistlib
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import deploy_taira_testnet_update as updater


def _arguments(index: int) -> tuple[str, ...]:
    return (
        "/usr/bin/python3",
        "/Library/SORA/Taira/taira_peer_supervisor.py",
        "--binary",
        "/Library/SORA/Taira/binaries/old/iroha3d",
        "--binary-sha256",
        "a" * 64,
        "--binary-device",
        "1",
        "--binary-inode",
        "2",
        "--binary-size",
        "3",
        "--binary-mtime-ns",
        "4",
        "--binary-ctime-ns",
        "5",
        "--config",
        f"/srv/taira/validator-{index}/config.toml",
        "--config-sha256",
        "b" * 64,
        "--workdir",
        f"/srv/taira/validator-{index}",
        "--workdir-device",
        "10",
        "--workdir-inode",
        str(100 + index),
        "--storage-dir",
        f"/srv/taira/validator-{index}/storage",
        "--storage-device",
        "10",
        "--storage-inode",
        str(200 + index),
        "--pid-file",
        f"/run/taira/validator-{index}.pid",
        "--terminal-unhealthy-file",
        f"/run/taira/validator-{index}-terminal.json",
        "--restart-generation",
        "c" * 64,
        "--initial-backoff-seconds",
        "1.0",
    )


def _snapshot(index: int) -> updater.PeerSnapshot:
    label = updater.LABELS[index - 1]
    arguments = _arguments(index)
    payload: dict[str, object] = {
        "Label": label,
        "ProgramArguments": list(arguments),
        "WorkingDirectory": f"/srv/taira/validator-{index}",
        "EnvironmentVariables": {
            "GENESIS": "/srv/taira/genesis.signed.nrt",
            "KURA_STORE_DIR": f"/srv/taira/validator-{index}/storage/kura",
            "SNAPSHOT_STORE_DIR": f"/srv/taira/validator-{index}/storage/snapshot",
        },
        "UserName": "iroha",
        "GroupName": "iroha",
    }
    body = plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)
    return updater.PeerSnapshot(
        label=label,
        port=updater.TORII_PORTS[index - 1],
        plist_path=Path(f"/Library/LaunchDaemons/{label}.plist"),
        plist_body=body,
        plist_mode=0o644,
        plist_uid=0,
        plist_gid=0,
        payload=payload,
        arguments=arguments,
        runtime_uid=501,
        runtime_gid=501,
        config=updater.ConfigSeal(
            Path(f"/srv/taira/validator-{index}/config.toml"),
            "b" * 64,
            (1,),
        ),
        workdir=updater.DirectorySeal(Path(f"/srv/taira/validator-{index}"), (2,)),
        storage=updater.DirectorySeal(
            Path(f"/srv/taira/validator-{index}/storage"), (3,)
        ),
    )


def _binary_info() -> SimpleNamespace:
    return SimpleNamespace(
        st_dev=90,
        st_ino=91,
        st_size=92,
        st_mtime_ns=93,
        st_ctime_ns=94,
    )


def test_rewrite_changes_only_binary_identity_and_preserves_live_paths() -> None:
    snapshot = _snapshot(1)
    body = updater.rewrite_plist(
        snapshot,
        Path("/Library/SORA/Taira/binaries/new/iroha3d"),
        "d" * 64,
        _binary_info(),
        "e" * 40,
    )
    payload = plistlib.loads(body)
    arguments = tuple(payload["ProgramArguments"])

    assert {
        key: value for key, value in payload.items() if key != "ProgramArguments"
    } == {
        key: value
        for key, value in snapshot.payload.items()
        if key != "ProgramArguments"
    }
    for option in updater.PRESERVED_OPTIONS:
        assert updater.required_option(arguments, option, snapshot.label) == (
            updater.required_option(snapshot.arguments, option, snapshot.label)
        )
    assert updater.required_option(arguments, "--binary", snapshot.label) == (
        "/Library/SORA/Taira/binaries/new/iroha3d"
    )
    assert (
        updater.required_option(arguments, "--binary-sha256", snapshot.label)
        == "d" * 64
    )
    assert [
        updater.required_option(arguments, option, snapshot.label)
        for option in updater.BINARY_STAT_OPTIONS
    ] == ["90", "91", "92", "93", "94"]
    assert updater.required_option(
        arguments, "--restart-generation", snapshot.label
    ) not in {"c" * 64, "d" * 64}


def test_directory_identity_detects_inode_replacement() -> None:
    first = SimpleNamespace(st_dev=1, st_ino=2, st_mode=0o040755, st_uid=3, st_gid=4)
    replacement = SimpleNamespace(
        st_dev=1, st_ino=99, st_mode=0o040755, st_uid=3, st_gid=4
    )

    assert updater.directory_identity(first) != updater.directory_identity(replacement)


def test_darwin_process_arguments_are_parsed_exactly() -> None:
    executable = b"/usr/bin/python3"
    arguments = (
        executable,
        b"-I",
        b"-S",
        b"/Library/SORA/Taira/taira_peer_supervisor.py",
    )
    raw = (
        len(arguments).to_bytes(
            updater.ctypes.sizeof(updater.ctypes.c_int),
            byteorder=updater.sys.byteorder,
            signed=True,
        )
        + executable
        + b"\0\0"
        + b"\0".join(arguments)
        + b"\0"
    )

    assert updater.parse_darwin_procargs2(raw) == tuple(
        value.decode("ascii") for value in arguments
    )
    with pytest.raises(updater.TestnetUpdateError, match="differs from argv"):
        updater.parse_darwin_procargs2(raw.replace(executable, b"/usr/bin/python9", 1))


def test_interrupts_enter_the_normal_error_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    installed: dict[updater.signal.Signals, object] = {}
    monkeypatch.setattr(
        updater.signal,
        "signal",
        lambda caught_signal, handler: installed.__setitem__(caught_signal, handler),
    )

    updater.install_interrupt_handlers()

    assert set(installed) == {
        updater.signal.SIGHUP,
        updater.signal.SIGINT,
        updater.signal.SIGTERM,
    }
    handler = installed[updater.signal.SIGTERM]
    assert callable(handler)
    with pytest.raises(updater.TestnetUpdateError, match="SIGTERM"):
        handler(updater.signal.SIGTERM, None)


def test_atomic_plist_interruption_removes_temporary_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    snapshot = updater.dataclasses.replace(
        _snapshot(1),
        plist_path=tmp_path / "validator.plist",
        plist_uid=os.getuid(),
        plist_gid=os.getgid(),
    )
    snapshot.plist_path.write_bytes(snapshot.plist_body)

    def interrupt_write(_descriptor: int, _body: object) -> int:
        raise updater.TestnetUpdateError("interrupted write")

    monkeypatch.setattr(updater.os, "write", interrupt_write)
    with pytest.raises(updater.TestnetUpdateError, match="interrupted write"):
        updater.atomic_replace_plist(snapshot, b"replacement")

    assert list(tmp_path.glob(".*.tmp")) == []
    assert snapshot.plist_path.read_bytes() == snapshot.plist_body


def test_post_restart_verification_requires_exact_supervisor_and_child(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    snapshot = _snapshot(1)
    supervisor_pid = 100
    child_pid = 200

    class IdentityOps:
        def __init__(self) -> None:
            self.supervisor_argv = snapshot.arguments
            self.child_argv = (
                updater.required_option(snapshot.arguments, "--binary", snapshot.label),
                "--sora",
                "--config",
                str(snapshot.config.path),
            )

        def launchd_record(self, _label: str, _deadline: float) -> str:
            return f"pid = {supervisor_pid}\n"

        def inspect_process(self, pid: int, _deadline: float) -> updater.ProcessInfo:
            if pid == supervisor_pid:
                return updater.ProcessInfo(
                    pid, 1, snapshot.runtime_uid, self.supervisor_argv
                )
            return updater.ProcessInfo(
                pid, supervisor_pid, snapshot.runtime_uid, self.child_argv
            )

        def child_pids(self, _parent_pid: int, _deadline: float) -> tuple[int, ...]:
            return (child_pid,)

    ops = IdentityOps()
    monkeypatch.setattr(updater, "parse_pid_file", lambda path, uid, gid: child_pid)

    updater.verify_managed_peer(
        snapshot,
        snapshot.plist_body,
        ops,  # type: ignore[arg-type]
        updater.time.monotonic() + 10,
    )
    ops.supervisor_argv = ("/different/python", *snapshot.arguments[1:])
    with pytest.raises(updater.TestnetUpdateError, match="live supervisor differs"):
        updater.verify_managed_peer(
            snapshot,
            snapshot.plist_body,
            ops,  # type: ignore[arg-type]
            updater.time.monotonic() + 10,
        )
    ops.supervisor_argv = snapshot.arguments
    ops.child_argv = ("/tmp/wrong",)
    with pytest.raises(updater.TestnetUpdateError, match="live validator differs"):
        updater.verify_managed_peer(
            snapshot,
            snapshot.plist_body,
            ops,  # type: ignore[arg-type]
            updater.time.monotonic() + 10,
        )


class FakeOps:
    """Track launchd order and assert no two validators are down together."""

    def __init__(self) -> None:
        self.events: list[tuple[str, str]] = []
        self.down: set[str] = set()
        self.maximum_down = 0

    def loaded(self, label: str, _deadline: float) -> bool:
        return label not in self.down

    def bootout(
        self, label: str, _deadline: float, *, allow_absent: bool = False
    ) -> None:
        del allow_absent
        self.events.append(("bootout", label))
        self.down.add(label)
        self.maximum_down = max(self.maximum_down, len(self.down))

    def bootstrap(self, plist: Path, _deadline: float) -> None:
        label = plist.stem
        self.events.append(("bootstrap", label))
        self.down.discard(label)


def test_rollout_restarts_exactly_one_peer_at_a_time(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    snapshots = tuple(_snapshot(index) for index in range(1, 5))
    monkeypatch.setattr(
        updater,
        "read_regular",
        lambda path, maximum: (
            next(
                snapshot.plist_body
                for snapshot in snapshots
                if snapshot.plist_path == path
            ),
            SimpleNamespace(),
        ),
    )
    writes: list[tuple[str, bytes]] = []
    waits: list[tuple[str, str | None]] = []
    ops = FakeOps()
    updater.roll_peers(
        snapshots,
        {snapshot.label: f"new-{snapshot.label}".encode() for snapshot in snapshots},
        "f" * 40,
        updater.time.monotonic() + 60,
        10,
        ops=ops,  # type: ignore[arg-type]
        writer=lambda snapshot, body: writes.append((snapshot.label, body)),
        waiter=lambda snapshot, plist, commit, deadline, timeout, waiter_ops: (
            waits.append((snapshot.label, commit))
        ),
        verifier=lambda peers: None,
    )

    assert ops.maximum_down == 1
    assert [label for label, _body in writes] == list(updater.LABELS)
    assert waits == [(label, "f" * 40) for label in (*updater.LABELS, *updater.LABELS)]
    assert ops.events == [
        event
        for label in updater.LABELS
        for event in (("bootout", label), ("bootstrap", label))
    ]


def test_peer_failure_rolls_touched_peers_back_in_reverse_and_stops(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    snapshots = tuple(_snapshot(index) for index in range(1, 5))
    monkeypatch.setattr(
        updater,
        "read_regular",
        lambda path, maximum: (
            next(
                snapshot.plist_body
                for snapshot in snapshots
                if snapshot.plist_path == path
            ),
            SimpleNamespace(),
        ),
    )
    new_plists = {
        snapshot.label: f"new-{snapshot.label}".encode() for snapshot in snapshots
    }
    writes: list[tuple[str, bytes]] = []
    ops = FakeOps()

    def wait(
        snapshot: updater.PeerSnapshot,
        _plist: bytes,
        commit: str | None,
        *_args: object,
    ) -> None:
        if snapshot.label == updater.LABELS[2] and commit is not None:
            raise updater.TestnetUpdateError("injected readiness failure")

    with pytest.raises(updater.TestnetUpdateError, match="injected readiness"):
        updater.roll_peers(
            snapshots,
            new_plists,
            "f" * 40,
            updater.time.monotonic() + 60,
            10,
            ops=ops,  # type: ignore[arg-type]
            writer=lambda snapshot, body: writes.append((snapshot.label, body)),
            waiter=wait,
            verifier=lambda peers: None,
        )

    assert writes == [
        (updater.LABELS[0], new_plists[updater.LABELS[0]]),
        (updater.LABELS[1], new_plists[updater.LABELS[1]]),
        (updater.LABELS[2], new_plists[updater.LABELS[2]]),
        (updater.LABELS[2], snapshots[2].plist_body),
        (updater.LABELS[1], snapshots[1].plist_body),
        (updater.LABELS[0], snapshots[0].plist_body),
    ]
    assert updater.LABELS[3] not in [label for label, _body in writes]
    assert ops.maximum_down == 1


def test_rollback_restores_plist_even_when_bootout_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    snapshot = _snapshot(1)
    monkeypatch.setattr(
        updater,
        "read_regular",
        lambda path, maximum: (snapshot.plist_body, SimpleNamespace()),
    )
    new_plist = b"new-plist"
    writes: list[bytes] = []

    class FailingRollbackBootout(FakeOps):
        def bootout(
            self, label: str, deadline: float, *, allow_absent: bool = False
        ) -> None:
            if allow_absent:
                raise updater.TestnetUpdateError("rollback bootout failed")
            super().bootout(label, deadline)

    def wait(
        _snapshot: updater.PeerSnapshot,
        _plist: bytes,
        commit: str | None,
        *_args: object,
    ) -> None:
        if commit is not None:
            raise updater.TestnetUpdateError("update failed")

    with pytest.raises(updater.TestnetUpdateError, match="rollback was incomplete"):
        updater.roll_peers(
            (snapshot,),
            {snapshot.label: new_plist},
            "f" * 40,
            updater.time.monotonic() + 60,
            10,
            ops=FailingRollbackBootout(),
            writer=lambda _snapshot, body: writes.append(body),
            waiter=wait,
            verifier=lambda peers: None,
        )

    assert writes == [new_plist, snapshot.plist_body]


def test_cli_is_small_bounded_and_requires_explicit_apply() -> None:
    args = updater.parse_args(
        [
            "--binary",
            "/tmp/iroha3d",
            "--expected-sha256",
            "a" * 64,
            "--expected-source-commit",
            "b" * 40,
        ]
    )
    assert args.apply is False
    assert args.deadline_seconds == 600
    assert args.health_timeout_seconds == 45
    assert updater.rollback_reserve_seconds(30) == 210
    assert updater.rollback_reserve_seconds(45) == 270
    with pytest.raises(SystemExit):
        updater.parse_args(
            [
                "--binary",
                "/tmp/iroha3d",
                "--expected-sha256",
                "a" * 64,
                "--expected-source-commit",
                "b" * 40,
                "--deadline-seconds",
                "901",
            ]
        )


def test_updater_has_no_reset_or_storage_mutation_surface() -> None:
    source = Path(updater.__file__).read_text(encoding="utf-8")
    for forbidden in (
        "shutil.rmtree",
        "os.truncate",
        "reset-manifest",
        "genesis.signed.nrt",
        "empty storage",
        "deploy-reset",
    ):
        assert forbidden not in source
    assert str(updater.INSTALLED_COMMAND) in source
    assert "CI must invoke that\ninstalled copy" in source
