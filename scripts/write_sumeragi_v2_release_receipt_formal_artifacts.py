# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

def _validate_multilane_apalache_evidence(
    snapshot: EvidenceSnapshot,
    workspace_source_manifest: str,
    multilane_source_manifest: str,
) -> None:
    data = snapshot.data
    if (
        len(data) > _MAX_SCALING_JSON_BYTES
        or not data.endswith(b"\n")
        or b"\r" in data
        or b"\0" in data
    ):
        raise ReceiptError(
            "formal multilane Apalache evidence is not bounded LF-only TSV"
        )
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(
            "formal multilane Apalache evidence is not UTF-8"
        ) from error
    expected_header = [
        "schema_version\t2",
        "backend\tapalache",
        f"version\t{_APALACHE_VERSION}",
        f"launcher_sha256\t{_APALACHE_LAUNCHER_SHA256}",
        f"jar_sha256\t{_APALACHE_JAR_SHA256}",
        f"workspace_source_manifest_sha256\t{workspace_source_manifest}",
        f"multilane_source_manifest_sha256\t{multilane_source_manifest}",
        f"result_count\t{len(_APALACHE_RESULTS)}",
    ]
    if len(lines) != len(expected_header) + len(_APALACHE_RESULTS):
        raise ReceiptError(
            "formal multilane Apalache evidence has the wrong result inventory"
        )
    if lines[: len(expected_header)] != expected_header:
        raise ReceiptError(
            "formal multilane Apalache evidence header is not the exact pinned profile"
        )
    for index, expected in enumerate(_APALACHE_RESULTS):
        fields = lines[len(expected_header) + index].split("\t")
        expected_prefix = ("result", *expected, "NoError")
        if (
            len(fields) != 9
            or tuple(fields[:6]) != expected_prefix
            or any(_DIGEST_RE.fullmatch(value) is None for value in fields[6:])
        ):
            raise ReceiptError(
                "formal multilane Apalache evidence result "
                f"{index} is not exact source-bound NoError evidence"
            )


