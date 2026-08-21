# SoraFS external software signer V1

This package runs one isolated signing role per Unix service identity. It is
software-key-qualified: it does not emit or imply an HSM attestation. The
public provider boundary remains compatible with a later HSM implementation.

Private keys exist only inside a mode-0700 state directory as a
ChaCha20-Poly1305 envelope and, while serving, as runtime memory. The 32-byte
wrapping key must arrive through a consumed inherited descriptor or a systemd
encrypted credential. Do not place either key in argv, environment variables,
TOML, the public binding, logs, or ledger state.

## Identities and filesystem layout

Provision a distinct Unix account for each signer role. The canonical public
binding fixes three different UIDs:

- `service_uid` owns the process, state, runtime directory, and sockets;
- `client_uid` is the only peer admitted on `request.sock`;
- `administrator_uid` is the only peer admitted on `administrator.sock`.

The runtime directory is exactly mode 0711. Socket entries are mode 0666 so
the two distinct peer UIDs can connect; authorization is the kernel-reported
peer UID, and the service rejects every other UID before decoding a request.
Only the service UID can mutate the runtime directory. The state directory is
exactly mode 0700; envelope and audit records are mode 0600 and single-link.

The supplied systemd template uses:

- `/var/lib/sorafs-signers/%i` for encrypted state;
- `/run/sorafs-signers/%i/{request,administrator}.sock` for local transport;
- `/etc/sorafs/signers/%i.binding.norito` for the non-secret reviewed binding;
- the systemd encrypted credential named `wrapping-key` for runtime decryption.

The macOS package supplies four native-role and seven opt-in typed-role
LaunchDaemons plus the fixed
`sorafs-external-software-signer-launchd-v1` launcher. Before loading a job,
an independent administrator creates its named pipe beneath the
root-owned, mode-0700 `/private/var/run/iroha-signer-credentials` directory and
writes exactly 32 wrapping-key bytes once per start. launchd opens that pipe as
standard input; the launcher only duplicates the already-open descriptor to fd
3. Neither component resolves the credential path or carries secret bytes in
argv, configuration, or the environment. Install the launcher mode 0555, the
plists mode 0444, role state directories mode 0700, and reviewed public
bindings as root-owned single-link files under `/private/etc/sorafs/signers`.

Install only the role belonging to that service host. The exact V1 instance,
binding basename, algorithm, and administration boundary are:

| Instance | Canonical handle | Algorithm | Installation boundary |
| --- | --- | --- | --- |
| `proof-outcome` | `software://sorafs/proof-outcome/primary` | Ed25519 or ML-DSA-65 | Taira validator runtime |
| `repair` | `software://sorafs/repair/primary` | Ed25519 or ML-DSA-65 | Taira validator runtime |
| `reserve` | `software://sorafs/reserve/primary` | Ed25519 or ML-DSA-65 | Taira validator runtime |
| `orderbook` | `software://sorafs/orderbook/primary` | Ed25519 or ML-DSA-65 | Taira validator runtime |
| `governance-dag` | `software://sorafs/governance-dag/primary` | Ed25519 | Governance DAG publisher host |
| `potr-gateway` | `software://sorafs/potr-gateway/primary` | Ed25519 | PoTR gateway host |
| `potr-provider` | `software://sorafs/potr-provider/primary` | ML-DSA-65 | independently administered PoTR provider host |
| `billing` | `software://sorafs/billing/primary` | Ed25519 | billing statement publisher host |
| `evidence-viewer` | `software://sorafs/evidence-viewer/primary` | Ed25519 | evidence-viewer host |
| `stream-token` | `software://sorafs/stream-token/primary` | Ed25519 | stream-token issuer host |
| `pop-credentials` | `software://sorafs/pop-credentials/primary` | Ed25519 | PoP enrollment issuer host |

The binding at `<instance>.binding.norito` fixes the service, client, and
independent administrator UIDs plus the reviewed purpose identity. Governance
fixes `publisher_peer_id`; PoTR fixes `signer_id` and, for the provider role,
`provider_id`; billing fixes `signer_id`; and PoP fixes `issuer_id`. A service
host must use a distinct role account and administrator identity and must not
co-host independently administered PoTR gateway/provider or PoP issuer roles.

