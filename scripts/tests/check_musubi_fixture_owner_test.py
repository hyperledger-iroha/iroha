"""Static and adversarial coverage for the signed Musubi fixture owner."""

from __future__ import annotations

import io
import json
import os
from pathlib import Path
import stat
import sys

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
sys.path.insert(0, os.fspath(SCRIPTS_ROOT))

import check_musubi_fixtures as checker  # noqa: E402
import write_musubi_fixtures as writer  # noqa: E402


def _fixture_contents(relative_path: str) -> bytes:
    return (
        json.dumps(
            {"fixture": Path(relative_path).name, "network_id": "hash:a5"},
            indent=2,
            sort_keys=True,
        )
        + "\n"
    ).encode()


def _rendered_outputs() -> tuple[writer.RenderedOutput, ...]:
    return tuple(
        writer.RenderedOutput(path, _fixture_contents(path)) for path in writer.OUTPUTS
    )


def _owner_envelope(
    outputs: tuple[writer.RenderedOutput, ...] | None = None,
) -> bytes:
    if outputs is None:
        outputs = _rendered_outputs()
    value = {
        "schema": writer.OWNER_SCHEMA,
        "outputs": [
            {
                "path": output.relative_path,
                "contents": output.contents.decode(),
            }
            for output in outputs
        ],
    }
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()


def _private_directory(path: Path) -> Path:
    path.mkdir(mode=0o700, parents=True, exist_ok=False)
    path.chmod(0o700)
    return path


def _external_test_root(tmp_path: Path, name: str = "stage") -> Path:
    # macOS exposes /var as a symlink; the production writer intentionally
    # rejects symbolic path components, so tests pass the physical path.
    parent = tmp_path.resolve()
    return _private_directory(parent / name)


def test_owner_is_registered_for_the_exact_two_outputs() -> None:
    manifest = tomllib.loads(
        (REPO_ROOT / "generated-files.toml").read_text(encoding="utf-8")
    )
    owners = [
        owner
        for owner in manifest["generated"]
        if owner["name"] == "musubi-v1-signed-fixtures"
    ]
    assert len(owners) == 1
    owner = owners[0]
    expected_outputs = set(writer.OUTPUTS)
    assert set(owner["outputs"]) == expected_outputs
    for output in expected_outputs:
        assert [
            candidate["name"]
            for candidate in manifest["generated"]
            if output in candidate.get("outputs", [])
        ] == ["musubi-v1-signed-fixtures"]
    assert owner["kind"] == "file"
    assert owner["check"].endswith("python3 scripts/check_musubi_fixtures.py")
    assert "IROHA_MUSUBI_FIXTURE_CARGO_TARGET_DIR" in owner["check"]
    assert "python3 scripts/write_musubi_fixtures.py" in owner["generator"]
    assert "IROHA_MUSUBI_FIXTURE_CARGO_TARGET_DIR" in owner["generator"]
    assert "--output-root" in owner["generator"]
    assert "IROHA_MUSUBI_FIXTURE_OUTPUT_ROOT" in owner["generator"]
    assert "empty private external staging root" in owner["generator"]
    assert set(owner["generator_sources"]) == {
        "crates/iroha_data_model/Cargo.toml",
        "crates/iroha_data_model/src/bin/musubi_fixture_values.rs",
        "crates/iroha_data_model/src/bin/musubi_fixtures.rs",
        "crates/iroha_data_model/src/bin/musubi_sdk_fixture_values.rs",
        "scripts/check_musubi_fixtures.py",
        "scripts/write_musubi_fixtures.py",
    }
    assert set(owner["inputs"]) == {
        "crates/iroha_data_model/src/id.rs",
        "crates/iroha_data_model/src/isi/mod.rs",
        "crates/iroha_data_model/src/isi/musubi.rs",
        "crates/iroha_data_model/src/musubi.rs",
        "crates/iroha_data_model/src/musubi/query_models.rs",
        "crates/iroha_data_model/src/sorafs/pin_registry.rs",
    }
    assert not set(owner["outputs"]).intersection(owner["generator_sources"])
    assert not set(owner["outputs"]).intersection(owner["inputs"])


def test_owner_adversarial_suite_is_registered_in_pr_ci() -> None:
    workflow = (REPO_ROOT / ".github/workflows/pr.yml").read_text(encoding="utf-8")
    assert "scripts/tests/check_musubi_fixture_owner_test.py" in workflow


