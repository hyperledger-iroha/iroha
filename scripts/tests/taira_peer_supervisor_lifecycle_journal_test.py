"""Fail-closed tests for the local Taira peer lifecycle journal."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

import pytest

from scripts import check_taira_public_v2_24h_soak_evidence as public_verifier
from scripts import taira_peer_supervisor as supervisor


VALIDATOR_ID = "taira-validator-1"
NODE_ID = "taira-node:receipt-signer:secp256k1:sha256:" + "1" * 64
RESTART_GENERATION = "c" * 64


def identity_args() -> argparse.Namespace:
    """Return the exact deployment fields bound into a local journal."""

    return argparse.Namespace(
        binary_sha256="a" * 64,
        binary_device=None,
        binary_inode=None,
        binary_size=None,
        binary_mtime_ns=None,
        binary_ctime_ns=None,
        config_sha256="b" * 64,
        restart_generation=RESTART_GENERATION,
    )


def binding() -> str:
    """Return the fixture's domain-separated peer binding."""

    return supervisor.lifecycle_binding_sha256(
        identity_args(), VALIDATOR_ID, NODE_ID
    )


def private_directory(path: Path) -> Path:
    """Create one explicitly owner-private fixture directory."""

    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def open_journal(tmp_path: Path) -> supervisor.LifecycleJournal:
    """Open one new fixture journal below a private preprovisioned parent."""

    parent = private_directory(tmp_path / "runtime")
    return supervisor.LifecycleJournal(
        parent / "lifecycle",
        binding(),
        VALIDATOR_ID,
        NODE_ID,
        RESTART_GENERATION,
    )


def test_records_exact_verifier_rows_and_exports_only_raw_peer_input(
    tmp_path: Path,
) -> None:
    """Rows match the verifier, while the file cannot pose as global evidence."""

    journal = open_journal(tmp_path)
    try:
        first = journal.record("healthy", observed_at_unix_ms=1_000)
        baseline = journal.checkpoint(captured_at_unix_ms=1_001)
        journal.record("healthy", observed_at_unix_ms=1_002)
        journal.record("healthy", observed_at_unix_ms=1_003)
        terminal = journal.checkpoint(captured_at_unix_ms=1_004)
        export_dir = private_directory(tmp_path / "export")
        result = journal.export_window(
            baseline, terminal, export_dir / "peer-window.jsonl"
        )
    finally:
        journal.close()

    assert set(first) == public_verifier.LIFECYCLE_JOURNAL_RECORD_FIELDS
    assert first == {
        "index": 0,
        "journal_sequence": 1,
        "observed_at_unix_ms": 1_000,
        "validator_id": VALIDATOR_ID,
        "node_id": NODE_ID,
        "event": "healthy",
        "restart_count": 0,
        "supervisor_generation": 1,
        "process_generation": 1,
        "unexpected_exit_total": 0,
    }
    lines = (export_dir / "peer-window.jsonl").read_bytes().splitlines(
        keepends=True
    )
    header = json.loads(lines[0])
    rows = [json.loads(line) for line in lines[1:]]
    assert header == {
        "baseline": baseline,
        "binding_sha256": binding(),
        "node_id": NODE_ID,
        "record_count": 2,
        "records_sha256": result["raw_records_sha256"],
        "schema": supervisor.LIFECYCLE_RAW_WINDOW_SCHEMA,
        "schema_version": 1,
        "terminal": terminal,
        "validator_id": VALIDATOR_ID,
    }
    assert header["schema"] != public_verifier.LIFECYCLE_JOURNAL_SCHEMA
    assert all(set(row) == public_verifier.LIFECYCLE_JOURNAL_RECORD_FIELDS for row in rows)
    assert [row["index"] for row in rows] == [0, 1]
    assert [row["journal_sequence"] for row in rows] == [2, 3]
    assert result["schema"] == supervisor.LIFECYCLE_RAW_WINDOW_SCHEMA
    assert result["record_count"] == 2
    assert result["sha256"] == hashlib.sha256(b"".join(lines)).hexdigest()


