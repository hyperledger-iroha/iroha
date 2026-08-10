from __future__ import annotations

import plistlib
import subprocess
from pathlib import Path

import pytest

from scripts import finalize_taira_rollout_authority as authority_finalizer

try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[2]
TAIRA_DIR = ROOT / "configs" / "soranexus" / "taira"
AUTHORITY_FINALIZER = ROOT / "scripts" / "finalize_taira_rollout_authority.py"


def test_taira_packages_exact_native_software_signers_and_separates_promotion() -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(
        encoding="utf-8"
    )
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    signer_root = ROOT / "configs" / "sorafs" / "external_software_signer"
    docs = (signer_root / "README.md").read_text(encoding="utf-8")

    for source in (builder, workflow):
        assert "sorafs_external_software_signer" in source
        assert "libexec/iroha-runtime-provider-broker-v1" in source
        assert 'cmp "$stage/bin/sorafs_external_software_signer"' in source or (
            'cmp "${bundle_dir}/bin/${SOFTWARE_SIGNER_BIN}"' in source
        )
        assert "sorafs_external_software_signer\" --help" in source or (
            '${SOFTWARE_SIGNER_BIN}" --help' in source
        )
    assert '"backend": "software"' in builder
    assert '"promotion_requires_separate_l2_host": True' in builder
    assert "Promotion uses the same supported binary and receipt protocol" in docs
    assert "separately administered L2 promotion host" in docs

    launchd = signer_root / "launchd"
    native_roles = {"proof-outcome", "repair", "reserve", "orderbook"}
    typed_roles = {
        "governance-dag",
        "potr-gateway",
        "potr-provider",
        "billing",
        "evidence-viewer",
        "stream-token",
        "pop-credentials",
    }
    plists = sorted(launchd.glob("org.hyperledger.iroha.sorafs-signer-*.plist"))
    discovered_roles = {
        path.stem.removeprefix("org.hyperledger.iroha.sorafs-signer-")
        for path in plists
    }
    assert discovered_roles == native_roles | typed_roles
    launcher = (launchd / "sorafs-external-software-signer-launchd-v1").read_text(
        encoding="utf-8"
    )
    assert (
        "proof-outcome|repair|reserve|orderbook|governance-dag|potr-gateway|"
        "potr-provider|billing|evidence-viewer|stream-token|pop-credentials"
        in launcher
    )
    assert "promotion" not in launcher
    assert "exec 3<&0" in launcher
    assert "--wrapping-key-fd 3" in launcher
    for path in plists:
        role = path.stem.removeprefix("org.hyperledger.iroha.sorafs-signer-")
        value = plistlib.loads(path.read_bytes())
        assert value["ProgramArguments"] == [
            "/usr/local/libexec/sorafs-external-software-signer-launchd-v1",
            role,
        ]
        assert value["StandardInPath"] == (
            f"/private/var/run/iroha-signer-credentials/{role}.wrapping-key"
        )
        assert value["SoftResourceLimits"]["Core"] == 0
        assert value["HardResourceLimits"]["Core"] == 0
        assert value["UserName"] == value["GroupName"] == f"sorafs-signer-{role}"

    assert all(f'                    "{role}",' in builder for role in native_roles)
    assert all(f'                    "{role}",' not in builder for role in typed_roles)
    assert "none of the seven typed roles is auto-launched on a validator" in docs
    assert "ExternalSoftwareSignerPopRegistryV1" in docs


def test_signer_release_policy_excludes_windows_without_false_smoke() -> None:
    policy = (
        ROOT
        / "configs/sorafs/external_software_signer/WINDOWS-UNSUPPORTED.md"
    ).read_text(encoding="utf-8")
    release_bundle = (ROOT / "scripts/build_release_bundle.sh").read_text(
        encoding="utf-8"
    )
    cli_workflow = (
        ROOT / ".github/workflows/sorafs-cli-release.yml"
    ).read_text(encoding="utf-8")
    candidate = (ROOT / "scripts/package_sorafs_cli_candidate.py").read_text(
        encoding="utf-8"
    )
    assert "unsupported on Windows" in policy
    assert 'signer_support="unsupported-windows"' in release_bundle
    assert 'if [[ "$os_tag" != "win" ]]' in release_bundle
    assert 'if [[ "$target" == *-windows-* ]]' in cli_workflow
    assert "WINDOWS-UNSUPPORTED-EXTERNAL-SOFTWARE-SIGNER.md" in cli_workflow
    assert 'if not target.endswith("windows-msvc")' in candidate
    assert '"clean_smoke_binary_count": 3' in candidate


def test_taira_release_freezes_fail_closed_network_time_policy() -> None:
    config = tomllib.loads((TAIRA_DIR / "config.toml").read_text(encoding="utf-8"))
    assert config["nts"] == {
        "sample_interval_ms": 5_000,
        "sample_cap_per_round": 8,
        "max_rtt_ms": 500,
        "trim_percent": 10,
        "per_peer_buffer": 16,
        "smoothing_enabled": False,
        "smoothing_alpha": 0.2,
        "max_adjust_ms_per_min": 50,
        "min_samples": 3,
        "max_offset_ms": 1_000,
        "max_confidence_ms": 500,
        "enforcement_mode": "reject",
    }

    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    assert (
        "canonical Taira config must contain the explicit [nts] release policy"
        in builder
    )
    assert (
        "canonical Taira [nts] release policy has missing or unknown fields" in builder
    )
    assert '"enforcement_mode": "reject"' in builder


