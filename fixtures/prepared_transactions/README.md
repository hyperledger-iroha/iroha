# Prepared-operation V1 contract

These vectors bind the exact signed onboarding receipt or solved faucet claim,
caller request identity, deadline, and signed transaction wire. Public envelopes
use `iroha.prepared-*`; the binding schema is
`iroha.prepared-operation.binding.v1`.

The binding has exactly `schema`, `semantic_hash_hex`, `kind`, `request_id`, and
`execution_expires_at_unix_ms`. `request_id` is a caller-owned 64-character
lowercase hexadecimal operation identity, persisted across attempts and retries.
It does not provide server deduplication: response-loss recovery reuses the exact
prepared transaction and reconciles its hash. The onboarding semantic hash is
the decoded 32 bytes of `receipt.plan_hash`, encoded as lowercase hexadecimal
(the JSON hash literal itself is not the binding value); its deadline cannot exceed `receipt.body.valid_until_ms`.
The faucet semantic hash uses the existing domain-separated canonical claim hash.
Signed creation time plus positive TTL cannot exceed the binding deadline.

Onboarding authority remains the dedicated scoped token and configured signer.
Verify the original request, expected network, trusted issuer, and signed receipt
before preparing; an embedded issuer signature alone does not establish trust.
Select and verify the fee payer independently. Native reset custody retains its
operator authorization and projects its child idempotency digest into the public
request identity; public customer requests contain no reset authorization fields.

The signature transcript starts with `iroha:prepared-transaction:v1\0`, followed
by the existing length-framed labels `transcript_schema`, `envelope_schema`,
`operation`, `binding.schema`, `binding.semantic_hash_hex`, `binding.kind`,
`binding.request_id`, and `binding.execution_expires_at_unix_ms`, then the exact
operation fields. Its schema is `iroha.prepared-signature-transcript.v1`.

Regenerate the JSON with the Torii library test
`routing::prepared_transaction_signature_fixture_tests::print_prepared_transaction_signature_fixture`
and `IROHA_PRINT_PREPARED_TRANSACTION_SIGNATURE_FIXTURE=1`. Recompile fixture
consumers after regeneration because Rust tests embed this JSON at compile time.
