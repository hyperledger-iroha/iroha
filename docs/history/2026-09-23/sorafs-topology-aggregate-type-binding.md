# SoraFS signed-topology aggregate type binding

On the `optimizations` checkout, the final-promotion checker now compares the
independently signed topology binding to the positive aggregate with recursive
exact JSON equality. The previous Python mapping equality treated an integer
and its floating-point spelling as equal (for example `369 == 369.0`), even
though they are distinct JSON values. A rebound aggregate could therefore
escape the specific topology-binding mismatch error. The checker already used
the exact comparison when deciding whether the topology native-authority
blocker applied; the signed-to-aggregate check now uses the same rule.

Two adversarial cases replace the aggregate's `chain_discriminant` or
`signer_key_revision` with the same numeric value as a JSON float while the
original signed topology bytes, independent trust, and inner replay inputs
remain intact. Both must report the topology-binding mismatch. The complete
promotion-checker suite passes: `python3 -m pytest
scripts/tests/check_sorafs_production_promotion_bundle_test.py -q` — 195 tests.

This does not establish role-16 signer authorization or an executed native
topology operation. The old detached signed envelope has no operation receipt
or finalized-state anchor. The purpose-owned State/ISI, current Check,
successful ordered input/result/output proof, generic signer dispatch, and
four-approval cutover described in `specs/sorafs/topology_signer_authority_v1.md`
remain open. The unconditional inner-approval promotion rejection is unchanged.
