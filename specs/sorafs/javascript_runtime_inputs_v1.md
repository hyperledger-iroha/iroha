# JavaScript runtime original-byte inputs V1

`scripts/sorafs_javascript_runtime_inputs.py` supplies a pure, bounded content
relation for a **restricted Darwin arm64 Node24 input profile**. It performs no
filesystem lookup, process launch or native import. It does not qualify the local
Node installation, a package, a candidate, a runtime pin or mapped execution.

The APIs are:

```python
manifest = produce_node_runtime_manifest(
    version="24.21.0", selected_executable=selected_path,
    executable=original_executable_path,
    original_images={path: (original_bytes, original_mode) for path in image_paths},
    aliases={alias_path: literal_target for alias_path in alias_paths},
)
manifest = parse_node_runtime_manifest(raw, expected_sha256=independent_pin)
bundle = parse_node_runtime_bundle(original_bytes,
                                   expected_manifest_sha256=independent_pin)
original_member = bundle.member_bytes(canonical_original_path)
```

The producer accepts only caller-supplied original byte strings, mode claims and
literal alias targets. It sorts image and alias names, derives alias resolutions,
projects each image through the same bounded Mach-O decoder, and uses the
verifier's shared graph derivation for every direct and inherited candidate
slot. Serialization is capped at 4 MiB while streaming JSON chunks, then
compared with the canonical serializer and parsed again. A changed image
command therefore produces a changed complete manifest; an old partial edge
list is never an input to the producer. It does no path I/O and does not frame,
approve or sign a bundle. Its returned SHA-256 is a content identifier only.

Both parsers require exact `bytes` and compare the independently supplied digest
before parsing the manifest. The caller remains responsible for that digest's
review/signing provenance. Parsing cannot manufacture approval. The manifest's
exact `24.minor.patch` version is an expectation for a later genuine process
observation; binary parsing alone does not prove the version.

## Closed schema and original bytes

The only schema is `sorafs.javascript.runtime_inputs.v1`. Required top-level
fields are `schema`, `platform` (`darwin`), `architecture` (`arm64`), `version`,
`selected_executable`, `executable`, `images`, `aliases`, and `edges`. Unknown or
missing fields and noncanonical JSON are rejected. JSON is ASCII escaped,
key-sorted, compact and LF terminated using the existing canonical serializer.
Duplicate keys use the existing evidence JSON refusal.

- `selected_executable` is the absolute selected pathname. `executable` is its
  canonical original image path; pure alias resolution must join the two.
- Sorted unique `images` rows contain `path`, lowercase nonzero `sha256`, exact
  integer `size`, and integer `mode`. Modes are 0444, 0555, 0644 or 0755; the
  executable must have owner execute permission. These are input claims until
  physical custody verifies actual mode/inode/owner and single-link policy.
- Sorted unique `aliases` rows contain `path`, original literal `target`, and
  `resolved`. The resolver derives the claimed result using the bounded declared
  namespace. Parent directories are implicit requirements of those paths, not
  permission to invent live directory evidence. Alias cycles, dangling links,
  original-file symlink ancestors, case/NFC aliases, and unused aliases refuse.
- `edges` are sorted by original image path and load-command index. Each row
  requires `source`, `index`, `command`, `name`, `scope`, `candidates`, `selected`.
  The only dependency command is `LC_LOAD_DYLIB` (12). Scope is `runtime` or
  `normal_os`. Each ordered candidate has `path` and `resolved`; JSON null
  explicitly means an absent leaf. A dangling leaf symlink is never absence.
  Every present candidate must identify the same captured original. Normal-OS
  edges have no candidates and null selected image.

The bundle is the ASCII magic `SORAFS_JAVASCRIPT_RUNTIME_INPUTS_V1` plus LF,
unsigned big-endian u64 manifest byte length, that exact manifest, then each
image's original bytes in the manifest's sorted order. Every size/hash, the
complete load graph, exact candidate sequence, aliases, original install IDs,
reachability and EOF are rechecked. No decoder opens the historical path labels.
`NodeRuntimeBundle` retains original bytes and member offsets, not physical or
execution authority; its dataclass must never be used as an approval token.

Bounds apply before decoding/copying the relevant payload: 4 MiB manifest,
128 images, 256 MiB per image, 512 MiB image total, 256 aliases, 4096 edges,
16384 candidates, 32768 namespace nodes and 4 MiB namespace spelling bytes.
Paths are NFC, at most 4096 UTF-8 bytes/64 components/255 bytes per component.
A cheap character-count refusal precedes UTF-8 allocation, including direct
projection API calls outside the bounded manifest parser.
The shared Mach-O parser receives original bundle bytes and offsets, avoiding
full image copies. Fixed-header admission caps each command table to 1 MiB and
4096 commands before that parser allocates command/string slices. Derived edge
and candidate budgets are consumed across images before projection allocations.
These are bounded component limits, not funded production admission evidence.

## Deliberately restricted loader relation

`sorafs_javascript_runtime_graph.py` calls the existing sole
`copy_sumeragi_v2_release_cargo_cache_cli.py::_parse_macho_thin`; it does not copy
or replace the Mach-O decoder. A thin little-endian arm64 CPU subtype 0 image
must have the expected executable/dylib type and one fixed `/usr/lib/dyld`
launcher command where appropriate. Fat/arm64e/other architectures, weak,
reexport, lazy, upward, environment and unknown commands are rejected. Dylib
install IDs must be canonical captured-original names, resolve to that same
image and be unique, preventing a claimed cached-name substitution.

