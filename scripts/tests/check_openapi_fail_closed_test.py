"""Static guards for fail-closed OpenAPI release generation."""

from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 and older
    import tomli as tomllib


REPO_ROOT = Path(__file__).resolve().parents[2]
XTASK = REPO_ROOT / "xtask" / "src" / "main.rs"
TORII_OPENAPI = REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs"
OPENAPI_GATE = REPO_ROOT / "ci" / "check_openapi_spec.sh"
GENERATED_FILES = REPO_ROOT / "generated-files.toml"
OPENAPI_LOCK_PROVISIONER = (
    REPO_ROOT / "tools" / "openapi" / "scripts" / "provision-openapi-cargo-lock.mjs"
)
OPENAPI_LOCK_PIN = REPO_ROOT / "release" / "openapi-cargo-lock-v1.txt"
OPENAPI_SCRIPTS = (
    REPO_ROOT / "tools" / "openapi" / "scripts" / "sync-openapi.mjs",
    REPO_ROOT / "tools" / "openapi" / "scripts" / "verify-openapi-versions.mjs",
    REPO_ROOT
    / "tools"
    / "openapi"
    / "scripts"
    / "verify-openapi-release-inputs.mjs",
    REPO_ROOT / "tools" / "openapi" / "scripts" / "check-openapi-signatures.mjs",
)


def test_openapi_generator_has_no_stub_fallback() -> None:
    xtask = XTASK.read_text(encoding="utf-8")
    torii_openapi = TORII_OPENAPI.read_text(encoding="utf-8")

    assert "allow_stub" not in xtask
    assert "build_stub_spec" not in xtask
    assert "pub fn stub_spec" not in torii_openapi
    # The sole occurrence is the negative parser test proving the deleted flag
    # remains rejected. Any production/help occurrence is a regression.
    assert xtask.count('"--allow-stub"') == 1
    assert 'let args = ["xtask", "openapi", "--allow-stub"];' in xtask
    assert "require_release_router_openapi(try_generate_router_openapi())?" in xtask


def test_every_openapi_manifest_boundary_validates_release_shape() -> None:
    xtask = XTASK.read_text(encoding="utf-8")

    for function_name in (
        "write_openapi_manifest",
        "write_openapi_manifest_with_signature",
        "write_openapi_manifest_unsigned",
        "write_openapi_manifest_from_bytes",
        "verify_openapi_manifest",
    ):
        start = xtask.index(f"fn {function_name}(")
        body_start = xtask.index("{", start)
        next_function = xtask.find("\nfn ", body_start)
        body = xtask[body_start : None if next_function < 0 else next_function]
        assert "validate_release_openapi_bytes" in body, function_name


def test_openapi_version_and_signature_paths_reject_empty_specs() -> None:
    for path in OPENAPI_SCRIPTS:
        source = path.read_text(encoding="utf-8")
        assert "validateReleaseOpenApiDocumentBytes" in source, path


def test_openapi_generated_owner_has_exact_outputs_and_staging_interfaces() -> None:
    registry = tomllib.loads(GENERATED_FILES.read_text(encoding="utf-8"))
    owners = [
        entry
        for entry in registry["generated"]
        if entry["name"] == "torii-openapi-release-bundle"
    ]
    assert len(owners) == 1
    owner = owners[0]
    assert owner["outputs"] == [
        "artifacts/openapi/torii.json",
        "artifacts/openapi/manifest.json",
        "artifacts/openapi/versions/current/torii.json",
        "artifacts/openapi/versions/current/manifest.json",
        "artifacts/openapi/versions.json",
    ]
    assert "artifacts/openapi/allowed_signers.json" in owner["inputs"]
    assert "artifacts/openapi/allowed_signers.json" not in owner["outputs"]
    assert "openapi --output-root artifacts/openapi" in owner["generator"]
    assert "--output-dir=artifacts/openapi" in owner["generator"]
    assert "--reuse-canonical-spec" not in owner["generator"]
    assert "--locked --offline --jobs 1 -Z unstable-options" in owner["generator"]
    assert "--lockfile-path Cargo.lock" in owner["generator"]
    assert owner["check"] == "bash ci/check_openapi_spec.sh"

    sync_source = OPENAPI_SCRIPTS[0].read_text(encoding="utf-8")
    assert "node:child_process" not in sync_source
    assert "runCargo" not in sync_source
    assert "readOpenApiStableFile(canonicalSpec" in sync_source


