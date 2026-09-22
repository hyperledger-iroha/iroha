# JavaScript original npm archive ownership

`scripts/sorafs_javascript_archive.py` is the shared content parser for the
pending installed SoraFS JavaScript producer and original-index adapter.
`scripts/sorafs_javascript_dependencies.py` joins the nine original production
dependency tarballs to the independently selected candidate lock.
`scripts/sorafs_javascript_package_source.py` joins package content to captured
source and checksum bytes. These are pure byte consumers. They perform no
extraction, installation, imports or execution; their frozen results confer no
process, signer or release authority.

## Archive contract

One complete gzip stream contains POSIX ustar regular files below `package/`.
The parser checks gzip CRC/size and each tar checksum. Concatenated gzip members,
trailing compressed bytes, truncation, nonzero padding, hidden content after the
two zero end blocks, links, devices, explicit directories, sparse layouts,
GNU extensions and global PAX metadata reject. Only regular modes 0644 and 0755
are admitted. Inert per-file PAX headers also permit mode zero.

Per-file PAX can supply one effective UTF-8 path with optional repeated `size`
and `mtime`; numeric fields must equal the following ustar header. Duplicate,
unknown, stacked, dangling, fractional or overriding records reject. This
handles actual npm long/Unicode filenames without an alternate decoder. Both
the effective path and original regular header name must be safe. Paths are
NFC, relative, component-bounded and unambiguous; duplicate, casefold, Unicode
normalization, file/ancestor and ancestor-spelling aliases reject.

| Resource | Fixed maximum |
| --- | --- |
| Original gzip | 64 MiB |
| Total inflated tar, including headers, metadata and zero padding | 256 MiB |
| One regular member | 32 MiB |
| Regular members | 20,000 |
| UTF-8 archive path, including `package/` | 1,024 bytes |
| One UTF-8 path component | 255 bytes |
| One PAX payload | 16 KiB |
| Distinct retained path nodes | 80,000 |
| Sum of retained ancestor-name UTF-8 bytes | 16 MiB |

The inflater emits at most 64 KiB per call. Declared padded member extents are
reserved against the remaining tar budget before payload reads. Path inventory
capacity is checked before insertion. `tar_byte_limit` may reduce the fixed
ceiling for a shared owner; it cannot raise it. Returned `tar_size` includes all
inflated bytes. These are format/allocation bounds, not a measured RSS promise.
The same `NpmPathInventory` admits projected source members and parsed tar
members; any failed admission permanently invalidates that inventory. Character
counts are bounded before UTF-8 encoding or prefix allocation.

## Dependency contract

The lock is exact original `package-lock.json` V3 with a separately supplied
SHA-256 identity and the reviewed root dependency declarations. The closure is
the nine current production locations: ciphers 1.3.0, curves 1.9.7, hashes 1.8.0,
base 2.2.0, bip39 2.2.0, bip39's nested hashes 2.2.0, base64-js 1.5.1, buffer 6.0.3
and ieee754 1.2.1. Package names alone cannot replace placement. Version,
registry URL, canonical single SHA-512 integrity and dependency edges are exact;
metadata projections must rederive the original lock bytes.

All compressed inputs share 64 MiB; all inflated tar bytes share 128 MiB.
Original integrity is checked before decompression and the next tar receives
only the remaining aggregate budget. Every package's actual JSON must match its
lock-owned name, version, dependencies and engine declarations. JSON duplicates,
unreviewed metadata, install scripts, explicit or implicit executable links,
alternate dependency resolution and platform selection reject. In particular,
`directories.bin` and `acceptDependencies` cannot bypass the fixed profile.
Bundled `node_modules`, `.npmrc`, secondary npm locks, native libraries and
retired Wasm/WASI artifacts reject. Ordinary upstream build/test scripts remain
inert captured bytes. A future producer must disable scripts and preserve these
originals throughout installation and execution.

## Package and source relation

