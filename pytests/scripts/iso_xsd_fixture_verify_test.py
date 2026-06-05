import base64
import contextlib
import importlib.util
import io
import json
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
        "repository": "https://github.com/example/iso20022-fixtures",
        "commit": "0123456789abcdef0123456789abcdef01234567",
        "path": f"xsd/iso/{message_id}.xsd",
        "license": "Apache-2.0",
        "sha256": VERIFIER.sha256_hex(schema_text.encode("utf-8")),
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
            self.assertEqual(summary["verified_schemas"], 6)
            self.assertEqual(summary["verified_fixtures"], 11)
            self.assertEqual(summary["schema_backed_fixtures"], 6)
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

            def read_once(path):
                resolved = Path(path).resolve()
                if resolved in watched:
                    read_counts[resolved] += 1
                    if read_counts[resolved] > 1:
                        raise AssertionError(f"{watched[resolved]} file was read more than once")
                return original_read(path)

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
            self.assertIn("not schema-backed", stderr)

    def test_profile_catalog_shape_is_fail_closed(self):
        cases = []
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

    def test_checked_in_profile_catalog_records_advertised_schema_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            summary_out = Path(raw_root) / "summary.json"

            rc, stdout, stderr = run_verify(
                [
                    "--manifest",
                    str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
                    "--profile-catalog",
                    str(REPO_ROOT / "crates" / "iroha_core" / "src" / "iso_bridge" / "profiles.rs"),
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
            self.assertIn("pacs.004.001.09", missing_ids)
            self.assertIn("pacs.002.001.12", missing_ids)
            self.assertNotIn("camt.056.001.09", missing_ids)
            self.assertGreater(summary["profile_checked_versions"], 0)
            self.assertGreater(summary["profile_schema_backed_versions"], 0)
            self.assertEqual(summary["profile_schema_backed_versions"], 26)
            self.assertEqual(len(summary["missing_profile_schema_versions"]), 29)

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
            (duplicate_complex, "exactly one xs:complexType name='Document'"),
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
            (missing_payload_complex, "has no xs:complexType name='FooPayload'"),
            (
                duplicate_payload_complex,
                "must contain exactly one xs:complexType name='FooPayload'",
            ),
            (
                payload_complex_without_sequence,
                "payload complex type 'FooPayload' must contain only one direct xs:sequence",
            ),
            (
                payload_complex_duplicate_sequence,
                "payload complex type 'FooPayload' must contain only one direct xs:sequence",
            ),
            (
                payload_complex_extra_choice,
                "payload complex type 'FooPayload' must contain only one direct xs:sequence",
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
                "payload complex type 'FooPayload' contains unsupported child",
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

    def test_restricted_schema_redistribution_terms_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = write_minimal_tree(root, minimal_manifest())
            schema_path = root / "xsd" / "iso" / "fooo.001.001.01.xsd"
            restricted_terms = """<!--Copyright (c) SWIFT scrl, 2020.

 This is a licensed product, which may only be redistributed upon agreement with SWIFT scrl.
 The user has no right, or right to authorise others, to:
   - rent, lease, or sell this component;
   - display publicly, distribute or otherwise provide this component;
-->
"""
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
        cases.append((missing_source, "source must be a JSON object"))

        unknown_source_key = minimal_manifest()
        unknown_source_key["schemas"][0]["source"]["unexpected"] = "value"
        cases.append((unknown_source_key, "unknown keys"))

        bad_repository = minimal_manifest()
        bad_repository["schemas"][0]["source"]["repository"] = (
            "https://github.com/example/iso20022-fixtures.git"
        )
        cases.append((bad_repository, "repository must be a canonical"))

        bad_commit = minimal_manifest()
        bad_commit["schemas"][0]["source"]["commit"] = (
            "0123456789abcdef0123456789abcdef0123456Z"
        )
        cases.append((bad_commit, "commit must be a lowercase 40-hex"))

        escaped_path = minimal_manifest()
        escaped_path["schemas"][0]["source"]["path"] = "../fooo.001.001.01.xsd"
        cases.append((escaped_path, "must not contain empty, dot, or parent segments"))

        mismatched_filename = minimal_manifest()
        mismatched_filename["schemas"][0]["source"]["path"] = "xsd/other.001.001.01.xsd"
        cases.append((mismatched_filename, "filename must match message_def_id"))

        unsupported_license = minimal_manifest()
        unsupported_license["schemas"][0]["source"]["license"] = "NOASSERTION"
        cases.append((unsupported_license, "license must be one of"))

        sha_drift = minimal_manifest()
        sha_drift["schemas"][0]["source"]["sha256"] = "0" * 64
        cases.append((sha_drift, "sha256 does not match checked-in XSD bytes"))

        for manifest, message in cases:
            with self.subTest(message=message):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    manifest_path = write_minimal_tree(root, manifest)

                    rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

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
        schema_dot_segment = minimal_manifest()
        schema_dot_segment["schemas"][0]["path"] = "iso/./fooo.001.001.01.xsd"
        cases.append((schema_dot_segment, "must not contain empty or dot segments"))
        schema_empty_segment = minimal_manifest()
        schema_empty_segment["schemas"][0]["path"] = "iso//fooo.001.001.01.xsd"
        cases.append((schema_empty_segment, "must not contain empty or dot segments"))
        fixture_parent_escape = minimal_manifest()
        fixture_parent_escape["fixtures"][0]["path"] = "../../foo_fixture.xml"
        cases.append((fixture_parent_escape, "must stay under"))
        fixture_non_xml = minimal_manifest()
        fixture_non_xml["fixtures"][0]["path"] = "../foo_fixture.txt"
        cases.append((fixture_non_xml, "must point to an .xml file"))
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

    def test_duplicate_manifest_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = root / "xsd" / "fixture_manifest.json"
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(
                '{"version":1,"version":1,"schemas":[],"fixtures":[]}\n',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_verify(["--manifest", str(manifest_path)])

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

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
            self.assertIn("duplicate fixture SHA-256", stderr)


if __name__ == "__main__":
    unittest.main()
