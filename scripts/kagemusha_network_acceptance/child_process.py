"""Shared natural child completion for maintained acceptance tools.

Unknown spawn/completion retains its identity and stops further work. This owner
does not use subprocess.run/check_output, a Popen context manager, or signals.
"""
import subprocess


class UnfinishedChild(RuntimeError):
    """Unknown child completion; callers must not launch further work."""
    def __init__(self, argv, pid):
        # Initial admission precedes a campaign directory. Keep child identity in
        # the uncaught diagnostic even when no failure receipt can be written.
        super().__init__(f'child completion unavailable; no signals sent; '
                         f'reconciliation required (pid={pid!r}, argv={argv!r})')
        self.argv, self.pid = argv, pid


def natural_run(argv, **kwargs):
    """subprocess.run subset that never signals a spawned tool or evaluator."""
    check = kwargs.pop('check', False)
    capture_output = kwargs.pop('capture_output', False)
    if 'timeout' in kwargs or 'input' in kwargs:
        raise ValueError('unreviewed child execution option')
    if capture_output:
        if 'stdout' in kwargs or 'stderr' in kwargs:
            raise ValueError('ambiguous child streams')
        kwargs.update(stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    child = None
    try:
        child = subprocess.Popen(argv, **kwargs)
        stdout, stderr = child.communicate()
    except BaseException as error:
        raise UnfinishedChild(argv, None if child is None else child.pid) from error
    completed = subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
    if check:
        completed.check_returncode()
    return completed
