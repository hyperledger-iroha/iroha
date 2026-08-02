# Runtime-provider broker deployment contract

These assets supervise an operator-supplied V1 runtime-provider broker. The
repository supplies the authenticated client/server protocol and the
credential-free process shell; it does not supply a concrete HSM, KMS,
WebAuthn, immutable-query, sealed-store, archive, or network backend registry.
The installed executable must statically link a reviewed deployment-owned
implementation of RuntimeProviderBrokerBackendRegistryV1.

The service CLI accepts exactly one public input:

    --catalog ABSOLUTE_PATH

Do not add credential, private-key, token, plugin, test-provider, or socket
override arguments. Provider objects retain private material internally.

## Fixed paths and identity

| Property | Linux | macOS |
| --- | --- | --- |
| Service identity | iroha:iroha | iroha:iroha |
| Executable | /usr/local/libexec/iroha-runtime-provider-broker-v1 | /usr/local/libexec/iroha-runtime-provider-broker-v1 |
| Public catalog | /etc/iroha/runtime-provider-broker/catalog.norito | /private/etc/iroha/runtime-provider-broker/catalog.norito |
| Runtime directory | /run/iroha-runtime-provider-broker-v1, mode 0700 | /private/var/iroha/run, mode 0700 |
| Broker socket | /run/iroha-runtime-provider-broker-v1/runtime-provider-broker-v1.sock | /private/var/iroha/run/runtime-provider-broker-v1.sock |

The broker and every stock client must run with the same effective UID.
Supplementary-group access is not a substitute for the peer-credential check.
The broker creates the fixed socket with mode 0660. The macOS catalog path
names /private/etc explicitly because /etc is a symlink and the process shell
rejects symlink path components.

The broker holds a single-link, mode-0600 instance lock in its runtime
directory for its full serving lifetime. After an unclean exit releases that
lock, the next process removes only an exact inactive socket with the expected
owner, mode, and stable device/inode identity before rebinding. Every other
pre-existing entry fails startup without being removed.

Install the executable, catalog, supervisor asset, and Linux consumer drop-ins
as single-link, non-symlink regular files owned by root. They must have no
owner/group/other write bit and no set-user-ID, set-group-ID, or sticky bit. A
typical installation uses mode 0555 for the executable, root:iroha mode 0440
for the catalog, and mode 0444 for the supervisor assets. Their parent
directories must be root-owned and not group/world writable. The installation
gate additionally binds the executable to the SHA-256 obtained from the
externally verified signed release provenance; a correctly named arbitrary
program is never accepted.

## Linux systemd

Install systemd/iroha-runtime-provider-broker-v1.service as:

    /etc/systemd/system/iroha-runtime-provider-broker-v1.service

The unit creates its dedicated `/run/iroha-runtime-provider-broker-v1`
directory with the service UID and mode 0700, passes only the fixed public
catalog path, and gives the process no environment-based provider selector.
The broker is the sole unit that manages this directory; the validator's
separate `/run/iroha` directory has an independent lifetime. The broker
directory is recreated across broker restarts. The unit uses
Type=notify: the deployment-owned binary must connect its
RuntimeProviderBrokerExecutableV1 readiness callback to systemd READY=1 only
after exact backend qualification and fixed-socket publication. This makes the
consumer After ordering a readiness boundary instead of merely a process-spawn
boundary. Provider packages that need a specific device, read-only vendor
library, or durable sealed-store path must add a reviewed systemd drop-in; do
not weaken the base unit or place credentials in Environment or
EnvironmentFile entries. The base unit also sets `LimitCORE=0` so provider
process memory is not written to a core image.

Both checked-in Linux consumer dependencies are mandatory in an enabled
production package:

- Install the taira-irohad.service.d drop-in for the validator.
- Install the sorafs-governance-dag@.service.d drop-in beside each
  deployment-owned Governance DAG instance unit.

The repository does not currently ship a base Governance DAG systemd unit
because its instance config path and public chain identity are
deployment-specific. Both drop-ins use Requires and After, so an enabled
consumer cannot start successfully without the ready broker unit. They also
pin User and Group to iroha so the stock Unix peer-credential check sees the
same effective identity, and mount the broker runtime directory read-only in
the consumer namespace so the client cannot mutate socket pathnames. Packages
with runtime providers explicitly disabled omit both drop-ins and the fixed
catalog. The container validator unit is intentionally not covered: sharing
this host socket into a container requires a separately reviewed UID and mount
contract.

## macOS launchd

Install launchd/org.hyperledger.iroha.runtime-provider-broker-v1.plist as:

    /Library/LaunchDaemons/org.hyperledger.iroha.runtime-provider-broker-v1.plist

Before bootstrapping the LaunchDaemon, create the persistent
`/private/var/iroha/run` directory as iroha:iroha with mode 0700 and install the
executable and public catalog at the fixed paths above. The plist uses no
environment variables, disables core images with zero soft and hard `Core`
limits, and restarts only after unsuccessful exit.

No validator or Governance DAG LaunchDaemon is checked in, so there is no safe
consumer plist to mutate here. Deployment packaging must bootstrap the broker
job before its consumer jobs. The consumers still fail closed if the broker
has not qualified its complete catalog and published the fixed socket.

## Static installation gate

Use the read-only checker with the same exported catalog used to build the
package:

    python3 scripts/check_runtime_provider_broker_install.py \
      --platform linux \
      --install-root / \
      --expected-catalog /secure/staging/runtime-provider-catalog.norito \
      --expected-executable-sha256 "$VERIFIED_BROKER_SHA256" \
      --check-runtime-directory

For a package intentionally containing no runtime-provider bindings:

    python3 scripts/check_runtime_provider_broker_install.py \
      --platform linux \
      --runtime-providers-disabled

For a non-empty expected catalog the checker rejects a missing, linked,
non-executable, mutable, specially permissioned, inaccessible, wrongly owned,
oversized, concurrently changed, or release-digest-mismatched broker binary; a
missing, linked, mutable, specially permissioned, inaccessible, or
byte-different installed catalog; a missing, linked, mutable, specially
permissioned, tampered, or byte-different systemd/launchd asset; a missing or
non-exact Linux consumer drop-in; and, when requested, a runtime directory
whose owner or mode is not exact. The expected executable digest must come
from the externally authenticated release provenance, not from the installed
file. The macOS runtime directory is mandatory even without
`--check-runtime-directory` because launchd does not create it. Expected
supervisor and drop-in bytes are derived from the checked-in platform
templates and cannot be overridden on the command line. The broker process
remains responsible for canonical Norito decoding, backend-set resolution,
live provider qualification, and creation of the authenticated socket.

Windows has no V1 authenticated runtime-provider transport. A Windows package
must not present these templates as a functioning production broker.
