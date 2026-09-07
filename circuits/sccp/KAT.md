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

`manifests/constraint-counts-final-v1.json` pairs each closed profile with its
constraint count and KAT digest. Fresh measurements additionally include the
canonical gnark `ConstraintSystem.WriteTo` byte length and SHA-256 identity;
the explicit pending list identifies any historical entries awaiting a new
measurement. `constraint-count --profile <closed-id>` emits those measurements
without generating key material or retaining a serialized R1CS output file.

`definition_source_closure_sha256` detects definition or dependency drift even
when positive KAT bytes remain unchanged. It hashes the UTF-8 domain
`sccp:r1cs-definition-source-closure:v1` followed by one zero byte, then sorted
relative paths for non-test `.go` files under `internal/circuit` and
`internal/profile`, plus `go.mod`, `go.sum`, `vendor/modules.txt`, and
`vendor-inventory-final-v1.json`. Each entry contributes its u32-LE path-byte
length, path bytes, u64-LE file length, and raw SHA-256 file digest. The inventory
test recomputes this closure and requires remeasurement after a mismatch; the
offline builder separately verifies every file in the pinned vendor inventory.

The canonical domain, codec, and transfer-tag alignment plus the composable
retained-anchor and same-height message checkpoint constraints invalidate
earlier affected R1CS definitions and their dependent artifacts.
Unchanged positive KAT bytes do not make an earlier R1CS compatible. Fresh
local measurements do not replace circuit-specific Phase-2 transcripts,
proving keys, verification keys, fixed verifiers, destination deployments, or
the three independent audits required by the signed production corridor.

`TestEightProfileKATsAndPublicMutationNegatives` solves each positive circuit
assignment and requires every single public-signal mutation to fail. The
epoch suite additionally composes one emitted successor anchor into a second
authenticated advance and rejects stale, wrong-roster, wrong-boundary, and
same-height-equivocation substitutions. Focused message authorization tests
check the exact checkpoint block, context, and finality-artifact identity in
both outer fields. KAT success does not establish production admissibility.
