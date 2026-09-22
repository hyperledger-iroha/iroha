"""Pure original-content and real POSIX tree controls; no SDK/native execution."""
from __future__ import annotations

from dataclasses import FrozenInstanceError, replace
import hashlib
import base64
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_installed as installed
import sorafs_javascript_installed_custody as custody
from sorafs_javascript_archive import ArchiveError, MAX_NAME_BYTES
from sorafs_javascript_package_fixtures import CHECKSUM, archive_bytes, expected_files, source_inputs
from sorafs_javascript_dependencies_test import _originals


@pytest.fixture(scope="module")
def originals():
    sources, _ = source_inputs()
    lock, dependencies = _originals()
    sources["package-lock.json"] = lock.raw
    return archive_bytes(expected_files(sources)), sources, lock, dependencies


def hidden_lock(originals):
    raw, sources, lock, _ = originals
    package=json.loads(sources["package.json"]); original=json.loads(lock.raw)
    sdk={"version":lock.sdk_version,"resolved":"file:../originals/sdk.tgz",
         "integrity":"sha512-"+base64.b64encode(hashlib.sha512(raw).digest()).decode(),
         "dependencies":package["dependencies"],"engines":package["engines"]}
    return json.dumps({"name":"iroha-sorafs-javascript-consumer","version":"1.0.0",
        "lockfileVersion":3,"requires":True,"packages":{"node_modules/@iroha/iroha-js":sdk,
        **{row.location:original["packages"][row.location] for row in lock.dependencies}}}).encode()


def project(originals, **overrides):
    raw, sources, lock, dependencies = originals
    return installed.installed_projection(overrides.pop("raw", raw), sources=overrides.pop("sources", sources),
        checksum_manifest=overrides.pop("checksum", CHECKSUM), lock=overrides.pop("lock", lock),
        dependency_originals=overrides.pop("dependencies", dependencies),
        npm_hidden_lock=overrides.pop("hidden",hidden_lock(originals)),
        environment_label=overrides.pop("environment","/owned/environment"),
        sdk_archive_label=overrides.pop("sdk_label","/owned/originals/sdk.tgz"))


@pytest.fixture(scope="module")
def projection(originals):
    return project(originals)


@pytest.fixture
def tree(tmp_path, projection):
    root = tmp_path / "environment" / "node_modules"
    root.mkdir(parents=True, mode=0o700)
    for row in projection.members:
        path = root / row.path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(row.content)
        path.chmod(row.mode)
    return root.resolve()


def test_exact_global_projection_preserves_original_bytes_modes_and_nested_owners(projection):
    assert len(projection.members) == 218
    rows = {row.path: row for row in projection.members}
    for path, owner in (("@noble/hashes/index.js", "node_modules/@noble/hashes"),
                        ("@scure/bip39/node_modules/@noble/hashes/index.js", "node_modules/@scure/bip39/node_modules/@noble/hashes")):
        assert rows[path].content == owner.encode()
        assert rows[path].owner == owner
        assert rows[path].archive_member == "index.js"
    assert rows["@iroha/iroha-js/native/iroha_js_host.checksums.json"].content == CHECKSUM
    assert all(row.mode == 0o644 for row in rows.values())
    assert list(rows) == sorted(rows)
    with pytest.raises(FrozenInstanceError): projection.members = ()
    with pytest.raises(FrozenInstanceError): projection.members[0].content = b"forged"
    installed.verify_installed_content(projection, files=projection.files(),
                                      modes={row.path: row.mode for row in projection.members})


def test_projection_and_captured_content_verification_do_no_io(originals, monkeypatch):
    def forbidden(*args, **kwargs): raise AssertionError("pure installed content attempted I/O")
    with monkeypatch.context() as patch:
        patch.setattr("builtins.open", forbidden); patch.setattr(Path, "open", forbidden)
        patch.setattr(os, "open", forbidden); patch.setattr(subprocess, "Popen", forbidden)
        result = project(originals)
        installed.verify_installed_content(result, files=result.files(),
                                          modes={row.path: row.mode for row in result.members})


@pytest.mark.parametrize("name", ("@iroha/iroha-js/dist/native.js", "@noble/hashes/index.js",
    "@scure/bip39/node_modules/@noble/hashes/index.js", "@iroha/iroha-js/native/iroha_js_host.checksums.json"))
