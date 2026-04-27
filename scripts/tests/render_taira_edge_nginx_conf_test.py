"""Tests for scripts/render_taira_edge_nginx_conf.py."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "render_taira_edge_nginx_conf.py"
SPEC = importlib.util.spec_from_file_location("render_taira_edge_nginx_conf", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
REPO_ROOT = MODULE_PATH.parents[1]
EXAMPLE_ROSTER_PATH = REPO_ROOT / "configs/soranexus/taira/validator_roster.example.toml"
CHECKED_IN_EXAMPLE_PATH = REPO_ROOT / "configs/soranexus/taira/taira-explorer.nginx.conf"


def _write_roster(path: Path, *, torii_address: str = "0.0.0.0:18080", include_edge_upstreams: bool = True) -> None:
    parts = [f'torii_address = "{torii_address}"', ""]
    for index in range(1, 5):
        parts.extend(
            [
                "[[validators]]",
                f'slug = "taira-validator-{index}"',
                f'public_key = "peer-{index}-public"',
                f'pop_hex = "peer-{index}-pop"',
                f'public_address = "taira-validator-{index}.sora.org:1337"',
                f'torii_public_address = "https://taira-validator-{index}.sora.org"',
            ]
        )
        if include_edge_upstreams:
            parts.append(f'edge_torii_upstream = "127.0.0.1:{18079 + index}"')
        parts.append("")
    path.write_text("\n".join(parts), encoding="utf-8")


def test_load_edge_validators_uses_explicit_edge_upstreams(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)

    validators = MODULE.load_edge_validators(roster_path)

    assert [validator.upstream_address for validator in validators] == [
        "127.0.0.1:18080",
        "127.0.0.1:18081",
        "127.0.0.1:18082",
        "127.0.0.1:18083",
    ]
    assert validators[0].validator_host == "taira-validator-1.sora.org"


def test_load_edge_validators_falls_back_to_torii_address(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, torii_address="0.0.0.0:29080", include_edge_upstreams=False)

    try:
        MODULE.load_edge_validators(roster_path)
    except ValueError as error:
        assert "duplicated" in str(error)
    else:  # pragma: no cover
        raise AssertionError("load_edge_validators accepted duplicated fallback upstreams")

    roster_path.write_text(
        "\n".join(
            [
                'torii_address = "0.0.0.0:29080"',
                "",
                '[[validators]]',
                'slug = "taira-validator-1"',
                'public_key = "peer-1-public"',
                'pop_hex = "peer-1-pop"',
                'public_address = "taira-validator-1.sora.org:1337"',
                'torii_public_address = "https://taira-validator-1.sora.org"',
                'torii_address = "0.0.0.0:29080"',
                "",
                '[[validators]]',
                'slug = "taira-validator-2"',
                'public_key = "peer-2-public"',
                'pop_hex = "peer-2-pop"',
                'public_address = "taira-validator-2.sora.org:1337"',
                'torii_public_address = "https://taira-validator-2.sora.org"',
                'torii_address = "0.0.0.0:29081"',
                "",
                '[[validators]]',
                'slug = "taira-validator-3"',
                'public_key = "peer-3-public"',
                'pop_hex = "peer-3-pop"',
                'public_address = "taira-validator-3.sora.org:1337"',
                'torii_public_address = "https://taira-validator-3.sora.org"',
                'torii_address = "0.0.0.0:29082"',
                "",
                '[[validators]]',
                'slug = "taira-validator-4"',
                'public_key = "peer-4-public"',
                'pop_hex = "peer-4-pop"',
                'public_address = "taira-validator-4.sora.org:1337"',
                'torii_public_address = "https://taira-validator-4.sora.org"',
                'torii_address = "0.0.0.0:29083"',
                "",
            ]
        ),
        encoding="utf-8",
    )

    validators = MODULE.load_edge_validators(roster_path)
    assert validators[0].upstream_address == "127.0.0.1:29080"
    assert validators[-1].upstream_address == "127.0.0.1:29083"


def test_render_edge_nginx_conf_includes_all_public_routes() -> None:
    validators = [
        MODULE.EdgeValidator(
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    rendered = MODULE.render_edge_nginx_conf(validators)

    assert "server_name taira.sora.org taira-explorer.sora.org" in rendered
    assert "server_name *.sorafs.taira.sora.org;" in rendered
    assert "map $host $taira_mon_alias_host" in rendered
    assert "server_name mon.taira.sora.net;" in rendered
    assert "Taira Soracloud Mon gateway" in rendered
    assert "https://mon.taira.sora.net/soradns/<alias>/<path>" in rendered
    assert "server_name *.mon.taira.sora.net ~^.+\\.mon\\.taira\\.sora\\.net$;" in rendered
    assert "proxy_set_header Host $taira_mon_alias_host;" in rendered
    assert "proxy_set_header X-Forwarded-Host $host;" in rendered
    assert "proxy_pass http://taira_public_edge_upstream;" in rendered
    assert "proxy_pass http://taira_public_edge_upstream$soradns_target_path$is_args$args;" in rendered
    assert "proxy_pass http://taira_validator_1_upstream;" in rendered
    assert "location = /v1/mcp" in rendered
    assert "location ^~ /v1/app-api/" in rendered
    assert "client_max_body_size 1g;" in rendered


def test_main_writes_rendered_conf(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_path = tmp_path / "taira.sora.org.conf"
    _write_roster(roster_path)

    exit_code = MODULE.main(["--roster", str(roster_path), "--output", str(output_path)])

    assert exit_code == 0
    rendered = output_path.read_text(encoding="utf-8")
    assert "Generated by scripts/render_taira_edge_nginx_conf.py" in rendered
    assert "server 127.0.0.1:18080;" in rendered


def test_checked_in_example_matches_rendered_example_roster() -> None:
    validators = MODULE.load_edge_validators(EXAMPLE_ROSTER_PATH)
    rendered = MODULE.render_edge_nginx_conf(validators)
    checked_in = CHECKED_IN_EXAMPLE_PATH.read_text(encoding="utf-8")

    assert checked_in.rstrip("\n") == rendered.rstrip("\n")
