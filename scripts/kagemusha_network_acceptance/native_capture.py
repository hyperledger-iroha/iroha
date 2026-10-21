"""Current CoreZK compiler-closure owner; historical supplements are refused."""
from pathlib import Path
import compiler_capture
from paths import ROOT, QUAL


def admit(item):
    # Grammar is checked before any capture/tool read; no supplemental historical
    # bytes or prior reviewer receipt can replace the prospective current build.
    compiler_capture.require(type(item) is dict and set(item) == {'directory', 'result_sha256', 'review_sha256'},
                             'exact current native capture; no supplements')
    result, inventory, source, custody = compiler_capture.admit(item, {'iroha_core_zk'})
    compiler_capture.require(result['profile'] == 'release', 'optimized current native library capture required')
    owner = dict(custody, binary=result['binaries']['iroha_core_zk'], source=source,
                 retained=custody['reviewed_originals'], packages=inventory['actual_packages'],
                 classification=result['classification'], heads=[custody['before_head'], custody['after_head']],
                 current_source_qualified=True)
    return owner, {}, result['source_before_sha256']


def unchanged(owner, pins):
    for name, digest in pins.items():
        compiler_capture.require(compiler_capture.sha(Path(name)) == digest, 'selected controller source/tool changed')
    compiler_capture.recheck(owner)
