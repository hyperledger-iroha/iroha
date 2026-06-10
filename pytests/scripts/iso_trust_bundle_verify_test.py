import argparse
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
PROFILE_FRESHNESS_ARGS = ["--max-source-age-days", "36500"]


def der_digest(der_b64):
    return VERIFIER.sha256_hex(base64.b64decode(der_b64, validate=True))


def valid_bundle():
    return {
        "version": 1,
        "profile_id": "swift-cbpr-plus",
        "rail": "swift-cbpr-plus",
        "environment": "preprod",
        "source": {
            "authority": "Local Bank Rail PKI",
            "version": "2026-Q2",
            "retrieved_at": "2026-06-04T00:00:00Z",
            "url": "https://pki.local-bank.bank/swift-cbpr-plus",
        },
        "embedded_signature_policy": "require-verified",
        "signature_public_key_sha256_pins": [],
        "trusted_public_key_sha256": [],
        "x509_trust_anchor_sha256_pins": [],
        "trusted_certificate_sha256": [],
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
        "revoked_certificate_sha256": [],
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
    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_trust_unknown_secret", "trust_unknown_secret"),
            ("%70assword_trust_unknown_leak", "trust_unknown_leak"),
            ("private-key_trust_unknown_leak", "trust_unknown_leak"),
            ("unexpected\x1btrust_key", "\x1b"),
            ("unexpected_trust_\uff4bey", "\uff4b"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    VERIFIER._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "bundle"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(VERIFIER.TrustBundleError) as caught:
            VERIFIER._reject_unknown_keys(many_unknown, set(), "bundle")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=trust-terminator-secret"
        cases = (
            (
                "raw",
                lambda: VERIFIER._preflight_raw_cli_secrets(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "path",
                lambda: VERIFIER._preflight_output_cli_paths(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "boolean",
                lambda: VERIFIER._preflight_boolean_cli_flags(
                    ["--", "--allow-record-only", hidden],
                    {"--allow-record-only"},
                ),
            ),
            (
                "positive_int",
                lambda: VERIFIER._preflight_positive_int_cli_values(
                    ["--", "--max-source-age-days", hidden],
                    {"--max-source-age-days"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("trust-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                VERIFIER.build_parser().parse_args(
                    ["--bundle", "bundle.json", "--summary-ou", "out"]
                )

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--summary-ou", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        hidden = "--unknown-trust\x1bflag"
        with self.assertRaises(VERIFIER.TrustBundleError) as caught:
            VERIFIER._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must not contain control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unknown-trust", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dsummary-out"
        with self.assertRaises(VERIFIER.TrustBundleError) as caught:
            VERIFIER._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("summary-out", message)

    def test_nested_control_material_in_bundle_is_rejected_without_echo(self):
        cases = (
            (
                {"metadata": {"unexpected\x1btrust_key": "redacted"}},
                "forbidden control-bearing field",
                "trust_key",
            ),
            (
                {"metadata": {"note": "warning \x1b[31mred"}},
                "unsafe control characters",
                "[31mred",
            ),
        )
        for body, expected, hidden in cases:
            with self.subTest(body=body):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    VERIFIER._check_no_secret_material(body)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn(hidden, message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--summary-out"],
            ["--summary-out", ""],
            ["--summary-out", "--emit-profile-json"],
            ["--summary-out="],
            ["--summary-out=--emit-profile-json"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    VERIFIER.TrustBundleError,
                    "--summary-out requires a path value",
                ):
                    VERIFIER._preflight_output_cli_paths(argv, {"--summary-out"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=trust-path-leak.summary.json", "token=trust-path-leak"),
            ("token%3Dtrust-path-leak.summary.json", "token=trust-path-leak"),
            ("%70assword%253Dtrust-path-leak.summary.json", "password=trust-path-leak"),
            ("token-trust-path-secret.summary.json", "token-trust-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    VERIFIER._preflight_output_cli_paths(
                        ["--summary-out", raw_path], {"--summary-out"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("trust-path-leak", message)

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (VERIFIER.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "raw overlong",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                overlong_path,
                f"no longer than {VERIFIER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "output overlong",
                lambda raw: VERIFIER._reject_output_path_smuggling(Path(raw), "output path"),
                overlong_path,
                f"no longer than {VERIFIER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw encoded dot",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/summary.json",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: VERIFIER._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/summary.json",
                "encoded dot or separator",
            ),
            (
                "raw uri prefix",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/summary.json",
                "URI or drive prefixes",
            ),
            (
                "output drive prefix",
                lambda raw: VERIFIER._reject_output_path_smuggling(Path(raw), "output path"),
                "C:/out/summary.json",
                "URI or drive prefixes",
            ),
            (
                "raw encoded semicolon",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%3b/summary.json",
                "encoded semicolon",
            ),
            (
                "raw encoded delimiter",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%40/summary.json",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/summary.json",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/summary.json",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/summary.json",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_overlong_bundle_strings_are_rejected_without_echo(self):
        overlong = "M" * (VERIFIER.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "required",
                lambda: VERIFIER._required_string(
                    {"authority": overlong}, "authority", "bundle.source"
                ),
                f"bundle.source.authority must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "optional",
                lambda: VERIFIER._optional_string(
                    {"label": overlong}, "label", "bundle.x509_trust_anchors[0]"
                ),
                f"bundle.x509_trust_anchors[0].label must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "oid",
                lambda: VERIFIER._oid_list(
                    {"x509_required_certificate_policy_oids": [overlong]},
                    "x509_required_certificate_policy_oids",
                    "bundle",
                ),
                f"bundle.x509_required_certificate_policy_oids[0] must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["environment"] = overlong
            path = write_bundle(root, bundle)

            rc, stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                f".environment must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
                stderr,
            )
            self.assertNotIn(overlong, stderr)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://pki.local-bank.bank/source:debug",
            "https://pki.local-bank.bank/source@debug",
            "https://pki.local-bank.bank/source[debug]",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    VERIFIER._validate_source_url(
                        url,
                        "source.url",
                        allow_insecure_source_url=False,
                    )

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(url, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://pki\u0661.local-bank.bank/source",
                "host must use printable ASCII",
            ),
            ("https://pki.local-bank.bank/source∕debug", "path must use printable ASCII"),
            (
                "https://pki.local-bank.bank/source%c3%a9",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                    VERIFIER._validate_source_url(
                        url,
                        "source.url",
                        allow_insecure_source_url=False,
                    )

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--allow-record-only=true"], "--allow-record-only", "--allow-record-only=true"),
            (["--allow-synthetic-der", "true"], "--allow-synthetic-der", "true"),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=trust-secret"],
            ["token=trust-secret"],
            ["password=trust-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("trust-secret", stderr)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "bundle.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        VERIFIER.TrustBundleError,
                        "max file bytes must be a positive integer",
                    ):
                        VERIFIER._read_regular_file(path, max_bytes=limit)

    def test_valid_bundle_emits_digest_bound_summary_and_profile_overrides(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())
            summary_path = root / "summary.json"
            profile_path = root / "profile.json"
            summary_path.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")
            profile_path.write_text('[{"stale": true}]\n' + ("x" * 4096), encoding="utf-8")

            rc, stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--summary-out",
                    str(summary_path),
                    "--emit-profile-json",
                    str(profile_path),
                    *PROFILE_FRESHNESS_ARGS,
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["version"], VERIFIER.TRUST_SUMMARY_VERSION)
            self.assertEqual(summary["verified_bundles"], 1)
            self.assertFalse(summary["allow_record_only"])
            self.assertFalse(summary["allow_insecure_source_url"])
            self.assertFalse(summary["allow_synthetic_der"])
            self.assertEqual(summary["max_source_age_days"], 36500)
            self.assertTrue(summary["profile_json_emittable"])
            self.assertTrue(summary["profile_json_emitted"])
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, VERIFIER.sha256_hex(VERIFIER._canonical_json_bytes(body)))
            self.assertEqual(json.loads(summary_path.read_text(encoding="utf-8")), summary)
            self.assertEqual(summary_path.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(summary_path.parent.glob(".iso-*.tmp")),
                [],
            )
            bundle_summary = summary["bundles"][0]
            self.assertEqual(bundle_summary["profile_id"], "swift-cbpr-plus")
            self.assertEqual(bundle_summary["material"]["x509_trust_anchor_pin_count"], 1)
            self.assertEqual(bundle_summary["material"]["revoked_certificate_pin_count"], 1)
            self.assertNotIn("der_base64", json.dumps(bundle_summary["x509_trust_anchors"]))
            profile_text = profile_path.read_text(encoding="utf-8")
            self.assertEqual(
                summary["profile_json_sha256"],
                VERIFIER.sha256_hex(profile_text.encode("utf-8")),
            )
            emitted = json.loads(profile_text)
            self.assertEqual(profile_path.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(profile_path.parent.glob(".iso-*.tmp")),
                [],
            )
            self.assertEqual(
                emitted[0]["x509_trust_anchor_sha256_pins"],
                [der_digest(CERT_ONE_B64)],
            )
            self.assertEqual(emitted[0]["x509_require_crl_revocation_check"], True)
            self.assertEqual(emitted[0]["x509_require_ocsp_revocation_check"], True)

    def test_symlinked_bundle_is_rejected_before_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = write_bundle(root, valid_bundle())
            bundle = root / "symlinked-trust-bundle.json"
            try:
                bundle.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_verify(["--bundle", str(bundle)])

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_symlinked_bundle_ancestor_is_rejected_before_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "bundle-target"
            target_dir.mkdir()
            target = write_bundle(target_dir, valid_bundle())
            ancestor = root / "bundle-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            bundle = ancestor / target.name

            rc, stdout, stderr = run_verify(["--bundle", str(bundle)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_directory_bundle_is_rejected_before_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            bundle = Path(raw_root) / "bundle-dir"
            bundle.mkdir()

            rc, _stdout, stderr = run_verify(["--bundle", str(bundle)])

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

    def test_oversized_bundle_is_rejected_before_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            bundle = Path(raw_root) / "oversized-trust-bundle.json"
            old_limit = VERIFIER.MAX_BUNDLE_JSON_BYTES
            try:
                VERIFIER.MAX_BUNDLE_JSON_BYTES = 128
                bundle.write_text(
                    '{"version":1,"profile_id":"swift-cbpr-plus","padding":"'
                    + ("a" * VERIFIER.MAX_BUNDLE_JSON_BYTES)
                    + '"}',
                    encoding="utf-8",
                )

                rc, stdout, stderr = run_verify(["--bundle", str(bundle)])
            finally:
                VERIFIER.MAX_BUNDLE_JSON_BYTES = old_limit

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("exceeds", stderr)

    def test_bundle_cli_path_rejects_raw_smuggling_before_read(self):
        cases = (
            ("semicolon", "bundle;debug.json", "semicolon path"),
            ("whitespace", "trust bundle.json", "whitespace"),
            ("leading-dash", "nested/-bundle.json", "leading-dash path segments"),
            ("parent", "nested/../bundle.json", "dot or parent"),
            ("dot", lambda root: f"{root}/nested/./bundle.json", "dot or parent"),
            ("empty", lambda root: f"{root}//bundle.json", "empty path"),
        )
        for name, bundle_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = (
                        bundle_arg(root) if callable(bundle_arg) else str(root / bundle_arg)
                    )

                    rc, stdout, stderr = run_verify(["--bundle", value])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_direct_run_paths_reject_smuggling_before_bundle_loading(self):
        def args_for(root, **overrides):
            values = {
                "bundle": [root / "missing-bundle.json"],
                "summary_out": root / "trust.summary.json",
                "emit_profile_json": None,
                "allow_record_only": False,
                "allow_insecure_source_url": False,
                "allow_synthetic_der": False,
                "max_source_age_days": None,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "bundle whitespace",
                lambda root: args_for(root, bundle=[root / "trust bundle.json"]),
                "--bundle[0] must not contain whitespace",
            ),
            (
                "profile parent",
                lambda root: args_for(
                    root,
                    emit_profile_json=root / "nested" / ".." / "profiles.json",
                ),
                "output path must not contain dot or parent segments",
            ),
            (
                "summary leading dash",
                lambda root: args_for(
                    root,
                    summary_out=root / "nested" / "-trust.summary.json",
                ),
                "output path must not contain leading-dash path segments",
            ),
        )
        for name, make_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaises(VERIFIER.TrustBundleError) as caught:
                        VERIFIER.run(make_args(root))

                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)

        for bundle in (None, []):
            with self.subTest(bundle=bundle):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaisesRegex(
                        VERIFIER.TrustBundleError,
                        "provide at least one --bundle",
                    ):
                        VERIFIER.run(args_for(root, bundle=bundle))

    def test_secret_looking_cli_paths_are_rejected_before_summary_output(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            clean_bundle = write_bundle(root, valid_bundle())
            cases = (
                (
                    ["--bundle", str(root / "token=trust-path-secret.bundle.json")],
                    root / "token=trust-path-secret.bundle.json",
                ),
                (
                    [
                        "--bundle",
                        str(clean_bundle),
                        "--summary-out",
                        str(root / "token=trust-summary-secret.summary.json"),
                    ],
                    root / "token=trust-summary-secret.summary.json",
                ),
            )
            for argv, secret_path in cases:
                with self.subTest(secret_path=secret_path.name):
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret_path.name, stderr)
                    self.assertFalse(secret_path.exists())

    def test_symlinked_output_files_are_rejected(self):
        cases = (
            ("summary", "--summary-out"),
            ("profile", "--emit-profile-json"),
        )
        for name, flag in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    path = write_bundle(root, valid_bundle())
                    target = root / f"{name}-target.json"
                    target.write_text("untouched\n", encoding="utf-8")
                    output_path = root / f"{name}-link.json"
                    try:
                        output_path.symlink_to(target)
                    except OSError as error:
                        self.skipTest(f"symlink creation unavailable: {error}")

                    argv = ["--bundle", str(path), flag, str(output_path)]
                    if flag == "--emit-profile-json":
                        argv.extend(PROFILE_FRESHNESS_ARGS)
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be a symlink", stderr)
                    self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_output_paths_reject_smuggled_segments(self):
        cases = (
            ("summary semicolon", "--summary-out", "summary;debug.json", "semicolon path"),
            ("profile whitespace", "--emit-profile-json", "profile out.json", "whitespace"),
            ("summary leading dash", "--summary-out", "nested/-summary.json", "leading-dash"),
            ("profile parent", "--emit-profile-json", "nested/../profile.json", "dot or parent"),
            (
                "summary dot",
                "--summary-out",
                lambda root: f"{root}/nested/./summary.json",
                "dot or parent",
            ),
            (
                "profile empty",
                "--emit-profile-json",
                lambda root: f"{root}//profile.json",
                "empty path",
            ),
        )
        for name, flag, output_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    path = write_bundle(root, valid_bundle())
                    output_path = (
                        output_arg(root) if callable(output_arg) else str(root / output_arg)
                    )

                    argv = ["--bundle", str(path), flag, output_path]
                    if flag == "--emit-profile-json":
                        argv.extend(PROFILE_FRESHNESS_ARGS)
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_output_files_reject_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "trust.summary.json"

            with self.assertRaisesRegex(
                VERIFIER.TrustBundleError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                VERIFIER._write_text_output(output_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())
            with self.assertRaisesRegex(
                VERIFIER.TrustBundleError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                VERIFIER._reject_repository_output_path(
                    Path("fixtures/iso20022/trust.summary.json"),
                    "output path",
                )

    def test_output_files_reject_repository_fixture_before_bundle_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            cases = (
                (
                    "summary",
                    "--summary-out",
                    root / "fixtures" / "iso20022" / "trust.summary.json",
                ),
                (
                    "profile",
                    "--emit-profile-json",
                    root / "fixtures" / "iso20022" / "trust-profile.json",
                ),
            )
            for name, flag, output_path in cases:
                with self.subTest(name=name):
                    rc, stdout, stderr = run_verify(
                        [
                            "--bundle",
                            str(root / f"missing-{name}-trust-bundle.json"),
                            flag,
                            str(output_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        "output path must not point to checked-in ISO fixture artifacts",
                        stderr,
                    )
                    self.assertNotIn("does not exist", stderr)
                    self.assertFalse((root / "fixtures").exists())

    def test_hardlinked_output_files_are_rejected(self):
        cases = (
            ("summary", "--summary-out"),
            ("profile", "--emit-profile-json"),
        )
        for name, flag in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    path = write_bundle(root, valid_bundle())
                    target = root / f"{name}-target.json"
                    target.write_text("untouched\n", encoding="utf-8")
                    output_path = root / f"{name}-hardlink.json"
                    try:
                        output_path.hardlink_to(target)
                    except OSError as error:
                        self.skipTest(f"hard link creation unavailable: {error}")

                    argv = ["--bundle", str(path), flag, str(output_path)]
                    if flag == "--emit-profile-json":
                        argv.extend(PROFILE_FRESHNESS_ARGS)
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be hard-linked", stderr)
                    self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_output_ancestors_are_rejected(self):
        cases = (
            ("summary", "--summary-out"),
            ("profile", "--emit-profile-json"),
        )
        for name, flag in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    path = write_bundle(root, valid_bundle())
                    target_dir = root / f"{name}-target"
                    target_dir.mkdir()
                    ancestor = root / f"{name}-ancestor-link"
                    try:
                        ancestor.symlink_to(target_dir, target_is_directory=True)
                    except OSError as error:
                        self.skipTest(f"symlink creation unavailable: {error}")
                    output_path = ancestor / "nested" / f"{name}.json"

                    argv = ["--bundle", str(path), flag, str(output_path)]
                    if flag == "--emit-profile-json":
                        argv.extend(PROFILE_FRESHNESS_ARGS)
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be a symlink", stderr)
                    self.assertFalse((target_dir / "nested").exists())

    def test_profile_output_failure_does_not_emit_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())
            summary_path = root / "summary.json"
            target = root / "profile-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            profile_path = root / "profile-link.json"
            try:
                profile_path.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--summary-out",
                    str(summary_path),
                    "--emit-profile-json",
                    str(profile_path),
                    *PROFILE_FRESHNESS_ARGS,
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertFalse(summary_path.exists())
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_summary_and_profile_outputs_must_be_distinct(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())
            output_path = root / "same-output.json"

            rc, stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--summary-out",
                    str(output_path),
                    "--emit-profile-json",
                    str(output_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertFalse(output_path.exists())
            self.assertIn("must be different paths", stderr)

    def test_revocation_policy_flags_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for flag in (
                "x509_require_crl_revocation_check",
                "x509_require_ocsp_revocation_check",
            ):
                with self.subTest(flag=flag):
                    bundle = valid_bundle()
                    del bundle[flag]
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{flag} must be a boolean", stderr)

    def test_material_arrays_must_be_explicit(self):
        keys = (
            "signature_public_key_sha256_pins",
            "trusted_public_key_sha256",
            "x509_trust_anchor_sha256_pins",
            "trusted_certificate_sha256",
            "x509_trust_anchors",
            "revoked_certificate_sha256",
            "revoked_certificates",
            "x509_required_certificate_policy_oids",
            "x509_crls",
            "x509_ocsp_responses",
        )
        for key in keys:
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle.pop(key)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{key} must be recorded as an array", stderr)

    def test_duplicate_bundle_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "trust-bundle.json"
            path.write_text(
                '{"version":1,"token=trust-duplicate-key-secret":1,"token=trust-duplicate-key-secret":2}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("trust-duplicate-key-secret", stderr)

    def test_non_finite_bundle_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "trust-bundle.json"
            path.write_text('{"version":NaN}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_bundle_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "trust-bundle.json"
            path.write_text('{"profile_id":"\\ud800"}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_duplicate_bundle_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            original = write_bundle(root, valid_bundle())

            rc, _stdout, stderr = run_verify(
                ["--bundle", str(original), "--bundle", str(original)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("--bundle[1] duplicates --bundle[0]", stderr)

            secret_bundle = root / "token=trust-duplicate-secret.bundle.json"
            secret_bundle.write_text(original.read_text(encoding="utf-8"), encoding="utf-8")
            rc, _stdout, stderr = run_verify(
                ["--bundle", str(secret_bundle), "--bundle", str(secret_bundle)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("trust-duplicate-secret", stderr)

            copied_dir = root / "copied"
            copied_dir.mkdir()
            copied = write_bundle(copied_dir, valid_bundle())
            rc, _stdout, stderr = run_verify(
                ["--bundle", str(original), "--bundle", str(copied)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("bundle_sha256 duplicates", stderr)

            same_profile_dir = root / "same-profile"
            same_profile_dir.mkdir()
            same_profile = valid_bundle()
            same_profile["source"]["version"] = "2026-Q3"
            same_profile_path = write_bundle(same_profile_dir, same_profile)
            rc, _stdout, stderr = run_verify(
                ["--bundle", str(original), "--bundle", str(same_profile_path)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("profile_id duplicates", stderr)

    def test_declared_der_digest_mismatch_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"][0]["sha256"] = "a" * 64
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("does not match der_base64", stderr)

    def test_declared_der_digest_must_be_recorded_as_string(self):
        for key in (
            "x509_trust_anchors",
            "revoked_certificates",
            "x509_crls",
            "x509_ocsp_responses",
        ):
            for value in (None, "omitted"):
                with self.subTest(key=key, value=value):
                    with tempfile.TemporaryDirectory() as raw_root:
                        root = Path(raw_root)
                        bundle = valid_bundle()
                        if value == "omitted":
                            bundle[key][0].pop("sha256")
                        else:
                            bundle[key][0]["sha256"] = value
                        path = write_bundle(root, bundle)

                        rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                        self.assertEqual(rc, 2)
                        self.assertIn(f"{key}[0].sha256 must be a string", stderr)

    def test_oversized_der_base64_is_rejected_before_decode(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_trust_anchors"][0]["der_base64"] = (
                "A" * (VERIFIER.MAX_DER_BASE64_CHARS + 1)
            )
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("must decode to no more than", stderr)

    def test_absent_der_labels_are_omitted_from_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            for key in (
                "x509_trust_anchors",
                "revoked_certificates",
                "x509_crls",
                "x509_ocsp_responses",
            ):
                bundle[key][0].pop("label")
            path = write_bundle(root, bundle)

            rc, stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--emit-profile-json",
                    str(root / "profile.json"),
                    *PROFILE_FRESHNESS_ARGS,
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            bundle_summary = summary["bundles"][0]
            for key in (
                "x509_trust_anchors",
                "revoked_certificates",
                "x509_crls",
                "x509_ocsp_responses",
            ):
                self.assertNotIn("label", bundle_summary[key][0])

    def test_json_strings_must_not_require_trimming(self):
        cases = (
            (
                "profile-id",
                lambda bundle: bundle.__setitem__("profile_id", " swift-cbpr-plus"),
                "profile_id must not have surrounding whitespace",
            ),
            (
                "source-version",
                lambda bundle: bundle["source"].__setitem__("version", "2026-Q2 "),
                "source.version must not have surrounding whitespace",
            ),
            (
                "source-authority",
                lambda bundle: bundle["source"].__setitem__(
                    "authority",
                    " Example Rail PKI",
                ),
                "source.authority must not have surrounding whitespace",
            ),
            (
                "public-pin",
                lambda bundle: bundle.__setitem__(
                    "signature_public_key_sha256_pins",
                    ["1" * 64 + " "],
                ),
                "signature_public_key_sha256_pins[0] must not have surrounding whitespace",
            ),
            (
                "policy-oid",
                lambda bundle: bundle["x509_required_certificate_policy_oids"].__setitem__(
                    0,
                    " 1.3.6.1.4.1.55555.1",
                ),
                "x509_required_certificate_policy_oids[0] must not have surrounding whitespace",
            ),
            (
                "der-base64",
                lambda bundle: bundle["x509_crls"][0].__setitem__(
                    "der_base64",
                    CRL_B64 + " ",
                ),
                "x509_crls[0].der_base64 must not have surrounding whitespace",
            ),
            (
                "der-digest",
                lambda bundle: bundle["x509_crls"][0].__setitem__(
                    "sha256",
                    der_digest(CRL_B64) + " ",
                ),
                "x509_crls[0].sha256 must not have surrounding whitespace",
            ),
            (
                "der-label-null",
                lambda bundle: bundle["x509_crls"][0].__setitem__("label", None),
                "x509_crls[0].label must be a non-empty string when provided",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_non_ascii_der_label_is_rejected_without_echo(self):
        hidden = "\u2011"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["x509_crls"][0]["label"] = f"rail{hidden}crl"
            path = write_bundle(root, bundle)

            rc, stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("x509_crls[0].label must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)

    def test_trust_profile_identity_fields_are_canonical(self):
        cases = (
            (
                "uppercase-profile-id",
                lambda bundle: bundle.__setitem__("profile_id", "Swift-CBPR-Plus"),
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "underscore-profile-id",
                lambda bundle: bundle.__setitem__("profile_id", "swift_cbpr_plus"),
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "trailing-hyphen-profile-id",
                lambda bundle: bundle.__setitem__("profile_id", "swift-cbpr-plus-"),
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "unknown-rail",
                lambda bundle: bundle.__setitem__("rail", "swift"),
                "rail must be one of",
            ),
            (
                "uppercase-rail",
                lambda bundle: bundle.__setitem__("rail", "Swift-CBPR-Plus"),
                "rail must be one of",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

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
            self.assertNotIn(der_digest(CERT_ONE_B64), stderr)

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
            self.assertNotIn("root-a", stderr)

    def test_trust_anchor_also_revoked_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["revoked_certificates"][0] = dict(bundle["x509_trust_anchors"][0])
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("trusted/revoked certificate pins", stderr)
            self.assertNotIn(der_digest(CERT_ONE_B64), stderr)

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
                    self.assertNotIn("1" * 64, stderr)
                    self.assertNotIn("2" * 64, stderr)

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

    def test_overlong_trust_profile_identity_values_are_rejected_without_echo(self):
        overlong_profile = "a" * 129
        overlong_policy = "require-verified-" + ("a" * 129)
        cases = (
            (
                "profile-id",
                lambda bundle: bundle.__setitem__("profile_id", overlong_profile),
                "profile_id must be no longer than 128 characters",
                overlong_profile,
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "policy",
                lambda bundle: bundle.__setitem__(
                    "embedded_signature_policy",
                    overlong_policy,
                ),
                "embedded_signature_policy must be no longer than 128 characters",
                overlong_policy,
                "embedded_signature_policy is unsupported",
            ),
        )
        for name, mutate, message, hidden, bypassed_message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(bypassed_message, stderr)

    def test_secret_material_and_unknown_keys_are_rejected(self):
        cases = [
            (
                lambda bundle: bundle.update({"authorization": "Bearer top-level-secret"}),
                "unknown keys",
                "top-level-secret",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"password_source_field_secret": "redacted"}
                ),
                "forbidden secret",
                "source_field_secret",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"private-key_source_field_secret": "redacted"}
                ),
                "forbidden secret",
                "source_field_secret",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"%70assword_source_field_secret": "redacted"}
                ),
                "forbidden secret",
                "source_field_secret",
            ),
            (
                lambda bundle: bundle.update(
                    {"profile_id": "token-trust-profile-secret"}
                ),
                "secret-looking material",
                "token-trust-profile-secret",
            ),
            (
                lambda bundle: bundle.update(
                    {"environment": "token-trust-environment-secret"}
                ),
                "secret-looking material",
                "token-trust-environment-secret",
            ),
            (
                lambda bundle: bundle.update({"rail": "token-trust-rail-secret"}),
                "secret-looking material",
                "token-trust-rail-secret",
            ),
            (
                lambda bundle: bundle.update(
                    {"embedded_signature_policy": "token-trust-policy-secret"}
                ),
                "secret-looking material",
                "token-trust-policy-secret",
            ),
            (
                lambda bundle: bundle.update(
                    {"embedded_signature_policy": "require-verif\u0456ed"}
                ),
                "must use printable ASCII",
                "require-verif\u0456ed",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"authority": "token-trust-authority-secret"}
                ),
                "secret-looking material",
                "token-trust-authority-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"authority": "ISO\u2011MDR"}),
                "must use printable ASCII",
                "ISO\u2011MDR",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"version": "session-key-trust-version-secret"}
                ),
                "secret-looking material",
                "session-key-trust-version-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"version": "2026\u2011Q2"}),
                "must use printable ASCII",
                "2026\u2011Q2",
            ),
            (
                lambda bundle: bundle["x509_trust_anchors"][0].update(
                    {"label": "token-trust-label-secret"}
                ),
                "secret-looking material",
                "token-trust-label-secret",
            ),
            (
                lambda bundle: bundle.update(
                    {"signature_public_key_sha256_pins": ["token-trust-pin-secret"]}
                ),
                "secret-looking material",
                "token-trust-pin-secret",
            ),
            (
                lambda bundle: bundle["x509_crls"][0].update(
                    {"sha256": "token-trust-der-digest-secret"}
                ),
                "secret-looking material",
                "token-trust-der-digest-secret",
            ),
            (
                lambda bundle: bundle["x509_required_certificate_policy_oids"].__setitem__(
                    0,
                    "token-trust-oid-secret",
                ),
                "secret-looking material",
                "token-trust-oid-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"version": "Bearer source-secret"}),
                "secret-looking material",
                "source-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"version": "token=source-secret"}),
                "secret-looking material",
                "source-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"version": "password=source-secret"}),
                "secret-looking material",
                "source-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"version": "token%3Dsource-leak"}),
                "secret-looking material",
                "source-leak",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"version": "%70assword%253Dsource-leak"}
                ),
                "secret-looking material",
                "source-leak",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"authority": "Authorization: Bearer source-secret"}
                ),
                "secret-looking material",
                "source-secret",
            ),
            (
                lambda bundle: bundle["source"].update({"authority": "private_key=source-secret"}),
                "secret-looking material",
                "source-secret",
            ),
            (
                lambda bundle: bundle["source"].update(
                    {"authority": "X-Iroha-Signature: source-secret"}
                ),
                "secret-looking material",
                "source-secret",
            ),
        ]
        for mutate, message, secret in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("password=", stderr)
                    self.assertNotIn("source-secret", stderr)
                    self.assertNotIn("top-level-secret", stderr)

    def test_environment_context_must_be_printable_ascii_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "prepr\u043ed"
            bundle = valid_bundle()
            bundle["environment"] = hidden
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("environment must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)

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

    def test_unused_local_overrides_are_rejected(self):
        cases = (
            (
                "--allow-record-only",
                "--allow-record-only requires at least one bundle with a "
                "non-production embedded_signature_policy",
            ),
            (
                "--allow-insecure-source-url",
                "--allow-insecure-source-url requires at least one bundle with "
                "an http:// source URL",
            ),
            (
                "--allow-synthetic-der",
                "--allow-synthetic-der requires at least one bundle with synthetic DER",
            ),
        )
        for flag, message in cases:
            with self.subTest(flag=flag):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    path = write_bundle(root, valid_bundle())

                    rc, stdout, stderr = run_verify(["--bundle", str(path), flag])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_placeholder_source_summary_only_is_not_profile_emittable(self):
        cases = (
            ("authority", lambda bundle: bundle["source"].__setitem__("authority", "Rail PKI placeholder")),
            ("dummy-authority", lambda bundle: bundle["source"].__setitem__("authority", "Dummy Rail PKI")),
            ("fake-version", lambda bundle: bundle["source"].__setitem__("version", "fake-v1")),
            ("sample-authority", lambda bundle: bundle["source"].__setitem__("authority", "Sample Rail PKI")),
            ("version", lambda bundle: bundle["source"].__setitem__("version", "replace-before-production")),
            ("template-version", lambda bundle: bundle["source"].__setitem__("version", "template-v1")),
            (
                "url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example.invalid/iso20022",
                ),
            ),
            (
                "reserved-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example.com/iso20022",
                ),
            ),
            (
                "reserved-tld-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example/iso20022",
                ),
            ),
            (
                "template-canary-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.operator-canary.bank/iso20022",
                ),
            ),
        )
        for name, mutate in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 0, stderr)
                    summary = json.loads(stdout)
                    self.assertFalse(summary["profile_json_emittable"])
                    self.assertFalse(summary["profile_json_emitted"])

    def test_profile_override_emission_rejects_local_and_placeholder_sources(self):
        cases = (
            (
                "record-only-override",
                lambda _bundle: None,
                ["--allow-record-only"],
                "--allow-record-only cannot be combined with --emit-profile-json",
            ),
            (
                "insecure-source-override",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "http://pki.local/swift-cbpr-plus",
                ),
                ["--allow-insecure-source-url"],
                "--allow-insecure-source-url cannot be combined with --emit-profile-json",
            ),
            (
                "placeholder-authority",
                lambda bundle: bundle["source"].__setitem__(
                    "authority",
                    "Rail PKI placeholder",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "dummy-authority",
                lambda bundle: bundle["source"].__setitem__(
                    "authority",
                    "Dummy Rail PKI",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "fake-version",
                lambda bundle: bundle["source"].__setitem__(
                    "version",
                    "fake-v1",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "sample-authority",
                lambda bundle: bundle["source"].__setitem__(
                    "authority",
                    "Sample Rail PKI",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "placeholder-version",
                lambda bundle: bundle["source"].__setitem__(
                    "version",
                    "replace-before-production",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "template-version",
                lambda bundle: bundle["source"].__setitem__(
                    "version",
                    "template-v1",
                ),
                [],
                "placeholder source metadata",
            ),
            (
                "placeholder-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example.invalid/iso20022",
                ),
                [],
                "reserved placeholder source provenance",
            ),
            (
                "reserved-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example.com/iso20022",
                ),
                [],
                "reserved placeholder source provenance",
            ),
            (
                "reserved-tld-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.example/iso20022",
                ),
                [],
                "reserved placeholder source provenance",
            ),
            (
                "template-canary-url",
                lambda bundle: bundle["source"].__setitem__(
                    "url",
                    "https://pki.swift.operator-canary.bank/iso20022",
                ),
                [],
                "reserved placeholder source provenance",
            ),
        )
        for name, mutate, extra_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--bundle",
                            str(path),
                            "--emit-profile-json",
                            str(root / "profile.json"),
                        ]
                        + extra_args
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_profile_override_emission_requires_source_freshness_budget(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())

            rc, stdout, stderr = run_verify(["--bundle", str(path)])
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertIsNone(summary["max_source_age_days"])
            self.assertFalse(summary["profile_json_emittable"])

            rc, _stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--emit-profile-json",
                    str(root / "profile.json"),
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("--max-source-age-days is required", stderr)

    def test_source_freshness_cli_flag_rejects_missing_empty_or_flag_like_values(self):
        cases = (
            ["--max-source-age-days"],
            ["--max-source-age-days", ""],
            ["--max-source-age-days", "--summary-out"],
            ["--max-source-age-days="],
            ["--max-source-age-days=--summary-out"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn(
                    "--max-source-age-days requires a positive integer value",
                    stderr,
                )

    def test_source_freshness_budget_must_be_positive_integer(self):
        cases = ("0", "-1", "1.5", " 7", "token=trust-secret")
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())
            for value in cases:
                with self.subTest(value=value):
                    rc, _stdout, stderr = run_verify(
                        ["--bundle", str(path), "--max-source-age-days", value]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("--max-source-age-days must be a positive integer", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("trust-secret", stderr)

    def test_source_freshness_budget_rejects_unicode_digits_without_echo(self):
        hidden = "\u0661"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            path = write_bundle(root, valid_bundle())

            rc, _stdout, stderr = run_verify(
                ["--bundle", str(path), "--max-source-age-days", hidden]
            )

            self.assertEqual(rc, 2)
            self.assertIn("--max-source-age-days must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)

    def test_stale_source_prevents_profile_override_emission(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["source"]["retrieved_at"] = "2020-01-01T00:00:00Z"
            path = write_bundle(root, bundle)

            rc, stdout, stderr = run_verify(
                ["--bundle", str(path), "--max-source-age-days", "7"]
            )
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["max_source_age_days"], 7)
            self.assertFalse(summary["profile_json_emittable"])

            rc, _stdout, stderr = run_verify(
                [
                    "--bundle",
                    str(path),
                    "--emit-profile-json",
                    str(root / "profile.json"),
                    "--max-source-age-days",
                    "7",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("source.retrieved_at is older than the 7-day freshness budget", stderr)

    def test_source_provenance_is_required(self):
        cases = [
            (lambda bundle: bundle.pop("source"), ".source must be recorded"),
            (lambda bundle: bundle.__setitem__("source", None), ".source must be a JSON object"),
            (lambda bundle: bundle.update({"source": {}}), ".source.authority"),
            (lambda bundle: bundle["source"].pop("authority"), ".source.authority"),
            (lambda bundle: bundle["source"].pop("version"), ".source.version"),
            (lambda bundle: bundle["source"].pop("retrieved_at"), ".source.retrieved_at"),
            (lambda bundle: bundle["source"].pop("url"), ".source.url"),
        ]
        for mutate, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_boolean_bundle_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["version"] = True
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn(".version must be 1", stderr)

    def test_source_identity_fields_are_required_and_clean(self):
        cases = (
            (
                "authority-null",
                lambda bundle: bundle["source"].__setitem__("authority", None),
                "source.authority must be a non-empty string",
            ),
            (
                "authority-empty",
                lambda bundle: bundle["source"].__setitem__("authority", ""),
                "source.authority must be a non-empty string",
            ),
            (
                "authority-numeric",
                lambda bundle: bundle["source"].__setitem__("authority", 7),
                "source.authority must be a non-empty string",
            ),
            (
                "authority-control",
                lambda bundle: bundle["source"].__setitem__(
                    "authority",
                    "Example\nRail PKI",
                ),
                "source.authority must not contain ASCII control characters",
            ),
            (
                "version-null",
                lambda bundle: bundle["source"].__setitem__("version", None),
                "source.version must be a non-empty string",
            ),
            (
                "version-empty",
                lambda bundle: bundle["source"].__setitem__("version", ""),
                "source.version must be a non-empty string",
            ),
            (
                "version-numeric",
                lambda bundle: bundle["source"].__setitem__("version", 2026),
                "source.version must be a non-empty string",
            ),
            (
                "version-control",
                lambda bundle: bundle["source"].__setitem__("version", "2026-Q2\n"),
                "source.version must not contain ASCII control characters",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_overlong_source_identity_values_are_rejected_without_echo(self):
        hidden = "A" * (VERIFIER.MAX_TRUST_SOURCE_TEXT_CHARS + 1)
        cases = (
            (
                "authority",
                lambda bundle: bundle["source"].__setitem__("authority", hidden),
                "source.authority must be no longer than 256 characters",
            ),
            (
                "version",
                lambda bundle: bundle["source"].__setitem__("version", hidden),
                "source.version must be no longer than 256 characters",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    mutate(bundle)
                    path = write_bundle(root, bundle)

                    rc, stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_source_url_rejects_credentials_query_fragment_and_local_addresses(self):
        long_host = ".".join(["a" * 63] * 4)
        long_url = "https://pki.example.invalid/" + ("a" * VERIFIER.MAX_SOURCE_URL_CHARS)
        cases = [
            ("https://user:pass@pki.example.invalid/swift-cbpr-plus", "credentials"),
            ("https://pki.example.invalid/swift-cbpr-plus?debug=true", "params, query, or fragment"),
            ("https://pki.example.invalid/swift-cbpr-plus#bundle", "params, query, or fragment"),
            ("https://pki.example.invalid/swift-cbpr-plus\nbad", "ASCII control"),
            ("https://pki.example.invalid/swift cbpr plus", "must not contain whitespace"),
            ("https://pki.example.invalid:abc/swift-cbpr-plus", "invalid port"),
            ("https://pki.example.invalid:/swift-cbpr-plus", "empty port"),
            ("https://pki.example.invalid:0/swift-cbpr-plus", "port must be positive"),
            ("https://pki.example.invalid:08443/swift-cbpr-plus", "leading zeros"),
            ("https://pki.example.invalid:99999/swift-cbpr-plus", "invalid port"),
            ("https://pki.example.invalid:443/swift-cbpr-plus", "default port"),
            (long_url, "no longer than 2048 characters"),
            ("https://PKI.example.invalid/swift-cbpr-plus", "host must be lowercase"),
            ("https://pki.example.invalid./swift-cbpr-plus", "host must not end with a dot"),
            ("https://pki..example.invalid/swift-cbpr-plus", "host must not contain empty labels"),
            (f"https://{long_host}/swift-cbpr-plus", "host must be at most 253 characters"),
            ("https://-pki.example.invalid/swift-cbpr-plus", "host labels must not start or end with hyphen"),
            ("https://pki._tcp.example.invalid/swift-cbpr-plus", "host labels must use lowercase ASCII letters, digits, or hyphens"),
            ("https://pki.example%2einvalid/swift-cbpr-plus", "host must not contain percent escapes"),
            ("https://123.000.000.001/swift-cbpr-plus", "numeric host labels must be a valid IP address"),
            ("https://pki.example.invalid/../swift-cbpr-plus", "path must not contain dot segments"),
            ("https://pki.example.invalid/swift//cbpr-plus", "path must not contain empty segments"),
            ("https://pki.example.invalid/%2e%2e/swift-cbpr-plus", "path must not contain encoded dot or separator characters"),
            ("https://pki.example.invalid/swift%2fcbpr-plus", "path must not contain encoded dot or separator characters"),
            ("https://pki.example.invalid/swift%252fcbpr-plus", "path must not contain encoded percent characters"),
            ("https://pki.example.invalid/sources;debug/swift-cbpr-plus", "path must not contain semicolon parameters"),
            ("https://pki.example.invalid/sources%3bdebug/swift-cbpr-plus", "path must not contain encoded semicolon parameters"),
            ("https://pki.example.invalid/sources%23debug/swift-cbpr-plus", "path must not contain encoded URL delimiter characters"),
            (r"https://pki.example.invalid/sources\swift-cbpr-plus", "path must use forward slashes"),
            ("https://pki.example.invalid/swift%20cbpr-plus", "percent-encoded control or space characters"),
            ("https://pki.example.invalid/swift%00cbpr-plus", "percent-encoded control or space characters"),
            ("https://pki.example.invalid/swift%7fcbpr-plus", "percent-encoded control or space characters"),
            ("https://pki.example.invalid/swift%zzcbpr-plus", "malformed percent escapes"),
            ("https://[::1", "malformed"),
            ("https:///swift-cbpr-plus", "include a host"),
            ("https://localhost/swift-cbpr-plus", "localhost"),
            ("https://127.0.0.1/swift-cbpr-plus", "local, private, or reserved IP"),
            ("https://10.1.2.3/swift-cbpr-plus", "local, private, or reserved IP"),
            ("https://[::1]/swift-cbpr-plus", "local, private, or reserved IP"),
            ("https://127.0.0.1.nip.io/swift-cbpr-plus", "rebinding hostnames"),
            ("https://0x7f000001/swift-cbpr-plus", "legacy IPv4 numeric notation"),
            ("https://[64:ff9b::7f00:1]/swift-cbpr-plus", "embed local, private, or reserved IPv4"),
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

    def test_rejected_source_url_does_not_echo_secret_query(self):
        cases = (
            "https://pki.example.invalid/swift-cbpr-plus?token=trust-url-secret",
            "https://pki.example.invalid/swift-cbpr-plus/token=trust-url-secret",
            "https://pki.example.invalid/swift-cbpr-plus/token-trust-url-secret",
            "https://pki.example.invalid/swift-cbpr-plus/token%3Dtrust-url-secret",
            "https://pki.example.invalid/swift-cbpr-plus/token%253Dtrust-url-secret",
        )
        for secret_url in cases:
            with self.subTest(secret_url=secret_url):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle["source"]["url"] = secret_url
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("trust-url-secret", stderr)

    def test_rejected_source_url_does_not_echo_secret_port(self):
        secret_url = "https://pki.example.invalid:token-trust-port-secret/swift-cbpr-plus"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["source"]["url"] = secret_url
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("invalid port", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token-trust-port-secret", stderr)

    def test_rejected_source_url_does_not_echo_secret_host_or_parser_error(self):
        cases = (
            (
                "https://token-trust-host-secret.pki.example/swift-cbpr-plus",
                "secret-looking material",
            ),
            ("https://[token-trust-host-secret/swift-cbpr-plus", "malformed"),
        )
        for secret_url, message in cases:
            with self.subTest(secret_url=secret_url):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    bundle = valid_bundle()
                    bundle["source"]["url"] = secret_url
                    path = write_bundle(root, bundle)

                    rc, _stdout, stderr = run_verify(["--bundle", str(path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token-trust-host-secret", stderr)

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
            ("2026-06-04T00:00:00Z\nbad", "ASCII control"),
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

    def test_overlong_source_retrieved_at_is_rejected_without_echo(self):
        hidden = "2" * (VERIFIER.MAX_TIMESTAMP_CHARS + 1)
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle["source"]["retrieved_at"] = hidden
            path = write_bundle(root, bundle)

            rc, stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("source.retrieved_at must be no longer than 128 characters", stderr)
            self.assertNotIn(hidden, stderr)

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
            self.assertIsNone(summary["profile_json_sha256"])
            self.assertNotIn("_uses_synthetic_der", json.dumps(summary))

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

    def test_embedded_signature_policy_must_be_explicit(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            bundle = valid_bundle()
            bundle.pop("embedded_signature_policy")
            path = write_bundle(root, bundle)

            rc, _stdout, stderr = run_verify(["--bundle", str(path)])

            self.assertEqual(rc, 2)
            self.assertIn("embedded_signature_policy must be recorded", stderr)

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
        self.assertFalse(summary["profile_json_emitted"])
        self.assertIsNone(summary["profile_json_sha256"])
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
