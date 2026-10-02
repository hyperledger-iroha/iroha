"""Disposable actual kernel/FD probes with public data; no Native issuer qualification."""
from pathlib import Path
import os
import signal
import subprocess
import sys
import unittest

PACKAGE = str(Path(__file__).resolve().parents[1] / "src")
PREFIX = "import sys; sys.path.insert(0,sys.argv[1]); "


class PrivateProcessTests(unittest.TestCase):
    def child(self, original):
        # This session contains only the test's own disposable worker/probe child.
        # No production process, credential or filesystem role is a target.
        child = subprocess.Popen([sys.executable, "-I", "-B", "-c", PREFIX + original, PACKAGE],
                                 stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, start_new_session=True, text=True)
        try:
            output, errors = child.communicate(timeout=6)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.communicate()
            self.fail("owned disposable process probe exceeded its external bound")
        self.assertEqual(child.returncode, 0, errors)
        return output.strip()

    def test_zero_core_limit_is_set_and_observed_in_disposable_image(self):
        self.assertEqual(self.child("from iroha_app_attestation.private_process import disable_core_dumps; "
                                    "import resource; disable_core_dumps(); "
                                    "assert resource.getrlimit(resource.RLIMIT_CORE)==(0,0); print('zero')"), "zero")

    @unittest.skipUnless(sys.platform == "darwin", "actual macOS kernel probe")
    def test_darwin_actual_same_uid_ptrace_and_three_task_access_rejections(self):
        self.assertEqual(self.child("from iroha_app_attestation.private_process import protect_darwin_process; "
                                    "protect_darwin_process(); print('observed')"), "observed")

    @unittest.skipUnless(sys.platform == "darwin", "actual macOS kernel probe")
    def test_darwin_generic_task_denial_without_deny_attach_cannot_pass(self):
        original = """
from iroha_app_attestation.private_process import disable_core_dumps, _darwin_library, _darwin_rejection_probe
from iroha_app_attestation.attestation import AttestationRejected
disable_core_dumps()
try:
    _darwin_rejection_probe(_darwin_library())
except AttestationRejected:
    print('rejected')
else:
    raise AssertionError('unprotected actual process formed protection evidence')
"""
        self.assertEqual(self.child(original), "rejected")

    @unittest.skipUnless(sys.platform == "darwin", "actual macOS native thread inventory")
    def test_darwin_multiple_actual_native_threads_deny_before_fork(self):
        original = """
from iroha_app_attestation.private_process import protect_darwin_process
from iroha_app_attestation.attestation import AttestationRejected
import threading
ready=threading.Event(); stop=threading.Event()
def wait():
    ready.set(); stop.wait()
thread=threading.Thread(target=wait);thread.start();ready.wait()
try:
    try:
        protect_darwin_process()
    except AttestationRejected:
        print('rejected')
    else:
        raise AssertionError('multiple native threads were admitted')
finally:
    stop.set();thread.join()
"""
        self.assertEqual(self.child(original), "rejected")

    @unittest.skipUnless(sys.platform == "darwin", "actual owned-child deadline and cleanup")
    def test_darwin_probe_timeout_missing_evidence_and_failed_self_control_reap_only_owned_child(self):
        for failure in ("timeout", "missing", "self-control"):
            with self.subTest(failure=failure):
                original = """
from iroha_app_attestation.private_process import disable_core_dumps, _darwin_library, _darwin_rejection_probe
from iroha_app_attestation.attestation import AttestationRejected
import os,time
disable_core_dumps();library=_darwin_library()
failure=FAILURE
def unavailable(*args):
    if failure=='timeout':time.sleep(4)
    if failure=='missing':raise RuntimeError('public injected availability failure')
    return -1
library.task_for_pid=unavailable
start=time.monotonic()
try:
    _darwin_rejection_probe(library)
except AttestationRejected:
    assert time.monotonic()-start<3
    try:os.waitpid(-1,os.WNOHANG)
    except ChildProcessError:pass
    else:raise AssertionError('owned disposable probe was not reaped')
    print('rejected-and-reaped')
else:
    raise AssertionError('missing actual rejection evidence was accepted')
""".replace("FAILURE", repr(failure))
                self.assertEqual(self.child(original), "rejected-and-reaped")

    def test_actual_worker_roster_closes_unrelated_public_fds_and_all_inheritance(self):
        original = """
from iroha_app_attestation.private_process import close_unrelated_worker_descriptors, require_worker_role_originals
from iroha_app_attestation.attestation import AttestationRejected
import errno,fcntl,os,tempfile
# Public regular files/pipes below select only untrusted descriptor data. Their
# user-owned files cannot pass the installed root-owned issuer role checks.
with tempfile.TemporaryDirectory() as directory:
    path=directory+'/public-fixture';open(path,'wb').write(b'public synthetic data')
    file=os.open(path,os.O_RDONLY);r,w=os.pipe();d=os.open(directory,os.O_RDONLY)
    sources=[fcntl.fcntl(x,fcntl.F_DUPFD_CLOEXEC,64) for x in [r,w,file,d]]
    for x in [file,r,w,d]:os.close(x)
    for fd,source in [(9,sources[0]),(10,sources[1]),(12,sources[2]),(14,sources[2]),
                      (15,sources[2]),(17,sources[3]),(18,sources[2])]:
        os.dup2(source,fd,inheritable=True)
    unrelated=os.dup(sources[2]);os.set_inheritable(unrelated,True)
    roles=close_unrelated_worker_descriptors()
    assert roles==frozenset([9,10,12,14,15,17,18])
    assert all(not os.get_inheritable(fd) for fd in roles)
    for fd in sources+[unrelated]:
        try:os.fstat(fd)
        except OSError as error:assert error.errno==errno.EBADF
        else:raise AssertionError('unrelated descriptor survived')
    try:require_worker_role_originals(roles,False)
    except AttestationRejected:pass
    else:raise AssertionError('user-owned public fixture became installed issuer role')
    try:require_worker_role_originals(roles,True)
    except AttestationRejected:pass
    else:raise AssertionError('absent Google role was accepted')
    for fd in roles:os.close(fd)
print('closed-and-rejected')
"""
        self.assertEqual(self.child(original), "closed-and-rejected")

    def test_missing_native_roles_deny_before_closing_an_unrelated_public_fd(self):
        original = """
from iroha_app_attestation.private_process import close_unrelated_worker_descriptors
from iroha_app_attestation.attestation import AttestationRejected
import os,tempfile
with tempfile.TemporaryFile() as original:
    fd=original.fileno()
    try:close_unrelated_worker_descriptors()
    except AttestationRejected:os.fstat(fd);print('missing')
    else:raise AssertionError('missing Native roles were admitted')
"""
        self.assertEqual(self.child(original), "missing")


if __name__ == "__main__":
    unittest.main()
