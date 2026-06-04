import contextlib
import importlib.util
import io
import json
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


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
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
    }


def run_verify(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = VERIFIER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


class IsoXsdFixtureVerifyTest(unittest.TestCase):
    def test_checked_in_manifest_passes_and_records_reviewed_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            summary_out = Path(raw_root) / "summary.json"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["verified_schemas"], 5)
            self.assertEqual(summary["verified_fixtures"], 10)
            self.assertEqual(summary["schema_backed_fixtures"], 5)
            self.assertEqual(len(summary["missing_schema_fixtures"]), 5)
            missing_schema_ids = sorted(
                entry["message_def_id"] for entry in summary["missing_schema_fixtures"]
            )
            self.assertIn("colr.012.001.05", missing_schema_ids)
            self.assertNotIn("colr.007.001.08", missing_schema_ids)
            self.assertEqual(summary["schema_only_entries"], [])
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, VERIFIER.sha256_hex(VERIFIER._canonical_json_bytes(body)))

    def test_strict_flags_reject_current_reviewed_gaps(self):
        manifest = str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json")

        rc, _stdout, stderr = run_verify(
            ["--manifest", manifest, "--require-schema-backed-fixtures"]
        )
        self.assertEqual(rc, 2)
        self.assertIn("not schema-backed", stderr)

        rc, _stdout, stderr = run_verify(
            ["--manifest", manifest, "--require-fixture-for-schema"]
        )
        self.assertEqual(rc, 0, stderr)
        summary = json.loads(_stdout)
        self.assertEqual(summary["schema_only_entries"], [])

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

    def test_schema_target_namespace_payload_and_element_form_drift_are_rejected(self):
        for mutate, message in [
            (
                lambda root: (root / "xsd" / "iso" / "fooo.001.001.01.xsd").write_text(
                    xsd_text("fooo.001.001.01", "FooPayload", target_message_id="barr.001.001.01"),
                    encoding="utf-8",
                ),
                "targetNamespace",
            ),
            (
                lambda root: (root / "xsd" / "iso" / "fooo.001.001.01.xsd").write_text(
                    xsd_text("fooo.001.001.01", "DifferentPayload"),
                    encoding="utf-8",
                ),
                "payload root",
            ),
            (
                lambda root: (root / "xsd" / "iso" / "fooo.001.001.01.xsd").write_text(
                    xsd_text("fooo.001.001.01", "FooPayload", element_form="unqualified"),
                    encoding="utf-8",
                ),
                "elementFormDefault",
            ),
        ]:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, minimal_manifest())
                    mutate(root)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_fixture_namespace_payload_root_and_document_root_drift_are_rejected(self):
        for xml, message in [
            (fixture_xml("barr.001.001.01", "FooPayload"), "namespace message id"),
            (fixture_xml("fooo.001.001.01", "DifferentPayload"), "payload root"),
            (fixture_xml("fooo.001.001.01", "FooPayload", root="Envelope"), "root element"),
            (
                fixture_xml(
                    "fooo.001.001.01",
                    "FooPayload",
                    payload_namespace="urn:iso:std:iso:20022:tech:xsd:barr.001.001.01",
                ),
                "payload namespace",
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
        cases.append((escaped, "must stay under"))
        schema_parent_escape = minimal_manifest()
        schema_parent_escape["schemas"][0]["path"] = "../fooo.001.001.01.xsd"
        cases.append((schema_parent_escape, "must stay under"))
        invalid_schema_id = minimal_manifest()
        invalid_schema_id["schemas"][0]["path"] = "iso/FOOO.001.001.01.xsd"
        invalid_schema_id["schemas"][0]["message_def_id"] = "FOOO.001.001.01"
        cases.append((invalid_schema_id, "lowercase ISO message id"))
        invalid_fixture_id = minimal_manifest()
        invalid_fixture_id["fixtures"][0]["message_def_id"] = "fooo.1.001.01"
        cases.append((invalid_fixture_id, "lowercase ISO message id"))
        no_schema_reason = minimal_manifest()
        no_schema_reason["fixtures"][0].pop("schema")
        cases.append((no_schema_reason, "must set schema or missing_schema_reason"))
        both_schema_and_reason = minimal_manifest()
        both_schema_and_reason["fixtures"][0]["missing_schema_reason"] = "not allowed"
        cases.append((both_schema_and_reason, "cannot set both"))
        unknown_schema_ref = minimal_manifest()
        unknown_schema_ref["fixtures"][0]["schema"] = "iso/missing.001.001.01.xsd"
        cases.append((unknown_schema_ref, "references unknown schema"))
        for manifest, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)


if __name__ == "__main__":
    unittest.main()
