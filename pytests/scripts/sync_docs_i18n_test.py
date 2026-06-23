from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


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
