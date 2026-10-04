"""Isolated transport controls; these do not qualify native signatures or finality."""

from __future__ import annotations

import importlib.util
import json
import sys
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

_SOURCE = Path(__file__).resolve().parents[1] / "src/iroha_python/private_transaction_counters.py"
_SPEC = importlib.util.spec_from_file_location("private_counter_source_controls", _SOURCE)
assert _SPEC is not None and _SPEC.loader is not None
counter = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = counter
_SPEC.loader.exec_module(counter)


class _Response:
    def __init__(self, root="https://peer1.example", chunks=(b"original response",)):
        self.url = root + "/v1/private/transaction-counters"
        self.history = []
        self.status_code = 200
        self.headers = {"Content-Type": "application/x-norito"}
        self.chunks = chunks
        self.polled = False
        self.closed = False

    @property
    def content(self):
        raise AssertionError("unbounded response content is prohibited")

    def iter_content(self, chunk_size):
        assert chunk_size == 8192
        self.polled = True
        yield from self.chunks

    def close(self):
        self.closed = True


class _Client(counter.ToriiClientPrivateTransactionCountersMixin):
    def __init__(self, root="https://peer1.example", response=None):
        self._base_url = root
        self._api_token = "selected private token"
        self._timeout = 30.0
        self.response = response or _Response(root)
        self.calls = []

    def _request(self, *args, **kwargs):
        self.calls.append((args, kwargs))
        return self.response


class _Native:
    def __init__(self):
        self.originals = None
        self.verify_inputs = None
        self.build_inputs = None
        self.commitment_inputs = None
        self.genesis_input = None

    def canonical_private_genesis_authority_v1(self, original):
        self.genesis_input = original
        return "native canonical authority"

    def commitments_private_transaction_counters_v1(self, *inputs):
        self.commitment_inputs = inputs
        return "native policy hash", "native manifest hash"

    def build_private_transaction_counters_request_v1(self, *inputs):
        self.build_inputs = inputs
        return b"signed original"

    def collect_private_transaction_counters_v1(self, originals):
        self.originals = originals
        return b"original native certificate"

    def verify_private_transaction_counters_v1(self, *inputs):
        self.verify_inputs = inputs
        groups = [{
            "key": {"party": "None", "category": "Transfer", "role": "Business",
                    "result": "Applied", "rejection": "None"}, "count": 7,
        }]
        return json.dumps({"claim": {"authority": "private evidence", "groups": groups},
                           "groups": groups}), b"promoted native checkpoint"


def _prepared():
    return counter.PreparedPrivateTransactionCountersV1(
        b"signed original", b"selected policy", "[original prefix]",
        "{independent expected context}", "selected chain", b"selected checkpoint",
    )


