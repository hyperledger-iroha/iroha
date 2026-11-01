"""Disposable one-shot CLI exercising actual bridge error exit and source custody."""
from contextlib import ExitStack
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import sys


def main():
    source, support, library, program, controls, mode = map(Path, sys.argv[1:])
    mode = str(mode)
    sys.path.insert(0, str(support))
    import workspace_source_keeper as keeper
    spec = importlib.util.spec_from_file_location('candidate_bridge', source)
    bridge_module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bridge_module)
    first = OSError('first original observation refusal')
    original_cause = ValueError('original observation cause')
    first.__cause__ = original_cause
    cleanup = RuntimeError('secondary original-owner cleanup failure')
    marker = controls / 'fault-observed'
    restore = controls / 'restore-observation'
    try:
        with ExitStack() as owners:
            owners.callback((controls / 'owners-closed').write_bytes, b'closed after terminal')
            if mode == 'read-error-cleanup':
                def cleanup_failure():
                    raise cleanup
                owners.callback(cleanup_failure)
            originals = owners.enter_context(keeper.SourceKeeper(program.parent))
            originals.capture([program.name, 'source-guard'])
            admitted_library = owners.enter_context(keeper.SourceKeeper(library.parent))
            admitted_library.capture([library.name])
            loaded = ctypes.CDLL(str(library), **({'winmode': 0x100 | 0x800} if os.name == 'nt' else {}))
            bridge = bridge_module.Bridge(loaded, owners=owners)
            actual_read, actual_wait, actual_close = bridge.read, bridge.wait, bridge.close
            injected = False
            refused_after_finish = 0

            def read(*arguments):
                nonlocal injected
                if mode in ('read-error', 'read-error-cleanup', 'unknown', 'outer-unwind') and not injected:
                    injected = True
                    marker.write_bytes(b'actual post-spawn refusal')
                    raise first
                return actual_read(*arguments)

            def wait(*arguments):
                nonlocal refused_after_finish
                if mode == 'unknown' and not restore.exists():
                    if (controls / 'workload-finished').exists():
                        refused_after_finish += 1
                        if refused_after_finish >= 5:
                            (controls / 'unknown-after-workload-finish').write_bytes(
                                b'five actual refusal callbacks after the workload finish marker')
                    return -3
                return actual_wait(*arguments)

            def close(*arguments):
                if mode == 'unknown' and not restore.exists():
                    return -3
                if mode == 'close-error':
                    marker.write_bytes(b'close after actual terminal wait')
                    raise first
                return actual_close(*arguments)

            bridge.read, bridge.wait, bridge.close = read, wait, close
            if mode == 'outer-unwind':
                def interrupt_between_guards(frame, event, _argument):
                    if (event == 'line' and frame.f_code is bridge.run.__func__.__code__
                            and frame.f_locals.get('custody') is not None
                            and frame.f_locals['custody'].first is first):
                        # A real one-shot interruption after the session guard
                        # must be caught by the enclosing custody's exit fallback.
                        sys.settrace(None)
                        (controls / 'outer-unwind-reached').write_bytes(b'between run guard and supervisor')
                        raise KeyboardInterrupt('later interruption outside the inner session guard')
                    return interrupt_between_guards
                sys.settrace(interrupt_between_guards)
            environment = {key: value for key, value in os.environ.items() if key.upper() == 'SYSTEMROOT'}
            mode_argument = 'root-with-leaf' if os.name == 'nt' else 'held-root'
            bridge.run({'program': str(program), 'directory': str(controls),
                        'arguments': [mode_argument, str(controls)], 'environment': environment}, owners)
            raise AssertionError('the injected original refusal must not become success')
    except BaseException as error:
        (controls / 'cli-refusal.json').write_text(json.dumps({
            'first_error_identity_preserved': error is first,
            'original_cause_preserved': error.__cause__ is original_cause,
            'secondary_cleanup_retained': error.__context__ is cleanup,
            'type': type(error).__name__, 'message': str(error),
        }))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
