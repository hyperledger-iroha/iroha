# Aggregate and foundational-prerequisite cases for production readiness.

import sccp_release_common as RELEASE_CRYPTO

def test_complete_aggregate_readiness_passes(tmp_path: Path) -> None:
    write_all_gates(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["status"] == "ready"
    assert payload["signer_qualification"] == "software-key-qualified"
    assert payload["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert payload["foundational_prerequisites"]["valid"] is True
    assert payload["foundational_prerequisites"]["required_ids"] == list(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert payload["foundational_prerequisites"]["prerequisite_count"] == len(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert payload["foundational_prerequisites"]["signer_backend"] == "software"
    assert payload["foundational_prerequisites"]["signer_service_id"] == (
        FOUNDATIONAL_SIGNER_SERVICE_ID
    )
    assert payload["foundational_prerequisites"]["signer_administrator_id"] == (
        FOUNDATIONAL_SIGNER_ADMINISTRATOR_ID
    )
    assert payload["foundational_prerequisites"]["signer_key_revision"] == 7
    assert payload["foundational_prerequisites"]["signer_policy_revision"] == 11
    assert payload["foundational_prerequisites"]["signer_policy_digest_sha256"] == (
        FOUNDATIONAL_SIGNER_POLICY_DIGEST
    )
    grouped = payload["foundational_prerequisites"][
        "prerequisite_readiness_summary_sha256"
    ]
    assert [row["id"] for row in grouped] == list(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    global_digests = {
        row["gate"]: row["sha256"]
        for row in payload["foundational_prerequisites"]["lane_summary_sha256"]
    }
    for row in grouped:
        expected_gates = MODULE.FOUNDATIONAL_PREREQUISITE_LANES[row["id"]]
        assert [item["gate"] for item in row["readiness_summary_sha256"]] == list(
            expected_gates
        )
        assert row["readiness_summary_sha256"] == [
            {"gate": gate_name, "sha256": global_digests[gate_name]}
            for gate_name in expected_gates
        ]
    assert "signature" not in payload["foundational_prerequisites"]
    assert payload["required"]["gateway_load"]["valid"] is True
    assert payload["required"]["gateway_load"]["path"] == "gateway_load.json"
    assert payload["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }


def test_aggregate_rejects_hsm_qualification_claim(tmp_path: Path) -> None:
    """The revised policy never permits an HSM-qualified readiness claim."""

    write_all_gates(tmp_path)
    summary_path = tmp_path / "aggregate.json"
    assert run_gate(tmp_path, "--summary-out", str(summary_path)) == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["signer_qualification"] = "hsm-qualified"
    errors: list[str] = []
    MODULE.validate_aggregate_summary_output(
        summary,
        MODULE.DEFAULT_REQUIRED_GATES,
        errors,
    )
    assert any("must be software-key-qualified or unqualified" in error for error in errors)


def test_aggregate_replay_cross_binds_foundational_and_required_lane_digests(
    tmp_path: Path,
) -> None:
    """A self-consistent foundational substitution cannot diverge from lane rows."""

    write_all_gates(tmp_path)
    summary_path = tmp_path / "aggregate.json"
    assert run_gate(tmp_path, "--summary-out", str(summary_path)) == 0
    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    gate_name = "reputation"
    substituted_digest = hashlib.sha256(
        b"substituted-foundational-reputation-summary"
    ).hexdigest()
    lane_row = next(
        row
        for row in summary["foundational_prerequisites"]["lane_summary_sha256"]
        if row["gate"] == gate_name
    )
    lane_row["sha256"] = substituted_digest
    group = next(
        row
        for row in summary["foundational_prerequisites"][
            "prerequisite_readiness_summary_sha256"
        ]
        if row["id"] == "SFM-1"
    )
    group["readiness_summary_sha256"][0]["sha256"] = substituted_digest

    errors: list[str] = []
    MODULE.validate_aggregate_summary_output(
        summary,
        MODULE.DEFAULT_REQUIRED_GATES,
        errors,
    )
    assert (
        "reputation aggregate foundational lane digest must match required row sha256"
        in errors
    )


def test_aggregate_ready_requires_exact_canonical_ordered_17_gate_tuple() -> None:
    assert (
        MODULE.aggregate_summary_status([], MODULE.DEFAULT_REQUIRED_GATES)
        == "ready"
    )
    assert (
        MODULE.aggregate_summary_status([], MODULE.DEFAULT_REQUIRED_GATES[:-1])
        == MODULE.NON_PROMOTABLE_STATUS
    )
    assert (
        MODULE.aggregate_summary_status(
            [],
            tuple(reversed(MODULE.DEFAULT_REQUIRED_GATES)),
        )
        == MODULE.NON_PROMOTABLE_STATUS
    )
    assert (
        MODULE.aggregate_summary_status(
            ["canonical aggregate blocker"],
            MODULE.DEFAULT_REQUIRED_GATES,
        )
        == "blocked"
    )


def test_complete_aggregate_without_topology_qualification_stays_blocked(
    tmp_path: Path,
) -> None:
    """Configuration omission can never fall through to a ready aggregate."""

    write_all_gates(tmp_path)
    write_foundational_summary(tmp_path)
    summary = tmp_path / "missing-topology-aggregate.json"
    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path),
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *resilience_cli_args(tmp_path),
                *foundational_cli_args(tmp_path),
                "--summary-out",
                str(summary),
            ]
        )
        == 2
    )
    assert not summary.exists()


def test_aggregate_rejects_lane_and_envelope_topology_substitution(
    tmp_path: Path,
) -> None:
    """One changed topology digest blocks lane and signed-envelope paths."""

    write_all_gates(tmp_path)
    lane_path = tmp_path / "gateway_load.json"
    lane = json.loads(lane_path.read_text(encoding="utf-8"))
    lane["topology_qualification"]["manifest_sha256"] = hashlib.sha256(
        b"foreign-lane-topology"
    ).hexdigest()
    write_json(lane_path, lane)
    summary = tmp_path / "lane-mismatch-aggregate.json"
    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    diagnostics = "\n".join(
        json.loads(summary.read_text(encoding="utf-8"))["errors"]
    )
    assert "must match the reviewed topology" in diagnostics

    summary.unlink()
    write_gate(tmp_path, "gateway_load")
    foundation = foundational_summary(
        lane_summary_sha256=lane_summary_digests(tmp_path)
    )
    foundation["topology_qualification"]["manifest_sha256"] = hashlib.sha256(
        b"foreign-envelope-topology"
    ).hexdigest()
    resign_foundational_summary(foundation)
    write_foundational_summary(tmp_path, foundation)
    summary = tmp_path / "envelope-mismatch-aggregate.json"
    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    diagnostics = "\n".join(
        json.loads(summary.read_text(encoding="utf-8"))["errors"]
    )
    assert "must match the reviewed topology" in diagnostics


def test_full_aggregate_rejects_lane_summary_bytes_swapped_after_signing(
    tmp_path: Path,
) -> None:
    """The software-signed envelope must bind the exact reviewed lane summary byte set."""

    write_all_gates(tmp_path)
    write_foundational_summary(tmp_path)
    gateway_summary_path = tmp_path / "gateway_load.json"
    gateway_summary_path.write_bytes(gateway_summary_path.read_bytes() + b"\n")
    summary_path = tmp_path / "aggregate.json"

    assert run_gate(tmp_path, "--summary-out", str(summary_path)) == 1
    result = json.loads(summary_path.read_text(encoding="utf-8"))
    diagnostics = "\n".join(result["errors"])
    assert result["status"] == "blocked"
    assert result["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert (
        "foundational prerequisite lane summary binding for gateway_load does "
        "not match the supplied readiness summary"
        in diagnostics
    )
    assert result["foundational_prerequisites"]["valid"] is False


def test_foundational_prerequisite_schema_inventories_are_closed() -> None:
    assert MODULE.FOUNDATIONAL_PREREQUISITE_FIELDS == {
        "schema",
        "status",
        "deployment",
        "generated_at_unix",
        "release_sequence",
        "previous_envelope_sha256",
        "topology_qualification",
        "resilience_qualification",
        "l1_lane_evidence_inventory_sha256",
        "prerequisites",
        "lane_summaries",
        "signature",
        "signer_receipt_bundle",
    }
    assert RECEIPT_SUPPORT.SIGNER_EVIDENCE.FOUNDATIONAL_SIGNER_RECEIPT_BUNDLE_FIELDS == {
        "schema", "verifier_sha256", "operation_id_hex", "binding_base64",
        "receipt_base64", "validation_sha256",
    }
    assert MODULE.FOUNDATIONAL_PREREQUISITE_DEPLOYMENT_FIELDS == {
        "deployment_id",
        "environment",
    }
    assert MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_FIELDS == {
        "administrator_id",
        "algorithm",
        "backend",
        "key_revision",
        "policy_digest_sha256",
        "policy_revision",
        "public_key_fingerprint_sha256",
        "service_id",
        "signature_hex",
    }
    assert MODULE.FOUNDATIONAL_PREREQUISITE_ROW_FIELDS == {
        "id",
        "status",
        "evidence_anchor_sha256",
        "evidence_generated_at_unix",
        "readiness_summary_sha256",
    }
    assert MODULE.FOUNDATIONAL_LANE_SUMMARY_ROW_FIELDS == {
        "gate",
        "sha256",
    }
    assert MODULE.AGGREGATE_FOUNDATIONAL_PREREQUISITE_READINESS_SUMMARY_ROW_FIELDS == {
        "id",
        "readiness_summary_sha256",
    }
    assert MODULE.FOUNDATIONAL_PREREQUISITE_LANES == {
        "SFM-1": ("reputation",),
        "SF-1": ("reference_sdk_release",),
        "SF-2": ("pdp",),
        "SF-2c": ("por", "potr"),
        "SF-3": ("gateway_compliance",),
        "SF-4": ("repair",),
        "SF-5b": ("gateway_load",),
        "SF-6": (
            "appeal_finance",
            "governance_dag",
            "hedging_billing",
            "orderbook",
            "reserve_rent",
        ),
        "SF-8a": (
            "ai_prescreen",
            "moderation_panel",
            "pop_credentials",
            "transparency",
        ),
    }
    assert tuple(MODULE.FOUNDATIONAL_PREREQUISITE_LANES) == (
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    mapped_lanes = [
        gate_name
        for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS
        for gate_name in MODULE.FOUNDATIONAL_PREREQUISITE_LANES[prerequisite_id]
    ]
    assert len(mapped_lanes) == len(set(mapped_lanes))
    assert set(mapped_lanes) == set(MODULE.DEFAULT_REQUIRED_GATES)


def test_foundational_prerequisite_readiness_groups_reject_resigned_attacks(
    tmp_path: Path,
) -> None:
    """Signed prerequisite rows must preserve their exact mapped lane digests."""

    def signed_mutation(mutator) -> dict:
        payload = foundational_summary()
        mutator(payload)
        resign_foundational_summary(payload)
        return payload

    def prerequisite_row(payload: dict, prerequisite_id: str) -> dict:
        return next(
            row for row in payload["prerequisites"] if row["id"] == prerequisite_id
        )

    def remove_legacy_field(payload: dict) -> None:
        prerequisite_row(payload, "SFM-1").pop("readiness_summary_sha256")

    def remove_mapped_lane(payload: dict) -> None:
        prerequisite_row(payload, "SF-6")["readiness_summary_sha256"].pop()

    def add_foreign_lane(payload: dict) -> None:
        global_by_gate = {
            row["gate"]: row for row in payload["lane_summaries"]
        }
        prerequisite_row(payload, "SF-6")["readiness_summary_sha256"].append(
            copy.deepcopy(global_by_gate["gateway_load"])
        )

    def reorder_mapped_lanes(payload: dict) -> None:
        prerequisite_row(payload, "SF-6")["readiness_summary_sha256"].reverse()

    def duplicate_mapped_lane(payload: dict) -> None:
        rows = prerequisite_row(payload, "SF-6")["readiness_summary_sha256"]
        rows.append(copy.deepcopy(rows[0]))

    def duplicate_mapped_digest(payload: dict) -> None:
        rows = prerequisite_row(payload, "SF-6")["readiness_summary_sha256"]
        rows[1]["sha256"] = rows[0]["sha256"]

    def mismatch_grouped_digest(payload: dict) -> None:
        prerequisite_row(payload, "SF-5b")["readiness_summary_sha256"][0][
            "sha256"
        ] = "55" * 32

    cases = (
        (
            "legacy-missing-field",
            remove_legacy_field,
            ".readiness_summary_sha256 must be an array",
        ),
        (
            "missing-lane",
            remove_mapped_lane,
            ".readiness_summary_sha256 is missing required gates",
        ),
        (
            "extra-lane",
            add_foreign_lane,
            ".readiness_summary_sha256 contains unknown gates",
        ),
        (
            "reordered-lanes",
            reorder_mapped_lanes,
            "must match the exact canonical readiness lanes for its prerequisite id",
        ),
        (
            "duplicate-lane",
            duplicate_mapped_lane,
            ".readiness_summary_sha256 must not contain duplicate gates",
        ),
        (
            "duplicate-digest",
            duplicate_mapped_digest,
            ".readiness_summary_sha256 must use unique summary digests",
        ),
        (
            "digest-mismatch",
            mismatch_grouped_digest,
            "grouped digest must match foundational lane_summaries",
        ),
    )

    for index, (name, mutator, expected_error) in enumerate(cases):
        exit_code, result = run_foundational_case(
            tmp_path / f"foundation-readiness-group-{index:02d}-{name}",
            signed_mutation(mutator),
        )
        assert exit_code == 1, name
        diagnostics = "\n".join(result["errors"])
        assert expected_error in diagnostics, name
        assert result["status"] == "blocked", name
        assert result["foundational_prerequisites"]["valid"] is False, name


def test_foundational_prerequisites_reject_schema_set_freshness_and_context_attacks(
    tmp_path: Path,
    capsys,
) -> None:
    """Exercise signed semantic attacks independently of signature forgery."""

    def signed_mutation(mutator) -> dict:
        payload = foundational_summary()
        mutator(payload)
        resign_foundational_summary(payload)
        return payload

    cases: list[tuple[str, dict, str]] = []

    cases.append(
        (
            "missing-id",
            signed_mutation(lambda payload: payload["prerequisites"].pop()),
            "foundational prerequisites are missing required ids",
        )
    )

    def add_unknown_id(payload: dict) -> None:
        row = copy.deepcopy(payload["prerequisites"][-1])
        row["id"] = "SF-99"
        row["evidence_anchor_sha256"] = hashlib.sha256(b"SF-99").hexdigest()
        payload["prerequisites"].append(row)

    cases.append(
        (
            "extra-id",
            signed_mutation(add_unknown_id),
            "foundational prerequisites contain unknown ids",
        )
    )
    cases.append(
        (
            "duplicate-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][-1].__setitem__(
                    "id", payload["prerequisites"][0]["id"]
                )
            ),
            "foundational prerequisites must not contain duplicate ids",
        )
    )
    cases.append(
        (
            "reordered-ids",
            signed_mutation(lambda payload: payload["prerequisites"].reverse()),
            "foundational prerequisites must match the exact required set and canonical order",
        )
    )
    cases.append(
        (
            "duplicate-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][1].__setitem__(
                    "evidence_anchor_sha256",
                    payload["prerequisites"][0]["evidence_anchor_sha256"],
                )
            ),
            "foundational prerequisites must use unique evidence anchors",
        )
    )
    cases.append(
        (
            "zero-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_anchor_sha256", "00" * 32
                )
            ),
            "evidence_anchor_sha256 must not be zero",
        )
    )
    cases.append(
        (
            "uppercase-anchor",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_anchor_sha256",
                    payload["prerequisites"][0]["evidence_anchor_sha256"].upper(),
                )
            ),
            "evidence_anchor_sha256 must be canonical lowercase SHA-256",
        )
    )
    cases.append(
        (
            "failed-row",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "status", "failed"
                )
            ),
            ".status must be `verified`",
        )
    )
    cases.append(
        (
            "failed-envelope",
            signed_mutation(lambda payload: payload.__setitem__("status", "failed")),
            "foundational prerequisite status must be `verified`",
        )
    )
    cases.append(
        (
            "stale-envelope",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "generated_at_unix",
                    NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
                )
            ),
            "foundational prerequisite generated_at_unix exceeds max summary artifact age",
        )
    )
    cases.append(
        (
            "future-envelope",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "generated_at_unix", NOW_UNIX + 1
                )
            ),
            "foundational prerequisite generated_at_unix must not be future",
        )
    )
    cases.append(
        (
            "stale-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix",
                    NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
                )
            ),
            "evidence_generated_at_unix exceeds max summary artifact age",
        )
    )
    cases.append(
        (
            "future-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix", NOW_UNIX + 1
                )
            ),
            "evidence_generated_at_unix must not be future",
        )
    )
    cases.append(
        (
            "post-envelope-evidence",
            signed_mutation(
                lambda payload: payload["prerequisites"][0].__setitem__(
                    "evidence_generated_at_unix", GENERATED_AT + 1
                )
            ),
            "evidence_generated_at_unix must not be later than the signed envelope",
        )
    )
    cases.append(
        (
            "mixed-deployment",
            signed_mutation(
                lambda payload: payload["deployment"].__setitem__(
                    "deployment_id", "sorafs-mainnet-2026-07"
                )
            ),
            "foundational prerequisite deployment_id must match --deployment-id",
        )
    )
    cases.append(
        (
            "mixed-environment",
            signed_mutation(
                lambda payload: payload["deployment"].__setitem__(
                    "environment", "prod"
                )
            ),
            "foundational prerequisite environment must match --environment",
        )
    )
    cases.append(
        (
            "unicode-control-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][2].__setitem__(
                    "id", "SF-\u202e2"
                )
            ),
            ".id must be a canonical string",
        )
    )
    cases.append(
        (
            "unicode-homoglyph-id",
            signed_mutation(
                lambda payload: payload["prerequisites"][2].__setitem__(
                    "id", "S\uff26-2"
                )
            ),
            "foundational prerequisites contain unknown ids",
        )
    )
    cases.append(
        (
            "boolean-sequence",
            signed_mutation(
                lambda payload: payload.__setitem__("release_sequence", True)
            ),
            "foundational prerequisite release_sequence must be an integer in 1..2^63-1",
        )
    )
    cases.append(
        (
            "zero-predecessor-after-genesis",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "previous_envelope_sha256", "00" * 32
                )
            ),
            "foundational prerequisite sequence after 1 must use a non-zero predecessor",
        )
    )
    cases.append(
        (
            "rollback-sequence",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "release_sequence", FOUNDATIONAL_RELEASE_SEQUENCE - 1
                )
            ),
            "release_sequence must match the operator-reviewed expected value",
        )
    )
    cases.append(
        (
            "replay-wrong-predecessor",
            signed_mutation(
                lambda payload: payload.__setitem__(
                    "previous_envelope_sha256", "22" * 32
                )
            ),
            "previous_envelope_sha256 must match the operator-reviewed expected digest",
        )
    )
    cases.append(
        (
            "wrong-algorithm",
            signed_mutation(
                lambda payload: payload["signature"].__setitem__(
                    "algorithm", "ed25519ph"
                )
            ),
            "signature algorithm must be `ed25519`",
        )
    )
    for name, field, value, expected_error in (
        (
            "hsm-backend",
            "backend",
            "hsm",
            "signer backend must be `software`",
        ),
        (
            "shared-administrator",
            "administrator_id",
            FOUNDATIONAL_SIGNER_SERVICE_ID,
            "service_id and administrator_id must differ",
        ),
        (
            "zero-key-revision",
            "key_revision",
            0,
            "signer key_revision must be in 1..2^63-1",
        ),
        (
            "zero-policy-digest",
            "policy_digest_sha256",
            "00" * 32,
            "policy_digest_sha256 must be non-zero canonical lowercase SHA-256",
        ),
    ):
        cases.append(
            (
                name,
                signed_mutation(
                    lambda payload, field=field, value=value: payload[
                        "signature"
                    ].__setitem__(field, value)
                ),
                expected_error,
            )
        )

    secret_path = "../../runtime-only-private-key-material"

    def add_path_payload(payload: dict) -> None:
        payload["prerequisites"][0]["evidence_path"] = secret_path

    cases.append(
        (
            "traversal-payload-field",
            signed_mutation(add_path_payload),
            "fields must match the schema-closed contract",
        )
    )

    def add_raw_secret(payload: dict) -> None:
        payload["raw_payload"] = "runtime-only-secret-payload"

    cases.append(
        (
            "raw-secret-field",
            signed_mutation(add_raw_secret),
            "is not allowed",
        )
    )

    for index, (name, payload, expected_error) in enumerate(cases):
        exit_code, result = run_foundational_case(
            tmp_path / f"foundation-semantic-{index:02d}-{name}",
            payload,
        )
        assert exit_code == 1, name
        diagnostics = "\n".join(result["errors"])
        assert expected_error in diagnostics, name
        assert result["status"] == "blocked", name
        assert result["foundational_prerequisites"]["valid"] is False, name
        assert "signature" not in result["foundational_prerequisites"], name
        captured = capsys.readouterr()
        rendered = diagnostics + captured.err + captured.out
        assert secret_path not in rendered, name
        assert "runtime-only-secret-payload" not in rendered, name


