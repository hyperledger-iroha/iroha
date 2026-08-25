#!/usr/bin/env python3
"""Authenticate the shared fixed emitter extension used by derive crates."""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MACRO_OWNER = "crates/iroha_derive/src/lib.rs"
MACRO_START = "/// Define the private diagnostic-emitter convenience trait"
MACRO_END = "/// Helper macro to expand FFI functions"
EXPECTED_MACRO_SHA256 = "19702ddcf0792954266cc58405074e1cb09c5f4cf710257c49d6903d51ec7b5e"
MAX_MACRO_LINES = 85
EXPECTED_LOCK_SHA256 = "ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7"
OPENING_BLOBS = (
    "9b2f6cfe74efa0aa8f96563195766a8a16b17739",
    "4060669431f999b95419e86da9ce82f9fa2b8cf9",
)
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
LOCAL_COPIES = {
    "crates/iroha_ffi/proc_macro/src/emitter_ext.rs": (
        "43af480793302e0749bccd6dcd864686e53af2f73c60d2e909f03c7465918dcf"
    ),
    "crates/iroha_telemetry_derive/src/emitter_ext.rs": (
        "fe9ebf34f8e02ccf103ffa37aae7d5923df0f737cdd4db0f48f5ebb29a572e6c"
    ),
}
LOCAL_OWNERS = {
    "crates/iroha_ffi/proc_macro/src/lib.rs": "crates/iroha_ffi/proc_macro/src/emitter_ext.rs",
    "crates/iroha_telemetry_derive/src/lib.rs": (
        "crates/iroha_telemetry_derive/src/emitter_ext.rs"
    ),
}


class GuardError(AssertionError):
    """Raised when the authenticated emitter-sharing contract drifts."""