def test_restart_and_unexpected_exit_counters_survive_supervisor_restart(
    tmp_path: Path,
) -> None:
    """Durable counters and generations advance only on their exact events."""

    journal = open_journal(tmp_path)
    root = journal.root
    journal.record("healthy", observed_at_unix_ms=2_000)
    unexpected = journal.record("unexpected_exit", observed_at_unix_ms=2_001)
    restarted = journal.record("restart", observed_at_unix_ms=2_002)
    journal.close()

    assert unexpected["unexpected_exit_total"] == 1
    assert unexpected["process_generation"] == 1
    assert restarted["restart_count"] == 1
    assert restarted["process_generation"] == 2

    reopened = supervisor.LifecycleJournal(
        root, binding(), VALIDATOR_ID, NODE_ID, RESTART_GENERATION
    )
    try:
        with pytest.raises(supervisor.IdentityError, match="records its child"):
            reopened.checkpoint(captured_at_unix_ms=2_003)
        resumed = reopened.record("restart", observed_at_unix_ms=2_003)
        checkpoint = reopened.checkpoint(captured_at_unix_ms=2_004)
        row = checkpoint["validators"][0]
        assert row["supervisor_generation"] == 2
        assert row["process_generation"] == 3
        assert row["restart_count"] == 2
        assert row["unexpected_exit_total"] == 1
        assert resumed["supervisor_generation"] == 2
        healthy = reopened.record("healthy", observed_at_unix_ms=2_005)
        assert healthy["supervisor_generation"] == 2
        assert healthy["process_generation"] == 3
    finally:
        reopened.close()


def test_initial_immediate_exit_is_not_mislabeled_healthy(tmp_path: Path) -> None:
    """A child that dies before its first live poll starts with an exit row."""

    journal = open_journal(tmp_path)
    try:
        record = journal.record("unexpected_exit", observed_at_unix_ms=2_100)
        assert record["event"] == "unexpected_exit"
        assert record["process_generation"] == 1
        assert record["unexpected_exit_total"] == 1
        assert record["restart_count"] == 0
    finally:
        journal.close()


def test_second_writer_cannot_take_active_owner_lease(tmp_path: Path) -> None:
    """One peer binding has exactly one active supervisor writer."""

    journal = open_journal(tmp_path)
    try:
        with pytest.raises(supervisor.IdentityError, match="already held"):
            supervisor.LifecycleJournal(
                journal.root,
                binding(),
                VALIDATOR_ID,
                NODE_ID,
                RESTART_GENERATION,
            )
    finally:
        journal.close()


def test_pending_transition_recovers_after_final_state_publication_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A durable record plus prepared state finalizes exactly once on restart."""

    journal = open_journal(tmp_path)
    root = journal.root
    real_publish = supervisor._publish_lifecycle_file
    state_publications = 0

    def fail_final_state(
        path: Path,
        body: bytes,
        label: str,
        maximum_bytes: int,
        *,
        allow_empty: bool = False,
    ) -> str:
        nonlocal state_publications
        if label == "lifecycle state":
            state_publications += 1
            if state_publications == 2:
                raise OSError("injected final state failure")
        return real_publish(
            path, body, label, maximum_bytes, allow_empty=allow_empty
        )

    monkeypatch.setattr(supervisor, "_publish_lifecycle_file", fail_final_state)
    with pytest.raises(OSError, match="injected final state failure"):
        journal.record("healthy", observed_at_unix_ms=3_000)
    journal.close()
    monkeypatch.setattr(supervisor, "_publish_lifecycle_file", real_publish)

    reopened = supervisor.LifecycleJournal(
        root, binding(), VALIDATOR_ID, NODE_ID, RESTART_GENERATION
    )
    try:
        assert reopened.process_has_started() is True
        assert len((root / supervisor.LifecycleJournal.JOURNAL_FILE).read_bytes().splitlines()) == 1
        resumed = reopened.record("restart", observed_at_unix_ms=3_001)
        assert resumed["journal_sequence"] == 2
    finally:
        reopened.close()


def test_pending_transition_recovers_when_journal_publication_did_not_happen(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Prepared state appends its exact missing row after a publication failure."""

    journal = open_journal(tmp_path)
    root = journal.root
    real_publish = supervisor._publish_lifecycle_file

    def fail_nonempty_journal(
        path: Path,
        body: bytes,
        label: str,
        maximum_bytes: int,
        *,
        allow_empty: bool = False,
    ) -> str:
        if label == "lifecycle journal" and body:
            raise OSError("injected journal failure")
        return real_publish(
            path, body, label, maximum_bytes, allow_empty=allow_empty
        )

    monkeypatch.setattr(supervisor, "_publish_lifecycle_file", fail_nonempty_journal)
    with pytest.raises(OSError, match="injected journal failure"):
        journal.record("healthy", observed_at_unix_ms=4_000)
    journal.close()
    monkeypatch.setattr(supervisor, "_publish_lifecycle_file", real_publish)

    reopened = supervisor.LifecycleJournal(
        root, binding(), VALIDATOR_ID, NODE_ID, RESTART_GENERATION
    )
    try:
        assert reopened.process_has_started() is True
        assert len((root / supervisor.LifecycleJournal.JOURNAL_FILE).read_bytes().splitlines()) == 1
        assert reopened.record("restart", observed_at_unix_ms=4_001)[
            "journal_sequence"
        ] == 2
    finally:
        reopened.close()