@pytest.mark.parametrize("change", ("bytes", "mode", "missing"))
def test_exact_installed_content_cannot_substitute_a_declared_member(projection, name, change):
    files=projection.files(); modes={row.path:row.mode for row in projection.members}
    if change=="bytes": files[name]+=b"substitute"
    if change=="mode": modes[name]=0o755
    if change=="missing": files.pop(name)
    with pytest.raises(ArchiveError): installed.verify_installed_content(projection,files=files,modes=modes)


@pytest.mark.parametrize("name", ("foreign.package-lock.json", ".npmrc", ".cache/data", "@iroha/iroha-js/src/native.js",
    "@iroha/iroha-js/native/iroha_js_host.node", "@noble/Hashes/index.js", "@noble/node_modules/@noble/hashes/index.js"))
def test_unowned_installed_entries_are_never_ignored(projection, name):
    files=projection.files();modes={row.path:row.mode for row in projection.members}
    files[name]=b"inert";modes[name]=0o644
    with pytest.raises(ArchiveError,match="inventory"): installed.verify_installed_content(projection,files=files,modes=modes)


@pytest.mark.parametrize("field,value", (("path","foreign/index.js"),("owner","node_modules/foreign"),
    ("archive_member","other.js"),("content",b"forged"),("mode",0o755)))
def test_frozen_result_rederives_actual_originals_before_live_use(projection, field, value):
    changed=replace(projection,members=(replace(projection.members[0],**{field:value}),*projection.members[1:]))
    with pytest.raises(ArchiveError,match="projection differs"): installed.validate_projection(changed)


@pytest.mark.parametrize("change", ("source_duplicate","source_order","member_order","member_duplicate","raw","checksum","dependency"))
def test_forged_projection_cannot_replace_or_collapse_original_input_owners(projection, change):
    changes={}
    if change=="source_duplicate": changes['sources']=(*projection.sources,projection.sources[0])
    if change=="source_order": changes['sources']=tuple(reversed(projection.sources))
    if change=="member_order": changes['members']=tuple(reversed(projection.members))
    if change=="member_duplicate": changes['members']=(*projection.members,projection.members[0])
    if change=="raw": changes['package_raw']=projection.package_raw+b'\0'
    if change=="checksum": changes['checksum_manifest']=b'{}'
    if change=="dependency": changes['dependency_originals']=projection.dependency_originals[:-1]
    with pytest.raises(ArchiveError): installed.validate_projection(replace(projection,**changes))


@pytest.mark.parametrize("kind", ("files","bytes","depth"))
def test_projection_exact_resource_capacity_and_one_below(originals, projection, monkeypatch, kind):
    name,value={'files':('MAX_INSTALLED_FILES',len(projection.members)),
                'bytes':('MAX_INSTALLED_BYTES',sum(len(row.content) for row in projection.members)),
                'depth':('MAX_DIRECTORY_DEPTH',max(len(row.path.split('/')) for row in projection.members))}[kind]
    monkeypatch.setattr(installed,name,value);assert project(originals)==projection
    monkeypatch.setattr(installed,name,value-1)
    with pytest.raises(ArchiveError):project(originals)


def test_live_tree_holds_real_root_and_ancestors_and_rechecks_without_changing_tree(tree,projection):
    owner=custody.OriginalInstalledTree(tree,projection)
    with owner:
        handles=owner._parent[2]
        assert len(handles)==len(tree.parts)
        assert (os.fstat(handles[-1]).st_dev,os.fstat(handles[-1]).st_ino)==(tree.stat().st_dev,tree.stat().st_ino)
        assert len(owner._state)==len(projection.members)+len(owner._directories)
        owner.recheck()
        assert all(os.fstat(fd) for fd in handles)
    for fd in handles:
        with pytest.raises(OSError):os.fstat(fd)
    owner.close()
    with pytest.raises(ArchiveError):owner.recheck()
    with pytest.raises(ArchiveError):owner.__enter__()


@pytest.mark.parametrize("change",("missing","extra","empty_directory","symlink_file","symlink_directory","fifo","hardlink","mode","directory_mode","bytes"))
def test_initial_live_inventory_refuses_aliases_special_files_and_unowned_entries(tree,projection,change):
    leaf=tree/'@noble/hashes/index.js';directory=tree/'@noble/hashes'
    if change=='missing':leaf.unlink()
    if change=='extra':(tree/'foreign.lock').write_bytes(b'{}')
    if change=='empty_directory':(tree/'foreign').mkdir()
    if change=='symlink_file':leaf.unlink();leaf.symlink_to(tree/'@scure/base/index.js')
    if change=='symlink_directory':directory.rename(tree.parent/'moved');directory.symlink_to(tree.parent/'moved')
    if change=='fifo':leaf.unlink();os.mkfifo(leaf)
    if change=='hardlink':leaf.unlink();os.link(tree/'@scure/base/index.js',leaf)
    if change=='mode':leaf.chmod(0o600)
    if change=='directory_mode':directory.chmod(0o777)
    if change=='bytes':leaf.write_bytes(b'substitute')
    owner=custody.OriginalInstalledTree(tree,projection)
    with pytest.raises((ArchiveError,OSError,RuntimeError)):owner.__enter__()
    assert owner._parent is None


