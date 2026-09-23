# zk-X509 Exact12 proof geometry, 2026-09-23

This is a read-only source audit at HEAD `1b8e5f92b6dadb1dcdd16f945286560e31b347ff`, not a proof-size measurement or activation certificate. No Cargo build or Rust edit was made for this audit.

## Complete relation that must survive a redesign

The canonical witness permits two or three leaf-first certificates (at most 4,096 DER bytes each), one complete signed base CRL (at most 4,096 DER bytes and 64 revoked entries), a depth-12 governed CA membership path, a low-`s` wallet-ownership signature, and up to four salted attribute disclosures. The native relation checks strict DER and the closed RFC 5280 profile, name/key/time/path linkage, certificate and CRL signatures, issuer-scoped leaf nonrevocation against **every** active CRL entry, CA membership, projections and holder ownership. Source: `crates/iroha_core/src/privacy_engines/zk_x509/{codec.rs:61-75,relation.rs:1-11,relation.rs:150-196,profile.rs:30-52,profile.rs:105-152}` and `crates/iroha_data_model/src/privacy/credentials.rs:90-99`.

MAIN registers 49 logical adapters in six same-log groups `[5, 8, 15, 16, 18, 19]` and 80 physical chunks; the separate compact-CA proof covers its 104 active rows on a padded log-13 trace. Assembly includes strict DER/RFC, all 29 SHA calls, five P-256 equations, projection and a shared byte-memory relation. Source: `crates/iroha_core/src/privacy_engines/zk_x509/{profile.rs:155-168,profile.rs:277-299,stark.rs:341-373,stark.rs:1759-1807,main_assembly.rs:1-11}`. Replacing that registration with a smaller certificate/CRL/path policy would change the admitted relation, not just its representation.

## Current canonical size

The shared protocol has 136 distinct queries, eightfold LDE, one Fp4 composition lane, six MAIN quotient chunks, 12 MAIN FRI rounds, and a log-22 common MAIN LDE. These are pinned in `profile.rs:191-255,270-299`. The aggregate codec writes every sampled base and auxiliary current/next trace field as an eight-byte value, then Fp4 composition/FRI values, roots and multiproof frontiers (`aggregate_stark.rs:4001-4095,4180-4246`). It pads each valid proof to the public-profile maximum (`aggregate_stark.rs:4152-4167`).

| Component | Exact bytes | Source and arithmetic |
| --- | ---: | --- |
| MAIN sampled trace fields | 12,235,648 | `136 queries × 2 rows × 5,623 columns × 8`; `profile.rs:288-296`, `stark/der_and_native_proof_tests.rs:469-475` |
| MAIN inner, including DEEP | 16,447,808 | `16,087,744` pre-DEEP + `360,064` MAIN DEEP; `profile.rs:246-255,288-291` |
| X5M1 claim frame | 11,952 | `profile.rs:252-253` |
| CA inner, including DEEP | 2,694,912 | `2,642,112` pre-DEEP + `52,800` CA DEEP; `profile.rs:246-255,297-299` |
| X5C1 claim frame | 1,310 | `profile.rs:250-251` |
| X5S1 outer frame | 92 | `credential_stark.rs:35-43,1055-1072` |
| **Combined canonical maximum** | **19,156,074** | Sum above; `profile.rs:328-329,816-833` |

The unchanged consensus ceiling is **9,437,184 bytes** (`profile.rs:263-265`). The maximum exceeds it by **9,718,890 bytes**. The sampled MAIN trace fields are **63.87%** of the combined maximum and exceed the *entire* ceiling by **2,798,464 bytes** before Merkle, FRI, DEEP or claim data. The log-19 MAIN group is 1,940 base + 1,772 auxiliary columns, or **8,077,312** sampled bytes; 2,395 of its columns and **5,211,520** sampled bytes are the five P-256 signatures (`stark.rs:7304-7352`).

If every non-trace byte stayed fixed, it would consume 6,920,426 bytes, leaving **2,516,758** bytes for trace openings or their replacement. An unchanged 136-query/current-next/eight-byte opening would therefore allow at most **1,156** columns, down from 5,623. This is only a screening calculation: a new AIR or argument changes other proof and soundness costs. Canonical Merkle deduplication and streaming already exist; neither removes sampled field values (`profile.rs:301-308`, `aggregate_stark.rs:4180-4237`). The current preflight rejects the complete MAIN before entropy or commitment, and profile validation rejects the combined maximum (`stark.rs:2294-2312`, `profile.rs:816-833`). The X5S1 main section's allocated ceiling is 6,740,870 bytes (`credential_stark.rs:44-57,1055-1084`).

## Redesign direction and unresolved obligations

A plausible *argument redesign to investigate* is recursive composition: keep complete, verifier-fixed child relations for DER/RFC/CRL, 29 SHA calls, five P-256 equations, byte memory, projections and compact CA; commit all child base traces before deriving the existing joint X5B1 challenge family; make a privacy-preserving outer proof verify every child proof, shared challenge chronology, all cross-trace grand-product terminals, the CA root/SPKI channel, and the same public statement. The outer verifier must check child verifier keys/profile digests and all equality links itself. The current joint schedule derives 272 fields only after six MAIN plus one CA base root, and binds them into both subproofs before auxiliary commitments (`credential_pre_aux.rs:1-8,53-85`); independently proving children without this schedule would be unsound. The exact X5S1 public binding and terminal checks are in `credential_stark.rs:1-7,70-118,344-365`.

This is **not** yet a defensible under-9-MiB implementation. There is no instantiated recursive verifier, bound on the outer proof's worst-case encoded size, composed 128-bit soundness/privacy analysis, or measured 300-second/12-GiB prover evidence in the audited source. A narrower microcoded AIR that serializes the five P-256 lanes and SHA slices is another representation research path, but it must demonstrate a complete byte-memory/cross-family relation, deterministic new trace/FRI geometry and a whole-proof bound; moving work into more rows alone does not establish that bound. The compiled KAT and soundness/resource pins are still zero and readiness remains closed (`profile.rs:330-346,541-581`). Neither lowering the 136 queries, omitting certificate/CRL cases, compressing masked field openings as if they were small integers, nor raising the 9-MiB cap follows from the current evidence.
