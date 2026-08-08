from __future__ import annotations

import hashlib
import json
import os
from dataclasses import replace
from pathlib import Path
import sys
import time

import pytest

from scripts import taira_privacy_sealed_controller as controller


ROOT = Path(__file__).resolve().parents[2]
DRIVER_SHA256 = "99" * 32


def _json_response(value: object, status: int = 200) -> controller.HttpObservation:
    return controller.HttpObservation(
        status,
        {"content-type": "application/json"},
        json.dumps(value, separators=(",", ":")).encode("ascii"),
    )


def _request(candidate: str, nonce: int) -> controller.VeRangeActionRequest:
    return controller.VeRangeActionRequest(
        asset_definition_id="verange_value#privacy",
        candidate_binding_sha256=candidate,
        chain_id="00000000-0000-0000-0000-000000000001",
        creation_time_millis=1_800_000_000_000 + nonce,
        genesis_hash_hex="22" * 32,
        nonce=nonce,
        ttl_millis=120_000,
        values=(0, 1, 17, 0xFFFF_FFFF),
    )


def _artifact(request: controller.VeRangeActionRequest, marker: int) -> controller.ActionArtifact:
    request_bytes = request.canonical_bytes()
    request_body = json.loads(request_bytes)
    transaction = b"NRT0" + bytes([marker]) * 64
    transaction_sha = hashlib.sha256(transaction).hexdigest()
    transaction_hash = f"{marker:02x}" * 32
    response = {
        "candidate_binding_sha256": request.candidate_binding_sha256,
        "operation": controller.VERANGE_OPERATION,
        "protocol": controller.VERANGE_PROTOCOL,
        "request_id": request_body["request_id"],
        "schema": controller.DRIVER_RESPONSE_SCHEMA,
        "schema_version": controller.DRIVER_SCHEMA_VERSION,
        "transaction_hash_hex": transaction_hash,
        "transaction_norito_hex": transaction.hex(),
        "transaction_sha256": transaction_sha,
    }
    return controller.ActionArtifact(
        request_id=request_body["request_id"],
        request_bytes=request_bytes,
        response_bytes=controller._driver_json_bytes(response),
        transaction=transaction,
        transaction_hash_hex=transaction_hash,
        transaction_sha256=transaction_sha,
        action_driver_sha256=DRIVER_SHA256,
    )


def _peers() -> tuple[controller.PeerEndpoint, ...]:
    return tuple(
        controller.PeerEndpoint(f"peer-{index}", f"http://127.0.0.1:{8080 + index}")
        for index in range(1, 5)
    )


def test_release_issuance_barrier_names_every_missing_native_operation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("IROHA_PRIVACY_ALLOW_SELF_REPORTED_RECEIPT", "1")
    missing = controller.missing_release_operations()
    assert len(missing) == 11
    assert controller.VERANGE_PROTOCOL not in missing
    assert "iroha-zk-ams-v1" in missing
    assert "iroha-zk-x509-stark-p256-v0" in missing
    with pytest.raises(
        controller.SealedPrivacyControllerError,
        match="release issuance is closed",
    ) as raised:
        controller.require_complete_release_operation_surface()
    assert "iroha-zk-ams-v1" in str(raised.value)
    assert "iroha-zk-x509-stark-p256-v0" in str(raised.value)
    with pytest.raises(TypeError):
        controller.CONTROLLER_OWNED_OPERATIONS["iroha-zk-ams-v1"] = "fake"  # type: ignore[index]


def test_driver_request_is_canonical_bounded_and_contains_no_network_authority() -> None:
    payload = _request("11" * 32, 7).canonical_bytes()
    assert payload.endswith(b"\n")
    assert len(payload) <= controller.MAX_DRIVER_REQUEST_BYTES
    assert b"endpoint" not in payload
    assert b"credential" not in payload
    assert b"password" not in payload
    parsed = json.loads(payload)
    request_id = parsed.pop("request_id")
    assert request_id == hashlib.sha256(
        controller.REQUEST_ID_DOMAIN + controller._driver_json_bytes(parsed)[:-1]
    ).hexdigest()


