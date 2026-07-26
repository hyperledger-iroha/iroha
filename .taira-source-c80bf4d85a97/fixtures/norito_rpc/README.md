## Norito-RPC Fixtures

This directory holds the canonical manifest and encoded payloads used to ensure
Norito-RPC parity across SDKs. The manifest (`transaction_fixtures.manifest.json`)
captures each sample transaction in both JSON/base64 form and as raw `.norito`
payloads. Use `cargo xtask norito-rpc-fixtures` to regenerate the canonical set
from `java/iroha_android/src/test/resources/transaction_payloads.json`, and run
`cargo xtask norito-rpc-verify` to validate the hashes, lengths, and SDK copies
whenever transaction layouts intentionally change.
Fixture regeneration rewrites `transaction_payloads.json` with the freshly
encoded payloads so SDK fixtures stay aligned with the current Norito encoding.

`schema_hashes.json` lists the Norito schema hash for every DTO that the NRPC
spec references (transactions, queries, and the SNS registrar payloads). It is
emitted alongside the fixture manifest so operators and SDK CI jobs can confirm
that Torii and clients share the same schema table before flipping transports.
`cargo xtask norito-rpc-fixtures` regenerates this file and
`cargo xtask norito-rpc-verify` ensures the hashes stay aligned with the
compiled data model.
