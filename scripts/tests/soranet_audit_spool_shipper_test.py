"""Security regressions for the SoraNet audit spool shipper."""

import importlib.util
import json
import os
import signal
import stat
import subprocess
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).parents[1] / "soranet_audit_spool_shipper.py"
SPEC = importlib.util.spec_from_file_location("soranet_audit_spool_shipper", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
SHIPPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SHIPPER)


def private_dir(path: Path) -> Path:
    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def event(path: Path, payload: object) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    path.chmod(0o600)
    return path


@pytest.mark.skipif(not hasattr(os, "geteuid"), reason="Unix custody test")
def test_archive_is_private_complete_and_sources_move_without_clobber(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    archives = private_dir(tmp_path / "archives")
    processed = private_dir(tmp_path / "processed")
    source = event(spool / "event.json", {"z": 1, "a": "event"})
    captured = list(SHIPPER.iter_spool_files(spool))

    archive = SHIPPER.write_archive(captured, archives, False)
    assert archive.read_text(encoding="utf-8") == '{"a":"event","z":1}\n'
    metadata = archive.stat()
    assert stat.S_IMODE(metadata.st_mode) == 0o600
    assert metadata.st_nlink == 1
    assert not list(archives.glob(".compliance-archive-*"))

    SHIPPER.cleanup_batch(captured, processed, False)
    assert not source.exists()
    assert (processed / source.name).read_text(encoding="utf-8")


@pytest.mark.skipif(not hasattr(os, "geteuid"), reason="Unix custody test")
def test_scanned_event_rejects_in_place_mutation_before_archive(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    archives = private_dir(tmp_path / "archives")
    source = event(spool / "event.json", {"event": "first"})
    captured = list(SHIPPER.iter_spool_files(spool))
    source.write_text(json.dumps({"event": "other"}), encoding="utf-8")
    source.chmod(0o600)

    with pytest.raises(ValueError, match="changed after spool scanning"):
        SHIPPER.write_archive(captured, archives, False)


@pytest.mark.skipif(not hasattr(os, "geteuid"), reason="Unix custody test")
def test_processed_publication_rejects_replacement_and_never_clobbers(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    processed = private_dir(tmp_path / "processed")
    source = event(spool / "event.json", {"event": "first"})
    captured = list(SHIPPER.iter_spool_files(spool))
    source.unlink()
    event(source, {"event": "replacement"})
    with pytest.raises(ValueError, match="changed before processed publication"):
        SHIPPER.cleanup_batch(captured, processed, False)

    source.unlink()
    source = event(spool / "event.json", {"event": "fresh"})
    captured = list(SHIPPER.iter_spool_files(spool))
    existing = event(processed / source.name, {"event": "existing"})
    with pytest.raises(FileExistsError):
        SHIPPER.cleanup_batch(captured, processed, False)
    assert json.loads(existing.read_text(encoding="utf-8")) == {"event": "existing"}
    assert source.exists()


@pytest.mark.skipif(not hasattr(os, "geteuid"), reason="Unix custody test")
def test_processed_batch_rolls_back_all_links_before_any_source_is_removed(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    processed = private_dir(tmp_path / "processed")
    first = event(spool / "a.json", {"event": "first"})
    second = event(spool / "b.json", {"event": "second"})
    captured = list(SHIPPER.iter_spool_files(spool))
    existing = event(processed / second.name, {"event": "preexisting"})

    with pytest.raises(FileExistsError):
        SHIPPER.cleanup_batch(captured, processed, False)

    assert first.exists()
    assert second.exists()
    assert not (processed / first.name).exists()
    assert json.loads(existing.read_text(encoding="utf-8")) == {"event": "preexisting"}


@pytest.mark.skipif(not hasattr(os, "symlink"), reason="symlink test")
def test_spool_rejects_symlink_and_hardlink_events(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    target = event(tmp_path / "target", {"event": "target"})
    (spool / "linked.json").symlink_to(target)
    with pytest.raises(ValueError, match="single-link regular file"):
        list(SHIPPER.iter_spool_files(spool))

    (spool / "linked.json").unlink()
    os.link(target, spool / "hardlinked.json")
    with pytest.raises(ValueError, match="single-link regular file"):
        list(SHIPPER.iter_spool_files(spool))


def test_ship_command_is_literal_and_requires_absolute_trusted_program(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    archive = event(tmp_path / "archive.jsonl", {"event": "test"})
    with pytest.raises(ValueError, match="absolute"):
        SHIPPER.ship_archive(
            archive, ["echo", "{archive}", "$(touch /tmp/not-run)"], False
        )


def test_shipping_process_isolated_and_descendants_are_swept(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    archive = tmp_path / "archive.jsonl"
    observed = {}
    signals = []

    class CompletedProcess:
        pid = 4242

        def wait(self, timeout=None):
            observed["timeout"] = timeout
            return 0

        def poll(self):
            return 0

    def popen(arguments, **kwargs):
        observed["arguments"] = arguments
        observed["kwargs"] = kwargs
        return CompletedProcess()

    monkeypatch.setattr(SHIPPER, "_validate_ship_program", lambda _program: "/bin/ship")
    monkeypatch.setattr(SHIPPER.subprocess, "Popen", popen)
    monkeypatch.setattr(SHIPPER.os, "killpg", lambda group, sig: signals.append((group, sig)))
    SHIPPER.ship_archive(archive, ["/bin/ship", "{archive}"], False)

    assert observed["arguments"] == ["/bin/ship", str(archive)]
    assert observed["kwargs"]["start_new_session"] is True
    assert observed["kwargs"]["stdin"] is subprocess.DEVNULL
    assert observed["timeout"] == SHIPPER.SHIP_TIMEOUT_SECONDS
    assert signals == [(4242, signal.SIGKILL)]


def test_shipping_timeout_terminates_the_complete_process_group(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    archive = tmp_path / "archive.jsonl"
    signals = []

    class TimedOutProcess:
        pid = 4343
        waits = 0

        def wait(self, timeout=None):
            self.waits += 1
            if self.waits == 1:
                raise subprocess.TimeoutExpired("ship", timeout)
            return -signal.SIGTERM

        def poll(self):
            return -signal.SIGTERM

    monkeypatch.setattr(SHIPPER, "_validate_ship_program", lambda _program: "/bin/ship")
    monkeypatch.setattr(
        SHIPPER.subprocess,
        "Popen",
        lambda *_args, **_kwargs: TimedOutProcess(),
    )
    monkeypatch.setattr(SHIPPER.os, "killpg", lambda group, sig: signals.append((group, sig)))

    with pytest.raises(subprocess.TimeoutExpired):
        SHIPPER.ship_archive(archive, ["/bin/ship", "{archive}"], False)
    assert signals == [(4343, signal.SIGTERM), (4343, signal.SIGKILL)]


def test_shipping_environment_is_allowlisted(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    archive = tmp_path / "archive.jsonl"
    monkeypatch.setenv("SHIP_TOKEN", "runtime-only")
    monkeypatch.setenv("UNREQUESTED_SECRET", "must-not-leak")
    environment = SHIPPER._shipping_environment(archive, ["SHIP_TOKEN"])
    assert environment["SHIP_TOKEN"] == "runtime-only"
    assert "UNREQUESTED_SECRET" not in environment
    with pytest.raises(ValueError, match="unsafe"):
        SHIPPER._shipping_environment(archive, ["LD_PRELOAD"])


def test_secure_directory_rejects_relative_and_permissive_leaf(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    with pytest.raises(ValueError, match="absolute"):
        SHIPPER.secure_directory(Path("relative"), "test")
    permissive = private_dir(tmp_path / "permissive")
    permissive.chmod(0o750)
    with pytest.raises(ValueError, match="mode 0700"):
        SHIPPER.secure_directory(permissive, "test")


@pytest.mark.skipif(
    not hasattr(os, "O_NOFOLLOW") or not hasattr(os, "O_DIRECTORY"),
    reason="Unix directory-lock custody test",
)
def test_spool_lock_rejects_concurrent_shipper(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    spool = private_dir(tmp_path / "spool")
    with SHIPPER.exclusive_spool_lock(spool):
        with pytest.raises(RuntimeError, match="another SoraNet audit shipper"):
            with SHIPPER.exclusive_spool_lock(spool):
                pytest.fail("a second shipper must not enter the critical section")
