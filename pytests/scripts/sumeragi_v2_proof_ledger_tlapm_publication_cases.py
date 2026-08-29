# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_tlapm_publication_is_atomic_no_replace_and_preserves_winner(
    tmp_path: Path,
) -> None:
    tmp_path = tmp_path.resolve(strict=True)
    tmp_path.chmod(0o700)
    formal_scripts = ROOT_DIR / "scripts" / "formal"
    lock = formal_scripts / "sumeragi_v2_tlapm_source_build_lock.json"
    helper = formal_scripts / "sumeragi_v2_tlapm_source_lock.py"
    common = [
        sys.executable,
        "-I",
        "-S",
        str(helper),
        "--lock",
        str(lock),
        "--platform",
        "arm64-darwin",
    ]
    archive = tmp_path / "archive.tar.gz"
    attestation = tmp_path / "attestation.json"
    archive.write_bytes(b"archive winner\n")
    attestation.write_bytes(b"attestation winner\n")
    bundle = tmp_path / "bundle"
    publish_bundle = [
        *common,
        "publish-output-bundle",
        "--archive",
        str(archive),
        "--attestation",
        str(attestation),
        "--output-bundle",
        str(bundle),
    ]
    published = subprocess.run(
        publish_bundle,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert published.returncode == 0, published.stderr
    winner = (bundle / "archive.tar.gz").read_bytes()
    raced = subprocess.run(
        publish_bundle,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert raced.returncode == 3
    assert (bundle / "archive.tar.gz").read_bytes() == winner

    failed_bundle = tmp_path / "failed-bundle"
    failed_command = [str(tmp_path / "missing-attestation") if argument == str(attestation)
        else str(failed_bundle) if argument == str(bundle) else argument for argument in publish_bundle]
    failed = subprocess.run(failed_command, check=False, capture_output=True, text=True, timeout=10)
    assert failed.returncode != 0 and not failed_bundle.exists()
    assert not tuple(tmp_path.glob(".failed-bundle.*.stage"))

    install_stage = tmp_path / "install-stage"
    install_stage.mkdir(mode=0o700)
    (install_stage / "winner").write_bytes(b"first\n")
    install = tmp_path / "installed"
    publish_install = [
        *common,
        "publish-install",
        "--staged",
        str(install_stage),
        "--destination",
        str(install),
    ]
    first_install = subprocess.run(
        publish_install,
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert first_install.returncode == 0, first_install.stderr
    second_stage = tmp_path / "second-stage"
    second_stage.mkdir(mode=0o700)
    (second_stage / "winner").write_bytes(b"second\n")
    second_install = subprocess.run(
        [
            *common,
            "publish-install",
            "--staged",
            str(second_stage),
            "--destination",
            str(install),
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert second_install.returncode == 3
    assert (install / "winner").read_bytes() == b"first\n"


def test_tlapm_archive_precedence_only_falls_back_on_asset_unavailability() -> None:
    installer = (
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh"
    ).read_text(encoding="utf-8")
    normalized = " ".join(installer.replace("\\\n", "").split())
    assert "/usr/bin/curl --proto '=https' --http1.1" in normalized

    caller_index = normalized.index(
        'if [[ -n "${TLAPM_ARCHIVE_PATH:-}" ]]; then'
    )
    asset_index = normalized.index("/usr/bin/curl --proto '=https'")
    builder_index = normalized.index(
        '/bin/bash "$FROZEN_SOURCE_BUILDER" "$PLATFORM" "$SOURCE_BUILD_BUNDLE"'
    )
    checksum_index = normalized.index(
        'if [[ "$archive_origin" != immutable-source-build'
    )
    extraction_index = normalized.index('tar -xzf "$archive_path"')
    assert caller_index < asset_index < builder_index < checksum_index < extraction_index

    caller_branch = normalized[caller_index:asset_index]
    assert "SOURCE_BUILD_SCRIPT" not in caller_branch
    assert 'archive_origin="caller-archive"' in caller_branch
    assert 'archive_origin="github-release-asset"' in normalized
    assert 'archive_origin="immutable-source-build"' in normalized
    assert normalized.count("verify-attestation") == 1
    assert "classify-release-fetch" in normalized
    assert "snapshot-corridor" in installer
    assert "verify-install" in installer
    assert "write-install-state" in installer
    assert "publish-install" in installer
    assert 'rm -rf -- "$INSTALL_DIR"' not in installer
    assert 'rm -f -- "$OUTPUT_ARCHIVE"' not in installer
    assert "refusing stale, partial, or unauthenticated TLAPM cache" in installer
    assert (
        'printf \'%s\\n\' "$actual_sha256" > "${INSTALL_STAGE}/archive.sha256"'
        in normalized
    )

    helper = ROOT_DIR / "scripts/formal/sumeragi_v2_tlapm_source_lock.py"
    lock = ROOT_DIR / "scripts/formal/sumeragi_v2_tlapm_source_build_lock.json"
    for curl_status, http_status, expected in ((0, "200", "github-release-asset"),
        (22, "404", "immutable-source-build"), (22, "410", "immutable-source-build"),
        (6, "000", None), (23, "404", None), (22, "403", None),
        (22, "429", None), (22, "500", None)):
        result = subprocess.run([sys.executable, "-I", "-S", str(helper), "--lock",
            str(lock), "--platform", "arm64-darwin", "classify-release-fetch",
            "--curl-status", str(curl_status), "--http-status", http_status],
            check=False, capture_output=True, text=True, timeout=10)
        assert (result.returncode == 0 and result.stdout.strip() == expected) if expected else result.returncode != 0
