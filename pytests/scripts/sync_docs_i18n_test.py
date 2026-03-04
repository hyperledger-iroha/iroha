from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


def test_language_stub_strings_do_not_use_todo_marker():
    script_path = (
        Path(__file__).resolve().parents[2] / "scripts" / "sync_docs_i18n.py"
    )
    spec = spec_from_file_location("sync_docs_i18n", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    import sys
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]

    placeholder = "TO" + "DO"
    for code, stub in module.LANGUAGE_STRINGS.items():
        assert (
            placeholder not in stub.org_todo
        ), f"org_todo for {code} still contains placeholder marker"
        assert (
            placeholder not in stub.markdown_todo
        ), f"markdown_todo for {code} still contains placeholder marker"
