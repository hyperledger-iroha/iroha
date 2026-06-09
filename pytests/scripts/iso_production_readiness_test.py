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
            ("unexpected\x1breadiness_key", "\x1b"),
            ("unexpected_readiness_\uff4bey", "\uff4b"),
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
        hidden = "--unknown-readiness\x1bflag"
        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._preflight_raw_cli_secrets([hidden], {"--summary-out"})

        message = str(caught.exception)
        self.assertIn("CLI argument must not contain control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("\x1b", message)
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

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        cases = (
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
            READINESS._check_no_secret_material({"metadata": "warning \x1b[31mred"})

        message = str(caught.exception)
        self.assertIn("unsafe control characters", message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("[31mred", message)

        with self.assertRaises(READINESS.ReadinessError) as caught:
            READINESS._check_no_secret_material(
                {"metadata": "%70assword%253Dreadiness-field-leak"}
            )

        message = str(caught.exception)
        self.assertIn("secret-looking material", message)
        self.assertNotIn("%70assword%253Dreadiness-field-leak", message)
        self.assertNotIn("password=readiness-field-leak", message)
        self.assertNotIn("readiness-field-leak", message)

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
                    "receipt policy or missing direct receipt archive verification",
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
            refresh_digest(first_summary)
            write_json(xsd_one, first_summary)
            second_summary = json.loads(xsd_two.read_text(encoding="utf-8"))
            second_summary["verified_at"] = "2026-06-04T00:00:01+00:00"
            second_summary["blocked_schema_sources"] = [
                xsd_test.blocked_schema_source("barr.001.001.01")
            ]
            second_summary["blocked_schema_source_count"] = 1
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
            self.assertIn("xsd.schema_path_reused", codes)
            self.assertIn("xsd.schema_digest_reused", codes)
            self.assertIn("xsd.schema_source_reused", codes)
            self.assertIn("xsd.blocked_source_reused", codes)
            self.assertIn("xsd.blocked_source_digest_reused", codes)
            self.assertIn("xsd.fixture_path_reused", codes)
            self.assertIn("xsd.fixture_digest_reused", codes)
            self.assertFalse(
                any(
                    key.startswith("_")
                    for xsd_summary in summary["xsd_summaries"]
                    for key in xsd_summary
                )
            )

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
            self.assertIn("trust.profile_id_reused", codes)
            self.assertIn("trust.bundle_digest_reused", codes)

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
            self.assertIn("non-finite numeric constant NaN", stderr)

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
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
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

            self.assertEqual(rc, 0, stderr)
            diagnostic = json.loads(stdout)
            self.assertTrue(diagnostic["ok"])
            self.assertEqual(
                diagnostic["xsd_summaries"][0]["blocked_schema_source_count"],
                3,
            )
            self.assertEqual(diagnostic["blockers"], [])
            self.assertEqual(
                {warning["code"] for warning in diagnostic["warnings"]},
                {
                    "xsd.strict_schema_backed_not_proven",
                    "xsd.profile_schema_backed_not_proven",
                    "xsd.missing_schema_fixtures",
                    "xsd.missing_profile_schema_versions",
                },
            )

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
            )

            manifest = xsd_test.minimal_manifest()
            schema_only_id = "fooo.001.001.02"
            schema_only_payload = "FooPayloadV2"
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
            self.assertEqual(len(schema_only["schema_only_entries"]), 1)
            schema_only_reason = json.loads(schema_only_summary.read_text(encoding="utf-8"))
            schema_only_reason["schema_only_entries"][0]["reason"] = "forged gap"
            schema_only_id_mismatch = json.loads(
                schema_only_summary.read_text(encoding="utf-8")
            )
            schema_only_id_mismatch["schema_only_entries"][0][
                "message_def_id"
            ] = "fooo.001.001.03"
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
                            "--allow-reviewed-xsd-gaps",
                        ]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

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

            fixture_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            fixture_count["verified_fixtures"] += 1
            fixture_count["schema_backed_fixtures"] += 1
            cases.append((fixture_count, "xsd.fixture_count_mismatch"))

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
                            "--allow-reviewed-xsd-gaps",
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

            bad_filename = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_filename["schemas"][0]["source"]["path"] = "xsd/other.001.001.01.xsd"
            blocker_cases.append((bad_filename, "xsd.schema_source_path_mismatch"))

            bad_license = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_license["schemas"][0]["source"]["license"] = "NOASSERTION"
            blocker_cases.append((bad_license, "xsd.schema_source_license_invalid"))

            bad_digest = json.loads(xsd_summary.read_text(encoding="utf-8"))
            bad_digest["schemas"][0]["source"]["sha256"] = "0" * 64
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

            bad_commit = json.loads(xsd_summary.read_text(encoding="utf-8"))
            attach_blocked_sources(bad_commit, [xsd_test.blocked_schema_source()])
            bad_commit["blocked_schema_sources"][0]["source"]["commit"] = (
                "89abcdef0123456789abcdef0123456789abcdeZ"
            )
            blocker_cases.append((bad_commit, "xsd.blocked_source_commit_invalid"))

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

    def test_forged_xsd_profile_catalog_metadata_blocks_readiness(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []

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

            profile_catalog_backed_count = json.loads(xsd_summary.read_text(encoding="utf-8"))
            profile_catalog_backed_count["profile_catalog"]["schema_backed_versions"] += 1
            cases.append(
                (
                    profile_catalog_backed_count,
                    "xsd.profile_catalog_schema_backed_count_mismatch",
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

    def test_tampered_xsd_or_evidence_summary_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            tampered_xsd = json.loads(xsd_summary.read_text(encoding="utf-8"))
            tampered_xsd["verified_schemas"] = 99
            tampered_xsd_path = write_json(root / "tampered-xsd.summary.json", tampered_xsd)

            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(tampered_xsd_path), "--evidence-summary", str(evidence_summary)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

            tampered_evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
            tampered_evidence["ok"] = False
            tampered_evidence_path = write_json(root / "tampered-evidence.summary.json", tampered_evidence)
            rc, _stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(tampered_evidence_path)]
            )
            self.assertEqual(rc, 2)
            self.assertIn("summary_sha256 mismatch", stderr)

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

    def test_missing_evidence_policy_flag_is_malformed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xsd_summary = write_strict_xsd_summary(root / "xsd")
            evidence_summary = add_archive_receipt_verification(
                write_evidence_summary(root / "evidence")
            )
            cases = []
            for key in sorted(READINESS.EVIDENCE_POLICY_KEYS):
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
                "version",
                lambda evidence: set_source_field(
                    evidence,
                    "version",
                    "replace-before-production",
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
            cases.append((duplicate, "stage_names must not contain duplicates"))
            unsupported = json.loads(evidence_summary.read_text(encoding="utf-8"))
            canary = unsupported["canary_summaries"][0]
            canary["stage_names"].append("diagnostic")
            extra_window = dict(canary["stage_windows"][0])
            extra_window["name"] = "diagnostic"
            canary["stage_windows"].append(extra_window)
            cases.append((unsupported, "stage_names contains unsupported stages"))
            for offset, (body, message) in enumerate(cases):
                with self.subTest(message=message):
                    refresh_digest(body)
                    mutated_path = write_json(root / f"stage-names-{offset}.summary.json", body)

                    rc, _stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

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
                    canary["stage_names"] = stage_names
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
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.policy_not_require_verified", codes)
            self.assertIn("trust.no_signature_or_x509_pins", codes)

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
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("trust.policy_unsupported", codes)
            self.assertNotIn("trust.policy_not_require_verified", codes)

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
                    "bad-crl-digest",
                    lambda profile: profile["x509_crl_der"][0].__setitem__(
                        "sha256",
                        "not-a-digest",
                    ),
                    "sha256 must be a lowercase SHA-256 digest",
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
            self.assertIn("trust.bundle_digest_reused", codes)

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
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipts_not_reverified", codes)

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
            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.policy.allow_canary_stage_receipts_only", codes)
            self.assertNotIn("evidence.archive_receipts_not_reverified", codes)

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
            evidence["receipt_verification"]["receipts"][0]["receipt_sha256"] = "f" * 64
            refresh_digest(evidence["receipt_verification"])
            refresh_digest(evidence)
            mutated_path = write_json(root / "unbound-receipts.summary.json", evidence)

            rc, stdout, stderr = run_readiness(
                ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
            )

            self.assertEqual(rc, 1, stderr)
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_missing_canary_digest", codes)

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
            codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
            self.assertIn("evidence.archive_receipt_unreferenced_digest", codes)

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
            )
            for name, summary_path, receipt_index, mutate in cases:
                with self.subTest(name=name):
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = evidence
                    for key in summary_path:
                        receipt_summary = receipt_summary[key]
                    mutate(receipt_summary["receipts"][receipt_index])
                    refresh_digest(receipt_summary)
                    refresh_digest(evidence)
                    mutated_path = write_json(root / f"{name}-metadata.summary.json", evidence)

                    rc, stdout, stderr = run_readiness(
                        ["--xsd-summary", str(xsd_summary), "--evidence-summary", str(mutated_path)]
                    )

                    self.assertEqual(rc, 1, stderr)
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn("evidence.archive_receipt_canary_metadata_mismatch", codes)

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
                    evidence = json.loads(evidence_summary.read_text(encoding="utf-8"))
                    receipt_summary = nested(evidence, path_parts)
                    receipt_summary["receipts"][0]["receipt_kind"] = "diagnostic-receipt"
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
                    codes = {blocker["code"] for blocker in json.loads(stdout)["blockers"]}
                    self.assertIn(code, codes)

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
                    "canary-failed-status",
                    ("canary_summaries", 0, "receipt_summary"),
                    lambda receipt: receipt.update({"ok": False, "status_code": 503}),
                    "evidence.receipt_not_successful",
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
                    "archive-redirect-status",
                    ("receipt_verification",),
                    lambda receipt: receipt.update({"ok": False, "status_code": 302}),
                    "evidence.archive_receipt_not_successful",
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
