from __future__ import annotations

import hashlib
import importlib.util
import inspect
import io
import os
import stat
import struct
import sys
from pathlib import Path

import pytest

MODULE_PATH = Path(__file__).parents[1] / "src/iroha_python/privacy_wallet_worker.py"
SPEC = importlib.util.spec_from_file_location("privacy_wallet_worker_source", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
worker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = worker
SPEC.loader.exec_module(worker)

AUTH_KEY = bytes(range(1, 33))
PROTOCOL = "iroha-jindo-polynomial-commitment-v1"
OPERATION = "jindo_polynomial_evaluation_v1"


class CapturePipe:
    def __init__(self) -> None:
        self.buffer = bytearray()
        self.closed = False

    def write(self, value: bytes | bytearray) -> int:
        if self.closed:
            raise ValueError("closed")
        self.buffer.extend(value)
        return len(value)

    def flush(self) -> None:
        if self.closed:
            raise ValueError("closed")

    def close(self) -> None:
        self.closed = True


class FakeProcess:
    def __init__(self, responses: bytes) -> None:
        self.stdin = CapturePipe()
        self.stdout = io.BytesIO(responses)
        self.killed = False
        self.terminated = False
        self.popen_args: tuple[object, ...] = ()
        self.popen_kwargs: dict[str, object] = {}

    def wait(self, timeout: int | None = None) -> int:
        del timeout
        return 0

    def kill(self) -> None:
        self.killed = True

    def terminate(self) -> None:
        self.terminated = True


def binding() -> worker.PrivacyWalletWitnessBindingV1:
    intent = b'{"public":"intent"}'
    return worker.PrivacyWalletWitnessBindingV1(
        network_id=b"\x11" * 32,
        signer_wallet_id="wallet-1",
        protocol_id=PROTOCOL,
        compiled_profile_digest=b"\x22" * 32,
        public_intent_digest=worker.privacy_wallet_public_intent_digest_v1(intent),
        nonce=b"\x33" * 32,
        signed_release_authority_digest=b"\x44" * 32,
    )


def put_text(value: str) -> bytes:
    encoded = value.encode("utf-8")
    return struct.pack(">H", len(encoded)) + encoded


def put_bytes_u16(value: bytes) -> bytes:
    return struct.pack(">H", len(value)) + value


def put_bytes_u32(value: bytes) -> bytes:
    return struct.pack(">I", len(value)) + value


def lease_payload(*, handle: bytes = b"\x51" * 32) -> bytes:
    return b"".join(
        (
            b"\x01",
            handle,
            struct.pack(">Q", 50_000),
            b"\x01",
            put_text("wallet-1"),
            put_text("alice@wonderland"),
            put_text("ed0120" + "a" * 64),
            put_text(PROTOCOL),
            put_text(OPERATION),
        )
    )


def signed_payload(*, network_id: bytes = b"\x11" * 32) -> bytes:
    adaptive = b"adaptive-signed-wire"
    versioned = b"versioned-signed-wire"
    return b"".join(
        (
            b"\x03",
            put_text(PROTOCOL),
            put_text(OPERATION),
            network_id,
            put_text("alice@wonderland"),
            put_text("ed0120" + "a" * 64),
            put_bytes_u32(adaptive),
            put_bytes_u32(versioned),
            put_bytes_u16(b"\x90" * 64),
            put_bytes_u16(b"\x91" * 32),
            b"\xa1" * 32,
            b"\xa2" * 32,
            b"\xa3" * 32,
            b"\xa4" * 32,
            struct.pack(">IIIII", 100, 200, 220, len(adaptive), len(versioned)),
        )
    )


def response_frame(
    command: worker.PrivacyWalletWorkerCommandV1,
    sequence: int,
    payload: bytes,
) -> bytes:
    return worker._encode_frame(command, sequence, payload, AUTH_KEY)


def executable(tmp_path: Path) -> tuple[Path, str]:
    path = tmp_path / "iroha_privacy_wallet_worker"
    path.write_bytes(b"not executed: Popen is replaced by the test")
    path.chmod(stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
    return path, hashlib.sha256(path.read_bytes()).hexdigest()


def controller(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    responses: bytes,
) -> tuple[worker.PrivacyWalletWorkerControllerV1, FakeProcess]:
    process = FakeProcess(responses)
    monkeypatch.setattr(worker.secrets, "token_bytes", lambda count: AUTH_KEY)

    def fake_popen(*args: object, **kwargs: object) -> FakeProcess:
        process.popen_args = args
        process.popen_kwargs = kwargs
        return process

    monkeypatch.setattr(worker.subprocess, "Popen", fake_popen)
    path, digest = executable(tmp_path)
    return (
        worker.PrivacyWalletWorkerControllerV1(path, expected_worker_sha256=digest),
        process,
    )


def written_frames(process: FakeProcess) -> list[tuple[object, int, bytes]]:
    assert bytes(process.stdin.buffer[:32]) == AUTH_KEY
    stream = io.BytesIO(bytes(process.stdin.buffer[32:]))
    frames = []
    while stream.tell() != len(stream.getbuffer()):
        frames.append(worker._read_frame(stream, AUTH_KEY))
    return frames


def test_generic11_worker_registry_is_closed_and_ordered() -> None:
    assert list(worker.PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1.items()) == [
        ("zk-ace-pq-authorization-v1", "zk_ace_authorization_action_v1"),
        ("anonymous-pgc-k-out-of-n-v1", "anonymous_pgc_payment_action_v1"),
        ("verange-transparent-range-v1", "verange_range_proof_v1"),
        ("iroha-zk-ams-v1", "zk_ams_admission_and_provisioning_v1"),
        ("vega-existing-credential-zk-v1", "vega_credential_presentation_v1"),
        ("iroha-jindo-polynomial-commitment-v1", "jindo_polynomial_evaluation_v1"),
        (
            "iroha-bootle-lantern-anoncred-v1",
            "bootle_lantern_credential_presentation_v1",
        ),
        ("orchard-halo2-actions-v1", "orchard_note_action_v1"),
        ("monero-fcmp-plus-plus-v1", "fcmp_membership_payment_v1"),
        ("iroha-ivm-private-note-stark-v1", "ivm_private_note_action_v1"),
        ("pq-masp-stark-v1", "pq_masp_note_action_v1"),
    ]


def test_public_intent_digest_matches_the_rust_domain_contract() -> None:
    assert worker.privacy_wallet_public_intent_digest_v1(b'{"public":"intent"}').hex() == (
        "c7a01b2b6b3b31554c83db74bfe48bfda91f284c555cbe490973cfab0a0b6dd8"
    )


@pytest.mark.parametrize(
    "change,exception",
    [
        ({"protocol_id": "sis-with-hints"}, ValueError),
        ({"protocol_id": "jindo-lattice-pcs-zk-v0"}, ValueError),
        ({"protocol_id": "iroha-zk-x509-stark-p256-v1"}, ValueError),
        ({"chain_id": "taira-testnet"}, TypeError),
        ({"genesis_digest": b"\x11" * 32}, TypeError),
        ({"network_id": b"\0" * 32}, ValueError),
        ({"network_id": b"\x10" * 32}, ValueError),
        ({"nonce": b"\0" * 32}, ValueError),
        ({"network_id": bytearray(b"\x11" * 32)}, TypeError),
    ],
)
def test_binding_rejects_retired_fields_aliases_zero_and_mutable_digests(
    change: dict[str, object], exception: type[Exception]
) -> None:
    values = {
        "network_id": b"\x11" * 32,
        "signer_wallet_id": "wallet-1",
        "protocol_id": PROTOCOL,
        "compiled_profile_digest": b"\x22" * 32,
        "public_intent_digest": b"\x23" * 32,
        "nonce": b"\x33" * 32,
        "signed_release_authority_digest": b"\x44" * 32,
    }
    values.update(change)
    with pytest.raises(exception):
        worker.PrivacyWalletWitnessBindingV1(**values)


def test_ping_uses_authenticated_sequence_one_and_zeroizes_on_close(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    client, process = controller(
        monkeypatch,
        tmp_path,
        response_frame(worker.PrivacyWalletWorkerCommandV1.PING, 1, b"\0"),
    )
    client.ping()
    assert written_frames(process) == [(worker.PrivacyWalletWorkerCommandV1.PING, 1, b"")]
    assert process.popen_kwargs["env"] == {}
    assert process.popen_kwargs["cwd"] == os.path.abspath(os.sep)
    assert process.popen_kwargs["close_fds"] is True
    assert process.popen_kwargs["start_new_session"] is True
    client.close()
    assert client.closed
    assert bytes(client._auth_key) == b"\0" * 32


def test_import_transports_only_absolute_path_and_public_binding(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    client, process = controller(
        monkeypatch,
        tmp_path,
        response_frame(worker.PrivacyWalletWorkerCommandV1.IMPORT, 1, lease_payload()),
    )
    credential_path = tmp_path / "owner-only.ipwb"
    witness_sentinel = b"must-never-cross-python-ipc-as-bytes"
    credential_path.write_bytes(witness_sentinel)
    credential_path.chmod(0o600)

    lease = client.import_credential(credential_path, binding(), ttl_millis=30_000)
    assert lease.protocol_id == PROTOCOL
    assert lease.operation_schema == OPERATION
    [(command, sequence, payload)] = written_frames(process)
    assert command == worker.PrivacyWalletWorkerCommandV1.IMPORT
    assert sequence == 1
    assert os.fspath(credential_path).encode() in payload
    assert b"\x11" * 32 in payload
    assert b"taira-testnet" not in payload
    assert witness_sentinel not in payload


def test_inspect_rejects_a_substituted_handle_and_terminates_session(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    client, process = controller(
        monkeypatch,
        tmp_path,
        response_frame(
            worker.PrivacyWalletWorkerCommandV1.INSPECT,
            1,
            lease_payload(handle=b"\x52" * 32),
        ),
    )
    with pytest.raises(worker.PrivacyWalletWorkerErrorV1, match="substituted"):
        client.inspect(worker.PrivacyWalletWitnessHandleV1(b"\x51" * 32), binding())
    assert client.closed
    assert process.killed


def test_remote_error_is_typed_and_consumes_exactly_one_sequence(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    remote_error = b"\xff" + struct.pack(">H", 20) + put_text("witness handle is unknown")
    responses = b"".join(
        (
            response_frame(worker.PrivacyWalletWorkerCommandV1.INSPECT, 1, remote_error),
            response_frame(worker.PrivacyWalletWorkerCommandV1.PING, 2, b"\0"),
        )
    )
    client, process = controller(monkeypatch, tmp_path, responses)
    with pytest.raises(worker.PrivacyWalletWorkerRemoteErrorV1) as raised:
        client.inspect(worker.PrivacyWalletWitnessHandleV1(b"\x51" * 32), binding())
    assert raised.value.code == worker.PrivacyWalletWorkerErrorCodeV1.UNKNOWN_HANDLE
    assert not client.closed
    client.ping()
    assert [frame[1] for frame in written_frames(process)] == [1, 2]


def test_execute_api_has_no_witness_or_bundle_byte_parameter_and_returns_typed_public_wire(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    parameters = inspect.signature(worker.PrivacyWalletWorkerControllerV1.execute).parameters
    assert "witness" not in parameters
    assert "execution_bundle" not in parameters
    assert set(parameters) == {
        "self",
        "handle",
        "binding",
        "canonical_public_intent",
        "canonical_execution_plan",
    }
    client, process = controller(
        monkeypatch,
        tmp_path,
        response_frame(worker.PrivacyWalletWorkerCommandV1.EXECUTE, 1, signed_payload()),
    )
    result = client.execute(
        worker.PrivacyWalletWitnessHandleV1(b"\x51" * 32),
        binding(),
        canonical_public_intent=b'{"public":"intent"}',
        canonical_execution_plan=b'{"public":"plan"}',
    )
    assert isinstance(result, worker.PrivacyWalletSignedActionV1)
    assert result.protocol_id == PROTOCOL
    assert result.network_id == b"\x11" * 32
    assert not hasattr(result, "chain_id")
    assert result.adaptive_signed_transaction == b"adaptive-signed-wire"
    assert result.versioned_signed_transaction == b"versioned-signed-wire"
    [(command, sequence, payload)] = written_frames(process)
    assert (command, sequence) == (worker.PrivacyWalletWorkerCommandV1.EXECUTE, 1)
    assert b'{"public":"intent"}' in payload
    assert b'{"public":"plan"}' in payload


def test_same_display_label_cannot_substitute_a_different_network_id(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    first_display_label = "same-privacy-network-label"
    second_display_label = "same-privacy-network-label"
    assert first_display_label == second_display_label
    client, process = controller(
        monkeypatch,
        tmp_path,
        response_frame(
            worker.PrivacyWalletWorkerCommandV1.EXECUTE,
            1,
            signed_payload(network_id=b"\x12" * 32),
        ),
    )
    with pytest.raises(worker.PrivacyWalletWorkerErrorV1, match="identity"):
        client.execute(
            worker.PrivacyWalletWitnessHandleV1(b"\x51" * 32),
            binding(),
            canonical_public_intent=b'{"public":"intent"}',
            canonical_execution_plan=b'{"public":"plan"}',
        )
    assert client.closed
    assert process.killed


@pytest.mark.parametrize("mutation", ["tag", "sequence", "command", "suffix", "truncate"])
def test_tampered_noncanonical_or_mismatched_responses_fail_closed(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, mutation: str
) -> None:
    frame = bytearray(
        response_frame(worker.PrivacyWalletWorkerCommandV1.PING, 1, b"\0")
    )
    if mutation == "tag":
        frame[-1] ^= 1
    elif mutation == "sequence":
        frame = bytearray(response_frame(worker.PrivacyWalletWorkerCommandV1.PING, 2, b"\0"))
    elif mutation == "command":
        frame = bytearray(response_frame(worker.PrivacyWalletWorkerCommandV1.CANCEL, 1, b"\0"))
    elif mutation == "suffix":
        frame.extend(b"x")
        frame[0:4] = struct.pack(">I", len(frame) - 4)
    else:
        del frame[-1]
    client, process = controller(monkeypatch, tmp_path, bytes(frame))
    with pytest.raises(worker.PrivacyWalletWorkerErrorV1):
        client.ping()
    assert client.closed
    assert process.killed
    assert bytes(client._auth_key) == b"\0" * 32


def test_worker_path_rejects_relative_symlink_writable_and_non_executable_files(
    tmp_path: Path,
) -> None:
    regular = tmp_path / "worker"
    regular.write_bytes(b"worker")
    regular.chmod(0o700)
    digest = hashlib.sha256(regular.read_bytes()).hexdigest()
    with pytest.raises(ValueError, match="absolute"):
        worker._require_worker_executable(Path("worker"), digest)
    with pytest.raises(ValueError, match="non-zero SHA-256"):
        worker._require_worker_executable(regular, "0" * 64)
    with pytest.raises(ValueError, match="admitted SHA-256"):
        worker._require_worker_executable(regular, "f" * 64)
    link = tmp_path / "worker-link"
    link.symlink_to(regular)
    with pytest.raises(ValueError, match="non-symlink"):
        worker._require_worker_executable(link, digest)
    regular.chmod(0o722)
    with pytest.raises(ValueError, match="writable"):
        worker._require_worker_executable(regular, digest)
    regular.chmod(0o600)
    with pytest.raises(ValueError, match="not executable"):
        worker._require_worker_executable(regular, digest)


def test_import_requires_path_not_bytes_and_execute_requires_immutable_public_bytes(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    responses = response_frame(worker.PrivacyWalletWorkerCommandV1.IMPORT, 1, lease_payload())
    client, _ = controller(monkeypatch, tmp_path, responses)
    with pytest.raises((TypeError, ValueError)):
        client.import_credential(b"owner bundle bytes", binding(), ttl_millis=30_000)
    with pytest.raises(TypeError):
        client.execute(
            worker.PrivacyWalletWitnessHandleV1(b"\x51" * 32),
            binding(),
            canonical_public_intent=bytearray(b"{}"),
            canonical_execution_plan=b"{}",
        )
