# Iroha dependency patch

Upstream http-body-util 0.1.5, crates.io archive SHA-256
`23169fe34a5fbcdd3f3862e78fb9b6fccd5f02a6dc6f732547005d45631ce71c`.

Retain post-deallocation body reclamation and its unwind tests.

The upstream normalized manifest and source are rebased together. Ownership tests
exercise the retained APIs; preserve them on subsequent upgrades.
