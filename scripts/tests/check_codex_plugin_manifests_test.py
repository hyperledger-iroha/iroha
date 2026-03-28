"""Tests for scripts/check_codex_plugin_manifests.py."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_codex_plugin_manifests.py"
SPEC = importlib.util.spec_from_file_location("check_codex_plugin_manifests", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
SPEC.loader.exec_module(MODULE)


def _write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")


def _write_valid_repo(root: Path) -> None:
    _write_json(
        root / "plugins" / "iroha" / ".codex-plugin" / "plugin.json",
        {
            "name": "iroha",
            "version": "0.1.0",
            "description": "desc",
            "author": {
                "name": "Hyperledger Iroha contributors",
                "url": "https://github.com/hyperledger-iroha/iroha/graphs/contributors",
            },
            "homepage": "https://docs.iroha.tech",
            "repository": "https://github.com/hyperledger-iroha/iroha",
            "license": "Apache-2.0",
            "skills": "./skills/",
            "mcpServers": "./.mcp.json",
            "interface": {
                "displayName": "Iroha",
                "category": "Coding",
                "capabilities": ["Interactive", "Read", "Write"],
                "defaultPrompt": ["one", "two", "three"],
                "composerIcon": "./assets/iroha-small.svg",
                "logo": "./assets/iroha-logo.svg",
                "screenshots": [],
            },
        },
    )
    _write_json(
        root / "plugins" / "iroha" / ".mcp.json",
        {
            "mcpServers": {
                "iroha-taira": {
                    "type": "http",
                    "url": "https://taira.sora.org/v1/mcp",
                    "note": "preset",
                }
            }
        },
    )
    _write_json(
        root / ".agents" / "plugins" / "marketplace.json",
        {
            "name": "iroha-local",
            "interface": {"displayName": "Iroha Local"},
            "plugins": [
                {
                    "name": "iroha",
                    "source": {"source": "local", "path": "./plugins/iroha"},
                    "policy": {
                        "installation": "AVAILABLE",
                        "authentication": "ON_INSTALL",
                    },
                    "category": "Coding",
                }
            ],
        },
    )
    (root / "plugins" / "iroha" / "assets").mkdir(parents=True, exist_ok=True)
    (root / "plugins" / "iroha" / "skills").mkdir(parents=True, exist_ok=True)
    (root / "plugins" / "iroha" / "assets" / "iroha-small.svg").write_text(
        "<svg/>", encoding="utf-8"
    )
    (root / "plugins" / "iroha" / "assets" / "iroha-logo.svg").write_text(
        "<svg/>", encoding="utf-8"
    )


def test_validate_repo_accepts_expected_layout(tmp_path: Path) -> None:
    _write_valid_repo(tmp_path)

    assert MODULE.validate_repo(tmp_path) == []
    assert MODULE.main(["--repo-root", str(tmp_path), "--quiet"]) == 0


def test_validate_repo_reports_missing_taira_entry(tmp_path: Path) -> None:
    _write_valid_repo(tmp_path)
    _write_json(tmp_path / "plugins" / "iroha" / ".mcp.json", {"mcpServers": {}})

    errors = MODULE.validate_repo(tmp_path)
    assert any("iroha-taira" in error for error in errors)
    assert MODULE.main(["--repo-root", str(tmp_path), "--quiet"]) == 1