def _sha256(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def _production(text: str) -> str:
    return text.split("#[cfg(test)]", 1)[0]


def _rust_shape(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    text = re.sub(r"//[^\n]*", "", text)
    return "".join(text.split())


def _opening_shape() -> str:
    shapes = []
    for blob in OPENING_BLOBS:
        data = subprocess.run(
            ["git", "cat-file", "-p", blob],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout.decode()
        shapes.append(_rust_shape(_production(data)))
    if len(set(shapes)) != 1:
        raise GuardError("opening emitter implementations are not equivalent")
    return shapes[0]


def _test_ids(text: str) -> tuple[str, ...]:
    suffix = text.split("#[cfg(test)]", 1)
    if len(suffix) != 2:
        raise GuardError("local emitter lost its test module")
    return tuple(re.findall(r"#\[test\]\s*fn\s+([A-Za-z_][A-Za-z0-9_]*)", suffix[1]))


def _macro_block(owner: str) -> str:
    try:
        start = owner.index(MACRO_START)
        end = owner.index(MACRO_END, start)
    except ValueError as error:
        raise GuardError("fixed emitter macro block markers drifted") from error
    return owner[start:end]


def validate(
    owner: str,
    consumers: dict[str, str],
    manifests: dict[str, str],
    internal_imports: dict[str, str],
    local_copies: dict[str, str],
    local_owners: dict[str, str],
    deleted_present: tuple[str, ...],
    cargo_lock: str,
) -> None:
    block = _macro_block(owner)
    if _sha256(block) != EXPECTED_MACRO_SHA256:
        raise GuardError("fixed emitter macro implementation drifted")
    if len(block.splitlines()) > MAX_MACRO_LINES:
        raise GuardError("fixed emitter macro exceeded its readable line ceiling")
    if max(map(len, block.splitlines())) > 100:
        raise GuardError("fixed emitter macro contains a packed source line")
    required = (
        "#[proc_macro]",
        "pub fn define_emitter_ext",
        "if !input.is_empty()",
        "define_emitter_ext! accepts no input",
        "trait EmitterExt",
        "impl EmitterExt for manyhow::Emitter",
        "manyhow::ToTokensError::to_tokens",
    )
    if any(token not in block for token in required):
        raise GuardError("fixed emitter macro lost a required typed operation")
    forbidden = ("$body", "$assert", "macro_rules!", "Action", "Step")
    if any(token in block for token in forbidden):
        raise GuardError("fixed emitter macro became a body/action/assertion DSL")

    if deleted_present:
        raise GuardError(f"duplicate emitter modules returned: {deleted_present}")
    for path, manifest_path in MACRO_CONSUMERS.items():
        source = consumers[path]
        if source.count("iroha_derive::define_emitter_ext!();") != 1:
            raise GuardError(f"{path} must invoke the fixed emitter macro exactly once")
        if "mod emitter_ext;" in source or "iroha_derive_primitives::EmitterExt" in source:
            raise GuardError(f"{path} restored a second emitter implementation")
        manifest = manifests[manifest_path]
        if manifest.count("iroha_derive = { workspace = true }") != 1:
            raise GuardError(f"{manifest_path} lost its pre-existing macro dependency")
        if "iroha_derive_primitives =" in manifest:
            raise GuardError(f"{manifest_path} added a lockfile-changing dependency edge")

    for path, source in internal_imports.items():
        if "EmitterExt" not in source:
            raise GuardError(f"{path} lost access to the generated extension trait")
        if "emitter_ext::EmitterExt" in source or "iroha_derive_primitives::EmitterExt" in source:
            raise GuardError(f"{path} bypasses the fixed crate-root definition")

    opening_shape = _opening_shape()
    for path, expected_digest in LOCAL_COPIES.items():
        source = local_copies[path]
        if _sha256(source) != expected_digest:
            raise GuardError(f"{path} local compatibility copy drifted")
        if _rust_shape(_production(source)) != opening_shape:
            raise GuardError(f"{path} local production behavior differs from the opening sources")
        if _test_ids(source) != EXPECTED_TEST_IDS:
            raise GuardError(f"{path} local test identities or order drifted")
    if "ManyhowError" not in local_copies["crates/iroha_ffi/proc_macro/src/emitter_ext.rs"]:
        raise GuardError("manyhow diagnostic coverage was lost")
    if "syn::Error" not in local_copies["crates/iroha_telemetry_derive/src/emitter_ext.rs"]:
        raise GuardError("syn diagnostic coverage was lost")
    for owner_path, local_path in LOCAL_OWNERS.items():
        source = local_owners[owner_path]
        if source.count("mod emitter_ext;") != 1 or source.count("emitter_ext::EmitterExt") != 1:
            raise GuardError(f"{owner_path} lost its package-local compatibility module")
        if local_path not in LOCAL_COPIES:
            raise GuardError(f"{owner_path} maps to an unknown compatibility module")

    if hashlib.sha256(cargo_lock.encode()).hexdigest() != EXPECTED_LOCK_SHA256:
        raise GuardError("Cargo.lock changed")


def current_inputs() -> tuple[
    str,
    dict[str, str],
    dict[str, str],
    dict[str, str],
    dict[str, str],
    dict[str, str],
    tuple[str, ...],
    str,
]:
    consumers = {path: (ROOT / path).read_text() for path in MACRO_CONSUMERS}
    manifests = {
        path: (ROOT / path).read_text() for path in dict.fromkeys(MACRO_CONSUMERS.values())
    }
    return (
        (ROOT / MACRO_OWNER).read_text(),
        consumers,
        manifests,
        {path: (ROOT / path).read_text() for path in INTERNAL_IMPORTS},
        {path: (ROOT / path).read_text() for path in LOCAL_COPIES},
        {path: (ROOT / path).read_text() for path in LOCAL_OWNERS},
        tuple(path for path in DELETED_COPIES if (ROOT / path).exists()),
        (ROOT / "Cargo.lock").read_text(),
    )


class SharedEmitterSourceTest(unittest.TestCase):
    def test_current_source(self) -> None:
        validate(*current_inputs())

    def test_mutations_fail_closed(self) -> None:
        inputs = current_inputs()
        owner, consumers, manifests, imports, copies, local_owners, deleted, lock = inputs
        first_consumer = next(iter(MACRO_CONSUMERS))
        first_manifest = MACRO_CONSUMERS[first_consumer]
        first_copy = next(iter(LOCAL_COPIES))
        mutations = (
            (owner.replace("Some(value)", "None", 1), consumers, manifests, imports, copies, local_owners, deleted, lock),
            (owner, {**consumers, first_consumer: consumers[first_consumer].replace("define_emitter_ext", "wrong", 1)}, manifests, imports, copies, local_owners, deleted, lock),
            (owner, consumers, {**manifests, first_manifest: manifests[first_manifest] + "\niroha_derive_primitives = { workspace = true }\n"}, imports, copies, local_owners, deleted, lock),
            (owner, consumers, manifests, {**imports, next(iter(imports)): ""}, copies, local_owners, deleted, lock),
            (owner, consumers, manifests, imports, {**copies, first_copy: copies[first_copy].replace("Some(value)", "None", 1)}, local_owners, deleted, lock),
            (owner, consumers, manifests, imports, copies, local_owners, (DELETED_COPIES[0],), lock),
            (owner, consumers, manifests, imports, copies, local_owners, deleted, lock + "\n"),
        )
        for mutation in mutations:
            with self.subTest(mutation=str(mutation[0])[:48]):
                with self.assertRaises(GuardError):
                    validate(*mutation)


if __name__ == "__main__":
    unittest.main()
