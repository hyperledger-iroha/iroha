# Executed lexically in write_sumeragi_v2_release_receipt.py after exact digest authentication.

def _canonical_production_tests(
    repo_root: Path,
    runner_snapshot: EvidenceSnapshot | None = None,
) -> list[str]:
    if runner_snapshot is None:
        runner_snapshot = _bounded_evidence_snapshot(
            repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
            "release runner inventory",
            maximum_bytes=_MAX_POLICY_BYTES,
            require_single_link=False,
        )
    source = _decode_lf_text(runner_snapshot, "release runner inventory")
    marker = "required_production_liveness_tests=(\n"
    if source.count(marker) != 1:
        raise ReceiptError("release runner lacks one canonical production inventory")
    body = source.split(marker, 1)[1].split("\n)", 1)[0]
    tests = [line.strip() for line in body.splitlines() if line.strip()]
    if (
        len(tests) != _PRODUCTION_TEST_COUNT
        or len(set(tests)) != _PRODUCTION_TEST_COUNT
        or any(
            not test.startswith(
                (
                    "sumeragi::",
                    "sumeragi_v2_runner::",
                    "block::",
                    "offline::",
                    "zk::",
                    "merge_sidecar::",
                    "state::",
                    "kura::",
                    "nexus::",
                    "peer::",
                    "network::",
                    "consensus_message_control::tests::",
                    "network_relay_tests::",
                    "tests::relay_fairness::",
                    "parameters::",
                )
            )
            for test in tests
        )
    ):
        raise ReceiptError(
            "release runner production inventory is not exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    return tests


def _canonical_g_unit_rows(
    repo_root: Path,
    runner_snapshot: EvidenceSnapshot | None = None,
) -> list[tuple[str, str, str]]:
    if runner_snapshot is None:
        runner_snapshot = _bounded_evidence_snapshot(
            repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
            "release runner G-UNIT inventory",
            maximum_bytes=_MAX_POLICY_BYTES,
            require_single_link=False,
        )
    source = _decode_lf_text(runner_snapshot, "release runner G-UNIT inventory")
    rows: list[tuple[str, str, str]] = []
    for array_name, leg_id, package, expected_count, cargo_target in _G_UNIT_GROUPS:
        marker = f"{array_name}=(\n"
        if source.count(marker) != 1:
            raise ReceiptError(
                f"release runner lacks one canonical {array_name} G-UNIT inventory"
            )
        body = source.split(marker, 1)[1].split("\n)", 1)[0]
        tests = [
            line.strip()
            for line in body.splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        if (
            len(tests) != expected_count
            or len(set(tests)) != expected_count
            or any(
                re.fullmatch(
                    (
                        r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+"
                        if cargo_target == "lib"
                        else r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*"
                    ),
                    test,
                )
                is None
                for test in tests
            )
        ):
            raise ReceiptError(
                f"release runner {array_name} inventory is not exactly "
                f"{expected_count} distinct tests"
            )
        rows.extend((leg_id, package, test) for test in tests)
    names = [test for _, _, test in rows]
    if len(rows) != _G_UNIT_TEST_COUNT or len(set(names)) != _G_UNIT_TEST_COUNT:
        raise ReceiptError(
            f"release runner G-UNIT inventory is not exactly "
            f"{_G_UNIT_TEST_COUNT} globally distinct tests"
        )
    return rows


def _g_unit_leg_command(array_name: str, package: str, cargo_target: str) -> str:
    if cargo_target == "lib":
        target = "--lib"
    elif cargo_target.startswith("test:"):
        test_target = cargo_target.removeprefix("test:")
        if re.fullmatch(r"[A-Za-z0-9_]+", test_target) is None:
            raise ReceiptError("G-UNIT test target is not canonical")
        target = f"--test {test_target}"
    else:
        raise ReceiptError("G-UNIT Cargo target is not canonical")
    return (
        f"for test in {array_name}; do cargo test --locked --offline "
        f'-p {package} {target} "$test" -- --exact --test-threads=1; done'
    )


def _production_module_command(module: str) -> str:
    if module == "sumeragi_v2_runner":
        return (
            "cargo test --locked --offline -p integration_tests --test "
            "sumeragi_v2_runner_isolated "
            f"{_PRODUCTION_INTEGRATION_MODULE} -- --test-threads=1"
        )
    if module in {
        "peer::run::tests",
        "network::tests",
        "network::inbound_source_memory_bound_tests",
        "network::handle_update_tests",
    }:
        return (
            "cargo test --locked --offline -p iroha_p2p --lib "
            f"{module} -- --test-threads=1"
        )
    if module in {
        "consensus_message_control::tests",
        "network_relay_tests",
        "tests::relay_fairness",
    }:
        return (
            "cargo test --locked --offline -p irohad --lib "
            "--features test-network-message-control "
            f"{module} -- --test-threads=1"
        )
    if module.startswith("parameters::"):
        return (
            "cargo test --locked --offline -p iroha_config --lib "
            f"{module} -- --test-threads=1"
        )
    if module in _DATA_MODEL_PRODUCTION_MODULES:
        return (
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{module} -- --test-threads=1"
        )
    return (
        "cargo test --locked --offline -p iroha_core --lib "
        f"{module} -- --test-threads=1"
    )

def _load_identity(
    path: Path, name: str
) -> tuple[EvidenceSnapshot, dict[str, Any]]:
    snapshot = _bounded_evidence_snapshot(
        path,
        name,
        maximum_bytes=_MAX_SIGNATURE_JSON_BYTES,
    )
    value = _decode_canonical_json(snapshot.data, name)
    if not isinstance(value, dict) or set(value) != _IDENTITY_KEYS:
        raise ReceiptError(f"{name} fields do not match the release identity schema")
    if type(value.get("schema_version")) is not int or value["schema_version"] != 1:
        raise ReceiptError(f"{name} has the wrong schema version")
    for field in ("head_commit", "head_tree", "index_tree"):
        item = value.get(field)
        if not isinstance(item, str) or not _OBJECT_ID_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase Git object ID")
    object_widths = {
        len(value[field]) for field in ("head_commit", "head_tree", "index_tree")
    }
    if len(object_widths) != 1:
        raise ReceiptError(f"{name} mixes Git object formats")
    for field in ("workspace_source_manifest_sha256", "cargo_lock_sha256"):
        item = value.get(field)
        if not isinstance(item, str) or not _DIGEST_RE.fullmatch(item):
            raise ReceiptError(f"{name}.{field} is not a lowercase SHA-256 digest")
    if value["head_tree"] != value["index_tree"]:
        raise ReceiptError(f"{name} does not describe one clean Git tree")
    return snapshot, value


def _load_tsv(
    path: Path,
    name: str,
    *,
    maximum_bytes: int = _MAX_RELEASE_TSV_BYTES,
) -> tuple[EvidenceSnapshot, dict[str, str]]:
    snapshot = _bounded_evidence_snapshot(
        path,
        name,
        maximum_bytes=maximum_bytes,
    )
    return snapshot, _tsv_fields_from_snapshot(snapshot, name)


def _require_fields(fields: dict[str, str], expected: set[str], name: str) -> None:
    if set(fields) != expected:
        raise ReceiptError(f"{name} fields do not match its completion schema")


def _artifact(snapshot: EvidenceSnapshot | PathContract) -> dict[str, str]:
    return {"path": str(snapshot.path), "sha256": snapshot.sha256}
def _tlaps_resource_int(value: Any, name: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        raise ReceiptError(f"{name} is not one bounded integer")
    return value

def _tlaps_resource_float(value: Any, name: str) -> float:
    if type(value) is not float or not math.isfinite(value) or value < 0.0:
        raise ReceiptError(f"{name} is not one finite non-negative decimal")
    return value

def _tlaps_resource_timestamp(value: Any, name: str) -> datetime:
    if (
        not isinstance(value, str)
        or _TLAPS_RESOURCE_TIMESTAMP_RE.fullmatch(value) is None
    ):
        raise ReceiptError(f"{name} is not one canonical UTC timestamp")
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%S.%fZ")
    except ValueError as error:
        raise ReceiptError(f"{name} is not one valid UTC timestamp") from error

def _validate_tlaps_resource_evidence(
    jsonl_snapshot: EvidenceSnapshot,
    summary_snapshot: EvidenceSnapshot,
) -> None:
    """Validate the exact successful resource-guard stream and its summary."""

    data = jsonl_snapshot.data
    if not data or not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError("TLAPS resource samples are not canonical LF-only JSONL")
    lines = data.splitlines(keepends=True)
    if len(lines) > _MAX_TLAPS_RESOURCE_RECORDS:
        raise ReceiptError("TLAPS resource samples exceed the record-count bound")
    records = [
        _decode_canonical_json(line, f"TLAPS resource record {index}")
        for index, line in enumerate(lines)
    ]
    if len(records) < 4:
        raise ReceiptError(
            "TLAPS resource samples lack start, spawn, sample, and summary records"
        )

    summary = _decode_canonical_json(
        summary_snapshot.data, "TLAPS resource summary"
    )
    summary_fields = {
        "child_exit_code",
        "ended_utc",
        "event",
        "exit_reason",
        "exit_status",
        "evidence_peak_rss_bytes",
        "kernel_peak_rss_bytes",
        "kernel_peak_rss_method",
        "kernel_peak_rss_scope",
        "memory_limit_bytes",
        "memory_enforcement_mode",
        "physical_footprint_interval_seconds",
        "peak_memory_bytes",
        "peak_physical_footprint_bytes",
        "peak_rss_bytes",
        "report_context",
        "sample_count",
        "sample_interval_seconds",
        "schema_version",
        "started_utc",
        "supervisor_pid",
    }
    _require_exact_json_fields(summary, summary_fields, "TLAPS resource summary")
    if records[-1] != summary:
        raise ReceiptError(
            "TLAPS resource samples do not terminate in the exact published summary"
        )

    start = _require_exact_json_fields(
        records[0],
        {
            "event",
            "memory_limit_bytes",
            "memory_enforcement_mode",
            "physical_footprint_interval_seconds",
            "report_context",
            "sample_interval_seconds",
            "schema_version",
            "started_utc",
            "supervisor_pid",
        },
        "TLAPS resource start record",
    )
    spawn = _require_exact_json_fields(
        records[1],
        {
            "event",
            "process_group_id",
            "schema_version",
            "timestamp_utc",
            "wrapper_pid",
        },
        "TLAPS resource spawn record",
    )
    sample_records = records[2:-1]
    sample_fields = {
        "accounting_method",
        "elapsed_seconds",
        "event",
        "memory_bytes",
        "memory_limit_bytes",
        "physical_footprint_bytes",
        "process_count",
        "process_group_id",
        "rss_bytes",
        "schema_version",
        "timestamp_utc",
    }

    summary_schema = _tlaps_resource_int(
        summary.get("schema_version"), "TLAPS resource summary.schema_version"
    )
    summary_child_status = _tlaps_resource_int(
        summary.get("child_exit_code"), "TLAPS resource summary.child_exit_code"
    )
    summary_exit_status = _tlaps_resource_int(
        summary.get("exit_status"), "TLAPS resource summary.exit_status"
    )
    summary_limit = _tlaps_resource_int(
        summary.get("memory_limit_bytes"),
        "TLAPS resource summary.memory_limit_bytes",
        minimum=1,
    )
    summary_samples = _tlaps_resource_int(
        summary.get("sample_count"),
        "TLAPS resource summary.sample_count",
        minimum=1,
    )
    summary_supervisor = _tlaps_resource_int(
        summary.get("supervisor_pid"),
        "TLAPS resource summary.supervisor_pid",
        minimum=2,
    )
    peak_memory = _tlaps_resource_int(
        summary.get("peak_memory_bytes"),
        "TLAPS resource summary.peak_memory_bytes",
    )
    peak_rss = _tlaps_resource_int(
        summary.get("peak_rss_bytes"), "TLAPS resource summary.peak_rss_bytes"
    )
    peak_footprint = _tlaps_resource_int(
        summary.get("peak_physical_footprint_bytes"),
        "TLAPS resource summary.peak_physical_footprint_bytes",
    )
    kernel_peak_rss = _tlaps_resource_int(
        summary.get("kernel_peak_rss_bytes"),
        "TLAPS resource summary.kernel_peak_rss_bytes",
    )
    evidence_peak_rss = _tlaps_resource_int(
        summary.get("evidence_peak_rss_bytes"),
        "TLAPS resource summary.evidence_peak_rss_bytes",
    )
    summary_sample_interval = _tlaps_resource_float(
        summary.get("sample_interval_seconds"),
        "TLAPS resource summary.sample_interval_seconds",
    )
    summary_footprint_interval = _tlaps_resource_float(
        summary.get("physical_footprint_interval_seconds"),
        "TLAPS resource summary.physical_footprint_interval_seconds",
    )
    started = _tlaps_resource_timestamp(
        summary.get("started_utc"), "TLAPS resource summary.started_utc"
    )
    ended = _tlaps_resource_timestamp(
        summary.get("ended_utc"), "TLAPS resource summary.ended_utc"
    )
    expected_kernel_method = (
        "wait4_ru_maxrss" if kernel_peak_rss > 0 else "unavailable"
    )
    if (
        summary_schema != 1
        or summary.get("event") != "summary"
        or summary.get("exit_reason") != "completed"
        or summary_child_status != 0
        or summary_exit_status != 0
        or summary_limit != _TLAPS_RESOURCE_MEMORY_LIMIT_BYTES
        or summary.get("memory_enforcement_mode")
        != _TLAPS_RESOURCE_MEMORY_ENFORCEMENT_MODE
        or summary_sample_interval
        != _TLAPS_RESOURCE_SAMPLE_INTERVAL_SECONDS
        or summary_footprint_interval
        != _TLAPS_RESOURCE_PHYSICAL_FOOTPRINT_INTERVAL_SECONDS
        or summary.get("report_context") is not None
        or summary.get("kernel_peak_rss_method") != expected_kernel_method
        or summary.get("kernel_peak_rss_scope") != "direct_guarded_body"
        or summary_samples != len(sample_records)
        or ended < started
        or peak_memory > summary_limit
        or kernel_peak_rss > summary_limit
        or evidence_peak_rss > summary_limit
        or evidence_peak_rss != max(peak_rss, kernel_peak_rss)
    ):
        raise ReceiptError(
            "TLAPS resource summary is not a successful bounded release run"
        )

    start_schema = _tlaps_resource_int(
        start.get("schema_version"), "TLAPS resource start.schema_version"
    )
    start_limit = _tlaps_resource_int(
        start.get("memory_limit_bytes"),
        "TLAPS resource start.memory_limit_bytes",
        minimum=1,
    )
    start_supervisor = _tlaps_resource_int(
        start.get("supervisor_pid"),
        "TLAPS resource start.supervisor_pid",
        minimum=2,
    )
    start_sample_interval = _tlaps_resource_float(
        start.get("sample_interval_seconds"),
        "TLAPS resource start.sample_interval_seconds",
    )
    start_footprint_interval = _tlaps_resource_float(
        start.get("physical_footprint_interval_seconds"),
        "TLAPS resource start.physical_footprint_interval_seconds",
    )
    start_time = _tlaps_resource_timestamp(
        start.get("started_utc"), "TLAPS resource start.started_utc"
    )
    if (
        start_schema != 1
        or start.get("event") != "start"
        or start_limit != summary_limit
        or start.get("memory_enforcement_mode")
        != _TLAPS_RESOURCE_MEMORY_ENFORCEMENT_MODE
        or start_sample_interval != summary_sample_interval
        or start_footprint_interval != summary_footprint_interval
        or start.get("report_context") is not None
        or start_supervisor != summary_supervisor
        or start.get("started_utc") != summary.get("started_utc")
        or start_time != started
    ):
        raise ReceiptError("TLAPS resource start record is not bound to its summary")

    spawn_schema = _tlaps_resource_int(
        spawn.get("schema_version"), "TLAPS resource spawn.schema_version"
    )
    process_group_id = _tlaps_resource_int(
        spawn.get("process_group_id"),
        "TLAPS resource spawn.process_group_id",
        minimum=2,
    )
    wrapper_pid = _tlaps_resource_int(
        spawn.get("wrapper_pid"), "TLAPS resource spawn.wrapper_pid", minimum=2
    )
    spawn_time = _tlaps_resource_timestamp(
        spawn.get("timestamp_utc"), "TLAPS resource spawn.timestamp_utc"
    )
    if (
        spawn_schema != 1
        or spawn.get("event") != "spawn"
        or process_group_id == wrapper_pid
        or process_group_id == summary_supervisor
        or wrapper_pid == summary_supervisor
        or spawn_time < started
        or spawn_time > ended
    ):
        raise ReceiptError("TLAPS resource spawn record is not one guarded body")

    observed_memory: list[int] = []
    observed_rss: list[int] = []
    observed_footprint: list[int] = []
    previous_elapsed = -1.0
    previous_timestamp = spawn_time
    for index, raw_sample in enumerate(sample_records):
        sample = _require_exact_json_fields(
            raw_sample, sample_fields, f"TLAPS resource sample record {index}"
        )
        sample_schema = _tlaps_resource_int(
            sample.get("schema_version"),
            f"TLAPS resource sample {index}.schema_version",
        )
        sample_limit = _tlaps_resource_int(
            sample.get("memory_limit_bytes"),
            f"TLAPS resource sample {index}.memory_limit_bytes",
            minimum=1,
        )
        sample_group = _tlaps_resource_int(
            sample.get("process_group_id"),
            f"TLAPS resource sample {index}.process_group_id",
            minimum=2,
        )
        _tlaps_resource_int(
            sample.get("process_count"),
            f"TLAPS resource sample {index}.process_count",
            minimum=1,
        )
        memory = _tlaps_resource_int(
            sample.get("memory_bytes"),
            f"TLAPS resource sample {index}.memory_bytes",
        )
        rss = _tlaps_resource_int(
            sample.get("rss_bytes"), f"TLAPS resource sample {index}.rss_bytes"
        )
        footprint = _tlaps_resource_int(
            sample.get("physical_footprint_bytes"),
            f"TLAPS resource sample {index}.physical_footprint_bytes",
        )
        elapsed = _tlaps_resource_float(
            sample.get("elapsed_seconds"),
            f"TLAPS resource sample {index}.elapsed_seconds",
        )
        timestamp = _tlaps_resource_timestamp(
            sample.get("timestamp_utc"),
            f"TLAPS resource sample {index}.timestamp_utc",
        )
        accounting_method = sample.get("accounting_method")
        if accounting_method == "rss":
            accounting_is_exact = footprint == 0 and memory == rss
        elif accounting_method == _TLAPS_RESOURCE_MEMORY_ENFORCEMENT_MODE:
            accounting_is_exact = footprint > 0 and memory == max(rss, footprint)
        else:
            accounting_is_exact = False
        if (
            sample_schema != 1
            or sample.get("event") != "sample"
            or sample_limit != summary_limit
            or sample_group != process_group_id
            or memory > sample_limit
            or not accounting_is_exact
            or elapsed < previous_elapsed
            or timestamp < previous_timestamp
            or timestamp > ended
        ):
            raise ReceiptError(
                f"TLAPS resource sample {index} is not exact bounded guard evidence"
            )
        previous_elapsed = elapsed
        previous_timestamp = timestamp
        observed_memory.append(memory)
        observed_rss.append(rss)
        observed_footprint.append(footprint)

    if (
        peak_memory != max(observed_memory)
        or peak_rss != max(observed_rss)
        or peak_footprint != max(observed_footprint)
    ):
        raise ReceiptError(
            "TLAPS resource summary peaks do not match the authenticated sample stream"
        )

def _prebuilt_directory_inventory(
    path: Path, expected_names: set[str], name: str
) -> None:
    names: list[str] = []
    try:
        with os.scandir(path) as iterator:
            for entry in iterator:
                if len(names) >= len(expected_names):
                    raise ReceiptError(
                        f"{name} contains more entries than its exact closed inventory"
                    )
                if (
                    _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(entry.name) is None
                    and entry.name != _PREBUILT_MANIFEST_NAME
                ):
                    raise ReceiptError(f"{name} does not have its exact closed inventory")
                names.append(entry.name)
    except OSError as error:
        raise ReceiptError(f"{name} cannot be enumerated") from error
    if len(names) != len(set(names)) or set(names) != expected_names:
        raise ReceiptError(f"{name} does not have its exact closed inventory")


def _prebuilt_version_transcripts(
    *,
    bundle_dir: Path,
    fields: dict[str, str],
    corridor_fields: dict[str, str],
    private_build_roots_available: bool,
) -> dict[str, dict[str, Any]]:
    tool_specs = (
        ("cargo", Path(corridor_fields["cargo_path"]), (), fields["cargo_version_sha256"]),
        ("rustc", Path(corridor_fields["rustc_path"]), ("-vV",), fields["rustc_version_sha256"]),
    )
    results: dict[str, dict[str, Any]] = {}
    environment = _closed_replay_environment(bundle_dir)
    for tool, executable, arguments, expected_digest in tool_specs:
        tool_label = "Cargo" if tool == "cargo" else tool
        size_text = corridor_fields[f"{tool}_version_size_bytes"]
        if (
            re.fullmatch(r"[1-9][0-9]*", size_text) is None
            or int(size_text) > _MAX_PREBUILT_VERSION_TRANSCRIPT_BYTES
        ):
            raise ReceiptError(
                f"authenticated {tool} version transcript size is not bounded"
            )
        expected_size = int(size_text)
        contract: PathContract | None = None
        if private_build_roots_available:
            contract = _capture_path_contract(
                executable,
                f"authenticated corridor {tool} tool",
                expected_sha256=corridor_fields[f"{tool}_sha256"],
                expected_owner=os.geteuid(),
                expected_nlink=1,
            )
            if contract.mode & 0o111 == 0:
                raise ReceiptError(
                    f"authenticated corridor {tool} tool is not executable"
                )
        if tool == "cargo":
            # This transcript was captured earlier through run_cargo --version.
            stdout = (corridor_fields["cargo_version"] + "\n").encode()
        elif private_build_roots_available:
            assert contract is not None
            status, stdout, stderr = _run_bounded_replay(
                executable,
                arguments,
                cwd=bundle_dir,
                environment=environment,
                name=f"authenticated {tool} version probe",
                maximum_output_bytes=_MAX_PREBUILT_VERSION_TRANSCRIPT_BYTES,
                executable_contract=contract,
            )
            if status != 0 or stderr:
                raise ReceiptError(
                    f"authenticated {tool} version probe did not produce exact stdout"
                )
        else:
            stdout = None
        if stdout is not None:
            if (
                not stdout.endswith(b"\n")
                or b"\r" in stdout
                or b"\0" in stdout
                or len(stdout) != expected_size
            ):
                raise ReceiptError(
                    f"authenticated {tool} version transcript is not exact"
                )
            observed_digest = hashlib.sha256(stdout).hexdigest()
            if observed_digest != expected_digest:
                source = (
                    "policy-captured corridor transcript"
                    if tool == "cargo"
                    else "authenticated tool"
                )
                raise ReceiptError(
                    f"prebuilt manifest {tool_label} version digest does not "
                    f"match the {source}"
                )
            try:
                lines = stdout.decode("utf-8").splitlines()
            except UnicodeDecodeError as error:
                raise ReceiptError(
                    f"authenticated {tool} version probe output is not UTF-8"
                ) from error
            if tool == "cargo":
                if lines != [corridor_fields["cargo_version"]]:
                    raise ReceiptError(
                        "policy-captured Cargo version transcript is not exact"
                    )
            else:
                version = re.fullmatch(
                    r"rustc ([0-9]+\.[0-9]+\.[0-9]+) "
                    r"\(([0-9a-f]{7,40}) ([0-9]{4}-[0-9]{2}-[0-9]{2})\)",
                    corridor_fields["rustc_version"],
                )
                expected_keys = (
                    "binary", "commit-hash", "commit-date", "host", "release",
                    "LLVM version",
                )
                parsed: dict[str, str] = {}
                if (
                    version is None
                    or not lines
                    or lines[0] != corridor_fields["rustc_version"]
                ):
                    raise ReceiptError(
                        "authenticated rustc version probe has the wrong version line"
                    )
                for line in lines[1:]:
                    key, separator, value = line.partition(": ")
                    if not separator or key in parsed or not value:
                        raise ReceiptError(
                            "authenticated rustc version probe is not exact "
                            "rustc -vV output"
                        )
                    parsed[key] = value
                if (
                    tuple(parsed) != expected_keys
                    or parsed["binary"] != "rustc"
                    or re.fullmatch(r"[0-9a-f]{40}", parsed["commit-hash"])
                    is None
                    or not parsed["commit-hash"].startswith(version.group(2))
                    or parsed["commit-date"] != version.group(3)
                    or parsed["host"] != fields["host_triple"]
                    or parsed["release"] != version.group(1)
                    or re.fullmatch(
                        r"[0-9]+\.[0-9]+(?:\.[0-9]+)?",
                        parsed["LLVM version"],
                    )
                    is None
                ):
                    raise ReceiptError(
                        "authenticated rustc version probe is not exact rustc -vV output"
                    )
        else:
            observed_digest = expected_digest
        if contract is not None:
            after = _capture_path_contract(
                executable,
                f"authenticated corridor {tool} tool after version probe",
                expected_sha256=contract.sha256,
                expected_mode=contract.mode,
                expected_owner=contract.owner,
                expected_nlink=contract.nlink,
                expected_size=contract.size,
            )
            if after != contract:
                raise ReceiptError(
                    f"authenticated corridor {tool} tool changed during version probe"
                )
        results[tool] = {
            "operation_id": f"{tool}.version.v1",
            "tool_archive_id": f"release-runner-tool.{tool}.v1",
            "sha256": observed_digest,
            "size_bytes": expected_size,
        }
    return results


def _prebuilt_binary_bundle(
    *,
    manifest_path: Path,
    expected_manifest_sha256: str,
    fields: dict[str, str],
    sealed: dict[str, Any],
    repo_root: Path,
    artifact_root: Path,
    cargo_target_root: Path,
    private_build_roots_available: bool,
) -> dict[str, Any]:
    expected_manifest_sha256 = _require_digest(
        expected_manifest_sha256, "prebuilt binary manifest digest"
    )
    if manifest_path.name != _PREBUILT_MANIFEST_NAME:
        raise ReceiptError("prebuilt binary manifest has the wrong filename")
    expected_programs = (
        artifact_root
        / "sumeragi-v2-release"
        / sealed["workspace_source_manifest_sha256"]
        / "programs"
    )
    bundle_dir = manifest_path.parent
    if (
        bundle_dir.parent != expected_programs
        or _PREBUILT_INVOCATION_RE.fullmatch(bundle_dir.name) is None
    ):
        raise ReceiptError(
            "prebuilt binary manifest is outside its exact source-bound "
            "invocation bundle"
        )
    for path, name in (
        (bundle_dir, "prebuilt invocation bundle"),
        (bundle_dir / "release", "prebuilt release directory"),
        (bundle_dir / "message-control", "prebuilt message-control directory"),
        (
            bundle_dir / "message-control" / "release",
            "prebuilt message-control release directory",
        ),
    ):
        _prebuilt_directory(path, name)
    _prebuilt_directory_inventory(
        bundle_dir,
        {_PREBUILT_MANIFEST_NAME, "release", "message-control"},
        "prebuilt invocation bundle",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "release",
        {"iroha3d", "iroha", "kagami"},
        "prebuilt release directory",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "message-control",
        {"release"},
        "prebuilt message-control directory",
    )
    _prebuilt_directory_inventory(
        bundle_dir / "message-control" / "release",
        {"iroha3d"},
        "prebuilt message-control release directory",
    )

    manifest = _read_evidence_snapshot(
        manifest_path,
        "prebuilt binary manifest",
        maximum_bytes=_MAX_PREBUILT_MANIFEST_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    if manifest.sha256 != expected_manifest_sha256:
        raise ReceiptError(
            "prebuilt binary manifest does not match its externally carried digest"
        )
    rows = _decode_g12_tsv(manifest, "prebuilt binary manifest")
    if (
        len(rows) != len(_PREBUILT_MANIFEST_FIELDS)
        or tuple(row[0] for row in rows) != _PREBUILT_MANIFEST_FIELDS
        or any(len(row) != 2 for row in rows)
    ):
        raise ReceiptError(
            "prebuilt binary manifest does not contain its exact ordered 25 fields"
        )
    manifest_fields = {row[0]: row[1] for row in rows}
    canonical_data = "".join(
        f"{name}\t{manifest_fields[name]}\n"
        for name in _PREBUILT_MANIFEST_FIELDS
    ).encode("utf-8")
    if manifest.data != canonical_data:
        raise ReceiptError("prebuilt binary manifest TSV is not canonical")
    if (
        manifest_fields["schema_version"] != "2"
        or manifest_fields["source_manifest_sha256"]
        != sealed["workspace_source_manifest_sha256"]
        or manifest_fields["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or manifest_fields["profile"] != "release"
        or manifest_fields["bundle_dir"] != str(bundle_dir)
        or _PREBUILT_TRIPLE_RE.fullmatch(manifest_fields["host_triple"]) is None
        or manifest_fields["target_triple"] != manifest_fields["host_triple"]
    ):
        raise ReceiptError(
            "prebuilt binary manifest is not bound to the exact release identity"
        )
    for name in ("cargo_version_sha256", "rustc_version_sha256"):
        _require_digest(manifest_fields[name], f"prebuilt manifest {name}")
    cargo_lock = _read_evidence_snapshot(
        repo_root / "Cargo.lock",
        "retained release Cargo.lock",
        maximum_bytes=_MAX_LOCK_BYTES,
        allowed_owners={os.geteuid()},
    )
    if cargo_lock.sha256 != manifest_fields["cargo_lock_sha256"]:
        raise ReceiptError(
            "prebuilt binary manifest Cargo.lock digest does not match retained source"
        )

    binaries: list[dict[str, Any]] = []
    for prefix, relative in _PREBUILT_BINARY_SPECS:
        size_text = manifest_fields[f"{prefix}_size_bytes"]
        if (
            re.fullmatch(r"[1-9][0-9]*", size_text) is None
            or int(size_text) > _MAX_PREBUILT_BINARY_BYTES
            or manifest_fields[f"{prefix}_relative_path"] != relative
            or manifest_fields[f"{prefix}_mode_octal"] != "0500"
        ):
            raise ReceiptError(
                f"prebuilt manifest {prefix} metadata is not exact and bounded"
            )
        digest = _require_digest(
            manifest_fields[f"{prefix}_sha256"],
            f"prebuilt manifest {prefix} digest",
        )
        pure_relative = PurePosixPath(relative)
        binary = _read_evidence_snapshot(
            bundle_dir.joinpath(*pure_relative.parts),
            f"prebuilt {prefix} binary",
            maximum_bytes=_MAX_PREBUILT_BINARY_BYTES,
            expected_mode=0o500,
            allowed_owners={os.geteuid()},
            executable=True,
            retain_bytes=False,
        )
        if binary.sha256 != digest or binary.size != int(size_text):
            raise ReceiptError(
                f"prebuilt {prefix} binary identity does not match manifest"
            )
        binaries.append(
            {
                "role": prefix,
                "relative_path": relative,
                "archive_id": f"release-prebuilt.binary.{prefix}.v1",
                "mode": f"{binary.mode:04o}",
                "sha256": binary.sha256,
                "size_bytes": binary.size,
            }
        )

    return {
        "schema_version": 3,
        "archive_id": f"release-prebuilt.bundle.v1:{bundle_dir.name}",
        "manifest": {
            "archive_id": "release-prebuilt.manifest.v2",
            "mode": f"{manifest.mode:04o}",
            "sha256": manifest.sha256,
            "size_bytes": manifest.size,
        },
        "source_manifest_sha256": manifest_fields["source_manifest_sha256"],
        "cargo_lock_sha256": manifest_fields["cargo_lock_sha256"],
        "cargo_version_sha256": manifest_fields["cargo_version_sha256"],
        "rustc_version_sha256": manifest_fields["rustc_version_sha256"],
        "host_triple": manifest_fields["host_triple"],
        "target_triple": manifest_fields["target_triple"],
        "profile": manifest_fields["profile"],
        "version_transcripts": _prebuilt_version_transcripts(
            bundle_dir=bundle_dir,
            fields=manifest_fields,
            corridor_fields=fields,
            private_build_roots_available=private_build_roots_available,
        ),
        "binaries": binaries,
    }


def _corridor_artifacts(
    completion: EvidenceSnapshot,
    fields: dict[str, str],
    sealed: dict[str, Any],
    repo_root: Path,
    bootstrap_runner_tools: dict[str, Any],
    bootstrap_trusted_input_digests: dict[str, str],
    *,
    expected_artifact_root: Path,
    expected_cargo_target_root: Path,
    private_build_roots_available: bool,
) -> tuple[
    PathContract,
    PathContract,
    PathContract,
    list[PathContract],
    dict[str, Any], dict[str, Any],
]:
    completion_path = completion.path
    _require_fields(
        fields,
        {
            "schema_version",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "artifact_root_path",
            "cargo_target_root_path",
            "leg_count",
            "production_required_test_count",
            "g_unit_expected_test_count",
            "g_unit_passed_test_count",
            "summary_sha256",
            "production_required_tests_sha256",
            "g_unit_inventory_sha256",
            "java_path",
            "java_sha256",
            "cargo_path",
            "cargo_sha256",
            "cargo_version",
            "cargo_version_size_bytes",
            "rustc_path",
            "rustc_sha256",
            "rustc_version",
            "rustc_version_size_bytes",
            "python3_path",
            "python3_sha256",
            "node_path",
            "node_sha256",
            "swift_path",
            "swift_sha256",
            "swift_version",
            "bash_path",
            "bash_sha256",
            "git_path",
            "git_sha256",
            "cargo_home_path",
            "cargo_cache_input_inventory_path", "cargo_cache_input_inventory_sha256", "cargo_cache_final_inventory_path", "cargo_cache_final_inventory_sha256", "runtime_inventory_path", "runtime_inventory_sha256", "runtime_home_path", "runtime_tmpdir_path", "runtime_tmp_path", "runtime_temp_path", "runtime_cache_path",
            "repo_cargo_config_sha256",
            "native_amx_grouped_fixture_sha256",
            "native_amx_grouped_suite_source_manifest_sha256",
            "native_amx_grouped_negative_control_count",
            "tlc_profile",
            "tlaps_threads",
            "prebuilt_manifest_path",
            "prebuilt_manifest_sha256",
        },
        "corridor completion",
    )
    artifact_root, cargo_target_root = _prebuilt_release_roots(
        repo_root=repo_root,
        fields=fields,
        expected_artifact_root=expected_artifact_root,
        expected_cargo_target_root=expected_cargo_target_root,
        private_build_roots_available=private_build_roots_available,
    )
    expected_identity = {
        "schema_version": "1",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "leg_count": str(len(_corridor_legs())),
        "production_required_test_count": str(_PRODUCTION_TEST_COUNT),
        "g_unit_expected_test_count": str(_G_UNIT_TEST_COUNT),
        "g_unit_passed_test_count": str(_G_UNIT_TEST_COUNT),
        "native_amx_grouped_negative_control_count": str(
            _NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT
        ),
        "tlc_profile": "ci",
        "tlaps_threads": "1",
    }
    if any(fields.get(name) != value for name, value in expected_identity.items()):
        raise ReceiptError("corridor completion is not the exact release preflight")
    if (
        fields["cargo_version"] != "cargo 1.93.1 (083ac5135 2025-12-15)"
        or fields["rustc_version"]
        != "rustc 1.93.1 (01f6ddf75 2026-02-11)"
    ):
        raise ReceiptError("corridor Rust tools do not match rust-toolchain.toml")
    runtime_root = expected_artifact_root.parent / "runtime"
    expected_tool_paths = {
        "java": runtime_root / "java-runtime" / "bin" / "java",
        "cargo": runtime_root / "rust-toolchain" / "bin" / "cargo",
        "rustc": runtime_root / "rust-toolchain" / "bin" / "rustc",
        "python3": runtime_root / "bin" / "python3",
        "node": runtime_root / "bin" / "node",
        "bash": runtime_root / "bin" / "bash",
        "git": runtime_root / "bin" / "git",
    }
    expected_tool_digests = {
        "python3": bootstrap_trusted_input_digests.get("python"),
        "bash": bootstrap_trusted_input_digests.get("bash"),
        "git": bootstrap_trusted_input_digests.get("git"),
    }
    for tool in ("java", "cargo", "rustc", "node", "swift"):
        runner_record = bootstrap_runner_tools.get(tool)
        if (
            not isinstance(runner_record, dict)
            or runner_record.get("archive_name") != f"runner-tools/{tool}"
            or runner_record.get("alias_name") != tool
        ):
            raise ReceiptError(
                f"corridor {tool} is not the authenticated bootstrap runner tool"
            )
        expected_tool_digests[tool] = runner_record.get("sha256")
    for tool in (
        "java",
        "cargo",
        "rustc",
        "python3",
        "node",
        "swift",
        "bash",
        "git",
    ):
        tool_path = Path(fields[f"{tool}_path"])
        digest = fields[f"{tool}_sha256"]
        expected_path = expected_tool_paths.get(tool)
        path_is_exact = (
            tool_path.parent == runtime_root / "swift-toolchain" / "bin"
            and _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(tool_path.name)
            is not None
            if tool == "swift"
            else tool_path == expected_path
        )
        if (
            not tool_path.is_absolute()
            or not path_is_exact
            or not _DIGEST_RE.fullmatch(digest)
            or digest != expected_tool_digests.get(tool)
        ):
            raise ReceiptError(
                f"corridor {tool} is not the authenticated private runtime tool"
            )
        if not private_build_roots_available:
            continue
        tool_contract = _bounded_path_contract(
            tool_path,
            f"corridor {tool} tool",
            maximum_bytes=_MAX_TOOL_BYTES,
            expected_mode=0o500,
            allowed_owners={os.geteuid()},
            require_single_link=True,
            executable=True,
        )
        if tool_contract.sha256 != digest:
            raise ReceiptError(f"corridor {tool} tool digest mismatch")
    if not fields["swift_version"].strip():
        raise ReceiptError("corridor Swift tool version is blank")
    cargo_cache_input = _validate_cargo_cache_input(
        fields,
        artifact_root=artifact_root,
        private_build_roots_available=private_build_roots_available,
    )
    repo_cargo_config = _bounded_path_contract(
        repo_root / ".cargo" / "config.toml",
        "repository Cargo config",
        maximum_bytes=_MAX_POLICY_BYTES,
    )
    if (
        not _DIGEST_RE.fullmatch(fields["repo_cargo_config_sha256"])
        or repo_cargo_config.sha256 != fields["repo_cargo_config_sha256"]
    ):
        raise ReceiptError("repository Cargo config digest mismatch")
    grouped_fixture = _bounded_path_contract(
        repo_root / _NATIVE_AMX_GROUPED_FIXTURE,
        "grouped Native AMX V2 fixture",
        maximum_bytes=_MAX_LOCALNET_MANIFEST_BYTES,
    )
    if (
        not _DIGEST_RE.fullmatch(fields["native_amx_grouped_fixture_sha256"])
        or grouped_fixture.sha256
        != fields["native_amx_grouped_fixture_sha256"]
    ):
        raise ReceiptError("grouped Native AMX V2 fixture digest mismatch")
    expected_suite_manifest = _sdk_suite_source_manifest(
        repo_root, _NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE
    )
    if (
        not _DIGEST_RE.fullmatch(
            fields["native_amx_grouped_suite_source_manifest_sha256"]
        )
        or fields["native_amx_grouped_suite_source_manifest_sha256"]
        != expected_suite_manifest
    ):
        raise ReceiptError(
            "grouped Native AMX V2 suite-source manifest digest mismatch"
        )
    expected_diagnostics_suite_manifest = _sdk_suite_source_manifest(
        repo_root, _SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE
    )

    release_runner = _bounded_evidence_snapshot(
        repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
        "release runner inventory",
        maximum_bytes=_MAX_POLICY_BYTES,
        require_single_link=False,
    )
    summary = _bounded_evidence_snapshot(
        completion_path.with_name("summary.tsv"),
        "corridor summary",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    required_snapshot = _bounded_evidence_snapshot(
        completion_path.with_name("production-required-tests.tsv"),
        "corridor production inventory",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    g_unit_snapshot = _bounded_evidence_snapshot(
        completion_path.with_name("g-unit-required-tests.tsv"),
        "corridor G-UNIT inventory",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    if summary.sha256 != fields["summary_sha256"]:
        raise ReceiptError("corridor summary digest mismatch")
    if required_snapshot.sha256 != fields["production_required_tests_sha256"]:
        raise ReceiptError("corridor production inventory digest mismatch")
    if g_unit_snapshot.sha256 != fields["g_unit_inventory_sha256"]:
        raise ReceiptError("corridor G-UNIT inventory digest mismatch")

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(
                    required_snapshot, "corridor production inventory"
                ),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != ("module", "test"):
            raise ReceiptError("corridor production inventory fields are not canonical")
        required_rows = list(reader)
    except csv.Error as error:
        raise ReceiptError(
            "corridor production inventory is malformed TSV"
        ) from error
    if len(required_rows) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError(
            "corridor production inventory must contain exactly "
            f"{_PRODUCTION_TEST_COUNT} tests"
        )
    required_names = [row.get("test", "") for row in required_rows]
    if len(set(required_names)) != _PRODUCTION_TEST_COUNT:
        raise ReceiptError("corridor production inventory contains duplicate tests")
    if required_names != _canonical_production_tests(repo_root, release_runner):
        raise ReceiptError("corridor production inventory is not the canonical release list")
    module_counts = {module: count for _, module, count in _PRODUCTION_MODULES}
    required_by_module: dict[str, list[str]] = {module: [] for module in module_counts}
    for row in required_rows:
        if None in row or set(row) != {"module", "test"}:
            raise ReceiptError("corridor production inventory has extra columns")
        module = row["module"]
        test = row["test"]
        if module not in module_counts or not test.startswith(f"{module}::"):
            raise ReceiptError("corridor production inventory has an invalid module binding")
        required_by_module[module].append(test)
    if any(
        len(required_by_module[module]) != expected
        for module, expected in module_counts.items()
    ):
        raise ReceiptError("corridor production inventory module counts are not exact")

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(g_unit_snapshot, "corridor G-UNIT inventory"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != ("leg_id", "crate", "test"):
            raise ReceiptError("corridor G-UNIT inventory fields are not canonical")
        g_unit_rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("corridor G-UNIT inventory is malformed TSV") from error
    canonical_g_unit_rows = _canonical_g_unit_rows(repo_root, release_runner)
    if len(g_unit_rows) != _G_UNIT_TEST_COUNT:
        raise ReceiptError(
            f"corridor G-UNIT inventory must contain exactly "
            f"{_G_UNIT_TEST_COUNT} tests"
        )
    for index, (row, expected_row) in enumerate(
        zip(g_unit_rows, canonical_g_unit_rows)
    ):
        expected_leg, expected_package, expected_test = expected_row
        if (
            None in row
            or set(row) != {"leg_id", "crate", "test"}
            or row
            != {
                "leg_id": expected_leg,
                "crate": expected_package,
                "test": expected_test,
            }
        ):
            raise ReceiptError(
                f"corridor G-UNIT inventory row {index} is not canonical"
            )

    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(summary, "corridor summary"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != _CORRIDOR_SUMMARY_FIELDS:
            raise ReceiptError("corridor summary fields are not canonical")
        rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("corridor summary is malformed TSV") from error
    expected_legs = _corridor_legs(fields["cargo_path"])
    if len(rows) != len(expected_legs):
        raise ReceiptError("corridor summary must contain every exact release leg")
    logs: list[PathContract] = []
    module_for_leg = {leg_id: module for leg_id, module, _ in _PRODUCTION_MODULES}
    g_unit_tests_by_leg: dict[str, list[str]] = {
        leg_id: [] for _, leg_id, _, _, _ in _G_UNIT_GROUPS
    }
    for leg_id, _, test in canonical_g_unit_rows:
        g_unit_tests_by_leg[leg_id].append(test)
    exact_cargo_tests: dict[str, tuple[str, ...]] = {
        "status-rust": (_DATA_STATUS_TEST,),
        "lane-certificate-rust": (_DATA_LANE_CERTIFICATE_TEST,),
        "cross-sdk-rust": _CROSS_SDK_TESTS,
        "sumeragi-diagnostics-rust": _RUST_SDK_DIAGNOSTICS_TESTS,
    }
    exact_cargo_tests.update(
        {
            f"taira-contract-{index}": (test,)
            for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
        }
    )
    # The exact-length equality above provides the same fail-closed guarantee
    # as ``zip(strict=True)`` while retaining the repository's Python 3.9
    # compatibility.
    for index, (row, expected_leg) in enumerate(zip(rows, expected_legs)):
        leg_id, kind, required_count, command = expected_leg
        expected_log = f"logs/{index:02d}-{leg_id}.log"
        expected_row = {
            "leg_index": str(index),
            "leg_id": leg_id,
            "kind": kind,
            "required_test_count": str(required_count),
            "command_status": "0",
            "tee_status": "0",
            "log": expected_log,
            "command": command,
        }
        if None in row or set(row) != set(_CORRIDOR_SUMMARY_FIELDS) or any(
            row.get(name) != value for name, value in expected_row.items()
        ):
            raise ReceiptError(f"corridor summary row {index} is not the exact release leg")
        digest = row.get("log_sha256", "")
        if not _DIGEST_RE.fullmatch(digest):
            raise ReceiptError(f"corridor summary row {index} has an invalid log digest")
        log = _bounded_evidence_snapshot(
            completion_path.parent / expected_log,
            f"corridor log {index}",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if log.sha256 != digest:
            raise ReceiptError(f"corridor log {index} digest mismatch")
        lines = _decode_lf_text(log, f"corridor log {index}").splitlines()
        observed = _test_count_from_log(lines, kind, f"corridor log {index}")
        if row.get("observed_test_count") != str(observed):
            raise ReceiptError(f"corridor summary row {index} has the wrong observed count")
        if kind == "cargo-module":
            if observed == 0 or observed < required_count:
                raise ReceiptError(f"corridor module {leg_id} ran too few tests")
            module = module_for_leg[leg_id]
            for test in required_by_module[module]:
                if lines.count(f"test {test} ... ok") != 1:
                    raise ReceiptError(
                        f"corridor module {leg_id} lacks one required passing test"
                    )
        elif observed != required_count:
            raise ReceiptError(f"corridor leg {leg_id} has the wrong passing count")
        if kind == "cargo-focus":
            expected_tests = g_unit_tests_by_leg.get(leg_id)
            if expected_tests is None or len(expected_tests) != required_count:
                raise ReceiptError(
                    f"corridor G-UNIT leg {leg_id} has no exact inventory binding"
                )
            passing_tests = [
                match.group(1)
                for line in lines
                if (
                    match := re.fullmatch(
                        r"test ([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*) \.\.\. ok",
                        line,
                    )
                )
            ]
            if passing_tests != expected_tests:
                raise ReceiptError(
                    f"corridor G-UNIT leg {leg_id} lacks one required "
                    "passing test or contains an unexpected result"
                )
        if kind == "cargo-exact":
            for test in exact_cargo_tests[leg_id]:
                if lines.count(f"test {test} ... ok") != 1:
                    raise ReceiptError(
                        f"corridor exact Cargo leg {leg_id} lacks its named test"
                    )
        if kind == "native-amx-sdk":
            surface = leg_id.removeprefix("native-amx-grouped-")
            expected_marker = (
                f"native-amx-v2-grouped-parity surface={surface} "
                f"tests={observed} "
                "fixture_sha256="
                f"{fields['native_amx_grouped_fixture_sha256']} "
                "suite_source_manifest_sha256="
                f"{fields['native_amx_grouped_suite_source_manifest_sha256']}"
            )
            if lines.count(expected_marker) != 1:
                raise ReceiptError(
                    f"corridor grouped Native AMX V2 {surface} leg is not "
                    "bound to the exact fixture and suite sources"
                )
            replay_marker_prefix = "openapi-two-mirror-replay "
            replay_markers = [
                line for line in lines if line.startswith(replay_marker_prefix)
            ]
            if surface == "openapi":
                expected_replay_marker = (
                    "openapi-two-mirror-replay status=success "
                    f"candidate_oid={sealed['head_commit']} "
                    f"candidate_tree={sealed['head_tree']} "
                    "mirrors=2 artifacts=5 require_signed=1"
                )
                if (
                    replay_markers != [expected_replay_marker]
                    or lines.index(expected_replay_marker)
                    >= lines.index(expected_marker)
                ):
                    raise ReceiptError(
                        "corridor grouped Native AMX V2 openapi leg lacks the "
                        "exact path-free two-mirror replay binding"
                    )
            elif replay_markers:
                raise ReceiptError(
                    f"corridor grouped Native AMX V2 {surface} leg contains "
                    "an unexpected OpenAPI replay binding"
                )
        if kind == "sdk-diagnostics":
            surface = leg_id.removeprefix("sumeragi-diagnostics-")
            expected_marker = (
                f"sumeragi-v2-sdk-diagnostics surface={surface} "
                f"tests={observed} suite_source_manifest_sha256="
                f"{expected_diagnostics_suite_manifest}"
            )
            if lines.count(expected_marker) != 1:
                raise ReceiptError(
                    f"corridor Sumeragi v2 SDK diagnostics {surface} leg is "
                    "not bound to the exact suite sources"
                )
        logs.append(_snapshot_contract(log))
    manifest_path = Path(fields["prebuilt_manifest_path"])
    prebuilt_bundle = _prebuilt_binary_bundle(
        manifest_path=manifest_path,
        expected_manifest_sha256=fields["prebuilt_manifest_sha256"],
        fields=fields,
        sealed=sealed,
        repo_root=repo_root,
        artifact_root=artifact_root,
        cargo_target_root=cargo_target_root,
        private_build_roots_available=private_build_roots_available,
    )
    return (
        _snapshot_contract(summary),
        _snapshot_contract(required_snapshot),
        _snapshot_contract(g_unit_snapshot),
        logs,
        prebuilt_bundle,
        cargo_cache_input,
    )


def _seed_run_logs(
    seed_completion: EvidenceSnapshot,
    summary: EvidenceSnapshot,
    manifest: str,
    cargo_target_root: Path,
    prebuilt_bundle_dir: Path,
    prebuilt_manifest_sha256: str,
) -> list[PathContract]:
    seed_path = seed_completion.path
    try:
        reader = csv.DictReader(
            io.StringIO(
                _decode_lf_text(summary, "seed summary"),
                newline="",
            ),
            delimiter="\t",
        )
        if tuple(reader.fieldnames or ()) != _SEED_SUMMARY_FIELDS:
            raise ReceiptError("seed summary fields are not canonical")
        rows = list(reader)
    except csv.Error as error:
        raise ReceiptError("seed summary is malformed TSV") from error
    if len(rows) != _SEED_RUN_COUNT:
        raise ReceiptError(
            f"seed summary must contain exactly {_SEED_RUN_COUNT} run rows"
        )

    run_logs = []
    cargo_target_dir = cargo_target_root
    program_target_dir = prebuilt_bundle_dir
    irohad = program_target_dir / "release" / "iroha3d"
    message_control_irohad = (
        program_target_dir / "message-control" / "release" / "iroha3d"
    )
    iroha = program_target_dir / "release" / "iroha"
    kagami = program_target_dir / "release" / "kagami"
    for index, row in enumerate(rows):
        if None in row or set(row) != set(_SEED_SUMMARY_FIELDS):
            raise ReceiptError(f"seed summary row {index} has extra or missing columns")
        scenario = _SEED_SCENARIOS[index // _SEED_RUNS_PER_SCENARIO]
        seed_index = index % _SEED_RUNS_PER_SCENARIO
        expected_seed = (
            scenario if seed_index == 0 else f"{scenario}:seed:{seed_index:02d}"
        )
        output = f"runs/run-{index:03d}.log"
        localnet = f"localnets/run-{index:03d}"
        expected_command = (
            f"CARGO_TARGET_DIR={cargo_target_dir} "
            f"IROHA_TEST_TARGET_DIR={program_target_dir} "
            f"IROHA_RELEASE_SOURCE_MANIFEST_SHA256={manifest} "
            f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={prebuilt_manifest_sha256} "
            f"TEST_NETWORK_BIN_IROHAD={irohad} "
            f"TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL={message_control_irohad} "
            f"TEST_NETWORK_BIN_IROHA={iroha} "
            f"KAGAMI_BIN={kagami} "
            "CARGO_NET_OFFLINE=true "
            "IROHA_TEST_REQUIRE_NETWORK=1 "
            "IROHA_TEST_NETWORK_START_ATTEMPTS=1 "
            "IROHA_TEST_SKIP_BUILD=1 "
            "IROHA_TEST_ALLOW_REENTRANT_BUILD=0 "
            "IROHA_TEST_BUILD_PROFILE=release "
            "PROFILE=release "
            "IROHA_TEST_BUILD_TIMEOUT_MS=3600 "
            "IROHA_TEST_PROCESS_TIMEOUT_MS=300 "
            "IROHA_TEST_NETWORK_PERMIT_WAIT_TIMEOUT=300 "
            f"IROHA_TEST_NETWORK_BASE_SEED={expected_seed} "
            "TEST_NETWORK_TMP_DIR=${SEED_MATRIX_EVIDENCE_DIRECTORY}/"
            f"{localnet} "
            "IROHA_TEST_NETWORK_KEEP_DIRS=1 "
            "cargo test --locked --offline -p integration_tests --test "
            "sumeragi_v2_runner_isolated "
            f"sumeragi_v2_runner::{scenario} -- --exact --nocapture "
            "--test-threads=1"
        )
        if (
            row.get("profile") != "release"
            or row.get("source_manifest_sha256") != manifest
            or row.get("scenario") != scenario
            or row.get("seed") != expected_seed
            or row.get("result") != "passed"
            or row.get("cargo_status") != "0"
            or row.get("tee_status") != "0"
            or row.get("output") != output
            or row.get("localnet") != localnet
            or row.get("command") != expected_command
        ):
            raise ReceiptError(f"seed summary row {index} is not the exact release run")
        digest = row.get("run_log_sha256")
        if not isinstance(digest, str) or not _DIGEST_RE.fullmatch(digest):
            raise ReceiptError(f"seed summary row {index} has an invalid log digest")
        run_log = _bounded_evidence_snapshot(
            seed_path.parent / output,
            f"seed run log {index}",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if run_log.sha256 != digest:
            raise ReceiptError(f"seed run log {index} digest mismatch")
        lines = _decode_lf_text(run_log, f"seed run log {index}").splitlines()
        running = [
            line for line in lines if re.fullmatch(r"running [0-9]+ tests?", line)
        ]
        results = [line for line in lines if line.startswith("test result:")]
        passing = [
            line
            for line in results
            if re.fullmatch(
                r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
                r"[0-9]+ filtered out; finished in .+",
                line,
            )
        ]
        test_prefix = (
            f"test sumeragi_v2_runner::{scenario} ... "
            f"{scenario}: deterministic network seed = {expected_seed}"
        )
        prefix_positions = [
            position for position, line in enumerate(lines) if line == test_prefix
        ]
        ok_positions = [position for position, line in enumerate(lines) if line == "ok"]
        if (
            running != ["running 1 test"]
            or len(results) != 1
            or len(passing) != 1
            or len(prefix_positions) != 1
            or len(ok_positions) != 1
            or prefix_positions[0] >= ok_positions[0]
        ):
            raise ReceiptError(
                f"seed run log {index} does not prove its one exact passing scenario"
            )
        run_logs.append(_snapshot_contract(run_log))
    return run_logs


def _seed_localnet_manifests(
    seed_completion: EvidenceSnapshot, fields: dict[str, str]
) -> tuple[PathContract, list[PathContract]]:
    seed_path = seed_completion.path
    if (
        fields["localnet_manifest_count"] != str(_SEED_RUN_COUNT)
        or fields["localnet_manifests_path"] != "localnet-manifests.tsv"
        or not _DIGEST_RE.fullmatch(fields["localnet_manifests_sha256"])
    ):
        raise ReceiptError("seed completion has an invalid localnet manifest binding")
    index_snapshot = _bounded_evidence_snapshot(
        seed_path.parent / fields["localnet_manifests_path"],
        "seed localnet manifest index",
        maximum_bytes=_MAX_LOCALNET_MANIFEST_INDEX_BYTES,
    )
    if index_snapshot.sha256 != fields["localnet_manifests_sha256"]:
        raise ReceiptError("seed localnet manifest index digest mismatch")
    try:
        reader = csv.DictReader(
            io.StringIO(index_snapshot.data.decode("utf-8")), delimiter="\t"
        )
        if tuple(reader.fieldnames or ()) != _SEED_LOCALNET_MANIFEST_FIELDS:
            raise ReceiptError("seed localnet manifest index fields are not canonical")
        rows = list(reader)
    except UnicodeDecodeError as error:
        raise ReceiptError("seed localnet manifest index is not UTF-8") from error
    if len(rows) != _SEED_RUN_COUNT:
        raise ReceiptError(
            f"seed localnet manifest index must contain exactly {_SEED_RUN_COUNT} rows"
        )

    records: list[tuple[int, str, str, str]] = []
    canonical_index_lines = ["\t".join(_SEED_LOCALNET_MANIFEST_FIELDS)]
    for index, row in enumerate(rows):
        localnet = f"localnets/run-{index:03d}"
        relative_manifest = f"localnet-manifests/run-{index:03d}.tsv"
        path_field = f"localnet_manifest_{index:03d}_path"
        digest_field = f"localnet_manifest_{index:03d}_sha256"
        digest = row.get("manifest_sha256", "")
        expected_row = {
            "run_index": str(index),
            "localnet": localnet,
            "manifest": relative_manifest,
            "manifest_sha256": digest,
        }
        if (
            None in row
            or set(row) != set(_SEED_LOCALNET_MANIFEST_FIELDS)
            or row != expected_row
            or not _DIGEST_RE.fullmatch(digest)
            or fields[path_field] != relative_manifest
            or fields[digest_field] != digest
        ):
            raise ReceiptError(
                f"seed localnet manifest index row {index} is not canonical"
            )
        records.append((index, localnet, relative_manifest, digest))
        canonical_index_lines.append(
            "\t".join((str(index), localnet, relative_manifest, digest))
        )
    canonical_index = ("\n".join(canonical_index_lines) + "\n").encode("utf-8")
    if index_snapshot.data != canonical_index:
        raise ReceiptError("seed localnet manifest index bytes are not canonical")

    manifests: list[PathContract] = []
    for index, localnet, relative_manifest, digest in records:
        manifest_candidate = seed_path.parent / relative_manifest
        try:
            resolved_manifest = manifest_candidate.resolve(strict=True)
        except (OSError, RuntimeError) as error:
            raise ReceiptError(
                f"seed localnet manifest {index} is unavailable"
            ) from error
        if resolved_manifest != manifest_candidate:
            raise ReceiptError(f"seed localnet manifest {index} escaped its archive")
        snapshot = _bounded_evidence_snapshot(
            manifest_candidate,
            f"seed localnet manifest {index}",
            maximum_bytes=_MAX_LOCALNET_MANIFEST_BYTES,
        )
        if snapshot.sha256 != digest:
            raise ReceiptError(f"seed localnet manifest {index} digest mismatch")
        try:
            expected = canonical_localnet_manifest(seed_path.parent / localnet)
        except LocalnetManifestError as error:
            raise ReceiptError(
                f"seed retained localnet {index} is unsafe or unstable: {error}"
            ) from error
        if snapshot.data != expected:
            raise ReceiptError(
                f"seed localnet manifest {index} does not match retained content"
            )
        manifests.append(_snapshot_contract(snapshot))
    return _snapshot_contract(index_snapshot), manifests


def _scan_scaling_bundle(
    root: Path,
) -> tuple[list[tuple[str, Path, os.stat_result]], list[str], int]:
    try:
        root_metadata = root.lstat()
    except OSError as error:
        raise ReceiptError("scaling evidence bundle root is unavailable") from error
    if (
        root.resolve(strict=True) != root
        or stat.S_ISLNK(root_metadata.st_mode)
        or not stat.S_ISDIR(root_metadata.st_mode)
        or root_metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            "scaling evidence bundle root must be an owner-owned resolved "
            "non-symlink directory"
        )

    files: list[tuple[str, Path, os.stat_result]] = []
    directories: list[str] = []
    inodes: dict[tuple[int, int], str] = {}
    total_bytes = 0

    def visit(directory: Path, prefix: PurePosixPath | None) -> None:
        nonlocal total_bytes
        try:
            with os.scandir(directory) as iterator:
                entries = sorted(iterator, key=lambda entry: entry.name)
        except OSError as error:
            raise ReceiptError(
                "scaling evidence bundle directory cannot be enumerated"
            ) from error
        for entry in entries:
            component = entry.name
            if _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(component) is None:
                raise ReceiptError(
                    "scaling evidence bundle contains an unsafe path component"
                )
            relative_path = (
                PurePosixPath(component)
                if prefix is None
                else prefix / component
            )
            relative = relative_path.as_posix()
            path = directory / component
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise ReceiptError(
                    f"scaling evidence bundle entry is unavailable: {relative}"
                ) from error
            if stat.S_ISLNK(metadata.st_mode):
                raise ReceiptError(
                    f"scaling evidence bundle contains a symlink: {relative}"
                )
            if stat.S_ISDIR(metadata.st_mode):
                directories.append(relative)
                if len(directories) > _MAX_SCALING_BUNDLE_DIRECTORY_COUNT:
                    raise ReceiptError(
                        "scaling evidence bundle exceeds its directory-count limit"
                    )
                visit(path, relative_path)
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise ReceiptError(
                    f"scaling evidence bundle contains a nonregular entry: {relative}"
                )
            if metadata.st_uid != os.geteuid():
                raise ReceiptError(
                    f"scaling evidence bundle file has an untrusted owner: {relative}"
                )
            if metadata.st_nlink != 1:
                raise ReceiptError(
                    f"scaling evidence bundle file has a hard-link alias: {relative}"
                )
            inode = (metadata.st_dev, metadata.st_ino)
            alias = inodes.get(inode)
            if alias is not None:
                raise ReceiptError(
                    "scaling evidence bundle files are hard-link aliases: "
                    f"{alias} and {relative}"
                )
            inodes[inode] = relative
            if metadata.st_size > _MAX_SCALING_BUNDLE_FILE_BYTES:
                raise ReceiptError(
                    f"scaling evidence bundle file exceeds its size limit: {relative}"
                )
            total_bytes += metadata.st_size
            if total_bytes > _MAX_SCALING_BUNDLE_TOTAL_BYTES:
                raise ReceiptError(
                    "scaling evidence bundle exceeds its aggregate size limit"
                )
            files.append((relative, path, metadata))
            if len(files) > _MAX_SCALING_BUNDLE_FILE_COUNT:
                raise ReceiptError(
                    "scaling evidence bundle exceeds its file-count limit"
                )

    visit(root, None)
    files.sort(key=lambda item: item[0])
    directories.sort()
    return files, directories, total_bytes


def _capture_scaling_bundle(
    root: Path,
) -> tuple[list[tuple[str, PathContract]], list[str], int]:
    scanned, directories, _ = _scan_scaling_bundle(root)
    directory_contracts = [
        _capture_directory_contract(root, "scaling evidence bundle root")
    ]
    directory_contracts.extend(
        _capture_directory_contract(
            root.joinpath(*PurePosixPath(relative).parts),
            f"scaling evidence bundle directory {index}",
        )
        for index, relative in enumerate(directories)
    )
    files: list[tuple[str, PathContract]] = []
    for index, (relative, path, metadata) in enumerate(scanned):
        contract = _capture_path_contract(
            path,
            f"scaling evidence bundle file {index}",
            expected_sha256=None,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=metadata.st_size,
        )
        files.append((relative, contract))

    final_scan, final_directories, final_total = _scan_scaling_bundle(root)
    if [item[0] for item in final_scan] != [item[0] for item in scanned]:
        raise ReceiptError("scaling evidence bundle file inventory changed while read")
    if final_directories != directories:
        raise ReceiptError(
            "scaling evidence bundle directory inventory changed while read"
        )
    for index, ((_, contract), (_, _, metadata)) in enumerate(
        zip(files, final_scan)
    ):
        observed = (
            metadata.st_dev,
            metadata.st_ino,
            stat.S_IMODE(metadata.st_mode),
            metadata.st_uid,
            metadata.st_nlink,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        )
        expected = (
            contract.device,
            contract.inode,
            contract.mode,
            contract.owner,
            contract.nlink,
            contract.size,
            contract.mtime_ns,
            contract.ctime_ns,
        )
        if observed != expected:
            raise ReceiptError(
                f"scaling evidence bundle file {index} changed after hashing"
            )
    for index, contract in enumerate(directory_contracts):
        if (
            _capture_directory_contract(
                contract.path, f"scaling evidence stable directory {index}"
            )
            != contract
        ):
            raise ReceiptError(
                "scaling evidence bundle directory changed while files were hashed"
            )
    if final_total != sum(contract.size for _, contract in files):
        raise ReceiptError("scaling evidence bundle size changed while read")
    return files, directories, final_total


def _load_scaling_json(path: Path, name: str) -> tuple[bytes, dict[str, Any]]:
    snapshot = _read_evidence_snapshot(
        path,
        name,
        maximum_bytes=_MAX_SCALING_JSON_BYTES,
        allowed_owners={os.geteuid()},
    )

    def reject_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        value: dict[str, Any] = {}
        for key, item in pairs:
            if key in value:
                raise ReceiptError(f"{name} contains a duplicate JSON field")
            value[key] = item
        return value

    def reject_constant(value: str) -> None:
        raise ReceiptError(f"{name} contains a nonfinite JSON value: {value}")

    try:
        value = json.loads(
            snapshot.data.decode("utf-8"),
            object_pairs_hook=reject_pairs,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReceiptError(f"{name} is not strict UTF-8 JSON") from error
    if not isinstance(value, dict):
        raise ReceiptError(f"{name} must contain one JSON object")
    return snapshot.data, value


def _scaling_ref_path(
    value: Any,
    *,
    root: Path,
    contracts: dict[str, PathContract],
    name: str,
) -> tuple[str, PathContract]:
    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        raise ReceiptError(f"{name} is not one canonical scaling artifact reference")
    relative = value.get("path")
    digest = value.get("sha256")
    if not isinstance(relative, str) or not isinstance(digest, str):
        raise ReceiptError(f"{name} scaling artifact reference is malformed")
    pure = PurePosixPath(relative)
    if (
        relative != pure.as_posix()
        or pure.is_absolute()
        or not pure.parts
        or any(
            part in {"", ".", ".."}
            or _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(part) is None
            for part in pure.parts
        )
    ):
        raise ReceiptError(f"{name} is not a safe normalized in-bundle path")
    contract = contracts.get(relative)
    if contract is None:
        raise ReceiptError(f"{name} is absent from the scaling bundle inventory")
    if _require_digest(digest, f"{name} digest") != contract.sha256:
        raise ReceiptError(f"{name} digest does not match the scaling bundle")
    expected_path = root.joinpath(*pure.parts)
    if contract.path != expected_path:
        raise ReceiptError(f"{name} resolves outside the scaling evidence bundle")
    return relative, contract


def _path_contract_artifact(contract: PathContract) -> dict[str, Any]:
    return {
        "path": str(contract.path),
        "sha256": contract.sha256,
        "size_bytes": contract.size,
        "mode": f"{contract.mode:04o}",
        "owner_uid": contract.owner,
        "nlink": contract.nlink,
    }


def _sdk_relative_path(value: Any, name: str, *, allow_root: bool) -> str:
    """Return one canonical path-withheld SDK inventory member name."""

    if not isinstance(value, str) or "\0" in value:
        raise ReceiptError(f"{name} is not text")
    if allow_root and value == ".":
        return value
    pure = PurePosixPath(value)
    if (
        pure.is_absolute()
        or value != pure.as_posix()
        or not pure.parts
        or any(part in {"", ".", ".."} for part in pure.parts)
    ):
        raise ReceiptError(f"{name} is not one canonical relative path")
    return value


def _sdk_inventory_records(
    value: Any,
    name: str,
    *,
    root_mode: str,
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]], int]:
    """Validate one exact path-free SDK member inventory."""

    if (
        not isinstance(value, list)
        or not value
        or len(value) > _MAX_SDK_RECORDS
    ):
        raise ReceiptError(f"{name} has an invalid record count")
    records: list[dict[str, Any]] = []
    by_path: dict[str, dict[str, Any]] = {}
    file_bytes = 0
    for index, raw in enumerate(value):
        if not isinstance(raw, dict):
            raise ReceiptError(f"{name} record {index} is not an object")
        kind = raw.get("kind")
        expected_fields = {
            "directory": {"path", "kind", "mode"},
            "file": {"path", "kind", "mode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        if expected_fields is None or set(raw) != expected_fields:
            raise ReceiptError(f"{name} record {index} has the wrong schema")
        relative = _sdk_relative_path(
            raw["path"], f"{name} record {index} path", allow_root=True
        )
        if relative in by_path:
            raise ReceiptError(f"{name} contains a duplicate member")
        mode = raw["mode"]
        if not isinstance(mode, str) or re.fullmatch(r"[0-7]{4}", mode) is None:
            raise ReceiptError(f"{name} record {index} has an invalid mode")
        if kind == "file":
            size = raw["size"]
            if type(size) is not int or not 0 <= size <= 4 * 1024 * 1024 * 1024:
                raise ReceiptError(f"{name} record {index} has an invalid size")
            _require_digest(raw["sha256"], f"{name} record {index} digest")
            file_bytes += size
            if file_bytes > _MAX_SDK_ARCHIVE_BYTES:
                raise ReceiptError(f"{name} exceeds its aggregate byte limit")
        elif kind == "symlink":
            target = raw["target"]
            if (
                not isinstance(target, str)
                or "\0" in target
                or PurePosixPath(target).is_absolute()
                or PurePosixPath(target).as_posix() != target
            ):
                raise ReceiptError(f"{name} record {index} has an unsafe symlink")
        records.append(raw)
        by_path[relative] = raw
    if records[0] != {"path": ".", "kind": "directory", "mode": root_mode}:
        raise ReceiptError(f"{name} has the wrong protected root")
    if [record["path"] for record in records[1:]] != sorted(by_path.keys() - {"."}):
        raise ReceiptError(f"{name} member ordering is not canonical")
    for relative, record in by_path.items():
        if relative == ".":
            continue
        pure = PurePosixPath(relative)
        parent = pure.parent.as_posix()
        parent = "." if parent == "." else parent
        parent_record = by_path.get(parent)
        if not isinstance(parent_record, dict) or parent_record.get("kind") != "directory":
            raise ReceiptError(f"{name} member lacks its exact parent directory")
        if record["kind"] == "symlink":
            parts = list(pure.parent.parts) if pure.parent.as_posix() != "." else []
            for part in PurePosixPath(record["target"]).parts:
                if part in {"", "."}:
                    continue
                if part == "..":
                    if not parts:
                        raise ReceiptError(f"{name} symlink escapes its archive")
                    parts.pop()
                else:
                    parts.append(part)
            target = "/".join(parts) or "."
            if target not in by_path:
                raise ReceiptError(f"{name} symlink target is not inventoried")
    return records, by_path, file_bytes


def _sdk_source_inventory(
    value: Any, name: str,
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    """Validate one protected complete source-tree member inventory."""

    document = _require_exact_json_fields(
        value,
        {
            "format", "schema_version", "record_count", "file_bytes",
            "records_sha256", "records",
        },
        name,
    )
    raw_records = document["records"]
    if (
        not isinstance(raw_records, list)
        or not raw_records
        or not isinstance(raw_records[0], dict)
        or raw_records[0].get("path") != "."
        or raw_records[0].get("kind") != "directory"
        or not isinstance(raw_records[0].get("mode"), str)
    ):
        raise ReceiptError(f"{name} lacks its exact source root")
    records, by_path, file_bytes = _sdk_inventory_records(
        raw_records, name, root_mode=raw_records[0]["mode"],
    )
    records_payload = json.dumps(
        records, ensure_ascii=True, sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")
    if (
        document["format"] != _SDK_SOURCE_INVENTORY_FORMAT
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 1
        or type(document["record_count"]) is not int
        or document["record_count"] != len(records)
        or type(document["file_bytes"]) is not int
        or document["file_bytes"] != file_bytes
        or document["records_sha256"]
        != hashlib.sha256(records_payload).hexdigest()
    ):
        raise ReceiptError(f"{name} accounting or aggregate digest is not exact")
    return records, by_path


def _sdk_source_path(value: Any, name: str) -> None:
    """Validate but do not publish one bootstrap-private original path."""

    if (
        not isinstance(value, str)
        or "\0" in value
        or not Path(value).is_absolute()
        or value != os.path.abspath(os.path.normpath(value))
    ):
        raise ReceiptError(f"{name} is not one normalized absolute private path")


def _sdk_project_source_records(
    records: list[dict[str, Any]], prefix: str,
) -> list[dict[str, Any]]:
    """Project source modes onto the sealed archive layout."""

    projected: list[dict[str, Any]] = []
    for source in records:
        relative = source["path"]
        record = dict(source)
        record["path"] = prefix if relative == "." else f"{prefix}/{relative}"
        if source["kind"] == "directory":
            record["mode"] = "0500"
        elif source["kind"] == "file":
            record["mode"] = (
                "0500" if int(str(source["mode"]), 8) & 0o111 else "0400"
            )
        projected.append(record)
    return sorted(projected, key=lambda record: str(record["path"]))


def _sdk_validate_private_source_manifest(
    *,
    path: Path,
    expected_sha256: str,
    archive_records: list[dict[str, Any]],
    bindings: dict[str, Any],
    expected_git_sha256: str,
) -> None:
    """Link the withheld source inventories to every retained archive member."""

    snapshot = _bounded_evidence_snapshot(
        path,
        "bootstrap-private SDK dependency source manifest",
        maximum_bytes=_MAX_SDK_MANIFEST_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    if snapshot.sha256 != expected_sha256:
        raise ReceiptError("SDK private source manifest digest changed")
    document = _require_exact_json_fields(
        _decode_canonical_json(snapshot.data, "SDK private source manifest"),
        {
            "format", "schema_version", "git", "node", "openapi_node",
            "swiftpm", "gradle",
        },
        "SDK private source manifest",
    )
    if (
        document["format"] != _SDK_SOURCE_MANIFEST_FORMAT
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 3
    ):
        raise ReceiptError("SDK private source manifest identity is not exact")
    git = _require_exact_json_fields(
        document["git"], {"executable", "sha256"}, "SDK private Git binding",
    )
    node = _require_exact_json_fields(
        document["node"],
        {"node_modules_root", "package_lock_sha256", "node_modules_inventory"},
        "SDK private Node source",
    )
    openapi_node = _require_exact_json_fields(
        document["openapi_node"],
        {"node_modules_root", "package_lock_sha256", "node_modules_inventory"},
        "SDK private OpenAPI Node source",
    )
    swift = _require_exact_json_fields(
        document["swiftpm"],
        {
            "cache_root", "cache_inventory", "package_resolved_sha256",
            "resolved_revisions",
        },
        "SDK private SwiftPM source",
    )
    gradle = _require_exact_json_fields(
        document["gradle"],
        {
            "distribution_archive", "distribution_sha256",
            "distribution_url", "gradle_user_home",
            "gradle_user_home_inventory", "java_wrapper_properties_sha256",
            "kotlin_wrapper_properties_sha256", "version", "wrapper_cache_key",
        },
        "SDK private Gradle source",
    )
    for value, name in (
        (git["executable"], "SDK private protected Git"),
        (node["node_modules_root"], "SDK private node_modules root"),
        (
            openapi_node["node_modules_root"],
            "SDK private OpenAPI node_modules root",
        ),
        (swift["cache_root"], "SDK private SwiftPM cache root"),
        (gradle["distribution_archive"], "SDK private Gradle distribution"),
        (gradle["gradle_user_home"], "SDK private Gradle user home"),
    ):
        _sdk_source_path(value, name)
    if Path(openapi_node["node_modules_root"]).parts[-3:] != (
        "tools", "openapi", "node_modules",
    ):
        raise ReceiptError(
            "SDK private OpenAPI node_modules root is not the exact tools/openapi root"
        )
    expected_bindings = bindings
    if (
        _require_digest(git["sha256"], "SDK private protected Git")
        != expected_git_sha256
        or _require_digest(node["package_lock_sha256"], "SDK private package lock")
        != expected_bindings["node"]["package_lock_sha256"]
        or _require_digest(
            openapi_node["package_lock_sha256"],
            "SDK private OpenAPI package lock",
        ) != expected_bindings["openapi_node"]["package_lock_sha256"]
        or _require_digest(
            swift["package_resolved_sha256"], "SDK private Package.resolved"
        ) != expected_bindings["swiftpm"]["package_resolved_sha256"]
        or swift["resolved_revisions"]
        != expected_bindings["swiftpm"]["resolved_revisions"]
        or _require_digest(
            gradle["distribution_sha256"], "SDK private Gradle distribution"
        ) != expected_bindings["gradle"]["distribution_sha256"]
        or _require_digest(
            gradle["java_wrapper_properties_sha256"],
            "SDK private Java Gradle wrapper",
        ) != expected_bindings["gradle"]["wrapper_properties_sha256"]["java"]
        or _require_digest(
            gradle["kotlin_wrapper_properties_sha256"],
            "SDK private Kotlin Gradle wrapper",
        ) != expected_bindings["gradle"]["wrapper_properties_sha256"]["kotlin"]
        or gradle["distribution_url"] != _SDK_GRADLE_DISTRIBUTION_URL
        or gradle["wrapper_cache_key"] != _SDK_GRADLE_WRAPPER_CACHE_KEY
        or gradle["version"] != "9.3.0"
    ):
        raise ReceiptError("SDK private source bindings do not match the archive")

    specifications = (
        (
            "node/node_modules", node["node_modules_inventory"],
            "SDK private Node member inventory",
        ),
        (
            "openapi/node_modules", openapi_node["node_modules_inventory"],
            "SDK private OpenAPI Node member inventory",
        ),
        (
            "swiftpm/cache", swift["cache_inventory"],
            "SDK private SwiftPM member inventory",
        ),
        (
            "gradle/gradle-user-home", gradle["gradle_user_home_inventory"],
            "SDK private Gradle member inventory",
        ),
    )
    archive_by_path = {str(record["path"]): record for record in archive_records}
    source_by_prefix: dict[str, dict[str, dict[str, Any]]] = {}
    for prefix, raw_inventory, name in specifications:
        source_records, source_by_path = _sdk_source_inventory(raw_inventory, name)
        source_by_prefix[prefix] = source_by_path
        projected = _sdk_project_source_records(source_records, prefix)
        retained = sorted(
            (
                record for relative, record in archive_by_path.items()
                if relative == prefix or relative.startswith(prefix + "/")
            ),
            key=lambda record: str(record["path"]),
        )
        if retained != projected:
            raise ReceiptError(
                f"{name} does not exactly reproduce the retained archive subtree"
            )

    swift_records = source_by_prefix["swiftpm/cache"]
    swift_children = {
        path for path in swift_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    if swift_children != {"checkouts", "repositories"} or any(
        swift_records[name].get("kind") != "directory"
        for name in swift_children
    ):
        raise ReceiptError("SDK private SwiftPM cache roots are not exact")
    revision_checkouts = {
        str(item["checkout"]) for item in swift["resolved_revisions"]
    }
    observed_checkouts = {
        path.removeprefix("checkouts/")
        for path, record in swift_records.items()
        if path.startswith("checkouts/")
        and "/" not in path.removeprefix("checkouts/")
        and record.get("kind") == "directory"
    }
    if observed_checkouts != revision_checkouts or any(
        swift_records.get(f"checkouts/{checkout}/.git/HEAD", {}).get("kind")
        != "file"
        for checkout in revision_checkouts
    ):
        raise ReceiptError("SDK private SwiftPM checkouts are not revision-complete")

    gradle_records = source_by_prefix["gradle/gradle-user-home"]
    gradle_children = {
        path for path in gradle_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    cache_key_root = (
        "wrapper/dists/gradle-9.3.0-bin/"
        f"{_SDK_GRADLE_WRAPPER_CACHE_KEY}"
    )
    if (
        gradle_children != {"caches", "wrapper"}
        or gradle_records.get("caches/9.3.0", {}).get("kind") != "directory"
        or gradle_records.get("caches/modules-2", {}).get("kind") != "directory"
        or gradle_records.get(cache_key_root, {}).get("kind") != "directory"
        or gradle_records.get(f"{cache_key_root}/gradle-9.3.0", {}).get("kind")
        != "directory"
        or gradle_records.get(
            f"{cache_key_root}/gradle-9.3.0-bin.zip.ok", {}
        ).get("kind") != "file"
        or gradle_records.get(
            _SDK_GRADLE_LAUNCHER_ARCHIVE_NAME.removeprefix(
                "gradle/gradle-user-home/"
            ), {}
        ).get("kind") != "file"
    ):
        raise ReceiptError("SDK private Gradle offline closure is not exact")


def _sdk_binding_contract(
    value: Any,
    records: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    """Validate the path-free Node, SwiftPM, and Gradle lock bindings."""

    bindings = _require_exact_json_fields(
        value, {"node", "openapi_node", "swiftpm", "gradle"},
        "SDK dependency bindings"
    )
    node = _require_exact_json_fields(
        bindings["node"],
        {
            "node_modules_archive_name",
            "package_lock_archive_name",
            "package_lock_sha256",
            "installed_lock_sha256",
        },
        "SDK Node binding",
    )
    if (
        node["node_modules_archive_name"] != "node/node_modules"
        or node["package_lock_archive_name"] != "node/package-lock.json"
    ):
        raise ReceiptError("SDK Node archive names are not exact")
    for field in ("package_lock_sha256", "installed_lock_sha256"):
        _require_digest(node[field], f"SDK Node {field}")
    openapi_node = _require_exact_json_fields(
        bindings["openapi_node"],
        {
            "node_modules_archive_name",
            "package_lock_archive_name",
            "package_lock_sha256",
            "installed_lock_sha256",
        },
        "SDK OpenAPI Node binding",
    )
    if (
        openapi_node["node_modules_archive_name"] != "openapi/node_modules"
        or openapi_node["package_lock_archive_name"]
        != "openapi/package-lock.json"
    ):
        raise ReceiptError("SDK OpenAPI Node archive names are not exact")
    for field in ("package_lock_sha256", "installed_lock_sha256"):
        _require_digest(
            openapi_node[field], f"SDK OpenAPI Node {field}"
        )
    swift = _require_exact_json_fields(
        bindings["swiftpm"],
        {
            "cache_archive_name",
            "package_resolved_archive_name",
            "package_resolved_sha256",
            "resolved_revisions",
        },
        "SDK SwiftPM binding",
    )
    if (
        swift["cache_archive_name"] != "swiftpm/cache"
        or swift["package_resolved_archive_name"] != "swiftpm/Package.resolved"
    ):
        raise ReceiptError("SDK SwiftPM archive names are not exact")
    _require_digest(swift["package_resolved_sha256"], "SDK Package.resolved digest")
    revisions = swift["resolved_revisions"]
    if not isinstance(revisions, list) or len(revisions) > _MAX_SDK_RECORDS:
        raise ReceiptError("SDK SwiftPM revisions are not bounded")
    revision_keys: list[str] = []
    for index, raw in enumerate(revisions):
        item = _require_exact_json_fields(
            raw, {"identity", "checkout", "revision", "tree"},
            f"SDK SwiftPM revision {index}",
        )
        identity = item["identity"]
        checkout = item["checkout"]
        revision = item["revision"]
        tree = item["tree"]
        if (
            not isinstance(identity, str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", identity) is None
            or not isinstance(checkout, str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", checkout) is None
            or not isinstance(revision, str)
            or _OBJECT_ID_RE.fullmatch(revision) is None
            or not isinstance(tree, str)
            or _OBJECT_ID_RE.fullmatch(tree) is None
        ):
            raise ReceiptError("SDK SwiftPM revision is malformed")
        revision_keys.append(identity)
    if revision_keys != sorted(set(revision_keys)):
        raise ReceiptError("SDK SwiftPM revisions are not uniquely ordered")
    gradle = _require_exact_json_fields(
        bindings["gradle"],
        {
            "distribution_archive_name",
            "distribution_sha256",
            "distribution_url",
            "gradle_user_home_archive_name",
            "launcher_archive_name",
            "wrapper_cache_key",
            "version",
            "wrapper_properties_sha256",
        },
        "SDK Gradle binding",
    )
    wrappers = _require_exact_json_fields(
        gradle["wrapper_properties_sha256"], {"java", "kotlin"},
        "SDK Gradle wrapper digests",
    )
    if (
        gradle["distribution_archive_name"] != "gradle/gradle-9.3.0-bin.zip"
        or gradle["gradle_user_home_archive_name"] != "gradle/gradle-user-home"
        or gradle["distribution_url"] != _SDK_GRADLE_DISTRIBUTION_URL
        or gradle["launcher_archive_name"]
        != _SDK_GRADLE_LAUNCHER_ARCHIVE_NAME
        or gradle["version"] != "9.3.0"
        or gradle["wrapper_cache_key"] != _SDK_GRADLE_WRAPPER_CACHE_KEY
    ):
        raise ReceiptError("SDK Gradle binding is not exact 9.3.0")
    _require_digest(gradle["distribution_sha256"], "SDK Gradle distribution digest")
    for name, digest in wrappers.items():
        _require_digest(digest, f"SDK Gradle {name} wrapper digest")
    expected_files = {
        "node/package-lock.json": node["package_lock_sha256"],
        "node/node_modules/.package-lock.json": node["installed_lock_sha256"],
        "openapi/package-lock.json": openapi_node["package_lock_sha256"],
        "openapi/node_modules/.package-lock.json": openapi_node[
            "installed_lock_sha256"
        ],
        "swiftpm/Package.resolved": swift["package_resolved_sha256"],
        "gradle/gradle-9.3.0-bin.zip": gradle["distribution_sha256"],
        "gradle/java-gradle-wrapper.properties": wrappers["java"],
        "gradle/kotlin-gradle-wrapper.properties": wrappers["kotlin"],
    }
    for relative, digest in expected_files.items():
        record = records.get(relative)
        if not isinstance(record, dict) or record.get("kind") != "file" or record.get("sha256") != digest:
            raise ReceiptError(f"SDK binding does not match archived {relative}")
    for relative in (
        "node/node_modules", "openapi/node_modules", "swiftpm/cache",
        "swiftpm/cache/checkouts",
        "swiftpm/cache/repositories", "gradle/gradle-user-home",
    ):
        record = records.get(relative)
        if not isinstance(record, dict) or record.get("kind") != "directory":
            raise ReceiptError(f"SDK binding omits archived directory {relative}")
    launcher = records.get(_SDK_GRADLE_LAUNCHER_ARCHIVE_NAME)
    if (
        not isinstance(launcher, dict)
        or launcher.get("kind") != "file"
        or int(str(launcher.get("mode")), 8) & 0o111 == 0
    ):
        raise ReceiptError("SDK binding omits its authenticated Gradle launcher")
    return bindings


def _sdk_validate_control_files(
    controls: dict[str, bytes], bindings: dict[str, Any]
) -> None:
    """Recheck the lock/revision/wrapper semantics from archived bytes."""

    def decoded(name: str) -> dict[str, Any]:
        data = controls.get(name)
        if data is None:
            raise ReceiptError(f"SDK archive omits control file {name}")
        try:
            value = json.loads(data)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ReceiptError(f"SDK archive control file {name} is malformed") from error
        if not isinstance(value, dict):
            raise ReceiptError(f"SDK archive control file {name} is not an object")
        return value

    package = decoded("node/package-lock.json")
    installed = decoded("node/node_modules/.package-lock.json")
    if (
        package.get("lockfileVersion") != 3
        or installed.get("lockfileVersion") != 3
        or (package.get("name"), package.get("version"))
        != (installed.get("name"), installed.get("version"))
        or not isinstance(package.get("packages"), dict)
        or not isinstance(installed.get("packages"), dict)
        or not installed["packages"]
        or any(package["packages"].get(key) != value for key, value in installed["packages"].items())
    ):
        raise ReceiptError("SDK archived Node closure disagrees with package-lock.json")
    openapi_package = decoded("openapi/package-lock.json")
    openapi_installed = decoded("openapi/node_modules/.package-lock.json")
    openapi_packages = openapi_package.get("packages")
    openapi_installed_packages = openapi_installed.get("packages")
    if (
        openapi_package.get("lockfileVersion") != 3
        or openapi_installed.get("lockfileVersion") != 3
        or (openapi_package.get("name"), openapi_package.get("version"))
        != (openapi_installed.get("name"), openapi_installed.get("version"))
        or not isinstance(openapi_packages, dict)
        or not isinstance(openapi_installed_packages, dict)
        or not openapi_installed_packages
        or "" not in openapi_packages
        or openapi_installed_packages
        != {
            key: value for key, value in openapi_packages.items() if key
        }
    ):
        raise ReceiptError(
            "SDK archived OpenAPI Node closure disagrees exactly with package-lock.json"
        )
    resolved = decoded("swiftpm/Package.resolved")
    resolved_pairs = sorted(
        (
            {"identity": pin.get("identity"), "revision": pin.get("state", {}).get("revision")}
            for pin in resolved.get("pins", [])
            if isinstance(pin, dict) and isinstance(pin.get("state"), dict)
        ),
        key=lambda item: str(item["identity"]),
    ) if isinstance(resolved.get("pins"), list) else []
    expected_pairs = [
        {"identity": item["identity"], "revision": item["revision"]}
        for item in bindings["swiftpm"]["resolved_revisions"]
    ]
    if resolved.get("version") != 2 or resolved_pairs != expected_pairs:
        raise ReceiptError("SDK archived Package.resolved revisions are not exact")
    for item in bindings["swiftpm"]["resolved_revisions"]:
        name = f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
        head = controls.get(name)
        if head is None:
            raise ReceiptError("SDK archived SwiftPM checkout HEAD is absent")
        try:
            observed_revision = head.decode("ascii", "strict").strip()
        except UnicodeDecodeError as error:
            raise ReceiptError("SDK archived SwiftPM checkout HEAD is malformed") from error
        if observed_revision != item["revision"]:
            raise ReceiptError("SDK archived SwiftPM checkout revision changed")
    for kind in ("java", "kotlin"):
        name = f"gradle/{kind}-gradle-wrapper.properties"
        try:
            lines = controls[name].decode("utf-8").splitlines()
        except (KeyError, UnicodeDecodeError) as error:
            raise ReceiptError(f"SDK archived {kind} Gradle wrapper is malformed") from error
        values = dict(
            line.split("=", 1) for line in lines
            if line and not line.startswith("#") and "=" in line
        )
        if values.get("distributionUrl") != _SDK_GRADLE_DISTRIBUTION_URL.replace(
            ":", r"\:", 1,
        ):
            raise ReceiptError(f"SDK archived {kind} Gradle wrapper is not 9.3.0")
        pinned = values.get("distributionSha256Sum")
        if pinned is not None and pinned != bindings["gradle"]["distribution_sha256"]:
            raise ReceiptError(f"SDK archived {kind} Gradle checksum is inconsistent")


def _sdk_validate_tar(
    archive: PathContract,
    records: list[dict[str, Any]],
    bindings: dict[str, Any],
) -> None:
    """Replay every retained SDK tar member against its sanitized inventory."""

    expected = {
        ("sdk-inputs" if record["path"] == "." else f"sdk-inputs/{record['path']}"): record
        for record in records
    }
    controls: dict[str, bytes] = {}
    control_names = {
        "node/package-lock.json",
        "node/node_modules/.package-lock.json",
        "openapi/package-lock.json",
        "openapi/node_modules/.package-lock.json",
        "swiftpm/Package.resolved",
        "gradle/java-gradle-wrapper.properties",
        "gradle/kotlin-gradle-wrapper.properties",
    }
    control_names.update(
        f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
        for item in bindings["swiftpm"]["resolved_revisions"]
    )
    try:
        with tarfile.open(archive.path, mode="r:") as stream:
            members = stream.getmembers()
            if len(members) != len(expected) or {item.name for item in members} != set(expected):
                raise ReceiptError("SDK retained tar inventory is not exact")
            for member in members:
                record = expected[member.name]
                kind = record["kind"]
                if (
                    member.uid != 0
                    or member.gid != 0
                    or member.mtime != 0
                    or member.mode != int(record["mode"], 8)
                    or (kind == "directory" and not member.isdir())
                    or (kind == "symlink" and (not member.issym() or member.linkname != record["target"]))
                    or (kind == "file" and (not member.isfile() or member.size != record["size"]))
                ):
                    raise ReceiptError("SDK retained tar member metadata changed")
                if kind != "file":
                    continue
                source = stream.extractfile(member)
                if source is None:
                    raise ReceiptError("SDK retained tar file is unavailable")
                digest = hashlib.sha256()
                captured = bytearray()
                relative = member.name.removeprefix("sdk-inputs/")
                while block := source.read(1024 * 1024):
                    digest.update(block)
                    if relative in control_names:
                        captured.extend(block)
                        if len(captured) > 16 * 1024 * 1024:
                            raise ReceiptError("SDK control file exceeds its bound")
                if digest.hexdigest() != record["sha256"]:
                    raise ReceiptError("SDK retained tar file digest changed")
                if relative in control_names:
                    controls[relative] = bytes(captured)
    except (OSError, tarfile.TarError) as error:
        raise ReceiptError("SDK retained tar is malformed") from error
    _sdk_validate_control_files(controls, bindings)
    after = _bounded_path_contract(
        archive.path,
        "SDK dependency archive after replay",
        maximum_bytes=_MAX_SDK_ARCHIVE_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    if after != archive:
        raise ReceiptError("SDK retained tar changed during replay")


def _sdk_public_archive(
    contract: EvidenceSnapshot | PathContract,
    *,
    archive_id: str,
    archive_name: str,
) -> dict[str, Any]:
    return {
        "archive_id": archive_id,
        "archive_name": archive_name,
        "mode": f"{contract.mode:04o}",
        "sha256": contract.sha256,
        "size_bytes": contract.size,
    }


def _validate_sdk_dependency_evidence(
    *,
    archive_path: Path,
    input_inventory_path: Path,
    final_work_inventory_path: Path,
    release_root: Path,
    source_manifest_path: Path,
    source_manifest_sha256: str,
    expected_git_sha256: str,
    bootstrap_private_inputs_available: bool,
) -> dict[str, Any]:
    """Authenticate the retained path-free SDK dependency closure."""

    invocation_root = release_root.parent
    exact = {
        archive_path: invocation_root / "sdk-dependency-bundle.tar",
        input_inventory_path: invocation_root / "sdk-dependency-input.json",
        final_work_inventory_path: invocation_root / "sdk-dependency-work-final.json",
    }
    if any(actual != expected for actual, expected in exact.items()):
        raise ReceiptError("SDK dependency evidence paths are not exact retained paths")
    archive = _bounded_path_contract(
        archive_path,
        "SDK dependency archive",
        maximum_bytes=_MAX_SDK_ARCHIVE_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    inventory = _bounded_evidence_snapshot(
        input_inventory_path,
        "SDK dependency input inventory",
        maximum_bytes=_MAX_SDK_INVENTORY_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    final_work = _bounded_evidence_snapshot(
        final_work_inventory_path,
        "SDK dependency final-work inventory",
        maximum_bytes=_MAX_SDK_INVENTORY_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    document = _require_exact_json_fields(
        _decode_canonical_json(inventory.data, "SDK dependency input inventory"),
        {
            "format", "schema_version", "archive_id", "source_disclosure",
            "source_manifest_sha256", "source_state_sha256", "bindings",
            "archive", "record_count", "file_bytes", "records",
            "work_initial_record_count", "work_initial_file_bytes",
            "work_initial_records",
        },
        "SDK dependency input inventory",
    )
    if (
        document["format"] != "iroha-sumeragi-v2-sdk-dependency-bundle"
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 1
        or document["archive_id"] != "release-sdk-dependencies.bundle.v1"
        or document["source_disclosure"] != "withheld"
        or document["source_manifest_sha256"] != source_manifest_sha256
    ):
        raise ReceiptError("SDK dependency input inventory identity is not exact")
    _require_digest(document["source_state_sha256"], "SDK source-state digest")
    records, by_path, file_bytes = _sdk_inventory_records(
        document["records"], "SDK dependency input inventory", root_mode="0500"
    )
    work_records, _, work_file_bytes = _sdk_inventory_records(
        document["work_initial_records"],
        "SDK dependency initial-work inventory",
        root_mode="0700",
    )
    if (
        type(document["record_count"]) is not int
        or document["record_count"] != len(records)
        or type(document["file_bytes"]) is not int
        or document["file_bytes"] != file_bytes
        or type(document["work_initial_record_count"]) is not int
        or document["work_initial_record_count"] != len(work_records)
        or type(document["work_initial_file_bytes"]) is not int
        or document["work_initial_file_bytes"] != work_file_bytes
    ):
        raise ReceiptError("SDK dependency inventory accounting is not exact")
    bindings = _sdk_binding_contract(document["bindings"], by_path)
    if bootstrap_private_inputs_available:
        _sdk_validate_private_source_manifest(
            path=source_manifest_path,
            expected_sha256=source_manifest_sha256,
            archive_records=records,
            bindings=bindings,
            expected_git_sha256=expected_git_sha256,
        )
    elif os.path.lexists(source_manifest_path):
        raise ReceiptError(
            "bootstrap-private SDK source manifest survived acknowledgment pruning"
        )
    expected_archive = _sdk_public_archive(
        archive,
        archive_id="release-sdk-dependencies.bundle.v1",
        archive_name="sdk-dependency-bundle.tar",
    )
    if document["archive"] != expected_archive:
        raise ReceiptError("SDK dependency tar binding is not exact")
    _sdk_validate_tar(archive, records, bindings)

    final_document = _require_exact_json_fields(
        _decode_canonical_json(final_work.data, "SDK final-work inventory"),
        {
            "format", "schema_version", "archive_id",
            "sdk_dependency_inventory_sha256", "record_count", "file_bytes",
            "records",
        },
        "SDK final-work inventory",
    )
    final_records, _, final_bytes = _sdk_inventory_records(
        final_document["records"], "SDK final-work inventory", root_mode="0700"
    )
    if (
        final_document["format"]
        != "iroha-sumeragi-v2-sdk-dependency-work-final"
        or type(final_document["schema_version"]) is not int
        or final_document["schema_version"] != 1
        or final_document["archive_id"]
        != "release-sdk-dependencies.work-final.v1"
        or final_document["sdk_dependency_inventory_sha256"] != inventory.sha256
        or type(final_document["record_count"]) is not int
        or final_document["record_count"] != len(final_records)
        or type(final_document["file_bytes"]) is not int
        or final_document["file_bytes"] != final_bytes
        or final_records != work_records
        or final_bytes != work_file_bytes
    ):
        raise ReceiptError("SDK final-work inventory binding is not exact")
    return {
        "schema_version": 1,
        "source_disclosure": "withheld",
        "source_manifest_sha256": source_manifest_sha256,
        "source_state_sha256": document["source_state_sha256"],
        "archive": expected_archive,
        "input_inventory": _sdk_public_archive(
            inventory,
            archive_id="release-sdk-dependencies.input-inventory.v1",
            archive_name="sdk-dependency-input.json",
        ),
        "final_work_inventory": _sdk_public_archive(
            final_work,
            archive_id="release-sdk-dependencies.work-final.v1",
            archive_name="sdk-dependency-work-final.json",
        ),
    }


def _validate_scaling_evidence(
    *,
    manifest_path: Path,
    sealed: dict[str, Any],
    repo_root: Path,
    checker_environment: dict[str, str],
    expected_trial_harness_sha256: str,
    expected_configuration_sha256: str,
    expected_irohad_sha256: str,
    expected_iroha_cli_sha256: str,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    expected_trial_harness_sha256 = _require_digest(
        expected_trial_harness_sha256,
        "expected scaling trial harness digest",
    )
    expected_configuration_sha256 = _require_digest(
        expected_configuration_sha256,
        "expected scaling configuration digest",
    )
    expected_irohad_sha256 = _require_digest(
        expected_irohad_sha256,
        "expected scaling irohad digest",
    )
    expected_iroha_cli_sha256 = _require_digest(
        expected_iroha_cli_sha256,
        "expected scaling iroha CLI digest",
    )
    if (
        not manifest_path.is_absolute()
        or Path(os.path.abspath(manifest_path)) != manifest_path
        or manifest_path.name != "scaling_evidence.json"
    ):
        raise ReceiptError(
            "scaling evidence manifest must be the absolute normalized "
            "scaling_evidence.json bundle root file"
        )
    root = manifest_path.parent
    files, directories, total_bytes = _capture_scaling_bundle(root)
    contracts = dict(files)
    manifest_contract = contracts.get("scaling_evidence.json")
    report_contract = contracts.get("validation_report.json")
    if manifest_contract is None:
        raise ReceiptError("scaling evidence manifest is absent from its bundle")
    if report_contract is None:
        raise ReceiptError(
            "scaling evidence bundle lacks canonical validation_report.json"
        )
    if manifest_contract.path != manifest_path:
        raise ReceiptError("scaling evidence manifest path is not its exact bundle file")

    manifest_data, manifest = _load_scaling_json(
        manifest_path, "scaling evidence manifest"
    )
    report_data, report = _load_scaling_json(
        report_contract.path, "scaling validation report"
    )
    if set(report) != {
        "schema",
        "result",
        "manifest_sha256",
        "errors",
        "metrics",
    }:
        raise ReceiptError("scaling validation report fields are not canonical")
    if (
        report.get("schema") != _SCALING_REPORT_SCHEMA
        or report.get("result") != "pass"
        or report.get("errors") != []
        or not isinstance(report.get("metrics"), dict)
        or report.get("manifest_sha256") != manifest_contract.sha256
    ):
        raise ReceiptError(
            "scaling validation report is not an exact pass for this manifest"
        )
    canonical_report = (
        json.dumps(report, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    if report_data != canonical_report:
        raise ReceiptError("scaling validation report is not canonical JSON")
    if hashlib.sha256(report_data).hexdigest() != report_contract.sha256:
        raise ReceiptError("scaling validation report changed while decoded")

    _, identity_contract = _scaling_ref_path(
        manifest.get("identity"),
        root=root,
        contracts=contracts,
        name="scaling identity",
    )
    identity_data, identity = _load_scaling_json(
        identity_contract.path, "scaling identity"
    )
    if hashlib.sha256(identity_data).hexdigest() != identity_contract.sha256:
        raise ReceiptError("scaling identity changed while decoded")
    software = identity.get("software")
    if not isinstance(software, dict):
        raise ReceiptError("scaling identity lacks its software binding")
    if software.get("source_revision") != sealed["head_commit"]:
        raise ReceiptError(
            "scaling identity source_revision is not the sealed head_commit"
        )
    if (
        software.get("workspace_source_sha256")
        != sealed["workspace_source_manifest_sha256"]
    ):
        raise ReceiptError(
            "scaling identity workspace_source_sha256 is not the sealed "
            "workspace manifest"
        )
    if software.get("irohad_sha256") != expected_irohad_sha256:
        raise ReceiptError(
            "scaling identity irohad_sha256 is not the authenticated digest"
        )
    if software.get("iroha_cli_sha256") != expected_iroha_cli_sha256:
        raise ReceiptError(
            "scaling identity iroha_cli_sha256 is not the authenticated digest"
        )

    _, configuration_contract = _scaling_ref_path(
        manifest.get("configuration"),
        root=root,
        contracts=contracts,
        name="scaling configuration",
    )
    if configuration_contract.sha256 != expected_configuration_sha256:
        raise ReceiptError(
            "scaling configuration is not the authenticated digest"
        )
    _, trial_harness_contract = _scaling_ref_path(
        manifest.get("trial_harness"),
        root=root,
        contracts=contracts,
        name="scaling trial harness",
    )
    if trial_harness_contract.sha256 != expected_trial_harness_sha256:
        raise ReceiptError(
            "scaling trial harness is not the authenticated digest"
        )

    tooling = manifest.get("tooling")
    if (
        not isinstance(tooling, list)
        or len(tooling) != len(_SCALING_REQUIRED_TOOLING)
    ):
        raise ReceiptError(
            "scaling tooling does not contain the exact retained tool set"
        )
    retained_tooling: list[tuple[str, str, PathContract]] = []
    for index, ((role, source_path), entry) in enumerate(
        zip(_SCALING_REQUIRED_TOOLING, tooling)
    ):
        if (
            not isinstance(entry, dict)
            or set(entry) != {"role", "source_path", "artifact"}
            or entry.get("role") != role
            or entry.get("source_path") != source_path
        ):
            raise ReceiptError(
                f"scaling tooling entry {index} is not the retained {role} tool"
            )
        _, archived_tool = _scaling_ref_path(
            entry.get("artifact"),
            root=root,
            contracts=contracts,
            name=f"scaling archived {role} tool",
        )
        retained_path = repo_root.joinpath(*PurePosixPath(source_path).parts)
        retained_tool = _capture_path_contract(
            retained_path,
            f"retained scaling {role} tool",
            expected_sha256=None,
            expected_owner=os.geteuid(),
            expected_nlink=1,
        )
        if archived_tool.sha256 != retained_tool.sha256:
            raise ReceiptError(
                f"scaling archived {role} tool is not the retained sealed tool"
            )
        retained_tooling.append((role, source_path, retained_tool))

    retained_validator = (
        repo_root
        / "scripts"
        / "nexus"
        / "validate_multilane_scaling_evidence.py"
    )
    retained_contract = _capture_path_contract(
        retained_validator,
        "retained scaling evidence validator",
        expected_sha256=None,
        expected_owner=os.geteuid(),
        expected_nlink=1,
    )
    _, archived_validator = _scaling_ref_path(
        manifest.get("validator"),
        root=root,
        contracts=contracts,
        name="archived scaling validator",
    )
    if archived_validator.sha256 != retained_contract.sha256:
        raise ReceiptError(
            "archived scaling validator is not the retained sealed validator"
        )

    with tempfile.TemporaryDirectory(prefix="sumeragi-v2-scaling-replay-") as temporary:
        replay_report = Path(temporary).resolve(strict=True) / "validation_report.json"
        status, _, _ = _run_bounded_python_validator(
            retained_validator,
            [
                str(manifest_path),
                "--expected-source-revision",
                sealed["head_commit"],
                "--expected-workspace-source-sha256",
                sealed["workspace_source_manifest_sha256"],
                "--expected-validator-sha256",
                retained_contract.sha256,
                "--expected-trial-harness-sha256",
                expected_trial_harness_sha256,
                "--expected-configuration-sha256",
                expected_configuration_sha256,
                "--expected-irohad-sha256",
                expected_irohad_sha256,
                "--expected-iroha-cli-sha256",
                expected_iroha_cli_sha256,
                "--expected-repository-root",
                str(repo_root),
                "--report",
                str(replay_report),
                "--quiet",
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="retained scaling evidence validator",
        )
        if status != 0:
            raise ReceiptError(
                "scaling evidence bundle failed retained-validator revalidation"
            )
        replay_data, replay = _load_scaling_json(
            replay_report, "recomputed scaling validation report"
        )
        if replay != report or replay_data != report_data:
            raise ReceiptError(
                "scaling validation report does not match retained revalidation"
            )

    final_files, final_directories, final_total = _capture_scaling_bundle(root)
    if (
        final_files != files
        or final_directories != directories
        or final_total != total_bytes
    ):
        raise ReceiptError(
            "scaling evidence bundle changed during retained revalidation"
        )
    final_retained = _capture_path_contract(
        retained_validator,
        "retained scaling evidence validator after replay",
        expected_sha256=retained_contract.sha256,
        expected_mode=retained_contract.mode,
        expected_owner=retained_contract.owner,
        expected_nlink=retained_contract.nlink,
        expected_size=retained_contract.size,
    )
    if final_retained != retained_contract:
        raise ReceiptError("retained scaling evidence validator changed during replay")
    for role, _, retained_tool in retained_tooling:
        final_tool = _capture_path_contract(
            retained_tool.path,
            f"retained scaling {role} tool after replay",
            expected_sha256=retained_tool.sha256,
            expected_mode=retained_tool.mode,
            expected_owner=retained_tool.owner,
            expected_nlink=retained_tool.nlink,
            expected_size=retained_tool.size,
        )
        if final_tool != retained_tool:
            raise ReceiptError(
                f"retained scaling {role} tool changed during replay"
            )
    if hashlib.sha256(manifest_data).hexdigest() != manifest_contract.sha256:
        raise ReceiptError("scaling evidence manifest changed while decoded")

    bundle = {
        "archive_id": "release-scaling.bundle.v1",
        "file_count": len(files),
        "total_size_bytes": total_bytes,
        "directories": directories,
        "files": [
            {
                "archive_id": "release-scaling.file.v1:" + relative,
                "relative_path": relative,
                "sha256": contract.sha256,
                "size_bytes": contract.size,
                "mode": f"{contract.mode:04o}",
            }
            for relative, contract in files
        ],
    }
    trust_anchors = {
        "trial_harness_sha256": expected_trial_harness_sha256,
        "configuration_sha256": expected_configuration_sha256,
        "irohad_sha256": expected_irohad_sha256,
        "iroha_cli_sha256": expected_iroha_cli_sha256,
        "retained_tooling": [
            {
                "role": role,
                "archive_id": f"release-scaling.retained-tool.{role}.v1",
                "sha256": contract.sha256,
                "size_bytes": contract.size,
                "mode": f"{contract.mode:04o}",
            }
            for role, _, contract in retained_tooling
        ],
    }
    return bundle, {
        "archive_id": "release-scaling.retained-validator.v1",
        "sha256": retained_contract.sha256,
        "size_bytes": retained_contract.size,
        "mode": f"{retained_contract.mode:04o}",
    }, trust_anchors


def _read_g12_snapshot(
    path: Path, name: str, *, maximum_bytes: int
) -> EvidenceSnapshot:
    return _read_evidence_snapshot(
        path,
        name,
        maximum_bytes=maximum_bytes,
        allowed_owners={os.geteuid()},
    )


def _decode_g12_tsv(
    snapshot: EvidenceSnapshot,
    name: str,
    *,
    expected_header: tuple[str, ...] | None = None,
) -> list[list[str]]:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} must be terminal-LF, LF-only TSV")
    try:
        text = data.decode("utf-8")
        rows = list(csv.reader(io.StringIO(text), delimiter="\t", strict=True))
    except (UnicodeDecodeError, csv.Error) as error:
        raise ReceiptError(f"{name} is not strict UTF-8 TSV") from error
    if not rows or any(not row or any(not field for field in row) for row in rows):
        raise ReceiptError(f"{name} contains an empty TSV field")
    if expected_header is not None and tuple(rows[0]) != expected_header:
        raise ReceiptError(f"{name} does not have its exact canonical header")
    return rows


def _g12_completion_fields(
    snapshot: EvidenceSnapshot, name: str
) -> dict[str, str]:
    rows = _decode_g12_tsv(snapshot, name)
    fields: dict[str, str] = {}
    for row in rows:
        if len(row) != 2 or row[0] in fields:
            raise ReceiptError(f"{name} contains malformed or duplicate fields")
        fields[row[0]] = row[1]
    return fields


def _validate_g12_log(snapshot: EvidenceSnapshot, name: str, test: str) -> None:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} is not terminal-LF, LF-only output")
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error
    results = [line for line in lines if line.startswith("test result:")]
    if (
        lines.count("running 1 test") != 1
        or lines.count(f"test {test} ... ok") != 1
        or len(results) != 1
        or re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"[0-9]+ filtered out; finished in .+",
            results[0],
        )
        is None
    ):
        raise ReceiptError(f"{name} does not prove one exact passing G-12P test")


def _require_g12_directory_inventory(
    directory: Path, expected_names: set[str], name: str
) -> None:
    try:
        metadata = directory.lstat()
        entries = list(directory.iterdir())
    except OSError as error:
        raise ReceiptError(f"{name} directory is unavailable") from error
    if (
        directory.resolve(strict=True) != directory
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            f"{name} directory must be an owner-owned resolved non-symlink directory"
        )
    actual_names: set[str] = set()
    for entry in entries:
        if (
            _SCALING_SAFE_PATH_COMPONENT_RE.fullmatch(entry.name) is None
            or entry.name in actual_names
        ):
            raise ReceiptError(f"{name} directory contains an unsafe entry")
        actual_names.add(entry.name)
        try:
            entry_metadata = entry.lstat()
        except OSError as error:
            raise ReceiptError(f"{name} directory entry is unavailable") from error
        if (
            stat.S_ISLNK(entry_metadata.st_mode)
            or not stat.S_ISREG(entry_metadata.st_mode)
            or entry_metadata.st_uid != os.geteuid()
            or entry_metadata.st_nlink != 1
        ):
            raise ReceiptError(
                f"{name} directory contains a nonregular or aliased artifact"
            )
    if actual_names != expected_names:
        raise ReceiptError(
            f"{name} directory inventory differs from its exact evidence schema"
        )


def _validate_g4p_log(snapshot: EvidenceSnapshot, name: str, test: str) -> None:
    data = snapshot.data
    if not data.endswith(b"\n") or b"\r" in data or b"\0" in data:
        raise ReceiptError(f"{name} is not terminal-LF, LF-only output")
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{name} is not UTF-8") from error
    results = [line for line in lines if line.startswith("test result:")]
    native_marker_count = lines.count(_G4P_NATIVE_AMX_GROUPED_PRUNING_MARKER)
    expected_native_marker_count = int(test == _G4P_RELEASE_TESTS[3][1])
    release_marker_count = int(
        test in (_G4P_RELEASE_TESTS[0][1], _G4P_RELEASE_TESTS[3][1])
    )
    if (
        lines.count("running 1 test") != 1
        or lines.count(f"test {test} ... ok") != 1
        or lines.count(f"[multilane-release-gate] started: {test}")
        != release_marker_count
        or lines.count(f"[multilane-release-gate] completed: {test}")
        != release_marker_count
        or native_marker_count != expected_native_marker_count
        or any("developer opt-out" in line for line in lines)
        or len(results) != 1
        or re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"[0-9]+ filtered out; finished in .+",
            results[0],
        )
        is None
    ):
        raise ReceiptError(
            f"{name} does not prove one exact passing mandatory G-4P test"
        )


def _validate_g4p_evidence(
    *,
    completion_path: Path,
    sealed: dict[str, Any],
    prebuilt_manifest_sha256: str,
) -> dict[str, Any]:
    completion = _read_g12_snapshot(
        completion_path,
        "G-4P completion",
        maximum_bytes=_MAX_G4P_TSV_BYTES,
    )
    completion_fields = _g12_completion_fields(completion, "G-4P completion")
    expected_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "skipped_runs",
        "native_grouped_pruning_evidence",
        "runs_sha256",
    }
    if set(completion_fields) != expected_fields:
        raise ReceiptError("G-4P completion fields are not canonical")
    expected_identity = {
        "schema_version": "1",
        "mode": "mandatory-four-peer-multilane-release",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "expected_runs": "4",
        "passed_runs": "4",
        "failed_runs": "0",
        "skipped_runs": "0",
        "native_grouped_pruning_evidence": "passed",
    }
    if any(
        completion_fields.get(field) != value
        for field, value in expected_identity.items()
    ):
        raise ReceiptError(
            "G-4P completion is not exact passing release-bound accounting"
        )
    _require_digest(completion_fields["runs_sha256"], "G-4P run summary digest")

    summary = _read_g12_snapshot(
        completion.path.with_name("runs.tsv"),
        "G-4P run summary",
        maximum_bytes=_MAX_G4P_TSV_BYTES,
    )
    if summary.sha256 != completion_fields["runs_sha256"]:
        raise ReceiptError("G-4P run summary digest mismatch")
    rows = _decode_g12_tsv(
        summary,
        "G-4P run summary",
        expected_header=("target", "test", "status", "log_sha256", "log"),
    )
    if len(rows) != len(_G4P_RELEASE_TESTS) + 1:
        raise ReceiptError("G-4P run summary must contain exactly four runs")

    logs: list[EvidenceSnapshot] = []
    expected_names = {"COMPLETED.tsv", "runs.tsv"}
    for index, ((target, test), row) in enumerate(
        zip(_G4P_RELEASE_TESTS, rows[1:])
    ):
        expected_log = f"run-{index:02d}-{target}.log"
        if (
            len(row) != 5
            or tuple(row[:3]) != (target, test, "passed")
            or _DIGEST_RE.fullmatch(row[3]) is None
            or row[4] != expected_log
        ):
            raise ReceiptError(f"G-4P run summary row {index} is not canonical")
        log = _read_g12_snapshot(
            completion.path.with_name(expected_log),
            f"G-4P run log {index}",
            maximum_bytes=_MAX_G4P_LOG_BYTES,
        )
        if log.sha256 != row[3]:
            raise ReceiptError(f"G-4P run log {index} digest mismatch")
        _validate_g4p_log(log, f"G-4P run log {index}", test)
        logs.append(log)
        expected_names.add(expected_log)
    _require_g12_directory_inventory(
        completion.path.parent,
        expected_names,
        "G-4P evidence",
    )

    return {
        "schema_version": 1,
        "completion": _snapshot_receipt_artifact(completion),
        "run_summary": _snapshot_receipt_artifact(summary),
        "run_logs": [
            _snapshot_receipt_artifact(snapshot) for snapshot in logs
        ],
    }


def _validate_g12_evidence(
    *,
    seed_completion_path: Path,
    fault_soak_completion_path: Path,
    sealed: dict[str, Any],
    prebuilt_manifest_sha256: str,
) -> dict[str, Any]:
    seed_completion = _read_g12_snapshot(
        seed_completion_path,
        "G-12P seed completion",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    seed_fields = _g12_completion_fields(
        seed_completion, "G-12P seed completion"
    )
    expected_seed_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "process_retry_runs",
        "runs_sha256",
    }
    if set(seed_fields) != expected_seed_fields:
        raise ReceiptError("G-12P seed completion fields are not canonical")
    expected_identity = {
        "schema_version": "1",
        "mode": "deterministic-seed-matrix",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "expected_runs": "10",
        "passed_runs": "10",
        "failed_runs": "0",
        "process_retry_runs": "0",
    }
    if any(seed_fields.get(field) != value for field, value in expected_identity.items()):
        raise ReceiptError(
            "G-12P seed completion is not exact passing release-bound accounting"
        )
    _require_digest(seed_fields["runs_sha256"], "G-12P seed summary digest")
    seed_summary_path = seed_completion.path.with_name("runs.tsv")
    seed_summary = _read_g12_snapshot(
        seed_summary_path,
        "G-12P seed summary",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    if seed_summary.sha256 != seed_fields["runs_sha256"]:
        raise ReceiptError("G-12P seed summary digest mismatch")
    rows = _decode_g12_tsv(
        seed_summary,
        "G-12P seed summary",
        expected_header=(
            "ordinal",
            "seed",
            "status",
            "process_retries",
            "log_sha256",
            "log",
        ),
    )
    if len(rows) != 11:
        raise ReceiptError("G-12P seed summary must contain exactly ten runs")
    seed_logs: list[EvidenceSnapshot] = []
    expected_seed_names = {"COMPLETED.tsv", "runs.tsv"}
    for ordinal, row in enumerate(rows[1:]):
        expected_log = f"seed-{ordinal:02d}.log"
        expected_row = (
            str(ordinal),
            f"{_G12_SEED_PREFIX}{ordinal:02d}",
            "passed",
            "0",
        )
        if (
            len(row) != 6
            or tuple(row[:4]) != expected_row
            or row[5] != expected_log
            or _DIGEST_RE.fullmatch(row[4]) is None
        ):
            raise ReceiptError(
                f"G-12P seed summary row {ordinal} is not canonical"
            )
        log = _read_g12_snapshot(
            seed_completion.path.with_name(expected_log),
            f"G-12P seed log {ordinal}",
            maximum_bytes=_MAX_G12_LOG_BYTES,
        )
        if log.sha256 != row[4]:
            raise ReceiptError(f"G-12P seed log {ordinal} digest mismatch")
        _validate_g12_log(log, f"G-12P seed log {ordinal}", _G12_SEED_TEST)
        seed_logs.append(log)
        expected_seed_names.add(expected_log)
    _require_g12_directory_inventory(
        seed_completion.path.parent,
        expected_seed_names,
        "G-12P seed evidence",
    )

    soak_completion = _read_g12_snapshot(
        fault_soak_completion_path,
        "G-12P fault-soak completion",
        maximum_bytes=_MAX_G12_TSV_BYTES,
    )
    soak_fields = _g12_completion_fields(
        soak_completion, "G-12P fault-soak completion"
    )
    expected_soak_fields = {
        "schema_version",
        "mode",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "prebuilt_manifest_sha256",
        "seed",
        "duration_seconds",
        "expected_runs",
        "passed_runs",
        "failed_runs",
        "process_retry_runs",
        "log_sha256",
    }
    if set(soak_fields) != expected_soak_fields:
        raise ReceiptError("G-12P fault-soak completion fields are not canonical")
    expected_soak = {
        "schema_version": "1",
        "mode": "two-hour-fault-soak",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
        "prebuilt_manifest_sha256": prebuilt_manifest_sha256,
        "seed": f"{_G12_SEED_PREFIX}00",
        "duration_seconds": "7200",
        "expected_runs": "1",
        "passed_runs": "1",
        "failed_runs": "0",
        "process_retry_runs": "0",
    }
    if any(soak_fields.get(field) != value for field, value in expected_soak.items()):
        raise ReceiptError(
            "G-12P fault-soak completion is not exact passing "
            "release-bound accounting"
        )
    _require_digest(soak_fields["log_sha256"], "G-12P fault-soak log digest")
    soak_log = _read_g12_snapshot(
        soak_completion.path.with_name("fault-soak.log"),
        "G-12P fault-soak log",
        maximum_bytes=_MAX_G12_LOG_BYTES,
    )
    if soak_log.sha256 != soak_fields["log_sha256"]:
        raise ReceiptError("G-12P fault-soak log digest mismatch")
    _validate_g12_log(soak_log, "G-12P fault-soak log", _G12_SOAK_TEST)
    _require_g12_directory_inventory(
        soak_completion.path.parent,
        {"COMPLETED.tsv", "fault-soak.log"},
        "G-12P fault-soak evidence",
    )
    if seed_completion.path.parent == soak_completion.path.parent:
        raise ReceiptError("G-12P seed and fault-soak evidence must be distinct")

    return {
        "seed_completion": _snapshot_receipt_artifact(seed_completion),
        "seed_summary": _snapshot_receipt_artifact(seed_summary),
        "seed_run_logs": [
            _snapshot_receipt_artifact(snapshot) for snapshot in seed_logs
        ],
        "fault_soak_completion": _snapshot_receipt_artifact(soak_completion),
        "fault_soak_log": _snapshot_receipt_artifact(soak_log),
    }


def _runtime_tool_probe_evidence(
    *,
    manifest_path: Path,
    result_path: Path,
    release_root: Path,
    bootstrap_authentication: dict[str, Any],
    bootstrap_evidence: dict[str, Any],
    bootstrap_evidence_root: Path,
    runtime_available: bool,
) -> tuple[dict[str, Any], list[PathContract]]:
    """Replay live probes, or authenticate the path-free retained result."""

    expected_parent = release_root.parent
    if (
        manifest_path.parent != expected_parent
        or result_path.parent != expected_parent
        or manifest_path.name != "runtime-tool-probe-manifest.json"
        or result_path.name != "runtime-tool-probe-result.json"
    ):
        raise ReceiptError("runtime tool-probe evidence path is not exact")
    bootstrap_tools = bootstrap_authentication["runner"]["tools"]
    if not isinstance(bootstrap_tools, dict) or len(bootstrap_tools) != 41:
        raise ReceiptError("bootstrap tool closure is not exact")
    result = _bounded_evidence_snapshot(
        result_path,
        "runtime tool probe result",
        maximum_bytes=1024 * 1024,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
    )
    contracts: list[PathContract] = [_snapshot_contract(result)]
    if runtime_available:
        manifest_snapshot = _bounded_evidence_snapshot(
            manifest_path,
            "runtime tool probe manifest",
            maximum_bytes=1024 * 1024,
            expected_mode=0o400,
            allowed_owners={os.geteuid()},
        )
        manifest_value = _require_exact_json_fields(
            _decode_canonical_json(
                manifest_snapshot.data, "runtime tool probe manifest"
            ),
            {"schema_version", "tools"},
            "runtime tool probe manifest",
        )
        manifest_tools = manifest_value["tools"]
        if (
            type(manifest_value["schema_version"]) is not int
            or manifest_value["schema_version"] != 1
            or not isinstance(manifest_tools, dict)
            or set(manifest_tools) != set(bootstrap_tools)
            or len(manifest_tools) != 41
        ):
            raise ReceiptError(
                "runtime tool probe manifest inventory is not exact"
            )
        runtime_root = expected_parent / "runtime"
        tools: dict[str, PathContract] = {}
        for name in sorted(manifest_tools):
            manifest_record = _require_exact_json_fields(
                manifest_tools[name],
                {"archive_id", "path", "sha256"},
                f"runtime tool probe manifest {name}",
            )
            if not isinstance(manifest_record["path"], str):
                raise ReceiptError(
                    f"runtime tool probe {name} path is not text"
                )
            path = Path(manifest_record["path"])
            if (
                manifest_record["archive_id"]
                != f"release-runtime-tool.{name}.v1"
                or not path.is_absolute()
                or Path(os.path.abspath(path)) != path
                or runtime_root not in path.parents
            ):
                raise ReceiptError(f"runtime tool probe {name} path escaped")
            tool = _bounded_path_contract(
                path,
                f"runtime tool probe {name}",
                maximum_bytes=_MAX_TOOL_BYTES,
                expected_mode=0o500,
                allowed_owners={os.geteuid()},
                require_single_link=True,
                executable=True,
            )
            bootstrap_record = bootstrap_tools[name]
            if (
                manifest_record["sha256"] != tool.sha256
                or bootstrap_record["sha256"] != tool.sha256
                or bootstrap_record["size_bytes"] != tool.size
            ):
                raise ReceiptError(
                    f"runtime tool probe {name} differs from bootstrap closure"
                )
            tools[name] = tool
        helper_record = bootstrap_evidence["trusted_inputs"][
            "tool_probe_helper"
        ]
        helper = _bounded_path_contract(
            bootstrap_evidence_root / "probe-release-tools.py",
            "bootstrap tool probe helper",
            maximum_bytes=_MAX_HELPER_BYTES,
            expected_mode=0o400,
            allowed_owners={os.geteuid()},
        )
        if helper.sha256 != helper_record["sha256"]:
            raise ReceiptError("bootstrap tool probe helper changed")
        python_candidates = (
            bootstrap_evidence_root / "python3",
            bootstrap_evidence_root / "python-runtime" / "bin" / "python3",
        )
        present_python = [
            path for path in python_candidates
            if path.exists() and not path.is_symlink()
        ]
        if len(present_python) != 1:
            raise ReceiptError(
                "bootstrap archived Python layout is not exact"
            )
        python_path = present_python[0]
        python = _bounded_path_contract(
            python_path,
            "bootstrap archived Python",
            maximum_bytes=_MAX_TOOL_BYTES,
            expected_mode=0o500,
            allowed_owners={os.geteuid()},
            require_single_link=True,
            executable=True,
        )
        manifest, replayed_result, value = (
            _validate_and_replay_tool_probe_closure(
                manifest_path=manifest_path,
                result_path=result_path,
                expected_value=None,
                tools=tools,
                python=python,
                helper=helper,
                archive_id_prefix="release-runtime-tool",
                probe_root=expected_parent / ".receipt-runtime-tool-probe",
            )
        )
        if replayed_result.sha256 != result.sha256:
            raise ReceiptError("runtime tool probe result changed during replay")
        contracts = [
            _snapshot_contract(manifest),
            _snapshot_contract(replayed_result),
            helper,
            python,
            *tools.values(),
        ]
    else:
        value = _require_exact_json_fields(
            _decode_canonical_json(result.data, "runtime tool probe result"),
            {
                "format", "host_family", "probe_contract_sha256",
                "schema_version", "tool_count", "tools",
            },
            "runtime tool probe result",
        )
        bootstrap_result = _decode_canonical_json(
            _bounded_evidence_snapshot(
                bootstrap_evidence_root / "runner-tool-probes.json",
                "bootstrap runner tool probe result",
                maximum_bytes=1024 * 1024,
                expected_mode=0o400,
                allowed_owners={os.geteuid()},
            ).data,
            "bootstrap runner tool probe result",
        )
        if (
            not isinstance(value["tools"], dict)
            or not isinstance(bootstrap_result, dict)
            or not isinstance(bootstrap_result.get("tools"), dict)
            or set(value["tools"]) != set(bootstrap_tools)
            or set(bootstrap_result["tools"]) != set(bootstrap_tools)
            or any(
                value[field] != bootstrap_result.get(field)
                for field in (
                    "format", "host_family", "probe_contract_sha256",
                    "schema_version", "tool_count",
                )
            )
        ):
            raise ReceiptError("retained runtime tool probe result is not exact")
        for name, bootstrap_tool in bootstrap_tools.items():
            runtime_record = value["tools"][name]
            bootstrap_record = bootstrap_result["tools"][name]
            if (
                not isinstance(runtime_record, dict)
                or not isinstance(bootstrap_record, dict)
                or runtime_record.get("archive_id")
                != f"release-runtime-tool.{name}.v1"
                or bootstrap_record.get("archive_id")
                != f"release-runner-tool.{name}.v1"
                or {key: item for key, item in runtime_record.items()
                    if key != "archive_id"}
                != {key: item for key, item in bootstrap_record.items()
                    if key != "archive_id"}
                or runtime_record.get("sha256") != bootstrap_tool["sha256"]
                or runtime_record.get("size_bytes")
                != bootstrap_tool["size_bytes"]
            ):
                raise ReceiptError(
                    f"retained runtime tool probe {name} is not exact"
                )
    record = {
        "format": value["format"],
        "schema_version": 1,
        "host_family": value["host_family"],
        "probe_contract_sha256": value["probe_contract_sha256"],
        "tool_count": 41,
        "result": _sdk_public_archive(
            result,
            archive_id="release-runtime.tool-probes.v1",
            archive_name="runtime-tool-probe-result.json",
        ),
    }
    return record, contracts
