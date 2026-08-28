# SCCP circuit KAT final V1

Every checked-in file under `internal/circuit/testdata/kats/` is one strict
public known-answer vector for exactly one closed profile. A vector is test
material, not a proof, key, ceremony receipt, or production-readiness claim.

The JSON object has exactly these fields:

- `schema`: `sccp-circuit-kat-final-v1`;
- `version`: `1`;
- `profile`, `role`, `curve`, and `independent_key_id`: exact values from the
  closed profile catalogue;
- `raw_signals`: eleven lowercase, 64-character hexadecimal byte words; and
- `public_signals`: eleven lowercase, 64-character hexadecimal scalar-field
  words.

For message profiles, raw signal order is message ID, payload hash, target
domain, commitment root, finality height, finality block hash, source domain,
statement hash, destination-binding hash, route-configuration hash, and SORA
finality-anchor hash. Their labels are the fixed labels in `message.go` and
match the destination public-signal schema.

For epoch-anchor-update profiles, raw signal order is current-anchor hash,
next-anchor hash, next-snapshot hash, next context ID, activation height,
transition block hash, Taira chain-ID hash, transition finality-artifact hash,
next-roster hash, deployment-policy hash, and independent circuit-key ID. The
label for role `R` is:

```text
sccp:groth16:{curve}:epoch-anchor:signal:{R}:v1
```

Here `{curve}` is exactly `bn254` or `bls12-381`. For BN254, a public word is
`Keccak-256(Keccak-256(label) || raw) mod Fr`. For BLS12-381, it is
`SHA-256(SHA-256(label) || raw) mod Fr`. Words are encoded as 32-byte
big-endian lowercase hexadecimal values.

`TestCheckedInPublicKATs` reconstructs and byte-compares all eight vectors.
`manifests/kat-inventory-final-v1.json` records their exact SHA-256 digests,
and `TestCheckedInKATInventoryAuthenticatesEveryVector` rejects any path,
profile ordering, or byte-level drift.

`manifests/constraint-counts-final-v1.json` records the fresh R1CS constraint
count for every closed profile definition and pairs it with that profile's KAT
digest. These source-level counts are not substitutes for the R1CS/PK/VK and
fixed-verifier artifact hashes produced and signed by the circuit-specific
ceremonies.

`TestEightProfileKATsAndPublicMutationNegatives` solves each positive circuit
assignment and requires every single public-signal mutation to fail. The
epoch suite additionally composes one emitted successor anchor into a second
authenticated advance and rejects stale, wrong-roster, wrong-boundary, and
same-height-equivocation substitutions. The source-level semantic
implementation guard remains false regardless of KAT success.
