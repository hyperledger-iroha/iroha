#!/usr/bin/env python3
"""Plan and atomically apply a typed alias setup document.

This is intentionally a small orchestration client.  Torii plans the setup
against live state and the ``iroha`` CLI verifies the returned plan, signs one
ordinary transaction, and submits it.  This script never constructs payment
proofs, handles private key material, or splits a setup into per-resource
requests.

The input is secret-free JSON with dependency-ordered resource groups::

    {
      "schema_version": 1,
      "dataspaces": [
        {
          "intent": {"kind": "Dataspace", "alias": "paynet", "owner": "..."},
          "acquisition": {"term_years": 1},
          "quote_guard": {
            "expected_policy_version": 1,
            "expected_payment_asset": "xor#sora",
            "max_amount": "1000",
            "valid_until_ms": 1900000000000
          }
        }
      ],
      "domains": [],
      "accounts": []
    }

Each entry is the JSON shape of ``EnsureAlias``: an ``AliasIntentV1``, an
``AliasLeaseAcquisitionV1``, and an ``AliasQuoteGuardV1``.  The planner request
contains one vector ordered as dataspaces, domains, then account aliases.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

SCHEMA_VERSION = 1
RESOURCE_GROUPS = ("dataspaces", "domains", "accounts")
EXPECTED_KINDS = {
    "dataspaces": "dataspace",
    "domains": "domain",
    "accounts": "account",
}
PLAN_HASH_RE = re.compile(r"^hash:([0-9A-F]{64})#([0-9A-F]{4})$")
SAFE_CODE_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,127}$")

# These are clean-break exclusions, not a general secret detector.  Public
# account keys and quote-guard payment asset IDs remain valid input.
FORBIDDEN_INTENT_KEYS = {
    "authority",
    "payer",
    "payment",
    "paymentproof",
    "paymentproofv1",
    "paymentsignature",
    "paymentgross",
    "paymentnet",
    "settlement",
    "settlementtx",
    "settlementproof",
    "suffixid",
    "leaseexpiryms",
    "privatekey",
    "privatekeyfile",
    "secretkey",
    "rawtoken",
    "token",
}
FORBIDDEN_PLAN_KEYS = {
    "privatekey",
    "privatekeyfile",
    "secretkey",
    "rawtoken",
    "token",
    "onboardingtoken",
    "onboardingtokenfile",
    "password",
}
RAW_SECRET_OPTIONS = {
    "--token",
    "--submit-token",
    "--private-key",
    "--secret-key",
    "--key",
}


class BulkOnboardError(Exception):
    """Raised when typed setup planning or application cannot continue safely."""


class PlanConflictError(BulkOnboardError):
    """Raised when Torii reports setup drift and deliberately returns no plan."""


@dataclass(frozen=True)
class ValidatedPlan:
    """A complete, blocker-free atomic transaction plan."""

    body: dict[str, Any]
    resource_count: int
    plan_hash: str


def _normalized_key(key: str) -> str:
    return re.sub(r"[^a-z0-9]", "", key.lower())


def _walk_keys(value: Any, *, path: str = "$") -> list[tuple[str, str]]:
    found: list[tuple[str, str]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str):
                raise BulkOnboardError(f"{path} contains a non-string JSON object key")
            child_path = f"{path}.{key}"
            found.append((child_path, _normalized_key(key)))
            found.extend(_walk_keys(child, path=child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found.extend(_walk_keys(child, path=f"{path}[{index}]"))
    return found


def _reject_forbidden_keys(
    value: Any,
    forbidden: set[str],
    *,
    context: str,
) -> None:
    matches = sorted(path for path, key in _walk_keys(value) if key in forbidden)
    if matches:
        # Paths identify schema mistakes without ever rendering their values.
        raise BulkOnboardError(
            f"{context} contains forbidden legacy or secret field(s): "
            + ", ".join(matches)
        )


def _reject_embedded_secrets(value: Any, *, context: str) -> None:
    """Reject unmistakable credential material even under an innocuous key."""

    if isinstance(value, str):
        lowered = value.lower()
        if "-----begin private key-----" in lowered or lowered.startswith("bearer "):
            raise BulkOnboardError(f"{context} contains embedded credential material")
        return
    if isinstance(value, list):
        for child in value:
            _reject_embedded_secrets(child, context=context)
        return
    if isinstance(value, dict):
        for child in value.values():
            _reject_embedded_secrets(child, context=context)


def _require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise BulkOnboardError(f"{context} must be a JSON object")
    return value


def _intent_kind(intent: Mapping[str, Any]) -> str | None:
    marker = intent.get("kind", intent.get("type"))
    if isinstance(marker, str):
        normalized = _normalized_key(marker)
        if normalized in {"dataspace", "dataspacealias"}:
            return "dataspace"
        if normalized in {"domain", "domainalias"}:
            return "domain"
        if normalized in {"account", "accountalias"}:
            return "account"
        return normalized

    for key in intent:
        normalized = _normalized_key(key)
        if normalized in {"dataspace", "dataspacealias"}:
            return "dataspace"
        if normalized in {"domain", "domainalias"}:
            return "domain"
        if normalized in {"account", "accountalias"}:
            return "account"
    return None


def _validate_quote_guard(value: Any, context: str) -> None:
    guard = _require_object(value, context)
    required = {
        "expected_policy_version",
        "expected_payment_asset",
        "max_amount",
        "valid_until_ms",
    }
    missing = sorted(required.difference(guard))
    extra = sorted(set(guard).difference(required))
    if missing:
        raise BulkOnboardError(f"{context} is missing: {', '.join(missing)}")
    if extra:
        raise BulkOnboardError(f"{context} contains unknown field(s): {', '.join(extra)}")

    policy_version = guard["expected_policy_version"]
    if isinstance(policy_version, bool) or not isinstance(policy_version, int):
        raise BulkOnboardError(f"{context}.expected_policy_version must be an integer")
    if policy_version < 0:
        raise BulkOnboardError(f"{context}.expected_policy_version must not be negative")

    payment_asset = guard["expected_payment_asset"]
    if not isinstance(payment_asset, str) or not payment_asset.strip():
        raise BulkOnboardError(f"{context}.expected_payment_asset must be a non-empty string")

    max_amount = guard["max_amount"]
    if isinstance(max_amount, bool) or not isinstance(max_amount, (int, str)):
        raise BulkOnboardError(f"{context}.max_amount must be a canonical integer or string")
    if isinstance(max_amount, int) and max_amount < 0:
        raise BulkOnboardError(f"{context}.max_amount must not be negative")
    if isinstance(max_amount, str) and not re.fullmatch(r"(?:0|[1-9][0-9]*)", max_amount):
        raise BulkOnboardError(f"{context}.max_amount must be a canonical unsigned integer")

    valid_until_ms = guard["valid_until_ms"]
    if isinstance(valid_until_ms, bool) or not isinstance(valid_until_ms, int):
        raise BulkOnboardError(f"{context}.valid_until_ms must be an integer")
    if valid_until_ms <= 0:
        raise BulkOnboardError(f"{context}.valid_until_ms must be positive")


def _validate_ensure_entry(entry: Any, group: str, index: int) -> dict[str, Any]:
    context = f"{group}[{index}]"
    ensure = _require_object(entry, context)
    expected_fields = {"intent", "acquisition", "quote_guard"}
    missing = sorted(expected_fields.difference(ensure))
    extra = sorted(set(ensure).difference(expected_fields))
    if missing:
        raise BulkOnboardError(f"{context} is missing: {', '.join(missing)}")
    if extra:
        raise BulkOnboardError(f"{context} contains unknown field(s): {', '.join(extra)}")

    intent = _require_object(ensure["intent"], f"{context}.intent")
    if not intent:
        raise BulkOnboardError(f"{context}.intent must not be empty")
    actual_kind = _intent_kind(intent)
    if actual_kind is None:
        raise BulkOnboardError(f"{context}.intent must declare its AliasIntentV1 variant")
    expected_kind = EXPECTED_KINDS[group]
    if actual_kind != expected_kind:
        raise BulkOnboardError(
            f"{context}.intent declares {actual_kind!r}, expected {expected_kind!r}"
        )

    acquisition = _require_object(ensure["acquisition"], f"{context}.acquisition")
    if not acquisition:
        raise BulkOnboardError(f"{context}.acquisition must not be empty")
    _validate_quote_guard(ensure["quote_guard"], f"{context}.quote_guard")
    return ensure


def validate_setup_intent(document: Any) -> dict[str, Any]:
    """Validate the secret-free setup document and return a defensive copy."""

    root = _require_object(document, "setup intent")
    expected_fields = {"schema_version", *RESOURCE_GROUPS}
    missing = sorted(expected_fields.difference(root))
    extra = sorted(set(root).difference(expected_fields))
    if missing:
        raise BulkOnboardError(f"setup intent is missing: {', '.join(missing)}")
    if extra:
        raise BulkOnboardError(
            "setup intent contains unknown field(s): " + ", ".join(extra)
        )
    if root["schema_version"] != SCHEMA_VERSION:
        raise BulkOnboardError(f"setup intent schema_version must be {SCHEMA_VERSION}")

    _reject_forbidden_keys(root, FORBIDDEN_INTENT_KEYS, context="setup intent")
    _reject_embedded_secrets(root, context="setup intent")
    count = 0
    for group in RESOURCE_GROUPS:
        entries = root[group]
        if not isinstance(entries, list):
            raise BulkOnboardError(f"setup intent {group} must be an array")
        for index, entry in enumerate(entries):
            _validate_ensure_entry(entry, group, index)
            count += 1
    if count == 0:
        raise BulkOnboardError("setup intent must contain at least one resource")
    return copy.deepcopy(root)


def load_setup_intent(path: Path) -> dict[str, Any]:
    """Load and validate a setup-intent file without echoing its contents."""

    try:
        raw = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise BulkOnboardError("cannot read setup intent file") from error
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        raise BulkOnboardError("setup intent file is not valid JSON") from error
    return validate_setup_intent(document)


def build_plan_request(document: Mapping[str, Any]) -> dict[str, Any]:
    """Build one dependency-ordered planner request from a validated document."""

    validated = validate_setup_intent(document)
    ordered: list[dict[str, Any]] = []
    for group in RESOURCE_GROUPS:
        ordered.extend(copy.deepcopy(validated[group]))
    return {
        "schema_version": SCHEMA_VERSION,
        "intents": ordered,
    }


def canonical_json_bytes(value: Any) -> bytes:
    """Encode deterministic JSON for the planner HTTP body."""

    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def is_canonical_plan_hash(value: str) -> bool:
    """Validate the Norito JSON literal used by ``HashOf<...>``.

    Iroha typed hashes render as ``hash:<64 uppercase hex>#<CRC16>``.  Merely
    matching the delimiters is insufficient because the checksum is part of
    the canonical literal.
    """

    matched = PLAN_HASH_RE.fullmatch(value)
    if matched is None:
        return False
    body, checksum = matched.groups()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    # Hash/HashOf also reserve the low bit as their validity marker.
    return checksum == f"{crc:04X}" and int(body[-2:], 16) & 1 == 1


def _safe_blocker_codes(blockers: Sequence[Any]) -> list[str]:
    codes: set[str] = set()
    for blocker in blockers:
        if not isinstance(blocker, dict):
            continue
        code = blocker.get("code")
        if isinstance(code, str) and SAFE_CODE_RE.fullmatch(code):
            codes.add(code)
    return sorted(codes)


def _collect_blockers(response: Mapping[str, Any], plan: Mapping[str, Any]) -> list[Any]:
    blockers: list[Any] = []
    for source in (response, plan):
        value = source.get("blockers", [])
        if value is None:
            continue
        if not isinstance(value, list):
            raise BulkOnboardError("planner blockers must be an array")
        blockers.extend(value)
    return blockers


def _status_is_blocked(value: Any) -> bool:
    return isinstance(value, str) and _normalized_key(value) in {
        "blocked",
        "conflict",
        "conflicted",
    }


def _disposition_is_conflict(value: Any) -> bool:
    if isinstance(value, str):
        return _normalized_key(value) == "conflict"
    if isinstance(value, dict):
        if any(_normalized_key(str(key)) == "conflict" for key in value):
            return True
        return any(
            _disposition_is_conflict(value.get(key))
            for key in ("disposition", "status", "kind")
            if key in value
        )
    return False


def _require_sequence_field(
    plan: Mapping[str, Any],
    names: Sequence[str],
    *,
    label: str,
) -> list[Any]:
    present = [name for name in names if name in plan]
    if len(present) != 1:
        expected = " or ".join(names)
        raise BulkOnboardError(f"plan must contain exactly one {label} field ({expected})")
    value = plan[present[0]]
    if not isinstance(value, list):
        raise BulkOnboardError(f"plan {present[0]} must be an array")
    return value


def validate_plan_response(response: Any, expected_resources: int) -> ValidatedPlan:
    """Reject blockers, drift, malformed hashes, and partial/split plans."""

    if expected_resources <= 0:
        raise BulkOnboardError("expected resource count must be positive")
    envelope = _require_object(response, "planner response")
    nested = envelope.get("plan")
    if "plan" in envelope:
        if nested is None:
            raise BulkOnboardError("planner returned no executable plan")
        plan = _require_object(nested, "planner response plan")
    else:
        plan = envelope

    body = _require_object(plan.get("body"), "planner response plan body")

    if (
        _status_is_blocked(envelope.get("status"))
        or _status_is_blocked(plan.get("status"))
        or _status_is_blocked(body.get("status"))
    ):
        raise PlanConflictError("planner classified alias setup as blocked or conflicting")

    blockers = _collect_blockers(envelope, plan)
    body_blockers = body.get("blockers", [])
    if body_blockers is not None:
        if not isinstance(body_blockers, list):
            raise BulkOnboardError("plan body blockers must be an array")
        blockers.extend(body_blockers)
    if blockers:
        codes = _safe_blocker_codes(blockers)
        detail = f": {', '.join(codes)}" if codes else ""
        raise BulkOnboardError(f"planner returned blocker(s){detail}")

    for key in ("partial", "partial_plan", "is_partial"):
        if envelope.get(key) is True or plan.get(key) is True:
            raise BulkOnboardError("planner returned a partial plan")

    schema_version = body.get("version")
    if schema_version != SCHEMA_VERSION:
        raise BulkOnboardError(f"plan body version must be {SCHEMA_VERSION}")

    plan_hash = plan.get("plan_hash")
    if not isinstance(plan_hash, str) or not is_canonical_plan_hash(plan_hash):
        raise BulkOnboardError(
            "plan_hash must use canonical hash:<64 uppercase hex>#<CRC16> format"
        )

    resources = _require_sequence_field(
        body,
        ("resources", "resource_dispositions", "dispositions"),
        label="resource disposition",
    )
    if len(resources) != expected_resources:
        raise BulkOnboardError(
            "planner resource count does not match the complete setup intent"
        )
    if any(_disposition_is_conflict(resource) for resource in resources):
        raise PlanConflictError("planner returned a conflicting resource disposition")

    frames = _require_sequence_field(
        body,
        ("framed_instructions", "instruction_frames", "instructions"),
        label="framed instruction",
    )
    if len(frames) != expected_resources:
        raise BulkOnboardError(
            "planner instruction count does not match the complete setup intent"
        )
    if any(frame in (None, "", [], {}) for frame in frames):
        raise BulkOnboardError("plan contains an empty framed instruction")

    transaction_count = body.get("transaction_count", 1)
    if isinstance(transaction_count, bool) or transaction_count != 1:
        raise BulkOnboardError("planner must return exactly one transaction")
    for key in ("batches", "chunks", "transactions"):
        if key not in body:
            continue
        groups = body[key]
        if not isinstance(groups, list) or len(groups) != 1:
            raise BulkOnboardError("planner attempted to split the resource vector")

    _reject_forbidden_keys(plan, FORBIDDEN_PLAN_KEYS, context="plan")
    _reject_embedded_secrets(plan, context="plan")
    return ValidatedPlan(copy.deepcopy(plan), expected_resources, plan_hash)


def write_plan_file(path: Path, plan: Mapping[str, Any]) -> None:
    """Atomically persist deterministic, secret-free JSON for the CLI."""

    _reject_forbidden_keys(plan, FORBIDDEN_PLAN_KEYS, context="plan")
    _reject_embedded_secrets(plan, context="plan")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.is_symlink():
            raise BulkOnboardError("plan output must not be a symlink")
        rendered = json.dumps(
            plan,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        ) + "\n"
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.",
            suffix=".tmp",
            dir=path.parent,
            text=True,
        )
        temporary_path = Path(temporary_name)
        try:
            os.fchmod(descriptor, 0o600)
            with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
                descriptor = -1
                handle.write(rendered)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary_path, path)
        finally:
            if descriptor >= 0:
                os.close(descriptor)
            try:
                temporary_path.unlink()
            except FileNotFoundError:
                pass
    except BulkOnboardError:
        raise
    except OSError as error:
        raise BulkOnboardError("cannot write plan output file") from error


def _safe_path_argument(path: Path, label: str) -> str:
    rendered = str(path)
    if not rendered or "\x00" in rendered or "\n" in rendered or "\r" in rendered:
        raise BulkOnboardError(f"{label} path is invalid")
    return rendered


def _base_cli_command(cli_path: str, config_file: Path | None) -> list[str]:
    if not cli_path or any(character in cli_path for character in "\x00\n\r"):
        raise BulkOnboardError("iroha CLI path is invalid")
    command = [cli_path]
    if config_file is not None:
        command.extend(["--config", _safe_path_argument(config_file, "config")])
    return command


def build_plan_command(
    cli_path: str,
    intent_file: Path,
    plan_file: Path,
    *,
    config_file: Path | None = None,
) -> list[str]:
    """Build the canonical-request-signed planner command."""

    return _base_cli_command(cli_path, config_file) + [
        "app",
        "alias",
        "setup",
        "plan",
        "--intent-file",
        _safe_path_argument(intent_file, "planner request"),
        "--plan-file",
        _safe_path_argument(plan_file, "plan"),
    ]


def build_apply_command(
    cli_path: str,
    plan_file: Path,
    *,
    config_file: Path | None = None,
) -> list[str]:
    """Build the exact-frame verification/sign/submission command."""

    return _base_cli_command(cli_path, config_file) + [
        "app",
        "alias",
        "setup",
        "apply",
        "--plan-file",
        _safe_path_argument(plan_file, "plan"),
    ]


def _run_alias_cli(command: Sequence[str], action: str, runner: Callable[..., Any] | None) -> None:
    run = runner or subprocess.run
    try:
        result = run(
            list(command),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
    except OSError as error:
        raise BulkOnboardError(f"cannot execute iroha alias setup {action}") from error
    if result.returncode != 0:
        # CLI output is deliberately not reflected; a downstream process must
        # not be able to inject configuration secrets into this script's log.
        raise BulkOnboardError(
            f"iroha alias setup {action} failed with exit status {result.returncode}"
        )


def request_plan_with_cli(
    cli_path: str,
    intent_file: Path,
    plan_file: Path,
    *,
    config_file: Path | None = None,
    runner: Callable[..., Any] | None = None,
) -> dict[str, Any]:
    """Have the Rust client sign the planner request and verify the returned plan."""

    _run_alias_cli(
        build_plan_command(
            cli_path,
            intent_file,
            plan_file,
            config_file=config_file,
        ),
        "plan",
        runner,
    )
    try:
        decoded = json.loads(plan_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise BulkOnboardError("iroha alias setup plan did not write valid JSON") from error
    return _require_object(decoded, "planner response")


def apply_plan(
    cli_path: str,
    plan_file: Path,
    *,
    config_file: Path | None = None,
    runner: Callable[..., Any] | None = None,
) -> None:
    """Delegate verification, signing, and one-transaction submission to ``iroha``."""

    command = build_apply_command(
        cli_path,
        plan_file,
        config_file=config_file,
    )
    _run_alias_cli(command, "apply", runner)


def redact_text(value: str, secrets: Sequence[str] = ()) -> str:
    """Return a bounded, single-line diagnostic with credentials removed."""

    redacted = value
    for secret in sorted((item for item in secrets if item), key=len, reverse=True):
        redacted = redacted.replace(secret, "<redacted>")
    redacted = re.sub(r"(?i)\bbearer\s+[^\s,;]+", "Bearer <redacted>", redacted)
    redacted = re.sub(
        r"(?i)\b(token|private[_ -]?key|secret|password)\s*[:=]\s*[^\s,;]+",
        lambda match: f"{match.group(1)}=<redacted>",
        redacted,
    )
    redacted = " ".join(redacted.replace("\x00", " ").splitlines()).strip()
    return redacted[:512]


class SafeArgumentParser(argparse.ArgumentParser):
    """Argparse variant that never reflects unknown argument values."""

    def error(self, message: str) -> None:
        if "unrecognized arguments" in message:
            message = "unsupported command-line argument"
        super().error(redact_text(message))


def _reject_raw_secret_options(argv: Sequence[str]) -> None:
    for argument in argv:
        option = argument.split("=", 1)[0]
        if option in RAW_SECRET_OPTIONS:
            raise BulkOnboardError(
                "raw token and private-key command-line values are forbidden; configure the "
                "file-backed signer in the ordinary client configuration"
            )


def build_argument_parser() -> argparse.ArgumentParser:
    parser = SafeArgumentParser(
        description=(
            "Plan a typed alias setup against live state and optionally apply the "
            "complete plan as one locally signed transaction."
        )
    )
    parser.add_argument("intent_file", help="Secret-free AliasIntentV1 setup JSON document.")
    parser.add_argument(
        "--plan-file",
        required=True,
        help="Destination for the secret-free AliasTransactionPlanV1 JSON.",
    )
    parser.add_argument(
        "--plan-only",
        action="store_true",
        help="Persist a verified plan without invoking the signing/submission CLI.",
    )
    parser.add_argument(
        "--iroha-cli",
        default="iroha",
        help="Path or command name for the iroha CLI (default: iroha).",
    )
    parser.add_argument(
        "--config",
        help="Client configuration path used for signed planning and local transaction signing.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    raw_argv = list(sys.argv[1:] if argv is None else argv)
    try:
        _reject_raw_secret_options(raw_argv)
        args = build_argument_parser().parse_args(raw_argv)

        intent_path = Path(args.intent_file).expanduser()
        plan_path = Path(args.plan_file).expanduser()
        if os.path.abspath(intent_path) == os.path.abspath(plan_path):
            raise BulkOnboardError("plan output must not overwrite the setup intent")

        setup_intent = load_setup_intent(intent_path)
        request_body = build_plan_request(setup_intent)
        resource_count = len(request_body["intents"])

        config_file = Path(args.config).expanduser() if args.config else None
        plan_path.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".alias-setup-", dir=plan_path.parent) as temp_dir:
            os.chmod(temp_dir, 0o700)
            request_path = Path(temp_dir) / "request.json"
            temporary_plan_path = Path(temp_dir) / "plan.json"
            write_plan_file(request_path, request_body)
            response = request_plan_with_cli(
                args.iroha_cli,
                request_path,
                temporary_plan_path,
                config_file=config_file,
            )
            validated = validate_plan_response(response, resource_count)
            write_plan_file(plan_path, validated.body)

        if not args.plan_only:
            apply_plan(
                args.iroha_cli,
                plan_path,
                config_file=config_file,
            )

        action = "Planned" if args.plan_only else "Applied"
        print(
            f"{action} {validated.resource_count} alias resource(s) atomically "
            f"with plan {validated.plan_hash}"
        )
        return 0
    except BulkOnboardError as error:
        print(f"error: {redact_text(str(error))}", file=sys.stderr)
        return 1
    except Exception:
        # Fail closed without reflecting unexpected exception text, paths, HTTP
        # bodies, or credentials into automation logs.
        print("error: unexpected alias setup failure", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
