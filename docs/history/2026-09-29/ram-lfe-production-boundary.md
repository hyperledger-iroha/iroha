# Encrypted RAM-LFE production boundary repair

Date: 2026-09-29. Implementation and scoped crypto qualification; encrypted
RAM-LFE remains unavailable. This is not a replacement encryption construction
or completion of the RAM execution relation.

The retained exact-lift BFV profile has q = 257 * 2^48 and errors divisible by
257. Reduction of the public-key equation modulo 257 removes its noise.
This source-confirmed defect cannot be repaired by a resolver signature,
execution proof, or a qualification document. No public-key recovery experiment
was executed. The subsequent first-release API cut retired 17 generic and
specialized seeded constructors into explicit development fixtures, and Soracloud
production admission/restore now refuses FHE. Raw diagnostic arithmetic remains;
a qualified secure replacement is still required before encrypted capability
can ship. See [the containment record](bfv-soracloud-production-containment.md).

`RamLfeBackend::require_production_support` returns the typed
`InsecureBfvProfile` error for both exact BFV tags. The three public evaluator
entry points check it before secret, request, program, or trace work. The
supported HKDF PRF remains available. Private diagnostic dispatch retains the
existing arithmetic, tape, canonical transcript and owner-erasure regressions.
Core policy validation, restoration, receipts and identifier admission use the
same boundary; Torii checks policy, commitment and receipt/draft backends before
runtime access, decoding, or signing. The isolated successor described below
now passes focused Core/model/Torii qualification; broader same-candidate
SDK/network qualification remains separate.

The old execute service also signed the ciphertext hash as
`opened_output_hash`. Its issuer and execute-response `output_opening` are
removed. Ciphertext and its execution receipt do not supply decrypted plaintext.
Independently supplied identifier openings keep their pinned-key, context,
signature and lifetime checks; the execute endpoint no longer fabricates one.
JS, Swift, Kotlin and Java response models and parsers follow the hard cut.
The three authored OpenAPI mirrors contain only ciphertext and receipt fields;
this source edit is not a compiled projection or signed publication.

## Validation

- Normal frozen crypto build and all 68 RAM-LFE controls pass, zero ignored,
  zero source drift. Four new controls exercise backend availability, refusal
  before malformed private inputs, trace refusal and unchanged HKDF behavior.
  Binary SHA-256:
  `87b79f9fc03677de5bc4df949da4890955d38d3e3c907c754e3fd1cc8a653822`.
  Warm build plus tests: 150.526 seconds. Evidence:
  `dist/zk-remediation/2026-09-29/ram-lfe-production-boundary/20260929T070514Z`.
- The candidate uses base `24789950586efd7fa3a6cc73a96f4acd5d760bf9`
  with 1,720 captured source inputs overlaid exactly. The earlier moving-main
  run also passed 68 tests but its guard rejected 23 source changes during
  an external merge. That rejected record remains at `20260929T065900Z`.
  Neither run qualifies the subsequently merged Core/workspace.
- All 63 focused OpenAPI/public SDK contract checks pass with the repository's
  pinned Python requirements. The new JSON Schema control accepts a
  ciphertext-only response, rejects the retired opening field, and compares
  its fields to the actual Rust DTO. Evidence: `.../openapi/tests.xml` and
  `.../openapi/amendment.json` beneath the boundary evidence directory.
  Initial system-Python collection failed for a missing `jsonschema` dependency;
  no tests ran in that attempt.
- Normal Kotlin production and Kotlin/Java test compilation pass. A stale
  `admissionIntent` argument in the sponsor roundtrip test was removed after
  the first compilation failed; all sponsor assertions remain. The normal
  test task then refused two missing required Sumeragi execution fixtures.
  Zero runtime tests and no stale JUnit output are counted as passes.
  Both attempts and source manifests are retained in `sdk-current` and
  `sdk-current-repair`, with zero source drift. JDK 8 API restrictions remain.
- JS/Swift scoped syntax, lint and parser-source checks are recorded separately
  by the SDK owner. Current-main JS correctly refused stale native provenance;
  preserved-candidate refreshes do not qualify current Core or network state.

Core's former successful BFV admission fixtures are replaced by 13 component and
refusal controls. The existing account/lifetime/index transition now has a
private verified-input owner constructed only after production admission checks.
Component tests preserve persisted claim, revoke and rebind index assertions;
actual registration, activation and claim entry points must refuse insecure
metadata without changing indexes. Opening and phone-attestation checks remain
independent. Correctly re-signed opaque-ID and receipt-hash mutations exercise the
actual binding predicate for both email and phone policies. Independent source
review found and repaired lost predicate and persisted-index coverage. The
initial shared-cache results (45 Core and 35 Torii passes) were only observed
binary outcomes because complete same-source dependency provenance was not retained. They are historical
evidence, not qualification. Exact predecessor source and review remain in
`.../core-fixtures/`; the successor below uses its own proven dependency closure.

The secure encryption replacement, complete hidden-program relation,
independent cryptographic review, and same-candidate SDK/network qualification
remain open in [the goals](../../../specs/zk_first_release_goals.md).

## Superseding source-bound component evidence

The separately frozen containment candidate passed 349 native crypto controls
and 37 independent default-feature API import rejections. Its later normal
Core/model/Torii build used a candidate-specific target begun empty, with every
reused local artifact matched to an already proven source/path/hash lineage.
Core passed all 367 required controls, including identifier refusal, stored-index,
restore and Soracloud containment assertions. Model execution passed all 385
required controls and 11 additional named instruction controls (396 total).
Every mandatory name ran exactly once with zero ignored tests. Torii initially
passed 35/36; the stale unsupported-HKDF error assertion remains a recorded failure.

The independently reviewed one-file assertion correction then passed the same
36 exact Torii controls after a normal rebuild. Source and immutable binary
checks passed, including all 71 local compiler artifacts. Evidence:
`dist/zk-remediation/2026-09-29/bfv-fhe-torii-fixture-retry-20260929T115654Z`.
Source manifest SHA-256:
`ac7741554a10fb026c4af6f1b107eb618c6f071b68fd6c509f8fcb82eef7e5dd`.
Binary SHA-256:
`1431a6c1f3ae370c41d28de6ca4596ef5845f150b2208556c2e0d0a91d55a8e1`.
The complete predecessor, exact execution-name audit and dependency provenance
are linked from [the containment record](bfv-soracloud-production-containment.md).
These results close the scoped production-refusal boundary; they do not enable
BFV or claim a secure encryption replacement, complete RAM proof, network pass
or release readiness.
