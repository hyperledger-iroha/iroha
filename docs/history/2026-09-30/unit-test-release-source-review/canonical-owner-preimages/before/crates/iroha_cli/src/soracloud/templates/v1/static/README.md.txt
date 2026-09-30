# Soracloud static templates v1

These files preserve the exact UTF-8 template bytes formerly embedded as Rust
raw strings in `soracloud.rs`. The existing substitution chains operate directly
on `include_str!` results, preserving their output bytes and allocation behavior.

`manifest.json` seals each asset and its exact historical Rust extraction span.
The repository compile-time asset checker also requires one package-local
`include_str!` consumer for every file.
