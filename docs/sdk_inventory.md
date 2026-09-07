# SDK operation inventory

[`specs/sdk_operation_inventory.tsv`](../specs/sdk_operation_inventory.tsv)
records the complete explicit Torii route catalog, sorted by route identity.
It includes HTTP method/path, audience, authentication, principal admission,
effect, transport, feature expression, projection flags, and private-cache policy.
The source authority is `iroha_torii_shared::route_catalog::CATALOGED_ROUTES`;
Torii's router builder checks each mounted descriptor against this catalog.
Implicit CORS preflight behavior is excluded from the operation inventory.

Run `python3 scripts/sdk_operation_inventory.py` for a read-only drift check,
or `python3 scripts/sdk_operation_inventory.py --write` after reviewing route
changes. The generator compiles the actual `std`-only Rust descriptors with the
repository toolchain. It needs no Cargo dependencies or node binaries. The
generated-file registry records its owner and complete source inputs.

The first-release client migration must account for every row and every
existing SDK call. The `sdk` column describes current generated projection
membership, not which handwritten SDK operations may be removed. For example,
status, configuration reads, and block streams are currently outside that
projection while the Rust SDK implements them. Preserve these capabilities
on their appropriate public, account, or operator contexts. The feature
expression describes availability and does not authorize enabling an
unqualified service.

TODO: Complete the canonical capability/request/response mapping and consumer
migration against this inventory, then check that each supported operation has
one authority-appropriate API and that no stale public wrappers remain. This
inventory alone does not establish SDK coverage or qualify node features.


## Subscription capability coverage

All eleven `application.subscriptions*` route identities have one asynchronous
Rust SDK operation. The flat synchronous methods have been removed.

| Context | Capability operations | Routes |
| --- | --- | --- |
| `Client::subscriptions()` | `list_plans`, `list`, `get` | GET plans, subscriptions, and subscription by ID |
| `AccountClient::subscriptions()` | `prepare_plan`, `prepare` | POST plans and subscriptions |
| `AccountClient::subscriptions()` | `prepare_pause`, `prepare_resume`, `prepare_cancel`, `prepare_keep`, `prepare_charge`, `prepare_usage` | POST subscription action and usage routes |

Wire records are owned by `iroha_torii_shared::subscriptions`. SDK preparation
results expose one typed `SubscriptionDraft`, including context identity,
operation metadata and decoded `TransactionPayload` or `InstructionBox` values.
The explicit blocking capabilities run these same async operations on an owned
runtime. The CLI consumes those facades and prints unsigned preparation results.
The subscription contract suites cover every route, exact signed requests,
count-mode forwarding, authority/resource binding and asynchronous dispatch.
Qualification evidence is recorded separately; this mapping is implementation
coverage, not a claim that the complete SDK migration is qualified.
