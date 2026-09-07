#!/usr/bin/env python3
"""Check shared emitter ownership, error propagation and retained diagnostic tests.

Current contracts are structural. Historical source/lock hashes and opening
objects remain in docs/history/2026-09-07/norito-helper-compaction.json; they do
not freeze unrelated dependencies, local test ordering or source formatting.
This source check does not replace compiled procedural-macro and UI suites.
"""

from __future__ import annotations

import re
import unittest

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 uses scripts/requirements.txt.
    import tomli as tomllib

from scripts import check_norito_codec_contracts as rust
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MACRO_OWNER = "crates/iroha_derive/src/lib.rs"
MAX_MACRO_LINES = 85
EXPECTED_TEST_IDS = (
    "handle_ok",
    "handle_err",
    "handle_or_default_returns_default",
    "finish_token_stream_with_appends_tokens",
)
MACRO_CONSUMERS = {
    "crates/iroha_data_model_derive/src/lib.rs": "crates/iroha_data_model_derive/Cargo.toml",
    "crates/iroha_executor_derive/src/lib.rs": "crates/iroha_executor_derive/Cargo.toml",
    "crates/iroha_schema_derive/src/lib.rs": "crates/iroha_schema_derive/Cargo.toml",
    "crates/iroha_smart_contract_derive/src/lib.rs": (
        "crates/iroha_smart_contract_derive/Cargo.toml"
    ),
    "crates/iroha_trigger_derive/src/lib.rs": "crates/iroha_trigger_derive/Cargo.toml",
}
INTERNAL_IMPORTS = (
    "crates/iroha_data_model_derive/src/event_set.rs",
    "crates/iroha_data_model_derive/src/has_origin.rs",
    "crates/iroha_data_model_derive/src/id.rs",
    "crates/iroha_executor_derive/src/default.rs",
)
DELETED_COPIES = tuple(path.replace("lib.rs", "emitter_ext.rs") for path in MACRO_CONSUMERS)
LOCAL_COPIES = (
    "crates/iroha_ffi/proc_macro/src/emitter_ext.rs",
    "crates/iroha_telemetry_derive/src/emitter_ext.rs",
)

LOCAL_OWNERS = {
    "crates/iroha_ffi/proc_macro/src/lib.rs": "crates/iroha_ffi/proc_macro/src/emitter_ext.rs",
    "crates/iroha_telemetry_derive/src/lib.rs": (
        "crates/iroha_telemetry_derive/src/emitter_ext.rs"
    ),
}


class GuardError(AssertionError):
    """Raised when the authenticated emitter-sharing contract drifts."""


def _require(condition: bool, diagnostic: str) -> None:
    if not condition:
        raise GuardError(diagnostic)


def _operation(source: str, name: str, owner: str):
    items = [item for item in rust.functions(source) if item.name == name]
    _require(len(items) == 1, f"emitter.operation:{owner}::{name}")
    return items[0]


def _emitter_behavior(source: str, owner: str, local: bool) -> None:
    code = rust.compact(source)
    _require(code.count("traitEmitterExt{") == 1, f"emitter.trait:{owner}")
    implementation = "implEmitterExtforEmitter{" if local else "implEmitterExtformanyhow::Emitter{"
    _require(code.count(implementation) == 1, f"emitter.impl:{owner}")
    expected = {
        "handle": "matchresult{Ok(value)=>Some(value),Err(err)=>{self.emit(err);None}}",
        "finish_token_stream": "self.finish_token_stream_with(proc_macro2::TokenStream::new())",
        "finish_token_stream_with": "ifletErr(err)=self.into_result(){manyhow::ToTokensError::to_tokens(&err,&muttokens);}tokens",
    }
    if local:
        expected.update({
            "handle_or_default": "self.handle(result).unwrap_or_default()",
            "finish_token_stream": "self.finish_token_stream_with(TokenStream::new())",
            "finish_token_stream_with": "ifletErr(err)=self.into_result(){err.to_tokens(&muttokens);}tokens",
        })
    for name, body in expected.items():
        item = _operation(source, name, owner)
        _require(item.code == body, f"emitter.behavior:{owner}::{name}")
        if name == "handle":
            _require("ToTokensError+'static,T>" in item.signature and "result:manyhow::Result<T,E>" in item.signature and item.signature.endswith("->Option<T>"), f"emitter.typed_result:{owner}")


