# Bootle/Lantern V1 revocation boundary

The first Exact12 Bootle/Lantern operation is anonymous selective disclosure.
Its native `ILN1` presentation proves knowledge of one valid Falcon/NTRU
credential opening and the requested attribute predicates against the exact
current committed issuer-policy record.

Bootle/Lantern V1 does **not** define a per-credential revocation accumulator,
a non-revocation witness, or an accumulator root/epoch in its public statement
or native proof relation. `PrivacyRootRoleV1::Revocation` is therefore reserved
and is incompatible with the Bootle/Lantern V1 namespace. Governance root
publication, root-head creation, and retention planning for that combination
must fail closed rather than advertise an unverifiable revocation capability.

The supported first-release revocation mechanism is the issuer-policy
lifecycle. A presentation selects the current policy epoch and complete record
digest. An active-to-active rotation makes proofs bound to the predecessor
record stale, while an active-to-terminal `Revoked` successor rejects every
credential in that issuer/policy lineage. The terminal record preserves the
issuer key, parameter binding, disclosure rules, and allowed values, and the
lineage cannot be rotated or reactivated afterward.

Adding per-credential revocation requires a new versioned protocol whose typed
statement, witness, native verifier equations, governance state, fixtures, and
release evidence all bind the same authoritative accumulator root and epoch.
It must not be introduced by enabling the reserved root role for the existing
V1 proof.
