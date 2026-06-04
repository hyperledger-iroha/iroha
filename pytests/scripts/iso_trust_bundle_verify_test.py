import contextlib
import base64
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_trust_bundle_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_trust_bundle_verify", SCRIPT_PATH)
VERIFIER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = VERIFIER
SPEC.loader.exec_module(VERIFIER)


SYNTHETIC_DER_B64 = "MAMCAQE="


def tlv(tag, value):
    if len(value) < 0x80:
        length = bytes([len(value)])
    else:
        length_body = len(value).to_bytes((len(value).bit_length() + 7) // 8, "big")
        length = bytes([0x80 | len(length_body)]) + length_body
    return bytes([tag]) + length + value


def seq(*children):
    return tlv(0x30, b"".join(children))


def der_integer(value):
    return tlv(0x02, bytes([value]))


def der_oid(raw):
    return tlv(0x06, raw)


def der_bit_string(raw=b"\x01"):
    return tlv(0x03, b"\x00" + raw)


def der_time(value=b"260604000000Z"):
    return tlv(0x17, value)


ALG_ID = seq(der_oid(b"\x2a\x86\x48\xce\x3d\x04\x03\x02"), tlv(0x05, b""))
NAME = seq()
VALIDITY = seq(der_time(), der_time(b"270604000000Z"))
SPKI = seq(ALG_ID, der_bit_string(b"\x02"))


def cert_b64(serial):
    version = tlv(0xA0, der_integer(2))
    tbs = seq(version, der_integer(serial), ALG_ID, NAME, VALIDITY, NAME, SPKI)
    return base64.b64encode(seq(tbs, ALG_ID, der_bit_string(b"\x03"))).decode("ascii")


def crl_b64():
    tbs = seq(der_integer(1), ALG_ID, NAME, der_time(), der_time(b"270604000000Z"))
    return base64.b64encode(seq(tbs, ALG_ID, der_bit_string(b"\x04"))).decode("ascii")


def ocsp_b64():
    response_bytes = seq(
        der_oid(b"\x2b\x06\x01\x05\x05\x07\x30\x01\x01"),
        tlv(0x04, seq()),
    )
    return base64.b64encode(seq(tlv(0x0A, b"\x00"), tlv(0xA0, response_bytes))).decode("ascii")


CERT_ONE_B64 = cert_b64(1)
CERT_TWO_B64 = cert_b64(2)
CERT_THREE_B64 = cert_b64(3)
CRL_B64 = crl_b64()
OCSP_B64 = ocsp_b64()


def der_digest(der_b64):
    return VERIFIER.sha256_hex(base64.b64decode(der_b64, validate=True))


def valid_bundle():
    return {
        "version": 1,
        "profile_id": "swift-cbpr-plus",
        "rail": "swift-cbpr-plus",
        "environment": "preprod",
        "source": {
            "authority": "Example Rail PKI",
            "version": "2026-Q2",
            "retrieved_at": "2026-06-04T00:00:00Z",
            "url": "https://pki.example.invalid/swift-cbpr-plus",
        },
        "embedded_signature_policy": "require-verified",
        "x509_trust_anchors": [
            {
                "label": "root-a",
                "der_base64": CERT_ONE_B64,
                "sha256": der_digest(CERT_ONE_B64),
            }
        ],
        "revoked_certificates": [
            {
                "label": "revoked-old-leaf",
                "der_base64": CERT_TWO_B64,
                "sha256": der_digest(CERT_TWO_B64),
            }
        ],
        "x509_required_certificate_policy_oids": ["1.3.6.1.4.1.55555.1"],
        "x509_require_crl_revocation_check": True,
        "x509_crls": [
            {
                "label": "rail-crl",
                "der_base64": CRL_B64,
                "sha256": der_digest(CRL_B64),
            }
        ],
        "x509_require_ocsp_revocation_check": True,
        "x509_ocsp_responses": [
            {
                "label": "rail-ocsp",
                "der_base64": OCSP_B64,
                "sha256": der_digest(OCSP_B64),
            }
        ],
    }


def write_bundle(root, body):
    path = root / "trust-bundle.json"
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")
    return path


def run_verify(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = VERIFIER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoTrustBundleVerifyTest(unittest.TestCase):
    def test_valid_bundle_emits_digest_bound_summary_and_profile_overrides(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())
            summary_path = root / "summary.json"
            profile_path = root / "profile.json"

            rc, stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--summary-out",
                    str(summary_path),
                    "--emit-profile-json",
                    str(profile_path),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["verified_bundles"], 1)
            self.assertFalse(summary["allow_record_only"])
            self.assertFalse(summary["allow_insecure_source_url"])
            self.assertFalse(summary["allow_synthetic_der"])
            self.assertTrue(summary["profile_json_emittable"])
            self.assertTrue(summary["profile_json_emitted"])
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, VERIFIER.sha256_hex(VERIFIER._canonical_json_bytes(body)))
            self.assertEqual(json.loads(summary_path.read_text(encoding="utf-8")), summary)
            bundle_summary = summary["bundles"][0]
            self.assertEqual(bundle_summary["profile_id"], "swift-cbpr-plus")
            self.assertEqual(bundle_summary["material"]["x509_trust_anchor_pin_count"], 1)
            self.assertEqual(bundle_summary["material"]["revoked_certificate_pin_count"], 1)
            self.assertNotIn("der_base64", json.dumps(bundle_summary["x509_trust_anchors"]))
            emitted = json.loads(profile_path.read_text(encoding="utf-8"))
            self.assertEqual(
                emitted[0]["x509_trust_anchor_sha256_pins"],
                [der_digest(CERT_ONE_B64)],
            )
            self.assertEqual(emitted[0]["x509_require_crl_revocation_check"], True)
            self.assertEqual(emitted[0]["x509_require_ocsp_revocation_check"], True)

    def test_declared_der_digest_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"][0]["sha256"] = "a" * 64
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("does not match der_base64", stderr)

    def test_noncanonical_pin_and_all_zero_pin_are_rejected(self):
        for pin in ["A" * 64, "0" * 64]:
            with self.subTest(pin=pin):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle["signature_public_key_sha256_pins"] = [pin]
                    path = write_bundle(root, bundle)

                    rc, _stdout, _stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)

    def test_duplicate_der_material_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            duplicate = dict(bundle["x509_trust_anchors"][0])
            duplicate["label"] = "root-a-copy"
            bundle["x509_trust_anchors"].append(duplicate)
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicates DER SHA-256", stderr)

    def test_duplicate_der_labels_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"].append(
                {
                    "label": "root-a",
                    "der_base64": CERT_THREE_B64,
                    "sha256": der_digest(CERT_THREE_B64),
                }
            )
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicates label", stderr)

    def test_trust_anchor_also_revoked_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["revoked_certificates"][0] = dict(bundle["x509_trust_anchors"][0])
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("trusted/revoked certificate pins", stderr)

    def test_legacy_and_current_pin_alias_conflicts_are_rejected(self):
        for mutate, message in [
            (
                lambda bundle: (
                    bundle.update(
                        {
                            "signature_public_key_sha256_pins": ["1" * 64],
                            "trusted_public_key_sha256": ["1" * 64],
                        }
                    )
                ),
                "signature_public_key_sha256_pins/trusted_public_key_sha256",
            ),
            (
                lambda bundle: (
                    bundle.update(
                        {
                            "x509_trust_anchor_sha256_pins": ["2" * 64],
                            "trusted_certificate_sha256": ["2" * 64],
                        }
                    )
                ),
                "x509_trust_anchor_sha256_pins/trusted_certificate_sha256",
            ),
        ]:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_required_crl_and_ocsp_material_must_be_present(self):
        for key, message in [
            ("x509_crls", "requires CRL revocation checking"),
            ("x509_ocsp_responses", "requires OCSP revocation checking"),
        ]:
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle[key] = []
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_secret_material_and_unknown_keys_are_rejected(self):
        for mutate, message in [
            (lambda bundle: bundle.update({"authorization": "Bearer secret"}), "unknown keys"),
            (lambda bundle: bundle["source"].update({"token": "secret"}), "forbidden secret"),
            (lambda bundle: bundle["source"].update({"version": "Bearer secret"}), "bearer-token"),
        ]:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_insecure_source_url_requires_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["source"]["url"] = "http://pki.local/swift-cbpr-plus"
            path = write_bundle(root, bundle)

            self.assertEqual(run_verify(["--bundle", str(path)])[0], 2)
            self.assertEqual(
                run_verify(["--bundle", str(path), "--allow-insecure-source-url"])[0],
                0,
            )

    def test_source_url_rejects_credentials_query_fragment_and_local_addresses(self):
        cases = [
            ("https://user:pass@pki.example.invalid/swift-cbpr-plus", "credentials"),
            ("https://pki.example.invalid/swift-cbpr-plus?token=abc", "params, query, or fragment"),
            ("https://pki.example.invalid/swift-cbpr-plus#bundle", "params, query, or fragment"),
            ("https://pki.example.invalid/swift-cbpr-plus\nX-Token: abc", "ASCII control"),
            ("https://[::1", "malformed"),
            ("https:///swift-cbpr-plus", "include a host"),
            ("https://localhost/swift-cbpr-plus", "localhost"),
            ("https://127.0.0.1/swift-cbpr-plus", "local, private, or reserved IP"),
            ("https://10.1.2.3/swift-cbpr-plus", "local, private, or reserved IP"),
            ("https://[::1]/swift-cbpr-plus", "local, private, or reserved IP"),
        ]
        for url, message in cases:
            with self.subTest(url=url):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle["source"]["url"] = url
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_local_source_url_override_is_limited_to_local_audits(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["source"]["url"] = "http://127.0.0.1/swift-cbpr-plus"
            path = write_bundle(root, bundle)

            self.assertEqual(run_verify(["--bundle", str(path)])[0], 2)
            self.assertEqual(
                run_verify(["--bundle", str(path), "--allow-insecure-source-url"])[0],
                0,
            )

    def test_source_retrieved_at_must_be_parseable_timezone_aware_and_not_future(self):
        future = (
            VERIFIER.dt.datetime.now(VERIFIER.dt.UTC) + VERIFIER.dt.timedelta(days=1)
        ).replace(microsecond=0).isoformat().replace("+00:00", "Z")
        cases = [
            ("not-a-date", "ISO 8601"),
            ("2026-06-04T00:00:00", "timezone"),
            (future, "future"),
            ("2026-06-04T00:00:00Z\nX-Token: abc", "ASCII control"),
        ]
        for retrieved_at, message in cases:
            with self.subTest(retrieved_at=retrieved_at):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle["source"]["retrieved_at"] = retrieved_at
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_malformed_der_envelope_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_crls"][0]["der_base64"] = "MAEAAA=="
            bundle["x509_crls"][0]["sha256"] = VERIFIER.sha256_hex(b"\x30\x01\x00\x00")
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("DER length does not consume", stderr)

    def test_der_material_must_match_declared_material_class(self):
        cases = [
            ("x509_trust_anchors", CRL_B64, "X.509 certificate"),
            ("x509_crls", CERT_ONE_B64, "X.509 CRL"),
            ("x509_ocsp_responses", CERT_ONE_B64, "OCSPResponse"),
        ]
        for key, der_b64, message in cases:
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle[key][0]["der_base64"] = der_b64
                    bundle[key][0]["sha256"] = der_digest(der_b64)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_synthetic_der_placeholder_requires_explicit_template_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"][0]["der_base64"] = SYNTHETIC_DER_B64
            bundle["x509_trust_anchors"][0]["sha256"] = der_digest(SYNTHETIC_DER_B64)
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("must look like an X.509 certificate", stderr)
            rc, stdout, stderr = run_verify(["--bundle", str(path), "--allow-synthetic-der"])
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["allow_synthetic_der"])
            self.assertFalse(summary["profile_json_emittable"])
            self.assertFalse(summary["profile_json_emitted"])

    def test_synthetic_der_cannot_emit_profile_overrides(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"][0]["der_base64"] = SYNTHETIC_DER_B64
            bundle["x509_trust_anchors"][0]["sha256"] = der_digest(SYNTHETIC_DER_B64)
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--allow-synthetic-der",
                    "--emit-profile-json",
                    str(root / "profile.json"),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("cannot be combined with --emit-profile-json", stderr)

    def test_record_only_policy_requires_explicit_nonproduction_override(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["embedded_signature_policy"] = "record-only"
            path = write_bundle(root, bundle)

            self.assertEqual(run_verify(["--bundle", str(path)])[0], 2)
            self.assertEqual(run_verify(["--bundle", str(path), "--allow-record-only"])[0], 0)

    def test_checked_in_trust_bundle_templates_verify(self):
        template_dir = REPO_ROOT / "fixtures" / "iso20022" / "trust_bundles"
        templates = sorted(template_dir.glob("*.example.json"))
        self.assertGreaterEqual(len(templates), 4)
        argv = []
        for template in templates:
            argv.extend(["--bundle", str(template)])
        argv.append("--allow-synthetic-der")

        rc, stdout, stderr = run_verify(argv)

        self.assertEqual(rc, 0, stderr)
        summary = json.loads(stdout)
        self.assertEqual(summary["verified_bundles"], len(templates))
        self.assertTrue(summary["allow_synthetic_der"])
        self.assertFalse(summary["profile_json_emittable"])
        self.assertEqual(
            sorted(bundle["profile_id"] for bundle in summary["bundles"]),
                [
                    "fedwire-funds",
                    "securities-csd",
                    "sepa-sct-inst",
                    "swift-cbpr-plus",
                ],
        )


if __name__ == "__main__":
    unittest.main()