def test_verify_soraswap_rollout_passes_expected_git_sha_to_mcp_check() -> None:
    source = (TAIRA_DIR / "verify_soraswap_rollout.sh").read_text(encoding="utf-8")

    assert 'EXPECTED_TAIRA_GIT_SHA="${EXPECTED_TAIRA_GIT_SHA:-}"' in source
    assert "--expected-git-sha)" in source
    assert 'mcp_cmd+=(--expected-git-sha "$EXPECTED_TAIRA_GIT_SHA")' in source
    assert "--validator-root)" in source
    assert 'mcp_cmd+=(--validator-root "$validator_root_spec")' in source
    assert "--offline-asset-definition-id" not in source
    assert "--offline-expected-identity" not in source
    assert (
        "public SoraSwap mutation/release paths cannot skip the "
        "Taira build/finality/dataspace fleet gate" in source
    )


def test_rollout_bundle_manifest_followup_pins_mcp_and_soraswap_checks() -> None:
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")

    assert (
        "check_mcp_rollout.sh --public-root https://<public-torii-root> "
        "--validator-root <label>=<validator-url> (once per validator) "
        "--require-all-validators --write-config "
        "/run/secrets/taira-canary-client.toml --expected-git-sha " in source
    )
    assert (
        "verify_soraswap_rollout.sh --public-root https://<public-torii-root> "
        "--validator-root <label>=<validator-url> (once per validator) "
        "--expected-git-sha " in source
    )
    assert "--offline-asset-definition-id" not in source
    assert "--offline-expected-identity" not in source
    assert '+ os.environ["GIT_HEAD"]' in source


def test_rollout_bundle_has_no_backend_offline_proof_prerequisite() -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")

    for obsolete in (
        "KAGEMUSHA_RELEASE_POLICY",
        "KAGEMUSHA_ARTIFACT_ROOT",
        "--kagemusha-release-policy",
        "--kagemusha-artifact-root",
        "check_kagemusha_production_readiness.sh",
        '"kagemusha/release-policy.norito"',
        '"kagemusha/v4/"',
    ):
        assert obsolete not in builder
        assert obsolete not in workflow
    assert "prepare_taira_offline_reset_bundle.py" not in workflow
    assert "prepare-reset" in workflow
    assert "TAIRA_MACOS_OFFLINE_GENESIS_PATH" not in workflow
    assert "TAIRA_MACOS_KAGEMUSHA_RELEASE_BUNDLE_PATH" not in workflow


def test_taira_validator_release_uses_post_build_feature_isolated_native_evidence() -> (
    None
):
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")

    assert 'IROHAD_RELEASE_FEATURES="embedded-soracloud-runtime,zk-stark"' in source
    assert 'PRIVACY_RELEASE_EVIDENCE_FEATURE="privacy-release-evidence"' in source
    assert 'PRIVACY_RELEASE_RUNNER_PACKAGE="iroha_test_network"' in source
    assert 'PRIVACY_RELEASE_RUNNER_BIN="taira_privacy_release_runner"' in source
    assert '--features "$IROHAD_RELEASE_FEATURES" -i iroha_core' in source
    assert 'irohad feature "zk-stark"' in source
    assert 'iroha_core feature "zk-stark"' in source
    assert (
        'if [[ "$irohad_core_feature_graph" == '
        '*"$PRIVACY_RELEASE_EVIDENCE_FEATURE"* ]]' in source
    )
    assert (
        "$PRIVACY_RELEASE_RUNNER_PACKAGE feature "
        '\\"$PRIVACY_RELEASE_EVIDENCE_FEATURE\\"' in source
    )
    assert 'iroha_core feature \\"$PRIVACY_RELEASE_EVIDENCE_FEATURE\\"' in source
    assert 'cargo "${core_build_args[@]}"' in source
    assert 'cargo "${privacy_runner_build_args[@]}"' in source
    assert "privacy_runner_build_args=(\n    rustc\n    --locked" in source
    assert "privacy_runner_build_args+=(-- -C target-feature=+crt-static)" in source
    assert 'if [[ "$(uname -s)" != "Linux" ]]' in source
    assert 'case "$(uname -m)" in' in source
    assert "aarch64)" in source
    assert "x86_64|aarch64)" not in source
    assert "Taira first-release archive requires native Linux aarch64" in source
    assert "readelf --program-headers --wide" in source
    assert "must not contain a PT_INTERP segment" in source
    assert "readelf --dynamic --wide" in source
    assert "must not contain DT_NEEDED entries" in source
    assert '"$privacy_runner_path" generate' in source
    assert source.index('cargo "${core_build_args[@]}"') < source.index(
        'cargo "${privacy_runner_build_args[@]}"'
    )
    assert source.index('cargo "${privacy_runner_build_args[@]}"') < source.index(
        "readelf --program-headers --wide"
    )
    assert source.index("readelf --program-headers --wide") < source.index(
        "readelf --dynamic --wide"
    )
    assert source.index("readelf --dynamic --wide") < source.index(
        '"$privacy_runner_path" generate'
    )
    assert "taira_privacy_prebundle_gate" not in source
    assert "PRIVACY_PREBUNDLE_REPORT" not in source
    assert '"privacy_release":' not in source

    assert "build_taira_rollout_bundle.sh" in workflow
    assert "--features embedded-soracloud-runtime,zk-stark" in workflow
    assert "capture-four-peer" in workflow
    assert "docker/build-push-action@" not in workflow
    assert 'test "${FEATURES}" = "embedded-soracloud-runtime,zk-stark"' in dockerfile
    assert "Taira irohad must not contain privacy-release-evidence" in dockerfile
    assert (
        "Taira irohad must not contain deterministic privacy test fixtures"
        in dockerfile
    )
    assert (
        """case "${runner_privacy_features}" in """
        """*'iroha_test_network feature "privacy-release-evidence"'*)""" in dockerfile
    )
    assert (
        "-p iroha_test_network --bin taira_privacy_release_runner "
        "--features privacy-release-evidence" in dockerfile
    )
    assert (
        """case "${runner_fixture_features}" in """
        """*'iroha_data_model feature "test-fixtures"'*)""" in dockerfile
    )
    assert "Taira privacy runner omits compiled exact12 semantics" in dockerfile
    assert "/outbin/taira_privacy_release_runner generate" in dockerfile
    assert "/outbin/taira_privacy_release_runner verify" in dockerfile
    assert "/usr/local/bin/taira_privacy_release_runner verify" in dockerfile
    assert "taira_privacy_prebundle_gate" not in dockerfile
    assert "/outprovenance/privacy-release.json" not in dockerfile


