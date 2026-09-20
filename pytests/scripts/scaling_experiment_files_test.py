"""Actual bounded public files and mocked original guards; no native processes."""
from dataclasses import replace
import fcntl
import hashlib
import os
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import pytest

import resource_evidence_budget as b
import scaling_experiment_files as e
import scaling_publication as publication
from resource_bundle import ControlBinding

RAW={'identity':b'{"identity":1}\n','plan':b'{"plan":1}\n','source_closure':b'{"closure":1}\n',
     'manifest':b'{"manifest":1}\n','report':b'{"report":1}\n'}


def budget(static=None,other=(),manifest_cap=256,report_cap=256):
    geometry=b.CaptureGeometry(4,2_000_000,40_000_000,2_000_000)
    runs=tuple(b.RunBudget(pair,variant,geometry,
        *(b.FileBudget(f'p{pair}.{variant}.{role}',128) for role in b.RUN_FILE_FIELDS))
        for pair in range(1,6) for variant in ('one_lane','four_lane'))
    return b.admit_experiment(policy=b.CapturePolicy(1,1),runs=runs,
        static_files=tuple(b.StaticFile(role,len(RAW[role])) for role in e._STATIC) if static is None else static,
        manifest=b.FileBudget('manifest',manifest_cap),report=b.FileBudget('report',report_cap),other_control=other)


@pytest.fixture
def case(tmp_path):
    root=tmp_path.resolve()/'evidence';root.mkdir(mode=0o700)
    fd=os.open(root,e._DIRECTORY);calls=[];box=[]
    def guard():
        calls.append(True)
        if box:box[0].check_namespace()
    original=budget();owner=e.FixedExperimentFiles(root,fd,original,guard);box.append(owner)
    value=SimpleNamespace(root=root,fd=fd,owner=owner,guard=guard,calls=calls,budget=original)
    yield value
    owner.close();os.close(fd)


def statics(c):
    return tuple(c.owner.publish_static(role,RAW[role]) for role in e._STATIC)


def complete(c):
    return (*statics(c),c.owner.publish_manifest(RAW['manifest']),c.owner.publish_report(RAW['report']))


def failed(owner):
    for operation in (owner.verify,owner.check_namespace,lambda:owner.controls):
        with pytest.raises(e.ExperimentFileError,match='^experiment_file_custody_failed$'):operation()


def test_exact_five_controls_and_original_bindings_survive_physical_verification(case):
    c=case;bindings=complete(c)
    assert tuple(item.label for item in bindings)==('identity','plan','source_closure','manifest','report')
    assert tuple(item.path for item in bindings)==('inputs/identity.json','inputs/plan.json',
        'inputs/source_closure.json','manifest.json','report.json')
    assert c.owner.verify()==bindings==c.owner.controls
    for item in bindings:
        assert c.owner.read_control(item,max_bytes=c.owner._caps[item.label])==RAW[item.label]
        assert c.owner.controls[e._ORDER.index(item.label)] is item
        path=c.root/item.path
        assert path.read_bytes()==RAW[item.label] and path.stat().st_nlink==1
        assert path.stat().st_mode&0o777==0o600
    assert c.calls and len(c.owner._chain)<=65
    c.owner.close();assert os.fstat(c.fd).st_ino==c.root.stat().st_ino
    assert all((c.root/item.path).is_file() for item in bindings)
    failed(c.owner)


def test_budget_copy_and_scope_accessors_are_independent_and_complete(case):
    c=case;copy=c.owner.allocation
    assert copy==c.budget and copy is not c.budget
    assert len(copy.runs)==10 and sum(len(run.files) for run in copy.runs)==150
    object.__setattr__(copy.runs[0].native_facts,'max_bytes',1)
    object.__setattr__(c.budget.runs[0].native_facts,'max_bytes',2)
    assert c.owner.allocation.runs[0].native_facts.max_bytes==128
    assert c.owner.directory==c.root
    complete(c);assert len(c.owner.verify())==5


def test_cheap_check_has_no_callback_or_body_read(case,monkeypatch):
    complete(case);before=len(case.calls)
    monkeypatch.setattr(e.os,'pread',lambda *_:(_ for _ in ()).throw(AssertionError('body read')))
    case.owner.check_namespace()
    assert len(case.calls)==before


