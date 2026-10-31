"""Black-box draft operation: reviewed registry and independent entry trust survive launch."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).parent))

from test_release_entry_owner import BASE, Fixture, native_entry, reviewed_modules


class Pipeline:
    def __init__(self, root, executable):
        self.fixture = Fixture(root/'invocation', executable)
        self.scripts = root/'reviewed-parent'/'scripts'
        self.scripts.mkdir(parents=True, mode=0o700)
        record={'reviewed_modules':reviewed_modules()}
        for name in ['run_release_pipeline.py', *record['reviewed_modules']]:
            shutil.copyfile(BASE/name,self.scripts/name)
        self.command=[sys.executable,str(self.scripts/'run_release_pipeline.py'),'prepare-sumeragi-programs']
        # The fixture compiler uses an explicit toolchain; the pipeline receives
        # its own valid caller environment and must continue refusing that override.
        self.environment=dict(os.environ)
        self.environment.pop('RUSTUP_TOOLCHAIN',None)
        value=self.fixture.invocation
        pins=self.fixture.pins
        for name,argument in (
            ('repo-root',value.repo_root),('entry-root',value.entry_root),('cargo-target-dir',value.cargo_root),
            ('artifact-root',value.artifact_root),('cargo-home',value.cargo_home),('cargo-bin',value.cargo_bin),
            ('rustc-bin',value.rustc_bin),('cancel-request',value.cancel_request),('name',value.name),
            ('trusted-entry-sha256',pins.executable_sha256),('trusted-entry-provenance-sha256',pins.provenance_sha256),
            ('trusted-workspace-source-sha256',pins.source_manifest_sha256),('native-target',pins.native_target)):
            self.command.extend(['--'+name,str(argument)])

    def run(self):
        return subprocess.run(self.command,capture_output=True,text=True,env=self.environment)


def test_actual_pipeline_operation_requires_parent_owner_before_native_activation(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    result=case.run()
    assert result.returncode==2
    assert 'requires independently pinned native bridge inputs' in result.stderr
    case.fixture.assert_no_entry()


@pytest.mark.parametrize('name', ['workspace_source_keeper.py','workspace_source_keeper_windows.py',
    'compute_workspace_source_manifest.py','sumeragi_prebuilt_bundle.py','release_entry_owner.py','environment-policy.json',
    'release_process_bridge.py','release_bridge_admission.py'])
def test_changed_reviewed_payload_is_refused_before_any_entry(tmp_path,native_entry,name):
    case=Pipeline(tmp_path,native_entry)
    with (case.scripts/name).open('ab') as output:
        output.write(b'\n# changed after independent review\n')
    result=case.run()
    assert result.returncode==2
    assert 'differ from the reviewed pipeline source' in result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


def test_initial_bootstrap_link_never_gains_content_authority(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    original=case.scripts/'workspace_source_keeper.py'
    alternate=tmp_path/'authentic-but-indirect.py'
    original.rename(alternate)
    os.symlink(alternate,original)
    result=case.run()
    assert result.returncode==2
    assert result.stdout==''
    case.fixture.assert_no_entry()


def test_arbitrary_helper_environment_does_not_replace_explicit_entry_pins(tmp_path,native_entry,monkeypatch):
    case=Pipeline(tmp_path,native_entry)
    monkeypatch.setenv('IROHA_RELEASE_BUNDLE_BIN',str(tmp_path/'foreign-helper'))
    index=case.command.index('--trusted-entry-sha256')+1
    case.command[index]='0'*64
    result=case.run()
    assert result.returncode==2
    assert result.stdout==''
    case.fixture.assert_no_entry()


def _add_original_bridge(case, root):
    """Actual compiled diagnostic library; pins are fixture trust, never release trust."""
    import hashlib
    library = Path(os.environ.get('IROHA_TEST_PROCESS_BRIDGE_LIBRARY',
        str(BASE.parent/'target'/'debug'/('iroha_process_bridge.dll' if os.name=='nt' else 'libiroha_process_bridge.so'))))
    assert library.is_file(), 'compile the same diagnostic bridge before this control'
    root.mkdir(parents=True)
    filename = 'iroha_process_bridge.dll' if os.name=='nt' else 'libiroha_process_bridge.so'
    selected=root/filename
    shutil.copyfile(library,selected)
    if os.name=='posix':selected.chmod(0o500)
    data=selected.read_bytes()
    digest=hashlib.sha256(data).hexdigest()
    value=dict(case.fixture.value)
    value.update(executable_filename=filename,executable_sha256=digest,executable_bytes=len(data))
    record=(json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode()
    (root/'publisher-source.json').write_bytes(record)
    case.command.extend(['--bridge-root',str(root),'--trusted-bridge-sha256',digest,
                         '--trusted-bridge-provenance-sha256',hashlib.sha256(record).hexdigest()])
    return selected


def test_actual_pipeline_keeps_all_original_owners_through_shared_bridge_entry(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    result=case.run()
    assert result.returncode==0,result.stderr
    assert result.stdout.splitlines()==[
        'bundle_dir\t'+str(case.fixture.invocation.artifact_root/'sumeragi-release'/case.fixture.aggregate/'programs'/'invocation.abc123'),
        'manifest_sha256\t'+'d'*64]
    assert (case.fixture.invocation.artifact_root/'entry-observed').is_file()


@pytest.mark.parametrize('name,status',[('invocation.FAIL00',19),('invocation.CNCL00',125)])
def test_pipeline_preserves_original_failure_and_cancellation_exit_without_acknowledgement(tmp_path,native_entry,name,status):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    case.command[case.command.index('--name')+1]=name
    result=case.run()
    assert result.returncode==status,result.stderr
    assert result.stdout==''
    assert f'exit {status}' in result.stderr
    assert (case.fixture.invocation.artifact_root/'entry-observed').is_file()


@pytest.mark.parametrize('argument',['trusted-bridge-sha256','trusted-bridge-provenance-sha256'])
def test_pipeline_bridge_independent_pin_refuses_before_entry(tmp_path,native_entry,argument):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    case.command[case.command.index('--'+argument)+1]='0'*64
    result=case.run()
    assert result.returncode==2,result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


@pytest.mark.parametrize('missing',['bridge-root','trusted-bridge-sha256','trusted-bridge-provenance-sha256'])
def test_pipeline_partial_bridge_trust_never_launches(tmp_path,native_entry,missing):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    index=case.command.index('--'+missing)
    del case.command[index:index+2]
    result=case.run()
    assert result.returncode==2,result.stderr
    assert 'bridge trust inputs are incomplete' in result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


def test_pipeline_bridge_root_cannot_be_its_source_or_output_namespace(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    index=case.command.index('--bridge-root')+1
    case.command[index]=str(case.fixture.invocation.artifact_root)
    result=case.run()
    assert result.returncode==2,result.stderr
    assert 'physically overlaps' in result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


if os.name=='nt':
    def test_pipeline_retains_registry_source_entry_and_bridge_until_descendant_release(tmp_path,native_entry):
        import errno
        import time
        case=Pipeline(tmp_path,native_entry)
        bridge=_add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
        index=case.command.index('--name')+1
        case.command[index]='invocation.CHAIN0'
        originals=[case.scripts/'release_process_bridge.py',case.fixture.source/'input.txt',case.fixture.path,bridge]
        for original in originals:
            # These exact fixture originals are writable before retained admission.
            with original.open('r+b'):pass
        child=subprocess.Popen(case.command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=case.environment)
        release=case.fixture.invocation.artifact_root/'entry-leaf-release'
        ready=case.fixture.invocation.artifact_root/'entry-leaf-ready'
        try:
            deadline=time.monotonic()+20
            while not ready.exists():
                assert child.poll() is None,'pipeline stopped before the real descendant handshake'
                assert time.monotonic()<deadline,'owned descendant never became ready'
                time.sleep(0.01)
            assert child.poll() is None
            for original in originals:
                with pytest.raises(OSError) as failure:
                    with original.open('r+b'):pass
                # CPython CRT opens may preserve only EACCES, not GetLastError.
                assert failure.value.errno == errno.EACCES
                assert failure.value.winerror in (None,5,32)
            release.write_bytes(b'original descendant protocol release')
            stdout,stderr=child.communicate()
            assert child.returncode==0,stderr.decode(errors='replace')
            assert stdout.splitlines()==[
                b'bundle_dir\t'+str(case.fixture.invocation.artifact_root/'sumeragi-release'/case.fixture.aggregate/'programs'/'invocation.CHAIN0').encode(),
                b'manifest_sha256\t'+b'd'*64]
        finally:
            if not release.exists():release.write_bytes(b'diagnostic cleanup release')
            # Only this owned process and its fixture protocol are observed here.
            # No timeout recovery kills or resolves a process by PID.
            while True:
                try:child.wait();break
                except BaseException:time.sleep(0.01)


def test_pipeline_entry_requires_its_exact_two_original_members(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    (case.fixture.tool/'unadmitted-runtime.dll').write_bytes(b'unrelated entry dependency')
    result=case.run()
    assert result.returncode==2,result.stderr
    assert 'entry directory is not the exact two-entry artifact' in result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


def test_pipeline_refuses_inherited_fixture_toolchain_override(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    _add_original_bridge(case,tmp_path/'native-bridge-parent'/'bridge')
    case.environment['RUSTUP_TOOLCHAIN']='untrusted-override'
    result=case.run()
    assert result.returncode==2,result.stderr
    assert 'unsupported inherited compiler, linker or Cargo override' in result.stderr
    assert result.stdout==''
    case.fixture.assert_no_entry()


@pytest.mark.parametrize('spelling',['dot-component','empty-component'])
def test_pipeline_refuses_raw_path_alias_spelling_before_path_construction(tmp_path,native_entry,spelling):
    case=Pipeline(tmp_path,native_entry)
    index=case.command.index('--repo-root')+1
    original=case.fixture.source
    case.command[index]=(str(original)+os.sep+'.' if spelling=='dot-component'
                         else str(original.parent)+os.sep+os.sep+original.name)
    result=case.run()
    assert result.returncode==2 and result.stdout==''
    assert 'normalized absolute spelling' in result.stderr
    case.fixture.assert_no_entry()


def test_pipeline_repeated_trust_argument_is_refused_before_registry_or_entry(tmp_path,native_entry):
    case=Pipeline(tmp_path,native_entry)
    case.command.extend(['--native-target',case.fixture.target])
    result=case.run()
    assert result.returncode==2 and result.stdout==''
    assert 'repeated invocation option' in result.stderr
    case.fixture.assert_no_entry()
