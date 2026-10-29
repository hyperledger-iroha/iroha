# Iroha dependency patch

Upstream: accesskit_atspi_common 0.21.1, crates.io archive SHA-256
`24f5baf62e34a4a6c30e3d697e4782f821a7337049c44c3b40576a27dfbb580d`.

Migrate the Linux accessibility dependency chain to maintained zbus 6, whose wire and names modules replace the retired standalone zvariant and zbus_names crates. Cargo.toml.orig records the original upstream manifest. All source changes belong to this migration; remove the local patch once upstream ships it.

The patched source uses zbus 6 types/derives and the unified error type directly, with no deprecated zvariant alias. XML checks use zbus_xml 6 iterators and preserve method direction. AT-SPI wire names, discriminants and signatures are unchanged. Accesskit drives the zbus runtime for the lifetime of its event loop, preserving accessibility; the copied zbus 5 executor was removed. Registration adapts the new return value (`None` means a new interface) without inverting duplicate handling.

Validation includes 1,141 AT-SPI wire/XML tests, 25 lockstep unit tests, and Accesskit application property roundtrips over actual Unix sockets under both default-rt and Tokio. This host validation does not qualify a Linux desktop or screen reader.

Rebased on the 2026-10-25 upstream patch: adapter insertion remains sorted by ID, and cache signals preserve their single structure argument. The existing zbus 6 migration and real-message regression remain in place.
