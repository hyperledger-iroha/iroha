# Canonical source publication lifetime

`compute_workspace_source_manifest.retained_workspace_source(root, digest)`
retains the canonical census while a publishing parent invokes its native child.
The aggregate still belongs to the existing workspace source owner: this API
does not introduce a file containing that digest, a second census, or a claim
about Cargo's consumed dependencies and generated inputs.

The context captures every selected regular file, symlink and parent original.
Canonical hashing reads duplicate retained file descriptors and original link
targets. Source inputs require single links; Cargo's separately admitted
`RetainedBuildInput` hard-link policy does not apply to source. The caller checks
`owner.revalidate()` before invoking publication and after receiving its durable
seal acknowledgement, before accepting the manifest anchor. Context exit also
checks custody. A failed observation permanently refuses that keeper.

Native Windows requires local fixed NTFS and native Python. Full volume/file
identity, native metadata and owner/group/DACL bytes are retained. Regular-file
handles exclude writers and deletion. Permanent subtree and parent notifications
also refuse transient namespace/security changes, including a root-only ACL
change restored before the next check. An unrelated sibling mutation can
conservatively refuse the parent latch, so source needs a dedicated parent and
all generated output outside the watched tree. This code observes existing
security descriptors; canonical publication permissions remain owned by
`iroha_fs`.

Only an actual supported symbolic-link reparse payload is decoded. Its target
uses CPython's `os.readlink` spelling; materialized Git symlinks remain regular
physical originals under the existing canonical logical-link policy. Junctions,
unknown tags and unavailable WSL/remote link payloads remain refused. Native
symbolic-link execution tests require real Windows symlink privilege; parser or
junction controls do not replace that evidence.

`pytests/scripts/workspace_source_keeper_test.py` uses unrelated temporary source
trees. It covers canonical aggregate equality, original-reader use, wrong
anchors, hard-link refusal, mutation/restore, original link targets, indirect
ancestry, native child writer exclusion and native root-security latching.
The producer adapters are not yet wired to this API; WI11 retains its explicit
release-host refusal until the complete native publisher/consumer path passes.
