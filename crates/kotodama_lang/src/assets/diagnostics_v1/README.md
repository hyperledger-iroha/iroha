# Kotodama diagnostic tables v1

These versioned TSV files are the canonical data behind the public diagnostic
explanation registry and its two table-driven diagnostic test matrices. Fields
use a strict backslash escape set (backslash, tab, carriage return, and line
feed); the build script validates every header, row count, field count, phase,
numeric line, stable identifier, and duplicate key before generating Rust
items in `OUT_DIR`.

The repository compile-time asset checker pins each asset's byte length and
SHA-256 plus the exact historical Rust constant declaration from which it was
projected. A focused standard-library Python test independently reconstructs
the projection and checks ordering and values.