def _active_test(source: str, name: str, owner: str):
    item = _operation(source, name, owner)
    attrs = re.search(r"((?:\s*#\[[^\n]*\])+\s*)$", source[:item.start])
    _require(attrs is not None and rust.has_literal_syntax(attrs.group(1), "#[test]") and not re.search(r"\b(?:cfg|cfg_attr|ignore)\b", rust.compact(attrs.group(1))), f"emitter.test_disabled:{owner}::{name}")
    return item


def _local_tests(source: str, owner: str) -> None:
    # Keep each real diagnostic assertion, without pinning order or whole files.
    tests = {
        "handle_ok": ("assert_eq!(value,Some(42));", "assert!(e.finish_token_stream().is_empty());"),
        "handle_err": ("assert!(value.is_none());", "assert!(!e.finish_token_stream().is_empty());"),
        "handle_or_default_returns_default": ("assert_eq!(value,0);",),
        "finish_token_stream_with_appends_tokens": ("assert!(token_string.contains(", "assert!(token_string.len()>"),
    }
    for name, assertions in tests.items():
        item = _active_test(source, name, owner)
        _require(all(re.search(r"(?<![\w])" + re.escape(assertion), item.code) for assertion in assertions), f"emitter.test_assertions:{owner}::{name}")
        if name == "finish_token_stream_with_appends_tokens":
            for syntax in ('quote! { initial }', 'token_string.contains("initial")', 'token_string.len() > "initial".len()'):
                _require(rust.has_literal_syntax(item.raw_body, syntax), f"emitter.appended_tokens_test:{owner}")
    diagnostic = "ManyhowError" if "iroha_ffi/" in owner else "Error"
    error_case = _active_test(source, "handle_err", owner)
    _require(f".handle::<{diagnostic},_>(Err(" in error_case.code, f"emitter.diagnostic_type:{owner}")
    if diagnostic == "Error":
        _require("usesyn::Error;" in rust.compact(source), f"emitter.diagnostic_type:{owner}")
    module = re.search(r"(?m)^((?:\s*#\[[^\n]*\])+\s*)mod\s+tests\s*\{", source)
    _require(module is not None and rust.compact(module.group(1)) == "#[cfg(test)]", f"emitter.test_module:{owner}")
    _require(not re.search(r"(?m)^#!\[cfg", rust._mask_non_code(source)), f"emitter.module_disabled:{owner}")


def _manifest(source: str, path: str) -> dict:
    try:
        return tomllib.loads(source)
    except tomllib.TOMLDecodeError as error:
        raise GuardError(f"emitter.manifest_invalid:{path}") from error


def _dependency_tables(document: dict):
    for key, value in document.items():
        if isinstance(value, dict):
            if key == "dependencies" or key.endswith("-dependencies"):
                yield value
            else:
                yield from _dependency_tables(value)


