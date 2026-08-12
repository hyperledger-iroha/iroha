# IVM compile-time text assets v1

The `.metal` files are the exact UTF-8 kernel source strings passed to the
existing Metal compilation path. The `.xml` files are the exact ISO 20022 test
inputs consumed by `iso20022.rs`. Each consumer uses `include_str!`, so the
result remains a `&'static str` with no runtime parser or added allocation.

`manifest.json` pins every asset's length and SHA-256 plus the exact historical
Rust line span from which it was extracted. The repository compile-time asset
checker verifies the asset inventory, source preimages, and unique package-local
consumer.
