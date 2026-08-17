"""Fail-closed tests for four-peer public-Taira lifecycle collection."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path

import pytest

from scripts import check_taira_public_v2_24h_soak_evidence as public_verifier
from scripts import collect_taira_public_v2_lifecycle_evidence as collector
from scripts import render_taira_validator_bundle as renderer
from scripts import taira_peer_supervisor as supervisor


DEPLOYED_MS = 1_000_000
RESTART_GENERATION = "a" * 64
NATIVE_BINARY = "b" * 64
NATIVE_SOURCE = "c" * 64
BINARY_SHA256 = "3" * 64
BINARY_STAT_SEAL = [101, 202, 303, 404, 505]


def canonical(value: object) -> bytes:
    """Encode one canonical test document."""

    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")


def private_directory(path: Path) -> Path:
    """Create one owner-private fixture directory."""

    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def write_private(path: Path, body: bytes) -> Path:
    """Write one owner-private, single-link fixture file."""

    path.write_bytes(body)
    path.chmod(0o600)
    return path


def node_id(index: int) -> str:
    """Return one canonical public fixture node identity."""

    return public_verifier._receipt_node_id(receipt_public_key(index))


def receipt_public_key(index: int) -> dict[str, str]:
    """Return one real compressed secp256k1 fixture public key."""

    payload = renderer._secp256k1_public_payload(index.to_bytes(32, "big"))
    return {"algorithm": "secp256k1", "payload_hex": payload.hex()}


def identity_args(index: int) -> argparse.Namespace:
    """Return peer-specific supervisor identity inputs."""

    return argparse.Namespace(
        binary_sha256=BINARY_SHA256,
        binary_device=BINARY_STAT_SEAL[0],
        binary_inode=BINARY_STAT_SEAL[1],
        binary_size=BINARY_STAT_SEAL[2],
        binary_mtime_ns=BINARY_STAT_SEAL[3],
        binary_ctime_ns=BINARY_STAT_SEAL[4],
        config_sha256=f"{index + 4:x}" * 64,
        restart_generation=RESTART_GENERATION,
    )


def peer_binding(index: int) -> str:
    """Return one exact peer-local binding digest."""

    return supervisor.lifecycle_binding_sha256(
        identity_args(index), public_verifier.VALIDATORS[index - 1], node_id(index)
    )


def deploy_handoff() -> dict[str, object]:
    """Return one closed deploy handoff with four public receipt signers."""

    receipt_signers: dict[str, object] = {}
    for index, validator in enumerate(public_verifier.VALIDATORS, start=1):
        runtime_binding = supervisor.terminal_binding_sha256(identity_args(index))
        receipt_signers[validator] = {
            "binary_stat_seal": list(BINARY_STAT_SEAL),
            "config_sha256": identity_args(index).config_sha256,
            "lifecycle_binding_sha256": peer_binding(index),
            "native_verifier_binary_sha256": NATIVE_BINARY,
            "native_verifier_receipt_sha256": f"{index + 4:x}" * 64,
            "native_verifier_receipt_size_bytes": 100 + index,
            "native_verifier_source_sha256": NATIVE_SOURCE,
            "node_id": node_id(index),
            "public_key": receipt_public_key(index),
            "runtime_binding_sha256": runtime_binding,
            "verification_result": "verified",
        }
    identity = {
        "admission_archive_sha256": "1" * 64,
        "admission_receipt_id": "2" * 64,
        "candidate_handoff_sha256": "3" * 64,
        "chain_id": "taira",
        "config_set_sha256": "4" * 64,
        "controller_host_id": "taira-controller-host",
        "controller_installation_id": "taira-controller-installation",
        "controller_sha256": "5" * 64,
        "deploy_handoff_manifest_sha256": "6" * 64,
        "deploy_receipt_sha256": "7" * 64,
        "deployment_completed_at_unix_ms": DEPLOYED_MS,
        "end_block_hash": {
            "algorithm": public_verifier.IROHA_HASH_ALGORITHM,
            "type": public_verifier.BLOCK_HASH_TYPE,
            "value": "8" * 64,
        },
        "end_height": 4,
        "genesis_block_hash": {
            "algorithm": public_verifier.IROHA_HASH_ALGORITHM,
            "type": public_verifier.BLOCK_HASH_TYPE,
            "value": "9" * 64,
        },
        "handoff_inventory_sha256": "a" * 64,
        "network_id": "taira-public",
        "network_name": "SORA Taira",
        "protocol_version": 4,
        "publication_handoff_sha256": "b" * 64,
        "publication_receipt_sha256": "c" * 64,
        "published_primary_oci_manifest_sha256": "d" * 64,
        "qualification_receipt_id": "e" * 64,
        "receipt_signers": receipt_signers,
        "restart_generation": RESTART_GENERATION,
        "signed_genesis_sha256": "f" * 64,
        "start_height": 1,
        "supervisor_sha256": "1" * 64,
        "topology_sha256": "2" * 64,
        "validator_binary_sha256": BINARY_SHA256,
    }
    assert set(identity) == public_verifier.DEPLOY_IDENTITY_FIELDS
    return {
        "identity": identity,
        "kind": "deploy",
        "schema": public_verifier.HANDOFF_SCHEMA,
        "schema_version": 1,
        "source": {
            "cargo_lock_sha256": "4" * 64,
            "commit": "5" * 40,
            "dpn_validator_release_commit": "6" * 40,
            "workspace_source_manifest_sha256": "7" * 64,
        },
    }


def raw_windows(
    tmp_path: Path,
    intervals: list[tuple[int, int, tuple[int, ...]]] | None = None,
) -> list[Path]:
    """Create four valid peer-local baseline-to-terminal windows."""

    export_root = private_directory(tmp_path / "raw")
    paths: list[Path] = []
    for index, validator in enumerate(public_verifier.VALIDATORS, start=1):
        journal_root = private_directory(tmp_path / f"runtime-{index}") / "journal"
        journal = supervisor.LifecycleJournal(
            journal_root,
            peer_binding(index),
            validator,
            node_id(index),
            RESTART_GENERATION,
        )
        try:
            if intervals is None:
                base_time = DEPLOYED_MS + 100
                baseline_time = base_time + 1
                record_times = (base_time + 10 + index, base_time + 20 + index)
                terminal_time = base_time + 30
            else:
                baseline_time, terminal_time, record_times = intervals[index - 1]
            journal.record("healthy", observed_at_unix_ms=baseline_time - 1)
            baseline = journal.checkpoint(captured_at_unix_ms=baseline_time)
            for observed_at in record_times:
                journal.record("healthy", observed_at_unix_ms=observed_at)
            terminal = journal.checkpoint(captured_at_unix_ms=terminal_time)
            target = export_root / f"{validator}.jsonl"
            journal.export_window(baseline, terminal, target)
            paths.append(target)
        finally:
            journal.close()
    return paths


def prepared_fixture(tmp_path: Path) -> tuple[Path, list[Path], Path, dict[str, object]]:
    """Prepare one valid collection and return its paths and request."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    output = private_directory(tmp_path / "output")
    request = collector._prepare(deploy_path, windows, output)
    return deploy_path, windows, output, request


