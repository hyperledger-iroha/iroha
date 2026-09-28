# Concrete model identity fixtures

These files pin the current native concrete identities and frames. The capture
configuration enables governance and HTTP and disables `ids_projection`;
those fields describe the fixture provenance.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_concrete_identity_frames.json` | 12 populated families, each as root, Vec, Some and BTreeMap: 48 frames | `ba37c9b267242b2733972245fbeeca14b6e7653ebfd42f0b00b2ed261989b7d3` |
| `block_message_send_identity_frame.json` | One encoding-only block-message adapter and its owned decoding projection | `609a71f1cd38dd9b421b187cca8caaf6b38a533e0e2cf74b0d9e667f056d8cb5` |
| `reputation_event_id_identity_frames.json` | Two encoding-only reputation event-ID projections and their owned decoding material | `7a4bdb7eae4c9aca0351bd6549628e185d3e24da0aa03cf54669f9e853c14ae1` |

The six concrete owners are Action, DataEvent, SmartContractContext,
ExecutorContext, BlockSubscriptionRequest and BlockMessage.
Actions cover schedules with and without retry and explicit execution. Data events
cover peer addition, account metadata, GameSession and Governance. The two
contexts contain populated authority/header values. Stream cases cover two
requested heights and a deterministic signed block with transaction results.

`actual_type_name` retains the compiler name observed before declaration, including
private model scopes. The permanent tests compare each declared nominal name to
that saved name, both codec directions to the declared frame hash, and every bare
payload, complete frame, frame digest and header flag to the original record.
Roundtrips, incorrect schema headers and truncated frames are also checked.
Temporary capture writers have been removed.

The context and signed genesis fixture builders share explicit pins for all five
confidential-feature fields. The policy hash is fixed codec input, independent of
the current genesis-policy default. Direct regressions compare the captured
context header and the final stream-block header and signature; both the genesis
proposal and result-bearing block check all five fields. The separate
`block_header_defaults_confidential_digest` and `genesis_defaults_confidential_digest`
tests cover the current production default. Updating that default must not
silently change the input to these identity fixtures.

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

On 2026-09-12, the native fixture producer refreshed contexts containing the
current default confidential-policy hash. The root payload changes were checked
to consist solely of that hash; outer frame checksums follow from the changed
payload. These synthetic frames do not qualify a network run.

The HTTP block fixture was also refreshed through the native signed-block
constructor. Its bare bytes differ only at the 32-byte policy hash and resulting
64-byte block signature. All other bytes, entrypoint commitments and result
commitments are unchanged, and the new signature verifies.

On 2026-09-26, `SignedBlock` gained its trailing `commit_certificate` field
(Sumeragi finality proof; absent from the fixture block). The native producer
refreshed only the block-message family and the HTTP block-message adapter: each
frame gains the encoded `None` field and its length prefixes and checksums follow.
All other families are unchanged.

On 2026-09-28, the unused `TriggerContext` payload was removed together with the
retired Rust trigger SDK, and its `trigger-context` family (four frames) was
dropped from the capture. The same native-producer refresh follows the committed
`BlockHeader` extension with the trailing required `global_beacon_pulse_hash`
option (absent in these fixtures): `smart-contract-context` and
`executor-context` gain two bare bytes, and `block-message` and the block-message
adapter gain four. Nominal names and schema hashes are unchanged, and every other
family is byte-for-byte unchanged. Frame counts in the earlier run summaries
above predate this refresh.
