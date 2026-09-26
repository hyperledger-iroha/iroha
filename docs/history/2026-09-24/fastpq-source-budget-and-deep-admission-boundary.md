# FASTPQ source budget and DEEP admission boundary — 2026-09-24

This record screens the current `optimizations` checkout. It does not contain a
new proof, a security argument, a full-domain run, or production qualification.
The 512 KiB segment and 1 MiB AXT inner-payload ceilings remain fixed.

The [source budget checker](../../../scripts/fastpq/check_compact_source_budget.py)
reads the live 375-query profile, fixed-row codec, Fp4/digest sizes, proof DTO,
admission checks, DEEP cap, and retained diagnostic metadata. It independently
computes the current shared proof's conservative maximum Norito frame and
compares it with the Rust `shared_prover_wire_bound` regression. It fails closed
if the codec, row/query shape, minimum-row check, typed byte admission, or
test-only DEEP module boundary changes. It reads retained artifact lengths from
ignored tests; it does not load or reverify those artifacts.

| Current complete-row shared proof | Bytes |
| --- | ---: |
| Mandatory 375 current rows × 342 `u64` cells | 1,026,000 |
| Mandatory 375 mixed/quotient Fp4 pairs | 24,000 |
| Raw floor before roots, framing, authentication or FRI | **1,050,000** |
| Raw floor over 512 KiB segment / 1 MiB AXT inner ceiling | **525,712 / 1,424** |
| Conservative maximal valid single-segment frame | **4,017,376** |
| FRI values / frontier digests in the independent-maxima bound | 272,512 / 875,760 |

The raw floor applies **only to the current complete-row proof design**. It is
not a lower bound for every possible compact argument. The recorded ordinary
and AXT single-segment lengths are 3,994,619 and 4,015,551 bytes; their
two-segment bundles are 7,986,384 and 8,011,999 bytes. These are source-owned
ignored-test metadata, not fresh measurements. The normal quantity producer
preflights the 4,017,376-byte bound before private trace expansion, and the
bounded shared verifier checks the actual canonical frame against its caller
limit. Enlarged offline diagnostic policies do not grant Core admission.

There is also a distinct **test-only DEEP candidate**. Its source-owned DTO
maximum is **506,351 bytes**, leaving 17,937 bytes under one 512 KiB segment
target. `deep_engine::verify` contains bounded decode, typed statement and
transcript binding, an out-of-domain AIR identity, authenticated row/quotient/FRI
and terminal openings, fold-chain checks, and all 128 terminal values. Its
negative and component tests operate on constructed fixtures. The modules are
`#[cfg(test)]` in `backend.rs`; no non-test call consumes `deep_engine::verify`,
and there is no same-profile DEEP producer or successful emitted complete proof.
The DTO bound also excludes the enclosing AXT carrier. It cannot replace the
current full proof on the strength of codec size or component checks.

The next production cut requires a same-profile producer, successful complete
proofs under exact public statements, reviewed DEEP/AIR/FRI/qROM and witness
privacy arguments for the fixed 64-query, 301-column, blowup-128 construction,
measured proving/verifying resources, and authenticated Core/AXT admission that
removes witness replay. AXT's ordered execution, authoritative roots and durable
spend nonce remain separate requirements. The current Core replay path remains
fail closed for compact admission.

Validation on this checkout: `python3 scripts/fastpq/check_compact_source_budget.py`
produced the exact byte table above; `python3 -m pytest -q
scripts/fastpq/tests/test_compact_source_budget.py` passed **8/8**, including
seven source-drift mutations; `python3 -m py_compile` passed for both new Python
files. No Cargo test was run in this slice because another owner held the shared
build slot. F07 remains open.
