#!/usr/bin/env python3
"""Render per-validator Taira config bundles from Taira roster material."""

from __future__ import annotations

import argparse
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_NETWORK_ADDRESS = "0.0.0.0:1337"
DEFAULT_TORII_ADDRESS = "0.0.0.0:18080"
MIN_VALIDATORS = 4
# Mirrors `iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT`.
MAX_VALIDATORS = 128


@dataclass(frozen=True)
class ValidatorEntry:
    """Validator-specific material for a rendered Taira config."""

    slug: str
    account_id: str
    public_key: str
    private_key: str
    pop_hex: str
    public_address: str
    network_address: str
    torii_address: str
    torii_public_address: str


@dataclass(frozen=True)
class RosterDefaults:
    """Shared defaults applied to validator entries."""

    network_address: str
    torii_address: str
    torii_public_address: str | None


@dataclass(frozen=True)
class SharedSecrets:
    """Runtime-only shared secret material injected into rendered configs."""

    account_onboarding_authority: str | None = None
    account_onboarding_private_key: str | None = None
    account_onboarding_api_token: str | None = None
    account_onboarding_credential_id: str | None = None
    account_onboarding_scope_domain: str | None = None
    account_onboarding_scope_dataspace: str | None = None
    torii_faucet_authority: str | None = None
    torii_faucet_private_key: str | None = None
    streaming_identity_public_key: str | None = None
    streaming_identity_private_key: str | None = None
    sorafs_council_public_keys: tuple[str, ...] = ()
    sorafs_council_signature_threshold: int | None = None


@dataclass(frozen=True)
class SecretMaterial:
    """User-local validator and shared secrets used during rendering."""

    validators: dict[str, str]
    shared: SharedSecrets


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


def _require_positive_integer(
    payload: dict[str, Any], key: str, context: str
) -> int:
    value = payload.get(key)
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ValueError(f"{context} field `{key}` must be a positive integer")
    return value


def _scaled_sumeragi_body_bytes(
    template: dict[str, Any], validator_count: int
) -> int:
    """Return an aggregate ingress budget isolating every configured source."""

    sumeragi = template.get("sumeragi")
    if not isinstance(sumeragi, dict):
        raise ValueError("config template must define a `[sumeragi]` table")
    queues = sumeragi.get("queues")
    if not isinstance(queues, dict):
        raise ValueError("config template must define a `[sumeragi.queues]` table")
    context = "config template `[sumeragi.queues]`"
    configured = _require_positive_integer(queues, "body_bytes", context)
    source_bytes = _require_positive_integer(queues, "body_source_bytes", context)
    authenticated_non_validator_sources = _require_positive_integer(
        queues, "authenticated_non_validator_sources", context
    )
    minimum = (
        validator_count + authenticated_non_validator_sources + 1
    ) * source_bytes
    return max(configured, minimum)


def _quote_toml(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def _blake3_token_hash(token: str) -> str:
    """Return the canonical digest stored in account-onboarding config."""

    try:
        import blake3
    except ModuleNotFoundError as error:  # pragma: no cover - environment specific
        raise SystemExit(
            "install scripts/requirements.txt before rendering Taira bundles"
        ) from error
    return f"blake3:{blake3.blake3(token.encode('utf-8')).hexdigest()}"


def _write_private_text(path: Path, value: str) -> None:
    """Create or replace one runtime-only sidecar without a permissive mode window."""

    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            descriptor = -1
            handle.write(value)
            if not value.endswith("\n"):
                handle.write("\n")
    finally:
        if descriptor >= 0:  # pragma: no cover - defensive cleanup
            os.close(descriptor)
    path.chmod(0o600)


def _validate_account_onboarding_secrets(
    shared: SharedSecrets, context: str
) -> None:
    fields = {
        "account_onboarding_authority": shared.account_onboarding_authority,
        "account_onboarding_private_key": shared.account_onboarding_private_key,
        "account_onboarding_api_token": shared.account_onboarding_api_token,
        "account_onboarding_credential_id": shared.account_onboarding_credential_id,
    }
    scopes = [
        shared.account_onboarding_scope_domain,
        shared.account_onboarding_scope_dataspace,
    ]
    if any(value is not None for value in (*fields.values(), *scopes)):
        missing = [key for key, value in fields.items() if value is None]
        if missing:
            raise ValueError(
                f"{context} account onboarding is incomplete; missing "
                + ", ".join(missing)
            )
        if sum(value is not None for value in scopes) != 1:
            raise ValueError(
                f"{context} account onboarding must set exactly one of "
                "account_onboarding_scope_domain or account_onboarding_scope_dataspace"
            )


def _load_validator_tables(payload: dict[str, Any], context: str) -> list[dict[str, Any]]:
    validators_raw = payload.get("validators")
    if not isinstance(validators_raw, list):
        raise ValueError(f"{context} must define a `validators` array of tables")
    validators: list[dict[str, Any]] = []
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"{context} validator entry #{index} must be a TOML table")
        validators.append(raw)
    return validators


