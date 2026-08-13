"""Authenticated validation-acknowledgment implementation for the release cache helper."""

def _validation_ack(
    ack_held: dict[str, object],
    receipt_held: dict[str, object],
    source: Path,
    bootstrap_evidence: Path,
    source_manifest_sha256: str,
    candidate_root: Path,
    scaling_evidence_manifest: Path,
    expected_signer_fingerprint: str,
    expected_scaling_trial_harness_sha256: str,
    expected_scaling_configuration_sha256: str,
    expected_scaling_irohad_sha256: str,
    expected_scaling_iroha_cli_sha256: str,
) -> tuple[str, int]:
    path, payload, metadata = ack_held["path"], ack_held["data"], ack_held["metadata"]
    receipt, receipt_payload, receipt_metadata = (
        receipt_held["path"],
        receipt_held["data"],
        receipt_held["metadata"],
    )
    assert (
        isinstance(path, Path)
        and isinstance(payload, bytes)
        and isinstance(metadata, os.stat_result)
        and isinstance(receipt, Path)
        and isinstance(receipt_payload, bytes)
        and isinstance(receipt_metadata, os.stat_result)
    )
    digest, size = hashlib.sha256(payload).hexdigest(), len(payload)
    if stat.S_IMODE(metadata.st_mode) != 0o400 or metadata.st_uid != os.geteuid() or metadata.st_nlink != 1:
        raise CacheCopyError("receipt validation acknowledgment metadata is not exact")
    try:
        value = json.loads(payload)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("receipt validation acknowledgment is malformed") from error
    validator = bootstrap_evidence / "validate-receipt.py"
    completion = bootstrap_evidence / "BOOTSTRAP_COMPLETED.json"
    validator_digest, _, _ = _digest_regular(validator, "archived receipt validator")
    completion_digest, _, _ = _digest_regular(completion, "bootstrap completion")
    receipt_digest, receipt_size = hashlib.sha256(receipt_payload).hexdigest(), len(receipt_payload)
    stdout = f"Sumeragi v2 aggregate release receipt verified: {receipt}\n".encode()
    expected_streams = {
        "stdout": {
            "sha256": hashlib.sha256(stdout).hexdigest(),
            "size_bytes": len(stdout),
        },
        "stderr": {
            "sha256": hashlib.sha256(b"").hexdigest(),
            "size_bytes": 0,
        },
    }
    if not isinstance(value, dict):
        raise CacheCopyError("receipt validation acknowledgment is malformed")
    try:
        receipt_document = json.loads(receipt_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("aggregate receipt is malformed") from error
    if not isinstance(receipt_document, dict):
        raise CacheCopyError("aggregate receipt is malformed")

    def receipt_path(*names: str) -> str:
        item: object = receipt_document
        try:
            for name in names:
                if not isinstance(item, dict):
                    raise KeyError(name)
                item = item[name]
        except KeyError as error:
            raise CacheCopyError(
                "aggregate receipt lacks a validator invocation path"
            ) from error
        if isinstance(item, dict):
            item = item.get("path")
        if not isinstance(item, str):
            raise CacheCopyError(
                "aggregate receipt validator invocation path is malformed"
            )
        return item

    try:
        authentication = receipt_document["authentication"]
        authentication["bootstrap"]
        receipt_document["evidence"]
    except (KeyError, TypeError) as error:
        raise CacheCopyError(
            "aggregate receipt lacks validator invocation authentication"
        ) from error

    completion_payload, _ = _read_regular(completion, "bootstrap completion")
    try:
        completion_document = json.loads(completion_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("bootstrap completion is malformed") from error
    if not isinstance(completion_document, dict):
        raise CacheCopyError("bootstrap completion is malformed")
    trusted_inputs = completion_document.get("trusted_inputs")
    if not isinstance(trusted_inputs, dict):
        raise CacheCopyError("bootstrap completion lacks trusted inputs")

    def trusted_digest(name: str) -> str:
        record = trusted_inputs.get(name)
        if not isinstance(record, dict):
            raise CacheCopyError(
                f"bootstrap completion lacks trusted {name} input"
            )
        value = record.get("sha256")
        if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
            raise CacheCopyError(
                f"bootstrap completion trusted {name} digest is malformed"
            )
        return value

    for name, expected_value in (
        ("expected signer fingerprint", expected_signer_fingerprint),
        (
            "expected scaling trial-harness digest",
            expected_scaling_trial_harness_sha256,
        ),
        (
            "expected scaling configuration digest",
            expected_scaling_configuration_sha256,
        ),
        ("expected scaling irohad digest", expected_scaling_irohad_sha256),
        (
            "expected scaling iroha CLI digest",
            expected_scaling_iroha_cli_sha256,
        ),
    ):
        pattern = r"SHA256:[A-Za-z0-9+/]{43}" if name == "expected signer fingerprint" else r"[0-9a-f]{64}"
        if (
            not isinstance(expected_value, str)
            or re.fullmatch(pattern, expected_value) is None
        ):
            raise CacheCopyError(f"{name} is malformed")

    identity_path = bootstrap_evidence / "candidate-identity.json"
    protected_archives = {
        "attestation": bootstrap_evidence / "identity-attestation.json",
        "transcript": bootstrap_evidence / "identity-transcript.json",
        "raw_commit": bootstrap_evidence / "identity-raw-commit",
        "cargo_lock": bootstrap_evidence / "identity-Cargo.lock",
        "allowed_signers": bootstrap_evidence / "identity-allowed-signers",
        "revocation": bootstrap_evidence / "identity-revocation",
        "git": bootstrap_evidence / "identity-git",
        "ssh_keygen": bootstrap_evidence / "identity-ssh-keygen",
    }
    expected_invocation_values = {
        "--candidate-identity": ("path", str(identity_path)),
        "--sealed-identity": ("path", str(source.parent / "sealed-identity.json")),
        "--release-root": ("path", str(source)),
        "--bootstrap-completion": ("path", str(completion)),
        "--bootstrap-evidence-dir": ("path", str(bootstrap_evidence)),
        "--bootstrap-identity": ("path", str(identity_path)),
        "--bootstrap-attestation": ("path", str(protected_archives["attestation"])),
        "--bootstrap-transcript": ("path", str(protected_archives["transcript"])),
        "--expected-bootstrap-completion-sha256": ("text", completion_digest),
        "--bootstrap-candidate-root": ("path", str(candidate_root)),
        "--bootstrap-runner": (
            "path", str(candidate_root / "scripts" / "run_sumeragi_v2_release_gates.sh")
        ),
        "--signature-attestation": ("path", str(protected_archives["attestation"])),
        "--signature-transcript": ("path", str(protected_archives["transcript"])),
        "--signature-raw-commit": ("path", str(protected_archives["raw_commit"])),
        "--signature-cargo-lock": ("path", str(protected_archives["cargo_lock"])),
        "--signature-allowed-signers": ("path", str(protected_archives["allowed_signers"])),
        "--signature-revocation": ("path", str(protected_archives["revocation"])),
        "--signature-git": ("path", str(protected_archives["git"])),
        "--signature-ssh-keygen": ("path", str(protected_archives["ssh_keygen"])),
        "--expected-git-sha256": ("text", trusted_digest("git")),
        "--expected-ssh-keygen-sha256": ("text", trusted_digest("ssh_keygen")),
        "--expected-allowed-signers-sha256": ("text", trusted_digest("allowed_signers")),
        "--expected-revocation-sha256": ("text", trusted_digest("revocation")),
        "--expected-signer-fingerprint": ("text", expected_signer_fingerprint),
        "--corridor-completion": (
            "path", receipt_path("evidence", "corridor_completion")
        ),
        "--formal-completion": (
            "path", receipt_path("evidence", "formal_completion")
        ),
        "--seed-completion": (
            "path", receipt_path("evidence", "seed_matrix_completion")
        ),
        "--chaos-completion": (
            "path", receipt_path("evidence", "chaos_completion")
        ),
        "--taira-completion": (
            "path", receipt_path("evidence", "taira_completion")
        ),
        "--g4p-completion": (
            "path", receipt_path("evidence", "g4p_multilane", "completion")
        ),
        "--g12-seed-completion": (
            "path",
            receipt_path("evidence", "g12_cross_dataspace", "seed_completion"),
        ),
        "--g12-fault-soak-completion": (
            "path",
            receipt_path(
                "evidence", "g12_cross_dataspace", "fault_soak_completion"
            ),
        ),
        "--scaling-evidence-manifest": ("path", str(scaling_evidence_manifest)),
        "--sdk-dependency-archive": (
            "path", str(source.parent / "sdk-dependency-bundle.tar")
        ),
        "--sdk-dependency-input-inventory": (
            "path", str(source.parent / "sdk-dependency-input.json")
        ),
        "--sdk-dependency-final-work-inventory": (
            "path", str(source.parent / "sdk-dependency-work-final.json")
        ),
        "--runtime-tool-probe-manifest": (
            "path", str(source.parent / "runtime-tool-probe-manifest.json")
        ),
        "--runtime-tool-probe-result": (
            "path", str(source.parent / "runtime-tool-probe-result.json")
        ),
        "--expected-scaling-trial-harness-sha256": (
            "text", expected_scaling_trial_harness_sha256
        ),
        "--expected-scaling-configuration-sha256": (
            "text", expected_scaling_configuration_sha256
        ),
        "--expected-scaling-irohad-sha256": (
            "text", expected_scaling_irohad_sha256
        ),
        "--expected-scaling-iroha-cli-sha256": (
            "text", expected_scaling_iroha_cli_sha256
        ),
        "--repository-root": ("path", str(source)),
        "--output": ("path", str(receipt)),
        "--verify-existing": ("flag", True),
        "--validation-ack": ("path", str(path)),
        "--source-manifest-sha256": ("text", source_manifest_sha256),
    }
    _validate_validator_invocation(
        value.get("invocation"),
        expected_values=expected_invocation_values,
    )
    if (
        set(value) != {"format", "schema_version", "profile", "sealed_source", "receipt", "validator", "invocation", "exit_status", "stdout", "stderr"}
        or value["format"] != "iroha-sumeragi-v2-receipt-validation-ack"
        or type(value["schema_version"]) is not int or value["schema_version"] != 3
        or value["profile"] != "release"
        or value["sealed_source"] != {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": source_manifest_sha256,
        }
        or not isinstance(value["receipt"], dict)
        or type(value["receipt"].get("size_bytes")) is not int
        or value["receipt"] != {
            "archive_id": "release-terminal.receipt.v1",
            "mode": f"{stat.S_IMODE(receipt_metadata.st_mode):04o}",
            "sha256": receipt_digest,
            "size_bytes": receipt_size,
        }
        or not isinstance(value["validator"], dict)
        or set(value["validator"]) != {"archive_id", "sha256", "bootstrap_completion_sha256"}
        or value["validator"] != {
            "archive_id": "release-bootstrap.receipt-validator.v1",
            "sha256": validator_digest,
            "bootstrap_completion_sha256": completion_digest,
        }
        or type(value["exit_status"]) is not int or value["exit_status"] != 0
        or not all(
            isinstance(value[name], dict)
            and type(value[name].get("size_bytes")) is int
            for name in ("stdout", "stderr")
        )
        or value["stdout"] != expected_streams["stdout"] or value["stderr"] != expected_streams["stderr"]
        or payload != _canonical_payload(value)
    ):
        raise CacheCopyError("receipt validation acknowledgment contract is not exact")
    return digest, size

