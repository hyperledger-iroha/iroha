#!/usr/bin/env python3
"""Validate the repo-local Codex plugin manifests."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


PLUGIN_NAME = "iroha"
PLUGIN_ROOT = Path("plugins") / PLUGIN_NAME
PLUGIN_MANIFEST = PLUGIN_ROOT / ".codex-plugin" / "plugin.json"
MCP_MANIFEST = PLUGIN_ROOT / ".mcp.json"
MARKETPLACE_MANIFEST = Path(".agents") / "plugins" / "marketplace.json"
TAIRA_SKILL_ROOT = Path("skills") / "sora-taira-testnet"
TAIRA_SKILL_MANIFEST = TAIRA_SKILL_ROOT / "SKILL.md"
TAIRA_SKILL_AGENT_MANIFEST = TAIRA_SKILL_ROOT / "agents" / "openai.yaml"


def _load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return payload


def _require_string(
    payload: dict[str, Any], key: str, context: str, errors: list[str]
) -> str | None:
    value = payload.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{context} field `{key}` must be a non-empty string")
        return None
    return value


def _expect_file(
    root: Path, relative_path: str, context: str, errors: list[str]
) -> None:
    if not relative_path.startswith("./"):
        errors.append(f"{context} path `{relative_path}` must start with `./`")
        return
    target = root / relative_path[2:]
    if not target.exists():
        errors.append(f"{context} path `{relative_path}` does not exist at {target}")


def _parse_simple_frontmatter(path: Path) -> dict[str, str]:
    content = path.read_text(encoding="utf-8")
    if not content.startswith("---\n"):
        raise ValueError(f"{path} must start with YAML frontmatter")
    end = content.find("\n---\n", 4)
    if end == -1:
        raise ValueError(f"{path} must close YAML frontmatter with `---`")
    frontmatter = content[4:end].splitlines()
    payload: dict[str, str] = {}
    for line in frontmatter:
        if not line.strip():
            continue
        key, separator, raw_value = line.partition(":")
        if not separator:
            raise ValueError(f"{path} has invalid frontmatter line: {line!r}")
        value = raw_value.strip()
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        payload[key.strip()] = value
    return payload


def _parse_simple_openai_yaml(path: Path) -> dict[str, dict[str, str]]:
    payload: dict[str, dict[str, str]] = {}
    current_section: str | None = None
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        if not raw_line.strip():
            continue
        if not raw_line.startswith(" "):
            key, separator, _ = raw_line.partition(":")
            if not separator:
                raise ValueError(f"{path} has invalid top-level line: {raw_line!r}")
            current_section = key.strip()
            payload[current_section] = {}
            continue
        if current_section is None:
            raise ValueError(f"{path} contains indented content before a section")
        key, separator, raw_value = raw_line.strip().partition(":")
        if not separator:
            raise ValueError(f"{path} has invalid nested line: {raw_line!r}")
        value = raw_value.strip()
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        payload[current_section][key.strip()] = value
    return payload


def validate_plugin_manifest(repo_root: Path) -> list[str]:
    errors: list[str] = []
    manifest_path = repo_root / PLUGIN_MANIFEST
    if not manifest_path.exists():
        return [f"missing plugin manifest: {manifest_path}"]
    payload = _load_json(manifest_path)
    plugin_root = repo_root / PLUGIN_ROOT

    if payload.get("name") != PLUGIN_NAME:
        errors.append(f"plugin manifest name must be `{PLUGIN_NAME}`")

    _require_string(payload, "version", "plugin manifest", errors)
    _require_string(payload, "description", "plugin manifest", errors)
    if payload.get("homepage") != "https://docs.iroha.tech":
        errors.append("plugin manifest homepage must point to https://docs.iroha.tech")
    if payload.get("repository") != "https://github.com/hyperledger-iroha/iroha":
        errors.append(
            "plugin manifest repository must point to https://github.com/hyperledger-iroha/iroha"
        )
    if payload.get("license") != "Apache-2.0":
        errors.append("plugin manifest license must be Apache-2.0")
    if payload.get("skills") != "./skills/":
        errors.append("plugin manifest skills must be `./skills/`")
    if payload.get("mcpServers") != "./.mcp.json":
        errors.append("plugin manifest mcpServers must be `./.mcp.json`")

    author = payload.get("author")
    if not isinstance(author, dict):
        errors.append("plugin manifest author must be an object")
    else:
        _require_string(author, "name", "plugin author", errors)
        _require_string(author, "url", "plugin author", errors)

    interface = payload.get("interface")
    if not isinstance(interface, dict):
        errors.append("plugin manifest interface must be an object")
        return errors

    if interface.get("displayName") != "Iroha":
        errors.append("plugin displayName must be `Iroha`")
    if interface.get("category") != "Coding":
        errors.append("plugin category must be `Coding`")
    capabilities = interface.get("capabilities")
    if capabilities != ["Interactive", "Read", "Write"]:
        errors.append(
            "plugin capabilities must exactly match ['Interactive', 'Read', 'Write']"
        )
    prompts = interface.get("defaultPrompt")
    if not isinstance(prompts, list) or not (1 <= len(prompts) <= 3):
        errors.append("plugin defaultPrompt must contain between 1 and 3 entries")

    composer_icon = _require_string(interface, "composerIcon", "plugin interface", errors)
    logo = _require_string(interface, "logo", "plugin interface", errors)
    if composer_icon is not None:
        _expect_file(plugin_root, composer_icon, "composerIcon", errors)
    if logo is not None:
        _expect_file(plugin_root, logo, "logo", errors)

    screenshots = interface.get("screenshots")
    if not isinstance(screenshots, list):
        errors.append("plugin screenshots must be an array")
    else:
        for screenshot in screenshots:
            if not isinstance(screenshot, str):
                errors.append("plugin screenshot entries must be strings")
                continue
            _expect_file(plugin_root, screenshot, "screenshot", errors)

    return errors


def validate_mcp_manifest(repo_root: Path) -> list[str]:
    errors: list[str] = []
    manifest_path = repo_root / MCP_MANIFEST
    if not manifest_path.exists():
        return [f"missing MCP manifest: {manifest_path}"]

    payload = _load_json(manifest_path)
    servers = payload.get("mcpServers")
    if not isinstance(servers, dict):
        return ["MCP manifest must contain an `mcpServers` object"]

    taira = servers.get("iroha-taira")
    if not isinstance(taira, dict):
        return ["MCP manifest must contain the `iroha-taira` server entry"]

    if taira.get("type") != "http":
        errors.append("iroha-taira MCP server must use HTTP transport")
    if taira.get("url") != "https://taira.sora.org/v1/mcp":
        errors.append("iroha-taira MCP server must target https://taira.sora.org/v1/mcp")
    _require_string(taira, "note", "iroha-taira MCP server", errors)
    return errors


def validate_marketplace_manifest(repo_root: Path) -> list[str]:
    errors: list[str] = []
    manifest_path = repo_root / MARKETPLACE_MANIFEST
    if not manifest_path.exists():
        return [f"missing marketplace manifest: {manifest_path}"]

    payload = _load_json(manifest_path)
    if payload.get("name") != "iroha-local":
        errors.append("marketplace name must be `iroha-local`")

    interface = payload.get("interface")
    if not isinstance(interface, dict):
        errors.append("marketplace interface must be an object")
    elif interface.get("displayName") != "Iroha Local":
        errors.append("marketplace interface.displayName must be `Iroha Local`")

    plugins = payload.get("plugins")
    if not isinstance(plugins, list):
        return ["marketplace plugins field must be an array"]

    entry = next(
        (
            candidate
            for candidate in plugins
            if isinstance(candidate, dict) and candidate.get("name") == PLUGIN_NAME
        ),
        None,
    )
    if entry is None:
        return ["marketplace must contain an `iroha` plugin entry"]

    source = entry.get("source")
    if not isinstance(source, dict):
        errors.append("marketplace source must be an object")
    else:
        if source.get("source") != "local":
            errors.append("marketplace source.source must be `local`")
        if source.get("path") != "./plugins/iroha":
            errors.append("marketplace source.path must be `./plugins/iroha`")

    policy = entry.get("policy")
    if not isinstance(policy, dict):
        errors.append("marketplace policy must be an object")
    else:
        if policy.get("installation") != "AVAILABLE":
            errors.append("marketplace installation policy must be AVAILABLE")
        if policy.get("authentication") != "ON_INSTALL":
            errors.append("marketplace authentication policy must be ON_INSTALL")

    if entry.get("category") != "Coding":
        errors.append("marketplace category must be `Coding`")
    return errors


def validate_taira_skill(repo_root: Path) -> list[str]:
    errors: list[str] = []
    skill_path = repo_root / TAIRA_SKILL_MANIFEST
    agent_path = repo_root / TAIRA_SKILL_AGENT_MANIFEST

    if not skill_path.exists():
        return [f"missing standalone Taira skill: {skill_path}"]
    if not agent_path.exists():
        return [f"missing standalone Taira agent metadata: {agent_path}"]

    try:
        frontmatter = _parse_simple_frontmatter(skill_path)
    except ValueError as error:
        return [str(error)]

    if frontmatter.get("name") != "sora-taira-testnet":
        errors.append("standalone Taira skill name must be `sora-taira-testnet`")
    description = frontmatter.get("description", "")
    if not description:
        errors.append("standalone Taira skill must include a non-empty description")
    elif "https://taira.sora.org/v1/mcp" not in description:
        errors.append("standalone Taira skill description must mention the Taira MCP URL")

    skill_text = skill_path.read_text(encoding="utf-8")
    for required in (
        "https://taira.sora.org/v1/mcp",
        "iroha.transactions.submit_and_wait",
        "authority",
        "private_key",
    ):
        if required not in skill_text:
            errors.append(f"standalone Taira skill body must mention `{required}`")

    try:
        agent_payload = _parse_simple_openai_yaml(agent_path)
    except ValueError as error:
        return [str(error)]

    interface = agent_payload.get("interface")
    if interface is None:
        return ["standalone Taira agent metadata must contain an `interface` section"]
    if interface.get("display_name") != "SORA Taira Testnet":
        errors.append("standalone Taira skill display_name must be `SORA Taira Testnet`")
    short_description = interface.get("short_description", "")
    if not (25 <= len(short_description) <= 64):
        errors.append("standalone Taira skill short_description must be 25-64 characters")
    default_prompt = interface.get("default_prompt", "")
    if "$sora-taira-testnet" not in default_prompt:
        errors.append("standalone Taira skill default_prompt must mention `$sora-taira-testnet`")
    return errors


def validate_repo(repo_root: Path) -> list[str]:
    errors: list[str] = []
    errors.extend(validate_plugin_manifest(repo_root))
    errors.extend(validate_mcp_manifest(repo_root))
    errors.extend(validate_marketplace_manifest(repo_root))
    errors.extend(validate_taira_skill(repo_root))
    return errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate the repo-local Codex plugin manifests")
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root containing plugins/ and .agents/",
    )
    parser.add_argument("--quiet", action="store_true", help="Only print failures")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo_root = Path(args.repo_root).resolve()
    errors = validate_repo(repo_root)
    if errors:
        if not args.quiet:
            print("Codex plugin manifest validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    if not args.quiet:
        print("Codex plugin manifests are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
