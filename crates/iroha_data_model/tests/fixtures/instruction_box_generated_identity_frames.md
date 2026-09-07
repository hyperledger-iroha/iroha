# Generated instruction-box frame capture

Captured on 2026-09-07 with the pinned workspace compiler, before adding any
`isi_box!` schema declaration or generator identity derive. The fixture is
immutable preparation evidence; active codecs have not been cut over.

The one-time unit capture first obtained actual compiler names and both codec
hashes. Existing instruction roundtrip values supplied populated variant
payloads; deterministic additional generic values cover metadata, grants,
revocations, mint/burn, transfers and a scheduled instruction trigger. Every
current box variant was then independently framed and decoded at its root, in
`Vec`, `Some`, `BTreeMap<u8, T>`, and through `InstructionBox`. Instruction-carrier
JSON also roundtripped. These are synthetic wire fixtures, not ledger admission,
proof-validation, signature custody or consensus-finality evidence.

The complete pre-declaration instruction selection passed 325 tests. The
additional exhaustive box capture passed separately. Every temporary writer,
probe and fixture hook was removed before inserting the declarations. All 53
prior instruction source files then matched their recorded hashes exactly;
after declaration, removing only the twelve literal attributes, one template
derive and new test registration also reproduced those original source hashes.

The permanent tests decode the frozen root bytes, compare the resulting variant
with an exhaustive Rust match, reject duplicate/missing variant cases, and
reconstruct every frame and JSON carrier for exact fixture comparison. No test
writer, inferred-name fallback or alternative accepted codec identity remains.

## Fixture

- File: `instruction_box_generated_identity_frames.json`
- SHA-256: `3932d6207469add1b3cdf1d8282fe860d488cb5d7c843b36a6bf0e3928b10418`
- Size: 156,306 bytes.
- Coverage: 12 types, 68 distinct variants, 340 complete frames, 68 JSON carriers.

| Type | Variants |
| --- | ---: |
| `iroha_data_model::isi::GrantBox` | 3 |
| `iroha_data_model::isi::RemoveKeyValueBox` | 5 |
| `iroha_data_model::isi::RevokeBox` | 3 |
| `iroha_data_model::isi::SetKeyValueBox` | 5 |
| `iroha_data_model::isi::defi::DeFiInstructionBox` | 12 |
| `iroha_data_model::isi::mint_burn::BurnBox` | 2 |
| `iroha_data_model::isi::mint_burn::MintBox` | 2 |
| `iroha_data_model::isi::register::RegisterBox` | 7 |
| `iroha_data_model::isi::register::UnregisterBox` | 7 |
| `iroha_data_model::isi::rwa::RwaInstructionBox` | 12 |
| `iroha_data_model::isi::settlement::SettlementInstructionBox` | 6 |
| `iroha_data_model::isi::transfer::TransferBox` | 4 |

## Before-declaration sources

| Owner | SHA-256 |
| --- | --- |
| `crates/iroha_data_model/src/isi/defi.rs` | `42a9f3b97c9f0e9f482d86d872db4448cb50f6adae100c86bd581ad2ac426a52` |
| `crates/iroha_data_model/src/isi/mint_burn.rs` | `a3eb02b60b6dec4ec869bdb68274286bac20c804ea3c0e29f61a30cd7040cd7f` |
| `crates/iroha_data_model/src/isi/mod.rs` | `867a57277307a2a69cf4f95ed39dbca8f0bda7583ce29fe632f32a0633646201` |
| `crates/iroha_data_model/src/isi/register.rs` | `9b8feecea85fe4e14e51db770568b93b25412abbff62c694173923c41636b6ab` |
| `crates/iroha_data_model/src/isi/rwa.rs` | `e7a072c9425896a88117582916cda039043a140468d5fbcd586088e73ceeb8a3` |
| `crates/iroha_data_model/src/isi/settlement.rs` | `66183cda49e96523c110454629e626fe72a7143b21d3ce63d1376de54934c64c` |
| `crates/iroha_data_model/src/isi/transfer.rs` | `d87e4f5c7b85ad4aa875037c61a3984307e4e1ab8e48bd11c94d1df891ebf8b7` |

The broader local compiler capture remains untracked under
`target/architecture-redesign/instruction-macro-current-before/`. Its raw
JSON-lines SHA-256 is
`b4e2a3f771b616193f790effa010b6a5cbdf08fb74a5540065ded01a3cebb973`.
It contains 433 concrete identities and additional instruction payload evidence;
this box fixture does not claim closure of the remaining `isi!` declarations,
manual codecs, generic markers, other feature selections or the release build.
