"""Contract tests for SoraFS fixture and native-SDK workflow coverage."""

from __future__ import annotations

import re
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_ROOT = REPO_ROOT / ".github" / "workflows"

NATIVE_ESCROW_TRIGGER_PATHS = {
    "crates/connect_norito_bridge/**",
    "crates/iroha_core/src/smartcontracts/isi/mod.rs",
    "crates/iroha_core/src/smartcontracts/isi/escrow.rs",
    "crates/iroha_data_model/src/bin/cancel_asset_lock_fixtures.rs",
    "crates/iroha_data_model/src/escrow.rs",
    "crates/iroha_data_model/src/events/data/escrow.rs",
    "crates/iroha_data_model/src/isi/mod.rs",
    "crates/iroha_data_model/src/isi/escrow.rs",
    "crates/iroha_data_model/src/isi/registry.rs",
    "crates/iroha_data_model/src/testing/cancel_asset_lock.rs",
    "crates/kotodama_lang/src/samples/native_escrow.ko",
    "crates/kotodama_lang/src/samples/native_escrow.to",
    "crates/sorafs_manifest/**",
    "fixtures/sorafs_manifest/appeal_finance/**",
    "fixtures/sorafs_manifest/reference_sdk/**",
    "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
    "integration_tests/tests/native_escrow.rs",
}

APPEAL_FINANCE_FIXTURE_PATHS = (
    "cancel_asset_lock_v1.json",
    "cancel_asset_lock_v1.to",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.json",
    "negative/cancel_asset_lock_legacy_missing_expected_v1.to",
    "negative/cancel_asset_lock_nested_escrow_id_v1.to",
    "negative/cancel_asset_lock_noncanonical_quantity_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.json",
    "negative/cancel_asset_lock_zero_expected_v1.to",
)

APPEAL_FINANCE_SHARED_FIXTURE_TRIGGER_PATHS = {
    f"fixtures/sorafs_manifest/appeal_finance/{relative}"
    for relative in APPEAL_FINANCE_FIXTURE_PATHS
}

APPEAL_FINANCE_VALIDATION_PROFILE_TRIGGER_PATHS = {
    (
        "fixtures/sorafs_manifest/reference_sdk/"
        "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
    ),
    (
        "fixtures/sorafs_manifest/reference_sdk/"
        "appeal_finance_cancel_asset_lock_zero_expected_negative_"
        "validation_outcome_v1.json"
    ),
}

SDK_FIXTURE_READERS = {
    "IrohaSwift/Tests/IrohaSwiftTests/CancelAssetLockV1Tests.swift": (
        "testAppealFinanceReferenceFixturesAreByteExactAndFailClosed",
        "Data(contentsOf:",
        "XCTFail(",
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/"
        "core/model/instructions/CancelAssetLockInstructionTest.kt"
    ): (
        "REQUIRED_FIXTURE_NAMES.associateWith",
        "readMandatoryFixture(root, relative)",
        "check(Files.isRegularFile(path))",
        "CancelAssetLockInstruction.fromCanonicalFields(",
        "CancelAssetLockInstruction.fromWirePayload(",
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/"
        "model/instructions/CancelAssetLockInstructionTests.java"
    ): (
        "for (final String relative : REQUIRED_FIXTURE_NAMES)",
        "readMandatoryFixture(root, relative)",
        "Files.isRegularFile(path)",
    ),
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/CancelAssetLockInstructionTests.cs": (
        "public void SharedFixturesEnforceTheV1HardCut()",
        "Assert.True(",
        "File.Exists(path)",
        "return File.ReadAllBytes(path);",
    ),
    "javascript/iroha_js/test/cancelAssetLockV1.test.js": (
        "requiredFixtureNames.map",
        "fs.readFileSync(path.join(fixtureRoot, name))",
        "all eight appeal-finance CancelAssetLock fixtures are mandatory",
    ),
    "python/iroha_python/tests/cancel_asset_lock_v1_test.py": (
        "_FIXTURES = {",
        "(_FIXTURE_ROOT / name).read_bytes()",
        "test_all_eight_appeal_finance_cancel_asset_lock_fixtures_are_mandatory",
    ),
}

