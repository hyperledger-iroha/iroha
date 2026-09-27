"""Fail-closed regressions for generated repository consistency artifacts."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import stat
import subprocess

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
CONSISTENCY_SCRIPT = REPO_ROOT / "scripts" / "tests" / "consistency.sh"


def _fixture(
    tmp_path: Path,
    generator: str,
    *,
    target: bytes = b'{"retained":"schema"}\n',
) -> tuple[Path, Path]:
    script = tmp_path / "scripts" / "tests" / "consistency.sh"
    script.parent.mkdir(parents=True)
    shutil.copy2(CONSISTENCY_SCRIPT, script)

    schema = tmp_path / "specs" / "references" / "schema.json"
    schema.parent.mkdir(parents=True)
    schema.write_bytes(target)
    (schema.parent / "genesis_schema.json").write_bytes(b'{"retained":"genesis"}\n')

    fake = tmp_path / "kagami-generator"
    fake.write_text(generator, encoding="utf-8")
    fake.chmod(0o755)
    return schema, fake


def _run(tmp_path: Path, fake: Path, *args: str) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env["BIN_KAGAMI"] = str(fake)
    return subprocess.run(
        ["bash", "scripts/tests/consistency.sh", *args],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def test_schema_uses_active_advanced_command() -> None:
    source = CONSISTENCY_SCRIPT.read_text(encoding="utf-8")
    hook = (REPO_ROOT / "hooks" / "pre-commit.sample").read_text(encoding="utf-8")

    assert 'cmd_schema="${bin_kagami[@]} advanced schema"' in source
    assert 'cmd_schema="${bin_kagami[@]} schema"' not in source
    assert 'check_schema' in source
    assert 'cmd_schema --genesis-out' in source
    assert 'staged_genesis="$(mktemp "specs/references/.genesis_schema.json.XXXXXX")"' in source
    assert "docs/source/references/schema.json" not in source
    assert "bash scripts/tests/consistency.sh --update schema" in hook.splitlines()
    assert "cargo run --bin kagami -- schema" not in hook
    assert "> ./specs/references/schema.json" not in hook
    assert "genesis_schema.json" not in hook


@pytest.mark.parametrize(
    ("payload", "expected_ok"),
    (
        ('{"consensus_fingerprint":null,"transactions":[]}\n', True),
        (
            '{"consensus_fingerprint":null,"kagemusha_mint_finality":{},"transactions":[]}\n',
            False,
        ),
    ),
)
def test_genesis_template_check_preserves_the_incomplete_boundary(
    tmp_path: Path, payload: str, expected_ok: bool
) -> None:
    _, fake = _fixture(tmp_path, "#!/bin/sh\nexit 99\n")
    template = tmp_path / "defaults" / "genesis.template.json"
    template.parent.mkdir()
    template.write_text(payload, encoding="utf-8")

    result = _run(tmp_path, fake, "genesis-template")

    assert (result.returncode == 0) is expected_ok
    assert template.read_text(encoding="utf-8") == payload


def test_failed_generator_cannot_truncate_checked_in_schema(tmp_path: Path) -> None:
    original = b'{"retained":"schema"}\n'
    schema, fake = _fixture(tmp_path, "#!/bin/sh\nexit 23\n", target=original)

    result = _run(tmp_path, fake, "--update", "schema")

    assert result.returncode != 0
    assert "generator command failed" in result.stdout
    assert schema.read_bytes() == original
    assert (schema.parent / "genesis_schema.json").read_bytes() == b'{"retained":"genesis"}\n'
    assert not list(schema.parent.glob(".schema.json.*"))
    assert not list(schema.parent.glob(".genesis_schema.json.*"))


def test_empty_generator_cannot_replace_checked_in_schema(tmp_path: Path) -> None:
    original = b'{"retained":"schema"}\n'
    schema, fake = _fixture(tmp_path, "#!/bin/sh\nexit 0\n", target=original)

    result = _run(tmp_path, fake, "--update", "schema")

    assert result.returncode != 0
    assert "generator produced an empty output" in result.stdout
    assert schema.read_bytes() == original
    assert (schema.parent / "genesis_schema.json").read_bytes() == b'{"retained":"genesis"}\n'
    assert not list(schema.parent.glob(".schema.json.*"))
    assert not list(schema.parent.glob(".genesis_schema.json.*"))


def test_missing_genesis_output_cannot_replace_checked_in_schema(tmp_path: Path) -> None:
    schema, fake = _fixture(
        tmp_path,
        "#!/bin/sh\nprintf '%s\\n' '{\"generated\":\"schema\"}'\n",
    )
    genesis = schema.parent / "genesis_schema.json"

    result = _run(tmp_path, fake, "--update", "schema")

    assert result.returncode != 0
    assert "generator produced an empty output" in result.stdout
    assert schema.read_bytes() == b'{"retained":"schema"}\n'
    assert genesis.read_bytes() == b'{"retained":"genesis"}\n'
    assert not list(schema.parent.glob(".schema.json.*"))
    assert not list(schema.parent.glob(".genesis_schema.json.*"))


def test_successful_update_replaces_both_schemas_with_public_mode(
    tmp_path: Path,
) -> None:
    schema, fake = _fixture(
        tmp_path,
        (
            "#!/bin/sh\n"
            'test "$1" = advanced\n'
            'test "$2" = schema\n'
            'test "$3" = --genesis-out\n'
            "printf '%s\\n' '{\"generated\":\"genesis\"}' > \"$4\"\n"
            "printf '%s\\n' '{\"generated\":\"schema\"}'\n"
        ),
    )

    result = _run(tmp_path, fake, "--update", "schema")

    assert result.returncode == 0, result.stdout + result.stderr
    assert schema.read_bytes() == b'{"generated":"schema"}\n'
    genesis = schema.parent / "genesis_schema.json"
    assert genesis.read_bytes() == b'{"generated":"genesis"}\n'
    assert stat.S_IMODE(schema.stat().st_mode) == 0o644
    assert stat.S_IMODE(genesis.stat().st_mode) == 0o644
    assert not list(schema.parent.glob(".schema.json.*"))
    assert not list(schema.parent.glob(".genesis_schema.json.*"))


def test_schema_check_reports_stale_genesis_without_changing_either_file(tmp_path: Path) -> None:
    schema, fake = _fixture(
        tmp_path,
        '#!/bin/sh\nprintf \'%s\\n\' \'{"generated":"genesis"}\' > "$4"\n'
        "printf '%s\\n' '{\"retained\":\"schema\"}'\n",
    )
    genesis = schema.parent / "genesis_schema.json"

    result = _run(tmp_path, fake, "schema")

    assert result.returncode != 0
    assert "[OK] specs/references/schema.json" in result.stdout
    assert "[DIFF] specs/references/genesis_schema.json" in result.stdout
    assert schema.read_bytes() == b'{"retained":"schema"}\n'
    assert genesis.read_bytes() == b'{"retained":"genesis"}\n'
