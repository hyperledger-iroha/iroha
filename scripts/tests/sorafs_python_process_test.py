"""Real small Python subprocess controls; no SDK execution qualification."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import time

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_python_process as owner


@pytest.fixture
def runtime(tmp_path):
    for name in ("home", "tmp"):
        (tmp_path / name).mkdir()
    return dict(cwd=tmp_path, stdout_path=tmp_path / "stdout.log", stderr_path=tmp_path / "stderr.log",
                home=tmp_path / "home", temporary=tmp_path / "tmp", stdout_limit=65536,
                stderr_limit=65536, timeout_seconds=3)


def run(source, runtime):
    return owner.run_python_process((sys.executable, "-I", "-B", "-c", source), **runtime)


def test_real_success_retains_exact_separate_binary_streams(runtime):
    result = run("import os; os.write(1,b'actual\\x00\\xffstdout'); os.write(2,b'actual stderr\\n')", runtime)
    assert result.returncode == 0
    for observation, expected in ((result.stdout,b'actual\x00\xffstdout'),(result.stderr,b'actual stderr\n')):
        assert observation.path.read_bytes() == expected
        assert observation.size == len(expected)
        assert observation.sha256 == hashlib.sha256(expected).hexdigest()
        assert observation.path.stat().st_mode & 0o777 == 0o600


def test_real_nonzero_status_is_preserved(runtime):
    result = run("import sys; print('failed attempt'); sys.exit(7)",runtime)
    assert result.returncode == 7
    assert result.stdout.path.read_bytes() == b'failed attempt\n'
    assert result.stderr.size == 0


def test_environment_has_exact_closed_inventory(runtime,monkeypatch):
    for name in ('PYTHONPATH','PYTHONSTARTUP','PYTEST_ADDOPTS','LD_PRELOAD','DYLD_INSERT_LIBRARIES','PIP_INDEX_URL','PRIVATE_SENTINEL'):
        monkeypatch.setenv(name,'must-not-cross')
    result=run('import json,os; print(json.dumps(dict(os.environ),sort_keys=True))',runtime)
    actual=json.loads(result.stdout.path.read_bytes())
    expected=owner.private_environment(runtime['home'],runtime['temporary'])
    assert {key:actual[key] for key in expected} == expected
    # Darwin's framework initializes this runtime value after exec; it was not
    # inherited or supplied by the producer. Other platforms add no variable.
    assert set(actual)-set(expected) <= ({'__CF_USER_TEXT_ENCODING'} if sys.platform=='darwin' else set())
    assert 'must-not-cross' not in actual.values()


def test_closed_stdin_and_empty_streams(runtime):
    result=run("import sys; assert sys.stdin.buffer.read()==b''",runtime)
    assert result.returncode==0 and result.stdout.size==result.stderr.size==0


@pytest.mark.parametrize('stream',(1,2))
@pytest.mark.parametrize('extra',(0,1))
def test_actual_exact_stream_ceiling_and_one_over(runtime,stream,extra):
    name='stdout' if stream==1 else 'stderr'
    runtime[name+'_limit']=8192
    code=f"import os; os.write({stream},b'x'*{8192+extra})"
    if extra:
        with pytest.raises(owner.ProcessError,match='byte limit'): run(code,runtime)
        assert runtime[name+'_path'].stat().st_size==8192
    else:
        result=run(code,runtime)
        assert getattr(result,name).size==8192


def test_both_pipes_drain_without_deadlock(runtime):
    runtime.update(stdout_limit=1024*1024,stderr_limit=1024*1024)
    result=run("import os\nfor i in range(128):\n os.write(1,b'a'*8192)\n os.write(2,b'b'*8192)",runtime)
    assert result.returncode==0
    assert result.stdout.size==result.stderr.size==1024*1024


def test_zero_stream_limit_rejects_actual_byte(runtime):
    runtime['stderr_limit']=0
    with pytest.raises(owner.ProcessError,match='byte limit'): run("import os; os.write(2,b'!')",runtime)
    assert runtime['stderr_path'].read_bytes()==b''


def test_timeout_terminates_and_reaps_own_python(runtime,monkeypatch):
    runtime['timeout_seconds']=0.2
    original=owner.subprocess.Popen; children=[]
    def capture(*args,**kwargs):
        process=original(*args,**kwargs); children.append(process); return process
    monkeypatch.setattr(owner.subprocess,'Popen',capture)
    started=time.monotonic()
    with pytest.raises(owner.ProcessError,match='wall-clock'):
        run("import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(20)",runtime)
    assert time.monotonic()-started<2
    assert len(children)==1 and children[0].poll() is not None
    with pytest.raises(ChildProcessError): os.waitpid(children[0].pid,os.WNOHANG)


def test_child_closing_pipes_still_has_wall_limit(runtime):
    runtime['timeout_seconds']=0.15
    with pytest.raises(owner.ProcessError,match='wall-clock'):
        run("import os,time; os.close(1); os.close(2); time.sleep(20)",runtime)


def test_read_error_reaps_the_actual_child(runtime,monkeypatch):
    original_popen=owner.subprocess.Popen; original_read=os.read; children=[]
    def capture(*args,**kwargs):
        process=original_popen(*args,**kwargs); children.append(process)
        pipe_fd=process.stdout.fileno()
        def fail(fd,size):
            if fd==pipe_fd: raise OSError('injected pipe read failure')
            return original_read(fd,size)
        monkeypatch.setattr(owner.os,'read',fail)
        return process
    monkeypatch.setattr(owner.subprocess,'Popen',capture)
    with pytest.raises(owner.ProcessError,match='output failed'):
        run("import os,time; os.write(1,b'ready'); time.sleep(20)",runtime)
    assert children[0].poll() is not None


@pytest.mark.parametrize('kind',('regular','symlink'))
def test_existing_log_is_never_overwritten(runtime,kind):
    marker=runtime['cwd']/'untouched'; marker.write_bytes(b'original')
    path=runtime['stdout_path']
    if kind=='regular': path.write_bytes(b'original')
    else: path.symlink_to(marker)
    with pytest.raises(owner.ProcessError): run('print("never")',runtime)
    assert marker.read_bytes()==b'original' and path.read_bytes()==b'original'


def test_retained_log_replacement_is_rejected(runtime):
    path=runtime['stdout_path']
    code=f"from pathlib import Path\np=Path({str(path)!r})\np.unlink()\np.write_bytes(b'substituted')"
    with pytest.raises(owner.ProcessError,match='file identity'): run(code,runtime)


def test_retained_log_in_place_mutation_is_rejected(runtime):
    path=runtime['stdout_path']
    code=f"import os,time\nos.write(1,b'actual')\ntime.sleep(.05)\nfrom pathlib import Path\nPath({str(path)!r}).write_bytes(b'forged')"
    with pytest.raises(owner.ProcessError,match='bytes changed'): run(code,runtime)


@pytest.mark.parametrize('key,value',(('timeout_seconds',0),('timeout_seconds',True),('timeout_seconds',float('nan')),
                                      ('timeout_seconds',float('inf')),('stdout_limit',-1),('stderr_limit',True),
                                      ('stdout_limit',owner.MAX_OUTPUT_BYTES+1)))
def test_invalid_limits_fail_before_creating_outputs(runtime,key,value):
    runtime[key]=value
    with pytest.raises(owner.ProcessError): run('pass',runtime)
    assert not runtime['stdout_path'].exists()


def test_distinct_log_owners_required(runtime):
    runtime['stderr_path']=runtime['stdout_path']
    with pytest.raises(owner.ProcessError): run('pass',runtime)
    assert not runtime['stdout_path'].exists()



def test_popen_receives_only_constructed_environment(runtime,monkeypatch):
    original=owner.subprocess.Popen; environments=[]
    def capture(*args,**kwargs):
        environments.append(kwargs["env"].copy())
        return original(*args,**kwargs)
    monkeypatch.setattr(owner.subprocess,"Popen",capture)
    assert run("pass",runtime).returncode==0
    assert environments==[owner.private_environment(runtime["home"],runtime["temporary"])]


def test_output_parent_substitution_is_rejected(runtime):
    parent=runtime["cwd"]/"logs"; parent.mkdir()
    runtime["stdout_path"]=parent/"stdout.log"; runtime["stderr_path"]=parent/"stderr.log"
    code=f"from pathlib import Path; p=Path({str(parent)!r}); p.rename(p.with_name('old-logs')); p.mkdir()"
    with pytest.raises(owner.ProcessError,match="parent was replaced"): run(code,runtime)



def test_inherited_pipe_timeout_reaps_direct_child_and_stops_own_session(runtime,monkeypatch):
    runtime["timeout_seconds"]=0.2
    original=owner.subprocess.Popen; children=[]
    def capture(*args,**kwargs):
        process=original(*args,**kwargs); children.append(process); return process
    monkeypatch.setattr(owner.subprocess,"Popen",capture)
    code=("import subprocess,sys; subprocess.Popen([sys.executable,'-I','-B','-c',"
          "\"import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(10)\"])")
    with pytest.raises(owner.ProcessError,match="wall-clock"): run(code,runtime)
    assert children[0].poll()==0
