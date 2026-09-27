# X509 complete MAIN registration partition, 2026-09-24

Scope: the existing `optimizations` checkout. The current maximum X5S1 proof
is 19,156,074 bytes against the unchanged 9,437,184-byte (9 MiB) ceiling.
Its 5,623 MAIN trace columns alone require 12,235,648 direct-opening bytes,
so an opening-family redesign is necessary while preserving every certificate,
CRL, signature, disclosure, and ownership check.

A test-only audit now derives the current MAIN registration partition from
`AggregateProofLayoutV1::for_full_profile_v1()`. It assigns every one of 49
registrations and all 5,623 base/aux columns exactly once, rejecting omitted
or overlapping coverage. The proposed replacement target owns 36 P-256
registrations and 4,000 columns plus four SHA registrations and 668 columns.
The retained MAIN portion owns nine registrations and 955 columns, including
five log-8 scalar-bit buses totaling 190 columns. An initial adapter-only
classification incorrectly counted those buses as replaceable; the current
source-derived test corrected that distinction.

At unchanged fixed costs, removing only the targeted sampled and DEEP opening
bytes leaves 8,699,754 bytes, or 737,430 bytes under the ceiling for a future
receipt and all new roots, claims, transcript data, and framing. This is an
accounting allowance, not a constructed proof bound. The focused Core
`full_main_replacement_partition` selector passes 2/2; scoped formatting,
diff check, and the geometry screen pass. The screen still reports
`production_qualified: false`.

The wire, certificate coverage, cap, and fail-closed activation are unchanged.
A reviewed recursive or narrower AIR construction, terminal linkage,
composed soundness/privacy argument, exact maximum proof, and measured
proving/verification resources remain open.
