# KAGEMUSHA V1 native layout authentication

The release has two distinct profile identities:

- `profile_digest` identifies the circuit-shape evidence file and the released
  state/helper protocol inventory.
- `native_profile_digest` identifies the exact native circuit configuration used
  before decoding structured-v1 Halo2 proving keys or Processed verifying keys. It is a required field of
  `KagemushaInternalValidationReceiptV1` and is authenticated by the receipt digest,
  manifest, and threshold release approvals. It must be nonzero and distinct from
  the evidence profile digest.

`KagemushaAuthenticatedReleaseV1` and `KagemushaAuthenticatedArtifactSetV1` retain
the native identity privately. `KagemushaRecursiveVerifierProfileV1` compares its
locally derived native digest with that identity before reading any artifact. A
decoded report, digest, or layout alone cannot construct an authenticated release.

The observed `iroha.kagemusha_v1.circuit_shape_report` requires `native_profile`:
the exact runtime profile JSON object, with 22 layout objects, seven 32-byte
unsigned integer arrays, and the required numeric `mint_authorization_family`.
Family `1` selects `RecursiveHardware84` with 84 public instances; family `2`
selects `OrdinaryPreDebit113` with 113 public instances. The width comes from the
selected circuit family; the JSON object cannot supply or override it. The release evidence verifier derives its digest itself;
it does not accept a report-supplied digest. Unknown or missing fields, booleans
used as numbers, noncanonical Pasta protocol scalars, invalid phase allocation,
integer overflow, and profiles exceeding the runtime 64-KiB JSON limit fail closed.
The mint-credit, mint-hash-shard, and mint-hash-claim protocol pairs must equal the
corresponding independently selected helper inventory. The nonzero genesis authorization
is committed by the same signed native identity; runtime finality validation must
still establish its relationship to the actual network and finalized authorization.

The native digest preimage is:

1. ASCII `iroha:kagemusha:v1:paired-recursive-circuit-profile`, one zero byte,
   and the version `1` as `u32LE`.
2. Tags 1 through 30 in ascending order, each as one byte followed by its value.
3. A layout value encodes `k:u64LE`, the advice phase vector, `num_fixed:u64LE`,
   the lookup phase vector, `lookup_bits` as `1:u8 || value:u64LE`, and
   `num_instance_columns:u64LE`. Each vector is `count:u64LE` followed by its
   `u64LE` elements. Digest values are exactly 32 bytes.
4. Tag 30 encodes the required family as one byte followed by its exact public
   instance count as `u64LE`. Missing or unknown family tags, mismatched widths,
   truncation, and trailing bytes fail native canonical decoding.
5. SHA-256 of the complete preimage, without an additional length prefix.

| Tags | Values in order |
| --- | --- |
| 1–14 | inner State Eq/Ep, outer State Eq/Ep, Guard Eq/Ep, TerminalAuthorization Eq/Ep, CommitWrapper Eq/Ep, outer MintAuthorization Eq/Ep, outer MintCredit Eq/Ep layouts |
| 15–17 | MintCredit Eq protocol, MintCredit Ep protocol, genesis authorization ID |
| 18–25 | inner MintAuthorization Eq/Ep, inner MintCredit Eq/Ep, MintHashShard Eq/Ep, MintHashClaim Eq/Ep layouts |
| 26–29 | MintHashShard Eq/Ep and MintHashClaim Eq/Ep protocol digests |
| 30 | mint authorization family byte and public instance count |

The native loader and Python verifier share the fixed layout rules: shard layouts
use `k=12`, all other layouts use `k=16`, and lookup bits equal `k-1`. Inner
MintAuthorization uses two instance columns, MintHashClaim uses three, and every
other layout uses one. Phase allocation follows native `BaseConfig` allocation
order, including zero-filled trailing phase entries.

The Rust/Python shape-only golden uses advice `[1]`, lookup advice `[1]`, fixed
count 1, the role-specific layout settings above, protocol scalars 1, 2, 4, 5, 6,
7 in tag order and authorization bytes `[3;32]`. Each complete preimage is 1,749
bytes. Family 1 hashes to
`5b14e09f3f4d83ceea1a1fd4e293e524a0338940d29b18fa394d28099ab7c413`;
family 2 hashes to
`dbe397f8f5e842cc83b446903357d31e69d1363a0b800a5a3c5bdb5fdfeee221`.
This fixture is codec evidence, not proof synthesis or hardware qualification.

Release construction fixes the exact layouts and artifact inventory before the
final receipt and manifest are signed. Bootstrap and monetary proofs then use
the resulting release identity. Release-dependent proof bytes must not enter the
native layout digest, which would create a circular release identity.
