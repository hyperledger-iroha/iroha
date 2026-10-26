# Iroha dependency patch

Upstream: ark-ff 0.5.0, crates.io archive SHA-256
`a177aba0ed1e0fbb62aa9f6d0502e9b46dad8c2eab04c14258a1212d2557ea70`.

Replace the unmaintained paste procedural macro with maintained pastey 0.2.3. The dependency keeps its upstream import name; runtime arithmetic and algorithms are unchanged.

The normalized Cargo.toml is the active manifest; Cargo.toml.orig records the unmodified upstream manifest. All other upstream source bytes are retained. Remove this patch when an upstream release carries this dependency change.