STRICT_NATIVE_PROFILE_MARKERS = {
    "IrohaSwift/Tests/IrohaSwiftTests/SorafsReferenceValidatorsTests.swift": (
        "guard SorafsReferenceValidators.isAppealFinanceNativeAvailable else {",
        'return XCTFail("ABI-21 appeal-finance reference bridge is required")',
    ),
    (
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/"
        "sorafs/SorafsReferenceValidatorsTest.kt"
    ): (
        "SorafsReferenceValidators.isNativeAvailable()",
        '"ABI-21 appeal-finance reference bridge is required"',
    ),
    (
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/"
        "sorafs/SorafsReferenceValidatorsTests.java"
    ): (
        "requireNativeBridge();",
        "throw new AssertionError(",
        '"ABI-21 connect_norito_bridge with all SoraFS reference symbols is required."',
    ),
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/SoraFsReferenceValidatorsTests.cs": (
        "SoraFsReferenceValidators.IsAppealFinanceAvailable()",
        '"ABI-21 appeal-finance reference bridge is required."',
    ),
    "javascript/iroha_js/test/helpers/native.js": (
        "registerNativeRequirementFailure(",
        "throw createError();",
        'error.code = "ERR_IROHA_NATIVE_TEST_REQUIREMENT"',
    ),
}

MATERIAL_CLOSURE_PATHS_BY_WORKFLOW = {
    "openapi.yml": {
        "crates/iroha_torii_shared/src/sorafs_moderation_api.rs",
        "crates/iroha_torii_shared/src/lib.rs",
        "crates/iroha_torii_shared/src/route_catalog.rs",
        "crates/iroha_torii/src/lib.rs",
        "crates/iroha_torii/src/openapi.rs",
        "crates/iroha_torii/src/sorafs/api.rs",
    },
    "mobile_sdk_artifacts.yml": {
        "IrohaSwift/Package.swift",
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift",
        "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.json",
        (
            "fixtures/sorafs_manifest/reference_sdk/"
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
        ),
        "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
        "integration_tests/tests/native_escrow.rs",
    },
    "pr_csharp.yml": {
        (
            "csharp/tests/Hyperledger.Iroha.Sdk.Tests/"
            "SoraFsReferenceValidatorsTests.cs"
        ),
        "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.json",
        (
            "fixtures/sorafs_manifest/reference_sdk/"
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
        ),
        "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
        "integration_tests/tests/native_escrow.rs",
    },
    "sorafs-orchestrator-sdk.yml": {
        "IrohaSwift/Package.swift",
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift",
        "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.json",
        (
            "fixtures/sorafs_manifest/reference_sdk/"
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
        ),
        "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
        "integration_tests/tests/native_escrow.rs",
    },
    "sorafs-fixtures-nightly.yml": {
        "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.json",
        (
            "fixtures/sorafs_manifest/reference_sdk/"
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
        ),
        "fixtures/sorafs_manifest/reference_sdk_validation_inventory_v1.json",
        "integration_tests/tests/native_escrow.rs",
    },
}


