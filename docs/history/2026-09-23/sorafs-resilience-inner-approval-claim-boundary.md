# Resilience inner approval claim boundary

The existing resilience approval is a detached, domain-separated Ed25519 JSON
receipt over the holistic 19-requirement evidence set and a signed topology
binding. The promotion checker reopens the exact positive-replay summary,
reconstructs the receipt, checks its canonical digest and signature against the
independently pinned resilience key, and compares its projected binding with
the positive aggregate. This establishes a qualification claim, not native
signer authorization or a completed signing operation.

The checker now uses strict JSON equality for the projected resilience binding,
so a numeric type substitution such as `5.0` for integer revision `5` cannot
pass Python's ordinary equality. On an exact replay match, it also requires
the purpose-specific 11-field binding schema, the exact replayed summary-byte
digest, and the independently pinned signer tuple. A foreign-purpose schema,
foreign administrator, or extra claimed `native_completed_operation` field
fails closed. The purpose-specific resilience blocker and the final release
blocker remain.

The current `SignerRoleV1` catalog has no resilience-approval role or purpose,
and no resilience-owned native custody, reservation, immutable completion or
finalized Check producer. A role-5 foundational promotion receipt, role-14
final-promotion receipt, or the resilience detached signature cannot fill this
gap. Authenticated software custody is sufficient when the new purpose and
authority are implemented; no HSM is required. The final promotion checker
must retain its unconditional rejection until all inner approvals are backed
by genuine purpose-owned completed-operation and finalized-state verification.

The focused command
`python3 -m pytest -q scripts/tests/check_sorafs_production_promotion_bundle_test.py`
passed 198/198. These tests include synthetic negative substitutions and do
not constitute deployment evidence or promotion qualification.
