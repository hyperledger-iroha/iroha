"""Actual source/bundle/package/plan/descriptor composition; framework seam only.

The native/Python executable fixtures are inert bytes and are never executed.
The existing full framework verifier is replaced only in positive setup, with a
separate actual rejection test. No host observation or RuntimeAdmission runs.
"""
from dataclasses import replace
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

from preflight_context import current_context
_CONTEXT = current_context()
BASE = _CONTEXT.work_root
WHEEL = _CONTEXT.dependency_root
SOURCE_BEFORE={str(path.relative_to(BASE/'candidate')):hashlib.sha256(path.read_bytes()).hexdigest()
    for path in sorted((BASE/'candidate').rglob('*.py'))}
import scaling_release_provisioning as m
SEED='ab'*32

def digest(raw):return hashlib.sha256(raw).hexdigest()
def write(path,raw,mode=0o600):
    path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
    path.write_bytes(raw);path.chmod(mode);return path

class Provisioning(unittest.TestCase):
    def setUp(self):
        self.loaded=set(sys.modules)
        self.temp=tempfile.TemporaryDirectory(dir=BASE);self.root=Path(self.temp.name).resolve()
        self.owners=[];self.events=[]
        source=self.root/'source';source.mkdir(mode=0o700)
        for name in m.python_contract.PYTHON_SOURCE_FILES:
            write(source/name,(BASE/'candidate'/name).read_bytes())
        write(source/'Cargo.lock',b'version = 4\n')
        paths=self.root/'source.paths'
        m.source_contract.write_source_path_list(paths,['Cargo.lock',*m.python_contract.PYTHON_SOURCE_FILES])
        workspace=m.source_contract.workspace_source_manifest_from_exact_path_list(source,paths)
        target=self.root/'target';target.mkdir(mode=0o700)
        artifacts=self.root/'artifacts';artifacts.mkdir(mode=0o700)
        default=target/'sumeragi-v2-release'/workspace/'program-build-cache/default'
        control=default.parent/'message-control'
        mach=struct.pack('<8I',0xfeedfacf,0x0100000c,0,2,1,24,0,0)+struct.pack('<II',0x1b,24)+b'abcdefghijklmnop'
        for cache,names in ((default,('iroha3d','iroha','kagami')),(control,('iroha3d',))):
            for name in names:write(cache/'release'/name,mach+name.encode(),0o500)
        rustc=write(self.root/'rustc.txt',b'rustc fixture\nhost: aarch64-apple-darwin\n')
        cargo=write(self.root/'cargo.txt',b'cargo fixture\n')
        bundle,bundlesha=m.binary_contract.create_bundle(source,workspace,target,artifacts,
            default,control,artifacts/'sumeragi-v2-release'/workspace/'programs',cargo,rustc)
        evidence=self.root/'python-evidence';evidence.mkdir(mode=0o700)
        python=write(evidence/'python-runtime/bin/python3',mach+b'python',0o500)
        self.framework=dict(records=[dict(path='bin/python3',kind='file',mode='0500',size=python.stat().st_size,sha256=digest(python.read_bytes()))])
        binding=write(self.root/'runtime.json',m.receipt_contract._canonical_json(self.framework))
        plan=write(self.root/'plan.json',(BASE/'fixtures/plan.json').read_bytes())
        budget=write(self.root/'budget.json',(BASE/'fixtures/budget.json').read_bytes())
        self.bounds=json.loads((BASE/'fixtures/bounds.json').read_text())
        self.selected=m.ParentScalingSelection(source,paths,digest(paths.read_bytes()),workspace,
            target,artifacts,bundle,bundlesha,rustc,evidence,binding,digest(binding.read_bytes()),
            WHEEL,plan,digest(plan.read_bytes()),budget,digest(budget.read_bytes()),
            self.root/'private-controls',artifacts/'fixed-scaling',self.root/'private-runtime',
            'lab inventory','declared storage','a'*40,(('PATH','/usr/bin:/bin'),),60)
        def framework(value,root):
            self.assertEqual(value,self.framework);self.assertEqual(root,evidence)
            self.events.append('framework');return ('explicit-framework-seam',)
        self.seam=patch.object(m.receipt_contract,'_validate_framework_python_runtime',side_effect=framework)
        self.seam.start()
    def tearDown(self):
        for owner in reversed(self.owners):
            if owner._state=='borrowed':owner.declare_child_reaped()
            owner.close()
        self.seam.stop()
        for name in tuple(set(sys.modules)-self.loaded):
            module=sys.modules[name];origin=getattr(module,'__file__',None)
            if name in ('blake3','blake3.blake3') or (isinstance(origin,str) and str(BASE) in origin):
                del sys.modules[name]
        for path in self.root.rglob('*'):
            if path.is_dir():path.chmod(0o700)
        self.temp.cleanup()
    def prepare(self,value=None):
        owner=m.PreparedScalingInputs.prepare(self.selected if value is None else value,SEED)
        self.owners.append(owner);return owner
    def assert_closed(self,fd):
        with self.assertRaises(OSError):os.fstat(fd)

    def test_actual_sources_bundle_blake3_plan_and_launch(self):
        owner=self.prepare();launch=owner.claim_launch()
        self.assertEqual(owner._dependencies.module.blake3(b'abc').hexdigest(),'6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85')
        self.assertEqual(launch.argv,m.python_contract.fixed_scaling_argv(launch.python,
            owner._sources,launch.launch_input_fd,launch.launch_input_sha256,launch.seed_fd))
        raw=os.pread(launch.launch_input_fd,65537,0)
        self.assertEqual(digest(raw),launch.launch_input_sha256)
        from scaling_experiment_cli_inputs import decode_launch_input
        decoded=decode_launch_input(raw)
        self.assertEqual(decoded.evidence_root,self.selected.evidence_root)
        self.assertEqual(decoded.runtime_root,self.selected.runtime_root)
        self.assertEqual(decoded.python_dependencies,owner._dependencies.paths)
        self.assertEqual(statmode:=os.fstat(launch.launch_input_fd).st_mode&0o777,0o400)
        self.assertEqual(fcntl.fcntl(launch.launch_input_fd,fcntl.F_GETFL)&os.O_ACCMODE,os.O_RDONLY)
        self.assertFalse(os.get_inheritable(launch.launch_input_fd));self.assertFalse(os.get_inheritable(launch.seed_fd))
        self.assertEqual(os.read(launch.seed_fd,65),SEED.encode());self.assertEqual(os.read(launch.seed_fd,1),b'')
        self.assertNotIn(SEED,repr(launch));self.assertNotIn(SEED,raw.decode())
        self.assertFalse(self.selected.evidence_root.exists());self.assertFalse(self.selected.runtime_root.exists())
        timeout=(self.bounds['experiment_timeout_ns']+m._NS-1)//m._NS+self.selected.observation_overhead_seconds
        self.assertEqual(launch.timeout_seconds,timeout)
        self.assertEqual(launch.deadline_ns-launch.original_started_ns,timeout*m._NS)
        self.assertEqual(launch.manifest_max_bytes,self.bounds['manifest_max_bytes'])
        self.assertEqual(launch.report_max_bytes,self.bounds['report_max_bytes'])
        self.assertEqual(launch.maximum_output_bytes,launch.manifest_max_bytes+launch.report_max_bytes)
        owner.validate()
        owner.declare_child_reaped();owner.close()
        self.assert_closed(launch.launch_input_fd);self.assert_closed(launch.seed_fd)
        self.assertTrue((self.selected.control_root/'launch.json').exists())

    def test_original_pending_lifetime_and_one_use(self):
        owner=self.prepare();launch=owner.claim_launch()
        with self.assertRaises(m.ScalingInputsBorrowedError):owner.close()
        os.fstat(launch.launch_input_fd);os.fstat(launch.seed_fd);owner._dependencies.verify()
        with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()
        owner.declare_child_reaped();owner.close();owner.close()
        with self.assertRaises(m.ScalingProvisioningError):owner.declare_child_reaped()

    def test_actual_runtime_admission_never_called(self):
        owner=self.prepare()
        import scaling_runtime_admission as admission
        with patch.object(admission.RuntimeAdmission,'admit',side_effect=AssertionError('parent admission forbidden')):
            owner.validate();owner.claim_launch();owner.declare_child_reaped();owner.close()

    def test_wrong_source_path_list_digest_before_provision(self):
        with self.assertRaises(m.ScalingProvisioningError):self.prepare(replace(self.selected,source_paths_sha256='f'*64))
        self.assertFalse((self.selected.control_root/'python-sources').exists())

    def test_actual_bundle_binary_drift_rejected(self):
        binary=self.selected.binary_bundle/'release/kagami';binary.chmod(0o700);binary.write_bytes(b'wrong');binary.chmod(0o500)
        with self.assertRaises(m.ScalingProvisioningError):self.prepare()

    def test_manifest_cannot_select_a_foreign_binary_path(self):
        foreign=write(self.root/'foreign',b'private unrelated input')
        path=self.selected.binary_bundle/m.binary_contract._MANIFEST_NAME
        path.chmod(0o600);raw=path.read_bytes().replace(b'kagami_relative_path\trelease/kagami',
            ('kagami_relative_path\t'+str(foreign)).encode());path.write_bytes(raw);path.chmod(0o400)
        actual=m._OriginalInputs.hold;opened=[]
        def hold(owner,path,*args,**kwargs):
            opened.append(path);return actual(owner,path,*args,**kwargs)
        with patch.object(m._OriginalInputs,'hold',hold):
            with self.assertRaises(m.ScalingProvisioningError):self.prepare(replace(self.selected,binary_manifest_sha256=digest(raw)))
        self.assertNotIn(foreign,opened)

    def test_actual_framework_rejects_receipt_only_projection(self):
        self.seam.stop()
        with self.assertRaises(m.ScalingProvisioningError):self.prepare()

    def test_wrong_plan_digest_closes_loaded_dependency(self):
        captured=[];actual=m.python_contract.PythonDependencies.load
        def load(owner):captured.append(owner);return actual(owner)
        with patch.object(m.python_contract.PythonDependencies,'load',load):
            with self.assertRaises(m.ScalingProvisioningError):self.prepare(replace(self.selected,plan_sha256='f'*64))
        self.assertEqual(len(captured),1)
        with self.assertRaises(m.python_contract.ScalingBootstrapError):captured[0].verify()
        for name,_,_ in captured[0]._module_pins:sys.modules.pop(name,None)

    def test_supplied_bound_fields_are_not_a_selection_api(self):
        # No compatibility inputs or defaults can bypass canonical derivation.
        for field in ('timeout_seconds','maximum_output_bytes','manifest_max_bytes','report_max_bytes'):
            with self.subTest(field=field):
                with self.assertRaises(TypeError):replace(self.selected,**{field:1})
                self.assertFalse(hasattr(self.selected,field))
                self.assertFalse(self.selected.control_root.exists())

    def test_every_derived_launch_bound_is_retained_and_rechecked(self):
        for field in ('timeout_seconds','maximum_output_bytes','manifest_max_bytes','report_max_bytes'):
            with self.subTest(field=field):
                owner=self.prepare(replace(self.selected,control_root=self.root/('controls-'+field)))
                launch=owner.claim_launch()
                object.__setattr__(launch,field,getattr(launch,field)+1)
                with self.assertRaises(m.ScalingProvisioningError):owner.validate()
                owner.declare_child_reaped();owner.close()
                sys.modules.pop('blake3',None);sys.modules.pop('blake3.blake3',None)

    def test_parent_choices_are_copied_and_launch_mutation_rejected(self):
        owner=self.prepare();original=self.selected.evidence_root
        object.__setattr__(self.selected,'evidence_root',self.selected.source_root)
        launch=owner.claim_launch();self.assertEqual(launch.evidence_root,original)
        object.__setattr__(launch,'launch_input_fd',True)
        with self.assertRaises(m.ScalingProvisioningError):owner.validate()
        owner.declare_child_reaped();owner.close()

    def test_output_under_artifact_container_and_private_separation(self):
        owner=self.prepare();self.assertIn(self.selected.artifact_root,self.selected.evidence_root.parents)
        self.assertNotIn(self.selected.evidence_root,self.selected.control_root.parents)
        owner.close()

    def test_existing_or_overlapping_outputs_reject(self):
        for field,path in (('evidence_root',self.selected.artifact_root),
            ('evidence_root',self.selected.source_root/'nested'),('runtime_root',self.selected.evidence_root/'runtime'),
            ('control_root',self.selected.evidence_root/'controls')):
            with self.subTest(field=field,path=str(path)):
                with self.assertRaises(m.ScalingProvisioningError):self.prepare(replace(self.selected,**{field:path}))

    def test_freshness_and_original_output_parent_rechecked(self):
        owner=self.prepare();self.selected.evidence_root.mkdir(mode=0o700)
        with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()

    def test_output_parent_rebind_rejected(self):
        owner=self.prepare();artifacts=self.selected.artifact_root
        artifacts.rename(artifacts.with_name('original-artifacts'));artifacts.mkdir(mode=0o700)
        with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()

    def test_original_absolute_deadline_not_renewed(self):
        with patch.object(m.time,'monotonic_ns',return_value=100):owner=self.prepare()
        original=owner._deadline
        with patch.object(m.time,'monotonic_ns',return_value=original):
            with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()
        self.assertEqual(owner._deadline,original)

    def test_source_and_dependency_elapsed_time_consumes_original_derived_scope(self):
        clock=[123];actual=m.python_contract.PythonDependencies.load
        def load(owner):
            result=actual(owner);clock[0]+=37*m._NS;return result
        with patch.object(m.time,'monotonic_ns',side_effect=lambda:clock[0]), \
                patch.object(m.python_contract.PythonDependencies,'load',load):
            owner=self.prepare();launch=owner.claim_launch()
        expected=(self.bounds['experiment_timeout_ns']+m._NS-1)//m._NS+self.selected.observation_overhead_seconds
        self.assertEqual(launch.original_started_ns,123)
        self.assertEqual(launch.deadline_ns,123+expected*m._NS)
        self.assertEqual(launch.deadline_ns-clock[0],(expected-37)*m._NS)
        self.assertEqual(launch.timeout_seconds,expected)
        owner.declare_child_reaped()

    def test_admission_spending_original_budget_rejects_before_launch_publication(self):
        clock=[123];actual=m.python_contract.PythonDependencies.load;captured=[]
        expected=(self.bounds['experiment_timeout_ns']+m._NS-1)//m._NS+self.selected.observation_overhead_seconds
        def load(owner):
            result=actual(owner);captured.append(owner);clock[0]+=expected*m._NS;return result
        with patch.object(m.time,'monotonic_ns',side_effect=lambda:clock[0]), \
                patch.object(m.python_contract.PythonDependencies,'load',load):
            with self.assertRaises(m.ScalingProvisioningError):self.prepare()
        self.assertFalse((self.selected.control_root/'launch.json').exists())
        self.assertFalse(self.selected.evidence_root.exists());self.assertFalse(self.selected.runtime_root.exists())
        self.assertEqual(len(captured),1)
        with self.assertRaises(m.python_contract.ScalingBootstrapError):captured[0].verify()

    def test_fractional_policy_timeout_rounds_up_and_current_budget_owns_output_caps(self):
        plan=json.loads(self.selected.plan_path.read_bytes())
        plan['experiment_timeout_ns']+=1
        plan_raw=json.dumps(plan,sort_keys=True,separators=(',',':')).encode('ascii')
        self.selected.plan_path.write_bytes(plan_raw)
        budget=json.loads(self.selected.budget_path.read_bytes())
        budget['experiment']['manifest']['max_bytes']=524288
        budget['experiment']['report']['max_bytes']=262144
        budget_raw=json.dumps(budget,sort_keys=True,separators=(',',':')).encode('ascii')
        self.selected.budget_path.write_bytes(budget_raw)
        selected=replace(self.selected,plan_sha256=digest(plan_raw),budget_sha256=digest(budget_raw),
                         observation_overhead_seconds=17)
        with patch.object(m.time,'monotonic_ns',return_value=123):
            owner=self.prepare(selected);launch=owner.claim_launch()
        self.assertEqual(launch.timeout_seconds,7018)
        self.assertEqual(launch.deadline_ns,123+7018*m._NS)
        self.assertEqual((launch.manifest_max_bytes,launch.report_max_bytes,launch.maximum_output_bytes),
                         (524288,262144,786432))
        self.assertEqual(owner.verification_inputs().plan_bytes,plan_raw)
        self.assertEqual(owner.verification_inputs().budget_bytes,budget_raw)
        owner.declare_child_reaped()

    def test_reused_launch_slot_preserved_on_cleanup(self):
        owner=self.prepare();launch=owner.claim_launch();fd=launch.launch_input_fd
        foreign=write(self.root/'foreign',b'foreign')
        other=os.open(foreign,os.O_RDONLY|os.O_CLOEXEC);os.dup2(other,fd,inheritable=False);os.close(other)
        try:
            with self.assertRaises(m.ScalingProvisioningError):owner.validate()
            with self.assertRaises(m.ScalingInputsBorrowedError):owner.close()
            owner.declare_child_reaped();owner.close()
            self.assertEqual(os.pread(fd,7,0),b'foreign')
        finally:os.close(fd)

    def test_reused_seed_slot_preserved_on_cleanup(self):
        owner=self.prepare();launch=owner.claim_launch();fd=launch.seed_fd
        foreign=write(self.root/'foreign',b'foreign');other=os.open(foreign,os.O_RDONLY|os.O_CLOEXEC)
        os.dup2(other,fd,inheritable=False);os.close(other)
        try:
            with self.assertRaises(m.ScalingProvisioningError):owner.validate()
            owner.declare_child_reaped();owner.close();self.assertEqual(os.read(fd,7),b'foreign')
        finally:os.close(fd)

    def test_inheritable_launch_slot_preserved(self):
        owner=self.prepare();launch=owner.claim_launch();fd=launch.launch_input_fd
        os.set_inheritable(fd,True)
        try:
            with self.assertRaises(m.ScalingProvisioningError):owner.validate()
            owner.declare_child_reaped();owner.close();os.fstat(fd)
        finally:os.close(fd)

    def test_admitted_dependency_same_inode_writable_reuse_preserved(self):
        owner=self.prepare();launch=owner.claim_launch()
        held=next(row for row in owner._dependencies._held if row['path']==owner._dependencies.paths.source_root/'blake3/__init__.py')
        fd=held['descriptor'];other=os.open(held['path'],os.O_WRONLY|os.O_CLOEXEC)
        os.dup2(other,fd,inheritable=False);os.close(other)
        try:
            with self.assertRaises(m.ScalingProvisioningError):owner.validate()
            owner.declare_child_reaped();owner.close();os.fstat(fd)
        finally:os.close(fd)

    def test_seed_and_failure_details_never_persist(self):
        value=replace(self.selected,environment=(('SEED',SEED),))
        with self.assertRaises(m.ScalingProvisioningError) as error:self.prepare(value)
        self.assertEqual(str(error.exception),'scaling_input_provisioning_failed')
        self.assertNotIn(SEED,str(error.exception));self.assertFalse(value.control_root.exists())

    def test_seed_in_private_path_is_rejected_before_persistence(self):
        value=replace(self.selected,control_root=self.root/SEED)
        with self.assertRaises(m.ScalingProvisioningError):self.prepare(value)
        self.assertFalse(value.control_root.exists())

    def test_source_member_drift_after_admission_poison(self):
        owner=self.prepare();path=self.selected.source_root/'Cargo.lock';path.write_bytes(b'version = 5\n')
        with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()

    def test_worker_and_python_source_membership_rechecked(self):
        owner=self.prepare();write(owner._sources.worker_sources/'extra.py',b'extra')
        with self.assertRaises(m.ScalingProvisioningError):owner.claim_launch()

    def test_replaced_borrowed_owner_still_closes_original_after_reap(self):
        owner=self.prepare();launch=owner.claim_launch();dependency=owner._dependencies
        owner._dependencies=None
        with self.assertRaises(m.ScalingProvisioningError):owner.validate()
        owner.declare_child_reaped();owner.close()
        with self.assertRaises(m.python_contract.ScalingBootstrapError):dependency.verify()
        self.assert_closed(launch.launch_input_fd)

    def test_selection_rejects_bool_bounds_wrong_types_and_noncanonical_paths(self):
        for field,value in (('observation_overhead_seconds',True),('observation_overhead_seconds',0),
            ('observation_overhead_seconds',601),('source_root',str(self.selected.source_root)),
            ('plan_sha256','A'*64),('environment',{'PATH':'/bin'}),('source_revision','A'*40)):
            with self.subTest(field=field):
                with self.assertRaises((m.ScalingProvisioningError,m.python_contract.ScalingBootstrapError)):
                    m._selection(replace(self.selected,**{field:value}))


    def test_verification_accessor_retains_exact_original_policy_and_image_controls(self):
        owner=self.prepare();value=owner.verification_inputs()
        self.assertEqual(value.plan_bytes,self.selected.plan_path.read_bytes())
        self.assertEqual(value.budget_bytes,self.selected.budget_path.read_bytes())
        self.assertEqual(value.kagami,self.selected.binary_bundle/owner._binary['kagami_relative_path'])
        self.assertEqual(value.kagami_sha256,owner._binary['kagami_sha256'])
        self.assertEqual(value.workspace_source_sha256,self.selected.workspace_source_sha256)
        self.assertEqual(value.source_revision,self.selected.source_revision)
        self.assertEqual(dict(value.executable_images)['resource_program'],self.framework['records'][0]['sha256'])
        self.assertEqual(value.worker_sources,tuple((name,(owner._sources.worker_sources/name).stat().st_size,digest((owner._sources.worker_sources/name).read_bytes())) for name in m.python_contract._WORKERS))
        with self.assertRaises(AttributeError):value.plan_bytes=b'changed'
        owner.claim_launch();self.assertEqual(owner.verification_inputs(),value)
        owner.declare_child_reaped();self.assertEqual(owner.verification_inputs(),value)
        owner.close()
        with self.assertRaises(m.ScalingProvisioningError):owner.verification_inputs()

    def test_verification_accessor_supports_partial_original_descriptor_reads(self):
        owner=self.prepare();expected=owner.verification_inputs();original=m.os.pread
        with patch.object(m.os,'pread',side_effect=lambda fd,size,offset:original(fd,min(size,4096),offset)):
            self.assertEqual(owner.verification_inputs(),expected)

    def test_verification_accessor_rejects_substituted_descriptor_and_keeps_foreign_slot(self):
        owner=self.prepare();foreign=os.open(self.selected.plan_path,os.O_RDONLY|os.O_CLOEXEC)
        try:
            owner._policy_fds=(foreign,owner._policy_fds[1])
            with self.assertRaises(m.ScalingProvisioningError):owner.verification_inputs()
            owner.close();os.fstat(foreign)
        finally:os.close(foreign)

    def test_verification_accessor_rejects_mutated_retained_binary_projection(self):
        owner=self.prepare();owner._binary['kagami_sha256']='f'*64
        with self.assertRaises(m.ScalingProvisioningError):owner.verification_inputs()

    def test_verification_accessor_rejects_original_policy_content_drift(self):
        owner=self.prepare();self.selected.plan_path.write_bytes(self.selected.plan_path.read_bytes()+b' ')
        with self.assertRaises(m.ScalingProvisioningError):owner.verification_inputs()


class OriginalFileTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=BASE);self.root=Path(self.temp.name).resolve()
        self.owner=m._OriginalInputs()
    def tearDown(self):
        self.owner.close();self.temp.cleanup()
    def test_oversize_rejected_before_read_and_original_descriptor_closed(self):
        path=write(self.root/'input',b'abcd');opened=[];actual=os.open
        def capture(*args,**kwargs):
            fd=actual(*args,**kwargs);opened.append(fd);return fd
        with patch.object(os,'open',capture),patch.object(os,'pread',side_effect=AssertionError('must not read')):
            with self.assertRaises(m.ScalingProvisioningError):self.owner.hold(path,3)
        with self.assertRaises(OSError):os.fstat(opened[-1])
    def test_error_read_preserves_reused_foreign_descriptor(self):
        path=write(self.root/'input',b'original');foreign=write(self.root/'foreign',b'foreign')
        actual=os.pread;slots=[]
        def reuse(fd,size,offset):
            raw=actual(fd,size,offset);other=os.open(foreign,os.O_RDONLY|os.O_CLOEXEC)
            os.dup2(other,fd,inheritable=False);os.close(other);slots.append(fd);return raw
        with patch.object(os,'pread',reuse):
            with self.assertRaises(m.ScalingProvisioningError):self.owner.hold(path,32)
        self.owner.close()
        try:self.assertEqual(actual(slots[0],7,0),b'foreign')
        finally:os.close(slots[0])
    def test_symlink_and_hardlink_controls_reject(self):
        source=write(self.root/'source',b'source');symlink=self.root/'symlink';symlink.symlink_to(source)
        hard=self.root/'hard';os.link(source,hard)
        for path in (symlink,hard):
            with self.subTest(kind=path.name):
                with self.assertRaises((m.ScalingProvisioningError,OSError)):self.owner.hold(path,32)