@pytest.mark.parametrize('kind',['empty_static','missing_static','extra_static','wrong_static','wrong_order','other_control','wrong_manifest'])
def test_complete_budget_has_only_three_fixed_static_and_two_dynamic_roles(tmp_path,kind):
    root=tmp_path.resolve()/'e';root.mkdir(mode=0o700);fd=os.open(root,e._DIRECTORY)
    static=tuple(b.StaticFile(role,len(RAW[role])) for role in e._STATIC);other=()
    if kind=='empty_static':static=(b.StaticFile('identity',0),*static[1:])
    elif kind=='missing_static':static=static[:-1]
    elif kind=='extra_static':static=(*static,b.StaticFile('extra',1))
    elif kind=='wrong_static':static=(b.StaticFile('trial_harness',1),*static[1:])
    elif kind=='wrong_order':static=tuple(reversed(static))
    elif kind=='other_control':other=(b.FileBudget('support',1),)
    allocation=budget(static,other)
    if kind=='wrong_manifest':
        allocation=b.admit_experiment(policy=allocation.policy,runs=allocation.runs,static_files=allocation.static_files,
            manifest=b.FileBudget('legacy_manifest',256),report=allocation.control_budgets[1],other_control=())
    before=set(os.listdir('/dev/fd'))
    try:
        with pytest.raises(e.ExperimentFileError):e.FixedExperimentFiles(root,fd,allocation,lambda:None)
        assert set(os.listdir('/dev/fd'))==before and not (root/'inputs').exists()
        assert os.fstat(fd).st_ino==root.stat().st_ino
    finally:os.close(fd)


@pytest.mark.parametrize('name',['inputs','manifest.json','report.json','extra','trial_harness'])
def test_existing_root_control_names_cannot_be_readmitted(tmp_path,name):
    root=tmp_path.resolve()/'e';root.mkdir(mode=0o700)
    (root/name).write_bytes(b'foreign');fd=os.open(root,e._DIRECTORY);before=set(os.listdir('/dev/fd'))
    try:
        with pytest.raises(e.ExperimentFileError):e.FixedExperimentFiles(root,fd,budget(),lambda:None)
        assert set(os.listdir('/dev/fd'))==before and (root/name).read_bytes()==b'foreign'
    finally:os.close(fd)


@pytest.mark.parametrize('kind',['other_root','regular','inheritable','relative','mode'])
def test_root_must_be_exact_borrowed_original_directory(tmp_path,kind):
    root=tmp_path.resolve()/'e';root.mkdir(mode=0o700)
    other=root.parent/'other';other.mkdir(mode=0o700)
    source=other if kind=='other_root' else root
    if kind=='regular':source=root.parent/'file';source.write_bytes(b'x')
    fd=os.open(source,os.O_RDONLY|os.O_CLOEXEC)
    if kind=='inheritable':os.set_inheritable(fd,True)
    if kind=='mode':root.chmod(0o755)
    before=set(os.listdir('/dev/fd'))
    try:
        with pytest.raises(e.ExperimentFileError):e.FixedExperimentFiles(Path('relative') if kind=='relative' else root,fd,budget(),lambda:None)
        assert set(os.listdir('/dev/fd'))==before
        assert os.fstat(fd).st_ino==source.stat().st_ino
    finally:os.close(fd)


