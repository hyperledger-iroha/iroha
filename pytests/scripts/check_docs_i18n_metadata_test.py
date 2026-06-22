import hashlib
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


def load_check_docs_i18n_metadata_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "ci" / "check_docs_i18n_metadata.py"
    )
    spec = spec_from_file_location("check_docs_i18n_metadata", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_needs_review_translation_allows_null_review_date(tmp_path):
    module = load_check_docs_i18n_metadata_module()
    module.REPO_ROOT = tmp_path
    source = tmp_path / "docs" / "formal" / "README.md"
    translation = tmp_path / "docs" / "formal" / "README.es.md"
    source.parent.mkdir(parents=True)
    source.write_text("# Source\n", encoding="utf-8")
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    translation.write_text(
        "\n".join(
            [
                "---",
                "source: docs/formal/README.md",
                f"source_hash: {source_hash}",
                "status: needs-review",
                "translation_last_reviewed: null",
                "---",
                "# Traduccion anterior",
                "",
            ]
        ),
        encoding="utf-8",
    )

    errors, warnings, report = module.audit_file(translation)

    assert errors == []
    assert warnings == []
    assert report["status"] == "needs-review"
    assert report["translation_last_reviewed"] == "null"


def test_complete_translation_requires_review_date(tmp_path):
    module = load_check_docs_i18n_metadata_module()
    module.REPO_ROOT = tmp_path
    source = tmp_path / "docs" / "formal" / "README.md"
    translation = tmp_path / "docs" / "formal" / "README.es.md"
    source.parent.mkdir(parents=True)
    source.write_text("# Source\n", encoding="utf-8")
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    translation.write_text(
        "\n".join(
            [
                "---",
                "source: docs/formal/README.md",
                f"source_hash: {source_hash}",
                "status: complete",
                "translation_last_reviewed: null",
                "---",
                "# Traduccion anterior",
                "",
            ]
        ),
        encoding="utf-8",
    )

    errors, warnings, report = module.audit_file(translation)

    assert warnings == []
    assert report["status"] == "complete"
    assert any("invalid translation_last_reviewed value" in error for error in errors)


def test_source_only_metadata_is_not_treated_as_translation(tmp_path):
    module = load_check_docs_i18n_metadata_module()
    module.REPO_ROOT = tmp_path
    page = tmp_path / "docs" / "portal" / "docs" / "example.md"
    page.parent.mkdir(parents=True)
    page.write_text(
        "\n".join(
            [
                "---",
                "title: Generated example",
                "source: crates/example/input.ko",
                "---",
                "# Example",
                "",
            ]
        ),
        encoding="utf-8",
    )

    errors, warnings, report = module.audit_file(page)

    assert errors == []
    assert warnings == []
    assert report is None


def test_lang_source_status_metadata_is_treated_as_translation(tmp_path):
    module = load_check_docs_i18n_metadata_module()
    module.REPO_ROOT = tmp_path
    source = tmp_path / "docs" / "portal" / "docs" / "example.md"
    translation = tmp_path / "docs" / "portal" / "i18n" / "es" / "example.md"
    source.parent.mkdir(parents=True)
    translation.parent.mkdir(parents=True)
    source.write_text("# Source\n", encoding="utf-8")
    translation.write_text(
        "\n".join(
            [
                "---",
                "lang: es",
                "source: docs/portal/docs/example.md",
                "status: complete",
                "translation_last_reviewed: 2026-06-22",
                "---",
                "# Ejemplo",
                "",
            ]
        ),
        encoding="utf-8",
    )

    errors, warnings, report = module.audit_file(translation)

    assert any("missing source_hash metadata" in error for error in errors)
    assert warnings == []
    assert report["path"] == "docs/portal/i18n/es/example.md"
