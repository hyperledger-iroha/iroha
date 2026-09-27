# SoraFS topology approval authority prerequisite

The production promotion checker now reports a topology-specific native
authority blocker even when the signed role-16 topology envelope matches the
positive aggregate. It still rejects the complete promotion unconditionally.
The envelope authenticates a configuration statement, but cannot prove signer
permission, a purpose-owned native Check, or an executed and completed operation.
A synthetic completed-operation field is rejected by the envelope's closed
schema.

The affected checker and regression are
`scripts/check_sorafs_production_promotion_bundle.py` and
`scripts/tests/check_sorafs_production_promotion_bundle_test.py`. The focused
checker suite passed 193/193 tests on this checkout. This result tests
fail-closed verification and cannot qualify topology deployment or promotion.

A later full run of `python3 -m pytest -q
scripts/tests/check_sorafs_production_promotion_bundle_test.py` on the moving
`optimizations` checkout passed **200/200**. The checker still rejects final
promotion until purpose-owned native completion and Check evidence is joined;
this run is not a signed candidate or approval receipt.

TODO: Implement role-16 native custody and immutable completion state, a
permissioned Check and exact finalized input/result/output verifier, then join
its actual result with the other three inner approvals. Authenticated software
custody has no HSM prerequisite. Keep the final promotion gate closed until
all four inner operations are independently verified against one candidate.