def native_receipt(
    request: dict[str, object],
    binary_sha256: str = NATIVE_BINARY,
    source_sha256: str = NATIVE_SOURCE,
) -> dict[str, object]:
    """Return the exact external verifier response for one request."""

    return {
        "journal_artifact_sha256": request["journal_artifact_sha256"],
        "journal_artifact_size_bytes": request["journal_artifact_size_bytes"],
        "journal_record_count": request["journal_record_count"],
        "journal_records_sha256": request["journal_records_sha256"],
        "lifecycle_window_sha256": request["lifecycle_window_sha256"],
        "protocol": public_verifier.NATIVE_JOURNAL_VERIFIER_PROTOCOL,
        "schema": public_verifier.LIFECYCLE_JOURNAL_RECEIPT_SCHEMA,
        "schema_version": 1,
        "verification_result": "verified",
        "verifier_binary_sha256": binary_sha256,
        "verifier_source_sha256": source_sha256,
    }


def native_verifier(
    tmp_path: Path,
    request: dict[str, object],
    *,
    verification_result: str = "verified",
    mutate_output: bool = False,
) -> tuple[Path, str]:
    """Create one immutable executable that emits a request-bound receipt."""

    receipt = native_receipt(
        request,
        binary_sha256="BINARY_DIGEST_PLACEHOLDER",
        source_sha256="SOURCE_DIGEST_PLACEHOLDER",
    )
    receipt["verification_result"] = verification_result
    template = canonical(receipt).decode("ascii")
    prefix, remainder = template.split("BINARY_DIGEST_PLACEHOLDER", 1)
    middle, suffix = remainder.split("SOURCE_DIGEST_PLACEHOLDER", 1)
    mutation = ":" if not mutate_output else "printf attacker > \"$PWD/attacker\""
    script = (
        "#!/bin/sh\n"
        "set -eu\n"
        "[ \"$#\" -eq 18 ]\n"
        "prepared= journal= request= binary= source= raw_count=0\n"
        "while [ \"$#\" -gt 0 ]; do\n"
        "  case \"$1\" in\n"
        "    --prepared) prepared=$2 ;;\n"
        "    --journal) journal=$2 ;;\n"
        "    --request) request=$2 ;;\n"
        "    --raw-window)\n"
        "      raw_count=$((raw_count + 1))\n"
        "      case \"$raw_count:$2\" in\n"
        "        1:*/lifecycle-raw-taira-validator-1.jsonl) ;;\n"
        "        2:*/lifecycle-raw-taira-validator-2.jsonl) ;;\n"
        "        3:*/lifecycle-raw-taira-validator-3.jsonl) ;;\n"
        "        4:*/lifecycle-raw-taira-validator-4.jsonl) ;;\n"
        "        *) exit 65 ;;\n"
        "      esac\n"
        "      [ -f \"$2\" ]\n"
        "      ;;\n"
        "    --expected-verifier-binary-sha256) binary=$2 ;;\n"
        "    --expected-verifier-source-sha256) source=$2 ;;\n"
        "    *) exit 64 ;;\n"
        "  esac\n"
        "  shift 2\n"
        "done\n"
        "[ -f \"$prepared\" ] && [ -f \"$journal\" ] && [ -f \"$request\" ]\n"
        "[ \"$raw_count\" -eq 4 ]\n"
        f"{mutation}\n"
        f"printf '%s%s%s%s%s' '{prefix}' \"$binary\" '{middle}' \"$source\" '{suffix}'\n"
    ).encode("ascii")
    directory = private_directory(tmp_path / "native")
    executable = directory / "lifecycle-native-verifier"
    executable.write_bytes(script)
    executable.chmod(0o555)
    return executable, hashlib.sha256(script).hexdigest()


