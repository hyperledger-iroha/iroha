# RAM-LFE execution-proof completion

Status: unimplemented. This is an implementation contract for the remaining
[ZK03 work](zk_first_release_goals.md), not a new proof format or an activation
decision. Encrypted policy registration, activation, restored-state validation
and receipts reject in both signed and proof modes. A secure encryption
replacement and complete, qualified relation are required before activation.

## Current implementation and trust boundary

The retained diagnostic [crypto interpreter](../crates/iroha_crypto/src/ram_lfe.rs) validates the
secret-bound policy, hidden program, registered parameters and encrypted input,
then executes the branchless tape. The closed
[BFV profile](../crates/iroha_crypto/src/fhe_bfv.rs) bounds it to 64 encrypted input
slots, four registers, 32 state lanes, 256 instructions, 64 outputs and
multiplicative depth 16. Its ring degree is 64, plaintext modulus is 257 and
ciphertext modulus is `257 * 2^48`.
The public backend tags are `bfv-affine-v1` and `bfv-programmed-v1`; they identify
the evaluator's semantics. Retired `sha3-256` tags are rejected. Exact hash and
initializer choices are specified by the compiled protocol and profile descriptor.

The exact-lift profile is insecure: reducing its public-key equation modulo 257
removes its plaintext-multiple noise. Signatures and execution proofs cannot
repair this encryption defect. Public evaluators and Core/Torii boundaries now
reject both BFV tags before private work; the HKDF PRF remains available.
Arithmetic regression tests use private diagnostic dispatch. Remaining exported
low-level BFV utilities still require retirement or a secure replacement.

Execution produces ciphertext. The former execute response incorrectly signed a
ciphertext hash as an opened-plaintext hash; that issuer and response field are
removed. An identifier's independent plaintext opening must come from its pinned
opening authority and bind the exact execution. This remains a trusted attestation,
not a decryption proof. See the [boundary repair](../docs/history/2026-09-29/ram-lfe-production-boundary.md).

The [Core receipt helper](../crates/iroha_core/src/smartcontracts/isi/ram_lfe.rs)
refuses the unavailable relation before parsing any proof or key. The former
generic verifier and four-payload-hash-limb acceptance path are removed. The
[identifier consumer](../crates/iroha_core/src/smartcontracts/isi/identifier.rs)
uses the same refusal, independently of policy registration and receipt preflight.
An otherwise valid replay-binding proof cannot establish program execution.
Policy validation also applies during
[state restoration](../crates/iroha_core/src/state/deserialize_core.rs).

No existing compiled relation supplies the missing semantics. Retired IVM
binding-only relations did not establish execution and provide no substitute for
the hidden-program relation. The BFV full-bootstrap verifier
requires full execution material, while its public-padding-only verifier rejects.
The existing Halo2 IPA engine, canonical key/envelope owners and bounded verifier
can be reused; a new semantic circuit still needs independent review. Proving the
current arithmetic does not qualify the separate BFV encryption or bootstrap
security claims documented in the crypto module.

## Required relation

The verifier-owned statement must bind the full trusted **policy hash**, hidden
program digest, parameter and evaluation-key digests, program identity and
associated data, exact input/output ciphertext hashes, and canonical receipt
payload. The current receipt payload lacks `policy_hash`; the future statement
must bind it separately or explicitly change the first-release payload. A proof
must not transfer between different secret commitments sharing program metadata.

Private witnesses must establish the policy's secret commitment, exact hidden
program encoding, initialized register reads, every instruction and memory
transition, output ordering, and exact BFV modular arithmetic and relinearization.
All coefficient, index, quotient, remainder and depth bounds belong in the
relation. Host-side interpreter checks alone cannot establish these facts.

