# Native filesystem custody

`iroha_fs` owns the native filesystem boundary shared by CLI workspaces, contract
artifacts, SDK key loading and daemon credentials. It owns no codec, secret format,
network policy, process signalling or persistent PID authority.

- `PrivateDirectory` requires current-user private custody. `OwnerDirectory`
  admits reader-shared project directories while refusing foreign mutation.
  Both retain ancestor authority and create private files/directories.
  `OwnerDirectory::open_private_child` and its optional form share the original
  ancestor handles while enforcing private child custody. Optional admission
  accepts absence only at the first native child open.
  `open_retained_regular_optional` retains reader-shared files under the same
  original ancestry; Windows excludes writers and deletion for that lifetime.
- `read_private` and `read_regular` perform bounded, stable descriptor reads into
  zeroizing allocations. `RetainedFile` supports streaming with explicit custody
  revalidation; `seal` freezes a completed writer's observation without reopening.
- Immutable journals use `open_exact` to reject all directory redirects and
  noncanonical spelling. `visit_private_files` streams bounded metadata, including
  incomplete private tombstones, without collecting names or reading bodies.
  Relative retained files share the existing ancestor handles.
  `OwnerDirectory::create_retained_private` uses the same bounded writer in a
  reader-shared project directory without changing its existing permissions.
- `read_tree_scope` shares only an identical retained ancestry prefix. Its
  `read_scope` callback brackets one directory's ordered reads and bounded
  inventories with fresh entry and exit checks; persistent exit custody errors
  take precedence over every ordinary callback result. Each leaf and inventory
  keeps its native admission and snapshot checks. Directory changes restored
  before exit can be unobserved: this is a closed read operation, not an atomic
  filesystem snapshot. Views expose no descriptors, paths or mutation methods.
  `read_admitted` preserves a caller-owned plain `Vec`; `read_admitted_zeroizing`
  uses the same native checks but preserves its zeroizing allocation through read
  errors and the outer custody fence. Transfer to a plain output only after that
  fence succeeds; neither variant duplicates the plaintext allocation.
- `PendingPrivateFile` permits bounded Write/Seek without exposing its descriptor.
  Consuming `seal_read_only` enforces exact Unix `0400` or a protected owner-read-only
  Windows DACL and returns opaque `SealedPrivateFile` with only Read/Seek. Recovery
  through `open_retained_read_only` rejects writable or oversized files. `publish_new_name` consumes
  the exact sealed creator for durable no-replace sibling publication; failures
  preserve evidence for reconciliation. Original descriptor rights stay private
  through publication; no writable handle can escape from either capability.
  Native libraries instead consume the original writer with `seal_executable`,
  which returns only an opaque move/read owner and enforces exact Unix `0500` or
  a protected current-user read-and-execute DACL. This separate capability keeps
  executable files out of ordinary receipt admission. Native Windows tests
  compile and load a real DLL after publication and verify that ordinary
  read-only receipt sealing still refuses DLL loading.
- `ReaderDirectory::open_sealed_executable` recovers an existing single-link
  executable with the same exact current-owner execute seal and a fixed byte bound.
  Its opaque `SealedExecutable` exposes only checked Read/Seek and original
  identity/snapshot observations, with no descriptor, write or publication authority.
  Native tests keep that reader and its original ancestry alive throughout actual
  process execution and compare the bytes and snapshot afterward. The parent retains
  ordinary safe reader policy; this does not seal a release directory, authenticate
  executable contents or establish an owned release-bundle spawn boundary.
- File publication stages and syncs before atomic replacement. Directory
  publication consumes the retained source, requires an absent destination, and
  rejects live descendants. A failure after publication requires reconciliation.
  The observed atomic writer reports `AtomicWriteStep::Published` immediately
  after native destination publication, before any subsequent file flush,
  custody check or directory sync can fail. Callers use this boundary to
  distinguish refused attempts from visible results with uncertain durability.
  `create_custody_writer` leaves journal durability ordering with its caller:
  write the complete payload, sync that original file, publish it, then sync the
  directory. Creation does not flush the empty staging file or its temporary
  name. Generic retained-file creation keeps its existing immediate syncs.
  `create_private_file_into` installs an exclusively created original in the
  caller's empty slot before fallible checks or syncs. Errors preserve that
  handle so the caller can retain its physical reservation for recovery.
- `OwnerDirectory::open_movable_regular` admits an original single-link file with
  retained move authority. Opaque `MovableFile` moves across retained directories
  without replacing an existing name; `MovedFile` keeps both parents, the same
  file identity and the exact post-rename snapshot. Source absence is verified
  through both namespace syncs and can be checked separately after return.
  `SealedPrivateFile::into_movable` transfers the original creator handle without
  reopening or upgrading recovered files. `PendingPrivateFile::finish_private`
  syncs and consumes a bounded creator without changing its ordinary private
  permissions; it returns only read/seek/move authority, not a receipt seal.
  `FileMoveStep::Published` reports the native rename before any later fallible
  checks; observers latch that state before returning an error. Callers sync source
  contents before admission and persist their own intent before moving. Interrupted moves never
  roll back or remove remaining files. Linux, Android and Apple use atomic native
  no-replace rename; Windows uses the original delete-capable write-through
  handle. Other Unix hosts refuse moves until an atomic native primitive exists.
