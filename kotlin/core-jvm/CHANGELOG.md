# Changelog

## v0.1.0 (unreleased)
- Revalidated parity after the Rust Norito JSON `Value` parser now rejects
  arrays/objects nested more than 128 levels deep instead of recursing until
  the thread stack overflows; Kotlin bindings required no implementation
  changes beyond rerunning the parity suite because this is an internal
  Rust-side safety check.
