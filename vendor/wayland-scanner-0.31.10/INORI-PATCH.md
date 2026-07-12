# Inori security patch

This directory vendors the crates.io `wayland-scanner` 0.31.10 release. Its
only local change is upgrading `quick-xml` from 0.39 to 0.41, which contains
the fixes for RUSTSEC-2026-0194 and RUSTSEC-2026-0195. The parser APIs used by
`wayland-scanner` are source-compatible with 0.41 and are covered by the
workspace's all-feature checks.

Remove this override once an upstream `wayland-scanner` release requires
`quick-xml >= 0.41`.
