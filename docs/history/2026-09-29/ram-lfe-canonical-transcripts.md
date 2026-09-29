# RAM-LFE canonical transcript repair

Date: 2026-09-29. Status: 65 native controls and two API doctests pass; integrated
Torii and complete current SDK qualification remain open.

Some RAM-LFE transcripts used `norito::to_bytes`, which inherits active decode
layout flags. The same semantic policy or request could therefore produce a
different commitment, PRF result or input ciphertext hash inside a decoder scope.
Canonical secret/tape commitments and output envelopes already selected the
canonical layout; the remaining outer transcripts now do the same.

## First-release encoding

- `PolicyCommitmentInputV1` declares the exact frame identity
  `iroha_crypto::ram_lfe::PolicyCommitmentInputV1`. Fields are backend,
  public-parameter bytes and the 32-byte secret commitment, in that order.
- `HkdfRequestInputV1` declares the exact frame identity
  `iroha_crypto::ram_lfe::HkdfRequestInputV1`. Fields are policy hash,
  public-parameter bytes, associated data and normalized input, in that order.
- Both BFV policy constructors decode and validate their public parameter frame,
  then re-encode it canonically before committing. The affine constructor now
  rejects malformed parameter frames immediately.
- Torii serializes encrypted input canonically before deriving its receipt hash
  bindings. Interpreter output was already canonical.

The named frames intentionally replace the former tuple identities. There is no
compatibility decoder, alias or alternative commitment path. Policy hashes and
dependent outputs must be regenerated from the new source. Parameter and
evaluation-key derivation algorithms do not change.

The public BFV backend names now describe their evaluator: `BfvAffineV1` /
`bfv-affine-v1` and `BfvProgrammedV1` / `bfv-programmed-v1`. Misleading SHA3-256
names and JSON aliases are removed across Rust, supported SDKs and shared fixtures.
The enum's binary ordering and explicit Norito schema name remain unchanged.
The existing Java duplicate receives the same wire-tag replacement without adding
an implementation surface; Kotlin remains its canonical replacement owner.
Backups and exact replacement hashes are retained under
`dist/zk-remediation/2026-09-29/ram-backend-names/`.

## Private storage

The PRF borrows request fields and writes its canonical frame directly into one
preallocated `Zeroizing<Vec<u8>>` through a fixed slice cursor. Partial writes
remain owned and clear on error or unwind. HKDF receives borrowed info components
through `expand_multi_info`, eliminating concatenated private-frame copies.
This scope does not claim erasure of the original caller request, returned output,
HKDF internal state or compiler-created copies.

## Validation plan and evidence

New controls compare commitments and PRF results under canonical and noncanonical
ambient flags, compare borrowed frames with independently declared owned reference
records, and compare multipart HKDF expansion with the concatenation contract.
BFV controls exercise both advertised parameter layouts and malformed frames.
A passing native ciphertext control confirms the registered 64-slot canonical
frame is exactly 75,187 bytes.
Torii's existing programmed execution test also compares output and all input,
output and receipt hashes across ambient layouts.

The first prepared patch incorrectly used references inside tuple schemas.
Independent review caught both missing `&Vec` serialization and changed nominal
frame identity before native validation. The named slice-backed records replace
that patch; Norito's global reference identity rules remain intact.

Prepared sources, before hashes, patch and application receipt are retained under
`dist/zk-remediation/2026-09-29/ram-canonical-transcripts-prepared/`.
The fixed FASTPQ/network candidate predates this repair and remains unchanged;
its qualification does not cover this transcript amendment.

The normal default-feature `iroha_crypto` build and all 65 selected native controls
pass without ignored tests or source drift. Both private-owner doctests pass,
including the compile-fail boundary. Build plus native execution took 203.70
seconds, and doctests took 45.36 seconds. The retained test binary SHA-256 is
`32af8c3ad619e217cd9c6c1149c8e246a0a05cb631cc160df840742b23000fd6`.
Evidence is under `dist/zk-remediation/2026-09-29/ram-canonical-native/20260929T054217Z/`.

The backend-name refresh passes 466 normal C# tests with no skips and two Python
identifier controls. Python uses the normally installed, previously qualified
ABI-24 wheel; this checks current receipt encoding, not current native BFV code.
Kotlin production and test sources compile, but its mandatory test inputs reference
three deleted Sumeragi fixtures, so no Kotlin test pass is claimed for this stage.
The preserved JavaScript SDK candidate now passes its normal native rebuild,
distribution build, all 42 selected identifier/RAM controls (zero skips) and
scoped lint after the exact backend-tag amendment. Its 20,879-file source guard
has no drift; retained native SHA-256 is
`acbf5b0443fa5fd15068bdd8ffd47b72e4e38d488f6a590bf828cf6107e28484`.
Evidence is in `dist/zk-remediation/2026-09-29/sdk-backend-tag-refresh/javascript/`.
This checks the preserved SDK plus tag changes, not current Core, canonical
transcript runtime or the later key-owner repair. Swift's normal five-slice
refresh remains active. Core's known Sumeragi migration failures still prevent
the integrated Torii control.
