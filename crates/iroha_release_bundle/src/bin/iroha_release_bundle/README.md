# Native release bundle command

This command is the first-release native owner. The Python/shell adapters have not
yet switched to it. It does not compute or authenticate the complete workspace
source census: the parent must wrap the entire call in
`compute_workspace_source_manifest.retained_workspace_source`, pass that canonical
aggregate, and revalidate the live source owner after successful exit before
accepting either output field. A digest alone is not full source custody.

All arguments are explicit, non-repeated option/value pairs. Paths must be absolute,
normalized UTF-8 without control characters. Successful publication/admission emits
exactly `bundle_dir<TAB>path<LF>manifest_sha256<TAB>digest<LF>` on stdout. Build output
and diagnostics go to stderr. Any nonzero exit invalidates all returned output.

`publish` requires `--parent`, `--name invocation.XXXXXX`, `--build-dir`,
`--source-sha256`, `--cargo-lock`, `--cargo-version`, and `--rustc-version`.
The supplied parent must already have canonical private custody. The native
publisher retains the four build originals, lockfile, and private successful tool
transcripts through complete flat-five sealing and content authentication.

`admit` requires `--parent`, `--name`, `--source-sha256`,
`--cargo-lock-sha256`, and `--manifest-sha256`. All three anchors come from the
parent invocation, never from the bundle being admitted.

`build-publish` requires `--repo-root`, `--source-sha256`, `--cargo-target-dir`,
`--artifact-root`, `--cargo-home`, `--cargo-bin`, `--rustc-bin`, `--cancel-request`,
and `--name`. Existing Cargo/artifact roots and the cancellation parent are private.
Output roots cannot overlap each other, the source, or Cargo home. The marker file
must be outside the source and outputs; its private parent may contain sibling
output directories. Physical checks compare retained native identities and ancestry,
including aliases with different path spelling. The helper retains config originals, pinned tool originals, the
current lockfile, both original persistent file locks, and private cache directories
through three `-j1 --locked --offline --release --target <native-target>` builds.
The fixed programs are `iroha3d`, `iroha3d_taira`, `iroha`, and `kagami`; Windows
publication requires their exact `.exe` names. The output directory is
`<cargo-target>/sumeragi-release/<source>/program-build-cache/default/<native-target>/release`.
Only those four cached output names are removed to require relinking. Other Cargo
hardlink aliases are preserved by the canonical lower owner.

The helper fixes `CARGO`, `RUSTC`, `CARGO_HOME`, `CARGO_TARGET_DIR`,
`RUSTUP_AUTO_INSTALL=0`, `ENABLE_RANS_BUNDLES=1`, and `NORITO_SKIP_BINDINGS_SYNC=1`
for its child commands. It never installs a toolchain, signals an existing process,
or treats failed tool output as a successful transcript. On Linux/Android, Cargo
is copied from its authenticated retained installed original into a fresh immutable
batch witness. A second, distinct anonymous memfd is copied and compared completely
against that witness, then kernel-sealed against writes, extent changes, execute-bit
changes and changes to the seal set. Only this anonymous image executes. Its diagnostic
name cannot be renamed to `cargo`, and its initial argv0 is fixed to a non-`cargo`
name; recursive commands use the inherited descriptor path. Renaming a filesystem
witness cannot select a descendant executable, although it still rejects final custody.
Installed source, witness and image remain held through terminal observation.
Executable memfd policy and all required seals (including Linux 6.3 execute sealing)
are mandatory; policy denial or unavailable support refuses without fallback or host
configuration changes. The additional memory allocation is bounded by the admitted
Cargo extent (maximum 2 GiB); ordinary resource admission remains required.
Original-source version output remains the proxy preflight baseline. Windows and Rustc
keep their existing original-tool selection. Dynamic dependencies remain separately
trusted; authored controls and source review are not runtime/release qualification.
Cancellation is the exact
existing canonical private request, checked between commands (exit125); an active
child finishes normally. A terminal observation error keeps the synchronous owner
alive until the original child is observed, then rejects the invocation using the
first error. Version output is bounded, with overflow still drained before reaping.
Sealed tool transcripts transfer from their creator through a retained read-only
bridge to a restrictive reader, comparing full snapshots and exact authored bytes.
Failed and completed private transcript directories
remain available for explicit diagnostics; automatic cleanup does not reacquire
or delete a potentially changed name.

Optional `--bundle-dir` and `--manifest-sha256` must be supplied together. They
authorize admission only: partial/invalid inherited anchors fail without creating
cache directories, acquiring locks, replacing bundles, or running builds. An
unanchored environment target is ignored. The parent supplies a fresh six-character
alphanumeric invocation suffix; actual exclusive native creation establishes
uniqueness, and collisions refuse without replacement.

TODO: qualify the native CLI, final adapter replacement, and full release producer
and launch consumer together before retiring the existing fail-closed boundary.

`build-bridge-publish` takes the same explicit required inputs as `build-publish`,
with no inherited bundle arguments. On Linux/Windows it performs a fresh locked,
offline native release `--lib` build of `iroha_process_bridge` in its own invocation
namespace. The Cargo protocol must identify the exact package, manifest, library
source, `cdylib`/`rlib` target, empty feature set, native filename and successful
terminal event. It retains that emitted original through copying and sealing the
exact two entries (`libiroha_process_bridge.so` or `iroha_process_bridge.dll`, plus
`publisher-source.json`) under `<artifact-root>/sumeragi-release/<source>/bridges/<name>`.
The normal Norito publisher record binds original tools, lock, source aggregate,
configuration and actual build environment. The library is not executed by this
operation. Its stdout is exactly `bridge_dir<TAB>path<LF>module_sha256<TAB>digest<LF>`
followed by `provenance_sha256<TAB>digest<LF>`. Every owner is revalidated after output.
These observed digests require independent approval/signing before a later loader
may trust them; this command never authenticates its own execution or source owner.
The enclosing canonical source keeper must survive through the complete operation,
including failure/terminal observation. No diagnostic provenance fixture becomes
publication authority. TODO: qualify actual emitted-library/public-CLI delivery and
finish the publisher-backed harness fixture migration on the complete candidate.
