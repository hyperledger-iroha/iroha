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
