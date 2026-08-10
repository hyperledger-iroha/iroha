# Payload-free aggregate artifact cases for production readiness.


def test_artifact_fingerprint_metadata_must_be_payload_free(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["fingerprint"]["optional"] = None
    payload["recognized_artifacts"][0]["fingerprint"]["optional"] = None
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        ".fingerprint.optional must contain only payload-free canonical metadata"
        in "\n".join(result["errors"])
    )
