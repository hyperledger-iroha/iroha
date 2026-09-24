# Final-promotion role-15 software key slice — 2026-09-24

The `optimizations` checkout now has a purpose-specific, one-use software key adapter for a
previously authorized final-promotion account transaction. It accepts only the complete configured
role-15 Ed25519 binding for the final-promotion account purpose, loads a canonical private-key
multihash from the existing owner-only supervisor credential reader, and checks the credential's
public key before retaining it. The runtime and key handles must be production-shaped role-15
software handles. At signing, the adapter compares the entire binding (including handle, policy,
network and key revision), validates the exact one-instruction native Reserve/Complete payload,
recomputes its ordinary transaction prehash, signs once, and verifies the returned signature.
Consuming the adapter prevents a second in-process use after an ambiguous provider outcome.

The adapter is reachable only through the account transaction's private typed key request, which
the existing continuation issues after finalized role-14 and role-15 Current Checks. That
continuation retains the signature until fresh post-key account and receipt Checks succeed. The
generic external software signer still rejects role 15. No HSM is required or claimed.

This is a signing component, not a production Reserve operation. The account continuation still
needs configured, independently reviewed bindings and secret delivery; a distinct observer key;
approved fee and transaction-timing policy; production ordinary-transaction submission and
same-envelope ambiguous-result reconciliation; qualified UTC; and a durable independently
retained chain/committee floor. The account and receipt Current and post-key Check collaborators
in the daemon still have only test implementations. A finalized Reserve source must join the exact
signed role-15 transaction and aligned successful execution to the immutable native operation row
and its intent, custody, reservation fence and expiry in one State view, with matching durable Kura
and revision-4 finality evidence. A fresh role-14 `BeforeProvider` Check must then establish
current reservation authority. The role-14 `SignerOperationStateSourceV1` phase driver, four-signature
receipt completion, restart journal, and independent promotion approvals remain open. A local
simulated native test cannot replace those production sources or release qualification.

Focused tests cover exact Reserve signing, wrong role/handle/generation/credential, full-payload
prehash substitution, revoked account custody before signature authorization, and a lost provider
response after key use that exposes no signed transaction or post-key Check. On the combined
`optimizations` source, `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib
role15_software_key -- --nocapture` passed 4/4; the compiled daemon binary's separate
`role15_revocation_after_check_execution_rejects_authorization_before_key_use` selector passed
1/1, and its complete `signer_operation::final_promotion::account_transaction::tests` selector
passed 24/24. These are local tests, not deployed qualification.
