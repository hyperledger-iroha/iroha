import argparse
import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_operator_evidence_verify_test as evidence_test
from pytests.scripts import iso_operator_canary_test as canary_test
from pytests.scripts import iso_xsd_fixture_verify_test as xsd_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("iso_production_readiness", SCRIPT_PATH)
READINESS = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = READINESS
SPEC.loader.exec_module(READINESS)


def write_json(path, body):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def refresh_digest(summary):
    summary.pop(READINESS.SUMMARY_DIGEST_FIELD, None)
    summary[READINESS.SUMMARY_DIGEST_FIELD] = READINESS.sha256_hex(
        READINESS._canonical_json_bytes(summary)
    )
    return summary


def receipt_verification_summary(
    receipt_kind=None,
    *,
    verified_receipts=None,
    allow_failed=False,
    allow_insecure_http=False,
    allow_legacy_colr007=False,
    allow_default_profile=False,
    require_source_files=True,
    endpoint_requires_insecure_http=False,
    receipts=None,
):
    kinds = receipt_kind or ["iso-audit-notary", "iso-rail-gateway"]
    if verified_receipts is None:
        verified_receipts = len(kinds)
    if receipts is None:
        receipts = [
            {
                "path": f"/ops/iso/receipts/{kind}.{offset}.receipt.json",
                "receipt_kind": kind,
                "receipt_sha256": f"{offset + 1:064x}",
                "ok": True,
                "status_code": 202,
                "response_body_sha256": f"{offset + 401:064x}",
                "endpoint_requires_insecure_http": endpoint_requires_insecure_http,
                **(
                    {
                        "anchor_path": "/ops/iso/notary/latest.notary.json",
                        "store_dir": "/ops/iso/notary-store",
                        "index_path": "/ops/iso/notary/messages.index.json",
                        "anchor_sha256": f"{offset + 101:064x}",
                        "index_sha256": f"{offset + 201:064x}",
                        "record_count": 1,
                    }
                    if kind == "iso-audit-notary"
                    else {
                        "message_type": "pacs.002",
                        "payload_sha256": f"{offset + 301:064x}",
                        "profile": "swift-cbpr-plus",
                        "rail_message_id": f"rail-drop-{offset}",
                        "source_path": f"/ops/iso/rail-inbox/rail-drop-{offset}.xml",
                    }
                ),
            }
            for offset, kind in enumerate(kinds[: max(verified_receipts, 0)])
        ]
    return refresh_digest(
        {
            "version": READINESS.RECEIPT_SUMMARY_VERSION,
            "verified_receipts": verified_receipts,
            "receipt_kind": kinds,
            "allow_failed": allow_failed,
            "allow_insecure_http": allow_insecure_http,
            "allow_legacy_colr007": allow_legacy_colr007,
            "allow_default_profile": allow_default_profile,
            "require_source_files": require_source_files,
            "receipts": receipts,
        }
    )


FRESHNESS_FLAGS = {
    "--max-xsd-age-days": "36500",
    "--max-evidence-age-days": "36500",
    "--max-canary-age-days": "36500",
    "--max-trust-age-days": "36500",
    "--max-trust-source-age-days": "36500",
}


def _has_flag(argv, flag):
    return any(item == flag or item.startswith(flag + "=") for item in argv)


def run_readiness(argv, *, include_context=True, include_freshness=True):
    argv = list(argv)
    if include_context and "--provider" not in argv:
        argv.extend(["--provider", "local-bank"])
    if include_context and "--environment" not in argv:
        argv.extend(["--environment", "preprod"])
    if include_freshness:
        for flag, value in FRESHNESS_FLAGS.items():
            if not _has_flag(argv, flag):
                argv.extend([flag, value])
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = READINESS.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def write_strict_xsd_summary(root):
    root.mkdir(parents=True, exist_ok=True)
    manifest_path = xsd_test.write_minimal_tree(root, xsd_test.minimal_manifest())
    profile_catalog = xsd_test.write_profile_catalog(root / "profiles.rs")
    summary_path = root / "xsd.summary.json"
    rc, _stdout, stderr = xsd_test.run_verify(
        [
            "--manifest",
            str(manifest_path),
            "--require-schema-backed-fixtures",
            "--require-fixture-for-schema",
            "--profile-catalog",
            str(profile_catalog),
            "--require-profile-schema-backed-versions",
            "--validate-xml-schema",
            "--summary-out",
            str(summary_path),
        ]
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_checked_in_xsd_summary(
    root,
    *,
    require_fixture_for_schema=False,
    require_profile_schema_backed_versions=False,
    profile_catalog=True,
    validate_xml_schema=True,
):
    root.mkdir(parents=True, exist_ok=True)
    summary_path = root / "checked-in-xsd.summary.json"
    argv = [
        "--manifest",
        str(REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"),
        "--summary-out",
        str(summary_path),
    ]
    if require_fixture_for_schema:
        argv.append("--require-fixture-for-schema")
    if profile_catalog:
        argv.extend(
            [
                "--profile-catalog",
                str(
                    REPO_ROOT
                    / "crates"
                    / "iroha_core"
                    / "src"
                    / "iso_bridge"
                    / "profiles.rs"
                ),
            ]
        )
    if require_profile_schema_backed_versions:
        argv.append("--require-profile-schema-backed-versions")
    if validate_xml_schema:
        argv.append("--validate-xml-schema")
    rc, _stdout, stderr = xsd_test.run_verify(argv)
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_evidence_summary(root, *, direct_receipts=True):
    root.mkdir(parents=True, exist_ok=True)
    receipt_args = []
    receipt_entries = None
    if direct_receipts:
        notary_receipts, rail_receipts = evidence_test.write_https_receipt_dirs(root)
        receipt_args = [
            "--receipt-dir",
            str(notary_receipts),
            "--receipt-dir",
            str(rail_receipts),
        ]
        receipt_entries = evidence_test.receipt_entries_from_dirs(
            notary_receipts,
            rail_receipts,
        )
    canary_path = evidence_test.write_canary(
        root,
        evidence_test.valid_canary_summary(receipt_entries=receipt_entries),
    )
    trust_path = evidence_test.write_trust_summary(root / "trust")
    argv = [
        "--canary-summary",
        str(canary_path),
        "--trust-summary",
        str(trust_path),
        "--provider",
        "local-bank",
        "--environment",
        "preprod",
    ]
    if direct_receipts:
        argv.extend(receipt_args)
    else:
        argv.append("--allow-canary-stage-receipts-only")
    summary_path = root / "evidence.summary.json"
    argv.extend(["--summary-out", str(summary_path)])
    rc, _stdout, stderr = evidence_test.run_evidence(
        argv
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def write_plan_only_evidence_summary(root):
    root.mkdir(parents=True, exist_ok=True)
    canary_path = evidence_test.write_canary(
        root,
        evidence_test.plan_only_canary_summary(),
    )
    trust_path = evidence_test.write_trust_summary(root / "trust")
    summary_path = root / "plan-only-evidence.summary.json"
    rc, _stdout, stderr = evidence_test.run_evidence(
        [
            "--canary-summary",
            str(canary_path),
            "--trust-summary",
            str(trust_path),
            "--provider",
            "local-bank",
            "--environment",
            "preprod",
            "--allow-plan-only",
            "--allow-canary-stage-receipts-only",
            "--summary-out",
            str(summary_path),
        ]
    )
    if rc != 0:
        raise AssertionError(stderr)
    return summary_path


def add_archive_receipt_verification(path, receipt_kind=None, *, verified_receipts=None):
    evidence = json.loads(path.read_text(encoding="utf-8"))
    receipts = None
    if receipt_kind is None and verified_receipts is None:
        receipts = [
            dict(receipt)
            for receipt in evidence["canary_summaries"][0]["receipt_summary"]["receipts"]
        ]
        receipt_kind = sorted({receipt["receipt_kind"] for receipt in receipts})
    evidence["receipt_verification"] = receipt_verification_summary(
        receipt_kind,
        verified_receipts=verified_receipts,
        receipts=receipts,
    )
    refresh_digest(evidence)
    return write_json(path, evidence)


class IsoProductionReadinessTest(unittest.TestCase):
    def test_text_output_symlink_ancestor_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = root / "target"
            target.mkdir()
            hidden = "hidden-readiness-output-link"
            link = root / hidden
            try:
                link.symlink_to(target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            with self.assertRaises(READINESS.ReadinessError) as caught:
                READINESS._write_text_output(
                    link / "summary.json",
                    "{}\n",
                    display_label="summary_out",
                )

            message = str(caught.exception)
            self.assertIn("summary_out", message)
            self.assertIn("must not be a symlink", message)
            self.assertNotIn(str(link), message)
            self.assertNotIn(hidden, message)

    def test_receipt_metadata_helper_rejects_unsupported_kind_without_echo(self):
        secret_kind = "token=readiness-receipt-kind-secret"
        receipt = {
            "receipt_kind": secret_kind,
            "ok": True,
            "status_code": 202,
            "response_body_sha256": "1" * 64,
            "endpoint_requires_insecure_http": False,
        }

        with self.assertRaisesRegex(READINESS.ReadinessError, "unsupported receipt_kind") as ctx:
            READINESS._receipt_entry_content_metadata(receipt)

        self.assertNotIn(secret_kind, str(ctx.exception))

    def test_replay_placeholder_predicates_match_direct_verifiers(self):
        endpoint_modules = (
            evidence_test.rail_test.ADAPTER,
            evidence_test.audit_test.ADAPTER,
            canary_test.CANARY,
            evidence_test.receipt_test.VERIFIER,
        )
        reserved_endpoint_hosts = endpoint_modules[0].RESERVED_PLACEHOLDER_HOST_SUFFIXES
        template_endpoint_hosts = endpoint_modules[0].TEMPLATE_CANARY_ENDPOINT_HOSTS
        for module in endpoint_modules[1:]:
            self.assertEqual(module.RESERVED_PLACEHOLDER_HOST_SUFFIXES, reserved_endpoint_hosts)
            self.assertEqual(module.TEMPLATE_CANARY_ENDPOINT_HOSTS, template_endpoint_hosts)
        self.assertEqual(
            evidence_test.EVIDENCE.TEMPLATE_CANARY_ENDPOINT_HOSTS,
            template_endpoint_hosts,
        )
        self.assertEqual(
            evidence_test.EVIDENCE.PLACEHOLDER_TRUST_SOURCE_HOSTS,
            reserved_endpoint_hosts | template_endpoint_hosts,
        )
        production_host = "pki.swift-bank.examplebank"
        for module in endpoint_modules:
            self.assertFalse(module._host_uses_reserved_placeholder_suffix(production_host))
            self.assertFalse(module._host_uses_template_canary_suffix(production_host))
            for host in sorted(reserved_endpoint_hosts):
                with self.subTest(module=module.__name__, reserved_host=host):
                    self.assertTrue(module._host_uses_reserved_placeholder_suffix(host))
                    self.assertTrue(
                        module._host_uses_reserved_placeholder_suffix(f"service.{host}")
                    )
                    self.assertFalse(module._host_uses_template_canary_suffix(host))
            for host in sorted(template_endpoint_hosts):
                with self.subTest(module=module.__name__, template_host=host):
                    self.assertFalse(module._host_uses_reserved_placeholder_suffix(host))
                    self.assertTrue(module._host_uses_template_canary_suffix(host))
                    self.assertTrue(
                        module._host_uses_template_canary_suffix(f"service.{host}")
                    )
        self.assertFalse(
            evidence_test.EVIDENCE._host_uses_reserved_placeholder_suffix(production_host)
        )
        for host in sorted(reserved_endpoint_hosts | template_endpoint_hosts):
            with self.subTest(evidence_replay_host=host):
                self.assertTrue(
                    evidence_test.EVIDENCE._host_uses_reserved_placeholder_suffix(host)
                )
                self.assertTrue(
                    evidence_test.EVIDENCE._host_uses_reserved_placeholder_suffix(
                        f"service.{host}"
                    )
                )

        self.assertEqual(
            READINESS.PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS,
            xsd_test.VERIFIER.PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS,
        )
        self.assertEqual(
            READINESS.PLACEHOLDER_TRUST_SOURCE_MARKERS,
            evidence_test.EVIDENCE.PLACEHOLDER_TRUST_SOURCE_MARKERS,
        )
        self.assertEqual(
            READINESS.PLACEHOLDER_TRUST_SOURCE_MARKERS,
            evidence_test.trust_test.VERIFIER.PLACEHOLDER_TRUST_SOURCE_MARKERS,
        )
        self.assertEqual(
            READINESS.PLACEHOLDER_TRUST_SOURCE_HOSTS,
            evidence_test.EVIDENCE.PLACEHOLDER_TRUST_SOURCE_HOSTS,
        )
        self.assertEqual(
            READINESS.PLACEHOLDER_TRUST_SOURCE_HOSTS,
            evidence_test.trust_test.VERIFIER.PLACEHOLDER_TRUST_SOURCE_HOSTS,
        )
        for value in (
            "Ｓａｍｐｌｅ Swift operator PKI",
            "ｔｅｍｐｌａｔｅ-v1",
            "ｒｅｐｌａｃｅ-before-production",
        ):
            with self.subTest(trust_source=value):
                self.assertTrue(READINESS._trust_source_text_is_placeholder(value))
                self.assertTrue(
                    evidence_test.EVIDENCE._trust_source_text_is_placeholder(value)
                )
                self.assertTrue(
                    evidence_test.trust_test.VERIFIER._trust_source_text_is_placeholder(
                        value
                    )
                )

        valid_repository = "https://github.com/moov-io/fedwire20022"
        self.assertFalse(READINESS._xsd_source_repository_is_invalid(valid_repository))
        self.assertEqual(
            xsd_test.VERIFIER._validate_source_repository(
                valid_repository,
                "source.repository",
            ),
            valid_repository,
        )
        for component in sorted(READINESS.PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS):
            for repository in (
                f"https://github.com/{component}/fedwire20022",
                f"https://github.com/moov-io/{component}",
            ):
                with self.subTest(repository=repository):
                    self.assertTrue(
                        READINESS._xsd_source_repository_is_invalid(repository)
                    )
                    with self.assertRaises(xsd_test.VERIFIER.FixtureManifestError):
                        xsd_test.VERIFIER._validate_source_repository(
                            repository,
                            "source.repository",
                        )

        for repository in (
            "https://github.com/moov-io/iso20022-replace_before_production-fixtures",
            "https://github.com/moov-io/operatorcanarybank",
        ):
            with self.subTest(repository=repository):
                self.assertTrue(READINESS._xsd_source_repository_is_invalid(repository))
                with self.assertRaises(xsd_test.VERIFIER.FixtureManifestError):
                    xsd_test.VERIFIER._validate_source_repository(
                        repository,
                        "source.repository",
                    )

        retrieved_at = READINESS.dt.datetime.now(READINESS.dt.UTC).isoformat()

        def source(**overrides):
            base = {
                "authority": "Swift operator PKI",
                "version": "2026-q2",
                "url": "https://pki.swift-bank.examplebank/source",
                "retrieved_at": retrieved_at,
            }
            base.update(overrides)
            return base

        production_source = source()
        self.assertFalse(
            evidence_test.trust_test.VERIFIER._summary_source_has_placeholder(
                production_source,
            )
        )
        self.assertTrue(
            evidence_test.EVIDENCE._computed_profile_json_emittable(
                allow_synthetic_der=False,
                allow_record_only=False,
                allow_insecure_source_url=False,
                max_source_age_days=365,
                bundle_summaries=[{"source": production_source}],
            )
        )
        self.assertTrue(
            READINESS._computed_profile_json_emittable(
                allow_synthetic_der=False,
                allow_record_only=False,
                allow_insecure_source_url=False,
                max_source_age_days=365,
                profiles=[{"source": production_source}],
            )
        )
        for flag in (
            "allow_synthetic_der",
            "allow_record_only",
            "allow_insecure_source_url",
        ):
            with self.subTest(flag=flag):
                evidence_kwargs = {
                    "allow_synthetic_der": False,
                    "allow_record_only": False,
                    "allow_insecure_source_url": False,
                    "max_source_age_days": 365,
                    "bundle_summaries": [{"source": production_source}],
                }
                evidence_kwargs[flag] = True
                readiness_kwargs = {
                    "allow_synthetic_der": False,
                    "allow_record_only": False,
                    "allow_insecure_source_url": False,
                    "max_source_age_days": 365,
                    "profiles": [{"source": production_source}],
                }
                readiness_kwargs[flag] = True
                self.assertFalse(
                    evidence_test.EVIDENCE._computed_profile_json_emittable(
                        **evidence_kwargs
                    )
                )
                self.assertFalse(
                    READINESS._computed_profile_json_emittable(**readiness_kwargs)
                )

        for marker in READINESS.PLACEHOLDER_TRUST_SOURCE_MARKERS:
            for field in ("authority", "version"):
                trust_source = source(**{field: f"rail {marker} source"})
                with self.subTest(marker=marker, field=field):
                    self.assertTrue(
                        evidence_test.trust_test.VERIFIER._summary_source_has_placeholder(
                            trust_source,
                        )
                    )
                    self.assertFalse(
                        evidence_test.EVIDENCE._computed_profile_json_emittable(
                            allow_synthetic_der=False,
                            allow_record_only=False,
                            allow_insecure_source_url=False,
                            max_source_age_days=365,
                            bundle_summaries=[{"source": trust_source}],
                        )
                    )
                    self.assertFalse(
                        READINESS._computed_profile_json_emittable(
                            allow_synthetic_der=False,
                            allow_record_only=False,
                            allow_insecure_source_url=False,
                            max_source_age_days=365,
                            profiles=[{"source": trust_source}],
                        )
                    )

        for host in sorted(READINESS.PLACEHOLDER_TRUST_SOURCE_HOSTS):
            trust_source = source(url=f"https://pki.{host}/source")
            with self.subTest(host=host):
                self.assertTrue(
                    evidence_test.trust_test.VERIFIER._summary_source_has_placeholder(
                        trust_source,
                    )
                )
                self.assertFalse(
                    evidence_test.EVIDENCE._computed_profile_json_emittable(
                        allow_synthetic_der=False,
                        allow_record_only=False,
                        allow_insecure_source_url=False,
                        max_source_age_days=365,
                        bundle_summaries=[{"source": trust_source}],
                    )
                )
                self.assertFalse(
                    READINESS._computed_profile_json_emittable(
                        allow_synthetic_der=False,
                        allow_record_only=False,
                        allow_insecure_source_url=False,
                        max_source_age_days=365,
                        profiles=[{"source": trust_source}],
                    )
                )

    def test_replay_receipt_policy_predicates_match_direct_verifiers(self):
        receipt_verifier = evidence_test.receipt_test.VERIFIER
        evidence_verifier = evidence_test.EVIDENCE
        rail_adapter = evidence_test.rail_test.ADAPTER

        self.assertEqual(
            READINESS.REQUIRED_RECEIPT_KINDS,
            evidence_verifier.REQUIRED_RECEIPT_KINDS,
        )
        self.assertEqual(
            READINESS.REQUIRED_RECEIPT_KINDS,
            receipt_verifier.SUPPORTED_KINDS,
        )
        self.assertEqual(
            READINESS.LEGACY_RAIL_MESSAGE_TYPES,
            evidence_verifier.LEGACY_RAIL_MESSAGE_TYPES,
        )
        self.assertEqual(
            READINESS.LEGACY_RAIL_MESSAGE_TYPES,
            receipt_verifier.LEGACY_RAIL_MESSAGE_TYPES,
        )
        self.assertEqual(
            READINESS.LEGACY_RAIL_MESSAGE_TYPES,
            rail_adapter.LEGACY_MESSAGE_TYPES,
        )
        self.assertEqual(
            READINESS.SUPPORTED_RAIL_MESSAGE_TYPES,
            evidence_verifier.SUPPORTED_RAIL_MESSAGE_TYPES,
        )
        self.assertEqual(
            READINESS.SUPPORTED_RAIL_MESSAGE_TYPES,
            receipt_verifier.SUPPORTED_RAIL_MESSAGE_TYPES,
        )
        self.assertEqual(
            READINESS.SUPPORTED_RAIL_MESSAGE_TYPES,
            set(rail_adapter.ENDPOINTS),
        )
        self.assertEqual(
            READINESS.RECEIPT_PATH_SUFFIX,
            evidence_verifier.RECEIPT_PATH_SUFFIX,
        )
        self.assertEqual(READINESS.RECEIPT_PATH_SUFFIX, ".receipt.json")
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            evidence_verifier.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            receipt_verifier.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            rail_adapter.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.MESSAGE_TYPE_RE.pattern,
            evidence_verifier.MESSAGE_TYPE_RE.pattern,
        )
        self.assertEqual(
            READINESS.MESSAGE_TYPE_RE.pattern,
            receipt_verifier.MESSAGE_TYPE_RE.pattern,
        )
        self.assertIn("colr.012", rail_adapter.ENDPOINTS)
        self.assertIn("colr.007", rail_adapter.ENDPOINTS)
        self.assertIn("colr.012", READINESS.SUPPORTED_RAIL_MESSAGE_TYPES)
        self.assertIn("colr.007", READINESS.SUPPORTED_RAIL_MESSAGE_TYPES)
        self.assertTrue(READINESS.MESSAGE_TYPE_RE.fullmatch("colr.012"))
        self.assertTrue(evidence_verifier.MESSAGE_TYPE_RE.fullmatch("colr.012"))
        self.assertEqual(
            "colr.007" in READINESS.LEGACY_RAIL_MESSAGE_TYPES,
            "colr.007" in rail_adapter.LEGACY_MESSAGE_TYPES,
        )
        self.assertNotIn("colr.012", READINESS.LEGACY_RAIL_MESSAGE_TYPES)
        self.assertNotIn("colr.012", rail_adapter.LEGACY_MESSAGE_TYPES)

    def test_replay_profile_taxonomy_predicates_match_direct_verifiers(self):
        trust_verifier = evidence_test.trust_test.VERIFIER
        evidence_verifier = evidence_test.EVIDENCE
        xsd_verifier = xsd_test.VERIFIER

        self.assertEqual(READINESS.KNOWN_RAILS, evidence_verifier.KNOWN_RAILS)
        self.assertEqual(READINESS.KNOWN_RAILS, trust_verifier.KNOWN_RAILS)
        self.assertEqual(READINESS.KNOWN_RAILS, xsd_verifier.PROFILE_RAILS)
        self.assertEqual(
            READINESS.PROFILE_DIRECTIONS,
            xsd_verifier.PROFILE_DIRECTIONS,
        )
        self.assertEqual(
            trust_verifier.POLICIES,
            xsd_verifier.PROFILE_SIGNATURE_POLICIES,
        )
        self.assertEqual(
            READINESS.TRUST_SIGNATURE_POLICIES,
            trust_verifier.POLICIES,
        )
        self.assertEqual(
            READINESS.TRUST_SIGNATURE_POLICIES,
            evidence_verifier.TRUST_SIGNATURE_POLICIES,
        )
        self.assertEqual(
            READINESS.REQUIRE_VERIFIED,
            evidence_verifier.REQUIRE_VERIFIED,
        )
        self.assertEqual(
            READINESS.REQUIRE_VERIFIED,
            trust_verifier.REQUIRE_VERIFIED,
        )
        self.assertIn(READINESS.REQUIRE_VERIFIED, trust_verifier.POLICIES)
        self.assertEqual(
            READINESS.ALLOWED_SCHEMA_SOURCE_LICENSES,
            xsd_verifier.ALLOWED_SCHEMA_SOURCE_LICENSES,
        )
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            evidence_verifier.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            trust_verifier.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.PROFILE_ID_RE.pattern,
            xsd_verifier.PROFILE_ID_RE.pattern,
        )
        self.assertEqual(
            READINESS.MESSAGE_TYPE_RE.pattern,
            evidence_verifier.MESSAGE_TYPE_RE.pattern,
        )
        self.assertEqual(
            READINESS.MESSAGE_TYPE_RE.pattern,
            xsd_verifier.MESSAGE_TYPE_RE.pattern,
        )

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_readiness_unknown_secret", "readiness_unknown_secret"),
            ("%70assword_readiness_unknown_leak", "readiness_unknown_leak"),
            ("private-key_readiness_unknown_leak", "readiness_unknown_leak"),
            ("private--key_readiness_unknown_leak", "readiness_unknown_leak"),
            ("private%09key_readiness_unknown_leak", "readiness_unknown_leak"),
            ("x--iroha--signature_readiness_unknown_leak", "readiness_unknown_leak"),
            ("unexpected\x1breadiness_key", "\x1b"),
            ("unexpected_readiness_\uff4bey", "\uff4b"),
            ("operator_note", "operator_note"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "summary"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._reject_unknown_keys(many_unknown, set(), "summary")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_separator_smuggled_secret_identifiers_are_detected(self):
        cases = (
            "private\tkey readiness identifier",
            "private--key readiness identifier",
            "private/key readiness identifier",
            "private\\key readiness identifier",
            "private%2fkey readiness identifier",
            "private\u200dkey readiness identifier",
            "private\u0301key readiness identifier",
            "ｐｒｉｖａｔｅｋｅｙ readiness identifier",
            "x--iroha--signature readiness identifier",
            "x/iroha/signature readiness identifier",
            "x%2firoha%2fsignature readiness identifier",
            "x\u200diroha\u200dsignature readiness identifier",
            "x\u0301iroha\u0301signature readiness identifier",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ readiness identifier",
            "token%09secret readiness identifier",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(READINESS._contains_secret_identifier_material(value))
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
                self.assertTrue(READINESS._is_secret_looking_key(key))

    def test_path_separator_secret_key_values_are_detected(self):
        cases = (
            "private/key=readiness-value-secret",
            "api/key:readiness-value-secret",
            "client/secret=readiness-value-secret",
            "set/cookie:readiness-value-secret",
            "x/iroha/signature: readiness-value-secret",
            "private%2fkey=readiness-value-secret",
            "private\u200dkey=readiness-value-secret",
            "private\u0301key=readiness-value-secret",
            "ｐｒｉｖａｔｅｋｅｙ=readiness-compat-secret",
            "ａｐｉ／ｋｅｙ:readiness-compat-secret",
            "x\u200diroha\u200dsignature: readiness-value-secret",
            "x\u0301iroha\u0301signature: readiness-value-secret",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ: readiness-compat-secret",
            "private%E2%80%8Dkey=readiness-value-secret",
            "private%CC%81key=readiness-value-secret",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(READINESS._contains_secret_material(value))

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=readiness-terminator-secret"
        cases = (
            (
                "raw",
                lambda: READINESS._preflight_raw_cli_secrets(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "context",
                lambda: READINESS._preflight_required_cli_values(
                    ["--", "--provider", hidden],
                    {"--provider"},
                    "context",
                ),
            ),
            (
                "boolean",
                lambda: READINESS._preflight_boolean_cli_flags(
                    ["--", "--allow-reviewed-xsd-gaps", hidden],
                    {"--allow-reviewed-xsd-gaps"},
                ),
            ),
            (
                "path",
                lambda: READINESS._preflight_output_cli_paths(
                    ["--", "--summary-out", hidden],
                    {"--summary-out"},
                ),
            ),
            (
                "numeric",
                lambda: READINESS._preflight_numeric_cli_values(
                    ["--", "--max-evidence-age-days", hidden],
                    integer_flags={"--max-evidence-age-days"},
                    number_flags=set(),
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("readiness-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                READINESS.build_parser().parse_args(["--summary-ou", "out"])

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--summary-ou", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        cases = (
            ("--unknown-readiness\x1bflag", "\x1b"),
            ("--unknown-readiness\u202eflag", "\u202e"),
        )
        for hidden, marker in cases:
            with self.subTest(hidden=hidden):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._preflight_raw_cli_secrets([hidden], {"--summary-out"})

                message = str(caught.exception)
                self.assertIn("CLI argument must not contain control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(marker, message)
                self.assertNotIn("unknown-readiness", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dsummary-out"
        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("summary-out", message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--summary-out"],
            ["--summary-out", ""],
            ["--summary-out", "--provider"],
            ["--summary-out="],
            ["--summary-out=--provider"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    READINESS.ReadinessError,
                    "--summary-out requires a path value",
                ):
                    READINESS._preflight_output_cli_paths(argv, {"--summary-out"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=readiness-path-leak.summary.json", "token=readiness-path-leak"),
            ("token%3Dreadiness-path-leak.summary.json", "token=readiness-path-leak"),
            (
                "private%20key%3Dreadiness-path-leak.summary.json",
                "private key=readiness-path-leak",
            ),
            (
                "private%20key-readiness-path-secret.summary.json",
                "private key-readiness-path-secret",
            ),
            (
                "private/key-readiness-path-secret.summary.json",
                "private/key-readiness-path-secret",
            ),
            (
                "x%2firoha%2fsignature-readiness-path-secret.summary.json",
                "x/iroha/signature-readiness-path-secret",
            ),
            (
                "%70assword%253Dreadiness-path-leak.summary.json",
                "password=readiness-path-leak",
            ),
            ("token-readiness-path-secret.summary.json", "token-readiness-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._preflight_output_cli_paths(
                        ["--summary-out", raw_path], {"--summary-out"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("readiness-path-leak", message)

    def test_summary_output_rejects_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "readiness.summary.json"

            with self.assertRaisesRegex(
                READINESS.ReadinessError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                READINESS._write_text_output(output_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())
            with self.assertRaisesRegex(
                READINESS.ReadinessError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                READINESS._reject_repository_output_path(
                    Path("fixtures/iso20022/readiness.summary.json"),
                    "output path",
                )

    def test_summary_output_rejects_repository_fixture_before_input_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            output_path = root / "fixtures" / "iso20022" / "readiness.summary.json"

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(root / "missing-xsd.summary.json"),
                    "--evidence-summary",
                    str(root / "missing-evidence.summary.json"),
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

    def test_missing_summary_output_parent_is_not_created_before_input_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "xsd.summary.json"
            evidence_summary = root / "evidence.summary.json"
            xsd_summary.write_text("{not valid xsd json\n", encoding="utf-8")
            evidence_summary.write_text(
                "{not valid evidence json\n",
                encoding="utf-8",
            )
            summary_parent = root / "summary" / "new"
            summary_out = summary_parent / "readiness.summary.json"

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("not valid JSON", stderr)
            self.assertFalse(summary_parent.exists())

    def test_summary_output_cannot_reuse_input_summary_paths_before_loading(self):
        cases = (
            ("xsd", "--xsd-summary[0]"),
            ("evidence", "--evidence-summary[0]"),
        )
        for output_kind, expected_label in cases:
            with self.subTest(output_kind=output_kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    xsd_summary = root / "xsd.summary.json"
                    evidence_summary = root / "evidence.summary.json"
                    xsd_summary.write_text("{not valid xsd json\n", encoding="utf-8")
                    evidence_summary.write_text(
                        "{not valid evidence json\n",
                        encoding="utf-8",
                    )
                    summary_out = (
                        xsd_summary if output_kind == "xsd" else evidence_summary
                    )
                    original_text = summary_out.read_text(encoding="utf-8")

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--summary-out",
                            str(summary_out),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        f"summary_out must not reuse {expected_label} path",
                        stderr,
                    )
                    self.assertNotIn("not valid JSON", stderr)
                    self.assertEqual(
                        summary_out.read_text(encoding="utf-8"),
                        original_text,
                    )

    def test_summary_output_cannot_hardlink_input_summary_paths_before_loading(self):
        cases = (
            ("xsd", "--xsd-summary[0]"),
            ("evidence", "--evidence-summary[0]"),
        )
        for output_kind, expected_label in cases:
            with self.subTest(output_kind=output_kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    xsd_summary = root / "xsd.summary.json"
                    evidence_summary = root / "evidence.summary.json"
                    xsd_summary.write_text("{not valid xsd json\n", encoding="utf-8")
                    evidence_summary.write_text(
                        "{not valid evidence json\n",
                        encoding="utf-8",
                    )
                    input_path = (
                        xsd_summary if output_kind == "xsd" else evidence_summary
                    )
                    summary_out = root / f"{output_kind}-readiness-alias.summary.json"
                    try:
                        summary_out.hardlink_to(input_path)
                    except OSError as error:
                        self.skipTest(f"hard link creation unavailable: {error}")
                    original_text = input_path.read_text(encoding="utf-8")

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--summary-out",
                            str(summary_out),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        f"summary_out must not reuse {expected_label} path",
                        stderr,
                    )
                    self.assertNotIn("not valid JSON", stderr)
                    self.assertEqual(
                        input_path.read_text(encoding="utf-8"),
                        original_text,
                    )
                    self.assertEqual(
                        summary_out.read_text(encoding="utf-8"),
                        original_text,
                    )

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (READINESS.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "raw overlong",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                overlong_path,
                f"no longer than {READINESS.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "output overlong",
                lambda raw: READINESS._reject_output_path_smuggling(Path(raw), "output path"),
                overlong_path,
                f"no longer than {READINESS.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw format-control",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/summary\u202e.json",
                "control characters",
            ),
            (
                "output format-control",
                lambda raw: READINESS._reject_output_path_smuggling(Path(raw), "output path"),
                "out/summary\u202e.json",
                "control characters",
            ),
            (
                "input format-control",
                lambda raw: READINESS._reject_path_smuggling(raw, "config_path"),
                "/ops/iso/readiness\u202e.json",
                "control characters",
            ),
            (
                "xsd source format-control",
                lambda raw: READINESS._validate_schema_source_path(raw, "source.path"),
                "schemas/camt\u202e.052.xsd",
                "control characters",
            ),
            (
                "xsd relative format-control",
                lambda raw: READINESS._validate_fixture_summary_path(
                    raw,
                    "fixtures[0].path",
                ),
                "fixtures/pacs\u202e.xml",
                "control characters",
            ),
            (
                "raw encoded dot",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/summary.json",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: READINESS._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/summary.json",
                "encoded dot or separator",
            ),
            (
                "raw uri prefix",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/summary.json",
                "URI or drive prefixes",
            ),
            (
                "input drive prefix",
                lambda raw: READINESS._reject_path_smuggling(raw, "config_path"),
                "C:/ops/readiness.json",
                "URI or drive prefixes",
            ),
            (
                "xsd source drive prefix",
                lambda raw: READINESS._validate_schema_source_path(raw, "source.path"),
                "C:/schemas/camt.052.xsd",
                "URI or drive prefixes",
            ),
            (
                "xsd source encoded dot",
                lambda raw: READINESS._validate_schema_source_path(raw, "source.path"),
                "schemas/camt%2e052.xsd",
                "encoded dot or separator",
            ),
            (
                "xsd relative encoded semicolon",
                lambda raw: READINESS._validate_fixture_summary_path(
                    raw,
                    "fixtures[0].path",
                ),
                "fixtures/%3b/pacs.xml",
                "encoded semicolon",
            ),
            (
                "input encoded semicolon",
                lambda raw: READINESS._reject_path_smuggling(raw, "config_path"),
                "/ops/%3b/readiness.json",
                "encoded semicolon",
            ),
            (
                "input encoded delimiter",
                lambda raw: READINESS._reject_path_smuggling(raw, "config_path"),
                "/ops/%3f/readiness.json",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/summary.json",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/summary.json",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: READINESS._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/summary.json",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://pki.local-bank.bank/source:debug",
            "https://pki.local-bank.bank/source@debug",
            "https://pki.local-bank.bank/source[debug]",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._validate_https_source_url(url, "source.url")

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(url, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://pki\u0661.local-bank.bank/source",
                "host must use printable ASCII",
            ),
            (
                "https://pki.local-bank.bank/source\u202edebug",
                "must not contain control characters",
            ),
            ("https://pki.local-bank.bank/source∕debug", "path must use printable ASCII"),
            (
                "https://pki.local-bank.bank/source%c3%a9",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._validate_https_source_url(url, "source.url")

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)

    def test_source_urls_reject_secret_path_without_echo(self):
        cases = (
            "https://pki.example.com/source/token=readiness-url-secret",
            "https://pki.example.com/source/token-readiness-url-secret",
            "https://pki.example.com/source/token%3Dreadiness-url-secret",
            "https://pki.example.com/source/token%253Dreadiness-url-secret",
        )
        for url in cases:
            with self.subTest(url=url):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._validate_https_source_url(url, "source.url")

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(url, message)
                self.assertNotIn("token=", message)
                self.assertNotIn("readiness-url-secret", message)

    def test_source_urls_reject_secret_host_and_parser_errors_without_echo(self):
        cases = (
            (
                "https://token-readiness-host-secret.pki.example.com/source",
                "secret-looking material",
            ),
            ("https://[token-readiness-host-secret/source", "is not a valid URL"),
        )
        for url, expected in cases:
            with self.subTest(url=url):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    READINESS._validate_https_source_url(url, "source.url")

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(url, message)
                self.assertNotIn("token-readiness-host-secret", message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (
                ["--allow-reviewed-xsd-gaps=true"],
                "--allow-reviewed-xsd-gaps",
                "--allow-reviewed-xsd-gaps=true",
            ),
            (
                ["--allow-canary-stage-receipts-only", "true"],
                "--allow-canary-stage-receipts-only",
                "true",
            ),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_readiness(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_numeric_cli_flags_reject_malformed_values_without_echo(self):
        cases = (
            ["--max-xsd-age-days", "token=readiness-secret"],
            ["--max-evidence-age-days=token=readiness-secret"],
            ["--max-canary-age-days", "--summary-out"],
            ["--max-trust-source-age-days="],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_readiness(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("readiness-secret", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--max-xsd-age-days", hidden],
            [f"--max-evidence-age-days={hidden}"],
            ["--max-trust-source-age-days", hidden],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_readiness(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=readiness-secret"],
            ["token=readiness-secret"],
            ["password=readiness-secret"],
            ["--environment", "token=readiness-secret"],
            ["--environment", "%70assword%253Dreadiness-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_readiness(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("readiness-secret", stderr)

    def test_cli_identity_values_are_rejected_after_summary_args_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            base_argv = [
                "--xsd-summary",
                str(xsd_summary),
                "--evidence-summary",
                str(evidence_summary),
            ]
            cases = (
                (
                    ["--provider", "token-readiness-cli-secret", "--environment", "preprod"],
                    "token-readiness-cli-secret",
                ),
                (
                    [
                        "--provider",
                        "local-bank",
                        "--environment",
                        "private-key-readiness-cli-secret",
                    ],
                    "private-key-readiness-cli-secret",
                ),
            )
            for argv, secret in cases:
                with self.subTest(argv=argv):
                    rc, _stdout, stderr = run_readiness(
                        base_argv + argv,
                        include_context=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking", stderr)
                    self.assertNotIn(secret, stderr)

    def test_recursive_secret_field_scanner_does_not_echo_key_material(self):
        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"password_readiness_field_secret": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden secret-looking field", message)
        self.assertNotIn("password", message)
        self.assertNotIn("readiness_field_secret", message)
        self.assertNotIn("readiness-field-secret", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"private-key_readiness_field_secret": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden secret-looking field", message)
        self.assertNotIn("private-key", message)
        self.assertNotIn("readiness_field_secret", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"unexpected\x1breadiness_key": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden control-bearing field", message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unexpected", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"unexpected\u202ereadiness_key": "redacted"}
            )

        message = str(caught.exception)
        self.assertIn("forbidden control-bearing field", message)
        self.assertNotIn("\u202e", message)
        self.assertNotIn("readiness_key", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material({"metadata": "warning \x1b[31mred"})

        message = str(caught.exception)
        self.assertIn("unsafe control characters", message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("[31mred", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "warning \u202ereadiness-bidi-leak"}
            )

        message = str(caught.exception)
        self.assertIn("unsafe control characters", message)
        self.assertNotIn("\u202e", message)
        self.assertNotIn("readiness-bidi-leak", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "%70assword%253Dreadiness-field-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("%70assword%253Dreadiness-field-leak", message)
        self.assertNotIn("password=readiness-field-leak", message)
        self.assertNotIn("readiness-field-leak", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "private%E2%80%8Dkey=readiness-field-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("private%E2%80%8Dkey=readiness-field-leak", message)
        self.assertNotIn("private\u200dkey=readiness-field-leak", message)
        self.assertNotIn("readiness-field-leak", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "private%CC%81key=readiness-mark-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("private%CC%81key=readiness-mark-leak", message)
        self.assertNotIn("private\u0301key=readiness-mark-leak", message)
        self.assertNotIn("readiness-mark-leak", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "ｐｒｉｖａｔｅｋｅｙ=readiness-compat-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("ｐｒｉｖａｔｅｋｅｙ=readiness-compat-leak", message)
        self.assertNotIn("privatekey=readiness-compat-leak", message)
        self.assertNotIn("readiness-compat-leak", message)

    def test_context_cli_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--provider"],
            ["--provider", ""],
            ["--provider", "--environment"],
            ["--provider="],
            ["--provider=--environment"],
            ["--environment"],
            ["--environment", ""],
            ["--environment", "--summary-out"],
            ["--environment="],
            ["--environment=--summary-out"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_readiness(argv)

                self.assertEqual(rc, 2)
                self.assertIn("requires a context value", stderr)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "summary.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        READINESS.ReadinessError,
                        "max file bytes must be a positive integer",
                    ):
                        READINESS._read_regular_file(path, max_bytes=limit)

    def test_strict_xsd_and_production_evidence_pass(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            summary_out = root / "readiness.summary.json"
            summary_out.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["blockers"], [])
            self.assertEqual(summary["warnings"], [])
            self.assertEqual(
                summary["xsd_summaries"][0]["version"],
                READINESS.XSD_SUMMARY_VERSION,
            )
            self.assertIn("verified_at", summary["xsd_summaries"][0])
            self.assertEqual(
                summary["xsd_summaries"][0]["schema_sources"][0]["license"],
                "Apache-2.0",
            )
            self.assertIn("verified_at", summary["evidence_summaries"][0])
            self.assertFalse(
                any(key.startswith("_") for key in summary["evidence_summaries"][0])
            )
            self.assertEqual(
                summary["evidence_summaries"][0]["policy"],
                {
                    "provider": "local-bank",
                    "environment": "preprod",
                    "default_rail_profile": None,
                    "max_canary_age_days": 36500,
                    "max_trust_age_days": 36500,
                    "max_trust_source_age_days": 36500,
                },
            )
            self.assertEqual(summary["policy"]["max_xsd_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_evidence_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_canary_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_age_days"], 36500)
            self.assertEqual(summary["policy"]["max_trust_source_age_days"], 36500)
            self.assertTrue(summary["xsd_summaries"][0]["strict"]["validate_xml_schema"])
            self.assertTrue(
                summary["xsd_summaries"][0]["strict"][
                    "require_profile_schema_backed_versions"
                ]
            )
            self.assertRegex(summary["xsd_summaries"][0]["manifest_sha256"], r"^[0-9a-f]{64}$")
            self.assertRegex(
                summary["xsd_summaries"][0]["profile_catalog"]["sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertRegex(
                summary["xsd_summaries"][0]["profile_catalog"]["catalog_json_sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertEqual(summary["xsd_summaries"][0]["schema_validated_fixtures"], 1)
            self.assertEqual(summary["xsd_summaries"][0]["profile_checked_versions"], 1)
            self.assertEqual(summary["xsd_summaries"][0]["profile_schema_backed_versions"], 1)
            self.assertEqual(summary["xsd_summaries"][0]["blocked_schema_source_count"], 0)
            self.assertEqual(summary["xsd_summaries"][0]["blocked_schema_sources"], [])
            self.assertEqual(
                summary["xsd_summaries"][0][
                    "unreviewed_profile_schema_message_id_count"
                ],
                0,
            )
            self.assertEqual(
                summary["xsd_summaries"][0]["unreviewed_profile_schema_message_ids"],
                [],
            )
            self.assertTrue(
                summary["evidence_summaries"][0]["canary_summaries"][0][
                    "require_explicit_policy"
                ]
            )
            canary_summary = summary["evidence_summaries"][0]["canary_summaries"][0]
            self.assertEqual(canary_summary["version"], READINESS.CANARY_SUMMARY_VERSION)
            self.assertTrue(canary_summary["path"].endswith("canary.summary.json"))
            self.assertEqual(canary_summary["config_path"], "/ops/iso/canary.json")
            self.assertRegex(canary_summary["summary_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(canary_summary["started_at"], "2026-06-04T00:00:00+00:00")
            self.assertEqual(canary_summary["finished_at"], "2026-06-04T00:00:01+00:00")
            self.assertEqual(
                [stage["name"] for stage in canary_summary["stage_windows"]],
                ["rail", "notary", "verify"],
            )
            self.assertEqual(canary_summary["stage_names"], ["rail", "notary", "verify"])
            self.assertEqual(canary_summary["stage_dry_run"], [False, False, False])
            rail_receipt = next(
                receipt
                for receipt in canary_summary["receipt_summary"]["receipts"]
                if receipt["receipt_kind"] == "iso-rail-gateway"
            )
            notary_receipt = next(
                receipt
                for receipt in canary_summary["receipt_summary"]["receipts"]
                if receipt["receipt_kind"] == "iso-audit-notary"
            )
            self.assertTrue(notary_receipt["anchor_path"].endswith("latest.notary.json"))
            self.assertTrue(notary_receipt["store_dir"].endswith("store"))
            self.assertTrue(notary_receipt["index_path"].endswith("messages.index.json"))
            self.assertTrue(rail_receipt["source_path"].endswith("rail-status.xml"))
            trust_profile = summary["evidence_summaries"][0]["trust_summaries"][0]["profiles"][0]
            trust_summary = summary["evidence_summaries"][0]["trust_summaries"][0]
            self.assertEqual(trust_summary["version"], READINESS.TRUST_SUMMARY_VERSION)
            self.assertTrue(trust_summary["path"].endswith("trust.summary.json"))
            self.assertRegex(trust_summary["verified_at"], r"^\d{4}-\d{2}-\d{2}T")
            self.assertRegex(trust_summary["summary_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(trust_summary["max_source_age_days"], 36500)
            self.assertFalse(trust_summary["allow_synthetic_der"])
            self.assertFalse(trust_summary["allow_record_only"])
            self.assertFalse(trust_summary["allow_insecure_source_url"])
            self.assertTrue(trust_summary["profile_json_emitted"])
            self.assertTrue(trust_summary["profile_json_emittable"])
            self.assertRegex(trust_summary["profile_json_sha256"], r"^[0-9a-f]{64}$")
            self.assertTrue(trust_profile["path"].endswith("trust-bundle.json"))
            self.assertRegex(trust_profile["bundle_sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(
                trust_profile["source"],
                {
                    "authority": "Local Bank Rail PKI",
                    "version": "2026-Q2",
                    "url": "https://pki.local-bank.bank/swift-cbpr-plus",
                    "retrieved_at": "2026-06-04T00:00:00Z",
                },
            )
            self.assertTrue(trust_profile["x509_require_crl_revocation_check"])
            self.assertEqual(trust_profile["x509_crl_count"], 1)
            self.assertEqual(len(trust_profile["x509_crl_der"]), 1)
            self.assertRegex(trust_profile["x509_crl_der"][0]["sha256"], r"^[0-9a-f]{64}$")
            self.assertGreater(trust_profile["x509_crl_der"][0]["byte_len"], 0)
            self.assertTrue(trust_profile["x509_require_ocsp_revocation_check"])
            self.assertEqual(trust_profile["x509_ocsp_response_count"], 1)
            self.assertEqual(len(trust_profile["x509_ocsp_response_der"]), 1)
            self.assertRegex(
                trust_profile["x509_ocsp_response_der"][0]["sha256"],
                r"^[0-9a-f]{64}$",
            )
            self.assertGreater(trust_profile["x509_ocsp_response_der"][0]["byte_len"], 0)
            self.assertEqual(trust_profile["revoked_certificate_pin_count"], 1)
            self.assertEqual(len(trust_profile["x509_trust_anchor_der"]), 1)
            self.assertEqual(len(trust_profile["revoked_certificate_der"]), 1)
            self.assertEqual(trust_profile["x509_required_certificate_policy_oid_count"], 1)
            self.assertEqual(json.loads(summary_out.read_text(encoding="utf-8")), summary)
            self.assertEqual(summary_out.stat().st_mode & 0o077, 0)
            self.assertEqual(
                list(summary_out.parent.glob(".iso-*.tmp")),
                [],
            )
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(digest, READINESS.sha256_hex(READINESS._canonical_json_bytes(body)))

    def test_unused_local_readiness_overrides_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "--allow-reviewed-xsd-gaps",
                    "requires at least one reviewed XSD gap warning",
                ),
                (
                    "--allow-canary-stage-receipts-only",
                    "requires at least one evidence summary with canary-stage-only "
                    "receipt policy and missing direct receipt archive verification",
                ),
            )
            for flag, message in cases:
                with self.subTest(flag=flag):
                    summary_out = root / f"unused-{flag.removeprefix('--')}.summary.json"
                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                            flag,
                            "--summary-out",
                            str(summary_out),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertFalse(summary_out.exists())

    def test_summary_versions_are_rechecked_before_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            for name, mutate in (
                ("missing", lambda body: body.pop("version")),
                ("boolean", lambda body: body.__setitem__("version", True)),
                (
                    "unsupported",
                    lambda body: body.__setitem__(
                        "version",
                        READINESS.EVIDENCE_VERSION + 1,
                    ),
                ),
            ):
                with self.subTest(kind="evidence", name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_evidence = write_json(
                        root / f"evidence-{name}-version.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_evidence),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(".version must be 1", stderr)

            for name, mutate in (
                ("missing", lambda body: body.pop("version")),
                ("boolean", lambda body: body.__setitem__("version", True)),
                (
                    "unsupported",
                    lambda body: body.__setitem__(
                        "version",
                        READINESS.XSD_SUMMARY_VERSION + 1,
                    ),
                ),
            ):
                with self.subTest(kind="xsd", name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(xsd)
                    refresh_digest(xsd)
                    mutated_xsd = write_json(root / f"xsd-{name}-version.summary.json", xsd)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_xsd),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("xsd.summary_version_unsupported", codes)

            compact_cases = (
                (
                    "canary",
                    lambda evidence: evidence["canary_summaries"][0],
                    READINESS.CANARY_SUMMARY_VERSION,
                    "evidence.canary_summary_version_unsupported",
                ),
                (
                    "trust",
                    lambda evidence: evidence["trust_summaries"][0],
                    READINESS.TRUST_SUMMARY_VERSION,
                    "trust.summary_version_unsupported",
                ),
            )
            for kind, select, expected_version, code in compact_cases:
                for name, mutate in (
                    ("missing", lambda body: body.pop("version")),
                    ("boolean", lambda body: body.__setitem__("version", True)),
                    (
                        "unsupported",
                        lambda body, expected_version=expected_version: body.__setitem__(
                            "version",
                            expected_version + 1,
                        ),
                    ),
                ):
                    with self.subTest(kind=kind, name=name):
                        evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        mutate(select(evidence))
                        refresh_digest(evidence)
                        mutated_evidence = write_json(
                            root / f"{kind}-{name}-version.summary.json",
                            evidence,
                        )

                        rc, stdout, stderr = run_readiness(
                            [
                                "--xsd-summary",
                                str(xsd_summary),
                                "--evidence-summary",
                                str(mutated_evidence),
                            ]
                        )

                        self.assertEqual(rc, 1, stderr)
                        codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                        self.assertIn(code, codes)

    def test_symlinked_summary_output_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            target = root / "readiness-target.summary.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "readiness-link.summary.json"
            try:
                summary_out.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_summary_output_path_rejects_smuggled_segments(self):
        cases = (
            ("semicolon", "readiness;debug.summary.json", "semicolon path"),
            ("whitespace", "readiness summary.json", "whitespace"),
            ("leading-dash", "nested/-readiness.summary.json", "leading-dash"),
            ("parent", "nested/../readiness.summary.json", "dot or parent"),
            (
                "dot",
                lambda root: f"{root}/nested/./readiness.summary.json",
                "dot or parent",
            ),
            ("empty", lambda root: f"{root}//readiness.summary.json", "empty path"),
        )
        for name, summary_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    xsd_summary = write_strict_xsd_summary(root / "xsd")
                    evidence_summary = add_archive_receipt_verification(
                        write_evidence_summary(root / "evidence")
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
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
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            target = root / "readiness-target.summary.json"
            target.write_text("untouched\n", encoding="utf-8")
            summary_out = root / "readiness-hardlink.summary.json"
            try:
                summary_out.hardlink_to(target)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
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
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            target_dir = root / "readiness-target"
            target_dir.mkdir()
            ancestor = root / "readiness-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "readiness.summary.json"

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_symlinked_summary_output_ancestor_is_rejected_before_summary_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "xsd.summary.json"
            evidence_summary = root / "evidence.summary.json"
            xsd_summary.write_text("{not valid xsd json\n", encoding="utf-8")
            evidence_summary.write_text("{not valid evidence json\n", encoding="utf-8")
            target_dir = root / "readiness-target"
            target_dir.mkdir()
            ancestor = root / "readiness-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            summary_out = ancestor / "nested" / "readiness.summary.json"

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--summary-out",
                    str(summary_out),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn("not valid JSON", stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_cli_input_paths_reject_raw_smuggling_before_read(self):
        cases = (
            ("xsd semicolon", "--xsd-summary", "xsd;debug.summary.json", "semicolon path"),
            ("evidence whitespace", "--evidence-summary", "evidence summary.json", "whitespace"),
            ("xsd leading-dash", "--xsd-summary", "nested/-xsd.summary.json", "leading-dash"),
            (
                "evidence parent",
                "--evidence-summary",
                "nested/../evidence.summary.json",
                "dot or parent",
            ),
            (
                "xsd dot",
                "--xsd-summary",
                lambda root: f"{root}/nested/./xsd.summary.json",
                "dot or parent",
            ),
            (
                "evidence empty",
                "--evidence-summary",
                lambda root: f"{root}//evidence.summary.json",
                "empty path",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)

                    rc, stdout, stderr = run_readiness([flag, value])

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_direct_run_summary_paths_reject_smuggling_before_input_loading(self):
        def args_for(root, **overrides):
            values = {
                "xsd_summary": [root / "missing-xsd.summary.json"],
                "evidence_summary": [root / "missing-evidence.summary.json"],
                "provider": "local-bank",
                "environment": "preprod",
                "summary_out": root / "readiness.summary.json",
                "max_xsd_age_days": 36500,
                "max_evidence_age_days": 36500,
                "max_canary_age_days": 36500,
                "max_trust_age_days": 36500,
                "max_trust_source_age_days": 36500,
                "allow_reviewed_xsd_gaps": False,
                "allow_canary_stage_receipts_only": False,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "xsd whitespace",
                lambda root: args_for(
                    root,
                    xsd_summary=[root / "missing xsd.summary.json"],
                ),
                "--xsd-summary[0] must not contain whitespace",
            ),
            (
                "evidence parent",
                lambda root: args_for(
                    root,
                    evidence_summary=[root / "nested" / ".." / "evidence.summary.json"],
                ),
                "--evidence-summary[0] must not contain dot or parent segments",
            ),
            (
                "output leading dash",
                lambda root: args_for(
                    root,
                    summary_out=root / "nested" / "-readiness.summary.json",
                ),
                "summary_out must not contain leading-dash path segments",
            ),
        )
        for name, make_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaises(READINESS.ReadinessError) as caught:
                        READINESS.run(make_args(root))

                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)

        missing_cases = (
            (
                "xsd",
                {"xsd_summary": None},
                "provide at least one --xsd-summary",
            ),
            (
                "evidence",
                {"evidence_summary": None},
                "provide at least one --evidence-summary",
            ),
        )
        for name, overrides, message in missing_cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaisesRegex(READINESS.ReadinessError, message):
                        READINESS.run(args_for(root, **overrides))

    def test_direct_run_policy_flags_must_be_booleans_before_input_loading(self):
        missing = object()
        cases = (
            (
                "allow_reviewed_xsd_gaps",
                missing,
                "--allow-reviewed-xsd-gaps must be a boolean",
            ),
            (
                "allow_reviewed_xsd_gaps",
                "false",
                "--allow-reviewed-xsd-gaps must be a boolean",
            ),
            (
                "allow_canary_stage_receipts_only",
                1,
                "--allow-canary-stage-receipts-only must be a boolean",
            ),
        )
        for field, value, message in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        xsd_summary=[root / "missing-xsd.summary.json"],
                        evidence_summary=[root / "missing-evidence.summary.json"],
                        provider="local-bank",
                        environment="preprod",
                        summary_out=None,
                        max_xsd_age_days=36500,
                        max_evidence_age_days=36500,
                        max_canary_age_days=36500,
                        max_trust_age_days=36500,
                        max_trust_source_age_days=36500,
                        allow_reviewed_xsd_gaps=False,
                        allow_canary_stage_receipts_only=False,
                    )
                    if value is missing:
                        delattr(args, field)
                    else:
                        setattr(args, field, value)

                    stdout = io.StringIO()
                    with self.assertRaises(READINESS.ReadinessError) as caught:
                        with contextlib.redirect_stdout(stdout):
                            READINESS.run(args)

                    self.assertEqual(stdout.getvalue(), "")
                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)
                    self.assertNotIn(str(root), error)

    def test_direct_run_scalar_paths_must_be_paths_before_input_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            args = argparse.Namespace(
                xsd_summary=[root / "missing-xsd.summary.json"],
                evidence_summary=[root / "missing-evidence.summary.json"],
                provider="local-bank",
                environment="preprod",
                summary_out=object(),
                max_xsd_age_days=36500,
                max_evidence_age_days=36500,
                max_canary_age_days=36500,
                max_trust_age_days=36500,
                max_trust_source_age_days=36500,
                allow_reviewed_xsd_gaps=False,
                allow_canary_stage_receipts_only=False,
            )

            stdout = io.StringIO()
            with self.assertRaises(READINESS.ReadinessError) as caught:
                with contextlib.redirect_stdout(stdout):
                    READINESS.run(args)

            self.assertEqual(stdout.getvalue(), "")
            message = str(caught.exception)
            self.assertIn("summary_out must be a path", message)
            self.assertNotIn("does not exist", message)
            self.assertNotIn(str(root), message)

    def test_direct_run_scalar_context_must_be_strings_before_input_loading(self):
        missing = object()
        cases = (
            ("missing provider", "provider", missing, "provide --provider"),
            ("provider", "provider", object(), "--provider must be a string"),
            ("missing environment", "environment", missing, "provide --environment"),
            ("environment", "environment", object(), "--environment must be a string"),
        )
        for name, field, value, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        xsd_summary=[root / "missing-xsd.summary.json"],
                        evidence_summary=[root / "missing-evidence.summary.json"],
                        provider="local-bank",
                        environment="preprod",
                        summary_out=None,
                        max_xsd_age_days=36500,
                        max_evidence_age_days=36500,
                        max_canary_age_days=36500,
                        max_trust_age_days=36500,
                        max_trust_source_age_days=36500,
                        allow_reviewed_xsd_gaps=False,
                        allow_canary_stage_receipts_only=False,
                    )
                    if value is missing:
                        delattr(args, field)
                    else:
                        setattr(args, field, value)

                    stdout = io.StringIO()
                    with self.assertRaises(READINESS.ReadinessError) as caught:
                        with contextlib.redirect_stdout(stdout):
                            READINESS.run(args)

                    self.assertEqual(stdout.getvalue(), "")
                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_freshness_limits_must_exist_before_input_loading(self):
        cases = (
            ("max_xsd_age_days", "provide --max-xsd-age-days"),
            ("max_evidence_age_days", "provide --max-evidence-age-days"),
            ("max_canary_age_days", "provide --max-canary-age-days"),
            ("max_trust_age_days", "provide --max-trust-age-days"),
            ("max_trust_source_age_days", "provide --max-trust-source-age-days"),
        )
        for field, expected in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        xsd_summary=[root / "missing-xsd.summary.json"],
                        evidence_summary=[root / "missing-evidence.summary.json"],
                        provider="local-bank",
                        environment="preprod",
                        summary_out=None,
                        max_xsd_age_days=36500,
                        max_evidence_age_days=36500,
                        max_canary_age_days=36500,
                        max_trust_age_days=36500,
                        max_trust_source_age_days=36500,
                        allow_reviewed_xsd_gaps=False,
                        allow_canary_stage_receipts_only=False,
                    )
                    delattr(args, field)

                    stdout = io.StringIO()
                    with self.assertRaises(READINESS.ReadinessError) as caught:
                        with contextlib.redirect_stdout(stdout):
                            READINESS.run(args)

                    self.assertEqual(stdout.getvalue(), "")
                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_repeatable_summary_paths_must_be_lists_before_input_loading(self):
        cases = (
            ("xsd bare string", "xsd_summary", "xsd.summary.json", "--xsd-summary"),
            ("xsd bad entry", "xsd_summary", [object()], "--xsd-summary[0]"),
            (
                "evidence bare string",
                "evidence_summary",
                "evidence.summary.json",
                "--evidence-summary",
            ),
            (
                "evidence bad entry",
                "evidence_summary",
                [object()],
                "--evidence-summary[0]",
            ),
        )
        for name, field, value, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        xsd_summary=[root / "missing-xsd.summary.json"],
                        evidence_summary=[root / "missing-evidence.summary.json"],
                        provider="local-bank",
                        environment="preprod",
                        summary_out=None,
                        max_xsd_age_days=36500,
                        max_evidence_age_days=36500,
                        max_canary_age_days=36500,
                        max_trust_age_days=36500,
                        max_trust_source_age_days=36500,
                        allow_reviewed_xsd_gaps=False,
                        allow_canary_stage_receipts_only=False,
                    )
                    setattr(args, field, value)

                    stdout = io.StringIO()
                    with self.assertRaises(READINESS.ReadinessError) as caught:
                        with contextlib.redirect_stdout(stdout):
                            READINESS.run(args)

                    self.assertEqual(stdout.getvalue(), "")
                    error = str(caught.exception)
                    if "bare string" in name:
                        self.assertIn(f"{label} must be a repeatable path list", error)
                    else:
                        self.assertIn(f"{label} must be a path", error)
                    self.assertNotIn("does not exist", error)
                    self.assertNotIn(str(root), error)

    def test_secret_looking_cli_paths_are_rejected_before_summary_output(self):
        cases = (
            (
                "--xsd-summary",
                "token=readiness-xsd-secret.summary.json",
                "readiness-xsd-secret",
            ),
            (
                "--evidence-summary",
                "token=readiness-evidence-secret.summary.json",
                "readiness-evidence-secret",
            ),
            (
                "--summary-out",
                "token=readiness-output-secret.summary.json",
                "readiness-output-secret",
            ),
        )
        for flag, raw_path, secret in cases:
            with self.subTest(flag=flag):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    summary_out = root / "readiness.summary.json"
                    argv = [flag, str(root / raw_path)]
                    if flag != "--summary-out":
                        argv.extend(["--summary-out", str(summary_out)])

                    rc, stdout, stderr = run_readiness(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)
                    self.assertFalse(summary_out.exists())

    def test_symlinked_summary_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            xsd_target = write_strict_xsd_summary(root / "xsd-target")
            xsd_link = root / "xsd-link.summary.json"
            try:
                xsd_link.symlink_to(xsd_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_link), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_target = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-target")
            )
            evidence_link = root / "evidence-link.summary.json"
            try:
                evidence_link.symlink_to(evidence_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_link)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must not be a symlink", stderr)

    def test_symlinked_summary_input_ancestors_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            xsd_target_dir = root / "xsd-target"
            xsd_target = write_strict_xsd_summary(xsd_target_dir)
            xsd_ancestor = root / "xsd-ancestor-link"
            try:
                xsd_ancestor.symlink_to(xsd_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            xsd_summary = xsd_ancestor / xsd_target.name

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_target_dir = root / "evidence-target"
            evidence_target = add_archive_receipt_verification(
                write_evidence_summary(evidence_target_dir)
            )
            evidence_ancestor = root / "evidence-ancestor-link"
            try:
                evidence_ancestor.symlink_to(evidence_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            evidence_summary = evidence_ancestor / evidence_target.name

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_summary_input_path_diagnostics_do_not_echo_paths(self):
        cases = (
            ("xsd", "--xsd-summary", "xsd_summaries[0]"),
            ("evidence", "--evidence-summary", "evidence_summaries[0]"),
        )
        payloads = (
            ("malformed-json", b"{", "is not valid JSON"),
            ("non-utf8-json", b"\xff", "is not UTF-8 JSON"),
            ("not-object", b"[]", "must be a JSON object"),
        )
        for kind, _flag, label in cases:
            for name, payload, expected in payloads:
                with self.subTest(kind=kind, name=name):
                    with tempfile.TemporaryDirectory() as raw_root:
                        root = Path(raw_root)
                        hidden_dir = root / f"local-readiness-leak-{kind}-{name}"
                        hidden_dir.mkdir()
                        summary_path = hidden_dir / f"{kind}.summary.json"
                        summary_path.write_bytes(payload)
                        xsd_summary = (
                            summary_path
                            if kind == "xsd"
                            else write_strict_xsd_summary(root / "xsd")
                        )
                        evidence_summary = (
                            summary_path
                            if kind == "evidence"
                            else add_archive_receipt_verification(
                                write_evidence_summary(root / "evidence")
                            )
                        )

                        rc, stdout, stderr = run_readiness(
                            [
                                "--xsd-summary",
                                str(xsd_summary),
                                "--evidence-summary",
                                str(evidence_summary),
                            ]
                        )

                        self.assertEqual(rc, 2)
                        self.assertEqual(stdout, "")
                        self.assertIn(expected, stderr)
                        self.assertIn(label, stderr)
                        self.assertNotIn(str(summary_path), stderr)
                        self.assertNotIn(hidden_dir.name, stderr)

    def test_summary_input_symlink_ancestor_diagnostics_do_not_echo_paths(self):
        cases = (
            ("xsd", "xsd_summaries[0]"),
            ("evidence", "evidence_summaries[0]"),
        )
        for kind, label in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    target_dir = root / f"{kind}-target"
                    target_dir.mkdir()
                    target = (
                        write_strict_xsd_summary(target_dir)
                        if kind == "xsd"
                        else add_archive_receipt_verification(
                            write_evidence_summary(target_dir)
                        )
                    )
                    hidden_link = root / f"local-readiness-leak-{kind}-ancestor"
                    try:
                        hidden_link.symlink_to(target_dir, target_is_directory=True)
                    except OSError as error:
                        self.skipTest(f"symlink creation unavailable: {error}")
                    summary_path = hidden_link / target.name
                    xsd_summary = (
                        summary_path
                        if kind == "xsd"
                        else write_strict_xsd_summary(root / "xsd")
                    )
                    evidence_summary = (
                        summary_path
                        if kind == "evidence"
                        else add_archive_receipt_verification(
                            write_evidence_summary(root / "evidence")
                        )
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("must not be a symlink", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(summary_path), stderr)
                    self.assertNotIn(hidden_link.name, stderr)

    def test_directory_summary_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            xsd_dir = root / "xsd-dir.summary.json"
            xsd_dir.mkdir()

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_dir), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_dir = root / "evidence-dir.summary.json"
            evidence_dir.mkdir()

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_dir)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("must be a regular file", stderr)

    def test_oversized_summary_inputs_are_rejected_before_validation(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            xsd_path = root / "oversized-xsd.summary.json"
            xsd_path.write_text(
                '{"padding":"' + ("a" * READINESS.MAX_SUMMARY_JSON_BYTES) + '"}',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_path), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds", stderr)

            xsd_summary = write_strict_xsd_summary(root / "xsd")
            oversized_evidence_path = root / "oversized-evidence.summary.json"
            oversized_evidence_path.write_text(
                '{"padding":"' + ("a" * READINESS.MAX_SUMMARY_JSON_BYTES) + '"}',
                encoding="utf-8",
            )

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(oversized_evidence_path),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds", stderr)

    def test_oversized_summary_input_diagnostics_do_not_echo_paths(self):
        cases = (
            ("xsd", "xsd_summaries[0]"),
            ("evidence", "evidence_summaries[0]"),
        )
        for kind, label in cases:
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    hidden_dir = root / f"local-readiness-leak-{kind}-oversized"
                    hidden_dir.mkdir()
                    summary_path = hidden_dir / f"{kind}.summary.json"
                    summary_path.write_text(
                        '{"padding":"'
                        + ("a" * READINESS.MAX_SUMMARY_JSON_BYTES)
                        + '"}',
                        encoding="utf-8",
                    )
                    xsd_summary = (
                        summary_path
                        if kind == "xsd"
                        else write_strict_xsd_summary(root / "xsd")
                    )
                    evidence_summary = (
                        summary_path
                        if kind == "evidence"
                        else add_archive_receipt_verification(
                            write_evidence_summary(root / "evidence")
                        )
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("exceeds", stderr)
                    self.assertIn(label, stderr)
                    self.assertNotIn(str(summary_path), stderr)
                    self.assertNotIn(hidden_dir.name, stderr)

    def test_xsd_and_evidence_summaries_are_required(self):
        rc, _stdout, stderr = run_readiness([])

        self.assertEqual(rc, 2)
        self.assertIn("provide at least one --xsd-summary", stderr)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")

            rc, _stdout, stderr = run_readiness(["--xsd-summary", str(xsd_summary)])

            self.assertEqual(rc, 2)
            self.assertIn("provide at least one --evidence-summary", stderr)

    def test_provider_and_environment_are_required_release_context(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            base_argv = [
                "--xsd-summary",
                str(xsd_summary),
                "--evidence-summary",
                str(evidence_summary),
            ]

            rc, _stdout, stderr = run_readiness(base_argv, include_context=False)
            self.assertEqual(rc, 2)
            self.assertIn("provide --provider", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-bank"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --environment", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", " ", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --provider", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-bank ", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--provider must not have surrounding whitespace", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-bank", "--environment", " preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--environment must not have surrounding whitespace", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-b\u00e1nk", "--environment", "preprod"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--provider must use printable ASCII", stderr)
            self.assertNotIn("local-b\u00e1nk", stderr)

            rc, _stdout, stderr = run_readiness(
                base_argv + ["--provider", "local-bank", "--environment", "prepr\u043ed"],
                include_context=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("--environment must use printable ASCII", stderr)
            self.assertNotIn("prepr\u043ed", stderr)

    def test_freshness_budgets_are_required_and_positive(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            base_argv = [
                "--xsd-summary",
                str(xsd_summary),
                "--evidence-summary",
                str(evidence_summary),
                "--provider",
                "local-bank",
                "--environment",
                "preprod",
            ]

            rc, _stdout, stderr = run_readiness(
                base_argv,
                include_context=False,
                include_freshness=False,
            )
            self.assertEqual(rc, 2)
            self.assertIn("provide --max-xsd-age-days", stderr)

            for flag in FRESHNESS_FLAGS:
                with self.subTest(flag=flag):
                    argv = list(base_argv)
                    for other_flag, value in FRESHNESS_FLAGS.items():
                        argv.extend([other_flag, "0" if other_flag == flag else value])

                    rc, _stdout, stderr = run_readiness(
                        argv,
                        include_context=False,
                        include_freshness=False,
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"{flag} must be a positive integer", stderr)

    def test_duplicate_input_and_compact_summaries_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            copied_xsd = root / "copied-xsd.summary.json"
            copied_xsd.write_text(xsd_summary.read_text(encoding="utf-8"), encoding="utf-8")
            copied_evidence = root / "copied-evidence.summary.json"
            copied_evidence.write_text(
                evidence_summary.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            duplicate_canary_path = json.loads(evidence_summary.read_text(encoding="utf-8"))
            duplicate_canary_path["canary_summaries"].append(
                dict(duplicate_canary_path["canary_summaries"][0])
            )
            refresh_digest(duplicate_canary_path)
            duplicate_canary_path_file = write_json(
                root / "duplicate-canary-path.evidence.summary.json",
                duplicate_canary_path,
            )
            duplicate_canary_digest = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_canary = dict(duplicate_canary_digest["canary_summaries"][0])
            copied_canary["path"] = "/ops/iso/copied-canary.summary.json"
            duplicate_canary_digest["canary_summaries"].append(copied_canary)
            refresh_digest(duplicate_canary_digest)
            duplicate_canary_digest_file = write_json(
                root / "duplicate-canary-digest.evidence.summary.json",
                duplicate_canary_digest,
            )
            duplicate_trust_digest = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_trust = dict(duplicate_trust_digest["trust_summaries"][0])
            copied_trust["path"] = "/ops/iso/copied-trust.summary.json"
            duplicate_trust_digest["trust_summaries"].append(copied_trust)
            refresh_digest(duplicate_trust_digest)
            duplicate_trust_digest_file = write_json(
                root / "duplicate-trust-digest.evidence.summary.json",
                duplicate_trust_digest,
            )
            cases = (
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "--xsd-summary[1] duplicates --xsd-summary[0]",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "--evidence-summary[1] duplicates --evidence-summary[0]",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--xsd-summary",
                        str(copied_xsd),
                        "--evidence-summary",
                        str(evidence_summary),
                    ],
                    "xsd_summaries[1].summary_sha256 duplicates xsd_summaries[0].summary_sha256",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(evidence_summary),
                        "--evidence-summary",
                        str(copied_evidence),
                    ],
                    "evidence_summaries[1].summary_sha256 duplicates evidence_summaries[0].summary_sha256",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_canary_path_file),
                    ],
                    "canary_summaries[1].path duplicates",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_canary_digest_file),
                    ],
                    "canary_summaries[1].summary_sha256 duplicates",
                ),
                (
                    [
                        "--xsd-summary",
                        str(xsd_summary),
                        "--evidence-summary",
                        str(duplicate_trust_digest_file),
                    ],
                    "trust_summaries[1].summary_sha256 duplicates",
                ),
            )
            for argv, message in cases:
                with self.subTest(message=message):
                    rc, _stdout, stderr = run_readiness(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_repository_fixture_summary_paths_are_production_blockers(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            fixture_root = root / "fixtures" / "iso20022"
            fixture_root.mkdir(parents=True)
            fixture_evidence = fixture_root / "evidence.summary.json"
            fixture_evidence.write_text(
                evidence_summary.read_text(encoding="utf-8"),
                encoding="utf-8",
            )

            compact_canary_path = json.loads(evidence_summary.read_text(encoding="utf-8"))
            compact_canary_path["canary_summaries"][0]["path"] = (
                "/ops/fixtures/iso20022/canary.summary.json"
            )
            refresh_digest(compact_canary_path)
            compact_canary_file = write_json(
                root / "repository-canary-summary.evidence.summary.json",
                compact_canary_path,
            )

            compact_trust_path = json.loads(evidence_summary.read_text(encoding="utf-8"))
            compact_trust_path["trust_summaries"][0]["path"] = (
                "/ops/fixtures/iso20022/trust.summary.json"
            )
            refresh_digest(compact_trust_path)
            compact_trust_file = write_json(
                root / "repository-trust-summary.evidence.summary.json",
                compact_trust_path,
            )

            cases = (
                (
                    fixture_evidence,
                    "evidence.repository_evidence_summary",
                ),
                (
                    compact_canary_file,
                    "evidence.repository_canary_summary",
                ),
                (
                    compact_trust_file,
                    "trust.repository_trust_summary",
                ),
            )
            for evidence_path, code in cases:
                with self.subTest(code=code):
                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_duplicate_summary_paths_do_not_echo_secret_segments(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            secret_xsd = root / "token=readiness-duplicate-secret.xsd.summary.json"
            secret_xsd.write_text(xsd_summary.read_text(encoding="utf-8"), encoding="utf-8")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(secret_xsd),
                    "--xsd-summary",
                    str(secret_xsd),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("secret-looking material", stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("readiness-duplicate-secret", stderr)

    def test_xsd_material_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_one = write_strict_xsd_summary(root / "xsd-one")
            xsd_two = write_strict_xsd_summary(root / "xsd-two")
            first_summary = json.loads(xsd_one.read_text(encoding="utf-8"))
            first_summary["blocked_schema_sources"] = [
                xsd_test.blocked_schema_source("barr.001.001.01")
            ]
            first_summary["blocked_schema_source_count"] = 1
            first_summary["pending_schema_sources"] = [
                xsd_test.pending_schema_source("bazz.001.001.01")
            ]
            first_summary["pending_schema_source_count"] = 1
            refresh_digest(first_summary)
            write_json(xsd_one, first_summary)
            second_summary = json.loads(xsd_two.read_text(encoding="utf-8"))
            second_summary["verified_at"] = "2026-06-04T00:00:01+00:00"
            second_summary["blocked_schema_sources"] = [
                xsd_test.blocked_schema_source("barr.001.001.01")
            ]
            second_summary["blocked_schema_source_count"] = 1
            second_summary["pending_schema_sources"] = [
                xsd_test.pending_schema_source("bazz.001.001.01")
            ]
            second_summary["pending_schema_source_count"] = 1
            second_summary["manifest"] = first_summary["manifest"]
            second_summary["profile_catalog"]["path"] = first_summary[
                "profile_catalog"
            ]["path"]
            refresh_digest(second_summary)
            write_json(xsd_two, second_summary)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_one),
                    "--xsd-summary",
                    str(xsd_two),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("xsd.schema_id_reused", codes)
            self.assertIn("xsd.schema_path_reused", codes)
            self.assertIn("xsd.schema_digest_reused", codes)
            self.assertIn("xsd.schema_source_reused", codes)
            self.assertIn("xsd.blocked_source_message_id_reused", codes)
            self.assertIn("xsd.blocked_source_reused", codes)
            self.assertIn("xsd.blocked_source_digest_reused", codes)
            self.assertIn("xsd.pending_source_message_id_reused", codes)
            self.assertIn("xsd.pending_source_message_name_reused", codes)
            self.assertIn("xsd.pending_source_reused", codes)
            self.assertIn("xsd.pending_source_download_url_reused", codes)
            self.assertIn("xsd.fixture_message_id_reused", codes)
            self.assertIn("xsd.fixture_path_reused", codes)
            self.assertIn("xsd.fixture_digest_reused", codes)
            self.assertIn("xsd.manifest_path_reused", codes)
            self.assertIn("xsd.manifest_digest_reused", codes)
            self.assertIn("xsd.profile_catalog_path_reused", codes)
            self.assertIn("xsd.profile_catalog_digest_reused", codes)
            self.assertIn("xsd.profile_catalog_json_digest_reused", codes)
            self.assertFalse(
                any(
                    key.startswith("_")
                    for xsd_summary in summary["xsd_summaries"]
                    for key in xsd_summary
                )
            )

    def test_xsd_pending_source_message_names_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_one = write_strict_xsd_summary(root / "xsd-one")
            xsd_two = write_strict_xsd_summary(root / "xsd-two")
            first_summary = json.loads(xsd_one.read_text(encoding="utf-8"))
            first_summary["pending_schema_sources"] = [
                xsd_test.pending_schema_source("barr.001.001.01")
            ]
            first_summary["pending_schema_source_count"] = 1
            refresh_digest(first_summary)
            write_json(xsd_one, first_summary)
            second_summary = json.loads(xsd_two.read_text(encoding="utf-8"))
            second_summary["verified_at"] = "2026-06-04T00:00:01+00:00"
            second_summary["pending_schema_sources"] = [
                xsd_test.pending_schema_source("bazz.001.001.01")
            ]
            second_summary["pending_schema_sources"][0]["source"][
                "download_url"
            ] = "https://www.iso20022.org/message/12346/download"
            second_summary["pending_schema_source_count"] = 1
            refresh_digest(second_summary)
            write_json(xsd_two, second_summary)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_one),
                    "--xsd-summary",
                    str(xsd_two),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("xsd.pending_source_message_name_reused", codes)
            self.assertNotIn("xsd.pending_source_message_id_reused", codes)
            self.assertNotIn("xsd.pending_source_reused", codes)
            self.assertNotIn("xsd.pending_source_download_url_reused", codes)

    def test_xsd_material_paths_cannot_reuse_other_xsd_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def add_blocked_source(body):
                body["blocked_schema_sources"] = [
                    xsd_test.blocked_schema_source("barr.001.001.01")
                ]
                body["blocked_schema_source_count"] = 1

            def summary_reuses_manifest(body, target_path):
                body["manifest"] = str(target_path)

            def summary_reuses_profile_catalog(body, target_path):
                body["profile_catalog"]["path"] = str(target_path)

            def manifest_reuses_schema(body, _target_path):
                body["manifest"] = body["schemas"][0]["path"]

            def canonicalize_fixture_path(body):
                body["fixtures"][0]["path"] = "foo_fixture.xml"
                return body["fixtures"][0]["path"]

            def manifest_reuses_fixture(body, _target_path):
                body["manifest"] = canonicalize_fixture_path(body)

            def profile_catalog_reuses_manifest(body, _target_path):
                body["profile_catalog"]["path"] = body["manifest"]

            def profile_catalog_reuses_schema(body, _target_path):
                body["profile_catalog"]["path"] = body["schemas"][0]["path"]

            def profile_catalog_reuses_fixture(body, _target_path):
                body["profile_catalog"]["path"] = canonicalize_fixture_path(body)

            def profile_catalog_reuses_blocked_source(body, _target_path):
                add_blocked_source(body)
                body["profile_catalog"]["path"] = body["blocked_schema_sources"][0][
                    "source"
                ]["path"]

            cases = (
                ("summary-manifest", summary_reuses_manifest),
                ("summary-profile-catalog", summary_reuses_profile_catalog),
                ("manifest-schema", manifest_reuses_schema),
                ("manifest-fixture", manifest_reuses_fixture),
                ("profile-catalog-manifest", profile_catalog_reuses_manifest),
                ("profile-catalog-schema", profile_catalog_reuses_schema),
                ("profile-catalog-fixture", profile_catalog_reuses_fixture),
                ("profile-catalog-blocked-source", profile_catalog_reuses_blocked_source),
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutated_path = root / f"xsd-path-role-{name}.summary.json"
                    mutate(body, mutated_path)
                    refresh_digest(body)
                    write_json(mutated_path, body)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("xsd.material_path_role_reused", codes)

    def test_evidence_material_cannot_be_reused_across_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second["verified_at"] = "2026-06-04T00:00:01+00:00"
            second["canary_summaries"] = json.loads(json.dumps(first["canary_summaries"]))
            second["trust_summaries"] = json.loads(json.dumps(first["trust_summaries"]))
            second["receipt_verification"] = json.loads(
                json.dumps(first["receipt_verification"])
            )
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_summary_path_reused", codes)
            self.assertIn("evidence.canary_summary_digest_reused", codes)
            self.assertIn("evidence.trust_summary_path_reused", codes)
            self.assertIn("evidence.trust_summary_digest_reused", codes)
            self.assertIn("evidence.canary_receipt_path_reused", codes)
            self.assertIn("evidence.canary_receipt_digest_reused", codes)
            self.assertIn("evidence.archive_receipt_path_reused", codes)
            self.assertIn("evidence.archive_receipt_digest_reused", codes)
            self.assertIn("evidence.canary_receipt_source_path_reused", codes)
            self.assertIn("evidence.canary_receipt_anchor_path_reused", codes)
            self.assertIn("evidence.archive_receipt_source_path_reused", codes)
            self.assertIn("evidence.archive_receipt_anchor_path_reused", codes)
            self.assertIn("trust.profile_id_reused", codes)
            self.assertIn("trust.bundle_digest_reused", codes)

    def test_receipt_source_material_cannot_be_reused_across_relabelled_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second_profile_id = "swift-cbpr-plus-two"
            profile = second["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = second_profile_id
            profile["bundle_sha256"] = "b" * 64

            source_fields = {
                "iso-audit-notary": (
                    "anchor_path",
                    "anchor_sha256",
                    "store_dir",
                    "index_path",
                    "index_sha256",
                ),
                "iso-rail-gateway": ("source_path", "payload_sha256"),
            }
            relabelled_receipts = {
                "iso-audit-notary": (
                    "/ops/iso/relabelled/notary.replayed-source.receipt.json",
                    "8" * 64,
                ),
                "iso-rail-gateway": (
                    "/ops/iso/relabelled/rail.replayed-source.receipt.json",
                    "9" * 64,
                ),
            }

            def copy_source_material(target_summary, source_summary):
                source_receipts = {
                    receipt["receipt_kind"]: receipt
                    for receipt in source_summary["receipts"]
                }
                for receipt in target_summary["receipts"]:
                    relabelled_path, relabelled_digest = relabelled_receipts[
                        receipt["receipt_kind"]
                    ]
                    receipt["path"] = relabelled_path
                    receipt["receipt_sha256"] = relabelled_digest
                    if receipt["receipt_kind"] == "iso-rail-gateway":
                        receipt["profile"] = second_profile_id
                    source_receipt = source_receipts[receipt["receipt_kind"]]
                    for field in source_fields[receipt["receipt_kind"]]:
                        receipt[field] = source_receipt[field]
                refresh_digest(target_summary)

            copy_source_material(
                second["canary_summaries"][0]["receipt_summary"],
                first["canary_summaries"][0]["receipt_summary"],
            )
            copy_source_material(
                second["receipt_verification"],
                first["receipt_verification"],
            )
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            expected = {
                "evidence.canary_receipt_source_path_reused",
                "evidence.canary_receipt_payload_digest_reused",
                "evidence.canary_receipt_anchor_path_reused",
                "evidence.canary_receipt_anchor_digest_reused",
                "evidence.canary_receipt_store_dir_reused",
                "evidence.canary_receipt_index_path_reused",
                "evidence.canary_receipt_index_digest_reused",
                "evidence.archive_receipt_source_path_reused",
                "evidence.archive_receipt_payload_digest_reused",
                "evidence.archive_receipt_anchor_path_reused",
                "evidence.archive_receipt_anchor_digest_reused",
                "evidence.archive_receipt_store_dir_reused",
                "evidence.archive_receipt_index_path_reused",
                "evidence.archive_receipt_index_digest_reused",
            }
            self.assertTrue(expected <= codes)
            self.assertNotIn("evidence.canary_receipt_path_reused", codes)
            self.assertNotIn("evidence.canary_receipt_digest_reused", codes)
            self.assertNotIn("evidence.archive_receipt_path_reused", codes)
            self.assertNotIn("evidence.archive_receipt_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_receipt_source_material_cannot_be_reused_within_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))

            def append_relabelled_copies(receipt_summary):
                base_receipts = json.loads(json.dumps(receipt_summary["receipts"]))
                for offset, receipt in enumerate(base_receipts):
                    copied = json.loads(json.dumps(receipt))
                    copied["path"] = (
                        f"/ops/iso/relabelled-within/"
                        f"{copied['receipt_kind']}.{offset}.receipt.json"
                    )
                    copied["receipt_sha256"] = f"{offset + 900:064x}"
                    copied["response_body_sha256"] = f"{offset + 950:064x}"
                    receipt_summary["receipts"].append(copied)
                receipt_summary["verified_receipts"] = len(receipt_summary["receipts"])
                refresh_digest(receipt_summary)

            append_relabelled_copies(
                evidence["canary_summaries"][0]["receipt_summary"],
            )
            append_relabelled_copies(evidence["receipt_verification"])
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "within-summary-source-material-replay.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            expected = {
                "evidence.canary_receipt_source_path_reused",
                "evidence.canary_receipt_payload_digest_reused",
                "evidence.archive_receipt_source_path_reused",
                "evidence.archive_receipt_payload_digest_reused",
            }
            self.assertTrue(expected <= codes)
            self.assertNotIn("evidence.canary_receipt_anchor_path_reused", codes)
            self.assertNotIn("evidence.canary_receipt_anchor_digest_reused", codes)
            self.assertNotIn("evidence.canary_receipt_index_path_reused", codes)
            self.assertNotIn("evidence.canary_receipt_index_digest_reused", codes)
            self.assertNotIn("evidence.archive_receipt_anchor_path_reused", codes)
            self.assertNotIn("evidence.archive_receipt_anchor_digest_reused", codes)
            self.assertNotIn("evidence.archive_receipt_index_path_reused", codes)
            self.assertNotIn("evidence.archive_receipt_index_digest_reused", codes)
            self.assertNotIn("evidence.receipt_path_duplicate", codes)
            self.assertNotIn("evidence.receipt_digest_duplicate", codes)
            self.assertNotIn("evidence.archive_receipt_path_duplicate", codes)
            self.assertNotIn("evidence.archive_receipt_digest_duplicate", codes)
            self.assertNotIn("evidence.canary_receipt_store_dir_reused", codes)
            self.assertNotIn("evidence.archive_receipt_store_dir_reused", codes)

    def test_duplicate_readiness_input_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "duplicate-xsd.summary.json"
            xsd_summary.write_text(
                '{"verified_schemas":1,"token=readiness-duplicate-key-secret":1,"token=readiness-duplicate-key-secret":2}\n',
                encoding="utf-8",
            )
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("readiness-duplicate-key-secret", stderr)

    def test_non_finite_readiness_input_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "nan-xsd.summary.json"
            xsd_summary.write_text('{"verified_schemas":NaN}\n', encoding="utf-8")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("NaN", stderr)

    def test_readiness_input_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = root / "surrogate-xsd.summary.json"
            xsd_summary.write_text('{"verified_schemas":"\\ud800"}\n', encoding="utf-8")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_xsd_summary_without_xml_schema_validation_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", xsd_test.minimal_manifest())
            profile_catalog = xsd_test.write_profile_catalog(root / "xsd" / "profiles.rs")
            xsd_summary = root / "xsd" / "xsd-no-validation.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--profile-catalog",
                    str(profile_catalog),
                    "--require-profile-schema-backed-versions",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.xml_schema_validation_not_proven", codes)

    def test_xsd_summary_without_profile_schema_proof_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", xsd_test.minimal_manifest())
            xsd_summary = root / "xsd" / "xsd-no-profile.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--validate-xml-schema",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.profile_schema_backed_not_proven", codes)

    def test_checked_in_xsd_gaps_block_by_default_and_can_be_diagnostic_warnings(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_checked_in_xsd_summary(
                root / "xsd",
                require_fixture_for_schema=True,
            )
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            blocked = json.loads(stdout)
            self.assertFalse(blocked["ok"])
            self.assertEqual(blocked["xsd_summaries"][0]["blocked_schema_source_count"], 3)
            self.assertEqual(
                sorted(
                    source["message_def_id"]
                    for source in blocked["xsd_summaries"][0]["blocked_schema_sources"]
                ),
                ["pacs.002.001.12", "pacs.008.001.10", "pacs.009.001.10"],
            )
            self.assertEqual(
                {blocker["code"] for blocker in blocked["blockers"]},
                {
                    "xsd.repository_fixture_manifest",
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
            )
            self.assertEqual(
                blocked["xsd_summaries"][0][
                    "unreviewed_profile_schema_message_id_count"
                ],
                0,
            )
            self.assertEqual(
                blocked["xsd_summaries"][0]["unreviewed_profile_schema_message_ids"],
                [],
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-reviewed-xsd-gaps",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            diagnostic = json.loads(stdout)
            self.assertFalse(diagnostic["ok"])
            self.assertEqual(
                diagnostic["xsd_summaries"][0]["blocked_schema_source_count"],
                3,
            )
            self.assertEqual(
                {blocker["code"] for blocker in diagnostic["blockers"]},
                {"xsd.repository_fixture_manifest"},
            )
            self.assertEqual(
                diagnostic["xsd_summaries"][0][
                    "unreviewed_profile_schema_message_id_count"
                ],
                0,
            )
            self.assertEqual(
                diagnostic["xsd_summaries"][0][
                    "unreviewed_profile_schema_message_ids"
                ],
                [],
            )
            self.assertEqual(
                {warning["code"] for warning in diagnostic["warnings"]},
                {
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
            )

    def test_xsd_gap_diagnostics_do_not_echo_reviewed_reason_text(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            schema_reason = "Reviewed schema-only gap: internal-ticket-readiness-427."
            missing_reason = "Reviewed missing schema package: internal-ticket-readiness-839."
            manifest = xsd_test.minimal_manifest()
            manifest["schemas"][0]["schema_only_reason"] = schema_reason
            manifest["fixtures"][0] = {
                "path": "../barr_fixture.xml",
                "message_def_id": "barr.001.001.01",
                "payload_root": "BarPayload",
                "missing_schema_reason": missing_reason,
            }
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", manifest)
            (root / "xsd" / "barr_fixture.xml").write_text(
                xsd_test.fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )
            xsd_summary = root / "xsd" / "reviewed-gap.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            for extra_args, collection, expected_rc in (
                ([], "blockers", 1),
                (["--allow-reviewed-xsd-gaps"], "warnings", 0),
            ):
                with self.subTest(collection=collection):
                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_summary),
                            *extra_args,
                        ]
                    )

                    self.assertEqual(rc, expected_rc, stderr)
                    self.assertNotIn(schema_reason, stdout)
                    self.assertNotIn(missing_reason, stdout)
                    self.assertNotIn("internal-ticket-readiness-427", stdout)
                    self.assertNotIn("internal-ticket-readiness-839", stdout)
                    self.assertNotIn(schema_reason, stderr)
                    self.assertNotIn(missing_reason, stderr)
                    result = json.loads(stdout)
                    gap_entries = [
                        entry
                        for item in result[collection]
                        if item["code"]
                        in {"xsd.missing_schema_fixtures", "xsd.schema_only_entries"}
                        for entry in item["entries"]
                    ]
                    self.assertEqual(
                        gap_entries,
                        [
                            {
                                "path": "../barr_fixture.xml",
                                "message_def_id": "barr.001.001.01",
                            },
                            {
                                "path": "iso/fooo.001.001.01.xsd",
                                "message_def_id": "fooo.001.001.01",
                            },
                        ],
                    )
                    self.assertTrue(all("reason" not in entry for entry in gap_entries))

    def test_allow_reviewed_xsd_gaps_keeps_unreviewed_profile_versions_blocking(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest = xsd_test.minimal_manifest()
            manifest["fixtures"].append(
                {
                    "path": "../barr_fixture.xml",
                    "message_def_id": "barr.001.001.01",
                    "payload_root": "BarPayload",
                    "missing_schema_reason": "Reviewed missing schema package.",
                }
            )
            manifest_path = xsd_test.write_minimal_tree(root / "xsd", manifest)
            (root / "xsd" / "barr_fixture.xml").write_text(
                xsd_test.fixture_xml("barr.001.001.01", "BarPayload"),
                encoding="utf-8",
            )
            profile_catalog = xsd_test.write_profile_catalog(
                root / "xsd" / "profiles.rs",
                catalog=[
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "barr.001",
                                "direction": "inbound",
                                "versions": ["barr.001.001.01"],
                            },
                            {
                                "message_type": "bazx.001",
                                "direction": "inbound",
                                "versions": ["bazx.001.001.01"],
                            },
                        ],
                    }
                ],
            )
            xsd_summary = root / "xsd" / "reviewed-and-unreviewed.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-fixture-for-schema",
                    "--profile-catalog",
                    str(profile_catalog),
                    "--validate-xml-schema",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-reviewed-xsd-gaps",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            result = json.loads(stdout)
            self.assertFalse(result["ok"])
            blocker_entries = [
                entry
                for blocker in result["blockers"]
                if blocker["code"] == "xsd.missing_profile_schema_versions"
                for entry in blocker["entries"]
            ]
            warning_entries = [
                entry
                for warning in result["warnings"]
                if warning["code"] == "xsd.missing_profile_schema_versions"
                for entry in warning["entries"]
            ]
            self.assertEqual(
                [entry["message_def_id"] for entry in blocker_entries],
                ["bazx.001.001.01"],
            )
            unique_blocker_entries = [
                entry
                for blocker in result["blockers"]
                if blocker["code"] == "xsd.unreviewed_profile_schema_message_ids"
                for entry in blocker["entries"]
            ]
            self.assertEqual(
                unique_blocker_entries,
                [
                    {
                        "message_def_id": "bazx.001.001.01",
                        "profile_version_count": 1,
                    }
                ],
            )
            unique_blocker_messages = [
                blocker["message"]
                for blocker in result["blockers"]
                if blocker["code"] == "xsd.unreviewed_profile_schema_message_ids"
            ]
            self.assertTrue(unique_blocker_messages)
            self.assertIn("pending-source evidence", unique_blocker_messages[0])
            self.assertEqual(
                [entry["message_def_id"] for entry in warning_entries],
                ["barr.001.001.01"],
            )

    def test_allow_reviewed_xsd_gaps_does_not_downgrade_unreviewed_profile_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(
                root / "xsd",
                xsd_test.minimal_manifest(),
            )
            profile_catalog = xsd_test.write_profile_catalog(
                root / "xsd" / "profiles.rs",
                versions=["fooo.001.001.01", "fooo.001.001.02"],
            )
            xsd_summary = root / "xsd" / "unreviewed-profile-gap.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--require-schema-backed-fixtures",
                    "--require-fixture-for-schema",
                    "--profile-catalog",
                    str(profile_catalog),
                    "--validate-xml-schema",
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-reviewed-xsd-gaps",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("requires at least one reviewed XSD gap warning", stderr)

    def test_repository_xsd_summary_path_is_production_blocker(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            fixture_root = root / "fixtures" / "iso20022" / "xsd"
            fixture_root.mkdir(parents=True)
            fixture_xsd_summary = fixture_root / "xsd.summary.json"
            fixture_xsd_summary.write_text(
                xsd_summary.read_text(encoding="utf-8"),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(fixture_xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.repository_xsd_summary", codes)

    def test_forged_xsd_reviewed_gap_entries_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            checked_in_summary = write_checked_in_xsd_summary(root / "checked-in-xsd")
            checked_in = json.loads(checked_in_summary.read_text(encoding="utf-8"))
            self.assertTrue(checked_in["missing_schema_fixtures"])
            missing_reason = json.loads(checked_in_summary.read_text(encoding="utf-8"))
            missing_reason["missing_schema_fixtures"][0]["reason"] = "forged gap"
            missing_path = json.loads(checked_in_summary.read_text(encoding="utf-8"))
            missing_path["missing_schema_fixtures"][0]["path"] = "../forged.xml"
            missing_order = json.loads(checked_in_summary.read_text(encoding="utf-8"))
            self.assertGreater(len(missing_order["missing_schema_fixtures"]), 1)
            missing_order["missing_schema_fixtures"] = list(
                reversed(missing_order["missing_schema_fixtures"])
            )
            missing_cases = (
                (
                    "missing-reason",
                    missing_reason,
                    "xsd.missing_schema_fixture_entries_mismatch",
                ),
                (
                    "missing-path",
                    missing_path,
                    "xsd.missing_schema_fixture_entries_mismatch",
                ),
                (
                    "missing-order",
                    missing_order,
                    "xsd.missing_schema_fixture_entries_order",
                ),
            )

            manifest = xsd_test.minimal_manifest()
            schema_only_entries = (
                ("fooo.001.001.02", "FooPayloadV2"),
                ("fooo.001.001.03", "FooPayloadV3"),
            )
            for schema_only_id, schema_only_payload in schema_only_entries:
                manifest["schemas"].append(
                    {
                        "path": f"iso/{schema_only_id}.xsd",
                        "message_def_id": schema_only_id,
                        "payload_root": schema_only_payload,
                        "schema_only_reason": "reviewed standalone fixture gap",
                        "source": xsd_test.source_provenance(
                            schema_only_id,
                            schema_only_payload,
                        ),
                    }
                )
            manifest_path = xsd_test.write_minimal_tree(root / "schema-only-xsd", manifest)
            for schema_only_id, schema_only_payload in schema_only_entries:
                (manifest_path.parent / "iso" / f"{schema_only_id}.xsd").write_text(
                    xsd_test.xsd_text(schema_only_id, schema_only_payload),
                    encoding="utf-8",
                )
            schema_only_summary = root / "schema-only.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--validate-xml-schema",
                    "--summary-out",
                    str(schema_only_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            schema_only = json.loads(schema_only_summary.read_text(encoding="utf-8"))
            self.assertEqual(len(schema_only["schema_only_entries"]), 2)
            schema_only_reason = json.loads(schema_only_summary.read_text(encoding="utf-8"))
            schema_only_reason["schema_only_entries"][0]["reason"] = "forged gap"
            schema_only_id_mismatch = json.loads(
                schema_only_summary.read_text(encoding="utf-8")
            )
            schema_only_id_mismatch["schema_only_entries"][0][
                "message_def_id"
            ] = "fooo.001.001.04"
            schema_only_order = json.loads(schema_only_summary.read_text(encoding="utf-8"))
            schema_only_order["schema_only_entries"] = list(
                reversed(schema_only_order["schema_only_entries"])
            )
            schema_only_cases = (
                (
                    "schema-only-reason",
                    schema_only_reason,
                    "xsd.schema_only_entries_mismatch",
                ),
                (
                    "schema-only-message-id",
                    schema_only_id_mismatch,
                    "xsd.schema_only_entries_mismatch",
                ),
                (
                    "schema-only-order",
                    schema_only_order,
                    "xsd.schema_only_entries_order",
                ),
            )

            for name, body, code in (*missing_cases, *schema_only_cases):
                with self.subTest(name=name):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"forged-gap-{name}.summary.json", body)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_xsd_gap_list_entries_reject_malformed_strings_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden_unicode_path = "unicod\u0435-gap-list-path"
            hidden_unicode_reason = "reviewed schem\u0430 gap"
            hidden_long_reason = "R" * (READINESS.MAX_REVIEWED_GAP_REASON_CHARS + 1)
            cases = (
                (
                    lambda body: body["missing_schema_fixtures"].append(
                        {
                            "path": f"../fixtures/{hidden_unicode_path}/ghost.xml",
                            "message_def_id": "fooo.001.001.01",
                            "reason": "reviewed missing schema package",
                        }
                    ),
                    "missing_schema_fixtures[0].path must use printable ASCII",
                    hidden_unicode_path,
                ),
                (
                    lambda body: body["missing_schema_fixtures"].append(
                        {
                            "path": "../fixtures/ghost.xml",
                            "message_def_id": "fooo.001.001.01",
                            "reason": hidden_unicode_reason,
                        }
                    ),
                    "missing_schema_fixtures[0].reason must use printable ASCII",
                    hidden_unicode_reason,
                ),
                (
                    lambda body: body["schema_only_entries"].append(
                        {
                            "path": "iso/%3f/fooo.001.001.01.xsd",
                            "message_def_id": "fooo.001.001.01",
                            "reason": "reviewed standalone fixture gap",
                        }
                    ),
                    "schema_only_entries[0].path must not contain encoded URL delimiter characters",
                    "%3f",
                ),
                (
                    lambda body: body["schema_only_entries"].append(
                        {
                            "path": "C:/iso/fooo.001.001.01.xsd",
                            "message_def_id": "fooo.001.001.01",
                            "reason": "reviewed standalone fixture gap",
                        }
                    ),
                    "schema_only_entries[0].path must not contain URI or drive prefixes",
                    "C:/iso",
                ),
                (
                    lambda body: body["schema_only_entries"].append(
                        {
                            "path": "iso/fooo.001.001.01.xsd",
                            "message_def_id": "fooo.001.001.01",
                            "reason": hidden_long_reason,
                        }
                    ),
                    "schema_only_entries[0].reason must be no longer than 1024 characters",
                    hidden_long_reason,
                ),
            )
            for offset, (mutate, message, hidden) in enumerate(cases):
                with self.subTest(offset=offset):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-gap-list-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_forged_xsd_summary_fixture_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            duplicate_fixture = json.loads(xsd_summary.read_text(encoding="utf-8"))
            copied_fixture = dict(duplicate_fixture["fixtures"][0])
            copied_fixture["path"] = "fixtures/copied_fixture.xml"
            duplicate_fixture["fixtures"].append(copied_fixture)
            duplicate_fixture["verified_fixtures"] += 1
            duplicate_fixture["schema_backed_fixtures"] += 1
            cases.append((duplicate_fixture, "xsd.fixture_digest_duplicate"))

            duplicate_fixture_message = json.loads(xsd_summary.read_text(encoding="utf-8"))
            copied_fixture = dict(duplicate_fixture_message["fixtures"][0])
            copied_fixture["path"] = "fixtures/copied_fixture_message.xml"
            copied_fixture["sha256"] = "2" * 64
            duplicate_fixture_message["fixtures"].append(copied_fixture)
            duplicate_fixture_message["verified_fixtures"] += 1
            duplicate_fixture_message["schema_backed_fixtures"] += 1
            if copied_fixture["schema_validated"]:
                duplicate_fixture_message["schema_validated_fixtures"] += 1
            cases.append((duplicate_fixture_message, "xsd.fixture_message_id_duplicate"))

            fixture_digest_reuses_schema = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_digest_reuses_schema["fixtures"][0]["sha256"] = (
                fixture_digest_reuses_schema["schemas"][0]["sha256"]
            )
            cases.append((fixture_digest_reuses_schema, "xsd.fixture_digest_matches_schema"))

            fixture_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_count["verified_fixtures"] += 1
            fixture_count["schema_backed_fixtures"] += 1
            cases.append((fixture_count, "xsd.fixture_count_mismatch"))

            target_namespace = json.loads(xsd_summary.read_text(encoding="utf-8"))
            target_namespace["schemas"][0]["target_namespace"] = (
                "urn:iso:std:iso:20022:tech:xsd:barr.001.001.01"
            )
            cases.append((target_namespace, "xsd.schema_target_namespace_mismatch"))

            fixture_message = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_message["fixtures"][0]["message_def_id"] = "barr.001.001.01"
            cases.append((fixture_message, "xsd.fixture_schema_message_mismatch"))

            fixture_payload = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_payload["fixtures"][0]["payload_root"] = "ForgedPayload"
            cases.append((fixture_payload, "xsd.fixture_payload_root_mismatch"))

            backed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            backed_count["fixtures"][0]["schema_backed"] = False
            backed_count["fixtures"][0]["schema"] = None
            backed_count["fixtures"][0]["missing_schema_reason"] = "forged gap"
            cases.append((backed_count, "xsd.schema_backed_count_mismatch"))

            schema_only_flag = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_flag["schemas"][0]["schema_only"] = True
            schema_only_flag["schemas"][0]["schema_only_reason"] = "forged gap"
            cases.append((schema_only_flag, "xsd.schema_only_flag_mismatch"))

            schema_only_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_reason["schemas"][0]["schema_only_reason"] = "forged gap"
            cases.append((schema_only_reason, "xsd.schema_only_reason_mismatch"))

            schema_only_missing_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_missing_reason["schemas"][0]["schema_only"] = True
            schema_only_missing_reason["schemas"][0]["schema_only_reason"] = None
            cases.append((schema_only_missing_reason, "xsd.schema_only_reason_absent"))

            missing_schema_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_schema_reason["fixtures"][0][
                "missing_schema_reason"
            ] = "forged missing schema gap"
            cases.append(
                (
                    missing_schema_reason,
                    "xsd.fixture_missing_schema_reason_mismatch",
                )
            )

            unbacked_checked_schema = json.loads(xsd_summary.read_text(encoding="utf-8"))
            unbacked_fixture = unbacked_checked_schema["fixtures"][0]
            unbacked_fixture["schema_backed"] = False
            unbacked_fixture["schema_validated"] = False
            unbacked_fixture["schema"] = None
            unbacked_fixture["missing_schema_reason"] = "forged missing schema gap"
            unbacked_checked_schema["schema_backed_fixtures"] -= 1
            unbacked_checked_schema["schema_validated_fixtures"] -= 1
            unbacked_checked_schema["missing_schema_fixtures"].append(
                {
                    "path": unbacked_fixture["path"],
                    "message_def_id": unbacked_fixture["message_def_id"],
                    "reason": unbacked_fixture["missing_schema_reason"],
                }
            )
            unbacked_schema = unbacked_checked_schema["schemas"][0]
            unbacked_schema["schema_only"] = True
            unbacked_schema["schema_only_reason"] = "forged standalone fixture gap"
            unbacked_checked_schema["schema_only_entries"].append(
                {
                    "path": unbacked_schema["path"],
                    "message_def_id": unbacked_schema["message_def_id"],
                    "reason": unbacked_schema["schema_only_reason"],
                }
            )
            cases.append(
                (
                    unbacked_checked_schema,
                    "xsd.fixture_missing_schema_matches_checked_schema",
                )
            )

            for offset, (body, code) in enumerate(cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"forged-xsd-{offset}.summary.json", body)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_xsd_reviewed_gap_reason_strings_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            schema_only_whitespace = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_whitespace["schemas"][0]["schema_only"] = True
            schema_only_whitespace["schemas"][0][
                "schema_only_reason"
            ] = " reviewed standalone fixture gap"
            cases.append(
                (
                    schema_only_whitespace,
                    "schema_only_reason must not have surrounding whitespace",
                )
            )

            schema_only_empty = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_empty["schemas"][0]["schema_only"] = True
            schema_only_empty["schemas"][0]["schema_only_reason"] = ""
            cases.append(
                (
                    schema_only_empty,
                    "schema_only_reason must be a non-empty string when provided",
                )
            )

            schema_only_numeric = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_numeric["schemas"][0]["schema_only"] = True
            schema_only_numeric["schemas"][0]["schema_only_reason"] = 7
            cases.append(
                (
                    schema_only_numeric,
                    "schema_only_reason must be a non-empty string when provided",
                )
            )

            schema_only_control = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only_control["schemas"][0]["schema_only"] = True
            schema_only_control["schemas"][0][
                "schema_only_reason"
            ] = "reviewed\nstandalone fixture gap"
            cases.append(
                (
                    schema_only_control,
                    "schema_only_reason must not contain control characters",
                )
            )

            missing_reason_whitespace = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason_whitespace["fixtures"][0]["schema_backed"] = False
            missing_reason_whitespace["fixtures"][0]["schema"] = None
            missing_reason_whitespace["fixtures"][0][
                "missing_schema_reason"
            ] = " reviewed missing schema package"
            cases.append(
                (
                    missing_reason_whitespace,
                    "missing_schema_reason must not have surrounding whitespace",
                )
            )

            missing_reason_empty = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason_empty["fixtures"][0]["schema_backed"] = False
            missing_reason_empty["fixtures"][0]["schema"] = None
            missing_reason_empty["fixtures"][0]["missing_schema_reason"] = ""
            cases.append(
                (
                    missing_reason_empty,
                    "missing_schema_reason must be a non-empty string when provided",
                )
            )

            missing_reason_numeric = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason_numeric["fixtures"][0]["schema_backed"] = False
            missing_reason_numeric["fixtures"][0]["schema"] = None
            missing_reason_numeric["fixtures"][0]["missing_schema_reason"] = 7
            cases.append(
                (
                    missing_reason_numeric,
                    "missing_schema_reason must be a non-empty string when provided",
                )
            )

            missing_reason_control = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason_control["fixtures"][0]["schema_backed"] = False
            missing_reason_control["fixtures"][0]["schema"] = None
            missing_reason_control["fixtures"][0][
                "missing_schema_reason"
            ] = "reviewed\nmissing schema package"
            cases.append(
                (
                    missing_reason_control,
                    "missing_schema_reason must not contain control characters",
                )
            )

            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-reviewed-gap-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_xsd_reviewed_gap_reasons_reject_non_ascii_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            hidden_schema_reason = "reviewed standal\u043ene fixture gap"
            schema_only = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only["schemas"][0]["schema_only"] = True
            schema_only["schemas"][0]["schema_only_reason"] = hidden_schema_reason

            hidden_missing_reason = "reviewed missing schem\u0430 package"
            missing_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason["fixtures"][0]["schema_backed"] = False
            missing_reason["fixtures"][0]["schema"] = None
            missing_reason["fixtures"][0]["missing_schema_reason"] = hidden_missing_reason

            hidden_blocked_reason = "candidate restricti\u043en requires review"
            blocked_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            blocked_reason["blocked_schema_sources"] = [
                {
                    **xsd_test.blocked_schema_source(),
                    "reason": hidden_blocked_reason,
                }
            ]
            blocked_reason["blocked_schema_source_count"] = 1

            cases = (
                (
                    schema_only,
                    "schemas[0].schema_only_reason must use printable ASCII",
                    hidden_schema_reason,
                ),
                (
                    missing_reason,
                    "fixtures[0].missing_schema_reason must use printable ASCII",
                    hidden_missing_reason,
                ),
                (
                    blocked_reason,
                    "blocked_schema_sources[0].reason must use printable ASCII",
                    hidden_blocked_reason,
                ),
            )

            for offset, (body, message, hidden) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-reviewed-gap-nonascii-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--allow-reviewed-xsd-gaps",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_xsd_reviewed_gap_reason_secrets_are_rejected_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            schema_secret = "readiness-schema-reason-secret"
            schema_only = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only["schemas"][0]["schema_only"] = True
            schema_only["schemas"][0]["schema_only_reason"] = (
                f"Reviewed gap private_key={schema_secret}"
            )

            missing_secret = "readiness-missing-reason-secret"
            missing_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason["fixtures"][0]["schema_backed"] = False
            missing_reason["fixtures"][0]["schema"] = None
            missing_reason["fixtures"][0]["missing_schema_reason"] = (
                f"Reviewed gap token={missing_secret}"
            )

            cases = (
                (
                    schema_only,
                    "schemas[0].schema_only_reason contains secret-looking material",
                    schema_secret,
                ),
                (
                    missing_reason,
                    "fixtures[0].missing_schema_reason contains secret-looking material",
                    missing_secret,
                ),
            )

            for offset, (body, message, secret) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-reviewed-gap-secret-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--allow-reviewed-xsd-gaps",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("private_key=", stderr)
                    self.assertNotIn(secret, stderr)

    def test_xsd_reviewed_gap_reasons_are_length_capped_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden = "A" * (READINESS.MAX_REVIEWED_GAP_REASON_CHARS + 1)

            schema_only = json.loads(xsd_summary.read_text(encoding="utf-8"))
            schema_only["schemas"][0]["schema_only"] = True
            schema_only["schemas"][0]["schema_only_reason"] = hidden

            missing_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_reason["fixtures"][0]["schema_backed"] = False
            missing_reason["fixtures"][0]["schema"] = None
            missing_reason["fixtures"][0]["missing_schema_reason"] = hidden

            blocked_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            blocked_reason["blocked_schema_sources"] = [
                {
                    **xsd_test.blocked_schema_source(),
                    "reason": hidden,
                }
            ]
            blocked_reason["blocked_schema_source_count"] = 1

            cases = (
                (
                    schema_only,
                    "schemas[0].schema_only_reason must be no longer than 1024 characters",
                ),
                (
                    missing_reason,
                    "fixtures[0].missing_schema_reason must be no longer than 1024 characters",
                ),
                (
                    blocked_reason,
                    "blocked_schema_sources[0].reason must be no longer than 1024 characters",
                ),
            )

            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-reviewed-gap-length-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--allow-reviewed-xsd-gaps",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_forged_xsd_schema_paths_are_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            malformed_cases = []
            parent_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            parent_path["schemas"][0]["path"] = "../fooo.001.001.01.xsd"
            malformed_cases.append((parent_path, "must not contain empty, dot, or parent segments"))

            backslash_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            backslash_path["schemas"][0]["path"] = r"iso\fooo.001.001.01.xsd"
            malformed_cases.append((backslash_path, "must use forward slashes"))

            whitespace_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            whitespace_path["schemas"][0]["path"] = "iso/fooo source.001.001.01.xsd"
            malformed_cases.append((whitespace_path, "path must not contain whitespace"))

            leading_dash_segment = json.loads(xsd_summary.read_text(encoding="utf-8"))
            leading_dash_segment["schemas"][0]["path"] = "iso/--fooo.001.001.01.xsd"
            malformed_cases.append(
                (
                    leading_dash_segment,
                    "path must not contain leading-dash path segments",
                )
            )

            semicolon_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            semicolon_path["schemas"][0]["path"] = "iso;debug/fooo.001.001.01.xsd"
            malformed_cases.append(
                (semicolon_path, "path must not contain semicolon path parameters")
            )

            non_xsd_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            non_xsd_path["schemas"][0]["path"] = "iso/fooo.001.001.01.xml"
            malformed_cases.append((non_xsd_path, "must point to an .xsd file"))

            fixture_schema_whitespace = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_schema_whitespace["fixtures"][0]["schema"] = (
                fixture_schema_whitespace["fixtures"][0]["schema"] + " "
            )
            malformed_cases.append(
                (
                    fixture_schema_whitespace,
                    "schema must not have surrounding whitespace",
                )
            )

            fixture_schema_control = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_schema_control["fixtures"][0]["schema"] = (
                fixture_schema_control["fixtures"][0]["schema"] + "\n"
            )
            malformed_cases.append(
                (
                    fixture_schema_control,
                    "schema must not contain control characters",
                )
            )

            fixture_schema_segment_dash = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_schema_segment_dash["fixtures"][0]["schema"] = (
                "iso/--fooo.001.001.01.xsd"
            )
            malformed_cases.append(
                (
                    fixture_schema_segment_dash,
                    "schema must not contain leading-dash path segments",
                )
            )

            for offset, (body, message) in enumerate(malformed_cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-schema-path-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            mismatched_filename = json.loads(xsd_summary.read_text(encoding="utf-8"))
            mismatched_filename["schemas"][0]["path"] = "iso/other.001.001.01.xsd"
            mismatched_filename["fixtures"][0]["schema"] = "iso/other.001.001.01.xsd"
            refresh_digest(mismatched_filename)
            mismatched_path = write_json(
                root / "forged-xsd-schema-path.summary.json",
                mismatched_filename,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(mismatched_path), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.schema_path_mismatch", codes)

    def test_forged_xsd_fixture_paths_are_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            backslash_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            backslash_path["fixtures"][0]["path"] = r"..\foo_fixture.xml"
            cases.append((backslash_path, "must use forward slashes"))

            absolute_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            absolute_path["fixtures"][0]["path"] = "/tmp/foo_fixture.xml"
            cases.append((absolute_path, "must be relative"))

            whitespace_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            whitespace_path["fixtures"][0]["path"] = "../foo fixture.xml"
            cases.append((whitespace_path, "path must not contain whitespace"))

            leading_dash_segment = json.loads(xsd_summary.read_text(encoding="utf-8"))
            leading_dash_segment["fixtures"][0]["path"] = "../fixtures/--foo_fixture.xml"
            cases.append(
                (
                    leading_dash_segment,
                    "path must not contain leading-dash path segments",
                )
            )

            semicolon_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            semicolon_path["fixtures"][0]["path"] = "../fixtures;debug/foo_fixture.xml"
            cases.append(
                (semicolon_path, "path must not contain semicolon path parameters")
            )

            non_xml_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            non_xml_path["fixtures"][0]["path"] = "../foo_fixture.txt"
            cases.append((non_xml_path, "must point to an .xml file"))

            dot_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            dot_path["fixtures"][0]["path"] = "fixtures/./foo_fixture.xml"
            cases.append((dot_path, "must not contain empty or dot segments"))

            empty_segment = json.loads(xsd_summary.read_text(encoding="utf-8"))
            empty_segment["fixtures"][0]["path"] = "fixtures//foo_fixture.xml"
            cases.append((empty_segment, "must not contain empty or dot segments"))

            nonleading_parent = json.loads(xsd_summary.read_text(encoding="utf-8"))
            nonleading_parent["fixtures"][0]["path"] = "fixtures/../foo_fixture.xml"
            cases.append((nonleading_parent, "parent segments must be leading"))

            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-fixture-path-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_xsd_relative_paths_reject_non_ascii_and_overlong_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden_unicode = "unicod\u0435-readiness-path"
            hidden_long = "x" * (READINESS.MAX_SOURCE_PATH_CHARS + 1)
            cases = (
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "path",
                        f"iso/{hidden_unicode}/fooo.001.001.01.xsd",
                    ),
                    "schemas[0].path must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "path",
                        "iso/" + hidden_long + "/fooo.001.001.01.xsd",
                    ),
                    "schemas[0].path must be no longer than 2048 characters",
                    hidden_long,
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "path",
                        f"../{hidden_unicode}/foo_fixture.xml",
                    ),
                    "fixtures[0].path must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "path",
                        "../" + hidden_long + "/foo_fixture.xml",
                    ),
                    "fixtures[0].path must be no longer than 2048 characters",
                    hidden_long,
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "schema",
                        f"iso/{hidden_unicode}/fooo.001.001.01.xsd",
                    ),
                    "fixtures[0].schema must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "schema",
                        "iso/" + hidden_long + "/fooo.001.001.01.xsd",
                    ),
                    "fixtures[0].schema must be no longer than 2048 characters",
                    hidden_long,
                ),
            )
            for offset, (mutate, message, hidden) in enumerate(cases):
                with self.subTest(offset=offset):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-relative-path-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_xsd_archived_identifiers_reject_unsafe_material_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden_unicode = "Payload\u0435Readiness"
            hidden_secret = "token=readiness-namespace-secret"
            hidden_long = "X" * (READINESS.MAX_XML_IDENTIFIER_CHARS + 1)
            cases = (
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "payload_root",
                        hidden_unicode,
                    ),
                    "schemas[0].payload_root must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "target_namespace",
                        f"urn:iso:std:iso:20022:tech:xsd:{hidden_secret}",
                    ),
                    "schemas[0].target_namespace contains secret-looking material",
                    hidden_secret,
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "payload_root",
                        hidden_long,
                    ),
                    "fixtures[0].payload_root must be no longer than 256 characters",
                    hidden_long,
                ),
            )
            for offset, (mutate, message, hidden) in enumerate(cases):
                with self.subTest(offset=offset):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-identifier-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_xsd_summary_paths_reject_encoded_smuggling_without_echo(self):
        def attach_blocked_sources(body, entries):
            body["blocked_schema_sources"] = entries
            body["blocked_schema_source_count"] = len(entries)
            return body

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "path",
                        "file:iso/fooo.001.001.01.xsd",
                    ),
                    "schemas[0].path must not contain URI or drive prefixes",
                    "file:iso",
                ),
                (
                    lambda body: body["schemas"][0].__setitem__(
                        "path",
                        "iso/%2e/fooo.001.001.01.xsd",
                    ),
                    "schemas[0].path must not contain encoded dot or separator characters",
                    "%2e",
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "path",
                        "../fixtures/%3b/foo_fixture.xml",
                    ),
                    "fixtures[0].path must not contain encoded semicolon parameters",
                    "%3b",
                ),
                (
                    lambda body: body["fixtures"][0].__setitem__(
                        "schema",
                        "iso/%3f/fooo.001.001.01.xsd",
                    ),
                    "fixtures[0].schema must not contain encoded URL delimiter characters",
                    "%3f",
                ),
                (
                    lambda body: body["schemas"][0]["source"].__setitem__(
                        "path",
                        "xsd/iso/fooo%2e001.001.01.xsd",
                    ),
                    "source.path must not contain encoded dot or separator characters",
                    "%2e",
                ),
                (
                    lambda body: (
                        attach_blocked_sources(body, [xsd_test.blocked_schema_source()]),
                        body["blocked_schema_sources"][0]["source"].__setitem__(
                            "path",
                            "xsd/%2f/barr.001.001.01.xsd",
                        ),
                    ),
                    "source.path must not contain encoded dot or separator characters",
                    "%2f",
                ),
            )
            for offset, (mutate, message, hidden) in enumerate(cases):
                with self.subTest(offset=offset):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"encoded-xsd-summary-path-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_rejected_xsd_summary_paths_do_not_echo_secret_absolute_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = [
                (
                    lambda body: body["schemas"][0].update(
                        {"path": "/tmp/token=readiness-schema-secret/fooo.001.001.01.xsd"}
                    ),
                    "readiness-schema-secret",
                ),
                (
                    lambda body: body["fixtures"][0].update(
                        {"path": "/tmp/token=readiness-fixture-secret/foo_fixture.xml"}
                    ),
                    "readiness-fixture-secret",
                ),
            ]
            for offset, (mutate, secret) in enumerate(cases):
                with self.subTest(secret=secret):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"absolute-secret-path-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)

    def test_forged_xsd_schema_source_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            malformed_cases = []
            missing_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_source["schemas"][0].pop("source")
            malformed_cases.append((missing_source, "source must be recorded"))

            null_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            null_source["schemas"][0]["source"] = None
            malformed_cases.append((null_source, "source must be a JSON object"))

            unknown_source_key = json.loads(xsd_summary.read_text(encoding="utf-8"))
            unknown_source_key["schemas"][0]["source"]["unexpected"] = "value"
            malformed_cases.append((unknown_source_key, "unknown keys"))

            escaped_source_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            escaped_source_path["schemas"][0]["source"]["path"] = "../fooo.001.001.01.xsd"
            malformed_cases.append((escaped_source_path, "must not contain empty, dot, or parent segments"))

            whitespace_source_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            whitespace_source_path["schemas"][0]["source"]["path"] = (
                "xsd/iso/fooo source.001.001.01.xsd"
            )
            malformed_cases.append(
                (
                    whitespace_source_path,
                    "source.path must not contain whitespace",
                )
            )

            leading_dash_source_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            leading_dash_source_path["schemas"][0]["source"]["path"] = (
                "xsd/iso/--fooo.001.001.01.xsd"
            )
            malformed_cases.append(
                (
                    leading_dash_source_path,
                    "source.path must not contain leading-dash path segments",
                )
            )

            semicolon_source_path = json.loads(xsd_summary.read_text(encoding="utf-8"))
            semicolon_source_path["schemas"][0]["source"]["path"] = (
                "xsd/iso;debug/fooo.001.001.01.xsd"
            )
            malformed_cases.append(
                (
                    semicolon_source_path,
                    "source.path must not contain semicolon path parameters",
                )
            )

            for offset, (body, message) in enumerate(malformed_cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-source-{offset}.summary.json",
                        body,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            secret_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            secret_repository["schemas"][0]["source"]["repository"] = (
                "https://github.com/moov-io/token-readiness-source-secret"
            )
            refresh_digest(secret_repository)
            secret_repository_path = write_json(
                root / "malformed-xsd-source-secret-repository.summary.json",
                secret_repository,
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(secret_repository_path),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("source.repository contains secret-looking material", stderr)
            self.assertNotIn("token-readiness-source-secret", stderr)

            blocker_cases = []
            bad_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_repository["schemas"][0]["source"]["repository"] += ".git"
            blocker_cases.append((bad_repository, "xsd.schema_source_repository_invalid"))

            uppercase_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            uppercase_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/Moov-IO/fedwire20022"
            blocker_cases.append(
                (uppercase_repository, "xsd.schema_source_repository_invalid")
            )

            underscore_owner_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            underscore_owner_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov_io/fedwire20022"
            blocker_cases.append(
                (underscore_owner_repository, "xsd.schema_source_repository_invalid")
            )

            leading_hyphen_owner_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            leading_hyphen_owner_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/-moov-io/fedwire20022"
            blocker_cases.append(
                (leading_hyphen_owner_repository, "xsd.schema_source_repository_invalid")
            )

            trailing_hyphen_owner_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            trailing_hyphen_owner_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io-/fedwire20022"
            blocker_cases.append(
                (trailing_hyphen_owner_repository, "xsd.schema_source_repository_invalid")
            )

            punctuation_only_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            punctuation_only_name_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io/---"
            blocker_cases.append(
                (punctuation_only_name_repository, "xsd.schema_source_repository_invalid")
            )

            leading_punctuation_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            leading_punctuation_name_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io/-fedwire20022"
            blocker_cases.append(
                (
                    leading_punctuation_name_repository,
                    "xsd.schema_source_repository_invalid",
                )
            )

            trailing_punctuation_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            trailing_punctuation_name_repository["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io/fedwire20022."
            blocker_cases.append(
                (
                    trailing_punctuation_name_repository,
                    "xsd.schema_source_repository_invalid",
                )
            )

            placeholder_repository_owner = json.loads(xsd_summary.read_text(encoding="utf-8"))
            placeholder_repository_owner["schemas"][0]["source"]["repository"] = (
                "https://github.com/example/iso20022-fixtures"
            )
            blocker_cases.append(
                (
                    placeholder_repository_owner,
                    "xsd.schema_source_repository_invalid",
                )
            )

            placeholder_repository_name = json.loads(xsd_summary.read_text(encoding="utf-8"))
            placeholder_repository_name["schemas"][0]["source"]["repository"] = (
                "https://github.com/moov-io/iso20022-template-fixtures"
            )
            blocker_cases.append(
                (
                    placeholder_repository_name,
                    "xsd.schema_source_repository_invalid",
                )
            )

            placeholder_repository_name_separated = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            placeholder_repository_name_separated["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io/iso20022-replace_before_production-fixtures"
            blocker_cases.append(
                (
                    placeholder_repository_name_separated,
                    "xsd.schema_source_repository_invalid",
                )
            )

            placeholder_repository_name_collapsed = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            placeholder_repository_name_collapsed["schemas"][0]["source"][
                "repository"
            ] = "https://github.com/moov-io/operatorcanarybank"
            blocker_cases.append(
                (
                    placeholder_repository_name_collapsed,
                    "xsd.schema_source_repository_invalid",
                )
            )

            long_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            long_repository["schemas"][0]["source"]["repository"] = (
                "https://github.com/example/"
                + ("a" * READINESS.MAX_SOURCE_REPOSITORY_CHARS)
            )
            blocker_cases.append((long_repository, "xsd.schema_source_repository_invalid"))

            bad_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_commit["schemas"][0]["source"]["commit"] = (
                "0123456789abcdef0123456789abcdef0123456Z"
            )
            blocker_cases.append((bad_commit, "xsd.schema_source_commit_invalid"))

            all_zero_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            all_zero_commit["schemas"][0]["source"]["commit"] = "0" * 40
            blocker_cases.append((all_zero_commit, "xsd.schema_source_commit_invalid"))

            bad_filename = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_filename["schemas"][0]["source"]["path"] = "xsd/other.001.001.01.xsd"
            blocker_cases.append((bad_filename, "xsd.schema_source_path_mismatch"))

            bad_license = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_license["schemas"][0]["source"]["license"] = "NOASSERTION"
            blocker_cases.append((bad_license, "xsd.schema_source_license_invalid"))

            bad_digest = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_digest["schemas"][0]["source"]["sha256"] = "f" * 64
            blocker_cases.append((bad_digest, "xsd.schema_source_digest_mismatch"))

            for offset, (body, code) in enumerate(blocker_cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_malformed_missing_profile_message_aggregate_rejects_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            missing_list = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_list.pop("missing_profile_schema_message_ids")
            cases.append(
                (
                    "missing-list",
                    missing_list,
                    "missing_profile_schema_message_ids must be a JSON array",
                    None,
                )
            )

            object_list = json.loads(xsd_summary.read_text(encoding="utf-8"))
            object_list["missing_profile_schema_message_ids"] = {}
            cases.append(
                (
                    "object-list",
                    object_list,
                    "missing_profile_schema_message_ids must be a JSON array",
                    None,
                )
            )

            secret_id = json.loads(xsd_summary.read_text(encoding="utf-8"))
            secret_id["missing_profile_schema_message_ids"].append(
                {
                    "message_def_id": "token=readiness-profile-message-secret",
                    "profile_version_count": 1,
                    "reviewed_missing_schema_fixture": False,
                    "reviewed_schema_only": False,
                    "blocked_source": False,
                    "pending_source": False,
                }
            )
            cases.append(
                (
                    "secret-id",
                    secret_id,
                    "message_def_id contains secret-looking material",
                    "readiness-profile-message-secret",
                )
            )

            nonpositive_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            nonpositive_count["missing_profile_schema_message_ids"].append(
                {
                    "message_def_id": "fooo.001.001.02",
                    "profile_version_count": 0,
                    "reviewed_missing_schema_fixture": False,
                    "reviewed_schema_only": False,
                    "blocked_source": False,
                    "pending_source": False,
                }
            )
            cases.append(
                (
                    "nonpositive-count",
                    nonpositive_count,
                    "profile_version_count must be a positive integer",
                    None,
                )
            )

            nonboolean_flag = json.loads(xsd_summary.read_text(encoding="utf-8"))
            nonboolean_flag["missing_profile_schema_message_ids"].append(
                {
                    "message_def_id": "fooo.001.001.02",
                    "profile_version_count": 1,
                    "reviewed_missing_schema_fixture": "false",
                    "reviewed_schema_only": False,
                    "blocked_source": False,
                    "pending_source": False,
                }
            )
            cases.append(
                (
                    "nonboolean-flag",
                    nonboolean_flag,
                    "reviewed_missing_schema_fixture must be a boolean",
                    None,
                )
            )

            missing_unreviewed_count = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            missing_unreviewed_count.pop("unreviewed_profile_schema_message_id_count")
            cases.append(
                (
                    "missing-unreviewed-count",
                    missing_unreviewed_count,
                    (
                        "unreviewed_profile_schema_message_id_count must be a "
                        "non-negative integer"
                    ),
                    None,
                )
            )

            object_unreviewed_list = json.loads(xsd_summary.read_text(encoding="utf-8"))
            object_unreviewed_list["unreviewed_profile_schema_message_ids"] = {}
            cases.append(
                (
                    "object-unreviewed-list",
                    object_unreviewed_list,
                    "unreviewed_profile_schema_message_ids must be a JSON array",
                    None,
                )
            )

            secret_unreviewed_id = json.loads(xsd_summary.read_text(encoding="utf-8"))
            secret_unreviewed_id["unreviewed_profile_schema_message_ids"].append(
                {
                    "message_def_id": "token=readiness-unreviewed-profile-secret",
                    "profile_version_count": 1,
                }
            )
            cases.append(
                (
                    "secret-unreviewed-id",
                    secret_unreviewed_id,
                    "message_def_id contains secret-looking material",
                    "readiness-unreviewed-profile-secret",
                )
            )

            nonpositive_unreviewed_count = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            nonpositive_unreviewed_count[
                "unreviewed_profile_schema_message_ids"
            ].append(
                {
                    "message_def_id": "fooo.001.001.02",
                    "profile_version_count": 0,
                }
            )
            cases.append(
                (
                    "nonpositive-unreviewed-count",
                    nonpositive_unreviewed_count,
                    "profile_version_count must be a positive integer",
                    None,
                )
            )

            for name, body, message, secret in cases:
                with self.subTest(name=name):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-profile-message-aggregate-{name}.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if secret is not None:
                        self.assertNotIn("token=", stderr)
                        self.assertNotIn(secret, stderr)

    def test_forged_missing_profile_message_aggregate_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            manifest_path = xsd_test.write_minimal_tree(
                root / "xsd",
                xsd_test.minimal_manifest(),
            )
            profile_catalog = xsd_test.write_profile_catalog(
                root / "xsd" / "profiles.rs",
                catalog=[
                    {
                        "id": "minimal-profile",
                        "message_profiles": [
                            {
                                "message_type": "fooo.001",
                                "direction": "inbound",
                                "versions": ["fooo.001.001.01", "fooo.001.001.02"],
                            },
                            {
                                "message_type": "barr.001",
                                "direction": "inbound",
                                "versions": ["barr.001.001.01"],
                            },
                        ],
                    }
                ],
            )
            xsd_summary = root / "xsd" / "missing-profile-message.summary.json"
            rc, _stdout, stderr = xsd_test.run_verify(
                [
                    "--manifest",
                    str(manifest_path),
                    "--profile-catalog",
                    str(profile_catalog),
                    "--summary-out",
                    str(xsd_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            expected_unreviewed_unique = [
                {
                    "message_def_id": "barr.001.001.01",
                    "profile_version_count": 1,
                },
                {
                    "message_def_id": "fooo.001.001.02",
                    "profile_version_count": 1,
                },
            ]
            xsd_body = json.loads(xsd_summary.read_text(encoding="utf-8"))
            self.assertEqual(
                xsd_body["unreviewed_profile_schema_message_id_count"],
                len(expected_unreviewed_unique),
            )
            self.assertEqual(
                xsd_body["unreviewed_profile_schema_message_ids"],
                expected_unreviewed_unique,
            )
            cases = []

            duplicate = json.loads(xsd_summary.read_text(encoding="utf-8"))
            duplicate["missing_profile_schema_message_ids"].append(
                dict(duplicate["missing_profile_schema_message_ids"][0])
            )
            cases.append((duplicate, "xsd.missing_profile_schema_message_id_duplicate"))

            reordered = json.loads(xsd_summary.read_text(encoding="utf-8"))
            self.assertGreater(
                len(reordered["missing_profile_schema_message_ids"]),
                1,
            )
            reordered["missing_profile_schema_message_ids"] = list(
                reversed(reordered["missing_profile_schema_message_ids"])
            )
            cases.append((reordered, "xsd.missing_profile_schema_message_ids_order"))

            reordered_missing_versions = json.loads(xsd_summary.read_text(encoding="utf-8"))
            self.assertGreater(
                len(reordered_missing_versions["missing_profile_schema_versions"]),
                1,
            )
            reordered_missing_versions["missing_profile_schema_versions"] = list(
                reversed(reordered_missing_versions["missing_profile_schema_versions"])
            )
            cases.append(
                (
                    reordered_missing_versions,
                    "xsd.missing_profile_schema_versions_order",
                )
            )

            reordered_catalog_missing = json.loads(xsd_summary.read_text(encoding="utf-8"))
            self.assertGreater(
                len(reordered_catalog_missing["profile_catalog"]["missing_schema_versions"]),
                1,
            )
            reordered_catalog_missing["profile_catalog"]["missing_schema_versions"] = list(
                reversed(
                    reordered_catalog_missing["profile_catalog"][
                        "missing_schema_versions"
                    ]
                )
            )
            cases.append(
                (
                    reordered_catalog_missing,
                    "xsd.profile_catalog_missing_schema_versions_order",
                )
            )

            wrong_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            wrong_count["missing_profile_schema_message_ids"][0][
                "profile_version_count"
            ] += 1
            cases.append((wrong_count, "xsd.missing_profile_schema_message_ids_mismatch"))

            forged_reviewed_flag = json.loads(xsd_summary.read_text(encoding="utf-8"))
            forged_reviewed_flag["missing_profile_schema_message_ids"][0][
                "reviewed_missing_schema_fixture"
            ] = True
            cases.append(
                (
                    forged_reviewed_flag,
                    "xsd.missing_profile_schema_message_ids_mismatch",
                )
            )

            unreviewed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            unreviewed_count["unreviewed_profile_schema_message_id_count"] += 1
            cases.append(
                (
                    unreviewed_count,
                    "xsd.unreviewed_profile_schema_message_id_count_mismatch",
                )
            )

            unreviewed_reordered = json.loads(xsd_summary.read_text(encoding="utf-8"))
            self.assertGreater(
                len(unreviewed_reordered["unreviewed_profile_schema_message_ids"]),
                1,
            )
            unreviewed_reordered["unreviewed_profile_schema_message_ids"] = list(
                reversed(unreviewed_reordered["unreviewed_profile_schema_message_ids"])
            )
            cases.append(
                (
                    unreviewed_reordered,
                    "xsd.unreviewed_profile_schema_message_ids_order",
                )
            )

            unreviewed_wrong_count = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            unreviewed_wrong_count["unreviewed_profile_schema_message_ids"][0][
                "profile_version_count"
            ] += 1
            cases.append(
                (
                    unreviewed_wrong_count,
                    "xsd.unreviewed_profile_schema_message_ids_mismatch",
                )
            )

            unreviewed_duplicate = json.loads(xsd_summary.read_text(encoding="utf-8"))
            unreviewed_duplicate["unreviewed_profile_schema_message_ids"].append(
                dict(unreviewed_duplicate["unreviewed_profile_schema_message_ids"][0])
            )
            unreviewed_duplicate["unreviewed_profile_schema_message_id_count"] = len(
                unreviewed_duplicate["unreviewed_profile_schema_message_ids"]
            )
            cases.append(
                (
                    unreviewed_duplicate,
                    "xsd.unreviewed_profile_schema_message_id_duplicate",
                )
            )

            for offset, (body, code) in enumerate(cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-profile-message-aggregate-{offset}.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)
                    unreviewed_unique = [
                        blocker
                        for blocker in json.loads(stdout)["blockers"]
                        if blocker["code"] == "xsd.unreviewed_profile_schema_message_ids"
                    ]
                    self.assertEqual(len(unreviewed_unique), 1)
                    self.assertEqual(
                        unreviewed_unique[0]["entries"],
                        expected_unreviewed_unique,
                    )

    def test_xsd_source_paths_reject_non_ascii_and_overlong_without_echo(self):
        def attach_blocked_sources(body, entries):
            body["blocked_schema_sources"] = entries
            body["blocked_schema_source_count"] = len(entries)
            return body

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden_unicode = "unicod\u0435-readiness-source-path"
            hidden_long = "x" * (READINESS.MAX_SOURCE_PATH_CHARS + 1)
            cases = (
                (
                    lambda body: body["schemas"][0]["source"].__setitem__(
                        "path",
                        f"xsd/{hidden_unicode}/fooo.001.001.01.xsd",
                    ),
                    "source.path must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: body["schemas"][0]["source"].__setitem__(
                        "path",
                        "xsd/" + hidden_long + "/fooo.001.001.01.xsd",
                    ),
                    "source.path must be no longer than 2048 characters",
                    hidden_long,
                ),
                (
                    lambda body: (
                        attach_blocked_sources(body, [xsd_test.blocked_schema_source()]),
                        body["blocked_schema_sources"][0]["source"].__setitem__(
                            "path",
                            f"xsd/{hidden_unicode}/barr.001.001.01.xsd",
                        ),
                    ),
                    "source.path must use printable ASCII",
                    hidden_unicode,
                ),
                (
                    lambda body: (
                        attach_blocked_sources(body, [xsd_test.blocked_schema_source()]),
                        body["blocked_schema_sources"][0]["source"].__setitem__(
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
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(body)
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-source-path-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_forged_xsd_blocked_schema_source_metadata_blocks_readiness(self):
        def attach_blocked_sources(body, entries):
            body["blocked_schema_sources"] = entries
            body["blocked_schema_source_count"] = len(entries)
            return body

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            malformed_cases = []
            missing_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(missing_source, [xsd_test.blocked_schema_source()])
            missing_source["blocked_schema_sources"][0].pop("source")
            malformed_cases.append((missing_source, "source must be recorded", None))

            null_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(null_source, [xsd_test.blocked_schema_source()])
            null_source["blocked_schema_sources"][0]["source"] = None
            malformed_cases.append((null_source, "source must be a JSON object", None))

            unknown_key = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(unknown_key, [xsd_test.blocked_schema_source()])
            unknown_key["blocked_schema_sources"][0]["unexpected"] = "value"
            malformed_cases.append((unknown_key, "unknown keys", None))

            unsupported_marker = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(unsupported_marker, [xsd_test.blocked_schema_source()])
            unsupported_marker["blocked_schema_sources"][0]["restriction_markers"] = [
                "unreviewed-marker"
            ]
            malformed_cases.append((unsupported_marker, "must be one of", None))

            copyright_only_marker = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                copyright_only_marker,
                [xsd_test.blocked_schema_source()],
            )
            copyright_only_marker["blocked_schema_sources"][0]["restriction_markers"] = [
                "swift-copyright-header"
            ]
            malformed_cases.append(
                (
                    copyright_only_marker,
                    "restriction_markers must include a redistribution restriction marker",
                    None,
                )
            )

            secret_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(secret_reason, [xsd_test.blocked_schema_source()])
            secret_reason["blocked_schema_sources"][0]["reason"] = (
                "Blocked token=readiness-blocked-source-secret"
            )
            malformed_cases.append(
                (
                    secret_reason,
                    "secret-looking material",
                    "readiness-blocked-source-secret",
                )
            )

            secret_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(secret_repository, [xsd_test.blocked_schema_source()])
            secret_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/token-readiness-blocked-secret"
            malformed_cases.append(
                (
                    secret_repository,
                    "source.repository contains secret-looking material",
                    "token-readiness-blocked-secret",
                )
            )

            for offset, (body, message, secret) in enumerate(malformed_cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-blocked-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if secret is not None:
                        self.assertNotIn("token=", stderr)
                        self.assertNotIn(secret, stderr)

            blocker_cases = []
            without_gap = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(without_gap, [xsd_test.blocked_schema_source()])
            blocker_cases.append((without_gap, "xsd.blocked_source_without_gap"))

            count_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(count_mismatch, [xsd_test.blocked_schema_source()])
            count_mismatch["blocked_schema_source_count"] = 0
            blocker_cases.append(
                (count_mismatch, "xsd.blocked_schema_source_count_mismatch")
            )

            bad_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(bad_repository, [xsd_test.blocked_schema_source()])
            bad_repository["blocked_schema_sources"][0]["source"]["repository"] += ".git"
            blocker_cases.append((bad_repository, "xsd.blocked_source_repository_invalid"))

            uppercase_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                uppercase_repository,
                [xsd_test.blocked_schema_source()],
            )
            uppercase_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/Prog-Nov/iso20022-messages-for-go"
            blocker_cases.append(
                (
                    uppercase_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            underscore_owner_repository = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                underscore_owner_repository,
                [xsd_test.blocked_schema_source()],
            )
            underscore_owner_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog_nov/iso20022-messages-for-go"
            blocker_cases.append(
                (
                    underscore_owner_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            leading_hyphen_owner_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                leading_hyphen_owner_repository,
                [xsd_test.blocked_schema_source()],
            )
            leading_hyphen_owner_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/-prog-nov/iso20022-messages-for-go"
            blocker_cases.append(
                (
                    leading_hyphen_owner_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            trailing_hyphen_owner_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                trailing_hyphen_owner_repository,
                [xsd_test.blocked_schema_source()],
            )
            trailing_hyphen_owner_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov-/iso20022-messages-for-go"
            blocker_cases.append(
                (
                    trailing_hyphen_owner_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            punctuation_only_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                punctuation_only_name_repository,
                [xsd_test.blocked_schema_source()],
            )
            punctuation_only_name_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/___"
            blocker_cases.append(
                (
                    punctuation_only_name_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            leading_punctuation_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                leading_punctuation_name_repository,
                [xsd_test.blocked_schema_source()],
            )
            leading_punctuation_name_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/-iso20022-messages-for-go"
            blocker_cases.append(
                (
                    leading_punctuation_name_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            trailing_punctuation_name_repository = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                trailing_punctuation_name_repository,
                [xsd_test.blocked_schema_source()],
            )
            trailing_punctuation_name_repository["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/iso20022-messages-for-go_"
            blocker_cases.append(
                (
                    trailing_punctuation_name_repository,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            placeholder_repository_owner = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                placeholder_repository_owner,
                [xsd_test.blocked_schema_source()],
            )
            placeholder_repository_owner["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/example/iso20022-blocked"
            blocker_cases.append(
                (
                    placeholder_repository_owner,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            placeholder_repository_name = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                placeholder_repository_name,
                [xsd_test.blocked_schema_source()],
            )
            placeholder_repository_name["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/iso20022-sample-blocked"
            blocker_cases.append(
                (
                    placeholder_repository_name,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            placeholder_repository_name_separated = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                placeholder_repository_name_separated,
                [xsd_test.blocked_schema_source()],
            )
            placeholder_repository_name_separated["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/iso20022-replace_before_production-blocked"
            blocker_cases.append(
                (
                    placeholder_repository_name_separated,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            placeholder_repository_name_collapsed = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_blocked_sources(
                placeholder_repository_name_collapsed,
                [xsd_test.blocked_schema_source()],
            )
            placeholder_repository_name_collapsed["blocked_schema_sources"][0]["source"][
                "repository"
            ] = "https://github.com/prog-nov/operatorcanarybank"
            blocker_cases.append(
                (
                    placeholder_repository_name_collapsed,
                    "xsd.blocked_source_repository_invalid",
                )
            )

            bad_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(bad_commit, [xsd_test.blocked_schema_source()])
            bad_commit["blocked_schema_sources"][0]["source"]["commit"] = (
                "89abcdef0123456789abcdef0123456789abcdeZ"
            )
            blocker_cases.append((bad_commit, "xsd.blocked_source_commit_invalid"))

            all_zero_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(all_zero_commit, [xsd_test.blocked_schema_source()])
            all_zero_commit["blocked_schema_sources"][0]["source"]["commit"] = "0" * 40
            blocker_cases.append((all_zero_commit, "xsd.blocked_source_commit_invalid"))

            bad_filename = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(bad_filename, [xsd_test.blocked_schema_source()])
            bad_filename["blocked_schema_sources"][0]["source"]["path"] = (
                "xsd/other.001.001.01.xsd"
            )
            blocker_cases.append((bad_filename, "xsd.blocked_source_path_mismatch"))

            already_checked_in = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                already_checked_in,
                [xsd_test.blocked_schema_source("fooo.001.001.01")],
            )
            blocker_cases.append(
                (already_checked_in, "xsd.blocked_source_already_checked_in")
            )

            duplicate_digest = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                duplicate_digest,
                [
                    xsd_test.blocked_schema_source("barr.001.001.01"),
                    xsd_test.blocked_schema_source("barr.001.001.02"),
                ],
            )
            duplicate_digest["blocked_schema_sources"][1]["source"]["path"] = (
                "xsd/barr.001.001.02.xsd"
            )
            blocker_cases.append(
                (duplicate_digest, "xsd.blocked_source_digest_duplicate")
            )

            duplicate_message_id = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                duplicate_message_id,
                [
                    xsd_test.blocked_schema_source("barr.001.001.01"),
                    xsd_test.blocked_schema_source("barr.001.001.01"),
                ],
            )
            duplicate_message_id["blocked_schema_sources"][1]["source"][
                "repository"
            ] = "https://github.com/moov-io/iso20022"
            duplicate_message_id["blocked_schema_sources"][1]["source"][
                "path"
            ] = "alternate/barr.001.001.01.xsd"
            duplicate_message_id["blocked_schema_sources"][1]["source"][
                "sha256"
            ] = "2" * 64
            blocker_cases.append(
                (duplicate_message_id, "xsd.blocked_source_message_id_duplicate")
            )

            digest_reuses_checked_schema = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                digest_reuses_checked_schema,
                [xsd_test.blocked_schema_source()],
            )
            digest_reuses_checked_schema["blocked_schema_sources"][0]["source"][
                "sha256"
            ] = digest_reuses_checked_schema["schemas"][0]["sha256"]
            blocker_cases.append(
                (
                    digest_reuses_checked_schema,
                    "xsd.blocked_source_digest_matches_checked_schema",
                )
            )

            digest_reuses_checked_fixture = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(
                digest_reuses_checked_fixture,
                [xsd_test.blocked_schema_source()],
            )
            digest_reuses_checked_fixture["blocked_schema_sources"][0]["source"][
                "sha256"
            ] = digest_reuses_checked_fixture["fixtures"][0]["sha256"]
            blocker_cases.append(
                (
                    digest_reuses_checked_fixture,
                    "xsd.blocked_source_digest_matches_fixture",
                )
            )

            redacted_blocker_ids = {
                "xsd.blocked_source_without_gap": "barr.001.001.01",
                "xsd.blocked_source_already_checked_in": "fooo.001.001.01",
            }
            for offset, (body, code) in enumerate(blocker_cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-blocked-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(code, codes)
                    if code in redacted_blocker_ids:
                        messages = [
                            blocker["message"]
                            for blocker in blockers
                            if blocker["code"] == code
                        ]
                        self.assertTrue(messages)
                        self.assertNotIn(redacted_blocker_ids[code], "\n".join(messages))

    def test_forged_xsd_pending_schema_source_metadata_rejects_readiness(self):
        def attach_pending_sources(body, entries):
            body["pending_schema_sources"] = entries
            body["pending_schema_source_count"] = len(entries)
            return body

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            malformed_cases = []
            missing_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(missing_source, [xsd_test.pending_schema_source()])
            missing_source["pending_schema_sources"][0].pop("source")
            malformed_cases.append((missing_source, "source must be recorded", None))

            null_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(null_source, [xsd_test.pending_schema_source()])
            null_source["pending_schema_sources"][0]["source"] = None
            malformed_cases.append((null_source, "source must be a JSON object", None))

            unknown_key = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(unknown_key, [xsd_test.pending_schema_source()])
            unknown_key["pending_schema_sources"][0]["unexpected"] = "value"
            malformed_cases.append((unknown_key, "unknown keys", None))

            bad_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_url, [xsd_test.pending_schema_source()])
            bad_url["pending_schema_sources"][0]["source"][
                "catalogue_url"
            ] = "https://example.com/iso-20022-message-definitions"
            malformed_cases.append(
                (
                    bad_url,
                    "source.catalogue_url must be an official ISO 20022 catalogue URL",
                    None,
                )
            )

            bad_archive_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_archive_url, [xsd_test.pending_schema_source()])
            bad_archive_url["pending_schema_sources"][0]["source"][
                "catalogue_url"
            ] = "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=x"
            malformed_cases.append(
                (
                    bad_archive_url,
                    "source.catalogue_url archive URL must set one numeric page",
                    None,
                )
            )

            encoded_archive_page = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                encoded_archive_page, [xsd_test.pending_schema_source()]
            )
            encoded_archive_page["pending_schema_sources"][0]["source"][
                "catalogue_url"
            ] = "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=%38"
            malformed_cases.append(
                (
                    encoded_archive_page,
                    "source.catalogue_url must not contain percent escapes",
                    None,
                )
            )

            trailing_archive_query = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_pending_sources(
                trailing_archive_query, [xsd_test.pending_schema_source()]
            )
            trailing_archive_query["pending_schema_sources"][0]["source"][
                "catalogue_url"
            ] = "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=8&"
            malformed_cases.append(
                (
                    trailing_archive_query,
                    "source.catalogue_url archive URL must set one numeric page",
                    None,
                )
            )

            leading_zero_archive_page = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_pending_sources(
                leading_zero_archive_page, [xsd_test.pending_schema_source()]
            )
            leading_zero_archive_page["pending_schema_sources"][0]["source"][
                "catalogue_url"
            ] = "https://www.iso20022.org/catalogue-messages/iso-20022-messages-archive?page=08"
            malformed_cases.append(
                (
                    leading_zero_archive_page,
                    "source.catalogue_url archive URL must set one numeric page",
                    None,
                )
            )

            missing_download_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                missing_download_url, [xsd_test.pending_schema_source()]
            )
            missing_download_url["pending_schema_sources"][0]["source"].pop(
                "download_url"
            )
            malformed_cases.append(
                (
                    missing_download_url,
                    "source.download_url must be a non-empty string",
                    None,
                )
            )

            bad_download_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_download_url, [xsd_test.pending_schema_source()])
            bad_download_url["pending_schema_sources"][0]["source"][
                "download_url"
            ] = "https://example.com/message/12345/download"
            malformed_cases.append(
                (
                    bad_download_url,
                    "source.download_url must be an official ISO 20022 XSD download URL",
                    None,
                )
            )

            encoded_download_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                encoded_download_url, [xsd_test.pending_schema_source()]
            )
            encoded_download_url["pending_schema_sources"][0]["source"][
                "download_url"
            ] = "https://www.iso20022.org/message/%31345/download"
            malformed_cases.append(
                (
                    encoded_download_url,
                    "source.download_url must not contain percent escapes",
                    None,
                )
            )

            secret_download_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                secret_download_url, [xsd_test.pending_schema_source()]
            )
            secret_download_url["pending_schema_sources"][0]["source"][
                "download_url"
            ] = "https://www.iso20022.org/message/12345/download?token=readiness-pending-download-secret"
            malformed_cases.append(
                (
                    secret_download_url,
                    "secret-looking material",
                    "readiness-pending-download-secret",
                )
            )

            bad_download_type = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_download_type, [xsd_test.pending_schema_source()])
            bad_download_type["pending_schema_sources"][0]["source"][
                "download_type"
            ] = "PDF"
            malformed_cases.append(
                (
                    bad_download_type,
                    "source.download_type must be one of XSD",
                    None,
                )
            )

            malformed_message_name = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                malformed_message_name, [xsd_test.pending_schema_source()]
            )
            malformed_message_name["pending_schema_sources"][0]["source"][
                "message_name"
            ] = "Bar PayloadV01"
            malformed_cases.append(
                (
                    malformed_message_name,
                    "source.message_name must be a canonical ISO message name ending in VNN",
                    None,
                )
            )

            bad_message_name_version = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_pending_sources(
                bad_message_name_version, [xsd_test.pending_schema_source()]
            )
            bad_message_name_version["pending_schema_sources"][0]["source"][
                "message_name"
            ] = "BarPayloadV02"
            malformed_cases.append(
                (
                    bad_message_name_version,
                    "source.message_name version suffix must match message_def_id version",
                    None,
                )
            )

            bad_submitter_semicolon = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                bad_submitter_semicolon, [xsd_test.pending_schema_source()]
            )
            bad_submitter_semicolon["pending_schema_sources"][0]["source"][
                "submitting_organisation"
            ] = "SWIFT; FPL"
            malformed_cases.append(
                (
                    bad_submitter_semicolon,
                    "source.submitting_organisation must not contain semicolon path parameters",
                    None,
                )
            )

            bad_submitter_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_submitter_url, [xsd_test.pending_schema_source()])
            bad_submitter_url["pending_schema_sources"][0]["source"][
                "submitting_organisation"
            ] = "https://www.iso20022.org/SWIFT"
            malformed_cases.append(
                (
                    bad_submitter_url,
                    "source.submitting_organisation must not contain URI or contact delimiters",
                    None,
                )
            )

            bad_submitter_slash = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_submitter_slash, [xsd_test.pending_schema_source()])
            bad_submitter_slash["pending_schema_sources"][0]["source"][
                "submitting_organisation"
            ] = "SWIFT//FPL"
            malformed_cases.append(
                (
                    bad_submitter_slash,
                    "source.submitting_organisation must use slash only inside organization tokens",
                    None,
                )
            )

            bad_submitter_list = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(bad_submitter_list, [xsd_test.pending_schema_source()])
            bad_submitter_list["pending_schema_sources"][0]["source"][
                "submitting_organisation"
            ] = "SWIFT,"
            malformed_cases.append(
                (
                    bad_submitter_list,
                    "source.submitting_organisation must be a comma-space separated list of organization names",
                    None,
                )
            )

            bad_submitter_placeholder = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            attach_pending_sources(
                bad_submitter_placeholder, [xsd_test.pending_schema_source()]
            )
            bad_submitter_placeholder["pending_schema_sources"][0]["source"][
                "submitting_organisation"
            ] = "Example Org"
            malformed_cases.append(
                (
                    bad_submitter_placeholder,
                    "source.submitting_organisation must not use placeholder organization metadata",
                    None,
                )
            )

            secret_reason = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(secret_reason, [xsd_test.pending_schema_source()])
            secret_reason["pending_schema_sources"][0]["reason"] = (
                "Pending token=readiness-pending-source-secret"
            )
            malformed_cases.append(
                (
                    secret_reason,
                    "secret-looking material",
                    "readiness-pending-source-secret",
                )
            )

            for offset, (body, message, secret) in enumerate(malformed_cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-xsd-pending-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if secret is not None:
                        self.assertNotIn("token=", stderr)
                        self.assertNotIn(secret, stderr)

            blocker_cases = []
            without_gap = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(without_gap, [xsd_test.pending_schema_source()])
            blocker_cases.append((without_gap, "xsd.pending_source_without_gap"))

            count_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(count_mismatch, [xsd_test.pending_schema_source()])
            count_mismatch["pending_schema_source_count"] = 0
            blocker_cases.append(
                (count_mismatch, "xsd.pending_schema_source_count_mismatch")
            )

            already_checked_in = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                already_checked_in,
                [xsd_test.pending_schema_source("fooo.001.001.01")],
            )
            blocker_cases.append(
                (already_checked_in, "xsd.pending_source_already_checked_in")
            )

            already_blocked = json.loads(xsd_summary.read_text(encoding="utf-8"))
            already_blocked["blocked_schema_sources"] = [
                xsd_test.blocked_schema_source()
            ]
            already_blocked["blocked_schema_source_count"] = 1
            attach_pending_sources(already_blocked, [xsd_test.pending_schema_source()])
            blocker_cases.append(
                (already_blocked, "xsd.pending_source_already_blocked")
            )

            duplicate_message_id = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                duplicate_message_id,
                [
                    xsd_test.pending_schema_source("barr.001.001.01"),
                    xsd_test.pending_schema_source("barr.001.001.01"),
                ],
            )
            duplicate_message_id["pending_schema_sources"][1]["source"][
                "message_name"
            ] = "AlternateBarPayloadV01"
            blocker_cases.append(
                (duplicate_message_id, "xsd.pending_source_message_id_duplicate")
            )

            duplicate_source = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                duplicate_source,
                [
                    xsd_test.pending_schema_source("barr.001.001.01"),
                    xsd_test.pending_schema_source("bazz.001.001.01"),
                ],
            )
            blocker_cases.append((duplicate_source, "xsd.pending_source_duplicate"))

            duplicate_message_name = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                duplicate_message_name,
                [
                    xsd_test.pending_schema_source("barr.001.001.01"),
                    xsd_test.pending_schema_source("bazz.001.001.01"),
                ],
            )
            duplicate_message_name["pending_schema_sources"][1]["source"][
                "download_url"
            ] = "https://www.iso20022.org/message/12346/download"
            blocker_cases.append(
                (duplicate_message_name, "xsd.pending_source_message_name_duplicate")
            )

            duplicate_download_url = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_pending_sources(
                duplicate_download_url,
                [
                    xsd_test.pending_schema_source("barr.001.001.01"),
                    xsd_test.pending_schema_source("barr.001.001.02"),
                ],
            )
            duplicate_download_url["pending_schema_sources"][1]["source"][
                "message_name"
            ] = "DifferentBarPayloadV02"
            blocker_cases.append(
                (
                    duplicate_download_url,
                    "xsd.pending_source_download_url_duplicate",
                )
            )

            for offset, (body, code) in enumerate(blocker_cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-pending-source-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_xsd_profile_catalog_must_be_recorded(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            omitted = json.loads(xsd_summary.read_text(encoding="utf-8"))
            omitted.pop("profile_catalog")
            refresh_digest(omitted)
            omitted_path = write_json(
                root / "omitted-xsd-profile-catalog.summary.json",
                omitted,
            )

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(omitted_path),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("profile_catalog must be recorded", stderr)

            explicit_null = json.loads(xsd_summary.read_text(encoding="utf-8"))
            explicit_null["profile_catalog"] = None
            explicit_null["profile_checked_versions"] = 0
            explicit_null["profile_schema_backed_versions"] = 0
            explicit_null["missing_profile_schema_versions"] = []
            refresh_digest(explicit_null)
            null_path = write_json(
                root / "null-xsd-profile-catalog.summary.json",
                explicit_null,
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(null_path),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("xsd.profile_catalog_empty", codes)

    def test_xsd_manifest_path_must_be_recorded_and_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "omitted",
                    lambda body: body.pop("manifest"),
                    "manifest must be a non-empty string",
                ),
                (
                    "parent-segment",
                    lambda body: body.__setitem__("manifest", "../fixture_manifest.json"),
                    "manifest must not contain dot or parent segments",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(xsd)
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"malformed-xsd-manifest-{name}.summary.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_repository_xsd_fixture_manifest_paths_remain_blockers(self):
        checked_in_manifest = REPO_ROOT / "fixtures" / "iso20022" / "xsd" / "fixture_manifest.json"
        cases = (
            "fixtures/iso20022/xsd/fixture_manifest.json",
            str(checked_in_manifest),
            "/ops/release/fixtures/iso20022/xsd/fixture_manifest.json",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, manifest in enumerate(cases):
                with self.subTest(manifest=manifest):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    xsd["manifest"] = manifest
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"repository-fixture-manifest-{offset}.summary.json",
                        xsd,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertEqual(codes, {"xsd.repository_fixture_manifest"})

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                            "--allow-reviewed-xsd-gaps",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(
                        "requires at least one reviewed XSD gap warning",
                        stderr,
                    )
                    self.assertNotIn(str(manifest), stderr)

    def test_repository_xsd_profile_catalog_paths_block_readiness(self):
        checked_in_catalog = REPO_ROOT / "fixtures" / "iso20022" / "profiles.rs"
        cases = (
            "fixtures/iso20022/profiles.rs",
            str(checked_in_catalog),
            "/ops/release/fixtures/iso20022/profiles.rs",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, catalog_path in enumerate(cases):
                with self.subTest(catalog_path=catalog_path):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    xsd["profile_catalog"]["path"] = catalog_path
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"repository-fixture-profile-catalog-{offset}.summary.json",
                        xsd,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertEqual(codes, {"xsd.repository_profile_catalog"})

    def test_archived_path_strings_reject_overlong_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden = "x" * (READINESS.MAX_SOURCE_PATH_CHARS + 1)
            cases = (
                (
                    "manifest",
                    "xsd",
                    lambda body: body.__setitem__("manifest", f"/ops/iso/{hidden}.json"),
                    "manifest must be no longer than 2048 characters",
                ),
                (
                    "profile-catalog",
                    "xsd",
                    lambda body: body["profile_catalog"].__setitem__(
                        "path",
                        f"/ops/iso/{hidden}.rs",
                    ),
                    "profile_catalog.path must be no longer than 2048 characters",
                ),
                (
                    "canary-summary",
                    "evidence",
                    lambda body: body["canary_summaries"][0].__setitem__(
                        "path",
                        f"/ops/iso/{hidden}.summary.json",
                    ),
                    "canary_summaries[0].path must be no longer than 2048 characters",
                ),
                (
                    "canary-config",
                    "evidence",
                    lambda body: body["canary_summaries"][0].__setitem__(
                        "config_path",
                        f"/ops/iso/{hidden}.json",
                    ),
                    "canary_summaries[0].config_path must be no longer than 2048 characters",
                ),
            )
            for offset, (name, target, mutate, message) in enumerate(cases):
                with self.subTest(name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    body = xsd if target == "xsd" else evidence
                    mutate(body)
                    if target == "xsd":
                        refresh_digest(xsd)
                        mutated_xsd = write_json(
                            root / f"overlong-xsd-path-{offset}.summary.json",
                            xsd,
                        )
                        mutated_evidence = evidence_summary
                    else:
                        refresh_digest(evidence)
                        mutated_xsd = xsd_summary
                        mutated_evidence = write_json(
                            root / f"overlong-evidence-path-{offset}.summary.json",
                            evidence,
                        )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_xsd),
                            "--evidence-summary",
                            str(mutated_evidence),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_forged_xsd_profile_catalog_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

            def attach_blocked_source(body):
                body["blocked_schema_sources"] = [xsd_test.blocked_schema_source()]
                body["blocked_schema_source_count"] = 1
                return body["blocked_schema_sources"][0]["source"]["sha256"]

            version_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            version_count["profile_checked_versions"] += 1
            cases.append((version_count, "xsd.profile_version_count_mismatch"))

            duplicate_version = json.loads(xsd_summary.read_text(encoding="utf-8"))
            duplicate_version["profile_catalog"]["versions"].append(
                dict(duplicate_version["profile_catalog"]["versions"][0])
            )
            duplicate_version["profile_checked_versions"] += 1
            duplicate_version["profile_schema_backed_versions"] += 1
            cases.append((duplicate_version, "xsd.profile_version_duplicate"))

            schema_backed_without_fixture = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            schema_backed_without_fixture["profile_catalog"]["versions"][0][
                "message_def_id"
            ] = "fooo.001.001.02"
            cases.append(
                (
                    schema_backed_without_fixture,
                    "xsd.profile_version_schema_backing_mismatch",
                )
            )

            fixture_backed_marked_missing = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            missing_version = dict(
                fixture_backed_marked_missing["profile_catalog"]["versions"][0]
            )
            missing_version.pop("schema_backed")
            fixture_backed_marked_missing["profile_catalog"]["versions"][0][
                "schema_backed"
            ] = False
            fixture_backed_marked_missing["profile_schema_backed_versions"] = 0
            fixture_backed_marked_missing["profile_catalog"]["schema_backed_versions"] = 0
            fixture_backed_marked_missing["missing_profile_schema_versions"].append(
                missing_version
            )
            fixture_backed_marked_missing["profile_catalog"][
                "missing_schema_versions"
            ].append(missing_version)
            cases.append(
                (
                    fixture_backed_marked_missing,
                    "xsd.profile_version_schema_backing_mismatch",
                )
            )

            missing_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_mismatch["missing_profile_schema_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "message_def_id": "fooo.001.001.02",
                }
            )
            cases.append((missing_mismatch, "xsd.missing_profile_schema_versions_mismatch"))

            missing_message_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            missing_message_mismatch["missing_profile_schema_message_ids"].append(
                {
                    "message_def_id": "fooo.001.001.02",
                    "profile_version_count": 1,
                    "reviewed_missing_schema_fixture": False,
                    "reviewed_schema_only": False,
                    "blocked_source": False,
                    "pending_source": False,
                }
            )
            cases.append(
                (
                    missing_message_mismatch,
                    "xsd.missing_profile_schema_message_ids_mismatch",
                )
            )

            catalog_missing_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            catalog_missing_mismatch["profile_catalog"]["missing_schema_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "message_def_id": "fooo.001.001.02",
                }
            )
            cases.append(
                (
                    catalog_missing_mismatch,
                    "xsd.profile_catalog_missing_schema_versions_mismatch",
                )
            )
            profile_catalog_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_count["profile_catalog"]["checked_versions"] += 1
            cases.append((profile_catalog_count, "xsd.profile_catalog_checked_count_mismatch"))

            profile_catalog_profiles = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_profiles["profile_catalog"]["profiles"] += 1
            cases.append(
                (
                    profile_catalog_profiles,
                    "xsd.profile_catalog_profile_count_mismatch",
                )
            )

            profile_catalog_backed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_backed_count["profile_catalog"]["schema_backed_versions"] += 1
            cases.append(
                (
                    profile_catalog_backed_count,
                    "xsd.profile_catalog_schema_backed_count_mismatch",
                )
            )

            manifest_reuses_schema = json.loads(xsd_summary.read_text(encoding="utf-8"))
            manifest_reuses_schema["manifest_sha256"] = (
                manifest_reuses_schema["schemas"][0]["sha256"]
            )
            cases.append((manifest_reuses_schema, "xsd.manifest_digest_matches_schema"))

            manifest_reuses_fixture = json.loads(xsd_summary.read_text(encoding="utf-8"))
            manifest_reuses_fixture["manifest_sha256"] = (
                manifest_reuses_fixture["fixtures"][0]["sha256"]
            )
            cases.append((manifest_reuses_fixture, "xsd.manifest_digest_matches_fixture"))

            manifest_reuses_blocked = json.loads(xsd_summary.read_text(encoding="utf-8"))
            manifest_reuses_blocked["manifest_sha256"] = attach_blocked_source(
                manifest_reuses_blocked
            )
            cases.append(
                (
                    manifest_reuses_blocked,
                    "xsd.manifest_digest_matches_blocked_source",
                )
            )

            manifest_reuses_profile_catalog = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            manifest_reuses_profile_catalog["manifest_sha256"] = (
                manifest_reuses_profile_catalog["profile_catalog"]["sha256"]
            )
            cases.append(
                (
                    manifest_reuses_profile_catalog,
                    "xsd.manifest_digest_matches_profile_catalog",
                )
            )

            manifest_reuses_profile_catalog_json = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            manifest_reuses_profile_catalog_json["manifest_sha256"] = (
                manifest_reuses_profile_catalog_json["profile_catalog"][
                    "catalog_json_sha256"
                ]
            )
            cases.append(
                (
                    manifest_reuses_profile_catalog_json,
                    "xsd.manifest_digest_matches_profile_catalog_json",
                )
            )

            catalog_source_reuses_schema = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_source_reuses_schema["profile_catalog"]["sha256"] = (
                catalog_source_reuses_schema["schemas"][0]["sha256"]
            )
            cases.append(
                (
                    catalog_source_reuses_schema,
                    "xsd.profile_catalog_digest_matches_schema",
                )
            )

            catalog_source_reuses_fixture = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_source_reuses_fixture["profile_catalog"]["sha256"] = (
                catalog_source_reuses_fixture["fixtures"][0]["sha256"]
            )
            cases.append(
                (
                    catalog_source_reuses_fixture,
                    "xsd.profile_catalog_digest_matches_fixture",
                )
            )

            catalog_source_reuses_blocked = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_source_reuses_blocked["profile_catalog"]["sha256"] = (
                attach_blocked_source(catalog_source_reuses_blocked)
            )
            cases.append(
                (
                    catalog_source_reuses_blocked,
                    "xsd.profile_catalog_digest_matches_blocked_source",
                )
            )

            catalog_source_reuses_json = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_source_reuses_json["profile_catalog"]["catalog_json_sha256"] = (
                catalog_source_reuses_json["profile_catalog"]["sha256"]
            )
            cases.append(
                (
                    catalog_source_reuses_json,
                    "xsd.profile_catalog_digest_matches_catalog_json",
                )
            )

            catalog_json_reuses_schema = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_json_reuses_schema["profile_catalog"]["catalog_json_sha256"] = (
                catalog_json_reuses_schema["schemas"][0]["sha256"]
            )
            cases.append(
                (
                    catalog_json_reuses_schema,
                    "xsd.profile_catalog_json_digest_matches_schema",
                )
            )

            catalog_json_reuses_fixture = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_json_reuses_fixture["profile_catalog"]["catalog_json_sha256"] = (
                catalog_json_reuses_fixture["fixtures"][0]["sha256"]
            )
            cases.append(
                (
                    catalog_json_reuses_fixture,
                    "xsd.profile_catalog_json_digest_matches_fixture",
                )
            )

            catalog_json_reuses_blocked = json.loads(
                xsd_summary.read_text(encoding="utf-8")
            )
            catalog_json_reuses_blocked["profile_catalog"]["catalog_json_sha256"] = (
                attach_blocked_source(catalog_json_reuses_blocked)
            )
            cases.append(
                (
                    catalog_json_reuses_blocked,
                    "xsd.profile_catalog_json_digest_matches_blocked_source",
                )
            )

            concrete_skipped = json.loads(xsd_summary.read_text(encoding="utf-8"))
            concrete_skipped["profile_catalog"]["skipped_family_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "version": "fooo.001.001.01",
                }
            )
            cases.append((concrete_skipped, "xsd.profile_catalog_skipped_concrete_version"))

            duplicate_skipped = json.loads(xsd_summary.read_text(encoding="utf-8"))
            duplicate_skipped["profile_catalog"]["skipped_family_versions"].extend(
                [
                    {
                        "profile_id": "minimal-profile",
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "version": "fooo.001",
                    },
                    {
                        "profile_id": "minimal-profile",
                        "message_type": "fooo.001",
                        "direction": "inbound",
                        "version": "fooo.001",
                    },
                ]
            )
            cases.append((duplicate_skipped, "xsd.profile_catalog_skipped_duplicate"))

            mismatched_skipped = json.loads(xsd_summary.read_text(encoding="utf-8"))
            mismatched_skipped["profile_catalog"]["skipped_family_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": "fooo.001",
                    "direction": "inbound",
                    "version": "fooo.002",
                }
            )
            cases.append(
                (
                    mismatched_skipped,
                    "xsd.profile_catalog_skipped_family_mismatch",
                )
            )

            for offset, (body, code) in enumerate(cases):
                with self.subTest(code=code):
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"forged-xsd-profile-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_xsd_material_digests_cannot_reuse_evidence_material_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))

            def set_schema_digest(body, digest):
                body["schemas"][0]["sha256"] = digest
                body["schemas"][0]["source"]["sha256"] = digest

            def set_blocked_source_digest(body, digest):
                body["blocked_schema_sources"] = [xsd_test.blocked_schema_source()]
                body["blocked_schema_sources"][0]["source"]["sha256"] = digest
                body["blocked_schema_source_count"] = 1

            cases = (
                (
                    "summary-trust-bundle",
                    lambda body: None,
                    "xsd.summary_digest_matches_evidence_material",
                    xsd["summary_sha256"],
                    lambda evidence_body: evidence_body["trust_summaries"][0]["profiles"][
                        0
                    ].__setitem__("bundle_sha256", xsd["summary_sha256"]),
                ),
                (
                    "manifest-evidence-summary",
                    lambda body: body.__setitem__(
                        "manifest_sha256",
                        evidence["summary_sha256"],
                    ),
                    "xsd.manifest_digest_matches_evidence_material",
                    evidence["summary_sha256"],
                    None,
                ),
                (
                    "schema-canary-summary",
                    lambda body: set_schema_digest(
                        body,
                        evidence["canary_summaries"][0]["summary_sha256"],
                    ),
                    "xsd.schema_digest_matches_evidence_material",
                    evidence["canary_summaries"][0]["summary_sha256"],
                    None,
                ),
                (
                    "fixture-receipt-material",
                    lambda body: body["fixtures"][0].__setitem__(
                        "sha256",
                        evidence["canary_summaries"][0]["receipt_summary"]["receipts"][
                            1
                        ]["payload_sha256"],
                    ),
                    "xsd.fixture_digest_matches_evidence_material",
                    evidence["canary_summaries"][0]["receipt_summary"]["receipts"][1][
                        "payload_sha256"
                    ],
                    None,
                ),
                (
                    "blocked-source-trust-bundle",
                    lambda body: set_blocked_source_digest(
                        body,
                        evidence["trust_summaries"][0]["profiles"][0]["bundle_sha256"],
                    ),
                    "xsd.blocked_source_digest_matches_evidence_material",
                    evidence["trust_summaries"][0]["profiles"][0]["bundle_sha256"],
                    None,
                ),
                (
                    "profile-catalog-profile-json",
                    lambda body: body["profile_catalog"].__setitem__(
                        "sha256",
                        evidence["trust_summaries"][0]["profile_json_sha256"],
                    ),
                    "xsd.profile_catalog_digest_matches_evidence_material",
                    evidence["trust_summaries"][0]["profile_json_sha256"],
                    None,
                ),
                (
                    "profile-catalog-json-trust-der",
                    lambda body: body["profile_catalog"].__setitem__(
                        "catalog_json_sha256",
                        evidence["trust_summaries"][0]["profiles"][0][
                            "x509_crl_der"
                        ][0]["sha256"],
                    ),
                    "xsd.profile_catalog_json_digest_matches_evidence_material",
                    evidence["trust_summaries"][0]["profiles"][0]["x509_crl_der"][0][
                        "sha256"
                    ],
                    None,
                ),
            )

            for offset, (
                name,
                mutate_xsd,
                expected_code,
                hidden_digest,
                mutate_evidence,
            ) in enumerate(cases):
                with self.subTest(name=name):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    evidence_body = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate_xsd(body)
                    if mutate_evidence is not None:
                        mutate_evidence(evidence_body)
                        refresh_digest(evidence_body)
                        evidence_path = write_json(
                            root / f"xsd-evidence-digest-role-{offset}.evidence.json",
                            evidence_body,
                        )
                    else:
                        evidence_path = evidence_summary
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"xsd-evidence-digest-role-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(expected_code, codes)
                    blocker_text = "\n".join(
                        blocker["message"]
                        for blocker in blockers
                        if blocker["code"] == expected_code
                    )
                    self.assertIn("xsd_summaries[0]", blocker_text)
                    self.assertIn("evidence_summaries[0]", blocker_text)
                    self.assertNotIn(hidden_digest, blocker_text)

    def test_xsd_material_paths_cannot_reuse_evidence_material_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def receipt_by_kind(receipt_summary, kind):
                return next(
                    receipt
                    for receipt in receipt_summary["receipts"]
                    if receipt["receipt_kind"] == kind
                )

            def add_blocked_source(body):
                body["blocked_schema_sources"] = [
                    xsd_test.blocked_schema_source("barr.001.001.01")
                ]
                body["blocked_schema_source_count"] = 1
                return body["blocked_schema_sources"][0]["source"]["path"]

            def set_archive_store_dir(evidence_body, value):
                archive = evidence_body["receipt_verification"]
                receipt_by_kind(archive, "iso-audit-notary")["store_dir"] = value
                refresh_digest(archive)

            def summary_reuses_canary_summary(body, evidence_body, xsd_path, _evidence_path):
                evidence_body["canary_summaries"][0]["path"] = str(xsd_path)

            def manifest_reuses_evidence_summary(body, _evidence_body, _xsd_path, evidence_path):
                body["manifest"] = str(evidence_path)

            def schema_reuses_archive_store_dir(body, evidence_body, _xsd_path, _evidence_path):
                set_archive_store_dir(evidence_body, body["schemas"][0]["path"])

            def fixture_reuses_canary_source(body, evidence_body, _xsd_path, _evidence_path):
                body["fixtures"][0]["path"] = "foo_fixture.xml"
                receipt_summary = evidence_body["canary_summaries"][0]["receipt_summary"]
                receipt_by_kind(receipt_summary, "iso-rail-gateway")["source_path"] = (
                    "foo_fixture.xml"
                )
                refresh_digest(receipt_summary)

            def blocked_source_reuses_archive_store_dir(
                body,
                evidence_body,
                _xsd_path,
                _evidence_path,
            ):
                set_archive_store_dir(evidence_body, add_blocked_source(body))

            def profile_catalog_reuses_trust_bundle(
                body,
                evidence_body,
                _xsd_path,
                _evidence_path,
            ):
                body["profile_catalog"]["path"] = evidence_body["trust_summaries"][0][
                    "profiles"
                ][0]["path"]

            cases = (
                (
                    "summary-canary-summary",
                    summary_reuses_canary_summary,
                    "xsd.summary_path_matches_evidence_material",
                ),
                (
                    "manifest-evidence-summary",
                    manifest_reuses_evidence_summary,
                    "xsd.manifest_path_matches_evidence_material",
                ),
                (
                    "schema-archive-store-dir",
                    schema_reuses_archive_store_dir,
                    "xsd.schema_path_matches_evidence_material",
                ),
                (
                    "fixture-canary-source",
                    fixture_reuses_canary_source,
                    "xsd.fixture_path_matches_evidence_material",
                ),
                (
                    "blocked-source-archive-store-dir",
                    blocked_source_reuses_archive_store_dir,
                    "xsd.blocked_source_path_matches_evidence_material",
                ),
                (
                    "profile-catalog-trust-bundle",
                    profile_catalog_reuses_trust_bundle,
                    "xsd.profile_catalog_path_matches_evidence_material",
                ),
            )
            for offset, (name, mutate, expected_code) in enumerate(cases):
                with self.subTest(name=name):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    evidence_body = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutated_xsd_path = root / f"xsd-evidence-path-role-{offset}.summary.json"
                    mutated_evidence_path = (
                        root / f"xsd-evidence-path-role-{offset}.evidence.summary.json"
                    )
                    mutate(body, evidence_body, mutated_xsd_path, mutated_evidence_path)
                    refresh_digest(body)
                    refresh_digest(evidence_body)
                    write_json(mutated_xsd_path, body)
                    write_json(mutated_evidence_path, evidence_body)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_xsd_path),
                            "--evidence-summary",
                            str(mutated_evidence_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(expected_code, codes)
                    blocker_text = "\n".join(
                        blocker["message"]
                        for blocker in blockers
                        if blocker["code"] == expected_code
                    )
                    self.assertIn("xsd_summaries[0]", blocker_text)
                    self.assertIn("evidence_summaries[0]", blocker_text)

    def test_xsd_profile_catalog_coordinates_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            missing_version = {
                "profile_id": "minimal-profile",
                "message_type": "fooo.001",
                "direction": "inbound",
                "message_def_id": "fooo.001.001.02",
            }

            def set_version_field(field, value):
                def mutate(body):
                    body["profile_catalog"]["versions"][0][field] = value

                return mutate

            def append_top_level_missing(entry):
                def mutate(body):
                    body["missing_profile_schema_versions"].append(entry)

                return mutate

            def append_catalog_missing(entry):
                def mutate(body):
                    body["profile_catalog"]["missing_schema_versions"].append(entry)

                return mutate

            hidden = "\u0660"
            unicode_digit_message_def_id = f"fooo.{hidden}{hidden}1.001.02"
            cases = (
                (
                    "profile-id",
                    set_version_field("profile_id", "MinimalProfile"),
                    "profile_id must be a canonical lowercase profile id",
                ),
                (
                    "message-type",
                    set_version_field("message_type", "FOOO.001"),
                    "message_type must be lowercase ISO family id",
                ),
                (
                    "direction",
                    set_version_field("direction", "sideways"),
                    "direction must be one of",
                ),
                (
                    "message-family",
                    set_version_field("message_def_id", "barr.001.001.01"),
                    "message_def_id must match message_type",
                ),
                (
                    "message-def-id-non-ascii",
                    set_version_field("message_def_id", unicode_digit_message_def_id),
                    "message_def_id must use printable ASCII",
                ),
                (
                    "top-level-missing-profile-id",
                    append_top_level_missing({**missing_version, "profile_id": "minimal_"}),
                    "profile_id must be a canonical lowercase profile id",
                ),
                (
                    "catalog-missing-message-type",
                    append_catalog_missing({**missing_version, "message_type": "fooo.01"}),
                    "message_type must be lowercase ISO family id",
                ),
            )

            for name, mutate, message in cases:
                with self.subTest(name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(xsd)
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"malformed-xsd-profile-coordinate-{name}.summary.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(unicode_digit_message_def_id, stderr)

    def test_xsd_profile_catalog_version_diagnostics_do_not_echo_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            hidden_message_type = "fooo.001"
            hidden_message_def_id = "barr.001.001.01"
            malformed = json.loads(xsd_summary.read_text(encoding="utf-8"))
            malformed["profile_catalog"]["versions"][0][
                "message_def_id"
            ] = hidden_message_def_id
            refresh_digest(malformed)
            malformed_path = write_json(root / "malformed-profile-version.json", malformed)

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(malformed_path),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("message_def_id must match message_type", stderr)
            self.assertNotIn(hidden_message_type, stderr)
            self.assertNotIn(hidden_message_def_id, stderr)

            skipped_hidden = "fooo.002"
            skipped_mismatch = json.loads(xsd_summary.read_text(encoding="utf-8"))
            skipped_mismatch["profile_catalog"]["skipped_family_versions"].append(
                {
                    "profile_id": "minimal-profile",
                    "message_type": hidden_message_type,
                    "direction": "inbound",
                    "version": skipped_hidden,
                }
            )
            refresh_digest(skipped_mismatch)
            skipped_path = write_json(root / "skipped-profile-version.json", skipped_mismatch)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(skipped_path),
                    "--evidence-summary",
                    str(evidence_summary),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            messages = [
                blocker["message"]
                for blocker in blockers
                if blocker["code"] == "xsd.profile_catalog_skipped_family_mismatch"
            ]
            self.assertTrue(messages)
            for message in messages:
                self.assertIn("version must equal message_type", message)
                self.assertNotIn(hidden_message_type, message)
                self.assertNotIn(skipped_hidden, message)

    def test_xsd_profile_skipped_versions_reject_malformed_aliases_without_echo(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden_digit = "\u0660"
            hidden_secret = "token-readiness-skipped-version-secret"
            cases = (
                (
                    f"fooo.{hidden_digit}01",
                    "skipped_family_versions[0].version must use printable ASCII",
                    hidden_digit,
                ),
                (
                    " fooo.001",
                    "skipped_family_versions[0].version must not have surrounding whitespace",
                    " fooo.001",
                ),
                (
                    hidden_secret,
                    "skipped_family_versions[0].version contains secret-looking material",
                    hidden_secret,
                ),
            )
            for offset, (version, message, hidden) in enumerate(cases):
                with self.subTest(offset=offset):
                    body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    body["profile_catalog"]["skipped_family_versions"].append(
                        {
                            "profile_id": "minimal-profile",
                            "message_type": "fooo.001",
                            "direction": "inbound",
                            "version": version,
                        }
                    )
                    refresh_digest(body)
                    mutated_path = write_json(
                        root / f"malformed-skipped-profile-version-{offset}.summary.json",
                        body,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_xsd_profile_catalog_path_is_canonical(self):
        cases = (
            (
                "/ops/iso/profile catalog.rs",
                "profile_catalog.path must not contain whitespace",
            ),
            (
                "--profiles.rs",
                "profile_catalog.path must not start with a dash",
            ),
            (
                "/ops/iso/--profiles.rs",
                "profile_catalog.path must not contain leading-dash path segments",
            ),
            (
                "/ops/iso/profiles.rs;v=1",
                "profile_catalog.path must not contain semicolon path parameters",
            ),
            (
                "/ops/iso//profiles.rs",
                "profile_catalog.path must not contain empty path segments",
            ),
            (
                "/ops/iso/../profiles.rs",
                "profile_catalog.path must not contain dot or parent segments",
            ),
            (
                r"/ops\iso/profiles.rs",
                "profile_catalog.path must use forward slashes",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for profile_path, message in cases:
                with self.subTest(profile_path=profile_path):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    xsd["profile_catalog"]["path"] = profile_path
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / "malformed-xsd-profile-catalog-path.summary.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_unknown_xsd_summary_keys_are_malformed(self):
        def get_nested(value, parts):
            target = value
            for part in parts:
                target = target[part]
            return target

        def set_unknown(parts):
            return lambda body: get_nested(body, parts).__setitem__("unexpected", "value")

        def append_unknown(parts, entry):
            return lambda body: get_nested(body, parts).append(
                {**entry, "unexpected": "value"}
            )

        version_gap = {
            "profile_id": "minimal-profile",
            "message_type": "fooo.001",
            "direction": "inbound",
            "message_def_id": "fooo.001.001.02",
        }
        skipped_gap = {
            "profile_id": "minimal-profile",
            "message_type": "fooo.001",
            "direction": "inbound",
            "version": "fooo.001",
        }
        schema_gap = {
            "path": "fixtures/iso20022/fooo.001.001.02.xml",
            "message_def_id": "fooo.001.001.02",
            "reason": "adversarial gap entry",
        }
        cases = (
            ("top-level", set_unknown(())),
            ("strict", set_unknown(("strict",))),
            ("schema", set_unknown(("schemas", 0))),
            ("schema-source", set_unknown(("schemas", 0, "source"))),
            ("fixture", set_unknown(("fixtures", 0))),
            ("missing-schema-fixture", append_unknown(("missing_schema_fixtures",), schema_gap)),
            ("schema-only-entry", append_unknown(("schema_only_entries",), schema_gap)),
            (
                "missing-profile-version",
                append_unknown(("missing_profile_schema_versions",), version_gap),
            ),
            ("profile-catalog", set_unknown(("profile_catalog",))),
            (
                "profile-catalog-version",
                set_unknown(("profile_catalog", "versions", 0)),
            ),
            (
                "profile-catalog-missing-version",
                append_unknown(("profile_catalog", "missing_schema_versions"), version_gap),
            ),
            (
                "profile-catalog-skipped-version",
                append_unknown(("profile_catalog", "skipped_family_versions"), skipped_gap),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(xsd)
                    refresh_digest(xsd)
                    mutated_path = write_json(root / f"unknown-xsd-{name}.summary.json", xsd)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)

    def test_xsd_provenance_digests_are_required(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (["manifest_sha256"], "manifest_sha256 must be a lowercase SHA-256 digest"),
                (
                    ["profile_catalog", "sha256"],
                    "profile_catalog.sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    ["profile_catalog", "catalog_json_sha256"],
                    (
                        "profile_catalog.catalog_json_sha256 must be a lowercase "
                        "SHA-256 digest"
                    ),
                ),
            )
            for path_parts, message in cases:
                with self.subTest(path_parts=path_parts):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    target = xsd
                    for part in path_parts[:-1]:
                        target = target[part]
                    del target[path_parts[-1]]
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"missing-xsd-provenance-{'-'.join(path_parts)}.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_xsd_provenance_digests_reject_all_zero_placeholders(self):
        def attach_blocked_sources(body, entries):
            body["blocked_schema_sources"] = entries
            body["blocked_schema_source_count"] = len(entries)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "manifest",
                    lambda body: body.__setitem__("manifest_sha256", "0" * 64),
                    "manifest_sha256 must not be all zero",
                ),
                (
                    "profile-catalog",
                    lambda body: body["profile_catalog"].__setitem__("sha256", "0" * 64),
                    "profile_catalog.sha256 must not be all zero",
                ),
                (
                    "profile-catalog-json",
                    lambda body: body["profile_catalog"].__setitem__(
                        "catalog_json_sha256",
                        "0" * 64,
                    ),
                    "profile_catalog.catalog_json_sha256 must not be all zero",
                ),
                (
                    "schema",
                    lambda body: body["schemas"][0].__setitem__("sha256", "0" * 64),
                    "schemas[0].sha256 must not be all zero",
                ),
                (
                    "schema-source",
                    lambda body: body["schemas"][0]["source"].__setitem__(
                        "sha256",
                        "0" * 64,
                    ),
                    "schemas[0].source.sha256 must not be all zero",
                ),
                (
                    "fixture",
                    lambda body: body["fixtures"][0].__setitem__("sha256", "0" * 64),
                    "fixtures[0].sha256 must not be all zero",
                ),
                (
                    "blocked-source",
                    lambda body: (
                        attach_blocked_sources(body, [xsd_test.blocked_schema_source()]),
                        body["blocked_schema_sources"][0]["source"].__setitem__(
                            "sha256",
                            "0" * 64,
                        ),
                    ),
                    "blocked_schema_sources[0].source.sha256 must not be all zero",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    mutate(xsd)
                    refresh_digest(xsd)
                    mutated_path = write_json(
                        root / f"zero-xsd-provenance-{name}.summary.json",
                        xsd,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(mutated_path),
                            "--evidence-summary",
                            str(evidence_summary),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_tampered_xsd_or_evidence_summary_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            tampered_xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            expected_xsd_digest = tampered_xsd[READINESS.SUMMARY_DIGEST_FIELD]
            tampered_xsd["verified_schemas"] = 99
            actual_xsd_body = dict(tampered_xsd)
            actual_xsd_body.pop(READINESS.SUMMARY_DIGEST_FIELD)
            actual_xsd_digest = READINESS.sha256_hex(
                READINESS._canonical_json_bytes(actual_xsd_body)
            )
            tampered_xsd_path = write_json(root / "tampered-xsd.summary.json", tampered_xsd)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(tampered_xsd_path), "--evidence-summary", str(evidence_summary)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)
            self.assertNotIn(expected_xsd_digest, stderr)
            self.assertNotIn(actual_xsd_digest, stderr)

            tampered_evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            expected_evidence_digest = tampered_evidence[READINESS.SUMMARY_DIGEST_FIELD]
            tampered_evidence["ok"] = False
            actual_evidence_body = dict(tampered_evidence)
            actual_evidence_body.pop(READINESS.SUMMARY_DIGEST_FIELD)
            actual_evidence_digest = READINESS.sha256_hex(
                READINESS._canonical_json_bytes(actual_evidence_body)
            )
            tampered_evidence_path = write_json(root / "tampered-evidence.summary.json", tampered_evidence)
            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(tampered_evidence_path)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)
            self.assertNotIn(expected_evidence_digest, stderr)
            self.assertNotIn(actual_evidence_digest, stderr)

    def test_input_summary_digest_rejects_all_zero_placeholder(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            actual_xsd_digest = xsd["summary_sha256"]
            xsd["summary_sha256"] = "0" * 64
            zero_xsd_path = write_json(root / "zero-xsd.summary.json", xsd)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(zero_xsd_path), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_xsd_digest, stderr)
            self.assertNotIn("mismatch", stderr)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            actual_evidence_digest = evidence["summary_sha256"]
            evidence["summary_sha256"] = "0" * 64
            zero_evidence_path = write_json(root / "zero-evidence.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(zero_evidence_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_evidence_digest, stderr)
            self.assertNotIn("mismatch", stderr)

    def test_input_summary_verified_at_is_required_timezone_aware_and_not_future(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("missing", None, "verified_at must be a non-empty string"),
                ("naive", "2026-06-04T00:00:00", "verified_at must include a timezone"),
                ("future", "2999-01-01T00:00:00+00:00", "verified_at must not be in the future"),
            )
            for target_name, source_path in (("xsd", xsd_summary), ("evidence", evidence_summary)):
                for name, value, message in cases:
                    with self.subTest(target=target_name, name=name):
                        body = json.loads(source_path.read_text(encoding="utf-8"))
                        if value is None:
                            del body["verified_at"]
                        else:
                            body["verified_at"] = value
                        refresh_digest(body)
                        mutated_path = write_json(
                            root / f"{target_name}-{name}-verified-at.summary.json",
                            body,
                        )
                        argv = ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
                        if target_name == "xsd":
                            argv[1] = str(mutated_path)
                        else:
                            argv[3] = str(mutated_path)

                        rc, _stdout, stderr = run_readiness(argv)

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)

    def test_stale_digest_correct_summaries_block_readiness(self):
        old_start = "2000-01-01T00:00:00+00:00"
        old_rail_done = "2000-01-01T00:00:01+00:00"
        old_notary_done = "2000-01-01T00:00:02+00:00"
        old_finish = "2000-01-01T00:00:03+00:00"

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            stale_xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            stale_xsd["verified_at"] = old_start
            refresh_digest(stale_xsd)
            stale_xsd_path = write_json(root / "stale-xsd.summary.json", stale_xsd)

            stale_evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            stale_evidence["verified_at"] = old_start
            refresh_digest(stale_evidence)
            stale_evidence_path = write_json(
                root / "stale-evidence.summary.json",
                stale_evidence,
            )

            stale_canary = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = stale_canary["canary_summaries"][0]
            canary["started_at"] = old_start
            canary["finished_at"] = old_finish
            canary["stage_windows"] = [
                {
                    "name": "rail",
                    "started_at": old_start,
                    "finished_at": old_rail_done,
                },
                {
                    "name": "notary",
                    "started_at": old_rail_done,
                    "finished_at": old_notary_done,
                },
                {
                    "name": "verify",
                    "started_at": old_notary_done,
                    "finished_at": old_finish,
                },
            ]
            refresh_digest(stale_canary)
            stale_canary_path = write_json(
                root / "stale-canary.summary.json",
                stale_canary,
            )

            stale_trust = json.loads(evidence_summary.read_text(encoding="utf-8"))
            stale_trust["trust_summaries"][0]["verified_at"] = old_start
            refresh_digest(stale_trust)
            stale_trust_path = write_json(root / "stale-trust.summary.json", stale_trust)

            cases = (
                (
                    ["--xsd-summary", str(stale_xsd_path), "--evidence-summary", str(evidence_summary), "--max-xsd-age-days", "1"],
                    "xsd.summary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_evidence_path), "--max-evidence-age-days", "1"],
                    "evidence.summary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_canary_path), "--max-canary-age-days", "1"],
                    "evidence.canary_stale",
                ),
                (
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(stale_trust_path), "--max-trust-age-days", "1"],
                    "trust.summary_stale",
                ),
            )
            for argv, code in cases:
                with self.subTest(code=code):
                    rc, stdout, stderr = run_readiness(argv)

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_missing_xsd_strict_flags_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for flag in (
                "require_schema_backed_fixtures",
                "require_fixture_for_schema",
                "require_profile_schema_backed_versions",
                "validate_xml_schema",
            ):
                with self.subTest(flag=flag):
                    xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
                    del xsd["strict"][flag]
                    refresh_digest(xsd)
                    mutated_path = write_json(root / f"missing-{flag}.xsd-summary.json", xsd)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(mutated_path), "--evidence-summary", str(evidence_summary)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(f"strict.{flag} must be a boolean", stderr)

    def test_evidence_policy_and_provider_environment_drift_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for flag in READINESS.PRODUCTION_FALSE_POLICY_FLAGS:
                evidence["policy"][flag] = True
            refresh_digest(evidence)
            mutated_path = write_json(root / "policy-evidence.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(mutated_path),
                    "--provider",
                    "other-bank",
                    "--environment",
                    "prod",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            for flag in READINESS.PRODUCTION_FALSE_POLICY_FLAGS:
                self.assertIn(f"evidence.policy.{flag}", codes)
            self.assertIn("evidence.policy_provider_mismatch", codes)
            self.assertIn("evidence.policy_environment_mismatch", codes)
            self.assertIn("evidence.provider_mismatch", codes)
            self.assertIn("evidence.environment_mismatch", codes)
            self.assertIn("trust.environment_mismatch", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in summary["blockers"])
            self.assertIn(
                "evidence policy provider does not match expected provider",
                blocker_text,
            )
            self.assertIn(
                "evidence policy environment does not match expected environment",
                blocker_text,
            )
            self.assertIn("canary provider does not match expected provider", blocker_text)
            self.assertIn(
                "canary environment does not match expected environment",
                blocker_text,
            )
            self.assertIn(
                "trust profile environment does not match expected environment",
                blocker_text,
            )
            self.assertNotIn("other-bank", blocker_text)
            self.assertNotIn("local-bank", blocker_text)
            self.assertNotIn("'prod'", blocker_text)
            self.assertNotIn("'preprod'", blocker_text)

    def test_profile_json_not_emitted_evidence_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_root = root / "evidence"
            evidence_root.mkdir()
            notary_receipts, rail_receipts = evidence_test.write_https_receipt_dirs(evidence_root)
            canary_path = evidence_test.write_canary(
                evidence_root,
                evidence_test.valid_canary_summary(
                    receipt_entries=evidence_test.receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                ),
            )
            trust_path = evidence_test.write_trust_summary(
                evidence_root / "trust",
                emit_profile_json=False,
            )
            evidence_summary = evidence_root / "evidence.summary.json"
            rc, _stdout, stderr = evidence_test.run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--allow-profile-json-not-emitted",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(evidence_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(evidence_summary)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("evidence.policy.allow_profile_json_not_emitted", codes)
            self.assertIn("trust.profile_json_not_emitted", codes)

    def test_profile_json_not_emitted_digest_must_be_recorded_null(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_root = root / "evidence"
            evidence_root.mkdir()
            notary_receipts, rail_receipts = evidence_test.write_https_receipt_dirs(
                evidence_root
            )
            canary_path = evidence_test.write_canary(
                evidence_root,
                evidence_test.valid_canary_summary(
                    receipt_entries=evidence_test.receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                ),
            )
            trust_path = evidence_test.write_trust_summary(
                evidence_root / "trust",
                emit_profile_json=False,
            )
            evidence_summary = evidence_root / "evidence.summary.json"
            rc, _stdout, stderr = evidence_test.run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--allow-profile-json-not-emitted",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(evidence_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)
            evidence_summary = add_archive_receipt_verification(evidence_summary)
            cases = (
                (
                    "missing",
                    lambda evidence: evidence["trust_summaries"][0].pop(
                        "profile_json_sha256"
                    ),
                ),
                (
                    "non-null",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "profile_json_sha256",
                        "0" * 64,
                    ),
                ),
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"{name}-profile-json-digest.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(
                        "profile_json_sha256 must be null when profile JSON was not emitted",
                        stderr,
                    )

    def test_profile_json_not_emittable_evidence_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["profile_json_emittable"] = False
            refresh_digest(evidence)
            mutated_path = write_json(root / "not-emittable-evidence.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("trust.profile_json_not_emittable", codes)
            self.assertIn("trust.profile_json_emitted_not_emittable", codes)

    def test_missing_compact_trust_source_blocks_readiness_without_malformed_abort(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_root = root / "evidence"
            evidence_root.mkdir()
            notary_receipts, rail_receipts = evidence_test.write_https_receipt_dirs(evidence_root)
            canary_path = evidence_test.write_canary(
                evidence_root,
                evidence_test.valid_canary_summary(
                    receipt_entries=evidence_test.receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                ),
            )
            trust_path = evidence_test.write_trust_summary(evidence_root / "trust")
            evidence_test.rewrite_trust_summary(
                trust_path,
                lambda trust: (
                    trust.__setitem__("max_source_age_days", None),
                    trust.__setitem__("profile_json_emitted", False),
                    trust.__setitem__("profile_json_emittable", False),
                    trust.__setitem__("profile_json_sha256", None),
                    trust["bundles"][0].__setitem__("source", None),
                ),
            )
            evidence_summary = evidence_root / "evidence.summary.json"
            rc, _stdout, stderr = evidence_test.run_evidence(
                [
                    "--canary-summary",
                    str(canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--allow-missing-trust-source",
                    "--allow-profile-json-not-emitted",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(evidence_summary),
                ]
            )
            self.assertEqual(rc, 0, stderr)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("evidence.policy.allow_missing_trust_source", codes)
            self.assertIn("evidence.policy.allow_profile_json_not_emitted", codes)
            self.assertIn("trust.source_missing", codes)
            self.assertIn("trust.profile_json_not_emitted", codes)
            self.assertIn("trust.profile_json_not_emittable", codes)
            trust_profile = summary["evidence_summaries"][0]["trust_summaries"][0]["profiles"][0]
            self.assertIsNone(trust_profile["source"])

    def test_compact_trust_verifier_override_flags_block_readiness(self):
        cases = (
            ("allow_synthetic_der", "trust.allow_synthetic_der", None),
            (
                "allow_record_only",
                "trust.allow_record_only",
                "no non-production embedded_signature_policy was recorded",
            ),
            (
                "allow_insecure_source_url",
                "trust.allow_insecure_source_url",
                "no http:// or local/private source URL was recorded",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for field, blocker_code, message in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0][field] = True
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"trust-override-{field}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(blocker_code, codes)
                    self.assertIn("trust.profile_json_emittable_drift", codes)
                    if message is not None:
                        matching = [
                            blocker for blocker in blockers if blocker["code"] == blocker_code
                        ]
                        self.assertTrue(
                            any(message in blocker["message"] for blocker in matching)
                        )

    def test_compact_insecure_trust_source_blocks_readiness_without_malformed_abort(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("http-source", "http://pki.local/swift-cbpr-plus"),
                ("local-private-https-source", "https://127.0.0.1/swift-cbpr-plus"),
            )
            for name, source_url in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["policy"]["allow_insecure_http"] = True
                    evidence["policy"]["allow_profile_json_not_emitted"] = True
                    trust_summary = evidence["trust_summaries"][0]
                    trust_summary["allow_insecure_source_url"] = True
                    trust_summary["profile_json_emitted"] = False
                    trust_summary["profile_json_emittable"] = False
                    trust_summary["profile_json_sha256"] = None
                    trust_summary["profiles"][0]["source"]["url"] = source_url
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"insecure-trust-source-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    summary = json.loads(stdout)
                    codes = {blocker["code"] for blocker in summary["blockers"]}
                    self.assertIn("evidence.policy.allow_insecure_http", codes)
                    self.assertIn("evidence.policy.allow_profile_json_not_emitted", codes)
                    self.assertIn("trust.allow_insecure_source_url", codes)
                    self.assertIn("trust.profile_json_not_emitted", codes)
                    self.assertIn("trust.profile_json_not_emittable", codes)
                    compact_source = summary["evidence_summaries"][0]["trust_summaries"][0][
                        "profiles"
                    ][0]["source"]
                    self.assertEqual(compact_source["url"], source_url)

    def test_missing_evidence_policy_flag_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            for key in sorted(READINESS.EVIDENCE_POLICY_KEYS):
                if key == "default_rail_profile":
                    continue
                if key in {"provider", "environment"}:
                    message = f"policy.{key} must be a non-empty string"
                elif key in READINESS.PRODUCTION_FALSE_POLICY_FLAGS:
                    message = f"policy.{key} must be a boolean"
                else:
                    self.assertIn(key, READINESS.EVIDENCE_FRESHNESS_POLICY_FIELDS)
                    message = f"policy.{key} must be a positive integer"
                cases.append((key, message))
            for key, message in cases:
                with self.subTest(key=key):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    del evidence["policy"][key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-policy-{key}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_evidence_policy_default_rail_profile_is_validated(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden = "\u0661"
            cases = (
                (True, "must be null or a canonical lowercase profile id", None),
                (
                    "Swift-CBPR-Plus",
                    "must be a canonical lowercase profile id",
                    "Swift-CBPR-Plus",
                ),
                (hidden, "must use printable ASCII", hidden),
                (
                    "token-readiness-profile-secret",
                    "secret-looking material",
                    "token-readiness-profile-secret",
                ),
            )
            for value, message, hidden_value in cases:
                with self.subTest(value=value):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["policy"]["default_rail_profile"] = value
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"policy-default-profile-{len(str(value))}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if hidden_value is not None:
                        self.assertNotIn(hidden_value, stderr)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["policy"]["default_rail_profile"] = "swift-cbpr-plus"
            evidence["policy"]["allow_default_profile"] = False
            refresh_digest(evidence)
            inconsistent_path = write_json(
                root / "policy-default-profile-without-override.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(inconsistent_path),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            self.assertEqual(stderr, "")
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn(
                "evidence.policy.default_rail_profile_without_override",
                codes,
            )

    def test_weaker_evidence_freshness_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("max_canary_age_days", "evidence.policy.max_canary_age_days_weaker_than_release"),
                ("max_trust_age_days", "evidence.policy.max_trust_age_days_weaker_than_release"),
                (
                    "max_trust_source_age_days",
                    "evidence.policy.max_trust_source_age_days_weaker_than_release",
                ),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["policy"][field] = 30
                    if field == "max_trust_source_age_days":
                        evidence["trust_summaries"][0]["max_source_age_days"] = 7
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"weaker-{field}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                            "--max-canary-age-days",
                            "7",
                            "--max-trust-age-days",
                            "7",
                            "--max-trust-source-age-days",
                            "7",
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_compact_identity_strings_reject_control_characters(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "policy-provider",
                ("policy", "provider"),
                "local\nbank",
                "policy.provider must not contain control characters",
            ),
            (
                "canary-provider",
                ("canary_summaries", 0, "provider"),
                "local\nbank",
                "provider must not contain control characters",
            ),
            (
                "stage-name",
                ("canary_summaries", 0, "stage_names", 0),
                "rail\nx",
                "stage_names[0] must not contain control characters",
            ),
            (
                "stage-window-name",
                ("canary_summaries", 0, "stage_windows", 0, "name"),
                "rail\nx",
                "name must not contain control characters",
            ),
            (
                "trust-profile-id",
                ("trust_summaries", 0, "profiles", 0, "profile_id"),
                "swift\ncbpr",
                "profile_id must not contain control characters",
            ),
            (
                "trust-policy",
                ("trust_summaries", 0, "profiles", 0, "embedded_signature_policy"),
                "require-verified\n",
                "embedded_signature_policy must not contain control characters",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, value, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, value)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"control-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_context_strings_reject_non_ascii_without_echo(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "policy-provider",
                ("policy", "provider"),
                "local-b\u00e1nk",
                "policy.provider must use printable ASCII",
            ),
            (
                "policy-environment",
                ("policy", "environment"),
                "prepr\u043ed",
                "policy.environment must use printable ASCII",
            ),
            (
                "canary-provider",
                ("canary_summaries", 0, "provider"),
                "local-b\u00e1nk",
                "provider must use printable ASCII",
            ),
            (
                "canary-environment",
                ("canary_summaries", 0, "environment"),
                "prepr\u043ed",
                "environment must use printable ASCII",
            ),
            (
                "trust-environment",
                ("trust_summaries", 0, "profiles", 0, "environment"),
                "prepr\u043ed",
                "environment must use printable ASCII",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, hidden, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, hidden)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"nonascii-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_compact_trust_profile_identity_fields_are_canonical(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "uppercase-profile-id",
                ("trust_summaries", 0, "profiles", 0, "profile_id"),
                "Swift-CBPR-Plus",
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "underscore-profile-id",
                ("trust_summaries", 0, "profiles", 0, "profile_id"),
                "swift_cbpr_plus",
                "profile_id must be a canonical lowercase profile id",
            ),
            (
                "unknown-rail",
                ("trust_summaries", 0, "profiles", 0, "rail"),
                "swift",
                "rail must be one of",
            ),
            (
                "uppercase-rail",
                ("trust_summaries", 0, "profiles", 0, "rail"),
                "Swift-CBPR-Plus",
                "rail must be one of",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, value, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, value)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trust-identity-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_secret_looking_compact_identity_values_are_rejected_without_echo(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "policy-provider",
                ("policy", "provider"),
                "token-readiness-policy-secret",
            ),
            (
                "policy-environment",
                ("policy", "environment"),
                "private-key-readiness-policy-secret",
            ),
            (
                "canary-provider",
                ("canary_summaries", 0, "provider"),
                "token-readiness-canary-secret",
            ),
            (
                "canary-environment",
                ("canary_summaries", 0, "environment"),
                "private-key-readiness-canary-secret",
            ),
            (
                "trust-profile-id",
                ("trust_summaries", 0, "profiles", 0, "profile_id"),
                "token-readiness-profile-secret",
            ),
            (
                "trust-environment",
                ("trust_summaries", 0, "profiles", 0, "environment"),
                "token-readiness-environment-secret",
            ),
            (
                "trust-rail",
                ("trust_summaries", 0, "profiles", 0, "rail"),
                "token-readiness-rail-secret",
            ),
            (
                "trust-policy",
                ("trust_summaries", 0, "profiles", 0, "embedded_signature_policy"),
                "token-readiness-trust-policy-secret",
            ),
            (
                "trust-bundle-sha",
                ("trust_summaries", 0, "profiles", 0, "bundle_sha256"),
                "token-readiness-bundle-sha-secret",
            ),
            (
                "trust-source-authority",
                ("trust_summaries", 0, "profiles", 0, "source", "authority"),
                "token-readiness-authority-secret",
            ),
            (
                "trust-source-version",
                ("trust_summaries", 0, "profiles", 0, "source", "version"),
                "session-key-readiness-version-secret",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, secret in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, secret)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"secret-identity-{name}.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret, stderr)

    def test_non_ascii_compact_trust_source_identity_values_are_rejected_without_echo(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "trust-source-authority",
                ("trust_summaries", 0, "profiles", 0, "source", "authority"),
                "ISO\u2011MDR",
            ),
            (
                "trust-source-version",
                ("trust_summaries", 0, "profiles", 0, "source", "version"),
                "2026\u2011Q2",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, hidden in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, hidden)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"nonascii-{name}.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("must use printable ASCII", stderr)
                    self.assertNotIn(hidden, stderr)

    def test_overlong_compact_trust_source_identity_values_are_rejected_without_echo(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        hidden = "A" * (READINESS.MAX_TRUST_SOURCE_TEXT_CHARS + 1)
        cases = (
            (
                "trust-source-authority",
                ("trust_summaries", 0, "profiles", 0, "source", "authority"),
                "source.authority must be no longer than 256 characters",
            ),
            (
                "trust-source-version",
                ("trust_summaries", 0, "profiles", 0, "source", "version"),
                "source.version must be no longer than 256 characters",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, hidden)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"overlong-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_secret_or_non_ascii_compact_receipt_kind_values_are_rejected_without_echo(self):
        def mutate_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value
            if parts[0] == "canary_summaries":
                refresh_digest(evidence["canary_summaries"][0]["receipt_summary"])
            else:
                refresh_digest(evidence["receipt_verification"])

        cases = (
            (
                "canary-kind-list",
                ("canary_summaries", 0, "receipt_summary", "receipt_kind", 0),
            ),
            (
                "canary-entry-kind",
                ("canary_summaries", 0, "receipt_summary", "receipts", 0, "receipt_kind"),
            ),
            (
                "archive-kind-list",
                ("receipt_verification", "receipt_kind", 0),
            ),
            (
                "archive-entry-kind",
                ("receipt_verification", "receipts", 0, "receipt_kind"),
            ),
        )
        values = (
            ("secret", "token-readiness-receipt-kind-secret", "secret-looking material"),
            ("non-ascii", "iso-rail-gatew\u0430y", "must use printable ASCII"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts in cases:
                for value_kind, value, message in values:
                    with self.subTest(name=name, value_kind=value_kind):
                        evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        mutate_nested(evidence, parts, value)
                        refresh_digest(evidence)
                        mutated_path = write_json(
                            root / f"{value_kind}-receipt-kind-{name}.summary.json",
                            evidence,
                        )

                        rc, _stdout, stderr = run_readiness(
                            [
                                "--xsd-summary",
                                str(xsd_summary),
                                "--evidence-summary",
                                str(mutated_path),
                            ]
                        )

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)
                        self.assertNotIn(value, stderr)
                        if value_kind == "non-ascii":
                            self.assertNotIn("unsupported", stderr)

    def test_overlong_compact_clean_strings_are_rejected_without_echo(self):
        overlong = "M" * (READINESS.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "required",
                lambda: READINESS._require_string({"provider": overlong}, "provider", "summary"),
                f"summary.provider must be no longer than {READINESS.MAX_CLEAN_STRING_CHARS} characters",
            ),
            (
                "cli",
                lambda: READINESS._require_cli_string(overlong, "--provider"),
                f"--provider must be no longer than {READINESS.MAX_CLEAN_STRING_CHARS} characters",
            ),
        )
        for name, call, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "receipt-kind",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "receipt_kind"
                    ].__setitem__(0, overlong),
                    lambda evidence: refresh_digest(
                        evidence["canary_summaries"][0]["receipt_summary"]
                    ),
                    "receipt_kind[0] must be no longer",
                ),
                (
                    "stage-name",
                    lambda evidence: evidence["canary_summaries"][0]["stage_names"].__setitem__(
                        0,
                        overlong,
                    ),
                    lambda _evidence: None,
                    "stage_names[0] must be no longer",
                ),
            )
            for name, mutate, refresh_nested, expected in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_nested(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"overlong-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(expected, stderr)
                    self.assertIn(
                        f"{READINESS.MAX_CLEAN_STRING_CHARS} characters",
                        stderr,
                    )
                    self.assertNotIn(overlong, stderr)
                    self.assertNotIn("unsupported", stderr)

    def test_compact_string_helpers_reject_unicode_format_controls_without_echo(self):
        hidden = "\u202ereadiness-string-leak"
        cases = (
            (
                "required",
                lambda: READINESS._require_string(
                    {"provider": "local" + hidden}, "provider", "summary"
                ),
            ),
            (
                "cli",
                lambda: READINESS._require_cli_string(
                    "local" + hidden,
                    "--provider",
                ),
            ),
            (
                "nullable-rail-message-id",
                lambda: READINESS._require_nullable_rail_message_id(
                    {"rail_message_id": "rail-message" + hidden},
                    "rail_message_id",
                    "receipt",
                ),
            ),
            (
                "reviewed-gap-reason",
                lambda: READINESS._validate_reviewed_gap_reason(
                    "Reviewed" + hidden,
                    "schema_only_reason",
                ),
            ),
        )
        for name, call in cases:
            with self.subTest(name=name):
                with self.assertRaises(READINESS.ReadinessError) as caught:
                    call()

                message = str(caught.exception)
                self.assertIn("control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("readiness-string-leak", message)

    def test_secret_or_non_ascii_compact_stage_names_are_rejected_without_echo(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "stage-name",
                ("canary_summaries", 0, "stage_names", 0),
            ),
            (
                "stage-window-name",
                ("canary_summaries", 0, "stage_windows", 0, "name"),
            ),
        )
        values = (
            ("secret", "token-readiness-stage-secret", "secret-looking material"),
            ("non-ascii", "ra\u0430l", "must use printable ASCII"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts in cases:
                for value_kind, value, message in values:
                    with self.subTest(name=name, value_kind=value_kind):
                        evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        set_nested(evidence, parts, value)
                        refresh_digest(evidence)
                        mutated_path = write_json(
                            root / f"{value_kind}-stage-name-{name}.summary.json",
                            evidence,
                        )

                        rc, _stdout, stderr = run_readiness(
                            [
                                "--xsd-summary",
                                str(xsd_summary),
                                "--evidence-summary",
                                str(mutated_path),
                            ]
                        )

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)
                        self.assertNotIn(value, stderr)
                        if value_kind == "non-ascii":
                            self.assertNotIn("unsupported stages", stderr)

    def test_compact_identity_strings_reject_surrounding_whitespace(self):
        def set_nested(evidence, parts, value):
            target = evidence
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = value

        cases = (
            (
                "policy-provider",
                ("policy", "provider"),
                "local-bank ",
                "policy.provider must not have surrounding whitespace",
            ),
            (
                "canary-path",
                ("canary_summaries", 0, "path"),
                " canary.summary.json",
                "path must not have surrounding whitespace",
            ),
            (
                "canary-config-path",
                ("canary_summaries", 0, "config_path"),
                "/ops/iso/canary.json ",
                "config_path must not have surrounding whitespace",
            ),
            (
                "stage-name",
                ("canary_summaries", 0, "stage_names", 0),
                "rail ",
                "stage_names[0] must not have surrounding whitespace",
            ),
            (
                "stage-window-name",
                ("canary_summaries", 0, "stage_windows", 0, "name"),
                " rail",
                "name must not have surrounding whitespace",
            ),
            (
                "trust-summary-path",
                ("trust_summaries", 0, "path"),
                " trust.summary.json",
                "path must not have surrounding whitespace",
            ),
            (
                "trust-source-url",
                ("trust_summaries", 0, "profiles", 0, "source", "url"),
                "https://pki.example.invalid/swift-cbpr-plus ",
                "source.url must not have surrounding whitespace",
            ),
            (
                "trust-profile-id",
                ("trust_summaries", 0, "profiles", 0, "profile_id"),
                " swift-cbpr-plus",
                "profile_id must not have surrounding whitespace",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, parts, value, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    set_nested(evidence, parts, value)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trim-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            with self.subTest(name="receipt-path"):
                evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
                receipt_summary["receipts"][0]["path"] += " "
                refresh_digest(receipt_summary)
                refresh_digest(evidence)
                mutated_path = write_json(root / "trim-receipt-path.summary.json", evidence)

                rc, _stdout, stderr = run_readiness(
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                )

                self.assertEqual(rc, 2)
                self.assertIn("path must not have surrounding whitespace", stderr)

            with self.subTest(name="receipt-kind"):
                evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
                receipt_summary["receipt_kind"][0] += " "
                refresh_digest(receipt_summary)
                refresh_digest(evidence)
                mutated_path = write_json(root / "trim-receipt-kind.summary.json", evidence)

                rc, _stdout, stderr = run_readiness(
                    ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                )

                self.assertEqual(rc, 2)
                self.assertIn("receipt_kind[0] must not have surrounding whitespace", stderr)

    def test_unknown_compact_evidence_keys_are_malformed(self):
        def get_nested(value, parts):
            target = value
            for part in parts:
                target = target[part]
            return target

        cases = (
            ("top-level", (), ()),
            ("policy", ("policy",), ()),
            ("canary", ("canary_summaries", 0), ()),
            ("stage-window", ("canary_summaries", 0, "stage_windows", 0), ()),
            (
                "canary-receipt-summary",
                ("canary_summaries", 0, "receipt_summary"),
                (("canary_summaries", 0, "receipt_summary"),),
            ),
            (
                "canary-receipt-entry",
                ("canary_summaries", 0, "receipt_summary", "receipts", 0),
                (("canary_summaries", 0, "receipt_summary"),),
            ),
            ("trust-summary", ("trust_summaries", 0), ()),
            ("trust-profile", ("trust_summaries", 0, "profiles", 0), ()),
            (
                "archive-receipt-summary",
                ("receipt_verification",),
                (("receipt_verification",),),
            ),
            (
                "archive-receipt-entry",
                ("receipt_verification", "receipts", 0),
                (("receipt_verification",),),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, target_path, digest_paths in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = get_nested(evidence, target_path)
                    target["unexpected"] = "value"
                    for digest_path in digest_paths:
                        refresh_digest(get_nested(evidence, digest_path))
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"unknown-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)

    def test_missing_evidence_status_booleans_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("summary-ok", [], "ok", "ok must be a boolean"),
                (
                    "canary-plan-only",
                    ["canary_summaries", 0],
                    "plan_only",
                    "plan_only must be a boolean",
                ),
                (
                    "canary-explicit-policy",
                    ["canary_summaries", 0],
                    "require_explicit_policy",
                    "require_explicit_policy must be a boolean",
                ),
                (
                    "canary-path",
                    ["canary_summaries", 0],
                    "path",
                    "path must be a non-empty string",
                ),
                (
                    "canary-summary-digest",
                    ["canary_summaries", 0],
                    "summary_sha256",
                    "summary_sha256 must be a lowercase SHA-256 digest",
                ),
            )
            for name, path_parts, key, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = evidence
                    for part in path_parts:
                        target = target[part]
                    del target[key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_missing_trust_summary_provenance_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "path must be a non-empty string"),
                ("verified_at", "verified_at must be a non-empty string"),
                ("max_source_age_days", "max_source_age_days must be recorded"),
                ("allow_synthetic_der", "allow_synthetic_der must be a boolean"),
                ("allow_record_only", "allow_record_only must be a boolean"),
                ("allow_insecure_source_url", "allow_insecure_source_url must be a boolean"),
                ("profile_json_emitted", "profile_json_emitted must be a boolean"),
                ("profile_json_emittable", "profile_json_emittable must be a boolean"),
                ("profile_json_sha256", "profile_json_sha256 must be a lowercase SHA-256 digest"),
                ("summary_sha256", "summary_sha256 must be a lowercase SHA-256 digest"),
            )
            for key, message in cases:
                with self.subTest(key=key):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    del evidence["trust_summaries"][0][key]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-trust-{key}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_trust_source_freshness_policy_is_required_and_strong_enough(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            malformed_cases = (
                ("null", None, "max_source_age_days must be a positive integer"),
                ("zero", 0, "max_source_age_days must be a positive integer"),
                ("bool", True, "max_source_age_days must be a positive integer"),
                ("string", "7", "max_source_age_days must be a positive integer"),
            )
            for name, value, message in malformed_cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0]["max_source_age_days"] = value
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trust-source-budget-{name}.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            refresh_digest(evidence)
            weaker_path = write_json(root / "trust-source-budget-weaker.json", evidence)
            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(weaker_path),
                    "--max-trust-source-age-days",
                    "7",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.policy.max_trust_source_age_days_weaker_than_release", codes)
            self.assertIn("trust.source_freshness_budget_weaker_than_release", codes)

    def test_compact_canary_and_trust_summary_paths_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "/ops/iso/canary\n.summary.json",
                    "must not contain control characters",
                ),
                (
                    "/ops/iso/can ary.summary.json",
                    "must not contain whitespace",
                ),
                (
                    "/ops/iso/can\u0430ry.summary.json",
                    "must use printable ASCII",
                ),
                (
                    "/ops/iso/canary.summary.json;v=1",
                    "must not contain semicolon path parameters",
                ),
                (
                    "/ops/iso//canary.summary.json",
                    "must not contain empty path segments",
                ),
                (
                    "/ops/iso/../canary.summary.json",
                    "must not contain dot or parent segments",
                ),
                (
                    r"..\canary.summary.json",
                    "must not contain dot or parent segments",
                ),
                (
                    r"/ops\iso/canary.summary.json",
                    "must use forward slashes",
                ),
                (
                    "/ops/iso/token=readiness-compact-summary-secret.summary.json",
                    "secret-looking material",
                ),
                (
                    "/ops/iso/canary.summary.txt",
                    "must point to a .json file",
                ),
            )
            locations = (
                ("canary", ("canary_summaries", 0)),
                ("trust", ("trust_summaries", 0)),
            )
            for location, parts in locations:
                for offset, (summary_path, message) in enumerate(cases):
                    with self.subTest(location=location, summary_path=summary_path):
                        evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        target = evidence
                        for part in parts:
                            target = target[part]
                        target["path"] = summary_path
                        refresh_digest(evidence)
                        mutated_path = write_json(
                            root / f"bad-{location}-summary-path-{offset}.summary.json",
                            evidence,
                        )

                        rc, _stdout, stderr = run_readiness(
                            ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                        )

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)
                        if any(ord(ch) > 0x7E for ch in summary_path):
                            self.assertNotIn(summary_path, stderr)

    def test_compact_canary_config_path_is_canonical(self):
        cases = (
            ("/ops/iso/canary\n.json", "config_path must not contain control characters"),
            ("/ops/iso/can ary.json", "config_path must not contain whitespace"),
            ("/ops/iso/can\u0430ry.json", "config_path must use printable ASCII"),
            ("--canary.json", "config_path must not start with a dash"),
            (
                "/ops/iso/--canary.json",
                "config_path must not contain leading-dash path segments",
            ),
            ("/ops/iso/canary.json;v=1", "config_path must not contain semicolon path parameters"),
            ("/ops/iso//canary.json", "config_path must not contain empty path segments"),
            ("/ops/iso/../canary.json", "config_path must not contain dot or parent segments"),
            (r"..\canary.json", "config_path must not contain dot or parent segments"),
            (r"/ops\iso/canary.json", "config_path must use forward slashes"),
            ("/ops/iso/canary.txt", "config_path must point to a .json file"),
            ("/ops/iso/token=readiness-config-secret.json", "secret-looking material"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, (config_path, message) in enumerate(cases):
                with self.subTest(config_path=config_path):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["canary_summaries"][0]["config_path"] = config_path
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"bad-canary-config-path-{offset}.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    if any(ord(ch) > 0x7E for ch in config_path):
                        self.assertNotIn(config_path, stderr)

    def test_compact_canary_config_path_blocks_checked_in_runbook_templates(self):
        checked_in_runbook = (
            REPO_ROOT
            / "fixtures"
            / "iso20022"
            / "operator_canary"
            / "swift_cbpr_plus.preprod.example.json"
        )
        cases = (
            "fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json",
            str(checked_in_runbook),
            "/ops/release/fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, config_path in enumerate(cases):
                with self.subTest(config_path=config_path):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["canary_summaries"][0]["config_path"] = config_path
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"template-canary-config-{offset}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertEqual(codes, {"evidence.repository_canary_config"})

    def test_compact_trust_bundle_path_blocks_checked_in_templates(self):
        checked_in_bundle = (
            REPO_ROOT
            / "fixtures"
            / "iso20022"
            / "trust_bundles"
            / "swift_cbpr_plus.preprod.example.json"
        )
        cases = (
            "fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json",
            str(checked_in_bundle),
            "/ops/release/fixtures/iso20022/trust_bundles/swift_cbpr_plus.preprod.example.json",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, bundle_path in enumerate(cases):
                with self.subTest(bundle_path=bundle_path):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0]["profiles"][0]["path"] = bundle_path
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"template-trust-bundle-{offset}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertEqual(codes, {"trust.repository_trust_bundle"})

    def test_compact_trust_summary_verified_at_is_rechecked_by_readiness(self):
        cases = [
            ("naive", "2026-06-04T00:00:00", "verified_at must include a timezone"),
            ("malformed", "not-a-timestamp", "verified_at must be an ISO 8601 timestamp"),
            ("future", "2999-01-01T00:00:00+00:00", "verified_at must not be in the future"),
            ("control", "2026-06-04T00:00:00+00:00\nbad", "verified_at must not contain control characters"),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, verified_at, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0]["verified_at"] = verified_at
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trust-{name}-verified-at.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_timestamp_helper_rejects_unicode_format_controls_without_echo(self):
        hidden = "\u202ereadiness-timestamp-leak"

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._require_timestamp(
                {"verified_at": "2026-06-04T00:00:00+00:00" + hidden},
                "verified_at",
                "summary",
            )

        message = str(caught.exception)
        self.assertIn("control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("readiness-timestamp-leak", message)

    def test_overlong_compact_timestamps_are_rejected_without_echo(self):
        def set_nested(value, parts):
            target = value
            for part in parts[:-1]:
                target = target[part]
            target[parts[-1]] = hidden

        hidden = "2" * (READINESS.MAX_TIMESTAMP_CHARS + 1)
        cases = (
            (
                "xsd-verified-at",
                "xsd",
                ("verified_at",),
                "verified_at must be no longer than 128 characters",
            ),
            (
                "evidence-verified-at",
                "evidence",
                ("verified_at",),
                "verified_at must be no longer than 128 characters",
            ),
            (
                "canary-started-at",
                "evidence",
                ("canary_summaries", 0, "started_at"),
                "started_at must be no longer than 128 characters",
            ),
            (
                "canary-stage-finished-at",
                "evidence",
                ("canary_summaries", 0, "stage_windows", 0, "finished_at"),
                "finished_at must be no longer than 128 characters",
            ),
            (
                "trust-verified-at",
                "evidence",
                ("trust_summaries", 0, "verified_at"),
                "verified_at must be no longer than 128 characters",
            ),
            (
                "trust-source-retrieved-at",
                "evidence",
                ("trust_summaries", 0, "profiles", 0, "source", "retrieved_at"),
                "source.retrieved_at must be no longer than 128 characters",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, target_name, parts, message in cases:
                with self.subTest(name=name):
                    xsd_path = xsd_summary
                    evidence_path = evidence_summary
                    if target_name == "xsd":
                        body = json.loads(xsd_summary.read_text(encoding="utf-8"))
                        set_nested(body, parts)
                        refresh_digest(body)
                        xsd_path = write_json(root / f"{name}.summary.json", body)
                    else:
                        body = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        set_nested(body, parts)
                        refresh_digest(body)
                        evidence_path = write_json(root / f"{name}.summary.json", body)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_path), "--evidence-summary", str(evidence_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)

    def test_compact_trust_source_is_required_and_rechecked_by_readiness(self):
        def set_source(evidence, value):
            evidence["trust_summaries"][0]["profiles"][0]["source"] = value

        def set_source_field(evidence, key, value):
            evidence["trust_summaries"][0]["profiles"][0]["source"][key] = value

        long_host = ".".join(["a" * 63] * 4)
        long_url = "https://pki.example.com/" + ("a" * READINESS.MAX_SOURCE_URL_CHARS)
        cases = (
            ("missing-source", lambda evidence: evidence["trust_summaries"][0]["profiles"][0].pop("source"), "source must be a JSON object"),
            ("non-object-source", lambda evidence: set_source(evidence, []), "source must be a JSON object"),
            ("unknown-source-key", lambda evidence: set_source_field(evidence, "unexpected", "value"), "source contains unknown keys"),
            ("missing-authority", lambda evidence: evidence["trust_summaries"][0]["profiles"][0]["source"].pop("authority"), "source.authority must be a non-empty string"),
            ("empty-authority", lambda evidence: set_source_field(evidence, "authority", ""), "source.authority must be a non-empty string"),
            ("numeric-authority", lambda evidence: set_source_field(evidence, "authority", 7), "source.authority must be a non-empty string"),
            ("padded-authority", lambda evidence: set_source_field(evidence, "authority", " Example Rail PKI"), "source.authority must not have surrounding whitespace"),
            ("control-authority", lambda evidence: set_source_field(evidence, "authority", "Example\nRail PKI"), "source.authority must not contain control characters"),
            ("missing-version", lambda evidence: evidence["trust_summaries"][0]["profiles"][0]["source"].pop("version"), "source.version must be a non-empty string"),
            ("empty-version", lambda evidence: set_source_field(evidence, "version", ""), "source.version must be a non-empty string"),
            ("numeric-version", lambda evidence: set_source_field(evidence, "version", 2026), "source.version must be a non-empty string"),
            ("padded-version", lambda evidence: set_source_field(evidence, "version", "2026-Q2 "), "source.version must not have surrounding whitespace"),
            ("control-version", lambda evidence: set_source_field(evidence, "version", "2026-Q2\n"), "source.version must not contain control characters"),
            ("missing-url", lambda evidence: evidence["trust_summaries"][0]["profiles"][0]["source"].pop("url"), "source.url must be a non-empty string"),
            ("http-url", lambda evidence: set_source_field(evidence, "url", "http://pki.example.invalid/source"), "source.url must use HTTPS URL"),
            ("credential-url", lambda evidence: set_source_field(evidence, "url", "https://user:pass@pki.example.invalid/source"), "source.url must not contain credentials"),
            ("query-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/source?debug=true"), "source.url must not contain params, query, or fragment"),
            ("whitespace-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift cbpr/source"), "source.url must not contain whitespace"),
            ("invalid-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:abc/source"), "source.url has invalid port"),
            ("empty-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:/source"), "source.url must not include an empty port"),
            ("zero-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:0/source"), "source.url port must be positive"),
            ("leading-zero-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:08443/source"), "source.url port must not contain leading zeros"),
            ("out-of-range-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:99999/source"), "source.url has invalid port"),
            ("default-port-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid:443/source"), "source.url must not explicitly specify the default port"),
            ("long-url", lambda evidence: set_source_field(evidence, "url", long_url), "source.url must be no longer than 2048 characters"),
            ("uppercase-host-url", lambda evidence: set_source_field(evidence, "url", "https://PKI.example.invalid/source"), "source.url host must be lowercase"),
            ("trailing-dot-host-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid./source"), "source.url host must not end with a dot"),
            ("empty-label-host-url", lambda evidence: set_source_field(evidence, "url", "https://pki..example.invalid/source"), "source.url host must not contain empty labels"),
            ("long-host-url", lambda evidence: set_source_field(evidence, "url", f"https://{long_host}/source"), "source.url host must be at most 253 characters"),
            ("hyphen-edge-host-url", lambda evidence: set_source_field(evidence, "url", "https://-pki.example.invalid/source"), "source.url host labels must not start or end with hyphen"),
            ("underscore-host-url", lambda evidence: set_source_field(evidence, "url", "https://pki._tcp.example.invalid/source"), "source.url host labels must use lowercase ASCII letters, digits, or hyphens"),
            ("encoded-host-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example%2einvalid/source"), "source.url host must not contain percent escapes"),
            ("numeric-spoof-host-url", lambda evidence: set_source_field(evidence, "url", "https://123.000.000.001/source"), "source.url numeric host labels must be a valid IP address"),
            ("dot-segment-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/../source"), "source.url path must not contain dot segments"),
            ("empty-segment-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift//source"), "source.url path must not contain empty segments"),
            ("encoded-dot-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/%2e%2e/source"), "source.url path must not contain encoded dot or separator characters"),
            ("encoded-slash-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%2fsource"), "source.url path must not contain encoded dot or separator characters"),
            ("encoded-percent-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%252fsource"), "source.url path must not contain encoded percent characters"),
            ("semicolon-path-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/sources;debug/source"), "source.url path must not contain semicolon parameters"),
            ("encoded-semicolon-path-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/sources%3bdebug/source"), "source.url path must not contain encoded semicolon parameters"),
            ("encoded-delimiter-path-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/sources%23debug/source"), "source.url path must not contain encoded URL delimiter characters"),
            ("backslash-path-url", lambda evidence: set_source_field(evidence, "url", r"https://pki.example.invalid/sources\source"), "source.url path must use forward slashes"),
            ("encoded-space-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%20source"), "source.url must not contain percent-encoded control or space characters"),
            ("encoded-nul-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%00source"), "source.url must not contain percent-encoded control or space characters"),
            ("encoded-del-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%7fsource"), "source.url must not contain percent-encoded control or space characters"),
            ("malformed-percent-url", lambda evidence: set_source_field(evidence, "url", "https://pki.example.invalid/swift%zzsource"), "source.url must not contain malformed percent escapes"),
            ("localhost-url", lambda evidence: set_source_field(evidence, "url", "https://localhost/source"), "source.url must not use localhost"),
            ("local-ip-url", lambda evidence: set_source_field(evidence, "url", "https://127.0.0.1/source"), "source.url must not use local, private, or reserved IP addresses"),
            ("rebinding-host-url", lambda evidence: set_source_field(evidence, "url", "https://127.0.0.1.nip.io/source"), "source.url must not use local/private rebinding hostnames"),
            ("legacy-ipv4-url", lambda evidence: set_source_field(evidence, "url", "https://0x7f000001/source"), "source.url host must not use legacy IPv4 numeric notation"),
            ("embedded-ipv4-url", lambda evidence: set_source_field(evidence, "url", "https://[64:ff9b::7f00:1]/source"), "source.url must not embed local, private, or reserved IPv4 addresses"),
            ("missing-retrieved-at", lambda evidence: evidence["trust_summaries"][0]["profiles"][0]["source"].pop("retrieved_at"), "source.retrieved_at must be a non-empty string"),
            ("naive-retrieved-at", lambda evidence: set_source_field(evidence, "retrieved_at", "2026-06-04T00:00:00"), "source.retrieved_at must include a timezone"),
            ("malformed-retrieved-at", lambda evidence: set_source_field(evidence, "retrieved_at", "not-a-timestamp"), "source.retrieved_at must be an ISO 8601 timestamp"),
            ("future-retrieved-at", lambda evidence: set_source_field(evidence, "retrieved_at", "2999-01-01T00:00:00+00:00"), "source.retrieved_at must not be in the future"),
            ("control-retrieved-at", lambda evidence: set_source_field(evidence, "retrieved_at", "2026-06-04T00:00:00+00:00\nbad"), "source.retrieved_at must not contain control characters"),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"trust-source-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_rejected_compact_trust_source_url_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            secret_url = "https://pki.example.invalid/source?token=readiness-source-secret"
            evidence["trust_summaries"][0]["profiles"][0]["source"]["url"] = secret_url
            refresh_digest(evidence)
            mutated_path = write_json(root / "trust-source-secret.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("source.url", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("readiness-source-secret", stderr)

    def test_stale_compact_trust_source_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["profiles"][0]["source"][
                "retrieved_at"
            ] = "2000-01-01T00:00:00+00:00"
            refresh_digest(evidence)
            mutated_path = write_json(root / "stale-trust-source.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(mutated_path),
                    "--max-trust-source-age-days",
                    "1",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.source_stale", codes)

    def test_compact_trust_profile_emittable_must_match_source_policy(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            trust = evidence["trust_summaries"][0]
            trust["max_source_age_days"] = 1
            trust["profiles"][0]["source"]["retrieved_at"] = "2020-01-01T00:00:00+00:00"
            refresh_digest(evidence)
            mutated_path = write_json(root / "trust-emittable-drift.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(mutated_path),
                    "--max-trust-source-age-days",
                    "36500",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_json_emittable_drift", codes)

    def test_placeholder_compact_trust_source_blocks_readiness(self):
        def set_source_field(evidence, key, value):
            evidence["trust_summaries"][0]["profiles"][0]["source"][key] = value

        cases = (
            (
                "authority",
                lambda evidence: set_source_field(
                    evidence,
                    "authority",
                    "Swift operator PKI placeholder",
                ),
            ),
            (
                "dummy-authority",
                lambda evidence: set_source_field(
                    evidence,
                    "authority",
                    "Dummy Swift operator PKI",
                ),
            ),
            (
                "fake-version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "fake-v1",
                ),
            ),
            (
                "sample-authority",
                lambda evidence: set_source_field(
                    evidence,
                    "authority",
                    "Sample Swift operator PKI",
                ),
            ),
            (
                "split-sample-authority",
                lambda evidence: set_source_field(
                    evidence,
                    "authority",
                    "Sam ple Swift operator PKI",
                ),
            ),
            (
                "version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "replace-before-production",
                ),
            ),
            (
                "spaced-version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "replace before production",
                ),
            ),
            (
                "underscore-version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "replace_before_production",
                ),
            ),
            (
                "template-version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "template-v1",
                ),
            ),
            (
                "split-template-version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "tem plate-v1",
                ),
            ),
            (
                "url",
                lambda evidence: set_source_field(
                    evidence,
                    "url",
                    "https://pki.swift.example.invalid/iso20022",
                ),
            ),
            (
                "reserved-url",
                lambda evidence: set_source_field(
                    evidence,
                    "url",
                    "https://pki.swift.example.com/iso20022",
                ),
            ),
            (
                "reserved-tld-url",
                lambda evidence: set_source_field(
                    evidence,
                    "url",
                    "https://pki.swift.example/iso20022",
                ),
            ),
            (
                "template-canary-url",
                lambda evidence: set_source_field(
                    evidence,
                    "url",
                    "https://pki.swift.operator-canary.bank/iso20022",
                ),
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"placeholder-trust-source-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("trust.source_placeholder", codes)

    def test_malformed_compact_summary_digests_are_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (["canary_summaries", 0], "A" * 64),
                (["trust_summaries", 0], "0" * 63),
            )
            for path_parts, digest in cases:
                with self.subTest(path_parts=path_parts):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = evidence
                    for part in path_parts:
                        target = target[part]
                    target["summary_sha256"] = digest
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"malformed-compact-digest-{path_parts[0]}.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("summary_sha256 must be a lowercase SHA-256 digest", stderr)

    def test_compact_summary_reference_digests_reject_all_zero_placeholders(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("canary", ["canary_summaries", 0]),
                ("trust", ["trust_summaries", 0]),
            )
            for name, path_parts in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    target = evidence
                    for part in path_parts:
                        target = target[part]
                    actual_digest = target["summary_sha256"]
                    target["summary_sha256"] = "0" * 64
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"zero-compact-{name}-digest.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("summary_sha256 must not be all zero", stderr)
                    self.assertNotIn(actual_digest, stderr)
                    self.assertNotIn("mismatch", stderr)

    def test_compact_trust_profile_json_digest_rejects_all_zero(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["profile_json_sha256"] = "0" * 64
            refresh_digest(evidence)
            mutated_path = write_json(root / "zero-profile-json.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("profile_json_sha256 must not be all zero", stderr)

    def test_compact_canary_timestamp_window_is_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            missing = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del missing["canary_summaries"][0]["started_at"]
            cases.append((missing, "started_at must be a non-empty string"))
            naive = json.loads(evidence_summary.read_text(encoding="utf-8"))
            naive["canary_summaries"][0]["started_at"] = "2026-06-04T00:00:00"
            cases.append((naive, "started_at must include a timezone"))
            future = json.loads(evidence_summary.read_text(encoding="utf-8"))
            future["canary_summaries"][0]["finished_at"] = "2999-01-01T00:00:00+00:00"
            cases.append((future, "finished_at must not be in the future"))
            reversed_window = json.loads(evidence_summary.read_text(encoding="utf-8"))
            reversed_window["canary_summaries"][0]["started_at"] = "2026-06-04T00:00:02+00:00"
            reversed_window["canary_summaries"][0]["finished_at"] = "2026-06-04T00:00:01+00:00"
            cases.append((reversed_window, "finished_at must not be before started_at"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"canary-time-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_stage_windows_are_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            missing = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del missing["canary_summaries"][0]["stage_windows"][0]["started_at"]
            cases.append((missing, "started_at must be a non-empty string"))
            future = json.loads(evidence_summary.read_text(encoding="utf-8"))
            future["canary_summaries"][0]["stage_windows"][0]["finished_at"] = "2999-01-01T00:00:00+00:00"
            cases.append((future, "finished_at must not be in the future"))
            reversed_window = json.loads(evidence_summary.read_text(encoding="utf-8"))
            reversed_window["canary_summaries"][0]["stage_windows"][0]["started_at"] = "2026-06-04T00:00:01+00:00"
            reversed_window["canary_summaries"][0]["stage_windows"][0]["finished_at"] = "2026-06-04T00:00:00+00:00"
            cases.append((reversed_window, "finished_at must not be before started_at"))
            outside_canary = json.loads(evidence_summary.read_text(encoding="utf-8"))
            outside_canary["canary_summaries"][0]["stage_windows"][0]["started_at"] = "2026-06-03T23:59:59+00:00"
            cases.append((outside_canary, "timestamp window must be inside canary window"))
            overlapping = json.loads(evidence_summary.read_text(encoding="utf-8"))
            overlapping["canary_summaries"][0]["stage_windows"][1]["started_at"] = (
                "2026-06-04T00:00:00.100000+00:00"
            )
            cases.append(
                (
                    overlapping,
                    "started_at must not be before previous stage finished_at",
                )
            )
            mismatch = json.loads(evidence_summary.read_text(encoding="utf-8"))
            mismatch["canary_summaries"][0]["stage_windows"][0]["name"] = "extra"
            cases.append((mismatch, "stage_windows must match stage_names"))
            reordered = json.loads(evidence_summary.read_text(encoding="utf-8"))
            windows = reordered["canary_summaries"][0]["stage_windows"]
            windows[0]["name"], windows[1]["name"] = windows[1]["name"], windows[0]["name"]
            cases.append((reordered, "stage_windows must match stage_names"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-window-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_stage_dry_run_is_rechecked_by_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            missing = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del missing["canary_summaries"][0]["stage_dry_run"]
            cases.append((missing, "stage_dry_run must be a JSON array"))
            non_array = json.loads(evidence_summary.read_text(encoding="utf-8"))
            non_array["canary_summaries"][0]["stage_dry_run"] = "false,false,false"
            cases.append((non_array, "stage_dry_run must be a JSON array"))
            short = json.loads(evidence_summary.read_text(encoding="utf-8"))
            short["canary_summaries"][0]["stage_dry_run"] = [False, False]
            cases.append((short, "stage_dry_run must match stage_names length"))
            long = json.loads(evidence_summary.read_text(encoding="utf-8"))
            long["canary_summaries"][0]["stage_dry_run"] = [
                False,
                False,
                False,
                False,
            ]
            cases.append((long, "stage_dry_run must match stage_names length"))
            non_boolean = json.loads(evidence_summary.read_text(encoding="utf-8"))
            non_boolean["canary_summaries"][0]["stage_dry_run"][1] = "false"
            cases.append((non_boolean, "stage_dry_run[1] must be a boolean"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-dry-run-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_stage_names_are_unique_and_supported(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            duplicate = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = duplicate["canary_summaries"][0]
            canary["stage_names"] = ["rail", "rail", "notary", "verify"]
            canary["stage_windows"].insert(1, dict(canary["stage_windows"][0]))
            cases.append((duplicate, "stage_names must not contain duplicates", None))
            unsupported = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = unsupported["canary_summaries"][0]
            hidden_stage = "diagnostic"
            canary["stage_names"].append(hidden_stage)
            extra_window = dict(canary["stage_windows"][0])
            extra_window["name"] = hidden_stage
            canary["stage_windows"].append(extra_window)
            cases.append((unsupported, "stage_names contains unsupported stages", hidden_stage))
            for offset, (body, message, hidden) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-names-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    if hidden is not None:
                        self.assertNotIn(hidden, stderr)

    def test_compact_stage_names_must_follow_canary_order(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = evidence["canary_summaries"][0]
            canary["stage_names"] = ["notary", "rail", "verify"]
            windows_by_name = {window["name"]: window for window in canary["stage_windows"]}
            canary["stage_windows"] = [
                {
                    **windows_by_name["notary"],
                    "started_at": "2026-06-04T00:00:00+00:00",
                    "finished_at": "2026-06-04T00:00:00.200000+00:00",
                },
                {
                    **windows_by_name["rail"],
                    "started_at": "2026-06-04T00:00:00.200000+00:00",
                    "finished_at": "2026-06-04T00:00:00.400000+00:00",
                },
                windows_by_name["verify"],
            ]
            refresh_digest(evidence)
            mutated_path = write_json(root / "stage-order.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("stage_names must follow canary order", stderr)

    def test_canary_without_explicit_policy_proof_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["canary_summaries"][0]["require_explicit_policy"] = False
            refresh_digest(evidence)
            mutated_path = write_json(root / "implicit-policy-canary.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_implicit_policy", codes)

    def test_missing_nested_receipt_policy_flag_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-insecure",
                    ["canary_summaries", 0, "receipt_summary"],
                    "allow_insecure_http",
                    "receipt_summary.allow_insecure_http must be a boolean",
                ),
                (
                    "archive-insecure",
                    ["receipt_verification"],
                    "allow_insecure_http",
                    "receipt_verification.allow_insecure_http must be a boolean",
                ),
                (
                    "canary-default-profile",
                    ["canary_summaries", 0, "receipt_summary"],
                    "allow_default_profile",
                    "receipt_summary.allow_default_profile must be a boolean",
                ),
                (
                    "archive-default-profile",
                    ["receipt_verification"],
                    "allow_default_profile",
                    "receipt_verification.allow_default_profile must be a boolean",
                ),
            )
            for name, path_parts, flag, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for part in path_parts:
                        receipt_summary = receipt_summary[part]
                    del receipt_summary[flag]
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"missing-{name}-receipt-policy-flag.summary.json",
                        evidence,
                    )

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_receipt_summary_version_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-missing-version",
                    ["canary_summaries", 0, "receipt_summary"],
                    lambda summary: summary.pop("version"),
                    "evidence.receipt_summary_version_unsupported",
                ),
                (
                    "archive-missing-version",
                    ["receipt_verification"],
                    lambda summary: summary.pop("version"),
                    "evidence.archive_receipt_summary_version_unsupported",
                ),
                (
                    "canary-unsupported-version",
                    ["canary_summaries", 0, "receipt_summary"],
                    lambda summary: summary.__setitem__(
                        "version",
                        READINESS.RECEIPT_SUMMARY_VERSION + 1,
                    ),
                    "evidence.receipt_summary_version_unsupported",
                ),
                (
                    "archive-unsupported-version",
                    ["receipt_verification"],
                    lambda summary: summary.__setitem__(
                        "version",
                        READINESS.RECEIPT_SUMMARY_VERSION + 1,
                    ),
                    "evidence.archive_receipt_summary_version_unsupported",
                ),
            )
            for name, path_parts, mutate, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for part in path_parts:
                        receipt_summary = receipt_summary[part]
                    mutate(receipt_summary)
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_missing_canary_stage_and_receipt_kind_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = evidence["canary_summaries"][0]
            canary["stage_names"] = ["rail", "notary"]
            canary["stage_dry_run"] = canary["stage_dry_run"][:2]
            canary["stage_windows"] = [
                window for window in canary["stage_windows"] if window["name"] in {"rail", "notary"}
            ]
            canary["receipt_summary"] = receipt_verification_summary(
                ["iso-rail-gateway"],
                allow_legacy_colr007=True,
            )
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-canary.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.missing_canary_stages", codes)
            self.assertIn("evidence.missing_receipt_kinds", codes)
            self.assertIn("evidence.receipts_allow_legacy_colr007", codes)

    def test_canary_stage_receipt_kinds_must_match_stage_names(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_evidence_summary(
                root / "evidence",
                direct_receipts=False,
            )
            cases = (
                (
                    "missing-rail-receipt",
                    ["rail", "verify"],
                    receipt_verification_summary(["iso-audit-notary"]),
                    "evidence.stage_receipt_kind_missing",
                ),
                (
                    "extra-notary-receipt",
                    ["rail", "verify"],
                    receipt_verification_summary(),
                    "evidence.stage_receipt_kind_unexecuted",
                ),
            )
            for name, stage_names, receipt_summary, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    canary = evidence["canary_summaries"][0]
                    dry_run_by_stage = dict(
                        zip(canary["stage_names"], canary["stage_dry_run"], strict=True)
                    )
                    canary["stage_names"] = stage_names
                    canary["stage_dry_run"] = [
                        dry_run_by_stage[stage_name] for stage_name in stage_names
                    ]
                    canary["stage_windows"] = [
                        window
                        for window in canary["stage_windows"]
                        if window["name"] in set(stage_names)
                    ]
                    canary["receipt_summary"] = receipt_summary
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                            "--allow-canary-stage-receipts-only",
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = evidence["canary_summaries"][0]
            canary["stage_dry_run"] = [True, False, False]
            canary["receipt_summary"] = receipt_verification_summary()
            evidence["policy"]["allow_dry_run"] = True
            refresh_digest(evidence)
            dry_run_path = write_json(root / "dry-run-rail-receipt.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(dry_run_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.stage_receipt_kind_unexecuted", codes)
            self.assertIn("evidence.policy.allow_dry_run", codes)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = evidence["canary_summaries"][0]
            canary["stage_dry_run"] = [True, False, False]
            canary["receipt_summary"] = receipt_verification_summary(
                ["iso-audit-notary"],
            )
            evidence["policy"]["allow_dry_run"] = True
            refresh_digest(evidence)
            dry_run_executed_receipts_path = write_json(
                root / "dry-run-rail-executed-receipts.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(dry_run_executed_receipts_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.policy.allow_dry_run", codes)
            self.assertIn("evidence.missing_receipt_kinds", codes)
            self.assertNotIn("evidence.stage_receipt_kind_missing", codes)
            self.assertNotIn("evidence.stage_receipt_kind_unexecuted", codes)

    def test_default_profile_receipt_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                receipt_summary["allow_default_profile"] = True
                receipt_summary["receipts"][1]["profile"] = None
                refresh_digest(receipt_summary)
            refresh_digest(evidence)
            mutated_path = write_json(root / "default-profile-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.receipts_allow_default_profile", codes)
            self.assertIn("evidence.archive_receipts_allow_default_profile", codes)

            missing_profile = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for receipt_summary in (
                missing_profile["canary_summaries"][0]["receipt_summary"],
                missing_profile["receipt_verification"],
            ):
                receipt_summary["allow_default_profile"] = True
                receipt_summary["receipts"][1].pop("profile")
                refresh_digest(receipt_summary)
            refresh_digest(missing_profile)
            missing_profile_path = write_json(
                root / "missing-default-profile-receipts.summary.json",
                missing_profile,
            )

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(missing_profile_path),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.receipt_metadata_invalid", codes)
            self.assertIn("evidence.archive_receipt_metadata_invalid", codes)

    def test_nonproduction_trust_policy_and_zero_pins_block_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["embedded_signature_policy"] = "record-only"
            profile["signature_public_key_pin_count"] = 0
            profile["x509_trust_anchor_pin_count"] = 0
            profile["x509_trust_anchor_der"] = []
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-trust.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("trust.policy_not_require_verified", codes)
            self.assertIn("trust.no_signature_or_x509_pins", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertNotIn("record-only", blocker_text)

            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["profiles"][0][
                "embedded_signature_policy"
            ] = "diagnostic-only"
            refresh_digest(evidence)
            mutated_path = write_json(root / "unsupported-trust-policy.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("trust.policy_unsupported", codes)
            self.assertNotIn("trust.policy_not_require_verified", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertNotIn("diagnostic-only", blocker_text)

    def test_trust_profile_blocker_messages_do_not_echo_profile_id(self):
        hidden_profile_id = "hidden-trust-profile"

        def retarget_trust_profile(evidence):
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = hidden_profile_id
            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                for receipt in receipt_summary["receipts"]:
                    if receipt["receipt_kind"] == "iso-rail-gateway":
                        receipt["profile"] = hidden_profile_id
                refresh_digest(receipt_summary)
            return profile

        cases = (
            (
                "missing-source",
                "trust.source_missing",
                lambda profile: profile.__setitem__("source", None),
            ),
            (
                "no-pins",
                "trust.no_signature_or_x509_pins",
                lambda profile: (
                    profile.__setitem__("signature_public_key_pin_count", 0),
                    profile.__setitem__("x509_trust_anchor_pin_count", 0),
                    profile.__setitem__("x509_trust_anchor_der", []),
                ),
            ),
            (
                "unsupported-policy",
                "trust.policy_unsupported",
                lambda profile: profile.__setitem__(
                    "embedded_signature_policy",
                    "diagnostic-only",
                ),
            ),
            (
                "record-only-policy",
                "trust.policy_not_require_verified",
                lambda profile: profile.__setitem__(
                    "embedded_signature_policy",
                    "record-only",
                ),
            ),
            (
                "crl-not-required",
                "trust.crl_revocation_not_required",
                lambda profile: profile.__setitem__(
                    "x509_require_crl_revocation_check",
                    False,
                ),
            ),
            (
                "missing-crl-material",
                "trust.no_crl_revocation_material",
                lambda profile: (
                    profile.__setitem__("x509_require_crl_revocation_check", True),
                    profile.__setitem__("x509_crl_count", 0),
                    profile.__setitem__("x509_crl_der", []),
                ),
            ),
            (
                "ocsp-not-required",
                "trust.ocsp_revocation_not_required",
                lambda profile: profile.__setitem__(
                    "x509_require_ocsp_revocation_check",
                    False,
                ),
            ),
            (
                "missing-ocsp-material",
                "trust.no_ocsp_revocation_material",
                lambda profile: (
                    profile.__setitem__("x509_require_ocsp_revocation_check", True),
                    profile.__setitem__("x509_ocsp_response_count", 0),
                    profile.__setitem__("x509_ocsp_response_der", []),
                ),
            ),
        )

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, expected_code, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    profile = retarget_trust_profile(evidence)
                    mutate(profile)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"profile-id-redaction-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(expected_code, codes)
                    blocker_text = "\n".join(blocker["message"] for blocker in blockers)
                    self.assertNotIn(hidden_profile_id, blocker_text)

    def test_non_ascii_compact_trust_policy_is_rejected_without_echo(self):
        hidden = "require-verif\u0456ed"
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["profiles"][0][
                "embedded_signature_policy"
            ] = hidden
            refresh_digest(evidence)
            mutated_path = write_json(root / "nonascii-trust-policy.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("embedded_signature_policy must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("uses unsupported policy", stderr)

    def test_overlong_compact_trust_identity_values_are_rejected_without_echo(self):
        cases = (
            (
                "profile-id",
                "profile_id",
                "a" * (READINESS.MAX_PROFILE_ID_CHARS + 1),
                "profile_id must be no longer than 128 characters",
                "trust profile",
            ),
            (
                "policy",
                "embedded_signature_policy",
                "record-" + ("a" * READINESS.MAX_TRUST_POLICY_CHARS),
                "embedded_signature_policy must be no longer than 128 characters",
                "uses unsupported policy",
            ),
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for name, field, hidden, message, forbidden in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["trust_summaries"][0]["profiles"][0][field] = hidden
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"overlong-{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(forbidden, stderr)

    def test_missing_trust_revocation_proof_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "x509_require_crl_revocation_check",
                    "x509_require_crl_revocation_check must be a boolean",
                ),
                ("x509_crl_count", "x509_crl_count must be a non-negative integer"),
                (
                    "x509_require_ocsp_revocation_check",
                    "x509_require_ocsp_revocation_check must be a boolean",
                ),
                (
                    "x509_ocsp_response_count",
                    "x509_ocsp_response_count must be a non-negative integer",
                ),
            )
            for flag, message in cases:
                with self.subTest(flag=flag):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    profile = evidence["trust_summaries"][0]["profiles"][0]
                    del profile[flag]
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"missing-{flag}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_trust_der_proofs_are_required_and_shape_checked(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "missing-crl-der",
                    lambda profile: profile.pop("x509_crl_der"),
                    "x509_crl_der must be a JSON array",
                ),
                (
                    "too-many-anchor-der",
                    lambda profile: profile.__setitem__(
                        "x509_trust_anchor_der",
                        ["not-an-object"] * (READINESS.MAX_TRUST_DER_BLOBS + 1),
                    ),
                    "x509_trust_anchor_der must not contain more than",
                ),
                (
                    "too-many-revoked-der",
                    lambda profile: profile.__setitem__(
                        "revoked_certificate_der",
                        ["not-an-object"] * (READINESS.MAX_TRUST_DER_BLOBS + 1),
                    ),
                    "revoked_certificate_der must not contain more than",
                ),
                (
                    "too-many-crl-der",
                    lambda profile: profile.__setitem__(
                        "x509_crl_der",
                        ["not-an-object"] * (READINESS.MAX_TRUST_DER_BLOBS + 1),
                    ),
                    "x509_crl_der must not contain more than",
                ),
                (
                    "too-many-ocsp-der",
                    lambda profile: profile.__setitem__(
                        "x509_ocsp_response_der",
                        ["not-an-object"] * (READINESS.MAX_TRUST_DER_BLOBS + 1),
                    ),
                    "x509_ocsp_response_der must not contain more than",
                ),
                (
                    "bad-crl-digest",
                    lambda profile: profile["x509_crl_der"][0].__setitem__(
                        "sha256",
                        "not-a-digest",
                    ),
                    "sha256 must be a lowercase SHA-256 digest",
                ),
                (
                    "all-zero-crl-digest",
                    lambda profile: profile["x509_crl_der"][0].__setitem__(
                        "sha256",
                        "0" * 64,
                    ),
                    "sha256 must not be all zero",
                ),
                (
                    "zero-crl-byte-len",
                    lambda profile: profile["x509_crl_der"][0].__setitem__(
                        "byte_len",
                        0,
                    ),
                    "byte_len must be a positive integer",
                ),
                (
                    "duplicate-ocsp-digest",
                    lambda profile: profile["x509_ocsp_response_der"].append(
                        dict(profile["x509_ocsp_response_der"][0])
                    ),
                    "duplicates",
                ),
                (
                    "crl-count-drift",
                    lambda profile: profile.__setitem__("x509_crl_der", []),
                    "x509_crl_der length does not match x509_crl_count",
                ),
                (
                    "ocsp-count-drift",
                    lambda profile: profile.__setitem__("x509_ocsp_response_der", []),
                    "x509_ocsp_response_der length does not match x509_ocsp_response_count",
                ),
                (
                    "anchor-count-drift",
                    lambda profile: profile.__setitem__(
                        "x509_trust_anchor_pin_count",
                        0,
                    ),
                    "x509_trust_anchor_der length exceeds x509_trust_anchor_pin_count",
                ),
            )
            for name, mutate, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    profile = evidence["trust_summaries"][0]["profiles"][0]
                    mutate(profile)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"bad-der-{name}.summary.json", evidence)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_compact_trust_der_proofs_cannot_be_reused_across_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "trusted-revoked",
                    lambda profile: profile["revoked_certificate_der"][0].update(
                        profile["x509_trust_anchor_der"][0]
                    ),
                ),
                (
                    "crl-ocsp",
                    lambda profile: profile["x509_ocsp_response_der"][0].update(
                        profile["x509_crl_der"][0]
                    ),
                ),
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    profile = evidence["trust_summaries"][0]["profiles"][0]
                    mutate(profile)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"reused-trust-der-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("trust.der_proof_reused_across_roles", codes)

    def test_trust_verified_bundle_count_must_match_profiles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["verified_bundles"] = 2
            refresh_digest(evidence)
            mutated_path = write_json(root / "mismatched-trust-count.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_count_mismatch", codes)

    def test_trust_profile_ids_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            trust_summary = evidence["trust_summaries"][0]
            trust_summary["profiles"].append(dict(trust_summary["profiles"][0]))
            trust_summary["verified_bundles"] = 2
            refresh_digest(evidence)
            mutated_path = write_json(root / "duplicate-trust-profile.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_id_duplicate", codes)

    def test_trust_bundle_digest_is_required_and_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            malformed = json.loads(evidence_summary.read_text(encoding="utf-8"))
            del malformed["trust_summaries"][0]["profiles"][0]["bundle_sha256"]
            refresh_digest(malformed)
            malformed_path = write_json(root / "missing-bundle-digest.summary.json", malformed)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(malformed_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("bundle_sha256 must be a lowercase SHA-256 digest", stderr)

            zero = json.loads(evidence_summary.read_text(encoding="utf-8"))
            zero["trust_summaries"][0]["profiles"][0]["bundle_sha256"] = "0" * 64
            refresh_digest(zero)
            zero_path = write_json(root / "zero-bundle-digest.summary.json", zero)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(zero_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("bundle_sha256 must not be all zero", stderr)

            duplicate = json.loads(evidence_summary.read_text(encoding="utf-8"))
            trust_summary = duplicate["trust_summaries"][0]
            copied_profile = {
                **trust_summary["profiles"][0],
                "profile_id": "fedwire-funds",
                "rail": "fedwire-funds",
            }
            trust_summary["profiles"].append(copied_profile)
            trust_summary["verified_bundles"] = 2
            refresh_digest(duplicate)
            duplicate_path = write_json(root / "duplicate-bundle-digest.summary.json", duplicate)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(duplicate_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.bundle_digest_duplicate", codes)

    def test_trust_profile_json_and_bundle_digests_cannot_reuse_trust_material_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def profile_json_reuses_bundle(trust):
                trust["profile_json_sha256"] = trust["profiles"][0]["bundle_sha256"]

            def profile_json_reuses_der(trust):
                trust["profile_json_sha256"] = trust["profiles"][0]["x509_crl_der"][0][
                    "sha256"
                ]

            def bundle_reuses_der(trust):
                profile = trust["profiles"][0]
                profile["bundle_sha256"] = profile["x509_ocsp_response_der"][0][
                    "sha256"
                ]

            cases = (
                (
                    "profile-json-bundle",
                    profile_json_reuses_bundle,
                    "trust.profile_json_digest_matches_bundle",
                ),
                (
                    "profile-json-der",
                    profile_json_reuses_der,
                    "trust.profile_json_digest_matches_der_proof",
                ),
                (
                    "bundle-der",
                    bundle_reuses_der,
                    "trust.bundle_digest_matches_der_proof",
                ),
            )

            for name, mutate, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence["trust_summaries"][0])
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_trust_profiles_cannot_be_reused_across_compact_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_trust = json.loads(json.dumps(evidence["trust_summaries"][0]))
            copied_trust["path"] = "/ops/iso/copied-trust.summary.json"
            copied_trust["summary_sha256"] = "e" * 64
            evidence["trust_summaries"].append(copied_trust)
            refresh_digest(evidence)
            mutated_path = write_json(root / "reused-trust-profiles.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_id_reused", codes)
            self.assertIn("trust.bundle_path_reused", codes)
            self.assertIn("trust.bundle_digest_reused", codes)

    def test_trust_bundle_path_cannot_be_reused_inside_compact_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            trust_summary = evidence["trust_summaries"][0]
            copied_profile = json.loads(json.dumps(trust_summary["profiles"][0]))
            copied_profile["profile_id"] = "fedwire-funds"
            copied_profile["rail"] = "fedwire-funds"
            copied_profile["bundle_sha256"] = "b" * 64
            trust_summary["profiles"].append(copied_profile)
            trust_summary["verified_bundles"] = 2
            refresh_digest(evidence)
            mutated_path = write_json(root / "duplicate-trust-bundle-path.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.bundle_path_duplicate", codes)
            self.assertNotIn("trust.profile_id_duplicate", codes)
            self.assertNotIn("trust.bundle_digest_duplicate", codes)

    def test_trust_bundle_path_cannot_be_reused_across_relabelled_compact_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_trust = json.loads(json.dumps(evidence["trust_summaries"][0]))
            copied_trust["path"] = "/ops/iso/relabelled-trust-bundle-path.summary.json"
            copied_trust["summary_sha256"] = "e" * 64
            copied_trust["profile_json_sha256"] = "c" * 64
            profile = copied_trust["profiles"][0]
            profile["profile_id"] = "swift-cbpr-plus-two"
            profile["bundle_sha256"] = "b" * 64
            evidence["trust_summaries"].append(copied_trust)
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "relabelled-trust-bundle-path.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.bundle_path_reused", codes)
            self.assertNotIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_compact_summary_paths_cannot_reuse_json_material_paths(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def canary_summary_reuses_canary_receipt(evidence):
                evidence["canary_summaries"][0]["path"] = evidence[
                    "canary_summaries"
                ][0]["receipt_summary"]["receipts"][0]["path"]

            def trust_summary_reuses_archive_receipt(evidence):
                evidence["trust_summaries"][0]["path"] = evidence[
                    "receipt_verification"
                ]["receipts"][0]["path"]

            cases = (
                (
                    "canary-summary-config",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "config_path",
                        evidence["canary_summaries"][0]["path"],
                    ),
                ),
                (
                    "trust-summary-bundle",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][
                        0
                    ].__setitem__(
                        "path",
                        evidence["trust_summaries"][0]["path"],
                    ),
                ),
                (
                    "canary-summary-trust-bundle",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][
                        0
                    ].__setitem__(
                        "path",
                        evidence["canary_summaries"][0]["path"],
                    ),
                ),
                (
                    "trust-summary-canary-config",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "config_path",
                        evidence["trust_summaries"][0]["path"],
                    ),
                ),
                (
                    "canary-summary-canary-receipt",
                    canary_summary_reuses_canary_receipt,
                ),
                (
                    "trust-summary-archive-receipt",
                    trust_summary_reuses_archive_receipt,
                ),
            )
            for name, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"{name}-path-role.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(
                        "evidence.compact_json_artifact_path_role_reused",
                        codes,
                    )

    def test_compact_summary_paths_cannot_reuse_json_material_paths_across_evidence_summaries(self):
        def next_digest():
            nonlocal digest_counter
            digest_counter += 1
            return f"{digest_counter:064x}"

        def uniquify_second_summary(evidence, profile_id):
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = profile_id
            profile["bundle_sha256"] = next_digest()
            for role in READINESS.TRUST_DER_PROOF_FIELDS:
                for entry in profile[role]:
                    entry["sha256"] = next_digest()
            evidence["trust_summaries"][0]["profile_json_sha256"] = next_digest()

            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                for offset, receipt in enumerate(receipt_summary["receipts"]):
                    receipt["path"] = (
                        f"/ops/iso/{profile_id}/receipts/"
                        f"{receipt['receipt_kind']}.{offset}.receipt.json"
                    )
                    receipt["receipt_sha256"] = next_digest()
                    receipt["response_body_sha256"] = next_digest()
                    if receipt["receipt_kind"] == "iso-audit-notary":
                        receipt["anchor_path"] = (
                            f"/ops/iso/{profile_id}/notary/anchors/"
                            f"{offset}.notary.json"
                        )
                        receipt["store_dir"] = f"/ops/iso/{profile_id}/notary-store"
                        receipt["index_path"] = (
                            f"/ops/iso/{profile_id}/notary/messages.index.json"
                        )
                        receipt["anchor_sha256"] = next_digest()
                        receipt["index_sha256"] = next_digest()
                    else:
                        receipt["source_path"] = (
                            f"/ops/iso/{profile_id}/rail-inbox/{offset}.xml"
                        )
                        receipt["payload_sha256"] = next_digest()
                        receipt["profile"] = profile_id
                        receipt["rail_message_id"] = f"{profile_id}-{offset}"
                refresh_digest(receipt_summary)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            base_second = json.loads(evidence_two.read_text(encoding="utf-8"))

            def canary_summary_reused_as_canary_receipt(second, first):
                second["canary_summaries"][0]["path"] = first["canary_summaries"][0][
                    "receipt_summary"
                ]["receipts"][0]["path"]

            def trust_summary_reused_as_archive_receipt(second, first):
                second["trust_summaries"][0]["path"] = first["receipt_verification"][
                    "receipts"
                ][0]["path"]

            cases = (
                (
                    "canary-summary-as-config",
                    lambda second, first: second["canary_summaries"][0].__setitem__(
                        "config_path",
                        first["canary_summaries"][0]["path"],
                    ),
                ),
                (
                    "trust-summary-as-bundle",
                    lambda second, first: second["trust_summaries"][0]["profiles"][
                        0
                    ].__setitem__(
                        "path",
                        first["trust_summaries"][0]["path"],
                    ),
                ),
                (
                    "canary-summary-as-bundle",
                    lambda second, first: second["trust_summaries"][0]["profiles"][
                        0
                    ].__setitem__(
                        "path",
                        first["canary_summaries"][0]["path"],
                    ),
                ),
                (
                    "trust-summary-as-config",
                    lambda second, first: second["canary_summaries"][0].__setitem__(
                        "config_path",
                        first["trust_summaries"][0]["path"],
                    ),
                ),
                (
                    "canary-summary-as-canary-receipt",
                    canary_summary_reused_as_canary_receipt,
                ),
                (
                    "trust-summary-as-archive-receipt",
                    trust_summary_reused_as_archive_receipt,
                ),
            )
            for offset, (name, mutate) in enumerate(cases):
                with self.subTest(name=name):
                    digest_counter = 0xB000 + offset * 0x100
                    second = json.loads(json.dumps(base_second))
                    uniquify_second_summary(second, f"swift-cbpr-plus-path-{offset}")
                    mutate(second, first)
                    refresh_digest(second)
                    second_path = write_json(
                        root / f"cross-summary-json-path-role-{name}.summary.json",
                        second,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_one),
                            "--evidence-summary",
                            str(second_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(
                        "evidence.compact_json_artifact_path_role_reused",
                        codes,
                    )
                    self.assertNotIn("trust.profile_json_digest_reused", codes)
                    self.assertNotIn("trust.profile_id_reused", codes)
                    self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_trust_bundle_path_cannot_be_reused_across_evidence_summaries(self):
        def next_digest():
            nonlocal digest_counter
            digest_counter += 1
            return f"{digest_counter:064x}"

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second_profile_id = "swift-cbpr-plus-path-replay"
            profile = second["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = second_profile_id
            profile["path"] = first["trust_summaries"][0]["profiles"][0]["path"]
            digest_counter = 0xC000
            profile["bundle_sha256"] = next_digest()
            for role in READINESS.TRUST_DER_PROOF_FIELDS:
                for entry in profile[role]:
                    entry["sha256"] = next_digest()
            second["trust_summaries"][0]["profile_json_sha256"] = next_digest()
            for receipt_summary in (
                second["canary_summaries"][0]["receipt_summary"],
                second["receipt_verification"],
            ):
                for offset, receipt in enumerate(receipt_summary["receipts"]):
                    receipt["path"] = (
                        f"/ops/iso/{second_profile_id}/receipts/"
                        f"{receipt['receipt_kind']}.{offset}.receipt.json"
                    )
                    receipt["receipt_sha256"] = next_digest()
                    receipt["response_body_sha256"] = next_digest()
                    if receipt["receipt_kind"] == "iso-audit-notary":
                        receipt["anchor_path"] = (
                            f"/ops/iso/{second_profile_id}/notary/anchors/"
                            f"{offset}.notary.json"
                        )
                        receipt["store_dir"] = f"/ops/iso/{second_profile_id}/notary-store"
                        receipt["index_path"] = (
                            f"/ops/iso/{second_profile_id}/notary/messages.index.json"
                        )
                        receipt["anchor_sha256"] = next_digest()
                        receipt["index_sha256"] = next_digest()
                    else:
                        receipt["source_path"] = (
                            f"/ops/iso/{second_profile_id}/rail-inbox/{offset}.xml"
                        )
                        receipt["payload_sha256"] = next_digest()
                        receipt["profile"] = second_profile_id
                        receipt["rail_message_id"] = f"{second_profile_id}-{offset}"
                refresh_digest(receipt_summary)
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.bundle_path_reused", codes)
            self.assertNotIn(
                "evidence.compact_json_artifact_path_role_reused",
                codes,
            )
            self.assertNotIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_trust_profile_json_digest_cannot_be_reused_across_relabelled_compact_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_trust = json.loads(json.dumps(evidence["trust_summaries"][0]))
            copied_trust["path"] = "/ops/iso/relabelled-trust.summary.json"
            copied_trust["summary_sha256"] = "e" * 64
            profile = copied_trust["profiles"][0]
            profile["profile_id"] = "swift-cbpr-plus-two"
            profile["bundle_sha256"] = "b" * 64
            evidence["trust_summaries"].append(copied_trust)
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "relabelled-trust-profile-json.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_trust_profile_json_digest_cannot_be_reused_across_relabelled_evidence_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second_profile_id = "swift-cbpr-plus-two"
            profile = second["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = second_profile_id
            profile["bundle_sha256"] = "b" * 64
            for receipt_summary in (
                second["canary_summaries"][0]["receipt_summary"],
                second["receipt_verification"],
            ):
                for receipt in receipt_summary["receipts"]:
                    if receipt["receipt_kind"] == "iso-rail-gateway":
                        receipt["profile"] = second_profile_id
                refresh_digest(receipt_summary)
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_canary_summary_path_cannot_be_reused_as_trust_summary_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["path"] = evidence["canary_summaries"][0][
                "path"
            ]
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "canary-trust-summary-path-reused.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_trust_summary_path_reused", codes)

    def test_canary_summary_digest_cannot_be_reused_as_trust_summary_digest(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["trust_summaries"][0]["summary_sha256"] = (
                evidence["canary_summaries"][0]["summary_sha256"]
            )
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "canary-trust-summary-digest-reused.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_trust_summary_digest_reused", codes)

    def test_canary_summary_path_cannot_be_reused_as_trust_summary_across_evidence_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second_profile_id = "swift-cbpr-plus-two"
            profile = second["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = second_profile_id
            profile["bundle_sha256"] = "b" * 64
            second["trust_summaries"][0]["profile_json_sha256"] = "c" * 64
            second["trust_summaries"][0]["path"] = first["canary_summaries"][0]["path"]
            for receipt_summary in (
                second["canary_summaries"][0]["receipt_summary"],
                second["receipt_verification"],
            ):
                for receipt in receipt_summary["receipts"]:
                    if receipt["receipt_kind"] == "iso-rail-gateway":
                        receipt["profile"] = second_profile_id
                refresh_digest(receipt_summary)
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_trust_summary_path_reused", codes)
            self.assertNotIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_canary_summary_digest_cannot_be_reused_as_trust_summary_across_evidence_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            second = json.loads(evidence_two.read_text(encoding="utf-8"))
            second_profile_id = "swift-cbpr-plus-two"
            profile = second["trust_summaries"][0]["profiles"][0]
            profile["profile_id"] = second_profile_id
            profile["bundle_sha256"] = "b" * 64
            second["trust_summaries"][0]["profile_json_sha256"] = "c" * 64
            second["trust_summaries"][0]["summary_sha256"] = (
                first["canary_summaries"][0]["summary_sha256"]
            )
            for receipt_summary in (
                second["canary_summaries"][0]["receipt_summary"],
                second["receipt_verification"],
            ):
                for receipt in receipt_summary["receipts"]:
                    if receipt["receipt_kind"] == "iso-rail-gateway":
                        receipt["profile"] = second_profile_id
                refresh_digest(receipt_summary)
            refresh_digest(second)
            write_json(evidence_two, second)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_one),
                    "--evidence-summary",
                    str(evidence_two),
                ]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_trust_summary_digest_reused", codes)
            self.assertNotIn("trust.profile_json_digest_reused", codes)
            self.assertNotIn("trust.profile_id_reused", codes)
            self.assertNotIn("trust.bundle_digest_reused", codes)

    def test_compact_summary_digests_cannot_reuse_nested_roles_across_evidence_summaries(self):
        def next_digest():
            nonlocal digest_counter
            digest_counter += 1
            return f"{digest_counter:064x}"

        def uniquify_second_summary(evidence, profile_id):
            trust_profile = evidence["trust_summaries"][0]["profiles"][0]
            trust_profile["profile_id"] = profile_id
            trust_profile["bundle_sha256"] = next_digest()
            for role in READINESS.TRUST_DER_PROOF_FIELDS:
                for entry in trust_profile[role]:
                    entry["sha256"] = next_digest()
            evidence["trust_summaries"][0]["profile_json_sha256"] = next_digest()

            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                for offset, receipt in enumerate(receipt_summary["receipts"]):
                    receipt["path"] = (
                        f"/ops/iso/{profile_id}/receipts/"
                        f"{receipt['receipt_kind']}.{offset}.receipt.json"
                    )
                    receipt["receipt_sha256"] = next_digest()
                    receipt["response_body_sha256"] = next_digest()
                    if receipt["receipt_kind"] == "iso-audit-notary":
                        receipt["anchor_path"] = (
                            f"/ops/iso/{profile_id}/notary/anchors/"
                            f"{offset}.notary.json"
                        )
                        receipt["store_dir"] = f"/ops/iso/{profile_id}/notary-store"
                        receipt["index_path"] = (
                            f"/ops/iso/{profile_id}/notary/messages.index.json"
                        )
                        receipt["anchor_sha256"] = next_digest()
                        receipt["index_sha256"] = next_digest()
                    else:
                        receipt["source_path"] = (
                            f"/ops/iso/{profile_id}/rail-inbox/{offset}.xml"
                        )
                        receipt["payload_sha256"] = next_digest()
                        receipt["profile"] = profile_id
                        receipt["rail_message_id"] = f"{profile_id}-{offset}"
                refresh_digest(receipt_summary)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_one = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-one")
            )
            evidence_two = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence-two")
            )
            first = json.loads(evidence_one.read_text(encoding="utf-8"))
            base_second = json.loads(evidence_two.read_text(encoding="utf-8"))

            cases = (
                (
                    "canary-receipt-summary",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "summary_sha256"
                    ],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_receipt_summary",
                ),
                (
                    "canary-receipt",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "receipts"
                    ][0]["receipt_sha256"],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_receipt",
                ),
                (
                    "canary-material",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "receipts"
                    ][1]["payload_sha256"],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_receipt_material",
                ),
                (
                    "trust-profile-json",
                    lambda evidence: evidence["trust_summaries"][0][
                        "profile_json_sha256"
                    ],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_profile_json",
                ),
                (
                    "trust-bundle",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][0][
                        "bundle_sha256"
                    ],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_bundle",
                ),
                (
                    "trust-der",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][0][
                        "x509_trust_anchor_der"
                    ][0]["sha256"],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_der_proof",
                ),
                (
                    "canary-trust-profile-json",
                    lambda evidence: evidence["trust_summaries"][0][
                        "profile_json_sha256"
                    ],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_trust_profile_json",
                ),
                (
                    "canary-trust-bundle",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][0][
                        "bundle_sha256"
                    ],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_trust_bundle",
                ),
                (
                    "canary-trust-der",
                    lambda evidence: evidence["trust_summaries"][0]["profiles"][0][
                        "x509_crl_der"
                    ][0]["sha256"],
                    lambda evidence, digest: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "evidence.canary_summary_digest_matches_trust_der_proof",
                ),
                (
                    "trust-receipt-summary",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "summary_sha256"
                    ],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_receipt_summary",
                ),
                (
                    "trust-receipt",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "receipts"
                    ][0]["receipt_sha256"],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_receipt",
                ),
                (
                    "trust-receipt-material",
                    lambda evidence: evidence["canary_summaries"][0]["receipt_summary"][
                        "receipts"
                    ][1]["payload_sha256"],
                    lambda evidence, digest: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        digest,
                    ),
                    "trust.summary_digest_matches_receipt_material",
                ),
            )

            for offset, (name, target_digest, mutate, expected_code) in enumerate(cases):
                with self.subTest(name=name):
                    digest_counter = 0xA000 + offset * 0x100
                    second = json.loads(json.dumps(base_second))
                    profile_id = f"swift-cbpr-plus-cross-{offset}"
                    uniquify_second_summary(second, profile_id)
                    mutate(second, target_digest(first))
                    refresh_digest(second)
                    second_path = write_json(
                        root / f"cross-summary-digest-role-{name}.summary.json",
                        second,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(evidence_one),
                            "--evidence-summary",
                            str(second_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(expected_code, codes)
                    blocker_text = "\n".join(
                        blocker["message"]
                        for blocker in blockers
                        if blocker["code"] == expected_code
                    )
                    self.assertIn("evidence_summaries[1]", blocker_text)
                    self.assertIn("evidence_summaries[0]", blocker_text)
                    self.assertNotIn(target_digest(first), blocker_text)

    def test_canary_rail_receipts_require_matching_trust_profile(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "wrong-rail",
                    lambda profile: profile.update(
                        {
                            "profile_id": "fedwire-funds",
                            "rail": "fedwire-funds",
                        }
                    ),
                    {"trust.canary_rail_without_profile"},
                ),
                (
                    "wrong-profile-id",
                    lambda profile: profile.update(
                        {"profile_id": "swift-cbpr-plus-alt"}
                    ),
                    {"trust.canary_rail_without_profile"},
                ),
                (
                    "wrong-environment",
                    lambda profile: profile.update({"environment": "prod"}),
                    {
                        "trust.canary_rail_without_profile",
                        "trust.environment_mismatch",
                    },
                ),
            )

            for name, mutate, expected_codes in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence["trust_summaries"][0]["profiles"][0])
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"canary-rail-missing-trust-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertTrue(expected_codes <= codes)
                    blocker_text = "\n".join(
                        blocker["message"]
                        for blocker in blockers
                        if blocker["code"] == "trust.canary_rail_without_profile"
                    )
                    self.assertIn(
                        "canary_summaries[0].receipt_summary.receipts[1].profile "
                        "has no matching trust profile coverage for canary environment",
                        blocker_text,
                    )
                    self.assertNotIn("'swift-cbpr-plus'", blocker_text)
                    self.assertNotIn("'preprod'", blocker_text)

    def test_default_profile_canary_receipts_require_policy_binding(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("null-policy-binding", lambda policy: None),
                (
                    "missing-policy-binding",
                    lambda policy: policy.pop("default_rail_profile"),
                ),
            )
            for name, mutate_policy in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate_policy(evidence["policy"])
                    for receipt_summary in (
                        evidence["canary_summaries"][0]["receipt_summary"],
                        evidence["receipt_verification"],
                    ):
                        receipt_summary["allow_default_profile"] = True
                        receipt_summary["receipts"][1]["profile"] = None
                        refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"default-profile-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn("trust.canary_rail_default_profile_unbound", codes)
                    blocker_text = "\n".join(
                        blocker["message"]
                        for blocker in blockers
                        if blocker["code"] == "trust.canary_rail_default_profile_unbound"
                    )
                    self.assertIn(
                        "canary_summaries[0].receipt_summary.receipts[1].profile "
                        "uses default rail profile without evidence.policy.default_rail_profile",
                        blocker_text,
                    )
                    self.assertNotIn("swift-cbpr-plus", blocker_text)
                    self.assertNotIn("preprod", blocker_text)

    def test_default_profile_canary_receipts_use_bound_profile_for_trust_coverage(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "matching-default-profile",
                    "swift-cbpr-plus",
                    set(),
                ),
                (
                    "untrusted-default-profile",
                    "swift-cbpr-plus-alt",
                    {"trust.canary_rail_without_profile"},
                ),
            )
            for name, default_profile, expected_trust_codes in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    evidence["policy"]["allow_default_profile"] = True
                    evidence["policy"]["default_rail_profile"] = default_profile
                    for receipt_summary in (
                        evidence["canary_summaries"][0]["receipt_summary"],
                        evidence["receipt_verification"],
                    ):
                        receipt_summary["allow_default_profile"] = True
                        receipt_summary["receipts"][1]["profile"] = None
                        refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn("evidence.policy.allow_default_profile", codes)
                    self.assertFalse(
                        {"trust.canary_rail_default_profile_unbound"} & codes
                    )
                    if expected_trust_codes:
                        self.assertTrue(expected_trust_codes <= codes)
                    else:
                        self.assertNotIn("trust.canary_rail_without_profile", codes)

    def test_custom_canary_profile_id_can_use_matching_trust_profile(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            custom_profile = "swift-cbpr-plus-alt"
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                receipt_summary["receipts"][1]["profile"] = custom_profile
                refresh_digest(receipt_summary)
            evidence["trust_summaries"][0]["profiles"][0]["profile_id"] = custom_profile
            refresh_digest(evidence)
            mutated_path = write_json(root / "custom-profile-evidence.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["ok"])
            self.assertEqual(summary["blockers"], [])
            self.assertEqual(
                summary["evidence_summaries"][0]["canary_summaries"][0][
                    "receipt_summary"
                ]["receipts"][1]["profile"],
                custom_profile,
            )
            self.assertEqual(
                summary["evidence_summaries"][0]["trust_summaries"][0]["profiles"][0][
                    "profile_id"
                ],
                custom_profile,
            )

    def test_custom_canary_profile_message_type_must_match_trust_rail(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            custom_profile = "swift-cbpr-plus-alt"
            hidden_message_type = "pacs.008"
            hidden_rail = "securities-csd"
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                rail_receipt = receipt_summary["receipts"][1]
                rail_receipt["profile"] = custom_profile
                rail_receipt["message_type"] = hidden_message_type
                refresh_digest(receipt_summary)
            trust_profile = evidence["trust_summaries"][0]["profiles"][0]
            trust_profile["profile_id"] = custom_profile
            trust_profile["rail"] = hidden_rail
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "custom-profile-wrong-rail-evidence.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("trust.canary_rail_message_type_without_profile", codes)
            blocker_text = "\n".join(
                blocker["message"]
                for blocker in blockers
                if blocker["code"] == "trust.canary_rail_message_type_without_profile"
            )
            self.assertIn(
                "canary_summaries[0].receipt_summary.receipts[1].message_type "
                "has no matching trust profile rail coverage",
                blocker_text,
            )
            self.assertNotIn(custom_profile, blocker_text)
            self.assertNotIn(hidden_message_type, blocker_text)
            self.assertNotIn(hidden_rail, blocker_text)

    def test_custom_canary_profile_id_without_trust_profile_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            custom_profile = "swift-cbpr-plus-alt"
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            for receipt_summary in (
                evidence["canary_summaries"][0]["receipt_summary"],
                evidence["receipt_verification"],
            ):
                receipt_summary["receipts"][1]["profile"] = custom_profile
                refresh_digest(receipt_summary)
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "custom-profile-without-trust.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("trust.canary_rail_without_profile", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertIn(
                "canary_summaries[0].receipt_summary.receipts[1].profile "
                "has no matching trust profile coverage for canary environment",
                blocker_text,
            )
            self.assertNotIn(custom_profile, blocker_text)
            self.assertNotIn("'preprod'", blocker_text)

    def test_weak_trust_revocation_posture_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["x509_require_crl_revocation_check"] = False
            profile["x509_crl_count"] = 0
            profile["x509_crl_der"] = []
            profile["x509_require_ocsp_revocation_check"] = False
            profile["x509_ocsp_response_count"] = 0
            profile["x509_ocsp_response_der"] = []
            refresh_digest(evidence)
            mutated_path = write_json(root / "weak-revocation.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.crl_revocation_not_required", codes)
            self.assertIn("trust.ocsp_revocation_not_required", codes)

    def test_missing_required_revocation_material_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            profile = evidence["trust_summaries"][0]["profiles"][0]
            profile["x509_crl_count"] = 0
            profile["x509_crl_der"] = []
            profile["x509_ocsp_response_count"] = 0
            profile["x509_ocsp_response_der"] = []
            refresh_digest(evidence)
            mutated_path = write_json(root / "missing-revocation-material.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.no_crl_revocation_material", codes)
            self.assertIn("trust.no_ocsp_revocation_material", codes)

    def test_missing_or_partial_archive_receipt_verification_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_evidence_summary(
                root / "evidence",
                direct_receipts=False,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipts_not_reverified", codes)
            self.assertIn("evidence.policy.allow_canary_stage_receipts_only", codes)

            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(evidence_summary),
                    "--allow-canary-stage-receipts-only",
                ]
            )
            self.assertEqual(rc, 0, stderr)
            diagnostic = json.loads(stdout)
            self.assertTrue(diagnostic["ok"])
            self.assertTrue(diagnostic["policy"]["allow_canary_stage_receipts_only"])

            forged_policy = json.loads(evidence_summary.read_text(encoding="utf-8"))
            forged_policy["policy"]["allow_canary_stage_receipts_only"] = False
            refresh_digest(forged_policy)
            forged_policy_path = write_json(
                root / "forged-archive-receipt-policy.summary.json",
                forged_policy,
            )
            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(forged_policy_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "requires at least one evidence summary with canary-stage-only "
                "receipt policy and missing direct receipt archive verification",
                stderr,
            )

            forged_policy_with_archive = json.loads(
                add_archive_receipt_verification(
                    write_evidence_summary(root / "forged-policy-with-archive")
                ).read_text(encoding="utf-8")
            )
            forged_policy_with_archive["policy"][
                "allow_canary_stage_receipts_only"
            ] = True
            refresh_digest(forged_policy_with_archive)
            forged_policy_with_archive_path = write_json(
                root / "forged-policy-with-archive.summary.json",
                forged_policy_with_archive,
            )
            rc, stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(forged_policy_with_archive_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "requires at least one evidence summary with canary-stage-only "
                "receipt policy and missing direct receipt archive verification",
                stderr,
            )

            omitted = json.loads(evidence_summary.read_text(encoding="utf-8"))
            omitted.pop("receipt_verification")
            refresh_digest(omitted)
            omitted_path = write_json(root / "omitted-archive-receipts.summary.json", omitted)
            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(omitted_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("receipt_verification must be recorded", stderr)

            partial = add_archive_receipt_verification(
                write_evidence_summary(root / "partial-evidence"),
                ["iso-rail-gateway"],
            )
            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(partial)]
            )
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_kinds_missing", codes)

    def test_plan_only_evidence_reports_blockers_without_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_plan_only_evidence_summary(root / "evidence")

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(evidence_summary)]
            )

            self.assertEqual(rc, 1, stderr)
            summary = json.loads(stdout)
            codes = {blocker["code"] for blocker in summary["blockers"]}
            self.assertIn("evidence.policy.allow_plan_only", codes)
            self.assertIn("evidence.policy.allow_canary_stage_receipts_only", codes)
            self.assertIn("evidence.plan_only", codes)
            self.assertIn("evidence.archive_receipts_not_reverified", codes)
            canary = summary["evidence_summaries"][0]["canary_summaries"][0]
            self.assertIsNone(canary["receipt_summary"])
            self.assertEqual(canary["verified_receipts"], 0)
            self.assertEqual(canary["receipt_kind"], [])

    def test_plan_only_evidence_must_not_smuggle_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_plan_only_evidence_summary(root / "evidence")
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["canary_summaries"][0]["receipt_summary"] = receipt_verification_summary()
            refresh_digest(evidence)
            forged_path = write_json(root / "plan-only-smuggled-receipts.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(forged_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_summary must be null for plan-only evidence", stderr)

    def test_plan_only_evidence_must_record_null_receipt_summary(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_plan_only_evidence_summary(root / "evidence")
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["canary_summaries"][0].pop("receipt_summary")
            refresh_digest(evidence)
            forged_path = write_json(root / "plan-only-omitted-receipts.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(forged_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_summary must be recorded", stderr)

    def test_plan_only_evidence_must_not_record_stage_windows(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = write_plan_only_evidence_summary(root / "evidence")
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["canary_summaries"][0]["stage_windows"] = [
                {
                    "name": "rail",
                    "started_at": "2026-06-04T00:00:00+00:00",
                    "finished_at": "2026-06-04T00:00:00+00:00",
                }
            ]
            refresh_digest(evidence)
            forged_path = write_json(root / "plan-only-stage-window.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                [
                    "--xsd-summary",
                    str(xsd_summary),
                    "--evidence-summary",
                    str(forged_path),
                    "--allow-canary-stage-receipts-only",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertIn("stage_windows must be empty for plan-only evidence", stderr)

    def test_tampered_archive_receipt_summary_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            evidence["receipt_verification"]["receipts"][0]["receipt_sha256"] = "f" * 64
            refresh_digest(evidence)
            tampered_path = write_json(root / "tampered-receipts.summary.json", evidence)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(tampered_path)]
            )

            self.assertEqual(rc, 2)
            self.assertIn("receipt_verification summary_sha256 mismatch", stderr)

    def test_archive_receipts_must_cover_canary_receipt_digests(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            missing_digest = evidence["canary_summaries"][0]["receipt_summary"][
                "receipts"
            ][0]["receipt_sha256"]
            evidence["receipt_verification"]["receipts"][0]["receipt_sha256"] = "f" * 64
            refresh_digest(evidence["receipt_verification"])
            refresh_digest(evidence)
            mutated_path = write_json(root / "unbound-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("evidence.archive_receipt_missing_canary_digest", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertNotIn(missing_digest, blocker_text)

    def test_local_canary_policies_do_not_hide_archive_digest_gaps(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            receipt_root = root / "receipts"
            receipt_root.mkdir()
            notary_receipts, rail_receipts = evidence_test.write_https_receipt_dirs(
                receipt_root
            )
            full_canary_path = write_json(
                root / "full-canary.summary.json",
                evidence_test.valid_canary_summary(
                    receipt_entries=evidence_test.receipt_entries_from_dirs(
                        notary_receipts,
                        rail_receipts,
                    )
                ),
            )
            partial_body = evidence_test.plan_only_canary_summary()
            partial_body["planned_stages"] = [
                partial_body["planned_stages"][0],
                partial_body["planned_stages"][2],
            ]
            del partial_body["planned_stages"][1]["command"][4:6]
            partial_body.pop("summary_sha256")
            partial_canary_path = write_json(
                root / "partial-canary.summary.json",
                evidence_test.digest_summary(partial_body),
            )
            dry_run_body = evidence_test.plan_only_canary_summary()
            dry_run_body["planned_stages"][0]["dry_run"] = True
            dry_run_body["planned_stages"][0]["command"].append("--dry-run")
            dry_run_body.pop("summary_sha256")
            dry_run_canary_path = write_json(
                root / "dry-run-canary.summary.json",
                evidence_test.digest_summary(dry_run_body),
            )
            trust_path = evidence_test.write_trust_summary(root / "trust")
            evidence_path = root / "local-policy.evidence.summary.json"
            rc, _stdout, stderr = evidence_test.run_evidence(
                [
                    "--canary-summary",
                    str(full_canary_path),
                    "--canary-summary",
                    str(partial_canary_path),
                    "--canary-summary",
                    str(dry_run_canary_path),
                    "--trust-summary",
                    str(trust_path),
                    "--provider",
                    "local-bank",
                    "--environment",
                    "preprod",
                    "--allow-plan-only",
                    "--allow-partial-canary",
                    "--allow-dry-run",
                    "--receipt-dir",
                    str(notary_receipts),
                    "--receipt-dir",
                    str(rail_receipts),
                    "--summary-out",
                    str(evidence_path),
                ]
            )
            self.assertEqual(rc, 0, stderr)

            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            notary_archive_receipt = next(
                receipt
                for receipt in evidence["receipt_verification"]["receipts"]
                if receipt["receipt_kind"] == "iso-audit-notary"
            )
            evidence["receipt_verification"] = receipt_verification_summary(
                ["iso-audit-notary"],
                receipts=[dict(notary_archive_receipt)],
            )
            refresh_digest(evidence)
            forged_path = write_json(
                root / "local-policy-missing-rail-archive.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(forged_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_missing_canary_digest", codes)
            self.assertIn("evidence.archive_receipt_kinds_missing", codes)
            self.assertIn("evidence.policy.allow_partial_canary", codes)
            self.assertIn("evidence.policy.allow_dry_run", codes)

    def test_compact_receipt_source_path_blocks_checked_in_iso_fixtures(self):
        checked_in_fixture = REPO_ROOT / "fixtures" / "iso20022" / "pacs008_fixture.xml"
        cases = (
            "fixtures/iso20022/pacs008_fixture.xml",
            str(checked_in_fixture),
            "/ops/release/fixtures/iso20022/pacs008_fixture.xml",
        )
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            for offset, source_path in enumerate(cases):
                with self.subTest(source_path=source_path):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    for receipt_summary in (
                        evidence["canary_summaries"][0]["receipt_summary"],
                        evidence["receipt_verification"],
                    ):
                        receipt_summary["receipts"][1]["source_path"] = source_path
                        refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"fixture-source-receipt-{offset}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    self.assertEqual(stderr, "")
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("evidence.receipt_metadata_invalid", codes)
                    self.assertIn("evidence.archive_receipt_metadata_invalid", codes)
                    self.assertIn("checked-in ISO XML fixtures", stdout)

    def test_archive_receipts_must_not_include_unreferenced_digests(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            archive = evidence["receipt_verification"]
            extra = dict(archive["receipts"][0])
            extra["path"] = "/ops/iso/receipts/extra-unreferenced.receipt.json"
            extra["receipt_sha256"] = "e" * 64
            archive["receipts"].append(extra)
            archive["verified_receipts"] += 1
            refresh_digest(archive)
            refresh_digest(evidence)
            mutated_path = write_json(root / "extra-archive-receipt.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("evidence.archive_receipt_unreferenced_digest", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertNotIn(extra["receipt_sha256"], blocker_text)

    def test_archive_receipts_must_bind_canary_receipt_kinds(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            receipts = evidence["canary_summaries"][0]["receipt_summary"]["receipts"]
            receipts[0]["receipt_kind"], receipts[1]["receipt_kind"] = (
                receipts[1]["receipt_kind"],
                receipts[0]["receipt_kind"],
            )
            refresh_digest(evidence["canary_summaries"][0]["receipt_summary"])
            refresh_digest(evidence)
            mutated_path = write_json(root / "kind-swapped-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("evidence.archive_receipt_canary_kind_mismatch", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertIn(
                "a receipt kind that does not match canary receipt kind",
                blocker_text,
            )
            self.assertNotIn("receipt_kind 'iso-rail-gateway'", blocker_text)
            self.assertNotIn("receipt_kind 'iso-audit-notary'", blocker_text)
            self.assertNotIn(receipts[0]["receipt_sha256"], blocker_text)

    def test_archive_receipts_must_bind_canary_receipt_filenames(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary_receipts = evidence["canary_summaries"][0]["receipt_summary"][
                "receipts"
            ]
            canary_receipts[0]["path"] = "/ops/iso/receipts/relabelled-notary.receipt.json"
            refresh_digest(evidence["canary_summaries"][0]["receipt_summary"])
            refresh_digest(evidence)
            mutated_path = write_json(root / "path-relabelled-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            blockers = json.loads(stdout)["blockers"]
            codes = {blocker["code"] for blocker in blockers}
            self.assertIn("evidence.archive_receipt_canary_path_mismatch", codes)
            blocker_text = "\n".join(blocker["message"] for blocker in blockers)
            self.assertIn(
                "a receipt filename that does not match canary receipt filename",
                blocker_text,
            )
            self.assertNotIn("relabelled-notary.receipt.json", blocker_text)
            self.assertNotIn("relabelled-notary.receipt.json", stderr)
            self.assertNotIn(canary_receipts[0]["receipt_sha256"], blocker_text)

    def test_archive_receipts_must_bind_canary_receipt_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-rail-profile",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("profile", "sepa-sct-inst"),
                ),
                (
                    "canary-rail-message-id",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("rail_message_id", "rail-drop-other"),
                ),
                (
                    "canary-status-code",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("status_code", 201),
                ),
                (
                    "canary-notary-anchor-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        f"/ops/iso/notary/anchors/{receipt['index_sha256']}.notary.json",
                    ),
                ),
                (
                    "canary-notary-store-dir",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "store_dir",
                        "/ops/iso/other-notary-store",
                    ),
                ),
                (
                    "canary-notary-index-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: (
                        receipt.__setitem__(
                            "anchor_path",
                            f"/ops/iso/other-notary/anchors/{receipt['index_sha256']}.notary.json",
                        ),
                        receipt.__setitem__(
                            "index_path",
                            "/ops/iso/other-notary/messages.index.json",
                        ),
                    ),
                ),
                (
                    "canary-response-body-digest",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("response_body_sha256", "f" * 64),
                ),
                (
                    "archive-endpoint-policy-evidence",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        True,
                    ),
                ),
                (
                    "archive-notary-record-count",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("record_count", 2),
                ),
                (
                    "archive-notary-anchor-path",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        f"/ops/iso/notary/anchors/{receipt['index_sha256']}.notary.json",
                    ),
                ),
                (
                    "archive-notary-store-dir",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "store_dir",
                        "/ops/iso/other-notary-store",
                    ),
                ),
                (
                    "archive-notary-index-path",
                    ("receipt_verification",),
                    0,
                    lambda receipt: (
                        receipt.__setitem__(
                            "anchor_path",
                            f"/ops/iso/other-notary/anchors/{receipt['index_sha256']}.notary.json",
                        ),
                        receipt.__setitem__(
                            "index_path",
                            "/ops/iso/other-notary/messages.index.json",
                        ),
                    ),
                ),
            )
            for name, summary_path, receipt_index, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for key in summary_path:
                        receipt_summary = receipt_summary[key]
                    receipt_digest = receipt_summary["receipts"][receipt_index][
                        "receipt_sha256"
                    ]
                    mutate(receipt_summary["receipts"][receipt_index])
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}-metadata.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn("evidence.archive_receipt_canary_metadata_mismatch", codes)
                    blocker_text = "\n".join(blocker["message"] for blocker in blockers)
                    self.assertNotIn(receipt_digest, blocker_text)

    def test_canary_receipts_cannot_be_reused_across_compact_summaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_canary = json.loads(json.dumps(evidence["canary_summaries"][0]))
            copied_canary["path"] = "/ops/iso/copied-canary.summary.json"
            copied_canary["config_path"] = "/ops/iso/copied-canary.json"
            copied_canary["summary_sha256"] = "e" * 64
            evidence["canary_summaries"].append(copied_canary)
            refresh_digest(evidence)
            mutated_path = write_json(root / "reused-canary-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.canary_receipt_path_reused", codes)
            self.assertIn("evidence.canary_receipt_digest_reused", codes)

    def test_canary_source_material_cannot_be_reused_across_relabelled_canaries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            copied_canary = json.loads(json.dumps(evidence["canary_summaries"][0]))
            copied_canary["path"] = "/ops/iso/relabelled-canary.summary.json"
            copied_canary["config_path"] = "/ops/iso/relabelled-canary.json"
            copied_canary["summary_sha256"] = "e" * 64
            for offset, receipt in enumerate(copied_canary["receipt_summary"]["receipts"]):
                receipt["path"] = (
                    f"/ops/iso/relabelled-canary/{receipt['receipt_kind']}.{offset}.receipt.json"
                )
                receipt["receipt_sha256"] = f"{offset + 8:064x}"
            refresh_digest(copied_canary["receipt_summary"])
            evidence["canary_summaries"].append(copied_canary)
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "relabelled-canary-source-replay.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            expected = {
                "evidence.canary_receipt_source_path_reused",
                "evidence.canary_receipt_payload_digest_reused",
                "evidence.canary_receipt_anchor_path_reused",
                "evidence.canary_receipt_anchor_digest_reused",
                "evidence.canary_receipt_store_dir_reused",
                "evidence.canary_receipt_index_path_reused",
                "evidence.canary_receipt_index_digest_reused",
            }
            self.assertTrue(expected <= codes)
            self.assertNotIn("evidence.canary_receipt_path_reused", codes)
            self.assertNotIn("evidence.canary_receipt_digest_reused", codes)

    def test_receipt_summary_kind_lists_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
            receipt_summary["receipt_kind"].append("iso-audit-notary")
            refresh_digest(receipt_summary)
            archive = evidence["receipt_verification"]
            archive["receipt_kind"].append("iso-audit-notary")
            refresh_digest(archive)
            refresh_digest(evidence)
            mutated_path = write_json(root / "duplicate-receipt-kinds.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.receipt_kind_entry_mismatch", codes)
            self.assertIn("evidence.archive_receipt_kind_entry_mismatch", codes)

    def test_receipt_entry_kinds_must_be_supported(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def nested(value, parts):
                for part in parts:
                    value = value[part]
                return value

            cases = (
                (
                    "canary",
                    ("canary_summaries", 0, "receipt_summary"),
                    "evidence.receipt_kind_entry_mismatch",
                ),
                (
                    "archive",
                    ("receipt_verification",),
                    "evidence.archive_receipt_kind_entry_mismatch",
                ),
            )
            for name, path_parts, code in cases:
                with self.subTest(name=name):
                    hidden_kind = "diagnostic-receipt"
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = nested(evidence, path_parts)
                    receipt_summary["receipts"][0]["receipt_kind"] = hidden_kind
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"unsupported-{name}-receipt-kind.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(code, codes)
                    blocker_text = "\n".join(blocker["message"] for blocker in blockers)
                    self.assertNotIn(hidden_kind, blocker_text)

    def test_receipt_kind_summary_lists_must_not_include_unsupported_values(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )

            def nested(value, parts):
                for part in parts:
                    value = value[part]
                return value

            cases = (
                (
                    "canary",
                    ("canary_summaries", 0, "receipt_summary"),
                    "evidence.receipt_kind_entry_mismatch",
                ),
                (
                    "archive",
                    ("receipt_verification",),
                    "evidence.archive_receipt_kind_entry_mismatch",
                ),
            )
            for name, path_parts, code in cases:
                with self.subTest(name=name):
                    hidden_kind = "diagnostic-receipt-list"
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = nested(evidence, path_parts)
                    receipt_summary["receipt_kind"].append(hidden_kind)
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"unsupported-{name}-receipt-kind-list.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    codes = {blocker["code"] for blocker in blockers}
                    self.assertIn(code, codes)
                    blocker_text = "\n".join(blocker["message"] for blocker in blockers)
                    self.assertNotIn(hidden_kind, blocker_text)

    def test_archive_receipt_metadata_binding_rejects_unsupported_internal_kind(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
            archive_summary = evidence["receipt_verification"]
            receipt_summary["receipts"][0]["receipt_kind"] = "diagnostic-receipt"
            archive_summary["receipts"][0]["receipt_kind"] = "diagnostic-receipt"
            refresh_digest(receipt_summary)
            refresh_digest(archive_summary)
            refresh_digest(evidence)
            mutated_path = write_json(
                root / "unsupported-metadata-kind-binding.summary.json",
                evidence,
            )

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.receipt_kind_entry_mismatch", codes)
            self.assertIn("evidence.archive_receipt_kind_entry_mismatch", codes)
            self.assertIn("evidence.archive_receipt_canary_metadata_mismatch", codes)

    def test_weak_archive_receipt_policy_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            archive = evidence["receipt_verification"]
            archive["allow_failed"] = True
            archive["allow_insecure_http"] = True
            archive["allow_legacy_colr007"] = True
            archive["allow_default_profile"] = True
            archive["require_source_files"] = False
            refresh_digest(archive)
            refresh_digest(evidence)
            weak_path = write_json(root / "weak-archive-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipts_allow_failed", codes)
            self.assertIn("evidence.archive_receipts_insecure_http", codes)
            self.assertIn("evidence.archive_receipts_allow_legacy_colr007", codes)
            self.assertIn("evidence.archive_receipts_allow_default_profile", codes)
            self.assertIn("evidence.archive_receipts_source_files_not_required", codes)

    def test_receipt_policy_flags_must_bind_receipt_entries(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-failed",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda summary: summary.__setitem__("allow_failed", True),
                    "evidence.receipts_allow_failed",
                    "no failed receipt entry was recorded",
                ),
                (
                    "archive-failed",
                    ("receipt_verification",),
                    lambda summary: summary.__setitem__("allow_failed", True),
                    "evidence.archive_receipts_allow_failed",
                    "no failed receipt entry was recorded",
                ),
                (
                    "canary-insecure-http",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda summary: summary.__setitem__("allow_insecure_http", True),
                    "evidence.receipts_allow_insecure_http",
                    "no http:// or local/private receipt endpoint was recorded",
                ),
                (
                    "archive-insecure-http",
                    ("receipt_verification",),
                    lambda summary: summary.__setitem__("allow_insecure_http", True),
                    "evidence.archive_receipts_insecure_http",
                    "no http:// or local/private receipt endpoint was recorded",
                ),
                (
                    "canary-legacy",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda summary: summary.__setitem__("allow_legacy_colr007", True),
                    "evidence.receipts_allow_legacy_colr007",
                    "no legacy colr.007 receipt entry was recorded",
                ),
                (
                    "archive-legacy",
                    ("receipt_verification",),
                    lambda summary: summary.__setitem__("allow_legacy_colr007", True),
                    "evidence.archive_receipts_allow_legacy_colr007",
                    "no legacy colr.007 receipt entry was recorded",
                ),
                (
                    "canary-default-profile",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda summary: summary.__setitem__("allow_default_profile", True),
                    "evidence.receipts_allow_default_profile",
                    "no default-profile receipt entry was recorded",
                ),
                (
                    "archive-default-profile",
                    ("receipt_verification",),
                    lambda summary: summary.__setitem__("allow_default_profile", True),
                    "evidence.archive_receipts_allow_default_profile",
                    "no default-profile receipt entry was recorded",
                ),
            )
            for name, path_parts, mutate, code, message in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for part in path_parts:
                        receipt_summary = receipt_summary[part]
                    mutate(receipt_summary)
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"forged-{name}-failed-policy.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    blockers = json.loads(stdout)["blockers"]
                    matching = [blocker for blocker in blockers if blocker["code"] == code]
                    self.assertTrue(matching)
                    self.assertTrue(
                        any(message in blocker["message"] for blocker in matching)
                    )

    def test_archive_receipt_entries_must_bind_each_receipt_digest(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            archive = evidence["receipt_verification"]
            del archive["receipts"][0]["receipt_sha256"]
            refresh_digest(archive)
            refresh_digest(evidence)
            weak_path = write_json(root / "missing-receipt-digest.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_digest_missing", codes)

    def test_receipt_entries_reject_all_zero_digest_placeholders(self):
        def nested(body, path_parts):
            target = body
            for part in path_parts:
                target = target[part]
            return target

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-receipt",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("receipt_sha256", "0" * 64),
                    "evidence.receipt_digest_missing",
                ),
                (
                    "archive-receipt",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("receipt_sha256", "0" * 64),
                    "evidence.archive_receipt_digest_missing",
                ),
                (
                    "canary-response-body",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("response_body_sha256", "0" * 64),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "archive-response-body",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("response_body_sha256", "0" * 64),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "canary-anchor",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("anchor_sha256", "0" * 64),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "archive-index",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("index_sha256", "0" * 64),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "canary-payload",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("payload_sha256", "0" * 64),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "archive-payload",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.__setitem__("payload_sha256", "0" * 64),
                    "evidence.archive_receipt_metadata_invalid",
                ),
            )
            for name, path_parts, receipt_index, mutate, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = nested(evidence, path_parts)
                    mutate(receipt_summary["receipts"][receipt_index])
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"zero-receipt-digest-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_receipt_entry_digests_cannot_reuse_response_or_source_roles(self):
        def mutate_receipts(receipt_summary):
            for receipt in receipt_summary["receipts"]:
                if receipt["receipt_kind"] == "iso-audit-notary":
                    receipt["response_body_sha256"] = receipt["anchor_sha256"]
                else:
                    receipt["receipt_sha256"] = receipt["payload_sha256"]
            refresh_digest(receipt_summary)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            mutate_receipts(evidence["canary_summaries"][0]["receipt_summary"])
            mutate_receipts(evidence["receipt_verification"])
            refresh_digest(evidence)
            mutated_path = write_json(root / "receipt-digest-role-reuse.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.receipt_digest_role_reuse", codes)
            self.assertIn("evidence.archive_receipt_digest_role_reuse", codes)

    def test_compact_summary_digests_cannot_reuse_nested_material_roles(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-receipt-summary",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"][
                            "summary_sha256"
                        ],
                    ),
                    "evidence.canary_summary_digest_matches_receipt_summary",
                ),
                (
                    "canary-receipt",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"]["receipts"][
                            0
                        ]["receipt_sha256"],
                    ),
                    "evidence.canary_summary_digest_matches_receipt",
                ),
                (
                    "canary-material",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"]["receipts"][
                            1
                        ]["payload_sha256"],
                    ),
                    "evidence.canary_summary_digest_matches_receipt_material",
                ),
                (
                    "trust-profile-json",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profile_json_sha256"],
                    ),
                    "trust.summary_digest_matches_profile_json",
                ),
                (
                    "trust-bundle",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profiles"][0]["bundle_sha256"],
                    ),
                    "trust.summary_digest_matches_bundle",
                ),
                (
                    "trust-der",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profiles"][0][
                            "x509_trust_anchor_der"
                        ][0]["sha256"],
                    ),
                    "trust.summary_digest_matches_der_proof",
                ),
                (
                    "canary-trust-profile-json",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profile_json_sha256"],
                    ),
                    "evidence.canary_summary_digest_matches_trust_profile_json",
                ),
                (
                    "canary-trust-bundle",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profiles"][0]["bundle_sha256"],
                    ),
                    "evidence.canary_summary_digest_matches_trust_bundle",
                ),
                (
                    "canary-trust-der",
                    lambda evidence: evidence["canary_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["trust_summaries"][0]["profiles"][0][
                            "x509_crl_der"
                        ][0]["sha256"],
                    ),
                    "evidence.canary_summary_digest_matches_trust_der_proof",
                ),
                (
                    "trust-receipt-summary",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"][
                            "summary_sha256"
                        ],
                    ),
                    "trust.summary_digest_matches_receipt_summary",
                ),
                (
                    "trust-receipt",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"]["receipts"][
                            0
                        ]["receipt_sha256"],
                    ),
                    "trust.summary_digest_matches_receipt",
                ),
                (
                    "trust-receipt-material",
                    lambda evidence: evidence["trust_summaries"][0].__setitem__(
                        "summary_sha256",
                        evidence["canary_summaries"][0]["receipt_summary"]["receipts"][
                            1
                        ]["payload_sha256"],
                    ),
                    "trust.summary_digest_matches_receipt_material",
                ),
            )
            for name, mutate, expected_code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    mutate(evidence)
                    refresh_digest(evidence)
                    mutated_path = write_json(
                        root / f"compact-summary-digest-role-{name}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        [
                            "--xsd-summary",
                            str(xsd_summary),
                            "--evidence-summary",
                            str(mutated_path),
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(expected_code, codes)

    def test_receipt_entries_must_be_successful(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "canary-missing-ok",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.pop("ok"),
                    "evidence.receipt_status_mismatch",
                ),
                (
                    "canary-missing-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.pop("status_code"),
                    "evidence.receipt_status_mismatch",
                ),
                (
                    "canary-mismatched-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update({"ok": True, "status_code": 500}),
                    "evidence.receipt_status_mismatch",
                ),
                (
                    "canary-too-large-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update({"ok": False, "status_code": 700}),
                    "evidence.receipt_status_mismatch",
                ),
                (
                    "canary-null-success-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update(
                        {"status_code": None, "response_body_sha256": None}
                    ),
                    "evidence.receipt_status_mismatch",
                ),
                (
                    "canary-failed-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update({"ok": False, "status_code": 503}),
                    "evidence.receipt_not_successful",
                ),
                (
                    "canary-transport-failed-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update(
                        {
                            "ok": False,
                            "status_code": None,
                            "response_body_sha256": None,
                        }
                    ),
                    "evidence.receipt_not_successful",
                ),
                (
                    "canary-transport-failed-with-response-digest",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update({"ok": False, "status_code": None}),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-missing-response-body-digest",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.pop("response_body_sha256"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-malformed-response-body-digest",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.__setitem__("response_body_sha256", "not-a-digest"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-missing-endpoint-policy-evidence",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.pop("endpoint_requires_insecure_http"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-malformed-endpoint-policy-evidence",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        "false",
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-hidden-endpoint-policy-evidence",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        True,
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "archive-missing-ok",
                    ("receipt_verification",),
                    lambda receipt: receipt.pop("ok"),
                    "evidence.archive_receipt_status_mismatch",
                ),
                (
                    "archive-missing-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.pop("status_code"),
                    "evidence.archive_receipt_status_mismatch",
                ),
                (
                    "archive-mismatched-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update({"ok": True, "status_code": 500}),
                    "evidence.archive_receipt_status_mismatch",
                ),
                (
                    "archive-too-large-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update({"ok": False, "status_code": 700}),
                    "evidence.archive_receipt_status_mismatch",
                ),
                (
                    "archive-null-success-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update(
                        {"status_code": None, "response_body_sha256": None}
                    ),
                    "evidence.archive_receipt_status_mismatch",
                ),
                (
                    "archive-redirect-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update({"ok": False, "status_code": 302}),
                    "evidence.archive_receipt_not_successful",
                ),
                (
                    "archive-transport-failed-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update(
                        {
                            "ok": False,
                            "status_code": None,
                            "response_body_sha256": None,
                        }
                    ),
                    "evidence.archive_receipt_not_successful",
                ),
                (
                    "archive-transport-failed-with-response-digest",
                    ("receipt_verification",),
                    lambda receipt: receipt.update({"ok": False, "status_code": None}),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-missing-endpoint-policy-evidence",
                    ("receipt_verification",),
                    lambda receipt: receipt.pop("endpoint_requires_insecure_http"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-malformed-endpoint-policy-evidence",
                    ("receipt_verification",),
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        "false",
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-hidden-endpoint-policy-evidence",
                    ("receipt_verification",),
                    lambda receipt: receipt.__setitem__(
                        "endpoint_requires_insecure_http",
                        True,
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-missing-response-body-digest",
                    ("receipt_verification",),
                    lambda receipt: receipt.pop("response_body_sha256"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-malformed-response-body-digest",
                    ("receipt_verification",),
                    lambda receipt: receipt.__setitem__("response_body_sha256", "not-a-digest"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
            )
            for name, summary_path, mutate, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for key in summary_path:
                        receipt_summary = receipt_summary[key]
                    mutate(receipt_summary["receipts"][0])
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_receipt_entries_must_preserve_kind_metadata(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            hidden = "\u0660"
            unicode_digit_message_type = f"pacs.{hidden}{hidden}2"
            cases = (
                (
                    "canary-missing-notary-anchor",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.pop("anchor_sha256"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-missing-notary-anchor-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.pop("anchor_path"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-missing-notary-store-dir",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.pop("store_dir"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-missing-notary-index-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.pop("index_path"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-wrong-notary-anchor-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        f"/ops/iso/notary/anchors/{'f' * 64}.notary.json",
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-fixture-notary-anchor-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        "/ops/release/fixtures/iso20022/latest.notary.json",
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-fixture-notary-store-dir",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "store_dir",
                        "/ops/release/fixtures/iso20022/notary-store",
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-fixture-notary-index-path",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "index_path",
                        "/ops/release/fixtures/iso20022/notary/messages.index.json",
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-empty-notary-record-count",
                    ("canary_summaries", 0, "receipt_summary"),
                    0,
                    lambda receipt: receipt.__setitem__("record_count", 0),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-rail-unknown-profile",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("profile", "unknown_rail"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-rail-missing-message-id",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.pop("rail_message_id"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-rail-unsupported-message-type",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "zzzz.999"),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-rail-non-ascii-message-type",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__(
                        "message_type",
                        unicode_digit_message_type,
                    ),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "canary-rail-forbidden-notary-field",
                    ("canary_summaries", 0, "receipt_summary"),
                    1,
                    lambda receipt: receipt.__setitem__("anchor_sha256", "0" * 64),
                    "evidence.receipt_metadata_invalid",
                ),
                (
                    "archive-missing-rail-payload",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.pop("payload_sha256"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-notary-negative-record-count",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("record_count", -1),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-empty-notary-record-count",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__("record_count", 0),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-wrong-notary-anchor-path",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        f"/ops/iso/notary/anchors/{'f' * 64}.notary.json",
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-fixture-notary-anchor-path",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "anchor_path",
                        "/ops/release/fixtures/iso20022/latest.notary.json",
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-fixture-notary-store-dir",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "store_dir",
                        "/ops/release/fixtures/iso20022/notary-store",
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-fixture-notary-index-path",
                    ("receipt_verification",),
                    0,
                    lambda receipt: receipt.__setitem__(
                        "index_path",
                        "/ops/release/fixtures/iso20022/notary/messages.index.json",
                    ),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-legacy-message-type",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "colr.007"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-unsupported-message-type",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "zzzz.999"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-secret-message-type",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.__setitem__("message_type", "token.001"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
                (
                    "archive-bad-rail-message-id",
                    ("receipt_verification",),
                    1,
                    lambda receipt: receipt.__setitem__("rail_message_id", "rail/drop/1"),
                    "evidence.archive_receipt_metadata_invalid",
                ),
            )
            for name, summary_path, receipt_index, mutate, code in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for key in summary_path:
                        receipt_summary = receipt_summary[key]
                    mutate(receipt_summary["receipts"][receipt_index])
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(root / f"{name}.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    result = json.loads(stdout)
                    codes = {blocker["code"] for blocker in result["blockers"]}
                    self.assertIn(code, codes)
                    blocker_text = "\n".join(blocker["message"] for blocker in result["blockers"])
                    self.assertNotIn(hidden, blocker_text)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn(unicode_digit_message_type, blocker_text)
                    self.assertNotIn(unicode_digit_message_type, stderr)
                    self.assertNotIn("token.001", blocker_text)
                    self.assertNotIn("token.001", stderr)
                    self.assertNotIn("zzzz.999", blocker_text)
                    self.assertNotIn("zzzz.999", stderr)
                    self.assertNotIn("colr.007", blocker_text)
                    self.assertNotIn("colr.007", stderr)

    def test_canary_receipt_entries_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "evidence.receipt_path_duplicate"),
                ("receipt_sha256", "evidence.receipt_digest_duplicate"),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence["canary_summaries"][0]["receipt_summary"]
                    receipt_summary["receipts"][1][field] = receipt_summary["receipts"][0][field]
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"duplicate-canary-receipt-{field}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_archive_receipt_entries_must_be_unique(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                ("path", "evidence.archive_receipt_path_duplicate"),
                ("receipt_sha256", "evidence.archive_receipt_digest_duplicate"),
            )
            for field, code in cases:
                with self.subTest(field=field):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    archive = evidence["receipt_verification"]
                    archive["receipts"][1][field] = archive["receipts"][0][field]
                    refresh_digest(archive)
                    refresh_digest(evidence)
                    weak_path = write_json(
                        root / f"duplicate-archive-receipt-{field}.summary.json",
                        evidence,
                    )

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

    def test_receipt_summary_paths_are_canonical(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = (
                (
                    "/ops/iso/receipts/rail\n0.receipt.json",
                    "must not contain control characters",
                ),
                (
                    "/ops/iso/receipts/rail 0.receipt.json",
                    "must not contain whitespace",
                ),
                (
                    "/ops/iso/receipts/r\u0430il.receipt.json",
                    "must use printable ASCII",
                ),
                (
                    "/ops/iso/receipts/rail.receipt.json;v=1",
                    "must not contain semicolon path parameters",
                ),
                (
                    "/ops/iso/receipts//rail.receipt.json",
                    "must not contain empty path segments",
                ),
                (
                    "/ops/iso/receipts/../rail.receipt.json",
                    "must not contain dot or parent segments",
                ),
                (
                    r"..\rail.receipt.json",
                    "must not contain dot or parent segments",
                ),
                (
                    r"/ops\iso/receipts/rail.receipt.json",
                    "must use forward slashes",
                ),
                (
                    "fixtures/iso20022/rail.receipt.json",
                    "must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "/ops/iso/receipts/rail.json",
                    "must point to a .receipt.json file",
                ),
            )
            locations = (
                ("canary", ("canary_summaries", 0, "receipt_summary")),
                ("archive", ("receipt_verification",)),
            )
            for location, parts in locations:
                for offset, (receipt_path, message) in enumerate(cases):
                    with self.subTest(location=location, receipt_path=receipt_path):
                        evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                        receipt_summary = evidence
                        for part in parts:
                            receipt_summary = receipt_summary[part]
                        receipt_summary["receipts"][0]["path"] = receipt_path
                        refresh_digest(receipt_summary)
                        refresh_digest(evidence)
                        weak_path = write_json(
                            root / f"bad-{location}-receipt-path-{offset}.summary.json",
                            evidence,
                        )

                        rc, _stdout, stderr = run_readiness(
                            ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(weak_path)]
                        )

                        self.assertEqual(rc, 2)
                        self.assertIn(message, stderr)
                        if any(ord(ch) > 0x7E for ch in receipt_path):
                            self.assertNotIn(receipt_path, stderr)


if __name__ == "__main__":
    unittest.main()
