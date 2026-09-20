"""File/owner tests with explicit host and archived-interpreter composition seams.

Real source, binary-bundle, Python-source and retained-descriptor verifiers run.
The toy Mach-O files are never executed. The complete framework verifier has a
separate rejection test; positive owner cases replace it with a declared seam.
These tests do not qualify a host, native build, runtime archive or performance.
"""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import struct
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

audit_attempts = []
def audit(event, arguments):
    if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork','os.forkpty','os.kill','os.killpg'):
        audit_attempts.append(event)
        raise AssertionError('test attempted child creation or process signal: '+event)
sys.addaudithook(audit)

from preflight_context import current_context
_CONTEXT = current_context()
ROOT = _CONTEXT.work_root/'candidate'
import scaling_cli_bootstrap as bootstrap
WHEEL_SOURCE = _CONTEXT.dependency_root
DEPENDENCY_TEMP = tempfile.TemporaryDirectory(dir=ROOT.parent)
DEPENDENCY_ROOT = Path(DEPENDENCY_TEMP.name).resolve()
bootstrap.stage_dependency_source(WHEEL_SOURCE,DEPENDENCY_ROOT/'source')
DEPENDENCIES = bootstrap.PythonDependencies.provision(DEPENDENCY_ROOT/'source',
    DEPENDENCY_ROOT/'private',DEPENDENCY_ROOT/'inventory.json')
DEPENDENCIES.load()
import scaling_runtime_admission as m
import scaling_fixed_trial as t
from scaling_experiment_plan import ExperimentPlan, ResourceLimits
from scaling_worker_sources import WorkerSourcePin


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def write(path, raw, mode=0o600):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.write_bytes(raw)
    path.chmod(mode)
    return path


def plan():
    trials = []
    mib = 1024 * 1024
    for pair in range(1, 6):
        for variant, lanes in (('one_lane', 1), ('four_lane', 4)):
            generator = t.GeneratorPlan('trial-test-chain', lanes, 4, '127.0.0.1', '127.0.0.1', 8080, 1337)
            load = t.NativeLoadPlan(pair, variant, digest(f'fixture:{pair}'.encode()), '500', 0,
                200_000_000, 10_000_000, 0, 2, 2, 20, 4, 32, 4, 1, 256, 10, 4, 2)
            reader = t.ReaderBudget(1000, 8*mib, 65536, mib, 1000, mib, 2*mib, os.geteuid())
            collection = t.CollectionLimits(65536, 65536, mib, 4096, 1000, 1000, 4*mib)
            facts = t.FactsBudget(65536, 65536, 65536, 65536, 8*mib, 65536,
                8*mib+65536, 8*mib, 4*mib, 2*mib, 2*mib, 1000, 128, 1024)
            outputs = t.NativeOutputBudget(*(65536 for _ in range(6)), 65536*6)
            trials.append(t.TrialPlan(generator, load, reader, collection, facts, outputs, 128*1024, 5_000_000_000))
    return ExperimentPlan('fixture', tuple(trials), 600_000_000_000, 7_000_000_000_000,
                          ResourceLimits(10000, 1000000, 1<<40, 1<<40))