def test_driver_response_rejects_digest_shell_suffix_and_duplicate_fields() -> None:
    request = _request("11" * 32, 7)
    artifact = _artifact(request, 0x33)
    parsed = controller._parse_action_response(
        artifact.response_bytes, artifact.request_bytes
    )
    assert parsed.transaction == artifact.transaction

    response = json.loads(artifact.response_bytes)
    response["transaction_norito_hex"] = "44"
    with pytest.raises(controller.SealedPrivacyControllerError, match="digest differs"):
        controller._parse_action_response(
            controller._driver_json_bytes(response), artifact.request_bytes
        )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not JSON"):
        controller._parse_action_response(
            artifact.response_bytes + b"{}\n", artifact.request_bytes
        )
    duplicate = artifact.response_bytes.replace(
        b'{"candidate_binding_sha256":',
        b'{"operation":"build-verange-action-v1","candidate_binding_sha256":',
        1,
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not JSON"):
        controller._parse_action_response(duplicate, artifact.request_bytes)


@pytest.mark.parametrize(
    "label,root",
    (
        ("peer-1", "https://127.0.0.1:8081"),
        ("peer-1", "http://localhost:8081"),
        ("peer-1", "http://user@127.0.0.1:8081"),
        ("peer-0", "http://127.0.0.1:8081"),
        ("peer-1", "http://127.0.0.1:8081/path"),
    ),
)
def test_direct_peer_roots_reject_redirectable_or_credential_bearing_aliases(
    label: str, root: str
) -> None:
    with pytest.raises(controller.SealedPrivacyControllerError, match="loopback"):
        controller.PeerEndpoint(label, root)


def test_peer_set_rejects_missing_duplicate_and_reordered_rows() -> None:
    peers = _peers()
    for hostile in (peers[:3], peers[::-1], (*peers[:3], peers[2])):
        with pytest.raises(controller.SealedPrivacyControllerError):
            controller.require_exact_peer_set(hostile)


def _write_driver(path: Path, source: str) -> tuple[Path, str]:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o555)
    resolved = path.resolve(strict=True)
    return resolved, hashlib.sha256(resolved.read_bytes()).hexdigest()


