from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path

import pytest


def load_sync_docs_i18n_module():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sync_docs_i18n.py"
    )
    spec = spec_from_file_location("sync_docs_i18n", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    import sys

    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_language_stub_strings_do_not_use_todo_marker():
    module = load_sync_docs_i18n_module()
    placeholder = "TO" + "DO"
    for code, stub in module.LANGUAGE_STRINGS.items():
        assert (
            placeholder not in stub.org_todo
        ), f"org_todo for {code} still contains placeholder marker"
        assert (
            placeholder not in stub.markdown_todo
        ), f"markdown_todo for {code} still contains placeholder marker"


def test_root_translation_path_uses_i18n_root_directory():
    module = load_sync_docs_i18n_module()

    assert module.compute_translation_path(Path("README.md"), "ja") == Path(
        "docs/i18n/root/ja/README.md"
    )
    assert module.compute_translation_path(Path("roadmap.md"), "pt") == Path(
        "docs/i18n/root/pt/roadmap.md"
    )


def test_nested_translation_path_remains_next_to_source():
    module = load_sync_docs_i18n_module()

    assert module.compute_translation_path(
        Path("docs/source/community.md"), "fr"
    ) == Path("docs/source/community.fr.md")


def test_needs_review_translation_is_not_managed_stub():
    module = load_sync_docs_i18n_module()
    content = "\n".join(
        [
            "---",
            "status: needs-review",
            "generator: scripts/sync_docs_i18n.py",
            "---",
            "# Existing translation requiring review",
            "",
        ]
    )

    assert not module._is_managed_stub(content, Path("README.es.md"))


def test_explicit_refresh_replaces_completed_translation(tmp_path: Path):
    module = load_sync_docs_i18n_module()
    module.REPO_ROOT = tmp_path
    source = tmp_path / "docs" / "source" / "guide.md"
    translation = tmp_path / "docs" / "source" / "guide.es.md"
    source.parent.mkdir(parents=True)
    source.write_text("# Current guide\n", encoding="utf-8")
    translation.write_text(
        "---\nstatus: complete\n---\n# Stale translated guide\n",
        encoding="utf-8",
    )
    language = module.Language(code="es", name="Spanish")

    assert (
        module.ensure_stub(
            source,
            translation,
            language,
            dry_run=False,
            force_refresh=True,
        )
        == "update"
    )
    refreshed = translation.read_text(encoding="utf-8")
    assert "status: needs-translation" in refreshed
    assert "source: docs/source/guide.md" in refreshed
    assert "Stale translated guide" not in refreshed


def test_explicit_refresh_refuses_symlinked_translation(tmp_path: Path):
    module = load_sync_docs_i18n_module()
    module.REPO_ROOT = tmp_path
    source = tmp_path / "docs" / "source" / "guide.md"
    translation = tmp_path / "docs" / "source" / "guide.es.md"
    target = tmp_path / "translation-target.md"
    source.parent.mkdir(parents=True)
    source.write_text("# Current guide\n", encoding="utf-8")
    target.write_text("# User translation\n", encoding="utf-8")
    translation.symlink_to(target)
    language = module.Language(code="es", name="Spanish")

    with pytest.raises(ValueError, match="symlinked translation"):
        module.ensure_stub(
            source,
            translation,
            language,
            dry_run=False,
            force_refresh=True,
        )
    assert target.read_text(encoding="utf-8") == "# User translation\n"


def test_refresh_sources_require_exact_configured_canonical_paths():
    module = load_sync_docs_i18n_module()
    available = [
        Path("docs/source/guide.md"),
        Path("docs/source/other.md"),
    ]

    assert module.normalize_refresh_sources(
        ["docs/source/guide.md"], available
    ) == {Path("docs/source/guide.md")}
    for invalid in (
        "../guide.md",
        "/docs/source/guide.md",
        "docs/source/../guide.md",
        "docs/source/missing.md",
    ):
        with pytest.raises(ValueError):
            module.normalize_refresh_sources([invalid], available)