class Fixture:
    def __init__(self, root):
        self.root = root
        self.source = root / 'source'
        self.source.mkdir(mode=0o700)
        rows = []
        for path in sorted((ROOT/'scripts').rglob('*.py')):
            relative = path.relative_to(ROOT).as_posix()
            raw = path.read_bytes()
            write(self.source/relative, raw)
            rows.append({'path':relative, 'sha256':digest(raw), 'size':len(raw)})
        write(self.source/'Cargo.lock', b'version = 4\n')
        self.pathlist = root/'source.paths'
        m.source_contract.write_source_path_list(self.pathlist,
            sorted(['Cargo.lock', *(row['path'] for row in rows)]))
        self.pathlist.chmod(0o600)
        source_hash = m.source_contract.workspace_source_manifest_from_exact_path_list(self.source, self.pathlist)
        self.python_manifest = {'schema':'fixture.source.v1', 'files':rows}
        python_raw = m.artifact_contract.canonical_json_bytes(self.python_manifest)
        python_manifest_path = write(root/'python-source.json', python_raw)
        self.target = root/'cargo-target'; self.target.mkdir(mode=0o700)
        self.artifacts = root/'artifacts'; self.artifacts.mkdir(mode=0o700)
        default = self.target/'sumeragi-v2-release'/source_hash/'program-build-cache/default'
        message = default.parent/'message-control'
        mach = struct.pack('<8I', 0xfeedfacf, 0x0100000c, 0, 2, 1, 24, 0, 0)
        mach += struct.pack('<II', 0x1b, 24) + b'abcdefghijklmnop'
        for cache, names in ((default, ('iroha3d', 'iroha', 'kagami')), (message, ('iroha3d',))):
            for name in names: write(cache/'release'/name, mach + name.encode(), 0o500)
        rustc_raw = b'rustc fixture\nhost: aarch64-apple-darwin\n'
        rustc = write(root/'rustc-version.txt', rustc_raw)
        cargo = write(root/'cargo-version.txt', b'cargo fixture\n')
        self.bundle, bundle_hash = m.binary_contract.create_bundle(self.source, source_hash,
            self.target, self.artifacts, default, message,
            self.artifacts/'sumeragi-v2-release'/source_hash/'programs', cargo, rustc)
        self.python_evidence = root/'python-evidence'; self.python_evidence.mkdir(mode=0o700)
        python = write(self.python_evidence/'python-runtime/bin/python3', mach+b'python3', 0o500)
        runtime_raw = m.receipt_contract._canonical_json({'fixture':'framework-verifier-seam'})
        runtime_binding = write(root/'python-runtime-binding.json', runtime_raw)
        self.paths = m.ReleaseRuntimePaths(self.source, self.pathlist, digest(self.pathlist.read_bytes()),
            self.target, self.artifacts, self.bundle, bundle_hash, rustc, ROOT,
            python_manifest_path, digest(python_raw), 'scripts/nexus/run_multilane_scaling_gate.py',
            self.python_evidence, runtime_binding, digest(runtime_raw))
        workers_dir = root/'workers'; workers_dir.mkdir(mode=0o700)
        pins = []
        for name in m.SOURCE_NAMES:
            raw = (ROOT/'scripts/nexus'/name).read_bytes()
            write(workers_dir/name, raw)
            pins.append(WorkerSourcePin(name, digest(raw), len(raw)))
        self.workers = m.WorkerSourceFiles(workers_dir, tuple(pins))
        images = []
        for path in (self.bundle/'release/kagami', self.bundle/'release/iroha',
                     self.bundle/'release/iroha3d', python):
            images.append(m.ExecutableImage(path, digest(path.read_bytes())))
        self.images = images
        self.runtime = m.TrialRuntime(*images, self.workers.worker_path, pins[0].sha256)
        self.host = m.HostObservation('fixture CPU', 4, 8, 1<<30, 'macOS fixture', 'fixture kernel',
                                      'arm64', 'Python fixture', 'fixture node')
        self.identity = m.ExperimentIdentity('lab-inventory', self.host.cpu_model, 'declared storage',
            4, 8, 1<<30, self.host.os, self.host.kernel, self.host.architecture,
            self.host.python_version, 'rustc fixture', 'a'*40, source_hash)
        self.plan = plan()
        self.owners = []
        self.patches = [patch.object(m, 'observe_host', return_value=self.host),
            patch.object(m.receipt_contract, '_validate_framework_python_runtime', return_value=[]),
            patch.object(sys, 'executable', str(python)),
            patch.object(sys,'argv',[str(ROOT/'scripts/nexus/run_multilane_scaling_gate.py')])]
        self.patches += [patch.object(sys, name, str(python.parent.parent)) for name in
                         ('prefix','exec_prefix','base_prefix','base_exec_prefix')]
        for item in self.patches: item.start()

    def admit(self, **changes):
        kwargs = dict(paths=self.paths, identity=self.identity, runtime=self.runtime,
                      workers=self.workers, plan=self.plan, dependencies=DEPENDENCIES)
        kwargs.update(changes)
        result = m.RuntimeAdmission.admit(**kwargs)
        self.owners.append(result)
        return result

    def close(self):
        for owner in self.owners: owner.close()
        for item in reversed(self.patches): item.stop()
        for image in self.images: image.close()
        self.workers.close()


