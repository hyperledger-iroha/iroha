# RAM-LFE HTTP fixture migration

Date: 2026-09-29. Source migration is complete in
`crates/iroha_torii/src/tests/lib_tests/part_1.rs` and `part_3.rs`.
Normal Core and Torii package builds produced observed passes: 45/45 Core
controls and 35/35 Torii controls, with zero ignored tests and unchanged
captured sources and retained binaries. Their reused local dependency artifacts
lack retained provenance tying them to the same source capture, so these are
not closed source qualifications. A fixed-candidate rerun with dependency
provenance is pending. Source parsing, review and the codec guard also pass.

Both exact BFV backends are unavailable because of their noiseless public-key
equation. Existing tests must not register those policies successfully, evaluate
diagnostic encryption through a bypass, or sign a ciphertext hash as if it were
an opened plaintext hash. Ten obsolete fixture helpers for BFV parameters,
encryption, fabricated opening/canonicality and duplicate registration were
removed. No production admission bypass was added.

The migrated coverage is:

| Surface | Current control |
| --- | --- |
| Program and identifier policy lists | Real HKDF policy registration/activation through normal ISIs; active metadata is returned without BFV encryption/profile metadata. |
| Encrypted execute, resolve and claim receipt | Supported HKDF policies explicitly fail the unsupported encryption operation. |
| BFV policy admission | Both backend variants in signed and proof modes reject through normal registration; no policy is inserted. |
| Request preflight | Both backend/commitment positions, including mismatches, reject before invalid-hex parsing or runtime lookup; the actual response is HTTP 503 with `ram_lfe_encryption_unavailable` in its rejection header and Norito envelope. |
| Receipt verification | BFV payloads remain invalid even when the supplied output bytes match their hash; a separate HKDF metadata fixture retains expiry checks. |
| Execution DTO | Typed synthetic ciphertext metadata preserves receipt fields and has neither `output_hex` nor a fabricated `output_opening`. |
| Identifier DTO | Typed synthetic receipt fields and signatures serialize correctly; the synthetic opened plaintext hash is explicitly different from the ciphertext hash. This does not qualify execution or decryption. |
| Malformed requests | Invalid hex, invalid Norito bytes and a correctly rechecksummed but truncated inner payload reject without panicking. |
| Receipt lookup | A missing admitted claim returns 404; a separate typed record retains projection coverage without inserting a fabricated claim. |

All three token/authentication tests and the unrelated alias tests remain
byte-identical. The source audit preserves 77 unchanged functions in part 1
and 41 in part 3. Structured error assertions inspect `ValidationFail` causes
because Torii's outer error display intentionally omits those messages. The
public unavailable-backend mapping has its own response control; unrelated
signing failures remain HTTP 500 with the private error marker absent from the
public body.

Before/after sources, function inventories and source-only results are retained
under ignored `dist/zk-remediation/2026-09-29/ram-http-fixtures/`. The public
runtime error mapping passed its actual HTTP response controls. This qualifies
the listed refusals, metadata and authentication behavior, not successful BFV
encryption or release readiness.

Normal native evidence is retained under `boundary-native-20260929T074949Z/
core-native-controls` (45 controls, 133.314 seconds) and
`boundary-native-20260929T080331Z` (35 controls, 912.644 seconds including the
863.847-second Torii build). Both records guard 20,722 captured inputs. The
Core binary SHA-256 is
`092880da0146840777033de1bb53e3e3dc61ead37536ccbf5bfbe43bfbca0795`;
the Torii binary is
`f168f2edae15b878ead6e514555ade949512056b6fbdf7003545234751a789ec`.
The Torii candidate adds only removal of a retired `Snapshot.bootstrap` fixture
field to the already-qualified Core candidate. Earlier six-call `NewBlock`
fixture compilation errors and the retired snapshot field failure are retained
with their exact repairs; no failed evidence was overwritten.

A later dependency audit found Core reused 68 of 70 local artifacts, Torii
27 of 71, and Kagami 26 of 66; all seven proc macros were marked fresh by
Cargo. The earlier source/binary guards do not establish the source of those
reused objects. Their observed test results remain retained, but must not be
presented as exact-source native qualification. No prior evidence is erased.