The canonical hidden program is a validated immutable shared owner. Its sole
`HiddenRamFheProgramV1` frame contains fixed profile metadata followed by 1..256
48-byte instruction slots: six little-endian u64 words per instruction, with
all unused words zero. The typed builder writes into one bounded clearing tape;
the explicit byte/config readers enforce the same format and reject retired
enum-sequence frames. The relation must constrain every tag, operand, reserved
word and bound in this exact encoding. Generic archive decoding is deliberately
unavailable for this secret owner.

The retained diagnostic initializer uses a fixed BLAKE3 derive-key XOF schedule.
A borrowed canonical Norito frame binds the initializer descriptor, policy hash,
secret and associated data. Exactly 1,024 bytes become 32 consecutive big-endian
256-bit values, each reduced modulo 257 by 32 fixed byte folds. No library range
sampler or rejection loop defines protocol semantics. The published profile's
mandatory `initializer_descriptor_hash` commits the framing, contexts, dimensions
and bounds. Superseded profiles are rejected rather than assigned another mapping.
The source change and its focused native qualification are recorded in the
[bounded-initializer record](../docs/history/2026-09-29/ram-lfe-bounded-initializer.md).

Secret commitment and private tape hashing now use separate BLAKE3 contexts and
clearing owned hash/XOF state. The outer policy and tape digests remain properly
typed Iroha Blake2b hashes of public commitments. Policy, program and dependent
output vectors change explicitly; parameter and evaluation-key algorithms do not.
The outer policy commits the canonical `PolicyCommitmentInputV1` frame, in field
order: backend, normalized public-parameter bytes and secret commitment. The PRF
uses the canonical `HkdfRequestInputV1` frame: policy hash, public parameters,
associated data and normalized input. These explicit first-release identities
replace ambient-layout tuples. Borrowed interpreter fields and owned reference
fixtures must produce identical frames; no reference-schema alias is introduced.
Torii hashes the canonical ciphertext frame independently of ambient decoder
flags. See the [canonical-transcript repair](../docs/history/2026-09-29/ram-lfe-canonical-transcripts.md)
for the exact changes and pending validation.
The execution trace comes from the sole interpreter and owns clearing snapshots
of its registers and memory. It is private prover input, not execution evidence.
The future circuit must constrain these exact hash/Norito/fold semantics and all
machine transitions; arbitrary initialized-state witnesses remain unacceptable.

## Implementation and acceptance criteria

1. Replace the insecure exact-lift encryption profile and independently qualify
   its security. Define the replacement statement, witness and bounded derivation;
   retain canonical Norito encoding without a legacy decoder or alternate relation.
2. Emit an owned, clearing execution trace from the existing interpreter. Cover
   all eleven instructions and maximum shapes with independent reference vectors
   before circuit synthesis. Trace generation alone is not proof completion.
3. Implement the complete fixed-profile circuit over the existing proof engine,
   with a dedicated relation identity, canonical key owner and typed verifier.
   Constrain encoding/hash preimages and initial-state derivation as well as the
   machine transitions. Do not use unconstrained native arithmetic callbacks.
4. Pass real native positive proofs and direct-witness negatives for altered
   secret/policy/program/parameters/keys, input/output/associated data/receipt,
   omitted or repeated transitions, invalid register or memory access, modular
   aliases, wrong relinearization and incorrect `SelectEqZero`. Qualify maximum
   proof size, prover/verifier work and memory, and error/unwind witness erasure
   under unchanged admission limits. Independently review soundness and hiding.
5. Integrate a canonical Torii producer without caller-selected circuits or keys,
   and one purpose-specific verifier shared by stateless and identifier receipt
   consumers. Qualify registration, activation, restoration and receipt mutations
   on the same source candidate, including a four-validator network. Only then
   replace the unavailable-relation refusal. Keep the output-opening signature's
   separate trust contract explicit; execution proof is not decryption proof.

Current source review and implementation status are also summarized in the
[audit matrix](zk_audit_matrix.md). There is no safe adapter-only completion or
payload-hash proof substitute for these requirements.
