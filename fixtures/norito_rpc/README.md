## Norito-RPC Fixtures

This directory holds the canonical manifest and encoded payloads used to ensure
Norito-RPC parity across SDKs. The manifest (`transaction_fixtures.manifest.json`)
captures each sample transaction in both JSON/base64 form and as raw `.norito`
payloads. Render two complete owner publications from this directory's
`transaction_payloads.json` at independent absent external roots:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
```

Both roots are create-only. Before any tracked update, require identical sorted
repository-relative path sets, entry types, modes, completion manifests, and
every file byte. Apply the reviewed identity-relative patch mechanically from
either sealed tree, then run `cargo run --locked -p xtask --features dev-tools
--bin xtask -- norito-rpc-verify` to validate the hashes, lengths, and SDK
copies. Fixture regeneration renders the derived fields in the canonical
descriptor and publishes byte-identical
payload JSON and manifest files to Java, Python, and Swift. Java also receives
local copies of the 27 active encoded payloads required by its resource-based
tests. Python and Swift receive only the descriptor and manifest; they do not
carry redundant `.norito` mirrors. Runtime-dependent fixtures are valid active
transactions and bind a positive gas limit in `fee_payment`.

The generator always reads the canonical workspace descriptor and writes the
complete repository-relative publication beneath each new external output root
without touching tracked files.

The canonical `transaction_fixtures.manifest.json` is the publication's
completion seal. It is linked into place without clobbering only after every
other owned file is complete and synced; a missing, partial, symlinked,
hard-linked, or non-0644 seal means the root is rejected residue, not a usable
publication. Publication checks revalidate the external parent, root, and every
created directory identity around each filesystem operation. Portable Rust
`std` APIs cannot eliminate a hostile same-UID process swapping a directory and
restoring it entirely between two checks, so release generation must use a
private parent directory that no other process can mutate.

`schema_hashes.json` lists the Norito schema hash for every DTO that the NRPC
spec references (transactions, queries, and the SNS registrar payloads). It is
emitted alongside the fixture manifest so operators and SDK CI jobs can confirm
that Torii and clients share the same schema table before flipping transports.
The `norito-rpc-fixtures --output-root` command above renders this file into the
sealed external tree; `norito-rpc-verify` validates the reviewed tracked copy
against the compiled data model.