def test_rollout_bundle_persists_typed_native_evidence_pairs_and_bundled_verification() -> (
    None
):
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    readme = (TAIRA_DIR / "README.md").read_text(encoding="utf-8")

    for stem in ("receipt", "stage-artifacts", "command-manifest"):
        assert f"{stem}-v1.norito" in source
        assert f"{stem}-v1.json" in source
    assert "native_release_expectations_v1.norito" in source
    assert "native_release_expectations_v1.json" in source
    assert "zk_x509_native_resource_v1.norito" in source
    assert "zk_x509_native_resource_v1.json" in source
    assert "zk-x509-resource-v1.norito" in source
    assert "zk-x509-resource-v1.json" in source
    for flag in (
        "--command-manifest-norito-out",
        "--command-manifest-json-out",
        "--stage-artifacts-norito-out",
        "--stage-artifacts-json-out",
        "--receipt-norito-out",
        "--receipt-json-out",
        "--command-manifest-norito",
        "--command-manifest-json",
        "--stage-artifacts-norito",
        "--stage-artifacts-json",
        "--receipt-norito",
        "--receipt-json",
        "--expectations-norito",
        "--expectations-json",
        "--x509-resource-norito",
        "--x509-resource-json",
    ):
        assert flag in source
    assert '"authoritative_encoding": "norito"' in source
    assert '"encoding": "norito"' in source
    assert '"deterministic_json_projection": {' in source
    assert '"typed_equal_to_norito": True' in source
    assert '"fixed_stage_block_count": 48' in source
    assert '"contains_witnesses": False' in source
    assert '"contains_canonical_proof_artifacts": True' in source
    assert "contains_witnesses_or_raw_proofs" not in source
    assert '"peak_rss_and_elapsed_ceilings_enforced": True' in source
    assert '"x509_native_resource_certificate": evidence_pair(' in source
    assert '"phase": "post_build"' in source
    assert '"bundled_verify_passed": True' in source
    assert '"algorithm": "sha256"' in source
    assert '"digest": os.environ["WORKSPACE_SOURCE_MANIFEST_SHA256"]' in source
    assert '"digest_file": {' in source
    assert '"PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256"' in source
    assert '"binary_identities": {' in source
    assert '"VALIDATOR_BINARY_SHA256"' in source
    assert '"PRIVACY_RUNNER_BINARY_SHA256"' in source
    assert '"validator_and_evidence_runner_bound_by_typed_receipt": True' in source
    assert '"broker_public_export_bound_by_plan_and_release_manifest": (' in source
    assert '"${bundle_dir}/bin/${PRIVACY_RELEASE_RUNNER_BIN}" verify' in source
    included_paths = source.partition('"included_paths": [')[2].partition(
        '"required_followup":'
    )[0]
    assert "f'bin/{os.environ[\"PRIVACY_RELEASE_RUNNER_BIN\"]}'" in included_paths
    assert 'os.environ["PRIVACY_NATIVE_RELATIVE_DIR"] + "/"' in included_paths
    assert (
        "--x509-resource-norito "
        '"${bundle}/provenance/privacy-native/zk-x509-resource-v1.norito"' in readme
    )
    assert (
        "--x509-resource-json "
        '"${bundle}/provenance/privacy-native/zk-x509-resource-v1.json"' in readme
    )


def _assert_native_privacy_composer_bundle_contract(
    builder: str, workflow: str
) -> None:
    assert (
        'TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR="${TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR:-}"'
        in builder
    )
    assert (
        "PUBLIC_INPUT_DIR: ${{ vars.TAIRA_PRIVACY_RELEASE_INPUT_DIR }}"
        in workflow
    )
    assert (
        '"TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR=$privacy_input"'
        in workflow
    )
    assert "Snapshot only the four public privacy inputs with the installed controller" in workflow
    for name in (
        "privacy_bootstrap_plan.json",
        "config.toml",
        "genesis.json",
        "bootle_lantern_broker_public.json",
    ):
        assert name in builder
    assert "privacy release input directory must contain exactly" in builder
    assert builder.count("flags |= os.O_NOFOLLOW") == 2
    assert "before.st_nlink != 1" in builder
    assert "privacy release input changed while opening" in builder
    assert "privacy release input changed while reading" in builder
    assert "privacy release input directory changed during snapshot" in builder
    assert (
        'kagami_build_args=(build --locked -p iroha_kagami --bin "$KAGAMI_BIN")'
        in builder
    )
    emit = builder.index('"$kagami_path" privacy-bootstrap emit-taira-v1')
    compose = builder.index('"$kagami_path" privacy-bootstrap render-taira-release-v1')
    compare = builder.index("compare_composed_privacy_input()")
    bundle_copy = builder.index(
        'cp "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC" \\\n    "${bundle_dir}/${privacy_bootstrap_broker_public_relative_path}"'
    )
    assert emit < compose < compare < bundle_copy
    for label in ("plan", "config", "genesis", "broker public export"):
        assert f'compare_composed_privacy_input \\\n    "{label}"' in builder
    assert builder.count('--broker-public "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC"') == 2
    assert (
        '--broker-public "${bundle_dir}/${privacy_bootstrap_broker_public_relative_path}"'
        in builder
    )
    assert '"privacy_bootstrap_release": privacy_bootstrap_release' in builder
    assert '"bound_by_plan_sha256": True' in builder
    assert '"native_recomposition_passed": True' in builder
    assert '"bundled_release_validation_passed": True' in builder
    assert 'os.environ["PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256"]' in builder


