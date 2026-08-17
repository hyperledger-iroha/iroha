"""Adversarial tests for the privileged Taira four-peer restart proof."""

from __future__ import annotations

import json
import os
import signal
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import capture_taira_macos_four_peer_receipt as capture
from scripts import deploy_taira_v21_reset as deploy
from scripts import taira_rollout_admission as admission
from scripts import taira_peer_supervisor as supervisor
from scripts.tests.taira_receipt_signer_test_support import receipt_signer_map


class FakeProcess:
    """Minimal stable supervisor process handle used by restart tests."""

    def __init__(self, pid: int = 111) -> None:
        self.pid = pid
        self.signals: list[int] = []

    def poll(self) -> None:
        return None

    def send_signal(self, value: int) -> None:
        self.signals.append(value)


class FakeOps:
    """Return one exact, stable child identity unless a test mutates it."""

    def __init__(
        self,
        process: deploy.ProcessInfo,
        children: tuple[int, ...] | None = None,
        repeated: deploy.ProcessInfo | None = None,
    ) -> None:
        self.process = process
        self.repeated = repeated or process
        self.children = children or (process.pid,)
        self.inspections = 0

    def inspect_process(self, _pid: int) -> deploy.ProcessInfo:
        self.inspections += 1
        return self.process if self.inspections == 1 else self.repeated

    def child_pids(self, _parent_pid: int) -> tuple[int, ...]:
        return self.children


def running_supervisor(tmp_path: Path) -> tuple[capture.RunningSupervisor, int, int]:
    """Create one owner-private PID file and its expected process binding."""

    uid, gid = os.geteuid(), os.getegid()
    pid_file = tmp_path / "validator.pid"
    pid_file.write_text("222\n", encoding="ascii")
    pid_file.chmod(0o600)
    process = FakeProcess()
    child_argv = ("/installed/iroha3d", "--sora", "--config", "/bundle/config.toml")
    item = capture.RunningSupervisor(
        peer=SimpleNamespace(label="taira-validator-1"),
        process=process,  # type: ignore[arg-type]
        pid_file=pid_file,
        terminal_file=tmp_path / "terminal.json",
        workdir=tmp_path,
        storage=tmp_path,
        child_argv=child_argv,
    )
    return item, uid, gid


def exact_child(item: capture.RunningSupervisor, uid: int) -> deploy.ProcessInfo:
    """Return the only process identity the capture authority may restart."""

    return deploy.ProcessInfo(
        pid=222,
        ppid=item.process.pid,
        uid=uid,
        argv=item.child_argv,
    )