@pytest.mark.parametrize("change",("same_byte_leaf_replace","leaf_bytes","leaf_mode","extra","empty_directory","subdirectory_replace","root_replace","ancestor_replace"))
def test_final_recheck_rejects_tree_identity_drift_and_permanently_poisons(tree,projection,change):
    owner=custody.OriginalInstalledTree(tree,projection).__enter__()
    try:
        leaf=tree/'@noble/hashes/index.js'
        if change=='same_byte_leaf_replace':
            old=leaf.read_bytes();leaf.rename(tree.parent/'old-leaf');leaf.write_bytes(old)
        if change=='leaf_bytes':leaf.write_bytes(b'changed')
        if change=='leaf_mode':leaf.chmod(0o600)
        if change=='extra':(tree/'.npmrc').write_bytes(b'')
        if change=='empty_directory':(tree/'extra').mkdir()
        if change=='subdirectory_replace':
            directory=leaf.parent;directory.rename(tree.parent/'old-subtree');shutil.copytree(tree.parent/'old-subtree',directory)
        if change=='root_replace':tree.rename(tree.parent/'old-root');shutil.copytree(tree.parent/'old-root',tree)
        if change=='ancestor_replace':
            parent=tree.parent;parent.rename(parent.parent/'old-environment');shutil.copytree(parent.parent/'old-environment',parent)
        with pytest.raises((ArchiveError,OSError,RuntimeError)):owner.recheck()
        with pytest.raises(ArchiveError,match='previously failed'):owner.recheck()
    finally:owner.close()


def test_no_follow_open_rejects_last_moment_leaf_substitution(tree,projection,monkeypatch):
    original=os.open; leaf=tree/'@noble/hashes/index.js';fired=False
    def changed(path,flags,*args,**kwargs):
        nonlocal fired
        if path=='index.js' and kwargs.get('dir_fd') is not None and not fired:
            fired=True
            # Replace a different first index leaf's named slot using its actual parent descriptor.
            parent=kwargs['dir_fd'];os.unlink(path,dir_fd=parent);os.symlink('/dev/null',path,dir_fd=parent)
        return original(path,flags,*args,**kwargs)
    monkeypatch.setattr(os,'open',changed)
    with pytest.raises((ArchiveError,OSError,RuntimeError)):
        with custody.OriginalInstalledTree(tree,projection):pass
    assert fired


def test_real_read_error_closes_transient_and_held_descriptors(tree,projection,monkeypatch):
    original=os.open;opened=[]
    def tracked(*args,**kwargs):
        fd=original(*args,**kwargs);opened.append(fd);return fd
    def broken(*args,**kwargs):raise OSError('injected read failure')
    monkeypatch.setattr(os,'open',tracked);monkeypatch.setattr(os,'read',broken)
    owner=custody.OriginalInstalledTree(tree,projection)
    with pytest.raises(OSError):owner.__enter__()
    assert opened and owner._parent is None
    for fd in set(opened):
        with pytest.raises(OSError):os.fstat(fd)


@pytest.mark.parametrize('change',('missing','extra','wrong_name','wrong_version','requires_integer','format_float',
    'source_uri','sdk_integrity','nested_swapped','root_row','unknown_field','foreign_sdk_dependency'))
def test_generated_hidden_lock_exact_graph_cannot_be_resealed(originals,change):
    value=json.loads(hidden_lock(originals));packages=value['packages'];sdk=packages['node_modules/@iroha/iroha-js']
    if change=='missing':packages.pop('node_modules/@noble/hashes')
    if change=='extra':packages['node_modules/extra']={'version':'1.0.0'}
    if change=='wrong_name':value['name']='unreviewed-consumer'
    if change=='wrong_version':value['version']='2.0.0'
    if change=='requires_integer':value['requires']=1
    if change=='format_float':value['lockfileVersion']=3.0
    if change=='source_uri':sdk['resolved']='file:/foreign/sdk.tgz'
    if change=='sdk_integrity':sdk['integrity']='sha512-'+base64.b64encode(bytes(64)).decode()
    if change=='nested_swapped':packages['node_modules/@scure/bip39/node_modules/@noble/hashes']=packages['node_modules/@noble/hashes']
    if change=='root_row':packages['']={'name':'extra'}
    if change=='unknown_field':sdk['hasInstallScript']=True
    if change=='foreign_sdk_dependency':sdk['dependencies']['foreign']='*'
    with pytest.raises(ArchiveError,match='hidden lock'):project(originals,hidden=json.dumps(value).encode())


