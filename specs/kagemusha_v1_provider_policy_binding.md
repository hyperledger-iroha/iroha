# KAGEMUSHA V1 governed provider registry

The provider secret used by `PlatformCredential` is authorized through its public
commitment in the threshold-authenticated release receipt. A witness-selected
Merkle root, a hardware-backed signing key, and the release's enabled-profile
list digest do not independently grant provider proof authority.

`KagemushaInternalValidationReceiptV1.provider_policy` contains exactly one
`KagemushaProviderPolicyEntryV1` per enabled hardware profile, in the same strict
profile-ID order, with a nonzero `provider_authority_commitment`, a unique
`provider_profile_index: u16`, and a required `issuer_signature`. At most 64 rows are admitted. Indices can occupy
any position in the depth-16 tree. No provider secret or supplied sibling list
is accepted in the release receipt. `provider_policy_root` must equal the
independently computed root. The existing `hardware_policy_digest` remains the
Norito-derived enabled-profile-list identity.

Both Rust inventory validation and the isolated Python release projector verify
`issuer_signature` independently under the exact hardware profile's
`governance_credential_public_key`. It is ECDSA-P256-SHA256, encoded as exactly
64 raw big-endian `r || s` bytes with nonzero in-range scalars and low S. The
review JSON represents these bytes as 128 lowercase hexadecimal characters;
there is no algorithm selector or optional authorization path. Observer
approvals, an OEM report, or agreement with a newly selected root cannot replace
this issuer signature.

`kagemusha_provider_policy_signing_bytes_v1` returns the exact 169-byte canonical
Norito frame with schema `iroha.kagemusha.v1.provider-policy-authorization` and
these ordered fields:

| Field | Type / value |
| --- | --- |
| `domain` | `Vec<u8>` containing `iroha:kagemusha:v1:provider-policy-authorization` |
| `version` | `u16`, exactly 1 |
| `hardware_profile_id` | Exact 32-byte hardware profile identity |
| `provider_profile_index` | `u16` registry position |
| `provider_authority_commitment` | 32-byte public provider commitment |

The profile identity already binds the issuer key, suite commitment, policy epoch,
capabilities, firmware policy and validity interval. The signing preimage has no
final release ID, registry root, VK inventory or signature, so the issuer can
authorize the public row before root-specific circuit keys exist. The required
signature is retained in the receipt and pre-run candidate context. Changing
only the signature therefore changes the candidate context, while the tree leaf
continues to use the exact fields below.

The exact SHA-256 tree uses these byte strings, with one trailing zero byte on
each domain:

- Provider commitment: `iroha:kagemusha:v1:provider-proof-authority`, then the
  provider's private 32 bytes. Only the resulting commitment leaves the service.
- Occupied leaf: `iroha:kagemusha:v1:hardware-policy-leaf`, then profile ID (32
  bytes), platform tag (one byte: Android 0, Apple 1, dedicated SE 2, other 3),
  capability mask (u16 little endian), and provider commitment (32 bytes).
- Unoccupied leaf: `iroha:kagemusha:v1:hardware-policy-empty`, with no payload.
- Parent: `iroha:kagemusha:v1:hardware-policy-node`, then left and right digests.

Class and capabilities are derived from the exact governed hardware profile.
Every unoccupied leaf is canonical; the verifier does not trust an opaque
candidate-supplied root or sibling list. The Rust and Python implementations
reduce at most 64 occupied nodes across 16 levels. The Rust public root/path
helpers produce deterministic public material; they do not create a runtime
release capability.

The leaf excludes `canonical_empty_effect_digest`. That sentinel depends on the
final release ID and remains bound in the complete credential and Guard
statement. Including it in the registry leaf would create a cycle through the
root, receipt, and final release ID. The leaf also excludes the VK set, full
qualification matrix, and final release ID. The hardware profile must retain an
independently chosen suite commitment and the stable physical-report identity;
its identity must not include this registry root. Root-specific circuit keys
can therefore be generated after selecting the public registry and before
assembling the final evidence closure.

