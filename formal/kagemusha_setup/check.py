"""Run bounded setup and fixed-Poseidon algebra controls with retained outputs."""
import argparse
import importlib
import json
from pathlib import Path
import sys
import time
import unittest

from . import custody


def main():
    """One fresh output tree; preserve partial artifacts and total failures."""
    custody.require(sys.flags.optimize == 0, 'unoptimized setup controls only')
    custody.require(sys.dont_write_bytecode, 'bytecode-free setup controls (-B)')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    manifest_raw = (custody.HERE/'source_manifest.json').read_bytes()
    custody.checked_sources()
    generated_before = custody.observed_generated_data()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    start = time.monotonic()
    (output/'started.json').write_text(json.dumps(
        {'argv': sys.argv, 'manifest_sha256': custody.sha(manifest_raw),
         'source_checked_before_imports': True, 'generated_data_before': generated_before,
         'scope': 'Diagnostic controls only'},
        indent=2, sort_keys=True)+'\n')
    record = {'success': False, 'C12_closed': False, 'current_setup_qualified': False,
              'planned_toy_k6_proofs': 14, 'planned_adapted_toy_k6_proofs': 6, 'native_proofs': 0, 'large_proofs': 0}
    proof_classes = []
    observation_error = None
    try:
        algebra = custody.validate_algebra()
        (output/'algebra.json').write_text(json.dumps(algebra, indent=2, sort_keys=True)+'\n')
        suites = []
        selected = []
        modules = [('test_custody', 'CustodyTests'), ('test_finite', 'FreshTarget'),
                   ('test_preimage', 'PreimageTests'),
                   ('test_raw', 'AdapterTests'), ('test_seam', 'SeamTests'),
                   ('test_proof', 'ProofControls'), ('test_bounded', 'ProducerTests'),
                   ('test_requests', 'OwnerTests'), ('test_generic', 'GenericKnownLog'),
                   ('test_compact_source', 'CompactSourceControls'),
                   ('test_file_custody', 'FileCustodyTests'),
                   ('test_reference_custody', 'ReferenceCustodyTests')]
        for name, class_name in modules:
            module = importlib.import_module(__package__+'.'+name)
            if name in ('test_seam', 'test_proof', 'test_bounded', 'test_requests', 'test_generic',
                        'test_compact_source'):
                module.OUTPUT = output/name
                module.OUTPUT.mkdir()
            klass = getattr(module, class_name)
            if name in ('test_proof', 'test_requests', 'test_generic'):
                proof_classes.append(klass)
            names = unittest.defaultTestLoader.getTestCaseNames(klass)
            selected.extend(name+'.'+class_name+'.'+case for case in names)
            suites.append(unittest.defaultTestLoader.loadTestsFromTestCase(klass))
        shape_class = importlib.import_module(__package__+'.test_requests').RequestShapeTests
        shape_names = unittest.defaultTestLoader.getTestCaseNames(shape_class)
        selected.extend('test_requests.RequestShapeTests.'+name for name in shape_names)
        suites.append(unittest.defaultTestLoader.loadTestsFromTestCase(shape_class))
        custody.require(len(selected) == 116 and len(set(selected)) == 116,
                        'exact 116 maintained controls')
        result = unittest.TextTestRunner(verbosity=2).run(unittest.TestSuite(suites))
        record.update(tests=result.testsRun, failures=len(result.failures),
                      errors=len(result.errors), skipped=len(result.skipped), selected=selected)
        custody.checked_sources()
        custody.require((custody.HERE/'source_manifest.json').read_bytes() == manifest_raw,
                        'source manifest changed during controls')
        custody.require(custody.observed_generated_data() == generated_before,
                        'generated DATA changed during controls')
        record['source_pins_unchanged'] = True
        record['success'] = (result.wasSuccessful() and result.testsRun == 116
                             and not result.skipped and
                             sum(c.verified_toy_proofs for c in proof_classes) == 14 and
                             sum(getattr(c, 'verified_adapted_toy_proofs', 0)
                                 for c in proof_classes) == 6)
        record['scope'] = ('Source-map ideal raw-RO setup, bounded k0/k2/k6 parameter ownership, and fourteen k6 fixed-Poseidon algebra cases; '
                           'Six further k6 proofs use explicit test-only x1=0, total-role-zero and identity-G adapters and are not fixed-Poseidon evidence. '
                           'Common proof coins within a curve are counterfactual diagnostics. '
                           'Sequential request/replay, live setup closure and bounded file/reference custody controls are diagnostic only. '
                           'Seven current compact-source algebra controls require explicit small public rows; no native source synthesis is inferred. '
                           'No adaptive privacy theorem, concrete hash, current authority or C12 claim.')
    except BaseException as error:
        record['success'] = False
        record['exception'] = type(error).__name__+': '+str(error)
        raise
    finally:
        record['generated_data_before'] = generated_before
        record['generated_data_after'] = None
        try:
            record['generated_data_after'] = custody.observed_generated_data()
        except BaseException as error:
            # Preserve a pending primary exception while retaining this refusal.
            observation_error = error
            record['generated_data_after_error'] = type(error).__name__+': '+str(error)
        record['generated_data_unchanged'] = (
            observation_error is None and record['generated_data_after'] == generated_before)
        record['success'] = record['success'] and record['generated_data_unchanged']
        record['generated_data_qualified'] = False
        record['verified_toy_k6_proofs'] = sum(c.verified_toy_proofs for c in proof_classes)
        record['verified_adapted_toy_k6_proofs'] = sum(
            getattr(c, 'verified_adapted_toy_proofs', 0) for c in proof_classes)
        record['elapsed_seconds'] = time.monotonic()-start
        record['manifest_sha256'] = custody.sha(manifest_raw)
        (output/'result.json').write_text(json.dumps(record, indent=2, sort_keys=True)+'\n')
    if observation_error is not None:
        raise observation_error
    return 0 if record['success'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
