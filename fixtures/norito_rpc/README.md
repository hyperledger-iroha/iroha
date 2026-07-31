## Norito-RPC Fixtures

This directory holds the canonical manifest and encoded payloads used to ensure
Norito-RPC parity across SDKs. The manifest (`transaction_fixtures.manifest.json`)
captures each sample transaction in both JSON/base64 form and as raw `.norito`
payloads. Use `cargo xtask norito-rpc-fixtures` to regenerate the canonical set
from this directory's `transaction_payloads.json`, and run `cargo xtask
norito-rpc-verify` to validate the hashes, lengths, and SDK copies whenever
transaction layouts intentionally change. Fixture regeneration rewrites the
derived fields in that canonical descriptor and publishes byte-identical
payload JSON and manifest files to Java, Python, and Swift. Java also receives
local copies of the 28 encoded payloads required by its resource-based tests;
Python and Swift consume the manifest's embedded base64 or the canonical blobs
and therefore do not carry redundant `.norito` mirrors.

For isolated regeneration, seed the canonical descriptor below a cache tree and
pass `--output-root <cache-tree>`. The generator writes the complete
repository-relative publication beneath that root without touching the live
repository.

`schema_hashes.json` lists the Norito schema hash for every DTO that the NRPC
spec references (transactions, queries, and the SNS registrar payloads). It is
emitted alongside the fixture manifest so operators and SDK CI jobs can confirm
that Torii and clients share the same schema table before flipping transports.
`cargo xtask norito-rpc-fixtures` regenerates this file and
`cargo xtask norito-rpc-verify` ensures the hashes stay aligned with the
compiled data model.
