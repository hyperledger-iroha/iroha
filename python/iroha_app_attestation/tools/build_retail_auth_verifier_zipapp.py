"""Build unsigned auth-only archive from explicit current generic and auth source originals.

The generated inventory is not signed release admission. No E1 worker, enrollment
scope, issuer key, credential constructor or caller-verdict extension is bundled.
"""
import argparse
import hashlib
import json
import os
import stat
import zipfile
from contextlib import ExitStack
from pathlib import Path

GENERIC = ('__init__.py','attestation.py','revocation.py','play_integrity.py',
           'google_oauth.py','openssl_private_rsa.py','native_time_interval.py')


def retain_original(source: Path, stack: ExitStack) -> tuple[bytes, object]:
    if not source.is_absolute() or source.resolve(strict=True) != source:
        raise ValueError('source_alias')
    identity = lambda v: (v.st_dev,v.st_ino,v.st_size,v.st_mtime_ns,v.st_ctime_ns,v.st_nlink)
    before = source.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or not 0 < before.st_size <= 2*1024*1024:
        raise ValueError('source_original')
    held = stack.enter_context(source.open('rb'))
    def recheck():
        if identity(os.fstat(held.fileno())) != identity(before) or identity(source.lstat()) != identity(before):
            raise ValueError('source_changed')
    recheck()
    value = held.read(2*1024*1024+1)
    if len(value) != before.st_size: raise ValueError('source_changed')
    recheck()
    return value, recheck


def build(generic_package: Path, auth_package: Path, destination: Path) -> dict:
    generic = generic_package/'src/iroha_app_attestation'
    auth = auth_package/'src/iroha_app_attestation'
    if destination.name != 'iroha-retail-auth-verifier.pyz': raise ValueError('archive_name')
    if not destination.is_absolute() or destination.parent.resolve(strict=True) != destination.parent:
        raise ValueError('destination_alias')
    with ExitStack() as stack:
        sources = [(generic/name, 'iroha_app_attestation/'+name) for name in GENERIC]
        sources += [(auth/'retail_auth_worker.py', 'iroha_app_attestation/retail_auth_worker.py')]
        originals, checks = {}, []
        for path, name in sources:
            value, recheck = retain_original(path, stack)
            originals[name] = value
            checks.append(recheck)
        originals['__main__.py'] = (b'from iroha_app_attestation.retail_auth_worker import main\n'
            b'try:\n    main()\nexcept Exception:\n    raise SystemExit(1) from None\n')
        # All input file identities remain held until the complete output has been sealed.
        # This archive and inventory still convey no signed release or runtime authority.
        with destination.open('xb+') as original:
            with zipfile.ZipFile(original,'w',compression=zipfile.ZIP_STORED) as archive:
                for name,value in sorted(originals.items()):
                    info = zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0))
                    info.external_attr = 0o100444 << 16
                    archive.writestr(info,value)
            for recheck in checks: recheck()
            original.flush()
            os.fchmod(original.fileno(), 0o444)
            os.fsync(original.fileno())
            identity = lambda m: (m.st_dev,m.st_ino,m.st_size,m.st_mtime_ns,m.st_ctime_ns,m.st_mode)
            sealed = identity(os.fstat(original.fileno()))
            original.seek(0)
            value = original.read()
            for recheck in checks: recheck()
            if identity(os.fstat(original.fileno())) != sealed or identity(destination.lstat()) != sealed:
                raise ValueError('archive_changed')
            return {'schema':'bpng.first-device-auth-verifier-unsigned-build.v1',
                'archive_sha256':hashlib.sha256(value).hexdigest(),
                'sources':{name:hashlib.sha256(value).hexdigest() for name,value in sorted(originals.items())},
                'signed_runtime_admission':False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--generic-package',type=Path,required=True)
    parser.add_argument('--auth-package',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--inventory',type=Path,required=True)
    args = parser.parse_args()
    args.inventory.write_text(json.dumps(build(args.generic_package,args.auth_package,args.output),indent=2)+'\n')
