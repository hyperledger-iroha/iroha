"""Run actual public-harness tests with explicitly supplied normal publisher pins."""
import argparse
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from zk_resource_harness_native_fixture import NormalPublicationFixture
import zk_resource_harness_test as cases


def main():
    """Configure the test seam without discovering or promoting native artifacts."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('source-root', 'bridge-root', 'module-sha256', 'provenance-sha256',
                 'source-manifest-sha256', 'native-target'):
        parser.add_argument('--'+name, required=True)
    parser.add_argument('--test', action='append', default=[],
                        help='Exact NativeRunTests method; omitted selects the whole class')
    args = parser.parse_args()
    if not sys.platform.startswith('linux'):
        parser.error('this explicit normal-publisher fixture invocation requires Linux')
    fixture = NormalPublicationFixture(args.source_root, args.bridge_root, args.module_sha256,
                                      args.provenance_sha256, args.source_manifest_sha256,
                                      args.native_target)
    cases.NativeRunTests.publication_fixture = fixture
    available = unittest.defaultTestLoader.getTestCaseNames(cases.NativeRunTests)
    selected = args.test or available
    if not selected or len(set(selected)) != len(selected) or any(name not in available for name in selected):
        parser.error('test selection must be unique exact actual NativeRunTests names')
    suite = unittest.TestSuite(cases.NativeRunTests(name) for name in selected)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() and result.testsRun == len(selected) and not result.skipped else 1


if __name__ == '__main__':
    raise SystemExit(main())