def test_release_bundle_recomposes_and_binds_exact_reviewed_privacy_inputs() -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    _assert_native_privacy_composer_bundle_contract(builder, workflow)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            "flags |= os.O_NOFOLLOW", "# no-follow removed", 1
        ),
        lambda source: source.replace("before.st_nlink != 1", "before.st_nlink < 0", 1),
        lambda source: source.replace(
            'compare_composed_privacy_input \\\n    "broker public export"',
            "# broker comparison removed",
            1,
        ),
        lambda source: source.replace(
            '--broker-public "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC"',
            "# broker validation removed",
            1,
        ),
        lambda source: source.replace(
            '"bound_by_plan_sha256": True',
            '"bound_by_plan_sha256": False',
            1,
        ),
    ),
    ids=(
        "symlink-following",
        "hardlink-accepted",
        "broker-byte-compare-removed",
        "broker-validation-removed",
        "plan-binding-removed",
    ),
)
def test_privacy_composer_bundle_contract_rejects_adversarial_mutations(
    mutation,
) -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    _assert_native_privacy_composer_bundle_contract(builder, workflow)
    with pytest.raises(AssertionError):
        _assert_native_privacy_composer_bundle_contract(mutation(builder), workflow)


def test_native_evidence_source_manifest_is_rechecked_across_release_boundaries() -> (
    None
):
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")

    assert (
        'python3 -I -S "$WORKSPACE_SOURCE_MANIFEST_SCRIPT" '
        '--root "$REPO_ROOT"' in source
    )
    initial = source.index(
        'workspace_source_manifest_sha256="$(compute_workspace_source_manifest)"'
    )
    post_build = source.index('assert_workspace_source_manifest_unchanged "post-build"')
    generate = source.index('"$privacy_runner_path" generate')
    post_evidence = source.index(
        'assert_workspace_source_manifest_unchanged "post-evidence"'
    )
    pre_archive = source.index(
        'assert_workspace_source_manifest_unchanged "pre-archive"'
    )
    archive = source.index('tar -C "$OUTPUT_DIR" -czf "$archive_path"')
    post_archive = source.index(
        'assert_workspace_source_manifest_unchanged "post-archive"'
    )
    assert initial < post_build < generate < post_evidence < pre_archive
    assert pre_archive < archive < post_archive

    assert 'workspace_source_manifest_before="$(python3 -I -S' in dockerfile
    assert "workspace_source_manifest_sha256" in workflow
    assert workflow.count("prepare_taira_release_source.sh") == 2
    assert workflow.count('cmp "$linux_stage/taira-source-identity-v1.json"') >= 1
    assert "staged_root" in workflow
    assert "inspect-handoff" in workflow
    assert "--extract-sealed-archive" in dockerfile
    assert dockerfile.count("--require-exact-closure") == 3


def test_taira_publish_workflow_has_no_competing_image_authority() -> None:
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    for retired in (
        "docker/build-push-action@",
        "docker/login-action@",
        "docker push",
        "docker manifest",
        "setup-buildx-action@",
        "push_latest",
        "taira-latest",
    ):
        assert retired not in workflow.lower()
    assert "Publish through the sealed installed authority controller" in workflow
    assert "publish-rollout --" in workflow
    assert "sign-manifest" not in workflow
    assert "oras push" not in workflow
    assert "TAIRA_OCI_PASSWORD" not in workflow


