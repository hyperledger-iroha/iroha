# Owned ciphertext ingress

`into_source_algebra_prerequisite_v2` consumes the original materialized source,
its 43 ciphertext manifests, the retained key authority, original randomness and
CAS providers. The private ingress has no separate ordering-seal input.

`exact_manifest_preflight_v2` first validates the materialized owner and then
requires the exact 43-record allocation and completed sample cursor. Every
manifest must match the retained release-profile key authority, level zero,
its sample ordinal and the canonical family/chunk/used-slot mapping. The
ordered families are X, U, E, rE, W and rW, with 1, 16, 16, 1, 8 and 1 records.
The underlying BGV encryption has 38 limbs per component; this is separate from
the native40 proof geometry.

Each key pre-read and second-read receipt must refer to one common source
snapshot. Every ciphertext publication and independent readback must share one
output publication identity and snapshot. The exact owned manifests, original
source receipt and materialized/key context enter the existing ordered bundle,
lineage and prerequisite digests. No digest or pointer-only replacement owner
is accepted.

The ingress takes its live owner before validation. Any error or unwind drops
that owner; there is no retry or recovery return. Freezing takes the preflight
owner, revalidates the materialized source and checks its private preflight axes
before constructing the prerequisite. These operations do not assert radix,
quotient, Hyrax, qPCS, operational-receipt or release completion.

TODO: connect the real Core statement/network/batch and governed key-admission
authorities to the opaque Phase23 context and source-correspondence constructor.
The current Core ZK-AMS credential route is distinct from MKHE source production.
The retained RNG handoff must continue that original health-checked session.
No arbitrary digest, RNG or detached seal can establish this missing authority.
Full source replay and cryptographic proof completion remain separate obligations.

The component tests build an actual tiny-profile encryption/publication manifest
and test the production key-axis comparison plus manifest receipt/order checks.
They do not construct a release-sized 43-record source or a production context.
