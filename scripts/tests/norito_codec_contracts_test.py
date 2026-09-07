#!/usr/bin/env python3
"""Current codec gate tests with passing baselines and diagnostic-specific mutations."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import sys
import unittest
from dataclasses import replace
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = SCRIPT_ROOT.parent
OVERLAY_ROOT: Path | None = None


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


GATE = load_module("norito_codec_contract_gate", SCRIPT_ROOT / "check_norito_codec_contracts.py")


class NoritoCodecContractsTest(unittest.TestCase):
    """Mutation results are valid only after the same validator's baseline passes."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.sources = GATE.read_sources(SOURCE_ROOT)
        if OVERLAY_ROOT is not None:
            for path in (OVERLAY_ROOT / "crates").rglob("*.rs"):
                cls.sources[str(path.relative_to(OVERLAY_ROOT))] = path.read_text()

    def assert_rejected(self, validator, sources, diagnostic: str) -> None:
        validator(self.sources)  # Establish this exact validator's valid baseline first.
        self.assertNotEqual(sources, self.sources, "a mutation must change its input")
        with self.assertRaises(GATE.ContractError) as raised:
            validator(sources)
        self.assertEqual(str(raised.exception), diagnostic)

    def mutated(self, path: str, old: str, new: str, owner: str | None = None):
        sources = dict(self.sources)
        original = sources[path]
        start, end = 0, len(original)
        if owner is not None:
            region = GATE.operation(sources, path, owner)
            start, end = region.opening, region.end + 1
        body = original[start:end]
        self.assertIn(old, body, "mutation target must exist in its current owner")
        changed = body.replace(old, new, 1)
        self.assertNotEqual(body, changed)
        sources[path] = original[:start] + changed + original[end:]
        return sources

    def test_unmodified_source_passes_each_current_validator(self) -> None:
        for validator in GATE.VALIDATORS:
            with self.subTest(validator=validator.__name__):
                validator(self.sources)
        GATE.validate(self.sources)

    def test_scoped_contract_mutations_have_exact_diagnostics(self) -> None:
        selection_owner = GATE.role(
            self.sources,
            GATE.COLUMNAR,
            lambda item: "ADAPTIVE_TAG_NCB,ncb" in item.code and "ADAPTIVE_TAG_AOS,aos" in item.code,
            "columnar.selection_owner",
        ).name
        cases = (
            (GATE.validate_columnar, GATE.COLUMNAR, None, "DESC_U64_STR_BOOL: u8 = 0x13", "DESC_U64_STR_BOOL: u8 = 0x12", "columnar.wire_value:DESC_U64_STR_BOOL"),
            (GATE.validate_columnar, GATE.COLUMNAR, None, "ncb_len < aos_len", "ncb_len <= aos_len", "columnar.aos_wins_equal_length"),
            (GATE.validate_columnar, GATE.COLUMNAR, selection_owner, 'feature = "adaptive-telemetry-log"', 'feature = "wrong-telemetry-log"', "columnar.telemetry_feature_isolation"),
            (GATE.validate_columnar, GATE.COLUMNAR, None, '#[cfg(not(feature = "simdutf8-validate"))]', '#[cfg(feature = "simdutf8-validate")]', "columnar.utf8_deterministic_fallback"),
            (GATE.validate_columnar, GATE.COLUMNAR, None, "bytes.len().saturating_sub(prefix.len())", "usize::MAX", "columnar.row_count_bounds"),
            (GATE.validate_encoding, GATE.CORE, "encoded_payload_len", "value.serialize(&mut encoder)?;", "let _ = value;", "encoding.actual_measurement"),
            (GATE.validate_encoding, GATE.CORE, "write_len_prefixed", "encoded_payload_len(value)?", "0", "encoding.field_uses_owned_measurement"),
            (GATE.validate_encoding, GATE.CORE, "serialize_to_writer_exact", "if exact.rejected_write()", "if false", "encoding.public_exact_checks_output"),
            (GATE.validate_encoding, GATE.FIELDS, "write_packed_fields", ".try_reserve_exact(fields.len())", ".try_reserve_exact(0)", "encoding.packed_allocation_bound"),
            (GATE.validate_encoding, GATE.FIELDS, "write_packed_fields", "bits.len() != fields.len().div_ceil(8)", "bits.len() < fields.len().div_ceil(8)", "encoding.packed_bitset_bounds"),
            (GATE.validate_encoding, GATE.FRAMES, "write_frame_with_prefix", "encoded_frame_len(value)?", "0", "encoding.frame_owned_measurement"),
            (GATE.validate_encoding, GATE.FRAMES, None, "return Err(Error::ChecksumMismatch);", "return Ok(());", "encoding.frame_rejects:ChecksumMismatch"),
            (GATE.validate_encoding, GATE.FRAMES, None, "serialize_result?;", "let _ = serialize_result;", "encoding.frame_preserves_child_error"),
            (GATE.validate_codegen, GATE.DERIVE, None, "let _norito_depth = norito::core::EncodeValueDepthGuard::enter()?;", "let _norito_depth = ();", "codegen.binary_depth_guard"),
            (GATE.validate_codegen, GATE.ATTRS, None, 'path.path.is_ident("u8")', 'path.path.is_ident("u16")', "codegen.raw_array_is_u8_only"),
            (GATE.validate_codegen, GATE.DERIVE, None, "rust_discriminant.checked_add(1)", "Some(rust_discriminant)", "codegen.explicit_enum_indices"),
            (GATE.validate_codegen, GATE.DERIVE, "derive_fast_json", "w.ensure_document_depth()?;", "let _ = &w;", "codegen.json_document_depth"),
            (GATE.validate_codegen, GATE.DERIVE, "derive_fast_json", "return Err(norito::json::Error::unknown_field(w.last_key()).into());", "w.skip_value()?;", "codegen.strict_json_rejection"),
            (GATE.validate_codegen, GATE.DERIVE, "derive_fast_json_write", "dyn norito::json::JsonWriteSink", "dyn std::fmt::Write", "codegen.bounded_json_sink"),
            (GATE.validate_codegen, GATE.JSON_WRITER, None, "norito::json::BoundedJsonError::Unsupported", "norito::json::BoundedJsonError::DepthLimit", "codegen.bounded_json_helper_rejection"),
            (GATE.validate_identity, GATE.IDENTITY, "frame_hash", "T::frame_name()", "T::nominal_name()", "identity.frame_hash_uses_declared_name"),
            (GATE.validate_identity, GATE.IDENTITY_DERIVE, None, "::norito::NoritoSchema>::nominal_name()", "::norito::NoritoSchema>::frame_name()", "identity.generic_nominal_arguments"),
            (GATE.validate_identity, GATE.IDENTITY_DERIVE, None, "let mut arguments = Vec::new();", "let mut arguments = Vec::new(); IntoSchema::update_schema_map(map);", "identity.no_structural_marker_expansion"),
        )
        for validator, path, owner, old, new, diagnostic in cases:
            with self.subTest(diagnostic=diagnostic):
                self.assert_rejected(validator, self.mutated(path, old, new, owner), diagnostic)

    def test_comment_cannot_restore_a_removed_type_classifier(self) -> None:
        sources = self.mutated(GATE.ATTRS, 'path.path.is_ident("u8")', 'path.path.is_ident("u16") /* path.path.is_ident("u8") */')
        self.assert_rejected(GATE.validate_codegen, sources, "codegen.raw_array_is_u8_only")

    def test_missing_owner_is_not_hidden_by_a_test_copy(self) -> None:
        sources = dict(self.sources)
        sources[GATE.FIELDS] = "#[cfg(test)]\nmod copied_owner {\n" + sources[GATE.FIELDS] + "\n}\n"
        self.assert_rejected(GATE.validate_encoding, sources, f"owner.operation:{GATE.FIELDS}::write_packed_fields")

    def test_current_runtime_contract_cannot_be_ignored(self) -> None:
        name = GATE.RUNTIME_CONTRACTS[GATE.CODEGEN][0]
        sources = self.mutated(GATE.CODEGEN, f"#[test]\nfn {name}", f"#[test]\n#[ignore]\nfn {name}")
        self.assert_rejected(GATE.validate_runtime_registration, sources, f"runtime.disabled:{GATE.CODEGEN}::{name}")

    def test_private_helper_renaming_preserves_the_gate(self) -> None:
        GATE.validate(self.sources)
        skip = GATE.role(self.sources, GATE.ENCODER, lambda item: "EncoderSink::Counting" in item.code and ".add(" in item.code, "encoding.count_destination_owner")
        owner = GATE.role(self.sources, GATE.CORE, lambda item: f".{skip.name}(" in item.code and "serialize_to_writer_exact(" in item.code, "encoding.measured_emission_owner")
        sources = {path: re.sub(rf"\b{re.escape(owner.name)}\b", "renamed_codec_owned_emission", text) for path, text in self.sources.items()}
        self.assertNotEqual(sources, self.sources)
        GATE.validate(sources)

    def test_lexer_ignores_literals_and_handles_array_return_types(self) -> None:
        source = 'fn frame() -> [u8; 16] { /* { nested /* } */ } */ let _ = r#"fn fake() { }"#; [0; 16] }'
        self.assertEqual([item.name for item in GATE.functions(source)], ["frame"])
        self.assertEqual(GATE.compact('// fake guard\n"another guard"'), "")
        with self.assertRaises(GATE.ContractError) as raised:
            GATE.functions("fn broken() {")
        self.assertEqual(str(raised.exception), "source.unbalanced_body")

    def test_history_verifier_distinguishes_verified_mismatched_and_missing_images(self) -> None:
        payload = b"historical source\n"
        record = {"images": [{"owner": "fixture", "role": "preimage", "git_blob": "recorded-id", "sha256": hashlib.sha256(payload).hexdigest(), "recorded_lines": 1}]}
        baseline = GATE.verify_history(record, lambda _: payload)
        self.assertEqual(baseline[0]["status"], "verified")
        mutated = b"changed historical source\n"
        self.assertNotEqual(payload, mutated)
        self.assertEqual(GATE.verify_history(record, lambda _: mutated)[0]["status"], "mismatch")
        self.assertEqual(GATE.verify_history(record, lambda _: None)[0]["status"], "unverified")
        postimage = {"images": [{**record["images"][0], "role": "postimage", "git_blob": None}]}
        self.assertEqual(GATE.verify_history(postimage, lambda _: self.fail("missing IDs must never be synthesized"))[0]["status"], "unverified")
        empty_record = {"images": []}
        self.assertNotEqual(record, empty_record)
        with self.assertRaises(GATE.ContractError) as raised:
            GATE.verify_history(empty_record, lambda _: payload)
        self.assertEqual(str(raised.exception), "history.images_missing")

    def test_authoritative_size_validator_has_its_own_valid_mutation_baseline(self) -> None:
        budget = load_module("norito_gate_source_budget", SOURCE_ROOT / "scripts/check_source_file_budget.py")
        configuration = budget.load_budget(SOURCE_ROOT / "ci/source_file_budget.json")
        default_limit = budget.limit_for(GATE.COLUMNAR, configuration)
        exceptions = {path: limit for path, limit in configuration.exceptions.items() if path != GATE.COLUMNAR}
        configurations = [configuration, replace(configuration, exceptions=exceptions)]
        current_limit = configuration.exceptions.get(GATE.COLUMNAR, default_limit)
        if current_limit > default_limit + 1:
            configurations.append(replace(configuration, exceptions={**exceptions, GATE.COLUMNAR: current_limit - 1}))
        for current in configurations:
            with self.subTest(columnar_limit=current.exceptions.get(GATE.COLUMNAR)):
                limit = current.exceptions.get(GATE.COLUMNAR, default_limit)
                baseline = {**current.exceptions, GATE.COLUMNAR: limit}
                self.assertEqual(budget.evaluate(baseline, current), [])
                mutation = {**baseline, GATE.COLUMNAR: limit + 1}
                self.assertNotEqual(mutation, baseline)
                findings = budget.evaluate(mutation, current)
                diagnostic = (
                    f"grew from baseline {limit} to {limit + 1} lines"
                    if GATE.COLUMNAR in current.exceptions
                    else f"{limit + 1} lines exceeds the {limit}-line production limit"
                )
                self.assertEqual([(item.path, item.message) for item in findings], [(GATE.COLUMNAR, diagnostic)])
        # Current oversize source is reported separately by the existing CI guard.
        # This validator fixture never treats that unrelated failure as evidence
        # that any codec ownership mutation was detected.


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=Path, default=SOURCE_ROOT)
    parser.add_argument("--overlay", type=Path)
    args, remainder = parser.parse_known_args()
    SOURCE_ROOT = args.source_root
    OVERLAY_ROOT = args.overlay
    unittest.main(argv=[sys.argv[0], *remainder])