def validate(
    owner: str,
    consumers: dict[str, str],
    manifests: dict[str, str],
    internal_imports: dict[str, str],
    local_copies: dict[str, str],
    local_owners: dict[str, str],
    deleted_present: tuple[str, ...],
) -> None:
    production = rust.production(owner)
    item = _operation(production, "define_emitter_ext", MACRO_OWNER)
    block = production[item.start:item.end + 1]
    _require(len(block.splitlines()) <= MAX_MACRO_LINES, "emitter.macro_line_ceiling")
    _require(max(map(len, block.splitlines())) <= 100, "emitter.macro_packed_line")
    prefix = production[:item.start]
    _require(re.search(r"#\[proc_macro\]\s*pub\s*$", prefix) is not None, "emitter.proc_macro_registration")
    _require("if!input.is_empty(){returnsyn::Error::new(" in item.code and ").to_compile_error().into();}" in item.code, "emitter.rejects_input")
    _require(rust.has_literal_syntax(item.raw_body, 'syn::Error::new(proc_macro2::Span::call_site(), "define_emitter_ext! accepts no input",)'), "emitter.input_diagnostic")
    _require("quote!{" in item.code and item.code.endswith("}.into()"), "emitter.expands_typed_items")
    _require(not re.search(r"macro_rules!|\$(?:body|assert)|\b(?:Action|Step)\b", rust._mask_non_code(block)), "emitter.no_body_dsl")
    _require(not re.search(r"\bpub(?:\([^)]*\))?\s+trait\s+EmitterExt", rust._mask_non_code(item.raw_body)), "emitter.private_generated_trait")
    _emitter_behavior(item.raw_body, MACRO_OWNER, False)

    _require(not deleted_present, "emitter.duplicate_modules")
    for path, manifest_path in MACRO_CONSUMERS.items():
        code = rust.compact(rust.production(consumers[path]))
        _require(code.count("iroha_derive::define_emitter_ext!();") == 1, f"emitter.consumer:{path}")
        _require(not any(token in code for token in ("modemitter_ext;", "iroha_derive_primitives::EmitterExt", "traitEmitterExt{")), f"emitter.duplicate_definition:{path}")
        document = _manifest(manifests[manifest_path], manifest_path)
        dependency = document.get("dependencies", {}).get("iroha_derive", {})
        _require(isinstance(dependency, dict) and dependency.get("workspace") is True and not dependency.get("optional", False), f"emitter.required_dependency:{manifest_path}")
        for table in _dependency_tables(document):
            _require(not any(name == "iroha_derive_primitives" or (isinstance(value, dict) and value.get("package") == "iroha_derive_primitives") for name, value in table.items()), f"emitter.forbidden_dependency:{manifest_path}")

    for path, source in internal_imports.items():
        code = rust.compact(rust.production(source))
        _require("EmitterExt" in code, f"emitter.internal_import:{path}")
        _require(not any(token in code for token in ("emitter_ext::EmitterExt", "iroha_derive_primitives::EmitterExt")), f"emitter.import_bypass:{path}")
    _require(set(local_copies) == set(LOCAL_COPIES), "emitter.local_owner_inventory")
    for path, source in local_copies.items():
        _emitter_behavior(rust.production(source), path, True)
        _local_tests(source, path)
    for path, local_path in LOCAL_OWNERS.items():
        source = rust.production(local_owners[path])
        code = rust.compact(source)
        _require(code.count("modemitter_ext;") == 1 and code.count("emitter_ext::EmitterExt") == 1 and local_path in local_copies, f"emitter.local_registration:{path}")
        _require(re.search(r"#\[(?:cfg|cfg_attr)\b[^\n]*\]\s*mod emitter_ext;", source) is None, f"emitter.local_disabled:{path}")


def current_inputs() -> tuple:
    return (
        (ROOT / MACRO_OWNER).read_text(),
        {path: (ROOT / path).read_text() for path in MACRO_CONSUMERS},
        {path: (ROOT / path).read_text() for path in dict.fromkeys(MACRO_CONSUMERS.values())},
        {path: (ROOT / path).read_text() for path in INTERNAL_IMPORTS},
        {path: (ROOT / path).read_text() for path in LOCAL_COPIES},
        {path: (ROOT / path).read_text() for path in LOCAL_OWNERS},
        tuple(path for path in DELETED_COPIES if (ROOT / path).exists()),
    )


class SharedEmitterSourceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.inputs = current_inputs()

    def assert_rejected(self, changed: tuple, diagnostic: str) -> None:
        validate(*self.inputs)
        self.assertNotEqual(changed, self.inputs, "mutation must change the input")
        with self.assertRaises(GuardError) as raised:
            validate(*changed)
        self.assertEqual(str(raised.exception), diagnostic)

    def changed(self, slot: int, path: str | None, old: str, new: str) -> tuple:
        result = list(self.inputs)
        source = result[slot] if path is None else result[slot][path]
        self.assertIn(old, source, "mutation target must exist")
        source = source.replace(old, new, 1)
        if path is None:
            result[slot] = source
        else:
            result[slot] = {**result[slot], path: source}
        return tuple(result)

    def test_current_source(self) -> None:
        validate(*self.inputs)

    def test_each_typed_emitter_operation_rejects_control_flow_mutations(self) -> None:
        cases = (
            ("if !input.is_empty()", "if input.is_empty()", "emitter.rejects_input"),
            ("Some(value)", "None", f"emitter.behavior:{MACRO_OWNER}::handle"),
            ("self.emit(err);", "let _ = err;", f"emitter.behavior:{MACRO_OWNER}::handle"),
            ("self.finish_token_stream_with(proc_macro2::TokenStream::new())", "proc_macro2::TokenStream::new()", f"emitter.behavior:{MACRO_OWNER}::finish_token_stream"),
            ("manyhow::ToTokensError::to_tokens(&err, &mut tokens);", "let _ = err;", f"emitter.behavior:{MACRO_OWNER}::finish_token_stream_with"),
            ("                tokens\n", "                proc_macro2::TokenStream::new()\n", f"emitter.behavior:{MACRO_OWNER}::finish_token_stream_with"),
        )
        for old, new, diagnostic in cases:
            with self.subTest(diagnostic=diagnostic):
                self.assert_rejected(self.changed(0, None, old, new), diagnostic)
        for path in LOCAL_COPIES:
            for old, new, name in (
                ("Some(value)", "None", "handle"),
                ("self.emit(err);", "let _ = err;", "handle"),
                ("self.handle(result).unwrap_or_default()", "Default::default()", "handle_or_default"),
                ("self.finish_token_stream_with(TokenStream::new())", "TokenStream::new()", "finish_token_stream"),
                ("err.to_tokens(&mut tokens);", "let _ = err;", "finish_token_stream_with"),
            ):
                self.assert_rejected(self.changed(4, path, old, new), f"emitter.behavior:{path}::{name}")

    def test_macro_visibility_and_test_module_activation_are_preserved(self) -> None:
        self.assert_rejected(self.changed(0, None, "trait EmitterExt {", "pub trait EmitterExt {"), "emitter.private_generated_trait")
        for path in LOCAL_COPIES:
            self.assert_rejected(self.changed(4, path, "#[cfg(test)]\nmod tests", "#[cfg(any())]\n#[cfg(test)]\nmod tests"), f"emitter.test_module:{path}")

    def test_ownership_and_dependency_disconnections_rejected(self) -> None:
        for path, manifest in MACRO_CONSUMERS.items():
            self.assert_rejected(self.changed(1, path, "iroha_derive::define_emitter_ext!();", "// iroha_derive::define_emitter_ext!();"), f"emitter.consumer:{path}")
            self.assert_rejected(self.changed(2, manifest, "iroha_derive = { workspace = true }", "iroha_derive = { workspace = true, optional = true }"), f"emitter.required_dependency:{manifest}")
            self.assert_rejected(self.changed(2, manifest, "[dependencies]", '[dependencies]\nrenamed = { package = "iroha_derive_primitives", workspace = true }'), f"emitter.forbidden_dependency:{manifest}")
        for path in INTERNAL_IMPORTS:
            self.assert_rejected(self.changed(3, path, "EmitterExt", "RemovedEmitter"), f"emitter.internal_import:{path}")
        changed = (*self.inputs[:6], (DELETED_COPIES[0],))
        self.assert_rejected(changed, "emitter.duplicate_modules")
        for path in LOCAL_OWNERS:
            self.assert_rejected(self.changed(5, path, "mod emitter_ext;", "#[cfg(any())]\nmod emitter_ext;"), f"emitter.local_disabled:{path}")

    def test_local_diagnostic_tests_cannot_be_disabled_or_weakened(self) -> None:
        for path in LOCAL_COPIES:
            for name in EXPECTED_TEST_IDS:
                old = f"#[test]\n    fn {name}"
                for attrs in ("", "#[test]\n    #[ignore]\n    ", "#[test]\n    #[cfg(any())]\n    "):
                    self.assert_rejected(self.changed(4, path, old, f"{attrs}fn {name}"), f"emitter.test_disabled:{path}::{name}")
            self.assert_rejected(self.changed(4, path, "assert_eq!(value, Some(42));", "debug_assert_eq!(value, Some(42));"), f"emitter.test_assertions:{path}::handle_ok")
            self.assert_rejected(self.changed(4, path, 'token_string.contains("initial")', 'token_string.contains("wrong")'), f"emitter.appended_tokens_test:{path}")

    def test_unrelated_manifest_and_formatting_changes_are_not_fingerprints(self) -> None:
        validate(*self.inputs)
        path = next(iter(MACRO_CONSUMERS.values()))
        changed = self.changed(2, path, "[dependencies]", '# owner documentation\n[dependencies]')
        validate(*changed)
        changed = self.changed(0, None, "Some(value)", "Some( value )")
        validate(*changed)
        for path in LOCAL_COPIES:
            source = self.inputs[4][path]
            first = _active_test(source, "handle_ok", path)
            second = _active_test(source, "handle_err", path)
            # Swap entire function bodies while retaining each name/body pair.
            first_text = source[first.start:first.end + 1]
            second_text = source[second.start:second.end + 1]
            moved = source[:first.start] + second_text + source[first.end + 1:second.start] + first_text + source[second.end + 1:]
            changed = list(self.inputs);changed[4] = {**changed[4], path: moved}
            self.assertNotEqual(tuple(changed), self.inputs)
            validate(*changed)


if __name__ == "__main__":
    unittest.main()
