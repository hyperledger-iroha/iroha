# Executed lexically in sumeragi_v2_release_receipt_test.py; do not import directly.


def _install_sdk_source_closure_fixture(
    repository_root: Path,
    writer_symbols: dict[str, object],
) -> None:
    """Install a minimal exact tracked closure for receipt integration tests."""

    resolver_relative = writer_symbols["_SDK_SOURCE_CLOSURE_RESOLVER"]
    manifest_relative = writer_symbols["_SDK_SOURCE_CLOSURE_MANIFEST"]
    native_harness = writer_symbols["_NATIVE_AMX_GROUPED_PARITY_HARNESS"]
    diagnostics_harness = writer_symbols["_SUMERAGI_SDK_DIAGNOSTICS_HARNESS"]
    native_suite = writer_symbols[
        "_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE"
    ]
    diagnostics_suite = writer_symbols[
        "_SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE"
    ]
    assert all(
        isinstance(value, str)
        for value in (
            resolver_relative,
            manifest_relative,
            native_harness,
            diagnostics_harness,
            native_suite,
            diagnostics_suite,
        )
    )
    assert isinstance(resolver_relative, str)
    assert isinstance(manifest_relative, str)
    assert isinstance(native_harness, str)
    assert isinstance(diagnostics_harness, str)
    assert isinstance(native_suite, str)
    assert isinstance(diagnostics_suite, str)

    for relative_path in (
        resolver_relative,
        native_harness,
        diagnostics_harness,
    ):
        destination = repository_root / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative_path, destination)
    production_relative = "sdk-fixture/python/client.py"
    production = repository_root / production_relative
    production.parent.mkdir(parents=True)
    production.write_text("STATUS_HEIGHT = 1\n", encoding="utf-8")
    native_fixture_relative = "fixtures/sumeragi_v2/native_amx_v2_grouped.json"
    wire_fixture_relative = "fixtures/sumeragi_v2/wire_v2.tsv"
    native_fixture = repository_root / native_fixture_relative
    native_fixture.parent.mkdir(parents=True, exist_ok=True)
    native_fixture.write_text('{"fixture_version":1}\n', encoding="utf-8")
    (repository_root / wire_fixture_relative).write_text(
        "# kind\tname\thex\texpectation\n",
        encoding="utf-8",
    )

    manifest_document = {
        "closure_roots": [
            {
                "extensions": [".py"],
                "group": "production",
                "path": "sdk-fixture/python",
                "recursive": True,
            }
        ],
        "format": "iroha-sumeragi-v2-sdk-production-source-closure",
        "groups": {
            "closure-resolver": [resolver_relative, manifest_relative],
            "diagnostics-suite": [
                diagnostics_harness,
                native_fixture_relative,
                wire_fixture_relative,
            ],
            "native-suite": [native_harness],
            "production": [production_relative],
        },
        "suites": {
            native_suite: [
                "closure-resolver",
                "native-suite",
                "production",
            ],
            diagnostics_suite: [
                "closure-resolver",
                "diagnostics-suite",
                "production",
            ],
        },
        "version": 1,
    }
    manifest = repository_root / manifest_relative
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(
        json.dumps(
            manifest_document,
            ensure_ascii=True,
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    subprocess.run(
        ["git", "init", "--quiet", str(repository_root)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    subprocess.run(
        ["git", "-C", str(repository_root), "add", "--all"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def _run_sdk_source_closure_resolver(
    repository_root: Path,
    module: ModuleType,
    suite: str,
    action: str,
) -> subprocess.CompletedProcess[str]:
    environment = {
        **os.environ,
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONHASHSEED": "0",
    }
    return subprocess.run(
        [
            sys.executable,
            str(repository_root / module._SDK_SOURCE_CLOSURE_RESOLVER),
            "--root",
            str(repository_root),
            "--manifest",
            str(repository_root / module._SDK_SOURCE_CLOSURE_MANIFEST),
            "--suite",
            suite,
            action,
        ],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
    )


def test_receipt_owns_no_duplicate_sdk_source_closure_arrays() -> None:
    source = SCRIPT.read_text(encoding="utf-8")
    component_source = SCRIPT.with_name(
        "write_sumeragi_v2_release_receipt_corridor_log.py"
    ).read_text(encoding="utf-8")
    source_closure = source + component_source
    for removed_symbol in (
        "_NATIVE_AMX_GROUPED_SUITE_SOURCE_PATHS",
        "_SUMERAGI_SDK_DIAGNOSTICS_SUITE_SOURCE_PATHS",
        "_native_amx_grouped_suite_source_manifest",
        "_sumeragi_sdk_diagnostics_suite_source_manifest",
    ):
        assert removed_symbol not in source_closure
    assert source_closure.count('"ci/resolve_sumeragi_v2_sdk_source_closure.py"') == 1
    assert source_closure.count('"ci/sumeragi_v2_sdk_source_closure.json"') == 1
    assert source_closure.count('"--manifest-sha256"') == 1


@pytest.mark.parametrize(
    "suite_symbol",
    (
        "_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE",
        "_SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE",
    ),
)
def test_receipt_sdk_digest_matches_exact_source_bound_resolver_records(
    tmp_path: Path,
    suite_symbol: str,
) -> None:
    module = load_writer_module()
    repository_root = tmp_path / "candidate"
    repository_root.mkdir()
    _install_sdk_source_closure_fixture(repository_root, vars(module))
    suite = getattr(module, suite_symbol)

    records = _run_sdk_source_closure_resolver(
        repository_root,
        module,
        suite,
        "--print-records",
    )
    assert records.returncode == 0, records.stderr
    assert records.stderr == ""
    record_lines = records.stdout.splitlines()
    assert record_lines == sorted(record_lines)
    record_paths = {line.split("\t", 1)[0] for line in record_lines}
    assert module._SDK_SOURCE_CLOSURE_RESOLVER in record_paths
    assert module._SDK_SOURCE_CLOSURE_MANIFEST in record_paths
    expected = hashlib.sha256(records.stdout.encode("utf-8")).hexdigest()
    assert module._sdk_suite_source_manifest(repository_root, suite) == expected


def test_receipt_sdk_source_closure_binds_resolver_and_fails_on_drift(
    tmp_path: Path,
) -> None:
    module = load_writer_module()
    repository_root = tmp_path / "candidate"
    repository_root.mkdir()
    _install_sdk_source_closure_fixture(repository_root, vars(module))
    suite = module._NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE
    original = module._sdk_suite_source_manifest(repository_root, suite)
    diagnostics_suite = module._SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE
    diagnostics_original = module._sdk_suite_source_manifest(
        repository_root,
        diagnostics_suite,
    )

    wire_fixture = repository_root / "fixtures/sumeragi_v2/wire_v2.tsv"
    wire_fixture.write_bytes(wire_fixture.read_bytes() + b"# tracked TSV drift\n")
    assert module._sdk_suite_source_manifest(repository_root, suite) == original
    assert (
        module._sdk_suite_source_manifest(repository_root, diagnostics_suite)
        != diagnostics_original
    )

    resolver = repository_root / module._SDK_SOURCE_CLOSURE_RESOLVER
    resolver.write_text(
        resolver.read_text(encoding="utf-8") + "\n# tracked fixture drift\n",
        encoding="utf-8",
    )
    rebound = module._sdk_suite_source_manifest(repository_root, suite)
    assert rebound != original

    unexpected = repository_root / "sdk-fixture" / "python" / "unreviewed.py"
    unexpected.write_text("UNREVIEWED = True\n", encoding="utf-8")
    with pytest.raises(module.ReceiptError, match="unexpected untracked input"):
        module._sdk_suite_source_manifest(repository_root, suite)