def artifact(path: Path) -> public_verifier.Artifact:
    """Capture one fixture with the verifier's immutable artifact type."""

    body = path.read_bytes()
    info = path.stat()
    return public_verifier.Artifact(
        path,
        body,
        hashlib.sha256(body).hexdigest(),
        len(body),
        info.st_dev,
        info.st_ino,
    )


def test_prepare_and_finalize_match_the_public_lifecycle_contract(tmp_path: Path) -> None:
    """Four raw chains become one exact journal accepted by the public checker."""

    _deploy_path, windows, output, request = prepared_fixture(tmp_path)
    verifier_path, verifier_digest = native_verifier(tmp_path, request)
    evidence = collector._finalize(
        output, verifier_path, verifier_digest, NATIVE_SOURCE
    )

    for source, name, identity in zip(
        windows,
        collector.RAW_FILENAMES,
        evidence["raw_windows"],
        strict=True,
    ):
        retained = output / name
        assert retained.read_bytes() == source.read_bytes()
        assert hashlib.sha256(retained.read_bytes()).hexdigest() == identity[
            "artifact_sha256"
        ]

    journal_artifact = artifact(output / collector.JOURNAL_FILENAME)
    evidence_artifact = artifact(output / collector.FINAL_FILENAME)
    receipt_artifact = artifact(output / collector.RECEIPT_FILENAME)
    deploy_identity = deploy_handoff()["identity"]
    deploy_projection = {
        "config_set_sha256": deploy_identity["config_set_sha256"],
        "deployment_completed_at_unix_ms": deploy_identity[
            "deployment_completed_at_unix_ms"
        ],
        "genesis_block_hash": deploy_identity["genesis_block_hash"]["value"],
        "native_verifier_receipts": {
            item["native_verifier_receipt_sha256"]
            for item in deploy_identity["receipt_signers"].values()
        },
        "receipt_signers": {
            validator: {
                "node_id": value["node_id"],
                "public_key": value["public_key"],
                "lifecycle_binding_sha256": value[
                    "lifecycle_binding_sha256"
                ],
            }
            for validator, value in deploy_identity["receipt_signers"].items()
        },
        "restart_generation": deploy_identity["restart_generation"],
        "signed_genesis_sha256": deploy_identity["signed_genesis_sha256"],
        "supervisor_sha256": deploy_identity["supervisor_sha256"],
        "topology_sha256": deploy_identity["topology_sha256"],
    }
    identity_sha256 = public_verifier._domain_digest(
        b"iroha.taira.public-v2-24h.lifecycle.v1\0", evidence
    )
    reference = {
        "identity_sha256": identity_sha256,
        "kind": "lifecycle-evidence",
        "schema": public_verifier.LIFECYCLE_SCHEMA,
        "sha256": evidence_artifact.sha256,
        "size_bytes": evidence_artifact.size,
    }
    baseline, returned_identity, metrics = public_verifier._validate_lifecycle(
        evidence_artifact,
        reference,
        journal_artifact=journal_artifact,
        native_receipt_artifact=receipt_artifact,
        deploy=deploy_projection,
        anchor_start_ms=int(evidence["baseline"]["captured_at_unix_ms"]) + 1,
        started_ms=int(evidence["baseline"]["captured_at_unix_ms"]) + 2,
        completed_ms=int(evidence["terminal"]["captured_at_unix_ms"]),
        expected_native_binary_sha256=verifier_digest,
        expected_native_source_sha256=NATIVE_SOURCE,
        used_native_receipts=set(deploy_projection["native_verifier_receipts"]),
    )
    assert set(baseline) == set(public_verifier.VALIDATORS)
    assert returned_identity == identity_sha256
    assert metrics["journal_record_count"] == 8


