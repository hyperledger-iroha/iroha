# Dependency graph review — 2026-09-30

The previous numeric census described the graph at `59144e0770` (fingerprint `98ce3b5c…`). Its exact checked-in bytes are retained below. The initial repair candidate at `0993e1c83f` already exceeded that census. The current baseline records this explicitly reviewed candidate with no growth allowance; package deny rules, every feature-resolved architecture boundary, and exact manifest fingerprint enforcement remain unchanged.

This is a host-independent Cargo **manifest-source graph**: required normal/build edges, declared optional edges, and root development edges only where configured. It is not a measurement of compiled binaries, registry transitive dependencies, or network qualification.

| Scope | Required local packages: previous / initial / current | Required declaration edges: previous / initial / current |
| --- | --- | --- |
| `iroha-cli-shipping` | 53 / 65 / 66 | 676 / 753 / 758 |
| `iroha-data-model-shipping` | 23 / 26 / 25 | 206 / 216 / 213 |
| `iroha-model-base-shipping` | 14 / 17 / 16 | 110 / 115 / 112 |
| `iroha-sdk-shipping` | 30 / 33 / 32 | 283 / 283 / 280 |
| `iroha3d-shipping` | 53 / 65 / 66 | 697 / 807 / 813 |
| `workspace-all-targets` | 105 / 109 / 110 | 1594 / 1675 / 1684 |

The pre-existing increases mainly come from explicit Sumeragi, Core ZK, software custody, accelerator, panic-hook, SCCP wallet/RPC, compiler-surface and CLI/service owners. The daemon now reaches the general `iroha` client through both `iroha_musubi_service` and `iroha_storage_client`; those normal dependencies remain visible current costs. `num-bigint` is an owned local patch. The audit records every initial added/removed required owner and direct declaration change, including newly local edges that previously resolved externally.

The new `iroha_allocation` owner establishes a std-only custody boundary beneath runtime storage. Compared with the initial candidate it reduces required model, model-base and SDK declaration edges and removes runtime storage from their lower closures; it adds one explicit lower owner and direct custody declarations elsewhere. It does not establish a broader footprint reduction: workspace and daemon declaration counts grew, while the model reductions and removed old owners are recorded independently.

| Preserved record | SHA-256 |
| --- | --- |
| [dependency_budget.json.txt](dependency_budget.json.txt) | `5cce2831bb515a72c13efced889cd0893260f7397245f6c666e636e808d2aef7` |
| [owner-audit.json.txt](owner-audit.json.txt) | `8a6aca076eddd2673c4b823a0e5d3eadaaab52b78f4d63faf4d4aa152cd55b0b` |
| [current-source-graph.json.txt](current-source-graph.json.txt) | `5987fc6a25f345760124163ca4396294707373eec1441e2d9d70e83c4b39b9ea` |

Current manifest fingerprint: `sha256:35dfce8ccaf8076ce34e10ccbd1daba51c32c4a97792195dad07899a82c0e515`. All numeric limits equal the measured values exactly. This snapshot is separate from model/schema unit fixture capture and does not qualify release readiness.
