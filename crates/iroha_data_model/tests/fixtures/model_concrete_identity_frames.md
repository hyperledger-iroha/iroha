# Concrete model identity fixtures

These files preserve the successful pre-declaration captures byte for byte. The
capture configuration enabled governance and HTTP and disabled `ids_projection`;
those fields describe the original evidence, not the features required to read it.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_concrete_identity_frames.json` | 13 populated families, each as root, Vec, Some and BTreeMap: 52 frames | `85a794d3976dac15aeb6a351938a0a7f76f264b3a3d1ecbd9b4e26adcbda599d` |
| `block_message_send_identity_frame.json` | One encoding-only block-message adapter and its owned decoding projection | `1b8e8810a594ef6e610f7cb6aa7c012be1a88230af938f6ad2bf874f7172e97d` |
| `reputation_event_id_identity_frames.json` | Two encoding-only reputation event-ID projections and their owned decoding material | `7a4bdb7eae4c9aca0351bd6549628e185d3e24da0aa03cf54669f9e853c14ae1` |

The seven concrete owners are Action, DataEvent, SmartContractContext,
ExecutorContext, TriggerContext, BlockSubscriptionRequest and BlockMessage.
Actions cover schedules with and without retry and explicit execution. Data events
cover peer addition, account metadata, GameSession and Governance. The three
contexts contain populated authority/header/event values. Stream cases cover two
requested heights and a deterministic signed block with transaction results.

`actual_type_name` retains the compiler name observed before declaration, including
private model scopes. The permanent tests compare each declared nominal name to
that saved name, both codec directions to the declared frame hash, and every bare
payload, complete frame, frame digest and header flag to the original record.
Roundtrips, incorrect schema headers and truncated frames are also checked.
Temporary capture writers have been removed.

BlockMessageSend delegates its root frame to BlockMessage. The private borrowed
reputation adapter delegates to ReputationJournalEntryV1 after clearing the event
ID. These adapters remain encoding-only: the tests decode through their owning
types, compare exact bytes and retain the domain-separated event-ID assertion.
No adapter decoder or alternate frame format is introduced.

Feature selections reuse the same canonical common frames. Only the Governance
family is omitted when governance is disabled; the three HTTP families and the
block adapter require HTTP. The disabled-governance test also rejects the captured
Governance frame. A separate pre-declaration consumer capture exposed GameSession
shifting from tag 22 to 21 when governance was absent. That shifted frame is not a
canonical fixture. DataEvent now reserves all 23 default discriminants; the schema
test checks every present name/tag pair and preserves the absent Governance slot.

These tests qualify identity and codec behavior, not transaction acceptance,
bridge proof verification, guest ABI delivery or release readiness.

The default/HTTP model-library run passes 3,593 tests with zero failures and six
existing fixture generators ignored; all 5,617 selected inputs remain unchanged.
The same three public tests pass in a normal-dependency consumer with governance
actually disabled, using locked/offline Cargo and unchanged package versions,
features and edges. All 5,624 selected inputs remain unchanged for that run.
Both runs unset `RUST_MIN_STACK`. The isolated consumer checks all 48 common
root/container frames, the block-send projection, all 22 available schema
variants and rejection of the reserved Governance frame. These are local codec
checks; complete release qualification remains open.
