# Iroha dependency patch

Upstream: zbus-lockstep 0.7.0, crates.io archive SHA-256
`9685b228021d41c5adc5ea4a7f1d0082726681222794e4b75b3816ae5c94b4a6`.

Migrate the Linux accessibility dependency chain to maintained zbus 6, whose wire and names modules replace the retired standalone zvariant and zbus_names crates. Cargo.toml.orig records the original upstream manifest. All source changes belong to this migration; remove the local patch once upstream ships it.

Preserved the upstream XML test fixture from commit `ac20711dd0ce42eb185f19c42d360c12b51aff1e`, `xml/test_definition_file.xml`, SHA-256 `207249b976f9fe0e44e8da4b0d0fef5795193c530ba15d1e1e3fb9924e35212a`. The crates.io archive omitted this upstream test input.

The patched source uses zbus 6 types/derives and the unified error type directly, with no deprecated zvariant alias. XML checks use zbus_xml 6 iterators and preserve method direction. AT-SPI wire names, discriminants and signatures are unchanged. Accesskit drives the zbus runtime for the lifetime of its event loop, preserving accessibility; the copied zbus 5 executor was removed. Registration adapts the new return value (`None` means a new interface) without inverting duplicate handling.

Validation includes 1,141 AT-SPI wire/XML tests, 25 lockstep unit tests, and Accesskit application property roundtrips over actual Unix sockets under both default-rt and Tokio. This host validation does not qualify a Linux desktop or screen reader.