class PrimitiveTests(unittest.TestCase):
    def test_path_and_digest_reject_types_and_noncanonical_values(self):
        for value in ('relative', Path('relative'), Path('/a/../b'), 1):
            with self.subTest(value=value), self.assertRaises(m.RuntimeAdmissionError): m._path(value)
        for value in (True, 'a'*63, 'A'*64, 'a'*65):
            with self.subTest(value=value), self.assertRaises(m.RuntimeAdmissionError): m._digest(value)
        self.assertEqual(m._digest('a'*64), 'a'*64)
        self.assertEqual(m._path(Path('/safe')), Path('/safe'))

    def test_no_public_or_subclass_constructor(self):
        with self.assertRaises(m.RuntimeAdmissionError): m.RuntimeAdmission(True)
        class Foreign(m.RuntimeAdmission): pass
        with self.assertRaises(m.RuntimeAdmissionError): Foreign.admit(None, None, None, None, None, None)

    def test_python_manifest_types_bounds_order_and_duplicates(self):
        row = dict(path='scripts/fixture.py', sha256='a'*64, size=1)
        good = dict(schema='fixture', files=[row])
        raw = m.artifact_contract.canonical_json_bytes(good)
        self.assertEqual(m._python_manifest(raw), good)
        bad = [dict(schema='x',files=[]), dict(schema='x',files=[dict(row,size=True)]),
               dict(schema='x',files=[dict(row,path='scripts/../escape.py')]),
               dict(schema='x',files=[row,row]), dict(schema='x',files=[dict(row,sha256='A'*64)]),
               dict(schema='x',files=[dict(row,size=8*1024*1024+1)]),
               dict(schema='x',files=[row]*257), dict(schema='x',files=[dict(row,extra=1)])]
        for item in bad:
            with self.subTest(item=str(item)[:80]), self.assertRaises((ValueError,RuntimeError)):
                m._python_manifest(m.artifact_contract.canonical_json_bytes(item))
        with self.assertRaises(m.artifact_contract.ReleaseArtifactError): m._python_manifest(b'{"schema":"a","schema":"b","files":[]}')

    def test_host_reads_are_bounded_and_use_fixed_kernel_keys(self):
        values = {b'machdep.cpu.brand_string': b'fixture CPU\0',
                  b'hw.physicalcpu': (4).to_bytes(4,sys.byteorder),
                  b'hw.logicalcpu': (8).to_bytes(4,sys.byteorder),
                  b'hw.memsize': (1<<30).to_bytes(8,sys.byteorder),
                  b'kern.osproductversion': b'26.0\0'}
        calls = []
        class Read:
            def __call__(self,name,buffer,size,write_value,write_size):
                self.test.assertEqual(size._obj.value,512)
                self.test.assertIsNone(write_value);self.test.assertEqual(write_size,0)
                raw=values[name];calls.append(name)
                buffer[:len(raw)]=raw;size._obj.value=len(raw)
                return 0
        read=Read();read.test=self
        class Library: pass
        library=Library();library.sysctlbyname=read
        with patch.object(m.ctypes,'CDLL',return_value=library) as load, patch.object(sys,'platform','darwin'):
            value=m.observe_host()
            self.assertEqual(value.physical_cores,4);self.assertEqual(value.memory_bytes,1<<30)
            self.assertEqual(value.os,'macOS 26.0');self.assertEqual(value.cpu_model,'fixture CPU')
            self.assertEqual(calls,list(values))
            load.assert_called_once_with('/usr/lib/libSystem.B.dylib',use_errno=True)
            for name,raw in ((b'hw.physicalcpu',(9).to_bytes(4,sys.byteorder)),
                             (b'machdep.cpu.brand_string',b'bad'),
                             (b'hw.memsize',b'bad')):
                original=values[name];values[name]=raw
                with self.assertRaises(m.RuntimeAdmissionError):m.observe_host()
                values[name]=original
        with patch.object(sys,'platform','unsupported'),self.assertRaises(m.RuntimeAdmissionError):m.observe_host()

    def test_cancellation_control_flow_is_preserved_without_private_text(self):
        for error,expected in ((KeyboardInterrupt('private'),KeyboardInterrupt),
                              (SystemExit('private'),SystemExit),(GeneratorExit('private'),GeneratorExit)):
            with self.subTest(expected=expected),self.assertRaises(expected) as caught:m._failure(error)
            self.assertNotIn('private',str(caught.exception))

    def test_real_framework_verifier_rejects_receipt_only_value(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(m.receipt_contract.ReceiptError):
                m.receipt_contract._validate_framework_python_runtime({'qualified':True}, Path(temporary))

    def test_retained_files_reject_mutation_and_preserve_foreign_fd_on_close(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); path = write(root/'data', b'abc')
            owner = m._RetainedFiles()
            self.assertEqual(owner.read(path,3,digest(b'abc')),b'abc')
            fd = owner.files[0][0]
            os.close(fd)
            other = write(root/'foreign',b'foreign')
            foreign = os.open(other,os.O_RDONLY)
            self.assertEqual(foreign,fd)
            with self.assertRaises(m.RuntimeAdmissionError): owner.validate()
            owner.close(); self.assertEqual(os.pread(foreign,7,0),b'foreign'); os.close(foreign)

    def test_retained_root_replacement_and_read_bounds(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); nested=root/'nested'; nested.mkdir(mode=0o700)
            path=write(nested/'data',b'abc'); owner=m._RetainedFiles()
            with self.assertRaises(m.RuntimeAdmissionError): owner.read(path,2)
            owner.read(path,3); nested.rename(root/'old'); nested.mkdir(mode=0o700)
            with self.assertRaises(m.RuntimeAdmissionError): owner.validate()
            owner.close()


class ComposedTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=ROOT.parent)
        self.f = Fixture(Path(self.temp.name).resolve())
    def tearDown(self):
        self.f.close()
        # Read-only synthetic bundle directories need owner-write for cleanup.
        for path in Path(self.temp.name).rglob('*'):
            if path.is_dir(): path.chmod(0o700)
        self.temp.cleanup()

    def test_original_properties_and_full_reverification(self):
        owner=self.f.admit(); owner.verify(); owner.validate()
        self.assertIs(owner.identity,self.f.identity); self.assertIs(owner.runtime,self.f.runtime)
        self.assertIs(owner.worker_sources,self.f.workers); self.assertIs(owner.plan,self.f.plan)
        self.assertGreater(len(owner._source_pin),40)
        owner.close()
        for name in ('identity','runtime','worker_sources','plan'):
            with self.assertRaises(m.RuntimeAdmissionError): getattr(owner,name)
        for image in self.f.images: image.validate()
        self.f.workers.validate()

    def test_exact_dependency_owner_required(self):
        for value in (True,None,{'inventory_sha256':'a'*64}):
            with self.subTest(value=value),self.assertRaises(m.RuntimeAdmissionError):
                self.f.admit(dependencies=value)

    def test_original_dependency_owner_retained_per_phase(self):
        owner=self.f.admit()
        with patch.object(DEPENDENCIES,'verify',wraps=DEPENDENCIES.verify) as verify:
            owner.verify()
            self.assertEqual(verify.call_count,1)
        owner._dependencies=True
        with self.assertRaises(m.RuntimeAdmissionError):owner.validate()
        owner._dependencies=DEPENDENCIES
        with self.assertRaises(m.RuntimeAdmissionError):owner.verify()

    def test_unloaded_dependency_bundle_cannot_admit_runtime(self):
        root=Path(self.temp.name).resolve()/'extra-dependency';root.mkdir(mode=0o700)
        bootstrap.stage_dependency_source(WHEEL_SOURCE,root/'source')
        dependency=bootstrap.PythonDependencies.provision(root/'source',root/'private',root/'inventory.json')
        try:
            with self.assertRaises(m.RuntimeAdmissionError):self.f.admit(dependencies=dependency)
        finally:dependency.close()

    def test_source_digest_mismatch_is_rejected(self):
        with self.assertRaises(m.RuntimeAdmissionError):
            self.f.admit(identity=replace(self.f.identity,workspace_source_sha256='f'*64))

    def test_original_source_permission_or_bytes_drift_poison(self):
        owner=self.f.admit(); path=self.f.source/'scripts/nexus/resource_process.py'
        path.chmod(0o400)
        with self.assertRaises(m.RuntimeAdmissionError): owner.verify()
        path.chmod(0o600)
        with self.assertRaises(m.RuntimeAdmissionError): owner.verify()

    def test_source_new_file_rejected(self):
        owner=self.f.admit(); write(self.f.source/'unlisted',b'extra')
        with self.assertRaises(m.RuntimeAdmissionError): owner.verify()

    def test_original_control_inode_replacement_rejected(self):
        owner=self.f.admit(); path=self.f.paths.rustc_version; raw=path.read_bytes()
        path.rename(path.with_suffix('.old'));write(path,raw)
        with self.assertRaises(m.RuntimeAdmissionError): owner.validate()

    def test_worker_bytes_mutation_rejected(self):
        owner=self.f.admit(); path=self.f.workers.worker_path; raw=path.read_bytes()
        write(path,raw+b'\n')
        with self.assertRaises(m.RuntimeAdmissionError): owner.validate()

    def test_wrong_binary_role_rejected(self):
        with self.assertRaises(m.RuntimeAdmissionError):
            self.f.admit(runtime=replace(self.f.runtime,kagami=self.f.runtime.cli))

    def test_host_drift_rejected_and_cannot_resume(self):
        owner=self.f.admit()
        with patch.object(m,'observe_host',return_value=replace(self.f.host,logical_cores=9)):
            with self.assertRaises(m.RuntimeAdmissionError): owner.validate()
        with self.assertRaises(m.RuntimeAdmissionError): owner.verify()

    def test_hardware_declaration_must_match_observation(self):
        with self.assertRaises(m.RuntimeAdmissionError):
            self.f.admit(identity=replace(self.f.identity,memory_bytes=123))

    def test_host_architecture_must_match_original_build_triple(self):
        with patch.object(m,'observe_host',return_value=replace(self.f.host,architecture='x86_64')):
            with self.assertRaises(m.RuntimeAdmissionError):
                self.f.admit(identity=replace(self.f.identity,architecture='x86_64'))

    def test_phase_cancellation_poison_preserves_interrupt(self):
        owner=self.f.admit()
        with patch.object(m.source_contract,'workspace_source_manifest_from_exact_path_list',side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):owner.verify()
        with self.assertRaises(m.RuntimeAdmissionError):owner.validate()

    def test_nested_plan_mutation_rejected(self):
        owner=self.f.admit();object.__setattr__(self.f.plan.resource_limits,'disk_bytes_max',1)
        with self.assertRaises(m.RuntimeAdmissionError): owner.validate()

    def test_hostile_identity_equality_is_never_called(self):
        class Hostile:
            def __eq__(self,other): raise AssertionError('foreign equality called')
        owner=self.f.admit();object.__setattr__(self.f.identity,'cpu_model',Hostile())
        with self.assertRaises(m.RuntimeAdmissionError): owner.validate()

    def test_runtime_binding_projection_mutation_rejected(self):
        owner=self.f.admit();owner._python_runtime['qualified']=True
        with self.assertRaises(m.RuntimeAdmissionError):owner.verify()

    def test_python_entrypoint_and_own_origin_must_be_bound(self):
        with self.assertRaises(m.RuntimeAdmissionError):
            self.f.admit(paths=replace(self.f.paths,python_entrypoint='scripts/no_such_file.py'))
        with self.assertRaises(m.RuntimeAdmissionError):
            self.f.admit(paths=replace(self.f.paths,python_sources=self.f.source))

    def test_failed_phase_does_not_accept_new_manifest_anchor(self):
        owner=self.f.admit();owner._paths=replace(owner._paths,binary_manifest_sha256='f'*64)
        with self.assertRaises(m.RuntimeAdmissionError):owner.verify()
        owner._paths=self.f.paths
        with self.assertRaises(m.RuntimeAdmissionError):owner.validate()

    def test_reentry_permanently_fails(self):
        owner=self.f.admit();owner._busy=True
        with self.assertRaises(m.RuntimeAdmissionError):owner.validate()
        with self.assertRaises(m.RuntimeAdmissionError):owner.verify()




def tearDownModule():
    DEPENDENCIES.close()
    for path in DEPENDENCY_ROOT.rglob('*'):
        if path.is_dir():path.chmod(0o700)
    DEPENDENCY_TEMP.cleanup()
