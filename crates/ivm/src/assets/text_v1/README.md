# IVM compile-time text assets v1

The `.metal` files are exact UTF-8 inputs to the separately built, embedded
Metal V1 library. `build.rs` includes them to bind source bytes to that bundle;
the VM loads the precompiled library and does not compile source at startup.
The `.xml` files are exact ISO 20022 test inputs consumed by `iso20022.rs`.
Each consumer uses `include_str!`, so the result remains a `&'static str` with
no runtime parser or added allocation.

`manifest.json` pins every asset's length and SHA-256 plus the exact historical
Rust line span from which it was extracted. The repository compile-time asset
checker verifies the asset inventory, source preimages, and unique package-local
consumer.