def _load_optional_validator_tables(
    payload: dict[str, Any], context: str
) -> list[dict[str, Any]]:
    validators_raw = payload.get("validators")
    if validators_raw is None:
        return []
    if not isinstance(validators_raw, list):
        raise ValueError(f"{context} must define a `validators` array of tables")
    validators: list[dict[str, Any]] = []
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"{context} validator entry #{index} must be a TOML table")
        validators.append(raw)
    return validators


def _load_defaults(payload: dict[str, Any]) -> RosterDefaults:
    values = {
        "network_address": payload.get("network_address", DEFAULT_NETWORK_ADDRESS),
        "torii_address": payload.get("torii_address", DEFAULT_TORII_ADDRESS),
    }
    for key, value in values.items():
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"roster default `{key}` must be a non-empty string")
    torii_public_address = payload.get("torii_public_address")
    if torii_public_address is not None:
        if (
            not isinstance(torii_public_address, str)
            or not torii_public_address.strip()
        ):
            raise ValueError(
                "roster default `torii_public_address` must be a non-empty string"
            )
        torii_public_address = torii_public_address.strip()
    return RosterDefaults(
        network_address=values["network_address"].strip(),
        torii_address=values["torii_address"].strip(),
        torii_public_address=torii_public_address,
    )


def _optional_string(payload: dict[str, Any], key: str, context: str) -> str | None:
    value = payload.get(key)
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} field `{key}` must be a non-empty string")
    return value.strip()


def _optional_string_list(
    payload: dict[str, Any], key: str, context: str
) -> tuple[str, ...]:
    value = payload.get(key)
    if value is None:
        return ()
    if not isinstance(value, list) or not value:
        raise ValueError(f"{context} field `{key}` must be a non-empty array")
    normalized: list[str] = []
    for index, entry in enumerate(value, start=1):
        if not isinstance(entry, str) or not entry.strip():
            raise ValueError(
                f"{context} field `{key}` entry #{index} must be a non-empty string"
            )
        normalized.append(entry.strip())
    if len(set(normalized)) != len(normalized):
        raise ValueError(f"{context} field `{key}` must not contain duplicates")
    return tuple(normalized)


def _validate_ed25519_public_key(value: str, context: str) -> None:
    prefix = "ed0120"
    payload = value[len(prefix) :] if value.startswith(prefix) else ""
    if (
        len(payload) != 64
        or payload != payload.upper()
        or any(character not in "0123456789ABCDEF" for character in payload)
        or set(payload) == {"0"}
    ):
        raise ValueError(
            f"{context} must be a canonical non-zero Ed25519 multihash key "
            f"(`{prefix}` plus 64 uppercase hex characters)"
        )