class PrivateCounterSourceControls(unittest.TestCase):
    def test_private_genesis_authority_preserves_authenticated_original_for_native_admission(self):
        native = _Native()
        original = b"independently authenticated original private genesis"
        with patch.object(counter, "_native", return_value=native):
            authority = counter.canonical_private_genesis_authority_v1(original)
        self.assertIs(native.genesis_input, original)
        self.assertEqual(authority, "native canonical authority")

    def test_private_genesis_authority_rejects_nonoriginal_or_oversized_input_before_dispatch(self):
        for original, error in ((bytearray(b"original"), TypeError), (b"", ValueError),
                                (b"x" * (64 * 1024 * 1024 + 1), ValueError)):
            with patch.object(counter, "_native") as native, self.assertRaises(error):
                counter.canonical_private_genesis_authority_v1(original)
            native.assert_not_called()

    def test_private_genesis_native_refusal_cannot_publish_authority(self):
        native = _Native()
        native.canonical_private_genesis_authority_v1 = lambda *_: (
            _ for _ in ()
        ).throw(ValueError("native signature or private scope refusal"))
        with patch.object(counter, "_native", return_value=native), self.assertRaises(ValueError):
            counter.canonical_private_genesis_authority_v1(b"offered foreign root")

    def test_original_commitments_preserve_frames_and_independently_selected_run(self):
        native = _Native()
        originals = (b"selected policy original", b"selected manifest original", "{selected session binding}")
        with patch.object(counter, "_native", return_value=native):
            result = counter.commitments_private_transaction_counters_v1(*originals)
        self.assertEqual(native.commitment_inputs, originals)
        self.assertEqual(result, ("native policy hash", "native manifest hash"))

    def test_original_commitments_require_explicit_selected_run(self):
        with patch.object(counter, "_native") as native, self.assertRaises(TypeError):
            counter.commitments_private_transaction_counters_v1(b"policy", b"manifest")
        native.assert_not_called()

    def test_original_commitments_bound_all_inputs_before_native_dispatch(self):
        for inputs, error in (
            ((bytearray(b"policy"), b"manifest", "{}"), TypeError),
            ((b"", b"manifest", "{}"), ValueError),
            ((b"p" * (65536 + 1), b"manifest", "{}"), ValueError),
            ((b"policy", b"m" * (1048576 + 1), "{}"), ValueError),
            ((b"policy", b"manifest", b"{}"), TypeError),
            ((b"policy", b"manifest", "x" * (65536 + 1)), ValueError),
        ):
            with self.subTest(error=error, input_types=tuple(type(x).__name__ for x in inputs)):
                with patch.object(counter, "_native") as native, self.assertRaises(error):
                    counter.commitments_private_transaction_counters_v1(*inputs)
                native.assert_not_called()

    def test_original_commitments_native_run_refusal_has_no_result(self):
        native = _Native()
        native.commitments_private_transaction_counters_v1 = lambda *_: (
            _ for _ in ()
        ).throw(ValueError("native current run refusal"))
        with patch.object(counter, "_native", return_value=native), self.assertRaises(ValueError):
            counter.commitments_private_transaction_counters_v1(b"policy", b"manifest", "{selected session binding}")

    def test_single_dispatch_preserves_originals_and_independent_inputs(self):
        native = _Native()
        client = _Client()
        peer = _Client("https://peer2.example", _Response("https://peer2.example", (b"peer2 original",)))
        with patch.object(counter, "_native", return_value=native):
            result = client.get_verified_private_transaction_counters_v1(request=_prepared(), other_peers=(peer,))
        self.assertEqual(native.originals, [b"original response", b"peer2 original"])
        self.assertEqual(native.verify_inputs, (
            b"signed original", b"original native certificate", b"selected policy",
            "[original prefix]", "{independent expected context}", "selected chain", b"selected checkpoint",
        ))
        for selected in (client, peer):
            self.assertEqual(len(selected.calls), 1)
            args, options = selected.calls[0]
            self.assertEqual(args, ("POST", "/v1/private/transaction-counters"))
            self.assertEqual(options["data"], b"signed original")
            self.assertFalse(options["allow_retry"])
            self.assertFalse(options["allow_redirects"])
            self.assertTrue(options["stream"])
            self.assertTrue(selected.response.closed)
        self.assertEqual(result.groups[0]["count"], 7)
        self.assertEqual(result.promoted_checkpoint, b"promoted native checkpoint")
        self.assertNotIn("private evidence", repr(result))
        with self.assertRaises(TypeError):
            result.groups[0]["count"] = 8
        with self.assertRaises(TypeError):
            result.groups[0]["key"]["category"] = "Mint"

    def test_all_peer_tokens_are_validated_before_any_dispatch(self):
        client, peer = _Client(), _Client("https://peer2.example")
        peer._api_token = None
        with self.assertRaises(ValueError):
            client.get_verified_private_transaction_counters_v1(request=_prepared(), other_peers=(peer,))
        self.assertEqual(client.calls, [])

    def test_peer_dispatch_is_concurrent_and_retains_original_peer_order(self):
        native = _Native()
        client = _Client()
        peer = _Client("https://peer2.example", _Response("https://peer2.example", (b"peer2 original",)))
        barrier = threading.Barrier(2, timeout=3)
        for selected in (client, peer):
            original_request = selected._request
            def concurrent_request(*args, _request=original_request, **kwargs):
                barrier.wait()
                return _request(*args, **kwargs)
            selected._request = concurrent_request
        with patch.object(counter, "_native", return_value=native):
            client.get_verified_private_transaction_counters_v1(request=_prepared(), other_peers=(peer,))
        self.assertEqual(native.originals, [b"original response", b"peer2 original"])
        self.assertEqual(len(client.calls), 1)
        self.assertEqual(len(peer.calls), 1)

    def test_unbounded_peer_timeout_refuses_before_dispatch(self):
        client = _Client()
        client._timeout = float("inf")
        with self.assertRaises(ValueError):
            client.get_verified_private_transaction_counters_v1(request=_prepared())
        self.assertEqual(client.calls, [])

    def test_duplicate_selected_root_refuses_before_dispatch(self):
        client = _Client()
        with self.assertRaises(ValueError):
            client.get_verified_private_transaction_counters_v1(request=_prepared(), other_peers=(client,))
        self.assertEqual(client.calls, [])

    def test_oversized_declared_response_is_never_polled(self):
        response = _Response()
        response.headers["Content-Length"] = "65537"
        with self.assertRaises(ValueError):
            counter._read_original(_Client(response=response), b"original")
        self.assertFalse(response.polled)
        self.assertTrue(response.closed)

    def test_stream_bound_closes_before_collecting(self):
        response = _Response(chunks=(b"x" * 32768, b"y" * 32769))
        with self.assertRaises(ValueError):
            counter._read_original(_Client(response=response), b"original")
        self.assertTrue(response.closed)

    def test_length_mismatch_refuses_original(self):
        response = _Response()
        response.headers["Content-Length"] = "1"
        with self.assertRaises(ValueError):
            counter._read_original(_Client(response=response), b"original")
        self.assertTrue(response.closed)

    def test_noncanonical_length_is_refused_without_body(self):
        response = _Response()
        response.headers["Content-Length"] = "0001"
        with self.assertRaises(ValueError):
            counter._read_original(_Client(response=response), b"original")
        self.assertFalse(response.polled)

    def test_redirected_route_is_refused_without_body(self):
        response = _Response()
        response.history = [object()]
        with self.assertRaises(ValueError):
            counter._read_original(_Client(response=response), b"original")
        self.assertFalse(response.polled)
        self.assertTrue(response.closed)

    def test_wrong_mime_and_encoding_are_refused_without_body(self):
        for header, value in (("Content-Type", "application/json"), ("Content-Encoding", "gzip")):
            response = _Response()
            response.headers[header] = value
            with self.assertRaises(ValueError):
                counter._read_original(_Client(response=response), b"original")
            self.assertFalse(response.polled)
            self.assertTrue(response.closed)

    def test_http_refusal_does_not_read_private_error_body_or_retry(self):
        response = _Response()
        response.status_code = 409
        client = _Client(response=response)
        with self.assertRaises(counter.requests.HTTPError):
            counter._read_original(client, b"original")
        self.assertEqual(len(client.calls), 1)
        self.assertFalse(response.polled)
        self.assertTrue(response.closed)

    def test_verifier_refusal_exposes_no_projection_or_checkpoint(self):
        native = _Native()
        native.verify_private_transaction_counters_v1 = lambda *_: (_ for _ in ()).throw(ValueError("native refusal"))
        client = _Client()
        with patch.object(counter, "_native", return_value=native), self.assertRaises(ValueError):
            client.get_verified_private_transaction_counters_v1(request=_prepared())
        self.assertTrue(client.response.closed)

    def test_native_owner_is_required_before_any_dispatch(self):
        client = _Client()
        with patch.object(counter, "_native", return_value=object()), self.assertRaises(AttributeError):
            client.get_verified_private_transaction_counters_v1(request=_prepared())
        self.assertEqual(client.calls, [])

    def test_preparation_delegates_original_context_to_native_builder(self):
        native = _Native()
        with patch.object(counter, "_native", return_value=native):
            request = counter.prepare_private_transaction_counters_v1(
                time_to_live_ms=10000, private_key=b"k" * 32,
                original_policy=b"policy", native_finality_proof_chain_json="[prefix]",
                expected_json="{independent context}", expected_chain="chain", trusted_checkpoint=b"checkpoint",
            )
        self.assertEqual(request.original_request, b"signed original")
        self.assertEqual(native.build_inputs, (b"k" * 32, 10000, b"policy",
            "[prefix]", "{independent context}", "chain", b"checkpoint"))
        self.assertNotIn("signed original", repr(request))

    def test_preparation_rejects_nonimmutable_keys_before_native_dispatch(self):
        with patch.object(counter, "_native") as native, self.assertRaises(TypeError):
            counter.prepare_private_transaction_counters_v1(
                time_to_live_ms=10000, private_key=bytearray(b"k" * 32), original_policy=b"policy",
                native_finality_proof_chain_json="[]", expected_json="{}", expected_chain="chain",
                trusted_checkpoint=b"checkpoint",
            )
        native.assert_not_called()


if __name__ == "__main__":
    unittest.main()
