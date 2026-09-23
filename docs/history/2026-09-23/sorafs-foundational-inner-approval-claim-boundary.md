# Foundational inner approval claim boundary

The foundational envelope signs nine ordered prerequisite groups, all 17 lane
summary digests, topology and resilience bindings, the signed lane-inventory
digest, deployment, release sequence and predecessor. The checker pins an
independently reviewed Ed25519 key and a SHA-256 of the exact
`sorafs_external_software_signer verify-receipt` binary. It reruns that verifier
on the public Norito binding and receipt, reconstructed signing payload and
detached signature, and validates the canonical result against the retained
bundle. This proves the software signer's purpose, signature, audit,
provenance and response consistency. It does not authenticate a current Core
custody state or a finalized native completed-operation row.

The promotion checker now explicitly requires receipt replay in its validation
options and rechecks the projected foundational claim against the independently
pinned signer tuple, verifier digest, reviewed sequence/predecessor, exact
inventory bytes, topology and resilience bindings, and all 17 replayed lane
digests. It rejects an extra fake `native_completed_operation` in the receipt
bundle, a substituted verifier, foreign signer, changed prerequisite or a
numeric-type change to the release sequence. A foundational-specific native
authority blocker and the unconditional final release blocker remain.

The existing foundational envelope does not carry the role-16 candidate-bound
topology subject or independently authenticate a release-manifest candidate.
The current software-signer receipt is a local operation record; it is not a
finalized native Reserve/Complete/Check proof. The missing production cut must
bind the exact release candidate and prove purpose-owned current custody,
completed operation and ordered finalized execution from the same native
state. An outer final-promotion receipt cannot substitute for an inner
foundational operation. Authenticated software custody suffices; HSM access is
not required.

The focused command
`python3 -m pytest -q scripts/tests/check_sorafs_production_promotion_bundle_test.py`
passed 200/200. These synthetic negative and replay tests are not independent
operator approval or production promotion evidence.
