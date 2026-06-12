"""Tests for scripts/check_android_device_lab_slot.py."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import tarfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest
from unittest import mock


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_android_device_lab_slot.py"
SPEC = importlib.util.spec_from_file_location("check_android_device_lab_slot", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
device_lab = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(device_lab)  # type: ignore[misc]

SIGNER_MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "sign_android_device_lab_evidence.py"
)
SIGNER_SPEC = importlib.util.spec_from_file_location(
    "sign_android_device_lab_evidence",
    SIGNER_MODULE_PATH,
)
assert SIGNER_SPEC and SIGNER_SPEC.loader  # pragma: no cover - import guard
evidence_signer = importlib.util.module_from_spec(SIGNER_SPEC)
SIGNER_SPEC.loader.exec_module(evidence_signer)  # type: ignore[misc]

SLOT_ASSEMBLER_MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "kagemusha_android_device_lab_slot.py"
)
SLOT_ASSEMBLER_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_android_device_lab_slot",
    SLOT_ASSEMBLER_MODULE_PATH,
)
assert SLOT_ASSEMBLER_SPEC and SLOT_ASSEMBLER_SPEC.loader  # pragma: no cover
slot_assembler = importlib.util.module_from_spec(SLOT_ASSEMBLER_SPEC)
SLOT_ASSEMBLER_SPEC.loader.exec_module(slot_assembler)  # type: ignore[misc]

ATTESTATION_REPORT_MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "kagemusha_android_attestation_report.py"
)
ATTESTATION_REPORT_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_android_attestation_report",
    ATTESTATION_REPORT_MODULE_PATH,
)
assert ATTESTATION_REPORT_SPEC and ATTESTATION_REPORT_SPEC.loader  # pragma: no cover
attestation_report = importlib.util.module_from_spec(ATTESTATION_REPORT_SPEC)
ATTESTATION_REPORT_SPEC.loader.exec_module(attestation_report)  # type: ignore[misc]

RAW_PULL_MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "kagemusha_pull_android_device_lab_raw_slot.py"
)
RAW_PULL_SPEC = importlib.util.spec_from_file_location(
    "kagemusha_pull_android_device_lab_raw_slot",
    RAW_PULL_MODULE_PATH,
)
assert RAW_PULL_SPEC and RAW_PULL_SPEC.loader  # pragma: no cover
raw_puller = importlib.util.module_from_spec(RAW_PULL_SPEC)
RAW_PULL_SPEC.loader.exec_module(raw_puller)  # type: ignore[misc]

KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS = (
    device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS
)
_MISSING_PATH_METHOD = object()
_PATH_TYPE = type(Path("."))
_PATH_TYPE_METHODS = ("exists", "is_symlink", "lstat", "mkdir", "rglob", "stat")
_ORIGINAL_PATH_TYPE_METHODS = {
    name: _PATH_TYPE.__dict__.get(name, _MISSING_PATH_METHOD)
    for name in _PATH_TYPE_METHODS
}


def restore_path_type_method_shadows() -> None:
    for name, original in _ORIGINAL_PATH_TYPE_METHODS.items():
        if original is _MISSING_PATH_METHOD:
            if name in _PATH_TYPE.__dict__:
                delattr(_PATH_TYPE, name)
        else:
            setattr(_PATH_TYPE, name, original)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def write_json(path: Path, payload: dict) -> None:
    write_text(path, json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_attestation_harness_result(
    path: Path,
    *,
    challenge_hex: str = "4145454245",
    attestation_security_level: str = "STRONG_BOX",
    keymaster_security_level: str = "STRONG_BOX",
    strongbox_attestation: bool = True,
    chain_length: int = 2,
    extra: dict | None = None,
) -> None:
    payload = {
        "alias": "strongbox-alias",
        "attestation_security_level": attestation_security_level,
        "keymaster_security_level": keymaster_security_level,
        "strongbox_attestation": strongbox_attestation,
        "challenge_hex": challenge_hex,
        "chain_length": chain_length,
    }
    if extra:
        payload.update(extra)
    write_json(path, payload)


def write_attestation_chain(path: Path) -> str:
    write_text(
        path,
        "-----BEGIN CERTIFICATE-----\n"
        "slot-bound-strongbox-keymint-certificate-leaf\n"
        "-----END CERTIFICATE-----\n"
        "-----BEGIN CERTIFICATE-----\n"
        "slot-bound-strongbox-keymint-certificate-issuer\n"
        "-----END CERTIFICATE-----\n",
    )
    return hashlib.sha256(path.read_bytes()).hexdigest()


def attestation_report_args(
    *,
    harness_result: Path,
    chain: Path,
    out: Path,
    extra: list[str] | None = None,
) -> list[str]:
    args = [
        "--harness-result",
        str(harness_result),
        "--slot-id",
        "pixel6",
        "--device-fingerprint",
        "google/oriole/oriole:16/CP1A.260405.005/15001963:user/release-keys",
        "--os-build-id",
        "CP1A.260405.005",
        "--attestation-certificate-chain",
        str(chain),
        "--physical-device-attestation",
        "--out",
        str(out),
    ]
    if extra:
        args.extend(extra)
    return args


def add_tar_file(tar: tarfile.TarFile, name: str, data: bytes) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(data)
    tar.addfile(info, io.BytesIO(data))


def add_tar_directory(tar: tarfile.TarFile, name: str) -> None:
    info = tarfile.TarInfo(name)
    info.type = tarfile.DIRTYPE
    tar.addfile(info)


def add_tar_link(
    tar: tarfile.TarFile,
    name: str,
    *,
    link_type: bytes,
    linkname: str,
) -> None:
    info = tarfile.TarInfo(name)
    info.type = link_type
    info.linkname = linkname
    tar.addfile(info)


def raw_slot_artifacts(slot_id: str = "pixel6") -> dict[str, bytes]:
    chain = b"".join(
        b"-----BEGIN CERTIFICATE-----\n"
        + f"slot-bound-strongbox-keymint-certificate-{index}\n".encode("utf-8")
        + b"-----END CERTIFICATE-----\n"
        for index in range(4)
    )
    challenge = bytes.fromhex("01020304")
    app_signing = hashlib.sha256(f"{slot_id}:app-signing".encode("utf-8")).hexdigest()
    policy = hashlib.sha256(b"kagemusha-offline-wallet-policy-v1").hexdigest()
    apk_digest = hashlib.sha256(f"{slot_id}:offline-wallet-apk".encode("utf-8")).hexdigest()
    queue_after = hashlib.sha256(f"{slot_id}:queue-after".encode("utf-8")).hexdigest()
    payload_digest = hashlib.sha256(f"{slot_id}:payload".encode("utf-8")).hexdigest()
    rollback_digest = hashlib.sha256(f"{slot_id}:rollback".encode("utf-8")).hexdigest()
    result = {
        "slot": slot_id,
        "status": "ok",
        "slot_id": slot_id,
        "device_fingerprint": "google/oriole/oriole:16/build/user/release-keys",
        "os_build_id": "build",
        "app_package_name": raw_puller.DEFAULT_RUN_AS_PACKAGE,
        "app_signing_certificate_sha256": app_signing,
        "attestation_challenge_sha256": hashlib.sha256(challenge).hexdigest(),
        "attestation_certificate_chain_path": "attestation/keymint-certificate-chain.pem",
        "attestation_certificate_chain_sha256": hashlib.sha256(chain).hexdigest(),
        "offline_wallet_policy_sha256": policy,
        "attestation_security_level": "STRONGBOX",
        "keymaster_security_level": "STRONGBOX",
        "keymint_security_level": "STRONGBOX",
        "strongbox_attestation": True,
        "physical_device_attestation": True,
    }
    return {
        "attestation/challenge.hex": challenge.hex().encode("utf-8") + b"\n",
        "attestation/harness-result.json": json.dumps(
            {
                "alias": "android-keystore-alias",
                "attestation_security_level": "STRONG_BOX",
                "keymaster_security_level": "STRONG_BOX",
                "strongbox_attestation": True,
                "challenge_hex": challenge.hex(),
                "chain_length": 4,
            },
            sort_keys=True,
        ).encode("utf-8")
        + b"\n",
        "attestation/keymint-certificate-chain.pem": chain,
        "attestation/result.json": json.dumps(result, sort_keys=True).encode("utf-8")
        + b"\n",
        "handoff/d2d-payment.json": json.dumps(
            {
                "schema": "iroha.android.device_lab.kagemusha.d2d_payment.v1",
                "slot_id": slot_id,
                "device_family": "Google Pixel 6 / 6a",
                "device_fingerprint": result["device_fingerprint"],
                "os_build_id": result["os_build_id"],
                "app_package_name": result["app_package_name"],
                "app_signing_certificate_sha256": app_signing,
                "attestation_challenge_sha256": result["attestation_challenge_sha256"],
                "offline_wallet_policy_sha256": policy,
                "offline_wallet_apk_sha256": apk_digest,
                "transport": "nearby_offline",
                "transport_offline": True,
                "payer_wallet_offline": True,
                "payee_wallet_offline": True,
                "payload_schema": "kagemusha.recursive_spend.reserved_lineage.d2d.v1",
                "payload_bytes": 3847,
                "transport_session_id_sha256": hashlib.sha256(
                    f"{slot_id}:session".encode("utf-8")
                ).hexdigest(),
                "payload_sha256": payload_digest,
                "received_payload_sha256": payload_digest,
                "receiver_ack_sha256": hashlib.sha256(
                    f"{slot_id}:ack".encode("utf-8")
                ).hexdigest(),
                "one_use_key_id_sha256": hashlib.sha256(
                    f"{slot_id}:one-use-key".encode("utf-8")
                ).hexdigest(),
                "payer_wallet_state_before_sha256": hashlib.sha256(
                    f"{slot_id}:payer-before".encode("utf-8")
                ).hexdigest(),
                "payer_wallet_state_after_sha256": hashlib.sha256(
                    f"{slot_id}:payer-after".encode("utf-8")
                ).hexdigest(),
                "payee_wallet_state_before_sha256": hashlib.sha256(
                    f"{slot_id}:payee-before".encode("utf-8")
                ).hexdigest(),
                "payee_wallet_state_after_sha256": hashlib.sha256(
                    f"{slot_id}:payee-after".encode("utf-8")
                ).hexdigest(),
                "queue_before_sha256": hashlib.sha256(
                    f"{slot_id}:queue-before".encode("utf-8")
                ).hexdigest(),
                "queue_after_sha256": queue_after,
                "one_use_key_consumed": True,
                "receiver_redeem_accepted": True,
                "double_spend_rejected": True,
            },
            sort_keys=True,
        ).encode("utf-8")
        + b"\n",
        "wallet/integrity.json": json.dumps(
            {
                "schema": "iroha.android.device_lab.kagemusha.wallet_integrity.v1",
                "slot_id": slot_id,
                "device_family": "Google Pixel 6 / 6a",
                "device_fingerprint": result["device_fingerprint"],
                "os_build_id": result["os_build_id"],
                "app_package_name": result["app_package_name"],
                "keymint_security_level": "STRONGBOX",
                "app_signing_certificate_sha256": app_signing,
                "attestation_challenge_sha256": result["attestation_challenge_sha256"],
                "attestation_certificate_chain_sha256": result[
                    "attestation_certificate_chain_sha256"
                ],
                "offline_wallet_policy_sha256": policy,
                "offline_wallet_apk_sha256": apk_digest,
                "rotation_session_id_sha256": hashlib.sha256(
                    f"{slot_id}:rotation".encode("utf-8")
                ).hexdigest(),
                "key_id_before_sha256": hashlib.sha256(
                    f"{slot_id}:key-before".encode("utf-8")
                ).hexdigest(),
                "key_id_after_sha256": hashlib.sha256(
                    f"{slot_id}:key-after".encode("utf-8")
                ).hexdigest(),
                "wallet_state_before_sha256": hashlib.sha256(
                    f"{slot_id}:wallet-before".encode("utf-8")
                ).hexdigest(),
                "wallet_state_after_rotation_sha256": hashlib.sha256(
                    f"{slot_id}:wallet-after".encode("utf-8")
                ).hexdigest(),
                "rollback_snapshot_sha256": rollback_digest,
                "restored_snapshot_sha256": rollback_digest,
                "one_use_key_rotation_passed": True,
                "old_key_invalidated": True,
                "rollback_rejection_passed": True,
                "stale_snapshot_rejected": True,
                "active_wallet_state_preserved_after_reject": True,
            },
            sort_keys=True,
        ).encode("utf-8")
        + b"\n",
        "telemetry/telemetry.json": json.dumps(
            {
                "schema_version": 1,
                "slot_id": slot_id,
                "suite": "kagemusha-device-lab",
                "device_model": "Pixel 6",
                "device_codename": "oriole",
                "app_package_name": result["app_package_name"],
            },
            sort_keys=True,
        ).encode("utf-8")
        + b"\n",
        "telemetry/status.ndjson": f'{{"status":"ok","slot_id":"{slot_id}"}}\n'.encode(
            "utf-8"
        ),
        "queue/pending_queue.json": json.dumps(
            {"slot_id": slot_id, "pending_transactions": []},
            sort_keys=True,
        ).encode("utf-8")
        + b"\n",
        "logs/runtime.log": b"kagemusha device-lab run complete\n",
    }


def write_raw_stage_slot(root: Path, slot_id: str = "pixel6") -> Path:
    stage_slot = root / slot_id
    for relative, data in raw_slot_artifacts(slot_id).items():
        path = stage_slot / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return stage_slot


def raw_slot_tar_bytes(
    slot_id: str = "pixel6",
    *,
    latest_slot_id: str | None = None,
    latest_slot_bytes: bytes | None = None,
    extra_files: dict[str, bytes] | None = None,
    omit_files: set[str] | None = None,
    symlinks: dict[str, str] | None = None,
    hardlinks: dict[str, str] | None = None,
) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w") as tar:
        add_tar_directory(tar, slot_id)
        for dirname in ("attestation", "handoff", "wallet", "telemetry", "queue", "logs"):
            add_tar_directory(tar, f"{slot_id}/{dirname}")
        skipped = omit_files or set()
        for relative, data in raw_slot_artifacts(slot_id).items():
            if relative in skipped:
                continue
            add_tar_file(tar, f"{slot_id}/{relative}", data)
        for relative, data in (extra_files or {}).items():
            add_tar_file(tar, relative, data)
        for relative, linkname in (symlinks or {}).items():
            add_tar_link(tar, relative, link_type=tarfile.SYMTYPE, linkname=linkname)
        for relative, linkname in (hardlinks or {}).items():
            add_tar_link(tar, relative, link_type=tarfile.LNKTYPE, linkname=linkname)
        add_tar_file(
            tar,
            "latest-slot.txt",
            latest_slot_bytes
            if latest_slot_bytes is not None
            else ((latest_slot_id or slot_id) + "\n").encode("utf-8"),
        )
    return buffer.getvalue()


def raw_pull_args(
    out_root: Path,
    *,
    slot_id: str | None = None,
    summary_out: Path | None = None,
) -> argparse.Namespace:
    return argparse.Namespace(
        adb="adb",
        serial="ABC123",
        run_as_package=raw_puller.DEFAULT_RUN_AS_PACKAGE,
        device_lab_root=raw_puller.DEFAULT_DEVICE_LAB_DEVICE_ROOT,
        slot_id=slot_id,
        out_root=out_root,
        summary_out=summary_out,
        adb_timeout_seconds=5,
    )


def fake_raw_pull_runner(tar_bytes: bytes, latest_slot_id: str = "pixel6"):
    calls: list[list[str]] = []

    def runner(command: list[str], **kwargs):
        calls.append(command)
        if "cat" in command:
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=latest_slot_id + "\n",
                stderr="",
            )
        if "exec-out" in command:
            return subprocess.CompletedProcess(command, 0, stdout=tar_bytes, stderr=b"")
        return subprocess.CompletedProcess(command, 1, stdout=b"", stderr=b"unexpected")

    runner.calls = calls  # type: ignore[attr-defined]
    return runner


def inject_duplicate_json_key(path: Path, key: str, first_value) -> None:
    text = path.read_text(encoding="utf-8")
    marker = f'"{key}":'
    index = text.find(marker)
    if index < 0:
        raise AssertionError(f"{path} does not contain JSON key {key}")
    line_start = text.rfind("\n", 0, index) + 1
    line_end = text.find("\n", index)
    if line_end < 0:
        line_end = len(text)
    line = text[line_start:line_end]
    indent = line[: len(line) - len(line.lstrip())]
    duplicate_line = f'{indent}"{key}": {json.dumps(first_value)},\n'
    path.write_text(text[:line_start] + duplicate_line + text[line_start:], encoding="utf-8")


def patch_slot_iterdir_failure(slot: Path):
    original_iterdir = Path.iterdir

    def failing_iterdir(path: Path):
        if path == slot:
            raise OSError("simulated slot listing failure")
        return original_iterdir(path)

    return mock.patch.object(Path, "iterdir", failing_iterdir)


def rewrite_sha256sum(slot: Path) -> None:
    lines = []
    for relative in sorted(device_lab._slot_files(slot)):  # type: ignore[attr-defined]
        path = slot / relative
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {relative}")
    write_text(slot / "sha256sum.txt", "\n".join(lines) + "\n")


def replace_with_symlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    link_path.unlink()
    try:
        link_path.symlink_to(target_path)
    except (NotImplementedError, OSError) as exc:
        test_case.skipTest(f"symlinks are not available in this test environment: {exc}")


def create_dir_symlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    try:
        link_path.symlink_to(target_path, target_is_directory=True)
    except (NotImplementedError, OSError) as exc:
        test_case.skipTest(f"directory symlinks are not available in this test environment: {exc}")


def replace_with_hardlink(test_case: unittest.TestCase, link_path: Path, target_path: Path) -> None:
    link_path.unlink()
    try:
        os.link(target_path, link_path)
    except (AttributeError, NotImplementedError, OSError) as exc:
        test_case.skipTest(f"hardlinks are not available in this test environment: {exc}")


def replace_with_fifo(test_case: unittest.TestCase, fifo_path: Path) -> None:
    fifo_path.unlink()
    try:
        os.mkfifo(fifo_path)
    except (AttributeError, NotImplementedError, OSError) as exc:
        test_case.skipTest(f"FIFOs are not available in this test environment: {exc}")


def with_read_bytes_failure(target_path: Path, callback):
    original_read_bytes = Path.read_bytes

    def failing_read_bytes(path: Path) -> bytes:
        if path == target_path:
            raise OSError("simulated read failure")
        return original_read_bytes(path)

    try:
        Path.read_bytes = failing_read_bytes
        return callback()
    finally:
        Path.read_bytes = original_read_bytes


def with_open_failure(target_path: Path, callback):
    original_open = Path.open

    def failing_open(path: Path, *args, **kwargs):
        if path == target_path:
            raise OSError("simulated open failure")
        return original_open(path, *args, **kwargs)

    try:
        Path.open = failing_open
        return callback()
    finally:
        Path.open = original_open


def with_write_text_failure(target_path: Path, callback):
    original_write_text = Path.write_text

    def failing_write_text(path: Path, *args, **kwargs) -> int:
        if path == target_path:
            raise OSError("simulated write failure")
        return original_write_text(path, *args, **kwargs)

    try:
        Path.write_text = failing_write_text
        return callback()
    finally:
        Path.write_text = original_write_text


def canonical_signed_evidence_payload(evidence: dict) -> bytes:
    payload = {
        key: value
        for key, value in evidence.items()
        if key not in {"signature", "signature_payload_sha256"}
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")


def create_test_signer(root: Path) -> dict[str, Path | str]:
    signer_dir = root / "trusted-signer"
    signer_dir.mkdir(parents=True, exist_ok=True)
    private_key = signer_dir / "ed25519-private.pem"
    public_key = signer_dir / "ed25519-public.pem"
    subprocess.run(
        ["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(private_key)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    subprocess.run(
        ["openssl", "pkey", "-in", str(private_key), "-pubout", "-out", str(public_key)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    der = subprocess.run(
        [
            "openssl",
            "pkey",
            "-pubin",
            "-in",
            str(public_key),
            "-pubout",
            "-outform",
            "DER",
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    ).stdout
    return {
        "private_key": private_key,
        "public_key": public_key,
        "public_key_sha256": hashlib.sha256(der).hexdigest(),
    }


def openssl_sign_ed25519(private_key: Path, payload: bytes) -> str:
    with tempfile.TemporaryDirectory() as temp:
        temp_path = Path(temp)
        payload_path = temp_path / "payload.bin"
        signature_path = temp_path / "signature.bin"
        payload_path.write_bytes(payload)
        subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-inkey",
                str(private_key),
                "-rawin",
                "-in",
                str(payload_path),
                "-out",
                str(signature_path),
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return signature_path.read_bytes().hex()


def sign_evidence(evidence: dict, signer: dict[str, Path | str]) -> dict:
    payload = canonical_signed_evidence_payload(evidence)
    evidence["signature_payload_sha256"] = hashlib.sha256(payload).hexdigest()
    evidence["signature"] = openssl_sign_ed25519(signer["private_key"], payload)  # type: ignore[arg-type]
    return evidence


def trusted_signers_for(signer: dict[str, Path | str]) -> dict[str, Path]:
    trusted, errors = device_lab.load_trusted_signer_public_keys([signer["public_key"]])
    if errors:
        raise AssertionError(errors)
    return trusted


def required_artifact_digests(slot: Path) -> dict[str, str]:
    digests = {}
    for relative in device_lab._required_signed_evidence_digest_paths(slot):  # type: ignore[attr-defined]
        digests[relative] = hashlib.sha256((slot / relative).read_bytes()).hexdigest()
    return digests


def refresh_signed_evidence_hash(slot: Path) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
        evidence_path.read_bytes()
    ).hexdigest()
    write_json(metadata_path, metadata)
    rewrite_sha256sum(slot)


def refresh_d2d_payment_transcript_hash(
    slot: Path,
    signer: dict[str, Path | str] | None = None,
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    transcript_path = slot / metadata["d2d_payment_transcript_path"]
    digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
    metadata["d2d_payment_transcript_sha256"] = digest
    write_json(metadata_path, metadata)
    if signer is not None:
        evidence_path = slot / "evidence" / "signed-evidence.json"
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
        evidence["d2d_payment_transcript_sha256"] = digest
        evidence["artifact_digests"] = required_artifact_digests(slot)
        write_json(evidence_path, sign_evidence(evidence, signer))
        refresh_signed_evidence_hash(slot)
    else:
        rewrite_sha256sum(slot)


def refresh_wallet_integrity_transcript_hash(
    slot: Path,
    signer: dict[str, Path | str] | None = None,
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    transcript_path = slot / metadata["wallet_integrity_transcript_path"]
    digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
    metadata["wallet_integrity_transcript_sha256"] = digest
    write_json(metadata_path, metadata)
    if signer is not None:
        evidence_path = slot / "evidence" / "signed-evidence.json"
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
        evidence["wallet_integrity_transcript_sha256"] = digest
        evidence["artifact_digests"] = required_artifact_digests(slot)
        write_json(evidence_path, sign_evidence(evidence, signer))
        refresh_signed_evidence_hash(slot)
    else:
        rewrite_sha256sum(slot)


def refresh_attestation_certificate_chain_hash(
    slot: Path,
    signer: dict[str, Path | str],
) -> None:
    metadata_path = slot / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    chain_path = slot / metadata["attestation_certificate_chain_path"]
    digest = hashlib.sha256(chain_path.read_bytes()).hexdigest()
    metadata["attestation_certificate_chain_sha256"] = digest
    write_json(metadata_path, metadata)

    attestation_path = slot / "attestation" / "result.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    attestation["attestation_certificate_chain_path"] = metadata[
        "attestation_certificate_chain_path"
    ]
    attestation["attestation_certificate_chain_sha256"] = digest
    write_json(attestation_path, attestation)

    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["attestation_certificate_chain_path"] = metadata[
        "attestation_certificate_chain_path"
    ]
    evidence["attestation_certificate_chain_sha256"] = digest
    evidence["artifact_digests"] = required_artifact_digests(slot)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(slot)


def mutate_signed_evidence(slot: Path, mutator) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    mutator(evidence)
    write_json(evidence_path, evidence)
    refresh_signed_evidence_hash(slot)


def resign_signed_evidence_artifacts(slot: Path, signer: dict[str, Path | str]) -> None:
    evidence_path = slot / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    evidence["artifact_digests"] = required_artifact_digests(slot)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(slot)


def write_d2d_payment_transcript(
    slot: Path,
    name: str,
    family: str,
    *,
    device_fingerprint: str,
    os_build_id: str,
    app_package_name: str,
    app_signing_certificate_sha256: str,
    attestation_challenge_sha256: str,
    offline_wallet_policy_sha256: str,
    offline_wallet_apk_sha256: str,
) -> tuple[str, str]:
    transcript_path = "handoff/d2d-payment.json"
    queue_after_sha256 = hashlib.sha256(
        (slot / "queue" / "pending_queue.json").read_bytes()
    ).hexdigest()
    queue_before_sha256 = hashlib.sha256(
        f"{name}:queue-before-d2d-payment".encode("utf-8")
    ).hexdigest()
    payload_sha256 = hashlib.sha256(
        f"{name}:reserved-lineage-d2d-payload".encode("utf-8")
    ).hexdigest()
    payer_before = hashlib.sha256(f"{name}:payer-wallet-before".encode("utf-8")).hexdigest()
    payer_after = hashlib.sha256(f"{name}:payer-wallet-after".encode("utf-8")).hexdigest()
    payee_before = hashlib.sha256(f"{name}:payee-wallet-before".encode("utf-8")).hexdigest()
    payee_after = hashlib.sha256(f"{name}:payee-wallet-after".encode("utf-8")).hexdigest()
    write_json(
        slot / transcript_path,
        {
            "schema": device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA,
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
            "transport": "nfc_hce",
            "transport_offline": True,
            "payer_wallet_offline": True,
            "payee_wallet_offline": True,
            "payload_schema": device_lab.D2D_PAYMENT_PAYLOAD_SCHEMA,
            "payload_bytes": 3847,
            "transport_session_id_sha256": hashlib.sha256(
                f"{name}:offline-handoff-session".encode("utf-8")
            ).hexdigest(),
            "payload_sha256": payload_sha256,
            "received_payload_sha256": payload_sha256,
            "receiver_ack_sha256": hashlib.sha256(
                f"{name}:receiver-ack".encode("utf-8")
            ).hexdigest(),
            "one_use_key_id_sha256": hashlib.sha256(
                f"{name}:one-use-key".encode("utf-8")
            ).hexdigest(),
            "payer_wallet_state_before_sha256": payer_before,
            "payer_wallet_state_after_sha256": payer_after,
            "payee_wallet_state_before_sha256": payee_before,
            "payee_wallet_state_after_sha256": payee_after,
            "queue_before_sha256": queue_before_sha256,
            "queue_after_sha256": queue_after_sha256,
            "one_use_key_consumed": True,
            "receiver_redeem_accepted": True,
            "double_spend_rejected": True,
        },
    )
    digest = hashlib.sha256((slot / transcript_path).read_bytes()).hexdigest()
    return transcript_path, digest


def write_wallet_integrity_transcript(
    slot: Path,
    name: str,
    family: str,
    *,
    device_fingerprint: str,
    os_build_id: str,
    app_package_name: str,
    app_signing_certificate_sha256: str,
    attestation_challenge_sha256: str,
    attestation_certificate_chain_sha256: str,
    offline_wallet_policy_sha256: str,
    offline_wallet_apk_sha256: str,
) -> tuple[str, str]:
    transcript_path = "wallet/integrity.json"
    key_before = hashlib.sha256(f"{name}:key-before".encode("utf-8")).hexdigest()
    key_after = hashlib.sha256(f"{name}:key-after".encode("utf-8")).hexdigest()
    wallet_before = hashlib.sha256(f"{name}:wallet-before".encode("utf-8")).hexdigest()
    wallet_after = hashlib.sha256(f"{name}:wallet-after-rotation".encode("utf-8")).hexdigest()
    rollback_snapshot = hashlib.sha256(
        f"{name}:rollback-snapshot".encode("utf-8")
    ).hexdigest()
    write_json(
        slot / transcript_path,
        {
            "schema": device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA,
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "keymint_security_level": "STRONGBOX",
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
            "rotation_session_id_sha256": hashlib.sha256(
                f"{name}:rotation-session".encode("utf-8")
            ).hexdigest(),
            "key_id_before_sha256": key_before,
            "key_id_after_sha256": key_after,
            "wallet_state_before_sha256": wallet_before,
            "wallet_state_after_rotation_sha256": wallet_after,
            "rollback_snapshot_sha256": rollback_snapshot,
            "restored_snapshot_sha256": rollback_snapshot,
            "one_use_key_rotation_passed": True,
            "old_key_invalidated": True,
            "rollback_rejection_passed": True,
            "stale_snapshot_rejected": True,
            "active_wallet_state_preserved_after_reject": True,
        },
    )
    digest = hashlib.sha256((slot / transcript_path).read_bytes()).hexdigest()
    return transcript_path, digest


def create_slot(
    root: Path,
    name: str,
    family: str | None = None,
    signer: dict[str, Path | str] | None = None,
) -> Path:
    slot = root / name
    device_fingerprint = f"{name}/fingerprint"
    os_build_id = f"{name}-build"
    app_package_name = "org.hyperledger.iroha.android.offlinewallet"
    app_signing_certificate_sha256 = hashlib.sha256(
        f"{name}:app-signing-certificate".encode("utf-8")
    ).hexdigest()
    attestation_challenge = f"{name}:attestation-challenge".encode("utf-8")
    attestation_challenge_sha256 = hashlib.sha256(attestation_challenge).hexdigest()
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    write_text(
        slot / attestation_certificate_chain_path,
        "-----BEGIN CERTIFICATE-----\n"
        f"{name}-strongbox-keymint-certificate-leaf\n"
        "-----END CERTIFICATE-----\n"
        "-----BEGIN CERTIFICATE-----\n"
        f"{name}-strongbox-keymint-certificate-issuer\n"
        "-----END CERTIFICATE-----\n",
    )
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    offline_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-offline-wallet-policy-v1"
    ).hexdigest()
    offline_wallet_apk_path = "evidence/offline-wallet-release.apk"
    write_text(
        slot / offline_wallet_apk_path,
        f"{name}:offline-wallet-release-apk\n",
    )
    offline_wallet_apk_sha256 = hashlib.sha256(
        (slot / offline_wallet_apk_path).read_bytes()
    ).hexdigest()
    raw_test_commands = list(KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS)
    write_json(
        slot / "telemetry" / "telemetry.json",
        {
            "schema_version": 1,
            "slot_id": name,
            "suite": "kagemusha-device-lab",
            "device_model": "Pixel 8",
            "device_codename": "husky",
            "app_package_name": app_package_name,
        },
    )
    write_text(
        slot / "telemetry" / "status.ndjson",
        f'{{"status":"ok","slot_id":"{name}"}}\n',
    )
    write_json(
        slot / "attestation" / "harness-result.json",
        {
            "alias": "android-keystore-alias",
            "attestation_security_level": "STRONG_BOX",
            "keymaster_security_level": "STRONG_BOX",
            "strongbox_attestation": True,
            "challenge_hex": attestation_challenge.hex(),
            "chain_length": 2,
        },
    )
    write_json(
        slot / "attestation" / "result.json",
        {
            "slot": name,
            "status": "ok",
            "slot_id": name,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "attestation_security_level": "STRONGBOX",
            "keymaster_security_level": "STRONGBOX",
            "keymint_security_level": "STRONGBOX",
            "strongbox_attestation": True,
            "physical_device_attestation": True,
        },
    )
    write_json(
        slot / "attestation" / "report.json",
        {
            "schema": device_lab.ATTESTATION_REPORT_SCHEMA,
            "slot_id": name,
            "device_fingerprint": device_fingerprint,
            "os_build_id": os_build_id,
            "app_package_name": app_package_name,
            "attestation_challenge_sha256": attestation_challenge_sha256,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "verifier": "android-keystore-attestation-harness",
            "verification": {
                "status": "ok",
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
                "attestation_security_level": "STRONGBOX",
                "keymaster_security_level": "STRONGBOX",
            },
        },
    )
    write_json(slot / "queue" / "pending_queue.json", {"slot_id": name, "pending_transactions": []})
    write_text(slot / "queue" / "pending.queue", "")
    write_text(slot / "logs" / "runtime.log", "kagemusha device-lab run complete\n")
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )
    wallet_integrity_transcript_path, wallet_integrity_transcript_sha256 = (
        write_wallet_integrity_transcript(
            slot,
            name,
            family or device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            device_fingerprint=device_fingerprint,
            os_build_id=os_build_id,
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=attestation_challenge_sha256,
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )

    if family is not None:
        if signer is None:
            raise AssertionError("production test slots require a signer")
        minimum_os = device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family]
        evidence_path = slot / "evidence" / "signed-evidence.json"
        write_json(
            evidence_path,
            sign_evidence(
                {
                    "schema": device_lab.SIGNED_EVIDENCE_SCHEMA,
                    "slot_id": name,
                    "device_family": family,
                    "device_fingerprint": device_fingerprint,
                    "os_build_id": os_build_id,
                    "minimum_os": minimum_os,
                    "app_package_name": app_package_name,
                    "attestation_certificate_chain_path": attestation_certificate_chain_path,
                    "offline_wallet_apk_path": offline_wallet_apk_path,
                    "d2d_payment_transcript_path": d2d_payment_transcript_path,
                    "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                    "app_signing_certificate_sha256": app_signing_certificate_sha256,
                    "attestation_challenge_sha256": attestation_challenge_sha256,
                    "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                    "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
                    "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
                    "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                    "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                    "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                    "strongbox_attestation": True,
                    "physical_device_attestation": True,
                    "keymint_security_level": "STRONGBOX",
                    "one_use_key_rotation_passed": True,
                    "rollback_rejection_passed": True,
                    "abi6_recursive_spend_jni_probe": "passed",
                    "abi7_recursive_compact_jni_probe": "one_hop_verified",
                    "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
                    "raw_test_commands": raw_test_commands,
                    "signed_at_utc": "2026-06-06T00:00:00Z",
                    "signer_key_id": "android-lab-release-signer-v1",
                    "signer_public_key_sha256": signer["public_key_sha256"],
                    "signature_algorithm": "ed25519",
                    "artifact_digests": required_artifact_digests(slot),
                },
                signer,
            ),
        )
        evidence_digest = hashlib.sha256(evidence_path.read_bytes()).hexdigest()
        write_json(
            slot / "slot.json",
            {
                "schema": "iroha.android.device_lab.kagemusha.v1",
                "slot_id": name,
                "device_family": family,
                "device_fingerprint": device_fingerprint,
                "os_build_id": os_build_id,
                "minimum_os": minimum_os,
                "app_package_name": app_package_name,
                "attestation_certificate_chain_path": attestation_certificate_chain_path,
                "offline_wallet_apk_path": offline_wallet_apk_path,
                "d2d_payment_transcript_path": d2d_payment_transcript_path,
                "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
                "app_signing_certificate_sha256": app_signing_certificate_sha256,
                "attestation_challenge_sha256": attestation_challenge_sha256,
                "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
                "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
                "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
                "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
                "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
                "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
                "one_use_key_rotation_passed": True,
                "rollback_rejection_passed": True,
                "abi6_recursive_spend_jni_probe": "passed",
                "abi7_recursive_compact_jni_probe": "one_hop_verified",
                "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
                "signed_evidence_artifact_path": "evidence/signed-evidence.json",
                "signed_evidence_artifact_sha256": evidence_digest,
                "raw_test_commands": raw_test_commands,
            },
        )

    rewrite_sha256sum(slot)
    return slot


def copy_slot_binding(
    *,
    source: Path,
    target: Path,
    signer: dict[str, Path | str],
    key: str,
) -> None:
    """Copy a signed slot binding across all artifacts for adversarial tests."""

    source_metadata = json.loads((source / "slot.json").read_text(encoding="utf-8"))
    copied = source_metadata[key]

    metadata_path = target / "slot.json"
    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata[key] = copied
    write_json(metadata_path, metadata)

    attestation_path = target / "attestation" / "result.json"
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    if key in attestation:
        attestation[key] = copied
        write_json(attestation_path, attestation)

    attestation_report_path = target / "attestation" / "report.json"
    attestation_report = json.loads(attestation_report_path.read_text(encoding="utf-8"))
    if key in attestation_report:
        attestation_report[key] = copied
        write_json(attestation_report_path, attestation_report)

    if key == "attestation_challenge_sha256":
        source_harness_path = source / "attestation" / "harness-result.json"
        source_harness = json.loads(source_harness_path.read_text(encoding="utf-8"))
        harness_path = target / "attestation" / "harness-result.json"
        harness = json.loads(harness_path.read_text(encoding="utf-8"))
        harness["challenge_hex"] = source_harness["challenge_hex"]
        write_json(harness_path, harness)

    transcript_path = target / "handoff" / "d2d-payment.json"
    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
    if key in transcript:
        transcript[key] = copied
        write_json(transcript_path, transcript)
        refresh_d2d_payment_transcript_hash(target, signer)

    wallet_transcript_path = target / "wallet" / "integrity.json"
    wallet_transcript = json.loads(wallet_transcript_path.read_text(encoding="utf-8"))
    if key in wallet_transcript:
        wallet_transcript[key] = copied
        write_json(wallet_transcript_path, wallet_transcript)
        refresh_wallet_integrity_transcript_hash(target, signer)

    evidence_path = target / "evidence" / "signed-evidence.json"
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    if key in evidence:
        evidence[key] = copied
    evidence["artifact_digests"] = required_artifact_digests(target)
    write_json(evidence_path, sign_evidence(evidence, signer))
    refresh_signed_evidence_hash(target)


def write_unsigned_production_slot_metadata(slot: Path, name: str, family: str) -> None:
    raw_test_commands = list(KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS)
    offline_wallet_apk_path = "evidence/offline-wallet-release.apk"
    app_package_name = "org.hyperledger.iroha.android.offlinewallet"
    app_signing_certificate_sha256 = hashlib.sha256(
        f"{name}:app-signing-certificate".encode("utf-8")
    ).hexdigest()
    offline_wallet_policy_sha256 = hashlib.sha256(
        b"kagemusha-offline-wallet-policy-v1"
    ).hexdigest()
    attestation_certificate_chain_path = "attestation/keymint-certificate-chain.pem"
    attestation_certificate_chain_sha256 = hashlib.sha256(
        (slot / attestation_certificate_chain_path).read_bytes()
    ).hexdigest()
    offline_wallet_apk_sha256 = hashlib.sha256(
        (slot / offline_wallet_apk_path).read_bytes()
    ).hexdigest()
    d2d_payment_transcript_path, d2d_payment_transcript_sha256 = (
        write_d2d_payment_transcript(
            slot,
            name,
            family,
            device_fingerprint=f"{name}/fingerprint",
            os_build_id=f"{name}-build",
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )
    wallet_integrity_transcript_path, wallet_integrity_transcript_sha256 = (
        write_wallet_integrity_transcript(
            slot,
            name,
            family,
            device_fingerprint=f"{name}/fingerprint",
            os_build_id=f"{name}-build",
            app_package_name=app_package_name,
            app_signing_certificate_sha256=app_signing_certificate_sha256,
            attestation_challenge_sha256=hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            attestation_certificate_chain_sha256=attestation_certificate_chain_sha256,
            offline_wallet_policy_sha256=offline_wallet_policy_sha256,
            offline_wallet_apk_sha256=offline_wallet_apk_sha256,
        )
    )
    write_json(
        slot / "slot.json",
        {
            "schema": "iroha.android.device_lab.kagemusha.v1",
            "slot_id": name,
            "device_family": family,
            "device_fingerprint": f"{name}/fingerprint",
            "os_build_id": f"{name}-build",
            "minimum_os": device_lab.KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS[family],
            "app_package_name": app_package_name,
            "attestation_certificate_chain_path": attestation_certificate_chain_path,
            "offline_wallet_apk_path": offline_wallet_apk_path,
            "d2d_payment_transcript_path": d2d_payment_transcript_path,
            "wallet_integrity_transcript_path": wallet_integrity_transcript_path,
            "app_signing_certificate_sha256": app_signing_certificate_sha256,
            "attestation_challenge_sha256": hashlib.sha256(
                f"{name}:attestation-challenge".encode("utf-8")
            ).hexdigest(),
            "attestation_certificate_chain_sha256": attestation_certificate_chain_sha256,
            "offline_wallet_policy_sha256": offline_wallet_policy_sha256,
            "offline_wallet_apk_sha256": offline_wallet_apk_sha256,
            "d2d_payment_transcript_sha256": d2d_payment_transcript_sha256,
            "wallet_integrity_transcript_sha256": wallet_integrity_transcript_sha256,
            "native_bridge_abi_version": device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
            "strongbox_attestation": True,
            "physical_device_attestation": True,
            "keymint_security_level": "STRONGBOX",
            "one_use_key_rotation_passed": True,
            "rollback_rejection_passed": True,
            "abi6_recursive_spend_jni_probe": "passed",
            "abi7_recursive_compact_jni_probe": "one_hop_verified",
            "abi7_recursive_compact_prover_state": "multi_hop_proof_composed",
            "signed_evidence_artifact_path": "evidence/signed-evidence.json",
            "signed_evidence_artifact_sha256": "0" * 64,
            "raw_test_commands": raw_test_commands,
        },
    )


def slot_assembler_args(
    *,
    slot_root: Path,
    source_slot: Path,
    signer: dict[str, Path | str] | None = None,
) -> list[str]:
    args = [
        "--slot-root",
        str(slot_root),
        "--slot-id",
        "pixel6",
        "--device-family",
        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
        "--device-fingerprint",
        "pixel6/fingerprint",
        "--os-build-id",
        "pixel6-build",
        "--device-model",
        "Pixel 6",
        "--device-codename",
        "oriole",
        "--attestation-result",
        str(source_slot / "attestation" / "result.json"),
        "--attestation-harness-result",
        str(source_slot / "attestation" / "harness-result.json"),
        "--attestation-report",
        str(source_slot / "attestation" / "report.json"),
        "--attestation-certificate-chain",
        str(source_slot / "attestation" / "keymint-certificate-chain.pem"),
        "--offline-wallet-apk",
        str(source_slot / "evidence" / "offline-wallet-release.apk"),
        "--d2d-payment-transcript",
        str(source_slot / "handoff" / "d2d-payment.json"),
        "--wallet-integrity-transcript",
        str(source_slot / "wallet" / "integrity.json"),
        "--telemetry-json",
        str(source_slot / "telemetry" / "telemetry.json"),
        "--status-ndjson",
        str(source_slot / "telemetry" / "status.ndjson"),
        "--pending-queue-json",
        str(source_slot / "queue" / "pending_queue.json"),
        "--runtime-log",
        str(source_slot / "logs" / "runtime.log"),
    ]
    if signer is not None:
        args.extend(
            [
                "--private-key",
                str(signer["private_key"]),
                "--public-key",
                str(signer["public_key"]),
                "--signer-key-id",
                "android-lab-release-signer-v1",
                "--signed-at-utc",
                "2026-06-06T00:00:00Z",
            ]
        )
    return args


class AndroidDeviceLabSlotTest(unittest.TestCase):
    def setUp(self) -> None:
        restore_path_type_method_shadows()

    def tearDown(self) -> None:
        restore_path_type_method_shadows()

    def test_checked_in_sample_slot_passes_default_validation(self) -> None:
        root = Path(__file__).resolve().parents[2] / "fixtures" / "android" / "device_lab"
        report = device_lab.scan_slot(root / "slot-sample")
        self.assertEqual(report["status"], "ok", report["errors"])

    def test_kagemusha_slot_metadata_defaults_to_lab_app_package(self) -> None:
        family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        metadata = slot_assembler.build_slot_metadata(
            slot_id="pixel6",
            family=family,
            facts={
                "device_fingerprint": "google/oriole/oriole:16/test:user/release-keys",
                "os_build_id": "TEST.1",
            },
            attestation_result={
                "app_signing_certificate_sha256": "1" * 64,
                "attestation_challenge_sha256": "2" * 64,
                "offline_wallet_policy_sha256": "7" * 64,
                "strongbox_attestation": True,
                "physical_device_attestation": True,
                "keymint_security_level": "STRONGBOX",
            },
            attestation_chain_path="attestation/keymint-certificate-chain.pem",
            attestation_chain_sha256="3" * 64,
            offline_wallet_apk_sha256="4" * 64,
            d2d_payment_transcript_sha256="5" * 64,
            wallet_integrity_transcript={
                "one_use_key_rotation_passed": True,
                "rollback_rejection_passed": True,
            },
            wallet_integrity_transcript_sha256="6" * 64,
            raw_test_commands=[],
        )

        self.assertEqual(
            metadata["app_package_name"],
            "org.hyperledger.iroha.sdk.offline.wallet.lab",
        )
        self.assertEqual(metadata["offline_wallet_policy_sha256"], "7" * 64)

    def test_kagemusha_slot_metadata_rejects_missing_source_policy_digest(self) -> None:
        family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        with self.assertRaisesRegex(
            ValueError,
            "attestation_result offline_wallet_policy_sha256 must be lowercase sha256 hex",
        ):
            slot_assembler.build_slot_metadata(
                slot_id="pixel6",
                family=family,
                facts={
                    "device_fingerprint": "google/oriole/oriole:16/test:user/release-keys",
                    "os_build_id": "TEST.1",
                },
                attestation_result={
                    "app_signing_certificate_sha256": "1" * 64,
                    "attestation_challenge_sha256": "2" * 64,
                    "strongbox_attestation": True,
                    "physical_device_attestation": True,
                    "keymint_security_level": "STRONGBOX",
                },
                attestation_chain_path="attestation/keymint-certificate-chain.pem",
                attestation_chain_sha256="3" * 64,
                offline_wallet_apk_sha256="4" * 64,
                d2d_payment_transcript_sha256="5" * 64,
                wallet_integrity_transcript={
                    "one_use_key_rotation_passed": True,
                    "rollback_rejection_passed": True,
                },
                wallet_integrity_transcript_sha256="6" * 64,
                raw_test_commands=[],
            )

    def test_kagemusha_slot_assembler_builds_signed_production_slot(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )
            trusted = trusted_signers_for(signer)
            report = device_lab.scan_slot(
                slot_root / "pixel6",
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            harness_exists = (
                slot_root / "pixel6" / "attestation" / "harness-result.json"
            ).is_file()
            signed_artifact_digests = json.loads(
                (slot_root / "pixel6" / "evidence" / "signed-evidence.json").read_text(
                    encoding="utf-8"
                )
            )["artifact_digests"]

        self.assertEqual(status, 0)
        self.assertIn("wrote", stdout.getvalue())
        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertTrue(harness_exists)
        self.assertIn(
            "attestation/harness-result.json",
            signed_artifact_digests,
        )

    def test_kagemusha_slot_assembler_requires_signing_by_default(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(slot_root=slot_root, source_slot=source_slot)
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signing inputs are required unless --allow-unsigned is set",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_control_root_before_classify(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_slot = Path(temp) / "source" / "pixel6"
            slot_root = Path(temp) / "device-lab\x1b[31m"
            args = slot_assembler.parse_args(
                slot_assembler_args(slot_root=slot_root, source_slot=source_slot)
            )

            with mock.patch.object(
                slot_assembler.device_lab,
                "classify_device_lab_root_path",
                side_effect=AssertionError(
                    "control root path should fail before classification"
                ),
            ):
                status, slot_path, errors = slot_assembler.assemble_slot(args)

            root_exists = slot_root.exists()

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertEqual(
            errors,
            ["device-lab root path must not contain control characters"],
        )
        self.assertFalse(root_exists)
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_slot_assembler_rejects_alias_root_before_classify(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "device-lab\\alias",
                "device-lab root path must not contain backslashes",
            ),
            (
                lambda base: base / "device-lab" / ".." / "alias",
                "device-lab root path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    source_slot = Path(temp) / "source" / "pixel6"
                    slot_root = path_factory(Path(temp))
                    args = slot_assembler.parse_args(
                        slot_assembler_args(slot_root=slot_root, source_slot=source_slot)
                    )

                    with mock.patch.object(
                        slot_assembler.device_lab,
                        "classify_device_lab_root_path",
                        side_effect=AssertionError(
                            "alias root path should fail before classification"
                        ),
                    ):
                        status, slot_path, errors = slot_assembler.assemble_slot(args)

                    root_exists = slot_root.exists()

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertEqual(errors, [expected_error])
                self.assertFalse(root_exists)

    def test_kagemusha_slot_assembler_rejects_control_source_metadata_string(
        self,
    ) -> None:
        errors: list[str] = []

        value = slot_assembler._require_source_string(  # type: ignore[attr-defined]
            {"device_fingerprint": "pixel6\x1b[31m"},
            "device_fingerprint",
            "slot.json",
            errors,
        )

        rendered = "\n".join(errors)
        self.assertIsNone(value)
        self.assertEqual(
            errors,
            ["slot.json device_fingerprint must not contain control characters"],
        )
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_slot_assembler_rejects_padded_slot_id_before_path_join(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--slot-id") + 1] = " pixel6 "

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)

        self.assertEqual(status, 1)
        self.assertIn("slot id must not contain whitespace", stderr.getvalue())
        self.assertFalse((slot_root / "pixel6").exists())
        self.assertFalse((slot_root / " pixel6 ").exists())

    def test_kagemusha_slot_assembler_rejects_noncanonical_slot_id_before_path_join(
        self,
    ) -> None:
        cases = ("./pixel6", "pixel6/", "pixel6/.")
        for slot_id in cases:
            with self.subTest(slot_id=slot_id):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    signer = create_test_signer(Path(temp) / "slot-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    slot_root = Path(temp) / "device-lab"
                    args = slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                    args[args.index("--slot-id") + 1] = slot_id

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(args)

                self.assertEqual(status, 1)
                self.assertIn(
                    "slot id must be a single safe directory name",
                    stderr.getvalue(),
                )
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_backslash_slot_id_before_path_join(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--slot-id") + 1] = "pixel\\6"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "slot id must be a single safe directory name",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel\\6").exists())

    def test_kagemusha_slot_assembler_rejects_control_slot_id_without_echo(
        self,
    ) -> None:
        unsafe_slot_id = "pixel6\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--slot-id") + 1] = unsafe_slot_id

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("slot id must not contain control characters", rendered)
        self.assertNotIn(unsafe_slot_id, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse((slot_root / unsafe_slot_id).exists())

    def test_kagemusha_slot_assembler_source_path_validators_reject_aliases_before_metadata(
        self,
    ) -> None:
        cases = (
            (
                "json-backslash",
                slot_assembler._load_source_json,  # type: ignore[attr-defined]
                "attestation result",
                lambda base: base / "source\\result.json",
                "attestation result path must not contain backslashes",
            ),
            (
                "json-parent",
                slot_assembler._load_source_json,  # type: ignore[attr-defined]
                "attestation result",
                lambda base: base / "source" / ".." / "result.json",
                "attestation result path must be canonical",
            ),
            (
                "copy-backslash",
                slot_assembler._normalise_source_path,  # type: ignore[attr-defined]
                "attestation certificate chain source",
                lambda base: base / "source\\chain.pem",
                "attestation certificate chain source path must not contain backslashes",
            ),
            (
                "copy-parent",
                slot_assembler._normalise_source_path,  # type: ignore[attr-defined]
                "attestation certificate chain source",
                lambda base: base / "source" / ".." / "chain.pem",
                "attestation certificate chain source path must be canonical",
            ),
        )
        for name, validator, label, path_factory, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    path = path_factory(Path(temp))
                    errors: list[str] = []

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias source path should fail before metadata"
                        ),
                    ):
                        if validator is slot_assembler._normalise_source_path:
                            result = validator(path, label, errors)
                        else:
                            result = validator(path, label, errors)

                self.assertIsNone(result)
                self.assertEqual(errors, [expected_error])

    def test_kagemusha_slot_assembler_rejects_padded_device_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--device-family") + 1] = (
                f" {device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]} "
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "device family must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_padded_identity_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--device-fingerprint") + 1] = " pixel6/fingerprint "

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "device_fingerprint must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_padded_adb_identity(self) -> None:
        outputs = {
            "ro.build.fingerprint": " google/oriole/oriole:16/test:user/release-keys \n",
            "ro.build.id": "CP1A.260405.005\n",
            "ro.product.model": "Pixel 6\n",
            "ro.product.device": "oriole\n",
        }

        def fake_run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess:
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=outputs[command[-1]],
                stderr="",
            )

        errors: list[str] = []
        with mock.patch.object(slot_assembler.subprocess, "run", side_effect=fake_run):
            facts = slot_assembler.read_device_identity(
                adb="adb",
                serial="ABC123",
                device_fingerprint=None,
                os_build_id=None,
                device_model=None,
                device_codename=None,
                errors=errors,
            )

        self.assertIn(
            "device_fingerprint must not contain surrounding whitespace",
            errors,
        )
        self.assertNotIn("device_fingerprint", facts)
        self.assertEqual(facts["os_build_id"], "CP1A.260405.005")
        self.assertEqual(facts["device_model"], "Pixel 6")
        self.assertEqual(facts["device_codename"], "oriole")

    def test_kagemusha_slot_assembler_rejects_noncanonical_adb_identity_output(
        self,
    ) -> None:
        outputs = {
            "ro.build.fingerprint": "google/oriole/oriole:16/test:user/release-keys",
            "ro.build.id": "CP1A.260405.005\n",
            "ro.product.model": "Pixel 6\n",
            "ro.product.device": "oriole\n",
        }

        def fake_run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess:
            return subprocess.CompletedProcess(
                command,
                0,
                stdout=outputs[command[-1]],
                stderr="",
            )

        errors: list[str] = []
        with mock.patch.object(slot_assembler.subprocess, "run", side_effect=fake_run):
            facts = slot_assembler.read_device_identity(
                adb="adb",
                serial=None,
                device_fingerprint=None,
                os_build_id=None,
                device_model=None,
                device_codename=None,
                errors=errors,
            )

        self.assertIn(
            (
                "adb getprop ro.build.fingerprint failed: adb getprop output "
                "must be exactly one LF-terminated value"
            ),
            errors,
        )
        self.assertNotIn("device_fingerprint", facts)
        self.assertEqual(facts["os_build_id"], "CP1A.260405.005")
        self.assertEqual(facts["device_model"], "Pixel 6")
        self.assertEqual(facts["device_codename"], "oriole")

    def test_kagemusha_slot_assembler_rejects_control_identity_override_without_echo(
        self,
    ) -> None:
        unsafe_codename = "oriole\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler_args(
                slot_root=slot_root,
                source_slot=source_slot,
                signer=signer,
            )
            args[args.index("--device-codename") + 1] = unsafe_codename

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(args)
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("device_codename must not contain control characters", rendered)
        self.assertNotIn(unsafe_codename, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_report_device_mismatch_before_install(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            report_path = source_slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["device_fingerprint"] = "other-device/fingerprint"
            write_json(report_path, report)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json device_fingerprint must match device identity",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_report_app_package_mismatch_before_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            report_path = source_slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["app_package_name"] = "org.hyperledger.iroha.android.other"
            write_json(report_path, report)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json app_package_name must match "
            "attestation/result.json app_package_name",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_unexpected_attestation_source_fields_before_publish(
        self,
    ) -> None:
        cases = (
            (
                "result",
                lambda result, _report: result.__setitem__("unexpected", "drift"),
                "attestation/result.json contains unexpected field unexpected",
            ),
            (
                "report",
                lambda _result, report: report.__setitem__("unexpected", "drift"),
                "attestation/report.json contains unexpected field unexpected",
            ),
            (
                "verification",
                lambda _result, report: report["verification"].__setitem__("debug", True),
                "attestation/report.json verification contains unexpected field debug",
            ),
        )
        for name, mutate, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    result_path = source_slot / "attestation" / "result.json"
                    report_path = source_slot / "attestation" / "report.json"
                    result = json.loads(result_path.read_text(encoding="utf-8"))
                    report = json.loads(report_path.read_text(encoding="utf-8"))
                    mutate(result, report)
                    write_json(result_path, result)
                    write_json(report_path, report)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected_error, stderr.getvalue())
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_bad_attestation_report_metadata_before_publish(
        self,
    ) -> None:
        cases = (
            (
                "schema",
                lambda report: report.__setitem__(
                    "schema",
                    "iroha.android.device_lab.kagemusha.attestation_report.v0",
                ),
                f"attestation/report.json schema must be {device_lab.ATTESTATION_REPORT_SCHEMA}",
            ),
            (
                "verifier",
                lambda report: report.__setitem__("verifier", "token=supersecret"),
                "attestation/report.json verifier must not contain secret-looking material",
            ),
        )
        for name, mutate, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    report_path = source_slot / "attestation" / "report.json"
                    report = json.loads(report_path.read_text(encoding="utf-8"))
                    mutate(report)
                    write_json(report_path, report)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected_error, stderr.getvalue())
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_unexpected_transcript_source_fields_before_publish(
        self,
    ) -> None:
        cases = (
            (
                "d2d",
                Path("handoff/d2d-payment.json"),
                "d2d payment transcript contains unexpected field unexpected",
            ),
            (
                "wallet",
                Path("wallet/integrity.json"),
                "wallet integrity transcript contains unexpected field unexpected",
            ),
        )
        for name, relative, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    transcript_path = source_slot / relative
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript["unexpected"] = "drift"
                    write_json(transcript_path, transcript)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected_error, stderr.getvalue())
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_transcript_schema_mismatch_before_publish(
        self,
    ) -> None:
        cases = (
            (
                "d2d",
                Path("handoff/d2d-payment.json"),
                "d2d payment transcript schema must be "
                f"{device_lab.D2D_PAYMENT_TRANSCRIPT_SCHEMA}",
            ),
            (
                "wallet",
                Path("wallet/integrity.json"),
                "wallet integrity transcript schema must be "
                f"{device_lab.WALLET_INTEGRITY_TRANSCRIPT_SCHEMA}",
            ),
        )
        for name, relative, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    transcript_path = source_slot / relative
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript["schema"] = "iroha.android.device_lab.kagemusha.legacy.v0"
                    write_json(transcript_path, transcript)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected_error, stderr.getvalue())
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_d2d_transcript_semantic_mismatch_before_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            transcript_path = source_slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["queue_after_sha256"] = "22" * 32
            write_json(transcript_path, transcript)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_wallet_transcript_semantic_mismatch_before_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            transcript_path = source_slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["wallet_state_after_rotation_sha256"] = transcript[
                "wallet_state_before_sha256"
            ]
            write_json(transcript_path, transcript)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "wallet integrity transcript wallet_state_before_sha256 must differ "
            "from wallet_state_after_rotation_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_malformed_required_runtime_artifacts_before_publish(
        self,
    ) -> None:
        cases = (
            (
                "queue",
                lambda slot: write_json(
                    slot / "queue" / "pending_queue.json",
                    {"slot_id": "pixel7", "pending_transactions": []},
                ),
                "queue/pending_queue.json slot_id must match slot id",
            ),
            (
                "telemetry",
                lambda slot: write_json(
                    slot / "telemetry" / "telemetry.json",
                    {"schema_version": 0, "slot_id": "pixel6", "suite": "kagemusha-device-lab"},
                ),
                "telemetry/telemetry.json schema_version must be 1",
            ),
            (
                "telemetry_app_package",
                lambda slot: write_json(
                    slot / "telemetry" / "telemetry.json",
                    {
                        "schema_version": 1,
                        "slot_id": "pixel6",
                        "suite": "kagemusha-device-lab",
                        "device_model": "Pixel 8",
                        "device_codename": "husky",
                        "app_package_name": "org.hyperledger.iroha.android.other",
                    },
                ),
                (
                    "telemetry/telemetry.json app_package_name must match "
                    "attestation/result.json app_package_name"
                ),
            ),
            (
                "status",
                lambda slot: write_text(
                    slot / "telemetry" / "status.ndjson",
                    '{"status":"failed","slot_id":"pixel6"}\n',
                ),
                "telemetry/status.ndjson line 1 status must not be 'failed'",
            ),
            (
                "status_closed_schema",
                lambda slot: write_text(
                    slot / "telemetry" / "status.ndjson",
                    '{"status":"ok","slot_id":"pixel6","debug_note":"ignored"}\n',
                ),
                "telemetry/status.ndjson line 1 contains unexpected field debug_note",
            ),
            (
                "status_value_closed",
                lambda slot: write_text(
                    slot / "telemetry" / "status.ndjson",
                    (
                        '{"status":"ok","slot_id":"pixel6"}\n'
                        '{"status":"skipped","slot_id":"pixel6"}\n'
                    ),
                ),
                "telemetry/status.ndjson line 2 status must be ok",
            ),
            (
                "status_slot_required",
                lambda slot: write_text(
                    slot / "telemetry" / "status.ndjson",
                    '{"status":"ok"}\n',
                ),
                "telemetry/status.ndjson line 1 slot_id must be a non-empty string",
            ),
            (
                "runtime",
                lambda slot: write_text(slot / "logs" / "runtime.log", "device-lab run started\n"),
                "logs/runtime.log must contain Kagemusha device-lab completion marker",
            ),
        )
        for name, mutate, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    mutate(source_slot)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected_error, stderr.getvalue())
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_symlinked_source_ancestor(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            external_evidence = Path(temp) / "external-evidence"
            shutil.copytree(source_slot / "evidence", external_evidence)
            shutil.rmtree(source_slot / "evidence")
            create_dir_symlink(self, source_slot / "evidence", external_evidence)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "offline wallet release APK source ancestor directory must not be a symlink",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_source_swap_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            apk_path = source_slot / "evidence" / "offline-wallet-release.apk"
            replacement = Path(temp) / "replacement.apk"
            replacement.write_bytes(b"replacement apk bytes")
            slot_root = Path(temp) / "device-lab"
            original_normalise = slot_assembler._normalise_source_path
            swapped = False

            def swapping_normalise(
                path: Path,
                label: str,
                errors: list[str],
            ):
                nonlocal swapped
                result = original_normalise(path, label, errors)
                if path == apk_path and result is not None and not swapped:
                    replace_with_symlink(self, apk_path, replacement)
                    swapped = True
                return result

            stderr = io.StringIO()
            with mock.patch.object(
                slot_assembler,
                "_normalise_source_path",
                side_effect=swapping_normalise,
            ):
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = slot_assembler.main(
                        slot_assembler_args(
                            slot_root=slot_root,
                            source_slot=source_slot,
                            signer=signer,
                        )
                    )

        self.assertEqual(status, 1)
        self.assertIn(
            "offline wallet release APK source changed while being read",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_publish_rejects_root_identity_swap(
        self,
    ) -> None:
        original_open = slot_assembler.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            slot_root = wrapper / "device-lab"
            slot_root.mkdir()
            root_identity = slot_assembler._file_identity(slot_root.lstat())
            temp_parent = slot_root / ".pixel6.stage"
            stage_slot = temp_parent / "pixel6"
            stage_slot.mkdir(parents=True)
            swapped_root = wrapper / "device-lab-swapped"
            swapped = False

            def swapping_root_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == slot_root and not swapped:
                    slot_root.rename(swapped_root)
                    slot_root.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(slot_assembler.os, "open", swapping_root_open):
                errors = slot_assembler._publish_stage_slot(
                    stage_slot=stage_slot,
                    root=slot_root,
                    slot_id="pixel6",
                    expected_root_identity=root_identity,
                    expected_temp_parent_identity=slot_assembler._file_identity(
                        temp_parent.lstat()
                    ),
                    expected_stage_identity=slot_assembler._file_identity(
                        stage_slot.lstat()
                    ),
                )
            final_slot_exists = (slot_root / "pixel6").exists()
            staged_slot_survived = (
                swapped_root / temp_parent.name / "pixel6"
            ).is_dir()

        self.assertTrue(swapped)
        self.assertEqual(errors, ["slot root directory changed before publish"])
        self.assertFalse(final_slot_exists)
        self.assertTrue(staged_slot_survived)

    def test_kagemusha_slot_assembler_publish_rejects_stage_identity_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            slot_root = wrapper / "device-lab"
            slot_root.mkdir()
            temp_parent = slot_root / ".pixel6.stage"
            stage_slot = temp_parent / "pixel6"
            stage_slot.mkdir(parents=True)
            root_identity = slot_assembler._file_identity(slot_root.lstat())
            temp_parent_identity = slot_assembler._file_identity(temp_parent.lstat())
            stage_identity = slot_assembler._file_identity(stage_slot.lstat())
            shutil.rmtree(stage_slot)
            stage_slot.mkdir()

            errors = slot_assembler._publish_stage_slot(
                stage_slot=stage_slot,
                root=slot_root,
                slot_id="pixel6",
                expected_root_identity=root_identity,
                expected_temp_parent_identity=temp_parent_identity,
                expected_stage_identity=stage_identity,
            )

        self.assertEqual(errors, ["staged slot directory changed before publish"])

    def test_kagemusha_slot_assembler_cleanup_removes_original_temp_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_parent = Path(temp) / "device-lab" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            (temp_parent / "marker").write_text("temporary\n", encoding="utf-8")
            temp_parent_identity = slot_assembler._file_identity(temp_parent.lstat())

            errors = slot_assembler._cleanup_temp_parent(
                temp_parent,
                expected_identity=temp_parent_identity,
            )
            removed = not temp_parent.exists()

        self.assertEqual(errors, [])
        self.assertTrue(removed)

    def test_kagemusha_slot_assembler_cleanup_reports_temp_parent_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_parent = Path(temp) / "device-lab" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            (temp_parent / "marker").write_text("temporary\n", encoding="utf-8")
            temp_parent_identity = slot_assembler._file_identity(temp_parent.lstat())

            with mock.patch.object(
                slot_assembler.shutil,
                "rmtree",
                side_effect=OSError("simulated slot cleanup failure"),
            ):
                errors = slot_assembler._cleanup_temp_parent(
                    temp_parent,
                    expected_identity=temp_parent_identity,
                )
            survived = temp_parent.is_dir()

        self.assertEqual(
            errors,
            ["staged slot temporary directory could not be removed"],
        )
        self.assertTrue(survived)

    def test_kagemusha_slot_assembler_cleanup_preserves_swapped_temp_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            temp_parent = wrapper / "device-lab" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            temp_parent_identity = slot_assembler._file_identity(temp_parent.lstat())
            swapped_temp_parent = wrapper / "swapped-stage"
            temp_parent.rename(swapped_temp_parent)
            temp_parent.mkdir()
            (temp_parent / "victim").write_text("do not remove\n", encoding="utf-8")

            errors = slot_assembler._cleanup_temp_parent(
                temp_parent,
                expected_identity=temp_parent_identity,
            )
            victim_survived = (temp_parent / "victim").is_file()
            original_survived = swapped_temp_parent.is_dir()

        self.assertEqual(errors, [])
        self.assertTrue(victim_survived)
        self.assertTrue(original_survived)

    def test_kagemusha_slot_assembler_reports_temp_parent_cleanup_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            slot_root = Path(temp) / "device-lab"
            args = slot_assembler.parse_args(
                slot_assembler_args(
                    slot_root=slot_root,
                    source_slot=source_slot,
                    signer=signer,
                )
            )

            with mock.patch.object(
                slot_assembler,
                "_cleanup_temp_parent",
                return_value=["staged slot temporary directory could not be removed"],
            ):
                status, slot_path, errors = slot_assembler.assemble_slot(args)
            final_slot_exists = (slot_root / "pixel6").is_dir()

        self.assertEqual(status, 1)
        self.assertEqual(slot_path, slot_root / "pixel6")
        self.assertEqual(
            errors,
            ["staged slot temporary directory could not be removed"],
        )
        self.assertTrue(final_slot_exists)

    def test_kagemusha_slot_assembler_json_write_rejects_parent_identity_swap(
        self,
    ) -> None:
        original_open = slot_assembler.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            slot_dir = wrapper / "slot"
            output = slot_dir / "slot.json"
            swapped_slot_dir = wrapper / "slot-swapped"
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == output.parent and not swapped:
                    output.parent.rename(swapped_slot_dir)
                    output.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(slot_assembler.os, "open", swapping_parent_open):
                errors = slot_assembler._write_json(
                    output,
                    {"schema": "test"},
                    "slot metadata",
                )
            written_text = (swapped_slot_dir / output.name).read_text(encoding="utf-8")

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["slot metadata parent directory changed before sync"],
        )
        self.assertIn('"schema": "test"', written_text)

    def test_kagemusha_slot_assembler_json_write_reports_temp_cleanup_failure(
        self,
    ) -> None:
        original_unlink = slot_assembler.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "slot.json"

            def failing_replace(_source: Path, _target: Path) -> None:
                raise OSError("simulated slot metadata replace failure")

            def failing_unlink(path: str, *args, **kwargs):
                if Path(path).name == f".{output.name}.android-slot.tmp":
                    raise OSError("simulated slot metadata temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(slot_assembler.os, "replace", failing_replace),
                mock.patch.object(slot_assembler.os, "unlink", failing_unlink),
            ):
                errors = slot_assembler._write_json(
                    output,
                    {"schema": "test"},
                    "slot metadata",
                )
            temp_exists = (output.parent / f".{output.name}.android-slot.tmp").exists()

        self.assertEqual(
            errors,
            [
                "slot metadata could not be written",
                "slot metadata temporary output could not be removed",
            ],
        )
        self.assertTrue(temp_exists)

    def test_kagemusha_slot_assembler_json_temp_cleanup_preserves_swapped_file(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            temp_output = root / ".slot.json.android-slot.tmp"
            temp_output.write_text("original\n", encoding="utf-8")
            temp_identity = slot_assembler._file_identity(temp_output.lstat())
            original_temp = root / "original-slot-temp"
            temp_output.rename(original_temp)
            temp_output.write_text("do not remove\n", encoding="utf-8")

            errors = slot_assembler._cleanup_temp_output(
                temp_output,
                "slot metadata",
                temp_identity,
            )
            replacement = temp_output.read_text(encoding="utf-8")
            original = original_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["slot metadata temporary output changed before cleanup"],
        )
        self.assertEqual(replacement, "do not remove\n")
        self.assertEqual(original, "original\n")

    def test_kagemusha_slot_assembler_json_write_verifies_installed_bytes(
        self,
    ) -> None:
        original_replace = slot_assembler.os.replace

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "slot.json"

            def tampering_replace(source: Path, target: Path) -> None:
                original_replace(source, target)
                target.write_text('{"schema":"tampered"}\n', encoding="utf-8")

            with mock.patch.object(
                slot_assembler.os,
                "replace",
                side_effect=tampering_replace,
            ):
                errors = slot_assembler._write_json(
                    output,
                    {"schema": "test"},
                    "slot metadata",
                )

        self.assertEqual(errors, ["slot metadata changed after write"])

    def test_kagemusha_slot_assembler_copy_rejects_parent_identity_swap(
        self,
    ) -> None:
        original_open = slot_assembler.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            source = wrapper / "source.apk"
            source.write_bytes(b"source apk bytes")
            destination = wrapper / "slot" / "evidence" / "offline-wallet-release.apk"
            swapped_parent = wrapper / "evidence-swapped"
            errors: list[str] = []
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == destination.parent and not swapped:
                    destination.parent.rename(swapped_parent)
                    destination.parent.mkdir(parents=True)
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(slot_assembler.os, "open", swapping_parent_open):
                digest = slot_assembler._copy_source_file(
                    source=source,
                    destination=destination,
                    label="offline wallet release APK source",
                    errors=errors,
                )
            copied_bytes = (swapped_parent / destination.name).read_bytes()

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "offline wallet release APK source parent directory changed before sync"
            ],
        )
        self.assertEqual(copied_bytes, b"source apk bytes")

    def test_kagemusha_slot_assembler_copy_rejects_control_source_path_before_copy(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            source = wrapper / "control\nsource.apk"
            source.write_bytes(b"source apk bytes")
            destination = wrapper / "slot" / "evidence" / "offline-wallet-release.apk"
            errors: list[str] = []

            digest = slot_assembler._copy_source_file(
                source=source,
                destination=destination,
                label="offline wallet release APK source",
                errors=errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["offline wallet release APK source path must not contain control characters"],
        )
        self.assertFalse(destination.exists())
        self.assertFalse(destination.parent.exists())
        self.assertNotIn(str(source), rendered)

    def test_kagemusha_slot_assembler_copy_verifies_installed_bytes(
        self,
    ) -> None:
        original_sync = slot_assembler._sync_directory

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            source = wrapper / "source.apk"
            source.write_bytes(b"source apk bytes")
            destination = wrapper / "slot" / "evidence" / "offline-wallet-release.apk"
            errors: list[str] = []

            def tampering_sync(path: Path, label: str, *, expected_identity):
                sync_errors = original_sync(
                    path,
                    label,
                    expected_identity=expected_identity,
                )
                if not sync_errors:
                    destination.write_bytes(b"tampered apk bytes")
                return sync_errors

            with mock.patch.object(
                slot_assembler,
                "_sync_directory",
                side_effect=tampering_sync,
            ):
                digest = slot_assembler._copy_source_file(
                    source=source,
                    destination=destination,
                    label="offline wallet release APK source",
                    errors=errors,
                )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["offline wallet release APK source changed after write"],
        )

    def test_kagemusha_slot_assembler_requires_attestation_harness_result(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            (source_slot / "attestation" / "harness-result.json").unlink()
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn("attestation harness result source is missing", stderr.getvalue())
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_harness_challenge_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            harness_path = source_slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["challenge_hex"] = "00"
            write_json(harness_path, harness)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result challenge_hex digest must match "
            "attestation/result.json attestation_challenge_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_blank_source_challenge_before_unsigned_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            for relative in (
                "attestation/result.json",
                "attestation/report.json",
            ):
                path = source_slot / relative
                payload = json.loads(path.read_text(encoding="utf-8"))
                payload["attestation_challenge_sha256"] = " "
                write_json(path, payload)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json attestation_challenge_sha256 must be a non-empty string",
            rendered,
        )
        self.assertIn(
            "attestation/report.json attestation_challenge_sha256 must be a non-empty string",
            rendered,
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_noncanonical_source_policy_before_unsigned_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            result_path = source_slot / "attestation" / "result.json"
            result = json.loads(result_path.read_text(encoding="utf-8"))
            result["offline_wallet_policy_sha256"] = "AA" * 32
            write_json(result_path, result)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json offline_wallet_policy_sha256 must be lowercase sha256 hex",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_report_level_mismatch_before_publish(
        self,
    ) -> None:
        for level_key in (
            "keymint_security_level",
            "attestation_security_level",
            "keymaster_security_level",
        ):
            with self.subTest(level_key=level_key):
                with tempfile.TemporaryDirectory() as temp:
                    source_signer = create_test_signer(Path(temp) / "source-keys")
                    source_slot = create_slot(
                        Path(temp) / "source",
                        "pixel6",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                        source_signer,
                    )
                    report_path = source_slot / "attestation" / "report.json"
                    report = json.loads(report_path.read_text(encoding="utf-8"))
                    report["verification"][level_key] = "STRONG_BOX"
                    write_json(report_path, report)
                    slot_root = Path(temp) / "device-lab"

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = slot_assembler.main(
                            slot_assembler_args(
                                slot_root=slot_root,
                                source_slot=source_slot,
                            )
                            + ["--allow-unsigned"]
                        )

                self.assertEqual(status, 1)
                self.assertIn(
                    f"attestation/report.json verification.{level_key} must match "
                    f"attestation/result.json {level_key}",
                    stderr.getvalue(),
                )
                self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_report_status_mismatch_before_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            report_path = source_slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["verification"]["status"] = "passed"
            write_json(report_path, report)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json verification.status must match "
            "attestation/result.json status",
            stderr.getvalue(),
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_passed_attestation_status_before_publish(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            result_path = source_slot / "attestation" / "result.json"
            result = json.loads(result_path.read_text(encoding="utf-8"))
            result["status"] = "passed"
            write_json(result_path, result)
            report_path = source_slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["verification"]["status"] = "passed"
            write_json(report_path, report)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                    )
                    + ["--allow-unsigned"]
                )

        self.assertEqual(status, 1)
        errors = stderr.getvalue()
        self.assertIn("attestation/result.json status must be ok", errors)
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            errors,
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_slot_assembler_rejects_noncanonical_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source_signer = create_test_signer(Path(temp) / "source-keys")
            signer = create_test_signer(Path(temp) / "slot-keys")
            source_slot = create_slot(
                Path(temp) / "source",
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                source_signer,
            )
            harness_path = source_slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["alias"] = " android-keystore-alias "
            harness["attestation_security_level"] = " strong_box "
            harness["keymaster_security_level"] = "strongbox"
            harness["challenge_hex"] = "01 02 03 04"
            write_json(harness_path, harness)
            slot_root = Path(temp) / "device-lab"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = slot_assembler.main(
                    slot_assembler_args(
                        slot_root=slot_root,
                        source_slot=source_slot,
                        signer=signer,
                    )
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result alias must not have surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "attestation harness result attestation_security_level must not have surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "attestation harness result keymaster_security_level must be STRONGBOX",
            rendered,
        )
        self.assertIn(
            "attestation harness result challenge_hex must be lowercase hexadecimal without whitespace",
            rendered,
        )
        self.assertFalse((slot_root / "pixel6").exists())

    def test_kagemusha_attestation_report_writer_emits_slot_bound_report(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            chain_digest = write_attestation_chain(chain_path)

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )
            report = json.loads(out_path.read_text(encoding="utf-8"))

        challenge_digest = hashlib.sha256(bytes.fromhex("4145454245")).hexdigest()
        self.assertEqual(status, 0)
        self.assertIn(str(out_path), stdout.getvalue())
        self.assertEqual(report["schema"], device_lab.ATTESTATION_REPORT_SCHEMA)
        self.assertEqual(report["slot_id"], "pixel6")
        self.assertEqual(
            report["app_package_name"],
            "org.hyperledger.iroha.sdk.offline.wallet.lab",
        )
        self.assertEqual(report["attestation_challenge_sha256"], challenge_digest)
        self.assertEqual(
            report["attestation_certificate_chain_path"],
            "attestation/keymint-certificate-chain.pem",
        )
        self.assertEqual(report["attestation_certificate_chain_sha256"], chain_digest)
        self.assertEqual(
            report["verification"],
            {
                "attestation_security_level": "STRONGBOX",
                "keymaster_security_level": "STRONGBOX",
                "keymint_security_level": "STRONGBOX",
                "physical_device_attestation": True,
                "status": "ok",
                "strongbox_attestation": True,
            },
        )

    def test_kagemusha_attestation_report_writer_rejects_parent_directory_identity_swap_before_sync(
        self,
    ) -> None:
        original_open = attestation_report.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "attestation-report-root"
            root.mkdir()
            out_path = root / "report.json"
            swapped_root = wrapper / "attestation-report-root-swapped"
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == out_path.parent and not swapped:
                    out_path.parent.rename(swapped_root)
                    out_path.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(attestation_report.os, "open", swapping_parent_open):
                errors = attestation_report.write_report(out_path, {"schema": "test"})
            report_text = (swapped_root / out_path.name).read_text(encoding="utf-8")

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["attestation report output parent directory changed before sync"],
        )
        self.assertEqual(report_text, '{\n  "schema": "test"\n}\n')

    def test_kagemusha_attestation_report_writer_temp_cleanup_rejects_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".report.json.swap.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = device_lab._file_identity(temp_path.lstat())
            swapped_temp = Path(temp) / "original-report-temp-file"
            temp_path.rename(swapped_temp)
            temp_path.write_text("do not remove\n", encoding="utf-8")

            errors = attestation_report._cleanup_temp_output(
                temp_path,
                "attestation report output",
                temp_identity,
            )
            victim_survived = temp_path.read_text(encoding="utf-8")
            original_survived = swapped_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["attestation report output temporary file changed before cleanup"],
        )
        self.assertEqual(victim_survived, "do not remove\n")
        self.assertEqual(original_survived, "original\n")

    def test_kagemusha_attestation_report_writer_requires_physical_device_assertion(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)
            args = attestation_report_args(
                harness_result=result_path,
                chain=chain_path,
                out=out_path,
            )
            args.remove("--physical-device-attestation")

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "physical device attestation must be explicitly asserted",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_non_strongbox_result(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(
                result_path,
                keymaster_security_level="TEE",
                strongbox_attestation=False,
            )
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result keymaster_security_level must be STRONGBOX",
            stderr.getvalue(),
        )
        self.assertIn(
            "attestation harness result strongbox_attestation must be true",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_noncanonical_strongbox_levels(
        self,
    ) -> None:
        cases = (
            (
                {"attestation_security_level": " strong_box ", "keymaster_security_level": "STRONGBOX"},
                "attestation harness result attestation_security_level must not contain whitespace",
            ),
            (
                {"attestation_security_level": "strongbox", "keymaster_security_level": "STRONGBOX"},
                "attestation harness result attestation_security_level must be STRONGBOX",
            ),
            (
                {"attestation_security_level": "STRONGBOX", "keymaster_security_level": "strong_box"},
                "attestation harness result keymaster_security_level must be STRONGBOX",
            ),
        )
        for levels, expected in cases:
            with self.subTest(levels=levels):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(result_path, **levels)
                    write_attestation_chain(chain_path)

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(
                            attestation_report_args(
                                harness_result=result_path,
                                chain=chain_path,
                                out=out_path,
                            )
                        )

                self.assertEqual(status, 1)
                self.assertIn(expected, stderr.getvalue())
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_chain_length_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path, chain_length=3)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result chain_length must match "
            "attestation certificate-chain certificate count",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_short_pem_chain(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path, chain_length=2)
            chain_path.write_text(
                "-----BEGIN CERTIFICATE-----\n"
                "slot-bound-strongbox-keymint-certificate-leaf\n"
                "-----END CERTIFICATE-----\n",
                encoding="utf-8",
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation certificate chain PEM must contain at least two certificates",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_challenge_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=["--expected-challenge-hex", "00"],
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result challenge_hex must match --expected-challenge-hex",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_noncanonical_harness_challenge(
        self,
    ) -> None:
        for challenge_hex in ("01 02 03 04", "ABCDEF00"):
            with self.subTest(challenge_hex=challenge_hex):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(
                        result_path,
                        challenge_hex=challenge_hex,
                    )
                    write_attestation_chain(chain_path)

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(
                            attestation_report_args(
                                harness_result=result_path,
                                chain=chain_path,
                                out=out_path,
                            )
                        )

                self.assertEqual(status, 1)
                self.assertIn(
                    "attestation harness result challenge_hex must be "
                    "lowercase hexadecimal without whitespace",
                    stderr.getvalue(),
                )
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_noncanonical_expected_challenge(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path, challenge_hex="01020304")
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=["--expected-challenge-hex", "01 02 03 04"],
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "--expected-challenge-hex must be lowercase hexadecimal without whitespace",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_whitespace_identity_args(
        self,
    ) -> None:
        cases = (
            ("--slot-id", " pixel6 ", "slot id must not contain whitespace", None),
            (
                "--device-fingerprint",
                " google/oriole/oriole:16/CP1A.260405.005/15001963:user/release-keys ",
                "device fingerprint must not contain whitespace",
                None,
            ),
            ("--os-build-id", " CP1A.260405.005 ", "os build id must not contain whitespace", None),
            (
                "--app-package-name",
                " org.hyperledger.iroha.sdk.offline.wallet.lab ",
                "app package name must not contain whitespace",
                "append",
            ),
            (
                "--verifier",
                " android-keystore-attestation-harness ",
                "verifier must not contain whitespace",
                "append",
            ),
        )
        for option, value, expected, mode in cases:
            with self.subTest(option=option):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(result_path)
                    write_attestation_chain(chain_path)
                    args = attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                    if mode == "append":
                        args.extend([option, value])
                    else:
                        args[args.index(option) + 1] = value

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(args)

                self.assertEqual(status, 1)
                self.assertIn(expected, stderr.getvalue())
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_noncanonical_slot_id(
        self,
    ) -> None:
        cases = ("./pixel6", "pixel6/", "pixel6/.")
        for slot_id in cases:
            with self.subTest(slot_id=slot_id):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(result_path)
                    write_attestation_chain(chain_path)
                    args = attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                    args[args.index("--slot-id") + 1] = slot_id

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(args)

                self.assertEqual(status, 1)
                self.assertIn(
                    "slot id must be a canonical single directory name",
                    stderr.getvalue(),
                )
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_backslash_slot_id(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)
            args = attestation_report_args(
                harness_result=result_path,
                chain=chain_path,
                out=out_path,
            )
            args[args.index("--slot-id") + 1] = "pixel\\6"

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(args)

        self.assertEqual(status, 1)
        self.assertIn(
            "slot id must be a single safe directory name",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_control_identity_args(
        self,
    ) -> None:
        cases = (
            (
                "--slot-id",
                "pixel6\x1b[31m",
                "slot id must not contain control characters",
                None,
            ),
            (
                "--device-fingerprint",
                "google/oriole/oriole:16/CP1A.260405.005/15001963:user/release-keys\x1b[31m",
                "device fingerprint must not contain control characters",
                None,
            ),
            (
                "--os-build-id",
                "CP1A.260405.005\x1b[31m",
                "os build id must not contain control characters",
                None,
            ),
            (
                "--app-package-name",
                "org.hyperledger.iroha\x1b[31m",
                "app package name must not contain control characters",
                "append",
            ),
            (
                "--verifier",
                "android-keystore-attestation-harness\x1b[31m",
                "verifier must not contain control characters",
                "append",
            ),
        )
        for option, value, expected, mode in cases:
            with self.subTest(option=option):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(result_path)
                    write_attestation_chain(chain_path)
                    args = attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                    if mode == "append":
                        args.extend([option, value])
                    else:
                        args[args.index(option) + 1] = value

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(args)
                    rendered = stderr.getvalue()

                self.assertEqual(status, 1)
                self.assertIn(expected, rendered)
                self.assertNotIn(value, rendered)
                self.assertNotIn("\x1b", rendered)
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_control_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(
                result_path,
                challenge_hex="4145454245\x1b[31m",
                attestation_security_level="STRONGBOX\x1b[31m",
                keymaster_security_level="STRONGBOX\x1b[31m",
                extra={"alias": "strongbox-alias\x1b[31m"},
            )
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result alias must not contain control characters",
            rendered,
        )
        self.assertIn(
            "attestation harness result attestation_security_level must not contain "
            "control characters",
            rendered,
        )
        self.assertIn(
            "attestation harness result keymaster_security_level must not contain "
            "control characters",
            rendered,
        )
        self.assertIn(
            "attestation harness result challenge_hex must not contain control characters",
            rendered,
        )
        self.assertNotIn("\x1b", rendered)
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_control_expected_challenge(
        self,
    ) -> None:
        expected_challenge = "4145454245\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=["--expected-challenge-hex", expected_challenge],
                    )
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "--expected-challenge-hex must not contain control characters",
            rendered,
        )
        self.assertNotIn(expected_challenge, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_unexpected_result_fields(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(
                result_path,
                extra={"debug": "operator-local"},
            )
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result contains unexpected field debug",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_redacts_control_unexpected_result_field(
        self,
    ) -> None:
        unsafe_key = "debug\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(
                result_path,
                extra={unsafe_key: "operator-local"},
            )
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                    )
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation harness result contains unexpected field "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn(unsafe_key, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_chain_path_control(
        self,
    ) -> None:
        unsafe_path = "attestation/keymint-certificate-chain.pem\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=[
                            "--attestation-certificate-chain-path",
                            unsafe_path,
                        ],
                    )
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation certificate chain path must not contain control characters",
            rendered,
        )
        self.assertNotIn(unsafe_path, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_control_chain_source_path_before_ancestor_check(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            chain_path = Path(temp) / "chain\x1b[31m" / "keymint-certificate-chain.pem"
            errors: list[str] = []

            with mock.patch.object(
                attestation_report.device_lab,
                "validate_no_symlink_ancestors",
                side_effect=AssertionError(
                    "control chain source path should fail before ancestor checks"
                ),
            ):
                chain_data, chain_digest = (  # type: ignore[attr-defined]
                    attestation_report._read_validated_chain(chain_path, errors)
                )

        rendered = "\n".join(errors)
        self.assertIsNone(chain_data)
        self.assertIsNone(chain_digest)
        self.assertEqual(
            errors,
            ["attestation certificate chain path must not contain control characters"],
        )
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_attestation_report_writer_rejects_alias_chain_source_path_before_metadata(
        self,
    ) -> None:
        path_type = type(Path("."))
        cases = (
            (
                Path("chain") / ".." / "keymint-certificate-chain.pem",
                "attestation certificate chain path must be canonical",
            ),
            (
                Path("chain\\keymint-certificate-chain.pem"),
                "attestation certificate chain path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for chain_path, expected_error in cases:
                chain_path = root / chain_path
                with self.subTest(chain_path=chain_path):
                    errors: list[str] = []
                    with mock.patch.object(
                        attestation_report.device_lab,
                        "validate_no_symlink_ancestors",
                        side_effect=AssertionError(
                            "alias chain source path should fail before ancestor checks"
                        ),
                    ), mock.patch.object(
                        path_type,
                        "lstat",
                        side_effect=AssertionError(
                            "alias chain source path should fail before metadata reads"
                        ),
                    ):
                        chain_data, chain_digest = (  # type: ignore[attr-defined]
                            attestation_report._read_validated_chain(
                                chain_path,
                                errors,
                            )
                        )
                    rendered = "\n".join(errors)

                    self.assertIsNone(chain_data)
                    self.assertIsNone(chain_digest)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(chain_path), rendered)

    def test_kagemusha_attestation_report_writer_rejects_alias_harness_result_path_before_metadata(
        self,
    ) -> None:
        path_type = type(Path("."))
        cases = (
            (
                Path("harness") / ".." / "result.json",
                "attestation harness result path must be canonical",
            ),
            (
                Path("harness\\result.json"),
                "attestation harness result path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for harness_path, expected_error in cases:
                harness_path = root / harness_path
                with self.subTest(harness_path=harness_path):
                    errors: list[str] = []
                    with mock.patch.object(
                        attestation_report.device_lab,
                        "validate_no_symlink_ancestors",
                        side_effect=AssertionError(
                            "alias harness result path should fail before ancestor checks"
                        ),
                    ), mock.patch.object(
                        path_type,
                        "lstat",
                        side_effect=AssertionError(
                            "alias harness result path should fail before metadata reads"
                        ),
                    ):
                        result = attestation_report._load_harness_result(  # type: ignore[attr-defined]
                            harness_path,
                            errors,
                        )
                    rendered = "\n".join(errors)

                    self.assertIsNone(result)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(harness_path), rendered)

    def test_kagemusha_attestation_report_writer_rejects_secret_harness_result_path_without_leak(
        self,
    ) -> None:
        path_type = type(Path("."))
        with tempfile.TemporaryDirectory() as temp:
            harness_path = Path(temp) / "token=supersecret" / "result.json"
            errors: list[str] = []
            with mock.patch.object(
                attestation_report.device_lab,
                "validate_no_symlink_ancestors",
                side_effect=AssertionError(
                    "secret harness result path should fail before ancestor checks"
                ),
            ), mock.patch.object(
                path_type,
                "lstat",
                side_effect=AssertionError(
                    "secret harness result path should fail before metadata reads"
                ),
            ):
                result = attestation_report._load_harness_result(  # type: ignore[attr-defined]
                    harness_path,
                    errors,
                )

        rendered = "\n".join(errors)
        self.assertIsNone(result)
        self.assertEqual(
            errors,
            ["attestation harness result path must not contain secret-looking material"],
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_attestation_report_writer_rejects_chain_path_escape(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=[
                            "--attestation-certificate-chain-path",
                            "../attestation/keymint-certificate-chain.pem",
                        ],
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation certificate chain path must stay under attestation/",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_noncanonical_chain_path(
        self,
    ) -> None:
        cases = (
            "attestation/./keymint-certificate-chain.pem",
            "attestation//keymint-certificate-chain.pem",
            "attestation/keymint-certificate-chain.pem/",
        )
        for chain_relative in cases:
            with self.subTest(chain_relative=chain_relative):
                with tempfile.TemporaryDirectory() as temp:
                    temp_path = Path(temp)
                    result_path = temp_path / "result.json"
                    chain_path = temp_path / "keymint-certificate-chain.pem"
                    out_path = temp_path / "report.json"
                    write_attestation_harness_result(result_path)
                    write_attestation_chain(chain_path)

                    stderr = io.StringIO()
                    with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                        status = attestation_report.main(
                            attestation_report_args(
                                harness_result=result_path,
                                chain=chain_path,
                                out=out_path,
                                extra=[
                                    "--attestation-certificate-chain-path",
                                    chain_relative,
                                ],
                            )
                        )

                self.assertEqual(status, 1)
                self.assertIn(
                    "attestation certificate chain path must be canonical",
                    stderr.getvalue(),
                )
                self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_backslash_chain_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=[
                            "--attestation-certificate-chain-path",
                            "attestation/keymint\\certificate-chain.pem",
                        ],
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation certificate chain path must not contain backslashes",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_attestation_report_writer_rejects_chain_path_whitespace(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            result_path = temp_path / "result.json"
            chain_path = temp_path / "keymint-certificate-chain.pem"
            out_path = temp_path / "report.json"
            write_attestation_harness_result(result_path)
            write_attestation_chain(chain_path)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = attestation_report.main(
                    attestation_report_args(
                        harness_result=result_path,
                        chain=chain_path,
                        out=out_path,
                        extra=[
                            "--attestation-certificate-chain-path",
                            " attestation/keymint-certificate-chain.pem ",
                        ],
                    )
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation certificate chain path must not contain whitespace",
            stderr.getvalue(),
        )
        self.assertFalse(out_path.exists())

    def test_kagemusha_android_raw_puller_reads_latest_and_installs_slot(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            summary_out = temp_path / "pull-summary.json"
            tar_bytes = raw_slot_tar_bytes("pixel6")
            runner = fake_raw_pull_runner(tar_bytes, "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root, summary_out=summary_out),
                runner=runner,
            )
            summary = json.loads(summary_out.read_text(encoding="utf-8"))
            latest_text = (out_root / "latest-slot.txt").read_text(encoding="utf-8")
            result_exists = (
                out_root / "pixel6" / "attestation" / "result.json"
            ).is_file()
            harness_exists = (
                out_root / "pixel6" / "attestation" / "harness-result.json"
            ).is_file()

        self.assertEqual(status, 0, errors)
        self.assertEqual(slot_path, out_root / "pixel6")
        self.assertEqual(latest_text, "pixel6\n")
        self.assertTrue(result_exists)
        self.assertTrue(harness_exists)
        self.assertEqual(summary["schema"], raw_puller.RAW_PULL_SUMMARY_SCHEMA)
        self.assertEqual(summary["slot_id"], "pixel6")
        self.assertEqual(
            set(summary["artifact_sha256"]),
            set(raw_puller.RAW_SLOT_REQUIRED_PATHS),
        )
        self.assertIn("attestation/harness-result.json", summary["artifact_sha256"])
        self.assertIn("attestation/result.json", summary["artifact_sha256"])
        self.assertTrue(any("cat" in call for call in runner.calls))  # type: ignore[attr-defined]
        self.assertTrue(any("exec-out" in call for call in runner.calls))  # type: ignore[attr-defined]

    def test_kagemusha_android_raw_puller_rejects_noncanonical_slot_id_before_adb(
        self,
    ) -> None:
        def forbidden_runner(command: list[str], **kwargs):
            raise AssertionError("raw puller must reject slot id before ADB")

        for slot_id in ("./pixel6", "pixel6/", "pixel6/."):
            with self.subTest(slot_id=slot_id):
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root, slot_id=slot_id),
                        runner=forbidden_runner,
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertEqual(
                    errors,
                    [
                        f"slot id {slot_id!r} must be a canonical single directory name"
                    ],
                )
                self.assertFalse(out_root.exists())

    def test_kagemusha_android_raw_puller_rejects_noncanonical_adb_arguments_before_command(
        self,
    ) -> None:
        cases = (
            (
                "adb",
                " adb ",
                "adb executable must not contain surrounding whitespace",
            ),
            (
                "run_as_package",
                f" {raw_puller.DEFAULT_RUN_AS_PACKAGE}",
                "run-as package must not contain surrounding whitespace",
            ),
            (
                "device_lab_root",
                raw_puller.DEFAULT_DEVICE_LAB_DEVICE_ROOT + "\x1b",
                "device lab root must not contain control characters",
            ),
            (
                "serial",
                "ABC123\x00",
                "ADB serial must not contain control characters",
            ),
            (
                "run_as_package",
                "token=supersecret",
                "run-as package must not contain secret-looking material",
            ),
        )
        for attr, value, expected_error in cases:
            with self.subTest(attr=attr, expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    args = raw_pull_args(Path(temp) / "raw")
                    setattr(args, attr, value)
                    runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6")

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        args,
                        runner=runner,
                    )

                rendered = "\n".join(errors)
                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)
                self.assertNotIn("\x00", rendered)
                self.assertNotIn("\x1b", rendered)
                self.assertEqual(runner.calls, [])  # type: ignore[attr-defined]

    def test_kagemusha_android_raw_puller_rejects_control_out_root_before_adb(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw\x1b[31m"
            args = raw_pull_args(out_root, slot_id="pixel6")
            runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                args,
                runner=runner,
            )

            out_root_exists = out_root.exists()

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw output root path must not contain control characters",
            errors,
        )
        self.assertFalse(out_root_exists)
        self.assertEqual(runner.calls, [])  # type: ignore[attr-defined]
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_rejects_alias_cli_paths_before_adb(
        self,
    ) -> None:
        cases = (
            (
                "out-root-backslash",
                lambda base: (base / "raw\\alias", None),
                "raw output root path must not contain backslashes",
            ),
            (
                "out-root-parent",
                lambda base: (base / "raw" / ".." / "alias", None),
                "raw output root path must be canonical",
            ),
            (
                "summary-backslash",
                lambda base: (base / "raw", base / "summary\\alias.json"),
                "raw pull summary output must not contain backslashes",
            ),
            (
                "summary-parent",
                lambda base: (base / "raw", base / "summary" / ".." / "alias.json"),
                "raw pull summary output must be canonical",
            ),
        )
        for name, paths, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    out_root, summary_out = paths(Path(temp))
                    runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6")

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(
                            out_root,
                            slot_id="pixel6",
                            summary_out=summary_out,
                        ),
                        runner=runner,
                    )

                    out_root_exists = out_root.exists()
                    summary_exists = summary_out.exists() if summary_out is not None else False

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertEqual(errors, [expected_error])
                self.assertFalse(out_root_exists)
                self.assertFalse(summary_exists)
                self.assertEqual(runner.calls, [])  # type: ignore[attr-defined]

    def test_kagemusha_android_raw_puller_rejects_control_summary_out_before_adb(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            summary_out = Path(temp) / "summary\x1b[31m.json"
            runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root, summary_out=summary_out),
                runner=runner,
            )

            out_root_exists = out_root.exists()

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw pull summary output must not contain control characters",
            errors,
        )
        self.assertFalse(out_root_exists)
        self.assertEqual(runner.calls, [])  # type: ignore[attr-defined]
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_rejects_control_raw_slot_path_before_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot_path = Path(temp) / "pixel6\x1b[31m"
            latest_path = Path(temp) / "latest-slot.txt"

            with mock.patch.object(
                Path,
                "lstat",
                side_effect=AssertionError("raw slot path should not be statted"),
            ):
                errors = raw_puller._validate_raw_slot_files(  # type: ignore[attr-defined]
                    slot_path,
                    "pixel6",
                    latest_path,
                )

        rendered = "\n".join(errors)
        self.assertEqual(errors, ["raw slot path must not contain control characters"])
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_rejects_alias_raw_slot_path_before_stat(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "pixel6\\alias",
                "raw slot path must not contain backslashes",
            ),
            (
                lambda base: base / "pixel6" / ".." / "alias",
                "raw slot path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    slot_path = path_factory(Path(temp))
                    latest_path = Path(temp) / "latest-slot.txt"

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError("raw slot path should not be statted"),
                    ):
                        errors = raw_puller._validate_raw_slot_files(  # type: ignore[attr-defined]
                            slot_path,
                            "pixel6",
                            latest_path,
                        )

                self.assertEqual(errors, [expected_error])

    def test_kagemusha_android_raw_puller_redacts_control_raw_artifact_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            slot_path = write_raw_stage_slot(temp_path, "pixel6")
            latest_path = temp_path / "latest-slot.txt"
            latest_path.write_text("pixel6\n", encoding="utf-8")
            unsafe_artifact = slot_path / "logs" / "debug\x1b[31m.log"
            unsafe_artifact.write_text("debug\n", encoding="utf-8")

            errors = raw_puller._validate_raw_slot_files(  # type: ignore[attr-defined]
                slot_path,
                "pixel6",
                latest_path,
            )

        rendered = "\n".join(errors)
        self.assertIn(
            "raw slot artifact paths must not contain control characters",
            errors,
        )
        self.assertNotIn("debug\x1b[31m.log", rendered)
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_rejects_control_tar_member_path_before_normalise(
        self,
    ) -> None:
        errors: list[str] = []
        with mock.patch.object(
            raw_puller,
            "PurePosixPath",
            side_effect=AssertionError(
                "control tar member paths should fail before normalisation"
            ),
        ):
            normalised = raw_puller._normalise_tar_member_name(  # type: ignore[attr-defined]
                "pixel6/logs/debug\x1b[31m.log",
                errors,
            )

        rendered = "\n".join(errors)
        self.assertIsNone(normalised)
        self.assertEqual(
            errors,
            ["raw slot tar member path must not contain control characters"],
        )
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_redacts_control_latest_adb_stderr(
        self,
    ) -> None:
        def runner(command: list[str], **_kwargs):  # type: ignore[no-untyped-def]
            return subprocess.CompletedProcess(
                command,
                1,
                stdout="",
                stderr="adb failed\x1b[31m",
            )

        with tempfile.TemporaryDirectory() as temp:
            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(Path(temp) / "raw"),
                runner=runner,
            )

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(raw_puller.CONTROL_OUTPUT_REDACTION, rendered)
        self.assertNotIn("adb failed\x1b[31m", rendered)
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_redacts_control_tar_adb_stderr(
        self,
    ) -> None:
        def runner(command: list[str], **_kwargs):  # type: ignore[no-untyped-def]
            if "cat" in command:
                return subprocess.CompletedProcess(
                    command,
                    0,
                    stdout="pixel6\n",
                    stderr="",
                )
            return subprocess.CompletedProcess(
                command,
                1,
                stdout=b"",
                stderr=b"tar failed\x1b[31m",
            )

        with tempfile.TemporaryDirectory() as temp:
            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(Path(temp) / "raw"),
                runner=runner,
            )

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(raw_puller.CONTROL_OUTPUT_REDACTION, rendered)
        self.assertNotIn("tar failed\x1b[31m", rendered)
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_install_refuses_late_existing_slot(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            final_slot.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")

            errors = raw_puller._install_validated_slot(
                stage_slot,
                final_slot,
                out_root,
            )
            final_slot_is_dir = final_slot.is_dir()

        self.assertEqual(
            errors,
            ["slot directory already exists; refuse to overwrite raw evidence"],
        )
        self.assertTrue(final_slot_is_dir)

    def test_kagemusha_android_raw_puller_install_rejects_unexpected_top_level_entry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            (stage_slot / "surprise").mkdir()

            errors = raw_puller._install_validated_slot(
                stage_slot,
                final_slot,
                out_root,
            )
            final_slot_exists = final_slot.exists()

        self.assertEqual(
            errors,
            ["raw slot install source contains unexpected top-level entry surprise"],
        )
        self.assertFalse(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_redacts_secret_top_level_entry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            (stage_slot / "token=supersecret").mkdir()

            errors = raw_puller._install_validated_slot(
                stage_slot,
                final_slot,
                out_root,
            )
            final_slot_exists = final_slot.exists()
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            [
                "raw slot install source contains unexpected top-level entry "
                f"{device_lab.SECRET_PATH_REDACTION}"
            ],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertFalse(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_redacts_control_top_level_entry(
        self,
    ) -> None:
        unsafe_name = "debug\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            (stage_slot / unsafe_name).mkdir()

            errors = raw_puller._install_validated_slot(
                stage_slot,
                final_slot,
                out_root,
            )
            final_slot_exists = final_slot.exists()
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            [
                "raw slot install source contains unexpected top-level entry "
                f"{device_lab.CONTROL_PATH_REDACTION}"
            ],
        )
        self.assertNotIn(unsafe_name, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertFalse(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_syncs_directories_and_cleans_failure(
        self,
    ) -> None:
        original_sync_directory = raw_puller._sync_directory
        sync_calls: list[tuple[Path, str, tuple[int, int] | None]] = []

        def fake_sync_directory(
            path: Path,
            error: str,
            *,
            expected_identity: tuple[int, int] | None = None,
        ) -> list[str]:
            sync_calls.append((path, error, expected_identity))
            if error == "raw slot directory parent could not be synced":
                return [error]
            return []

        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            raw_puller._sync_directory = fake_sync_directory
            try:
                errors = raw_puller._install_validated_slot(
                    stage_slot,
                    final_slot,
                    out_root,
                )
            finally:
                raw_puller._sync_directory = original_sync_directory
            final_slot_exists = final_slot.exists()

        self.assertEqual(errors, ["raw slot directory parent could not be synced"])
        self.assertIsNotNone(sync_calls[0][2])
        self.assertIsNotNone(sync_calls[1][2])
        self.assertEqual(
            [(path, error) for path, error, _identity in sync_calls],
            [
                (final_slot, "raw slot directory could not be synced"),
                (out_root, "raw slot directory parent could not be synced"),
            ],
        )
        self.assertFalse(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_rejects_destination_identity_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp)
            parent_identity = raw_puller._file_identity(parent.lstat())
            final_slot = parent / "pixel6"
            final_slot.mkdir()
            original_identity, identity_errors = raw_puller._slot_entry_identity(
                final_slot,
                parent,
                parent_identity,
            )
            self.assertEqual(identity_errors, [])
            assert original_identity is not None
            final_slot.rmdir()
            final_slot.mkdir()

            errors = raw_puller._created_slot_identity_errors(
                final_slot,
                original_identity,
                parent,
                parent_identity,
            )

        self.assertEqual(errors, ["raw slot directory changed during install"])

    def test_kagemusha_android_raw_puller_install_rejects_output_root_identity_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output_root = Path(temp) / "raw"
            output_root.mkdir()
            original_identity = raw_puller._file_identity(output_root.lstat())
            output_root.rmdir()
            output_root.mkdir()

            errors = raw_puller._sync_directory(
                output_root,
                "raw slot directory parent could not be synced",
                expected_identity=original_identity,
            )

        self.assertEqual(errors, ["raw slot directory parent could not be synced"])

    def test_kagemusha_android_raw_puller_install_rejects_parent_identity_before_slot_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp) / "raw"
            parent.mkdir()
            original_identity = raw_puller._file_identity(parent.lstat())
            final_slot = parent / "pixel6"
            final_slot.mkdir()
            final_identity = raw_puller._file_identity(final_slot.lstat())
            final_slot.rmdir()
            parent.rmdir()
            parent.mkdir()
            (parent / "pixel6").mkdir()

            errors = raw_puller._created_slot_identity_errors(
                parent / "pixel6",
                final_identity,
                parent,
                original_identity,
            )

        self.assertEqual(errors, ["raw output root directory changed during install"])

    def test_kagemusha_android_raw_puller_install_cleanup_preserves_swapped_destination(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp)
            parent_identity = raw_puller._file_identity(parent.lstat())
            final_slot = parent / "pixel6"
            final_slot.mkdir()
            original_identity = raw_puller._file_identity(final_slot.lstat())
            final_slot.rmdir()
            final_slot.mkdir()

            raw_puller._remove_created_slot(
                final_slot,
                original_identity,
                parent,
                parent_identity,
            )
            final_slot_exists = final_slot.is_dir()

        self.assertTrue(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_cleanup_uses_parent_dir_fd(
        self,
    ) -> None:
        original_rmtree = raw_puller.shutil.rmtree
        rmtree_calls: list[tuple[str, object]] = []

        def fake_rmtree(path, *, dir_fd=None):  # type: ignore[no-untyped-def]
            rmtree_calls.append((path, dir_fd))

        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp)
            parent_identity = raw_puller._file_identity(parent.lstat())
            final_slot = parent / "pixel6"
            final_slot.mkdir()
            final_identity = raw_puller._file_identity(final_slot.lstat())
            raw_puller.shutil.rmtree = fake_rmtree
            try:
                raw_puller._remove_created_slot(
                    final_slot,
                    final_identity,
                    parent,
                    parent_identity,
                )
            finally:
                raw_puller.shutil.rmtree = original_rmtree

        self.assertEqual(len(rmtree_calls), 1)
        self.assertEqual(rmtree_calls[0][0], "pixel6")
        self.assertIsNotNone(rmtree_calls[0][1])

    def test_kagemusha_android_raw_puller_install_cleanup_reports_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            (stage_slot / "surprise").mkdir()

            with mock.patch.object(
                raw_puller.shutil,
                "rmtree",
                side_effect=OSError("simulated partial install cleanup failure"),
            ):
                errors = raw_puller._install_validated_slot(
                    stage_slot,
                    final_slot,
                    out_root,
                )
            final_slot_exists = final_slot.is_dir()

        self.assertEqual(
            errors,
            [
                "raw slot install source contains unexpected top-level entry surprise",
                "raw slot partial install could not be removed",
            ],
        )
        self.assertTrue(final_slot_exists)

    def test_kagemusha_android_raw_puller_install_moves_with_directory_fds(
        self,
    ) -> None:
        original_rename = raw_puller.os.rename
        rename_calls: list[tuple[str, str, int | None, int | None]] = []

        def fake_rename(
            src: str,
            dst: str,
            *,
            src_dir_fd: int | None = None,
            dst_dir_fd: int | None = None,
        ) -> None:
            rename_calls.append((src, dst, src_dir_fd, dst_dir_fd))
            original_rename(
                src,
                dst,
                src_dir_fd=src_dir_fd,
                dst_dir_fd=dst_dir_fd,
            )

        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            temp_parent = out_root / ".pixel6.stage"
            final_slot = out_root / "pixel6"
            out_root.mkdir()
            stage_slot = write_raw_stage_slot(temp_parent, "pixel6")
            raw_puller.os.rename = fake_rename
            try:
                errors = raw_puller._install_validated_slot(
                    stage_slot,
                    final_slot,
                    out_root,
                )
            finally:
                raw_puller.os.rename = original_rename

        self.assertEqual(errors, [])
        self.assertEqual(
            {src for src, _dst, _src_fd, _dst_fd in rename_calls},
            set(raw_puller.RAW_SLOT_ALLOWED_DIRECTORIES),
        )
        self.assertEqual(
            {src for src, _dst, _src_fd, _dst_fd in rename_calls},
            {dst for _src, dst, _src_fd, _dst_fd in rename_calls},
        )
        self.assertTrue(
            all(src_fd is not None for _src, _dst, src_fd, _dst_fd in rename_calls)
        )
        self.assertTrue(
            all(dst_fd is not None for _src, _dst, _src_fd, dst_fd in rename_calls)
        )

    def test_kagemusha_android_raw_puller_temp_cleanup_removes_original_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_parent = Path(temp) / "raw" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            (temp_parent / "marker").write_text("temporary\n", encoding="utf-8")
            temp_parent_identity = raw_puller._file_identity(temp_parent.lstat())

            errors = raw_puller._cleanup_temp_parent(
                temp_parent,
                expected_identity=temp_parent_identity,
            )
            removed = not temp_parent.exists()

        self.assertEqual(errors, [])
        self.assertTrue(removed)

    def test_kagemusha_android_raw_puller_temp_cleanup_reports_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_parent = Path(temp) / "raw" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            (temp_parent / "marker").write_text("temporary\n", encoding="utf-8")
            temp_parent_identity = raw_puller._file_identity(temp_parent.lstat())

            with mock.patch.object(
                raw_puller.shutil,
                "rmtree",
                side_effect=OSError("simulated raw cleanup failure"),
            ):
                errors = raw_puller._cleanup_temp_parent(
                    temp_parent,
                    expected_identity=temp_parent_identity,
                )
            survived = temp_parent.is_dir()

        self.assertEqual(
            errors,
            ["raw pull temporary directory could not be removed"],
        )
        self.assertTrue(survived)

    def test_kagemusha_android_raw_puller_temp_cleanup_preserves_swapped_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            temp_parent = wrapper / "raw" / ".pixel6.stage"
            temp_parent.mkdir(parents=True)
            temp_parent_identity = raw_puller._file_identity(temp_parent.lstat())
            swapped_temp_parent = wrapper / "swapped-stage"
            temp_parent.rename(swapped_temp_parent)
            temp_parent.mkdir()
            (temp_parent / "victim").write_text("do not remove\n", encoding="utf-8")

            errors = raw_puller._cleanup_temp_parent(
                temp_parent,
                expected_identity=temp_parent_identity,
            )
            victim_survived = (temp_parent / "victim").is_file()
            original_survived = swapped_temp_parent.is_dir()

        self.assertEqual(errors, [])
        self.assertTrue(victim_survived)
        self.assertTrue(original_survived)

    def test_kagemusha_android_raw_puller_reports_temp_cleanup_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            summary_out = temp_path / "pull-summary.json"
            tar_bytes = raw_slot_tar_bytes("pixel6")
            runner = fake_raw_pull_runner(tar_bytes, "pixel6")

            with mock.patch.object(
                raw_puller,
                "_cleanup_temp_parent",
                return_value=["raw pull temporary directory could not be removed"],
            ):
                status, slot_path, errors = raw_puller.pull_raw_slot(
                    raw_pull_args(out_root, summary_out=summary_out),
                    runner=runner,
                )
            final_slot_exists = (out_root / "pixel6").is_dir()
            latest_exists = (out_root / "latest-slot.txt").exists()
            summary_exists = summary_out.exists()

        self.assertEqual(status, 1)
        self.assertEqual(slot_path, out_root / "pixel6")
        self.assertEqual(
            errors,
            ["raw pull temporary directory could not be removed"],
        )
        self.assertTrue(final_slot_exists)
        self.assertFalse(latest_exists)
        self.assertFalse(summary_exists)

    def test_kagemusha_android_raw_puller_install_sync_rejects_identity_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            actual = temp_path / "actual"
            other = temp_path / "other"
            actual.mkdir()
            other.mkdir()
            other_identity = raw_puller._file_identity(other.lstat())

            errors = raw_puller._sync_directory(
                actual,
                "raw slot directory could not be synced",
                expected_identity=other_identity,
            )

        self.assertEqual(errors, ["raw slot directory could not be synced"])

    def test_kagemusha_android_raw_puller_latest_writer_syncs_parent_identity(
        self,
    ) -> None:
        original_sync_directory = raw_puller._sync_directory
        sync_calls: list[tuple[Path, str, tuple[int, int] | None]] = []

        def fake_sync_directory(
            path: Path,
            error: str,
            *,
            expected_identity: tuple[int, int] | None = None,
        ) -> list[str]:
            sync_calls.append((path, error, expected_identity))
            return []

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            raw_puller._sync_directory = fake_sync_directory
            try:
                errors = raw_puller._write_latest_slot(root, "pixel6")
            finally:
                raw_puller._sync_directory = original_sync_directory
            latest_text = (root / "latest-slot.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, [])
        self.assertEqual(latest_text, "pixel6\n")
        self.assertEqual(
            [(path, error) for path, error, _identity in sync_calls],
            [(root, "raw latest-slot output parent directory could not be synced")],
        )
        self.assertIsNotNone(sync_calls[0][2])

    def test_kagemusha_android_raw_puller_latest_writer_rejects_symlink_after_replace(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "external-latest-slot.txt"
            target.write_text("external\n", encoding="utf-8")

            def replace_with_symlink(src, dst):  # type: ignore[no-untyped-def]
                original_replace(src, dst)
                Path(dst).unlink()
                Path(dst).symlink_to(target)

            raw_puller.os.replace = replace_with_symlink
            try:
                errors = raw_puller._write_latest_slot(root, "pixel6")
            finally:
                raw_puller.os.replace = original_replace

        self.assertEqual(
            errors,
            ["raw latest-slot output must not be a symlink after writing"],
        )

    def test_kagemusha_android_raw_puller_latest_writer_rejects_hardlink_after_replace(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = root / "external-latest-slot.txt"
            target.write_text("external\n", encoding="utf-8")

            def replace_with_hardlink(src, dst):  # type: ignore[no-untyped-def]
                original_replace(src, dst)
                Path(dst).unlink()
                os.link(target, dst)

            raw_puller.os.replace = replace_with_hardlink
            try:
                errors = raw_puller._write_latest_slot(root, "pixel6")
            finally:
                raw_puller.os.replace = original_replace

        self.assertEqual(
            errors,
            ["raw latest-slot output must not be hardlinked after writing"],
        )

    def test_kagemusha_android_raw_puller_latest_writer_rejects_readback_path_swap(
        self,
    ) -> None:
        original_open = raw_puller.Path.open
        swapped = False

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            latest_path = root / "latest-slot.txt"
            replacement = root / "replacement-latest-slot.txt"

            def open_with_swap(self, *args, **kwargs):  # type: ignore[no-untyped-def]
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if self == latest_path and mode == "rb" and not swapped:
                    swapped = True
                    replacement.write_text("replacement\n", encoding="utf-8")
                    os.replace(replacement, latest_path)
                return original_open(self, *args, **kwargs)

            raw_puller.Path.open = open_with_swap
            try:
                errors = raw_puller._write_latest_slot(root, "pixel6")
            finally:
                raw_puller.Path.open = original_open

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["raw latest-slot output changed while being read back"],
        )

    def test_kagemusha_android_raw_puller_latest_writer_reports_temp_cleanup_failure(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace
        original_unlink = raw_puller.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)

            def failing_replace(_src, _dst):  # type: ignore[no-untyped-def]
                raise OSError("simulated latest-slot replace failure")

            def failing_unlink(path: str, *args, **kwargs):
                if Path(path).name.startswith(".latest-slot.") and Path(path).suffix == ".tmp":
                    raise OSError("simulated latest-slot temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            raw_puller.os.replace = failing_replace
            raw_puller.os.unlink = failing_unlink
            try:
                errors = raw_puller._write_latest_slot(root, "pixel6")
            finally:
                raw_puller.os.replace = original_replace
                raw_puller.os.unlink = original_unlink
            temp_outputs = list(root.glob(".latest-slot.*.tmp"))

        self.assertEqual(
            errors,
            [
                "raw latest-slot output could not be written",
                "raw latest-slot output temporary output could not be removed",
            ],
        )
        self.assertEqual(len(temp_outputs), 1)

    def test_kagemusha_android_raw_puller_latest_writer_temp_cleanup_rejects_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            temp_output = root / ".latest-slot.swap.tmp"
            temp_output.write_text("original\n", encoding="utf-8")
            temp_identity = raw_puller._file_identity(temp_output.lstat())
            original_temp = root / "original-latest-slot-temp"
            temp_output.rename(original_temp)
            temp_output.write_text("do not remove\n", encoding="utf-8")

            errors = raw_puller._cleanup_temp_output(
                temp_output,
                "raw latest-slot output",
                temp_identity,
            )
            replacement = temp_output.read_text(encoding="utf-8")
            original = original_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["raw latest-slot output temporary output changed before cleanup"],
        )
        self.assertEqual(replacement, "do not remove\n")
        self.assertEqual(original, "original\n")

    def test_kagemusha_android_raw_puller_summary_rejects_nonfinite_json_before_tempfile(
        self,
    ) -> None:
        original_mkstemp = raw_puller.tempfile.mkstemp
        mkstemp_called = False

        def fail_mkstemp(*args, **kwargs):  # type: ignore[no-untyped-def]
            nonlocal mkstemp_called
            mkstemp_called = True
            raise AssertionError("mkstemp must not be called")

        with tempfile.TemporaryDirectory() as temp:
            summary_out = Path(temp) / "pull-summary.json"
            raw_puller.tempfile.mkstemp = fail_mkstemp
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA, "bad": float("nan")},
                )
            finally:
                raw_puller.tempfile.mkstemp = original_mkstemp

        self.assertEqual(errors, ["raw pull summary output is not strict JSON"])
        self.assertFalse(mkstemp_called)
        self.assertFalse(summary_out.exists())

    def test_kagemusha_android_raw_puller_summary_rejects_oversized_json_before_tempfile(
        self,
    ) -> None:
        original_mkstemp = raw_puller.tempfile.mkstemp
        original_limit = raw_puller.device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
        mkstemp_called = False

        def fail_mkstemp(*args, **kwargs):  # type: ignore[no-untyped-def]
            nonlocal mkstemp_called
            mkstemp_called = True
            raise AssertionError("mkstemp must not be called")

        with tempfile.TemporaryDirectory() as temp:
            summary_out = Path(temp) / "pull-summary.json"
            raw_puller.tempfile.mkstemp = fail_mkstemp
            raw_puller.device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = 4
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.tempfile.mkstemp = original_mkstemp
                raw_puller.device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = original_limit

        self.assertEqual(
            errors,
            ["raw pull summary output must be no more than 4 bytes"],
        )
        self.assertFalse(mkstemp_called)
        self.assertFalse(summary_out.exists())

    def test_kagemusha_android_raw_puller_summary_rejects_symlink_after_replace(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace

        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            summary_out = temp_path / "pull-summary.json"
            target = temp_path / "external-summary.json"
            target.write_text("external\n", encoding="utf-8")

            def replace_with_symlink(src, dst):  # type: ignore[no-untyped-def]
                original_replace(src, dst)
                Path(dst).unlink()
                Path(dst).symlink_to(target)

            raw_puller.os.replace = replace_with_symlink
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.os.replace = original_replace

        self.assertEqual(
            errors,
            ["raw pull summary output must not be a symlink after writing"],
        )

    def test_kagemusha_android_raw_puller_summary_rejects_hardlink_after_replace(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace

        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            summary_out = temp_path / "pull-summary.json"
            target = temp_path / "external-summary.json"
            target.write_text("external\n", encoding="utf-8")

            def replace_with_hardlink(src, dst):  # type: ignore[no-untyped-def]
                original_replace(src, dst)
                Path(dst).unlink()
                os.link(target, dst)

            raw_puller.os.replace = replace_with_hardlink
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.os.replace = original_replace

        self.assertEqual(
            errors,
            ["raw pull summary output must not be hardlinked after writing"],
        )

    def test_kagemusha_android_raw_puller_summary_rejects_readback_path_swap(
        self,
    ) -> None:
        original_open = raw_puller.Path.open
        swapped = False

        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            summary_out = temp_path / "pull-summary.json"
            replacement = temp_path / "replacement-summary.json"

            def open_with_swap(self, *args, **kwargs):  # type: ignore[no-untyped-def]
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if self == summary_out and mode == "rb" and not swapped:
                    swapped = True
                    replacement.write_text("replacement\n", encoding="utf-8")
                    os.replace(replacement, summary_out)
                return original_open(self, *args, **kwargs)

            raw_puller.Path.open = open_with_swap
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.Path.open = original_open

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["raw pull summary output changed while being read back"],
        )

    def test_kagemusha_android_raw_puller_summary_reports_temp_cleanup_failure(
        self,
    ) -> None:
        original_replace = raw_puller.os.replace
        original_unlink = raw_puller.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_out = Path(temp) / "pull-summary.json"

            def failing_replace(_src, _dst):  # type: ignore[no-untyped-def]
                raise OSError("simulated raw summary replace failure")

            def failing_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{summary_out.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated raw summary temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            raw_puller.os.replace = failing_replace
            raw_puller.os.unlink = failing_unlink
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.os.replace = original_replace
                raw_puller.os.unlink = original_unlink
            temp_outputs = list(summary_out.parent.glob(f".{summary_out.name}.*.tmp"))

        self.assertEqual(
            errors,
            [
                "raw pull summary output could not be written",
                "raw pull summary output temporary output could not be removed",
            ],
        )
        self.assertEqual(len(temp_outputs), 1)

    def test_kagemusha_android_raw_puller_summary_temp_cleanup_rejects_swap(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            temp_output = root / ".pull-summary.json.swap.tmp"
            temp_output.write_text("original\n", encoding="utf-8")
            temp_identity = raw_puller._file_identity(temp_output.lstat())
            original_temp = root / "original-raw-summary-temp"
            temp_output.rename(original_temp)
            temp_output.write_text("do not remove\n", encoding="utf-8")

            errors = raw_puller._cleanup_temp_output(
                temp_output,
                "raw pull summary output",
                temp_identity,
            )
            replacement = temp_output.read_text(encoding="utf-8")
            original = original_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["raw pull summary output temporary output changed before cleanup"],
        )
        self.assertEqual(replacement, "do not remove\n")
        self.assertEqual(original, "original\n")

    def test_kagemusha_android_raw_puller_summary_sync_rejects_parent_identity_swap(
        self,
    ) -> None:
        original_open = raw_puller.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "raw-summary-root"
            root.mkdir()
            summary_out = root / "pull-summary.json"
            swapped_root = wrapper / "raw-summary-root-swapped"
            swapped = False

            def swapping_parent_open(path, flags, *args, **kwargs):  # type: ignore[no-untyped-def]
                nonlocal swapped
                if Path(path) == summary_out.parent and not swapped:
                    summary_out.parent.rename(swapped_root)
                    summary_out.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            raw_puller.os.open = swapping_parent_open
            try:
                errors = raw_puller._write_summary(
                    summary_out,
                    {"schema": raw_puller.RAW_PULL_SUMMARY_SCHEMA},
                )
            finally:
                raw_puller.os.open = original_open
            written = (swapped_root / summary_out.name).read_text(encoding="utf-8")

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["raw pull summary output parent directory could not be synced"],
        )
        self.assertIn(raw_puller.RAW_PULL_SUMMARY_SCHEMA, written)

    def test_kagemusha_android_raw_puller_summary_digest_rejects_symlinked_artifact(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            target = temp_path / "external-runtime.log"
            target.write_text("kagemusha device-lab run complete\n", encoding="utf-8")
            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6"),
            )
            self.assertEqual(status, 0, errors)
            assert slot_path is not None
            replace_with_symlink(
                self,
                slot_path / "logs" / "runtime.log",
                target,
            )

            digests, digest_errors = raw_puller._raw_artifact_digests(slot_path)

        self.assertEqual(set(digests), raw_puller.RAW_SLOT_ALLOWED_PATHS - {"logs/runtime.log"})
        self.assertIn(
            "raw artifact digest logs/runtime.log must not be a symlink",
            digest_errors,
        )
        self.assertIn(
            "raw artifact digest inventory must include every required artifact",
            digest_errors,
        )

    def test_kagemusha_android_raw_puller_summary_digest_rejects_hardlinked_artifact(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            target = temp_path / "external-runtime.log"
            target.write_text("kagemusha device-lab run complete\n", encoding="utf-8")
            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6"),
            )
            self.assertEqual(status, 0, errors)
            assert slot_path is not None
            replace_with_hardlink(
                self,
                slot_path / "logs" / "runtime.log",
                target,
            )

            _digests, digest_errors = raw_puller._raw_artifact_digests(slot_path)

        self.assertIn(
            "raw artifact digest logs/runtime.log must not be hardlinked",
            digest_errors,
        )

    def test_kagemusha_android_raw_puller_requires_harness_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
            )
            runner = fake_raw_pull_runner(tar_bytes, "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=runner,
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot artifact attestation/harness-result.json is missing",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_harness_challenge_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            out_root = temp_path / "raw"
            mismatched_harness = {
                "alias": "android-keystore-alias",
                "attestation_security_level": "STRONG_BOX",
                "keymaster_security_level": "STRONG_BOX",
                "strongbox_attestation": True,
                "challenge_hex": "00",
                "chain_length": 4,
            }
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        mismatched_harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )
            runner = fake_raw_pull_runner(tar_bytes, "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=runner,
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json challenge_hex must match attestation/challenge.hex",
            errors,
        )

    def test_kagemusha_android_raw_puller_refuses_existing_slot_before_adb_tar(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            (out_root / "pixel6").mkdir(parents=True)
            runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root, slot_id="pixel6"),
                runner=runner,
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "slot directory already exists; refuse to overwrite raw evidence",
            errors,
        )
        self.assertFalse(any("exec-out" in call for call in runner.calls))  # type: ignore[attr-defined]

    def test_kagemusha_android_raw_puller_rejects_tar_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes("pixel6", extra_files={"../escape": b"x"})

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("raw slot tar member has unsafe path '../escape'", errors)
        self.assertFalse((out_root / "escape").exists())

    def test_kagemusha_android_raw_puller_rejects_compressed_tar_stream(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = gzip.compress(raw_slot_tar_bytes("pixel6"))

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertEqual(errors, ["raw slot tar stream could not be parsed"])
        self.assertFalse((out_root / "pixel6").exists())

    def test_kagemusha_android_raw_puller_rejects_noncanonical_tar_member_path(
        self,
    ) -> None:
        cases = (
            (
                "pixel6/./logs/runtime.log",
                "raw slot tar member has noncanonical path "
                "'pixel6/./logs/runtime.log'",
            ),
            (
                "pixel6//logs/runtime.log",
                "raw slot tar member has noncanonical path "
                "'pixel6//logs/runtime.log'",
            ),
            (
                "pixel6/logs/runtime.log/",
                "raw slot tar member has noncanonical path "
                "'pixel6/logs/runtime.log/'",
            ),
        )
        for member_name, expected_error in cases:
            with self.subTest(member_name=member_name):
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={"logs/runtime.log"},
                        extra_files={member_name: b"kagemusha device-lab run complete\n"},
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)
                self.assertFalse((out_root / "pixel6").exists())

    def test_kagemusha_android_raw_puller_allows_trailing_slash_directory_members(
        self,
    ) -> None:
        errors: list[str] = []

        normalised = raw_puller._normalise_tar_member_name(  # type: ignore[attr-defined]
            "pixel6/logs/",
            errors,
            allow_trailing_slash=True,
        )

        self.assertEqual(errors, [])
        self.assertEqual(normalised, "pixel6/logs")

    def test_kagemusha_android_raw_puller_rejects_tar_symlink_member(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                symlinks={"pixel6/logs/linked-runtime.log": "logs/runtime.log"},
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot tar member pixel6/logs/linked-runtime.log must not be a symlink or hardlink",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_tar_hardlink_member(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                hardlinks={"pixel6/logs/hard-runtime.log": "pixel6/logs/runtime.log"},
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot tar member pixel6/logs/hard-runtime.log must not be a symlink or hardlink",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_unexpected_raw_artifact(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                extra_files={"pixel6/debug/extra.log": b"debug-only\n"},
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot artifact debug/extra.log is not an allowed path",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_oversized_tar_member(self) -> None:
        original_limit = raw_puller.MAX_RAW_SLOT_FILE_BYTES
        try:
            raw_puller.MAX_RAW_SLOT_FILE_BYTES = 4
            with tempfile.TemporaryDirectory() as temp:
                out_root = Path(temp) / "raw"

                status, slot_path, errors = raw_puller.pull_raw_slot(
                    raw_pull_args(out_root),
                    runner=fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), "pixel6"),
                )
        finally:
            raw_puller.MAX_RAW_SLOT_FILE_BYTES = original_limit

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertTrue(
            any("must not exceed 4 bytes" in error for error in errors),
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_latest_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes("pixel6", latest_slot_id="pixel7")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("latest-slot.txt must be canonical and match slot id", errors)

    def test_kagemusha_android_raw_puller_rejects_noncanonical_latest_slot_query(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            runner = fake_raw_pull_runner(raw_slot_tar_bytes("pixel6"), " pixel6")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=runner,
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "latest-slot.txt must be canonical and contain exactly one slot id",
            errors,
        )
        self.assertFalse(any("exec-out" in call for call in runner.calls))  # type: ignore[attr-defined]

    def test_kagemusha_android_raw_puller_rejects_noncanonical_latest_slot(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                latest_slot_bytes=b" pixel6\n",
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("latest-slot.txt must be canonical and match slot id", errors)

    def test_kagemusha_android_raw_puller_rejects_noncanonical_challenge_file(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/challenge.hex"},
                extra_files={"pixel6/attestation/challenge.hex": b"ABCDEF00\n"},
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_challenge_file_newline(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/challenge.hex"},
                extra_files={"pixel6/attestation/challenge.hex": b"01020304"},
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/challenge.hex must be canonical lowercase hexadecimal plus trailing newline",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_tar_file_parent_collision(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                extra_files={
                    "pixel6/collision": b"file-parent\n",
                    "pixel6/collision/child": b"nested-child\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot tar member pixel6/collision/child parent directory could not be created",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_tar_directory_collision(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                extra_files={"pixel6/file-then-directory": b"file-parent\n"},
            )
            buffer = io.BytesIO(tar_bytes)
            collision_tar = io.BytesIO()
            with tarfile.open(fileobj=collision_tar, mode="w") as out_tar:
                with tarfile.open(fileobj=buffer, mode="r:*") as in_tar:
                    for member in in_tar:
                        extracted = in_tar.extractfile(member) if member.isfile() else None
                        out_tar.addfile(member, extracted)
                add_tar_directory(out_tar, "pixel6/file-then-directory/nested")

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(collision_tar.getvalue(), "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "raw slot tar directory pixel6/file-then-directory/nested could not be created",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_result_slot_field(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        del result["slot"]
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("attestation/result.json slot must match slot id", errors)

    def test_kagemusha_android_raw_puller_requires_result_chain_digest(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        del result["attestation_certificate_chain_sha256"]
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json attestation_certificate_chain_sha256 "
            "must be a lowercase SHA-256 hex digest",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_result_challenge_digest(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        del result["attestation_challenge_sha256"]
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json attestation_challenge_sha256 "
            "must be a lowercase SHA-256 hex digest",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_result_extra_field(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        result["debug_note"] = "not production evidence"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json contains unexpected field debug_note",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_result_identity_strings(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        result["device_fingerprint"] = " google/oriole "
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json device_fingerprint must not have surrounding whitespace",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_control_result_identity_strings(
        self,
    ) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        result["os_build_id"] = "TQ3A\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json os_build_id must not contain control characters",
            errors,
        )
        self.assertNotIn("\x1b", rendered)

    def test_kagemusha_android_raw_puller_requires_result_sdk_digests(self) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        del result["app_signing_certificate_sha256"]
        result["offline_wallet_policy_sha256"] = "ABCDEF"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json app_signing_certificate_sha256 "
            "must be a lowercase SHA-256 hex digest",
            errors,
        )
        self.assertIn(
            "attestation/result.json offline_wallet_policy_sha256 "
            "must be a lowercase SHA-256 hex digest",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_result_strongbox_levels(
        self,
    ) -> None:
        result = json.loads(raw_slot_artifacts("pixel6")["attestation/result.json"])
        result["keymint_security_level"] = "TEE"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/result.json"},
                extra_files={
                    "pixel6/attestation/result.json": json.dumps(
                        result,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/result.json keymint_security_level must be STRONGBOX",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_queue_slot_mismatch(self) -> None:
        queue = json.loads(raw_slot_artifacts("pixel6")["queue/pending_queue.json"])
        queue["slot_id"] = "pixel7"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"queue/pending_queue.json"},
                extra_files={
                    "pixel6/queue/pending_queue.json": json.dumps(
                        queue,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("queue/pending_queue.json slot_id must match slot id", errors)

    def test_kagemusha_android_raw_puller_rejects_queue_extra_field(self) -> None:
        queue = json.loads(raw_slot_artifacts("pixel6")["queue/pending_queue.json"])
        queue["debug_note"] = "not production evidence"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"queue/pending_queue.json"},
                extra_files={
                    "pixel6/queue/pending_queue.json": json.dumps(
                        queue,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "queue/pending_queue.json contains unexpected field debug_note",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_nonempty_pending_queue(self) -> None:
        queue = json.loads(raw_slot_artifacts("pixel6")["queue/pending_queue.json"])
        queue["pending_transactions"] = [{"id": "leftover-transfer"}]
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"queue/pending_queue.json"},
                extra_files={
                    "pixel6/queue/pending_queue.json": json.dumps(
                        queue,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_telemetry_slot_mismatch(
        self,
    ) -> None:
        telemetry = json.loads(raw_slot_artifacts("pixel6")["telemetry/telemetry.json"])
        telemetry["slot_id"] = "pixel7"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/telemetry.json"},
                extra_files={
                    "pixel6/telemetry/telemetry.json": json.dumps(
                        telemetry,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn("telemetry/telemetry.json slot_id must match slot id", errors)

    def test_kagemusha_android_raw_puller_rejects_whitespace_normalized_telemetry_slot(
        self,
    ) -> None:
        telemetry = json.loads(raw_slot_artifacts("pixel6")["telemetry/telemetry.json"])
        telemetry["slot_id"] = " pixel6 "
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/telemetry.json"},
                extra_files={
                    "pixel6/telemetry/telemetry.json": json.dumps(
                        telemetry,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/telemetry.json slot_id must not contain surrounding whitespace",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_telemetry_extra_field(self) -> None:
        telemetry = json.loads(raw_slot_artifacts("pixel6")["telemetry/telemetry.json"])
        telemetry["debug_note"] = "not production evidence"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/telemetry.json"},
                extra_files={
                    "pixel6/telemetry/telemetry.json": json.dumps(
                        telemetry,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/telemetry.json contains unexpected field debug_note",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_noncanonical_telemetry_identity_strings(
        self,
    ) -> None:
        cases = (
            (
                "device_model",
                " Pixel 6 ",
                "telemetry/telemetry.json device_model must not contain surrounding whitespace",
            ),
            (
                "device_codename",
                "oriole\u0000",
                "telemetry/telemetry.json device_codename must not contain control characters",
            ),
            (
                "app_package_name",
                "",
                "telemetry/telemetry.json app_package_name must be a non-empty string",
            ),
            (
                "app_package_name",
                "token=not-for-telemetry",
                "telemetry/telemetry.json app_package_name must not contain secret-looking material",
            ),
        )
        for field, value, expected_error in cases:
            with self.subTest(field=field, expected_error=expected_error):
                telemetry = json.loads(
                    raw_slot_artifacts("pixel6")["telemetry/telemetry.json"]
                )
                telemetry[field] = value
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={"telemetry/telemetry.json"},
                        extra_files={
                            "pixel6/telemetry/telemetry.json": json.dumps(
                                telemetry,
                                sort_keys=True,
                            ).encode("utf-8")
                            + b"\n",
                        },
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)

    def test_kagemusha_android_raw_puller_rejects_telemetry_app_package_mismatch(
        self,
    ) -> None:
        telemetry = json.loads(raw_slot_artifacts("pixel6")["telemetry/telemetry.json"])
        telemetry["app_package_name"] = "org.hyperledger.iroha.android.other"
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/telemetry.json"},
                extra_files={
                    "pixel6/telemetry/telemetry.json": json.dumps(
                        telemetry,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/telemetry.json app_package_name must match "
            "attestation/result.json app_package_name",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_noncanonical_json_slot_bindings(
        self,
    ) -> None:
        cases = (
            (
                "queue/pending_queue.json",
                "pixel6/queue/pending_queue.json",
                " pixel6 ",
                "queue/pending_queue.json slot_id must not contain surrounding whitespace",
            ),
            (
                "telemetry/telemetry.json",
                "pixel6/telemetry/telemetry.json",
                "pixel6\u0000",
                "telemetry/telemetry.json slot_id must not contain control characters",
            ),
            (
                "handoff/d2d-payment.json",
                "pixel6/handoff/d2d-payment.json",
                7,
                "handoff/d2d-payment.json slot_id must be a non-empty string",
            ),
            (
                "wallet/integrity.json",
                "pixel6/wallet/integrity.json",
                "",
                "wallet/integrity.json slot_id must be a non-empty string",
            ),
        )
        for relative, tar_member, slot_value, expected_error in cases:
            with self.subTest(relative=relative, expected_error=expected_error):
                payload = json.loads(raw_slot_artifacts("pixel6")[relative])
                payload["slot_id"] = slot_value
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={relative},
                        extra_files={
                            tar_member: json.dumps(
                                payload,
                                sort_keys=True,
                            ).encode("utf-8")
                            + b"\n",
                        },
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)

    def test_kagemusha_android_raw_puller_rejects_noncanonical_telemetry_suite(
        self,
    ) -> None:
        cases = (
            (
                "KAGEMUSHA-DEVICE-LAB",
                "telemetry/telemetry.json suite must identify a Kagemusha device-lab run",
            ),
            (
                " kagemusha-device-lab ",
                "telemetry/telemetry.json suite must not contain surrounding whitespace",
            ),
            (
                "kagemusha-device-lab\u0000",
                "telemetry/telemetry.json suite must not contain control characters",
            ),
            (
                "",
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
            (
                7,
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
        )
        for suite, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                telemetry = json.loads(
                    raw_slot_artifacts("pixel6")["telemetry/telemetry.json"]
                )
                telemetry["suite"] = suite
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={"telemetry/telemetry.json"},
                        extra_files={
                            "pixel6/telemetry/telemetry.json": json.dumps(
                                telemetry,
                                sort_keys=True,
                            ).encode("utf-8")
                            + b"\n",
                        },
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)

    def test_kagemusha_android_raw_puller_rejects_d2d_online_handoff(self) -> None:
        d2d = json.loads(raw_slot_artifacts("pixel6")["handoff/d2d-payment.json"])
        d2d["transport_offline"] = False
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"handoff/d2d-payment.json"},
                extra_files={
                    "pixel6/handoff/d2d-payment.json": json.dumps(
                        d2d,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "handoff/d2d-payment.json transport_offline must be true",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_wallet_rollback_failure(
        self,
    ) -> None:
        wallet = json.loads(raw_slot_artifacts("pixel6")["wallet/integrity.json"])
        wallet["rollback_rejection_passed"] = False
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"wallet/integrity.json"},
                extra_files={
                    "pixel6/wallet/integrity.json": json.dumps(
                        wallet,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "wallet/integrity.json rollback_rejection_passed must be true",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_failed_status_ndjson(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/status.ndjson"},
                extra_files={
                    "pixel6/telemetry/status.ndjson": b'{"status":"failed","slot_id":"pixel6"}\n',
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            errors,
        )
        self.assertIn("telemetry/status.ndjson must contain at least one ok status", errors)

    def test_kagemusha_android_raw_puller_rejects_status_ndjson_unexpected_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/status.ndjson"},
                extra_files={
                    "pixel6/telemetry/status.ndjson": (
                        b'{"status":"ok","slot_id":"pixel6","debug_note":"ignored"}\n'
                    ),
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/status.ndjson line 1 contains unexpected field debug_note",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_unknown_status_ndjson(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/status.ndjson"},
                extra_files={
                    "pixel6/telemetry/status.ndjson": (
                        b'{"status":"ok","slot_id":"pixel6"}\n'
                        b'{"status":"skipped","slot_id":"pixel6"}\n'
                    ),
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/status.ndjson line 2 status must be ok",
            errors,
        )

    def test_kagemusha_android_raw_puller_requires_status_slot_id(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/status.ndjson"},
                extra_files={
                    "pixel6/telemetry/status.ndjson": b'{"status":"ok"}\n',
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/status.ndjson line 1 slot_id must be a non-empty string",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_noncanonical_status_ndjson(
        self,
    ) -> None:
        cases = (
            (
                b' {"status":"ok","slot_id":"pixel6"}\n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                b'{"status":"ok","slot_id":"pixel6"} \n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                b'{"status":"ok","slot_id":"pixel6"}\r\n',
                "telemetry/status.ndjson must use LF line endings",
            ),
            (
                b'{"status":"ok","slot_id":"pixel6"}',
                "telemetry/status.ndjson must end with a trailing newline",
            ),
            (
                b'{"status":"OK","slot_id":"pixel6"}\n',
                "telemetry/status.ndjson line 1 status must be lowercase",
            ),
            (
                b'{"status":" ok ","slot_id":"pixel6"}\n',
                "telemetry/status.ndjson line 1 status must not contain surrounding whitespace",
            ),
            (
                b'{"status":"ok\\u0000","slot_id":"pixel6"}\n',
                "telemetry/status.ndjson line 1 status must not contain control characters",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={"telemetry/status.ndjson"},
                        extra_files={"pixel6/telemetry/status.ndjson": payload},
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)
                if not expected_error.startswith("telemetry/status.ndjson must "):
                    self.assertIn(
                        "telemetry/status.ndjson must contain at least one ok status",
                        errors,
                    )

    def test_kagemusha_android_raw_puller_rejects_status_slot_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"telemetry/status.ndjson"},
                extra_files={
                    "pixel6/telemetry/status.ndjson": b'{"status":"ok","slot_id":"pixel7"}\n',
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "telemetry/status.ndjson line 1 slot_id must match slot id",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_noncanonical_status_slot_binding(
        self,
    ) -> None:
        cases = (
            (
                b'{"status":"ok","slot_id":" pixel6 "}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain surrounding whitespace",
            ),
            (
                b'{"status":"ok","slot_id":"pixel6\\u0000"}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain control characters",
            ),
            (
                b'{"status":"ok","slot_id":6}\n',
                "telemetry/status.ndjson line 1 slot_id must be a string",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    out_root = Path(temp) / "raw"
                    tar_bytes = raw_slot_tar_bytes(
                        "pixel6",
                        omit_files={"telemetry/status.ndjson"},
                        extra_files={"pixel6/telemetry/status.ndjson": payload},
                    )

                    status, slot_path, errors = raw_puller.pull_raw_slot(
                        raw_pull_args(out_root),
                        runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
                    )

                self.assertEqual(status, 1)
                self.assertIsNone(slot_path)
                self.assertIn(expected_error, errors)

    def test_kagemusha_android_raw_puller_rejects_runtime_failure_marker(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"logs/runtime.log"},
                extra_files={
                    "pixel6/logs/runtime.log": (
                        b"kagemusha device-lab run complete\n"
                        b"TEST FAILED: offline handoff regressed\n"
                    ),
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "logs/runtime.log must not contain failure marker TEST FAILED",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_noncanonical_harness_strings(
        self,
    ) -> None:
        harness = {
            "alias": " android-keystore-alias ",
            "attestation_security_level": " strong_box ",
            "keymaster_security_level": "strongbox",
            "strongbox_attestation": True,
            "challenge_hex": "01 02 03 04",
            "chain_length": 4,
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json alias must not have surrounding whitespace",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json attestation_security_level must not have surrounding whitespace",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json keymaster_security_level must be STRONGBOX",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_control_harness_strings(
        self,
    ) -> None:
        harness = {
            "alias": "android-keystore-alias\x1b[31m",
            "attestation_security_level": "STRONG_BOX\x1b[31m",
            "keymaster_security_level": "STRONG_BOX\x1b[31m",
            "strongbox_attestation": True,
            "challenge_hex": "01020304",
            "chain_length": 4,
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json alias must not contain control characters",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json attestation_security_level must not contain control characters",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json keymaster_security_level must not contain control characters",
            errors,
        )
        self.assertNotIn("\x1b", "\n".join(errors))

    def test_kagemusha_android_raw_puller_rejects_harness_chain_length_mismatch(
        self,
    ) -> None:
        harness = {
            "alias": "android-keystore-alias",
            "attestation_security_level": "STRONG_BOX",
            "keymaster_security_level": "STRONG_BOX",
            "strongbox_attestation": True,
            "challenge_hex": "01020304",
            "chain_length": 3,
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json chain_length must match "
            "attestation/keymint-certificate-chain.pem certificate count",
            errors,
        )

    def test_kagemusha_android_raw_puller_rejects_malformed_harness_result(
        self,
    ) -> None:
        harness = {
            "alias": "android-keystore-alias",
            "attestation_security_level": "TEE",
            "keymaster_security_level": "SOFTWARE",
            "strongbox_attestation": False,
            "challenge_hex": "01020304",
            "chain_length": 1,
            "unexpected": "field",
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json contains unexpected field unexpected",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json attestation_security_level must be STRONGBOX",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json keymaster_security_level must be STRONGBOX",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json strongbox_attestation must be true",
            errors,
        )
        self.assertIn(
            "attestation/harness-result.json chain_length must be at least 2",
            errors,
        )

    def test_kagemusha_android_raw_puller_redacts_unexpected_harness_field(
        self,
    ) -> None:
        harness = {
            "alias": "android-keystore-alias",
            "attestation_security_level": "STRONG_BOX",
            "keymaster_security_level": "STRONG_BOX",
            "strongbox_attestation": True,
            "challenge_hex": "01020304",
            "chain_length": 4,
            "token=supersecret": "field",
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json contains unexpected field "
            f"{device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_kagemusha_android_raw_puller_redacts_control_unexpected_harness_field(
        self,
    ) -> None:
        unsafe_key = "debug\x1b[31m"
        harness = {
            "alias": "android-keystore-alias",
            "attestation_security_level": "STRONG_BOX",
            "keymaster_security_level": "STRONG_BOX",
            "strongbox_attestation": True,
            "challenge_hex": "01020304",
            "chain_length": 4,
            unsafe_key: "field",
        }
        with tempfile.TemporaryDirectory() as temp:
            out_root = Path(temp) / "raw"
            tar_bytes = raw_slot_tar_bytes(
                "pixel6",
                omit_files={"attestation/harness-result.json"},
                extra_files={
                    "pixel6/attestation/harness-result.json": json.dumps(
                        harness,
                        sort_keys=True,
                    ).encode("utf-8")
                    + b"\n",
                },
            )

            status, slot_path, errors = raw_puller.pull_raw_slot(
                raw_pull_args(out_root),
                runner=fake_raw_pull_runner(tar_bytes, "pixel6"),
            )

        rendered = "\n".join(errors)
        self.assertEqual(status, 1)
        self.assertIsNone(slot_path)
        self.assertIn(
            "attestation/harness-result.json contains unexpected field "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn(unsafe_key, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_scan_slot_rejects_missing_attestation_harness_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            (slot / "attestation" / "harness-result.json").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("missing attestation/harness-result.json", report["errors"])
        self.assertIn(
            "signed evidence artifact required slot artifact is missing "
            "attestation/harness-result.json",
            report["errors"],
        )

    def test_scan_slot_rejects_attestation_harness_challenge_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["challenge_hex"] = "00"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json challenge_hex digest must match "
            "slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_scan_slot_rejects_noncanonical_attestation_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["alias"] = " android-keystore-alias "
            harness["attestation_security_level"] = " strong_box "
            harness["keymaster_security_level"] = "strongbox"
            harness["challenge_hex"] = "01 02 03 04"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json alias must not have surrounding whitespace",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json attestation_security_level must not have surrounding whitespace",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json keymaster_security_level must be STRONGBOX",
            report["errors"],
        )
        self.assertIn(
            "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace",
            report["errors"],
        )

    def test_scan_slot_rejects_control_attestation_harness_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                signer,
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["alias"] = "android-keystore-alias\x1b[31m"
            write_json(harness_path, harness)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/harness-result.json alias must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_scan_slot_rejects_sha256_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            write_text(slot / "logs" / "runtime.log", "tampered after manifest\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn("sha256sum.txt digest mismatch for logs/runtime.log", report["errors"])

    def test_scan_slot_rejects_padded_sha256sum_line(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            manifest = slot / "sha256sum.txt"
            lines = manifest.read_text(encoding="utf-8").splitlines()
            lines[0] = f"{lines[0]} "
            write_text(manifest, "\n".join(lines) + "\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: must not contain surrounding whitespace",
            report["errors"],
        )

    def test_scan_slot_rejects_star_normalized_sha256sum_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            digest = hashlib.sha256(runtime_log.read_bytes()).hexdigest()
            write_text(slot / "sha256sum.txt", f"{digest}  *logs/runtime.log\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: unsafe path '*logs/runtime.log'",
            report["errors"],
        )

    def test_scan_slot_rejects_noncanonical_sha256sum_path(self) -> None:
        cases = ("logs/./runtime.log", "logs//runtime.log", "logs/runtime.log/")
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    slot = create_slot(Path(temp), "slot-a")
                    runtime_log = slot / "logs" / "runtime.log"
                    digest = hashlib.sha256(runtime_log.read_bytes()).hexdigest()
                    write_text(slot / "sha256sum.txt", f"{digest}  {relative}\n")

                    report = device_lab.scan_slot(slot)

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    "sha256sum.txt line 1: unsafe path is not canonical",
                    report["errors"],
                )

    def test_normalise_safe_relative_path_rejects_control_before_strip(
        self,
    ) -> None:
        errors: list[str] = []

        normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
            "\nlogs/runtime.log",
            errors,
            "slot artifact path",
        )

        rendered = "\n".join(errors)
        self.assertIsNone(normalised)
        self.assertEqual(
            errors,
            ["slot artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn("\nlogs/runtime.log", rendered)

    def test_normalise_safe_relative_path_rejects_surrounding_whitespace(
        self,
    ) -> None:
        errors: list[str] = []

        normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
            " logs/runtime.log ",
            errors,
            "slot artifact path",
        )

        self.assertIsNone(normalised)
        self.assertEqual(
            errors,
            ["slot artifact path: unsafe path contains surrounding whitespace"],
        )

    def test_normalise_safe_relative_path_rejects_noncanonical_aliases(
        self,
    ) -> None:
        cases = (
            "logs/./runtime.log",
            "logs//runtime.log",
            "logs/runtime.log/",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                errors: list[str] = []

                normalised = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                    relative,
                    errors,
                    "slot artifact path",
                )

                self.assertIsNone(normalised)
                self.assertEqual(
                    errors,
                    ["slot artifact path: unsafe path is not canonical"],
                )

    def test_scan_slot_rejects_symlinked_slot_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root, "real-slot")
            slot_link = root / "slot-link"
            create_dir_symlink(self, slot_link, real_slot)

            report = device_lab.scan_slot(slot_link)

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot directory must not be a symlink"])

    def test_scan_slot_rejects_slot_directory_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot directory metadata could not be read"])

    def test_scan_slot_rejects_symlinked_slot_parent_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-slots"
            create_slot(real_root, "slot-a")
            root_link = root / "linked-slots"
            create_dir_symlink(self, root_link, real_root)

            report = device_lab.scan_slot(root_link / "slot-a")

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot parent directory must not be a symlink"])

    def test_scan_slot_rejects_slot_parent_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp) / "slots", "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot.parent:
                        raise OSError("simulated slot parent lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot parent directory metadata could not be read"],
        )

    def test_scan_slot_uses_lstat_before_expected_directory_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertTrue(report["present"]["logs"])
        self.assertEqual(report["file_counts"]["logs"], 1)

    def test_scan_slot_reports_expected_directory_metadata_failure_before_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated scan slot directory metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir
                path_type.lstat = failing_lstat

                report = device_lab.scan_slot(slot)
        finally:
            path_type.lstat = original_lstat
            path_type.is_dir = original_is_dir

        self.assertEqual(report["status"], "error")
        self.assertIn("logs/ metadata could not be read", report["errors"])
        self.assertFalse(report["present"]["logs"])
        self.assertNotIn("missing logs/ directory", report["errors"])

    def test_scan_slot_counts_artifacts_with_lstat_before_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                runtime_log = slot / "logs" / "runtime.log"

                def failing_is_file(path: Path, *args, **kwargs):
                    if path == runtime_log:
                        raise OSError("simulated scan slot artifact is_file failure")
                    return original_is_file(path, *args, **kwargs)

                path_type.is_file = failing_is_file

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_file = original_is_file

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertEqual(report["file_counts"]["logs"], 1)

    def test_scan_slot_sha_presence_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                sha_path = slot / "sha256sum.txt"

                def failing_is_file(path: Path, *args, **kwargs):
                    if path == sha_path:
                        raise OSError("simulated scan slot sha is_file failure")
                    return original_is_file(path, *args, **kwargs)

                path_type.is_file = failing_is_file

                report = device_lab.scan_slot(slot)
        finally:
            path_type.is_file = original_is_file

        self.assertEqual(report["status"], "ok", report["errors"])
        self.assertTrue(report["present"]["sha256sum.txt"])

    def test_scan_slot_rejects_symlinked_slot_ancestor_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            create_slot(real_root, "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            report = device_lab.scan_slot(linked_parent / "device_lab" / "slot-a")

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["errors"], ["slot ancestor directory must not be a symlink"])

    def test_scan_slot_rejects_directory_traversal_failure_without_traceback(self) -> None:
        original_scandir = device_lab.os.scandir

        def failing_scandir(path: Path):
            if not isinstance(path, (str, bytes, device_lab.os.PathLike)):
                return original_scandir(path)
            if Path(path).name == "logs":
                raise OSError("simulated directory traversal failure")
            return original_scandir(path)

        try:
            device_lab.os.scandir = failing_scandir
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                report = device_lab.scan_slot(slot)
        finally:
            device_lab.os.scandir = original_scandir

        rendered = json.dumps(report)
        self.assertEqual(report["status"], "error")
        self.assertIn("logs/ could not be listed", report["errors"])
        self.assertNotIn("Traceback", rendered)

    def test_load_json_rejects_symlinked_ancestor_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_json_dir = root / "real-json"
            real_json_dir.mkdir()
            write_json(real_json_dir / "payload.json", {"status": "ok"})
            linked_json_dir = root / "linked-json"
            create_dir_symlink(self, linked_json_dir, real_json_dir)
            errors: list[str] = []

            data = device_lab._load_json(
                linked_json_dir / "payload.json",
                "test json",
                errors,
            )

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json ancestor directory must not be a symlink"])

    def test_load_json_rejects_symlink_swap_after_preflight(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "payload.json"
                target = root / "aliased-payload.json"
                write_json(payload, {"status": "ok"})
                write_json(target, {"status": "aliased"})
                errors: list[str] = []
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == payload and not swapped:
                        replace_with_symlink(self, payload, target)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                data = device_lab._load_json(payload, "test json", errors)
                target_status = json.loads(target.read_text(encoding="utf-8"))["status"]
        finally:
            path_type.stat = original_stat

        self.assertTrue(swapped)
        self.assertIsNone(data)
        self.assertEqual(errors, ["test json must not be a symlink"])
        self.assertEqual(target_status, "aliased")

    def test_load_json_rejects_regular_file_swap_after_preflight(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "payload.json"
                replacement = root / "replacement-payload.json"
                write_json(payload, {"status": "ok"})
                write_json(replacement, {"status": "replacement"})
                errors: list[str] = []
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == payload and not swapped:
                        replacement.replace(payload)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                data = device_lab._load_json(payload, "test json", errors)
                final_status = json.loads(payload.read_text(encoding="utf-8"))["status"]
        finally:
            path_type.stat = original_stat

        self.assertTrue(swapped)
        self.assertIsNone(data)
        self.assertEqual(errors, ["test json changed while being read"])
        self.assertEqual(final_status, "replacement")

    def test_validate_no_symlink_ancestors_rejects_cwd_failure(self) -> None:
        with mock.patch.object(
            Path,
            "cwd",
            side_effect=OSError("simulated cwd failure"),
        ):
            errors = device_lab.validate_no_symlink_ancestors(
                Path("payload.json"),
                "test json ancestor directory",
            )

        self.assertEqual(errors, ["test json ancestor directory metadata could not be read"])

    def test_validate_no_symlink_ancestors_rejects_ancestor_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["test json ancestor directory metadata could not be read"])

    def test_validate_no_symlink_ancestors_uses_lstat_before_is_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_is_symlink(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor is_symlink failure")
                    return original_is_symlink(path, *args, **kwargs)

                path_type.is_symlink = failing_is_symlink

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.is_symlink = original_is_symlink

        self.assertEqual(errors, [])

    def test_validate_no_symlink_ancestors_uses_lstat_before_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                payload = root / "slot" / "payload.json"
                payload.parent.mkdir()

                def failing_exists(path: Path, *args, **kwargs):
                    if path == payload.parent:
                        raise OSError("simulated ancestor exists failure")
                    return original_exists(path, *args, **kwargs)

                path_type.exists = failing_exists

                errors = device_lab.validate_no_symlink_ancestors(
                    payload,
                    "test json ancestor directory",
                )
        finally:
            path_type.exists = original_exists

        self.assertEqual(errors, [])

    def test_load_json_rejects_secret_path_directly_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_dir = Path(temp) / "token=supersecret-json"
            secret_dir.mkdir()
            payload = secret_dir / "payload.json"
            payload.write_text("{not-json", encoding="utf-8")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(
            errors,
            ["test json path must not contain secret-looking material"],
        )
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_control_path_directly_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_dir = Path(temp) / "control\njson"
            control_dir.mkdir()
            payload = control_dir / "payload.json"
            payload.write_text("{not-json", encoding="utf-8")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json path must not contain control characters"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_alias_path_directly_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "json\\payload.json",
                "test json path must not contain backslashes",
            ),
            (
                lambda base: base / "json" / ".." / "payload.json",
                "test json path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    payload = path_factory(Path(temp))
                    errors: list[str] = []

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias JSON path should fail before metadata"
                        ),
                    ):
                        data = device_lab._load_json(payload, "test json", errors)

                self.assertIsNone(data)
                self.assertEqual(errors, [expected_error])

    def test_load_json_rejects_nonfinite_json_constant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp) / "payload.json"
            payload.write_text('{"value": NaN}\n', encoding="utf-8")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json contains non-finite constant NaN"])

    def test_load_json_rejects_oversized_json_before_parse(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp) / "payload.json"
            payload.write_bytes(
                b'{"value":"' + b"x" * device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES + b'"}\n'
            )
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)

        self.assertIsNone(data)
        self.assertEqual(
            errors,
            [
                "test json must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_load_json_rejects_non_utf8_bytes_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp) / "payload.json"
            payload.write_bytes(b"\xff\xfe\xfd")
            errors: list[str] = []

            data = device_lab._load_json(payload, "test json", errors)
            rendered = "\n".join(errors)

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json could not be read"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn(str(payload), rendered)

    def test_load_json_rejects_file_metadata_failure_before_missing(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                payload = Path(temp) / "payload.json"
                write_json(payload, {"status": "ok"})
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == payload:
                        raise OSError("simulated JSON metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                data = device_lab._load_json(payload, "test json", errors)
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(data)
        self.assertEqual(errors, ["test json file metadata could not be read"])

    def test_scan_slot_rejects_symlinked_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must not be a symlink",
            report["errors"],
        )

    def test_scan_slot_rejects_hardlinked_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must not be hardlinked",
            report["errors"],
        )

    def test_scan_slot_rejects_non_regular_required_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            replace_with_fifo(self, slot / "logs" / "runtime.log")

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact logs/runtime.log must be a regular file",
            report["errors"],
        )
        self.assertIn(
            "sha256sum.txt references non-regular artifact logs/runtime.log",
            report["errors"],
        )

    def test_production_metadata_rejects_symlinked_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-signed-evidence.json"
            target.write_bytes(evidence_path.read_bytes())
            replace_with_symlink(self, evidence_path, target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must not be a symlink",
            report["errors"],
        )

    def test_production_metadata_rejects_hardlinked_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-signed-evidence.json"
            target.write_bytes(evidence_path.read_bytes())
            replace_with_hardlink(self, evidence_path, target)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must not be hardlinked",
            report["errors"],
        )

    def test_production_metadata_rejects_non_regular_signed_evidence_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root / "slots",
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            replace_with_fifo(self, slot / "evidence" / "signed-evidence.json")

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot artifact evidence/signed-evidence.json must be a regular file",
            report["errors"],
        )

    def test_explicit_missing_slot_returns_structured_error(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "missing-slot",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("[device-lab] missing-slot: slot directory missing", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_validate_slot_ids_rejects_duplicate_explicit_slots(self) -> None:
        slot_ids, errors = device_lab.validate_slot_ids(
            ["slot-a", "slot-a", "slot-b", "slot-b"]
        )

        self.assertEqual(slot_ids, ["slot-a", "slot-b"])
        self.assertEqual(
            errors,
            [
                "slot id 1 must not duplicate slot id 0",
                "slot id 3 must not duplicate slot id 2",
            ],
        )

    def test_validate_slot_ids_rejects_noncanonical_slot_aliases(self) -> None:
        slot_ids, errors = device_lab.validate_slot_ids(
            ["./slot-a", "slot-b/", "slot-c/."]
        )

        self.assertEqual(slot_ids, [])
        self.assertEqual(
            errors,
            [
                "slot id './slot-a' must be a canonical single directory name",
                "slot id 'slot-b/' must be a canonical single directory name",
                "slot id 'slot-c/.' must be a canonical single directory name",
            ],
        )

    def test_explicit_duplicate_slot_id_rejected_before_scan(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot-a",
                        "--slot",
                        "slot-a",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 1 must not duplicate slot id 0", rendered)
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_root_validator_rejects_secret_path_directly_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_root = Path(temp) / "token=supersecret-slots"

            errors = device_lab.validate_device_lab_root_path(secret_root)
            rendered = json.dumps(errors)

        self.assertEqual(
            errors,
            ["device-lab root path must not contain secret-looking material"],
        )
        self.assertNotIn(str(secret_root), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_root_validator_rejects_control_path_directly_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_root = Path(temp) / "control\nslots"

            errors = device_lab.validate_device_lab_root_path(control_root)
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["device-lab root path must not contain control characters"])
        self.assertNotIn(str(control_root), rendered)

    def test_root_validator_rejects_alias_path_directly_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "slots\\alias",
                "device-lab root path must not contain backslashes",
            ),
            (
                lambda base: base / "slots" / ".." / "alias",
                "device-lab root path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    root = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias device-lab root should fail before metadata"
                        ),
                    ):
                        errors = device_lab.validate_device_lab_root_path(root)

                self.assertEqual(errors, [expected_error])

    def test_root_validator_rejects_metadata_failure_directly_without_leak(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "slots"
                root.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == root:
                        raise OSError("simulated device-lab root metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_device_lab_root_path(root)
                rendered = json.dumps(errors)
                root_exists = root.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["device-lab root metadata could not be read"])
        self.assertTrue(root_exists)
        self.assertNotIn(str(root), rendered)

    def test_main_uses_lstat_before_missing_root_exists_preflight(self) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "missing-slots"

                def failing_exists(path: Path, *args, **kwargs):
                    if path == root:
                        raise OSError("simulated device-lab root exists failure")
                    return original_exists(path, *args, **kwargs)

                path_type.exists = failing_exists

                stdout = io.StringIO()
                stderr = io.StringIO()
                with redirect_stdout(stdout), redirect_stderr(stderr):
                    status = device_lab.main(
                        [
                            "--root",
                            str(root),
                            "--allow-missing-root",
                        ]
                    )
                rendered = stdout.getvalue() + stderr.getvalue()
        finally:
            path_type.exists = original_exists

        self.assertEqual(status, 0)
        self.assertIn("[device-lab] root missing; skipping", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_main_rejects_symlinked_device_lab_root_before_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-slots"
            create_slot(real_root, "slot-a")
            root_link = root / "linked-slots"
            create_dir_symlink(self, root_link, real_root)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root_link),
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("device-lab root must not be a symlink", rendered)
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_main_rejects_symlinked_device_lab_root_ancestor_before_discovery(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            create_slot(real_root, "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(linked_parent / "device_lab"),
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "device-lab root ancestor directory must not be a symlink",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_discover_slots_returns_structured_error_on_root_list_failure(self) -> None:
        original_iterdir = Path.iterdir

        def failing_iterdir(path: Path):
            if path.name == "device_lab":
                raise OSError("simulated root listing failure")
            return original_iterdir(path)

        try:
            Path.iterdir = failing_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                root.mkdir()

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [])
        self.assertEqual(errors, ["device-lab root could not be listed"])

    def test_discover_slots_uses_lstat_before_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot = create_slot(root, "slot-a")

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(slot_paths, [slot])
        self.assertEqual(errors, [])

    def test_discover_slots_reports_slot_metadata_failure_before_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot = create_slot(root, "slot-a")

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated discovery metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir
                path_type.lstat = failing_lstat

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            path_type.lstat = original_lstat
            path_type.is_dir = original_is_dir

        self.assertEqual(slot_paths, [])
        self.assertEqual(
            errors,
            ["device-lab slot directory metadata could not be read"],
        )

    def test_discover_slots_preserves_symlinked_slot_for_scan_slot_rejection(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / "device_lab"
            root.mkdir()
            target_slot = create_slot(base / "external_slots", "slot-a")
            linked_slot = root / "slot-link"
            create_dir_symlink(self, linked_slot, target_slot)

            slot_paths, errors = device_lab.discover_slots(root, None)

        self.assertEqual(slot_paths, [linked_slot])
        self.assertEqual(errors, [])

    def test_discover_slots_returns_stable_sorted_order(self) -> None:
        original_iterdir = Path.iterdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                slot_b = create_slot(root, "slot-b")
                slot_a = create_slot(root, "slot-a")

                def reverse_iterdir(path: Path):
                    if path == root:
                        return iter([slot_b, slot_a])
                    return original_iterdir(path)

                Path.iterdir = reverse_iterdir

                slot_paths, errors = device_lab.discover_slots(root, None)
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [slot_a, slot_b])
        self.assertEqual(errors, [])

    def test_discover_slots_revalidates_explicit_slot_ids_directly(self) -> None:
        original_iterdir = Path.iterdir

        def unexpected_iterdir(path: Path):
            raise AssertionError("root discovery must not run for invalid slot ids")

        try:
            Path.iterdir = unexpected_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"

                slot_paths, errors = device_lab.discover_slots(
                    root,
                    ["slot-a", "slot-a", "../outside"],
                )
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(slot_paths, [])
        self.assertIn("slot id 1 must not duplicate slot id 0", errors)
        self.assertIn(
            "slot id '../outside' must be a single safe directory name",
            errors,
        )

    def test_main_rejects_device_lab_root_list_failure_without_traceback(self) -> None:
        original_iterdir = Path.iterdir

        def failing_iterdir(path: Path):
            if path.name == "device_lab":
                raise OSError("simulated root listing failure")
            return original_iterdir(path)

        try:
            Path.iterdir = failing_iterdir
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "device_lab"
                root.mkdir()
                stdout = io.StringIO()
                stderr = io.StringIO()

                with redirect_stdout(stdout), redirect_stderr(stderr):
                    status = device_lab.main(
                        [
                            "--root",
                            str(root),
                            "--require-slot",
                        ]
                    )
                rendered = stdout.getvalue() + stderr.getvalue()
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(status, 1)
        self.assertIn("device-lab root could not be listed", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_unsafe_slot_id_rejected_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            create_slot(Path(temp), "outside")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "../outside",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id '../outside' must be a single safe directory name",
            rendered,
        )
        self.assertNotIn("[device-lab] outside: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_noncanonical_slot_id_rejected_before_path_join(self) -> None:
        for slot_id in ("./slot-a", "slot-a/", "slot-a/."):
            with self.subTest(slot_id=slot_id):
                with tempfile.TemporaryDirectory() as temp:
                    root = Path(temp) / "slots"
                    create_slot(root, "slot-a")
                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with redirect_stdout(stdout), redirect_stderr(stderr):
                        status = device_lab.main(
                            [
                                "--root",
                                str(root),
                                "--slot",
                                slot_id,
                                "--require-slot",
                            ]
                        )

                self.assertEqual(status, 1)
                rendered = stdout.getvalue() + stderr.getvalue()
                self.assertIn(
                    f"slot id {slot_id!r} must be a canonical single directory name",
                    rendered,
                )
                self.assertNotIn("[device-lab] slot-a: ok", rendered)
                self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_surrounding_whitespace_before_path_join(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        " slot-a ",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id 0 must not contain whitespace",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_newline_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot-a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot-a\n",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn(
            "slot id 0 must not contain whitespace",
            rendered,
        )
        self.assertNotIn("[device-lab] slot-a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_internal_whitespace_before_path_join(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, "slot a")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "slot a",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 0 must not contain whitespace", rendered)
        self.assertNotIn("[device-lab] slot a: ok", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_slot_id_rejects_control_character_before_path_join(self) -> None:
        unsafe_slot_id = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            create_slot(root, unsafe_slot_id)
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        unsafe_slot_id,
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        rendered = stdout.getvalue() + stderr.getvalue()
        self.assertIn("slot id 0 must not contain control characters", rendered)
        self.assertNotIn(unsafe_slot_id, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("Traceback", rendered)

    def test_explicit_secret_looking_slot_id_is_not_echoed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "token=supersecret",
                        "--require-slot",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot id 0 must not contain secret-looking material",
            stderr.getvalue(),
        )
        self.assertNotIn("token=supersecret", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())

    def test_discovered_secret_looking_slot_directory_is_not_echoed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            secret_slot = root / "token=supersecret-slot"
            secret_slot.mkdir(parents=True)
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn(
            "slot directory name must not contain secret-looking material",
            rendered,
        )
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, summary_text)
        self.assertNotIn(str(secret_slot), rendered)
        self.assertNotIn(str(secret_slot), summary_text)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("token=supersecret", summary_text)
        self.assertNotIn("Traceback", rendered)

    def test_discovered_whitespace_slot_directory_is_rejected_before_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, "slot a")
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
            )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn("slot directory name must not contain whitespace", rendered)
        self.assertIn("slot directory name must not contain whitespace", summary_text)
        self.assertNotIn("[device-lab] slot a: ok", rendered)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_discovered_control_slot_directory_is_rejected_without_echo(
        self,
    ) -> None:
        unsafe_slot_name = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, unsafe_slot_name)
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn(
            "slot directory name must not contain control characters",
            rendered,
        )
        self.assertIn(
            "slot directory name must not contain control characters",
            summary_text,
        )
        self.assertIn("<unsafe-slot-name>", rendered)
        self.assertIn("<unsafe-slot-name>", summary_text)
        self.assertNotIn(unsafe_slot_name, rendered)
        self.assertNotIn(unsafe_slot_name, summary_text)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("\x1b", summary_text)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_discovered_backslash_slot_directory_is_rejected_before_metadata(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            slot = create_slot(root, "slot-a\\b")
            summary_path = Path(temp) / "summary.json"
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            slot_still_exists = slot.exists()

        self.assertEqual(status, 1)
        self.assertIn("slot directory name must not contain backslashes", rendered)
        self.assertIn(
            "slot directory name must not contain backslashes",
            summary_text,
        )
        self.assertNotIn("[device-lab] slot-a\\b: ok", rendered)
        self.assertNotIn("Traceback", rendered)
        self.assertTrue(slot_still_exists)

    def test_scan_slot_rejects_control_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\x1b[31m")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(report["slot"], "<unsafe-slot-name>")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain control characters"],
        )

    def test_scan_slot_rejects_backslash_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\\b")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain backslashes"],
        )

    def test_scan_slot_rejects_newline_slot_directory_before_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a\n")

            report = device_lab.scan_slot(slot)

        self.assertEqual(report["status"], "error")
        self.assertEqual(
            report["errors"],
            ["slot directory name must not contain whitespace"],
        )

    def test_scan_slot_redacts_secret_looking_manifest_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not leak\n")
            write_text(
                slot / "sha256sum.txt",
                f"{'00' * 32}  {secret_path}\n",
            )

            report = device_lab.scan_slot(slot)

        rendered_errors = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            "sha256sum.txt line 1: unsafe path contains secret-looking material",
            rendered_errors,
        )
        self.assertNotIn(secret_path, rendered_errors)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered_errors)

    def test_slot_files_missing_slot_returns_empty_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "missing-slot"

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_root_entries_returns_stable_sorted_order(self) -> None:
        original_iterdir = Path.iterdir

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                z_entry = slot / "z-extra"
                a_entry = slot / "a-extra"
                write_text(z_entry, "z\n")
                write_text(a_entry, "a\n")

                def reverse_iterdir(path: Path):
                    if path == slot:
                        return iter([z_entry, a_entry])
                    return original_iterdir(path)

                Path.iterdir = reverse_iterdir

                errors: list[str] = []
                entries = device_lab._slot_root_entries(  # type: ignore[attr-defined]
                    slot,
                    errors,
                )
        finally:
            Path.iterdir = original_iterdir

        self.assertEqual(entries, [a_entry, z_entry])
        self.assertEqual(errors, [])

    def test_slot_files_non_directory_root_returns_empty_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "slot-a"
            write_text(slot, "not a directory\n")

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_reports_slot_metadata_failure_without_omission(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(files, set())
        self.assertEqual(errors, ["slot directory metadata could not be read"])

    def test_slot_files_secret_slot_path_returns_empty_without_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "token=supersecret-slot")

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_rejects_symlinked_slot_root_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            files = device_lab._slot_files(linked_slot)  # type: ignore[attr-defined]

        self.assertEqual(files, set())

    def test_slot_files_rejects_symlinked_slot_ancestor_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            create_slot(real_root, "slot-a")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            files = device_lab._slot_files(  # type: ignore[attr-defined]
                linked_root / "slot-a"
            )

        self.assertEqual(files, set())

    def test_slot_files_skips_symlinked_artifact_directory_directly_without_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_logs = root / "external-logs"
            write_text(external_logs / "runtime.log", "external log\n")
            for entry in (slot / "logs").iterdir():
                entry.unlink()
            (slot / "logs").rmdir()
            create_dir_symlink(self, slot / "logs", external_logs)

            files = device_lab._slot_files(slot)  # type: ignore[attr-defined]

        self.assertNotIn("logs/runtime.log", files)

    def test_slot_files_reports_artifact_directory_metadata_failure_without_omission(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                directory = slot / "logs"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == directory:
                        raise OSError("simulated artifact directory lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertNotIn("logs/runtime.log", files)
        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_files_reports_top_level_listing_failure_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]

        self.assertIn("slot directory could not be listed", errors)
        self.assertIn("logs/runtime.log", files)

    def test_slot_files_reports_artifact_metadata_failure_without_omission(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact = slot / "logs" / "runtime.log"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated inventory metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]
        finally:
            path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )
        self.assertNotIn("logs/runtime.log", files)

    def test_artifact_shape_validators_report_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            symlink_errors: list[str] = []
            hardlink_errors: list[str] = []
            regular_errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                device_lab.validate_no_slot_symlink_artifacts(slot, symlink_errors)
                device_lab.validate_no_slot_hardlink_artifacts(slot, hardlink_errors)
                device_lab.validate_slot_regular_file_artifacts(slot, regular_errors)

        self.assertIn("slot directory could not be listed", symlink_errors)
        self.assertIn("slot directory could not be listed", hardlink_errors)
        self.assertIn("slot directory could not be listed", regular_errors)

    def test_verify_sha256_manifest_reports_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            rewrite_sha256sum(slot)

            with patch_slot_iterdir_failure(slot):
                errors = device_lab.verify_sha256_manifest(slot)

        self.assertIn("slot directory could not be listed", errors)

    def test_required_signed_evidence_digest_paths_reports_top_level_listing_failure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            errors: list[str] = []

            with patch_slot_iterdir_failure(slot):
                paths = device_lab._required_signed_evidence_digest_paths(  # type: ignore[attr-defined]
                    slot,
                    errors,
                )

        self.assertIn("slot directory could not be listed", errors)
        self.assertIn("logs/runtime.log", paths)

    def test_signer_manifest_rewrite_rejects_top_level_listing_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")

            with patch_slot_iterdir_failure(slot):
                errors = evidence_signer.rewrite_sha256_manifest(slot)

        self.assertEqual(errors, ["slot directory could not be listed"])

    def test_parse_sha256_manifest_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_control_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "control\nslot"
            slot.mkdir()
            write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot path must not contain control characters"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_alias_slot_path_before_metadata(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    slot = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias manifest slot path should fail before metadata"
                        ),
                    ):
                        entries, errors = device_lab.parse_sha256_manifest(slot)

                self.assertEqual(entries, {})
                self.assertEqual(errors, [expected_error])

    def test_parse_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            write_text(real_slot / "sha256sum.txt", "not-a-manifest-line\n")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            entries, errors = device_lab.parse_sha256_manifest(linked_slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot directory must not be a symlink"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_slot_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                write_text(slot / "sha256sum.txt", "not-a-manifest-line\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_symlinked_slot_ancestor_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            real_slot = create_slot(real_root, "slot-a")
            write_text(real_slot / "sha256sum.txt", "not-a-manifest-line\n")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            entries, errors = device_lab.parse_sha256_manifest(linked_root / "slot-a")
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_hardlinked_manifest_before_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_manifest = root / "external-sha256sum.txt"
            write_text(external_manifest, "not-a-manifest-line\n")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_file_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                manifest_path = slot / "sha256sum.txt"
                write_text(manifest_path, "not-a-manifest-line\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == manifest_path:
                        raise OSError("simulated manifest file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
                manifest_text = manifest_path.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt file metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertEqual(manifest_text, "not-a-manifest-line\n")

    def test_parse_sha256_manifest_rejects_hardlink_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                manifest_path = slot / "sha256sum.txt"
                write_text(manifest_path, "not-a-manifest-line\n")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == manifest_path and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated manifest hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.stat = original_stat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt hardlink metadata could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_parse_sha256_manifest_rejects_non_utf8_bytes_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            (slot / "sha256sum.txt").write_bytes(b"\xff\xfe\xfd")

            entries, errors = device_lab.parse_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt could not be read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_parse_sha256_manifest_rejects_oversized_manifest_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            manifest_path = slot / "sha256sum.txt"
            manifest_path.write_bytes(
                b"#"
                + b"x" * device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES
                + b"\n"
            )

            entries, errors = device_lab.parse_sha256_manifest(slot)

        self.assertEqual(entries, {})
        self.assertEqual(
            errors,
            [
                "sha256sum.txt must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES} bytes"
            ],
        )

    def test_parse_sha256_manifest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                manifest_path = slot / "sha256sum.txt"
                replacement = root / "replacement-sha256sum.txt"
                write_text(replacement, "not-a-manifest-line\n")
                swapped = False

                def swapping_stat(path: Path, *args, **kwargs):
                    nonlocal swapped
                    result = original_stat(path, *args, **kwargs)
                    if path == manifest_path and not swapped:
                        replacement.replace(manifest_path)
                        swapped = True
                    return result

                path_type.stat = swapping_stat

                entries, errors = device_lab.parse_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.stat = original_stat

        self.assertEqual(entries, {})
        self.assertEqual(errors, ["sha256sum.txt changed while being read"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)

    def test_verify_sha256_manifest_rejects_secret_slot_path_directly_before_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            logs_dir = slot / "logs"
            logs_dir.mkdir()
            write_text(logs_dir / "runtime.log", "must not be traversed\n")
            write_text(slot / "sha256sum.txt", "\n")

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("sha256sum.txt is empty", rendered)
        self.assertNotIn("missing entry", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_files_rejects_alias_slot_path_before_metadata(self) -> None:
        cases = (
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    slot = path_factory(Path(temp))
                    errors: list[str] = []

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias slot path should fail before metadata"
                        ),
                    ):
                        files = device_lab._slot_files(slot, errors)  # type: ignore[attr-defined]

                self.assertEqual(files, set())
                self.assertEqual(errors, [expected_error])

    def test_verify_sha256_manifest_rejects_symlinked_slot_root_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = create_slot(root / "real", "slot-a")
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = device_lab.verify_sha256_manifest(linked_slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot directory must not be a symlink"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_slot_metadata_failure_before_parse(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == slot:
                        raise OSError("simulated slot lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.verify_sha256_manifest(slot)
                rendered = "\n".join(errors)
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_symlinked_slot_ancestor_before_discovery(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_root = root / "real-root"
            create_slot(real_root, "slot-a")
            linked_root = root / "linked-root"
            create_dir_symlink(self, linked_root, real_root)

            errors = device_lab.verify_sha256_manifest(linked_root / "slot-a")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("digest mismatch", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_missing_slot_returns_missing_manifest_without_traceback(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "missing-slot"

            errors = device_lab.verify_sha256_manifest(slot)

        self.assertEqual(errors, ["missing sha256sum.txt"])

    def test_verify_sha256_manifest_rejects_hardlinked_manifest_before_discovery(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_manifest = root / "external-sha256sum.txt"
            write_text(external_manifest, "not-a-manifest-line\n")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertNotIn("expected '<sha256> <path>'", rendered)
        self.assertNotIn("missing entry", rendered)

    def test_verify_sha256_manifest_rejects_symlinked_artifact_directory_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slot-a"
            slot.mkdir()
            external_logs = root / "external-logs"
            external_log = external_logs / "runtime.log"
            write_text(external_log, "external log\n")
            create_dir_symlink(self, slot / "logs", external_logs)
            external_digest = hashlib.sha256(external_log.read_bytes()).hexdigest()
            write_text(slot / "sha256sum.txt", f"{external_digest}  logs/runtime.log\n")

            errors = device_lab.verify_sha256_manifest(slot)
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact under symlink directory "
                "logs/runtime.log"
            ],
        )
        self.assertNotIn("digest mismatch", rendered)

    def test_manifest_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_manifest_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_manifest_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt references symlink artifact logs/runtime.log"],
        )

    def test_manifest_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["sha256sum.txt references hardlinked artifact logs/runtime.log"],
        )

    def test_manifest_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = device_lab._manifest_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact logs/runtime.log "
                "must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_manifest_artifact_digest_rejects_file_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "logs" / "runtime.log"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError("simulated manifest artifact metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact file metadata could not be read "
                "logs/runtime.log"
            ],
        )

    def test_manifest_artifact_digest_uses_lstat_before_relative_ancestor_is_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                logs_dir = slot / "logs"
                target = logs_dir / "runtime.log"
                expected_digest = hashlib.sha256(target.read_bytes()).hexdigest()

                def failing_is_symlink(path: Path, *args, **kwargs):
                    if path == logs_dir:
                        raise OSError("simulated relative ancestor is_symlink failure")
                    return original_is_symlink(path, *args, **kwargs)

                path_type.is_symlink = failing_is_symlink

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.is_symlink = original_is_symlink

        self.assertEqual(digest, expected_digest)
        self.assertEqual(errors, [])

    def test_manifest_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact that could not be read "
                "logs/runtime.log"
            ],
        )

    def test_manifest_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_manifest_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_manifest_artifact_for_digest = swapping_validate

                digest, errors = device_lab._manifest_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_manifest_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            [
                "sha256sum.txt references artifact changed while being read "
                "logs/runtime.log"
            ],
        )

    def test_verify_sha256_manifest_revalidates_artifact_before_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_parse = device_lab.parse_sha256_manifest

            def parse_then_alias(slot_path: Path):
                entries, errors = original_parse(slot_path)
                if not errors:
                    replace_with_symlink(self, slot_path / "logs" / "runtime.log", target)
                return entries, errors

            try:
                device_lab.parse_sha256_manifest = parse_then_alias
                errors = device_lab.verify_sha256_manifest(slot)
            finally:
                device_lab.parse_sha256_manifest = original_parse

        self.assertIn(
            "sha256sum.txt references symlink artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn("sha256sum.txt digest mismatch for logs/runtime.log", errors)

    def test_attestation_result_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            attestation_dir = slot / "attestation"
            attestation_dir.mkdir(parents=True)
            write_text(attestation_dir / "result.json", "{not-json")
            errors: list[str] = []

            device_lab.validate_attestation_result(slot, {}, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            transcript_path = Path(temp) / "d2d-payment.json"
            write_text(transcript_path, "{not-json")
            errors: list[str] = []

            device_lab.validate_d2d_payment_transcript(
                slot,
                transcript_path,
                {},
                errors,
            )
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            handoff_dir = slot / "handoff"
            handoff_dir.mkdir(parents=True)
            write_text(handoff_dir / "d2d-payment.json", "{not-json")
            errors: list[str] = []

            relative, digest = device_lab.validate_d2d_payment_transcript_binding(
                slot,
                {
                    "d2d_payment_transcript_path": "handoff/d2d-payment.json",
                    "d2d_payment_transcript_sha256": "00" * 64,
                },
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(relative)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("does not match", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_wallet_transcript_binding_rejects_secret_slot_path_directly_before_artifact_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            wallet_dir = slot / "wallet"
            wallet_dir.mkdir(parents=True)
            write_text(wallet_dir / "integrity.json", "{not-json")
            errors: list[str] = []

            relative, digest = device_lab.validate_wallet_integrity_transcript_binding(
                slot,
                {
                    "wallet_integrity_transcript_path": "wallet/integrity.json",
                    "wallet_integrity_transcript_sha256": "00" * 64,
                },
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(relative)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("does not match", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_d2d_transcript_binding_rejects_symlink_path_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-d2d-payment.json"
            write_text(target, (slot / "handoff" / "d2d-payment.json").read_text())
            replace_with_symlink(self, slot / "handoff" / "d2d-payment.json", target)
            errors: list[str] = []

            relative, digest = device_lab.validate_d2d_payment_transcript_binding(
                slot,
                metadata,
                errors,
            )

        self.assertEqual(relative, "handoff/d2d-payment.json")
        self.assertIsNone(digest)
        self.assertIn(
            "slot.json d2d_payment_transcript_path references symlink artifact "
            "handoff/d2d-payment.json",
            errors,
        )
        self.assertNotIn(
            "slot.json d2d_payment_transcript_sha256 does not match "
            "d2d_payment_transcript_path",
            errors,
        )

    def test_wallet_transcript_binding_rejects_hardlink_path_before_digest_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-integrity.json"
            write_text(target, (slot / "wallet" / "integrity.json").read_text())
            replace_with_hardlink(self, slot / "wallet" / "integrity.json", target)
            errors: list[str] = []

            relative, digest = device_lab.validate_wallet_integrity_transcript_binding(
                slot,
                metadata,
                errors,
            )

        self.assertEqual(relative, "wallet/integrity.json")
        self.assertIsNone(digest)
        self.assertIn(
            "slot.json wallet_integrity_transcript_path references hardlinked "
            "artifact wallet/integrity.json",
            errors,
        )
        self.assertNotIn(
            "slot.json wallet_integrity_transcript_sha256 does not match "
            "wallet_integrity_transcript_path",
            errors,
        )

    def test_d2d_transcript_rejects_symlinked_queue_before_digest_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            target = root / "outside-pending-queue.json"
            write_text(target, (slot / "queue" / "pending_queue.json").read_text())
            replace_with_symlink(self, slot / "queue" / "pending_queue.json", target)
            errors: list[str] = []

            device_lab.validate_d2d_payment_transcript(
                slot,
                slot / "handoff" / "d2d-payment.json",
                metadata,
                errors,
            )

        self.assertIn(
            "d2d payment transcript queue_after_sha256 references symlink "
            "artifact queue/pending_queue.json",
            errors,
        )
        self.assertNotIn(
            "d2d payment transcript queue_after_sha256 must match "
            "queue/pending_queue.json",
            errors,
        )

    def test_d2d_transcript_uses_lstat_before_queue_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            queue_path = slot / "queue" / "pending_queue.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated queue is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated queue lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_d2d_payment_transcript(
                    slot,
                    slot / "handoff" / "d2d-payment.json",
                    metadata,
                    errors,
                )
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "d2d payment transcript queue_after_sha256 references artifact file "
            "metadata could not be read queue/pending_queue.json",
            errors,
        )

    def test_required_artifact_shapes_rejects_secret_slot_path_directly_before_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            write_text(log_dir / "runtime.log", "")
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must be non-empty", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_required_artifact_shapes_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        old_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
        try:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                write_text(slot / "logs" / "runtime.log", "runtime log too large\n")
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
        finally:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_limit

        self.assertIn(
            "required slot artifact logs/runtime.log must be no more than 8 bytes",
            errors,
        )

    def test_required_status_artifact_rejects_symlink_before_text_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-status.ndjson"
            write_text(target, '{"status":"failed"}\n')
            replace_with_symlink(self, slot / "telemetry" / "status.ndjson", target)
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)

        self.assertIn(
            "telemetry/status.ndjson references symlink artifact "
            "telemetry/status.ndjson",
            errors,
        )
        self.assertNotIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            errors,
        )

    def test_required_runtime_log_rejects_hardlink_before_text_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "missing completion marker\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)

        self.assertIn(
            "logs/runtime.log references hardlinked artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            errors,
        )

    def test_required_runtime_log_rejects_symlink_swap_after_preflight(self) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "pixel8")
                runtime_log = slot / "logs" / "runtime.log"
                target = root / "outside-runtime.log"
                write_text(target, "TEST FAILED\nmissing completion marker\n")
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, validate_errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == runtime_log and not validate_errors and not swapped:
                        replace_with_symlink(self, runtime_log, target)
                        swapped = True
                    return artifact, artifact_stat, validate_errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
                target_text = target.read_text(encoding="utf-8")
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertEqual(target_text, "TEST FAILED\nmissing completion marker\n")
        self.assertIn(
            "logs/runtime.log references symlink artifact logs/runtime.log",
            errors,
        )
        self.assertNotIn("logs/runtime.log contains failure marker TEST FAILED", errors)
        self.assertNotIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            errors,
        )

    def test_required_status_artifact_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            status_path = slot / "telemetry" / "status.ndjson"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated status is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated status lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn("telemetry/status.ndjson file metadata could not be read", errors)

    def test_required_runtime_log_uses_lstat_before_is_file_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn("logs/runtime.log file metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_rejects_secret_slot_path_directly_before_traversal(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            target = Path(temp) / "outside-runtime.log"
            write_text(target, "outside\n")
            write_text(log_dir / "runtime.log", "placeholder\n")
            replace_with_symlink(self, log_dir / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must not be a symlink", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_symlink_artifact_validator_reports_slot_metadata_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_reports_directory_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_symlink_artifact_validator_reports_nested_artifact_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_symlink_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_hardlink_artifact_validator_rejects_secret_slot_path_directly_before_stat(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            target = Path(temp) / "outside-runtime.log"
            write_text(target, "outside\n")
            write_text(log_dir / "runtime.log", "placeholder\n")
            replace_with_hardlink(self, log_dir / "runtime.log", target)
            errors: list[str] = []

            device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must not be hardlinked", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_hardlink_artifact_validator_reports_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                artifact = slot / "logs" / "runtime.log"
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == artifact:
                        raise OSError("simulated hardlink validator metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
        finally:
            path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_hardlink_artifact_validator_uses_lstat_before_directory_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_exists(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_no_slot_hardlink_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_rejects_secret_slot_path_directly_before_shape(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            log_dir = slot / "logs"
            log_dir.mkdir(parents=True)
            directory_artifact = log_dir / "runtime.log"
            directory_artifact.mkdir()
            errors: list[str] = []

            device_lab.validate_slot_regular_file_artifacts(slot, errors)
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("must be a regular file", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_slot_regular_artifact_validator_reports_slot_metadata_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_regular_artifact_validator_uses_lstat_before_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            slot_metadata_path = slot / "slot.json"
            write_text(slot_metadata_path, "{}\n")

            def failing_exists(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot_metadata_path:
                    raise OSError("simulated slot metadata lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("slot.json file metadata could not be read", errors)

    def test_slot_regular_artifact_validator_reports_directory_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_uses_lstat_before_directory_exists_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_exists = path_type.exists
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            logs_dir = slot / "logs"

            def failing_exists(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs exists failure")
                return original_exists(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == logs_dir:
                    raise OSError("simulated logs lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.exists = failing_exists
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.exists = original_exists
                path_type.lstat = original_lstat

        self.assertIn("logs/ metadata could not be read", errors)

    def test_slot_regular_artifact_validator_reports_nested_artifact_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_slot_regular_artifact_validator_uses_lstat_before_nested_symlink_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_symlink = path_type.is_symlink
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"

            def failing_is_symlink(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log symlink preflight failure")
                return original_is_symlink(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == runtime_log:
                    raise OSError("simulated runtime log lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_symlink = failing_is_symlink
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_slot_regular_file_artifacts(slot, errors)
            finally:
                path_type.is_symlink = original_is_symlink
                path_type.lstat = original_lstat

        self.assertIn(
            "slot artifact logs/runtime.log file metadata could not be read",
            errors,
        )

    def test_required_artifact_shapes_reports_required_artifact_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            status_path = slot / "telemetry" / "status.ndjson"

            def failing_stat(path: Path, *args, **kwargs):
                if path == status_path:
                    raise OSError("simulated required artifact stat failure")
                return original_stat(path, *args, **kwargs)

            try:
                path_type.stat = failing_stat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.stat = original_stat

        self.assertIn(
            "required slot artifact metadata could not be read telemetry/status.ndjson",
            errors,
        )

    def test_required_artifact_shapes_uses_lstat_before_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            queue_path = slot / "queue" / "pending_queue.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated required artifact is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == queue_path:
                    raise OSError("simulated required artifact lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat
                errors: list[str] = []

                device_lab.validate_required_kagemusha_slot_artifact_shapes(slot, errors)
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "required slot artifact metadata could not be read queue/pending_queue.json",
            errors,
        )

    def test_signed_evidence_artifact_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            evidence_path = Path(temp) / "signed-evidence.json"
            write_text(evidence_path, "{not-json")
            errors: list[str] = []

            details = device_lab.validate_signed_evidence_artifact(
                slot,
                evidence_path,
                {},
                {},
                errors,
            )
            rendered = "\n".join(errors)

        self.assertEqual(details, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_kagemusha_production_metadata_rejects_secret_slot_path_directly_before_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir()
            write_text(slot / "slot.json", "{not-json")

            errors, details = device_lab.validate_kagemusha_production_metadata(slot)
            rendered = "\n".join(errors)

        self.assertEqual(details, {})
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_json_summary_redacts_secret_looking_unlisted_artifact_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            slot = create_slot(root, "slot-a")
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not leak\n")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stderr_text = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(secret_path, stderr_text)
        self.assertNotIn(secret_path, summary_text)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, stderr_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, summary_text)

    def test_production_json_summary_redacts_secret_looking_required_artifact_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            signer = create_test_signer(root / "keys")
            secret_path = "logs/token=supersecret.log"
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / secret_path, "must not leak\n")
            rewrite_sha256sum(slot)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stdout_text = stdout.getvalue()
            rendered = stdout_text + stderr.getvalue() + summary_text

        self.assertEqual(status, 1)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(str(summary_path), stdout_text)
        self.assertNotIn(secret_path, rendered)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn(device_lab.SECRET_PATH_REDACTION, rendered)
        self.assertIn(
            f"signed evidence artifact artifact_digests[{device_lab.SECRET_PATH_REDACTION}] must be lowercase sha256 hex",
            rendered,
        )

    def test_production_metadata_rejects_duplicate_slot_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "slot.json", "schema", "shadow")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_redacts_secret_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "slot.json",
                '{"token=supersecret": 1, "token=supersecret": 2}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json contains duplicate JSON object key {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_redacts_control_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "slot.json",
                '{"debug\\u001b[31m": 1, "debug\\u001b[31m": 2}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains duplicate JSON object key "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("debug\x1b[31m", rendered)
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_duplicate_attestation_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "attestation" / "result.json", "slot", "shadow")
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json contains duplicate JSON object key slot",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_signed_evidence_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(
                slot / "evidence" / "signed-evidence.json",
                "schema",
                "shadow",
            )
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_d2d_transcript_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "handoff" / "d2d-payment.json", "schema", "shadow")
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_duplicate_wallet_integrity_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            inject_duplicate_json_key(slot / "wallet" / "integrity.json", "schema", "shadow")
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript contains duplicate JSON object key schema",
            report["errors"],
        )

    def test_production_metadata_rejects_unavailable_recursive_compact_one_hop_probe(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["abi7_recursive_compact_jni_probe"] = "unavailable"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json abi7_recursive_compact_jni_probe must be one of ['one_hop_verified']",
            report["errors"],
        )

    def test_production_metadata_rejects_generic_recursive_compact_prover_state(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["abi7_recursive_compact_prover_state"] = (
                "multi_hop_proof_composition_unavailable"
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json abi7_recursive_compact_prover_state must be one of ['multi_hop_proof_composed']",
            report["errors"],
        )

    def test_production_metadata_rejects_abi6_probe_ok_status_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["abi6_recursive_spend_jni_probe"] = "ok"
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["abi6_recursive_spend_jni_probe"] = "ok"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json abi6_recursive_spend_jni_probe must be one of ['passed']",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_probe_states(self) -> None:
        cases = (
            (
                "abi6_recursive_spend_jni_probe",
                " passed ",
                "slot.json abi6_recursive_spend_jni_probe must not contain surrounding whitespace",
            ),
            (
                "abi6_recursive_spend_jni_probe",
                "PASSED",
                "slot.json abi6_recursive_spend_jni_probe must be lowercase",
            ),
            (
                "abi7_recursive_compact_jni_probe",
                "one_hop_verified\u0000",
                "slot.json abi7_recursive_compact_jni_probe must not contain control characters",
            ),
            (
                "abi7_recursive_compact_jni_probe",
                "",
                "slot.json abi7_recursive_compact_jni_probe must be a non-empty string",
            ),
            (
                "abi7_recursive_compact_prover_state",
                7,
                "slot.json abi7_recursive_compact_prover_state must be a non-empty string",
            ),
        )
        for field, value, expected_error in cases:
            with self.subTest(field=field, expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata[field] = value
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_signed_evidence_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_sha256"] = "00" * 32
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path",
            report["errors"],
        )

    def test_production_metadata_uses_lstat_before_signed_evidence_is_file_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_file = path_type.is_file
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"

            def failing_is_file(path: Path, *args, **kwargs):
                if path == evidence_path:
                    raise OSError("simulated signed evidence is_file failure")
                return original_is_file(path, *args, **kwargs)

            def failing_lstat(path: Path, *args, **kwargs):
                if path == evidence_path:
                    raise OSError("simulated signed evidence lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.is_file = failing_is_file
                path_type.lstat = failing_lstat

                errors, _details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted,
                )
            finally:
                path_type.is_file = original_is_file
                path_type.lstat = original_lstat

        self.assertIn(
            "slot.json signed_evidence_artifact_path references artifact file "
            "metadata could not be read evidence/signed-evidence.json",
            errors,
        )

    def test_metadata_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "evidence/token=supersecret.apk"
            write_text(slot / secret_relative, "must not be hashed\n")

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                secret_relative,
                "slot.json offline_wallet_apk_path",
                "slot.json offline_wallet_apk_path must point to an existing file",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot.json offline_wallet_apk_path must not contain secret-looking material"],
        )
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_metadata_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "evidence/offline-wallet\x1b[31m.apk"

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                control_relative,
                "slot.json offline_wallet_apk_path",
                "slot.json offline_wallet_apk_path must point to an existing file",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path: unsafe path contains "
                "control characters"
            ],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_metadata_artifact_digest_rejects_file_metadata_failure(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "evidence" / "offline-wallet-release.apk"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError("simulated metadata artifact metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path references artifact file metadata "
                "could not be read evidence/offline-wallet-release.apk"
            ],
        )

    def test_metadata_artifact_digest_rejects_oversized_artifact_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "offline-wallet-release.apk"
            with apk_path.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                slot,
                "evidence/offline-wallet-release.apk",
                "slot.json offline_wallet_apk_path",
                "slot.json offline_wallet_apk_path must point to an existing file",
            )

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path references artifact "
                "evidence/offline-wallet-release.apk must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_metadata_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "offline-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = 32
                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                    device_lab._slot_artifact_max_bytes("evidence/offline-wallet-release.apk"),
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(payload, b"x" * 16)
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_metadata_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "evidence" / "offline-wallet-release.apk"

            payload, digest, errors = with_open_failure(
                target,
                lambda: device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                ),
            )

        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot.json offline_wallet_apk_path could not be read"])

    def test_metadata_artifact_digest_rejects_symlink_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                artifact_path = slot / "evidence" / "offline-wallet-release.apk"
                target = root / "outside-release.apk"
                write_text(target, "outside release apk\n")
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        replace_with_symlink(self, artifact_path, target)
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                )
                target_bytes = target.read_bytes()
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(target_bytes, b"outside release apk\n")
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path references symlink artifact "
                "evidence/offline-wallet-release.apk"
            ],
        )

    def test_metadata_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_metadata_artifact_for_read

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                slot = create_slot(root, "slot-a")
                artifact_path = slot / "evidence" / "offline-wallet-release.apk"
                swapped = False

                def swapping_validate(
                    slot_path: Path,
                    relative: str,
                    label: str,
                    missing_error: str,
                ):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                        label,
                        missing_error,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement release apk\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_metadata_artifact_for_read = swapping_validate

                payload, digest, errors = device_lab._metadata_artifact_bytes_and_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                    "slot.json offline_wallet_apk_path",
                    "slot.json offline_wallet_apk_path must point to an existing file",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_metadata_artifact_for_read = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(payload)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement release apk\n")
        self.assertEqual(
            errors,
            [
                "slot.json offline_wallet_apk_path references artifact changed "
                "while being read evidence/offline-wallet-release.apk"
            ],
        )

    def test_production_metadata_rejects_symlinked_signed_evidence_digest_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-signed-evidence.json"
            write_text(target, (slot / "evidence" / "signed-evidence.json").read_text())
            replace_with_symlink(
                self,
                slot / "evidence" / "signed-evidence.json",
                target,
            )

            errors, _details = device_lab.validate_kagemusha_production_metadata(
                slot,
                trusted,
            )

        self.assertIn(
            "slot.json signed_evidence_artifact_path references symlink artifact "
            "evidence/signed-evidence.json",
            errors,
        )

    def test_production_metadata_rejects_hardlinked_release_apk_digest_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            target = root / "outside-release.apk"
            write_text(target, "apk bytes\n")
            replace_with_hardlink(
                self,
                slot / "evidence" / "offline-wallet-release.apk",
                target,
            )

            errors, _details = device_lab.validate_kagemusha_production_metadata(
                slot,
                trusted,
            )

        self.assertIn(
            "slot.json offline_wallet_apk_path references hardlinked artifact "
            "evidence/offline-wallet-release.apk",
            errors,
        )

    def test_production_metadata_rejects_unsafe_signed_evidence_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "../signed-evidence.json"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path: unsafe path '../signed-evidence.json'",
            report["errors"],
        )

    def test_production_metadata_rejects_star_normalized_signed_evidence_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "*evidence/signed-evidence.json"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            (
                "slot.json signed_evidence_artifact_path: unsafe path "
                "'*evidence/signed-evidence.json'"
            ),
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_signed_evidence_path(
        self,
    ) -> None:
        cases = (
            "evidence/./signed-evidence.json",
            "evidence//signed-evidence.json",
            "evidence/signed-evidence.json/",
        )
        for relative in cases:
            with self.subTest(relative=relative):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata_path = slot / "slot.json"
                    metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                    metadata["signed_evidence_artifact_path"] = relative
                    write_json(metadata_path, metadata)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    "slot.json signed_evidence_artifact_path: unsafe path is not canonical",
                    report["errors"],
                )

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_path(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = (
                " evidence/signed-evidence.json "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_control_signed_evidence_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = (
                "evidence/signed-evidence.json\x1b[31m"
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_sha256"] = (
                f" {metadata['signed_evidence_artifact_sha256']} "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_sha256 must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_artifact_outside_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "evidence" / "signed-evidence.json"
            target = slot / "telemetry" / "signed-evidence.json"
            target.write_bytes(source.read_bytes())

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "telemetry/signed-evidence.json"
            metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
                target.read_bytes()
            ).hexdigest()
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must stay under evidence/",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_signed_evidence_filename(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "evidence" / "signed-evidence.json"
            target = slot / "evidence" / "signed-evidence-copy.json"
            target.write_bytes(source.read_bytes())

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = "evidence/signed-evidence-copy.json"
            metadata["signed_evidence_artifact_sha256"] = hashlib.sha256(
                target.read_bytes()
            ).hexdigest()
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json signed_evidence_artifact_path must be evidence/signed-evidence.json",
            report["errors"],
        )

    def test_production_metadata_rejects_unexpected_slot_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["token=supersecret"] = "must not ship"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_redacts_control_unexpected_slot_field(self) -> None:
        unsafe_key = "debug\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata[unsafe_key] = "must not ship"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json contains unexpected field "
            f"{device_lab.CONTROL_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn(unsafe_key, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_unexpected_attestation_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["token=supersecret"] = "must not ship"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"attestation/result.json contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_noncanonical_attestation_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["offline_wallet_policy_sha256"] = "AA" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json offline_wallet_policy_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_virtual_device_attestation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["physical_device_attestation"] = False
            write_json(metadata_path, metadata)
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["physical_device_attestation"] = False
            write_json(attestation_path, attestation)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["physical_device_attestation"] = False
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("slot.json physical_device_attestation must be true", report["errors"])
        self.assertIn(
            "attestation/result.json physical_device_attestation must be true",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact physical_device_attestation must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_wrong_minimum_os_for_device_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel6",
                "Google Pixel 6 / 6a",
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["minimum_os"] = "Android 15"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json minimum_os for Google Pixel 6 / 6a must be Android 14",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_challenge_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("attestation_challenge_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_challenge_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_chain_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("attestation_certificate_chain_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_chain_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "tampered certificate chain\n",
            )
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_chain_summary_file_substitution(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "attestation" / "chain-summary.txt", "summary only\n")
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["attestation_certificate_chain_path"] = "attestation/chain-summary.txt"
            write_json(metadata_path, metadata)
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json attestation_certificate_chain_path must end in .pem or .der",
            report["errors"],
        )

    def test_production_metadata_rejects_malformed_attestation_chain_pem(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "not a pem certificate chain\n",
            )
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation certificate chain PEM must contain certificate boundaries",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_attestation_chain(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "attestation" / "keymint-certificate-chain.pem",
                "-----BEGIN CERTIFICATE-----\n"
                + ("A" * device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES)
                + "\n-----END CERTIFICATE-----\n",
            )
            refresh_attestation_certificate_chain_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation certificate chain must be no more than "
            f"{device_lab.MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES} bytes",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_release_apk_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("offline_wallet_apk_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json offline_wallet_apk_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_release_apk_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "evidence" / "offline-wallet-release.apk", "tampered apk\n")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_d2d_payment_transcript_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("d2d_payment_transcript_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payload_bytes"] = 4096
            write_json(transcript_path, transcript)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_sha256 does not match d2d_payment_transcript_path",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_outside_handoff(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            source = slot / "handoff" / "d2d-payment.json"
            target = slot / "telemetry" / "d2d-payment.json"
            target.write_bytes(source.read_bytes())
            digest = hashlib.sha256(target.read_bytes()).hexdigest()

            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["d2d_payment_transcript_path"] = "telemetry/d2d-payment.json"
            metadata["d2d_payment_transcript_sha256"] = digest
            write_json(metadata_path, metadata)

            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["d2d_payment_transcript_path"] = "telemetry/d2d-payment.json"
            evidence["d2d_payment_transcript_sha256"] = digest
            evidence["artifact_digests"] = required_artifact_digests(slot)
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json d2d_payment_transcript_path must stay under handoff/",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_wallet_integrity_transcript_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("wallet_integrity_transcript_sha256")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json wallet_integrity_transcript_sha256 must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_wallet_integrity_transcript_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["old_key_invalidated"] = False
            write_json(transcript_path, transcript)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json wallet_integrity_transcript_sha256 does not match wallet_integrity_transcript_path",
            report["errors"],
        )

    def test_production_metadata_rejects_pending_queue_shape(self) -> None:
        cases = (
            (
                {"slot_id": "pixel7", "pending_transactions": []},
                "queue/pending_queue.json slot_id must match slot id",
            ),
            (
                {"slot_id": " pixel8 ", "pending_transactions": []},
                "queue/pending_queue.json slot_id must not contain surrounding whitespace",
            ),
            (
                {"slot_id": "pixel8", "pending_transactions": {}},
                "queue/pending_queue.json pending_transactions must be an array",
            ),
            (
                {
                    "slot_id": "pixel8",
                    "pending_transactions": [{"id": "leftover-transfer"}],
                },
                "queue/pending_queue.json pending_transactions must be empty after D2D handoff",
            ),
            (
                {
                    "slot_id": "pixel8",
                    "pending_transactions": [],
                    "debug_note": "not production evidence",
                },
                "queue/pending_queue.json contains unexpected field debug_note",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_json(slot / "queue" / "pending_queue.json", payload)
                    transcript_path = slot / "handoff" / "d2d-payment.json"
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript["queue_after_sha256"] = hashlib.sha256(
                        (slot / "queue" / "pending_queue.json").read_bytes()
                    ).hexdigest()
                    write_json(transcript_path, transcript)
                    refresh_d2d_payment_transcript_hash(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_wallet_integrity_false_rollback_claim(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["stale_snapshot_rejected"] = False
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript stale_snapshot_rejected must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_transcript_strings(self) -> None:
        cases = (
            (
                "d2d_payment_transcript_path",
                "device_family",
                " Google Pixel 8 / 8a / 8 Pro ",
                "d2d payment transcript device_family must not contain surrounding whitespace",
                refresh_d2d_payment_transcript_hash,
            ),
            (
                "d2d_payment_transcript_path",
                "slot_id",
                "pixel8\u0000",
                "d2d payment transcript slot_id must not contain control characters",
                refresh_d2d_payment_transcript_hash,
            ),
            (
                "wallet_integrity_transcript_path",
                "device_family",
                " Google Pixel 8 / 8a / 8 Pro ",
                "wallet integrity transcript device_family must not contain surrounding whitespace",
                refresh_wallet_integrity_transcript_hash,
            ),
            (
                "wallet_integrity_transcript_path",
                "slot_id",
                "pixel8\u0000",
                "wallet integrity transcript slot_id must not contain control characters",
                refresh_wallet_integrity_transcript_hash,
            ),
        )
        for metadata_path_key, field, value, expected_error, refresh in cases:
            with self.subTest(metadata_path_key=metadata_path_key, field=field):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    metadata = json.loads(
                        (slot / "slot.json").read_text(encoding="utf-8")
                    )
                    relative = metadata[metadata_path_key]
                    transcript_path = slot / relative
                    transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
                    transcript[field] = value
                    write_json(transcript_path, transcript)
                    refresh(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_wallet_integrity_unchanged_rotation_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["key_id_after_sha256"] = transcript["key_id_before_sha256"]
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "wallet integrity transcript key_id_before_sha256 must differ from key_id_after_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_secret_field_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["token=supersecret"] = "must not ship"
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"d2d payment transcript contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_d2d_payment_transcript_queue_splice(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["queue_after_sha256"] = "11" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_online_wallets(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payer_wallet_offline"] = False
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payer_wallet_offline must be true",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_attestation_challenge_splice(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["attestation_challenge_sha256"] = "33" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_d2d_payment_transcript_unchanged_payer_wallet_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payer_wallet_state_after_sha256"] = transcript[
                "payer_wallet_state_before_sha256"
            ]
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payer_wallet_state_before_sha256 must differ from payer_wallet_state_after_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_d2d_payment_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["payload_bytes"] = device_lab.MAX_D2D_PAYMENT_PAYLOAD_BYTES + 1
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "d2d payment transcript payload_bytes must be no more than "
            f"{device_lab.MAX_D2D_PAYMENT_PAYLOAD_BYTES}",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_d2d_handoff_raw_command_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stale_commands = [
                command.replace(",org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest", "")
                for command in KAGEMUSHA_ANDROID_RAW_TEST_COMMANDS
            ]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = stale_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = stale_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
            report["errors"],
        )

    def test_production_metadata_rejects_stale_native_bridge_abi_version(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["native_bridge_abi_version"] = (
                device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION - 1
            )
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["native_bridge_abi_version"] = metadata["native_bridge_abi_version"]
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"slot.json native_bridge_abi_version must be {device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION}",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_challenge_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_challenge_sha256"] = "22" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_chain_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_certificate_chain_sha256"] = "44" * 32
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_slot_alias_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["slot"] = "stale-slot"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json slot must match the slot directory name",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_slot_alias(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["slot"] = f" {slot.name} "
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json slot must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_attestation_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["status"] = "OK"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("attestation/result.json status must be ok", report["errors"])

    def test_production_metadata_rejects_attestation_passed_status_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["status"] = "passed"
            write_json(attestation_path, attestation)
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "passed"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("attestation/result.json status must be ok", report["errors"])
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_without_strongbox(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["attestation_security_level"] = "TEE"
            attestation["keymaster_security_level"] = "TEE"
            attestation["keymint_security_level"] = "TEE"
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json must report STRONGBOX security level",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_strongbox_level(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["keymint_security_level"] = " STRONGBOX "
            write_json(attestation_path, attestation)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json keymint_security_level must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_slot_keymint_level(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["keymint_security_level"] = "strongbox"
            write_json(metadata_path, metadata)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json keymint_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_result_slot_keymint_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["keymint_security_level"] = "STRONG_BOX"
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["keymint_security_level"] = "STRONG_BOX"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/result.json keymint_security_level must match "
            "slot.json keymint_security_level",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_report(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            (slot / "attestation" / "report.json").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn("missing attestation/report.json", report["errors"])

    def test_production_metadata_rejects_attestation_report_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["attestation_certificate_chain_sha256"] = "55" * 32
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_report_binding(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["device_fingerprint"] = (
                f" {attestation_report['device_fingerprint']} "
            )
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json device_fingerprint must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_without_strongbox(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["keymint_security_level"] = "TEE"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.keymint_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_attestation_report_strongbox(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["keymint_security_level"] = " STRONGBOX "
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.keymint_security_level "
            "must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_attestation_report_level_fields(
        self,
    ) -> None:
        for level_key in ("attestation_security_level", "keymaster_security_level"):
            with self.subTest(level_key=level_key):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    report_path = slot / "attestation" / "report.json"
                    attestation_report = json.loads(
                        report_path.read_text(encoding="utf-8")
                    )
                    del attestation_report["verification"][level_key]
                    write_json(report_path, attestation_report)
                    resign_signed_evidence_artifacts(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"attestation/report.json verification.{level_key} "
                    "must be a non-empty string",
                    report["errors"],
                )

    def test_production_metadata_rejects_attestation_report_result_level_mismatch(
        self,
    ) -> None:
        for level_key in (
            "keymint_security_level",
            "attestation_security_level",
            "keymaster_security_level",
        ):
            with self.subTest(level_key=level_key):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    report_path = slot / "attestation" / "report.json"
                    attestation_report = json.loads(
                        report_path.read_text(encoding="utf-8")
                    )
                    attestation_report["verification"][level_key] = "STRONG_BOX"
                    write_json(report_path, attestation_report)
                    resign_signed_evidence_artifacts(slot, signer)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(
                    f"attestation/report.json verification.{level_key} must match "
                    f"attestation/result.json {level_key}",
                    report["errors"],
                )

    def test_production_metadata_rejects_attestation_report_result_status_mismatch(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "passed"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.status must match "
            "attestation/result.json status",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_unexpected_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["unexpected"] = "drift"
            attestation_report["verification"]["debug"] = True
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json contains unexpected field unexpected",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification contains unexpected field debug",
            report["errors"],
        )

    def test_production_metadata_rejects_attestation_report_weak_verifier_status(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verifier"] = "token=supersecret"
            attestation_report["verification"]["status"] = "failed"
            attestation_report["verification"]["physical_device_attestation"] = False
            attestation_report["verification"]["keymaster_security_level"] = "TEE"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verifier must not contain secret-looking material",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.physical_device_attestation must be true",
            report["errors"],
        )
        self.assertIn(
            "attestation/report.json verification.keymaster_security_level must be STRONGBOX",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_attestation_report_status(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            report_path = slot / "attestation" / "report.json"
            attestation_report = json.loads(report_path.read_text(encoding="utf-8"))
            attestation_report["verification"]["status"] = "OK"
            write_json(report_path, attestation_report)
            resign_signed_evidence_artifacts(slot, signer)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "attestation/report.json verification.status must be ok",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_challenge_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "attestation_challenge_sha256",
                    "11" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact attestation_challenge_sha256 must match slot.json attestation_challenge_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_attestation_chain_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "attestation_certificate_chain_sha256",
                    "55" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact attestation_certificate_chain_sha256 must match slot.json attestation_certificate_chain_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_apk_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("offline_wallet_apk_sha256", "11" * 32),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact offline_wallet_apk_sha256 must match slot.json offline_wallet_apk_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_d2d_transcript_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "d2d_payment_transcript_sha256",
                    "11" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact d2d_payment_transcript_sha256 must match slot.json d2d_payment_transcript_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_wallet_integrity_digest_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "wallet_integrity_transcript_sha256",
                    "66" * 32,
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact wallet_integrity_transcript_sha256 must match slot.json wallet_integrity_transcript_sha256",
            report["errors"],
        )

    def test_production_metadata_rejects_unexpected_signed_evidence_fields_with_redaction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["token=supersecret"] = "must not ship"
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        rendered = "\n".join(report["errors"])
        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact contains unexpected field {device_lab.SECRET_PATH_REDACTION}",
            rendered,
        )
        self.assertNotIn("token=supersecret", rendered)

    def test_production_metadata_rejects_signed_evidence_probe_state_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["abi6_recursive_spend_jni_probe"] = "ok"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact abi6_recursive_spend_jni_probe must match slot.json abi6_recursive_spend_jni_probe",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_raw_command_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = [
                "./gradlew :offline-wallet-android:connectedDebugAndroidTest --rerun"
            ]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact raw_test_commands must match slot.json raw_test_commands",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_raw_test_command_strings(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            commands = list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS)
            commands[0] = f" {commands[0]} "
            commands[1] = f"{commands[1]}\x1b[31m"
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )
            rendered = "\n".join(report["errors"])

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands[0] must not contain surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "slot.json raw_test_commands[1] must not contain control characters",
            rendered,
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands[0] must not contain surrounding whitespace",
            rendered,
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands[1] must not contain control characters",
            rendered,
        )
        self.assertNotIn("\x1b", rendered)

    def test_production_metadata_rejects_irrelevant_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            unrelated_commands = ["./gradlew test --tests unrelated.HealthCheck"]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = unrelated_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = unrelated_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:connectedDebugAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include :client-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-lab-app:installReleaseAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include adb shell am instrument",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
            report["errors"],
        )
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :offline-wallet-android:connectedDebugAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :client-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :offline-wallet-android:assembleRelease",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include :offline-wallet-lab-app:installReleaseAndroidTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include adb shell am instrument",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must include org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
            report["errors"],
        )

    def test_production_metadata_rejects_marker_stuffed_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stuffed_commands = [
                (
                    "echo :client-android:assembleRelease "
                    ":offline-wallet-android:assembleRelease "
                    ":offline-wallet-android:connectedDebugAndroidTest "
                    ":offline-wallet-lab-app:assembleRelease "
                    ":offline-wallet-lab-app:installRelease "
                    ":offline-wallet-lab-app:installReleaseAndroidTest "
                    "adb shell am instrument "
                    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest "
                    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest "
                    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest"
                )
            ]
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = stuffed_commands
            write_json(metadata_path, metadata)
            evidence_path = slot / "evidence" / "signed-evidence.json"
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            evidence["raw_test_commands"] = stuffed_commands
            write_json(evidence_path, sign_evidence(evidence, signer))
            refresh_signed_evidence_hash(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "slot.json raw_test_commands must exactly match the Kagemusha Android production raw test command",
            report["errors"],
        )
        self.assertIn(
            "signed evidence artifact raw_test_commands must exactly match the Kagemusha Android production raw test command",
            report["errors"],
        )
        for marker in device_lab.RAW_TEST_COMMAND_REQUIRED_MARKERS:
            self.assertNotIn(
                f"slot.json raw_test_commands must include {marker}",
                report["errors"],
            )

    def test_production_metadata_rejects_noncanonical_signed_evidence_timestamp(self) -> None:
        for timestamp, expected_error in (
            (
                "2026-06-06T00:00:00+00:00",
                "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            ),
            (
                " 2026-06-06T00:00:00Z ",
                "signed evidence artifact signed_at_utc must not contain surrounding whitespace",
            ),
        ):
            with self.subTest(timestamp=timestamp):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    evidence_path = slot / "evidence" / "signed-evidence.json"
                    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
                    evidence["signed_at_utc"] = timestamp
                    write_json(evidence_path, sign_evidence(evidence, signer))
                    refresh_signed_evidence_hash(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_signed_evidence_schema_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("schema", "iroha.android.device_lab.legacy"),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact schema must be {device_lab.SIGNED_EVIDENCE_SCHEMA}",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("device_family", "Samsung Galaxy S24"),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must match slot.json device_family",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_slot_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "device_family",
                    f" {metadata['device_family']} ",
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_control_signed_evidence_slot_field(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__(
                    "device_family",
                    f"{metadata['device_family']}\x1b[31m",
                ),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact device_family must not contain control characters",
            report["errors"],
        )
        self.assertNotIn("\x1b", "\n".join(report["errors"]))

    def test_production_metadata_rejects_whitespace_normalized_signed_evidence_algorithm(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature_algorithm", " ed25519 "),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature_algorithm must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_digest_map_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def corrupt_runtime_log_digest(evidence: dict) -> None:
                evidence["artifact_digests"]["logs/runtime.log"] = "00" * 32

            mutate_signed_evidence(slot, corrupt_runtime_log_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact digest mismatch for logs/runtime.log",
            report["errors"],
        )

    def test_signed_evidence_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest path must not contain "
                "secret-looking material"
            ],
        )
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signed_evidence_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest path: unsafe path contains "
                "control characters"
            ],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_signed_evidence_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references symlink artifact "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references hardlinked artifact "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = device_lab._signed_evidence_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact "
                "logs/runtime.log must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_signed_evidence_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            apk_path = slot / "evidence" / "offline-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = 32
                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_signed_evidence_artifact_digest_rejects_file_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                target = slot / "logs" / "runtime.log"

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == target:
                        raise OSError(
                            "simulated signed evidence artifact metadata failure"
                        )
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact file metadata "
                "could not be read logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "slot-a")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact that could not be read "
                "logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = device_lab._validate_signed_evidence_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "slot-a")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                device_lab._validate_signed_evidence_artifact_for_digest = (
                    swapping_validate
                )

                digest, errors = device_lab._signed_evidence_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            device_lab._validate_signed_evidence_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            [
                "signed evidence artifact digest references artifact changed while "
                "being read logs/runtime.log"
            ],
        )

    def test_signed_evidence_artifact_revalidates_required_digest_before_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root)
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            evidence_path = slot / "evidence" / "signed-evidence.json"
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_validate = device_lab.validate_required_kagemusha_slot_artifact_shapes

            def validate_then_alias(
                slot_path: Path,
                errors: list[str],
                *args: object,
                **kwargs: object,
            ) -> None:
                original_validate(slot_path, errors, *args, **kwargs)
                if not errors:
                    replace_with_symlink(
                        self,
                        slot_path / "logs" / "runtime.log",
                        target,
                    )

            errors: list[str] = []
            try:
                device_lab.validate_required_kagemusha_slot_artifact_shapes = (
                    validate_then_alias
                )
                device_lab.validate_signed_evidence_artifact(
                    slot,
                    evidence_path,
                    metadata,
                    trusted,
                    errors,
                )
            finally:
                device_lab.validate_required_kagemusha_slot_artifact_shapes = (
                    original_validate
                )

        self.assertIn(
            "signed evidence artifact digest references symlink artifact "
            "logs/runtime.log",
            errors,
        )
        self.assertNotIn(
            "signed evidence artifact digest mismatch for logs/runtime.log",
            errors,
        )

    def test_production_metadata_rejects_signed_evidence_missing_required_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def remove_runtime_log_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop("logs/runtime.log")

            mutate_signed_evidence(slot, remove_runtime_log_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact artifact_digests[logs/runtime.log] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_release_apk_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            apk_path = metadata["offline_wallet_apk_path"]

            def remove_release_apk_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop(apk_path, None)

            mutate_signed_evidence(slot, remove_release_apk_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact artifact_digests[{apk_path}] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_attestation_chain_digest(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            chain_path = metadata["attestation_certificate_chain_path"]

            def remove_attestation_chain_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop(chain_path, None)

            mutate_signed_evidence(slot, remove_attestation_chain_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            f"signed evidence artifact artifact_digests[{chain_path}] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_required_slot_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            (slot / "logs" / "runtime.log").unlink()
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact required slot artifact is missing logs/runtime.log",
            report["errors"],
        )

    def test_production_metadata_rejects_empty_required_slot_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "logs" / "runtime.log", "")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "required slot artifact logs/runtime.log must be non-empty",
            report["errors"],
        )

    def test_production_metadata_rejects_oversized_required_slot_artifact(self) -> None:
        old_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
        try:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
            with tempfile.TemporaryDirectory() as temp:
                signer = create_test_signer(Path(temp))
                trusted = trusted_signers_for(signer)
                slot = create_slot(
                    Path(temp),
                    "pixel8",
                    device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                    signer,
                )
                write_text(slot / "logs" / "runtime.log", "runtime log too large\n")
                rewrite_sha256sum(slot)

                report = device_lab.scan_slot(
                    slot,
                    require_kagemusha_production_evidence=True,
                    trusted_signer_public_keys=trusted,
                )
        finally:
            device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_limit

        self.assertEqual(report["status"], "error")
        self.assertTrue(
            any(
                "logs/runtime.log must be no more than 8 bytes" in error
                for error in report["errors"]
            ),
            report["errors"],
        )

    def test_production_metadata_rejects_telemetry_slot_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["slot_id"] = "other-slot"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json slot_id must match the slot directory name",
            report["errors"],
        )

    def test_production_metadata_rejects_whitespace_normalized_telemetry_slot(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["slot_id"] = " pixel8 "
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json slot_id must not contain surrounding whitespace",
            report["errors"],
        )

    def test_production_metadata_rejects_telemetry_extra_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["debug_note"] = "not production evidence"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json contains unexpected field debug_note",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_telemetry_identity_strings(
        self,
    ) -> None:
        cases = (
            (
                "device_model",
                " Pixel 8 ",
                "telemetry/telemetry.json device_model must not contain surrounding whitespace",
            ),
            (
                "device_codename",
                "husky\u0000",
                "telemetry/telemetry.json device_codename must not contain control characters",
            ),
            (
                "app_package_name",
                "",
                "telemetry/telemetry.json app_package_name must be a non-empty string",
            ),
            (
                "app_package_name",
                "token=not-for-telemetry",
                "telemetry/telemetry.json app_package_name must not contain secret-looking material",
            ),
        )
        for field, value, expected_error in cases:
            with self.subTest(field=field, expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry[field] = value
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_telemetry_app_package_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            telemetry_path = slot / "telemetry" / "telemetry.json"
            telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
            telemetry["app_package_name"] = "org.hyperledger.iroha.android.other"
            write_json(telemetry_path, telemetry)
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/telemetry.json app_package_name must match slot.json app_package_name",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_telemetry_slot_binding(
        self,
    ) -> None:
        cases = (
            (
                "pixel8\u0000",
                "telemetry/telemetry.json slot_id must not contain control characters",
            ),
            (
                "",
                "telemetry/telemetry.json slot_id must be a non-empty string",
            ),
            (
                8,
                "telemetry/telemetry.json slot_id must be a non-empty string",
            ),
        )
        for slot_value, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry["slot_id"] = slot_value
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_noncanonical_telemetry_suite(self) -> None:
        cases = (
            (
                "KAGEMUSHA-DEVICE-LAB",
                "telemetry/telemetry.json suite must identify a Kagemusha device-lab run",
            ),
            (
                " kagemusha-device-lab ",
                "telemetry/telemetry.json suite must not contain surrounding whitespace",
            ),
            (
                "kagemusha-device-lab\u0000",
                "telemetry/telemetry.json suite must not contain control characters",
            ),
            (
                "",
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
            (
                7,
                "telemetry/telemetry.json suite must be a non-empty string",
            ),
        )
        for suite, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    telemetry_path = slot / "telemetry" / "telemetry.json"
                    telemetry = json.loads(telemetry_path.read_text(encoding="utf-8"))
                    telemetry["suite"] = suite
                    write_json(telemetry_path, telemetry)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_failed_status_ndjson(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"failed"}\n')
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            report["errors"],
        )
        self.assertIn(
            "telemetry/status.ndjson must contain at least one ok status",
            report["errors"],
        )

    def test_production_metadata_rejects_status_ndjson_unexpected_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "telemetry" / "status.ndjson",
                '{"status":"ok","slot_id":"pixel8","debug_note":"ignored"}\n',
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 contains unexpected field debug_note",
            report["errors"],
        )

    def test_production_metadata_rejects_unknown_status_ndjson(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "telemetry" / "status.ndjson",
                (
                    '{"status":"ok","slot_id":"pixel8"}\n'
                    '{"status":"skipped","slot_id":"pixel8"}\n'
                ),
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 2 status must be ok",
            report["errors"],
        )

    def test_production_metadata_requires_status_ndjson_slot_id(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"ok"}\n')
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "telemetry/status.ndjson line 1 slot_id must be a non-empty string",
            report["errors"],
        )

    def test_production_metadata_rejects_noncanonical_status_ndjson(self) -> None:
        cases = (
            (
                ' {"status":"ok","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"} \n',
                "telemetry/status.ndjson line 1 must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"}\r\n',
                "telemetry/status.ndjson must use LF line endings",
            ),
            (
                '{"status":"ok","slot_id":"pixel8"}',
                "telemetry/status.ndjson must end with a trailing newline",
            ),
            (
                '{"status":"OK","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must be lowercase",
            ),
            (
                '{"status":" ok ","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok\\u0000","slot_id":"pixel8"}\n',
                "telemetry/status.ndjson line 1 status must not contain control characters",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_text(slot / "telemetry" / "status.ndjson", payload)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])
                if not expected_error.startswith("telemetry/status.ndjson must "):
                    self.assertIn(
                        "telemetry/status.ndjson must contain at least one ok status",
                        report["errors"],
                    )

    def test_production_metadata_rejects_status_ndjson_slot_mismatch(self) -> None:
        cases = (
            (
                '{"status":"ok","slot_id":" pixel8 "}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain surrounding whitespace",
            ),
            (
                '{"status":"ok","slot_id":"pixel8\\u0000"}\n',
                "telemetry/status.ndjson line 1 slot_id must not contain control characters",
            ),
            (
                '{"status":"ok","slot_id":8}\n',
                "telemetry/status.ndjson line 1 slot_id must be a string",
            ),
        )
        for payload, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    signer = create_test_signer(Path(temp))
                    trusted = trusted_signers_for(signer)
                    slot = create_slot(
                        Path(temp),
                        "pixel8",
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                        signer,
                    )
                    write_text(slot / "telemetry" / "status.ndjson", payload)
                    rewrite_sha256sum(slot)

                    report = device_lab.scan_slot(
                        slot,
                        require_kagemusha_production_evidence=True,
                        trusted_signer_public_keys=trusted,
                    )

                self.assertEqual(report["status"], "error")
                self.assertIn(expected_error, report["errors"])

    def test_production_metadata_rejects_runtime_log_without_completion_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(slot / "logs" / "runtime.log", "device-lab run started\n")
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "logs/runtime.log must contain Kagemusha device-lab completion marker",
            report["errors"],
        )

    def test_production_metadata_rejects_runtime_log_failure_marker(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            write_text(
                slot / "logs" / "runtime.log",
                "kagemusha device-lab run complete\nBUILD FAILED\n",
            )
            rewrite_sha256sum(slot)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "logs/runtime.log must not contain failure marker BUILD FAILED",
            report["errors"],
        )

    def test_production_metadata_rejects_signed_evidence_missing_handoff_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            def remove_handoff_digest(evidence: dict) -> None:
                evidence["artifact_digests"].pop("handoff/d2d-payment.json")

            mutate_signed_evidence(slot, remove_handoff_digest)

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact artifact_digests[handoff/d2d-payment.json] must be lowercase sha256 hex",
            report["errors"],
        )

    def test_production_metadata_rejects_missing_trusted_signer_public_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(slot, require_kagemusha_production_evidence=True)

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "trusted signer public key required for Kagemusha production evidence",
            report["errors"],
        )

    def test_production_metadata_rejects_untrusted_signed_evidence_key(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "signer-a")
            other_signer = create_test_signer(root / "signer-b")
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted_signers_for(other_signer),
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signer_public_key_sha256 must match a trusted signer public key",
            report["errors"],
        )

    def test_production_metadata_rejects_trusted_signer_public_key_symlinked_ancestor_from_direct_map(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = root / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(real_parent)
            trusted = {signer["public_key_sha256"]: linked_public_key}
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "trusted signer public key ancestor directory must not be a symlink",
            report["errors"],
        )
        rendered = "\n".join(report["errors"])
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_production_metadata_rejects_control_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "pixel8"
            trusted = {"0" * 64: Path(temp) / "control\nsigner.pem"}

            with mock.patch.object(
                device_lab,
                "_load_json",
                side_effect=AssertionError("slot metadata must not be read"),
            ) as load_json:
                errors, details = device_lab.validate_kagemusha_production_metadata(
                    slot,
                    trusted_signer_public_keys=trusted,
                )

            rendered = "\n".join(errors)

        load_json.assert_not_called()
        self.assertEqual(details, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain control characters"],
        )
        self.assertNotIn("control\nsigner.pem", rendered)

    def test_production_metadata_rejects_alias_trusted_signer_map_before_metadata_read(
        self,
    ) -> None:
        cases = (
            (
                Path("keys") / ".." / "signer.pem",
                "trusted signer public key path must be canonical",
            ),
            (
                Path("keys\\signer.pem"),
                "trusted signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for key_path, expected_error in cases:
                key_path = root / key_path
                with self.subTest(key_path=key_path):
                    slot = root / "pixel8"
                    trusted = {"0" * 64: key_path}

                    with mock.patch.object(
                        device_lab,
                        "_load_json",
                        side_effect=AssertionError("slot metadata must not be read"),
                    ) as load_json:
                        errors, details = device_lab.validate_kagemusha_production_metadata(
                            slot,
                            trusted_signer_public_keys=trusted,
                        )
                    rendered = "\n".join(errors)

                    load_json.assert_not_called()
                    self.assertEqual(details, {})
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(key_path), rendered)

    def test_production_metadata_rejects_signed_evidence_payload_hash_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature_payload_sha256", "00" * 32),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature_payload_sha256 mismatch",
            report["errors"],
        )

    def test_signed_evidence_canonical_payload_rejects_nonfinite_json(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            errors: list[str] = []

            with mock.patch.object(
                device_lab,
                "_load_json",
                return_value={
                    "schema": device_lab.SIGNED_EVIDENCE_SCHEMA,
                    "nonfinite": float("nan"),
                },
            ):
                device_lab.validate_signed_evidence_artifact(
                    Path(temp),
                    Path(temp) / "signed-evidence.json",
                    {},
                    {},
                    errors,
                )

        self.assertIn(
            "signed evidence artifact signature payload is not strict JSON",
            errors,
        )

    def test_production_metadata_rejects_signed_evidence_signature_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp))
            trusted = trusted_signers_for(signer)
            slot = create_slot(
                Path(temp),
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            mutate_signed_evidence(
                slot,
                lambda evidence: evidence.__setitem__("signature", "00" * 64),
            )

            report = device_lab.scan_slot(
                slot,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys=trusted,
            )

        self.assertEqual(report["status"], "error")
        self.assertIn(
            "signed evidence artifact signature verification failed",
            report["errors"],
        )

    def test_standard_matrix_requires_every_kagemusha_device_family(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 1)

    def test_signer_helper_generates_validator_accepted_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel6")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            )

            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                sign_status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
                validate_status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        "pixel6",
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

            evidence = json.loads(
                (slot / "evidence" / "signed-evidence.json").read_text(encoding="utf-8")
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))

        self.assertEqual(sign_status, 0)
        self.assertEqual(validate_status, 0)
        self.assertEqual(evidence["signer_public_key_sha256"], signer["public_key_sha256"])
        self.assertEqual(
            metadata["signed_evidence_artifact_sha256"],
            hashlib.sha256(
                json.dumps(evidence, indent=2, sort_keys=True).encode("utf-8") + b"\n"
            ).hexdigest(),
        )

    def test_signer_helper_rejects_nonfinite_canonical_payload_before_signing(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel6")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel6",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            errors: list[str] = []

            with (
                mock.patch.object(
                    evidence_signer,
                    "_artifact_digests",
                    return_value={"nonfinite": float("nan")},
                ),
                mock.patch.object(
                    evidence_signer,
                    "_sign_ed25519",
                    side_effect=AssertionError("signature should not be attempted"),
                ),
            ):
                evidence = evidence_signer.build_signed_evidence(
                    slot,
                    metadata,
                    private_key_path=signer["private_key"],
                    public_key_path=signer["public_key"],
                    signer_key_id="android-lab-release-signer-v1",
                    signed_at_utc="2026-06-06T00:00:00Z",
                    errors=errors,
                )

        self.assertIsNone(evidence)
        self.assertEqual(errors, ["signed evidence payload is not strict JSON"])

    def test_signer_helper_rejects_mismatched_private_and_public_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            private_signer = create_test_signer(Path(temp) / "private-signer")
            public_signer = create_test_signer(Path(temp) / "public-signer")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(private_signer["private_key"]),
                        "--public-key",
                        str(public_signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "private key did not produce a signature accepted by the signer public key",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_trusted_signer_public_key_rejects_symlink_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            public_key_link = root / "trusted-public-key-link.pem"
            try:
                public_key_link.symlink_to(signer["public_key"])
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [public_key_link]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(errors, ["trusted signer public key must not be a symlink"])
        self.assertNotIn(str(public_key_link), "\n".join(errors))

    def test_trusted_signer_public_key_rejects_secret_looking_path_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_public_key = Path(temp) / "token=supersecret-public.pem"
            secret_public_key.write_text("not a public key\n", encoding="utf-8")

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [secret_public_key]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain secret-looking material"],
        )
        rendered = "\n".join(errors)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_secret_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"

                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [secret_public_key]
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_control_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                control_public_key = Path(temp) / "control\npublic.pem"

                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [control_public_key]
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key path must not contain control characters"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(control_public_key), rendered)

    def test_trusted_signer_public_key_rejects_aliases_before_openssl_lookup(
        self,
    ) -> None:
        cases = (
            (
                Path("keys") / ".." / "trusted-public.pem",
                "trusted signer public key path must be canonical",
            ),
            (
                Path("keys\\trusted-public.pem"),
                "trusted signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for public_key, expected_error in cases:
                public_key = root / public_key
                with self.subTest(public_key=public_key):
                    with mock.patch.object(
                        device_lab.shutil,
                        "which",
                        side_effect=AssertionError("OpenSSL lookup must not run"),
                    ):
                        trusted, errors = device_lab.load_trusted_signer_public_keys(
                            [public_key]
                        )
                    rendered = "\n".join(errors)

                    self.assertEqual(trusted, {})
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(public_key), rendered)

    def test_verify_signature_rejects_secret_public_key_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=secret_public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertEqual(
            errors,
            ["signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_openssl_public_key_der_rejects_spawn_failure_after_path_shape(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key OpenSSL public key command could not be run"],
        )

    def test_openssl_public_key_der_rejects_invalid_public_key_after_openssl_failure(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise subprocess.CalledProcessError(1, args[0])

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("invalid public key data\n", encoding="utf-8")
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key must be a valid OpenSSL public key"],
        )

    def test_openssl_public_key_der_rejects_missing_public_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a missing public key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                missing_public_key = Path(temp) / "missing-public.pem"
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    missing_public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key must point to an existing public key file"],
        )

    def test_openssl_public_key_der_rejects_non_regular_public_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a public key directory")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key_directory = Path(temp) / "public-directory.pem"
                public_key_directory.mkdir()
                errors: list[str] = []

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key_directory,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(der)
        self.assertEqual(errors, ["trusted signer public key must be a regular file"])

    def test_openssl_public_key_der_rejects_file_metadata_failure_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after public key metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == public_key:
                        raise OSError("simulated public key lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                der = device_lab._openssl_public_key_der(  # type: ignore[attr-defined]
                    public_key,
                    errors=errors,
                    label="trusted signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.lstat = original_lstat

        self.assertIsNone(der)
        self.assertEqual(
            errors,
            ["trusted signer public key file metadata could not be read"],
        )
        self.assertNotIn(str(public_key), "\n".join(errors))

    def test_verify_signature_rejects_staging_write_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging write failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                with mock.patch.object(
                    device_lab.os,
                    "fsync",
                    side_effect=OSError("simulated payload staging fsync failure"),
                ):
                    device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                        public_key_path=public_key,
                        payload=b"payload",
                        signature=b"signature",
                        errors=errors,
                        label="signer public key",
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(
            errors,
            ["signature verification staging files could not be written"],
        )

    def test_verify_signature_rejects_payload_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_read_staged_bytes = device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "payload.bin":
                return b"mutated payload", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab._read_staged_bytes = drifting_payload_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertEqual(
            errors,
            ["signature verification staged payload did not match input"],
        )

    def test_verify_signature_rejects_signature_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_read_staged_bytes = device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_signature_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "signature.bin":
                return b"mutated signature", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab._read_staged_bytes = drifting_signature_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertEqual(
            errors,
            ["signature verification staged signature did not match input"],
        )

    def test_write_staged_bytes_rejects_regular_file_swap_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stage = root / "payload.bin"
            replacement = root / "replacement.bin"
            replacement.write_bytes(b"replacement")
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == stage and "r" in mode and not swapped:
                    replacement.replace(stage)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = device_lab._write_staged_bytes(  # type: ignore[attr-defined]
                    stage,
                    b"payload",
                    write_error="stage could not be written",
                    verification_error="stage readback mismatch",
                )
            stage_payload = stage.read_bytes()

        self.assertEqual(errors, ["stage readback mismatch"])
        self.assertEqual(stage_payload, b"replacement")

    def test_write_staged_bytes_rejects_hardlink_created_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            stage = root / "payload.bin"
            alias = root / "payload-hardlink.bin"
            linked = False

            def hardlinking_open(path: Path, *args, **kwargs):
                nonlocal linked
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == stage and "r" in mode and not linked:
                    try:
                        os.link(stage, alias)
                    except (AttributeError, NotImplementedError, OSError) as exc:
                        self.skipTest(
                            f"hardlinks are not available in this test environment: {exc}"
                        )
                    linked = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", hardlinking_open):
                errors = device_lab._write_staged_bytes(  # type: ignore[attr-defined]
                    stage,
                    b"payload",
                    write_error="stage could not be written",
                    verification_error="stage readback mismatch",
                )
            link_count = stage.stat().st_nlink

        self.assertEqual(errors, ["stage readback mismatch"])
        self.assertGreater(link_count, 1)

    def test_verify_signature_rejects_tempdir_failure_before_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run
        original_tempdir = device_lab.tempfile.TemporaryDirectory

        def failing_tempdir(*args, **kwargs):
            raise OSError("simulated temporary directory failure")

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after tempdir failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = unexpected_run
            device_lab.tempfile.TemporaryDirectory = failing_tempdir
            with original_tempdir() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run
            device_lab.tempfile.TemporaryDirectory = original_tempdir

        self.assertEqual(
            errors,
            ["signature verification temporary directory could not be created"],
        )

    def test_verify_signature_rejects_spawn_failure_after_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = device_lab.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            device_lab.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                public_key = Path(temp) / "public.pem"
                public_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                device_lab._verify_ed25519_signature(  # type: ignore[attr-defined]
                    public_key_path=public_key,
                    payload=b"payload",
                    signature=b"signature",
                    errors=errors,
                    label="signer public key",
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            device_lab.subprocess.run = original_run

        self.assertEqual(errors, ["signature verification command could not be run"])

    def test_private_public_pair_preserves_public_key_path_error_before_mismatch(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_public_key = Path(temp) / "token=supersecret-public.pem"
                errors: list[str] = []

                evidence_signer._validate_private_public_pair(  # type: ignore[attr-defined]
                    secret_public_key,
                    b"payload",
                    b"signature",
                    errors,
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertEqual(
            errors,
            ["signer public key path must not contain secret-looking material"],
        )
        self.assertNotIn(
            "private key did not produce a signature accepted by the signer public key",
            rendered,
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_trusted_signer_public_key_rejects_symlinked_ancestor_without_path_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = root / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(real_parent)

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [linked_public_key]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key ancestor directory must not be a symlink"],
        )
        rendered = "\n".join(errors)
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)

    def test_trusted_signer_public_key_rejects_hardlink_without_path_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            public_key_link = root / "trusted-public-key-hardlink.pem"
            write_text(public_key_link, "placeholder\n")
            replace_with_hardlink(self, public_key_link, signer["public_key"])

            trusted, errors = device_lab.load_trusted_signer_public_keys(
                [public_key_link]
            )

        self.assertEqual(trusted, {})
        self.assertEqual(errors, ["trusted signer public key must not be hardlinked"])
        self.assertNotIn(str(public_key_link), "\n".join(errors))

    def test_trusted_signer_public_key_rejects_hardlink_metadata_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_stat = path_type.stat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                signer = create_test_signer(Path(temp) / "keys")
                public_key = signer["public_key"]
                public_key_stat_calls = 0

                def failing_public_key_stat(path: Path, *args, **kwargs):
                    nonlocal public_key_stat_calls
                    if path == public_key and kwargs.get("follow_symlinks", True):
                        public_key_stat_calls += 1
                        if public_key_stat_calls > 0:
                            raise OSError("simulated public key stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_public_key_stat
                trusted, errors = device_lab.load_trusted_signer_public_keys(
                    [public_key]
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.stat = original_stat

        self.assertEqual(trusted, {})
        self.assertEqual(
            errors,
            ["trusted signer public key hardlink metadata could not be read"],
        )
        self.assertNotIn(str(public_key), "\n".join(errors))

    def test_signer_helper_rejects_symlinked_private_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            private_key_link = Path(temp) / "signing-key-link.pem"
            try:
                private_key_link.symlink_to(signer["private_key"])
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(private_key_link),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("private key must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_private_key_ancestor_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            real_parent = Path(temp) / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = Path(temp) / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_private_key = linked_parent / signer["private_key"].relative_to(
                real_parent
            )
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(linked_private_key),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn("private key ancestor directory must not be a symlink", rendered)
        self.assertNotIn(str(linked_private_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_public_key_ancestor_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            real_parent = Path(temp) / "real-keys"
            signer = create_test_signer(real_parent / "keys")
            linked_parent = Path(temp) / "linked-keys"
            create_dir_symlink(self, linked_parent, real_parent)
            linked_public_key = linked_parent / signer["public_key"].relative_to(
                real_parent
            )
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(linked_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        rendered = stderr.getvalue()
        self.assertEqual(status, 1)
        self.assertIn(
            "signer public key ancestor directory must not be a symlink",
            rendered,
        )
        self.assertNotIn(str(linked_public_key), rendered)
        self.assertNotIn(str(real_parent), rendered)
        self.assertNotIn(str(linked_parent), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_public_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            public_key_link = Path(temp) / "public-key-hardlink.pem"
            write_text(public_key_link, "placeholder\n")
            replace_with_hardlink(self, public_key_link, signer["public_key"])

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(public_key_link),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("signer public key must not be hardlinked", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_public_key_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_public_key = Path(temp) / "token=supersecret-public.pem"
            secret_public_key.write_text("not an ed25519 public key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(secret_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signer public key path must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(str(secret_public_key), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_control_public_key_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            control_public_key = Path(temp) / "control\npublic.pem"
            control_public_key.write_text("not an ed25519 public key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(control_public_key),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer public key path must not contain control characters", rendered)
        self.assertNotIn(str(control_public_key), rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_key_aliases_before_metadata_read(self) -> None:
        cases = (
            (
                "private",
                Path("keys") / ".." / "private.pem",
                "private key path must be canonical",
            ),
            (
                "private",
                Path("keys\\private.pem"),
                "private key path must not contain backslashes",
            ),
            (
                "public",
                Path("keys") / ".." / "public.pem",
                "signer public key path must be canonical",
            ),
            (
                "public",
                Path("keys\\public.pem"),
                "signer public key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "pixel8"
            for key_kind, key_path, expected_error in cases:
                key_path = root / key_path
                private_key = key_path if key_kind == "private" else root / "private.pem"
                public_key = key_path if key_kind == "public" else root / "public.pem"
                with self.subTest(key_kind=key_kind, key_path=key_path):
                    with mock.patch.object(
                        evidence_signer,
                        "_require_slot_metadata",
                        side_effect=AssertionError("slot metadata must not be read"),
                    ) as require_slot_metadata:
                        status, output_relative, errors = evidence_signer.sign_slot_evidence(
                            slot_path=slot,
                            private_key_path=private_key,
                            public_key_path=public_key,
                            signer_key_id="android-lab-release-signer-v1",
                            signed_at_utc="2026-06-06T00:00:00Z",
                            output=None,
                            update_slot_json=False,
                            update_sha256sum=False,
                        )
                    rendered = "\n".join(errors)

                    require_slot_metadata.assert_not_called()
                    self.assertEqual(status, 1)
                    self.assertIsNone(output_relative)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn(str(key_path), rendered)

    def test_signer_helper_rejects_secret_looking_slot_path_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            secret_slot = Path(temp) / "token=supersecret-slot"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(secret_slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("slot path must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_slot), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_slot_path_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            control_slot = Path(temp) / "control\nslot"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(control_slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("slot path must not contain control characters", rendered)
        self.assertNotIn(str(control_slot), rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_alias_slot_path_before_metadata_read(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "slot\\alias",
                "slot path must not contain backslashes",
            ),
            (
                lambda base: base / "slot" / ".." / "alias",
                "slot path must be canonical",
            ),
        )
        path_type = type(Path("."))
        original_lstat = path_type.lstat
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    root = Path(temp)
                    signer = create_test_signer(root / "keys")
                    slot = path_factory(root)

                    def failing_lstat(path: Path, *args, **kwargs):
                        if path == slot:
                            raise AssertionError(
                                "alias signer slot path should fail before metadata"
                            )
                        return original_lstat(path, *args, **kwargs)

                    stdout = io.StringIO()
                    stderr = io.StringIO()
                    with mock.patch.object(path_type, "lstat", failing_lstat):
                        with redirect_stdout(stdout), redirect_stderr(stderr):
                            status = evidence_signer.main(
                                [
                                    "--slot",
                                    str(slot),
                                    "--private-key",
                                    str(signer["private_key"]),
                                    "--public-key",
                                    str(signer["public_key"]),
                                    "--signer-key-id",
                                    "android-lab-release-signer-v1",
                                    "--signed-at-utc",
                                    "2026-06-06T00:00:00Z",
                                ]
                            )
                    rendered = stdout.getvalue() + stderr.getvalue()

                self.assertEqual(status, 1)
                self.assertIn(expected_error, rendered)
                self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_slot_directory_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            stderr = io.StringIO()

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot:
                    raise OSError("simulated slot directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = evidence_signer.main(
                        [
                            "--slot",
                            str(slot),
                            "--private-key",
                            str(signer["private_key"]),
                            "--public-key",
                            str(signer["public_key"]),
                            "--signer-key-id",
                            "android-lab-release-signer-v1",
                            "--signed-at-utc",
                            "2026-06-06T00:00:00Z",
                        ]
                    )
            finally:
                path_type.lstat = original_lstat

        self.assertEqual(status, 1)
        self.assertIn("slot directory metadata could not be read", stderr.getvalue())
        self.assertNotIn("slot.json could not be read", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_slot_parent_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            stderr = io.StringIO()

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot.parent:
                    raise OSError("simulated slot parent lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                    status = evidence_signer.main(
                        [
                            "--slot",
                            str(slot),
                            "--private-key",
                            str(signer["private_key"]),
                            "--public-key",
                            str(signer["public_key"]),
                            "--signer-key-id",
                            "android-lab-release-signer-v1",
                            "--signed-at-utc",
                            "2026-06-06T00:00:00Z",
                        ]
                    )
            finally:
                path_type.lstat = original_lstat

        self.assertEqual(status, 1)
        self.assertIn("slot parent directory metadata could not be read", stderr.getvalue())
        self.assertNotIn("slot.json could not be read", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_output_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            secret_output = "evidence/token=supersecret-signed-evidence.json"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        secret_output,
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(secret_output, rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_output_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            control_output = "evidence/control\nsigned-evidence.json"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        control_output,
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signed evidence output path must not contain control characters", rendered)
        self.assertNotIn(control_output, rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_secret_looking_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            secret_signer_key_id = "token=supersecret"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        secret_signer_key_id,
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "signer key id must be non-empty and must not contain secret-looking material",
            rendered,
        )
        self.assertNotIn(secret_signer_key_id, rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_padded_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        " android-lab-release-signer-v1 ",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer key id must not contain surrounding whitespace", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_control_signer_key_id_before_metadata_read(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            signer = create_test_signer(Path(temp) / "keys")
            slot = Path(temp) / "slots" / "pixel8"
            unsafe_signer_key_id = "android-lab\x1b[31m"

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        unsafe_signer_key_id,
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("signer key id must not contain control characters", rendered)
        self.assertNotIn(unsafe_signer_key_id, rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertNotIn("slot directory missing", rendered)

    def test_signer_helper_rejects_output_outside_evidence_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "telemetry/signed-evidence.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must stay under evidence/",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "telemetry" / "signed-evidence.json").exists())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_noncanonical_output_filename_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "evidence/signed-evidence-copy.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must be evidence/signed-evidence.json",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence-copy.json").exists())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_backslash_output_path_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        "evidence\\signed-evidence.json",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence output path must not contain backslashes",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_absolute_parent_segment_output_path_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            alias_output = (
                slot
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json"
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                        "--output",
                        str(alias_output),
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("signed evidence output path must be canonical", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_metadata_output_path_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["signed_evidence_artifact_path"] = " evidence/signed-evidence.json "
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json signed_evidence_artifact_path must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_output_normalise_rejects_output_resolve_failure(self) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = Path(temp) / "slots" / "pixel8"
                slot.mkdir(parents=True)
                output = slot / "evidence" / "signed-evidence.json"
                errors: list[str] = []

                def failing_resolve(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                    slot,
                    {},
                    str(output),
                    errors,
                )
        finally:
            path_type.resolve = original_resolve

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path could not be resolved"])
        self.assertFalse(output.exists())

    def test_signer_output_normalise_rejects_slot_resolve_failure(self) -> None:
        path_type = type(Path("."))
        original_resolve = path_type.resolve

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = Path(temp) / "slots" / "pixel8"
                slot.mkdir(parents=True)
                output = slot / "evidence" / "signed-evidence.json"
                output_resolved = False
                errors: list[str] = []

                def failing_resolve(path: Path, *args, **kwargs):
                    nonlocal output_resolved
                    if path == output and not output_resolved:
                        output_resolved = True
                        return original_resolve(path, *args, **kwargs)
                    if path == slot:
                        raise OSError("simulated slot resolve failure")
                    return original_resolve(path, *args, **kwargs)

                path_type.resolve = failing_resolve

                result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                    slot,
                    {},
                    str(output),
                    errors,
                )
        finally:
            path_type.resolve = original_resolve

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path could not be resolved"])
        self.assertFalse(output.exists())

    def test_signer_output_normalise_rejects_absolute_symlinked_output_ancestor(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slots" / "pixel8"
            slot.joinpath("evidence").mkdir(parents=True)
            alias = root / "linked-slot"
            create_dir_symlink(self, alias, slot)
            errors: list[str] = []

            result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                slot,
                {},
                str(alias / "evidence" / "signed-evidence.json"),
                errors,
            )

        self.assertIsNone(result)
        self.assertEqual(
            errors,
            ["signed evidence output path ancestor directory must not be a symlink"],
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_output_normalise_rejects_absolute_symlinked_output_leaf(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = root / "slots" / "pixel8"
            output = slot / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            output.parent.mkdir(parents=True)
            target.write_text("external\n", encoding="utf-8")
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            errors: list[str] = []

            result = evidence_signer._normalise_output_path(  # type: ignore[attr-defined]
                slot,
                {},
                str(output),
                errors,
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(result)
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_symlinked_output_parent_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            external = root / "external-evidence"
            external.mkdir()
            output_parent = root / "slot" / "evidence"
            output_parent.parent.mkdir(parents=True)
            create_dir_symlink(self, output_parent, external)

            errors = evidence_signer._write_json(
                output_parent / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory must not be a symlink"],
        )
        self.assertFalse((external / "signed-evidence.json").exists())

    def test_signer_write_json_uses_lstat_before_parent_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                output.parent.mkdir(parents=True)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')

    def test_signer_write_json_rejects_parent_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                output.parent.mkdir(parents=True)

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_exists = output.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory metadata could not be read"],
        )
        self.assertFalse(output_exists)

    def test_signer_write_json_rejects_symlinked_output_ancestor_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = root / "real-slot"
            real_slot.joinpath("evidence").mkdir(parents=True)
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = evidence_signer._write_json(
                linked_slot / "evidence" / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

            self.assertEqual(
                errors,
                ["signed evidence output path ancestor directory must not be a symlink"],
            )
            self.assertFalse((real_slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_write_json_rejects_symlinked_output_ancestor_before_creating_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_slot = root / "real-slot"
            real_slot.mkdir()
            linked_slot = root / "linked-slot"
            create_dir_symlink(self, linked_slot, real_slot)

            errors = evidence_signer._write_json(
                linked_slot / "evidence" / "signed-evidence.json",
                {"schema": "test"},
                "signed evidence output path",
            )

            self.assertEqual(
                errors,
                ["signed evidence output path ancestor directory must not be a symlink"],
            )
            self.assertFalse((real_slot / "evidence").exists())

    def test_signer_write_json_rejects_symlinked_output_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(target, "external\n")
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_hardlinked_output_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(target, "external\n")
            write_text(output, "placeholder\n")
            replace_with_hardlink(self, output, target)

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["signed evidence output path must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_json_rejects_hardlink_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, "placeholder\n")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == output and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated output hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(
            errors,
            ["signed evidence output path hardlink metadata could not be read"],
        )
        self.assertEqual(output_text, "placeholder\n")

    def test_signer_write_json_rejects_file_metadata_failure_before_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, "placeholder\n")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(
            errors,
            ["signed evidence output path file metadata could not be read"],
        )
        self.assertEqual(output_text, "placeholder\n")

    def test_signer_write_json_rejects_secret_output_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = (
                Path(temp)
                / "slot"
                / "token=supersecret-output"
                / "signed-evidence.json"
            )

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["signed evidence output path must not contain secret-looking material"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(output), rendered)

    def test_signer_write_json_rejects_control_output_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = (
                Path(temp)
                / "slot"
                / "control\noutput"
                / "signed-evidence.json"
            )

            errors = evidence_signer._write_json(
                output,
                {"schema": "test"},
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["signed evidence output path must not contain control characters"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn(str(output), rendered)

    def test_signer_write_json_rejects_nonfinite_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            errors = evidence_signer._write_json(
                output,
                {"schema": "test", "value": float("nan")},
                "signed evidence output path",
            )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(errors, ["signed evidence output path is not strict JSON"])
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_oversized_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            payload = {"schema": "test"}
            text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
            limit = len(text.encode("utf-8")) - 1

            with mock.patch.object(
                evidence_signer.device_lab,
                "MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
                limit,
            ):
                errors = evidence_signer._write_json(
                    output,
                    payload,
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(
            errors,
            [f"signed evidence output path must be no more than {limit} bytes"],
        )
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            with mock.patch.object(
                evidence_signer.os,
                "fsync",
                side_effect=OSError("simulated signed evidence fsync failure"),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(errors, ["signed evidence output path could not be written"])
        self.assertFalse(output.exists())

    def test_signer_write_json_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            write_text(output, "existing signed evidence\n")

            with mock.patch.object(
                evidence_signer.os,
                "replace",
                side_effect=OSError("simulated signed evidence replace failure"),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(errors, ["signed evidence output path could not be written"])
        self.assertEqual(output_text, "existing signed evidence\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_json_reports_temp_cleanup_failure_after_write_failure(
        self,
    ) -> None:
        original_unlink = evidence_signer.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            def failing_replace(src: Path, dst: Path) -> None:
                raise OSError("simulated signer replace failure")

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{output.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated signer temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(evidence_signer.os, "replace", failing_replace),
                mock.patch.object(evidence_signer.os, "unlink", failing_temp_unlink),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(
            errors,
            [
                "signed evidence output path could not be written",
                "signed evidence output path temporary file could not be removed",
            ],
        )
        self.assertEqual(len(temp_files), 1)

    def test_signer_json_output_validators_reject_alias_paths_before_metadata(
        self,
    ) -> None:
        cases = (
            (
                "write-backslash",
                evidence_signer._validate_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "write-parent",
                evidence_signer._validate_json_output_path,  # type: ignore[attr-defined]
                lambda base: base
                / "slot"
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
            (
                "existing-backslash",
                evidence_signer._validate_existing_json_output_path,  # type: ignore[attr-defined]
                lambda base: base / "slot" / "evidence\\signed-evidence.json",
                "signed evidence output path must not contain backslashes",
            ),
            (
                "existing-parent",
                evidence_signer._validate_existing_json_output_path,  # type: ignore[attr-defined]
                lambda base: base
                / "slot"
                / "evidence"
                / ".."
                / "evidence"
                / "signed-evidence.json",
                "signed evidence output path must be canonical",
            ),
        )
        for name, validator, path_factory, expected_error in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as temp:
                    output = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias signed-evidence output should fail before metadata"
                        ),
                    ):
                        errors = validator(output, "signed evidence output path")

                self.assertEqual(errors, [expected_error])

    def test_signer_write_json_reports_temp_cleanup_failure_after_post_stage_validation_failure(
        self,
    ) -> None:
        original_validate = evidence_signer._validate_json_output_path
        original_unlink = evidence_signer.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            validation_calls = 0

            def racing_validate(path: Path, label: str) -> list[str]:
                nonlocal validation_calls
                if path == output and label == "signed evidence output path":
                    validation_calls += 1
                    if validation_calls == 2:
                        return ["signed evidence output path changed after staging"]
                return original_validate(path, label)

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{output.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated signer temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(
                    evidence_signer,
                    "_validate_json_output_path",
                    racing_validate,
                ),
                mock.patch.object(evidence_signer.os, "unlink", failing_temp_unlink),
            ):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
            output_exists = output.exists()

        self.assertEqual(
            errors,
            [
                "signed evidence output path changed after staging",
                "signed evidence output path temporary file could not be removed",
            ],
        )
        self.assertEqual(validation_calls, 2)
        self.assertFalse(output_exists)
        self.assertEqual(len(temp_files), 1)

    def test_signer_write_json_temp_cleanup_rejects_swapped_temp_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".signed-evidence.json.swap.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = evidence_signer._file_identity(temp_path.lstat())
            swapped_temp = Path(temp) / "original-signed-evidence-temp-file"
            temp_path.rename(swapped_temp)
            temp_path.write_text("do not remove\n", encoding="utf-8")

            errors = evidence_signer._cleanup_temp_output(
                temp_path,
                "signed evidence output path",
                temp_identity,
            )
            victim_survived = temp_path.read_text(encoding="utf-8")
            original_survived = swapped_temp.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["signed evidence output path temporary file changed before cleanup"],
        )
        self.assertEqual(victim_survived, "do not remove\n")
        self.assertEqual(original_survived, "original\n")

    def test_signer_write_json_rejects_parent_directory_sync_failure_after_replace(
        self,
    ) -> None:
        original_fsync = evidence_signer.os.fsync

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            sync_calls = 0

            def failing_parent_fsync(fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError("simulated signer parent sync failure")
                original_fsync(fd)

            with mock.patch.object(evidence_signer.os, "fsync", failing_parent_fsync):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")
            temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))

        self.assertEqual(sync_calls, 2)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory could not be synced"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_parent_directory_identity_swap_before_sync(
        self,
    ) -> None:
        original_open = evidence_signer.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "signed-evidence-root"
            output = root / "signed-evidence.json"
            root.mkdir()
            swapped_root = wrapper / "signed-evidence-root-swapped"
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == output.parent and not swapped:
                    output.parent.rename(swapped_root)
                    output.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(evidence_signer.os, "open", swapping_parent_open):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = (swapped_root / output.name).read_text(encoding="utf-8")

        self.assertTrue(swapped)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory changed before sync"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')

    def test_signer_write_json_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = evidence_signer._validate_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "evidence" / "signed-evidence.json"
                target = root / "external-signed-evidence.json"
                output.parent.mkdir(parents=True)
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 2:
                        write_text(target, "external\n")
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_json_output_path = validate_then_alias
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                target_text = target.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._validate_json_output_path = original_validate

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_readback_mismatch(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def mismatching_read_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched signed evidence\n", []
                    return original_read_output_text(
                        path,
                        expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = mismatching_read_output_text
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(
            errors,
            ["signed evidence output path write verification failed"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')

    def test_signer_write_json_rejects_readback_failure(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

                def failing_read_output_text(
                    path: Path,
                    _expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(
                        path,
                        _expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = failing_read_output_text
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(
            errors,
            ["signed evidence output path write verification failed"],
        )
        self.assertEqual(output_text, '{\n  "schema": "test"\n}\n')
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_oversized_readback_after_replace(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            payload = {"schema": "test"}
            text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
            limit = len(text.encode("utf-8")) + 4
            mutated = False

            def appending_open(path: Path, *args, **kwargs):
                nonlocal mutated
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == output and "r" in str(mode) and not mutated:
                    with original_open(path, "a", encoding="utf-8") as handle:
                        handle.write("X" * 16)
                    mutated = True
                return original_open(path, *args, **kwargs)

            with (
                mock.patch.object(
                    evidence_signer.device_lab,
                    "MAX_ANDROID_DEVICE_LAB_JSON_BYTES",
                    limit,
                ),
                mock.patch.object(Path, "open", appending_open),
            ):
                errors = evidence_signer._write_json(
                    output,
                    payload,
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertTrue(mutated)
        self.assertGreater(
            len(output_text.encode("utf-8")),
            len(text.encode("utf-8")),
        )
        self.assertEqual(
            errors,
            [f"signed evidence output path must be no more than {limit} bytes"],
        )

    def test_signer_write_json_rejects_regular_file_swap_before_readback(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            replacement = root / "replacement-signed-evidence.json"
            write_text(replacement, '{"schema":"replacement"}\n')
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                if path == output and not swapped:
                    replacement.replace(output)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertEqual(
            errors,
            ["signed evidence output path changed while being read"],
        )
        self.assertEqual(output_text, '{"schema":"replacement"}\n')

    def test_signer_write_json_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = evidence_signer._validate_existing_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "evidence" / "signed-evidence.json"
                target = root / "external-signed-evidence.json"
                write_text(target, "external\n")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 1:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_existing_json_output_path = validate_then_alias
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )
                target_text = target.read_text(encoding="utf-8")
                output_is_symlink = output.is_symlink()
                temp_files = list(output.parent.glob(".signed-evidence.json.*.tmp"))
        finally:
            evidence_signer._validate_existing_json_output_path = original_validate

        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_signer_write_json_rejects_parent_create_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            def failing_mkdir(path: Path, *args, **kwargs):
                if path == output.parent:
                    raise OSError("simulated signed evidence parent mkdir failure")
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", failing_mkdir):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory could not be created"],
        )
        self.assertFalse(output.exists())

    def test_signer_write_json_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "late-linked-output" / "signed-evidence.json"
            alias_target = root / "external-output"
            alias_target.mkdir()

            def replacing_mkdir(path: Path, *args, **kwargs):
                if path == output.parent:
                    create_dir_symlink(self, path, alias_target)
                    return None
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", replacing_mkdir):
                errors = evidence_signer._write_json(
                    output,
                    {"schema": "test"},
                    "signed evidence output path",
                )

        self.assertEqual(
            errors,
            ["signed evidence output path parent directory must not be a symlink"],
        )
        self.assertFalse((alias_target / "signed-evidence.json").exists())

    def test_signer_output_digest_rejects_secret_path_directly_without_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "token=supersecret-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path must not contain secret-looking material"],
        )
        self.assertNotIn(str(output), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signer_output_digest_rejects_missing_parent_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory is missing"],
        )

    def test_signer_output_digest_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir
        payload = '{"schema":"test"}\n'

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, payload)

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output digest parent is_dir preflight failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(payload.encode("utf-8")).hexdigest())

    def test_signer_output_digest_rejects_parent_metadata_failure_before_read(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output.parent:
                        raise OSError("simulated output digest parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path parent directory metadata could not be read"],
        )

    def test_signer_output_digest_rejects_missing_leaf_before_read(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            output.parent.mkdir(parents=True)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path must exist before digest"],
        )

    def test_signer_output_digest_rejects_symlinked_leaf_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(target, "external\n")
            replace_with_symlink(self, output, target)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_output_digest_rejects_hardlinked_leaf_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            target = root / "external-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(target, "external\n")
            replace_with_hardlink(self, output, target)

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_output_digest_rejects_oversized_output_after_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            output.parent.mkdir(parents=True)
            with output.open("wb") as handle:
                handle.seek(device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES)
                handle.write(b"x")

            digest, errors = evidence_signer._output_file_sha256(
                output,
                "signed evidence output path",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "signed evidence output path must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_signer_output_digest_rejects_hardlink_metadata_failure_after_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_stat(path: Path, *args, **kwargs):
                    if path == output and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated output digest hardlink metadata failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path hardlink metadata could not be read"],
        )
        self.assertEqual(output_text, '{"schema":"test"}\n')

    def test_signer_output_digest_rejects_file_metadata_failure_after_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
                write_text(output, '{"schema":"test"}\n')

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == output:
                        raise OSError("simulated output digest file metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path file metadata could not be read"],
        )
        self.assertEqual(output_text, '{"schema":"test"}\n')

    def test_signer_output_digest_rejects_read_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "evidence" / "signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')

            digest, errors = with_open_failure(
                output,
                lambda: evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["signed evidence output path could not be read"])

    def test_signer_output_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "evidence" / "signed-evidence.json"
            replacement = root / "replacement-signed-evidence.json"
            write_text(output, '{"schema":"test"}\n')
            write_text(replacement, '{"schema":"replacement"}\n')
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                if path == output and not swapped:
                    replacement.replace(output)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                digest, errors = evidence_signer._output_file_sha256(
                    output,
                    "signed evidence output path",
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["signed evidence output path changed while being read"],
        )
        self.assertEqual(output_text, '{"schema":"replacement"}\n')

    def test_signer_helper_revalidates_output_digest_before_slot_json_update(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            alias_target = root / "external-signed-evidence.json"
            original_build_signed_evidence = evidence_signer.build_signed_evidence
            original_write_json = evidence_signer._write_json

            def fake_build_signed_evidence(*_args, **_kwargs):
                return {"schema": device_lab.SIGNED_EVIDENCE_SCHEMA}

            def write_json_then_alias(path: Path, payload: dict, label: str) -> list[str]:
                errors = original_write_json(path, payload, label)
                if not errors and label == "signed evidence output path":
                    alias_target.write_text(path.read_text(encoding="utf-8"), encoding="utf-8")
                    path.unlink()
                    try:
                        path.symlink_to(alias_target)
                    except (NotImplementedError, OSError) as exc:
                        self.skipTest(
                            f"symlinks are not available in this test environment: {exc}"
                        )
                return errors

            try:
                evidence_signer.build_signed_evidence = fake_build_signed_evidence
                evidence_signer._write_json = write_json_then_alias
                status, output_relative, errors = evidence_signer.sign_slot_evidence(
                    slot_path=slot,
                    private_key_path=root / "private.pem",
                    public_key_path=root / "public.pem",
                    signer_key_id="android-lab-release-signer-v1",
                    signed_at_utc="2026-06-06T00:00:00Z",
                    output=None,
                    update_slot_json=True,
                    update_sha256sum=False,
                )
            finally:
                evidence_signer.build_signed_evidence = original_build_signed_evidence
                evidence_signer._write_json = original_write_json
            metadata_after = (slot / "slot.json").read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertEqual(output_relative, "evidence/signed-evidence.json")
        self.assertEqual(errors, ["signed evidence output path must not be a symlink"])
        self.assertEqual(metadata_after, original_metadata)

    def test_signer_write_text_rejects_symlinked_manifest_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "external-sha256sum.txt"
            write_text(target, "external\n")
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_text_rejects_dangling_symlinked_manifest_leaf_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "missing-sha256sum.txt"
            output.parent.mkdir(parents=True)
            try:
                output.symlink_to(target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertFalse(target.exists())

    def test_signer_write_text_rejects_hardlinked_manifest_leaf_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output = root / "slot" / "sha256sum.txt"
            target = root / "external-sha256sum.txt"
            write_text(target, "external\n")
            write_text(output, "placeholder\n")
            replace_with_hardlink(self, output, target)

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            target_text = target.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertEqual(target_text, "external\n")

    def test_signer_write_text_rejects_secret_manifest_path_directly_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "token=supersecret-sha256sum.txt"

            errors = evidence_signer._write_text(
                output,
                "replacement\n",
                "sha256sum.txt",
            )
            rendered = "\n".join(errors)

        self.assertEqual(
            errors,
            ["sha256sum.txt must not contain secret-looking material"],
        )
        self.assertFalse(output.exists())
        self.assertFalse(output.parent.exists())
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(output), rendered)

    def test_signer_write_text_rejects_oversized_manifest_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            text = "replacement\n"
            limit = len(text.encode("utf-8")) - 1

            errors = evidence_signer._write_text(
                output,
                text,
                "sha256sum.txt",
                max_bytes=limit,
            )
            temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))

        self.assertEqual(errors, [f"sha256sum.txt must be no more than {limit} bytes"])
        self.assertFalse(output.exists())
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"

            with mock.patch.object(
                evidence_signer.os,
                "fsync",
                side_effect=OSError("simulated sha256sum fsync failure"),
            ):
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )

        self.assertEqual(errors, ["sha256sum.txt could not be written"])
        self.assertFalse(output.exists())

    def test_signer_write_text_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            write_text(output, "existing manifest\n")

            with mock.patch.object(
                evidence_signer.os,
                "replace",
                side_effect=OSError("simulated sha256sum replace failure"),
            ):
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
            output_text = output.read_text(encoding="utf-8")
            temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))

        self.assertEqual(errors, ["sha256sum.txt could not be written"])
        self.assertEqual(output_text, "existing manifest\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = evidence_signer._validate_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "sha256sum.txt"
                target = root / "external-sha256sum.txt"
                output.parent.mkdir(parents=True)
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 2:
                        write_text(target, "external\n")
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_json_output_path = validate_then_alias
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                target_text = target.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._validate_json_output_path = original_validate

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_readback_mismatch(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "sha256sum.txt"

                def mismatching_read_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return "mismatched manifest\n", []
                    return original_read_output_text(
                        path,
                        expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = mismatching_read_output_text
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                output_text = output.read_text(encoding="utf-8")
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(errors, ["sha256sum.txt write verification failed"])
        self.assertEqual(output_text, "replacement\n")

    def test_signer_write_text_rejects_oversized_readback_after_replace(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "slot" / "sha256sum.txt"
            text = "replacement\n"
            limit = len(text.encode("utf-8")) + 4
            mutated = False

            def appending_open(path: Path, *args, **kwargs):
                nonlocal mutated
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == output and "r" in str(mode) and not mutated:
                    with original_open(path, "a", encoding="utf-8") as handle:
                        handle.write("X" * 16)
                    mutated = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", appending_open):
                errors = evidence_signer._write_text(
                    output,
                    text,
                    "sha256sum.txt",
                    max_bytes=limit,
                )
            output_text = output.read_text(encoding="utf-8")

        self.assertTrue(mutated)
        self.assertGreater(
            len(output_text.encode("utf-8")),
            len(text.encode("utf-8")),
        )
        self.assertEqual(errors, [f"sha256sum.txt must be no more than {limit} bytes"])

    def test_signer_write_text_rejects_readback_failure(self) -> None:
        original_read_output_text = evidence_signer._read_existing_output_text

        try:
            with tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "slot" / "sha256sum.txt"

                def failing_read_output_text(
                    path: Path,
                    _expected_stat: os.stat_result,
                    label: str,
                    *,
                    max_bytes: int | None = None,
                ) -> tuple[str | None, list[str]]:
                    if path == output:
                        return None, [f"{label} write verification failed"]
                    return original_read_output_text(
                        path,
                        _expected_stat,
                        label,
                        max_bytes=max_bytes,
                    )

                evidence_signer._read_existing_output_text = failing_read_output_text
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                output_text = output.read_text(encoding="utf-8")
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._read_existing_output_text = original_read_output_text

        self.assertEqual(errors, ["sha256sum.txt write verification failed"])
        self.assertEqual(output_text, "replacement\n")
        self.assertEqual(temp_files, [])

    def test_signer_write_text_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = evidence_signer._validate_existing_json_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                output = root / "slot" / "sha256sum.txt"
                target = root / "external-sha256sum.txt"
                write_text(target, "external\n")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == output and calls == 1:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                evidence_signer._validate_existing_json_output_path = validate_then_alias
                errors = evidence_signer._write_text(
                    output,
                    "replacement\n",
                    "sha256sum.txt",
                )
                target_text = target.read_text(encoding="utf-8")
                output_is_symlink = output.is_symlink()
                temp_files = list(output.parent.glob(".sha256sum.txt.*.tmp"))
        finally:
            evidence_signer._validate_existing_json_output_path = original_validate

        self.assertEqual(errors, ["sha256sum.txt must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_rewrite_sha256_manifest_rejects_oversized_manifest_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            with mock.patch.object(
                evidence_signer.device_lab,
                "MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES",
                1,
            ):
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must be no more than 1 bytes"])
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_symlinked_artifact_when_called_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertIn("slot artifact logs/runtime.log must not be a symlink", errors)
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_hardlinked_manifest_when_called_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            external_manifest = root / "external-sha256sum.txt"
            external_manifest.write_text("do not overwrite\n", encoding="utf-8")
            replace_with_hardlink(self, slot / "sha256sum.txt", external_manifest)

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            target_text = external_manifest.read_text(encoding="utf-8")

        self.assertEqual(errors, ["sha256sum.txt must not be hardlinked"])
        self.assertEqual(target_text, "do not overwrite\n")

    def test_rewrite_sha256_manifest_rejects_secret_looking_artifact_when_called_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            secret_artifact = "logs/token=supersecret.log"
            write_text(slot / secret_artifact, "must not be persisted\n")

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertEqual(manifest_after, original_manifest)
        self.assertNotIn(secret_artifact, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_rewrite_sha256_manifest_rejects_secret_slot_path_directly_without_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "token=supersecret-slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            errors = evidence_signer.rewrite_sha256_manifest(slot)
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            rendered = "\n".join(errors)

        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertEqual(manifest_after, original_manifest)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_rewrite_sha256_manifest_rejects_slot_directory_metadata_failure_without_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp) / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot:
                    raise OSError("simulated slot directory lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                path_type.lstat = original_lstat
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot directory metadata could not be read"])
        self.assertEqual(manifest_after, original_manifest)

    def test_rewrite_sha256_manifest_rejects_slot_parent_metadata_failure_without_write(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp) / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")

            def failing_lstat(path: Path, *args, **kwargs):
                if path == slot.parent:
                    raise OSError("simulated slot parent lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                path_type.lstat = original_lstat
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot parent directory metadata could not be read"])
        self.assertEqual(manifest_after, original_manifest)

    def test_signer_slot_artifact_digest_rejects_secret_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            secret_relative = "logs/token=supersecret.log"
            write_text(slot / secret_relative, "must not be hashed\n")

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                secret_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifacts must not contain secret-looking material"])
        self.assertNotIn(secret_relative, rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_signer_slot_artifact_digest_rejects_control_relative_path_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            control_relative = "logs/runtime\x1b[31m.log"

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                control_relative,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact path: unsafe path contains control characters"],
        )
        self.assertNotIn(control_relative, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_signer_slot_artifact_digest_rejects_symlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be a symlink"])

    def test_signer_slot_artifact_digest_rejects_hardlink_directly(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "pixel8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be hardlinked"])

    def test_signer_slot_artifact_digest_rejects_oversized_artifact_directly(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            runtime_log = slot / "logs" / "runtime.log"
            with runtime_log.open("wb") as handle:
                handle.seek(device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES)
                handle.write(b"x")

            digest, errors = evidence_signer._slot_artifact_sha256(
                slot,
                "logs/runtime.log",
            )

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            [
                "slot artifact logs/runtime.log must be no more than "
                f"{device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            ],
        )

    def test_signer_slot_artifact_digest_uses_release_apk_specific_limit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            apk_path = slot / "evidence" / "offline-wallet-release.apk"
            apk_path.write_bytes(b"x" * 16)
            old_base_limit = device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
            old_apk_limit = device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES
            try:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 8
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = 32
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "evidence/offline-wallet-release.apk",
                )
            finally:
                device_lab.MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = old_base_limit
                device_lab.MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = old_apk_limit

        self.assertEqual(errors, [])
        self.assertEqual(digest, hashlib.sha256(b"x" * 16).hexdigest())

    def test_signer_slot_artifact_digest_rejects_hardlink_metadata_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            def failing_stat(path: Path, *args, **kwargs):
                if path == target and kwargs.get("follow_symlinks", True):
                    raise OSError("simulated slot artifact stat failure")
                return original_stat(path, *args, **kwargs)

            try:
                path_type.stat = failing_stat
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
            finally:
                path_type.stat = original_stat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log hardlink metadata could not be read"],
        )

    def test_signer_slot_artifact_digest_rejects_file_metadata_failure_after_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            def failing_lstat(path: Path, *args, **kwargs):
                if path == target:
                    raise OSError("simulated slot artifact lstat failure")
                return original_lstat(path, *args, **kwargs)

            try:
                path_type.lstat = failing_lstat
                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
            finally:
                path_type.lstat = original_lstat

        self.assertIsNone(digest)
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log file metadata could not be read"],
        )

    def test_signer_slot_artifact_digest_rejects_read_failure_after_preflight(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "pixel8")
            target = slot / "logs" / "runtime.log"

            digest, errors = with_open_failure(
                target,
                lambda: evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                ),
            )

        self.assertIsNone(digest)
        self.assertEqual(errors, ["slot artifact logs/runtime.log could not be read"])

    def test_signer_slot_artifact_digest_rejects_regular_file_swap_after_preflight(
        self,
    ) -> None:
        original_validate = evidence_signer._validate_slot_artifact_for_digest

        try:
            with tempfile.TemporaryDirectory() as temp:
                slot = create_slot(Path(temp), "pixel8")
                artifact_path = slot / "logs" / "runtime.log"
                swapped = False

                def swapping_validate(slot_path: Path, relative: str):
                    nonlocal swapped
                    artifact, artifact_stat, errors = original_validate(
                        slot_path,
                        relative,
                    )
                    if artifact == artifact_path and not errors and not swapped:
                        artifact_path.unlink()
                        write_text(artifact_path, "replacement runtime log\n")
                        swapped = True
                    return artifact, artifact_stat, errors

                evidence_signer._validate_slot_artifact_for_digest = swapping_validate

                digest, errors = evidence_signer._slot_artifact_sha256(
                    slot,
                    "logs/runtime.log",
                )
                replacement_bytes = artifact_path.read_bytes()
        finally:
            evidence_signer._validate_slot_artifact_for_digest = original_validate

        self.assertTrue(swapped)
        self.assertIsNone(digest)
        self.assertEqual(replacement_bytes, b"replacement runtime log\n")
        self.assertEqual(
            errors,
            ["slot artifact logs/runtime.log changed while being read"],
        )

    def test_rewrite_sha256_manifest_revalidates_artifact_before_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            original_manifest = (slot / "sha256sum.txt").read_text(encoding="utf-8")
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            original_validate = evidence_signer._validate_slot_for_manifest_rewrite

            def validate_then_alias(slot_path: Path) -> list[str]:
                errors = original_validate(slot_path)
                if not errors:
                    replace_with_symlink(self, slot_path / "logs" / "runtime.log", target)
                return errors

            try:
                evidence_signer._validate_slot_for_manifest_rewrite = validate_then_alias
                errors = evidence_signer.rewrite_sha256_manifest(slot)
            finally:
                evidence_signer._validate_slot_for_manifest_rewrite = original_validate
            manifest_after = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(errors, ["slot artifact logs/runtime.log must not be a symlink"])
        self.assertEqual(manifest_after, original_manifest)

    def test_signer_metadata_loader_rejects_secret_slot_path_directly_without_parse(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = Path(temp) / "token=supersecret-slot"
            slot.mkdir(parents=True)
            (slot / "slot.json").write_text("{not-json", encoding="utf-8")

            metadata, errors = evidence_signer._require_slot_metadata(slot)
            rendered = "\n".join(errors)

        self.assertIsNone(metadata)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("not valid JSON", rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_signer_artifact_digests_include_release_apk_attestation_chain_and_report(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            metadata = json.loads((slot / "slot.json").read_text(encoding="utf-8"))
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(  # type: ignore[attr-defined]
                slot,
                errors,
                metadata,
            )

        self.assertEqual(errors, [])
        self.assertIsNotNone(digests)
        assert digests is not None
        self.assertIn(metadata["offline_wallet_apk_path"], digests)
        self.assertIn(metadata["attestation_certificate_chain_path"], digests)
        self.assertIn("attestation/report.json", digests)

    def test_signer_artifact_digests_rejects_secret_slot_path_directly_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            slot = create_slot(Path(temp), "token=supersecret-slot")
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(slot, errors)  # type: ignore[attr-defined]
            rendered = "\n".join(errors)

        self.assertIsNone(digests)
        self.assertEqual(errors, ["slot path must not contain secret-looking material"])
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn(str(slot), rendered)

    def test_signer_artifact_digests_rejects_symlinked_slot_ancestor_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            real_parent = root / "real-parent"
            create_slot(real_parent / "device_lab", "slot-a")
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, real_parent)
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(  # type: ignore[attr-defined]
                linked_parent / "device_lab" / "slot-a",
                errors,
            )
            rendered = "\n".join(errors)

        self.assertIsNone(digests)
        self.assertEqual(errors, ["slot ancestor directory must not be a symlink"])
        self.assertNotIn("missing", rendered)
        self.assertNotIn("digest", rendered)

    def test_signer_artifact_digests_rejects_symlinked_artifact_directory_before_hash(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root, "slot-a")
            external_logs = root / "external-logs"
            write_text(external_logs / "runtime.log", "external log\n")
            for entry in (slot / "logs").iterdir():
                entry.unlink()
            (slot / "logs").rmdir()
            create_dir_symlink(self, slot / "logs", external_logs)
            errors: list[str] = []

            digests = evidence_signer._artifact_digests(slot, errors)  # type: ignore[attr-defined]

        self.assertIsNone(digests)
        self.assertEqual(errors, ["logs/ must not be a symlink"])

    def test_signer_helper_rejects_symlinked_slot_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            external_slot_json = root / "external-slot.json"
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            write_text(external_slot_json, original_metadata)
            replace_with_symlink(self, slot / "slot.json", external_slot_json)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            target_text = external_slot_json.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot.json must not be a symlink", stderr.getvalue())
        self.assertEqual(target_text, original_metadata)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_slot_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            external_slot_json = root / "external-slot.json"
            original_metadata = (slot / "slot.json").read_text(encoding="utf-8")
            write_text(external_slot_json, original_metadata)
            replace_with_hardlink(self, slot / "slot.json", external_slot_json)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            target_text = external_slot_json.read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot.json must not be hardlinked", stderr.getvalue())
        self.assertEqual(target_text, original_metadata)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_metadata_loader_preflights_symlinked_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            metadata, errors = evidence_signer._require_slot_metadata(slot)

        self.assertIsNone(metadata)
        self.assertIn("slot artifact logs/runtime.log must not be a symlink", errors)

    def test_signer_metadata_loader_preflights_hardlinked_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            metadata, errors = evidence_signer._require_slot_metadata(slot)

        self.assertIsNone(metadata)
        self.assertIn("slot artifact logs/runtime.log must not be hardlinked", errors)

    def test_signer_helper_rejects_symlinked_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_symlink(self, slot / "logs" / "runtime.log", target)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must not be a symlink",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_directory_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            slot_link = root / "slot-link"
            create_dir_symlink(self, slot_link, slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot_link),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_parent_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            real_root = root / "slots"
            slot = create_slot(real_root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            linked_root = root / "linked-slots"
            create_dir_symlink(self, linked_root, real_root)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(linked_root / "pixel8"),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot parent directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_symlinked_slot_ancestor_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            external_parent = root / "external-parent"
            real_root = external_parent / "device_lab"
            slot = create_slot(real_root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            linked_parent = root / "linked-parent"
            create_dir_symlink(self, linked_parent, external_parent)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(linked_parent / "device_lab" / "pixel8"),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot ancestor directory must not be a symlink", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_hardlinked_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            target = root / "outside-runtime.log"
            write_text(target, "kagemusha device-lab run complete\n")
            replace_with_hardlink(self, slot / "logs" / "runtime.log", target)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must not be hardlinked",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_non_regular_required_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            signer = create_test_signer(root / "keys")
            slot = create_slot(root / "slots", "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            replace_with_fifo(self, slot / "logs" / "runtime.log")

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot artifact logs/runtime.log must be a regular file",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_noncanonical_signed_at_utc(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00+00:00",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_unexpected_slot_metadata_field(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["operator_notes"] = "unexpected metadata"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json contains unexpected field operator_notes",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_slot_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "slot.json", "schema", "shadow")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot.json contains duplicate JSON object key schema", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_attestation_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "attestation" / "result.json", "slot", "shadow")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json contains duplicate JSON object key slot",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_d2d_transcript_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "handoff" / "d2d-payment.json", "schema", "shadow")
            refresh_d2d_payment_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "d2d payment transcript contains duplicate JSON object key schema",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_duplicate_wallet_integrity_json_key_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            inject_duplicate_json_key(slot / "wallet" / "integrity.json", "schema", "shadow")
            refresh_wallet_integrity_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "wallet integrity transcript contains duplicate JSON object key schema",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_slot_string_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["device_family"] = (
                f" {device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1]} "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json device_family must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_control_slot_string_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["app_package_name"] = "org.hyperledger.iroha\x1b[31m"
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )
            rendered = stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json app_package_name must not contain control characters",
            rendered,
        )
        self.assertNotIn("\x1b", rendered)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_attestation_chain_path_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["attestation_certificate_chain_path"] = (
                " attestation/keymint-certificate-chain.pem "
            )
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json attestation_certificate_chain_path must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_irrelevant_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = ["./gradlew test --tests unrelated.HealthCheck"]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:connectedDebugAndroidTest",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include :client-android:assembleRelease",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-android:assembleRelease",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include :offline-wallet-lab-app:installReleaseAndroidTest",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include adb shell am instrument",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
            stderr.getvalue(),
        )
        self.assertIn(
            "slot.json raw_test_commands must include org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_padded_raw_test_command_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            commands = list(device_lab.KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS)
            commands[0] = f" {commands[0]} "
            metadata["raw_test_commands"] = commands
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json raw_test_commands[0] must not contain surrounding whitespace",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_marker_stuffed_raw_test_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata["raw_test_commands"] = [
                (
                    "echo :client-android:assembleRelease "
                    ":offline-wallet-android:assembleRelease "
                    ":offline-wallet-android:connectedDebugAndroidTest "
                    ":offline-wallet-lab-app:assembleRelease "
                    ":offline-wallet-lab-app:installRelease "
                    ":offline-wallet-lab-app:installReleaseAndroidTest "
                    "adb shell am instrument "
                    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest "
                    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest "
                    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest"
                )
            ]
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json raw_test_commands must exactly match the Kagemusha Android production raw test command",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_native_bridge_abi_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            metadata_path = slot / "slot.json"
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            metadata.pop("native_bridge_abi_version")
            write_json(metadata_path, metadata)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "slot.json native_bridge_abi_version must be an integer",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_attestation_result_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            attestation_path = slot / "attestation" / "result.json"
            attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
            attestation["offline_wallet_policy_sha256"] = "33" * 32
            write_json(attestation_path, attestation)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/result.json offline_wallet_policy_sha256 must match slot.json offline_wallet_policy_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_attestation_report_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            report_path = slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["verification"]["strongbox_attestation"] = False
            write_json(report_path, report)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json verification.strongbox_attestation must be true",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_attestation_report_level_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            report_path = slot / "attestation" / "report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            del report["verification"]["keymaster_security_level"]
            write_json(report_path, report)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/report.json verification.keymaster_security_level "
            "must be a non-empty string",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_harness_challenge_mismatch_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["challenge_hex"] = "00"
            write_json(harness_path, harness)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/harness-result.json challenge_hex digest must match "
            "slot.json attestation_challenge_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_harness_chain_length_mismatch_before_write(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            harness_path = slot / "attestation" / "harness-result.json"
            harness = json.loads(harness_path.read_text(encoding="utf-8"))
            harness["chain_length"] = 3
            write_json(harness_path, harness)
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "attestation/harness-result.json chain_length must match "
            "attestation certificate-chain certificate count",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_d2d_transcript_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            transcript_path = slot / "handoff" / "d2d-payment.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["queue_after_sha256"] = "22" * 32
            write_json(transcript_path, transcript)
            refresh_d2d_payment_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_wallet_integrity_transcript_mismatch_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel7")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel7",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[1],
            )
            transcript_path = slot / "wallet" / "integrity.json"
            transcript = json.loads(transcript_path.read_text(encoding="utf-8"))
            transcript["wallet_state_after_rotation_sha256"] = transcript[
                "wallet_state_before_sha256"
            ]
            write_json(transcript_path, transcript)
            refresh_wallet_integrity_transcript_hash(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "wallet integrity transcript wallet_state_before_sha256 must differ from wallet_state_after_rotation_sha256",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_secret_looking_artifact_paths_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_path = "logs/token=supersecret.log"
            write_text(slot / secret_path, "must not be persisted\n")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()
            manifest_text = (slot / "sha256sum.txt").read_text(encoding="utf-8")

        self.assertEqual(status, 1)
        self.assertIn("slot artifacts must not contain secret-looking material", rendered)
        self.assertNotIn(secret_path, rendered)
        self.assertNotIn(secret_path, manifest_text)
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_missing_required_slot_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            (slot / "logs" / "runtime.log").unlink()
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("slot artifact logs/runtime.log is missing", stderr.getvalue())
        self.assertNotIn("Traceback", stderr.getvalue())
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_empty_required_slot_artifact_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            write_text(slot / "logs" / "runtime.log", "")
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "required slot artifact logs/runtime.log must be non-empty",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_rejects_failed_status_ndjson_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            write_text(slot / "telemetry" / "status.ndjson", '{"status":"failed"}\n')
            rewrite_sha256sum(slot)

            stderr = io.StringIO()
            with redirect_stdout(io.StringIO()), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(signer["private_key"]),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn(
            "telemetry/status.ndjson line 1 status must not be 'failed'",
            stderr.getvalue(),
        )
        self.assertFalse((slot / "evidence" / "signed-evidence.json").exists())

    def test_signer_helper_does_not_leak_secret_looking_private_key_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            slot = create_slot(root, "pixel8")
            write_unsigned_production_slot_metadata(
                slot,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
            )
            secret_private_key = Path(temp) / "private_key=supersecret.pem"
            secret_private_key.write_text("not an ed25519 private key\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = evidence_signer.main(
                    [
                        "--slot",
                        str(slot),
                        "--private-key",
                        str(secret_private_key),
                        "--public-key",
                        str(signer["public_key"]),
                        "--signer-key-id",
                        "android-lab-release-signer-v1",
                        "--signed-at-utc",
                        "2026-06-06T00:00:00Z",
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertNotIn(str(secret_private_key), rendered)
        self.assertNotIn("private_key=supersecret", rendered)
        self.assertIn(
            "private key path must not contain secret-looking material",
            rendered,
        )

    def test_sign_ed25519_rejects_secret_private_key_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                secret_private_key = Path(temp) / "private_key=supersecret.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    secret_private_key,
                    b"payload",
                    errors,
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain secret-looking material"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(secret_private_key), rendered)
        self.assertNotIn("private_key=supersecret", rendered)

    def test_sign_ed25519_rejects_control_private_key_path_before_openssl_lookup(
        self,
    ) -> None:
        original_which = device_lab.shutil.which
        try:
            device_lab.shutil.which = lambda _command: None
            with tempfile.TemporaryDirectory() as temp:
                control_private_key = Path(temp) / "control\nprivate.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    control_private_key,
                    b"payload",
                    errors,
                )
                rendered = "\n".join(errors)
        finally:
            device_lab.shutil.which = original_which

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key path must not contain control characters"],
        )
        self.assertNotIn("openssl is required", rendered)
        self.assertNotIn(str(control_private_key), rendered)

    def test_sign_ed25519_rejects_private_key_aliases_before_metadata_or_openssl(
        self,
    ) -> None:
        path_type = type(Path("."))
        cases = (
            (
                Path("keys") / ".." / "private.pem",
                "private key path must be canonical",
            ),
            (
                Path("keys\\private.pem"),
                "private key path must not contain backslashes",
            ),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for private_key, expected_error in cases:
                private_key = root / private_key
                with self.subTest(private_key=private_key):
                    with (
                        mock.patch.object(
                            path_type,
                            "lstat",
                            side_effect=AssertionError(
                                "private key metadata must not be read"
                            ),
                        ) as lstat,
                        mock.patch.object(
                            device_lab,
                            "_require_openssl",
                            side_effect=AssertionError("OpenSSL lookup must not run"),
                        ) as require_openssl,
                    ):
                        errors: list[str] = []
                        signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                            private_key,
                            b"payload",
                            errors,
                        )
                    rendered = "\n".join(errors)

                    self.assertIsNone(signature)
                    self.assertEqual(errors, [expected_error])
                    self.assertNotIn("openssl is required", rendered)
                    self.assertNotIn(str(private_key), rendered)
                    lstat.assert_not_called()
                    require_openssl.assert_not_called()

    def test_sign_ed25519_rejects_missing_private_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a missing private key")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                missing_private_key = Path(temp) / "missing-signer.pem"
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    missing_private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key must point to an existing file"])

    def test_sign_ed25519_rejects_non_regular_private_key_before_openssl_lookup(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for a private key directory")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key_directory = Path(temp) / "signer-directory.pem"
                private_key_directory.mkdir()
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key_directory,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key must be a regular file"])

    def test_sign_ed25519_rejects_private_key_file_metadata_failure_before_openssl(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up for unreadable key metadata")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == private_key:
                        raise OSError("simulated private key lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            path_type.lstat = original_lstat
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key file metadata could not be read"])
        self.assertNotIn(str(private_key), "\n".join(errors))

    def test_sign_ed25519_rejects_private_key_hardlink_metadata_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        path_type = type(Path("."))
        original_stat = path_type.stat

        def unexpected_require_openssl(_errors: list[str]) -> str | None:
            raise AssertionError("OpenSSL should not be looked up after metadata failure")

        try:
            device_lab._require_openssl = unexpected_require_openssl  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                private_key_stat_calls = 0

                def failing_private_key_stat(path: Path, *args, **kwargs):
                    nonlocal private_key_stat_calls
                    if path == private_key and kwargs.get("follow_symlinks", True):
                        private_key_stat_calls += 1
                        if private_key_stat_calls > 0:
                            raise OSError("simulated private key stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_private_key_stat
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            path_type.stat = original_stat

        self.assertIsNone(signature)
        self.assertEqual(errors, ["private key hardlink metadata could not be read"])
        self.assertNotIn(str(private_key), "\n".join(errors))

    def test_sign_ed25519_rejects_signature_read_failure_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            return subprocess.CompletedProcess(args=args, returncode=0)

        def failing_signature_open(path: Path, *args, **kwargs):
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode:
                raise OSError("simulated signature read failure")
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = failing_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_rejects_signature_output_swap_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open
        replacement_signature = b"y" * device_lab.ED25519_SIGNATURE_BYTES

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            replacement_path = out_path.with_name("replacement-signature.bin")
            replacement_path.write_bytes(replacement_signature)
            return subprocess.CompletedProcess(args=command, returncode=0)

        swapped = False

        def swapping_signature_open(path: Path, *args, **kwargs):
            nonlocal swapped
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode and not swapped:
                path.with_name("replacement-signature.bin").replace(path)
                swapped = True
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = swapping_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_rejects_signature_output_hardlink_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * device_lab.ED25519_SIGNATURE_BYTES)
            return subprocess.CompletedProcess(args=command, returncode=0)

        linked = False

        def hardlinking_signature_open(path: Path, *args, **kwargs):
            nonlocal linked
            mode = args[0] if args else kwargs.get("mode", "r")
            if path.name == "signature.bin" and "r" in mode and not linked:
                try:
                    os.link(path, path.with_name("signature-output-hardlink.bin"))
                except (AttributeError, NotImplementedError, OSError) as exc:
                    self.skipTest(
                        f"hardlinks are not available in this test environment: {exc}"
                    )
                linked = True
            return original_open(path, *args, **kwargs)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = hardlinking_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output could not be read"])

    def test_sign_ed25519_reads_only_shape_bound_signature_output_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_open = Path.open
        read_sizes: list[int] = []
        read_limit = device_lab.ED25519_SIGNATURE_BYTES + 1

        class SignatureOutputReader:
            def __init__(self, handle):
                self._handle = handle

            def __enter__(self):
                self._handle.__enter__()
                return self

            def __exit__(self, exc_type, exc, traceback):
                return self._handle.__exit__(exc_type, exc, traceback)

            def fileno(self):
                return self._handle.fileno()

            def read(self, size=-1):
                read_sizes.append(size)
                if size > read_limit:
                    raise AssertionError("signature output read exceeded shape bound")
                return self._handle.read(size)

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"x" * read_limit)
            return subprocess.CompletedProcess(args=command, returncode=0)

        def bounded_signature_open(path: Path, *args, **kwargs):
            mode = args[0] if args else kwargs.get("mode", "r")
            handle = original_open(path, *args, **kwargs)
            if path.name == "signature.bin" and "r" in mode:
                return SignatureOutputReader(handle)
            return handle

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            Path.open = bounded_signature_open
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            Path.open = original_open

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output must be 64 bytes"])
        self.assertEqual(read_sizes, [read_limit])

    def test_sign_ed25519_rejects_short_signature_output_after_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def fake_run(command, *args, **kwargs):
            out_path = Path(command[command.index("-out") + 1])
            out_path.write_bytes(b"short signature")
            return subprocess.CompletedProcess(args=command, returncode=0)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = fake_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature output must be 64 bytes"])

    def test_sign_ed25519_rejects_tempdir_failure_before_payload_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_tempdir = evidence_signer.tempfile.TemporaryDirectory

        def failing_tempdir(*args, **kwargs):
            raise OSError("simulated temporary directory failure")

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after tempdir failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            evidence_signer.tempfile.TemporaryDirectory = failing_tempdir
            with original_tempdir() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            evidence_signer.tempfile.TemporaryDirectory = original_tempdir

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature temporary directory could not be created"])

    def test_sign_ed25519_rejects_spawn_failure_after_payload_staging(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def failing_run(*args, **kwargs):
            raise OSError("simulated OpenSSL spawn failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature command could not be run"])

    def test_sign_ed25519_rejects_invalid_private_key_after_openssl_failure(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def failing_run(*args, **kwargs):
            raise subprocess.CalledProcessError(1, args[0])

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = failing_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("invalid key data\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(
            errors,
            ["private key must be a valid OpenSSL Ed25519 private key"],
        )

    def test_sign_ed25519_rejects_payload_staging_write_failure_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging write failure")

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                with mock.patch.object(
                    device_lab.os,
                    "fsync",
                    side_effect=OSError("simulated payload staging fsync failure"),
                ):
                    signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                        private_key,
                        b"payload",
                        errors,
                    )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature payload could not be staged"])

    def test_sign_ed25519_rejects_payload_staging_readback_mismatch_before_openssl(
        self,
    ) -> None:
        original_require_openssl = device_lab._require_openssl  # type: ignore[attr-defined]
        original_run = evidence_signer.subprocess.run
        original_read_staged_bytes = evidence_signer.device_lab._read_staged_bytes  # type: ignore[attr-defined]

        def unexpected_run(*args, **kwargs):
            raise AssertionError("OpenSSL should not run after staging readback drift")

        def drifting_payload_read(
            path: Path,
            expected_stat: os.stat_result,
            verification_error: str,
        ) -> tuple[bytes | None, list[str]]:
            if path.name == "payload.bin":
                return b"mutated payload", []
            return original_read_staged_bytes(path, expected_stat, verification_error)

        try:
            device_lab._require_openssl = lambda _errors: "/usr/bin/openssl"  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = unexpected_run
            evidence_signer.device_lab._read_staged_bytes = drifting_payload_read  # type: ignore[attr-defined]
            with tempfile.TemporaryDirectory() as temp:
                private_key = Path(temp) / "signing.pem"
                private_key.write_text("not used by mocked openssl\n", encoding="utf-8")
                errors: list[str] = []

                signature = evidence_signer._sign_ed25519(  # type: ignore[attr-defined]
                    private_key,
                    b"payload",
                    errors,
                )
        finally:
            device_lab._require_openssl = original_require_openssl  # type: ignore[attr-defined]
            evidence_signer.subprocess.run = original_run
            evidence_signer.device_lab._read_staged_bytes = original_read_staged_bytes  # type: ignore[attr-defined]

        self.assertIsNone(signature)
        self.assertEqual(errors, ["signature payload staging verification failed"])

    def test_standard_matrix_accepts_all_kagemusha_device_families(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                    ]
                )

        self.assertEqual(status, 0)

    def test_standard_matrix_rejects_duplicate_device_fingerprint(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
            copy_slot_binding(
                source=root / "slot-0",
                target=root / "slot-1",
                signer=signer,
                key="device_fingerprint",
            )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary_text = summary_path.read_text(encoding="utf-8")
            summary = json.loads(summary_text)

        self.assertEqual(status, 1)
        self.assertIn(
            "duplicate Kagemusha device_fingerprint_sha256 across slots: slot-0, slot-1",
            rendered,
        )
        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "device_fingerprint_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(
            duplicate["value_sha256"],
            hashlib.sha256(b"slot-0/fingerprint").hexdigest(),
        )
        self.assertNotIn("slot-0/fingerprint", rendered)
        self.assertNotIn("slot-0/fingerprint", summary_text)

    def test_standard_matrix_rejects_duplicate_attestation_challenge(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
            copy_slot_binding(
                source=root / "slot-0",
                target=root / "slot-1",
                signer=signer,
                key="attestation_challenge_sha256",
            )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()
            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 1)
        self.assertIn(
            "duplicate Kagemusha attestation_challenge_sha256 across slots: "
            "slot-0, slot-1",
            rendered,
        )
        duplicate = summary["kagemusha"]["duplicate_bindings"][
            "attestation_challenge_sha256"
        ][0]
        self.assertEqual(duplicate["slots"], ["slot-0", "slot-1"])
        self.assertEqual(
            duplicate["value_sha256"],
            hashlib.sha256(b"slot-0:attestation-challenge").hexdigest(),
        )

    def test_duplicate_matrix_bindings_redacts_unsafe_direct_report_slots(self) -> None:
        duplicate_fingerprint = "a" * 64
        reports = [
            {
                "slot": "token=supersecret-slot",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": duplicate_fingerprint,
                },
            },
            {
                "slot": "slot-\x1b[31m",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": duplicate_fingerprint,
                },
            },
        ]

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(reports)
        rendered = json.dumps(duplicates, allow_nan=False)

        duplicate = duplicates["device_fingerprint_sha256"][0]
        self.assertEqual(
            duplicate["slots"],
            [device_lab.SECRET_PATH_REDACTION, "<unsafe-slot-name>"],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("token=supersecret-slot", rendered)
        self.assertNotIn("slot-\\u001b[31m", rendered)
        self.assertNotIn("\\u001b", rendered)

    def test_duplicate_matrix_bindings_ignores_non_sha256_direct_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "token=supersecret-fingerprint",
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "device_fingerprint_sha256": "token=supersecret-fingerprint",
                },
            },
        ]

        duplicates = device_lab.kagemusha_duplicate_matrix_bindings(reports)
        rendered = json.dumps(duplicates, allow_nan=False)

        self.assertEqual(duplicates, {})
        self.assertNotIn("supersecret", rendered)

    def test_build_summary_redacts_unsafe_direct_report_strings(self) -> None:
        duplicate_fingerprint = "b" * 64
        duplicate_challenge = "c" * 64
        family = device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
        reports = [
            {
                "slot": "token=supersecret-slot",
                "status": "ok",
                "errors": ["bearer supersecret-token"],
                "kagemusha": {
                    "device_family": family,
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": duplicate_challenge,
                    "unsafe_note": "x-iroha-signature supersecret-signature",
                },
            },
            {
                "slot": "slot-\x1b[31m",
                "status": "ok",
                "errors": ["plain error"],
                "kagemusha": {
                    "device_family": family,
                    "device_fingerprint_sha256": duplicate_fingerprint,
                    "attestation_challenge_sha256": duplicate_challenge,
                },
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False)

        self.assertEqual(summary["slots"][0]["slot"], device_lab.SECRET_PATH_REDACTION)
        self.assertEqual(summary["slots"][1]["slot"], "<unsafe-slot-name>")
        self.assertEqual(
            summary["slots"][0]["errors"],
            [device_lab.SECRET_PATH_REDACTION],
        )
        self.assertEqual(
            summary["slots"][0]["kagemusha"]["unsafe_note"],
            device_lab.SECRET_PATH_REDACTION,
        )
        duplicate_fingerprint_summary = summary["kagemusha"]["duplicate_bindings"][
            "device_fingerprint_sha256"
        ][0]
        self.assertEqual(
            duplicate_fingerprint_summary["slots"],
            [device_lab.SECRET_PATH_REDACTION, "<unsafe-slot-name>"],
        )
        duplicate_challenge_summary = summary["kagemusha"]["duplicate_bindings"][
            "attestation_challenge_sha256"
        ][0]
        self.assertEqual(
            duplicate_challenge_summary["slots"],
            [device_lab.SECRET_PATH_REDACTION, "<unsafe-slot-name>"],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("token=supersecret-slot", rendered)
        self.assertNotIn("slot-\\u001b[31m", rendered)
        self.assertNotIn("\\u001b", rendered)

    def test_build_summary_marks_redacted_key_collision_without_overwrite(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "token=supersecret-first-key": "first-value",
                "token=supersecret-second-key": "second-value",
                "kagemusha": {
                    "token=supersecret-nested-first": "nested-first",
                    "token=supersecret-nested-second": "nested-second",
                },
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        slot_summary = summary["slots"][0]
        self.assertTrue(slot_summary[device_lab.SUMMARY_REDACTION_KEY_COLLISION_FIELD])
        self.assertEqual(slot_summary[device_lab.SECRET_PATH_REDACTION], "first-value")
        self.assertEqual(
            slot_summary["kagemusha"][device_lab.SECRET_PATH_REDACTION],
            "nested-first",
        )
        self.assertNotIn("second-value", rendered)
        self.assertNotIn("nested-second", rendered)
        self.assertNotIn("supersecret", rendered)

    def test_build_summary_normalizes_malformed_direct_report_status(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "errors": ["scan aborted"],
                "kagemusha": {"required": True},
            },
            {
                "slot": "slot-1",
                "status": "OK",
                "errors": ["status was uppercase"],
                "kagemusha": {"required": True},
            },
            "not-a-report",
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(summary["ok"], 0)
        self.assertEqual(summary["failed"], 3)
        self.assertEqual(summary["slots"][0]["status"], "error")
        self.assertEqual(summary["slots"][1]["status"], "error")
        self.assertEqual(summary["slots"][2]["status"], "error")
        self.assertTrue(summary["slots"][0][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][1][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][2][device_lab.SUMMARY_STATUS_NORMALIZED_FIELD])
        self.assertNotIn("Traceback", rendered)

    def test_build_summary_normalizes_malformed_direct_report_errors(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "errors": "token=supersecret-error",
            },
            {
                "slot": "slot-1",
                "status": "error",
                "errors": [
                    "plain error",
                    {"token=supersecret-error-key": "secret-value"},
                    7,
                ],
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(
            summary["slots"][0]["errors"],
            [device_lab.SUMMARY_ERROR_REDACTION],
        )
        self.assertEqual(
            summary["slots"][1]["errors"],
            [
                "plain error",
                device_lab.SUMMARY_ERROR_REDACTION,
                device_lab.SUMMARY_ERROR_REDACTION,
            ],
        )
        self.assertTrue(summary["slots"][0][device_lab.SUMMARY_ERRORS_NORMALIZED_FIELD])
        self.assertTrue(summary["slots"][1][device_lab.SUMMARY_ERRORS_NORMALIZED_FIELD])
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("secret-value", rendered)

    def test_build_summary_normalizes_non_string_direct_report_keys(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                7: "first-value",
                ("tuple-key",): "second-value",
                "kagemusha": {11: "nested-first", 12: "nested-second"},
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD]
        )
        self.assertTrue(slot_summary[device_lab.SUMMARY_REDACTION_KEY_COLLISION_FIELD])
        self.assertEqual(
            slot_summary[device_lab.SUMMARY_NON_STRING_KEY_REDACTION],
            "first-value",
        )
        self.assertEqual(
            slot_summary["kagemusha"][device_lab.SUMMARY_NON_STRING_KEY_REDACTION],
            "nested-first",
        )
        self.assertNotIn("second-value", rendered)
        self.assertNotIn("nested-second", rendered)
        self.assertNotIn("tuple-key", rendered)

    def test_build_summary_redacts_nonfinite_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "duration_seconds": float("nan"),
                "kagemusha": {
                    "required": True,
                    "latency_seconds": float("inf"),
                },
                "samples": [1.0, float("-inf")],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["duration_seconds"],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["latency_seconds"],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][1],
            device_lab.SUMMARY_NONFINITE_NUMBER_REDACTION,
        )
        self.assertNotIn("NaN", rendered)
        self.assertNotIn("Infinity", rendered)

    def test_build_summary_normalizes_finite_float_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "duration_seconds": 0.25,
                "kagemusha": {
                    "required": True,
                    "latency_seconds": 1.5,
                },
                "samples": [1.0, {"nested": 2.5}],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["duration_seconds"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["latency_seconds"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][0],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][1]["nested"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertNotIn("0.25", rendered)
        self.assertNotIn("1.5", rendered)
        self.assertNotIn("2.5", rendered)

    def test_build_summary_normalizes_unsupported_direct_report_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "error",
                "raw_bytes": b"token=supersecret-bytes",
                "path_value": Path("token=supersecret-path"),
                "kagemusha": {
                    "required": True,
                    "object_value": object(),
                },
                "samples": [{"nested_set"}],
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(Path(temp), reports)
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(
            slot_summary[device_lab.SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD]
        )
        self.assertEqual(
            slot_summary["raw_bytes"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["path_value"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["kagemusha"]["object_value"],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertEqual(
            slot_summary["samples"][0],
            device_lab.SUMMARY_UNSUPPORTED_VALUE_REDACTION,
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("nested_set", rendered)

    def test_build_summary_normalizes_malformed_kagemusha_report_shape(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": "token=supersecret-kagemusha",
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)
        slot_summary = summary["slots"][0]

        self.assertTrue(slot_summary[device_lab.SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD])
        self.assertEqual(slot_summary["kagemusha"], {})
        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(summary["kagemusha"]["duplicate_bindings"], {})
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertNotIn("supersecret", rendered)

    def test_build_summary_ignores_malformed_direct_device_family_values(self) -> None:
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_family": [
                        device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0]
                    ],
                },
            },
            {
                "slot": "slot-1",
                "status": "ok",
                "kagemusha": {
                    "device_family": "token=supersecret-family",
                },
            },
            {
                "slot": "slot-2",
                "status": "ok",
                "kagemusha": {
                    "device_family": "Unreviewed Device",
                },
            },
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                require_kagemusha_standard_matrix=True,
            )
        rendered = json.dumps(summary, allow_nan=False, sort_keys=True)

        self.assertEqual(summary["kagemusha"]["covered_device_families"], [])
        self.assertEqual(
            summary["kagemusha"]["missing_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("Unreviewed Device", summary["kagemusha"]["covered_device_families"])

    def test_build_summary_ignores_non_sha256_direct_trusted_signer_keys(self) -> None:
        signer_digest = "d" * 64
        reports = [
            {
                "slot": "slot-0",
                "status": "ok",
                "kagemusha": {
                    "device_family": device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[0],
                },
            }
        ]

        with tempfile.TemporaryDirectory() as temp:
            summary = device_lab.build_summary(
                Path(temp),
                reports,
                require_kagemusha_production_evidence=True,
                trusted_signer_public_keys={
                    signer_digest: Path(temp) / "safe.pem",
                    "token=supersecret-signer": Path(temp) / "unsafe.pem",
                    "E" * 64: Path(temp) / "uppercase.pem",
                },
            )
        rendered = json.dumps(summary, allow_nan=False)

        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            [signer_digest],
        )
        self.assertNotIn("supersecret", rendered)
        self.assertNotIn("E" * 64, rendered)

    def test_json_summary_reports_kagemusha_matrix_and_signer_pins(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            for index, family in enumerate(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES):
                create_slot(root, f"slot-{index}", family, signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-standard-matrix",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary = json.loads(summary_path.read_text(encoding="utf-8"))

        self.assertEqual(status, 0)
        self.assertEqual(summary["ok"], len(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES))
        self.assertEqual(summary["failed"], 0)
        self.assertEqual(summary["root"], device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL)
        self.assertTrue(summary["kagemusha"]["production_evidence_required"])
        self.assertTrue(summary["kagemusha"]["standard_matrix_required"])
        self.assertEqual(
            summary["kagemusha"]["required_device_families"],
            list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertEqual(
            sorted(summary["kagemusha"]["covered_device_families"]),
            sorted(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
        )
        self.assertEqual(summary["kagemusha"]["missing_device_families"], [])
        self.assertEqual(
            summary["kagemusha"]["trusted_signer_public_key_sha256"],
            [signer["public_key_sha256"]],
        )
        first_slot = summary["slots"][0]["kagemusha"]
        self.assertEqual(
            first_slot["offline_wallet_apk_path"],
            "evidence/offline-wallet-release.apk",
        )
        self.assertRegex(first_slot["offline_wallet_apk_sha256"], r"^[0-9a-f]{64}$")
        self.assertEqual(
            first_slot["native_bridge_abi_version"],
            device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
        )

    def test_json_summary_does_not_leak_trusted_signer_key_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(status, 0)
        self.assertNotIn(str(signer["public_key"]), summary_text)
        self.assertNotIn(str(signer["private_key"]), summary_text)
        self.assertIn(str(signer["public_key_sha256"]), summary_text)

    def test_json_summary_does_not_leak_device_lab_root_or_summary_output_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(io.StringIO()):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            summary_text = summary_path.read_text(encoding="utf-8")
            stdout_text = stdout.getvalue()

        self.assertEqual(status, 0)
        self.assertNotIn(str(root), summary_text)
        self.assertNotIn(str(summary_path), stdout_text)
        self.assertIn(device_lab.DEVICE_LAB_ROOT_SUMMARY_LABEL, summary_text)
        self.assertIn("[device-lab] wrote summary", stdout_text)

    def test_main_rejects_secret_looking_root_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_root = Path(temp) / "token=supersecret-slots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(secret_root),
                        "--allow-missing-root",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("--root must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_root), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("root missing; skipping", rendered)

    def test_main_rejects_control_root_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_root = Path(temp) / "control\nslots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(control_root),
                        "--allow-missing-root",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertIn("--root must not contain control characters", rendered)
        self.assertNotIn(str(control_root), rendered)
        self.assertNotIn("root missing; skipping", rendered)

    def test_main_rejects_control_slot_before_root_classify_without_leak(
        self,
    ) -> None:
        unsafe_slot = "slot-a\x1b[31m"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    device_lab,
                    "classify_device_lab_root_path",
                    side_effect=AssertionError("root classification must not run"),
                ) as classify_root,
                redirect_stdout(stdout),
                redirect_stderr(stderr),
            ):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--slot",
                        unsafe_slot,
                        "--require-slot",
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        classify_root.assert_not_called()
        self.assertIn("slot id 0 must not contain control characters", rendered)
        self.assertNotIn(unsafe_slot, rendered)
        self.assertNotIn("\x1b", rendered)

    def test_main_rejects_control_trusted_signer_before_slot_discovery_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            root.mkdir()
            unsafe_key = Path(temp) / "control\nsigner.pem"
            stdout = io.StringIO()
            stderr = io.StringIO()

            with (
                mock.patch.object(
                    device_lab,
                    "discover_slots",
                    side_effect=AssertionError("slot discovery must not run"),
                ) as discover_slots,
                mock.patch.object(
                    device_lab,
                    "load_trusted_signer_public_keys",
                    side_effect=AssertionError("trusted signer loading must not run"),
                ) as load_signers,
                redirect_stdout(stdout),
                redirect_stderr(stderr),
            ):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(unsafe_key),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        discover_slots.assert_not_called()
        load_signers.assert_not_called()
        self.assertIn(
            "--trusted-signer-public-key[0] must not contain control characters",
            rendered,
        )
        self.assertNotIn(str(unsafe_key), rendered)

    def test_json_summary_rejects_secret_looking_output_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(secret_summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(secret_summary_path.exists())
        self.assertIn("--json-out must not contain secret-looking material", rendered)
        self.assertNotIn(str(secret_summary_path), rendered)
        self.assertNotIn("token=supersecret", rendered)
        self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_json_summary_rejects_control_output_without_leak(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            control_summary_path = Path(temp) / "control\nsummary" / "summary.json"
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(
                root,
                "pixel8",
                device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2],
                signer,
            )
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(control_summary_path),
                    ]
                )
            rendered = stdout.getvalue() + stderr.getvalue()

        self.assertEqual(status, 1)
        self.assertFalse(control_summary_path.exists())
        self.assertFalse(control_summary_path.parent.exists())
        self.assertIn("--json-out must not contain control characters", rendered)
        self.assertNotIn(str(control_summary_path), rendered)
        self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_write_summary_rejects_secret_output_path_directly_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            secret_summary_path = Path(temp) / "token=supersecret-summary.json"

            errors = device_lab.write_summary(secret_summary_path, {"ok": False})
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["--json-out must not contain secret-looking material"])
        self.assertFalse(secret_summary_path.exists())
        self.assertNotIn(str(secret_summary_path), rendered)
        self.assertNotIn("token=supersecret", rendered)

    def test_write_summary_rejects_control_output_path_directly_without_leak(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            control_summary_path = Path(temp) / "control\nsummary" / "summary.json"

            errors = device_lab.write_summary(control_summary_path, {"ok": False})
            rendered = json.dumps(errors)

        self.assertEqual(errors, ["--json-out must not contain control characters"])
        self.assertFalse(control_summary_path.exists())
        self.assertFalse(control_summary_path.parent.exists())
        self.assertNotIn(str(control_summary_path), rendered)

    def test_write_summary_rejects_nonfinite_json_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            errors = device_lab.write_summary(summary_path, {"value": float("inf")})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(errors, ["--json-out summary is not strict JSON"])
        self.assertFalse(summary_path.exists())
        self.assertEqual(temp_files, [])

    def test_write_summary_rejects_oversized_json_before_write(self) -> None:
        summary = {"ok": False}
        summary_text = json.dumps(summary, indent=2, allow_nan=False) + "\n"
        test_limit = len(summary_text) - 1
        old_limit = device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES
        try:
            device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = test_limit
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                errors = device_lab.write_summary(summary_path, summary)
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES = old_limit

        self.assertEqual(
            errors,
            ["--json-out must be no more than " f"{test_limit} bytes"],
        )
        self.assertFalse(summary_path.exists())
        self.assertEqual(temp_files, [])

    def test_validate_summary_output_path_uses_lstat_before_parent_is_dir_preflight(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = device_lab.validate_summary_output_path(
                    summary_path,
                    "--json-out",
                )
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertFalse(summary_path.exists())

    def test_validate_summary_output_path_rejects_parent_metadata_failure(
        self,
    ) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.validate_summary_output_path(
                    summary_path,
                    "--json-out",
                )
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_validate_summary_output_path_rejects_aliases_before_parent_metadata(
        self,
    ) -> None:
        cases = (
            (
                lambda base: base / "summary\\out.json",
                "--json-out must not contain backslashes",
            ),
            (
                lambda base: base / "summary" / ".." / "out.json",
                "--json-out must be canonical",
            ),
        )
        for path_factory, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                with tempfile.TemporaryDirectory() as temp:
                    summary_path = path_factory(Path(temp))

                    with mock.patch.object(
                        Path,
                        "lstat",
                        side_effect=AssertionError(
                            "alias summary output should fail before metadata"
                        ),
                    ):
                        errors = device_lab.validate_summary_output_path(
                            summary_path,
                            "--json-out",
                        )

                    output_exists = summary_path.exists()

                self.assertEqual(errors, [expected_error])
                self.assertFalse(output_exists)

    def test_write_summary_uses_lstat_before_parent_is_dir_preflight(self) -> None:
        path_type = type(Path("."))
        original_is_dir = path_type.is_dir

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_is_dir(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary writer parent is_dir failure")
                    return original_is_dir(path, *args, **kwargs)

                path_type.is_dir = failing_is_dir

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.is_dir = original_is_dir

        self.assertEqual(errors, [])
        self.assertIn('"ok": false', summary_text)

    def test_write_summary_rejects_parent_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary-parent" / "summary.json"
                summary_path.parent.mkdir()

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path.parent:
                        raise OSError("simulated scanner summary writer parent metadata failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                output_exists = summary_path.exists()
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out parent directory metadata could not be read"])
        self.assertFalse(output_exists)

    def test_write_summary_rejects_parent_create_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "missing-parent" / "summary.json"

            def failing_mkdir(path: Path, *args, **kwargs):
                if path == summary_path.parent:
                    raise OSError("simulated summary parent mkdir failure")
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", failing_mkdir):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(errors, ["--json-out parent directory could not be created"])
        self.assertFalse(summary_path.exists())

    def test_write_summary_rejects_file_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_lstat = path_type.lstat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_lstat(path: Path, *args, **kwargs):
                    if path == summary_path:
                        raise OSError("simulated summary lstat failure")
                    return original_lstat(path, *args, **kwargs)

                path_type.lstat = failing_lstat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.lstat = original_lstat

        self.assertEqual(errors, ["--json-out file metadata could not be read"])
        self.assertEqual(summary_text, "existing summary\n")

    def test_write_summary_rejects_hardlink_metadata_failure_before_write(self) -> None:
        path_type = type(Path("."))
        original_stat = path_type.stat

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"
                summary_path.write_text("existing summary\n", encoding="utf-8")

                def failing_stat(path: Path, *args, **kwargs):
                    if path == summary_path and kwargs.get("follow_symlinks", True):
                        raise OSError("simulated summary stat failure")
                    return original_stat(path, *args, **kwargs)

                path_type.stat = failing_stat

                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            path_type.stat = original_stat

        self.assertEqual(errors, ["--json-out hardlink metadata could not be read"])
        self.assertEqual(summary_text, "existing summary\n")

    def test_write_summary_rejects_write_failure_after_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            with mock.patch.object(
                device_lab.os,
                "fsync",
                side_effect=OSError("simulated summary fsync failure"),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(errors, ["--json-out could not be written"])
        self.assertFalse(summary_path.exists())

    def test_write_summary_preserves_existing_output_on_replace_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            summary_path.write_text("existing summary\n", encoding="utf-8")

            with mock.patch.object(
                device_lab.os,
                "replace",
                side_effect=OSError("simulated summary replace failure"),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = summary_path.read_text(encoding="utf-8")
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(errors, ["--json-out could not be written"])
        self.assertEqual(summary_text, "existing summary\n")
        self.assertEqual(temp_files, [])

    def test_write_summary_reports_temp_cleanup_failure_after_write_failure(
        self,
    ) -> None:
        original_unlink = device_lab.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"

            def failing_replace(src: Path, dst: Path) -> None:
                raise OSError("simulated summary replace failure")

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{summary_path.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated summary temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(device_lab.os, "replace", failing_replace),
                mock.patch.object(device_lab.os, "unlink", failing_temp_unlink),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(
            errors,
            [
                "--json-out could not be written",
                "--json-out temporary file could not be removed",
            ],
        )
        self.assertEqual(len(temp_files), 1)

    def test_write_summary_reports_temp_cleanup_failure_after_post_stage_validation_failure(
        self,
    ) -> None:
        original_validate = device_lab.validate_summary_output_path
        original_unlink = device_lab.os.unlink

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            validation_calls = 0

            def racing_validate(path: Path, label: str) -> list[str]:
                nonlocal validation_calls
                if path == summary_path and label == "--json-out":
                    validation_calls += 1
                    if validation_calls == 2:
                        return ["--json-out changed after staging"]
                return original_validate(path, label)

            def failing_temp_unlink(path: str, *args, **kwargs):
                if (
                    Path(path).name.startswith(f".{summary_path.name}.")
                    and Path(path).suffix == ".tmp"
                ):
                    raise OSError("simulated summary temp cleanup failure")
                return original_unlink(path, *args, **kwargs)

            with (
                mock.patch.object(
                    device_lab,
                    "validate_summary_output_path",
                    racing_validate,
                ),
                mock.patch.object(device_lab.os, "unlink", failing_temp_unlink),
            ):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
            output_exists = summary_path.exists()

        self.assertEqual(
            errors,
            [
                "--json-out changed after staging",
                "--json-out temporary file could not be removed",
            ],
        )
        self.assertEqual(validation_calls, 2)
        self.assertFalse(output_exists)
        self.assertEqual(len(temp_files), 1)

    def test_write_summary_temp_cleanup_rejects_swapped_temp_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp) / ".summary.json.swap.tmp"
            temp_path.write_text("original\n", encoding="utf-8")
            temp_identity = device_lab._file_identity(temp_path.lstat())
            swapped_temp = Path(temp) / "original-temp-file"
            temp_path.rename(swapped_temp)
            temp_path.write_text("do not remove\n", encoding="utf-8")

            errors = device_lab._cleanup_summary_output(
                temp_path,
                temp_identity,
            )
            victim_survived = temp_path.read_text(encoding="utf-8")
            original_survived = swapped_temp.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--json-out temporary file changed before cleanup"])
        self.assertEqual(victim_survived, "do not remove\n")
        self.assertEqual(original_survived, "original\n")

    def test_write_summary_rejects_parent_directory_sync_failure_after_replace(
        self,
    ) -> None:
        original_fsync = device_lab.os.fsync

        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            sync_calls = 0

            def failing_parent_fsync(fd: int) -> None:
                nonlocal sync_calls
                sync_calls += 1
                if sync_calls == 2:
                    raise OSError("simulated summary parent sync failure")
                original_fsync(fd)

            with mock.patch.object(device_lab.os, "fsync", failing_parent_fsync):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = summary_path.read_text(encoding="utf-8")
            temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))

        self.assertEqual(sync_calls, 2)
        self.assertEqual(errors, ["--json-out parent directory could not be synced"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')
        self.assertEqual(temp_files, [])

    def test_write_summary_rejects_parent_directory_identity_swap_before_sync(
        self,
    ) -> None:
        original_open = device_lab.os.open

        with tempfile.TemporaryDirectory() as temp:
            wrapper = Path(temp)
            root = wrapper / "summary-output-root"
            root.mkdir()
            summary_path = root / "summary.json"
            swapped_root = wrapper / "summary-output-root-swapped"
            swapped = False

            def swapping_parent_open(path: Path, flags: int, *args, **kwargs):
                nonlocal swapped
                if Path(path) == summary_path.parent and not swapped:
                    summary_path.parent.rename(swapped_root)
                    summary_path.parent.mkdir()
                    swapped = True
                return original_open(path, flags, *args, **kwargs)

            with mock.patch.object(device_lab.os, "open", swapping_parent_open):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = (swapped_root / summary_path.name).read_text(
                encoding="utf-8"
            )

        self.assertTrue(swapped)
        self.assertEqual(errors, ["--json-out parent directory changed before sync"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')

    def test_write_summary_rejects_symlink_swap_before_replace(self) -> None:
        original_validate = device_lab.validate_summary_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                summary_path = root / "summary.json"
                alias_target = root / "external-summary.json"
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == summary_path and calls == 2:
                        alias_target.write_text("external\n", encoding="utf-8")
                        try:
                            path.symlink_to(alias_target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                device_lab.validate_summary_output_path = validate_then_alias
                errors = device_lab.write_summary(summary_path, {"ok": False})
                target_text = alias_target.read_text(encoding="utf-8")
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.validate_summary_output_path = original_validate

        self.assertEqual(errors, ["--json-out must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertEqual(temp_files, [])

    def test_write_summary_rejects_readback_mismatch(self) -> None:
        original_read_summary_output_text = device_lab._read_summary_output_text  # type: ignore[attr-defined]

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                def mismatching_read_summary_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                ) -> tuple[str | None, list[str]]:
                    if path == summary_path:
                        return '{"ok": true}\n', []
                    return original_read_summary_output_text(path, expected_stat)

                device_lab._read_summary_output_text = mismatching_read_summary_output_text  # type: ignore[attr-defined]
                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
        finally:
            device_lab._read_summary_output_text = original_read_summary_output_text  # type: ignore[attr-defined]

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')

    def test_write_summary_rejects_readback_failure(self) -> None:
        original_read_summary_output_text = device_lab._read_summary_output_text  # type: ignore[attr-defined]

        try:
            with tempfile.TemporaryDirectory() as temp:
                summary_path = Path(temp) / "summary.json"

                def failing_read_summary_output_text(
                    path: Path,
                    expected_stat: os.stat_result,
                ) -> tuple[str | None, list[str]]:
                    if path == summary_path:
                        return None, ["--json-out write verification failed"]
                    return original_read_summary_output_text(path, expected_stat)

                device_lab._read_summary_output_text = failing_read_summary_output_text  # type: ignore[attr-defined]
                errors = device_lab.write_summary(summary_path, {"ok": False})
                summary_text = summary_path.read_text(encoding="utf-8")
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab._read_summary_output_text = original_read_summary_output_text  # type: ignore[attr-defined]

        self.assertEqual(errors, ["--json-out write verification failed"])
        self.assertEqual(summary_text, '{\n  "ok": false\n}\n')
        self.assertEqual(temp_files, [])

    def test_read_summary_output_rejects_oversized_readback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary_path = Path(temp) / "summary.json"
            with summary_path.open("wb") as handle:
                handle.seek(device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES)
                handle.write(b"x")
            expected_stat = summary_path.lstat()

            text, errors = device_lab._read_summary_output_text(  # type: ignore[attr-defined]
                summary_path,
                expected_stat,
            )

        self.assertIsNone(text)
        self.assertEqual(
            errors,
            [
                "--json-out must be no more than "
                f"{device_lab.MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
            ],
        )

    def test_write_summary_rejects_regular_file_swap_before_readback(self) -> None:
        original_open = Path.open

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "summary.json"
            replacement = root / "replacement-summary.json"
            replacement.write_text('{"ok":true}\n', encoding="utf-8")
            swapped = False

            def swapping_open(path: Path, *args, **kwargs):
                nonlocal swapped
                mode = args[0] if args else kwargs.get("mode", "r")
                if path == summary_path and "r" in mode and not swapped:
                    replacement.replace(summary_path)
                    swapped = True
                return original_open(path, *args, **kwargs)

            with mock.patch.object(Path, "open", swapping_open):
                errors = device_lab.write_summary(summary_path, {"ok": False})
            summary_text = summary_path.read_text(encoding="utf-8")

        self.assertEqual(errors, ["--json-out changed while being read"])
        self.assertEqual(summary_text, '{"ok":true}\n')

    def test_write_summary_rejects_symlink_swap_after_replace(self) -> None:
        original_validate = device_lab.validate_summary_output_path

        try:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                summary_path = root / "summary.json"
                alias_target = root / "external-summary.json"
                alias_target.write_text("external\n", encoding="utf-8")
                calls = 0

                def validate_then_alias(path: Path, label: str) -> list[str]:
                    nonlocal calls
                    calls += 1
                    if path == summary_path and calls == 3:
                        path.unlink(missing_ok=True)
                        try:
                            path.symlink_to(alias_target)
                        except (NotImplementedError, OSError) as exc:
                            self.skipTest(
                                "symlinks are not available in this test "
                                f"environment: {exc}"
                            )
                    return original_validate(path, label)

                device_lab.validate_summary_output_path = validate_then_alias
                errors = device_lab.write_summary(summary_path, {"ok": False})
                target_text = alias_target.read_text(encoding="utf-8")
                output_is_symlink = summary_path.is_symlink()
                temp_files = list(summary_path.parent.glob(".summary.json.*.tmp"))
        finally:
            device_lab.validate_summary_output_path = original_validate

        self.assertEqual(errors, ["--json-out must not be a symlink"])
        self.assertEqual(target_text, "external\n")
        self.assertTrue(output_is_symlink)
        self.assertEqual(temp_files, [])

    def test_write_summary_rechecks_parent_after_create_before_write(self) -> None:
        path_type = type(Path("."))
        original_mkdir = path_type.mkdir

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            summary_path = root / "late-linked-summary" / "summary.json"
            alias_target = root / "external-summary"
            alias_target.mkdir()

            def replacing_mkdir(path: Path, *args, **kwargs):
                if path == summary_path.parent:
                    create_dir_symlink(self, path, alias_target)
                    return None
                return original_mkdir(path, *args, **kwargs)

            with mock.patch.object(path_type, "mkdir", replacing_mkdir):
                errors = device_lab.write_summary(summary_path, {"ok": False})

        self.assertEqual(errors, ["--json-out parent directory must not be a symlink"])
        self.assertFalse((alias_target / "summary.json").exists())

    def test_json_summary_rejects_symlinked_output_without_following_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            alias_target = Path(temp) / "external-summary.json"
            alias_target.write_text("do not overwrite\n", encoding="utf-8")
            try:
                summary_path.symlink_to(alias_target)
            except (NotImplementedError, OSError) as exc:
                self.skipTest(f"symlinks are not available in this test environment: {exc}")
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

            self.assertEqual(status, 1)
            self.assertEqual(alias_target.read_text(encoding="utf-8"), "do not overwrite\n")
            self.assertIn("--json-out must not be a symlink", rendered)
            self.assertNotIn(str(alias_target), rendered)
            self.assertNotIn("[device-lab] wrote summary", rendered)

    def test_json_summary_rejects_hardlinked_output_without_overwriting_alias(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "slots"
            summary_path = Path(temp) / "summary.json"
            alias_target = Path(temp) / "external-summary.json"
            alias_target.write_text("do not overwrite\n", encoding="utf-8")
            summary_path.write_text("placeholder\n", encoding="utf-8")
            replace_with_hardlink(self, summary_path, alias_target)
            signer = create_test_signer(Path(temp) / "keys")
            create_slot(root, "pixel8", device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES[2], signer)
            stdout = io.StringIO()
            stderr = io.StringIO()

            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = device_lab.main(
                    [
                        "--root",
                        str(root),
                        "--require-slot",
                        "--require-kagemusha-production-evidence",
                        "--trusted-signer-public-key",
                        str(signer["public_key"]),
                        "--json-out",
                        str(summary_path),
                    ]
                )

            rendered = stdout.getvalue() + stderr.getvalue()

            self.assertEqual(status, 1)
            self.assertEqual(alias_target.read_text(encoding="utf-8"), "do not overwrite\n")
            self.assertIn("--json-out must not be hardlinked", rendered)
            self.assertNotIn(str(alias_target), rendered)
            self.assertNotIn("[device-lab] wrote summary", rendered)


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
