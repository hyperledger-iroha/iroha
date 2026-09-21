"""Real staged-file/publication controls; no signing or SDK qualification."""
from __future__ import annotations

import hashlib
import io
import os
from pathlib import Path
import sys
import zipfile

import pytest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
import sorafs_python_publication as publication

BUNDLE=b'original captured runtime bundle bytes'
BUFFER=io.BytesIO()
with zipfile.ZipFile(BUFFER,'w') as archive:
    archive.writestr('observation.txt',b'actual retained observation fixture')
ARCHIVE=BUFFER.getvalue()


def stage(pub):
    with pub.runtime_stream() as stream:
        assert stream.write(BUNDLE[:10])==10
        assert stream.write(BUNDLE[10:])==len(BUNDLE)-10
    assert pub.read_runtime_bundle(expected_sha256=hashlib.sha256(BUNDLE).hexdigest(),expected_size=len(BUNDLE))==BUNDLE
    pub.stage_execution_archive(ARCHIVE)


def finals(work):
    return work/'python-runtime-inputs.bundle',work/'python-consumer.zip'


def test_actual_staged_readback_then_final_check_and_zip_last(tmp_path,monkeypatch):
    original=os.link; observed=[]; calls=[]
    def linked(source,destination,**kwargs):
        observed.append(destination)
        return original(source,destination,**kwargs)
    monkeypatch.setattr(publication.os,'link',linked)
    monkeypatch.setattr(publication.os,'supports_dir_fd',os.supports_dir_fd|{linked})
    monkeypatch.setattr(publication.os,'supports_follow_symlinks',os.supports_follow_symlinks|{linked})
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub)
        descriptors=(pub.runtime.fd,pub.archive.fd)
        def check():
            calls.append('checked')
            assert all(not path.exists() for path in finals(tmp_path))
            assert all(os.fstat(fd).st_nlink==1 for fd in descriptors)
        result=pub.publish(check_originals=check)
        assert result.runtime_bundle.path.read_bytes()==BUNDLE
        assert result.execution_archive.path.read_bytes()==ARCHIVE
        assert result.runtime_bundle.sha256==hashlib.sha256(BUNDLE).hexdigest()
        assert result.execution_archive.sha256==hashlib.sha256(ARCHIVE).hexdigest()
        assert all(path.stat().st_nlink==1 for path in finals(tmp_path))
        assert all(os.fstat(fd).st_nlink==1 for fd in descriptors)
    assert calls==['checked'] and observed==['python-runtime-inputs.bundle','python-consumer.zip']
    assert not tuple(tmp_path.glob('.*.pending'))
    for fd in descriptors:
        with pytest.raises(OSError): os.fstat(fd)


def test_final_original_recheck_failure_leaves_no_completed_names(tmp_path):
    with pytest.raises(RuntimeError,match='original changed'):
        with publication.PythonArtifactPublication(tmp_path) as pub:
            stage(pub)
            def fail(): raise RuntimeError('original changed')
            pub.publish(check_originals=fail)
    assert all(not path.exists() for path in finals(tmp_path))
    assert (tmp_path/'.python-runtime-inputs.bundle.pending').read_bytes()==BUNDLE
    assert (tmp_path/'.python-consumer.zip.pending').read_bytes()==ARCHIVE


@pytest.mark.parametrize('member',('runtime','archive'))
@pytest.mark.parametrize('mutation',('replace','inplace','same_bytes','hardlink'))
def test_final_check_cannot_reseal_changed_pending_output(tmp_path,member,mutation):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub); pending=getattr(pub,member); path=tmp_path/pending.pending
        raw=path.read_bytes()
        def change():
            if mutation=='replace': path.unlink(); path.write_bytes(raw)
            elif mutation=='inplace': path.write_bytes(b'x'*len(raw))
            elif mutation=='same_bytes': path.write_bytes(raw)
            else: os.link(path,tmp_path/'alias')
        with pytest.raises(publication.PublicationError): pub.publish(check_originals=change)
    assert all(not path.exists() for path in finals(tmp_path))


@pytest.mark.parametrize('name',('python-runtime-inputs.bundle','python-consumer.zip'))
@pytest.mark.parametrize('kind',('file','directory','dangling_symlink'))
def test_existing_completed_name_is_never_replaced(tmp_path,name,kind):
    path=tmp_path/name
    if kind=='file': path.write_bytes(b'foreign')
    elif kind=='directory': path.mkdir()
    else: path.symlink_to(tmp_path/'absent')
    with pytest.raises(publication.PublicationError,match='already exists'):
        with publication.PythonArtifactPublication(tmp_path): pass
    assert os.path.lexists(path)
    if kind=='file': assert path.read_bytes()==b'foreign'
    assert not tuple(tmp_path.glob('.*.pending'))


def test_final_name_created_during_input_check_is_not_removed(tmp_path):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub)
        def change(): finals(tmp_path)[1].write_bytes(b'foreign')
        with pytest.raises(publication.PublicationError,match='already exists'):
            pub.publish(check_originals=change)
    assert finals(tmp_path)[1].read_bytes()==b'foreign'
    assert not finals(tmp_path)[0].exists()


