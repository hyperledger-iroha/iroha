"""Draft independent artifact admission before loading the shared native process bridge."""
from dataclasses import dataclass
import ctypes
import hashlib
import os

from release_process_bridge import Bridge


@dataclass(frozen=True)
class BridgePins:
    """External pipeline trust binds bridge, entry candidate source and native host."""
    executable_sha256: str
    provenance_sha256: str
    source_manifest_sha256: str
    native_target: str


def filename():
    """Exact diagnostic artifact spelling; no maintained shipping artifact exists yet."""
    return 'iroha_process_bridge.dll' if os.name=='nt' else 'libiroha_process_bridge.so'


def load_original(root,pins,source,owners,*,entry_module,custody_module,environment_policy,native_module=None):
    """Place ALL admitted originals in the caller's eventual process-custody stack.

    The exact same closed Rust publisher record is admitted here; no hash calculated
    from the selected library becomes authority. Windows loader dependencies search
    System32 only, never an unpinned sibling or PATH location. The source context and
    reviewed module registry must already be in `owners` before this function.
    """
    root=entry_module._path(str(root))
    entry_module.EntryPins(pins.executable_sha256,pins.provenance_sha256,
        pins.source_manifest_sha256,pins.native_target).validate()
    if source.manifest!=pins.source_manifest_sha256:
        raise RuntimeError('native bridge source differs from the independently retained workspace')
    original=owners.enter_context(custody_module.SourceKeeper(root,maximum_members=2,native_module=native_module))
    names=[filename(),'publisher-source.json']
    original.capture(names)
    if set(os.listdir(root))!=set(names):
        raise RuntimeError('native bridge directory is not the exact two-entry artifact')
    original.revalidate()
    with original.reader(root/'publisher-source.json') as reader:
        raw=reader.read(entry_module._MAX_PROVENANCE+1)
    evidence=entry_module._provenance(raw,pins,environment_policy,expected_filename=filename())
    lock_hash=hashlib.sha256()
    observed=0
    with source.reader(source.root/'Cargo.lock') as reader:
        while block:=reader.read(64*1024):
            observed+=len(block)
            if observed>16*1024*1024:
                raise RuntimeError('original bridge Cargo.lock exceeds its bound')
            lock_hash.update(block)
    if evidence['cargo_lock_sha256']!=lock_hash.hexdigest():
        raise RuntimeError('native bridge provenance differs from the independent original lock')
    with original.reader(root/filename()) as reader:
        entry_module._executable(reader,pins,evidence['executable_bytes'],library=True)
        source.revalidate();original.revalidate()
        if os.name=='nt':
            library=ctypes.CDLL(str(root/filename()),winmode=0x800)
        else:
            path='/proc/self/fd/'+str(reader.fileno())
            held=os.fstat(reader.fileno()); selected=os.stat(path)
            if (held.st_dev,held.st_ino)!=(selected.st_dev,selected.st_ino):
                raise RuntimeError('native bridge descriptor projection changed original identity')
            library=ctypes.CDLL(path)
        original.revalidate();source.revalidate()
    def validate():
        source.revalidate()
        original.revalidate()
    def validate_originals():
        """Check the same retained source/artifact originals without a new source census."""
        source.revalidate_originals()
        original.revalidate()
    bridge=Bridge(library,owners=owners,validate=validate,validate_originals=validate_originals)
    # Keep the loaded library reference alive through the same owner stack even if
    # a caller abandons the facade while an operation permanently retains custody.
    owners.callback(lambda retained=library: None)
    return bridge