def test_openapi_cargo_lock_pin_has_one_staging_only_owner() -> None:
    registry = tomllib.loads(GENERATED_FILES.read_text(encoding="utf-8"))
    owners = [
        entry
        for entry in registry["generated"]
        if entry["name"] == "openapi-cargo-lock-pin"
    ]
    assert len(owners) == 1
    owner = owners[0]
    assert owner["outputs"] == ["release/openapi-cargo-lock-v1.txt"]
    assert owner["inputs"] == ["release/openapi-generator-inputs-v1.txt"]
    assert "provision-openapi-cargo-lock.mjs pin" in owner["generator"]
    assert "--source=\"$PWD/Cargo.lock\"" in owner["generator"]
    assert "IROHA_OPENAPI_CARGO_LOCK_PIN_STAGE" in owner["generator"]
    assert "--check=\"$PWD/release/openapi-cargo-lock-v1.txt\"" in owner["check"]

    xtask = XTASK.read_text(encoding="utf-8")
    provisioner = OPENAPI_LOCK_PROVISIONER.read_text(encoding="utf-8")
    pin_fields = dict(
        line.split("=", 1)
        for line in OPENAPI_LOCK_PIN.read_text(encoding="utf-8").splitlines()[1:]
    )
    pinned_size = pin_fields["bytes"]
    pinned_digest = pin_fields["sha256_hex"]
    for source in (xtask, provisioner):
        assert pinned_size not in source.replace("_", "")
        assert pinned_digest not in source
    assert "outside the repository" in provisioner
    assert "assertOpenApiCargoLockSnapshotStable" in provisioner


def test_release_gate_is_clean_pinned_and_replays_complete_bundles_independently() -> None:
    gate = OPENAPI_GATE.read_text(encoding="utf-8")

    assert gate.count(
        "node tools/openapi/scripts/verify-musubi-v1-contract.mjs"
    ) == 1
    assert gate.index(
        "node tools/openapi/scripts/verify-musubi-v1-contract.mjs"
    ) < gate.index("require_clean_checkout\n")
    assert "require_clean_checkout" in gate
    assert "EXPECTED_GENERATOR_COMMIT" not in gate
    assert gate.count(
        "node tools/openapi/scripts/verify-openapi-release-inputs.mjs"
    ) == 2
    assert gate.count(
        "python3 scripts/check_sorafs_release_version_map.py"
    ) == 2
    assert gate.count(
        'build_unsigned_replay_bundle "${REPLAY_WORKTREE_FIRST}" '
        '"${REPLAY_BUNDLE_FIRST}"'
    ) == 1
    assert gate.count(
        'build_unsigned_replay_bundle "${REPLAY_WORKTREE_SECOND}" '
        '"${REPLAY_BUNDLE_SECOND}"'
    ) == 1
    assert 'create_replay_worktree "${REPLAY_WORKTREE_FIRST}"' in gate
    assert 'create_replay_worktree "${REPLAY_WORKTREE_SECOND}"' in gate
    assert (
        'REPLAY_COMMIT="$(git -C "${REPO_ROOT}" rev-parse --verify '
        '"HEAD^{commit}")"'
    ) in gate
    assert (
        'worktree add --quiet --detach "${worktree}" "${REPLAY_COMMIT}"'
        in gate
    )
    assert "const sourcePath = await realpath(sourceArgument);" in gate
    assert "provisionOpenApiCargoLock," in gate
    assert "const summary = await provisionOpenApiCargoLock({" in gate
    assert "repoRoot: worktreeRoot," in gate
    assert "summary.status !== 'installed'" in gate
    assert "summary.source !== 'operator'" in gate
    assert 'summary.path !== \'Cargo.lock\'' in gate
    assert '"${REPO_ROOT}/Cargo.lock"' in gate
    assert 'cp "${REPO_ROOT}/Cargo.lock"' not in gate
    assert 'REPLAY_CARGO_TARGET_DIR="${TMP_DIR}/cargo-target"' in gate
    assert 'REPLAY_CARGO_TARGET_DIR="${REPO_ROOT}' not in gate
    assert 'CARGO_TARGET_DIR="${REPLAY_CARGO_TARGET_DIR}"' in gate
    assert 'allowedSignersFile: join(outputDir, \'allowed_signers.json\')' not in gate
    assert 'allowedSignersFile,' in gate
    assert '"${ALLOWED_SIGNERS_PATH}"' in gate
    assert 'cp -R "${REPLAY_BASELINE}/." "${output_dir}/"' in gate
    assert (
        'run_xtask_in_repo "${source_root}" openapi --unsigned-manifest'
        in gate
    )
    assert "const {syncOpenApi} = await import(syncModule)" in gate
    assert "requireSigned: false" in gate
    assert "is not clean and unsigned" in gate
    artifact_block = gate.split("GENERATED_RELEASE_ARTIFACTS=(\n", 1)[1].split(
        "\n)", 1
    )[0]
    assert [
        line.strip().removeprefix('"').removesuffix('"')
        for line in artifact_block.splitlines()
        if line.strip()
    ] == [
        "torii.json",
        "manifest.json",
        "versions/current/torii.json",
        "versions/current/manifest.json",
        "versions.json",
    ]
    assert 'diff -u "${first}" "${second}"' in gate
    assert (
        'diff -ru "${REPLAY_BUNDLE_FIRST}" "${REPLAY_BUNDLE_SECOND}"'
        in gate
    )
    assert 'diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}"' in gate
    assert (
        'diff -u "${RELEASE_INPUT_SUMMARY_FIRST}" '
        '"${RELEASE_INPUT_SUMMARY_SECOND}"'
        in gate
    )
    assert (
        'diff -u "${VERSION_MAP_SUMMARY_FIRST}" '
        '"${VERSION_MAP_SUMMARY_SECOND}"'
        in gate
    )
    assert "VERSION_VERIFY_POLICY_ARGS" not in gate