On Linux, install the template once and explicitly enable only the approved
instances on the corresponding service host, for example
`systemctl enable --now sorafs-external-software-signer@governance-dag.service`.
On macOS, install and bootstrap only the matching checked plist. Merely shipping
these opt-in assets does not activate them. On a Taira validator, explicitly
enable only `proof-outcome`, `repair`, `reserve`, and `orderbook`;
none of the seven typed roles is auto-launched on a validator.
Promotion uses the same supported binary and receipt protocol, but must run on a
separately administered L2 promotion host with its own inherited credential
descriptor; it has no
systemd instance or LaunchDaemon in this package and is never coupled to the
validator or runtime-broker jobs.

## Provisioning

Run provisioning as the final service UID and pass the wrapping key on a
dedicated inherited descriptor. The descriptor number is public; its bytes do
not appear in the command line. A representative invocation is:

```text
sorafs_external_software_signer provision \
  --state-directory /var/lib/sorafs-signers/promotion \
  --binding-out /var/lib/sorafs-signers/promotion/binding-review-staging.norito \
  --handle software://sorafs/promotion/primary \
  --service-id promotion-signer-primary \
  --administrator-id release-security-primary \
  --service-uid 4101 --client-uid 4102 --administrator-uid 4103 \
  --role promotion --algorithm ed25519 \
  --key-revision 1 --policy-revision 1 \
  --policy-digest-sha256 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --max-request-bytes 33554432 --wrapping-key-fd 3
```

Use the SHA-256 digest of the exact reviewed policy bytes; the example value is
only syntax. A promotion binding always rejects ML-DSA. Native proof-outcome,
repair, reserve/rent, and orderbook roles admit Ed25519 or ML-DSA-65.
Governance DAG, PoTR gateway, billing, evidence viewer, stream-token, and PoP
credentials require Ed25519; PoTR provider requires ML-DSA-65. Each typed
binding also requires its role-specific provisioning argument described above;
an omitted, extra, or cross-role identity is rejected.

Provisioning runs as the final service UID, so it must write the initial public
binding only to service-owned review staging. An independent administrator
must validate that artifact and install it without replacement as a root-owned,
single-link file at `/etc/sorafs/signers/promotion.binding.norito` (or the
corresponding native-role path). The signer service must never own or retain
write access to the reviewed binding directory or installed binding.

## Signing and evidence

`sign` consumes an absolute regular payload path, a stable non-zero
`operation-id`, and a reviewed binding. It creates two new files without
replacement: a raw detached signature and a canonical JSON public receipt.
All CLI artifact reads and writes traverse from an opened root directory with
`openat`/`O_NOFOLLOW`, pin every ancestor and leaf by device/inode, require
secure ownership, modes, and single-link files, and revalidate the complete
chain after I/O. Outputs are staged, fsynced, content-checked, and published by
no-replace `renameat`; ancestor, leaf, hard-link, and path-swap races fail
closed.
Promotion signs the exact bytes beginning with
`iroha:sorafs:production-readiness:foundational-prerequisites:v1\0`; native
roles decode the canonical transaction payload, verify its authority, and sign
the Iroha transaction prehash. Each native service accepts exactly one
role-specific instruction: proof outcome submission; repair task/action/appeal;
reserve/rent movement, lifecycle, credit, or appeal; or orderbook match,
maintenance, or settlement receipt. Empty, multi-instruction, opaque, and
cross-role payloads fail closed at the signer boundary.

The receipt binds:

- `backend=Software`, service and independent administrator identities;
- isolated role/domain and Ed25519 or ML-DSA algorithm;
- key revision, policy revision/digest, public key and public-key digest;
- request and exact payload digests, raw signature, immutable commit sequence;
- audit genesis and committed/live hash-chain heads;
- a live active-key provenance attestation and a response attestation.

The receipt schema is
`sorafs.external_software_signer.signature_receipt.v1`. Hex is lowercase;
`binding.policy_digest_sha256` is the reviewed policy SHA-256, while fields
named `*_blake3_hex` use the stated BLAKE3 contract. The raw signature output
exists only as a compatibility handoff and must not be accepted without its
receipt.

Promotion finalizers must use the offline verifier rather than attempting to
reimplement the response/provenance BLAKE3 contracts in Python:

```text
sorafs_external_software_signer verify-receipt \
  --binding /etc/sorafs/signers/promotion.binding.norito \
  --payload /var/lib/sorafs-release/foundational.signing-payload.bin \
  --signature /var/lib/sorafs-release/promotion.signature.raw \
  --receipt /var/lib/sorafs-release/promotion.signature-receipt.json \
  --expected-operation-id 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --validation-out /var/lib/sorafs-release/promotion.receipt-validation.json
```

