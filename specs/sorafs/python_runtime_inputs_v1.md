# Python consumer runtime and offline inputs V1

`scripts/sorafs_python_runtime_inputs.py`,
`sorafs_python_runtime_custody.py`, and `sorafs_python_dependency_inputs.py`
own the bounded original inputs for the fixed SoraFS Python consumer. The host
profile is CPython 3.12 on Darwin or Linux. A valid input inventory or bundle is
not execution evidence or release authorization.

The producer receives independently reviewed SHA-256 pins for two canonical JSON
manifests. It must not derive those trust pins from the ambient machine or an
online resolver during qualification. JSON uses ASCII escapes, sorted keys,
compact separators and one final newline. Unknown and missing fields reject.

## Runtime manifest

This is a structural example; the paths, byte sizes, omitted rows and digest
placeholders must be replaced with the complete independently reviewed inventory.
It is not an accepted fixture or a supplied production pin.

```json
{
  "schema": "sorafs.python.runtime_inputs.v1",
  "platform": "linux",
  "version": "3.12.14",
  "executable": {"path": "/opt/python/bin/python3.12", "sha256": "<sha256>", "size": 1234},
  "shared_runtime": [
    {"path": "/opt/python/lib/libpython3.12.so.1.0", "sha256": "<sha256>", "size": 5678}
  ],
  "stdlib": {
    "root": "/opt/python/lib/python3.12",
    "directories": ["encodings", "lib-dynload"],
    "files": [
      {"path": "encodings/__init__.py", "sha256": "<sha256>", "size": 123},
      {"path": "os.py", "sha256": "<sha256>", "size": 123},
      {"path": "site.py", "sha256": "<sha256>", "size": 123},
      {"path": "sysconfig.py", "sha256": "<sha256>", "size": 123}
    ],
    "links": []
  },
  "stdlib_zip": {"path": "/opt/python/lib/python312.zip", "sha256": null, "size": null},
  "site_packages": {"kind": "absent", "target": null}
}
```

The complete stdlib tree includes lib-dynload, regular files, empty directories
and explicit file symlinks. Only the exact root `site-packages` entry is excluded:
its absent/directory/symlink kind and raw link target remain bound and rechecked.
Nested directories with that name are ordinary inventoried runtime inputs.
Direct-root `sitecustomize` and `usercustomize` module/package families reject,
including sourceless bytecode, native-extension suffixes and case variants.

Every explicit link has the closed shape
`{"path":"config/libpython.so","target":"../../../libpython.so","resolved":"/opt/python/libpython.so"}`.
The target must resolve through declared directories/links to a pinned regular
file. Both the captured-byte parser and live owner enforce this join. The live
owner additionally checks actual symlink and file identities. Cycles, missing
components and traversal through a regular file reject. Shared-library aliases
such as stock macOS framework config links can reference the actual pinned
framework executable; the runtime is not relocated.

The manifest limit is 4 MiB; the complete stdlib permits at most 8,192 regular
files and links combined, 8,192 directories, and 64 file links. There are at most
16 shared-runtime regular files. A file is at most 256 MiB and aggregate runtime
content at most 512 MiB. File, directory and link inventories are sorted and
unique. The manifest pins content; live custody additionally requires single-link
regular files, original ancestor identities and an executable mode.

The `stdlib_zip` path is the exact configured parent slot `python312.zip`.
An absent slot has null digest and size, and the owner rejects any later entry,
including a dangling symlink. A present zip is opaque pinned runtime content;
its import policy and actual loader path remain the fixed parent probe's job.

## Original byte bundle and live owner

`parse_runtime_manifest(raw, expected_sha256=...)` authenticates original JSON.
`OriginalPythonRuntime(manifest)` retains bounded original ancestor descriptors
through the producer operation. `recheck()` verifies complete actual file bytes,
metadata, directory inventory, exclusions, links and absence. Individual member
file descriptors are temporary; this is ordinary signed-producer toolchain
custody, not proof against a malicious operating system.

`write_bundle(parent_owned_stream)` writes to the parent's fresh unpublished
binary output. The sole format is the magic `SORAFS_PYTHON_RUNTIME_INPUTS_V1\n`,
an unsigned eight-byte big-endian manifest length, the exact manifest bytes,
then the content returned by `manifest.files()` in order: executable, sorted
shared runtime, optional stdlib zip, sorted stdlib files. The manifest supplies
all member lengths and digests. Partial writes are handled; failed writes do not
produce a completion digest. The parent owns output creation/publication.

`parse_runtime_bundle(raw, expected_manifest_sha256=...)` validates every byte
and exact EOF. `member_bytes(absolute_path)` returns only a verified member.
One indexed bundle therefore retains complete runtime original bytes without
exceeding the package index's 256-original-file limit or retaining thousands of
descriptors. Manifest claims and captured byte equality are separate from the
parent's actual process observations.

The parent must join its fixed isolated probe to the exact implementation,
version, platform, configured stdlib, import paths, original executable and the
actual `dladdr(Py_Initialize)` runtime library. It must preserve actual original
runtime custody through execution. Normal OS libraries and additional native
library provenance remain platform/toolchain inventory and operator
qualification, not full-machine attestation.

## Offline dependency manifest

The closed root is `{"schema":"sorafs.python.offline_dependencies.v1","wheels":[...]}`.
Each row is `{"module":"pytest","version":"9.0.3","file":{"path":"/absolute/original/pytest-9.0.3-py3-none-any.whl","sha256":"<sha256>","size":123}}`.
The exact sorted 15-row inventory is `blake3`, `certifi`, `cffi`,
`charset-normalizer`, `cryptography`, `idna`, `iniconfig`, `packaging`, `pip`,
`pluggy`, `pycparser`, `pygments`, `pytest`, `requests`, `urllib3`. Direct versions
match `scripts/requirements.txt`; all transitive versions and all original wheel
bytes require independent pins. This profile is POSIX CPython 3.12, where the
conditional Windows/colorama and older-Python typing-extensions dependencies do
not apply. Existing privacy CI's pytest 8.4.2 lock is not reused or silently
rewritten for the fixed pytest 9.0.3 consumer.

`parse_dependency_manifest(raw, expected_sha256=...)` returns immutable
`DependencyManifest.wheels`; each `DependencyWheel` has `module`, `version`, and
`file: FileReference` with an absolute original producer path. The limit is
64 KiB of manifest, 256 MiB per nonempty wheel and 1 GiB total wheel content.
`authenticate_dependency_inputs(opened_index, manifest, paths_by_module=...)`
joins the same artifacts to Python-owned index paths and reads only original
held descriptors. Mapping an original absolute label to an indexed path does
not change the byte identity. The owner authenticates opaque bytes and does not
parse archives, install dependencies or grant installed-module authority.

The existing `ci/verify_privacy_python_wheel.py` remains the sole native/SDK
wheel parser. The parent uses exact pinned pip for a fresh offline install,
retains the full installed tree and metadata, and the adapter must derive
installed origins from the actual original wheels. No network resolution,
second native/SDK parser or synthetic execution mode is introduced here.

## Qualification boundary

Component tests use inert runtime bytes and opaque dependency fixtures with real
filesystem and stream custody. They do not execute a CPython fixture, install
wheels, run the unchanged 77 native assertions, establish installed third-party
provenance, or qualify production. Actual parent execution, candidate/native
ABI-23 joins, complete installed-origin adapter verification, platform coverage
and signed release evidence remain separate requirements.