def test_prepare_globally_orders_and_resequences_four_local_streams(tmp_path: Path) -> None:
    """Colliding local indexes become one deterministic canonical sequence."""

    _deploy_path, _windows, output, request = prepared_fixture(tmp_path)
    lines = (output / collector.JOURNAL_FILENAME).read_bytes().splitlines()
    rows = [json.loads(line) for line in lines[1:]]
    assert [row["index"] for row in rows] == list(range(8))
    assert [row["journal_sequence"] for row in rows] == list(range(1, 9))
    assert [row["validator_id"] for row in rows[:4]] == list(
        public_verifier.VALIDATORS
    )
    assert request["journal_record_count"] == 8


def test_prepare_rejects_disjoint_peer_local_windows(tmp_path: Path) -> None:
    """A union of four unrelated time spans is not cohort-wide evidence."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    intervals = [
        (DEPLOYED_MS + offset, DEPLOYED_MS + offset + 30,
         (DEPLOYED_MS + offset + 10, DEPLOYED_MS + offset + 20))
        for offset in (100, 200, 300, 400)
    ]
    windows = raw_windows(tmp_path, intervals)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="do not overlap"):
        collector._prepare(deploy_path, windows, output)
    assert list(output.iterdir()) == []


def test_prepare_uses_only_the_common_staggered_interval(tmp_path: Path) -> None:
    """The global journal is the intersection, never the union, of peer windows."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    intervals = [
        (DEPLOYED_MS + 100, DEPLOYED_MS + 200,
         (DEPLOYED_MS + 130, DEPLOYED_MS + 150, DEPLOYED_MS + 199)),
        (DEPLOYED_MS + 110, DEPLOYED_MS + 190,
         (DEPLOYED_MS + 130, DEPLOYED_MS + 160, DEPLOYED_MS + 189)),
        (DEPLOYED_MS + 120, DEPLOYED_MS + 180,
         (DEPLOYED_MS + 140, DEPLOYED_MS + 160, DEPLOYED_MS + 179)),
        (DEPLOYED_MS + 130, DEPLOYED_MS + 170,
         (DEPLOYED_MS + 131, DEPLOYED_MS + 169)),
    ]
    windows = raw_windows(tmp_path, intervals)
    output = private_directory(tmp_path / "output")
    request = collector._prepare(deploy_path, windows, output)

    prepared = json.loads((output / collector.PREPARED_FILENAME).read_bytes())
    assert prepared["baseline"]["captured_at_unix_ms"] == DEPLOYED_MS + 130
    assert prepared["terminal"]["captured_at_unix_ms"] == DEPLOYED_MS + 170
    rows = [
        json.loads(line)
        for line in (output / collector.JOURNAL_FILENAME).read_bytes().splitlines()[1:]
    ]
    assert request["journal_record_count"] == 8
    assert all(
        DEPLOYED_MS + 130 <= row["observed_at_unix_ms"] <= DEPLOYED_MS + 170
        for row in rows
    )
    assert {
        validator: sum(row["validator_id"] == validator for row in rows)
        for validator in public_verifier.VALIDATORS
    } == {validator: 2 for validator in public_verifier.VALIDATORS}


