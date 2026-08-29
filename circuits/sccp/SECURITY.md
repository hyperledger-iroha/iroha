# Security reporting and release invariants

Do not deploy artifacts from this directory while the source-level semantic
implementation guard is false. A successful Groth16 equation is not evidence
that an SCCP transfer, Taira finality artifact, or epoch transition is valid
unless every final-V1 semantic constraint is present in the R1CS and the exact
ceremony/audit closure passes.

Never commit Phase-2 toxic waste, prover secrets, live-chain credentials,
guardian keys, audit signing keys, or beacon preimages. Do not accept an
arbitrary verification key at runtime. Verification keys and generated
verifiers must be profile/role-specific fixed release artifacts whose hashes
are bound by the signed deployment policy.

Report suspected vulnerabilities through the Hyperledger Iroha security
process rather than publishing exploitable details in an issue.

## BLS-normal verification

Every message and epoch finality statement verifies the aggregate BLS-normal
Commit-vote signature against the exact canonical vote bytes and the signer
keys selected by the exact `2f+1` bitmap. A message/current-roster path does not
repeat every individual PoP pairing: the exact ordered public-key and durable
PoP-hash commitment must match its authenticated epoch anchor.

An epoch-anchor transition verifies the complete successor-roster PoP batch
once. It does not accept a prover-supplied batching scalar. SHA-256 derives the
challenge from a domain tag, fixed profile and role, destination lane, complete
eleven-signal statement (including route configuration and anchors), epoch
bounds, ordered validator count and signer bitmap, all compressed successor
keys and PoPs, aggregate signature, and exact vote bytes. The first two digest
bits are cleared to obtain a uniform 254-bit value below the BLS12-381 scalar
modulus, and zero is rejected. Roster key digests are distinct and roster order
is included in the height-context identity, anchor, and batch transcript. Tests
include invalid PoPs whose errors cancel in an unweighted multi-pairing,
independent PoP reordering, and a full-roster permutation under an unchanged
public statement.

The BLS12-381 G1/G2 arithmetic, hash-to-curve, subgroup checks, scalar
multiplication, and pairing are emulated by pinned gnark gadgets in both outer
Groth16 fields. Run `sccp-circuits constraint-count --profile <closed-id>` to
record the exact constraint count for a source revision; this command emits no
key material. Constraint and prover resource ceilings are release blockers, not
grounds for weakening these equations.
