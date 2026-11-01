"""Draft entry trust/custody controls using actual Git source and native Rust child bytes."""
from dataclasses import replace
from contextlib import ExitStack
import ast
import hashlib
import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

BASE = Path(__file__).resolve().parents[2] / "scripts"
ENVIRONMENT_POLICY = (BASE/'environment-policy.json').read_bytes()
sys.path.insert(0, str(BASE))
entry = importlib.import_module('release_entry_owner')
custody = importlib.import_module('workspace_source_keeper')
source = importlib.import_module('compute_workspace_source_manifest')
NATIVE_MODULE = importlib.import_module('workspace_source_keeper_windows') if os.name == 'nt' else None


def reviewed_modules():
    """Read the shipped static registry literal without executing the pipeline."""
    tree=ast.parse((BASE/'run_release_pipeline.py').read_text())
    values=[node.value for node in tree.body if isinstance(node,ast.Assign) and
            any(isinstance(target,ast.Name) and target.id=='_NATIVE_PREPARATION_MODULE_SHA256' for target in node.targets)]
    assert len(values)==1
    return ast.literal_eval(values[0])


def digest(data): return hashlib.sha256(data).hexdigest()


@pytest.fixture(scope='module')
def native_entry(tmp_path_factory):
    root = tmp_path_factory.mktemp('entry-compiler')
    code = root / 'entry.rs'
    program = root / entry._filename()
    code.write_text(r'''//! Genuine executable fixture for retained entry protocol only, not a publisher.
use std::{env, fs, io::Write as _, path::PathBuf, time::Duration};
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--owned-leaf") {
        let controls=PathBuf::from(&args[1]);
        fs::write(controls.join("entry-leaf-ready"), b"original descendant running").unwrap();
        while !controls.join("entry-leaf-release").exists() { std::thread::sleep(Duration::from_millis(5)); }
        return;
    }
    assert_eq!(args.first().map(String::as_str), Some("bootstrap-build-publish"));
    let value = |key: &str| args.windows(2).find(|pair| pair[0] == key).unwrap()[1].clone();
    assert!(env::var_os("IROHA_RUNTIME_SIGNING_KEY").is_none());
    assert!(env::var_os("GITHUB_TOKEN").is_none());
    let artifact = PathBuf::from(value("--artifact-root"));
    fs::write(artifact.join("entry-observed"), b"actual original native entry").unwrap();
    let inherited = args.iter().any(|argument| argument == "--bundle-dir");
    let mut calls = fs::OpenOptions::new().create(true).append(true)
        .open(artifact.join("entry-invocations")).unwrap();
    calls.write_all(if inherited { b"admit\n" } else { b"create\n" }).unwrap();
    drop(calls);
    let name = value("--name");
    if name == "invocation.BADsrc" {
        let input = PathBuf::from(value("--repo-root")).join("input.txt");
        fs::write(input, b"changed source").unwrap();
    }
    if name == "invocation.FAIL00" { std::process::exit(19); }
    if name == "invocation.CNCL00" { std::process::exit(125); }
    if name == "invocation.CHAIN0" {
        let _leaf = std::process::Command::new(env::current_exe().unwrap())
            .arg("--owned-leaf").arg(&artifact)
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit()).spawn().unwrap();
    }
    let path = artifact.join("sumeragi-release").join(value("--source-sha256")).join("programs").join(name);
    println!("bundle_dir\t{}\nmanifest_sha256\t{}", path.display(), "d".repeat(64));
}
''')
    compiler=['rustc']+(['+1.93.1-x86_64-pc-windows-msvc'] if os.name=='nt' else [])
    subprocess.run([*compiler, '--edition=2024', '-C', 'debuginfo=0', str(code), '-o', str(program)], check=True)
    return program.read_bytes()