def test_prepare_rejects_raw_record_digest_tampering(tmp_path: Path) -> None:
    """A caller cannot alter a raw row while retaining its exported header."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    lines = windows[0].read_bytes().splitlines(keepends=True)
    row = json.loads(lines[1])
    row["observed_at_unix_ms"] += 1
    windows[0].write_bytes(lines[0] + canonical(row) + b"".join(lines[2:]))
    windows[0].chmod(0o600)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="record digest"):
        collector._prepare(deploy_path, windows, output)
    assert list(output.iterdir()) == []


def test_prepare_rejects_local_terminal_chain_substitution(tmp_path: Path) -> None:
    """Rehashing the raw payload cannot substitute its local terminal chain."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    lines = windows[1].read_bytes().splitlines(keepends=True)
    header = json.loads(lines[0])
    header["terminal"]["journal_chain_sha256"] = "f" * 64
    windows[1].write_bytes(canonical(header) + b"".join(lines[1:]))
    windows[1].chmod(0o600)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="terminal chain"):
        collector._prepare(deploy_path, windows, output)


def test_prepare_rejects_locally_regressing_observation_times(tmp_path: Path) -> None:
    """Global sorting cannot hide a time regression in one peer-local chain."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    lines = windows[0].read_bytes().splitlines(keepends=True)
    header = json.loads(lines[0])
    rows = [json.loads(line) for line in lines[1:]]
    rows[1]["observed_at_unix_ms"] = rows[0]["observed_at_unix_ms"] - 1
    record_bytes = b"".join(canonical(row) for row in rows)
    header["records_sha256"] = hashlib.sha256(
        b"iroha.taira.peer-supervisor-raw-window-records.v1\0" + record_bytes
    ).hexdigest()
    chain = header["baseline"]["journal_chain_sha256"]
    for row in rows:
        original = dict(row)
        original["index"] = row["journal_sequence"] - 1
        chain = collector._local_next_chain(chain, original)
    header["terminal"]["journal_chain_sha256"] = chain
    windows[0].write_bytes(canonical(header) + record_bytes)
    windows[0].chmod(0o600)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="timestamps regress"):
        collector._prepare(deploy_path, windows, output)


def test_prepare_rejects_raw_binding_outside_the_deployed_runtime(
    tmp_path: Path,
) -> None:
    """A valid local chain from another runtime cannot enter the public window."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    lines = windows[1].read_bytes().splitlines(keepends=True)
    header = json.loads(lines[0])
    header["binding_sha256"] = "f" * 64
    windows[1].write_bytes(canonical(header) + b"".join(lines[1:]))
    windows[1].chmod(0o600)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="differs from deployment"):
        collector._prepare(deploy_path, windows, output)


def test_prepare_rejects_restart_or_counter_drift(tmp_path: Path) -> None:
    """The no-fault public soak cannot hide a peer restart in a raw window."""

    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy_handoff()))
    windows = raw_windows(tmp_path)
    lines = windows[2].read_bytes().splitlines(keepends=True)
    header = json.loads(lines[0])
    row = json.loads(lines[1])
    row["event"] = "restart"
    records = canonical(row) + b"".join(lines[2:])
    header["records_sha256"] = hashlib.sha256(
        b"iroha.taira.peer-supervisor-raw-window-records.v1\0" + records
    ).hexdigest()
    windows[2].write_bytes(canonical(header) + records)
    windows[2].chmod(0o600)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="non-healthy"):
        collector._prepare(deploy_path, windows, output)


