# Iroha dependency patch

Upstream: zbus-lockstep-macros 0.7.0, crates.io archive SHA-256
`d0212bfb9b37899fa93da3e0abc187c48a3db13b19b46f982abeb7a832814bbd`.

Migrate the Linux accessibility dependency chain to maintained zbus 6, whose wire and names modules replace the retired standalone zvariant and zbus_names crates. Cargo.toml.orig records the original upstream manifest. All source changes belong to this migration; remove the local patch once upstream ships it.

The patched source uses zbus 6 types/derives and the unified error type directly, with no deprecated zvariant alias. XML checks use zbus_xml 6 iterators and preserve method direction. AT-SPI wire names, discriminants and signatures are unchanged. Accesskit drives the zbus runtime for the lifetime of its event loop, preserving accessibility; the copied zbus 5 executor was removed. Registration adapts the new return value (`None` means a new interface) without inverting duplicate handling.

Validation includes 1,141 AT-SPI wire/XML tests, 25 lockstep unit tests, and Accesskit application property roundtrips over actual Unix sockets under both default-rt and Tokio. This host validation does not qualify a Linux desktop or screen reader.