@pytest.mark.parametrize('kind',['plan_first','manifest_first','report_first','unknown','duplicate_static','report_before_manifest','static_after_manifest','second_manifest','second_report'])
def test_each_fixed_publication_is_single_use_and_ordered(case,kind):
    c=case
    if kind=='plan_first':operation=lambda:c.owner.publish_static('plan',RAW['plan'])
    elif kind=='manifest_first':operation=lambda:c.owner.publish_manifest(RAW['manifest'])
    elif kind=='report_first':operation=lambda:c.owner.publish_report(RAW['report'])
    elif kind=='unknown':operation=lambda:c.owner.publish_static('trial_log',b'x')
    elif kind=='duplicate_static':
        c.owner.publish_static('identity',RAW['identity']);operation=lambda:c.owner.publish_static('identity',RAW['identity'])
    else:
        statics(c)
        if kind=='report_before_manifest':operation=lambda:c.owner.publish_report(RAW['report'])
        else:
            c.owner.publish_manifest(RAW['manifest'])
            if kind=='static_after_manifest':operation=lambda:c.owner.publish_static('identity',RAW['identity'])
            elif kind=='second_manifest':operation=lambda:c.owner.publish_manifest(RAW['manifest'])
            else:c.owner.publish_report(RAW['report']);operation=lambda:c.owner.publish_report(RAW['report'])
    before={str(path.relative_to(c.root)):path.read_bytes() for path in c.root.rglob('*') if path.is_file()}
    with pytest.raises(e.ExperimentFileError):operation()
    assert before=={str(path.relative_to(c.root)):path.read_bytes() for path in c.root.rglob('*') if path.is_file()}
    failed(c.owner)


@pytest.mark.parametrize('raw',[b'',b'x',b'x'*100,bytearray(RAW['identity']),memoryview(RAW['identity']),RAW['identity'].decode(),True,None])
def test_static_bytes_are_exact_immutable_preallocated_size_before_write(case,raw):
    with patch.object(publication.os,'pwrite',side_effect=AssertionError('must not write')):
        with pytest.raises(e.ExperimentFileError):case.owner.publish_static('identity',raw)
    assert not tuple((case.root/'inputs').iterdir());failed(case.owner)


@pytest.mark.parametrize('role',['manifest','report'])
def test_dynamic_exact_cap_is_accepted_and_next_byte_is_rejected(tmp_path,role):
    for delta in (0,1):
        root=tmp_path.resolve()/f'e{delta}';root.mkdir(mode=0o700);fd=os.open(root,e._DIRECTORY)
        owner=e.FixedExperimentFiles(root,fd,budget(manifest_cap=32,report_cap=32),lambda:None)
        c=SimpleNamespace(owner=owner);statics(c)
        if role=='report':owner.publish_manifest(b'{}')
        try:
            operation=owner.publish_manifest if role=='manifest' else owner.publish_report
            if delta:
                with pytest.raises(e.ExperimentFileError):operation(b'x'*33)
                assert not (root/f'{role}.json').exists()
            else:
                binding=operation(b'x'*32);assert owner.read_control(binding,max_bytes=32)==b'x'*32
        finally:owner.close();os.close(fd)


@pytest.mark.parametrize('kind',['equal_binding','changed_binding','bool_cap','small_cap','large_cap','mutable_binding'])
def test_read_control_requires_original_binding_and_independent_semantic_cap(case,kind):
    original=case.owner.publish_static('identity',RAW['identity']);binding=original;cap=len(RAW['identity'])
    if kind=='equal_binding':binding=replace(original)
    elif kind=='changed_binding':binding=replace(original,sha256='a'*64)
    elif kind=='bool_cap':cap=True
    elif kind=='small_cap':cap-=1
    elif kind=='large_cap':cap+=1
    else:binding={'label':original.label,'path':original.path,'sha256':original.sha256}
    with patch.object(publication.os,'pread',side_effect=AssertionError('cap/binding before body read')):
        with pytest.raises(e.ExperimentFileError):case.owner.read_control(binding,max_bytes=cap)
    failed(case.owner)


def test_outer_runs_resources_names_are_allowed_but_not_adopted(case):
    statics(case)
    for name in ('runs','resources'):
        (case.root/name).mkdir(mode=0o700)
        (case.root/name/'owned-by-outer').write_bytes(b'outer census authority')
    assert tuple(item.label for item in case.owner.verify())==e._STATIC
    case.owner.publish_manifest(RAW['manifest']);case.owner.publish_report(RAW['report'])
    assert len(case.owner.verify())==5


@pytest.mark.parametrize('kind',['unexpected_root','unexpected_inputs','outer_file','outer_symlink','outer_mode'])
def test_only_fixed_root_namespace_and_outer_directory_types_are_permitted(case,kind):
    statics(case)
    if kind=='unexpected_root':(case.root/'extra').write_bytes(b'x')
    elif kind=='unexpected_inputs':(case.root/'inputs/extra').write_bytes(b'x')
    elif kind=='outer_file':(case.root/'runs').write_bytes(b'x')
    elif kind=='outer_symlink':(case.root/'runs').symlink_to(case.root/'inputs',target_is_directory=True)
    else:(case.root/'resources').mkdir(mode=0o755)
    with pytest.raises(e.ExperimentFileError):case.owner.check_namespace()
    failed(case.owner)