def load_secret_material(path: Path) -> SecretMaterial:
    """Load per-validator private keys plus shared runtime-only secret material."""

    payload = _load_toml(path)
    validators_raw = _load_optional_validator_tables(payload, f"secrets file {path}")
    secrets: dict[str, str] = {}
    for raw in validators_raw:
        slug = _require_string(raw, "slug", f"secrets file `{path}`")
        private_key = _require_string(raw, "private_key", f"secrets file `{slug}`")
        if slug in secrets:
            raise ValueError(f"secrets file `{path}` duplicates validator slug `{slug}`")
        secrets[slug] = private_key
    shared_raw = payload.get("shared", {})
    if not isinstance(shared_raw, dict):
        raise ValueError(f"secrets file `{path}` field `shared` must be a TOML table")
    legacy_onboarding_fields = sorted(
        field
        for field in ("torii_onboarding_authority", "torii_onboarding_private_key")
        if field in shared_raw
    )
    if legacy_onboarding_fields:
        raise ValueError(
            f"secrets file `{path}` uses removed onboarding fields: "
            + ", ".join(legacy_onboarding_fields)
            + "; use account_onboarding_* fields"
        )
    sorafs_council_public_keys = _optional_string_list(
        shared_raw,
        "sorafs_council_public_keys",
        f"secrets file `{path}`",
    )
    for index, key in enumerate(sorafs_council_public_keys, start=1):
        _validate_ed25519_public_key(
            key,
            f"secrets file `{path}` SoraFS council key #{index}",
        )
    sorafs_council_signature_threshold = shared_raw.get(
        "sorafs_council_signature_threshold"
    )
    if sorafs_council_signature_threshold is not None and (
        isinstance(sorafs_council_signature_threshold, bool)
        or not isinstance(sorafs_council_signature_threshold, int)
        or sorafs_council_signature_threshold <= 0
    ):
        raise ValueError(
            f"secrets file `{path}` field `sorafs_council_signature_threshold` "
            "must be a positive integer"
        )
    if bool(sorafs_council_public_keys) != (
        sorafs_council_signature_threshold is not None
    ):
        raise ValueError(
            f"secrets file `{path}` must configure both sorafs_council_public_keys "
            "and sorafs_council_signature_threshold"
        )
    if (
        sorafs_council_signature_threshold is not None
        and sorafs_council_signature_threshold > len(sorafs_council_public_keys)
    ):
        raise ValueError(
            f"secrets file `{path}` SoraFS council threshold exceeds the trusted key count"
        )

    shared = SharedSecrets(
        account_onboarding_authority=_optional_string(
            shared_raw, "account_onboarding_authority", f"secrets file `{path}`"
        ),
        account_onboarding_private_key=_optional_string(
            shared_raw, "account_onboarding_private_key", f"secrets file `{path}`"
        ),
        account_onboarding_api_token=_optional_string(
            shared_raw, "account_onboarding_api_token", f"secrets file `{path}`"
        ),
        account_onboarding_credential_id=_optional_string(
            shared_raw, "account_onboarding_credential_id", f"secrets file `{path}`"
        ),
        account_onboarding_scope_domain=_optional_string(
            shared_raw, "account_onboarding_scope_domain", f"secrets file `{path}`"
        ),
        account_onboarding_scope_dataspace=_optional_string(
            shared_raw,
            "account_onboarding_scope_dataspace",
            f"secrets file `{path}`",
        ),
        torii_faucet_authority=_optional_string(
            shared_raw, "torii_faucet_authority", f"secrets file `{path}`"
        ),
        torii_faucet_private_key=_optional_string(
            shared_raw, "torii_faucet_private_key", f"secrets file `{path}`"
        ),
        streaming_identity_public_key=_optional_string(
            shared_raw, "streaming_identity_public_key", f"secrets file `{path}`"
        ),
        streaming_identity_private_key=_optional_string(
            shared_raw, "streaming_identity_private_key", f"secrets file `{path}`"
        ),
        sorafs_council_public_keys=sorafs_council_public_keys,
        sorafs_council_signature_threshold=sorafs_council_signature_threshold,
    )
    _validate_account_onboarding_secrets(shared, f"secrets file `{path}`")
    if bool(shared.torii_faucet_authority) != bool(shared.torii_faucet_private_key):
        raise ValueError(
            f"secrets file `{path}` must configure both torii_faucet_authority "
            "and torii_faucet_private_key"
        )

    return SecretMaterial(
        validators=secrets,
        shared=shared,
    )


def load_secret_keys(path: Path) -> dict[str, str]:
    """Load per-validator private keys from a user-local secrets file."""

    return load_secret_material(path).validators


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


