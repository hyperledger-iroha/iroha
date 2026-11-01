"""Preserve every observation/error-order control at the single original owner."""
from pathlib import Path
import sys

import pytest

sys.path.insert(0,str(Path(__file__).parent))
from test_release_process_bridge import Fixture, compiled, bridge_parent


@pytest.mark.parametrize('stage,interruption',[
    ('read',KeyboardInterrupt),('close',SystemExit),('wait',KeyboardInterrupt),
    ('wait',SystemExit),('wait',OSError),('sleep',KeyboardInterrupt),('sleep',SystemExit),
])
def test_observation_failure_retains_original_child_and_source_until_terminal(tmp_path,compiled,monkeypatch,stage,interruption):
    case=Fixture(tmp_path,compiled)
    first=interruption('injected original observation interruption')
    actual_read,actual_wait,actual_close=case.bridge.read,case.bridge.wait,case.bridge.close
    actual_sleep=bridge_parent.time.sleep
    interrupted=[]
    def once():
        if not interrupted:
            interrupted.append(True)
            raise first
    def read(*arguments):
        case.original.revalidate()
        if stage=='read':once()
        return actual_read(*arguments)
    waits=[]
    def wait(*arguments):
        case.original.revalidate()
        waits.append(True)
        if stage=='wait':once()
        if stage=='sleep' and len(waits)==1:return 2
        return actual_wait(*arguments)
    def close(*arguments):
        case.original.revalidate()
        if stage=='close':once()
        return actual_close(*arguments)
    def sleep(duration):
        if stage=='sleep':once()
        return actual_sleep(duration)
    case.bridge.read,case.bridge.wait,case.bridge.close=read,wait,close
    monkeypatch.setattr(bridge_parent.time,'sleep',sleep)
    with pytest.raises(interruption) as refused:
        with case.owners:
            try:case.run()
            finally:
                assert case.finished.is_file()
                assert interrupted==[True]
                assert case.closed==[]
                case.original.revalidate()
    assert refused.value is first
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]


def test_overflow_remains_first_when_close_and_wait_are_interrupted(tmp_path,compiled):
    case=Fixture(tmp_path,compiled,'overflow')
    actual_wait,actual_close=case.bridge.wait,case.bridge.close
    waits=[];closes=[]
    def wait(*arguments):
        if not waits:
            waits.append(True)
            raise SystemExit('later wait interruption')
        return actual_wait(*arguments)
    def close(*arguments):
        if not closes:
            closes.append(True)
            raise KeyboardInterrupt('later close interruption')
        return actual_close(*arguments)
    case.bridge.wait,case.bridge.close=wait,close
    with pytest.raises(RuntimeError,match='exceeds its bound') as refused:
        with case.owners:
            try:case.run()
            finally:
                assert case.finished.is_file()
                assert case.closed==[]
                case.original.revalidate()
    assert type(refused.value) is RuntimeError
    assert waits==[True] and closes==[True]
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]


def test_nonzero_original_child_never_yields_plausible_acknowledgement(tmp_path,compiled):
    case=Fixture(tmp_path,compiled,'failure_seven')
    with pytest.raises(bridge_parent.NativeFailure) as refused:
        with case.owners:
            try:case.run()
            finally:
                assert case.finished.is_file()
                assert case.closed==[]
                case.original.revalidate()
    assert refused.value.status==7
    assert case.closed==['closed']
    assert bridge_parent._FAILED_CUSTODY==[]
