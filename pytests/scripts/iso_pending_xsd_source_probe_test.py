import contextlib
import importlib.util
import io
import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_pending_xsd_source_probe.py"
SPEC = importlib.util.spec_from_file_location("iso_pending_xsd_source_probe", SCRIPT_PATH)
PROBE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = PROBE
SPEC.loader.exec_module(PROBE)


class FakeHeaders(dict):
    def get_content_type(self):
        return self.get("content-type")


class FakeResponse:
    def __init__(self, data, *, status=206, content_type="application/xml"):
        self.status = status
        self.headers = FakeHeaders({"content-type": content_type})
        self._data = data

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return False

    def read(self, _size):
        return self._data


def run_probe(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = PROBE.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoPendingXsdSourceProbeTest(unittest.TestCase):
    def test_build_summary_records_reachable_xsd_probes_in_canonical_order(self):
        calls = []

        def opener(request, *, timeout):
            calls.append((request.full_url, timeout, request.headers.get("Range")))
            return FakeResponse(b"<?xml version='1.0'?><xs:schema/>")

        summary = PROBE.build_summary(
            message_def_ids=["sese.025.001.11", "colr.012.001.05"],
            timeout_secs=1.5,
            max_bytes=128,
            opener=opener,
        )

        self.assertTrue(summary["ok"])
        self.assertEqual(summary["probe_count"], 2)
        self.assertEqual(summary["successful_probe_count"], 2)
        self.assertEqual(
            [probe["message_def_id"] for probe in summary["probes"]],
            ["colr.012.001.05", "sese.025.001.11"],
        )
        self.assertTrue(all(probe["looks_like_xsd"] for probe in summary["probes"]))
        self.assertTrue(all(probe["status"] == "reachable" for probe in summary["probes"]))
        self.assertEqual([call[1] for call in calls], [1.5, 1.5])
        self.assertEqual([call[2] for call in calls], ["bytes=0-127", "bytes=0-127"])
        body = dict(summary)
        digest = body.pop(PROBE.SUMMARY_DIGEST_FIELD)
        self.assertEqual(digest, PROBE.sha256_hex(PROBE._canonical_json_bytes(body)))

    def test_build_summary_records_timeout_without_echoing_error_details(self):
        def opener(_request, *, timeout):
            self.assertEqual(timeout, 0.25)
            raise TimeoutError("token=probe-secret should not be archived")

        summary = PROBE.build_summary(
            message_def_ids=["colr.012.001.05"],
            timeout_secs=0.25,
            max_bytes=64,
            opener=opener,
        )

        self.assertFalse(summary["ok"])
        self.assertEqual(summary["successful_probe_count"], 0)
        probe = summary["probes"][0]
        self.assertEqual(probe["status"], "timeout")
        self.assertEqual(probe["error_kind"], "TimeoutError")
        self.assertNotIn("probe-secret", json.dumps(summary, sort_keys=True))

    def test_selector_validation_rejects_unknown_and_duplicate_ids_without_network(self):
        cases = (
            (
                ["--message-def-id", "pacs.999.001.01"],
                "is not a known pending schema",
            ),
            (
                [
                    "--message-def-id",
                    "colr.012.001.05",
                    "--message-def-id",
                    "colr.012.001.05",
                ],
                "duplicates --message-def-id[0]",
            ),
        )
        for argv, message in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_probe(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(message, stderr)

    def test_probe_limits_reject_boolean_aliases(self):
        with self.assertRaisesRegex(PROBE.ProbeError, "positive finite number"):
            PROBE.build_summary(timeout_secs=True)

        with self.assertRaisesRegex(PROBE.ProbeError, "positive integer"):
            PROBE.build_summary(max_bytes=False)


if __name__ == "__main__":
    unittest.main()