def _render_governance_manifest(validators: list[ValidatorEntry]) -> str:
    """Render the Parliament lane manifest used for authoritative routing."""

    payload = {
        "lane": "governance",
        "governance": "parliament",
        "version": 1,
        "validators": [
            {
                "validator": validator.account_id,
                "peer_id": validator.public_key,
            }
            for validator in validators
        ],
        "quorum": max(1, (len(validators) * 2 // 3) + 1),
        "protected_namespaces": [
            "apps",
            "governance",
        ],
        "hooks": {
            "runtime_upgrade": {
                "allow": True,
                "require_metadata": True,
                "metadata_key": "gov_upgrade_id",
            },
        },
    }
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def render_genesis_template(
    base_genesis_path: Path,
    validators: list[ValidatorEntry],
    output_dir: Path,
) -> Path:
    """Render the unsigned shared genesis template with the exact public BLS roster.

    The matching private validator keys are intentionally absent. ``kagami
    genesis sign --config`` stages this template, derives the signed Nexus/AMX
    height-context commitment from the chosen validator config, and only then
    emits the final Norito genesis block.
    """

    payload = json.loads(base_genesis_path.read_text(encoding="utf-8"))
    transactions = payload.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        raise ValueError(
            f"base genesis {base_genesis_path} must contain a non-empty transactions array"
        )
    if not isinstance(payload.get("sumeragi_v2"), dict):
        raise ValueError(
            f"base genesis {base_genesis_path} is missing required sumeragi_v2 parameters"
        )
    for transaction in transactions:
        if not isinstance(transaction, dict):
            raise ValueError(
                f"base genesis {base_genesis_path} contains a non-object transaction"
            )
        transaction["topology"] = []

    registered_accounts: set[str] = set()
    for transaction in transactions:
        instructions = transaction.get("instructions", [])
        if not isinstance(instructions, list):
            raise ValueError(
                f"base genesis {base_genesis_path} contains a non-array instructions field"
            )
        for instruction in instructions:
            if not isinstance(instruction, dict):
                continue
            account = instruction.get("Register", {}).get("Account")
            if isinstance(account, dict) and isinstance(account.get("id"), str):
                registered_accounts.add(account["id"])

    validator_account_instructions = [
        {
            "Register": {
                "Account": {
                    "id": validator.account_id,
                    "metadata": {
                        "purpose": "taira_validator_payout_recipient",
                        "validator_slug": validator.slug,
                    },
                }
            }
        }
        for validator in validators
        if validator.account_id not in registered_accounts
    ]
    transactions.append(
        {
            "instructions": validator_account_instructions,
            "ivm_triggers": [],
            "topology": [
                {"peer": validator.public_key, "pop_hex": validator.pop_hex}
                for validator in validators
            ],
        }
    )

    target = output_dir / "genesis.json"
    target.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    signing_command = output_dir / "genesis-signing-command.txt"
    signing_command.write_text(
        " ".join(
            [
                "kagami genesis sign",
                str(target),
                "--config",
                str(output_dir / validators[0].slug / "config.toml"),
                "--private-key \"$TAIRA_GENESIS_PRIVATE_KEY\"",
                "--out-file",
                str(output_dir / "genesis.signed.nrt"),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    return target


def load_roster(
    path: Path,
    secrets_path: Path | None = None,
    secrets: SecretMaterial | None = None,
) -> list[ValidatorEntry]:
    """Load and validate Taira validator material."""

    payload = _load_toml(path)
    defaults = _load_defaults(payload)
    validators_raw = _load_validator_tables(payload, "roster")
    if len(validators_raw) < MIN_VALIDATORS:
        raise ValueError(
            f"roster must define at least {MIN_VALIDATORS} validators for Taira"
        )
    if len(validators_raw) > MAX_VALIDATORS:
        raise ValueError(
            f"roster must define at most {MAX_VALIDATORS} validators for the "
            "Sumeragi v2 protocol"
        )
    if secrets is None and secrets_path is not None:
        secrets = load_secret_material(secrets_path)
    secrets_by_slug = secrets.validators if secrets is not None else {}

    validators: list[ValidatorEntry] = []
    seen_slugs: set[str] = set()
    seen_account_ids: set[str] = set()
    seen_public_keys: set[str] = set()
    seen_public_addresses: set[str] = set()
    seen_torii_public_addresses: set[str] = set()
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"validator entry #{index} must be a TOML table")
        slug = _require_string(raw, "slug", f"validator `{index}`")
        public_key = _require_string(raw, "public_key", f"validator `{slug}`")
        private_key_value = raw.get("private_key", secrets_by_slug.get(slug))
        if not isinstance(private_key_value, str) or not private_key_value.strip():
            raise ValueError(
                f"validator `{slug}` is missing `private_key`; provide it inline or via --secrets"
            )
        private_key = private_key_value.strip()
        pop_hex = _require_string(raw, "pop_hex", f"validator `{slug}`")
        public_address = _require_string(raw, "public_address", f"validator `{slug}`")
        network_address = raw.get("network_address", defaults.network_address)
        torii_address = raw.get("torii_address", defaults.torii_address)
        torii_public_address = raw.get(
            "torii_public_address", defaults.torii_public_address
        )
        if not isinstance(network_address, str) or not network_address.strip():
            raise ValueError(f"validator `{slug}` field `network_address` is invalid")
        if not isinstance(torii_address, str) or not torii_address.strip():
            raise ValueError(f"validator `{slug}` field `torii_address` is invalid")
        if not isinstance(torii_public_address, str) or not torii_public_address.strip():
            raise ValueError(
                f"validator `{slug}` must set `torii_public_address` explicitly; "
                "public Taira deploys use direct per-node Torii hostnames"
            )
        if slug in seen_slugs:
            raise ValueError(f"validator slug `{slug}` is duplicated")
        account_id = _require_string(raw, "account_id", f"validator `{slug}`")
        if account_id in seen_account_ids:
            raise ValueError(f"validator account_id `{account_id}` is duplicated")
        if public_key in seen_public_keys:
            raise ValueError(f"validator public_key `{public_key}` is duplicated")
        if public_address in seen_public_addresses:
            raise ValueError(f"validator public_address `{public_address}` is duplicated")
        if torii_public_address.strip() in seen_torii_public_addresses:
            raise ValueError(
                f"validator torii_public_address `{torii_public_address.strip()}` is duplicated; "
                "each public validator must expose its own direct Torii hostname"
            )
        seen_slugs.add(slug)
        seen_account_ids.add(account_id)
        seen_public_keys.add(public_key)
        seen_public_addresses.add(public_address)
        seen_torii_public_addresses.add(torii_public_address.strip())
        validators.append(
            ValidatorEntry(
                slug=slug,
                account_id=account_id,
                public_key=public_key,
                private_key=private_key,
                pop_hex=pop_hex,
                public_address=public_address,
                network_address=network_address.strip(),
                torii_address=torii_address.strip(),
                torii_public_address=torii_public_address.strip(),
            )
        )

    unknown_secret_slugs = sorted(set(secrets_by_slug).difference(seen_slugs))
    if unknown_secret_slugs:
        raise ValueError(
            "secrets file contains validators not present in the public roster: "
            + ", ".join(unknown_secret_slugs)
        )

    return validators


def render_validator_config(
    template_text: str,
    validator: ValidatorEntry,
    validators: list[ValidatorEntry],
    shared_secrets: SharedSecrets | None = None,
    onboarding_private_key_file: Path | None = None,
    onboarding_token_hash: str | None = None,
    faucet_private_key_file: Path | None = None,
    sumeragi_body_bytes: int | None = None,
) -> str:
    """Rewrite the checked-in peer-1 baseline for one validator."""

    current_section: str | None = None
    skipping_array: str | None = None
    body_bytes_rewritten = False
    rendered: list[str] = []
    trusted_peers_lines = _render_trusted_peers(validators)
    trusted_peers_pop_lines = _render_trusted_peers_pop(validators)
    shared = shared_secrets or SharedSecrets()

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
        if (
            current_section == "[sumeragi.queues]"
            and stripped.partition("=")[0].strip() == "body_bytes"
            and sumeragi_body_bytes is not None
        ):
            rendered.append(f"body_bytes = {sumeragi_body_bytes}")
            body_bytes_rewritten = True
            continue
        if current_section == "[torii]" and stripped.startswith("address = "):
            rendered.append(f'address = {_quote_toml(validator.torii_address)}')
            continue
        if current_section == "[torii]" and stripped.startswith("public_address = "):
            rendered.append(
                f'public_address = {_quote_toml(validator.torii_public_address)}'
            )
            continue
        if (
            current_section == "[torii.account_onboarding]"
            and stripped.startswith("authority = ")
            and shared.account_onboarding_authority is not None
        ):
            rendered.append(
                f'authority = {_quote_toml(shared.account_onboarding_authority)}'
            )
            continue
        if (
            current_section == "[torii.account_onboarding]"
            and stripped.startswith("private_key_file = ")
            and onboarding_private_key_file is not None
        ):
            rendered.append(
                f'private_key_file = {_quote_toml(str(onboarding_private_key_file))}'
            )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("id = ")
            and shared.account_onboarding_credential_id is not None
        ):
            rendered.append(
                f'id = {_quote_toml(shared.account_onboarding_credential_id)}'
            )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("scope = ")
            and (
                shared.account_onboarding_scope_domain is not None
                or shared.account_onboarding_scope_dataspace is not None
            )
        ):
            if shared.account_onboarding_scope_domain is not None:
                rendered.append(
                    "scope = { domain = "
                    f"{_quote_toml(shared.account_onboarding_scope_domain)} }}"
                )
            else:
                rendered.append(
                    "scope = { dataspace = "
                    f"{_quote_toml(shared.account_onboarding_scope_dataspace or '')} }}"
                )
            continue
        if (
            current_section == "[[torii.account_onboarding.credentials]]"
            and stripped.startswith("token_hash = ")
            and onboarding_token_hash is not None
        ):
            rendered.append(f'token_hash = {_quote_toml(onboarding_token_hash)}')
            continue
        if (
            current_section == "[torii.faucet]"
            and stripped.startswith("authority = ")
            and shared.torii_faucet_authority is not None
        ):
            rendered.append(f'authority = {_quote_toml(shared.torii_faucet_authority)}')
            continue
        if (
            current_section == "[torii.faucet]"
            and stripped.startswith("private_key_file = ")
            and faucet_private_key_file is not None
        ):
            rendered.append(
                f'private_key_file = {_quote_toml(str(faucet_private_key_file))}'
            )
            continue
        if (
            current_section == "[streaming]"
            and stripped.startswith("identity_public_key = ")
            and shared.streaming_identity_public_key is not None
        ):
            rendered.append(
                f'identity_public_key = {_quote_toml(shared.streaming_identity_public_key)}'
            )
            continue
        if (
            current_section == "[sorafs.discovery.admission]"
            and stripped.startswith("trusted_council_keys = ")
            and shared.sorafs_council_public_keys
        ):
            rendered_keys = ", ".join(
                _quote_toml(key) for key in shared.sorafs_council_public_keys
            )
            rendered.append(f"trusted_council_keys = [{rendered_keys}]")
            continue
        if (
            current_section == "[sorafs.discovery.admission]"
            and stripped.startswith("signature_threshold = ")
            and shared.sorafs_council_signature_threshold is not None
        ):
            rendered.append(
                "signature_threshold = "
                f"{shared.sorafs_council_signature_threshold}"
            )
            continue
        if (
            current_section == "[streaming]"
            and stripped.startswith("identity_private_key = ")
            and shared.streaming_identity_private_key is not None
        ):
            rendered.append(
                f'identity_private_key = {_quote_toml(shared.streaming_identity_private_key)}'
            )
            continue
        if current_section == "[nexus.registry]" and stripped.startswith(
            "manifest_directory = "
        ):
            rendered.append('manifest_directory = "manifests"')
            continue
        if current_section == "[nexus.registry]" and stripped.startswith(
            "cache_directory = "
        ):
            rendered.append('cache_directory = "manifests"')
            continue

        rendered.append(raw_line)

    rendered_text = "\n".join(rendered)
    if not rendered_text.endswith("\n"):
        rendered_text += "\n"
    if sumeragi_body_bytes is not None and not body_bytes_rewritten:
        raise ValueError(
            f"rendered config for `{validator.slug}` could not rewrite the "
            "`[sumeragi.queues] body_bytes` assignment"
        )
    if "REPLACE_WITH_" in rendered_text:
        raise ValueError(
            f"rendered config for `{validator.slug}` still contains template placeholder "
            "values; provide the matching validator/shared secrets in the roster or "
            "--secrets file before rendering"
        )
    return rendered_text


def render_bundle(
    base_config_path: Path,
    roster_path: Path,
    output_dir: Path,
    secrets_path: Path | None = None,
    only: str | None = None,
    base_genesis_path: Path | None = None,
) -> list[Path]:
    """Render one config.toml per validator into output_dir."""

    secret_material = (
        load_secret_material(secrets_path) if secrets_path is not None else None
    )
    validators = load_roster(roster_path, secrets=secret_material)
    template = _load_toml(base_config_path)
    sumeragi_body_bytes = _scaled_sumeragi_body_bytes(template, len(validators))
    template_text = base_config_path.read_text(encoding="utf-8")
    output_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    output_dir.chmod(0o700)
    _write_private_text(output_dir / ".gitignore", "*\n!.gitignore")

    written: list[Path] = []
    for validator in validators:
        if only is not None and validator.slug != only:
            continue
        target_dir = output_dir / validator.slug
        target_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        target_dir.chmod(0o700)
        runtime_dir = target_dir / "runtime"
        runtime_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        runtime_dir.chmod(0o700)
        manifest_dir = target_dir / "manifests"
        manifest_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        manifest_dir.chmod(0o700)

        onboarding_private_key_file: Path | None = None
        onboarding_token_hash: str | None = None
        faucet_private_key_file: Path | None = None
        if secret_material is not None:
            shared = secret_material.shared
            if shared.account_onboarding_private_key is not None:
                onboarding_private_key_file = (
                    runtime_dir / "onboarding-signer.key"
                ).resolve()
                _write_private_text(
                    onboarding_private_key_file,
                    shared.account_onboarding_private_key,
                )
            if shared.account_onboarding_api_token is not None:
                _write_private_text(
                    runtime_dir / "onboarding-token",
                    shared.account_onboarding_api_token,
                )
                onboarding_token_hash = _blake3_token_hash(
                    shared.account_onboarding_api_token
                )
            if shared.torii_faucet_private_key is not None:
                faucet_private_key_file = (runtime_dir / "faucet-signer.key").resolve()
                _write_private_text(
                    faucet_private_key_file,
                    shared.torii_faucet_private_key,
                )

        target_path = target_dir / "config.toml"
        _write_private_text(
            target_path,
            render_validator_config(
                template_text,
                validator,
                validators,
                shared_secrets=secret_material.shared if secret_material else None,
                onboarding_private_key_file=onboarding_private_key_file,
                onboarding_token_hash=onboarding_token_hash,
                faucet_private_key_file=faucet_private_key_file,
                sumeragi_body_bytes=sumeragi_body_bytes,
            ),
        )
        (manifest_dir / "governance.manifest.json").write_text(
            _render_governance_manifest(validators),
            encoding="utf-8",
        )
        written.append(target_path)

    if only is not None and not written:
        raise ValueError(f"validator `{only}` is not present in {roster_path}")
    if base_genesis_path is not None:
        render_genesis_template(base_genesis_path, validators, output_dir)
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
        "--base-genesis",
        default="configs/soranexus/taira/genesis.json",
        help="checked-in unsigned Taira genesis template to populate with the public roster",
    )
    parser.add_argument(
        "--roster",
        required=True,
        help="TOML roster with validator public addresses, public keys, and PoPs",
    )
    parser.add_argument(
        "--secrets",
        help="optional user-local TOML with per-validator private keys",
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
        secrets_path=Path(args.secrets) if args.secrets else None,
        only=args.only,
        base_genesis_path=Path(args.base_genesis),
    )
    for path in written:
        print(f"config: {path}")
        runtime_dir = path.parent / "runtime"
        for filename in (
            "onboarding-signer.key",
            "onboarding-token",
            "faucet-signer.key",
        ):
            sidecar = runtime_dir / filename
            if sidecar.exists():
                print(f"sidecar: {sidecar}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
