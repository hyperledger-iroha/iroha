"""Focused coverage for the ISO profile catalog ``include_str!`` boundary."""

import hashlib
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_xsd_fixture_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_xsd_fixture_verify_include", SCRIPT_PATH)
VERIFIER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(VERIFIER)

CATALOG_BYTES = b'\n[{"id":"minimal-profile"}]\n'
CHECKED_IN_CATALOG_BYTES = 12_862
CHECKED_IN_CATALOG_SHA256 = (
    "8f8413ceede90c6aef5566bd0441b086bd7560e0c5e87c1df1b28a2edf1b3990"
)


def write_include(root: Path, include_path: str = "catalog.json") -> tuple[Path, Path]:
    source = root / "profiles.rs"
    asset = root / "catalog.json"
    source.write_text(
        'const DEFAULT_PROFILES_JSON: &str = include_str!("'
        + include_path
        + '");\n',
        encoding="utf-8",
    )
    return source, asset


class IsoXsdProfileCatalogIncludeTest(unittest.TestCase):
    def test_checked_in_source_includes_exact_catalog_asset(self):
        source = VERIFIER.DEFAULT_PROFILE_CATALOG
        asset = source.with_name("default_profiles.json")

        profiles, source_digest, json_digest, material_path = (
            VERIFIER._load_profile_catalog(source)
        )

        self.assertEqual(len(profiles), 5)
        self.assertEqual(source_digest, VERIFIER.sha256_hex(source.read_bytes()))
        catalog_bytes = asset.read_bytes()
        self.assertEqual(len(catalog_bytes), CHECKED_IN_CATALOG_BYTES)
        self.assertEqual(
            hashlib.sha256(catalog_bytes).hexdigest(), CHECKED_IN_CATALOG_SHA256
        )
        self.assertEqual(json_digest, CHECKED_IN_CATALOG_SHA256)
        self.assertEqual(material_path, asset)

    def test_include_loads_exact_bounded_json_bytes(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)

            profiles, source_digest, json_digest, material_path = (
                VERIFIER._load_profile_catalog(source)
            )

            self.assertEqual(profiles, [{"id": "minimal-profile"}])
            self.assertEqual(source_digest, VERIFIER.sha256_hex(source.read_bytes()))
            self.assertEqual(json_digest, VERIFIER.sha256_hex(CATALOG_BYTES))
            self.assertEqual(material_path, asset)
            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "summary_out must not reuse profile catalog JSON include path",
            ):
                VERIFIER._reject_summary_output_input_alias(
                    asset,
                    (("profile catalog JSON include", material_path),),
                )

    def test_include_path_must_be_one_relative_json_filename(self):
        for include_path in (
            "../catalog.json",
            "nested/catalog.json",
            "/tmp/catalog.json",
            "catalog.txt",
            " catalog.json",
            r"..\catalog.json",
            r"..\\catalog.json",
            "catalog%2fescape.json",
        ):
            with self.subTest(include_path=include_path):
                with tempfile.TemporaryDirectory() as raw_root:
                    source, _asset = write_include(Path(raw_root), include_path)
                    with self.assertRaisesRegex(
                        VERIFIER.FixtureManifestError,
                        "must use one raw string or one canonical relative JSON include",
                    ):
                        VERIFIER._load_profile_catalog(source)

    def test_comment_hidden_include_declaration_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                source.read_text(encoding="utf-8")
                + 'const DEFAULT_PROFILES_JSON: &str = include_str!('
                '/* hidden */ "second.json");\n',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "exactly one active DEFAULT_PROFILES_JSON declaration",
            ):
                VERIFIER._load_profile_catalog(source)

    def test_comment_inside_only_include_is_not_accepted(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                'const DEFAULT_PROFILES_JSON: &str = include_str!('
                '/* hidden */ "catalog.json");\n',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "must use one raw string or one canonical relative JSON include",
            ):
                VERIFIER._load_profile_catalog(source)

    def test_quote_char_literals_cannot_hide_cfg_duplicate_declarations(self):
        for quote_declaration in (
            "const QUOTE: char = '\"';",
            "const QUOTE: u8 = b'\"';",
        ):
            with self.subTest(quote_declaration=quote_declaration):
                with tempfile.TemporaryDirectory() as raw_root:
                    source, asset = write_include(Path(raw_root), "first.json")
                    asset.write_bytes(CATALOG_BYTES)
                    source.write_text(
                        '#[cfg(unix)]\nconst DEFAULT_PROFILES_JSON: &str = '
                        'include_str!("first.json");\n'
                        + quote_declaration
                        + '\n#[cfg(not(unix))]\nconst DEFAULT_PROFILES_JSON: &str = '
                        'include_str!("second.json");\n',
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        VERIFIER.FixtureManifestError,
                        "exactly one active DEFAULT_PROFILES_JSON declaration",
                    ):
                        VERIFIER._load_profile_catalog(source)

    def test_quote_char_literal_before_sole_declaration_is_accepted(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                "const QUOTE: char = '\"';\n"
                'const DEFAULT_PROFILES_JSON: &str = include_str!("catalog.json");\n',
                encoding="utf-8",
            )

            profiles, _source_digest, _json_digest, material_path = (
                VERIFIER._load_profile_catalog(source)
            )

            self.assertEqual(profiles, [{"id": "minimal-profile"}])
            self.assertEqual(material_path, asset)

    def test_alternate_identifier_bindings_are_rejected(self):
        alternatives = (
            'static DEFAULT_PROFILES_JSON: &str = include_str!("second.json");',
            "use alternate::VALUE as DEFAULT_PROFILES_JSON;",
            "alt!(DEFAULT_PROFILES_JSON);",
            "alt!(prefix, nested(DEFAULT_PROFILES_JSON));",
            "macro_rules! alt { () => { const DEFAULT_PROFILES_JSON: &str = \"x\"; } }",
        )
        for alternative in alternatives:
            with self.subTest(alternative=alternative):
                with tempfile.TemporaryDirectory() as raw_root:
                    source, asset = write_include(Path(raw_root))
                    asset.write_bytes(CATALOG_BYTES)
                    source.write_text(
                        source.read_text(encoding="utf-8") + alternative + "\n",
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        VERIFIER.FixtureManifestError,
                        "(may use DEFAULT_PROFILES_JSON only in its canonical declaration|"
                        "exactly one active DEFAULT_PROFILES_JSON declaration)",
                    ):
                        VERIFIER._load_profile_catalog(source)

    def test_exact_runtime_read_is_allowed(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                source.read_text(encoding="utf-8")
                + "fn catalog() { let _ = json::from_json(DEFAULT_PROFILES_JSON); }\n",
                encoding="utf-8",
            )

            profiles, _source_digest, _json_digest, material_path = (
                VERIFIER._load_profile_catalog(source)
            )

            self.assertEqual(profiles, [{"id": "minimal-profile"}])
            self.assertEqual(material_path, asset)

    def test_tuple_destructuring_cannot_shadow_runtime_catalog(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                source.read_text(encoding="utf-8")
                + "mod runtime { fn parse() { "
                + 'let (DEFAULT_PROFILES_JSON,) = ("alternate",); '
                + "let _ = json::from_json(DEFAULT_PROFILES_JSON); } }\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "may use DEFAULT_PROFILES_JSON only in its canonical declaration",
            ):
                VERIFIER._load_profile_catalog(source)

    def test_raw_and_include_declarations_are_rejected_as_duplicates(self):
        with tempfile.TemporaryDirectory() as raw_root:
            source, asset = write_include(Path(raw_root))
            asset.write_bytes(CATALOG_BYTES)
            source.write_text(
                source.read_text(encoding="utf-8")
                + 'const DEFAULT_PROFILES_JSON: &str = r#"[]"#;\n',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                VERIFIER.FixtureManifestError,
                "exactly one active DEFAULT_PROFILES_JSON declaration",
            ):
                VERIFIER._load_profile_catalog(source)

    def test_include_reuses_strict_regular_file_and_byte_bounds(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            source, asset = write_include(root)
            with self.assertRaisesRegex(VERIFIER.FixtureManifestError, "does not exist"):
                VERIFIER._load_profile_catalog(source)

            asset.write_bytes(b"\xff")
            with self.assertRaisesRegex(VERIFIER.FixtureManifestError, "not valid UTF-8"):
                VERIFIER._load_profile_catalog(source)

            asset.write_bytes(CATALOG_BYTES)
            old_limit = VERIFIER.MAX_PROFILE_CATALOG_BYTES
            try:
                VERIFIER.MAX_PROFILE_CATALOG_BYTES = len(CATALOG_BYTES) - 1
                with self.assertRaisesRegex(VERIFIER.FixtureManifestError, "exceeds"):
                    VERIFIER._load_profile_catalog(source)
            finally:
                VERIFIER.MAX_PROFILE_CATALOG_BYTES = old_limit

            target = root / "target.json"
            target.write_bytes(CATALOG_BYTES)
            asset.unlink()
            try:
                os.symlink(target, asset)
            except (NotImplementedError, OSError):
                self.skipTest("symbolic links are unavailable")
            with self.assertRaisesRegex(VERIFIER.FixtureManifestError, "must not be a symlink"):
                VERIFIER._load_profile_catalog(source)


if __name__ == "__main__":
    unittest.main()
