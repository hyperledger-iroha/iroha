# SoraFS initialization on the existing Taira chain

Taira has no initialized SoraFS admission council and no holder of the required
SoraFS authority roots. A ledger reset is prohibited because BPNG accounts,
balances, wallets, controllers, contracts and history must be preserved.

The earlier proposal to append an action to the selected threshold-key lifecycle
was rejected: encoding compatibility does not permit changing a selected immutable
definition. It has been removed from code and qualification tests.

On 2026-11-07 the network owner explicitly authorized one repair signed by all four
validators. [`taira_repair_v1.md`](taira_repair_v1.md) specifies its implementation:
a new typed instruction, separately named kernel and exact successor selection,
with SoraFS initialization and activation in the same transaction overlay. Existing
definitions, bindings and storage remain unchanged. The exception grants no general
Parliament, executor-upgrade, runtime-proposal or role-management authority.

Authorization is not evidence of successful deployment. Retained native replay,
BPNG preservation comparisons, compatible mobile releases and actual proof
verification across the repair remain required before reporting completion.
