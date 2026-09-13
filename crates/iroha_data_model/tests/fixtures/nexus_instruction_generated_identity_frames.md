# Nexus instruction identity capture

`nexus_instruction_generated_identity_frames.json` was captured on 2026-09-07
before adding schema declarations to the 12 Nexus `model_single!` records.
The 24 populated cases preserve 96 complete root, `Vec`, `Option` and
`BTreeMap<u8, T>` frames, plus 24 canonical `InstructionBox` frames and their
24 JSON carriers. Each frame and supported JSON value is roundtripped during
capture and checking.

Eleven records have no direct JSON codec. Their canonical JSON representation
is the framed `InstructionBox` carrier, which is tested without introducing a
second public representation. `WithdrawFeeSponsorProgram` also has its existing
direct JSON codec, so its two direct JSON values are recorded separately.

The cases reuse the owner tests' deterministic accounts, sponsor programs,
rules, asset budgets, lane commitments and proof blobs. They vary program names,
source heights, revision numbers, optional proof expiry and positive fractional
quantities. The relay fixture has no commit QC and uses synthetic proof bytes;
this is wire-format evidence, not relay/proof admission or finality qualification.
The reserved effect-proof field remains absent; its production rejection policy
is unchanged.

Fixture SHA-256:
`f4a6f13e09de60f60f1947658a37cdf1de613829fb078d2056c025e0d7d214d5`.
Before registering the owner-scoped test module, `src/isi/nexus.rs` had SHA-256
`a04c38689d905903656ae92fad27f32df20e041bf38b8ba3aa71cf9b020690d3`.
Its production code and existing fixture constructors remained unchanged during
capture. Every actual compiler name and both directional hashes match the
historical generated-family review, SHA-256
`2e00a0c82cc36b5b14d97586bd392f4563fde0de303b7414724a2822a29f2778`.

The one-time writer was removed before adding the declarations. The permanent
`nexus_instructions_preserve_captured_frames` test checks the typed declarations,
both hashes and every complete frame/JSON carrier against this immutable capture.
The separate `isi!` caller in the same source is not included in this batch.
Complete generated/generic coverage, active codec cutover, other feature
selections and source-bound release qualification remain open.

On 2026-09-12, the native fixture producer refreshed contexts containing the
current default confidential-policy hash. The root payload changes were checked
to consist solely of that hash; outer frame checksums follow from the changed
payload. These synthetic frames do not qualify a network run.
