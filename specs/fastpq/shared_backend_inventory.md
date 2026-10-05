# FASTPQ shared-backend inventory

Updated: 2026-10-05, for task B.1 of the [ZK delivery plan](../zk_delivery_plan.md).
The facts live in [`shared_backend_inventory.json`](shared_backend_inventory.json);
this page explains how to read and maintain it. The inventory records where code
is today. It does not claim that any owner is qualified, secure or migrated.

## What it lists

- **Engines**: every owner of FASTPQ-style field, AIR, FRI, transcript or
  commitment code, with its files, its construction and every importer.
- **Consumers**: the production roles of the plan, the engine each one uses, its
  entry points and the task that migrates it. A `facade` block lists every
  caller of a role's entry points.
- **Roles without an engine**: plan roles that have no FASTPQ-style consumer
  today, with the paths that were checked. RAM-LFE is the only one.
- **Distinct families**: Halo2/IPA, Bulletproofs, the Jindo lattice PCS and
  Spartan/Nova. They keep their own mathematics and are not migration targets.
- **Retained utilities**: public IO, limits, CLI trace builders, observers,
  accelerators and arithmetic helpers of the superseded `fastpq_prover` driver
  files that still have users and must move before a driver file is deleted.
- **Engine utilities**: the limits and observers owned by the other duplicate
  engines.
- **Sealed relations**: every type that implements the q77 engine's sealed
  relation marker, with the role `air::q77::SEALED_RELATIONS` publishes for it.
- **Goldilocks modulus definitions** and **crate dependents** of `fastpq_prover`
  and `fastpq_isi`.

## Engines

| Engine | Class | Owner | Used by |
| --- | --- | --- | --- |
| `fastpq-q77` | canonical | `crates/fastpq_prover/` | ordinary effects, AXT; X509 uses its transform accelerators |
| `fastpq-isi-parameters` | canonical support | `crates/fastpq_isi/` | parameters and hashes for the canonical engine and several duplicates |
| `fastpq-legacy-replay` | duplicate | `fastpq_prover` `src/proof.rs`, gated items of `src/backend.rs` and `src/trace.rs`, four `backend/` modules | tests, fuzz targets and `dev-tools` binaries only |
| `privacy-execution-fork` | duplicate | `iroha_core_privacy` `execution_proofs/stark/` | RaceV1, classed RaceV1, all IVM step chips |
| `privacy-substrate` | duplicate | `iroha_core_privacy` `privacy_engines/{transparent,aggregate,proof_managed_note}_stark*`, `privacy_outer_hash.rs` | IVM private notes, PQ-MASP, atomic settlement, ZK-ACE transforms, X509 |
| `zk-ace-stark` | duplicate | `privacy_engines/zk_ace_stark.rs` | ZK-ACE |
| `zk-x509-driver` | duplicate | `privacy_engines/zk_x509/stark.rs`, `stark/`, `composition_masking.rs`, `fixed_algebraic.rs` | X509 MAIN/CA |
| `core-zk-native-stark` | duplicate | `iroha_core_zk` `src/stark.rs` | generic semantic verification, SoraCloud VK/BFV checks |
| `soracloud-bfv-native-stark` | duplicate | `iroha_crypto` `src/fhe_bfv.rs` (the STARK material only) | SoraCloud VK/BFV checks |
| `zk-ams-rns-qpcs` | conditional | `iroha_zkp_halo2` `vega/zk_ams/mkhe/rns_native_qpcs_*`, transcript and proof hash | ZK-AMS qPCS |

`soracloud-bfv-native-stark` is the SoraCloud BFV proof material: its own
Goldilocks prime and arithmetic, the BFV arithmetic AIR constraint-system and
composition material, the native STARK/FRI profile constants and the key payload
types. It has no FRI of its own; `core-zk-native-stark` verifies against it. The
BFV scheme in the same file is FHE and is mapped by
[`fhe_ownership_inventory.json`](../fhe_ownership_inventory.json).

