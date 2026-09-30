# Sealed occupied dispatcher transition

The root-only Linux owner advances exactly `/usr/local/libexec/iroha-taira-public-reset-v1`
and the five existing role `guard.json` files. It leaves state, current selectors,
configuration, service definitions, services, trust, lease/progress and journal
history unchanged. It supports the explicitly stopped four-validator predecessor,
active edge, and absent epoch supervisor. It does not qualify prior daemon execution
or reinterpret an installed launcher's program; the current typed runtime input
must select the independently reviewed actual daemon and unit bindings.

Use the exact `iroha` executable in a successful maintained release import. The
import includes `preparation/{result,request,checks,capture}.json`, all mode0400,
bound into its ten-payload request and completed transfer. Preparation, compiled
source identity, all four transferred binaries, source receipt and completed
transfer must join. Six-payload imports do not satisfy this command.

```sh
"$IMPORTED_IROHA" taira public-reset capture-dispatcher-current-runtime \
  --expected-host-identity-sha256 "$APPROVED_GUEST_HOST_IDENTITY_SHA256" \
  --output "$CURRENT_RUNTIME_INPUT"

"$IMPORTED_IROHA" taira public-reset prepare-dispatcher-transition \
  --import-root "$COMPLETED_IMPORT" \
  --expected-result-sha256 "$QUALIFIED_RESULT_SHA256" \
  --retained-inventory "$RETAINED_INVENTORY" \
  --expected-retained-inventory-sha256 "$RETAINED_INVENTORY_SHA256" \
  --current-runtime "$CURRENT_RUNTIME_INPUT" \
  --expected-current-runtime-sha256 "$CURRENT_RUNTIME_SHA256" \
  --trusted-public-key "$EXISTING_TRUSTED_KEY" \
  --operation-id "$OPERATION_ID_32_HEX" \
  --output "$PRIVATE_PLAN_PATH"

"$IMPORTED_IROHA" taira public-reset dispatcher-transition \
  --plan "$PRIVATE_PLAN_PATH" --expected-plan-sha256 "$PLAN_SHA256" --action check
```

The output parent must already be an owner-private directory. No command opens an
SSH connection or reads client/signing credentials. Invoke through the approved
pinned route as root; authorization for ledger replacement remains the separate
reset coordinator's responsibility.

Run capture on the approved Linux guest after stopping all four validator units
and before starting the replacement deployment. It reads the guest's Ed25519 SSH
host public key, acquires the existing transition locks, checks all four loaded
systemd units are stopped and nginx is running, derives the configuration release
from each `current` symlink and the daemon revision/argv from each installed unit,
hashes all installed artifacts, and records exact stopped state identities. It
refuses changed selectors, units, files, host identity, or unsafe ownership, then
atomically writes a new mode0600 typed record. Its JSON result prints the digest
for `--expected-current-runtime-sha256`; the operator never edits runtime JSON.
The installed unit source revision is the revision in its exact daemon path, which
must use the current `release-<commit>-update-<operation>` launcher form.

`current-runtime` is a mode0600 closed Norito JSON record with
`schema: "iroha.taira.dispatcher-current-runtime.v1"`, `host_identity_sha256`,
`validators`, and `edge`. The four ordered validators are existing current
`ValidatorAdmittedReleaseV1` values (configuration commit/release root, exact argv,
five ordered `OccupiedArtifactV1` records and explicit stopped state device/inode).
The edge is `EdgeAdmittedReleaseV1`. These are the same prior-release records used
by current reset input construction. Executable and unit source revisions remain
independent of the configuration release. Select them from retained admitted update
facts plus fresh observations; do not infer the executable from the configuration
selector or substitute inventory artifacts from an earlier deployment. Unknown
schemas and fields fail. The retained inventory is hash-bound as opaque history;
it is not decoded through an older schema or re-admitted as a current candidate.

The producer acquires the existing updater, journal, and host action locks and
retains all input descriptors while deriving a private plan. It derives current
selector/state identities, exact old controller/guards, and the sealed completed
lease/progress/terminal pins. Retained positive terminal counters describe that
exact pinned predecessor proof, independently of the candidate's execution steps.
Installed guards must already bind the supplied trusted public-key bytes. Review
the generated plan and digest before invoking `--action apply` with those same
arguments. No new signing key or trust root is created.

Apply first retains exact old bytes, publishes durable ownership and identity
records, removes the fixed dispatcher, then rejects live references to its inode
through process executables, descriptors and maps. Each guard is published under
that absence barrier; the dispatcher is published last. Locks alone are not a
claim that old pre-admitted requests are gone. Partial operations fail closed and
resume using the same plan and `--action apply`. Exact partial prefixes are
resumed; changed files, inodes, foreign namespace entries and mismatched intents
are refused. Check reports target guard hashes; successful apply reports observed
result hashes for the next reset's public input producer.

Copy staging stays private at mode0600 until its complete bytes match the pinned
source. The held descriptor then receives the exact final mode before exclusive
publication, independently of the inherited umask. A resumed private prefix must
match the source; a staged file already carrying the final mode must be complete
and hash-identical. Other modes, changed inodes and foreign bytes are refused.

Use the same pinned command with `--action rollback` to restore all six exact old
byte sequences. Rollback first revalidates the unchanged lease/progress, current
selectors, state identities, file pins and service states, then uses the same
absence barrier. It refuses after a new deployment starts or protected runtime
changes. Interrupted rollback resumes with that same action. Completed repeats
verify custody without republishing. Retained backups and control history are
never removed by either action. A rolled-back operation cannot be reapplied.

The transaction requires guest space for both dispatcher copies plus the existing
2GiB deployment reserve. The operator must also pass maintained Mac backing-store
capacity admission before dispatch; guest free space does not establish APFS
capacity. The transition is controller preparation, not a release qualification
or a ledger-reset receipt.
