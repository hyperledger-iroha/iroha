"""Adversarial coverage for the Taikai ingest smoke harness."""

from __future__ import annotations

import json
import os
import subprocess
import textwrap
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "taikai_ingest_smoke.sh"


def _minimal_fixture(label: str) -> dict[str, object]:
    return {
        "label": label,
        "payload_hex": "00",
        "args": {
            "event_id": "event",
            "stream_id": "stream",
            "rendition_id": "rendition",
            "track_kind": "data",
            "codec": "custom:test",
            "bitrate_kbps": 1,
            "segment_sequence": 0,
            "segment_start_pts": 0,
            "segment_duration": 1,
            "wallclock_unix_ms": 1,
            "manifest_hash": "11" * 32,
            "storage_ticket": "22" * 32,
        },
    }


def _write_successful_fake_bundler(path: Path) -> None:
    path.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json
            import pathlib
            import sys

            args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
            pathlib.Path(args["--car-out"]).write_bytes(b"car")
            pathlib.Path(args["--envelope-out"]).write_bytes(b"envelope")
            pathlib.Path(args["--indexes-out"]).write_text(
                json.dumps({"cid_key": {}, "time_key": {}}), encoding="utf-8"
            )
            pathlib.Path(args["--ingest-metadata-out"]).write_text(
                "{}", encoding="utf-8"
            )
            """
        ),
        encoding="utf-8",
    )
    path.chmod(0o700)


def test_fixture_label_cannot_escape_output_directory(tmp_path: Path) -> None:
    """A fixture label must never become an `rm -rf` traversal target."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "new" / "nested" / "output"
    fixtures.mkdir(parents=True)
    sentinel = workspace / "must-survive.txt"
    sentinel.write_text("preserve me", encoding="utf-8")

    fixture = fixtures / "malicious.json"
    fixture.write_text(json.dumps({"label": ".."}), encoding="utf-8")
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "unsafe label" in result.stderr
    assert sentinel.read_text(encoding="utf-8") == "preserve me"


