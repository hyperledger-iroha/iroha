"""Static guards for fail-closed OpenAPI release generation."""

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
XTASK = REPO_ROOT / "xtask" / "src" / "main.rs"
TORII_OPENAPI = REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs"
PORTAL_SCRIPTS = (
    REPO_ROOT / "docs" / "portal" / "scripts" / "sync-openapi.mjs",
    REPO_ROOT / "docs" / "portal" / "scripts" / "verify-openapi-versions.mjs",
    REPO_ROOT / "docs" / "portal" / "scripts" / "check-openapi-signatures.mjs",
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


def test_portal_version_and_signature_paths_reject_empty_specs() -> None:
    for path in PORTAL_SCRIPTS:
        source = path.read_text(encoding="utf-8")
        assert "validateReleaseOpenApiDocumentBytes" in source, path
