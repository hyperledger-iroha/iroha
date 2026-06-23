#!/usr/bin/env python3
"""Build the Kagemusha 4-peer localnet lifecycle acceptance report."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import kagemusha_lineage_proof_evidence as lineage_helper  # noqa: E402
import kagemusha_localnet_lifecycle_evidence as localnet_helper  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


LOCALNET_ARTIFACT_FLAGS: tuple[tuple[str, str, str], ...] = (
    ("smoke_tx_hash", "smoke_artifact", "--smoke-artifact"),
    ("replay_rejection_hash", "replay_rejection_artifact", "--replay-rejection-artifact"),
    (
        "restart_replay_rejection_hash",
        "restart_replay_rejection_artifact",
        "--restart-replay-rejection-artifact",
    ),
    ("state_recovery_hash", "state_recovery_artifact", "--state-recovery-artifact"),
    ("lifecycle_shield_tx_hash", "lifecycle_shield_tx_artifact", "--lifecycle-shield-tx-artifact"),
    ("lifecycle_hop_proof_hash", "lifecycle_hop_proof_artifact", "--lifecycle-hop-proof-artifact"),
    (
        "lifecycle_recursive_init_hash",
        "lifecycle_recursive_init_artifact",
        "--lifecycle-recursive-init-artifact",
    ),
    (
        "lifecycle_recursive_init_verify_hash",
        "lifecycle_recursive_init_verify_artifact",
        "--lifecycle-recursive-init-verify-artifact",
    ),
    (
        "lifecycle_recursive_append_hash",
        "lifecycle_recursive_append_artifact",
        "--lifecycle-recursive-append-artifact",
    ),
    (
        "lifecycle_recursive_append_verify_hash",
        "lifecycle_recursive_append_verify_artifact",
        "--lifecycle-recursive-append-verify-artifact",
    ),
    (
        "lifecycle_unshield_proof_hash",
        "lifecycle_unshield_proof_artifact",
        "--lifecycle-unshield-proof-artifact",
    ),
    ("lifecycle_redeem_tx_hash", "lifecycle_redeem_tx_artifact", "--lifecycle-redeem-tx-artifact"),
)
LOCALNET_TRUE_FIELDS: tuple[str, ...] = (
    "smoke_passed",
    "replay_rejected",
    "restart_persistence_checked",
    "restart_replay_rejected",
    "state_recovery_passed",
    "lifecycle_passed",
)
LOCALNET_SOURCE_SCHEMA = "iroha.kagemusha.localnet.lifecycle.source.v1"
LOCALNET_SOURCE_KINDS: dict[str, str] = {
    "smoke_artifact": "transaction",
    "replay_rejection_artifact": "replay_rejection",
    "restart_replay_rejection_artifact": "replay_rejection",
    "state_recovery_artifact": "localnet_event",
    "lifecycle_shield_tx_artifact": "transaction",
    "lifecycle_hop_proof_artifact": "transaction",
    "lifecycle_recursive_init_artifact": "transaction",
    "lifecycle_recursive_init_verify_artifact": "transaction",
    "lifecycle_recursive_append_artifact": "transaction",
    "lifecycle_recursive_append_verify_artifact": "transaction",
    "lifecycle_unshield_proof_artifact": "transaction",
    "lifecycle_redeem_tx_artifact": "transaction",
}
LOCALNET_SOURCE_BASE_FIELDS = frozenset(
    (
        "schema",
        "artifact",
        "context",
        "run_id",
        "chain_id",
        "peer_ids",
        "generated_at_unix_ms",
        "kind",
        "non_empty_target",
    )
)
LOCALNET_SOURCE_KIND_FIELDS = {
    "transaction": frozenset(("tx_hash",)),
    "replay_rejection": frozenset(("replayed_tx_hash", "rejection")),
    "localnet_event": frozenset(("event",)),
}
LOCALNET_SOURCE_STRING_FIELD_MESSAGES: dict[str, dict[str, str]] = {
    "context": {
        "empty": "source artifact context must be a non-empty string",
        "control": "source artifact context must not contain control characters",
        "secret": "source artifact context must not contain secret-looking material",
    },
    "event": {
        "empty": "source artifact event must be a non-empty string",
        "control": "source artifact event must not contain control characters",
        "secret": "source artifact event must not contain secret-looking material",
    },
    "tx_hash": {
        "empty": "source artifact tx_hash must be a non-empty string",
        "control": "source artifact tx_hash must not contain control characters",
        "secret": "source artifact tx_hash must not contain secret-looking material",
    },
    "replayed_tx_hash": {
        "empty": "source artifact replayed_tx_hash must be a non-empty string",
        "control": "source artifact replayed_tx_hash must not contain control characters",
        "secret": "source artifact replayed_tx_hash must not contain secret-looking material",
    },
    "rejection": {
        "empty": "source artifact rejection must be a non-empty string",
        "control": "source artifact rejection must not contain control characters",
        "secret": "source artifact rejection must not contain secret-looking material",
    },
}
DEFAULT_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_PATH = (
    localnet_helper.DEFAULT_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_PATH
)


def _sha256_uri(path: Path, label: str) -> tuple[str | None, list[str]]:
    digest, errors = readiness._sha256_file(path, label)
    if errors:
        return None, errors
    assert digest is not None
    return f"sha256:{digest}", []


def validate_acceptance_output_path(artifact_dir: Path, out_path: Path) -> list[str]:
    """Reject unsafe acceptance output paths before hashing localnet artifacts."""

    if out_path.name == readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME:
        return ["--out must not use the release evidence filename"]
    if out_path.name != localnet_helper.LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME:
        return [
            "--out must be named "
            f"{localnet_helper.LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME}"
        ]
    corridor_errors = lineage_helper.validate_output_corridor(out_path, artifact_dir)
    if corridor_errors:
        return corridor_errors
    return lineage_helper.validate_output_path(out_path, "--out")


def validate_acceptance_identity(
    *,
    run_id: object,
    chain_id: object,
    peer_ids: object,
) -> list[str]:
    """Reject malformed localnet identities before hashing source artifacts."""

    errors: list[str] = []
    if not readiness._localnet_run_id_is_valid(run_id):
        errors.append("--run-id must identify a production 4-peer localnet run")
    if not readiness._localnet_chain_id_is_valid(chain_id):
        errors.append("--chain-id must identify a production localnet chain")
    if (
        not isinstance(peer_ids, list)
        or len(peer_ids) != readiness.EXPECTED_LOCALNET_PEER_COUNT
        or any(not readiness._localnet_peer_id_is_valid(peer_id) for peer_id in peer_ids)
        or len(set(peer_ids)) != readiness.EXPECTED_LOCALNET_PEER_COUNT
        or peer_ids != sorted(peer_ids)
    ):
        errors.append(
            "--peer-id must be repeated exactly four times with distinct sorted "
            "production localnet peer ids"
        )
    return errors


def validate_source_artifact_path_shapes(*, artifacts: dict[str, Path]) -> list[str]:
    """Reject malformed source artifact path strings before metadata reads."""

    errors: list[str] = []
    for _hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        path = artifacts.get(attr_name)
        if not isinstance(path, Path):
            errors.append(f"{flag} must be a local source artifact path")
            continue
        secret_error = lineage_helper._secret_path_error(str(path), f"{flag} path")
        if secret_error is not None:
            errors.append(secret_error)
    return errors


def validate_source_artifact_paths(*, artifacts: dict[str, Path]) -> list[str]:
    """Reject unsafe source artifact paths before hashing any source bytes."""

    shape_errors = validate_source_artifact_path_shapes(artifacts=artifacts)
    if shape_errors:
        return shape_errors

    errors: list[str] = []
    for _hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        path = artifacts.get(attr_name)
        if not isinstance(path, Path):
            errors.append(f"{flag} must be a local source artifact path")
            continue
        errors.extend(readiness.validate_lineage_local_file(path, flag))
    return errors


def validate_source_artifact_file_identities(*, artifacts: dict[str, Path]) -> list[str]:
    """Reject reused source artifact files before hashing any source bytes."""

    seen: dict[tuple[int, int], str] = {}
    errors: list[str] = []
    for _hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        path = artifacts.get(attr_name)
        if not isinstance(path, Path):
            errors.append(f"{flag} must be a local source artifact path")
            continue
        try:
            file_stat = path.lstat()
        except OSError:
            errors.append(f"{flag} file metadata could not be read")
            continue
        identity = (file_stat.st_dev, file_stat.st_ino)
        first_flag = seen.get(identity)
        if first_flag is not None:
            errors.append(
                f"{flag} source artifact file must be distinct from {first_flag}"
            )
            continue
        seen[identity] = flag
    return errors


def _json_positive_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _json_non_negative_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _json_non_empty_string(value: object) -> bool:
    return isinstance(value, str) and value.strip() == value and value != ""


def _append_json_source_string_errors(
    errors: list[str],
    document: dict[str, Any],
    field: str,
    flag: str,
) -> None:
    value = document.get(field)
    messages = LOCALNET_SOURCE_STRING_FIELD_MESSAGES[field]
    if not _json_non_empty_string(value):
        errors.append(f"{flag} {messages['empty']}")
        return
    assert isinstance(value, str)
    if readiness.device_lab._contains_control_character(value):  # type: ignore[attr-defined]
        errors.append(f"{flag} {messages['control']}")
    if readiness.device_lab.SECRET_RE.search(value):  # type: ignore[attr-defined]
        errors.append(f"{flag} {messages['secret']}")


def _append_source_hash_errors(
    errors: list[str],
    document: dict[str, Any],
    field: str,
    flag: str,
) -> None:
    value = document.get(field)
    if not _json_non_empty_string(value):
        return
    assert isinstance(value, str)
    if (
        readiness.device_lab._contains_control_character(value)  # type: ignore[attr-defined]
        or readiness.device_lab.SECRET_RE.search(value)  # type: ignore[attr-defined]
    ):
        return
    if (
        readiness.device_lab.SHA256_HEX_RE.fullmatch(value) is None  # type: ignore[attr-defined]
        or value == "0" * 64
    ):
        errors.append(
            f"{flag} source artifact {field} must be a non-zero lowercase sha256 hex string"
        )


def _source_artifact_messages(blockers: list[dict[str, Any]]) -> list[str]:
    return [
        str(blocker.get("message", "source artifact could not be loaded"))
        for blocker in blockers
    ]


def _validate_source_artifact_document(
    *,
    document: dict[str, Any],
    attr_name: str,
    flag: str,
    run_id: str,
    chain_id: str,
    peer_ids: list[str],
) -> list[str]:
    errors: list[str] = []
    expected_kind = LOCALNET_SOURCE_KINDS[attr_name]
    expected_fields = LOCALNET_SOURCE_BASE_FIELDS | LOCALNET_SOURCE_KIND_FIELDS[expected_kind]
    if set(document) - expected_fields:
        errors.append(f"{flag} source artifact contains unexpected field")
    if document.get("schema") != LOCALNET_SOURCE_SCHEMA:
        errors.append(f"{flag} source artifact schema must be {LOCALNET_SOURCE_SCHEMA}")
    if document.get("artifact") != attr_name:
        errors.append(f"{flag} source artifact slot must be {attr_name}")
    if document.get("run_id") != run_id:
        errors.append(f"{flag} source artifact run_id must match --run-id")
    if document.get("chain_id") != chain_id:
        errors.append(f"{flag} source artifact chain_id must match --chain-id")
    if document.get("peer_ids") != peer_ids:
        errors.append(f"{flag} source artifact peer_ids must match sorted --peer-id values")
    if not _json_non_negative_int(document.get("generated_at_unix_ms")):
        errors.append(f"{flag} source artifact generated_at_unix_ms must be a JSON integer")
    _append_json_source_string_errors(errors, document, "context", flag)
    if not _json_positive_int(document.get("non_empty_target")):
        errors.append(f"{flag} source artifact non_empty_target must be a positive JSON integer")

    if document.get("kind") != expected_kind:
        errors.append(f"{flag} source artifact kind must be {expected_kind}")
        return errors

    if expected_kind == "transaction":
        _append_json_source_string_errors(errors, document, "tx_hash", flag)
        _append_source_hash_errors(errors, document, "tx_hash", flag)
    elif expected_kind == "replay_rejection":
        _append_json_source_string_errors(errors, document, "replayed_tx_hash", flag)
        _append_source_hash_errors(errors, document, "replayed_tx_hash", flag)
        _append_json_source_string_errors(errors, document, "rejection", flag)
    elif expected_kind == "localnet_event":
        _append_json_source_string_errors(errors, document, "event", flag)
    else:  # pragma: no cover - guarded by LOCALNET_SOURCE_KINDS construction
        errors.append(f"{flag} source artifact kind is unsupported")
    return errors


def validate_source_artifact_documents(
    *,
    run_id: str,
    chain_id: str,
    peer_ids: list[str],
    artifacts: dict[str, Path],
) -> list[str]:
    """Reject source artifacts that are not bound to the reported localnet run."""

    errors: list[str] = []
    for _hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        document, blockers = readiness._load_json_artifact(
            artifacts[attr_name],
            missing_code="localnet_lifecycle_source_missing",
            invalid_code="localnet_lifecycle_source_invalid_json",
            unreadable_code="localnet_lifecycle_source_unreadable",
            shape_code="localnet_lifecycle_source_file_shape",
            not_object_code="localnet_lifecycle_source_not_object",
            label=f"{flag} source artifact",
            max_bytes=localnet_helper.MAX_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_JSON_BYTES,
        )
        if blockers:
            errors.extend(_source_artifact_messages(blockers))
            continue
        assert document is not None
        errors.extend(
            _validate_source_artifact_document(
                document=document,
                attr_name=attr_name,
                flag=flag,
                run_id=run_id,
                chain_id=chain_id,
                peer_ids=peer_ids,
            )
        )
    return errors


def build_acceptance_report(
    *,
    artifact_dir: Path,
    run_id: str,
    chain_id: str,
    peer_ids: list[str],
    artifacts: dict[str, Path],
) -> tuple[dict[str, Any] | None, list[str]]:
    """Build and readiness-validate the localnet lifecycle acceptance payload."""

    acceptance: dict[str, Any] = {
        "run_id": run_id,
        "target": readiness.EXPECTED_LOCALNET_TARGET,
        "peer_count": readiness.EXPECTED_LOCALNET_PEER_COUNT,
        "peer_ids": peer_ids,
        "chain_id": chain_id,
    }
    for field in LOCALNET_TRUE_FIELDS:
        acceptance[field] = True

    errors: list[str] = []
    identity_errors = validate_acceptance_identity(
        run_id=run_id,
        chain_id=chain_id,
        peer_ids=peer_ids,
    )
    if identity_errors:
        return None, identity_errors
    source_artifact_path_errors = validate_source_artifact_paths(artifacts=artifacts)
    if source_artifact_path_errors:
        return None, source_artifact_path_errors
    source_artifact_identity_errors = validate_source_artifact_file_identities(
        artifacts=artifacts
    )
    if source_artifact_identity_errors:
        return None, source_artifact_identity_errors
    source_artifact_document_errors = validate_source_artifact_documents(
        run_id=run_id,
        chain_id=chain_id,
        peer_ids=peer_ids,
        artifacts=artifacts,
    )
    if source_artifact_document_errors:
        return None, source_artifact_document_errors

    for hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        digest_uri, digest_errors = _sha256_uri(artifacts[attr_name], flag)
        if digest_errors:
            errors.extend(digest_errors)
            continue
        acceptance[hash_field] = digest_uri
    if errors:
        return None, errors

    evidence = {
        "schema": readiness.LOCALNET_LIFECYCLE_EVIDENCE_SCHEMA,
        "generated_at_utc": readiness.utc_now(),
        "localnet_run_id": run_id,
        "chain_id": chain_id,
        "localnet_acceptance": acceptance,
    }
    validation_errors = localnet_helper.validate_evidence_document(evidence, artifact_dir)
    if validation_errors:
        return None, validation_errors
    return acceptance, []


def write_acceptance_report(path: Path, report: dict[str, Any]) -> list[str]:
    """Write the canonical acceptance report with the shared private JSON writer."""

    return lineage_helper.write_evidence(
        path,
        report,
        max_bytes=localnet_helper.MAX_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_JSON_BYTES,
    )


def build_parser() -> argparse.ArgumentParser:
    """Return the CLI parser for the localnet lifecycle acceptance producer."""

    parser = argparse.ArgumentParser(
        description=(
            "Build the canonical Kagemusha 4-peer localnet lifecycle acceptance "
            "report from concrete run artifacts."
        )
    )
    parser.add_argument(
        "--artifact-dir",
        type=Path,
        default=Path("artifacts/kagemusha"),
        help="Directory where the acceptance report is written.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(DEFAULT_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_PATH),
        help="Output acceptance report path under --artifact-dir.",
    )
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--chain-id", required=True)
    parser.add_argument(
        "--peer-id",
        action="append",
        default=[],
        help="Production localnet peer id. Repeat exactly four times in sorted order.",
    )
    for _hash_field, attr_name, flag in LOCALNET_ARTIFACT_FLAGS:
        parser.add_argument(flag, dest=attr_name, type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    path_errors = validate_acceptance_output_path(args.artifact_dir, args.out)
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-localnet-lifecycle-acceptance] error: {error}", file=sys.stderr)
        return 1

    acceptance, errors = build_acceptance_report(
        artifact_dir=args.artifact_dir,
        run_id=args.run_id,
        chain_id=args.chain_id,
        peer_ids=args.peer_id,
        artifacts={attr_name: getattr(args, attr_name) for _, attr_name, _ in LOCALNET_ARTIFACT_FLAGS},
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-localnet-lifecycle-acceptance] error: {error}", file=sys.stderr)
        return 1
    assert acceptance is not None

    write_errors = write_acceptance_report(args.out, acceptance)
    if write_errors:
        for error in write_errors:
            print(f"[kagemusha-localnet-lifecycle-acceptance] error: {error}", file=sys.stderr)
        return 1

    print("[kagemusha-localnet-lifecycle-acceptance] wrote acceptance report")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
