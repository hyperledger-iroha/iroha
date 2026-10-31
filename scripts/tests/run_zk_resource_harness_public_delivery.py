"""Run the closed six normal-publication public-delivery controls with explicit pins."""
import argparse
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from zk_resource_harness_native_fixture import NormalPublicationFixture
from zk_resource_harness_public_delivery_test import PublicDeliveryFiniteTests, FINITE_NAMES


def main():
    """Select only reviewed finite methods and retain their actual public run evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('source-root', 'bridge-root', 'module-sha256', 'provenance-sha256',
                 'source-manifest-sha256', 'native-target', 'retention-root',
                 'disposable-worktree-root'):
        parser.add_argument('--'+name, required=True)
    args = parser.parse_args()
    if not sys.platform.startswith('linux'):
        parser.error('finite native publication controls require Linux')
    source = Path(args.source_root)
    disposable = Path(args.disposable_worktree_root)
    if (not source.is_absolute() or source.resolve() != disposable.resolve()
            or not (source/'.git').is_file()):
        parser.error('late mutation requires explicit identical genuine linked-worktree root')
    retention = Path(args.retention_root)
    if (not retention.is_absolute() or retention.exists()
            or retention.resolve().is_relative_to(source.resolve())):
        parser.error('retention must be fresh, absolute and outside the source worktree')
    fixture = NormalPublicationFixture(args.source_root, args.bridge_root, args.module_sha256,
                                      args.provenance_sha256, args.source_manifest_sha256,
                                      args.native_target)
    retention.mkdir(mode=0o700)
    PublicDeliveryFiniteTests.publication_fixture = fixture
    PublicDeliveryFiniteTests.retention_root = retention
    suite = unittest.TestSuite(PublicDeliveryFiniteTests(name) for name in FINITE_NAMES)
    result = unittest.TextTestRunner(verbosity=2, failfast=True).run(suite)
    return 0 if (result.wasSuccessful() and result.testsRun == len(FINITE_NAMES)
                 and not result.skipped) else 1


if __name__ == '__main__':
    raise SystemExit(main())
