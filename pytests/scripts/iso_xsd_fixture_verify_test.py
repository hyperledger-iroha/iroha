import argparse
import base64
import contextlib
import importlib.util
import io
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_xsd_fixture_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_xsd_fixture_verify", SCRIPT_PATH)
VERIFIER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = VERIFIER
SPEC.loader.exec_module(VERIFIER)


def xsd_text(message_id, payload_root, *, target_message_id=None, element_form="qualified"):
    target = target_message_id or message_id
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns="urn:iso:std:iso:20022:tech:xsd:{target}"
           xmlns:xs="http://www.w3.org/2001/XMLSchema"
           elementFormDefault="{element_form}"
           targetNamespace="urn:iso:std:iso:20022:tech:xsd:{target}">
  <xs:element name="Document" type="Document"/>
  <xs:complexType name="Document">
    <xs:sequence>
      <xs:element name="{payload_root}" type="{payload_root}"/>
    </xs:sequence>
  </xs:complexType>
  <xs:complexType name="{payload_root}">
    <xs:sequence/>
  </xs:complexType>
</xs:schema>
"""


def fixture_xml(message_id, payload_root, *, root="Document", payload_namespace=None):
    namespace = f"urn:iso:std:iso:20022:tech:xsd:{message_id}"
    child_namespace = payload_namespace or namespace
    return (
        f'<doc:{root} xmlns:doc="{namespace}" xmlns:payload="{child_namespace}">'
        f"<payload:{payload_root}/>"
        f"</doc:{root}>\n"
    )


def source_provenance(message_id, payload_root):
    schema_text = xsd_text(message_id, payload_root)
    return {
        "repository": "https://github.com/moov-io/fedwire20022",
        "commit": "0123456789abcdef0123456789abcdef01234567",
        "path": f"xsd/iso/{message_id}.xsd",
        "license": "Apache-2.0",
        "sha256": VERIFIER.sha256_hex(schema_text.encode("utf-8")),
    }


def blocked_schema_source(message_id="barr.001.001.01"):
    return {
        "message_def_id": message_id,
        "source": {
            "repository": "https://github.com/prog-nov/iso20022-messages-for-go",
            "commit": "89abcdef0123456789abcdef0123456789abcdef",
            "path": f"xsd/{message_id}.xsd",
            "sha256": "1" * 64,
        },
        "reason": "Candidate carries redistribution restrictions.",
        "restriction_markers": [
            "swift-copyright-header",
            "licensed-product-redistribution-agreement",
        ],
    }


def pending_schema_source(message_id="barr.001.001.01"):
    return {
        "message_def_id": message_id,
        "source": {
            "catalogue_url": "https://www.iso20022.org/iso-20022-message-definitions",
            "download_url": "https://www.iso20022.org/message/12345/download",
            "download_type": "XSD",
            "message_name": "BarPayloadV01",
            "submitting_organisation": "SWIFT",
        },
        "reason": "Official ISO catalogue lists an XSD download.",
    }


def known_pending_schema_source(message_id="colr.012.001.05"):
    return {
        "message_def_id": message_id,
        "source": dict(VERIFIER.KNOWN_PENDING_SCHEMA_SOURCE_METADATA[message_id]),
        "reason": "Official ISO catalogue lists an XSD download.",
    }


def rewrite_schema(root, message_id, content, *, manifest_path=None):
    schema_path = root / "xsd" / "iso" / f"{message_id}.xsd"
    schema_path.write_text(content, encoding="utf-8")
    if manifest_path is not None:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["schemas"][0]["source"]["sha256"] = VERIFIER.sha256_hex(
            content.encode("utf-8")
        )
        write_json(manifest_path, manifest)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def write_profile_catalog(path, versions=None, catalog=None):
    versions = versions or ["fooo.001.001.01"]
    if catalog is None:
        catalog = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": versions,
                    }
                ],
            }
        ]
    for profile in catalog:
        if isinstance(profile, dict):
            profile.setdefault("rail", "generic-iso20022")
            profile.setdefault("embedded_signature_policy", "record-only")
            for message in profile.get("message_profiles", []):
                if isinstance(message, dict):
                    message.setdefault("structured_address_mode", "permissive")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        'const DEFAULT_PROFILES_JSON: &str = r#"\n'
        + json.dumps(catalog, indent=2, sort_keys=True)
        + '\n"#;\n',
        encoding="utf-8",
    )
    return path


def write_minimal_tree(root, manifest):
    xsd_dir = root / "xsd"
    schema_dir = xsd_dir / "iso"
    schema_dir.mkdir(parents=True)
    fixture_path = root / "foo_fixture.xml"
    message_id = "fooo.001.001.01"
    payload_root = "FooPayload"
    (schema_dir / f"{message_id}.xsd").write_text(
        xsd_text(message_id, payload_root),
        encoding="utf-8",
    )
    fixture_path.write_text(fixture_xml(message_id, payload_root), encoding="utf-8")
    manifest_path = xsd_dir / "fixture_manifest.json"
    write_json(manifest_path, manifest)
    return manifest_path


def minimal_manifest():
    return {
        "version": 1,
        "schemas": [
            {
                "path": "iso/fooo.001.001.01.xsd",
                "message_def_id": "fooo.001.001.01",
                "payload_root": "FooPayload",
                "source": source_provenance("fooo.001.001.01", "FooPayload"),
            }
        ],
        "fixtures": [
            {
                "path": "../foo_fixture.xml",
                "message_def_id": "fooo.001.001.01",
                "payload_root": "FooPayload",
                "schema": "iso/fooo.001.001.01.xsd",
            }
        ],
        "blocked_schema_sources": [],
        "pending_schema_sources": [],
    }


def run_verify(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = VERIFIER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoXsdFixtureVerifyTest(unittest.TestCase):
    def test_checked_in_pending_schema_sources_are_exact_metadata_pinned(self):
        manifest = json.loads(VERIFIER.DEFAULT_MANIFEST.read_text(encoding="utf-8"))
        pending_entries = manifest["pending_schema_sources"]
        pending_ids = {entry["message_def_id"] for entry in pending_entries}

        self.assertEqual(
            pending_ids,
            set(VERIFIER.KNOWN_PENDING_SCHEMA_SOURCE_METADATA),
        )
        for entry in pending_entries:
            message_def_id = entry["message_def_id"]
            self.assertEqual(
                entry["source"],
                VERIFIER.KNOWN_PENDING_SCHEMA_SOURCE_METADATA[message_def_id],
            )

    def test_json_arrays_are_count_bounded_without_echo(self):
        items = [None] * (VERIFIER.MAX_JSON_ARRAY_ITEMS + 1)

        with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
            VERIFIER._require_array(items, "manifest.schemas")

        error = str(caught.exception)
        self.assertIn(
            f"manifest.schemas must contain at most {VERIFIER.MAX_JSON_ARRAY_ITEMS} items",
            error,
        )
        self.assertNotIn(str(len(items)), error)

    def test_recursive_json_array_scans_are_count_bounded_without_echo(self):
        items = [None] * (VERIFIER.MAX_JSON_ARRAY_ITEMS + 1)
        cases = (
            (
                "surrogates",
                lambda: VERIFIER._reject_json_surrogates(items),
                f"JSON array must contain at most {VERIFIER.MAX_JSON_ARRAY_ITEMS} items",
            ),
            (
                "secret scan",
                lambda: VERIFIER._check_no_secret_material(items, "manifest.extra"),
                f"manifest.extra must contain at most {VERIFIER.MAX_JSON_ARRAY_ITEMS} items",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    action()

                error = str(caught.exception)
                self.assertIn(expected, error)
                self.assertNotIn(str(len(items)), error)
                self.assertNotIn("[0]", error)

    def test_recursive_json_object_scans_are_count_bounded_without_echo(self):
        members = {
            f"hidden_key_{offset}": None
            for offset in range(VERIFIER.MAX_JSON_OBJECT_MEMBERS + 1)
        }
        pairs = list(members.items())
        cases = (
            (
                "json hook",
                lambda: VERIFIER._reject_duplicate_json_keys(pairs),
                f"JSON object must contain at most {VERIFIER.MAX_JSON_OBJECT_MEMBERS} members",
            ),
            (
                "surrogates",
                lambda: VERIFIER._reject_json_surrogates(members),
                f"JSON object must contain at most {VERIFIER.MAX_JSON_OBJECT_MEMBERS} members",
            ),
            (
                "secret scan",
                lambda: VERIFIER._check_no_secret_material(members, "manifest.extra"),
                f"manifest.extra must contain at most {VERIFIER.MAX_JSON_OBJECT_MEMBERS} object members",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    action()

                error = str(caught.exception)
                self.assertIn(expected, error)
                self.assertNotIn(str(len(members)), error)
                self.assertNotIn("hidden_key_0", error)

    def test_recursive_json_depth_scans_are_bounded_without_echo(self):
        nested = "hidden_leaf"
        for _ in range(VERIFIER.MAX_JSON_NESTING_DEPTH + 1):
            nested = [nested]
        expected = (
            f"JSON nesting depth must be at most {VERIFIER.MAX_JSON_NESTING_DEPTH} levels"
        )
        cases = (
            ("surrogates", lambda: VERIFIER._reject_json_surrogates(nested)),
            ("secret scan", lambda: VERIFIER._check_no_secret_material(nested, "manifest.extra")),
        )
        for name, action in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    action()

                error = str(caught.exception)
                self.assertIn(expected, error)
                self.assertNotIn("hidden_leaf", error)
                self.assertNotIn("[0]", error)

    def test_json_parse_recursion_error_is_bounded_without_echo(self):
        hidden = "hidden-xsd-recursion"
        original_loads = VERIFIER.json.loads

        def raising_loads(*_args, **_kwargs):
            raise RecursionError(hidden)

        VERIFIER.json.loads = raising_loads
        try:
            with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                VERIFIER._load_json_bytes(b"[]", Path(hidden), display_label="manifest")
        finally:
            VERIFIER.json.loads = original_loads

        error = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {VERIFIER.MAX_JSON_NESTING_DEPTH} levels",
            error,
        )
        self.assertNotIn(hidden, error)

    def test_direct_run_policy_flags_must_be_booleans_before_manifest_loading(self):
        cases = (
            (
                "require_schema_backed_fixtures",
                "--require-schema-backed-fixtures",
                "true",
            ),
            ("require_fixture_for_schema", "--require-fixture-for-schema", 1),
            (
                "require_profile_schema_backed_versions",
                "--require-profile-schema-backed-versions",
                None,
            ),
            ("validate_xml_schema", "--validate-xml-schema", []),
        )
        for attr, label, value in cases:
            with self.subTest(flag=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        manifest=root / "missing-fixture-manifest.json",
                        summary_out=None,
                        require_schema_backed_fixtures=False,
                        require_fixture_for_schema=False,
                        profile_catalog=None,
                        require_profile_schema_backed_versions=False,
                        validate_xml_schema=False,
                        xmllint_timeout_secs=1.0,
                    )
                    setattr(args, attr, value)

                    with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                        VERIFIER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a boolean", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_scalar_paths_must_be_paths_before_manifest_loading(self):
        cases = (
            ("manifest", "manifest", object(), "--manifest"),
            ("summary", "summary_out", object(), "summary_out"),
            ("profile catalog", "profile_catalog", object(), "--profile-catalog"),
        )
        for name, field, value, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        manifest=root / "missing-fixture-manifest.json",
                        summary_out=None,
                        require_schema_backed_fixtures=False,
                        require_fixture_for_schema=False,
                        profile_catalog=None,
                        require_profile_schema_backed_versions=False,
                        validate_xml_schema=False,
                        xmllint_timeout_secs=1.0,
                    )
                    setattr(args, field, value)

                    with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                        VERIFIER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a path", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_text_output_symlink_ancestor_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = root / "target"
            target.mkdir()
            hidden = "hidden-xsd-output-link"
            link = root / hidden
            try:
                link.symlink_to(target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                VERIFIER._write_text_output(
                    link / "summary.json",
                    "{}\n",
                    display_label="summary_out",
                )

            message = str(caught.exception)
            self.assertIn("summary_out", message)
            self.assertIn("must not be a symlink", message)
            self.assertNotIn(str(link), message)
            self.assertNotIn(hidden, message)

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_xsd_unknown_secret", "xsd_unknown_secret"),
            ("%70assword_xsd_unknown_leak", "xsd_unknown_leak"),
            ("private-key_xsd_unknown_leak", "xsd_unknown_leak"),
            ("private--key_xsd_unknown_leak", "xsd_unknown_leak"),
            ("private%09key_xsd_unknown_leak", "xsd_unknown_leak"),
            ("x--iroha--signature_xsd_unknown_leak", "xsd_unknown_leak"),
            ("unexpected\x1bxsd_key", "\x1b"),
            ("unexpected_xsd_\uff4bey", "\uff4b"),
            ("operator_note", "operator_note"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    VERIFIER._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "manifest"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
            VERIFIER._reject_unknown_keys(many_unknown, set(), "manifest")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_separator_smuggled_secret_identifiers_are_detected(self):
        cases = (
            "private\tkey xsd identifier",
            "private--key xsd identifier",
            "private/key xsd identifier",
            "private\\key xsd identifier",
            "private%2fkey xsd identifier",
            "private\u200dkey xsd identifier",
            "private\u0301key xsd identifier",
            "ｐｒｉｖａｔｅｋｅｙ xsd identifier",
            "x--iroha--signature xsd identifier",
            "x/iroha/signature xsd identifier",
            "x%2firoha%2fsignature xsd identifier",
            "x\u200diroha\u200dsignature xsd identifier",
            "x\u0301iroha\u0301signature xsd identifier",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ xsd identifier",
            "token%09secret xsd identifier",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(VERIFIER._contains_secret_identifier_material(value))
        for key in (
            "private/key",
            "private%2fkey",
            "private\u0301key",
            "ｐｒｉｖａｔｅｋｅｙ",
            "x/iroha/signature",
            "x%2firoha%2fsignature",
            "x\u0301iroha\u0301signature",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ",
        ):
            with self.subTest(key=key):
                self.assertTrue(VERIFIER._is_secret_looking_key(key))

    def test_path_separator_secret_key_values_are_detected(self):
        cases = (
            "private/key=xsd-value-secret",
            "api/key:xsd-value-secret",
            "client/secret=xsd-value-secret",
            "set/cookie:xsd-value-secret",
            "x/iroha/signature: xsd-value-secret",
            "private%2fkey=xsd-value-secret",
            "private\u200dkey=xsd-value-secret",
            "private\u0301key=xsd-value-secret",
            "ｐｒｉｖａｔｅｋｅｙ=xsd-compat-secret",
            "ａｐｉ／ｋｅｙ:xsd-compat-secret",
            "x\u200diroha\u200dsignature: xsd-value-secret",
            "x\u0301iroha\u0301signature: xsd-value-secret",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ: xsd-compat-secret",
            "private%E2%80%8Dkey=xsd-value-secret",
            "private%CC%81key=xsd-value-secret",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(VERIFIER._contains_secret_material(value))

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=xsd-terminator-secret"
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
                    ["--", "--validate-xml-schema", hidden],
                    {"--validate-xml-schema"},
                ),
            ),
            (
                "numeric",
                lambda: VERIFIER._preflight_numeric_cli_values(
                    ["--", "--xmllint-timeout-secs", hidden],
                    integer_flags=set(),
                    number_flags={"--xmllint-timeout-secs"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("xsd-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                VERIFIER.build_parser().parse_args(["--summary-ou", "out"])

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--summary-ou", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        cases = (
            ("--unknown-xsd\x1bflag", "\x1b"),
            ("--unknown-xsd\u202eflag", "\u202e"),
        )
        for hidden, marker in cases:
            with self.subTest(hidden=hidden):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    VERIFIER._preflight_raw_cli_secrets([hidden], {"--summary-out"})

                message = str(caught.exception)
                self.assertIn("CLI argument must not contain control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(marker, message)
                self.assertNotIn("unknown-xsd", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dsummary-out"
        with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
            VERIFIER._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("summary-out", message)

    def test_nested_control_material_in_manifest_is_rejected_without_echo(self):
        cases = (
            (
                {"metadata": {"unexpected\x1bxsd_key": "redacted"}},
                "forbidden control-bearing field",
                "xsd_key",
            ),
            (
                {"metadata": {"unexpected\u202exsd_key": "redacted"}},
                "forbidden control-bearing field",
                "xsd_key",
            ),
            (
                {"metadata": {"note": "warning \x1b[31mred"}},
                "unsafe control characters",
                "[31mred",
            ),
            (
                {"metadata": {"note": "warning \u202exsd-bidi-leak"}},
                "unsafe control characters",
                "xsd-bidi-leak",
            ),
            (
                {"metadata": {"note": "private%E2%80%8Dkey=xsd-field-leak"}},
                "secret-looking material",
                "xsd-field-leak",
            ),
            (
                {"metadata": {"note": "private%CC%81key=xsd-mark-leak"}},
                "secret-looking material",
                "xsd-mark-leak",
            ),
            (
                {"metadata": {"note": "ｐｒｉｖａｔｅｋｅｙ=xsd-compat-leak"}},
                "secret-looking material",
                "xsd-compat-leak",
            ),
        )
        for body, expected, hidden in cases:
            with self.subTest(body=body):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    VERIFIER._check_no_secret_material(body)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn("\u202e", message)
                self.assertNotIn(hidden, message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--summary-out"],
            ["--summary-out", ""],
            ["--summary-out", "--require-fixture-for-schema"],
            ["--summary-out="],
            ["--summary-out=--require-fixture-for-schema"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    VERIFIER.FixtureManifestError,
                    "--summary-out requires a path value",
                ):
                    VERIFIER._preflight_output_cli_paths(argv, {"--summary-out"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=xsd-path-leak.summary.json", "token=xsd-path-leak"),
            ("token%3Dxsd-path-leak.summary.json", "token=xsd-path-leak"),
            ("private%20key%3Dxsd-path-leak.summary.json", "private key=xsd-path-leak"),
            ("private%20key-xsd-path-secret.summary.json", "private key-xsd-path-secret"),
            ("private/key-xsd-path-secret.summary.json", "private/key-xsd-path-secret"),
            ("x%2firoha%2fsignature-xsd-path-secret.summary.json", "x/iroha/signature-xsd-path-secret"),
            ("%70assword%253Dxsd-path-leak.summary.json", "password=xsd-path-leak"),
            ("token-xsd-path-secret.summary.json", "token-xsd-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    VERIFIER._preflight_output_cli_paths(
                        ["--summary-out", raw_path], {"--summary-out"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("xsd-path-leak", message)

    def test_summary_output_rejects_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "xsd" / "summary.json"

            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                VERIFIER._write_text_output(output_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())
            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                VERIFIER._reject_repository_output_path(
                    Path("fixtures/iso20022/xsd/summary.json"),
                    "output path",
                )

    def test_summary_output_rejects_repository_fixture_before_manifest_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "xsd" / "summary.json"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(root / "missing-manifest.json"),
                    "--summary-out",
                    str(output_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "summary_out must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((root / "fixtures").exists())

    def test_missing_summary_output_parent_is_not_created_before_manifest_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "fixture_manifest.json"
            manifest_path.write_text("{not valid manifest json\n", encoding="utf-8")
            summary_parent = root / "summary" / "new"
            summary_out = summary_parent / "xsd-summary.json"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("not valid JSON", stderr)
            self.assertFalse(summary_parent.exists())

    def test_summary_output_cannot_reuse_manifest_or_profile_catalog_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root / "xsd", minimal_manifest())
            original_manifest = manifest_path.read_text(encoding="utf-8")
            profile_catalog = root / "profiles.rs"
            profile_catalog.write_text("not a profile catalog\n", encoding="utf-8")

            cases = (
                (
                    "manifest",
                    str(manifest_path),
                    ["--manifest", str(manifest_path), "--summary-out", str(manifest_path)],
                    "summary_out must not reuse --manifest path",
                ),
                (
                    "profile-catalog",
                    str(profile_catalog),
                    [
                        "--manifest",
                        str(manifest_path),
                        "--profile-catalog",
                        str(profile_catalog),
                        "--summary-out",
                        str(profile_catalog),
                    ],
                    "summary_out must not reuse --profile-catalog path",
                ),
            )
            for name, hidden_path, argv, message in cases:
                with self.subTest(name=name):
                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden_path, stderr)
                    self.assertEqual(manifest_path.read_text(encoding="utf-8"), original_manifest)
                    self.assertEqual(
                        profile_catalog.read_text(encoding="utf-8"),
                        "not a profile catalog\n",
                    )

    def test_summary_output_cannot_hardlink_manifest(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root / "xsd", minimal_manifest())
            summary_out = root / "manifest-hardlink.summary.json"
            try:
                os.link(manifest_path, summary_out)
            except OSError as error:
                self.skipTest(f"hardlink creation unavailable: {error}")
            original_manifest = manifest_path.read_text(encoding="utf-8")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("summary_out must not reuse --manifest path", stderr)
            self.assertNotIn(str(manifest_path), stderr)
            self.assertNotIn(str(summary_out), stderr)
            self.assertEqual(manifest_path.read_text(encoding="utf-8"), original_manifest)

    def test_summary_output_cannot_reuse_discovered_schema_or_fixture_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root / "xsd", minimal_manifest())
            schema_path = root / "xsd" / "xsd" / "iso" / "fooo.001.001.01.xsd"
            fixture_path = root / "xsd" / "foo_fixture.xml"
            cases = (
                (
                    "schema",
                    schema_path,
                    "summary_out must not reuse manifest.schemas[0].path path",
                ),
                (
                    "fixture",
                    fixture_path,
                    "summary_out must not reuse manifest.fixtures[0].path path",
                ),
            )
            for name, output_path, message in cases:
                with self.subTest(name=name):
                    original = output_path.read_text(encoding="utf-8")

                    rc, stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--summary-out",
                            str(output_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertEqual(output_path.read_text(encoding="utf-8"), original)

    def test_summary_output_cannot_hardlink_discovered_schema(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root / "xsd", minimal_manifest())
            schema_path = root / "xsd" / "xsd" / "iso" / "fooo.001.001.01.xsd"
            summary_out = root / "schema-hardlink.summary.json"
            try:
                os.link(schema_path, summary_out)
            except OSError as error:
                self.skipTest(f"hardlink creation unavailable: {error}")
            original_schema = schema_path.read_text(encoding="utf-8")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("summary_out must not be hard-linked", stderr)
            self.assertNotIn(str(schema_path), stderr)
            self.assertNotIn(str(summary_out), stderr)
            self.assertEqual(schema_path.read_text(encoding="utf-8"), original_schema)
            self.assertEqual(summary_out.read_text(encoding="utf-8"), original_schema)

    def test_profile_catalog_rejects_repository_fixture_before_manifest_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            profile_catalog = root / "fixtures" / "iso20022" / "profiles.rs"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(root / "missing-manifest.json"),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--profile-catalog must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((root / "fixtures").exists())

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
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
                    lambda raw: VERIFIER._reject_output_path_smuggling(
                        Path(raw),
                        "output path",
                    ),
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
                    lambda raw: VERIFIER._reject_output_path_smuggling(
                        Path(raw),
                        "output path",
                    ),
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
                    "source drive prefix",
                    lambda raw: VERIFIER._validate_source_path(raw, "source.path"),
                    "C:/schemas/camt.052.xsd",
                    "URI or drive prefixes",
                ),
                (
                    "source encoded dot",
                    lambda raw: VERIFIER._validate_source_path(raw, "source.path"),
                    "schemas/camt%2e.052.xsd",
                    "encoded dot or separator",
                ),
                (
                    "relative encoded semicolon",
                    lambda raw: VERIFIER._validate_relative_path(
                        raw,
                        root,
                        root,
                        "fixture.path",
                        allow_parent_segments=False,
                    ),
                    "fixtures/%3b/pacs.xml",
                    "encoded semicolon",
                ),
                (
                    "raw encoded delimiter",
                    lambda raw: VERIFIER._reject_raw_output_path_smuggling(raw, "raw path"),
                    "out/%3f/summary.json",
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
                    with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                        call(raw)

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn(raw, message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--validate-xml-schema=true"], "--validate-xml-schema", "--validate-xml-schema=true"),
            (
                ["--require-profile-schema-backed-versions", "true"],
                "--require-profile-schema-backed-versions",
                "true",
            ),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_numeric_cli_flags_reject_malformed_values_without_echo(self):
        cases = (
            ["--xmllint-timeout-secs", "token=xsd-secret"],
            ["--xmllint-timeout-secs=token=xsd-secret"],
            ["--xmllint-timeout-secs", "--summary-out"],
            ["--xmllint-timeout-secs="],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("xsd-secret", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--xmllint-timeout-secs", hidden],
            [f"--xmllint-timeout-secs={hidden}.5"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=xsd-secret"],
            ["token=xsd-secret"],
            ["password=xsd-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_verify(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("xsd-secret", stderr)

    def test_profile_catalog_secret_material_is_rejected_without_echo(self):
        def base_catalog():
            return [
                {
                    "id": "minimal-profile",
                    "rail": "generic-iso20022",
                    "embedded_signature_policy": "record-only",
                    "required_reference_datasets": [],
                    "message_profiles": [
                        {
                            "message_type": "fooo.001",
                            "direction": "inbound",
                            "versions": ["fooo.001.001.01"],
                            "structured_address_mode": "permissive",
                        }
                    ],
                }
            ]

        cases = (
            ("rail", lambda catalog: catalog[0].__setitem__("rail", "token=xsd-profile-secret")),
            (
                "policy",
                lambda catalog: catalog[0].__setitem__(
                    "embedded_signature_policy",
                    "%70assword%253Dxsd-profile-secret",
                ),
            ),
            (
                "dataset",
                lambda catalog: catalog[0].__setitem__(
                    "required_reference_datasets",
                    ["client_secret=xsd-profile-secret"],
                ),
            ),
            (
                "address",
                lambda catalog: catalog[0]["message_profiles"][0].__setitem__(
                    "structured_address_mode",
                    "api_key=xsd-profile-secret",
                ),
            ),
            (
                "version",
                lambda catalog: catalog[0]["message_profiles"][0].__setitem__(
                    "versions",
                    ["session_key=xsd-profile-secret"],
                ),
            ),
        )
        for name, mutate in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    catalog = base_catalog()
                    mutate(catalog)
                    profile_catalog = write_profile_catalog(root / "profiles.rs", catalog=catalog)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--profile-catalog",
                            str(profile_catalog),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("password=", stderr)
                    self.assertNotIn("client_secret=", stderr)
                    self.assertNotIn("api_key=", stderr)
                    self.assertNotIn("session_key=", stderr)
                    self.assertNotIn("xsd-profile-secret", stderr)

    def test_profile_catalog_secret_looking_identifiers_are_rejected_without_echo(self):
        catalog = [
            {
                "id": "token_xsd_profile_secret",
                "rail": "generic-iso20022",
                "embedded_signature_policy": "record-only",
                "required_reference_datasets": [],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": "permissive",
                    }
                ],
            }
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs", catalog=catalog)

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token_xsd_profile_secret", stderr)

    def test_profile_catalog_overlong_id_is_rejected_without_echo(self):
        hidden = "a" * (VERIFIER.MAX_PROFILE_CATALOG_IDENTIFIER_CHARS + 1)
        catalog = [
            {
                "id": hidden,
                "rail": "generic-iso20022",
                "embedded_signature_policy": "record-only",
                "required_reference_datasets": [],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": "permissive",
                    }
                ],
            },
            {
                "id": hidden,
                "rail": "generic-iso20022",
                "embedded_signature_policy": "record-only",
                "required_reference_datasets": [],
                "message_profiles": [
                    {
                        "message_type": "fooo.002",
                        "direction": "inbound",
                        "versions": ["fooo.002.001.01"],
                        "structured_address_mode": "permissive",
                    }
                ],
            },
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs", catalog=catalog)

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "profiles[0].id must be no longer than 128 characters",
                stderr,
            )
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("duplicates profile id", stderr)

    def test_profile_catalog_overlong_business_services_are_rejected_without_echo(self):
        hidden = "service." + ("a" * VERIFIER.MAX_PROFILE_CATALOG_IDENTIFIER_CHARS)
        catalog = [
            {
                "id": "minimal-profile",
                "rail": "generic-iso20022",
                "embedded_signature_policy": "record-only",
                "required_reference_datasets": [],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": "permissive",
                        "business_services": [hidden],
                        "require_app_header": True,
                        "require_business_service": True,
                    }
                ],
            }
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs", catalog=catalog)

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "business_services[0] must be no longer than 128 characters",
                stderr,
            )
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("business_services must not be empty", stderr)

    def test_profile_catalog_non_ascii_enum_values_are_rejected_without_echo(self):
        cases = (
            (
                "rail",
                "generic-iso2002\u0433",
                lambda catalog, value: catalog[0].__setitem__("rail", value),
            ),
            (
                "embedded-policy",
                "record-onl\u0443",
                lambda catalog, value: catalog[0].__setitem__(
                    "embedded_signature_policy",
                    value,
                ),
            ),
            (
                "reference-dataset",
                "bic-director\u0443",
                lambda catalog, value: catalog[0].__setitem__(
                    "required_reference_datasets",
                    [value],
                ),
            ),
            (
                "structured-address-mode",
                "permiss\u0456ve",
                lambda catalog, value: catalog[0]["message_profiles"][0].__setitem__(
                    "structured_address_mode",
                    value,
                ),
            ),
            (
                "message-type",
                "fooo.\u0660\u0660\u0661",
                lambda catalog, value: catalog[0]["message_profiles"][0].__setitem__(
                    "message_type",
                    value,
                ),
            ),
            (
                "message-def-id",
                "fooo.\u0660\u0660\u0661.001.01",
                lambda catalog, value: catalog[0]["message_profiles"][0].__setitem__(
                    "versions",
                    [value],
                ),
            ),
            (
                "business-service",
                "swift.cbprplus.\u043e2",
                lambda catalog, value: catalog[0]["message_profiles"][0].__setitem__(
                    "business_services",
                    [value],
                ),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            for name, hidden, mutate in cases:
                with self.subTest(name=name):
                    catalog = [
                        {
                            "id": "minimal-profile",
                            "rail": "generic-iso20022",
                            "embedded_signature_policy": "record-only",
                            "required_reference_datasets": [],
                            "message_profiles": [
                                {
                                    "message_type": "fooo.001",
                                    "direction": "inbound",
                                    "versions": ["fooo.001.001.01"],
                                    "structured_address_mode": "permissive",
                                }
                            ],
                        }
                    ]
                    mutate(catalog, hidden)
                    profile_catalog = write_profile_catalog(
                        root / f"{name}.profiles.rs",
                        catalog=catalog,
                    )

                    rc, _stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--profile-catalog",
                            str(profile_catalog),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("must use printable ASCII", stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn("unknown rail", stderr)
                    self.assertNotIn("unknown policy", stderr)
                    self.assertNotIn("unknown dataset", stderr)
                    self.assertNotIn("unknown mode", stderr)

    def test_profile_catalog_unknown_enum_values_are_rejected_without_echo(self):
        cases = (
            (
                "rail",
                "shadow-rail",
                lambda value: VERIFIER._validate_profile_catalog_profile_fields(
                    {
                        "rail": value,
                        "embedded_signature_policy": "record-only",
                    },
                    "profiles[0]",
                ),
                "unknown rail",
            ),
            (
                "embedded-policy",
                "allow-unverified",
                lambda value: VERIFIER._validate_profile_catalog_profile_fields(
                    {
                        "rail": "generic-iso20022",
                        "embedded_signature_policy": value,
                    },
                    "profiles[0]",
                ),
                "unknown policy",
            ),
            (
                "reference-dataset",
                "swift-pki",
                lambda value: VERIFIER._validate_profile_catalog_profile_fields(
                    {
                        "rail": "generic-iso20022",
                        "embedded_signature_policy": "record-only",
                        "required_reference_datasets": [value],
                    },
                    "profiles[0]",
                ),
                "unknown dataset",
            ),
            (
                "structured-address-mode",
                "optional",
                lambda value: VERIFIER._validate_profile_catalog_message_fields(
                    {"structured_address_mode": value},
                    "profiles[0].message_profiles[0]",
                ),
                "unknown mode",
            ),
        )
        for name, hidden, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    call(hidden)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(hidden, message)

    def test_profile_catalog_version_diagnostics_do_not_echo_values(self):
        cases = (
            (
                "duplicate-profile",
                "shadow-profile",
                [
                    {
                        "id": "shadow-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001.001.01"],
                            }
                        ],
                    },
                    {
                        "id": "shadow-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.002",
                                "direction": "inbound",
                                "versions": ["fooo.002.001.01"],
                            }
                        ],
                    },
                ],
                "duplicates profile id",
            ),
            (
                "wrong-family-alias",
                "barr.002",
                [
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["barr.002"],
                            }
                        ],
                    }
                ],
                "must equal message_type",
            ),
            (
                "duplicate-family-alias",
                "fooo.001",
                [
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001", "fooo.001"],
                            }
                        ],
                    }
                ],
                "duplicates profile/message/direction family alias",
            ),
            (
                "wrong-concrete-version",
                "barr.002.001.01",
                [
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["barr.002.001.01"],
                            }
                        ],
                    }
                ],
                "does not match message_type",
            ),
            (
                "duplicate-concrete-version",
                "fooo.001.001.01",
                [
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": [
                                    "fooo.001.001.01",
                                    "fooo.001.001.01",
                                ],
                            }
                        ],
                    }
                ],
                "duplicates profile/message/direction version",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, hidden, catalog, expected in cases:
                with self.subTest(name=name):
                    profile_catalog = write_profile_catalog(
                        root / f"{name}.profiles.rs",
                        catalog=catalog,
                    )

                    with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                        VERIFIER.verify_profile_catalog(
                            profile_catalog,
                            {"fooo.001.001.01"},
                        )

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn(hidden, message)

    def test_strict_profile_schema_backed_failure_does_not_echo_version(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            hidden_profile = "minimal-profile"
            hidden_versions = ("fooo.001.001.02", "fooo.001.001.03")
            profile_catalog = write_profile_catalog(
                root / "profiles.rs",
                list(hidden_versions),
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "profile catalog has 2 message versions not schema-backed",
                stderr,
            )
            self.assertNotIn(hidden_profile, stderr)
            for hidden_version in hidden_versions:
                self.assertNotIn(hidden_version, stderr)

    def test_source_filename_mismatch_diagnostics_do_not_echo_message_id(self):
        cases = (
            (
                "schema-source",
                "barr.001.001.01",
                lambda manifest, hidden: manifest["schemas"][0]["source"].__setitem__(
                    "path",
                    f"xsd/iso/{hidden}.xsd",
                ),
                "filename must match message_def_id",
            ),
            (
                "blocked-source",
                "fooo.001.001.01",
                lambda manifest, hidden: (
                    manifest["blocked_schema_sources"].append(
                        blocked_schema_source("barr.001.001.01")
                    ),
                    manifest["blocked_schema_sources"][0]["source"].__setitem__(
                        "path",
                        f"xsd/{hidden}.xsd",
                    ),
                ),
                "filename must match message_def_id",
            ),
        )
        for name, hidden, mutate, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest, hidden)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(str(root), stderr)

    def test_schema_fixture_mismatch_diagnostics_do_not_echo_values(self):
        def set_manifest(manifest_path, mutate):
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            mutate(manifest)
            write_json(manifest_path, manifest)

        def schema_target_namespace(root, manifest_path, hidden):
            rewrite_schema(
                root,
                "fooo.001.001.01",
                xsd_text(
                    "fooo.001.001.01",
                    "FooPayload",
                    target_message_id=hidden,
                ),
                manifest_path=manifest_path,
            )

        def schema_payload_root(root, manifest_path, hidden):
            rewrite_schema(
                root,
                "fooo.001.001.01",
                xsd_text("fooo.001.001.01", hidden),
                manifest_path=manifest_path,
            )

        def fixture_namespace(root, _manifest_path, hidden):
            (root / "foo_fixture.xml").write_text(
                fixture_xml(hidden, "FooPayload"),
                encoding="utf-8",
            )

        def fixture_payload_root(root, _manifest_path, hidden):
            (root / "foo_fixture.xml").write_text(
                fixture_xml("fooo.001.001.01", hidden),
                encoding="utf-8",
            )

        def unknown_schema_ref(_root, manifest_path, hidden):
            set_manifest(
                manifest_path,
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "schema",
                    f"iso/{hidden}.xsd",
                ),
            )

        def fixture_schema_message_id(root, manifest_path, hidden):
            (root / "foo_fixture.xml").write_text(
                fixture_xml(hidden, "FooPayload"),
                encoding="utf-8",
            )
            set_manifest(
                manifest_path,
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "message_def_id",
                    hidden,
                ),
            )

        def fixture_schema_payload_root(root, manifest_path, hidden):
            schema_text = xsd_text("fooo.001.001.01", hidden)
            (root / "xsd" / "iso" / "fooo.001.001.01.xsd").write_text(
                schema_text,
                encoding="utf-8",
            )
            set_manifest(
                manifest_path,
                lambda manifest: (
                    manifest["schemas"][0].__setitem__("payload_root", hidden),
                    manifest["schemas"][0]["source"].__setitem__(
                        "sha256",
                        VERIFIER.sha256_hex(schema_text.encode("utf-8")),
                    ),
                ),
            )

        cases = (
            (
                "schema-target-namespace",
                "barr.001.001.01",
                schema_target_namespace,
                "targetNamespace does not match manifest message_def_id",
            ),
            (
                "schema-payload-root",
                "DriftSchemaPayload",
                schema_payload_root,
                "payload root does not match manifest payload_root",
            ),
            (
                "fixture-namespace",
                "barr.001.001.01",
                fixture_namespace,
                "namespace message id does not match manifest fixture",
            ),
            (
                "fixture-payload-root",
                "DriftFixturePayload",
                fixture_payload_root,
                "payload root does not match manifest fixture",
            ),
            (
                "unknown-schema-ref",
                "hidden.001.001.01",
                unknown_schema_ref,
                "references unknown schema",
            ),
            (
                "fixture-schema-message-id",
                "barr.001.001.01",
                fixture_schema_message_id,
                "schema message id does not match fixture",
            ),
            (
                "fixture-schema-payload-root",
                "DriftLinkedSchemaPayload",
                fixture_schema_payload_root,
                "schema payload root does not match fixture",
            ),
        )
        for name, hidden, mutate, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path, hidden)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_profile_catalog_overlong_enum_values_are_rejected_without_echo(self):
        hidden = "x" * 129
        cases = (
            (
                "rail",
                lambda catalog: catalog[0].__setitem__("rail", hidden),
            ),
            (
                "embedded-policy",
                lambda catalog: catalog[0].__setitem__(
                    "embedded_signature_policy",
                    hidden,
                ),
            ),
            (
                "reference-dataset",
                lambda catalog: catalog[0].__setitem__(
                    "required_reference_datasets",
                    [hidden],
                ),
            ),
            (
                "structured-address-mode",
                lambda catalog: catalog[0]["message_profiles"][0].__setitem__(
                    "structured_address_mode",
                    hidden,
                ),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            for name, mutate in cases:
                with self.subTest(name=name):
                    catalog = [
                        {
                            "id": "minimal-profile",
                            "rail": "generic-iso20022",
                            "embedded_signature_policy": "record-only",
                            "required_reference_datasets": [],
                            "message_profiles": [
                                {
                                    "message_type": "fooo.001",
                                    "direction": "inbound",
                                    "versions": ["fooo.001.001.01"],
                                    "structured_address_mode": "permissive",
                                }
                            ],
                        }
                    ]
                    mutate(catalog)
                    profile_catalog = write_profile_catalog(
                        root / f"{name}-overlong.profiles.rs",
                        catalog=catalog,
                    )

                    rc, _stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--profile-catalog",
                            str(profile_catalog),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("must be no longer than 128 characters", stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn("unknown rail", stderr)
                    self.assertNotIn("unknown policy", stderr)
                    self.assertNotIn("unknown dataset", stderr)
                    self.assertNotIn("unknown mode", stderr)

    def test_profile_catalog_overlong_generic_strings_are_rejected_without_echo(self):
        hidden = "M" * (VERIFIER.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "required",
                lambda: VERIFIER._required_string({"rail": hidden}, "rail", "profiles[0]"),
                f"profiles[0].rail must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "optional",
                lambda: VERIFIER._optional_string(
                    {"schema": hidden}, "schema", "fixtures[0]"
                ),
                f"fixtures[0].schema must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "list",
                lambda: VERIFIER._optional_string_list(
                    {"required_reference_datasets": [hidden]},
                    "required_reference_datasets",
                    "profiles[0]",
                ),
                f"profiles[0].required_reference_datasets[0] must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(hidden, message)

        catalog = [
            {
                "id": "minimal-profile",
                "rail": hidden,
                "embedded_signature_policy": "record-only",
                "required_reference_datasets": [],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": "permissive",
                    }
                ],
            }
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs", catalog=catalog)

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                f"profiles[0].rail must be no longer than {VERIFIER.MAX_CLEAN_STRING_CHARS} characters",
                stderr,
            )
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("unknown rail", stderr)

    def test_profile_catalog_generic_strings_reject_unicode_format_controls_without_echo(self):
        hidden = "\u202exsd-string-leak"
        cases = (
            (
                "required",
                lambda: VERIFIER._required_string(
                    {"rail": "generic" + hidden}, "rail", "profiles[0]"
                ),
            ),
            (
                "optional",
                lambda: VERIFIER._optional_string(
                    {"schema": "iso/fooo.001.001.01" + hidden + ".xsd"},
                    "schema",
                    "fixtures[0]",
                ),
            ),
            (
                "list",
                lambda: VERIFIER._optional_string_list(
                    {"required_reference_datasets": ["bic-lei" + hidden]},
                    "required_reference_datasets",
                    "profiles[0]",
                ),
            ),
        )
        for name, call in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn("control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("xsd-string-leak", message)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "fixture_manifest.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        VERIFIER.FixtureManifestError,
                        "max file bytes must be a positive integer",
                    ):
                        VERIFIER._read_regular_file(path, max_bytes=limit)

    def test_checked_in_manifest_passes_and_records_reviewed_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            summary_out = Path(raw_root) / "summary.json"
            summary_out.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
                    "--profile-catalog",
                    str(VERIFIER.DEFAULT_PROFILE_CATALOG),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["version"], VERIFIER.SUMMARY_VERSION)
            self.assertEqual(summary["verified_schemas"], 7)
            self.assertEqual(summary["verified_fixtures"], 11)
            self.assertEqual(summary["schema_backed_fixtures"], 7)
            self.assertEqual(
                summary["manifest_sha256"],
                VERIFIER.sha256_hex(
                    (
                        REPO_ROOT
                        / "fixtures"
                        / "iso20022"
                        / "xsd"
                        / "fixture_manifest.json"
                    ).read_bytes()
                ),
            )
            self.assertEqual(len(summary["missing_schema_fixtures"]), 4)
            missing_schema_ids = sorted(
                entry["message_def_id"] for entry in summary["missing_schema_fixtures"]
            )
            self.assertIn("colr.012.001.05", missing_schema_ids)
            self.assertNotIn("colr.007.001.08", missing_schema_ids)
            self.assertEqual(summary["blocked_schema_source_count"], 3)
            self.assertEqual(
                sorted(
                    entry["message_def_id"] for entry in summary["blocked_schema_sources"]
                ),
                ["pacs.002.001.12", "pacs.008.001.10", "pacs.009.001.10"],
            )
            self.assertEqual(summary["pending_schema_source_count"], 8)
            self.assertEqual(
                sorted(
                    entry["message_def_id"] for entry in summary["pending_schema_sources"]
                ),
                [
                    "colr.012.001.05",
                    "sese.023.001.09",
                    "sese.023.001.11",
                    "sese.024.001.09",
                    "sese.024.001.10",
                    "sese.025.001.08",
                    "sese.025.001.10",
                    "sese.025.001.11",
                ],
            )
            self.assertEqual(summary["schema_only_entries"], [])
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(summary_out.parent.glob(".iso-*.tmp")),
                [],
            )
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, VERIFIER.sha256_hex(VERIFIER._canonical_json_bytes(body)))

    def test_long_summary_output_leaf_uses_bounded_atomic_temp_name(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            summary_out = root / (("x" * 240) + ".json")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)
            self.assertEqual(list(summary_out.parent.glob(".iso-*.tmp")), [])

    def test_rejected_manifest_paths_do_not_echo_secret_absolute_paths(self):
        cases = [
            (
                lambda body: body["schemas"][0].update(
                    {"path": "/tmp/token=xsd-schema-secret/fooo.001.001.01.xsd"}
                ),
                "schemas[0].path must be relative",
                "xsd-schema-secret",
            ),
            (
                lambda body: body["fixtures"][0].update(
                    {"path": "/tmp/token=xsd-fixture-secret/foo_fixture.xml"}
                ),
                "fixtures[0].path must be relative",
                "xsd-fixture-secret",
            ),
        ]
        for mutate, expected, secret in cases:
            with self.subTest(expected=expected):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_looking_manifest_summary_strings_are_rejected_without_echo(self):
        cases = [
            (
                lambda body: body["schemas"][0].update(
                    {"path": "iso/token=xsd-schema-secret/fooo.001.001.01.xsd"}
                ),
                "schemas[0].path must not contain secret-looking material",
                "xsd-schema-secret",
            ),
            (
                lambda body: body["schemas"][0].update(
                    {"path": "iso/token-xsd-schema-secret/fooo.001.001.01.xsd"}
                ),
                "schemas[0].path must not contain secret-looking material",
                "token-xsd-schema-secret",
            ),
            (
                lambda body: body["fixtures"][0].update(
                    {"path": "../token=xsd-fixture-secret/foo_fixture.xml"}
                ),
                "fixtures[0].path must not contain secret-looking material",
                "xsd-fixture-secret",
            ),
            (
                lambda body: body["fixtures"][0].update(
                    {"path": "../token-xsd-fixture-secret/foo_fixture.xml"}
                ),
                "fixtures[0].path must not contain secret-looking material",
                "token-xsd-fixture-secret",
            ),
            (
                lambda body: body["schemas"][0]["source"].update(
                    {"path": "xsd/token=xsd-source-secret/fooo.001.001.01.xsd"}
                ),
                "schemas[0].source.path must not contain secret-looking material",
                "xsd-source-secret",
            ),
            (
                lambda body: body["schemas"][0]["source"].update(
                    {
                        "repository": (
                            "https://github.com/moov-io/token-xsd-source-repo-secret"
                        )
                    }
                ),
                "schemas[0].source.repository must not contain secret-looking material",
                "token-xsd-source-repo-secret",
            ),
            (
                lambda body: body["schemas"][0]["source"].update(
                    {"path": "xsd/token-xsd-source-secret/fooo.001.001.01.xsd"}
                ),
                "schemas[0].source.path must not contain secret-looking material",
                "token-xsd-source-secret",
            ),
            (
                lambda body: body["schemas"][0].update(
                    {"payload_root": "token_xsd_schema_payload_secret"}
                ),
                "schemas[0].payload_root must not contain secret-looking material",
                "token_xsd_schema_payload_secret",
            ),
            (
                lambda body: body["fixtures"][0].update(
                    {"payload_root": "%70assword%253Dxsd-fixture-payload-secret"}
                ),
                "fixtures[0].payload_root must not contain secret-looking material",
                "xsd-fixture-payload-secret",
            ),
            (
                lambda body: body["schemas"][0].update(
                    {
                        "schema_only_reason": (
                            "Reviewed gap private_key=xsd-schema-reason-secret"
                        )
                    }
                ),
                "schemas[0].schema_only_reason must not contain secret-looking material",
                "xsd-schema-reason-secret",
            ),
            (
                lambda body: (
                    body["fixtures"][0].pop("schema"),
                    body["fixtures"][0].update(
                        {
                            "missing_schema_reason": (
                                "Reviewed gap token=xsd-missing-reason-secret"
                            )
                        }
                    ),
                ),
                "fixtures[0].missing_schema_reason must not contain secret-looking material",
                "xsd-missing-reason-secret",
            ),
            (
                lambda body: body.update(
                    {
                        "blocked_schema_sources": [
                            {
                                **blocked_schema_source(),
                                "reason": (
                                    "Blocked candidate Authorization: xsd-blocked-reason-secret"
                                ),
                            }
                        ]
                    }
                ),
                "blocked_schema_sources[0].reason must not contain secret-looking material",
                "xsd-blocked-reason-secret",
            ),
            (
                lambda body: (
                    body.update({"blocked_schema_sources": [blocked_schema_source()]}),
                    body["blocked_schema_sources"][0]["source"].update(
                        {
                            "repository": (
                                "https://github.com/prog-nov/"
                                "token-xsd-blocked-repo-secret"
                            )
                        }
                    ),
                ),
                (
                    "blocked_schema_sources[0].source.repository must not contain "
                    "secret-looking material"
                ),
                "token-xsd-blocked-repo-secret",
            ),
        ]
        for mutate, expected, secret in cases:
            with self.subTest(expected=expected):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("Authorization:", stderr)
                    self.assertNotIn(secret, stderr)

    def test_symlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            target = root / "xsd-target.summary.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "xsd-link.summary.json"
            try:
                summary_out.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_summary_output_path_rejects_smuggled_segments(self):
        cases = (
            ("semicolon", "xsd;debug.summary.json", "semicolon path"),
            ("whitespace", "xsd summary.json", "whitespace"),
            ("leading-dash", "nested/-xsd.summary.json", "leading-dash"),
            ("parent", "nested/../xsd.summary.json", "dot or parent"),
            ("dot", lambda root: f"{root}/nested/./xsd.summary.json", "dot or parent"),
            ("empty", lambda root: f"{root}//xsd.summary.json", "empty path"),
        )
        for name, summary_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())

                    rc, stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--summary-out",
                            summary_arg(root)
                            if callable(summary_arg)
                            else str(root / summary_arg),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_hardlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            target = root / "xsd-target.summary.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "xsd-hardlink.summary.json"
            try:
                summary_out.hardlink_to(target)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be hard-linked", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_summary_output_ancestor_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            target_dir = root / "xsd-target"
            target_dir.mkdir()
            ancestor = root / "xsd-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "xsd.summary.json"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_symlinked_summary_output_ancestor_is_rejected_before_xmllint(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            target_dir = root / "xsd-target"
            target_dir.mkdir()
            ancestor = root / "xsd-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "xsd.summary.json"
            marker = root / "xmllint-called"

            def fake_run(*_args, **_kwargs):
                marker.write_text("called\n", encoding="utf-8")
                return (0, "", False, "", False, False)

            original_which = VERIFIER.shutil.which
            original_run = VERIFIER._run_command_bounded
            VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
            VERIFIER._run_command_bounded = fake_run
            try:
                rc, stdout, stderr = run_verify(
                    [
                        "--manifest",
                        str(manifest_path),
                        "--validate-xml-schema",
                        "--summary-out",
                        str(summary_out),
                    ]
                )
            finally:
                VERIFIER.shutil.which = original_which
                VERIFIER._run_command_bounded = original_run

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse(marker.exists())
            self.assertFalse((target_dir / "nested").exists())

    def test_cli_input_paths_reject_raw_smuggling_before_read(self):
        cases = (
            ("manifest semicolon", "--manifest", "fixture;debug.json", "semicolon path"),
            ("manifest whitespace", "--manifest", "fixture manifest.json", "whitespace"),
            ("catalog leading-dash", "--profile-catalog", "nested/-profiles.rs", "leading-dash"),
            ("catalog parent", "--profile-catalog", "nested/../profiles.rs", "dot or parent"),
            (
                "manifest dot",
                "--manifest",
                lambda root: f"{root}/nested/./fixture_manifest.json",
                "dot or parent",
            ),
            (
                "catalog empty",
                "--profile-catalog",
                lambda root: f"{root}//profiles.rs",
                "empty path",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)

                    rc, stdout, stderr = run_verify([flag, value])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_direct_run_paths_reject_smuggling_before_manifest_loading(self):
        def args_for(root, **overrides):
            values = {
                "manifest": root / "missing-manifest.json",
                "summary_out": root / "xsd.summary.json",
                "require_schema_backed_fixtures": False,
                "require_fixture_for_schema": False,
                "profile_catalog": None,
                "require_profile_schema_backed_versions": False,
                "validate_xml_schema": False,
                "xmllint_timeout_secs": VERIFIER.DEFAULT_XMLLINT_TIMEOUT_SECS,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "manifest whitespace",
                lambda root: args_for(root, manifest=root / "fixture manifest.json"),
                "--manifest must not contain whitespace",
            ),
            (
                "catalog parent",
                lambda root: args_for(
                    root,
                    profile_catalog=root / "nested" / ".." / "profiles.rs",
                ),
                "--profile-catalog must not contain dot or parent segments",
            ),
            (
                "output leading dash",
                lambda root: args_for(
                    root,
                    summary_out=root / "nested" / "-xsd.summary.json",
                ),
                "summary_out must not contain leading-dash path segments",
            ),
        )
        for name, make_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                        VERIFIER.run(make_args(root))

                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)

    def test_path_helpers_reject_unicode_format_controls_without_echo(self):
        hidden = "\u202exsd-path-leak"
        base = Path("/tmp/xsd")
        containment_root = Path("/tmp")
        cases = (
            (
                "path-object",
                lambda: VERIFIER._reject_output_path_smuggling(
                    Path("out") / f"summary{hidden}.json", "output path"
                ),
            ),
            (
                "raw-path",
                lambda: VERIFIER._reject_raw_output_path_smuggling(
                    f"out/summary{hidden}.json", "--summary-out"
                ),
            ),
            (
                "source-path",
                lambda: VERIFIER._validate_source_path(
                    f"xsd/{hidden}/fooo.001.001.01.xsd", "source.path"
                ),
            ),
            (
                "relative-path",
                lambda: VERIFIER._validate_relative_path(
                    f"../{hidden}/fixture.xml",
                    base,
                    containment_root,
                    "fixture.path",
                    allow_parent_segments=True,
                ),
            ),
        )
        for name, call in cases:
            with self.subTest(name=name):
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn("control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("xsd-path-leak", message)

    def test_symlinked_manifest_ancestor_is_rejected_before_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "manifest-target"
            manifest_path = write_minimal_tree(target_dir, minimal_manifest())
            ancestor = root / "manifest-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            manifest = ancestor / manifest_path.relative_to(target_dir)

            rc, stdout, stderr = run_verify(["--manifest", str(manifest)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_strict_flags_reject_current_reviewed_gaps(self):
        manifest = str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json")

        rc, _stdout, stderr = run_verify(
            [
                "--manifest",
                manifest,
                "--profile-catalog",
                str(VERIFIER.DEFAULT_PROFILE_CATALOG),
                "--require-schema-backed-fixtures",
            ]
        )
        self.assertEqual(rc, 2)
        self.assertIn("not schema-backed", stderr)

        rc, _stdout, stderr = run_verify(
            [
                "--manifest",
                manifest,
                "--profile-catalog",
                str(VERIFIER.DEFAULT_PROFILE_CATALOG),
                "--require-fixture-for-schema",
            ]
        )
        self.assertEqual(rc, 0, stderr)
        summary = json.loads(_stdout)
        self.assertEqual(summary["schema_only_entries"], [])

    def test_strict_reviewed_gap_failures_do_not_echo_reason_text(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            missing_reason = "Reviewed missing schema package: internal-ticket-xsd-713."
            missing_manifest = minimal_manifest()
            missing_manifest["fixtures"].append(
                {
                    "path": "../barr_fixture.xml",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "missing_schema_reason": missing_reason,
                }
            )
            missing_manifest_path = write_minimal_tree(root / "missing", missing_manifest)
            (root / "missing" / "barr_fixture.xml").write_text(
                fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(missing_manifest_path),
                    "--require-schema-backed-fixtures",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("is not schema-backed", stderr)
            self.assertIn("reviewed missing-schema reason is recorded", stderr)
            self.assertNotIn(missing_reason, stderr)
            self.assertNotIn("internal-ticket-xsd-713", stderr)

            schema_only_reason = "Reviewed standalone fixture gap: internal-ticket-xsd-914."
            schema_only_manifest = minimal_manifest()
            schema_only_manifest["schemas"].append(
                {
                    "path": "iso/barr.001.001.01.xsd",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "schema_only_reason": schema_only_reason,
                    "source": source_provenance("barr.001.001.01", "BarPayload"),
                }
            )
            schema_only_root = root / "schema-only"
            schema_only_manifest_path = write_minimal_tree(
                schema_only_root,
                schema_only_manifest,
            )
            (schema_only_root / "xsd" / "iso" / "barr.001.001.01.xsd").write_text(
                xsd_text("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(schema_only_manifest_path),
                    "--require-fixture-for-schema",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("has no standalone fixture", stderr)
            self.assertIn("reviewed schema-only reason is recorded", stderr)
            self.assertNotIn(schema_only_reason, stderr)
            self.assertNotIn("internal-ticket-xsd-914", stderr)

    def test_minimal_schema_backed_manifest_passes_with_strict_flags(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["schema_backed_fixtures"], 1)
            self.assertEqual(summary["missing_schema_fixtures"], [])
            self.assertEqual(summary["schema_only_entries"], [])

    def test_summary_emits_schemas_and_fixtures_in_canonical_order(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            manifest["schemas"].append(
                {
                    "path": "iso/barr.001.001.01.xsd",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "source": source_provenance("barr.001.001.01", "BarPayload"),
                }
            )
            manifest["fixtures"].append(
                {
                    "path": "../bar_fixture.xml",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "schema": "iso/barr.001.001.01.xsd",
                }
            )
            manifest_path = write_minimal_tree(root, manifest)
            (root / "xsd" / "iso" / "barr.001.001.01.xsd").write_text(
                xsd_text("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )
            (root / "bar_fixture.xml").write_text(
                fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(
                [schema["message_def_id"] for schema in summary["schemas"]],
                ["barr.001.001.01", "fooo.001.001.01"],
            )
            self.assertEqual(
                [fixture["message_def_id"] for fixture in summary["fixtures"]],
                ["barr.001.001.01", "fooo.001.001.01"],
            )

    def test_manifest_verification_parses_and_hashes_each_checked_file_once(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            schema_path = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
            fixture_path = root / "foo_fixture.xml"
            watched = {
                manifest_path.resolve(): "manifest",
                schema_path.resolve(): "schema",
                fixture_path.resolve(): "fixture",
            }
            expected_bytes = {
                path: path.read_bytes()
                for path in watched
            }
            read_counts = dict.fromkeys(watched, 0)
            original_read = VERIFIER._read_regular_file

            def read_once(path, *, max_bytes=None, display_label=None):
                resolved = Path(path).resolve()
                if resolved in watched:
                    read_counts[resolved] += 1
                    if read_counts[resolved] > 1:
                        raise AssertionError(f"{watched[resolved]} file was read more than once")
                return original_read(
                    path,
                    max_bytes=max_bytes,
                    display_label=display_label,
                )

            args = VERIFIER.build_parser().parse_args(["--manifest", str(manifest_path)])
            VERIFIER._read_regular_file = read_once
            try:
                summary = VERIFIER.verify_manifest(manifest_path, args)
            finally:
                VERIFIER._read_regular_file = original_read

            self.assertEqual(read_counts, dict.fromkeys(watched, 1))
            self.assertEqual(summary["manifest_sha256"], VERIFIER.sha256_hex(
                expected_bytes[manifest_path.resolve()]
            ))
            self.assertEqual(summary["schemas"][0]["sha256"], VERIFIER.sha256_hex(
                expected_bytes[schema_path.resolve()]
            ))
            self.assertEqual(summary["fixtures"][0]["sha256"], VERIFIER.sha256_hex(
                expected_bytes[fixture_path.resolve()]
            ))

    def test_profile_catalog_schema_backed_versions_are_recorded(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["strict"]["require_profile_schema_backed_versions"])
            self.assertEqual(summary["profile_checked_versions"], 1)
            self.assertEqual(summary["profile_schema_backed_versions"], 1)
            self.assertEqual(summary["missing_profile_schema_versions"], [])
            self.assertEqual(summary["missing_profile_schema_message_ids"], [])
            self.assertEqual(summary["unreviewed_profile_schema_message_id_count"], 0)
            self.assertEqual(summary["unreviewed_profile_schema_message_ids"], [])
            self.assertEqual(summary["profile_catalog"]["profiles"], 1)
            self.assertEqual(
                summary["profile_catalog"]["sha256"],
                VERIFIER.sha256_hex(profile_catalog.read_bytes()),
            )
            match = VERIFIER.PROFILE_CATALOG_RE.search(
                profile_catalog.read_text(encoding="utf-8")
            )
            self.assertIsNotNone(match)
            catalog_json = match.group("body")
            self.assertEqual(
                summary["profile_catalog"]["catalog_json_sha256"],
                VERIFIER.sha256_hex(catalog_json.encode("utf-8")),
            )

    def test_strict_profile_schema_backed_versions_uses_default_catalog(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs")
            original_default = VERIFIER.DEFAULT_PROFILE_CATALOG
            VERIFIER.DEFAULT_PROFILE_CATALOG = profile_catalog
            try:
                rc, stdout, stderr = run_verify(
                    [
                        "--manifest",
                        str(manifest_path),
                        "--require-profile-schema-backed-versions",
                    ]
                )
            finally:
                VERIFIER.DEFAULT_PROFILE_CATALOG = original_default

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["strict"]["require_profile_schema_backed_versions"])
            self.assertEqual(summary["profile_catalog"]["path"], str(profile_catalog))
            self.assertEqual(summary["profile_checked_versions"], 1)
            self.assertEqual(summary["profile_schema_backed_versions"], 1)
            self.assertEqual(summary["missing_profile_schema_versions"], [])
            self.assertEqual(summary["missing_profile_schema_message_ids"], [])
            self.assertEqual(summary["unreviewed_profile_schema_message_id_count"], 0)
            self.assertEqual(summary["unreviewed_profile_schema_message_ids"], [])

    def test_profile_catalog_loader_ignores_commented_or_string_spoofs(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs")
            real_catalog = profile_catalog.read_text(encoding="utf-8")
            spoof_catalog = [
                {
                    "id": "spoof-profile",
                    "rail": "generic-iso20022",
                    "embedded_signature_policy": "record-only",
                    "message_profiles": [
                        {
                            "message_type": "fooo.001",
                            "direction": "inbound",
                            "versions": ["fooo.001.001.02"],
                            "structured_address_mode": "permissive",
                        }
                    ],
                }
            ]
            spoof_json = json.dumps(spoof_catalog, indent=2, sort_keys=True)
            profile_catalog.write_text(
                "/*\n"
                'const DEFAULT_PROFILES_JSON: &str = r#"\n'
                + spoof_json
                + '\n"#;\n'
                "*/\n"
                'const IGNORED: &str = r###"\n'
                'const DEFAULT_PROFILES_JSON: &str = r#"\n'
                + spoof_json
                + '\n"#;\n'
                '"###;\n'
                + real_catalog,
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["profile_checked_versions"], 1)
            self.assertEqual(
                summary["profile_catalog"]["versions"][0]["profile_id"],
                "minimal-profile",
            )

    def test_profile_catalog_loader_rejects_duplicate_active_constants(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(root / "profiles.rs")
            real_catalog = profile_catalog.read_text(encoding="utf-8")
            profile_catalog.write_text(real_catalog + "\n" + real_catalog, encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("exactly one DEFAULT_PROFILES_JSON", stderr)

    def test_profile_catalog_loader_rejects_non_finite_json_constants(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = root / "profiles.rs"
            profile_catalog.write_text(
                'const DEFAULT_PROFILES_JSON: &str = r#"\n[NaN]\n"#;\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("NaN", stderr)

    def test_profile_catalog_loader_rejects_json_surrogate_strings(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = root / "profiles.rs"
            profile_catalog.write_text(
                'const DEFAULT_PROFILES_JSON: &str = r#"\n["\\ud800"]\n"#;\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_profile_catalog_strict_flag_rejects_missing_schema_versions(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(
                root / "profiles.rs",
                ["fooo.001.001.01", "fooo.001.001.02"],
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertFalse(summary["strict"]["require_profile_schema_backed_versions"])
            self.assertEqual(summary["profile_checked_versions"], 2)
            self.assertEqual(summary["profile_schema_backed_versions"], 1)
            self.assertEqual(
                summary["missing_profile_schema_versions"][0]["message_def_id"],
                "fooo.001.001.02",
            )
            self.assertEqual(
                summary["missing_profile_schema_message_ids"],
                [
                    {
                        "message_def_id": "fooo.001.001.02",
                        "profile_version_count": 1,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": False,
                    }
                ],
            )
            self.assertEqual(summary["unreviewed_profile_schema_message_id_count"], 1)
            self.assertEqual(
                summary["unreviewed_profile_schema_message_ids"],
                [
                    {
                        "message_def_id": "fooo.001.001.02",
                        "profile_version_count": 1,
                    }
                ],
            )

            rc, _stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn(
                "profile catalog has 1 message version not schema-backed",
                stderr,
            )
            self.assertNotIn("fooo.001.001.02", stderr)

    def test_profile_catalog_summary_arrays_are_canonical_order(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(
                root / "profiles.rs",
                catalog=[
                    {
                        "id": "z-profile",
                        "rail": "generic-iso20022",
                        "embedded_signature_policy": "record-only",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001.001.02", "fooo.001"],
                            }
                        ],
                    },
                    {
                        "id": "a-profile",
                        "rail": "generic-iso20022",
                        "embedded_signature_policy": "record-only",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001.001.01", "fooo.001"],
                            }
                        ],
                    },
                ],
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(
                [
                    (entry["profile_id"], entry["message_def_id"])
                    for entry in summary["profile_catalog"]["versions"]
                ],
                [
                    ("a-profile", "fooo.001.001.01"),
                    ("z-profile", "fooo.001.001.02"),
                ],
            )
            self.assertEqual(
                [
                    (entry["profile_id"], entry["version"])
                    for entry in summary["profile_catalog"]["skipped_family_versions"]
                ],
                [("a-profile", "fooo.001"), ("z-profile", "fooo.001")],
            )
            self.assertEqual(
                [
                    (entry["profile_id"], entry["message_def_id"])
                    for entry in summary["missing_profile_schema_versions"]
                ],
                [("z-profile", "fooo.001.001.02")],
            )

    def test_profile_catalog_shape_is_fail_closed(self):
        cases = []
        crl_der = b"\x30\x07\x30\x00\x30\x00\x03\x01\x00"
        crl_der_b64 = base64.b64encode(crl_der).decode("ascii")
        crl_der_sha256 = VERIFIER.sha256_hex(crl_der)
        duplicate_profile = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            },
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.002",
                        "direction": "inbound",
                        "versions": ["fooo.002.001.01"],
                    }
                ],
            },
        ]
        cases.append((duplicate_profile, "duplicates profile id"))
        duplicate_message = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    },
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.02"],
                    },
                ],
            }
        ]
        cases.append((duplicate_message, "duplicates profile/message/direction entry"))
        bad_direction = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "sideways",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_direction, "direction must be one of"))
        bad_message_type = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "FOOO.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_message_type, "message_type must be lowercase ISO family id"))
        empty_versions = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": [],
                    }
                ],
            }
        ]
        cases.append((empty_versions, "versions must not be empty"))
        wrong_family_alias = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.002"],
                    }
                ],
            }
        ]
        cases.append((wrong_family_alias, "must equal message_type"))
        duplicate_family_alias = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001", "fooo.001"],
                    }
                ],
            }
        ]
        cases.append((duplicate_family_alias, "duplicates profile/message/direction family alias"))
        unknown_profile_key = [
            {
                "id": "minimal-profile",
                "unexpected": "release-ready",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((unknown_profile_key, "contains unknown keys"))
        unknown_message_key = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "unexpected": "release-ready",
                    }
                ],
            }
        ]
        cases.append((unknown_message_key, "contains unknown keys"))
        control_profile_id = [
            {
                "id": "minimal\nprofile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((control_profile_id, "id must not contain control characters"))
        missing_rail = [
            {
                "id": "minimal-profile",
                "rail": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((missing_rail, "rail must be a non-empty string"))
        missing_signature_policy = [
            {
                "id": "minimal-profile",
                "embedded_signature_policy": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                missing_signature_policy,
                "embedded_signature_policy must be a non-empty string",
            )
        )
        bad_rail = [
            {
                "id": "minimal-profile",
                "rail": "unknown-rail",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_rail, "unknown rail"))
        bad_signature_policy = [
            {
                "id": "minimal-profile",
                "embedded_signature_policy": "allow-unverified",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_signature_policy, "unknown policy"))
        bad_reference_dataset = [
            {
                "id": "minimal-profile",
                "required_reference_datasets": ["swift-pki"],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_reference_dataset, "unknown dataset"))
        null_reference_datasets = [
            {
                "id": "minimal-profile",
                "required_reference_datasets": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((null_reference_datasets, "required_reference_datasets must be a JSON array"))
        bad_trust_pin = [
            {
                "id": "minimal-profile",
                "signature_public_key_sha256_pins": ["0" * 64],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_trust_pin, "canonical nonzero SHA-256"))
        null_trust_pin_list = [
            {
                "id": "minimal-profile",
                "signature_public_key_sha256_pins": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((null_trust_pin_list, "signature_public_key_sha256_pins must be a JSON array"))
        bad_policy_oid = [
            {
                "id": "minimal-profile",
                "x509_required_certificate_policy_oids": ["01.2"],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_policy_oid, "dotted numeric OID"))
        overlapping_public_pins = [
            {
                "id": "minimal-profile",
                "signature_public_key_sha256_pins": ["1" * 64],
                "trusted_public_key_sha256": ["1" * 64],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                overlapping_public_pins,
                "signature_public_key_sha256_pins/trusted_public_key_sha256",
            )
        )
        overlapping_revoked_pins = [
            {
                "id": "minimal-profile",
                "x509_trust_anchor_sha256_pins": ["2" * 64],
                "revoked_certificate_sha256": ["2" * 64],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((overlapping_revoked_pins, "trusted/revoked certificate pins"))
        overlapping_public_certificate_pins = [
            {
                "id": "minimal-profile",
                "signature_public_key_sha256_pins": ["3" * 64],
                "x509_trust_anchor_sha256_pins": ["3" * 64],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                overlapping_public_certificate_pins,
                "public-key/certificate SHA-256 pins",
            )
        )
        overlapping_pin_revocation_der = [
            {
                "id": "minimal-profile",
                "signature_public_key_sha256_pins": [crl_der_sha256],
                "x509_crl_der_base64": [crl_der_b64],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                overlapping_pin_revocation_der,
                "trust pin/revocation DER SHA-256 roles",
            )
        )
        bad_revocation_bool = [
            {
                "id": "minimal-profile",
                "x509_require_crl_revocation_check": "true",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((bad_revocation_bool, "must be a boolean"))
        null_revocation_bool = [
            {
                "id": "minimal-profile",
                "x509_require_crl_revocation_check": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((null_revocation_bool, "must be a boolean"))
        missing_address_mode = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": None,
                    }
                ],
            }
        ]
        cases.append((missing_address_mode, "structured_address_mode must be a non-empty string"))
        missing_crl_material = [
            {
                "id": "minimal-profile",
                "x509_require_crl_revocation_check": True,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((missing_crl_material, "CRL revocation is required"))
        malformed_ocsp_material = [
            {
                "id": "minimal-profile",
                "x509_ocsp_response_der_base64": ["not-base64"],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((malformed_ocsp_material, "must be canonical base64"))
        too_many_ocsp_responses = [
            {
                "id": "minimal-profile",
                "x509_ocsp_response_der_base64": [
                    f"not-base64-{index}"
                    for index in range(VERIFIER.MAX_PROFILE_DER_BLOBS + 1)
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                too_many_ocsp_responses,
                "x509_ocsp_response_der_base64 must not contain more than",
            )
        )
        non_ocsp_sequence = [
            {
                "id": "minimal-profile",
                "x509_ocsp_response_der_base64": [
                    base64.b64encode(b"\x30\x03\x02\x01\x00").decode("ascii")
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((non_ocsp_sequence, "must look like a successful DER OCSP response"))
        null_crl_material = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": None,
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((null_crl_material, "x509_crl_der_base64 must be a JSON array"))
        too_many_crls = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": [
                    f"not-base64-{index}"
                    for index in range(VERIFIER.MAX_PROFILE_DER_BLOBS + 1)
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append(
            (
                too_many_crls,
                "x509_crl_der_base64 must not contain more than",
            )
        )
        malformed_crl_der = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": [
                    base64.b64encode(b"\x04\x01x").decode("ascii")
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((malformed_crl_der, "must be a DER SEQUENCE"))
        non_crl_sequence = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": [
                    base64.b64encode(b"\x30\x03\x02\x01\x00").decode("ascii")
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((non_crl_sequence, "must look like a DER CRL"))
        truncated_crl_der = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": [
                    base64.b64encode(b"\x30\x03\x01").decode("ascii")
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((truncated_crl_der, "DER length does not consume the whole value"))
        oversized_crl_material = [
            {
                "id": "minimal-profile",
                "x509_crl_der_base64": [
                    base64.b64encode(
                        b"x" * (VERIFIER.MAX_PROFILE_DER_BYTES + 1)
                    ).decode("ascii")
                ],
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((oversized_crl_material, "must decode to no more than"))
        require_verified_without_pins = [
            {
                "id": "minimal-profile",
                "embedded_signature_policy": "require-verified",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                    }
                ],
            }
        ]
        cases.append((require_verified_without_pins, "has no public-key or X.509 trust pins"))
        bad_address_mode = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "structured_address_mode": "optional",
                    }
                ],
            }
        ]
        cases.append((bad_address_mode, "unknown mode"))
        service_without_app_header = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "business_services": ["minimal.service"],
                        "require_app_header": False,
                        "require_business_service": True,
                    }
                ],
            }
        ]
        cases.append((service_without_app_header, "require_app_header must be true"))
        null_business_services = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "business_services": None,
                    }
                ],
            }
        ]
        cases.append((null_business_services, "business_services must be a JSON array"))
        null_require_app_header = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "require_app_header": None,
                    }
                ],
            }
        ]
        cases.append((null_require_app_header, "require_app_header must be a boolean"))
        missing_business_service = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "require_business_service": True,
                    }
                ],
            }
        ]
        cases.append((missing_business_service, "business_services must not be empty"))
        case_drift_duplicate_business_services = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "business_services": [
                            "Service.A",
                            "service.a",
                        ],
                    }
                ],
            }
        ]
        cases.append(
            (
                case_drift_duplicate_business_services,
                "ignoring ASCII case",
            )
        )
        zero_supplementary_cap = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "supplementary_data_max_bytes": 0,
                    }
                ],
            }
        ]
        cases.append((zero_supplementary_cap, "supplementary_data_max_bytes must be positive"))
        null_supplementary_cap = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "supplementary_data_max_bytes": None,
                    }
                ],
            }
        ]
        cases.append(
            (
                null_supplementary_cap,
                "supplementary_data_max_bytes must be a non-negative integer",
            )
        )
        oversized_supplementary_cap = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "supplementary_data_max_bytes": (
                            VERIFIER.MAX_PROFILE_UNSIGNED_INT + 1
                        ),
                    }
                ],
            }
        ]
        cases.append(
            (
                oversized_supplementary_cap,
                "supplementary_data_max_bytes must fit in u64",
            )
        )
        null_amount_minor_units = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "amount_minor_units": None,
                    }
                ],
            }
        ]
        cases.append((null_amount_minor_units, "amount_minor_units must be a JSON array"))
        bad_minor_units_currency = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "amount_minor_units": [
                            {"currency": "usd", "minor_units": 2},
                        ],
                    }
                ],
            }
        ]
        cases.append((bad_minor_units_currency, "uppercase ISO 4217 code"))
        null_minor_units = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "amount_minor_units": [
                            {"currency": "USD", "minor_units": None},
                        ],
                    }
                ],
            }
        ]
        cases.append((null_minor_units, "minor_units must be a non-negative integer"))
        duplicate_minor_units_currency = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "amount_minor_units": [
                            {"currency": "USD", "minor_units": 2},
                            {"currency": "USD", "minor_units": 3},
                        ],
                    }
                ],
            }
        ]
        cases.append((duplicate_minor_units_currency, "currency duplicates"))
        excessive_minor_units = [
            {
                "id": "minimal-profile",
                "message_profiles": [
                    {
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "versions": ["fooo.001.001.01"],
                        "amount_minor_units": [
                            {"currency": "USD", "minor_units": 5},
                        ],
                    }
                ],
            }
        ]
        cases.append((excessive_minor_units, "minor_units must be at most 4"))

        for catalog, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    profile_catalog = write_profile_catalog(
                        root / "profiles.rs",
                        catalog=catalog,
                    )

                    rc, _stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--profile-catalog",
                            str(profile_catalog),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(str(root), stderr)
                    if message == "already checked-in schema":
                        self.assertNotIn("fooo.001.001.01", stderr)
                    if message == "trust pin/revocation DER SHA-256 roles":
                        self.assertNotIn(crl_der_sha256, stderr)

    def test_profile_catalog_duplicate_strings_do_not_echo_secret_material(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            profile_catalog = write_profile_catalog(
                root / "profiles.rs",
                catalog=[
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001.001.01"],
                                "business_services": [
                                    "token=xsd-duplicate-secret",
                                    "token=xsd-duplicate-secret",
                                ],
                            }
                        ],
                    }
                ],
            )

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("business_services[0] contains secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("xsd-duplicate-secret", stderr)

    def test_checked_in_profile_catalog_records_advertised_schema_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            summary_out = Path(raw_root) / "summary.json"
            profile_catalog = VERIFIER.DEFAULT_PROFILE_CATALOG

            self.assertTrue(profile_catalog.exists())
            self.assertEqual(profile_catalog.name, "profiles.rs")
            self.assertEqual(profile_catalog.parent.name, "iso_bridge")

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            missing_ids = {
                entry["message_def_id"]
                for entry in summary["missing_profile_schema_versions"]
            }
            self.assertNotIn("pacs.004.001.09", missing_ids)
            self.assertIn("pacs.002.001.12", missing_ids)
            self.assertNotIn("camt.056.001.09", missing_ids)
            self.assertGreater(summary["profile_checked_versions"], 0)
            self.assertGreater(summary["profile_schema_backed_versions"], 0)
            self.assertEqual(summary["profile_schema_backed_versions"], 31)
            self.assertEqual(len(summary["missing_profile_schema_versions"]), 24)
            self.assertEqual(
                summary["missing_profile_schema_message_ids"],
                [
                    {
                        "message_def_id": "colr.012.001.05",
                        "profile_version_count": 2,
                        "reviewed_missing_schema_fixture": True,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "pacs.002.001.12",
                        "profile_version_count": 4,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": True,
                        "pending_source": False,
                    },
                    {
                        "message_def_id": "pacs.008.001.10",
                        "profile_version_count": 3,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": True,
                        "pending_source": False,
                    },
                    {
                        "message_def_id": "pacs.009.001.10",
                        "profile_version_count": 3,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": True,
                        "pending_source": False,
                    },
                    {
                        "message_def_id": "sese.023.001.09",
                        "profile_version_count": 1,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "sese.023.001.11",
                        "profile_version_count": 2,
                        "reviewed_missing_schema_fixture": True,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "sese.024.001.09",
                        "profile_version_count": 2,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "sese.024.001.10",
                        "profile_version_count": 2,
                        "reviewed_missing_schema_fixture": True,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "sese.025.001.08",
                        "profile_version_count": 2,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                    {
                        "message_def_id": "sese.025.001.10",
                        "profile_version_count": 3,
                        "reviewed_missing_schema_fixture": False,
                        "reviewed_schema_only": False,
                        "blocked_source": False,
                        "pending_source": True,
                    },
                ],
            )
            self.assertEqual(summary["unreviewed_profile_schema_message_id_count"], 0)
            self.assertEqual(summary["unreviewed_profile_schema_message_ids"], [])

    def test_xml_schema_validation_bounds_xmllint_output(self):
        cases = [
            (
                "failed",
                (1, "", False, "E" * 32, True, False),
                "xmllint output truncated",
            ),
            (
                "successful",
                (0, "O" * 32, True, "", False, False),
                "xmllint output exceeded",
            ),
        ]
        for name, result, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                manifest_path = write_minimal_tree(root, minimal_manifest())
                original_which = VERIFIER.shutil.which
                original_run = VERIFIER._run_command_bounded
                VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
                VERIFIER._run_command_bounded = (
                    lambda *_args, result=result, **_kwargs: result
                )
                try:
                    rc, _stdout, stderr = run_verify(
                        ["--manifest", str(manifest_path), "--validate-xml-schema"]
                    )
                finally:
                    VERIFIER.shutil.which = original_which
                    VERIFIER._run_command_bounded = original_run

                self.assertEqual(rc, 2)
                self.assertIn(expected, stderr)

    def test_xmllint_failure_output_redacts_secret_material_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            cases = (
                ("schema validator echoed token=xmllint-secret", "token="),
                (
                    "schema validator echoed token-xmllint-identifier-secret",
                    "token-xmllint-identifier-secret",
                ),
            )
            for leaked_output, leaked_marker in cases:
                with self.subTest(leaked_output=leaked_output):
                    original_which = VERIFIER.shutil.which
                    original_run = VERIFIER._run_command_bounded
                    VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
                    VERIFIER._run_command_bounded = lambda *_args, **_kwargs: (
                        1,
                        "",
                        False,
                        leaked_output,
                        False,
                        False,
                    )
                    try:
                        rc, stdout, stderr = run_verify(
                            ["--manifest", str(manifest_path), "--validate-xml-schema"]
                        )
                    finally:
                        VERIFIER.shutil.which = original_which
                        VERIFIER._run_command_bounded = original_run

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("failed XML schema validation", stderr)
                    self.assertIn("xmllint output redacted", stderr)
                    self.assertNotIn(leaked_marker, stderr)
                    self.assertNotIn("xmllint-secret", stderr)
                    self.assertNotIn("xmllint-identifier-secret", stderr)

    def test_xmllint_failure_output_redacts_local_paths_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            schema_path = manifest_path.resolve().parent / "iso/fooo.001.001.01.xsd"
            fixture_path = manifest_path.resolve().parent / "../foo_fixture.xml"
            cases = (
                ("schema path", f"{schema_path}: schema validation warning"),
                ("fixture path", f"{fixture_path}: fixture validation warning"),
            )
            for name, leaked_output in cases:
                with self.subTest(name=name):
                    original_which = VERIFIER.shutil.which
                    original_run = VERIFIER._run_command_bounded
                    VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
                    VERIFIER._run_command_bounded = lambda *_args, **_kwargs: (
                        1,
                        "",
                        False,
                        leaked_output,
                        False,
                        False,
                    )
                    try:
                        rc, stdout, stderr = run_verify(
                            ["--manifest", str(manifest_path), "--validate-xml-schema"]
                        )
                    finally:
                        VERIFIER.shutil.which = original_which
                        VERIFIER._run_command_bounded = original_run

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("failed XML schema validation", stderr)
                    self.assertIn("xmllint output redacted: local paths", stderr)
                    self.assertNotIn(str(root), stderr)

    def test_xmllint_diagnostics_redact_control_characters_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            cases = (
                ("failed stderr escape", 1, "", "schema validator \x1b[31mwarning"),
                ("failed stdout nul", 1, "schema validator \x00warning", ""),
                ("failed stderr bidi", 1, "", "schema validator \u202ebidi-warning"),
                ("failed stdout bidi", 1, "schema validator \u202ebidi-warning", ""),
                ("successful stderr escape", 0, "", "schema validator \x1b[31mwarning"),
                ("successful stdout nul", 0, "schema validator \x00warning", ""),
                ("successful stderr bidi", 0, "", "schema validator \u202ebidi-warning"),
                ("successful stdout bidi", 0, "schema validator \u202ebidi-warning", ""),
            )
            for name, returncode, fake_stdout, fake_stderr in cases:
                with self.subTest(name=name):
                    original_which = VERIFIER.shutil.which
                    original_run = VERIFIER._run_command_bounded
                    VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
                    VERIFIER._run_command_bounded = lambda *_args, **_kwargs: (
                        returncode,
                        fake_stdout,
                        False,
                        fake_stderr,
                        False,
                        False,
                    )
                    try:
                        rc, stdout, stderr = run_verify(
                            ["--manifest", str(manifest_path), "--validate-xml-schema"]
                        )
                    finally:
                        VERIFIER.shutil.which = original_which
                        VERIFIER._run_command_bounded = original_run

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("xmllint output redacted: control characters", stderr)
                    self.assertNotIn("\x1b", stderr)
                    self.assertNotIn("\x00", stderr)
                    self.assertNotIn("\u202e", stderr)
                    self.assertNotIn("[31mwarning", stderr)
                    self.assertNotIn("bidi-warning", stderr)
                    self.assertNotIn("schema validator", stderr)

    def test_xmllint_success_output_must_be_expected_validation_line(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            fixture_path = manifest_path.resolve().parent / "../foo_fixture.xml"
            schema_path = manifest_path.resolve().parent / "iso/fooo.001.001.01.xsd"
            expected_success = f"{fixture_path} validates"
            cases = (
                ("allowed stderr", "", expected_success, 0, ""),
                ("allowed stdout", expected_success, "", 0, ""),
                ("warning stderr", "", "schema validator warning", 2, "unexpected output"),
                ("warning stdout", "schema validator warning", "", 2, "unexpected output"),
                (
                    "local fixture path stderr",
                    "",
                    f"{fixture_path}: validator warning",
                    2,
                    "xmllint output redacted: local paths",
                ),
                (
                    "local schema path stdout",
                    f"{schema_path}: validator warning",
                    "",
                    2,
                    "xmllint output redacted: local paths",
                ),
                (
                    "secret stderr",
                    "",
                    "schema validator echoed token=xmllint-secret",
                    2,
                    "xmllint output redacted",
                ),
                (
                    "secret identifier stdout",
                    "schema validator echoed token-xmllint-identifier-secret",
                    "",
                    2,
                    "xmllint output redacted",
                ),
            )
            for name, fake_stdout, fake_stderr, expected_rc, expected_error in cases:
                with self.subTest(name=name):
                    original_which = VERIFIER.shutil.which
                    original_run = VERIFIER._run_command_bounded
                    VERIFIER.shutil.which = lambda command: "/usr/bin/xmllint"
                    VERIFIER._run_command_bounded = lambda *_args, **_kwargs: (
                        0,
                        fake_stdout,
                        False,
                        fake_stderr,
                        False,
                        False,
                    )
                    try:
                        rc, stdout, stderr = run_verify(
                            ["--manifest", str(manifest_path), "--validate-xml-schema"]
                        )
                    finally:
                        VERIFIER.shutil.which = original_which
                        VERIFIER._run_command_bounded = original_run

                    self.assertEqual(rc, expected_rc)
                    if expected_rc == 0:
                        self.assertEqual(stderr, "")
                        self.assertNotEqual(stdout, "")
                    else:
                        self.assertEqual(stdout, "")
                        self.assertIn(expected_error, stderr)
                        self.assertNotIn("token=", stderr)
                        self.assertNotIn("xmllint-secret", stderr)
                        self.assertNotIn("xmllint-identifier-secret", stderr)
                        self.assertNotIn(str(root), stderr)

    def test_boolean_xmllint_output_limit_is_rejected(self):
        with self.assertRaisesRegex(
            VERIFIER.FixtureManifestError,
            "output limit bytes must be positive",
        ):
            VERIFIER._run_command_bounded(
                [sys.executable, "-c", "print('ok')"],
                True,
                1.0,
            )

    def test_xmllint_startup_failure_is_controlled_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            hidden = "token=xmllint-startup-secret"

            def raising_popen(*_args, **_kwargs):
                raise OSError(hidden)

            original_which = VERIFIER.shutil.which
            original_popen = VERIFIER.subprocess.Popen
            VERIFIER.shutil.which = lambda command: str(root / hidden / "xmllint")
            VERIFIER.subprocess.Popen = raising_popen
            try:
                with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                    VERIFIER._run_command_bounded(
                        [str(root / hidden / "xmllint")],
                        128,
                        1.0,
                    )
                message = str(caught.exception)
                self.assertIn("xmllint could not be started", message)
                self.assertNotIn(str(root), message)
                self.assertNotIn(hidden, message)
                self.assertIsNone(caught.exception.__cause__)
                self.assertTrue(caught.exception.__suppress_context__)

                rc, stdout, stderr = run_verify(
                    ["--manifest", str(manifest_path), "--validate-xml-schema"]
                )
            finally:
                VERIFIER.shutil.which = original_which
                VERIFIER.subprocess.Popen = original_popen

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("xmllint could not be started", stderr)
            self.assertNotIn(str(root), stderr)
            self.assertNotIn(hidden, stderr)

    def test_xmllint_output_read_failure_is_controlled_without_echo(self):
        hidden = "token=xmllint-pipe-secret"

        def raising_read(*_args, **_kwargs):
            raise OSError(hidden)

        original_read = VERIFIER._read_limited_pipe
        VERIFIER._read_limited_pipe = raising_read
        try:
            with self.assertRaises(VERIFIER.FixtureManifestError) as caught:
                VERIFIER._run_command_bounded(
                    [sys.executable, "-c", "print('ok')"],
                    128,
                    1.0,
                )
        finally:
            VERIFIER._read_limited_pipe = original_read

        message = str(caught.exception)
        self.assertIn("xmllint output could not be read", message)
        self.assertNotIn(hidden, message)
        self.assertIsNone(caught.exception.__cause__)
        self.assertTrue(caught.exception.__suppress_context__)

    def test_xml_schema_validation_bounds_xmllint_runtime(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            fake_xmllint = root / "xmllint"
            fake_xmllint.write_text(
                "\n".join(
                    [
                        "#!/usr/bin/env python3",
                        "import sys",
                        "import time",
                        "sys.stdout.write('started')",
                        "sys.stdout.flush()",
                        "time.sleep(5)",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            fake_xmllint.chmod(0o700)
            original_which = VERIFIER.shutil.which
            VERIFIER.shutil.which = lambda command: str(fake_xmllint)
            try:
                rc, stdout, stderr = run_verify(
                    [
                        "--manifest",
                        str(manifest_path),
                        "--validate-xml-schema",
                        "--xmllint-timeout-secs",
                        "1",
                    ]
                )
            finally:
                VERIFIER.shutil.which = original_which

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("xmllint timed out after 1 seconds", stderr)

    def test_xmllint_timeout_cli_rejects_nonpositive_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            for value in ("0", "-1", "nan", "inf"):
                with self.subTest(value=value):
                    rc, stdout, stderr = run_verify(
                        [
                            "--manifest",
                            str(manifest_path),
                            "--xmllint-timeout-secs",
                            value,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("positive finite number", stderr)

    def test_xmllint_timeout_cli_rejects_overlarge_values_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            hidden = "9" * 64

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--xmllint-timeout-secs",
                    hidden,
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must be no more than 300 seconds", stderr)
            self.assertNotIn(hidden, stderr)

    @unittest.skipUnless(shutil.which("xmllint"), "xmllint is required for XSD validation")
    def test_xml_schema_validation_flag_validates_schema_backed_fixtures(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--validate-xml-schema",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["strict"]["validate_xml_schema"])
            self.assertEqual(summary["schema_validated_fixtures"], 1)
            self.assertTrue(summary["fixtures"][0]["schema_validated"])

    @unittest.skipUnless(shutil.which("xmllint"), "xmllint is required for XSD validation")
    def test_xml_schema_validation_rejects_schema_invalid_fixture(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            (root / "foo_fixture.xml").write_text(
                fixture_xml("fooo.001.001.01", "FooPayload").replace(
                    "<payload:FooPayload/>",
                    "<payload:FooPayload><payload:Unexpected/></payload:FooPayload>",
                ),
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(
                ["--manifest", str(manifest_path), "--validate-xml-schema"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("failed XML schema validation", stderr)

    def test_schema_target_namespace_payload_and_element_form_drift_are_rejected(self):
        for mutate, message in [
            (
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload", target_message_id="barr.001.001.01"),
                    manifest_path=manifest_path,
                ),
                "targetNamespace",
            ),
            (
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "DifferentPayload"),
                    manifest_path=manifest_path,
                ),
                "payload root",
            ),
            (
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload", element_form="unqualified"),
                    manifest_path=manifest_path,
                ),
                "elementFormDefault",
            ),
            (
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01">',
                        (
                            'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"\n'
                            '           attributeFormDefault="qualified">'
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "xs:schema root must declare exactly elementFormDefault, targetNamespace",
            ),
            (
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload")
                    .replace(
                        'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
                        (
                            'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                            '           xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"'
                        ),
                        1,
                    )
                    .replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01">',
                        (
                            'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"\n'
                            '           xsi:schemaLocation="urn:example:external external.xsd">'
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "xs:schema root must declare exactly elementFormDefault, targetNamespace",
            ),
        ]:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_secret_looking_schema_target_namespace_is_rejected_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            rewrite_schema(
                root,
                "fooo.001.001.01",
                xsd_text("fooo.001.001.01", "FooPayload").replace(
                    'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"',
                    'targetNamespace="urn:iso:std:iso:20022:tech:xsd:token_xsd_namespace_secret"',
                    1,
                ),
                manifest_path=manifest_path,
            )

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("targetNamespace must not contain secret-looking material", stderr)
            self.assertNotIn("token_xsd_namespace_secret", stderr)

    def test_non_ascii_schema_and_fixture_identifiers_are_rejected_without_echo(self):
        non_ascii_zero = "\uff10"
        non_ascii_name = "\u00e9"
        non_ascii_attr = f"attr{non_ascii_name}"

        def set_manifest_payload_root(manifest_path, section, value):
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest[section][0]["payload_root"] = value
            write_json(manifest_path, manifest)

        cases = [
            (
                "schema_target_namespace",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"',
                        (
                            "targetNamespace="
                            '"urn:iso:std:iso:20022:tech:xsd:fooo.001.001.'
                            f'{non_ascii_zero}1"'
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "targetNamespace must use printable ASCII",
                non_ascii_zero,
            ),
            (
                "schema_payload_name",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        f'<xs:element name="Foo{non_ascii_name}ayload" type="FooPayload"/>',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element name must use printable ASCII",
                non_ascii_name,
            ),
            (
                "schema_payload_type",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload")
                    .replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        f'<xs:element name="FooPayload" type="Foo{non_ascii_name}ayload"/>',
                        1,
                    )
                    .replace(
                        '<xs:complexType name="FooPayload">',
                        f'<xs:complexType name="Foo{non_ascii_name}ayload">',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element type must use printable ASCII",
                non_ascii_name,
            ),
            (
                "schema_attribute_name",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01">',
                        (
                            'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"\n'
                            f'           {non_ascii_attr}="value">'
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "unexpected attributes",
                non_ascii_attr,
            ),
            (
                "manifest_schema_payload_root",
                lambda _root, manifest_path: set_manifest_payload_root(
                    manifest_path,
                    "schemas",
                    f"Foo{non_ascii_name}ayload",
                ),
                "schemas[0].payload_root must use printable ASCII",
                non_ascii_name,
            ),
            (
                "manifest_fixture_payload_root",
                lambda _root, manifest_path: set_manifest_payload_root(
                    manifest_path,
                    "fixtures",
                    f"Foo{non_ascii_name}ayload",
                ),
                "fixtures[0].payload_root must use printable ASCII",
                non_ascii_name,
            ),
            (
                "fixture_namespace",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml(f"fooo.001.001.{non_ascii_zero}1", "FooPayload"),
                    encoding="utf-8",
                ),
                "XML fixture namespace must use printable ASCII",
                non_ascii_zero,
            ),
            (
                "fixture_payload_root",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml("fooo.001.001.01", f"Foo{non_ascii_name}ayload"),
                    encoding="utf-8",
                ),
                "XML fixture[0] element must use printable ASCII",
                non_ascii_name,
            ),
        ]
        for case_name, mutate, message, hidden in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_overlong_schema_and_fixture_identifiers_are_rejected_without_echo(self):
        hidden = "A" * (VERIFIER.MAX_XML_IDENTIFIER_CHARS + 1)
        long_payload_root = f"Foo{hidden}Payload"
        too_long_message = (
            f"must be no longer than {VERIFIER.MAX_XML_IDENTIFIER_CHARS} characters"
        )

        def set_manifest_payload_root(manifest_path, section, value):
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest[section][0]["payload_root"] = value
            write_json(manifest_path, manifest)

        cases = [
            (
                "schema_target_namespace",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"',
                        (
                            'targetNamespace="urn:iso:std:iso:20022:tech:xsd:'
                            f"{hidden}\""
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "targetNamespace " + too_long_message,
            ),
            (
                "schema_payload_name",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        f'<xs:element name="{long_payload_root}" type="FooPayload"/>',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element name " + too_long_message,
            ),
            (
                "schema_payload_type",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload")
                    .replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        f'<xs:element name="FooPayload" type="{long_payload_root}"/>',
                        1,
                    )
                    .replace(
                        '<xs:complexType name="FooPayload">',
                        f'<xs:complexType name="{long_payload_root}">',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element type " + too_long_message,
            ),
            (
                "schema_attribute_name",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01">',
                        (
                            'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"\n'
                            f'           {hidden}="value">'
                        ),
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "unexpected attributes",
            ),
            (
                "manifest_schema_payload_root",
                lambda _root, manifest_path: set_manifest_payload_root(
                    manifest_path,
                    "schemas",
                    long_payload_root,
                ),
                "schemas[0].payload_root " + too_long_message,
            ),
            (
                "manifest_fixture_payload_root",
                lambda _root, manifest_path: set_manifest_payload_root(
                    manifest_path,
                    "fixtures",
                    long_payload_root,
                ),
                "fixtures[0].payload_root " + too_long_message,
            ),
            (
                "fixture_namespace",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml(f"fooo.001.001.01{hidden}", "FooPayload"),
                    encoding="utf-8",
                ),
                "XML fixture namespace " + too_long_message,
            ),
            (
                "fixture_payload_root",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml("fooo.001.001.01", long_payload_root),
                    encoding="utf-8",
                ),
                "XML fixture[0] element " + too_long_message,
            ),
        ]
        for case_name, mutate, message in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(long_payload_root, stderr)

    def test_xmllint_non_ascii_diagnostics_are_redacted_without_echo(self):
        hidden = "\u00e9"

        detail = VERIFIER._xmllint_output_detail(
            f"schema validation failed near Foo{hidden}Payload\n"
        )

        self.assertEqual(detail, "[xmllint output redacted: non-ASCII material]")
        self.assertNotIn(hidden, detail)

    def test_secret_looking_fixture_xml_content_is_rejected_without_echo(self):
        cases = [
            (
                "namespace",
                lambda xml: xml.replace(
                    "urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01",
                    "urn:iso:std:iso:20022:tech:xsd:token_xsd_fixture_namespace_secret",
                    1,
                ),
                "token_xsd_fixture_namespace_secret",
            ),
            (
                "nested_element",
                lambda xml: xml.replace(
                    "<payload:FooPayload/>",
                    "<payload:FooPayload><payload:token_xsd_nested_secret/></payload:FooPayload>",
                    1,
                ),
                "token_xsd_nested_secret",
            ),
            (
                "nested_attribute",
                lambda xml: xml.replace(
                    "<payload:FooPayload/>",
                    (
                        "<payload:FooPayload>"
                        '<payload:Nested token-secret="redacted"/>'
                        "</payload:FooPayload>"
                    ),
                    1,
                ),
                "token-secret",
            ),
            (
                "nested_text",
                lambda xml: xml.replace(
                    "<payload:FooPayload/>",
                    "<payload:FooPayload>token=xsd-fixture-text-secret</payload:FooPayload>",
                    1,
                ),
                "xsd-fixture-text-secret",
            ),
        ]
        for case_name, mutate, secret in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    (root / "foo_fixture.xml").write_text(
                        mutate(fixture_xml("fooo.001.001.01", "FooPayload")),
                        encoding="utf-8",
                    )

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_looking_schema_and_fixture_payload_roots_are_rejected_without_echo(self):
        cases = [
            (
                "schema_payload_name",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        '<xs:element name="token_xsd_schema_payload_secret" type="FooPayload"/>',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element name must not contain secret-looking material",
                "token_xsd_schema_payload_secret",
            ),
            (
                "schema_payload_type",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    xsd_text("fooo.001.001.01", "FooPayload").replace(
                        '<xs:element name="FooPayload" type="FooPayload"/>',
                        '<xs:element name="FooPayload" type="token_xsd_schema_type_secret"/>',
                        1,
                    ),
                    manifest_path=manifest_path,
                ),
                "Document payload element type must not contain secret-looking material",
                "token_xsd_schema_type_secret",
            ),
            (
                "fixture_payload_root",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml("fooo.001.001.01", "FooPayload").replace(
                        "FooPayload",
                        "token_xsd_fixture_payload_secret",
                        1,
                    ),
                    encoding="utf-8",
                ),
                "XML fixture[0] element must not contain secret-looking material",
                "token_xsd_fixture_payload_secret",
            ),
        ]
        for case_name, mutate, message, secret in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret, stderr)

    def test_secret_looking_schema_attribute_names_are_rejected_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            rewrite_schema(
                root,
                "fooo.001.001.01",
                xsd_text("fooo.001.001.01", "FooPayload").replace(
                    'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01">',
                    (
                        'targetNamespace="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"\n'
                        '           token-secret="operator-value">'
                    ),
                    1,
                ),
                manifest_path=manifest_path,
            )

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("unexpected attributes", stderr)
            self.assertNotIn("token-secret", stderr)

    def test_schema_document_declarations_must_be_unambiguous(self):
        prefixed_document_type = xsd_text("fooo.001.001.01", "FooPayload").replace(
            'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
            (
                'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                '           xmlns:evil="urn:example:evil"'
            ),
            1,
        ).replace(
            '<xs:element name="Document" type="Document"/>',
            '<xs:element name="Document" type="evil:Document"/>',
            1,
        )
        optional_document = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="Document" type="Document"/>',
            '<xs:element name="Document" type="Document" minOccurs="0"/>',
            1,
        )
        duplicate_document = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="Document" type="Document"/>',
            (
                '<xs:element name="Document" type="Document"/>\n'
                '  <xs:element name="Document" type="OtherDocument"/>'
            ),
            1,
        )
        duplicate_complex = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '</xs:complexType>\n  <xs:complexType name="FooPayload">',
            (
                '</xs:complexType>\n'
                '  <xs:complexType name="Document"><xs:sequence/></xs:complexType>\n'
                '  <xs:complexType name="FooPayload">'
            ),
            1,
        )
        duplicate_sequence = xsd_text("fooo.001.001.01", "FooPayload").replace(
            "</xs:sequence>\n  </xs:complexType>",
            "</xs:sequence>\n    <xs:sequence/>\n  </xs:complexType>",
            1,
        )
        cases = [
            (prefixed_document_type, "Document element type must be exactly 'Document'"),
            (optional_document, "Document element must declare exactly name, type"),
            (duplicate_document, "exactly one top-level xs:element name='Document'"),
            (duplicate_complex, "exactly one document xs:complexType"),
            (duplicate_sequence, "Document complex type must contain only one direct xs:sequence"),
        ]
        for content, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    rewrite_schema(
                        root,
                        "fooo.001.001.01",
                        content,
                        manifest_path=manifest_path,
                    )

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_schema_document_payload_element_must_be_direct_and_local(self):
        extra_document_choice = xsd_text("fooo.001.001.01", "FooPayload").replace(
            "</xs:sequence>\n  </xs:complexType>",
            (
                "</xs:sequence>\n"
                '    <xs:choice><xs:element name="Other" type="Other"/></xs:choice>\n'
                "  </xs:complexType>"
            ),
            1,
        )
        extra_sequence_any = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="FooPayload" type="FooPayload"/>',
            (
                '<xs:element name="FooPayload" type="FooPayload"/>\n'
                '      <xs:any namespace="##other" processContents="lax"/>'
            ),
            1,
        )
        payload_ref = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="FooPayload" type="FooPayload"/>',
            '<xs:element name="FooPayload" ref="evil:FooPayload" type="FooPayload"/>',
            1,
        ).replace(
            'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
            (
                'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                '           xmlns:evil="urn:example:evil"'
            ),
            1,
        )
        optional_payload = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="FooPayload" type="FooPayload"/>',
            '<xs:element name="FooPayload" type="FooPayload" minOccurs="0"/>',
            1,
        )
        missing_payload_type = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="FooPayload" type="FooPayload"/>',
            '<xs:element name="FooPayload"/>',
            1,
        )
        prefixed_payload_type = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '<xs:element name="FooPayload" type="FooPayload"/>',
            '<xs:element name="FooPayload" type="evil:FooPayload"/>',
            1,
        ).replace(
            'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
            (
                'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                '           xmlns:evil="urn:example:evil"'
            ),
            1,
        )
        missing_payload_complex = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '  <xs:complexType name="FooPayload">\n    <xs:sequence/>\n  </xs:complexType>\n',
            "",
            1,
        )
        duplicate_payload_complex = xsd_text("fooo.001.001.01", "FooPayload").replace(
            "</xs:schema>",
            (
                '  <xs:complexType name="FooPayload"><xs:sequence/></xs:complexType>\n'
                "</xs:schema>"
            ),
            1,
        )
        payload_complex_without_sequence = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '  <xs:complexType name="FooPayload">\n    <xs:sequence/>\n  </xs:complexType>\n',
            '  <xs:complexType name="FooPayload"/>\n',
            1,
        )
        payload_complex_duplicate_sequence = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '  <xs:complexType name="FooPayload">\n    <xs:sequence/>\n  </xs:complexType>\n',
            (
                '  <xs:complexType name="FooPayload">\n'
                '    <xs:sequence/>\n'
                '    <xs:sequence/>\n'
                '  </xs:complexType>\n'
            ),
            1,
        )
        payload_complex_extra_choice = xsd_text("fooo.001.001.01", "FooPayload").replace(
            '  <xs:complexType name="FooPayload">\n    <xs:sequence/>\n  </xs:complexType>\n',
            (
                '  <xs:complexType name="FooPayload">\n'
                '    <xs:sequence/>\n'
                '    <xs:choice><xs:element name="Other" type="Other"/></xs:choice>\n'
                '  </xs:complexType>\n'
            ),
            1,
        )
        cases = [
            (
                extra_document_choice,
                "Document complex type must contain only one direct xs:sequence",
            ),
            (
                extra_sequence_any,
                "Document sequence must contain only one direct xs:element",
            ),
            (payload_ref, "Document payload element must declare exactly name, type"),
            (optional_payload, "Document payload element must declare exactly name, type"),
            (missing_payload_type, "Document payload element must declare exactly name, type"),
            (
                prefixed_payload_type,
                "Document payload element type must be local and unprefixed",
            ),
            (missing_payload_complex, "has no payload xs:complexType"),
            (
                duplicate_payload_complex,
                "must contain exactly one payload xs:complexType",
            ),
            (
                payload_complex_without_sequence,
                "payload complex type must contain only one direct xs:sequence",
            ),
            (
                payload_complex_duplicate_sequence,
                "payload complex type must contain only one direct xs:sequence",
            ),
            (
                payload_complex_extra_choice,
                "payload complex type must contain only one direct xs:sequence",
            ),
        ]
        for content, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    rewrite_schema(
                        root,
                        "fooo.001.001.01",
                        content,
                        manifest_path=manifest_path,
                    )

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_schema_complex_type_diagnostics_do_not_echo_payload_type(self):
        hidden_payload = "HiddenSettlementPayload"
        cases = (
            (
                "missing",
                xsd_text("fooo.001.001.01", hidden_payload).replace(
                    (
                        f'  <xs:complexType name="{hidden_payload}">\n'
                        "    <xs:sequence/>\n"
                        "  </xs:complexType>\n"
                    ),
                    "",
                    1,
                ),
                "has no payload xs:complexType",
            ),
            (
                "duplicate",
                xsd_text("fooo.001.001.01", hidden_payload).replace(
                    "</xs:schema>",
                    (
                        f'  <xs:complexType name="{hidden_payload}">'
                        "<xs:sequence/></xs:complexType>\n"
                        "</xs:schema>"
                    ),
                    1,
                ),
                "must contain exactly one payload xs:complexType",
            ),
            (
                "unsupported-child",
                xsd_text("fooo.001.001.01", hidden_payload).replace(
                    (
                        f'  <xs:complexType name="{hidden_payload}">\n'
                        "    <xs:sequence/>\n"
                        "  </xs:complexType>\n"
                    ),
                    (
                        f'  <xs:complexType name="{hidden_payload}">\n'
                        "    <xs:choice/>\n"
                        "  </xs:complexType>\n"
                    ),
                    1,
                ),
                "payload complex type must contain only one direct xs:sequence",
            ),
        )
        for name, content, message in cases:
            with self.subTest(case=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    manifest["schemas"][0]["payload_root"] = hidden_payload
                    manifest_path = write_minimal_tree(root, manifest)
                    rewrite_schema(
                        root,
                        "fooo.001.001.01",
                        content,
                        manifest_path=manifest_path,
                    )

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden_payload, stderr)

    def test_schema_composition_and_foreign_children_are_rejected(self):
        base_schema = xsd_text("fooo.001.001.01", "FooPayload")

        def with_evil_namespace(content):
            return content.replace(
                'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
                (
                    'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                    '           xmlns:evil="urn:example:evil"'
                ),
                1,
            )

        cases = [
            (
                base_schema.replace(
                    '  <xs:element name="Document" type="Document"/>',
                    (
                        '  <xs:import namespace="urn:example:external" '
                        'schemaLocation="external.xsd"/>\n'
                        '  <xs:element name="Document" type="Document"/>'
                    ),
                    1,
                ),
                "xs:schema must not contain xs:import",
            ),
            (
                base_schema.replace(
                    '  <xs:element name="Document" type="Document"/>',
                    (
                        '  <xs:include schemaLocation="shared.xsd"/>\n'
                        '  <xs:element name="Document" type="Document"/>'
                    ),
                    1,
                ),
                "xs:schema must not contain xs:include",
            ),
            (
                base_schema.replace(
                    '  <xs:element name="Document" type="Document"/>',
                    (
                        '  <xs:redefine schemaLocation="shared.xsd"/>\n'
                        '  <xs:element name="Document" type="Document"/>'
                    ),
                    1,
                ),
                "xs:schema must not contain xs:redefine",
            ),
            (
                base_schema.replace(
                    '  <xs:element name="Document" type="Document"/>',
                    (
                        '  <xs:override schemaLocation="shared.xsd"/>\n'
                        '  <xs:element name="Document" type="Document"/>'
                    ),
                    1,
                ),
                "xs:schema must not contain xs:override",
            ),
            (
                with_evil_namespace(base_schema).replace(
                    '  <xs:element name="Document" type="Document"/>',
                    '  <evil:metadata/>\n  <xs:element name="Document" type="Document"/>',
                    1,
                ),
                "xs:schema contains unsupported foreign child",
            ),
            (
                with_evil_namespace(base_schema).replace(
                    '  <xs:complexType name="Document">\n    <xs:sequence>',
                    (
                        '  <xs:complexType name="Document">\n'
                        '    <evil:metadata/>\n'
                        '    <xs:sequence>'
                    ),
                    1,
                ),
                "Document complex type contains unsupported child",
            ),
            (
                with_evil_namespace(base_schema).replace(
                    '<xs:element name="FooPayload" type="FooPayload"/>',
                    (
                        '<xs:element name="FooPayload" type="FooPayload"/>\n'
                        '      <evil:metadata/>'
                    ),
                    1,
                ),
                "Document sequence contains unsupported child",
            ),
            (
                with_evil_namespace(base_schema).replace(
                    '  <xs:complexType name="FooPayload">\n    <xs:sequence/>',
                    (
                        '  <xs:complexType name="FooPayload">\n'
                        '    <evil:metadata/>\n'
                        '    <xs:sequence/>'
                    ),
                    1,
                ),
                "payload complex type contains unsupported child",
            ),
        ]
        for content, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    rewrite_schema(
                        root,
                        "fooo.001.001.01",
                        content,
                        manifest_path=manifest_path,
                    )

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(str(root), stderr)

    def test_secret_looking_schema_foreign_child_namespaces_are_rejected_without_echo(self):
        base_schema = xsd_text("fooo.001.001.01", "FooPayload").replace(
            'xmlns:xs="http://www.w3.org/2001/XMLSchema"',
            (
                'xmlns:xs="http://www.w3.org/2001/XMLSchema"\n'
                '           xmlns:evil="urn:example:token=foreign-child-secret"'
            ),
            1,
        )
        cases = [
            (
                "schema_root",
                base_schema.replace(
                    '  <xs:element name="Document" type="Document"/>',
                    '  <evil:metadata/>\n  <xs:element name="Document" type="Document"/>',
                    1,
                ),
                "xs:schema contains unsupported foreign child",
            ),
            (
                "nested_document",
                base_schema.replace(
                    '  <xs:complexType name="Document">\n    <xs:sequence>',
                    (
                        '  <xs:complexType name="Document">\n'
                        '    <evil:metadata/>\n'
                        '    <xs:sequence>'
                    ),
                    1,
                ),
                "Document complex type contains unsupported child",
            ),
        ]
        for case_name, content, message in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    rewrite_schema(
                        root,
                        "fooo.001.001.01",
                        content,
                        manifest_path=manifest_path,
                    )

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn("token=foreign-child-secret", stderr)

    def test_restricted_schema_redistribution_terms_are_rejected(self):
        cases = (
            """<!--Copyright (c) SWIFT scrl, 2020.

 This is a licensed product, which may only be redistributed upon agreement with SWIFT scrl.
 The user has no right, or right to authorise others, to:
   - rent, lease, or sell this component;
   - display publicly, distribute or otherwise provide this component;