@pytest.mark.parametrize('race',('error','foreign_collision','error_after_link'))
def test_partial_link_publication_rolls_back_only_owned_finals(tmp_path,monkeypatch,race):
    original=os.link
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub)
        def fail(source,destination,**kwargs):
            if destination=='python-consumer.zip':
                if race=='foreign_collision': finals(tmp_path)[1].write_bytes(b'foreign')
                elif race=='error_after_link':
                    original(source,destination,**kwargs)
                    raise OSError('injected post-link failure')
                else: raise OSError('injected link failure')
            return original(source,destination,**kwargs)
        monkeypatch.setattr(publication.os,'link',fail)
        with pytest.raises(OSError): pub.publish(check_originals=lambda:None)
    assert not finals(tmp_path)[0].exists()
    if race=='foreign_collision': assert finals(tmp_path)[1].read_bytes()==b'foreign'
    else: assert not finals(tmp_path)[1].exists()


def test_publish_has_no_automatic_post_publication_recheck(tmp_path,monkeypatch):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub); calls=[]
        pub.publish(check_originals=lambda:calls.append('once'))
        def forbidden(): raise AssertionError('post-publication recheck')
        monkeypatch.setattr(pub,'_lineage',forbidden)
    assert calls==['once'] and all(path.exists() for path in finals(tmp_path))


def test_runtime_readback_never_reopens_pending_leaf(tmp_path,monkeypatch):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        with pub.runtime_stream() as stream: stream.write(BUNDLE)
        original=os.open
        def guarded(path,*args,**kwargs):
            assert not str(path).endswith('.pending'),'pending leaf was reopened'
            return original(path,*args,**kwargs)
        monkeypatch.setattr(publication.os,'open',guarded)
        assert pub.read_runtime_bundle(expected_sha256=hashlib.sha256(BUNDLE).hexdigest(),expected_size=len(BUNDLE))==BUNDLE


@pytest.mark.parametrize('field',('sha256','size','unsealed'))
def test_original_runtime_identity_is_mandatory(tmp_path,field):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        with pub.runtime_stream() as stream: stream.write(BUNDLE)
        pub.stage_execution_archive(ARCHIVE)
        if field=='unsealed':
            with pytest.raises(publication.PublicationError): pub.publish(check_originals=lambda:None)
        else:
            kwargs=dict(expected_sha256=hashlib.sha256(BUNDLE).hexdigest(),expected_size=len(BUNDLE))
            kwargs['expected_'+field]='a'*64 if field=='sha256' else len(BUNDLE)+1
            with pytest.raises(publication.PublicationError): pub.read_runtime_bundle(**kwargs)
    assert all(not path.exists() for path in finals(tmp_path))


def test_original_runtime_identity_cannot_be_rebound(tmp_path):
    with publication.PythonArtifactPublication(tmp_path) as pub:
        stage(pub)
        with pytest.raises(publication.PublicationError,match='rebound'):
            pub.read_runtime_bundle(expected_sha256='a'*64,expected_size=len(BUNDLE))


def test_pending_stream_limit_fails_without_unbounded_write(tmp_path,monkeypatch):
    monkeypatch.setattr(publication,'MAX_BUNDLE_BYTES',8)
    with publication.PythonArtifactPublication(tmp_path) as pub:
        with pytest.raises(publication.PublicationError,match='byte bound'):
            with pub.runtime_stream() as stream: stream.write(b'x'*9)
    assert (tmp_path/'.python-runtime-inputs.bundle.pending').stat().st_size==0
    assert all(not path.exists() for path in finals(tmp_path))


def test_parent_replacement_refuses_publication(tmp_path):
    work=tmp_path/'work'; work.mkdir()
    with publication.PythonArtifactPublication(work) as pub:
        stage(pub)
        def change(): work.rename(tmp_path/'old-work'); work.mkdir()
        with pytest.raises(publication.PublicationError,match='parent was replaced'):
            pub.publish(check_originals=change)
    assert all(not path.exists() for path in finals(work))
    assert all(not path.exists() for path in finals(tmp_path/'old-work'))


def test_one_shot_context_and_publication(tmp_path):
    pub=publication.PythonArtifactPublication(tmp_path)
    with pub:
        stage(pub); pub.publish(check_originals=lambda:None)
        with pytest.raises(publication.PublicationError): pub.publish(check_originals=lambda:None)
    pub.close()
    with pytest.raises(publication.PublicationError): pub.__enter__()


@pytest.mark.parametrize('member',('runtime','archive'))
@pytest.mark.parametrize('mutation',('bytes','bytes_restore_mtime','metadata','parent'))
def test_change_during_link_cannot_complete(tmp_path,monkeypatch,member,mutation):
    work=tmp_path/'work'; work.mkdir(); original=os.link
    with publication.PythonArtifactPublication(work) as pub:
        stage(pub); pending=getattr(pub,member)
        def changed(source,destination,**kwargs):
            result=original(source,destination,**kwargs)
            if destination==pending.name:
                path=work/pending.pending
                if mutation=='parent': work.rename(tmp_path/'old'); work.mkdir()
                elif mutation=='metadata': path.chmod(0o400)
                else:
                    before=path.stat(); path.write_bytes(b'x'*before.st_size)
                    if mutation=='bytes_restore_mtime':
                        os.utime(path,ns=(before.st_atime_ns,before.st_mtime_ns))
            return result
        monkeypatch.setattr(publication.os,'link',changed)
        with pytest.raises(publication.PublicationError): pub.publish(check_originals=lambda:None)
    assert all(not path.exists() for path in finals(work))
    assert all(not path.exists() for path in finals(tmp_path/'old'))