`zk-ams-rns-qpcs` runs FRI over the RNS-native field, not Goldilocks. It
migrates only if C.5 selects the FASTPQ backend as its PCS.

RAM-LFE has no engine row. Core refuses RAM-LFE proof mode
(`PROOF_RELATION_UNAVAILABLE`), `iroha_crypto`'s RAM-LFE code contains no STARK
or FRI, and the three `iroha_core_zk` `ram_lfe_*` modules are test-only Halo2
experiments. R.11, R.2, R.3, R.8, R.12, R.7, R.9 and R.10 add the consumers.

## Canonical replacement path

`fastpq_prover::air` is the interface a relation is written against:
`SemanticAir`, the sealed `PolynomialField`, public IO, `WorkLimits` and
`Observer`. The q77 engine already takes its limits only as
`air::q77::VerifierLimits` and `air::q77::ProducerLimits`, each carrying one
`WorkLimits`. B.2 generalizes the engine behind the interface, and B.3 moves
each consumer in the table to it with that consumer's own capability. A
duplicate is deleted only after its listed importers have moved. Until B.2
lands, relations other than the sealed q77 ones have only the uncommitted
reference (`build_reference`/`check_reference`), which is not a proof.

## Check

```
cargo test -p fastpq_prover --test shared_backend_inventory
```

The test re-derives every importer set from the source with the rules in the
`scan` and `detect` fields. It fails when:

- a listed path is gone, or an entry-point or utility symbol no longer stands
  as a whole token outside comments;
- a file starts or stops importing an engine, a consumer facade or a utility,
  or its role (source, test, bench, fuzz, cli) changes. This holds file by file
  inside a listed `tree` as well;
- a `const` or `static` equal to the Goldilocks modulus is defined in an
  unlisted file, in any spelling and across line breaks;
- a source file or directory named `*stark*` or `*fri*` is not covered;
- an IVM step chip module is added or removed at any depth below
  `ivm_step_air.rs`;
- a type starts or stops implementing the q77 sealed relation marker, or the
  listed roles differ from `air::q77::SEALED_RELATIONS`;
- a path recorded under a role without an engine starts importing a proof engine;
- a crate starts or stops depending on `fastpq_prover` or `fastpq_isi`, in key,
  table or renamed-package form;
- a required plan role or distinct family is missing, a distinct family is
  listed as a FASTPQ-style owner, or a duplicate owner has neither a consumer
  nor a note.

Update the JSON in the same change that adds, moves or removes an importer; the
failure message names each entry to add or remove. A `tree` (the X509 relation,
the IVM step chips, the Core FASTPQ lane and the ZK-AMS proof) is one migration
unit, and its `files` list still names every importing file below it.

## Limits of the scan

Matching is textual: line comments are removed, a needle must stand at
identifier boundaries and a fragment (at least 12 identifier characters) must
lie inside an identifier, whatever verb or suffix surrounds it. A use hidden
behind a macro, a re-export under another name or a block comment is not seen.
Outside its own tree the zk-X509 driver is matched by import forms only (a path
use, a module declaration, a use-list member, a rename, or a file path with a
leading directory), so a string that merely names `zk_x509`, such as a
provenance label, is not an importer; a `#[path]` or `include!` whose string
starts with `zk_x509/` would not be seen. The
modulus check evaluates integer constant expressions built from literals, the
unsigned `MAX` constants, casts, `from`, shifts and arithmetic; a value computed
by a function is found only when the constant's name claims the modulus. Only
Rust sources are scanned. A hand search on 2026-10-05 found the modulus outside
Rust in the Metal and CUDA kernels of `crates/fastpq_prover`, in the Python
reference and evidence scripts under `scripts/`, and in Digest384 lane
canonicity checks of the Python and JavaScript SDKs; none is a second engine.
Treat a pass as "the inventory agrees with what the scan can see", not as proof
that no other copy exists.