-->
""",
            """<!--Licensed product.
 The user has no
 right, or right to authorise others, to display publicly, distribute or otherwise provide this component.
-->
""",
            """<!--Licensed product.
 This is a licensed product, which may only be redistributed upon
 agreement with SWIFT scrl.
 The user may not rent,\tlease,\tor sell this component.
-->
""",
            """<!--Licensed product.
 This is a licensed product, which may only be redistribu\u200dted upon agreement with SWIFT scrl.
 The user has no right, or right to\u200bauthorise others, to distribute this component.
-->
""",
        )
        for offset, restricted_terms in enumerate(cases):
            with self.subTest(offset=offset):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    schema_path = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
                    schema_path.write_text(
                        xsd_text("fooo.001.001.01", "FooPayload").replace(
                            "<xs:schema",
                            restricted_terms + "<xs:schema",
                            1,
                        ),
                        encoding="utf-8",
                    )

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                self.assertEqual(rc, 2)
                self.assertIn("restricted redistribution terms", stderr)

    def test_schema_or_fixture_dtd_and_entities_are_rejected(self):
        cases = [
            (
                "schema-doctype",
                lambda root, manifest_path: rewrite_schema(
                    root,
                    "fooo.001.001.01",
                    (
                        '<!DOCTYPE xs:schema [<!ENTITY payload "FooPayload">]>\n'
                        + xsd_text("fooo.001.001.01", "FooPayload")
                    ),
                    manifest_path=manifest_path,
                ),
            ),
            (
                "fixture-doctype",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    '<!DOCTYPE doc:Document [<!ENTITY payload "FooPayload">]>\n'
                    + fixture_xml("fooo.001.001.01", "FooPayload"),
                    encoding="utf-8",
                ),
            ),
            (
                "fixture-entity",
                lambda root, _manifest_path: (root / "foo_fixture.xml").write_text(
                    fixture_xml("fooo.001.001.01", "FooPayload").replace(
                        "</doc:Document>",
                        "<!ENTITY payload \"FooPayload\"></doc:Document>",
                    ),
                    encoding="utf-8",
                ),
            ),
        ]
        for name, mutate in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root, manifest_path)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn("must not contain DTD or entity declarations", stderr)

    def test_schema_source_provenance_shape_and_digest_are_rejected(self):
        cases = []
        missing_source = minimal_manifest()
        missing_source["schemas"][0].pop("source")
        cases.append((missing_source, "source must be recorded"))

        null_source = minimal_manifest()
        null_source["schemas"][0]["source"] = None
        cases.append((null_source, "source must be a JSON object"))

        unknown_source_key = minimal_manifest()
        unknown_source_key["schemas"][0]["source"]["unexpected"] = "value"
        cases.append((unknown_source_key, "unknown keys"))

        bad_repository = minimal_manifest()
        bad_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/example/iso20022-fixtures.git"
        )
        cases.append((bad_repository, "repository must be a canonical"))

        uppercase_repository = minimal_manifest()
        uppercase_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/Moov-IO/fedwire20022"
        )
        cases.append((uppercase_repository, "repository must be a canonical"))

        underscore_owner_repository = minimal_manifest()
        underscore_owner_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov_io/fedwire20022"
        )
        cases.append((underscore_owner_repository, "repository must be a canonical"))

        leading_hyphen_owner_repository = minimal_manifest()
        leading_hyphen_owner_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/-moov-io/fedwire20022"
        )
        cases.append((leading_hyphen_owner_repository, "repository must be a canonical"))

        trailing_hyphen_owner_repository = minimal_manifest()
        trailing_hyphen_owner_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io-/fedwire20022"
        )
        cases.append((trailing_hyphen_owner_repository, "repository must be a canonical"))

        punctuation_only_name_repository = minimal_manifest()
        punctuation_only_name_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/---"
        )
        cases.append((punctuation_only_name_repository, "repository must be a canonical"))

        leading_punctuation_name_repository = minimal_manifest()
        leading_punctuation_name_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/-fedwire20022"
        )
        cases.append(
            (leading_punctuation_name_repository, "repository must be a canonical")
        )

        trailing_punctuation_name_repository = minimal_manifest()
        trailing_punctuation_name_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/fedwire20022."
        )
        cases.append(
            (trailing_punctuation_name_repository, "repository must be a canonical")
        )

        placeholder_repository_owner = minimal_manifest()
        placeholder_repository_owner["schemas"][0]["source"]["repository"] = (
            "https://github.com/example/iso20022-fixtures"
        )
        cases.append(
            (
                placeholder_repository_owner,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name = minimal_manifest()
        placeholder_repository_name["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/iso20022-template-fixtures"
        )
        cases.append(
            (
                placeholder_repository_name,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name_separated = minimal_manifest()
        placeholder_repository_name_separated["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/iso20022-replace_before_production-fixtures"
        )
        cases.append(
            (
                placeholder_repository_name_separated,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name_collapsed = minimal_manifest()
        placeholder_repository_name_collapsed["schemas"][0]["source"]["repository"] = (
            "https://github.com/moov-io/operatorcanarybank"
        )
        cases.append(
            (
                placeholder_repository_name_collapsed,
                "repository must not use placeholder repository coordinates",
            )
        )

        long_repository = minimal_manifest()
        long_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/example/" + ("a" * VERIFIER.MAX_SOURCE_REPOSITORY_CHARS)
        )
        cases.append((long_repository, "repository must be no longer than 2048 characters"))

        bad_commit = minimal_manifest()
        bad_commit["schemas"][0]["source"]["commit"] = (
            "0123456789abcdef0123456789abcdef0123456Z"
        )
        cases.append((bad_commit, "commit must be a lowercase 40-hex"))

        all_zero_commit = minimal_manifest()
        all_zero_commit["schemas"][0]["source"]["commit"] = "0" * 40
        cases.append((all_zero_commit, "commit must not be all zero"))

        escaped_path = minimal_manifest()
        escaped_path["schemas"][0]["source"]["path"] = "../fooo.001.001.01.xsd"
        cases.append((escaped_path, "must not contain empty, dot, or parent segments"))

        whitespace_path = minimal_manifest()
        whitespace_path["schemas"][0]["source"]["path"] = (
            "xsd/iso/fooo source.001.001.01.xsd"
        )
        cases.append((whitespace_path, "source.path must not contain whitespace"))
        dash_path = minimal_manifest()
        dash_path["schemas"][0]["source"]["path"] = "--fooo.001.001.01.xsd"
        cases.append((dash_path, "source.path must not start with a dash"))
        segment_dash_path = minimal_manifest()
        segment_dash_path["schemas"][0]["source"]["path"] = (
            "xsd/iso/--fooo.001.001.01.xsd"
        )
        cases.append(
            (
                segment_dash_path,
                "source.path must not contain leading-dash path segments",
            )
        )

        semicolon_path = minimal_manifest()
        semicolon_path["schemas"][0]["source"]["path"] = (
            "xsd/iso;debug/fooo.001.001.01.xsd"
        )
        cases.append(
            (
                semicolon_path,
                "source.path must not contain semicolon path parameters",
            )
        )

        mismatched_filename = minimal_manifest()
        mismatched_filename["schemas"][0]["source"]["path"] = "xsd/other.001.001.01.xsd"
        cases.append((mismatched_filename, "filename must match message_def_id"))

        unsupported_license = minimal_manifest()
        unsupported_license["schemas"][0]["source"]["license"] = "NOASSERTION"
        cases.append((unsupported_license, "license must be one of"))

        sha_drift = minimal_manifest()
        sha_drift["schemas"][0]["source"]["sha256"] = "f" * 64
        cases.append((sha_drift, "sha256 does not match checked-in XSD bytes"))

        all_zero_sha = minimal_manifest()
        all_zero_sha["schemas"][0]["source"]["sha256"] = "0" * 64
        cases.append((all_zero_sha, "sha256 must not be all zero"))

        for manifest, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                self.assertEqual(rc, 2)
                self.assertIn(message, stderr)

    def test_schema_source_paths_reject_non_ascii_and_overlong_without_echo(self):
        hidden_unicode = "unicod\u0435-source-path"
        hidden_long = "x" * (VERIFIER.MAX_SOURCE_PATH_CHARS + 1)
        cases = (
            (
                lambda manifest: manifest["schemas"][0]["source"].__setitem__(
                    "path",
                    f"xsd/{hidden_unicode}/fooo.001.001.01.xsd",
                ),
                "source.path must use printable ASCII",
                hidden_unicode,
            ),
            (
                lambda manifest: manifest["schemas"][0]["source"].__setitem__(
                    "path",
                    "xsd/" + hidden_long + "/fooo.001.001.01.xsd",
                ),
                "source.path must be no longer than 2048 characters",
                hidden_long,
            ),
            (
                lambda manifest: (
                    manifest.__setitem__(
                        "blocked_schema_sources",
                        [blocked_schema_source()],
                    ),
                    manifest["blocked_schema_sources"][0]["source"].__setitem__(
                        "path",
                        f"xsd/{hidden_unicode}/barr.001.001.01.xsd",
                    ),
                ),
                "source.path must use printable ASCII",
                hidden_unicode,
            ),
            (
                lambda manifest: (
                    manifest.__setitem__(
                        "blocked_schema_sources",
                        [blocked_schema_source()],
                    ),
                    manifest["blocked_schema_sources"][0]["source"].__setitem__(
                        "path",
                        "xsd/" + hidden_long + "/barr.001.001.01.xsd",
                    ),
                ),
                "source.path must be no longer than 2048 characters",
                hidden_long,
            ),
        )
        for offset, (mutate, message, hidden) in enumerate(cases):
            with self.subTest(offset=offset):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_manifest_relative_paths_reject_non_ascii_and_overlong_without_echo(self):
        hidden_unicode = "unicod\u0435-manifest-path"
        hidden_long = "x" * (VERIFIER.MAX_SOURCE_PATH_CHARS + 1)
        cases = (
            (
                lambda manifest: manifest["schemas"][0].__setitem__(
                    "path",
                    f"iso/{hidden_unicode}/fooo.001.001.01.xsd",
                ),
                "schemas[0].path must use printable ASCII",
                hidden_unicode,
            ),
            (
                lambda manifest: manifest["schemas"][0].__setitem__(
                    "path",
                    "iso/" + hidden_long + "/fooo.001.001.01.xsd",
                ),
                "schemas[0].path must be no longer than 2048 characters",
                hidden_long,
            ),
            (
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "path",
                    f"../{hidden_unicode}/foo_fixture.xml",
                ),
                "fixtures[0].path must use printable ASCII",
                hidden_unicode,
            ),
            (
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "path",
                    "../" + hidden_long + "/foo_fixture.xml",
                ),
                "fixtures[0].path must be no longer than 2048 characters",
                hidden_long,
            ),
            (
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "schema",
                    f"iso/{hidden_unicode}/fooo.001.001.01.xsd",
                ),
                "fixtures[0].schema must use printable ASCII",
                hidden_unicode,
            ),
            (
                lambda manifest: manifest["fixtures"][0].__setitem__(
                    "schema",
                    "iso/" + hidden_long + "/fooo.001.001.01.xsd",
                ),
                "fixtures[0].schema must be no longer than 2048 characters",
                hidden_long,
            ),
        )
        for offset, (mutate, message, hidden) in enumerate(cases):
            with self.subTest(offset=offset):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_blocked_schema_source_provenance_and_markers_are_rejected(self):
        cases = []

        missing_source = minimal_manifest()
        missing_source["blocked_schema_sources"] = [blocked_schema_source()]
        missing_source["blocked_schema_sources"][0].pop("source")
        cases.append((missing_source, "source must be recorded"))

        null_source = minimal_manifest()
        null_source["blocked_schema_sources"] = [blocked_schema_source()]
        null_source["blocked_schema_sources"][0]["source"] = None
        cases.append((null_source, "source must be a JSON object"))

        unknown_key = minimal_manifest()
        unknown_key["blocked_schema_sources"] = [blocked_schema_source()]
        unknown_key["blocked_schema_sources"][0]["unexpected"] = True
        cases.append((unknown_key, "unknown keys"))

        bad_repository = minimal_manifest()
        bad_repository["blocked_schema_sources"] = [blocked_schema_source()]
        bad_repository["blocked_schema_sources"][0]["source"]["repository"] = (
            "https://github.com/example/iso20022-blocked.git"
        )
        cases.append((bad_repository, "repository must be a canonical"))

        uppercase_repository = minimal_manifest()
        uppercase_repository["blocked_schema_sources"] = [blocked_schema_source()]
        uppercase_repository["blocked_schema_sources"][0]["source"]["repository"] = (
            "https://github.com/Prog-Nov/iso20022-messages-for-go"
        )
        cases.append((uppercase_repository, "repository must be a canonical"))

        underscore_owner_repository = minimal_manifest()
        underscore_owner_repository["blocked_schema_sources"] = [blocked_schema_source()]
        underscore_owner_repository["blocked_schema_sources"][0]["source"]["repository"] = (
            "https://github.com/prog_nov/iso20022-messages-for-go"
        )
        cases.append((underscore_owner_repository, "repository must be a canonical"))

        leading_hyphen_owner_repository = minimal_manifest()
        leading_hyphen_owner_repository["blocked_schema_sources"] = [blocked_schema_source()]
        leading_hyphen_owner_repository["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/-prog-nov/iso20022-messages-for-go"
        cases.append((leading_hyphen_owner_repository, "repository must be a canonical"))

        trailing_hyphen_owner_repository = minimal_manifest()
        trailing_hyphen_owner_repository["blocked_schema_sources"] = [blocked_schema_source()]
        trailing_hyphen_owner_repository["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov-/iso20022-messages-for-go"
        cases.append((trailing_hyphen_owner_repository, "repository must be a canonical"))

        punctuation_only_name_repository = minimal_manifest()
        punctuation_only_name_repository["blocked_schema_sources"] = [blocked_schema_source()]
        punctuation_only_name_repository["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/___"
        cases.append((punctuation_only_name_repository, "repository must be a canonical"))

        leading_punctuation_name_repository = minimal_manifest()
        leading_punctuation_name_repository["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        leading_punctuation_name_repository["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/-iso20022-messages-for-go"
        cases.append(
            (leading_punctuation_name_repository, "repository must be a canonical")
        )

        trailing_punctuation_name_repository = minimal_manifest()
        trailing_punctuation_name_repository["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        trailing_punctuation_name_repository["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/iso20022-messages-for-go_"
        cases.append(
            (trailing_punctuation_name_repository, "repository must be a canonical")
        )

        placeholder_repository_owner = minimal_manifest()
        placeholder_repository_owner["blocked_schema_sources"] = [blocked_schema_source()]
        placeholder_repository_owner["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/example/iso20022-blocked"
        cases.append(
            (
                placeholder_repository_owner,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name = minimal_manifest()
        placeholder_repository_name["blocked_schema_sources"] = [blocked_schema_source()]
        placeholder_repository_name["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/iso20022-sample-blocked"
        cases.append(
            (
                placeholder_repository_name,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name_separated = minimal_manifest()
        placeholder_repository_name_separated["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        placeholder_repository_name_separated["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/iso20022-replace_before_production-blocked"
        cases.append(
            (
                placeholder_repository_name_separated,
                "repository must not use placeholder repository coordinates",
            )
        )

        placeholder_repository_name_collapsed = minimal_manifest()
        placeholder_repository_name_collapsed["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        placeholder_repository_name_collapsed["blocked_schema_sources"][0]["source"][
            "repository"
        ] = "https://github.com/prog-nov/operatorcanarybank"
        cases.append(
            (
                placeholder_repository_name_collapsed,
                "repository must not use placeholder repository coordinates",
            )
        )

        bad_commit = minimal_manifest()
        bad_commit["blocked_schema_sources"] = [blocked_schema_source()]
        bad_commit["blocked_schema_sources"][0]["source"]["commit"] = (
            "89abcdef0123456789abcdef0123456789abcdeZ"
        )
        cases.append((bad_commit, "commit must be a lowercase 40-hex"))

        all_zero_commit = minimal_manifest()
        all_zero_commit["blocked_schema_sources"] = [blocked_schema_source()]
        all_zero_commit["blocked_schema_sources"][0]["source"]["commit"] = "0" * 40
        cases.append((all_zero_commit, "commit must not be all zero"))

        mismatched_filename = minimal_manifest()
        mismatched_filename["blocked_schema_sources"] = [blocked_schema_source()]
        mismatched_filename["blocked_schema_sources"][0]["source"]["path"] = (
            "xsd/other.001.001.01.xsd"
        )
        cases.append((mismatched_filename, "filename must match message_def_id"))

        bad_sha = minimal_manifest()
        bad_sha["blocked_schema_sources"] = [blocked_schema_source()]
        bad_sha["blocked_schema_sources"][0]["source"]["sha256"] = "not-a-digest"
        cases.append((bad_sha, "sha256 must be a lowercase SHA-256 digest"))

        all_zero_sha = minimal_manifest()
        all_zero_sha["blocked_schema_sources"] = [blocked_schema_source()]
        all_zero_sha["blocked_schema_sources"][0]["source"]["sha256"] = "0" * 64
        cases.append((all_zero_sha, "sha256 must not be all zero"))

        empty_markers = minimal_manifest()
        empty_markers["blocked_schema_sources"] = [blocked_schema_source()]
        empty_markers["blocked_schema_sources"][0]["restriction_markers"] = []
        cases.append((empty_markers, "restriction_markers must not be empty"))

        unsupported_marker = minimal_manifest()
        unsupported_marker["blocked_schema_sources"] = [blocked_schema_source()]
        unsupported_marker["blocked_schema_sources"][0]["restriction_markers"] = [
            "unknown-license-marker"
        ]
        cases.append((unsupported_marker, "must be one of"))

        duplicate_marker = minimal_manifest()
        duplicate_marker["blocked_schema_sources"] = [blocked_schema_source()]
        duplicate_marker["blocked_schema_sources"][0]["restriction_markers"] = [
            "swift-copyright-header",
            "swift-copyright-header",
        ]
        cases.append((duplicate_marker, "duplicates"))

        copyright_only_marker = minimal_manifest()
        copyright_only_marker["blocked_schema_sources"] = [blocked_schema_source()]
        copyright_only_marker["blocked_schema_sources"][0]["restriction_markers"] = [
            "swift-copyright-header"
        ]
        cases.append(
            (
                copyright_only_marker,
                "restriction_markers must include a redistribution restriction marker",
            )
        )

        blocked_checked_in_schema = minimal_manifest()
        blocked_checked_in_schema["blocked_schema_sources"] = [
            blocked_schema_source("fooo.001.001.01")
        ]
        cases.append((blocked_checked_in_schema, "already checked-in schema"))

        duplicate_candidate_digest = minimal_manifest()
        duplicate_candidate_digest["blocked_schema_sources"] = [
            blocked_schema_source("barr.001.001.01"),
            blocked_schema_source("barr.001.001.02"),
        ]
        duplicate_candidate_digest["blocked_schema_sources"][1]["source"]["path"] = (
            "xsd/barr.001.001.02.xsd"
        )
        cases.append((duplicate_candidate_digest, "duplicate candidate SHA-256 values"))

        duplicate_blocked_message_id = minimal_manifest()
        duplicate_blocked_message_id["blocked_schema_sources"] = [
            blocked_schema_source("barr.001.001.01"),
            blocked_schema_source("barr.001.001.01"),
        ]
        duplicate_blocked_message_id["blocked_schema_sources"][1]["source"][
            "repository"
        ] = "https://github.com/moov-io/iso20022"
        duplicate_blocked_message_id["blocked_schema_sources"][1]["source"][
            "path"
        ] = "alternate/barr.001.001.01.xsd"
        duplicate_blocked_message_id["blocked_schema_sources"][1]["source"][
            "sha256"
        ] = "2" * 64
        cases.append(
            (
                duplicate_blocked_message_id,
                "duplicate message_def_id values",
            )
        )

        checked_in_schema_digest_candidate = minimal_manifest()
        checked_in_schema_digest_candidate["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        checked_in_schema_digest_candidate["blocked_schema_sources"][0]["source"][
            "sha256"
        ] = checked_in_schema_digest_candidate["schemas"][0]["source"]["sha256"]
        cases.append(
            (
                checked_in_schema_digest_candidate,
                "candidate SHA-256 values that already identify checked-in schemas",
            )
        )

        checked_in_fixture_digest_candidate = minimal_manifest()
        checked_in_fixture_digest_candidate["blocked_schema_sources"] = [
            blocked_schema_source()
        ]
        checked_in_fixture_digest_candidate["blocked_schema_sources"][0]["source"][
            "sha256"
        ] = VERIFIER.sha256_hex(
            fixture_xml("fooo.001.001.01", "FooPayload").encode("utf-8")
        )
        cases.append(
            (
                checked_in_fixture_digest_candidate,
                "candidate SHA-256 values that already identify checked-in fixtures",
            )
        )

        for manifest, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_blocked_schema_sources_must_be_explicit(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            manifest.pop("blocked_schema_sources")
            manifest_path = write_minimal_tree(root, manifest)

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("blocked_schema_sources must be recorded as an array", stderr)

    def test_pending_schema_sources_must_be_explicit(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            manifest.pop("pending_schema_sources")
            manifest_path = write_minimal_tree(root, manifest)

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("pending_schema_sources must be recorded as an array", stderr)

    def test_pending_schema_sources_record_official_catalogue_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            manifest["fixtures"].append(
                {
                    "path": "../barr_fixture.xml",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "missing_schema_reason": "Official schema package pending.",
                }
            )
            manifest["pending_schema_sources"] = [pending_schema_source()]
            manifest_path = write_minimal_tree(root, manifest)
            (root / "barr_fixture.xml").write_text(
                fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["pending_schema_source_count"], 1)
            self.assertEqual(
                summary["pending_schema_sources"][0]["message_def_id"],
                "barr.001.001.01",
            )

    def test_pending_schema_sources_validate_official_catalogue_shape(self):
        cases = (
            (
                lambda manifest: manifest["pending_schema_sources"].append(
                    pending_schema_source("fooo.001.001.01")
                ),
                "pending_schema_sources includes an already checked-in schema",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].pop(
                        "download_url"
                    ),
                ),
                "source.download_url must be a non-empty string",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://example.com/message/12345/download",
                    ),
                ),
                "source.download_url must be an official ISO 20022 XSD download URL",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/not-a-number/download",
                    ),
                ),
                "source.download_url must be an official ISO 20022 XSD download URL",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/"
                        + ("1" * 2010)
                        + "/download",
                    ),
                ),
                "source.download_url must be no longer than",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/%31345/download",
                    ),
                ),
                "source.download_url must not contain percent escapes",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/12345/download?token=xsd-secret",
                    ),
                ),
                "source.download_url must not contain secret-looking material",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(
                        known_pending_schema_source("colr.012.001.05")
                    ),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/22505/download",
                    ),
                ),
                "source.download_url must match known official ISO pending-source metadata",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(
                        known_pending_schema_source("sese.023.001.11")
                    ),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "message_name",
                        "SecuritiesSettlementTransactionStatusAdviceV11",
                    ),
                ),
                "source.message_name must match known official ISO pending-source metadata",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://example.com/iso-20022-message-definitions",
                    ),
                ),
                "source.catalogue_url must be an official ISO 20022 catalogue URL",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=%38",
                    ),
                ),
                "source.catalogue_url must not contain percent escapes",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page="
                        + ("1" * 1990),
                    ),
                ),
                "source.catalogue_url must be no longer than",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=8&",
                    ),
                ),
                "source.catalogue_url archive URL must set one numeric page",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=08",
                    ),
                ),
                "source.catalogue_url archive URL must set one numeric page",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "catalogue_url",
                        "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=x",
                    ),
                ),
                "source.catalogue_url archive URL must set one numeric page",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "download_type",
                        "PDF",
                    ),
                ),
                "source.download_type must be one of XSD",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "message_name",
                        "Bar PayloadV01",
                    ),
                ),
                "source.message_name must be a canonical ISO message name ending in VNN",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "message_name",
                        "BarPayloadV02",
                    ),
                ),
                "source.message_name version suffix must match message_def_id version",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "submitting_organisation",
                        "SWIFT; FPL",
                    ),
                ),
                "source.submitting_organisation must not contain semicolon path parameters",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "submitting_organisation",
                        "https://www.iso20022.org/SWIFT",
                    ),
                ),
                "source.submitting_organisation must not contain URI or contact delimiters",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "submitting_organisation",
                        "SWIFT//FPL",
                    ),
                ),
                "source.submitting_organisation must use slash only inside organization tokens",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "submitting_organisation",
                        "SWIFT,",
                    ),
                ),
                "source.submitting_organisation must be a comma-space separated list of organization names",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"][0]["source"].__setitem__(
                        "submitting_organisation",
                        "Example Org",
                    ),
                ),
                "source.submitting_organisation must not use placeholder organization metadata",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"].append(
                        pending_schema_source("bazz.001.001.01")
                    ),
                ),
                "pending_schema_sources contains duplicate source provenance",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"].append(
                        pending_schema_source("bazz.001.001.01")
                    ),
                    manifest["pending_schema_sources"][1]["source"].__setitem__(
                        "download_url",
                        "https://www.iso20022.org/message/12346/download",
                    ),
                ),
                "pending_schema_sources contains duplicate message_name values",
            ),
            (
                lambda manifest: (
                    manifest["pending_schema_sources"].append(pending_schema_source()),
                    manifest["pending_schema_sources"].append(
                        pending_schema_source("barr.001.001.02")
                    ),
                    manifest["pending_schema_sources"][1]["source"].__setitem__(
                        "message_name",
                        "DifferentBarPayloadV02",
                    ),
                ),
                "pending_schema_sources contains duplicate download_url values",
            ),
        )
        for mutate, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_reviewed_xsd_gap_reasons_reject_non_ascii_without_echo(self):
        hidden_schema_reason = "reviewed standal\u043ene fixture gap"
        hidden_missing_reason = "reviewed missing schem\u0430 package"
        hidden_blocked_reason = "candidate restricti\u043en requires review"
        cases = (
            (
                lambda manifest: manifest["schemas"][0].update(
                    {"schema_only_reason": hidden_schema_reason}
                ),
                "schemas[0].schema_only_reason must use printable ASCII",
                hidden_schema_reason,
            ),
            (
                lambda manifest: (
                    manifest["fixtures"][0].pop("schema"),
                    manifest["fixtures"][0].update(
                        {"missing_schema_reason": hidden_missing_reason}
                    ),
                ),
                "fixtures[0].missing_schema_reason must use printable ASCII",
                hidden_missing_reason,
            ),
            (
                lambda manifest: manifest.update(
                    {
                        "blocked_schema_sources": [
                            {
                                **blocked_schema_source(),
                                "reason": hidden_blocked_reason,
                            }
                        ]
                    }
                ),
                "blocked_schema_sources[0].reason must use printable ASCII",
                hidden_blocked_reason,
            ),
        )
        for mutate, message, hidden in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_reviewed_xsd_gap_reasons_are_length_capped_without_echo(self):
        hidden = "A" * (VERIFIER.MAX_REVIEWED_GAP_REASON_CHARS + 1)
        cases = (
            (
                lambda manifest: manifest["schemas"][0].update(
                    {"schema_only_reason": hidden}
                ),
                "schemas[0].schema_only_reason must be no longer than 1024 characters",
            ),
            (
                lambda manifest: (
                    manifest["fixtures"][0].pop("schema"),
                    manifest["fixtures"][0].update({"missing_schema_reason": hidden}),
                ),
                "fixtures[0].missing_schema_reason must be no longer than 1024 characters",
            ),
            (
                lambda manifest: manifest.update(
                    {
                        "blocked_schema_sources": [
                            {
                                **blocked_schema_source(),
                                "reason": hidden,
                            }
                        ]
                    }
                ),
                "blocked_schema_sources[0].reason must be no longer than 1024 characters",
            ),
        )
        for mutate, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest = minimal_manifest()
                    mutate(manifest)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_profile_catalog_rejects_blocked_source_without_current_gap(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "barr.001.001.01"
            manifest = minimal_manifest()
            manifest["blocked_schema_sources"] = [blocked_schema_source()]
            manifest_path = write_minimal_tree(root, manifest)
            profile_catalog = write_profile_catalog(root / "profiles.rs")

            rc, _stdout, stderr = run_verify(
                ["--manifest", str(manifest_path), "--profile-catalog", str(profile_catalog)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("blocked_schema_sources includes an entry", stderr)
            self.assertIn("without a current missing schema/profile gap", stderr)
            self.assertNotIn(hidden, stderr)

    def test_profile_only_blocked_source_requires_profile_catalog(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "barr.001.001.01"
            manifest = minimal_manifest()
            manifest["blocked_schema_sources"] = [blocked_schema_source()]
            manifest_path = write_minimal_tree(root, manifest)

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("blocked_schema_sources includes an entry", stderr)
            self.assertIn("pass --profile-catalog to prove profile-version gaps", stderr)
            self.assertNotIn(hidden, stderr)

    def test_missing_fixture_blocked_source_does_not_require_profile_catalog(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            manifest["fixtures"].append(
                {
                    "path": "../barr_fixture.xml",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "missing_schema_reason": "Reviewed missing schema package.",
                }
            )
            manifest["blocked_schema_sources"] = [blocked_schema_source()]
            manifest_path = write_minimal_tree(root, manifest)
            (root / "barr_fixture.xml").write_text(
                fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertIsNone(summary["profile_catalog"])
            self.assertEqual(
                [entry["message_def_id"] for entry in summary["missing_schema_fixtures"]],
                ["barr.001.001.01"],
            )
            self.assertEqual(
                [entry["message_def_id"] for entry in summary["blocked_schema_sources"]],
                ["barr.001.001.01"],
            )

    def test_unbacked_fixture_cannot_claim_checked_in_schema_gap(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "fooo.001.001.01"
            manifest = minimal_manifest()
            manifest["schemas"][0]["schema_only_reason"] = (
                "Reviewed standalone fixture gap."
            )
            manifest["fixtures"][0].pop("schema")
            manifest["fixtures"][0]["missing_schema_reason"] = (
                "Reviewed missing schema package."
            )
            manifest_path = write_minimal_tree(root, manifest)

            rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "missing-schema fixture for an already checked-in schema", stderr
            )
            self.assertNotIn(hidden, stderr)

    def test_fixture_namespace_payload_root_and_document_root_drift_are_rejected(self):
        for xml, message in [
            (fixture_xml("barr.001.001.01", "FooPayload"), "namespace message id"),
            (fixture_xml("fooo.001.001.01", "DifferentPayload"), "payload root"),
            (fixture_xml("fooo.001.001.01", "FooPayload", root="Envelope"), "root element"),
            (
                fixture_xml("fooo.001.001.01", "FooPayload").replace(
                    '<doc:Document xmlns:doc="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01"',
                    (
                        '<doc:Document xmlns:doc="urn:iso:std:iso:20022:tech:xsd:fooo.001.001.01" '
                        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
                        'xsi:schemaLocation="urn:example:external external.xsd"'
                    ),
                    1,
                ),
                "Document element must not declare attributes",
            ),
            (
                fixture_xml(
                    "fooo.001.001.01",
                    "FooPayload",
                    payload_namespace="urn:iso:std:iso:20022:tech:xsd:barr.001.001.01",
                ),
                "payload namespace",
            ),
            (
                fixture_xml("fooo.001.001.01", "FooPayload").replace(
                    "<payload:FooPayload/>",
                    '<payload:FooPayload xml:lang="en"/>',
                    1,
                ),
                "payload element must not declare attributes",
            ),
        ]:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    (root / "foo_fixture.xml").write_text(xml, encoding="utf-8")

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_manifest_unknown_duplicate_escape_and_missing_schema_entries_are_rejected(self):
        cases = []
        unknown = minimal_manifest()
        unknown["schemas"][0]["unexpected"] = True
        cases.append((unknown, "unknown keys"))
        duplicate_schema = minimal_manifest()
        duplicate_schema["schemas"].append(dict(duplicate_schema["schemas"][0]))
        cases.append((duplicate_schema, "duplicate schema paths"))
        duplicate_fixture = minimal_manifest()
        duplicate_fixture["fixtures"].append(dict(duplicate_fixture["fixtures"][0]))
        cases.append((duplicate_fixture, "duplicate fixture paths"))
        escaped = minimal_manifest()
        escaped["schemas"][0]["path"] = "../../fooo.001.001.01.xsd"
        cases.append((escaped, "must not contain parent segments"))
        schema_parent_escape = minimal_manifest()
        schema_parent_escape["schemas"][0]["path"] = "../fooo.001.001.01.xsd"
        cases.append((schema_parent_escape, "must not contain parent segments"))
        schema_backslash = minimal_manifest()
        schema_backslash["schemas"][0]["path"] = r"iso\fooo.001.001.01.xsd"
        cases.append((schema_backslash, "must use forward slashes"))
        schema_whitespace = minimal_manifest()
        schema_whitespace["schemas"][0]["path"] = "iso/fooo source.001.001.01.xsd"
        cases.append((schema_whitespace, "path must not contain whitespace"))
        schema_dash = minimal_manifest()
        schema_dash["schemas"][0]["path"] = "--fooo.001.001.01.xsd"
        cases.append((schema_dash, "path must not start with a dash"))
        schema_segment_dash = minimal_manifest()
        schema_segment_dash["schemas"][0]["path"] = "iso/--fooo.001.001.01.xsd"
        cases.append(
            (
                schema_segment_dash,
                "path must not contain leading-dash path segments",
            )
        )
        schema_semicolon = minimal_manifest()
        schema_semicolon["schemas"][0]["path"] = "iso;debug/fooo.001.001.01.xsd"
        cases.append(
            (
                schema_semicolon,
                "path must not contain semicolon path parameters",
            )
        )
        schema_dot_segment = minimal_manifest()
        schema_dot_segment["schemas"][0]["path"] = "iso/./fooo.001.001.01.xsd"
        cases.append((schema_dot_segment, "must not contain empty or dot segments"))
        schema_empty_segment = minimal_manifest()
        schema_empty_segment["schemas"][0]["path"] = "iso//fooo.001.001.01.xsd"
        cases.append((schema_empty_segment, "must not contain empty or dot segments"))
        null_schema_only_reason = minimal_manifest()
        null_schema_only_reason["schemas"][0]["schema_only_reason"] = None
        cases.append((null_schema_only_reason, "schema_only_reason must be a non-empty string"))
        control_schema_only_reason = minimal_manifest()
        control_schema_only_reason["schemas"][0]["schema_only_reason"] = "schema\nonly"
        cases.append(
            (
                control_schema_only_reason,
                "schema_only_reason must not contain control characters",
            )
        )
        fixture_parent_escape = minimal_manifest()
        fixture_parent_escape["fixtures"][0]["path"] = "../../foo_fixture.xml"
        cases.append((fixture_parent_escape, "must stay under"))
        fixture_non_xml = minimal_manifest()
        fixture_non_xml["fixtures"][0]["path"] = "../foo_fixture.txt"
        cases.append((fixture_non_xml, "must point to an .xml file"))
        fixture_whitespace = minimal_manifest()
        fixture_whitespace["fixtures"][0]["path"] = "../fixtures/foo fixture.xml"
        cases.append((fixture_whitespace, "path must not contain whitespace"))
        fixture_dash = minimal_manifest()
        fixture_dash["fixtures"][0]["path"] = "--foo_fixture.xml"
        cases.append((fixture_dash, "path must not start with a dash"))
        fixture_segment_dash = minimal_manifest()
        fixture_segment_dash["fixtures"][0]["path"] = "../fixtures/--foo_fixture.xml"
        cases.append(
            (
                fixture_segment_dash,
                "path must not contain leading-dash path segments",
            )
        )
        fixture_semicolon = minimal_manifest()
        fixture_semicolon["fixtures"][0]["path"] = "../fixtures;debug/foo_fixture.xml"
        cases.append(
            (
                fixture_semicolon,
                "path must not contain semicolon path parameters",
            )
        )
        fixture_dot_segment = minimal_manifest()
        fixture_dot_segment["fixtures"][0]["path"] = ".././foo_fixture.xml"
        cases.append((fixture_dot_segment, "must not contain empty or dot segments"))
        fixture_empty_segment = minimal_manifest()
        fixture_empty_segment["fixtures"][0]["path"] = "../fixtures//foo_fixture.xml"
        cases.append((fixture_empty_segment, "must not contain empty or dot segments"))
        fixture_nonleading_parent = minimal_manifest()
        fixture_nonleading_parent["fixtures"][0]["path"] = "../fixtures/../foo_fixture.xml"
        cases.append((fixture_nonleading_parent, "parent segments must be leading"))
        invalid_schema_id = minimal_manifest()
        invalid_schema_id["schemas"][0]["path"] = "iso/FOOO.001.001.01.xsd"
        invalid_schema_id["schemas"][0]["message_def_id"] = "FOOO.001.001.01"
        cases.append((invalid_schema_id, "lowercase ISO message id"))
        invalid_fixture_id = minimal_manifest()
        invalid_fixture_id["fixtures"][0]["message_def_id"] = "fooo.1.001.01"
        cases.append((invalid_fixture_id, "lowercase ISO message id"))
        null_fixture_schema = minimal_manifest()
        null_fixture_schema["fixtures"][0]["schema"] = None
        cases.append((null_fixture_schema, "schema must be a non-empty string"))
        no_schema_reason = minimal_manifest()
        no_schema_reason["fixtures"][0].pop("schema")
        cases.append((no_schema_reason, "must set schema or missing_schema_reason"))
        null_missing_schema_reason = minimal_manifest()
        null_missing_schema_reason["fixtures"][0].pop("schema")
        null_missing_schema_reason["fixtures"][0]["missing_schema_reason"] = None
        cases.append(
            (
                null_missing_schema_reason,
                "missing_schema_reason must be a non-empty string",
            )
        )
        control_missing_schema_reason = minimal_manifest()
        control_missing_schema_reason["fixtures"][0].pop("schema")
        control_missing_schema_reason["fixtures"][0]["missing_schema_reason"] = "missing\nschema"
        cases.append(
            (
                control_missing_schema_reason,
                "missing_schema_reason must not contain control characters",
            )
        )
        both_schema_and_reason = minimal_manifest()
        both_schema_and_reason["fixtures"][0]["missing_schema_reason"] = "not allowed"
        cases.append((both_schema_and_reason, "cannot set both"))
        unknown_schema_ref = minimal_manifest()
        unknown_schema_ref["fixtures"][0]["schema"] = "iso/missing.001.001.01.xsd"
        cases.append((unknown_schema_ref, "references unknown schema"))
        fixture_schema_whitespace = minimal_manifest()
        fixture_schema_whitespace["fixtures"][0]["schema"] = (
            "iso/fooo source.001.001.01.xsd"
        )
        cases.append((fixture_schema_whitespace, "schema must not contain whitespace"))
        fixture_schema_semicolon = minimal_manifest()
        fixture_schema_semicolon["fixtures"][0]["schema"] = (
            "iso;debug/fooo.001.001.01.xsd"
        )
        cases.append(
            (
                fixture_schema_semicolon,
                "schema must not contain semicolon path parameters",
            )
        )
        for manifest, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    if message == "must stay under":
                        self.assertIn("manifest root", stderr)
                        self.assertNotIn(str(root), stderr)

    def test_fixture_message_id_cannot_be_reused_with_distinct_fixture_material(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            copied_fixture = dict(manifest["fixtures"][0])
            copied_fixture["path"] = "../foo_fixture_copy.xml"
            manifest["fixtures"].append(copied_fixture)
            manifest_path = write_minimal_tree(root, manifest)
            (root / "foo_fixture_copy.xml").write_text(
                fixture_xml("fooo.001.001.01", "FooPayload")
                + "<!-- duplicate fixture material with distinct bytes -->\n",
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("fixtures contains duplicate message_def_id values", stderr)

    def test_duplicate_manifest_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "xsd" / "fixture_manifest.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                '{"version":1,"token=xsd-duplicate-key-secret":1,"token=xsd-duplicate-key-secret":2,"schemas":[],"fixtures":[]}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("xsd-duplicate-key-secret", stderr)

    def test_non_finite_manifest_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "xsd" / "fixture_manifest.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                '{"version":NaN,"schemas":[],"fixtures":[]}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("NaN", stderr)

    def test_boolean_manifest_version_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "xsd" / "fixture_manifest.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                '{"version":true,"schemas":[],"fixtures":[]}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn(".version must be 1", stderr)

    def test_manifest_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "xsd" / "fixture_manifest.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                '{"version":"\\ud800","schemas":[],"fixtures":[]}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_top_level_input_path_diagnostics_do_not_echo_paths(self):
        manifest_payloads = (
            ("malformed-json", b"{", "is not valid JSON"),
            ("non-utf8-json", b"\xff", "is not UTF-8 JSON"),
            ("not-object", b"[]", "must be a JSON object"),
        )
        profile_payloads = (
            ("non-utf8-rust", b"\xff", "is not valid UTF-8"),
            (
                "missing-default-raw-string",
                b"// profile catalog without active default\n",
                "does not contain DEFAULT_PROFILES_JSON raw string",
            ),
            (
                "malformed-default-json",
                b'const DEFAULT_PROFILES_JSON: &str = r#"{\n"#;\n',
                "DEFAULT_PROFILES_JSON is not valid JSON",
            ),
            (
                "not-array-default-json",
                b'const DEFAULT_PROFILES_JSON: &str = r#"{}"#;\n',
                "must be a JSON array",
            ),
        )
        cases = (
            ("manifest", "manifest", manifest_payloads),
            ("profile_catalog", "profile catalog", profile_payloads),
        )
        for kind, label, payloads in cases:
            for name, payload, expected in payloads:
                with self.subTest(kind=kind, name=name):
                    with tempfile.TemporaryDirectory() as raw_root:
                        root = Path(raw_root)
                        hidden_dir = root / f"local-xsd-input-leak-{kind}-{name}"
                        hidden_dir.mkdir()
                        hidden_path = hidden_dir / (
                            "fixture_manifest.json"
                            if kind == "manifest"
                            else "profiles.rs"
                        )
                        hidden_path.write_bytes(payload)
                        if kind == "manifest":
                            argv = ["--manifest", str(hidden_path)]
                        else:
                            manifest_path = write_minimal_tree(
                                root / "valid",
                                minimal_manifest(),
                            )
                            argv = [
                                "--manifest",
                                str(manifest_path),
                                "--profile-catalog",
                                str(hidden_path),
                            ]

                        rc, stdout, stderr = run_verify(argv)

                        self.assertEqual(rc, 2)
                        self.assertEqual(stdout, "")
                        self.assertIn(expected, stderr)
                        self.assertIn(label, stderr)
                        self.assertNotIn(str(hidden_path), stderr)
                        self.assertNotIn(hidden_dir.name, stderr)

    def test_top_level_input_symlink_ancestor_diagnostics_do_not_echo_paths(self):
        cases = (
            ("manifest", "manifest"),
            ("profile_catalog", "profile catalog"),
        )
        for kind, label in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    target_dir = root / f"{kind}-target"
                    if kind == "manifest":
                        target = write_minimal_tree(target_dir, minimal_manifest())
                    else:
                        target = write_profile_catalog(target_dir / "profiles.rs")
                    hidden_link = root / f"local-xsd-input-leak-{kind}-ancestor"
                    try:
                        hidden_link.symlink_to(target_dir, target_is_directory=True)
                    except OSError as error:
                        self.skipTest(f"symlink creation unavailable: {error}")
                    hidden_path = hidden_link / target.relative_to(target_dir)
                    if kind == "manifest":
                        argv = ["--manifest", str(hidden_path)]
                    else:
                        manifest_path = write_minimal_tree(
                            root / "valid",
                            minimal_manifest(),
                        )
                        argv = [
                            "--manifest",
                            str(manifest_path),
                            "--profile-catalog",
                            str(hidden_path),
                        ]

                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be a symlink", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(hidden_path), stderr)
                    self.assertNotIn(hidden_link.name, stderr)

    def test_manifest_referenced_file_diagnostics_do_not_echo_paths(self):
        def schema_path(tree):
            return tree / "xsd" / "iso" / "fooo.001.001.01.xsd"

        def fixture_path(tree):
            return tree / "foo_fixture.xml"

        cases = (
            (
                "schema",
                "non-utf8",
                "manifest.schemas[0].path",
                lambda tree, manifest_path: schema_path(tree).write_bytes(b"\xff"),
                "is not valid UTF-8",
            ),
            (
                "schema",
                "malformed-xml",
                "manifest.schemas[0].path",
                lambda tree, manifest_path: rewrite_schema(
                    tree,
                    "fooo.001.001.01",
                    "<",
                    manifest_path=manifest_path,
                ),
                "is not well-formed XML",
            ),
            (
                "schema",
                "restricted-terms",
                "manifest.schemas[0].path",
                lambda tree, manifest_path: schema_path(tree).write_text(
                    "may only be redistributed upon agreement",
                    encoding="utf-8",
                ),
                "contains restricted redistribution terms",
            ),
            (
                "fixture",
                "malformed-xml",
                "manifest.fixtures[0].path",
                lambda tree, manifest_path: fixture_path(tree).write_text(
                    "<",
                    encoding="utf-8",
                ),
                "is not well-formed XML",
            ),
            (
                "fixture",
                "dtd",
                "manifest.fixtures[0].path",
                lambda tree, manifest_path: fixture_path(tree).write_text(
                    "<!DOCTYPE foo><Document/>",
                    encoding="utf-8",
                ),
                "must not contain DTD or entity declarations",
            ),
        )
        for kind, name, label, mutate, expected in cases:
            with self.subTest(kind=kind, name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    tree = root / f"local-xsd-ref-leak-{kind}-{name}"
                    manifest_path = write_minimal_tree(tree, minimal_manifest())
                    target_path = schema_path(tree) if kind == "schema" else fixture_path(tree)
                    mutate(tree, manifest_path)

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(target_path), stderr)
                    self.assertNotIn(tree.name, stderr)

    def test_manifest_referenced_file_symlink_ancestor_diagnostics_do_not_echo_paths(self):
        cases = (
            ("schema", "manifest.schemas[0].path"),
            ("fixture", "manifest.fixtures[0].path"),
        )
        for kind, label in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    tree = root / f"local-xsd-ref-leak-{kind}-ancestor"
                    manifest = minimal_manifest()
                    if kind == "fixture":
                        manifest["fixtures"][0]["path"] = "../fixtures/foo_fixture.xml"
                    manifest_path = write_minimal_tree(tree, manifest)
                    if kind == "schema":
                        schema = tree / "xsd" / "iso" / "fooo.001.001.01.xsd"
                        target_dir = tree / "xsd" / "schema-target"
                        target_dir.mkdir()
                        (target_dir / schema.name).write_bytes(schema.read_bytes())
                        shutil.rmtree(schema.parent)
                        try:
                            schema.parent.symlink_to(target_dir, target_is_directory=True)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                        target_path = schema
                    else:
                        target_dir = tree / "fixture-target"
                        target_dir.mkdir()
                        (target_dir / "foo_fixture.xml").write_text(
                            fixture_xml("fooo.001.001.01", "FooPayload"),
                            encoding="utf-8",
                        )
                        fixture_link = tree / "fixtures"
                        try:
                            fixture_link.symlink_to(target_dir, target_is_directory=True)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                        target_path = fixture_link / "foo_fixture.xml"

                    rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be a symlink", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(target_path), stderr)
                    self.assertNotIn(tree.name, stderr)

    def test_symlinked_manifest_schema_fixture_or_profile_catalog_is_rejected(self):
        cases = ("manifest", "schema", "fixture", "profile_catalog")
        for name in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    argv = ["--manifest", str(manifest_path)]
                    if name == "manifest":
                        copy = manifest_path.with_name("fixture_manifest.copy.json")
                        copy.write_bytes(manifest_path.read_bytes())
                        link = manifest_path.with_name("fixture_manifest.link.json")
                        try:
                            link.symlink_to(copy)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                        argv = ["--manifest", str(link)]
                    elif name == "schema":
                        schema = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
                        copy = schema.with_name("fooo.001.001.01.copy.xsd")
                        copy.write_bytes(schema.read_bytes())
                        schema.unlink()
                        try:
                            schema.symlink_to(copy)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                    elif name == "fixture":
                        fixture = root / "foo_fixture.xml"
                        copy = root / "foo_fixture.copy.xml"
                        copy.write_bytes(fixture.read_bytes())
                        fixture.unlink()
                        try:
                            fixture.symlink_to(copy)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                    else:
                        profile_catalog = write_profile_catalog(root / "profiles.rs")
                        copy = root / "profiles.copy.rs"
                        copy.write_bytes(profile_catalog.read_bytes())
                        profile_catalog.unlink()
                        try:
                            profile_catalog.symlink_to(copy)
                        except OSError as error:
                            self.skipTest(f"symlink creation unavailable: {error}")
                        argv.extend(["--profile-catalog", str(profile_catalog)])

                    rc, _stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("must not be a symlink", stderr)

    def test_directory_manifest_schema_fixture_or_profile_catalog_is_rejected(self):
        cases = ("manifest", "schema", "fixture", "profile_catalog")
        for name in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    argv = ["--manifest", str(manifest_path)]
                    if name == "manifest":
                        manifest_dir = manifest_path.with_name("fixture_manifest.dir.json")
                        manifest_dir.mkdir()
                        argv = ["--manifest", str(manifest_dir)]
                    elif name == "schema":
                        schema = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
                        schema.unlink()
                        schema.mkdir()
                    elif name == "fixture":
                        fixture = root / "foo_fixture.xml"
                        fixture.unlink()
                        fixture.mkdir()
                    else:
                        profile_catalog = root / "profiles.rs"
                        profile_catalog.mkdir()
                        argv.extend(["--profile-catalog", str(profile_catalog)])

                    rc, _stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("must be a regular file", stderr)

    def test_oversized_top_level_input_diagnostics_do_not_echo_paths(self):
        cases = (
            ("manifest", "manifest", "MAX_MANIFEST_JSON_BYTES"),
            ("profile_catalog", "profile catalog", "MAX_PROFILE_CATALOG_BYTES"),
        )
        for kind, label, limit_name in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    hidden_dir = root / f"local-xsd-input-leak-{kind}-oversized"
                    hidden_dir.mkdir()
                    hidden_path = hidden_dir / (
                        "fixture_manifest.json"
                        if kind == "manifest"
                        else "profiles.rs"
                    )
                    hidden_path.write_text(
                        '{"padding":"' + ("a" * 128) + '"}'
                        if kind == "manifest"
                        else (
                            'const DEFAULT_PROFILES_JSON: &str = r#"\n'
                            + ("a" * 128)
                            + '\n"#;\n'
                        ),
                        encoding="utf-8",
                    )
                    old_limit = getattr(VERIFIER, limit_name)
                    try:
                        setattr(VERIFIER, limit_name, 128)
                        if kind == "manifest":
                            argv = ["--manifest", str(hidden_path)]
                        else:
                            manifest_path = write_minimal_tree(
                                root / "valid",
                                minimal_manifest(),
                            )
                            argv = [
                                "--manifest",
                                str(manifest_path),
                                "--profile-catalog",
                                str(hidden_path),
                            ]
                        rc, stdout, stderr = run_verify(argv)
                    finally:
                        setattr(VERIFIER, limit_name, old_limit)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("exceeds", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(hidden_path), stderr)
                    self.assertNotIn(hidden_dir.name, stderr)

    def test_oversized_manifest_referenced_file_diagnostics_do_not_echo_paths(self):
        cases = (
            ("schema", "manifest.schemas[0].path", "MAX_SCHEMA_BYTES"),
            ("fixture", "manifest.fixtures[0].path", "MAX_FIXTURE_XML_BYTES"),
        )
        for kind, label, limit_name in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    tree = root / f"local-xsd-ref-leak-{kind}-oversized"
                    manifest_path = write_minimal_tree(tree, minimal_manifest())
                    if kind == "schema":
                        target_path = tree / "xsd" / "iso" / "fooo.001.001.01.xsd"
                        target_path.write_text("<xs:schema>" + ("a" * 128), encoding="utf-8")
                    else:
                        target_path = tree / "foo_fixture.xml"
                        target_path.write_text("<Document>" + ("a" * 128), encoding="utf-8")
                    old_limit = getattr(VERIFIER, limit_name)
                    try:
                        setattr(VERIFIER, limit_name, 128)
                        rc, stdout, stderr = run_verify(["--manifest", str(manifest_path)])
                    finally:
                        setattr(VERIFIER, limit_name, old_limit)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("exceeds", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(target_path), stderr)
                    self.assertNotIn(tree.name, stderr)

    def test_oversized_manifest_schema_fixture_or_profile_catalog_is_rejected(self):
        cases = (
            ("manifest", "MAX_MANIFEST_JSON_BYTES"),
            ("schema", "MAX_SCHEMA_BYTES"),
            ("fixture", "MAX_FIXTURE_XML_BYTES"),
            ("profile_catalog", "MAX_PROFILE_CATALOG_BYTES"),
        )
        for name, limit_name in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    profile_catalog = None
                    argv = ["--manifest", str(manifest_path)]
                    old_limit = getattr(VERIFIER, limit_name)
                    try:
                        setattr(VERIFIER, limit_name, 128)
                        if name == "manifest":
                            manifest_path.write_text(
                                '{"version":1,"padding":"' + ("a" * 128) + '"}',
                                encoding="utf-8",
                            )
                        elif name == "schema":
                            schema = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
                            schema.write_text("<xs:schema>" + ("a" * 128), encoding="utf-8")
                        elif name == "fixture":
                            fixture = root / "foo_fixture.xml"
                            fixture.write_text("<Document>" + ("a" * 128), encoding="utf-8")
                        else:
                            profile_catalog = root / "profiles.rs"
                            profile_catalog.write_text(
                                'const DEFAULT_PROFILES_JSON: &str = r#"\n'
                                + ("a" * 128)
                                + '\n"#;\n',
                                encoding="utf-8",
                            )
                            argv.extend(["--profile-catalog", str(profile_catalog)])

                        rc, _stdout, stderr = run_verify(argv)
                    finally:
                        setattr(VERIFIER, limit_name, old_limit)

                    self.assertEqual(rc, 2)
                    self.assertIn("exceeds", stderr)

    def test_copied_fixture_payloads_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = minimal_manifest()
            copied_fixture = dict(manifest["fixtures"][0])
            copied_fixture["path"] = "../foo_fixture_copy.xml"
            manifest["fixtures"].append(copied_fixture)
            manifest_path = write_minimal_tree(root, manifest)
            (root / "foo_fixture_copy.xml").write_bytes(
                (root / "foo_fixture.xml").read_bytes()
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate message_def_id values", stderr)


if __name__ == "__main__":
    unittest.main()