@pytest.mark.parametrize('kind',['bytes','same_bytes','inode','hardlink','mode','ancestor','binding'])
def test_retained_original_file_and_ancestor_mutation_is_permanent(case,kind):
    bindings=complete(case);path=case.root/bindings[0].path
    if kind=='bytes':path.write_bytes(b'x'*path.stat().st_size)
    elif kind=='same_bytes':path.write_bytes(path.read_bytes())
    elif kind=='inode':
        raw=path.read_bytes();path.unlink();path.write_bytes(raw);path.chmod(0o600)
    elif kind=='hardlink':os.link(path,case.root.parent/'hardlink')
    elif kind=='mode':path.chmod(0o644)
    elif kind=='ancestor':case.root.rename(case.root.with_name('moved'));case.root.mkdir(mode=0o700)
    else:object.__setattr__(bindings[0],'sha256','a'*64)
    with pytest.raises(e.ExperimentFileError):case.owner.verify()
    failed(case.owner)


def test_later_file_read_cannot_hide_an_earlier_in_place_mutation(case,monkeypatch):
    bindings=complete(case);first=case.root/bindings[0].path;last=case.owner._files['report'].fd
    original=os.pread;changed=[]
    def reading(fd,count,offset):
        raw=original(fd,count,offset)
        if fd==last and not changed:changed.append(True);first.write_bytes(first.read_bytes())
        return raw
    monkeypatch.setattr(e.os,'pread',reading)
    with pytest.raises(e.ExperimentFileError):case.owner.verify()
    assert changed;failed(case.owner)


def test_stage_cursor_change_uses_same_shared_positional_publisher(case,monkeypatch):
    stage=case.root/'inputs/identity.json.publishing';original=publication.os.pwrite;offsets=[]
    def writing(fd,raw,offset):
        os.lseek(fd,100000,os.SEEK_SET);offsets.append(offset)
        return original(fd,raw,offset)
    monkeypatch.setattr(publication.os,'pwrite',writing)
    binding=case.owner.publish_static('identity',RAW['identity'])
    assert offsets==[0] and not stage.exists()
    assert (case.root/binding.path).stat().st_size==len(RAW['identity'])


def test_replaced_construction_guard_is_rejected(case):
    case.owner._guard_callback=lambda:None
    with pytest.raises(e.ExperimentFileError):case.owner.publish_static('identity',RAW['identity'])
    assert not tuple((case.root/'inputs').iterdir());failed(case.owner)


def test_caught_guard_reentry_cannot_publish(case):
    owner=case.owner;attempted=[];original=owner._guard_callback
    def guard():
        original()
        if not attempted:
            attempted.append(True)
            with pytest.raises(e.ExperimentFileError):owner.verify()
    # Test installs one admitted callback before the operation; its own reentry
    # must poison even when it catches the inner exception.
    owner._guard_callback=owner._original_guard=guard
    with pytest.raises(e.ExperimentFileError):owner.publish_static('identity',RAW['identity'])
    assert attempted;failed(owner)


def test_original_deadline_callback_failure_is_terminal_after_publication(case):
    owner=case.owner;original=owner._guard_callback;expired=[]
    def guard():
        original()
        if expired:raise RuntimeError('private deadline detail')
    owner._guard_callback=owner._original_guard=guard
    complete(case);expired.append(True)
    with pytest.raises(e.ExperimentFileError,match='^experiment_file_custody_failed$'):owner.verify()
    failed(owner)


def test_reused_owned_descriptor_is_not_closed_as_foreign(case):
    complete(case);target=case.owner._files['identity'].fd
    path=case.root.parent/'foreign';path.write_bytes(b'foreign');foreign=os.open(path,os.O_RDONLY)
    try:
        os.dup2(foreign,target,inheritable=False)
        with pytest.raises(e.ExperimentFileError):case.owner.verify()
        case.owner.close()
        assert os.pread(target,7,0)==b'foreign'
    finally:os.close(target);os.close(foreign)


