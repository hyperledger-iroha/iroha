# Iroha dependency patch

Upstream: metal 0.33.0, crates.io archive SHA-256
`c7047791b5bc903b8cd963014b355f71dc9864a9a0b727057676c1dcae5cbc15`.

Replace the unmaintained paste procedural macro with maintained pastey 0.2.3. The dependency keeps its upstream import name; runtime arithmetic and algorithms are unchanged.

The normalized Cargo.toml is the active manifest; Cargo.toml.orig records the unmodified upstream manifest. All other upstream source bytes are retained. Remove this patch when an upstream release carries this dependency change.
