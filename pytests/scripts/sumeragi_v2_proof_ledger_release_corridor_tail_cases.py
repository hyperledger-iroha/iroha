# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


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
    for curl_status, http_status, expected in (
        (0, "200", "github-release-asset"),
        (22, "404", "immutable-source-build"),
        (22, "410", "immutable-source-build"),
        (6, "000", None),
        (23, "404", None),
        (22, "403", None),
        (22, "429", None),
        (22, "500", None),
    ):
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                str(helper),
                "--lock",
                str(lock),
                "--platform",
                "arm64-darwin",
                "classify-release-fetch",
                "--curl-status",
                str(curl_status),
                "--http-status",
                http_status,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
        assert (
            result.returncode == 0 and result.stdout.strip() == expected
            if expected
            else result.returncode != 0
        )