def test_prepare_rejects_deploy_node_substitution(tmp_path: Path) -> None:
    """Each raw peer must match its deploy-authenticated receipt-signer node ID."""

    deploy = deploy_handoff()
    deploy["identity"]["receipt_signers"][public_verifier.VALIDATORS[0]][
        "node_id"
    ] = "taira-node:receipt-signer:substituted"
    deploy_path = write_private(tmp_path / "deploy.json", canonical(deploy))
    windows = raw_windows(tmp_path)
    output = private_directory(tmp_path / "output")
    with pytest.raises(collector.LifecycleCollectionError, match="not derived"):
        collector._prepare(deploy_path, windows, output)


def test_finalize_rejects_wrong_native_verifier_pin_without_publication(
    tmp_path: Path,
) -> None:
    """A self-consistent receipt from an untrusted verifier cannot finalize."""

    _deploy_path, _windows, output, request = prepared_fixture(tmp_path)
    verifier_path, _verifier_digest = native_verifier(tmp_path, request)
    with pytest.raises(collector.LifecycleCollectionError, match="executable pin"):
        collector._finalize(output, verifier_path, "d" * 64, NATIVE_SOURCE)
    assert not (output / collector.FINAL_FILENAME).exists()
    assert not (output / collector.RECEIPT_FILENAME).exists()
    assert not (output / collector.FINALIZE_PARTIAL_FILENAME).exists()


def test_finalize_rejects_retained_raw_window_tampering(tmp_path: Path) -> None:
    """Prepared identities cannot be reused with altered peer-local bytes."""

    _deploy_path, _windows, output, request = prepared_fixture(tmp_path)
    retained = output / collector.RAW_FILENAMES[0]
    lines = retained.read_bytes().splitlines(keepends=True)
    row = json.loads(lines[1])
    row["observed_at_unix_ms"] += 1
    retained.write_bytes(lines[0] + canonical(row) + b"".join(lines[2:]))
    retained.chmod(0o600)
    verifier_path, verifier_digest = native_verifier(tmp_path, request)

    with pytest.raises(collector.LifecycleCollectionError, match="record digest"):
        collector._finalize(output, verifier_path, verifier_digest, NATIVE_SOURCE)
    assert not (output / collector.FINAL_FILENAME).exists()
    assert not (output / collector.RECEIPT_FILENAME).exists()


def test_finalize_rejects_native_verifier_output_mutation(tmp_path: Path) -> None:
    """The pinned verifier cannot alter the prepared collector transaction."""

    _deploy_path, _windows, output, request = prepared_fixture(tmp_path)
    verifier_path, verifier_digest = native_verifier(
        tmp_path, request, mutate_output=True
    )
    with pytest.raises(collector.LifecycleCollectionError, match="output inventory"):
        collector._finalize(output, verifier_path, verifier_digest, NATIVE_SOURCE)
    assert not (output / collector.FINAL_FILENAME).exists()
    assert not (output / collector.RECEIPT_FILENAME).exists()


def test_finalize_rejects_unverified_native_result(tmp_path: Path) -> None:
    """A pinned executable cannot publish a receipt that reports failure."""

    _deploy_path, _windows, output, request = prepared_fixture(tmp_path)
    verifier_path, verifier_digest = native_verifier(
        tmp_path, request, verification_result="rejected"
    )
    with pytest.raises(collector.LifecycleCollectionError, match="does not exactly"):
        collector._finalize(output, verifier_path, verifier_digest, NATIVE_SOURCE)
    assert not (output / collector.FINAL_FILENAME).exists()
    assert not (output / collector.RECEIPT_FILENAME).exists()


def test_cli_help_exposes_only_prepare_and_finalize() -> None:
    """The script advertises its bounded two-phase interface."""

    parser = collector.build_parser()
    help_text = parser.format_help()
    assert "prepare" in help_text
    assert "finalize" in help_text
    assert "authority" in help_text
    finalize = parser._subparsers._group_actions[0].choices["finalize"]
    finalize_help = finalize.format_help()
    assert "--native-verifier" in finalize_help
    assert "--native-verifier-receipt" not in finalize_help