class Fixture:
    def __init__(self, root, executable, *, source_files=None):
        self.source = root / 'source-parent' / 'source'
        self.tool = root / 'entry-parent' / 'entry'
        self.source.mkdir(parents=True, mode=0o700)
        self.tool.mkdir(parents=True, mode=0o700)
        for name in ('cargo', 'artifact', 'cargo-home', 'controls'):
            (root / name).mkdir(mode=0o700)
        subprocess.run(['git', 'init', '-q', str(self.source)], check=True)
        subprocess.run(['git', '-C', str(self.source), 'config', 'core.autocrlf', 'false'], check=True)
        (self.source / 'Cargo.lock').write_bytes(b'version = 4\n')
        (self.source / 'input.txt').write_bytes(b'original source\n')
        for name, payload in (source_files or {}).items():
            selected=self.source/name
            selected.parent.mkdir(parents=True,exist_ok=True)
            selected.write_bytes(payload)
        subprocess.run(['git', '-C', str(self.source), 'add', '.'], check=True)
        # Native clean-index admission needs a real HEAD tree. Build inert Git
        # fixture objects directly; never change commit-signing configuration or
        # create a commit in the working repository under test.
        tree = subprocess.run(['git', '-C', str(self.source), 'write-tree'], check=True,
                              stdout=subprocess.PIPE).stdout.strip()
        identity = b'Entry Fixture <fixture@example.invalid> 1700000000 +0000\n'
        payload = b'tree ' + tree + b'\nauthor ' + identity + b'committer ' + identity + b'\nentry fixture\n'
        commit = subprocess.run(['git', '-C', str(self.source), 'hash-object', '-t', 'commit', '-w', '--stdin'],
                                input=payload, check=True, stdout=subprocess.PIPE).stdout.strip().decode('ascii')
        subprocess.run(['git', '-C', str(self.source), 'update-ref', 'HEAD', commit], check=True)
        self.aggregate = source.native_artifact_workspace_source_manifest(self.source)
        self.target = ('x86_64-pc-windows-msvc' if os.name == 'nt' else 'x86_64-unknown-linux-gnu')
        if entry.platform.machine().lower() in ('aarch64', 'arm64'):
            self.target = ('aarch64-pc-windows-msvc' if os.name == 'nt' else 'aarch64-unknown-linux-gnu')
        self.path = self.tool / entry._filename()
        self.path.write_bytes(executable)
        if os.name == 'posix': self.path.chmod(0o500)
        self.value = {'schema':'iroha.publisher.v1', 'source_manifest_sha256':self.aggregate,
                      'native_target':self.target, 'executable_filename':entry._filename(),
                      'executable_sha256':digest(executable), 'executable_bytes':len(executable),
                      'build_environment_policy_sha256':digest(ENVIRONMENT_POLICY),
                      'cargo_lock_sha256':digest((self.source/'Cargo.lock').read_bytes()),
                      'package_id':'path+file:///fixture/iroha_release_bundle#0.0.0',
                      'cargo_tool':{'bytes':len(executable), 'sha256':digest(executable)},
                      'rustc_tool':{'bytes':len(executable), 'sha256':digest(executable)},
                      'cargo_version':'cargo fixture\n', 'rustc_version':'rustc fixture\nhost: '+self.target+'\n',
                      'configurations':[{'path':str(self.source/'.cargo'), 'present':False,
                                         'config_sha256':None, 'config_toml_sha256':None}],
                      'bootstrap_build_environment':{'CARGO':str(self.path), 'RUSTC':str(self.path),
                          'CARGO_HOME':str(root/'cargo-home'), 'CARGO_TARGET_DIR':str(root/'cargo'),
                          'CARGO_BUILD_JOBS':'1', 'CARGO_INCREMENTAL':'0', 'RUSTUP_AUTO_INSTALL':'0',
                          'ENABLE_RANS_BUNDLES':'1', 'NORITO_SKIP_BINDINGS_SYNC':'1'}}
        self.pins = None
        self.write_provenance()
        self.invocation = entry.Invocation(self.source, self.tool, root/'cargo', root/'artifact', root/'cargo-home',
                                          self.path, self.path, root/'controls'/'cancel.json', 'invocation.abc123')

    def write_provenance(self):
        raw = (json.dumps(self.value, sort_keys=True, separators=(',', ':')) + '\n').encode()
        (self.tool/'publisher-source.json').write_bytes(raw)
        self.pins = entry.EntryPins(self.value['executable_sha256'], digest(raw), self.aggregate, self.target)

    def run(self, **kwargs):
        # Every entry uses the admitted process owner: whole Job on Windows,
        # original root on Unix, including focused entry refusal controls.
        import release_bridge_admission as bridge_admission
        selected=Path(os.environ.get('IROHA_TEST_PROCESS_BRIDGE_LIBRARY',
            str(BASE.parent/'target'/'debug'/('iroha_process_bridge.dll' if os.name=='nt' else 'libiroha_process_bridge.so'))))
        assert selected.is_file(), 'compile the actual process bridge first'
        bridge_root=self.invocation.artifact_root.parent/'bridge-parent'/'bridge'
        bridge_root.mkdir(parents=True)
        library=bridge_root/bridge_admission.filename()
        shutil.copyfile(selected,library)
        if os.name=='posix':library.chmod(0o500)
        raw=library.read_bytes()
        value=dict(self.value)
        value.update(executable_filename=library.name,executable_sha256=digest(raw),executable_bytes=len(raw))
        provenance=(json.dumps(value,sort_keys=True,separators=(',',':'))+'\n').encode()
        (bridge_root/'publisher-source.json').write_bytes(provenance)
        pins=bridge_admission.BridgePins(digest(raw),digest(provenance),self.aggregate,self.target)
        def bridge_factory(original_source,owners):
            return bridge_admission.load_original(bridge_root,pins,original_source,owners,
                entry_module=entry,custody_module=custody,native_module=NATIVE_MODULE,
                environment_policy=ENVIRONMENT_POLICY)
        with ExitStack() as owners:
            return entry.prepare_sumeragi_programs(kwargs.get('invocation', self.invocation), kwargs.get('pins', self.pins),
                source_module=source,custody_module=custody,environment_policy=ENVIRONMENT_POLICY,
                native_module=NATIVE_MODULE,owners=owners,bridge_factory=bridge_factory)

    def assert_no_entry(self):
        assert not (self.invocation.artifact_root/'entry-observed').exists()