- `OwnerDirectory::move_no_replace` transfers an original directory between
  retained parents, refusing shared originals and descendant destinations.
  `MovedDirectory` retains both ancestry chains through the explicit transfer
  back into a mutable owner. Exact original metadata and Windows owner/group/DACL
  custody survive native authority transitions; only rename change time may vary.
  Both parent namespaces are synced. `DirectoryMoveError::publication_may_be_visible`
  is set immediately after the native move, before any observer or validation can
  fail. Callers preserve durable intent on that error and never infer permission
  to roll back or delete either remaining name. Tree contents must already be synced.
- `OwnerDirectory::remove_private_if_snapshot` joins a previously verified private
  staging snapshot to the exact original admitted for deletion. Replacement, changed
  extent or custody refuses before deletion; only first-open absence returns `false`.
  Callers verify their bounded bytes and capture the snapshot through the same reader,
  then release it before the Windows DELETE-capable admission.
- Advisory `open_lock` returns an unlocked persistent file. The caller acquires
  `File::try_lock`. Managed children use `open_ownership_lock`: Windows also
  retains a writer-sharing fence through inherited duplicates, independent of a
  process-scoped byte-range lock.
- Windows `observe_volume_identity` and `observe_volume_space` expose bounded native
  GUID/capacity queries for local planning. These path-based observations confer no retained
  namespace or file authority; quota-aware available bytes are preserved.
- Windows `current_user_local_data_directory` retains the effective user's
  OS-selected LocalAppData directory using the actual token. It rejects network
  drives and reparse paths and ignores environment directory overrides. Callers
  create their own private child beneath that retained current-user directory;
  configured local folder redirection receives the same custody checks, and the
  existing profile directory's permissions are never repaired.
- Reset holds the caller's operation/runtime locks and clears contents through
  retained authority, preserving those exact lock names. Unsafe entries cause an
  error and may leave a partially cleared directory.

Unix admission uses owner modes, no-follow descriptor-relative operations,
single-link files, metadata/namespace revalidation and directory `fsync`.
Root-owned sticky temporary ancestors and immutable root-owned system aliases
are admitted by general paths; `open_exact` rejects aliases. macOS extended ACL
grants are checked separately from mode bits.