def test_active_writer_refuses_same_uid_state_rewrite(tmp_path: Path) -> None:
    """A coherent-looking pathname rewrite cannot redirect the active writer."""

    journal = open_journal(tmp_path)
    journal.record("healthy", observed_at_unix_ms=5_000)
    state = json.loads(journal.state_path.read_bytes())
    state["supervisor_generation"] += 1
    journal.state_path.write_bytes(supervisor.canonical_json_line(state))
    journal.state_path.chmod(0o600)
    try:
        with pytest.raises(supervisor.IdentityError, match="outside the active writer"):
            journal.record("healthy", observed_at_unix_ms=5_001)
    finally:
        journal.close()


def test_active_writer_refuses_lock_path_replacement_during_publication(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A replacement lock inode cannot let a transition report success."""

    journal = open_journal(tmp_path)
    journal.record("healthy", observed_at_unix_ms=5_100)
    real_publish = supervisor._publish_lifecycle_file
    replaced = False

    def replace_lock_before_publish(
        path: Path,
        body: bytes,
        label: str,
        maximum_bytes: int,
        *,
        allow_empty: bool = False,
    ) -> str:
        nonlocal replaced
        if label == "lifecycle state" and not replaced:
            replaced = True
            lock_path = journal.root / supervisor.LifecycleJournal.STATE_LOCK
            lock_path.unlink()
            lock_path.write_bytes(b"")
            lock_path.chmod(0o600)
        return real_publish(
            path,
            body,
            label,
            maximum_bytes,
            allow_empty=allow_empty,
        )

    monkeypatch.setattr(
        supervisor, "_publish_lifecycle_file", replace_lock_before_publish
    )
    try:
        with pytest.raises(supervisor.IdentityError, match="lock path changed"):
            journal.record("healthy", observed_at_unix_ms=5_101)
    finally:
        journal.close()


def test_journal_tamper_is_not_silently_reconciled(tmp_path: Path) -> None:
    """Bytes outside the prepared transition make the journal unusable."""

    journal = open_journal(tmp_path)
    journal.record("healthy", observed_at_unix_ms=6_000)
    with journal.journal_path.open("ab") as stream:
        stream.write(b"\n")
        stream.flush()
        os.fsync(stream.fileno())
    try:
        with pytest.raises(supervisor.IdentityError, match="record is not JSON"):
            journal.record("healthy", observed_at_unix_ms=6_001)
    finally:
        journal.close()


def test_external_checkpoint_is_stable_while_supervisor_owns_writer_lease(
    tmp_path: Path,
) -> None:
    """A collector can capture one state-locked cursor without writer takeover."""

    journal = open_journal(tmp_path)
    try:
        journal.record("healthy", observed_at_unix_ms=7_000)
        checkpoint = supervisor.capture_lifecycle_checkpoint(
            journal.root,
            binding(),
            VALIDATOR_ID,
            NODE_ID,
            captured_at_unix_ms=7_001,
        )
        assert checkpoint == journal.checkpoint(captured_at_unix_ms=7_001)
    finally:
        journal.close()


def test_external_raw_export_rejects_checkpoint_chain_substitution(
    tmp_path: Path,
) -> None:
    """Raw export binds both ends to the local append-only chain."""

    journal = open_journal(tmp_path)
    journal.record("healthy", observed_at_unix_ms=8_000)
    baseline = journal.checkpoint(captured_at_unix_ms=8_001)
    journal.record("healthy", observed_at_unix_ms=8_002)
    terminal = journal.checkpoint(captured_at_unix_ms=8_003)
    valid_baseline_chain = baseline["journal_chain_sha256"]
    root = journal.root
    journal.close()
    baseline["journal_chain_sha256"] = "f" * 64
    export_dir = private_directory(tmp_path / "export")

    with pytest.raises(supervisor.IdentityError, match="checkpoint chain"):
        supervisor.export_lifecycle_raw_window(
            root,
            binding(),
            VALIDATOR_ID,
            NODE_ID,
            baseline,
            terminal,
            export_dir / "raw.jsonl",
        )
    baseline["journal_chain_sha256"] = valid_baseline_chain
    result = supervisor.export_lifecycle_raw_window(
        root,
        binding(),
        VALIDATOR_ID,
        NODE_ID,
        baseline,
        terminal,
        export_dir / "valid-raw.jsonl",
    )
    assert result["schema"] == supervisor.LIFECYCLE_RAW_WINDOW_SCHEMA
    assert result["record_count"] == 1


def test_unsafe_root_entry_and_mode_fail_closed(tmp_path: Path) -> None:
    """The journal refuses ambiguous contents and a non-private root."""

    parent = private_directory(tmp_path / "runtime")
    root = private_directory(parent / "lifecycle")
    (root / "attacker").write_text("unexpected", encoding="ascii")
    with pytest.raises(supervisor.IdentityError, match="unexpected entries"):
        supervisor.LifecycleJournal(
            root, binding(), VALIDATOR_ID, NODE_ID, RESTART_GENERATION
        )
    (root / "attacker").unlink()
    root.chmod(0o755)
    with pytest.raises(supervisor.IdentityError, match="owner-private"):
        supervisor.LifecycleJournal(
            root, binding(), VALIDATOR_ID, NODE_ID, RESTART_GENERATION
        )


def supervisor_argv(tmp_path: Path) -> list[str]:
    """Build one complete parser fixture without starting a supervisor."""

    binary = Path("/usr/bin/true")
    config = tmp_path / "peer.toml"
    config.write_text("[torii]\n", encoding="ascii")
    workdir = private_directory(tmp_path / "work")
    storage = private_directory(tmp_path / "storage")
    return [
        "--binary",
        str(binary),
        "--binary-sha256",
        hashlib.sha256(binary.read_bytes()).hexdigest(),
        "--config",
        str(config),
        "--config-sha256",
        hashlib.sha256(config.read_bytes()).hexdigest(),
        "--workdir",
        str(workdir),
        "--workdir-device",
        str(workdir.stat().st_dev),
        "--workdir-inode",
        str(workdir.stat().st_ino),
        "--storage-dir",
        str(storage),
        "--storage-device",
        str(storage.stat().st_dev),
        "--storage-inode",
        str(storage.stat().st_ino),
        "--pid-file",
        str(tmp_path / "peer.pid"),
        "--terminal-unhealthy-file",
        str(tmp_path / "terminal" / "peer.json"),
        "--restart-generation",
        RESTART_GENERATION,
        "--lifecycle-journal-root",
        str(tmp_path / "lifecycle" / VALIDATOR_ID),
        "--validator-id",
        VALIDATOR_ID,
        "--node-id",
        NODE_ID,
    ]


def test_lifecycle_cli_identity_is_mandatory(tmp_path: Path) -> None:
    """Every supervisor invocation must carry one complete peer identity."""

    argv = supervisor_argv(tmp_path)
    parsed = supervisor.parse_args(argv)
    assert parsed.lifecycle_journal_root == str(tmp_path / "lifecycle" / VALIDATOR_ID)
    for option in ("--lifecycle-journal-root", "--validator-id", "--node-id"):
        index = argv.index(option)
        incomplete = [*argv[:index], *argv[index + 2 :]]
        with pytest.raises(SystemExit):
            supervisor.parse_args(incomplete)
    invalid_node_id = list(argv)
    invalid_node_id[invalid_node_id.index("--node-id") + 1] = "taira-node:validator-1"
    with pytest.raises(SystemExit):
        supervisor.parse_args(invalid_node_id)


def test_supervisor_runtime_emits_periodic_health_without_exit_event(
    tmp_path: Path,
) -> None:
    """The real wait loop journals live health and clean shutdown stays clean."""

    binary = tmp_path / "waiting-validator"
    binary.write_text(
        "#!/bin/sh\ntrap 'exit 0' TERM INT HUP\nwhile :; do /bin/sleep 0.02; done\n",
        encoding="ascii",
    )
    binary.chmod(0o700)
    config = tmp_path / "peer.toml"
    config.write_text("[torii]\n", encoding="ascii")
    workdir = private_directory(tmp_path / "work")
    storage = private_directory(tmp_path / "storage")
    lifecycle_parent = private_directory(tmp_path / "lifecycle-parent")
    lifecycle_root = lifecycle_parent / "peer"
    terminal = tmp_path / "terminal" / "peer.json"
    command = [
        sys.executable,
        str(Path(supervisor.__file__).resolve()),
        "--binary",
        str(binary),
        "--binary-sha256",
        hashlib.sha256(binary.read_bytes()).hexdigest(),
        "--config",
        str(config),
        "--config-sha256",
        hashlib.sha256(config.read_bytes()).hexdigest(),
        "--workdir",
        str(workdir),
        "--workdir-device",
        str(workdir.stat().st_dev),
        "--workdir-inode",
        str(workdir.stat().st_ino),
        "--storage-dir",
        str(storage),
        "--storage-device",
        str(storage.stat().st_dev),
        "--storage-inode",
        str(storage.stat().st_ino),
        "--pid-file",
        str(tmp_path / "peer.pid"),
        "--terminal-unhealthy-file",
        str(terminal),
        "--restart-generation",
        RESTART_GENERATION,
        "--lifecycle-journal-root",
        str(lifecycle_root),
        "--validator-id",
        VALIDATOR_ID,
        "--node-id",
        NODE_ID,
        "--lifecycle-healthy-interval-seconds",
        "0.03",
    ]
    process = subprocess.Popen(
        command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    try:
        deadline = time.monotonic() + 5
        journal_path = lifecycle_root / supervisor.LifecycleJournal.JOURNAL_FILE
        while time.monotonic() < deadline:
            if process.poll() is not None:
                pytest.fail("journal-enabled supervisor exited before health capture")
            try:
                lines = journal_path.read_bytes().splitlines()
            except FileNotFoundError:
                lines = []
            if len(lines) >= 3:
                break
            time.sleep(0.01)
        assert len(lines) >= 3
    finally:
        process.terminate()
        process.wait(timeout=3)

    assert process.returncode == 0
    state = supervisor._decode_lifecycle_state(
        (lifecycle_root / supervisor.LifecycleJournal.STATE_FILE).read_bytes()
    )
    records = supervisor._decode_lifecycle_records(journal_path.read_bytes())
    assert state["unexpected_exit_total"] == 0
    assert state["restart_count"] == 0
    assert state["process_generation"] == 1
    assert {record["event"] for record in records} == {"healthy"}
    assert not terminal.exists()
