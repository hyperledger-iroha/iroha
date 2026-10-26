"""Native RP57 schedule/custody controls, with zero fold proof or key generation."""

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import transcript as subject


class FoldTranscript(unittest.TestCase):
    @staticmethod
    def case(tag, count=2, rho=42, masks=tuple(range(43, 75))):
        return subject.committed_case(tag, subject.fixture_inputs(tag, count),
                                      'AInitial' if tag == 0 else 'SigmaBatch', rho, masks)

    @staticmethod
    def script(tag, case, count):
        return subject.fold_script(tag, case['parameter_digest'],
                                   'AInitial' if tag == 0 else 'SigmaBatch', count, case['d'])

    @classmethod
    def setUpClass(cls):
        subject.checked_sources()

    @classmethod
    def tearDownClass(cls):
        subject.checked_sources()

    def test_exact_inventory_and_canonical_native_tables(self):
        subject.checked_sources()
        constants = subject.reference().KATS['poseidon_constants']
        self.assertEqual((constants['full_rounds'], constants['partial_rounds']), (8, 57))
        for tag, field in ((0, 'fp'), (1, 'fq')):
            c = subject.curve(tag)
            table = constants[field]
            self.assertEqual(len(table['round_constants']), 65)
            self.assertEqual(len(table['mds']), 3)
            rows = table['round_constants'] + table['mds']
            self.assertTrue(all(len(row) == 3 for row in rows))
            words = [bytes.fromhex(word) for row in rows for word in row]
            self.assertTrue(all(len(word) == 32 and int.from_bytes(word, 'little') < c.base
                                for word in words))
            native = subject.ROOT / ('crates/iroha_pasta/src/poseidon/rp57_' + field + '.bin')
            self.assertEqual(b''.join(words), native.read_bytes())

    def test_native_two_curve_prelude_known_answers(self):
        expected = (
            '50b9499cbbbbde6c024e4a0ccfe4411c9e6481d1b1e35c126c3636996af9601a',
            '2d3a0dfc3a3f7b376931fae7c76def42a3f5543375b2470c3359a99fe418f44f',
        )
        for tag in (0, 1):
            buffers = (self.case(tag)['prelude'], (), ())
            result = subject.trace_script(tag, buffers)
            subject.check_trace(tag, buffers, result)
            encoded = b''.join(value.to_bytes(32, 'little') for value in result['mapped'])
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), expected[tag])

    def test_canonical_scalar_lanes_and_full_width_maps(self):
        p, q = subject.curve(0).base, subject.curve(0).scalar
        self.assertLess(p, q)
        self.assertLess(q, 2*p)
        for tag in (0, 1):
            c = subject.curve(tag)
            for value in (0, 1, (1 << 128)-1, 1 << 128, c.scalar-1):
                words = subject.scalar_words(tag, value)
                self.assertTrue(all(0 <= word < c.base for word in words))
                self.assertEqual(len(words), 2 if tag == 0 else 1)
                reconstructed = words[0] + ((words[1] << 128) if tag == 0 else 0)
                self.assertEqual(reconstructed, value)
            self.assertEqual(subject.mapped_word(tag, 0), 0)
            self.assertEqual(subject.mapped_word(tag, c.base-1), (c.base-1) % c.scalar)
            for invalid in (-1, c.scalar, True):
                with self.assertRaises(ValueError):
                    subject.scalar_words(tag, invalid)
            for invalid in (-1, c.base, True):
                with self.assertRaises(ValueError):
                    subject.mapped_word(tag, invalid)
        self.assertEqual(subject.scalar_words(0, 0), (0, 0))
        self.assertEqual(subject.scalar_words(0, 1 << 128), (0, 1))
        self.assertEqual([subject.mapped_word(1, v) for v in (p-1, p, p+1, q-1)],
                         [p-1, 0, 1, q-p-1])

    def test_all_nineteen_squeezes_and_exact_primitive_accounting(self):
        for tag in (0, 1):
            for count in (1, 2, 4, 9, 16):
                with self.subTest(tag=tag, count=count):
                    case = self.case(tag, count)
                    buffers = self.script(tag, case, count)
                    result = subject.trace_script(tag, buffers)
                    subject.check_trace(tag, buffers, result)
                    self.assertEqual(len(result['mapped']), 19)
                    self.assertEqual(len(result['edges']), subject.expected_positions(tag, count))
                    self.assertEqual(result['edges'][0][0],
                                     (1 << 64, subject.DOMAIN, case['prelude'][1]))
                    self.assertEqual(len(case['digest_trace']['edges']),
                                     subject.expected_digest_positions(tag, count))
                    self.assertEqual(case['digest_trace']['edges'][0][0],
                                     (1 << 64, subject.INPUT_DOMAIN, len(case['digest_words'])-2))
                    self.assertEqual(tuple(b-a for a, b in zip(result['endpoints'],
                                                               result['endpoints'][1:])),
                                     (1, 1) + (3,) * 16)
        self.assertEqual(subject.expected_positions(1, 4), 54)

    def test_padding_aliases_without_prior_output_tags(self):
        self.assertEqual(subject.padded_blocks(()), ((1, 0),))
        self.assertEqual(subject.padded_blocks((2,)), ((2, 1),))
        self.assertEqual(subject.padded_blocks((2, 3)), ((2, 3), (1, 0)))
        self.assertEqual(subject.padded_blocks((2, 3, 4)), ((2, 3), (4, 1)))
        for tag in (0, 1):
            # A generic native padding alias, outside the fixed committed grammar.
            left = subject.trace_script(tag, ((subject.DOMAIN,), ()))
            right = subject.trace_script(tag, ((subject.DOMAIN, 1),))
            self.assertEqual(left['edges'], right['edges'])
            self.assertEqual(left['state'], right['state'])
            self.assertEqual(left['raw'][-1], right['raw'][-1])
            tagged = subject.trace_script(tag, ((subject.DOMAIN,), (left['raw'][0],)))
            self.assertNotEqual(left['state'], tagged['state'])

    def test_transcript_mutations_and_trace_corruption_are_detected(self):
        for tag in (0, 1):
            inputs = subject.fixture_inputs(tag, 2)
            case = self.case(tag)
            first = case['prelude']
            purpose = 'AInitial' if tag == 0 else 'SigmaBatch'
            original = subject.trace_script(tag, (first, (), ()))
            alternatives = (
                subject.committed_case(tag, tuple(reversed(inputs)), purpose, 42,
                                       tuple(range(43, 75)))['prelude'],
                self.case(tag, rho=43)['prelude'],
                (int.from_bytes(b'pipa-rb1', 'little'),) + first[1:],
            )
            for mutated in alternatives:
                self.assertNotEqual(subject.trace_script(tag, (mutated, (), ()))['mapped'],
                                    original['mapped'])
            reset = subject.trace_script(tag, ((),))
            self.assertNotEqual(original['raw'][1], reset['raw'][0])
            broken = copy.deepcopy(original)
            broken['edges'] = broken['edges'][:-1]
            with self.assertRaises(ValueError):
                subject.check_trace(tag, (first, (), ()), broken)
            broken = copy.deepcopy(original)
            first_input, first_output = broken['edges'][0]
            broken['edges'] = (((first_input[0], first_input[2], first_input[1]), first_output),) + broken['edges'][1:]
            with self.assertRaises(ValueError):
                subject.check_trace(tag, (first, (), ()), broken)
            broken = copy.deepcopy(original)
            broken['raw'] = ((broken['raw'][0] + 1) % subject.curve(tag).base,) + broken['raw'][1:]
            with self.assertRaises(ValueError):
                subject.check_trace(tag, (first, (), ()), broken)

    def test_final_scalar_and_generator_add_no_challenge_or_permutation(self):
        for tag in (0, 1):
            ref, c = subject.reference(), subject.curve(tag)
            buffers = self.script(tag, self.case(tag, 1), 1)
            trace = subject.trace_script(tag, buffers)
            for final_c in (0, 7, c.scalar-1):
                # This unmodified reader absorbs c but reads the final G unabsorbed.
                # No verifier acceptance is claimed for these syntactic messages.
                for suffix in ((c.base-1, 2, 1), (c.base-1, c.base-2, 1), (0, 1, 0)):
                    proof = final_c.to_bytes(32, 'little') + c.encode(suffix)
                    reader = ref.Transcript(c, 2, proof)
                    reader.sponge.state = list(trace['state'])
                    reader.sponge.buffer = []
                    self.assertEqual(reader.scalar(), final_c)
                    self.assertEqual(tuple(reader.sponge.buffer), subject.scalar_words(tag, final_c))
                    self.assertEqual(tuple(reader.sponge.state), trace['state'])
                    self.assertTrue(c.equal(reader.generator(), suffix))
                    reader.finish()
                    self.assertEqual(reader.challenges, [])
                    self.assertEqual(tuple(reader.sponge.state), trace['state'])

    def test_invalid_or_unbounded_schedule_inputs_refuse(self):
        for tag in (-1, 2, True):
            with self.assertRaises(ValueError):
                subject.curve(tag)
        for count in (0, 17, True):
            with self.assertRaises(ValueError):
                subject.fixture_inputs(0, count)
            with self.assertRaises(ValueError):
                subject.expected_positions(0, count)
        for tag in (0, 1):
            c = subject.curve(tag)
            good = subject.fixture_inputs(tag, 2)
            for bad in ((), good[:1], [(good[-1])], (good[-1][:2],)):
                with self.assertRaises(ValueError):
                    subject.input_words(tag, bad)
            point, k, challenges = good[-1]
            for row in ((point, True, challenges), (point, k, challenges[:-1]),
                        (point, k, (0,) + challenges[1:]), ((1, 1), k, challenges),
                        (point, k, (c.scalar,) + challenges[1:])):
                with self.assertRaises(ValueError):
                    subject.input_words(tag, (row,))
            for rho in (-1, c.scalar, True):
                with self.assertRaises(ValueError):
                    subject.commitment_opening(tag, 1, rho)
            for buffers in ((), ((),) * 33, ((0,) * 1025,), ((c.base,),), ((True,),)):
                with self.assertRaises(ValueError):
                    subject.trace_script(tag, buffers)

    def test_diagnostic_k4_rebuild_and_no_new_parameter_authority(self):
        expected = ('49708ed23c310f1c97fae59d99dd7d7eaec88f2f4f077610ce3dda92ad0d0707',
                    '5c0b8093a964af7191df4749a7310c00140ebc9977b118debe5715fda30efa7f')
        for tag in (0, 1):
            c, ref = subject.curve(tag), subject.reference()
            owner = subject.importlib.import_module(ref.__package__ + '.parameters')
            raw = subject.k4_parameter_bytes(tag)
            k6 = owner.Parameters.decode(owner.seed_parameters_k6(c), c, 6)
            self.assertEqual((len(raw), raw[:4]), (1092, b'\x04\x00\x00\x00'))
            self.assertEqual(hashlib.sha256(raw).hexdigest(), expected[tag])
            self.assertEqual(raw[4:4+16*32], b''.join(c.encode(p) for p in k6.g[:16]))
            self.assertEqual(raw[-64:], c.encode(k6.w) + c.encode(k6.u))
            self.assertNotEqual(raw[4+16*32:-64], b''.join(c.encode(p) for p in k6.lagrange[:16]))
            with self.assertRaisesRegex(ValueError, 'no pinned parameter authority'):
                owner.Parameters.decode(raw, c, 4)
            # Independent quadratic Fourier sum checks every reconstructed point;
            # the implementation instead uses a recursive butterfly transform.
            omega, scale = c.omega(4), pow(16, -1, c.scalar)
            for j in range(16):
                direct = c.sum((scale * pow(omega, (-i*j) % 16, c.scalar) % c.scalar, g)
                               for i, g in enumerate(k6.g[:16]))
                self.assertEqual(c.encode(direct), raw[4+(16+j)*32:4+(17+j)*32])

    def test_exact_purposes_context_and_mask_binding(self):
        for tag in (0, 1):
            c, case = subject.curve(tag), self.case(tag)
            purpose = 'AInitial' if tag == 0 else 'SigmaBatch'
            inputs, masks = subject.fixture_inputs(tag, 2), tuple(range(43, 75))
            context = subject.context_words(tag, case['parameter_digest'], purpose, 2)
            self.assertEqual(context[:2], tuple(int.from_bytes(case['parameter_digest'][i:i+16], 'little')
                                                for i in (0, 16)))
            self.assertEqual(context[2:], (int.from_bytes(subject.PURPOSES[purpose][1], 'little'), 2))
            self.assertEqual(case['digest_words'][2:6], context)
            self.assertEqual(case['prelude'][1:5], context)
            self.assertEqual(case['prelude'][5:], case['d'])
            self.assertEqual(case['digest_words'][-32*(2 if tag == 0 else 1):],
                             tuple(word for mask in masks for word in subject.scalar_words(tag, mask)))
            for index in range(32):
                changed = masks[:index] + (masks[index]+1,) + masks[index+1:]
                other = subject.committed_case(tag, inputs, purpose, 42, changed)
                self.assertNotEqual(other['h'], case['h'])
                self.assertNotEqual(other['d'], case['d'])
            changed_rho = self.case(tag, rho=43)
            self.assertEqual(changed_rho['h'], case['h'])
            self.assertNotEqual(changed_rho['d'], case['d'])
            # Each context coordinate separately changes the actual challenge trace.
            original = subject.trace_script(tag, (case['prelude'], (), ()))
            for index in range(1, 5):
                words = list(case['prelude']); words[index] += 1
                self.assertNotEqual(subject.trace_script(tag, (tuple(words), (), ()))['mapped'],
                                    original['mapped'])
            opposite = (case['d'][0], (-case['d'][1]) % c.base)
            changed_d = subject.prelude_words(tag, case['parameter_digest'], purpose, 2, opposite)
            self.assertNotEqual(subject.trace_script(tag, (changed_d, (), ()))['mapped'], original['mapped'])
            for bad_masks in ((), masks[:-1], masks+(1,), list(masks), (-1,)+masks[1:],
                              (c.scalar,)+masks[1:], (True,)+masks[1:]):
                with self.assertRaises(ValueError):
                    subject.input_digest_words(tag, case['parameter_digest'], purpose, inputs, bad_masks)
            for digest in (b'', bytes(31), bytes(33), bytearray(32)):
                with self.assertRaises(ValueError):
                    subject.context_words(tag, digest, purpose, 2)
            for bad in ('WrapperVesta', 'unknown', 1, True,
                        'SigmaBatch' if tag == 0 else 'AInitial'):
                with self.assertRaises(ValueError):
                    subject.context_words(tag, bytes(32), bad, 4)
        for purpose in ('AInitial', 'AContinuation', 'ATerminal'):
            words = subject.context_words(0, bytes(32), purpose, 4)
            self.assertEqual(words[2], int.from_bytes(subject.PURPOSES[purpose][1], 'little'))
        for count in (1, 2, 3, 5):
            with self.assertRaises(ValueError):
                subject.context_words(0, bytes(32), 'ATerminal', count)

    def test_round_nonzero_role_keeps_raw_prelude_and_has_no_retry(self):
        for tag in (0, 1):
            c = subject.curve(tag)
            raw = (0, 0, 0) + (0, 1, c.scalar-1, 2)*4
            actual = subject.role_challenges(tag, raw)
            self.assertEqual(actual[:3], (0, 0, 0))
            if tag == 1:
                # Full-width base conversion precedes nz: p maps to zero,
                # while p+1 maps to one; both yield actual round one.
                for base_word, mapped in ((c.scalar, 0), (c.scalar+1, 1)):
                    converted = subject.mapped_word(tag, base_word)
                    self.assertEqual(converted, mapped)
                    roles = subject.role_challenges(tag, (converted,)*19)
                    self.assertEqual(roles[:3], (mapped,)*3)
                    self.assertEqual(roles[3:], (1,)*16)
            self.assertEqual(actual[3:], (1, 1, c.scalar-1, 2)*4)
            self.assertTrue(all(value != 0 for value in actual[3:]))
            trace = subject.trace_script(tag, self.script(tag, self.case(tag), 2))
            before = copy.deepcopy(trace)
            subject.role_challenges(tag, trace['mapped'])
            self.assertEqual(trace, before)
            for invalid in ((), raw[:-1], raw+(1,), list(raw), (True,)+raw[1:],
                            (-1,)+raw[1:], (c.scalar,)+raw[1:]):
                with self.assertRaises(ValueError):
                    subject.role_challenges(tag, invalid)

    def test_identity_scope_and_ordered_finite_message_refusal(self):
        for tag in (0, 1):
            ref, c = subject.reference(), subject.curve(tag)
            case = self.case(tag, 1)
            identity_input = (((0, 0), 16, (1,)*16),)
            words = subject.input_words(tag, identity_input)
            self.assertEqual(words[:3], (0, 0, 16))
            purpose = 'AInitial' if tag == 0 else 'SigmaBatch'
            identity = subject.committed_case(tag, identity_input, purpose, 42, tuple(range(43, 75)))
            self.assertNotEqual(identity['h'], case['h'])
            with self.assertRaises(ValueError):
                subject.point_words(tag, (0, 0))
            with self.assertRaises(ValueError):
                subject.prelude_words(tag, case['parameter_digest'], purpose, 1, (0, 0))
            with self.assertRaisesRegex(ValueError, 'identity commitment D'):
                subject.commitment_opening(tag, 0, 0)
            with self.assertRaises(ValueError):
                subject.input_words(tag, (((False, 0), 16, (1,)*16),))
            finite = c.encode((c.base-1, 2, 1))
            # Reader-level D,L,R stops only: later messages remain unconsumed and
            # no squeeze occurs. This does not test native computation/cancellation.
            for stop in range(3):
                encoded = finite*stop + bytes(32) + finite*(2-stop)
                reader = ref.Transcript(c, 2, encoded)
                for _ in range(stop):
                    reader.point()
                prior = tuple(reader.sponge.buffer)
                with self.assertRaisesRegex(ValueError, 'identity point'):
                    reader.point()
                self.assertEqual(reader.position, (stop+1)*32)
                self.assertEqual(tuple(reader.sponge.buffer), prior)
                self.assertEqual(reader.challenges, [])
            reader = ref.Transcript(c, 2, bytes(32))
            self.assertTrue(c.equal(reader.generator(), (0, 1, 0)))
            reader.finish()

    def test_source_inventory_mutations_refuse(self):
        manifest = json.loads((subject.HERE / 'transcript_source_manifest.json').read_text())
        for section, name in (('files', 'transcript.py'),
                              ('sources', 'crates/iroha_plonk/src/pcs/ipa/mod.rs'),
                              ('sources', 'crates/iroha_plonk/src/pcs/ipa/generator_prefix.rs'),
                              ('sources', 'crates/iroha_plonk_recursion/src/tests.rs'),
                              ('sources', 'fixtures/native_prover/reference_verifier/parameters.py'),
                              ('sources', 'crates/iroha_pasta/src/params.rs'),
                              ('sources', 'crates/iroha_plonk_recursion/src/fold_tape.rs'),
                              ('sources', 'crates/iroha_plonk_recursion/src/purpose.rs')):
            absent = copy.deepcopy(manifest)
            del absent[section][name]
            with self.assertRaises(ValueError):
                subject.validate_manifest(absent)
            extra = copy.deepcopy(manifest)
            extra[section]['../foreign.py'] = '0' * 64
            with self.assertRaises(ValueError):
                subject.validate_manifest(extra)
            wrong = copy.deepcopy(manifest)
            wrong[section][name] = 'A' * 64
            with self.assertRaises(ValueError):
                subject.validate_manifest(wrong)
        wrong = copy.deepcopy(manifest)
        wrong['schema'] = 'unknown'
        with self.assertRaises(ValueError):
            subject.validate_manifest(wrong)

    def test_changed_or_missing_retained_source_refuses_before_reference_load(self):
        with tempfile.TemporaryDirectory() as temporary:
            here = Path(temporary)
            for name in (*subject.FILES, 'transcript_source_manifest.json'):
                (here / name).write_bytes((subject.HERE / name).read_bytes())
            (here / 'README.md').write_bytes((here / 'README.md').read_bytes() + b'\nchanged\n')
            with patch.object(subject, 'HERE', here):
                with self.assertRaisesRegex(ValueError, 'changed source README.md'):
                    subject.checked_sources()
                (here / 'README.md').unlink()
                with self.assertRaises(FileNotFoundError):
                    subject.checked_sources()

    def test_delegated_generator_prefix_sources_are_not_replaceable(self):
        read_bytes = Path.read_bytes
        for name in ('crates/iroha_plonk/src/pcs/ipa/mod.rs',
                     'crates/iroha_plonk/src/pcs/ipa/generator_prefix.rs',
                     'fixtures/native_prover/reference_verifier/parameters.py',
                     'crates/iroha_pasta/src/params.rs',
                     'crates/iroha_plonk_recursion/src/purpose.rs',
                     'crates/iroha_plonk_recursion/src/fold_tape.rs'):
            def substituted(path, selected=subject.ROOT / name):
                raw = read_bytes(path)
                return raw + b'\nchanged\n' if path == selected else raw
            with self.subTest(source=name):
                with patch.object(Path, 'read_bytes', substituted):
                    with self.assertRaisesRegex(ValueError, 'changed source ' + name):
                        subject.checked_sources()


if __name__ == '__main__':
    unittest.main()