@pytest.mark.parametrize('value',('',None,False,'relative/path','//ambiguous/root','/root/../other','/root/./other',
    '/root//other','/root/trailing/','/root/space here','/root/%20','/root/\ud800','/'+'x'*4096))
def test_npm_path_labels_are_bounded_pure_canonical_values(originals,value):
    with pytest.raises(ArchiveError):project(originals,environment=value)


def test_consumer_bootstrap_is_fixed_and_does_not_open_historical_path(monkeypatch):
    from sorafs_javascript_install_metadata import consumer_package
    def forbidden(*args,**kwargs):raise AssertionError('opened historical npm input label')
    monkeypatch.setattr(Path,'open',forbidden);monkeypatch.setattr(os,'open',forbidden)
    actual=consumer_package('/historical/originals/sdk.tgz')
    assert json.loads(actual)=={'name':'iroha-sorafs-javascript-consumer','version':'1.0.0','private':True,
        'type':'module','dependencies':{'@iroha/iroha-js':'file:///historical/originals/sdk.tgz'}}


@pytest.mark.parametrize('raw',(b'{}',b'{"name":"x","name":"y"}',b'[]',b'\xff',b'{"x":NaN}'))
def test_hidden_lock_uses_the_sole_closed_duplicate_free_json_owner(originals,raw):
    with pytest.raises(ArchiveError):project(originals,hidden=raw)


def test_file_growth_during_read_is_bounded_and_invalidates_owner(tree,projection,monkeypatch):
    original=os.read;grown=False
    def append(descriptor,size):
        nonlocal grown
        if not grown:
            grown=True
            # The first file is the hidden lock. A second descriptor appends while it is read.
            with (tree/'.package-lock.json').open('ab') as out:out.write(b'x')
        return original(descriptor,size)
    monkeypatch.setattr(os,'read',append)
    with pytest.raises(ArchiveError):
        with custody.OriginalInstalledTree(tree,projection):pass
    assert grown


def test_closed_owner_cannot_be_entered_even_before_first_capture(tree,projection):
    owner=custody.OriginalInstalledTree(tree,projection);owner.close()
    with pytest.raises(ArchiveError):owner.__enter__()


def test_root_symlink_is_refused_even_when_all_bytes_match(tree,projection,tmp_path):
    alias=tmp_path/'alias';alias.symlink_to(tree,target_is_directory=True)
    with pytest.raises((ArchiveError,OSError,RuntimeError)):
        with custody.OriginalInstalledTree(alias,projection):pass


def test_cleanup_preserves_an_original_exception(tree,projection):
    owner=custody.OriginalInstalledTree(tree,projection)
    with pytest.raises(LookupError,match='original failure'):
        with owner:
            handles=owner._parent[2]
            raise LookupError('original failure')
    for fd in handles:
        with pytest.raises(OSError):os.fstat(fd)


@pytest.mark.parametrize("kind",("mutable_bytes","float_mode","list_members","dict_sources"))
def test_value_equal_but_mutable_or_foreign_projection_types_reject(projection,kind):
    changes={}
    if kind=="mutable_bytes":changes['members']=(replace(projection.members[0],content=bytearray(projection.members[0].content)),*projection.members[1:])
    if kind=="float_mode":changes['members']=(replace(projection.members[0],mode=float(projection.members[0].mode)),*projection.members[1:])
    if kind=="list_members":changes['members']=list(projection.members)
    if kind=="dict_sources":changes['sources']=dict(projection.sources)
    with pytest.raises(ArchiveError):installed.validate_projection(replace(projection,**changes))


