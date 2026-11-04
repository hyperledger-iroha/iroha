"""Standalone Ironwood controller contracts; runnable without a native extension or pytest."""
from __future__ import annotations

import hashlib
import importlib.util
import io
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

MODULE_PATH = Path(__file__).parents[1] / "src/iroha_python/privacy_wallet_worker.py"
SPEC = importlib.util.spec_from_file_location("ironwood_worker_source", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
worker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = worker
SPEC.loader.exec_module(worker)
AUTH_KEY = bytes(range(1, 33))


class _Pipe(io.BytesIO):
    def close(self) -> None:
        pass


class _Process:
    def __init__(self, response: bytes) -> None:
        self.stdin = _Pipe()
        self.stdout = io.BytesIO(response)
        self.terminated = False
        self.killed = False

    def wait(self, timeout: int | None = None) -> int:
        return 0

    def terminate(self) -> None:
        self.terminated = True

    def kill(self) -> None:
        self.killed = True


def _text(value: str) -> bytes:
    value = value.encode()
    return struct.pack(">H", len(value)) + value


def _bytes(value: bytes) -> bytes:
    return struct.pack(">I", len(value)) + value


class IronwoodControllerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def controller(self, command: object, public_response: bytes):
        response = worker._encode_frame(command, 1, b"\x06" + _bytes(public_response), AUTH_KEY)
        process = _Process(response)
        executable = self.root / "worker"
        executable.write_bytes(b"native process is substituted only in controller framing tests")
        executable.chmod(0o700)
        with patch.object(worker.secrets, "token_bytes", return_value=AUTH_KEY), patch.object(worker.subprocess, "Popen", return_value=process):
            client = worker.PrivacyWalletWorkerControllerV1(executable, expected_worker_sha256=hashlib.sha256(executable.read_bytes()).hexdigest())
        return client, process

    def frame(self, process: _Process):
        written = process.stdin.getvalue()
        self.assertEqual(written[:32], AUTH_KEY)
        stream = io.BytesIO(written[32:])
        result = worker._read_frame(stream, AUTH_KEY)
        self.assertEqual(stream.tell(), len(stream.getvalue()))
        return result

    def test_separate_capability_preserves_original_inventory_and_ordinals(self) -> None:
        self.assertEqual(len(worker.PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1), 11)
        self.assertNotIn("ironwood-halo2-actions-v1", worker.PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1)
        self.assertEqual(worker.PRIVACY_WORKER_OPERATION_SCHEMAS_V1, {**worker.PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1, "ironwood-halo2-actions-v1": "ironwood_note_action_v1"})
        self.assertEqual([int(command) for command in worker.PrivacyWalletWorkerCommandV1], list(range(1, 16)))
        self.assertEqual(worker.PrivacyWalletWorkerCommandV1.DERIVE_IRONWOOD_ADDRESS, 10)
        self.assertEqual(worker.PrivacyWalletWorkerCommandV1.SCAN_IRONWOOD_RECEIPTS, 11)

    def test_native_deposit_import_transports_only_payer_path_and_approved_public_action(self) -> None:
        command = worker.PrivacyWalletWorkerCommandV1.IMPORT_IRONWOOD_DEPOSIT
        bound = worker.PrivacyWalletWitnessBindingV1(network_id=b"\x11"*32, signer_wallet_id="payer", protocol_id="ironwood-halo2-actions-v1", compiled_profile_digest=b"\x22"*32, public_intent_digest=b"\x33"*32, nonce=b"\x44"*32, signed_release_authority_digest=b"\x55"*32)
        client, process = self.controller(command, b"")
        # Frame transport is inert here; native lease and signing are covered by Rust's real round trip.
        action = b'{"changes":[],"outputs":[]}'
        with patch.object(worker.PrivacyWalletWorkerControllerV1, "_decode_lease", return_value="lease") as decode, patch.object(worker.PrivacyWalletWorkerControllerV1, "_exchange", return_value=b"native lease") as exchange:
            self.assertEqual(client.import_ironwood_deposit(self.root / "payer.seed", bound, action,
                ttl_millis=30_000, chain_discriminant=369), "lease")
            opcode, payload = exchange.call_args.args
            self.assertEqual(opcode, command)
            self.assertEqual(payload, _text(str(self.root / "payer.seed")) + worker._encode_binding(bound)
                + struct.pack(">QH", 30_000, 369) + _bytes(action))
            decode.assert_called_once_with(b"native lease", bound)
            with self.assertRaises(ValueError):
                client.import_ironwood_deposit(self.root / "payer.seed", bound, action,
                    ttl_millis=0, chain_discriminant=369)
            with self.assertRaises(TypeError):
                client.import_ironwood_deposit(self.root / "payer.seed", bound, action, ttl_millis=30_000)
            for prefix in (True, False, None, -1, 65536, "369", 369.0):
                with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                    client.import_ironwood_deposit(self.root / "payer.seed", bound, action,
                        ttl_millis=30_000, chain_discriminant=prefix)
            self.assertEqual(exchange.call_count, 1)
            for prefix in (0, 65535):
                with self.subTest(prefix=prefix):
                    client.import_ironwood_deposit(self.root / "payer.seed", bound, action,
                        ttl_millis=30_000, chain_discriminant=prefix)
                    self.assertEqual(exchange.call_args.args[1], _text(str(self.root / "payer.seed"))
                        + worker._encode_binding(bound) + struct.pack(">QH", 30_000, prefix) + _bytes(action))

    def test_recipient_export_sends_only_key_path_and_public_arguments(self) -> None:
        public = b"\x63" * 43 + b"\x21" * 32
        command = worker.PrivacyWalletWorkerCommandV1.DERIVE_IRONWOOD_ADDRESS
        client, process = self.controller(command, public)
        path = self.root / "merchant.key"
        recipient = client.derive_ironwood_address(path, diversifier_index=123, internal_scope=True)
        self.assertEqual(recipient.address, public[:43])
        self.assertEqual(recipient.compiled_profile_digest, public[43:])
        self.assertEqual(self.frame(process), (command, 1, _text(str(path)) + struct.pack(">IB", 123, 1)))

    def test_receipt_request_is_forwarded_without_interpretation_or_secrets(self) -> None:
        request = b'{"public":"independent-checkpoint-and-exact-receipt"}'
        result = b'{"checkpoint_hex":"authenticated","receipts":[]}'
        command = worker.PrivacyWalletWorkerCommandV1.SCAN_IRONWOOD_RECEIPTS
        client, process = self.controller(command, result)
        path = self.root / "merchant.key"
        self.assertEqual(client.scan_ironwood_receipts(path, request), result)
        self.assertEqual(self.frame(process), (command, 1, _text(str(path)) + _bytes(request)))

    def test_deposit_binds_public_recipient_exact_amount_and_memo(self) -> None:
        recipient = worker.IronwoodRecipientV1(b"\x63" * 43, b"\x21" * 32)
        arguments = dict(asset_definition_id="fixture-asset", pool_id=b"\x61" * 32, anchor=b"\x65" * 32, anchor_epoch=1, expiry_height=99, recipient=recipient, amount=17, memo=b"m" * 512, expected_profile_digest=recipient.compiled_profile_digest)
        action = worker.ironwood_deposit_public_action_v1(**arguments)
        self.assertEqual(action["public_debit_decimal"], "17")
        self.assertEqual(action["public_balance_scope"], "global")
        self.assertEqual(action["outputs"], [{"recipient_hex": recipient.address.hex(), "value_decimal": "17", "memo_hex": (b"m" * 512).hex()}])
        self.assertEqual(action["changes"], [])
        self.assertNotIn("spending_key", repr(action))
        for field, wrong in [("amount", 0), ("amount", True), ("amount", 2**63), ("memo", b"short"), ("anchor_epoch", 0), ("expected_profile_digest", b"\x22" * 32)]:
            with self.subTest(field=field, value=wrong), self.assertRaises((ValueError, TypeError)):
                worker.ironwood_deposit_public_action_v1(**{**arguments, field: wrong})

    def test_viewing_key_export_and_scan_never_send_key_bytes(self) -> None:
        command = worker.PrivacyWalletWorkerCommandV1.EXPORT_IRONWOOD_VIEWING_KEY
        profile = b"\x21" * 32
        client, process = self.controller(command, profile)
        source, destination = self.root / "spending.key", self.root / "incoming.ivk"
        self.assertEqual(client.export_ironwood_viewing_key(source, destination, internal_scope=True), profile)
        self.assertEqual(self.frame(process), (command, 1, _text(str(source)) + _text(str(destination)) + b"\x01"))
        command = worker.PrivacyWalletWorkerCommandV1.SCAN_IRONWOOD_RECEIPTS_WITH_VIEWING_KEY
        response = b'{"checkpoint_hex":"authenticated","receipts":[]}'
        client, process = self.controller(command, response)
        request = b'{"request":"public"}'
        self.assertEqual(client.scan_ironwood_receipts_with_viewing_key(destination, request), response)
        self.assertEqual(self.frame(process), (command, 1, _text(str(destination)) + _bytes(request)))

    def test_signed_ironwood_mapping_uses_new_tag_and_preserves_output_order(self) -> None:
        protocol = "ironwood-halo2-actions-v1"
        binding = worker.PrivacyWalletWitnessBindingV1(
            network_id=b"\x01" * 32, signer_wallet_id="payer", protocol_id=protocol,
            compiled_profile_digest=b"\x21" * 32, public_intent_digest=b"\x31" * 32,
            nonce=b"\x41" * 32, signed_release_authority_digest=b"\x51" * 32)
        base = (b"\x07" + _text(protocol) + _text("ironwood_note_action_v1") + binding.network_id
                + _text("authority") + _text("public-key") + _bytes(b"adaptive") + _bytes(b"versioned")
                + struct.pack(">H", 64) + b"s" * 64 + struct.pack(">H", 32) + b"p" * 32
                + b"d" * 128 + struct.pack(">IIIII", 1, 2, 3, 8, 9))
        client, _ = self.controller(worker.PrivacyWalletWorkerCommandV1.EXECUTE, b"unused")
        result = client._decode_signed_action(base + struct.pack(">IBBB", 2, 2, 1, 0), binding)
        self.assertEqual(result.ironwood_direct_instruction_index, 2)
        self.assertEqual(result.ironwood_output_action_indices, (1, 0))
        for suffix in [struct.pack(">IBBB", 2, 2, 1, 1), struct.pack(">IBB", 2, 1, 2)]:
            client, _ = self.controller(worker.PrivacyWalletWorkerCommandV1.EXECUTE, b"unused")
            with self.assertRaises(worker.PrivacyWalletWorkerErrorV1):
                client._decode_signed_action(base + suffix, binding)
        client, _ = self.controller(worker.PrivacyWalletWorkerCommandV1.EXECUTE, b"unused")
        with self.assertRaises(worker.PrivacyWalletWorkerErrorV1):
            client._decode_signed_action(b"\x03" + base[1:], binding)

    def test_bad_recipient_response_fails_closed(self) -> None:
        command = worker.PrivacyWalletWorkerCommandV1.DERIVE_IRONWOOD_ADDRESS
        client, process = self.controller(command, b"\x01" * 74)
        with self.assertRaises(worker.PrivacyWalletWorkerErrorV1):
            client.derive_ironwood_address(self.root / "merchant.key")
        self.assertTrue(process.terminated or process.killed)


if __name__ == "__main__":
    unittest.main()