Exit zero means the new payload-free validation artifact was fsynced with
schema
`sorafs.external_software_signer.signature_receipt_validation.v1` and
`status=valid`. Validation pins the complete reviewed binding, exact payload,
detached signature, operation ID, request/response digests, live non-revoked
audit head, provenance attestation, and response attestation. The committed
signature sequence/head must equal the live sequence/head. Exit one means the
receipt failed closed; exit two is command-line usage failure.

## Standard irohad wiring

Install a byte-identical, single-link copy of
`sorafs_external_software_signer` as the standard deployment executable
`/usr/local/libexec/iroha-runtime-provider-broker-v1`. The executable selects
broker mode only from that exact basename and then exposes the repository's
catalog-only CLI:

```text
/usr/local/libexec/iroha-runtime-provider-broker-v1 \
  --catalog /etc/iroha/runtime-provider-broker/catalog.norito
```

There are no binding, socket, credential, or provider-selector overrides.
Linux uses fixed binding files under `/etc/sorafs/signers` and fixed sockets
under `/run/sorafs-signers`; macOS uses `/private/etc/sorafs/signers` and
`/private/var/iroha/sorafs-signers`. The broker loads the exact public runtime
provider catalog, pins each external endpoint to its reviewed binding, performs
two adjacent live qualification probes, constructs the existing native or
typed runtime signer adapter, and injects it through
`ExternalSoftwareSignerBackendsV1`'s implementation of
`RuntimeProviderBrokerBackendRegistryV1`. Registry resolution requires the
catalog's exact handle, role, purpose identity, public key, key/policy revision,
and policy digest, and rejects missing, extra, duplicated, or non-phase-one
signer entries. The
`RuntimeProviderBrokerExecutableV1` launcher then repeats exact binding
qualification before publishing its fixed endpoint. Standard `irohad` resolves
configured signer handles through its stock broker client; no private key or
signer credential enters the daemon.

The stock basename-selected executable intentionally accepts signer-only
catalogs. A deployment with non-signer provider slots must inject its complete
deployment registry as `ExternalSoftwareSignerBackendsV1`'s base registry; the
exact partition rejects missing base providers and rejects a base registry that
tries to supply any software-signer slot. PoP is stricter: its configured slot
is the complete credential-provider registry, so the signer service cannot
stand alone in that slot. A PoP issuer deployment must manually launch the
`pop-credentials` service on the enrollment host and wrap the already-qualified
complete registry with `ExternalSoftwareSignerPopRegistryV1`, which replaces
only `issuer_signer` after exact handle/key/policy/purpose checks. The stock
signer-only broker rejects a PoP registry catalog rather than silently dropping
enrollment, wallet, wrapping, or persistence providers.

Reuse the checked assets in `configs/sorafs/runtime_provider_broker`; do not
install a competing broker unit. Install this package's
`systemd/iroha-runtime-provider-broker-v1.service.d/20-external-software-signers.conf`
drop-in to bind the existing unit to the exact four signer instances. The
broker must run as the same Unix UID as irohad, and that UID must equal every
signer binding's `client_uid`. A missing service, wrong socket owner,
substituted binding, stale/revoked policy, duplicate role, catalog mismatch, or
capability marked as test causes startup to fail before the broker endpoint is
ready. Under systemd, readiness is emitted only after signal handlers are
installed, both live qualification rounds pass, and the authenticated broker
endpoint is bound. The existing macOS LaunchDaemon invokes the same basename
and catalog-only surface and publishes no readiness claim until synchronous
startup qualification has completed.

The journal never stores signing payloads. It fsyncs a canonical hash-chained
record before reporting success, rejects idempotency-key equivocation, and
requires predecessor-bound monotonic rotation. Revocation is terminal. Startup
rejects corrupt records, stale envelopes, incomplete substitution, wrong AEAD
key/AAD, insecure permissions, hard links, and symlinks.

An exact native-operation replay may return its original commit sequence and
head together with a later live audit head; both positions are bound by the
active response attestation. A fresh signature always commits at the live head.
Promotion receipts are stricter: their commit sequence and head must equal the
live provenance sequence and head before promotion tooling accepts them.

Rotation writes a new binding rather than replacing the reviewed predecessor.
Promotion and qualification tooling should consume that successor only after
independent review. A future HSM migration requires a new backend deployment,
new qualification evidence, and new promotion signatures; these artifacts
must continue to be described as software-key-qualified.

## Windows exclusion

Windows has no V1 authenticated Unix peer transport. Release builders include
`WINDOWS-UNSUPPORTED.md` in Windows packages and must exclude the signer
binary, broker alias, install assets, and signer smoke claim. Windows support
requires a new reviewed transport and new qualification evidence.