The source owner must supply a complete authenticated candidate census: every
`src/` regular file, literal published files, `recipes/README.md`, the original
lock, `scripts/build-dist.mjs` and the unpublished prepack dependency/runtime
guard `scripts/check-node-engine.mjs` plus its pure `node-engine-contract.mjs`
sibling. Exact package/build/engine recipe
SHA-256 pins identify the reviewed selection. All 23 mandatory build outputs and
six consumer entrypoints are required. Source paths follow the reviewed flat,
`public/` or `kotodamaCompiler/` layout with ordinary ASCII module basenames,
optional `.browser`, and `.js`/`.d.ts` suffixes. Selector files, unreviewed
directories and names that npm would silently omit reject; this owner does not
reimplement npm ignore rules.

The recipe copies each source byte unchanged to `dist/`; literal members retain
their bytes, and npm's implicit recipe README is included. Current source has
202 census inputs and 199 package members. Complete member names, content and
0644 permissions must match, with no extra native addon or test payload. The
checksum manifest is a separately held original, never selected from a stale
dist tree. Its public staged copy uses 0644; original custody permissions do not
change. Manifest authenticity and native identity belong to the native owner.

Source admission permits at most 4,096 files, 16 MiB per file and 64 MiB total;
the separate checksum JSON is bounded to 1 MiB. The shared tar namespace also
bounds the projected members. A caller-supplied dictionary cannot establish
that arbitrary source files were not omitted: candidate census authentication
and physical input custody remain producer/index-adapter responsibilities.

## Installed graph and physical tree

`scripts/sorafs_javascript_installed.py` consumes the same original package,
source and dependency owners to derive exact node_modules paths, bytes and
modes. `sorafs_javascript_install_metadata.py` requires the generated hidden
lock's exact ten-package graph, SDK SHA-512, original dependency integrity and
fixed private consumer metadata. Missing or foreign generated metadata rejects.
The native artifact stays separately admitted outside the portable package.

The graph permits 8,192 files, 192 MiB content and 64 path components.
Cardinality and name bounds precede set/sort allocation. Frozen projections
must rederive from their original bytes; caller-supplied originals do not prove
candidate authenticity. Labels are canonical bounded paths, not authorization
to reopen historical producer files.

`sorafs_javascript_installed_custody.py` retains the actual root and ancestor
descriptors and scans descendants through no-follow relative opens. Initial
and final checks cover the complete file/directory inventory, single-link
regular files, owner/modes, content and physical identities. Reads are bounded
to 64 KiB. Any refusal or reentrant check poisons the owner. Cleanup detaches
owned handles and attempts each once, including after an ambiguous close error.
These observations do not establish which modules executed between checks.

## Fixed qualification source

`sorafs_javascript_qualification_source.py` admits the exact 191-file source
and fixture core: nine pinned code/contract inputs and 182 fixed fixture names.
The catalog does not authenticate candidate fixture bytes; those remain bound
to their independent original inputs. Its dedicated physical owner shares
`sorafs_javascript_tree_custody.py` with the installed tree, while retaining a
separate content relation. See the [canonical source contract](javascript_qualification_source_v1.md).
The future child/runner tools require their own closed source extension.

## Shared assertion ownership

The six source profile entrypoints bind one canonical assertion body each in
`javascript/iroha_js/test/sorafsNativeSuites/`. They retain 172 assertion
expressions, 46 top-level cases and nine named nested cases. The source wrappers
select their original concrete source exports; the pending installed child
must select the equivalent actual public package exports plus the verified
private `dist/native.js` and `dist/toriiTestHooks.js`. There are no new public
SDK exports or parallel assertion implementations.

Test-owned contexts supply only those exact subjects, actual native binding or
load error, original fixture directory URLs and the owned temporary root.
`test/helpers/nativeRequirements.js` is the sole pure native requirement
implementation. The existing eager source helper binds it after the original
load attempt; shared suites never import that eager source helper. Missing
native capabilities fail with the original error instead of skipping cases.
The orderbook suite's intentional transport/native mocks remain component
controls; orchestrator local-fixture providers do not qualify deployment.

The source contract fixture and TypeScript AST controls enforce unchanged
statements, assertions, names, callback bodies, helpers and closed imports.
Registration controls do not execute native assertion bodies and cannot supply
an installed execution report. The final observer must verify actual complete
case results, with no missing, extra, duplicate, failed, skipped or todo cases.

