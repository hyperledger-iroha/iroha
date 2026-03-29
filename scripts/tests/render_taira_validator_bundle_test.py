"""Tests for scripts/render_taira_validator_bundle.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "render_taira_validator_bundle.py"
SPEC = importlib.util.spec_from_file_location("render_taira_validator_bundle", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


BASE_CONFIG = """# baseline
public_key = "peer-1-public"
private_key = "peer-1-private"

trusted_peers = [
  "peer-1-public@taira-validator-1.sora.org:1337",
]
trusted_peers_pop = [
  { public_key = "peer-1-public", pop_hex = "peer-1-pop" },
]

[network]
address = "0.0.0.0:1337"
public_address = "taira-validator-1.sora.org:1337"

[torii]
address = "0.0.0.0:18080"
public_address = "https://taira.sora.org"

[torii.mcp]
enabled = true
"""


def _write_roster(path: Path, validator_count: int = 4, inline_private_keys: bool = True) -> None:
    validators = []
    for index in range(1, validator_count + 1):
        entry = [
            "[[validators]]",
            f'slug = "taira-validator-{index}"',
            f'public_key = "peer-{index}-public"',
        ]
        if inline_private_keys:
            entry.append(f'private_key = "peer-{index}-private"')
        entry.extend(
            [
                f'pop_hex = "peer-{index}-pop"',
                f'public_address = "taira-validator-{index}.sora.org:1337"',
                "",
            ]
        )
        validators.extend(entry)
    path.write_text(
        "\n".join(
            [
                'torii_public_address = "https://taira.sora.org"',
                "",
                *validators,
            ]
        ),
        encoding="utf-8",
    )


def _write_secrets(path: Path, validator_count: int = 4) -> None:
    validators = []
    for index in range(1, validator_count + 1):
        validators.extend(
            [
                "[[validators]]",
                f'slug = "taira-validator-{index}"',
                f'private_key = "peer-{index}-private"',
                "",
            ]
        )
    path.write_text("\n".join(validators), encoding="utf-8")


def test_render_bundle_rewrites_peer_specific_sections(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    written = MODULE.render_bundle(base_config_path, roster_path, output_dir)

    assert len(written) == 4
    config = (output_dir / "taira-validator-3" / "config.toml").read_text(
        encoding="utf-8"
    )
    assert 'public_key = "peer-3-public"' in config
    assert 'private_key = "peer-3-private"' in config
    assert (
        'public_address = "taira-validator-3.sora.org:1337"' in config
    )
    assert '"peer-4-public@taira-validator-4.sora.org:1337"' in config
    assert '{ public_key = "peer-2-public", pop_hex = "peer-2-pop" }' in config


def test_load_roster_requires_four_validators(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, validator_count=3)

    try:
        MODULE.load_roster(roster_path)
    except ValueError as error:
        assert "at least 4 validators" in str(error)
    else:  # pragma: no cover - defensive assertion
        raise AssertionError("load_roster accepted a too-small validator roster")


def test_load_roster_merges_private_keys_from_secrets(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)

    validators = MODULE.load_roster(roster_path, secrets_path=secrets_path)

    assert validators[0].private_key == "peer-1-private"
    assert validators[-1].private_key == "peer-4-private"


def test_main_supports_single_validator_render(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    secrets_path = tmp_path / "validator_secrets.toml"
    base_config_path = tmp_path / "config.toml"
    output_dir = tmp_path / "out"
    _write_roster(roster_path, inline_private_keys=False)
    _write_secrets(secrets_path)
    base_config_path.write_text(BASE_CONFIG, encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--base-config",
            str(base_config_path),
            "--roster",
            str(roster_path),
            "--secrets",
            str(secrets_path),
            "--output-dir",
            str(output_dir),
            "--only",
            "taira-validator-2",
        ]
    )

    assert exit_code == 0
    assert (output_dir / "taira-validator-2" / "config.toml").exists()
    assert not (output_dir / "taira-validator-1" / "config.toml").exists()