def test_foundational_prerequisites_reject_signature_digest_and_trust_attacks(
    tmp_path: Path,
) -> None:
    """Reject forgeries, self-selected signers, and non-canonical signatures."""

    inventory_root = tmp_path / "signature-inventory-baseline"
    inventory_root.mkdir()
    write_gate(inventory_root, "gateway_load")
    inventory_path, _lanes, _topology = lane_inventory_fixture(inventory_root)
    inventory_sha256 = hashlib.sha256(inventory_path.read_bytes()).hexdigest()

    def baseline() -> dict:
        return foundational_summary(
            l1_lane_evidence_inventory_sha256=inventory_sha256
        )

    cases: list[tuple[str, dict, str]] = []

    forged_signature = baseline()
    signature_hex = forged_signature["signature"]["signature_hex"]
    forged_signature["signature"]["signature_hex"] = (
        ("0" if signature_hex[0] != "0" else "1") + signature_hex[1:]
    )
    cases.append(
        (
            "forged-signature",
            forged_signature,
            "foundational prerequisite signature verification failed",
        )
    )

    forged_digest = baseline()
    forged_digest["prerequisites"][0]["evidence_anchor_sha256"] = "33" * 32
    cases.append(
        (
            "forged-digest",
            forged_digest,
            "foundational prerequisite signature verification failed",
        )
    )

    malleable_signature = baseline()
    signature = bytes.fromhex(malleable_signature["signature"]["signature_hex"])
    scalar = int.from_bytes(signature[32:], "little") + RELEASE_CRYPTO._ED_L  # noqa: SLF001
    malleable_signature["signature"]["signature_hex"] = (
        signature[:32] + scalar.to_bytes(32, "little")
    ).hex()
    cases.append(
        (
            "non-canonical-scalar",
            malleable_signature,
            "foundational prerequisite signature verification failed",
        )
    )

    alternate_signer = baseline()
    resign_foundational_summary(alternate_signer, seed=bytes.fromhex("2f" * 32))
    cases.append(
        (
            "self-selected-signer",
            alternate_signer,
            "signer fingerprint must match the operator-trusted key",
        )
    )

    wrong_fingerprint = baseline()
    wrong_fingerprint["signature"]["public_key_fingerprint_sha256"] = "44" * 32
    wrong_fingerprint["signature"]["signature_hex"] = "00" * 64
    wrong_fingerprint["signature"]["signature_hex"] = ed25519_sign(
        FOUNDATIONAL_SIGNING_SEED,
        MODULE.foundational_signing_payload(wrong_fingerprint),
    ).hex()
    cases.append(
        (
            "forged-fingerprint",
            wrong_fingerprint,
            "signer fingerprint must match the operator-trusted key",
        )
    )

    zero_signature = baseline()
    zero_signature["signature"]["signature_hex"] = "00" * 64
    cases.append(
        (
            "zero-signature",
            zero_signature,
            "signature must be a non-zero canonical Ed25519 signature",
        )
    )

    uppercase_signature = baseline()
    uppercase_signature["signature"]["signature_hex"] = uppercase_signature[
        "signature"
    ]["signature_hex"].upper()
    cases.append(
        (
            "uppercase-signature",
            uppercase_signature,
            "signature must be a non-zero canonical Ed25519 signature",
        )
    )

    for index, (name, payload, expected_error) in enumerate(cases):
        exit_code, result = run_foundational_case(
            tmp_path / f"foundation-signature-{index:02d}-{name}",
            payload,
        )
        assert exit_code == 1, name
        diagnostics = "\n".join(result["errors"])
        assert expected_error in diagnostics, name
        assert result["status"] == "blocked", name
        assert result["foundational_prerequisites"]["valid"] is False, name


