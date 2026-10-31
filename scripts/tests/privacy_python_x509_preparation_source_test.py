"""Pure Python entry-point contract; native custody and admission are tested in Rust."""
from __future__ import annotations

import ast
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "python/iroha_python/src/iroha_python/tx.py"
METHOD = "prepare_privacy_zk_x509_identity_presentation_action_v1"


def _entry():
    tree = ast.parse(SOURCE.read_text(), filename=str(SOURCE))
    owner = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "TransactionDraft")
    method = next(node for node in owner.body if isinstance(node, ast.FunctionDef) and node.name == METHOD)
    namespace = {}
    # Compile the unchanged production method with postponed annotations. No
    # module named iroha_native is installed, spoofed or exercised by this test.
    module = ast.Module(body=[ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0), method], type_ignores=[])
    exec(compile(ast.fix_missing_locations(module), str(SOURCE), "exec"), namespace)
    return namespace[METHOD], owner


def _draft(builder):
    return SimpleNamespace(_explicit_batch=False, _entries=[], _lane_privacy_attachments=[], to_builder=lambda: builder)


def test_exact_request_returns_identical_opaque_native_owner():
    entry, _ = _entry()
    owner = object()
    calls = []
    builder = SimpleNamespace(**{METHOD: lambda request: calls.append(request) or owner})
    draft = _draft(builder)
    request = b"canonical-proof-free-request"
    prepared = entry(draft, canonical_request_archive=request)
    assert prepared is owner
    assert len(calls) == 1 and calls[0] is request
    draft._entries.append(object())
    assert prepared is owner


@pytest.mark.parametrize("value", [bytearray(b"x"), memoryview(b"x"), "x", None, 1])
def test_no_implicit_conversion_or_mutable_source(value):
    entry, _ = _entry()
    draft = _draft(None)
    draft.to_builder = lambda: pytest.fail("invalid archive crossed native boundary")
    with pytest.raises(TypeError, match="exact immutable bytes"):
        entry(draft, canonical_request_archive=value)


def test_bytes_subclasses_cannot_override_the_preparation_source():
    class SubstitutedBytes(bytes):
        def __bytes__(self):
            return b"different"
    entry, _ = _entry()
    with pytest.raises(TypeError, match="exact immutable bytes"):
        entry(_draft(None), canonical_request_archive=SubstitutedBytes(b"original"))


@pytest.mark.parametrize("field", ["_explicit_batch", "_entries", "_lane_privacy_attachments"])
def test_mixed_action_or_attachment_is_refused_before_native_call(field):
    entry, _ = _entry()
    draft = _draft(None)
    draft.to_builder = lambda: pytest.fail("mixed source crossed native boundary")
    setattr(draft, field, True if field == "_explicit_batch" else [object()])
    with pytest.raises(ValueError, match="otherwise empty"):
        entry(draft, canonical_request_archive=b"request")


def test_native_refusal_propagates_without_a_fallback():
    entry, _ = _entry()
    refusal = RuntimeError("joint-security-reduction-incomplete")
    def refuse(_request):
        raise refusal
    draft = _draft(SimpleNamespace(**{METHOD: refuse}))
    with pytest.raises(RuntimeError) as caught:
        entry(draft, canonical_request_archive=b"request")
    assert caught.value is refusal


def test_retired_statement_and_mutable_signing_api_are_absent():
    entry, owner = _entry()
    names = {node.name for node in owner.body if isinstance(node, ast.FunctionDef)}
    assert "sign_privacy_zk_x509_identity_presentation_action_v1" not in names
    with pytest.raises(TypeError, match="unexpected keyword"):
        entry(_draft(None), canonical_statement_archive=b"retired")