Windows admission requires a local NTFS volume, protected current-user DACLs for
private material, safe system/current-user ancestors, no reparse points, and
single-link files. Ancestor handles deny delete sharing. Publication retains the
exact source identity, renames by handle and uses write-through metadata plus
file flushing. This follows the NTFS behavior documented in
[Microsoft's file caching contract](https://learn.microsoft.com/en-us/windows/win32/fileio/file-caching).
Append-only Windows logs use write-through instead of requesting general write
authority for a later `FlushFileBuffers` call.
Public ancestry admits the exact Windows SYSTEM, Administrators and TrustedInstaller
identities; other service identities remain untrusted. Private objects still require
the current user's ownership and protected user-only permissions.

All platforms assume the operating system and storage honor their native
access-control and durability contracts. Privileged administrators and another
process already running as the same user are not separate secret principals.
Namespace changes are rejected; this crate does not provide a second user sandbox.

The `platform_native` workflow runs focused custody and AVX2 correctness on native
Linux x86-64, macOS Intel and Windows MSVC, retaining failures and actual logs.
Its Windows runner must permit file and directory symlinks; missing privilege is
a failed preflight, never skipped evidence. Full installed bundles retain the
separate `devex_native` gate.

Run `cargo test -p iroha_fs` on each native release runner. Cross-target
`cargo clippy -p iroha_fs --all-targets --target <target> -- -D warnings` checks
platform compilation but does not qualify native ACL, sharing, rename, or crash
durability behavior. Windows-specific tests include protected DACL admission and
inherited-handle fencing; release qualification must execute them on Windows.

Private-file admission costs are derived in
[PRIVATE_FILE_ACCOUNTING.md](PRIVATE_FILE_ACCOUNTING.md). These are finite local
resource charges, not consensus parameters or native-platform qualification.

`OwnerDirectory::remove_build_output` retires an exact Cargo output under the
caller's retained exclusive build lock. Its explicit build-cache policy admits
the original hardlink count, freezes the complete original snapshot, and checks
that deletion removes exactly one link while preserving the other aliases.
Ordinary and private file removal retain their single-link requirement; retained
build-input readers never acquire deletion authority.


`PendingFlatBatch<N>` exclusively creates a final-name private directory and a
fixed inventory of bounded executable/read-only child writers. Sealing consumes
every writer, gives the original parent exact Unix `0500` or protected current-SID
read/execute access, and retains all original children through independent
read-only handle handoffs. No child owner, mutable directory or native handle
escapes; generic directory-move descendant guards remain unchanged. An interrupted
seal preserves its names and cannot return a sealed capability. Existing batches
are recovered through `ReaderDirectory::open_sealed_flat_batch`, with the same
strict parent policy, exact inventory, original identities and child byte bounds.
The caller authenticates the manifest and executable contents.

The creator records the original parent principals before any child writer or
observer is exposed. Windows also freezes its complete security descriptor and
retains a parent-security notification through the private creation phase. Each
child-sealing callback must leave that state unchanged, including transient ACL
changes later restored. Only the deliberate final DACL change is admitted; its
owner and group must stay equal to the creator's. Descriptor control permits only
the native setter's addition of `SE_DACL_AUTO_INHERITED`; protection and all other
control bits remain exact.

Each immutable ancestry link carries its original directory seal. Windows retains
two non-resetting notifications: direct child namespace/security/content changes,
and security changes to direct children of the original parent (including the
batch root itself). The latter conservatively invalidates the batch when a sibling
directory's security changes. All directory/leaf owner, group and DACL bytes stay
bound across native creator-to-reader transitions; ordinary immutable readers
retain no writable or delete-sharing handle. Unix binds exact original directory
and child metadata, including ownership and change times. Resource ownership is
bounded by the fixed inventory and retained ancestry; Windows holds one temporary
creator notification and two final notifications per batch. Reading bodies uses
caller-provided buffers.

`Arc<SealedFlatBatch<N>>::command` projects a launch command only for executable
entries. Linux/Android require native ELF and execute the exact retained descriptor
through `/proc/self/fd`, capturing the batch inside the command. Windows projects
the pathname pinned by its non-delete-sharing ancestry; the release command owner
must retain the batch through spawn. On every platform the caller retains the batch
through child exit and revalidates around execution. This API does not authenticate
dynamic dependencies. Other Unix descriptor execution remains explicitly
unsupported pending a proven native mechanism; there is no mutable-path fallback.


`ProcessCommand` consumes one absolute native executable selection and explicit
`ProcessStdio` values. It has no raw standard-library command conversion and
confers no source, executable or release qualification. `SealedFlatBatch::command`
returns this same process type while preserving its original descriptor/path
selection. Unix retains descriptor mapping and process-group configuration.

Windows installs its unnamed Job and exact inherited stream list as part of
`CreateProcessW`, before the child runs. `ProcessChild` retains original process,
primary-thread and Job handles, and completion requires root exit plus zero live
Job members. Only explicit disk, pipe or character streams are admitted. Named
pipes use the canonical protected effective-owner DACL. Temporary inheritable
stream duplicates can still be inherited by an unrelated concurrent launcher
using unrestricted native inheritance; this API makes no broader exclusion claim.

`ProcessChild::wait_async` needs no Tokio runtime or timer driver. Its cancelable
wake worker holds only wake state; the child stays available for polling and
termination. Drop reaping uses original objects and preserves them indefinitely
if terminal observation or reaper creation fails. A caller with admission custody
must keep that owner paired with the child through every wait, cancellation and
reaper path; the release bundle wrapper enforces this additional obligation.

`Arc<RetainedBuildInput>::command` selects the already retained native tool.
Linux/Android execute the original descriptor even if a mutable ancestor is
replaced between validation and launch. Windows preserves original path sharing
exclusion. This does not seal mutable input bytes or authenticate dynamic
dependencies. The caller retains and revalidates its tool, configuration, source
and lock owners through actual child completion; the command does not establish
that wider lifetime on its own. Other Unix descriptor launch remains unsupported.


`ProcessCommand::env_retained_build_input` binds a selected tool for a child
launcher. Linux keeps its new duplicate CLOEXEC in the parent and clears that bit
only in the intended child; no parent-PID lookup or parent inheritance window is
used. Windows preserves the original path under native sharing exclusion. Apply
the binding after environment defaults and preserve all caller custody until
terminal observation. Descendants must preserve both the value and descriptor;
this API cannot impose that behavior on another launcher. A descendant sees the
descriptor spelling as argv0, so native tools require an exact mode preflight.

The Linux original-descriptor control covers Cargo invoking original rustc and a
build script invoking that same inherited rustc. `env_retained_build_input` alone
does not constrain Cargo's rewriting of build-script `CARGO` from its current
executable pathname. Arbitrary recursive Cargo therefore requires the separate
sealed-image mechanism below and its genuine recursive qualification. Direct
`RetainedBuildInput::command` preserves the original tool spelling as argv0 while
selecting the original file.

`SealedFlatBatch::sealed_tool_image` creates a distinct anonymous Linux/Android
executable from an admitted batch entry. Complete bounded readback joins every byte;
the kernel enforces content, size and execute-bit seals. The batch remains a held
witness, not executable pathname authority. `ProcessCommand::env_sealed_tool_image`
binds a child-only inherited descriptor using the existing owned-duplicate mechanism.
Commands use a fixed non-multicall argv0, and the anonymous diagnostic name cannot be
renamed to influence Cargo's current-executable preference. No raw file/descriptor
escapes. Complete source admission and original-child terminal ownership remain the
caller's responsibility. Executable memfd creation and all required seals must be
permitted by the running kernel/security policy; no pathname fallback or policy change
is attempted. These APIs do not authenticate dynamic dependencies. Genuine recursive
Cargo and mutation controls remain mandatory before qualification.