The physical release verifier includes each public provider row in the immutable
pre-run candidate context. The fresh OEM challenge binds that context. It checks
the physical transcript and endpoint's `hardware_policy_id` against the derived
root and requires the OEM report to name the exact public provider commitment
and index. In addition to the independently checked issuer authorization, the separately pinned OEM verifier must authenticate this commitment
as belonging to the qualified service and verify its actual attestation or
provider-issued authorization; copying the challenge fields into a positive
report is insufficient. Raw attestation, exact verifier implementation, trust
roots, transcript, and observer policy remain in the authenticated evidence
closure. The physical summary report remains independent of these downstream
identities to avoid a profile-ID feedback cycle.

`KagemushaAuthenticatedReleaseV1.provider_policy_root()` and `.provider_policy()`
expose the authenticated result only after the complete receipt, manifest, and
threshold approvals validate. The credential circuit must bind the accepted
root in its actual circuit/key configuration and carry that binding through
recursive verification; checking a runtime field alone is insufficient. Native
admission must compare the same root before accepting affected Guard proofs.
Until that complete boundary and actual provider evidence pass, these schema
and provenance checks do not qualify a production device or enable runtime use.

Tests cover canonical root/path derivation, a full dense-tree oracle, all four
platform tags, boundary indices, missing/reordered/duplicate rows, substituted
commitments/roots, rejected secret/sibling fields, canonical issuer signing
bytes, OpenSSL signature interoperability, invalid scalars/points/infinity,
signed OEM/transcript substitution, and consistently changed provider rows and
roots with fresh observer approvals but stale issuer authorization. Receipt
roundtrips and positive replacement controls require fresh issuer and complete release
approval after changing a provider authority. All test authorities and physical
reports are synthetic fixtures.

The MintAuthorization inner semantic column carries 90 cells: the existing 84-cell
transport projection, both paired carrier commitments at `84..88`, and the exact
SHA-bound credential policy root at `88..90`. Both root limbs join the four inner
audit limbs and four carrier limbs in the same reciprocal audit binding. Each
compact outer circuit verifies its actual inner proof and copy-constrains those
root cells into `ProviderPolicyRootConfigV1`, whose constants form part of the
native-reconstructed constraint system. The public outer column remains 84 cells;
the compact wire format gains no caller-selected policy field.

MintAuthorization setup takes an explicit nonzero registry root and rejects a
credential under another root before constructing proof graphs. Setup inputs
remain non-authorizing. Production Eq/Ep key loaders obtain the root from
`KagemushaAuthenticatedArtifactSetV1`, require it to match the separately supplied
authenticated release, and reconstruct both PK/VK configurations with that root.
Proving requires the two loaded parities and relation to agree. Native protocol
admission checks the compiled outer identities against the approved helper pair.
Tests cover both Pasta fields, exact 90/84 projection, missing/substituted roots,
root-cell copy detachment, u128 bounds, and changed native VK identity; the real
mint diagnostic also attempts different genuine statements under reused keys.
These source changes and small constraint tests do not establish a passing real
mint corridor, provider qualification, or safe production enablement. Monetary
entry gates remain closed pending complete proof-chain and release evidence.

Guard exposes 44 public cells in one column: normalized statement digest at
`0..2`, Eq audit at `2..4`, Ep audit at `4..6`, predecessor credential digest at
`6..8`, successor credential digest at `8..10`, and complete history at `10..44`.
PlatformCredential retains its 40-cell column. Each Guard credential digest is
the exact SHA-bound statement consumed by its recursively verified credential
proof; detached archive annotations do not supply an opening. The private Guard
archive carries both digests, and native verification requires the caller's
expected pair and verifies those public cells in both parities.

Terminal authorization matches its reconstructed credential digests against
those actual Guard outputs. `credential_issuance_digest` is the canonical compact
hardware credential ID, consistently with MintAuthorization. The sender's exact
compact credential and governed profile preimages authenticate the validity
cells used by the terminal circuit. A trusted commit must fall at or after each
start and strictly before each expiry; a lease must be nonempty and wholly inside
both half-open intervals. Receipt wall time does not re-date a valid commitment.
The aggregate also supplies these exact credential digest cells to its Guard
verifier and allocates them, together with Guard history, in the recursive
loader's active virtual context. These changes require regenerated Guard,
aggregate and terminal keys and renewed release evidence.
