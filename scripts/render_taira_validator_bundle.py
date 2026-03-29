#!/usr/bin/env python3
"""Render per-validator Taira config bundles from a single roster file."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_NETWORK_ADDRESS = "0.0.0.0:1337"
DEFAULT_TORII_ADDRESS = "0.0.0.0:18080"
DEFAULT_TORII_PUBLIC_ADDRESS = "https://taira.sora.org"
MIN_VALIDATORS = 4


@dataclass(frozen=True)
class ValidatorEntry:
    """Validator-specific material for a rendered Taira config."""

    slug: str
    public_key: str
    private_key: str
    pop_hex: str
    public_address: str
    network_address: str
    torii_address: str
    torii_public_address: str


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        import tomllib
    except ModuleNotFoundError:
        try:
            import tomli as tomllib
        except ModuleNotFoundError as error:  # pragma: no cover - environment specific
            raise SystemExit(
                "python3 must provide tomllib (Python 3.11+) or tomli to load roster TOML"
            ) from error

    with path.open("rb") as handle:
        payload = tomllib.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a top-level TOML table")
    return payload


def _require_string(payload: dict[str, Any], key: str, context: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} field `{key}` must be a non-empty string")
    return value.strip()


def _quote_toml(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def _render_trusted_peers(validators: list[ValidatorEntry]) -> list[str]:
    lines = ["trusted_peers = ["]
    for validator in validators:
        lines.append(
            f"  {_quote_toml(f'{validator.public_key}@{validator.public_address}')},"
        )
    lines.append("]")
    return lines


def _render_trusted_peers_pop(validators: list[ValidatorEntry]) -> list[str]:
    lines = ["trusted_peers_pop = ["]
    for validator in validators:
        lines.append(
            "  { public_key = "
            f"{_quote_toml(validator.public_key)}, "
            f"pop_hex = {_quote_toml(validator.pop_hex)} }},"
        )
    lines.append("]")
    return lines


def load_roster(path: Path) -> list[ValidatorEntry]:
    """Load and validate a Taira validator roster TOML file."""

    payload = _load_toml(path)
    defaults = {
        "network_address": payload.get("network_address", DEFAULT_NETWORK_ADDRESS),
        "torii_address": payload.get("torii_address", DEFAULT_TORII_ADDRESS),
        "torii_public_address": payload.get(
            "torii_public_address", DEFAULT_TORII_PUBLIC_ADDRESS
        ),
    }
    for key, value in defaults.items():
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"roster default `{key}` must be a non-empty string")

    validators_raw = payload.get("validators")
    if not isinstance(validators_raw, list):
        raise ValueError("roster must define a `validators` array of tables")
    if len(validators_raw) < MIN_VALIDATORS:
        raise ValueError(
            f"roster must define at least {MIN_VALIDATORS} validators for Taira"
        )

    validators: list[ValidatorEntry] = []
    seen_slugs: set[str] = set()
    seen_public_keys: set[str] = set()
    seen_public_addresses: set[str] = set()
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"validator entry #{index} must be a TOML table")
        slug = _require_string(raw, "slug", f"validator `{index}`")
        public_key = _require_string(raw, "public_key", f"validator `{slug}`")
        private_key = _require_string(raw, "private_key", f"validator `{slug}`")
        pop_hex = _require_string(raw, "pop_hex", f"validator `{slug}`")
        public_address = _require_string(raw, "public_address", f"validator `{slug}`")
        network_address = raw.get("network_address", defaults["network_address"])
        torii_address = raw.get("torii_address", defaults["torii_address"])
        torii_public_address = raw.get(
            "torii_public_address", defaults["torii_public_address"]
        )
        if not isinstance(network_address, str) or not network_address.strip():
            raise ValueError(f"validator `{slug}` field `network_address` is invalid")
        if not isinstance(torii_address, str) or not torii_address.strip():
            raise ValueError(f"validator `{slug}` field `torii_address` is invalid")
        if not isinstance(torii_public_address, str) or not torii_public_address.strip():
            raise ValueError(
                f"validator `{slug}` field `torii_public_address` is invalid"
            )
        if slug in seen_slugs:
            raise ValueError(f"validator slug `{slug}` is duplicated")
        if public_key in seen_public_keys:
            raise ValueError(f"validator public_key `{public_key}` is duplicated")
        if public_address in seen_public_addresses:
            raise ValueError(f"validator public_address `{public_address}` is duplicated")
        seen_slugs.add(slug)
        seen_public_keys.add(public_key)
        seen_public_addresses.add(public_address)
        validators.append(
            ValidatorEntry(
                slug=slug,
                public_key=public_key,
                private_key=private_key,
                pop_hex=pop_hex,
                public_address=public_address,
                network_address=network_address.strip(),
                torii_address=torii_address.strip(),
                torii_public_address=torii_public_address.strip(),
            )
        )

    return validators


def render_validator_config(
    template_text: str, validator: ValidatorEntry, validators: list[ValidatorEntry]
) -> str:
    """Rewrite the checked-in peer-1 baseline for one validator."""

    current_section: str | None = None
    skipping_array: str | None = None
    rendered: list[str] = []
    trusted_peers_lines = _render_trusted_peers(validators)
    trusted_peers_pop_lines = _render_trusted_peers_pop(validators)

    for raw_line in template_text.splitlines():
        stripped = raw_line.strip()

        if skipping_array is not None:
            if stripped == "]":
                skipping_array = None
            continue

        if stripped.startswith("[[") or stripped.startswith("["):
            current_section = stripped
            rendered.append(raw_line)
            continue

        if current_section is None and stripped.startswith("public_key = "):
            rendered.append(f"public_key = {_quote_toml(validator.public_key)}")
            continue
        if current_section is None and stripped.startswith("private_key = "):
            rendered.append(f"private_key = {_quote_toml(validator.private_key)}")
            continue
        if current_section is None and stripped == "trusted_peers = [":
            rendered.extend(trusted_peers_lines)
            skipping_array = "trusted_peers"
            continue
        if current_section is None and stripped == "trusted_peers_pop = [":
            rendered.extend(trusted_peers_pop_lines)
            skipping_array = "trusted_peers_pop"
            continue

        if current_section == "[network]" and stripped.startswith("address = "):
            rendered.append(f'address = {_quote_toml(validator.network_address)}')
            continue
        if current_section == "[network]" and stripped.startswith("public_address = "):
            rendered.append(f'public_address = {_quote_toml(validator.public_address)}')
            continue
        if current_section == "[torii]" and stripped.startswith("address = "):
            rendered.append(f'address = {_quote_toml(validator.torii_address)}')
            continue
        if current_section == "[torii]" and stripped.startswith("public_address = "):
            rendered.append(
                f'public_address = {_quote_toml(validator.torii_public_address)}'
            )
            continue

        rendered.append(raw_line)

    rendered_text = "\n".join(rendered)
    if not rendered_text.endswith("\n"):
        rendered_text += "\n"
    return rendered_text


def render_bundle(
    base_config_path: Path,
    roster_path: Path,
    output_dir: Path,
    only: str | None = None,
) -> list[Path]:
    """Render one config.toml per validator into output_dir."""

    validators = load_roster(roster_path)
    template_text = base_config_path.read_text(encoding="utf-8")
    output_dir.mkdir(parents=True, exist_ok=True)

    written: list[Path] = []
    for validator in validators:
        if only is not None and validator.slug != only:
            continue
        target_dir = output_dir / validator.slug
        target_dir.mkdir(parents=True, exist_ok=True)
        target_path = target_dir / "config.toml"
        target_path.write_text(
            render_validator_config(template_text, validator, validators),
            encoding="utf-8",
        )
        written.append(target_path)

    if only is not None and not written:
        raise ValueError(f"validator `{only}` is not present in {roster_path}")
    return written


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint."""

    parser = argparse.ArgumentParser(
        description="Render per-validator Taira config.toml files from a roster."
    )
    parser.add_argument(
        "--base-config",
        default="configs/soranexus/taira/config.toml",
        help="checked-in peer-1 baseline config to rewrite",
    )
    parser.add_argument(
        "--roster",
        required=True,
        help="TOML roster with validator public addresses, keys, and PoPs",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="directory where <validator-slug>/config.toml files will be written",
    )
    parser.add_argument(
        "--only",
        help="render only one validator slug instead of the full bundle",
    )
    args = parser.parse_args(argv)

    written = render_bundle(
        Path(args.base_config),
        Path(args.roster),
        Path(args.output_dir),
        only=args.only,
    )
    for path in written:
        print(path)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
