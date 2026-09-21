# Python offline dependency origin join V1

`scripts/sorafs_python_dependency_archive.py` and
`scripts/sorafs_python_dependency_install.py` authenticate installed dependency
bytes before the SoraFS Python producer starts any installed distribution probe
or the fixed pytest child. The fixed fifteen original owners and independent
pins come from `python_runtime_inputs_v1.md`. This is a source and byte-origin
check, not a process result or release authorization.

`parse_dependency_wheel(raw, wheel=DependencyWheel) -> DependencyArchive` uses
original captured bytes and the exact pinned size/SHA-256. It reuses the sole
privacy wheel verifier's canonical ZIP envelope/local-record, member name/mode,
bounded member reader, metadata identity and exact SHA-256 RECORD primitives.
It does not accept native/SDK owners or provide an alternate parser for them.
No archive is extracted and no dependency package is imported.

The dependency profile is closed to the actual fifteen owner roots. It permits
pytest's `_pytest`, `pytest`, and `py.py`, cffi's package and `_cffi_backend.*.so`,
and cryptography's package and optional `cryptography.libs`; other owners have
their single canonical package root. Exactly one matching versioned dist-info
root is required. METADATA Name/version, WHEEL version/install scheme, complete
original RECORD coverage and every member digest are validated. Case/ancestor
aliases, file/directory collisions, links, devices, unexpected root packages,
`.data` relocation, bytecode, `.pth`, startup customization and preseeded pip
metadata reject. This bounded profile is not a general-purpose PyPI installer;
an independently selected wheel with unsupported layout must be reviewed before
changing the source contract.

Console script names derive from the original `entry_points.txt`. Pip receives
its fixed CPython 3.12 `pip`, `pip3`, `pip3.12` aliases. Cffi's current pinned
`distutils.setup_keywords` metadata is accepted only as the exact
`cffi_modules=cffi.setuptools_ext:cffi_modules` row; this helper executes no build
entry point. Other unreviewed groups reject. Dependencies cannot overlap each
other's source roots or generated console-script ownership.

The archive uses the existing 4,096-member, 256 MiB/member, 512 MiB expanded-byte
and 200:1 compression limits; pinned dependency originals remain at most 256 MiB
per wheel. Installed input is bounded at 16,000 files / 512 MiB aggregate and the
same member byte ceiling. All checks consume bytes already retained by the
producer's filesystem owner.

## Parent interface and ordering

1. Capture and hold the original dependency wheels selected by the independently
   pinned manifest. Parse their archives before invoking the exact pinned pip.
2. Perform the fixed fresh offline installation, then capture the entire installed
   environment and reject startup hooks before another Python startup.
3. Use the existing sole native/SDK verifier's `derive_installed_layout` and
   `verify_installed_files` to obtain the two real `InstalledFileSet` results.
   These operations inspect files and do not import either package. Their
   `.content` fields carry the installed byte relation separately from physical
   paths and seals.
4. Call `verify_dependency_install(archives, environment_files,
   environment=absolute_environment, wheel_paths_by_module=actual_install_paths,
   native_sdk_content=(native_files.content, sdk_files.content)) -> InstalledDependencySet`.
5. Only after success may the producer start its distribution probe and fixed
   pytest child. Recheck original inputs and repeat the join on the final complete
   installed capture before retaining successful evidence.

The archive tuple is in the manifest's exact fifteen-owner order. The wheel-path
mapping names the actual copied original paths passed to pip, so installed
`direct_url.json` must name those originals and their exact digests. The native
and SDK arguments are the sole verifier's `InstalledWheelContent` results, not
path exclusions. Each captured native/SDK byte must match that result's digest,
size and canonical owner root. Physical paths, seals and process authority remain
separate and cannot be manufactured by this byte relation.

`verify_installed_wheel_bytes(parsed_archive, source_uri=original_uri,
wheel_sha256=original_digest, installed_files=site_relative_bytes)` owns the one
installed native/SDK content algorithm. The live `verify_installed_files` owner
captures actual files and seals, then invokes this same function. The producer's
post-execution report join also invokes it after authenticating actual observed
file seals. An offline adapter can consume original indexed archive and installed
bytes without constructing a fake live file owner. Its caller must authenticate
the original wheel's URI, digest and captured archive bytes independently.
Canonical POSIX, Windows drive and UNC URI labels retain the original host's
path grammar; accepting a URI label supplies no evidence of host execution.

The byte relation requires the exact original member inventory plus modeled pip
metadata, exact source/native/metadata hashes and sizes, complete installed
RECORD coverage, and matching direct URL/digest. Repeated direct-URL JSON keys
reject. The existing archive count/member/aggregate limits apply before captured
content hashing. Live captures also reserve the remaining aggregate bound before
each read. The immutable result has no filesystem path or seal. The dependency
join uses those same content identities with the whole captured environment;
there is no second native/SDK parser, path exclusion or compatibility input.

Every dependency package/native/data file and original dist-info file must match
its original archive digest and size. `INSTALLER` is exactly `pip\n`, `REQUESTED`
is empty, and `direct_url.json` follows the existing verifier's original-wheel
policy. The installed RECORD must contain precisely all original files, these
modeled metadata files, its own empty-hash row, and the declared console outputs.
Forging a RECORD cannot authorize substituted package bytes. The entire observed
site-packages inventory must equal these dependency-owned files plus the two
independently verified native/SDK inventories.

Generated console scripts are bounded retained output, with exact digest/size
and coverage in their installed RECORD. They are not represented as source bytes
from the wheel or as authorized execution inputs. The fixed process owner keeps
the environment's `bin` directory off PATH, and runtime probes exclude it from
Python's import path. Future workflows that execute console programs require
an explicit stronger generated-program contract.

The immutable result's `.files` rows contain `module`, relative environment
`path`, `sha256`, `size`, and `generated`. The producer retains actual bytes and
observations separately. This pure join opens no live file, launches no process,
installs no package and supplies no synthetic production mode.

## Evidence boundary

Focused tests run the unchanged existing CI wheel harness, then reuse its exact
ZIP/member/RECORD fixture helpers for synthetic dependency archives. Native/SDK
fixtures go through the actual sole parser and installed-file verifier; their
native member is inert and is not executed. The tests exercise altered source
and native members, forged/missing/excess RECORD coverage, origin URLs, generated
scripts, competing owners, startup injection, limits and unknown files. They do
not install real wheels or execute the 77 SoraFS native assertions. Actual offline
producer execution, adapter replay from original indexed bytes, platform tests
and signed release evidence remain separate qualification requirements.
