# X509 fixed-family source storage

Status: source implementation and isolated owner/AIR controls; normal Core and
complete-proof qualification pending. The preceding frozen optimized baseline
remains unchanged. This work does not change the AIR, compiled profile, proof
format, supported inputs or resource ceilings.

The pinned maximum assembly retains 542,564,850 bytes against its 596,974,144-byte
allowance. Its remaining 54,409,294 bytes cannot admit a log22 Metal transform:
one, two and four columns require 135,299,583, 202,424,830 and 336,675,324 bytes,
including the full shared pool and staging charges. The existing selector
therefore chooses CPU for that measured maximum assembly.

Most RFC family rows retain a 285-field array even though their source operands
occupy a fixed prefix. `rfc5280_source_rows.rs` selects storage solely from the
public verifier-owned family: SourceNode retains 123 fields, Grammar 102,
Calendar and Decimal all 285, and every other family 66. Decimal deliberately
retains its distant generalized-time flag. Every omitted field must be zero;
unexpected source operands fail instead of being discarded. Reconstruction
restores all 285 fields before unchanged carried-selector/helper evaluation.
This is fixed layout storage, not compression selected by private values.

The construction owner remains the existing full-row builder. Once its semantic,
multiplicity, lookup and ordinal scratch owners have dropped, compaction charges
all remaining original capacities, prior compact capacities, the next allocation
and the surviving public schedule against the existing 1 GiB source scratch
allowance. Admission occurs before allocation and actual capacity is checked
again before private writes. An original charge is removed only after its clearing
owner drops. This is a construction-phase payload bound; borrowed source owners,
other proof phases, allocator metadata and process RSS keep their separate
accounting. It is not a whole-process memory certificate.

Compact rows have redacted Debug and clearing ownership across clone, error,
unwind and explicit cleanup. Original rows and partly filled replacements also
clear. The ordinal tuple scratch now has a clearing owner. Native stack and
compiler-created copies retain the existing qualification limits.

Eight isolated exact-source tests pass: four new storage tests and four existing
private-owner tests. They use the actual Goldilocks field and eraser, cover all
public families and every omitted column, compare every reconstructed cell,
reject overflowing/insufficient overlap budgets including spare capacity, and
observe real cells before release on success/error/unwind. Receipt:
`dist/zk-remediation/2026-09-29/x509-family-storage/owner-controls/result.json`.
The first isolation invocation found removed old dependency artifacts and did
not compile; that log is retained. The retry selects existing pinned-toolchain
libraries and records their paths. No normal Core or full-proof result is inferred.

The optimized isolated preflight also passes on the real ordinary and maximum
certificate/CRL fixtures. It compiles the actual parser, field, construction,
column providers and AIR, preserving the preceding isolation's explicitly
documented removal of unused transcript/terminal-codec entrypoints. Every one of
285 base and 280 auxiliary columns is generated; every populated base row and
the selected complete boundary rows satisfy the AIR. DER/RFC handoffs, two
maximum-fixture temporal mutation controls and four generic field-extension AIR
controls pass. The source guard records no drift. Both all-column fixtures take
224.10 seconds together with 482,508,800 bytes peak RSS. This is a component
preflight measurement, not the full credential's 300-second qualification.

Actual retained RFC payload is 63,097,464 bytes for the ordinary fixture and
80,058,504 bytes for the maximum fixture, down from the preceding maximum
421,806,624 bytes. The separate capacity/adversarial phases peak at 515,719,168
and 563,363,840 bytes process RSS. Receipt:
`dist/zk-remediation/2026-09-29/x509-family-storage/release-preflight/complete-receipt.json`.

The remaining checks are normal Core owner/assembly/profile regressions,
measured maximum whole assembly and device selection, and a complete maximum
proof with independent verification. The preceding optimized baseline retains
its repaired focused controls; its not-yet-started full proof is superseded by
the compact candidate rather than reported as a pass or failure.
A lower retained payload alone does not establish the 300-second target or a
12 GiB process bound.
