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

The initial audited current manifest fingerprint was `sha256:35dfce8ccaf8076ce34e10ccbd1daba51c32c4a97792195dad07899a82c0e515`. The records above retain that observation exactly.

A subsequent current-source review adds one root development declaration in `iroha_config`: `iroha_data_model` with `default-features = false` and the `bls` feature. Configuration already depends on the model normally; this test-only feature enables the canonical curve-3 identity used by the checked-in Taira validator templates. No shipping scope metric changes. Workspace required and declared declaration edges become exactly 1685 and 1849, respectively; all package and external-dependency counts remain unchanged. This reviewed test-owner cost is explicit and receives no headroom.

| Preserved config test-owner record | SHA-256 |
| --- | --- |
| [dependency_budget-before-config-test-owner.json.txt](dependency_budget-before-config-test-owner.json.txt) | `c9d2157b22f11bb021ce0f3e25218054d7157855bda8a52a9bac7dba0b501e42` |
| [config-bls-test-owner-source-graph.json.txt](config-bls-test-owner-source-graph.json.txt) | `3dbb148be5fc6ee481a9e55a1e32f16205dd597399ccee94087434b89417efb4` |

The config test-owner manifest fingerprint was `sha256:68a98e9a185060b7c052e51241b484b020a57e18da855132782e16acd7b59c86`.

An actual macOS native build then found that stripping a host proc-macro dynamic library can make its Mach-O image unloadable. The shared `dev.build-override` and `test.build-override` now set `strip = "none"`, matching the existing release host-library policy. Their debug/codegen settings and every dependency selector are unchanged. This changes only the raw root-manifest fingerprint in the budget: a structural comparison of the previous and current budget confirms that all other fields, including every measured cost and configured limit, are identical.

| Preserved host-profile record | SHA-256 |
| --- | --- |
| [dependency_budget-before-host-profile-repair.json.txt](dependency_budget-before-host-profile-repair.json.txt) | `e37af9a3be8564106bd957cda2eb88cb65df641e1ab1474b85b6f2a34f75460c` |
| [host-profile-source-graph.json.txt](host-profile-source-graph.json.txt) | `c2a34f1c27512acc7f2bd16e0986703840b288701194dadf8c88b620b6b3f52d` |

The build-override repair alone produced manifest fingerprint `sha256:52737e5a8243445e3e4b620a2efd8f0622aaa4533b8900a99104812034375c4b`. An actual Cargo unit graph then showed that the existing `dev.package."*"` and `test.package."*"` overrides take precedence for external host macros. Both now also specify `strip = "none"`; the measured unit graph resolves all 41 host proc-macro libraries without stripping. Another structural comparison confirms that the budget again changes only its raw manifest fingerprint, with no cost, selector or limit changes.

| Preserved effective host-profile record | SHA-256 |
| --- | --- |
| [dependency_budget-before-effective-host-profile-repair.json.txt](dependency_budget-before-effective-host-profile-repair.json.txt) | `72a566cd38c0312ffb2af913a4733e2f143907b01c6efccf4da37467cb955043` |
| [effective-host-profile-source-graph.json.txt](effective-host-profile-source-graph.json.txt) | `f38da226e3b3fdca692f43b3ff68d4844a81a73de052e7cbe78e2f7d7c6c845b` |

Current manifest fingerprint: `sha256:50c20cafba54d040df41318a191d52a35e53a20b5967f6f8baeb85ee28ea9e48`. All numeric limits equal the measured values exactly. Package denies, architecture boundaries, and no-growth enforcement remain unchanged. This snapshot is separate from model/schema unit fixture capture and does not qualify release readiness.