def test_driver_admission_requires_exact_nonzero_digest_and_pins_bytes(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    driver, digest = _write_driver(tmp_path / "driver", "#!/bin/sh\nexit 0\n")
    with controller._pinned_action_driver(driver, digest, tmp_path) as pinned:
        assert pinned.sha256 == digest
        assert pinned.execution_method in {
            "darwin-private-fd-copy",
            "linux-pinned-fd",
        }
        assert os.path.samestat(os.fstat(pinned.source_descriptor), driver.stat())
        if sys.platform == "linux":
            assert pinned.execution_path.startswith("/proc/self/fd/")
            assert pinned.inherited_descriptors == (pinned.source_descriptor,)
        else:
            assert pinned.private_directory is not None
            assert Path(pinned.execution_path).parent == pinned.private_directory
    assert not tuple(tmp_path.glob(".privacy-action-driver-*"))

    with pytest.raises(controller.SealedPrivacyControllerError, match="nonzero"):
        controller._admit_action_driver(driver, "0" * 64, tmp_path)
    with pytest.raises(controller.SealedPrivacyControllerError, match="differ"):
        controller._admit_action_driver(driver, "aa" * 32, tmp_path)


def test_driver_admission_rejects_writable_hardlinked_and_nonregular_inputs(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    writable = tmp_path / "writable"
    writable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    writable.chmod(0o755)
    digest = hashlib.sha256(writable.read_bytes()).hexdigest()
    with pytest.raises(controller.SealedPrivacyControllerError, match="non-writable"):
        controller._admit_action_driver(writable.resolve(), digest, tmp_path)

    driver, digest = _write_driver(tmp_path / "linked", "#!/bin/sh\nexit 0\n")
    os.link(driver, tmp_path / "second-link")
    with pytest.raises(controller.SealedPrivacyControllerError, match="singly linked"):
        controller._admit_action_driver(driver, digest, tmp_path)

    directory = tmp_path / "not-a-file"
    directory.mkdir(mode=0o500)
    with pytest.raises(controller.SealedPrivacyControllerError, match="regular file"):
        controller._admit_action_driver(directory, "aa" * 32, tmp_path)


def test_pinned_driver_detects_source_path_substitution(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    driver, digest = _write_driver(tmp_path / "driver", "#!/bin/sh\nexit 0\n")
    replacement, _ = _write_driver(
        tmp_path / "replacement", "#!/bin/sh\nexit 1\n"
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="identity changed"):
        with controller._pinned_action_driver(driver, digest, tmp_path):
            driver.rename(tmp_path / "displaced")
            replacement.rename(driver)


def test_driver_invocation_kills_retained_descendants_and_reports_digest(
    tmp_path: Path,
) -> None:
    tmp_path.chmod(0o700)
    source = f"""#!{sys.executable}
import hashlib
import json
import subprocess
import sys

request = json.load(sys.stdin)
with open("descendant.pid", "w", encoding="ascii") as output:
    child = subprocess.Popen(
        ["/bin/sleep", "30"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    output.write(str(child.pid) + "\\n")
transaction = b"proof-bearing-test-action"
response = {{
    "candidate_binding_sha256": request["candidate_binding_sha256"],
    "operation": "build-verange-action-v1",
    "protocol": "verange-transparent-range-v1",
    "request_id": request["request_id"],
    "schema": "iroha.taira.privacy_action_driver_response",
    "schema_version": 1,
    "transaction_hash_hex": "33" * 32,
    "transaction_norito_hex": transaction.hex(),
    "transaction_sha256": hashlib.sha256(transaction).hexdigest(),
}}
sys.stdout.write(json.dumps(response, sort_keys=True, separators=(",", ":")) + "\\n")
"""
    driver, digest = _write_driver(tmp_path / "driver", source)
    artifact = controller.invoke_action_driver(
        driver,
        _request("11" * 32, 7),
        expected_sha256=digest,
        work_directory=tmp_path,
        timeout_seconds=5,
    )
    assert artifact.action_driver_sha256 == digest
    child_pid = int((tmp_path / "descendant.pid").read_text(encoding="ascii"))
    for _ in range(100):
        try:
            os.kill(child_pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.01)
    else:
        pytest.fail("action-driver descendant survived controller cleanup")


def test_case_rejects_a_substituted_driver_digest_before_network_use(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidate = "11" * 32
    primary_request = _request(candidate, 7)
    successor_request = _request(candidate, 8)
    primary = replace(
        _artifact(primary_request, 0x33), action_driver_sha256="aa" * 32
    )
    successor = _artifact(successor_request, 0x44)
    actions = iter((primary, successor))
    monkeypatch.setattr(
        controller,
        "invoke_action_driver",
        lambda *_args, **_kwargs: next(actions),
    )

    class Supervisor:
        peer = _peers()[-1]

    with pytest.raises(controller.SealedPrivacyControllerError, match="wrong driver"):
        controller.run_verange_diagnostic_case(
            case="driver-substitution",
            candidate_binding_sha256=candidate,
            action_driver=tmp_path / "unused-by-mock",
            expected_action_driver_sha256=DRIVER_SHA256,
            work_directory=tmp_path,
            peers=_peers(),
            restarted_supervisor=Supervisor(),  # type: ignore[arg-type]
            primary_request=primary_request,
            successor_request=successor_request,
            timeout_seconds=5,
        )


def test_controller_owns_full_verange_network_sequence_and_emits_canonical_records(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    candidate = "11" * 32
    primary_request = _request(candidate, 7)
    successor_request = _request(candidate, 8)
    primary = _artifact(primary_request, 0x33)
    successor = _artifact(successor_request, 0x44)
    actions = iter((primary, successor))
    monkeypatch.setattr(
        controller,
        "invoke_action_driver",
        lambda *_args, **_kwargs: next(actions),
    )

    def inspect(
        action: controller.ActionArtifact,
        request: controller.VeRangeActionRequest,
    ) -> controller.NativeVeRangeInspection:
        request_id = bytes.fromhex(action.request_id)
        candidate_bytes = bytes.fromhex(request.candidate_binding_sha256)
        policy_id = hashlib.sha256(
            controller.DRIVER_SEED_DOMAIN
            + candidate_bytes
            + request_id
            + b"\x02"
        ).hexdigest()
        return controller.NativeVeRangeInspection(
            action.transaction_hash_hex,
            "55" * 32,
            "66" * 32,
            "77" * 32,
            policy_id,
            1024,
            512,
        )

    monkeypatch.setattr(controller, "_inspect_native_verange_action", inspect)

    peers = _peers()
    fleet_rounds = (
        (10, "aa" * 32),
        (11, "bb" * 32),
        (11, "bb" * 32),
        (12, "cc" * 32),
    )
    status_calls = 0
    peer_round: dict[str, int] = {}
    post_statuses = iter((202, 409, 400, 202))

    def exchange(
        peer: controller.PeerEndpoint,
        method: str,
        path: str,
        *,
        body: bytes | None,
        timeout_seconds: float,
    ) -> controller.HttpObservation:
        nonlocal status_calls
        assert timeout_seconds > 0
        if method == "POST":
            return controller.HttpObservation(next(post_statuses), {}, b"")
        if path == "/status":
            round_index = status_calls // 4
            peer_round[peer.label] = round_index
            status_calls += 1
            return _json_response({"blocks": fleet_rounds[round_index][0]})
        if path == "/v1/sumeragi/status":
            height, block_hash = fleet_rounds[peer_round[peer.label]]
            return _json_response(
                {
                    "last_committed_height": height,
                    "last_committed_subject": {"block_hash": block_hash},
                }
            )
        if path.startswith("/v1/pipeline/transactions/status?"):
            query = urllib_parse(path)
            return _json_response(
                {
                    "hash": query["hash"],
                    "resolved_from": peer.label,
                    "scope": "global",
                    "status": {"kind": "Applied"},
                }
            )
        raise AssertionError((peer, method, path, body))

    monkeypatch.setattr(controller, "_direct_exchange", exchange)

    class Supervisor:
        peer = peers[-1]

        @staticmethod
        def restart(deadline: float) -> tuple[int, int, int]:
            assert deadline > 0
            return 101, 202, 25

    records = controller.run_verange_diagnostic_case(
        case="verange-controller-owned-diagnostic",
        candidate_binding_sha256=candidate,
        action_driver=tmp_path / "unused-by-mock",
        expected_action_driver_sha256=DRIVER_SHA256,
        work_directory=tmp_path,
        peers=peers,
        restarted_supervisor=Supervisor(),  # type: ignore[arg-type]
        primary_request=primary_request,
        successor_request=successor_request,
        timeout_seconds=5,
    )
    transcript = json.loads(records.transcript)
    result = json.loads(records.result)
    assert controller._canonical_json_bytes(transcript) == records.transcript
    assert controller._canonical_json_bytes(result) == records.result
    assert result["diagnostic_only"] is True
    assert result["operation_surface_complete"] is False
    assert result["action_driver_sha256"] == DRIVER_SHA256
    assert transcript["action_driver_sha256"] == DRIVER_SHA256
    assert result["sentinel_height"] == result["recovered_height"] == 11
    assert result["sentinel_hash"] == result["recovered_hash"] == "bb" * 32
    assert result["successor_height"] == 12
    assert len([row for row in transcript["events"] if row["kind"] == "direct-peer-http"]) == 48
    assert any(row["kind"] == "controller-owned-restart" for row in transcript["events"])
    native_rows = [row for row in transcript["events"] if row["kind"] == "native-action"]
    assert len(native_rows) == 2
    assert all("response_base64" not in row for row in native_rows)
    assert all("transaction_base64" in row for row in native_rows)
    assert all(row["action_driver_sha256"] == DRIVER_SHA256 for row in native_rows)


def urllib_parse(path: str) -> dict[str, str]:
    from urllib.parse import parse_qs, urlsplit

    values = parse_qs(urlsplit(path).query, strict_parsing=True)
    return {key: rows[0] for key, rows in values.items()}


def test_pipeline_status_cannot_lie_about_transaction_identity() -> None:
    peer = _peers()[0]
    response = _json_response(
        {
            "hash": "22" * 32,
            "resolved_from": peer.label,
            "scope": "global",
            "status": {"kind": "Applied"},
        }
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="not bound"):
        controller._pipeline_status_kind(response, "11" * 32, peer)


def test_replay_or_adversary_success_status_fails_closed() -> None:
    transcript = controller.TranscriptBuilder("case", "11" * 32, DRIVER_SHA256)
    peer = _peers()[0]
    with pytest.MonkeyPatch.context() as monkeypatch:
        monkeypatch.setattr(
            controller,
            "_direct_exchange",
            lambda *_args, **_kwargs: controller.HttpObservation(202, {}, b""),
        )
        with pytest.raises(controller.SealedPrivacyControllerError, match="accepted"):
            controller._submit(
                transcript, peer, b"proof-bearing", expected="rejected"
            )


def test_exact_restart_sentinel_rejects_a_higher_or_changed_common_sample(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    transcript = controller.TranscriptBuilder("case", "11" * 32, DRIVER_SHA256)
    monkeypatch.setattr(
        controller,
        "_fleet_sample",
        lambda *_args, **_kwargs: controller.FleetSample(12, "cc" * 32),
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="exact sentinel"):
        controller._wait_for_exact_sentinel(
            transcript,
            _peers(),
            controller.FleetSample(11, "bb" * 32),
            float("inf"),
        )


def test_controller_owned_pid_file_rejects_world_readable_or_symlink(
    tmp_path: Path,
) -> None:
    peer = _peers()[-1]

    class Process:
        @staticmethod
        def poll() -> None:
            return None

    pid_file = tmp_path / "child.pid"
    pid_file.write_text("123\n", encoding="ascii")
    pid_file.chmod(0o644)
    supervisor = controller.ControllerOwnedSupervisor(
        peer,
        Process(),  # type: ignore[arg-type]
        pid_file,
        pid_file.stat().st_uid,
        pid_file.stat().st_gid,
    )
    with pytest.raises(controller.SealedPrivacyControllerError, match="unsafe"):
        supervisor._child_pid()
    pid_file.chmod(0o600)
    link = tmp_path / "child-link.pid"
    link.symlink_to(pid_file)
    supervisor.child_pid_file = link
    with pytest.raises(controller.SealedPrivacyControllerError, match="unsafe"):
        supervisor._child_pid()


def test_source_boundary_has_no_driver_network_or_self_attestation_surface() -> None:
    driver = (
        ROOT / "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
    ).read_text(encoding="utf-8")
    sealed = (
        ROOT / "scripts/taira_privacy_sealed_controller.py"
    ).read_text(encoding="utf-8")
    capture = (
        ROOT / "scripts/capture_taira_privacy_protocol_four_peer_receipt.py"
    ).read_text(encoding="utf-8")
    sealer = (ROOT / "scripts/seal_taira_release_controllers.py").read_text(
        encoding="utf-8"
    )
    assert "reqwest" not in driver
    assert "ureq" not in driver
    request_fields = driver.split("struct BuildVeRangeRequestV1 {", 1)[1].split("}", 1)[0]
    assert "endpoint" not in request_fields
    assert "credential" not in request_fields
    assert "std::net" not in driver
    assert "test result: ok" not in driver
    assert 'transaction_norito_hex: String' in driver
    assert 'transaction_hash_hex: String' in driver
    assert '"/v1/pipeline/transactions"' in sealed
    assert '"/v1/sumeragi/status"' in sealed
    assert "signal.SIGUSR1" in sealed
    assert "require_complete_release_operation_surface" in sealed
    assert "privacy protocol v2 issuance is closed" in capture
    assert capture.index("require_complete_release_operation_surface()") < capture.index(
        "case_rows = ["
    )
    assert '"scripts/taira_privacy_sealed_controller.py"' in sealer