def _assert_docker_sealed_source_and_final_verify_contract(
    dockerfile: str,
) -> None:
    assert "COPY . /build-context/" in dockerfile
    assert 'if [ "${CONFIG_PROFILE}" = "taira" ]; then' in dockerfile
    assert "--validate-sealed-context /build-context" in dockerfile
    assert "sha256sum /build-context/taira-workspace-source-v1.seal" in dockerfile
    assert (
        "--extract-sealed-archive /build-context/taira-workspace-source-v1.seal"
        in dockerfile
    )
    assert "cmp /build-context/Dockerfile /app/Dockerfile" in dockerfile
    assert (
        "cmp /build-context/scripts/compute_workspace_source_manifest.py "
        "/app/scripts/compute_workspace_source_manifest.py" in dockerfile
    )
    assert (
        "cmp /build-context/scripts/taira_image_smoke.sh "
        "/app/scripts/taira_image_smoke.sh" in dockerfile
    )
    assert "else \\\n        mkdir -p /app;" in dockerfile
    assert "cp -a /build-context/. /app/" in dockerfile
    assert "--mount=type=cache,target=/cargo-target" in dockerfile
    assert "export CARGO_TARGET_DIR=/cargo-target" in dockerfile
    assert "effective_cargo_build_jobs" not in dockerfile
    assert dockerfile.count('CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}"') == 3
    assert dockerfile.count("--require-exact-closure") == 3
    first_exact = dockerfile.index("--require-exact-closure")
    first_cargo = dockerfile.index("cargo ${CARGOFLAGS} build")
    final_exact = dockerfile.rindex("--require-exact-closure")
    evidence = dockerfile.index("/outbin/taira_privacy_release_runner verify")
    assert first_exact < first_cargo < evidence < final_exact
    for source_name in (
        "zk_x509_native_resource_v1.norito",
        "zk_x509_native_resource_v1.json",
    ):
        assert (
            f"test -s /app/fixtures/privacy/{source_name} && "
            f"test ! -L /app/fixtures/privacy/{source_name} && "
            f"test \"$(stat -c '%h' /app/fixtures/privacy/{source_name})\" = 1"
            in dockerfile
        )
    for installed_name in (
        "zk-x509-resource-v1.norito",
        "zk-x509-resource-v1.json",
    ):
        assert f"/outprovenance/privacy-native/{installed_name}" in dockerfile
        assert f"/opt/iroha/provenance/privacy-native/{installed_name}" in dockerfile
    assert (
        "cp /app/fixtures/privacy/zk_x509_native_resource_v1.norito "
        "/outprovenance/privacy-native/zk-x509-resource-v1.norito" in dockerfile
    )
    assert (
        "cp /app/fixtures/privacy/zk_x509_native_resource_v1.json "
        "/outprovenance/privacy-native/zk-x509-resource-v1.json" in dockerfile
    )
    assert dockerfile.count("--x509-resource-norito") == 3
    assert dockerfile.count("--x509-resource-json") == 3
    evidence_hash_block = dockerfile[
        dockerfile.index(
            "(cd /outprovenance/privacy-native && find . -type f"
        ) : dockerfile.index('workspace_source_manifest_after="')
    ]
    assert "sha256sum > sha256sums.txt" in evidence_hash_block

    final_verify_marker = "/usr/local/bin/taira_privacy_release_runner verify"
    assert final_verify_marker in dockerfile
    final_verify = dockerfile.index(final_verify_marker)
    final_user = dockerfile.index("USER ${UID}:${GID}")
    final_entrypoint = dockerfile.index('ENTRYPOINT ["docker_entrypoint.sh"]')
    assert final_verify < final_user < final_entrypoint
    final_block = dockerfile[final_verify:final_user]
    assert "--validator-binary /usr/local/bin/irohad" in final_block
    assert "--cargo-lock /opt/iroha/provenance/Cargo.lock" in final_block
    for stem in ("command-manifest", "stage-artifacts", "receipt"):
        assert f"/opt/iroha/provenance/privacy-native/{stem}-v1.norito" in final_block
        assert f"/opt/iroha/provenance/privacy-native/{stem}-v1.json" in final_block
    assert (
        "--x509-resource-norito "
        "/opt/iroha/provenance/privacy-native/zk-x509-resource-v1.norito" in final_block
    )
    assert (
        "--x509-resource-json "
        "/opt/iroha/provenance/privacy-native/zk-x509-resource-v1.json" in final_block
    )


def test_docker_recomputes_sealed_source_and_verifies_final_runtime_paths() -> None:
    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")
    _assert_docker_sealed_source_and_final_verify_contract(dockerfile)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            "/usr/local/bin/taira_privacy_release_runner verify",
            "# final runtime verification deleted",
            1,
        ),
        lambda source: source.replace(
            "--validator-binary /usr/local/bin/irohad",
            "--validator-binary /outbin/irohad",
            1,
        ),
        lambda source: source.replace(
            "--cargo-lock /opt/iroha/provenance/Cargo.lock",
            "--cargo-lock /outprovenance/Cargo.lock",
            1,
        ),
        lambda source: source.replace(
            "--x509-resource-json "
            "/opt/iroha/provenance/privacy-native/zk-x509-resource-v1.json",
            "# final X.509 resource JSON omitted",
            1,
        ),
        lambda source: source.replace(
            "cp /app/fixtures/privacy/zk_x509_native_resource_v1.norito "
            "/outprovenance/privacy-native/zk-x509-resource-v1.norito",
            "# X.509 resource Norito copy omitted",
            1,
        ),
        lambda source: source.replace(
            'CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS}"',
            'CARGO_BUILD_JOBS="2"',
            1,
        ),
    ),
    ids=(
        "final-verify-deleted",
        "builder-validator-path",
        "builder-evidence-path",
        "resource-json-omitted",
        "resource-norito-copy-omitted",
        "cargo-jobs-overridden",
    ),
)
def test_docker_release_contract_rejects_deleted_overridden_or_builder_path_mutations(
    mutation,
) -> None:
    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")
    _assert_docker_sealed_source_and_final_verify_contract(dockerfile)
    with pytest.raises(AssertionError):
        _assert_docker_sealed_source_and_final_verify_contract(mutation(dockerfile))


def test_taira_archive_publication_contract_is_documented() -> None:
    readme = (TAIRA_DIR / "README.md").read_text(encoding="utf-8")

    assert "## Dual-target archive publication" in readme
    assert "Linux/aarch64" in readme
    assert "macOS/arm64" in readme
    assert "ORAS `1.3.2`" in readme
    assert "7929f792cf272268412375ecad6f0fb3c20f164368d5b57966e67ad6d36eca53" in readme
    assert "OCI image" in readme
    assert "pull by digest" in readme
    assert "signed publication receipt" in readme
    assert "has no arbitrary checkout input" in readme
    assert "persisted Git credentials disabled" in readme
    assert "private reset bundle" in readme
    assert "never enter Actions artifact storage or OCI" in readme
    assert "first without `--apply`" in readme
    assert "then with `--apply`" in readme
    assert "never composed beneath `RUNNER_TEMP`" in readme
    assert "TAIRA_MACOS_RESET_STAGING_ROOT" in readme
    assert "TAIRA_MACOS_SOURCE_RESET_BUNDLE_SHA256" in readme
    assert "a path alone is not an authority" in readme
    assert "exactly five files" in readme
    assert "not an OCI layer" in readme
    assert "exact four files" in readme
    assert "step-scoped" in readme
    assert "macos-arm64.deploy" not in readme