def test_owner_command_is_argument_free() -> None:
    command = writer.owner_command()
    assert command[-2:] == ["--bin", "musubi_fixtures"]
    assert command[command.index("--jobs") + 1] == "1"
    assert set(command[command.index("--features") + 1].split(",")) == {
        "dev-tools",
        "test-fixtures",
        "json",
        "transparent_api",
    }
    assert "--locked" in command
    assert "--offline" in command
    assert "-Z" not in command
    assert "unstable-options" not in command
    assert "--lockfile-path" not in command
    assert "--write" not in command
    assert "--check" not in command
    assert "--output-root" not in command


def test_owner_target_requires_an_explicit_private_external_directory(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    with pytest.raises(RuntimeError, match="CARGO_TARGET_DIR must name"):
        writer.resolve_owner_cargo_target_dir()

    relative = Path("relative-target")
    monkeypatch.setenv("CARGO_TARGET_DIR", os.fspath(relative))
    with pytest.raises(RuntimeError, match="must be absolute"):
        writer.resolve_owner_cargo_target_dir()

    cargo_target = _external_test_root(tmp_path, "cargo-target")
    monkeypatch.setenv("CARGO_TARGET_DIR", os.fspath(cargo_target))
    assert writer.resolve_owner_cargo_target_dir() == cargo_target.resolve()


def test_owner_run_forces_the_supplied_external_cargo_target(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    cargo_target = _external_test_root(tmp_path, "cargo-target")
    captured: dict[str, object] = {}

    class FakeProcess:
        def __init__(self) -> None:
            self.stdout = io.BytesIO(_owner_envelope())

        @staticmethod
        def wait() -> int:
            return 0

    def popen(*args: object, **kwargs: object) -> FakeProcess:
        captured["args"] = args
        captured["kwargs"] = kwargs
        return FakeProcess()

    monkeypatch.setattr(writer.subprocess, "Popen", popen)
    assert writer.run_owner(cargo_target) == _owner_envelope()
    environment = captured["kwargs"]["env"]
    assert isinstance(environment, dict)
    assert environment["CARGO_TARGET_DIR"] == os.fspath(cargo_target.resolve())
    assert Path(environment["CARGO_TARGET_DIR"]) != REPO_ROOT / "target"


def test_envelope_accepts_only_the_exact_ordered_closed_pair() -> None:
    expected = _rendered_outputs()
    assert writer.parse_owner_envelope(_owner_envelope()) == expected

    with pytest.raises(RuntimeError, match="closed V1 pair"):
        writer.parse_owner_envelope(_owner_envelope(expected[:1]))
    with pytest.raises(RuntimeError, match="closed V1 pair"):
        writer.parse_owner_envelope(_owner_envelope(tuple(reversed(expected))))
    extra = (*expected, writer.RenderedOutput("fixtures/musubi/extra.json", b"{}\n"))
    with pytest.raises(RuntimeError, match="closed V1 pair"):
        writer.parse_owner_envelope(_owner_envelope(extra))


def test_envelope_rejects_duplicate_keys_and_noncanonical_boundaries() -> None:
    duplicate = (
        b'{"schema":"'
        + writer.OWNER_SCHEMA.encode()
        + b'","schema":"'
        + writer.OWNER_SCHEMA.encode()
        + b'","outputs":[]}\n'
    )
    with pytest.raises(RuntimeError, match="duplicate JSON key"):
        writer.parse_owner_envelope(duplicate)
    with pytest.raises(RuntimeError, match="trailing newline"):
        writer.parse_owner_envelope(_owner_envelope().rstrip(b"\n"))


@pytest.mark.parametrize(
    "legacy_key", ["chain_id", "genesis_hash", "genesis_block_hash"]
)
def test_legacy_deployment_keys_are_rejected_at_any_depth(legacy_key: str) -> None:
    with pytest.raises(RuntimeError, match=legacy_key):
        writer.reject_legacy_keys(
            {"network_id": "hash:a5", "nested": [{"deeper": {legacy_key: "old"}}]}
        )
    writer.reject_legacy_keys({"network_id": "hash:a5", "nested": []})


def test_descriptor_writer_stages_the_exact_private_pair(tmp_path: Path) -> None:
    root = _external_test_root(tmp_path)
    expected = _rendered_outputs()
    writer.write_outputs(root, expected)

    assert writer.read_closed_outputs(root) == expected
    assert {entry.name for entry in (root / "fixtures/musubi").iterdir()} == set(
        writer.OUTPUT_BASENAMES
    )
    assert stat.S_IMODE((root / "fixtures").stat().st_mode) == 0o700
    assert stat.S_IMODE((root / "fixtures/musubi").stat().st_mode) == 0o700
    for relative_path in writer.OUTPUTS:
        assert stat.S_IMODE((root / relative_path).stat().st_mode) == 0o600


def test_descriptor_writer_rejects_a_nonempty_root_without_mutating_it(
    tmp_path: Path,
) -> None:
    root = _external_test_root(tmp_path)
    extra = root / "unrelated-sentinel"
    extra.write_bytes(b"outside sentinel\n")
    extra.chmod(0o600)

    with pytest.raises(RuntimeError, match="does not contain its exact closed set"):
        writer.write_outputs(root, _rendered_outputs())
    assert extra.read_bytes() == b"outside sentinel\n"
    assert not any((root / path).exists() for path in writer.OUTPUTS)


def test_descriptor_writer_rejects_preexisting_output_without_overwrite(
    tmp_path: Path,
) -> None:
    root = _external_test_root(tmp_path)
    fixtures = _private_directory(root / "fixtures")
    musubi = _private_directory(fixtures / "musubi")
    output = musubi / writer.OUTPUT_BASENAMES[0]
    output.write_bytes(b"preexisting output\n")
    output.chmod(0o600)

    with pytest.raises(RuntimeError, match="does not contain its exact closed set"):
        writer.write_outputs(root, _rendered_outputs())
    assert output.read_bytes() == b"preexisting output\n"
    assert not (musubi / writer.OUTPUT_BASENAMES[1]).exists()


def test_second_file_failure_retains_stage_and_never_mutates_unrelated_files(
    tmp_path: Path,
) -> None:
    root = _external_test_root(tmp_path, "stage")
    outside = _external_test_root(tmp_path, "outside")
    outside_sentinel = outside / "sentinel.json"
    outside_sentinel.write_bytes(b"outside sentinel\n")
    injected_output = b"attacker-owned second output\n"
    unrelated = b"unrelated failed-stage evidence\n"

    def obstruct_second_output(directory_fd: int, name: str) -> None:
        if name != writer.OUTPUT_BASENAMES[1]:
            return
        output_fd = os.open(
            name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600,
            dir_fd=directory_fd,
        )
        try:
            os.write(output_fd, injected_output)
            os.fsync(output_fd)
        finally:
            os.close(output_fd)
        unrelated_fd = os.open(
            "unrelated.keep",
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600,
            dir_fd=directory_fd,
        )
        try:
            os.write(unrelated_fd, unrelated)
            os.fsync(unrelated_fd)
        finally:
            os.close(unrelated_fd)

    with pytest.raises(RuntimeError, match="created exclusively"):
        writer.write_outputs(
            root,
            _rendered_outputs(),
            before_output_open=obstruct_second_output,
        )
    musubi = root / "fixtures/musubi"
    assert (musubi / writer.OUTPUT_BASENAMES[0]).read_bytes() == _rendered_outputs()[
        0
    ].contents
    assert (musubi / writer.OUTPUT_BASENAMES[1]).read_bytes() == injected_output
    assert (musubi / "unrelated.keep").read_bytes() == unrelated
    assert outside_sentinel.read_bytes() == b"outside sentinel\n"


def test_descriptor_writer_rejects_a_non_private_root(tmp_path: Path) -> None:
    root = _external_test_root(tmp_path)
    root.chmod(0o755)

    with pytest.raises(RuntimeError, match="group or other permissions"):
        writer.write_outputs(root, _rendered_outputs())


def test_final_name_substitution_after_open_is_detected_without_overwrite(
    tmp_path: Path,
) -> None:
    root = _external_test_root(tmp_path)
    substituted = b"substituted final-name sentinel\n"
    detached_name = ".detached-first-output"

    def substitute_first_final_name(directory_fd: int, name: str) -> None:
        if name != writer.OUTPUT_BASENAMES[0]:
            return
        os.rename(
            name,
            detached_name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
        )
        replacement_fd = os.open(
            name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600,
            dir_fd=directory_fd,
        )
        try:
            os.write(replacement_fd, substituted)
            os.fsync(replacement_fd)
        finally:
            os.close(replacement_fd)

    with pytest.raises(RuntimeError, match="identity changed"):
        writer.write_outputs(
            root,
            _rendered_outputs(),
            after_output_open=substitute_first_final_name,
        )
    musubi = root / "fixtures/musubi"
    assert (musubi / writer.OUTPUT_BASENAMES[0]).read_bytes() == substituted
    assert (musubi / detached_name).read_bytes() == _rendered_outputs()[0].contents
    assert not (musubi / writer.OUTPUT_BASENAMES[1]).exists()


@pytest.mark.parametrize("substitution_target", ["ancestor", "root"])
def test_path_substitution_cannot_redirect_descriptor_writes(
    tmp_path: Path,
    substitution_target: str,
) -> None:
    parent = tmp_path.resolve()
    authority = _private_directory(parent / "authority")
    root = _private_directory(authority / "stage")
    detached_authority = parent / "detached-authority"
    detached_root = authority / "detached-stage"
    replacement_sentinel = b"replacement root sentinel\n"

    def substitute_path() -> None:
        if substitution_target == "ancestor":
            authority.rename(detached_authority)
            replacement_authority = _private_directory(parent / "authority")
            replacement_root = _private_directory(replacement_authority / "stage")
        else:
            root.rename(detached_root)
            replacement_root = _private_directory(authority / "stage")
        fixtures = _private_directory(replacement_root / "fixtures")
        musubi = _private_directory(fixtures / "musubi")
        sentinel = musubi / writer.OUTPUT_BASENAMES[0]
        sentinel.write_bytes(replacement_sentinel)
        sentinel.chmod(0o600)

    with pytest.raises(RuntimeError, match="path identity changed"):
        writer.write_outputs(
            root,
            _rendered_outputs(),
            after_root_open=substitute_path,
        )

    original_root = (
        detached_authority / "stage"
        if substitution_target == "ancestor"
        else detached_root
    )
    assert not any(
        (original_root / output.relative_path).exists()
        for output in _rendered_outputs()
    )
    replacement_root = parent / "authority/stage"
    assert (replacement_root / writer.OUTPUTS[0]).read_bytes() == replacement_sentinel
    assert not (replacement_root / writer.OUTPUTS[1]).exists()


def test_writer_has_no_pathname_replacement_or_cleanup_calls() -> None:
    source = (SCRIPTS_ROOT / "write_musubi_fixtures.py").read_text(encoding="utf-8")
    for forbidden in (
        "os.rename(",
        "os.replace(",
        "os.unlink(",
        "os.remove(",
        "shutil.rmtree(",
    ):
        assert forbidden not in source


def test_checker_is_read_only_and_requires_two_identical_owner_passes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    envelope = _owner_envelope()
    calls = 0
    cargo_target = Path("/external/musubi-owner-target")

    def run_owner(received_target: Path) -> bytes:
        nonlocal calls
        assert received_target == cargo_target
        calls += 1
        return envelope

    monkeypatch.setattr(
        writer,
        "resolve_owner_cargo_target_dir",
        lambda: cargo_target,
    )
    monkeypatch.setattr(writer, "run_owner", run_owner)
    monkeypatch.setattr(writer, "read_closed_outputs", lambda _: _rendered_outputs())
    monkeypatch.setattr(
        writer,
        "write_outputs",
        lambda *_args, **_kwargs: pytest.fail("read-only checker attempted a write"),
    )
    checker.check()
    assert calls == 2


def test_checker_rejects_nondeterministic_owner_envelopes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    envelopes = iter([_owner_envelope(), _owner_envelope() + b" "])
    monkeypatch.setattr(
        writer,
        "resolve_owner_cargo_target_dir",
        lambda: Path("/external/musubi-owner-target"),
    )
    monkeypatch.setattr(writer, "run_owner", lambda _target: next(envelopes))
    with pytest.raises(RuntimeError, match="nondeterministic"):
        checker.check()


def test_two_pass_comparison_detects_byte_drift() -> None:
    expected = _rendered_outputs()
    drifted = (
        expected[0],
        writer.RenderedOutput(expected[1].relative_path, b'{"drift":true}\n'),
    )
    with pytest.raises(RuntimeError, match="fixture drift"):
        checker.compare_outputs(expected, drifted, description="test drift")
