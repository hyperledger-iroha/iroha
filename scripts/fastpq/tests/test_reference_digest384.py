"""Tests for the independent, standard-library FASTPQ Digest384 oracle."""

from dataclasses import replace
from pathlib import Path
from tempfile import TemporaryDirectory
import contextlib
import io
import unittest

from scripts.fastpq import reference_digest384 as reference


class ReferenceDigest384Tests(unittest.TestCase):
    def test_sha3_parameter_asset_and_all_canonical_lane_words(self) -> None:
        self.assertEqual(reference.parameter_asset_digest().hex(), reference.PARAMETER_SHA3_256)
        initial_states = set()
        for lane in range(reference.LANES):
            initial, constants = reference.lane_parameters(lane)
            self.assertEqual(len(initial), 3)
            self.assertEqual(len(constants), 65)
            self.assertTrue(all(0 <= word < reference.MODULUS for word in initial))
            self.assertTrue(all(0 <= word < reference.MODULUS for row in constants for word in row))
            initial_states.add(initial)
        self.assertEqual(len(initial_states), reference.LANES)

    def test_pinned_vectors_match_independent_reference(self) -> None:
        self.assertEqual(reference.FIXTURE.read_text(encoding="ascii"), reference.fixture_text())
        self.assertEqual(len(reference.cases()), 31)

    def test_framing_preserves_empty_fields_splits_and_trailing_zeros(self) -> None:
        domain = reference.Domain()
        inputs = [(), (b"",), (b"abc", b"def"), (b"abcdef",), (b"abcdef\0",)]
        self.assertEqual(len({reference.digest(domain, fields) for fields in inputs}), len(inputs))

    def test_every_domain_coordinate_is_bound(self) -> None:
        domain = reference.Domain()
        original = reference.digest(domain, (b"abc",))
        for field in ("catalog", "protocol", "profile", "role", "phase"):
            with self.subTest(field=field):
                changed = replace(domain, **{field: b"changed"})
                self.assertNotEqual(reference.digest(changed, (b"abc",)), original)
        for field in ("level", "index", "counter"):
            with self.subTest(field=field):
                changed = replace(domain, **{field: 2**64 - 1})
                self.assertNotEqual(reference.digest(changed, (b"abc",)), original)

    def test_invalid_lanes_coordinates_and_payload_types_fail(self) -> None:
        for lane in (-1, 6, True):
            with self.subTest(lane=lane), self.assertRaises(ValueError):
                reference.lane_parameters(lane)
        for value in (-1, 2**64, True):
            with self.subTest(value=value), self.assertRaises(ValueError):
                reference.digest(replace(reference.Domain(), level=value), ())
        with self.assertRaises(ValueError):
            reference.digest(reference.Domain(), ("not-bytes",))

    def test_default_check_is_read_only_and_explicit_write_is_reproducible(self) -> None:
        with TemporaryDirectory() as directory:
            fixture = Path(directory) / "vectors.tsv"
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(reference.main(["--fixture", str(fixture), "--write"]), 0)
                original = fixture.read_bytes()
                self.assertEqual(reference.main(["--fixture", str(fixture)]), 0)
            self.assertEqual(fixture.read_bytes(), original)
            fixture.write_bytes(original.replace(b"no-fields", b"changed-id", 1))
            corrupted = fixture.read_bytes()
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                reference.main(["--fixture", str(fixture)])
            self.assertEqual(fixture.read_bytes(), corrupted)


if __name__ == "__main__":
    unittest.main()
