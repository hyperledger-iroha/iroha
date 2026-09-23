# Topology inner approval claim boundary

The production promotion checker still rejects release. Its topology inner
input is the schema-closed, detached
`sorafs.l1.deployment_qualification.signed_envelope.v1` JSON envelope. The
existing loader verifies that envelope against an independently pinned Ed25519
key and signer tuple, exact topology summary bytes, deployment and evaluation
time. The promotion checker now also rechecks the returned binding's exact
qualification-only field set, authentication kind and independently pinned
signer identity. A self-consistent foreign signer tuple, changed fingerprint,
or extra `native_completed_operation` field cannot turn this binding into a
completed native approval. The final release blocker remains unconditional.

The required role-16 authority artifacts do not yet exist in this producer
chain. The detached envelope lacks the canonical candidate-bound
`TopologyApprovalSubjectV1` (including release-manifest SHA-256), current
governed custody, original reservation, four-signature receipt, immutable
completed operation and finalized exact Check input/result/output. The Rust
`check_topology_receipt_consistency_v1` validates a caller-supplied receipt and
state only for consistency; it deliberately grants no native authority token.
Core has no topology-owned native ISI/State/query producer or executable
finalized Check consumer. The checker cannot invent these artifacts, infer them
from the detached signature, or reuse another signer's operation. Authenticated
software custody is sufficient once the native authority is implemented; HSM
access is not a prerequisite.

The focused Python suite
`python3 -m pytest -q scripts/tests/check_sorafs_production_promotion_bundle_test.py`
passed 196/196. These are checker and synthetic negative tests, not production
evidence, independent authorization, native completion or promotion.
