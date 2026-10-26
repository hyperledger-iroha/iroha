# Iroha dependency patch

Upstream: halo2curves 0.10.0, crates.io archive SHA-256
`d124fcb02bccc7a9253a8aaa99a365f47f0f8b5a363b26a494f06f4a221bd4b7`.

Replace the unmaintained paste procedural macro with maintained pastey 0.2.3. The dependency keeps its upstream import name; runtime arithmetic and algorithms are unchanged.

The normalized Cargo.toml is the active manifest; Cargo.toml.orig records the unmodified upstream manifest. All other upstream source bytes are retained. Remove this patch when an upstream release carries this dependency change.
