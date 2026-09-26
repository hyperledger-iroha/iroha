# F02 pending-penalty peer-key charge, 2026-09-24

Scope: the existing `optimizations` checkout. Parent penalty preparation now
counts exact compact public-key clone bytes through the same borrowed State
view used to fill the pending plan. It reserves those bytes from the original
State-owned evidence preparation pool after funding the fixed pending backing
and before constructing the stake index or cloning a pending `PeerId`. Each
key's tag-plus-payload layout receives a move-only `AllocationCharge` stored
with that key in the pending entry; the charge refunds after its `PeerId`
drops. A one-byte-short pool refuses with typed local capacity/release-wait,
leaves committed evidence untouched, and can retry from the same State after
the original reservation releases.

The config-owned pending-entry layout includes this charge. The one-plan
minimum now includes 124 maximum-size compact public keys in addition to the
pending and prune backings; the default still funds eight maximum plans.
`iroha_config` obtains the charge type from the existing `mv` crate. The
snapshot fixture reflects the resulting default. This is process-local
resource admission; consensus evidence and block wire layouts do not change.

The coordinated locked Core build passed the nested-key refusal and same-State
retry selector 1/1. The same-source cached Core binary passed the adjacent
pending-backing refusal, parent-plan, and canonical `offender_peer` selectors
3/3. Locked Config selectors for pool minimum/default and the minimal snapshot
passed 2/2. `cargo fmt --all -- --check` passed. These focused results cover
the new owner boundary; they do not qualify the complete penalty path.

This slice does not fund the stake-index B-trees and share vectors, validator
locator maps and their cloned keys, adaptive `Quantity` values, scratch
`StateBlock`, emitted actions or all publication overlap. Those remain F02
release blockers. No full workspace, multi-validator, or release qualification
is claimed from this source change.