def test_restart_targets_known_supervisor_not_pid_file_child(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    item, uid, gid = running_supervisor(tmp_path)
    ops = FakeOps(exact_child(item, uid))

    def forbidden_kill(_pid: int, _signal: int) -> None:
        raise AssertionError("capture authority must not signal a PID-file-selected child")

    monkeypatch.setattr(capture.os, "kill", forbidden_kill)
    assert capture._request_child_restart(item, uid, gid, ops) == 222
    assert item.process.signals == [signal.SIGUSR1]


@pytest.mark.parametrize("mutation", ("parent", "uid", "argv", "siblings"))
def test_restart_rejects_confused_deputy_child_identity(
    tmp_path: Path, mutation: str
) -> None:
    item, uid, gid = running_supervisor(tmp_path)
    process = exact_child(item, uid)
    children = (222,)
    if mutation == "parent":
        process = deploy.ProcessInfo(process.pid, item.process.pid + 1, uid, process.argv)
    elif mutation == "uid":
        process = deploy.ProcessInfo(process.pid, item.process.pid, uid + 1, process.argv)
    elif mutation == "argv":
        process = deploy.ProcessInfo(
            process.pid,
            item.process.pid,
            uid,
            ("/usr/bin/python3", "/attacker.py"),
        )
    else:
        children = (222, 333)

    with pytest.raises(capture.MacosFourPeerCaptureError):
        capture._request_child_restart(item, uid, gid, FakeOps(process, children))
    assert item.process.signals == []


def test_restart_rejects_pid_file_substitution_during_inspection(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    item, uid, gid = running_supervisor(tmp_path)
    values = iter((222, 333))
    monkeypatch.setattr(
        capture.deploy,
        "parse_pid_file",
        lambda _path, _uid, _gid: next(values),
    )

    with pytest.raises(capture.MacosFourPeerCaptureError, match="PID changed"):
        capture._request_child_restart(item, uid, gid, FakeOps(exact_child(item, uid)))
    assert item.process.signals == []


def test_restart_rejects_process_replacement_after_child_inventory_check(
    tmp_path: Path,
) -> None:
    item, uid, gid = running_supervisor(tmp_path)
    initial = exact_child(item, uid)
    replaced = deploy.ProcessInfo(
        initial.pid,
        initial.ppid,
        initial.uid,
        ("/installed/iroha3d", "--sora", "--config", "/attacker/config.toml"),
    )

    with pytest.raises(capture.MacosFourPeerCaptureError, match="changed"):
        capture._request_child_restart(
            item,
            uid,
            gid,
            FakeOps(initial, repeated=replaced),
        )
    assert item.process.signals == []


def test_supervisor_forwards_authority_restart_only_to_its_live_child() -> None:
    child = FakeProcess(pid=222)
    supervisor.forward_restart_to_child(child)  # type: ignore[arg-type]
    assert child.signals == [signal.SIGTERM]

    child.poll = lambda: 0  # type: ignore[method-assign]
    supervisor.forward_restart_to_child(child)  # type: ignore[arg-type]
    assert child.signals == [signal.SIGTERM]


def test_supervisor_run_installs_distinct_restart_signal_contract() -> None:
    import inspect

    source = inspect.getsource(supervisor.run)
    assert "signal.signal(signal.SIGUSR1, request_restart)" in source
    assert "restart_requested = False" in source
    assert "fatal_tracker.observe(None)" in source


def test_capture_accepts_only_root_controlled_supervisor_source(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "taira_peer_supervisor.py"
    source.write_bytes(b"reviewed supervisor")
    checked: list[tuple[Path, bool]] = []
    monkeypatch.setattr(
        capture.deploy,
        "require_root_controlled_file",
        lambda path, *, executable: checked.append((path, executable)),
    )
    digest = capture._root_controlled_supervisor(source.resolve())
    assert digest == capture.stable_hash_path(source).sha256
    assert checked == [(source.resolve(), False)]

    def reject(_path: Path, *, executable: bool) -> None:
        raise deploy.DeploymentError("caller-owned source")

    monkeypatch.setattr(capture.deploy, "require_root_controlled_file", reject)
    with pytest.raises(capture.MacosFourPeerCaptureError, match="root-controlled"):
        capture._root_controlled_supervisor(source.resolve())


def test_capture_receipt_binds_ordered_signers_to_exact_peer_rows() -> None:
    signers = deploy.require_receipt_signer_map(
        receipt_signer_map(), "capture fixture receipt signers"
    )
    peers = tuple(
        SimpleNamespace(
            config_sha256=f"{number}" * 64,
            label=f"io.soramitsu.taira.validator-{number}",
            number=number,
            slug=slug,
        )
        for number, slug in enumerate(deploy.SLUGS, start=1)
    )
    bundle = SimpleNamespace(
        manifest_sha256="8" * 64,
        peers=peers,
        receipt_signers=signers,
    )
    source = admission.SourceIdentity("a" * 40, "b" * 40, "c" * 64, "d" * 64)
    receipt = capture._receipt(
        source=source,
        bundle=bundle,
        binary_sha256="3" * 64,
        artifact_handoff_sha256="2" * 64,
        supervisor_sha256="7" * 64,
        restart_generation="6" * 64,
        start=SimpleNamespace(block_hash="4" * 64, height=101),
        end=SimpleNamespace(block_hash="5" * 64, height=102),
        issued_at=1_000,
    )

    assert list(receipt["receipt_signers"]) == list(deploy.SLUGS)
    assert [row["receipt_signer_node_id"] for row in receipt["peers"]] == [
        receipt["receipt_signers"][slug]["node_id"] for slug in deploy.SLUGS
    ]
    assert "812620" not in json.dumps(receipt)
    assert admission._validate_macos_receipt(
        admission.canonical_json_bytes(receipt),
        expected_source=source,
        expected_receipt_id=receipt["receipt_id"],
        consumed_receipt_ids=set(),
        now_unix=1_000,
    )["receipt_signers"] == receipt["receipt_signers"]