def test_foundational_prerequisite_missing_duplicate_and_untrusted_inputs_block(
    tmp_path: Path,
) -> None:
    missing_root = tmp_path / "missing-foundation"
    missing_root.mkdir()
    lane_path = write_json(
        missing_root / "gateway_load.json",
        gate_summary("gateway_load"),
    )
    missing_summary = missing_root / "aggregate.json"
    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane_path),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *topology_cli_args(missing_root),
                *foundational_cli_args(missing_root),
                "--summary-out",
                str(missing_summary),
            ]
        )
        == 1
    )
    missing = json.loads(missing_summary.read_text(encoding="utf-8"))
    assert missing["foundational_prerequisites"] == {
        "schema": MODULE.FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "present": False,
        "valid": False,
        "errors": ["missing required foundational prerequisite summary"],
    }
    assert missing["status"] == "blocked"

    duplicate_root = tmp_path / "duplicate-foundation"
    duplicate_root.mkdir()
    write_gate(duplicate_root, "gateway_load")
    write_json(
        duplicate_root / "foundational_prerequisites_copy.json",
        foundational_summary(),
    )
    duplicate_summary = duplicate_root / "aggregate.json"
    assert (
        run_gate(
            duplicate_root,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(duplicate_summary),
        )
        == 1
    )
    duplicate = json.loads(duplicate_summary.read_text(encoding="utf-8"))
    assert "duplicate foundational prerequisite summary" in duplicate["errors"]
    assert duplicate["foundational_prerequisites"]["valid"] is False

    untrusted_root = tmp_path / "untrusted-foundation"
    untrusted_root.mkdir()
    write_gate(untrusted_root, "gateway_load")
    write_foundational_summary(untrusted_root)
    untrusted_summary = untrusted_root / "aggregate.json"
    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(untrusted_root),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *topology_cli_args(untrusted_root),
                "--summary-out",
                str(untrusted_summary),
            ]
        )
        == 1
    )
    untrusted = json.loads(untrusted_summary.read_text(encoding="utf-8"))
    diagnostics = "\n".join(untrusted["errors"])
    assert "operator-trusted Ed25519 public key" in diagnostics
    assert "operator-reviewed expected value" in diagnostics
    assert "operator-reviewed expected digest" in diagnostics