def read(relative: str) -> str:
    """Read one repository file as UTF-8."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def workflow_event_paths(workflow_name: str, event: str) -> set[str]:
    """Return one literal GitHub workflow event path filter."""

    source = (WORKFLOW_ROOT / workflow_name).read_text(encoding="utf-8")
    match = re.search(
        rf"(?m)^  {re.escape(event)}:\n"
        r"(?P<body>(?:^    [^\n]*\n)*)",
        source,
    )
    assert match is not None, f"{workflow_name} must define {event}"
    body = match.group("body")
    paths = re.search(
        r'(?m)^    paths:\n(?P<paths>(?:^      - "[^"\n]+"\n)+)',
        body,
    )
    assert paths is not None, f"{workflow_name} must define {event}.paths"
    return {
        line.removeprefix("      - ").strip().strip('"')
        for line in paths.group("paths").splitlines()
    }


def pull_request_paths(workflow_name: str) -> set[str]:
    """Return the literal pull-request path filter from one workflow."""

    return workflow_event_paths(workflow_name, "pull_request")


def workflow_filter_covers(path: str, filters: set[str]) -> bool:
    """Return whether literal GitHub path filters cover one repository path."""

    for candidate in filters:
        if candidate == path:
            return True
        if candidate.endswith("/**"):
            prefix = candidate.removesuffix("/**")
            if path == prefix or path.startswith(f"{prefix}/"):
                return True
    return False


@pytest.mark.parametrize(
    ("workflow_name", "material_paths"),
    MATERIAL_CLOSURE_PATHS_BY_WORKFLOW.items(),
)
def test_material_closure_files_are_routed_to_relevant_workflows(
    workflow_name: str,
    material_paths: set[str],
) -> None:
    """Changed V1 closure inputs must start every workflow that consumes them."""

    filters = pull_request_paths(workflow_name)
    missing_files = sorted(
        path for path in material_paths if not (REPO_ROOT / path).is_file()
    )
    assert not missing_files, f"closure inventory names missing files: {missing_files}"
    uncovered = sorted(
        path
        for path in material_paths
        if not workflow_filter_covers(path, filters)
    )
    assert not uncovered, f"{workflow_name} omits closure triggers: {uncovered}"


def test_openapi_push_and_pull_request_filters_cover_moderation_sources() -> None:
    """The canonical generator must run for moderation changes on both events."""

    for event in ("pull_request", "push"):
        filters = workflow_event_paths("openapi.yml", event)
        uncovered = sorted(
            path
            for path in MATERIAL_CLOSURE_PATHS_BY_WORKFLOW["openapi.yml"]
            if not workflow_filter_covers(path, filters)
        )
        assert not uncovered, (
            f"openapi.yml {event} omits closure triggers: {uncovered}"
        )


@pytest.mark.parametrize(
    "workflow_name",
    [
        "mobile_sdk_artifacts.yml",
        "pr_csharp.yml",
        "sorafs-orchestrator-sdk.yml",
    ],
)
def test_native_sdk_workflows_cover_appeal_finance_and_escrow_sources(
    workflow_name: str,
) -> None:
    """Every relevant SDK job reruns for all shared native-escrow inputs."""

    paths = pull_request_paths(workflow_name)
    assert not any(
        path.startswith(("jobs:", "name:", "run:", "runs-on:", "uses:"))
        for path in paths
    ), f"{workflow_name} path parser leaked workflow job fields"
    assert NATIVE_ESCROW_TRIGGER_PATHS <= paths
    assert "scripts/check_sorafs_reference_sdk_fixtures.py" in paths
    assert "scripts/tests/check_sorafs_fixture_workflow_contract_test.py" in paths


@pytest.mark.parametrize(
    "workflow_name",
    [
        "mobile_sdk_artifacts.yml",
        "pr_csharp.yml",
        "sorafs-orchestrator-sdk.yml",
    ],
)
def test_native_sdk_workflows_cover_every_shared_appeal_finance_fixture(
    workflow_name: str,
) -> None:
    """Each mandatory payload and outcome must independently rerun SDK parity."""

    required = (
        APPEAL_FINANCE_SHARED_FIXTURE_TRIGGER_PATHS
        | APPEAL_FINANCE_VALIDATION_PROFILE_TRIGGER_PATHS
    )
    missing_files = sorted(
        path for path in required if not (REPO_ROOT / path).is_file()
    )
    assert not missing_files, (
        f"appeal-finance workflow inventory names missing files: {missing_files}"
    )

    filters = pull_request_paths(workflow_name)
    uncovered = sorted(
        path for path in required if not workflow_filter_covers(path, filters)
    )
    assert not uncovered, (
        f"{workflow_name} omits appeal-finance fixture triggers: {uncovered}"
    )


@pytest.mark.parametrize(
    ("test_path", "mandatory_markers"),
    SDK_FIXTURE_READERS.items(),
)
def test_every_sdk_unconditionally_reads_all_eight_appeal_finance_fixtures(
    test_path: str,
    mandatory_markers: tuple[str, ...],
) -> None:
    """Every SDK loader must fail when any shared cancellation fixture is absent."""

    source = read(test_path)
    for fixture_path in APPEAL_FINANCE_FIXTURE_PATHS:
        assert Path(fixture_path).name in source, (
            f"{test_path} does not require {fixture_path}"
        )
    for marker in mandatory_markers:
        assert marker in source, f"{test_path} lost mandatory loader marker {marker!r}"
    assert "fixture missing; skipping" not in source.lower()
    assert "skipif" not in source.lower()


@pytest.mark.parametrize(
    ("test_path", "required_markers"),
    STRICT_NATIVE_PROFILE_MARKERS.items(),
)
def test_native_appeal_finance_profiles_fail_instead_of_skipping(
    test_path: str,
    required_markers: tuple[str, ...],
) -> None:
    """A missing ABI-21 bridge is a parity failure in every native SDK lane."""

    source = read(test_path)
    for marker in required_markers:
        assert marker in source, f"{test_path} lost strict native marker {marker!r}"
    for skip_marker in (
        "XCTSkip(",
        "Assumptions.assume",
        "assumeTrue(",
        "Assert.Skip(",
        "test.skip(",
    ):
        assert skip_marker not in source


def test_appeal_finance_fixture_directory_has_the_exact_eight_payloads() -> None:
    """The shared payload directory cannot silently add or lose a V1 fixture."""

    fixture_root = (
        REPO_ROOT / "fixtures" / "sorafs_manifest" / "appeal_finance"
    )
    actual = {
        path.relative_to(fixture_root).as_posix()
        for path in fixture_root.rglob("*")
        if path.is_file() and path.suffix in {".json", ".to"}
    }
    assert actual == set(APPEAL_FINANCE_FIXTURE_PATHS)


def test_por_generator_closed_set_count_matches_checked_in_managed_outputs() -> None:
    """Keep the generator tripwire synchronized with its closed output tree."""

    generator = read("crates/sorafs_manifest/src/bin/generate_por_fixtures.rs")
    count_match = re.search(
        r"EXPECTED_MANAGED_FIXTURE_COUNT:\s*usize\s*=\s*(\d+);",
        generator,
    )
    assert count_match is not None

    fixture_root = REPO_ROOT / "fixtures" / "sorafs_manifest"
    managed_directories = (
        "governance",
        "moderation",
        "por",
        "potr",
        "reference_sdk",
        "repair",
    )
    managed_paths = {
        path.relative_to(fixture_root).as_posix()
        for directory in managed_directories
        for path in (fixture_root / directory).rglob("*")
        if path.is_file() and path.name != "README.md"
    }
    managed_paths.add("reference_sdk_validation_inventory_v1.json")

    assert int(count_match.group(1)) == len(managed_paths) == 55
    assert {
        (
            "reference_sdk/"
            "appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json"
        ),
        (
            "reference_sdk/"
            "appeal_finance_cancel_asset_lock_zero_expected_negative_"
            "validation_outcome_v1.json"
        ),
    } <= managed_paths


def test_fixture_workflow_covers_closed_reference_sdk_inputs() -> None:
    """The nightly fixture job cannot miss wire, fixture, or checker changes."""

    paths = pull_request_paths("sorafs-fixtures-nightly.yml")
    expected = NATIVE_ESCROW_TRIGGER_PATHS - {"crates/connect_norito_bridge/**"}
    assert expected <= paths
    assert {
        "ci/check_sorafs_fixtures.sh",
        "fixtures/sorafs_manifest/**",
        "scripts/check_sorafs_reference_sdk_fixtures.py",
        "scripts/tests/check_sorafs_fixture_workflow_contract_test.py",
        "scripts/tests/check_sorafs_reference_sdk_fixtures_test.py",
    } <= paths


def test_fixture_workflow_requires_its_installed_toolchains() -> None:
    """A broken Node or Go setup is a gate failure, not a capability skip."""

    workflow = read(".github/workflows/sorafs-fixtures-nightly.yml")
    fixture_gate = read("ci/check_sorafs_fixtures.sh")
    assert "SORAFS_FIXTURE_REQUIRE_TOOLCHAIN" not in workflow
    assert "SORAFS_FIXTURE_REQUIRE_TOOLCHAIN" not in fixture_gate
    assert "require_fixture_tool node" in fixture_gate
    assert "require_fixture_tool go" in fixture_gate
    assert "error: ${check_label} requires ${tool_name}" in fixture_gate
    assert "skipping ${check_label}" not in fixture_gate


def test_reference_sdk_regeneration_is_closed_and_double_run_stable() -> None:
    """Both generators, the signed inventory, and all bytes are checked twice."""

    fixture_gate = read("ci/check_sorafs_fixtures.sh")
    cargo_commands = [
        line.strip()
        for line in fixture_gate.splitlines()
        if line.strip().startswith("cargo ")
        or line.strip().startswith("NORITO_SKIP_BINDINGS_SYNC=1 cargo ")
    ]
    assert cargo_commands
    assert all("--locked" in command for command in cargo_commands)
    assert (
        fixture_gate.count(
            "python3 scripts/check_sorafs_reference_sdk_fixtures.py"
        )
        == 2
    )
    assert "for fixture_regeneration_pass in 1 2; do" in fixture_gate
    assert "--bin cancel_asset_lock_fixtures" in fixture_gate
    assert "--bin generate_pdp_fixtures" in fixture_gate
    assert "--bin generate_por_fixtures" in fixture_gate
    assert 'copy_manifest_tree "${pass_root}"' in fixture_gate
    assert 'verify_manifest_tree_paths "${pass_root}"' in fixture_gate
    assert '--output-dir "${pass_root}/appeal_finance"' in fixture_gate
    assert '--output-dir "${pass_root}/pdp"' in fixture_gate
    assert '--output-dir "${pass_root}"' in fixture_gate
    assert '--inventory "${pass_root}/reference_sdk_validation_inventory_v1.json"' in fixture_gate
    regeneration_loop = fixture_gate.split(
        "for fixture_regeneration_pass in 1 2; do", maxsplit=1
    )[1].split("\ndone\n", maxsplit=1)[0]
    assert regeneration_loop.index("--bin generate_pdp_fixtures") < (
        regeneration_loop.index("--bin generate_por_fixtures")
    )
    assert '"git", "ls-files", "-z", "--", str(source_root)' in fixture_gate
    assert "if actual_paths != tracked_paths:" in fixture_gate
    assert "missing = sorted(tracked_paths - actual_paths)" in fixture_gate
    assert "extra = sorted(actual_paths - tracked_paths)" in fixture_gate
    assert '"fixtures/sorafs_manifest"' in fixture_gate
    assert '"byte_length": byte_length' in fixture_gate
    assert '"sha256": digest.hexdigest()' in fixture_gate
    assert "generators run in place" not in fixture_gate
    assert "Reference-SDK generators run in two isolated" in fixture_gate
    assert 'getattr(os, "O_NOFOLLOW", 0)' in fixture_gate
    assert "descriptor = os.open(path, read_flags)" in fixture_gate
    assert "path.open(" not in fixture_gate
    assert "opened.st_nlink != 1" in fixture_gate
    assert "(before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino)" in fixture_gate
    assert "opened.st_mtime_ns != after.st_mtime_ns" in fixture_gate
    assert "byte_length != after.st_size" in fixture_gate
    assert "max_copy_file_bytes = 64 << 20" in fixture_gate
    assert "changed during fixture copy" in fixture_gate
    assert "changed while it was hashed" in fixture_gate
    assert "directory_identities" in fixture_gate
    assert "manifest-checked-in.json" in fixture_gate
    assert "manifest-pass-1.json" in fixture_gate
    assert "manifest-pass-2.json" in fixture_gate
    assert "cmp -s" in fixture_gate
    assert 'fixture_snapshot_root="$(mktemp -d ' in fixture_gate
    assert 'cd -- "${fixture_snapshot_root}"' in fixture_gate
    assert "pwd -P" in fixture_gate
    assert "git status --short --untracked-files=all -- fixtures/sorafs_manifest" not in fixture_gate


def test_manifest_generator_enables_required_vrf_and_drand_crypto() -> None:
    """The standalone locked generator must compile without feature unification."""

    manifest = read("crates/sorafs_manifest/Cargo.toml")
    assert (
        'iroha_crypto = { workspace = true, default-features = false, '
        'features = ["application", "bls"] }'
    ) in manifest


def test_por_fixture_generator_has_a_strict_isolated_output_root() -> None:
    """The aggregate generator cannot ambiguously redirect fixture writes."""

    generator = read(
        "crates/sorafs_manifest/src/bin/generate_por_fixtures.rs"
    )
    assert "fn parse_args(" in generator
    assert 'Some("--output-dir")' in generator
    assert "`--output-dir` may be specified only once" in generator
    assert "`--output-dir` requires a separate path argument" in generator
    assert "`--output-dir` path must be valid UTF-8" in generator
    assert "`--output-dir` path must not be ambiguous with an option" in generator
    assert "Component::CurDir" in generator
    assert "Component::ParentDir" in generator
    assert "`--output-dir` must name a bounded fixture directory" in generator
    assert "fn generate_fixtures(fixtures_root: &Path)" in generator
    assert "struct BoundDirectory" in generator
    assert "struct BoundWorkingDirectory" in generator
    assert "require_real_directory_ancestry" in generator
    assert "same_directory_identity" in generator
    assert "set_working_directory_handle" in generator
    assert "fchdir(directory.as_raw_fd())" in generator
    assert "output_root_binding_rejects_an_existing_non_directory" in generator
    assert "output_root_binding_rejects_an_existing_symlink" in generator
    assert "bound_output_never_follows_a_parent_substitution" in generator
    assert "output_dir_rejects_missing_duplicate_and_joined_values" in generator
    assert "output_dir_rejects_ambiguous_or_unbounded_paths" in generator


def test_pdp_fixture_generator_has_a_strict_isolated_output_root() -> None:
    """The PDP precursor cannot ambiguously redirect or alias fixture writes."""

    generator = read(
        "crates/sorafs_manifest/src/bin/generate_pdp_fixtures.rs"
    )
    assert "fn parse_args(" in generator
    assert 'Some("--output-dir")' in generator
    assert "`--output-dir` may be specified only once" in generator
    assert "`--output-dir` requires a separate path argument" in generator
    assert "`--output-dir` path must be valid UTF-8" in generator
    assert "`--output-dir` path must not be ambiguous with an option" in generator
    assert "Component::CurDir" in generator
    assert "Component::ParentDir" in generator
    assert "MAX_OUTPUT_PATH_BYTES" in generator
    assert "MAX_OUTPUT_PATH_COMPONENTS" in generator
    assert "`--output-dir` must name a bounded PDP fixture directory" in generator
    assert "struct BoundOutputDirectory" in generator
    assert "require_real_directory_ancestry" in generator
    assert "same_directory_identity" in generator
    assert "fn write_fixture_file(" in generator
    assert "must have exactly one hard link" in generator


def test_parity_fixture_snapshot_rejects_missing_inputs() -> None:
    """The SDK parity artifact collector cannot silently omit a fixture."""

    parity_gate = read("ci/sdk_sorafs_orchestrator.sh")
    assert 'sys.exit(f"[sorafs-sdk] {label} missing: {path}")' in parity_gate
    assert "warning: fixture file missing" not in parity_gate
    assert "if source is None:" not in parity_gate


def test_native_release_jobs_build_and_require_the_bridge() -> None:
    """Native release jobs build optimized, fail-closed ABI-21 bridges."""

    csharp = read(".github/workflows/pr_csharp.yml")
    mobile = read(".github/workflows/mobile_sdk_artifacts.yml")
    parity = read(".github/workflows/sorafs-orchestrator-sdk.yml")

    assert 'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"' in csharp
    assert (
        'cargo build --locked --release -p connect_norito_bridge --target "$target"'
        in csharp
    )
    assert "package_csharp_native_artifacts.py verify-package" in csharp
    assert mobile.count('IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"') == 2
    assert "name: Build NoritoBridge XCFramework" in mobile
    assert "name: Build host SoraFS reference native bridge" in mobile
    parity_runner = read("ci/sdk_sorafs_orchestrator.sh")
    assert parity_runner.count("npm run build:native") == 1
    assert parity_runner.count("IROHA_JS_NATIVE_BUILD_PROFILE=") == 1
    assert (
        "IROHA_JS_NATIVE_BUILD_PROFILE=release npm run build:native"
        in parity_runner
    )
    assert 'mktemp -d "${TMPDIR:-/tmp}/iroha-sorafs-js-native-target.XXXXXX"' in parity_runner
    assert "native_cargo=\"$(rustup which cargo)\"" in parity_runner
    assert "native_rustc=\"$(rustup which rustc)\"" in parity_runner
    assert "native_rustdoc=\"$(rustup which rustdoc)\"" in parity_runner
    for build_binding in (
        "CARGO_BUILD_JOBS=1",
        "CARGO_INCREMENTAL=0",
        "CARGO_NET_OFFLINE=true",
        'CARGO_TARGET_DIR="${native_build_target}"',
        'IROHA_JS_CARGO_LOCKFILE_PATH="${REPO_ROOT}/Cargo.lock"',
        'IROHA_JS_CARGO_PATH="${native_cargo}"',
        'RUSTC="${native_rustc}"',
        "RUSTC_BOOTSTRAP=1",
        'RUSTDOC="${native_rustdoc}"',
    ):
        assert build_binding in parity_runner
    assert "node scripts/run-test-profile.mjs sorafs-native" in parity_runner
    javascript_profile_runner = read("javascript/iroha_js/scripts/run-test-profile.mjs")
    assert '"cancelAssetLockV1.test.js"' in javascript_profile_runner
    assert '"sorafsAppealFinanceValidation.test.js"' in javascript_profile_runner
    assert '"sorafsOrchestrator.parity.test.js"' in javascript_profile_runner
    assert "swift test --filter SorafsOrchestratorParityTests" in parity_runner
    assert "swift test --filter CancelAssetLockV1Tests" in parity_runner
    assert "swift test --filter SorafsReferenceValidatorsTests" in parity_runner
    assert "name: Build exact ABI-21 NoritoBridge XCFramework" in parity
    assert "check_mobile_sdk_artifacts.sh --apple-only" in parity
    assert parity.count("IROHA_JS_NATIVE_BUILD_PROFILE:") == 1
    assert 'IROHA_JS_NATIVE_BUILD_PROFILE: "release"' in parity
    assert 'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"' in parity
    assert '"crates/iroha_js_host/**"' in parity
    assert "bash ci/sdk_sorafs_orchestrator.sh" in parity


def test_swift_native_bridge_contract_requires_universal_macos_slice() -> None:
    """Apple release lanes build and authenticate both macOS architectures."""

    mobile = read(".github/workflows/mobile_sdk_artifacts.yml")
    parity = read(".github/workflows/sorafs-orchestrator-sdk.yml")
    for workflow in (mobile, parity):
        assert (
            "rustup target add aarch64-apple-ios aarch64-apple-ios-sim "
            "x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin"
            in workflow
        )

    builder = read("scripts/build_norito_xcframework.sh")
    assert 'MACOS_ARM_TRIPLE="aarch64-apple-darwin"' in builder
    assert 'MACOS_X64_TRIPLE="x86_64-apple-darwin"' in builder
    assert (
        '"$LIPO_BINARY" -create -output "$MAC_UNI" '
        '"$LIB_MAC_ARM" "$LIB_MAC_X64"'
        in builder
    )
    assert '"macos-arm64_x86_64"' in builder

    checker = read("scripts/check_mobile_sdk_artifacts.sh")
    assert (
        "local slices=(ios-arm64 ios-arm64_x86_64-simulator "
        "macos-arm64_x86_64)"
        in checker
    )
    assert "ios-arm64_x86_64-simulator|macos-arm64_x86_64)" in checker
    assert '"macos-arm64_x86_64": {' in checker
    assert '"architectures": ["arm64", "x86_64"]' in checker
    assert (
        "NoritoBridge Info.plist does not declare the canonical universal "
        "Apple slices"
        in checker
    )

    loader = read("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
    assert 'return "macos-arm64_x86_64"' in loader
    assert '"macos-arm64_x86_64":' in loader
    assert '"macos-arm64":' not in loader


def test_python_native_lane_covers_appeal_finance_and_provider_ingest_without_skips() -> None:
    """The Python ABI-21 lane must exercise every native SoraFS V1 profile."""

    runner = read("ci/check_sorafs_python_native_sdk.sh")
    ignore_rules = read(".gitignore")
    assert 'if [[ "${PYTHON_VERSION}" != "3.12" ]]' in runner
    assert 'git -C "${ROOT_DIR}" ls-files -- \\' in runner
    for native_pattern in ("*.so", "*.so.*", "*.dylib", "*.pyd", "*.dll"):
        assert (
            f"'python/iroha_python/src/iroha_python/{native_pattern}'" in runner
        )
        assert (
            f"python/iroha_python/src/iroha_python/{native_pattern}" in ignore_rules
        )
    assert "Python native SDK artifacts must be rebuilt in the ABI-21 lane, not tracked" in runner
    assert 'export VIRTUAL_ENV="${SDK_SESSION}/venv"' in runner
    assert 'export PATH="${VIRTUAL_ENV}/bin:${PATH}"' in runner
    assert '"${VENV_PYTHON}" -m maturin develop --release --locked' in runner
    assert (
        '"${VENV_PYTHON}" -I "${ROOT_DIR}/scripts/check_native_sdk_abi21_artifact.py"'
        in runner
    )
    assert "tests/cancel_asset_lock_v1_test.py" in runner
    assert "tests/cancel_asset_lock_client_helpers_test.py" in runner
    assert "tests/client_ledger_helpers_test.py" in runner
    assert "tests/sorafs_reference_validation_test.py" in runner
    assert "tests/sorafs_replication_instruction_test.py" in runner
    assert '--junitxml "${JUNIT_REPORT}"' in runner
    assert 'skipped = sum(int(suite.attrib.get("skipped", "0")) for suite in suites)' in runner
    assert "SoraFS native Python SDK parity may not contain skipped tests" in runner


def test_python_cancel_builder_has_exact_archive_and_typed_two_argument_coverage() -> None:
    """The native Python lane must decode and pin the hard-cut cancellation archive."""

    tests = read("python/iroha_python/tests/cancel_asset_lock_client_helpers_test.py")
    crypto = read("python/iroha_python/src/iroha_python/crypto.py")
    assert "instruction_json_bytes = draft.instructions[0].to_json().encode(\"utf-8\")" in tests
    assert "instruction_archive = base64.b64decode(" in tests
    assert "json.loads(instruction_json_bytes)" in tests
    assert "validate=True" in tests
    assert "cancel_asset_lock_archive = instruction_archive[-85:]" in tests
    assert "decode_cancel_asset_lock_v1(cancel_asset_lock_archive)" in tests
    assert "assert instruction_json_bytes == (" in tests
    assert "assert instruction_archive == bytes.fromhex(" in tests
    assert "assert len(cancel_asset_lock_archive) == 85" in tests
    assert "decoded_cancel_asset_lock.escrow_id ==" in tests
    assert "decoded_cancel_asset_lock.expected_remaining_amount == \"10\"" in tests
    assert "in draft.instructions[0].to_json()" not in tests
    typed_builder = (
        "def cancel_asset_lock(\n"
        "            escrow_id: str,\n"
        "            expected_remaining_amount: str,\n"
        "        ) -> Instruction:"
    )
    assert typed_builder in crypto
