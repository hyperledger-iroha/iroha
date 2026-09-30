# Source review inventory correction, 2026-09-30

The exact SoraFS manifest preimage is retained as `Cargo.toml.txt`. Its bytes and
SHA-256 are unchanged. The manifest-seal inventory discovers every Cargo.toml,
including historical copies, so the original archive filename itself entered
the reviewed inventory. The first optional-tools snapshot was collected before
writing this preimage; its 7,025-row listing therefore omitted the added archive
manifest. Its digest was computed afterwards and included the archive. No
successful complete source-guard run or release qualification was claimed for
that transitional capture. Subsequent source review must collect and verify
its full inventory after all preimages are safely stored as text.

The failed follow-up audit also refused concurrent root and caller workspace
manifest edits instead of assuming the budget fingerprint was the sole delta.
Those source changes are preserved and require their own semantic review.
