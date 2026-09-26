# Release-manifest software service assembly

2026-09-24, `optimizations`. This bounded F04 signing cut connects the existing
role-13 release-manifest ceremony to the owner-only supervisor-credential
software key provider through one purpose-specific constructor. The constructor
requires an explicit `SignerOperationStateSourceV1` and the already opened
private receipt journal. It rejects an absent source, wrong role/purpose,
unreviewed manifest coordinates or wrong journal family before opening the
credential. The key provider and coordinator receive the same source, so the
existing reserve, four ordered key operations, private receipt staging,
authoritative completion, fresh release observation and read-only recovery
remain one path. There is no HSM requirement and no compatibility layout.

The focused test uses an actual owner-only Ed25519 credential with the injected
operation source. It requires a durable completion before release, recovers the
same receipt after the credential file is removed, refuses duplicate operation-id
signing without another reserved key read, and refuses recovery after custody
revocation. Other focused preflight cases use a missing credential path to prove
that absent state, wrong role and invalid review coordinates fail before key I/O.
The injected source is test-only; the test does not produce finalized-native
authority or deployment evidence.

Production role-13 signing remains closed. The generic service has no production
`SignerOperationStateSourceV1` backed by finalized custody/audit,
reservation/completion and fresh Check evidence. The external software adapter,
configured daemon purpose dispatch, exact fee/submission and observer assembly,
ambiguous-operation restart reconciliation, independent trust inputs, and
promotion qualification remain open. These cannot be replaced by the private
receipt journal or by injected test observations.

On the combined checkout,
`scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p irohad
--lib software_release_manifest -- --nocapture` passed 2/2 focused tests (1,334
other daemon tests filtered). The first attempt stopped on two unrelated Torii
`NetworkId` borrow mismatches; the integration owner corrected those call sites,
and the unchanged focused selector passed on retry. The daemon library compiled
through that test run. Scoped Rust formatting, `git diff --check`, and
`scripts/check_no_legacy_codec.sh` passed. No full workspace, four-validator,
external signer or promotion qualification was performed for this cut.
