"""Exhaustive stopped-law controls for a bounded deterministic toy construction."""

from collections import Counter
from dataclasses import replace
import hashlib
from itertools import product
import json
from pathlib import Path
import unittest

import model


def experiments():
    """Four fixed parameter sets, each with two different deciding input lists."""
    for p, k, generators, w, auxiliary in (
            (5, 1, (1, 4), 2, 3),
            (5, 2, (1, 1, 1, 1), 2, 3),
            (7, 1, (1, 6), 3, 2),
            (7, 2, (1, 6, 3, 4), 3, 2)):
        shape = model.Shape(p, k, generators, w, auxiliary)
        first = (model.source(shape, (1,) * k), model.source(shape, (2,)))
        second = (model.source(shape, (2,) * k), model.source(shape, (3,)))
        yield shape, (first, second)


class CommittedFold(unittest.TestCase):
    def test_exhaustive_forward_inverse_and_stopped_multiplicities(self):
        """Every input tape and inverse completion occurs once, including failures."""
        total = 0
        for shape, input_pair in experiments():
            p, k = shape.p, shape.k
            expected_tapes = p ** (1 + 2 * k)
            public_laws = []
            for inputs in input_pair:
                with self.subTest(p=p, k=k, inputs=inputs):
                    forward_law = Counter()
                    stops = Counter()
                    successes = 0
                    for tape in product(range(p), repeat=1 + 2 * k):
                        trace = model.forward(shape, inputs, tape)
                        forward_law[trace.public_key()] += 1
                        stops[trace.stop] += 1
                        unseen = tape[len(trace.points):]
                        self.assertEqual(model.inverse(shape, inputs, trace.points, unseen), tape)
                        if trace.stop is None:
                            successes += 1
                            self.assertEqual(model.equation_residual(shape, inputs, trace, tape[1:]), 0)
                            self.assertEqual(trace.generator,
                                             model.decided_generator(shape, trace.challenges))
                    recovered = Counter()
                    inverse_law = Counter()
                    for points, completion in model.inverse_domain(shape):
                        tape = model.inverse(shape, inputs, points, completion)
                        recovered[tape] += 1
                        trace = model.forward(shape, inputs, tape)
                        self.assertEqual(trace.points, points)
                        inverse_law[trace.public_key()] += 1
                    self.assertEqual(len(recovered), expected_tapes)
                    self.assertEqual(set(recovered.values()), {1})
                    self.assertEqual(sum(forward_law.values()), expected_tapes)
                    self.assertEqual(inverse_law, forward_law)
                    for stop in range(1 + 2 * k):
                        # A particular stopped prefix has all unseen masks free.
                        multiplicity = p ** (2 * k - stop)
                        self.assertEqual(stops[stop], (p - 1) ** stop * multiplicity)
                        self.assertTrue(all(count == multiplicity
                                            for key, count in forward_law.items() if key[2] == stop))
                    self.assertEqual(successes, (p - 1) ** (1 + 2 * k))
                    self.assertTrue(all(count == 1 for key, count in forward_law.items()
                                        if key[2] is None))
                    public_laws.append(forward_law)
                    total += expected_tapes
            self.assertNotEqual(*input_pair)
            self.assertEqual(*public_laws)
        self.assertEqual(total, 40_800)

    def test_unused_masks_are_preserved_and_rho_is_solved_last(self):
        shape, (inputs, _) = list(experiments())[1]
        witnesses = []
        for points, completion in model.inverse_domain(shape):
            if not completion:
                continue
            tape = model.inverse(shape, inputs, points, completion)
            self.assertEqual(tape[-len(completion):], completion)
            early = model.inverse(shape, inputs, points, completion, early_rho=True)
            if model.forward(shape, inputs, early).points != points:
                witnesses.append((points, completion, tape, early))
        self.assertTrue(any(points == (0,) for points, *_ in witnesses))
        self.assertTrue(any(len(points) > 1 for points, *_ in witnesses))
        # In particular, a failed left write does not discard its unobserved right mask.
        self.assertTrue(any(len(points) % 2 == 0 for points, *_ in witnesses))

    def test_current_right_offset_precedes_the_left_write(self):
        """Changing only this round's L mask cannot influence its R candidate."""
        for shape, input_pair in experiments():
            for inputs in input_pair:
                fixed_path = (1,) * (1 + 2 * shape.k)
                original = model.inverse(shape, inputs, fixed_path)
                for j in range(shape.k):
                    right_points, left_stops = [], 0
                    for left_mask in range(shape.p):
                        masks = list(original[1:])
                        masks[2 * j] = left_mask
                        # Keep D fixed while changing a digest-bound private mask.
                        rho = (1 - model.digest_offset(shape, inputs, masks)) * pow(shape.w, -1, shape.p) % shape.p
                        trace = model.forward(shape, inputs, (rho, *masks))
                        self.assertEqual(trace.points[:1 + 2 * j], fixed_path[:1 + 2 * j])
                        if trace.stop == 1 + 2 * j:
                            left_stops += 1
                            self.assertEqual(len(trace.points), 2 + 2 * j)
                        else:
                            right_points.append(trace.points[2 + 2 * j])
                    self.assertEqual(left_stops, 1)
                    self.assertEqual(right_points, [1] * (shape.p - 1))

    def test_mask_omission_changes_source_correspondence_but_not_bijection(self):
        """Do not mistake a consistently different digest for a distribution attack."""
        shape, (inputs, _) = list(experiments())[1]
        p, width = shape.p, 1 + 2 * shape.k
        for omitted in range(2 * shape.k):
            changed = mismatched_roundtrips = coordinate_dependencies = 0
            recovered = Counter()
            for tape in product(range(p), repeat=width):
                current = model.digest_limbs(shape, inputs, tape[1:])
                altered = model.digest_limbs(shape, inputs, tape[1:], omit=omitted)
                changed += current != altered
                varied = list(tape[1:])
                varied[omitted] = (varied[omitted] + 1) % p
                coordinate_dependencies += current != model.digest_limbs(shape, inputs, varied)
                trace = model.forward(shape, inputs, tape)
                # Only inverse is changed here: it no longer restores the fixed source tape.
                wrong = model.inverse(shape, inputs, trace.points, tape[len(trace.points):],
                                      omit=omitted)
                mismatched_roundtrips += wrong != tape
                # If BOTH algorithms adopt the other digest, their bijection still holds.
                mutant = model.forward(shape, inputs, tape, omit=omitted)
                self.assertEqual(model.inverse(shape, inputs, mutant.points,
                                               tape[len(mutant.points):], omit=omitted), tape)
            for points, completion in model.inverse_domain(shape):
                tape = model.inverse(shape, inputs, points, completion, omit=omitted)
                self.assertEqual(model.forward(shape, inputs, tape, omit=omitted).points, points)
                recovered[tape] += 1
            self.assertGreater(changed, 0)
            self.assertGreater(coordinate_dependencies, 0)
            self.assertGreater(mismatched_roundtrips, 0)
            self.assertEqual(len(recovered), p ** width)
            self.assertEqual(set(recovered.values()), {1})

    def test_identity_resampling_and_lost_completion_mass_do_not_preserve_law(self):
        shape, (inputs, _) = next(experiments())
        law = Counter(model.forward(shape, inputs, tape).public_key()
                      for tape in product(range(shape.p), repeat=1 + 2 * shape.k))
        self.assertGreater(sum(count for key, count in law.items() if key[2] is not None), 0)
        # Unlimited retry-until-success conditions the result on success and deletes all stops.
        retry_support = {key for key in law if key[2] is None}
        self.assertNotEqual(retry_support, set(law))
        # Keeping one arbitrary completion per stopped path loses precisely its fiber weight.
        one_completion = Counter({key: 1 for key in law})
        self.assertNotEqual(one_completion, law)
        self.assertLess(sum(one_completion.values()), sum(law.values()))

    def test_gamma_mutations_and_private_witness_dependence(self):
        omitted = wrong_sign = private_difference = identity_suffix = 0
        zero_initial_roles = set()
        mapped_zero = 0
        for shape, (first, second) in experiments():
            for points, completion in model.inverse_domain(shape):
                if points[-1] == 0:
                    continue
                first_tape = model.inverse(shape, first, points, completion)
                second_tape = model.inverse(shape, second, points, completion)
                a = model.forward(shape, first, first_tape)
                b = model.forward(shape, second, second_tape)
                self.assertEqual(a.public_key(), b.public_key())
                self.assertEqual(model.equation_residual(shape, first, a, first_tape[1:]), 0)
                omitted += model.equation_residual(shape, first, a, first_tape[1:], gamma_sign=0) != 0
                wrong_sign += model.equation_residual(shape, first, a, first_tape[1:], gamma_sign=1) != 0
                private_difference += (first_tape, a.coefficient) != (second_tape, b.coefficient)
                identity_suffix += a.generator == 0
                zero_initial_roles.update(index for index, word in enumerate(
                    model.initial_challenges(shape, points[0])) if word == 0)
                for j, u in enumerate(a.challenges):
                    prefix = points[:3 + 2 * j]
                    raw = model.toy_hash((23, *shape.context, *prefix), shape.p)
                    if raw == 0:
                        mapped_zero += 1
                        self.assertEqual(u, 1)
        self.assertGreater(omitted, 0)
        self.assertGreater(wrong_sign, 0)
        self.assertGreater(private_difference, 0)
        self.assertGreater(identity_suffix, 0)
        self.assertEqual(zero_initial_roles, {0, 1, 2})
        self.assertGreater(mapped_zero, 0)

    def test_validation_and_first_stop_refuse_malformed_inputs(self):
        shape, (inputs, _) = next(experiments())
        for altered in (replace(shape, p=9), replace(shape, p=True), replace(shape, k=3),
                        replace(shape, k=True), replace(shape, w=0), replace(shape, auxiliary=0),
                        replace(shape, generators=(1,)), replace(shape, generators=(1, 0)),
                        replace(shape, context=(1, 2, 3, 1))):
            with self.assertRaises(ValueError):
                model.validate(altered, inputs)
        for altered in (replace(shape, p=9), replace(shape, p=True), replace(shape, k=3),
                        replace(shape, k=True)):
            with self.assertRaises(ValueError):
                next(model.inverse_domain(altered))
        for invalid in ((), (model.source(shape, ()),),
                        (model.Source((0,), 1), inputs[1]),
                        (replace(inputs[0], claim=(inputs[0].claim + 1) % shape.p), inputs[1])):
            with self.assertRaises(ValueError):
                model.validate(shape, invalid)
        with self.assertRaisesRegex(ValueError, 'source width must be one'):
            model.validate(shape, (inputs[0], model.source(shape, ())))
        for tape in ((0, 0), (0, 0, 0, 0), (True, 0, 0), (5, 0, 0), (-1, 0, 0)):
            with self.assertRaises(ValueError):
                model.forward(shape, inputs, tape)
        for path in ((), (1,), (0, 1), (1, 0, 1), (1, 0, 0), (1, True, 1), (1, 5, 1),
                     (1, 1, 1, 1)):
            with self.assertRaises(ValueError):
                model.path_shape(shape, path)
        for path, completion in (((0,), ()), ((1, 0), ()), ((0,), (0, 5))):
            with self.assertRaises(ValueError):
                model.inverse(shape, inputs, path, completion)
        with self.assertRaises(ValueError):
            model.equation_residual(shape, inputs, model.forward(shape, inputs,
                                    model.inverse(shape, inputs, (0,), (0, 0))), (0, 0))

    def test_exact_control_and_correspondence_inventory(self):
        here = Path(__file__).resolve().parent
        root = here.parents[1]
        manifest = json.loads((here / 'source_manifest.json').read_text())
        self.assertEqual(set(manifest), {'schema', 'files', 'sources'})
        self.assertEqual(manifest['schema'], 'kagemusha.committed-fold.scalar-control.v1')
        self.assertEqual(set(manifest['files']), {'model.py', 'test_model.py', 'README.md'})
        self.assertEqual(set(manifest['sources']), {
            'crates/iroha_plonk_recursion/src/accumulation.rs',
            'crates/iroha_plonk_recursion/src/accumulation_circuit.rs',
            'crates/iroha_plonk_recursion/src/fold_tape.rs',
            'crates/iroha_plonk_recursion/src/purpose.rs',
            'crates/iroha_plonk_recursion/src/claim.rs',
            'crates/iroha_plonk/src/transcript/mod.rs',
            'crates/iroha_plonk/src/transcript/pipa_r.rs',
            'crates/iroha_plonk/src/pcs/ipa/mod.rs',
            'specs/kagemusha_c12_composition_v1.md',
        })
        for base, entries in ((here, manifest['files']), (root, manifest['sources'])):
            for name, digest in entries.items():
                self.assertEqual(hashlib.sha256((base / name).read_bytes()).hexdigest(), digest, name)


if __name__ == '__main__':
    unittest.main()
