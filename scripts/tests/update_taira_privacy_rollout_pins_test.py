"""Exact-byte and fail-closed tests for the Taira privacy rollout pin chain."""
import hashlib
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("taira_rollout_pins", ROOT / "scripts/update_taira_privacy_rollout_pins.py")
PINS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PINS)


def sources():
    return tuple((ROOT / path).read_bytes().decode() for path in (PINS.ROLLOUT_PLAN, PINS.BOOTSTRAP_PLAN, PINS.KAGAMI_RELEASE))


class TairaRolloutPinTests(unittest.TestCase):
    def test_checked_in_chain_matches_the_exact_lock(self):
        current = sources()
        self.assertEqual(PINS.update_pins((ROOT / PINS.LOCK).read_bytes(), *current), current)

    def test_lock_change_moves_every_pin_and_nothing_else(self):
        lock = (ROOT / PINS.LOCK).read_bytes() + b"\n"
        current = sources()
        rollout, bootstrap, release = PINS.update_pins(lock, *current)
        self.assertIn(hashlib.sha256(lock).hexdigest(), rollout)
        digest = hashlib.sha256(rollout.encode()).hexdigest()
        self.assertIn(digest, bootstrap)
        self.assertIn(digest, release)
        for before, after in zip(current, (rollout, bootstrap, release)):
            changed = [pair for pair in zip(before.splitlines(), after.splitlines()) if pair[0] != pair[1]]
            self.assertEqual(len(before.splitlines()), len(after.splitlines()))
            self.assertEqual(len(changed), 1)
        self.assertEqual(PINS.update_pins(lock, rollout, bootstrap, release), (rollout, bootstrap, release))

    def test_missing_or_duplicated_pin_rejects(self):
        lock = (ROOT / PINS.LOCK).read_bytes()
        rollout, bootstrap, release = sources()
        for changed in (
            (rollout.replace('"cargo_lock_sha256"', '"lock"'), bootstrap, release),
            (rollout + rollout, bootstrap, release),
            (rollout, bootstrap.replace('"rollout_plan_sha256"', '"plan"'), release),
            (rollout, bootstrap, release.replace("ROLLOUT_PLAN_SHA256_V1", "PLAN_DIGEST")),
        ):
            with self.assertRaises(ValueError):
                PINS.update_pins(lock, *changed)


if __name__ == "__main__":
    unittest.main()