@pytest.mark.parametrize('timing',('before','after'))
def test_close_drains_once_and_never_recloses_an_ambiguously_reused_descriptor(tree,projection,tmp_path,monkeypatch,timing):
    owner=custody.OriginalInstalledTree(tree,projection).__enter__();handles=owner._parent[2]
    real_close=os.close;real_open=os.open;calls=[];replacement=[];fired=False
    extra=tmp_path/'owned-replacement';extra.write_bytes(b'inert separate owner')
    def ambiguous(fd):
        nonlocal fired
        calls.append(fd)
        if not fired:
            fired=True
            if timing=='after':
                real_close(fd);replacement.append(real_open(extra,os.O_RDONLY));assert replacement[-1]==fd
            raise OSError('ambiguous close observation')
        real_close(fd)
    try:
        with monkeypatch.context() as patch:
            patch.setattr(os,'close',ambiguous)
            with pytest.raises(OSError,match='ambiguous'):owner.close()
            assert calls==list(reversed(handles))
            assert owner._parent is None and owner._closed and owner._failed
            owner.close();assert len(calls)==len(handles)
        for fd in handles[:-1]:
            with pytest.raises(OSError):os.fstat(fd)
        assert os.fstat(replacement[0] if replacement else handles[-1])
    finally:
        for fd in set((*handles,*replacement)):
            try:real_close(fd)
            except OSError:pass


def test_temporary_lineage_close_failure_drains_all_temporary_handles(tree,projection,monkeypatch):
    owner=custody.OriginalInstalledTree(tree,projection).__enter__();held=owner._parent[2]
    real_open=os.open;real_close=os.close;opened=[];closed=[];fired=False
    def tracked(*args,**kwargs):
        fd=real_open(*args,**kwargs);opened.append(fd);return fd
    def ambiguous(fd):
        nonlocal fired
        closed.append(fd);real_close(fd)
        if not fired and fd not in held:fired=True;raise OSError('temporary close failure')
    try:
        with monkeypatch.context() as patch:
            patch.setattr(os,'open',tracked);patch.setattr(os,'close',ambiguous)
            with pytest.raises(OSError,match='temporary'):owner.recheck()
        assert fired and len(opened)==len(held)
        assert closed==list(reversed(opened))
        for fd in opened:
            with pytest.raises(OSError):os.fstat(fd)
        assert all(os.fstat(fd) for fd in held)
        with pytest.raises(ArchiveError,match='previously failed'):owner.recheck()
    finally:owner.close()


@pytest.mark.parametrize('phase',('initial','recheck'))
@pytest.mark.parametrize('action',('close','nested_recheck','nested_enter'))
def test_last_real_descriptor_observation_cannot_hide_ended_or_reentrant_custody(tree,projection,monkeypatch,phase,action):
    owner=custody.OriginalInstalledTree(tree,projection)
    if phase=='recheck':owner.__enter__()
    original=os.fstat;count=0;fired=False;inner_errors=[]
    def callback(fd):
        nonlocal count,fired
        observed=original(fd)
        if owner._parent is not None and fd==owner._parent[0]:
            count+=1
            if count==6 and not fired:
                fired=True
                try:
                    if action=='close':owner.close()
                    elif action=='nested_enter':owner.__enter__()
                    else:owner.recheck()
                except ArchiveError as error:inner_errors.append(error)
        return observed
    try:
        with monkeypatch.context() as patch:
            patch.setattr(os,'fstat',callback)
            with pytest.raises(ArchiveError):
                if phase=='initial':owner.__enter__()
                else:owner.recheck()
        assert fired and owner._failed
        if action!='close':assert len(inner_errors)==1
        with pytest.raises(ArchiveError):owner.recheck()
    finally:owner.close()


def test_installed_dictionary_count_is_admitted_before_set_materialization(projection,monkeypatch):
    real_set=set;seen=[]
    def tracked(values):
        if isinstance(values,dict):seen.append(len(values))
        return real_set(values)
    files=projection.files();modes={row.path:row.mode for row in projection.members}
    files['extra']=b'';modes['extra']=0o644
    monkeypatch.setattr(installed,'set',tracked,raising=False)
    with pytest.raises(ArchiveError):installed.verify_installed_content(projection,files=files,modes=modes)
    assert len(files) not in seen


def test_original_name_bound_precedes_hashing_or_sorting(projection,monkeypatch):
    real_set=set;seen=[]
    def tracked(values):
        if isinstance(values,list):seen.extend(len(value) for value in values if type(value)is str)
        return real_set(values)
    monkeypatch.setattr(installed,'set',tracked,raising=False)
    forged=replace(projection,sources=(('x'*(MAX_NAME_BYTES+1),b''),))
    with pytest.raises(ArchiveError):installed.validate_projection(forged)
    assert not seen
