"""Real thin shell -> reviewed pipeline -> pinned Rust entry and shared bridge controls."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

import pytest

sys.path.insert(0,str(Path(__file__).parent))
from test_release_entry_owner import BASE,Fixture,native_entry,reviewed_modules
from test_pipeline_preparation import _add_original_bridge


class Shell:
    def __init__(self,root,executable):
        record={'reviewed_modules':reviewed_modules()}
        files={f'scripts/{name}':(BASE/name).read_bytes() for name in
               ['run_release_pipeline.py','sumeragi_prebuilt_bundle.sh',*record['reviewed_modules']]}
        files['.gitignore']=b'/target\n'
        self.fixture=Fixture(root/'invocation-é-東京',executable,source_files=files)
        self.command=[]
        _add_original_bridge(self,root/'bridge-parent'/'bridge')
        value=self.fixture.invocation;pins=self.fixture.pins
        self.arguments=[str(value.repo_root),pins.source_manifest_sha256,
            '--entry-root',str(value.entry_root),'--trusted-entry-sha256',pins.executable_sha256,
            '--trusted-entry-provenance-sha256',pins.provenance_sha256,
            '--native-target',pins.native_target,*self.command]
        self.environment=dict(os.environ)
        self.environment.pop('RUSTUP_TOOLCHAIN',None)
        self.environment.pop('IROHA_RELEASE_PREBUILT_MANIFEST_SHA256',None)
        self.environment.update(IROHA_RELEASE_POLICY_PYTHON=sys.executable,
            CARGO_TARGET_DIR=str(value.cargo_root),IROHA_RELEASE_ARTIFACT_ROOT=str(value.artifact_root),
            CARGO_HOME=str(value.cargo_home),IROHA_RELEASE_CARGO_BIN=str(value.cargo_bin),
            IROHA_RELEASE_RUSTC_BIN=str(value.rustc_bin),IROHA_RELEASE_CANCEL_REQUEST_PATH=str(value.cancel_request),
            IROHA_TEST_TARGET_DIR=str(root/'unanchored-target'),IROHA_RELEASE_BUNDLE_BIN=str(root/'foreign-helper'))
        self.script=value.repo_root/'scripts/sumeragi_prebuilt_bundle.sh'
        if os.name=='nt':
            self.bash=Path('C:/Program Files/Git/bin/bash.exe')
            assert self.bash.is_file(),'native Git Bash fixture host is required'
        else:
            self.bash=shutil.which('bash')
            assert self.bash is not None

    def run(self,body):
        program='''set -e
script="$1"
shift
case "$(uname -s)" in MINGW*|MSYS*) script="$(cygpath -au -- "$script")" ;; esac
source "$script"
'''+body
        return subprocess.run([str(self.bash),'--noprofile','--norc','-c',program,'shell-owner-fixture',str(self.script),*self.arguments],env=self.environment,encoding="utf-8",capture_output=True)


def test_shell_routes_independent_pins_and_exports_all_four_native_names(tmp_path,native_entry):
    case=Shell(tmp_path,native_entry)
    result=case.run('''sumeragi_ensure_source_bound_localnet_binaries "$@"
sumeragi_export_source_bound_localnet_binaries "$@"
printf '%s\\n' "$IROHA_TEST_TARGET_DIR" "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" "$TEST_NETWORK_BIN_IROHAD" "$TEST_NETWORK_BIN_IROHA" "$KAGAMI_BIN" "$TEST_NETWORK_BIN_IROHAD_TAIRA"
''')
    assert result.returncode==0,result.stderr
    lines=result.stdout.splitlines();assert len(lines)==6
    bundle=Path(lines[0]);namespace=case.fixture.invocation.artifact_root/'sumeragi-release'/case.fixture.aggregate/'programs'
    assert str(bundle.parent)==str(namespace)
    assert re.fullmatch(r'invocation\.[a-zA-Z0-9]{6}',bundle.name)
    assert lines[1]=='d'*64
    suffix='.exe' if os.name=='nt' else ''
    assert lines[2:]==[str(bundle/(name+suffix)) for name in ('iroha3d','iroha','kagami','iroha3d_taira')]


def test_clearing_shell_anchor_creates_a_distinct_fresh_invocation(tmp_path,native_entry):
    case=Shell(tmp_path,native_entry)
    result=case.run('''sumeragi_ensure_source_bound_localnet_binaries "$@"
first="$IROHA_TEST_TARGET_DIR"
unset IROHA_RELEASE_PREBUILT_MANIFEST_SHA256
sumeragi_ensure_source_bound_localnet_binaries "$@"
printf '%s\\n' "$first" "$IROHA_TEST_TARGET_DIR"
''')
    assert result.returncode==0,result.stderr
    values=result.stdout.splitlines();assert len(values)==2 and values[0]!=values[1]
    assert all(re.fullmatch(r'invocation\.[a-zA-Z0-9]{6}',Path(value).name) for value in values)


def test_shell_partial_inherited_anchor_never_launches_entry(tmp_path,native_entry):
    case=Shell(tmp_path,native_entry)
    case.environment['IROHA_RELEASE_PREBUILT_MANIFEST_SHA256']='d'*64
    case.environment.pop('IROHA_TEST_TARGET_DIR')
    result=case.run('sumeragi_ensure_source_bound_localnet_binaries "$@"\n')
    assert result.returncode==2 and result.stdout==''
    case.fixture.assert_no_entry()


def test_shell_repeated_trust_option_never_launches_entry(tmp_path,native_entry):
    case=Shell(tmp_path,native_entry)
    case.arguments.extend(['--native-target',case.fixture.target])
    result=case.run('sumeragi_ensure_source_bound_localnet_binaries "$@"\n')
    assert result.returncode==2 and result.stdout==''
    case.fixture.assert_no_entry()


def test_creation_ignores_repository_target_symlink_authority(tmp_path,native_entry):
    case=Shell(tmp_path,native_entry)
    unrelated=tmp_path/'unrelated-repository-target'
    unrelated.mkdir()
    selected=case.fixture.source/'target'
    selected.symlink_to(unrelated,target_is_directory=True)
    result=case.run('''sumeragi_ensure_source_bound_localnet_binaries "$@"
printf '%s\\n' "$IROHA_TEST_TARGET_DIR"
''')
    assert result.returncode==0,result.stderr
    bundle=Path(result.stdout.strip())
    expected=case.fixture.invocation.artifact_root/'sumeragi-release'/case.fixture.aggregate/'programs'
    assert str(bundle.parent)==str(expected)
    assert selected.is_symlink()
    assert list(unrelated.iterdir())==[]
    assert (case.fixture.invocation.artifact_root/'entry-invocations').read_text()=='create\n'


def test_shell_reuses_inherited_anchors_and_refuses_changed_acknowledgement(tmp_path,native_entry):
    """Assert shell/entry forwarding; actual Rust admission separately proves no rebuild."""
    case=Shell(tmp_path,native_entry)
    result=case.run('''sumeragi_ensure_source_bound_localnet_binaries "$@"
first="$IROHA_TEST_TARGET_DIR"
anchor="$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"
sumeragi_ensure_source_bound_localnet_binaries "$@"
[[ "$IROHA_TEST_TARGET_DIR" == "$first" ]]
[[ "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256" == "$anchor" ]]
IROHA_RELEASE_PREBUILT_MANIFEST_SHA256="0000000000000000000000000000000000000000000000000000000000000000"
if sumeragi_ensure_source_bound_localnet_binaries "$@"; then
  exit 66
fi
printf '%s\\n' "$first" "$anchor"
''')
    assert result.returncode==0,result.stderr
    assert len(result.stdout.splitlines())==2
    assert result.stdout.splitlines()[1]=='d'*64
    assert 'entry changed independent inherited anchors' in result.stderr
    assert (case.fixture.invocation.artifact_root/'entry-invocations').read_text().splitlines()==[
        'create','admit','admit']
