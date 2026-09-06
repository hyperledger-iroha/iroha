# KAGEMUSHA V1 physical-evidence closure

This is the implementation-coupled contract enforced by
`scripts/verify_kagemusha_v1_release_evidence.py`,
`scripts/verify_kagemusha_v1_physical_device.py`, and
`scripts/run_kagemusha_v1_release_evidence.py`. Synthetic unit-test transcripts
are not OEM evidence or hardware qualification.

## Non-circular identities

The small `hardware_profile_qualification_report` remains the hardware-profile
preimage: provider, policy epoch, run-derived verification identifier and exact
physical-check list. Its SHA-256 enters `HardwareProfileV1`, whose identity
enters the release candidate context. The signed raw physical transcript then
binds that context in `run.candidate_context_digest`, plus the exact release
artifact-set digest. Existing `run.candidate_digest` retains its separate meaning
as the candidate artifact recovered in the physical state-transition test.

The profile's manifest row MUST also contain `physical_evidence` with exactly:

| Field | Evidence kind | Meaning |
| --- | --- | --- |
| `transcript` | `physical_transcript` | Complete canonical, observer-approved physical transcript, including event chain. |
| `attestation` | `oem_attestation` | Complete raw OEM enrollment/service attestation bytes. |
| `trust_roots` | `oem_trust_roots` | Exact governed OEM verifier trust-root/policy bundle. |
| `observer_policy` | `observer_policy` | Byte-identical copy of the independently supplied, hash-pinned observer policy. |
| `oem_report` | `report` | Independent native OEM attestation verification result. |

The manifest's closed file inventory and verification observations commit all
five files and the small qualification report. The final release receipt binds
the manifest, observer policy and signed verification records. No transcript or
OEM report digest is inserted back into the hardware-profile preimage. This
ordering avoids a transcript/profile/report hash cycle.

## Independent trust and native OEM validation

The separately pinned observer policy MUST admit `physical-device-verifier`
with the exact SHA-256 of the fixed operator-local physical verifier and the
hardware qualification report schema. The release projector loads those
already authenticated source bytes using its own authenticated release module;
it never imports or executes candidate-provided source. It reruns the complete
transcript checker and observer-signature verification and requires the exact
derived qualification report. The collection runner binds this physical source
in its tooling closure and rechecks it before publication. The release projector
also rechecks the source before returning its projection.

The transcript must match the governed profile identity, provider, policy epoch,
capability mask, platform, qualification report, validity interval, release
candidate context and artifact inventory. Its attestation digest must match the
retained raw OEM bytes.

`iroha.kagemusha_v1.oem_attestation_verification_report` is a closed V1 JSON
report. Besides standard schema/version/verification identifier fields, it binds:

- The exact hardware profile, provider, epoch, capability mask and hardware policy.
- The device, product, firmware, OS build and platform seen in the transcript,
  and the governed product-class and firmware-policy digests.
- `attestation_verifier_sha256`, equal to the hardware profile's
  `enrollment_attestation_verifier_digest` and the actual threshold-observed
  verifier executable identity admitted by the independent observer policy.
- `{sha256, byte_len}` bindings for raw `attestation`, `trust_roots`, `transcript`
  and `observer_policy`. Root bytes must independently hash to the profile's
  `attestation_trust_roots_digest`. Both governed implementation/root fields
  use the SHA-256 of the exact retained implementation/root-bundle bytes for
  this release-evidence corridor.
- `challenge_sha256`, the SHA-256 of the domain
  `iroha:kagemusha:v1:physical-oem-challenge\0` followed by canonical JSON of
  `hardware_profile_id`, `device_id`, `firmware_digest`, `os_build_digest`,
  `candidate_context_digest`, `artifact_set_digest` and `run_id`. The native OEM
  verifier must require the raw attestation to authenticate this challenge.
  These are pre-attestation inputs, avoiding another hash cycle.
- Release candidate context, artifact inventory, run identifier, start/end times,
  and exact hardware-backed/production-build/no-software-fallback assertions.

The native OEM verifier invocation consumes exactly this report and those four
raw files. The separate physical qualification invocation consumes its small
report and all five sidecar files. Both invocations require independent
threshold-signed observations over the exact input bytes and current candidate
context; a command cannot be reused for both reports. A generic positive report,
a changed candidate, or self-reported hardware booleans cannot replace this
chain.

Actual OEM chain parsing, revocation, freshness/challenge verification and the
binding from native attestation claims to product/firmware policy are the
responsibility of the authorized, independently admitted OEM verifier. There
is no generic parser that establishes those properties for arbitrary iPhone,
Samsung, Huawei, Google or Meizu attestations. An OEM/laboratory must provide
and qualify that verifier, its controlled trust-root bundle and complete raw
physical evidence for each enabled profile. Without them the release gate
remains closed; this implementation supplies no qualified profile or attestation.

## Focused checks

Run `python3 -m pytest pytests/scripts/kagemusha_v1_physical_device_test.py
pytests/scripts/kagemusha_v1_release_evidence_test.py
pytests/scripts/kagemusha_v1_physical_provenance_test.py
scripts/tests/run_kagemusha_v1_release_evidence_test.py`. The tests cover raw-byte
substitution, independently pinned policy and roots, OEM report subject
substitution even after generic observer reapproval, missing transcript
signatures, rechecked event-chain semantics and candidate replay.
