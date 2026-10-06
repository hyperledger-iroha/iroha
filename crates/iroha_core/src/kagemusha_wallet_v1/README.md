# KAGEMUSHA ledger implementation

`KagemushaWalletLedgerV1` is the closed instruction boundary. Registration requires an
exact `CanManageKagemushaWallet { asset_definition }` grant and the reserve account's
own transaction authority. Registration permanently binds the network, scheme, asset
incarnation, scale, reserve account and balance scope. Exact retry does not increment
permanent reference counts. Ordinary transfer, burn and account/asset teardown cannot
spend or remove the reserve. Current and rollback snapshot validation checks the reserve
indexes and each retained issuance's registration, wallet and ordinal bindings.

`IssueLoad` is an ordinary signed transaction instruction. The payer commits the exact
asset, wallet, request identity, expected next ordinal, amount and optional charge. The
ledger checks the ordinal before atomically debiting the payer, crediting the reserve and
retaining the immutable load receipt. A stale ordinal or changed retry cannot debit funds.
Only a deterministic retry of the original transaction at its original height can return
the same receipt. A later transaction reusing the request fails; clients recover the original
receipt through a committed query, including after loads close. Load requires the original
global root scope and rejects independent private roots before debit; a dataspace lane of
the global chain remains eligible. Genesis cannot issue loads. Dedicated issuer keys, voucher
publication and a second publication transaction do not exist.

Committed recovery reads borrow one generation-consistent `StateView` under finite
record and cumulative decode limits. `CommittedLoadReceipts` checks the payer, registered
network/scheme/asset, wallet ordinal and original transaction hash/height membership.
Historical certificate availability does not limit receipt recovery after chain growth.
The returned receipt is transport data; it grants no finality or offline balance authority.
Clients independently authenticate native global consensus and the exact successful direct
transaction with `verify_finalized_kagemusha_wallet_load_v1`, and must verify the complete
Load proof before crediting value. A caller-supplied receipt or height cannot replace that
verification. The receipt retains request, scheme, asset, wallet, ordinal, value, charge,
payer and original transaction/height; later chain growth never replaces it.

Creating that receipt also emits the typed `KagemushaLoadCommittedV1` event with its
canonical transcript digest. The ordinary execution result commits to the exact event
bytes, order and count. An exact execution retry emits no second event, and a failed
enclosing transaction rolls the event back with its debit and receipt. The independent
event-inclusion verifier requires native global finality and the complete expected
receipt. Event naming or an unsigned receipt alone cannot authorize offline value.

Load deposits and Unload/fee payouts use the existing atomic numeric movement owner,
including transfer controls, source custody, exact balances, admission and execution
transcripts. Both balance buckets must equal the registered scope. Online asset controls
can delay payouts but cannot rewrite a retained claim or impose a haircut. An undelivered
load remains a reserve liability; no timeout refunds it.

The native package verifier is mandatory. `InstallVerifierPack` requires the registered
reserve account's authority and live exact asset-management permission. It authenticates
the signed sigma/Omega inventory against the registered scheme and selected manifest,
then retains the exact original bytes in one immutable World row. Proof-consuming
instructions mount the same-overlay installation and fully verify sigma, required Omega
and both transported accumulator claims. Missing originals retain `ArtifactsUnavailable`.
Model validation and finalized load receipts never replace native proof verification.

One package reserves one confidential operation and one sigma verification plus an Omega
verification when lineage is carried. Aggregate transport bytes include both accumulator
originals. All transaction/block proof limits are checked together before reservation
counters change or native verification starts. Fixture verifiers remain test-only
orchestration.

TODO(G3/G6): implement and qualify the compact offline Load relation consuming ordinary consensus evidence,
complete producer artifacts and the end-to-end device/network flow. The online receipt
alone grants no foreign wallet-open or proof-acceptance capability.