The main executable's `@rpath` and a bounded shared-image input relation are
projected. Original rpaths retain order, including bare `@loader_path`; each
absent earlier slot stays explicit. Shared requesters must be reachable from the
executable through independently resolved direct loads. Every direct ancestry
route must produce the same ordered candidate sequence; distinct route
sequences, cycles, unreachable shared requesters and the configured state,
candidate or byte bounds refuse. Shared dependencies do not establish the
ancestry of another shared requester in this restricted profile.
Direct canonical absolute names and reviewed loader/executable token expansions
are supported. Internal `..` that could cross an alias is refused. The fixed
normal-OS boundary is direct canonical `/usr/lib/` and `/System/Library/` load
names; a caller cannot supply prefixes or hide traversal through that boundary.
Casefold variants of those prefixes and either directory endpoint are refused;
this is a restricted spelling policy, not inferred physical canonicalization.

The pure relation requires every candidate slot across the common ancestry
sequence to be declared and all present slots to resolve to one original. It
retains unique absolute install IDs and requires repeated `@rpath` load names
in the captured graph to select the same original. Apple's dyld source pushes
the requester on the
rpath stack, then visits ancestors until a loadable candidate is found. It also
checks already-loaded rpath matches before searching. The three-case host
loader control confirmed that a present own candidate can be skipped as
unloadable and an already-loaded matching install ID can override another own
candidate. Thus **this relation cannot establish actual loadability, cached
selection or mapped bytes**. It confers no runtime approval or process authority.

The recorded local Homebrew Node24 20-image manifest omits four original
executable-ancestor slots across its two Brotli shared `@rpath` edges. It
remains rejected by the exact command/candidate relation. The independently
pinned complete original namespace, physical absent-slot/alias custody and
same-process runtime observation are still absent. No complete local runtime
acceptance or production evidence is claimed.

Re-deriving against those captured image bytes yields, for each Brotli edge,
its own present `libbrotlicommon.1.dylib` candidate followed by the claimed
absent `node@24/24.21.0/bin/libbrotlicommon.1.dylib` and
`node@24/24.21.0/lib/libbrotlicommon.1.dylib` ancestor slots. An in-memory
manifest amended with all four null-resolved slots satisfies this pure content
relation against the unchanged 20 image byte strings; its manifest digest is
`8a5f68d1ca035abf3de72010cdd6d906f2eec05b5047af43723535cfd9e7e51b`.
The pure producer re-derives that same diagnostic digest from the supplied
20 original byte strings, original modes and literal aliases; reframing those
bytes with its output passes the pure parser (20 images, 74 edges). That digest
is an observation, not an independent approval pin. The recorded
manifest and bundle remain rejected, and the amended null claims still require
physical absence custody. Accepting the recorded bundle by skipping derived
slots would weaken the exact original command/candidate relation.

Primary evidence: the local `dyld(1)` run-path description and Apple's
[Loader.cpp](https://github.com/apple-oss-distributions/dyld/blob/fd8d0c4d52320ebf64db34f3cb280310d905c5ae/dyld/Loader.cpp)
and [JustInTimeLoader.cpp](https://github.com/apple-oss-distributions/dyld/blob/fd8d0c4d52320ebf64db34f3cb280310d905c5ae/dyld/JustInTimeLoader.cpp).
These justify the conservative refusal; they are not a seal of the host's actual
mapped dyld or a complete executable loader model.

## Remaining production joins

`HeldRuntimeAbsentLeaf` and `HeldRuntimeAlias` in
`scripts/sorafs_javascript_runtime_custody.py` now hold one canonical
no-follow parent lineage each. The former rejects an absent candidate if a
file, dangling symlink, parent replacement, or unsafe writable parent appears;
the latter rechecks a symlink's literal target and inode without following it.
Their four local leaf tests pass. `OriginalNodeRuntimeInputs` parses the independently
pinned complete original-byte bundle, reserves at most 2,048 image/alias/absence
handles including one transient recheck lineage, and retains every declared
image, alias and absent leaf through a one-shot physical input scope. Its five
focused assembly tests, including partial-construction, failed-entry and
changed-exit descriptor cleanup, pass; the combined physical/pure selection
passes 133 tests. These are input owners only; the fixed child, physical alias-resolution
join and actual runtime observation are not connected.
An absent path through a declared directory symlink must first be resolved and
authenticated at its canonical parent; a symlink traversal alone never proves
absence.

TODO: extend beyond the restricted direct-ancestry profile only with a reviewed
complete inherited/cached candidate relation. Preserve original slots and
installed-name identity; do not infer loadability or runtime selection from a
successful library-list observation. Obtain an independent approval pin and
physical absence/alias custody for a freshly captured complete original bundle
before reconsidering the local Node profile.

TODO: apply the bounded physical owner to independently pinned actual Node
originals, complete the declared-symlink-to-canonical-target join, and retain
those owners through the selected process's final EOF/exit and failure cleanup.
The synthetic owner tests and original-byte input parser do not establish a
physical owner for the rejected local 20-image runtime.

TODO: join independently reviewed runtime pins, scrubbed fixed Node24 launch,
actual same-process runtime/native-checker observations, original input fd3,
all unchanged SDK cases, EOF/exit/cleanup and signed original-index publication.
Normal OS/toolchain qualification remains explicit; this is not sandboxing or
mapped-memory attestation. No Linux ELF or other platform fallback exists.