def _assert_native_evidence_fail_closed_static_contract(source: str) -> None:
    assert 'if [[ ! -s "$release_input" || -L "$release_input" ]]' in source
    assert "stat -c '%h' \"$release_input\"" in source
    assert 'if [[ ! -s "$evidence_path" || -L "$evidence_path" ]]' in source
    assert 'cp "$PRIVACY_EXPECTATIONS_NORITO"' in source
    assert 'cp "$PRIVACY_EXPECTATIONS_JSON"' in source
    assert 'cp "$PRIVACY_X509_RESOURCE_NORITO"' in source
    assert 'cp "$PRIVACY_X509_RESOURCE_JSON"' in source
    bundled_verify = source.partition(
        '"${bundle_dir}/bin/${PRIVACY_RELEASE_RUNNER_BIN}" verify'
    )[2].partition(
        'assert_workspace_source_manifest_unchanged "post-bundled-runner-verification"'
    )[0]
    bundled_common = source.partition("bundled_privacy_runner_common_args=(")[
        2
    ].partition("\n)")[0]
    assert "--expectations-norito" in bundled_common
    assert "--expectations-json" in bundled_common
    assert "--x509-resource-norito" in bundled_common
    assert "--x509-resource-json" in bundled_common
    assert '"${bundled_privacy_runner_common_args[@]}"' in bundled_verify
    assert "--command-manifest-norito" in bundled_verify
    assert "--command-manifest-json" in bundled_verify
    assert "--stage-artifacts-norito" in bundled_verify
    assert "--stage-artifacts-json" in bundled_verify
    assert "--receipt-norito" in bundled_verify
    assert "--receipt-json" in bundled_verify


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            'if [[ ! -s "$release_input" || -L "$release_input" ]]',
            'if [[ ! -s "$release_input" ]]',
            1,
        ),
        lambda source: source.replace(
            'cp "$PRIVACY_EXPECTATIONS_JSON"',
            "# missing JSON expectation projection",
            1,
        ),
        lambda source: source.replace(
            'cp "$PRIVACY_X509_RESOURCE_JSON"',
            "# missing JSON resource-certificate projection",
            1,
        ),
        lambda source: source.replace(
            '  --receipt-json "${bundle_dir}/${privacy_release_json_relative_path}"',
            "  # mutated receipt pair omitted",
            1,
        ),
    ),
    ids=(
        "symlink-accepted",
        "projection-missing",
        "resource-projection-missing",
        "pair-mutated",
    ),
)
def test_native_evidence_static_contract_rejects_adversarial_mutations(
    mutation,
) -> None:
    source = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    _assert_native_evidence_fail_closed_static_contract(source)
    with pytest.raises(AssertionError):
        _assert_native_evidence_fail_closed_static_contract(mutation(source))


