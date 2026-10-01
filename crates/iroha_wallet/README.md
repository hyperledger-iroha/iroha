# Iroha wallet service

This crate owns private developer custody and durable native account operations.
Musubi and the ordinary Taira account CLI use these services; transaction construction,
fee quotes, plan verification and finality observation remain native SDK responsibilities.

- `WalletStore` creates or imports immutable named wallets outside project directories.
  Public inspection retains exact network/profile identity without reading signing keys.
  Signing uses native `Config` and its bounded, no-follow `private_key_file` reader.
  Typed key-pair import binds an existing owner to a separately authenticated network
  without copying endpoint credentials from another client configuration.
- `AccountService` delegates authoritative balance reads and cumulative principal/fee
  solvency checks to native SDK `blocking::Client::{balance,check_funding}` with exact
  `AssetId` principal buckets. XOR transfers and paid parent alias totals explicitly
  select global balances; fee quotes select each registered asset's policy and exact
  route dataspace. Balance reports retain the full scoped asset identity. The service
  then prepares, submits or reconciles an
  exact fee-paying transfer or alias operation. Private journals retain the signed envelope
  before dispatch and prevent repeated submission after an attempted write.
  `BoundedTransactionOptions` caps aggregate execution fees and shares one deadline across
  paid alias preparation, parent registration and anchoring. Bounded alias recovery requires
  the original alias request, rent guards and fee authorization; it never signs another wire.
  Typed private-root registration and anchor requests bind the exact child, retained parent
  cursor, caller fee maxima and one shared deadline. Their submit/resume methods compare that
  selected request while holding the journal lock. An `Applied` transport result does not
  replace the attachment service's independently verified parent inclusion proof.
- `namespace` derives paid one-year domain requests and canonical private dataspace
  leases. Private dataspace names use SNS-derived ids and reject physical parent
  catalog collisions; preparing a lease never adds a parent lane or bootstrap grant.
- `OnboardingService` owns ordinary account admission and faucet recovery.
  Submission uses the native bounded Applied waiter and exact committed-wire readback;
  resume performs an immediate read-only observation.
  `faucet_pow` is the one bounded solver shared with the independent public-reset child.

No service prints or persists credentials in public reports. Filesystem custody uses
the shared `iroha_fs` native handles, immutable publication and file locks on Windows,
macOS and Linux. Unix directories are owner-only 0700 and new single-link files are
0600; private reads also accept 0400. Windows uses protected owner ACLs and retained
native file identities. Tests use temporary directories and mock transports;
cross-compilation alone does not qualify native Windows execution or public rollout.

See the source-coupled [Musubi specification](../../specs/musubi.md) and
[workflow qualification ledger](../../specs/musubi_taira_workflow_goals.md).
