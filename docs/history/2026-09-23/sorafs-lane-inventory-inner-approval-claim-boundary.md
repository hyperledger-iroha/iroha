# Lane-inventory inner approval claim boundary

The existing signed lane inventory is a canonical JSON file with one detached
Ed25519 signature over the 17 ordered readiness summaries. Its verifier
replays the exact summary bytes, deployment and topology anchors, requires the
`l1-lane-evidence-inventory` signer role label and independently supplied
software signer tuple, and checks the signature. The promotion checker compares
that projection with the positive aggregate and all 17 replayed lane hashes.
This proves qualification-file integrity, not native signer custody or a
completed operation.

The promotion checker now separately requires the exact eight-field
verification projection, canonical inventory-byte digest, expected
purpose-specific signer tuple, topology deployment and four topology anchors.
Foreign role/administrator, changed topology, numeric-type substitution, and
an extra fake `native_completed_operation` field fail closed. It emits a
lane-inventory-specific native-authority blocker and retains the unconditional
final release blocker.

`SignerRoleV1` has no lane-inventory role or purpose. No purpose-owned native
custody, exclusive reservation, immutable completed operation or finalized
Check producer exists for this approval. The signed JSON inventory cannot be
relabelled as another role's operation or as a native approval. Authenticated
software custody can satisfy the future key requirement; an HSM is not a
prerequisite.

The focused command
`python3 -m pytest -q scripts/tests/check_sorafs_production_promotion_bundle_test.py`
passed 199/199. Synthetic negative tests and checker replay are not release
qualification or operator-signed promotion evidence.