def test_release_bundle_rejects_skip_build_before_external_prerequisites() -> None:
    builder = TAIRA_DIR / "build_taira_rollout_bundle.sh"
    source = builder.read_text(encoding="utf-8")
    result = subprocess.run(
        ["bash", str(builder), "--profile", "release", "--skip-build"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert (
        "refusing --skip-build with --profile release: release binaries must be "
        "rebuilt from the exact tested source" in result.stderr
    )
    assert "Kagemusha release policy is mandatory" not in result.stderr
    release_guard = source.index(
        'if [[ "$PROFILE" == "release" && $SKIP_BUILD -eq 1 ]]'
    )
    prerequisite_checks = source.index("release_inputs=(")
    assert release_guard < prerequisite_checks


def test_release_bundle_rejects_skipped_regressions_before_external_prerequisites() -> (
    None
):
    builder = TAIRA_DIR / "build_taira_rollout_bundle.sh"
    source = builder.read_text(encoding="utf-8")
    result = subprocess.run(
        [
            "bash",
            str(builder),
            "--profile",
            "release",
            "--skip-local-regressions",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert (
        "refusing --skip-local-regressions with --profile release: every "
        "release gate is mandatory" in result.stderr
    )
    assert "must be provisioned outside the Iroha checkout" not in result.stderr
    release_guard = source.index(
        'if [[ "$PROFILE" == "release" && $SKIP_LOCAL_REGRESSIONS -eq 1 ]]'
    )
    prerequisite_checks = source.index("release_inputs=(")
    assert release_guard < prerequisite_checks


def _assert_portable_signed_taira_authority_contract(
    builder: str,
    finalizer: str,
    workflow: str,
) -> None:
    for required in (
        "taira_release_authority",
        "release_artifact_contract",
        "generate_release_manifest",
        "release_manifest_signing",
    ):
        assert required in finalizer
    for variable in (
        "TAIRA_RELEASE_EXTERNAL_SIGNER_PATH",
        "TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH",
        "TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT",
        "TAIRA_RELEASE_MANIFEST_VERIFIER_PATH",
        "TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256",
    ):
        assert variable not in builder
        assert variable in workflow

    assert '"repo_root":' not in builder
    assert '"repo_root":' not in finalizer
    assert "taira-exact12-release-authority-v1.json" in finalizer
    assert "sign_release_manifest(" in finalizer
    assert finalizer.count("verify_release_manifest(") == 1
    assert "build_authority(" in finalizer
    assert finalizer.count("build_release_manifest(") == 2
    assert "finalize-linux --" in workflow
    assert "publish-rollout --" in workflow
    assert "sign-manifest" not in workflow
    assert (
        'parser.add_argument("--trusted-signing-fingerprint", required=True)'
        in finalizer
    )
    assert "--trusted-signing-fingerprint" in workflow
    assert "--trusted-release-manifest-verifier-sha256" in finalizer
    assert "--trusted-release-manifest-verifier-sha256" in workflow
    assert "sorafs-validate" in finalizer
    # The authority host executes the sealed finalizer through the fixed
    # controller.  Source-controlled workflow text must not select the
    # validator implementation directly.
    assert "sorafs-validate" not in workflow
    assert 'output / "release_manifest.json.sig"' in finalizer
    assert 'output / "release_manifest.json.pub"' in finalizer
    assert "release_manifest.json.sig" in workflow
    assert "release_manifest.json.pub" in workflow
    assert "release manifest replay differs after signing" in finalizer
    assert "CONTROLLER_MANIFEST_NAME" in finalizer
    assert "authority-controller-v1.json" in workflow
    assert "verify_controller_closure(" in finalizer
    assert 'parser.add_argument("--checkout-root", required=True)' in finalizer
    assert (
        'parser.add_argument("--public-privacy-input-dir", required=True)'
        in finalizer
    )
    assert 'parser.add_argument("--controller-manifest", required=True)' in finalizer
    assert 'parser.add_argument("--controller-digest", required=True)' in finalizer
    assert '--controller-manifest "$TAIRA_CONTROLLER_ROOT/authority-controller-v1.json"' in workflow
    assert '--controller-digest "$EXPECTED_CONTROLLER_DIGEST"' in workflow
    assert "comparison.resolve(strict=True)" in finalizer
    assert "canonical != expected" in finalizer
    assert finalizer.count("_external_authority_file(") == 4
    assert "must be provisioned outside the checkout" in finalizer
    assert "expected_archive = evidence_root.parent" in finalizer
    assert "expected_output = evidence_root.parent" in finalizer
    assert "authority output inventory is not exactly closed" in finalizer
    assert "public_input_digests = _verify_public_privacy_inputs(" in finalizer
    assert "unsigned archive substituted the trusted public privacy input" in finalizer
    assert '--public-privacy-input-dir "$public_root/inputs"' in workflow
    assert "--expected-kind public-privacy-input" in workflow
    assert "import subprocess" not in finalizer
    assert "os.system" not in finalizer
    assert "subprocess." not in finalizer
    assert "taira_privacy_release_runner" not in finalizer
    assert "bin/irohad" not in finalizer
    assert "target/release" not in finalizer
    linux_snapshot = workflow.index("Snapshot only the four public privacy inputs")
    linux_build = workflow.index("Reconstruct source and build the unsigned Linux archive")
    linux_authenticate = workflow.index("Finalize and sign Linux bytes using only installed authority code")
    qualification = workflow.index("macos-secret-free-qualification:")
    sign = workflow.index("macos-candidate-authority:")
    direct = workflow.index("macos-deploy:")
    public = workflow.index("Require exact-four public advancement without signer or OCI credentials")
    publish = workflow.index("Publish through the sealed installed authority controller")
    assert (
        linux_snapshot
        < linux_build
        < linux_authenticate
        < qualification
        < sign
        < direct
        < public
        < publish
    )
    assert workflow.count("actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093") >= 9
    assert "inspect-handoff" in workflow[linux_authenticate:]
    assert "publish-rollout --" in workflow[publish:]
    assert "admit -- verify" not in workflow[publish:]
    linux_build_block = workflow[linux_build:linux_authenticate]
    for protected in (
        "TAIRA_RELEASE_EXTERNAL_SIGNER_PATH",
        "TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH",
        "TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT",
        "TAIRA_RELEASE_MANIFEST_VERIFIER_PATH",
        "TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256",
        "TAIRA_PRIVACY_RELEASE_INPUT_DIR",
    ):
        assert protected not in linux_build_block
    assert "env -i" in linux_build_block
    authentication_block = workflow[
        linux_authenticate : workflow.index("macos-native-build:")
    ]
    assert "finalize-linux" in authentication_block
    assert "$TAIRA_CONTROLLER_COMMAND" in authentication_block
    assert "cargo " not in authentication_block
    assert "build_taira_rollout_bundle.sh" not in authentication_block


def test_taira_release_requires_portable_signed_exact12_authority() -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    finalizer = AUTHORITY_FINALIZER.read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    _assert_portable_signed_taira_authority_contract(builder, finalizer, workflow)


@pytest.mark.parametrize(
    "mutation",
    (
        lambda source: source.replace(
            "sign_release_manifest(",
            "deleted_signature_gate(",
            1,
        ),
        lambda source: source.replace(
            "replay_manifest = canonical_json_bytes(build_release_manifest(manifest_args))",
            "replay_manifest = manifest_payload  # replay deleted",
            1,
        ),
        lambda source: source.replace(
            'parser.add_argument("--trusted-signing-fingerprint", required=True)',
            'parser.add_argument("--trusted-signing-fingerprint")',
        ),
        lambda source: source.replace(
            "if canonical != expected:",
            "if False:  # canonical external-path guard deleted",
            1,
        ),
        lambda source: (
            source + '\n# "repo_root": reintroduced absolute build host path\n'
        ),
        lambda source: source.replace(
            "public_input_digests = _verify_public_privacy_inputs(",
            "public_input_digests = deleted_public_input_verification(",
            1,
        ),
    ),
    ids=(
        "signing-deleted",
        "replay-deleted",
        "fingerprint-unpinned",
        "canonical-external-path-guard-deleted",
        "absolute-host-path",
        "trusted-public-input-verification-deleted",
    ),
)
def test_bundle_signed_authority_static_contract_rejects_mutations(
    mutation,
) -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    finalizer = AUTHORITY_FINALIZER.read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    _assert_portable_signed_taira_authority_contract(builder, finalizer, workflow)
    with pytest.raises(AssertionError):
        _assert_portable_signed_taira_authority_contract(
            builder,
            mutation(finalizer),
            workflow,
        )


def test_authority_finalizer_rejects_parent_alias_into_checkout() -> None:
    aliased_repo_file = str(ROOT / ".." / ROOT.name / "Cargo.lock")
    assert ".." in aliased_repo_file
    with pytest.raises(
        authority_finalizer.FinalizationError,
        match="canonical physical path without symlink aliases",
    ):
        authority_finalizer._canonical_absolute(
            Path(aliased_repo_file),
            "external signer",
        )


def test_authority_finalizer_rejects_authority_file_inside_checkout() -> None:
    with pytest.raises(
        authority_finalizer.FinalizationError,
        match="must be provisioned outside the checkout",
    ):
        authority_finalizer._external_authority_file(
            ROOT / "Cargo.lock",
            "external signer",
            ROOT,
        )


def test_workflow_canonical_external_path_guard_rejects_mutation() -> None:
    builder = (TAIRA_DIR / "build_taira_rollout_bundle.sh").read_text(encoding="utf-8")
    finalizer = AUTHORITY_FINALIZER.read_text(encoding="utf-8")
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    mutated = finalizer.replace(
        "if canonical != expected:",
        "if False:  # canonical external-path guard deleted",
        1,
    )

    _assert_portable_signed_taira_authority_contract(builder, finalizer, workflow)
    with pytest.raises(AssertionError):
        _assert_portable_signed_taira_authority_contract(builder, mutated, workflow)


def test_workflow_dispatch_inputs_never_enter_shell_source() -> None:
    workflow = (
        ROOT / ".github" / "workflows" / "publish_taira_validator.yml"
    ).read_text(encoding="utf-8")
    assert (
        "TAIRA_INPUT_VALIDATOR_RELEASE_REF: "
        "${{ inputs.validator_release_ref }}" in workflow
    )
    assert "checkout_ref" not in workflow
    assert "permissions:\n  contents: read" in workflow
    assert workflow.count("ref: ${{ github.sha }}") == 2
    assert workflow.count("persist-credentials: false") == 2
    assert (
        workflow.count('[[ "$TAIRA_INPUT_VALIDATOR_RELEASE_REF" =~ ^[0-9a-f]{40}$ ]]')
        == 3
    )
    assert workflow.count('[[ "$TAIRA_WORKFLOW_COMMIT" =~ ^[0-9a-f]{40}$ ]]') == 3
    assert "inputs.artifact_suffix" not in workflow
    assert "push_latest" not in workflow
    assert "release_ref='${{ inputs.validator_release_ref }}'" not in workflow
    assert 'raw_suffix="${{ inputs.artifact_suffix }}"' not in workflow
    assert "TAIRA_OCI_TAG_SUFFIX: ${{ vars.TAIRA_OCI_TAG_SUFFIX }}" in workflow
    assert '--suffix "$TAIRA_OCI_TAG_SUFFIX"' in workflow
    assert "publish-rollout --" in workflow
    assert "source-${TAIRA_WORKSPACE_SOURCE_MANIFEST_SHA256:0:12}" not in workflow


def test_mcp_rollout_has_no_backend_offline_admission_gate() -> None:
    source = (TAIRA_DIR / "check_mcp_rollout.sh").read_text(encoding="utf-8")

    assert "/v1/offline/readiness" not in source
    assert "--offline-asset-definition-id" not in source
    assert "--offline-expected-identity" not in source
    assert "OFFLINE_ASSET_DEFINITION_ID" not in source
    assert "OFFLINE_EXPECTED_IDENTITY_PATH" not in source
    assert (
        'EXPECTED_IS_ROUTE_ALIAS="${EXPECTED_IS_ROUTE_ALIAS:-external-poc}"' in source
    )
    assert (
        'EXPECTED_IS2_ROUTE_ALIAS="${EXPECTED_IS2_ROUTE_ALIAS:-boi-mobile}"' in source
    )
    assert '"$(normalize_root_url "$root")/health"' in source
    assert '"$(normalize_root_url "$root")/readyz"' in source
    assert '"$(normalize_root_url "$root")/v1/nexus/lifecycle"' in source


def test_mcp_automatic_canary_threads_explicit_onboarding_token_file() -> None:
    source = (TAIRA_DIR / "check_mcp_rollout.sh").read_text(encoding="utf-8")

    assert "--onboarding-token-file)" in source
    assert (
        "automatic canary bootstrap requires --onboarding-token-file "
        "ABSOLUTE_PATH" in source
    )
    assert '--onboarding-token-file "$ROLLOUT_CANARY_ONBOARDING_TOKEN_FILE"' in source
    assert 'domain = account.get("domain", "universal")' in source
    assert 'domain = f"{domain}.universal"' not in source


def test_public_cutover_cannot_skip_fleet_or_exact_commit() -> None:
    source = (TAIRA_DIR / "check_mcp_rollout.sh").read_text(encoding="utf-8")

    assert "TAIRA_RELEASE_VALIDATOR_COUNT=4" in source
    assert "public Taira rollout requires --require-all-validators" in source
    assert (
        "public Taira rollout requires exactly ${TAIRA_RELEASE_VALIDATOR_COUNT}"
        in source
    )
    assert (
        "public Taira rollout requires --expected-git-sha with the exact full 40-character commit"
        in source
    )
    assert (
        "public Taira rollout requires at least three advancing validator fleet samples"
        in source
    )
    assert "REQUIRE_EXACT_GIT_SHA=1" in source
    assert (
        "Taira MCP local diagnostic checks passed; this is not public cutover evidence."
        in source
    )


def test_readme_rollout_commands_are_executable_under_fail_closed_parser() -> None:
    readme = (TAIRA_DIR / "README.md").read_text(encoding="utf-8")
    command_lines = [
        line
        for line in readme.splitlines()
        if "check_mcp_rollout.sh" in line
        and line.lstrip().startswith(("- `bash", "`bash"))
    ]
    assert command_lines
    for line in command_lines:
        assert "--offline-asset-definition-id" not in line, line
        assert "--offline-expected-identity" not in line, line
        if "--public-root" in line:
            assert '"${TAIRA_VALIDATOR_ARGS[@]}"' in line, line
            assert "--require-all-validators" in line, line
            assert "--expected-git-sha" in line, line
