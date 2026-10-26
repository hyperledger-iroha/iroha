# Iroha dependency patch

Upstream num-bigint 0.5.1, crates.io archive SHA-256
`93e7820bc0a80a0238e650327316f929ba18d5be054b647490a3a6a339f3e7c0`.

Retain native-digit adoption and borrowing APIs and the std-only build. Full-capacity canonical vectors remain heap-owned without repacking; ordinary small integers use the upstream inline representation.

The upstream normalized manifest and source are rebased together. Ownership tests
exercise the retained APIs; preserve them on subsequent upgrades.