## Fixed Node 24 event relation

`scripts/sorafs_javascript_test_events.mjs` consumes the actual structured
`TestsStream` for the fixed Node 24 qualification child. The existing original
assertion-contract bytes are its sole case inventory: 46 top-level and nine
nested cases. It checks file/name/parent identity, stable coordinates, unique
IDs, serial sibling order, all five case events, both plans and complete final
counts. Reporter buffering can deliver completion before start; successful
pass still requires both original observations.

Any failed event, including an entry/final-hook failure, rejects even if Node's
summary reports success. Skips, TODOs, foreign fields/events, invalid numbers,
duplicate or incomplete observations and events after summary reject. A refusal
permanently invalidates the owner, including a swallowed reentrant refusal.
Limits are 1,024 events, 1 MiB aggregate text, 4,096 code units per string,
24 fields per object, 64 code units per field name and four nested levels.
The consumer checks these event bounds; it does not authenticate physical files
or bound allocations made by the test runner before events are delivered.

`finish()` must follow actual original-stream EOF. Its frozen observations
grant no execution or release authority. The fixed child still owns stream
provenance, retained inputs, native identity and process completion. The actual
Node 24 inert controls live in
`scripts/tests/sorafs_javascript_test_events_test.mjs`, outside SDK unit discovery;
they do not execute the 172 original assertions or qualify Node 20/22 SDK lanes.
The SoraFS SDK workflow runs the full controls immediately after its existing
Node 24 setup. Both SoraFS workflows watch these tooling paths explicitly;
the workflow validator requires the fixed unconditional step and real triggers.

## Native load observation

`scripts/sorafs_javascript_native_cache.mjs` captures a native-free CommonJS
cache before SDK import or use. After a normal installed SDK call, it requires
exactly one already loaded native module before invoking the verified private
getter to identify its exports. Cache key, module id, filename, completed-load
flag, own data descriptors and export/callable identities must agree and remain
unchanged on recheck. Refusal invalidates the observer; no addon is required,
reset or loaded by the observer itself.

This is mutable process-state observation. It does not prove an original file's
custody, actual ABI behavior or mapped process memory. The fixed child must
connect this observer to `scripts/sorafs_javascript_child_files.mjs`, which
retains the original file and controlled temporary-root ancestry before SDK
import, then identifies the actual snapshot through the original cache method.
It checks loader path policy, private modes, single snapshot membership,
raw equality and stable physical/cache identities through final recheck.
The fixed 1 GiB original-file ceiling and 64 KiB reads bound this component.
Refusal is permanent; cleanup attempts each original descriptor once and never
retries a potentially reused number. Node has no openat, so pathname/held-lineage
checks detect observed replacement without promising to prevent transient
restored substitutions. No native code is loaded by this component.

The existing Node 24 CI step runs both event and file-owner suites. Actual
source/runtime/installed/native ABI joins and same-process execution remain
the fixed child's responsibility.

## Remaining execution work

TODO: implement the actual fixed installed-package producer using the shared
package/source and installed-content relations, exact offline installation,
loaded-module joins, runtime/npm original custody, retained native snapshot/ABI-23
join, bounded complete six-suite observations, publication and original-index
consumer. Preserve the existing assertion bodies and distinguish mocked
transport controls from native verification. The JS native build tree digest
and workspace source manifest use different domains and must each be checked.

The package and root lock require Node >=20.19.0, matching the nested hashes
dependency. The publish/prepack engine guard checks the locked production
closure and the current process against that floor. SoraFS CI still selects 24,
privacy CI selects 20 and Kotodama CI selects 22; their matrices are unchanged.
The privacy shell keeps its Node 20 selection and runs the same current-runtime
floor check before building native code. Source-order and negative execution
controls require that check; its package, lock, scripts and tests also trigger
the privacy workflow. These content and metadata checks do not qualify any
runtime/platform matrix.
The signed aggregate and SF11 gate remain open; real installed execution and
independent matching-candidate release authority are still required.
