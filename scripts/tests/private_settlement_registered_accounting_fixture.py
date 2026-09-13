"""Explicit synthetic native facts through mandatory retained release validators.

Only clean-source, smoke and observation-utility admission facts are test doubles.
The current filesystem, request, control, process, packet, economics, accounting
and release reducers are always invoked. No Iroha process or capture is launched.
"""
from __future__ import annotations
import atexit
from contextlib import contextmanager,ExitStack
from pathlib import Path
import tempfile
from unittest.mock import patch

from retained_scope_fixture import build_scope,canonical_configuration_inputs,raw,RUNNER
import private_settlement_registered_session_replay as replay

_AREA=tempfile.TemporaryDirectory(prefix='retained-synthetic-image-admission-')
atexit.register(_AREA.cleanup)
_ROOT=Path(_AREA.name).resolve()
(_ROOT/'source').mkdir(mode=0o700)
for name,payload in [('plan-harness',b'synthetic Python harness owner\n'),
                     ('worker',b'synthetic retained native worker\n'),
                     ('validator',b'reproducible iroha3d candidate\n')]:
    (_ROOT/name).write_bytes(payload);(_ROOT/name).chmod(0o700)

def admitted_images():
    """Bind real, nonexecuted disposable image files used by synthetic records."""
    return {name:{'path':str(_ROOT/name),**RUNNER.verify_harness(_ROOT/name)} for name in ('worker','validator')}

@contextmanager
def fixture_admission(*,source_side_effect=None):
    """Run the public admission call graph with declared synthetic prerequisites."""
    images=admitted_images()
    prerequisite={'passed':True,'runs':10,'integration_sha256':images['worker']['sha256'],
                  'validator_sha256':images['validator']['sha256']}
    actual=RUNNER.verify_harness
    def image(path):
        if Path(path)==Path('/bin/ps').resolve():return {'sha256':'c'*64,'bytes':actual(path)['bytes']}
        return actual(path)
    with ExitStack() as stack:
        stack.enter_context(patch.object(RUNNER,'verify_source_checkout',side_effect=source_side_effect))
        stack.enter_context(patch.object(RUNNER,'validate_smoke_prerequisite',return_value=prerequisite))
        stack.enter_context(patch.object(RUNNER,'verify_harness',side_effect=image))
        stack.enter_context(patch.object(replay.samples.network,'_utility_identity',return_value=(replay.samples.network.LSOF,'d'*64)))
        yield dict(source_root=_ROOT/'source',plan_harness=_ROOT/'plan-harness',smoke_campaign=_ROOT/'synthetic-smoke',
                   worker_path=_ROOT/'worker',validator_path=_ROOT/'validator')

def build_registered_accounting_fixture(root,*,validator_sha256,**kwargs):
    """Build one complete session campaign and its explicitly failed predecessor."""
    images=admitted_images()
    if validator_sha256!=images['validator']['sha256']:
        raise ValueError('fixture validator digest differs from admitted image')
    return build_scope(root,images=images,plan_harness=RUNNER.verify_harness(_ROOT/'plan-harness'),**kwargs)
