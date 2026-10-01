#!/usr/bin/env python3
"""Build a deterministic shared issuer archive; signing remains release-owned."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import stat
import zipfile
from pathlib import Path

ENTRY = b"from iroha_app_attestation.ordinary_worker import main\nraise SystemExit(main())\n"


def build_archive(package: Path) -> tuple[bytes, dict[str, str]]:
    if package.is_symlink() or not package.is_dir():
        raise ValueError("issuer source directory differs")
    sources = sorted(package.glob("*.py"))
    if not sources or not (package/"ordinary_worker.py").is_file():
        raise ValueError("canonical issuer entry is absent")
    originals = {}
    for path in sources:
        if path.is_symlink() or not path.is_file():
            raise ValueError("issuer source must be an original regular file")
        data = path.read_bytes()
        if len(data)>1024*1024:
            raise ValueError("issuer source outside bound")
        compile(data,str(path),"exec")
        originals["iroha_app_attestation/"+path.name]=data
    manifest={name:hashlib.sha256(data).hexdigest() for name,data in originals.items()}
    originals["__main__.py"]=ENTRY
    originals["iroha_app_attestation/_source_manifest.json"]=json.dumps(manifest,sort_keys=True,separators=(",",":")).encode()
    stream=io.BytesIO()
    with zipfile.ZipFile(stream,"w",compression=zipfile.ZIP_STORED,allowZip64=False) as archive:
        for name,data in sorted(originals.items()):
            info=zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0))
            info.create_system=3;info.external_attr=(stat.S_IFREG|0o444)<<16
            archive.writestr(info,data)
    for path in sources:
        name="iroha_app_attestation/"+path.name
        if path.is_symlink() or not path.is_file() or path.read_bytes()!=originals[name]:
            raise ValueError("issuer source changed during archive construction")
    if sorted(package.glob("*.py"))!=sources:
        raise ValueError("issuer source roster changed during archive construction")
    return stream.getvalue(),manifest


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    if not args.output.is_absolute() or args.output.exists() or args.output.is_symlink():
        parser.error("output must be a fresh absolute path")
    package=Path(__file__).resolve().parents[1]/"src/iroha_app_attestation"
    original,manifest=build_archive(package)
    descriptor=os.open(args.output,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o444)
    try:
        offset=0
        while offset<len(original):
            size=os.write(descriptor,original[offset:])
            if size<=0:raise OSError("archive write did not advance")
            offset+=size
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    print(json.dumps({"schema":"iroha.ordinary-app-issuer-archive.v1","path":str(args.output),
        "bytes":len(original),"sha256":hashlib.sha256(original).hexdigest(),"source_sha256":manifest,
        "qualification":"Unsigned build output; no Native installation, signer custody or live mutation."},sort_keys=True))


if __name__=="__main__":
    main()
