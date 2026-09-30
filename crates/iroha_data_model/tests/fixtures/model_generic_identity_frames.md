# Generic model identity fixtures

The current canonical model capture is reproduced by the explicit ignored unit
`generic_identity_tests::capture_current_generic_identity_frames`. Each of the
12 generic families and five concrete-argument cases has root, populated Vec,
Some and ordered BTreeMap frames: 68 frames in total.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_generic_identity_frames.json` | 68 current canonical populated frames | `982eb45b2c8c217ac66704a41e3fd7850f651a6e6b16d704fe0b28a94238f731` |
| `fhe_provenance_identity_frames.json` | Five actual FHE signing preimages | `bd71e00979ea12bbd3eff32144291cfd70ca474d717a768f91e2ad7e66bd7fb0` |

The generic owners are MetadataChanged, Validate and Mismatch. Concrete arguments
include RwaId, SignedTransaction, AnyQueryBox and TransactionDomain. InstructionBox
uses its canonical wire-ID/payload-pair root projection; containers retain the
nominal instruction identity. Metadata covers seven target aliases. Executor
inputs contain instructions, a query and a deterministic signed transaction.

Tests compare explicit nominal identities, both codec directions, adaptive
payloads and complete canonical frames. They decode and re-encode each frame,
and reject wrong headers and truncation. Marker-only checks require no payload
codec; equal String and Box<str> payloads retain distinct nominal arguments.

The borrowed FHE helper is encoding-only. Its erased lifetime slot is part of
its nominal identity. Tests compare its frames with the public signing-preimage
function and distinguish ordered and reversed execution proofs. Supplied proof
records and transaction values are codec fixtures and carry no authorization or
execution claim.

The exact preceding fixture and original documentation are preserved under
[the September 30 historical record](../../../../docs/history/2026-09-30/codec-fixtures-before-unit-repair/).
Earlier component test counts apply only to their original candidate. Current
captures and tests use the current canonical wire layout; historical frames are
never decoder inputs or compatibility paths.