def test_preexisting_unowned_label_directory_is_never_deleted(tmp_path: Path) -> None:
    """A valid label must not authorize recursive deletion under a user-selected output root."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    run_dir = output / "existing"
    fixtures.mkdir(parents=True)
    run_dir.mkdir(parents=True)
    sentinel = run_dir / "must-survive.txt"
    sentinel.write_text("preserve me", encoding="utf-8")
    (fixtures / "existing.json").write_text(
        json.dumps({"label": "existing"}), encoding="utf-8"
    )
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "refusing to reuse unowned" in result.stderr
    assert sentinel.read_text(encoding="utf-8") == "preserve me"


def test_owned_label_directory_cleanup_preserves_unrelated_files(tmp_path: Path) -> None:
    """Rerunning an owned fixture removes generated artifacts only."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    run_dir = output / "existing"
    fixtures.mkdir(parents=True)
    run_dir.mkdir(parents=True)
    (run_dir / ".taikai_ingest_smoke_owned_v1").write_text(
        "taikai-ingest-smoke-owned-v1\n", encoding="utf-8"
    )
    sentinel = run_dir / "must-survive.txt"
    sentinel.write_text("preserve me", encoding="utf-8")
    (run_dir / "segment.car").write_text("stale", encoding="utf-8")
    (fixtures / "existing.json").write_text(
        json.dumps(
            {
                "label": "existing",
                "payload_hex": "00",
                "args": {
                    "event_id": "event",
                    "stream_id": "stream",
                    "rendition_id": "rendition",
                    "track_kind": "data",
                    "codec": "custom:test",
                    "bitrate_kbps": 1,
                    "segment_sequence": 0,
                    "segment_start_pts": 0,
                    "segment_duration": 1,
                    "wallclock_unix_ms": 1,
                    "manifest_hash": "11" * 32,
                    "storage_ticket": "22" * 32,
                },
            }
        ),
        encoding="utf-8",
    )
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json
            import pathlib
            import sys

            args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
            pathlib.Path(args["--car-out"]).write_bytes(b"new-car")
            pathlib.Path(args["--envelope-out"]).write_bytes(b"envelope")
            pathlib.Path(args["--indexes-out"]).write_text(
                json.dumps({"cid_key": {}, "time_key": {}}), encoding="utf-8"
            )
            pathlib.Path(args["--ingest-metadata-out"]).write_text(
                "{}", encoding="utf-8"
            )
            """
        ),
        encoding="utf-8",
    )
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert sentinel.read_text(encoding="utf-8") == "preserve me"
    assert (run_dir / "segment.car").read_bytes() == b"new-car"


def test_successful_bundler_must_emit_car_and_envelope(tmp_path: Path) -> None:
    """A zero-exit bundler cannot make the harness pass without its core outputs."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    fixtures.mkdir(parents=True)
    output.mkdir()
    fixture = fixtures / "missing-outputs.json"
    fixture.write_text(
        json.dumps(
            {
                "label": "missing-outputs",
                "payload_hex": "00",
                "args": {
                    "event_id": "event",
                    "stream_id": "stream",
                    "rendition_id": "rendition",
                    "track_kind": "data",
                    "codec": "custom:test",
                    "bitrate_kbps": 1,
                    "segment_sequence": 0,
                    "segment_start_pts": 0,
                    "segment_duration": 1,
                    "wallclock_unix_ms": 1,
                    "manifest_hash": "11" * 32,
                    "storage_ticket": "22" * 32,
                },
            }
        ),
        encoding="utf-8",
    )
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json
            import pathlib
            import sys

            args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
            pathlib.Path(args["--indexes-out"]).write_text(
                json.dumps({"cid_key": {}, "time_key": {}}), encoding="utf-8"
            )
            pathlib.Path(args["--ingest-metadata-out"]).write_text(
                "{}", encoding="utf-8"
            )
            """
        ),
        encoding="utf-8",
    )
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "segment.car missing or empty" in result.stderr
    assert "segment.norito missing or empty" in result.stderr


def test_fixture_newline_cannot_inject_an_extra_cli_flag(tmp_path: Path) -> None:
    """Fixture values are transported as arguments rather than line-split flags."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    fixtures.mkdir(parents=True)
    output.mkdir()
    injected_output = workspace / "injected.json"
    fixture = fixtures / "newline.json"
    fixture.write_text(
        json.dumps(
            {
                "label": "newline",
                "payload_hex": "00",
                "args": {
                    "event_id": "event",
                    "stream_id": "stream",
                    "rendition_id": "rendition",
                    "track_kind": "data",
                    "codec": "custom:test",
                    "bitrate_kbps": 1,
                    "segment_sequence": 0,
                    "segment_start_pts": 0,
                    "segment_duration": 1,
                    "wallclock_unix_ms": 1,
                    "manifest_hash": "11" * 32,
                    "storage_ticket": "22" * 32,
                    "ingest_node_id": (
                        f"node-a\n--summary-out\n{injected_output}"
                    ),
                },
            }
        ),
        encoding="utf-8",
    )
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env python3
            import json
            import pathlib
            import sys

            if "--summary-out" in sys.argv:
                raise SystemExit(88)
            args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
            pathlib.Path(args["--car-out"]).write_bytes(b"car")
            pathlib.Path(args["--envelope-out"]).write_bytes(b"envelope")
            pathlib.Path(args["--indexes-out"]).write_text(
                json.dumps({"cid_key": {}, "time_key": {}}), encoding="utf-8"
            )
            pathlib.Path(args["--ingest-metadata-out"]).write_text(
                "{}", encoding="utf-8"
            )
            """
        ),
        encoding="utf-8",
    )
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert not injected_output.exists()


def test_symlinked_path_ancestors_are_canonicalized(tmp_path: Path) -> None:
    """Existing symlink ancestors are resolved even when the output does not exist yet."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    fixtures.mkdir(parents=True)
    linked_workspace = tmp_path / "workspace-link"
    linked_workspace.symlink_to(workspace, target_is_directory=True)
    fixture = fixtures / "canonical-path.json"
    fixture.write_text(
        json.dumps(
            {
                "label": "canonical-path",
                "payload_hex": "00",
                "args": {
                    "event_id": "event",
                    "stream_id": "stream",
                    "rendition_id": "rendition",
                    "track_kind": "data",
                    "codec": "custom:test",
                    "bitrate_kbps": 1,
                    "segment_sequence": 0,
                    "segment_start_pts": 0,
                    "segment_duration": 1,
                    "wallclock_unix_ms": 1,
                    "manifest_hash": "11" * 32,
                    "storage_ticket": "22" * 32,
                },
            }
        ),
        encoding="utf-8",
    )
    fake_bundler = workspace / "taikai_car"
    fake_bundler.write_text(
        textwrap.dedent(
            f"""\
            #!/usr/bin/env python3
            import json
            import pathlib
            import sys

            symlink_prefix = {str(linked_workspace)!r}
            if any(value.startswith(symlink_prefix) for value in sys.argv[1:]):
                raise SystemExit(87)
            args = dict(zip(sys.argv[1::2], sys.argv[2::2]))
            pathlib.Path(args["--car-out"]).write_bytes(b"car")
            pathlib.Path(args["--envelope-out"]).write_bytes(b"envelope")
            pathlib.Path(args["--indexes-out"]).write_text(
                json.dumps({{"cid_key": {{}}, "time_key": {{}}}}), encoding="utf-8"
            )
            pathlib.Path(args["--ingest-metadata-out"]).write_text(
                "{{}}", encoding="utf-8"
            )
            """
        ),
        encoding="utf-8",
    )
    fake_bundler.chmod(0o700)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(linked_workspace),
            "--fixtures",
            str(linked_workspace / "fixtures"),
            "--out",
            str(linked_workspace / "new" / "nested" / "output"),
            "--taikai-car",
            str(linked_workspace / "taikai_car"),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert (workspace / "new" / "nested" / "output" / "canonical-path").is_dir()


