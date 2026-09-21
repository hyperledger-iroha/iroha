# Iroha wallet service

This crate owns private developer custody and durable native account operations.
Musubi and the ordinary Taira account CLI use these services; transaction construction,
fee quotes, plan verification and finality observation remain native SDK responsibilities.

- `WalletStore` creates or imports immutable named wallets outside project directories.
  Public inspection retains exact network/profile identity without reading signing keys.
  Signing uses native `Config` and its bounded, no-follow `private_key_file` reader.
- `AccountService` delegates authoritative balance reads and cumulative principal/fee
  solvency checks to native SDK `blocking::Client::{balance,check_funding}`, then prepares, submits or reconciles an
  exact fee-paying transfer or alias operation. Private journals retain the signed envelope
  before dispatch and prevent repeated submission after an attempted write.
- `OnboardingService` owns ordinary account admission and faucet recovery.
  Submission uses the native bounded Applied waiter and exact committed-wire readback;
  resume performs an immediate read-only observation.
  `faucet_pow` is the one bounded solver shared with the independent public-reset child.

No service prints or persists credentials in public reports. Filesystem custody requires
supported Unix descriptor and atomic no-replace operations; directories are owner-only
0700 and new regular single-link files are 0600; private reads also accept owner-read-only
0400 files. Tests use temporary directories and native
mock transports. They do not qualify public network rollout.

See the source-coupled [Musubi specification](../../specs/musubi.md) and
[workflow qualification ledger](../../specs/musubi_taira_workflow_goals.md).
