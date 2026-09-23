# Retained JavaScript child input boundary

`scripts/sorafs_javascript_parent_input.py` constructs the existing fixed child
input from original content relations and retains its actual file descriptor.
It is an input-only prerequisite. It does not verify a candidate, execute a
native ABI check, qualify Node, launch the child, or authorize promotion.

`OriginalJavascriptChildInput` accepts the actual `InstalledProjection` and
`QualificationProjection`, independently selected commit and both source
digests, the original native/checksum/ABI paths, the original tool directory,
and private environment/temporary roots. The selected target is a POSIX
`darwin|linux` / `arm64|x64` Node 24 label. Original installed labels retain the
existing bounded ASCII path contract. No alternative archive or manifest parser
is introduced; the constructor uses the canonical projection validators and
ABI manifest schema/serializer.

The parent prepares `node_modules`, `qualification/core` and
`qualification/tools` before entering the owner. The exact 191-file source core
is unchanged. The eight private tools have a separately reviewed literal source
selection in `sorafs_javascript_child_tools.py`; caller-supplied hashes cannot
replace this selection. Original tool descriptors and the exact copied tool
tree remain held. Source/candidate authorization still belongs to the parent.

The actual original addon, adjacent checksum and ABI manifest are held through
construction and recheck. The ABI manifest's artifact size/hash, target, commit
and workspace source digest must match the selected relation. The installed
package checksum bytes must equal the held checksum original; selected native
source-context fields must match the separately selected native source digest.
This projection does not replace the actual native loader's checksum or
re-signing verification. Parsing an ABI manifest does not prove its observations.

Only `environment/child-input.json` is created, exclusively at mode 0600. Its
closed `sorafs.javascript.child_input.v1` document has sorted member rows and
canonical ASCII JSON plus one newline, bounded at 8 MiB while encoding. The
borrowed `descriptor` and derived `sha256` are usable only after successful
construction. The future fixed process owner must map that original descriptor
to child fd 3 and retain this owner until process/output completion. The original
created descriptor is readable and writable; this component provides no
read-only-descriptor or process sandbox claim.

Original native hashing streams 64 KiB reads, using the existing 1 GiB ceiling
from the native snapshot/index owners, without retaining a native-sized byte
buffer. Captured tool, checksum and ABI bytes have separate finite limits.
Original ancestor descriptors and full file seals, including ctime, stay bound
to the observed files. Environment/temporary roots retain private mode 0700 and
original ancestry while allowing unrelated sibling work. Executed trees,
temporary storage, originals and output have explicit separation checks.

Refusal is terminal. Reentrant observations, even when their errors are caught,
and close during observation prevent outer success. Cleanup detaches ownership
before attempting every original descriptor once, retains cleanup errors, and
never retries an ambiguously closed number. Failed or partial request files
remain available for inspection. Cleanup never unlinks a substituted pathname.
Before/after observations cannot exclude transient restored pathname changes or
attest mapped code; normal OS/toolchain trust remains explicit.

TODO: connect the separately owned genuine native checker, complete original
Node 24 runtime and npm input custody, actual process/fd/pipes/EOF handling and
original-index publication. Authenticate clean candidate source and both source
domains before and after execution. Run the unchanged 55 cases / 172 assertions
with the actual rebuilt addon. Current controls use inert archive/native bytes,
real local file ownership and the existing pure child parser only; they do not
establish SDK, native, deployment or release qualification.
