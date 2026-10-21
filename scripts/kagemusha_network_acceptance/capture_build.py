"""Launch an explicitly selected compiler capture without imported bytecode.

Run: python3.12 -I -B -S capture_build.py MANIFEST_SHA --role native --slot SLOT
     --profile release --output /absolute/repo/target/qualification/FRESH
The selected manifest is the adjacent reviewed controller manifest. No Cargo or
native process is started until all source helpers have been authenticated.
"""
from pathlib import Path
import hashlib
import json
import os
import stat
import sys
from types import ModuleType

ORDER = ('paths', 'child_process', 'source_audit', 'capture', 'monetary_depfile', 'compiler_capture', 'source_admission')

SCHEMA = 'iroha.kagemusha.network-acceptance-runtime.v1'
LOCAL = {'run.py', 'source_admission.py', 'custody.py', 'native_data.py', 'native_catalog.py',
         'native_capture.py', 'sealed_data.py', 'catalog_policy.py', 'protocol.py', 'paths.py',
         'raw_reuse.py', 'raw_reuse_policy.py', 'compiler_capture.py', 'capture.py',
         'source_audit.py', 'monetary_depfile.py', 'capture_build.py', 'child_process.py'}
EXTERNAL = {'scripts/audit_native_source_inputs.py', 'scripts/native_sdk_source_custody.py',
            'scripts/kagemusha_qualify.py'}


def source_bytes(path):
    path = Path(path)
    if path.resolve(strict=True) != path or any(p.is_symlink() for p in (path, *path.parents)):
        raise ValueError('aliased runtime source')
    before = path.stat()
    if not stat.S_ISREG(before.st_mode):
        raise ValueError('regular runtime source required')
    with path.open('rb') as reader:
        opened = os.fstat(reader.fileno()); raw = reader.read(); after = os.fstat(reader.fileno())
    final = path.stat()
    identity = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
    if len({identity(info) for info in (before, opened, after, final)}) != 1:
        raise ValueError('runtime source changed during read')
    return raw


def bootstrap(manifest, expected):
    import re
    if not re.fullmatch('[0-9a-f]{64}', expected):
        raise ValueError('selected runtime manifest SHA256 required')
    raw = source_bytes(manifest)
    if hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError('runtime manifest changed')
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError('duplicate runtime manifest field')
            value[key] = item
        return value
    value = json.loads(raw, object_pairs_hook=unique)
    runtime = value['runtime']
    if value.get('schema') != SCHEMA or set(runtime) != {'local', 'external'} or set(runtime['local']) != LOCAL or set(runtime['external']) != EXTERNAL:
        raise ValueError('exact current runtime roster required')
    here = manifest.parent
    roots = [p for p in (here, *here.parents) if (p / 'Cargo.toml').is_file() and (p / '.git').exists()]
    if not roots:
        raise ValueError('repository root unavailable')
    root = roots[0]
    sources = {}
    for kind, prefix in (('local', here), ('external', root)):
        for name, digest in runtime[kind].items():
            path = prefix / name
            if Path(name).is_absolute() or '..' in Path(name).parts or not path.is_relative_to(prefix):
                raise ValueError('runtime source escapes selected owner')
            contents = source_bytes(path)
            if hashlib.sha256(contents).hexdigest() != digest:
                raise ValueError('selected runtime source changed: ' + name)
            sources[str(path)] = contents
    required = {'capture_build.py', *(name + '.py' for name in ORDER)}
    if not required <= runtime['local'].keys():
        raise ValueError('missing capture runtime module')
    for name in ORDER:
        if name in sys.modules:
            raise ValueError('foreign preloaded capture helper')
        path = here / (name + '.py')
        module = ModuleType(name); module.__file__ = str(path)
        sys.modules[name] = module
        exec(compile(sources[str(path)], str(path), 'exec'), module.__dict__)
    for path, contents in sources.items():
        if source_bytes(Path(path)) != contents:
            raise ValueError('runtime source changed during loading')
    module = sys.modules['compiler_capture']
    module.AUTHENTICATED_RUNTIME = {path: hashlib.sha256(raw).hexdigest() for path, raw in sources.items()}
    return module


def main():
    if len(sys.argv) == 2 and sys.argv[1] in ('--help', '-h'):
        print(__doc__)
        return
    if sys.version_info < (3, 11):
        raise SystemExit('Python 3.11+ is required by maintained source custody tooling')
    if not (sys.flags.isolated and sys.flags.no_site and sys.dont_write_bytecode):
        raise SystemExit('use Python -I -B -S for authenticated source-only capture')
    if len(sys.argv) < 2:
        raise SystemExit('selected runtime manifest digest required')
    expected = sys.argv.pop(1)
    module = bootstrap(Path(__file__).absolute().with_name('manifest.json'), expected)
    module.main()


if __name__ == '__main__':
    main()
