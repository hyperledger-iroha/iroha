"""HTTP pipeline status absence contracts shared by SDK and CLI mock consumers."""

from __future__ import annotations

import importlib.util
import json
import sys
import unittest
from http.client import HTTPConnection
from pathlib import Path
from urllib.parse import urlencode, urlparse

# The standalone mock has no native dependencies; load it without importing the
# SDK package and its unrelated native-backed identity implementation.
MODULE_PATH = Path(__file__).resolve().parents[1] / "mock.py"
MODULE_SPEC = importlib.util.spec_from_file_location(
    "iroha_torii_client_mock_pipeline_test_module", MODULE_PATH
)
assert MODULE_SPEC is not None and MODULE_SPEC.loader is not None
MOCK = importlib.util.module_from_spec(MODULE_SPEC)
sys.modules[MODULE_SPEC.name] = MOCK
MODULE_SPEC.loader.exec_module(MOCK)

STATUS_PATH = "/v1/pipeline/transactions/status"
HASHES = ("11" * 32, "33" * 32)


def _status_path(hash_value: str, scope: str | None) -> str:
    params = {"hash": hash_value}
    if scope is not None:
        params["scope"] = scope
    return f"{STATUS_PATH}?{urlencode(params)}"


def _assert_absence(response, hash_value: str, scope: str | None) -> None:
    body = response.read()
    assert response.status == 404
    assert response.headers.get_all("Content-Type") == ["application/json"]
    assert len(body) <= 4096
    envelope = json.loads(body)
    assert envelope["code"] == "pipeline_transaction_status_not_found"
    assert isinstance(envelope["message"], str) and envelope["message"]
    assert envelope["details"] == {
        "pipeline_transaction_status_not_found": {
            "hash": hash_value,
            "scope": scope or "global",
        }
    }


class MockPipelineTests(unittest.TestCase):
    def setUp(self) -> None:
        server = MOCK.ToriiMockServer().start()
        self.addCleanup(server.stop)
        parsed = urlparse(server.base_url)
        self.connection = HTTPConnection(parsed.hostname, parsed.port, timeout=5)
        self.addCleanup(self.connection.close)

    def test_missing_status_returns_exact_scoped_json_absence(self) -> None:
        for hash_value in HASHES:
            for scope in [None, "global", "local"]:
                with self.subTest(hash=hash_value, scope=scope):
                    self.connection.request("GET", _status_path(hash_value, scope))
                    _assert_absence(self.connection.getresponse(), hash_value, scope)

    def test_exhausted_status_sequence_returns_exact_scoped_json_absence(self) -> None:
        hash_value = HASHES[0]
        for scope in [None, "global", "local"]:
            with self.subTest(scope=scope):
                self.connection.request(
                    "POST",
                    "/__mock__/pipeline/config",
                    body=json.dumps({
                        "hash": hash_value,
                        "repeat_last": False,
                        "statuses": [{"kind": "Queued", "scope": scope or "global"}],
                    }),
                    headers={"Content-Type": "application/json"},
                )
                configured = self.connection.getresponse()
                configured.read()
                self.assertEqual(configured.status, 200)

                # The configured result is returned once and retained for one final poll.
                for _ in range(2):
                    self.connection.request("GET", _status_path(hash_value, scope))
                    response = self.connection.getresponse()
                    self.assertEqual(response.status, 200)
                    self.assertEqual(json.loads(response.read())["status"]["kind"], "Queued")

                self.connection.request("GET", _status_path(hash_value, scope))
                _assert_absence(self.connection.getresponse(), hash_value, scope)

    def test_invalid_status_selector_is_rejected(self) -> None:
        for params in [{}, {"hash": HASHES[0], "scope": "invalid"}]:
            with self.subTest(params=params):
                self.connection.request("GET", f"{STATUS_PATH}?{urlencode(params)}")
                response = self.connection.getresponse()
                response.read()
                self.assertEqual(response.status, 400)


if __name__ == "__main__":
    unittest.main()
