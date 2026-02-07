---
lang: kk
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
---

# Lookup Grand-Product Example

This example expands the FASTPQ permission lookup argument mentioned in
`fastpq_plan.md`.  In the Stage 2 pipeline the prover evaluates the selector
(`s_perm`) and witness (`perm_hash`) columns on the low-degree extension (LDE)
domain, updates a running grand product `Z_i`, and finally commits the entire
sequence with Poseidon.  The hashed accumulator is appended to the transcript
under the `fastpq:v1:lookup:product` domain, while the final `Z_i` still matches
the committed permission table product `T`.

We consider a tiny batch with the following selector values:

| row | `s_perm` | `perm_hash`                                   |
| --- | -------- | ---------------------------------------------- |
| 0   | 1        | `0x019a...` (grant role = auditor, perm = transfer_asset) |
| 1   | 0        | `0xabcd...` (no permission change)                |
| 2   | 1        | `0x42ff...` (revoke role = auditor, perm = burn_asset) |

Let `gamma = 0xdead...` be the Fiat-Shamir lookup challenge derived from the
transcript.  The prover initialises `Z_0 = 1` and folds each row:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

Rows where `s_perm = 0` do not alter the accumulator.  After processing the
trace, the prover Poseidon-hashes the sequence `[Z_1, Z_2, ...]` for the transcript
yet also publishes `Z_final = Z_3` (the final running product) to match the table
boundary condition.

On the table side, the committed permission Merkle tree encodes the deterministic
set of active permissions for the slot.  The verifier (or the prover during
witness generation) computes

```
T = product over entries: (entry.hash + gamma)
```

The protocol enforces the boundary constraint `Z_final / T = 1`.  If the trace
introduced a permission that is not present in the table (or omitted one that
is), the grand product ratio diverges from 1 and the verifier rejects.  Because
both sides multiply by `(value + gamma)` inside the Goldilocks field, the ratio
remains stable across CPU/GPU backends.

To serialise the example as Norito JSON for fixtures, record the tuple of
`perm_hash`, selector, and accumulator after each row, for example:

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

The hexadecimal placeholders (`0x...`) can be replaced with concrete Goldilocks
field elements when generating automated tests.  Stage 2 fixtures additionally
record the Poseidon hash of the running accumulator but keep the same JSON shape,
so the example can double as a template for future test vectors.
