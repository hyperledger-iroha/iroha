# Concrete model identity fixtures

These files pin the current native concrete identities and canonical frames.
The `governance` and `http` fields record the capture configuration.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_concrete_identity_frames.json` | 11 populated families, each as root, Vec, Some and BTreeMap: 44 frames | `7f44e0868076371ec1360f85c99e91d37531b98b46149ffe08b493a0276cc7d3` |
| `block_message_send_identity_frame.json` | One encoding-only block-message adapter and its owned decoding projection | `255ea92e67fc28627401b397ce39762ccd8de1bb4e61811e84863793fe6f0429` |
| `reputation_event_id_identity_frames.json` | Two encoding-only reputation event-ID projections and their owned decoding material | `cd77d568a81af2df4523521af263ebbcc8757a1ee0bd654edd39da863690627d` |

The five concrete owners are Action, DataEvent, ExecutorContext,
BlockSubscriptionRequest and BlockMessage. Actions cover schedules with and
without retry and explicit execution. Data events cover peer addition, account
metadata, GameSession and Governance. The executor context contains populated
authority and header values. Stream cases cover two requested heights and a
deterministic signed block with complete typed network execution output.

The tests compare declared nominal names, both codec directions' frame hashes,
bare payloads, complete frames, frame digests and header flags with the saved
records. They also check exact decode/re-encode roundtrips, incorrect schema
headers and truncated frames. Governance is present in every build; its captured
frame must decode to the Governance variant. The schema test checks all 26
DataEvent name/discriminant pairs. Only the three HTTP families and the
block-message adapter require the `http` feature.

The context and signed genesis fixture builders share explicit pins for all five
confidential-feature fields. The policy hash is fixed codec input, independent of
the current genesis-policy default. Direct regressions compare the captured
context header and the final stream-block header and signature. Both the genesis
proposal and the result-bearing block check all five fields. The separate
`block_header_defaults_confidential_digest` and `genesis_defaults_confidential_digest`
tests cover the production default; changing that default must not silently change
these identity-fixture inputs.

BlockMessageSend delegates its root frame to BlockMessage. The borrowed reputation
adapter delegates to ReputationJournalEntryV1 after clearing the event ID. These
adapters are encoding-only: tests decode through their owning types, compare exact
bytes and retain the domain-separated reputation event-ID assertion.

The ignored native producer `concrete_identity_tests::capture_current_concrete_identity_frames`
prints `CONCRETE_CANONICAL_IDENTITY_CAPTURE=` and
`BLOCK_MESSAGE_CANONICAL_IDENTITY_CAPTURE=` JSON records. Run it explicitly after
reviewing an intentional model layout change:

```sh
cargo test --locked -p iroha_data_model --lib --features http concrete_identity_tests::capture_current_concrete_identity_frames -- --exact --ignored --nocapture --test-threads=1
```

The producer uses the normal signed-block constructor, validates entrypoint and
output commitments, and verifies the final signature before emitting frames. Its
record helpers perform canonical and bare roundtrips and reject wrong schemas
and truncations. Save those emitted records, then rebuild and run the ordinary
comparison tests because their fixtures are embedded with `include_str!`.

The current refresh changes only the block-message family and its send adapter;
the other ten concrete families are unchanged. The block root frame is 987 bytes.
The independent `query_wire_ids_v1.txt` inventory contains all 35 canonical
assignments from `define_builtin_query_registry!`, including FI context snapshots,
FI transfer receipts and escrow scoped originals. Its test compares the complete
sorted assignment list and separately requires unique wire identifiers. Registry
tests also reject type-name aliases and alternate IDs for built-in query types.

These checks establish fixture and codec behavior. Complete model suites and
release qualification must be evaluated against the current compiled candidate;
a fixture-generation pass alone does not establish them.
