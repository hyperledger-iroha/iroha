# Iroha dependency patch

Upstream bytes 1.12.1, crates.io archive SHA-256
`fc652a48c352aef3ea3aed32080501cf3ef6ed5da78602a020c991775b0aff04`.

Retain native-only APIs, stable Criterion benches, and post-deallocation owner reclamation. The consumed vtable callbacks now accept the upstream by-value raw pointer.
The concurrency stress test has no alternate-runtime target exclusion; it retains
the existing native endianness/Miri gate and all thread/iteration checks.

The upstream normalized manifest and source are rebased together. Ownership tests
exercise the retained APIs; preserve them on subsequent upgrades.