def test_pinned_original_entry_runs_with_the_real_canonical_source_owner(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    result = fixture.run()
    assert result == (fixture.invocation.artifact_root/'sumeragi-release'/fixture.aggregate/'programs'/'invocation.abc123', 'd'*64)
    assert (fixture.invocation.artifact_root/'entry-observed').read_bytes() == b'actual original native entry'
    assert source.native_artifact_workspace_source_manifest(fixture.source) == fixture.aggregate


@pytest.mark.parametrize('field', ['executable_sha256', 'provenance_sha256', 'source_manifest_sha256'])
def test_independent_pin_mismatch_refuses_before_entry_execution(tmp_path, native_entry, field):
    fixture = Fixture(tmp_path, native_entry)
    with pytest.raises((RuntimeError, OSError)):
        fixture.run(pins=replace(fixture.pins, **{field: '0'*64}))
    fixture.assert_no_entry()


@pytest.mark.parametrize('field,value', [('schema','legacy'), ('native_target','foreign-host'),
    ('source_manifest_sha256','0'*64), ('executable_filename','alias'), ('signature','unverified-claim')])
def test_even_independently_pinned_provenance_must_match_the_closed_contract(tmp_path, native_entry, field, value):
    fixture = Fixture(tmp_path, native_entry)
    fixture.value[field] = value
    fixture.write_provenance()
    with pytest.raises((RuntimeError, OSError)):
        fixture.run()
    fixture.assert_no_entry()


def test_wrong_actual_bytes_are_not_replaced_with_a_fresh_self_hash(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    if os.name == 'posix': fixture.path.chmod(0o700)
    fixture.path.write_bytes(b'not the independently trusted executable')
    if os.name == 'posix': fixture.path.chmod(0o500)
    with pytest.raises((RuntimeError, OSError)):
        fixture.run()
    fixture.assert_no_entry()


def test_nonzero_native_entry_with_plausible_protocol_never_returns_success(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    with pytest.raises(RuntimeError):
        fixture.run(invocation=replace(fixture.invocation, name='invocation.FAIL00'))
    assert (fixture.invocation.artifact_root/'entry-observed').exists()


def test_real_source_mutation_or_native_writer_exclusion_both_refuse_acceptance(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    with pytest.raises((RuntimeError, OSError)):
        fixture.run(invocation=replace(fixture.invocation, name='invocation.BADsrc'))
    assert (fixture.invocation.artifact_root/'entry-observed').exists()


def test_ambient_helper_environment_never_selects_the_entry(tmp_path, native_entry, monkeypatch):
    fixture = Fixture(tmp_path, native_entry)
    monkeypatch.setenv('IROHA_RELEASE_BUNDLE_BIN', str(tmp_path/'untrusted'))
    monkeypatch.setenv('IROHA_TEST_TARGET_DIR', str(tmp_path/'unanchored'))
    fixture.run()
    assert (fixture.invocation.artifact_root/'entry-observed').exists()


def test_partial_inherited_anchor_refuses_without_bootstrap_or_output_mutation(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    with pytest.raises(RuntimeError):
        fixture.run(invocation=replace(fixture.invocation, inherited_manifest='d'*64))
    fixture.assert_no_entry()


def test_same_physical_source_and_output_root_refuses_before_entry(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    with pytest.raises(RuntimeError):
        fixture.run(invocation=replace(fixture.invocation, artifact_root=fixture.source))
    fixture.assert_no_entry()


def test_root_security_allows_child_creation_but_refuses_policy_change(tmp_path):
    with pytest.raises(RuntimeError, match='expected root policy refusal'):
        with entry.RootSecurity(tmp_path, custody, native_module=NATIVE_MODULE) as original:
            (tmp_path/'child').write_bytes(b'Rust-owned output stand-in')
            original.revalidate()
            if os.name == 'posix':
                tmp_path.chmod(0o755 if tmp_path.stat().st_mode & 0o777 == 0o700 else 0o700)
            else:
                subprocess.run(['icacls', str(tmp_path), '/grant', '*S-1-1-0:(RX)', '/Q'], check=True, capture_output=True)
            with pytest.raises((RuntimeError, OSError)):
                original.revalidate()
            # A refused owner must not be used as a success context; preserve its latched refusal.
            assert original.failed
            raise RuntimeError('expected root policy refusal')


def test_unrelated_private_values_are_not_forwarded_or_added_to_provenance(tmp_path, native_entry, monkeypatch):
    fixture = Fixture(tmp_path, native_entry)
    monkeypatch.setenv('IROHA_RUNTIME_SIGNING_KEY', 'fixture-only-secret')
    monkeypatch.setenv('GITHUB_TOKEN', 'fixture-only-token')
    fixture.run()
    assert b'fixture-only' not in (fixture.tool/'publisher-source.json').read_bytes()


@pytest.mark.parametrize('name', ['RUSTC_WRAPPER', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS',
    'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'LD_PRELOAD'])
def test_unsupported_override_refuses_before_entry_or_output_allocation(tmp_path, native_entry, monkeypatch, name):
    fixture = Fixture(tmp_path, native_entry)
    monkeypatch.setenv(name, 'unapproved')
    with pytest.raises(RuntimeError, match='unsupported inherited'):
        fixture.run()
    fixture.assert_no_entry()


@pytest.fixture(scope='module')
def emitter():
    """The driver compiles the exact Rust emitter, never a Python substitute."""
    filename = 'provenance_fixture.exe' if os.name == 'nt' else 'provenance_fixture'
    path = Path(os.environ.get('IROHA_TEST_PROVENANCE_EMITTER',
        str(BASE.parent / 'target' / 'debug' / 'examples' / filename)))
    assert path.is_file(), 'compile the actual provenance_fixture example before these controls'
    return path


def emit(emitter, value, *, success=True):
    process = subprocess.run([str(emitter)], input=json.dumps(value).encode(), capture_output=True)
    if success:
        assert process.returncode == 0, process.stderr
    else:
        assert process.returncode != 0
    return process.stdout


def test_actual_rust_emitter_roundtrips_into_closed_python_admission(tmp_path, native_entry, emitter):
    fixture = Fixture(tmp_path, native_entry)
    raw = emit(emitter, fixture.value)
    pins = replace(fixture.pins, provenance_sha256=digest(raw))
    assert entry._provenance(raw, pins, ENVIRONMENT_POLICY) == fixture.value
    (fixture.tool/'publisher-source.json').write_bytes(raw)
    fixture.run(pins=pins)
    assert (fixture.invocation.artifact_root/'entry-observed').is_file()


@pytest.mark.parametrize('field', ['schema', 'source_manifest_sha256', 'cargo_lock_sha256', 'native_target',
    'package_id', 'executable_filename', 'executable_sha256', 'executable_bytes', 'cargo_tool', 'rustc_tool',
    'cargo_version', 'rustc_version', 'configurations', 'build_environment_policy_sha256', 'bootstrap_build_environment'])
def test_every_missing_emitter_field_refuses_in_both_languages(tmp_path, native_entry, emitter, field):
    fixture = Fixture(tmp_path, native_entry)
    value = dict(fixture.value)
    del value[field]
    emit(emitter, value, success=False)
    raw = json.dumps(value).encode()
    with pytest.raises(RuntimeError, match='field inventory'):
        entry._provenance(raw, replace(fixture.pins, provenance_sha256=digest(raw)), ENVIRONMENT_POLICY)


@pytest.mark.parametrize('nested', [None, 'cargo_tool', 'rustc_tool', 'configuration'])
def test_unknown_evidence_fields_refuse_in_both_languages(tmp_path, native_entry, emitter, nested):
    fixture = Fixture(tmp_path, native_entry)
    value = fixture.value
    selected = value if nested is None else value['configurations'][0] if nested == 'configuration' else value[nested]
    selected['unverified_signature'] = 'must-not-authorize'
    emit(emitter, value, success=False)
    raw = json.dumps(value).encode()
    with pytest.raises(RuntimeError):
        entry._provenance(raw, replace(fixture.pins, provenance_sha256=digest(raw)), ENVIRONMENT_POLICY)


def test_emitter_success_cannot_reauthorize_a_substituted_record(tmp_path, native_entry, emitter):
    fixture = Fixture(tmp_path, native_entry)
    original = emit(emitter, fixture.value)
    trusted = replace(fixture.pins, provenance_sha256=digest(original))
    fixture.value['cargo_tool']['sha256'] = '0'*64
    replacement = emit(emitter, fixture.value)
    assert replacement != original
    with pytest.raises(RuntimeError, match='independent pipeline trust'):
        entry._provenance(replacement, trusted, ENVIRONMENT_POLICY)


def test_original_lock_must_match_even_an_independently_pinned_entry_record(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    fixture.value['cargo_lock_sha256'] = '0'*64
    fixture.write_provenance()
    with pytest.raises(RuntimeError, match='independent original Cargo.lock'):
        fixture.run()
    fixture.assert_no_entry()


def test_recorded_environment_cannot_contain_an_unrelated_private_value(tmp_path, native_entry):
    fixture = Fixture(tmp_path, native_entry)
    fixture.value['bootstrap_build_environment']['IROHA_RUNTIME_SIGNING_KEY'] = 'fixture-only-secret'
    fixture.write_provenance()
    with pytest.raises(RuntimeError, match='unreviewed environment'):
        fixture.run()
    fixture.assert_no_entry()