def test_foundational_prerequisite_path_policy_rejects_symlink_and_traversal(
    tmp_path: Path,
    capsys,
) -> None:
    root = tmp_path / "path-policy"
    root.mkdir()
    lane = write_json(root / "gateway_load.json", gate_summary("gateway_load"))
    target = write_json(root / "foundation-target.json", foundational_summary())
    symlink = root / "foundation-link.json"
    symlink.symlink_to(target)

    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane),
                "--evidence",
                str(symlink),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *topology_cli_args(root),
                *foundational_cli_args(root),
            ]
        )
        == 1
    )
    captured = capsys.readouterr()
    assert "evidence file must not be a symlink" in captured.err
    assert "foundation-target" not in captured.err

    (root / "nested").mkdir()
    traversal = root / "nested" / ".." / "foundation-target.json"
    assert (
        MODULE.main(
            [
                "--evidence",
                str(lane),
                "--evidence",
                str(traversal),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                *topology_cli_args(root),
                *foundational_cli_args(root),
            ]
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "checker-rendered paths" in captured.err


def test_foundational_prerequisite_cli_trust_values_fail_closed_without_echo(
    tmp_path: Path,
    capsys,
) -> None:
    write_gate(tmp_path, "gateway_load")
    malformed_values = (
        (
            "--foundational-prerequisite-signer-public-key-hex",
            "private-key-runtime-only",
            "must be exactly 32 bytes of lowercase hex",
        ),
        (
            "--foundational-prerequisite-signer-public-key-hex",
            "00" * 32,
            "must not be the all-zero key",
        ),
        (
            "--foundational-prerequisite-previous-envelope-sha256",
            "SECRET-PREDECESSOR",
            "must be canonical lowercase SHA-256",
        ),
        (
            "--foundational-prerequisite-release-sequence",
            str(1 << 63),
            "must be in 1..2^63-1",
        ),
    )
    for flag, value, expected_error in malformed_values:
        assert (
            MODULE.main(
                [
                    *topology_cli_args(tmp_path),
                    "--evidence-dir",
                    str(tmp_path),
                    "--require-gate",
                    "gateway_load",
                    "--now-unix",
                    str(NOW_UNIX),
                    "--deployment-id",
                    DEPLOYMENT_ID,
                    "--environment",
                    ENVIRONMENT,
                    *topology_cli_args(tmp_path),
                    *foundational_cli_args(tmp_path),
                    flag,
                    value,
                ]
            )
            == 2
        )
        captured = capsys.readouterr()
        assert expected_error in captured.err
        assert value not in captured.err
        assert captured.out == ""