def test_duplicate_fixture_labels_are_rejected(tmp_path: Path) -> None:
    """Two fixtures must not silently overwrite the same result directory."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    fixtures.mkdir(parents=True)
    (fixtures / "first.json").write_text(
        json.dumps(_minimal_fixture("duplicate")), encoding="utf-8"
    )
    (fixtures / "second.json").write_text(
        json.dumps(_minimal_fixture("duplicate")), encoding="utf-8"
    )
    fake_bundler = workspace / "taikai_car"
    _write_successful_fake_bundler(fake_bundler)

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(fake_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "duplicate Taikai fixture label 'duplicate'" in result.stderr


def test_cleanup_never_deletes_selected_bundler_alias(tmp_path: Path) -> None:
    """An allowlisted output hard link must not authorize deleting the bundler."""

    workspace = tmp_path / "workspace"
    fixtures = workspace / "fixtures"
    output = workspace / "output"
    run_dir = output / "protected"
    fixtures.mkdir(parents=True)
    run_dir.mkdir(parents=True)
    (run_dir / ".taikai_ingest_smoke_owned_v1").write_text(
        "taikai-ingest-smoke-owned-v1\n", encoding="utf-8"
    )
    (fixtures / "protected.json").write_text(
        json.dumps(_minimal_fixture("protected")), encoding="utf-8"
    )
    real_bundler = workspace / "taikai_car"
    _write_successful_fake_bundler(real_bundler)
    generated_alias = run_dir / "segment.car"
    os.link(real_bundler, generated_alias)
    original = real_bundler.read_bytes()

    result = subprocess.run(
        [
            "bash",
            str(SCRIPT),
            "--workspace",
            str(workspace),
            "--fixtures",
            str(fixtures),
            "--out",
            str(output),
            "--taikai-car",
            str(real_bundler),
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode != 0
    assert "aliases the taikai_car executable" in result.stderr
    assert real_bundler.read_bytes() == original
    assert generated_alias.read_bytes() == original