def test_failed_created_input_directory_preserves_foreign_reused_fd(case,monkeypatch):
    # A fresh instance is needed because inputs is create-once.
    root=case.root.parent/'second';root.mkdir(mode=0o700);fd=os.open(root,e._DIRECTORY)
    foreign_path=root.parent/'foreign';foreign_path.write_bytes(b'foreign');foreign=os.open(foreign_path,os.O_RDONLY)
    original_open=os.open;created=[];switched=[]
    def opening(path,*args,**kwargs):
        output=original_open(path,*args,**kwargs)
        if path=='inputs':created.append(output)
        return output
    def sync(_):
        os.dup2(foreign,created[0],inheritable=False);switched.append(True)
        raise OSError('private sync detail')
    try:
        with patch.object(e.os,'open',opening),patch.object(e.os,'fsync',sync):
            with pytest.raises(e.ExperimentFileError):e.FixedExperimentFiles(root,fd,budget(),lambda:None)
        assert switched and os.pread(created[0],7,0)==b'foreign'
    finally:
        for item in created:os.close(item)
        os.close(fd);os.close(foreign)


@pytest.mark.parametrize('exception',[RuntimeError,KeyboardInterrupt,SystemExit,GeneratorExit])
def test_constructor_guard_failures_release_owned_fds_and_redact_details(tmp_path,exception):
    root=tmp_path.resolve()/'e';root.mkdir(mode=0o700);fd=os.open(root,e._DIRECTORY)
    before=set(os.listdir('/dev/fd'))
    def guard():raise exception('PRIVATE INPUT DETAIL')
    expected=e.ExperimentFileError if exception is RuntimeError else exception
    try:
        with pytest.raises(expected) as caught:e.FixedExperimentFiles(root,fd,budget(),guard)
        assert 'PRIVATE' not in str(caught.value)
        if exception is SystemExit:assert caught.value.code==1
        assert set(os.listdir('/dev/fd'))==before and os.fstat(fd).st_ino==root.stat().st_ino
    finally:os.close(fd)


def test_multichunk_partial_publication_and_narrow_read_keep_exact_counted_offsets(tmp_path,monkeypatch):
    root=tmp_path.resolve()/'e';root.mkdir(mode=0o700);fd=os.open(root,e._DIRECTORY)
    raw=b'x'*(2*65536+1)
    static=(b.StaticFile('identity',len(raw)),*(b.StaticFile(role,len(RAW[role])) for role in e._STATIC[1:]))
    owner=e.FixedExperimentFiles(root,fd,budget(static),lambda:None)
    original_write=os.pwrite;writes=[]
    def writing(file,chunk,offset):
        writes.append((len(chunk),offset))
        return original_write(file,chunk[:1] if len(writes)==1 else chunk,offset)
    try:
        monkeypatch.setattr(publication.os,'pwrite',writing)
        binding=owner.publish_static('identity',raw)
        assert writes==[(65536,0),(65535,1),(65536,65536),(1,131072)]
        assert (root/binding.path).read_bytes()==raw
        reads=[];original_read=os.pread
        def reading(file,count,offset):reads.append((count,offset));return original_read(file,count,offset)
        monkeypatch.setattr(publication.os,'pread',reading)
        assert owner.read_control(binding,max_bytes=len(raw))==raw
        assert reads==[(65536,0),(65536,65536),(1,131072)]
    finally:owner.close();os.close(fd)


def test_last_external_guard_cannot_change_earlier_file_after_complete_hash_batch(case,monkeypatch):
    bindings=complete(case);owner=case.owner;digested=[];changed=[]
    original_digest=e.digest_file;original_guard=owner._guard_callback
    def digest(*args):result=original_digest(*args);digested.append(True);return result
    def guard():
        original_guard()
        if len(digested)==5 and not changed:
            path=case.root/bindings[0].path;path.write_bytes(path.read_bytes());changed.append(True)
    owner._guard_callback=owner._original_guard=guard
    monkeypatch.setattr(e,'digest_file',digest)
    with pytest.raises(e.ExperimentFileError):owner.verify()
    assert len(digested)==5 and changed;failed(owner)