def _validate_formal_snapshot_replays(
    *,
    snapshots: dict[str, EvidenceSnapshot],
    checker: Path,
    checker_environment: dict[str, str],
    repo_root: Path,
) -> None:
    """Run retained formal validators over private copies of captured bytes."""

    replay_keys = (
        "ledger",
        "evidence",
        "verus_evidence",
        "verus_log",
        "cross_tool_evidence",
        "production_trace_extraction_evidence",
    )
    with tempfile.TemporaryDirectory(
        prefix="sumeragi-v2-formal-snapshot-replay-"
    ) as temporary:
        replay_root = Path(temporary).resolve(strict=True)
        replay_paths: dict[str, Path] = {}
        for key in replay_keys:
            snapshot = snapshots[key]
            destination = replay_root / snapshot.path.name
            try:
                destination.write_bytes(snapshot.data)
                destination.chmod(0o400)
            except OSError as error:
                raise ReceiptError(
                    "formal snapshot replay could not materialize captured evidence"
                ) from error
            replay_paths[key] = destination

        cross_tool_status, cross_tool_stdout, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--print-cross-tool-obligations",
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal cross-tool validator",
        )
        if cross_tool_status != 0:
            raise ReceiptError(
                "archived formal ledger has an invalid cross-tool evidence requirement"
            )
        if not cross_tool_stdout.strip():
            raise ReceiptError(
                "archived formal release ledger does not require cross-tool evidence"
            )

        verus_checker = (
            repo_root / "scripts" / "formal" / "sumeragi_v2_verus_evidence.py"
        )
        verus_status, _, _ = _run_bounded_python_validator(
            verus_checker,
            [
                "validate",
                "--root",
                str(repo_root),
                "--evidence",
                str(replay_paths["verus_evidence"]),
                "--log",
                str(replay_paths["verus_log"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal Verus validator",
        )
        if verus_status != 0:
            raise ReceiptError("archived formal Verus evidence failed validation")

        status, _, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--release",
                "--evidence",
                str(replay_paths["evidence"]),
                "--verus-evidence",
                str(replay_paths["verus_evidence"]),
                "--verus-log",
                str(replay_paths["verus_log"]),
                "--cross-tool-evidence",
                str(replay_paths["cross_tool_evidence"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal release validator",
        )
        if status != 0:
            raise ReceiptError(
                "archived formal ledger/evidence failed release validation"
            )

        # Recompute the canonical theorem certificate from the sealed checkout
        # and the private artifact snapshots. A stored boolean or certificate
        # digest alone is never release authority.
        trace_status, _, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--release",
                "--evidence",
                str(replay_paths["evidence"]),
                "--verus-evidence",
                str(replay_paths["verus_evidence"]),
                "--verus-log",
                str(replay_paths["verus_log"]),
                "--cross-tool-evidence",
                str(replay_paths["cross_tool_evidence"]),
                "--production-trace-extraction-evidence",
                str(replay_paths["production_trace_extraction_evidence"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="authenticated production trace-extraction validator",
        )
        if trace_status != 0:
            raise ReceiptError(
                "archived formal evidence does not authenticate production "
                "trace extraction"
            )


def _formal_artifacts(
    completion: EvidenceSnapshot,
    fields: dict[str, str],
    sealed: dict[str, Any],
    checker_environment: dict[str, str],
    repo_root: Path,
) -> tuple[
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
]:
    completion_path = completion.path
    checker = repo_root / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
    expected_completion_fields = {
        "schema_version",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "formal_gate_log_sha256",
        "proof_coverage_sha256",
        "proof_evidence_sha256",
        "verus_evidence_sha256",
        "verus_log_sha256",
        "multilane_apalache_evidence_sha256",
        "cross_tool_evidence_sha256",
        "production_trace_extraction_evidence_sha256",
        "harness_cargo_lock_sha256",
        "formal_toolchain_sha256",
        "tlaps_resource_jsonl_sha256",
        "tlaps_resource_summary_sha256",
    }
    if fields.get("schema_version") != "2":
        raise ReceiptError(
            "formal completion is release-ineligible without authenticated "
            "production trace-extraction evidence"
        )
    _require_fields(
        fields,
        expected_completion_fields,
        "formal completion",
    )
    expected = {
        "schema_version": "2",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
    }
    if any(fields.get(name) != value for name, value in expected.items()):
        raise ReceiptError("formal completion is not bound to the release identity")

    artifact_specs = (
        (
            "gate_log",
            completion_path.with_name("formal-gate.log"),
            "formal_gate_log_sha256",
            "formal gate log",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "ledger",
            completion_path.with_name("proof_coverage.json"),
            "proof_coverage_sha256",
            "formal proof ledger",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "evidence",
            completion_path.with_name("proof_evidence.json"),
            "proof_evidence_sha256",
            "formal proof evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "verus_evidence",
            completion_path.with_name("verus_evidence.json"),
            "verus_evidence_sha256",
            "formal Verus evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "verus_log",
            completion_path.with_name("verus.log"),
            "verus_log_sha256",
            "formal Verus log",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "multilane_apalache_evidence",
            completion_path.with_name("multilane_apalache_evidence.tsv"),
            "multilane_apalache_evidence_sha256",
            "formal multilane Apalache evidence",
            _MAX_RELEASE_TSV_BYTES,
        ),
        (
            "cross_tool_evidence",
            completion_path.with_name("cross_tool_evidence.json"),
            "cross_tool_evidence_sha256",
            "formal cross-tool evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "production_trace_extraction_evidence",
            completion_path.with_name(
                "production_trace_extraction_evidence.json"
            ),
            "production_trace_extraction_evidence_sha256",
            "formal production trace-extraction evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "harness_lock",
            completion_path.with_name("harness-Cargo.lock"),
            "harness_cargo_lock_sha256",
            "formal harness lock",
            _MAX_LOCK_BYTES,
        ),
        (
            "toolchain",
            completion_path.with_name("formal-toolchain.tsv"),
            "formal_toolchain_sha256",
            "formal toolchain",
            _MAX_RELEASE_TSV_BYTES,
        ),
        (
            "tlaps_resource_jsonl",
            completion_path.with_name("tlaps_resource.jsonl"),
            "tlaps_resource_jsonl_sha256",
            "TLAPS resource samples",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "tlaps_resource_summary",
            completion_path.with_name("tlaps_resource_summary.json"),
            "tlaps_resource_summary_sha256",
            "TLAPS resource summary",
            _MAX_RELEASE_JSON_BYTES,
        ),
    )
    snapshots: dict[str, EvidenceSnapshot] = {}
    for key, path, digest_field, name, maximum_bytes in artifact_specs:
        snapshot = _bounded_evidence_snapshot(
            path,
            name,
            maximum_bytes=maximum_bytes,
        )
        if snapshot.sha256 != fields[digest_field]:
            raise ReceiptError(f"{name} digest mismatch")
        snapshots[key] = snapshot
    gate_log = snapshots["gate_log"]
    ledger = snapshots["ledger"]
    evidence = snapshots["evidence"]
    verus_evidence = snapshots["verus_evidence"]
    verus_log = snapshots["verus_log"]
    multilane_apalache_evidence = snapshots["multilane_apalache_evidence"]
    cross_tool_evidence = snapshots["cross_tool_evidence"]
    production_trace_extraction_evidence = snapshots[
        "production_trace_extraction_evidence"
    ]
    harness_lock = snapshots["harness_lock"]
    toolchain_snapshot = snapshots["toolchain"]
    tlaps_resource_jsonl = snapshots["tlaps_resource_jsonl"]
    tlaps_resource_summary = snapshots["tlaps_resource_summary"]
    _validate_formal_snapshot_replays(
        snapshots=snapshots,
        checker=checker,
        checker_environment=checker_environment,
        repo_root=repo_root,
    )
    trace = _decode_canonical_json(
        production_trace_extraction_evidence.data,
        "production trace-extraction evidence",
    )
    if trace.get("workspace_source_manifest_sha256") != sealed[
        "workspace_source_manifest_sha256"
    ]:
        raise ReceiptError(
            "production trace extraction is not bound to the sealed workspace"
        )
    multilane_source_manifest = trace.get("multilane_source_manifest_sha256")
    if (
        not isinstance(multilane_source_manifest, str)
        or _DIGEST_RE.fullmatch(multilane_source_manifest) is None
    ):
        raise ReceiptError(
            "production trace extraction lacks its multilane source manifest"
        )
    _validate_multilane_apalache_evidence(
        multilane_apalache_evidence,
        sealed["workspace_source_manifest_sha256"],
        multilane_source_manifest,
    )
    _validate_tlaps_resource_evidence(
        tlaps_resource_jsonl, tlaps_resource_summary
    )
    if fields["harness_cargo_lock_sha256"] != _HARNESS_LOCK_SHA256:
        raise ReceiptError("formal harness lock is not the pinned dependency graph")
    toolchain = _tsv_fields_from_snapshot(
        toolchain_snapshot, "formal toolchain"
    )
    _require_fields(
        toolchain,
        {
            "schema_version",
            "java_path",
            "java_sha256",
            "tlapm_path",
            "tlapm_sha256",
            "tla2tools_path",
            "tla2tools_sha256",
            "verus_path",
            "verus_sha256",
            "cargo_verus_path",
            "cargo_verus_sha256",
            "tlc_profile",
            "tlaps_threads",
        },
        "formal toolchain",
    )
    if (
        toolchain["schema_version"] != "1"
        or toolchain["tlc_profile"] != "ci"
        or toolchain["tlaps_threads"] != "1"
    ):
        raise ReceiptError("formal toolchain does not describe the pinned release profile")
    for tool in ("java", "tlapm", "tla2tools", "verus", "cargo_verus"):
        raw_path = Path(toolchain[f"{tool}_path"])
        if not raw_path.is_absolute():
            raise ReceiptError(f"formal {tool} path is not absolute")
        tool_snapshot = _bounded_evidence_snapshot(
            raw_path,
            f"formal {tool} tool",
            maximum_bytes=_MAX_TOOL_BYTES,
            require_single_link=False,
        )
        digest = toolchain[f"{tool}_sha256"]
        if not _DIGEST_RE.fullmatch(digest) or tool_snapshot.sha256 != digest:
            raise ReceiptError(f"formal {tool} tool digest mismatch")
    log_lines = _decode_lf_text(gate_log, "formal gate log").splitlines()
    if (
        not log_lines
        or log_lines[-1] != _FORMAL_FINAL_MARKER
        or log_lines.count(_FORMAL_FINAL_MARKER) != 1
    ):
        raise ReceiptError("formal gate log lacks its one exact final success marker")
    return (
        _snapshot_contract(gate_log),
        _snapshot_contract(ledger),
        _snapshot_contract(evidence),
        _snapshot_contract(verus_evidence),
        _snapshot_contract(verus_log),
        _snapshot_contract(multilane_apalache_evidence),
        _snapshot_contract(cross_tool_evidence),
        _snapshot_contract(production_trace_extraction_evidence),
        _snapshot_contract(harness_lock),
        _snapshot_contract(toolchain_snapshot),
        _snapshot_contract(tlaps_resource_jsonl),
        _snapshot_contract(tlaps_resource_summary),
    )
