"""Actual isolated BLAKE3 import and private source/bundle file tests; no child."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch

from preflight_context import current_context
_CONTEXT = current_context()
BASE = _CONTEXT.work_root
WHEEL_SOURCE = _CONTEXT.dependency_root
import scaling_cli_bootstrap as m
ACTUAL_IMPORT = False
ACTUAL_BOOTSTRAP = False


def sha(raw): return hashlib.sha256(raw).hexdigest()

def write(path,raw,mode=0o600):
    path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
    path.write_bytes(raw);path.chmod(mode);return path


def make_source(root):
    source=root/'source';source.mkdir(mode=0o700)
    for name in m.PYTHON_SOURCE_FILES:write(source/name,('# fixture '+name+'\n').encode())
    write(source/'Cargo.lock',b'version = 4\n')
    paths=root/'source.paths';m.source_contract.write_source_path_list(paths,sorted(['Cargo.lock',*m.PYTHON_SOURCE_FILES]));paths.chmod(0o600)
    manifest=m.source_contract.workspace_source_manifest_from_exact_path_list(source,paths)
    return source,paths,sha(paths.read_bytes()),manifest


class DependencyTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=BASE)
        self.root=Path(self.temp.name).resolve();self.source=self.root/'input';self.owners=[]
        m.stage_dependency_source(WHEEL_SOURCE,self.source)
    def tearDown(self):
        for owner in self.owners:
            for name,module,_ in owner._module_pins:
                if sys.modules.get(name) is module: del sys.modules[name]
            owner.close()
        for path in self.root.rglob('*'):
            if path.is_dir():path.chmod(0o700)
        self.temp.cleanup()
    def owner(self):
        result=m.PythonDependencies.provision(self.source,self.root/'private',self.root/'inventory.json')
        self.owners.append(result);return result
    def test_stage_excludes_caches_and_binds_record(self):
        self.assertEqual(set(m.artifact_contract.scan_inventory_paths(self.source)),set(m._package_files()))
        self.assertFalse(any(path.name=='__pycache__' for path in self.source.rglob('*')))
        for name in m._package_files():self.assertEqual((self.source/name).read_bytes(),(WHEEL_SOURCE/name).read_bytes())
    def test_original_private_bundle_reverification(self):
        owner=self.owner();owner.validate();owner.verify()
        self.assertIs(type(owner.paths),m.PythonDependencyPaths)
        self.assertEqual(owner.paths.inventory_sha256,sha((self.root/'inventory.json').read_bytes()))
        with self.assertRaises(m.ScalingBootstrapError):owner.module
    def test_wrong_record_hash_rejected(self):
        write(self.source/'blake3/__init__.py',b'# replacement\n')
        with self.assertRaises(m.ScalingBootstrapError):self.owner()
    def test_extra_empty_directory_rejected(self):
        (self.source/'empty').mkdir(mode=0o700)
        with self.assertRaises(m.ScalingBootstrapError):self.owner()
    def test_extra_cache_rejected(self):
        write(self.source/'blake3/__pycache__/injected.pyc',b'cache')
        with self.assertRaises(m.ScalingBootstrapError):self.owner()
    def test_symlink_and_hardlink_rejected(self):
        path=self.source/'blake3/__init__.pyi';path.unlink();path.symlink_to('__init__.py')
        with self.assertRaises((ValueError,RuntimeError)):self.owner()
        path.unlink();os.link(self.source/'blake3/__init__.py',path)
        with self.assertRaises((ValueError,RuntimeError)):self.owner()
    def test_wrong_version_rejected(self):
        metadata=self.source/'blake3-1.0.9.dist-info/METADATA'
        write(metadata,metadata.read_bytes().replace(b'Version: 1.0.9',b'Version: 9.9.9'))
        with self.assertRaises(m.ScalingBootstrapError):self.owner()
    def test_inherited_inventory_digest_is_required(self):
        owner=self.owner();paths=replace(owner.paths,inventory_sha256='f'*64)
        with self.assertRaises(m.ScalingBootstrapError):m.PythonDependencies.admit(paths)
    def test_mutated_original_source_poison(self):
        owner=self.owner();path=self.source/'blake3/py.typed';path.chmod(0o400)
        with self.assertRaises(m.ScalingBootstrapError):owner.validate()
        path.chmod(0o600)
        with self.assertRaises(m.ScalingBootstrapError):owner.verify()
    def test_destination_inode_replacement_rejected(self):
        owner=self.owner();path=self.root/'private/blake3/__init__.py';raw=path.read_bytes()
        path.parent.chmod(0o700);path.rename(path.with_name('old'));write(path,raw,0o400)
        with self.assertRaises(m.ScalingBootstrapError):owner.validate()
    def test_inventory_anchor_type_mutation_rejected(self):
        owner=self.owner();object.__setattr__(owner._paths,'inventory_sha256',True)
        with self.assertRaises(m.ScalingBootstrapError):owner.validate()
    def test_closed_owner_cannot_reopen(self):
        owner=self.owner();owner.close()
        with self.assertRaises(m.ScalingBootstrapError):owner.verify()
    def test_cleanup_preserves_reused_file_fd_and_closes_own_parent(self):
        owner=self.owner();held=owner._held[0]
        descriptor,parent=held['descriptor'],held['parent_fd']
        os.close(descriptor)
        foreign_path=write(self.root/'foreign',b'foreign')
        foreign=os.open(foreign_path,os.O_RDONLY)
        self.assertEqual(foreign,descriptor)
        try:
            with self.assertRaises(m.ScalingBootstrapError):owner.validate()
            owner.close()
            self.assertEqual(os.pread(foreign,7,0),b'foreign')
            with self.assertRaises(OSError):os.fstat(parent)
        finally:os.close(foreign)
    def test_foreign_ambient_import_is_rejected(self):
        owner=self.owner();fake=types.ModuleType('blake3');fake.__file__='/foreign/blake3.py'
        with patch.dict(sys.modules,{'blake3':fake}):
            with self.assertRaises(m.ScalingBootstrapError):owner.load()
            self.assertIs(sys.modules['blake3'],fake)
    def test_actual_isolated_import_and_hash_vectors(self):
        global ACTUAL_IMPORT
        owner=self.owner();before=tuple(sys.path);module=owner.load()
        self.assertEqual(tuple(sys.path),before)
        self.assertIs(owner.module,module)
        self.assertEqual(module.blake3(b'').hexdigest(),'af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262')
        self.assertEqual(module.blake3(b'abc').hexdigest(),'6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85')
        data=b'fixed config\n'*1000;stream=module.blake3()
        for offset in range(0,len(data),7):stream.update(data[offset:offset+7])
        self.assertEqual(stream.digest(),module.blake3(data).digest())
        self.assertEqual(Path(module.__file__),self.root/'private/blake3/__init__.py')
        self.assertEqual(Path(sys.modules['blake3.blake3'].__file__),self.root/'private/blake3'/('blake3'+m._profile()))
        owner.verify()
        ACTUAL_IMPORT = True
        module.__file__='/foreign/alias.py'
        with self.assertRaises(m.ScalingBootstrapError):owner.validate()
    def test_caller_constructor_and_subclass_rejected(self):
        with self.assertRaises(m.ScalingBootstrapError):m.PythonDependencies(True)
        class Foreign(m.PythonDependencies):pass
        with self.assertRaises(m.ScalingBootstrapError):Foreign.admit(None)


class SourceAndInvocationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=BASE);self.root=Path(self.temp.name).resolve()
    def tearDown(self):self.temp.cleanup()
    def provision(self,args):
        return m.provision_python_sources(*args,self.root/'python-sources',self.root/'python-sources.json',self.root/'workers')
    def test_exact_source_closure_and_worker_pack(self):
        args=make_source(self.root);result=self.provision(args)
        manifest=json.loads(result.manifest.read_bytes())
        self.assertEqual([row['path'] for row in manifest['files']],list(m.PYTHON_SOURCE_FILES))
        self.assertEqual(result.manifest_sha256,sha(result.manifest.read_bytes()))
        m.artifact_contract.verify_private_python_source_closure(result.root,manifest,result.manifest_sha256,owner_uid=os.geteuid())
        self.assertEqual(set(m.artifact_contract.scan_inventory_paths(result.worker_sources)),set(m._WORKERS))
        for name in m._WORKERS:self.assertEqual((result.worker_sources/name).read_bytes(),(args[0]/'scripts/nexus'/name).read_bytes())
        for path in result.root.rglob('*'):self.assertEqual(path.stat().st_mode&0o777,0o700 if path.is_dir() else 0o600)
    def test_source_manifest_mismatch_no_provision(self):
        args=list(make_source(self.root));args[3]='f'*64
        with self.assertRaises(m.ScalingBootstrapError):self.provision(args)
        self.assertFalse((self.root/'python-sources').exists())
    def test_path_list_anchor_mismatch_no_provision(self):
        args=list(make_source(self.root));args[2]='f'*64
        with self.assertRaises(m.ScalingBootstrapError):self.provision(args)
        self.assertFalse((self.root/'python-sources').exists())
    def test_missing_mandatory_source_rejected(self):
        args=make_source(self.root);(args[0]/m.PYTHON_SOURCE_FILES[0]).unlink()
        with self.assertRaises((ValueError,RuntimeError)):self.provision(args)
    def test_no_output_readmission(self):
        args=make_source(self.root);self.provision(args)
        with self.assertRaises(FileExistsError):self.provision(args)
    def test_fixed_argv_has_only_descriptors_and_nonsecret_digest(self):
        sources=m.ProvisionedPythonSources(self.root/'sources',self.root/'manifest','a'*64,self.root/'workers')
        argv=m.fixed_scaling_argv(self.root/'python',sources,7,'b'*64,9)
        self.assertEqual(argv,(str(self.root/'python'),'-I','-B','-S',str(self.root/'sources'/m._ENTRYPOINT),
            '--launch-input-fd','7','--launch-input-sha256','b'*64,'--seed-fd','9'))
        for first,second in ((True,9),(2,9),(9,9),(7,1<<21)):
            with self.assertRaises(m.ScalingBootstrapError):m.fixed_scaling_argv(self.root/'python',sources,first,'b'*64,second)
        self.assertFalse(any('trial-command' in item or 'IROHA_GSCALE' in item for item in argv))


class BootstrapCompositionTests(unittest.TestCase):
    """Real closure/bundle verifiers; only sys.argv[0] is a declared CLI seam."""
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=BASE);self.root=Path(self.temp.name).resolve()
        self.owners=[]
        rows=[]
        for relative in m.PYTHON_SOURCE_FILES:
            raw=(BASE/'candidate'/relative).read_bytes()
            rows.append(dict(path=relative,sha256=sha(raw),size=len(raw)))
        manifest=dict(schema=m._SOURCE_SCHEMA,files=rows)
        raw=m.artifact_contract.canonical_json_bytes(manifest)
        self.manifest=write(self.root/'sources.json',raw,0o400)
        self.value=dict(schema='iroha.sumeragi_v2.multilane_scaling.launch.v1',
            runtime_paths=dict(python_sources=str(BASE/'candidate'),
                python_entrypoint=m._ENTRYPOINT,python_source_manifest=str(self.manifest),
                python_source_manifest_sha256=sha(raw)),
            python_dependencies=dict(source_root=str(self.root/'dependency-source'),
                bundle_root=str(self.root/'dependency-private'),inventory=str(self.root/'inventory.json'),
                inventory_sha256='f'*64),
            plan=dict(path=str(self.root/'plan.json'),sha256='a'*64),
            budget=dict(path=str(self.root/'budget.json'),sha256='b'*64),
            evidence_root=str(self.root/'evidence'),runtime_root=str(self.root/'runtime'),
            worker_sources=str(self.root/'workers'),
            identity=dict(machine_id='fixture',storage_model='declared fixture',source_revision='a'*40))
    def tearDown(self):
        for owner in self.owners:
            for name,module,_ in owner._module_pins:
                if sys.modules.get(name) is module:del sys.modules[name]
            owner.close()
        for path in self.root.rglob('*'):
            if path.is_dir():path.chmod(0o700)
        self.temp.cleanup()
    def invoke(self):
        with patch.object(sys,'argv',[str(BASE/'candidate'/m._ENTRYPOINT)]):
            return m.bootstrap_runtime(json.dumps(self.value).encode())
    def provision_dependencies(self):
        m.stage_dependency_source(WHEEL_SOURCE,self.root/'dependency-source')
        owner=m.PythonDependencies.provision(self.root/'dependency-source',
            self.root/'dependency-private',self.root/'inventory.json')
        self.owners.append(owner)
        self.value['python_dependencies']['inventory_sha256']=owner.paths.inventory_sha256
    def test_actual_source_closure_and_private_package_composition(self):
        global ACTUAL_BOOTSTRAP
        self.provision_dependencies();before=tuple(sys.path)
        owner=self.invoke();self.owners.append(owner)
        self.assertIs(type(owner),m.PythonDependencies)
        self.assertEqual(tuple(sys.path),before)
        self.assertEqual(owner.module.blake3(b'abc').hexdigest(),
            '6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85')
        owner.verify();ACTUAL_BOOTSTRAP=True
    def test_wrong_module_root_rejected_before_package_admission(self):
        self.value['runtime_paths']['python_sources']=str(self.root/'foreign')
        with self.assertRaises(m.ScalingBootstrapError):self.invoke()
        self.assertNotIn('blake3',sys.modules)
    def test_wrong_manifest_anchor_rejected_before_package_admission(self):
        self.value['runtime_paths']['python_source_manifest_sha256']='a'*64
        with self.assertRaises(m.ScalingBootstrapError):self.invoke()
        self.assertNotIn('blake3',sys.modules)
    def test_wrong_entrypoint_rejected_before_package_admission(self):
        self.value['runtime_paths']['python_entrypoint']='scripts/nexus/arbitrary.py'
        with self.assertRaises(m.ScalingBootstrapError):self.invoke()
        self.assertNotIn('blake3',sys.modules)
    def test_noncanonical_dependency_shape_rejected(self):
        self.value['python_dependencies']['qualified']='true'
        with self.assertRaises(m.ScalingBootstrapError):self.invoke()
    def test_unbound_loaded_source_rejected_before_package_import(self):
        self.provision_dependencies()
        fake=types.ModuleType('scaling_fixed_trial');fake.__file__='/foreign/scaling_fixed_trial.py'
        with patch.dict(sys.modules,{'scaling_fixed_trial':fake}):
            with self.assertRaises(m.ScalingBootstrapError):self.invoke()
        self.assertNotIn('blake3',sys.modules)
    def test_manifest_mutation_during_package_load_rejected(self):
        self.provision_dependencies()
        actual_load=m.PythonDependencies.load
        def mutate(owner):
            result=actual_load(owner);self.owners.append(owner)
            self.manifest.chmod(0o600)
            return result
        with patch.object(m.PythonDependencies,'load',mutate):
            with self.assertRaises(m.ScalingBootstrapError):self.invoke()
        with self.assertRaises(m.ScalingBootstrapError):self.owners[-1].verify()
